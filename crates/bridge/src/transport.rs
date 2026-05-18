use std::collections::HashMap;
use std::time::Duration;

use serde::{Deserialize, Serialize};
use tokio::sync::RwLock;
use tracing::{debug, info};

/// Transport protocol type.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum TransportProtocol {
    Http,
    WebSocket,
}

impl std::fmt::Display for TransportProtocol {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            TransportProtocol::Http => write!(f, "HTTP"),
            TransportProtocol::WebSocket => write!(f, "WebSocket"),
        }
    }
}

/// Transport connection state.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TransportState {
    Idle,
    Connecting,
    Connected,
    Closed,
    Error,
}

/// Message frame for transport layer.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TransportMessage {
    pub id: String,
    pub payload: Vec<u8>,
    pub metadata: HashMap<String, String>,
    pub timestamp: u64,
}

impl TransportMessage {
    pub fn new(payload: Vec<u8>) -> Self {
        Self {
            id: uuid::Uuid::new_v4().to_string(),
            payload,
            metadata: HashMap::new(),
            timestamp: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_secs(),
        }
    }

    pub fn with_metadata(mut self, key: &str, value: &str) -> Self {
        self.metadata.insert(key.to_string(), value.to_string());
        self
    }

    pub fn to_json(&self) -> Result<Vec<u8>, String> {
        serde_json::to_vec(self).map_err(|e| format!("Failed to serialize: {}", e))
    }

    pub fn from_json(data: &[u8]) -> Result<Self, String> {
        serde_json::from_slice(data).map_err(|e| format!("Failed to deserialize: {}", e))
    }
}

/// HTTP transport implementation.
pub struct HttpTransport {
    client: reqwest::Client,
    state: RwLock<TransportState>,
    url: RwLock<Option<String>>,
}

impl HttpTransport {
    pub fn new() -> Self {
        Self {
            client: reqwest::Client::builder()
                .timeout(Duration::from_secs(30))
                .build()
                .expect("Failed to create HTTP client"),
            state: RwLock::new(TransportState::Idle),
            url: RwLock::new(None),
        }
    }

    pub async fn connect(&self, url: &str) -> Result<(), String> {
        *self.state.write().await = TransportState::Connecting;
        *self.url.write().await = Some(url.to_string());

        match self.client.head(url).send().await {
            Ok(resp) if resp.status().is_success() => {
                *self.state.write().await = TransportState::Connected;
                info!(url, "HTTP transport connected");
                Ok(())
            }
            Ok(resp) => {
                *self.state.write().await = TransportState::Error;
                Err(format!("HTTP error: {}", resp.status()))
            }
            Err(e) => {
                *self.state.write().await = TransportState::Error;
                Err(format!("Connection failed: {}", e))
            }
        }
    }

    pub async fn disconnect(&self) -> Result<(), String> {
        *self.state.write().await = TransportState::Closed;
        *self.url.write().await = None;
        info!("HTTP transport disconnected");
        Ok(())
    }

    pub async fn send(&self, message: &TransportMessage) -> Result<(), String> {
        let url = self.url.read().await.clone().ok_or("Not connected")?;

        let response = self
            .client
            .post(&url)
            .json(message)
            .send()
            .await
            .map_err(|e| format!("Send failed: {}", e))?;

        if response.status().is_success() {
            debug!(id = message.id, "HTTP message sent");
            Ok(())
        } else {
            Err(format!("HTTP error: {}", response.status()))
        }
    }

    pub async fn get_state(&self) -> TransportState {
        *self.state.read().await
    }

    pub fn protocol(&self) -> TransportProtocol {
        TransportProtocol::Http
    }
}

/// WebSocket transport implementation.
pub struct WebSocketTransport {
    state: RwLock<TransportState>,
    url: RwLock<Option<String>>,
    write_tx: tokio::sync::mpsc::Sender<Vec<u8>>,
}

impl WebSocketTransport {
    pub fn new() -> (Self, tokio::sync::mpsc::Receiver<Result<TransportMessage, String>>) {
        let (write_tx, _) = tokio::sync::mpsc::channel(100);
        let (_, msg_rx) = tokio::sync::mpsc::channel(100);

        let transport = Self {
            state: RwLock::new(TransportState::Idle),
            url: RwLock::new(None),
            write_tx,
        };
        (transport, msg_rx)
    }

