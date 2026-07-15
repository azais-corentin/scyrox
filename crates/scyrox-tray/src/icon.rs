//! System-font discovery and RGBA tray-icon rendering.
//!
//! Every state renders into a 64x64 transparent [`tiny_skia::Pixmap`], which is
//! converted to a straight-RGBA [`tray_icon::Icon`] at the end. Plasma/Wayland
//! (SNI) does not render text labels next to the icon, so the battery
//! percentage must be rasterized into the pixmap itself.

use ab_glyph::{Font, FontVec, OutlinedGlyph, PxScale, ScaleFont, point};
use anyhow::{Context, Result, anyhow};
use tiny_skia::{FillRule, Paint, PathBuilder, Pixmap, PremultipliedColorU8, Stroke, Transform};
use tracing::warn;

use crate::state::{TrayState, is_low};

/// Icon canvas size (square). Appindicator/Plasma downscales pixmaps to the
/// panel size; 64px keeps digits crisp after downscale.
const SIZE: u32 = 64;

type Rgb = (u8, u8, u8);

const WHITE: Rgb = (255, 255, 255);
const LOW: Rgb = (238, 68, 68);
const AMBER: Rgb = (255, 179, 0);
const GREY: Rgb = (158, 158, 158);

/// Discover a usable bold sans-serif system font.
///
/// Prefers a bold sans-serif, falls back to regular weight, then to the first
/// available face. A desktop with no font at all is broken, so this bails
/// rather than substituting a silent no-op.
pub fn load_font() -> Result<FontVec> {
    let mut db = fontdb::Database::new();
    db.load_system_fonts();

    let sans_serif = |weight: fontdb::Weight| fontdb::Query {
        families: &[fontdb::Family::SansSerif],
        weight,
        ..Default::default()
    };

    let id = db
        .query(&sans_serif(fontdb::Weight::BOLD))
        .or_else(|| db.query(&sans_serif(fontdb::Weight::NORMAL)))
        .or_else(|| db.faces().next().map(|face| face.id))
        .ok_or_else(|| anyhow!("no usable system font found"))?;

    let (source, index) = db
        .face_source(id)
        .ok_or_else(|| anyhow!("queried font face has no source"))?;

    let data: Vec<u8> = match &source {
        fontdb::Source::Binary(bytes) => bytes.as_ref().as_ref().to_vec(),
        fontdb::Source::SharedFile(_, bytes) => bytes.as_ref().as_ref().to_vec(),
        fontdb::Source::File(path) => {
            std::fs::read(path).with_context(|| format!("reading font file {}", path.display()))?
        }
    };

    FontVec::try_from_vec_and_index(data, index)
        .map_err(|e| anyhow!("failed to parse system font: {e}"))
}

/// Render the tray icon for a given state.
pub fn render(
    state: &TrayState,
    font: &FontVec,
    low_battery_threshold: Option<u8>,
) -> tray_icon::Icon {
    let mut pixmap = Pixmap::new(SIZE, SIZE).expect("64x64 is a valid pixmap size");

    match state {
        TrayState::Battery {
            percentage,
            charging,
            ..
        } => {
            if *charging {
                draw_bolt(&mut pixmap);
            } else {
                let color = battery_text_color(state, low_battery_threshold);
                render_text(&mut pixmap, &percentage.to_string(), color, font);
            }
        }
        TrayState::Disconnected | TrayState::DaemonDown => draw_mouse(&mut pixmap),
    }

    to_icon(pixmap)
}

fn battery_text_color(state: &TrayState, low_battery_threshold: Option<u8>) -> Rgb {
    if low_battery_threshold.is_some_and(|threshold| is_low(state, threshold)) {
        LOW
    } else {
        WHITE
    }
}

/// Convert the premultiplied pixmap into a straight-RGBA `tray_icon::Icon`.
fn to_icon(pixmap: Pixmap) -> tray_icon::Icon {
    let rgba = pixmap.take_demultiplied();
    tray_icon::Icon::from_rgba(rgba, SIZE, SIZE).expect("64x64 RGBA buffer is a valid icon")
}

// =============================================================================
// Text rendering
// =============================================================================

const TRIAL_SCALE: f32 = 48.0;
const MAX_TEXT_W: f32 = 60.0;
const MAX_TEXT_H: f32 = 56.0;

