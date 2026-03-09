//! Desktop notifications via notify-rust (freedesktop).
//!
//! Sends system notifications for important mouse events:
//! - Battery low warnings
//! - Device connected / disconnected
//! - Profile changes

use notify_rust::Notification;
use tracing::{debug, warn};

/// Notify that the mouse battery is low.
pub async fn battery_low(percentage: u8) {
    debug!(percentage, "sending low battery notification");
    if let Err(e) = Notification::new()
        .appname("Scyrox")
        .summary("Low Battery")
        .body(&format!(
            "Mouse battery is at {percentage}%. Please charge soon."
        ))
        .icon("battery-caution")
        .urgency(notify_rust::Urgency::Critical)
        .show_async()
        .await
    {
        warn!("failed to show battery notification: {e}");
    }
}

/// Notify that the mouse was connected.
pub async fn device_connected() {
    debug!("sending device connected notification");
    if let Err(e) = Notification::new()
        .appname("Scyrox")
        .summary("Mouse Connected")
        .body("Scyrox mouse is now connected.")
        .icon("input-mouse")
        .show_async()
        .await
    {
        warn!("failed to show connection notification: {e}");
    }
}

/// Notify that the mouse was disconnected.
pub async fn device_disconnected() {
    debug!("sending device disconnected notification");
    if let Err(e) = Notification::new()
        .appname("Scyrox")
        .summary("Mouse Disconnected")
        .body("Scyrox mouse has been disconnected.")
        .icon("input-mouse-symbolic")
        .show_async()
        .await
    {
        warn!("failed to show disconnection notification: {e}");
    }
}

/// Notify that a profile was applied.
pub async fn profile_applied(name: &str) {
    debug!(name, "sending profile applied notification");
    if let Err(e) = Notification::new()
        .appname("Scyrox")
        .summary("Profile Applied")
        .body(&format!("Switched to profile: {name}"))
        .icon("input-mouse")
        .show_async()
        .await
    {
        warn!("failed to show profile notification: {e}");
    }
}
