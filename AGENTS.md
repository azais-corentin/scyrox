# AGENTS.md - Coding Agent Guidelines for scyroxd

## Project Overview

Scyroxd is a Rust workspace for configuring Scyrox gaming mice over USB/HID. It consists of:

- `scyrox`: Core library for USB communication and mouse configuration
- `scyrox-proto`: Protobuf definitions for gRPC IPC between CLI and daemon
- `scyroxd`: Background daemon exposing gRPC service over Unix socket
- `scyroxctl`: CLI tool for interacting with the daemon or direct USB access

## Build Commands

```bash
cargo build                    # Build entire workspace
cargo build --release          # Build in release mode
cargo build -p scyrox          # Build specific crate
```

## Test Commands

```bash
cargo test                              # Run all tests
cargo test -p scyrox                    # Run tests for a specific crate
cargo test test_checksum_calculation    # Run a single test by name
cargo test -p scyrox test_encode_decode_dpi  # Single test in specific crate
cargo test -- --nocapture               # Run tests with output shown
cargo test dpi                          # Run tests matching a pattern
cargo test -- --list                    # List available tests
```

**Note:** Tests run sequentially (`RUST_TEST_THREADS=1` in `.cargo/config.toml`) to avoid USB conflicts.

## Lint & Format Commands

```bash
cargo check                    # Check code (no build)
cargo fmt                      # Format code
cargo fmt -- --check           # Check formatting without applying
cargo clippy                   # Run clippy lints
cargo clippy -- -D warnings    # Fail on warnings
```

## Code Style Guidelines

### Imports

Order imports in groups separated by blank lines:
1. Standard library (`std::`)
2. External crates (alphabetically)
3. Internal crates (`crate::`, `super::`)

### Naming Conventions

- **Types/Structs/Enums**: `PascalCase` - `MouseConfig`, `PollingRate`
- **Functions/Methods**: `snake_case` - `get_polling_rate`, `set_config`
- **Constants**: `SCREAMING_SNAKE_CASE` - `VENDOR_ID`, `PACKET_LENGTH`
- **Variables**: `snake_case` - `device_info`, `byte_count`
- **Modules**: `snake_case` - `protocol`, `types`

### Documentation

```rust
//! Module-level documentation using //!

/// Function/struct documentation using ///
pub fn example() {}
```

### Error Handling

Use `thiserror` for custom error types:

```rust
#[derive(Error, Debug)]
pub enum MouseError {
    #[error("Mouse not found (VID: 0x{vid:04x}, PIDs: {pids:?})")]
    NotFound { vid: u16, pids: Vec<u16> },

    #[error("USB error: {0}")]
    Usb(#[from] nusb::Error),
}

pub type Result<T> = std::result::Result<T, MouseError>;
```

### Logging

Use `tracing` for structured logging with appropriate levels:

```rust
use tracing::{debug, error, info, trace, warn, instrument};

#[instrument(skip(self))]
pub fn get_config(&mut self) -> Result<MouseConfig> {
    info!("retrieving mouse configuration");
    debug!(offset = format!("0x{:04X}", offset), "reading memory");
    trace!(?data, "raw response received");
    warn!(byte, "unexpected value, using default");
    error!(expected = CMD_BATTERY, got = response[1], "command mismatch");
    Ok(config)
}
```

### Type Patterns

Enums with wire format conversion use `from_byte()` -> `Option<Self>` and `impl fmt::Display`.

### Async Code

Use `tokio` for async runtime. Mark async functions appropriately:

```rust
#[tokio::main]
async fn main() -> Result<()> { /* ... */ }
```

### CLI Structure (clap)

Use derive macros for CLI argument parsing with `Parser`, `Subcommand`, and `ValueEnum`.

### Test Structure

Write tests as inline module at the end of the file:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_encode_decode_roundtrip() {
        let original = 800u16;
        let encoded = encode_dpi(original);
        let decoded = decode_dpi(&encoded);
        assert_eq!(decoded, original, "DPI {} round-trip failed", original);
    }
}
```

### gRPC/Protobuf

Proto files are in `crates/scyrox-proto/proto/`. Build with `tonic-prost-build`:

```rust
// build.rs
fn main() -> Result<(), Box<dyn std::error::Error>> {
    tonic_prost_build::configure()
        .build_server(true)
        .build_client(true)
        .compile_protos(&["./proto/scyrox.proto"], &["./proto"])?;
    Ok(())
}
```

## Project Structure

```
scyroxd/
  Cargo.toml              # Workspace root
  .cargo/config.toml      # Cargo configuration
  crates/
    scyrox/               # Core USB/HID library
      src/
        lib.rs            # Public API re-exports
        error.rs          # Error types
        mouse.rs          # Mouse struct and methods
        protocol.rs       # USB protocol constants and packets
        types.rs          # Data types and enums
    scyrox-proto/         # Protobuf definitions
      proto/scyrox.proto  # gRPC service definition
      build.rs            # Proto compilation
      src/lib.rs          # Re-exports generated code
    scyroxd/              # Daemon binary
      src/
        main.rs           # Entry point, socket setup
        server.rs         # gRPC service implementation
        config.rs         # Daemon configuration
        profiles.rs       # Profile management
    scyroxctl/            # CLI binary
      src/
        main.rs           # Entry point
        cli.rs            # Argument definitions
        client.rs         # gRPC client wrapper
        commands/         # Command implementations
```

## Key Dependencies

- `nusb`: USB device access
- `tokio`: Async runtime
- `tonic`/`prost`: gRPC and protobuf
- `clap`: CLI argument parsing
- `tracing`: Structured logging
- `thiserror`/`anyhow`: Error handling
- `serde`/`toml`: Configuration serialization