/// Lay out `text` and blend it (with a soft drop shadow for legibility on light
/// panels) centered into the pixmap.
fn render_text(pixmap: &mut Pixmap, text: &str, color: Rgb, font: &FontVec) {
    // Measure at a trial scale, then uniformly rescale so the text block fits
    // the target box, and lay out again at the final scale.
    let Some((_, trial_w, trial_h)) = layout(font, TRIAL_SCALE, text) else {
        warn!(text, "no renderable glyphs for tray icon text");
        return;
    };
    let factor = (MAX_TEXT_W / trial_w).min(MAX_TEXT_H / trial_h);
    let final_scale = TRIAL_SCALE * factor;

    let Some((glyphs, w, h)) = layout(font, final_scale, text) else {
        return;
    };

    // Center the union bounds in the canvas.
    let (min_x, min_y) = union_min(&glyphs);
    let off_x = (SIZE as f32 - w) / 2.0 - min_x;
    let off_y = (SIZE as f32 - h) / 2.0 - min_y;

    // Shadow first (black, semi-transparent, offset), then the colored pass.
    draw_glyphs(
        pixmap,
        &glyphs,
        off_x + 2.0,
        off_y + 2.0,
        (0, 0, 0),
        180.0 / 255.0,
    );
    draw_glyphs(pixmap, &glyphs, off_x, off_y, color, 1.0);
}

/// Lay out glyphs at a scale, returning the outlined glyphs plus the union
/// bounding-box width and height. `None` when nothing is renderable.
fn layout(font: &FontVec, scale: f32, text: &str) -> Option<(Vec<OutlinedGlyph>, f32, f32)> {
    let px = PxScale::from(scale);
    let scaled = font.as_scaled(px);

    let mut pen_x = 0.0f32;
    let mut glyphs = Vec::with_capacity(text.len());
    for ch in text.chars() {
        let id = font.glyph_id(ch);
        let glyph = id.with_scale_and_position(px, point(pen_x, scaled.ascent()));
        pen_x += scaled.h_advance(id);
        if let Some(outlined) = font.outline_glyph(glyph) {
            glyphs.push(outlined);
        }
    }

    if glyphs.is_empty() {
        return None;
    }

    let (min_x, min_y) = union_min(&glyphs);
    let (max_x, max_y) = union_max(&glyphs);
    Some((glyphs, (max_x - min_x).max(1.0), (max_y - min_y).max(1.0)))
}

fn union_min(glyphs: &[OutlinedGlyph]) -> (f32, f32) {
    let mut min_x = f32::MAX;
    let mut min_y = f32::MAX;
    for g in glyphs {
        let b = g.px_bounds();
        min_x = min_x.min(b.min.x);
        min_y = min_y.min(b.min.y);
    }
    (min_x, min_y)
}

fn union_max(glyphs: &[OutlinedGlyph]) -> (f32, f32) {
    let mut max_x = f32::MIN;
    let mut max_y = f32::MIN;
    for g in glyphs {
        let b = g.px_bounds();
        max_x = max_x.max(b.max.x);
        max_y = max_y.max(b.max.y);
    }
    (max_x, max_y)
}

fn draw_glyphs(
    pixmap: &mut Pixmap,
    glyphs: &[OutlinedGlyph],
    off_x: f32,
    off_y: f32,
    color: Rgb,
    alpha_mul: f32,
) {
    for g in glyphs {
        let bounds = g.px_bounds();
        g.draw(|gx, gy, coverage| {
            let x = (bounds.min.x + gx as f32 + off_x).round() as i32;
            let y = (bounds.min.y + gy as f32 + off_y).round() as i32;
            blend_pixel(pixmap, x, y, color, coverage * alpha_mul);
        });
    }
}

/// Source-over blend a straight-color sample into the premultiplied pixmap.
fn blend_pixel(pixmap: &mut Pixmap, x: i32, y: i32, color: Rgb, coverage: f32) {
    let a_s = coverage.clamp(0.0, 1.0);
    if a_s <= 0.0 {
        return;
    }
    let (w, h) = (pixmap.width() as i32, pixmap.height() as i32);
    if x < 0 || y < 0 || x >= w || y >= h {
        return;
    }

    let idx = (y * w + x) as usize;
    let pixels = pixmap.pixels_mut();
    let dst = pixels[idx];
    let inv = 1.0 - a_s;

    // Premultiplied output: src (premultiplied) over dst (premultiplied).
    let out = |src_straight: u8, dst_premul: u8| -> u8 {
        (src_straight as f32 * a_s + dst_premul as f32 * inv)
            .round()
            .clamp(0.0, 255.0) as u8
    };

    let r = out(color.0, dst.red());
    let g = out(color.1, dst.green());
    let b = out(color.2, dst.blue());
    let a = (255.0 * a_s + dst.alpha() as f32 * inv)
        .round()
        .clamp(0.0, 255.0) as u8;

    if let Some(p) = PremultipliedColorU8::from_rgba(r, g, b, a) {
        pixels[idx] = p;
    }
}

