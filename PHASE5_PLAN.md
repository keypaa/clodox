# Phase 5: Command System — Complete Plan

## Architecture: TypeScript → Rust

| TypeScript Concept | Rust Equivalent |
|---|---|
| `Command` union type (`prompt`/`local`/`local-jsx`) | `Command` trait with `CommandType` enum |
| `load: () => import('./impl.js')` lazy loading | `Arc<dyn CommandImpl>` + lazy init via `OnceLock` |
| `LocalCommandCall(args, context) → LocalCommandResult` | `async fn execute(&self, args: &str, ctx: &CommandContext) -> CommandResult` |
| `PromptCommand.getPromptForCommand(args)` | `fn build_prompt(&self, args: &str) -> Vec<ContentBlockParam>` |
| `LocalJSXCommand` → React.ReactNode | `CommandOutput::TuiWidget` (ratatui widget) |
| `availability: ['claude-ai', 'console']` | `availability: Vec<AuthType>` enum |
| `isEnabled: () => boolean` | `is_enabled: Box<dyn Fn() -> bool + Send>` |
| `setMessages(updater)` | `state.write().messages = new_messages` |
| `getAppState() / setAppState(f)` | `Arc<RwLock<AppState>>` read/write |

## Command Types

### `CommandType::Local`
- Runs entirely client-side
- Returns `CommandResult::Text`, `CommandResult::Compact`, or `CommandResult::Skip`
- Examples: `/clear`, `/theme`, `/vim`, `/config`

### `CommandType::Prompt`
- Builds a prompt that gets sent to the model
- Returns `CommandResult::Prompt` with content blocks
- Examples: `/commit`, `/review`, `/doctor`

### `CommandType::Tui`
- Renders a TUI widget (replaces JSX commands)
- Returns `CommandResult::Widget` with a ratatui widget
- Examples: `/help`, `/cost`, `/context`, `/mcp`

## Command Trait

```rust
#[async_trait]
pub trait Command: Send + Sync {
    fn name(&self) -> &str;
    fn description(&self) -> &str;
    fn aliases(&self) -> &[&str];
    fn argument_hint(&self) -> Option<&str>;
    fn availability(&self) -> &[AuthType];
    fn is_enabled(&self) -> bool;
    fn is_hidden(&self) -> bool;
    fn command_type(&self) -> CommandType;

    async fn execute(&self, args: &str, ctx: &CommandContext) -> CommandResult;
}
```

## Command Context

```rust
pub struct CommandContext {
    pub state: SharedState,
    pub query_engine: Arc<QueryEngine>,
    pub terminal: Arc<TerminalManager>,
    pub theme: Theme,
    pub abort_signal: CancellationToken,
}
```

## Command Result

```rust
pub enum CommandResult {
    Text { message: String, display: CommandResultDisplay },
    Compact { compaction_result: CompactionResult, display_text: Option<String> },
    Skip,
    Prompt { content: Vec<ContentBlockParam>, allowed_tools: Option<Vec<String>> },
    Widget { widget: Box<dyn Widget + Send> },
    Error { message: String },
}

pub enum CommandResultDisplay {
    System,  // Dim, system-style display
    User,    // Normal user message style
    Skip,    // Don't display anything
}
```

## Sub-Phases (8 parts, implemented sequentially)

### 5.1: Command Registry & Trait System
- `Command` trait with all metadata methods
- `CommandType` enum (Local, Prompt, Tui)
- `CommandResult` enum with display modes
- `CommandContext` struct for execution context
- `CommandRegistry` for registration, lookup, filtering
- `get_commands()` → all enabled commands
- `get_command(name)` → lookup by name or alias
- Availability filtering by auth type
- Lazy loading pattern via `OnceLock`

### 5.2: Core Tier 1 Commands (Essential)
- `/help` — Show help text with command list (TUI widget)
- `/clear` — Clear conversation, reset session
- `/compact` — Trigger context compaction
- `/config` — View/edit settings
- `/login` — API key authentication
- `/logout` — Clear auth state
- `/resume` — Session picker
- `/diff` — Show git diff
- `/cost` — Token usage and cost display

