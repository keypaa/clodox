use async_trait::async_trait;

use crate::traits::{Command, CommandContext, CommandResult, CommandType};

pub struct BranchCommand;

impl BranchCommand {
    pub fn new() -> Self {
        Self
    }
}

impl Default for BranchCommand {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl Command for BranchCommand {
    fn name(&self) -> &str {
        "branch"
    }

    fn description(&self) -> &str {
        "Create or switch git branches"
    }

    fn aliases(&self) -> &[&str] {
        &[]
    }

    fn command_type(&self) -> CommandType {
        CommandType::Local
    }

    async fn execute(&self, args: &str, _ctx: &CommandContext) -> CommandResult {
        let output = std::process::Command::new("git")
            .arg("branch")
            .arg("--show-current")
            .output();

        match output {
            Ok(o) if o.status.success() => {
                let current = String::from_utf8_lossy(&o.stdout).trim().to_string();
                if args.trim().is_empty() {
                    CommandResult::text(format!("Current branch: {}", current))
                } else {
                    let create = std::process::Command::new("git")
                        .arg("checkout")
                        .arg("-b")
                        .arg(args.trim())
                        .output();
                    match create {
                        Ok(o) if o.status.success() => CommandResult::text(format!("Created and switched to branch: {}", args.trim())),
                        Ok(o) => CommandResult::error(format!("Git error: {}", String::from_utf8_lossy(&o.stderr))),
                        Err(e) => CommandResult::error(format!("Failed to run git: {}", e)),
                    }
                }
            }
            _ => CommandResult::error("Not a git repository"),
        }
    }
}
