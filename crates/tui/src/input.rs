use std::collections::VecDeque;
use std::time::Instant;

use crossterm::event::{
    Event, KeyCode, KeyEvent, KeyEventKind, KeyEventState, KeyModifiers, KeyboardEnhancementFlags,
    MediaKeyCode, ModifierKeyCode, MouseButton, MouseEvent, MouseEventKind,
};

/// Key event wrapper with all modifiers.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InputKey {
    pub code: KeyCode,
    pub modifiers: KeyModifiers,
    pub kind: KeyEventKind,
    pub state: KeyEventState,
}

impl InputKey {
    pub fn from_event(event: KeyEvent) -> Self {
        Self {
            code: event.code,
            modifiers: event.modifiers,
            kind: event.kind,
            state: event.state,
        }
    }

    /// Check if Ctrl is held.
    pub fn ctrl(&self) -> bool {
        self.modifiers.contains(KeyModifiers::CONTROL)
    }

    /// Check if Shift is held.
    pub fn shift(&self) -> bool {
        self.modifiers.contains(KeyModifiers::SHIFT)
    }

    /// Check if Alt/Meta is held.
    pub fn alt(&self) -> bool {
        self.modifiers.contains(KeyModifiers::ALT)
    }

    /// Check if this is a press event (not release).
    pub fn is_press(&self) -> bool {
        self.kind == KeyEventKind::Press
    }

    /// Check if this matches a specific key combination.
    pub fn matches(&self, code: KeyCode, modifiers: KeyModifiers) -> bool {
        self.code == code && self.modifiers == modifiers
    }

    /// Check for Ctrl+C.
    pub fn is_ctrl_c(&self) -> bool {
        self.ctrl() && self.code == KeyCode::Char('c')
    }

    /// Check for Ctrl+D.
    pub fn is_ctrl_d(&self) -> bool {
        self.ctrl() && self.code == KeyCode::Char('d')
    }

    /// Check for Ctrl+L.
    pub fn is_ctrl_l(&self) -> bool {
        self.ctrl() && self.code == KeyCode::Char('l')
    }

    /// Check for Ctrl+O.
    pub fn is_ctrl_o(&self) -> bool {
        self.ctrl() && self.code == KeyCode::Char('o')
    }

    /// Check for Ctrl+R.
    pub fn is_ctrl_r(&self) -> bool {
        self.ctrl() && self.code == KeyCode::Char('r')
    }

    /// Check for Ctrl+A.
    pub fn is_ctrl_a(&self) -> bool {
        self.ctrl() && self.code == KeyCode::Char('a')
    }

    /// Check for Ctrl+E.
    pub fn is_ctrl_e(&self) -> bool {
        self.ctrl() && self.code == KeyCode::Char('e')
    }

    /// Check for Ctrl+K.
    pub fn is_ctrl_k(&self) -> bool {
        self.ctrl() && self.code == KeyCode::Char('k')
    }

    /// Check for Ctrl+U.
    pub fn is_ctrl_u(&self) -> bool {
        self.ctrl() && self.code == KeyCode::Char('u')
    }

    /// Check for Ctrl+W.
    pub fn is_ctrl_w(&self) -> bool {
        self.ctrl() && self.code == KeyCode::Char('w')
    }

    /// Check for Ctrl+Y.
    pub fn is_ctrl_y(&self) -> bool {
        self.ctrl() && self.code == KeyCode::Char('y')
    }

    /// Check for Escape.
    pub fn is_escape(&self) -> bool {
        self.code == KeyCode::Esc
    }

    /// Check for Enter.
    pub fn is_enter(&self) -> bool {
        self.code == KeyCode::Enter
    }

    /// Check for Tab.
    pub fn is_tab(&self) -> bool {
        self.code == KeyCode::Tab
    }

    /// Check for Backspace.
    pub fn is_backspace(&self) -> bool {
        self.code == KeyCode::Backspace
    }

    /// Check for Delete.
    pub fn is_delete(&self) -> bool {
        self.code == KeyCode::Delete
    }

    /// Check for Home.
    pub fn is_home(&self) -> bool {
        self.code == KeyCode::Home
    }

    /// Check for End.
    pub fn is_end(&self) -> bool {
        self.code == KeyCode::End
    }

    /// Check for Page Up.
    pub fn is_page_up(&self) -> bool {
        self.code == KeyCode::PageUp
    }

    /// Check for Page Down.
    pub fn is_page_down(&self) -> bool {
        self.code == KeyCode::PageDown
    }

    /// Check for Up arrow.
    pub fn is_up(&self) -> bool {
        self.code == KeyCode::Up
    }

    /// Check for Down arrow.
    pub fn is_down(&self) -> bool {
        self.code == KeyCode::Down
    }

    /// Check for Left arrow.
    pub fn is_left(&self) -> bool {
        self.code == KeyCode::Left
    }

    /// Check for Right arrow.
    pub fn is_right(&self) -> bool {
        self.code == KeyCode::Right
    }
}

