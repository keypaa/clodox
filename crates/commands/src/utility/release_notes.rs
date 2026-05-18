use async_trait::async_trait;

use crate::traits::{Command, CommandContext, CommandResult, CommandType};

pub struct ReleaseNotesCommand;

impl ReleaseNotesCommand {
    pub fn new() -> Self {
        Self
    }
}

impl Default for ReleaseNotesCommand {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl Command for ReleaseNotesCommand {
    fn name(&self) -> &str {
        "release_notes"
    }

    fn description(&self) -> &str {
        "Show release notes"
    }

    fn aliases(&self) -> &[&str] {
        &["changelog"]
    }

    fn command_type(&self) -> CommandType {
        CommandType::Local
    }

    async fn execute(&self, _args: &str, _ctx: &CommandContext) -> CommandResult {
        let output = r#"Release Notes — claude-code-rs v0.1.0

  Initial Rust port of Claude Code.

  Features implemented:
  - Full TUI with ratatui + crossterm
  - Command system with 62+ slash commands
  - Query engine with streaming
  - Tool execution (Bash, Read, Write, Edit, Grep, Glob)
  - Permission system with risk assessment
  - Session lifecycle management
  - Token/cost tracking

  Features in progress:
  - Service layer (MCP, API, compact)
  - Bridge system
  - Missing tools (web_fetch, web_search, agent)

  Known gaps:
  - REPL (requires Node.js vm module)
  - Voice mode
  - Agent swarms
  - Computer use"#;
        CommandResult::text(output.to_string())
    }
}
