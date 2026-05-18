use std::collections::HashMap;
use std::sync::Arc;

use async_trait::async_trait;
use cc_query::api_client::ApiConfig;
use cc_query::api_types::SystemPromptBlock;
use cc_query::engine::{QueryConfig, QueryEngine, QueryEvent, TokenBudget};
use cc_query::retry::RetryOptions;
use crate::utils::check_read_permission;
use cc_core::messages::{ContentBlockParam, UserMessage};
use cc_core::permissions::PermissionResult;
use cc_core::tools::{
    InterruptBehavior, McpToolCaller, SearchOrReadInfo, Tool, ToolProgress, ToolPromptOptions,
    ToolResult, ToolUseContext,
};
use cc_core::types::ValidationResult;
use futures::StreamExt;
use serde::{Deserialize, Serialize};
use tokio::sync::{RwLock, watch};
use tracing::{debug, info, warn};

// =========================================================================
// Constants
// =========================================================================

/// Agent tool name.
pub const AGENT_TOOL_NAME: &str = "Agent";

/// Legacy agent tool name (alias).
pub const LEGACY_AGENT_TOOL_NAME: &str = "Task";

/// Progress threshold in milliseconds (show background hint after 2s).
const PROGRESS_THRESHOLD_MS: u64 = 2000;

/// Max result size in characters.
const MAX_RESULT_SIZE_CHARS: usize = 100_000;

// =========================================================================
// Agent Definition
// =========================================================================

/// Source of an agent definition.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum AgentSource {
    BuiltIn,
    Custom(String), // Path to agents/ directory
}

/// Agent definition loaded from built-in or user's agents/ directory.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FullAgentDefinition {
    /// Unique agent type identifier (e.g., "general-purpose", "code-reviewer").
    pub agent_type: String,
    /// Human-readable name.
    pub name: String,
    /// Description of when to use this agent.
    pub when_to_use: String,
    /// Model to use (sonnet, opus, haiku). None = inherit from parent.
    pub model: Option<String>,
    /// Tools allowlist. None = all tools.
    pub tools: Option<Vec<String>>,
    /// Tools deny list.
    pub disallowed_tools: Option<Vec<String>>,
    /// Permission mode for this agent.
    pub permission_mode: Option<String>,
    /// Required MCP server name patterns.
    pub required_mcp_servers: Option<Vec<String>>,
    /// Isolation mode (worktree, remote).
    pub isolation: Option<String>,
    /// Whether this agent should always run in background.
    pub background: bool,
    /// Predefined color for UI display.
    pub color: Option<String>,
    /// System prompt / instructions.
    pub instructions: Option<String>,
    /// Source of this definition.
    pub source: AgentSource,
}

impl FullAgentDefinition {
    /// Check if this is a custom (user-defined) agent.
    pub fn is_custom(&self) -> bool {
        matches!(self.source, AgentSource::Custom(_))
    }

    /// Get the system prompt for this agent.
    pub fn get_system_prompt(&self) -> String {
        self.instructions
            .clone()
            .unwrap_or_else(|| format!("You are the {} agent. {}", self.name, self.when_to_use))
    }
}

// =========================================================================
// Built-in Agents
// =========================================================================

/// General-purpose agent — the default when no subagent_type is specified.
fn general_purpose_agent() -> FullAgentDefinition {
    FullAgentDefinition {
        agent_type: "general-purpose".to_string(),
        name: "General Purpose Agent".to_string(),
        when_to_use: "Use for general tasks that don't require a specialized agent.".to_string(),
        model: None,
        tools: None,
        disallowed_tools: None,
        permission_mode: None,
        required_mcp_servers: None,
        isolation: None,
        background: false,
        color: Some("#3B82F6".to_string()),
        instructions: Some(
            "You are a general-purpose autonomous agent. You have access to all standard tools. \
             Complete the task given to you thoroughly and efficiently."
                .to_string(),
        ),
        source: AgentSource::BuiltIn,
    }
}

/// Plan agent — for creating detailed implementation plans.
fn plan_agent() -> FullAgentDefinition {
    FullAgentDefinition {
        agent_type: "plan".to_string(),
        name: "Plan Agent".to_string(),
        when_to_use: "Use to create detailed implementation plans before writing code.".to_string(),
        model: Some("sonnet".to_string()),
        tools: Some(vec!["Read".to_string(), "Glob".to_string(), "Grep".to_string()]),
        disallowed_tools: Some(vec!["Edit".to_string(), "Write".to_string(), "Bash".to_string()]),
        permission_mode: Some("plan".to_string()),
        required_mcp_servers: None,
        isolation: None,
        background: false,
        color: Some("#10B981".to_string()),
        instructions: Some(
            "You are a planning agent. Your job is to analyze codebases and create detailed \
             implementation plans. You can read files and search the codebase, but you cannot \
             modify any files. Output a clear, step-by-step plan."
                .to_string(),
        ),
        source: AgentSource::BuiltIn,
    }
}

/// Explore agent — for exploring and understanding codebases.
fn explore_agent() -> FullAgentDefinition {
    FullAgentDefinition {
        agent_type: "explore".to_string(),
        name: "Explore Agent".to_string(),
        when_to_use: "Use to explore and understand a codebase's structure and architecture.".to_string(),
        model: Some("haiku".to_string()),
        tools: Some(vec![
            "Read".to_string(),
            "Glob".to_string(),
            "Grep".to_string(),
            "Bash".to_string(),
        ]),
        disallowed_tools: Some(vec!["Edit".to_string(), "Write".to_string()]),
        permission_mode: None,
        required_mcp_servers: None,
        isolation: None,
        background: false,
        color: Some("#F59E0B".to_string()),
        instructions: Some(
            "You are an exploration agent. Your job is to explore and understand the codebase. \
             You can read files, search for patterns, and run read-only commands. Do not modify \
             any files. Provide a clear summary of what you find."
                .to_string(),
        ),
        source: AgentSource::BuiltIn,
    }
}

/// Verification agent — for verifying code correctness.
fn verification_agent() -> FullAgentDefinition {
    FullAgentDefinition {
        agent_type: "verification".to_string(),
        name: "Verification Agent".to_string(),
        when_to_use: "Use to verify code correctness, run tests, and check implementations.".to_string(),
        model: Some("sonnet".to_string()),
        tools: None,
        disallowed_tools: None,
        permission_mode: None,
        required_mcp_servers: None,
        isolation: None,
        background: false,
        color: Some("#8B5CF6".to_string()),
        instructions: Some(
            "You are a verification agent. Your job is to verify that code works correctly. \
             Read the code, run tests, and check implementations. Report any issues found."
                .to_string(),
        ),
        source: AgentSource::BuiltIn,
    }
}

/// Get all built-in agent definitions.
fn get_builtin_agents() -> Vec<FullAgentDefinition> {
    vec![
        general_purpose_agent(),
        plan_agent(),
        explore_agent(),
        verification_agent(),
    ]
}

// =========================================================================
// Agent Color Management
// =========================================================================

/// Global agent color registry.
pub struct AgentColorManager {
    colors: RwLock<HashMap<String, String>>,
}

impl std::fmt::Debug for AgentColorManager {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("AgentColorManager").finish_non_exhaustive()
    }
}

