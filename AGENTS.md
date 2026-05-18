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

Note: `cc-tui` crate not yet created — will depend on `cc-core` and `cc-query`.
