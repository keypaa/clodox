use std::collections::HashMap;
use std::sync::Arc;

use async_trait::async_trait;
use serde::{Deserialize, Serialize};

use crate::messages::{
    AssistantMessage, AttachmentMessage, ContentBlockParam, SystemMessage,
    UserMessage,
};
use crate::permissions::{PermissionResult, ToolPermissionContext};
use crate::types::{QuerySource, ValidationResult};

/// JSON Schema for tool input (raw format).
pub type ToolInputJsonSchema = serde_json::Value;

/// Tool progress data (generic, tool-specific).
pub trait ToolProgressData: Clone + Serialize + for<'de> Deserialize<'de> + Send + Sync + 'static {}

impl<T> ToolProgressData for T where T: Clone + Serialize + for<'de> Deserialize<'de> + Send + Sync + 'static {}

/// Progress event for a tool call.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolProgress<P = serde_json::Value> {
    pub tool_use_id: String,
    pub data: P,
}

/// Result from a tool execution.
#[derive(Debug, Clone)]
pub struct ToolResult<T> {
    pub data: T,
    pub new_messages: Option<Vec<SystemOrUserOrAssistantMessage>>,
    pub mcp_meta: Option<McpMeta>,
}

/// Union of message types that can be added by tool results.
#[derive(Debug, Clone)]
pub enum SystemOrUserOrAssistantMessage {
    User(UserMessage),
    Assistant(AssistantMessage),
    Attachment(AttachmentMessage),
    System(SystemMessage),
}

/// MCP protocol metadata passthrough.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct McpMeta {
    #[serde(flatten)]
    pub meta: Option<HashMap<String, serde_json::Value>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub structured_content: Option<HashMap<String, serde_json::Value>>,
}

/// Query chain tracking for nested tool calls.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QueryChainTracking {
    pub chain_id: String,
    pub depth: usize,
}

/// Compact progress events.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum CompactProgressEvent {
    HooksStart {
        hook_type: String, // "pre_compact", "post_compact", "session_start"
    },
    CompactStart,
    CompactEnd,
}

impl std::fmt::Debug for ToolUseOptions {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ToolUseOptions")
            .field("debug", &self.debug)
            .field("main_loop_model", &self.main_loop_model)
            .field("verbose", &self.verbose)
            .finish_non_exhaustive()
    }
}

/// Tool use context - carries state through tool execution.
pub struct ToolUseContext {
    pub options: ToolUseOptions,
    pub abort_controller: tokio::sync::watch::Sender<bool>,
    pub messages: Vec<crate::messages::Message>,
    pub agent_id: Option<crate::types::AgentId>,
    pub agent_type: Option<String>,
    pub tool_use_id: Option<String>,
    pub user_modified: Option<bool>,
    pub require_can_use_tool: Option<bool>,
    pub query_tracking: Option<QueryChainTracking>,
    pub file_reading_limits: Option<FileReadingLimits>,
    pub glob_limits: Option<GlobLimits>,
    pub content_replacement_state: Option<ContentReplacementState>,
    pub local_denial_tracking: Option<DenialTrackingState>,
    pub rendered_system_prompt: Option<String>,
}

/// Trait for calling MCP tools from within other tools.
/// Implemented by McpService in cc-services.
#[async_trait]
pub trait McpToolCaller: Send + Sync {
    /// Call a tool on a remote MCP server.
    async fn call_mcp_tool(
        &self,
        server_name: &str,
        tool_name: &str,
        arguments: serde_json::Value,
    ) -> Result<serde_json::Value, String>;

    /// Get tools from a specific remote MCP server.
    async fn get_remote_tools(
        &self,
        server_name: &str,
    ) -> Vec<(String, String, String, serde_json::Value)>;
}

