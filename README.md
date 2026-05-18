# Clodox

A complete 1:1 Rust port of [Claude Code](https://docs.anthropic.com/en/docs/claude-code/overview) — an agentic coding assistant with a terminal UI, MCP protocol support, advanced agent orchestration, and a bridge system for remote sessions.

## Features

- **Full TUI** — ratatui-based terminal interface with markdown rendering, syntax highlighting, virtual scrolling, spinner animations, and permission dialogs
- **62 Commands** — `/help`, `/clear`, `/compact`, `/config`, `/login`, `/resume`, `/diff`, `/cost`, `/commit`, `/review`, `/memory`, `/mcp`, `/theme`, `/vim`, `/agents`, and more
- **Core Tools** — Bash, Read, Write, Edit, Grep, Glob with permission system and risk assessment
- **Advanced Tools** — WebFetch (URL→markdown), WebSearch (Exa MCP), Agent (sync/async execution, worktree isolation, fork, swarms)
- **MCP Protocol** — Full client via `rmcp` with stdio + HTTP/SSE transports, tool/resource discovery
- **Bridge System** — Daemon process, REPL bridge, JWT auth, message protocol, session runner with concurrency management
- **Service Layer** — API service with rate limiting/caching, analytics, plugin management, LSP integration, token estimation, memory extraction

## Architecture

```
clodox/
├── crates/
│   ├── core/        # Types, messages, permissions, tools trait, settings, state
│   ├── query/       # API client, streaming, query engine, compaction, retry
│   ├── tools/       # Bash, Read, Write, Edit, Grep, Glob, WebFetch, WebSearch, Agent
│   ├── commands/    # 62 commands across 4 tiers
│   ├── services/    # API, MCP, analytics, plugins, LSP, token estimation, memory
│   ├── bridge/      # Daemon, REPL bridge, messaging, transport, JWT, session runner
│   ├── tui/         # ratatui TUI, markdown rendering, spinner, prompts, screens
│   └── cli/         # CLI entrypoint, session management, logging
└── ...
```

## Tech Stack

| Component | Crate |
|---|---|
| TUI | `ratatui` + `crossterm` |
| CLI | `clap` |
| Async | `tokio` + `async-trait` |
| Serialization | `serde` + `serde_json` |
| HTTP | `reqwest` (rustls) |
| MCP | `rmcp` |
| Markdown | `comrak` + `syntect` |
| HTML→Markdown | `html2text` |
| Caching | `lru` |

## Build & Run

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

# Format
cargo fmt

# Lint
cargo clippy
```

## Project Status

| Phase | Status |
|---|---|
| 1. Core Types & Query Engine | ✅ Complete |
| 2. Core Tools | ✅ Complete |
| 3. CLI & Settings | ✅ Complete |
| 4. TUI Engine | ✅ Complete |
| 5. Command System (62 commands) | ✅ Complete |
| 6. Service Layer | ✅ Complete |
| 7. Bridge System | ✅ Complete |
| 8. Auxiliary Systems (Vim, Skills, Voice) | ⏸ Deferred |
| 9. TUI Integration & Polish | ✅ Complete |
| 10. MCP Protocol + Web Tools + Agent | ✅ Complete |
| 11. TUI Integration for Advanced Tools | ✅ Complete |
| **Tests** | **381 unit tests** |

## Stats

- **39,278 lines** of Rust code
- **381 unit tests** across all 8 crates
- **0 compilation errors**
- **141 tests** in the bridge system alone

## License

MIT
