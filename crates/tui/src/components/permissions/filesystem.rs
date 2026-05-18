use ratatui::layout::Rect;

use crate::theme::{Theme, Themeable};
use crate::components::permissions::dialog::{PermissionDialog, PermissionDialogWidget, PermissionAction};
use cc_core::permissions::RiskLevel;

#[derive(Debug, Clone, PartialEq)]
pub enum FilesystemOperation {
    Read,
    List,
    Stat,
}

#[derive(Debug, Clone)]
pub struct FilesystemPermissionDialog {
    pub path: String,
    pub operation: FilesystemOperation,
    pub risk_level: RiskLevel,
    pub inner: PermissionDialog,
}

impl FilesystemPermissionDialog {
    pub fn new(path: &str, operation: FilesystemOperation) -> Self {
        let risk_level = Self::compute_risk(path, &operation);

        let (title, message) = match &operation {
            FilesystemOperation::Read => (
                "Read File",
                format!("Claude wants to read {}:", path),
            ),
            FilesystemOperation::List => (
                "List Directory",
                format!("Claude wants to list contents of {}:", path),
            ),
            FilesystemOperation::Stat => (
                "Stat File",
                format!("Claude wants to stat {}:", path),
            ),
        };

        let preview = path.to_string();

        let inner = PermissionDialog::new(
            title,
            &message,
            &preview,
            risk_level,
        );

        Self {
            path: path.to_string(),
            operation,
            risk_level,
            inner,
        }
    }

    pub fn with_risk_level(mut self, risk: RiskLevel) -> Self {
        self.risk_level = risk;
        self.inner.risk_level = risk;
        self
    }

    fn compute_risk(path: &str, operation: &FilesystemOperation) -> RiskLevel {
        let sensitive_paths = [
            ".env", ".git/", "passwd", "shadow", "sudoers",
            ".ssh/", "id_rsa", "id_ed25519", ".aws/",
            ".config/", "credentials",
        ];

        let path_lower = path.to_lowercase();
        for sensitive in &sensitive_paths {
            if path_lower.contains(sensitive) {
                return RiskLevel::High;
            }
        }

        match operation {
            FilesystemOperation::Read => RiskLevel::Low,
            FilesystemOperation::List => RiskLevel::Low,
            FilesystemOperation::Stat => RiskLevel::Low,
        }
    }

    pub fn handle_key(&mut self, key: &str) -> Option<PermissionAction> {
        self.inner.handle_key(key)
    }
}

impl Themeable for FilesystemPermissionDialog {
    fn render_themed(&self, area: Rect, buf: &mut ratatui::buffer::Buffer, theme: &Theme) {
        let widget = PermissionDialogWidget::new(&self.inner, theme, area);
        widget.render_themed(area, buf, theme);
    }
}
