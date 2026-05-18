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

    /// Run a sync agent with worktree isolation and cwd override.
    async fn run_sync_agent_with_worktree(
        &self,
        agent: &FullAgentDefinition,
        prompt: &str,
        description: &str,
        context: &ToolUseContext,
        model: &str,
        on_progress: Option<Arc<dyn Fn(ToolProgress) + Send + Sync>>,
        worktree: Option<&WorktreeInfo>,
        effective_cwd: &str,
    ) -> anyhow::Result<serde_json::Value> {
        let start = std::time::Instant::now();
        let agent_id = uuid::Uuid::new_v4().to_string();

        info!(
            agent_type = %agent.agent_type,
            model = %model,
            description = %description,
            cwd = %effective_cwd,
            "Starting sync agent with worktree isolation"
        );

        // Set agent color
        if let Some(ref color) = agent.color {
            self.color_manager
                .set_color(&agent.agent_type, color)
                .await;
        }

        // Build agent-specific tool pool
        let agent_tools = self.build_agent_tool_pool(agent, context);

        // Build agent-specific system prompt with worktree context
        let mut system_prompt = self.build_agent_system_prompt(agent, prompt, context);

        // Inject worktree notice if applicable
        if let Some(wt) = worktree {
            let parent_cwd = std::env::current_dir()
                .ok()
                .and_then(|p| p.to_str().map(|s| s.to_string()))
                .unwrap_or_default();
            let notice = build_worktree_notice(&parent_cwd, &wt.worktree_path);
            system_prompt.push(SystemPromptBlock::Text {
                text: notice,
                cache_control: None,
            });
        }

        // Inject cwd into system prompt
        system_prompt.push(SystemPromptBlock::Text {
            text: format!("Current working directory: {effective_cwd}"),
            cache_control: None,
        });

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

        // Run the query loop with cwd override
        let mut event_stream = std::pin::pin!(run_with_cwd_override(effective_cwd, async {
            query_engine.submit_message(user_msg).await
        }).await);

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
                        if let cc_core::messages::StreamEvent::MessageDelta { usage, .. } = &core_event {
                            total_input_tokens += usage.input_tokens;
                            total_output_tokens += usage.output_tokens;
                        }
                        progress_tracker.update_from_stream(&core_event);
                        if let Some(ref cb) = on_progress {
                            cb(progress_tracker.to_progress(&agent_id));
                        }
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
            "worktree_path": worktree.map(|wt| wt.worktree_path.clone()),
        }))
    }

    /// Run an async agent with worktree isolation and cwd override.
    async fn run_async_agent_with_worktree(
        &self,
        agent: &FullAgentDefinition,
        prompt: &str,
        description: &str,
        context: &ToolUseContext,
        model: &str,
        worktree: Option<&WorktreeInfo>,
        effective_cwd: &str,
    ) -> anyhow::Result<serde_json::Value> {
        let agent_id = uuid::Uuid::new_v4().to_string();
        let output_file = format!(".claude/agents/{agent_id}/transcript.json");

        info!(
            agent_type = %agent.agent_type,
            model = %model,
            description = %description,
            agent_id = %agent_id,
            cwd = %effective_cwd,
            "Starting async agent with worktree isolation"
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
        let effective_cwd_clone = effective_cwd.to_string();
        let worktree_branch = worktree.map(|wt| wt.worktree_branch.clone());
        let worktree_head = worktree.map(|wt| wt.head_commit.clone());

        // Spawn the background task
        tokio::spawn(async move {
            let result = Self::run_async_agent_loop_with_worktree(
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
                &effective_cwd_clone,
                worktree_branch.as_deref(),
                worktree_head.as_deref(),
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
            "worktree_path": worktree.map(|wt| wt.worktree_path.clone()),
        }))
    }

    /// The actual async agent loop with worktree isolation (runs in background tokio task).
    async fn run_async_agent_loop_with_worktree(
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
        effective_cwd: &str,
        worktree_branch: Option<&str>,
        worktree_head: Option<&str>,
    ) -> anyhow::Result<String> {
        let start = std::time::Instant::now();

        // Build agent-specific tool pool
        let agent_tools = Self::build_agent_tool_pool_from_options(agent, options);

        // Build agent-specific system prompt with worktree context
        let mut system_prompt = Self::build_agent_system_prompt_from_options(agent, prompt, options);

        // Inject worktree notice if applicable
        if let Some(branch) = worktree_branch {
            let notice = format!(
                "You are running in an isolated git worktree on branch '{branch}'.\n\
                All file operations are confined to this worktree.\n\
                Changes you make will not affect the parent repository until merged.",
            );
            system_prompt.push(SystemPromptBlock::Text {
                text: notice,
                cache_control: None,
            });
        }

        // Inject cwd into system prompt
        system_prompt.push(SystemPromptBlock::Text {
            text: format!("Current working directory: {effective_cwd}"),
            cache_control: None,
        });

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

        // Run the query loop with cwd override
        let mut event_stream = std::pin::pin!(run_with_cwd_override(effective_cwd, async {
            query_engine.submit_message(user_msg).await
        }).await);

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
            "Async agent with worktree completed"
        );

        Ok(summary)
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
        let worktree_info = if let Some(mode) = isolation {
            if mode == "worktree" {
                let slug = format!("agent-{}", &agent_id[..8.min(agent_id.len())]);
                match create_agent_worktree(&slug).await {
                    Ok(info) => {
                        info!(worktree_path = %info.worktree_path, branch = %info.worktree_branch, "Created worktree for agent");
                        Some(info)
                    }
                    Err(e) => {
                        warn!(error = %e, "Failed to create worktree, falling back to current directory");
                        None
                    }
                }
            } else if mode == "remote" {
                info!("Remote isolation requested (not yet implemented)");
                None
            } else {
                None
            }
        } else {
            None
        };

        // Handle cwd override (worktree takes precedence)
        let effective_cwd = if let Some(ref wt) = worktree_info {
            wt.worktree_path.clone()
        } else if let Some(cwd_path) = cwd {
            cwd_path.to_string()
        } else {
            std::env::current_dir()
                .ok()
                .and_then(|p| p.to_str().map(|s| s.to_string()))
                .unwrap_or_default()
        };

        // Run agent (sync or async)
        let result = if run_in_background || selected_agent.background {
            self.run_async_agent_with_worktree(selected_agent, prompt, description, context, &model, worktree_info.as_ref(), &effective_cwd)
                .await?
        } else {
            self.run_sync_agent_with_worktree(selected_agent, prompt, description, context, &model, on_progress, worktree_info.as_ref(), &effective_cwd)
                .await?
        };

        // Auto-cleanup worktree if agent completed without changes
        if let Some(ref wt) = worktree_info {
            cleanup_worktree_if_no_changes(wt).await;
        }

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
// Worktree Isolation (Phase 10.3d)
// =========================================================================

/// Information about a created worktree.
#[derive(Debug, Clone)]
pub struct WorktreeInfo {
    /// Path to the worktree directory.
    pub worktree_path: String,
    /// Branch name created for the worktree.
    pub worktree_branch: String,
    /// HEAD commit at creation time.
    pub head_commit: String,
    /// Git root of the original repo.
    pub git_root: String,
    /// Slug used for the worktree name.
    pub slug: String,
}

/// Create a git worktree for agent isolation.
/// Runs: `git worktree add .claude/worktrees/<slug> -b agent-<slug>`
async fn create_agent_worktree(slug: &str) -> anyhow::Result<WorktreeInfo> {
    // Find git root
    let git_root = find_git_root().await?;

    let worktree_path = format!("{git_root}/.claude/worktrees/{slug}");
    let branch_name = format!("agent-{slug}");

    // Get current HEAD commit
    let head_commit = run_git_command(&git_root, &["rev-parse", "HEAD"]).await?;

    // Create the worktree
    run_git_command(
        &git_root,
        &["worktree", "add", &worktree_path, "-b", &branch_name],
    )
    .await?;

    info!(
        worktree_path = %worktree_path,
        branch = %branch_name,
        head_commit = %head_commit,
        "Created agent worktree"
    );

    Ok(WorktreeInfo {
        worktree_path,
        worktree_branch: branch_name,
        head_commit,
        git_root,
        slug: slug.to_string(),
    })
}

/// Find the git root directory.
async fn find_git_root() -> anyhow::Result<String> {
    let output = tokio::process::Command::new("git")
        .args(["rev-parse", "--show-toplevel"])
        .output()
        .await
        .map_err(|e| anyhow::anyhow!("Failed to run git: {e}"))?;

    if !output.status.success() {
        return Err(anyhow::anyhow!("Not a git repository"));
    }

    let path = String::from_utf8_lossy(&output.stdout);
    Ok(path.trim().to_string())
}

/// Run a git command in the specified directory.
async fn run_git_command(git_root: &str, args: &[&str]) -> anyhow::Result<String> {
    let output = tokio::process::Command::new("git")
        .current_dir(git_root)
        .args(args)
        .output()
        .await
        .map_err(|e| anyhow::anyhow!("Failed to run git {:?}: {e}", args))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(anyhow::anyhow!(
            "git {:?} failed: {}",
            args,
            stderr.trim()
        ));
    }

    Ok(String::from_utf8_lossy(&output.stdout).trim().to_string())
}

