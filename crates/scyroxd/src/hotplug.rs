//! USB hotplug monitoring for device connect/disconnect events.

use nusb::DeviceId;
use nusb::hotplug::HotplugEvent;
use scyrox::{ConnectionMode, PID_WIRED, PID_WIRELESS_4K, PID_WIRELESS_STD, VENDOR_ID};
use tokio::sync::broadcast;
use tokio_stream::StreamExt;
use tracing::{debug, error, info, warn};

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

/// Tracks which Scyrox devices are physically connected.
#[derive(Default, Debug)]
struct DeviceState {
    /// Device ID of connected wired device (PID_WIRED).
    wired_id: Option<DeviceId>,
    /// Device ID of connected wireless dongle (PID_WIRELESS_4K or PID_WIRELESS_STD).
    wireless_id: Option<DeviceId>,
    /// Currently active connection mode (which interface is claimed).
    active_mode: Option<ConnectionMode>,
}

impl DeviceState {
    /// Handle a device connection event.
    fn handle_connected(&mut self, info: &nusb::DeviceInfo, tx: &broadcast::Sender<DeviceEvent>) {
        if info.vendor_id() != VENDOR_ID {
            return;
        }

        let pid = info.product_id();
        let id = info.id();

        match pid {
            PID_WIRED => {
                debug!(id = ?id, "wired device connected");
                self.wired_id = Some(id);

                match self.active_mode {
                    Some(ConnectionMode::Wireless) => {
                        // Wireless -> Wired transition
                        info!("mode change: Wireless -> Wired");
                        self.active_mode = Some(ConnectionMode::Wired);
                        let _ = tx.send(DeviceEvent::ModeChanged {
                            from: ConnectionMode::Wireless,
                            to: ConnectionMode::Wired,
                        });
                    }
                    None => {
                        // Fresh connection
                        info!("device connected: Wired");
                        self.active_mode = Some(ConnectionMode::Wired);
                        let _ = tx.send(DeviceEvent::Connected {
                            mode: ConnectionMode::Wired,
                        });
                    }
                    Some(ConnectionMode::Wired) => {
                        // Already in wired mode, shouldn't happen
                        warn!("wired device connected but already in wired mode");
                    }
                }
            }
            PID_WIRELESS_4K | PID_WIRELESS_STD => {
                debug!(id = ?id, pid = pid, "wireless dongle connected");
                self.wireless_id = Some(id);

                if self.active_mode.is_none() {
                    // Fresh wireless connection
                    info!("device connected: Wireless");
                    self.active_mode = Some(ConnectionMode::Wireless);
                    let _ = tx.send(DeviceEvent::Connected {
                        mode: ConnectionMode::Wireless,
                    });
                }
                // If in wired mode, just store the ID for potential future fallback
                // Don't switch modes - wired takes priority
            }
            _ => {
                debug!(pid = pid, "ignoring unknown Scyrox product");
            }
        }
    }

    /// Handle a device disconnection event.
    fn handle_disconnected(&mut self, id: DeviceId, tx: &broadcast::Sender<DeviceEvent>) {
        if self.wired_id == Some(id) {
            debug!(id = ?id, "wired device disconnected");
            self.wired_id = None;

            if self.active_mode == Some(ConnectionMode::Wired) {
                if self.wireless_id.is_some() {
                    // Wired -> Wireless transition (wireless dongle was already connected)
                    info!("mode change: Wired -> Wireless");
                    self.active_mode = Some(ConnectionMode::Wireless);
                    let _ = tx.send(DeviceEvent::ModeChanged {
                        from: ConnectionMode::Wired,
                        to: ConnectionMode::Wireless,
                    });
                } else {
                    // No wireless fallback, device is truly disconnected
                    info!("device disconnected");
                    self.active_mode = None;
                    let _ = tx.send(DeviceEvent::Disconnected);
                }
            }
        } else if self.wireless_id == Some(id) {
            debug!(id = ?id, "wireless dongle disconnected");
            self.wireless_id = None;

            // Only emit disconnect if we were in wireless mode AND wired isn't connected
            if self.active_mode == Some(ConnectionMode::Wireless) && self.wired_id.is_none() {
                info!("device disconnected");
                self.active_mode = None;
                let _ = tx.send(DeviceEvent::Disconnected);
            }
            // If in wired mode, silently clear wireless ID (dongle unplugged while wired)
        }
    }

    /// Initialize state from currently connected devices.
    async fn init_from_current_devices(&mut self) {
        let devices = match nusb::list_devices().await {
            Ok(iter) => iter,
            Err(e) => {
                warn!("failed to list devices during init: {}", e);
                return;
            }
        };

        for info in devices {
            if info.vendor_id() != VENDOR_ID {
                continue;
            }

            let pid = info.product_id();
            let id = info.id();

            match pid {
                PID_WIRED => {
                    debug!(id = ?id, "found existing wired device");
                    self.wired_id = Some(id);
                }
                PID_WIRELESS_4K | PID_WIRELESS_STD => {
                    debug!(id = ?id, pid = pid, "found existing wireless dongle");
                    self.wireless_id = Some(id);
                }
                _ => {}
            }
        }

        // Determine initial mode (wired takes priority)
        if self.wired_id.is_some() {
            self.active_mode = Some(ConnectionMode::Wired);
            debug!("initial mode: Wired");
        } else if self.wireless_id.is_some() {
            self.active_mode = Some(ConnectionMode::Wireless);
            debug!("initial mode: Wireless");
        } else {
            debug!("no device connected at startup");
        }
    }
}

/// Monitors USB hotplug events for Scyrox devices.
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
        tokio::spawn(async move {
            if let Err(e) = Self::monitor_loop(tx_clone).await {
                error!("hotplug monitor error: {}", e);
            }
        });

        Ok((Self { _event_tx: tx }, rx))
    }

    /// The main monitoring loop.
    async fn monitor_loop(tx: broadcast::Sender<DeviceEvent>) -> anyhow::Result<()> {
        let watch = nusb::watch_devices()?;

        // Initialize state from currently connected devices
        let mut state = DeviceState::default();
        state.init_from_current_devices().await;

        info!(
            wired = state.wired_id.is_some(),
            wireless = state.wireless_id.is_some(),
            mode = ?state.active_mode,
            "hotplug monitor started"
        );

        // Pin the stream and process events
        tokio::pin!(watch);

        while let Some(event) = watch.next().await {
            match event {
                HotplugEvent::Connected(info) => {
                    state.handle_connected(&info, &tx);
                }
                HotplugEvent::Disconnected(id) => {
                    state.handle_disconnected(id, &tx);
                }
            }
        }

        warn!("hotplug watch stream ended unexpectedly");
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_device_state_default() {
        let state = DeviceState::default();
        assert!(state.wired_id.is_none());
        assert!(state.wireless_id.is_none());
        assert!(state.active_mode.is_none());
    }
}
