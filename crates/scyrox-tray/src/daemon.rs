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

use scyrox_client::{Backend, DaemonClient};
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

/// What the worker should do next.
enum Flow {
    /// The event loop is gone; terminate the worker.
    Stop,
    /// Reconnect from the top.
    Reconnect,
}

/// Spawn the worker on a dedicated thread with its own Tokio runtime.
pub fn spawn(proxy: EventLoopProxy<UserEvent>) {
    std::thread::Builder::new()
        .name("scyrox-tray-daemon".to_string())
        .spawn(move || match tokio::runtime::Runtime::new() {
            Ok(rt) => rt.block_on(worker(proxy)),
            Err(e) => error!("failed to build worker runtime: {e}"),
        })
        .expect("failed to spawn daemon worker thread");
}

async fn worker(proxy: EventLoopProxy<UserEvent>) {
    loop {
        let client = match connect(&proxy).await {
            Ok(client) => client,
            Err(Flow::Stop) => return,
            Err(Flow::Reconnect) => continue,
        };

        // Push an initial snapshot before subscribing to the stream.
        if !push_state(&proxy, initial_state(&client).await) {
            return;
        }

        match run_stream(&proxy, &client).await {
            Flow::Stop => return,
            Flow::Reconnect => {
                if !push_state(&proxy, TrayState::DaemonDown) {
                    return;
                }
                tokio::time::sleep(Duration::from_secs(1)).await;
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

/// Consume the event stream until it ends or errors.
async fn run_stream(proxy: &EventLoopProxy<UserEvent>, client: &DaemonClient) -> Flow {
    let mut stream = match client.watch_events().await {
        Ok(stream) => stream,
        Err(e) => {
            warn!("failed to open event stream: {e}");
            return Flow::Reconnect;
        }
    };

    loop {
        match stream.message().await {
            Ok(Some(message)) => {
                if !handle_event(proxy, client, message).await {
                    return Flow::Stop;
                }
            }
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

/// Dispatch a single proto event. Returns `false` if the event loop is gone.
async fn handle_event(
    proxy: &EventLoopProxy<UserEvent>,
    client: &DaemonClient,
    message: scyrox_proto::Event,
) -> bool {
    let Some(event) = message.event else {
        return true;
    };

    match event {
        Event::BatteryUpdate(update) => match update.status {
            Some(status) => push_state(proxy, battery_from_proto(&status)),
            None => true,
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
            push_state(proxy, state)
        }
        Event::LowBatteryAlert(alert) => {
            notifications::battery_low(alert.percentage as u8).await;
            true
        }
        Event::ProfileApplied(_) | Event::SettingsChanged(_) => true,
    }
}

/// Push a state update; returns `false` when the event loop has closed.
fn push_state(proxy: &EventLoopProxy<UserEvent>, state: TrayState) -> bool {
    proxy.send_event(UserEvent::State(state)).is_ok()
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