/// Check if a worktree has changes compared to its head commit.
async fn has_worktree_changes(worktree_path: &str, head_commit: &str) -> bool {
    let result = run_git_command(
        worktree_path,
        &["diff", "--quiet", "HEAD"],
    )
    .await;

    // diff --quiet returns non-zero if there are changes
    result.is_err()
}

/// Check if worktree has untracked files.
async fn has_untracked_files(worktree_path: &str) -> bool {
    let output = tokio::process::Command::new("git")
        .current_dir(worktree_path)
        .args(["ls-files", "--others", "--exclude-standard"])
        .output()
        .await;

    match output {
        Ok(out) => !out.stdout.is_empty(),
        Err(_) => false,
    }
}

/// Auto-cleanup worktree if no changes were made.
async fn cleanup_worktree_if_no_changes(worktree: &WorktreeInfo) {
    let has_changes = has_worktree_changes(&worktree.worktree_path, &worktree.head_commit).await
        || has_untracked_files(&worktree.worktree_path).await;

    if has_changes {
        info!(
            worktree_path = %worktree.worktree_path,
            branch = %worktree.worktree_branch,
            "Worktree has changes, keeping it for review"
        );
        // Write a notice file
        let notice_path = format!("{}/.agent-worktree-notice.txt", worktree.worktree_path);
        let notice = format!(
            "This worktree was created by an agent session.\n\
            Branch: {}\n\
            Original HEAD: {}\n\
            The agent made changes here. Review and merge when ready.",
            worktree.worktree_branch,
            worktree.head_commit
        );
        if let Err(e) = std::fs::write(&notice_path, notice) {
            warn!(error = %e, "Failed to write worktree notice");
        }
    } else {
        info!(
            worktree_path = %worktree.worktree_path,
            "No changes in worktree, cleaning up"
        );
        // Remove the worktree
        let _ = run_git_command(
            &worktree.git_root,
            &["worktree", "remove", "-f", &worktree.worktree_path],
        )
        .await;

        // Delete the branch
        let _ = run_git_command(
            &worktree.git_root,
            &["branch", "-D", &worktree.worktree_branch],
        )
        .await;
    }
}

