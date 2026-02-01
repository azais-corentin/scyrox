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

/// Connection mode (wired vs wireless) - simple classification based on PID.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConnectionMode {
    /// USB wired connection (PID 0xf5f6, 64-byte packets)
    Wired,
    /// 2.4GHz wireless connection (PID 0xf5f7 or 0xf5f4, 49-byte packets)
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

/// Connection type as reported by the device in handshake response.
///
/// This provides more detailed information about the connection type
/// and maximum supported polling rate.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum ConnectionType {
    /// Wireless standard (1000 Hz max)
    WirelessStandard = 0,
    /// Wireless 4K dongle (4000 Hz max)
    Wireless4K = 1,
    /// Wired standard (1000 Hz max)
    WiredStandard = 2,
    /// Wired high-speed (8000 Hz max)
    WiredHighSpeed = 3,
    /// Wireless 2K dongle (2000 Hz max)
    Wireless2K = 4,
    /// Wireless 8K dongle (8000 Hz max)
    Wireless8K = 5,
}

impl ConnectionType {
    /// Parse from wire byte value.
    pub fn from_byte(byte: u8) -> Option<Self> {
        match byte {
            0 => Some(ConnectionType::WirelessStandard),
            1 => Some(ConnectionType::Wireless4K),
            2 => Some(ConnectionType::WiredStandard),
            3 => Some(ConnectionType::WiredHighSpeed),
            4 => Some(ConnectionType::Wireless2K),
            5 => Some(ConnectionType::Wireless8K),
            _ => None,
        }
    }

    /// Get the maximum polling rate in Hz for this connection type.
    pub fn max_polling_rate_hz(self) -> u16 {
        match self {
            ConnectionType::WirelessStandard | ConnectionType::WiredStandard => 1000,
            ConnectionType::Wireless2K => 2000,
            ConnectionType::Wireless4K => 4000,
            ConnectionType::WiredHighSpeed | ConnectionType::Wireless8K => 8000,
        }
    }

    /// Check if this is a wireless connection.
    pub fn is_wireless(self) -> bool {
        matches!(
            self,
            ConnectionType::WirelessStandard
                | ConnectionType::Wireless4K
                | ConnectionType::Wireless2K
                | ConnectionType::Wireless8K
        )
    }
}

impl fmt::Display for ConnectionType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ConnectionType::WirelessStandard => write!(f, "Wireless (1000 Hz)"),
            ConnectionType::Wireless4K => write!(f, "Wireless 4K (4000 Hz)"),
            ConnectionType::WiredStandard => write!(f, "Wired (1000 Hz)"),
            ConnectionType::WiredHighSpeed => write!(f, "Wired High-Speed (8000 Hz)"),
            ConnectionType::Wireless2K => write!(f, "Wireless 2K (2000 Hz)"),
            ConnectionType::Wireless8K => write!(f, "Wireless 8K (8000 Hz)"),
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
    /// Debounce time in milliseconds (0-30).
    pub debounce_time: u8,
    /// Motion sync enabled.
    pub motion_sync: bool,
    /// Moving off light time.
    pub moving_off_light_time: u8,
    /// Performance/sleep time value.
    pub performance_time: SleepTime,
    /// Sensor mode (low power vs high performance).
    pub sensor_mode: SensorMode,
    /// 20K sensor mode enabled.
    pub sensor_20k: bool,
}

/// Battery status information.
#[derive(Debug, Clone)]
pub struct BatteryStatus {
    /// Battery voltage in millivolts.
    pub voltage_mv: u16,
    /// Battery percentage (0-100) as reported by the device.
    pub percentage: u8,
    /// Whether the battery is currently charging.
    pub charging: bool,
}

/// Firmware version information.
#[derive(Debug, Clone)]
pub struct FirmwareInfo {
    /// Mouse firmware version string (e.g., "v2.22").
    pub mouse_version: String,
    /// Receiver firmware version string, if available (wireless mode only).
    pub receiver_version: Option<String>,
}

/// Device identification information from handshake.
#[derive(Debug, Clone)]
pub struct DeviceInfo {
    /// Company ID.
    pub cid: u8,
    /// Model ID.
    pub mid: u8,
    /// Connection type.
    pub connection_type: ConnectionType,
    /// Whether the mouse is online (connected to dongle).
    pub online: bool,
    /// Device address (3 bytes, for wireless pairing).
    pub address: [u8; 3],
}

impl Default for DeviceInfo {
    fn default() -> Self {
        Self {
            cid: 0,
            mid: 0,
            connection_type: ConnectionType::WirelessStandard,
            online: false,
            address: [0; 3],
        }
    }
}

/// Pairing status.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum PairStatus {
    /// Idle / Not pairing.
    Idle = 0,
    /// Pairing in progress.
    Pairing = 1,
    /// Pairing failed.
    Failed = 2,
    /// Pairing successful.
    Success = 3,
}

impl PairStatus {
    /// Parse from wire byte value.
    pub fn from_byte(byte: u8) -> Option<Self> {
        match byte {
            0 => Some(PairStatus::Idle),
            1 => Some(PairStatus::Pairing),
            2 => Some(PairStatus::Failed),
            3 => Some(PairStatus::Success),
            _ => None,
        }
    }
}

impl fmt::Display for PairStatus {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            PairStatus::Idle => write!(f, "Idle"),
            PairStatus::Pairing => write!(f, "Pairing"),
            PairStatus::Failed => write!(f, "Failed"),
            PairStatus::Success => write!(f, "Success"),
        }
    }
}

/// DPI stage configuration.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DpiStage {
    /// DPI value (50-26000, in steps of 50).
    pub value: u16,
    /// RGB color for this DPI stage.
    pub color: [u8; 3],
}

impl Default for DpiStage {
    fn default() -> Self {
        Self {
            value: 800,
            color: [255, 255, 255],
        }
    }
}

/// Key function type.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum KeyFunctionType {
    /// Key disabled.
    Disabled = 0,
    /// Mouse button.
    MouseButton = 1,
    /// DPI switch.
    DpiSwitch = 2,
    /// Scroll wheel.
    ScrollWheel = 3,
    /// Fire key (rapid fire).
    FireKey = 4,
    /// Keyboard shortcut.
    ShortcutKey = 5,
    /// Macro.
    Macro = 6,
    /// Report rate switch.
    ReportRateSwitch = 7,
}

impl KeyFunctionType {
    /// Parse from wire byte value.
    pub fn from_byte(byte: u8) -> Option<Self> {
        match byte {
            0 => Some(KeyFunctionType::Disabled),
            1 => Some(KeyFunctionType::MouseButton),
            2 => Some(KeyFunctionType::DpiSwitch),
            3 => Some(KeyFunctionType::ScrollWheel),
            4 => Some(KeyFunctionType::FireKey),
            5 => Some(KeyFunctionType::ShortcutKey),
            6 => Some(KeyFunctionType::Macro),
            7 => Some(KeyFunctionType::ReportRateSwitch),
            _ => None,
        }
    }
}

impl fmt::Display for KeyFunctionType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            KeyFunctionType::Disabled => write!(f, "Disabled"),
            KeyFunctionType::MouseButton => write!(f, "Mouse Button"),
            KeyFunctionType::DpiSwitch => write!(f, "DPI Switch"),
            KeyFunctionType::ScrollWheel => write!(f, "Scroll Wheel"),
            KeyFunctionType::FireKey => write!(f, "Fire Key"),
            KeyFunctionType::ShortcutKey => write!(f, "Shortcut Key"),
            KeyFunctionType::Macro => write!(f, "Macro"),
            KeyFunctionType::ReportRateSwitch => write!(f, "Report Rate Switch"),
        }
    }
}