// =============================================================================
// Shape rendering
// =============================================================================

/// Amber lightning bolt (charging).
fn draw_bolt(pixmap: &mut Pixmap) {
    let mut pb = PathBuilder::new();
    pb.move_to(36.0, 2.0);
    pb.line_to(14.0, 36.0);
    pb.line_to(28.0, 36.0);
    pb.line_to(24.0, 62.0);
    pb.line_to(50.0, 24.0);
    pb.line_to(34.0, 24.0);
    pb.close();

    if let Some(path) = pb.finish() {
        let mut paint = Paint::default();
        paint.set_color_rgba8(AMBER.0, AMBER.1, AMBER.2, 255);
        paint.anti_alias = true;
        pixmap.fill_path(
            &path,
            &paint,
            FillRule::Winding,
            Transform::identity(),
            None,
        );
    }
}

/// Grey mouse silhouette (disconnected / daemon down).
fn draw_mouse(pixmap: &mut Pixmap) {
    let mut paint = Paint::default();
    paint.set_color_rgba8(GREY.0, GREY.1, GREY.2, 255);
    paint.anti_alias = true;
    let stroke = Stroke {
        width: 5.0,
        ..Default::default()
    };

    // Body: rounded-rect stroke.
    let mut body = PathBuilder::new();
    rounded_rect(&mut body, 18.0, 6.0, 28.0, 52.0, 14.0);
    if let Some(path) = body.finish() {
        pixmap.stroke_path(&path, &paint, &stroke, Transform::identity(), None);
    }

    // Scroll wheel line.
    let mut wheel = PathBuilder::new();
    wheel.move_to(32.0, 14.0);
    wheel.line_to(32.0, 26.0);
    if let Some(path) = wheel.finish() {
        pixmap.stroke_path(&path, &paint, &stroke, Transform::identity(), None);
    }
}

/// Append a rounded-rectangle contour to a path builder (cubic-bezier corners).
fn rounded_rect(pb: &mut PathBuilder, x: f32, y: f32, w: f32, h: f32, r: f32) {
    // Cubic control-point distance for a circular arc quadrant.
    const KAPPA: f32 = 0.552_284_8;
    let k = r * KAPPA;
    let (l, t, right, b) = (x, y, x + w, y + h);

    pb.move_to(l + r, t);
    pb.line_to(right - r, t);
    pb.cubic_to(right - r + k, t, right, t + r - k, right, t + r);
    pb.line_to(right, b - r);
    pb.cubic_to(right, b - r + k, right - r + k, b, right - r, b);
    pb.line_to(l + r, b);
    pb.cubic_to(l + r - k, b, l, b - r + k, l, b - r);
    pb.line_to(l, t + r);
    pb.cubic_to(l, t + r - k, l + r - k, t, l + r, t);
    pb.close();
}

#[cfg(test)]
mod tests {
    use super::*;

    // Renders every state; a panic (e.g. `Icon::from_rgba` rejecting the
    // buffer) fails the test. Uses real system fonts, present in the dev shell.
    #[test]
    fn render_all_states() {
        let font = load_font().expect("system font available in dev shell");
        let states = [
            TrayState::DaemonDown,
            TrayState::Disconnected,
            TrayState::Battery {
                percentage: 85,
                voltage_mv: 3900,
                charging: false,
            },
            TrayState::Battery {
                percentage: 15,
                voltage_mv: 3500,
                charging: false,
            },
            TrayState::Battery {
                percentage: 15,
                voltage_mv: 3500,
                charging: true,
            },
            TrayState::Battery {
                percentage: 100,
                voltage_mv: 4200,
                charging: false,
            },
        ];
        for state in states {
            let _with_threshold = render(&state, &font, Some(10));
            let _without_threshold = render(&state, &font, None);
        }
    }

    #[test]
    fn low_battery_color_requires_daemon_threshold() {
        let state = TrayState::Battery {
            percentage: 10,
            voltage_mv: 3500,
            charging: false,
        };

        assert_eq!(battery_text_color(&state, Some(10)), LOW);
        assert_eq!(battery_text_color(&state, None), WHITE);
    }
}
