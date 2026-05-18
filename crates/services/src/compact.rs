use std::sync::Arc;

use cc_core::messages::{ContentBlockParam, Message, UserMessage};
use tokio::sync::RwLock;
use tracing::info;

/// Compaction strategy to use when context is full.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum CompactionStrategy {
    /// Summarize the conversation history into a condensed form.
    #[default]
    Summarize,
    /// Remove oldest messages first (FIFO eviction).
    FifoEviction,
    /// Remove messages based on importance score.
    SmartEviction,
    /// Keep only the most recent N messages.
    Truncate { keep_last: usize },
}

/// Hook type for compaction lifecycle events.
#[derive(Debug, Clone)]
pub enum CompactionHook {
    PreCompact,
    PostCompact,
    SessionStart,
}

/// Hook callback function type.
pub type CompactionHookFn = Arc<dyn Fn(CompactionHook) + Send + Sync>;

/// Compaction event for progress tracking.
#[derive(Debug, Clone)]
pub enum CompactionEvent {
    HooksStart { hook_type: String },
    CompactStart,
    CompactEnd {
        messages_before: usize,
        messages_after: usize,
        tokens_saved: u64,
    },
}

/// Summary of a compaction operation.
#[derive(Debug, Clone, Default)]
pub struct CompactionSummary {
    pub messages_before: usize,
    pub messages_after: usize,
    pub tokens_before: u64,
    pub tokens_after: u64,
    pub strategy: CompactionStrategy,
    pub duration_ms: u64,
}

impl CompactionSummary {
    pub fn tokens_saved(&self) -> u64 {
        self.tokens_before.saturating_sub(self.tokens_after)
    }

    pub fn messages_removed(&self) -> usize {
        self.messages_before.saturating_sub(self.messages_after)
    }
}

/// Compact service — manages context compaction strategies and token budget.
pub struct CompactService {
    strategy: RwLock<CompactionStrategy>,
    hooks: RwLock<Vec<CompactionHookFn>>,
    event_tx: tokio::sync::broadcast::Sender<CompactionEvent>,
    /// Maximum tokens before compaction is triggered.
    max_tokens: RwLock<u64>,
    /// Estimated tokens per message (rough average).
    tokens_per_message: RwLock<u64>,
    /// Compaction history.
    history: RwLock<Vec<CompactionSummary>>,
}

impl CompactService {
    pub fn new(max_tokens: u64) -> Self {
        let (event_tx, _) = tokio::sync::broadcast::channel(32);
        Self {
            strategy: RwLock::new(CompactionStrategy::Summarize),
            hooks: RwLock::new(Vec::new()),
            event_tx,
            max_tokens: RwLock::new(max_tokens),
            tokens_per_message: RwLock::new(1000),
            history: RwLock::new(Vec::new()),
        }
    }

    /// Subscribe to compaction events.
    pub fn subscribe(&self) -> tokio::sync::broadcast::Receiver<CompactionEvent> {
        self.event_tx.subscribe()
    }

    /// Register a compaction lifecycle hook.
    pub async fn register_hook(&self, hook: CompactionHookFn) {
        self.hooks.write().await.push(hook);
    }

    /// Set the compaction strategy.
    pub async fn set_strategy(&self, strategy: CompactionStrategy) {
        *self.strategy.write().await = strategy;
        info!(?strategy, "Compaction strategy updated");
    }

    /// Get the current compaction strategy.
    pub async fn get_strategy(&self) -> CompactionStrategy {
        *self.strategy.read().await
    }

    /// Set the maximum token threshold for triggering compaction.
    pub async fn set_max_tokens(&self, max_tokens: u64) {
        *self.max_tokens.write().await = max_tokens;
    }

    /// Check if compaction is needed based on current message count and token estimate.
    pub async fn needs_compaction(&self, message_count: usize) -> bool {
        let max = *self.max_tokens.read().await;
        let per_msg = *self.tokens_per_message.read().await;
        let estimated = message_count as u64 * per_msg;
        estimated >= max
    }

