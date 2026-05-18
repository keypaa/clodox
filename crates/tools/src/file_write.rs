use std::collections::HashMap;
use std::path::Path;
use std::sync::Arc;

use async_trait::async_trait;
use cc_core::messages::ContentBlockParam;
use cc_core::permissions::PermissionResult;
use cc_core::tools::{
    InterruptBehavior, SearchOrReadInfo, Tool, ToolProgress, ToolPromptOptions, ToolResult,
    ToolUseContext,
};
use cc_core::types::ValidationResult;

use crate::utils::{check_write_permission, expand_path};

/// Write file state for staleness tracking.
#[derive(Debug, Clone)]
pub struct WriteFileState {
    pub content: String,
    pub mtime: std::time::SystemTime,
}

/// FileWrite tool - writes files to the filesystem.
#[derive(Debug)]
pub struct FileWriteTool {
    read_state: Arc<std::sync::Mutex<HashMap<String, crate::file_read::ReadFileState>>>,
    write_state: Arc<std::sync::Mutex<HashMap<String, WriteFileState>>>,
}

impl FileWriteTool {
    pub fn new(
        read_state: Arc<std::sync::Mutex<HashMap<String, crate::file_read::ReadFileState>>>,
    ) -> Arc<dyn Tool> {
        Arc::new(Self {
            read_state,
            write_state: Arc::new(std::sync::Mutex::new(HashMap::new())),
        })
    }

    fn generate_diff(original: &str, new: &str) -> String {
        let orig_lines: Vec<&str> = original.lines().collect();
        let new_lines: Vec<&str> = new.lines().collect();

        let mut diff = String::new();
        let mut i = 0;
        let mut j = 0;

        while i < orig_lines.len() || j < new_lines.len() {
            if i < orig_lines.len() && j < new_lines.len() && orig_lines[i] == new_lines[j] {
                i += 1;
                j += 1;
            } else if j < new_lines.len() {
                diff.push_str(&format!("+ {}\n", new_lines[j]));
                j += 1;
            } else {
                diff.push_str(&format!("- {}\n", orig_lines[i]));
                i += 1;
            }
        }

        if diff.is_empty() {
            "(no changes)".to_string()
        } else {
            diff
        }
    }
}

#[async_trait]
impl Tool for FileWriteTool {
    fn name(&self) -> &str {
        "Write"
    }

    fn search_hint(&self) -> Option<&str> {
        Some("Write file contents")
    }

    async fn call(
        &self,
        input: serde_json::Value,
        _context: &ToolUseContext,
        _on_progress: Option<Arc<dyn Fn(ToolProgress) + Send + Sync>>,
    ) -> anyhow::Result<ToolResult<serde_json::Value>> {
        let file_path = input["file_path"]
            .as_str()
            .ok_or_else(|| anyhow::anyhow!("file_path is required"))?;
        let file_path = expand_path(file_path);
        let content = input["content"]
            .as_str()
            .ok_or_else(|| anyhow::anyhow!("content is required"))?;

        let path = Path::new(&file_path);

        // Check read-first requirement
        let read_state = self.read_state.lock().unwrap();
        let was_read = read_state.contains_key(&file_path);
        drop(read_state);

        if !was_read && path.exists() {
            return Err(anyhow::anyhow!(
                "You must use the Read tool first to read the file's contents before writing to it."
            ));
        }

        // Staleness check for existing files
        if path.exists() {
            let read_state = self.read_state.lock().unwrap();
            if let Some(state) = read_state.get(&file_path) {
                if let Ok(current_metadata) = std::fs::metadata(path) {
                    if let Ok(current_mtime) = current_metadata.modified() {
                        if current_mtime != state.mtime {
                            // Content comparison fallback
                            if let Ok(current_content) = std::fs::read_to_string(path) {
                                if current_content != state.content {
                                    return Err(anyhow::anyhow!(
                                        "File has been modified since it was last read. Read it again before writing."
                                    ));
                                }
                            }
                        }
                    }
                }
            }
        }

        // Get original content for diff
        let original = if path.exists() {
            std::fs::read_to_string(path).ok()
        } else {
            None
        };

        let is_new = !path.exists();

        // Create parent directory if needed
        if let Some(parent) = path.parent() {
            if !parent.exists() {
                std::fs::create_dir_all(parent)
                    .map_err(|e| anyhow::anyhow!("Failed to create directory: {}", e))?;
            }
        }

        // Write content with LF line endings
        let normalized = content.replace("\r\n", "\n");
        std::fs::write(path, &normalized)
            .map_err(|e| anyhow::anyhow!("Failed to write file: {}", e))?;

        // Update write state
        if let Ok(mtime) = std::fs::metadata(path).and_then(|m| m.modified()) {
            let mut write_state = self.write_state.lock().unwrap();
            write_state.insert(
                file_path.clone(),
                WriteFileState {
                    content: normalized.clone(),
                    mtime,
                },
            );
        }

        // Generate diff
        let structured_patch = if let Some(orig) = &original {
            Self::generate_diff(orig, &normalized)
        } else {
            "(new file)".to_string()
        };

        let action_type = if is_new { "create" } else { "update" };
        let message = if is_new {
            format!("File created successfully at: {}", file_path)
        } else {
            format!("The file {} has been updated successfully.", file_path)
        };

        let result = serde_json::json!({
            "type": action_type,
            "filePath": file_path,
            "content": normalized,
            "structuredPatch": structured_patch,
            "originalFile": original,
            "output": message,
        });

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
        Ok("Writes a file to the local filesystem".to_string())
    }