#[derive(Clone)]
pub struct ToolUseOptions {
    pub commands: Vec<crate::commands::Command>,
    pub debug: bool,
    pub main_loop_model: String,
    pub tools: Tools,
    pub verbose: bool,
    pub thinking_config: ThinkingConfig,
    pub mcp_clients: Vec<McpServerConnection>,
    pub mcp_resources: HashMap<String, Vec<ServerResource>>,
    pub is_non_interactive_session: bool,
    pub agent_definitions: AgentDefinitionsResult,
    pub max_budget_usd: Option<f64>,
    pub custom_system_prompt: Option<String>,
    pub append_system_prompt: Option<String>,
    pub query_source: Option<QuerySource>,
    pub refresh_tools: Option<Arc<dyn Fn() -> Tools + Send + Sync>>,
    pub mcp_service: Option<Arc<dyn McpToolCaller>>,
}

#[derive(Debug, Clone)]
pub struct ThinkingConfig {
    pub enabled: bool,
    pub budget_tokens: Option<u64>,
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

#[derive(Debug, Clone)]
pub struct AgentDefinitionsResult {
    pub agents: Vec<AgentDefinition>,
}

#[derive(Debug, Clone)]
pub struct AgentDefinition {
    pub name: String,
    pub description: String,
    pub agent_type: String,
}

#[derive(Debug, Clone)]
pub struct FileReadingLimits {
    pub max_tokens: Option<u64>,
    pub max_size_bytes: Option<u64>,
}

#[derive(Debug, Clone)]
pub struct GlobLimits {
    pub max_results: Option<usize>,
}

#[derive(Debug, Clone)]
pub struct ContentReplacementState {
    pub replacements: HashMap<String, serde_json::Value>,
}

#[derive(Debug, Clone)]
pub struct DenialTrackingState {
    pub denial_count: usize,
    pub last_denial_time: Option<chrono::DateTime<chrono::Utc>>,
}

/// A collection of tools.
pub type Tools = Arc<Vec<Arc<dyn Tool>>>;

/// The core Tool trait - every tool must implement this.
#[async_trait]
pub trait Tool: Send + Sync + std::fmt::Debug {
    /// Tool name (primary identifier).
    fn name(&self) -> &str;

    /// Optional aliases for backwards compatibility.
    fn aliases(&self) -> &[String] {
        &[]
    }

    /// One-line capability phrase for ToolSearch keyword matching.
    fn search_hint(&self) -> Option<&str> {
        None
    }

    /// Execute the tool with the given input.
    async fn call(
        &self,
        input: serde_json::Value,
        context: &ToolUseContext,
        on_progress: Option<Arc<dyn Fn(ToolProgress) + Send + Sync>>,
    ) -> anyhow::Result<ToolResult<serde_json::Value>>;

    /// Generate tool description for the LLM prompt.
    async fn description(
        &self,
        input: serde_json::Value,
        options: &DescriptionOptions,
    ) -> anyhow::Result<String>;

    /// JSON Schema for tool input validation.
    fn input_schema(&self) -> serde_json::Value;

    /// Optional JSON Schema (for MCP tools that specify directly).
    fn input_json_schema(&self) -> Option<ToolInputJsonSchema> {
        None
    }

    /// Check if two inputs are equivalent (for deduplication).
    fn inputs_equivalent(&self, _a: &serde_json::Value, _b: &serde_json::Value) -> bool {
        false
    }

    /// Whether this tool can safely run concurrently with itself.
    fn is_concurrency_safe(&self, _input: &serde_json::Value) -> bool {
        false
    }

    /// Whether this tool is currently enabled.
    fn is_enabled(&self) -> bool {
        true
    }

    /// Whether this tool only reads (no side effects).
    fn is_read_only(&self, _input: &serde_json::Value) -> bool {
        false
    }

    /// Whether this tool performs irreversible operations.
    fn is_destructive(&self, _input: &serde_json::Value) -> bool {
        false
    }

    /// Interrupt behavior when user submits a new message.
    fn interrupt_behavior(&self) -> InterruptBehavior {
        InterruptBehavior::Block
    }

