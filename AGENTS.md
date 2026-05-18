# AGENTS.md — Issues, Notes, and Decisions Tracker

This file tracks issues encountered, architecture decisions, and things to remember during the port.

---

## Issues Encountered

### 1. OpenSSL Dependency (2026-05-15)
**Problem**: `reqwest` default features pull in `native-tls` which requires system OpenSSL via `pkg-config`.
**Fix**: Changed `reqwest` to use `rustls-tls` instead:
```toml
reqwest = { version = "0.12", default-features = false, features = ["json", "stream", "rustls-tls"] }
```

### 2. PermissionResult Serialization Conflict (2026-05-15)
**Problem**: `PermissionResult` used `#[serde(flatten)]` to wrap `PermissionDecision`, but `PermissionDecisionReason` contained `HashMap<String, PermissionResult>` creating a circular serialization dependency.
**Fix**: Flattened `PermissionResult` into direct variants (`Allow`, `Ask`, `Deny`, `Passthrough`) instead of wrapping `PermissionDecision`. Removed `Serialize/Deserialize` from `PermissionDecisionReason` (used internally only).

### 3. Debug Derive on Dyn Trait Fields (2026-05-15)
**Problem**: `#[derive(Debug)]` on structs containing `Arc<dyn Fn() + Send + Sync>` fails because `dyn Fn` doesn't implement `Debug`.
**Fix**: Manual `Debug` implementations for `ToolUseOptions`, `SpeculationState`, `CommandBase`.

### 4. SettingSource Duplication (2026-05-15)
**Problem**: `SettingSource` was defined in both `types/mod.rs` and `commands/mod.rs`.
**Fix**: Defined once in `types/mod.rs`, imported in `commands/mod.rs`.

### 5. PermissionResult::Decision Variant Removed (2026-05-15)
**Problem**: After flattening `PermissionResult`, code in `tools/mod.rs` still referenced `PermissionResult::Decision(PermissionDecision::Allow { ... })`.
**Fix**: Changed to direct `PermissionResult::Allow { ... }`.

### 6. AgentDefinition Type Conflict (2026-05-18)
**Problem**: `cc_core::tools::AgentDefinition` (minimal: name, description, agent_type) conflicted with local `AgentDefinition` in `agent.rs` (rich: model, tools, permissions, MCP requirements, etc.). Rust doesn't allow same-name types in scope.
**Fix**: Renamed local type to `FullAgentDefinition`. Added conversion from `cc_core::tools::AgentDefinition` → `FullAgentDefinition` in `get_active_agents()` and `prompt()` methods.

---

## Architecture Decisions

### 1. Tool Trait Uses `serde_json::Value` for I/O
**Decision**: The `Tool` trait uses `serde_json::Value` for input/output rather than generic types.
**Rationale**: Matches TypeScript's dynamic nature. Tools define their own JSON schemas. Generic types would require per-tool trait implementations that are harder to compose in a registry.
**Revisit**: Consider generic `Tool<Input, Output>` if type safety becomes critical.

### 2. Store Uses `std::sync::RwLock`
**Decision**: The `Store<T>` uses `std::sync::RwLock` rather than `tokio::sync::RwLock`.
**Rationale**: State updates are fast (in-memory clone). `std::sync::RwLock` has lower overhead for short critical sections.
**Revisit**: If state updates become async-heavy or cause blocking, switch to `tokio::sync::RwLock`.

### 3. No JSX Equivalent
**Decision**: Tool render methods (`renderToolUseMessage`, etc.) will return `Vec<TextSpan>` or `String` rather than a JSX-like DSL.
**Rationale**: Rust has no JSX. ratatui uses a widget-based approach. Building a JSX-like macro would add complexity without clear benefit.
**Revisit**: Consider a macro-based DSL if component composition becomes unwieldy.

### 4. Feature Flags via Cargo Features
**Decision**: Replace `bun:bundle` feature() gates with Cargo features + `cfg!` macros.
**Rationale**: Idiomatic Rust approach. Dead code elimination works at the crate level.
**Revisit**: Consider `build.rs` for more granular compile-time DCE if needed.

### 5. Async Generators via `futures::stream::Stream`
**Decision**: Replace TypeScript `async function*` with `impl Stream<Item = T>`.
**Rationale**: Standard Rust async streaming pattern. Compatible with `tokio` ecosystem.
**Revisit**: Consider `async-stream` crate for simpler generator syntax.

---

## TypeScript → Rust Mapping Notes

