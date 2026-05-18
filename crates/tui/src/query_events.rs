use cc_core::messages::{ContentBlockDelta, Message, StreamEvent};
use cc_core::state::{QueryState, TurnTokenCounts};
use cc_core::permissions::{PermissionMode, RiskLevel, ToolPermissionContext};
use cc_query::engine::QueryEvent;
use cc_query::errors::QueryError;

use crate::components::messages::row::RenderMessage;
use crate::components::permissions::dialog::PermissionDialog;
use crate::state::SharedState;

/// Streaming accumulator — builds complete messages from incremental deltas.
pub struct StreamingAccumulator {
    current_text: String,
    current_thinking: String,
    current_tool_id: Option<String>,
    current_tool_name: Option<String>,
    current_tool_json: String,
    finalized_messages: Vec<RenderMessage>,
}

impl StreamingAccumulator {
    pub fn new() -> Self {
        Self {
            current_text: String::new(),
            current_thinking: String::new(),
            current_tool_id: None,
            current_tool_name: None,
            current_tool_json: String::new(),
            finalized_messages: Vec::new(),
        }
    }

    pub fn apply_stream_event(&mut self, event: &StreamEvent) {
        match event {
            StreamEvent::MessageStart { .. } => {
                self.reset();
            }
            StreamEvent::ContentBlockDelta { delta, .. } => match delta {
                ContentBlockDelta::TextDelta { text } => {
                    self.current_text.push_str(text);
                }
                ContentBlockDelta::ThinkingDelta { thinking } => {
                    self.current_thinking.push_str(thinking);
                }
                ContentBlockDelta::InputJsonDelta { partial_json } => {
                    self.current_tool_json.push_str(partial_json);
                }
            },
            StreamEvent::ContentBlockStart { content_block, .. } => {
                if let cc_core::messages::ContentBlockParam::ToolUse { id, name, .. } = content_block {
                    self.finalize_current_blocks();
                    self.current_tool_id = Some(id.clone());
                    self.current_tool_name = Some(name.clone());
                    self.current_tool_json.clear();
                }
            }
            StreamEvent::ContentBlockStop { .. } => {
                self.finalize_current_blocks();
            }
            StreamEvent::MessageDelta { usage, .. } => {
                if usage.input_tokens > 0 || usage.output_tokens > 0 {
                    self.finalize_current_blocks();
                }
            }
            StreamEvent::MessageStop => {
                self.finalize_current_blocks();
            }
            _ => {}
        }
    }

    pub fn take_messages(&mut self) -> Vec<RenderMessage> {
        std::mem::take(&mut self.finalized_messages)
    }

    pub fn has_streaming_content(&self) -> bool {
        !self.current_text.is_empty() || !self.current_thinking.is_empty()
    }

    pub fn streaming_text(&self) -> Option<&str> {
        if self.current_text.is_empty() {
            None
        } else {
            Some(&self.current_text)
        }
    }

    pub fn streaming_thinking(&self) -> Option<&str> {
        if self.current_thinking.is_empty() {
            None
        } else {
            Some(&self.current_thinking)
        }
    }

    pub fn reset(&mut self) {
        self.finalize_current_blocks();
    }

    fn finalize_current_blocks(&mut self) {
        if !self.current_thinking.is_empty() {
            self.finalized_messages.push(RenderMessage::AssistantThinking {
                thinking: std::mem::take(&mut self.current_thinking),
                is_expanded: false,
            });
        }

        if !self.current_text.is_empty() {
            self.finalized_messages.push(RenderMessage::AssistantText {
                text: std::mem::take(&mut self.current_text),
            });
        }

        if let (Some(_id), Some(name)) = (self.current_tool_id.take(), self.current_tool_name.take()) {
            let details = if self.current_tool_json.is_empty() {
                None
            } else {
                Some(self.current_tool_json.clone())
            };
            self.finalized_messages.push(RenderMessage::AssistantToolUse {
                tool_name: name,
                details,
                status: Some("Running…".to_string()),
                is_resolved: false,
                is_error: false,
                output: None,
                is_expanded: false,
                duration_ms: None,
            });
            self.current_tool_json.clear();
        }
    }

    pub fn push_tool_result(&mut self, tool_name: String, success: bool, output: String) {
        self.finalized_messages.push(RenderMessage::ToolResultInline {
            tool_name,
            success,
            output_preview: output.chars().take(200).collect(),
        });
    }

