use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::permissions::PermissionMode;
use crate::types::{AgentId, EffortValue, QuerySource};

/// Origin of a message in the conversation.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum MessageOrigin {
    User,
    Assistant,
    System,
    Attachment,
    Progress,
}

/// A content block parameter for API requests.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ContentBlockParam {
    Text { text: String },
    Image {
        source: ImageSource,
    },
    ToolUse {
        id: String,
        name: String,
        input: serde_json::Value,
    },
    ToolResult {
        tool_use_id: String,
        content: Vec<ToolResultContent>,
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

/// Content within a tool result block.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ToolResultContent {
    Text { text: String },
    Image { source: ImageSource },
}

/// Base message trait for all conversation messages.
pub trait MessageTrait: Send + Sync {
    fn id(&self) -> &Uuid;
    fn origin(&self) -> MessageOrigin;
    fn timestamp(&self) -> chrono::DateTime<chrono::Utc>;
}

/// A user message in the conversation.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UserMessage {
    pub id: Uuid,
    pub content: Vec<ContentBlockParam>,
    pub timestamp: chrono::DateTime<chrono::Utc>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub is_meta: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub origin_query_source: Option<QuerySource>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub effort: Option<EffortValue>,
}

/// An assistant message from the model.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AssistantMessage {
    pub id: Uuid,
    pub content: Vec<ContentBlockParam>,
    pub timestamp: chrono::DateTime<chrono::Utc>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub model: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub usage: Option<Usage>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub stop_reason: Option<StopReason>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub is_meta: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub agent_id: Option<AgentId>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Usage {
    pub input_tokens: u64,
    pub output_tokens: u64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cache_read_input_tokens: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cache_creation_input_tokens: Option<u64>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum StopReason {
    EndTurn,
    StopSequence,
    ToolUse,
    MaxTokens,
}

/// An attachment message (files, images, etc.).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AttachmentMessage {
    pub id: Uuid,
    pub attachments: Vec<Attachment>,
    pub timestamp: chrono::DateTime<chrono::Utc>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub is_meta: Option<bool>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Attachment {
    pub path: Option<String>,
    pub content: Option<String>,
    pub mime_type: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub is_image: Option<bool>,
}

/// A progress message for ongoing tool execution.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProgressMessage<P = serde_json::Value> {
    pub id: Uuid,
    pub tool_use_id: String,
    pub data: P,
    pub timestamp: chrono::DateTime<chrono::Utc>,
}