/// Mouse button codes (for KeyFunctionType::MouseButton).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u16)]
pub enum MouseButton {
    /// Left click.
    Left = 0x0100,
    /// Right click.
    Right = 0x0200,
    /// Middle click.
    Middle = 0x0400,
    /// Back button.
    Back = 0x0800,
    /// Forward button.
    Forward = 0x1000,
}

impl MouseButton {
    /// Parse from wire value.
    pub fn from_u16(value: u16) -> Option<Self> {
        match value {
            0x0100 => Some(MouseButton::Left),
            0x0200 => Some(MouseButton::Right),
            0x0400 => Some(MouseButton::Middle),
            0x0800 => Some(MouseButton::Back),
            0x1000 => Some(MouseButton::Forward),
            _ => None,
        }
    }
}

impl fmt::Display for MouseButton {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            MouseButton::Left => write!(f, "Left Click"),
            MouseButton::Right => write!(f, "Right Click"),
            MouseButton::Middle => write!(f, "Middle Click"),
            MouseButton::Back => write!(f, "Back"),
            MouseButton::Forward => write!(f, "Forward"),
        }
    }
}

/// DPI switch mode codes (for KeyFunctionType::DpiSwitch).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u16)]
pub enum DpiSwitchMode {
    /// Cycle through DPI stages.
    Cycle = 0x0100,
    /// Increase DPI stage.
    Up = 0x0200,
    /// Decrease DPI stage.
    Down = 0x0300,
}

impl DpiSwitchMode {
    /// Parse from wire value.
    pub fn from_u16(value: u16) -> Option<Self> {
        match value {
            0x0100 => Some(DpiSwitchMode::Cycle),
            0x0200 => Some(DpiSwitchMode::Up),
            0x0300 => Some(DpiSwitchMode::Down),
            _ => None,
        }
    }
}

impl fmt::Display for DpiSwitchMode {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            DpiSwitchMode::Cycle => write!(f, "DPI Cycle"),
            DpiSwitchMode::Up => write!(f, "DPI Up"),
            DpiSwitchMode::Down => write!(f, "DPI Down"),
        }
    }
}

/// Scroll wheel direction codes (for KeyFunctionType::ScrollWheel).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u16)]
pub enum ScrollWheelDirection {
    /// Scroll left.
    Left = 0x0100,
    /// Scroll right.
    Right = 0x0200,
}

impl ScrollWheelDirection {
    /// Parse from wire value.
    pub fn from_u16(value: u16) -> Option<Self> {
        match value {
            0x0100 => Some(ScrollWheelDirection::Left),
            0x0200 => Some(ScrollWheelDirection::Right),
            _ => None,
        }
    }
}

impl fmt::Display for ScrollWheelDirection {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ScrollWheelDirection::Left => write!(f, "Scroll Left"),
            ScrollWheelDirection::Right => write!(f, "Scroll Right"),
        }
    }
}

/// Fire key configuration (for KeyFunctionType::FireKey).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FireKeyConfig {
    /// Interval between clicks in milliseconds (10-255).
    pub interval_ms: u8,
    /// Repeat count (0-3, 0 = hold to repeat).
    pub repeat_count: u8,
}

impl Default for FireKeyConfig {
    fn default() -> Self {
        Self {
            interval_ms: 50,
            repeat_count: 0,
        }
    }
}

impl FireKeyConfig {
    /// Encode to wire format (2 bytes: high=interval, low=repeat).
    pub fn to_u16(self) -> u16 {
        ((self.interval_ms as u16) << 8) | (self.repeat_count as u16)
    }

    /// Decode from wire format.
    pub fn from_u16(value: u16) -> Self {
        Self {
            interval_ms: ((value >> 8) & 0xFF) as u8,
            repeat_count: (value & 0xFF) as u8,
        }
    }
}

impl fmt::Display for FireKeyConfig {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.repeat_count == 0 {
            write!(f, "Fire Key ({}ms, hold to repeat)", self.interval_ms)
        } else {
            write!(
                f,
                "Fire Key ({}ms, {} clicks)",
                self.interval_ms, self.repeat_count
            )
        }
    }
}

/// Macro key reference configuration (for KeyFunctionType::Macro).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MacroKeyConfig {
    /// Macro slot index (0-7).
    pub slot: u8,
    /// Cycle count (1-255, 253-255 = special modes).
    pub cycle_count: u8,
}

impl Default for MacroKeyConfig {
    fn default() -> Self {
        Self {
            slot: 0,
            cycle_count: 1,
        }
    }
}

impl MacroKeyConfig {
    /// Encode to wire format (2 bytes: high=slot, low=cycle).
    pub fn to_u16(self) -> u16 {
        ((self.slot as u16) << 8) | (self.cycle_count as u16)
    }

    /// Decode from wire format.
    pub fn from_u16(value: u16) -> Self {
        Self {
            slot: ((value >> 8) & 0xFF) as u8,
            cycle_count: (value & 0xFF) as u8,
        }
    }
}

impl fmt::Display for MacroKeyConfig {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "Macro {} ({})",
            self.slot,
            MacroCycleMode::from_byte(self.cycle_count)
        )
    }
}

/// Key function configuration.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct KeyFunction {
    /// Function type.
    pub function_type: KeyFunctionType,
    /// Function parameter (meaning depends on type).
    pub parameter: u16,
}

impl Default for KeyFunction {
    fn default() -> Self {
        Self {
            function_type: KeyFunctionType::Disabled,
            parameter: 0,
        }
    }
}

impl KeyFunction {
    /// Create a disabled key function.
    pub fn disabled() -> Self {
        Self {
            function_type: KeyFunctionType::Disabled,
            parameter: 0,
        }
    }

    /// Create a mouse button key function.
    pub fn mouse_button(button: MouseButton) -> Self {
        Self {
            function_type: KeyFunctionType::MouseButton,
            parameter: button as u16,
        }
    }

    /// Create a DPI switch key function.
    pub fn dpi_switch(mode: DpiSwitchMode) -> Self {
        Self {
            function_type: KeyFunctionType::DpiSwitch,
            parameter: mode as u16,
        }
    }

    /// Create a scroll wheel key function.
    pub fn scroll_wheel(direction: ScrollWheelDirection) -> Self {
        Self {
            function_type: KeyFunctionType::ScrollWheel,
            parameter: direction as u16,
        }
    }

    /// Create a fire key function.
    pub fn fire_key(config: FireKeyConfig) -> Self {
        Self {
            function_type: KeyFunctionType::FireKey,
            parameter: config.to_u16(),
        }
    }

    /// Create a shortcut key function (reference to shortcut slot).
    pub fn shortcut_key(slot: u8) -> Self {
        Self {
            function_type: KeyFunctionType::ShortcutKey,
            parameter: slot as u16,
        }
    }

    /// Create a macro key function.
    pub fn macro_key(config: MacroKeyConfig) -> Self {
        Self {
            function_type: KeyFunctionType::Macro,
            parameter: config.to_u16(),
        }
    }

    /// Create a report rate switch key function.
    pub fn report_rate_switch() -> Self {
        Self {
            function_type: KeyFunctionType::ReportRateSwitch,
            parameter: 0,
        }
    }