    pub fn update_last_tool_status(&mut self, status: Option<String>, is_resolved: bool, is_error: bool, output: Option<String>, duration_ms: Option<u64>) {
        for msg in self.finalized_messages.iter_mut().rev() {
            if let RenderMessage::AssistantToolUse {
                status: s,
                is_resolved: r,
                is_error: e,
                output: o,
                duration_ms: d,
                ..
            } = msg {
                if let Some(st) = status { *s = Some(st); }
                *r = is_resolved;
                *e = is_error;
                *o = output;
                *d = duration_ms;
                break;
            }
        }
    }
}

impl Default for StreamingAccumulator {
    fn default() -> Self {
        Self::new()
    }
}

/// Process a query event and update application state.
pub fn handle_query_event(
    event_result: Result<QueryEvent, QueryError>,
    state: &SharedState,
    accumulator: &mut StreamingAccumulator,
    permission_mode: PermissionMode,
    _permission_context: &ToolPermissionContext,
) {
    match event_result {
        Ok(event) => match event {
            QueryEvent::Stream(core_event) => {
                accumulator.apply_stream_event(&core_event);

                match &core_event {
                    StreamEvent::MessageStart { .. } => {
                        let mut s = state.write().expect("state lock poisoned");
                        s.query_state = QueryState::Streaming;
                    }
                    StreamEvent::MessageDelta { usage, .. } => {
                        let mut s = state.write().expect("state lock poisoned");
                        s.token_counts.input_tokens += usage.input_tokens;
                        s.token_counts.output_tokens += usage.output_tokens;
                        if let Some(cr) = usage.cache_read_input_tokens {
                            s.token_counts.cache_read_tokens += cr;
                        }
                        if let Some(cc) = usage.cache_creation_input_tokens {
                            s.token_counts.cache_creation_tokens += cc;
                        }
                        s.current_turn_tokens = Some(TurnTokenCounts {
                            input_tokens: usage.input_tokens,
                            output_tokens: usage.output_tokens,
                            cache_read_tokens: usage.cache_read_input_tokens.unwrap_or(0),
                            cache_creation_tokens: usage.cache_creation_input_tokens.unwrap_or(0),
                            cost_usd: estimate_cost(usage),
                        });
                    }
                    _ => {}
                }

                let new_messages = accumulator.take_messages();
                if !new_messages.is_empty() {
                    let mut s = state.write().expect("state lock poisoned");
                    s.messages.extend(new_messages.into_iter().map(|rm| {
                        Message::User(cc_core::messages::UserMessage {
                            id: uuid::Uuid::new_v4(),
                            content: vec![cc_core::messages::ContentBlockParam::Text {
                                text: render_message_to_text(&rm),
                            }],
                            timestamp: chrono::Utc::now(),
                            is_meta: None,
                            origin_query_source: None,
                            effort: None,
                        })
                    }));
                }
            }
            QueryEvent::TurnComplete { message } => {
                let new_messages = accumulator.take_messages();
                let mut s = state.write().expect("state lock poisoned");
                s.messages.push(Message::Assistant(message));
                for rm in new_messages {
                    s.messages.push(Message::User(cc_core::messages::UserMessage {
                        id: uuid::Uuid::new_v4(),
                        content: vec![cc_core::messages::ContentBlockParam::Text {
                            text: render_message_to_text(&rm),
                        }],
                        timestamp: chrono::Utc::now(),
                        is_meta: None,
                        origin_query_source: None,
                        effort: None,
                    }));
                }
                s.query_state = QueryState::Idle;
                s.is_querying = false;
                s.streaming_text = None;
                s.streaming_thinking = None;
                s.streaming_tool_json.clear();
            }
            QueryEvent::ToolCallsPending { message, tool_calls } => {
                let new_messages = accumulator.take_messages();
                let mut s = state.write().expect("state lock poisoned");
                s.messages.push(Message::Assistant(message));
                for rm in new_messages {
                    s.messages.push(Message::User(cc_core::messages::UserMessage {
                        id: uuid::Uuid::new_v4(),
                        content: vec![cc_core::messages::ContentBlockParam::Text {
                            text: render_message_to_text(&rm),
                        }],
                        timestamp: chrono::Utc::now(),
                        is_meta: None,
                        origin_query_source: None,
                        effort: None,
                    }));
                }
                s.query_state = QueryState::ToolRunning;
                s.pending_tool_calls = tool_calls.iter().map(|tc| {
                    let display_text = format_tool_call_display(&tc.name, &tc.input);
                    cc_core::state::PendingToolCall {
                        id: tc.id.clone(),
                        name: tc.name.clone(),
                        input: tc.input.clone(),
                        display_text,
                    }
                }).collect();

                if permission_mode == PermissionMode::BypassPermissions || permission_mode == PermissionMode::Auto {
                    return;
                }

                if permission_mode == PermissionMode::Plan || permission_mode == PermissionMode::DontAsk {
                    s.pending_permission_dialog = None;
                    return;
                }

                let needs_approval = tool_calls.iter().find(|tc| {
                    is_tool_call_risky(&tc.name, &tc.input)
                });

                if let Some(tc) = needs_approval {
                    let risk = assess_tool_risk(&tc.name, &tc.input);
                    let _dialog = PermissionDialog::new(
                        &format!("Allow {}?", tc.name),
                        &format_tool_permission_message(&tc.name, &tc.input),
                        &format_tool_call_display(&tc.name, &tc.input),
                        risk,
                    );
                    s.pending_permission_dialog = Some(cc_core::state::PermissionDialogState {
                        tool_name: tc.name.clone(),
                        tool_input: tc.input.clone(),
                        tool_display: format_tool_call_display(&tc.name, &tc.input),
                        tool_call_id: tc.id.clone(),
                    });
                }
            }
            QueryEvent::ToolResult { tool_call_id, tool_name, success } => {
                let mut s = state.write().expect("state lock poisoned");
                s.pending_tool_calls.retain(|tc| tc.id != tool_call_id);
                if s.pending_tool_calls.is_empty() {
                    s.query_state = QueryState::Streaming;
                }
                let result_text = if success {
                    format!("Tool call {tool_call_id} succeeded.")
                } else {
                    format!("Tool call {tool_call_id} failed.")
                };
                s.messages.push(Message::User(cc_core::messages::UserMessage {
                    id: uuid::Uuid::new_v4(),
                    content: vec![cc_core::messages::ContentBlockParam::Text {
                        text: result_text.clone(),
                    }],
                    timestamp: chrono::Utc::now(),
                    is_meta: None,
                    origin_query_source: None,
                    effort: None,
                }));

                accumulator.push_tool_result(tool_name, success, result_text);
            }
            QueryEvent::MaxTokensReached { message } => {
                let mut s = state.write().expect("state lock poisoned");
                s.messages.push(Message::Assistant(message));
                s.query_state = QueryState::Idle;
                s.is_querying = false;
            }
            QueryEvent::Aborted => {
                let mut s = state.write().expect("state lock poisoned");
                s.query_state = QueryState::Idle;
                s.is_querying = false;
                s.messages.push(Message::User(cc_core::messages::UserMessage {
                    id: uuid::Uuid::new_v4(),
                    content: vec![cc_core::messages::ContentBlockParam::Text {
                        text: "Query cancelled.".to_string(),
                    }],
                    timestamp: chrono::Utc::now(),
                    is_meta: None,
                    origin_query_source: None,
                    effort: None,
                }));
            }
        },
        Err(e) => {
            let mut s = state.write().expect("state lock poisoned");
            s.query_state = QueryState::Error;
            s.is_querying = false;
            s.query_error = Some(format!("{e}"));
        }
    }
}

