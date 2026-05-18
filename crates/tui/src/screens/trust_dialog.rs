use ratatui::layout::Rect;
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::Widget;

use crate::theme::{Theme, Themeable};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TrustAction {
    Trust,
    Deny,
}

#[derive(Debug, Clone)]
pub struct TrustDialog {
    pub workspace_path: String,
    pub selected_action: Option<TrustAction>,
}

impl TrustDialog {
    pub fn new(workspace_path: &str) -> Self {
        Self {
            workspace_path: workspace_path.to_string(),
            selected_action: None,
        }
    }

    pub fn handle_key(&mut self, key: &str) -> Option<TrustAction> {
        match key {
            "y" => {
                self.selected_action = Some(TrustAction::Trust);
                Some(TrustAction::Trust)
            }
            "n" => {
                self.selected_action = Some(TrustAction::Deny);
                Some(TrustAction::Deny)
            }
            _ => None,
        }
    }

    fn render_lines(&self) -> Vec<Line<'static>> {
        let mut lines = Vec::new();

        let title_style = Style::default()
            .fg(ratatui::style::Color::Yellow)
            .add_modifier(Modifier::BOLD);

        lines.push(Line::from(vec![
            Span::styled("⚠ Trust this workspace?", title_style),
        ]));
        lines.push(Line::from(vec![Span::raw("")]));

        let path_style = Style::default()
            .fg(ratatui::style::Color::Cyan)
            .add_modifier(Modifier::BOLD);
        lines.push(Line::from(vec![
            Span::raw("  "),
            Span::styled(self.workspace_path.clone(), path_style),
        ]));
        lines.push(Line::from(vec![Span::raw("")]));

        let msg_style = Style::default()
            .fg(ratatui::style::Color::White)
            .add_modifier(Modifier::DIM);
        lines.push(Line::from(vec![
            Span::styled("This workspace has not been trusted before.", msg_style),
        ]));
        lines.push(Line::from(vec![
            Span::styled("Claude will have read/write access to files here.", msg_style),
        ]));
        lines.push(Line::from(vec![Span::raw("")]));

        let key_style = Style::default()
            .fg(ratatui::style::Color::Green)
            .add_modifier(Modifier::BOLD);
        let label_style = Style::default()
            .fg(ratatui::style::Color::White)
            .add_modifier(Modifier::DIM);
        let deny_style = Style::default()
            .fg(ratatui::style::Color::Red)
            .add_modifier(Modifier::BOLD);

        lines.push(Line::from(vec![
            Span::styled("[y] ", key_style),
            Span::styled("Trust", label_style),
            Span::raw("    "),
            Span::styled("[n] ", deny_style),
            Span::styled("Don't trust", label_style),
        ]));

        lines
    }
}

pub struct TrustDialogWidget<'a> {
    dialog: &'a TrustDialog,
    theme: &'a Theme,
}

impl<'a> TrustDialogWidget<'a> {
    pub fn new(dialog: &'a TrustDialog, theme: &'a Theme) -> Self {
        Self { dialog, theme }
    }
}

impl Themeable for TrustDialogWidget<'_> {
    fn render_themed(&self, area: Rect, buf: &mut ratatui::buffer::Buffer, theme: &Theme) {
        // Clear the entire screen first to remove any leftover content from previous modes
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

        let lines = self.dialog.render_lines();
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

        let border_style = Style::default().fg(ratatui::style::Color::Yellow);

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
                                if char_idx + 3 < dialog_width {
                                    let cell_x = dialog_area.x + char_idx + 2;
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

impl Widget for TrustDialogWidget<'_> {
    fn render(self, area: Rect, buf: &mut ratatui::buffer::Buffer) {
        self.render_themed(area, buf, &Theme::dark());
    }
}
