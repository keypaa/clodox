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

/// Default timeout in milliseconds (120 seconds).
const DEFAULT_TIMEOUT_MS: u64 = 120_000;

/// Max timeout in milliseconds (600 seconds = 10 minutes).
const MAX_TIMEOUT_MS: u64 = 600_000;

/// Progress threshold in milliseconds.
const PROGRESS_THRESHOLD_MS: u64 = 2_000;

/// Bash tool - executes shell commands.
#[derive(Debug)]
pub struct BashTool;

impl BashTool {
    pub fn new() -> Arc<dyn Tool> {
        Arc::new(Self)
    }

    /// Check if a command is read-only (safe to auto-allow).
    pub fn is_read_only(input: &serde_json::Value) -> bool {
        let command = input["command"].as_str().unwrap_or("");
        let trimmed = command.trim();

        // Allowlist of safe read-only commands
        let safe_prefixes = [
            "ls ", "cat ", "head ", "tail ", "wc ", "diff ", "stat ", "file ",
            "find ", "grep ", "rg ", "tree ", "ps ", "ss ", "netstat ",
            "df ", "du ", "free ", "uptime ", "uname ", "whoami ", "pwd ",
            "which ", "whereis ", "readlink ", "realpath ", "basename ", "dirname ",
            "git status", "git branch", "git log", "git diff", "git show",
            "git remote", "git tag", "git describe", "git rev-parse",
            "echo ", "printf ", "true", "false", "test ",
            "date ", "time ", "env ", "printenv ",
        ];

        for prefix in &safe_prefixes {
            if trimmed.starts_with(prefix) {
                return true;
            }
        }

        // Exact matches for safe commands
        let safe_exact = ["ls", "pwd", "whoami", "uname", "date", "uptime", "free", "true", "false"];
        safe_exact.contains(&trimmed)
    }

    /// Execute a command and return stdout, stderr, and exit code.
    fn execute_command(
        command: &str,
        timeout_ms: u64,
        _on_progress: Option<&dyn Fn(String)>,
    ) -> Result<(String, String, i32, bool), String> {
        let timeout = std::time::Duration::from_millis(timeout_ms);

        let mut child = std::process::Command::new("bash")
            .arg("-c")
            .arg(command)
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::piped())
            .spawn()
            .map_err(|e| format!("Failed to execute command: {}", e))?;

        let start = std::time::Instant::now();
        let mut interrupted = false;

        // Poll for completion with timeout
        loop {
            if start.elapsed() > timeout {
                // Kill the process
                let _ = child.kill();
                interrupted = true;
                break;
            }

            match child.try_wait() {
                Ok(Some(status)) => {
                    let code = status.code().unwrap_or(-1);
                    let output = child
                        .wait_with_output()
                        .map_err(|e| format!("Failed to get output: {}", e))?;
                    let stdout = String::from_utf8_lossy(&output.stdout).to_string();
                    let stderr = String::from_utf8_lossy(&output.stderr).to_string();
                    return Ok((stdout, stderr, code, interrupted));
                }
                Ok(None) => {
                    // Progress callback
                    if let Some(cb) = _on_progress {
                        if start.elapsed().as_millis() as u64 > PROGRESS_THRESHOLD_MS {
                            cb("Command still running...".to_string());
                        }
                    }
                    std::thread::sleep(std::time::Duration::from_millis(100));
                }
                Err(e) => {
                    return Err(format!("Failed to wait for command: {}", e));
                }
            }
        }

        // Process was killed
        let output = child
            .wait_with_output()
            .map_err(|e| format!("Failed to get output: {}", e))?;
        let stdout = String::from_utf8_lossy(&output.stdout).to_string();
        let stderr = String::from_utf8_lossy(&output.stderr).to_string();
        Ok((stdout, stderr, -1, interrupted))
    }
}

#[async_trait]
impl Tool for BashTool {
    fn name(&self) -> &str {
        "Bash"
    }

    fn search_hint(&self) -> Option<&str> {
        Some("Execute shell commands")
    }

