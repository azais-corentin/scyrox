//! Background IO task for USB communication.
//!
//! This module implements the async USB communication layer. The IO task runs
//! in the background, continuously reading from the interrupt endpoint and
//! routing packets to either:
//! - Command callers (via oneshot channels) for command responses
//! - Notification subscribers (via broadcast) for unsolicited notifications
//!
//! The task uses `tokio::select!` to concurrently wait for:
//! - Commands from the Mouse handle
//! - USB packets from the device (responses and notifications)
//!
//! This ensures notifications are received even when no command is in-flight.

use std::pin::Pin;
use std::time::Duration;

use nusb::transfer::{Buffer, ControlOut, ControlType, In, Interrupt, Recipient, TransferError};
use nusb::{Endpoint, Interface};
use tokio::sync::{broadcast, mpsc, oneshot};
use tracing::{debug, error, info, trace, warn};

use crate::error::{MouseError, Result};
use crate::protocol::*;
use crate::types::{ConnectionMode, Notification};

/// Timeout for command responses.
const RESPONSE_TIMEOUT: Duration = Duration::from_millis(200);

/// A command to be sent to the IO task.
pub(crate) struct Command {
    /// The 16-byte command packet to send.
    pub packet: [u8; PACKET_LENGTH],
    /// Channel to send the response back.
    pub response_tx: oneshot::Sender<Result<Vec<u8>>>,
}

/// The background IO task that handles USB communication.
pub(crate) struct IoTask {
    interface: Interface,
    endpoint: Endpoint<Interrupt, In>,
    mode: ConnectionMode,
    command_rx: mpsc::Receiver<Command>,
    notification_tx: broadcast::Sender<Notification>,
}

impl IoTask {
    /// Create a new IO task.
    pub fn new(
        interface: Interface,
        endpoint: Endpoint<Interrupt, In>,
        mode: ConnectionMode,
        command_rx: mpsc::Receiver<Command>,
        notification_tx: broadcast::Sender<Notification>,
    ) -> Self {
        Self {
            interface,
            endpoint,
            mode,
            command_rx,
            notification_tx,
        }
    }

    /// Run the IO task.
    ///
    /// This method runs until the device disconnects or an unrecoverable error occurs.
    /// It continuously reads from the interrupt endpoint and processes incoming packets,
    /// even when no command is in-flight (to receive notifications).
    pub async fn run(mut self) {
        info!("IO task started");

        // State: the command we're currently waiting for a response to
        let mut pending_command: Option<Command> = None;
        // State: sleep future for response timeout (boxed and pinned for use in select!)
        let mut response_timeout: Option<Pin<Box<tokio::time::Sleep>>> = None;

        // Always maintain a pending read buffer to receive notifications
        self.submit_read_buffer();

        loop {
            tokio::select! {
                biased; // Prefer USB reads to avoid buffer starvation

                // USB packet received from device
                completion = self.endpoint.next_complete() => {
                    // Immediately submit a new buffer for the next read
                    self.submit_read_buffer();

                    match self.process_completion(completion) {
                        Ok(packet) => {
                            if is_status_changed_notification(&packet) {
                                // Unsolicited notification - broadcast to subscribers
                                if let Some(flags) = parse_status_changed_notification(&packet) {
                                    debug!(?flags, "received status changed notification");
                                    let _ = self.notification_tx.send(Notification::StatusChanged(flags));
                                }
                            } else if let Some(cmd) = pending_command.take() {
                                // Response to our pending command
                                trace!(?packet, "received command response");
                                let _ = cmd.response_tx.send(Ok(packet));
                                response_timeout = None;
                            } else {
                                // Unexpected packet while idle - log and ignore
                                warn!(?packet, "received unexpected packet while idle");
                            }
                        }
                        Err(MouseError::Disconnected) => {
                            error!("device disconnected");
                            let _ = self.notification_tx.send(Notification::Disconnected);
                            if let Some(cmd) = pending_command.take() {
                                let _ = cmd.response_tx.send(Err(MouseError::Disconnected));
                            }
                            info!("IO task shutting down due to disconnect");
                            return;
                        }
                        Err(e) => {
                            error!(?e, "USB transfer error");
                            if let Some(cmd) = pending_command.take() {
                                let _ = cmd.response_tx.send(Err(e));
                                response_timeout = None;
                            }
                        }
                    }
                }

                // Command received from Mouse handle (only accept when no pending command)
                cmd = self.command_rx.recv(), if pending_command.is_none() => {
                    match cmd {
                        Some(cmd) => {
                            trace!(?cmd.packet, "received command to send");
                            if let Err(e) = self.send_control_transfer(&cmd.packet).await {
                                // Send failed - return error immediately
                                let _ = cmd.response_tx.send(Err(e));
                            } else {
                                // Command sent successfully, wait for response
                                response_timeout = Some(Box::pin(tokio::time::sleep(RESPONSE_TIMEOUT)));
                                pending_command = Some(cmd);
                            }
                        }
                        None => {
                            // Channel closed, Mouse handle was dropped
                            info!("command channel closed, shutting down IO task");
                            break;
                        }
                    }
                }

                // Response timeout (only active when we have a pending command)
                _ = async {
                    match response_timeout.as_mut() {
                        Some(t) => t.await,
                        None => std::future::pending::<()>().await,
                    }
                } => {
                    if let Some(cmd) = pending_command.take() {
                        let error = match self.mode {
                            ConnectionMode::Wireless => MouseError::DeviceOffline,
                            ConnectionMode::Wired => MouseError::Timeout,
                        };
                        warn!("command response timeout");
                        let _ = cmd.response_tx.send(Err(error));
                        response_timeout = None;
                    }
                }
            }
        }

        info!("IO task shutting down");
    }

