use async_trait::async_trait;

use crate::traits::{Command, CommandContext, CommandResult, CommandType};

pub struct InitCommand;

impl InitCommand {
    pub fn new() -> Self {
        Self
    }
}

impl Default for InitCommand {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl Command for InitCommand {
    fn name(&self) -> &str {
        "init"
    }

    fn description(&self) -> &str {
        "Initialize Claude Code in a project"
    }

    fn aliases(&self) -> &[&str] {
        &[]
    }

    fn command_type(&self) -> CommandType {
        CommandType::Local
    }

    async fn execute(&self, _args: &str, _ctx: &CommandContext) -> CommandResult {
        let cwd = std::env::current_dir().unwrap_or_default();
        let claude_md = cwd.join("CLAUDE.md");
        let settings = cwd.join(".claude").join("settings.json");

        let mut output = String::from("Project initialization:\n\n");
        output.push_str(&format!("  Working directory: {}\n", cwd.display()));
        output.push_str(&format!("  CLAUDE.md:         {}\n", if claude_md.exists() { "exists" } else { "not found" }));
        output.push_str(&format!("  .claude/settings:  {}\n", if settings.exists() { "exists" } else { "not found" }));
        output.push_str("\nUse /init to create missing config files.\n(Init not yet implemented)");
        CommandResult::text(output)
    }
}
