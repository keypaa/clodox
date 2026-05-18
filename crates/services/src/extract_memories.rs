use std::path::{Path, PathBuf};

use cc_core::messages::Message;
use serde::{Deserialize, Serialize};
use tracing::{debug, info, warn};

/// Extracted memory entry.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MemoryEntry {
    pub content: String,
    pub category: MemoryCategory,
    pub source: String,
    pub confidence: f64,
}

/// Memory category for extracted content.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum MemoryCategory {
    /// Project structure and architecture.
    ProjectStructure,
    /// Build and test commands.
    BuildCommands,
    /// Coding conventions and style.
    Conventions,
    /// Dependencies and setup.
    Dependencies,
    /// Deployment and infrastructure.
    Deployment,
    /// General project information.
    General,
}

impl std::fmt::Display for MemoryCategory {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            MemoryCategory::ProjectStructure => write!(f, "Project Structure"),
            MemoryCategory::BuildCommands => write!(f, "Build Commands"),
            MemoryCategory::Conventions => write!(f, "Conventions"),
            MemoryCategory::Dependencies => write!(f, "Dependencies"),
            MemoryCategory::Deployment => write!(f, "Deployment"),
            MemoryCategory::General => write!(f, "General"),
        }
    }
}

/// Extract memories service — CLAUDE.md content extraction from conversations and project files.
pub struct ExtractMemoriesService {
    /// Project root directory.
    project_root: PathBuf,
}

impl ExtractMemoriesService {
    pub fn new(project_root: PathBuf) -> Self {
        Self { project_root }
    }

    /// Extract memories from the project's existing CLAUDE.md files.
    pub fn extract_from_project(&self) -> Vec<MemoryEntry> {
        let mut memories = Vec::new();

        // Check for CLAUDE.md in project root
        let paths = [
            self.project_root.join("CLAUDE.md"),
            self.project_root.join(".claude").join("CLAUDE.md"),
            self.project_root.join("CLAUDE.local.md"),
        ];

        for path in &paths {
            if path.exists() {
                match std::fs::read_to_string(path) {
                    Ok(content) => {
                        let entries = self.parse_claude_md(&content, path);
                        memories.extend(entries);
                    }
                    Err(e) => {
                        warn!(path = %path.display(), error = %e, "Failed to read CLAUDE.md");
                    }
                }
            }
        }

        // Also extract from common project files
        memories.extend(self.extract_from_readme());
        memories.extend(self.extract_from_package_json());
        memories.extend(self.extract_from_cargo_toml());

        info!(count = memories.len(), "Memories extracted from project");
        memories
    }

    /// Extract memories from conversation messages.
    pub fn extract_from_conversation(&self, messages: &[Message]) -> Vec<MemoryEntry> {
        let mut memories = Vec::new();

        // Look for patterns that indicate project knowledge
        for msg in messages {
            match msg {
                Message::User(u) => {
                    for block in &u.content {
                        if let cc_core::messages::ContentBlockParam::Text { text } = block {
                            memories.extend(self.extract_from_text(text, "user message"));
                        }
                    }
                }
                Message::Assistant(a) => {
                    for block in &a.content {
                        if let cc_core::messages::ContentBlockParam::Text { text } = block {
                            memories.extend(self.extract_from_text(text, "assistant response"));
                        }
                    }
                }
                _ => {}
            }
        }

        // Deduplicate by content similarity
        memories.dedup_by(|a, b| a.content == b.content);

        info!(count = memories.len(), "Memories extracted from conversation");
        memories
    }

    /// Parse CLAUDE.md content into memory entries.
    fn parse_claude_md(&self, content: &str, source: &Path) -> Vec<MemoryEntry> {
        let mut entries = Vec::new();
        let source_str = source.display().to_string();

        // Split by markdown headings
        let mut current_category = MemoryCategory::General;
        let mut current_content = String::new();

        for line in content.lines() {
            if line.starts_with("# ") {
                // Save previous section
                if !current_content.is_empty() {
                    entries.push(MemoryEntry {
                        content: current_content.trim().to_string(),
                        category: current_category,
                        source: source_str.clone(),
                        confidence: 1.0, // CLAUDE.md is authoritative
                    });
                    current_content.clear();
                }

                // Determine category from heading
                current_category = self.categorize_heading(line);
            } else {
                current_content.push_str(line);
                current_content.push('\n');
            }
        }

        // Save last section
        if !current_content.is_empty() {
            entries.push(MemoryEntry {
                content: current_content.trim().to_string(),
                category: current_category,
                source: source_str,
                confidence: 1.0,
            });
        }

        entries
    }

