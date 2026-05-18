use std::collections::HashMap;

use serde::{Deserialize, Serialize};

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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_bridge_message_type_name() {
        assert_eq!(BridgeMessage::heartbeat().type_name(), "heartbeat");
        assert_eq!(BridgeMessage::error("404", "Not found").type_name(), "error");
        assert_eq!(BridgeMessage::user_message("Hello").type_name(), "user_message");

        let init = BridgeMessage::Init {
            session_id: "s1".to_string(),
            environment_id: None,
            auth_token: None,
        };
        assert_eq!(init.type_name(), "init");

        let custom = BridgeMessage::Custom {
            name: "my_custom".to_string(),
            payload: serde_json::json!({}),
        };
        assert_eq!(custom.type_name(), "my_custom");
    }

    #[test]
    fn test_bridge_message_serialize_deserialize() {
        let msg = BridgeMessage::UserMessage {
            message_id: "msg-1".to_string(),
            content: "Hello".to_string(),
            metadata: HashMap::new(),
        };
        let bytes = msg.to_bytes().unwrap();
        let decoded = BridgeMessage::from_bytes(&bytes).unwrap();
        match decoded {
            BridgeMessage::UserMessage { content, .. } => assert_eq!(content, "Hello"),
            _ => panic!("Expected UserMessage"),
        }
    }

    #[test]
    fn test_bridge_message_init_serialize() {
        let msg = BridgeMessage::Init {
            session_id: "sess-1".to_string(),
            environment_id: Some("env-1".to_string()),
            auth_token: Some("token".to_string()),
        };
        let bytes = msg.to_bytes().unwrap();
        let decoded = BridgeMessage::from_bytes(&bytes).unwrap();
        match decoded {
            BridgeMessage::Init { session_id, .. } => assert_eq!(session_id, "sess-1"),
            _ => panic!("Expected Init"),
        }
    }

    #[test]
    fn test_bridge_message_tool_use_serialize() {
        let msg = BridgeMessage::ToolUse {
            tool_use_id: "tu-1".to_string(),
            tool_name: "Bash".to_string(),
            input: serde_json::json!({"command": "ls"}),
        };
        let bytes = msg.to_bytes().unwrap();
        let decoded = BridgeMessage::from_bytes(&bytes).unwrap();
        match decoded {
            BridgeMessage::ToolUse { tool_name, input, .. } => {
                assert_eq!(tool_name, "Bash");
                assert_eq!(input["command"], "ls");
            }
            _ => panic!("Expected ToolUse"),
        }
    }

    #[test]
    fn test_heartbeat_message() {
        let msg = BridgeMessage::heartbeat();
        match msg {
            BridgeMessage::Heartbeat { timestamp } => assert!(timestamp > 0),
            _ => panic!("Expected Heartbeat"),
        }
    }

    #[test]
    fn test_error_message() {
        let msg = BridgeMessage::error("ERR_001", "Something went wrong");
        match msg {
            BridgeMessage::Error { error_code, message, details } => {
                assert_eq!(error_code, "ERR_001");
                assert_eq!(message, "Something went wrong");
                assert!(details.is_none());
            }
            _ => panic!("Expected Error"),
        }
    }

    #[test]
    fn test_user_message_factory() {
        let msg = BridgeMessage::user_message("Test content");
        match msg {
            BridgeMessage::UserMessage { content, metadata, .. } => {
                assert_eq!(content, "Test content");
                assert!(metadata.is_empty());
            }
            _ => panic!("Expected UserMessage"),
        }
    }

    #[test]
    fn test_message_codec_encode_decode() {
        let msg = BridgeMessage::Heartbeat { timestamp: 12345 };
        let framed = MessageCodec::encode(&msg).unwrap();
        assert!(framed.len() > 4);
        let (decoded, bytes_read) = MessageCodec::decode(&framed).unwrap();
        assert_eq!(bytes_read, framed.len());
        match decoded {
            BridgeMessage::Heartbeat { timestamp } => assert_eq!(timestamp, 12345),
            _ => panic!("Expected Heartbeat"),
        }
    }

    #[test]
    fn test_message_codec_decode_incomplete_frame() {
        let result = MessageCodec::decode(&[0, 0, 0]);
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("at least 4 bytes"));
    }

    #[test]
    fn test_message_codec_decode_truncated_payload() {
        let payload = b"short";
        let len = (payload.len() + 100) as u32;
        let mut data = len.to_be_bytes().to_vec();
        data.extend_from_slice(payload);
        let result = MessageCodec::decode(&data);
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("Incomplete frame"));
    }

    #[test]
    fn test_message_codec_text_encode_decode() {
        let msg = BridgeMessage::Disconnect {
            reason: "timeout".to_string(),
        };
        let text = MessageCodec::encode_text(&msg).unwrap();
        let decoded = MessageCodec::decode_text(&text).unwrap();
        match decoded {
            BridgeMessage::Disconnect { reason } => assert_eq!(reason, "timeout"),
            _ => panic!("Expected Disconnect"),
        }
    }

    #[test]
    fn test_message_codec_text_decode_invalid() {
        let result = MessageCodec::decode_text("not json");
        assert!(result.is_err());
    }

    #[test]
    fn test_message_dispatcher_type_specific_handler() {
        struct CountingHandler {
            count: std::sync::atomic::AtomicUsize,
        }
        impl MessageHandler for CountingHandler {
            fn handle(&self, _message: &BridgeMessage) {
                self.count.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
            }
        }

        let mut dispatcher = MessageDispatcher::new();
        let handler = CountingHandler {
            count: std::sync::atomic::AtomicUsize::new(0),
        };
        dispatcher.register_handler("heartbeat", handler);

        let msg = BridgeMessage::heartbeat();
        dispatcher.dispatch(&msg);
        assert_eq!(dispatcher.handler_count("heartbeat"), 1);
    }

    #[test]
    fn test_message_dispatcher_default_handler() {
        struct CountingHandler {
            count: std::sync::atomic::AtomicUsize,
        }
        impl MessageHandler for CountingHandler {
            fn handle(&self, _message: &BridgeMessage) {
                self.count.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
            }
        }

        let mut dispatcher = MessageDispatcher::new();
        let handler = CountingHandler {
            count: std::sync::atomic::AtomicUsize::new(0),
        };
        dispatcher.register_default_handler(handler);

        dispatcher.dispatch(&BridgeMessage::heartbeat());
        dispatcher.dispatch(&BridgeMessage::error("e", "m"));
        assert_eq!(dispatcher.total_handler_count(), 1);
    }

    #[test]
    fn test_message_queue_push_pop() {
        let mut queue = MessageQueue::new(10);
        queue.push(BridgeMessage::heartbeat()).unwrap();
        queue.push(BridgeMessage::error("e", "m")).unwrap();
        assert_eq!(queue.len(), 2);

        let msg = queue.pop().unwrap();
        assert_eq!(msg.type_name(), "heartbeat");
        assert_eq!(queue.len(), 1);
    }

    #[test]
    fn test_message_queue_full() {
        let mut queue = MessageQueue::new(2);
        queue.push(BridgeMessage::heartbeat()).unwrap();
        queue.push(BridgeMessage::heartbeat()).unwrap();
        let result = queue.push(BridgeMessage::heartbeat());
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("full"));
    }

    #[test]
    fn test_message_queue_peek() {
        let mut queue = MessageQueue::new(10);
        assert!(queue.peek().is_none());
        queue.push(BridgeMessage::heartbeat()).unwrap();
        assert!(queue.peek().is_some());
        assert_eq!(queue.len(), 1);
    }

    #[test]
    fn test_message_queue_drain() {
        let mut queue = MessageQueue::new(10);
        queue.push(BridgeMessage::heartbeat()).unwrap();
        queue.push(BridgeMessage::error("e", "m")).unwrap();
        let msgs = queue.drain();
        assert_eq!(msgs.len(), 2);
        assert!(queue.is_empty());
    }

    #[test]
    fn test_message_queue_clear() {
        let mut queue = MessageQueue::new(10);
        queue.push(BridgeMessage::heartbeat()).unwrap();
        queue.clear();
        assert!(queue.is_empty());
    }

    #[test]
    fn test_message_queue_default() {
        let queue = MessageQueue::default();
        assert!(queue.is_empty());
    }

    #[test]
    fn test_bridge_message_permission_request() {
        let msg = BridgeMessage::PermissionRequest {
            request_id: "req-1".to_string(),
            tool_name: "Bash".to_string(),
            tool_input: serde_json::json!({"command": "rm -rf /"}),
            risk_level: "high".to_string(),
        };
        let bytes = msg.to_bytes().unwrap();
        let decoded = BridgeMessage::from_bytes(&bytes).unwrap();
        match decoded {
            BridgeMessage::PermissionRequest { risk_level, .. } => assert_eq!(risk_level, "high"),
            _ => panic!("Expected PermissionRequest"),
        }
    }

    #[test]
    fn test_bridge_message_stream_event() {
        let msg = BridgeMessage::StreamEvent {
            event_id: "ev-1".to_string(),
            event_type: "text_delta".to_string(),
            data: serde_json::json!({"text": "Hello"}),
        };
        let bytes = msg.to_bytes().unwrap();
        let decoded = BridgeMessage::from_bytes(&bytes).unwrap();
        match decoded {
            BridgeMessage::StreamEvent { event_type, .. } => assert_eq!(event_type, "text_delta"),
            _ => panic!("Expected StreamEvent"),
        }
    }
}
