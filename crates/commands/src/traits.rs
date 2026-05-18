use std::sync::Arc;

use async_trait::async_trait;
use cc_core::messages::ContentBlockParam;
use cc_core::state::AppState;
use cc_query::compaction::CompactionResult;

/// Auth/provider types for command availability filtering.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum AuthType {
    ClaudeAi,
    Console,
}

/// Command execution type.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CommandType {
    Local,
    Prompt,
    Tui,
}

/// How to display command results.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum CommandResultDisplay {
    System,
    #[default]
    User,
    Skip,
}

/// Result from command execution.
pub enum CommandResult {
    Text {
        message: String,
        display: CommandResultDisplay,
    },
    Compact {
        compaction_result: CompactionResult,
        display_text: Option<String>,
    },
    Skip,
    Prompt {
        content: Vec<ContentBlockParam>,
        allowed_tools: Option<Vec<String>>,
    },
    Navigate {
        screen: String,
    },
    Error {
        message: String,
    },
}

impl CommandResult {
    pub fn text(message: impl Into<String>) -> Self {
        Self::Text {
            message: message.into(),
            display: CommandResultDisplay::User,
        }
    }

    pub fn system(message: impl Into<String>) -> Self {
        Self::Text {
            message: message.into(),
            display: CommandResultDisplay::System,
        }
    }

    pub fn skip() -> Self {
        Self::Skip
    }

    pub fn error(message: impl Into<String>) -> Self {
        Self::Error {
            message: message.into(),
        }
    }
}

/// Shared state type used by commands.
pub type SharedState = Arc<std::sync::RwLock<AppState>>;

/// Command context passed to command execution.
pub struct CommandContext {
    pub state: SharedState,
    pub abort_signal: tokio_util::sync::CancellationToken,
}

impl CommandContext {
    pub fn new(state: SharedState) -> Self {
        Self {
            state,
            abort_signal: tokio_util::sync::CancellationToken::new(),
        }
    }

    pub fn cancel(&self) {
        self.abort_signal.cancel();
    }

    pub fn is_cancelled(&self) -> bool {
        self.abort_signal.is_cancelled()
    }
}

/// Command trait — all commands implement this.
#[async_trait]
pub trait Command: Send + Sync {
    fn name(&self) -> &str;
    fn description(&self) -> &str;
    fn aliases(&self) -> &[&str] { &[] }
    fn argument_hint(&self) -> Option<&str> { None }
    fn availability(&self) -> &[AuthType] { &[] }
    fn is_enabled(&self) -> bool { true }
    fn is_hidden(&self) -> bool { false }
    fn command_type(&self) -> CommandType;

    async fn execute(&self, args: &str, ctx: &CommandContext) -> CommandResult;
}

/// Command availability check.
pub fn meets_availability(command: &dyn Command, user_auth: Option<&AuthType>) -> bool {
    let available = command.availability();
    if available.is_empty() {
        return true;
    }
    match user_auth {
        Some(auth) => available.contains(auth),
        None => false,
    }
}

/// Resolve the user-visible name for a command.
pub fn get_command_name(cmd: &dyn Command) -> &str {
    cmd.name()
}

/// Check if a command is enabled.
pub fn is_command_enabled(cmd: &dyn Command) -> bool {
    cmd.is_enabled()
}
