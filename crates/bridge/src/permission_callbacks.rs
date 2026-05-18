use std::collections::HashMap;
use std::sync::Arc;

use cc_core::permissions::{PermissionBehavior, PermissionMode, PermissionResult};
use serde::{Deserialize, Serialize};
use tokio::sync::RwLock;
use tracing::{debug, info, warn};

/// Permission callback request from the bridge.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PermissionCallbackRequest {
    pub request_id: String,
    pub session_id: String,
    pub tool_name: String,
    pub tool_input: serde_json::Value,
    pub risk_level: String,
    pub permission_mode: String,
}

/// Permission callback response.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PermissionCallbackResponse {
    pub request_id: String,
    pub decision: PermissionDecision,
    pub reason: Option<String>,
    pub updated_input: Option<serde_json::Value>,
}

/// Permission decision from the bridge.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum PermissionDecision {
    Allow,
    Deny,
    Ask,
}

impl std::fmt::Display for PermissionDecision {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            PermissionDecision::Allow => write!(f, "allow"),
            PermissionDecision::Deny => write!(f, "deny"),
            PermissionDecision::Ask => write!(f, "ask"),
        }
    }
}

/// Pending permission request awaiting response.
#[derive(Debug)]
struct PendingPermissionRequest {
    pub request: PermissionCallbackRequest,
    pub responder: tokio::sync::oneshot::Sender<PermissionDecision>,
}

/// Permission callback handler for bridge-specific permission decisions.
pub struct PermissionCallbacks {
    /// Pending requests awaiting local user response.
    pending: RwLock<HashMap<String, PendingPermissionRequest>>,
    /// Auto-allow rules for the bridge.
    auto_allow_rules: RwLock<Vec<String>>,
    /// Auto-deny rules for the bridge.
    auto_deny_rules: RwLock<Vec<String>>,
    /// Default permission mode for the bridge.
    default_mode: RwLock<PermissionMode>,
    /// Response channel for sending permission requests to the UI.
    request_tx: tokio::sync::mpsc::Sender<PermissionCallbackRequest>,
}

impl PermissionCallbacks {
    pub fn new(request_tx: tokio::sync::mpsc::Sender<PermissionCallbackRequest>) -> Self {
        Self {
            pending: RwLock::new(HashMap::new()),
            auto_allow_rules: RwLock::new(Vec::new()),
            auto_deny_rules: RwLock::new(Vec::new()),
            default_mode: RwLock::new(PermissionMode::Default),
            request_tx,
        }
    }

    /// Handle a permission request from the bridge.
    pub async fn handle_request(
        &self,
        request: PermissionCallbackRequest,
    ) -> Result<PermissionDecision, String> {
        let tool_name = &request.tool_name;

        // Check auto-allow rules
        let allow_rules = self.auto_allow_rules.read().await;
        if allow_rules.iter().any(|rule| rule == tool_name || rule == "*") {
            debug!(tool_name, "Auto-allowed by rule");
            return Ok(PermissionDecision::Allow);
        }
        drop(allow_rules);

        // Check auto-deny rules
        let deny_rules = self.auto_deny_rules.read().await;
        if deny_rules.iter().any(|rule| rule == tool_name || rule == "*") {
            debug!(tool_name, "Auto-denied by rule");
            return Ok(PermissionDecision::Deny);
        }
        drop(deny_rules);

        // Check permission mode
        let mode = *self.default_mode.read().await;
        match mode {
            PermissionMode::BypassPermissions => {
                debug!(tool_name, "Auto-allowed by bypass mode");
                return Ok(PermissionDecision::Allow);
            }
            PermissionMode::DontAsk => {
                // Auto-allow read-only tools, ask for destructive ones
                if is_read_only_tool(tool_name, &request.tool_input) {
                    return Ok(PermissionDecision::Allow);
                }
            }
            PermissionMode::AcceptEdits => {
                // Auto-allow file editing tools
                if is_edit_tool(tool_name) {
                    return Ok(PermissionDecision::Allow);
                }
            }
            PermissionMode::Plan => {
                // In plan mode, always ask
            }
            _ => {}
        }

        // Create a oneshot channel for the response
        let (tx, rx) = tokio::sync::oneshot::channel();

        let pending = PendingPermissionRequest {
            request: request.clone(),
            responder: tx,
        };

        self.pending
            .write()
            .await
            .insert(request.request_id.clone(), pending);

        // Send the request to the UI
        self.request_tx
            .send(request.clone())
            .await
            .map_err(|_| "Failed to send permission request to UI".to_string())?;

        info!(
            request_id = request.request_id,
            tool_name,
            "Permission request sent to UI"
        );

        // Wait for the response
        match rx.await {
            Ok(decision) => {
                debug!(request_id = request.request_id, ?decision, "Permission response received");
                Ok(decision)
            }
            Err(_) => {
                warn!(request_id = request.request_id, "Permission request channel closed");
                Ok(PermissionDecision::Deny)
            }
        }
    }