    /// Execute compaction on a message list.
    pub async fn compact(
        &self,
        messages: Vec<Message>,
        model_name: &str,
    ) -> (Vec<Message>, CompactionSummary) {
        let start = std::time::Instant::now();
        let messages_before = messages.len();
        let per_msg = *self.tokens_per_message.read().await;
        let tokens_before = self.estimate_tokens(&messages, per_msg);

        let strategy = *self.strategy.read().await;

        // Fire pre-compact hooks
        self.fire_hooks(CompactionHook::PreCompact).await;

        let _ = self
            .event_tx
            .send(CompactionEvent::CompactStart);

        let (compacted, tokens_after) = match strategy {
            CompactionStrategy::Summarize => {
                self.summarize_compact(messages, model_name).await
            }
            CompactionStrategy::FifoEviction => {
                let max = *self.max_tokens.read().await;
                let per_msg = *self.tokens_per_message.read().await;
                self.fifo_compact(messages, max, per_msg)
            }
            CompactionStrategy::SmartEviction => {
                let max = *self.max_tokens.read().await;
                let per_msg = *self.tokens_per_message.read().await;
                self.smart_compact(messages, max, per_msg)
            }
            CompactionStrategy::Truncate { keep_last } => {
                let per_msg = *self.tokens_per_message.read().await;
                self.truncate_compact(messages, keep_last, per_msg)
            }
        };

        let messages_after = compacted.len();
        let duration_ms = start.elapsed().as_millis() as u64;

        // Fire post-compact hooks
        self.fire_hooks(CompactionHook::PostCompact).await;

        let summary = CompactionSummary {
            messages_before,
            messages_after,
            tokens_before,
            tokens_after,
            strategy,
            duration_ms,
        };

        info!(
            messages_before,
            messages_after,
            tokens_saved = summary.tokens_saved(),
            strategy = ?strategy,
            "Compaction complete"
        );

        let _ = self.event_tx.send(CompactionEvent::CompactEnd {
            messages_before,
            messages_after,
            tokens_saved: summary.tokens_saved(),
        });

        // Store in history
        self.history.write().await.push(summary.clone());

        (compacted, summary)
    }

    /// Summarize-based compaction: replace early messages with a summary.
    async fn summarize_compact(
        &self,
        messages: Vec<Message>,
        _model_name: &str,
    ) -> (Vec<Message>, u64) {
        let per_msg = *self.tokens_per_message.read().await;
        // In a full implementation, this would call the API with a summarization
        // prompt to condense early messages. For now, we keep the system prompt,
        // the last few messages, and insert a summary placeholder.
        if messages.len() <= 4 {
            let tokens = self.estimate_tokens(&messages, per_msg);
            return (messages, tokens);
        }

        let mut compacted = Vec::new();

        // Keep system messages
        for msg in &messages {
            if matches!(msg, Message::System(_)) {
                compacted.push(msg.clone());
            }
        }

        // Insert summary placeholder
        let summary_text = format!(
            "[Previous conversation summarized — {} messages compacted to save context tokens]",
            messages.len() - 4
        );
        compacted.push(Message::User(UserMessage {
            id: uuid::Uuid::new_v4(),
            content: vec![ContentBlockParam::Text { text: summary_text }],
            timestamp: chrono::Utc::now(),
            is_meta: Some(true),
            origin_query_source: None,
            effort: None,
        }));

        // Keep last 3 messages (most recent context)
        for msg in messages.iter().rev().take(3).rev() {
            compacted.push(msg.clone());
        }

        let tokens = self.estimate_tokens(&compacted, per_msg);
        (compacted, tokens)
    }

    /// FIFO compaction: remove oldest messages first.
    fn fifo_compact(&self, messages: Vec<Message>, max_tokens: u64, per_msg: u64) -> (Vec<Message>, u64) {

        // Keep system messages, evict oldest non-system messages
        let mut system_msgs: Vec<Message> = Vec::new();
        let mut other_msgs: Vec<Message> = Vec::new();

        for msg in messages {
            if matches!(msg, Message::System(_)) {
                system_msgs.push(msg);
            } else {
                other_msgs.push(msg);
            }
        }

        // Remove from the front until under budget
        let max_messages = (max_tokens / per_msg.max(1)) as usize;
        while other_msgs.len() > max_messages {
            other_msgs.remove(0);
        }

        let mut compacted = system_msgs;
        compacted.extend(other_msgs);

        let tokens = self.estimate_tokens(&compacted, per_msg);
        (compacted, tokens)
    }

