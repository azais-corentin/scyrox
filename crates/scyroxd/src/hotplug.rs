//! HID device hotplug monitoring via periodic polling.
//!
//! Since hidapi does not provide event-driven hotplug notifications, this module
//! polls `HidApi::refresh_devices()` at a regular interval to detect device
//! connect/disconnect events.

use std::ffi::CString;
use std::time::Duration;

use hidapi::HidApi;
use scyrox::{ConnectionMode, PID_WIRED, PID_WIRELESS_4K, PID_WIRELESS_STD, VENDOR_ID};
use tokio::sync::broadcast;
use tracing::{debug, error, info, warn};

/// Polling interval for device enumeration.
const POLL_INTERVAL: Duration = Duration::from_secs(1);

/// Interface number to filter on (must match scyrox::protocol::INTERFACE_NUM).
const INTERFACE_NUM: i32 = 1;

/// Events emitted when device connection state changes.
#[derive(Debug, Clone)]
pub enum DeviceEvent {
    /// Device connected (fresh connection, no previous device).
    Connected { mode: ConnectionMode },
    /// Device disconnected (no fallback available).
    Disconnected,
    /// Connection mode changed (e.g., wireless to wired or vice versa).
    ModeChanged {
        from: ConnectionMode,
        to: ConnectionMode,
    },
}

/// Tracks which Scyrox devices are physically connected by their HID device path.
#[derive(Default, Debug)]
struct DeviceState {
    /// Device path of connected wired device (PID_WIRED).
    wired_path: Option<CString>,
    /// Device path of connected wireless dongle (PID_WIRELESS_4K or PID_WIRELESS_STD).
    wireless_path: Option<CString>,
    /// Currently active connection mode (which interface is claimed).
    active_mode: Option<ConnectionMode>,
}

impl DeviceState {
    /// Scan the current device list and emit connect/disconnect/mode-change events
    /// by comparing against previously known state.
    fn update(&mut self, api: &HidApi, tx: &broadcast::Sender<DeviceEvent>) {
        let mut current_wired: Option<CString> = None;
        let mut current_wireless: Option<CString> = None;

        for dev in api.device_list() {
            if dev.vendor_id() != VENDOR_ID || dev.interface_number() != INTERFACE_NUM {
                continue;
            }

            match dev.product_id() {
                PID_WIRED => {
                    current_wired = Some(dev.path().to_owned());
                }
                PID_WIRELESS_4K | PID_WIRELESS_STD => {
                    current_wireless = Some(dev.path().to_owned());
                }
                _ => {}
            }
        }

        // Detect wired connect/disconnect
        let was_wired = self.wired_path.is_some();
        let is_wired = current_wired.is_some();

        // Detect wireless connect/disconnect
        let was_wireless = self.wireless_path.is_some();
        let is_wireless = current_wireless.is_some();

        // Update stored paths
        self.wired_path = current_wired;
        self.wireless_path = current_wireless;

        // Emit events based on state transitions
        match (was_wired, is_wired, was_wireless, is_wireless) {
            // Wired appeared
            (false, true, _, _) => match self.active_mode {
                Some(ConnectionMode::Wireless) => {
                    info!("mode change: Wireless -> Wired");
                    self.active_mode = Some(ConnectionMode::Wired);
                    let _ = tx.send(DeviceEvent::ModeChanged {
                        from: ConnectionMode::Wireless,
                        to: ConnectionMode::Wired,
                    });
                }
                None => {
                    info!("device connected: Wired");
                    self.active_mode = Some(ConnectionMode::Wired);
                    let _ = tx.send(DeviceEvent::Connected {
                        mode: ConnectionMode::Wired,
                    });
                }
                Some(ConnectionMode::Wired) => {
                    warn!("wired device connected but already in wired mode");
                }
            },
            // Wired disappeared
            (true, false, _, _) => {
                if self.active_mode == Some(ConnectionMode::Wired) {
                    if is_wireless {
                        info!("mode change: Wired -> Wireless");
                        self.active_mode = Some(ConnectionMode::Wireless);
                        let _ = tx.send(DeviceEvent::ModeChanged {
                            from: ConnectionMode::Wired,
                            to: ConnectionMode::Wireless,
                        });
                    } else {
                        info!("device disconnected");
                        self.active_mode = None;
                        let _ = tx.send(DeviceEvent::Disconnected);
                    }
                }
            }
            // Wireless appeared (wired unchanged)
            (w, w2, false, true) if w == w2 => {
                if self.active_mode.is_none() {
                    info!("device connected: Wireless");
                    self.active_mode = Some(ConnectionMode::Wireless);
                    let _ = tx.send(DeviceEvent::Connected {
                        mode: ConnectionMode::Wireless,
                    });
                }
                // If in wired mode, just note the wireless dongle is available
            }
            // Wireless disappeared (wired unchanged)
            (w, w2, true, false)
                if w == w2 && self.active_mode == Some(ConnectionMode::Wireless) && !is_wired =>
            {
                info!("device disconnected");
                self.active_mode = None;
                let _ = tx.send(DeviceEvent::Disconnected);
            }
            // No change or simultaneous change (rare)
            _ => {}
        }
    }