impl AgentColorManager {
    pub fn new() -> Arc<Self> {
        Arc::new(Self {
            colors: RwLock::new(HashMap::new()),
        })
    }

    /// Set the color for an agent type.
    pub async fn set_color(&self, agent_type: &str, color: &str) {
        let mut colors = self.colors.write().await;
        colors.insert(agent_type.to_string(), color.to_string());
    }

    /// Get the color for an agent type.
    pub async fn get_color(&self, agent_type: &str) -> Option<String> {
        let colors = self.colors.read().await;
        colors.get(agent_type).cloned()
    }
}

// =========================================================================
// Async Agent State
// =========================================================================

/// Status of a running async agent.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum AsyncAgentStatus {
    Running,
    Completed,
    Aborted,
    Killed,
    Error,
}

/// State of a running async agent.
pub struct AsyncAgentState {
    pub agent_id: String,
    pub agent_type: String,
    pub description: String,
    pub prompt: String,
    pub model: String,
    pub status: AsyncAgentStatus,
    pub output_file: String,
    pub abort_tx: watch::Sender<bool>,
    pub start_time: chrono::DateTime<chrono::Utc>,
    pub summary: Option<String>,
}

/// Global registry for running async agents.
pub struct AsyncAgentRegistry {
    agents: RwLock<HashMap<String, Arc<AsyncAgentState>>>,
}

impl std::fmt::Debug for AsyncAgentRegistry {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("AsyncAgentRegistry").finish_non_exhaustive()
    }
}

impl AsyncAgentRegistry {
    pub fn new() -> Arc<Self> {
        Arc::new(Self {
            agents: RwLock::new(HashMap::new()),
        })
    }

    /// Register a new async agent.
    pub async fn register(&self, state: AsyncAgentState) {
        let mut agents = self.agents.write().await;
        agents.insert(state.agent_id.clone(), Arc::new(state));
    }

    /// Get an agent by ID.
    pub async fn get(&self, agent_id: &str) -> Option<Arc<AsyncAgentState>> {
        let agents = self.agents.read().await;
        agents.get(agent_id).cloned()
    }

    /// Kill a running async agent.
    pub async fn kill(&self, agent_id: &str) -> bool {
        let agents = self.agents.read().await;
        if let Some(state) = agents.get(agent_id) {
            let _ = state.abort_tx.send(true);
            // Update status to killed
            drop(agents);
            self.update_status(agent_id, AsyncAgentStatus::Killed).await;
            true
        } else {
            false
        }
    }

    /// Update agent status.
    pub async fn update_status(&self, agent_id: &str, status: AsyncAgentStatus) {
        let mut agents = self.agents.write().await;
        if let Some(state) = agents.get_mut(agent_id) {
            if let Some(inner) = Arc::get_mut(state) {
                inner.status = status;
            }
        }
    }

    /// List all running agents.
    pub async fn list_running(&self) -> Vec<Arc<AsyncAgentState>> {
        let agents = self.agents.read().await;
        agents
            .values()
            .filter(|s| s.status == AsyncAgentStatus::Running)
            .cloned()
            .collect()
    }

    /// Remove a completed/failed agent from the registry.
    pub async fn remove(&self, agent_id: &str) {
        let mut agents = self.agents.write().await;
        agents.remove(agent_id);
    }
}

// =========================================================================
// Agent Progress Tracking
// =========================================================================

/// Tracks progress of a running agent.
struct AgentProgressTracker {
    description: String,
    activity_description: String,
    tool_uses: usize,
    token_count: u64,
    current_tool: Option<String>,
    status: AgentProgressStatus,
    error_message: Option<String>,
}

#[derive(Debug, Clone, PartialEq)]
enum AgentProgressStatus {
    Running,
    Completed,
    Aborted,
    Error,
}

impl AgentProgressTracker {
    fn new(description: String) -> Self {
        Self {
            description: description.clone(),
            activity_description: "Starting...".to_string(),
            tool_uses: 0,
            token_count: 0,
            current_tool: None,
            status: AgentProgressStatus::Running,
            error_message: None,
        }
    }

    fn update_from_stream(&mut self, event: &cc_core::messages::StreamEvent) {
        if let cc_core::messages::StreamEvent::ContentBlockDelta { delta, .. } = event {
            if let cc_core::messages::ContentBlockDelta::TextDelta { text } = delta {
                // Extract activity description from text (e.g., "Running tests...", "Reading file...")
                if text.starts_with("Running") || text.starts_with("Reading") || text.starts_with("Writing") || text.starts_with("Searching") {
                    let line = text.lines().next().unwrap_or(text);
                    if line.len() <= 80 {
                        self.activity_description = line.to_string();
                    }
                }
            }
        }
    }

    fn record_tool_calls(&mut self, tool_calls: &[cc_query::engine::ToolCallInfo]) {
        self.tool_uses += tool_calls.len();
        if let Some(first) = tool_calls.first() {
            self.current_tool = Some(first.name.clone());
            self.activity_description = format!("Running {}", first.name);
        }
    }

    fn record_tool_result(&mut self, tool_name: &str, success: bool) {
        if self.current_tool.as_deref() == Some(tool_name) {
            self.current_tool = None;
        }
        if !success {
            self.activity_description = format!("{tool_name} failed");
        }
    }

    fn mark_completed(&mut self) {
        self.status = AgentProgressStatus::Completed;
        self.activity_description = "Completed".to_string();
    }

    fn mark_aborted(&mut self) {
        self.status = AgentProgressStatus::Aborted;
        self.activity_description = "Aborted".to_string();
    }

    fn mark_error(&mut self, error: &str) {
        self.status = AgentProgressStatus::Error;
        self.error_message = Some(error.to_string());
        self.activity_description = "Error".to_string();
    }

    fn to_progress(&self, agent_id: &str) -> ToolProgress {
        let data = serde_json::json!({
            "agentId": agent_id,
            "description": self.description,
            "activityDescription": self.activity_description,
            "toolUses": self.tool_uses,
            "tokenCount": self.token_count,
            "status": match self.status {
                AgentProgressStatus::Running => "running",
                AgentProgressStatus::Completed => "completed",
                AgentProgressStatus::Aborted => "aborted",
                AgentProgressStatus::Error => "error",
            },
            "currentTool": self.current_tool,
            "errorMessage": self.error_message,
        });
        ToolProgress {
            tool_use_id: agent_id.to_string(),
            data,
        }
    }
}

// =========================================================================
// Agent Tool
// =========================================================================

/// Agent tool — spawns specialized subagents for complex tasks.
#[derive(Debug)]
pub struct AgentTool {
    color_manager: Arc<AgentColorManager>,
    async_registry: Arc<AsyncAgentRegistry>,
}

impl AgentTool {
    pub fn new(color_manager: Arc<AgentColorManager>) -> Arc<dyn Tool> {
        Arc::new(Self {
            color_manager,
            async_registry: AsyncAgentRegistry::new(),
        })
    }

