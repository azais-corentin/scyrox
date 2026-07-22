//! Get commands.

use anyhow::Result;

use crate::cli::{GetCommand, GetWhat};
use crate::output::Output;
use scyrox_client::Backend;

pub async fn run(backend: &dyn Backend, cmd: &GetCommand, output: &Output) -> Result<()> {
    match &cmd.what {
        GetWhat::Config => get_config(backend, output).await,
        GetWhat::Battery => get_battery(backend, output).await,
        GetWhat::Firmware => get_firmware(backend, output).await,
        GetWhat::PollingRate => get_polling_rate(backend, output).await,
        GetWhat::Lod => get_lod(backend, output).await,
        GetWhat::SleepTimeout => get_sleep_timeout(backend, output).await,
        GetWhat::Dpi => get_dpi(backend, output).await,
    }
}

async fn get_config(backend: &dyn Backend, output: &Output) -> Result<()> {
    let config = backend.get_config().await?;
    output.print_config(&config);
    Ok(())
}

async fn get_battery(backend: &dyn Backend, output: &Output) -> Result<()> {
    let battery = backend.get_battery().await?;
    output.print_battery(&battery);
    Ok(())
}

async fn get_firmware(backend: &dyn Backend, output: &Output) -> Result<()> {
    let firmware = backend.get_firmware().await?;
    output.print_firmware(&firmware);
    Ok(())
}

async fn get_polling_rate(backend: &dyn Backend, output: &Output) -> Result<()> {
    let config = backend.get_config().await?;
    output.print_value(&config.polling_rate);
    Ok(())
}

async fn get_lod(backend: &dyn Backend, output: &Output) -> Result<()> {
    let config = backend.get_config().await?;
    output.print_value(&config.lift_off_distance);
    Ok(())
}

async fn get_sleep_timeout(backend: &dyn Backend, output: &Output) -> Result<()> {
    let config = backend.get_config().await?;
    output.print_value(&format!("{}s", config.sleep_timeout_seconds));
    Ok(())
}

async fn get_dpi(backend: &dyn Backend, output: &Output) -> Result<()> {
    let config = backend.get_config().await?;
    output.print_dpi(&config);
    Ok(())
}
