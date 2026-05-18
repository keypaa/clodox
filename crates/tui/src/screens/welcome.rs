use ratatui::layout::Rect;
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::Widget;
use std::path::Path;

use crate::theme::{Theme, Themeable};
use crate::screens::resume::SessionEntry;

#[derive(Debug, Clone)]
pub struct WelcomeScreen {
    pub version: String,
    pub username: Option<String>,
    pub cwd: String,
    pub model_name: String,
    pub billing_type: String,
    pub recent_activity: Vec<SessionEntry>,
}

impl WelcomeScreen {
    pub fn new() -> Self {
        Self {
            version: env!("CARGO_PKG_VERSION").to_string(),
            username: None,
            cwd: std::env::current_dir()
                .ok()
                .map(|p| p.display().to_string())
                .unwrap_or_else(|| ".".to_string()),
            model_name: "claude-sonnet-4-20250514".to_string(),
            billing_type: "API Usage Billing".to_string(),
            recent_activity: Vec::new(),
        }
    }

    pub fn render_lines<'a>(&'a self, width: u16) -> Vec<Line<'a>> {
        let mut lines = Vec::new();

        let title_style = Style::default()
            .fg(ratatui::style::Color::Cyan)
            .add_modifier(Modifier::BOLD);

        let header = format!("─ Claude Code v{} ─", self.version);
        lines.push(Line::from(vec![Span::styled(header, title_style)]));

        let welcome_style = Style::default()
            .fg(ratatui::style::Color::White)
            .add_modifier(Modifier::BOLD);
        let section_style = Style::default()
            .fg(ratatui::style::Color::Cyan)
            .add_modifier(Modifier::BOLD);

        lines.push(Line::from(vec![
            Span::styled("Welcome back!", welcome_style),
            Span::raw("  │  "),
            Span::styled("Recent Activity", section_style),
        ]));

        lines.push(Line::from(vec![Span::raw("─".repeat(40))]));

        if self.recent_activity.is_empty() {
            let dim_style = Style::default()
                .fg(ratatui::style::Color::DarkGray)
                .add_modifier(Modifier::DIM);
            lines.push(Line::from(vec![
                Span::raw("  "),
                Span::styled("No recent activity", dim_style),
            ]));
        } else {
            for session in self.recent_activity.iter().take(3) {
                let dim_style = Style::default()
                    .fg(ratatui::style::Color::DarkGray)
                    .add_modifier(Modifier::DIM);
                lines.push(Line::from(vec![
                    Span::raw("  "),
                    Span::styled(format!("{} ({})", session.summary, session.time_ago()), dim_style),
                ]));
            }
        }

        lines.push(Line::from(vec![Span::raw("")]));

        let info_style = Style::default()
            .fg(ratatui::style::Color::DarkGray)
            .add_modifier(Modifier::DIM);

        lines.push(Line::from(vec![
            Span::styled(self.model_name.as_str(), info_style.clone()),
            Span::raw("  •  "),
            Span::styled(self.billing_type.as_str(), info_style.clone()),
        ]));

        lines.push(Line::from(vec![
            Span::styled(self.cwd.as_str(), info_style),
        ]));

        lines.push(Line::from(vec![Span::raw("")]));

        lines
    }
}

pub struct WelcomeWidget<'a> {
    screen: &'a WelcomeScreen,
    theme: &'a Theme,
}

impl<'a> WelcomeWidget<'a> {
    pub fn new(screen: &'a WelcomeScreen, theme: &'a Theme) -> Self {
        Self { screen, theme }
    }
}

impl Themeable for WelcomeWidget<'_> {
    fn render_themed(&self, area: Rect, buf: &mut ratatui::buffer::Buffer, theme: &Theme) {
        // Clear the entire screen first
        let bg_style = Style::default().bg(ratatui::style::Color::Black);
        for y in 0..area.height {
            for x in 0..area.width {
                let bx = area.x + x;
                let by = area.y + y;
                if bx >= buf.area.width || by >= buf.area.height {
                    continue;
                }
                if let Some(cell) = buf.cell_mut((bx, by)) {
                    cell.set_symbol(" ");
                    cell.set_style(bg_style);
                }
            }
        }

        let lines = self.screen.render_lines(area.width);
        let dialog_height = lines.len() as u16 + 2;
        let dialog_width = 60.min(area.width);

        let x = area.x + (area.width.saturating_sub(dialog_width)) / 2;
        let y = area.y + (area.height.saturating_sub(dialog_height)) / 2;

        let dialog_area = Rect {
            x,
            y,
            width: dialog_width,
            height: dialog_height,
        };

        let border_style = Style::default().fg(ratatui::style::Color::Cyan);

        // Render border
        for dx in 0..dialog_width {
            for dy in 0..dialog_height {
                let bx = dialog_area.x + dx;
                let by = dialog_area.y + dy;
                if bx >= buf.area.width || by >= buf.area.height {
                    continue;
                }

                let cell = buf.cell_mut((bx, by)).unwrap();

                if dx == 0 && dy == 0 {
                    cell.set_symbol("┌");
                    cell.set_style(border_style);
                } else if dx == dialog_width - 1 && dy == 0 {
                    cell.set_symbol("┐");
                    cell.set_style(border_style);
                } else if dx == 0 && dy == dialog_height - 1 {
                    cell.set_symbol("└");
                    cell.set_style(border_style);
                } else if dx == dialog_width - 1 && dy == dialog_height - 1 {
                    cell.set_symbol("┘");
                    cell.set_style(border_style);
                } else if dx == 0 || dx == dialog_width - 1 {
                    cell.set_symbol("│");
                    cell.set_style(border_style);
                } else if dy == 0 || dy == dialog_height - 1 {
                    cell.set_symbol("─");
                    cell.set_style(border_style);
                } else {
                    let content_idx = dy - 1;
                    if content_idx < lines.len() as u16 {
                        let line = &lines[content_idx as usize];
                        let mut char_idx = 0u16;
                        for span in &line.spans {
                            for ch in span.content.chars() {
                                if char_idx + 2 < dialog_width {
                                    let cell_x = dialog_area.x + char_idx + 1;
                                    if cell_x < buf.area.width {
                                        if let Some(c) = buf.cell_mut((cell_x, by)) {
                                            c.set_symbol(&ch.to_string());
                                            c.set_style(span.style);
                                        }
                                    }
                                }
                                char_idx += 1;
                            }
                        }
                    } else {
                        cell.set_symbol(" ");
                    }
                }
            }
        }
    }
}

impl Widget for WelcomeWidget<'_> {
    fn render(self, area: Rect, buf: &mut ratatui::buffer::Buffer) {
        self.render_themed(area, buf, &Theme::dark());
    }
}
