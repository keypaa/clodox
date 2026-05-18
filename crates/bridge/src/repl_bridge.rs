use std::collections::HashMap;
use std::time::{Duration, Instant};

use serde::{Deserialize, Serialize};
use tokio::sync::RwLock;
use tracing::{debug, info, warn};

/// Connection status for the REPL bridge.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum BridgeConnectionStatus {
    Disconnected,
    Connecting,
    Connected,
    Reconnecting,
    Error,
}

/// Bridge connection configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BridgeConfig {
    pub url: String,
    pub session_id: Option<String>,
    pub environment_id: Option<String>,
    pub auth_token: Option<String>,
    pub reconnect_attempts: usize,
    pub reconnect_delay_ms: u64,
    pub heartbeat_interval_ms: u64,
}

impl Default for BridgeConfig {
    fn default() -> Self {
        Self {
            url: String::new(),
            session_id: None,
            environment_id: None,
            auth_token: None,
            reconnect_attempts: 5,
            reconnect_delay_ms: 1000,
            heartbeat_interval_ms: 30000,
        }
    }
}

/// Bridge event types.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum BridgeEvent {
    Connected { session_id: String },
    Disconnected { reason: String },
    MessageReceived { message: String },
    MessageSent { message_id: String },
    Error { error: String },
    Heartbeat { timestamp: u64 },
    ReconnectAttempt { attempt: usize },
}

/// Pending message awaiting acknowledgment.
#[derive(Debug, Clone)]
struct PendingMessage {
    id: String,
    content: String,
    sent_at: Instant,
    attempts: usize,
}

/// REPL bridge service — manages remote session connections and lifecycle.
pub struct ReplBridgeService {
    config: RwLock<BridgeConfig>,
    status: RwLock<BridgeConnectionStatus>,
    session_id: RwLock<Option<String>>,
    environment_id: RwLock<Option<String>>,
    error: RwLock<Option<String>>,
    event_tx: tokio::sync::broadcast::Sender<BridgeEvent>,
    pending_messages: RwLock<HashMap<String, PendingMessage>>,
    connected_at: RwLock<Option<Instant>>,
    message_count: RwLock<usize>,
}

impl ReplBridgeService {
    pub fn new(config: BridgeConfig) -> Self {
        let (event_tx, _) = tokio::sync::broadcast::channel(128);
        Self {
            config: RwLock::new(config),
            status: RwLock::new(BridgeConnectionStatus::Disconnected),
            session_id: RwLock::new(None),
            environment_id: RwLock::new(None),
            error: RwLock::new(None),
            event_tx,
            pending_messages: RwLock::new(HashMap::new()),
            connected_at: RwLock::new(None),
            message_count: RwLock::new(0),
        }
    }

    /// Subscribe to bridge events.
    pub fn subscribe(&self) -> tokio::sync::broadcast::Receiver<BridgeEvent> {
        self.event_tx.subscribe()
    }

    /// Update the bridge configuration.
    pub async fn update_config(&self, config: BridgeConfig) {
        *self.config.write().await = config;
    }

    /// Get the current configuration.
    pub async fn get_config(&self) -> BridgeConfig {
        self.config.read().await.clone()
    }

    /// Connect to the remote session.
    pub async fn connect(&self) -> Result<(), String> {
        let config = self.config.read().await.clone();

        if config.url.is_empty() {
            return Err("Bridge URL is required".to_string());
        }

        *self.status.write().await = BridgeConnectionStatus::Connecting;
        *self.error.write().await = None;

        info!(url = config.url, "Connecting to bridge");

        // In a full implementation, this would establish a WebSocket connection
        // to the remote bridge server and perform the handshake protocol.
        // For now, we simulate the connection lifecycle.

        *self.status.write().await = BridgeConnectionStatus::Connected;
        *self.session_id.write().await = config.session_id.clone();
        *self.environment_id.write().await = config.environment_id.clone();
        *self.connected_at.write().await = Some(Instant::now());

        let session_id = config.session_id.clone().unwrap_or_else(|| "unknown".to_string());
        let session_id_for_event = session_id.clone();

        let _ = self.event_tx.send(BridgeEvent::Connected { session_id: session_id_for_event });

        info!(session_id, "Bridge connected");
        Ok(())
    }

    /// Disconnect from the remote session.
    pub async fn disconnect(&self, reason: &str) {
        *self.status.write().await = BridgeConnectionStatus::Disconnected;
        *self.connected_at.write().await = None;
        self.pending_messages.write().await.clear();

        let _ = self.event_tx.send(BridgeEvent::Disconnected {
            reason: reason.to_string(),
        });

        info!(reason, "Bridge disconnected");
    }

