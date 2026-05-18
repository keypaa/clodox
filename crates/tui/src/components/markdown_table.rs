use ratatui::layout::Rect;
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::Widget;
use unicode_width::UnicodeWidthStr;

use crate::theme::{Theme, Themeable};

#[derive(Debug, Clone)]
pub struct MarkdownTable {
    headers: Vec<String>,
    rows: Vec<Vec<String>>,
}

impl MarkdownTable {
    pub fn new(headers: Vec<String>, rows: Vec<Vec<String>>) -> Self {
        Self { headers, rows }
    }

    pub fn from_markdown(markdown: &str) -> Self {
        let mut headers = Vec::new();
        let mut rows = Vec::new();
        let mut parsing_header = true;

        for line in markdown.lines() {
            let trimmed = line.trim();
            if trimmed.is_empty() {
                continue;
            }

            if trimmed.starts_with('|') && trimmed.ends_with('|') {
                let cells: Vec<String> = trimmed
                    .trim_matches('|')
                    .split('|')
                    .map(|c| c.trim().to_string())
                    .collect();

                if parsing_header {
                    if cells.iter().any(|c| c.chars().all(|ch| matches!(ch, '-' | ':' | ' '))) {
                        parsing_header = false;
                        continue;
                    }
                    headers = cells;
                    parsing_header = false;
                } else {
                    rows.push(cells);
                }
            }
        }

        Self { headers, rows }
    }

    fn column_widths(&self) -> Vec<usize> {
        let mut widths = vec![0; self.headers.len()];

        for (i, header) in self.headers.iter().enumerate() {
            widths[i] = widths[i].max(header.width());
        }

        for row in &self.rows {
            for (i, cell) in row.iter().enumerate() {
                if i < widths.len() {
                    widths[i] = widths[i].max(cell.width());
                }
            }
        }

        widths
    }

    fn render_row(&self, cells: &[String], widths: &[usize], theme: &Theme, is_header: bool) -> Line<'static> {
        let mut spans = Vec::new();
        spans.push(Span::raw("│"));

        for (i, cell) in cells.iter().enumerate() {
            let width = if i < widths.len() { widths[i] } else { cell.width() };
            let padded = format!(" {}{} ", cell, " ".repeat(width.saturating_sub(cell.width())));

            let style = if is_header {
                Style::default()
                    .fg(theme.colors.suggestion)
                    .add_modifier(Modifier::BOLD)
            } else {
                Style::default().fg(theme.colors.text)
            };

            spans.push(Span::styled(padded, style));
            spans.push(Span::raw("│"));
        }

        Line::from(spans)
    }

    fn render_separator(&self, widths: &[usize], theme: &Theme) -> Line<'static> {
        let mut spans = Vec::new();
        spans.push(Span::styled("├", Style::default().fg(theme.colors.inactive)));

        for (i, width) in widths.iter().enumerate() {
            let inner = "─".repeat(width + 2);
            spans.push(Span::styled(inner, Style::default().fg(theme.colors.inactive)));
            if i < widths.len() - 1 {
                spans.push(Span::styled("┼", Style::default().fg(theme.colors.inactive)));
            } else {
                spans.push(Span::styled("┤", Style::default().fg(theme.colors.inactive)));
            }
        }

        Line::from(spans)
    }

    pub fn render_lines(&self, theme: &Theme) -> Vec<Line<'static>> {
        if self.headers.is_empty() {
            return vec![];
        }

        let widths = self.column_widths();
        let mut lines = Vec::new();

        let top_border = {
            let mut spans = Vec::new();
            spans.push(Span::styled("┌", Style::default().fg(theme.colors.inactive)));
            for (i, width) in widths.iter().enumerate() {
                let inner = "─".repeat(width + 2);
                spans.push(Span::styled(inner, Style::default().fg(theme.colors.inactive)));
                if i < widths.len() - 1 {
                    spans.push(Span::styled("┬", Style::default().fg(theme.colors.inactive)));
                } else {
                    spans.push(Span::styled("┐", Style::default().fg(theme.colors.inactive)));
                }
            }
            Line::from(spans)
        };
        lines.push(top_border);

        lines.push(self.render_row(&self.headers, &widths, theme, true));
        lines.push(self.render_separator(&widths, theme));

        for row in &self.rows {
            lines.push(self.render_row(row, &widths, theme, false));
        }

        let bottom_border = {
            let mut spans = Vec::new();
            spans.push(Span::styled("└", Style::default().fg(theme.colors.inactive)));
            for (i, width) in widths.iter().enumerate() {
                let inner = "─".repeat(width + 2);
                spans.push(Span::styled(inner, Style::default().fg(theme.colors.inactive)));
                if i < widths.len() - 1 {
                    spans.push(Span::styled("┴", Style::default().fg(theme.colors.inactive)));
                } else {
                    spans.push(Span::styled("┘", Style::default().fg(theme.colors.inactive)));
                }
            }
            Line::from(spans)
        };
        lines.push(bottom_border);

        lines
    }
}

impl Themeable for MarkdownTable {
    fn render_themed(&self, area: Rect, buf: &mut ratatui::buffer::Buffer, theme: &Theme) {
        let lines = self.render_lines(theme);
        let max_lines = area.height as usize;
        let display_lines: Vec<Line> = lines.into_iter().take(max_lines).collect();

        let y_end = (area.y + area.height).min(buf.area.height);
        for (i, line) in display_lines.iter().enumerate() {
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

impl Widget for MarkdownTable {
    fn render(self, area: Rect, buf: &mut ratatui::buffer::Buffer) {
        let theme = Theme::dark();
        self.render_themed(area, buf, &theme);
    }
}
