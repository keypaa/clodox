# Phase 4: TUI Engine — Complete Plan

## Architecture Mapping: Ink → Ratatui

| Ink Concept | Ratatui Equivalent |
|---|---|
| React component tree | Widget trait implementations |
| Yoga WASM layout | `ratatui::layout::Layout` + `Constraint` |
| Double-buffered Frame | `Terminal::draw()` (already double-buffers) |
| Packed Int32Array cells | `Buffer` (ratatui's cell buffer) |
| StylePool/CharPool | `Style` + `Span` (ratatui's styling) |
| Diff engine | `Terminal::draw()` diff (built-in) |
| DECSTBM scroll | ratatui scroll region + `Clear` |
| Blit optimization | ratatui's `Buffer::merge()` |
| TextInput | Custom widget with `crossterm::event` |
| KeybindingContext | Custom keybinding resolver |
| React state | `Arc<RwLock<AppState>>` |

## Terminal Mode

- **Default: Main-screen mode** — messages render to native terminal scrollback, input at bottom (matches TS default)
- **Optional: Alt-screen mode** — full TUI with virtual scrolling (`CLAUDE_CODE_FULLSCREEN=1`)

## Visual Layout (Main-Screen Mode — Default)

```
[Logo + Status Notices]          ← LogoV2 + StatusNotices (rendered once, scrolls up)

● user: what is 2+2?            ← User message: › pointer + text
● 4                              ← Assistant message: ● dot + markdown
● Bash (ls -la)                  ← Tool use: loader + bold name + (details)
  Running…                       ← Status text (dim)
● Search (pattern: "error")      ← Grep tool: userFacingName + details

⠋ Requesting…                   ← SpinnerWithVerb (braille glyph + verb)

┌──────────────────────────────┐ ← PromptInput border (round, bottom only)
› _                              ← ModeIndicator + TextInput (inverse cursor)
└──────────────────────────────┘
default · ctrl+c exit · ctrl+o transcript  ← PromptInputFooter (dim)
```

## Visual Layout (Alt-Screen Mode — `CLAUDE_CODE_FULLSCREEN=1`)

```
┌─────────────────────────────────────────────┐
│ [LogoHeader]                                │
│                                             │
│ [ScrollBox - Messages] (flexGrow=1)         │
│   ● user: what is 2+2?                     │
│   ● 4                                       │
│   ● Bash (ls -la)                          │
│     Running…                                │
│   [N new messages] pill                     │
│                                             │
├─────────────────────────────────────────────┤
│ ⠋ Requesting…                              │
│                                             │
│ ┌─────────────────────────────────────────┐ │
│ │ › _                                     │ │
│ └─────────────────────────────────────────┘ │
│ default · ctrl+c exit · ctrl+o transcript   │
└─────────────────────────────────────────────┘
```

## Exact Message Rendering

| Component | Visual Output |
|-----------|--------------|
| **UserTextMessage** | `› <text>` — pointer `›` color `suggestion`, text color `text`, background `userMessageBackground` |
| **UserPromptMessage** | Same as UserTextMessage with metadata |
| **UserCommandMessage** | `/<command> <args>` — shown as command tag |
| **UserToolSuccessMessage** | Result text + `✓ Auto-approved · matched "rule"` (dim, green tick) |
| **UserToolErrorMessage** | Error text in red, max 10 lines, `… +N lines (ctrl+o to see all)` truncation hint |
| **UserToolCanceledMessage** | `Interrupted · What should Claude do instead?` (dim) |
| **UserToolRejectMessage** | Rejected tool use with input details |
| **AssistantTextMessage** | `● <markdown>` — dot `●` (U+25CF, Linux) or `⏺` (macOS), color `text`/`suggestion` if selected |
| **AssistantToolUseMessage** | `[loader] **TOOL NAME** (details)` — loader: blinking ● (dim=unresolved, green=resolved, red=errored), tool name: **bold**, details: plain text in `()` |
| **AssistantThinkingMessage** | `∴ Thinking (ctrl+o to expand)` — dim, italic, `∴` = U+2234 |
| **SystemAPIErrorMessage** | Error text in red, truncated to 1000 chars + `…` |
| **RateLimitMessage** | Rate limit warning with upgrade hint |
| **CompactBoundaryMessage** | Context compaction markers |

## Tool Use Rendering (Exact)

| Tool | userFacingName | Details (in parentheses) | Status Text |
|------|---------------|-------------------------|-------------|
| **Bash** | `Bash` / `SandboxedBash` | command (truncated 2 lines, 160 chars) | `Running…` / `Waiting for permission…` |
| **Read** | `Read` / `Reading Plan` | displayPath [+ `· pages N` / `· lines X-Y`] | — |
| **Write** | `Write` / `Updated plan` | displayPath | — |
| **Edit** | `Update` / `Create` | displayPath | — |
| **Grep** | `Search` | `pattern: "..."[, path: "..."]` | — |
| **Glob** | `Search` | `pattern: "..."[, path: "..."]` | — |

## Spinner System (Exact)

- **Glyph**: Braille characters cycling forward then reverse at 120ms intervals
- **Reduced motion**: Single dot `●` with 2-second visible/dim cycle
- **Stall detection**: After 3s of no tokens, spinner gradually turns red (ERROR_RED = RGB(171, 43, 63)), intensity fades over 2s with 0.1 factor per 50ms tick
- **Verb**: `verb + "…"` — selected from overrideMessage → currentTodo.activeForm → currentTodo.subject → randomVerb
- **Tips** (shown after elapsed time):
  - `>30 min`: `"Use /clear to start fresh when switching topics and free up context"`
  - `>30s, no /btw`: `"Use /btw to ask a quick side question without interrupting Claude's current work"`
- **Idle state**: `∗ Idle · teammates running` (dim)

## TextInput (Exact)

- **Cursor**: Inverse video space character (`Style::reversed()` + `" "`)
- **Pointer**: `›` (U+203A) + space, color `suggestion`
- **Border**: Round style, bottom only, color `promptBorder`
- **Enter**: Submits on last line, inserts `\n` otherwise
- **Ctrl+C**: Double-press to exit (1st shows "Ctrl-C again to exit")
- **Ctrl+D**: If input empty → double-press to exit; if non-empty → delete forward char
- **Ctrl+A/E**: Start/end of line
- **Ctrl+K**: Kill to end of line (push to kill ring)
- **Ctrl+U**: Kill to start of line (push to kill ring)
- **Ctrl+W**: Kill word before (push to kill ring)
- **Ctrl+Y**: Yank from kill ring
- **Meta+Y**: Cycle through kill ring
- **Tab**: Autocomplete accept
- **Escape**: Double-press to clear input (1st shows "Esc again to clear" notification)
- **Up/Down**: History navigation when at first/last line
- **Kill ring**: Global shared, max 10 entries, consecutive kills accumulate

## Key Bindings (Global + Chat Contexts)

| Key | Action |
|-----|--------|
| `ctrl+c` | Interrupt (double-press to exit) |
| `ctrl+d` | Exit (if input empty, double-press) |
| `ctrl+l` | Redraw |
| `ctrl+o` | Toggle transcript mode |
| `ctrl+e` | Toggle show all in transcript |
| `ctrl+r` | History search |
| `escape` | Cancel / dismiss dialogs |
| `tab` | Autocomplete accept |
| `shift+tab` | Cycle mode |
| `enter` | Submit prompt |
| `up` | History previous |
| `down` | History next |
| `ctrl+a` | Start of line |
| `ctrl+e` | End of line |
| `ctrl+k` | Kill to end of line |
| `ctrl+u` | Kill to start of line |
| `ctrl+w` | Kill word before |
| `ctrl+y` | Yank |
| `meta+y` | Cycle kill ring |
| `meta+b` | Previous word |
| `meta+f` | Next word |
| `pageup` | Scroll page up |
| `pagedown` | Scroll page down |
| `home` | Start of line |
| `end` | End of line |

## Theme Colors (Dark Theme Default)

| Color Key | Purpose |
|-----------|---------|
| `text` | Primary text color |
| `error` | Error messages (red) |
| `warning` | Warnings (yellow) |
| `suggestion` | Suggestions/highlights (blue) |
| `inactive` | Dimmed/inactive text |
| `inverseText` | Inverted colors (for badges) |
| `messageActionsBackground` | Message action bg |
| `subtle` | Subtle text |
| `success` | Success indicators (green) |
| `userMessageBackground` | User message bg |
| `promptBorder` | Input border color |

## Small Terminal Behavior

- `is_short` (fullscreen && rows < 24): hides StatusLine in footer
- `is_narrow` (columns < 80): footer uses column layout, gap=0
- Input max visible lines clamped to prevent full repaint
- Progressive width gating — thinking/timer/tokens hide when space insufficient
- `MIN_INPUT_VIEWPORT_LINES = 3` for PromptInput
- `PROMPT_FOOTER_LINES = 5` reserved for footer

## State Management

- Pattern: `Arc<RwLock<AppState>>` — simple shared state (not Redux)
- Rationale: Ratatui redraws everything every frame with built-in diff; no need for fine-grained subscriptions
- AppState fields: tool_permission_context, messages, tasks, viewing_agent_task_id, main_loop_model, thinking_enabled, fast_mode, effort_value, footer_selection, status_line_text, speculation, prompt_suggestion

## Markdown Rendering

- Parser: `comrak` (CommonMark)
- Syntax highlighting: `syntect`
- LRU token cache: 500 entries
- Fast path: skip parsing if no markdown syntax in first 500 chars
- `StreamingMarkdown`: stable prefix tracking, incremental re-parsing

## Dependencies

```toml
ratatui = "0.29"
crossterm = "0.28"
comrak = "0.31"
syntect = "5"
unicode-width = "0.2"
unicode-segmentation = "1"
lru = "0.12"
```

## File Structure

```
crates/tui/
├── Cargo.toml
└── src/
    ├── lib.rs
    ├── terminal.rs          # Terminal backend (4.2)
    ├── theme.rs             # Theme system (4.3)
    ├── state.rs             # State management (4.17)
    ├── input.rs             # Input handling (4.14)
    ├── animations.rs        # Animations (4.16)
    ├── virtual_scroll.rs    # Virtual scrolling (4.15)
    ├── main_loop.rs         # TUI main loop (4.18)
    ├── screens/
    │   ├── mod.rs
    │   ├── repl.rs          # Main-screen REPL (4.5)
    │   ├── fullscreen.rs    # Alt-screen fullscreen (4.5)
    │   ├── onboarding.rs    # First-run (4.13)
    │   ├── trust_dialog.rs  # Workspace trust (4.13)
    │   └── resume.rs        # Session picker (4.13)
    └── components/
        ├── mod.rs
        ├── box.rs           # Box widget (4.4)
        ├── text.rs          # Text widget (4.4)
        ├── messages/
        │   ├── mod.rs
        │   ├── row.rs       # MessageRow
        │   ├── user_text.rs
        │   ├── user_prompt.rs
        │   ├── user_command.rs
        │   ├── user_tool_result.rs
        │   ├── assistant_text.rs
        │   ├── assistant_tool_use.rs
        │   ├── assistant_thinking.rs
        │   ├── system_error.rs
        │   └── rate_limit.rs
        ├── tool_use/
        │   ├── mod.rs
        │   ├── loader.rs
        │   ├── bash.rs
        │   ├── read.rs
        │   ├── write.rs
        │   ├── edit.rs
        │   ├── grep.rs
        │   └── glob.rs
        ├── spinner/
        │   ├── mod.rs
        │   ├── glyph.rs
        │   ├── animation_row.rs
        │   ├── with_verb.rs
        │   ├── brief.rs
        │   └── stall_detection.rs
        ├── prompt_input/
        │   ├── mod.rs
        │   ├── text_input.rs
        │   ├── history.rs
        │   ├── autocomplete.rs
        │   └── footer.rs
        ├── markdown.rs      # Markdown + StreamingMarkdown (4.9)
        ├── markdown_table.rs
        ├── status_line.rs   # Status line (4.11)
        └── permissions/
            ├── mod.rs
            ├── dialog.rs
            ├── bash.rs
            ├── file_edit.rs
            ├── file_write.rs
            ├── filesystem.rs
            └── ask_user.rs
```

## Sub-Phases (18 parts, implemented sequentially)

### 4.1: TUI Crate Setup
- `crates/tui/Cargo.toml` with dependencies
- `crates/tui/src/lib.rs` — module exports

### 4.2: Terminal Backend (`crates/tui/src/terminal.rs`)
- crossterm init: raw mode, alternate screen (optional), synchronized output (DEC BSU/ESU)
- Kitty Keyboard Protocol: `PushKeyboardEnhancementFlags`
- Bracketed paste: `EnableBracketedPaste`
- Mouse tracking: `EnableMouseCapture`
- Terminal focus reporting: DECSET 1004
- Cleanup on exit
- FPS tracking

### 4.3: Theme System (`crates/tui/src/theme.rs`)
- Dark/Light/Auto themes matching TypeScript palette
- Auto-detection via `$COLORFGBG`
- Color helper: `color(key)` → `ratatui::style::Color`

### 4.4: Component Model (`crates/tui/src/components/`)
- `Box` widget — flexbox via `ratatui::layout::Layout` + `Constraint`
- `Text` widget — styled spans with color, bold, dim, wrap, truncate
- `Spacer` widget — flexible spacing
- `RawAnsi` widget — raw ANSI string rendering

### 4.5: Screen Layout (`crates/tui/src/screens/`)
- `ReplScreen` — main-screen mode layout
- `FullscreenScreen` — alt-screen mode with virtual scrolling
- `LogoHeader` — LogoV2 + StatusNotices
- Progressive width gating for small terminals

### 4.6: Message Rendering (`crates/tui/src/components/messages/`)
- `MessageRow` — `●` dot + content
- All message types (user, assistant, tool, system)
- Message normalization: grouping, collapsing

### 4.7: Tool Use Rendering (`crates/tui/src/components/tool_use/`)
- `ToolUseLoader` — blinking ●
- Per-tool displays (Bash, Read, Write, Edit, Grep, Glob)
- Status texts

### 4.8: Spinner System (`crates/tui/src/components/spinner/`)
- Braille glyph cycling at 120ms
- Stall detection (3s → gradual red)
- Shimmer effect
- Verb selection + tips
- BriefSpinner, BriefIdleStatus

### 4.9: Markdown Rendering (`crates/tui/src/components/markdown.rs`)
- `comrak` + `syntect`
- LRU token cache (500 entries)
- Fast path (no markdown syntax)
- `StreamingMarkdown` with stable prefix

### 4.10: PromptInput (`crates/tui/src/components/prompt_input/`)
- `TextInput` — multi-line with inverse cursor
- History navigation
- Slash command autocomplete
- Mode indicator
- Footer pills

### 4.11: Status Line (`crates/tui/src/components/status_line.rs`)
- Model, permission mode, directory, cost, etc.
- 300ms debounce

### 4.12: Permission Dialogs (`crates/tui/src/components/permissions/`)
- Bash, FileEdit, FileWrite, Filesystem, AskUserQuestion
- Allow once / Allow session / Deny
- y/n keybindings

### 4.13: Setup Screens (`crates/tui/src/screens/`)
- Onboarding, TrustDialog, Resume Picker, Login

### 4.14: Input Handling (`crates/tui/src/input.rs`)
- Full keybinding table
- Emacs-style editing (Ctrl+A/E/K/U/W/Y)
- Kill ring (max 10 entries)
- Chord support (1s timeout)
- Bracketed paste

### 4.15: Virtual Scrolling (`crates/tui/src/virtual_scroll.rs`)
- `VirtualMessageList`
- Height cache
- 200 cap without virtualization, 30 cap in transcript mode

### 4.16: Animations (`crates/tui/src/animations.rs`)
- Spinner glyph 120ms
- Shimmer 50/200ms
- Token counter smooth increment
- Stall 0.1 factor
- Reduced motion support
- FPS tracking

### 4.17: State Management (`crates/tui/src/state.rs`)
- `AppState` struct with `Arc<RwLock<AppState>>`
- Contexts: Theme, Keybinding, Modal

### 4.18: TUI Main Loop (`crates/tui/src/main_loop.rs`)
```
init → setup → render → poll(Key/Mouse/Resize/Paste/Tick) → handle → draw → cleanup
```

## Implementation Order

1. **Terminal + Theme** (4.2-4.3) — foundation
2. **Components + Layout** (4.4-4.5) — rendering primitives
3. **Messages + Tools** (4.6-4.7) — core content
4. **Spinner + Markdown** (4.8-4.9) — visual polish
5. **Input + PromptInput** (4.10, 4.14) — interaction
6. **Status + Permissions** (4.11-4.12) — dialogs
7. **Setup + Virtual Scroll** (4.13, 4.15) — advanced features
8. **Animations + State + Main Loop** (4.16-4.18) — wiring it all together