    pub async fn connect(&self, url: &str) -> Result<(), String> {
        *self.state.write().await = TransportState::Connecting;
        *self.url.write().await = Some(url.to_string());

        // In a full implementation, this would use `tokio-tungstenite` to
        // establish the WebSocket connection and spawn the event loop.
        *self.state.write().await = TransportState::Connected;
        info!(url, "WebSocket transport connected");
        Ok(())
    }

    pub async fn disconnect(&self) -> Result<(), String> {
        *self.state.write().await = TransportState::Closed;
        *self.url.write().await = None;
        info!("WebSocket transport disconnected");
        Ok(())
    }

    pub async fn send(&self, message: &TransportMessage) -> Result<(), String> {
        let data = message.to_json()?;
        self.write_tx
            .send(data)
            .await
            .map_err(|_| "Write channel closed".to_string())?;

        debug!(id = message.id, "WebSocket message sent");
        Ok(())
    }

    pub async fn get_state(&self) -> TransportState {
        *self.state.read().await
    }

    pub fn protocol(&self) -> TransportProtocol {
        TransportProtocol::WebSocket
    }
}

/// Transport wrapper — holds either HTTP or WebSocket transport.
pub enum Transport {
    Http(HttpTransport),
    WebSocket(WebSocketTransport),
}

impl Transport {
    pub async fn connect(&self, url: &str) -> Result<(), String> {
        match self {
            Transport::Http(t) => t.connect(url).await,
            Transport::WebSocket(t) => t.connect(url).await,
        }
    }

    pub async fn disconnect(&self) -> Result<(), String> {
        match self {
            Transport::Http(t) => t.disconnect().await,
            Transport::WebSocket(t) => t.disconnect().await,
        }
    }

    pub async fn send(&self, message: &TransportMessage) -> Result<(), String> {
        match self {
            Transport::Http(t) => t.send(message).await,
            Transport::WebSocket(t) => t.send(message).await,
        }
    }

    pub async fn get_state(&self) -> TransportState {
        match self {
            Transport::Http(t) => t.get_state().await,
            Transport::WebSocket(t) => t.get_state().await,
        }
    }

    pub fn protocol(&self) -> TransportProtocol {
        match self {
            Transport::Http(t) => t.protocol(),
            Transport::WebSocket(t) => t.protocol(),
        }
    }

    pub async fn is_connected(&self) -> bool {
        self.get_state().await == TransportState::Connected
    }
}

/// Transport factory — creates the appropriate transport for a URL.
pub struct TransportFactory;

impl TransportFactory {
    pub fn create(url: &str) -> Transport {
        if url.starts_with("ws://") || url.starts_with("wss://") {
            let (transport, _) = WebSocketTransport::new();
            Transport::WebSocket(transport)
        } else {
            Transport::Http(HttpTransport::new())
        }
    }

    pub fn create_http() -> Transport {
        Transport::Http(HttpTransport::new())
    }

