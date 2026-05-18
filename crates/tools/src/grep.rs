use std::sync::Arc;

use async_trait::async_trait;
use cc_core::messages::ContentBlockParam;
use cc_core::permissions::PermissionResult;
use cc_core::tools::{
    InterruptBehavior, SearchOrReadInfo, Tool, ToolProgress, ToolPromptOptions, ToolResult,
    ToolUseContext,
};
use cc_core::types::ValidationResult;

use crate::utils::check_read_permission;

/// Default head limit for grep results.
const DEFAULT_HEAD_LIMIT: usize = 250;

/// Max column width for grep output.
const MAX_COLUMNS: usize = 500;

/// Output mode for grep.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum OutputMode {
    Content,
    FilesWithMatches,
    Count,
}

impl std::str::FromStr for OutputMode {
    type Err = String;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "content" => Ok(OutputMode::Content),
            "files_with_matches" => Ok(OutputMode::FilesWithMatches),
            "count" => Ok(OutputMode::Count),
            _ => Err(format!("Unknown output mode: {}", s)),
        }
    }
}

/// Grep tool - content search using ripgrep.
#[derive(Debug)]
pub struct GrepTool;

impl GrepTool {
    pub fn new() -> Arc<dyn Tool> {
        Arc::new(Self)
    }

    fn execute_grep(
        &self,
        pattern: &str,
        path: Option<&str>,
        glob: Option<&str>,
        output_mode: OutputMode,
        case_insensitive: bool,
        context: Option<usize>,
        before: Option<usize>,
        after: Option<usize>,
        show_line_numbers: bool,
        multiline: bool,
        head_limit: usize,
        offset: usize,
        file_type: Option<&str>,
    ) -> Result<serde_json::Value, String> {
        let search_path = path.unwrap_or(".");

        let mut cmd = std::process::Command::new("rg");
        cmd.arg("--hidden")
            .arg("--max-columns")
            .arg(MAX_COLUMNS.to_string())
            .arg("--no-heading")
            .arg("--line-number");

        // VCS exclusions
        for vcs in &[".git", ".svn", ".hg", ".bzr", ".jj", ".sl"] {
            cmd.arg("--glob").arg(format!("!{}", vcs));
        }

        if case_insensitive {
            cmd.arg("-i");
        }
        if multiline {
            cmd.arg("-U").arg("--multiline-dotall");
        }
        if !show_line_numbers {
            cmd.arg("-N");
        }

        match output_mode {
            OutputMode::Content => {
                if let Some(ctx) = context {
                    cmd.arg("-C").arg(ctx.to_string());
                }
                if let Some(b) = before {
                    cmd.arg("-B").arg(b.to_string());
                }
                if let Some(a) = after {
                    cmd.arg("-A").arg(a.to_string());
                }
            }
            OutputMode::FilesWithMatches => {
                cmd.arg("-l");
            }
            OutputMode::Count => {
                cmd.arg("--count");
            }
        }

        if let Some(g) = glob {
            for pat in g.split([',', ' ']).filter(|s| !s.is_empty()) {
                cmd.arg("--glob").arg(pat);
            }
        }

        if let Some(t) = file_type {
            cmd.arg("--type").arg(t);
        }

        // Pattern handling
        if pattern.starts_with('-') {
            cmd.arg("-e").arg(pattern);
        } else {
            cmd.arg(pattern);
        }

        cmd.arg(search_path);

        let output = cmd.output().map_err(|e| format!("Failed to run rg: {}", e))?;
        let stdout = String::from_utf8_lossy(&output.stdout);

        let lines: Vec<&str> = stdout.lines().collect();
        let total_matches = lines.len();
        let applied_limit = head_limit > 0 && total_matches > head_limit;
        let sliced: Vec<&&str> = lines.iter().skip(offset).take(head_limit).collect();

        let mut result = serde_json::Map::new();

        match output_mode {
            OutputMode::Content => {
                let content = sliced.iter().map(|l| **l).collect::<Vec<_>>().join("\n");
                result.insert("mode".into(), "content".into());
                result.insert("content".into(), content.into());
                result.insert("numLines".into(), (sliced.len() as u64).into());
            }
            OutputMode::FilesWithMatches => {
                let files: Vec<serde_json::Value> = sliced
                    .iter()
                    .map(|l| serde_json::Value::String(l.to_string()))
                    .collect();
                result.insert("mode".into(), "files_with_matches".into());
                result.insert("filenames".into(), files.into());
                result.insert("numFiles".into(), (sliced.len() as u64).into());
            }
            OutputMode::Count => {
                let total: u64 = sliced
                    .iter()
                    .filter_map(|l| l.split(':').last().and_then(|n| n.trim().parse::<u64>().ok()))
                    .sum();
                let content = sliced.iter().map(|l| **l).collect::<Vec<_>>().join("\n");
                result.insert("mode".into(), "count".into());
                result.insert("content".into(), content.into());
                result.insert("numMatches".into(), total.into());
            }
        }

        if applied_limit {
            result.insert("appliedLimit".into(), (head_limit as u64).into());
        }
        if offset > 0 {
            result.insert("appliedOffset".into(), (offset as u64).into());
        }

        Ok(serde_json::Value::Object(result))
    }
}

#[async_trait]
impl Tool for GrepTool {
    fn name(&self) -> &str {
        "Grep"
    }

