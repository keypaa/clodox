use std::io::{self, stdout, Stdout};
use std::time::Instant;

use crossterm::{
    cursor,
    event::{
        self, DisableBracketedPaste, DisableFocusChange, DisableMouseCapture, EnableBracketedPaste,
        EnableFocusChange, EnableMouseCapture, Event, KeyboardEnhancementFlags,
        PushKeyboardEnhancementFlags,
    },
    terminal::{
        disable_raw_mode, enable_raw_mode, EnterAlternateScreen,
        LeaveAlternateScreen,
    },
    ExecutableCommand,
};
use ratatui::{backend::CrosstermBackend, Terminal};

/// Terminal mode: main-screen (default) or alt-screen (fullscreen).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TerminalMode {
    /// Main-screen mode — messages render to native terminal scrollback.
    /// This is the TypeScript default.
    MainScreen,
    /// Alt-screen mode — full TUI with virtual scrolling.
    /// Enabled via `CLAUDE_CODE_FULLSCREEN=1`.
    AltScreen,
}

impl TerminalMode {
    pub fn from_env() -> Self {
        if std::env::var("CLAUDE_CODE_FULLSCREEN")
            .ok()
            .as_deref()
            == Some("1")
        {
            Self::AltScreen
        } else {
            Self::MainScreen
        }
    }
}

/// FPS tracking for frame timing.
#[derive(Debug, Clone)]
pub struct FpsTracker {
    pub frame_count: u64,
    pub last_fps_update: Instant,
    pub current_fps: f64,
    pub phase_times: FramePhaseTimes,
}

#[derive(Debug, Clone, Default)]
pub struct FramePhaseTimes {
    pub event_poll_ms: f64,
    pub event_handle_ms: f64,
    pub render_ms: f64,
    pub total_ms: f64,
}

impl FpsTracker {
    pub fn new() -> Self {
        Self {
            frame_count: 0,
            last_fps_update: Instant::now(),
            current_fps: 0.0,
            phase_times: FramePhaseTimes::default(),
        }
    }

    /// Update FPS counter.
    pub fn tick(&mut self) {
        self.frame_count += 1;
        let elapsed = self.last_fps_update.elapsed();
        if elapsed.as_secs_f64() >= 1.0 {
            self.current_fps = self.frame_count as f64 / elapsed.as_secs_f64();
            self.frame_count = 0;
            self.last_fps_update = Instant::now();
        }
    }
}

impl Default for FpsTracker {
    fn default() -> Self {
        Self::new()
    }
}

/// Terminal manager — handles initialization, rendering, and cleanup.
pub struct TerminalManager {
    pub terminal: Terminal<CrosstermBackend<Stdout>>,
    pub mode: TerminalMode,
    pub fps: FpsTracker,
    pub is_initialized: bool,
}

impl TerminalManager {
    /// Create a new terminal manager.
    pub fn new() -> io::Result<Self> {
        let mode = TerminalMode::from_env();
        let backend = CrosstermBackend::new(stdout());
        let terminal = Terminal::new(backend)?;

        Ok(Self {
            terminal,
            mode,
            fps: FpsTracker::new(),
            is_initialized: false,
        })
    }

    /// Initialize the terminal (raw mode, keyboard enhancements, etc.).
    pub fn initialize(&mut self) -> io::Result<()> {
        if self.is_initialized {
            return Ok(());
        }

        // Enter raw mode
        enable_raw_mode()?;

        // Enter alternate screen if in fullscreen mode
        if self.mode == TerminalMode::AltScreen {
            stdout().execute(EnterAlternateScreen)?;
        }

        // Enable keyboard enhancements (Kitty Keyboard Protocol)
        stdout().execute(PushKeyboardEnhancementFlags(
            KeyboardEnhancementFlags::DISAMBIGUATE_ESCAPE_CODES
                | KeyboardEnhancementFlags::REPORT_ALL_KEYS_AS_ESCAPE_CODES
                | KeyboardEnhancementFlags::REPORT_ALTERNATE_KEYS
                | KeyboardEnhancementFlags::REPORT_EVENT_TYPES,
        ))?;

        // Enable bracketed paste
        stdout().execute(EnableBracketedPaste)?;

        // Enable mouse tracking
        stdout().execute(EnableMouseCapture)?;

        // Enable terminal focus reporting (DECSET 1004)
        stdout().execute(EnableFocusChange)?;

        // Hide cursor during rendering (we manage it manually)
        stdout().execute(cursor::Hide)?;

        self.is_initialized = true;

        tracing::info!(
            "Terminal initialized: mode={:?}, size={:?}",
            self.mode,
            self.size()?
        );

        Ok(())
    }

    /// Clean up the terminal (restore to normal state).
    pub fn cleanup(&mut self) -> io::Result<()> {
        if !self.is_initialized {
            return Ok(());
        }

        // Show cursor
        stdout().execute(cursor::Show)?;

        // Disable focus reporting
        stdout().execute(DisableFocusChange)?;

        // Disable mouse tracking
        stdout().execute(DisableMouseCapture)?;

        // Disable bracketed paste
        stdout().execute(DisableBracketedPaste)?;

        // Leave alternate screen if in fullscreen mode
        if self.mode == TerminalMode::AltScreen {
            stdout().execute(LeaveAlternateScreen)?;
        }

        // Exit raw mode
        disable_raw_mode()?;

        self.is_initialized = false;

        tracing::info!("Terminal cleaned up");

        Ok(())
    }

    /// Get the terminal size (columns, rows).
    pub fn size(&self) -> io::Result<(u16, u16)> {
        let size = crossterm::terminal::size()?;
        Ok(size)
    }

    /// Check if the terminal is a TTY.
    pub fn is_tty() -> bool {
        use std::io::IsTerminal;
        io::stdout().is_terminal()
    }

    /// Poll for an event with a timeout.
    pub fn poll_event(&self, timeout: std::time::Duration) -> io::Result<bool> {
        event::poll(timeout)
    }

    /// Read the next event.
    pub fn read_event(&self) -> io::Result<Event> {
        event::read()
    }

    /// Draw a frame to the terminal.
    pub fn draw<F>(&mut self, render_fn: F) -> io::Result<()>
    where
        F: FnOnce(&mut ratatui::Frame),
    {
        self.terminal.draw(render_fn)?;
        self.fps.tick();
        Ok(())
    }

    /// Clear the screen.
    pub fn clear(&mut self) -> io::Result<()> {
        stdout().execute(crossterm::terminal::Clear(
            crossterm::terminal::ClearType::All,
        ))?;
        Ok(())
    }

    /// Force a redraw (equivalent to Ctrl+L).
    pub fn force_redraw(&mut self) -> io::Result<()> {
        self.clear()?;
        Ok(())
    }
}

impl Drop for TerminalManager {
    fn drop(&mut self) {
        let _ = self.cleanup();
    }
}
