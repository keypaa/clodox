use std::path::{Path, PathBuf};


use crate::types::SettingSource;

/// Get the path to the Claude config directory (~/.claude).
pub fn claude_dir() -> Option<PathBuf> {
    dirs::home_dir().map(|h| h.join(".claude"))
}

/// Get the path to user settings (~/.claude/settings.json).
pub fn user_settings_path() -> Option<PathBuf> {
    claude_dir().map(|d| d.join("settings.json"))
}

/// Get the path to project settings (<cwd>/.claude/settings.json).
pub fn project_settings_path() -> Option<PathBuf> {
    std::env::current_dir()
        .ok()
        .map(|cwd| cwd.join(".claude").join("settings.json"))
}

/// Get the path to local settings (<cwd>/.claude/settings.local.json).
pub fn local_settings_path() -> Option<PathBuf> {
    std::env::current_dir()
        .ok()
        .map(|cwd| cwd.join(".claude").join("settings.local.json"))
}

/// Get the path to the global config (~/.claude/config.json).
pub fn global_config_path() -> Option<PathBuf> {
    claude_dir().map(|d| d.join("config.json"))
}

/// Parse a settings file from a path.
pub fn parse_settings_file(path: &Path) -> Result<serde_json::Value, SettingsError> {
    let content = std::fs::read_to_string(path).map_err(|e| SettingsError::ReadFailed {
        path: path.to_path_buf(),
        source: e,
    })?;

    let value: serde_json::Value =
        serde_json::from_str(&content).map_err(|e| SettingsError::ParseFailed {
            path: path.to_path_buf(),
            source: e,
        })?;

    Ok(value)
}

/// Write settings to a file, preserving existing content on parse errors.
pub fn write_settings_file(path: &Path, value: &serde_json::Value) -> Result<(), SettingsError> {
    // Ensure parent directory exists
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(|e| SettingsError::WriteFailed {
            path: path.to_path_buf(),
            source: e,
        })?;
    }

    let content = serde_json::to_string_pretty(value).map_err(|e| SettingsError::SerializeFailed {
        source: e,
    })?;

    std::fs::write(path, content).map_err(|e| SettingsError::WriteFailed {
        path: path.to_path_buf(),
        source: e,
    })?;

    Ok(())
}

/// Update settings for a specific source.
pub fn update_settings_for_source(
    source: SettingSource,
    updates: &serde_json::Value,
) -> Result<(), SettingsError> {
    let path = match source {
        SettingSource::UserSettings => {
            user_settings_path().ok_or(SettingsError::NoHomeDir)?
        }
        SettingSource::ProjectSettings => {
            project_settings_path().ok_or(SettingsError::NoCwd)?
        }
        SettingSource::LocalSettings => {
            local_settings_path().ok_or(SettingsError::NoCwd)?
        }
        _ => return Err(SettingsError::InvalidSource),
    };

    let existing = if path.exists() {
        parse_settings_file(&path).unwrap_or(serde_json::Value::Object(serde_json::Map::new()))
    } else {
        serde_json::Value::Object(serde_json::Map::new())
    };

    let merged = crate::settings::deep_merge(&existing, updates);
    write_settings_file(&path, &merged)?;

    Ok(())
}

/// Get enabled setting sources based on the --setting-sources flag.
pub fn get_enabled_setting_sources(sources_flag: Option<&str>) -> Vec<SettingSource> {
    if let Some(flag) = sources_flag {
        let mut enabled = Vec::new();
        for part in flag.split(',') {
            match part.trim() {
                "user" => enabled.push(SettingSource::UserSettings),
                "project" => enabled.push(SettingSource::ProjectSettings),
                "local" => enabled.push(SettingSource::LocalSettings),
                _ => {}
            }
        }
        enabled
    } else {
        // Default: all sources
        vec![
            SettingSource::UserSettings,
            SettingSource::ProjectSettings,
            SettingSource::LocalSettings,
        ]
    }
}

/// Settings error enumeration.
#[derive(Debug, thiserror::Error)]
pub enum SettingsError {
    #[error("Failed to read settings file: {path}")]
    ReadFailed {
        path: PathBuf,
        source: std::io::Error,
    },

    #[error("Failed to parse settings file: {path}")]
    ParseFailed {
        path: PathBuf,
        source: serde_json::Error,
    },

    #[error("Failed to write settings file: {path}")]
    WriteFailed {
        path: PathBuf,
        source: std::io::Error,
    },

    #[error("Failed to serialize settings")]
    SerializeFailed { source: serde_json::Error },

    #[error("Home directory not found")]
    NoHomeDir,

    #[error("Current working directory not found")]
    NoCwd,

    #[error("Invalid setting source")]
    InvalidSource,

    #[error("Settings validation failed: {0}")]
    ValidationFailed(String),
}