    /// Submit a read buffer to the interrupt endpoint.
    ///
    /// This must be called to maintain a pending read buffer at all times,
    /// ensuring we can receive both command responses and unsolicited notifications.
    fn submit_read_buffer(&mut self) {
        let packet_size = self.mode.packet_size();
        let buf = Buffer::new(packet_size);
        self.endpoint.submit(buf);
    }

    /// Process a completed USB transfer.
    fn process_completion(&self, completion: nusb::transfer::Completion) -> Result<Vec<u8>> {
        match completion.status {
            Ok(()) => {
                if completion.actual_len == 0 {
                    return Err(MouseError::Timeout);
                }
                Ok(completion.buffer[..completion.actual_len].to_vec())
            }
            Err(TransferError::Disconnected) => Err(MouseError::Disconnected),
            Err(e) => Err(MouseError::Transfer(e)),
        }
    }

    /// Send a command packet to the device via control transfer.
    ///
    /// This only sends the command; the caller is responsible for handling
    /// the response which will arrive on the interrupt endpoint.
    async fn send_control_transfer(&self, cmd: &[u8; PACKET_LENGTH]) -> Result<()> {
        // Prepend report ID to command packet for HID transfer
        let mut report = [0u8; PACKET_LENGTH + 1];
        report[0] = REPORT_ID;
        report[1..].copy_from_slice(cmd);

        // Send command via control transfer (SET_REPORT)
        let result = self
            .interface
            .control_out(
                ControlOut {
                    control_type: ControlType::Class,
                    recipient: Recipient::Interface,
                    request: 0x09, // SET_REPORT
                    value: 0x0208, // Report Type (Output=2) | Report ID (0x08)
                    index: INTERFACE_NUM as u16,
                    data: &report,
                },
                Duration::from_millis(100),
            )
            .await;

        match result {
            Ok(_) => Ok(()),
            Err(TransferError::Disconnected) => Err(MouseError::Disconnected),
            Err(_) => {
                // Retry with Vendor request type (some devices need this)
                warn!("control transfer with Class request failed, trying Vendor request");
                self.interface
                    .control_out(
                        ControlOut {
                            control_type: ControlType::Vendor,
                            recipient: Recipient::Interface,
                            request: 0x09,
                            value: 0x0208,
                            index: INTERFACE_NUM as u16,
                            data: &report,
                        },
                        Duration::from_millis(100),
                    )
                    .await
                    .map_err(|e| {
                        if e == TransferError::Disconnected {
                            MouseError::Disconnected
                        } else {
                            MouseError::Transfer(e)
                        }
                    })?;
                Ok(())
            }
        }
    }
}
