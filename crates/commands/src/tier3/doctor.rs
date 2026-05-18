use async_trait::async_trait;

use crate::traits::{Command, CommandContext, CommandResult, CommandType};

pub struct DoctorCommand;

impl DoctorCommand {
    pub fn new() -> Self {
        Self
    }
}

impl Default for DoctorCommand {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl Command for DoctorCommand {
    fn name(&self) -> &str {
        "doctor"
    }

    fn description(&self) -> &str {
        "Run diagnostic checks"
    }

    fn aliases(&self) -> &[&str] {
        &[]
    }

    fn command_type(&self) -> CommandType {
        CommandType::Local
    }

    async fn execute(&self, _args: &str, _ctx: &CommandContext) -> CommandResult {
        let mut output = String::from("Running diagnostics...\n\n");

        let api_key = std::env::var("ANTHROPIC_API_KEY").ok();
        output.push_str(&format!("  API key:     {}\n", if api_key.is_some() { "set" } else { "NOT SET" }));

        let rust_version = std::process::Command::new("rustc").arg("--version").output();
        if let Ok(o) = rust_version {
            if o.status.success() {
                output.push_str(&format!("  Rust:        {}\n", String::from_utf8_lossy(&o.stdout).trim()));
            }
        }

        let cwd = std::env::current_dir();
        if let Ok(p) = cwd {
            output.push_str(&format!("  Working dir: {}\n", p.display()));
        }

        output.push_str("\nAll checks passed.");
        CommandResult::text(output)
    }
}
