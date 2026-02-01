//! Scyrox mouse configuration daemon.
//!
//! This daemon provides a gRPC service over IPC (Unix socket) for managing
//! Scyrox mouse configuration. It handles:
//!
//! - Mouse configuration read/write
//! - Profile management
//! - Event streaming (battery, connection status)
//! - Auto-apply profiles on device connection

mod config;
mod hotplug;
mod profiles;
mod server;

use std::io;
use std::path::PathBuf;
use std::pin::Pin;
use std::task::{Context, Poll};

use anyhow::Result;
use directories::ProjectDirs;
use interprocess::local_socket::tokio::prelude::*;
use interprocess::local_socket::{GenericFilePath, ListenerOptions};
use pin_project_lite::pin_project;
use tokio::fs;
use tokio::io::{AsyncRead, AsyncWrite, ReadBuf};
use tonic::transport::Server;
use tonic::transport::server::Connected;
use tracing::{error, info};
use tracing_subscriber::EnvFilter;

use scyrox_proto::ScyroxServer;

use crate::config::DaemonConfig;
use crate::hotplug::HotplugMonitor;
use crate::server::ScyroxService;

/// Default socket name within the runtime directory.
const SOCKET_NAME: &str = "scyroxd.sock";

pin_project! {
    /// Wrapper for interprocess stream that implements tonic's Connected trait.
    struct IpcStream {
        #[pin]
        inner: LocalSocketStream,
    }
}

impl IpcStream {
    fn new(stream: LocalSocketStream) -> Self {
        Self { inner: stream }
    }
}

/// Connection info for IPC streams (empty - no metadata available).
#[derive(Clone, Debug)]
pub struct IpcConnectInfo;

impl Connected for IpcStream {
    type ConnectInfo = IpcConnectInfo;

    fn connect_info(&self) -> Self::ConnectInfo {
        IpcConnectInfo
    }
}

impl AsyncRead for IpcStream {
    fn poll_read(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &mut ReadBuf<'_>,
    ) -> Poll<io::Result<()>> {
        self.project().inner.poll_read(cx, buf)
    }
}

impl AsyncWrite for IpcStream {
    fn poll_write(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &[u8],
    ) -> Poll<io::Result<usize>> {
        self.project().inner.poll_write(cx, buf)
    }

    fn poll_flush(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<io::Result<()>> {
        self.project().inner.poll_flush(cx)
    }

    fn poll_shutdown(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<io::Result<()>> {
        self.project().inner.poll_shutdown(cx)
    }
}

#[tokio::main]
async fn main() -> Result<()> {
    // Initialize logging
    tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::from_default_env().add_directive("scyroxd=info".parse()?))
        .init();

    info!("Starting scyroxd v{}", env!("CARGO_PKG_VERSION"));

    // Load configuration
    let dirs = ProjectDirs::from("", "", "scyrox")
        .ok_or_else(|| anyhow::anyhow!("Failed to determine project directories"))?;

    let config = DaemonConfig::load(&dirs).await?;
    info!(?config, "Loaded configuration");

    // Ensure runtime directory exists
    let socket_path = get_socket_path(&dirs)?;
    if let Some(parent) = socket_path.parent() {
        fs::create_dir_all(parent).await?;
    }

    // Remove stale socket if it exists
    if socket_path.exists() {
        fs::remove_file(&socket_path).await?;
    }

    // Start the hotplug monitor
    let (_hotplug_monitor, device_event_rx) = HotplugMonitor::start()?;
    info!("Hotplug monitor started");

    // Create the gRPC service
    let (service, mut shutdown_rx, device_event_rx) =
        ScyroxService::new(config, dirs, device_event_rx).await?;

    // Spawn the device event handler task
    let event_handler = service.create_device_event_handler(device_event_rx);
    tokio::spawn(event_handler);

    // Bind to Unix socket
    info!(?socket_path, "Binding to socket");
    let listener = ListenerOptions::new()
        .name(
            socket_path
                .as_path()
                .as_os_str()
                .to_fs_name::<GenericFilePath>()?,
        )
        .create_tokio()?;

    // Create incoming stream for tonic
    let incoming = async_stream::stream! {
        loop {
            match listener.accept().await {
                Ok(conn) => yield Ok::<_, std::io::Error>(IpcStream::new(conn)),
                Err(e) => {
                    error!("Failed to accept connection: {}", e);
                }
            }
        }
    };

    info!("Daemon ready, accepting connections");

    // Run the gRPC server with graceful shutdown
    let server = Server::builder()
        .add_service(ScyroxServer::new(service))
        .serve_with_incoming_shutdown(incoming, async move {
            // Wait for shutdown signal from RPC or Ctrl+C
            tokio::select! {
                _ = shutdown_rx.changed() => {
                    if *shutdown_rx.borrow() {
                        info!("Shutdown signal received");
                    }
                }
                _ = tokio::signal::ctrl_c() => {
                    info!("Ctrl+C received, shutting down");
                }
            }
        });

    server.await?;

    // Cleanup: remove socket file
    if socket_path.exists() {
        let _ = fs::remove_file(&socket_path).await;
    }

    info!("Daemon stopped");
    Ok(())
}

/// Get the socket path, using XDG_RUNTIME_DIR if available.
fn get_socket_path(dirs: &ProjectDirs) -> Result<PathBuf> {
    // Try XDG_RUNTIME_DIR first (e.g., /run/user/1000)
    if let Ok(runtime_dir) = std::env::var("XDG_RUNTIME_DIR") {
        let path = PathBuf::from(runtime_dir).join("scyrox").join(SOCKET_NAME);
        return Ok(path);
    }

    // Fall back to state directory
    let state_dir = dirs.state_dir().unwrap_or_else(|| dirs.data_local_dir());
    Ok(state_dir.join(SOCKET_NAME))
}
