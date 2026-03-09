//! WatchEvents stream consumer.
//!
//! Connects to the daemon's `WatchEvents` gRPC stream and maps proto events
//! into application-level messages for the iced UI and notification system.

use scyrox_proto::event::Event;
use tracing::{debug, warn};

use crate::app::events::AppEvent;
use crate::notifications;

/// Process a single proto event, dispatching notifications and returning
/// an `AppEvent` for the iced application to handle.
pub async fn handle_event(event: Event) -> Option<AppEvent> {
    match event {
        Event::ConnectionChange(change) => {
            debug!(connected = change.connected, "connection change event");
            if change.connected {
                notifications::device_connected().await;
            } else {
                notifications::device_disconnected().await;
            }
            Some(AppEvent::ConnectionChanged {
                connected: change.connected,
            })
        }
        Event::BatteryUpdate(update) => {
            let percentage = update
                .status
                .as_ref()
                .map(|s| s.percentage as u8)
                .unwrap_or(0);
            debug!(percentage, "battery update event");
            Some(AppEvent::BatteryUpdated { percentage })
        }
        Event::LowBatteryAlert(alert) => {
            let percentage = alert.percentage as u8;
            warn!(percentage, "low battery alert");
            notifications::battery_low(percentage).await;
            Some(AppEvent::LowBattery { percentage })
        }
        Event::ProfileApplied(applied) => {
            debug!(profile = applied.profile_name, "profile applied event");
            notifications::profile_applied(&applied.profile_name).await;
            Some(AppEvent::ProfileApplied {
                name: applied.profile_name,
            })
        }
        Event::SettingsChanged(_) => {
            debug!("settings changed event");
            Some(AppEvent::SettingsChanged)
        }
    }
}
