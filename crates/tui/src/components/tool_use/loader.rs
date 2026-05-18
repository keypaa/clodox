use ratatui::{
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::Paragraph,
    Frame,
};

use crate::theme::Theme;
use crate::components::box_widget::{dim_style, text_style};

/// The dot character for tool loaders.
pub const DOT_CHAR: &str = "●";

/// Render a tool use loader.
///
/// Visual format:
///   ●  (blinking)
///
/// Color: dim (unresolved), green (resolved), red (errored)
pub fn render_tool_loader(
    frame: &mut Frame,
    area: ratatui::layout::Rect,
    is_resolved: bool,
    is_error: bool,
    theme: &Theme,
) {
    let color = if is_error {
        theme.colors.error
    } else if is_resolved {
        theme.colors.success
    } else {
        theme.colors.inactive
    };

    let loader = Span::styled(DOT_CHAR, Style::default().fg(color));
    let paragraph = Paragraph::new(loader);
    frame.render_widget(paragraph, area);
}

/// Render a tool use message header.
///
/// Visual format:
///   [loader] TOOL_NAME (details)
///
/// Returns the spans for the tool name line.
pub fn tool_use_header_spans(
    tool_name: &str,
    details: Option<&str>,
    is_resolved: bool,
    is_error: bool,
    theme: &Theme,
) -> Vec<Span<'static>> {
    // Loader
    let loader_color = if is_error {
        theme.colors.error
    } else if is_resolved {
        theme.colors.success
    } else {
        theme.colors.inactive
    };

    let mut spans = vec![
        Span::styled(DOT_CHAR, Style::default().fg(loader_color)),
        Span::raw(" "),
    ];

    // Tool name (bold)
    spans.push(Span::styled(
        tool_name.to_string(),
        Style::default()
            .fg(theme.colors.text)
            .add_modifier(Modifier::BOLD),
    ));

    // Details in parentheses
    if let Some(d) = details {
        spans.push(Span::styled(
            format!(" ({})", d),
            text_style(theme.colors.text),
        ));
    }

    spans
}

/// Render a tool use status line.
///
/// Visual format:
///   <status text>  (dim)
pub fn tool_use_status_spans(status: &str, theme: &Theme) -> Line<'static> {
    Line::from(Span::styled(
        status.to_string(),
        dim_style(theme.colors.inactive),
    ))
}
