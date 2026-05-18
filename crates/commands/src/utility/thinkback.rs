use async_trait::async_trait;

use crate::traits::{Command, CommandContext, CommandResult, CommandType};

pub struct ThinkbackCommand;

impl ThinkbackCommand {
    pub fn new() -> Self {
        Self
    }
}

impl Default for ThinkbackCommand {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl Command for ThinkbackCommand {
    fn name(&self) -> &str {
        "thinkback"
    }

    fn description(&self) -> &str {
        "Review past conversation"
    }

    fn aliases(&self) -> &[&str] {
        &[]
    }

    fn command_type(&self) -> CommandType {
        CommandType::Local
    }

    async fn execute(&self, args: &str, ctx: &CommandContext) -> CommandResult {
        let state = ctx.state.read().expect("state lock poisoned");
        let total = state.messages.len();

        match args.trim() {
            "" => {
                CommandResult::text(format!("Conversation has {} messages.\nUsage: /thinkback <search-term> or /thinkback <N> (last N messages)", total))
            }
            n if n.parse::<usize>().is_ok() => {
                let count = n.parse::<usize>().unwrap_or(5).min(total);
                let start = total.saturating_sub(count);
                let mut output = format!("Last {} messages:\n\n", count);
                for (i, msg) in state.messages.iter().enumerate().skip(start) {
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
                            cc_core::messages::Message::ToolUseSummary(_) => "ToolSummary",
                        };
                        let preview: String = text.chars().take(100).collect();
                        output.push_str(&format!("  [{}] {}: {}\n", i + 1, role, preview));
                    }
                }
                CommandResult::text(output)
            }
            term => {
                let mut results = Vec::new();
                for (i, msg) in state.messages.iter().enumerate() {
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
                        _ => None,
                    };
                    if let Some(text) = text {
                        if text.to_lowercase().contains(&term.to_lowercase()) {
                            results.push((i + 1, text.chars().take(100).collect::<String>()));
                        }
                    }
                }

                if results.is_empty() {
                    CommandResult::text(format!("No messages matching '{}'", term))
                } else {
                    let mut output = format!("Found {} matches for '{}':\n\n", results.len(), term);
                    for (idx, preview) in results.iter().take(10) {
                        output.push_str(&format!("  [{}] {}\n", idx, preview));
                    }
                    if results.len() > 10 {
                        output.push_str(&format!("\n... and {} more", results.len() - 10));
                    }
                    CommandResult::text(output)
                }
            }
        }
    }
}
