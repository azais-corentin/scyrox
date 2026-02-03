#![allow(dead_code)]

use assert_cmd::Command;
#[allow(unused_imports)]
use predicates::prelude::*;
use serde_json::Value;

/// Returns a Command for scyroxctl with -d (direct) flag
pub fn scyroxctl() -> Command {
    let mut cmd = Command::cargo_bin("scyroxctl").unwrap();
    cmd.arg("-d");
    cmd
}

/// Returns a Command without -d flag (for CLI parsing tests)
pub fn scyroxctl_raw() -> Command {
    Command::cargo_bin("scyroxctl").unwrap()
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

impl Drop for ConfigGuard {
    fn drop(&mut self) {
        restore_config(&self.original);
    }
}

/// Helper to parse JSON output from command
pub fn parse_json_output(output: &std::process::Output) -> Value {
    serde_json::from_slice(&output.stdout).expect("Failed to parse JSON output")
}