    /// Whether this is a search/read operation for condensed UI display.
    fn is_search_or_read_command(&self, _input: &serde_json::Value) -> SearchOrReadInfo {
        SearchOrReadInfo::default()
    }

    /// Whether this tool operates in an open-world context.
    fn is_open_world(&self, _input: &serde_json::Value) -> bool {
        false
    }

    /// Whether this tool requires user interaction.
    fn requires_user_interaction(&self) -> bool {
        false
    }

    /// Whether this is an MCP tool.
    fn is_mcp(&self) -> bool {
        false
    }

    /// Whether this is an LSP tool.
    fn is_lsp(&self) -> bool {
        false
    }

    /// Whether this tool should be deferred (sent with defer_loading: true).
    fn should_defer(&self) -> bool {
        false
    }

    /// Whether this tool is never deferred.
    fn always_load(&self) -> bool {
        false
    }

    /// MCP server/tool info.
    fn mcp_info(&self) -> Option<McpInfo> {
        None
    }

    /// Maximum result size in characters before persisting to disk.
    fn max_result_size_chars(&self) -> usize;

    /// Whether strict mode is enabled for this tool.
    fn strict(&self) -> bool {
        false
    }

    /// Mutate input for observers (hooks, transcript, permissions).
    fn backfill_observable_input(&self, _input: &mut serde_json::Value) {}

    /// Validate tool input before permission check.
    async fn validate_input(
        &self,
        _input: &serde_json::Value,
        _context: &ToolUseContext,
    ) -> anyhow::Result<ValidationResult> {
        Ok(ValidationResult::Valid)
    }

    /// Check permissions for this tool use.
    async fn check_permissions(
        &self,
        input: &serde_json::Value,
        _context: &ToolUseContext,
    ) -> anyhow::Result<PermissionResult> {
        Ok(PermissionResult::Allow {
            updated_input: Some(input.clone()),
            user_modified: None,
            decision_reason: None,
            tool_use_id: None,
            accept_feedback: None,
            content_blocks: None,
        })
    }

    /// Get file path if this tool operates on a file.
    fn get_path(&self, _input: &serde_json::Value) -> Option<String> {
        None
    }

    /// Prepare a matcher for hook if conditions.
    fn prepare_permission_matcher(
        &self,
        _input: &serde_json::Value,
    ) -> Option<Box<dyn Fn(&str) -> bool + Send + Sync>> {
        None
    }

    /// Generate prompt content for this tool.
    async fn prompt(&self, options: &ToolPromptOptions) -> anyhow::Result<String>;

    /// Human-readable name for display.
    fn user_facing_name(&self, _input: Option<&serde_json::Value>) -> String {
        self.name().to_string()
    }

    /// Background color for the user-facing name badge.
    fn user_facing_name_background_color(
        &self,
        _input: Option<&serde_json::Value>,
    ) -> Option<String> {
        None
    }

    /// Whether this tool is a transparent wrapper.
    fn is_transparent_wrapper(&self) -> bool {
        false
    }

    /// Short summary for compact views.
    fn get_tool_use_summary(&self, _input: Option<&serde_json::Value>) -> Option<String> {
        None
    }

    /// Activity description for spinner display.
    fn get_activity_description(&self, _input: Option<&serde_json::Value>) -> Option<String> {
        None
    }

    /// Compact representation for auto-mode security classifier.
    fn to_auto_classifier_input(&self, _input: &serde_json::Value) -> serde_json::Value {
        serde_json::Value::String(String::new())
    }

    /// Map tool result to API result block.
    fn map_tool_result_to_block(
        &self,
        content: serde_json::Value,
        tool_use_id: &str,
    ) -> ContentBlockParam;

    /// Whether the result is truncated in non-verbose mode.
    fn is_result_truncated(&self, _output: &serde_json::Value) -> bool {
        false
    }