    /// Categorize a markdown heading.
    fn categorize_heading(&self, heading: &str) -> MemoryCategory {
        let lower = heading.to_lowercase();
        if lower.contains("build") || lower.contains("test") || lower.contains("run") {
            MemoryCategory::BuildCommands
        } else if lower.contains("convention") || lower.contains("style") || lower.contains("pattern") {
            MemoryCategory::Conventions
        } else if lower.contains("depend") || lower.contains("setup") || lower.contains("install") {
            MemoryCategory::Dependencies
        } else if lower.contains("deploy") || lower.contains("infra") || lower.contains("server") {
            MemoryCategory::Deployment
        } else if lower.contains("structure") || lower.contains("architect") || lower.contains("layout") {
            MemoryCategory::ProjectStructure
        } else {
            MemoryCategory::General
        }
    }

    /// Extract memories from README.md.
    fn extract_from_readme(&self) -> Vec<MemoryEntry> {
        let readme_path = self.project_root.join("README.md");
        if !readme_path.exists() {
            return Vec::new();
        }

        let content = match std::fs::read_to_string(&readme_path) {
            Ok(c) => c,
            Err(_) => return Vec::new(),
        };

        let mut memories = Vec::new();

        // Extract project description (first paragraph after title)
        let lines: Vec<&str> = content.lines().collect();
        let mut in_description = false;
        let mut description = String::new();

        for line in &lines {
            if line.starts_with("# ") {
                in_description = true;
                continue;
            }
            if in_description {
                if line.is_empty() && !description.is_empty() {
                    break;
                }
                if !line.starts_with('#') {
                    description.push_str(line);
                    description.push(' ');
                }
            }
        }

        if !description.is_empty() {
            memories.push(MemoryEntry {
                content: description.trim().to_string(),
                category: MemoryCategory::General,
                source: "README.md".to_string(),
                confidence: 0.8,
            });
        }

        // Extract build/test commands from code blocks
        for line in &lines {
            if line.starts_with("```") && line.contains("bash") || line.starts_with("```") && line.contains("sh") {
                // Would need to parse the code block content
            }
        }

        memories
    }

    /// Extract memories from package.json.
    fn extract_from_package_json(&self) -> Vec<MemoryEntry> {
        let pkg_path = self.project_root.join("package.json");
        if !pkg_path.exists() {
            return Vec::new();
        }

        let content = match std::fs::read_to_string(&pkg_path) {
            Ok(c) => c,
            Err(_) => return Vec::new(),
        };

        let mut memories = Vec::new();

        if let Ok(pkg) = serde_json::from_str::<serde_json::Value>(&content) {
            // Extract scripts
            if let Some(scripts) = pkg.get("scripts").and_then(|s| s.as_object()) {
                let mut script_text = String::from("Available npm scripts:\n");
                for (name, cmd) in scripts {
                    script_text.push_str(&format!("  - {name}: {cmd}\n"));
                }
                memories.push(MemoryEntry {
                    content: script_text,
                    category: MemoryCategory::BuildCommands,
                    source: "package.json".to_string(),
                    confidence: 0.9,
                });
            }

            // Extract dependencies
            if let Some(deps) = pkg.get("dependencies").and_then(|d| d.as_object()) {
                let dep_list: Vec<_> = deps.keys().cloned().collect();
                memories.push(MemoryEntry {
                    content: format!("Dependencies: {}", dep_list.join(", ")),
                    category: MemoryCategory::Dependencies,
                    source: "package.json".to_string(),
                    confidence: 0.7,
                });
            }
        }

        memories
    }

