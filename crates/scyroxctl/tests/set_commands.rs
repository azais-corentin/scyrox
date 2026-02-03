mod common;

use common::{ConfigGuard, assert_device_connected, get_config_json, scyroxctl};
use predicates::prelude::*;

// ============ Polling Rate Tests ============

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

#[test]
fn test_set_polling_rate_250() {
    assert_device_connected();
    let _guard = ConfigGuard::new();

    scyroxctl()
        .args(["set", "polling-rate", "250"])
        .assert()
        .success();

    let config = get_config_json();
    assert_eq!(config["polling_rate"], 250);
}

#[test]
fn test_set_polling_rate_500() {
    assert_device_connected();
    let _guard = ConfigGuard::new();

    scyroxctl()
        .args(["set", "polling-rate", "500"])
        .assert()
        .success();

    let config = get_config_json();
    assert_eq!(config["polling_rate"], 500);
}

#[test]
fn test_set_polling_rate_1000() {
    assert_device_connected();
    let _guard = ConfigGuard::new();

    scyroxctl()
        .args(["set", "polling-rate", "1000"])
        .assert()
        .success();

    let config = get_config_json();
    assert_eq!(config["polling_rate"], 1000);
}

#[test]
fn test_set_polling_rate_2000() {
    assert_device_connected();
    let _guard = ConfigGuard::new();

    scyroxctl()
        .args(["set", "polling-rate", "2000"])
        .assert()
        .success();

    let config = get_config_json();
    assert_eq!(config["polling_rate"], 2000);
}

#[test]
fn test_set_polling_rate_4000() {
    assert_device_connected();
    let _guard = ConfigGuard::new();

    scyroxctl()
        .args(["set", "polling-rate", "4000"])
        .assert()
        .success();

    let config = get_config_json();
    assert_eq!(config["polling_rate"], 4000);
}

#[test]
fn test_set_polling_rate_8000() {
    assert_device_connected();
    let _guard = ConfigGuard::new();

    scyroxctl()
        .args(["set", "polling-rate", "8000"])
        .assert()
        .success();

    let config = get_config_json();
    assert_eq!(config["polling_rate"], 8000);
}

#[test]
fn test_set_polling_rate_success_message() {
    assert_device_connected();
    let _guard = ConfigGuard::new();

    scyroxctl()
        .args(["set", "polling-rate", "1000"])
        .assert()
        .success()
        .stdout(predicate::str::contains("Polling rate set to 1000"));
}

// ============ LOD Tests ============

#[test]
fn test_set_lod_low() {
    assert_device_connected();
    let _guard = ConfigGuard::new();

    scyroxctl().args(["set", "lod", "low"]).assert().success();

    let config = get_config_json();
    let lod = config["lift_off_distance"].as_f64().unwrap();
    assert!((lod - 0.7).abs() < 0.01, "Expected 0.7, got {}", lod);
}

#[test]
fn test_set_lod_medium() {
    assert_device_connected();
    let _guard = ConfigGuard::new();

    scyroxctl()
        .args(["set", "lod", "medium"])
        .assert()
        .success();

    let config = get_config_json();
    let lod = config["lift_off_distance"].as_f64().unwrap();
    assert!((lod - 1.0).abs() < 0.01, "Expected 1.0, got {}", lod);
}

#[test]
fn test_set_lod_high() {
    assert_device_connected();
    let _guard = ConfigGuard::new();

    scyroxctl().args(["set", "lod", "high"]).assert().success();

    let config = get_config_json();
    let lod = config["lift_off_distance"].as_f64().unwrap();
    assert!((lod - 2.0).abs() < 0.01, "Expected 2.0, got {}", lod);
}

#[test]
fn test_set_lod_success_message() {
    assert_device_connected();
    let _guard = ConfigGuard::new();

    scyroxctl()
        .args(["set", "lod", "medium"])
        .assert()
        .success()
        .stdout(predicate::str::contains("Lift-off distance set to 1.0mm"));
}

// ============ Sleep Timeout Tests ============

#[test]
fn test_set_sleep_timeout_zero() {
    assert_device_connected();
    let _guard = ConfigGuard::new();

    scyroxctl()
        .args(["set", "sleep-timeout", "0"])
        .assert()
        .success()
        .stdout(predicate::str::contains("disabled"));
}

#[test]
fn test_set_sleep_timeout_value() {
    assert_device_connected();
    let _guard = ConfigGuard::new();

    scyroxctl()
        .args(["set", "sleep-timeout", "30"])
        .assert()
        .success();

    let config = get_config_json();
    assert_eq!(config["sleep_timeout_seconds"], 30);
}

