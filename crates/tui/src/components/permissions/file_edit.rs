use ratatui::layout::Rect;
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::Widget;

use crate::theme::{Theme, Themeable};
use crate::components::permissions::dialog::{PermissionDialog, PermissionDialogWidget, PermissionAction};
use cc_core::permissions::RiskLevel;

#[derive(Debug, Clone)]
pub struct FileEditPermissionDialog {
    pub file_path: String,
    pub old_content: String,
    pub new_content: String,
    pub start_line: usize,
    pub end_line: usize,
    pub risk_level: RiskLevel,
    pub inner: PermissionDialog,
}

impl FileEditPermissionDialog {
    pub fn new(file_path: &str, old_content: &str, new_content: &str, start_line: usize, end_line: usize) -> Self {
        let risk_level = Self::compute_risk(file_path, old_content, new_content);

        let mut preview = String::new();
        preview.push_str(&format!("--- {}\n", file_path));
        preview.push_str(&format!("+++ {}\n", file_path));

        let old_lines: Vec<&str> = old_content.lines().collect();
        let new_lines: Vec<&str> = new_content.lines().collect();

        let start = start_line.saturating_sub(2).min(old_lines.len());
        let end = (end_line + 2).min(old_lines.len().max(new_lines.len()));

        for i in start..end {
            if i < old_lines.len() && i < new_lines.len() && old_lines[i] == new_lines[i] {
                preview.push_str(&format!("  {}\n", old_lines[i]));
            } else {
                if i < old_lines.len() {
                    preview.push_str(&format!("- {}\n", old_lines[i]));
                }
                if i < new_lines.len() {
                    preview.push_str(&format!("+ {}\n", new_lines[i]));
                }
            }
        }

        let message = format!("Claude wants to edit {}:", file_path);

        let inner = PermissionDialog::new(
            "File Edit",
            &message,
            &preview,
            risk_level,
        );

        Self {
            file_path: file_path.to_string(),
            old_content: old_content.to_string(),
            new_content: new_content.to_string(),
            start_line,
            end_line,
            risk_level,
            inner,
        }
    }

    pub fn with_risk_level(mut self, risk: RiskLevel) -> Self {
        self.risk_level = risk;
        self.inner.risk_level = risk;
        self
    }

    fn compute_risk(file_path: &str, _old: &str, _new: &str) -> RiskLevel {
        let sensitive_paths = [
            ".env", ".git/config", "package.json", "Cargo.toml",
            "pyproject.toml", "tsconfig.json", ".gitignore",
            "Makefile", "Dockerfile", "docker-compose",
        ];

        let path_lower = file_path.to_lowercase();
        for sensitive in &sensitive_paths {
            if path_lower.contains(sensitive) {
                return RiskLevel::Medium;
            }
        }

        RiskLevel::Low
    }

    pub fn handle_key(&mut self, key: &str) -> Option<PermissionAction> {
        self.inner.handle_key(key)
    }
}

impl Themeable for FileEditPermissionDialog {
    fn render_themed(&self, area: Rect, buf: &mut ratatui::buffer::Buffer, theme: &Theme) {
        let widget = PermissionDialogWidget::new(&self.inner, theme, area);
        widget.render_themed(area, buf, theme);
    }
}
