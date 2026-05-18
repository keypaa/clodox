use ratatui::{
    layout::{Constraint, Direction, Layout},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::Paragraph,
    Frame,
};

use crate::theme::Theme;
use crate::components::messages::user_text::{
    render_user_text, render_user_prompt, render_user_command,
    render_user_tool_result, render_assistant_text,
    render_assistant_tool_use, render_assistant_thinking,
    render_system_error,
};
use crate::components::messages::rate_limit::render_rate_limit;

/// A renderable message for the TUI.
///
/// This is a simplified representation of messages for rendering purposes.
/// The actual message types from cc-core are converted to these before rendering.
#[derive(Debug, Clone)]
pub enum RenderMessage {
    /// User text input.
    UserText { text: String },
    /// User prompt with metadata.
    UserPrompt { content: String },
    /// Slash command.
    UserCommand { command: String, args: String },
    /// Tool result.
    UserToolResult { content: String, is_error: bool },
    /// Assistant text response.
    AssistantText { text: String },
    /// Assistant tool use.
    AssistantToolUse {
        tool_name: String,
        details: Option<String>,
        status: Option<String>,
        is_resolved: bool,
        is_error: bool,
    },
    /// Assistant thinking.
    AssistantThinking { thinking: String, is_expanded: bool },
    /// System error.
    SystemError { error: String },
    /// Rate limit warning.
    RateLimit { text: String, upgrade_hint: Option<String> },
}

/// Render a message row.
///
/// Visual format:
///   ● <message content>
///
/// Dot character: ● (U+25CF) on Linux, ⏺ on macOS
/// Dot color: "text" normally, "suggestion" when selected
pub fn render_message_row(
    frame: &mut Frame,
    area: ratatui::layout::Rect,
    message: &RenderMessage,
    theme: &Theme,
    is_selected: bool,
) {
    // Layout: dot (width 2) + content
    let layout = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Length(2),
            Constraint::Min(1),
        ])
        .split(area);

    // Dot character
    let dot_char = "●";
    let dot_color = if is_selected {
        Color::Blue // suggestion color
    } else {
        theme.colors.text
    };
    let dot = Span::styled(dot_char, Style::default().fg(dot_color));
    let dot_paragraph = Paragraph::new(dot);
    frame.render_widget(dot_paragraph, layout[0]);

    // Render message content based on type
    match message {
        RenderMessage::UserText { text } => {
            render_user_text(frame, layout[1], text, theme, is_selected);
        }
        RenderMessage::UserPrompt { content } => {
            render_user_prompt(frame, layout[1], content, theme, is_selected);
        }
        RenderMessage::UserCommand { command, args } => {
            render_user_command(frame, layout[1], command, args, theme);
        }
        RenderMessage::UserToolResult { content, is_error } => {
            render_user_tool_result(frame, layout[1], content, *is_error, theme);
        }
        RenderMessage::AssistantText { text } => {
            render_assistant_text(frame, layout[1], text, theme, is_selected);
        }
        RenderMessage::AssistantToolUse {
            tool_name,
            details,
            status,
            is_resolved,
            is_error,
        } => {
            render_assistant_tool_use(
                frame,
                layout[1],
                tool_name,
                details.as_deref(),
                status.as_deref(),
                *is_resolved,
                *is_error,
                theme,
            );
        }
        RenderMessage::AssistantThinking {
            thinking,
            is_expanded,
        } => {
            render_assistant_thinking(frame, layout[1], thinking, theme, *is_expanded);
        }
        RenderMessage::SystemError { error } => {
            render_system_error(frame, layout[1], error, theme);
        }
        RenderMessage::RateLimit {
            text,
            upgrade_hint,
        } => {
            render_rate_limit(frame, layout[1], text, upgrade_hint.as_deref(), theme);
        }
    }
}

/// Render a list of messages.
pub fn render_messages(
    frame: &mut Frame,
    area: ratatui::layout::Rect,
    messages: &[RenderMessage],
    theme: &Theme,
    selected_id: Option<usize>,
) {
    let mut y = area.y;
    for (i, message) in messages.iter().enumerate() {
        if y >= area.bottom() {
            break;
        }

        // Calculate height needed for this message (rough estimate)
        let message_height = estimate_message_height(message, area.width);

        let message_area = ratatui::layout::Rect {
            x: area.x,
            y,
            width: area.width,
            height: message_height.min(area.bottom() - y),
        };

        let is_selected = selected_id.map_or(false, |id| id == i);
        render_message_row(frame, message_area, message, theme, is_selected);

        y += message_height;
    }
}

/// Estimate the height needed for a message.
fn estimate_message_height(message: &RenderMessage, width: u16) -> u16 {
    match message {
        RenderMessage::UserText { text } => {
            let lines = text.lines().count();
            let wrapped = text.len() / width as usize + 1;
            (lines.max(wrapped) as u16).max(1)
        }
        RenderMessage::UserPrompt { content } => {
            let lines = content.lines().count();
            (lines as u16).max(1)
        }
        RenderMessage::UserCommand { .. } => 1,
        RenderMessage::UserToolResult { .. } => 1,
        RenderMessage::AssistantText { text } => {
            let lines = text.lines().count();
            let wrapped = text.len() / width as usize + 1;
            (lines.max(wrapped) as u16).max(1)
        }
        RenderMessage::AssistantToolUse { status, .. } => {
            // Tool name line + optional status line
            if status.is_some() { 2 } else { 1 }
        }
        RenderMessage::AssistantThinking { is_expanded, .. } => {
            if *is_expanded { 2 } else { 1 }
        }
        RenderMessage::SystemError { error } => {
            if error.len() > 1000 { 2 } else { 1 }
        }
        RenderMessage::RateLimit { .. } => 1,
    }
}