    /// Smart compaction: estimate importance and remove least important messages.
    fn smart_compact(&self, messages: Vec<Message>, max_tokens: u64, per_msg: u64) -> (Vec<Message>, u64) {

        let mut system_msgs: Vec<Message> = Vec::new();
        let mut scored: Vec<(u8, Message)> = Vec::new();

        for msg in messages {
            match &msg {
                Message::System(_) => system_msgs.push(msg),
                Message::User(u) => {
                    let score = if u.is_meta.unwrap_or(false) { 1 } else { 3 };
                    scored.push((score, msg));
                }
                Message::Assistant(a) => {
                    let score = if a.is_meta.unwrap_or(false) { 1 } else { 5 };
                    scored.push((score, msg));
                }
                _ => scored.push((2, msg)),
            }
        }

        // Sort by score ascending (lowest first)
        scored.sort_by_key(|(s, _)| *s);

        let max_messages = (max_tokens / per_msg.max(1)) as usize;
        while scored.len() > max_messages {
            scored.remove(0);
        }

        let mut compacted = system_msgs;
        compacted.extend(scored.into_iter().map(|(_, m)| m));

        let tokens = self.estimate_tokens(&compacted, per_msg);
        (compacted, tokens)
    }

    /// Truncate compaction: keep only the last N messages.
    fn truncate_compact(&self, messages: Vec<Message>, keep_last: usize, per_msg: u64) -> (Vec<Message>, u64) {
        let mut system_msgs: Vec<Message> = Vec::new();
        let mut other_msgs: Vec<Message> = Vec::new();

        for msg in messages {
            if matches!(msg, Message::System(_)) {
                system_msgs.push(msg);
            } else {
                other_msgs.push(msg);
            }
        }

        // Keep last N non-system messages
        let truncated: Vec<Message> = if other_msgs.len() > keep_last {
            other_msgs.split_off(other_msgs.len() - keep_last)
        } else {
            other_msgs
        };

        let mut compacted = system_msgs;
        compacted.extend(truncated);

        let tokens = self.estimate_tokens(&compacted, per_msg);
        (compacted, tokens)
    }

    /// Estimate total tokens in a message list.
    fn estimate_tokens(&self, messages: &[Message], per_msg: u64) -> u64 {
        messages.len() as u64 * per_msg
    }

    /// Fire all registered hooks.
    async fn fire_hooks(&self, hook: CompactionHook) {
        let hooks = self.hooks.read().await;
        for hook_fn in hooks.iter() {
            hook_fn(hook.clone());
        }
    }

    /// Get compaction history.
    pub async fn get_history(&self) -> Vec<CompactionSummary> {
        self.history.read().await.clone()
    }

    /// Get total tokens saved across all compactions.
    pub async fn total_tokens_saved(&self) -> u64 {
        self.history
            .read()
            .await
            .iter()
            .map(|s| s.tokens_saved())
            .sum()
    }

    /// Get compaction count.
    pub async fn compaction_count(&self) -> usize {
        self.history.read().await.len()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_compaction_summary_tokens_saved() {
        let summary = CompactionSummary {
            messages_before: 100,
            messages_after: 20,
            tokens_before: 50_000,
            tokens_after: 10_000,
            strategy: CompactionStrategy::Summarize,
            duration_ms: 500,
        };
        assert_eq!(summary.tokens_saved(), 40_000);
        assert_eq!(summary.messages_removed(), 80);
    }

    #[test]
    fn test_compaction_summary_default() {
        let summary = CompactionSummary::default();
        assert_eq!(summary.messages_before, 0);
        assert_eq!(summary.messages_after, 0);
        assert_eq!(summary.tokens_saved(), 0);
    }

    #[test]
    fn test_compaction_strategy_default() {
        let strategy = CompactionStrategy::default();
        assert_eq!(strategy, CompactionStrategy::Summarize);
    }

    #[tokio::test]
    async fn test_needs_compaction_below_threshold() {
        let service = CompactService::new(10_000);
        assert!(!service.needs_compaction(5).await);
    }

    #[tokio::test]
    async fn test_needs_compaction_above_threshold() {
        let service = CompactService::new(10_000);
        assert!(service.needs_compaction(20).await);
    }

    #[tokio::test]
    async fn test_compact_small_list() {
        let service = CompactService::new(10_000);
        let messages = vec![];
        let (compacted, summary) = service.compact(messages, "test-model").await;
        assert_eq!(compacted.len(), 0);
        assert_eq!(summary.messages_before, 0);
        assert_eq!(summary.messages_after, 0);
    }

    #[tokio::test]
    async fn test_set_and_get_strategy() {
        let service = CompactService::new(10_000);
        service.set_strategy(CompactionStrategy::FifoEviction).await;
        assert_eq!(service.get_strategy().await, CompactionStrategy::FifoEviction);
    }

    #[tokio::test]
    async fn test_history_tracks_compactions() {
        let service = CompactService::new(10_000);
        let messages = vec![];
        let _ = service.compact(messages, "test-model").await;
        assert_eq!(service.compaction_count().await, 1);
        assert!(service.total_tokens_saved().await >= 0);
    }
}