/// Input action from keybinding resolution.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum InputAction {
    /// Insert a character.
    InsertChar(char),
    /// Insert a newline.
    InsertNewline,
    /// Delete character before cursor.
    Backspace,
    /// Delete word before cursor.
    KillWordBefore,
    /// Delete character after cursor.
    Delete,
    /// Delete to end of line.
    KillToLineEnd,
    /// Delete to start of line.
    KillToLineStart,
    /// Move cursor left.
    MoveLeft,
    /// Move cursor right.
    MoveRight,
    /// Move cursor to previous word.
    MovePrevWord,
    /// Move cursor to next word.
    MoveNextWord,
    /// Move cursor to start of line.
    MoveStartOfLine,
    /// Move cursor to end of line.
    MoveEndOfLine,
    /// Move cursor up (or history up).
    MoveUp,
    /// Move cursor down (or history down).
    MoveDown,
    /// Yank from kill ring.
    Yank,
    /// Cycle through kill ring.
    YankPop,
    /// Abort in-flight query.
    AbortQuery,
    /// Submit the input.
    Submit,
    /// Cancel/clear input.
    Cancel,
    /// Exit the application.
    Exit,
    /// Redraw the screen.
    Redraw,
    /// Toggle transcript mode.
    ToggleTranscript,
    /// Toggle show all in transcript.
    ToggleShowAll,
    /// Open history search.
    HistorySearch,
    /// Accept autocomplete.
    AcceptAutocomplete,
    /// Dismiss autocomplete.
    DismissAutocomplete,
    /// Previous autocomplete.
    PreviousAutocomplete,
    /// Next autocomplete.
    NextAutocomplete,
    /// Confirm (y/n dialogs).
    Confirm(bool),
    /// Suspend (Ctrl+Z).
    Suspend,
    /// Unknown/unbound.
    Unknown,
}

/// Kill ring for Emacs-style editing.
#[derive(Debug, Clone)]
pub struct KillRing {
    entries: VecDeque<String>,
    max_entries: usize,
}

impl KillRing {
    pub fn new() -> Self {
        Self {
            entries: VecDeque::with_capacity(10),
            max_entries: 10,
        }
    }

    /// Push a killed string to the ring.
    pub fn push(&mut self, text: String) {
        // If consecutive kills, accumulate with the last entry
        if let Some(last) = self.entries.back_mut() {
            // Prepend new text (kill before cursor)
            let mut combined = text;
            combined.push_str(last);
            *last = combined;
        } else {
            self.entries.push_back(text);
        }

        // Cap the ring size
        while self.entries.len() > self.max_entries {
            self.entries.pop_front();
        }
    }

    /// Get the most recent entry.
    pub fn current(&self) -> Option<&str> {
        self.entries.back().map(|s| s.as_str())
    }

    /// Rotate backwards (for Meta+Y).
    pub fn rotate_back(&mut self) {
        if self.entries.len() > 1 {
            if let Some(entry) = self.entries.pop_back() {
                self.entries.push_front(entry);
            }
        }
    }

    /// Clear the ring.
    pub fn clear(&mut self) {
        self.entries.clear();
    }
}

impl Default for KillRing {
    fn default() -> Self {
        Self::new()
    }
}

/// Chord state for multi-key sequences.
#[derive(Debug, Clone)]
pub struct ChordState {
    pub keys: Vec<InputKey>,
    pub started_at: Instant,
    pub timeout_ms: u64,
}

impl ChordState {
    pub const TIMEOUT_MS: u64 = 1000;

    pub fn new(key: InputKey) -> Self {
        Self {
            keys: vec![key],
            started_at: Instant::now(),
            timeout_ms: Self::TIMEOUT_MS,
        }
    }

    /// Add a key to the chord.
    pub fn add(&mut self, key: InputKey) {
        self.keys.push(key);
    }

    /// Check if the chord has timed out.
    pub fn is_expired(&self) -> bool {
        self.started_at.elapsed().as_millis() as u64 > self.timeout_ms
    }

    /// Reset the chord state.
    pub fn reset(&mut self) {
        self.keys.clear();
    }

    /// Check if a chord is active.
    pub fn is_active(&self) -> bool {
        !self.keys.is_empty() && !self.is_expired()
    }
}

impl Default for ChordState {
    fn default() -> Self {
        Self {
            keys: Vec::new(),
            started_at: Instant::now(),
            timeout_ms: Self::TIMEOUT_MS,
        }
    }
}

/// Input handler — processes events and resolves keybindings.
pub struct InputHandler {
    pub kill_ring: KillRing,
    pub chord_state: ChordState,
    pub exit_confirmation: Option<Instant>,
    pub is_query_active: bool,
}

impl InputHandler {
    pub fn new() -> Self {
        Self {
            kill_ring: KillRing::new(),
            chord_state: ChordState::default(),
            exit_confirmation: None,
            is_query_active: false,
        }
    }

    /// Process a crossterm event into an input action.
    pub fn process_event(&mut self, event: Event) -> Option<InputAction> {
        match event {
            Event::Key(key_event) => self.process_key(key_event),
            Event::Mouse(mouse_event) => self.process_mouseouse(mouse_event),
            Event::Paste(text) => Some(InputAction::InsertChar('\n')), // Placeholder
            Event::FocusGained | Event::FocusLost => None,
            Event::Resize(_, _) => None,
        }
    }

