use std::collections::HashMap;

use serde::{Deserialize, Serialize};
use tracing::{debug, info, warn};

/// Bridge message types.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum BridgeMessage {
    /// Initialize a new session.
    Init {
        session_id: String,
        environment_id: Option<String>,
        auth_token: Option<String>,
    },
    /// Session initialized successfully.
    InitAck {
        session_id: String,
        client_id: String,
    },
    /// Send a user message to the remote session.
    UserMessage {
        message_id: String,
        content: String,
        metadata: HashMap<String, String>,
    },
    /// Receive an assistant message from the remote session.
    AssistantMessage {
        message_id: String,
        content: String,
        metadata: HashMap<String, String>,
    },
    /// Stream event from the remote session.
    StreamEvent {
        event_id: String,
        event_type: String,
        data: serde_json::Value,
    },
    /// Tool use request from the remote session.
    ToolUse {
        tool_use_id: String,
        tool_name: String,
        input: serde_json::Value,
    },
    /// Tool result to send back to the remote session.
    ToolResult {
        tool_use_id: String,
        output: serde_json::Value,
        is_error: bool,
    },
    /// Permission request for tool execution.
    PermissionRequest {
        request_id: String,
        tool_name: String,
        tool_input: serde_json::Value,
        risk_level: String,
    },
    /// Permission response.
    PermissionResponse {
        request_id: String,
        decision: String, // "allow", "deny", "ask"
    },
    /// Heartbeat to keep connection alive.
    Heartbeat {
        timestamp: u64,
    },
    /// Heartbeat acknowledgment.
    HeartbeatAck {
        timestamp: u64,
    },
    /// Error message.
    Error {
        error_code: String,
        message: String,
        details: Option<serde_json::Value>,
    },
    /// Disconnect notification.
    Disconnect {
        reason: String,
    },
    /// Custom message for extension.
    Custom {
        name: String,
        payload: serde_json::Value,
    },
}

impl BridgeMessage {
    /// Serialize to JSON bytes.
    pub fn to_bytes(&self) -> Result<Vec<u8>, String> {
        serde_json::to_vec(self).map_err(|e| format!("Serialization failed: {}", e))
    }

    /// Deserialize from JSON bytes.
    pub fn from_bytes(data: &[u8]) -> Result<Self, String> {
        serde_json::from_slice(data).map_err(|e| format!("Deserialization failed: {}", e))
    }

    /// Get the message type name.
    pub fn type_name(&self) -> &str {
        match self {
            BridgeMessage::Init { .. } => "init",
            BridgeMessage::InitAck { .. } => "init_ack",
            BridgeMessage::UserMessage { .. } => "user_message",
            BridgeMessage::AssistantMessage { .. } => "assistant_message",
            BridgeMessage::StreamEvent { .. } => "stream_event",
            BridgeMessage::ToolUse { .. } => "tool_use",
            BridgeMessage::ToolResult { .. } => "tool_result",
            BridgeMessage::PermissionRequest { .. } => "permission_request",
            BridgeMessage::PermissionResponse { .. } => "permission_response",
            BridgeMessage::Heartbeat { .. } => "heartbeat",
            BridgeMessage::HeartbeatAck { .. } => "heartbeat_ack",
            BridgeMessage::Error { .. } => "error",
            BridgeMessage::Disconnect { .. } => "disconnect",
            BridgeMessage::Custom { name, .. } => name.as_str(),
        }
    }

    /// Create a heartbeat message.
    pub fn heartbeat() -> Self {
        BridgeMessage::Heartbeat {
            timestamp: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_secs(),
        }
    }

    /// Create an error message.
    pub fn error(error_code: &str, message: &str) -> Self {
        BridgeMessage::Error {
            error_code: error_code.to_string(),
            message: message.to_string(),
            details: None,
        }
    }

    /// Create a user message.
    pub fn user_message(content: &str) -> Self {
        BridgeMessage::UserMessage {
            message_id: uuid::Uuid::new_v4().to_string(),
            content: content.to_string(),
            metadata: HashMap::new(),
        }
    }
}

/// Message handler trait for processing bridge messages.
pub trait MessageHandler: Send + Sync {
    /// Handle an incoming message.
    fn handle(&self, message: &BridgeMessage);
}

/// Message dispatcher — routes messages to registered handlers.
pub struct MessageDispatcher {
    handlers: HashMap<String, Vec<Box<dyn MessageHandler>>>,
    default_handlers: Vec<Box<dyn MessageHandler>>,
}

impl MessageDispatcher {
    pub fn new() -> Self {
        Self {
            handlers: HashMap::new(),
            default_handlers: Vec::new(),
        }
    }