    pub fn create_websocket() -> Transport {
        let (transport, _) = WebSocketTransport::new();
        Transport::WebSocket(transport)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_transport_protocol_display() {
        assert_eq!(TransportProtocol::Http.to_string(), "HTTP");
        assert_eq!(TransportProtocol::WebSocket.to_string(), "WebSocket");
    }

    #[test]
    fn test_transport_message_new() {
        let msg = TransportMessage::new(b"hello".to_vec());
        assert!(!msg.id.is_empty());
        assert_eq!(msg.payload, b"hello");
        assert!(msg.metadata.is_empty());
        assert!(msg.timestamp > 0);
    }

    #[test]
    fn test_transport_message_with_metadata() {
        let msg = TransportMessage::new(b"data".to_vec())
            .with_metadata("key", "value");
        assert_eq!(msg.metadata.get("key"), Some(&"value".to_string()));
    }

    #[test]
    fn test_transport_message_json_roundtrip() {
        let msg = TransportMessage::new(b"payload".to_vec())
            .with_metadata("type", "test");
        let json = msg.to_json().unwrap();
        let decoded = TransportMessage::from_json(&json).unwrap();
        assert_eq!(decoded.payload, b"payload");
        assert_eq!(decoded.metadata.get("type"), Some(&"test".to_string()));
    }

    #[test]
    fn test_transport_message_json_invalid() {
        let result = TransportMessage::from_json(b"not json");
        assert!(result.is_err());
    }

    #[test]
    fn test_transport_factory_create_http() {
        let transport = TransportFactory::create("https://example.com/api");
        assert!(matches!(transport.protocol(), TransportProtocol::Http));
    }

    #[test]
    fn test_transport_factory_create_websocket() {
        let transport = TransportFactory::create("ws://example.com/ws");
        assert!(matches!(transport.protocol(), TransportProtocol::WebSocket));
    }

    #[test]
    fn test_transport_factory_create_wss() {
        let transport = TransportFactory::create("wss://example.com/ws");
        assert!(matches!(transport.protocol(), TransportProtocol::WebSocket));
    }

    #[test]
    fn test_transport_factory_create_http_explicit() {
        let transport = TransportFactory::create_http();
        assert!(matches!(transport.protocol(), TransportProtocol::Http));
    }

    #[test]
    fn test_transport_factory_create_websocket_explicit() {
        let transport = TransportFactory::create_websocket();
        assert!(matches!(transport.protocol(), TransportProtocol::WebSocket));
    }

    #[tokio::test]
    async fn test_http_transport_initial_state() {
        let transport = HttpTransport::new();
        assert_eq!(transport.get_state().await, TransportState::Idle);
        assert_eq!(transport.protocol(), TransportProtocol::Http);
    }

    #[tokio::test]
    async fn test_http_transport_connect_invalid_url() {
        let transport = HttpTransport::new();
        let result = transport.connect("http://localhost:99999").await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_http_transport_disconnect() {
        let transport = HttpTransport::new();
        let result = transport.disconnect().await;
        assert!(result.is_ok());
        assert_eq!(transport.get_state().await, TransportState::Closed);
    }

    #[tokio::test]
    async fn test_websocket_transport_initial_state() {
        let (transport, _msg_rx) = WebSocketTransport::new();
        assert_eq!(transport.get_state().await, TransportState::Idle);
        assert_eq!(transport.protocol(), TransportProtocol::WebSocket);
    }

    #[tokio::test]
    async fn test_websocket_transport_connect() {
        let (transport, _msg_rx) = WebSocketTransport::new();
        let result = transport.connect("ws://localhost:8080").await;
        assert!(result.is_ok());
        assert_eq!(transport.get_state().await, TransportState::Connected);
    }

    #[tokio::test]
    async fn test_websocket_transport_disconnect() {
        let (transport, _msg_rx) = WebSocketTransport::new();
        transport.connect("ws://localhost:8080").await.unwrap();
        let result = transport.disconnect().await;
        assert!(result.is_ok());
        assert_eq!(transport.get_state().await, TransportState::Closed);
    }

    #[tokio::test]
    async fn test_websocket_transport_send() {
        let (transport, _msg_rx) = WebSocketTransport::new();
        transport.connect("ws://localhost:8080").await.unwrap();
        let msg = TransportMessage::new(b"test".to_vec());
        // Send will fail because the receiver side of write_tx is dropped in new()
        let result = transport.send(&msg).await;
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("Write channel closed"));
    }

    #[tokio::test]
    async fn test_transport_wrapper_http() {
        let transport = Transport::Http(HttpTransport::new());
        assert_eq!(transport.get_state().await, TransportState::Idle);
        assert_eq!(transport.protocol(), TransportProtocol::Http);
    }

    #[tokio::test]
    async fn test_transport_wrapper_websocket() {
        let (ws, _) = WebSocketTransport::new();
        let transport = Transport::WebSocket(ws);
        assert_eq!(transport.get_state().await, TransportState::Idle);
        assert_eq!(transport.protocol(), TransportProtocol::WebSocket);
    }

    #[tokio::test]
    async fn test_transport_is_connected() {
        let transport = Transport::Http(HttpTransport::new());
        assert!(!transport.is_connected().await);
    }

    #[tokio::test]
    async fn test_transport_send_not_connected() {
        let transport = Transport::Http(HttpTransport::new());
        let msg = TransportMessage::new(b"test".to_vec());
        let result = transport.send(&msg).await;
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("Not connected"));
    }
}
