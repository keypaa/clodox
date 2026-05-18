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
| **10** | MCP + Web Tools + Agent | Missing tools + MCP protocol for Exa integration |

---

## Phase 10: MCP Protocol + Web Tools + Agent (Full 1:1)

### Phase 10.0: MCP JSON-RPC Protocol via `rmcp` (Official Rust SDK)

**Goal**: Replace the stub `McpService` with a full MCP client using the official `rmcp` crate (v1.7.0, 3.4k stars, supports stdio + streamable HTTP + SSE transport).

**Dependencies** (add to `crates/services/Cargo.toml`):
```toml
rmcp = { version = "1.7", features = ["client", "transport-child-process", "transport-streamable-http-client-reqwest"] }
```

**Transport layer** (provided by `rmcp`):
- `TokioChildProcess` — stdio transport for local MCP servers (spawned child processes)
- `StreamableHttpClientTransport` — HTTP/SSE transport for remote MCP servers (Exa, etc.)

**Connection lifecycle**:
- `connect_stdio(name, command, args, env)` — spawn child process, connect via stdio, call `initialize`
- `connect_http(name, base_url)` — connect to remote MCP endpoint, call `initialize`
- Each connection creates an `rmcp::Peer<RoleServer>` for sending/receiving JSON-RPC messages

**MCP methods** (via `rmcp::Peer`):
- `peer.list_tools(None)` → `ListToolsResult { tools: Vec<Tool> }`
- `peer.call_tool(CallToolRequestParam { name, arguments })` → `CallToolResult`
- `peer.list_resources(None)` → `ListResourcesResult`
- `peer.read_resource(ReadResourceRequestParam { uri })` → `ReadResourceResult`

**Server state** — keep existing `McpServerState` but add `peer: Option<Arc<Peer<RoleServer>>>` field

**Exa MCP integration**:
- `connect_exa()` method — connects to `https://mcp.exa.ai/mcp` via HTTP/SSE
- Auto-discovers `web_search_advanced_exa` tool
- Stores discovered tools in `McpToolInfo` format for use by `web_search` tool

**Tool invocation from other crates**:
- `call_mcp_tool(server_name, tool_name, args)` → `Result<serde_json::Value, String>`
- Used by `web_search` tool to call Exa's `web_search_advanced_exa`

**Key `rmcp` types**:
- `rmcp::transport::child_process::TokioChildProcess` — stdio transport
- `rmcp::transport::streamable_http_client::StreamableHttpClientTransport` — HTTP transport
- `rmcp::service::serve_client` — create client service from transport
- `rmcp::service::Peer<RoleServer>` — peer for sending requests to server
- `rmcp::model::*` — MCP protocol types (`Tool`, `CallToolRequestParam`, etc.)

**Files modified**:
- `crates/services/Cargo.toml` — add `rmcp` dependency
- `crates/services/src/mcp.rs` — complete rewrite with `rmcp` integration

---

### Phase 10.1: `web_fetch` Tool (Full 1:1 Port)

**Input schema**:
```json
{
  "url": "string (required, must be valid URL)",
  "prompt": "string (required, what to extract from the content)"
}
```

**Output schema**:
```json
{
  "bytes": "number (size of fetched content)",
  "code": "number (HTTP response code)",
  "codeText": "string (HTTP response code text)",
  "result": "string (processed content after applying prompt)",
  "durationMs": "number (time taken)",
  "url": "string (the URL that was fetched)"
}
```

**Implementation details**:

1. **URL validation** (`url` crate):
   - Length < 2000 chars
   - No username/password in URL
   - Hostname must have ≥ 2 parts (reject localhost, intranet)
   - Protocol: http/https only, auto-upgrade http→https

2. **HTTP fetch** (`reqwest`):
   - Custom User-Agent header
   - Max content length: 10MB
   - Fetch timeout: 60 seconds
   - Redirect handling: follow only same-origin redirects (www. prefix changes allowed)
   - Max redirects: 10 (prevent redirect loops)
   - Accept header: `text/markdown, text/html, */*`

3. **HTML→markdown** (`html2text` crate):
   - For `text/html` content types: convert via `html2text`
   - For non-HTML: pass through raw content
   - Truncate to 100,000 chars before processing

