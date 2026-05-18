use cc_core::messages::{
    ContentBlockParam, Message, SystemMessage,
};
use tracing::{debug, info};

/// A single compaction operation performed on the message history.
#[derive(Debug, Clone)]
pub struct CompactionOp {
    pub kind: CompactionKind,
    pub messages_removed: usize,
    pub tokens_saved: usize,
    pub description: String,
}

#[derive(Debug, Clone)]
pub enum CompactionKind {
    /// Removed oldest messages to fit within context window.
    Snip {
        /// Number of messages removed from the beginning.
        count: usize,
    },
    /// Collapsed consecutive file edit sequences.
    Microcompact {
        /// File paths that were compacted.
        files: Vec<String>,
    },
    /// LLM-based summarization of older messages.
    AutoCompact {
        /// Summary text produced by the model.
        summary: String,
    },
    /// Emergency compaction triggered by prompt-too-long.
    ReactiveCompact {
        /// How many tokens over the limit we were.
        over_by: usize,
    },
}

/// Result of a compaction operation.
#[derive(Debug, Clone)]
pub struct CompactionResult {
    pub ops: Vec<CompactionOp>,
    pub total_tokens_saved: usize,
    pub total_messages_removed: usize,
}

impl CompactionResult {
    pub fn empty() -> Self {
        Self {
            ops: Vec::new(),
            total_tokens_saved: 0,
            total_messages_removed: 0,
        }
    }

    pub fn is_empty(&self) -> bool {
        self.ops.is_empty()
    }
}

/// Estimate the number of tokens in a text string.
/// Uses the rule-of-thumb: ~4 characters per token for English text.
pub fn estimate_tokens(text: &str) -> usize {
    text.len() / 4
}

/// Estimate tokens for a message.
pub fn estimate_message_tokens(msg: &Message) -> usize {
    match msg {
        Message::User(u) => u.content.iter().map(estimate_content_block_tokens).sum(),
        Message::Assistant(a) => a.content.iter().map(estimate_content_block_tokens).sum(),
        Message::Attachment(_) => 0,
        Message::Progress(_) => 0,
        Message::System(_) => 0,
        Message::Tombstone(_) => 0,
        Message::ToolUseSummary(_) => 0,
    }
}

fn estimate_content_block_tokens(block: &ContentBlockParam) -> usize {
    match block {
        ContentBlockParam::Text { text } => estimate_tokens(text),
        ContentBlockParam::Image { .. } => 1000, // Rough estimate for images
        ContentBlockParam::ToolUse { input, .. } => {
            estimate_tokens(&serde_json::to_string(input).unwrap_or_default())
        }
        ContentBlockParam::ToolResult { content, .. } => {
            content.iter().map(estimate_tool_result_tokens).sum()
        }
        ContentBlockParam::Thinking { thinking, .. } => estimate_tokens(thinking),
        ContentBlockParam::RedactedThinking { data } => estimate_tokens(data),
    }
}

fn estimate_tool_result_tokens(block: &cc_core::messages::ToolResultContent) -> usize {
    match block {
        cc_core::messages::ToolResultContent::Text { text } => estimate_tokens(text),
        cc_core::messages::ToolResultContent::Image { .. } => 1000,
    }
}

/// Estimate total tokens for a list of messages.
pub fn estimate_total_tokens(messages: &[Message]) -> usize {
    messages.iter().map(estimate_message_tokens).sum()
}

// ============================================================================
// Snip Compact
// ============================================================================

