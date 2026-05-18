use std::sync::Arc;

use cc_core::messages::{
    AssistantMessage, ContentBlockParam, Message, StopReason, StreamEvent as CoreStreamEvent,
    UserMessage,
};
use cc_core::permissions::ToolPermissionContext;
use cc_core::tools::Tools;
use futures::Stream;
use futures::StreamExt;
use tokio::sync::watch;
use uuid::Uuid;

use crate::api_client::{build_request, ApiClient, ApiConfig};
use crate::api_types::{
    MessageParam, Role, StreamingMessage, SystemPromptBlock,
    ToolDefinition, ToolChoice,
};
use crate::errors::QueryError;
use crate::retry::RetryOptions;
use crate::streaming::StreamingToolExecutor;

/// Token budget tracking for a conversation.
#[derive(Debug, Clone)]
pub struct TokenBudget {
    pub max_total: Option<u64>,
    pub used: u64,
    pub result_budget: Option<u64>,
    pub task_budget: Option<u64>,
}

impl TokenBudget {
    pub fn new(max_total: Option<u64>) -> Self {
        Self {
            max_total,
            used: 0,
            result_budget: None,
            task_budget: None,
        }
    }

    pub fn remaining(&self) -> Option<u64> {
        self.max_total.map(|max| max.saturating_sub(self.used))
    }

    pub fn is_exhausted(&self) -> bool {
        self.remaining().map(|r| r == 0).unwrap_or(false)
    }

    pub fn add_usage(&mut self, input: u64, output: u64) {
        self.used += input + output;
    }
}

/// Compaction state for the query loop.
#[derive(Debug, Clone, Default)]
pub struct CompactionState {
    pub compacted_this_turn: bool,
    pub messages_compacted: usize,
    pub last_reason: Option<String>,
}

/// Recovery state for handling API errors.
#[derive(Debug, Clone, Default)]
pub enum RecoveryState {
    #[default]
    None,
    PromptTooLong {
        actual_tokens: u64,
        limit_tokens: u64,
        retry_count: usize,
    },
    MaxOutputTokens {
        retry_count: usize,
    },
    ModelFallback {
        from_model: String,
        to_model: String,
    },
}

/// The main query engine state.
pub struct QueryState {
    pub messages: Vec<Message>,
    pub token_budget: TokenBudget,
    pub compaction_state: CompactionState,
    pub recovery_state: RecoveryState,
    pub abort_signal: watch::Receiver<bool>,
    pub consecutive_errors: usize,
}

impl QueryState {
    pub fn new(abort_signal: watch::Receiver<bool>) -> Self {
        Self {
            messages: Vec::new(),
            token_budget: TokenBudget::new(None),
            compaction_state: CompactionState::default(),
            recovery_state: RecoveryState::default(),
            abort_signal,
            consecutive_errors: 0,
        }
    }

    pub fn is_aborted(&self) -> bool {
        *self.abort_signal.borrow()
    }
}

/// Configuration for a query.
#[derive(Clone)]
pub struct QueryConfig {
    pub model: String,
    pub max_tokens: u64,
    pub system_prompt: Vec<SystemPromptBlock>,
    pub tools: Tools,
    pub permission_context: ToolPermissionContext,
    pub temperature: Option<f64>,
    pub thinking_enabled: bool,
    pub thinking_budget: Option<u64>,
    pub token_budget: TokenBudget,
    pub api_config: ApiConfig,
    pub retry_options: RetryOptions,
    pub verbose: bool,
    pub debug: bool,
}

/// Information about a tool call from the model.
#[derive(Debug, Clone)]
pub struct ToolCallInfo {
    pub id: String,
    pub name: String,
    pub input: serde_json::Value,
}

/// Events yielded by the query engine.
#[derive(Debug)]
pub enum QueryEvent {
    Stream(CoreStreamEvent),
    TurnComplete { message: AssistantMessage },
    ToolCallsPending {
        message: AssistantMessage,
        tool_calls: Vec<ToolCallInfo>,
    },
    ToolResult {
        tool_call_id: String,
        tool_name: String,
        success: bool,
    },
    MaxTokensReached { message: AssistantMessage },
    Aborted,
}

/// The QueryEngine manages a single conversation.
pub struct QueryEngine {
    config: QueryConfig,
    state: QueryState,
    api_client: ApiClient,
    tool_executor: Arc<StreamingToolExecutor>,
}

