# Repository Guidelines

## Project Overview

Scyrox is a Rust workspace for configuring Scyrox gaming mice over USB HID. It provides a core library, a background daemon with gRPC IPC, and a CLI tool.

## Architecture & Data Flow

```
scyroxctl (CLI)          scyroxd (daemon)
    |                        |
    |--- gRPC over ----->  tonic server
    |    Unix socket         |
    |                    ScyroxService
    |                   Arc<Mutex<Option<Mouse>>>
    |                        |
    +--- direct HID --->  scyrox (core lib)
         fallback            |
                          Mouse::open()
                             |
                          IoTask (spawn_blocking)
                             |
                          hidapi::HidDevice
                          write() / read_timeout()
```

**Core communication model**: `Mouse` sends `Command` structs (16-byte packet + oneshot response channel) over an mpsc channel to `IoTask`, which runs on a dedicated blocking thread. The IO task writes HID reports and polls for responses with 10ms timeouts. Unsolicited notifications (status changes, disconnect) are broadcast to subscribers.

**Daemon model**: `HotplugMonitor` polls `hidapi::refresh_devices()` every 1s, emitting `DeviceEvent`s. The daemon holds the `Mouse` behind `Arc<Mutex<Option<Mouse>>>` (None when disconnected). A `with_mouse!` macro in `server.rs` ensures the device is connected before executing RPC bodies.

**CLI model**: Tries daemon gRPC first, falls back to direct HID access (`-d` flag forces direct). Backend trait abstracts both modes.

## Key Directories

```
crates/
  scyrox/           Core library: HID communication, protocol, types
    src/
      mouse.rs      Public API (Mouse struct, all getters/setters) [~1700 lines]
      io.rs         Background IO task (blocking hidapi loop)
      protocol.rs   Wire format: constants, packet builders, validation [~1000 lines]
      types.rs      Domain types: enums, configs, notifications [~1900 lines]
      error.rs      MouseError enum (thiserror)
      paths.rs      XDG socket path resolution
      lib.rs        Module declarations and public re-exports
  scyrox-proto/     Protobuf definitions and gRPC codegen
    proto/
      scyrox.proto  Single service, 25+ RPCs
    src/
      lib.rs        Re-exports generated types
      convert.rs    Proto <-> domain type conversions
    build.rs        tonic-prost-build compilation
  scyroxd/          Background daemon
    src/
      main.rs       Entry point, Unix socket listener, graceful shutdown
      server.rs     ScyroxService: all gRPC RPC implementations [~930 lines]
      hotplug.rs    Polling-based device connect/disconnect detection
      profiles.rs   TOML profile persistence (XDG config dir)
      config.rs     Daemon configuration (TOML)
  scyroxctl/        CLI tool
    src/
      main.rs       Entry point, tracing setup, backend selection
      cli.rs        Clap derive structs (Commands, GetWhat, SetWhat, etc.)
      backend.rs    Backend async_trait (unified daemon/direct interface)
      client.rs     DaemonClient (gRPC over Unix socket)
      direct.rs     DirectBackend (direct HID access)
      output.rs     Text/JSON output formatting
      commands/     Per-subcommand handler modules
    tests/          Integration tests (hardware-gated)
  scyrox-tray/      Cross-platform system tray battery indicator
    src/
      main.rs       tao event loop, tray creation, UserEvent dispatch
      state.rs      TrayState enum + text formatting helpers
      icon.rs       Font discovery + RGBA icon rendering
      daemon.rs     Daemon spawn/connect and WatchEvents worker
      notifications.rs  Low-battery desktop notification
docs/               Protocol spec, battery estimation, firmware notes
```

## Build Commands

```bash
cargo build                    # Build entire workspace
cargo build --release          # Release build
cargo build -p scyrox          # Build specific crate
cargo check                    # Type-check without codegen
```

## Test Commands

```bash
cargo test                     # All tests (integration tests need hardware)
cargo test -p scyrox           # Core library unit tests only (no hardware)
cargo test -p scyroxd          # Daemon unit tests only (no hardware)
cargo test -p scyrox -- dpi    # Tests matching a pattern
cargo test test_checksum_calculation  # Single test by name
```

Tests run sequentially (`RUST_TEST_THREADS=1` in `.cargo/config.toml`) to avoid HID device conflicts. Integration tests in `scyroxctl/tests/` require a physically connected, awake Scyrox mouse. They are gated behind `assert_device_connected()` and will panic if no device is present.

## Lint & Format

```bash
dprint fmt                     # Format entire repo (Rust via rustfmt, plus proto/toml/yaml/json/md/nix)
cargo clippy -- -D warnings    # Lint with warnings-as-errors
```

**Always use `dprint fmt`**, not `cargo fmt`. dprint delegates `.rs` files to `rustfmt` (style_edition = 2024) and handles all other file types.

## Code Conventions & Patterns

### Imports

Three groups separated by blank lines:

```rust
use std::time::Duration; // 1. Standard library

use hidapi::HidDevice; // 2. External crates (alphabetical)
use tokio::sync::{broadcast, mpsc};
use tracing::{debug, info, warn};

use crate::error::Result; // 3. Internal (crate::, super::)
use crate::protocol::*;
```

### Naming

| Kind                | Convention             | Examples                                  |
| ------------------- | ---------------------- | ----------------------------------------- |
| Types/Structs/Enums | `PascalCase`           | `MouseConfig`, `PollingRate`, `DpiStage`  |
| Functions/Methods   | `snake_case`           | `get_polling_rate`, `set_config`          |
| Constants           | `SCREAMING_SNAKE_CASE` | `VENDOR_ID`, `PACKET_LENGTH`, `PID_WIRED` |
| Modules             | `snake_case`           | `protocol`, `types`, `hotplug`            |

### Error Handling

