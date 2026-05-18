# Comprehensive Plan: Claude Code TypeScript → Rust Port

## Phase 1: API Client + Query Engine (`crates/query/`)

### 1.1 Anthropic API Client (`src/api_client.rs`)

**Goal**: Implement streaming API client matching `@anthropic-ai/sdk` behavior.

**Key components**:
- `ApiClient` struct with configuration (API key, base URL, model, timeout)
- `stream_message()` → `impl Stream<Item = Result<StreamEvent>>`
- Request builder matching Anthropic Messages API:
  - `model`, `max_tokens`, `system`, `messages`, `tools`, `tool_choice`
  - `thinking` (budget_tokens), `metadata`, `temperature`, `top_p`
  - Prompt caching headers (`anthropic-beta: prompt-caching-2024-07-31`)
- Response parsing:
  - SSE event stream parsing (`message_start`, `content_block_start`, `content_block_delta`, `content_block_stop`, `message_delta`, `message_stop`, `ping`)
  - Delta types: `TextDelta`, `ThinkingDelta`, `InputJsonDelta`
  - Usage tracking: `input_tokens`, `output_tokens`, `cache_read_input_tokens`, `cache_creation_input_tokens`
- Error handling:
  - `APIError` enum: `RateLimit`, `Overloaded`, `InvalidRequest`, `Authentication`, `Permission`, `NotFound`, `InternalServerError`, `ServiceUnavailable`
  - Retry logic with exponential backoff (matching `withRetry.ts`)
  - Request ID tracking for debugging
- Token estimation (local, pre-API):
  - Rough token counter for context window management
  - Cache prefix detection for prompt caching optimization

**Files to create**:
- `crates/query/src/api_client.rs` — main client
- `crates/query/src/api_types.rs` — request/response types
- `crates/query/src/errors.rs` — error types
- `crates/query/src/retry.rs` — retry logic

**Key challenges**:
- SSE streaming with `reqwest` requires careful byte-level parsing
- Tool use input JSON arrives as partial JSON deltas — need incremental JSON parser
- Prompt caching: must track byte-level cache prefixes across requests

### 1.2 Query Engine State Machine (`src/engine.rs`)

**Goal**: Port the core query loop from `QueryEngine.ts` (~46K lines).

**State machine design**:
```
State {
    messages: Vec<Message>,
    tool_results_pending: Vec<PendingToolResult>,
    token_budget: TokenBudget,
    abort_signal: AbortSignal,
    compaction_state: CompactionState,
    recovery_state: Option<RecoveryState>,
}
```

**Core loop** (`query()` async generator):
1. **Pre-loop setup**:
   - Assemble system prompt (custom + default + memory mechanics + append)
   - Load tools → JSON schema for API
   - Initialize token tracking

2. **Each iteration**:
   a. **Snip compact**: Remove old messages if history too long
   b. **Microcompact**: Collapse cached edit sequences
   c. **Context collapse**: If context window approaching limit
   d. **Auto-compact**: Trigger based on token budget
   e. **API call**: `call_model()` → stream events
   f. **Tool execution**: `StreamingToolExecutor` for parallel tool calls
   g. **Post-sampling hooks**: Run registered hooks
   h. **Recovery paths**:
      - `prompt-too-long` → collapse drain → reactive compact → retry
      - `max-output-tokens` → escalate → retry
      - `model-fallback` → switch model → retry

3. **Termination**:
   - `stop_reason: "end_turn"` → done
   - `stop_reason: "stop_sequence"` → done
   - `stop_reason: "tool_use"` → execute tools, continue
   - `stop_reason: "max_tokens"` → recovery or done
   - Abort signal → cancel

**Tool execution pipeline**:
```
ToolCall → validate_input() → check_permissions() → can_use_tool() → call() → ToolResult
```

**Streaming tool executor**:
- Run multiple tool calls in parallel (up to concurrency limit)
- Progress updates via callback
- Timeout handling per tool
- Result aggregation back into message stream

