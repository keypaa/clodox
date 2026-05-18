use std::collections::HashMap;

use serde::{Deserialize, Serialize};

use crate::types::ThemeName;

/// UI-related settings.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct UiSettings {
    /// Theme setting (dark, light, auto).
    pub theme: Option<ThemeSetting>,
    /// Keybindings.
    pub keybindings: HashMap<String, KeyBinding>,
    /// Output style.
    pub output_style: Option<String>,
    /// Status line configuration.
    pub status_line: Option<StatusLineSettings>,
    /// Prefers reduced motion.
    pub prefers_reduced_motion: Option<bool>,
    /// Show spinner tree.
    pub show_spinner_tree: Option<bool>,
    /// Show expanded TODOs.
    pub show_expanded_todos: Option<bool>,
    /// Disable syntax highlighting.
    pub syntax_highlighting_disabled: Option<bool>,
}

/// Theme setting enum.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum ThemeSetting {
    #[default]
    Dark,
    Light,
    Auto,
}

impl From<ThemeName> for ThemeSetting {
    fn from(name: ThemeName) -> Self {
        match name {
            ThemeName::Dark => ThemeSetting::Dark,
            ThemeName::Light => ThemeSetting::Light,
            ThemeName::System => ThemeSetting::Auto,
        }
    }
}

impl From<ThemeSetting> for ThemeName {
    fn from(setting: ThemeSetting) -> Self {
        match setting {
            ThemeSetting::Dark => ThemeName::Dark,
            ThemeSetting::Light => ThemeName::Light,
            ThemeSetting::Auto => ThemeName::System,
        }
    }
}

/// Key binding configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct KeyBinding {
    /// Key combination (e.g., "Ctrl+C", "Tab").
    pub keys: String,
    /// Action to perform.
    pub action: String,
}

/// Status line settings.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct StatusLineSettings {
    /// Whether the status line is enabled.
    pub enabled: bool,
    /// Padding configuration.
    pub padding: Option<String>,
}
