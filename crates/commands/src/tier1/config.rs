use async_trait::async_trait;
use std::path::PathBuf;

use crate::traits::{Command, CommandContext, CommandResult, CommandType};

pub struct ConfigCommand;

impl ConfigCommand {
    pub fn new() -> Self {
        Self
    }
}

impl Default for ConfigCommand {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl Command for ConfigCommand {
    fn name(&self) -> &str {
        "config"
    }

    fn description(&self) -> &str {
        "View or open configuration settings"
    }

    fn aliases(&self) -> &[&str] {
        &["settings", "cfg"]
    }

    fn command_type(&self) -> CommandType {
        CommandType::Local
    }

    async fn execute(&self, args: &str, ctx: &CommandContext) -> CommandResult {
        let state = ctx.state.read().expect("state lock poisoned");

        let config_dir = get_config_dir();
        let config_path = config_dir.join("settings.json");

        let model = state.main_loop_model.name.clone();
        let thinking = state.thinking_enabled;
        let fast_mode = state.fast_mode;
        let effort = format!("{:?}", state.effort);
        let vim_mode = state.vim_mode;
        let brief_mode = state.brief_mode;
        let transcript_mode = state.transcript_mode;
        drop(state);

        match args.trim() {
            "open" | "edit" => {
                if config_path.exists() {
                    CommandResult::text(format!("Config file: {}", config_path.display()))
                } else {
                    CommandResult::text(format!("No config file found at {}", config_path.display()))
                }
            }
            _ => {
                let mut output = String::from("Current settings:\n\n");
                output.push_str(&format!("  Model:          {}\n", model));
                output.push_str(&format!("  Thinking:       {}\n", thinking));
                output.push_str(&format!("  Fast mode:      {}\n", fast_mode));
                output.push_str(&format!("  Effort:         {}\n", effort));
                output.push_str(&format!("  Vim mode:       {}\n", vim_mode));
                output.push_str(&format!("  Brief mode:     {}\n", brief_mode));
                output.push_str(&format!("  Transcript:     {}\n", transcript_mode));
                output.push_str(&format!("\nConfig file:    {}\n", config_path.display()));
                output.push_str("\nUse /config open to view the config file.");

                CommandResult::text(output)
            }
        }
    }
}

fn get_config_dir() -> PathBuf {
    let mut path = dirs::home_dir().unwrap_or_else(|| PathBuf::from("."));
    path.push(".claude");
    path
}
