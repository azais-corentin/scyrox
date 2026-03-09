//! Status command.

use anyhow::Result;

use crate::output::{Output, StatusOutput};
use scyrox_client::Backend;

pub async fn run(backend: &dyn Backend, output: &Output) -> Result<()> {
    let connected = backend.is_connected().await;

    let polling_rate = if connected {
        backend
            .get_config()
            .await
            .ok()
            .map(|c| c.polling_rate.to_string())
    } else {
        None
    };

    let battery = if connected {
        backend.get_battery().await.ok()
    } else {
        None
    };

    let daemon = backend.get_daemon_info().await?;

    let status = StatusOutput {
        connected,
        polling_rate,
        battery,
        daemon,
    };

    output.print_status(&status);

    Ok(())
}