    /// Reconnect with exponential backoff.
    pub async fn reconnect(&self) -> Result<(), String> {
        let config = self.config.read().await.clone();

        for attempt in 1..=config.reconnect_attempts {
            *self.status.write().await = BridgeConnectionStatus::Reconnecting;

            let _ = self.event_tx.send(BridgeEvent::ReconnectAttempt { attempt });

            info!(attempt, "Reconnect attempt");

            // Wait with backoff
            let delay = Duration::from_millis(config.reconnect_delay_ms * 2u64.pow((attempt - 1) as u32));
            tokio::time::sleep(delay).await;

            match self.connect().await {
                Ok(()) => return Ok(()),
                Err(e) => {
                    warn!(attempt, error = %e, "Reconnect failed");
                }
            }
        }

        *self.status.write().await = BridgeConnectionStatus::Error;
        *self.error.write().await = Some("All reconnect attempts failed".to_string());

        Err("All reconnect attempts failed".to_string())
    }

    /// Send a message to the remote session.
    pub async fn send_message(&self, content: &str) -> Result<String, String> {
        let status = *self.status.read().await;
        if status != BridgeConnectionStatus::Connected {
            return Err(format!("Bridge is not connected (status: {:?})", status));
        }

        let message_id = uuid::Uuid::new_v4().to_string();

        let pending = PendingMessage {
            id: message_id.clone(),
            content: content.to_string(),
            sent_at: Instant::now(),
            attempts: 1,
        };

        self.pending_messages.write().await.insert(message_id.clone(), pending);

        let _ = self.event_tx.send(BridgeEvent::MessageSent {
            message_id: message_id.clone(),
        });

        *self.message_count.write().await += 1;

        debug!(message_id, "Message sent to bridge");
        Ok(message_id)
    }

    /// Acknowledge a sent message.
    pub async fn acknowledge_message(&self, message_id: &str) {
        self.pending_messages.write().await.remove(message_id);
        debug!(message_id, "Message acknowledged");
    }

    /// Handle a received message from the remote session.
    pub async fn handle_received_message(&self, content: &str) {
        let _ = self.event_tx.send(BridgeEvent::MessageReceived {
            message: content.to_string(),
        });

        debug!("Message received from bridge");
    }

    /// Send a heartbeat to keep the connection alive.
    pub async fn send_heartbeat(&self) {
        let status = *self.status.read().await;
        if status == BridgeConnectionStatus::Connected {
            let timestamp = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_secs();

            let _ = self.event_tx.send(BridgeEvent::Heartbeat { timestamp });
        }
    }

    /// Get the current connection status.
    pub async fn get_status(&self) -> BridgeConnectionStatus {
        *self.status.read().await
    }

    /// Get the current session ID.
    pub async fn get_session_id(&self) -> Option<String> {
        self.session_id.read().await.clone()
    }

    /// Get the current environment ID.
    pub async fn get_environment_id(&self) -> Option<String> {
        self.environment_id.read().await.clone()
    }

    /// Get the current error (if any).
    pub async fn get_error(&self) -> Option<String> {
        self.error.read().await.clone()
    }

    /// Get pending message count.
    pub async fn pending_message_count(&self) -> usize {
        self.pending_messages.read().await.len()
    }

    /// Get total message count.
    pub async fn message_count(&self) -> usize {
        *self.message_count.read().await
    }

    /// Check if connected.
    pub async fn is_connected(&self) -> bool {
        *self.status.read().await == BridgeConnectionStatus::Connected
    }

    /// Get connection duration.
    pub async fn connection_duration(&self) -> Option<Duration> {
        self.connected_at.read().await.map(|t| t.elapsed())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_config() -> BridgeConfig {
        BridgeConfig {
            url: "ws://localhost:8080".to_string(),
            session_id: Some("sess-1".to_string()),
            environment_id: Some("env-1".to_string()),
            auth_token: Some("token".to_string()),
            reconnect_attempts: 3,
            reconnect_delay_ms: 10,
            heartbeat_interval_ms: 30000,
        }
    }

    #[test]
    fn test_bridge_config_default() {
        let config = BridgeConfig::default();
        assert!(config.url.is_empty());
        assert!(config.session_id.is_none());
        assert_eq!(config.reconnect_attempts, 5);
        assert_eq!(config.reconnect_delay_ms, 1000);
        assert_eq!(config.heartbeat_interval_ms, 30000);
    }

    #[tokio::test]
    async fn test_new_bridge_is_disconnected() {
        let config = test_config();
        let bridge = ReplBridgeService::new(config);
        assert_eq!(bridge.get_status().await, BridgeConnectionStatus::Disconnected);
        assert!(!bridge.is_connected().await);
        assert!(bridge.get_session_id().await.is_none());
        assert!(bridge.connection_duration().await.is_none());
    }

    #[tokio::test]
    async fn test_connect_success() {
        let config = test_config();
        let bridge = ReplBridgeService::new(config);
        bridge.connect().await.unwrap();
        assert_eq!(bridge.get_status().await, BridgeConnectionStatus::Connected);
        assert!(bridge.is_connected().await);
        assert_eq!(bridge.get_session_id().await, Some("sess-1".to_string()));
        assert_eq!(bridge.get_environment_id().await, Some("env-1".to_string()));
    }

    #[tokio::test]
    async fn test_connect_requires_url() {
        let config = BridgeConfig::default();
        let bridge = ReplBridgeService::new(config);
        let result = bridge.connect().await;
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("URL is required"));
    }

