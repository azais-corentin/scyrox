//! Scyrox mouse configuration library.
//!
//! This library provides an API to read and write configuration settings
//! for Scyrox gaming mice over USB.
//!
//! # Example
//!
//! ```no_run
//! use scyroxd::{Mouse, PollingRate};
//!
//! fn main() -> scyroxd::Result<()> {
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
pub use mouse::Mouse;
pub use types::{
    BatteryStatus, ConnectionMode, FirmwareInfo, LiftOffDistance, MouseConfig, PollingRate,
};