    /// Get all active agent definitions (built-in + custom).
    fn get_active_agents(&self, context: &ToolUseContext) -> Vec<FullAgentDefinition> {
        let mut agents = get_builtin_agents();

        // Add custom agents from context if available
        if !context.options.agent_definitions.agents.is_empty() {
            for def in &context.options.agent_definitions.agents {
                agents.push(FullAgentDefinition {
                    agent_type: def.agent_type.clone(),
                    name: def.name.clone(),
                    when_to_use: def.description.clone(),
                    model: None,
                    tools: None,
                    disallowed_tools: None,
                    permission_mode: None,
                    required_mcp_servers: None,
                    isolation: None,
                    background: false,
                    color: None,
                    instructions: None,
                    source: AgentSource::Custom(String::new()),
                });
            }
        }

        agents
    }

    /// Filter agents by allowed types.
    fn filter_agents_by_types(
        agents: Vec<FullAgentDefinition>,
        allowed_types: Option<&[String]>,
    ) -> Vec<FullAgentDefinition> {
        match allowed_types {
            Some(types) => agents
                .into_iter()
                .filter(|a| types.contains(&a.agent_type))
                .collect(),
            None => agents,
        }
    }

    /// Filter agents by MCP requirements.
    async fn filter_by_mcp_requirements(
        &self,
        agents: Vec<FullAgentDefinition>,
        mcp_service: Option<&Arc<dyn McpToolCaller>>,
    ) -> Vec<FullAgentDefinition> {
        let mut result = Vec::new();

        for agent in agents {
            if let Some(ref required) = agent.required_mcp_servers {
                if required.is_empty() {
                    result.push(agent);
                    continue;
                }

                // Check if required MCP servers have tools available
                if let Some(service) = mcp_service {
                    let mut all_matched = true;
                    for pattern in required {
                        let mut found = false;
                        // Check all remote servers for matching pattern
                        // In a full implementation, we'd check local servers too
                        for server_name in ["exa"] {
                            let tools = service.get_remote_tools(server_name).await;
                            if !tools.is_empty() {
                                found = true;
                                break;
                            }
                        }
                        if !found {
                            all_matched = false;
                            break;
                        }
                    }
                    if all_matched {
                        result.push(agent);
                    }
                }
            } else {
                result.push(agent);
            }
        }

        result
    }

    /// Resolve the model for an agent.
    fn resolve_model(
        agent: &FullAgentDefinition,
        parent_model: &str,
        explicit_model: Option<&str>,
    ) -> String {
        // Explicit model override wins
        if let Some(model) = explicit_model {
            if model == "inherit" {
                return parent_model.to_string();
            }
            return model.to_string();
        }

        // Agent definition model
        if let Some(model) = &agent.model {
            if model == "inherit" {
                return parent_model.to_string();
            }
            return model.clone();
        }

        // Default fallback
        parent_model.to_string()
    }

    /// Generate unique agent name (handle duplicates).
    async fn generate_unique_name(
        base_name: &str,
        existing_names: &HashMap<String, String>,
    ) -> String {
        let lower_base = base_name.to_lowercase();
        if !existing_names.values().any(|n| n.to_lowercase() == lower_base) {
            return base_name.to_string();
        }

        let mut suffix = 2;
        loop {
            let candidate = format!("{base_name}-{suffix}");
            if !existing_names
                .values()
                .any(|n| n.to_lowercase() == candidate.to_lowercase())
            {
                return candidate;
            }
            suffix += 1;
        }
    }

    /// Sanitize agent name (remove @ and other problematic chars).
    fn sanitize_name(name: &str) -> String {
        name.chars()
            .filter(|c| c.is_alphanumeric() || *c == '-' || *c == '_')
            .collect()
    }

    /// Format agent ID from name and team.
    fn format_agent_id(name: &str, team_name: &str) -> String {
        format!("{name}@{team_name}")
    }

    /// Run a sync agent to completion.
    async fn run_sync_agent(
        &self,
        agent: &FullAgentDefinition,
        prompt: &str,
        description: &str,
        context: &ToolUseContext,
        model: &str,
        on_progress: Option<Arc<dyn Fn(ToolProgress) + Send + Sync>>,
    ) -> anyhow::Result<serde_json::Value> {
        let start = std::time::Instant::now();
        let agent_id = uuid::Uuid::new_v4().to_string();

        info!(
            agent_type = %agent.agent_type,
            model = %model,
            description = %description,
            "Starting sync agent"
        );

        // Set agent color
        if let Some(ref color) = agent.color {
            self.color_manager
                .set_color(&agent.agent_type, color)
                .await;
        }

        // Build agent-specific tool pool
        let agent_tools = self.build_agent_tool_pool(agent, context);

        // Build agent-specific system prompt
        let system_prompt = self.build_agent_system_prompt(agent, prompt, context);

        // Create API config from environment
        let api_key = std::env::var("ANTHROPIC_API_KEY")
            .map_err(|_| anyhow::anyhow!("ANTHROPIC_API_KEY not set"))?;
        let api_config = ApiConfig {
            api_key,
            base_url: std::env::var("ANTHROPIC_BASE_URL")
                .unwrap_or_else(|_| "https://api.anthropic.com".to_string()),
            ..Default::default()
        };

        // Create query config
        let query_config = QueryConfig {
            model: model.to_string(),
            max_tokens: 4096,
            system_prompt,
            tools: agent_tools,
            permission_context: cc_core::permissions::ToolPermissionContext::default(),
            temperature: None,
            thinking_enabled: context.options.thinking_config.enabled,
            thinking_budget: context.options.thinking_config.budget_tokens,
            token_budget: TokenBudget::new(None),
            api_config,
            retry_options: RetryOptions {
                max_retries: 3,
                model: model.to_string(),
                fallback_model: None,
                initial_consecutive_529: 0,
            },
            verbose: context.options.verbose,
            debug: context.options.debug,
        };

        // Create abort channel for the agent
        let (abort_tx, abort_rx) = tokio::sync::watch::channel(false);

        // Create isolated QueryEngine
        let mut query_engine = QueryEngine::new(query_config, abort_rx)
            .map_err(|e| anyhow::anyhow!("Failed to create QueryEngine: {e}"))?;

        // Create user message from prompt
        let user_msg = UserMessage {
            id: uuid::Uuid::new_v4(),
            content: vec![ContentBlockParam::Text {
                text: prompt.to_string(),
            }],
            timestamp: chrono::Utc::now(),
            is_meta: None,
            origin_query_source: None,
            effort: None,
        };

        // Run the query loop and collect events
        let mut event_stream = std::pin::pin!(query_engine.submit_message(user_msg).await);
        let mut all_messages = Vec::new();
        let mut total_tool_use_count = 0;
        let mut total_input_tokens = 0u64;
        let mut total_output_tokens = 0u64;
        let mut final_content = String::new();
        let mut progress_tracker = AgentProgressTracker::new(description.to_string());

        // Emit initial progress
        if let Some(ref cb) = on_progress {
            cb(progress_tracker.to_progress(&agent_id));
        }

        while let Some(event_result) = event_stream.next().await {
            match event_result {
                Ok(event) => match event {
                    QueryEvent::Stream(core_event) => {
                        // Track token usage from stream events
                        if let cc_core::messages::StreamEvent::MessageDelta { usage, .. } = &core_event {
                            total_input_tokens += usage.input_tokens;
                            total_output_tokens += usage.output_tokens;
                        }
                        // Update progress from streaming content
                        progress_tracker.update_from_stream(&core_event);
                        if let Some(ref cb) = on_progress {
                            cb(progress_tracker.to_progress(&agent_id));
                        }
                    }
                    QueryEvent::TurnComplete { message } => {
                        // Collect final text content
                        for block in &message.content {
                            if let ContentBlockParam::Text { text } = block {
                                final_content.push_str(text);
                            }
                        }
                        all_messages.push(message);
                        progress_tracker.mark_completed();
                        break;
                    }
                    QueryEvent::ToolCallsPending { tool_calls, .. } => {
                        total_tool_use_count += tool_calls.len();
                        progress_tracker.record_tool_calls(&tool_calls);
                        if let Some(ref cb) = on_progress {
                            cb(progress_tracker.to_progress(&agent_id));
                        }
                    }
                    QueryEvent::ToolResult { tool_name, success, .. } => {
                        debug!(%tool_name, success, "Agent tool result");
                        progress_tracker.record_tool_result(&tool_name, success);
                    }
                    QueryEvent::MaxTokensReached { message } => {
                        warn!("Agent hit max tokens limit");
                        for block in &message.content {
                            if let ContentBlockParam::Text { text } = block {
                                final_content.push_str(text);
                            }
                        }
                        all_messages.push(message);
                        progress_tracker.mark_completed();
                        break;
                    }
                    QueryEvent::Aborted => {
                        info!("Agent was aborted");
                        progress_tracker.mark_aborted();
                        break;
                    }
                },
                Err(e) => {
                    warn!(error = %e, "Agent query error");
                    progress_tracker.mark_error(&e.to_string());
                    break;
                }
            }
        }

        let duration_ms = start.elapsed().as_millis() as u64;
        let total_tokens = total_input_tokens + total_output_tokens;

        // Write transcript to file
        let transcript_path = self.write_agent_transcript(
            &agent_id,
            agent,
            prompt,
            description,
            &all_messages,
            total_tool_use_count,
            total_tokens,
            duration_ms,
        );

        info!(
            agent_type = %agent.agent_type,
            duration_ms,
            tool_uses = total_tool_use_count,
            tokens = total_tokens,
            "Sync agent completed"
        );

        Ok(serde_json::json!({
            "status": "completed",
            "agentId": agent_id,
            "agentType": agent.agent_type,
            "model": model,
            "prompt": prompt,
            "description": description,
            "content": final_content,
            "totalToolUseCount": total_tool_use_count,
            "totalDurationMs": duration_ms,
            "totalTokens": total_tokens,
            "output_file": transcript_path,
        }))
    }

