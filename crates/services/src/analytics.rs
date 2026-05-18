use std::collections::HashMap;
use std::sync::Arc;
use std::time::Instant;

use serde::{Deserialize, Serialize};
use tokio::sync::RwLock;
use tracing::{debug, info};

/// Analytics event types.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum AnalyticsEvent {
    SessionStart {
        session_id: String,
        model: String,
    },
    SessionEnd {
        session_id: String,
        duration_ms: u64,
        total_tokens: u64,
        total_cost: f64,
    },
    MessageSent {
        message_length: usize,
        has_attachments: bool,
    },
    ToolUsed {
        tool_name: String,
        success: bool,
        duration_ms: u64,
    },
    CommandExecuted {
        command_name: String,
        args_length: usize,
    },
    CompactionPerformed {
        messages_before: usize,
        messages_after: usize,
        tokens_saved: u64,
    },
    Error {
        error_type: String,
        error_message: String,
    },
    FeatureFlagEvaluated {
        flag_name: String,
        enabled: bool,
    },
    Custom {
        name: String,
        properties: HashMap<String, serde_json::Value>,
    },
}

/// Feature flag value (cached, may be stale).
#[derive(Debug, Clone)]
pub struct FeatureFlagValue {
    pub value: serde_json::Value,
    pub evaluated_at: Instant,
    pub ttl: std::time::Duration,
}

impl FeatureFlagValue {
    pub fn is_expired(&self) -> bool {
        self.evaluated_at.elapsed() > self.ttl
    }
}

/// Analytics service — event tracking, session metrics, feature flags.
pub struct AnalyticsService {
    events: RwLock<Vec<AnalyticsEvent>>,
    feature_flags: RwLock<HashMap<String, FeatureFlagValue>>,
    session_start: RwLock<Option<Instant>>,
    session_id: RwLock<Option<String>>,
    event_count: RwLock<usize>,
    /// Maximum events to keep in memory before flushing.
    max_buffer_size: usize,
}

impl AnalyticsService {
    pub fn new(max_buffer_size: usize) -> Self {
        Self {
            events: RwLock::new(Vec::with_capacity(max_buffer_size)),
            feature_flags: RwLock::new(HashMap::new()),
            session_start: RwLock::new(None),
            session_id: RwLock::new(None),
            event_count: RwLock::new(0),
            max_buffer_size,
        }
    }

    /// Start a new session.
    pub async fn start_session(&self, session_id: String, model: String) {
        let event = AnalyticsEvent::SessionStart {
            session_id: session_id.clone(),
            model,
        };

        *self.session_start.write().await = Some(Instant::now());
        let sid_for_log = session_id.clone();
        *self.session_id.write().await = Some(session_id);

        self.track(event).await;
        info!(session_id = sid_for_log, "Analytics session started");
    }

    /// End the current session.
    pub async fn end_session(&self, total_tokens: u64, total_cost: f64) {
        let session_id = self.session_id.read().await.clone();
        let duration_ms = self
            .session_start
            .read()
            .await
            .map(|s| s.elapsed().as_millis() as u64)
            .unwrap_or(0);

        if let Some(sid) = session_id {
            let event = AnalyticsEvent::SessionEnd {
                session_id: sid,
                duration_ms,
                total_tokens,
                total_cost,
            };
            self.track(event).await;
            info!(duration_ms, total_tokens, "Analytics session ended");
        }

        *self.session_start.write().await = None;
        *self.session_id.write().await = None;
    }

    /// Track an analytics event.
    pub async fn track(&self, event: AnalyticsEvent) {
        let mut events = self.events.write().await;
        events.push(event);
        *self.event_count.write().await += 1;

        // Flush if buffer is full
        if events.len() >= self.max_buffer_size {
            debug!(count = events.len(), "Analytics buffer full, clearing");
            events.clear();
        }
    }

