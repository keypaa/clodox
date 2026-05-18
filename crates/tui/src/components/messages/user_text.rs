use ratatui::{
    layout::{Constraint, Direction, Layout},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::Paragraph,
    Frame,
};

use crate::theme::Theme;
use crate::components::box_widget::{dim_style, text_style};

/// The dot character for messages.
/// ● (U+25CF) on Linux, ⏺ (U+23FA) on macOS
pub const DOT_CHAR: &str = "●";

/// The pointer character for user messages.
/// › (U+203A)
pub const POINTER_CHAR: &str = "›";

/// Render a user text message.
///
/// Visual format:
///   › <text>
///
/// Background: userMessageBackground
/// Pointer color: suggestion (blue) when selected, subtle otherwise
/// Text color: text (white/dark)
pub fn render_user_text(
    frame: &mut Frame,
    area: ratatui::layout::Rect,
    text: &str,
    theme: &Theme,
    is_selected: bool,
) {
    let pointer_color = if is_selected {
        Color::Blue
    } else {
        Color::DarkGray
    };

    let pointer = Span::styled(
        format!("{} ", POINTER_CHAR),
        Style::default().fg(pointer_color),
    );

    let content = Span::styled(
        text,
        text_style(theme.colors.text),
    );

    let line = Line::from(vec![pointer, content]);
    let paragraph = Paragraph::new(line).wrap(ratatui::widgets::Wrap { trim: false });
    frame.render_widget(paragraph, area);
}

/// Render a user prompt message.
///
/// Same as user text but with metadata display.
pub fn render_user_prompt(
    frame: &mut Frame,
    area: ratatui::layout::Rect,
    content: &str,
    theme: &Theme,
    is_selected: bool,
) {
    render_user_text(frame, area, content, theme, is_selected);
}

/// Render a user command message (slash command).
///
/// Visual format:
///   /<command> <args>
pub fn render_user_command(
    frame: &mut Frame,
    area: ratatui::layout::Rect,
    command: &str,
    args: &str,
    theme: &Theme,
) {
    let cmd_span = Span::styled(
        format!("/{}", command),
        Style::default()
            .fg(Color::Blue)
            .add_modifier(Modifier::BOLD),
    );

    let args_span = Span::styled(
        format!(" {}", args),
        text_style(theme.colors.text),
    );

    let line = Line::from(vec![cmd_span, args_span]);
    let paragraph = Paragraph::new(line);
    frame.render_widget(paragraph, area);
}

/// Render a user tool result message.
///
/// Dispatches to success/error/cancel/reject variants.
pub fn render_user_tool_result(
    frame: &mut Frame,
    area: ratatui::layout::Rect,
    content: &str,
    is_error: bool,
    theme: &Theme,
) {
    if is_error {
        render_tool_error(frame, area, content, theme);
    } else {
        render_tool_success(frame, area, content, theme);
    }
}

/// Render a successful tool result.
///
/// Visual format:
///   <result text>
///   ✓ Auto-approved · matched "rule"  (dim, green)
fn render_tool_success(
    frame: &mut Frame,
    area: ratatui::layout::Rect,
    content: &str,
    theme: &Theme,
) {
    let content_span = Span::styled(content, text_style(theme.colors.text));
    let line = Line::from(content_span);
    let paragraph = Paragraph::new(line).wrap(ratatui::widgets::Wrap { trim: false });
    frame.render_widget(paragraph, area);
}

/// Render a tool error.
///
/// Visual format:
///   Error: <error text>
///   … +N lines (ctrl+o to see all)  (dim)
fn render_tool_error(
    frame: &mut Frame,
    area: ratatui::layout::Rect,
    content: &str,
    theme: &Theme,
) {
    let error_text = if content.starts_with("Error: ") {
        content.to_string()
    } else {
        format!("Error: {}", content)
    };

    let error_span = Span::styled(&error_text, Style::default().fg(theme.colors.error));
    let line = Line::from(error_span);
    let paragraph = Paragraph::new(line).wrap(ratatui::widgets::Wrap { trim: false });
    frame.render_widget(paragraph, area);
}

/// Render an assistant text message.
///
/// Visual format:
///   ● <markdown content>
///
/// Dot color: text normally, suggestion when selected
/// Content rendered as plain text (markdown rendering comes later)
pub fn render_assistant_text(
    frame: &mut Frame,
    area: ratatui::layout::Rect,
    text: &str,
    theme: &Theme,
    is_selected: bool,
) {
    let dot_color = if is_selected {
        Color::Blue
    } else {
        theme.colors.text
    };

    let dot = Span::styled(DOT_CHAR, Style::default().fg(dot_color));
    let content = Span::styled(text, text_style(theme.colors.text));

    let line = Line::from(vec![dot, Span::raw(" "), content]);
    let paragraph = Paragraph::new(line).wrap(ratatui::widgets::Wrap { trim: false });
    frame.render_widget(paragraph, area);
}

