//! Desktop notifications via notify-rust (freedesktop).

use notify_rust::Notification;
use tracing::{debug, warn};

/// Notify that the mouse battery is low.
pub async fn battery_low(percentage: u8) {
    debug!(percentage, "sending low battery notification");

    let mut notification = Notification::new();
    notification
        .appname("Scyrox")
        .summary("Low Battery")
        .body(&format!(
            "Mouse battery is at {percentage}%. Please charge soon."
        ))
        .icon("battery-caution");

    // `Urgency` is only available on Linux/BSD in notify-rust.
    #[cfg(all(unix, not(target_os = "macos")))]
    notification.urgency(notify_rust::Urgency::Critical);

    if let Err(e) = notification.show_async().await {
        warn!("failed to show battery notification: {e}");
    }
}
