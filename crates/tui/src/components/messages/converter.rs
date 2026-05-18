use crate::components::messages::row::RenderMessage;

/// Extract a display-friendly tool name.
pub fn extract_tool_display_name(name: &str) -> String {
    match name {
        "bash" | "Bash" => "Bash".to_string(),
        "sandboxed_bash" | "SandboxedBash" => "SandboxedBash".to_string(),
        "read" | "Read" => "Read".to_string(),
        "write" | "Write" => "Write".to_string(),
        "edit" | "Edit" => "Edit".to_string(),
        "grep" | "Grep" => "Grep".to_string(),
        "glob" | "Glob" => "Glob".to_string(),
        "web_fetch" | "WebFetch" => "WebFetch".to_string(),
        "web_search" | "WebSearch" => "WebSearch".to_string(),
        "agent" | "Agent" | "Task" => "Agent".to_string(),
        _ => name.to_string(),
    }
}

/// Extract display details from tool input based on tool type.
pub fn extract_tool_details(name: &str, input: &serde_json::Value) -> Option<String> {
    match name {
        "bash" | "Bash" | "sandboxed_bash" | "SandboxedBash" => {
            input.get("command").and_then(|v| v.as_str()).map(|cmd| {
                if cmd.len() > 160 {
                    format!("{}…", &cmd[..159])
                } else {
                    cmd.to_string()
                }
            })
        }
        "read" | "Read" => {
            input.get("file_path").or_else(|| input.get("path"))
                .and_then(|v| v.as_str()).map(|p| p.to_string())
        }
        "write" | "Write" => {
            input.get("file_path").and_then(|v| v.as_str()).map(|p| p.to_string())
        }
        "edit" | "Edit" => {
            input.get("file_path").and_then(|v| v.as_str()).map(|p| p.to_string())
        }
        "grep" | "Grep" => {
            input.get("pattern").and_then(|v| v.as_str()).map(|p| p.to_string())
        }
        "glob" | "Glob" => {
            input.get("pattern").and_then(|v| v.as_str()).map(|p| p.to_string())
        }
        "web_fetch" | "WebFetch" => {
            input.get("url").and_then(|v| v.as_str()).map(|url| {
                if url.len() > 120 {
                    format!("{}…", &url[..119])
                } else {
                    url.to_string()
                }
            })
        }
        "web_search" | "WebSearch" => {
            input.get("query").and_then(|v| v.as_str()).map(|q| q.to_string())
        }
        "agent" | "Agent" | "Task" => {
            let desc = input.get("description").and_then(|v| v.as_str()).unwrap_or("agent");
            let agent_type = input.get("subagent_type").and_then(|v| v.as_str());
            match agent_type {
                Some(atype) => Some(format!("{desc} ({atype})")),
                None => Some(desc.to_string()),
            }
        }
        _ => None,
    }
}

/// Convert a core message to a RenderMessage with proper tool details.
pub fn core_message_to_render_message(msg: &cc_core::messages::Message) -> RenderMessage {
    use cc_core::messages::Message;

    match msg {
        Message::User(user_msg) => {
            let text = user_msg.content.iter()
                .filter_map(|block| match block {
                    cc_core::messages::ContentBlockParam::Text { text } => Some(text.clone()),
                    _ => None,
                })
                .collect::<Vec<_>>()
                .join("\n");

            RenderMessage::UserText { text }
        }
        Message::Assistant(assistant_msg) => {
            let text = assistant_msg.content.iter()
                .filter_map(|block| match block {
                    cc_core::messages::ContentBlockParam::Text { text } => Some(text.clone()),
                    _ => None,
                })
                .collect::<Vec<_>>()
                .join("\n");

            if !text.is_empty() {
                RenderMessage::AssistantText { text }
            } else {
                let tool_use = assistant_msg.content.iter()
                    .find_map(|block| match block {
                        cc_core::messages::ContentBlockParam::ToolUse { name, input, .. } => {
                            Some((name.clone(), input.clone()))
                        }
                        _ => None,
                    });

                match tool_use {
                    Some((name, input)) => {
                        let details = extract_tool_details(&name, &input);
                        let display_name = extract_tool_display_name(&name);
                        RenderMessage::AssistantToolUse {
                            tool_name: display_name,
                            details,
                            status: Some("Running…".to_string()),
                            is_resolved: false,
                            is_error: false,
                            output: None,
                            is_expanded: false,
                            duration_ms: None,
                        }
                    }
                    None => RenderMessage::AssistantText { text: String::new() },
                }
            }
        }
        Message::System(system_msg) => {
            let text = match system_msg {
                cc_core::messages::SystemMessage::Informational(msg) => msg.text.clone(),
                cc_core::messages::SystemMessage::ApiError(msg) => msg.error.clone(),
                _ => String::new(),
            };

            RenderMessage::SystemError { error: text }
        }
        _ => RenderMessage::AssistantText { text: String::new() },
    }
}
