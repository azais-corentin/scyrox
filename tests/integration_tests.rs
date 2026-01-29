//! Integration tests for mouse communication.
//!
//! These tests require a physical mouse to be connected.
//! Run with: cargo test
//!
//! Tests are run sequentially (RUST_TEST_THREADS=1) to avoid USB conflicts.

use scyroxd::{LiftOffDistance, Mouse, PollingRate};

// =============================================================================
// Connection Tests
// =============================================================================

#[test]
fn test_open_mouse() {
    let mouse = Mouse::open().expect("Failed to open mouse - is it connected?");
    println!("Connected via {:?}", mouse.connection_mode());
}

#[test]
fn test_read_config() {
    let mut mouse = Mouse::open().expect("Failed to open mouse");
    let config = mouse.get_config().expect("Failed to read config");
    println!("Current configuration:");
    println!("  Polling Rate:      {}", config.polling_rate);
    println!("  Lift-Off Distance: {}", config.lift_off_distance);
    println!(
        "  Sleep Timeout:     {} seconds",
        config.sleep_timeout_seconds
    );
    println!("  Angle Snapping:    {}", config.angle_snapping);
    println!("  Ripple Control:    {}", config.ripple_control);
    println!("  High Speed Mode:   {}", config.high_speed_mode);
    println!("  Long Distance:     {}", config.long_distance_mode);
}

// =============================================================================
// Polling Rate Round-Trip Test
// =============================================================================

#[test]
fn test_polling_rate_round_trip() {
    let mut mouse = Mouse::open().expect("Failed to open mouse");

    // Read original value
    let original = mouse
        .get_polling_rate()
        .expect("Failed to read polling rate");
    println!("Original polling rate: {}", original);

    // Change to next value
    let new_rate = original.next();
    println!("Setting polling rate to: {}", new_rate);
    mouse
        .set_polling_rate(new_rate)
        .expect("Failed to set polling rate");

    // Verify the change
    let readback = mouse
        .get_polling_rate()
        .expect("Failed to read back polling rate");
    assert_eq!(
        readback, new_rate,
        "Polling rate mismatch: expected {:?}, got {:?}",
        new_rate, readback
    );
    println!("Verified: polling rate is now {}", readback);

    // Restore original value
    println!("Restoring original polling rate: {}", original);
    mouse
        .set_polling_rate(original)
        .expect("Failed to restore polling rate");

    let restored = mouse
        .get_polling_rate()
        .expect("Failed to read restored polling rate");
    assert_eq!(
        restored, original,
        "Failed to restore original polling rate"
    );
    println!("Restored: polling rate is back to {}", restored);
}

// =============================================================================
// Lift-Off Distance Round-Trip Test
// =============================================================================

#[test]
fn test_lift_off_distance_round_trip() {
    let mut mouse = Mouse::open().expect("Failed to open mouse");

    // Read original value
    let original = mouse
        .get_lift_off_distance()
        .expect("Failed to read lift-off distance");
    println!("Original lift-off distance: {}", original);

    // Change to next value
    let new_lod = original.next();
    println!("Setting lift-off distance to: {}", new_lod);
    mouse
        .set_lift_off_distance(new_lod)
        .expect("Failed to set lift-off distance");

    // Verify the change
    let readback = mouse
        .get_lift_off_distance()
        .expect("Failed to read back lift-off distance");
    assert_eq!(
        readback, new_lod,
        "Lift-off distance mismatch: expected {:?}, got {:?}",
        new_lod, readback
    );
    println!("Verified: lift-off distance is now {}", readback);

    // Restore original value
    println!("Restoring original lift-off distance: {}", original);
    mouse
        .set_lift_off_distance(original)
        .expect("Failed to restore lift-off distance");

    let restored = mouse
        .get_lift_off_distance()
        .expect("Failed to read restored lift-off distance");
    assert_eq!(
        restored, original,
        "Failed to restore original lift-off distance"
    );
    println!("Restored: lift-off distance is back to {}", restored);
}

// =============================================================================
// Sleep Timeout Round-Trip Test
// =============================================================================

#[test]
fn test_sleep_timeout_round_trip() {
    let mut mouse = Mouse::open().expect("Failed to open mouse");

    // Read original value
    let original = mouse
        .get_sleep_timeout()
        .expect("Failed to read sleep timeout");
    println!("Original sleep timeout: {} seconds", original);

    // Cycle to a different value (10s -> 30s -> 60s -> 10s)
    let new_timeout = match original {
        0..=10 => 30,
        11..=30 => 60,
        31..=60 => 300,
        _ => 10,
    };
    println!("Setting sleep timeout to: {} seconds", new_timeout);
    mouse
        .set_sleep_timeout(new_timeout)
        .expect("Failed to set sleep timeout");

    // Verify the change
    let readback = mouse
        .get_sleep_timeout()
        .expect("Failed to read back sleep timeout");
    assert_eq!(
        readback, new_timeout,
        "Sleep timeout mismatch: expected {}, got {}",
        new_timeout, readback
    );
    println!("Verified: sleep timeout is now {} seconds", readback);

    // Restore original value
    println!("Restoring original sleep timeout: {} seconds", original);
    mouse
        .set_sleep_timeout(original)
        .expect("Failed to restore sleep timeout");

    let restored = mouse
        .get_sleep_timeout()
        .expect("Failed to read restored sleep timeout");
    assert_eq!(
        restored, original,
        "Failed to restore original sleep timeout"
    );
    println!("Restored: sleep timeout is back to {} seconds", restored);
}

