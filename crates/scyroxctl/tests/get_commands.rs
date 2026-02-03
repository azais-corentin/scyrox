mod common;

use common::{assert_device_connected, get_config_json, parse_json_output, scyroxctl};
use predicates::prelude::*;

// ============ Text Output Tests ============

#[test]
fn test_get_config_text() {
    assert_device_connected();

    scyroxctl()
        .args(["get", "config"])
        .assert()
        .success()
        .stdout(
            predicate::str::contains("Polling Rate:")
                .and(predicate::str::contains("Lift-Off Distance:"))
                .and(predicate::str::contains("Sleep Timeout:"))
                .and(predicate::str::contains("Angle Snapping:"))
                .and(predicate::str::contains("Ripple Control:"))
                .and(predicate::str::contains("High Speed Mode:"))
                .and(predicate::str::contains("Long Distance:")),
        );
}

#[test]
fn test_get_battery_text() {
    assert_device_connected();

    scyroxctl()
        .args(["get", "battery"])
        .assert()
        .success()
        .stdout(
            predicate::str::contains("Voltage:")
                .and(predicate::str::contains("mV"))
                .and(predicate::str::contains("%")),
        );
}

#[test]
fn test_get_firmware_text() {
    assert_device_connected();

    scyroxctl()
        .args(["get", "firmware"])
        .assert()
        .success()
        .stdout(predicate::str::contains("Mouse:").and(predicate::str::contains("v")));
}

#[test]
fn test_get_polling_rate_text() {
    assert_device_connected();

    scyroxctl()
        .args(["get", "polling-rate"])
        .assert()
        .success()
        .stdout(predicate::str::contains("Hz"));
}

#[test]
fn test_get_lod_text() {
    assert_device_connected();

    scyroxctl()
        .args(["get", "lod"])
        .assert()
        .success()
        .stdout(predicate::str::contains("mm"));
}

#[test]
fn test_get_sleep_timeout_text() {
    assert_device_connected();

    scyroxctl()
        .args(["get", "sleep-timeout"])
        .assert()
        .success()
        .stdout(predicate::str::contains("s"));
}

// ============ JSON Output Tests ============

#[test]
fn test_get_config_json() {
    assert_device_connected();

    let output = scyroxctl()
        .args(["-f", "json", "get", "config"])
        .output()
        .expect("Failed to execute");

    assert!(output.status.success());
    let json = parse_json_output(&output);

    // Validate polling_rate is valid value
    let pr = json["polling_rate"]
        .as_u64()
        .expect("polling_rate should be int");
    assert!(
        [125, 250, 500, 1000, 2000, 4000, 8000].contains(&(pr as u16)),
        "Invalid polling rate: {}",
        pr
    );

    // Validate lift_off_distance is valid value
    let lod = json["lift_off_distance"]
        .as_f64()
        .expect("lift_off_distance should be float");
    assert!(
        (lod - 0.7).abs() < 0.01 || (lod - 1.0).abs() < 0.01 || (lod - 2.0).abs() < 0.01,
        "Invalid lift_off_distance: {}",
        lod
    );

    // Validate sleep_timeout_seconds is in range
    let st = json["sleep_timeout_seconds"]
        .as_u64()
        .expect("sleep_timeout_seconds should be int");
    assert!(st <= 2550, "sleep_timeout_seconds out of range: {}", st);

    // Validate boolean fields exist
    assert!(json["angle_snapping"].is_boolean());
    assert!(json["ripple_control"].is_boolean());
    assert!(json["high_speed_mode"].is_boolean());
    assert!(json["long_distance_mode"].is_boolean());
}

#[test]
fn test_get_battery_json() {
    assert_device_connected();

    let output = scyroxctl()
        .args(["-f", "json", "get", "battery"])
        .output()
        .expect("Failed to execute");

    assert!(output.status.success());
    let json = parse_json_output(&output);

    let voltage = json["voltage_mv"]
        .as_u64()
        .expect("voltage_mv should be int");
    assert!(voltage > 0, "voltage_mv should be > 0");

    let percentage = json["percentage"]
        .as_u64()
        .expect("percentage should be int");
    assert!(percentage <= 100, "percentage should be 0-100");

    assert!(json["charging"].is_boolean());
}