**Files to create**:
- `crates/query/src/engine.rs` — QueryEngine struct + query loop
- `crates/query/src/state.rs` — query state types
- `crates/query/src/streaming.rs` — streaming tool executor
- `crates/query/src/compaction.rs` — compaction logic
- `crates/query/src/system_prompt.rs` — system prompt assembly
- `crates/query/src/token_budget.rs` — token tracking

**Key challenges**:
- The query loop is a complex async state machine with 7+ continue sites
- Recovery paths require careful state preservation
- Tool execution during streaming needs async coordination
- Context compaction must preserve conversation semantics

### 1.3 Compaction System (`src/compaction.rs`)

**Goal**: Port 4 compaction strategies.

**Strategies**:
1. **Snip compact**: Remove oldest messages, keep recent context
2. **Microcompact**: Collapse consecutive file edit sequences
3. **Auto-compact**: LLM-based summarization when context window full
4. **Reactive compact**: Emergency compaction on `prompt-too-long`

**Compaction hooks**:
- `pre_compact`: Run before compaction (save state, notify UI)
- `post_compact`: Run after compaction (update UI, log metrics)
- `session_start`: Run at session start

**Files to create**:
- `crates/query/src/compaction.rs` — all compaction strategies
- `crates/query/src/hooks.rs` — hook system

### 1.4 System Prompt Assembly (`src/system_prompt.rs`)

**Goal**: Build system prompt from components.

**Components**:
- Base system prompt (tool instructions, behavior guidelines)
- Custom system prompt (user-provided override)
- Append system prompt (additional instructions)
- Memory mechanics instruction
- CLAUDE.md content injection
- Tool descriptions (dynamic, based on enabled tools)
- Skill/command descriptions
- Output style configuration

---

## Phase 2: Core Tool Implementations (`crates/tools/`)

### 2.1 BashTool (`src/bash.rs`)

**Input**: `{ command: string, timeout?: number, description?: string }`
**Output**: `{ stdout: string, stderr: string, exit_code: number }`

**Implementation**:
- `tokio::process::Command` for async execution
- PTY support for interactive commands
- Sandbox mode (seccomp, namespaces)
- Permission checking:
  - Command parsing → classifier input
  - Safety rules matching
  - Working directory validation
- Progress updates:
  - Stdout/stderr streaming
  - Timeout warnings
- Result bounding:
  - Max output size
  - Truncation with file fallback
- Interrupt handling:
  - SIGINT on user interrupt
  - Process group management

**Files**: `crates/tools/src/bash.rs`, `crates/tools/src/bash/` (classifier, sandbox, progress)

### 2.2 FileReadTool (`src/file_read.rs`)

**Input**: `{ path: string, offset?: number, limit?: number, image?: boolean }`
**Output**: `{ content: string, mime_type?: string, truncated?: boolean }`

**Implementation**:
- `tokio::fs::read` for async file reading
- Image support: base64 encoding
- PDF support: text extraction
- Notebook support: cell rendering
- Permission: read-only, path validation
- LRU cache for recently read files
- Token limit enforcement

**Files**: `crates/tools/src/file_read.rs`

### 2.3 FileEditTool (`src/file_edit.rs`)

**Input**: `{ path: string, old_string: string, new_string: string }`
**Output**: `{ success: boolean, diff: string }`

**Implementation**:
- String replacement with validation
- Multiple edits support (batch)
- Diff generation (unified diff format)
- Permission: destructive, path validation
- File state tracking (read → edit consistency)
- Conflict detection (concurrent edits)

**Files**: `crates/tools/src/file_edit.rs`

### 2.4 FileWriteTool (`src/file_write.rs`)

**Input**: `{ path: string, content: string }`
**Output**: `{ success: boolean, bytes_written: number }`

