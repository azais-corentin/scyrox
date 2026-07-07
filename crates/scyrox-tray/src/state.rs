//! Tray state model and pure text-formatting helpers.

/// Low-battery threshold (percentage, inclusive).
///
/// Mirrors the daemon default in `crates/scyroxd/src/config.rs`
/// (`default_low_battery_threshold()` returns 20). Hardcoded here because the
/// daemon does not currently expose the configured threshold over gRPC.
pub const LOW_BATTERY_THRESHOLD: u8 = 20;

/// Observable state of the tray, driven by the daemon event worker.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TrayState {
    /// Daemon unreachable (socket connect failed / stream ended).
    DaemonDown,
    /// Daemon up, mouse disconnected or asleep.
    Disconnected,
    /// Daemon up, mouse connected.
    Battery {
        percentage: u8,
        voltage_mv: u16,
        charging: bool,
    },
}

/// Tooltip text shown on hover.
pub fn tooltip(state: &TrayState) -> String {
    match state {
        TrayState::DaemonDown => "Scyrox — daemon not running".to_string(),
        TrayState::Disconnected => "Scyrox — mouse disconnected".to_string(),
        TrayState::Battery {
            percentage,
            charging,
            ..
        } => {
            if *charging {
                format!("Scyrox — {percentage}% (charging)")
            } else {
                format!("Scyrox — {percentage}%")
            }
        }
    }
}

/// Disabled status line shown in the context menu.
pub fn menu_line(state: &TrayState) -> String {
    match state {
        TrayState::DaemonDown => "Daemon unreachable".to_string(),
        TrayState::Disconnected => "Mouse disconnected".to_string(),
        TrayState::Battery {
            percentage,
            voltage_mv,
            charging,
        } => {
            let volts = *voltage_mv as f32 / 1000.0;
            if *charging {
                format!("Battery: {percentage}% ({volts:.2} V), charging")
            } else {
                format!("Battery: {percentage}% ({volts:.2} V)")
            }
        }
    }
}

/// Whether the state should be rendered as a low-battery warning.
///
/// A charging mouse is never "low".
pub fn is_low(state: &TrayState) -> bool {
    matches!(
        state,
        TrayState::Battery {
            percentage,
            charging: false,
            ..
        } if *percentage <= LOW_BATTERY_THRESHOLD
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tooltip_battery() {
        assert_eq!(
            tooltip(&TrayState::Battery {
                percentage: 85,
                voltage_mv: 3900,
                charging: false,
            }),
            "Scyrox — 85%"
        );
    }

    #[test]
    fn tooltip_battery_charging() {
        assert_eq!(
            tooltip(&TrayState::Battery {
                percentage: 85,
                voltage_mv: 3900,
                charging: true,
            }),
            "Scyrox — 85% (charging)"
        );
    }

    #[test]
    fn tooltip_disconnected() {
        assert_eq!(
            tooltip(&TrayState::Disconnected),
            "Scyrox — mouse disconnected"
        );
    }

    #[test]
    fn tooltip_daemon_down() {
        assert_eq!(
            tooltip(&TrayState::DaemonDown),
            "Scyrox — daemon not running"
        );
    }

    #[test]
    fn menu_line_battery() {
        assert_eq!(
            menu_line(&TrayState::Battery {
                percentage: 85,
                voltage_mv: 3900,
                charging: false,
            }),
            "Battery: 85% (3.90 V)"
        );
    }

    #[test]
    fn menu_line_battery_charging() {
        assert_eq!(
            menu_line(&TrayState::Battery {
                percentage: 85,
                voltage_mv: 3900,
                charging: true,
            }),
            "Battery: 85% (3.90 V), charging"
        );
    }

    #[test]
    fn menu_line_disconnected() {
        assert_eq!(menu_line(&TrayState::Disconnected), "Mouse disconnected");
    }

    #[test]
    fn menu_line_daemon_down() {
        assert_eq!(menu_line(&TrayState::DaemonDown), "Daemon unreachable");
    }

    #[test]
    fn is_low_boundary() {
        // 20 -> true (inclusive), 21 -> false.
        assert!(is_low(&TrayState::Battery {
            percentage: 20,
            voltage_mv: 3600,
            charging: false,
        }));
        assert!(!is_low(&TrayState::Battery {
            percentage: 21,
            voltage_mv: 3600,
            charging: false,
        }));
    }

    #[test]
    fn is_low_charging_never_low() {
        assert!(!is_low(&TrayState::Battery {
            percentage: 20,
            voltage_mv: 3600,
            charging: true,
        }));
    }

    #[test]
    fn is_low_non_battery_states() {
        assert!(!is_low(&TrayState::DaemonDown));
        assert!(!is_low(&TrayState::Disconnected));
    }
}