    /// Encode to wire format (4 bytes).
    pub fn encode(&self) -> [u8; 4] {
        let type_byte = self.function_type as u8;
        let param_high = ((self.parameter >> 8) & 0xFF) as u8;
        let param_low = (self.parameter & 0xFF) as u8;
        let checksum = 0x55u8
            .wrapping_sub(type_byte)
            .wrapping_sub(param_high)
            .wrapping_sub(param_low);
        [type_byte, param_high, param_low, checksum]
    }

    /// Decode from wire format (4 bytes).
    pub fn decode(bytes: &[u8; 4]) -> Option<Self> {
        let function_type = KeyFunctionType::from_byte(bytes[0])?;
        let parameter = ((bytes[1] as u16) << 8) | (bytes[2] as u16);
        Some(Self {
            function_type,
            parameter,
        })
    }
}

/// Light mode options.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum LightMode {
    /// Light off.
    Off = 0,
    /// Color flow (rainbow cycle).
    ColorFlow = 1,
    /// Single color breathing.
    SingleColorBreathing = 2,
    /// Constant single color.
    ConstantColor = 3,
    /// Neon effect.
    Neon = 4,
    /// Mixed color breathing.
    MixedColorBreathing = 5,
    /// Colorful constant.
    ColorfulConstant = 6,
}

impl LightMode {
    /// Parse from wire byte value.
    pub fn from_byte(byte: u8) -> Option<Self> {
        match byte {
            0 => Some(LightMode::Off),
            1 => Some(LightMode::ColorFlow),
            2 => Some(LightMode::SingleColorBreathing),
            3 => Some(LightMode::ConstantColor),
            4 => Some(LightMode::Neon),
            5 => Some(LightMode::MixedColorBreathing),
            6 => Some(LightMode::ColorfulConstant),
            _ => None,
        }
    }
}

impl fmt::Display for LightMode {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            LightMode::Off => write!(f, "Off"),
            LightMode::ColorFlow => write!(f, "Color Flow"),
            LightMode::SingleColorBreathing => write!(f, "Single Color Breathing"),
            LightMode::ConstantColor => write!(f, "Constant Color"),
            LightMode::Neon => write!(f, "Neon"),
            LightMode::MixedColorBreathing => write!(f, "Mixed Color Breathing"),
            LightMode::ColorfulConstant => write!(f, "Colorful Constant"),
        }
    }
}

/// Light settings configuration.
#[derive(Debug, Clone)]
pub struct LightSettings {
    /// Light mode.
    pub mode: LightMode,
    /// RGB color.
    pub color: [u8; 3],
    /// Speed (1-10).
    pub speed: u8,
    /// Brightness (0-255).
    pub brightness: u8,
    /// On/off state.
    pub enabled: bool,
}

impl Default for LightSettings {
    fn default() -> Self {
        Self {
            mode: LightMode::ConstantColor,
            color: [255, 255, 255],
            speed: 5,
            brightness: 128,
            enabled: true,
        }
    }
}

/// DPI effect mode.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum DpiEffectMode {
    /// Effect off.
    Off = 0,
    /// Constant color.
    Constant = 1,
    /// Breathing effect.
    Breathing = 2,
}

impl DpiEffectMode {
    /// Parse from wire byte value.
    pub fn from_byte(byte: u8) -> Option<Self> {
        match byte {
            0 => Some(DpiEffectMode::Off),
            1 => Some(DpiEffectMode::Constant),
            2 => Some(DpiEffectMode::Breathing),
            _ => None,
        }
    }
}

impl fmt::Display for DpiEffectMode {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            DpiEffectMode::Off => write!(f, "Off"),
            DpiEffectMode::Constant => write!(f, "Constant"),
            DpiEffectMode::Breathing => write!(f, "Breathing"),
        }
    }
}

/// DPI effect settings configuration.
#[derive(Debug, Clone)]
pub struct DpiEffectSettings {
    /// Effect mode.
    pub mode: DpiEffectMode,
    /// Brightness level (1-10).
    pub brightness: u8,
    /// Speed (1-10).
    pub speed: u8,
    /// On/off state.
    pub enabled: bool,
}

impl Default for DpiEffectSettings {
    fn default() -> Self {
        Self {
            mode: DpiEffectMode::Off,
            brightness: 5,
            speed: 5,
            enabled: false,
        }
    }
}

/// Sensor mode.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum SensorMode {
    /// Low power mode.
    LowPower = 0,
    /// High performance mode.
    HighPerformance = 1,
}

impl SensorMode {
    /// Parse from wire byte value.
    pub fn from_byte(byte: u8) -> Option<Self> {
        match byte {
            0 => Some(SensorMode::LowPower),
            1 => Some(SensorMode::HighPerformance),
            _ => None,
        }
    }
}

impl fmt::Display for SensorMode {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            SensorMode::LowPower => write!(f, "Low Power"),
            SensorMode::HighPerformance => write!(f, "High Performance"),
        }
    }
}

/// Sleep/Performance time values.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SleepTime {
    /// 10 seconds.
    Sec10,
    /// 30 seconds.
    Sec30,
    /// 1 minute.
    Min1,
    /// 5 minutes.
    Min5,
    /// 10 minutes.
    Min10,
    /// 30 minutes.
    Min30,
}

impl SleepTime {
    /// Convert to wire byte value.
    pub fn to_byte(self) -> u8 {
        match self {
            SleepTime::Sec10 => 1,
            SleepTime::Sec30 => 3,
            SleepTime::Min1 => 6,
            SleepTime::Min5 => 30,
            SleepTime::Min10 => 60,
            SleepTime::Min30 => 180,
        }
    }

    /// Parse from wire byte value.
    pub fn from_byte(byte: u8) -> Option<Self> {
        match byte {
            1 => Some(SleepTime::Sec10),
            3 => Some(SleepTime::Sec30),
            6 => Some(SleepTime::Min1),
            30 => Some(SleepTime::Min5),
            60 => Some(SleepTime::Min10),
            180 => Some(SleepTime::Min30),
            _ => None,
        }
    }

    /// Get the time in seconds.
    pub fn to_seconds(self) -> u16 {
        match self {
            SleepTime::Sec10 => 10,
            SleepTime::Sec30 => 30,
            SleepTime::Min1 => 60,
            SleepTime::Min5 => 300,
            SleepTime::Min10 => 600,
            SleepTime::Min30 => 1800,
        }
    }
}

impl fmt::Display for SleepTime {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            SleepTime::Sec10 => write!(f, "10 seconds"),
            SleepTime::Sec30 => write!(f, "30 seconds"),
            SleepTime::Min1 => write!(f, "1 minute"),
            SleepTime::Min5 => write!(f, "5 minutes"),
            SleepTime::Min10 => write!(f, "10 minutes"),
            SleepTime::Min30 => write!(f, "30 minutes"),
        }
    }
}

/// Status change flags bitmask.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct StatusChangeFlags(pub u8);

impl StatusChangeFlags {
    /// Current DPI changed.
    pub const DPI_CHANGED: u8 = 0x01;
    /// Report rate changed.
    pub const REPORT_RATE_CHANGED: u8 = 0x02;
    /// Profile changed.
    pub const PROFILE_CHANGED: u8 = 0x04;
    /// DPI settings changed.
    pub const DPI_SETTINGS_CHANGED: u8 = 0x08;
    /// Light settings changed.
    pub const LIGHT_SETTINGS_CHANGED: u8 = 0x20;
    /// Battery status changed.
    pub const BATTERY_CHANGED: u8 = 0x40;