/// Render an assistant tool use message.
///
/// Visual format:
///   [loader] TOOL NAME (details)
///            Running…
///
/// Loader: blinking ● (dim=unresolved, green=resolved, red=errored)
/// Tool name: bold
/// Details: plain text in parentheses
/// Status: dim, below tool line
pub fn render_assistant_tool_use(
    frame: &mut Frame,
    area: ratatui::layout::Rect,
    tool_name: &str,
    details: Option<&str>,
    status: Option<&str>,
    is_resolved: bool,
    is_error: bool,
    theme: &Theme,
) {
    let layout = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(1), // Tool name line
            Constraint::Length(1), // Status line (if any)
        ])
        .split(area);

    // Tool name line: [loader] TOOL NAME (details)
    // Loader character
    let loader_color = if is_error {
        theme.colors.error
    } else if is_resolved {
        theme.colors.success
    } else {
        theme.colors.inactive
    };

    let loader = Span::styled(DOT_CHAR, Style::default().fg(loader_color));

    // Tool name (bold)
    let name_span = Span::styled(
        tool_name,
        Style::default()
            .fg(theme.colors.text)
            .add_modifier(Modifier::BOLD),
    );

    // Build the line
    let mut spans = vec![loader, Span::raw(" "), name_span];

    if let Some(d) = details {
        let details_span = Span::styled(
            format!(" ({})", d),
            text_style(theme.colors.text),
        );
        spans.push(details_span);
    }

    let tool_line = Line::from(spans);
    let tool_paragraph = Paragraph::new(tool_line);
    frame.render_widget(tool_paragraph, layout[0]);

    // Status line (dim)
    if let Some(s) = status {
        let status_span = Span::styled(s, dim_style(theme.colors.inactive));
        let status_line = Line::from(status_span);
        let status_paragraph = Paragraph::new(status_line);
        frame.render_widget(status_paragraph, layout[1]);
    }
}

/// Render an assistant thinking message.
///
/// Collapsed (non-verbose):
///   ∴ Thinking (ctrl+o to expand)
///
/// Expanded (verbose/transcript):
///   ∴ Thinking…
///     <full thinking content>
pub fn render_assistant_thinking(
    frame: &mut Frame,
    area: ratatui::layout::Rect,
    thinking: &str,
    theme: &Theme,
    is_expanded: bool,
) {
    // Therefore sign: ∴ (U+2234)
    const THEREFORE: &str = "∴";

    if is_expanded {
        // Expanded mode: show full thinking
        let header = Span::styled(
            format!("{} Thinking…", THEREFORE),
            dim_style(theme.colors.inactive)
                .add_modifier(Modifier::ITALIC),
        );
        let content = Span::styled(
            thinking,
            dim_style(theme.colors.inactive),
        );

        let lines = vec![
            Line::from(header),
            Line::from(content),
        ];
        let paragraph = Paragraph::new(lines).wrap(ratatui::widgets::Wrap { trim: false });
        frame.render_widget(paragraph, area);
    } else {
        // Collapsed mode: just the hint
        let text = Span::styled(
            format!("{} Thinking (ctrl+o to expand)", THEREFORE),
            dim_style(theme.colors.inactive)
                .add_modifier(Modifier::ITALIC),
        );
        let paragraph = Paragraph::new(Line::from(text));
        frame.render_widget(paragraph, area);
    }
}

/// Render a system error message.
///
/// Visual format:
///   <error text>  (red)
///   Ctrl+O to expand  (dim, if truncated)
pub fn render_system_error(
    frame: &mut Frame,
    area: ratatui::layout::Rect,
    error: &str,
    theme: &Theme,
) {
    // Truncate to 1000 chars if needed
    let (display, truncated) = if error.len() > 1000 {
        (format!("{}…", &error[..999]), true)
    } else {
        (error.to_string(), false)
    };

    let error_span = Span::styled(&display, Style::default().fg(theme.colors.error));
    let mut lines = vec![Line::from(error_span)];

    if truncated {
        let hint = Span::styled(
            "(ctrl+o to see all)",
            dim_style(theme.colors.inactive),
        );
        lines.push(Line::from(hint));
    }

    let paragraph = Paragraph::new(lines).wrap(ratatui::widgets::Wrap { trim: false });
    frame.render_widget(paragraph, area);
}