// =============================================================================
// Angle Snapping Round-Trip Test
// =============================================================================

#[test]
fn test_angle_snapping_round_trip() {
    let mut mouse = Mouse::open().expect("Failed to open mouse");

    // Read original value
    let original = mouse
        .get_angle_snapping()
        .expect("Failed to read angle snapping");
    println!(
        "Original angle snapping: {}",
        if original { "On" } else { "Off" }
    );

    // Toggle the value
    let new_value = !original;
    println!(
        "Setting angle snapping to: {}",
        if new_value { "On" } else { "Off" }
    );
    mouse
        .set_angle_snapping(new_value)
        .expect("Failed to set angle snapping");

    // Verify the change
    let readback = mouse
        .get_angle_snapping()
        .expect("Failed to read back angle snapping");
    assert_eq!(
        readback, new_value,
        "Angle snapping mismatch: expected {}, got {}",
        new_value, readback
    );
    println!(
        "Verified: angle snapping is now {}",
        if readback { "On" } else { "Off" }
    );

    // Restore original value
    println!(
        "Restoring original angle snapping: {}",
        if original { "On" } else { "Off" }
    );
    mouse
        .set_angle_snapping(original)
        .expect("Failed to restore angle snapping");

    let restored = mouse
        .get_angle_snapping()
        .expect("Failed to read restored angle snapping");
    assert_eq!(
        restored, original,
        "Failed to restore original angle snapping"
    );
    println!(
        "Restored: angle snapping is back to {}",
        if restored { "On" } else { "Off" }
    );
}

// =============================================================================
// Ripple Control Round-Trip Test
// =============================================================================

#[test]
fn test_ripple_control_round_trip() {
    let mut mouse = Mouse::open().expect("Failed to open mouse");

    // Read original value
    let original = mouse
        .get_ripple_control()
        .expect("Failed to read ripple control");
    println!(
        "Original ripple control: {}",
        if original { "On" } else { "Off" }
    );

    // Toggle the value
    let new_value = !original;
    println!(
        "Setting ripple control to: {}",
        if new_value { "On" } else { "Off" }
    );
    mouse
        .set_ripple_control(new_value)
        .expect("Failed to set ripple control");

    // Verify the change
    let readback = mouse
        .get_ripple_control()
        .expect("Failed to read back ripple control");
    assert_eq!(
        readback, new_value,
        "Ripple control mismatch: expected {}, got {}",
        new_value, readback
    );
    println!(
        "Verified: ripple control is now {}",
        if readback { "On" } else { "Off" }
    );

    // Restore original value
    println!(
        "Restoring original ripple control: {}",
        if original { "On" } else { "Off" }
    );
    mouse
        .set_ripple_control(original)
        .expect("Failed to restore ripple control");

    let restored = mouse
        .get_ripple_control()
        .expect("Failed to read restored ripple control");
    assert_eq!(
        restored, original,
        "Failed to restore original ripple control"
    );
    println!(
        "Restored: ripple control is back to {}",
        if restored { "On" } else { "Off" }
    );
}

// =============================================================================
// High Speed Mode Round-Trip Test
// =============================================================================

#[test]
fn test_high_speed_mode_round_trip() {
    let mut mouse = Mouse::open().expect("Failed to open mouse");

    // Read original value
    let original = mouse
        .get_high_speed_mode()
        .expect("Failed to read high speed mode");
    println!(
        "Original high speed mode: {}",
        if original { "On" } else { "Off" }
    );

    // Toggle the value
    let new_value = !original;
    println!(
        "Setting high speed mode to: {}",
        if new_value { "On" } else { "Off" }
    );
    mouse
        .set_high_speed_mode(new_value)
        .expect("Failed to set high speed mode");

    // Verify the change
    let readback = mouse
        .get_high_speed_mode()
        .expect("Failed to read back high speed mode");
    assert_eq!(
        readback, new_value,
        "High speed mode mismatch: expected {}, got {}",
        new_value, readback
    );
    println!(
        "Verified: high speed mode is now {}",
        if readback { "On" } else { "Off" }
    );

    // Restore original value
    println!(
        "Restoring original high speed mode: {}",
        if original { "On" } else { "Off" }
    );
    mouse
        .set_high_speed_mode(original)
        .expect("Failed to restore high speed mode");

    let restored = mouse
        .get_high_speed_mode()
        .expect("Failed to read restored high speed mode");
    assert_eq!(
        restored, original,
        "Failed to restore original high speed mode"
    );
    println!(
        "Restored: high speed mode is back to {}",
        if restored { "On" } else { "Off" }
    );
}

