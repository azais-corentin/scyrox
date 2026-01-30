//! Custom error types for mouse communication.

use thiserror::Error;

/// Errors that can occur during mouse communication.
#[derive(Error, Debug)]
pub enum MouseError {
    /// Mouse device not found on USB bus.
    #[error("Mouse not found (VID: 0x{vid:04x}, PIDs: {pids:?})")]
    NotFound { vid: u16, pids: Vec<u16> },

    /// USB communication error.
    #[error("USB error: {0}")]
    Usb(#[from] nusb::Error),

    /// USB transfer error.
    #[error("USB transfer error: {0}")]
    Transfer(#[from] nusb::transfer::TransferError),

    /// Invalid polling rate byte value.
    #[error("Invalid polling rate value: 0x{0:02x}")]
    InvalidPollingRate(u8),

    /// Invalid lift-off distance byte value.
    #[error("Invalid lift-off distance value: 0x{0:02x}")]
    InvalidLiftOffDistance(u8),

    /// Sleep timeout value out of range.
    #[error("Invalid sleep timeout: {0} seconds (max 2550)")]
    InvalidSleepTimeout(u16),

    /// Invalid DPI stage index (must be 0-7).
    #[error("Invalid DPI stage: {0} (must be 0-7)")]
    InvalidDpiStage(u8),

    /// Invalid DPI value (must be 50-26000 in steps of 50).
    #[error("Invalid DPI value: {0} (must be 50-26000)")]
    InvalidDpiValue(u16),

    /// Invalid debounce time (must be 0-30 ms).
    #[error("Invalid debounce time: {0} ms (must be 0-30)")]
    InvalidDebounceTime(u8),

    /// Invalid profile index (must be 0-3).
    #[error("Invalid profile: {0} (must be 0-3)")]
    InvalidProfile(u8),

    /// Timeout waiting for device response.
    #[error("Communication timeout")]
    Timeout,

    /// Device returned unexpected response.
    #[error("Unexpected response: expected cmd 0x{expected:02x}, got 0x{got:02x}")]
    UnexpectedResponse { expected: u8, got: u8 },

    /// Response contains insufficient data.
    #[error("Insufficient data: need {need} bytes, got {got}")]
    InsufficientData { need: usize, got: usize },

    /// Command not supported by device.
    #[error("Command not supported")]
    NotSupported,

    /// Device is offline (wireless mouse not connected to dongle).
    #[error("Device offline")]
    DeviceOffline,
}

/// Result type alias using MouseError.
pub type Result<T> = std::result::Result<T, MouseError>;
