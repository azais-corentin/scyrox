//! Scyrox mouse configuration CLI tool.

use scyroxd::{ConnectionMode, Mouse, MouseError};

fn main() -> Result<(), MouseError> {
    println!("Scyrox Mouse Configuration Reader");
    println!("==================================\n");

    // Open connection to mouse
    let mut mouse = match Mouse::open() {
        Ok(m) => m,
        Err(MouseError::NotFound { vid, pids }) => {
            println!("Mouse not found!");
            println!("  Looking for VID: 0x{:04x}", vid);
            println!("  Looking for PIDs: {:?}", pids);
            println!(
                "\nMake sure the mouse is connected and you have permission to access USB devices."
            );
            return Ok(());
        }
        Err(e) => return Err(e),
    };

    println!("Connection: {}\n", mouse.connection_mode());

    // Read firmware info
    println!("=== Firmware ===\n");
    match mouse.get_firmware_info() {
        Ok(firmware) => {
            println!("  Mouse:    {}", firmware.mouse_version);
            match (mouse.connection_mode(), firmware.receiver_version) {
                (ConnectionMode::Wireless, Some(ref v)) => println!("  Receiver: {}", v),
                (ConnectionMode::Wireless, None) => println!("  Receiver: Unknown"),
                (ConnectionMode::Wired, _) => println!("  Receiver: N/A (wired mode)"),
            }
        }
        Err(e) => println!("  Error reading firmware: {}", e),
    }

    // Read battery status
    println!("\n=== Battery ===\n");
    match mouse.get_battery() {
        Ok(battery) => {
            println!("  Voltage:    {} mV", battery.voltage_mv);
            println!("  Percentage: {}%", battery.percentage);
        }
        Err(e) => println!("  Error reading battery: {}", e),
    }

    // Read configuration
    println!("\n=== Configuration ===\n");
    match mouse.get_config() {
        Ok(config) => {
            println!("  Polling Rate:      {}", config.polling_rate);
            println!("  Lift-Off Distance: {}", config.lift_off_distance);
            println!(
                "  Sleep Timeout:     {} seconds",
                config.sleep_timeout_seconds
            );
            println!(
                "  Angle Snapping:    {}",
                if config.angle_snapping { "On" } else { "Off" }
            );
            println!(
                "  Ripple Control:    {}",
                if config.ripple_control { "On" } else { "Off" }
            );
            println!(
                "  High Speed Mode:   {}",
                if config.high_speed_mode { "On" } else { "Off" }
            );
            println!(
                "  Long Distance:     {}",
                if config.long_distance_mode {
                    "On"
                } else {
                    "Off"
                }
            );
        }
        Err(e) => println!("  Error reading configuration: {}", e),
    }

    Ok(())
}
