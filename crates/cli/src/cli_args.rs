use std::path::PathBuf;

use clap::{Args, Parser, Subcommand, ValueEnum};

/// Claude Code — an AI-powered software engineering assistant.
#[derive(Parser, Debug)]
#[command(
    name = "claude",
    about = "Claude Code — an AI-powered software engineering assistant",
    long_about = None,
    version = env!("CARGO_PKG_VERSION"),
)]
pub struct Cli {
    /// Optional prompt text. If omitted, enters interactive mode.
    #[arg(value_name = "PROMPT")]
    pub prompt: Option<String>,

    /// Enable debug mode with optional category filtering.
    #[arg(short = 'd', long, value_name = "FILTER", num_args = 0..=1, default_missing_value = "")]
    pub debug: Option<String>,

    /// Enable debug mode to stderr.
    #[arg(long, hide = true)]
    pub debug_to_stderr: bool,

    /// Write debug logs to specific file.
    #[arg(long, value_name = "PATH")]
    pub debug_file: Option<PathBuf>,

    /// Override verbose mode from config.
    #[arg(long)]
    pub verbose: bool,

    /// Print response and exit (non-interactive mode).
    #[arg(short = 'p', long)]
    pub print: bool,

    /// Output format (only valid with --print).
    #[arg(long, value_name = "FORMAT", default_value = "text")]
    pub output_format: OutputFormat,

    /// Input format.
    #[arg(long, value_name = "FORMAT")]
    pub input_format: Option<InputFormat>,

    /// JSON Schema for structured output.
    #[arg(long, value_name = "SCHEMA")]
    pub json_schema: Option<String>,

    /// Include hook lifecycle events in stream.
    #[arg(long)]
    pub include_hook_events: bool,

    /// Include partial messages in stream.
    #[arg(long)]
    pub include_partial_messages: bool,

    /// Maximum number of turns.
    #[arg(long, value_name = "N")]
    pub max_turns: Option<u32>,

    /// Maximum USD budget.
    #[arg(long, value_name = "AMOUNT")]
    pub max_budget_usd: Option<f64>,

    /// API-side task budget in tokens.
    #[arg(long, value_name = "TOKENS", hide = true)]
    pub task_budget: Option<u64>,

    /// Re-emit user messages on stdout.
    #[arg(long)]
    pub replay_user_messages: bool,

    /// Tool names to allow.
    #[arg(long, value_name = "TOOLS", num_args = 1..)]
    pub allowed_tools: Vec<String>,

    /// Specify available tools.
    #[arg(long, value_name = "TOOLS", num_args = 1..)]
    pub tools: Vec<String>,

    /// Tool names to deny.
    #[arg(long, value_name = "TOOLS", num_args = 1..)]
    pub disallowed_tools: Vec<String>,

    /// Load MCP servers from JSON files/strings.
    #[arg(long, value_name = "CONFIGS", num_args = 1..)]
    pub mcp_config: Vec<String>,

    /// System prompt for session.
    #[arg(long, value_name = "PROMPT")]
    pub system_prompt: Option<String>,

    /// Read system prompt from file.
    #[arg(long, value_name = "FILE")]
    pub system_prompt_file: Option<PathBuf>,

    /// Append to default system prompt.
    #[arg(long, value_name = "PROMPT")]
    pub append_system_prompt: Option<String>,

    /// Permission mode.
    #[arg(long, value_name = "MODE")]
    pub permission_mode: Option<PermissionModeArg>,

    /// Continue most recent conversation.
    #[arg(short = 'c', long)]
    pub r#continue: bool,

    /// Resume by session ID or open picker.
    #[arg(short = 'r', long, value_name = "VALUE", num_args = 0..=1, default_missing_value = "")]
    pub resume: Option<String>,

    /// Create new session when resuming.
    #[arg(long)]
    pub fork_session: bool,

    /// Model for session (alias or full name).
    #[arg(long, value_name = "MODEL")]
    pub model: Option<String>,

    /// Effort level.
    #[arg(long, value_name = "LEVEL")]
    pub effort: Option<EffortArg>,

    /// Agent for session.
    #[arg(long, value_name = "AGENT")]
    pub agent: Option<String>,

    /// Beta headers for API.
    #[arg(long, value_name = "BETAS", num_args = 1..)]
    pub betas: Vec<String>,

    /// Fallback model (only valid with --print).
    #[arg(long, value_name = "MODEL")]
    pub fallback_model: Option<String>,

    /// Path to settings JSON file or JSON string.
    #[arg(long, value_name = "FILE_OR_JSON")]
    pub settings: Option<String>,

    /// Additional directories for tool access.
    #[arg(long, value_name = "DIRECTORIES", num_args = 1..)]
    pub add_dir: Vec<String>,

    /// Auto-connect to IDE on startup.
    #[arg(long)]
    pub ide: bool,

    /// Only use MCP from --mcp-config.
    #[arg(long)]
    pub strict_mcp_config: bool,

    /// Specific session UUID.
    #[arg(long, value_name = "UUID")]
    pub session_id: Option<String>,

    /// Display name for session.
    #[arg(short = 'n', long, value_name = "NAME")]
    pub name: Option<String>,

    /// JSON defining custom agents.
    #[arg(long, value_name = "JSON")]
    pub agents: Option<String>,

    /// Comma-separated setting sources: user, project, local.
    #[arg(long, value_name = "SOURCES")]
    pub setting_sources: Option<String>,

    /// Load plugins from directory.
    #[arg(long, value_name = "PATH", num_args = 1..)]
    pub plugin_dir: Vec<PathBuf>,

