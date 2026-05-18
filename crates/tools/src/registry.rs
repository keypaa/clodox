use std::collections::HashMap;
use std::sync::Arc;

use cc_core::tools::Tool;

use crate::bash::BashTool;
use crate::file_edit::FileEditTool;
use crate::file_read::FileReadTool;
use crate::file_write::FileWriteTool;
use crate::glob::GlobTool;
use crate::grep::GrepTool;

/// Tool registry - manages all available tools.
#[derive(Debug)]
pub struct ToolRegistry {
    tools: HashMap<String, Arc<dyn Tool>>,
}

impl ToolRegistry {
    pub fn new() -> Self {
        Self {
            tools: HashMap::new(),
        }
    }

    /// Register a tool.
    pub fn register(&mut self, tool: Arc<dyn Tool>) {
        let name = tool.name().to_string();
        self.tools.insert(name, tool);
    }

    /// Get a tool by name.
    pub fn get(&self, name: &str) -> Option<&Arc<dyn Tool>> {
        self.tools.get(name)
    }

    /// Get all tools.
    pub fn all(&self) -> Vec<Arc<dyn Tool>> {
        self.tools.values().cloned().collect()
    }

    /// Check if a tool exists.
    pub fn contains(&self, name: &str) -> bool {
        self.tools.contains_key(name)
    }

    /// Get tool count.
    pub fn len(&self) -> usize {
        self.tools.len()
    }

    /// Check if registry is empty.
    pub fn is_empty(&self) -> bool {
        self.tools.is_empty()
    }

    /// Build the default tool registry with all core tools.
    pub fn default_registry() -> Self {
        let mut registry = Self::new();

        // Create shared read state for Read/Write/Edit coordination
        let read_state = Arc::new(std::sync::Mutex::new(HashMap::new()));

        // Register core tools
        registry.register(BashTool::new());
        registry.register(FileReadTool::new());
        registry.register(FileWriteTool::new(read_state.clone()));
        registry.register(FileEditTool::new(read_state));
        registry.register(GrepTool::new());
        registry.register(GlobTool::new());

        registry
    }
}

impl Default for ToolRegistry {
    fn default() -> Self {
        Self::new()
    }
}
