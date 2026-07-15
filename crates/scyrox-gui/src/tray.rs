//! System tray integration via ksni (StatusNotifierItem / DBus).
//!
//! Provides a persistent tray icon with a context menu for quick access
//! to common operations and an "Open" action to show the main window.
//!
//! Menu activations are forwarded to the iced application over an
//! [`UnboundedSender`] rather than acting on the tray directly, so window
//! lifecycle and shutdown stay owned by the app.

use ksni::{self, menu::StandardItem};
use tokio::sync::mpsc::UnboundedSender;

/// Commands emitted by tray menu activations, consumed by the iced app.
#[derive(Debug, Clone, Copy)]
pub enum TrayCommand {
    /// Show (or focus) the main window.
    ShowWindow,
    /// Quit the application.
    Quit,
}

/// Tray icon state.
pub struct ScyroxTray {
    connected: bool,
    tx: UnboundedSender<TrayCommand>,
}

impl ScyroxTray {
    pub fn new(tx: UnboundedSender<TrayCommand>) -> Self {
        Self {
            connected: false,
            tx,
        }
    }

    pub fn set_connected(&mut self, connected: bool) {
        self.connected = connected;
    }
}

impl ksni::Tray for ScyroxTray {
    fn id(&self) -> String {
        "scyrox".to_string()
    }

    fn title(&self) -> String {
        if self.connected {
            "Scyrox - Connected".to_string()
        } else {
            "Scyrox - Disconnected".to_string()
        }
    }

    fn icon_name(&self) -> String {
        if self.connected {
            "input-mouse".to_string()
        } else {
            "input-mouse-symbolic".to_string()
        }
    }

    fn menu(&self) -> Vec<ksni::MenuItem<Self>> {
        vec![
            StandardItem {
                label: "Open Scyrox".to_string(),
                activate: Box::new(|this: &mut Self| {
                    let _ = this.tx.send(TrayCommand::ShowWindow);
                }),
                ..Default::default()
            }
            .into(),
            ksni::MenuItem::Separator,
            StandardItem {
                label: if self.connected {
                    "Status: Connected".to_string()
                } else {
                    "Status: Disconnected".to_string()
                },
                enabled: false,
                ..Default::default()
            }
            .into(),
            ksni::MenuItem::Separator,
            StandardItem {
                label: "Quit".to_string(),
                activate: Box::new(|this: &mut Self| {
                    let _ = this.tx.send(TrayCommand::Quit);
                }),
                ..Default::default()
            }
            .into(),
        ]
    }
}
