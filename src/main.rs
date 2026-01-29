use nusb::transfer::{Buffer, ControlOut, ControlType, In, Interrupt, Recipient};
use nusb::{Endpoint, MaybeFuture};
use std::time::Duration;

// You'll need to find these for your specific mouse
// Use `lsusb` to find Vendor ID and Product ID
const VENDOR_ID: u16 = 0x3554; // TODO: Replace with your mouse's VID
// Supported Product IDs (preferred first)
const PRODUCT_IDS: [u16; 2] = [0xf5f6, 0xf5f7];

const INTERFACE_NUM: u8 = 1; // Configuration interface (may need adjustment)
const INTERRUPT_EP_IN: u8 = 0x82; // Interrupt endpoint for responses

const PID_WIRED: u16 = 0xf5f6;
const PID_WIRELESS: u16 = 0xf5f7;

// Max packet sizes for endpoint 0x82 (from USB descriptors)
const PACKET_SIZE_WIRED: usize = 64;
const PACKET_SIZE_WIRELESS: usize = 49;

#[derive(Debug, Clone, Copy, PartialEq)]
enum ConnectionMode {
    Wired,    // PID 0xf5f6
    Wireless, // PID 0xf5f7
}

impl ConnectionMode {
    fn packet_size(self) -> usize {
        match self {
            ConnectionMode::Wired => PACKET_SIZE_WIRED,
            ConnectionMode::Wireless => PACKET_SIZE_WIRELESS,
        }
    }
}

/// Calculate checksum for command packet
/// The checksum appears to make the sum of all bytes equal a constant
fn calculate_checksum(data: &[u8]) -> u8 {
    let sum: u8 = data.iter().fold(0u8, |acc, &x| acc.wrapping_add(x));
    0x55u8.wrapping_sub(sum)
}

/// Build a command packet (17 bytes: 16 data + checksum)
fn build_command(cmd: u8, subcmd: u8, offset: u16, length: u8) -> [u8; 17] {
    let mut packet = [0u8; 17];
    packet[0] = 0x08;
    packet[1] = cmd;
    packet[2] = subcmd;
    packet[3] = (offset >> 8) as u8; // Offset HIGH byte
    packet[4] = offset as u8; // Offset LOW byte
    packet[5] = length;
    // Bytes 6-15 are zero
    packet[16] = calculate_checksum(&packet[0..16]);
    packet
}

/// Build memory read command (cmd 0x08)
fn build_memory_read(offset: u16, length: u8) -> [u8; 17] {
    build_command(0x08, 0x00, offset, length)
}

/// Build device info command (cmd 0x01)
fn build_device_info_cmd(offset: u16, length: u8) -> [u8; 17] {
    build_command(0x01, 0x00, offset, length)
}

/// Build firmware version command (cmd 0x1d)
fn build_firmware_cmd() -> [u8; 17] {
    build_command(0x1d, 0x00, 0, 0)
}

/// Build status command (cmd 0x03)
fn build_status_cmd() -> [u8; 17] {
    build_command(0x03, 0x00, 0, 0)
}

/// Build battery status command (cmd 0x04)
fn build_battery_cmd() -> [u8; 17] {
    build_command(0x04, 0x00, 0, 0)
}

/// Build wireless status command (cmd 0x17) - wireless only
fn build_wireless_status_cmd() -> [u8; 17] {
    build_command(0x17, 0x00, 0, 0)
}

/// Build config flags command (cmd 0x02)
fn build_config_flags_cmd(param: u16) -> [u8; 17] {
    build_command(0x02, 0x00, param, 0)
}

