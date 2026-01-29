use nusb::transfer::{Buffer, ControlOut, ControlType, In, Interrupt, Recipient};
use nusb::MaybeFuture;
use std::time::Duration;

// You'll need to find these for your specific mouse
// Use `lsusb` to find Vendor ID and Product ID
const VENDOR_ID: u16 = 0x3554; // TODO: Replace with your mouse's VID
const PRODUCT_ID: u16 = 0xf5f7; // TODO: Replace with your mouse's PID

const INTERFACE_NUM: u8 = 1; // Configuration interface (may need adjustment)
const INTERRUPT_EP_IN: u8 = 0x82; // Interrupt endpoint for responses

/// Calculate checksum for command packet
/// The checksum appears to make the sum of all bytes equal a constant
fn calculate_checksum(data: &[u8]) -> u8 {
    let sum: u8 = data.iter().fold(0u8, |acc, &x| acc.wrapping_add(x));
    0u8.wrapping_sub(sum)
}

/// Build a command packet
fn build_command(cmd: u8, subcmd: u8, offset: u16, length: u8) -> [u8; 16] {
    let mut packet = [0u8; 16];
    packet[0] = 0x08;
    packet[1] = cmd;
    packet[2] = subcmd;
    packet[3] = 0x00;
    packet[4] = (offset >> 8) as u8; // Offset high byte
    packet[5] = offset as u8; // Offset low byte
    packet[6] = length;
    // Bytes 7-14 are zero
    packet[15] = calculate_checksum(&packet[0..15]);
    packet
}

/// Build memory read command (cmd 0x08)
fn build_memory_read(offset: u16, length: u8) -> [u8; 16] {
    build_command(0x08, 0x00, offset, length)
}

/// Build device info command (cmd 0x01)
fn build_device_info_cmd(offset: u16, length: u8) -> [u8; 16] {
    build_command(0x01, 0x00, offset, length)
}

/// Build firmware version command (cmd 0x1d)
fn build_firmware_cmd() -> [u8; 16] {
    build_command(0x1d, 0x00, 0, 0)
}

/// Build status command (cmd 0x03)
fn build_status_cmd() -> [u8; 16] {
    build_command(0x03, 0x00, 0, 0)
}

/// Build config flags command (cmd 0x02)
fn build_config_flags_cmd(param: u16) -> [u8; 16] {
    build_command(0x02, 0x00, param, 0)
}

// Battery voltage to percentage conversion
// Based on observed data:
//   3800mV ≈ 39%
//   3804mV ≈ 40%
//   3808mV ≈ 41%
// Typical Li-ion: ~3000mV = 0%, ~4200mV = 100%
fn voltage_to_percentage(voltage_mv: u16) -> u8 {
    const MIN_VOLTAGE: u16 = 3000; // 0%
    const MAX_VOLTAGE: u16 = 4200; // 100%

    if voltage_mv <= MIN_VOLTAGE {
        return 0;
    }
    if voltage_mv >= MAX_VOLTAGE {
        return 100;
    }

    ((voltage_mv - MIN_VOLTAGE) as u32 * 100 / (MAX_VOLTAGE - MIN_VOLTAGE) as u32) as u8
}

#[derive(Debug)]
struct BatteryStatus {
    voltage_mv: u16,
    percentage: u8,
}

