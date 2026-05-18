use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Paragraph, Widget, Wrap},
};

/// Helper to build a styled paragraph.
pub fn styled_paragraph(text: &str, style: Style, wrap: bool) -> Paragraph {
    let mut p = Paragraph::new(Span::styled(text, style));
    if wrap {
        p = p.wrap(Wrap { trim: false });
    }
    p
}

/// Layout helper — split an area into vertical sections.
pub fn vertical_layout(area: Rect, constraints: &[Constraint]) -> Vec<Rect> {
    Layout::default()
        .direction(Direction::Vertical)
        .constraints(constraints.to_vec())
        .split(area)
        .to_vec()
}

/// Layout helper — split an area into horizontal sections.
pub fn horizontal_layout(area: Rect, constraints: &[Constraint]) -> Vec<Rect> {
    Layout::default()
        .direction(Direction::Horizontal)
        .constraints(constraints.to_vec())
        .split(area)
        .to_vec()
}

/// Apply margin to a rect.
pub fn apply_margin(area: Rect, margin: u16) -> Rect {
    Rect {
        x: area.x + margin,
        y: area.y + margin,
        width: area.width.saturating_sub(margin * 2),
        height: area.height.saturating_sub(margin * 2),
    }
}

/// Apply padding to a rect (inset).
pub fn apply_padding(area: Rect, padding: u16) -> Rect {
    apply_margin(area, padding)
}

/// Create a text span with the given style.
pub fn span(text: impl Into<String>, style: Style) -> Span<'static> {
    Span::styled(text.into(), style)
}

/// Create a line from multiple spans.
pub fn line(spans: Vec<Span<'static>>) -> Line<'static> {
    Line::from(spans)
}

/// Create a simple style.
pub fn style(fg: Option<Color>, bg: Option<Color>, modifiers: Modifier) -> Style {
    let mut s = Style::default();
    if let Some(c) = fg {
        s = s.fg(c);
    }
    if let Some(c) = bg {
        s = s.bg(c);
    }
    s = s.add_modifier(modifiers);
    s
}

/// Dim style.
pub fn dim_style(fg: Color) -> Style {
    style(Some(fg), None, Modifier::DIM)
}

/// Bold style.
pub fn bold_style(fg: Color) -> Style {
    style(Some(fg), None, Modifier::BOLD)
}

/// Default text style.
pub fn text_style(fg: Color) -> Style {
    style(Some(fg), None, Modifier::empty())
}
