use async_trait::async_trait;

use crate::traits::{Command, CommandContext, CommandResult, CommandType};

pub struct StickersCommand;

impl StickersCommand {
    pub fn new() -> Self {
        Self
    }
}

impl Default for StickersCommand {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl Command for StickersCommand {
    fn name(&self) -> &str {
        "stickers"
    }

    fn description(&self) -> &str {
        "Show available stickers"
    }

    fn aliases(&self) -> &[&str] {
        &[]
    }

    fn command_type(&self) -> CommandType {
        CommandType::Local
    }

    async fn execute(&self, _args: &str, _ctx: &CommandContext) -> CommandResult {
        let output = r#"Available stickers:

  🦀  Rustacean
  🚀  Rocket
  💡  Idea
  ✅  Done
  ❌  Error
  🔧  Fix
  📝  Note
  🎯  Target

(Stickers are decorative — they don't affect functionality)"#;
        CommandResult::text(output.to_string())
    }
}