| TypeScript | Rust Equivalent |
|---|---|
| `DeepImmutable<T>` | Rust ownership (default immutable), `Clone` for sharing |
| `React.ReactNode` | `Vec<ratatui::text::Span>` or `String` |
| `async function*` | `impl Stream<Item = T>` or `async-stream` |
| `zod` schema | `serde` derive + manual validation |
| `bun:bundle feature()` | `cfg!(feature = "...")` |
| `require()` (lazy) | `Arc<dyn Trait>` + lazy init |
| `lodash-es/memoize` | `once_cell::sync::Lazy` |
| `lodash-es/last` | `slice::last()` |
| `lodash-es/isObject` | `matches!(x, serde_json::Value::Object(_))` |
| `crypto.randomUUID()` | `uuid::Uuid::new_v4()` |
| `node:fs` | `tokio::fs` |
| `node:path` | `std::path::Path` |
| `node:os` | `dirs` crate, `std::env` |
| `node:vm` (REPL) | No direct equivalent — may need `rquickjs` or skip |
| `@anthropic-ai/sdk` | `reqwest` + custom types |
| `@modelcontextprotocol/sdk` | Custom MCP impl |
| `Commander.js` | `clap` |
| `GrowthBook` | Custom feature flags |
| `OpenTelemetry + gRPC` | `opentelemetry` + `tonic` |

---

## Known Gaps / TODOs

1. **REPLTool**: Uses Node.js `vm` module for code execution. No Rust equivalent. Options:
   - Skip (ant-only feature)
   - Use `rquickjs` (embedded QuickJS)
   - Use `deno_core` (embedded V8)

2. **Voice Mode**: Feature-flagged in TypeScript. Requires audio capture + STT. Defer until core is working.

3. **MCP OAuth**: Full OAuth flow for MCP servers. Complex — defer.

4. **Agent Swarms**: Multi-agent orchestration with team management. Complex state machine — defer.

5. **Speculative Execution**: Predictive pre-execution with overlay filesystem. Advanced — defer.

6. **Bridge System**: Remote session management with dual transports. Complex — defer.

7. **Plugin Marketplace**: Plugin installation from registry. Defer.

8. **LSP Integration**: Language Server Protocol for code intelligence. Defer.

9. **Vim Mode**: Full vim keybindings and modal editing. Defer.

10. **Computer Use**: MCP-based screen control. Defer.

11. **MCP Commands (`/mcp`)**: Stubs in Phase 5. Full implementation needs Phase 6 Service Layer:
    - MCP server lifecycle management (start/stop/restart servers)
    - Server connection status display
    - Tool listing from connected servers
    - Dynamic MCP config add/remove/enable/disable
    - Server stdout/stderr logging
    - OAuth flow for MCP servers requiring auth
    - Resource browsing from MCP servers
    - Elicitation handler for MCP prompts
    - Server-scoped permission rules
    - Plugin reconnect key management
    - MCP command registration from server tools

---

## Completed Phases

### Phase 1: Core Types & Query Engine ✅
- API types, streaming client, retry logic
- QueryEngine state machine, compaction system
- System prompt assembly

### Phase 2: Core Tools ✅
- Bash, Read, Write, Edit, Grep, Glob tools
- Tool registry, utilities

### Phase 3: CLI & Settings ✅
- CLI argument parsing (50+ flags, 5 subcommands)
- Logging/tracing, settings system (8 parts)
- Session lifecycle

### Phase 4: TUI Engine ✅ (18/18 sub-phases)
- Terminal backend, theme system, component model
- Message/tool rendering, spinner system, markdown rendering
- PromptInput, status line, permission dialogs, setup screens
- Input handling, virtual scrolling, animations
- State management, main loop, screen layouts

### Phase 5: Command System ✅
- `Command` trait, `CommandRegistry`, fuzzy matching
- **62 commands implemented** (0 TODO stubs remaining):
  - Tier 1 (9): `/help`, `/clear`, `/compact`, `/config`, `/login`, `/logout`, `/resume`, `/diff`, `/cost`
  - Tier 2 (9): `/commit`, `/review`, `/memory`, `/mcp`, `/theme`, `/vim`, `/context`, `/model`, `/skills`, `/tasks`
  - Tier 3 (16): `/doctor`, `/share`, `/pr_comments`, `/permissions`, `/output_style`, `/feedback`, `/hooks`, `/effort`, `/fast`, `/brief`, `/agents`, `/branch`, `/copy`, `/exit`, `/version`
  - Utility (28): `/btw`, `/stats`, `/status`, `/files`, `/export`, `/rename`, `/color`, `/release_notes`, `/keybindings`, `/passes`, `/plan`, `/sandbox_toggle`, `/terminal_setup`, `/upgrade`, `/usage`, `/voice`, `/chrome`, `/ide`, `/init`, `/remote_setup`, `/remote_env`, `/privacy_settings`, `/rate_limit_options`, `/reload_plugins`, `/stickers`, `/tag`, `/thinkback`, `/thinkback_play`