    /// Check if DPI changed.
    pub fn dpi_changed(self) -> bool {
        self.0 & Self::DPI_CHANGED != 0
    }

    /// Check if report rate changed.
    pub fn report_rate_changed(self) -> bool {
        self.0 & Self::REPORT_RATE_CHANGED != 0
    }

    /// Check if profile changed.
    pub fn profile_changed(self) -> bool {
        self.0 & Self::PROFILE_CHANGED != 0
    }

    /// Check if DPI settings changed.
    pub fn dpi_settings_changed(self) -> bool {
        self.0 & Self::DPI_SETTINGS_CHANGED != 0
    }

    /// Check if light settings changed.
    pub fn light_settings_changed(self) -> bool {
        self.0 & Self::LIGHT_SETTINGS_CHANGED != 0
    }

    /// Check if battery status changed.
    pub fn battery_changed(self) -> bool {
        self.0 & Self::BATTERY_CHANGED != 0
    }
}

/// Macro cycle mode.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MacroCycleMode {
    /// Repeat N times (1-250).
    Count(u8),
    /// Loop until key pressed again.
    UntilKeyPressedAgain,
    /// Loop until key released.
    UntilKeyReleased,
    /// Loop until any key pressed.
    UntilAnyKeyPressed,
}

impl MacroCycleMode {
    /// Convert to wire byte value.
    pub fn to_byte(self) -> u8 {
        match self {
            MacroCycleMode::Count(n) => n.clamp(1, 250),
            MacroCycleMode::UntilKeyPressedAgain => 253,
            MacroCycleMode::UntilKeyReleased => 254,
            MacroCycleMode::UntilAnyKeyPressed => 255,
        }
    }

    /// Parse from wire byte value.
    pub fn from_byte(byte: u8) -> Self {
        match byte {
            253 => MacroCycleMode::UntilKeyPressedAgain,
            254 => MacroCycleMode::UntilKeyReleased,
            255 => MacroCycleMode::UntilAnyKeyPressed,
            n => MacroCycleMode::Count(n.max(1).min(250)),
        }
    }
}

impl fmt::Display for MacroCycleMode {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            MacroCycleMode::Count(n) => write!(f, "{} times", n),
            MacroCycleMode::UntilKeyPressedAgain => write!(f, "Until key pressed again"),
            MacroCycleMode::UntilKeyReleased => write!(f, "Until key released"),
            MacroCycleMode::UntilAnyKeyPressed => write!(f, "Until any key pressed"),
        }
    }
}

/// Shortcut key event type (bits 0-3 of event type byte).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum ShortcutKeyType {
    /// Modifier key (Ctrl, Shift, Alt, Win).
    Modifier = 0,
    /// Normal keyboard key.
    Normal = 1,
    /// Media/consumer key.
    Media = 2,
}

impl ShortcutKeyType {
    /// Parse from wire byte value.
    pub fn from_byte(byte: u8) -> Option<Self> {
        match byte & 0x0F {
            0 => Some(ShortcutKeyType::Modifier),
            1 => Some(ShortcutKeyType::Normal),
            2 => Some(ShortcutKeyType::Media),
            _ => None,
        }
    }
}

impl fmt::Display for ShortcutKeyType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ShortcutKeyType::Modifier => write!(f, "Modifier"),
            ShortcutKeyType::Normal => write!(f, "Normal"),
            ShortcutKeyType::Media => write!(f, "Media"),
        }
    }
}

/// Shortcut key event.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ShortcutKeyEvent {
    /// Key down event (true) or key up (false).
    pub key_down: bool,
    /// Key type.
    pub key_type: ShortcutKeyType,
    /// Key code.
    pub key_code: u16,
}

impl ShortcutKeyEvent {
    /// Encode to wire format (3 bytes).
    ///
    /// Per protocol spec:
    /// - Byte 0: Event type (bit 7=key down, bit 6=key up, bits 0-3=key type)
    /// - Byte 1: Key code low byte
    /// - Byte 2: Key code high byte
    pub fn encode(&self) -> [u8; 3] {
        let mut type_byte = self.key_type as u8;
        if self.key_down {
            type_byte |= 0x80;
        } else {
            type_byte |= 0x40;
        }
        [
            type_byte,
            (self.key_code & 0xFF) as u8,
            ((self.key_code >> 8) & 0xFF) as u8,
        ]
    }

    /// Decode from wire format (3 bytes).
    pub fn decode(bytes: &[u8; 3]) -> Option<Self> {
        let key_down = (bytes[0] & 0x80) != 0;
        let key_type = ShortcutKeyType::from_byte(bytes[0])?;
        let key_code = (bytes[1] as u16) | ((bytes[2] as u16) << 8);
        Some(Self {
            key_down,
            key_type,
            key_code,
        })
    }
}

/// Shortcut key definition (up to 10 events).
#[derive(Debug, Clone)]
pub struct ShortcutKey {
    /// Events in the shortcut (both key down and key up).
    pub events: Vec<ShortcutKeyEvent>,
}

impl Default for ShortcutKey {
    fn default() -> Self {
        Self { events: Vec::new() }
    }
}

impl ShortcutKey {
    /// Maximum events per shortcut key.
    pub const MAX_EVENTS: usize = 10;

    /// Create a new empty shortcut key.
    pub fn new() -> Self {
        Self::default()
    }

    /// Add a key press (down and up events).
    pub fn add_key_press(&mut self, key_type: ShortcutKeyType, key_code: u16) {
        if self.events.len() + 2 <= Self::MAX_EVENTS * 2 {
            self.events.push(ShortcutKeyEvent {
                key_down: true,
                key_type,
                key_code,
            });
            self.events.push(ShortcutKeyEvent {
                key_down: false,
                key_type,
                key_code,
            });
        }
    }

    /// Encode to wire format (32 bytes).
    pub fn encode(&self) -> [u8; 32] {
        let mut data = [0u8; 32];
        data[0] = self.events.len() as u8;

        for (i, event) in self.events.iter().enumerate().take(Self::MAX_EVENTS * 2) {
            let encoded = event.encode();
            let offset = 1 + i * 3;
            if offset + 3 <= 32 {
                data[offset..offset + 3].copy_from_slice(&encoded);
            }
        }

        data
    }

    /// Decode from wire format (32 bytes).
    pub fn decode(data: &[u8; 32]) -> Option<Self> {
        let event_count = data[0] as usize;
        if event_count > Self::MAX_EVENTS * 2 {
            return None;
        }

        let mut events = Vec::with_capacity(event_count);
        for i in 0..event_count {
            let offset = 1 + i * 3;
            if offset + 3 <= 32 {
                let bytes = [data[offset], data[offset + 1], data[offset + 2]];
                if let Some(event) = ShortcutKeyEvent::decode(&bytes) {
                    events.push(event);
                }
            }
        }

        Some(Self { events })
    }
}

/// Macro event key type.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum MacroEventKeyType {
    /// Keyboard key.
    Keyboard = 1,
    /// Mouse button.
    Mouse = 4,
}

impl MacroEventKeyType {
    /// Parse from wire byte value.
    pub fn from_byte(byte: u8) -> Option<Self> {
        match byte & 0x0F {
            1 => Some(MacroEventKeyType::Keyboard),
            4 => Some(MacroEventKeyType::Mouse),
            _ => None,
        }
    }
}

