//! Main iced application.

use std::time::Duration;

use iced::futures::SinkExt;
use iced::widget::{center, column, text};
use iced::{Element, Subscription, Task as IcedTask, Theme};
use scyrox::Notification;
use scyrox_client::{Backend, DaemonClient, DirectBackend};
use scyrox_proto::event;
use tracing::{info, warn};

/// Whether the GUI is using the daemon or direct HID.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum BackendMode {
    Daemon,
    Direct,
}

/// Connection status for the daemon/device.
#[derive(Debug, Clone, PartialEq, Eq)]
enum ConnectionStatus {
    /// Waiting for initial state.
    Connecting,
    /// Device is connected (via daemon or direct).
    Connected,
    /// Daemon reachable but device is disconnected.
    Disconnected,
    /// Neither daemon nor device reachable.
    Unavailable,
}

/// Top-level application message type.
#[allow(dead_code)]
#[derive(Debug, Clone)]
pub enum Message {
    /// Initial state fetched on startup.
    InitialState {
        mode: BackendMode,
        connected: bool,
        battery: Option<u8>,
    },
    /// Failed to connect to both daemon and device on startup.
    ConnectionFailed,
    /// A stream event was received.
    EventReceived(events::AppEvent),
    /// Tray requested the window to be shown.
    ShowWindow,
    /// Tray requested application quit.
    Quit,
}

/// Events that map from daemon proto events to application-level events.
pub mod events {
    use super::BackendMode;

    #[allow(dead_code)]
    #[derive(Debug, Clone)]
    pub enum AppEvent {
        /// Device connected or disconnected.
        ConnectionChanged { connected: bool },
        /// Battery level updated.
        BatteryUpdated { percentage: u8 },
        /// Battery is low.
        LowBattery { percentage: u8 },
        /// A profile was applied.
        ProfileApplied { name: String },
        /// Settings were changed on the device.
        SettingsChanged,
        /// Backend mode changed (daemon <-> direct).
        ModeChanged { mode: BackendMode },
        /// Neither daemon nor device is reachable.
        NothingAvailable,
    }
}

/// Main application state.
pub struct App {
    status: ConnectionStatus,
    battery_percentage: Option<u8>,
    backend_mode: Option<BackendMode>,
}

impl App {
    fn new() -> (Self, IcedTask<Message>) {
        let task = IcedTask::perform(fetch_initial_state(), |result| match result {
            Ok((mode, connected, battery)) => Message::InitialState {
                mode,
                connected,
                battery,
            },
            Err(_) => Message::ConnectionFailed,
        });

        (
            Self {
                status: ConnectionStatus::Connecting,
                battery_percentage: None,
                backend_mode: None,
            },
            task,
        )
    }

    fn update(&mut self, message: Message) -> IcedTask<Message> {
        match message {
            Message::InitialState {
                mode,
                connected,
                battery,
            } => {
                self.backend_mode = Some(mode);
                self.status = if connected {
                    ConnectionStatus::Connected
                } else {
                    ConnectionStatus::Disconnected
                };
                self.battery_percentage = battery;
                info!(?self.status, ?self.battery_percentage, ?mode, "initial state loaded");
                IcedTask::none()
            }
            Message::ConnectionFailed => {
                warn!("failed to connect to daemon and device for initial state");
                self.status = ConnectionStatus::Unavailable;
                IcedTask::none()
            }
            Message::EventReceived(event) => {
                match event {
                    events::AppEvent::ConnectionChanged { connected } => {
                        self.status = if connected {
                            ConnectionStatus::Connected
                        } else {
                            self.battery_percentage = None;
                            ConnectionStatus::Disconnected
                        };
                    }
                    events::AppEvent::BatteryUpdated { percentage } => {
                        self.battery_percentage = Some(percentage);
                    }
                    events::AppEvent::LowBattery { .. } => {
                        // Notification handled by events module
                    }
                    events::AppEvent::ProfileApplied { .. } => {
                        // Could refresh config display
                    }
                    events::AppEvent::SettingsChanged => {
                        // Could refresh config display
                    }
                    events::AppEvent::ModeChanged { mode } => {
                        self.backend_mode = Some(mode);
                        info!(?mode, "backend mode changed");
                    }
                    events::AppEvent::NothingAvailable => {
                        self.status = ConnectionStatus::Unavailable;
                        self.battery_percentage = None;
                    }
                }
                IcedTask::none()
            }
            Message::ShowWindow => {
                // TODO: toggle window visibility
                IcedTask::none()
            }
            Message::Quit => iced::exit(),
        }
    }