/// Build worktree notice for fork children (path translation info).
fn build_worktree_notice(parent_cwd: &str, child_cwd: &str) -> String {
    format!(
        "NOTE: This agent is running in an isolated worktree.\n\
        Parent working directory: {parent_cwd}\n\
        Your working directory: {child_cwd}\n\
        When referencing file paths, use paths relative to your working directory.",
    )
}

/// Run a future with a temporary cwd override.
async fn run_with_cwd_override<T, F: std::future::Future<Output = T>>(cwd: &str, f: F) -> T {
    let original_cwd = std::env::current_dir().ok();

    // Change to the target directory
    let _ = std::env::set_current_dir(cwd);

    // Run the future
    let result = f.await;

    // Restore original directory
    if let Some(ref orig) = original_cwd {
        let _ = std::env::set_current_dir(orig);
    }

    result
}

// =========================================================================
// Fork Subagent (Phase 10.3e)
// =========================================================================

/// Context for a fork subagent, inherited from parent.
#[derive(Debug, Clone)]
pub struct ForkContext {
    /// Parent's full conversation messages (assistant messages with tool_use blocks).
    pub parent_messages: Vec<cc_core::messages::Message>,
    /// Parent's rendered system prompt (cache-identical).
    pub parent_system_prompt: String,
    /// Parent's exact tool array (cache-identical prefix).
    pub parent_tools: cc_core::tools::Tools,
    /// Parent's working directory.
    pub parent_cwd: String,
    /// Parent's thinking config (inherited).
    pub thinking_config: cc_core::tools::ThinkingConfig,
    /// Whether this is a non-interactive session.
    pub is_non_interactive: bool,
}

/// Result of building forked messages.
pub struct ForkedMessages {
    /// Messages to send to the fork agent.
    pub messages: Vec<cc_core::messages::Message>,
    /// Fork directive appended to the prompt.
    pub fork_directive: String,
}

/// Check if the current context is already inside a fork (fork guard).
/// Prevents recursive fork in children.
pub fn is_inside_fork(context: &ToolUseContext) -> bool {
    // Check query_source for agent marker
    if context.options.query_source.is_some() {
        // If we're already running as an agent, check if it's a fork agent
        // by looking at the messages for fork markers
    }

    // Scan messages for fork child marker
    for msg in &context.messages {
        if let cc_core::messages::Message::User(u) = msg {
            for block in &u.content {
                if let ContentBlockParam::Text { text } = block {
                    if text.contains("[FORK_CHILD_MARKER]") {
                        return true;
                    }
                }
            }
        }
    }

    false
}

/// Build forked messages from parent context.
/// Fork inherits parent's FULL conversation context:
/// - Clone parent's assistant messages as user messages
/// - Add placeholder tool_results for each tool_use block
/// - Append fork directive + user prompt
pub fn build_forked_messages(
    fork_context: &ForkContext,
    user_prompt: &str,
) -> ForkedMessages {
    let mut messages = Vec::new();
    let mut tool_use_counter = 0;

    for msg in &fork_context.parent_messages {
        match msg {
            cc_core::messages::Message::Assistant(assistant) => {
                // Clone assistant message as user message
                let mut user_content = Vec::new();

                for block in &assistant.content {
                    match block {
                        ContentBlockParam::Text { text } => {
                            user_content.push(ContentBlockParam::Text {
                                text: text.clone(),
                            });
                        }
                        ContentBlockParam::ToolUse { id, name, input } => {
                            // Add placeholder tool_result for each tool_use
                            tool_use_counter += 1;
                            user_content.push(ContentBlockParam::ToolResult {
                                tool_use_id: format!("fork-placeholder-{tool_use_counter}"),
                                content: vec![cc_core::messages::ToolResultContent::Text {
                                    text: format!(
                                        "[Result of {name} call — fork inherits this context]"
                                    ),
                                }],
                                is_error: Some(false),
                            });
                        }
                        ContentBlockParam::Thinking { thinking, .. } => {
                            user_content.push(ContentBlockParam::Text {
                                text: format!("[Thinking: {thinking}]"),
                            });
                        }
                        _ => {}
                    }
                }

                if !user_content.is_empty() {
                    messages.push(cc_core::messages::Message::User(
                        cc_core::messages::UserMessage {
                            id: uuid::Uuid::new_v4(),
                            content: user_content,
                            timestamp: assistant.timestamp,
                            is_meta: None,
                            origin_query_source: None,
                            effort: None,
                        },
                    ));
                }
            }
            cc_core::messages::Message::User(user) => {
                // Skip the original user message (we'll add our own with fork directive)
                // But keep attachment/system messages
                messages.push(msg.clone());
            }
            _ => {
                messages.push(msg.clone());
            }
        }
    }

    // Build fork directive
    let fork_directive = format!(
        "[FORK_CHILD_MARKER]\n\n\
        === FORK DIRECTIVE ===\n\
        You are a forked subagent. The conversation context above has been inherited \
        from the parent agent's full session. You have access to all previous tool \
        calls and their results.\n\n\
        Your task is: {prompt}\n\n\
        IMPORTANT:\n\
        - Do NOT re-explain background already covered in the inherited context.\n\
        - Be specific about scope: what's in, what's out, what another agent is handling.\n\
        - You have the same tools as the parent agent (cache-identical tool array).\n\
        - Parent working directory was: {cwd}\n\
        =====================",
        prompt = user_prompt,
        cwd = fork_context.parent_cwd,
    );

    // Add the fork directive as the final user message
    messages.push(cc_core::messages::Message::User(
        cc_core::messages::UserMessage {
            id: uuid::Uuid::new_v4(),
            content: vec![ContentBlockParam::Text {
                text: fork_directive.clone(),
            }],
            timestamp: chrono::Utc::now(),
            is_meta: None,
            origin_query_source: None,
            effort: None,
        },
    ));

    ForkedMessages {
        messages,
        fork_directive,
    }
}

