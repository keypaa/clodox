use ratatui::layout::Rect;
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::Widget;

use crate::theme::{Theme, Themeable};

#[derive(Debug, Clone)]
pub struct CollapsibleBlock {
    pub title: String,
    pub content: String,
    pub is_expanded: bool,
    pub max_collapsed_lines: usize,
    pub icon_expanded: String,
    pub icon_collapsed: String,
}

impl CollapsibleBlock {
    pub fn new(title: &str, content: &str) -> Self {
        Self {
            title: title.to_string(),
            content: content.to_string(),
            is_expanded: false,
            max_collapsed_lines: 3,
            icon_expanded: "▼".to_string(),
            icon_collapsed: "▶".to_string(),
        }
    }

    pub fn with_max_collapsed_lines(mut self, max: usize) -> Self {
        self.max_collapsed_lines = max;
        self
    }

    pub fn toggle(&mut self) {
        self.is_expanded = !self.is_expanded;
    }

    pub fn render_lines(&self, width: u16) -> Vec<Line<'static>> {
        let mut lines = Vec::new();

        let icon = if self.is_expanded {
            self.icon_expanded.clone()
        } else {
            self.icon_collapsed.clone()
        };

        let header_style = Style::default()
            .fg(ratatui::style::Color::Cyan)
            .add_modifier(Modifier::BOLD);

        lines.push(Line::from(vec![
            Span::styled(format!("{} ", icon), header_style),
            Span::styled(self.title.clone(), header_style),
        ]));

        if self.is_expanded {
            let content_style = Style::default()
                .fg(ratatui::style::Color::White)
                .add_modifier(Modifier::DIM);

            let content_lines: Vec<String> = self.content.lines().map(|l| l.to_string()).collect();
            let max_lines = if width < 60 { 15 } else { 25 };
            let to_show = content_lines.len().min(max_lines);

            for i in 0..to_show {
                let line = format!("  {}", content_lines[i]);
                lines.push(Line::from(vec![Span::raw(line)]));
            }

            if content_lines.len() > max_lines {
                let remaining = content_lines.len() - max_lines;
                let hint = format!("  ... +{} more lines", remaining);
                let hint_style = Style::default()
                    .fg(ratatui::style::Color::DarkGray)
                    .add_modifier(Modifier::DIM);
                lines.push(Line::from(vec![Span::raw(hint)]));
            }
        } else {
            let preview: Vec<String> = self.content.lines().take(self.max_collapsed_lines).map(|l| l.to_string()).collect();
            if !preview.is_empty() {
                let preview_style = Style::default()
                    .fg(ratatui::style::Color::DarkGray)
                    .add_modifier(Modifier::DIM);
                for line in preview {
                    let truncated = if line.len() > width as usize - 6 {
                        format!("    {}…", &line[..width as usize - 7])
                    } else {
                        format!("    {}", line)
                    };
                    lines.push(Line::from(vec![Span::styled(truncated, preview_style)]));
                }
                if self.content.lines().count() > self.max_collapsed_lines {
                    let hint = format!("    ... +{} lines (Enter to expand)", self.content.lines().count() - self.max_collapsed_lines);
                    let hint_style = Style::default()
                        .fg(ratatui::style::Color::DarkGray)
                        .add_modifier(Modifier::DIM);
                    lines.push(Line::from(vec![Span::raw(hint)]));
                }
            }
        }

        lines
    }
}

pub struct CollapsibleWidget<'a> {
    block: &'a CollapsibleBlock,
    theme: &'a Theme,
}

impl<'a> CollapsibleWidget<'a> {
    pub fn new(block: &'a CollapsibleBlock, theme: &'a Theme) -> Self {
        Self { block, theme }
    }
}

impl Themeable for CollapsibleWidget<'_> {
    fn render_themed(&self, area: Rect, buf: &mut ratatui::buffer::Buffer, _theme: &Theme) {
        let lines = self.block.render_lines(area.width);
        let y_end = (area.y + area.height).min(buf.area.height);

        for (i, line) in lines.iter().enumerate() {
            let y = area.y + i as u16;
            if y >= area.y && y < y_end {
                let mut x = area.x;
                for span in &line.spans {
                    for ch in span.content.chars() {
                        if x < area.x + area.width {
                            if let Some(cell) = buf.cell_mut((x, y)) {
                                cell.set_symbol(&ch.to_string());
                                cell.set_style(span.style);
                            }
                        }
                        x += 1;
                    }
                }
            }
        }
    }
}

impl Widget for CollapsibleWidget<'_> {
    fn render(self, area: Rect, buf: &mut ratatui::buffer::Buffer) {
        self.render_themed(area, buf, &Theme::dark());
    }
}
