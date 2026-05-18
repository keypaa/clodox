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
use crate::file_read::ReadFileState;

/// Max file size for editing (1 GiB).
const MAX_FILE_SIZE_BYTES: u64 = 1024 * 1024 * 1024;

/// FileEdit tool - performs exact string replacements in files.
#[derive(Debug)]
pub struct FileEditTool {
    read_state: Arc<std::sync::Mutex<HashMap<String, ReadFileState>>>,
}

impl FileEditTool {
    pub fn new(
        read_state: Arc<std::sync::Mutex<HashMap<String, ReadFileState>>>,
    ) -> Arc<dyn Tool> {
        Arc::new(Self { read_state })
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

    fn count_occurrences(content: &str, old_string: &str) -> usize {
        if old_string.is_empty() {
            return 0;
        }
        content.matches(old_string).count()
    }

    fn normalize_quotes(s: &str) -> String {
        s.replace('\u{2018}', "'")
            .replace('\u{2019}', "'")
            .replace('\u{201c}', "\"")
            .replace('\u{201d}', "\"")
    }
}

#[async_trait]
impl Tool for FileEditTool {
    fn name(&self) -> &str {
        "Edit"
    }

    fn search_hint(&self) -> Option<&str> {
        Some("Edit file contents")
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
        let old_string = input["old_string"].as_str().unwrap_or("");
        let new_string = input["new_string"]
            .as_str()
            .ok_or_else(|| anyhow::anyhow!("new_string is required"))?;
        let replace_all = input["replace_all"].as_bool().unwrap_or(false);

        let path = Path::new(&file_path);

        // Error code 1: old_string === new_string
        if old_string == new_string {
            return Err(anyhow::anyhow!(
                "error code 1: old_string and new_string are the same"
            ));
        }

        // Error code 4: File doesn't exist with non-empty old_string
        if !path.exists() && !old_string.is_empty() {
            return Err(anyhow::anyhow!(
                "error code 4: File does not exist. To create a new file, use old_string: \"\""
            ));
        }

        // Error code 3: File exists with empty old_string (not empty file)
        if path.exists() && old_string.is_empty() {
            let content = std::fs::read_to_string(path).unwrap_or_default();
            if !content.is_empty() {
                return Err(anyhow::anyhow!(
                    "error code 3: File exists and is not empty. To replace the entire file, use the Write tool."
                ));
            }
        }

        // Error code 5: Jupyter notebook
        if path.extension().map(|e| e == "ipynb").unwrap_or(false) {
            return Err(anyhow::anyhow!(
                "error code 5: Cannot edit Jupyter notebooks with Edit tool"
            ));
        }

        // Error code 6: File not read
        let read_state = self.read_state.lock().unwrap();
        if !read_state.contains_key(&file_path) && path.exists() {
            return Err(anyhow::anyhow!(
                "error code 6: You must use the Read tool first to read the file's contents before editing it."
            ));
        }

        // Error code 7: File modified since read
        if let Some(state) = read_state.get(&file_path) {
            if let Ok(current_metadata) = std::fs::metadata(path) {
                if let Ok(current_mtime) = current_metadata.modified() {
                    if current_mtime != state.mtime {
                        if let Ok(current_content) = std::fs::read_to_string(path) {
                            if current_content != state.content {
                                return Err(anyhow::anyhow!(
                                    "error code 7: File has been modified since it was last read. Read it again before editing."
                                ));
                            }
                        }
                    }
                }
            }
        }
        drop(read_state);

        // Read file content
        let original_content = if path.exists() {
            std::fs::read_to_string(path).map_err(|e| anyhow::anyhow!("{}", e))?
        } else {
            String::new()
        };

        // Check file size
        if let Ok(metadata) = std::fs::metadata(path) {
            if metadata.len() > MAX_FILE_SIZE_BYTES {
                return Err(anyhow::anyhow!(
                    "error code 10: File too large ({} bytes, max {} bytes)",
                    metadata.len(),
                    MAX_FILE_SIZE_BYTES
                ));
            }
        }

        // Normalize quotes for matching
        let normalized_old = Self::normalize_quotes(old_string);
        let normalized_content = Self::normalize_quotes(&original_content);

        // Count occurrences
        let occurrences = Self::count_occurrences(&normalized_content, &normalized_old);

        // Error code 8: String not found
        if occurrences == 0 {
            return Err(anyhow::anyhow!(
                "error code 8: String not found in file. The edit will FAIL if old_string is not unique in the file."
            ));
        }

        // Error code 9: Multiple matches without replace_all
        if occurrences > 1 && !replace_all {
            return Err(anyhow::anyhow!(
                "error code 9: String found {} times in file. Use replace_all: true to replace all occurrences, or provide more context to make old_string unique.",
                occurrences
            ));
        }

        // Perform replacement
        let new_content = if replace_all {
            normalized_content.replace(&normalized_old, new_string)
        } else {
            normalized_content.replacen(&normalized_old, new_string, 1)
        };

        // Write to disk
        std::fs::write(path, &new_content)
            .map_err(|e| anyhow::anyhow!("Failed to write file: {}", e))?;

        // Generate diff
        let structured_patch = Self::generate_diff(&original_content, &new_content);

        let message = if replace_all {
            format!(
                "The file {} has been updated. All occurrences were successfully replaced.",
                file_path
            )
        } else {
            format!("The file {} has been updated successfully.", file_path)
        };

        let result = serde_json::json!({
            "filePath": file_path,
            "oldString": old_string,
            "newString": new_string,
            "originalFile": original_content,
            "structuredPatch": structured_patch,
            "userModified": false,
            "replaceAll": replace_all,
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
        Ok("Performs exact string replacements in files".to_string())
    }

    fn input_schema(&self) -> serde_json::Value {
        serde_json::json!({
            "type": "object",
            "properties": {
                "file_path": {
                    "type": "string",
                    "description": "Absolute path to the file to modify"
                },
                "old_string": {
                    "type": "string",
                    "description": "The text to replace"
                },
                "new_string": {
                    "type": "string",
                    "description": "The text to replace it with (must differ from old_string)"
                },
                "replace_all": {
                    "type": "boolean",
                    "description": "Replace all occurrences (default: false)"
                }
            },
            "required": ["file_path", "old_string", "new_string"]
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
        if input["new_string"].as_str().is_none() {
            return Ok(ValidationResult::Invalid {
                message: "new_string is required".to_string(),
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
        check_write_permission(input, context, "edit")
    }

    fn max_result_size_chars(&self) -> usize {
        100_000
    }

    async fn prompt(&self, _options: &ToolPromptOptions) -> anyhow::Result<String> {
        Ok(
            "Performs exact string replacements in files.\n\n\
            Usage:\n\
            - You must use your Read tool at least once in the conversation before editing. This tool will error if you attempt an edit without reading the file.\n\
            - When editing text from Read tool output, ensure you preserve the exact indentation (tabs/spaces) as it appears AFTER the line number prefix. The line number prefix format is: spaces + line number + arrow. Never include any part of the line number prefix in the old_string or new_string.\n\
            - ALWAYS prefer editing existing files in the codebase. NEVER write new files unless explicitly required.\n\
            - Only use emojis if the user explicitly requests it.\n\
            - The edit will FAIL if old_string is not unique in the file. Either provide a larger string with more surrounding context to make it unique or use replace_all to change every instance.\n\
            - Use replace_all for replacing and renaming strings across the file."
                .to_string(),
        )
    }

    fn user_facing_name(&self, _input: Option<&serde_json::Value>) -> String {
        "Edit".to_string()
    }

    fn get_activity_description(&self, _input: Option<&serde_json::Value>) -> Option<String> {
        Some("Editing file".to_string())
    }

    fn map_tool_result_to_block(
        &self,
        content: serde_json::Value,
        tool_use_id: &str,
    ) -> ContentBlockParam {
        let output = content["output"]
            .as_str()
            .unwrap_or("File edited")
            .to_string();
        ContentBlockParam::ToolResult {
            tool_use_id: tool_use_id.to_string(),
            content: vec![cc_core::messages::ToolResultContent::Text { text: output }],
            is_error: None,
        }
    }
}