impl QueryEngine {
    pub fn new(config: QueryConfig, abort_signal: watch::Receiver<bool>) -> Result<Self, QueryError> {
        let api_client = ApiClient::new(config.api_config.clone())?;
        let tool_executor = Arc::new(StreamingToolExecutor::new(
            config.tools.clone(),
        ));

        Ok(Self {
            config,
            state: QueryState::new(abort_signal),
            api_client,
            tool_executor,
        })
    }

    /// Submit a user message and run the query loop.
    pub async fn submit_message(
        &mut self,
        user_message: UserMessage,
    ) -> impl Stream<Item = Result<QueryEvent, QueryError>> + Send + 'static {
        self.state.messages.push(Message::User(user_message));
        self.run_query_loop()
    }

    /// The core query loop.
    fn run_query_loop(
        &mut self,
    ) -> impl Stream<Item = Result<QueryEvent, QueryError>> + Send + 'static {
        // Move everything needed into the stream
        let config = self.config.clone();
        let mut state_messages = self.state.messages.clone();
        let abort_signal = self.state.abort_signal.clone();
        let tool_executor = self.tool_executor.clone();
        let api_config = self.config.api_config.clone();

        let stream = async_stream::stream! {
            let current_model = config.model.clone();
            let mut max_consecutive_errors = 0;

            loop {
                if *abort_signal.borrow() {
                    yield Ok(QueryEvent::Aborted);
                    return;
                }

                // Build API request
                let request = build_api_request_from_messages(
                    &current_model,
                    &state_messages,
                    &config,
                );

                // Stream from API
                let mut accumulator = StreamingMessage::new();
                let mut tool_calls: Vec<ToolCallInfo> = Vec::new();
                let mut stream_had_error = false;

                let api_client = match ApiClient::new(api_config.clone()) {
                    Ok(c) => c,
                    Err(e) => {
                        yield Err(e);
                        return;
                    }
                };

                let retry_opts = RetryOptions {
                    max_retries: config.retry_options.max_retries,
                    model: current_model.clone(),
                    fallback_model: config.retry_options.fallback_model.clone(),
                    initial_consecutive_529: max_consecutive_errors,
                };

                let mut event_stream = api_client.stream_message_with_retry(
                    request,
                    retry_opts,
                );

                while let Some(event_result) = event_stream.next().await {
                    match event_result {
                        Ok(event) => {
                            accumulator.apply_event(&event);

                            if let Some(core_event) = convert_stream_event(&event) {
                                yield Ok(QueryEvent::Stream(core_event));
                            }

                            if let crate::api_types::StreamEvent::ContentBlockStart { content_block, .. } = &event {
                                if let crate::api_types::StreamContentBlock::ToolUse { id, name, input } = content_block {
                                    tool_calls.push(ToolCallInfo {
                                        id: id.clone(),
                                        name: name.clone(),
                                        input: input.clone(),
                                    });
                                }
                            }
                        }
                        Err(e) => {
                            max_consecutive_errors += 1;
                            stream_had_error = true;
                            yield Err(e);
                            return;
                        }
                    }
                }

                if stream_had_error {
                    return;
                }

                max_consecutive_errors = 0;

                let assistant_message = AssistantMessage {
                    id: Uuid::new_v4(),
                    content: accumulator.to_content_blocks(),
                    timestamp: chrono::Utc::now(),
                    model: Some(accumulator.model.clone()),
                    usage: accumulator.usage.clone().map(|u| cc_core::messages::Usage {
                        input_tokens: u.input_tokens,
                        output_tokens: u.output_tokens,
                        cache_read_input_tokens: u.cache_read_input_tokens,
                        cache_creation_input_tokens: u.cache_creation_input_tokens,
                    }),
                    stop_reason: accumulator.stop_reason.as_deref().map(|s| match s.as_ref() {
                        "end_turn" => StopReason::EndTurn,
                        "stop_sequence" => StopReason::StopSequence,
                        "tool_use" => StopReason::ToolUse,
                        "max_tokens" => StopReason::MaxTokens,
                        _ => StopReason::EndTurn,
                    }),
                    is_meta: None,
                    agent_id: None,
                };

                state_messages.push(Message::Assistant(assistant_message.clone()));

                match assistant_message.stop_reason {
                    Some(StopReason::EndTurn) | Some(StopReason::StopSequence) => {
                        yield Ok(QueryEvent::TurnComplete {
                            message: assistant_message,
                        });
                        return;
                    }
                    Some(StopReason::MaxTokens) => {
                        yield Ok(QueryEvent::MaxTokensReached {
                            message: assistant_message,
                        });
                        return;
                    }
                    Some(StopReason::ToolUse) if !tool_calls.is_empty() => {
                        yield Ok(QueryEvent::ToolCallsPending {
                            message: assistant_message.clone(),
                            tool_calls: tool_calls.clone(),
                        });

                        let tool_results = tool_executor
                            .execute_all(&tool_calls)
                            .await;

                        for (call, result) in tool_calls.iter().zip(tool_results.iter()) {
                            match result {
                                Ok(output) => {
                                    let tool_result_message = Message::User(UserMessage {
                                        id: Uuid::new_v4(),
                                        content: vec![ContentBlockParam::ToolResult {
                                            tool_use_id: call.id.clone(),
                                            content: vec![cc_core::messages::ToolResultContent::Text {
                                                text: serde_json::to_string(output)
                                                    .unwrap_or_else(|_| String::new()),
                                            }],
                                            is_error: Some(false),
                                        }],
                                        timestamp: chrono::Utc::now(),
                                        is_meta: None,
                                        origin_query_source: None,
                                        effort: None,
                                    });
                                    state_messages.push(tool_result_message);
                                    yield Ok(QueryEvent::ToolResult {
                                        tool_call_id: call.id.clone(),
                                        tool_name: call.name.clone(),
                                        success: true,
                                    });
                                }
                                Err(e) => {
                                    let tool_result_message = Message::User(UserMessage {
                                        id: Uuid::new_v4(),
                                        content: vec![ContentBlockParam::ToolResult {
                                            tool_use_id: call.id.clone(),
                                            content: vec![cc_core::messages::ToolResultContent::Text {
                                                text: format!("Error: {e}"),
                                            }],
                                            is_error: Some(true),
                                        }],
                                        timestamp: chrono::Utc::now(),
                                        is_meta: None,
                                        origin_query_source: None,
                                        effort: None,
                                    });
                                    state_messages.push(tool_result_message);
                                    yield Ok(QueryEvent::ToolResult {
                                        tool_call_id: call.id.clone(),
                                        tool_name: call.name.clone(),
                                        success: false,
                                    });
                                }
                            }
                        }

                        continue;
                    }
                    _ => {
                        yield Ok(QueryEvent::TurnComplete {
                            message: assistant_message,
                        });
                        return;
                    }
                }
            }
        };

        stream
    }

    pub fn messages(&self) -> &[Message] {
        &self.state.messages
    }

    pub fn token_budget(&self) -> &TokenBudget {
        &self.state.token_budget
    }
}

