use std::sync::Arc;
use std::time::Instant;

use async_trait::async_trait;
use cc_core::messages::ContentBlockParam;
use cc_core::permissions::PermissionResult;
use cc_core::tools::{
    InterruptBehavior, SearchOrReadInfo, Tool, ToolProgress, ToolPromptOptions, ToolResult,
    ToolUseContext,
};
use cc_core::types::ValidationResult;

use crate::utils::check_read_permission;

/// Blocked device paths for security.
const BLOCKED_DEVICE_PATHS: &[&str] = &[
    "/dev/zero",
    "/dev/random",
    "/dev/urandom",
    "/dev/full",
    "/dev/stdin",
    "/dev/tty",
    "/dev/console",
    "/dev/stdout",
    "/dev/stderr",
];

/// Default max results for glob.
const DEFAULT_MAX_RESULTS: usize = 100;

/// Glob tool - file pattern matching.
#[derive(Debug)]
pub struct GlobTool;

impl GlobTool {
    pub fn new() -> Arc<dyn Tool> {
        Arc::new(Self)
    }

    fn execute_glob(&self, pattern: &str, path: Option<&str>) -> Result<Vec<String>, String> {
        let search_dir = path.unwrap_or(".");

        // Use walkdir for glob matching
        let mut results = Vec::new();
        let pattern_lower = pattern.to_lowercase();

        for entry in walkdir::WalkDir::new(search_dir)
            .follow_links(false)
            .into_iter()
            .filter_map(|e| e.ok())
        {
            let path = entry.path();
            if let Some(name) = path.file_name().and_then(|n| n.to_str()) {
                if self.matches_glob(name, &pattern_lower) {
                    if let Some(p) = path.to_str() {
                        results.push(p.to_string());
                    }
                }
            }
        }

        // Sort by modification time (newest first)
        results.sort_by(|a, b| {
            let ma = std::fs::metadata(a).and_then(|m| m.modified()).ok();
            let mb = std::fs::metadata(b).and_then(|m| m.modified()).ok();
            mb.cmp(&ma)
        });

        Ok(results)
    }

    fn matches_glob(&self, name: &str, pattern: &str) -> bool {
        // Simple glob matching: * matches any chars, ? matches single char
        let pattern = pattern.replace("**/", ""); // Flatten **/
        let pattern = pattern.replace("*/", ""); // Flatten */
        let pattern = pattern.replace("*", ".*").replace("?", ".");
        let re = format!("^{}$", pattern);
        regex::Regex::new(&re)
            .map(|r| r.is_match(name))
            .unwrap_or(false)
    }
}

#[async_trait]
impl Tool for GlobTool {
    fn name(&self) -> &str {
        "Glob"
    }

    fn search_hint(&self) -> Option<&str> {
        Some("Find files by name patterns")
    }

    async fn call(
        &self,
        input: serde_json::Value,
        _context: &ToolUseContext,
        _on_progress: Option<Arc<dyn Fn(ToolProgress) + Send + Sync>>,
    ) -> anyhow::Result<ToolResult<serde_json::Value>> {
        let pattern = input["pattern"]
            .as_str()
            .ok_or_else(|| anyhow::anyhow!("pattern is required"))?;
        let path = input["path"].as_str();

        let start = Instant::now();
        let max_results = DEFAULT_MAX_RESULTS;

        let all_files = self.execute_glob(pattern, path).map_err(|e| anyhow::anyhow!(e))?;
        let truncated = all_files.len() > max_results;
        let files: Vec<_> = all_files.into_iter().take(max_results).collect();
        let duration_ms = start.elapsed().as_millis() as u64;

        let num_files = files.len();
        let filenames: Vec<serde_json::Value> =
            files.into_iter().map(serde_json::Value::String).collect();

        let mut result = serde_json::json!({
            "durationMs": duration_ms,
            "numFiles": num_files,
            "filenames": filenames,
            "truncated": truncated,
        });

        let output = if num_files == 0 {
            "No files found".to_string()
        } else {
            let mut out = filenames
                .iter()
                .filter_map(|v| v.as_str())
                .collect::<Vec<_>>()
                .join("\n");
            if truncated {
                out.push_str(
                    "\n\n(Results are truncated. Consider using a more specific path or pattern.)",
                );
            }
            out
        };

        result["output"] = serde_json::Value::String(output);

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
        Ok("Fast file pattern matching tool that works with any codebase size".to_string())
    }

    fn input_schema(&self) -> serde_json::Value {
        serde_json::json!({
            "type": "object",
            "properties": {
                "pattern": {
                    "type": "string",
                    "description": "The glob pattern to match files against"
                },
                "path": {
                    "type": "string",
                    "description": "Directory to search in (defaults to cwd)"
                }
            },
            "required": ["pattern"]
        })
    }

    fn is_read_only(&self, _input: &serde_json::Value) -> bool {
        true
    }

    fn is_concurrency_safe(&self, _input: &serde_json::Value) -> bool {
        true
    }

    fn interrupt_behavior(&self) -> InterruptBehavior {
        InterruptBehavior::Cancel
    }

    fn is_search_or_read_command(&self, _input: &serde_json::Value) -> SearchOrReadInfo {
        SearchOrReadInfo {
            is_search: true,
            is_read: false,
            is_list: true,
        }
    }

    async fn validate_input(
        &self,
        input: &serde_json::Value,
        _context: &ToolUseContext,
    ) -> anyhow::Result<ValidationResult> {
        if input["pattern"].as_str().is_none() {
            return Ok(ValidationResult::Invalid {
                message: "pattern is required".to_string(),
                error_code: 0,
            });
        }
        if let Some(path) = input["path"].as_str() {
            for blocked in BLOCKED_DEVICE_PATHS {
                if path.starts_with(blocked) {
                    return Ok(ValidationResult::Invalid {
                        message: format!("Access to {} is not allowed", blocked),
                        error_code: 0,
                    });
                }
            }
        }
        Ok(ValidationResult::Valid)
    }

    async fn check_permissions(
        &self,
        input: &serde_json::Value,
        context: &ToolUseContext,
    ) -> anyhow::Result<PermissionResult> {
        check_read_permission(input, context, "glob")
    }

    fn max_result_size_chars(&self) -> usize {
        50_000
    }

    async fn prompt(&self, _options: &ToolPromptOptions) -> anyhow::Result<String> {
        Ok(
            "- Fast file pattern matching tool that works with any codebase size\n\
            - Supports glob patterns like \"**/*.js\" or \"src/**/*.ts\"\n\
            - Returns matching file paths sorted by modification time\n\
            - Use this tool when you need to find files by name patterns\n\
            - When you are doing an open ended search that may require multiple rounds of globbing and grepping, use the Agent tool instead"
                .to_string(),
        )
    }

    fn user_facing_name(&self, _input: Option<&serde_json::Value>) -> String {
        "Glob".to_string()
    }

    fn get_activity_description(&self, _input: Option<&serde_json::Value>) -> Option<String> {
        Some("Searching for files".to_string())
    }

    fn map_tool_result_to_block(
        &self,
        content: serde_json::Value,
        tool_use_id: &str,
    ) -> ContentBlockParam {
        let output = content["output"]
            .as_str()
            .unwrap_or("No results")
            .to_string();
        ContentBlockParam::ToolResult {
            tool_use_id: tool_use_id.to_string(),
            content: vec![cc_core::messages::ToolResultContent::Text { text: output }],
            is_error: None,
        }
    }
}
