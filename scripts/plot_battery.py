#!/usr/bin/env -S uv run --script
# /// script
# requires-python = ">=3.11"
# dependencies = ["plotly>=5"]
# ///
"""Plot scyroxd's battery capture log as an interactive plotly HTML.

Reads the JSONL battery log written by scyroxd (see
crates/scyroxd/src/battery_log.rs), and renders voltage, device/estimated
percentage, charging shading, and sleep shading (device_offline refresh
errors). Disconnected periods and daemon downtime are cut from the time axis
via plotly rangebreaks and marked with "cut <duration>" seams.
"""

import argparse
import json
import statistics
import sys
import webbrowser
from datetime import datetime, timedelta
from pathlib import Path

import plotly.graph_objects as go

KNOWN_EVENTS = {
    "sample",
    "refresh_error",
    "device_connected",
    "device_disconnected",
    "connection_mode_changed",
}

SLEEP_FILL = "rgba(120,120,120,0.30)"
CHARGING_FILL = "rgba(46,160,67,0.15)"
# Legend swatches: same hues at ~0.5 opacity so they are visible.
SLEEP_SWATCH = "rgba(120,120,120,0.5)"
CHARGING_SWATCH = "rgba(46,160,67,0.5)"


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(
        description="Plot a scyroxd battery capture log (JSONL) to interactive HTML."
    )
    parser.add_argument(
        "log",
        nargs="?",
        default="~/.config/scyrox/captures/battery.jsonl",
        help="path to battery.jsonl (default: %(default)s)",
    )
    parser.add_argument(
        "-o",
        "--output",
        default="battery.html",
        help="output HTML path (default: %(default)s)",
    )
    parser.add_argument(
        "--no-open",
        action="store_true",
        help="do not open the result in a browser",
    )
    return parser.parse_args()


def load_events(path: Path) -> list[dict]:
    if not path.exists():
        sys.exit(f"error: {path} not found")

    events: list[dict] = []
    skipped = 0
    unknown_warned: set[str] = set()

    with path.open() as f:
        for line in f:
            line = line.strip()
            if not line:
                continue
            try:
                record = json.loads(line)
            except json.JSONDecodeError:
                skipped += 1
                continue
            event = record.get("event")
            if event not in KNOWN_EVENTS:
                if event not in unknown_warned:
                    unknown_warned.add(event)
                    print(f"warning: unknown event type {event!r}", file=sys.stderr)
                continue
            events.append(record)

    if skipped:
        print(f"skipped {skipped} malformed line(s)", file=sys.stderr)
    if not events:
        sys.exit("error: no events found in log")

    events.sort(key=lambda e: e["timestamp_unix_ms"])
    return events


def ts(event: dict) -> datetime:
    return datetime.fromtimestamp(event["timestamp_unix_ms"] / 1000)