fn parse_battery_packet(data: &[u8]) -> Option<BatteryStatus> {
    if data.len() >= 10 && data[0] == 0x08 && data[1] == 0x04 {
        // Bytes 8-9: battery voltage in mV (big-endian)
        let voltage_mv = u16::from_be_bytes([data[8], data[9]]);
        let percentage = voltage_to_percentage(voltage_mv);
        Some(BatteryStatus {
            voltage_mv,
            percentage,
        })
    } else {
        None
    }
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("Mouse Configuration Reader");
    println!("==========================\n");

    // List all USB devices to help find the mouse
    println!("Available USB devices:");
    for device in nusb::list_devices().wait()? {
        println!(
            "  {:04x}:{:04x} - {} {}",
            device.vendor_id(),
            device.product_id(),
            device.manufacturer_string().unwrap_or("Unknown"),
            device.product_string().unwrap_or("Unknown")
        );
    }
    println!();

    if VENDOR_ID == 0 || PRODUCT_ID == 0 {
        println!("ERROR: Please set VENDOR_ID and PRODUCT_ID constants to match your mouse.");
        println!("       Look for your mouse in the device list above.");
        return Ok(());
    }

    // Find and open the mouse
    let device_info = nusb::list_devices()
        .wait()?
        .find(|d| d.vendor_id() == VENDOR_ID && d.product_id() == PRODUCT_ID)
        .ok_or("Mouse not found")?;

    println!(
        "Found mouse: {} {}",
        device_info.manufacturer_string().unwrap_or("Unknown"),
        device_info.product_string().unwrap_or("Unknown")
    );

    let device = device_info.open().wait()?;

    // Detach kernel driver and claim interface
    let interface = device.detach_and_claim_interface(INTERFACE_NUM).wait()?;

    println!("Interface claimed successfully\n");

    // Read initial device information
    println!("=== Device Information ===\n");

    // Command 0x01 - Device info/serial
    send_and_receive(
        &interface,
        &build_device_info_cmd(0x0008, 0x28),
        "Device Info",
    )?;

    // Command 0x1d - Firmware version
    send_and_receive(&interface, &build_firmware_cmd(), "Firmware Version")?;

    // Command 0x03 - Status
    send_and_receive(&interface, &build_status_cmd(), "Status")?;

    // Command 0x02 - Config flags
    send_and_receive(&interface, &build_config_flags_cmd(0x0101), "Config Flags")?;

    // Read another device info
    send_and_receive(
        &interface,
        &build_device_info_cmd(0x0008, 0x00),
        "Device Info 2",
    )?;

    // Now read the full configuration memory
    println!("\n=== Configuration Memory Dump ===\n");

    let mut config_data = Vec::new();
    let chunk_size = 10u8;
    let total_size = 0xE0u16; // ~224 bytes, covers the observed range

    for offset in (0..total_size).step_by(chunk_size as usize) {
        let cmd = build_memory_read(offset, chunk_size);

        match send_and_receive(&interface, &cmd, &format!("Memory 0x{:04X}", offset)) {
            Ok(response) => {
                // Extract data bytes (skip header, take 'length' bytes)
                if response.len() >= 16 {
                    let data_start = 6;
                    let data_end = data_start + chunk_size as usize;
                    if data_end <= response.len() {
                        config_data.extend_from_slice(&response[data_start..data_end]);
                    }
                }
            }
            Err(e) => {
                println!("  Error reading offset 0x{:04X}: {}", offset, e);
                break;
            }
        }
    }

    // Print the full configuration dump
    println!(
        "\n=== Full Configuration Data ({} bytes) ===\n",
        config_data.len()
    );
    print_hex_dump(&config_data);

    Ok(())
}

/// Send a command and receive the response
fn send_and_receive(
    interface: &nusb::Interface,
    cmd: &[u8; 16],
    description: &str,
) -> Result<Vec<u8>, Box<dyn std::error::Error>> {
    println!(">>> {} ", description);
    println!("    TX: {}", hex::encode(cmd));

    // Send command via control transfer
    // Based on the capture, this appears to be a vendor-specific control transfer
    // You may need to adjust these parameters based on your mouse
    let result = interface
        .control_out(
            ControlOut {
                control_type: ControlType::Class,
                recipient: Recipient::Interface,
                request: 0x09, // SET_REPORT
                value: 0x0308, // Report Type (Feature=3) | Report ID (0x08)
                index: INTERFACE_NUM as u16,
                data: cmd,
            },
            Duration::from_millis(100),
        )
        .wait();

    if result.is_err() {
        // Try alternative: vendor-specific transfer
        interface
            .control_out(
                ControlOut {
                    control_type: ControlType::Vendor,
                    recipient: Recipient::Interface,
                    request: 0x09,
                    value: 0x0308,
                    index: INTERFACE_NUM as u16,
                    data: cmd,
                },
                Duration::from_millis(100),
            )
            .wait()?;
    }

    // Small delay to allow device to process
    std::thread::sleep(Duration::from_millis(5));

    // Read response from interrupt endpoint
    let mut response = [0u8; 64];
    let mut endpoint = interface.endpoint::<Interrupt, In>(INTERRUPT_EP_IN)?;

    const PACKET_SIZE: usize = 64;
    let mut buf = Buffer::new(PACKET_SIZE);
    buf.set_requested_len(PACKET_SIZE);
    endpoint.submit(buf);

    let bytes_read = match endpoint.wait_next_complete(Duration::from_millis(100)) {
        Some(completion) if completion.status.is_ok() => {
            let len = completion.buffer.len();
            response[..len].copy_from_slice(&completion.buffer);
            len
        }
        _ => 0,
    };

    if bytes_read > 0 {
        println!("    RX: {}", hex::encode(&response[..bytes_read]));

        // Check for battery status packet (0x08 0x04)
        if let Some(battery) = parse_battery_packet(&response[..bytes_read]) {
            println!(
                "    Battery: {} mV ({}%)",
                battery.voltage_mv, battery.percentage
            );
        }
    } else {
        println!("    RX: (no response)");
    }

    Ok(response[..bytes_read].to_vec())
}

/// Print a hex dump of data
fn print_hex_dump(data: &[u8]) {
    for (i, chunk) in data.chunks(16).enumerate() {
        let offset = i * 16;
        print!("{:04X}: ", offset);

        for (j, byte) in chunk.iter().enumerate() {
            print!("{:02X} ", byte);
            if j == 7 {
                print!(" ");
            }
        }

        // Padding for incomplete lines
        for j in chunk.len()..16 {
            print!("   ");
            if j == 7 {
                print!(" ");
            }
        }

        print!(" |");
        for byte in chunk {
            if *byte >= 0x20 && *byte <= 0x7E {
                print!("{}", *byte as char);
            } else {
                print!(".");
            }
        }
        println!("|");
    }
}