fn estimate_cost(usage: &cc_core::messages::Usage) -> f64 {
    let input_cost = (usage.input_tokens as f64) * 3.0 / 1_000_000.0;
    let output_cost = (usage.output_tokens as f64) * 15.0 / 1_000_000.0;
    let cache_read = (usage.cache_read_input_tokens.unwrap_or(0) as f64) * 0.3 / 1_000_000.0;
    let cache_creation = (usage.cache_creation_input_tokens.unwrap_or(0) as f64) * 3.75 / 1_000_000.0;
    input_cost + output_cost + cache_read + cache_creation
}

fn format_tool_call_display(name: &str, input: &serde_json::Value) -> String {
    match name {
        "bash" | "Bash" => {
            if let Some(cmd) = input.get("command").and_then(|v| v.as_str()) {
                format!("bash: {cmd}")
            } else {
                "bash".to_string()
            }
        }
        "read" | "Read" => {
            if let Some(path) = input.get("file_path").and_then(|v| v.as_str()) {
                format!("read: {path}")
            } else if let Some(path) = input.get("path").and_then(|v| v.as_str()) {
                format!("read: {path}")
            } else {
                "read".to_string()
            }
        }
        "write" | "Write" => {
            if let Some(path) = input.get("file_path").and_then(|v| v.as_str()) {
                format!("write: {path}")
            } else {
                "write".to_string()
            }
        }
        "edit" | "Edit" => {
            if let Some(path) = input.get("file_path").and_then(|v| v.as_str()) {
                format!("edit: {path}")
            } else {
                "edit".to_string()
            }
        }
        "grep" | "Grep" => {
            if let Some(pattern) = input.get("pattern").and_then(|v| v.as_str()) {
                format!("grep: {pattern}")
            } else {
                "grep".to_string()
            }
        }
        "glob" | "Glob" => {
            if let Some(pattern) = input.get("pattern").and_then(|v| v.as_str()) {
                format!("glob: {pattern}")
            } else {
                "glob".to_string()
            }
        }
        "web_fetch" | "WebFetch" => {
            if let Some(url) = input.get("url").and_then(|v| v.as_str()) {
                format!("web_fetch: {url}")
            } else {
                "web_fetch".to_string()
            }
        }
        "web_search" | "WebSearch" => {
            if let Some(query) = input.get("query").and_then(|v| v.as_str()) {
                format!("web_search: {query}")
            } else {
                "web_search".to_string()
            }
        }
        "agent" | "Agent" | "Task" => {
            let desc = input.get("description").and_then(|v| v.as_str()).unwrap_or("agent");
            let agent_type = input.get("subagent_type").and_then(|v| v.as_str());
            match agent_type {
                Some(atype) => format!("agent ({atype}): {desc}"),
                None => format!("agent: {desc}"),
            }
        }
        _ => name.to_string(),
    }
}