    /// Render tag after tool use message.
    fn render_tool_use_tag(&self, _input: Option<&serde_json::Value>) -> Option<String> {
        None
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InterruptBehavior {
    Cancel,
    Block,
}

#[derive(Debug, Clone, Default)]
pub struct SearchOrReadInfo {
    pub is_search: bool,
    pub is_read: bool,
    pub is_list: bool,
}

#[derive(Debug, Clone)]
pub struct McpInfo {
    pub server_name: String,
    pub tool_name: String,
}

#[derive(Debug, Clone)]
pub struct DescriptionOptions {
    pub is_non_interactive_session: bool,
    pub tool_permission_context: ToolPermissionContext,
    pub tools: Tools,
}

#[derive(Debug, Clone)]
pub struct ToolPromptOptions {
    pub tools: Tools,
    pub agents: Vec<AgentDefinition>,
    pub allowed_agent_types: Option<Vec<String>>,
}

/// Build a tool from a builder pattern.
pub struct ToolBuilder {
    name: String,
    aliases: Vec<String>,
    search_hint: Option<String>,
    input_schema: serde_json::Value,
    input_json_schema: Option<ToolInputJsonSchema>,
    max_result_size_chars: usize,
    should_defer: bool,
    always_load: bool,
    is_mcp: bool,
    is_lsp: bool,
    strict: bool,
    mcp_info: Option<McpInfo>,
}

impl ToolBuilder {
    pub fn new(name: impl Into<String>, input_schema: serde_json::Value) -> Self {
        Self {
            name: name.into(),
            aliases: Vec::new(),
            search_hint: None,
            input_schema,
            input_json_schema: None,
            max_result_size_chars: 100_000,
            should_defer: false,
            always_load: false,
            is_mcp: false,
            is_lsp: false,
            strict: false,
            mcp_info: None,
        }
    }

    pub fn aliases(mut self, aliases: Vec<String>) -> Self {
        self.aliases = aliases;
        self
    }

    pub fn search_hint(mut self, hint: impl Into<String>) -> Self {
        self.search_hint = Some(hint.into());
        self
    }

    pub fn input_json_schema(mut self, schema: ToolInputJsonSchema) -> Self {
        self.input_json_schema = Some(schema);
        self
    }

    pub fn max_result_size_chars(mut self, size: usize) -> Self {
        self.max_result_size_chars = size;
        self
    }

    pub fn should_defer(mut self, defer: bool) -> Self {
        self.should_defer = defer;
        self
    }

    pub fn always_load(mut self, load: bool) -> Self {
        self.always_load = load;
        self
    }

    pub fn is_mcp(mut self, mcp: bool) -> Self {
        self.is_mcp = mcp;
        self
    }

    pub fn mcp_info(mut self, server: impl Into<String>, tool: impl Into<String>) -> Self {
        self.mcp_info = Some(McpInfo {
            server_name: server.into(),
            tool_name: tool.into(),
        });
        self
    }

    pub fn strict(mut self, strict: bool) -> Self {
        self.strict = strict;
        self
    }

    pub fn build(self) -> ToolConfig {
        ToolConfig {
            name: self.name,
            aliases: self.aliases,
            search_hint: self.search_hint,
            input_schema: self.input_schema,
            input_json_schema: self.input_json_schema,
            max_result_size_chars: self.max_result_size_chars,
            should_defer: self.should_defer,
            always_load: self.always_load,
            is_mcp: self.is_mcp,
            is_lsp: self.is_lsp,
            strict: self.strict,
            mcp_info: self.mcp_info,
        }
    }
}

/// Tool configuration (without implementation).
#[derive(Debug, Clone)]
pub struct ToolConfig {
    pub name: String,
    pub aliases: Vec<String>,
    pub search_hint: Option<String>,
    pub input_schema: serde_json::Value,
    pub input_json_schema: Option<ToolInputJsonSchema>,
    pub max_result_size_chars: usize,
    pub should_defer: bool,
    pub always_load: bool,
    pub is_mcp: bool,
    pub is_lsp: bool,
    pub strict: bool,
    pub mcp_info: Option<McpInfo>,
}
