use std::collections::HashMap;
use std::sync::Arc;

use serde::{Deserialize, Serialize};
use tokio::sync::RwLock;
use tracing::{debug, info, warn};

/// MCP server connection status.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ServerStatus {
    Disconnected,
    Connecting,
    Connected,
    Error,
}

/// MCP server configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct McpServerConfig {
    pub name: String,
    pub command: String,
    pub args: Vec<String>,
    pub env: HashMap<String, String>,
    pub enabled: bool,
}

/// MCP tool discovered from a server.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct McpToolInfo {
    pub server_name: String,
    pub tool_name: String,
    pub description: String,
    pub input_schema: serde_json::Value,
}

/// MCP resource discovered from a server.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct McpResourceInfo {
    pub server_name: String,
    pub uri: String,
    pub name: String,
    pub description: String,
    pub mime_type: Option<String>,
}

/// MCP server state.
#[derive(Debug, Clone)]
pub struct McpServerState {
    pub config: McpServerConfig,
    pub status: ServerStatus,
    pub tools: Vec<McpToolInfo>,
    pub resources: Vec<McpResourceInfo>,
    pub error: Option<String>,
    pub pid: Option<u32>,
    pub started_at: Option<std::time::SystemTime>,
}

impl McpServerState {
    fn from_config(config: McpServerConfig) -> Self {
        Self {
            config,
            status: ServerStatus::Disconnected,
            tools: Vec::new(),
            resources: Vec::new(),
            error: None,
            pid: None,
            started_at: None,
        }
    }
}

/// Events emitted by the MCP service.
#[derive(Debug, Clone)]
pub enum McpEvent {
    ServerStarted { name: String },
    ServerStopped { name: String },
    ServerError { name: String, error: String },
    ToolsUpdated { name: String, count: usize },
    ResourcesUpdated { name: String, count: usize },
}

/// MCP service — manages server lifecycle, connection status, and tool discovery.
pub struct McpService {
    servers: RwLock<HashMap<String, McpServerState>>,
    event_tx: tokio::sync::broadcast::Sender<McpEvent>,
}

impl McpService {
    pub fn new() -> Self {
        let (event_tx, _) = tokio::sync::broadcast::channel(64);
        Self {
            servers: RwLock::new(HashMap::new()),
            event_tx,
        }
    }

    /// Subscribe to MCP events.
    pub fn subscribe(&self) -> tokio::sync::broadcast::Receiver<McpEvent> {
        self.event_tx.subscribe()
    }

    /// Register an MCP server configuration (does not start it).
    pub async fn register_server(&self, config: McpServerConfig) {
        let name = config.name.clone();
        let mut servers = self.servers.write().await;
        servers.insert(name.clone(), McpServerState::from_config(config));
        info!(name, "MCP server registered");
    }

    /// Start an MCP server process.
    pub async fn start_server(&self, name: &str) -> Result<(), String> {
        let mut servers = self.servers.write().await;
        let state = servers
            .get_mut(name)
            .ok_or_else(|| format!("Server not found: {name}"))?;

        if !state.config.enabled {
            return Err(format!("Server {name} is disabled"));
        }

        state.status = ServerStatus::Connecting;
        state.error = None;

        // Spawn the server process
        let mut cmd = tokio::process::Command::new(&state.config.command);
        cmd.args(&state.config.args);

        for (key, value) in &state.config.env {
            cmd.env(key, value);
        }

        cmd.stdin(std::process::Stdio::piped())
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::piped());

