//! Profile storage and management.

use std::path::PathBuf;

use anyhow::{Result, anyhow};
use directories::ProjectDirs;
use serde::{Deserialize, Serialize};
use tokio::fs;
use tracing::{debug, info};

/// A saved mouse configuration profile.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Profile {
    /// Unique identifier (filename without extension).
    pub id: String,
    /// Human-readable name.
    pub name: String,
    /// The mouse configuration.
    pub config: ProfileConfig,
    /// Whether this is the default profile.
    #[serde(default)]
    pub is_default: bool,
}

/// Mouse configuration stored in a profile.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProfileConfig {
    pub polling_rate_hz: u16,
    pub lift_off_distance_mm: f32,
    pub sleep_timeout_seconds: u16,
    pub angle_snapping: bool,
    pub ripple_control: bool,
    pub high_speed_mode: bool,
    pub long_distance_mode: bool,
}

/// Profile storage manager.
pub struct ProfileStore {
    profiles_dir: PathBuf,
}

impl ProfileStore {
    /// Create a new profile store.
    pub fn new(dirs: &ProjectDirs) -> Self {
        Self {
            profiles_dir: dirs.config_dir().join("profiles"),
        }
    }

    /// Ensure the profiles directory exists.
    pub async fn init(&self) -> Result<()> {
        fs::create_dir_all(&self.profiles_dir).await?;
        Ok(())
    }

    /// List all profiles.
    pub async fn list(&self) -> Result<Vec<Profile>> {
        let mut profiles = Vec::new();

        if !self.profiles_dir.exists() {
            return Ok(profiles);
        }

        let mut entries = fs::read_dir(&self.profiles_dir).await?;
        while let Some(entry) = entries.next_entry().await? {
            let path = entry.path();
            if path.extension().is_some_and(|ext| ext == "toml") {
                match self.load_profile(&path).await {
                    Ok(profile) => profiles.push(profile),
                    Err(e) => {
                        debug!(?path, ?e, "Failed to load profile");
                    }
                }
            }
        }

        Ok(profiles)
    }

    /// Get a profile by ID.
    pub async fn get(&self, id: &str) -> Result<Profile> {
        let path = self.profile_path(id);
        self.load_profile(&path).await
    }

    /// Create a new profile.
    pub async fn create(&self, name: String, config: ProfileConfig) -> Result<Profile> {
        self.init().await?;

        // Generate ID from name
        let id = slugify(&name);

        // Check for conflicts
        let path = self.profile_path(&id);
        if path.exists() {
            return Err(anyhow!("Profile with ID '{}' already exists", id));
        }

        let profile = Profile {
            id,
            name,
            config,
            is_default: false,
        };

        self.save_profile(&profile).await?;
        info!(id = %profile.id, "Created profile");

        Ok(profile)
    }

    /// Update an existing profile.
    pub async fn update(
        &self,
        id: &str,
        name: Option<String>,
        config: Option<ProfileConfig>,
    ) -> Result<Profile> {
        let mut profile = self.get(id).await?;

        if let Some(name) = name {
            profile.name = name;
        }
        if let Some(config) = config {
            profile.config = config;
        }

        self.save_profile(&profile).await?;
        info!(id = %profile.id, "Updated profile");

        Ok(profile)
    }

    /// Delete a profile.
    pub async fn delete(&self, id: &str) -> Result<()> {
        let path = self.profile_path(id);
        if !path.exists() {
            return Err(anyhow!("Profile '{}' not found", id));
        }

        fs::remove_file(&path).await?;
        info!(id, "Deleted profile");

        Ok(())
    }

    /// Set a profile as the default.
    pub async fn set_default(&self, id: &str) -> Result<()> {
        // First, unset any existing default
        let profiles = self.list().await?;
        for mut profile in profiles {
            if profile.is_default && profile.id != id {
                profile.is_default = false;
                self.save_profile(&profile).await?;
            }
        }

        // Set the new default
        let mut profile = self.get(id).await?;
        profile.is_default = true;
        self.save_profile(&profile).await?;

        info!(id, "Set default profile");
        Ok(())
    }

    /// Get the default profile, if any.
    pub async fn get_default(&self) -> Result<Option<Profile>> {
        let profiles = self.list().await?;
        Ok(profiles.into_iter().find(|p| p.is_default))
    }

    /// Get the path for a profile file.
    fn profile_path(&self, id: &str) -> PathBuf {
        self.profiles_dir.join(format!("{}.toml", id))
    }

    /// Load a profile from a file.
    async fn load_profile(&self, path: &PathBuf) -> Result<Profile> {
        let contents = fs::read_to_string(path).await?;
        let mut profile: Profile = toml::from_str(&contents)?;

        // Set ID from filename if not present
        if profile.id.is_empty() {
            if let Some(stem) = path.file_stem() {
                profile.id = stem.to_string_lossy().into_owned();
            }
        }

        Ok(profile)
    }

    /// Save a profile to a file.
    async fn save_profile(&self, profile: &Profile) -> Result<()> {
        let path = self.profile_path(&profile.id);
        let contents = toml::to_string_pretty(profile)?;
        fs::write(&path, contents).await?;
        Ok(())
    }
}

/// Convert a string to a URL-safe slug.
fn slugify(s: &str) -> String {
    s.to_lowercase()
        .chars()
        .map(|c| if c.is_alphanumeric() { c } else { '-' })
        .collect::<String>()
        .split('-')
        .filter(|s| !s.is_empty())
        .collect::<Vec<_>>()
        .join("-")
}

/// Convert from scyrox types to profile config.
impl From<scyrox::MouseConfig> for ProfileConfig {
    fn from(config: scyrox::MouseConfig) -> Self {
        Self {
            polling_rate_hz: config.polling_rate.to_hz(),
            lift_off_distance_mm: config.lift_off_distance.to_mm(),
            sleep_timeout_seconds: config.sleep_timeout_seconds,
            angle_snapping: config.angle_snapping,
            ripple_control: config.ripple_control,
            high_speed_mode: config.high_speed_mode,
            long_distance_mode: config.long_distance_mode,
        }
    }
}
