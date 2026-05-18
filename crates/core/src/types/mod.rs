use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// A session ID uniquely identifies a Claude Code session.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct SessionId(pub Uuid);

/// An agent ID uniquely identifies a subagent within a session.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct AgentId(pub String);

impl AgentId {
    const PATTERN: &'static str = r"^a(?:.+-)?[0-9a-f]{16}$";

    pub fn new(id: String) -> Option<Self> {
        if regex::Regex::new(Self::PATTERN)
            .ok()
            .map(|r| r.is_match(&id))
            .unwrap_or(false)
        {
            Some(Self(id))
        } else {
            None
        }
    }

    pub fn new_unchecked(id: String) -> Self {
        Self(id)
    }
}

/// Validation result for tool input.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "result")]
pub enum ValidationResult {
    #[serde(rename = "true")]
    Valid,
    #[serde(rename = "false")]
    Invalid {
        message: String,
        error_code: i32,
    },
}

/// Spinner display mode.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum SpinnerMode {
    #[default]
    Normal,
    Compact,
    Verbose,
}

/// Theme name for UI styling.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum ThemeName {
    #[default]
    Light,
    Dark,
    System,
}

/// Theme configuration for UI colors.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Theme {
    pub name: ThemeName,
    pub colors: std::collections::HashMap<String, String>,
}

/// Query source for analytics tracking.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum QuerySource {
    Cli,
    Repl,
    Sdk,
    Bridge,
    Agent,
}

/// SDK status for session state.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum SdkStatus {
    Idle,
    Running,
    Paused,
    Error { message: String },
}

/// Effort level for API calls.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum EffortValue {
    #[default]
    Low,
    Medium,
    High,
}

/// Additional working directory with its source.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AdditionalWorkingDirectory {
    pub path: String,
    pub source: PermissionRuleSource,
}

/// Attribution state for git commits.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct AttributionState {
    pub attributed_commits: Vec<String>,
    pub pending_changes: Vec<String>,
}

/// File history state tracking.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct FileHistoryState {
    pub history: std::collections::HashMap<String, Vec<FileHistoryEntry>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileHistoryEntry {
    pub content: String,
    pub timestamp: chrono::DateTime<chrono::Utc>,
    pub tool_use_id: String,
}

/// Setting source enumeration.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum SettingSource {
    UserSettings,
    ProjectSettings,
    LocalSettings,
    Builtin,
    Mcp,
    Plugin,
    Bundled,
    Managed,
}

use crate::permissions::PermissionRuleSource;
