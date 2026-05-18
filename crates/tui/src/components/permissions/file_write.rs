use ratatui::layout::Rect;
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::Widget;

use crate::theme::{Theme, Themeable};
use crate::components::permissions::dialog::{PermissionDialog, PermissionDialogWidget, PermissionAction};
use cc_core::permissions::RiskLevel;

#[derive(Debug, Clone)]
pub struct FileWritePermissionDialog {
    pub file_path: String,
    pub content: String,
    pub is_new_file: bool,
    pub risk_level: RiskLevel,
    pub inner: PermissionDialog,
}

impl FileWritePermissionDialog {
    pub fn new(file_path: &str, content: &str, is_new_file: bool) -> Self {
        let risk_level = Self::compute_risk(file_path, content);

        let mut preview = String::new();
        if is_new_file {
            preview.push_str(&format!("Creating new file: {}\n\n", file_path));
        } else {
            preview.push_str(&format!("Overwriting: {}\n\n", file_path));
        }

        let lines: Vec<&str> = content.lines().collect();
        let to_show = lines.len().min(10);
        for i in 0..to_show {
            preview.push_str(&format!("  {}\n", lines[i]));
        }
        if lines.len() > 10 {
            preview.push_str(&format!("  ... +{} more lines\n", lines.len() - 10));
        }

        preview.push_str(&format!("\n# {} bytes", content.len()));

        let message = if is_new_file {
            format!("Claude wants to create {}:", file_path)
        } else {
            format!("Claude wants to overwrite {}:", file_path)
        };

        let inner = PermissionDialog::new(
            if is_new_file { "Create File" } else { "Write File" },
            &message,
            &preview,
            risk_level,
        );

        Self {
            file_path: file_path.to_string(),
            content: content.to_string(),
            is_new_file,
            risk_level,
            inner,
        }
    }

    pub fn with_risk_level(mut self, risk: RiskLevel) -> Self {
        self.risk_level = risk;
        self.inner.risk_level = risk;
        self
    }

    fn compute_risk(file_path: &str, content: &str) -> RiskLevel {
        let sensitive_paths = [
            ".env", ".git/", "package.json", "Cargo.toml",
            "pyproject.toml", "tsconfig.json", ".gitignore",
            "Makefile", "Dockerfile", "docker-compose",
            ".bashrc", ".zshrc", ".profile", "passwd", "shadow",
        ];

        let path_lower = file_path.to_lowercase();
        for sensitive in &sensitive_paths {
            if path_lower.contains(sensitive) {
                return RiskLevel::High;
            }
        }

        if content.len() > 100_000 {
            return RiskLevel::Medium;
        }

        RiskLevel::Low
    }

    pub fn handle_key(&mut self, key: &str) -> Option<PermissionAction> {
        self.inner.handle_key(key)
    }
}

impl Themeable for FileWritePermissionDialog {
    fn render_themed(&self, area: Rect, buf: &mut ratatui::buffer::Buffer, theme: &Theme) {
        let widget = PermissionDialogWidget::new(&self.inner, theme, area);
        widget.render_themed(area, buf, theme);
    }
}