    /// Disable all skills/slash commands.
    #[arg(long)]
    pub disable_slash_commands: bool,

    /// Enable/disable Claude in Chrome.
    #[arg(long)]
    pub chrome: Option<bool>,

    /// File resources to download at startup.
    #[arg(long, value_name = "SPECS", num_args = 1..)]
    pub file: Vec<String>,

    /// Pre-fill the prompt input.
    #[arg(long, value_name = "TEXT")]
    pub prefill: Option<String>,

    /// Skip permission prompts.
    #[arg(long)]
    pub dangerously_skip_permissions: bool,

    /// Thinking mode.
    #[arg(long, value_name = "MODE")]
    pub thinking: Option<ThinkingModeArg>,

    /// Max thinking tokens.
    #[arg(long, value_name = "N")]
    pub max_thinking_tokens: Option<u64>,

    /// Minimal mode: skip hooks, LSP, plugin sync, etc.
    #[arg(long)]
    pub bare: bool,

    /// Run Setup hooks with init trigger.
    #[arg(long)]
    pub init: bool,

    /// Run Setup and SessionStart hooks, then exit.
    #[arg(long)]
    pub init_only: bool,

    /// Run Setup hooks with maintenance trigger.
    #[arg(long)]
    pub maintenance: bool,

    #[command(subcommand)]
    pub command: Option<Commands>,
}

#[derive(Subcommand, Debug)]
pub enum Commands {
    /// Authentication management.
    Auth(AuthArgs),
    /// Diagnostic tool.
    Doctor,
    /// MCP server management.
    Mcp(McpArgs),
    /// Plugin management.
    Plugin(PluginArgs),
    /// Open a remote session (internal).
    Open(OpenArgs),
}

#[derive(Args, Debug)]
pub struct AuthArgs {
    #[command(subcommand)]
    pub command: AuthCommands,
}

#[derive(Subcommand, Debug)]
pub enum AuthCommands {
    /// Sign in to Claude.
    Login {
        /// Use Console API key flow.
        #[arg(long)]
        console: bool,
        /// Use claude.ai OAuth flow.
        #[arg(long)]
        claudeai: bool,
    },
    /// Sign out of Claude.
    Logout,
    /// Show authentication status.
    Status,
}

#[derive(Args, Debug)]
pub struct McpArgs {
    #[command(subcommand)]
    pub command: Option<McpCommands>,
}

#[derive(Subcommand, Debug)]
pub enum McpCommands {
    /// Start MCP server (for IDE integration).
    Serve,
}

#[derive(Args, Debug)]
pub struct PluginArgs {
    #[command(subcommand)]
    pub command: Option<PluginCommands>,
}

#[derive(Subcommand, Debug)]
pub enum PluginCommands {
    /// List installed plugins.
    List,
    /// Install a plugin.
    Install {
        /// Plugin name or path.
        name: String,
    },
}

#[derive(Args, Debug)]
pub struct OpenArgs {
    /// Session URL or ID to open.
    pub session: String,
}

/// Output format for non-interactive mode.
#[derive(ValueEnum, Debug, Clone, Copy, PartialEq, Eq, Default)]
#[value(rename_all = "kebab-case")]
pub enum OutputFormat {
    #[default]
    Text,
    Json,
    StreamJson,
}

/// Input format for non-interactive mode.
#[derive(ValueEnum, Debug, Clone, Copy, PartialEq, Eq)]
#[value(rename_all = "kebab-case")]
pub enum InputFormat {
    Text,
    StreamJson,
}

/// Permission mode argument.
#[derive(ValueEnum, Debug, Clone, Copy, PartialEq, Eq)]
#[value(rename_all = "kebab-case")]
pub enum PermissionModeArg {
    /// Normal permission prompts.
    Default,
    /// Auto-accept file edits.
    AcceptEdits,
    /// Auto-accept all (except dangerous).
    AutoAccept,
    /// Plan mode (no tool execution).
    Plan,
    /// Full auto (everything allowed).
    DangerFullAuto,
}

/// Effort level argument.
#[derive(ValueEnum, Debug, Clone, Copy, PartialEq, Eq)]
#[value(rename_all = "kebab-case")]
pub enum EffortArg {
    Low,
    Medium,
    High,
    Max,
}

/// Thinking mode argument.
#[derive(ValueEnum, Debug, Clone, Copy, PartialEq, Eq)]
#[value(rename_all = "kebab-case")]
pub enum ThinkingModeArg {
    Enabled,
    Disabled,
    Adaptive,
}

/// Determine if we are in non-interactive mode.
pub fn is_non_interactive(cli: &Cli) -> bool {
    cli.print
        || cli.init_only
        || cli.command.is_some()
}

/// Get the effective permission mode from CLI args.
pub fn get_permission_mode(cli: &Cli) -> Option<PermissionModeArg> {
    cli.permission_mode
}

/// Get the effective model from CLI args.
pub fn get_model(cli: &Cli) -> Option<&str> {
    cli.model.as_deref()
}

/// Get the effective effort level from CLI args.
pub fn get_effort(cli: &Cli) -> Option<EffortArg> {
    cli.effort
}

/// Get the effective thinking mode from CLI args.
pub fn get_thinking_mode(cli: &Cli) -> Option<ThinkingModeArg> {
    cli.thinking
}

/// Check if debug mode is enabled.
pub fn is_debug(cli: &Cli) -> bool {
    cli.debug.is_some() || cli.debug_to_stderr
}

/// Check if bare mode is enabled.
pub fn is_bare(cli: &Cli) -> bool {
    cli.bare
}
