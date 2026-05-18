use async_trait::async_trait;

use crate::traits::{Command, CommandContext, CommandResult, CommandType};

pub struct HelpCommand;

impl HelpCommand {
    pub fn new() -> Self {
        Self
    }
}

impl Default for HelpCommand {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl Command for HelpCommand {
    fn name(&self) -> &str {
        "help"
    }

    fn description(&self) -> &str {
        "Show available commands and their descriptions"
    }

    fn aliases(&self) -> &[&str] {
        &["h", "?"]
    }

    fn command_type(&self) -> CommandType {
        CommandType::Local
    }

    async fn execute(&self, _args: &str, _ctx: &CommandContext) -> CommandResult {
        let mut output = String::from("Available commands:\n\n");

        let commands = vec![
            ("/help", "Show this help message"),
            ("/clear", "Clear the conversation history"),
            ("/compact", "Compact the conversation context"),
            ("/config", "Open or view configuration settings"),
            ("/login", "Authenticate with API key"),
            ("/logout", "Clear authentication state"),
            ("/resume", "Resume a previous session"),
            ("/diff", "Show git diff of changes"),
            ("/cost", "Show API cost summary"),
        ];

        let max_name_width = commands.iter().map(|(n, _)| n.len()).max().unwrap_or(0);

        for (name, desc) in &commands {
            output.push_str(&format!("  {:<width$}  {}\n", name, desc, width = max_name_width + 2));
        }

        output.push_str("\nUse /help <command> for more details on a specific command.");

        CommandResult::text(output)
    }
}
