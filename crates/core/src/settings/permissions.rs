use serde::{Deserialize, Serialize};

/// Permission-related settings.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct PermissionSettings {
    /// Auto-allowed commands/tools.
    pub allow: Vec<PermissionRule>,
    /// Auto-denied commands/tools.
    pub deny: Vec<PermissionRule>,
    /// Always-prompt commands/tools.
    pub ask: Vec<PermissionRule>,
    /// Default permission mode.
    pub default_mode: Option<PermissionMode>,
    /// Extra allowed directories.
    pub additional_directories: Vec<AdditionalDirectory>,
}

/// A permission rule matching a tool/command/path.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PermissionRule {
    /// Tool name or wildcard (e.g., "Bash", "Read", "*").
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tool: Option<String>,
    /// Command pattern for Bash tool (e.g., "npm test", "git status").
    #[serde(skip_serializing_if = "Option::is_none")]
    pub command: Option<String>,
    /// Path pattern for file tools (wildcard matching).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub path: Option<String>,
    /// Sub-command pattern.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub sub_command: Option<String>,
}

/// Permission mode enumeration.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum PermissionMode {
    /// Normal permission prompts.
    #[default]
    Default,
    /// Auto-accept file edits.
    AcceptEdits,
    /// Auto-accept all (except dangerous).
    AutoAccept,
    /// Plan mode (no tool execution).
    Plan,
    /// Full auto (everything allowed).
    DangerFullAuto,
}

/// Additional working directory with its source.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AdditionalDirectory {
    pub path: String,
    pub source: PermissionRuleSource,
}

/// Source of a permission rule.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum PermissionRuleSource {
    User,
    Project,
    Local,
    Managed,
    Builtin,
}

impl PermissionSettings {
    /// Check if a tool/command matches an allow rule.
    pub fn is_allowed(&self, tool_name: &str, command: Option<&str>, path: Option<&str>) -> bool {
        self.allow
            .iter()
            .any(|rule| rule.matches(tool_name, command, path))
    }

    /// Check if a tool/command matches a deny rule.
    pub fn is_denied(&self, tool_name: &str, command: Option<&str>, path: Option<&str>) -> bool {
        self.deny
            .iter()
            .any(|rule| rule.matches(tool_name, command, path))
    }

    /// Check if a tool/command matches an ask rule.
    pub fn is_ask(&self, tool_name: &str, command: Option<&str>, path: Option<&str>) -> bool {
        self.ask
            .iter()
            .any(|rule| rule.matches(tool_name, command, path))
    }
}

impl PermissionRule {
    /// Check if this rule matches the given tool/command/path.
    pub fn matches(&self, tool_name: &str, command: Option<&str>, path: Option<&str>) -> bool {
        // Tool must match (with wildcard support)
        if let Some(ref rule_tool) = self.tool {
            if !match_wildcard(rule_tool, tool_name) {
                return false;
            }
        }

        // Command must match if specified
        if let Some(ref rule_command) = self.command {
            if let Some(cmd) = command {
                if !match_wildcard(rule_command, cmd) {
                    return false;
                }
            } else {
                return false;
            }
        }

        // Path must match if specified
        if let Some(ref rule_path) = self.path {
            if let Some(p) = path {
                if !match_wildcard(rule_path, p) {
                    return false;
                }
            } else {
                return false;
            }
        }

        true
    }
}

/// Match a string against a wildcard pattern.
/// Supports `*` (any characters) and `**` (any path segments).
pub fn match_wildcard(pattern: &str, text: &str) -> bool {
    if pattern == "*" {
        return true;
    }
    if pattern == text {
        return true;
    }

    // Simple wildcard matching
    let pattern_parts: Vec<&str> = pattern.split('*').collect();
    if pattern_parts.len() == 1 {
        return pattern == text;
    }

    let mut pos = 0;
    for (i, part) in pattern_parts.iter().enumerate() {
        if part.is_empty() {
            continue;
        }
        if i == 0 {
            // Must match at start
            if !text.starts_with(part) {
                return false;
            }
            pos = part.len();
        } else if i == pattern_parts.len() - 1 {
            // Must match at end
            if !text[pos..].ends_with(part) {
                return false;
            }
        } else {
            // Must match somewhere in between
            if let Some(found) = text[pos..].find(part) {
                pos = pos + found + part.len();
            } else {
                return false;
            }
        }
    }

    true
}