/// Extract assistant messages with tool_use blocks from context for fork inheritance.
pub fn extract_fork_context_messages(context: &ToolUseContext) -> Vec<cc_core::messages::Message> {
    context
        .messages
        .iter()
        .filter(|msg| matches!(msg, cc_core::messages::Message::Assistant(_)))
        .cloned()
        .collect()
}

/// Build a ForkContext from the current ToolUseContext.
pub fn build_fork_context(context: &ToolUseContext) -> ForkContext {
    let parent_messages = extract_fork_context_messages(context);

    // Build parent system prompt string from components
    let mut prompt_parts = Vec::new();
    if let Some(ref custom) = context.options.custom_system_prompt {
        prompt_parts.push(custom.clone());
    }
    if let Some(ref append) = context.options.append_system_prompt {
        prompt_parts.push(append.clone());
    }
    let parent_system_prompt = prompt_parts.join("\n\n");

    ForkContext {
        parent_messages,
        parent_system_prompt,
        parent_tools: context.options.tools.clone(),
        parent_cwd: std::env::current_dir()
            .ok()
            .and_then(|p| p.to_str().map(|s| s.to_string()))
            .unwrap_or_default(),
        thinking_config: context.options.thinking_config.clone(),
        is_non_interactive: context.options.is_non_interactive_session,
    }
}

/// Create an error response for fork guard violation.
pub fn fork_guard_error() -> anyhow::Result<serde_json::Value> {
    Err(anyhow::anyhow!(
        "Fork is not available inside a forked worker. \
        Recursive forking is not allowed to prevent infinite delegation chains."
    ))
}

/// Format fork examples for the tool prompt (port from TS).
fn format_fork_examples() -> String {
    r#"
<fork_examples>
Example 1 — Ship audit:
user: "Audit the changes in this PR and make sure everything looks good"
assistant: Uses the Agent tool with subagent_type "fork" to delegate the audit
  prompt: "Review all changes since the last commit. Check for: consistency, edge cases, missing tests."

Example 2 — Mid-wait status:
user: "What's the status of the background task?"
assistant: Uses the Agent tool with subagent_type "fork"
  prompt: "Check the status of the running task and report back."

Example 3 — Migration review:
user: "I just migrated from X to Y. Can you review my work?"
assistant: Uses the Agent tool with subagent_type "fork"
  prompt: "Review the migration from X to Y. Check for correctness, edge cases, and performance implications."
</fork_examples>"#
        .to_string()
}

// =========================================================================
// Agent Swarms — tmux/iTerm2 Spawning (Phase 10.3f)
// =========================================================================

/// Constants for agent swarms.
pub const SWARM_SESSION_NAME: &str = "claude-swarm";
pub const TEAM_LEAD_NAME: &str = "lead";
pub const TMUX_COMMAND: &str = "tmux";

/// Backend type for teammate spawning.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum SpawnBackend {
    Tmux,
    Iterm2,
    InProcess,
}

/// Team member in a swarm.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TeamMember {
    pub agent_id: String,
    pub name: String,
    pub agent_type: String,
    pub model: String,
    pub prompt: String,
    pub color: String,
    pub plan_mode_required: bool,
    pub joined_at: chrono::DateTime<chrono::Utc>,
    pub tmux_pane_id: Option<String>,
    pub cwd: String,
    pub backend_type: SpawnBackend,
}

/// Team file structure.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TeamFile {
    pub name: String,
    pub members: Vec<TeamMember>,
    pub lead_agent_id: String,
}

/// Mailbox message for inter-agent communication.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MailboxMessage {
    pub from: String,
    pub text: String,
    pub timestamp: chrono::DateTime<chrono::Utc>,
}

/// Detect available spawn backend.
pub async fn detect_spawn_backend() -> SpawnBackend {
    // Check tmux availability
    if check_command_available(TMUX_COMMAND).await {
        return SpawnBackend::Tmux;
    }

    // Check iTerm2 + it2 availability
    if check_command_available("it2").await {
        return SpawnBackend::Iterm2;
    }

    // Fallback to in-process
    SpawnBackend::InProcess
}

/// Check if a command is available in PATH.
async fn check_command_available(cmd: &str) -> bool {
    tokio::process::Command::new(cmd)
        .arg("--version")
        .output()
        .await
        .is_ok()
}

