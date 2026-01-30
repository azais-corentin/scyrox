//! Get commands.

use anyhow::Result;

use crate::backend::Backend;
use crate::cli::{GetCommand, GetWhat};

pub async fn run(backend: &dyn Backend, cmd: &GetCommand) -> Result<()> {
    match &cmd.what {
        GetWhat::Config => get_config(backend).await,
        GetWhat::Battery => get_battery(backend).await,
        GetWhat::Firmware => get_firmware(backend).await,
        GetWhat::PollingRate => get_polling_rate(backend).await,
        GetWhat::Lod => get_lod(backend).await,
        GetWhat::SleepTimeout => get_sleep_timeout(backend).await,
    }
}

async fn get_config(backend: &dyn Backend) -> Result<()> {
    let config = backend.get_config().await?;

    println!("Configuration:");
    println!("  Polling Rate:      {}", config.polling_rate);
    println!("  Lift-Off Distance: {}", config.lift_off_distance);
    println!("  Sleep Timeout:     {} seconds", config.sleep_timeout_seconds);
    println!("  Angle Snapping:    {}", if config.angle_snapping { "On" } else { "Off" });
    println!("  Ripple Control:    {}", if config.ripple_control { "On" } else { "Off" });
    println!("  High Speed Mode:   {}", if config.high_speed_mode { "On" } else { "Off" });
    println!("  Long Distance:     {}", if config.long_distance_mode { "On" } else { "Off" });

    Ok(())
}

async fn get_battery(backend: &dyn Backend) -> Result<()> {
    let battery = backend.get_battery().await?;

    println!("Battery:");
    println!("  Voltage:    {} mV", battery.voltage_mv);
    println!("  Percentage: {}%", battery.percentage);

    Ok(())
}

async fn get_firmware(backend: &dyn Backend) -> Result<()> {
    let firmware = backend.get_firmware().await?;

    println!("Firmware:");
    println!("  Mouse:    {}", firmware.mouse_version);
    if let Some(receiver) = &firmware.receiver_version {
        println!("  Receiver: {}", receiver);
    }

    Ok(())
}

async fn get_polling_rate(backend: &dyn Backend) -> Result<()> {
    let config = backend.get_config().await?;
    println!("{}", config.polling_rate);
    Ok(())
}

async fn get_lod(backend: &dyn Backend) -> Result<()> {
    let config = backend.get_config().await?;
    println!("{}", config.lift_off_distance);
    Ok(())
}

async fn get_sleep_timeout(backend: &dyn Backend) -> Result<()> {
    let config = backend.get_config().await?;
    println!("{}s", config.sleep_timeout_seconds);
    Ok(())
}
