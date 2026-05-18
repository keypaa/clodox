use std::path::Path;

use cc_core::permissions::{PermissionDecisionReason, PermissionResult};
use cc_core::tools::ToolUseContext;

/// Expand a path string, handling ~ and normalizing.
pub fn expand_path(path: &str) -> String {
    let path = path.trim();

    // Handle ~ expansion
    if path.starts_with("~/") || path == "~" {
        if let Some(home) = dirs::home_dir() {
            let rest = path.trim_start_matches("~");
            return format!("{}{}", home.display(), rest);
        }
    }

    path.to_string()
}

/// Check if a path is a UNC path (\\ or //).
pub fn is_unc_path(path: &str) -> bool {
    path.starts_with("\\\\") || path.starts_with("//")
}

/// Check if a path is a blocked device path.
pub fn is_blocked_device_path(path: &str) -> bool {
    let blocked = [
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
    blocked.iter().any(|b| path.starts_with(*b))
}

/// Check read permission for a tool.
pub fn check_read_permission(
    input: &serde_json::Value,
    _context: &ToolUseContext,
    _tool_name: &str,
) -> anyhow::Result<PermissionResult> {
    // Extract path from input
    let path = input["file_path"]
        .as_str()
        .or_else(|| input["path"].as_str())
        .or_else(|| input["pattern"].as_str())
        .unwrap_or("");

    // Check for UNC paths
    if is_unc_path(path) {
        return Ok(PermissionResult::Deny {
            message: "UNC paths are not allowed for security reasons".to_string(),
            decision_reason: PermissionDecisionReason::Other { reason: "Blocked path".to_string() },
            tool_use_id: None,
        });
    }

    // Check for blocked device paths
    if is_blocked_device_path(path) {
        return Ok(PermissionResult::Deny {
            message: "Access to device paths is not allowed".to_string(),
            decision_reason: PermissionDecisionReason::Other { reason: "Blocked path".to_string() },
            tool_use_id: None,
        });
    }

    // Default: allow
    Ok(PermissionResult::Allow {
        updated_input: Some(input.clone()),
        user_modified: None,
        decision_reason: None,
        tool_use_id: None,
        accept_feedback: None,
        content_blocks: None,
    })
}

/// Check write permission for a tool.
pub fn check_write_permission(
    input: &serde_json::Value,
    _context: &ToolUseContext,
    _tool_name: &str,
) -> anyhow::Result<PermissionResult> {
    let path = input["file_path"].as_str().unwrap_or("");

    // Check for UNC paths
    if is_unc_path(path) {
        return Ok(PermissionResult::Deny {
            message: "UNC paths are not allowed for security reasons".to_string(),
            decision_reason: PermissionDecisionReason::Other { reason: "Blocked path".to_string() },
            tool_use_id: None,
        });
    }

    // Default: allow
    Ok(PermissionResult::Allow {
        updated_input: Some(input.clone()),
        user_modified: None,
        decision_reason: None,
        tool_use_id: None,
        accept_feedback: None,
        content_blocks: None,
    })
}

/// Check if a file extension is binary.
pub fn is_binary_extension(path: &Path) -> bool {
    let binary_exts = [
        "exe", "dll", "so", "dylib", "bin", "o", "a", "lib", "pyc", "pyo",
        "class", "jar", "war", "ear", "zip", "tar", "gz", "bz2", "xz", "7z",
        "rar", "iso", "img", "dmg", "wasm", "ttf", "otf", "woff", "woff2",
        "eot", "ico", "cur", "mp3", "mp4", "avi", "mov", "wmv", "flv",
    ];
    path.extension()
        .and_then(|e| e.to_str())
        .map(|e| binary_exts.contains(&e.to_lowercase().as_str()))
        .unwrap_or(false)
}

/// Check if a file extension is an image.
pub fn is_image_extension(path: &Path) -> bool {
    let image_exts = ["png", "jpg", "jpeg", "gif", "webp", "bmp", "svg"];
    path.extension()
        .and_then(|e| e.to_str())
        .map(|e| image_exts.contains(&e.to_lowercase().as_str()))
        .unwrap_or(false)
}
