//! Main iced application.

use iced::widget::{center, column, text};
use iced::{Element, Task as IcedTask, Theme};

/// Top-level application message type.
#[derive(Debug, Clone)]
pub enum Message {
    /// A daemon event was received.
    EventReceived(events::AppEvent),
    /// Tray requested the window to be shown.
    ShowWindow,
    /// Tray requested application quit.
    Quit,
}

/// Events that map from daemon proto events to application-level events.
pub mod events {
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
    }
}

/// Main application state.
pub struct App {
    connected: bool,
    battery_percentage: Option<u8>,
}

impl App {
    fn new() -> (Self, IcedTask<Message>) {
        (
            Self {
                connected: false,
                battery_percentage: None,
            },
            IcedTask::none(),
        )
    }

    fn update(&mut self, message: Message) -> IcedTask<Message> {
        match message {
            Message::EventReceived(event) => {
                match event {
                    events::AppEvent::ConnectionChanged { connected } => {
                        self.connected = connected;
                    }
                    events::AppEvent::BatteryUpdated { percentage } => {
                        self.battery_percentage = Some(percentage);
                    }
                    events::AppEvent::LowBattery { .. } => {
                        // Notification handled by notifications module
                    }
                    events::AppEvent::ProfileApplied { .. } => {
                        // Could refresh config display
                    }
                    events::AppEvent::SettingsChanged => {
                        // Could refresh config display
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
        let status = if self.connected {
            "Mouse: Connected"
        } else {
            "Mouse: Disconnected"
        };

        let battery = match self.battery_percentage {
            Some(pct) => format!("Battery: {pct}%"),
            None => "Battery: Unknown".to_string(),
        };

        center(
            column![text(status).size(24), text(battery).size(18),]
                .spacing(12)
                .align_x(iced::Alignment::Center),
        )
        .into()
    }

    fn theme(&self) -> Theme {
        Theme::Dark
    }
}

/// Run the iced application.
pub fn run() -> anyhow::Result<()> {
    iced::application(App::new, App::update, App::view)
        .theme(App::theme)
        .window_size((480.0, 360.0))
        .centered()
        .run()?;

    Ok(())
}
