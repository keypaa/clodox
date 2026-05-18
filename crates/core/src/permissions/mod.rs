use serde::{Deserialize, Serialize};

use crate::messages::ContentBlockParam;

/// Permission modes for tool execution.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum PermissionMode {
    #[default]
    Default,
    AcceptEdits,
    BypassPermissions,
    DontAsk,
    Plan,
    Auto,
    Bubble,
}

/// External permission modes (user-addressable).
pub const EXTERNAL_PERMISSION_MODES: &[PermissionMode] = &[
    PermissionMode::AcceptEdits,
    PermissionMode::BypassPermissions,
    PermissionMode::Default,
    PermissionMode::DontAsk,
    PermissionMode::Plan,
];

/// Internal permission modes (including non-user-addressable).
pub const INTERNAL_PERMISSION_MODES: &[PermissionMode] = &[
    PermissionMode::AcceptEdits,
    PermissionMode::BypassPermissions,
    PermissionMode::Default,
    PermissionMode::DontAsk,
    PermissionMode::Plan,
    PermissionMode::Auto,
];

/// Permission behavior result.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum PermissionBehavior {
    Allow,
    Deny,
    Ask,
}

/// Source of a permission rule.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum PermissionRuleSource {
    UserSettings,
    ProjectSettings,
    LocalSettings,
    FlagSettings,
    PolicySettings,
    CliArg,
    Command,
    Session,
}

/// Value of a permission rule.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PermissionRuleValue {
    pub tool_name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub rule_content: Option<String>,
}

/// A complete permission rule.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PermissionRule {
    pub source: PermissionRuleSource,
    pub rule_behavior: PermissionBehavior,
    pub rule_value: PermissionRuleValue,
}

/// Where a permission update should be persisted.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum PermissionUpdateDestination {
    UserSettings,
    ProjectSettings,
    LocalSettings,
    Session,
    CliArg,
}

/// Permission update operations.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "camelCase")]
pub enum PermissionUpdate {
    AddRules {
        destination: PermissionUpdateDestination,
        rules: Vec<PermissionRuleValue>,
        behavior: PermissionBehavior,
    },
    ReplaceRules {
        destination: PermissionUpdateDestination,
        rules: Vec<PermissionRuleValue>,
        behavior: PermissionBehavior,
    },
    RemoveRules {
        destination: PermissionUpdateDestination,
        rules: Vec<PermissionRuleValue>,
        behavior: PermissionBehavior,
    },
    SetMode {
        destination: PermissionUpdateDestination,
        mode: PermissionMode,
    },
    AddDirectories {
        destination: PermissionUpdateDestination,
        directories: Vec<String>,
    },
    RemoveDirectories {
        destination: PermissionUpdateDestination,
        directories: Vec<String>,
    },
}

/// Permission decision result.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "behavior", rename_all = "camelCase")]
pub enum PermissionDecision<Input = serde_json::Value> {
    Allow {
        #[serde(skip_serializing_if = "Option::is_none")]
        updated_input: Option<Input>,
        #[serde(skip_serializing_if = "Option::is_none")]
        user_modified: Option<bool>,
        #[serde(skip_serializing_if = "Option::is_none")]
        decision_reason: Option<PermissionDecisionReason>,
        #[serde(skip_serializing_if = "Option::is_none")]
        tool_use_id: Option<String>,
        #[serde(skip_serializing_if = "Option::is_none")]
        accept_feedback: Option<String>,
        #[serde(skip_serializing_if = "Option::is_none")]
        content_blocks: Option<Vec<ContentBlockParam>>,
    },
    Ask {
        message: String,
        #[serde(skip_serializing_if = "Option::is_none")]
        updated_input: Option<Input>,
        #[serde(skip_serializing_if = "Option::is_none")]
        decision_reason: Option<PermissionDecisionReason>,
        #[serde(skip_serializing_if = "Option::is_none")]
        suggestions: Option<Vec<PermissionUpdate>>,
        #[serde(skip_serializing_if = "Option::is_none")]
        blocked_path: Option<String>,
        #[serde(skip_serializing_if = "Option::is_none")]
        pending_classifier_check: Option<PendingClassifierCheck>,
        #[serde(skip_serializing_if = "Option::is_none")]
        content_blocks: Option<Vec<ContentBlockParam>>,
    },
    Deny {
        message: String,
        decision_reason: PermissionDecisionReason,
        #[serde(skip_serializing_if = "Option::is_none")]
        tool_use_id: Option<String>,
    },
}