4. **Preapproved hosts** (port full `PREAPPROVED_HOSTS` set — 130+ domains):
   - Anthropic: `platform.claude.com`, `code.claude.com`, `modelcontextprotocol.io`
   - Python: `docs.python.org`, `pandas.pydata.org`, `numpy.org`, `pytorch.org`, etc.
   - JavaScript: `developer.mozilla.org`, `react.dev`, `nodejs.org`, `nextjs.org`, etc.
   - Cloud: `docs.aws.amazon.com`, `kubernetes.io`, `www.docker.com`, etc.
   - Path-scoped entries: `github.com/anthropics`, `vercel.com/docs`, etc.
   - O(1) Set lookup for hostname, path-prefix matching for scoped entries

5. **LLM summarization** (Option B — preapproved = raw, others = summarized):
   - Preapproved hosts: return raw markdown directly (no LLM call, saves tokens)
   - Non-preapproved: send markdown + user prompt to secondary model via `ApiClient`
   - Secondary model prompt: "Web page content: ---{content}---\n\n{prompt}\n\n{guidelines}"
   - Guidelines differ: preapproved = "provide concise response", non-preapproved = strict 125-char quote limit

6. **Cache** (`lru` crate):
   - LRU cache with 15-minute TTL
   - 50MB size limit
   - URL-keyed
   - Separate domain-check cache (hostname-keyed, 5-min TTL, 128 entries)

7. **Redirect handling**:
   - Detect cross-host redirects (not just www. changes)
   - Return special message: "REDIRECT DETECTED: ... Please use WebFetch again with url: '{redirectUrl}'"
   - Model is expected to re-fetch with the new URL

8. **Binary content**:
   - Detect binary content types (PDF, images, etc.)
   - Save raw bytes to `.claude/webfetch-<timestamp>-<random>.<ext>`
   - Note path in result: "[Binary content (application/pdf, 2.3MB) also saved to ...]"

9. **Permission**:
   - Auto-allow for preapproved hosts
   - Passthrough for non-preapproved (permission system handles)
   - Domain-specific allow/deny rules via permission system

**Dependencies** (add to `crates/tools/Cargo.toml`):
```toml
html2text = "0.16"
url = "2"
lru = "0.12"
```

**Files**:
- `crates/tools/src/web_fetch.rs` — full implementation (replace TODO stub)
- `crates/tools/src/web_fetch/preapproved.rs` — preapproved host list (optional, can inline)

---

### Phase 10.2: `web_search` Tool (Full 1:1 Port via Exa MCP)

**Input schema**:
```json
{
  "query": "string (required, min 2 chars)",
  "allowed_domains": "string[] (optional, only include results from these domains)",
  "blocked_domains": "string[] (optional, never include results from these domains)"
}
```

**Output schema**:
```json
{
  "query": "string (the search query)",
  "results": "array of SearchResult objects or string commentaries",
  "durationSeconds": "number (time taken)"
}
```

**Three-tier architecture**:

1. **Anthropic models** (native `web_search_20250305`):
   - Make secondary API call with search tool schema as `extraToolSchemas`
   - Stream events, accumulate `server_tool_use` + `web_search_tool_result` blocks
   - Parse results: extract titles, URLs from `web_search_tool_result.content`
   - Model provides commentary in `text` blocks between search results
   - Max 8 searches per call

2. **Exa MCP** (primary fallback for non-Anthropic models):
   - Use `McpService.call_mcp_tool("exa", "web_search_advanced_exa", args)` from Phase 10.0
   - Arguments: `{ query, numResults: 5, text: true }`
   - Parse response into `SearchResult { title, url }` format
   - No API key needed (Exa MCP is free, hosted at `mcp.exa.ai`)

3. **Error** (if neither available):
   - Return helpful error: "Web search is not available. Use an Anthropic model or configure the Exa MCP server."

**Implementation details**:

1. **Progress events**:
   - `query_update` — emitted when search query is extracted from tool input
   - `search_results_received` — emitted when search results arrive, includes result count

2. **Result formatting** (for `map_tool_result_to_block`):
   ```
   Web search results for query: "{query}"

   {string commentaries}
   Links: [{title, url}, ...]

   REMINDER: You MUST include the sources above in your response to the user using markdown hyperlinks.
   ```

3. **Permission**: passthrough (model decides when to search)

