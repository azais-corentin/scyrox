- [ ] **Device Initialization Workflow**
  - [ ] Implement device enumeration by VID/PID
  - [ ] Open HID device and claim interface
  - [ ] Build handshake command (EncryptionData 0x01)
  - [ ] Parse handshake response (CID, MID, ConnectionType)
  - [ ] Implement check_online (DeviceOnLine 0x03)
  - [ ] Implement set_driver_status (PCDriverStatus 0x02)
  - [ ] Implement read_full_flash for configuration read
  - [ ] Implement get_current_profile (GetCurrentConfig 0x0E)
  - [ ] Implement GetLongRangeMode query (0x17)
  - [ ] Create unified initialize() method that executes full sequence per section 8.1
  - [ ] Add DeviceOnLine polling loop (1500ms intervals) before handshake
  - [ ] Get dongle version as first step after opening device
  - [ ] Implement retry logic in initialization (reconnect on failure)

- [ ] **StatusChanged Notification Handling (0x0A)**
  - [ ] Implement parse_status_changed_notification function
  - [ ] Define StatusChangeFlags with all flag bits
  - [ ] Implement is_status_changed_notification check
  - [ ] Implement active background listener for unsolicited notifications
  - [ ] Add callback/observer registration system for notification events
  - [ ] Auto-refresh affected data when flags detected:
    - [ ] DPI changed (0x01) - re-read address 0x0004
    - [ ] Report rate changed (0x02) - re-read address 0x0000
    - [ ] Profile changed (0x04) - call GetCurrentConfig
    - [ ] DPI settings changed (0x08) - re-read addresses 0x000C-0x004B
    - [ ] Light settings changed (0x20) - re-read address 0x00A0+
    - [ ] Battery changed (0x40) - call BatteryLevel command

- [ ] **Battery Polling System**
  - [ ] Implement get_battery command (BatteryLevel 0x04)
  - [ ] Parse battery percentage, charging status, voltage
  - [ ] Implement voltage_to_percentage_table lookup
  - [ ] Add background timer for automatic polling (5-10 second intervals)
  - [ ] Store cached battery status in Mouse struct
  - [ ] Implement stop_battery_polling method
  - [ ] Add battery status change event notification

- [ ] **Pairing Mode Support**
  - [ ] Implement enter_pairing_mode (DongleEnterPair 0x05)
  - [ ] Implement get_pair_state (GetPairState 0x06)
  - [ ] Define PairStatus enum (Idle, Pairing, Failed, Success)
  - [ ] Parse time remaining from response
  - [ ] Add CLI command for pairing initiation
  - [ ] Add CLI command for pairing status check
  - [ ] Implement pairing timeout monitoring

- [ ] **Profile Management**
  - [ ] Implement get_current_profile (GetCurrentConfig 0x0E)
  - [ ] Implement set_current_profile (SetCurrentConfig 0x0F)
  - [ ] Add profile switching with automatic config re-read
  - [ ] Add CLI command for device profile get/set (0-3)
  - [ ] Document 4 profiles (0-3) with independent settings

- [ ] **Factory Reset Flow**
  - [ ] Implement factory_reset (ClearSetting 0x09)
  - [ ] Add 1200ms delay after reset command per section 5.9
  - [ ] Re-establish connection after device reconnects
  - [ ] Add CLI command for factory reset with confirmation prompt

- [ ] **Thread Safety and Concurrency**
  - [ ] Wrap Mouse struct in Mutex for HID command serialization
  - [ ] Implement channel/queue system for concurrent requests
  - [ ] Ensure battery polling doesn't interfere with commands
  - [ ] Document thread safety guarantees in public API

- [ ] **Power Management and Cleanup**
  - [ ] Implement set_driver_status for connection notification
  - [ ] Create disconnect() method that calls set_driver_status(false)
  - [ ] Implement graceful shutdown for background tasks
  - [ ] Detect and handle device disconnect events
  - [ ] Add reconnection logic for transient disconnects

- [ ] **Retry Logic and Error Handling**
  - [ ] Implement checksum calculation (calculate_checksum)
  - [ ] Implement response validation (validate_response)
  - [ ] Implement verify_response_checksum
  - [ ] Check response status byte (0x00=success, 0x01=error)
  - [ ] Create send_with_retry wrapper with configurable max_retries
  - [ ] Use validate_response in all command flows
  - [ ] Add 200ms timeout for standard commands per section 9
  - [ ] Verify response checksums on all received packets using verify_response_checksum
  - [ ] Handle NotSupported responses gracefully for optional features

- [ ] **Flash Memory Operations**
  - [ ] Read/write basic configuration (0x0000-0x00FF)
  - [ ] Read/write DPI values and colors
  - [ ] Read/write key functions
  - [ ] Read/write light settings
  - [ ] Read/write sensor settings (debounce, motion sync, etc.)
  - [ ] Read/write shortcut keys (0x0100-0x01FF)
  - [ ] Read/write macros (0x0300-0x0BFF)
  - [ ] Implement complement format for single-byte writes (value + 0x55-value)

