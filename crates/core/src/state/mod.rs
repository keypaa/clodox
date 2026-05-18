use std::collections::HashMap;
use std::sync::{Arc, RwLock};

use serde::{Deserialize, Serialize};

use crate::messages::Message;
use crate::permissions::{ToolPermissionContext, ToolPermissionRulesBySource};
use crate::tools::Tools;
use crate::types::{
    AdditionalWorkingDirectory, AttributionState, EffortValue, FileHistoryState, ThemeName,
};

/// A minimal Redux-like store.
pub struct Store<T> {
    state: RwLock<T>,
    listeners: RwLock<Vec<Box<dyn Fn() + Send + Sync>>>,
    on_change: Option<Box<dyn Fn(&T, &T) + Send + Sync>>,
}

impl<T: Clone + PartialEq + Send + Sync> Store<T> {
    pub fn new(initial: T) -> Self {
        Self {
            state: RwLock::new(initial),
            listeners: RwLock::new(Vec::new()),
            on_change: None,
        }
    }

    pub fn with_on_change(initial: T, on_change: impl Fn(&T, &T) + Send + Sync + 'static) -> Self {
        Self {
            state: RwLock::new(initial),
            listeners: RwLock::new(Vec::new()),
            on_change: Some(Box::new(on_change)),
        }
    }

    pub fn get_state(&self) -> T {
        self.state.read().unwrap().clone()
    }

    pub fn set_state(&self, updater: impl FnOnce(&T) -> T) {
        let mut state = self.state.write().unwrap();
        let prev = state.clone();
        let next = updater(&state);
        if next == prev {
            return;
        }
        if let Some(on_change) = &self.on_change {
            on_change(&prev, &next);
        }
        *state = next;
        let listeners = self.listeners.read().unwrap();
        for listener in listeners.iter() {
            listener();
        }
    }

    pub fn subscribe(&self, listener: impl Fn() + Send + Sync + 'static) {
        self.listeners.write().unwrap().push(Box::new(listener));
    }
}

impl std::fmt::Debug for SpeculationState {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            SpeculationState::Idle => write!(f, "Idle"),
            SpeculationState::Active { id, .. } => f.debug_struct("Active").field("id", id).finish_non_exhaustive(),
        }
    }
}

/// Completion boundary for speculation.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "camelCase")]
pub enum CompletionBoundary {
    Complete {
        completed_at: i64,
        output_tokens: u64,
    },
    Bash {
        command: String,
        completed_at: i64,
    },
    Edit {
        tool_name: String,
        file_path: String,
        completed_at: i64,
    },
    DeniedTool {
        tool_name: String,
        detail: String,
        completed_at: i64,
    },
}

/// Speculation state for predictive pre-execution.
#[derive(Clone)]
pub enum SpeculationState {
    Idle,
    Active {
        id: String,
        abort: Arc<dyn Fn() + Send + Sync>,
        start_time: i64,
        suggestion_length: usize,
        tool_use_count: usize,
        is_pipelined: bool,
        boundary: Option<CompletionBoundary>,
    },
}

/// Footer item selection.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum FooterItem {
    Tasks,
    Tmux,
    Bagel,
    Teams,
    Bridge,
    Companion,
}

/// MCP server state.
#[derive(Debug, Clone, Default)]
pub struct McpState {
    pub clients: Vec<McpServerConnection>,
    pub tools: Tools,
    pub commands: Vec<crate::commands::Command>,
    pub resources: HashMap<String, Vec<ServerResource>>,
    pub plugin_reconnect_key: usize,
}

#[derive(Debug, Clone)]
pub struct McpServerConnection {
    pub name: String,
    pub connected: bool,
}

#[derive(Debug, Clone)]
pub struct ServerResource {
    pub name: String,
    pub uri: String,
}

/// Plugin state.
#[derive(Debug, Clone, Default)]
pub struct PluginState {
    pub enabled: Vec<LoadedPlugin>,
    pub disabled: Vec<LoadedPlugin>,
    pub commands: Vec<crate::commands::Command>,
    pub errors: Vec<PluginError>,
    pub installation_status: PluginInstallationStatus,
}

