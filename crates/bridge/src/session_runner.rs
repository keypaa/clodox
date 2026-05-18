use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;

use serde::{Deserialize, Serialize};
use tokio::sync::RwLock;
use tokio::task::JoinHandle;
use tracing::{debug, info, warn};

/// Session runner state.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum RunnerState {
    Idle,
    Running,
    Paused,
    Stopping,
    Error,
}

/// Session execution configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionConfig {
    pub session_id: String,
    pub working_dir: String,
    pub model: String,
    pub api_key: String,
    pub permission_mode: String,
    pub max_tokens: u64,
    pub timeout_secs: u64,
    pub environment: HashMap<String, String>,
}

/// Session execution result.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionResult {
    pub session_id: String,
    pub success: bool,
    pub exit_code: Option<i32>,
    pub duration_ms: u64,
    pub error: Option<String>,
    pub tokens_used: u64,
    pub cost_usd: f64,
}

/// Session runner — executes sessions in background threads.
pub struct SessionRunner {
    state: RwLock<RunnerState>,
    active_sessions: RwLock<HashMap<String, SessionHandle>>,
    completed_sessions: RwLock<Vec<SessionResult>>,
    max_concurrent: usize,
}

/// Handle to a running session.
struct SessionHandle {
    pub config: SessionConfig,
    pub started_at: std::time::SystemTime,
    pub task: JoinHandle<SessionResult>,
}

impl SessionRunner {
    pub fn new(max_concurrent: usize) -> Self {
        Self {
            state: RwLock::new(RunnerState::Idle),
            active_sessions: RwLock::new(HashMap::new()),
            completed_sessions: RwLock::new(Vec::new()),
            max_concurrent,
        }
    }

    /// Start a session in the background.
    pub async fn start_session(&self, config: SessionConfig) -> Result<String, String> {
        let current_state = *self.state.read().await;
        if current_state == RunnerState::Stopping {
            return Err("Runner is stopping".to_string());
        }

        let active = self.active_sessions.read().await;
        if active.len() >= self.max_concurrent {
            return Err(format!(
                "Maximum concurrent sessions reached ({})",
                self.max_concurrent
            ));
        }
        drop(active);

        let session_id = config.session_id.clone();
        let config_clone = config.clone();

        // Spawn the session task
        let task = tokio::spawn(async move {
            Self::run_session(config_clone).await
        });

        let handle = SessionHandle {
            config,
            started_at: std::time::SystemTime::now(),
            task,
        };

        self.active_sessions
            .write()
            .await
            .insert(session_id.clone(), handle);

        *self.state.write().await = RunnerState::Running;

        info!(session_id, "Session started");
        Ok(session_id)
    }

    /// Wait for a session to complete.
    pub async fn wait_for_session(&self, session_id: &str) -> Result<SessionResult, String> {
        let handle = self
            .active_sessions
            .write()
            .await
            .remove(session_id)
            .ok_or_else(|| format!("Session not found: {session_id}"))?;

        let result = handle
            .task
            .await
            .map_err(|e| format!("Session task panicked: {}", e))?;

        self.completed_sessions
            .write()
            .await
            .push(result.clone());

        // Update runner state
        if self.active_sessions.read().await.is_empty() {
            *self.state.write().await = RunnerState::Idle;
        }

        info!(session_id, success = result.success, "Session completed");
        Ok(result)
    }

    /// Stop a running session.
    pub async fn stop_session(&self, session_id: &str) -> Result<(), String> {
        let handle = self
            .active_sessions
            .write()
            .await
            .remove(session_id)
            .ok_or_else(|| format!("Session not found: {session_id}"))?;

        // Abort the task
        handle.task.abort();

        let result = SessionResult {
            session_id: session_id.to_string(),
            success: false,
            exit_code: None,
            duration_ms: handle.started_at.elapsed().unwrap_or_default().as_millis() as u64,
            error: Some("Session aborted".to_string()),
            tokens_used: 0,
            cost_usd: 0.0,
        };

        self.completed_sessions
            .write()
            .await
            .push(result);

        info!(session_id, "Session stopped");
        Ok(())
    }

    /// Stop all running sessions.
    pub async fn stop_all(&self) {
        *self.state.write().await = RunnerState::Stopping;

        let session_ids: Vec<String> = self.active_sessions.read().await.keys().cloned().collect();
        for session_id in session_ids {
            let _ = self.stop_session(&session_id).await;
        }

        *self.state.write().await = RunnerState::Idle;
        info!("All sessions stopped");
    }

    /// Pause the runner (prevent new sessions, keep running ones).
    pub async fn pause(&self) {
        *self.state.write().await = RunnerState::Paused;
        info!("Runner paused");
    }

    /// Resume the runner.
    pub async fn resume(&self) {
        *self.state.write().await = RunnerState::Idle;
        info!("Runner resumed");
    }

