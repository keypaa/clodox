use std::collections::HashMap;

use serde::{Deserialize, Serialize};

/// Hook lifecycle configuration.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct HookSettings {
    /// Hooks keyed by event type.
    pub hooks: HashMap<String, Vec<HookConfig>>,
}

/// Hook event types.
pub mod hook_types {
    pub const PRE_TOOL_USE: &str = "PreToolUse";
    pub const POST_TOOL_USE: &str = "PostToolUse";
    pub const NOTIFICATION: &str = "Notification";
    pub const USER_PROMPT_SUBMIT: &str = "UserPromptSubmit";
    pub const SESSION_START: &str = "SessionStart";
    pub const SESSION_END: &str = "SessionEnd";
    pub const PRE_COMPACT: &str = "PreCompact";
    pub const POST_COMPACT: &str = "PostCompact";
    pub const ABORT: &str = "Abort";
}

/// Configuration for a single hook.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct HookConfig {
    /// Command to run.
    pub command: String,
    /// Environment variables.
    #[serde(default)]
    pub env: HashMap<String, String>,
    /// Timeout in milliseconds.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub timeout_ms: Option<u64>,
    /// What to do on failure.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub on_failure: Option<HookFailureMode>,
}

/// Hook failure handling mode.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum HookFailureMode {
    #[default]
    Continue,
    Abort,
    Warn,
}

impl HookSettings {
    /// Get hooks for a specific event type.
    pub fn get_hooks(&self, event_type: &str) -> Vec<&HookConfig> {
        self.hooks
            .get(event_type)
            .map(|hooks| hooks.iter().collect())
            .unwrap_or_default()
    }

    /// Check if any hooks are configured for an event type.
    pub fn has_hooks(&self, event_type: &str) -> bool {
        self.hooks.get(event_type).map_or(false, |h| !h.is_empty())
    }
}