**Implementation**:
- `tokio::fs::write` for async writing
- Directory creation (recursive)
- Permission: destructive, path validation
- Backup on overwrite (optional)

**Files**: `crates/tools/src/file_write.rs`

### 2.5 GrepTool (`src/grep.rs`)

**Input**: `{ pattern: string, paths?: string[], glob?: string[], output_mode?: string }`
**Output**: `{ matches: [...], truncated?: boolean }`

**Implementation**:
- Spawn `rg` (ripgrep) subprocess
- Parse JSON output format
- Result bounding (max matches)
- Permission: read-only
- Working directory validation

**Files**: `crates/tools/src/grep.rs`

### 2.6 GlobTool (`src/glob.rs`)

**Input**: `{ pattern: string, path?: string }`
**Output**: `{ matches: [...], truncated?: boolean }`

**Implementation**:
- `glob` crate for pattern matching
- BFS traversal for performance
- Result bounding
- Permission: read-only

**Files**: `crates/tools/src/glob.rs`

### 2.7 WebSearchTool (`src/web_search.rs`)

**Input**: `{ query: string }`
**Output**: `{ results: [...] }`

**Implementation**:
- External search API integration
- Result formatting
- Rate limiting

**Files**: `crates/tools/src/web_search.rs`

### 2.8 WebFetchTool (`src/web_fetch.rs`)

**Input**: `{ url: string }`
**Output**: `{ content: string, title?: string }`

**Implementation**:
- `reqwest` for HTTP fetching
- HTML → markdown conversion
- Content size limiting
- URL validation/safety

**Files**: `crates/tools/src/web_fetch.rs`

### 2.9 AgentTool (`src/agent.rs`)

**Input**: `{ prompt: string, model?: string, subagent_type?: string, ... }`
**Output**: `{ result: string }`

**Implementation**:
- Subagent spawning (new QueryEngine instance)
- Context isolation
- Token budget allocation
- Progress streaming
- Fork/async/coordinator modes
- Team/swarm support

**Files**: `crates/tools/src/agent.rs`, `crates/tools/src/agent/` (modes, context)

### 2.10 Tool Registry (`src/registry.rs`)

**Goal**: Tool assembly and filtering.

**Functions**:
- `get_all_base_tools()` → Vec<Arc<dyn Tool>>
- `get_tools(permission_context)` → filtered set
- `assemble_tool_pool()` → built-in + MCP tools
- Tool deduplication by name
- Cache-stable sorting

**Files**: `crates/tools/src/registry.rs`

---

## Phase 3: CLI Entrypoint + Basic REPL (`crates/cli/`)

### 3.1 CLI Parsing (`src/cli.rs`)

**Goal**: Port Commander.js CLI to clap.

**Commands**:
- Default: interactive REPL
- `-p, --print`: non-interactive mode
- `--model <model>`: model selection
- `--permission-mode <mode>`: permission mode
- `--verbose`: debug output
- `--settings <path|json>`: settings loading
- `--version`: version display
- `--help`: help text

**Files**: `crates/cli/src/cli.rs`

### 3.2 Bootstrap (`src/bootstrap.rs`)

**Goal**: Fast startup with parallel prefetch.

**Steps**:
1. Early side-effects (parallel):
   - MDM settings read
   - Keychain prefetch
   - API preconnect
2. Settings loading (file or CLI arg)
3. Auth initialization
4. Migration runner
5. Branch: interactive vs non-interactive

**Files**: `crates/cli/src/bootstrap.rs`

### 3.3 Settings System (`src/settings.rs`)

**Goal**: Settings loading, validation, migration.

**Storage**:
- User settings: `~/.claude/settings.json`
- Project settings: `.claude/settings.json`
- Local settings: `.claude/local.json`

**Schema**: Zod → schemars + serde

**Migration**: Version-based migration runner

**Files**: `crates/cli/src/settings.rs`

### 3.4 Basic REPL (`src/repl.rs`)