    /// Get the status of a specific session.
    pub async fn get_session_status(&self, session_id: &str) -> Option<SessionStatus> {
        let active = self.active_sessions.read().await;
        if let Some(handle) = active.get(session_id) {
            return Some(SessionStatus {
                session_id: session_id.to_string(),
                state: "running".to_string(),
                started_at: handle.started_at,
                duration: handle.started_at.elapsed().unwrap_or_default(),
                model: handle.config.model.clone(),
                working_dir: handle.config.working_dir.clone(),
            });
        }

        // Check completed sessions
        let completed = self.completed_sessions.read().await;
        if let Some(result) = completed.iter().find(|r| r.session_id == session_id) {
            return Some(SessionStatus {
                session_id: session_id.to_string(),
                state: if result.success {
                    "completed".to_string()
                } else {
                    "failed".to_string()
                },
                started_at: std::time::SystemTime::now(), // Approximate
                duration: Duration::from_millis(result.duration_ms),
                model: String::new(),
                working_dir: String::new(),
            });
        }

        None
    }

    /// Get all active session IDs.
    pub async fn get_active_sessions(&self) -> Vec<String> {
        self.active_sessions.read().await.keys().cloned().collect()
    }

    /// Get all completed session results.
    pub async fn get_completed_sessions(&self) -> Vec<SessionResult> {
        self.completed_sessions.read().await.clone()
    }

    /// Get active session count.
    pub async fn active_count(&self) -> usize {
        self.active_sessions.read().await.len()
    }

    /// Get completed session count.
    pub async fn completed_count(&self) -> usize {
        self.completed_sessions.read().await.len()
    }

    /// Get runner state.
    pub async fn get_state(&self) -> RunnerState {
        *self.state.read().await
    }

    /// Check if the runner is idle.
    pub async fn is_idle(&self) -> bool {
        *self.state.read().await == RunnerState::Idle
    }

    /// Internal: run a single session.
    async fn run_session(config: SessionConfig) -> SessionResult {
        let start = std::time::Instant::now();

        info!(session_id = config.session_id, "Running session");

        // In a full implementation, this would:
        // 1. Set up the working directory
        // 2. Initialize the QueryEngine with the model and API key
        // 3. Run the query loop
        // 4. Handle tool execution, permissions, compaction
        // 5. Track token usage and cost
        // 6. Return the result

        // For now, simulate a session run
        let timeout = Duration::from_secs(config.timeout_secs);

        // Simulate some work
        tokio::time::sleep(Duration::from_millis(100)).await;

        let duration_ms = start.elapsed().as_millis() as u64;

        // Check if we exceeded timeout
        if duration_ms > timeout.as_millis() as u64 {
            warn!(session_id = config.session_id, "Session timed out");
            return SessionResult {
                session_id: config.session_id,
                success: false,
                exit_code: None,
                duration_ms,
                error: Some("Session timed out".to_string()),
                tokens_used: 0,
                cost_usd: 0.0,
            };
        }

        SessionResult {
            session_id: config.session_id,
            success: true,
            exit_code: Some(0),
            duration_ms,
            error: None,
            tokens_used: 1000, // Placeholder
            cost_usd: 0.003,   // Placeholder
        }
    }
}

impl Default for SessionRunner {
    fn default() -> Self {
        Self::new(5)
    }
}

/// Session status information.
#[derive(Debug, Clone)]
pub struct SessionStatus {
    pub session_id: String,
    pub state: String,
    pub started_at: std::time::SystemTime,
    pub duration: Duration,
    pub model: String,
    pub working_dir: String,
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_config(session_id: &str) -> SessionConfig {
        SessionConfig {
            session_id: session_id.to_string(),
            working_dir: "/tmp".to_string(),
            model: "claude-sonnet-4-20250514".to_string(),
            api_key: "test-key".to_string(),
            permission_mode: "default".to_string(),
            max_tokens: 100_000,
            timeout_secs: 300,
            environment: HashMap::new(),
        }
    }

    #[test]
    fn test_runner_default() {
        let runner = SessionRunner::default();
        assert_eq!(runner.max_concurrent, 5);
    }

    #[tokio::test]
    async fn test_new_runner_is_idle() {
        let runner = SessionRunner::new(3);
        assert_eq!(runner.get_state().await, RunnerState::Idle);
        assert!(runner.is_idle().await);
        assert_eq!(runner.active_count().await, 0);
        assert_eq!(runner.completed_count().await, 0);
    }

    #[tokio::test]
    async fn test_start_session() {
        let runner = SessionRunner::new(3);
        let config = test_config("sess-1");
        let session_id = runner.start_session(config).await.unwrap();
        assert_eq!(session_id, "sess-1");
        assert_eq!(runner.get_state().await, RunnerState::Running);
        assert_eq!(runner.active_count().await, 1);
    }

