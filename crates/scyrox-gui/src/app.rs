//! Main iced application.

use std::time::Duration;

use iced::futures::{SinkExt, Stream};
use iced::widget::{center, column, text};
use iced::{Element, Subscription, Task as IcedTask, Theme, window};
use ksni::TrayMethods;
use scyrox::Notification;
use scyrox_client::{Backend, DaemonClient, DirectBackend};
use scyrox_proto::event;
use tracing::{info, warn};

use crate::tray::{ScyroxTray, TrayCommand};

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

/// A handle to the running ksni tray, stored in app state so connection
/// changes can be pushed to the tray menu.
#[derive(Clone)]
pub struct TrayHandle(pub ksni::Handle<ScyroxTray>);

impl std::fmt::Debug for TrayHandle {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("TrayHandle(..)")
    }
}

/// Top-level application message type.
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
    /// Tray requested the window to be shown (or focused).
    ShowWindow,
    /// Tray requested application quit.
    Quit,
    /// A window finished opening.
    WindowOpened(window::Id),
    /// A window was closed by the user.
    WindowClosed(window::Id),
    /// The tray spawned successfully; carries its handle.
    TrayReady(TrayHandle),
    /// The tray could not be spawned (no StatusNotifier host).
    TrayUnavailable,
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

/// Logical settings for the main window.
fn window_settings() -> window::Settings {
    window::Settings {
        size: iced::Size::new(480.0, 360.0),
        ..Default::default()
    }
}

/// Main application state.
pub struct App {
    status: ConnectionStatus,
    battery_percentage: Option<u8>,
    backend_mode: Option<BackendMode>,
    /// The currently open window, if any.
    window: Option<window::Id>,
    /// Handle to the running tray, if it spawned.
    tray: Option<TrayHandle>,
    /// Whether a tray is (or may still become) available. When false and the
    /// window closes, the app has no remaining UI and must exit.
    tray_available: bool,
}

impl App {
    fn boot() -> (Self, IcedTask<Message>) {
        let fetch = IcedTask::perform(fetch_initial_state(), |result| match result {
            Ok((mode, connected, battery)) => Message::InitialState {
                mode,
                connected,
                battery,
            },
            Err(_) => Message::ConnectionFailed,
        });

        let (_id, open) = window::open(window_settings());
        let open = open.map(Message::WindowOpened);

        (
            Self {
                status: ConnectionStatus::Connecting,
                battery_percentage: None,
                backend_mode: None,
                window: None,
                tray: None,
                tray_available: true,
            },
            IcedTask::batch([fetch, open]),
        )
    }

    /// Push the current connection state to the tray menu, if the tray exists.
    fn push_tray_connected(&self, connected: bool) -> IcedTask<Message> {
        if let Some(TrayHandle(handle)) = &self.tray {
            let handle = handle.clone();
            IcedTask::future(async move {
                handle.update(|t| t.set_connected(connected)).await;
            })
            .discard()
        } else {
            IcedTask::none()
        }
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
                self.push_tray_connected(connected)
            }
            Message::ConnectionFailed => {
                warn!("failed to connect to daemon and device for initial state");
                self.status = ConnectionStatus::Unavailable;
                self.push_tray_connected(false)
            }
            Message::EventReceived(event) => match event {
                events::AppEvent::ConnectionChanged { connected } => {
                    self.status = if connected {
                        ConnectionStatus::Connected
                    } else {
                        self.battery_percentage = None;
                        ConnectionStatus::Disconnected
                    };
                    self.push_tray_connected(connected)
                }
                events::AppEvent::BatteryUpdated { percentage } => {
                    self.battery_percentage = Some(percentage);
                    IcedTask::none()
                }
                events::AppEvent::LowBattery { .. } => {
                    // Notification handled by events module
                    IcedTask::none()
                }
                events::AppEvent::ProfileApplied { .. } => {
                    // Could refresh config display
                    IcedTask::none()
                }
                events::AppEvent::SettingsChanged => {
                    // Could refresh config display
                    IcedTask::none()
                }
                events::AppEvent::ModeChanged { mode } => {
                    self.backend_mode = Some(mode);
                    info!(?mode, "backend mode changed");
                    self.push_tray_connected(self.status == ConnectionStatus::Connected)
                }
                events::AppEvent::NothingAvailable => {
                    self.status = ConnectionStatus::Unavailable;
                    self.battery_percentage = None;
                    self.push_tray_connected(false)
                }
            },
            Message::ShowWindow => {
                if let Some(id) = self.window {
                    window::gain_focus(id)
                } else {
                    let (_id, open) = window::open(window_settings());
                    open.map(Message::WindowOpened)
                }
            }
            Message::WindowOpened(id) => {
                self.window = Some(id);
                IcedTask::none()
            }
            Message::WindowClosed(id) => {
                if self.window == Some(id) {
                    self.window = None;
                }
                if self.tray_available {
                    IcedTask::none()
                } else {
                    // No tray and no window: nothing left to interact with.
                    iced::exit()
                }
            }
            Message::TrayReady(handle) => {
                info!("tray ready");
                self.tray = Some(handle);
                // Reflect the current connection state immediately.
                self.push_tray_connected(self.status == ConnectionStatus::Connected)
            }
            Message::TrayUnavailable => {
                self.tray_available = false;
                IcedTask::none()
            }
            Message::Quit => iced::exit(),
        }
    }

    fn view(&self, _window: window::Id) -> Element<'_, Message> {
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

    fn theme(&self, _window: window::Id) -> Theme {
        Theme::Dark
    }

    fn subscription(&self) -> Subscription<Message> {
        Subscription::batch([
            Subscription::run(event_stream).map(Message::EventReceived),
            Subscription::run(tray_stream),
            window::close_events().map(Message::WindowClosed),
        ])
    }
}

/// Spawn the ksni tray and bridge its menu activations into app messages.
fn tray_stream() -> impl Stream<Item = Message> {
    iced::stream::channel(
        16,
        |mut out: iced::futures::channel::mpsc::Sender<Message>| async move {
            let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel();
            match ScyroxTray::new(tx).spawn().await {
                Ok(handle) => {
                    out.send(Message::TrayReady(TrayHandle(handle))).await.ok();
                }
                Err(e) => {
                    warn!("tray unavailable: {e}");
                    out.send(Message::TrayUnavailable).await.ok();
                    return;
                }
            }

            while let Some(cmd) = rx.recv().await {
                let msg = match cmd {
                    TrayCommand::ShowWindow => Message::ShowWindow,
                    TrayCommand::Quit => Message::Quit,
                };
                if out.send(msg).await.is_err() {
                    return;
                }
            }
        },
    )
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

/// Run the iced application as a daemon (persists in the tray with no window).
pub fn run() -> anyhow::Result<()> {
    iced::daemon(App::boot, App::update, App::view)
        .title("Scyrox")
        .theme(App::theme)
        .subscription(App::subscription)
        .run()?;

    Ok(())
}