4. **Enabled check**:
   - Always enabled (no provider gating needed since we have Exa fallback)
   - `is_read_only() = true`, `is_concurrency_safe() = true`

5. **Prompt** (port from TS `getWebSearchPrompt()`):
   - Includes current month/year for accurate search queries
   - Mandatory sources section requirement
   - Domain filtering support notes

**Files**:
- `crates/tools/src/web_search.rs` — full implementation (replace TODO stub)

---

### Phase 10.3: `agent` Tool (Full 1:1 Port — All 21 Features)

**Input schema**:
```json
{
  "description": "string (required, 3-5 word summary)",
  "prompt": "string (required, task description)",
  "subagent_type": "string (optional, agent type selector)",
  "model": "enum['sonnet','opus','haiku'] (optional, model override)",
  "run_in_background": "bool (optional, fire-and-forget mode)",
  "name": "string (optional, addressable name for SendMessage)",
  "team_name": "string (optional, team context)",
  "mode": "enum['acceptEdits','plan','auto'] (optional, permission mode)",
  "isolation": "enum['worktree','remote'] (optional, isolation mode)",
  "cwd": "string (optional, working directory override)"
}
```

**Output schema** (sync):
```json
{
  "status": "completed",
  "prompt": "string",
  "content": "array of content blocks",
  "totalToolUseCount": "number",
  "totalDurationMs": "number",
  "totalTokens": "number",
  "output_file": "string (path to transcript)"
}
```

**Output schema** (async):
```json
{
  "status": "async_launched",
  "agentId": "string",
  "description": "string",
  "prompt": "string",
  "outputFile": "string (path to check progress)"
}
```

#### 10.3a: Agent Definition System + Core Infrastructure

- **Agent definition loading** from `agents/` directory:
  - YAML/Markdown frontmatter parsing (name, description, agentType, model, tools, permissionMode, etc.)
  - Built-in agents: `general-purpose`, `plan`, `explore`, `verification`, `claude-code-guide`
  - Custom agents from user's `agents/` dir
  - MCP server requirements per agent (`requiredMcpServers: ["exa"]`)
  - Model frontmatter per agent (`model: opus`)
  - Isolation mode per agent (`isolation: worktree`)
  - Background mode per agent (`background: true`)
  - Color per agent (`color: "#FF5733"`)

- **Agent color management**:
  - `HashMap<String, String>` for agentType → color mapping
  - Color assignment on spawn
  - Used for grouped UI display

- **Agent tool pool assembly**:
  - Each agent gets independent tool pool based on its `permissionMode`
  - Filter by agent's `tools` allowlist / `disallowedTools` denylist
  - MCP tools included if agent's `requiredMcpServers` are connected
  - Cache-stable tool ordering (deterministic for prompt caching)

- **System prompt assembly per agent type**:
  - Each agent has its own `getSystemPrompt()` method
  - Built-in agents have hardcoded prompts
  - Custom agents use frontmatter `instructions` field
  - Environment details enhancement (cwd, git branch, OS, etc.)

- **Agent model selection**:
  - Inheritance chain: agent definition model → explicit `model` param → parent model → default
  - `'inherit'` alias resolves to parent's model
  - Fork agents inherit parent's model (cache-identical prefix)

- **MCP server requirement checking**:
  - Before spawning, check if agent's required MCP servers have tools available
  - Wait up to 30 seconds for pending servers to connect
  - Error if required servers missing: "Agent 'X' requires MCP servers matching: ..."

- **Permission filtering**:
  - Filter agents denied via `Agent(AgentName)` permission rules
  - `getDenyRuleForAgent()` — check if agent type is explicitly denied
  - Error message includes denial source: "denied by permission rule 'Agent(X)' from settings"

#### 10.3b: Sync Agent Execution

- **`runAgent` function** — full agent loop:
  - Create isolated `QueryEngine` instance for the agent
  - Assemble agent-specific system prompt + tool pool
  - Run query loop: API call → tool execution → repeat until `end_turn`
  - Collect all messages, tool use counts, token usage
  - Return `AgentResult` with content, usage stats, duration

- **Progress tracking**:
  - `createProgressTracker()` — tracks tool calls, tokens, activity
  - `createActivityDescriptionResolver()` — resolves "Running command" → "Running tests"
  - `updateProgressFromMessage()` — update tracker from each assistant message
  - `getProgressUpdate()` → `{ toolUses, tokenCount, activityDescription }`