/// Permission result with additional passthrough option.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "behavior", rename_all = "camelCase")]
pub enum PermissionResult<Input = serde_json::Value> {
    Allow {
        #[serde(skip_serializing_if = "Option::is_none")]
        updated_input: Option<Input>,
        #[serde(skip_serializing_if = "Option::is_none")]
        user_modified: Option<bool>,
        #[serde(skip_serializing_if = "Option::is_none")]
        decision_reason: Option<PermissionDecisionReason>,
        #[serde(skip_serializing_if = "Option::is_none")]
        tool_use_id: Option<String>,
        #[serde(skip_serializing_if = "Option::is_none")]
        accept_feedback: Option<String>,
        #[serde(skip_serializing_if = "Option::is_none")]
        content_blocks: Option<Vec<ContentBlockParam>>,
    },
    Ask {
        message: String,
        #[serde(skip_serializing_if = "Option::is_none")]
        updated_input: Option<Input>,
        #[serde(skip_serializing_if = "Option::is_none")]
        decision_reason: Option<PermissionDecisionReason>,
        #[serde(skip_serializing_if = "Option::is_none")]
        suggestions: Option<Vec<PermissionUpdate>>,
        #[serde(skip_serializing_if = "Option::is_none")]
        blocked_path: Option<String>,
        #[serde(skip_serializing_if = "Option::is_none")]
        pending_classifier_check: Option<PendingClassifierCheck>,
        #[serde(skip_serializing_if = "Option::is_none")]
        content_blocks: Option<Vec<ContentBlockParam>>,
    },
    Deny {
        message: String,
        decision_reason: PermissionDecisionReason,
        #[serde(skip_serializing_if = "Option::is_none")]
        tool_use_id: Option<String>,
    },
    Passthrough {
        message: String,
        #[serde(skip_serializing_if = "Option::is_none")]
        decision_reason: Option<PermissionDecisionReason>,
        #[serde(skip_serializing_if = "Option::is_none")]
        suggestions: Option<Vec<PermissionUpdate>>,
        #[serde(skip_serializing_if = "Option::is_none")]
        blocked_path: Option<String>,
        #[serde(skip_serializing_if = "Option::is_none")]
        pending_classifier_check: Option<PendingClassifierCheck>,
    },
}

/// Metadata for a pending classifier check.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PendingClassifierCheck {
    pub command: String,
    pub cwd: String,
    pub descriptions: Vec<String>,
}

/// Explanation of why a permission decision was made.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "camelCase")]
pub enum PermissionDecisionReason {
    Rule { rule: PermissionRule },
    Mode { mode: PermissionMode },
    SubcommandResults {
        reasons: std::collections::HashMap<String, serde_json::Value>,
    },
    PermissionPromptTool {
        permission_prompt_tool_name: String,
        tool_result: serde_json::Value,
    },
    Hook {
        hook_name: String,
        #[serde(skip_serializing_if = "Option::is_none")]
        hook_source: Option<String>,
        #[serde(skip_serializing_if = "Option::is_none")]
        reason: Option<String>,
    },
    AsyncAgent { reason: String },
    SandboxOverride {
        reason: SandboxOverrideReason,
    },
    Classifier {
        classifier: String,
        reason: String,
    },
    WorkingDir { reason: String },
    SafetyCheck {
        reason: String,
        classifier_approvable: bool,
    },
    Other { reason: String },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum SandboxOverrideReason {
    ExcludedCommand,
    DangerouslyDisableSandbox,
}

/// Classifier result for bash command safety.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClassifierResult {
    pub matches: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub matched_description: Option<String>,
    pub confidence: ClassifierConfidence,
    pub reason: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum ClassifierConfidence {
    High,
    Medium,
    Low,
}

/// Classifier behavior type.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum ClassifierBehavior {
    Deny,
    Ask,
    Allow,
}

/// Classifier token usage.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClassifierUsage {
    pub input_tokens: u64,
    pub output_tokens: u64,
    pub cache_read_input_tokens: u64,
    pub cache_creation_input_tokens: u64,
}

/// Risk level for permission explanations.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "UPPERCASE")]
pub enum RiskLevel {
    Low,
    Medium,
    High,
}

/// Permission explanation with risk assessment.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PermissionExplanation {
    pub risk_level: RiskLevel,
    pub explanation: String,
    pub reasoning: String,
    pub risk: String,
}

/// Mapping of permission rules by their source.
pub type ToolPermissionRulesBySource = std::collections::HashMap<PermissionRuleSource, Vec<String>>;

/// Context needed for permission checking in tools.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ToolPermissionContext {
    pub mode: PermissionMode,
    pub additional_working_directories: std::collections::HashMap<String, AdditionalWorkingDirectory>,
    pub always_allow_rules: ToolPermissionRulesBySource,
    pub always_deny_rules: ToolPermissionRulesBySource,
    pub always_ask_rules: ToolPermissionRulesBySource,
    pub is_bypass_permissions_mode_available: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub is_auto_mode_available: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub stripped_dangerous_rules: Option<ToolPermissionRulesBySource>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub should_avoid_permission_prompts: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub await_automated_checks_before_dialog: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub pre_plan_mode: Option<PermissionMode>,
}

use crate::types::AdditionalWorkingDirectory;

pub fn get_empty_tool_permission_context() -> ToolPermissionContext {
    ToolPermissionContext {
        mode: PermissionMode::Default,
        additional_working_directories: std::collections::HashMap::new(),
        always_allow_rules: std::collections::HashMap::new(),
        always_deny_rules: std::collections::HashMap::new(),
        always_ask_rules: std::collections::HashMap::new(),
        is_bypass_permissions_mode_available: false,
        is_auto_mode_available: None,
        stripped_dangerous_rules: None,
        should_avoid_permission_prompts: None,
        await_automated_checks_before_dialog: None,
        pre_plan_mode: None,
    }
}
