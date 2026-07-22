//! CLI argument definitions using clap.

use std::path::PathBuf;

use clap::{Parser, Subcommand, ValueEnum};

/// Output format for command results.
#[derive(Copy, Clone, Default, Debug, PartialEq, Eq, ValueEnum)]
pub enum OutputFormat {
    /// Human-readable text output
    #[default]
    Text,
    /// JSON output
    Json,
}

/// Scyrox mouse configuration tool.
#[derive(Parser)]
#[command(name = "scyroxctl")]
#[command(author, version, about, long_about = None)]
pub struct Cli {
    /// Use direct USB access instead of connecting to daemon
    #[arg(short, long, global = true)]
    pub direct: bool,

    /// Output format
    #[arg(short = 'f', long, global = true, default_value = "text")]
    pub format: OutputFormat,

    /// Increase verbosity (-v for debug, -vv for trace)
    #[arg(short, long, action = clap::ArgAction::Count, global = true)]
    pub verbose: u8,

    /// Enable trace-level logging (equivalent to -vv)
    #[arg(long, global = true)]
    pub trace: bool,

    #[command(subcommand)]
    pub command: Commands,
}

#[derive(Subcommand)]
pub enum Commands {
    /// Get device information and settings
    Get(GetCommand),

    /// Set device settings
    Set(SetCommand),

    /// Manage configuration profiles
    Profile(ProfileCommand),

    /// Show device and daemon status
    Status,

    /// Manage the scyroxd daemon
    Daemon(DaemonCommand),
}

// =============================================================================
// Get Commands
// =============================================================================

#[derive(Parser)]
pub struct GetCommand {
    #[command(subcommand)]
    pub what: GetWhat,
}

#[derive(Subcommand)]
pub enum GetWhat {
    /// Get all configuration settings
    Config,
    /// Get battery status
    Battery,
    /// Get firmware version information
    Firmware,
    /// Get polling rate
    PollingRate,
    /// Get lift-off distance
    Lod,
    /// Get sleep timeout
    SleepTimeout,
    /// Get DPI stages and active stage
    Dpi,
}

// =============================================================================
// Set Commands
// =============================================================================

#[derive(Parser)]
pub struct SetCommand {
    #[command(subcommand)]
    pub what: SetWhat,
}

#[derive(Subcommand)]
pub enum SetWhat {
    /// Set polling rate
    PollingRate {
        /// Polling rate in Hz
        #[arg(value_enum)]
        rate: PollingRateArg,
    },
    /// Set lift-off distance
    Lod {
        /// Lift-off distance
        #[arg(value_enum)]
        distance: LodArg,
    },
    /// Set sleep timeout
    SleepTimeout {
        /// Timeout in seconds (0 = never, max 2550)
        seconds: u16,
    },
    /// Set angle snapping
    AngleSnapping {
        /// Enable or disable
        #[arg(value_enum)]
        state: BoolArg,
    },
    /// Set ripple control
    RippleControl {
        /// Enable or disable
        #[arg(value_enum)]
        state: BoolArg,
    },
    /// Set high speed mode
    HighSpeedMode {
        /// Enable or disable
        #[arg(value_enum)]
        state: BoolArg,
    },
    /// Set long distance mode
    LongDistanceMode {
        /// Enable or disable
        #[arg(value_enum)]
        state: BoolArg,
    },
    /// Set DPI value for a stage
    Dpi {
        /// DPI value (50-26000, steps of 50)
        value: u16,
        /// Stage index 0-7 (default: active stage)
        #[arg(short, long)]
        stage: Option<u8>,
    },
    /// Set the active DPI stage
    DpiStage {
        /// Stage index (0-7)
        index: u8,
    },
    /// Set the number of active DPI stages
    DpiCount {
        /// Stage count (1-8)
        count: u8,
    },
    /// Set a DPI stage's color
    DpiColor {
        /// Color as RRGGBB hex (leading # optional)
        #[arg(value_parser = parse_hex_color)]
        color: [u8; 3],
        /// Stage index 0-7 (default: active stage)
        #[arg(short, long)]
        stage: Option<u8>,
    },
}