- **Agent name registry**:
  - `Map<String, AgentId>` for `SendMessage` routing
  - Register on spawn, unregister on completion
  - Allows addressing agents by name: `SendMessage({ to: "tester", text: "..." })`

- **Output file paths**:
  - Each agent writes transcript to `.claude/agents/<agent-id>/transcript.json`
  - `getTaskOutputPath(agentId)` returns the path
  - Parent can read output file to check progress (but shouldn't — "Don't peek")

- **Agent metadata writing**:
  - Write `agentType`, `description`, `model`, `startTime` to metadata file
  - Used for `/resume` to restore agent state

- **Initial progress message**:
  - Emit `agent_progress` event with prompt text on spawn
  - Shows in UI as "Agent X: {prompt}"

- **Background hint display**:
  - After 2 seconds of running, show "This is running in the background" hint
  - Only shown in sync mode (async agents already show as background task)

#### 10.3c: Async Agent Execution

- **`registerAsyncAgent`** — background task registration:
  - Create task with `taskId`, `agentId`, `description`, `prompt`
  - Independent abort controller (not linked to parent's)
  - Background agents survive parent cancellation
  - Register in `AppState.tasks` map

- **Background signal** (auto-background after timeout):
  - `getAutoBackgroundMs()` — 120,000ms if `CLAUDE_AUTO_BACKGROUND_TASKS` or GrowthBook gate
  - `registerAgentForeground()` — registers with auto-background signal
  - After timeout, task transitions to background automatically

- **Foreground→background transition**:
  - Mid-execution handoff: clean up foreground iterator, continue in background
  - Independent summarization stop function for backgrounded closure
  - Progress tracking continues with new tracker
  - Task completion notification fires when background agent finishes

- **Agent summarization**:
  - `startAgentSummarization()` — starts summarizing agent output for notifications
  - `stop()` — stops summarization, returns summary text
  - Enabled for coordinator mode, fork subagents, or GrowthBook gate

- **Task completion notification**:
  - `enqueueAgentNotification()` — queues notification for user
  - Includes: taskId, description, status, finalMessage, usage stats
  - Renders as `<task-notification>` in UI

- **Agent kill/abort**:
  - `killAsyncAgent(agentId)` — transitions task to killed status
  - Abort controller signals cancellation
  - Worktree cleanup happens before notification

- **Worktree cleanup after async completion**:
  - `cleanupWorktreeIfNeeded()` — check if worktree has changes
  - No changes → remove worktree, clear metadata
  - Has changes → keep worktree, log path

#### 10.3d: Worktree Isolation

- **Git worktree creation**:
  - `createAgentWorktree(slug)` — `git worktree add .claude/worktrees/<slug> -b agent-<slug>`
  - Records: `worktreePath`, `worktreeBranch`, `headCommit`, `gitRoot`
  - Slug format: `agent-<agent-id-slice(8)>`

- **Working directory override**:
  - All filesystem operations inside agent use worktree path
  - `runWithCwdOverride(path, fn)` — temporarily override `getCwd()`
  - System prompt rebuilt inside override to reflect correct cwd

- **Worktree change detection**:
  - `hasWorktreeChanges(worktreePath, headCommit)` — compare HEAD before/after
  - Uses `git diff --quiet` or file change detection
  - Determines whether to keep or remove worktree

- **Auto-cleanup**:
  - No changes → `git worktree remove <path>`, delete branch
  - Has changes → keep, return path in result
  - Hook-based worktrees always kept (can't detect VCS changes)

- **Worktree notice injection** (for fork children):
  - `buildWorktreeNotice(parentCwd, childCwd)` — tells fork child about path translation
  - Appended after fork directive in prompt messages
  - Child must re-read files from new paths

#### 10.3e: Fork Subagent

- **Fork context inheritance**:
  - Fork inherits parent's FULL conversation context (all assistant messages with tool_use blocks)
  - Cache-identical system prompt forwarding (parent's rendered system prompt, not FORK_AGENT's)
  - `buildForkedMessages(prompt, assistantMessage)`:
    - Clone parent's assistant messages as user messages
    - Add placeholder tool_results for each tool_use block
    - Append fork directive + user prompt

- **Fork guard**:
  - Prevent recursive fork in children
  - Check `querySource === "agent:builtin:fork"` or scan messages for fork child marker
  - Error: "Fork is not available inside a forked worker."

- **Cache-identical prefix**:
  - Fork uses parent's exact tool array (not rebuilt under different permission mode)
  - `useExactTools: true` flag in `runAgent` params
  - Inherits parent's `thinkingConfig` and `isNonInteractiveSession`

- **Directive-style prompt generation**:
  - Fork prompt is a directive (what to do), not a briefing (what the situation is)
  - Since fork inherits context, don't re-explain background
  - Be specific about scope: what's in, what's out, what another agent is handling

- **Fork examples in tool prompt** (port from TS):
  - Ship audit example
  - Mid-wait status example
  - Migration review example
  - All demonstrate proper fork usage patterns

#### 10.3f: Agent Swarms (tmux/iTerm2 Spawning)

- **`spawnTeammate`** — main entry point for teammate spawning:
  - Routes to: split-pane, separate window, or in-process based on backend detection

- **tmux session/window/pane management**:
  - `tmux has-session -t <name>` — check session existence
  - `tmux new-session -d -s <name>` — create detached session
  - `tmux new-window -t <session> -n <name> -P -F '#{pane_id}'` — create window
  - `tmux split-window` — split current window
  - `tmux send-keys -t <target> <command> Enter` — send command to pane

- **iTerm2 native split pane** (via `it2` CLI):
  - `it2 session new` — create new session
  - `it2 pane split` — split current pane
  - Setup prompt if `it2` not installed (user can install or fall back to tmux)

- **Backend detection**:
  - Check tmux availability (`tmux -V`)
  - Check iTerm2 + `it2` availability
  - Check in-process feature flag
  - Preference order: configurable (auto, tmux, iterm2, in-process)
  - Cache detection result (reset on setup change)

- **Team file management**:
  - Read/write YAML/JSON team files
  - `TeamFile { name, members: [TeamMember], leadAgentId }`
  - `TeamMember { agentId, name, agentType, model, prompt, color, planModeRequired, joinedAt, tmuxPaneId, cwd, subscriptions, backendType }`
  - Sanitize names (no `@` in agent IDs)
  - Unique name generation (suffix collision: `tester` → `tester-2` → `tester-3`)

- **Mailbox system** for inter-agent communication:
  - `.claude/mailbox/<team-name>/<agent-name>.json`
  - Leader writes: `{ from, text, timestamp }`
  - Teammate polls mailbox on startup, picks up initial prompt
  - File-based IPC (no network, works cross-platform)

- **Agent identity CLI args**:
  - `--agent-id <id>`
  - `--agent-name <name>`
  - `--team-name <team>`
  - `--agent-color <color>`
  - `--parent-session-id <id>`
  - `--plan-mode-required` (if plan mode)
  - `--agent-type <type>`

- **Permission mode propagation**:
  - Inherit parent's permission mode via CLI flags
  - `--permission-mode acceptEdits` / `--permission-mode auto` / `--dangerously-skip-permissions`
  - Plan mode takes precedence (don't inherit bypass permissions)

- **Model resolution**:
  - `'inherit'` → parent's model
  - `undefined` → default (hardcoded Opus fallback)
  - Explicit model → use specified model
  - Remove inherited `--model` flag if overriding

- **Task registration for background agents**:
  - `registerOutOfProcessTeammateTask()` — makes tmux teammates visible in tasks pill
  - Task state: `InProcessTeammateTaskState` with identity, prompt, abort controller
  - Abort signal → kill pane via backend (`tmux kill-pane` or `it2 session close`)

#### 10.3g: Handoff Classification + Agent Memory

- **Handoff classifier**:
  - Transcript quality assessment after agent completion
  - Checks if agent result is complete or needs follow-up
  - Adds warning to notification if handoff quality is poor
  - Uses secondary model call for classification

- **Agent memory snapshots**:
  - Scope-based memory loading (project, user, team scopes)
  - Memory loaded into agent's system prompt
  - `getAgentMemorySnapshot(agentType)` → memory content
  - Memory event logging: `tengu_agent_memory_loaded`

- **Agent progress summaries**:
  - SDK agent progress summaries enabled via GrowthBook gate
  - Summarizes tool use, tokens, duration for display
  - `getSdkAgentProgressSummariesEnabled()`

#### 10.3h: Coordinator Mode + Proactive + Remote

- **Coordinator mode**:
  - Slim agent prompt (system prompt already covers usage notes)
  - Multi-agent orchestration (coordinator spawns and manages agents)
  - `isCoordinatorMode()` — check `CLAUDE_CODE_COORDINATOR_MODE` env var
  - Force all agents async in coordinator mode

- **Proactive mode**:
  - Proactive agent triggering based on context
  - `isProactiveActive()` — check if proactive mode is enabled
  - Force all agents async when proactive mode active

- **Remote agent isolation** (ant-only, stub for external builds):
  - `teleportToRemote()` — create remote CCR session
  - `checkRemoteAgentEligibility()` — check if eligible for remote
  - `registerRemoteAgentTask()` — register with remote task tracker
  - `getRemoteTaskSessionUrl(sessionId)` — URL to view remote session
  - Status: `remote_launched` with taskId, sessionUrl, outputFile
  - **External builds**: dead code elimination via `cfg!(feature = "ant")`

- **GrowthBook feature gates**:
  - All gated features with `cfg!` macros or env var checks
  - `CLAUDE_CODE_DISABLE_BACKGROUND_TASKS` — disable all background agents
  - `CLAUDE_AUTO_BACKGROUND_TASKS` — enable auto-background after timeout
  - `CLAUDE_CODE_AGENT_LIST_IN_MESSAGES` — agent list as attachment vs inline
  - `CLAUDE_CODE_COORDINATOR_MODE` — coordinator mode
  - `USER_TYPE === 'ant'` — ant-only features (remote isolation, analytics)

- **Agent swarms constants**:
  - `SWARM_SESSION_NAME = "claude-swarm"`
  - `TEAM_LEAD_NAME = "lead"`
  - `TMUX_COMMAND = "tmux"`
  - `TEAMMATE_COMMAND_ENV_VAR = "CLAUDE_CODE_TEAMMATE_COMMAND"`

**Files**:
- `crates/tools/src/agent.rs` — main AgentTool implementation
- `crates/tools/src/agent/definitions.rs` — agent definition loading
- `crates/tools/src/agent/built_in.rs` — built-in agent definitions
- `crates/tools/src/agent/colors.rs` — agent color management
- `crates/tools/src/agent/run.rs` — runAgent function (agent loop)
- `crates/tools/src/agent/progress.rs` — progress tracking
- `crates/tools/src/agent/async_agent.rs` — async agent registration + lifecycle
- `crates/tools/src/agent/worktree.rs` — git worktree isolation
- `crates/tools/src/agent/fork.rs` — fork subagent logic
- `crates/tools/src/agent/spawn.rs` — teammate spawning (tmux/iTerm2/in-process)
- `crates/tools/src/agent/mailbox.rs` — file-based mailbox system
- `crates/tools/src/agent/team_file.rs` — team file read/write
- `crates/tools/src/agent/memory.rs` — agent memory snapshots
- `crates/tools/src/agent/handoff.rs` — handoff classification
- `crates/tools/src/agent/prompt.rs` — agent tool prompt (full port from TS)

---

### Phase 10.4: Registry Update

**Update `crates/tools/src/registry.rs`**:
- Add `WebFetchTool`, `WebSearchTool`, `AgentTool` to `default_registry()`
- Agent tool requires shared state for agent definitions → pass via constructor
- Web tools require API client access for summarization → pass via constructor

**Updated `default_registry()`**:
```rust
pub fn default_registry(api_client: Arc<ApiClient>, mcp_service: Arc<McpService>) -> Self {
    let mut registry = Self::new();
    registry.register(BashTool::new());
    registry.register(FileReadTool::new());
    registry.register(FileWriteTool::new(read_state.clone()));
    registry.register(FileEditTool::new(read_state));
    registry.register(GrepTool::new());
    registry.register(GlobTool::new());
    registry.register(WebFetchTool::new(api_client.clone()));
    registry.register(WebSearchTool::new(api_client, mcp_service));
    registry.register(AgentTool::new(api_client, mcp_service));
    registry
}
```

---

### Phase 10.5: Compilation Verification

- `cargo check` — verify all crates compile cleanly
- `cargo clippy` — lint checks
- `cargo fmt` — format all code
- Commit each sub-phase separately with descriptive messages