    /// Initialize state from currently connected devices (no events emitted).
    fn init_from_current_devices(&mut self, api: &HidApi) {
        for dev in api.device_list() {
            if dev.vendor_id() != VENDOR_ID || dev.interface_number() != INTERFACE_NUM {
                continue;
            }

            match dev.product_id() {
                PID_WIRED => {
                    debug!("found existing wired device");
                    self.wired_path = Some(dev.path().to_owned());
                }
                PID_WIRELESS_4K | PID_WIRELESS_STD => {
                    debug!(pid = dev.product_id(), "found existing wireless dongle");
                    self.wireless_path = Some(dev.path().to_owned());
                }
                _ => {}
            }
        }

        // Determine initial mode (wired takes priority)
        if self.wired_path.is_some() {
            self.active_mode = Some(ConnectionMode::Wired);
            debug!("initial mode: Wired");
        } else if self.wireless_path.is_some() {
            self.active_mode = Some(ConnectionMode::Wireless);
            debug!("initial mode: Wireless");
        } else {
            debug!("no device connected at startup");
        }
    }
}

/// Monitors HID device connect/disconnect events via polling.
pub struct HotplugMonitor {
    /// Sender to keep channel alive.
    _event_tx: broadcast::Sender<DeviceEvent>,
}

impl HotplugMonitor {
    /// Start the hotplug monitor.
    ///
    /// Returns the monitor and a receiver for device events.
    pub fn start() -> anyhow::Result<(Self, broadcast::Receiver<DeviceEvent>)> {
        let (tx, rx) = broadcast::channel(16);
        let tx_clone = tx.clone();

        // Start the monitoring task
        tokio::task::spawn_blocking(move || {
            if let Err(e) = Self::monitor_loop(tx_clone) {
                error!("hotplug monitor error: {}", e);
            }
        });

        Ok((Self { _event_tx: tx }, rx))
    }

    /// The main monitoring loop.
    fn monitor_loop(tx: broadcast::Sender<DeviceEvent>) -> anyhow::Result<()> {
        let mut api = HidApi::new()?;

        // Initialize state from currently connected devices
        let mut state = DeviceState::default();
        state.init_from_current_devices(&api);

        info!(
            wired = state.wired_path.is_some(),
            wireless = state.wireless_path.is_some(),
            mode = ?state.active_mode,
            "hotplug monitor started"
        );

        loop {
            std::thread::sleep(POLL_INTERVAL);

            if let Err(e) = api.refresh_devices() {
                warn!("failed to refresh device list: {}", e);
                continue;
            }

            state.update(&api, &tx);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_device_state_default() {
        let state = DeviceState::default();
        assert!(state.wired_path.is_none());
        assert!(state.wireless_path.is_none());
        assert!(state.active_mode.is_none());
    }
}