    /// Build a filtered tool pool for the agent based on its allowlist/denylist.
    fn build_agent_tool_pool(
        &self,
        agent: &FullAgentDefinition,
        context: &ToolUseContext,
    ) -> cc_core::tools::Tools {
        let all_tools = &context.options.tools;

        // Filter by allowlist (if specified)
        let filtered: Vec<Arc<dyn cc_core::tools::Tool>> = if let Some(ref allowed) = agent.tools {
            all_tools
                .iter()
                .filter(|t| allowed.contains(&t.name().to_string()) || allowed.contains(&t.name().to_lowercase()))
                .cloned()
                .collect()
        } else {
            all_tools.iter().cloned().collect()
        };

        // Apply denylist
        if let Some(ref denied) = agent.disallowed_tools {
            Arc::new(
                filtered
                    .into_iter()
                    .filter(|t| !denied.contains(&t.name().to_string()) && !denied.contains(&t.name().to_lowercase()))
                    .collect::<Vec<_>>(),
            )
        } else {
            Arc::new(filtered)
        }
    }

    /// Build agent-specific system prompt.
    fn build_agent_system_prompt(
        &self,
        agent: &FullAgentDefinition,
        prompt: &str,
        context: &ToolUseContext,
    ) -> Vec<SystemPromptBlock> {
        let mut parts = Vec::new();

        // Agent's own instructions/system prompt
        let agent_instructions = agent.get_system_prompt();
        if !agent_instructions.is_empty() {
            parts.push(SystemPromptBlock::Text {
                text: agent_instructions,
                cache_control: None,
            });
        }

        // Base instructions for subagents
        let base_instructions = format!(
            "You are a specialized subagent (type: {agent_type}) executing a task delegated by the main assistant.\n\
            Task description: {description}\n\n\
            Follow the instructions below carefully. Use available tools when needed.\n\
            When you have completed the task, provide a clear and concise result.",
            agent_type = agent.agent_type,
            description = agent.when_to_use,
        );
        parts.push(SystemPromptBlock::Text {
            text: base_instructions,
            cache_control: None,
        });

        // Environment context
        let cwd = std::env::current_dir()
            .ok()
            .and_then(|p| p.to_str().map(|s| s.to_string()))
            .unwrap_or_else(|| "<unknown>".to_string());
        let env_context = format!(
            "Working directory: {cwd}\n\
            OS: {}\n\
            Architecture: {}",
            std::env::consts::OS,
            std::env::consts::ARCH,
        );
        parts.push(SystemPromptBlock::Text {
            text: env_context,
            cache_control: None,
        });

        // Inherit parent's custom system prompt if present
        if let Some(ref parent_prompt) = context.options.custom_system_prompt {
            parts.push(SystemPromptBlock::Text {
                text: parent_prompt.clone(),
                cache_control: None,
            });
        }

        parts
    }

    /// Write agent transcript to .claude/agents/<agent-id>/transcript.json
    fn write_agent_transcript(
        &self,
        agent_id: &str,
        agent: &FullAgentDefinition,
        prompt: &str,
        description: &str,
        messages: &[cc_core::messages::AssistantMessage],
        tool_use_count: usize,
        total_tokens: u64,
        duration_ms: u64,
    ) -> String {
        let transcript_path = format!(".claude/agents/{agent_id}/transcript.json");
        let dir = std::path::Path::new(&transcript_path)
            .parent()
            .unwrap_or(std::path::Path::new(".claude/agents"));

        if let Err(e) = std::fs::create_dir_all(dir) {
            warn!(error = %e, path = ?dir, "Failed to create agent transcript directory");
            return transcript_path;
        }

        let transcript = serde_json::json!({
            "agentId": agent_id,
            "agentType": agent.agent_type,
            "name": agent.name,
            "model": "unknown", // Would be resolved in full impl
            "prompt": prompt,
            "description": description,
            "startTime": chrono::Utc::now().to_rfc3339(),
            "durationMs": duration_ms,
            "totalToolUseCount": tool_use_count,
            "totalTokens": total_tokens,
            "messages": messages.iter().map(|m| {
                serde_json::json!({
                    "role": "assistant",
                    "content": m.content.iter().map(|c| {
                        match c {
                            ContentBlockParam::Text { text } => serde_json::json!({"type": "text", "text": text}),
                            ContentBlockParam::ToolUse { id, name, input } => serde_json::json!({"type": "tool_use", "id": id, "name": name, "input": input}),
                            ContentBlockParam::Thinking { thinking, signature } => serde_json::json!({"type": "thinking", "thinking": thinking}),
                            _ => serde_json::json!({"type": "unknown"}),
                        }
                    }).collect::<Vec<_>>(),
                })
            }).collect::<Vec<_>>(),
        });

        if let Err(e) = std::fs::write(&transcript_path, serde_json::to_string_pretty(&transcript).unwrap_or_default()) {
            warn!(error = %e, path = %transcript_path, "Failed to write agent transcript");
        }

        transcript_path
    }

