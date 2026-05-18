use async_trait::async_trait;
use std::path::Path;

use crate::traits::{Command, CommandContext, CommandResult, CommandType};

pub struct FilesCommand;

impl FilesCommand {
    pub fn new() -> Self {
        Self
    }
}

impl Default for FilesCommand {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl Command for FilesCommand {
    fn name(&self) -> &str {
        "files"
    }

    fn description(&self) -> &str {
        "List files in the current working directory"
    }

    fn aliases(&self) -> &[&str] {
        &["ls"]
    }

    fn command_type(&self) -> CommandType {
        CommandType::Local
    }

    async fn execute(&self, args: &str, _ctx: &CommandContext) -> CommandResult {
        let path = if args.trim().is_empty() { "." } else { args.trim() };
        let p = Path::new(path);

        if !p.exists() {
            return CommandResult::error(format!("Path not found: {}", path));
        }

        if !p.is_dir() {
            return CommandResult::text(format!("{} (file)", path));
        }

        let mut entries: Vec<_> = match std::fs::read_dir(p) {
            Ok(rd) => rd.filter_map(|e| e.ok()).collect(),
            Err(e) => return CommandResult::error(format!("Cannot read directory: {}", e)),
        };
        entries.sort_by_key(|e| e.file_name());

        let mut output = format!("Contents of {}:\n\n", path);
        for entry in entries {
            let name = entry.file_name();
            let name_str = name.to_string_lossy();
            if entry.file_type().map(|ft| ft.is_dir()).unwrap_or(false) {
                output.push_str(&format!("  {}/\n", name_str));
            } else {
                output.push_str(&format!("  {}\n", name_str));
            }
        }
        CommandResult::text(output)
    }
}