- [ ] **DPI Management**
  - [ ] Implement get_dpi_count
  - [ ] Implement get_current_dpi_index / set_current_dpi_index
  - [ ] Implement get_dpi_value / set_dpi_value
  - [ ] Implement get_dpi_color / set_dpi_color
  - [ ] Implement get_dpi_stages (all stages)
  - [ ] Implement DPI encoding/decoding per section 6.7
  - [ ] Add CLI command for DPI stage get/set
  - [ ] Add CLI command for DPI color configuration
  - [ ] Add CLI command for current DPI index get/set
  - [ ] Add CLI command for DPI count configuration

- [ ] **Light Settings**
  - [ ] Implement get_light_settings / set_light_settings
  - [ ] Define LightMode enum (Off, ColorFlow, Breathing, Constant, Neon, etc.)
  - [ ] Implement get_dpi_effect_settings / set_dpi_effect_settings
  - [ ] Add CLI command for light mode configuration
  - [ ] Add CLI command for light color/brightness/speed
  - [ ] Add CLI command for DPI effect settings

- [ ] **Key Function Configuration**
  - [ ] Implement get_key_function / set_key_function
  - [ ] Implement get_all_key_functions
  - [ ] Define KeyFunctionType enum (Disabled, MouseButton, DpiSwitch, etc.)
  - [ ] Define parameter types (MouseButton, DpiSwitchMode, FireKeyConfig, etc.)
  - [ ] Add CLI command for key function configuration
  - [ ] Add CLI command for key function get/set
  - [ ] Add human-readable display of key function configuration

- [ ] **Shortcut Key Configuration**
  - [ ] Implement get_shortcut_key / set_shortcut_key
  - [ ] Define ShortcutKey and ShortcutKeyEvent types
  - [ ] Define ShortcutKeyType (Modifier, Normal, Media)
  - [ ] Define ModifierKey and MediaKey enums
  - [ ] Add CLI command for shortcut key get/set
  - [ ] Implement get_all_shortcut_keys method for bulk retrieval
  - [ ] Add human-readable display of shortcut key events

- [ ] **Macro Configuration**
  - [ ] Implement get_macro / set_macro
  - [ ] Define Macro, MacroEvent, MacroEventKeyType types
  - [ ] Define MacroCycleMode (Count, UntilKeyPressed, etc.)
  - [ ] Define MacroMouseButton enum
  - [ ] Implement macro encoding/decoding per section 6.14
  - [ ] Add CLI command for macro list/get/set
  - [ ] Validate macro limits (70 events, 30 char names) in CLI
  - [ ] Add human-readable display of macro events

- [ ] **HID Keyboard Scan Codes**
  - [ ] Define HidKeyCode enum with all codes from section 7
  - [ ] Implement code() and from_code() methods
  - [ ] Add helper for human-readable key name display

- [ ] **Connection Type Detection**
  - [ ] Define ConnectionType enum (WirelessStandard, Wireless4K, WiredStandard, etc.)
  - [ ] Implement from_byte parser
  - [ ] Implement max_polling_rate_hz method
  - [ ] Implement is_wireless check

- [ ] **CLI Commands**
  - [ ] Implement status command (battery, firmware, config)
  - [ ] Implement get command (polling rate, LOD, etc.)
  - [ ] Implement set command (polling rate, LOD, sleep, etc.)
  - [ ] Add pair command (initiate pairing, check status)
  - [ ] Add reset command (factory reset with confirmation)
  - [ ] Add device-profile command (get/set hardware profile 0-3)
  - [ ] Add dpi command (get/set DPI stages, colors, current index)
  - [ ] Add light command (mode, color, brightness, speed)
  - [ ] Add key command (configure button functions)
  - [ ] Add shortcut command (configure shortcut keys)
  - [ ] Add macro command (list, create, edit macros)
  - [ ] Expose connection type info in status command
  - [ ] Add watch command for monitoring StatusChanged notifications

- [ ] **Daemon Functionality**
  - [ ] Basic daemon server structure
  - [ ] gRPC protocol definition
  - [ ] Implement StatusChanged notification forwarding to clients
  - [ ] Implement battery status caching and push updates
  - [ ] Add device connect/disconnect event handling
  - [ ] Implement automatic profile application on device connect

- [ ] **Testing**
  - [ ] Unit tests for checksum calculation
  - [ ] Unit tests for DPI encoding/decoding
  - [ ] Unit tests for report rate encoding/decoding
  - [ ] Unit tests for brightness encoding/decoding
  - [ ] Unit tests for firmware version formatting
  - [ ] Unit tests for validate_response
  - [ ] Unit tests for parse_status_changed_notification
  - [ ] Integration tests for full initialization sequence
  - [ ] Tests for notification handling with all flag combinations
  - [ ] Tests for profile switching scenarios
  - [ ] Tests for factory reset and recovery
  - [ ] Tests for retry logic behavior
  - [ ] Tests for concurrent access handling

- [ ] **Documentation**
  - [ ] Document full initialization sequence with required delays
  - [ ] Document recommended timeouts per operation type
  - [ ] Document thread safety guarantees and usage patterns
  - [ ] Document macro limitations (70 events, 30 char names, 10-65535ms delay)
  - [ ] Add examples for common configuration tasks
  - [ ] Document error handling and recovery procedures
