//! Scyrox system tray battery indicator.
//!
//! Ensures the `scyroxd` daemon is running (spawning it detached if not), then
//! displays the mouse battery level as rendered text in the system tray — or a
//! charging icon while plugged in — updating live from the daemon's event
//! stream. It is a gRPC client of the daemon and leaves it running on quit.

mod daemon;
mod icon;
mod notifications;
mod state;

use anyhow::Result;
use tao::event::{Event, StartCause};
use tao::event_loop::{ControlFlow, EventLoopBuilder};
use tracing::{error, info};
use tracing_subscriber::EnvFilter;
use tray_icon::TrayIconBuilder;
use tray_icon::menu::{Menu, MenuEvent, MenuItem, PredefinedMenuItem};

use crate::state::TrayState;

/// Events funneled into the `tao` event loop.
enum UserEvent {
    /// A new tray state produced by the daemon worker.
    State(TrayState),
    /// The daemon-owned low-battery threshold changed.
    LowBatteryThreshold(u8),
    /// A context-menu activation.
    Menu(MenuEvent),
}

fn main() -> Result<()> {
    let filter = EnvFilter::from_default_env()
        .add_directive("scyrox_tray=info".parse()?)
        .add_directive("scyrox=info".parse()?);

    tracing_subscriber::fmt()
        .with_env_filter(filter)
        .with_writer(std::io::stderr)
        .init();

    info!("starting scyrox-tray");

    // A desktop with no usable font is broken; bail before the loop.
    let font = icon::load_font()?;

    let event_loop = EventLoopBuilder::<UserEvent>::with_user_event().build();

    // Forward menu events into the loop and wake it up.
    let menu_proxy = event_loop.create_proxy();
    MenuEvent::set_event_handler(Some(move |event| {
        let _ = menu_proxy.send_event(UserEvent::Menu(event));
    }));

    // Proxy for the daemon worker thread (spawned once the loop is running).
    let worker_proxy = event_loop.create_proxy();

    let mut tray_icon = None;
    let mut battery_item: Option<MenuItem> = None;
    let mut quit_item: Option<MenuItem> = None;
    let mut last_state: Option<TrayState> = None;
    let mut low_battery_threshold: Option<u8> = None;

    event_loop.run(move |event, _, control_flow| {
        *control_flow = ControlFlow::Wait;

        match event {
            // The tray must be created once the loop is actually running, per
            // tray-icon docs (creating it earlier breaks on some platforms).
            Event::NewEvents(StartCause::Init) => {
                let battery = MenuItem::new("Connecting…", false, None);
                let quit = MenuItem::new("Quit", true, None);

                let menu = Menu::new();
                if let Err(e) =
                    menu.append_items(&[&battery, &PredefinedMenuItem::separator(), &quit])
                {
                    error!("failed to build tray menu: {e}");
                }

                match TrayIconBuilder::new()
                    .with_menu(Box::new(menu))
                    .with_tooltip("Scyrox — connecting…")
                    .with_icon(icon::render(&TrayState::DaemonDown, &font, None))
                    .build()
                {
                    Ok(tray) => tray_icon = Some(tray),
                    Err(e) => {
                        error!("failed to create tray icon: {e}");
                        *control_flow = ControlFlow::Exit;
                        return;
                    }
                }

                battery_item = Some(battery);
                quit_item = Some(quit);

                // Start the worker only now, so proxy events cannot arrive
                // before the loop is running.
                if let Err(e) = daemon::spawn(worker_proxy.clone()) {
                    error!("failed to spawn daemon worker thread: {e}");
                    *control_flow = ControlFlow::Exit;
                }
            }

            Event::UserEvent(UserEvent::State(new_state)) => {
                if last_state != Some(new_state) {
                    last_state = Some(new_state);

                    if let Some(tray) = &tray_icon {
                        let _ = tray.set_icon(Some(icon::render(
                            &new_state,
                            &font,
                            low_battery_threshold,
                        )));
                        let _ = tray.set_tooltip(Some(state::tooltip(&new_state)));
                    }
                    if let Some(item) = &battery_item {
                        item.set_text(state::menu_line(&new_state));
                    }
                }
            }

            Event::UserEvent(UserEvent::LowBatteryThreshold(threshold)) => {
                if low_battery_threshold != Some(threshold) {
                    low_battery_threshold = Some(threshold);
                    if let (Some(tray), Some(state)) = (&tray_icon, last_state) {
                        let _ =
                            tray.set_icon(Some(icon::render(&state, &font, low_battery_threshold)));
                    }
                }
            }

            Event::UserEvent(UserEvent::Menu(menu_event)) => {
                if let Some(quit) = &quit_item
                    && menu_event.id == *quit.id()
                {
                    // Leave the daemon running; only drop the tray and exit.
                    tray_icon.take();
                    *control_flow = ControlFlow::Exit;
                }
            }

            _ => {}
        }
    })
}