- **Core library**: `thiserror` derive on `MouseError` enum with human-readable `#[error("...")]` messages. `Result<T>` alias in `error.rs`.
- **Daemon/CLI**: `anyhow::Result` for application-level errors. `MouseError` mapped to `tonic::Status` codes in `server.rs`.

```rust
#[derive(Error, Debug)]
pub enum MouseError {
    #[error("HID error: {0}")]
    Hid(#[from] hidapi::HidError),
    #[error("Mouse not found")]
    NotFound { vid: u16, pids: Vec<u16> },
    // ...
}
```

### Logging

Use `tracing` with structured fields. All public async methods on `Mouse` use `#[instrument(skip(self))]`.

| Level   | Usage                                                     |
| ------- | --------------------------------------------------------- |
| `trace` | Raw packets, channel operations                           |
| `debug` | State reads, device enumeration details                   |
| `info`  | Mutations, lifecycle events (connect/disconnect/shutdown) |
| `warn`  | Unexpected packets, fallback behavior, timeouts           |
| `error` | HID errors, device not found, RPC failures                |

```rust
#[instrument(skip(self))]
pub async fn get_polling_rate(&self) -> Result<PollingRate> {
    debug!("getting polling rate");
    // ...
    debug!(?rate, "polling rate retrieved");
    Ok(rate)
}
```

### Enum Patterns

Wire-format enums use `num_enum` for `u8 <-> enum` conversion and `strum` for `Display`:

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq, TryFromPrimitive, IntoPrimitive, Display)]
#[repr(u8)]
pub enum PollingRate {
    #[strum(to_string = "125 Hz")]
    Hz125 = 0x08,
    // ...
}
```

Enums with non-trivial byte mapping use `from_byte() -> Option<Self>` instead of `TryFrom`.

### Macro Pattern

`impl_bool_setting!` generates matched getter/setter pairs for boolean settings stored as single bytes in mouse memory:

```rust
impl_bool_setting!(
    get_angle_snapping,
    set_angle_snapping,
    OFFSET_ANGLE_SNAPPING,
    "angle snapping"
);
```

### Async / Threading

- **Tokio** is the async runtime (`features = ["full"]`).
- **IO task** runs on `tokio::task::spawn_blocking` (not `tokio::spawn`) because hidapi is synchronous. The blocking loop uses `try_recv()` for commands and `read_timeout(10ms)` for device data.
- **Channels**: `mpsc` for commands, `broadcast` for notifications and events, `oneshot` for per-command responses, `watch` for shutdown signaling.
- **Mutex**: `tokio::sync::Mutex` guards `Option<Mouse>` in the daemon (held across `.await` points).

### gRPC / Protobuf

- Single proto file: `crates/scyrox-proto/proto/scyrox.proto`
- Build: `tonic-prost-build` in `build.rs`
- IPC: Unix domain socket via `interprocess` crate, path from `scyrox::paths::get_socket_path()`
- Server: tonic with `with_mouse!` macro for device access
- Client: tonic `ScyroxClient` wrapped in `DaemonClient`

### CLI Structure (clap)

Derive macros with nested `Subcommand` enums:

```rust
#[derive(Parser)]
struct Cli {
    #[arg(short, long)]
    direct: bool,
    #[arg(short, long, default_value = "text")]
    format: OutputFormat,
    #[command(subcommand)]
    command: Commands,
}
```

## Testing Patterns

### Unit Tests

Inline `#[cfg(test)]` modules at end of file:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_encode_decode_roundtrip() {
        let original = 800u16;
        let encoded = encode_dpi(original);
        let decoded = decode_dpi(&encoded);
        assert_eq!(decoded, original);
    }
}
```

Unit tests cover: protocol math (checksums, packet construction), type conversions (encode/decode round-trips), and default state.

### Integration Tests

Located in `crates/scyroxctl/tests/`. Use `assert_cmd` to invoke the CLI binary. All hardware-dependent tests call `assert_device_connected()` first. Set-command tests use `ConfigGuard` (RAII pattern) to restore device state after each test.

```rust
#[test]
fn test_set_polling_rate_125() {
    assert_device_connected();
    let _guard = ConfigGuard::new();
    scyroxctl()
        .args(["set", "polling-rate", "125"])
        .assert()
        .success();
    let config = get_config_json();
    assert_eq!(config["polling_rate"], 125);
}
```

### What Requires Hardware

All `scyroxctl/tests/` integration tests except some `cli_parsing.rs` tests (--help, --version, invalid args). Core library (`scyrox`) and daemon (`scyroxd`) unit tests do **not** require hardware.

## Key Dependencies

| Crate                  | Role                                     |
| ---------------------- | ---------------------------------------- |
| `hidapi`               | HID device enumeration and communication |
| `tokio`                | Async runtime, channels, spawn_blocking  |
| `tonic` / `prost`      | gRPC server/client and protobuf codegen  |
| `interprocess`         | Unix domain socket IPC                   |
| `clap`                 | CLI argument parsing (derive)            |
| `tracing`              | Structured logging                       |
| `thiserror` / `anyhow` | Error handling (library / application)   |
| `serde` / `toml`       | Config and profile serialization         |
| `num_enum` / `strum`   | Enum <-> byte/string conversions         |
| `directories`          | XDG path resolution                      |

## Environment

- **Edition**: 2024 (resolver = "3")
- **Toolchain**: Rust stable (managed via Nix flake with rust-overlay)
- **Dev shell**: Nix flake + direnv (`use flake`); provides pkg-config, libusb1, protobuf, dprint, nixfmt
- **Packages**: `nix build .#scyroxctl` / `.#scyroxd` (crane-based, tests disabled in sandbox)
- **No CI**: No `.github/workflows` present; hardware-dependent nature makes CI non-trivial
