use std::collections::VecDeque;

const MAX_HISTORY: usize = 100;

#[derive(Debug, Clone)]
pub struct InputHistory {
    entries: VecDeque<String>,
    current_index: Option<usize>,
    draft: Option<String>,
}

impl InputHistory {
    pub fn new() -> Self {
        Self {
            entries: VecDeque::with_capacity(MAX_HISTORY),
            current_index: None,
            draft: None,
        }
    }

    pub fn add(&mut self, text: String) {
        if text.trim().is_empty() {
            return;
        }

        if self.entries.back().map_or(false, |last| last == &text) {
            return;
        }

        self.entries.push_back(text);

        while self.entries.len() > MAX_HISTORY {
            self.entries.pop_front();
        }

        self.current_index = None;
        self.draft = None;
    }

    pub fn go_up(&mut self, current_text: &str) -> Option<String> {
        if self.entries.is_empty() {
            return None;
        }

        if self.current_index.is_none() {
            self.draft = Some(current_text.to_string());
            self.current_index = Some(self.entries.len() - 1);
        } else if let Some(idx) = self.current_index {
            if idx > 0 {
                self.current_index = Some(idx - 1);
            }
        }

        self.current_index
            .and_then(|idx| self.entries.get(idx))
            .cloned()
    }

    pub fn go_down(&mut self) -> Option<String> {
        if self.entries.is_empty() {
            return None;
        }

        match self.current_index {
            None => None,
            Some(idx) => {
                if idx + 1 >= self.entries.len() {
                    self.current_index = None;
                    self.draft.take()
                } else {
                    self.current_index = Some(idx + 1);
                    self.current_index
                        .and_then(|i| self.entries.get(i))
                        .cloned()
                }
            }
        }
    }

    pub fn cancel_navigation(&mut self) {
        self.current_index = None;
        self.draft = None;
    }

    pub fn is_navigating(&self) -> bool {
        self.current_index.is_some()
    }

    pub fn entries(&self) -> &VecDeque<String> {
        &self.entries
    }

    pub fn len(&self) -> usize {
        self.entries.len()
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }
}

impl Default for InputHistory {
    fn default() -> Self {
        Self::new()
    }
}
