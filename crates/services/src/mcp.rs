use std::collections::HashMap;
use std::sync::Arc;

use rmcp::handler::client::ClientHandler;
use rmcp::model::{
    CallToolRequestParams, CallToolResult, ReadResourceRequestParams, ReadResourceResult,
};
use rmcp::service::{Peer, RoleClient};
use rmcp::transport::child_process::TokioChildProcess;
use rmcp::transport::streamable_http_client::StreamableHttpClientTransport;
use rmcp::ServiceExt;
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

/// A no-op client handler — we only need the client to send requests,
/// not to handle server-to-client callbacks (sampling, roots, etc.).
#[derive(Debug, Clone, Default)]
struct NoOpClientHandler;

impl ClientHandler for NoOpClientHandler {}

/// MCP server state.
pub struct McpServerState {
    pub config: McpServerConfig,
    pub status: ServerStatus,
    pub tools: Vec<McpToolInfo>,
    pub resources: Vec<McpResourceInfo>,
    pub error: Option<String>,
    pub peer: Option<Arc<Peer<RoleClient>>>,
    pub started_at: Option<std::time::SystemTime>,
}

impl std::fmt::Debug for McpServerState {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("McpServerState")
            .field("config", &self.config)
            .field("status", &self.status)
            .field("tools", &self.tools)
            .field("resources", &self.resources)
            .field("error", &self.error)
            .field("started_at", &self.started_at)
            .finish()
    }
}

impl McpServerState {
    fn from_config(config: McpServerConfig) -> Self {
        Self {
            config,
            status: ServerStatus::Disconnected,
            tools: Vec::new(),
            resources: Vec::new(),
            error: None,
            peer: None,
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

/// Remote MCP server configuration (HTTP/SSE transport).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RemoteMcpConfig {
    pub name: String,
    pub url: String,
    pub headers: HashMap<String, String>,
    pub enabled: bool,
}

/// Remote MCP server state.
pub struct RemoteMcpServerState {
    pub config: RemoteMcpConfig,
    pub status: ServerStatus,
    pub tools: Vec<McpToolInfo>,
    pub resources: Vec<McpResourceInfo>,
    pub error: Option<String>,
    pub peer: Option<Arc<Peer<RoleClient>>>,
    pub started_at: Option<std::time::SystemTime>,
}

impl std::fmt::Debug for RemoteMcpServerState {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("RemoteMcpServerState")
            .field("config", &self.config)
            .field("status", &self.status)
            .field("tools", &self.tools)
            .field("resources", &self.resources)
            .field("error", &self.error)
            .field("started_at", &self.started_at)
            .finish()
    }
}

impl RemoteMcpServerState {
    fn from_config(config: RemoteMcpConfig) -> Self {
        Self {
            config,
            status: ServerStatus::Disconnected,
            tools: Vec::new(),
            resources: Vec::new(),
            error: None,
            peer: None,
            started_at: None,
        }
    }
}

/// MCP service — manages server lifecycle, connection status, and tool discovery.
/// Supports both stdio-based local servers and HTTP/SSE remote servers.
pub struct McpService {
    servers: RwLock<HashMap<String, McpServerState>>,
    remote_servers: RwLock<HashMap<String, RemoteMcpServerState>>,
    event_tx: tokio::sync::broadcast::Sender<McpEvent>,
}

impl McpService {
    pub fn new() -> Self {
        let (event_tx, _) = tokio::sync::broadcast::channel(64);
        Self {
            servers: RwLock::new(HashMap::new()),
            remote_servers: RwLock::new(HashMap::new()),
            event_tx,
        }
    }

    /// Subscribe to MCP events.
    pub fn subscribe(&self) -> tokio::sync::broadcast::Receiver<McpEvent> {
        self.event_tx.subscribe()
    }

    // =========================================================================
    // Local (stdio) MCP servers
    // =========================================================================

    /// Register a local MCP server configuration (does not start it).
    pub async fn register_server(&self, config: McpServerConfig) {
        let name = config.name.clone();
        let mut servers = self.servers.write().await;
        servers.insert(name.clone(), McpServerState::from_config(config));
        info!(name, "MCP server registered");
    }

    /// Start a local MCP server process and connect via stdio transport.
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

