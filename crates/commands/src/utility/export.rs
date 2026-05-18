use async_trait::async_trait;

use crate::traits::{Command, CommandContext, CommandResult, CommandType};

pub struct ExportCommand;

impl ExportCommand {
    pub fn new() -> Self {
        Self
    }
}

impl Default for ExportCommand {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl Command for ExportCommand {
    fn name(&self) -> &str {
        "export"
    }

    fn description(&self) -> &str {
        "Export conversation to a file"
    }

    fn aliases(&self) -> &[&str] {
        &[]
    }

    fn command_type(&self) -> CommandType {
        CommandType::Local
    }

    async fn execute(&self, args: &str, ctx: &CommandContext) -> CommandResult {
        let state = ctx.state.read().expect("state lock poisoned");
        let msg_count = state.messages.len();

        if args.trim().is_empty() {
            CommandResult::text(format!("Usage: /export <filename.md>\n\nCurrent conversation has {} messages.", msg_count))
        } else {
            let path = std::path::Path::new(args.trim());
            let mut content = String::from("# Conversation Export\n\n");
            for msg in &state.messages {
                let text = match msg {
                    cc_core::messages::Message::User(u) => {
                        u.content.iter().filter_map(|b| {
                            if let cc_core::messages::ContentBlockParam::Text { text } = b {
                                Some(text.clone())
                            } else {
                                None
                            }
                        }).next()
                    }
                    cc_core::messages::Message::Assistant(a) => {
                        a.content.iter().filter_map(|b| {
                            if let cc_core::messages::ContentBlockParam::Text { text } = b {
                                Some(text.clone())
                            } else {
                                None
                            }
                        }).next()
                    }
                    cc_core::messages::Message::Attachment(att) => {
                        let files: Vec<_> = att.attachments.iter().filter_map(|a| a.path.as_ref()).collect();
                        Some(format!("[Attachments: {}]", files.iter().map(|p| p.as_str()).collect::<Vec<_>>().join(", ")))
                    }
                    cc_core::messages::Message::Progress(p) => {
                        Some(format!("[Progress: tool={}]", p.tool_use_id))
                    }
                    _ => None,
                };
                if let Some(text) = text {
                    let role = match msg {
                        cc_core::messages::Message::User(_) => "User",
                        cc_core::messages::Message::Assistant(_) => "Assistant",
                        cc_core::messages::Message::Attachment(_) => "Attachment",
                        cc_core::messages::Message::Progress(_) => "Progress",
                        cc_core::messages::Message::System(_) => "System",
                        cc_core::messages::Message::Tombstone(_) => "Tombstone",
                        cc_core::messages::Message::ToolUseSummary(_) => "Tool Summary",
                    };
                    content.push_str(&format!("## {}\n\n{}\n\n", role, text));
                }
            }

            match std::fs::write(path, &content) {
                Ok(()) => CommandResult::text(format!("Exported {} messages to {}", msg_count, path.display())),
                Err(e) => CommandResult::error(format!("Failed to write file: {}", e)),
            }
        }
    }
}