/// Union of all system message variants.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "system_type", rename_all = "snake_case")]
pub enum SystemMessage {
    Informational(SystemInformationalMessage),
    LocalCommand(SystemLocalCommandMessage),
    ApiError(SystemApiErrorMessage),
    CompactBoundary(SystemCompactBoundaryMessage),
    MicrocompactBoundary(SystemMicrocompactBoundaryMessage),
    PermissionRetry(SystemPermissionRetryMessage),
    ApiMetrics(SystemApiMetricsMessage),
    TurnDuration(SystemTurnDurationMessage),
    AgentsKilled(SystemAgentsKilledMessage),
    ScheduledTaskFire(SystemScheduledTaskFireMessage),
    StopHookSummary(SystemStopHookSummaryMessage),
    AwaySummary(SystemAwaySummaryMessage),
    BridgeStatus(SystemBridgeStatusMessage),
    MemorySaved(SystemMemorySavedMessage),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SystemInformationalMessage {
    pub id: Uuid,
    pub text: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub level: Option<SystemMessageLevel>,
    pub timestamp: chrono::DateTime<chrono::Utc>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum SystemMessageLevel {
    #[default]
    Info,
    Warning,
    Error,
    Success,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SystemLocalCommandMessage {
    pub id: Uuid,
    pub command_name: String,
    pub result: String,
    pub timestamp: chrono::DateTime<chrono::Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SystemApiErrorMessage {
    pub id: Uuid,
    pub error: String,
    pub timestamp: chrono::DateTime<chrono::Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SystemCompactBoundaryMessage {
    pub id: Uuid,
    pub direction: CompactDirection,
    pub timestamp: chrono::DateTime<chrono::Utc>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum CompactDirection {
    Forward,
    Backward,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SystemMicrocompactBoundaryMessage {
    pub id: Uuid,
    pub timestamp: chrono::DateTime<chrono::Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SystemPermissionRetryMessage {
    pub id: Uuid,
    pub timestamp: chrono::DateTime<chrono::Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SystemApiMetricsMessage {
    pub id: Uuid,
    pub input_tokens: u64,
    pub output_tokens: u64,
    pub timestamp: chrono::DateTime<chrono::Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SystemTurnDurationMessage {
    pub id: Uuid,
    pub duration_ms: u64,
    pub timestamp: chrono::DateTime<chrono::Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SystemAgentsKilledMessage {
    pub id: Uuid,
    pub agent_ids: Vec<AgentId>,
    pub timestamp: chrono::DateTime<chrono::Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SystemScheduledTaskFireMessage {
    pub id: Uuid,
    pub task_id: String,
    pub timestamp: chrono::DateTime<chrono::Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SystemStopHookSummaryMessage {
    pub id: Uuid,
    pub summary: String,
    pub timestamp: chrono::DateTime<chrono::Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SystemAwaySummaryMessage {
    pub id: Uuid,
    pub summary: String,
    pub timestamp: chrono::DateTime<chrono::Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SystemBridgeStatusMessage {
    pub id: Uuid,
    pub status: String,
    pub timestamp: chrono::DateTime<chrono::Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SystemMemorySavedMessage {
    pub id: Uuid,
    pub memory_count: usize,
    pub timestamp: chrono::DateTime<chrono::Utc>,
}

/// A tombstone message marking a removed/compacted message.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TombstoneMessage {
    pub id: Uuid,
    pub original_id: Uuid,
    pub reason: TombstoneReason,
    pub timestamp: chrono::DateTime<chrono::Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TombstoneReason {
    Compacted,
    Snipped,
    Cancelled,
    Error,
}

/// A summary of tool use for display.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolUseSummaryMessage {
    pub id: Uuid,
    pub tool_name: String,
    pub summary: String,
    pub timestamp: chrono::DateTime<chrono::Utc>,
}

/// A stream event from the API.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "event_type", rename_all = "snake_case")]
pub enum StreamEvent {
    MessageStart { message: AssistantMessage },
    ContentBlockStart { index: usize, content_block: ContentBlockParam },
    ContentBlockDelta { index: usize, delta: ContentBlockDelta },
    ContentBlockStop { index: usize },
    MessageDelta { usage: Usage, stop_reason: Option<StopReason> },
    MessageStop,
    Ping,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ContentBlockDelta {
    TextDelta { text: String },
    ThinkingDelta { thinking: String },
    InputJsonDelta { partial_json: String },
}

/// Union of all message types.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "message_type", rename_all = "snake_case")]
pub enum Message {
    User(UserMessage),
    Assistant(AssistantMessage),
    Attachment(AttachmentMessage),
    Progress(ProgressMessage),
    System(SystemMessage),
    Tombstone(TombstoneMessage),
    ToolUseSummary(ToolUseSummaryMessage),
}

/// Normalized message types for API submission.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "message_type", rename_all = "snake_case")]
pub enum NormalizedMessage {
    User(NormalizedUserMessage),
    Assistant(NormalizedAssistantMessage),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NormalizedUserMessage {
    pub content: Vec<ContentBlockParam>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NormalizedAssistantMessage {
    pub content: Vec<ContentBlockParam>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub model: Option<String>,
}

/// Partial compact direction for compaction hints.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PartialCompactDirection {
    pub direction: CompactDirection,
    pub count: usize,
}

/// Stop hook info for post-sampling hooks.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StopHookInfo {
    pub hook_name: String,
    pub context: serde_json::Value,
}

/// A request start event for tracking.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RequestStartEvent {
    pub timestamp: chrono::DateTime<chrono::Utc>,
    pub model: String,
    pub permission_mode: PermissionMode,
}
