use std::path::PathBuf;
use std::time::Duration;

use serde::{Deserialize, Serialize};
use tokio::sync::RwLock;
use tracing::{info, warn};

/// Daemon state.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum DaemonState {
    Stopped,
    Starting,
    Running,
    Stopping,
    Error,
}

/// Daemon configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DaemonConfig {
    /// PID file path.
    pub pid_file: PathBuf,
    /// Log file path.
    pub log_file: PathBuf,
    /// Working directory.
    pub working_dir: PathBuf,
    /// Port for the daemon's HTTP API.
    pub port: u16,
    /// Maximum number of concurrent sessions.
    pub max_sessions: usize,
    /// Session idle timeout.
    pub idle_timeout_secs: u64,
}

impl Default for DaemonConfig {
    fn default() -> Self {
        Self {
            pid_file: PathBuf::from("/tmp/claude-code-daemon.pid"),
            log_file: PathBuf::from("/tmp/claude-code-daemon.log"),
            working_dir: std::env::current_dir().unwrap_or_default(),
            port: 8765,
            max_sessions: 10,
            idle_timeout_secs: 3600,
        }
    }
}

/// Session info tracked by the daemon.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DaemonSessionInfo {
    pub session_id: String,
    pub pid: u32,
    pub started_at: std::time::SystemTime,
    pub last_activity: std::time::SystemTime,
    pub working_dir: String,
    pub model: String,
    pub is_idle: bool,
}

/// Daemon process — background session runner with PID management.
pub struct DaemonProcess {
    config: DaemonConfig,
    state: RwLock<DaemonState>,
    sessions: RwLock<Vec<DaemonSessionInfo>>,
    pid: RwLock<Option<u32>>,
    error: RwLock<Option<String>>,
}

impl DaemonProcess {
    pub fn new(config: DaemonConfig) -> Self {
        Self {
            config,
            state: RwLock::new(DaemonState::Stopped),
            sessions: RwLock::new(Vec::new()),
            pid: RwLock::new(None),
            error: RwLock::new(None),
        }
    }

    /// Start the daemon process.
    pub async fn start(&self) -> Result<(), String> {
        let current_state = *self.state.read().await;
        if current_state == DaemonState::Running {
            return Err("Daemon is already running".to_string());
        }

        *self.state.write().await = DaemonState::Starting;
        *self.error.write().await = None;

        // Check if another daemon is already running
        if self.is_already_running().await {
            *self.state.write().await = DaemonState::Error;
            *self.error.write().await = Some("Another daemon instance is already running".to_string());
            return Err("Another daemon instance is already running".to_string());
        }

        // Write PID file
        let pid = std::process::id();
        match std::fs::write(&self.config.pid_file, pid.to_string()) {
            Ok(()) => {
                info!(pid, pid_file = %self.config.pid_file.display(), "PID file written");
            }
            Err(e) => {
                warn!(error = %e, "Failed to write PID file");
            }
        }

        *self.pid.write().await = Some(pid);
        *self.state.write().await = DaemonState::Running;

        info!(pid, port = self.config.port, "Daemon started");
        Ok(())
    }

    /// Stop the daemon process.
    pub async fn stop(&self) -> Result<(), String> {
        let current_state = *self.state.read().await;
        if current_state != DaemonState::Running {
            return Err("Daemon is not running".to_string());
        }

        *self.state.write().await = DaemonState::Stopping;

        // Kill all managed sessions
        let sessions = self.sessions.read().await.clone();
        for session in &sessions {
            let _ = self.kill_session(&session.session_id).await;
        }

        // Remove PID file
        let _ = std::fs::remove_file(&self.config.pid_file);

        *self.pid.write().await = None;
        *self.state.write().await = DaemonState::Stopped;
        self.sessions.write().await.clear();

        info!("Daemon stopped");
        Ok(())
    }

    /// Check if the daemon is already running by reading the PID file.
    pub async fn is_already_running(&self) -> bool {
        if !self.config.pid_file.exists() {
            return false;
        }

        match std::fs::read_to_string(&self.config.pid_file) {
            Ok(content) => {
                if let Ok(pid) = content.trim().parse::<u32>() {
                    // Check if the process is still alive by attempting to get its info
                    #[cfg(unix)]
                    {
                        std::path::Path::new(&format!("/proc/{}", pid)).exists()
                    }
                    #[cfg(not(unix))]
                    {
                        // On Windows, simplified check
                        false
                    }
                } else {
                    false
                }
            }
            Err(_) => false,
        }
    }

