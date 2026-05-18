use ratatui::{
    style::Style,
    text::{Line, Span},
    widgets::Paragraph,
    Frame,
};

use crate::theme::Theme;

/// Render a rate limit message.
///
/// Visual format:
///   Rate limit warning text  (red)
///   Upgrade hint  (dim)
pub fn render_rate_limit(
    frame: &mut Frame,
    area: ratatui::layout::Rect,
    text: &str,
    upgrade_hint: Option<&str>,
    theme: &Theme,
) {
    let mut spans = vec![
        Span::styled(text, Style::default().fg(theme.colors.error)),
    ];

    if let Some(hint) = upgrade_hint {
        spans.push(Span::raw(" · "));
        spans.push(Span::styled(
            hint,
            Style::default()
                .fg(theme.colors.inactive)
                .add_modifier(ratatui::style::Modifier::DIM),
        ));
    }

    let paragraph = Paragraph::new(Line::from(spans));
    frame.render_widget(paragraph, area);
}