### 5.3: Tier 2 Commands (Important)
- `/commit` — Git commit with AI-generated message
- `/review` — Code review of changes
- `/memory` — Memory management (view/clear)
- `/skills` — Skill management
- `/tasks` — Background task management
- `/mcp` — MCP server management
- `/theme` — Theme switching (dark/light/auto)
- `/vim` — Vim mode toggle
- `/context` — Context visualization

### 5.4: Tier 3 Commands (Nice to Have)
- `/doctor` — Diagnostics and health check
- `/share` — Share session link
- `/pr_comments` — PR comment management
- `/desktop` / `/mobile` — App handoff
- `/model` — Model switching
- `/permissions` — Permission mode management
- `/output-style` — Output style toggle
- `/feedback` — Send feedback
- `/hooks` — Hook management
- `/effort` — Effort level toggle
- `/fast` — Fast mode toggle
- `/brief` — Brief mode toggle
- `/agents` — Agent management
- `/branch` — Git branch management
- `/copy` — Copy to clipboard
- `/exit` / `/quit` — Exit application
- `/version` — Show version info

### 5.5: Utility Commands & Aliases
- `/btw` — Side question without interrupting
- `/stats` — Usage statistics
- `/status` — Current status display
- `/files` — List tracked files
- `/export` — Export conversation
- `/rename` — Rename session
- `/color` — Set agent color
- `/release-notes` — Show recent changes
- `/keybindings` — Show keybindings
- `/passes` — Pass management
- `/plan` — Enter plan mode
- `/sandbox-toggle` — Toggle sandbox
- `/terminal-setup` — Terminal configuration
- `/upgrade` — Upgrade info
- `/usage` — Usage details
- `/voice` — Voice mode toggle
- `/chrome` — Chrome integration
- `/ide` — IDE integration
- `/init` — Initialize project
- `/remote-setup` — Remote setup
- `/remote-env` — Remote environment
- `/privacy-settings` — Privacy config
- `/rate-limit-options` — Rate limit config
- `/reload-plugins` — Plugin reload
- `/stickers` — Sticker mode
- `/tag` — Tag session
- `/thinkback` — Thinkback mode
- `/thinkback-play` — Thinkback playback

### 5.6: Command Execution Pipeline
- Slash command parsing from input
- Argument extraction and validation
- Command dispatch through registry
- Result handling and display
- Error handling with user-friendly messages
- Abort/cancellation support
- Progress reporting during execution
- Integration with TUI main loop

### 5.7: Model-Visible Commands (Skill Tools)
- Commands that the model can invoke
- `get_skill_tool_commands()` → model-visible subset
- Tool-like interface for commands
- Command-as-tool for agent workflows
- Permission checks for model-invoked commands

### 5.8: Command Completion & Typeahead
- Slash command autocomplete integration
- Fuzzy matching for command names
- Argument hints display
- Description display in autocomplete
- Command categorization in UI
- Recent/frequent command prioritization

## Implementation Order

1. **Registry + Trait** (5.1) — Foundation for all commands
2. **Tier 1 Commands** (5.2) — Essential commands first
3. **Tier 2 Commands** (5.3) — Important features
4. **Tier 3 Commands** (5.4) — Nice-to-have features
5. **Utility Commands** (5.5) — Remaining commands
6. **Execution Pipeline** (5.6) — Wire commands into main loop
7. **Model-Visible Commands** (5.7) — Skill tool integration
8. **Completion & Typeahead** (5.8) — Autocomplete polish

## File Structure