#[test]
fn test_get_firmware_json() {
    assert_device_connected();

    let output = scyroxctl()
        .args(["-f", "json", "get", "firmware"])
        .output()
        .expect("Failed to execute");

    assert!(output.status.success());
    let json = parse_json_output(&output);

    let mouse_version = json["mouse_version"]
        .as_str()
        .expect("mouse_version should be string");
    assert!(
        mouse_version.starts_with('v'),
        "mouse_version should start with 'v': {}",
        mouse_version
    );

    // receiver_version may be null if not connected via receiver
    if !json["receiver_version"].is_null() {
        let rv = json["receiver_version"]
            .as_str()
            .expect("receiver_version should be string if present");
        assert!(
            rv.starts_with('v'),
            "receiver_version should start with 'v'"
        );
    }
}

#[test]
fn test_get_polling_rate_json() {
    assert_device_connected();

    let output = scyroxctl()
        .args(["-f", "json", "get", "polling-rate"])
        .output()
        .expect("Failed to execute");

    assert!(output.status.success());
    let json = parse_json_output(&output);

    // Individual get commands return the numeric value directly
    let pr = json.as_u64().expect("polling-rate should be integer");
    assert!(
        [125, 250, 500, 1000, 2000, 4000, 8000].contains(&(pr as u16)),
        "Invalid polling rate: {}",
        pr
    );
}

#[test]
fn test_get_lod_json() {
    assert_device_connected();

    let output = scyroxctl()
        .args(["-f", "json", "get", "lod"])
        .output()
        .expect("Failed to execute");

    assert!(output.status.success());
    let json = parse_json_output(&output);

    let lod = json.as_f64().expect("lod should be float");
    assert!(
        (lod - 0.7).abs() < 0.01 || (lod - 1.0).abs() < 0.01 || (lod - 2.0).abs() < 0.01,
        "Invalid LOD value: {}",
        lod
    );
}

#[test]
fn test_get_sleep_timeout_json() {
    assert_device_connected();

    let output = scyroxctl()
        .args(["-f", "json", "get", "sleep-timeout"])
        .output()
        .expect("Failed to execute");

    assert!(output.status.success());
    let json = parse_json_output(&output);

    let st_str = json.as_str().expect("sleep-timeout should be string");
    assert!(st_str.ends_with('s'), "Should end with 's': {}", st_str);
}

// ============ Consistency Tests ============

#[test]
fn test_get_config_polling_rate_matches_individual() {
    assert_device_connected();

    let config = get_config_json();
    let config_pr = config["polling_rate"].as_u64().unwrap();

    let output = scyroxctl()
        .args(["-f", "json", "get", "polling-rate"])
        .output()
        .expect("Failed to execute");

    let individual = parse_json_output(&output);
    let individual_pr = individual.as_u64().expect("polling-rate should be integer");

    assert_eq!(
        config_pr, individual_pr,
        "Config PR {} should match individual {}",
        config_pr, individual_pr
    );
}

#[test]
fn test_get_config_lod_matches_individual() {
    assert_device_connected();

    let config = get_config_json();
    let config_lod = config["lift_off_distance"].as_f64().unwrap();

    let output = scyroxctl()
        .args(["-f", "json", "get", "lod"])
        .output()
        .expect("Failed to execute");

    let individual = parse_json_output(&output);
    let individual_lod = individual.as_f64().expect("lod should be float");

    assert!(
        (config_lod - individual_lod).abs() < 0.01,
        "Config LOD {} should match individual {}",
        config_lod,
        individual_lod
    );
}

#[test]
fn test_get_config_sleep_timeout_matches_individual() {
    assert_device_connected();

    let config = get_config_json();
    let config_st = config["sleep_timeout_seconds"].as_u64().unwrap();

    let output = scyroxctl()
        .args(["-f", "json", "get", "sleep-timeout"])
        .output()
        .expect("Failed to execute");

    let individual = parse_json_output(&output);
    let individual_str = individual.as_str().unwrap();

    assert!(
        individual_str.contains(&config_st.to_string()),
        "Config ST {} should match individual {}",
        config_st,
        individual_str
    );
}

#[test]
fn test_all_get_commands_exit_success() {
    assert_device_connected();

    let commands = [
        vec!["get", "config"],
        vec!["get", "battery"],
        vec!["get", "firmware"],
        vec!["get", "polling-rate"],
        vec!["get", "lod"],
        vec!["get", "sleep-timeout"],
    ];

    for cmd in commands {
        scyroxctl().args(&cmd).assert().success();
    }
}