impl fmt::Display for MacroEventKeyType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            MacroEventKeyType::Keyboard => write!(f, "Keyboard"),
            MacroEventKeyType::Mouse => write!(f, "Mouse"),
        }
    }
}

/// Macro event.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MacroEvent {
    /// Whether this is a key down event (true) or key up event (false).
    pub key_down: bool,
    /// Key type (1 = keyboard, 4 = mouse).
    pub key_type: MacroEventKeyType,
    /// Key code.
    pub key_code: u16,
    /// Delay in milliseconds after this event.
    pub delay_ms: u16,
}

impl MacroEvent {
    /// Macro event status: Key down (bits 6-7 = 01).
    const KEY_DOWN_FLAG: u8 = 0x40;
    /// Macro event status: Key up (bits 6-7 = 10).
    const KEY_UP_FLAG: u8 = 0x80;

    /// Encode to wire format (5 bytes).
    ///
    /// Per protocol spec section 6.14:
    /// - Byte 0: Status and type (bits 6-7=status, bits 0-3=key type)
    /// - Byte 1: Key code low byte
    /// - Byte 2: Key code high byte
    /// - Byte 3: Delay high byte
    /// - Byte 4: Delay low byte
    pub fn encode(&self) -> [u8; 5] {
        let mut status_type = self.key_type as u8;
        if self.key_down {
            status_type |= Self::KEY_DOWN_FLAG;
        } else {
            status_type |= Self::KEY_UP_FLAG;
        }

        [
            status_type,
            (self.key_code & 0xFF) as u8,
            ((self.key_code >> 8) & 0xFF) as u8,
            ((self.delay_ms >> 8) & 0xFF) as u8,
            (self.delay_ms & 0xFF) as u8,
        ]
    }

    /// Decode from wire format (5 bytes).
    pub fn decode(bytes: &[u8; 5]) -> Option<Self> {
        let status_type = bytes[0];
        let key_down = (status_type & 0xC0) == Self::KEY_DOWN_FLAG;
        let key_type = MacroEventKeyType::from_byte(status_type)?;
        let key_code = (bytes[1] as u16) | ((bytes[2] as u16) << 8);
        let delay_ms = ((bytes[3] as u16) << 8) | (bytes[4] as u16);

        Some(Self {
            key_down,
            key_type,
            key_code,
            delay_ms,
        })
    }
}

/// Macro definition.
#[derive(Debug, Clone)]
pub struct Macro {
    /// Macro name (max 30 characters).
    pub name: String,
    /// Macro events (max 70 events).
    pub events: Vec<MacroEvent>,
    /// Cycle mode.
    pub cycle_mode: MacroCycleMode,
}

impl Default for Macro {
    fn default() -> Self {
        Self {
            name: String::new(),
            events: Vec::new(),
            cycle_mode: MacroCycleMode::Count(1),
        }
    }
}

impl Macro {
    /// Maximum name length.
    pub const MAX_NAME_LENGTH: usize = 30;
    /// Maximum events per macro.
    pub const MAX_EVENTS: usize = 70;
    /// Event size in bytes.
    pub const EVENT_SIZE: usize = 5;
    /// Total slot size in bytes.
    pub const SLOT_SIZE: usize = 384;

    /// Create a new empty macro.
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            events: Vec::new(),
            cycle_mode: MacroCycleMode::Count(1),
        }
    }

    /// Encode macro to wire format (384 bytes per slot).
    ///
    /// Per protocol spec section 6.14:
    /// - Offset 0: Name length (1-30)
    /// - Offset 1-30: Name (ASCII characters)
    /// - Offset 31: Event count (2-70)
    /// - Offset 32+: Events (5 bytes each)
    pub fn encode(&self) -> [u8; Self::SLOT_SIZE] {
        let mut data = [0u8; Self::SLOT_SIZE];

        // Encode name
        let name_bytes = self.name.as_bytes();
        let name_len = name_bytes.len().min(Self::MAX_NAME_LENGTH);
        data[0] = name_len as u8;
        data[1..1 + name_len].copy_from_slice(&name_bytes[..name_len]);

        // Encode event count
        let event_count = self.events.len().min(Self::MAX_EVENTS);
        data[31] = event_count as u8;

        // Encode events
        for (i, event) in self.events.iter().enumerate().take(Self::MAX_EVENTS) {
            let offset = 32 + i * Self::EVENT_SIZE;
            let encoded = event.encode();
            data[offset..offset + Self::EVENT_SIZE].copy_from_slice(&encoded);
        }

        data
    }

    /// Decode macro from wire format (384 bytes).
    pub fn decode(data: &[u8]) -> Option<Self> {
        if data.len() < 32 {
            return None;
        }

        // Decode name
        let name_len = (data[0] as usize).min(Self::MAX_NAME_LENGTH);
        let mut name_bytes = Vec::with_capacity(name_len);
        for &b in data[1..1 + name_len].iter() {
            if b == 0 {
                break;
            }
            name_bytes.push(b);
        }
        let name = String::from_utf8_lossy(&name_bytes).to_string();

        // Decode event count
        let event_count = (data[31] as usize).min(Self::MAX_EVENTS);

        // Decode events
        let mut events = Vec::with_capacity(event_count);
        for i in 0..event_count {
            let offset = 32 + i * Self::EVENT_SIZE;
            if offset + Self::EVENT_SIZE > data.len() {
                break;
            }
            let bytes = [
                data[offset],
                data[offset + 1],
                data[offset + 2],
                data[offset + 3],
                data[offset + 4],
            ];
            if let Some(event) = MacroEvent::decode(&bytes) {
                events.push(event);
            }
        }

        Some(Self {
            name,
            events,
            cycle_mode: MacroCycleMode::Count(1), // Default, cycle mode stored elsewhere
        })
    }
}

/// Modifier key codes (for shortcut key type 0).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum ModifierKey {
    /// Left Ctrl.
    LeftCtrl = 1,
    /// Left Shift.
    LeftShift = 2,
    /// Left Alt.
    LeftAlt = 4,
    /// Left Win.
    LeftWin = 8,
    /// Right Ctrl.
    RightCtrl = 16,
    /// Right Shift.
    RightShift = 32,
    /// Right Alt.
    RightAlt = 64,
    /// Right Win.
    RightWin = 128,
}

impl ModifierKey {
    /// Parse from wire byte value.
    pub fn from_byte(byte: u8) -> Option<Self> {
        match byte {
            1 => Some(ModifierKey::LeftCtrl),
            2 => Some(ModifierKey::LeftShift),
            4 => Some(ModifierKey::LeftAlt),
            8 => Some(ModifierKey::LeftWin),
            16 => Some(ModifierKey::RightCtrl),
            32 => Some(ModifierKey::RightShift),
            64 => Some(ModifierKey::RightAlt),
            128 => Some(ModifierKey::RightWin),
            _ => None,
        }
    }
}

impl fmt::Display for ModifierKey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ModifierKey::LeftCtrl => write!(f, "Left Ctrl"),
            ModifierKey::LeftShift => write!(f, "Left Shift"),
            ModifierKey::LeftAlt => write!(f, "Left Alt"),
            ModifierKey::LeftWin => write!(f, "Left Win"),
            ModifierKey::RightCtrl => write!(f, "Right Ctrl"),
            ModifierKey::RightShift => write!(f, "Right Shift"),
            ModifierKey::RightAlt => write!(f, "Right Alt"),
            ModifierKey::RightWin => write!(f, "Right Win"),
        }
    }
}