#[test]
fn test_set_sleep_timeout_max() {
    assert_device_connected();
    let _guard = ConfigGuard::new();

    scyroxctl()
        .args(["set", "sleep-timeout", "2550"])
        .assert()
        .success();

    let config = get_config_json();
    assert_eq!(config["sleep_timeout_seconds"], 2550);
}

// ============ Boolean Setting Tests ============

#[test]
fn test_set_angle_snapping_on() {
    assert_device_connected();
    let _guard = ConfigGuard::new();

    scyroxctl()
        .args(["set", "angle-snapping", "on"])
        .assert()
        .success();

    let config = get_config_json();
    assert_eq!(config["angle_snapping"], true);
}

#[test]
fn test_set_angle_snapping_off() {
    assert_device_connected();
    let _guard = ConfigGuard::new();

    scyroxctl()
        .args(["set", "angle-snapping", "off"])
        .assert()
        .success();

    let config = get_config_json();
    assert_eq!(config["angle_snapping"], false);
}

#[test]
fn test_set_angle_snapping_true() {
    assert_device_connected();
    let _guard = ConfigGuard::new();

    scyroxctl()
        .args(["set", "angle-snapping", "true"])
        .assert()
        .success();

    let config = get_config_json();
    assert_eq!(config["angle_snapping"], true);
}

#[test]
fn test_set_angle_snapping_1() {
    assert_device_connected();
    let _guard = ConfigGuard::new();

    scyroxctl()
        .args(["set", "angle-snapping", "1"])
        .assert()
        .success();

    let config = get_config_json();
    assert_eq!(config["angle_snapping"], true);
}

#[test]
fn test_set_ripple_control_toggle() {
    assert_device_connected();
    let _guard = ConfigGuard::new();

    // Set on
    scyroxctl()
        .args(["set", "ripple-control", "on"])
        .assert()
        .success();

    let config = get_config_json();
    assert_eq!(config["ripple_control"], true);

    // Set off
    scyroxctl()
        .args(["set", "ripple-control", "off"])
        .assert()
        .success();

    let config = get_config_json();
    assert_eq!(config["ripple_control"], false);
}

#[test]
fn test_set_high_speed_mode_toggle() {
    assert_device_connected();
    let _guard = ConfigGuard::new();

    // Set on
    scyroxctl()
        .args(["set", "high-speed-mode", "on"])
        .assert()
        .success();

    let config = get_config_json();
    assert_eq!(config["high_speed_mode"], true);

    // Set off
    scyroxctl()
        .args(["set", "high-speed-mode", "off"])
        .assert()
        .success();

    let config = get_config_json();
    assert_eq!(config["high_speed_mode"], false);
}

#[test]
fn test_set_long_distance_mode_toggle() {
    assert_device_connected();
    let _guard = ConfigGuard::new();

    // Set on
    scyroxctl()
        .args(["set", "long-distance-mode", "on"])
        .assert()
        .success();

    let config = get_config_json();
    assert_eq!(config["long_distance_mode"], true);

    // Set off
    scyroxctl()
        .args(["set", "long-distance-mode", "off"])
        .assert()
        .success();

    let config = get_config_json();
    assert_eq!(config["long_distance_mode"], false);
}

// ============ General Tests ============

#[test]
fn test_all_set_commands_exit_success() {
    assert_device_connected();
    let _guard = ConfigGuard::new();

    let commands = [
        vec!["set", "polling-rate", "1000"],
        vec!["set", "lod", "medium"],
        vec!["set", "sleep-timeout", "60"],
        vec!["set", "angle-snapping", "off"],
        vec!["set", "ripple-control", "off"],
        vec!["set", "high-speed-mode", "off"],
        vec!["set", "long-distance-mode", "off"],
    ];

    for cmd in commands {
        scyroxctl().args(&cmd).assert().success();
    }
}

#[test]
fn test_multiple_settings_persist() {
    assert_device_connected();
    let _guard = ConfigGuard::new();

    // Set multiple different settings
    scyroxctl()
        .args(["set", "polling-rate", "500"])
        .assert()
        .success();

    scyroxctl().args(["set", "lod", "high"]).assert().success();

    scyroxctl()
        .args(["set", "angle-snapping", "on"])
        .assert()
        .success();

    // Verify all persisted
    let config = get_config_json();
    assert_eq!(config["polling_rate"], 500);
    let lod = config["lift_off_distance"].as_f64().unwrap();
    assert!((lod - 2.0).abs() < 0.01);
    assert_eq!(config["angle_snapping"], true);
}