def build_figure(events: list[dict], log_path: Path) -> go.Figure:
    samples = [e for e in events if e["event"] == "sample"]

    # Gap threshold: break lines across daemon downtime / sleep.
    dts = [
        (b["timestamp_unix_ms"] - a["timestamp_unix_ms"]) / 1000
        for a, b in zip(samples, samples[1:])
    ]
    median_dt = statistics.median(dts) if dts else 5.0
    gap_threshold = max(30.0, 5 * median_dt)

    x: list = []
    device_pct: list = []
    estimated_pct: list = []
    voltage: list = []
    prev = None
    for s in samples:
        if prev is not None:
            dt = (s["timestamp_unix_ms"] - prev["timestamp_unix_ms"]) / 1000
            if dt > gap_threshold or s["session_started_unix_ms"] != prev["session_started_unix_ms"]:
                x.append(None)
                device_pct.append(None)
                estimated_pct.append(None)
                voltage.append(None)
        x.append(ts(s))
        device_pct.append(s["device_percentage"])
        estimated_pct.append(s["estimated_percentage"])
        voltage.append(s["voltage_mv"])
        prev = s

    fig = go.Figure()
    fig.add_trace(
        go.Scatter(x=x, y=device_pct, mode="lines", name="Device %", line=dict(color="#1f77b4"))
    )
    fig.add_trace(
        go.Scatter(
            x=x, y=estimated_pct, mode="lines", name="Estimated %", line=dict(color="#ff7f0e")
        )
    )
    fig.add_trace(
        go.Scatter(
            x=x,
            y=voltage,
            mode="lines",
            name="Voltage (mV)",
            yaxis="y2",
            line=dict(color="#9467bd"),
        )
    )

    # Non-offline refresh errors as markers.
    other_errors = [
        e
        for e in events
        if e["event"] == "refresh_error" and e.get("error_kind") != "device_offline"
    ]
    if other_errors:
        fig.add_trace(
            go.Scatter(
                x=[ts(e) for e in other_errors],
                y=[0] * len(other_errors),
                mode="markers",
                name="Refresh error",
                marker=dict(symbol="x", size=8, color="red"),
                text=[
                    f"{e.get('error_kind')}: {e.get('error_message')}" for e in other_errors
                ],
                hoverinfo="x+text",
            )
        )

    add_sleep_spans(fig, events)
    add_charging_spans(fig, samples)
    cut_spans = compute_cut_spans(events)
    add_cut_seams(fig, cut_spans)
    add_boundaries(fig, events, cut_spans)

    first_ts = ts(events[0])
    last_ts = ts(events[-1])
    rangebreaks = []
    for x0, x1 in cut_spans:
        span_ms = (x1 - x0).total_seconds() * 1000
        eps = timedelta(milliseconds=min(1000, span_ms / 4))
        rangebreaks.append(dict(bounds=[x0 + eps, x1 - eps]))
    fig.update_layout(
        title=f"scyroxd battery log — {log_path} ({first_ts:%Y-%m-%d %H:%M} → {last_ts:%Y-%m-%d %H:%M})",
        hovermode="x unified",
        xaxis=dict(title="Time", rangebreaks=rangebreaks),
        yaxis=dict(title="Battery (%)", range=[0, 105]),
        yaxis2=dict(title="Voltage (mV)", overlaying="y", side="right"),
    )
    return fig


def add_legend_swatch(fig: go.Figure, name: str, color: str) -> None:
    fig.add_trace(
        go.Scatter(
            x=[None],
            y=[None],
            mode="markers",
            marker=dict(size=12, symbol="square", color=color),
            name=name,
        )
    )


def add_sleep_spans(fig: go.Figure, events: list[dict]) -> None:
    """Contiguous runs of device_offline refresh errors, broken by any other
    event or a session change."""
    spans = []
    run_start = None
    run_last = None
    run_session = None
    for e in events:
        is_offline = e["event"] == "refresh_error" and e.get("error_kind") == "device_offline"
        if run_start is not None:
            if is_offline and e["session_started_unix_ms"] == run_session:
                run_last = e
                continue
            # Run ends here. If the breaking event is in the same session, it
            # bounds the span; otherwise the run's last timestamp does.
            if e["session_started_unix_ms"] == run_session:
                spans.append((ts(run_start), ts(e)))
            else:
                spans.append((ts(run_start), ts(run_last)))
            run_start = None
        if is_offline:
            run_start = e
            run_last = e
            run_session = e["session_started_unix_ms"]
    if run_start is not None:
        spans.append((ts(run_start), ts(run_last)))

    for x0, x1 in spans:
        fig.add_vrect(x0=x0, x1=x1, fillcolor=SLEEP_FILL, opacity=1, line_width=0, layer="below")
    if spans:
        add_legend_swatch(fig, "Sleep", SLEEP_SWATCH)


def add_charging_spans(fig: go.Figure, samples: list[dict]) -> None:
    """Contiguous runs of charging samples within a session."""
    spans = []
    run_start = None
    run_last = None
    run_session = None
    for s in samples:
        charging = bool(s.get("charging"))
        if run_start is not None:
            if charging and s["session_started_unix_ms"] == run_session:
                run_last = s
                continue
            if s["session_started_unix_ms"] == run_session:
                spans.append((ts(run_start), ts(s)))
            else:
                spans.append((ts(run_start), ts(run_last)))
            run_start = None
        if charging:
            run_start = s
            run_last = s
            run_session = s["session_started_unix_ms"]
    if run_start is not None:
        spans.append((ts(run_start), ts(run_last)))

    for x0, x1 in spans:
        fig.add_vrect(
            x0=x0, x1=x1, fillcolor=CHARGING_FILL, opacity=1, line_width=0, layer="below"
        )
    if spans:
        add_legend_swatch(fig, "Charging", CHARGING_SWATCH)