    fn view(&self) -> Element<'_, Message> {
        let status_text = match (&self.status, self.backend_mode) {
            (ConnectionStatus::Connecting, _) => "Mouse: Connecting...".to_string(),
            (ConnectionStatus::Connected, Some(BackendMode::Direct)) => {
                "Mouse: Connected (Direct)".to_string()
            }
            (ConnectionStatus::Connected, _) => "Mouse: Connected".to_string(),
            (ConnectionStatus::Disconnected, _) => "Mouse: Disconnected".to_string(),
            (ConnectionStatus::Unavailable, _) => "Mouse: Unavailable".to_string(),
        };

        let battery = match self.battery_percentage {
            Some(pct) => format!("Battery: {pct}%"),
            None => "Battery: Unknown".to_string(),
        };

        center(
            column![text(status_text).size(24), text(battery).size(18),]
                .spacing(12)
                .align_x(iced::Alignment::Center),
        )
        .into()
    }

    fn theme(&self) -> Theme {
        Theme::Dark
    }

    fn subscription(&self) -> Subscription<Message> {
        Subscription::run(event_stream).map(Message::EventReceived)
    }
}

/// Fetch initial state: try daemon first, then direct HID.
async fn fetch_initial_state() -> anyhow::Result<(BackendMode, bool, Option<u8>)> {
    // Try daemon first
    if let Ok(client) = DaemonClient::connect().await {
        let connected = client.is_connected().await;
        let battery = if connected {
            client.get_battery().await.ok().map(|b| b.percentage)
        } else {
            None
        };
        return Ok((BackendMode::Daemon, connected, battery));
    }

    // Fall back to direct HID
    let direct = DirectBackend::new().await?;
    let battery = direct.get_battery().await.ok().map(|b| b.percentage);
    Ok((BackendMode::Direct, true, battery))
}

/// Unified event stream: daemon-preferred with direct HID fallback.
fn event_stream() -> impl iced::futures::Stream<Item = events::AppEvent> {
    iced::stream::channel(32, |mut sender| async move {
        loop {
            // Phase 1: Try daemon
            match connect_and_stream_daemon(&mut sender).await {
                Ok(()) => {
                    info!("daemon event stream ended cleanly");
                    continue;
                }
                Err(e) => {
                    warn!("daemon unavailable: {e}");
                }
            }

            // Phase 2: Fall back to direct HID with daemon reconnect probing
            match connect_and_stream_direct(&mut sender).await {
                Ok(()) => {
                    info!("direct mode ended, retrying from top");
                    continue;
                }
                Err(e) => {
                    warn!("direct mode unavailable: {e}");
                }
            }

            // Phase 3: Nothing available, wait and retry
            sender.send(events::AppEvent::NothingAvailable).await.ok();
            tokio::time::sleep(Duration::from_secs(5)).await;
        }
    })
}