```
crates/commands/
├── Cargo.toml
└── src/
    ├── lib.rs              # Module exports
    ├── registry.rs         # CommandRegistry, lookup, filtering
    ├── traits.rs           # Command trait, CommandType, CommandResult
    ├── context.rs          # CommandContext
    ├── tier1/              # Essential commands
    │   ├── mod.rs
    │   ├── help.rs
    │   ├── clear.rs
    │   ├── compact.rs
    │   ├── config.rs
    │   ├── login.rs
    │   ├── logout.rs
    │   ├── resume.rs
    │   ├── diff.rs
    │   └── cost.rs
    ├── tier2/              # Important commands
    │   ├── mod.rs
    │   ├── commit.rs
    │   ├── review.rs
    │   ├── memory.rs
    │   ├── skills.rs
    │   ├── tasks.rs
    │   ├── mcp.rs
    │   ├── theme.rs
    │   ├── vim.rs
    │   └── context.rs
    ├── tier3/              # Nice-to-have commands
    │   ├── mod.rs
    │   ├── doctor.rs
    │   ├── share.rs
    │   ├── pr_comments.rs
    │   ├── model.rs
    │   ├── permissions.rs
    │   └── ... (more)
    ├── utility/            # Utility commands
    │   ├── mod.rs
    │   ├── btw.rs
    │   ├── stats.rs
    │   └── ... (more)
    └── completion.rs       # Autocomplete integration
```

## Key Design Decisions

### 1. Trait vs Enum for Commands
**Decision**: Use `Command` trait rather than enum.
**Rationale**: Commands have diverse implementations (local, prompt, TUI). A trait allows each command to implement its own logic without match exhaustion. New commands can be added without modifying existing code.
**Revisit**: If command count stays small (<20), an enum could be simpler.

### 2. Lazy Loading Pattern
**Decision**: Use `std::sync::OnceLock<Arc<dyn CommandImpl>>` for lazy initialization.
**Rationale**: Matches TypeScript's `load: () => import('./impl.js')` pattern. Heavy dependencies (git, MCP, etc.) are only loaded when the command is first invoked.
**Revisit**: Consider `tokio::sync::OnceLock` if async init is needed.

### 3. Command Context Borrowing
**Decision**: `CommandContext` holds `Arc<RwLock<AppState>>` rather than direct references.
**Rationale**: Commands execute asynchronously and need to access/modify state. `Arc<RwLock>` allows concurrent access without lifetime issues.
**Revisit**: If contention becomes an issue, consider more granular locks.

### 4. TUI Widget Commands
**Decision**: Replace JSX commands with `CommandResult::Widget { widget: Box<dyn Widget + Send> }`.
**Rationale**: Rust has no JSX. ratatui's `Widget` trait is the natural equivalent. Commands return widgets that the main loop renders.
**Revisit**: Consider a macro-based DSL if widget composition becomes unwieldy.

### 5. Command Availability
**Decision**: `availability: Vec<AuthType>` checked at registration time, not per-invocation.
**Rationale**: Availability is static (auth type doesn't change during a session). Filtering at registration avoids repeated checks.
**Revisit**: If availability becomes dynamic (feature flags), move check to `is_enabled()`.

## Questions / Open Items

1. **How should `/compact` work without the full compaction service?** — We have the compaction system in cc-query, but it needs the API client to be connected. Should `/compact` be a stub that calls the existing `compactConversation` function, or should it be fully wired?

2. **Should `/login` use the existing LoginScreen from Phase 4.13?** — We already built a login screen. Should the `/login` command trigger it, or should it be a separate CLI-style flow?

3. **How to handle commands that need git operations?** — Commands like `/commit`, `/diff`, `/review` need git integration. Should we add a `git` module to cc-core, or keep it in cc-commands?

4. **Should MCP commands be stubs or functional?** — MCP service layer (Phase 6) isn't built yet. Should `/mcp` commands be stubs or should we implement basic MCP config management now?

5. **How to handle commands that modify AppState deeply (like `/clear`)?** — `/clear` in TypeScript clears messages, regenerates session ID, clears caches, runs hooks. Should we implement the full behavior or a simplified version?
