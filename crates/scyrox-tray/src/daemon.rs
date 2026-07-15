//! Daemon lifecycle management and the gRPC event-stream worker.
//!
//! The worker runs on a dedicated thread with its own Tokio runtime (the `tao`
//! event loop owns the main thread). It ensures `scyroxd` is running (spawning
//! it detached when unreachable), then pushes [`TrayState`] updates to the UI
//! through the event-loop proxy, reconnecting forever.

use std::process::Command;
use std::time::Duration;

use tao::event_loop::EventLoopProxy;
use tracing::{debug, error, info, warn};

use scyrox_client::{Backend, DaemonClient, DaemonConfig, EventStream};
use scyrox_proto::event::Event;

use crate::UserEvent;
use crate::notifications;
use crate::state::TrayState;

/// Attempts to connect to a freshly spawned daemon.
const SPAWN_CONNECT_ATTEMPTS: u32 = 10;
/// Delay between post-spawn connect attempts.
const SPAWN_CONNECT_DELAY: Duration = Duration::from_millis(500);
/// Backoff before retrying after a failed connect/spawn round.
const RETRY_BACKOFF: Duration = Duration::from_secs(5);
/// Delay before reconnecting after an established daemon session fails.
const RECONNECT_DELAY: Duration = Duration::from_secs(1);

/// What the worker should do next.
enum Flow {
    /// The event loop is gone; terminate the worker.
    Stop,
    /// Reconnect from the top.
    Reconnect,
}

/// Spawn the worker on a dedicated thread with its own Tokio runtime.
///
/// Returns the underlying [`std::io::Error`] if the thread could not be
/// spawned; a tray with no worker is useless, so the caller is expected to
/// exit the event loop on failure.
pub fn spawn(proxy: EventLoopProxy<UserEvent>) -> std::io::Result<()> {
    std::thread::Builder::new()
        .name("scyrox-tray-daemon".to_string())
        .spawn(move || match tokio::runtime::Runtime::new() {
            Ok(rt) => rt.block_on(worker(proxy)),
            Err(e) => error!("failed to build worker runtime: {e}"),
        })?;
    Ok(())
}

async fn worker(proxy: EventLoopProxy<UserEvent>) {
    loop {
        let client = match connect(&proxy).await {
            Ok(client) => client,
            Err(Flow::Stop) => return,
            Err(Flow::Reconnect) => continue,
        };

        // Subscribe before reading config so a concurrent threshold change is
        // never lost: it lands either in the fetched snapshot or in the
        // already-open stream.
        let stream = match client.watch_events().await {
            Ok(stream) => stream,
            Err(e) => {
                warn!("failed to open event stream: {e}");
                if !push_state(&proxy, TrayState::DaemonDown) {
                    return;
                }
                tokio::time::sleep(RECONNECT_DELAY).await;
                continue;
            }
        };

        // Best-effort: an older daemon (Unimplemented) or an invalid stored
        // config must not take down an otherwise healthy session; rendering
        // degrades to no threshold coloring.
        match client.get_daemon_config().await {
            Ok(config) => {
                if !push_threshold(&proxy, config.low_battery_threshold) {
                    return;
                }
            }
            Err(e) => warn!("failed to read daemon configuration: {e}"),
        }

        // Push an initial snapshot only after the effective threshold.
        if !push_state(&proxy, initial_state(&client).await) {
            return;
        }

        match consume_stream(&proxy, &client, stream).await {
            Flow::Stop => return,
            Flow::Reconnect => {
                if !push_state(&proxy, TrayState::DaemonDown) {
                    return;
                }
                tokio::time::sleep(RECONNECT_DELAY).await;
            }
        }
    }
}

/// Connect to the daemon, spawning it detached if unreachable.
async fn connect(proxy: &EventLoopProxy<UserEvent>) -> Result<DaemonClient, Flow> {
    if let Ok(client) = DaemonClient::connect().await {
        return Ok(client);
    }

    if !push_state(proxy, TrayState::DaemonDown) {
        return Err(Flow::Stop);
    }

    info!("daemon unreachable; attempting to spawn scyroxd");
    if !spawn_daemon() {
        error!("could not spawn scyroxd (is it installed and on PATH?)");
        tokio::time::sleep(RETRY_BACKOFF).await;
        return Err(Flow::Reconnect);
    }

    for attempt in 1..=SPAWN_CONNECT_ATTEMPTS {
        tokio::time::sleep(SPAWN_CONNECT_DELAY).await;
        if let Ok(client) = DaemonClient::connect().await {
            info!(attempt, "connected to spawned daemon");
            return Ok(client);
        }
    }

    warn!("spawned scyroxd but could not connect");
    tokio::time::sleep(RETRY_BACKOFF).await;
    Err(Flow::Reconnect)
}