fn render_message_to_text(rm: &RenderMessage) -> String {
    match rm {
        RenderMessage::UserText { text } => text.clone(),
        RenderMessage::UserPrompt { content } => content.clone(),
        RenderMessage::UserCommand { command, args } => {
            if args.is_empty() {
                format!("/{command}")
            } else {
                format!("/{command} {args}")
            }
        }
        RenderMessage::UserToolResult { content, is_error } => {
            if *is_error {
                format!("[Tool Error] {content}")
            } else {
                format!("[Tool Result] {content}")
            }
        }
        RenderMessage::AssistantText { text } => text.clone(),
        RenderMessage::AssistantToolUse { tool_name, details, status, .. } => {
            let detail = details.as_deref().unwrap_or("");
            let st = status.as_deref().unwrap_or("");
            format!("[{tool_name}] {detail} {st}")
        }
        RenderMessage::AssistantThinking { thinking, .. } => {
            format!("[Thinking] {thinking}")
        }
        RenderMessage::SystemError { error } => {
            format!("[Error] {error}")
        }
        RenderMessage::RateLimit { text, .. } => {
            format!("[Rate Limit] {text}")
        }
        RenderMessage::AssistantToolUseStreaming { tool_name, partial_json } => {
            format!("[{tool_name}] {partial_json}")
        }
        RenderMessage::ToolResultInline { tool_name, success, output_preview } => {
            let status = if *success { "OK" } else { "FAIL" };
            format!("[{tool_name} {status}] {output_preview}")
        }
    }
}

fn is_tool_call_risky(name: &str, input: &serde_json::Value) -> bool {
    match name {
        "bash" => {
            if let Some(cmd) = input.get("command").and_then(|v| v.as_str()) {
                let trimmed = cmd.trim();
                let dangerous_prefixes = [
                    "rm ", "sudo ", "chmod ", "chown ", "dd ", "mkfs ",
                    "curl ", "wget ", "nc ", "ncat ", "ssh ", "scp ",
                    "eval ", "exec ", "source ", ".", "bash -c", "sh -c",
                    "python ", "python3 ", "node ", "ruby ", "perl ",
                    "pip ", "npm ", "cargo ", "go ",
                ];
                for prefix in &dangerous_prefixes {
                    if trimmed.starts_with(prefix) {
                        return true;
                    }
                }
                let dangerous_exact = ["reboot", "shutdown", "halt", "poweroff"];
                dangerous_exact.contains(&trimmed)
            } else {
                false
            }
        }
        "write" | "edit" => true,
        "grep" | "glob" | "read" => false,
        _ => true,
    }
}

fn assess_tool_risk(name: &str, input: &serde_json::Value) -> RiskLevel {
    match name {
        "bash" => {
            if let Some(cmd) = input.get("command").and_then(|v| v.as_str()) {
                let trimmed = cmd.trim();
                if trimmed.starts_with("rm ") || trimmed.starts_with("sudo ") || trimmed.starts_with("chmod ") {
                    return RiskLevel::High;
                }
                if trimmed.starts_with("git ") || trimmed.starts_with("ls ") || trimmed.starts_with("cat ") {
                    return RiskLevel::Low;
                }
                RiskLevel::Medium
            } else {
                RiskLevel::Medium
            }
        }
        "write" => RiskLevel::Medium,
        "edit" => RiskLevel::Medium,
        "read" | "grep" | "glob" => RiskLevel::Low,
        _ => RiskLevel::Medium,
    }
}