    /// Register a handler for a specific message type.
    pub fn register_handler<H: MessageHandler + 'static>(&mut self, message_type: &str, handler: H) {
        self.handlers
            .entry(message_type.to_string())
            .or_default()
            .push(Box::new(handler));
    }

    /// Register a default handler for all messages.
    pub fn register_default_handler<H: MessageHandler + 'static>(&mut self, handler: H) {
        self.default_handlers.push(Box::new(handler));
    }

    /// Dispatch a message to the appropriate handlers.
    pub fn dispatch(&self, message: &BridgeMessage) {
        let type_name = message.type_name();

        // Type-specific handlers
        if let Some(handlers) = self.handlers.get(type_name) {
            for handler in handlers {
                handler.handle(message);
            }
        }

        // Default handlers
        for handler in &self.default_handlers {
            handler.handle(message);
        }
    }

    /// Get handler count for a message type.
    pub fn handler_count(&self, message_type: &str) -> usize {
        self.handlers.get(message_type).map(|h| h.len()).unwrap_or(0)
    }

    /// Get total handler count.
    pub fn total_handler_count(&self) -> usize {
        let type_count: usize = self.handlers.values().map(|h| h.len()).sum();
        type_count + self.default_handlers.len()
    }
}

impl Default for MessageDispatcher {
    fn default() -> Self {
        Self::new()
    }
}

/// Message codec — handles framing and serialization for transport.
pub struct MessageCodec;

impl MessageCodec {
    /// Encode a message with length prefix framing.
    pub fn encode(message: &BridgeMessage) -> Result<Vec<u8>, String> {
        let payload = message.to_bytes()?;
        let len = payload.len() as u32;
        let mut framed = Vec::with_capacity(4 + payload.len());
        framed.extend_from_slice(&len.to_be_bytes());
        framed.extend_from_slice(&payload);
        Ok(framed)
    }

    /// Decode a message from length-prefixed framing.
    pub fn decode(data: &[u8]) -> Result<(BridgeMessage, usize), String> {
        if data.len() < 4 {
            return Err("Incomplete frame: need at least 4 bytes for length".to_string());
        }

        let len = u32::from_be_bytes([data[0], data[1], data[2], data[3]]) as usize;

        if data.len() < 4 + len {
            return Err(format!(
                "Incomplete frame: need {} bytes, have {}",
                4 + len,
                data.len()
            ));
        }

        let payload = &data[4..4 + len];
        let message = BridgeMessage::from_bytes(payload)?;

        Ok((message, 4 + len))
    }

    /// Encode without framing (for WebSocket text frames).
    pub fn encode_text(message: &BridgeMessage) -> Result<String, String> {
        serde_json::to_string(message).map_err(|e| format!("Serialization failed: {}", e))
    }

    /// Decode without framing.
    pub fn decode_text(data: &str) -> Result<BridgeMessage, String> {
        serde_json::from_str(data).map_err(|e| format!("Deserialization failed: {}", e))
    }
}

/// Message queue for buffering and ordering.
pub struct MessageQueue {
    messages: Vec<BridgeMessage>,
    max_size: usize,
}

impl MessageQueue {
    pub fn new(max_size: usize) -> Self {
        Self {
            messages: Vec::with_capacity(max_size),
            max_size,
        }
    }

    /// Push a message to the queue.
    pub fn push(&mut self, message: BridgeMessage) -> Result<(), String> {
        if self.messages.len() >= self.max_size {
            return Err(format!("Message queue full (max: {})", self.max_size));
        }
        self.messages.push(message);
        Ok(())
    }

    /// Pop the next message from the queue.
    pub fn pop(&mut self) -> Option<BridgeMessage> {
        if self.messages.is_empty() {
            None
        } else {
            Some(self.messages.remove(0))
        }
    }

    /// Peek at the next message without removing it.
    pub fn peek(&self) -> Option<&BridgeMessage> {
        self.messages.first()
    }

    /// Get queue length.
    pub fn len(&self) -> usize {
        self.messages.len()
    }

    /// Check if queue is empty.
    pub fn is_empty(&self) -> bool {
        self.messages.is_empty()
    }

    /// Clear the queue.
    pub fn clear(&mut self) {
        self.messages.clear();
    }

    /// Get all messages and clear the queue.
    pub fn drain(&mut self) -> Vec<BridgeMessage> {
        std::mem::take(&mut self.messages)
    }
}

impl Default for MessageQueue {
    fn default() -> Self {
        Self::new(1000)
    }
}