    /// Run an async agent in the background.
    async fn run_async_agent(
        &self,
        agent: &FullAgentDefinition,
        prompt: &str,
        description: &str,
        context: &ToolUseContext,
        model: &str,
    ) -> anyhow::Result<serde_json::Value> {
        let agent_id = uuid::Uuid::new_v4().to_string();
        let output_file = format!(".claude/agents/{agent_id}/transcript.json");

        info!(
            agent_type = %agent.agent_type,
            model = %model,
            description = %description,
            agent_id = %agent_id,
            "Starting async agent"
        );

        // Set agent color
        if let Some(ref color) = agent.color {
            self.color_manager
                .set_color(&agent.agent_type, color)
                .await;
        }

        // Create independent abort channel (not linked to parent's)
        let (abort_tx, abort_rx) = watch::channel(false);

        // Write agent metadata before spawning
        self.write_agent_metadata(&agent_id, agent, prompt, description, model);

        // Clone data for the background task
        let agent_clone = agent.clone();
        let prompt_clone = prompt.to_string();
        let description_clone = description.to_string();
        let model_clone = model.to_string();
        let context_options = context.options.clone();
        let registry_clone = self.async_registry.clone();
        let color_manager_clone = self.color_manager.clone();
        let agent_id_clone = agent_id.clone();
        let output_file_clone = output_file.clone();

        // Spawn the background task
        tokio::spawn(async move {
            let result = Self::run_async_agent_loop_with_options(
                &agent_clone,
                &prompt_clone,
                &description_clone,
                &context_options,
                &model_clone,
                abort_rx,
                &agent_id_clone,
                &output_file_clone,
                &registry_clone,
                &color_manager_clone,
            )
            .await;

            match result {
                Ok(summary) => {
                    info!(agent_id = %agent_id_clone, "Async agent completed: {summary}");
                }
                Err(e) => {
                    warn!(agent_id = %agent_id_clone, error = %e, "Async agent failed");
                }
            }
        });

        // Register the agent
        let state = AsyncAgentState {
            agent_id: agent_id.clone(),
            agent_type: agent.agent_type.clone(),
            description: description.to_string(),
            prompt: prompt.to_string(),
            model: model.to_string(),
            status: AsyncAgentStatus::Running,
            output_file: output_file.clone(),
            abort_tx,
            start_time: chrono::Utc::now(),
            summary: None,
        };
        self.async_registry.register(state).await;

        // Emit initial progress (via log since we're in background)
        info!(
            agent_id = %agent_id,
            description = %description,
            "Async agent launched in background"
        );

        let can_read_output = context.options.tools.iter().any(|t| {
            t.name() == "Read" || t.name() == "Bash"
        });

        Ok(serde_json::json!({
            "status": "async_launched",
            "agentId": agent_id,
            "description": description,
            "prompt": prompt,
            "outputFile": output_file,
            "canReadOutputFile": can_read_output,
        }))
    }

    /// The actual async agent loop (runs in background tokio task).
    async fn run_async_agent_loop_with_options(
        agent: &FullAgentDefinition,
        prompt: &str,
        description: &str,
        options: &cc_core::tools::ToolUseOptions,
        model: &str,
        abort_rx: watch::Receiver<bool>,
        agent_id: &str,
        output_file: &str,
        registry: &Arc<AsyncAgentRegistry>,
        color_manager: &Arc<AgentColorManager>,
    ) -> anyhow::Result<String> {
        let start = std::time::Instant::now();

        // Build agent-specific tool pool
        let agent_tools = Self::build_agent_tool_pool_from_options(agent, options);

        // Build agent-specific system prompt
        let system_prompt = Self::build_agent_system_prompt_from_options(agent, prompt, options);

        // Create API config from environment
        let api_key = std::env::var("ANTHROPIC_API_KEY")
            .map_err(|_| anyhow::anyhow!("ANTHROPIC_API_KEY not set"))?;
        let api_config = ApiConfig {
            api_key,
            base_url: std::env::var("ANTHROPIC_BASE_URL")
                .unwrap_or_else(|_| "https://api.anthropic.com".to_string()),
            ..Default::default()
        };

        // Create query config
        let query_config = QueryConfig {
            model: model.to_string(),
            max_tokens: 4096,
            system_prompt,
            tools: agent_tools,
            permission_context: cc_core::permissions::ToolPermissionContext::default(),
            temperature: None,
            thinking_enabled: options.thinking_config.enabled,
            thinking_budget: options.thinking_config.budget_tokens,
            token_budget: TokenBudget::new(None),
            api_config,
            retry_options: RetryOptions {
                max_retries: 3,
                model: model.to_string(),
                fallback_model: None,
                initial_consecutive_529: 0,
            },
            verbose: options.verbose,
            debug: options.debug,
        };

        // Create isolated QueryEngine
        let mut query_engine = QueryEngine::new(query_config, abort_rx)
            .map_err(|e| anyhow::anyhow!("Failed to create QueryEngine: {e}"))?;

        // Create user message from prompt
        let user_msg = UserMessage {
            id: uuid::Uuid::new_v4(),
            content: vec![ContentBlockParam::Text {
                text: prompt.to_string(),
            }],
            timestamp: chrono::Utc::now(),
            is_meta: None,
            origin_query_source: None,
            effort: None,
        };

        // Run the query loop and collect events
        let mut event_stream = std::pin::pin!(query_engine.submit_message(user_msg).await);
        let mut all_messages = Vec::new();
        let mut total_tool_use_count = 0;
        let mut total_input_tokens = 0u64;
        let mut total_output_tokens = 0u64;
        let mut final_content = String::new();
        let mut progress_tracker = AgentProgressTracker::new(description.to_string());

        while let Some(event_result) = event_stream.next().await {
            match event_result {
                Ok(event) => match event {
                    QueryEvent::Stream(core_event) => {
                        if let cc_core::messages::StreamEvent::MessageDelta { usage, .. } = &core_event {
                            total_input_tokens += usage.input_tokens;
                            total_output_tokens += usage.output_tokens;
                        }
                        progress_tracker.update_from_stream(&core_event);
                    }
                    QueryEvent::TurnComplete { message } => {
                        for block in &message.content {
                            if let ContentBlockParam::Text { text } = block {
                                final_content.push_str(text);
                            }
                        }
                        all_messages.push(message);
                        progress_tracker.mark_completed();
                        break;
                    }
                    QueryEvent::ToolCallsPending { tool_calls, .. } => {
                        total_tool_use_count += tool_calls.len();
                        progress_tracker.record_tool_calls(&tool_calls);
                    }
                    QueryEvent::ToolResult { tool_name, success, .. } => {
                        debug!(%tool_name, success, "Async agent tool result");
                        progress_tracker.record_tool_result(&tool_name, success);
                    }
                    QueryEvent::MaxTokensReached { message } => {
                        warn!("Async agent hit max tokens limit");
                        for block in &message.content {
                            if let ContentBlockParam::Text { text } = block {
                                final_content.push_str(text);
                            }
                        }
                        all_messages.push(message);
                        progress_tracker.mark_completed();
                        break;
                    }
                    QueryEvent::Aborted => {
                        info!("Async agent was aborted");
                        progress_tracker.mark_aborted();
                        break;
                    }
                },
                Err(e) => {
                    warn!(error = %e, "Async agent query error");
                    progress_tracker.mark_error(&e.to_string());
                    break;
                }
            }
        }

        let duration_ms = start.elapsed().as_millis() as u64;
        let total_tokens = total_input_tokens + total_output_tokens;

        // Write transcript
        let transcript_path = Self::write_agent_transcript_static(
            agent_id,
            agent,
            prompt,
            description,
            &all_messages,
            total_tool_use_count,
            total_tokens,
            duration_ms,
        );

        // Generate summary for notification
        let summary = if final_content.is_empty() {
            "Agent completed with no output".to_string()
        } else if final_content.len() > 500 {
            format!("{}...", &final_content[..500])
        } else {
            final_content.clone()
        };

        // Update registry status
        registry.update_status(agent_id, AsyncAgentStatus::Completed).await;

        info!(
            agent_type = %agent.agent_type,
            agent_id = %agent_id,
            duration_ms,
            tool_uses = total_tool_use_count,
            tokens = total_tokens,
            "Async agent completed"
        );

        info!(
            agent_id = %agent_id,
            description = %description,
            transcript_path = %transcript_path,
            "Agent notification: {summary}"
        );

        Ok(summary)
    }

