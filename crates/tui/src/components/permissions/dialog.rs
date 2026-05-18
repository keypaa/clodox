use ratatui::layout::{Constraint, Direction, Layout, Rect};
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::Widget;

use crate::theme::{Theme, Themeable};
use cc_core::permissions::{PermissionMode, RiskLevel};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PermissionAction {
    AllowOnce,
    AllowSession,
    Deny,
    Cancel,
}

#[derive(Debug, Clone)]
pub struct PermissionDialog {
    pub title: String,
    pub message: String,
    pub preview: String,
    pub risk_level: RiskLevel,
    pub show_allow_session: bool,
    pub selected_action: Option<PermissionAction>,
    pub max_preview_lines: usize,
}

impl PermissionDialog {
    pub fn new(title: &str, message: &str, preview: &str, risk_level: RiskLevel) -> Self {
        Self {
            title: title.to_string(),
            message: message.to_string(),
            preview: preview.to_string(),
            risk_level,
            show_allow_session: true,
            selected_action: None,
            max_preview_lines: 8,
        }
    }

    pub fn with_allow_session(mut self, show: bool) -> Self {
        self.show_allow_session = show;
        self
    }

    pub fn with_max_preview_lines(mut self, max: usize) -> Self {
        self.max_preview_lines = max;
        self
    }

    pub fn handle_key(&mut self, key: &str) -> Option<PermissionAction> {
        match key {
            "y" => {
                self.selected_action = Some(PermissionAction::AllowOnce);
                Some(PermissionAction::AllowOnce)
            }
            "a" if self.show_allow_session => {
                self.selected_action = Some(PermissionAction::AllowSession);
                Some(PermissionAction::AllowSession)
            }
            "n" => {
                self.selected_action = Some(PermissionAction::Deny);
                Some(PermissionAction::Deny)
            }
            "esc" => {
                self.selected_action = Some(PermissionAction::Cancel);
                Some(PermissionAction::Cancel)
            }
            _ => None,
        }
    }

    pub fn risk_color(&self) -> ratatui::style::Color {
        match self.risk_level {
            RiskLevel::Low => ratatui::style::Color::Green,
            RiskLevel::Medium => ratatui::style::Color::Yellow,
            RiskLevel::High => ratatui::style::Color::Red,
        }
    }

    pub fn risk_label(&self) -> &str {
        match self.risk_level {
            RiskLevel::Low => "Low",
            RiskLevel::Medium => "Medium",
            RiskLevel::High => "High",
        }
    }

    fn render_lines(&self, width: u16) -> Vec<Line<'static>> {
        let mut lines = Vec::new();

        let risk_color = self.risk_color();
        let title_style = Style::default()
            .fg(risk_color)
            .add_modifier(Modifier::BOLD);

        let warning_icon = match self.risk_level {
            RiskLevel::Low => "ℹ",
            RiskLevel::Medium => "⚠",
            RiskLevel::High => "⛔",
        };

        lines.push(Line::from(vec![
            Span::styled(format!(" {} ", warning_icon), title_style),
            Span::styled(self.title.clone(), title_style),
        ]));

        lines.push(Line::from(vec![Span::raw("")]));

        let msg_style = Style::default().fg(ratatui::style::Color::White);
        for msg_line in self.message.lines() {
            lines.push(Line::from(vec![Span::styled(msg_line.to_string(), msg_style)]));
        }

        if !self.preview.is_empty() {
            lines.push(Line::from(vec![Span::raw("")]));

            let preview_style = Style::default()
                .fg(ratatui::style::Color::Cyan)
                .add_modifier(Modifier::DIM);

            let preview_lines: Vec<&str> = self.preview.lines().collect();
            let to_show = preview_lines.len().min(self.max_preview_lines);

            for i in 0..to_show {
                let line = format!("  {}", preview_lines[i]);
                lines.push(Line::from(vec![Span::styled(line, preview_style)]));
            }

            if preview_lines.len() > self.max_preview_lines {
                let remaining = preview_lines.len() - self.max_preview_lines;
                let hint = format!("  ... +{} lines (ctrl+o to see all)", remaining);
                let hint_style = Style::default()
                    .fg(ratatui::style::Color::DarkGray)
                    .add_modifier(Modifier::DIM);
                lines.push(Line::from(vec![Span::styled(hint, hint_style)]));
            }
        }