/// Build an API request from the current message history.
fn build_api_request_from_messages(
    model: &str,
    messages: &[Message],
    config: &QueryConfig,
) -> crate::api_types::MessageRequest {
    let api_messages: Vec<MessageParam> = messages
        .iter()
        .filter_map(|msg| match msg {
            Message::User(u) => Some(MessageParam {
                role: Role::User,
                content: u.content.iter().map(content_block_to_api).collect(),
            }),
            Message::Assistant(a) => Some(MessageParam {
                role: Role::Assistant,
                content: a.content.iter().map(content_block_to_api).collect(),
            }),
            _ => None,
        })
        .collect();

    let tools: Option<Vec<ToolDefinition>> = if !config.tools.is_empty() {
        Some(config.tools.iter().filter_map(|t| {
            let tool = t.as_ref();
            Some(ToolDefinition {
                name: tool.name().to_string(),
                description: String::new(),
                input_schema: tool.input_schema(),
                cache_control: None,
            })
        }).collect())
    } else {
        None
    };

    let thinking = if config.thinking_enabled {
        config.thinking_budget.map(|budget| {
            crate::api_types::ThinkingConfig {
                thinking_type: "enabled".to_string(),
                budget_tokens: budget,
            }
        })
    } else {
        None
    };

    build_request(
        model,
        config.max_tokens,
        api_messages,
        Some(config.system_prompt.clone()),
        tools,
        Some(ToolChoice::Auto),
        config.temperature,
        thinking,
    )
}