/// Create a tmux session for the swarm.
async fn create_tmux_session(session_name: &str) -> anyhow::Result<()> {
    // Check if session already exists
    let exists = tokio::process::Command::new(TMUX_COMMAND)
        .args(["has-session", "-t", session_name])
        .output()
        .await
        .map(|o| o.status.success())
        .unwrap_or(false);

    if exists {
        return Ok(());
    }

    // Create detached session
    let output = tokio::process::Command::new(TMUX_COMMAND)
        .args(["new-session", "-d", "-s", session_name])
        .output()
        .await
        .map_err(|e| anyhow::anyhow!("Failed to create tmux session: {e}"))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(anyhow::anyhow!("tmux new-session failed: {}", stderr.trim()));
    }

    Ok(())
}

/// Create a new tmux window in a session.
async fn create_tmux_window(session_name: &str, window_name: &str) -> anyhow::Result<String> {
    let output = tokio::process::Command::new(TMUX_COMMAND)
        .args([
            "new-window",
            "-t",
            session_name,
            "-n",
            window_name,
            "-P",
            "-F",
            "#{pane_id}",
        ])
        .output()
        .await
        .map_err(|e| anyhow::anyhow!("Failed to create tmux window: {e}"))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(anyhow::anyhow!("tmux new-window failed: {}", stderr.trim()));
    }

    Ok(String::from_utf8_lossy(&output.stdout).trim().to_string())
}

/// Split a tmux pane.
async fn split_tmux_pane(target: &str, vertical: bool) -> anyhow::Result<String> {
    let args = if vertical {
        vec!["split-window", "-t", target, "-P", "-F", "#{pane_id}"]
    } else {
        vec!["split-window", "-h", "-t", target, "-P", "-F", "#{pane_id}"]
    };

    let output = tokio::process::Command::new(TMUX_COMMAND)
        .args(&args)
        .output()
        .await
        .map_err(|e| anyhow::anyhow!("Failed to split tmux pane: {e}"))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(anyhow::anyhow!("tmux split-window failed: {}", stderr.trim()));
    }

    Ok(String::from_utf8_lossy(&output.stdout).trim().to_string())
}

/// Send keys to a tmux pane.
async fn send_tmux_keys(target: &str, keys: &str) -> anyhow::Result<()> {
    let output = tokio::process::Command::new(TMUX_COMMAND)
        .args(["send-keys", "-t", target, keys, "Enter"])
        .output()
        .await
        .map_err(|e| anyhow::anyhow!("Failed to send tmux keys: {e}"))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(anyhow::anyhow!("tmux send-keys failed: {}", stderr.trim()));
    }

    Ok(())
}

/// Kill a tmux pane.
async fn kill_tmux_pane(target: &str) -> anyhow::Result<()> {
    let output = tokio::process::Command::new(TMUX_COMMAND)
        .args(["kill-pane", "-t", target])
        .output()
        .await
        .map_err(|e| anyhow::anyhow!("Failed to kill tmux pane: {e}"))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(anyhow::anyhow!("tmux kill-pane failed: {}", stderr.trim()));
    }

    Ok(())
}

/// Spawn a teammate in a tmux session.
pub async fn spawn_teammate_tmux(
    team_name: &str,
    member: &TeamMember,
    claude_command: &str,
) -> anyhow::Result<String> {
    // Create or verify swarm session
    create_tmux_session(SWARM_SESSION_NAME).await?;

    // Create window for the team member
    let window_name = format!("{team_name}-{}", member.name);
    let pane_id = create_tmux_window(SWARM_SESSION_NAME, &window_name).await?;

    // Build the command with agent identity CLI args
    let agent_cmd = build_agent_command(claude_command, member, team_name);

    // Send the command to the pane
    send_tmux_keys(&pane_id, &agent_cmd).await?;

    info!(
        team_name = %team_name,
        member_name = %member.name,
        pane_id = %pane_id,
        "Spawned teammate in tmux"
    );

    Ok(pane_id)
}

/// Build the command line for an agent with identity args.
fn build_agent_command(base_command: &str, member: &TeamMember, team_name: &str) -> String {
    let mut cmd = base_command.to_string();

    cmd.push_str(&format!(" --agent-id {}", member.agent_id));
    cmd.push_str(&format!(" --agent-name {}", member.name));
    cmd.push_str(&format!(" --team-name {team_name}"));
    cmd.push_str(&format!(" --agent-color {}", member.color));
    cmd.push_str(&format!(" --agent-type {}", member.agent_type));

    if member.plan_mode_required {
        cmd.push_str(" --plan-mode-required");
    }

    cmd
}

/// Sanitize agent name for team file (no @ in agent IDs).
pub fn sanitize_team_name(name: &str) -> String {
    name.chars()
        .filter(|c| c.is_alphanumeric() || *c == '-' || *c == '_')
        .collect()
}

/// Generate unique team member name (handle collisions).
pub async fn generate_unique_team_member_name(
    base_name: &str,
    team_file: &Option<TeamFile>,
) -> String {
    let lower_base = base_name.to_lowercase();

    let existing_names: Vec<String> = team_file
        .as_ref()
        .map(|tf| tf.members.iter().map(|m| m.name.to_lowercase()).collect())
        .unwrap_or_default();

    if !existing_names.iter().any(|n| n == &lower_base) {
        return base_name.to_string();
    }

    let mut suffix = 2;
    loop {
        let candidate = format!("{base_name}-{suffix}");
        if !existing_names.iter().any(|n| n == &candidate.to_lowercase()) {
            return candidate;
        }
        suffix += 1;
    }
}

