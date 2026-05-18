use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};

use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::cli_args::Cli;

/// Session metadata stored alongside messages.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SessionMeta {
    /// Session ID.
    pub session_id: String,
    /// Display name/title.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    /// Model used.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub model: Option<String>,
    /// Total cost in USD.
    #[serde(default)]
    pub total_cost_usd: f64,
    /// Total input tokens.
    #[serde(default)]
    pub total_input_tokens: u64,
    /// Total output tokens.
    #[serde(default)]
    pub total_output_tokens: u64,
    /// Total cache read tokens.
    #[serde(default)]
    pub total_cache_read_tokens: u64,
    /// Total cache creation tokens.
    #[serde(default)]
    pub total_cache_creation_tokens: u64,
    /// Number of turns.
    #[serde(default)]
    pub num_turns: u64,
    /// Session start time (ISO 8601).
    pub started_at: String,
    /// Session end time (ISO 8601).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ended_at: Option<String>,
    /// Stop reason.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub stop_reason: Option<String>,
    /// Working directory.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cwd: Option<String>,
    /// Permission mode.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub permission_mode: Option<String>,
}

impl SessionMeta {
    pub fn new(session_id: &str, cli: &Cli) -> Self {
        Self {
            session_id: session_id.to_string(),
            name: cli.name.clone(),
            model: cli.model.clone(),
            total_cost_usd: 0.0,
            total_input_tokens: 0,
            total_output_tokens: 0,
            total_cache_read_tokens: 0,
            total_cache_creation_tokens: 0,
            num_turns: 0,
            started_at: chrono::Utc::now().to_rfc3339(),
            ended_at: None,
            stop_reason: None,
            cwd: std::env::current_dir()
                .ok()
                .map(|p| p.to_string_lossy().to_string()),
            permission_mode: cli.permission_mode.map(|m| format!("{:?}", m)),
        }
    }
}

/// Session state manager.
pub struct Session {
    /// Session ID.
    pub id: String,
    /// Session metadata.
    pub meta: SessionMeta,
    /// Session directory path.
    pub dir: PathBuf,
    /// Whether the session is active.
    active: AtomicBool,
}

impl Session {
    /// Create a new session.
    pub fn new(cli: &Cli) -> anyhow::Result<Self> {
        let id = cli
            .session_id
            .clone()
            .unwrap_or_else(|| Uuid::new_v4().to_string());

        let meta = SessionMeta::new(&id, cli);
        let dir = session_dir(&id)?;

        // Create session directory
        std::fs::create_dir_all(&dir)?;

        // Write lock file (PID-based concurrent session detection)
        let lock_path = dir.join("lock");
        std::fs::write(&lock_path, std::process::id().to_string())?;

        // Write initial meta
        Self::write_meta(&dir, &meta)?;

        Ok(Self {
            id,
            meta,
            dir,
            active: AtomicBool::new(true),
        })
    }

    /// Load an existing session.
    pub fn load(session_id: &str) -> anyhow::Result<Self> {
        let dir = session_dir(session_id)?;
        if !dir.exists() {
            anyhow::bail!("Session not found: {}", session_id);
        }

        let meta = Self::read_meta(&dir)?;

        Ok(Self {
            id: session_id.to_string(),
            meta,
            dir,
            active: AtomicBool::new(true),
        })
    }

    /// Find the most recent session for --continue.
    pub fn find_most_recent() -> anyhow::Result<Option<Self>> {
        let sessions_dir = sessions_dir()?;
        if !sessions_dir.exists() {
            return Ok(None);
        }

        let mut latest: Option<(std::time::SystemTime, String)> = None;

        for entry in std::fs::read_dir(&sessions_dir)? {
            let entry = entry?;
            let path = entry.path();
            if !path.is_dir() {
                continue;
            }

            let meta_path = path.join("meta.json");
            if !meta_path.exists() {
                continue;
            }

            if let Ok(metadata) = std::fs::metadata(&path) {
                if let Ok(modified) = metadata.modified() {
                    let session_id = path
                        .file_name()
                        .and_then(|n| n.to_str())
                        .unwrap_or("")
                        .to_string();

                    match &latest {
                        Some((latest_time, _)) if modified > *latest_time => {
                            latest = Some((modified, session_id));
                        }
                        None => {
                            latest = Some((modified, session_id));
                        }
                        _ => {}
                    }
                }
            }
        }

        if let Some((_, session_id)) = latest {
            Ok(Some(Self::load(&session_id)?))
        } else {
            Ok(None)
        }
    }