#[derive(Debug, Clone)]
pub struct LoadedPlugin {
    pub name: String,
    pub version: String,
    pub manifest: PluginManifest,
}

#[derive(Debug, Clone)]
pub struct PluginManifest {
    pub name: String,
    pub version: String,
    pub description: Option<String>,
}

#[derive(Debug, Clone)]
pub struct PluginError {
    pub plugin_name: String,
    pub error: String,
    pub context: Option<String>,
}

#[derive(Debug, Clone, Default)]
pub struct PluginInstallationStatus {
    pub marketplaces: Vec<MarketplaceStatus>,
}

#[derive(Debug, Clone)]
pub struct MarketplaceStatus {
    pub name: String,
    pub status: InstallationStatus,
    pub error: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InstallationStatus {
    Pending,
    Installing,
    Installed,
    Failed,
}

/// Task state.
#[derive(Debug, Clone)]
pub struct TaskState {
    pub id: String,
    pub name: String,
    pub status: TaskStatus,
    pub agent_id: Option<crate::types::AgentId>,
    pub created_at: chrono::DateTime<chrono::Utc>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TaskStatus {
    Pending,
    Running,
    Completed,
    Failed,
    Killed,
}

/// Notification types.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Notification {
    pub id: String,
    pub message: String,
    pub level: NotificationLevel,
    pub timestamp: chrono::DateTime<chrono::Utc>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum NotificationLevel {
    Info,
    Warning,
    Error,
    Success,
}

/// Model setting.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModelSetting {
    pub name: String,
    pub provider: String,
}

impl std::fmt::Display for ModelSetting {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.name)
    }
}

/// Settings JSON (simplified).
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct SettingsJson {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub default_mode: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub default_model: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub permission_mode: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub theme: Option<ThemeName>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub verbose: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub max_budget_usd: Option<f64>,
    #[serde(flatten)]
    pub extra: HashMap<String, serde_json::Value>,
}

/// Token counts from API responses.
#[derive(Debug, Clone, Default)]
pub struct TokenCounts {
    pub input_tokens: u64,
    pub output_tokens: u64,
    pub cache_read_tokens: u64,
    pub cache_creation_tokens: u64,
}

/// Per-turn token breakdown.
#[derive(Debug, Clone, Default)]
pub struct TurnTokenCounts {
    pub input_tokens: u64,
    pub output_tokens: u64,
    pub cache_read_tokens: u64,
    pub cache_creation_tokens: u64,
    pub cost_usd: f64,
}

/// Query lifecycle state.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum QueryState {
    #[default]
    Idle,
    Sending,
    Streaming,
    ToolPending,
    ToolRunning,
    Compacting,
    Cancelling,
    Error,
}

/// Pending tool call awaiting permission/execution.
#[derive(Debug, Clone)]
pub struct PendingToolCall {
    pub id: String,
    pub name: String,
    pub input: serde_json::Value,
    pub display_text: String,
}

/// Permission dialog state for inline display.
#[derive(Debug, Clone)]
pub struct PermissionDialogState {
    pub tool_name: String,
    pub tool_input: serde_json::Value,
    pub tool_display: String,
    pub tool_call_id: String,
}

