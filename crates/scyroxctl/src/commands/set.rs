//! Set commands.

use anyhow::Result;
use scyrox::{LiftOffDistance, PollingRate};

use crate::backend::Backend;
use crate::cli::{LodArg, PollingRateArg, SetCommand, SetWhat};
use crate::output::Output;

pub async fn run(backend: &dyn Backend, cmd: &SetCommand, output: &Output) -> Result<()> {
    match &cmd.what {
        SetWhat::PollingRate { rate } => set_polling_rate(backend, *rate, output).await,
        SetWhat::Lod { distance } => set_lod(backend, *distance, output).await,
        SetWhat::SleepTimeout { seconds } => set_sleep_timeout(backend, *seconds, output).await,
        SetWhat::AngleSnapping { state } => {
            set_angle_snapping(backend, state.to_bool(), output).await
        }
        SetWhat::RippleControl { state } => {
            set_ripple_control(backend, state.to_bool(), output).await
        }
        SetWhat::HighSpeedMode { state } => {
            set_high_speed_mode(backend, state.to_bool(), output).await
        }
        SetWhat::LongDistanceMode { state } => {
            set_long_distance_mode(backend, state.to_bool(), output).await
        }
    }
}

async fn set_polling_rate(
    backend: &dyn Backend,
    rate: PollingRateArg,
    output: &Output,
) -> Result<()> {
    let rate = match rate {
        PollingRateArg::Hz125 => PollingRate::Hz125,
        PollingRateArg::Hz250 => PollingRate::Hz250,
        PollingRateArg::Hz500 => PollingRate::Hz500,
        PollingRateArg::Hz1000 => PollingRate::Hz1000,
        PollingRateArg::Hz2000 => PollingRate::Hz2000,
        PollingRateArg::Hz4000 => PollingRate::Hz4000,
        PollingRateArg::Hz8000 => PollingRate::Hz8000,
    };

    backend.set_polling_rate(rate).await?;
    output.print_success(&format!("Polling rate set to {}", rate));
    Ok(())
}

async fn set_lod(backend: &dyn Backend, distance: LodArg, output: &Output) -> Result<()> {
    let lod = match distance {
        LodArg::Low => LiftOffDistance::Low,
        LodArg::Medium => LiftOffDistance::Medium,
        LodArg::High => LiftOffDistance::High,
    };

    backend.set_lift_off_distance(lod).await?;
    output.print_success(&format!("Lift-off distance set to {}", lod));
    Ok(())
}

async fn set_sleep_timeout(backend: &dyn Backend, seconds: u16, output: &Output) -> Result<()> {
    let actual = backend.set_sleep_timeout(seconds).await?;
    if actual == 0 {
        output.print_success("Sleep timeout disabled");
    } else {
        output.print_success(&format!("Sleep timeout set to {} seconds", actual));
    }
    Ok(())
}

async fn set_angle_snapping(backend: &dyn Backend, enabled: bool, output: &Output) -> Result<()> {
    backend.set_angle_snapping(enabled).await?;
    output.print_success(&format!(
        "Angle snapping {}",
        if enabled { "enabled" } else { "disabled" }
    ));
    Ok(())
}

async fn set_ripple_control(backend: &dyn Backend, enabled: bool, output: &Output) -> Result<()> {
    backend.set_ripple_control(enabled).await?;
    output.print_success(&format!(
        "Ripple control {}",
        if enabled { "enabled" } else { "disabled" }
    ));
    Ok(())
}

async fn set_high_speed_mode(backend: &dyn Backend, enabled: bool, output: &Output) -> Result<()> {
    backend.set_high_speed_mode(enabled).await?;
    output.print_success(&format!(
        "High speed mode {}",
        if enabled { "enabled" } else { "disabled" }
    ));
    Ok(())
}

async fn set_long_distance_mode(
    backend: &dyn Backend,
    enabled: bool,
    output: &Output,
) -> Result<()> {
    backend.set_long_distance_mode(enabled).await?;
    output.print_success(&format!(
        "Long distance mode {}",
        if enabled { "enabled" } else { "disabled" }
    ));
    Ok(())
}