    #[tokio::test]
    async fn test_disconnect() {
        let config = test_config();
        let bridge = ReplBridgeService::new(config);
        bridge.connect().await.unwrap();
        bridge.disconnect("user request").await;
        assert_eq!(bridge.get_status().await, BridgeConnectionStatus::Disconnected);
        assert!(!bridge.is_connected().await);
        assert!(bridge.connection_duration().await.is_none());
    }

    #[tokio::test]
    async fn test_send_message_not_connected() {
        let config = test_config();
        let bridge = ReplBridgeService::new(config);
        let result = bridge.send_message("Hello").await;
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("not connected"));
    }

    #[tokio::test]
    async fn test_send_message_success() {
        let config = test_config();
        let bridge = ReplBridgeService::new(config);
        bridge.connect().await.unwrap();
        let message_id = bridge.send_message("Hello").await.unwrap();
        assert!(!message_id.is_empty());
        assert_eq!(bridge.pending_message_count().await, 1);
        assert_eq!(bridge.message_count().await, 1);
    }

    #[tokio::test]
    async fn test_acknowledge_message() {
        let config = test_config();
        let bridge = ReplBridgeService::new(config);
        bridge.connect().await.unwrap();
        let message_id = bridge.send_message("Hello").await.unwrap();
        assert_eq!(bridge.pending_message_count().await, 1);
        bridge.acknowledge_message(&message_id).await;
        assert_eq!(bridge.pending_message_count().await, 0);
    }

    #[tokio::test]
    async fn test_handle_received_message() {
        let config = test_config();
        let bridge = ReplBridgeService::new(config);
        let mut rx = bridge.subscribe();
        bridge.handle_received_message("Response from server").await;
        let event = rx.try_recv().unwrap();
        match event {
            BridgeEvent::MessageReceived { message } => assert_eq!(message, "Response from server"),
            _ => panic!("Expected MessageReceived"),
        }
    }

    #[tokio::test]
    async fn test_send_heartbeat_when_connected() {
        let config = test_config();
        let bridge = ReplBridgeService::new(config);
        bridge.connect().await.unwrap();
        let mut rx = bridge.subscribe();
        bridge.send_heartbeat().await;
        let event = rx.try_recv().unwrap();
        assert!(matches!(event, BridgeEvent::Heartbeat { .. }));
    }

    #[tokio::test]
    async fn test_send_heartbeat_when_disconnected() {
        let config = test_config();
        let bridge = ReplBridgeService::new(config);
        bridge.send_heartbeat().await;
        // Should not panic, just silently skip
    }

    #[tokio::test]
    async fn test_reconnect_success() {
        let config = test_config();
        let bridge = ReplBridgeService::new(config);
        let result = bridge.reconnect().await;
        assert!(result.is_ok());
        assert_eq!(bridge.get_status().await, BridgeConnectionStatus::Connected);
    }

    #[tokio::test]
    async fn test_reconnect_requires_url() {
        let config = BridgeConfig::default();
        let bridge = ReplBridgeService::new(config);
        let result = bridge.reconnect().await;
        assert!(result.is_err());
        assert_eq!(bridge.get_status().await, BridgeConnectionStatus::Error);
    }

    #[tokio::test]
    async fn test_disconnect_clears_pending_messages() {
        let config = test_config();
        let bridge = ReplBridgeService::new(config);
        bridge.connect().await.unwrap();
        bridge.send_message("msg1").await.unwrap();
        bridge.send_message("msg2").await.unwrap();
        assert_eq!(bridge.pending_message_count().await, 2);
        bridge.disconnect("test").await;
        assert_eq!(bridge.pending_message_count().await, 0);
    }

    #[tokio::test]
    async fn test_connection_duration() {
        let config = test_config();
        let bridge = ReplBridgeService::new(config);
        assert!(bridge.connection_duration().await.is_none());
        bridge.connect().await.unwrap();
        tokio::time::sleep(Duration::from_millis(50)).await;
        let duration = bridge.connection_duration().await;
        assert!(duration.is_some());
        assert!(duration.unwrap().as_millis() >= 50);
    }

    #[tokio::test]
    async fn test_get_error_initially_none() {
        let config = test_config();
        let bridge = ReplBridgeService::new(config);
        assert!(bridge.get_error().await.is_none());
    }

    #[tokio::test]
    async fn test_update_config() {
        let config = test_config();
        let bridge = ReplBridgeService::new(config);
        let new_config = BridgeConfig {
            url: "ws://new-host:9090".to_string(),
            ..BridgeConfig::default()
        };
        bridge.update_config(new_config).await;
        let retrieved = bridge.get_config().await;
        assert_eq!(retrieved.url, "ws://new-host:9090");
    }

    #[tokio::test]
    async fn test_subscribe() {
        let config = test_config();
        let bridge = ReplBridgeService::new(config);
        let mut rx = bridge.subscribe();
        bridge.connect().await.unwrap();
        let event = rx.try_recv().unwrap();
        assert!(matches!(event, BridgeEvent::Connected { .. }));
    }
}
