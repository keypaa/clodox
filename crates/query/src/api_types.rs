use serde::{Deserialize, Serialize};

use cc_core::messages::ContentBlockParam;

// ============================================================================
// Request Types
// ============================================================================

/// Anthropic Messages API request body.
#[derive(Debug, Clone, Serialize)]
pub struct MessageRequest {
    pub model: String,
    pub max_tokens: u64,
    pub messages: Vec<MessageParam>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub system: Option<Vec<SystemPromptBlock>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tools: Option<Vec<ToolDefinition>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tool_choice: Option<ToolChoice>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub temperature: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub top_p: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub top_k: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub thinking: Option<ThinkingConfig>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub metadata: Option<RequestMetadata>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub stop_sequences: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub stream: Option<bool>,
    #[serde(flatten)]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub extra: Option<serde_json::Value>,
}

/// A single message in the conversation history.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MessageParam {
    pub role: Role,
    pub content: Vec<ContentBlock>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Role {
    User,
    Assistant,
}

/// Content block within a message.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ContentBlock {
    Text { text: String },
    Image { source: ImageSource },
    ToolUse {
        id: String,
        name: String,
        input: serde_json::Value,
    },
    ToolResult {
        tool_use_id: String,
        content: Vec<ToolResultContentBlock>,
        #[serde(skip_serializing_if = "Option::is_none")]
        is_error: Option<bool>,
    },
    Thinking {
        thinking: String,
        signature: String,
    },
    RedactedThinking {
        data: String,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ImageSource {
    #[serde(rename = "type")]
    pub source_type: String,
    pub media_type: String,
    pub data: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ToolResultContentBlock {
    Text { text: String },
    Image { source: ImageSource },
}

/// System prompt block (supports caching annotations).
#[derive(Debug, Clone, Serialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum SystemPromptBlock {
    Text {
        text: String,
        #[serde(skip_serializing_if = "Option::is_none")]
        cache_control: Option<CacheControl>,
    },
}

#[derive(Debug, Clone, Serialize)]
pub struct CacheControl {
    #[serde(rename = "type")]
    pub cache_type: String, // "ephemeral"
}

// ============================================================================
// Tool Definitions
// ============================================================================

#[derive(Debug, Clone, Serialize)]
pub struct ToolDefinition {
    pub name: String,
    pub description: String,
    pub input_schema: serde_json::Value,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cache_control: Option<CacheControl>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ToolChoice {
    Auto,
    Any,
    Tool { name: String },
}

// ============================================================================
// Thinking Configuration
// ============================================================================

#[derive(Debug, Clone, Serialize)]
pub struct ThinkingConfig {
    #[serde(rename = "type")]
    pub thinking_type: String, // "enabled"
    pub budget_tokens: u64,
}

// ============================================================================
// Request Metadata
// ============================================================================

#[derive(Debug, Clone, Serialize)]
pub struct RequestMetadata {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub user_id: Option<String>,
}

// ============================================================================
// Response Types (non-streaming)
// ============================================================================

#[derive(Debug, Clone, Deserialize)]
pub struct MessageResponse {
    pub id: String,
    #[serde(rename = "type")]
    pub response_type: String,
    pub role: String,
    pub content: Vec<ResponseContentBlock>,
    pub model: String,
    pub stop_reason: Option<String>,
    pub stop_sequence: Option<String>,
    pub usage: Usage,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ResponseContentBlock {
    Text { text: String },
    ToolUse {
        id: String,
        name: String,
        input: serde_json::Value,
    },
    Thinking {
        thinking: String,
        signature: String,
    },
    RedactedThinking {
        data: String,
    },
}

#[derive(Debug, Clone, Deserialize)]
pub struct Usage {
    pub input_tokens: u64,
    pub output_tokens: u64,
    #[serde(default)]
    pub cache_read_input_tokens: Option<u64>,
    #[serde(default)]
    pub cache_creation_input_tokens: Option<u64>,
}

// ============================================================================
// Streaming Event Types
// ============================================================================

/// A single SSE event from the streaming API.
#[derive(Debug, Clone, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum StreamEvent {
    MessageStart {
        message: StreamMessageStart,
    },
    ContentBlockStart {
        index: usize,
        content_block: StreamContentBlock,
    },
    ContentBlockDelta {
        index: usize,
        delta: ContentBlockDelta,
    },
    ContentBlockStop {
        index: usize,
    },
    MessageDelta {
        delta: MessageDelta,
        usage: MessageDeltaUsage,
    },
    MessageStop,
    Ping,
}

#[derive(Debug, Clone, Deserialize)]
pub struct StreamMessageStart {
    pub id: String,
    pub role: String,
    pub content: Vec<serde_json::Value>,
    pub model: String,
    pub stop_reason: Option<String>,
    pub stop_sequence: Option<String>,
    pub usage: Usage,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum StreamContentBlock {
    Text { text: String },
    ToolUse {
        id: String,
        name: String,
        input: serde_json::Value,
    },
    Thinking {
        thinking: String,
        signature: String,
    },
    RedactedThinking {
        data: String,
    },
}

#[derive(Debug, Clone, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ContentBlockDelta {
    TextDelta { text: String },
    ThinkingDelta { thinking: String },
    InputJsonDelta { partial_json: String },
}

#[derive(Debug, Clone, Deserialize)]
pub struct MessageDelta {
    pub stop_reason: Option<String>,
    pub stop_sequence: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct MessageDeltaUsage {
    pub output_tokens: u64,
}

// ============================================================================
// Accumulated State During Streaming
// ============================================================================

/// Accumulates streaming events into a complete message.
#[derive(Debug, Clone, Default)]
pub struct StreamingMessage {
    pub id: String,
    pub model: String,
    pub content_blocks: Vec<AccumulatedContentBlock>,
    pub stop_reason: Option<String>,
    pub stop_sequence: Option<String>,
    pub usage: Option<Usage>,
}

#[derive(Debug, Clone)]
pub enum AccumulatedContentBlock {
    Text { text: String },
    ToolUse {
        id: String,
        name: String,
        input_json: String,
    },
    Thinking {
        thinking: String,
        signature: String,
    },
    RedactedThinking {
        data: String,
    },
}

impl StreamingMessage {
    pub fn new() -> Self {
        Self::default()
    }

    /// Apply a streaming event to this accumulator.
    pub fn apply_event(&mut self, event: &StreamEvent) {
        match event {
            StreamEvent::MessageStart { message } => {
                self.id.clone_from(&message.id);
                self.model.clone_from(&message.model);
                self.usage = Some(message.usage.clone());
            }
            StreamEvent::ContentBlockStart { content_block, .. } => {
                let block = match content_block {
                    StreamContentBlock::Text { text } => {
                        AccumulatedContentBlock::Text {
                            text: text.clone(),
                        }
                    }
                    StreamContentBlock::ToolUse { id, name, input } => {
                        AccumulatedContentBlock::ToolUse {
                            id: id.clone(),
                            name: name.clone(),
                            input_json: serde_json::to_string(input)
                                .unwrap_or_default(),
                        }
                    }
                    StreamContentBlock::Thinking {
                        thinking,
                        signature,
                    } => AccumulatedContentBlock::Thinking {
                        thinking: thinking.clone(),
                        signature: signature.clone(),
                    },
                    StreamContentBlock::RedactedThinking { data } => {
                        AccumulatedContentBlock::RedactedThinking {
                            data: data.clone(),
                        }
                    }
                };
                self.content_blocks.push(block);
            }
            StreamEvent::ContentBlockDelta { index, delta } => {
                if let Some(block) = self.content_blocks.get_mut(*index) {
                    match (block, delta) {
                        (
                            AccumulatedContentBlock::Text { text },
                            ContentBlockDelta::TextDelta { text: delta_text },
                        ) => {
                            text.push_str(delta_text);
                        }
                        (
                            AccumulatedContentBlock::ToolUse { input_json, .. },
                            ContentBlockDelta::InputJsonDelta { partial_json },
                        ) => {
                            input_json.push_str(partial_json);
                        }
                        (
                            AccumulatedContentBlock::Thinking { thinking, .. },
                            ContentBlockDelta::ThinkingDelta { thinking: delta_thinking },
                        ) => {
                            thinking.push_str(delta_thinking);
                        }
                        _ => {}
                    }
                }
            }
            StreamEvent::MessageDelta { delta, usage } => {
                self.stop_reason.clone_from(&delta.stop_reason);
                self.stop_sequence.clone_from(&delta.stop_sequence);
                if let Some(existing) = &mut self.usage {
                    existing.output_tokens = usage.output_tokens;
                }
            }
            _ => {}
        }
    }

    /// Convert accumulated blocks to API-compatible content blocks.
    pub fn to_content_blocks(&self) -> Vec<ContentBlockParam> {
        self.content_blocks
            .iter()
            .filter_map(|block| match block {
                AccumulatedContentBlock::Text { text } => {
                    Some(ContentBlockParam::Text { text: text.clone() })
                }
                AccumulatedContentBlock::ToolUse {
                    id,
                    name,
                    input_json,
                } => {
                    let input: serde_json::Value =
                        serde_json::from_str(input_json).unwrap_or(
                            serde_json::Value::Object(Default::default()),
                        );
                    Some(ContentBlockParam::ToolUse {
                        id: id.clone(),
                        name: name.clone(),
                        input,
                    })
                }
                AccumulatedContentBlock::Thinking {
                    thinking,
                    signature,
                } => Some(ContentBlockParam::Thinking {
                    thinking: thinking.clone(),
                    signature: signature.clone(),
                }),
                AccumulatedContentBlock::RedactedThinking { data } => {
                    Some(ContentBlockParam::RedactedThinking {
                        data: data.clone(),
                    })
                }
            })
            .collect()
    }
}