**Goal**: Text-based REPL (no TUI yet).

**Features**:
- Read line from stdin
- Parse slash commands
- Submit to QueryEngine
- Stream response to stdout
- Handle tool permission prompts
- History navigation (arrow keys)
- Interrupt handling (Ctrl+C)

**Files**: `crates/cli/src/repl.rs`

### 3.5 Main Entry (`src/main.rs`)

**Goal**: Wire everything together.

**Flow**:
```
main() → parse CLI → bootstrap() → init() → branch:
  ├─ interactive → REPL → QueryEngine → TUI (later)
  └─ non-interactive → QueryEngine → print response → exit
```

---

## Phase 4: TUI Engine (`crates/tui/`) — NEW CRATE

### 4.1 Crate Setup

**Dependencies**: `ratatui`, `crossterm`, `unicode-width`

**Files**:
- `crates/tui/Cargo.toml`
- `crates/tui/src/lib.rs`

### 4.2 Terminal Backend (`src/terminal.rs`)

**Goal**: crossterm terminal management.

**Components**:
- Terminal initialization (raw mode, alternate screen)
- Event polling (keyboard, mouse, resize)
- Synchronized output (DEC BSU/ESU)
- FPS tracking
- Cleanup on exit

**Files**: `crates/tui/src/terminal.rs`

### 4.3 Layout Engine (`src/layout.rs`)

**Goal**: Port custom Ink layout (Yoga-based).

