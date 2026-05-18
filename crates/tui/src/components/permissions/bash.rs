use ratatui::layout::Rect;

use crate::theme::{Theme, Themeable};
use crate::components::permissions::dialog::{PermissionDialog, PermissionDialogWidget, PermissionAction};
use cc_core::permissions::{ClassifierResult, RiskLevel};

#[derive(Debug, Clone)]
pub struct BashPermissionDialog {
    pub command: String,
    pub cwd: String,
    pub sandboxed: bool,
    pub classifier: Option<ClassifierResult>,
    pub risk_level: RiskLevel,
    pub inner: PermissionDialog,
}

impl BashPermissionDialog {
    pub fn new(command: &str, cwd: &str, sandboxed: bool) -> Self {
        let risk_level = Self::compute_risk(command, sandboxed);
        let message = if sandboxed {
            "Claude wants to run a sandboxed command:"
        } else {
            "Claude wants to run:"
        };

        let mut preview = command.to_string();
        if !cwd.is_empty() {
            preview.push_str(&format!("\n# cwd: {}", cwd));
        }

        let inner = PermissionDialog::new(
            "Bash Command",
            message,
            &preview,
            risk_level,
        );

        Self {
            command: command.to_string(),
            cwd: cwd.to_string(),
            sandboxed,
            classifier: None,
            risk_level,
            inner,
        }
    }

    pub fn with_classifier(mut self, classifier: ClassifierResult) -> Self {
        self.classifier = Some(classifier.clone());
        if let Some(ref desc) = classifier.matched_description {
            self.inner.message = format!(
                "Claude wants to run:\n⚠ Classifier: {} (confidence: {:?})",
                desc, classifier.confidence
            );
        }
        self
    }

    pub fn with_risk_level(mut self, risk: RiskLevel) -> Self {
        self.risk_level = risk;
        self.inner.risk_level = risk;
        self
    }

    fn compute_risk(command: &str, sandboxed: bool) -> RiskLevel {
        if sandboxed {
            return RiskLevel::Low;
        }

        let dangerous_patterns = [
            "rm -rf", "rm -r", "sudo", "chmod 777", "dd if=",
            "mkfs", "fdisk", "curl | sh", "wget | bash",
            "eval(", "exec(", "system(",
        ];

        let cmd_lower = command.to_lowercase();
        for pattern in &dangerous_patterns {
            if cmd_lower.contains(pattern) {
                return RiskLevel::High;
            }
        }

        let moderate_patterns = [
            "rm ", "mv ", "chmod", "chown", "kill",
            "pkill", "killall", "shutdown", "reboot",
        ];

        for pattern in &moderate_patterns {
            if cmd_lower.contains(pattern) {
                return RiskLevel::Medium;
            }
        }

        RiskLevel::Low
    }

    pub fn handle_key(&mut self, key: &str) -> Option<PermissionAction> {
        self.inner.handle_key(key)
    }
}

impl Themeable for BashPermissionDialog {
    fn render_themed(&self, area: Rect, buf: &mut ratatui::buffer::Buffer, theme: &Theme) {
        let widget = PermissionDialogWidget::new(&self.inner, theme, area);
        widget.render_themed(area, buf, theme);
    }
}