    /// Spawn a new session in the daemon.
    pub async fn spawn_session(
        &self,
        session_id: &str,
        working_dir: &str,
        model: &str,
    ) -> Result<u32, String> {
        if *self.state.read().await != DaemonState::Running {
            return Err("Daemon is not running".to_string());
        }

        let sessions = self.sessions.read().await;
        if sessions.len() >= self.config.max_sessions {
            return Err(format!("Maximum sessions reached ({})", self.config.max_sessions));
        }
        drop(sessions);

        // In a full implementation, this would spawn a new Claude Code process
        // as a child of the daemon, with the session ID and configuration passed
        // via environment variables or command-line arguments.
        let pid = std::process::id() + 1; // Placeholder

        let session_info = DaemonSessionInfo {
            session_id: session_id.to_string(),
            pid,
            started_at: std::time::SystemTime::now(),
            last_activity: std::time::SystemTime::now(),
            working_dir: working_dir.to_string(),
            model: model.to_string(),
            is_idle: false,
        };

        self.sessions.write().await.push(session_info);

        info!(session_id, pid, "Session spawned in daemon");
        Ok(pid)
    }

    /// Kill a specific session.
    pub async fn kill_session(&self, session_id: &str) -> Result<(), String> {
        let mut sessions = self.sessions.write().await;

        if let Some(pos) = sessions.iter().position(|s| s.session_id == session_id) {
            let session = sessions.remove(pos);

            // Kill the process
            #[cfg(unix)]
            {
                let _ = std::process::Command::new("kill")
                    .arg(session.pid.to_string())
                    .output();
            }
            #[cfg(not(unix))]
            {
                let _ = std::process::Command::new("taskkill")
                    .args(["/PID", &session.pid.to_string(), "/F"])
                    .output();
            }

            info!(session_id, pid = session.pid, "Session killed");
            Ok(())
        } else {
            Err(format!("Session not found: {session_id}"))
        }
    }

    /// Update session activity timestamp.
    pub async fn update_activity(&self, session_id: &str) {
        let mut sessions = self.sessions.write().await;
        if let Some(session) = sessions.iter_mut().find(|s| s.session_id == session_id) {
            session.last_activity = std::time::SystemTime::now();
            session.is_idle = false;
        }
    }

    /// Check for idle sessions and clean them up.
    pub async fn cleanup_idle_sessions(&self) -> Vec<String> {
        let timeout = Duration::from_secs(self.config.idle_timeout_secs);
        let now = std::time::SystemTime::now();
        let mut killed = Vec::new();

        let mut sessions = self.sessions.write().await;
        sessions.retain(|session| {
            if let Ok(elapsed) = now.duration_since(session.last_activity) {
                if elapsed > timeout {
                    killed.push(session.session_id.clone());
                    false
                } else {
                    true
                }
            } else {
                true
            }
        });

        if !killed.is_empty() {
            info!(count = killed.len(), "Idle sessions cleaned up");
        }

        killed
    }

    /// Get the current daemon state.
    pub async fn get_state(&self) -> DaemonState {
        *self.state.read().await
    }

    /// Get the daemon PID.
    pub async fn get_pid(&self) -> Option<u32> {
        *self.pid.read().await
    }

    /// Get the current error (if any).
    pub async fn get_error(&self) -> Option<String> {
        self.error.read().await.clone()
    }

    /// Get all session info.
    pub async fn get_sessions(&self) -> Vec<DaemonSessionInfo> {
        self.sessions.read().await.clone()
    }

    /// Get session count.
    pub async fn session_count(&self) -> usize {
        self.sessions.read().await.len()
    }

    /// Get the daemon config.
    pub fn get_config(&self) -> &DaemonConfig {
        &self.config
    }

    /// Check if running.
    pub async fn is_running(&self) -> bool {
        *self.state.read().await == DaemonState::Running
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn unique_pid_file() -> PathBuf {
        let id = std::process::id();
        let thread_id = format!("{:?}", std::thread::current().id());
        PathBuf::from(format!("/tmp/test-daemon-{}-{}.pid", id, thread_id))
    }

    fn test_config() -> DaemonConfig {
        DaemonConfig {
            pid_file: unique_pid_file(),
            log_file: PathBuf::from("/tmp/test-daemon.log"),
            working_dir: PathBuf::from("/tmp"),
            port: 9876,
            max_sessions: 5,
            idle_timeout_secs: 1,
        }
    }

    #[test]
    fn test_daemon_config_default() {
        let config = DaemonConfig::default();
        assert_eq!(config.port, 8765);
        assert_eq!(config.max_sessions, 10);
        assert_eq!(config.idle_timeout_secs, 3600);
    }

    #[tokio::test]
    async fn test_new_daemon_is_stopped() {
        let config = test_config();
        let daemon = DaemonProcess::new(config);
        assert_eq!(daemon.get_state().await, DaemonState::Stopped);
        assert!(!daemon.is_running().await);
        assert!(daemon.get_pid().await.is_none());
        assert_eq!(daemon.session_count().await, 0);
    }

    #[tokio::test]
    async fn test_start_daemon() {
        let config = test_config();
        let daemon = DaemonProcess::new(config);
        daemon.start().await.unwrap();
        assert_eq!(daemon.get_state().await, DaemonState::Running);
        assert!(daemon.is_running().await);
        assert!(daemon.get_pid().await.is_some());
    }

    #[tokio::test]
    async fn test_start_already_running() {
        let config = test_config();
        let daemon = DaemonProcess::new(config);
        daemon.start().await.unwrap();
        let result = daemon.start().await;
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("already running"));
    }

