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

use crate::utils::{check_read_permission, expand_path};

/// Default max lines to read.
const DEFAULT_MAX_LINES: usize = 2000;

/// Default max output tokens (rough estimate: chars/4).
const DEFAULT_MAX_TOKENS: usize = 25_000;

/// Default max file size (256 KB).
const DEFAULT_MAX_SIZE_BYTES: u64 = 256 * 1024;

/// Image extensions supported.
const IMAGE_EXTENSIONS: &[&str] = &[".png", ".jpg", ".jpeg", ".gif", ".webp", ".bmp", ".svg"];

/// Blocked device paths.
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

/// Read file state for staleness checking.
#[derive(Debug, Clone)]
pub struct ReadFileState {
    pub content: String,
    pub mtime: std::time::SystemTime,
    pub offset: usize,
    pub limit: usize,
}

/// FileRead tool - reads files from the filesystem.
#[derive(Debug)]
pub struct FileReadTool {
    read_state: Arc<std::sync::Mutex<HashMap<String, ReadFileState>>>,
}

impl FileReadTool {
    pub fn new() -> Arc<dyn Tool> {
        Arc::new(Self {
            read_state: Arc::new(std::sync::Mutex::new(HashMap::new())),
        })
    }

    pub fn is_image(path: &Path) -> bool {
        path.extension()
            .and_then(|e| e.to_str())
            .map(|e| IMAGE_EXTENSIONS.contains(&e.to_lowercase().as_str()))
            .unwrap_or(false)
    }

    pub fn is_notebook(path: &Path) -> bool {
        path.extension()
            .and_then(|e| e.to_str())
            .map(|e| e.to_lowercase() == "ipynb")
            .unwrap_or(false)
    }

    pub fn is_pdf(path: &Path) -> bool {
        path.extension()
            .and_then(|e| e.to_str())
            .map(|e| e.to_lowercase() == "pdf")
            .unwrap_or(false)
    }

    pub fn is_binary_extension(path: &Path) -> bool {
        let binary_exts = [
            ".exe", ".dll", ".so", ".dylib", ".bin", ".o", ".a", ".lib", ".pyc", ".pyo",
            ".class", ".jar", ".war", ".ear", ".zip", ".tar", ".gz", ".bz2", ".xz", ".7z",
            ".rar", ".iso", ".img", ".dmg", ".wasm", ".ttf", ".otf", ".woff", ".woff2",
            ".eot", ".ico", ".cur", ".mp3", ".mp4", ".avi", ".mov", ".wmv", ".flv",
        ];
        path.extension()
            .and_then(|e| e.to_str())
            .map(|e| binary_exts.contains(&e.to_lowercase().as_str()))
            .unwrap_or(false)
    }

    pub fn read_text_file(
        &self,
        file_path: &str,
        offset: usize,
        limit: usize,
    ) -> Result<(String, usize, usize), String> {
        let content = std::fs::read_to_string(file_path)
            .map_err(|e| format!("Failed to read file: {}", e))?;

        let lines: Vec<&str> = content.lines().collect();
        let total_lines = lines.len();
        let start = offset.saturating_sub(1); // 1-indexed
        let end = (start + limit).min(total_lines);

        let selected: Vec<&str> = lines.iter().skip(start).take(limit).copied().collect();

        // Format with line numbers (cat -n style)
        let numbered: Vec<String> = selected
            .iter()
            .enumerate()
            .map(|(i, line)| format!("{:>6}\t{}", start + i + 1, line))
            .collect();

        Ok((numbered.join("\n"), total_lines, start + 1))
    }
}

#[async_trait]
impl Tool for FileReadTool {
    fn name(&self) -> &str {
        "Read"
    }

