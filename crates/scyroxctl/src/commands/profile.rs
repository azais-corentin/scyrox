//! Profile management commands.

use anyhow::Result;

use crate::backend::Backend;
use crate::cli::{ProfileAction, ProfileCommand};

pub async fn run(backend: &dyn Backend, cmd: &ProfileCommand) -> Result<()> {
    match &cmd.action {
        ProfileAction::List => list_profiles(backend).await,
        ProfileAction::Show { id } => show_profile(backend, id).await,
        ProfileAction::Create { name, default } => create_profile(backend, name, *default).await,
        ProfileAction::Apply { id } => apply_profile(backend, id).await,
        ProfileAction::Delete { id } => delete_profile(backend, id).await,
        ProfileAction::SetDefault { id } => set_default(backend, id).await,
    }
}

async fn list_profiles(backend: &dyn Backend) -> Result<()> {
    let profiles = backend.list_profiles().await?;

    if profiles.is_empty() {
        println!("No profiles found.");
        println!("Create one with: scyroxctl profile create <name>");
        return Ok(());
    }

    println!("Profiles:");
    for profile in profiles {
        let default_marker = if profile.is_default { " (default)" } else { "" };
        println!("  {} - {}{}", profile.id, profile.name, default_marker);
    }

    Ok(())
}

async fn show_profile(backend: &dyn Backend, id: &str) -> Result<()> {
    let profile = backend.get_profile(id).await?;

    println!("Profile: {}", profile.name);
    println!("  ID:      {}", profile.id);
    println!("  Default: {}", if profile.is_default { "Yes" } else { "No" });
    println!();
    println!("Configuration:");
    println!("  Polling Rate:      {}", profile.config.polling_rate);
    println!("  Lift-Off Distance: {}", profile.config.lift_off_distance);
    println!("  Sleep Timeout:     {} seconds", profile.config.sleep_timeout_seconds);
    println!("  Angle Snapping:    {}", if profile.config.angle_snapping { "On" } else { "Off" });
    println!("  Ripple Control:    {}", if profile.config.ripple_control { "On" } else { "Off" });
    println!("  High Speed Mode:   {}", if profile.config.high_speed_mode { "On" } else { "Off" });
    println!("  Long Distance:     {}", if profile.config.long_distance_mode { "On" } else { "Off" });

    Ok(())
}

async fn create_profile(backend: &dyn Backend, name: &str, set_default: bool) -> Result<()> {
    let profile = backend.create_profile(name, set_default).await?;

    println!("Created profile: {} ({})", profile.name, profile.id);
    if profile.is_default {
        println!("Set as default profile");
    }

    Ok(())
}

async fn apply_profile(backend: &dyn Backend, id: &str) -> Result<()> {
    backend.apply_profile(id).await?;
    println!("Applied profile: {}", id);
    Ok(())
}

async fn delete_profile(backend: &dyn Backend, id: &str) -> Result<()> {
    backend.delete_profile(id).await?;
    println!("Deleted profile: {}", id);
    Ok(())
}

async fn set_default(backend: &dyn Backend, id: &str) -> Result<()> {
    backend.set_default_profile(id).await?;
    println!("Set default profile: {}", id);
    Ok(())
}