/// Spawn `scyroxd` detached. Mirrors `scyroxctl`'s `start_daemon`: `setsid
/// --fork scyroxd`, falling back to a plain spawn (also the non-Unix path).
fn spawn_daemon() -> bool {
    Command::new("setsid")
        .args(["--fork", "scyroxd"])
        .spawn()
        .or_else(|_| Command::new("scyroxd").spawn())
        .is_ok()
}

/// Read the current state directly (used on connect and on reconnection).
async fn initial_state(client: &DaemonClient) -> TrayState {
    if !client.is_connected().await {
        return TrayState::Disconnected;
    }
    match client.get_battery().await {
        Ok(battery) => battery_from_domain(&battery),
        Err(e) => {
            debug!("get_battery failed: {e}");
            TrayState::Disconnected
        }
    }
}

/// Consume an already-open event stream until it ends or errors.
async fn consume_stream(
    proxy: &EventLoopProxy<UserEvent>,
    client: &DaemonClient,
    mut stream: EventStream,
) -> Flow {
    loop {
        match stream.message().await {
            Ok(Some(message)) => match handle_event(proxy, client, message).await {
                Ok(()) => {}
                Err(flow) => return flow,
            },
            Ok(None) => {
                info!("event stream ended");
                return Flow::Reconnect;
            }
            Err(e) => {
                warn!("event stream error: {e}");
                return Flow::Reconnect;
            }
        }
    }
}

/// Dispatch a proto event, requesting a reconnect for invalid daemon config.
async fn handle_event(
    proxy: &EventLoopProxy<UserEvent>,
    client: &DaemonClient,
    message: scyrox_proto::Event,
) -> Result<(), Flow> {
    let Some(event) = message.event else {
        return Ok(());
    };

    match event {
        Event::BatteryUpdate(update) => match update.status {
            Some(status) => {
                if push_state(proxy, battery_from_proto(&status)) {
                    Ok(())
                } else {
                    Err(Flow::Stop)
                }
            }
            None => Ok(()),
        },
        Event::ConnectionChange(change) => {
            let state = if change.connected {
                // A fresh reading beats waiting for the next poll tick.
                match client.get_battery().await {
                    Ok(battery) => battery_from_domain(&battery),
                    Err(_) => TrayState::Disconnected,
                }
            } else {
                TrayState::Disconnected
            };
            if push_state(proxy, state) {
                Ok(())
            } else {
                Err(Flow::Stop)
            }
        }
        Event::LowBatteryAlert(alert) => {
            notifications::battery_low(alert.percentage as u8).await;
            Ok(())
        }
        Event::DaemonConfigChanged(change) => {
            let Some(config) = change.config else {
                warn!("daemon configuration event missing config");
                return Err(Flow::Reconnect);
            };
            let config = DaemonConfig::try_from(config).map_err(|e| {
                warn!("invalid daemon configuration event: {e}");
                Flow::Reconnect
            })?;
            if push_threshold(proxy, config.low_battery_threshold) {
                Ok(())
            } else {
                Err(Flow::Stop)
            }
        }
        Event::ProfileApplied(_) | Event::SettingsChanged(_) => Ok(()),
    }
}

/// Push a state update; returns `false` when the event loop has closed.
fn push_state(proxy: &EventLoopProxy<UserEvent>, state: TrayState) -> bool {
    proxy.send_event(UserEvent::State(state)).is_ok()
}

fn push_threshold(proxy: &EventLoopProxy<UserEvent>, threshold: u8) -> bool {
    proxy
        .send_event(UserEvent::LowBatteryThreshold(threshold))
        .is_ok()
}

fn battery_from_domain(status: &scyrox::BatteryStatus) -> TrayState {
    TrayState::Battery {
        percentage: status.percentage,
        voltage_mv: status.voltage_mv,
        charging: status.charging,
    }
}

fn battery_from_proto(status: &scyrox_proto::BatteryStatus) -> TrayState {
    TrayState::Battery {
        percentage: status.percentage as u8,
        voltage_mv: status.voltage_mv as u16,
        charging: status.charging,
    }
}