fn content_block_to_api(block: &ContentBlockParam) -> crate::api_types::ContentBlock {
    match block {
        ContentBlockParam::Text { text } => crate::api_types::ContentBlock::Text {
            text: text.clone(),
        },
        ContentBlockParam::Image { source } => crate::api_types::ContentBlock::Image {
            source: crate::api_types::ImageSource {
                source_type: source.source_type.clone(),
                media_type: source.media_type.clone(),
                data: source.data.clone(),
            },
        },
        ContentBlockParam::ToolUse { id, name, input } => {
            crate::api_types::ContentBlock::ToolUse {
                id: id.clone(),
                name: name.clone(),
                input: input.clone(),
            }
        }
        ContentBlockParam::ToolResult {
            tool_use_id,
            content,
            is_error,
        } => crate::api_types::ContentBlock::ToolResult {
            tool_use_id: tool_use_id.clone(),
            content: content
                .iter()
                .map(|c| match c {
                    cc_core::messages::ToolResultContent::Text { text } => {
                        crate::api_types::ToolResultContentBlock::Text {
                            text: text.clone(),
                        }
                    }
                    cc_core::messages::ToolResultContent::Image { source } => {
                        crate::api_types::ToolResultContentBlock::Image {
                            source: crate::api_types::ImageSource {
                                source_type: source.source_type.clone(),
                                media_type: source.media_type.clone(),
                                data: source.data.clone(),
                            },
                        }
                    }
                })
                .collect(),
            is_error: *is_error,
        },
        ContentBlockParam::Thinking { thinking, signature } => {
            crate::api_types::ContentBlock::Thinking {
                thinking: thinking.clone(),
                signature: signature.clone(),
            }
        }
        ContentBlockParam::RedactedThinking { data } => {
            crate::api_types::ContentBlock::RedactedThinking {
                data: data.clone(),
            }
        }
    }
}

fn convert_stream_event(event: &crate::api_types::StreamEvent) -> Option<CoreStreamEvent> {
    match event {
        crate::api_types::StreamEvent::MessageStart { message } => {
            Some(CoreStreamEvent::MessageStart {
                message: AssistantMessage {
                    id: Uuid::new_v4(),
                    content: Vec::new(),
                    timestamp: chrono::Utc::now(),
                    model: Some(message.model.clone()),
                    usage: Some(cc_core::messages::Usage {
                        input_tokens: message.usage.input_tokens,
                        output_tokens: message.usage.output_tokens,
                        cache_read_input_tokens: message.usage.cache_read_input_tokens,
                        cache_creation_input_tokens: message.usage.cache_creation_input_tokens,
                    }),
                    stop_reason: message.stop_reason.as_deref().map(|s| match s.as_ref() {
                        "end_turn" => StopReason::EndTurn,
                        "tool_use" => StopReason::ToolUse,
                        "max_tokens" => StopReason::MaxTokens,
                        _ => StopReason::EndTurn,
                    }),
                    is_meta: None,
                    agent_id: None,
                },
            })
        }
        crate::api_types::StreamEvent::ContentBlockDelta { index, delta } => {
            let core_delta = match delta {
                crate::api_types::ContentBlockDelta::TextDelta { text } => {
                    cc_core::messages::ContentBlockDelta::TextDelta { text: text.clone() }
                }
                crate::api_types::ContentBlockDelta::ThinkingDelta { thinking } => {
                    cc_core::messages::ContentBlockDelta::ThinkingDelta { thinking: thinking.clone() }
                }
                crate::api_types::ContentBlockDelta::InputJsonDelta { partial_json } => {
                    cc_core::messages::ContentBlockDelta::InputJsonDelta {
                        partial_json: partial_json.clone(),
                    }
                }
            };
            Some(CoreStreamEvent::ContentBlockDelta {
                index: *index,
                delta: core_delta,
            })
        }
        crate::api_types::StreamEvent::MessageDelta { delta, usage } => {
            Some(CoreStreamEvent::MessageDelta {
                usage: cc_core::messages::Usage {
                    input_tokens: 0,
                    output_tokens: usage.output_tokens,
                    cache_read_input_tokens: None,
                    cache_creation_input_tokens: None,
                },
                stop_reason: delta.stop_reason.as_deref().map(|s| match s.as_ref() {
                    "end_turn" => StopReason::EndTurn,
                    "tool_use" => StopReason::ToolUse,
                    "max_tokens" => StopReason::MaxTokens,
                    _ => StopReason::EndTurn,
                }),
            })
        }
        crate::api_types::StreamEvent::MessageStop => Some(CoreStreamEvent::MessageStop),
        crate::api_types::StreamEvent::Ping => Some(CoreStreamEvent::Ping),
        _ => None,
    }
}