/// Connect to the daemon and stream gRPC events until disconnect or error.
async fn connect_and_stream_daemon(
    sender: &mut iced::futures::channel::mpsc::Sender<events::AppEvent>,
) -> anyhow::Result<()> {
    let client = DaemonClient::connect().await?;

    sender
        .send(events::AppEvent::ModeChanged {
            mode: BackendMode::Daemon,
        })
        .await
        .ok();

    // Send initial connection state
    let connected = client.is_connected().await;
    sender
        .send(events::AppEvent::ConnectionChanged { connected })
        .await
        .ok();

    if connected && let Ok(battery) = client.get_battery().await {
        sender
            .send(events::AppEvent::BatteryUpdated {
                percentage: battery.percentage,
            })
            .await
            .ok();
    }

    let mut stream = client.watch_events().await?;

    while let Some(event_msg) = stream.message().await? {
        if let Some(event) = event_msg.event {
            let is_reconnect = matches!(&event, event::Event::ConnectionChange(c) if c.connected);

            if let Some(app_event) = crate::events::handle_event(event).await
                && sender.send(app_event).await.is_err()
            {
                return Ok(());
            }

            // Re-fetch battery after device reconnect — the daemon's own
            // attempt may have failed if the device wasn't ready yet.
            if is_reconnect && let Ok(battery) = client.get_battery().await {
                sender
                    .send(events::AppEvent::BatteryUpdated {
                        percentage: battery.percentage,
                    })
                    .await
                    .ok();
            }
        }
    }

    Ok(())
}

/// Open a direct HID connection and stream notifications, polling battery
/// and probing for daemon availability in the background.
async fn connect_and_stream_direct(
    sender: &mut iced::futures::channel::mpsc::Sender<events::AppEvent>,
) -> anyhow::Result<()> {
    let direct = DirectBackend::new().await?;

    sender
        .send(events::AppEvent::ModeChanged {
            mode: BackendMode::Direct,
        })
        .await
        .ok();

    sender
        .send(events::AppEvent::ConnectionChanged { connected: true })
        .await
        .ok();

    // Send initial battery
    if let Ok(battery) = direct.get_battery().await {
        sender
            .send(events::AppEvent::BatteryUpdated {
                percentage: battery.percentage,
            })
            .await
            .ok();
    }

    let mut notifications = direct.subscribe_notifications().await;
    let mut battery_interval = tokio::time::interval(Duration::from_secs(30));
    let mut daemon_check_interval = tokio::time::interval(Duration::from_secs(10));

    // Consume the first immediate tick from both intervals
    battery_interval.tick().await;
    daemon_check_interval.tick().await;

    loop {
        tokio::select! {
            notification = notifications.recv() => {
                match notification {
                    Ok(Notification::Disconnected) => {
                        sender
                            .send(events::AppEvent::ConnectionChanged { connected: false })
                            .await
                            .ok();
                        return Ok(());
                    }
                    Ok(Notification::StatusChanged(flags)) => {
                        sender.send(events::AppEvent::SettingsChanged).await.ok();
                        if flags.battery_changed()
                            && let Ok(battery) = direct.get_battery().await
                        {
                            sender
                                .send(events::AppEvent::BatteryUpdated {
                                    percentage: battery.percentage,
                                })
                                .await
                                .ok();
                        }
                    }
                    Err(tokio::sync::broadcast::error::RecvError::Lagged(n)) => {
                        warn!(skipped = n, "notification receiver lagged");
                        // Non-fatal, continue
                    }
                    Err(tokio::sync::broadcast::error::RecvError::Closed) => {
                        return Ok(());
                    }
                }
            }
            _ = battery_interval.tick() => {
                if let Ok(battery) = direct.get_battery().await {
                    sender
                        .send(events::AppEvent::BatteryUpdated {
                            percentage: battery.percentage,
                        })
                        .await
                        .ok();
                }
            }
            _ = daemon_check_interval.tick() => {
                if DaemonClient::connect().await.is_ok() {
                    info!("daemon became available, switching from direct mode");
                    return Ok(());
                }
            }
        }
    }
}

/// Run the iced application.
pub fn run() -> anyhow::Result<()> {
    iced::application(App::new, App::update, App::view)
        .theme(App::theme)
        .subscription(App::subscription)
        .window_size((480.0, 360.0))
        .centered()
        .run()?;

    Ok(())
}
