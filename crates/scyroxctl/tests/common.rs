#![allow(dead_code)]

use assert_cmd::Command;
#[allow(unused_imports)]
use predicates::prelude::*;
use serde_json::Value;

/// Returns a Command for scyroxctl with -d (direct) flag
pub fn scyroxctl() -> Command {
    let mut cmd = assert_cmd::cargo::cargo_bin_cmd!("scyroxctl");
    cmd.arg("-d");
    cmd
}

/// Returns a Command without -d flag (for CLI parsing tests)
pub fn scyroxctl_raw() -> Command {
    assert_cmd::cargo::cargo_bin_cmd!("scyroxctl")
}

/// Returns a Command WITHOUT -d, so the CLI uses the daemon when reachable.
pub fn scyroxctl_daemon() -> Command {
    assert_cmd::cargo::cargo_bin_cmd!("scyroxctl")
}

/// Panics with a clear message unless the scyroxd daemon is running and reachable.
pub fn assert_daemon_running() {
    let output = scyroxctl_daemon()
        .args(["-f", "json", "status"])
        .output()
        .expect("Failed to execute scyroxctl");

    if !output.status.success() {
        panic!(
            "Daemon status failed - start scyroxd first (scyroxctl daemon start).\nstderr: {}",
            String::from_utf8_lossy(&output.stderr)
        );
    }

    let json: Value = serde_json::from_slice(&output.stdout).expect("Failed to parse status JSON");

    // The `daemon` field is only populated when the daemon backend is in use;
    // in direct-fallback mode it is skipped. Its presence proves reachability.
    if json.get("daemon").map(Value::is_null).unwrap_or(true) {
        panic!(
            "Daemon not reachable (CLI fell back to direct mode). \
             Start scyroxd first: scyroxctl daemon start"
        );
    }
}

/// Panics with clear message if device not connected
pub fn assert_device_connected() {
    let output = scyroxctl()
        .args(["-f", "json", "status"])
        .output()
        .expect("Failed to execute scyroxctl");

    if !output.status.success() {
        panic!(
            "Failed to run scyroxctl. Ensure it builds correctly.\n\
             stderr: {}",
            String::from_utf8_lossy(&output.stderr)
        );
    }

    let json: Value = serde_json::from_slice(&output.stdout).expect("Failed to parse status JSON");

    if json["connected"] != true {
        panic!(
            "Scyrox mouse not connected. These integration tests require a physical device.\n\
             Connect the mouse and try again."
        );
    }

    // Also verify we can actually communicate (mouse not sleeping)
    let config_output = scyroxctl()
        .args(["-f", "json", "get", "config"])
        .output()
        .expect("Failed to execute get config");

    if !config_output.status.success() {
        panic!(
            "Scyrox mouse connected but not responding (may be sleeping).\n\
             Wake the mouse by moving it and try again.\n\
             Error: {}",
            String::from_utf8_lossy(&config_output.stderr)
        );
    }
}

/// Gets current config as JSON Value
pub fn get_config_json() -> Value {
    let output = scyroxctl()
        .args(["-f", "json", "get", "config"])
        .output()
        .expect("Failed to get config");

    assert!(
        output.status.success(),
        "get config failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    serde_json::from_slice(&output.stdout).expect("Failed to parse config JSON")
}

/// Restores all settings from saved config
pub fn restore_config(config: &Value) {
    // Restore polling rate
    if let Some(pr) = config["polling_rate"].as_u64() {
        scyroxctl()
            .args(["set", "polling-rate", &pr.to_string()])
            .assert()
            .success();
    }

    // Restore LOD (convert 0.7 -> "low", 1.0 -> "medium", 2.0 -> "high")
    if let Some(lod) = config["lift_off_distance"].as_f64() {
        let lod_arg = if lod < 0.9 {
            "low"
        } else if lod < 1.5 {
            "medium"
        } else {
            "high"
        };
        scyroxctl().args(["set", "lod", lod_arg]).assert().success();
    }

    // Restore sleep timeout
    if let Some(st) = config["sleep_timeout_seconds"].as_u64() {
        scyroxctl()
            .args(["set", "sleep-timeout", &st.to_string()])
            .assert()
            .success();
    }

    // Restore boolean settings
    for (key, cmd) in [
        ("angle_snapping", "angle-snapping"),
        ("ripple_control", "ripple-control"),
        ("high_speed_mode", "high-speed-mode"),
        ("long_distance_mode", "long-distance-mode"),
    ] {
        if let Some(val) = config[key].as_bool() {
            let arg = if val { "on" } else { "off" };
            scyroxctl().args(["set", cmd, arg]).assert().success();
        }
    }

    // Restore DPI stages (empty array means DPI was unavailable; leave untouched)
    if let Some(stages) = config["dpi_stages"].as_array()
        && !stages.is_empty()
    {
        scyroxctl()
            .args(["set", "dpi-count", &stages.len().to_string()])
            .assert()
            .success();
        for (i, stage) in stages.iter().enumerate() {
            if let Some(value) = stage["value"].as_u64() {
                scyroxctl()
                    .args(["set", "dpi", &value.to_string(), "--stage", &i.to_string()])
                    .assert()
                    .success();
            }
            if let Some(color) = stage["color"].as_str() {
                scyroxctl()
                    .args(["set", "dpi-color", color, "--stage", &i.to_string()])
                    .assert()
                    .success();
            }
        }
        if let Some(idx) = config["current_dpi_index"].as_u64() {
            scyroxctl()
                .args(["set", "dpi-stage", &idx.to_string()])
                .assert()
                .success();
        }
    }
}

/// RAII guard for config restoration
pub struct ConfigGuard {
    original: Value,
}

impl ConfigGuard {
    pub fn new() -> Self {
        Self {
            original: get_config_json(),
        }
    }
}

impl Default for ConfigGuard {
    fn default() -> Self {
        Self::new()
    }
}

impl Drop for ConfigGuard {
    fn drop(&mut self) {
        restore_config(&self.original);
    }
}

/// Helper to parse JSON output from command
pub fn parse_json_output(output: &std::process::Output) -> Value {
    serde_json::from_slice(&output.stdout).expect("Failed to parse JSON output")
}
