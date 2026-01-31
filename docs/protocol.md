# Scyrox Mouse HID Communication Protocol Specification

## Document Purpose

This document provides a complete technical specification of the HID communication protocol used by Scyrox gaming mice. It is intended to enable implementation of a driver in any programming language.

---

## 0. Table of contents

- [Scyrox Mouse HID Communication Protocol Specification](#scyrox-mouse-hid-communication-protocol-specification)
  - [Document Purpose](#document-purpose)
  - [0. Table of contents](#0-table-of-contents)
  - [1. Device Identification](#1-device-identification)
    - [USB Identifiers](#usb-identifiers)
    - [Device Variants](#device-variants)
    - [Connection Type Detection](#connection-type-detection)
    - [Dual Connection Behavior](#dual-connection-behavior)
  - [2. HID Report Configuration](#2-hid-report-configuration)
    - [Report Parameters](#report-parameters)
    - [HID Collection Selection](#hid-collection-selection)
  - [3. Packet Structure](#3-packet-structure)
    - [Output Packet Format (Host → Device)](#output-packet-format-host--device)
    - [Input Packet Format (Device → Host)](#input-packet-format-device--host)
    - [Checksum Calculation](#checksum-calculation)
  - [4. Command Reference](#4-command-reference)
    - [Command ID Table](#command-id-table)
  - [5. Command Details](#5-command-details)
    - [5.1 EncryptionData (0x01) - Handshake](#51-encryptiondata-0x01---handshake)
    - [5.2 PCDriverStatus (0x02) - Driver Connection Status](#52-pcdriverstatus-0x02---driver-connection-status)
    - [5.3 DeviceOnLine (0x03) - Connection Check](#53-deviceonline-0x03---connection-check)
    - [5.4 BatteryLevel (0x04) - Battery Status](#54-batterylevel-0x04---battery-status)
    - [5.5 DongleEnterPair (0x05) - Enter Pairing Mode](#55-dongleenterpair-0x05---enter-pairing-mode)
    - [5.6 GetPairState (0x06) - Pairing Status](#56-getpairstate-0x06---pairing-status)
    - [5.7 WriteFlashData (0x07) - Write to Flash](#57-writeflashdata-0x07---write-to-flash)
    - [5.8 ReadFlashData (0x08) - Read from Flash](#58-readflashdata-0x08---read-from-flash)
    - [5.9 ClearSetting (0x09) - Factory Reset](#59-clearsetting-0x09---factory-reset)
    - [5.10 StatusChanged (0x0A) - Change Notification](#510-statuschanged-0x0a---change-notification)
    - [5.11 GetCurrentConfig (0x0E) - Get Profile](#511-getcurrentconfig-0x0e---get-profile)
    - [5.12 SetCurrentConfig (0x0F) - Set Profile](#512-setcurrentconfig-0x0f---set-profile)
    - [5.13 ReadVersionID (0x12) - Mouse Firmware Version](#513-readversionid-0x12---mouse-firmware-version)
    - [5.14 SetLongRangeMode (0x16) - Long Range Mode](#514-setlongrangemode-0x16---long-range-mode)
    - [5.15 GetLongRangeMode (0x17) - Query Long Range Mode](#515-getlongrangemode-0x17---query-long-range-mode)
    - [5.16 GetDongleVersion (0x1D) - Dongle Firmware Version](#516-getdongleversion-0x1d---dongle-firmware-version)
  - [6. Flash Memory Map](#6-flash-memory-map)
    - [6.1 Memory Layout Overview](#61-memory-layout-overview)
    - [6.2 Report Rate (Address 0x0000)](#62-report-rate-address-0x0000)
    - [6.3 DPI Count (Address 0x0002)](#63-dpi-count-address-0x0002)
    - [6.4 Current DPI Index (Address 0x0004)](#64-current-dpi-index-address-0x0004)
    - [6.5 20K Sensor Mode (Address 0x0008)](#65-20k-sensor-mode-address-0x0008)
    - [6.6 LOD - Lift-off Distance (Address 0x000A)](#66-lod---lift-off-distance-address-0x000a)
    - [6.7 DPI Values (Address 0x000C-0x002B)](#67-dpi-values-address-0x000c-0x002b)
    - [6.8 DPI Colors (Address 0x002C-0x004B)](#68-dpi-colors-address-0x002c-0x004b)
    - [6.9 DPI Effect Settings (Addresses 0x004C-0x0053)](#69-dpi-effect-settings-addresses-0x004c-0x0053)
    - [6.10 Key Functions (Address 0x0060-0x007F)](#610-key-functions-address-0x0060-0x007f)
    - [6.11 Light Settings (Address 0x00A0-0x00A7)](#611-light-settings-address-0x00a0-0x00a7)
    - [6.12 Sensor Settings](#612-sensor-settings)
    - [6.13 Shortcut Keys (Address 0x0100-0x01FF)](#613-shortcut-keys-address-0x0100-0x01ff)
    - [6.14 Macros (Address 0x0300-0x0BFF)](#614-macros-address-0x0300-0x0bff)
  - [7. HID Keyboard Scan Codes](#7-hid-keyboard-scan-codes)
  - [8. Communication Sequence](#8-communication-sequence)
    - [8.1 Initial Connection](#81-initial-connection)
    - [8.2 Reading Flash Memory](#82-reading-flash-memory)
    - [8.3 Writing Settings](#83-writing-settings)
    - [8.4 Retry Logic](#84-retry-logic)
  - [9. Error Handling](#9-error-handling)
    - [Response Status Codes](#response-status-codes)
    - [Common Error Conditions](#common-error-conditions)
    - [Recommended Timeouts](#recommended-timeouts)
  - [10. Implementation Notes](#10-implementation-notes)
    - [Thread Safety](#thread-safety)
    - [Power Management](#power-management)
    - [Profile Management](#profile-management)
    - [Macro Limitations](#macro-limitations)
  - [Appendix A: Complete Checksum Implementation](#appendix-a-complete-checksum-implementation)
  - [Appendix B: Rust Type Definitions](#appendix-b-rust-type-definitions)
  - [Appendix C: Example Implementation](#appendix-c-example-implementation)

## 1. Device Identification

### USB Identifiers

| Field                          | Value    | Notes                    |
| ------------------------------ | -------- | ------------------------ |
| Vendor ID                      | `0x3554` | Scyrox                   |
| Product ID (Wireless Dongle 1) | `0xF5F7` | 4K wireless dongle       |
| Product ID (Wireless Dongle 2) | `0xF5F4` | Standard wireless dongle |
| Product ID (Wired)             | `0xF5F6` | Direct USB connection    |

### Device Variants

Devices are identified by CID (Company ID) and MID (Model ID) received during handshake:

| CID | MID | Description     |
| --- | --- | --------------- |
| 62  | 1   | Model variant 1 |
| 62  | 2   | Model variant 2 |

### Connection Type Detection

The `type` field in the handshake response indicates connection type:

| Type Value | Connection           | Max Polling Rate |
| ---------- | -------------------- | ---------------- |
| 0          | Wireless (standard)  | 1000 Hz          |
| 1          | Wireless (4K dongle) | 4000 Hz          |
| 2          | Wired (standard)     | 1000 Hz          |
| 3          | Wired (high-speed)   | 8000 Hz          |
| 4          | Wireless (2K dongle) | 2000 Hz          |
| 5          | Wireless (8K dongle) | 8000 Hz          |

### Dual Connection Behavior

When the mouse is connected via both wired and wireless interfaces simultaneously, the wired connection takes priority. The wireless interface becomes inaccessible and the device appears offline (sleeping) through the dongle, while operating normally through the wired connection.

---

## 2. HID Report Configuration

### Report Parameters

| Parameter     | Value      |
| ------------- | ---------- |
| Report ID     | `8` (0x08) |
| Report Length | 16 bytes   |
| Input Report  | 16 bytes   |
| Output Report | 16 bytes   |

### HID Collection Selection

When enumerating HID collections, select the collection where:

- `inputReports.length == 1`
- `outputReports.length == 1`
- `outputReports[0].reportId == 8`

---

## 3. Packet Structure

### Output Packet Format (Host → Device)

All commands sent to the device use this 16-byte structure:

```
Offset  Size  Field
------  ----  -----
0       1     Command ID
1       1     Reserved (always 0x00)
2       1     Address High Byte (for flash operations)
3       1     Address Low Byte (for flash operations)
4       1     Data Length (number of data bytes, max 10)
5-14    10    Data Payload
15      1     Checksum
```

### Input Packet Format (Device → Host)

Responses from the device use this 16-byte structure:

```
Offset  Size  Field
------  ----  -----
0       1     Command ID (echo of sent command)
1       1     Status (0x00 = success, 0x01 = error)
2       1     Address High Byte (for read operations)
3       1     Address Low Byte (for read operations)
4       1     Data Length
5-14    10    Data Payload
15      1     Checksum
```

### Checksum Calculation

The checksum is calculated as follows:

```rust
fn calculate_checksum(packet: &[u8; 16]) -> u8 {
    let sum: u16 = packet[0..15].iter().map(|&b| b as u16).sum();
    let truncated = (sum & 0xFF) as u8;
    0x55u8.wrapping_sub(truncated).wrapping_sub(8) // Subtract report ID
}
```

Alternatively expressed:

```
checksum = (0x55 - (sum of bytes 0-14) - REPORT_ID) & 0xFF
```

Where `REPORT_ID = 8`.

---

## 4. Command Reference

### Command ID Table

| ID   | Name             | Direction     | Description                           |
| ---- | ---------------- | ------------- | ------------------------------------- |
| 0x01 | EncryptionData   | Bidirectional | Device handshake/identification       |
| 0x02 | PCDriverStatus   | Host → Device | Notify device of driver connection    |
| 0x03 | DeviceOnLine     | Bidirectional | Check if mouse is connected to dongle |
| 0x04 | BatteryLevel     | Device → Host | Get battery status                    |
| 0x05 | DongleEnterPair  | Host → Device | Enter pairing mode                    |
| 0x06 | GetPairState     | Bidirectional | Query pairing status                  |
| 0x07 | WriteFlashData   | Host → Device | Write to flash memory                 |
| 0x08 | ReadFlashData    | Bidirectional | Read from flash memory                |
| 0x09 | ClearSetting     | Host → Device | Factory reset                         |
| 0x0A | StatusChanged    | Device → Host | Configuration change notification     |
| 0x0E | GetCurrentConfig | Bidirectional | Get active profile index              |
| 0x0F | SetCurrentConfig | Host → Device | Set active profile                    |
| 0x12 | ReadVersionID    | Bidirectional | Get mouse firmware version            |
| 0x16 | SetLongRangeMode | Host → Device | Enable/disable long range mode        |
| 0x17 | GetLongRangeMode | Bidirectional | Query long range mode status          |
| 0x1D | GetDongleVersion | Bidirectional | Get dongle firmware version           |

---

## 5. Command Details

### 5.1 EncryptionData (0x01) - Handshake

This command initiates communication and retrieves device identification.

**Request:**

```
Byte 0:     0x01 (command)
Byte 1:     0x00
Byte 2-3:   0x00, 0x00
Byte 4:     0x08 (data length)
Byte 5-8:   Random bytes (any values, used as handshake token)
Byte 9-12:  0x00, 0x00, 0x00, 0x00
Byte 13-14: 0x00, 0x00
Byte 15:    Checksum
```

**Response:**

```
Byte 0:     0x01
Byte 1:     0x00 (success)
Byte 5-8:   Echo of random bytes sent
Byte 9:     CID (Company ID)
Byte 10:    MID (Model ID)
Byte 11:    Type (connection type, see section 1)
Byte 15:    Checksum
```

### 5.2 PCDriverStatus (0x02) - Driver Connection Status

Notifies the device that a driver is connected/disconnected.

**Request:**

```
Byte 0:     0x02
Byte 1:     0x00
Byte 4:     0x01 (data length)
Byte 5:     Status (0x01 = connected, 0x00 = disconnected)
Byte 15:    Checksum
```

**Response:** Standard acknowledgment.

### 5.3 DeviceOnLine (0x03) - Connection Check

Checks if the mouse is connected to the wireless dongle.

**Request:**

```
Byte 0:     0x03
Byte 1-14:  0x00
Byte 15:    Checksum
```

**Response:**

```
Byte 0:     0x03
Byte 1:     0x00 (success)
Byte 5:     Online status (0x01 = online, 0x00 = offline)
Byte 6:     Device address byte 2
Byte 7:     Device address byte 1
Byte 8:     Device address byte 0
Byte 15:    Checksum
```

The device address (3 bytes) uniquely identifies the paired mouse.

### 5.4 BatteryLevel (0x04) - Battery Status

Retrieves current battery level and charging status.

**Request:**

```
Byte 0:     0x04
Byte 1-14:  0x00
Byte 15:    Checksum
```

**Response:**

```
Byte 0:     0x04
Byte 1:     0x00 (success)
Byte 5:     Battery level (0-100 percentage)
Byte 6:     Charging status (0x01 = charging, 0x00 = not charging)
Byte 7:     Voltage high byte
Byte 8:     Voltage low byte
Byte 15:    Checksum
```

**Voltage Interpretation:**

```rust
let voltage_mv: u16 = ((response[7] as u16) << 8) | (response[8] as u16);
```

Voltage-to-percentage lookup table (millivolts):

```
3050 → 0%
3420 → 5%
3480 → 10%
3540 → 15%
3600 → 20%
3660 → 25%
3720 → 30%
3760 → 35%
3800 → 40%
3840 → 45%
3880 → 50%
3920 → 55%
3940 → 60%
3960 → 65%
3980 → 70%
4000 → 75%
4020 → 80%
4040 → 85%
4060 → 90%
4080 → 95%
4110 → 100%
```

### 5.5 DongleEnterPair (0x05) - Enter Pairing Mode

Puts the dongle into pairing mode to accept a new mouse.

**Request:**

```
Byte 0:     0x05
Byte 1:     0x00
Byte 2-3:   0x00, 0x00
Byte 4:     0x02 (data length)
Byte 5:     0x00
Byte 6:     0x00
Byte 7:     0x3E (62 decimal - pairing timeout in seconds)
Byte 8-14:  0x00
Byte 15:    Checksum
```

**Response:** Standard acknowledgment. The dongle enters pairing mode.

### 5.6 GetPairState (0x06) - Pairing Status

Queries the current pairing state.

**Request:**

```
Byte 0:     0x06
Byte 1-14:  0x00
Byte 15:    Checksum
```

**Response:**

```
Byte 0:     0x06
Byte 1:     0x00 (success)
Byte 5:     Pairing status
Byte 6:     Time remaining (seconds)
Byte 15:    Checksum
```

**Pairing Status Values:**

| Value | Meaning             |
| ----- | ------------------- |
| 0     | Idle / Not pairing  |
| 1     | Pairing in progress |
| 2     | Pairing failed      |
| 3     | Pairing successful  |

### 5.7 WriteFlashData (0x07) - Write to Flash

Writes data to the mouse's flash memory.

**Request:**

```
Byte 0:     0x07
Byte 1:     0x00
Byte 2:     Address high byte
Byte 3:     Address low byte
Byte 4:     Data length (1-10)
Byte 5-14:  Data bytes
Byte 15:    Checksum
```

**Response:** Standard acknowledgment with echoed address.

**Important:** For single-byte writes that require validation, use the complement checksum format:

```
Byte 4:     0x02 (always 2 bytes)
Byte 5:     Value
Byte 6:     0x55 - Value (complement)
```

### 5.8 ReadFlashData (0x08) - Read from Flash

Reads data from the mouse's flash memory.

**Request:**

```
Byte 0:     0x08
Byte 1:     0x00
Byte 2:     Address high byte
Byte 3:     Address low byte
Byte 4:     Length to read (1-10)
Byte 5-14:  0x00
Byte 15:    Checksum
```

**Response:**

```
Byte 0:     0x08
Byte 1:     0x00 (success)
Byte 2:     Address high byte (echo)
Byte 3:     Address low byte (echo)
Byte 4:     Data length
Byte 5-14:  Data bytes
Byte 15:    Checksum
```

**Response Validation:**
Verify that bytes 0-4 of the response match the request before accepting data.

### 5.9 ClearSetting (0x09) - Factory Reset

Resets the mouse to factory defaults.

**Request:**

```
Byte 0:     0x09
Byte 1-14:  0x00
Byte 15:    Checksum
```

**Response:** Standard acknowledgment. Device will reset and reconnect.

**Note:** After sending this command, wait up to 1200ms (4 × 300ms polls) for the device to complete the reset before re-reading configuration.

### 5.10 StatusChanged (0x0A) - Change Notification

This is an **unsolicited notification** sent by the device when settings change (e.g., DPI button pressed on mouse).

**Response (unsolicited):**

```
Byte 0:     0x0A
Byte 1:     0x00
Byte 5:     Change flags (bitmask)
Byte 15:    Checksum
```

**Change Flag Bitmask:**

| Bit      | Meaning                | Action Required                      |
| -------- | ---------------------- | ------------------------------------ |
| 0 (0x01) | Current DPI changed    | Re-read address 4                    |
| 1 (0x02) | Report rate changed    | Re-read address 0                    |
| 2 (0x04) | Profile changed        | Re-read address via GetCurrentConfig |
| 3 (0x08) | DPI settings changed   | Re-read addresses 12-75              |
| 5 (0x20) | Light settings changed | Re-read address 160+                 |
| 6 (0x40) | Battery status changed | Re-read via BatteryLevel command     |

### 5.11 GetCurrentConfig (0x0E) - Get Profile

Gets the currently active profile index.

**Request:**

```
Byte 0:     0x0E
Byte 1-14:  0x00
Byte 15:    Checksum
```

**Response:**

```
Byte 0:     0x0E
Byte 1:     0x00 (success)
Byte 5:     Profile index (0-3)
Byte 15:    Checksum
```

### 5.12 SetCurrentConfig (0x0F) - Set Profile

Sets the active profile.

**Request:**

```
Byte 0:     0x0F
Byte 1:     0x00
Byte 2-3:   0x00, 0x00
Byte 4:     0x01 (data length)
Byte 5:     Profile index (0-3)
Byte 6-14:  0x00
Byte 15:    Checksum
```

**Response:** Standard acknowledgment.

### 5.13 ReadVersionID (0x12) - Mouse Firmware Version

Gets the mouse's firmware version.

**Request:**

```
Byte 0:     0x12
Byte 1-14:  0x00
Byte 15:    Checksum
```

**Response:**

```
Byte 0:     0x12
Byte 1:     0x00 (success)
Byte 5:     Major version (decimal)
Byte 6:     Minor version (BCD/hex format)
Byte 15:    Checksum
```

**Version String Format:**

```rust
let version = format!("v{}.{:02x}", response[5], response[6]);
// Example: major=2, minor=0x20 → "v2.20"
```

### 5.14 SetLongRangeMode (0x16) - Long Range Mode

Enables or disables long-range wireless mode (increased power consumption).

**Request:**

```
Byte 0:     0x16
Byte 1:     0x00
Byte 2-3:   0x00, 0x00
Byte 4:     0x0A (data length = 10)
Byte 5:     Enable (0x01) or Disable (0x00)
Byte 6-14:  0x00
Byte 15:    Checksum
```

**Response:** Standard acknowledgment.

### 5.15 GetLongRangeMode (0x17) - Query Long Range Mode

Queries the current long-range mode status.

**Request:**

```
Byte 0:     0x17
Byte 1-14:  0x00
Byte 15:    Checksum
```

**Response:**

```
Byte 0:     0x17
Byte 1:     0x00 (success) or 0x01 (not supported)
Byte 5:     Status (0x01 = enabled, 0x00 = disabled)
Byte 15:    Checksum
```

**Note:** If `Byte 1 == 0x01`, the device does not support long-range mode.

### 5.16 GetDongleVersion (0x1D) - Dongle Firmware Version

Gets the wireless dongle's firmware version.

**Request:**

```
Byte 0:     0x1D
Byte 1-14:  0x00
Byte 15:    Checksum
```

**Response:**

```
Byte 0:     0x1D
Byte 1:     0x00 (success)
Byte 5:     Major version (decimal)
Byte 6:     Minor version (BCD/hex format)
Byte 15:    Checksum
```

---

## 6. Flash Memory Map

The mouse stores all configuration in flash memory. Total readable configuration space is 256 bytes for basic settings, with extended areas for shortcuts and macros.

### 6.1 Memory Layout Overview

| Address Range | Size | Description                  |
| ------------- | ---- | ---------------------------- |
| 0x0000-0x0001 | 2    | Report Rate                  |
| 0x0002-0x0003 | 2    | Max DPI Count                |
| 0x0004-0x0005 | 2    | Current DPI Index            |
| 0x0006-0x0007 | 2    | Reserved                     |
| 0x0008-0x0009 | 2    | 20K Sensor Mode              |
| 0x000A-0x000B | 2    | LOD (Lift-off Distance)      |
| 0x000C-0x002B | 32   | DPI Values (8 × 4 bytes)     |
| 0x002C-0x004B | 32   | DPI Colors (8 × 4 bytes)     |
| 0x004C-0x004D | 2    | DPI Effect Mode              |
| 0x004E-0x004F | 2    | DPI Effect Brightness        |
| 0x0050-0x0051 | 2    | DPI Effect Speed             |
| 0x0052-0x0053 | 2    | DPI Effect State             |
| 0x0060-0x007F | 32   | Key Functions (8 × 4 bytes)  |
| 0x00A0-0x00A6 | 7    | Light Settings               |
| 0x00A7        | 1    | Light On/Off State           |
| 0x00A9        | 1    | Debounce Time                |
| 0x00AB        | 1    | Motion Sync                  |
| 0x00AD        | 1    | Sleep Time                   |
| 0x00AF        | 1    | Angle Snapping               |
| 0x00B1        | 1    | Ripple Control               |
| 0x00B3        | 1    | Moving Off Light             |
| 0x00B5        | 1    | Performance State            |
| 0x00B7        | 1    | Performance/Sleep Time Value |
| 0x00B9        | 1    | Sensor Mode                  |
| 0x0100-0x01FF | 256  | Shortcut Keys (8 × 32 bytes) |
| 0x0300-0x0BFF | 3072 | Macros (8 × 384 bytes)       |

### 6.2 Report Rate (Address 0x0000)

Single byte encoding:

| Stored Value | Report Rate |
| ------------ | ----------- |
| 8            | 125 Hz      |
| 4            | 250 Hz      |
| 2            | 500 Hz      |
| 1            | 1000 Hz     |
| 16           | 2000 Hz     |
| 32           | 4000 Hz     |
| 64           | 8000 Hz     |

**Encoding Formula:**

```rust
fn encode_report_rate(hz: u16) -> u8 {
    if hz <= 1000 {
        (1000 / hz) as u8
    } else {
        ((hz / 2000) * 16) as u8
    }
}

fn decode_report_rate(value: u8) -> u16 {
    if value >= 16 {
        (value as u16 / 16) * 2000
    } else {
        1000 / value as u16
    }
}
```

### 6.3 DPI Count (Address 0x0002)

Single byte: Number of active DPI stages (1-8).

### 6.4 Current DPI Index (Address 0x0004)

Single byte: Currently selected DPI stage (0-7).

### 6.5 20K Sensor Mode (Address 0x0008)

Single byte: `0x01` = enabled, `0x00` = disabled.

Enables 20,000 FPS sensor mode (requires Performance Mode enabled).

### 6.6 LOD - Lift-off Distance (Address 0x000A)

Single byte:

| Value | LOD   |
| ----- | ----- |
| 3     | 0.7mm |
| 1     | 1.0mm |
| 2     | 2.0mm |

### 6.7 DPI Values (Address 0x000C-0x002B)

8 DPI stages, 4 bytes each:

```
Offset 0: DPI low byte (value / 50 - 1) & 0xFF
Offset 1: DPI low byte (duplicate)
Offset 2: High bits: ((value / 50 - 1) >> 8) << 2 | ((value / 50 - 1) >> 8) << 6
Offset 3: Checksum of bytes 0-2
```

**Encoding:**

```rust
fn encode_dpi(dpi: u16) -> [u8; 4] {
    let encoded = (dpi / 50) - 1;
    let low = (encoded & 0xFF) as u8;
    let high = ((encoded >> 8) & 0x03) as u8;
    let byte2 = (high << 2) | (high << 6);
    let checksum = low.wrapping_add(low).wrapping_add(byte2);
    let checksum = 0x55u8.wrapping_sub(checksum);
    [low, low, byte2, checksum]
}

fn decode_dpi(bytes: &[u8; 4]) -> u16 {
    let high_bits = ((bytes[2] & 0x0C) >> 2) as u16;
    let value = (bytes[0] as u16) | (high_bits << 8);
    (value + 1) * 50
}
```

**DPI Range:** 50 - 26000, in steps of 50.

### 6.8 DPI Colors (Address 0x002C-0x004B)

8 DPI stages, 4 bytes each:

```
Offset 0: Red (0-255)
Offset 1: Green (0-255)
Offset 2: Blue (0-255)
Offset 3: Reserved/Checksum
```

### 6.9 DPI Effect Settings (Addresses 0x004C-0x0053)

| Address | Description | Values                         |
| ------- | ----------- | ------------------------------ |
| 0x004C  | Effect Mode | 0=Off, 1=Constant, 2=Breathing |
| 0x004E  | Brightness  | See brightness table           |
| 0x0050  | Speed       | 1-10                           |
| 0x0052  | State       | 0=Off, 1=On                    |

**Brightness Encoding:**

| Index | Raw Value |
| ----- | --------- |
| 1     | 16        |
| 2     | 30        |
| 3     | 60        |
| 4     | 90        |
| 5     | 128       |
| 6     | 150       |
| 7     | 180       |
| 8     | 210       |
| 9     | 230       |
| 10    | 255       |

### 6.10 Key Functions (Address 0x0060-0x007F)

8 keys, 4 bytes each:

```
Offset 0: Function type
Offset 1: Parameter high byte
Offset 2: Parameter low byte
Offset 3: Checksum
```

**Function Types:**

| Type | Description        | Parameters                 |
| ---- | ------------------ | -------------------------- |
| 0    | Disabled           | None                       |
| 1    | Mouse Button       | Button code (see below)    |
| 2    | DPI Switch         | Mode code (see below)      |
| 3    | Scroll Wheel       | Direction code             |
| 4    | Fire Key           | Interval, Count            |
| 5    | Shortcut Key       | Reference to shortcut slot |
| 6    | Macro              | Macro slot, Cycle count    |
| 7    | Report Rate Switch | None                       |

**Mouse Button Codes (Type 1):**

| Code   | Button       |
| ------ | ------------ |
| 0x0100 | Left Click   |
| 0x0200 | Right Click  |
| 0x0400 | Middle Click |
| 0x0800 | Back         |
| 0x1000 | Forward      |

**DPI Switch Codes (Type 2):**

| Code   | Action    |
| ------ | --------- |
| 0x0100 | DPI Cycle |
| 0x0200 | DPI Up    |
| 0x0300 | DPI Down  |

**Scroll Codes (Type 3):**

| Code   | Action       |
| ------ | ------------ |
| 0x0100 | Scroll Left  |
| 0x0200 | Scroll Right |

**Fire Key Format (Type 4):**

```
Byte 1: Interval (10-255 ms)
Byte 2: Repeat count (0-3, 0 = hold to repeat)
```

**Macro Format (Type 6):**

```
Byte 1: Macro slot index (0-7)
Byte 2: Cycle count (1-255, 253-255 = special modes)
```

### 6.11 Light Settings (Address 0x00A0-0x00A7)

```
Offset 0 (0xA0): Light mode
Offset 1 (0xA1): Red
Offset 2 (0xA2): Green
Offset 3 (0xA3): Blue
Offset 4 (0xA4): Speed (1-10)
Offset 5 (0xA5): Brightness (0-255)
Offset 6 (0xA6): Reserved
Offset 7 (0xA7): On/Off state (0=off, 1=on)
```

**Light Modes:**

| Value | Mode                   |
| ----- | ---------------------- |
| 0     | Off                    |
| 1     | Color Flow             |
| 2     | Single Color Breathing |
| 3     | Constant Color         |
| 4     | Neon                   |
| 5     | Mixed Color Breathing  |
| 6     | Colorful Constant      |

### 6.12 Sensor Settings

| Address | Setting                | Values                   |
| ------- | ---------------------- | ------------------------ |
| 0x00A9  | Debounce Time          | 0-30 ms                  |
| 0x00AB  | Motion Sync            | 0=off, 1=on              |
| 0x00AD  | Sleep Time             | Same as Performance Time |
| 0x00AF  | Angle Snapping         | 0=off, 1=on              |
| 0x00B1  | Ripple Control         | 0=off, 1=on              |
| 0x00B3  | Moving Off Light Time  | Time value               |
| 0x00B5  | Performance Mode       | 0=off, 1=on              |
| 0x00B7  | Sleep/Performance Time | See table                |
| 0x00B9  | Sensor Mode            | 0=LP, 1=HP               |

**Sleep/Performance Time Values:**

| Value | Time       |
| ----- | ---------- |
| 1     | 10 seconds |
| 3     | 30 seconds |
| 6     | 1 minute   |
| 30    | 5 minutes  |
| 60    | 10 minutes |
| 180   | 30 minutes |

### 6.13 Shortcut Keys (Address 0x0100-0x01FF)

8 shortcut slots, 32 bytes each.

**Structure:**

```
Offset 0: Total event count (key down + key up events)
Offset 1+: Event triplets (3 bytes each)
```

**Event Triplet Format:**

```
Byte 0: Event type
        Bit 7: Key down (0x80)
        Bit 6: Key up (0x40)
        Bits 0-3: Modifier type (0=modifier key, 1=normal key, 2=media key)
Byte 1: Key code low byte
Byte 2: Key code high byte
```

**Modifier Key Codes (type 0):**

| Value | Key         |
| ----- | ----------- |
| 1     | Left Ctrl   |
| 2     | Left Shift  |
| 4     | Left Alt    |
| 8     | Left Win    |
| 16    | Right Ctrl  |
| 32    | Right Shift |
| 64    | Right Alt   |
| 128   | Right Win   |

**Media Key Codes (type 2):**

| Code   | Function       |
| ------ | -------------- |
| 0x0183 | Media Player   |
| 0x00CD | Play/Pause     |
| 0x00B5 | Next Track     |
| 0x00B6 | Previous Track |
| 0x00B7 | Stop           |
| 0x00E2 | Mute           |
| 0x00E9 | Volume Up      |
| 0x00EA | Volume Down    |
| 0x018A | Email          |
| 0x0192 | Calculator     |
| 0x0194 | My Computer    |
| 0x0221 | Search         |
| 0x0223 | Home Page      |
| 0x0224 | Web Back       |
| 0x0225 | Web Forward    |
| 0x0226 | Web Stop       |
| 0x0227 | Refresh        |
| 0x022A | Favorites      |

### 6.14 Macros (Address 0x0300-0x0BFF)

8 macro slots, 384 bytes each.

**Structure:**

```
Offset 0:      Name length (1-30)
Offset 1-30:   Name (ASCII characters)
Offset 31:     Event count (2-70)
Offset 32+:    Events (5 bytes each)
```

**Event Format (5 bytes):**

```
Byte 0: Status and type
        Bits 6-7: Status (1=key down, 2=key up)
        Bits 0-3: Key type (1=keyboard, 4=mouse button)
Byte 1: Key code low byte
Byte 2: Key code high byte
Byte 3: Delay high byte
Byte 4: Delay low byte
```

**Mouse Button Codes (type 4):**

| Code | Button  |
| ---- | ------- |
| 0x01 | Left    |
| 0x02 | Right   |
| 0x04 | Middle  |
| 0x08 | Back    |
| 0x10 | Forward |

**Cycle Count Special Values:**

| Value | Behavior                     |
| ----- | ---------------------------- |
| 1-250 | Repeat N times               |
| 253   | Loop until key pressed again |
| 254   | Loop until key released      |
| 255   | Loop until any key pressed   |

---

## 7. HID Keyboard Scan Codes

For keyboard shortcuts and macros, use USB HID scan codes:

| Key | Code | Key    | Code | Key          | Code |
| --- | ---- | ------ | ---- | ------------ | ---- |
| A   | 4    | N      | 17   | 0            | 39   |
| B   | 5    | O      | 18   | Enter        | 40   |
| C   | 6    | P      | 19   | Escape       | 41   |
| D   | 7    | Q      | 20   | Backspace    | 42   |
| E   | 8    | R      | 21   | Tab          | 43   |
| F   | 9    | S      | 22   | Space        | 44   |
| G   | 10   | T      | 23   | Minus        | 45   |
| H   | 11   | U      | 24   | Equal        | 46   |
| I   | 12   | V      | 25   | LeftBracket  | 47   |
| J   | 13   | W      | 26   | RightBracket | 48   |
| K   | 14   | X      | 27   | Backslash    | 49   |
| L   | 15   | Y      | 28   | Semicolon    | 51   |
| M   | 16   | Z      | 29   | Quote        | 52   |
| 1   | 30   | Comma  | 54   | Backquote    | 53   |
| 2   | 31   | Period | 55   | CapsLock     | 57   |
| 3   | 32   | Slash  | 56   | F1           | 58   |
| 4   | 33   | F2     | 59   | F7           | 64   |
| 5   | 34   | F3     | 60   | F8           | 65   |
| 6   | 35   | F4     | 61   | F9           | 66   |
| 7   | 36   | F5     | 62   | F10          | 67   |
| 8   | 37   | F6     | 63   | F11          | 68   |
| 9   | 38   | F12    | 69   | PrintScreen  | 70   |

| Key            | Code | Key            | Code |
| -------------- | ---- | -------------- | ---- |
| ScrollLock     | 71   | Pause          | 72   |
| Insert         | 73   | Home           | 74   |
| PageUp         | 75   | Delete         | 76   |
| End            | 77   | PageDown       | 78   |
| ArrowRight     | 79   | ArrowLeft      | 80   |
| ArrowDown      | 81   | ArrowUp        | 82   |
| NumLock        | 83   | NumpadDivide   | 84   |
| NumpadMultiply | 85   | NumpadSubtract | 86   |
| NumpadAdd      | 87   | NumpadEnter    | 88   |
| Numpad1        | 89   | Numpad2        | 90   |
| Numpad3        | 91   | Numpad4        | 92   |
| Numpad5        | 93   | Numpad6        | 94   |
| Numpad7        | 95   | Numpad8        | 96   |
| Numpad9        | 97   | Numpad0        | 98   |
| NumpadDecimal  | 99   |                |      |

---

## 8. Communication Sequence

### 8.1 Initial Connection

```
1. Enumerate HID devices with VID=0x3554
2. Select device with matching PID and correct report configuration
3. Open HID device
4. Send GetDongleVersion (0x1D)
5. Poll DeviceOnLine (0x03) until mouse is connected (poll every 1500ms)
6. Send PCDriverStatus (0x02) with status=1
7. Send EncryptionData (0x01) handshake
8. Read full flash configuration (0x0000-0x00FF in 10-byte chunks)
9. Read key configurations
10. Start battery polling interval (every 5-10 seconds)
11. Send GetCurrentConfig (0x0E) to get active profile
12. Send ReadVersionID (0x12) to get mouse firmware version
13. If wireless, send GetLongRangeMode (0x17)
```

### 8.2 Reading Flash Memory

```rust
async fn read_full_flash(device: &HidDevice) -> Result<[u8; 256], Error> {
    let mut flash = [0u8; 256];
    let mut address = 0u16;

    while address < 256 {
        let response = read_flash(device, address, 10).await?;
        flash[address as usize..address as usize + 10].copy_from_slice(&response);
        address += 10;
    }

    Ok(flash)
}
```

### 8.3 Writing Settings

For single-byte settings, use the complement checksum format:

```rust
async fn write_setting(device: &HidDevice, address: u16, value: u8) -> Result<(), Error> {
    let mut packet = [0u8; 16];
    packet[0] = 0x07; // WriteFlashData
    packet[1] = 0x00;
    packet[2] = (address >> 8) as u8;
    packet[3] = (address & 0xFF) as u8;
    packet[4] = 2; // Length
    packet[5] = value;
    packet[6] = 0x55u8.wrapping_sub(value); // Complement
    packet[15] = calculate_checksum(&packet);

    send_report(device, &packet).await
}
```

### 8.4 Retry Logic

All commands should be sent with retry logic:

```rust
async fn send_with_retry(
    device: &HidDevice,
    packet: &[u8; 16],
    max_retries: u8,
) -> Result<[u8; 16], Error> {
    for attempt in 0..max_retries {
        device.write(packet)?;

        // Wait for response with timeout
        let response = timeout(Duration::from_millis(200), device.read()).await?;

        // Validate response matches request
        if validate_response(packet, &response) {
            return Ok(response);
        }

        sleep(Duration::from_millis(10)).await;
    }

    Err(Error::MaxRetriesExceeded)
}

fn validate_response(request: &[u8; 16], response: &[u8; 16]) -> bool {
    if request[0] == 0x08 {
        // ReadFlashData
        request[0..5] == response[0..5]
    } else {
        request[0..3] == response[0..3]
    }
}
```

---

## 9. Error Handling

### Response Status Codes

| Status (Byte 1) | Meaning               |
| --------------- | --------------------- |
| 0x00            | Success               |
| 0x01            | Error / Not Supported |

### Common Error Conditions

1. **Device Offline:** `DeviceOnLine` returns `byte[5] == 0`
2. **Command Not Supported:** Response `byte[1] == 1`
3. **Timeout:** No response within 200ms
4. **Checksum Mismatch:** Response checksum invalid

### Recommended Timeouts

| Operation             | Timeout      |
| --------------------- | ------------ |
| Standard command      | 200ms        |
| Factory reset         | 1200ms       |
| Flash write           | 200ms        |
| Online poll interval  | 1500ms       |
| Battery poll interval | 5000-10000ms |

---

## 10. Implementation Notes

### Thread Safety

- HID communication should be serialized (one command at a time)
- Use a mutex or channel to queue commands
- Battery polling can run on a separate timer

### Power Management

- Call `PCDriverStatus(0)` when closing the driver
- Stop polling when device disconnects
- Long-range mode increases power consumption

### Profile Management

- There are 4 profiles (0-3)
- Each profile has independent settings
- Changing profile requires re-reading flash

### Macro Limitations

- Maximum 30 characters for macro name
- Maximum 70 events per macro
- Delay range: 10-65535ms

---

## Appendix A: Complete Checksum Implementation

```rust
const REPORT_ID: u8 = 8;

fn calculate_checksum(packet: &[u8; 16]) -> u8 {
    let sum: u16 = packet[0..15].iter().map(|&b| b as u16).sum();
    0x55u8
        .wrapping_sub((sum & 0xFF) as u8)
        .wrapping_sub(REPORT_ID)
}

fn build_simple_command(command: u8) -> [u8; 16] {
    let mut packet = [0u8; 16];
    packet[0] = command;
    packet[15] = calculate_checksum(&packet);
    packet
}

fn build_command_with_data(command: u8, data: &[u8]) -> [u8; 16] {
    let mut packet = [0u8; 16];
    packet[0] = command;
    packet[4] = data.len() as u8;
    for (i, &byte) in data.iter().enumerate().take(10) {
        packet[5 + i] = byte;
    }
    packet[15] = calculate_checksum(&packet);
    packet
}

fn build_flash_read(address: u16, length: u8) -> [u8; 16] {
    let mut packet = [0u8; 16];
    packet[0] = 0x08;
    packet[2] = (address >> 8) as u8;
    packet[3] = (address & 0xFF) as u8;
    packet[4] = length;
    packet[15] = calculate_checksum(&packet);
    packet
}

fn build_flash_write(address: u16, data: &[u8]) -> [u8; 16] {
    let mut packet = [0u8; 16];
    packet[0] = 0x07;
    packet[2] = (address >> 8) as u8;
    packet[3] = (address & 0xFF) as u8;
    packet[4] = data.len() as u8;
    for (i, &byte) in data.iter().enumerate().take(10) {
        packet[5 + i] = byte;
    }
    packet[15] = calculate_checksum(&packet);
    packet
}

fn build_single_byte_write(address: u16, value: u8) -> [u8; 16] {
    let mut packet = [0u8; 16];
    packet[0] = 0x07;
    packet[2] = (address >> 8) as u8;
    packet[3] = (address & 0xFF) as u8;
    packet[4] = 2;
    packet[5] = value;
    packet[6] = 0x55u8.wrapping_sub(value);
    packet[15] = calculate_checksum(&packet);
    packet
}
```

---

## Appendix B: Rust Type Definitions

```rust
#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Command {
    EncryptionData = 0x01,
    PCDriverStatus = 0x02,
    DeviceOnLine = 0x03,
    BatteryLevel = 0x04,
    DongleEnterPair = 0x05,
    GetPairState = 0x06,
    WriteFlashData = 0x07,
    ReadFlashData = 0x08,
    ClearSetting = 0x09,
    StatusChanged = 0x0A,
    GetCurrentConfig = 0x0E,
    SetCurrentConfig = 0x0F,
    ReadVersionID = 0x12,
    SetLongRangeMode = 0x16,
    GetLongRangeMode = 0x17,
    GetDongleVersion = 0x1D,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FlashAddress {
    ReportRate = 0x0000,
    MaxDpi = 0x0002,
    CurrentDpi = 0x0004,
    Sensor20K = 0x0008,
    Lod = 0x000A,
    DpiValues = 0x000C,
    DpiColors = 0x002C,
    DpiEffectMode = 0x004C,
    DpiEffectBrightness = 0x004E,
    DpiEffectSpeed = 0x0050,
    DpiEffectState = 0x0052,
    KeyFunctions = 0x0060,
    LightSettings = 0x00A0,
    LightState = 0x00A7,
    DebounceTime = 0x00A9,
    MotionSync = 0x00AB,
    SleepTime = 0x00AD,
    AngleSnapping = 0x00AF,
    RippleControl = 0x00B1,
    MovingOffLight = 0x00B3,
    PerformanceState = 0x00B5,
    PerformanceTime = 0x00B7,
    SensorMode = 0x00B9,
    ShortcutKeys = 0x0100,
    Macros = 0x0300,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConnectionType {
    WirelessStandard = 0, // 1000 Hz max
    Wireless4K = 1,       // 4000 Hz max
    WiredStandard = 2,    // 1000 Hz max
    WiredHighSpeed = 3,   // 8000 Hz max
    Wireless2K = 4,       // 2000 Hz max
    Wireless8K = 5,       // 8000 Hz max
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PairStatus {
    Idle = 0,
    Pairing = 1,
    Failed = 2,
    Success = 3,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum KeyFunctionType {
    Disabled = 0,
    MouseButton = 1,
    DpiSwitch = 2,
    ScrollWheel = 3,
    FireKey = 4,
    ShortcutKey = 5,
    Macro = 6,
    ReportRateSwitch = 7,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MouseButton {
    Left = 0x0100,
    Right = 0x0200,
    Middle = 0x0400,
    Back = 0x0800,
    Forward = 0x1000,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DpiSwitchMode {
    Cycle = 0x0100,
    Up = 0x0200,
    Down = 0x0300,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LodSetting {
    Lod07mm = 3,
    Lod10mm = 1,
    Lod20mm = 2,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LightMode {
    Off = 0,
    ColorFlow = 1,
    SingleColorBreathing = 2,
    ConstantColor = 3,
    Neon = 4,
    MixedColorBreathing = 5,
    ColorfulConstant = 6,
}

#[derive(Debug, Clone)]
pub struct DeviceInfo {
    pub cid: u8,
    pub mid: u8,
    pub connection_type: ConnectionType,
    pub online: bool,
    pub address: [u8; 3],
}

#[derive(Debug, Clone)]
pub struct BatteryStatus {
    pub level: u8,
    pub charging: bool,
    pub voltage_mv: u16,
}

#[derive(Debug, Clone)]
pub struct DpiStage {
    pub value: u16,     // 50-26000
    pub color: [u8; 3], // RGB
}

#[derive(Debug, Clone)]
pub struct MouseConfig {
    pub report_rate: u16,
    pub dpi_count: u8,
    pub current_dpi: u8,
    pub dpis: [DpiStage; 8],
    pub lod: LodSetting,
    pub debounce_time: u8,
    pub motion_sync: bool,
    pub angle_snapping: bool,
    pub ripple_control: bool,
    pub performance_mode: bool,
    pub sleep_time: u8,
    pub sensor_20k: bool,
    pub light_mode: LightMode,
    pub light_color: [u8; 3],
    pub light_brightness: u8,
    pub light_speed: u8,
    pub light_on: bool,
}

#[derive(Debug, Clone)]
pub struct KeyFunction {
    pub function_type: KeyFunctionType,
    pub parameter: u16,
}

#[derive(Debug, Clone)]
pub struct MacroEvent {
    pub key_down: bool, // true = down, false = up
    pub key_type: u8,   // 1 = keyboard, 4 = mouse
    pub key_code: u16,
    pub delay_ms: u16,
}

#[derive(Debug, Clone)]
pub struct Macro {
    pub name: String,            // max 30 chars
    pub events: Vec<MacroEvent>, // max 70 events
    pub cycle_count: u8,         // 1-250, or 253-255 for special modes
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MacroCycleMode {
    Count(u8),            // 1-250
    UntilKeyPressedAgain, // 253
    UntilKeyReleased,     // 254
    UntilAnyKeyPressed,   // 255
}

impl From<u8> for MacroCycleMode {
    fn from(value: u8) -> Self {
        match value {
            253 => MacroCycleMode::UntilKeyPressedAgain,
            254 => MacroCycleMode::UntilKeyReleased,
            255 => MacroCycleMode::UntilAnyKeyPressed,
            n => MacroCycleMode::Count(n.max(1).min(250)),
        }
    }
}

impl From<MacroCycleMode> for u8 {
    fn from(mode: MacroCycleMode) -> Self {
        match mode {
            MacroCycleMode::Count(n) => n,
            MacroCycleMode::UntilKeyPressedAgain => 253,
            MacroCycleMode::UntilKeyReleased => 254,
            MacroCycleMode::UntilAnyKeyPressed => 255,
        }
    }
}
```

---

## Appendix C: Example Implementation

```rust
use hidapi::HidApi;
use std::time::Duration;

const VENDOR_ID: u16 = 0x3554;
const PRODUCT_IDS: &[u16] = &[0xF5F7, 0xF5F4, 0xF5F6];
const REPORT_ID: u8 = 8;

pub struct ScyroxMouse {
    device: hidapi::HidDevice,
    device_info: DeviceInfo,
    config: MouseConfig,
}

impl ScyroxMouse {
    pub fn open() -> Result<Self, Box<dyn std::error::Error>> {
        let api = HidApi::new()?;

        // Find device
        let device = PRODUCT_IDS
            .iter()
            .find_map(|&pid| api.open(VENDOR_ID, pid).ok())
            .ok_or("Device not found")?;

        device.set_blocking_mode(true)?;

        let mut mouse = Self {
            device,
            device_info: DeviceInfo::default(),
            config: MouseConfig::default(),
        };

        mouse.initialize()?;
        Ok(mouse)
    }

    fn initialize(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        // Get dongle version
        self.get_dongle_version()?;

        // Wait for device online
        while !self.check_online()? {
            std::thread::sleep(Duration::from_millis(1500));
        }

        // Notify driver connected
        self.set_pc_status(true)?;

        // Handshake
        self.handshake()?;

        // Read configuration
        self.read_full_config()?;

        Ok(())
    }

    fn send_command(&self, packet: &[u8; 16]) -> Result<[u8; 16], Box<dyn std::error::Error>> {
        // Prepend report ID for hidapi
        let mut report = [0u8; 17];
        report[0] = REPORT_ID;
        report[1..].copy_from_slice(packet);

        self.device.write(&report)?;

        let mut response = [0u8; 16];
        self.device.read_timeout(&mut response, 200)?;

        Ok(response)
    }

    fn check_online(&mut self) -> Result<bool, Box<dyn std::error::Error>> {
        let packet = build_simple_command(Command::DeviceOnLine as u8);
        let response = self.send_command(&packet)?;

        self.device_info.online = response[5] == 1;
        if self.device_info.online {
            self.device_info.address = [response[8], response[7], response[6]];
        }

        Ok(self.device_info.online)
    }

    fn handshake(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        let mut data = [0u8; 8];
        for i in 0..4 {
            data[i] = rand::random();
        }

        let packet = build_command_with_data(Command::EncryptionData as u8, &data);
        let response = self.send_command(&packet)?;

        self.device_info.cid = response[9];
        self.device_info.mid = response[10];
        self.device_info.connection_type = ConnectionType::from(response[11]);

        Ok(())
    }

    pub fn set_dpi(&mut self, index: u8, value: u16) -> Result<(), Box<dyn std::error::Error>> {
        let address = FlashAddress::DpiValues as u16 + (index as u16 * 4);
        let data = encode_dpi(value);

        let packet = build_flash_write(address, &data);
        self.send_command(&packet)?;

        self.config.dpis[index as usize].value = value;
        Ok(())
    }

    pub fn set_polling_rate(&mut self, hz: u16) -> Result<(), Box<dyn std::error::Error>> {
        let value = encode_report_rate(hz);
        let packet = build_single_byte_write(FlashAddress::ReportRate as u16, value);
        self.send_command(&packet)?;

        self.config.report_rate = hz;
        Ok(())
    }

    pub fn get_battery(&self) -> Result<BatteryStatus, Box<dyn std::error::Error>> {
        let packet = build_simple_command(Command::BatteryLevel as u8);
        let response = self.send_command(&packet)?;

        Ok(BatteryStatus {
            level: response[5],
            charging: response[6] == 1,
            voltage_mv: ((response[7] as u16) << 8) | (response[8] as u16),
        })
    }
}
```

---

_End of Protocol Specification_
