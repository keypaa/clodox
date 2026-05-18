use std::sync::Arc;

use async_trait::async_trait;
use serde::{Deserialize, Serialize};

use crate::permissions::ToolPermissionContext;
use crate::tools::ToolUseContext;
use crate::types::{EffortValue, SettingSource};

/// Result from a local command execution.
#[derive(Debug, Clone)]
pub enum LocalCommandResult {
    Text { value: String },
    Compact {
        compaction_result: CompactionResult,
        display_text: Option<String>,
    },
    Skip,
}

/// Compaction result from a command.
#[derive(Debug, Clone)]
pub struct CompactionResult {
    pub tokens_before: u64,
    pub tokens_after: u64,
    pub messages_compacted: usize,
}

/// A prompt command expands to LLM prompt content.
#[derive(Debug, Clone)]
pub struct PromptCommand {
    pub name: String,
    pub description: String,
    pub progress_message: String,
    pub content_length: usize,
    pub arg_names: Option<Vec<String>>,
    pub allowed_tools: Option<Vec<String>>,
    pub model: Option<String>,
    pub source: SettingSource,
    pub disable_non_interactive: bool,
    pub hooks: Option<HooksSettings>,
    pub skill_root: Option<String>,
    pub context: CommandContext,
    pub agent: Option<String>,
    pub effort: Option<EffortValue>,
    pub paths: Option<Vec<String>>,
}

#[async_trait]
pub trait PromptCommandImpl: Send + Sync {
    async fn get_prompt(
        &self,
        args: &str,
        context: &ToolUseContext,
    ) -> anyhow::Result<Vec<crate::messages::ContentBlockParam>>;
}

/// Execution context for a command.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum CommandContext {
    #[default]
    Inline,
    Fork,
}

/// Hooks settings for commands/skills.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HooksSettings {
    pub pre_compact: Option<Vec<HookConfig>>,
    pub post_compact: Option<Vec<HookConfig>>,
    pub session_start: Option<Vec<HookConfig>>,
    pub post_sampling: Option<Vec<HookConfig>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HookConfig {
    pub name: String,
    pub command: String,
}

/// Command availability per auth/provider environment.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum CommandAvailability {
    ClaudeAi,
    Console,
}

/// Base properties shared by all command types.
#[derive(Clone)]
pub struct CommandBase {
    pub availability: Option<Vec<CommandAvailability>>,
    pub description: String,
    pub has_user_specified_description: bool,
    pub is_enabled: Option<Arc<dyn Fn() -> bool + Send + Sync>>,
    pub is_hidden: bool,
    pub name: String,
    pub aliases: Vec<String>,
    pub is_mcp: bool,
    pub argument_hint: Option<String>,
    pub when_to_use: Option<String>,
    pub version: Option<String>,
    pub disable_model_invocation: bool,
    pub user_invocable: bool,
    pub loaded_from: Option<CommandLoadedFrom>,
    pub kind: Option<CommandKind>,
    pub immediate: bool,
    pub is_sensitive: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum CommandLoadedFrom {
    CommandsDeprecated,
    Skills,
    Plugin,
    Managed,
    Bundled,
    Mcp,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum CommandKind {
    Workflow,
}

/// Command result display option.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum CommandResultDisplay {
    Skip,
    System,
    #[default]
    User,
}

/// Callback options for command completion.
#[derive(Debug, Clone, Default)]
pub struct CommandOnDoneOptions {
    pub display: CommandResultDisplay,
    pub should_query: bool,
    pub meta_messages: Vec<String>,
    pub next_input: Option<String>,
    pub submit_next_input: bool,
}

/// Resume entrypoint for session restoration.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum ResumeEntrypoint {
    CliFlag,
    SlashCommandPicker,
    SlashCommandSessionId,
    SlashCommandTitle,
    Fork,
}

/// Local JSX command context.
pub struct LocalJsxCommandContext {
    pub tool_use_context: ToolUseContext,
    pub options: LocalJsxCommandOptions,
    pub tool_permission_context: ToolPermissionContext,
}

pub struct LocalJsxCommandOptions {
    pub dynamic_mcp_config: Option<serde_json::Value>,
    pub ide_installation_status: Option<IdeExtensionInstallationStatus>,
    pub theme: crate::types::ThemeName,
}

#[derive(Debug, Clone)]
pub struct IdeExtensionInstallationStatus {
    pub vscode: Option<bool>,
    pub jetbrains: Option<bool>,
}

/// Union of all command types.
#[derive(Debug, Clone)]
pub enum Command {
    Prompt(PromptCommand),
    // Local and LocalJsx commands would have their own impls
    // For now, we define the trait-based approach
}

impl std::fmt::Debug for CommandBase {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("CommandBase")
            .field("name", &self.name)
            .field("description", &self.description)
            .field("aliases", &self.aliases)
            .finish_non_exhaustive()
    }
}

impl Command {
    pub fn name(&self) -> &str {
        match self {
            Command::Prompt(cmd) => &cmd.name,
        }
    }

    pub fn description(&self) -> &str {
        match self {
            Command::Prompt(cmd) => &cmd.description,
        }
    }

    pub fn is_enabled(&self) -> bool {
        match self {
            Command::Prompt(_cmd) => true,
        }
    }

    pub fn aliases(&self) -> &[String] {
        &[]
    }
}

/// Resolves the user-visible name for a command.
pub fn get_command_name(base: &CommandBase) -> String {
    base.name.clone()
}

/// Resolves whether the command is enabled.
pub fn is_command_enabled(base: &CommandBase) -> bool {
    base.is_enabled.as_ref().map(|f| f()).unwrap_or(true)
}
