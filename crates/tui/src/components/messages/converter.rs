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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_extract_tool_display_name_core_tools() {
        assert_eq!(extract_tool_display_name("bash"), "Bash");
        assert_eq!(extract_tool_display_name("Bash"), "Bash");
        assert_eq!(extract_tool_display_name("read"), "Read");
        assert_eq!(extract_tool_display_name("write"), "Write");
        assert_eq!(extract_tool_display_name("edit"), "Edit");
        assert_eq!(extract_tool_display_name("grep"), "Grep");
        assert_eq!(extract_tool_display_name("glob"), "Glob");
    }

    #[test]
    fn test_extract_tool_display_name_advanced_tools() {
        assert_eq!(extract_tool_display_name("web_fetch"), "WebFetch");
        assert_eq!(extract_tool_display_name("WebFetch"), "WebFetch");
        assert_eq!(extract_tool_display_name("web_search"), "WebSearch");
        assert_eq!(extract_tool_display_name("WebSearch"), "WebSearch");
        assert_eq!(extract_tool_display_name("agent"), "Agent");
        assert_eq!(extract_tool_display_name("Agent"), "Agent");
        assert_eq!(extract_tool_display_name("Task"), "Agent");
    }

    #[test]
    fn test_extract_tool_display_name_unknown() {
        assert_eq!(extract_tool_display_name("UnknownTool"), "UnknownTool");
    }

    #[test]
    fn test_extract_tool_details_bash() {
        let input = serde_json::json!({"command": "ls -la"});
        assert_eq!(extract_tool_details("bash", &input), Some("ls -la".to_string()));
    }

    #[test]
    fn test_extract_tool_details_bash_truncated() {
        let long_cmd = "a".repeat(200);
        let input = serde_json::json!({"command": long_cmd});
        let details = extract_tool_details("bash", &input).unwrap();
        assert!(details.chars().count() == 160);
        assert!(details.ends_with("…"));
    }

    #[test]
    fn test_extract_tool_details_read() {
        let input = serde_json::json!({"file_path": "/path/to/file.rs"});
        assert_eq!(extract_tool_details("read", &input), Some("/path/to/file.rs".to_string()));
    }

    #[test]
    fn test_extract_tool_details_read_alt_path() {
        let input = serde_json::json!({"path": "/path/to/file.rs"});
        assert_eq!(extract_tool_details("read", &input), Some("/path/to/file.rs".to_string()));
    }

    #[test]
    fn test_extract_tool_details_write() {
        let input = serde_json::json!({"file_path": "/path/to/output.txt"});
        assert_eq!(extract_tool_details("write", &input), Some("/path/to/output.txt".to_string()));
    }

    #[test]
    fn test_extract_tool_details_grep() {
        let input = serde_json::json!({"pattern": "fn main"});
        assert_eq!(extract_tool_details("grep", &input), Some("fn main".to_string()));
    }

    #[test]
    fn test_extract_tool_details_glob() {
        let input = serde_json::json!({"pattern": "**/*.rs"});
        assert_eq!(extract_tool_details("glob", &input), Some("**/*.rs".to_string()));
    }

    #[test]
    fn test_extract_tool_details_web_fetch() {
        let input = serde_json::json!({"url": "https://docs.python.org/3/library/os.html"});
        assert_eq!(extract_tool_details("web_fetch", &input), Some("https://docs.python.org/3/library/os.html".to_string()));
    }

    #[test]
    fn test_extract_tool_details_web_fetch_truncated() {
        let long_url = format!("https://example.com/{}", "a".repeat(200));
        let input = serde_json::json!({"url": long_url});
        let details = extract_tool_details("web_fetch", &input).unwrap();
        assert!(details.chars().count() == 120);
        assert!(details.ends_with("…"));
    }

    #[test]
    fn test_extract_tool_details_web_search() {
        let input = serde_json::json!({"query": "Rust async programming"});
        assert_eq!(extract_tool_details("web_search", &input), Some("Rust async programming".to_string()));
    }

    #[test]
    fn test_extract_tool_details_agent_with_type() {
        let input = serde_json::json!({
            "description": "Review the code",
            "subagent_type": "code-reviewer"
        });
        assert_eq!(extract_tool_details("agent", &input), Some("Review the code (code-reviewer)".to_string()));
    }

    #[test]
    fn test_extract_tool_details_agent_without_type() {
        let input = serde_json::json!({"description": "Do some work"});
        assert_eq!(extract_tool_details("agent", &input), Some("Do some work".to_string()));
    }

    #[test]
    fn test_extract_tool_details_agent_default_description() {
        let input = serde_json::json!({});
        assert_eq!(extract_tool_details("agent", &input), Some("agent".to_string()));
    }

    #[test]
    fn test_extract_tool_details_unknown_tool() {
        let input = serde_json::json!({"key": "value"});
        assert_eq!(extract_tool_details("UnknownTool", &input), None);
    }

    #[test]
    fn test_core_message_to_render_message_user_text() {
        let msg = cc_core::messages::Message::User(cc_core::messages::UserMessage {
            id: uuid::Uuid::new_v4(),
            content: vec![cc_core::messages::ContentBlockParam::Text { text: "Hello".to_string() }],
            timestamp: chrono::Utc::now(),
            is_meta: None,
            origin_query_source: None,
            effort: None,
        });
        let rendered = core_message_to_render_message(&msg);
        match rendered {
            RenderMessage::UserText { text } => assert_eq!(text, "Hello"),
            _ => panic!("Expected UserText"),
        }
    }

    #[test]
    fn test_core_message_to_render_message_assistant_text() {
        let msg = cc_core::messages::Message::Assistant(cc_core::messages::AssistantMessage {
            id: uuid::Uuid::new_v4(),
            content: vec![cc_core::messages::ContentBlockParam::Text { text: "Hi there".to_string() }],
            timestamp: chrono::Utc::now(),
            model: None,
            usage: None,
            stop_reason: None,
            is_meta: None,
            agent_id: None,
        });
        let rendered = core_message_to_render_message(&msg);
        match rendered {
            RenderMessage::AssistantText { text } => assert_eq!(text, "Hi there"),
            _ => panic!("Expected AssistantText"),
        }
    }

    #[test]
    fn test_core_message_to_render_message_assistant_tool_use() {
        let msg = cc_core::messages::Message::Assistant(cc_core::messages::AssistantMessage {
            id: uuid::Uuid::new_v4(),
            content: vec![cc_core::messages::ContentBlockParam::ToolUse {
                name: "bash".to_string(),
                input: serde_json::json!({"command": "ls"}),
                id: "tool-1".to_string(),
            }],
            timestamp: chrono::Utc::now(),
            model: None,
            usage: None,
            stop_reason: None,
            is_meta: None,
            agent_id: None,
        });
        let rendered = core_message_to_render_message(&msg);
        match rendered {
            RenderMessage::AssistantToolUse { tool_name, details, .. } => {
                assert_eq!(tool_name, "Bash");
                assert_eq!(details, Some("ls".to_string()));
            }
            _ => panic!("Expected AssistantToolUse"),
        }
    }

    #[test]
    fn test_core_message_to_render_message_system_error() {
        let msg = cc_core::messages::Message::System(cc_core::messages::SystemMessage::ApiError(
            cc_core::messages::SystemApiErrorMessage {
                id: uuid::Uuid::new_v4(),
                error: "API error".to_string(),
                timestamp: chrono::Utc::now(),
            },
        ));
        let rendered = core_message_to_render_message(&msg);
        match rendered {
            RenderMessage::SystemError { error } => assert_eq!(error, "API error"),
            _ => panic!("Expected SystemError"),
        }
    }

    #[test]
    fn test_core_message_to_render_message_system_info() {
        let msg = cc_core::messages::Message::System(cc_core::messages::SystemMessage::Informational(
            cc_core::messages::SystemInformationalMessage {
                id: uuid::Uuid::new_v4(),
                text: "Info".to_string(),
                level: None,
                timestamp: chrono::Utc::now(),
            },
        ));
        let rendered = core_message_to_render_message(&msg);
        match rendered {
            RenderMessage::SystemError { error } => assert_eq!(error, "Info"),
            _ => panic!("Expected SystemError"),
        }
    }
}

