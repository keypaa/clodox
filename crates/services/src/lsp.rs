use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use serde::{Deserialize, Serialize};
use tokio::sync::RwLock;
use tracing::{debug, info, warn};

/// LSP server configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LspServerConfig {
    pub name: String,
    pub command: String,
    pub args: Vec<String>,
    pub file_patterns: Vec<String>,
    pub enabled: bool,
}

/// LSP server connection status.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum LspServerStatus {
    Disconnected,
    Starting,
    Connected,
    Error,
}

/// Diagnostic severity from LSP.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum DiagnosticSeverity {
    Error,
    Warning,
    Information,
    Hint,
}

/// A single diagnostic from an LSP server.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Diagnostic {
    pub source: String,
    pub severity: DiagnosticSeverity,
    pub message: String,
    pub line: u32,
    pub column: u32,
    pub end_line: u32,
    pub end_column: u32,
    pub code: Option<String>,
}

/// Code action from LSP.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CodeAction {
    pub title: String,
    pub kind: String,
    pub is_preferred: bool,
    pub command: Option<String>,
}

/// Symbol information from LSP.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SymbolInfo {
    pub name: String,
    pub kind: String,
    pub file: String,
    pub line: u32,
    pub column: u32,
}

/// Completion item from LSP.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CompletionItem {
    pub label: String,
    pub kind: String,
    pub detail: Option<String>,
    pub documentation: Option<String>,
    pub sort_text: Option<String>,
}

/// LSP server state.
#[derive(Debug, Clone)]
pub struct LspServerState {
    pub config: LspServerConfig,
    pub status: LspServerStatus,
    pub root_uri: Option<String>,
    pub diagnostics: HashMap<String, Vec<Diagnostic>>,
    pub error: Option<String>,
    pub pid: Option<u32>,
}

impl LspServerState {
    fn from_config(config: LspServerConfig) -> Self {
        Self {
            config,
            status: LspServerStatus::Disconnected,
            root_uri: None,
            diagnostics: HashMap::new(),
            error: None,
            pid: None,
        }
    }
}

/// LSP service — language server protocol integration for code intelligence.
pub struct LspService {
    servers: RwLock<HashMap<String, LspServerState>>,
    /// Maps file extensions to server names.
    file_map: RwLock<HashMap<String, String>>,
}

impl LspService {
    pub fn new() -> Self {
        Self {
            servers: RwLock::new(HashMap::new()),
            file_map: RwLock::new(HashMap::new()),
        }
    }

    /// Register an LSP server configuration.
    pub async fn register_server(&self, config: LspServerConfig) {
        let name = config.name.clone();
        let extensions: Vec<String> = config
            .file_patterns
            .iter()
            .filter_map(|p| p.strip_prefix("*.").map(|s| s.to_string()))
            .collect();

        let mut servers = self.servers.write().await;
        servers.insert(name.clone(), LspServerState::from_config(config));

        let mut file_map = self.file_map.write().await;
        for ext in extensions {
            file_map.insert(ext, name.clone());
        }

        info!(name, "LSP server registered");
    }

    /// Start an LSP server for a given workspace root.
    pub async fn start_server(&self, name: &str, root_path: &Path) -> Result<(), String> {
        let mut servers = self.servers.write().await;
        let state = servers
            .get_mut(name)
            .ok_or_else(|| format!("Server not found: {name}"))?;

        if !state.config.enabled {
            return Err(format!("Server {name} is disabled"));
        }

        state.status = LspServerStatus::Starting;
        state.error = None;
        state.root_uri = Some(format!("file://{}", root_path.display()));

        // Spawn the LSP server process
        let mut cmd = tokio::process::Command::new(&state.config.command);
        cmd.args(&state.config.args);
        cmd.stdin(std::process::Stdio::piped())
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::piped());