    /// Build agent tool pool from ToolUseOptions.
    fn build_agent_tool_pool_from_options(
        agent: &FullAgentDefinition,
        options: &cc_core::tools::ToolUseOptions,
    ) -> cc_core::tools::Tools {
        let all_tools = &options.tools;

        let filtered: Vec<Arc<dyn cc_core::tools::Tool>> = if let Some(ref allowed) = agent.tools {
            all_tools
                .iter()
                .filter(|t| allowed.contains(&t.name().to_string()) || allowed.contains(&t.name().to_lowercase()))
                .cloned()
                .collect()
        } else {
            all_tools.iter().cloned().collect()
        };

        if let Some(ref denied) = agent.disallowed_tools {
            Arc::new(
                filtered
                    .into_iter()
                    .filter(|t| !denied.contains(&t.name().to_string()) && !denied.contains(&t.name().to_lowercase()))
                    .collect::<Vec<_>>(),
            )
        } else {
            Arc::new(filtered)
        }
    }

    /// Build agent system prompt from ToolUseOptions.
    fn build_agent_system_prompt_from_options(
        agent: &FullAgentDefinition,
        prompt: &str,
        options: &cc_core::tools::ToolUseOptions,
    ) -> Vec<SystemPromptBlock> {
        let mut parts = Vec::new();

        let agent_instructions = agent.get_system_prompt();
        if !agent_instructions.is_empty() {
            parts.push(SystemPromptBlock::Text {
                text: agent_instructions,
                cache_control: None,
            });
        }

        let base_instructions = format!(
            "You are a specialized subagent (type: {agent_type}) executing a task delegated by the main assistant.\n\
            Task description: {description}\n\n\
            Follow the instructions below carefully. Use available tools when needed.\n\
            When you have completed the task, provide a clear and concise result.",
            agent_type = agent.agent_type,
            description = agent.when_to_use,
        );
        parts.push(SystemPromptBlock::Text {
            text: base_instructions,
            cache_control: None,
        });

        let cwd = std::env::current_dir()
            .ok()
            .and_then(|p| p.to_str().map(|s| s.to_string()))
            .unwrap_or_else(|| "<unknown>".to_string());
        let env_context = format!(
            "Working directory: {cwd}\n\
            OS: {}\n\
            Architecture: {}",
            std::env::consts::OS,
            std::env::consts::ARCH,
        );
        parts.push(SystemPromptBlock::Text {
            text: env_context,
            cache_control: None,
        });

        if let Some(ref parent_prompt) = options.custom_system_prompt {
            parts.push(SystemPromptBlock::Text {
                text: parent_prompt.clone(),
                cache_control: None,
            });
        }

        parts
    }

    /// Static version of write_agent_transcript for use in async task.
    fn write_agent_transcript_static(
        agent_id: &str,
        agent: &FullAgentDefinition,
        prompt: &str,
        description: &str,
        messages: &[cc_core::messages::AssistantMessage],
        tool_use_count: usize,
        total_tokens: u64,
        duration_ms: u64,
    ) -> String {
        let transcript_path = format!(".claude/agents/{agent_id}/transcript.json");
        let dir = std::path::Path::new(&transcript_path)
            .parent()
            .unwrap_or(std::path::Path::new(".claude/agents"));

        if let Err(e) = std::fs::create_dir_all(dir) {
            warn!(error = %e, path = ?dir, "Failed to create agent transcript directory");
            return transcript_path;
        }

        let transcript = serde_json::json!({
            "agentId": agent_id,
            "agentType": agent.agent_type,
            "name": agent.name,
            "model": "unknown",
            "prompt": prompt,
            "description": description,
            "startTime": chrono::Utc::now().to_rfc3339(),
            "durationMs": duration_ms,
            "totalToolUseCount": tool_use_count,
            "totalTokens": total_tokens,
            "messages": messages.iter().map(|m| {
                serde_json::json!({
                    "role": "assistant",
                    "content": m.content.iter().map(|c| {
                        match c {
                            ContentBlockParam::Text { text } => serde_json::json!({"type": "text", "text": text}),
                            ContentBlockParam::ToolUse { id, name, input } => serde_json::json!({"type": "tool_use", "id": id, "name": name, "input": input}),
                            ContentBlockParam::Thinking { thinking, signature } => serde_json::json!({"type": "thinking", "thinking": thinking}),
                            _ => serde_json::json!({"type": "unknown"}),
                        }
                    }).collect::<Vec<_>>(),
                })
            }).collect::<Vec<_>>(),
        });

        if let Err(e) = std::fs::write(&transcript_path, serde_json::to_string_pretty(&transcript).unwrap_or_default()) {
            warn!(error = %e, path = %transcript_path, "Failed to write agent transcript");
        }

        transcript_path
    }

    /// Write agent metadata file for /resume support.
    fn write_agent_metadata(
        &self,
        agent_id: &str,
        agent: &FullAgentDefinition,
        prompt: &str,
        description: &str,
        model: &str,
    ) {
        let metadata_path = format!(".claude/agents/{agent_id}/metadata.json");
        let dir = std::path::Path::new(&metadata_path)
            .parent()
            .unwrap_or(std::path::Path::new(".claude/agents"));

        if let Err(e) = std::fs::create_dir_all(dir) {
            warn!(error = %e, path = ?dir, "Failed to create agent metadata directory");
            return;
        }

        let metadata = serde_json::json!({
            "agentId": agent_id,
            "agentType": agent.agent_type,
            "name": agent.name,
            "model": model,
            "prompt": prompt,
            "description": description,
            "startTime": chrono::Utc::now().to_rfc3339(),
            "status": "running",
        });

        if let Err(e) = std::fs::write(&metadata_path, serde_json::to_string_pretty(&metadata).unwrap_or_default()) {
            warn!(error = %e, path = %metadata_path, "Failed to write agent metadata");
        }
    }
}