impl Default for PermissionDialogState {
    fn default() -> Self {
        Self {
            tool_name: String::new(),
            tool_input: serde_json::Value::Null,
            tool_display: String::new(),
            tool_call_id: String::new(),
        }
    }
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

/// The main AppState.
#[derive(Debug, Clone)]
pub struct AppState {
    pub settings: SettingsJson,
    pub verbose: bool,
    pub main_loop_model: ModelSetting,
    pub main_loop_model_for_session: Option<ModelSetting>,
    pub status_line_text: Option<String>,
    pub expanded_view: ExpandedView,
    pub is_brief_only: bool,
    pub selected_ip_agent_index: usize,
    pub coordinator_task_index: isize,
    pub view_selection_mode: ViewSelectionMode,
    pub footer_selection: Option<FooterItem>,
    pub tool_permission_context: ToolPermissionContext,
    pub spinner_tip: Option<String>,
    pub agent: Option<String>,
    pub kairos_enabled: bool,
    pub remote_session_url: Option<String>,
    pub remote_connection_status: RemoteConnectionStatus,
    pub remote_background_task_count: usize,
    pub repl_bridge_enabled: bool,
    pub repl_bridge_explicit: bool,
    pub repl_bridge_outbound_only: bool,
    pub repl_bridge_connected: bool,
    pub repl_bridge_session_active: bool,
    pub repl_bridge_reconnecting: bool,
    pub repl_bridge_connect_url: Option<String>,
    pub repl_bridge_session_url: Option<String>,
    pub repl_bridge_environment_id: Option<String>,
    pub repl_bridge_session_id: Option<String>,
    pub repl_bridge_error: Option<String>,
    pub repl_bridge_initial_name: Option<String>,
    pub show_remote_callout: bool,
    pub tasks: HashMap<String, TaskState>,
    pub agent_name_registry: HashMap<String, crate::types::AgentId>,
    pub foregrounded_task_id: Option<String>,
    pub viewing_agent_task_id: Option<String>,
    pub companion_reaction: Option<String>,
    pub companion_pet_at: Option<i64>,
    pub mcp: McpState,
    pub plugins: PluginState,
    pub notifications: Vec<Notification>,
    pub elicitation_queue: Vec<ElicitationRequest>,
    pub file_history_state: FileHistoryState,
    pub attribution_state: AttributionState,
    pub thinking_mode: bool,
    pub thinking_enabled: bool,
    pub fast_mode: bool,
    pub effort: EffortValue,
    pub prompt_suggestions: Vec<String>,
    pub session_hooks_state: SessionHooksState,
    pub inbox_messages: Vec<InboxMessage>,
    pub tungsten_panel_state: Option<TungstenPanelState>,
    pub computer_use_mcp_state: Option<ComputerUseMcpState>,
    pub speculation_state: SpeculationState,
    pub worker_sandbox_permissions: HashMap<String, Vec<String>>,
    pub ultra_plan_state: Option<UltraPlanState>,
    pub todo_list: Option<TodoList>,
    // Conversation state
    pub messages: Vec<Message>,
    pub token_counts: TokenCounts,
    pub total_cost_usd: f64,
    pub is_querying: bool,
    pub session_id: Option<String>,
    pub transcript_mode: bool,
    pub show_all_in_transcript: bool,
    pub brief_mode: bool,
    pub vim_mode: bool,
    pub is_exiting: bool,
    pub exit_confirmation: Option<ExitConfirmation>,
    // Query lifecycle (Phase 9)
    pub query_state: QueryState,
    pub streaming_text: Option<String>,
    pub streaming_thinking: Option<String>,
    pub streaming_tool_json: HashMap<String, String>,
    pub pending_tool_calls: Vec<PendingToolCall>,
    pub pending_permission_dialog: Option<PermissionDialogState>,
    pub current_turn_tokens: Option<TurnTokenCounts>,
    pub query_error: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ExpandedView {
    #[default]
    None,
    Tasks,
    Teammates,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ViewSelectionMode {
    #[default]
    None,
    SelectingAgent,
    ViewingAgent,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum RemoteConnectionStatus {
    #[default]
    Connecting,
    Connected,
    Reconnecting,
    Disconnected,
}

#[derive(Debug, Clone)]
pub struct ElicitationRequest {
    pub server_name: String,
    pub url: String,
    pub label: Option<String>,
}

#[derive(Debug, Clone, Default)]
pub struct SessionHooksState {
    pub registered_hooks: Vec<String>,
}

#[derive(Debug, Clone)]
pub struct InboxMessage {
    pub id: String,
    pub content: String,
    pub timestamp: chrono::DateTime<chrono::Utc>,
}

#[derive(Debug, Clone)]
pub struct TungstenPanelState {
    pub visible: bool,
    pub panels: Vec<String>,
}

#[derive(Debug, Clone)]
pub struct ComputerUseMcpState {
    pub connected: bool,
    pub screen_size: Option<(u32, u32)>,
}

#[derive(Debug, Clone)]
pub struct UltraPlanState {
    pub steps: Vec<UltraPlanStep>,
}

#[derive(Debug, Clone)]
pub struct UltraPlanStep {
    pub id: String,
    pub description: String,
    pub status: UltraPlanStepStatus,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UltraPlanStepStatus {
    Pending,
    InProgress,
    Completed,
    Failed,
}

#[derive(Debug, Clone)]
pub struct TodoList {
    pub items: Vec<TodoItem>,
}

#[derive(Debug, Clone)]
pub struct TodoItem {
    pub id: String,
    pub content: String,
    pub status: TodoStatus,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TodoStatus {
    Pending,
    InProgress,
    Completed,
}

impl Default for AppState {
    fn default() -> Self {
        Self {
            settings: SettingsJson::default(),
            verbose: false,
            main_loop_model: ModelSetting {
                name: "claude-sonnet-4-20250514".to_string(),
                provider: "anthropic".to_string(),
            },
            main_loop_model_for_session: None,
            status_line_text: None,
            expanded_view: ExpandedView::None,
            is_brief_only: false,
            selected_ip_agent_index: 0,
            coordinator_task_index: -1,
            view_selection_mode: ViewSelectionMode::None,
            footer_selection: None,
            tool_permission_context: ToolPermissionContext::default(),
            spinner_tip: None,
            agent: None,
            kairos_enabled: false,
            remote_session_url: None,
            remote_connection_status: RemoteConnectionStatus::Disconnected,
            remote_background_task_count: 0,
            repl_bridge_enabled: false,
            repl_bridge_explicit: false,
            repl_bridge_outbound_only: false,
            repl_bridge_connected: false,
            repl_bridge_session_active: false,
            repl_bridge_reconnecting: false,
            repl_bridge_connect_url: None,
            repl_bridge_session_url: None,
            repl_bridge_environment_id: None,
            repl_bridge_session_id: None,
            repl_bridge_error: None,
            repl_bridge_initial_name: None,
            show_remote_callout: false,
            tasks: HashMap::new(),
            agent_name_registry: HashMap::new(),
            foregrounded_task_id: None,
            viewing_agent_task_id: None,
            companion_reaction: None,
            companion_pet_at: None,
            mcp: McpState::default(),
            plugins: PluginState::default(),
            notifications: Vec::new(),
            elicitation_queue: Vec::new(),
            file_history_state: FileHistoryState::default(),
            attribution_state: AttributionState::default(),
            thinking_mode: false,
            thinking_enabled: false,
            fast_mode: false,
            effort: EffortValue::Medium,
            prompt_suggestions: Vec::new(),
            session_hooks_state: SessionHooksState::default(),
            inbox_messages: Vec::new(),
            tungsten_panel_state: None,
            computer_use_mcp_state: None,
            speculation_state: SpeculationState::Idle,
            worker_sandbox_permissions: HashMap::new(),
            ultra_plan_state: None,
            todo_list: None,
            messages: Vec::new(),
            token_counts: TokenCounts::default(),
            total_cost_usd: 0.0,
            is_querying: false,
            session_id: None,
            transcript_mode: false,
            show_all_in_transcript: false,
            brief_mode: false,
            vim_mode: false,
            is_exiting: false,
            exit_confirmation: None,
            query_state: QueryState::Idle,
            streaming_text: None,
            streaming_thinking: None,
            streaming_tool_json: HashMap::new(),
            pending_tool_calls: Vec::new(),
            pending_permission_dialog: None,
            current_turn_tokens: None,
            query_error: None,
        }
    }
}