fn format_tool_permission_message(name: &str, input: &serde_json::Value) -> String {
    match name {
        "bash" => {
            if let Some(cmd) = input.get("command").and_then(|v| v.as_str()) {
                format!("Execute shell command:\n  {}", cmd)
            } else {
                "Execute shell command".to_string()
            }
        }
        "write" => {
            if let Some(path) = input.get("file_path").and_then(|v| v.as_str()) {
                format!("Write to file:\n  {}", path)
            } else {
                "Write to file".to_string()
            }
        }
        "edit" => {
            if let Some(path) = input.get("file_path").and_then(|v| v.as_str()) {
                format!("Edit file:\n  {}", path)
            } else {
                "Edit file".to_string()
            }
        }
        "read" => {
            if let Some(path) = input.get("file_path").and_then(|v| v.as_str()) {
                format!("Read file:\n  {}", path)
            } else {
                "Read file".to_string()
            }
        }
        "grep" => {
            if let Some(pattern) = input.get("pattern").and_then(|v| v.as_str()) {
                format!("Search for pattern:\n  {}", pattern)
            } else {
                "Search files".to_string()
            }
        }
        "glob" => {
            if let Some(pattern) = input.get("pattern").and_then(|v| v.as_str()) {
                format!("Find files matching:\n  {}", pattern)
            } else {
                "Find files".to_string()
            }
        }
        _ => format!("Use tool: {}", name),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_format_tool_call_display_bash() {
        let input = serde_json::json!({"command": "ls -la"});
        assert_eq!(format_tool_call_display("bash", &input), "bash: ls -la");
    }

    #[test]
    fn test_format_tool_call_display_read() {
        let input = serde_json::json!({"file_path": "src/main.rs"});
        assert_eq!(format_tool_call_display("read", &input), "read: src/main.rs");
    }

    #[test]
    fn test_format_tool_call_display_write() {
        let input = serde_json::json!({"file_path": "output.txt"});
        assert_eq!(format_tool_call_display("write", &input), "write: output.txt");
    }

    #[test]
    fn test_format_tool_call_display_edit() {
        let input = serde_json::json!({"file_path": "config.json"});
        assert_eq!(format_tool_call_display("edit", &input), "edit: config.json");
    }

    #[test]
    fn test_format_tool_call_display_grep() {
        let input = serde_json::json!({"pattern": "fn main"});
        assert_eq!(format_tool_call_display("grep", &input), "grep: fn main");
    }

    #[test]
    fn test_format_tool_call_display_glob() {
        let input = serde_json::json!({"pattern": "**/*.rs"});
        assert_eq!(format_tool_call_display("glob", &input), "glob: **/*.rs");
    }

    #[test]
    fn test_format_tool_call_display_web_fetch() {
        let input = serde_json::json!({"url": "https://example.com"});
        assert_eq!(format_tool_call_display("web_fetch", &input), "web_fetch: https://example.com");
    }

    #[test]
    fn test_format_tool_call_display_web_search() {
        let input = serde_json::json!({"query": "Rust programming"});
        assert_eq!(format_tool_call_display("web_search", &input), "web_search: Rust programming");
    }

    #[test]
    fn test_format_tool_call_display_agent_with_type() {
        let input = serde_json::json!({
            "description": "Review code",
            "subagent_type": "code-reviewer"
        });
        assert_eq!(format_tool_call_display("agent", &input), "agent (code-reviewer): Review code");
    }

    #[test]
    fn test_format_tool_call_display_agent_without_type() {
        let input = serde_json::json!({"description": "Do work"});
        assert_eq!(format_tool_call_display("agent", &input), "agent: Do work");
    }

    #[test]
    fn test_format_tool_call_display_task_alias() {
        let input = serde_json::json!({"description": "Task work"});
        assert_eq!(format_tool_call_display("Task", &input), "agent: Task work");
    }

    #[test]
    fn test_format_tool_call_display_unknown_tool() {
        let input = serde_json::json!({});
        assert_eq!(format_tool_call_display("UnknownTool", &input), "UnknownTool");
    }

    #[test]
    fn test_format_tool_call_display_missing_field() {
        let input = serde_json::json!({});
        assert_eq!(format_tool_call_display("bash", &input), "bash");
        assert_eq!(format_tool_call_display("read", &input), "read");
        assert_eq!(format_tool_call_display("web_fetch", &input), "web_fetch");
    }
}

