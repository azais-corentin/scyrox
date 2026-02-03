//! Path utilities for scyrox components.

use std::path::PathBuf;

use directories::ProjectDirs;

/// The socket filename used for daemon communication.
pub const SOCKET_NAME: &str = "scyrox.sock";

/// Get the path to the Unix socket for daemon communication.
///
/// Uses `$XDG_RUNTIME_DIR/scyrox/scyrox.sock` if available,
/// otherwise falls back to the project state directory.
pub fn get_socket_path() -> Option<PathBuf> {
    if let Ok(runtime_dir) = std::env::var("XDG_RUNTIME_DIR") {
        return Some(PathBuf::from(runtime_dir).join("scyrox").join(SOCKET_NAME));
    }

    let dirs = ProjectDirs::from("", "", "scyrox")?;
    Some(dirs.state_dir()?.join(SOCKET_NAME))
}