/// Media key codes (for shortcut key type 2).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u16)]
pub enum MediaKey {
    /// Media Player.
    MediaPlayer = 0x0183,
    /// Play/Pause.
    PlayPause = 0x00CD,
    /// Next Track.
    NextTrack = 0x00B5,
    /// Previous Track.
    PreviousTrack = 0x00B6,
    /// Stop.
    Stop = 0x00B7,
    /// Mute.
    Mute = 0x00E2,
    /// Volume Up.
    VolumeUp = 0x00E9,
    /// Volume Down.
    VolumeDown = 0x00EA,
    /// Email.
    Email = 0x018A,
    /// Calculator.
    Calculator = 0x0192,
    /// My Computer.
    MyComputer = 0x0194,
    /// Search.
    Search = 0x0221,
    /// Home Page.
    HomePage = 0x0223,
    /// Web Back.
    WebBack = 0x0224,
    /// Web Forward.
    WebForward = 0x0225,
    /// Web Stop.
    WebStop = 0x0226,
    /// Refresh.
    Refresh = 0x0227,
    /// Favorites.
    Favorites = 0x022A,
}

impl MediaKey {
    /// Parse from wire value.
    pub fn from_u16(value: u16) -> Option<Self> {
        match value {
            0x0183 => Some(MediaKey::MediaPlayer),
            0x00CD => Some(MediaKey::PlayPause),
            0x00B5 => Some(MediaKey::NextTrack),
            0x00B6 => Some(MediaKey::PreviousTrack),
            0x00B7 => Some(MediaKey::Stop),
            0x00E2 => Some(MediaKey::Mute),
            0x00E9 => Some(MediaKey::VolumeUp),
            0x00EA => Some(MediaKey::VolumeDown),
            0x018A => Some(MediaKey::Email),
            0x0192 => Some(MediaKey::Calculator),
            0x0194 => Some(MediaKey::MyComputer),
            0x0221 => Some(MediaKey::Search),
            0x0223 => Some(MediaKey::HomePage),
            0x0224 => Some(MediaKey::WebBack),
            0x0225 => Some(MediaKey::WebForward),
            0x0226 => Some(MediaKey::WebStop),
            0x0227 => Some(MediaKey::Refresh),
            0x022A => Some(MediaKey::Favorites),
            _ => None,
        }
    }
}

impl fmt::Display for MediaKey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            MediaKey::MediaPlayer => write!(f, "Media Player"),
            MediaKey::PlayPause => write!(f, "Play/Pause"),
            MediaKey::NextTrack => write!(f, "Next Track"),
            MediaKey::PreviousTrack => write!(f, "Previous Track"),
            MediaKey::Stop => write!(f, "Stop"),
            MediaKey::Mute => write!(f, "Mute"),
            MediaKey::VolumeUp => write!(f, "Volume Up"),
            MediaKey::VolumeDown => write!(f, "Volume Down"),
            MediaKey::Email => write!(f, "Email"),
            MediaKey::Calculator => write!(f, "Calculator"),
            MediaKey::MyComputer => write!(f, "My Computer"),
            MediaKey::Search => write!(f, "Search"),
            MediaKey::HomePage => write!(f, "Home Page"),
            MediaKey::WebBack => write!(f, "Web Back"),
            MediaKey::WebForward => write!(f, "Web Forward"),
            MediaKey::WebStop => write!(f, "Web Stop"),
            MediaKey::Refresh => write!(f, "Refresh"),
            MediaKey::Favorites => write!(f, "Favorites"),
        }
    }
}

/// Macro mouse button codes (for macro event key type 4).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum MacroMouseButton {
    /// Left button.
    Left = 0x01,
    /// Right button.
    Right = 0x02,
    /// Middle button.
    Middle = 0x04,
    /// Back button.
    Back = 0x08,
    /// Forward button.
    Forward = 0x10,
}

impl MacroMouseButton {
    /// Parse from wire byte value.
    pub fn from_byte(byte: u8) -> Option<Self> {
        match byte {
            0x01 => Some(MacroMouseButton::Left),
            0x02 => Some(MacroMouseButton::Right),
            0x04 => Some(MacroMouseButton::Middle),
            0x08 => Some(MacroMouseButton::Back),
            0x10 => Some(MacroMouseButton::Forward),
            _ => None,
        }
    }
}

impl fmt::Display for MacroMouseButton {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            MacroMouseButton::Left => write!(f, "Left"),
            MacroMouseButton::Right => write!(f, "Right"),
            MacroMouseButton::Middle => write!(f, "Middle"),
            MacroMouseButton::Back => write!(f, "Back"),
            MacroMouseButton::Forward => write!(f, "Forward"),
        }
    }
}

/// USB HID Keyboard scan codes.
///
/// These codes are used for keyboard shortcuts and macros.
/// Per USB HID Usage Tables specification.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum HidKeyCode {
    // Letters
    A = 4,
    B = 5,
    C = 6,
    D = 7,
    E = 8,
    F = 9,
    G = 10,
    H = 11,
    I = 12,
    J = 13,
    K = 14,
    L = 15,
    M = 16,
    N = 17,
    O = 18,
    P = 19,
    Q = 20,
    R = 21,
    S = 22,
    T = 23,
    U = 24,
    V = 25,
    W = 26,
    X = 27,
    Y = 28,
    Z = 29,

    // Numbers
    Num1 = 30,
    Num2 = 31,
    Num3 = 32,
    Num4 = 33,
    Num5 = 34,
    Num6 = 35,
    Num7 = 36,
    Num8 = 37,
    Num9 = 38,
    Num0 = 39,

    // Control keys
    Enter = 40,
    Escape = 41,
    Backspace = 42,
    Tab = 43,
    Space = 44,

    // Symbols
    Minus = 45,
    Equal = 46,
    LeftBracket = 47,
    RightBracket = 48,
    Backslash = 49,
    Semicolon = 51,
    Quote = 52,
    Backquote = 53,
    Comma = 54,
    Period = 55,
    Slash = 56,

    // Modifiers and function keys
    CapsLock = 57,
    F1 = 58,
    F2 = 59,
    F3 = 60,
    F4 = 61,
    F5 = 62,
    F6 = 63,
    F7 = 64,
    F8 = 65,
    F9 = 66,
    F10 = 67,
    F11 = 68,
    F12 = 69,

    // System keys
    PrintScreen = 70,
    ScrollLock = 71,
    Pause = 72,
    Insert = 73,
    Home = 74,
    PageUp = 75,
    Delete = 76,
    End = 77,
    PageDown = 78,

    // Arrow keys
    ArrowRight = 79,
    ArrowLeft = 80,
    ArrowDown = 81,
    ArrowUp = 82,

    // Numpad
    NumLock = 83,
    NumpadDivide = 84,
    NumpadMultiply = 85,
    NumpadSubtract = 86,
    NumpadAdd = 87,
    NumpadEnter = 88,
    Numpad1 = 89,
    Numpad2 = 90,
    Numpad3 = 91,
    Numpad4 = 92,
    Numpad5 = 93,
    Numpad6 = 94,
    Numpad7 = 95,
    Numpad8 = 96,
    Numpad9 = 97,
    Numpad0 = 98,
    NumpadDecimal = 99,
}

impl HidKeyCode {
    /// Get the scan code value.
    pub fn code(self) -> u8 {
        self as u8
    }