    /// Respond to a pending permission request.
    pub async fn respond(&self, request_id: &str, decision: PermissionDecision) -> Result<(), String> {
        let mut pending = self.pending.write().await;

        if let Some(pending_req) = pending.remove(request_id) {
            pending_req
                .responder
                .send(decision)
                .map_err(|_| "Failed to send response".to_string())?;

            debug!(request_id, ?decision, "Permission response sent");
            Ok(())
        } else {
            Err(format!("Pending request not found: {request_id}"))
        }
    }

    /// Add an auto-allow rule.
    pub async fn add_auto_allow_rule(&self, tool_name: &str) {
        self.auto_allow_rules
            .write()
            .await
            .push(tool_name.to_string());
        info!(tool_name, "Auto-allow rule added");
    }

    /// Add an auto-deny rule.
    pub async fn add_auto_deny_rule(&self, tool_name: &str) {
        self.auto_deny_rules
            .write()
            .await
            .push(tool_name.to_string());
        info!(tool_name, "Auto-deny rule added");
    }

    /// Remove an auto-allow rule.
    pub async fn remove_auto_allow_rule(&self, tool_name: &str) {
        self.auto_allow_rules
            .write()
            .await
            .retain(|r| r != tool_name);
        info!(tool_name, "Auto-allow rule removed");
    }

    /// Remove an auto-deny rule.
    pub async fn remove_auto_deny_rule(&self, tool_name: &str) {
        self.auto_deny_rules
            .write()
            .await
            .retain(|r| r != tool_name);
        info!(tool_name, "Auto-deny rule removed");
    }

    /// Set the default permission mode.
    pub async fn set_default_mode(&self, mode: PermissionMode) {
        *self.default_mode.write().await = mode;
        info!(?mode, "Default permission mode set");
    }

    /// Get the default permission mode.
    pub async fn get_default_mode(&self) -> PermissionMode {
        *self.default_mode.read().await
    }

    /// Get pending request count.
    pub async fn pending_count(&self) -> usize {
        self.pending.read().await.len()
    }

    /// Get all pending request IDs.
    pub async fn get_pending_request_ids(&self) -> Vec<String> {
        self.pending.read().await.keys().cloned().collect()
    }

    /// Get auto-allow rules.
    pub async fn get_auto_allow_rules(&self) -> Vec<String> {
        self.auto_allow_rules.read().await.clone()
    }

    /// Get auto-deny rules.
    pub async fn get_auto_deny_rules(&self) -> Vec<String> {
        self.auto_deny_rules.read().await.clone()
    }

    /// Clear all pending requests.
    pub async fn clear_pending(&self) {
        self.pending.write().await.clear();
        info!("All pending permission requests cleared");
    }
}

/// Check if a tool is read-only.
fn is_read_only_tool(tool_name: &str, _input: &serde_json::Value) -> bool {
    matches!(
        tool_name,
        "Read" | "Grep" | "Glob" | "web_fetch" | "web_search"
    )
}

/// Check if a tool is an editing tool.
fn is_edit_tool(tool_name: &str) -> bool {
    matches!(tool_name, "Edit" | "Write")
}

