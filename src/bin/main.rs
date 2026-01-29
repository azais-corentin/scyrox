use nusb::transfer::{Buffer, ControlOut, ControlType, In, Interrupt, Recipient};
use nusb::MaybeFuture;
use std::time::{Duration, Instant};

const VID: u16 = 0x3554; // Compx
const PID: u16 = 0xf5f7; // SCYROX 8K Dongle

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

fn print_packet(data: &[u8]) {
    if data.len() < 2 {
        return;
    }

    let report_id = data[0];
    let report_type = data[1];

    match (report_id, report_type) {
        (0x08, 0x04) => {
            if let Some(battery) = parse_battery_packet(data) {
                println!("┌─────────────────────────────────────┐");
                println!("│         BATTERY STATUS              │");
                println!("├─────────────────────────────────────┤");
                println!(
                    "│  Voltage: {:>5} mV                  │",
                    battery.voltage_mv
                );
                println!(
                    "│  Level:   {:>5} %                   │",
                    battery.percentage
                );
                println!("└─────────────────────────────────────┘");
            }
        }
        (0x08, 0x01) => println!("[0x0801] Sensor/Status packet"),
        (0x08, 0x02) => println!("[0x0802] Config packet"),
        (0x08, 0x03) => println!("[0x0803] Device info packet"),
        (0x08, 0x08) => {} // Data/color table packets - silent
        (0x08, 0x0e) => println!("[0x080e] Status packet"),
        (0x08, 0x12) => println!("[0x0812] Config packet"),
        (0x08, 0x17) => println!("[0x0817] Final packet"),
        (0x08, 0x1d) => println!("[0x081d] Info packet"),
        _ => println!(
            "[0x{:02x}{:02x}] Packet ({} bytes)",
            report_id,
            report_type,
            data.len()
        ),
    }
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Find the device
    let device_info = nusb::list_devices()
        .wait()?
        .find(|dev| dev.vendor_id() == VID && dev.product_id() == PID)
        .ok_or("Device not found. Is the mouse dongle connected?")?;

    println!(
        "Found device: {} {:04x}:{:04x}",
        device_info.product_string().unwrap_or("Unknown"),
        device_info.vendor_id(),
        device_info.product_id()
    );

    // Open the device
    let device = device_info.open().wait()?;
    println!("Device opened successfully");

    // Claim interface 1 (as indicated by wIndex in the control transfer)
    // Use detach_and_claim_interface to detach kernel driver (e.g., usbhid) first
    let interface = device.detach_and_claim_interface(1).wait()?;
    println!("Claimed interface 1");

    // Control transfer parameters from Wireshark capture:
    // bmRequestType: 0x21 (Host→Device, Class, Interface)
    // bRequest: 9 (HID SET_REPORT)
    // wValue: 0x0208 (Report Type: Output (0x02), Report ID: 0x08)
    // wIndex: 1 (Interface 1)
    let data: [u8; 17] = [
        0x08, 0x01, 0x00, 0x00, 0x00, 0x08, 0x3b, 0x58, 0x4c, 0x94, 0x00, 0x00, 0x00, 0x00, 0x00,
        0x00, 0xd1,
    ];

    let control_out = ControlOut {
        control_type: ControlType::Class,
        recipient: Recipient::Interface,
        request: 0x09, // HID SET_REPORT
        value: 0x0208, // Report Type: Output, Report ID: 0x08
        index: 1,      // Interface 1
        data: &data,
    };

    // Send the control transfer
    match interface
        .control_out(control_out, Duration::from_millis(100))
        .wait()
    {
        Ok(()) => {
            println!("Control transfer sent successfully ({} bytes)", data.len());
        }
        Err(e) => {
            eprintln!("Control transfer failed: {:?}", e);
            return Ok(());
        }
    }

    println!("\nReading response packets...\n");

    // Read responses from interrupt endpoint
    // Interface 1 typically uses endpoint 0x82 (IN endpoint 2)
    // Adjust if your device uses a different endpoint
    let endpoint_addr: u8 = 0x82;

    // Open the interrupt IN endpoint
    let mut endpoint = interface.endpoint::<Interrupt, In>(endpoint_addr)?;

    // Submit initial buffers for reading
    const PACKET_SIZE: usize = 17;
    const NUM_BUFFERS: usize = 4;

    for _ in 0..NUM_BUFFERS {
        let mut buf = Buffer::new(PACKET_SIZE);
        buf.set_requested_len(PACKET_SIZE);
        endpoint.submit(buf);
    }

    // Read packets until timeout
    let timeout = Duration::from_millis(1000);
    let start = Instant::now();
    let mut packets_received = 0;
    let mut battery_status: Option<BatteryStatus> = None;

    while start.elapsed() < timeout && endpoint.pending() > 0 {
        // Wait for completed transfers with a small timeout
        match endpoint.wait_next_complete(Duration::from_millis(10)) {
            Some(completion) => {
                if completion.status.is_ok() {
                    let pkt_data: &[u8] = &completion.buffer;
                    if !pkt_data.is_empty() {
                        packets_received += 1;

                        // Check for battery packet
                        if let Some(status) = parse_battery_packet(pkt_data) {
                            battery_status = Some(status);
                        }

                        print_packet(pkt_data);
                    }
                }
                // Resubmit buffer
                let mut buf = Buffer::new(PACKET_SIZE);
                buf.set_requested_len(PACKET_SIZE);
                endpoint.submit(buf);
            }
            None => {
                // Timeout, continue loop
            }
        }
    }

    println!("\n═══════════════════════════════════════");
    println!("Received {} packets total", packets_received);

    if let Some(status) = battery_status {
        println!("\nFinal Battery Status:");
        println!("  Voltage: {} mV", status.voltage_mv);
        println!("  Level:   {}%", status.percentage);
    } else {
        println!("\nWarning: Battery status packet (0x0804) not found!");
        println!("Try adjusting the endpoint address (currently 0x82)");
    }

    Ok(())
}
