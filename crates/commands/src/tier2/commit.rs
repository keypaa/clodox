use async_trait::async_trait;

use crate::traits::{Command, CommandContext, CommandResult, CommandType};

pub struct CommitCommand;

impl CommitCommand {
    pub fn new() -> Self {
        Self
    }
}

impl Default for CommitCommand {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl Command for CommitCommand {
    fn name(&self) -> &str {
        "commit"
    }

    fn description(&self) -> &str {
        "Commit changes with an AI-generated message"
    }

    fn aliases(&self) -> &[&str] {
        &[]
    }

    fn command_type(&self) -> CommandType {
        CommandType::Local
    }

    async fn execute(&self, args: &str, _ctx: &CommandContext) -> CommandResult {
        let status = std::process::Command::new("git")
            .arg("status")
            .arg("--short")
            .output();

        match status {
            Ok(o) if o.status.success() => {
                let output = String::from_utf8_lossy(&o.stdout);
                if output.trim().is_empty() {
                    return CommandResult::text("Nothing to commit — working tree is clean.");
                }

                let staged = std::process::Command::new("git")
                    .arg("diff")
                    .arg("--staged")
                    .arg("--stat")
                    .output();

                let unstaged = std::process::Command::new("git")
                    .arg("diff")
                    .arg("--stat")
                    .output();

                let mut result = String::from("Changes to commit:\n\n");
                result.push_str(&output);

                if let Ok(o) = staged {
                    let stat = String::from_utf8_lossy(&o.stdout);
                    if !stat.trim().is_empty() {
                        result.push_str(&format!("\nStaged changes:\n{}\n", stat));
                    }
                }

                if let Ok(o) = unstaged {
                    let stat = String::from_utf8_lossy(&o.stdout);
                    if !stat.trim().is_empty() {
                        result.push_str(&format!("\nUnstaged changes:\n{}\n", stat));
                    }
                }

                if args.trim().is_empty() {
                    result.push_str("\nUsage: /commit <message>  (or /commit --amend)");
                    CommandResult::text(result)
                } else if args.trim() == "--amend" {
                    let commit = std::process::Command::new("git")
                        .arg("commit")
                        .arg("--amend")
                        .arg("--no-edit")
                        .output();
                    match commit {
                        Ok(o) if o.status.success() => CommandResult::text("Amended last commit."),
                        Ok(o) => CommandResult::error(format!("Git error: {}", String::from_utf8_lossy(&o.stderr))),
                        Err(e) => CommandResult::error(format!("Failed to run git: {}", e)),
                    }
                } else {
                    let commit = std::process::Command::new("git")
                        .arg("commit")
                        .arg("-m")
                        .arg(args.trim())
                        .output();
                    match commit {
                        Ok(o) if o.status.success() => {
                            let stdout = String::from_utf8_lossy(&o.stdout);
                            CommandResult::text(format!("Committed:\n{}", stdout.trim()))
                        }
                        Ok(o) => CommandResult::error(format!("Git error: {}", String::from_utf8_lossy(&o.stderr))),
                        Err(e) => CommandResult::error(format!("Failed to run git: {}", e)),
                    }
                }
            }
            _ => CommandResult::error("Not a git repository"),
        }
    }
}