    /// Extract memories from Cargo.toml.
    fn extract_from_cargo_toml(&self) -> Vec<MemoryEntry> {
        let cargo_path = self.project_root.join("Cargo.toml");
        if !cargo_path.exists() {
            return Vec::new();
        }

        let content = match std::fs::read_to_string(&cargo_path) {
            Ok(c) => c,
            Err(_) => return Vec::new(),
        };

        let mut memories = Vec::new();

        // Extract package info
        if let Some(package_section) = content.split("[package]").nth(1) {
            let lines: Vec<&str> = package_section
                .lines()
                .take_while(|l| !l.starts_with('['))
                .collect();

            for line in lines {
                if let Some(eq_pos) = line.find('=') {
                    let key = line[..eq_pos].trim();
                    let value = line[eq_pos + 1..].trim().trim_matches('"');

                    if key == "name" || key == "description" || key == "version" {
                        memories.push(MemoryEntry {
                            content: format!("{key}: {value}"),
                            category: MemoryCategory::General,
                            source: "Cargo.toml".to_string(),
                            confidence: 0.9,
                        });
                    }
                }
            }
        }

        // Extract dependencies
        if let Some(deps_section) = content.split("[dependencies]").nth(1) {
            let lines: Vec<&str> = deps_section
                .lines()
                .take_while(|l| !l.starts_with('['))
                .filter(|l| l.contains('='))
                .collect();

            let dep_names: Vec<_> = lines
                .iter()
                .filter_map(|l| l.find('=').map(|i| l[..i].trim().to_string()))
                .collect();

            if !dep_names.is_empty() {
                memories.push(MemoryEntry {
                    content: format!("Rust dependencies: {}", dep_names.join(", ")),
                    category: MemoryCategory::Dependencies,
                    source: "Cargo.toml".to_string(),
                    confidence: 0.7,
                });
            }
        }

        memories
    }

    /// Extract memories from arbitrary text.
    fn extract_from_text(&self, text: &str, source: &str) -> Vec<MemoryEntry> {
        let mut memories = Vec::new();

        // Look for explicit memory statements
        for line in text.lines() {
            let lower = line.trim().to_lowercase();

            if lower.starts_with("remember:") || lower.starts_with("note:") || lower.starts_with("important:") {
                memories.push(MemoryEntry {
                    content: line.trim().to_string(),
                    category: MemoryCategory::General,
                    source: source.to_string(),
                    confidence: 0.6,
                });
            }

            // Look for build/test commands
            if lower.starts_with("to build:") || lower.starts_with("to run:") || lower.starts_with("to test:") {
                memories.push(MemoryEntry {
                    content: line.trim().to_string(),
                    category: MemoryCategory::BuildCommands,
                    source: source.to_string(),
                    confidence: 0.5,
                });
            }
        }

        memories
    }

    /// Generate a CLAUDE.md file from extracted memories.
    pub fn generate_claude_md(&self, memories: &[MemoryEntry]) -> String {
        let mut output = String::from("# Project Context\n\n");

        // Group by category
        let mut grouped: std::collections::HashMap<MemoryCategory, Vec<&MemoryEntry>> =
            std::collections::HashMap::new();
        for entry in memories {
            grouped.entry(entry.category).or_default().push(entry);
        }

        // Output each category
        let categories = [
            MemoryCategory::General,
            MemoryCategory::ProjectStructure,
            MemoryCategory::BuildCommands,
            MemoryCategory::Conventions,
            MemoryCategory::Dependencies,
            MemoryCategory::Deployment,
        ];

        for category in &categories {
            if let Some(entries) = grouped.get(category) {
                output.push_str(&format!("## {}\n\n", category));
                for entry in entries {
                    output.push_str(&format!("{}\n\n", entry.content));
                }
            }
        }

        output
    }