        match cmd.spawn() {
            Ok(mut child) => {
                let pid = child.id();
                state.pid = pid;
                state.status = LspServerStatus::Connected;

                info!(name, pid, root = ?root_path, "LSP server started");

                // Note: Full LSP protocol implementation (JSON-RPC over stdio)
                // would go here: initialize, send didOpen, handle diagnostics, etc.
                // For now, we record that the server is connected.

                Ok(())
            }
            Err(e) => {
                state.status = LspServerStatus::Error;
                state.error = Some(e.to_string());
                warn!(name, error = %e, "Failed to start LSP server");
                Err(e.to_string())
            }
        }
    }

    /// Stop an LSP server.
    pub async fn stop_server(&self, name: &str) -> Result<(), String> {
        let mut servers = self.servers.write().await;
        let state = servers
            .get_mut(name)
            .ok_or_else(|| format!("Server not found: {name}"))?;

        if let Some(pid) = state.pid {
            #[cfg(unix)]
            {
                let _ = std::process::Command::new("kill").arg(pid.to_string()).output();
            }
            #[cfg(not(unix))]
            {
                let _ = std::process::Command::new("taskkill")
                    .args(["/PID", &pid.to_string(), "/F"])
                    .output();
            }
        }

        state.status = LspServerStatus::Disconnected;
        state.pid = None;
        state.diagnostics.clear();

        info!(name, "LSP server stopped");
        Ok(())
    }

    /// Find the appropriate LSP server for a given file path.
    pub async fn find_server_for_file(&self, path: &Path) -> Option<String> {
        let ext = path
            .extension()
            .and_then(|e| e.to_str())
            .unwrap_or("")
            .to_string();

        let file_map = self.file_map.read().await;
        file_map.get(&ext).cloned()
    }

    /// Get diagnostics for a file.
    pub async fn get_diagnostics(&self, file_path: &str) -> Vec<Diagnostic> {
        let servers = self.servers.read().await;
        servers
            .values()
            .filter(|s| s.status == LspServerStatus::Connected)
            .flat_map(|s| s.diagnostics.get(file_path).cloned().unwrap_or_default())
            .collect()
    }

    /// Get all diagnostics across all files.
    pub async fn get_all_diagnostics(&self) -> HashMap<String, Vec<Diagnostic>> {
        let servers = self.servers.read().await;
        let mut all = HashMap::new();
        for server in servers.values().filter(|s| s.status == LspServerStatus::Connected) {
            for (file, diags) in &server.diagnostics {
                all.entry(file.clone())
                    .or_insert_with(Vec::new)
                    .extend(diags.clone());
            }
        }
        all
    }

    /// Get code actions for a file at a given position.
    pub async fn get_code_actions(
        &self,
        _file_path: &str,
        _line: u32,
        _column: u32,
    ) -> Vec<CodeAction> {
        // Full implementation would send textDocument/codeAction to the LSP server
        Vec::new()
    }

    /// Get completions for a file at a given position.
    pub async fn get_completions(
        &self,
        _file_path: &str,
        _line: u32,
        _column: u32,
        _trigger_character: Option<&str>,
    ) -> Vec<CompletionItem> {
        // Full implementation would send textDocument/completion to the LSP server
        Vec::new()
    }

    /// Get symbol definitions across the workspace.
    pub async fn get_symbols(&self, _query: &str) -> Vec<SymbolInfo> {
        // Full implementation would send workspace/symbol to the LSP server
        Vec::new()
    }

    /// Get go-to-definition for a file position.
    pub async fn get_definition(
        &self,
        _file_path: &str,
        _line: u32,
        _column: u32,
    ) -> Option<SymbolInfo> {
        // Full implementation would send textDocument/definition to the LSP server
        None
    }

    /// Get hover information for a file position.
    pub async fn get_hover(
        &self,
        _file_path: &str,
        _line: u32,
        _column: u32,
    ) -> Option<String> {
        // Full implementation would send textDocument/hover to the LSP server
        None
    }

    /// Get server status.
    pub async fn get_server_status(&self, name: &str) -> Option<LspServerStatus> {
        self.servers.read().await.get(name).map(|s| s.status)
    }

    /// Get all connected server names.
    pub async fn get_connected_servers(&self) -> Vec<String> {
        self.servers
            .read()
            .await
            .values()
            .filter(|s| s.status == LspServerStatus::Connected)
            .map(|s| s.config.name.clone())
            .collect()
    }

    /// Get server count.
    pub async fn server_count(&self) -> usize {
        self.servers.read().await.len()
    }

    /// Get connected server count.
    pub async fn connected_count(&self) -> usize {
        self.servers
            .read()
            .await
            .values()
            .filter(|s| s.status == LspServerStatus::Connected)
            .count()
    }
}

impl Default for LspService {
    fn default() -> Self {
        Self::new()
    }
}