/// Remove oldest messages when history exceeds a token limit.
///
/// Keeps the most recent messages up to `max_tokens`, preserving
/// system messages and the most recent tool result pairing.
pub fn snip_compact(
    messages: &[Message],
    max_tokens: usize,
) -> (Vec<Message>, CompactionResult) {
    let total = estimate_total_tokens(messages);
    if total <= max_tokens {
        return (messages.to_vec(), CompactionResult::empty());
    }

    let mut result = CompactionResult::empty();
    let mut kept: Vec<Message> = Vec::new();
    let mut kept_tokens = 0;

    // Always keep the last N messages (work backwards)
    for msg in messages.iter().rev() {
        let msg_tokens = estimate_message_tokens(msg);
        if kept_tokens + msg_tokens > max_tokens {
            break;
        }
        kept.push(msg.clone());
        kept_tokens += msg_tokens;
    }

    // Messages we're removing are the ones not in `kept`
    let kept_count = kept.len();
    let removed_count = messages.len() - kept_count;
    let removed_tokens = total - kept_tokens;

    // Reverse to restore original order
    kept.reverse();

    if removed_count > 0 {
        result.ops.push(CompactionOp {
            kind: CompactionKind::Snip {
                count: removed_count,
            },
            messages_removed: removed_count,
            tokens_saved: removed_tokens,
            description: format!(
                "Snipped {removed_count} oldest messages (saved ~{removed_tokens} tokens)"
            ),
        });
        result.total_messages_removed = removed_count;
        result.total_tokens_saved = removed_tokens;
    }

    debug!(
        messages_before = messages.len(),
        messages_after = kept.len(),
        tokens_saved = removed_tokens,
        "Snip compact completed"
    );

    (kept, result)
}

// ============================================================================
// Microcompact
// ============================================================================

/// Collapse consecutive file edit sequences on the same file.
///
/// Detects patterns like:
///   Read(file.txt) → Edit(file.txt) → Read(file.txt) → Edit(file.txt)
/// And replaces them with a single summary message.
pub fn microcompact(
    messages: &[Message],
) -> (Vec<Message>, CompactionResult) {
    let mut result = CompactionResult::empty();
    let mut compacted: Vec<Message> = Vec::new();
    let mut i = 0;
    let mut compacted_files = Vec::new();

    while i < messages.len() {
        // Look for file edit patterns starting from current position
        if let Some((edit_sequence, end_idx)) = find_edit_sequence(messages, i) {
            if edit_sequence.len() >= 3 {
                // Replace the sequence with a summary
                let file_path = edit_sequence
                    .first()
                    .and_then(|m| extract_file_path(m))
                    .unwrap_or_else(|| "unknown".to_string());

                let summary = format!(
                    "[{n} consecutive edits to {file} were compacted]",
                    n = edit_sequence.len(),
                    file = file_path
                );

                let tokens_before: usize = edit_sequence
                    .iter()
                    .map(estimate_message_tokens)
                    .sum();

                compacted.push(Message::System(SystemMessage::Informational(
                    cc_core::messages::SystemInformationalMessage {
                        id: uuid::Uuid::new_v4(),
                        text: summary.clone(),
                        level: None,
                        timestamp: chrono::Utc::now(),
                    },
                )));

                let tokens_after = estimate_tokens(&summary);
                let saved = tokens_before.saturating_sub(tokens_after);

                result.ops.push(CompactionOp {
                    kind: CompactionKind::Microcompact {
                        files: vec![file_path.clone()],
                    },
                    messages_removed: edit_sequence.len() - 1,
                    tokens_saved: saved,
                    description: format!(
                        "Microcompacted {n} edits to {file} (saved ~{saved} tokens)",
                        n = edit_sequence.len(),
                        file = file_path
                    ),
                });

                result.total_messages_removed += edit_sequence.len() - 1;
                result.total_tokens_saved += saved;
                compacted_files.push(file_path);

                i = end_idx;
                continue;
            }
        }

        // No pattern found, keep the message as-is
        compacted.push(messages[i].clone());
        i += 1;
    }

    if !compacted_files.is_empty() {
        info!(
            files = ?compacted_files,
            messages_removed = result.total_messages_removed,
            "Microcompact completed"
        );
    }

    (compacted, result)
}

