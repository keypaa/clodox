use ratatui::layout::Rect;
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::Widget;

use crate::theme::{Theme, Themeable};
use crate::components::permissions::dialog::{PermissionDialog, PermissionDialogWidget, PermissionAction};
use cc_core::permissions::RiskLevel;

#[derive(Debug, Clone)]
pub struct AskUserDialog {
    pub question: String,
    pub suggestions: Vec<String>,
    pub risk_level: RiskLevel,
    pub inner: PermissionDialog,
}

impl AskUserDialog {
    pub fn new(question: &str) -> Self {
        let risk_level = RiskLevel::Low;

        let mut preview = question.to_string();

        let inner = PermissionDialog::new(
            "Question",
            "Claude is asking:",
            &preview,
            risk_level,
        );

        Self {
            question: question.to_string(),
            suggestions: Vec::new(),
            risk_level,
            inner,
        }
    }

    pub fn with_suggestions(mut self, suggestions: Vec<String>) -> Self {
        self.suggestions = suggestions;
        self
    }

    pub fn with_risk_level(mut self, risk: RiskLevel) -> Self {
        self.risk_level = risk;
        self.inner.risk_level = risk;
        self
    }

    pub fn handle_key(&mut self, key: &str) -> Option<PermissionAction> {
        if !self.suggestions.is_empty() {
            if let Ok(idx) = key.parse::<usize>() {
                if idx > 0 && idx <= self.suggestions.len() {
                    return Some(PermissionAction::AllowOnce);
                }
            }
        }
        self.inner.handle_key(key)
    }

    pub fn selected_suggestion(&self, key: &str) -> Option<String> {
        if let Ok(idx) = key.parse::<usize>() {
            if idx > 0 && idx <= self.suggestions.len() {
                return Some(self.suggestions[idx - 1].clone());
            }
        }
        None
    }
}

impl Themeable for AskUserDialog {
    fn render_themed(&self, area: Rect, buf: &mut ratatui::buffer::Buffer, theme: &Theme) {
        let widget = PermissionDialogWidget::new(&self.inner, theme, area);
        widget.render_themed(area, buf, theme);

        if !self.suggestions.is_empty() {
            let y_offset = area.y + 12;
            if y_offset < buf.area.height {
                let suggestion_style = Style::default()
                    .fg(theme.colors.suggestion)
                    .add_modifier(Modifier::BOLD);
                let number_style = Style::default()
                    .fg(theme.colors.success)
                    .add_modifier(Modifier::BOLD);

                for (i, suggestion) in self.suggestions.iter().take(3).enumerate() {
                    let x = area.x + 4;
                    let y = y_offset + i as u16;
                    if y < buf.area.height {
                        let text = format!("[{}] {}", i + 1, suggestion);
                        let mut current_x = x;
                        for ch in text.chars() {
                            if current_x < area.x + area.width {
                                if let Some(cell) = buf.cell_mut((current_x, y)) {
                                    cell.set_symbol(&ch.to_string());
                                    cell.set_style(suggestion_style);
                                }
                            }
                            current_x += 1;
                        }
                    }
                }
            }
        }
    }
}