        lines.push(Line::from(vec![Span::raw("")]));

        let risk_style = Style::default()
            .fg(risk_color)
            .add_modifier(Modifier::BOLD);
        lines.push(Line::from(vec![
            Span::styled("Risk: ", Style::default().fg(ratatui::style::Color::DarkGray)),
            Span::styled(self.risk_label().to_string(), risk_style),
        ]));

        lines.push(Line::from(vec![Span::raw("")]));

        let key_style = Style::default()
            .fg(ratatui::style::Color::Green)
            .add_modifier(Modifier::BOLD);
        let label_style = Style::default()
            .fg(ratatui::style::Color::White)
            .add_modifier(Modifier::DIM);

        let mut button_spans = vec![
            Span::styled("[y] ", key_style),
            Span::styled("Allow once  ", label_style),
        ];

        if self.show_allow_session {
            button_spans.push(Span::styled("[a] ", key_style));
            button_spans.push(Span::styled("Allow session  ", label_style));
        }

        button_spans.push(Span::styled("[n] ", Style::default().fg(ratatui::style::Color::Red).add_modifier(Modifier::BOLD)));
        button_spans.push(Span::styled("Deny  ", label_style));
        button_spans.push(Span::styled("[esc] ", Style::default().fg(ratatui::style::Color::DarkGray).add_modifier(Modifier::BOLD)));
        button_spans.push(Span::styled("Cancel", label_style));

        lines.push(Line::from(button_spans));

        lines
    }
}

pub struct PermissionDialogWidget<'a> {
    dialog: &'a PermissionDialog,
    theme: &'a Theme,
    area: Rect,
}

impl<'a> PermissionDialogWidget<'a> {
    pub fn new(dialog: &'a PermissionDialog, theme: &'a Theme, area: Rect) -> Self {
        Self { dialog, theme, area }
    }
}

impl Themeable for PermissionDialogWidget<'_> {
    fn render_themed(&self, area: Rect, buf: &mut ratatui::buffer::Buffer, theme: &Theme) {
        let lines = self.dialog.render_lines(area.width);

        let dialog_height = lines.len() as u16 + 4;
        let dialog_width = area.width.min(80);

        let x = area.x + (area.width.saturating_sub(dialog_width)) / 2;
        let y = area.y + (area.height.saturating_sub(dialog_height)) / 2;

        let dialog_area = Rect {
            x,
            y,
            width: dialog_width,
            height: dialog_height,
        };

        let border_style = Style::default().fg(self.dialog.risk_color());

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
                    let content_idx = dy - 2;
                    if content_idx < lines.len() as u16 {
                        let line = &lines[content_idx as usize];
                        let char_idx = dx - 2;
                        if char_idx < line.spans.iter().map(|s| s.content.chars().count()).sum::<usize>() as u16 {
                            let mut current_char = 0;
                            for span in &line.spans {
                                for ch in span.content.chars() {
                                    if current_char == char_idx {
                                        cell.set_symbol(&ch.to_string());
                                        cell.set_style(span.style);
                                        break;
                                    }
                                    current_char += 1;
                                }
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

impl Widget for PermissionDialogWidget<'_> {
    fn render(self, area: Rect, buf: &mut ratatui::buffer::Buffer) {
        self.render_themed(area, buf, &Theme::dark());
    }
}

pub fn permission_mode_to_label(mode: &PermissionMode) -> &str {
    match mode {
        PermissionMode::Default => "default",
        PermissionMode::AcceptEdits => "accept-edits",
        PermissionMode::BypassPermissions => "bypass",
        PermissionMode::DontAsk => "dont-ask",
        PermissionMode::Plan => "plan",
        PermissionMode::Auto => "auto",
        PermissionMode::Bubble => "bubble",
    }
}