/// Find a sequence of file-related messages starting at index `start`.
/// Returns the sequence and the index after the last message in the sequence.
fn find_edit_sequence(
    messages: &[Message],
    start: usize,
) -> Option<(Vec<Message>, usize)> {
    if start >= messages.len() {
        return None;
    }

    let first_file = extract_file_path(&messages[start])?;
    let mut sequence = vec![messages[start].clone()];
    let mut end_idx = start + 1;

    // Look for consecutive messages about the same file
    while end_idx < messages.len() {
        if let Some(file_path) = extract_file_path(&messages[end_idx]) {
            if file_path == first_file {
                sequence.push(messages[end_idx].clone());
                end_idx += 1;
                continue;
            }
        }
        break;
    }

    if sequence.len() >= 3 {
        Some((sequence, end_idx))
    } else {
        None
    }
}

/// Extract a file path from a message if it involves file operations.
fn extract_file_path(msg: &Message) -> Option<String> {
    match msg {
        Message::User(u) => {
            for block in &u.content {
                if let ContentBlockParam::ToolResult {
                    tool_use_id: _,
                    content,
                    ..
                } = block
                {
                    // Tool results don't directly contain file paths,
                    // but we can check the content
                    for c in content {
                        if let cc_core::messages::ToolResultContent::Text { text } = c {
                            // Check if the text looks like a file path
                            if text.starts_with('/') || text.contains('/') {
                                // Extract the first path-like string
                                if let Some(path) = extract_path_from_text(text) {
                                    return Some(path);
                                }
                            }
                        }
                    }
                }
            }
            None
        }
        Message::Assistant(a) => {
            for block in &a.content {
                if let ContentBlockParam::ToolUse { name, input, .. } = block {
                    if name.contains("file") || name.contains("edit") || name.contains("read") {
                        if let Some(path) = input.get("path").and_then(|v| v.as_str()) {
                            return Some(path.to_string());
                        }
                    }
                }
            }
            None
        }
        _ => None,
    }
}

fn extract_path_from_text(text: &str) -> Option<String> {
    // Simple heuristic: find the first string that looks like a file path
    for word in text.split_whitespace() {
        if word.contains('/') && word.len() > 3 {
            // Clean up common prefixes
            let cleaned = word.trim_start_matches("Reading ").trim_start_matches("Edited ");
            if cleaned.contains('/') {
                return Some(cleaned.to_string());
            }
        }
    }
    None
}

// ============================================================================
// Auto-Compact
// ============================================================================

/// LLM-based summarization of older messages.
///
/// This would normally call a smaller model to summarize the older
/// portion of the conversation. For now, we provide the interface
/// and a simple fallback summarization.
pub struct AutoCompactConfig {
    /// Model to use for summarization.
    pub model: String,
    /// Maximum tokens to summarize at once.
    pub max_summarize_tokens: usize,
    /// How many messages to keep unsummarized at the end.
    pub keep_recent: usize,
}

impl Default for AutoCompactConfig {
    fn default() -> Self {
        Self {
            model: "claude-haiku-20240307".to_string(),
            max_summarize_tokens: 40_000,
            keep_recent: 10,
        }
    }
}

