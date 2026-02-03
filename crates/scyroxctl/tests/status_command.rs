mod common;

use common::{assert_device_connected, parse_json_output, scyroxctl};
use predicates::prelude::*;

#[test]
fn test_status_text_shows_device_section() {
    assert_device_connected();

    scyroxctl()
        .args(["status"])
        .assert()
        .success()
        .stdout(predicate::str::contains("Device Status:"));
}

#[test]
fn test_status_text_shows_connected() {
    assert_device_connected();

    scyroxctl()
        .args(["status"])
        .assert()
        .success()
        .stdout(predicate::str::contains("Connected:").and(predicate::str::contains("Yes")));
}

#[test]
fn test_status_text_shows_polling_rate() {
    assert_device_connected();

    scyroxctl()
        .args(["status"])
        .assert()
        .success()
        .stdout(predicate::str::contains("Polling Rate:").and(predicate::str::contains("Hz")));
}

#[test]
fn test_status_text_shows_battery() {
    assert_device_connected();

    scyroxctl()
        .args(["status"])
        .assert()
        .success()
        .stdout(predicate::str::contains("Battery:").and(predicate::str::contains("%")));
}

#[test]
fn test_status_json_structure() {
    assert_device_connected();

    let output = scyroxctl()
        .args(["-f", "json", "status"])
        .output()
        .expect("Failed to execute");

    assert!(output.status.success());
    let json = parse_json_output(&output);

    // connected should be true (we verified this in assert_device_connected)
    assert_eq!(json["connected"], true);

    // polling_rate should be a string with Hz
    let pr = json["polling_rate"]
        .as_str()
        .expect("polling_rate should be string");
    assert!(pr.contains("Hz"), "polling_rate should contain Hz: {}", pr);

    // battery should be an object with voltage_mv and percentage
    let battery = &json["battery"];
    assert!(
        battery["voltage_mv"].is_u64(),
        "battery.voltage_mv should be int"
    );
    assert!(
        battery["percentage"].is_u64(),
        "battery.percentage should be int"
    );
}

#[test]
fn test_status_exits_success() {
    assert_device_connected();

    scyroxctl().args(["status"]).assert().success();
}
