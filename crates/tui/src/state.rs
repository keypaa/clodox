use std::collections::HashMap;
use std::sync::{Arc, RwLock};

use cc_core::messages::Message;
use cc_core::permissions::ToolPermissionContext;
use cc_core::types::EffortValue;

/// Application state — shared across the TUI via `Arc<RwLock<AppState>>`.
///
/// This is a simple shared state pattern (not Redux). Ratatui redraws everything
/// every frame with built-in diff, so we don't need fine-grained subscriptions.
#[derive(Debug, Clone)]
pub struct AppState {
    /// Conversation messages.
    pub messages: Vec<Message>,
    /// Permission context (mode, directories, etc.).
    pub tool_permission_context: ToolPermissionContext,
    /// Background agent tasks.
    pub tasks: Vec<AgentTask>,
    /// Currently viewed subagent task ID.
    pub viewing_agent_task_id: Option<String>,
    /// Expanded view: 'tasks' | 'teammates' | None.
    pub expanded_view: Option<ExpandedView>,
    /// Current model.
    pub main_loop_model: String,
    /// Whether thinking is enabled.
    pub thinking_enabled: bool,
    /// Fast mode toggle.
    pub fast_mode: bool,
    /// Effort level.
    pub effort: EffortValue,
    /// Selected footer pill.
    pub footer_selection: Option<String>,
    /// Status line text.
    pub status_line_text: Option<String>,
    /// Prompt speculation state.
    pub speculation: Option<PromptSpeculation>,
    /// AI-generated suggestions.
    pub prompt_suggestions: Vec<String>,
    /// Whether transcript mode is active.
    pub transcript_mode: bool,
    /// Whether to show all messages in transcript.
    pub show_all_in_transcript: bool,
    /// Whether brief mode is active.
    pub brief_mode: bool,
    /// Whether vim mode is active.
    pub vim_mode: bool,
    /// Session ID.
    pub session_id: Option<String>,
    /// Total cost in USD.
    pub total_cost_usd: f64,
    /// Token counts.
    pub token_counts: TokenCounts,
    /// Whether a query is in progress.
    pub is_querying: bool,
    /// Whether the app is exiting.
    pub is_exiting: bool,
    /// Exit confirmation (Ctrl+C double-press).
    pub exit_confirmation: Option<ExitConfirmation>,
}

/// Token counts from API responses.
#[derive(Debug, Clone, Default)]
pub struct TokenCounts {
    pub input_tokens: u64,
    pub output_tokens: u64,
    pub cache_read_tokens: u64,
    pub cache_creation_tokens: u64,
}

/// Agent task state.
#[derive(Debug, Clone)]
pub struct AgentTask {
    pub id: String,
    pub name: String,
    pub status: AgentTaskStatus,
    pub progress: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AgentTaskStatus {
    Running,
    Completed,
    Failed,
    Cancelled,
}

/// Expanded view type.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ExpandedView {
    Tasks,
    Teammates,
}

/// Prompt speculation state.
#[derive(Debug, Clone)]
pub struct PromptSpeculation {
    pub speculation_text: String,
}

/// Exit confirmation state (for Ctrl+C double-press).
#[derive(Debug)]
pub struct ExitConfirmation {
    pub first_press_time: std::time::Instant,
}

impl Clone for ExitConfirmation {
    fn clone(&self) -> Self {
        Self {
            first_press_time: std::time::Instant::now(),
        }
    }
}

impl Default for AppState {
    fn default() -> Self {
        Self {
            messages: Vec::new(),
            tool_permission_context: ToolPermissionContext::default(),
            tasks: Vec::new(),
            viewing_agent_task_id: None,
            expanded_view: None,
            main_loop_model: "claude-sonnet-4-20250514".to_string(),
            thinking_enabled: false,
            fast_mode: false,
            effort: EffortValue::Medium,
            footer_selection: None,
            status_line_text: None,
            speculation: None,
            prompt_suggestions: Vec::new(),
            transcript_mode: false,
            show_all_in_transcript: false,
            brief_mode: false,
            vim_mode: false,
            session_id: None,
            total_cost_usd: 0.0,
            token_counts: TokenCounts::default(),
            is_querying: false,
            is_exiting: false,
            exit_confirmation: None,
        }
    }
}

/// Shared state wrapper.
pub type SharedState = Arc<RwLock<AppState>>;

/// Create a new shared state.
pub fn create_state() -> SharedState {
    Arc::new(RwLock::new(AppState::default()))
}

/// Read lock helper.
pub fn read_state(state: &SharedState) -> std::sync::RwLockReadGuard<AppState> {
    state.read().expect("State read lock poisoned")
}

/// Write lock helper.
pub fn write_state(state: &SharedState) -> std::sync::RwLockWriteGuard<AppState> {
    state.write().expect("State write lock poisoned")
}

/// Clone state for snapshotting (used during render).
pub fn snapshot_state(state: &SharedState) -> AppState {
    read_state(state).clone()
}