#[async_trait]
impl Tool for AgentTool {
    fn name(&self) -> &str {
        AGENT_TOOL_NAME
    }

    fn aliases(&self) -> &[String] {
        static ALIASES: std::sync::LazyLock<Vec<String>> =
            std::sync::LazyLock::new(|| vec![LEGACY_AGENT_TOOL_NAME.to_string()]);
        &*ALIASES
    }

    fn search_hint(&self) -> Option<&str> {
        Some("delegate work to a subagent")
    }

    async fn call(
        &self,
        input: serde_json::Value,
        context: &ToolUseContext,
        on_progress: Option<Arc<dyn Fn(ToolProgress) + Send + Sync>>,
    ) -> anyhow::Result<ToolResult<serde_json::Value>> {
        let prompt = input["prompt"]
            .as_str()
            .ok_or_else(|| anyhow::anyhow!("prompt is required"))?;
        let description = input["description"]
            .as_str()
            .ok_or_else(|| anyhow::anyhow!("description is required"))?;

        let subagent_type = input["subagent_type"].as_str();
        let model_override = input["model"].as_str();
        let run_in_background = input["run_in_background"].as_bool().unwrap_or(false);
        let name = input["name"].as_str();
        let team_name = input["team_name"].as_str();
        let isolation = input["isolation"].as_str();
        let cwd = input["cwd"].as_str();

        // Get parent model
        let parent_model = &context.options.main_loop_model;

        // Get active agents
        let agents = self.get_active_agents(context);

        // Select agent
        let selected_agent = if let Some(agent_type) = subagent_type {
            agents
                .iter()
                .find(|a| a.agent_type == agent_type)
                .ok_or_else(|| {
                    let available: Vec<&str> = agents.iter().map(|a| a.agent_type.as_str()).collect();
                    anyhow::anyhow!(
                        "Agent type '{agent_type}' not found. Available agents: {}",
                        available.join(", ")
                    )
                })?
        } else {
            // Default to general-purpose
            agents
                .iter()
                .find(|a| a.agent_type == "general-purpose")
                .ok_or_else(|| anyhow::anyhow!("No general-purpose agent found"))?
        };

        // Resolve model
        let model = Self::resolve_model(selected_agent, parent_model, model_override);

        // Set agent color from definition
        if let Some(ref color) = selected_agent.color {
            self.color_manager
                .set_color(&selected_agent.agent_type, color)
                .await;
        }

        // Generate unique name if provided
        let agent_name = if let Some(n) = name {
            Self::sanitize_name(n)
        } else {
            selected_agent.agent_type.clone()
        };

        // Generate agent ID
        let team = team_name.unwrap_or("default");
        let agent_id = Self::format_agent_id(&agent_name, team);

        // Log agent selection
        debug!(
            agent_type = %selected_agent.agent_type,
            model = %model,
            is_async = run_in_background,
            "Agent selected"
        );

        // Handle isolation mode (worktree)
        if let Some(mode) = isolation {
            if mode == "worktree" {
                // In a full implementation, create a git worktree
                info!("Worktree isolation requested (not yet implemented)");
            } else if mode == "remote" {
                // In a full implementation, spawn remote agent
                info!("Remote isolation requested (not yet implemented)");
            }
        }

        // Handle cwd override
        if let Some(_cwd_path) = cwd {
            debug!("CWD override requested: {}", _cwd_path);
        }

        // Run agent (sync or async)
        let result = if run_in_background || selected_agent.background {
            self.run_async_agent(selected_agent, prompt, description, context, &model)
                .await?
        } else {
            self.run_sync_agent(selected_agent, prompt, description, context, &model, on_progress)
                .await?
        };

        Ok(ToolResult {
            data: result,
            new_messages: None,
            mcp_meta: None,
        })
    }

    async fn description(
        &self,
        _input: serde_json::Value,
        _options: &cc_core::tools::DescriptionOptions,
    ) -> anyhow::Result<String> {
        Ok("Launch a new agent".to_string())
    }

    fn input_schema(&self) -> serde_json::Value {
        serde_json::json!({
            "type": "object",
            "properties": {
                "description": {
                    "type": "string",
                    "description": "A short (3-5 word) description of the task"
                },
                "prompt": {
                    "type": "string",
                    "description": "The task for the agent to perform"
                },
                "subagent_type": {
                    "type": "string",
                    "description": "The type of specialized agent to use for this task"
                },
                "model": {
                    "type": "string",
                    "enum": ["sonnet", "opus", "haiku"],
                    "description": "Optional model override for this agent"
                },
                "run_in_background": {
                    "type": "boolean",
                    "description": "Set to true to run this agent in the background"
                },
                "name": {
                    "type": "string",
                    "description": "Name for the spawned agent"
                },
                "team_name": {
                    "type": "string",
                    "description": "Team name for spawning"
                },
                "isolation": {
                    "type": "string",
                    "enum": ["worktree", "remote"],
                    "description": "Isolation mode"
                },
                "cwd": {
                    "type": "string",
                    "description": "Absolute path to run the agent in"
                }
            },
            "required": ["description", "prompt"]
        })
    }

    fn is_concurrency_safe(&self, _input: &serde_json::Value) -> bool {
        true
    }

    fn is_read_only(&self, _input: &serde_json::Value) -> bool {
        false
    }

    fn interrupt_behavior(&self) -> InterruptBehavior {
        InterruptBehavior::Cancel
    }

    fn is_search_or_read_command(&self, _input: &serde_json::Value) -> SearchOrReadInfo {
        SearchOrReadInfo::default()
    }

    fn is_open_world(&self, _input: &serde_json::Value) -> bool {
        true
    }

    fn max_result_size_chars(&self) -> usize {
        MAX_RESULT_SIZE_CHARS
    }

    async fn validate_input(
        &self,
        input: &serde_json::Value,
        _context: &ToolUseContext,
    ) -> anyhow::Result<ValidationResult> {
        if input["prompt"].as_str().is_none() {
            return Ok(ValidationResult::Invalid {
                message: "prompt is required".to_string(),
                error_code: 1,
            });
        }
        if input["description"].as_str().is_none() {
            return Ok(ValidationResult::Invalid {
                message: "description is required".to_string(),
                error_code: 1,
            });
        }
        Ok(ValidationResult::Valid)
    }

    async fn check_permissions(
        &self,
        input: &serde_json::Value,
        context: &ToolUseContext,
    ) -> anyhow::Result<PermissionResult> {
        check_read_permission(input, context, "agent")
    }

