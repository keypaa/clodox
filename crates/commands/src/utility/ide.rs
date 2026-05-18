use async_trait::async_trait;

use crate::traits::{Command, CommandContext, CommandResult, CommandType};

pub struct IdeCommand;

impl IdeCommand {
    pub fn new() -> Self {
        Self
    }
}

impl Default for IdeCommand {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl Command for IdeCommand {
    fn name(&self) -> &str {
        "ide"
    }

    fn description(&self) -> &str {
        "IDE integration settings"
    }

    fn aliases(&self) -> &[&str] {
        &[]
    }

    fn command_type(&self) -> CommandType {
        CommandType::Local
    }

    async fn execute(&self, _args: &str, _ctx: &CommandContext) -> CommandResult {
        CommandResult::text("IDE integration status:\n\n  No IDE connected.\n\nSupported IDEs: VS Code, JetBrains, Neovim\n(IDE integration not yet implemented)")
    }
}
