use std::collections::HashMap;

use serde::{Deserialize, Serialize};

/// MCP server configuration.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct McpSettings {
    /// MCP servers keyed by name.
    pub mcp_servers: HashMap<String, McpServerConfig>,
    /// Whether MCP is enabled.
    pub mcp_enabled: Option<bool>,
}

/// Configuration for a single MCP server.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct McpServerConfig {
    /// Server command.
    pub command: String,
    /// Server arguments.
    #[serde(default)]
    pub args: Vec<String>,
    /// Environment variables.
    #[serde(default)]
    pub env: HashMap<String, String>,
    /// Transport type.
    #[serde(default)]
    pub transport: McpTransport,
    /// Whether the server is disabled.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub disabled: Option<bool>,
    /// Timeout in milliseconds.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub timeout: Option<u64>,
}

/// MCP transport type.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "camelCase")]
pub enum McpTransport {
    #[default]
    Stdio,
    Sse { url: String },
    Http { url: String },
}

impl McpSettings {
    /// Get enabled MCP servers.
    pub fn enabled_servers(&self) -> Vec<(&str, &McpServerConfig)> {
        self.mcp_servers
            .iter()
            .filter(|(_, config)| !config.disabled.unwrap_or(false))
            .map(|(name, config)| (name.as_str(), config))
            .collect()
    }

    /// Check if MCP is enabled.
    pub fn is_enabled(&self) -> bool {
        self.mcp_enabled.unwrap_or(true)
    }
}