/// clap value parser: parse an "RRGGBB" (or "#RRGGBB") hex color into RGB bytes.
fn parse_hex_color(s: &str) -> Result<[u8; 3], String> {
    scyrox::parse_color_hex(s).ok_or_else(|| format!("Invalid color '{s}': expected RRGGBB hex"))
}

#[derive(Copy, Clone, PartialEq, Eq, ValueEnum)]
pub enum PollingRateArg {
    #[value(name = "125")]
    Hz125,
    #[value(name = "250")]
    Hz250,
    #[value(name = "500")]
    Hz500,
    #[value(name = "1000")]
    Hz1000,
    #[value(name = "2000")]
    Hz2000,
    #[value(name = "4000")]
    Hz4000,
    #[value(name = "8000")]
    Hz8000,
}

#[derive(Copy, Clone, PartialEq, Eq, ValueEnum)]
pub enum LodArg {
    /// 0.7mm
    Low,
    /// 1.0mm
    Medium,
    /// 2.0mm
    High,
}

#[derive(Copy, Clone, PartialEq, Eq, ValueEnum)]
pub enum BoolArg {
    On,
    Off,
    #[value(name = "true")]
    True,
    #[value(name = "false")]
    False,
    #[value(name = "1")]
    One,
    #[value(name = "0")]
    Zero,
}

impl BoolArg {
    pub fn to_bool(self) -> bool {
        matches!(self, BoolArg::On | BoolArg::True | BoolArg::One)
    }
}

// =============================================================================
// Profile Commands
// =============================================================================

#[derive(Parser)]
pub struct ProfileCommand {
    #[command(subcommand)]
    pub action: ProfileAction,
}

#[derive(Subcommand)]
pub enum ProfileAction {
    /// List all profiles
    List,
    /// Show a profile
    Show {
        /// Profile ID
        id: String,
    },
    /// Create a new profile from current settings
    Create {
        /// Profile name
        name: String,
        /// Set as default profile
        #[arg(long)]
        default: bool,
    },
    /// Apply a profile
    Apply {
        /// Profile ID
        id: String,
    },
    /// Delete a profile
    Delete {
        /// Profile ID
        id: String,
    },
    /// Set the default profile
    SetDefault {
        /// Profile ID
        id: String,
    },
}

// =============================================================================
// Daemon Commands
// =============================================================================

#[derive(Parser)]
pub struct DaemonCommand {
    #[command(subcommand)]
    pub action: DaemonAction,
}

#[derive(Subcommand)]
pub enum DaemonAction {
    /// Start the daemon
    Start {
        /// Run in foreground (don't daemonize)
        #[arg(long)]
        foreground: bool,
    },
    /// Stop the daemon
    Stop,
    /// Show daemon status
    Status,
    /// Restart the daemon
    Restart,
    /// Manage daemon configuration
    Config(DaemonConfigCommand),
}

#[derive(Parser)]
pub struct DaemonConfigCommand {
    #[command(subcommand)]
    pub action: DaemonConfigAction,
}

#[derive(Subcommand)]
pub enum DaemonConfigAction {
    /// Show daemon configuration
    Get,
    /// Update a daemon setting
    Set {
        #[command(subcommand)]
        setting: DaemonConfigSetAction,
    },
    /// Clear an optional daemon setting
    Unset {
        #[command(subcommand)]
        setting: DaemonConfigUnsetAction,
    },
}

#[derive(Subcommand)]
pub enum DaemonConfigSetAction {
    /// Set the low-battery alert threshold
    LowBatteryThreshold {
        /// Low-battery alert threshold as a percentage
        #[arg(value_parser = clap::value_parser!(u8).range(0..=100))]
        percentage: u8,
    },
    /// Set the append-only battery JSON Lines destination
    BatteryLogPath {
        /// Config-relative or absolute log path
        path: PathBuf,
    },
}

#[derive(Subcommand)]
pub enum DaemonConfigUnsetAction {
    /// Disable battery logging
    BatteryLogPath,
}