        // Build the child process command
        let mut cmd = tokio::process::Command::new(&state.config.command);
        cmd.args(&state.config.args);
        for (key, value) in &state.config.env {
            cmd.env(key, value);
        }
        cmd.stdin(std::process::Stdio::piped())
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::piped());

        // Create stdio transport via rmcp's TokioChildProcess
        let transport = match TokioChildProcess::new(cmd) {
            Ok(t) => t,
            Err(e) => {
                state.status = ServerStatus::Error;
                state.error = Some(e.to_string());
                warn!(name, error = %e, "Failed to create MCP transport");
                let _ = self.event_tx.send(McpEvent::ServerError {
                    name: name.to_string(),
                    error: e.to_string(),
                });
                return Err(e.to_string());
            }
        };

        // Connect via rmcp service layer: handler.serve(transport)
        let handler = NoOpClientHandler;
        match handler.serve(transport).await {
            Ok(running_service) => {
                let peer = running_service.peer().clone();
                state.peer = Some(Arc::new(peer));
                state.status = ServerStatus::Connected;
                state.started_at = Some(std::time::SystemTime::now());

                info!(name, "MCP server connected via stdio");
                let _ = self.event_tx.send(McpEvent::ServerStarted {
                    name: name.to_string(),
                });

                // Discover tools and resources
                if let Err(e) = self.discover_tools_local(name).await {
                    warn!(name, error = %e, "Tool discovery failed");
                }
                if let Err(e) = self.discover_resources_local(name).await {
                    warn!(name, error = %e, "Resource discovery failed");
                }

                Ok(())
            }
            Err(e) => {
                state.status = ServerStatus::Error;
                state.error = Some(e.to_string());
                warn!(name, error = %e, "Failed to connect to MCP server");
                let _ = self.event_tx.send(McpEvent::ServerError {
                    name: name.to_string(),
                    error: e.to_string(),
                });
                Err(e.to_string())
            }
        }
    }

    /// Stop a local MCP server.
    pub async fn stop_server(&self, name: &str) -> Result<(), String> {
        let mut servers = self.servers.write().await;
        let state = servers
            .get_mut(name)
            .ok_or_else(|| format!("Server not found: {name}"))?;

        state.peer = None;
        state.status = ServerStatus::Disconnected;
        state.started_at = None;
        state.tools.clear();
        state.resources.clear();

        info!(name, "MCP server disconnected");
        let _ = self
            .event_tx
            .send(McpEvent::ServerStopped { name: name.to_string() });

        Ok(())
    }

    /// Restart a local MCP server.
    pub async fn restart_server(&self, name: &str) -> Result<(), String> {
        let _ = self.stop_server(name).await;
        tokio::time::sleep(std::time::Duration::from_millis(500)).await;
        self.start_server(name).await
    }

    /// Enable or disable a local MCP server.
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

    // =========================================================================
    // Remote (HTTP/SSE) MCP servers
    // =========================================================================

    /// Register a remote MCP server configuration (HTTP/SSE transport).
    pub async fn register_remote_server(&self, config: RemoteMcpConfig) {
        let name = config.name.clone();
        let url = config.url.clone();
        let mut servers = self.remote_servers.write().await;
        servers.insert(name.clone(), RemoteMcpServerState::from_config(config));
        info!(name, url = %url, "Remote MCP server registered");
    }

    /// Connect to a remote MCP server via HTTP/SSE transport.
    pub async fn connect_remote_server(&self, name: &str) -> Result<(), String> {
        let mut servers = self.remote_servers.write().await;
        let state = servers
            .get_mut(name)
            .ok_or_else(|| format!("Remote server not found: {name}"))?;

        if !state.config.enabled {
            return Err(format!("Remote server {name} is disabled"));
        }

        state.status = ServerStatus::Connecting;
        state.error = None;

        let base_url = state.config.url.clone();

        info!(name, url = %base_url, "Connecting to remote MCP server");

        // Create Streamable HTTP transport
        let transport = StreamableHttpClientTransport::from_uri(base_url);

        // Connect via rmcp service layer
        let handler = NoOpClientHandler;
        match handler.serve(transport).await {
            Ok(running_service) => {
                let peer = running_service.peer().clone();
                state.peer = Some(Arc::new(peer));
                state.status = ServerStatus::Connected;
                state.started_at = Some(std::time::SystemTime::now());

                info!(name, "Remote MCP server connected");
                let _ = self.event_tx.send(McpEvent::ServerStarted {
                    name: name.to_string(),
                });

                // Discover tools and resources
                if let Err(e) = self.discover_tools_remote(name).await {
                    warn!(name, error = %e, "Tool discovery failed");
                }
                if let Err(e) = self.discover_resources_remote(name).await {
                    warn!(name, error = %e, "Resource discovery failed");
                }

                Ok(())
            }
            Err(e) => {
                state.status = ServerStatus::Error;
                state.error = Some(e.to_string());
                warn!(name, error = %e, "Failed to connect to remote MCP server");
                let _ = self.event_tx.send(McpEvent::ServerError {
                    name: name.to_string(),
                    error: e.to_string(),
                });
                Err(e.to_string())
            }
        }
    }

    /// Disconnect from a remote MCP server.
    pub async fn disconnect_remote_server(&self, name: &str) -> Result<(), String> {
        let mut servers = self.remote_servers.write().await;
        let state = servers
            .get_mut(name)
            .ok_or_else(|| format!("Remote server not found: {name}"))?;

        state.peer = None;
        state.status = ServerStatus::Disconnected;
        state.started_at = None;
        state.tools.clear();
        state.resources.clear();

        info!(name, "Remote MCP server disconnected");
        let _ = self
            .event_tx
            .send(McpEvent::ServerStopped { name: name.to_string() });

        Ok(())
    }

    // =========================================================================
    // Tool discovery
    // =========================================================================

    /// Discover tools from a local MCP server.
    async fn discover_tools_local(&self, name: &str) -> Result<(), String> {
        let peer = {
            let servers = self.servers.read().await;
            let state = servers
                .get(name)
                .ok_or_else(|| format!("Server not found: {name}"))?;
            state
                .peer
                .clone()
                .ok_or_else(|| format!("Server {name} has no peer"))?
        };

        match peer.list_tools(None).await {
            Ok(list_tools_result) => {
                let tools: Vec<McpToolInfo> = list_tools_result
                    .tools
                    .into_iter()
                    .map(|tool| McpToolInfo {
                        server_name: name.to_string(),
                        tool_name: tool.name.to_string(),
                        description: tool.description.as_ref().map(|c| c.to_string()).unwrap_or_default(),
                        input_schema: serde_json::Value::Object((*tool.input_schema).clone()),
                    })
                    .collect();

                let count = tools.len();
                let mut servers = self.servers.write().await;
                if let Some(state) = servers.get_mut(name) {
                    state.tools = tools;
                }
                info!(name, count, "Tools discovered");
                let _ = self
                    .event_tx
                    .send(McpEvent::ToolsUpdated { name: name.to_string(), count });
                Ok(())
            }
            Err(e) => Err(format!("Failed to list tools from {name}: {e}")),
        }
    }

    /// Discover tools from a remote MCP server.
    async fn discover_tools_remote(&self, name: &str) -> Result<(), String> {
        let peer = {
            let servers = self.remote_servers.read().await;
            let state = servers
                .get(name)
                .ok_or_else(|| format!("Remote server not found: {name}"))?;
            state
                .peer
                .clone()
                .ok_or_else(|| format!("Remote server {name} has no peer"))?
        };

        match peer.list_tools(None).await {
            Ok(list_tools_result) => {
                let tools: Vec<McpToolInfo> = list_tools_result
                    .tools
                    .into_iter()
                    .map(|tool| McpToolInfo {
                        server_name: name.to_string(),
                        tool_name: tool.name.to_string(),
                        description: tool.description.as_ref().map(|c| c.to_string()).unwrap_or_default(),
                        input_schema: serde_json::Value::Object((*tool.input_schema).clone()),
                    })
                    .collect();

                let count = tools.len();
                let mut servers = self.remote_servers.write().await;
                if let Some(state) = servers.get_mut(name) {
                    state.tools = tools;
                }
                info!(name, count, "Tools discovered from remote server");
                let _ = self
                    .event_tx
                    .send(McpEvent::ToolsUpdated { name: name.to_string(), count });
                Ok(())
            }
            Err(e) => Err(format!("Failed to list tools from remote {name}: {e}")),
        }
    }

    /// Discover resources from a local MCP server.
    async fn discover_resources_local(&self, name: &str) -> Result<(), String> {
        let peer = {
            let servers = self.servers.read().await;
            let state = servers
                .get(name)
                .ok_or_else(|| format!("Server not found: {name}"))?;
            state
                .peer
                .clone()
                .ok_or_else(|| format!("Server {name} has no peer"))?
        };

        match peer.list_resources(None).await {
            Ok(list_resources_result) => {
                let resources: Vec<McpResourceInfo> = list_resources_result
                    .resources
                    .into_iter()
                    .map(|r| McpResourceInfo {
                        server_name: name.to_string(),
                        uri: r.uri.clone(),
                        name: r.name.clone(),
                        description: r.description.clone().unwrap_or_default(),
                        mime_type: r.mime_type.clone(),
                    })
                    .collect();

                let count = resources.len();
                let mut servers = self.servers.write().await;
                if let Some(state) = servers.get_mut(name) {
                    state.resources = resources;
                }
                debug!(name, count, "Resources discovered");
                let _ = self.event_tx.send(McpEvent::ResourcesUpdated {
                    name: name.to_string(),
                    count,
                });
                Ok(())
            }
            Err(e) => {
                debug!(name, error = %e, "Resource listing not supported or failed");
                Ok(())
            }
        }
    }

    /// Discover resources from a remote MCP server.
    async fn discover_resources_remote(&self, name: &str) -> Result<(), String> {
        let peer = {
            let servers = self.remote_servers.read().await;
            let state = servers
                .get(name)
                .ok_or_else(|| format!("Remote server not found: {name}"))?;
            state
                .peer
                .clone()
                .ok_or_else(|| format!("Remote server {name} has no peer"))?
        };

        match peer.list_resources(None).await {
            Ok(list_resources_result) => {
                let resources: Vec<McpResourceInfo> = list_resources_result
                    .resources
                    .into_iter()
                    .map(|r| McpResourceInfo {
                        server_name: name.to_string(),
                        uri: r.uri.clone(),
                        name: r.name.clone(),
                        description: r.description.clone().unwrap_or_default(),
                        mime_type: r.mime_type.clone(),
                    })
                    .collect();

                let count = resources.len();
                let mut servers = self.remote_servers.write().await;
                if let Some(state) = servers.get_mut(name) {
                    state.resources = resources;
                }
                debug!(name, count, "Resources discovered from remote server");
                let _ = self.event_tx.send(McpEvent::ResourcesUpdated {
                    name: name.to_string(),
                    count,
                });
                Ok(())
            }
            Err(e) => {
                debug!(name, error = %e, "Resource listing not supported or failed");
                Ok(())
            }
        }
    }

    // =========================================================================
    // Tool invocation
    // =========================================================================

    /// Call a tool on a local MCP server.
    pub async fn call_tool(
        &self,
        server_name: &str,
        tool_name: &str,
        arguments: serde_json::Value,
    ) -> Result<serde_json::Value, String> {
        let peer = {
            let servers = self.servers.read().await;
            let state = servers
                .get(server_name)
                .ok_or_else(|| format!("Server not found: {server_name}"))?;

            if state.status != ServerStatus::Connected {
                return Err(format!(
                    "Server {server_name} is not connected (status: {:?})",
                    state.status
                ));
            }

            state
                .peer
                .clone()
                .ok_or_else(|| format!("Server {server_name} has no peer"))?
        };

        let arguments_map = arguments
            .as_object()
            .map(|obj| obj.iter().map(|(k, v)| (k.clone(), v.clone())).collect());

        let call_result = peer
            .call_tool({
                let mut params = CallToolRequestParams::new(tool_name.to_string());
                params.arguments = arguments_map;
                params
            })
            .await
            .map_err(|e| format!("Failed to call tool {tool_name} on {server_name}: {e}"))?;

        Ok(extract_tool_result_json(&call_result))
    }

    /// Call a tool on a remote MCP server.
    pub async fn call_remote_tool(
        &self,
        server_name: &str,
        tool_name: &str,
        arguments: serde_json::Value,
    ) -> Result<serde_json::Value, String> {
        let peer = {
            let servers = self.remote_servers.read().await;
            let state = servers
                .get(server_name)
                .ok_or_else(|| format!("Remote server not found: {server_name}"))?;

            if state.status != ServerStatus::Connected {
                return Err(format!(
                    "Remote server {server_name} is not connected (status: {:?})",
                    state.status
                ));
            }

            state
                .peer
                .clone()
                .ok_or_else(|| format!("Remote server {server_name} has no peer"))?
        };

        let arguments_map = arguments
            .as_object()
            .map(|obj| obj.iter().map(|(k, v)| (k.clone(), v.clone())).collect());

        let call_result = peer
            .call_tool({
                let mut params = CallToolRequestParams::new(tool_name.to_string());
                params.arguments = arguments_map;
                params
            })
            .await
            .map_err(|e| {
                format!("Failed to call remote tool {tool_name} on {server_name}: {e}")
            })?;

        Ok(extract_tool_result_json(&call_result))
    }

    // =========================================================================
    // Resource reading
    // =========================================================================

    /// Read a resource from a local MCP server.
    pub async fn read_resource(
        &self,
        server_name: &str,
        uri: &str,
    ) -> Result<ReadResourceResult, String> {
        let peer = {
            let servers = self.servers.read().await;
            let state = servers
                .get(server_name)
                .ok_or_else(|| format!("Server not found: {server_name}"))?;
            state
                .peer
                .clone()
                .ok_or_else(|| format!("Server {server_name} has no peer"))?
        };

        peer.read_resource(ReadResourceRequestParams::new(uri))
            .await
            .map_err(|e| format!("Failed to read resource {uri} on {server_name}: {e}"))
    }

    /// Read a resource from a remote MCP server.
    pub async fn read_remote_resource(
        &self,
        server_name: &str,
        uri: &str,
    ) -> Result<ReadResourceResult, String> {
        let peer = {
            let servers = self.remote_servers.read().await;
            let state = servers
                .get(server_name)
                .ok_or_else(|| format!("Remote server not found: {server_name}"))?;
            state
                .peer
                .clone()
                .ok_or_else(|| format!("Remote server {server_name} has no peer"))?
        };

        peer.read_resource(ReadResourceRequestParams::new(uri))
            .await
            .map_err(|e| format!("Failed to read remote resource {uri} on {server_name}: {e}"))
    }

    // =========================================================================
    // Exa MCP integration
    // =========================================================================

    /// Exa MCP server URL.
    pub const EXA_MCP_URL: &'static str = "https://mcp.exa.ai/mcp";

    /// Register and connect to the Exa MCP server.
    pub async fn connect_exa(&self) -> Result<(), String> {
        let config = RemoteMcpConfig {
            name: "exa".to_string(),
            url: Self::EXA_MCP_URL.to_string(),
            headers: HashMap::new(),
            enabled: true,
        };

        self.register_remote_server(config).await;
        self.connect_remote_server("exa").await?;

        // Log discovered tools
        let tools = self.get_remote_server_tools("exa").await;
        info!(count = tools.len(), "Exa MCP server connected");

        Ok(())
    }

    // =========================================================================
    // Query methods
    // =========================================================================

    /// Get the status of all local servers.
    pub async fn get_server_status(&self) -> HashMap<String, ServerStatus> {
        let servers = self.servers.read().await;
        servers
            .iter()
            .map(|(name, state)| (name.clone(), state.status))
            .collect()
    }

    /// Get the status of all remote servers.
    pub async fn get_remote_server_status(&self) -> HashMap<String, ServerStatus> {
        let servers = self.remote_servers.read().await;
        servers
            .iter()
            .map(|(name, state)| (name.clone(), state.status))
            .collect()
    }

    /// Get tools from all connected local servers.
    pub async fn get_all_tools(&self) -> Vec<McpToolInfo> {
        let servers = self.servers.read().await;
        servers
            .values()
            .filter(|s| s.status == ServerStatus::Connected)
            .flat_map(|s| s.tools.clone())
            .collect()
    }

    /// Get tools from all connected remote servers.
    pub async fn get_all_remote_tools(&self) -> Vec<McpToolInfo> {
        let servers = self.remote_servers.read().await;
        servers
            .values()
            .filter(|s| s.status == ServerStatus::Connected)
            .flat_map(|s| s.tools.clone())
            .collect()
    }

    /// Get tools from a specific local server.
    pub async fn get_server_tools(&self, name: &str) -> Vec<McpToolInfo> {
        let servers = self.servers.read().await;
        servers
            .get(name)
            .filter(|s| s.status == ServerStatus::Connected)
            .map(|s| s.tools.clone())
            .unwrap_or_default()
    }

    /// Get tools from a specific remote server.
    pub async fn get_remote_server_tools(&self, name: &str) -> Vec<McpToolInfo> {
        let servers = self.remote_servers.read().await;
        servers
            .get(name)
            .filter(|s| s.status == ServerStatus::Connected)
            .map(|s| s.tools.clone())
            .unwrap_or_default()
    }

    /// Get resources from all connected local servers.
    pub async fn get_all_resources(&self) -> Vec<McpResourceInfo> {
        let servers = self.servers.read().await;
        servers
            .values()
            .filter(|s| s.status == ServerStatus::Connected)
            .flat_map(|s| s.resources.clone())
            .collect()
    }

    /// Get resources from all connected remote servers.
    pub async fn get_all_remote_resources(&self) -> Vec<McpResourceInfo> {
        let servers = self.remote_servers.read().await;
        servers
            .values()
            .filter(|s| s.status == ServerStatus::Connected)
            .flat_map(|s| s.resources.clone())
            .collect()
    }

    /// Get a specific local server's state.
    pub async fn get_server(&self, name: &str) -> Option<McpServerState> {
        self.servers.read().await.get(name).map(|state| {
            let mut copy = McpServerState::from_config(state.config.clone());
            copy.status = state.status;
            copy.tools = state.tools.clone();
            copy.resources = state.resources.clone();
            copy.error = state.error.clone();
            copy.started_at = state.started_at;
            copy
        })
    }

    /// Get a specific remote server's state.
    pub async fn get_remote_server(&self, name: &str) -> Option<RemoteMcpServerState> {
        self.remote_servers.read().await.get(name).map(|state| {
            let mut copy = RemoteMcpServerState::from_config(state.config.clone());
            copy.status = state.status;
            copy.tools = state.tools.clone();
            copy.resources = state.resources.clone();
            copy.error = state.error.clone();
            copy.started_at = state.started_at;
            copy
        })
    }

    /// Count of connected local servers.
    pub async fn connected_count(&self) -> usize {
        let servers = self.servers.read().await;
        servers
            .values()
            .filter(|s| s.status == ServerStatus::Connected)
            .count()
    }

    /// Count of connected remote servers.
    pub async fn remote_connected_count(&self) -> usize {
        let servers = self.remote_servers.read().await;
        servers
            .values()
            .filter(|s| s.status == ServerStatus::Connected)
            .count()
    }

    /// Remove a local server from the registry.
    pub async fn remove_server(&self, name: &str) -> Result<(), String> {
        let _ = self.stop_server(name).await;
        let mut servers = self.servers.write().await;
        servers.remove(name);
        info!(name, "MCP server removed");
        Ok(())
    }

    /// Remove a remote server from the registry.
    pub async fn remove_remote_server(&self, name: &str) -> Result<(), String> {
        let _ = self.disconnect_remote_server(name).await;
        let mut servers = self.remote_servers.write().await;
        servers.remove(name);
        info!(name, "Remote MCP server removed");
        Ok(())
    }
}

impl Default for McpService {
    fn default() -> Self {
        Self::new()
    }
}

/// Extract tool result content as JSON.
fn extract_tool_result_json(result: &CallToolResult) -> serde_json::Value {
    let content_text = result
        .content
        .iter()
        .filter_map(|c| {
            if let rmcp::model::RawContent::Text(text_block) = &c.raw {
                Some(text_block.text.clone())
            } else {
                None
            }
        })
        .collect::<Vec<String>>()
        .join("\n");

    // Try to parse as JSON, otherwise return as string wrapper
    if let Ok(json) = serde_json::from_str::<serde_json::Value>(&content_text) {
        json
    } else {
        serde_json::json!({ "content": content_text })
    }
}