// Battery voltage to percentage conversion
// Based on observed data:
//   4088mV => 86%
//   3967mV => 58%
//   3808mV => 41%
//   3804mV => 40%
//   3800mV => 39%
//   3766mV => 35%
//   3750mV => 32%
//   3679mV => 27%
// Typical Li-ion: ~3000mV = 0%, ~4200mV = 100%
fn voltage_to_percentage(voltage_mv: u16) -> u8 {
    let x = voltage_mv as f32;
    let percent = 0.000034 * x * x - 0.1496 * x + 116.4;
    let percent = percent.round() as i32;
    if percent < 0 {
        0
    } else if percent > 100 {
        100
    } else {
        percent as u8
    }
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

fn parse_firmware_response(data: &[u8], mode: ConnectionMode) -> String {
    if data.len() < 8 || data[0] != 0x08 || data[1] != 0x1d {
        return "Unknown".to_string();
    }
    match mode {
        ConnectionMode::Wireless => {
            // Wireless: version at bytes 6-7 (e.g., 0x02, 0x16 -> "v2.22")
            format!("v{}.{}", data[6], data[7])
        }
        ConnectionMode::Wired => {
            // Wired: byte 2 = 0x01 indicates ready, no version info
            if data[2] == 0x01 {
                "N/A (wired mode)".to_string()
            } else {
                "Unknown".to_string()
            }
        }
    }
}

/// Poll firmware/ready status - wired needs 5 polls, wireless needs 1
fn poll_device_ready(
    endpoint: &mut Endpoint<Interrupt, In>,
    interface: &nusb::Interface,
    mode: ConnectionMode,
) -> Result<Vec<u8>, Box<dyn std::error::Error>> {
    let cmd = build_firmware_cmd();
    let poll_count = match mode {
        ConnectionMode::Wired => 1,    // Wired: 5 polls (from dump)
        ConnectionMode::Wireless => 1, // Wireless: single poll
    };

    let mut last_response = Vec::new();
    for i in 0..poll_count {
        let desc = if poll_count > 1 {
            format!("Ready Poll {}/{}", i + 1, poll_count)
        } else {
            "Firmware Version".to_string()
        };
        last_response = send_and_receive(endpoint, interface, &cmd, &desc, mode)?;
    }
    Ok(last_response)
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

    if VENDOR_ID == 0 || PRODUCT_IDS.iter().all(|&pid| pid == 0) {
        println!("ERROR: Please set VENDOR_ID and PRODUCT_IDS constants to match your mouse.");
        println!("       Look for your mouse in the device list above.");
        return Ok(());
    }

    // Find and open the mouse (prefer earlier PIDs in the list)
    let (device_info, mode) = PRODUCT_IDS
        .iter()
        .find_map(|&pid| {
            let device = nusb::list_devices()
                .wait()
                .ok()?
                .find(|d| d.vendor_id() == VENDOR_ID && d.product_id() == pid)?;
            let mode = match pid {
                PID_WIRED => ConnectionMode::Wired,
                PID_WIRELESS => ConnectionMode::Wireless,
                _ => ConnectionMode::Wireless, // Default fallback
            };
            Some((device, mode))
        })
        .ok_or("Mouse not found")?;

    let mode_str = match mode {
        ConnectionMode::Wired => "Wired (USB)",
        ConnectionMode::Wireless => "Wireless (2.4GHz)",
    };

    println!(
        "Found mouse: {} {}",
        device_info.manufacturer_string().unwrap_or("Unknown"),
        device_info.product_string().unwrap_or("Unknown")
    );
    println!("Connection: {}", mode_str);

    let device = device_info.open().wait()?;

    // Detach kernel driver and claim interface
    let interface = device.detach_and_claim_interface(INTERFACE_NUM).wait()?;

    println!("Interface claimed successfully\n");

    // Create endpoint once and reuse for all commands
    let mut endpoint = interface.endpoint::<Interrupt, In>(INTERRUPT_EP_IN)?;

    // Read initial device information
    println!("=== Device Information ===\n");

    // Command 0x01 - Device info/serial
    send_and_receive(
        &mut endpoint,
        &interface,
        &build_device_info_cmd(0x0008, 0x28),
        "Device Info",
        mode,
    )?;

    // Command 0x1d - Firmware version / ready poll
    let fw_response = poll_device_ready(&mut endpoint, &interface, mode)?;
    let fw_version = parse_firmware_response(&fw_response, mode);
    println!("    Firmware: {}", fw_version);

    // Command 0x03 - Status
    send_and_receive(
        &mut endpoint,
        &interface,
        &build_status_cmd(),
        "Status",
        mode,
    )?;

    // Command 0x02 - Config flags
    send_and_receive(
        &mut endpoint,
        &interface,
        &build_config_flags_cmd(0x0101),
        "Config Flags",
        mode,
    )?;

    // Read another device info
    send_and_receive(
        &mut endpoint,
        &interface,
        &build_device_info_cmd(0x0008, 0x00),
        "Device Info 2",
        mode,
    )?;

    // Now read the full configuration memory
    println!("\n=== Configuration Memory Dump ===\n");

    let mut config_data = Vec::new();
    let chunk_size = 10u8;
    let total_size = 0xE0u16; // ~224 bytes, covers the observed range

    for offset in (0..total_size).step_by(chunk_size as usize) {
        let cmd = build_memory_read(offset, chunk_size);

        match send_and_receive(
            &mut endpoint,
            &interface,
            &cmd,
            &format!("Memory 0x{:04X}", offset),
            mode,
        ) {
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

    // Query battery status
    println!("\n=== Battery Status ===\n");
    send_and_receive(
        &mut endpoint,
        &interface,
        &build_battery_cmd(),
        "Battery",
        mode,
    )?;

    // Wireless-only: Query wireless status (0x17)
    if mode == ConnectionMode::Wireless {
        println!("\n=== Wireless Status ===\n");
        send_and_receive(
            &mut endpoint,
            &interface,
            &build_wireless_status_cmd(),
            "Wireless Status",
            mode,
        )?;
    }

    Ok(())
}

/// Send a command and receive the response
fn send_and_receive(
    endpoint: &mut Endpoint<Interrupt, In>,
    interface: &nusb::Interface,
    cmd: &[u8; 17],
    description: &str,
    mode: ConnectionMode,
) -> Result<Vec<u8>, Box<dyn std::error::Error>> {
    println!(">>> {} ", description);
    println!("    TX: {}", hex::encode(cmd));

    // Set up read buffer BEFORE sending command
    // This ensures we're listening when the device responds
    // Use mode-specific packet size from USB descriptors
    let packet_size = mode.packet_size();
    let mut response = vec![0u8; packet_size];

    let buf = Buffer::new(packet_size);
    endpoint.submit(buf);

    // Now send command via control transfer
    let result = interface
        .control_out(
            ControlOut {
                control_type: ControlType::Class,
                recipient: Recipient::Interface,
                request: 0x09, // SET_REPORT
                value: 0x0208, // Report Type (Output=2) | Report ID (0x08)
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
                    value: 0x0208,
                    index: INTERFACE_NUM as u16,
                    data: cmd,
                },
                Duration::from_millis(100),
            )
            .wait()?;
    }

    // Wait for response on interrupt endpoint
    // Try reading multiple packets in case there's an ACK followed by data
    let mut bytes_read = 0;
    for i in 0..3 {
        // Submit another buffer for next read
        if i > 0 {
            let buf = Buffer::new(packet_size);
            endpoint.submit(buf);
        }

        match endpoint.wait_next_complete(Duration::from_millis(100)) {
            Some(completion) => {
                if completion.status.is_ok() {
                    let len = completion.buffer.len();
                    println!(
                        "    RX[{}]: {} ({} bytes)",
                        i,
                        hex::encode(&*completion.buffer),
                        len
                    );
                    // Use the last successful response
                    if len > 0 {
                        let copy_len = len.min(response.len());
                        response[..copy_len].copy_from_slice(&completion.buffer[..copy_len]);
                        bytes_read = copy_len;
                    }
                } else {
                    println!("    RX[{}] error: {:?}", i, completion.status);
                    break;
                }
            }
            None => {
                if i == 0 {
                    println!("    EP timeout");
                }
                break;
            }
        }
    }
    let bytes_read = bytes_read;

    if bytes_read > 0 {
        // Check for battery status packet (0x08 0x04)
        if let Some(battery) = parse_battery_packet(&response[..bytes_read]) {
            println!(
                "    Battery: {} mV ({}%)",
                battery.voltage_mv, battery.percentage
            );
        }
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