/// Convert a PermissionDecision to a PermissionResult.
pub fn decision_to_result(
    decision: PermissionDecision,
    tool_input: serde_json::Value,
    reason: Option<String>,
) -> PermissionResult {
    let reason_str = reason.clone();
    match decision {
        PermissionDecision::Allow => PermissionResult::Allow {
            updated_input: Some(tool_input),
            user_modified: None,
            decision_reason: None,
            tool_use_id: None,
            accept_feedback: None,
            content_blocks: None,
        },
        PermissionDecision::Deny => PermissionResult::Deny {
            message: reason.unwrap_or_else(|| "Permission denied".to_string()),
            decision_reason: cc_core::permissions::PermissionDecisionReason::Other {
                reason: reason_str.unwrap_or_else(|| "Denied by bridge".to_string()),
            },
            tool_use_id: None,
        },
        PermissionDecision::Ask => PermissionResult::Ask {
            message: reason.unwrap_or_else(|| "Permission requested".to_string()),
            updated_input: Some(tool_input),
            decision_reason: None,
            suggestions: None,
            blocked_path: None,
            pending_classifier_check: None,
            content_blocks: None,
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_permission_decision_display() {
        assert_eq!(PermissionDecision::Allow.to_string(), "allow");
        assert_eq!(PermissionDecision::Deny.to_string(), "deny");
        assert_eq!(PermissionDecision::Ask.to_string(), "ask");
    }

    #[test]
    fn test_decision_to_result_allow() {
        let input = serde_json::json!({"command": "ls"});
        let result = decision_to_result(PermissionDecision::Allow, input.clone(), None);
        assert!(matches!(result, PermissionResult::Allow { .. }));
        if let PermissionResult::Allow { updated_input, .. } = result {
            assert_eq!(updated_input, Some(input));
        }
    }

    #[test]
    fn test_decision_to_result_deny() {
        let input = serde_json::json!({"command": "rm -rf /"});
        let result = decision_to_result(PermissionDecision::Deny, input, Some("Too dangerous".to_string()));
        assert!(matches!(result, PermissionResult::Deny { .. }));
        if let PermissionResult::Deny { message, .. } = result {
            assert_eq!(message, "Too dangerous");
        }
    }

    #[test]
    fn test_decision_to_result_deny_default_message() {
        let input = serde_json::json!({});
        let result = decision_to_result(PermissionDecision::Deny, input, None);
        if let PermissionResult::Deny { message, .. } = result {
            assert_eq!(message, "Permission denied");
        }
    }

    #[test]
    fn test_decision_to_result_ask() {
        let input = serde_json::json!({"command": "npm install"});
        let result = decision_to_result(PermissionDecision::Ask, input.clone(), None);
        assert!(matches!(result, PermissionResult::Ask { .. }));
        if let PermissionResult::Ask { updated_input, .. } = result {
            assert_eq!(updated_input, Some(input));
        }
    }

    #[test]
    fn test_is_read_only_tool() {
        assert!(is_read_only_tool("Read", &serde_json::json!({})));
        assert!(is_read_only_tool("Grep", &serde_json::json!({})));
        assert!(is_read_only_tool("Glob", &serde_json::json!({})));
        assert!(is_read_only_tool("web_fetch", &serde_json::json!({})));
        assert!(is_read_only_tool("web_search", &serde_json::json!({})));
        assert!(!is_read_only_tool("Bash", &serde_json::json!({})));
        assert!(!is_read_only_tool("Write", &serde_json::json!({})));
    }

    #[test]
    fn test_is_edit_tool() {
        assert!(is_edit_tool("Edit"));
        assert!(is_edit_tool("Write"));
        assert!(!is_edit_tool("Read"));
        assert!(!is_edit_tool("Bash"));
    }

    #[tokio::test]
    async fn test_auto_allow_rule() {
        let (tx, _rx) = tokio::sync::mpsc::channel(10);
        let callbacks = PermissionCallbacks::new(tx);
        callbacks.add_auto_allow_rule("Read").await;

        let request = PermissionCallbackRequest {
            request_id: "req-1".to_string(),
            session_id: "sess-1".to_string(),
            tool_name: "Read".to_string(),
            tool_input: serde_json::json!({"path": "file.txt"}),
            risk_level: "low".to_string(),
            permission_mode: "default".to_string(),
        };

        let decision = callbacks.handle_request(request).await.unwrap();
        assert_eq!(decision, PermissionDecision::Allow);
    }

    #[tokio::test]
    async fn test_auto_deny_rule() {
        let (tx, _rx) = tokio::sync::mpsc::channel(10);
        let callbacks = PermissionCallbacks::new(tx);
        callbacks.add_auto_deny_rule("Bash").await;

        let request = PermissionCallbackRequest {
            request_id: "req-2".to_string(),
            session_id: "sess-1".to_string(),
            tool_name: "Bash".to_string(),
            tool_input: serde_json::json!({"command": "rm -rf /"}),
            risk_level: "high".to_string(),
            permission_mode: "default".to_string(),
        };

        let decision = callbacks.handle_request(request).await.unwrap();
        assert_eq!(decision, PermissionDecision::Deny);
    }

    #[tokio::test]
    async fn test_wildcard_allow_rule() {
        let (tx, _rx) = tokio::sync::mpsc::channel(10);
        let callbacks = PermissionCallbacks::new(tx);
        callbacks.add_auto_allow_rule("*").await;

        let request = PermissionCallbackRequest {
            request_id: "req-3".to_string(),
            session_id: "sess-1".to_string(),
            tool_name: "AnyTool".to_string(),
            tool_input: serde_json::json!({}),
            risk_level: "low".to_string(),
            permission_mode: "default".to_string(),
        };

        let decision = callbacks.handle_request(request).await.unwrap();
        assert_eq!(decision, PermissionDecision::Allow);
    }

    #[tokio::test]
    async fn test_bypass_permissions_mode() {
        let (tx, _rx) = tokio::sync::mpsc::channel(10);
        let callbacks = PermissionCallbacks::new(tx);
        callbacks.set_default_mode(PermissionMode::BypassPermissions).await;

        let request = PermissionCallbackRequest {
            request_id: "req-4".to_string(),
            session_id: "sess-1".to_string(),
            tool_name: "Bash".to_string(),
            tool_input: serde_json::json!({"command": "ls"}),
            risk_level: "low".to_string(),
            permission_mode: "bypass".to_string(),
        };

        let decision = callbacks.handle_request(request).await.unwrap();
        assert_eq!(decision, PermissionDecision::Allow);
    }

    #[tokio::test]
    async fn test_dont_ask_mode_read_only() {
        let (tx, _rx) = tokio::sync::mpsc::channel(10);
        let callbacks = PermissionCallbacks::new(tx);
        callbacks.set_default_mode(PermissionMode::DontAsk).await;

        let request = PermissionCallbackRequest {
            request_id: "req-5".to_string(),
            session_id: "sess-1".to_string(),
            tool_name: "Read".to_string(),
            tool_input: serde_json::json!({"path": "file.txt"}),
            risk_level: "low".to_string(),
            permission_mode: "dont_ask".to_string(),
        };

        let decision = callbacks.handle_request(request).await.unwrap();
        assert_eq!(decision, PermissionDecision::Allow);
    }

    #[tokio::test]
    async fn test_accept_edits_mode() {
        let (tx, _rx) = tokio::sync::mpsc::channel(10);
        let callbacks = PermissionCallbacks::new(tx);
        callbacks.set_default_mode(PermissionMode::AcceptEdits).await;

        let request = PermissionCallbackRequest {
            request_id: "req-6".to_string(),
            session_id: "sess-1".to_string(),
            tool_name: "Write".to_string(),
            tool_input: serde_json::json!({"path": "file.txt", "content": "hello"}),
            risk_level: "medium".to_string(),
            permission_mode: "accept_edits".to_string(),
        };

        let decision = callbacks.handle_request(request).await.unwrap();
        assert_eq!(decision, PermissionDecision::Allow);
    }

    #[tokio::test]
    async fn test_respond_to_permission_request() {
        let (tx, mut rx) = tokio::sync::mpsc::channel(10);
        let callbacks = Arc::new(PermissionCallbacks::new(tx));

        let request = PermissionCallbackRequest {
            request_id: "req-7".to_string(),
            session_id: "sess-1".to_string(),
            tool_name: "Bash".to_string(),
            tool_input: serde_json::json!({"command": "ls"}),
            risk_level: "low".to_string(),
            permission_mode: "default".to_string(),
        };

        let callbacks_clone = callbacks.clone();
        let handle = tokio::spawn(async move {
            callbacks_clone.handle_request(request).await
        });

        let incoming = rx.recv().await.unwrap();
        assert_eq!(incoming.request_id, "req-7");
        callbacks.respond("req-7", PermissionDecision::Allow).await.unwrap();

        let decision = handle.await.unwrap().unwrap();
        assert_eq!(decision, PermissionDecision::Allow);
    }

    #[tokio::test]
    async fn test_respond_to_nonexistent_request() {
        let (tx, _rx) = tokio::sync::mpsc::channel(10);
        let callbacks = PermissionCallbacks::new(tx);
        let result = callbacks.respond("nonexistent", PermissionDecision::Allow).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_add_and_remove_allow_rule() {
        let (tx, _rx) = tokio::sync::mpsc::channel(10);
        let callbacks = PermissionCallbacks::new(tx);
        callbacks.add_auto_allow_rule("Bash").await;
        assert_eq!(callbacks.get_auto_allow_rules().await, vec!["Bash"]);
        callbacks.remove_auto_allow_rule("Bash").await;
        assert!(callbacks.get_auto_allow_rules().await.is_empty());
    }

    #[tokio::test]
    async fn test_add_and_remove_deny_rule() {
        let (tx, _rx) = tokio::sync::mpsc::channel(10);
        let callbacks = PermissionCallbacks::new(tx);
        callbacks.add_auto_deny_rule("Write").await;
        assert_eq!(callbacks.get_auto_deny_rules().await, vec!["Write"]);
        callbacks.remove_auto_deny_rule("Write").await;
        assert!(callbacks.get_auto_deny_rules().await.is_empty());
    }

    #[tokio::test]
    async fn test_pending_count() {
        let (tx, _rx) = tokio::sync::mpsc::channel(10);
        let callbacks = PermissionCallbacks::new(tx);
        assert_eq!(callbacks.pending_count().await, 0);
    }

    #[tokio::test]
    async fn test_clear_pending() {
        let (tx, _rx) = tokio::sync::mpsc::channel(10);
        let callbacks = PermissionCallbacks::new(tx);
        callbacks.clear_pending().await;
        assert_eq!(callbacks.pending_count().await, 0);
    }

    #[tokio::test]
    async fn test_get_default_mode() {
        let (tx, _rx) = tokio::sync::mpsc::channel(10);
        let callbacks = PermissionCallbacks::new(tx);
        assert_eq!(callbacks.get_default_mode().await, PermissionMode::Default);
        callbacks.set_default_mode(PermissionMode::Plan).await;
        assert_eq!(callbacks.get_default_mode().await, PermissionMode::Plan);
    }

    #[tokio::test]
    async fn test_channel_closed_returns_deny() {
        let (tx, rx) = tokio::sync::mpsc::channel(10);
        drop(rx);
        let callbacks = PermissionCallbacks::new(tx);

        let request = PermissionCallbackRequest {
            request_id: "req-8".to_string(),
            session_id: "sess-1".to_string(),
            tool_name: "Bash".to_string(),
            tool_input: serde_json::json!({"command": "ls"}),
            risk_level: "low".to_string(),
            permission_mode: "default".to_string(),
        };

        let result = callbacks.handle_request(request).await;
        assert!(result.is_err());
    }
}
