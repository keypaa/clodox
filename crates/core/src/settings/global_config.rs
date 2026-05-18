use serde::{Deserialize, Serialize};

use super::ThemeSetting;

/// Global config stored at ~/.claude/config.json.
///
/// Separate from settings — stores app-level state, not user preferences.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct GlobalConfig {
    /// Migration version (for config schema migrations).
    pub migration_version: Option<u32>,
    /// Whether the user has completed onboarding.
    pub has_completed_onboarding: Option<bool>,
    /// Verbose mode flag.
    pub verbose: Option<bool>,
    /// Theme preference.
    pub theme: Option<ThemeSetting>,
    /// Show spinner tree.
    pub show_spinner_tree: Option<bool>,
    /// Show expanded TODOs.
    pub show_expanded_todos: Option<bool>,
    /// Trusted devices for remote control.
    pub trusted_devices: Vec<String>,
    /// Cached GrowthBook features.
    pub cached_growth_book_features: Option<serde_json::Value>,
}

impl GlobalConfig {
    /// Load global config from ~/.claude/config.json.
    pub fn load() -> Result<Self, crate::settings::SettingsError> {
        let path = super::sources::global_config_path()
            .ok_or(crate::settings::SettingsError::NoHomeDir)?;

        if path.exists() {
            let value = super::sources::parse_settings_file(&path)?;
            let config: GlobalConfig = serde_json::from_value(value).map_err(|e| {
                crate::settings::SettingsError::ParseFailed {
                    path,
                    source: e,
                }
            })?;
            Ok(config)
        } else {
            Ok(Self::default())
        }
    }

    /// Save global config to ~/.claude/config.json.
    pub fn save(&self) -> Result<(), crate::settings::SettingsError> {
        let path = super::sources::global_config_path()
            .ok_or(crate::settings::SettingsError::NoHomeDir)?;

        let value = serde_json::to_value(self).map_err(|e| {
            crate::settings::SettingsError::SerializeFailed { source: e }
        })?;

        super::sources::write_settings_file(&path, &value)
    }
}