    fn search_hint(&self) -> Option<&str> {
        Some("Read file contents")
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
        let offset = input["offset"].as_u64().unwrap_or(1) as usize;
        let limit = input["limit"].as_u64().unwrap_or(DEFAULT_MAX_LINES as u64) as usize;

        let path = Path::new(&file_path);

        // Check file exists
        if !path.exists() {
            return Err(anyhow::anyhow!("File not found: {}", file_path));
        }

        // Check file size
        let metadata = std::fs::metadata(path).map_err(|e| anyhow::anyhow!("{}", e))?;
        if metadata.len() > DEFAULT_MAX_SIZE_BYTES {
            return Err(anyhow::anyhow!(
                "File too large ({} bytes, max {} bytes)",
                metadata.len(),
                DEFAULT_MAX_SIZE_BYTES
            ));
        }

        // Check if directory
        if path.is_dir() {
            return Err(anyhow::anyhow!(
                "Cannot read directory: use Glob or Bash instead"
            ));
        }

        // Check blocked device paths
        for blocked in BLOCKED_DEVICE_PATHS {
            if file_path.starts_with(blocked) {
                return Err(anyhow::anyhow!("Access to {} is not allowed", blocked));
            }
        }

        // Check binary extension
        if Self::is_binary_extension(path) && !Self::is_image(path) && !Self::is_pdf(path) {
            return Err(anyhow::anyhow!(
                "Cannot read binary file: {}",
                file_path
            ));
        }

        // Handle different file types
        if Self::is_image(path) {
            // Image handling - return placeholder (full implementation needs base64 encoding)
            let result = serde_json::json!({
                "type": "image",
                "file": {
                    "filePath": file_path,
                    "message": "Image reading requires base64 encoding (not yet implemented)"
                }
            });
            return Ok(ToolResult {
                data: result,
                new_messages: None,
                mcp_meta: None,
            });
        }

        if Self::is_notebook(path) {
            let content = std::fs::read_to_string(path).map_err(|e| anyhow::anyhow!("{}", e))?;
            let result = serde_json::json!({
                "type": "notebook",
                "file": {
                    "filePath": file_path,
                    "content": content
                }
            });
            return Ok(ToolResult {
                data: result,
                new_messages: None,
                mcp_meta: None,
            });
        }

        if Self::is_pdf(path) {
            let result = serde_json::json!({
                "type": "pdf",
                "file": {
                    "filePath": file_path,
                    "message": "PDF reading requires pdf parsing (not yet implemented)"
                }
            });
            return Ok(ToolResult {
                data: result,
                new_messages: None,
                mcp_meta: None,
            });
        }

        // Text file reading
        let (content, total_lines, start_line) = self
            .read_text_file(&file_path, offset, limit)
            .map_err(|e| anyhow::anyhow!(e))?;

        // Token estimation
        let estimated_tokens = content.len() / 4;
        if estimated_tokens > DEFAULT_MAX_TOKENS {
            return Err(anyhow::anyhow!(
                "File content too large (~{} tokens, max ~{} tokens). Use offset/limit to read a smaller portion.",
                estimated_tokens,
                DEFAULT_MAX_TOKENS
            ));
        }

        // Store read state for staleness checking
        if let Ok(raw_content) = std::fs::read_to_string(&file_path) {
            if let Ok(mtime) = metadata.modified() {
                let mut state = self.read_state.lock().unwrap();
                state.insert(
                    file_path.clone(),
                    ReadFileState {
                        content: raw_content,
                        mtime,
                        offset,
                        limit,
                    },
                );
            }
        }

        let result = serde_json::json!({
            "type": "text",
            "file": {
                "filePath": file_path,
                "content": content,
                "numLines": limit,
                "startLine": start_line,
                "totalLines": total_lines
            }
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
        Ok("Reads a file from the local filesystem".to_string())
    }

    fn input_schema(&self) -> serde_json::Value {
        serde_json::json!({
            "type": "object",
            "properties": {
                "file_path": {
                    "type": "string",
                    "description": "Absolute path to file"
                },
                "offset": {
                    "type": "number",
                    "description": "Line number to start reading from (1-indexed, default: 1)"
                },
                "limit": {
                    "type": "number",
                    "description": "Number of lines to read (default: 2000)"
                },
                "pages": {
                    "type": "string",
                    "description": "Page range for PDFs (e.g., \"1-5\", \"3\", \"10-20\")"
                }
            },
            "required": ["file_path"]
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
            is_search: false,
            is_read: true,
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
        let file_path = input["file_path"].as_str().unwrap();
        let file_path = expand_path(file_path);
        for blocked in BLOCKED_DEVICE_PATHS {
            if file_path.starts_with(blocked) {
                return Ok(ValidationResult::Invalid {
                    message: format!("Access to {} is not allowed", blocked),
                    error_code: 0,
                });
            }
        }
        Ok(ValidationResult::Valid)
    }

    async fn check_permissions(
        &self,
        input: &serde_json::Value,
        context: &ToolUseContext,
    ) -> anyhow::Result<PermissionResult> {
        check_read_permission(input, context, "read")
    }

    fn max_result_size_chars(&self) -> usize {
        100_000
    }

    async fn prompt(&self, _options: &ToolPromptOptions) -> anyhow::Result<String> {
        Ok(
            "Reads a file from the local filesystem. You can access any file directly by using this tool.\n\
            Assume this tool is able to read all files on the machine. If the User provides a path to a file assume that path is valid.\n\n\
            Usage:\n\
            - The file_path parameter must be an absolute path, not a relative path\n\
            - By default, it reads up to 2000 lines starting from the beginning of the file\n\
            - You can optionally specify a line offset and limit\n\
            - Results are returned using cat -n format, with line numbers starting at 1\n\
            - This tool can read images (PNG, JPG, etc.) - presented visually as multimodal\n\
            - This tool can read PDF files (.pdf). For large PDFs (>10 pages), use pages parameter. Max 20 pages per request.\n\
            - This tool can read Jupyter notebooks (.ipynb)\n\
            - This tool can only read files, not directories\n\
            - If you read a file that exists but has empty contents you will receive a system reminder warning"
                .to_string(),
        )
    }

    fn user_facing_name(&self, _input: Option<&serde_json::Value>) -> String {
        "Read".to_string()
    }

    fn get_activity_description(&self, _input: Option<&serde_json::Value>) -> Option<String> {
        Some("Reading file".to_string())
    }

    fn map_tool_result_to_block(
        &self,
        content: serde_json::Value,
        tool_use_id: &str,
    ) -> ContentBlockParam {
        let file = &content["file"];
        let output = match content["type"].as_str() {
            Some("text") => file["content"].as_str().unwrap_or("").to_string(),
            Some("image") => "[Image content]".to_string(),
            Some("notebook") => "[Notebook content]".to_string(),
            Some("pdf") => "[PDF content]".to_string(),
            _ => "[Unknown content type]".to_string(),
        };
        ContentBlockParam::ToolResult {
            tool_use_id: tool_use_id.to_string(),
            content: vec![cc_core::messages::ToolResultContent::Text { text: output }],
            is_error: None,
        }
    }
}