        match cmd.spawn() {
            Ok(mut child) => {
                let pid = child.id();
                state.pid = pid;
                state.started_at = Some(std::time::SystemTime::now());
                state.status = ServerStatus::Connected;

                info!(name, pid, "MCP server started");

                let _ = self.event_tx.send(McpEvent::ServerStarted {
                    name: name.to_string(),
                });

                // Discover tools and resources
                let _ = self.discover_tools(name, &mut child).await;
                let _ = self.discover_resources(name, &mut child).await;

                Ok(())
            }
            Err(e) => {
                state.status = ServerStatus::Error;
                state.error = Some(e.to_string());
                warn!(name, error = %e, "Failed to start MCP server");

                let _ = self.event_tx.send(McpEvent::ServerError {
                    name: name.to_string(),
                    error: e.to_string(),
                });

                Err(e.to_string())
            }
        }
    }

    /// Stop an MCP server process.
    pub async fn stop_server(&self, name: &str) -> Result<(), String> {
        let mut servers = self.servers.write().await;
        let state = servers
            .get_mut(name)
            .ok_or_else(|| format!("Server not found: {name}"))?;

        if let Some(pid) = state.pid {
            // Kill the process using platform-specific approach
            #[cfg(unix)]
            {
                let _ = std::process::Command::new("kill")
                    .arg(pid.to_string())
                    .output();
            }
            #[cfg(not(unix))]
            {
                let _ = std::process::Command::new("taskkill")
                    .args(["/PID", &pid.to_string(), "/F"])
                    .output();
            }
        }

        state.status = ServerStatus::Disconnected;
        state.pid = None;
        state.started_at = None;
        state.tools.clear();
        state.resources.clear();

        info!(name, "MCP server stopped");

        let _ = self
            .event_tx
            .send(McpEvent::ServerStopped { name: name.to_string() });

        Ok(())
    }

    /// Restart an MCP server.
    pub async fn restart_server(&self, name: &str) -> Result<(), String> {
        let _ = self.stop_server(name).await;
        tokio::time::sleep(std::time::Duration::from_millis(500)).await;
        self.start_server(name).await
    }

    /// Enable or disable an MCP server.
    pub async fn set_server_enabled(&self, name: &str, enabled: bool) -> Result<(), String> {
        let mut servers = self.servers.write().await;
        let state = servers
            .get_mut(name)
            .ok_or_else(|| format!("Server not found: {name}"))?;

        state.config.enabled = enabled;

        if !enabled && state.status == ServerStatus::Connected {
            drop(servers);
            self.stop_server(name).await?;
        }

        Ok(())
    }

    /// Discover tools from a running MCP server.
    async fn discover_tools(
        &self,
        name: &str,
        _child: &mut tokio::process::Child,
    ) -> Result<(), String> {
        // In a full implementation, this would use JSON-RPC over stdio
        // to send tools/list to the MCP server and parse the response.
        // For now, we record that discovery was attempted.
        debug!(name, "Tool discovery attempted");
        Ok(())
    }

    /// Discover resources from a running MCP server.
    async fn discover_resources(
        &self,
        name: &str,
        _child: &mut tokio::process::Child,
    ) -> Result<(), String> {
        // In a full implementation, this would use JSON-RPC over stdio
        // to send resources/list to the MCP server and parse the response.
        debug!(name, "Resource discovery attempted");
        Ok(())
    }

    /// Get the status of all registered servers.
    pub async fn get_server_status(&self) -> HashMap<String, ServerStatus> {
        let servers = self.servers.read().await;
        servers
            .iter()
            .map(|(name, state)| (name.clone(), state.status))
            .collect()
    }

    /// Get tools from all connected servers.
    pub async fn get_all_tools(&self) -> Vec<McpToolInfo> {
        let servers = self.servers.read().await;
        servers
            .values()
            .filter(|s| s.status == ServerStatus::Connected)
            .flat_map(|s| s.tools.clone())
            .collect()
    }

    /// Get resources from all connected servers.
    pub async fn get_all_resources(&self) -> Vec<McpResourceInfo> {
        let servers = self.servers.read().await;
        servers
            .values()
            .filter(|s| s.status == ServerStatus::Connected)
            .flat_map(|s| s.resources.clone())
            .collect()
    }

    /// Get a specific server's state.
    pub async fn get_server(&self, name: &str) -> Option<McpServerState> {
        self.servers.read().await.get(name).cloned()
    }

    /// Get count of connected servers.
    pub async fn connected_count(&self) -> usize {
        let servers = self.servers.read().await;
        servers
            .values()
            .filter(|s| s.status == ServerStatus::Connected)
            .count()
    }

    /// Remove a server from the registry.
    pub async fn remove_server(&self, name: &str) -> Result<(), String> {
        let _ = self.stop_server(name).await;
        let mut servers = self.servers.write().await;
        servers.remove(name);
        info!(name, "MCP server removed");
        Ok(())
    }
}

impl Default for McpService {
    fn default() -> Self {
        Self::new()
    }
}