    /// Track a tool usage event.
    pub async fn track_tool_used(&self, tool_name: &str, success: bool, duration_ms: u64) {
        self.track(AnalyticsEvent::ToolUsed {
            tool_name: tool_name.to_string(),
            success,
            duration_ms,
        })
        .await;
    }

    /// Track a command execution event.
    pub async fn track_command(&self, command_name: &str, args_length: usize) {
        self.track(AnalyticsEvent::CommandExecuted {
            command_name: command_name.to_string(),
            args_length,
        })
        .await;
    }

    /// Track an error event.
    pub async fn track_error(&self, error_type: &str, error_message: &str) {
        self.track(AnalyticsEvent::Error {
            error_type: error_type.to_string(),
            error_message: error_message.to_string(),
        })
        .await;
    }

    /// Set a feature flag value (simulating GrowthBook evaluation).
    pub async fn set_feature_flag(&self, flag_name: &str, value: serde_json::Value, ttl: std::time::Duration) {
        let mut flags = self.feature_flags.write().await;
        flags.insert(
            flag_name.to_string(),
            FeatureFlagValue {
                value,
                evaluated_at: Instant::now(),
                ttl,
            },
        );
    }

    /// Get a feature flag value (cached, may be stale).
    pub async fn get_feature_flag(&self, flag_name: &str, default: serde_json::Value) -> serde_json::Value {
        let flags = self.feature_flags.read().await;
        match flags.get(flag_name) {
            Some(flag) if !flag.is_expired() => flag.value.clone(),
            _ => default,
        }
    }

    /// Get a feature flag as a boolean.
    pub async fn get_feature_flag_bool(&self, flag_name: &str, default: bool) -> bool {
        let value = self
            .get_feature_flag(flag_name, serde_json::Value::Bool(default))
            .await;
        value.as_bool().unwrap_or(default)
    }

    /// Get a feature flag as a string.
    pub async fn get_feature_flag_str(&self, flag_name: &str, default: &str) -> String {
        let value = self
            .get_feature_flag(flag_name, serde_json::Value::String(default.to_string()))
            .await;
        value.as_str().unwrap_or(default).to_string()
    }

    /// Get all buffered events.
    pub async fn get_events(&self) -> Vec<AnalyticsEvent> {
        self.events.read().await.clone()
    }

    /// Get event count.
    pub async fn event_count(&self) -> usize {
        *self.event_count.read().await
    }

    /// Get buffered event count.
    pub async fn buffered_event_count(&self) -> usize {
        self.events.read().await.len()
    }

    /// Clear all buffered events.
    pub async fn clear_events(&self) {
        self.events.write().await.clear();
    }

    /// Get session metrics.
    pub async fn get_session_metrics(&self) -> SessionMetrics {
        let events = self.events.read().await;
        let mut tools_used = 0usize;
        let mut commands_executed = 0usize;
        let mut errors = 0usize;
        let mut compactions = 0usize;

        for event in events.iter() {
            match event {
                AnalyticsEvent::ToolUsed { .. } => tools_used += 1,
                AnalyticsEvent::CommandExecuted { .. } => commands_executed += 1,
                AnalyticsEvent::Error { .. } => errors += 1,
                AnalyticsEvent::CompactionPerformed { .. } => compactions += 1,
                _ => {}
            }
        }

        let duration_ms = self
            .session_start
            .read()
            .await
            .map(|s| s.elapsed().as_millis() as u64)
            .unwrap_or(0);

        SessionMetrics {
            duration_ms,
            tools_used,
            commands_executed,
            errors,
            compactions,
            total_events: events.len(),
        }
    }
}

impl Default for AnalyticsService {
    fn default() -> Self {
        Self::new(1000)
    }
}

/// Session metrics summary.
#[derive(Debug, Clone, Default)]
pub struct SessionMetrics {
    pub duration_ms: u64,
    pub tools_used: usize,
    pub commands_executed: usize,
    pub errors: usize,
    pub compactions: usize,
    pub total_events: usize,
}
