//! Data types and enums for mouse configuration.

use std::fmt;

/// Polling rate options.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PollingRate {
    Hz125,
    Hz250,
    Hz500,
    Hz1000,
    Hz2000,
    Hz4000,
    Hz8000,
}

impl PollingRate {
    /// All polling rates in ascending order.
    pub const ALL: [PollingRate; 7] = [
        PollingRate::Hz125,
        PollingRate::Hz250,
        PollingRate::Hz500,
        PollingRate::Hz1000,
        PollingRate::Hz2000,
        PollingRate::Hz4000,
        PollingRate::Hz8000,
    ];

    /// Convert to the wire byte value.
    pub fn to_byte(self) -> u8 {
        match self {
            PollingRate::Hz125 => 0x08,
            PollingRate::Hz250 => 0x04,
            PollingRate::Hz500 => 0x02,
            PollingRate::Hz1000 => 0x01,
            PollingRate::Hz2000 => 0x10,
            PollingRate::Hz4000 => 0x20,
            PollingRate::Hz8000 => 0x40,
        }
    }

    /// Parse from wire byte value.
    pub fn from_byte(byte: u8) -> Option<Self> {
        match byte {
            0x08 => Some(PollingRate::Hz125),
            0x04 => Some(PollingRate::Hz250),
            0x02 => Some(PollingRate::Hz500),
            0x01 => Some(PollingRate::Hz1000),
            0x10 => Some(PollingRate::Hz2000),
            0x20 => Some(PollingRate::Hz4000),
            0x40 => Some(PollingRate::Hz8000),
            _ => None,
        }
    }

    /// Get the polling rate in Hz.
    pub fn to_hz(self) -> u16 {
        match self {
            PollingRate::Hz125 => 125,
            PollingRate::Hz250 => 250,
            PollingRate::Hz500 => 500,
            PollingRate::Hz1000 => 1000,
            PollingRate::Hz2000 => 2000,
            PollingRate::Hz4000 => 4000,
            PollingRate::Hz8000 => 8000,
        }
    }

    /// Cycle to the next polling rate (wraps around).
    pub fn next(self) -> Self {
        match self {
            PollingRate::Hz125 => PollingRate::Hz250,
            PollingRate::Hz250 => PollingRate::Hz500,
            PollingRate::Hz500 => PollingRate::Hz1000,
            PollingRate::Hz1000 => PollingRate::Hz2000,
            PollingRate::Hz2000 => PollingRate::Hz4000,
            PollingRate::Hz4000 => PollingRate::Hz8000,
            PollingRate::Hz8000 => PollingRate::Hz125,
        }
    }

    /// Cycle to the previous polling rate (wraps around).
    pub fn prev(self) -> Self {
        match self {
            PollingRate::Hz125 => PollingRate::Hz8000,
            PollingRate::Hz250 => PollingRate::Hz125,
            PollingRate::Hz500 => PollingRate::Hz250,
            PollingRate::Hz1000 => PollingRate::Hz500,
            PollingRate::Hz2000 => PollingRate::Hz1000,
            PollingRate::Hz4000 => PollingRate::Hz2000,
            PollingRate::Hz8000 => PollingRate::Hz4000,
        }
    }
}

impl fmt::Display for PollingRate {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{} Hz", self.to_hz())
    }
}

/// Lift-off distance options.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LiftOffDistance {
    /// 0.7mm (Low)
    Low,
    /// 1.0mm (Medium)
    Medium,
    /// 2.0mm (High)
    High,
}

impl LiftOffDistance {
    /// All lift-off distances in ascending order.
    pub const ALL: [LiftOffDistance; 3] = [
        LiftOffDistance::Low,
        LiftOffDistance::Medium,
        LiftOffDistance::High,
    ];

    /// Convert to the wire byte value.
    pub fn to_byte(self) -> u8 {
        match self {
            LiftOffDistance::Low => 0x03,
            LiftOffDistance::Medium => 0x01,
            LiftOffDistance::High => 0x02,
        }
    }

    /// Parse from wire byte value.
    pub fn from_byte(byte: u8) -> Option<Self> {
        match byte {
            0x03 => Some(LiftOffDistance::Low),
            0x01 => Some(LiftOffDistance::Medium),
            0x02 => Some(LiftOffDistance::High),
            _ => None,
        }
    }

    /// Get the distance in millimeters.
    pub fn to_mm(self) -> f32 {
        match self {
            LiftOffDistance::Low => 0.7,
            LiftOffDistance::Medium => 1.0,
            LiftOffDistance::High => 2.0,
        }
    }

    /// Cycle to the next lift-off distance (wraps around).
    pub fn next(self) -> Self {
        match self {
            LiftOffDistance::Low => LiftOffDistance::Medium,
            LiftOffDistance::Medium => LiftOffDistance::High,
            LiftOffDistance::High => LiftOffDistance::Low,
        }
    }

    /// Cycle to the previous lift-off distance (wraps around).
    pub fn prev(self) -> Self {
        match self {
            LiftOffDistance::Low => LiftOffDistance::High,
            LiftOffDistance::Medium => LiftOffDistance::Low,
            LiftOffDistance::High => LiftOffDistance::Medium,
        }
    }
}

impl fmt::Display for LiftOffDistance {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}mm", self.to_mm())
    }
}

/// Connection mode (wired vs wireless).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConnectionMode {
    /// USB wired connection (PID 0xf5f6, 64-byte packets)
    Wired,
    /// 2.4GHz wireless connection (PID 0xf5f7, 49-byte packets)
    Wireless,
}

impl ConnectionMode {
    /// Get the USB packet size for this connection mode.
    pub fn packet_size(self) -> usize {
        match self {
            ConnectionMode::Wired => 64,
            ConnectionMode::Wireless => 49,
        }
    }
}

impl fmt::Display for ConnectionMode {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ConnectionMode::Wired => write!(f, "Wired (USB)"),
            ConnectionMode::Wireless => write!(f, "Wireless (2.4GHz)"),
        }
    }
}

/// Full mouse configuration snapshot.
#[derive(Debug, Clone)]
pub struct MouseConfig {
    /// Polling rate in Hz.
    pub polling_rate: PollingRate,
    /// Lift-off distance.
    pub lift_off_distance: LiftOffDistance,
    /// Sleep timeout in seconds (0 = never sleep).
    pub sleep_timeout_seconds: u16,
    /// Angle snapping (motion smoothing).
    pub angle_snapping: bool,
    /// Ripple control.
    pub ripple_control: bool,
    /// High speed mode (competition mode).
    pub high_speed_mode: bool,
    /// Long distance mode (extended wireless range).
    pub long_distance_mode: bool,
}

/// Battery status information.
#[derive(Debug, Clone)]
pub struct BatteryStatus {
    /// Battery voltage in millivolts.
    pub voltage_mv: u16,
    /// Estimated battery percentage (0-100).
    pub percentage: u8,
}

/// Firmware version information.
#[derive(Debug, Clone)]
pub struct FirmwareInfo {
    /// Mouse firmware version string (e.g., "v2.22").
    pub mouse_version: String,
    /// Receiver firmware version string, if available (wireless mode only).
    pub receiver_version: Option<String>,
}
