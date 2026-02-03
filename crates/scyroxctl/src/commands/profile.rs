//! Profile management commands.

use anyhow::Result;

use crate::backend::Backend;
use crate::cli::{ProfileAction, ProfileCommand};
use crate::output::Output;

pub async fn run(backend: &dyn Backend, cmd: &ProfileCommand, output: &Output) -> Result<()> {
    match &cmd.action {
        ProfileAction::List => list_profiles(backend, output).await,
        ProfileAction::Show { id } => show_profile(backend, id, output).await,
        ProfileAction::Create { name, default } => {
            create_profile(backend, name, *default, output).await
        }
        ProfileAction::Apply { id } => apply_profile(backend, id, output).await,
        ProfileAction::Delete { id } => delete_profile(backend, id, output).await,
        ProfileAction::SetDefault { id } => set_default(backend, id, output).await,
    }
}

async fn list_profiles(backend: &dyn Backend, output: &Output) -> Result<()> {
    let profiles = backend.list_profiles().await?;
    output.print_profiles(&profiles);
    Ok(())
}

async fn show_profile(backend: &dyn Backend, id: &str, output: &Output) -> Result<()> {
    let profile = backend.get_profile(id).await?;
    output.print_profile(&profile);
    Ok(())
}

async fn create_profile(
    backend: &dyn Backend,
    name: &str,
    set_default: bool,
    output: &Output,
) -> Result<()> {
    let profile = backend.create_profile(name, set_default).await?;
    output.print_profile(&profile);
    Ok(())
}

async fn apply_profile(backend: &dyn Backend, id: &str, output: &Output) -> Result<()> {
    backend.apply_profile(id).await?;
    output.print_success(&format!("Applied profile: {}", id));
    Ok(())
}

async fn delete_profile(backend: &dyn Backend, id: &str, output: &Output) -> Result<()> {
    backend.delete_profile(id).await?;
    output.print_success(&format!("Deleted profile: {}", id));
    Ok(())
}

async fn set_default(backend: &dyn Backend, id: &str, output: &Output) -> Result<()> {
    backend.set_default_profile(id).await?;
    output.print_success(&format!("Set default profile: {}", id));
    Ok(())
}