    /// Parse from scan code value.
    pub fn from_code(code: u8) -> Option<Self> {
        match code {
            4 => Some(HidKeyCode::A),
            5 => Some(HidKeyCode::B),
            6 => Some(HidKeyCode::C),
            7 => Some(HidKeyCode::D),
            8 => Some(HidKeyCode::E),
            9 => Some(HidKeyCode::F),
            10 => Some(HidKeyCode::G),
            11 => Some(HidKeyCode::H),
            12 => Some(HidKeyCode::I),
            13 => Some(HidKeyCode::J),
            14 => Some(HidKeyCode::K),
            15 => Some(HidKeyCode::L),
            16 => Some(HidKeyCode::M),
            17 => Some(HidKeyCode::N),
            18 => Some(HidKeyCode::O),
            19 => Some(HidKeyCode::P),
            20 => Some(HidKeyCode::Q),
            21 => Some(HidKeyCode::R),
            22 => Some(HidKeyCode::S),
            23 => Some(HidKeyCode::T),
            24 => Some(HidKeyCode::U),
            25 => Some(HidKeyCode::V),
            26 => Some(HidKeyCode::W),
            27 => Some(HidKeyCode::X),
            28 => Some(HidKeyCode::Y),
            29 => Some(HidKeyCode::Z),
            30 => Some(HidKeyCode::Num1),
            31 => Some(HidKeyCode::Num2),
            32 => Some(HidKeyCode::Num3),
            33 => Some(HidKeyCode::Num4),
            34 => Some(HidKeyCode::Num5),
            35 => Some(HidKeyCode::Num6),
            36 => Some(HidKeyCode::Num7),
            37 => Some(HidKeyCode::Num8),
            38 => Some(HidKeyCode::Num9),
            39 => Some(HidKeyCode::Num0),
            40 => Some(HidKeyCode::Enter),
            41 => Some(HidKeyCode::Escape),
            42 => Some(HidKeyCode::Backspace),
            43 => Some(HidKeyCode::Tab),
            44 => Some(HidKeyCode::Space),
            45 => Some(HidKeyCode::Minus),
            46 => Some(HidKeyCode::Equal),
            47 => Some(HidKeyCode::LeftBracket),
            48 => Some(HidKeyCode::RightBracket),
            49 => Some(HidKeyCode::Backslash),
            51 => Some(HidKeyCode::Semicolon),
            52 => Some(HidKeyCode::Quote),
            53 => Some(HidKeyCode::Backquote),
            54 => Some(HidKeyCode::Comma),
            55 => Some(HidKeyCode::Period),
            56 => Some(HidKeyCode::Slash),
            57 => Some(HidKeyCode::CapsLock),
            58 => Some(HidKeyCode::F1),
            59 => Some(HidKeyCode::F2),
            60 => Some(HidKeyCode::F3),
            61 => Some(HidKeyCode::F4),
            62 => Some(HidKeyCode::F5),
            63 => Some(HidKeyCode::F6),
            64 => Some(HidKeyCode::F7),
            65 => Some(HidKeyCode::F8),
            66 => Some(HidKeyCode::F9),
            67 => Some(HidKeyCode::F10),
            68 => Some(HidKeyCode::F11),
            69 => Some(HidKeyCode::F12),
            70 => Some(HidKeyCode::PrintScreen),
            71 => Some(HidKeyCode::ScrollLock),
            72 => Some(HidKeyCode::Pause),
            73 => Some(HidKeyCode::Insert),
            74 => Some(HidKeyCode::Home),
            75 => Some(HidKeyCode::PageUp),
            76 => Some(HidKeyCode::Delete),
            77 => Some(HidKeyCode::End),
            78 => Some(HidKeyCode::PageDown),
            79 => Some(HidKeyCode::ArrowRight),
            80 => Some(HidKeyCode::ArrowLeft),
            81 => Some(HidKeyCode::ArrowDown),
            82 => Some(HidKeyCode::ArrowUp),
            83 => Some(HidKeyCode::NumLock),
            84 => Some(HidKeyCode::NumpadDivide),
            85 => Some(HidKeyCode::NumpadMultiply),
            86 => Some(HidKeyCode::NumpadSubtract),
            87 => Some(HidKeyCode::NumpadAdd),
            88 => Some(HidKeyCode::NumpadEnter),
            89 => Some(HidKeyCode::Numpad1),
            90 => Some(HidKeyCode::Numpad2),
            91 => Some(HidKeyCode::Numpad3),
            92 => Some(HidKeyCode::Numpad4),
            93 => Some(HidKeyCode::Numpad5),
            94 => Some(HidKeyCode::Numpad6),
            95 => Some(HidKeyCode::Numpad7),
            96 => Some(HidKeyCode::Numpad8),
            97 => Some(HidKeyCode::Numpad9),
            98 => Some(HidKeyCode::Numpad0),
            99 => Some(HidKeyCode::NumpadDecimal),
            _ => None,
        }
    }
}

