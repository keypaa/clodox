use serde::{Deserialize, Serialize};

/// Sandbox configuration.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct SandboxSettings {
    /// Whether sandbox is enabled.
    pub enabled: Option<bool>,
    /// Network access in sandbox.
    pub network: Option<bool>,
    /// Filesystem restrictions.
    pub filesystem: Option<FilesystemSandbox>,
    /// Commands excluded from sandbox.
    pub excluded_commands: Vec<String>,
}

/// Filesystem sandbox configuration.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct FilesystemSandbox {
    /// Allowed read paths.
    pub read: Vec<String>,
    /// Allowed write paths.
    pub write: Vec<String>,
}