/// Read team file from disk.
pub fn read_team_file(team_name: &str) -> anyhow::Result<Option<TeamFile>> {
    let path = format!(".claude/teams/{team_name}.json");
    if !std::path::Path::new(&path).exists() {
        return Ok(None);
    }

    let content = std::fs::read_to_string(&path)
        .map_err(|e| anyhow::anyhow!("Failed to read team file: {e}"))?;

    let team: TeamFile = serde_json::from_str(&content)
        .map_err(|e| anyhow::anyhow!("Failed to parse team file: {e}"))?;

    Ok(Some(team))
}

/// Write team file to disk.
pub fn write_team_file(team: &TeamFile) -> anyhow::Result<()> {
    let dir = ".claude/teams";
    std::fs::create_dir_all(dir)
        .map_err(|e| anyhow::anyhow!("Failed to create teams directory: {e}"))?;

    let path = format!("{dir}/{}.json", team.name);
    let content = serde_json::to_string_pretty(team)
        .map_err(|e| anyhow::anyhow!("Failed to serialize team file: {e}"))?;

    std::fs::write(&path, content)
        .map_err(|e| anyhow::anyhow!("Failed to write team file: {e}"))?;

    Ok(())
}

/// Write a mailbox message for inter-agent communication.
pub fn write_mailbox_message(
    team_name: &str,
    agent_name: &str,
    message: &MailboxMessage,
) -> anyhow::Result<()> {
    let dir = format!(".claude/mailbox/{team_name}");
    std::fs::create_dir_all(&dir)
        .map_err(|e| anyhow::anyhow!("Failed to create mailbox directory: {e}"))?;

    let path = format!("{dir}/{agent_name}.json");
    let content = serde_json::to_string_pretty(message)
        .map_err(|e| anyhow::anyhow!("Failed to serialize mailbox message: {e}"))?;

    std::fs::write(&path, content)
        .map_err(|e| anyhow::anyhow!("Failed to write mailbox message: {e}"))?;

    Ok(())
}

/// Read mailbox message for an agent.
pub fn read_mailbox_message(
    team_name: &str,
    agent_name: &str,
) -> anyhow::Result<Option<MailboxMessage>> {
    let path = format!(".claude/mailbox/{team_name}/{agent_name}.json");
    if !std::path::Path::new(&path).exists() {
        return Ok(None);
    }

    let content = std::fs::read_to_string(&path)
        .map_err(|e| anyhow::anyhow!("Failed to read mailbox: {e}"))?;

    let message: MailboxMessage = serde_json::from_str(&content)
        .map_err(|e| anyhow::anyhow!("Failed to parse mailbox message: {e}"))?;

    Ok(Some(message))
}

/// Build CLI args for permission mode propagation.
fn build_permission_mode_args(permission_mode: &str) -> Vec<String> {
    match permission_mode {
        "acceptEdits" => vec!["--permission-mode".to_string(), "acceptEdits".to_string()],
        "auto" => vec!["--permission-mode".to_string(), "auto".to_string()],
        "dangerously-skip-permissions" => vec!["--dangerously-skip-permissions".to_string()],
        _ => Vec::new(),
    }
}

/// Resolve model for teammate spawning.
/// 'inherit' → parent's model, undefined → default, explicit → use specified.
fn resolve_teammate_model(
    agent_model: Option<&str>,
    parent_model: &str,
) -> String {
    match agent_model {
        Some("inherit") | None => parent_model.to_string(),
        Some(model) => model.to_string(),
    }
}

/// Register an out-of-process teammate as a background task.
pub fn register_out_of_process_teammate(
    agent_id: &str,
    name: &str,
    prompt: &str,
    pane_id: &str,
    backend: &SpawnBackend,
) -> serde_json::Value {
    serde_json::json!({
        "taskId": format!("teammate-{agent_id}"),
        "agentId": agent_id,
        "name": name,
        "prompt": prompt,
        "paneId": pane_id,
        "backend": match backend {
            SpawnBackend::Tmux => "tmux",
            SpawnBackend::Iterm2 => "iterm2",
            SpawnBackend::InProcess => "in-process",
        },
        "status": "running",
    })
}

/// Kill a teammate pane.
pub async fn kill_teammate_pane(pane_id: &str, backend: &SpawnBackend) -> anyhow::Result<()> {
    match backend {
        SpawnBackend::Tmux => kill_tmux_pane(pane_id).await,
        SpawnBackend::Iterm2 => {
            // it2 session close would go here
            warn!("iTerm2 backend not fully implemented");
            Ok(())
        }
        SpawnBackend::InProcess => {
            // In-process agents are killed via abort channel
            warn!("In-process teammate kill not implemented");
            Ok(())
        }
    }
}

// =========================================================================
// Handoff Classification + Agent Memory (Phase 10.3g)
// =========================================================================

/// Result of handoff quality assessment.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HandoffAssessment {
    /// Whether the agent result is complete.
    pub is_complete: bool,
    /// Quality score (0.0 - 1.0).
    pub quality_score: f64,
    /// Warning message if quality is poor.
    pub warning: Option<String>,
    /// Suggested follow-up if incomplete.
    pub suggested_follow_up: Option<String>,
}