    async fn call(
        &self,
        input: serde_json::Value,
        context: &ToolUseContext,
        on_progress: Option<Arc<dyn Fn(ToolProgress) + Send + Sync>>,
    ) -> anyhow::Result<ToolResult<serde_json::Value>> {
        let command = input["command"]
            .as_str()
            .ok_or_else(|| anyhow::anyhow!("command is required"))?;
        let timeout = input["timeout"]
            .as_u64()
            .unwrap_or(DEFAULT_TIMEOUT_MS)
            .min(MAX_TIMEOUT_MS);
        let run_in_background = input["run_in_background"].as_bool().unwrap_or(false);

        // Check abort
        let abort_rx = context.abort_controller.subscribe();
        let mut aborted = false;

        let progress_cb = |msg: String| {
            if let Some(ref cb) = on_progress {
                cb(ToolProgress {
                    tool_use_id: String::new(),
                    data: serde_json::json!({ "message": msg }),
                });
            }
        };

        let (stdout, stderr, code, interrupted) = if run_in_background {
            // Background execution - spawn and return immediately
            let cmd = command.to_string();
            let timeout_val = timeout;
            std::thread::spawn(move || {
                let _ = Self::execute_command(&cmd, timeout_val, None);
            });
            let task_id = uuid::Uuid::new_v4().to_string();
            (
                String::new(),
                String::new(),
                0,
                false,
            )
        } else {
            // Check for abort
            if *abort_rx.borrow() {
                aborted = true;
                (String::new(), String::new(), -1, true)
            } else {
                Self::execute_command(command, timeout, Some(&progress_cb))
                    .map_err(|e| anyhow::anyhow!(e))?
            }
        };

        // Build result
        let mut result = serde_json::json!({
            "stdout": stdout,
            "stderr": stderr,
            "code": code,
            "interrupted": interrupted || aborted,
        });

        if run_in_background {
            let task_id = uuid::Uuid::new_v4().to_string();
            result["backgroundTaskId"] = serde_json::Value::String(task_id);
        }

        // Format output for model
        let output = if stdout.is_empty() && stderr.is_empty() {
            if code == 0 {
                "(no output)".to_string()
            } else {
                format!("(command exited with code {})", code)
            }
        } else {
            let mut out = String::new();
            if !stdout.is_empty() {
                out.push_str(&stdout);
            }
            if !stderr.is_empty() {
                if !out.is_empty() {
                    out.push_str("\n");
                }
                out.push_str(&stderr);
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
        Ok("Executes a given bash command".to_string())
    }

    fn input_schema(&self) -> serde_json::Value {
        serde_json::json!({
            "type": "object",
            "properties": {
                "command": {
                    "type": "string",
                    "description": "The command to execute"
                },
                "timeout": {
                    "type": "number",
                    "description": "Optional timeout in milliseconds (max 600000)"
                },
                "description": {
                    "type": "string",
                    "description": "Clear, concise description of what this command does"
                },
                "run_in_background": {
                    "type": "boolean",
                    "description": "Run command in background"
                },
                "dangerouslyDisableSandbox": {
                    "type": "boolean",
                    "description": "Override sandbox mode"
                }
            },
            "required": ["command"]
        })
    }

    fn is_read_only(&self, input: &serde_json::Value) -> bool {
        Self::is_read_only(input)
    }

    fn is_destructive(&self, input: &serde_json::Value) -> bool {
        !Self::is_read_only(input)
    }

    fn is_concurrency_safe(&self, input: &serde_json::Value) -> bool {
        // Bash commands are not concurrency-safe in general
        Self::is_read_only(input)
    }

    fn interrupt_behavior(&self) -> InterruptBehavior {
        InterruptBehavior::Cancel
    }

    fn is_search_or_read_command(&self, input: &serde_json::Value) -> SearchOrReadInfo {
        let is_read = Self::is_read_only(input);
        SearchOrReadInfo {
            is_search: false,
            is_read: is_read,
            is_list: false,
        }
    }

    fn is_open_world(&self, _input: &serde_json::Value) -> bool {
        true
    }

    async fn validate_input(
        &self,
        input: &serde_json::Value,
        _context: &ToolUseContext,
    ) -> anyhow::Result<ValidationResult> {
        if input["command"].as_str().is_none() {
            return Ok(ValidationResult::Invalid {
                message: "command is required".to_string(),
                error_code: 0,
            });
        }
        let command = input["command"].as_str().unwrap();
        if command.trim().is_empty() {
            return Ok(ValidationResult::Invalid {
                message: "command cannot be empty".to_string(),
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
        // Auto-allow read-only commands
        if Self::is_read_only(input) {
            return Ok(PermissionResult::Allow {
                updated_input: Some(input.clone()),
                user_modified: None,
                decision_reason: None,
                tool_use_id: None,
                accept_feedback: None,
                content_blocks: None,
            });
        }

        // Fall back to general read permission check for now
        // Full bash permission system is complex (AST-based security checks)
        check_read_permission(input, context, "bash")
    }

    fn max_result_size_chars(&self) -> usize {
        200_000
    }

    async fn prompt(&self, _options: &ToolPromptOptions) -> anyhow::Result<String> {
        Ok(
            "Executes a given bash command and returns its output.\n\n\
            Usage:\n\
            - The working directory persists between commands, but shell state does not. The shell environment is initialized from the user's profile (bash or zsh).\n\
            - For multiple independent commands, run them in parallel in a single message.\n\
            - For sequential commands where later commands depend on earlier ones, use && to chain them.\n\
            - Use ; only when earlier command failures don't matter.\n\
            - Prefer dedicated tools over bash commands when available:\n\
              * Use Read instead of cat, head, tail\n\
              * Use Edit instead of sed, awk\n\
              * Use Write instead of echo, cat with redirect\n\
              * Use Grep instead of grep, rg\n\
              * Use Glob instead of find\n\
            - Timeout: Commands timeout after 120 seconds by default (max 600 seconds).\n\
            - Background: Use run_in_background: true for long-running commands.\n\
            - Git safety: Never commit, push, or force-push unless explicitly asked. Never amend commits."
                .to_string(),
        )
    }

    fn user_facing_name(&self, _input: Option<&serde_json::Value>) -> String {
        "Bash".to_string()
    }

    fn get_activity_description(&self, _input: Option<&serde_json::Value>) -> Option<String> {
        Some("Running command".to_string())
    }

    fn map_tool_result_to_block(
        &self,
        content: serde_json::Value,
        tool_use_id: &str,
    ) -> ContentBlockParam {
        let output = content["output"]
            .as_str()
            .unwrap_or("(no output)")
            .to_string();
        ContentBlockParam::ToolResult {
            tool_use_id: tool_use_id.to_string(),
            content: vec![cc_core::messages::ToolResultContent::Text { text: output }],
            is_error: None,
        }
    }
}