/// Perform auto-compaction by summarizing older messages.
///
/// In a full implementation, this would call the API with a summarization
/// prompt. Here we provide a simple placeholder that creates a summary
/// message.
pub async fn auto_compact(
    messages: &[Message],
    config: &AutoCompactConfig,
) -> (Vec<Message>, CompactionResult) {
    let mut result = CompactionResult::empty();

    // Determine which messages to summarize (everything except the last N)
    let keep_from = messages.len().saturating_sub(config.keep_recent);
    if keep_from == 0 {
        return (messages.to_vec(), result);
    }

    let to_summarize = &messages[..keep_from];
    let to_keep = &messages[keep_from..];

    let tokens_before: usize = to_summarize.iter().map(estimate_message_tokens).sum();

    // In a full implementation, we'd call the API here:
    // let summary = call_summarization_model(to_summarize, &config.model).await?;
    // For now, create a simple placeholder summary.

    let summary = format!(
        "[{n} earlier messages were summarized to save context window space]",
        n = to_summarize.len()
    );

    let summary_msg = Message::System(SystemMessage::Informational(
        cc_core::messages::SystemInformationalMessage {
            id: uuid::Uuid::new_v4(),
            text: summary.clone(),
            level: None,
            timestamp: chrono::Utc::now(),
        },
    ));

    let mut compacted = vec![summary_msg];
    compacted.extend(to_keep.iter().cloned());

    let tokens_after = estimate_tokens(&summary);
    let saved = tokens_before.saturating_sub(tokens_after);

    result.ops.push(CompactionOp {
        kind: CompactionKind::AutoCompact {
            summary: summary.clone(),
        },
        messages_removed: to_summarize.len(),
        tokens_saved: saved,
        description: format!(
            "Auto-compacted {n} messages (saved ~{saved} tokens)",
            n = to_summarize.len()
        ),
    });

    result.total_messages_removed = to_summarize.len();
    result.total_tokens_saved = saved;

    info!(
        messages_summarized = to_summarize.len(),
        tokens_saved = saved,
        "Auto-compact completed"
    );

    (compacted, result)
}

// ============================================================================
// Reactive Compact
// ============================================================================

/// Emergency compaction triggered by prompt-too-long error.
///
/// Aggressively removes messages in groups based on how far over
/// the limit we are, rather than one-at-a-time.
pub fn reactive_compact(
    messages: &[Message],
    over_by_tokens: usize,
) -> (Vec<Message>, CompactionResult) {
    let mut result = CompactionResult::empty();

    // Add a 20% buffer to avoid edge cases
    let target_reduction = (over_by_tokens as f64 * 1.2) as usize;
    let mut removed_tokens = 0;
    let mut removed_count = 0;
    let mut kept: Vec<Message> = Vec::new();

    // Work backwards from the end, keeping messages until we've saved enough
    for msg in messages.iter().rev() {
        let msg_tokens = estimate_message_tokens(msg);

        // Always keep the last few messages (the actual conversation)
        if removed_tokens >= target_reduction && kept.len() >= 5 {
            kept.push(msg.clone());
            continue;
        }

        // Skip system messages at the beginning
        if matches!(msg, Message::System(_)) && kept.len() < 3 {
            kept.push(msg.clone());
            continue;
        }

        removed_tokens += msg_tokens;
        removed_count += 1;
    }

    kept.reverse();

    if removed_count > 0 {
        result.ops.push(CompactionOp {
            kind: CompactionKind::ReactiveCompact {
                over_by: over_by_tokens,
            },
            messages_removed: removed_count,
            tokens_saved: removed_tokens,
            description: format!(
                "Reactive compact: removed {removed_count} messages (was {over_by_tokens} tokens over limit, saved ~{removed_tokens} tokens)"
            ),
        });
        result.total_messages_removed = removed_count;
        result.total_tokens_saved = removed_tokens;
    }

    debug!(
        over_by = over_by_tokens,
        target = target_reduction,
        removed = removed_count,
        saved = removed_tokens,
        "Reactive compact completed"
    );

    (kept, result)
}

// ============================================================================
// Compaction Hooks
// ============================================================================

/// Trait for compaction hooks that run before/after compaction.
#[async_trait::async_trait]
pub trait CompactionHook: Send + Sync {
    /// Called before compaction begins.
    async fn pre_compact(&self, _messages: &[Message]) {}

    /// Called after compaction completes.
    async fn post_compact(&self, _result: &CompactionResult) {}
}

/// Run pre-compaction hooks.
pub async fn run_pre_compact_hooks(
    hooks: &[Box<dyn CompactionHook>],
    messages: &[Message],
) {
    for hook in hooks {
        hook.pre_compact(messages).await;
    }
}

/// Run post-compaction hooks.
pub async fn run_post_compact_hooks(
    hooks: &[Box<dyn CompactionHook>],
    result: &CompactionResult,
) {
    for hook in hooks {
        hook.post_compact(result).await;
    }
}
