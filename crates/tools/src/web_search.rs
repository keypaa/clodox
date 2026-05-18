use std::sync::Arc;

use async_trait::async_trait;
use cc_core::messages::ContentBlockParam;
use cc_core::permissions::PermissionResult;
use cc_core::tools::{
    InterruptBehavior, SearchOrReadInfo, Tool, ToolProgress, ToolPromptOptions, ToolResult,
    ToolUseContext,
};
use cc_core::types::ValidationResult;
use serde::{Deserialize, Serialize};
use url::Url;

use crate::utils::check_read_permission;

/// Current month/year for search query accuracy.
fn current_month_year() -> String {
    let now = chrono::Utc::now();
    now.format("%B %Y").to_string()
}

/// WebSearch tool name.
pub const WEB_SEARCH_TOOL_NAME: &str = "WebSearch";

/// A single search result hit.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SearchResult {
    pub title: String,
    pub url: String,
}

/// WebSearch tool - searches the web for current information.
/// Uses Exa MCP server for non-Anthropic models.
#[derive(Debug)]
pub struct WebSearchTool;

impl WebSearchTool {
    pub fn new() -> Arc<dyn Tool> {
        Arc::new(Self)
    }

    /// Build the tool prompt.
    fn get_prompt() -> String {
        let current = current_month_year();
        format!(
            r#"- Allows Claude to search the web and use the results to inform responses
- Provides up-to-date information for current events and recent data
- Returns search result information formatted as search result blocks, including links as markdown hyperlinks
- Use this tool for accessing information beyond Claude's knowledge cutoff
- Searches are performed automatically within a single API call

CRITICAL REQUIREMENT - You MUST follow this:
  - After answering the user's question, you MUST include a "Sources:" section at the end of your response
  - In the Sources section, list all relevant URLs from the search results as markdown hyperlinks: [Title](URL)
  - This is MANDATORY - never skip including sources in your response
  - Example format:

    [Your answer here]

    Sources:
    - [Source Title 1](https://example.com/1)
    - [Source Title 2](https://example.com/2)

Usage notes:
  - Domain filtering is supported to include or block specific websites
  - Web search is only available in the US

IMPORTANT - Use the correct year in search queries:
  - The current month is {current}. You MUST use this year when searching for recent information, documentation, or current events.
  - Example: If the user asks for "latest React docs", search for "React documentation" with the current year, NOT last year"#
        )
    }
}

#[async_trait]
impl Tool for WebSearchTool {
    fn name(&self) -> &str {
        WEB_SEARCH_TOOL_NAME
    }

    fn search_hint(&self) -> Option<&str> {
        Some("search the web for current information")
    }

    async fn call(
        &self,
        input: serde_json::Value,
        context: &ToolUseContext,
        _on_progress: Option<Arc<dyn Fn(ToolProgress) + Send + Sync>>,
    ) -> anyhow::Result<ToolResult<serde_json::Value>> {
        let query = input["query"]
            .as_str()
            .ok_or_else(|| anyhow::anyhow!("query is required"))?;

        let allowed_domains = input["allowed_domains"]
            .as_array()
            .map(|arr| arr.iter().filter_map(|v| v.as_str().map(String::from)).collect::<Vec<_>>());

        let blocked_domains = input["blocked_domains"]
            .as_array()
            .map(|arr| arr.iter().filter_map(|v| v.as_str().map(String::from)).collect::<Vec<_>>());

        let start = std::time::Instant::now();

        // Try Exa MCP server first via the McpToolCaller trait
        if let Some(ref mcp_service) = context.options.mcp_service {
            // Check if Exa MCP is connected by looking for the tool
            let tools = mcp_service.get_remote_tools("exa").await;
            let has_exa = tools.iter().any(|(_, name, _, _)| {
                name == "web_search_advanced_exa" || name == "web_search_exa"
            });

            if has_exa {
                let tool_name = if tools.iter().any(|(_, name, _, _)| name == "web_search_advanced_exa") {
                    "web_search_advanced_exa"
                } else {
                    "web_search_exa"
                };

                let mut args = serde_json::json!({
                    "query": query,
                    "numResults": 5,
                    "text": true,
                });

                if let Some(ref domains) = allowed_domains {
                    args["allowed_domains"] = serde_json::Value::Array(
                        domains.iter().map(|d| serde_json::Value::String(d.clone())).collect()
                    );
                }
                if let Some(ref domains) = blocked_domains {
                    args["blocked_domains"] = serde_json::Value::Array(
                        domains.iter().map(|d| serde_json::Value::String(d.clone())).collect()
                    );
                }

                match mcp_service.call_mcp_tool("exa", tool_name, args).await {
                    Ok(result) => {
                        let duration_seconds = start.elapsed().as_secs_f64();
                        let results = parse_exa_response(&result, query);

                        let output = serde_json::json!({
                            "query": query,
                            "results": results,
                            "durationSeconds": duration_seconds,
                        });

                        return Ok(ToolResult {
                            data: output,
                            new_messages: None,
                            mcp_meta: None,
                        });
                    }
                    Err(e) => {
                        tracing::warn!(error = %e, "Exa MCP search failed");
                    }
                }
            }
        }

        // Fallback: return error suggesting setup
        let exa_url = "https://mcp.exa.ai/mcp";
        let output = serde_json::json!({
            "query": query,
            "results": [format!("Web search is not available. Configure the Exa MCP server at {exa_url} for model-agnostic search, or use an Anthropic model with native web search support.")],
            "durationSeconds": start.elapsed().as_secs_f64(),
        });

        Ok(ToolResult {
            data: output,
            new_messages: None,
            mcp_meta: None,
        })
    }

    async fn description(
        &self,
        input: serde_json::Value,
        _options: &cc_core::tools::DescriptionOptions,
    ) -> anyhow::Result<String> {
        let query = input["query"].as_str().unwrap_or("unknown");
        Ok(format!("Claude wants to search the web for: {query}"))
    }

    fn input_schema(&self) -> serde_json::Value {
        serde_json::json!({
            "type": "object",
            "properties": {
                "query": {
                    "type": "string",
                    "description": "The search query to use"
                },
                "allowed_domains": {
                    "type": "array",
                    "items": { "type": "string" },
                    "description": "Only include search results from these domains"
                },
                "blocked_domains": {
                    "type": "array",
                    "items": { "type": "string" },
                    "description": "Never include search results from these domains"
                }
            },
            "required": ["query"]
        })
    }

    fn is_concurrency_safe(&self, _input: &serde_json::Value) -> bool {
        true
    }

    fn is_read_only(&self, _input: &serde_json::Value) -> bool {
        true
    }

    fn interrupt_behavior(&self) -> InterruptBehavior {
        InterruptBehavior::Cancel
    }

    fn is_search_or_read_command(&self, _input: &serde_json::Value) -> SearchOrReadInfo {
        SearchOrReadInfo {
            is_search: true,
            is_read: false,
            is_list: false,
        }
    }

    fn is_open_world(&self, _input: &serde_json::Value) -> bool {
        true
    }

    fn should_defer(&self) -> bool {
        true
    }

    async fn validate_input(
        &self,
        input: &serde_json::Value,
        _context: &ToolUseContext,
    ) -> anyhow::Result<ValidationResult> {
        let query = input["query"].as_str();
        match query {
            None => Ok(ValidationResult::Invalid {
                message: "Error: Missing query".to_string(),
                error_code: 1,
            }),
            Some(q) if q.is_empty() => Ok(ValidationResult::Invalid {
                message: "Error: Missing query".to_string(),
                error_code: 1,
            }),
            _ => {
                let allowed = input["allowed_domains"].as_array();
                let blocked = input["blocked_domains"].as_array();
                if allowed.is_some() && blocked.is_some() {
                    return Ok(ValidationResult::Invalid {
                        message: "Error: Cannot specify both allowed_domains and blocked_domains in the same request".to_string(),
                        error_code: 2,
                    });
                }
                Ok(ValidationResult::Valid)
            }
        }
    }

    async fn check_permissions(
        &self,
        input: &serde_json::Value,
        context: &ToolUseContext,
    ) -> anyhow::Result<PermissionResult> {
        check_read_permission(input, context, "web_search")
    }

    fn max_result_size_chars(&self) -> usize {
        100_000
    }

    async fn prompt(&self, _options: &ToolPromptOptions) -> anyhow::Result<String> {
        Ok(Self::get_prompt())
    }

    fn user_facing_name(&self, _input: Option<&serde_json::Value>) -> String {
        "Web Search".to_string()
    }

    fn get_activity_description(&self, input: Option<&serde_json::Value>) -> Option<String> {
        if let Some(input) = input {
            if let Some(query) = input["query"].as_str() {
                return Some(format!("Searching for {query}"));
            }
        }
        Some("Searching the web".to_string())
    }

    fn get_tool_use_summary(&self, input: Option<&serde_json::Value>) -> Option<String> {
        if let Some(input) = input {
            return input["query"].as_str().map(String::from);
        }
        None
    }

    fn map_tool_result_to_block(
        &self,
        content: serde_json::Value,
        tool_use_id: &str,
    ) -> ContentBlockParam {
        let query = content["query"].as_str().unwrap_or("").to_string();
        let results = content["results"].as_array();

        let mut formatted = format!("Web search results for query: \"{query}\"\n\n");

        if let Some(results) = results {
            for result in results {
                if let Some(s) = result.as_str() {
                    formatted.push_str(s);
                    formatted.push_str("\n\n");
                } else if let Some(obj) = result.as_object() {
                    if let Some(links) = obj.get("links") {
                        formatted.push_str(&format!("Links: {links}\n\n"));
                    } else {
                        formatted.push_str("No links found.\n\n");
                    }
                }
            }
        }

        formatted.push_str("\nREMINDER: You MUST include the sources above in your response to the user using markdown hyperlinks.");

        ContentBlockParam::ToolResult {
            tool_use_id: tool_use_id.to_string(),
            content: vec![cc_core::messages::ToolResultContent::Text { text: formatted.trim().to_string() }],
            is_error: None,
        }
    }
}

/// Parse Exa MCP response into search results.
fn parse_exa_response(response: &serde_json::Value, query: &str) -> Vec<serde_json::Value> {
    let mut results = Vec::new();

    // Exa response format: { "results": [{ "title": "...", "url": "...", "text": "..." }, ...] }
    if let Some(exa_results) = response["results"].as_array() {
        for item in exa_results {
            let title = item["title"].as_str().unwrap_or("Unknown").to_string();
            let url = item["url"].as_str().unwrap_or("").to_string();

            // Validate URL
            if Url::parse(&url).is_ok() {
                results.push(serde_json::json!({
                    "title": title,
                    "url": url,
                }));
            }
        }
    }

    // If no results, return the raw content as a string
    if results.is_empty() {
        if let Some(content) = response["content"].as_str() {
            results.push(serde_json::Value::String(content.to_string()));
        } else {
            results.push(serde_json::Value::String(format!("No results found for query: \"{query}\"")));
        }
    }

    results
}
