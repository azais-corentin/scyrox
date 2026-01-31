//! Scyrox mouse configuration library.
//!
//! This library provides an API to read and write configuration settings
//! for Scyrox gaming mice over USB.
//!
//! # Example
//!
//! ```no_run
//! use scyrox::{Mouse, PollingRate};
//!
//! fn main() -> scyrox::Result<()> {
//!     let mut mouse = Mouse::open()?;
//!
//!     // Read current configuration
//!     let config = mouse.get_config()?;
//!     println!("Polling rate: {}", config.polling_rate);
//!
//!     // Change polling rate
//!     mouse.set_polling_rate(PollingRate::Hz1000)?;
//!
//!     Ok(())
//! }
//! ```

pub mod error;
pub mod mouse;
pub mod protocol;
pub mod types;

// Re-export main types at crate root for convenience
pub use error::{MouseError, Result};
pub use mouse::{KEY_FUNCTION_COUNT, MACRO_COUNT, Mouse, SHORTCUT_KEY_COUNT};
pub use protocol::{
    // USB device identifiers (for hotplug detection)
    PID_WIRED,
    PID_WIRELESS_4K,
    PID_WIRELESS_STD,
    PRODUCT_IDS,
    VENDOR_ID,
    is_status_changed_notification,
    parse_status_changed_notification,
    validate_response,
    verify_response_checksum,
};
pub use types::{
    BatteryStatus, ConnectionMode, ConnectionType, DeviceInfo, DpiEffectMode, DpiEffectSettings,
    DpiStage, DpiSwitchMode, FireKeyConfig, FirmwareInfo, HidKeyCode, KeyFunction, KeyFunctionType,
    LiftOffDistance, LightMode, LightSettings, Macro, MacroCycleMode, MacroEvent,
    MacroEventKeyType, MacroKeyConfig, MacroMouseButton, MediaKey, ModifierKey, MouseButton,
    MouseConfig, PairStatus, PollingRate, ScrollWheelDirection, SensorMode, ShortcutKey,
    ShortcutKeyEvent, ShortcutKeyType, SleepTime, StatusChangeFlags,
};
