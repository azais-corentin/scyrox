// TODO(phase-3): Remove once event wiring connects all scaffold modules.
#![allow(dead_code)]

//! Scyrox GUI application with systray and desktop notifications.
//!
//! This binary provides:
//! - A persistent system tray icon with context menu
//! - Desktop notifications for battery, connection, and profile events
//! - An iced-based GUI window that opens/closes on demand from the tray

mod app;
mod events;
mod notifications;
mod tray;

use anyhow::Result;
use tracing::info;
use tracing_subscriber::EnvFilter;

fn main() -> Result<()> {
    // Initialize tracing
    let filter = EnvFilter::from_default_env()
        .add_directive("scyrox_gui=info".parse()?)
        .add_directive("scyrox=info".parse()?)
        .add_directive("iced=warn".parse()?);

    tracing_subscriber::fmt()
        .with_env_filter(filter)
        .with_writer(std::io::stderr)
        .init();

    info!("starting scyrox-gui");

    app::run()
}
