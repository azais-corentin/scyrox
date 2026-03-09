//! System tray integration via ksni (StatusNotifierItem / DBus).
//!
//! Provides a persistent tray icon with a context menu for quick access
//! to common operations and an "Open" action to show the main window.

use ksni::{self, menu::StandardItem};

/// Tray icon state.
pub struct ScyroxTray {
    connected: bool,
}

impl ScyroxTray {
    pub fn new() -> Self {
        Self { connected: false }
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
                activate: Box::new(|_| {
                    // TODO: send ShowWindow message to iced app
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
                activate: Box::new(|_| {
                    // TODO: send Quit message to iced app
                    std::process::exit(0);
                }),
                ..Default::default()
            }
            .into(),
        ]
    }
}