    /// Process a key event.
    pub fn process_key(&mut self, key_event: KeyEvent) -> Option<InputAction> {
        let key = InputKey::from_event(key_event);

        // Only process press events
        if !key.is_press() {
            return None;
        }

        // Handle Ctrl+C with double-press to exit
        if key.is_ctrl_c() {
            return self.handle_ctrl_c(self.is_query_active);
        }

        // Handle Ctrl+D with double-press to exit (if input empty)
        if key.is_ctrl_d() {
            return self.handle_ctrl_d();
        }

        // Global keybindings
        if key.is_ctrl_l() {
            return Some(InputAction::Redraw);
        }
        if key.is_ctrl_o() {
            return Some(InputAction::ToggleTranscript);
        }
        if key.is_ctrl_r() {
            return Some(InputAction::HistorySearch);
        }

        // Emacs-style editing
        if key.is_ctrl_a() {
            return Some(InputAction::MoveStartOfLine);
        }
        if key.is_ctrl_e() {
            return Some(InputAction::MoveEndOfLine);
        }
        if key.is_ctrl_k() {
            return Some(InputAction::KillToLineEnd);
        }
        if key.is_ctrl_u() {
            return Some(InputAction::KillToLineStart);
        }
        if key.is_ctrl_w() {
            return Some(InputAction::KillWordBefore);
        }
        if key.is_ctrl_y() {
            return Some(InputAction::Yank);
        }

        // Meta+Y for kill ring cycling
        if key.alt() && key.code == KeyCode::Char('y') {
            return Some(InputAction::YankPop);
        }

        // Meta+B/F for word movement
        if key.alt() && key.code == KeyCode::Char('b') {
            return Some(InputAction::MovePrevWord);
        }
        if key.alt() && key.code == KeyCode::Char('f') {
            return Some(InputAction::MoveNextWord);
        }

        // Arrow keys
        if key.is_up() {
            return Some(InputAction::MoveUp);
        }
        if key.is_down() {
            return Some(InputAction::MoveDown);
        }
        if key.is_left() {
            if key.ctrl() || key.alt() {
                return Some(InputAction::MovePrevWord);
            }
            return Some(InputAction::MoveLeft);
        }
        if key.is_right() {
            if key.ctrl() || key.alt() {
                return Some(InputAction::MoveNextWord);
            }
            return Some(InputAction::MoveRight);
        }

        // Home/End
        if key.is_home() {
            return Some(InputAction::MoveStartOfLine);
        }
        if key.is_end() {
            return Some(InputAction::MoveEndOfLine);
        }

        // PageUp/PageDown
        if key.is_page_up() {
            return Some(InputAction::MoveStartOfLine);
        }
        if key.is_page_down() {
            return Some(InputAction::MoveEndOfLine);
        }

        // Backspace/Delete
        if key.is_backspace() {
            if key.ctrl() || key.alt() {
                return Some(InputAction::KillWordBefore);
            }
            return Some(InputAction::Backspace);
        }
        if key.is_delete() {
            if key.alt() {
                return Some(InputAction::KillToLineEnd);
            }
            return Some(InputAction::Delete);
        }

        // Tab
        if key.is_tab() {
            return Some(InputAction::AcceptAutocomplete);
        }

        // Escape
        if key.is_escape() {
            return Some(InputAction::Cancel);
        }

        // Enter
        if key.is_enter() {
            // Shift+Enter or Meta+Enter inserts newline
            if key.shift() || key.alt() {
                return Some(InputAction::InsertNewline);
            }
            return Some(InputAction::Submit);
        }

        // Character input
        if let KeyCode::Char(c) = key.code {
            return Some(InputAction::InsertChar(c));
        }

        Some(InputAction::Unknown)
    }

    /// Handle Ctrl+C (double-press to exit, or abort query if active).
    fn handle_ctrl_c(&mut self, is_query_active: bool) -> Option<InputAction> {
        if is_query_active {
            return Some(InputAction::AbortQuery);
        }
        if let Some(first_press) = self.exit_confirmation {
            if first_press.elapsed().as_millis() < 2000 {
                // Double-press within 2 seconds → exit
                self.exit_confirmation = None;
                return Some(InputAction::Exit);
            }
        }
        // First press → set confirmation
        self.exit_confirmation = Some(Instant::now());
        None
    }

    /// Handle Ctrl+D (double-press to exit if input empty).
    fn handle_ctrl_d(&mut self) -> Option<InputAction> {
        // In the full implementation, this checks if input is empty
        // For now, treat similar to Ctrl+C
        if let Some(first_press) = self.exit_confirmation {
            if first_press.elapsed().as_millis() < 2000 {
                self.exit_confirmation = None;
                return Some(InputAction::Exit);
            }
        }
        self.exit_confirmation = Some(Instant::now());
        None
    }

    /// Process a mouse event.
    pub fn process_mouseouse(&self, _event: MouseEvent) -> Option<InputAction> {
        // Mouse handling will be implemented later
        None
    }
}

impl Default for InputHandler {
    fn default() -> Self {
        Self::new()
    }
}