    /// Append a message to the session log.
    pub fn append_message(&self, message: &serde_json::Value) -> anyhow::Result<()> {
        let messages_path = self.dir.join("messages.jsonl");
        let file = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(&messages_path)?;

        use std::io::Write;
        let mut writer = std::io::BufWriter::new(file);
        writeln!(writer, "{}", serde_json::to_string(message)?)?;
        writer.flush()?;

        Ok(())
    }

    /// Read all messages from the session log.
    pub fn read_messages(&self) -> anyhow::Result<Vec<serde_json::Value>> {
        let messages_path = self.dir.join("messages.jsonl");
        if !messages_path.exists() {
            return Ok(Vec::new());
        }

        let content = std::fs::read_to_string(&messages_path)?;
        let messages = content
            .lines()
            .filter(|l| !l.trim().is_empty())
            .filter_map(|l| serde_json::from_str(l).ok())
            .collect();

        Ok(messages)
    }

    /// Update session metadata.
    pub fn update_meta(&mut self, meta: &SessionMeta) -> anyhow::Result<()> {
        self.meta = meta.clone();
        Self::write_meta(&self.dir, meta)
    }

    /// Mark the session as ended.
    pub fn end(&mut self, stop_reason: &str) -> anyhow::Result<()> {
        self.active.store(false, Ordering::SeqCst);
        self.meta.ended_at = Some(chrono::Utc::now().to_rfc3339());
        self.meta.stop_reason = Some(stop_reason.to_string());
        Self::write_meta(&self.dir, &self.meta)?;

        // Remove lock file
        let lock_path = self.dir.join("lock");
        let _ = std::fs::remove_file(&lock_path);

        Ok(())
    }

    /// Check if the session is active.
    pub fn is_active(&self) -> bool {
        self.active.load(Ordering::SeqCst)
    }

    /// Write meta.json to the session directory.
    fn write_meta(dir: &Path, meta: &SessionMeta) -> anyhow::Result<()> {
        let meta_path = dir.join("meta.json");
        let content = serde_json::to_string_pretty(meta)?;
        std::fs::write(&meta_path, content)?;
        Ok(())
    }

    /// Read meta.json from the session directory.
    fn read_meta(dir: &Path) -> anyhow::Result<SessionMeta> {
        let meta_path = dir.join("meta.json");
        let content = std::fs::read_to_string(&meta_path)?;
        let meta: SessionMeta = serde_json::from_str(&content)?;
        Ok(meta)
    }
}

/// Get the sessions directory (~/.claude/sessions).
fn sessions_dir() -> anyhow::Result<PathBuf> {
    let claude_dir = dirs::home_dir()
        .ok_or_else(|| anyhow::anyhow!("Home directory not found"))?
        .join(".claude");
    Ok(claude_dir.join("sessions"))
}

/// Get the directory for a specific session.
fn session_dir(session_id: &str) -> anyhow::Result<PathBuf> {
    Ok(sessions_dir()?.join(session_id))
}

/// Graceful shutdown handler.
pub struct GracefulShutdown {
    shutdown_requested: AtomicBool,
}

impl GracefulShutdown {
    pub fn new() -> Self {
        Self {
            shutdown_requested: AtomicBool::new(false),
        }
    }

    /// Request a graceful shutdown.
    pub fn request_shutdown(&self) {
        self.shutdown_requested.store(true, Ordering::SeqCst);
    }

    /// Check if shutdown has been requested.
    pub fn is_shutdown_requested(&self) -> bool {
        self.shutdown_requested.load(Ordering::SeqCst)
    }

    /// Wait for a shutdown signal (Ctrl+C).
    pub async fn wait_for_signal(&self) -> anyhow::Result<()> {
        use tokio::signal;

        // Handle Ctrl+C
        signal::ctrl_c().await?;

        tracing::info!("Received Ctrl+C, requesting graceful shutdown");
        self.request_shutdown();

        Ok(())
    }
}

impl Default for GracefulShutdown {
    fn default() -> Self {
        Self::new()
    }
}