    #[tokio::test]
    async fn test_stop_daemon() {
        let config = test_config();
        let daemon = DaemonProcess::new(config);
        daemon.start().await.unwrap();
        daemon.stop().await.unwrap();
        assert_eq!(daemon.get_state().await, DaemonState::Stopped);
        assert!(!daemon.is_running().await);
        assert!(daemon.get_pid().await.is_none());
    }

    #[tokio::test]
    async fn test_stop_not_running() {
        let config = test_config();
        let daemon = DaemonProcess::new(config);
        let result = daemon.stop().await;
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("not running"));
    }

    #[tokio::test]
    async fn test_spawn_session() {
        let config = test_config();
        let daemon = DaemonProcess::new(config);
        daemon.start().await.unwrap();
        let pid = daemon.spawn_session("sess-1", "/tmp", "claude-sonnet-4").await.unwrap();
        assert!(pid > 0);
        assert_eq!(daemon.session_count().await, 1);
    }

    #[tokio::test]
    async fn test_spawn_session_not_running() {
        let config = test_config();
        let daemon = DaemonProcess::new(config);
        let result = daemon.spawn_session("sess-1", "/tmp", "claude-sonnet-4").await;
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("not running"));
    }

    #[tokio::test]
    async fn test_max_sessions() {
        let mut config = test_config();
        config.max_sessions = 2;
        let daemon = DaemonProcess::new(config);
        daemon.start().await.unwrap();
        daemon.spawn_session("s1", "/tmp", "model").await.unwrap();
        daemon.spawn_session("s2", "/tmp", "model").await.unwrap();
        let result = daemon.spawn_session("s3", "/tmp", "model").await;
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("Maximum sessions"));
    }

    #[tokio::test]
    async fn test_kill_session() {
        let config = test_config();
        let daemon = DaemonProcess::new(config);
        daemon.start().await.unwrap();
        daemon.spawn_session("s1", "/tmp", "model").await.unwrap();
        daemon.kill_session("s1").await.unwrap();
        assert_eq!(daemon.session_count().await, 0);
    }

    #[tokio::test]
    async fn test_kill_nonexistent_session() {
        let config = test_config();
        let daemon = DaemonProcess::new(config);
        daemon.start().await.unwrap();
        let result = daemon.kill_session("nonexistent").await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_update_activity() {
        let config = test_config();
        let daemon = DaemonProcess::new(config);
        daemon.start().await.unwrap();
        daemon.spawn_session("s1", "/tmp", "model").await.unwrap();
        daemon.update_activity("s1").await;
        let sessions = daemon.get_sessions().await;
        assert_eq!(sessions.len(), 1);
        assert!(!sessions[0].is_idle);
    }

    #[tokio::test]
    async fn test_cleanup_idle_sessions() {
        let config = test_config();
        let daemon = DaemonProcess::new(config);
        daemon.start().await.unwrap();
        daemon.spawn_session("s1", "/tmp", "model").await.unwrap();

        // Wait for idle timeout
        tokio::time::sleep(Duration::from_secs(2)).await;

        let killed = daemon.cleanup_idle_sessions().await;
        assert_eq!(killed.len(), 1);
        assert_eq!(killed[0], "s1");
        assert_eq!(daemon.session_count().await, 0);
    }

    #[tokio::test]
    async fn test_stop_kills_all_sessions() {
        let config = test_config();
        let daemon = DaemonProcess::new(config);
        daemon.start().await.unwrap();
        daemon.spawn_session("s1", "/tmp", "model").await.unwrap();
        daemon.spawn_session("s2", "/tmp", "model").await.unwrap();
        daemon.stop().await.unwrap();
        assert_eq!(daemon.session_count().await, 0);
    }

    #[tokio::test]
    async fn test_get_sessions() {
        let config = test_config();
        let daemon = DaemonProcess::new(config);
        daemon.start().await.unwrap();
        daemon.spawn_session("s1", "/tmp", "model").await.unwrap();
        let sessions = daemon.get_sessions().await;
        assert_eq!(sessions.len(), 1);
        assert_eq!(sessions[0].session_id, "s1");
        assert_eq!(sessions[0].working_dir, "/tmp");
        assert_eq!(sessions[0].model, "model");
    }

    #[tokio::test]
    async fn test_get_config() {
        let config = test_config();
        let daemon = DaemonProcess::new(config.clone());
        assert_eq!(daemon.get_config().port, config.port);
        assert_eq!(daemon.get_config().max_sessions, config.max_sessions);
    }

    #[tokio::test]
    async fn test_is_already_running_no_pid_file() {
        let config = DaemonConfig {
            pid_file: PathBuf::from("/tmp/nonexistent-daemon-12345.pid"),
            ..DaemonConfig::default()
        };
        let daemon = DaemonProcess::new(config);
        assert!(!daemon.is_already_running().await);
    }

    #[tokio::test]
    async fn test_get_error_initially_none() {
        let config = test_config();
        let daemon = DaemonProcess::new(config);
        assert!(daemon.get_error().await.is_none());
    }
}