### Phase 8: Auxiliary Systems (Deferred)
- 8.1 Vim Mode — modal editing, keybindings
- 8.2 Skills System — skill loading/discovery
- 8.3 Coordinator — multi-agent orchestration
- 8.4 Voice — audio capture, STT
- 8.5 Migrations — config/data migration

### Phase 9: TUI Integration & Polish ✅ (9/9 sub-phases)
- State enrichment (`QueryState`, `TurnTokenCounts`, `PendingToolCall`, `PermissionDialogState`)
- Query engine integration (tokio runtime, mpsc channel, `spawn_query()`, `poll_query_events()`)
- Stream event → state (`StreamingAccumulator`, `handle_query_event()`, cost estimation)
- Permission system (risk assessment, dialog rendering, permission mode routing)
- Dynamic tool rendering (collapsible blocks, streaming variants)
- Spinner enhancement (phase-aware verbs)
- Cancel/abort (Ctrl+C routing, watch channel)
- Screen navigation (Escape navigates between screens)
- Token/cost display (footer pills, adaptive formatting)

### Phase 10: MCP Protocol & Advanced Tools ✅ (5/5 sub-phases)
- 10.0: MCP Protocol via `rmcp` crate — stdio + HTTP/SSE transports, Exa MCP integration
- 10.1: `web_fetch` tool — URL validation, HTML→markdown, 130+ preapproved hosts, LRU cache
- 10.2: `web_search` tool — Exa MCP search, domain filtering, result parsing
- 10.3: `agent` tool — definition system, built-in agents, color management, sync/async execution
- 10.4: Registry update — register all new tools, `AgentColorManager` shared instance
- 10.5: Final compilation verification — clean `cargo check` across all crates

### Phase 11: TUI Integration for Advanced Tools ✅ (1/1 sub-phases)
- 11.0: Tool rendering — `web_fetch`, `web_search`, `agent` display components, shared message converter, spinner integration

---

## Remaining Work

### Phase 6: Service Layer ✅ (10/10 sub-phases)
- 6.1: API Service — client wrapper, rate limiting, request caching, retry orchestration, usage tracking
- 6.2: MCP Service — server lifecycle (stdio + HTTP/SSE), tool/resource discovery, Exa integration, `McpToolCaller` trait
- 6.3: Analytics Service — event tracking, session metrics, feature flags (GrowthBook-compatible)
- 6.4: Plugin Service — discovery, manifest parsing, enable/disable, directory scanning
- 6.5: LSP Service — server management, file mapping, diagnostics/completions/symbols stubs
- 6.6: Token Estimation — per-model pricing, cost calculation, context window tracking
- 6.7: Team Memory Sync — shared memory, version-based conflict detection, file-based sync
- 6.8: Extract Memories — CLAUDE.md parsing, project file extraction, conversation analysis
- 6.9: Prompt Suggestion — template-based suggestions, keyword matching, confidence scoring
- 6.10: Tests — 103 unit tests across all services

### Phase 7: Bridge System (Not Started)
- REPL bridge
- Transport layer
- Messaging protocol
- Daemon process

### Phase 8: Auxiliary Systems (Deferred)
- Vim Mode (basic toggle exists, full modal editing deferred)
- Skills System (plugin listing exists, dynamic discovery deferred)
- Coordinator (multi-agent orchestration deferred)
- Voice (audio capture + STT deferred)
- Migrations (config version migration deferred)

### Missing Tools (Not Implemented)
- (all core tools implemented — advanced agent features deferred)

---

## Build Commands

```bash
# Check compilation
cargo check

# Build
cargo build

# Build release
cargo build --release

# Run
cargo run --bin claude-code

# Run with verbose logging
RUST_LOG=debug cargo run --bin claude-code

# Test
cargo test

# Test specific crate
cargo test -p cc-core

# Format
cargo fmt

# Lint
cargo clippy
```

---

## Crate Dependency Graph

```
cc-cli
├── cc-core
├── cc-query (→ cc-core)
├── cc-tools (→ cc-core)
├── cc-commands (→ cc-core)
├── cc-services (→ cc-core)
└── cc-bridge (→ cc-core, cc-services)
```

Note: `cc-tui` crate is created and fully functional — depends on `cc-core` and `cc-commands`.