    /// Save extracted memories to CLAUDE.md.
    pub fn save_to_claude_md(&self, memories: &[MemoryEntry]) -> Result<PathBuf, String> {
        let content = self.generate_claude_md(memories);
        let path = self.project_root.join("CLAUDE.md");

        std::fs::write(&path, &content)
            .map_err(|e| format!("Failed to write CLAUDE.md: {}", e))?;

        info!(path = %path.display(), count = memories.len(), "Memories saved to CLAUDE.md");
        Ok(path)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_memory_category_display() {
        assert_eq!(MemoryCategory::ProjectStructure.to_string(), "Project Structure");
        assert_eq!(MemoryCategory::BuildCommands.to_string(), "Build Commands");
        assert_eq!(MemoryCategory::Conventions.to_string(), "Conventions");
        assert_eq!(MemoryCategory::Dependencies.to_string(), "Dependencies");
        assert_eq!(MemoryCategory::Deployment.to_string(), "Deployment");
        assert_eq!(MemoryCategory::General.to_string(), "General");
    }

    #[test]
    fn test_categorize_heading_build() {
        let service = ExtractMemoriesService::new(PathBuf::from("/tmp"));
        assert_eq!(
            service.categorize_heading("# Build and Test Commands"),
            MemoryCategory::BuildCommands
        );
    }

    #[test]
    fn test_categorize_heading_conventions() {
        let service = ExtractMemoriesService::new(PathBuf::from("/tmp"));
        assert_eq!(
            service.categorize_heading("# Coding Conventions"),
            MemoryCategory::Conventions
        );
    }

    #[test]
    fn test_categorize_heading_dependencies() {
        let service = ExtractMemoriesService::new(PathBuf::from("/tmp"));
        assert_eq!(
            service.categorize_heading("# Dependencies"),
            MemoryCategory::Dependencies
        );
    }

    #[test]
    fn test_categorize_heading_deployment() {
        let service = ExtractMemoriesService::new(PathBuf::from("/tmp"));
        assert_eq!(
            service.categorize_heading("# Deployment"),
            MemoryCategory::Deployment
        );
    }

    #[test]
    fn test_categorize_heading_structure() {
        let service = ExtractMemoriesService::new(PathBuf::from("/tmp"));
        assert_eq!(
            service.categorize_heading("# Project Structure"),
            MemoryCategory::ProjectStructure
        );
    }

    #[test]
    fn test_categorize_heading_general() {
        let service = ExtractMemoriesService::new(PathBuf::from("/tmp"));
        assert_eq!(
            service.categorize_heading("# Introduction"),
            MemoryCategory::General
        );
    }

    #[test]
    fn test_extract_from_text_remember() {
        let service = ExtractMemoriesService::new(PathBuf::from("/tmp"));
        let memories = service.extract_from_text("Remember: always use cargo test", "user message");
        assert_eq!(memories.len(), 1);
        assert_eq!(memories[0].category, MemoryCategory::General);
        assert!(memories[0].content.contains("Remember:"));
    }

    #[test]
    fn test_extract_from_text_build_command() {
        let service = ExtractMemoriesService::new(PathBuf::from("/tmp"));
        let memories = service.extract_from_text("To build: run cargo build --release", "user message");
        assert_eq!(memories.len(), 1);
        assert_eq!(memories[0].category, MemoryCategory::BuildCommands);
    }

    #[test]
    fn test_generate_claude_md() {
        let service = ExtractMemoriesService::new(PathBuf::from("/tmp"));
        let memories = vec![
            MemoryEntry {
                content: "This is a test project".to_string(),
                category: MemoryCategory::General,
                source: "test".to_string(),
                confidence: 0.8,
            },
            MemoryEntry {
                content: "Use cargo test".to_string(),
                category: MemoryCategory::BuildCommands,
                source: "test".to_string(),
                confidence: 0.9,
            },
        ];
        let output = service.generate_claude_md(&memories);
        assert!(output.contains("# Project Context"));
        assert!(output.contains("## General"));
        assert!(output.contains("## Build Commands"));
        assert!(output.contains("This is a test project"));
        assert!(output.contains("Use cargo test"));
    }

    #[test]
    fn test_extract_from_project_no_files() {
        let service = ExtractMemoriesService::new(PathBuf::from("/tmp/nonexistent"));
        let memories = service.extract_from_project();
        assert!(memories.is_empty());
    }
}