    fn search_hint(&self) -> Option<&str> {
        Some("Search file contents with regex")
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
        let glob = input["glob"].as_str();
        let output_mode_str = input["output_mode"].as_str().unwrap_or("files_with_matches");
        let output_mode = output_mode_str
            .parse::<OutputMode>()
            .unwrap_or(OutputMode::FilesWithMatches);
        let case_insensitive = input["-i"].as_bool().unwrap_or(false);
        let context = input["context"].as_u64().map(|v| v as usize);
        let before = input["-B"].as_u64().map(|v| v as usize);
        let after = input["-A"].as_u64().map(|v| v as usize);
        let show_line_numbers = input["-n"].as_bool().unwrap_or(true);
        let multiline = input["multiline"].as_bool().unwrap_or(false);
        let head_limit = input["head_limit"].as_u64().map(|v| v as usize).unwrap_or(DEFAULT_HEAD_LIMIT);
        let offset = input["offset"].as_u64().map(|v| v as usize).unwrap_or(0);
        let file_type = input["type"].as_str();

        let result = self
            .execute_grep(
                pattern,
                path,
                glob,
                output_mode,
                case_insensitive,
                context,
                before,
                after,
                show_line_numbers,
                multiline,
                head_limit,
                offset,
                file_type,
            )
            .map_err(|e| anyhow::anyhow!(e))?;

        // Format output for display
        let output = match result["mode"].as_str() {
            Some("content") => {
                let content = result["content"].as_str().unwrap_or("");
                if content.is_empty() {
                    "No matches found".to_string()
                } else if result.get("appliedLimit").is_some() {
                    format!(
                        "{}\n\n(Results truncated. Use offset to see more.)",
                        content
                    )
                } else {
                    content.to_string()
                }
            }
            Some("files_with_matches") => {
                let num = result["numFiles"].as_u64().unwrap_or(0);
                if num == 0 {
                    "No files found".to_string()
                } else {
                    let files = result["filenames"]
                        .as_array()
                        .map(|arr| {
                            arr.iter()
                                .filter_map(|v| v.as_str())
                                .collect::<Vec<_>>()
                                .join("\n")
                        })
                        .unwrap_or_default();
                    format!("Found {} files\n{}", num, files)
                }
            }
            Some("count") => {
                let content = result["content"].as_str().unwrap_or("");
                let total = result["numMatches"].as_u64().unwrap_or(0);
                if total == 0 {
                    "No matches found".to_string()
                } else {
                    format!(
                        "{}\n\nFound {} total occurrences",
                        content, total
                    )
                }
            }
            _ => "Unknown output mode".to_string(),
        };

        let mut result = result;
        if let Some(obj) = result.as_object_mut() {
            obj.insert("output".into(), output.into());
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
        Ok("A powerful search tool built on ripgrep".to_string())
    }

    fn input_schema(&self) -> serde_json::Value {
        serde_json::json!({
            "type": "object",
            "properties": {
                "pattern": {
                    "type": "string",
                    "description": "Regular expression pattern to search for"
                },
                "path": {
                    "type": "string",
                    "description": "File or directory to search in (defaults to cwd)"
                },
                "glob": {
                    "type": "string",
                    "description": "Glob pattern to filter files (e.g., \"*.js\", \"*.{ts,tsx}\")"
                },
                "output_mode": {
                    "type": "string",
                    "enum": ["content", "files_with_matches", "count"],
                    "description": "Output mode (default: files_with_matches)"
                },
                "-B": {
                    "type": "number",
                    "description": "Lines before each match"
                },
                "-A": {
                    "type": "number",
                    "description": "Lines after each match"
                },
                "-C": {
                    "type": "number",
                    "description": "Lines before and after each match"
                },
                "context": {
                    "type": "number",
                    "description": "Lines before and after each match"
                },
                "-n": {
                    "type": "boolean",
                    "description": "Show line numbers (default: true)"
                },
                "-i": {
                    "type": "boolean",
                    "description": "Case insensitive search"
                },
                "type": {
                    "type": "string",
                    "description": "File type to search (js, py, rust, go, java, etc.)"
                },
                "head_limit": {
                    "type": "number",
                    "description": "Limit output (default: 250, 0 = unlimited)"
                },
                "offset": {
                    "type": "number",
                    "description": "Skip first N results (default: 0)"
                },
                "multiline": {
                    "type": "boolean",
                    "description": "Enable multiline mode"
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
            is_list: false,
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
        Ok(ValidationResult::Valid)
    }

    async fn check_permissions(
        &self,
        input: &serde_json::Value,
        context: &ToolUseContext,
    ) -> anyhow::Result<PermissionResult> {
        check_read_permission(input, context, "grep")
    }

    fn max_result_size_chars(&self) -> usize {
        100_000
    }

    async fn prompt(&self, _options: &ToolPromptOptions) -> anyhow::Result<String> {
        Ok(
            "A powerful search tool built on ripgrep\n\n\
            Usage:\n\
            - ALWAYS use Grep for search tasks. NEVER invoke grep or rg as a Bash command.\n\
            - Supports full regex syntax (e.g., \"log.*Error\", \"function\\\\s+\\\\w+\")\n\
            - Filter files with glob parameter or type parameter\n\
            - Output modes: \"content\" shows matching lines, \"files_with_matches\" shows only file paths (default), \"count\" shows match counts\n\
            - Pattern syntax: Uses ripgrep (not grep) - literal braces need escaping\n\
            - Multiline matching: By default patterns match within single lines only. For cross-line patterns, use multiline: true"
                .to_string(),
        )
    }

    fn user_facing_name(&self, _input: Option<&serde_json::Value>) -> String {
        "Search".to_string()
    }

    fn get_activity_description(&self, _input: Option<&serde_json::Value>) -> Option<String> {
        Some("Searching".to_string())
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