    #[tokio::test]
    async fn test_wait_for_session() {
        let runner = SessionRunner::new(3);
        let config = test_config("sess-2");
        runner.start_session(config).await.unwrap();
        let result = runner.wait_for_session("sess-2").await.unwrap();
        assert!(result.success);
        assert_eq!(result.session_id, "sess-2");
        assert!(result.duration_ms > 0);
        assert_eq!(runner.completed_count().await, 1);
    }

    #[tokio::test]
    async fn test_wait_for_nonexistent_session() {
        let runner = SessionRunner::new(3);
        let result = runner.wait_for_session("nonexistent").await;
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("not found"));
    }

    #[tokio::test]
    async fn test_stop_session() {
        let runner = SessionRunner::new(3);
        let config = test_config("sess-3");
        runner.start_session(config).await.unwrap();
        runner.stop_session("sess-3").await.unwrap();
        assert_eq!(runner.active_count().await, 0);
        assert_eq!(runner.completed_count().await, 1);
    }

    #[tokio::test]
    async fn test_stop_nonexistent_session() {
        let runner = SessionRunner::new(3);
        let result = runner.stop_session("nonexistent").await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_stop_all() {
        let runner = SessionRunner::new(5);
        runner.start_session(test_config("s1")).await.unwrap();
        runner.start_session(test_config("s2")).await.unwrap();
        runner.start_session(test_config("s3")).await.unwrap();
        assert_eq!(runner.active_count().await, 3);
        runner.stop_all().await;
        assert_eq!(runner.active_count().await, 0);
        assert!(runner.is_idle().await);
    }

    #[tokio::test]
    async fn test_max_concurrent_sessions() {
        let runner = SessionRunner::new(2);
        runner.start_session(test_config("s1")).await.unwrap();
        runner.start_session(test_config("s2")).await.unwrap();
        let result = runner.start_session(test_config("s3")).await;
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("Maximum concurrent"));
    }

    #[tokio::test]
    async fn test_pause_and_resume() {
        let runner = SessionRunner::new(3);
        runner.pause().await;
        assert_eq!(runner.get_state().await, RunnerState::Paused);
        runner.resume().await;
        assert_eq!(runner.get_state().await, RunnerState::Idle);
    }

    #[tokio::test]
    async fn test_pause_sets_paused_state() {
        let runner = SessionRunner::new(3);
        runner.pause().await;
        assert_eq!(runner.get_state().await, RunnerState::Paused);
        runner.resume().await;
    }

    #[tokio::test]
    async fn test_stop_all_sets_idle() {
        let runner = SessionRunner::new(3);
        runner.start_session(test_config("s1")).await.unwrap();
        runner.stop_all().await;
        assert_eq!(runner.get_state().await, RunnerState::Idle);
    }

    #[tokio::test]
    async fn test_get_session_status_active() {
        let runner = SessionRunner::new(3);
        let config = test_config("s1");
        runner.start_session(config).await.unwrap();
        let status = runner.get_session_status("s1").await;
        assert!(status.is_some());
        let status = status.unwrap();
        assert_eq!(status.session_id, "s1");
        assert_eq!(status.state, "running");
        assert_eq!(status.model, "claude-sonnet-4-20250514");
    }

    #[tokio::test]
    async fn test_get_session_status_completed() {
        let runner = SessionRunner::new(3);
        let config = test_config("s1");
        runner.start_session(config).await.unwrap();
        runner.wait_for_session("s1").await.unwrap();
        let status = runner.get_session_status("s1").await;
        assert!(status.is_some());
        assert_eq!(status.unwrap().state, "completed");
    }

    #[tokio::test]
    async fn test_get_session_status_nonexistent() {
        let runner = SessionRunner::new(3);
        let status = runner.get_session_status("nonexistent").await;
        assert!(status.is_none());
    }

    #[tokio::test]
    async fn test_get_active_sessions() {
        let runner = SessionRunner::new(3);
        runner.start_session(test_config("s1")).await.unwrap();
        runner.start_session(test_config("s2")).await.unwrap();
        let active = runner.get_active_sessions().await;
        assert_eq!(active.len(), 2);
        assert!(active.contains(&"s1".to_string()));
        assert!(active.contains(&"s2".to_string()));
    }

    #[tokio::test]
    async fn test_get_completed_sessions() {
        let runner = SessionRunner::new(3);
        let config = test_config("s1");
        runner.start_session(config).await.unwrap();
        runner.wait_for_session("s1").await.unwrap();
        let completed = runner.get_completed_sessions().await;
        assert_eq!(completed.len(), 1);
        assert!(completed[0].success);
    }

    #[tokio::test]
    async fn test_session_result_aborted() {
        let runner = SessionRunner::new(3);
        runner.start_session(test_config("s1")).await.unwrap();
        runner.stop_session("s1").await.unwrap();
        let completed = runner.get_completed_sessions().await;
        assert_eq!(completed.len(), 1);
        assert!(!completed[0].success);
        assert!(completed[0].error.as_ref().unwrap().contains("aborted"));
    }
}