    async fn prompt(&self, options: &ToolPromptOptions) -> anyhow::Result<String> {
        let agents: Vec<FullAgentDefinition> = options
            .agents
            .iter()
            .map(|def| FullAgentDefinition {
                agent_type: def.agent_type.clone(),
                name: def.name.clone(),
                when_to_use: def.description.clone(),
                model: None,
                tools: None,
                disallowed_tools: None,
                permission_mode: None,
                required_mcp_servers: None,
                isolation: None,
                background: false,
                color: None,
                instructions: None,
                source: AgentSource::Custom(String::new()),
            })
            .collect();
        let tools = &options.tools;

        // Get MCP servers that have tools
        let mut mcp_servers_with_tools: Vec<String> = Vec::new();
        for tool in tools.iter() {
            let name = tool.name();
            if name.starts_with("mcp__") {
                let parts: Vec<&str> = name.split("__").collect();
                if parts.len() >= 2 {
                    let server_name = parts[1].to_string();
                    if !mcp_servers_with_tools.contains(&server_name) {
                        mcp_servers_with_tools.push(server_name);
                    }
                }
            }
        }

        // Filter agents by MCP requirements
        let agents_with_mcp: Vec<&FullAgentDefinition> = agents
            .iter()
            .filter(|a| {
                if let Some(ref required) = a.required_mcp_servers {
                    required.iter().all(|pattern| {
                        mcp_servers_with_tools
                            .iter()
                            .any(|s| s.to_lowercase().contains(&pattern.to_lowercase()))
                    })
                } else {
                    true
                }
            })
            .collect();

        // Format agent list
        let agent_list: Vec<String> = agents_with_mcp
            .iter()
            .map(|a| {
                let tools_desc = format_agent_tools(a);
                format!("- {}: {} (Tools: {})", a.agent_type, a.when_to_use, tools_desc)
            })
            .collect();

        let agent_list_section = if agent_list.is_empty() {
            "Available agent types are listed in <system-reminder> messages in the conversation."
        } else {
            &format!(
                "Available agent types and the tools they have access to:\n{}",
                agent_list.join("\n")
            )
        };

        Ok(format_agent_prompt(agent_list_section))
    }

    fn user_facing_name(&self, _input: Option<&serde_json::Value>) -> String {
        "Agent".to_string()
    }

    fn get_activity_description(&self, input: Option<&serde_json::Value>) -> Option<String> {
        if let Some(input) = input {
            if let Some(desc) = input["description"].as_str() {
                return Some(format!("Launching agent: {desc}"));
            }
        }
        Some("Launching a new agent".to_string())
    }

    fn get_tool_use_summary(&self, input: Option<&serde_json::Value>) -> Option<String> {
        if let Some(input) = input {
            return input["description"].as_str().map(String::from);
        }
        None
    }

    fn map_tool_result_to_block(
        &self,
        content: serde_json::Value,
        tool_use_id: &str,
    ) -> ContentBlockParam {
        let status = content["status"].as_str().unwrap_or("unknown");
        let result_text = if let Some(c) = content["content"].as_str() {
            c.to_string()
        } else if let Some(p) = content["prompt"].as_str() {
            format!("Agent launched: {p}")
        } else {
            format!("Agent completed with status: {status}")
        };

        ContentBlockParam::ToolResult {
            tool_use_id: tool_use_id.to_string(),
            content: vec![cc_core::messages::ToolResultContent::Text { text: result_text }],
            is_error: None,
        }
    }
}

// =========================================================================
// Helper Functions
// =========================================================================

/// Format an agent's tool access description.
fn format_agent_tools(agent: &FullAgentDefinition) -> String {
    match (&agent.tools, &agent.disallowed_tools) {
        (Some(allow), Some(deny)) => {
            let deny_set: std::collections::HashSet<&str> =
                deny.iter().map(|s| s.as_str()).collect();
            let effective: Vec<&str> = allow
                .iter()
                .map(|s| s.as_str())
                .filter(|t| !deny_set.contains(t))
                .collect();
            if effective.is_empty() {
                "None".to_string()
            } else {
                effective.join(", ")
            }
        }
        (Some(allow), None) => allow.join(", "),
        (None, Some(deny)) => format!("All tools except {}", deny.join(", ")),
        (None, None) => "All tools".to_string(),
    }
}

/// Format the full agent tool prompt.
fn format_agent_prompt(agent_list_section: &str) -> String {
    format!(
        r#"Launch a new agent to handle complex, multi-step tasks autonomously.

The {tool_name} tool launches specialized agents (subprocesses) that autonomously handle complex tasks. Each agent type has specific capabilities and tools available to it.

{agent_list}

When using the {tool_name} tool, specify a subagent_type parameter to select which agent type to use. If omitted, the general-purpose agent is used.

When NOT to use the {tool_name} tool:
- If you want to read a specific file path, use the Read tool or the Glob tool instead, to find the match more quickly
- If you are searching for a specific class definition like "class Foo", use the Glob tool instead, to find the match more quickly
- If you are searching for code within a specific file or set of 2-3 files, use the Read tool instead of the {tool_name} tool, to find the match more quickly
- Other tasks that are not related to the agent descriptions above

Usage notes:
- Always include a short description (3-5 words) summarizing what the agent will do
- Launch multiple agents concurrently whenever possible, to maximize performance; to do that, use a single message with multiple tool uses
- When the agent is done, it will return a single message back to you. The result returned by the agent is not visible to the user. To show the user the result, you should send a text message back to the user with a concise summary of the result.
- You can optionally run agents in the background using the run_in_background parameter. When an agent runs in the background, you will be automatically notified when it completes — do NOT sleep, poll, or proactively check on its progress. Continue with other work or respond to the user instead.
- To continue a previously spawned agent, use SendMessage with the agent's ID or name as the `to` field. The agent resumes with its full context preserved.
- Each Agent invocation starts fresh — provide a complete task description.
- The agent's outputs should generally be trusted.
- Clearly tell the agent whether you expect it to write code or just to do research (search, file reads, web fetches, etc.), since it is not aware of the user's intent.
- If the agent description mentions that it should be used proactively, then you should try your best to use it without the user having to ask for it first. Use your judgement.
- If the user specifies that they want you to run agents "in parallel", you MUST send a single message with multiple {tool_name} tool use content blocks.
- You can optionally set `isolation: "worktree"` to run the agent in a temporary git worktree, giving it an isolated copy of the repository.

Example usage:

<example_agent_descriptions>
"test-runner": use this agent after you are done writing code to run tests
"greeting-responder": use this agent to respond to user greetings with a friendly joke
</example_agent_descriptions>

<example>
user: "Please write a function that checks if a number is prime"
assistant: I'm going to use the Write tool to write the following code:
<code>
function isPrime(n) {{
  if (n <= 1) return false
  for (let i = 2; i * i <= n; i++) {{
    if (n % i === 0) return false
  }}
  return true
}}
</code>
<commentary>
Since a significant piece of code was written and the task was completed, now use the test-runner agent to run the tests
</commentary>
assistant: Uses the {tool_name} tool to launch the test-runner agent
</example>

<example>
user: "Hello"
<commentary>
Since the user is greeting, use the greeting-responder agent to respond with a friendly joke
</commentary>
assistant: "I'm going to use the {tool_name} tool to launch the greeting-responder agent"
</example>"#,
        tool_name = AGENT_TOOL_NAME,
        agent_list = agent_list_section,
    )
}
