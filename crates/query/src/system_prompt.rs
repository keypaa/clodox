use cc_core::tools::Tools;

use crate::api_types::{CacheControl, SystemPromptBlock, ToolDefinition};

/// Configuration for system prompt assembly.
#[derive(Debug, Clone)]
pub struct SystemPromptConfig {
    /// Custom system prompt that replaces the default.
    pub custom_system_prompt: Option<String>,
    /// Additional system prompt appended after the main prompt.
    pub append_system_prompt: Option<String>,
    /// Whether to include memory mechanics instructions.
    pub include_memory_mechanics: bool,
    /// CLAUDE.md content to inject.
    pub claude_md_content: Option<String>,
    /// Output style configuration.
    pub output_style: Option<String>,
}

impl Default for SystemPromptConfig {
    fn default() -> Self {
        Self {
            custom_system_prompt: None,
            append_system_prompt: None,
            include_memory_mechanics: true,
            claude_md_content: None,
            output_style: None,
        }
    }
}

/// Assemble the system prompt blocks for an API request.
///
/// Returns a vector of `SystemPromptBlock` with cache control annotations
/// for prompt caching optimization.
pub fn assemble_system_prompt(
    config: &SystemPromptConfig,
    tools: &Tools,
) -> Vec<SystemPromptBlock> {
    // If a custom system prompt is provided, use it exclusively
    if let Some(ref custom) = config.custom_system_prompt {
        let mut blocks = vec![
            SystemPromptBlock::Text {
                text: custom.clone(),
                cache_control: None,
            },
        ];
        if let Some(append) = build_append_block(config) {
            blocks.push(append);
        }
        return blocks
            .into_iter()
            .filter(|b| !is_empty_block(b))
            .collect();
    }

    let mut blocks = Vec::new();

    // 1. Base system prompt (core instructions)
    blocks.push(SystemPromptBlock::Text {
        text: build_base_system_prompt(),
        cache_control: None,
    });

    // 2. Tool instructions
    if !tools.is_empty() {
        blocks.push(SystemPromptBlock::Text {
            text: build_tool_instructions(tools),
            cache_control: None,
        });
    }

    // 3. Memory mechanics
    if config.include_memory_mechanics {
        blocks.push(SystemPromptBlock::Text {
            text: build_memory_mechanics(),
            cache_control: None,
        });
    }

    // 4. CLAUDE.md content
    if let Some(ref content) = config.claude_md_content {
        blocks.push(SystemPromptBlock::Text {
            text: format!(
                "# Project Context (CLAUDE.md)\n\n{content}\n\n---\n\nUse this context to inform your understanding of the project."
            ),
            cache_control: None,
        });
    }

    // 5. Output style
    if let Some(ref style) = config.output_style {
        blocks.push(SystemPromptBlock::Text {
            text: format!("# Output Style\n\n{style}"),
            cache_control: None,
        });
    }

    // 6. Append system prompt
    if let Some(append_block) = build_append_block(config) {
        blocks.push(append_block);
    }

    // Add cache control to the last block for prompt caching
    if let Some(last) = blocks.last_mut() {
        *last = add_cache_control(last.clone());
    }

    blocks
}

/// Build the base system prompt with core instructions.
fn build_base_system_prompt() -> String {
    r#"You are Claude, an AI assistant created by Anthropic. You are being used within a software engineering tool called Claude Code.

## Core Instructions

You are a highly capable software engineering assistant. Your primary function is to help users with software engineering tasks including:
- Reading, understanding, and editing code
- Running commands and analyzing output
- Searching codebases for patterns and content
- Managing git workflows
- Debugging and troubleshooting

## Behavior Guidelines

1. **Be direct and concise** — Avoid unnecessary preamble or postamble. Answer the user's question directly.
2. **Show, don't tell** — Demonstrate changes through code rather than describing them.
3. **Follow conventions** — Match the existing code style, frameworks, and patterns in the codebase.
4. **Be thorough** — When making changes, consider edge cases, error handling, and testing.
5. **Ask when uncertain** — If a request is ambiguous, clarify before making changes.

## Important Notes

- You have access to tools that allow you to interact with the user's system.
- Always verify your changes work by running tests or commands when appropriate.
- When editing files, make precise, targeted changes.
- Preserve existing functionality unless explicitly asked to change it.
- If you encounter errors, analyze them carefully and try to fix the root cause."#.to_string()
}

/// Build tool instructions section.
fn build_tool_instructions(tools: &Tools) -> String {
    let mut instructions = String::from("## Available Tools\n\n");
    instructions.push_str("You have access to the following tools. Use them to accomplish the user's goals.\n\n");

    for tool in tools.iter() {
        instructions.push_str(&format!(
            "- **{}**: {}\n",
            tool.name(),
            tool.get_activity_description(None).unwrap_or_else(|| "A tool".to_string())
        ));
    }

    instructions.push_str(
        "\n### Tool Usage Guidelines\n\n\
        1. Use tools to gather information before making changes.\n\
        2. Read files before editing them to understand the current state.\n\
        3. Run tests after making changes to verify correctness.\n\
        4. Use search tools (grep, glob) to find relevant code.\n\
        5. When in doubt, read more context before making changes.\n",
    );

    instructions
}

/// Build memory mechanics instructions.
fn build_memory_mechanics() -> String {
    r#"## Memory

You can save important information to memory for use in future sessions. Use memory to:
- Remember user preferences and working styles
- Store project-specific knowledge that isn't in code files
- Capture lessons learned from debugging sessions

Save memories proactively when the user shares preferences or when you discover non-obvious project knowledge. Do not save information that is already in the codebase or that changes frequently."#.to_string()
}

/// Build the append system prompt block.
fn build_append_block(config: &SystemPromptConfig) -> Option<SystemPromptBlock> {
    config.append_system_prompt.as_ref().map(|prompt| {
        SystemPromptBlock::Text {
            text: prompt.clone(),
            cache_control: None,
        }
    })
}

/// Add cache control annotation to a block for prompt caching.
fn add_cache_control(block: SystemPromptBlock) -> SystemPromptBlock {
    match block {
        SystemPromptBlock::Text { text, .. } => SystemPromptBlock::Text {
            text,
            cache_control: Some(CacheControl {
                cache_type: "ephemeral".to_string(),
            }),
        },
    }
}

/// Check if a system prompt block is empty.
fn is_empty_block(block: &SystemPromptBlock) -> bool {
    match block {
        SystemPromptBlock::Text { text, .. } => text.trim().is_empty(),
    }
}

/// Build tool definitions for the API from the tool registry.
pub fn build_tool_definitions(tools: &Tools) -> Vec<ToolDefinition> {
    tools
        .iter()
        .filter(|t| t.is_enabled())
        .map(|t| {
            ToolDefinition {
                name: t.name().to_string(),
                description: t
                    .get_activity_description(None)
                    .unwrap_or_else(|| "A tool".to_string())
                    .to_string(),
                input_schema: t.input_schema(),
                cache_control: None,
            }
        })
        .collect()
}

/// Build tool definitions with cache control on the last tool.
pub fn build_tool_definitions_with_caching(tools: &Tools) -> Vec<ToolDefinition> {
    let mut defs = build_tool_definitions(tools);

    // Add cache control to the last tool definition
    if let Some(last) = defs.last_mut() {
        last.cache_control = Some(CacheControl {
            cache_type: "ephemeral".to_string(),
        });
    }

    defs
}