    fn input_schema(&self) -> serde_json::Value {
        serde_json::json!({
            "type": "object",
            "properties": {
                "file_path": {
                    "type": "string",
                    "description": "Absolute path (must be absolute, not relative)"
                },
                "content": {
                    "type": "string",
                    "description": "Content to write to the file"
                }
            },
            "required": ["file_path", "content"]
        })
    }

    fn is_read_only(&self, _input: &serde_json::Value) -> bool {
        false
    }

    fn is_destructive(&self, _input: &serde_json::Value) -> bool {
        true
    }

    fn is_concurrency_safe(&self, _input: &serde_json::Value) -> bool {
        false
    }

    fn interrupt_behavior(&self) -> InterruptBehavior {
        InterruptBehavior::Block
    }

    fn is_search_or_read_command(&self, _input: &serde_json::Value) -> SearchOrReadInfo {
        SearchOrReadInfo {
            is_search: false,
            is_read: false,
            is_list: false,
        }
    }

    async fn validate_input(
        &self,
        input: &serde_json::Value,
        _context: &ToolUseContext,
    ) -> anyhow::Result<ValidationResult> {
        if input["file_path"].as_str().is_none() {
            return Ok(ValidationResult::Invalid {
                message: "file_path is required".to_string(),
                error_code: 0,
            });
        }
        if input["content"].as_str().is_none() {
            return Ok(ValidationResult::Invalid {
                message: "content is required".to_string(),
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
        check_write_permission(input, context, "write")
    }

    fn max_result_size_chars(&self) -> usize {
        100_000
    }

    async fn prompt(&self, _options: &ToolPromptOptions) -> anyhow::Result<String> {
        Ok(
            "Writes a file to the local filesystem.\n\n\
            Usage:\n\
            - This tool will overwrite the existing file if there is one at the provided path.\n\
            - If this is an existing file, you MUST use the Read tool first to read the file's contents. This tool will fail if you did not read the file first.\n\
            - Prefer the Edit tool for modifying existing files -- it only sends the diff. Only use this tool to create new files or for complete rewrites.\n\
            - NEVER create documentation files (*.md) or README files unless explicitly requested by the User.\n\
            - Only use emojis if the user explicitly requests it."
                .to_string(),
        )
    }

    fn user_facing_name(&self, _input: Option<&serde_json::Value>) -> String {
        "Write".to_string()
    }

    fn get_activity_description(&self, _input: Option<&serde_json::Value>) -> Option<String> {
        Some("Writing file".to_string())
    }

    fn map_tool_result_to_block(
        &self,
        content: serde_json::Value,
        tool_use_id: &str,
    ) -> ContentBlockParam {
        let output = content["output"]
            .as_str()
            .unwrap_or("File written")
            .to_string();
        ContentBlockParam::ToolResult {
            tool_use_id: tool_use_id.to_string(),
            content: vec![cc_core::messages::ToolResultContent::Text { text: output }],
            is_error: None,
        }
    }
}
