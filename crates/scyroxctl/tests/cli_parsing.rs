mod common;

use common::{assert_device_connected, scyroxctl, scyroxctl_raw};
use predicates::prelude::*;

#[test]
fn test_help_shows_usage() {
    scyroxctl_raw()
        .arg("--help")
        .assert()
        .success()
        .stdout(predicate::str::contains("Usage:"));
}

#[test]
fn test_version_shows_version() {
    scyroxctl_raw()
        .arg("--version")
        .assert()
        .success()
        .stdout(predicate::str::contains("scyroxctl"));
}

#[test]
fn test_invalid_subcommand_fails() {
    scyroxctl_raw()
        .arg("invalid")
        .assert()
        .failure()
        .stderr(predicate::str::contains("invalid"));
}

#[test]
fn test_missing_subcommand_shows_help() {
    scyroxctl_raw()
        .assert()
        .failure()
        .stderr(predicate::str::contains("Usage:"));
}

#[test]
fn test_get_requires_subcommand() {
    scyroxctl_raw().arg("get").assert().failure().stderr(
        predicate::str::is_match("config|battery|firmware|polling-rate|lod|sleep-timeout").unwrap(),
    );
}

#[test]
fn test_set_requires_subcommand() {
    scyroxctl_raw()
        .arg("set")
        .assert()
        .failure()
        .stderr(predicate::str::is_match("polling-rate|lod|sleep-timeout|angle-snapping").unwrap());
}

#[test]
fn test_invalid_polling_rate_rejected() {
    scyroxctl()
        .args(["set", "polling-rate", "999"])
        .assert()
        .failure()
        .stderr(
            predicate::str::contains("125")
                .and(predicate::str::contains("250"))
                .and(predicate::str::contains("8000")),
        );
}

#[test]
fn test_invalid_lod_rejected() {
    scyroxctl()
        .args(["set", "lod", "invalid"])
        .assert()
        .failure()
        .stderr(
            predicate::str::contains("low")
                .and(predicate::str::contains("medium"))
                .and(predicate::str::contains("high")),
        );
}

#[test]
fn test_invalid_bool_rejected() {
    scyroxctl()
        .args(["set", "angle-snapping", "maybe"])
        .assert()
        .failure()
        .stderr(
            predicate::str::contains("on")
                .and(predicate::str::contains("off"))
                .and(predicate::str::contains("true"))
                .and(predicate::str::contains("false")),
        );
}

#[test]
fn test_sleep_timeout_over_max_rejected() {
    assert_device_connected();

    scyroxctl()
        .args(["set", "sleep-timeout", "5000"])
        .assert()
        .failure()
        .stderr(predicate::str::contains("2550"));
}

#[test]
fn test_format_flag_accepts_json() {
    assert_device_connected();

    let output = scyroxctl()
        .args(["-f", "json", "status"])
        .output()
        .expect("Failed to execute");

    assert!(output.status.success());

    // Verify it's valid JSON
    let _: serde_json::Value =
        serde_json::from_slice(&output.stdout).expect("Output should be valid JSON");
}

#[test]
fn test_format_flag_rejects_invalid() {
    scyroxctl()
        .args(["-f", "xml", "status"])
        .assert()
        .failure()
        .stderr(predicate::str::contains("text").and(predicate::str::contains("json")));
}

#[test]
fn test_daemon_config_get_shape_is_accepted() {
    scyroxctl_raw()
        .args(["daemon", "config", "get", "--help"])
        .assert()
        .success()
        .stdout(predicate::str::contains("Usage:"));
}

#[test]
fn test_daemon_config_set_threshold_shape_is_accepted() {
    scyroxctl_raw()
        .args([
            "daemon",
            "config",
            "set",
            "low-battery-threshold",
            "10",
            "--help",
        ])
        .assert()
        .success()
        .stdout(predicate::str::contains("Usage:"));
}

#[test]
fn test_daemon_config_set_battery_log_path_shape_is_accepted() {
    scyroxctl_raw()
        .args([
            "daemon",
            "config",
            "set",
            "battery-log-path",
            "captures/battery.jsonl",
            "--help",
        ])
        .assert()
        .success()
        .stdout(predicate::str::contains("Usage:"));
}

#[test]
fn test_daemon_config_unset_battery_log_path_shape_is_accepted() {
    scyroxctl_raw()
        .args(["daemon", "config", "unset", "battery-log-path", "--help"])
        .assert()
        .success()
        .stdout(predicate::str::contains("Usage:"));
}

#[test]
fn test_daemon_config_rejects_threshold_above_one_hundred() {
    scyroxctl_raw()
        .args(["daemon", "config", "set", "low-battery-threshold", "101"])
        .assert()
        .failure()
        .stderr(predicate::str::contains("101").and(predicate::str::contains("0..=100")));
}

#[test]
fn test_daemon_config_rejects_removed_threshold_flag() {
    scyroxctl_raw()
        .args(["daemon", "config", "set", "--low-battery-threshold", "10"])
        .assert()
        .failure()
        .stderr(predicate::str::contains("--low-battery-threshold"));
}