/// Assess handoff quality of an agent's transcript.
/// Uses secondary model call for classification.
pub fn assess_handoff_quality(
    transcript: &str,
    original_prompt: &str,
    tool_use_count: usize,
) -> HandoffAssessment {
    // Simple heuristic-based assessment (full impl would use secondary model call)

    let is_empty = transcript.trim().is_empty();
    let has_error = transcript.contains("Error:") || transcript.contains("error:");
    let is_truncated = transcript.ends_with("...");
    let has_tool_use = tool_use_count > 0;

    // Calculate quality score
    let mut quality_score: f64 = 1.0;

    if is_empty {
        quality_score -= 0.5;
    }
    if has_error {
        quality_score -= 0.3;
    }
    if is_truncated {
        quality_score -= 0.2;
    }
    if !has_tool_use && !is_empty {
        // No tools used but produced output — might be just thinking
        quality_score -= 0.1;
    }

    quality_score = quality_score.max(0.0_f64);

    // Generate warning if quality is poor
    let warning = if quality_score < 0.5 {
        Some(format!(
            "Agent result quality is low (score: {quality_score:.2}). \
            The agent may not have completed the task fully. \
            Consider reviewing the transcript or asking the agent to continue."
        ))
    } else {
        None
    };

    // Suggest follow-up if incomplete
    let suggested_follow_up = if is_empty || is_truncated {
        Some("Continue with the task where you left off. Provide the complete result.".to_string())
    } else {
        None
    };

    HandoffAssessment {
        is_complete: !is_empty && !is_truncated && !has_error,
        quality_score,
        warning,
        suggested_follow_up,
    }
}

/// Agent memory snapshot loaded from scope-based files.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentMemorySnapshot {
    /// Memory content loaded into agent's system prompt.
    pub content: String,
    /// Scope of the memory (project, user, team).
    pub scope: String,
    /// Source file path.
    pub source_path: String,
    /// Last modified timestamp.
    pub last_modified: chrono::DateTime<chrono::Utc>,
}

/// Load agent memory snapshot for a given agent type.
/// Scopes: project (.claude/memory/), user (~/.claude/memory/), team (.claude/teams/).
pub fn get_agent_memory_snapshot(agent_type: &str) -> Option<AgentMemorySnapshot> {
    // Try project scope first
    let project_path = format!(".claude/memory/{agent_type}.md");
    if let Ok(snapshot) = load_memory_file(&project_path, "project") {
        return Some(snapshot);
    }

    // Try user scope
    if let Some(config_dir) = dirs::config_dir() {
        let user_path = config_dir.join("claude").join("memory").join(format!("{agent_type}.md"));
        if let Ok(snapshot) = load_memory_file(user_path.to_str()?, "user") {
            return Some(snapshot);
        }
    }

    // Try team scope
    let team_path = format!(".claude/teams/memory-{agent_type}.md");
    if let Ok(snapshot) = load_memory_file(&team_path, "team") {
        return Some(snapshot);
    }

    None
}

/// Load a memory file into a snapshot.
fn load_memory_file(path: &str, scope: &str) -> anyhow::Result<AgentMemorySnapshot> {
    let metadata = std::fs::metadata(path)
        .map_err(|e| anyhow::anyhow!("Memory file not found: {e}"))?;

    let last_modified = metadata
        .modified()
        .map(|t| t.into())
        .unwrap_or_else(|_| chrono::Utc::now());

    let content = std::fs::read_to_string(path)
        .map_err(|e| anyhow::anyhow!("Failed to read memory file: {e}"))?;

    Ok(AgentMemorySnapshot {
        content,
        scope: scope.to_string(),
        source_path: path.to_string(),
        last_modified,
    })
}

/// Agent progress summary for display.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentProgressSummary {
    pub agent_id: String,
    pub agent_type: String,
    pub description: String,
    pub status: String,
    pub tool_uses: usize,
    pub tokens: u64,
    pub duration_ms: u64,
    pub activity_description: String,
}

/// Format agent progress summary for notification display.
pub fn format_progress_notification(summary: &AgentProgressSummary) -> String {
    let status_emoji = match summary.status.as_str() {
        "completed" => "✓",
        "error" => "✗",
        "aborted" => "⊘",
        _ => "⟳",
    };

    format!(
        "{status_emoji} {agent_type}: {description}\n\
        Tools: {tool_uses} | Tokens: {tokens} | Duration: {duration:.1}s\n\
        {activity}",
        status_emoji = status_emoji,
        agent_type = summary.agent_type,
        description = summary.description,
        tool_uses = summary.tool_uses,
        tokens = summary.tokens,
        duration = summary.duration_ms as f64 / 1000.0,
        activity = summary.activity_description,
    )
}

/// Check if SDK agent progress summaries are enabled.
/// In full impl, this would check GrowthBook gate.
pub fn get_sdk_agent_progress_summaries_enabled() -> bool {
    // Check env var as fallback for GrowthBook gate
    std::env::var("CLAUDE_CODE_AGENT_PROGRESS_SUMMARIES")
        .ok()
        .map(|v| v == "1" || v == "true")
        .unwrap_or(false)
}

// =========================================================================
// Coordinator Mode + Proactive + Remote (Phase 10.3h)
// =========================================================================

/// Check if coordinator mode is enabled.
/// Controlled by CLAUDE_CODE_COORDINATOR_MODE env var.
pub fn is_coordinator_mode() -> bool {
    std::env::var("CLAUDE_CODE_COORDINATOR_MODE")
        .ok()
        .map(|v| v == "1" || v == "true" || v == "coordinator")
        .unwrap_or(false)
}

/// Check if proactive mode is active.
pub fn is_proactive_active() -> bool {
    std::env::var("CLAUDE_CODE_PROACTIVE_MODE")
        .ok()
        .map(|v| v == "1" || v == "true" || v == "active")
        .unwrap_or(false)
}

