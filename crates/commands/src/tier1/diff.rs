use async_trait::async_trait;
use std::process::Command as ProcessCommand;

use crate::traits::{Command, CommandContext, CommandResult, CommandType};

pub struct DiffCommand;

impl DiffCommand {
    pub fn new() -> Self {
        Self
    }
}

impl Default for DiffCommand {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl Command for DiffCommand {
    fn name(&self) -> &str {
        "diff"
    }

    fn description(&self) -> &str {
        "Show git diff of changes made in the current session"
    }

    fn aliases(&self) -> &[&str] {
        &["changes"]
    }

    fn command_type(&self) -> CommandType {
        CommandType::Local
    }

    async fn execute(&self, args: &str, _ctx: &CommandContext) -> CommandResult {
        let staged = args.trim() == "--staged" || args.trim() == "-s";

        let output = if staged {
            ProcessCommand::new("git")
                .args(["diff", "--staged"])
                .output()
        } else {
            ProcessCommand::new("git")
                .args(["diff"])
                .output()
        };

        match output {
            Ok(result) => {
                let stdout = String::from_utf8_lossy(&result.stdout);
                let stderr = String::from_utf8_lossy(&result.stderr);

                if stdout.is_empty() && stderr.is_empty() {
                    CommandResult::text("No changes to show. Working tree is clean.")
                } else if !stdout.is_empty() {
                    CommandResult::text(stdout.to_string())
                } else {
                    CommandResult::error(stderr.to_string())
                }
            }
            Err(e) => CommandResult::error(format!("Failed to run git diff: {}", e)),
        }
    }
}
