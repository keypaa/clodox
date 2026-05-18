use std::collections::HashMap;
use std::sync::Arc;
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