/// Determine if all agents should be forced to async mode.
/// Coordinator mode and proactive mode both force async.
pub fn should_force_async_agents() -> bool {
    is_coordinator_mode() || is_proactive_active()
}

/// Remote agent launch result.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RemoteAgentResult {
    pub status: String,
    pub task_id: String,
    pub session_url: Option<String>,
    pub output_file: String,
}

/// Check if remote agent features are available (ant-only).
/// External builds should have this disabled via feature flags.
pub fn is_remote_agent_available() -> bool {
    // In a full impl, this would check cfg!(feature = "ant")
    // For now, check env var
    std::env::var("CLAUDE_CODE_REMOTE_AGENTS")
        .ok()
        .map(|v| v == "1" || v == "true")
        .unwrap_or(false)
}

/// Teleport agent to remote CCR session (ant-only).
/// Stub for external builds — returns error if remote not available.
pub async fn teleport_to_remote(
    agent_id: &str,
    agent_type: &str,
    prompt: &str,
    model: &str,
) -> anyhow::Result<RemoteAgentResult> {
    if !is_remote_agent_available() {
        return Err(anyhow::anyhow!(
            "Remote agent isolation is not available in this build. \
            This feature requires Anthropic internal access."
        ));
    }

    // In a full impl, this would:
    // 1. Create a remote CCR session
    // 2. Send agent config to remote
    // 3. Return session URL and task ID

    let task_id = format!("remote-{agent_id}");
    let session_url = format!("https://remote.claude.com/sessions/{task_id}");

    info!(
        agent_id = %agent_id,
        task_id = %task_id,
        session_url = %session_url,
        "Agent teleported to remote"
    );

    Ok(RemoteAgentResult {
        status: "remote_launched".to_string(),
        task_id,
        session_url: Some(session_url),
        output_file: format!(".claude/agents/{agent_id}/transcript.json"),
    })
}

/// Check if agent is eligible for remote execution.
/// Only certain agent types and models are eligible.
pub fn check_remote_agent_eligibility(
    agent_type: &str,
    model: &str,
) -> anyhow::Result<()> {
    if !is_remote_agent_available() {
        return Err(anyhow::anyhow!("Remote agents not available"));
    }

    // Only certain agent types are eligible for remote
    let eligible_types = ["general-purpose", "code-reviewer", "test-runner"];
    if !eligible_types.contains(&agent_type) {
        return Err(anyhow::anyhow!(
            "Agent type '{agent_type}' is not eligible for remote execution"
        ));
    }

    // Only certain models are supported for remote
    let eligible_models = ["sonnet", "opus"];
    if !eligible_models.contains(&model) {
        return Err(anyhow::anyhow!(
            "Model '{model}' is not supported for remote execution"
        ));
    }

    Ok(())
}

/// Register a remote agent task with the task tracker.
pub fn register_remote_agent_task(
    task_id: &str,
    agent_id: &str,
    description: &str,
    session_url: &str,
) -> serde_json::Value {
    serde_json::json!({
        "taskId": task_id,
        "agentId": agent_id,
        "description": description,
        "sessionUrl": session_url,
        "status": "remote_running",
        "type": "remote",
    })
}

/// Get URL to view a remote task session.
pub fn get_remote_task_session_url(task_id: &str) -> Option<String> {
    if !is_remote_agent_available() {
        return None;
    }
    Some(format!("https://remote.claude.com/tasks/{task_id}"))
}

/// Feature gate: Check if background tasks are disabled.
pub fn is_background_tasks_disabled() -> bool {
    std::env::var("CLAUDE_CODE_DISABLE_BACKGROUND_TASKS")
        .ok()
        .map(|v| v == "1" || v == "true")
        .unwrap_or(false)
}

/// Feature gate: Get auto-background timeout (120,000ms if enabled).
pub fn get_auto_background_ms() -> Option<u64> {
    let enabled = std::env::var("CLAUDE_AUTO_BACKGROUND_TASKS")
        .ok()
        .map(|v| v == "1" || v == "true")
        .unwrap_or(false);

    if enabled {
        Some(120_000) // 2 minutes
    } else {
        None
    }
}

/// Feature gate: Check if agent list should be shown as attachment vs inline.
pub fn is_agent_list_as_attachment() -> bool {
    std::env::var("CLAUDE_CODE_AGENT_LIST_IN_MESSAGES")
        .ok()
        .map(|v| v == "1" || v == "true" || v == "attachment")
        .unwrap_or(false)
}

/// Build coordinator-specific system prompt (slim version).
pub fn build_coordinator_system_prompt(base_prompt: &str) -> String {
    format!(
        "{base_prompt}\n\n\
        === COORDINATOR MODE ===\n\
        You are running in coordinator mode. Your role is to orchestrate multiple \
        agents to complete complex tasks. Delegate work to specialized agents using \
        the Agent tool with run_in_background=true. Monitor progress and coordinate \
        the results.\n\
        ========================"
    )
}

/// Determine if agent should run in async mode based on context.
/// Considers: explicit flag, agent definition, coordinator mode, proactive mode.
pub fn should_run_agent_async(
    run_in_background: bool,
    agent_background: bool,
) -> bool {
    // Explicit flag or agent definition background flag
    if run_in_background || agent_background {
        return true;
    }

    // Coordinator mode and proactive mode force all agents async
    if should_force_async_agents() {
        return true;
    }

    false
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
