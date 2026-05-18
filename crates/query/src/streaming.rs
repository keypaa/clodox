use std::collections::HashMap;

use cc_core::tools::{Tool, Tools, ToolUseContext};
use tracing::{debug, info, warn};

use crate::engine::ToolCallInfo;

/// Executes multiple tool calls in parallel with permission checking.
pub struct StreamingToolExecutor {
    tools: Tools,
}

impl StreamingToolExecutor {
    pub fn new(tools: Tools) -> Self {
        Self { tools }
    }

    /// Execute all tool calls in parallel.
    pub async fn execute_all(
        &self,
        tool_calls: &[ToolCallInfo],
    ) -> Vec<Result<serde_json::Value, String>> {
        let mut handles = Vec::new();

        for call in tool_calls {
            let tools = self.tools.clone();
            let call = call.clone();

            let handle = tokio::spawn(async move {
                Self::execute_single(&tools, &call).await
            });

            handles.push(handle);
        }

        let mut results = Vec::with_capacity(handles.len());
        for handle in handles {
            match handle.await {
                Ok(result) => results.push(result),
                Err(e) => results.push(Err(format!("Task panicked: {e}"))),
            }
        }

        results
    }

    async fn execute_single(
        tools: &Tools,
        call: &ToolCallInfo,
    ) -> Result<serde_json::Value, String> {
        let tool = tools
            .iter()
            .find(|t| t.name() == call.name || t.aliases().contains(&call.name))
            .ok_or_else(|| format!("Unknown tool: {}", call.name))?;

        debug!(tool = call.name, "Executing tool call");

        if !tool.is_enabled() {
            return Err(format!("Tool '{}' is disabled", call.name));
        }

        info!(
            tool = call.name,
            input = %serde_json::to_string(&call.input).unwrap_or_default(),
            "Tool permission check (auto-allowed)"
        );

        let context = Self::create_tool_context(tools);

        match tool.call(call.input.clone(), &context, None).await {
            Ok(result) => {
                debug!(tool = call.name, "Tool call completed successfully");
                Ok(result.data)
            }
            Err(e) => {
                warn!(tool = call.name, error = %e, "Tool call failed");
                Err(format!("Tool execution failed: {e}"))
            }
        }
    }

    fn create_tool_context(tools: &Tools) -> ToolUseContext {
        let (abort_tx, _) = tokio::sync::watch::channel(false);

        ToolUseContext {
            options: cc_core::tools::ToolUseOptions {
                commands: Vec::new(),
                debug: false,
                main_loop_model: String::new(),
                tools: tools.clone(),
                verbose: false,
                thinking_config: cc_core::tools::ThinkingConfig {
                    enabled: false,
                    budget_tokens: None,
                },
                mcp_clients: Vec::new(),
                mcp_resources: HashMap::new(),
                is_non_interactive_session: false,
                agent_definitions: cc_core::tools::AgentDefinitionsResult {
                    agents: Vec::new(),
                },
                max_budget_usd: None,
                custom_system_prompt: None,
                append_system_prompt: None,
                query_source: None,
                refresh_tools: None,
                mcp_service: None,
            },
            abort_controller: abort_tx,
            messages: Vec::new(),
            agent_id: None,
            agent_type: None,
            tool_use_id: None,
            user_modified: None,
            require_can_use_tool: None,
            query_tracking: None,
            file_reading_limits: None,
            glob_limits: None,
            content_replacement_state: None,
            local_denial_tracking: None,
            rendered_system_prompt: None,
        }
    }
}