def format_duration(seconds: float) -> str:
    """Two most significant units, e.g. '1d 4h', '2h 13m', '5m 12s', '45s'."""
    seconds = int(round(seconds))
    days, rem = divmod(seconds, 86400)
    hours, rem = divmod(rem, 3600)
    minutes, secs = divmod(rem, 60)
    parts = [(days, "d"), (hours, "h"), (minutes, "m"), (secs, "s")]
    # Drop leading zero units, then keep at most the first two remaining.
    while len(parts) > 1 and parts[0][0] == 0:
        parts.pop(0)
    parts = parts[:2]
    return " ".join(f"{v}{u}" for v, u in parts if not (v == 0 and len(parts) > 1)) or "0s"


def compute_cut_spans(events: list[dict]) -> list:
    """Time windows removed from the x-axis: disconnected spans and daemon
    downtime, merged into non-overlapping intervals."""
    spans = []
    # Disconnected spans: each device_disconnected to the next device_connected
    # in the same session, else to the session's last event.
    for i, e in enumerate(events):
        if e["event"] != "device_disconnected":
            continue
        session = e["session_started_unix_ms"]
        end = None
        for later in events[i + 1 :]:
            if later["session_started_unix_ms"] != session:
                break
            end = later
            if later["event"] == "device_connected":
                break
        if end is not None:
            spans.append((ts(e), ts(end)))
    # Daemon downtime: gaps between consecutive events in different sessions.
    for a, b in zip(events, events[1:]):
        if a["session_started_unix_ms"] != b["session_started_unix_ms"]:
            spans.append((ts(a), ts(b)))
    # Merge overlapping/touching spans; drop empty ones.
    spans.sort(key=lambda s: s[0])
    merged: list = []
    for x0, x1 in spans:
        if x1 <= x0:
            continue
        if merged and x0 <= merged[-1][1]:
            merged[-1] = (merged[-1][0], max(merged[-1][1], x1))
        else:
            merged.append((x0, x1))
    return merged


def add_cut_seams(fig: go.Figure, spans: list) -> None:
    """Mark each removed window with a dashed vline at the resume point,
    labeled with the skipped duration."""
    for x0, x1 in spans:
        fig.add_vline(
            x=x1,
            line=dict(color="#888888", dash="dash"),
            annotation_text=f"cut {format_duration((x1 - x0).total_seconds())}",
            annotation_position="bottom right",
        )


def add_boundaries(fig: go.Figure, events: list[dict], cut_spans: list) -> None:
    # Session starts: first event of each session_started_unix_ms value.
    seen_sessions: set[int] = set()
    for e in events:
        session = e["session_started_unix_ms"]
        if session not in seen_sessions:
            seen_sessions.add(session)
            fig.add_vline(
                x=ts(e),
                line=dict(color="#333333", dash="dash"),
                annotation_text="daemon start",
                annotation_position="top left",
            )

    for e in events:
        if e["event"] in ("device_connected", "device_disconnected") and any(
            x0 <= ts(e) <= x1 for x0, x1 in cut_spans
        ):
            continue
        if e["event"] == "device_connected":
            fig.add_vline(
                x=ts(e),
                line=dict(color="green", dash="dot"),
                annotation_text=f"connect ({e.get('source')})",
                annotation_position="top right",
            )
        elif e["event"] == "device_disconnected":
            fig.add_vline(
                x=ts(e),
                line=dict(color="red", dash="dot"),
                annotation_text="disconnect",
                annotation_position="top right",
            )


def main() -> None:
    args = parse_args()
    log_path = Path(args.log).expanduser()
    output = Path(args.output)

    events = load_events(log_path)
    fig = build_figure(events, log_path)
    fig.write_html(output)
    print(output)

    if not args.no_open:
        webbrowser.open(output.resolve().as_uri())


if __name__ == "__main__":
    main()