**Components**:
- Flexbox layout (ratatui's built-in)
- Constraint-based sizing
- Scroll handling
- Focus management

**Files**: `crates/tui/src/layout.rs`

### 4.4 Theme System (`src/theme.rs`)

**Goal**: Theme management.

**Components**:
- Built-in themes (light, dark, system)
- Color palette
- Style definitions
- Theme switching

**Files**: `crates/tui/src/theme.rs`

### 4.5 Core Components (`src/components/`)

**Port all 144 components** (prioritized):

**Tier 1 (essential)**:
- `App` — top-level wrapper
- `Messages` / `Message` / `MessageRow` — message rendering
- `PromptInput` — text input
- `Spinner` — loading animation
- `StatusLine` — bottom status bar
- `FullscreenLayout` — expanded views

**Tier 2 (important)**:
- `VirtualMessageList` — virtualized scrolling
- `PermissionDialog` — permission prompts
- `ModelPicker` — model selection
- `SettingsPanel` — settings UI
- `DiffViewer` — diff display
- `MarkdownRenderer` — markdown rendering

**Tier 3 (nice to have)**:
- `TaskList` — task management
- `TeammateView` — team agent view
- `BridgeDialog` — bridge status
- `OnboardingFlow` — first-run setup
- `ThemePicker` — theme selection

**Files**: `crates/tui/src/components/mod.rs` + individual files

### 4.6 Input Handling (`src/input.rs`)

**Goal**: Keyboard input processing.

**Components**:
- Key event parsing
- Vim mode support
- History navigation
- Tab completion
- Clipboard integration

**Files**: `crates/tui/src/input.rs`

---

## Phase 5: Command System (`crates/commands/`)

### 5.1 Command Registry (`src/registry.rs`)

**Goal**: Command loading and filtering.

**Functions**:
- `get_commands()` → all commands (built-in + skills + plugins)
- `get_skill_tool_commands()` → model-visible commands
- Availability filtering (auth/provider)
- Typeahead integration

**Files**: `crates/commands/src/registry.rs`

### 5.2 Core Commands

**Port all ~101 commands** (prioritized):

**Tier 1 (essential)**:
- `/help` — help text
- `/compact` — context compression
- `/config` — settings management
- `/login` / `/logout` — authentication
- `/resume` — restore session
- `/diff` — view changes
- `/cost` — usage cost

**Tier 2 (important)**:
- `/commit` — git commit
- `/review` — code review
- `/memory` — memory management
- `/skills` — skill management
- `/tasks` — task management
- `/mcp` — MCP management
- `/theme` — theme switching
- `/vim` — vim mode toggle
- `/context` — context visualization

**Tier 3 (nice to have)**:
- `/doctor` — diagnostics
- `/share` — share session
- `/pr_comments` — PR comments
- `/desktop` / `/mobile` — app handoff
- All remaining commands

---

## Phase 6: Service Layer (`crates/services/`)

### 6.1 API Service (`src/api/`)

**Components**:
- API client wrapper (higher-level than query's raw client)
- Bootstrap data fetching
- Session ingress (WebSocket)
- Usage tracking

**Files**: `crates/services/src/api/mod.rs`

### 6.2 MCP Service (`src/mcp/`)

**Components**:
- MCP client management
- Server connections (stdio, HTTP, SSE)
- Configuration parsing (JSON, env expansion)
- OAuth flow
- Elicitation handler
- Resource management
- Official server registry

**Files**: `crates/services/src/mcp/mod.rs` + submodules

### 6.3 Analytics Service (`src/analytics/`)

**Components**:
- Feature flags (GrowthBook-compatible)
- Event logging
- Telemetry (OpenTelemetry)
- Statsig gates

**Files**: `crates/services/src/analytics/mod.rs`

### 6.4 Plugin Service (`src/plugins/`)

**Components**:
- Plugin loading
- Installation from marketplace
- Manifest parsing
- Error handling

**Files**: `crates/services/src/plugins/mod.rs`

### 6.5 Other Services

- **LSP** (`src/lsp/`) — Language Server Protocol manager
- **Token estimation** (`src/token_estimation.rs`) — token counting
- **Team memory sync** (`src/team_memory_sync.rs`) — shared memory
- **Extract memories** (`src/extract_memories.rs`) — auto memory extraction
- **Prompt suggestion** (`src/prompt_suggestion.rs`) — suggestion generation

---

## Phase 7: Bridge System (`crates/bridge/`)

### 7.1 Bridge Daemon (`src/daemon.rs`)

**Components**:
- Poll loop for work items
- Child session spawning
- Heartbeat management
- Token refresh
- Capacity management
- Worktree creation
- Multi-session mode (up to 32 concurrent)
- CCR v2 support (SSE transport)

**Files**: `crates/bridge/src/daemon.rs`

### 7.2 REPL Bridge (`src/repl_bridge.rs`)

**Components**:
- Environment registration
- Session creation
- Poll loop → ingress WS
- Message forwarding (bidirectional)
- Control request handling
- Reconnection (3 strategies)
- Crash-recovery pointer
- v1 (WebSocket) and v2 (SSE + CCR) transport

**Files**: `crates/bridge/src/repl_bridge.rs`

### 7.3 Bridge Messaging (`src/messaging.rs`)

**Components**:
- Message protocol definition
- Permission callbacks
- JWT authentication
- Session runner

**Files**: `crates/bridge/src/messaging.rs`, `src/permission_callbacks.rs`, `src/jwt_utils.rs`, `src/session_runner.rs`

---

## Phase 8: Auxiliary Systems

### 8.1 Vim Mode (`crates/core/src/vim/`)
- Vim keybindings
- Modal editing (normal, insert, visual)
- Command mode

### 8.2 Skills System (`crates/core/src/skills/`)
- Skill loading from directories
- Skill execution
- Dynamic discovery

### 8.3 Coordinator (`crates/core/src/coordinator/`)
- Multi-agent orchestration
- Task panel UI
- Agent lifecycle management

### 8.4 Voice (`crates/core/src/voice/`)
- Voice input (feature-flagged)
- Audio capture
- Speech-to-text

### 8.5 Migrations (`crates/cli/src/migrations/`)
- Config version migration
- Data migration

---

## Things to Note / Known Challenges

### TypeScript → Rust Translation Challenges

1. **DeepImmutable<T>**: TypeScript's deep readonly types → Rust's ownership model naturally enforces immutability. Use `Clone` for shared state.

2. **React/JSX rendering**: Tool `renderToolUseMessage()` etc. return `React.ReactNode` → In Rust, these become string/TextSpan generators for ratatui. No direct JSX equivalent.

3. **`bun:bundle` feature flags**: Compile-time DCE → Rust's `cfg!` macros and Cargo features.

4. **Dynamic `require()`**: Lazy loading → Rust's `Arc<dyn Trait>` with lazy initialization patterns.

5. **Circular dependencies**: TypeScript's `require()` in functions → Rust's careful module ordering and trait objects.

6. **Async generators**: TypeScript `async function*` → Rust's `futures::stream::Stream` or `async-stream` crate.

7. **Zod runtime validation**: → serde's derive macros + manual validation where needed.

8. **Node.js builtins** (`fs`, `path`, `os`, `crypto`, `vm`): → Rust std lib equivalents. Note: `vm` (REPL tool) has no direct Rust equivalent — may need embedded JS engine (deno_core, rquickjs) or skip.

9. **Lodash utilities** (`memoize`, `last`, `isObject`): → Rust standard lib + itertools. Memoization: `once_cell` or `lazy_static`.

10. **GrowthBook/Statsig**: Feature flags → Custom implementation or SDK if available.

### Architecture Decisions to Revisit

1. **Error handling**: `anyhow` for app-level, `thiserror` for library-level. Consider unified error type.

2. **State management**: Current `Store<T>` uses `RwLock` — may need `tokio::sync::RwLock` for async. Consider `dux` or `rearch` crate.

3. **Tool trait**: Currently uses `serde_json::Value` for input/output — consider generic types with trait bounds for better type safety.

4. **Streaming**: `reqwest` SSE → consider `eventsource-client` crate for robustness.

5. **Testing**: Plan for extensive unit tests for tools, integration tests for query loop, e2e tests for full app.

---

## File Tree (Final Target)

```
claude-code-rs/
├── Cargo.toml
├── AGENTS.md                          # Issues & notes tracker
├── PLAN.md                            # This file
├── crates/
│   ├── core/
│   │   ├── Cargo.toml
│   │   └── src/
│   │       ├── lib.rs
│   │       ├── types/mod.rs           # IDs, themes, settings
│   │       ├── messages/mod.rs        # All message types
│   │       ├── permissions/mod.rs     # Permission system
│   │       ├── tools/mod.rs           # Tool trait + context
│   │       ├── commands/mod.rs        # Command types
│   │       ├── state/mod.rs           # Store + AppState
│   │       ├── vim/                   # Vim mode
│   │       ├── skills/                # Skills system
│   │       └── coordinator/           # Multi-agent
│   ├── query/
│   │   ├── Cargo.toml
│   │   └── src/
│   │       ├── lib.rs
│   │       ├── api_client.rs          # Anthropic API client
│   │       ├── api_types.rs           # API request/response types
│   │       ├── errors.rs              # API errors
│   │       ├── retry.rs               # Retry logic
│   │       ├── engine.rs              # QueryEngine + state machine
│   │       ├── state.rs               # Query state
│   │       ├── streaming.rs           # Streaming tool executor
│   │       ├── compaction.rs          # Compaction strategies
│   │       ├── system_prompt.rs       # System prompt assembly
│   │       ├── token_budget.rs        # Token tracking
│   │       └── hooks.rs               # Hook system
│   ├── tools/
│   │   ├── Cargo.toml
│   │   └── src/
│   │       ├── lib.rs
│   │       ├── bash.rs                # BashTool
│   │       ├── file_read.rs           # FileReadTool
│   │       ├── file_write.rs          # FileWriteTool
│   │       ├── file_edit.rs           # FileEditTool
│   │       ├── grep.rs                # GrepTool
│   │       ├── glob.rs                # GlobTool
│   │       ├── web_search.rs          # WebSearchTool
│   │       ├── web_fetch.rs           # WebFetchTool
│   │       ├── agent.rs               # AgentTool
│   │       └── registry.rs            # Tool registry
│   ├── commands/
│   │   ├── Cargo.toml
│   │   └── src/
│   │       ├── lib.rs
│   │       ├── registry.rs            # Command registry
│   │       ├── help.rs                # /help
│   │       ├── compact.rs             # /compact
│   │       ├── config.rs              # /config
│   │       ├── login.rs               # /login
│   │       ├── logout.rs              # /logout
│   │       ├── resume.rs              # /resume
│   │       ├── diff.rs                # /diff
│   │       ├── cost.rs                # /cost
│   │       ├── commit.rs              # /commit
│   │       ├── review.rs              # /review
│   │       ├── memory.rs              # /memory
│   │       ├── skills.rs              # /skills
│   │       ├── tasks.rs               # /tasks
│   │       ├── mcp.rs                 # /mcp
│   │       ├── theme.rs               # /theme
│   │       ├── vim.rs                 # /vim
│   │       ├── context.rs             # /context
│   │       ├── doctor.rs              # /doctor
│   │       └── share.rs               # /share
│   ├── services/
│   │   ├── Cargo.toml
│   │   └── src/
│   │       ├── lib.rs
│   │       ├── api/mod.rs             # API service
│   │       ├── mcp/mod.rs             # MCP service
│   │       ├── analytics/mod.rs       # Analytics
│   │       ├── plugins/mod.rs         # Plugins
│   │       ├── lsp/mod.rs             # LSP
│   │       ├── token_estimation.rs    # Token estimation
│   │       ├── team_memory_sync.rs    # Team memory
│   │       ├── extract_memories.rs    # Memory extraction
│   │       └── prompt_suggestion.rs   # Prompt suggestions
│   ├── bridge/
│   │   ├── Cargo.toml
│   │   └── src/
│   │       ├── lib.rs
│   │       ├── daemon.rs              # Bridge daemon
│   │       ├── repl_bridge.rs         # REPL bridge
│   │       ├── messaging.rs           # Message protocol
│   │       ├── permission_callbacks.rs
│   │       ├── session_runner.rs
│   │       ├── jwt_utils.rs
│   │       └── transport.rs           # Transport layer
│   ├── tui/
│   │   ├── Cargo.toml
│   │   └── src/
│   │       ├── lib.rs
│   │       ├── terminal.rs            # Terminal backend
│   │       ├── layout.rs              # Layout engine
│   │       ├── theme.rs               # Theme system
│   │       ├── input.rs               # Input handling
│   │       └── components/            # UI components
│   │           ├── mod.rs
│   │           ├── app.rs
│   │           ├── messages.rs
│   │           ├── message.rs
│   │           ├── prompt_input.rs
│   │           ├── spinner.rs
│   │           ├── status_line.rs
│   │           └── ... (144 total)
│   └── cli/
│       ├── Cargo.toml
│       └── src/
│           ├── main.rs                # Entrypoint
│           ├── cli.rs                 # CLI parsing
│           ├── bootstrap.rs           # Bootstrap logic
│           ├── settings.rs            # Settings system
│           ├── repl.rs                # Basic REPL
│           └── migrations/            # Migrations
```

---

## Execution Priority Summary

| Phase | What | Why First |
|-------|------|-----------|
| **1** | API Client + Query Engine | Heart of the app, testable headlessly |
| **2** | Core Tools (6 tools) | Query loop needs tools to execute |
| **3** | CLI + Basic REPL | End-to-end working app (text-only) |
| **4** | TUI Engine | Rendering layer on top of working core |
| **5** | Commands | User-facing features |
| **6** | Services | External integrations |
| **7** | Bridge | Remote/IDE features |
| **8** | Auxiliary | Vim, skills, coordinator, voice |
