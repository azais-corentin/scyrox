//! Background IO task for HID communication.
//!
//! This module implements the blocking HID communication layer. The IO task runs
//! on a dedicated blocking thread via `tokio::task::spawn_blocking`, continuously
//! reading from the HID device and routing packets to either:
//! - Command callers (via oneshot channels) for command responses
//! - Notification subscribers (via broadcast) for unsolicited notifications
//!
//! The task polls the device with short read timeouts to concurrently handle:
//! - Commands from the Mouse handle (via `try_recv` on the mpsc channel)
//! - HID packets from the device (via `read_timeout`)
//!
//! This ensures notifications are received even when no command is in-flight.

use std::time::{Duration, Instant};

use hidapi::HidDevice;
use tokio::sync::{broadcast, mpsc, oneshot};
use tracing::{debug, error, info, trace, warn};

use crate::error::{MouseError, Result};
use crate::protocol::*;
use crate::types::{ConnectionMode, Notification};

/// Timeout for command responses.
const RESPONSE_TIMEOUT: Duration = Duration::from_millis(200);

/// Read poll interval in milliseconds.
///
/// This controls how often the blocking loop checks for new commands when the
/// device has no data. Lower values increase responsiveness at the cost of CPU.
/// 10ms provides sub-frame latency while keeping CPU usage negligible.
const READ_POLL_MS: i32 = 10;

/// A command to be sent to the IO task.
pub(crate) struct Command {
    /// The 16-byte command packet to send.
    pub packet: [u8; PACKET_LENGTH],
    /// Channel to send the response back.
    pub response_tx: oneshot::Sender<Result<Vec<u8>>>,
}

/// The background IO task that handles HID communication.
pub(crate) struct IoTask {
    device: HidDevice,
    mode: ConnectionMode,
    command_rx: mpsc::Receiver<Command>,
    notification_tx: broadcast::Sender<Notification>,
}

impl IoTask {
    /// Create a new IO task.
    pub fn new(
        device: HidDevice,
        mode: ConnectionMode,
        command_rx: mpsc::Receiver<Command>,
        notification_tx: broadcast::Sender<Notification>,
    ) -> Self {
        Self {
            device,
            mode,
            command_rx,
            notification_tx,
        }
    }

    /// Run the IO task (blocking).
    ///
    /// This method runs until the device disconnects, an unrecoverable error occurs,
    /// or the command channel is closed (Mouse handle dropped). It continuously reads
    /// from the HID device and processes incoming packets, even when no command is
    /// in-flight (to receive notifications).
    ///
    /// Must be called from `tokio::task::spawn_blocking`.
    pub fn run(mut self) {
        info!("IO task started");

        let packet_size = self.mode.packet_size();
        let mut pending_command: Option<Command> = None;
        let mut response_deadline: Option<Instant> = None;

        loop {
            // Accept new command if none pending
            if pending_command.is_none() {
                match self.command_rx.try_recv() {
                    Ok(cmd) => {
                        trace!(?cmd.packet, "received command to send");
                        match self.send_report(&cmd.packet) {
                            Ok(()) => {
                                response_deadline = Some(Instant::now() + RESPONSE_TIMEOUT);
                                pending_command = Some(cmd);
                            }
                            Err(e) => {
                                let _ = cmd.response_tx.send(Err(e));
                            }
                        }
                    }
                    Err(mpsc::error::TryRecvError::Empty) => {}
                    Err(mpsc::error::TryRecvError::Disconnected) => {
                        info!("command channel closed, shutting down IO task");
                        break;
                    }
                }
            }

            // Read from device (short timeout)
            let mut buf = vec![0u8; packet_size];
            match self.device.read_timeout(&mut buf, READ_POLL_MS) {
                Ok(0) => {
                    // Timeout, no data available — fall through to deadline check
                }
                Ok(n) => {
                    let packet = buf[..n].to_vec();

                    if is_status_changed_notification(&packet) {
                        // Unsolicited notification — broadcast to subscribers
                        if let Some(flags) = parse_status_changed_notification(&packet) {
                            debug!(?flags, "received status changed notification");
                            let _ = self
                                .notification_tx
                                .send(Notification::StatusChanged(flags));
                        }
                    } else if let Some(cmd) = pending_command.take() {
                        // Response to our pending command
                        trace!(?packet, "received command response");
                        let _ = cmd.response_tx.send(Ok(packet));
                        response_deadline = None;
                    } else {
                        // Unexpected packet while idle — log and ignore
                        warn!(?packet, "received unexpected packet while idle");
                    }
                }
                Err(e) => {
                    // hidapi read errors typically indicate disconnect or fatal device error
                    error!(?e, "HID read error");
                    let _ = self.notification_tx.send(Notification::Disconnected);
                    if let Some(cmd) = pending_command.take() {
                        let _ = cmd.response_tx.send(Err(MouseError::Disconnected));
                    }
                    info!("IO task shutting down due to device error");
                    return;
                }
            }

            // Check response timeout
            if let Some(deadline) = response_deadline
                && Instant::now() >= deadline
                && let Some(cmd) = pending_command.take()
            {
                let error = match self.mode {
                    ConnectionMode::Wireless => MouseError::DeviceOffline,
                    ConnectionMode::Wired => MouseError::Timeout,
                };
                warn!("command response timeout");
                let _ = cmd.response_tx.send(Err(error));
                response_deadline = None;
            }
        }

        info!("IO task shutting down");
    }

    /// Send a command packet to the device via HID output report.
    ///
    /// This only sends the command; the caller is responsible for handling
    /// the response which will arrive as a subsequent read.
    fn send_report(&self, cmd: &[u8; PACKET_LENGTH]) -> Result<()> {
        // Prepend report ID to command packet for HID transfer
        let mut report = [0u8; PACKET_LENGTH + 1];
        report[0] = REPORT_ID;
        report[1..].copy_from_slice(cmd);

        self.device.write(&report).map_err(MouseError::from)?;
        Ok(())
    }
}