impl fmt::Display for HidKeyCode {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            HidKeyCode::A => write!(f, "A"),
            HidKeyCode::B => write!(f, "B"),
            HidKeyCode::C => write!(f, "C"),
            HidKeyCode::D => write!(f, "D"),
            HidKeyCode::E => write!(f, "E"),
            HidKeyCode::F => write!(f, "F"),
            HidKeyCode::G => write!(f, "G"),
            HidKeyCode::H => write!(f, "H"),
            HidKeyCode::I => write!(f, "I"),
            HidKeyCode::J => write!(f, "J"),
            HidKeyCode::K => write!(f, "K"),
            HidKeyCode::L => write!(f, "L"),
            HidKeyCode::M => write!(f, "M"),
            HidKeyCode::N => write!(f, "N"),
            HidKeyCode::O => write!(f, "O"),
            HidKeyCode::P => write!(f, "P"),
            HidKeyCode::Q => write!(f, "Q"),
            HidKeyCode::R => write!(f, "R"),
            HidKeyCode::S => write!(f, "S"),
            HidKeyCode::T => write!(f, "T"),
            HidKeyCode::U => write!(f, "U"),
            HidKeyCode::V => write!(f, "V"),
            HidKeyCode::W => write!(f, "W"),
            HidKeyCode::X => write!(f, "X"),
            HidKeyCode::Y => write!(f, "Y"),
            HidKeyCode::Z => write!(f, "Z"),
            HidKeyCode::Num1 => write!(f, "1"),
            HidKeyCode::Num2 => write!(f, "2"),
            HidKeyCode::Num3 => write!(f, "3"),
            HidKeyCode::Num4 => write!(f, "4"),
            HidKeyCode::Num5 => write!(f, "5"),
            HidKeyCode::Num6 => write!(f, "6"),
            HidKeyCode::Num7 => write!(f, "7"),
            HidKeyCode::Num8 => write!(f, "8"),
            HidKeyCode::Num9 => write!(f, "9"),
            HidKeyCode::Num0 => write!(f, "0"),
            HidKeyCode::Enter => write!(f, "Enter"),
            HidKeyCode::Escape => write!(f, "Escape"),
            HidKeyCode::Backspace => write!(f, "Backspace"),
            HidKeyCode::Tab => write!(f, "Tab"),
            HidKeyCode::Space => write!(f, "Space"),
            HidKeyCode::Minus => write!(f, "-"),
            HidKeyCode::Equal => write!(f, "="),
            HidKeyCode::LeftBracket => write!(f, "["),
            HidKeyCode::RightBracket => write!(f, "]"),
            HidKeyCode::Backslash => write!(f, "\\"),
            HidKeyCode::Semicolon => write!(f, ";"),
            HidKeyCode::Quote => write!(f, "'"),
            HidKeyCode::Backquote => write!(f, "`"),
            HidKeyCode::Comma => write!(f, ","),
            HidKeyCode::Period => write!(f, "."),
            HidKeyCode::Slash => write!(f, "/"),
            HidKeyCode::CapsLock => write!(f, "CapsLock"),
            HidKeyCode::F1 => write!(f, "F1"),
            HidKeyCode::F2 => write!(f, "F2"),
            HidKeyCode::F3 => write!(f, "F3"),
            HidKeyCode::F4 => write!(f, "F4"),
            HidKeyCode::F5 => write!(f, "F5"),
            HidKeyCode::F6 => write!(f, "F6"),
            HidKeyCode::F7 => write!(f, "F7"),
            HidKeyCode::F8 => write!(f, "F8"),
            HidKeyCode::F9 => write!(f, "F9"),
            HidKeyCode::F10 => write!(f, "F10"),
            HidKeyCode::F11 => write!(f, "F11"),
            HidKeyCode::F12 => write!(f, "F12"),
            HidKeyCode::PrintScreen => write!(f, "PrintScreen"),
            HidKeyCode::ScrollLock => write!(f, "ScrollLock"),
            HidKeyCode::Pause => write!(f, "Pause"),
            HidKeyCode::Insert => write!(f, "Insert"),
            HidKeyCode::Home => write!(f, "Home"),
            HidKeyCode::PageUp => write!(f, "PageUp"),
            HidKeyCode::Delete => write!(f, "Delete"),
            HidKeyCode::End => write!(f, "End"),
            HidKeyCode::PageDown => write!(f, "PageDown"),
            HidKeyCode::ArrowRight => write!(f, "Right"),
            HidKeyCode::ArrowLeft => write!(f, "Left"),
            HidKeyCode::ArrowDown => write!(f, "Down"),
            HidKeyCode::ArrowUp => write!(f, "Up"),
            HidKeyCode::NumLock => write!(f, "NumLock"),
            HidKeyCode::NumpadDivide => write!(f, "Num/"),
            HidKeyCode::NumpadMultiply => write!(f, "Num*"),
            HidKeyCode::NumpadSubtract => write!(f, "Num-"),
            HidKeyCode::NumpadAdd => write!(f, "Num+"),
            HidKeyCode::NumpadEnter => write!(f, "NumEnter"),
            HidKeyCode::Numpad1 => write!(f, "Num1"),
            HidKeyCode::Numpad2 => write!(f, "Num2"),
            HidKeyCode::Numpad3 => write!(f, "Num3"),
            HidKeyCode::Numpad4 => write!(f, "Num4"),
            HidKeyCode::Numpad5 => write!(f, "Num5"),
            HidKeyCode::Numpad6 => write!(f, "Num6"),
            HidKeyCode::Numpad7 => write!(f, "Num7"),
            HidKeyCode::Numpad8 => write!(f, "Num8"),
            HidKeyCode::Numpad9 => write!(f, "Num9"),
            HidKeyCode::Numpad0 => write!(f, "Num0"),
            HidKeyCode::NumpadDecimal => write!(f, "Num."),
        }
    }
}

/// Notifications received from the mouse.
///
/// These are either unsolicited messages from the device (like StatusChanged when
/// the user presses the DPI button) or synthetic notifications (like Disconnected
/// when the device is unplugged).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Notification {
    /// Settings changed on the device (e.g., DPI button pressed, profile switched).
    ///
    /// The flags indicate which settings changed. Use the accessor methods on
    /// `StatusChangeFlags` to check specific changes.
    StatusChanged(StatusChangeFlags),

    /// Device was disconnected.
    ///
    /// After receiving this notification, all subsequent commands will fail with
    /// `MouseError::Disconnected`. The `Mouse` handle should be dropped and a new
    /// connection established via `Mouse::open()`.
    Disconnected,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_macro_event_encode_decode() {
        let event = MacroEvent {
            key_down: true,
            key_type: MacroEventKeyType::Keyboard,
            key_code: 0x04, // 'A' key
            delay_ms: 100,
        };

        let encoded = event.encode();
        let decoded = MacroEvent::decode(&encoded).unwrap();

        assert_eq!(decoded.key_down, event.key_down);
        assert_eq!(decoded.key_type, event.key_type);
        assert_eq!(decoded.key_code, event.key_code);
        assert_eq!(decoded.delay_ms, event.delay_ms);
    }

    #[test]
    fn test_macro_event_key_up() {
        let event = MacroEvent {
            key_down: false,
            key_type: MacroEventKeyType::Mouse,
            key_code: 0x01, // Left button
            delay_ms: 50,
        };

        let encoded = event.encode();
        let decoded = MacroEvent::decode(&encoded).unwrap();

        assert!(!decoded.key_down);
        assert_eq!(decoded.key_type, MacroEventKeyType::Mouse);
        assert_eq!(decoded.key_code, 0x01);
        assert_eq!(decoded.delay_ms, 50);
    }

    #[test]
    fn test_macro_encode_decode() {
        let mut macro_def = Macro::new("Test Macro");
        macro_def.events.push(MacroEvent {
            key_down: true,
            key_type: MacroEventKeyType::Keyboard,
            key_code: 0x04,
            delay_ms: 10,
        });
        macro_def.events.push(MacroEvent {
            key_down: false,
            key_type: MacroEventKeyType::Keyboard,
            key_code: 0x04,
            delay_ms: 10,
        });

        let encoded = macro_def.encode();
        let decoded = Macro::decode(&encoded).unwrap();

        assert_eq!(decoded.name, "Test Macro");
        assert_eq!(decoded.events.len(), 2);
        assert_eq!(decoded.events[0].key_code, 0x04);
        assert!(decoded.events[0].key_down);
        assert!(!decoded.events[1].key_down);
    }

    #[test]
    fn test_macro_name_truncation() {
        let long_name = "This is a very long macro name that exceeds 30 characters";
        let macro_def = Macro::new(long_name);

        let encoded = macro_def.encode();
        let decoded = Macro::decode(&encoded).unwrap();

        assert!(decoded.name.len() <= Macro::MAX_NAME_LENGTH);
        assert_eq!(decoded.name, &long_name[..Macro::MAX_NAME_LENGTH]);
    }

    #[test]
    fn test_status_change_flags() {
        let flags = StatusChangeFlags(0x43); // DPI + Report Rate + Battery

        assert!(flags.dpi_changed());
        assert!(flags.report_rate_changed());
        assert!(!flags.profile_changed());
        assert!(!flags.dpi_settings_changed());
        assert!(!flags.light_settings_changed());
        assert!(flags.battery_changed());
    }

    #[test]
    fn test_macro_cycle_mode() {
        assert_eq!(MacroCycleMode::from_byte(1).to_byte(), 1);
        assert_eq!(MacroCycleMode::from_byte(250).to_byte(), 250);
        assert_eq!(MacroCycleMode::from_byte(253).to_byte(), 253);
        assert_eq!(MacroCycleMode::from_byte(254).to_byte(), 254);
        assert_eq!(MacroCycleMode::from_byte(255).to_byte(), 255);

        // Values below 1 should be clamped to 1
        assert_eq!(MacroCycleMode::from_byte(0).to_byte(), 1);
    }
}