// =============================================================================
// Long Distance Mode Round-Trip Test
// =============================================================================

#[test]
fn test_long_distance_mode_round_trip() {
    let mut mouse = Mouse::open().expect("Failed to open mouse");

    // Read original value
    let original = mouse
        .get_long_distance_mode()
        .expect("Failed to read long distance mode");
    println!(
        "Original long distance mode: {}",
        if original { "On" } else { "Off" }
    );

    // Toggle the value
    let new_value = !original;
    println!(
        "Setting long distance mode to: {}",
        if new_value { "On" } else { "Off" }
    );
    mouse
        .set_long_distance_mode(new_value)
        .expect("Failed to set long distance mode");

    // Verify the change
    let readback = mouse
        .get_long_distance_mode()
        .expect("Failed to read back long distance mode");
    assert_eq!(
        readback, new_value,
        "Long distance mode mismatch: expected {}, got {}",
        new_value, readback
    );
    println!(
        "Verified: long distance mode is now {}",
        if readback { "On" } else { "Off" }
    );

    // Restore original value
    println!(
        "Restoring original long distance mode: {}",
        if original { "On" } else { "Off" }
    );
    mouse
        .set_long_distance_mode(original)
        .expect("Failed to restore long distance mode");

    let restored = mouse
        .get_long_distance_mode()
        .expect("Failed to read restored long distance mode");
    assert_eq!(
        restored, original,
        "Failed to restore original long distance mode"
    );
    println!(
        "Restored: long distance mode is back to {}",
        if restored { "On" } else { "Off" }
    );
}

// =============================================================================
// Battery and Firmware Read Tests
// =============================================================================

#[test]
fn test_battery_read() {
    let mut mouse = Mouse::open().expect("Failed to open mouse");
    let battery = mouse.get_battery().expect("Failed to read battery");

    println!(
        "Battery: {} mV ({}%)",
        battery.voltage_mv, battery.percentage
    );

    // Sanity checks
    assert!(
        battery.voltage_mv >= 3000 && battery.voltage_mv <= 4500,
        "Battery voltage {} mV out of expected range (3000-4500 mV)",
        battery.voltage_mv
    );
    assert!(
        battery.percentage <= 100,
        "Battery percentage {} > 100%",
        battery.percentage
    );
}

#[test]
fn test_firmware_read() {
    let mut mouse = Mouse::open().expect("Failed to open mouse");
    let firmware = mouse.get_firmware_info().expect("Failed to read firmware");

    println!("Mouse firmware: {}", firmware.mouse_version);
    if let Some(ref receiver) = firmware.receiver_version {
        println!("Receiver firmware: {}", receiver);
    }

    // Mouse firmware should start with 'v'
    assert!(
        firmware.mouse_version.starts_with('v'),
        "Unexpected firmware format: {}",
        firmware.mouse_version
    );
}

// =============================================================================
// All Polling Rates Test
// =============================================================================

#[test]
fn test_all_polling_rates() {
    let mut mouse = Mouse::open().expect("Failed to open mouse");

    // Save original
    let original = mouse
        .get_polling_rate()
        .expect("Failed to read original polling rate");
    println!("Original polling rate: {}", original);

    // Test each polling rate
    for rate in PollingRate::ALL {
        println!("Testing polling rate: {}", rate);
        mouse
            .set_polling_rate(rate)
            .expect(&format!("Failed to set polling rate to {}", rate));

        let readback = mouse.get_polling_rate().expect("Failed to read back");
        assert_eq!(readback, rate, "Polling rate mismatch for {}", rate);
        println!("  Verified: {}", readback);
    }

    // Restore original
    mouse
        .set_polling_rate(original)
        .expect("Failed to restore polling rate");
    println!("Restored: {}", original);
}

// =============================================================================
// All Lift-Off Distances Test
// =============================================================================

#[test]
fn test_all_lift_off_distances() {
    let mut mouse = Mouse::open().expect("Failed to open mouse");

    // Save original
    let original = mouse
        .get_lift_off_distance()
        .expect("Failed to read original lift-off distance");
    println!("Original lift-off distance: {}", original);

    // Test each lift-off distance
    for lod in LiftOffDistance::ALL {
        println!("Testing lift-off distance: {}", lod);
        mouse
            .set_lift_off_distance(lod)
            .expect(&format!("Failed to set lift-off distance to {}", lod));

        let readback = mouse.get_lift_off_distance().expect("Failed to read back");
        assert_eq!(readback, lod, "Lift-off distance mismatch for {}", lod);
        println!("  Verified: {}", readback);
    }

    // Restore original
    mouse
        .set_lift_off_distance(original)
        .expect("Failed to restore lift-off distance");
    println!("Restored: {}", original);
}
