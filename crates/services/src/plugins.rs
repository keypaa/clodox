use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;

use serde::{Deserialize, Serialize};
use tokio::sync::RwLock;
use tracing::{debug, info, warn};

/// Plugin manifest loaded from a plugin directory.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PluginManifest {
    pub name: String,
    pub version: String,
    pub description: Option<String>,
    pub author: Option<String>,
    pub main: Option<String>,
    pub tools: Vec<String>,
    pub commands: Vec<String>,
}

/// Plugin loading status.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum PluginStatus {
    Loading,
    Loaded,
    Error,
    Disabled,
}

/// Plugin error information.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PluginError {
    pub plugin_name: String,
    pub error: String,
    pub context: Option<String>,
}

/// Loaded plugin state.
#[derive(Debug, Clone)]
pub struct LoadedPlugin {
    pub manifest: PluginManifest,
    pub path: PathBuf,
    pub status: PluginStatus,
    pub error: Option<PluginError>,
    pub load_time_ms: u64,
}

/// Plugin service — discovery, loading, lifecycle management.
pub struct PluginService {
    plugins: RwLock<HashMap<String, LoadedPlugin>>,
    plugin_dirs: RwLock<Vec<PathBuf>>,
    errors: RwLock<Vec<PluginError>>,
}

impl PluginService {
    pub fn new() -> Self {
        Self {
            plugins: RwLock::new(HashMap::new()),
            plugin_dirs: RwLock::new(Vec::new()),
            errors: RwLock::new(Vec::new()),
        }
    }

    /// Register a plugin directory to scan.
    pub async fn add_plugin_dir(&self, dir: PathBuf) {
        self.plugin_dirs.write().await.push(dir);
    }

    /// Register a plugin directory and immediately scan it.
    pub async fn add_plugin_dir_and_scan(&self, dir: PathBuf) -> Result<usize, String> {
        self.add_plugin_dir(dir.clone()).await;
        self.scan_dir(&dir).await
    }

    /// Scan a directory for plugins (directories with manifest files).
    pub async fn scan_dir(&self, dir: &PathBuf) -> Result<usize, String> {
        if !dir.exists() {
            return Err(format!("Plugin directory not found: {}", dir.display()));
        }

        if !dir.is_dir() {
            return Err(format!("Not a directory: {}", dir.display()));
        }

        let mut loaded = 0;

        let entries = match std::fs::read_dir(dir) {
            Ok(e) => e,
            Err(e) => return Err(format!("Cannot read directory: {}", e)),
        };

        for entry in entries.filter_map(|e| e.ok()) {
            let path = entry.path();
            if !path.is_dir() {
                continue;
            }

            // Look for manifest.json or plugin.json
            let manifest_path = path.join("manifest.json");
            if !manifest_path.exists() {
                continue;
            }

            match self.load_plugin(&path).await {
                Ok(_) => loaded += 1,
                Err(e) => {
                    warn!(plugin = ?path, error = %e, "Failed to load plugin");
                }
            }
        }

        info!(dir = %dir.display(), loaded, "Plugin directory scanned");
        Ok(loaded)
    }

    /// Load a single plugin from its directory.
    pub async fn load_plugin(&self, dir: &PathBuf) -> Result<(), String> {
        let start = std::time::Instant::now();

        // Read manifest
        let manifest_path = dir.join("manifest.json");
        let manifest_content = std::fs::read_to_string(&manifest_path)
            .map_err(|e| format!("Cannot read manifest: {}", e))?;

        let manifest: PluginManifest = serde_json::from_str(&manifest_content)
            .map_err(|e| format!("Invalid manifest: {}", e))?;

        let name = manifest.name.clone();

        // Validate required fields
        if manifest.version.is_empty() {
            return Err(format!("Plugin {name}: version is required"));
        }

        let load_time_ms = start.elapsed().as_millis() as u64;

        let plugin = LoadedPlugin {
            manifest,
            path: dir.clone(),
            status: PluginStatus::Loaded,
            error: None,
            load_time_ms,
        };

        self.plugins.write().await.insert(name.clone(), plugin);

        info!(name, load_time_ms, "Plugin loaded");
        Ok(())
    }

    /// Unload a plugin.
    pub async fn unload_plugin(&self, name: &str) -> Result<(), String> {
        let mut plugins = self.plugins.write().await;
        if plugins.remove(name).is_some() {
            info!(name, "Plugin unloaded");
            Ok(())
        } else {
            Err(format!("Plugin not found: {name}"))
        }
    }

    /// Disable a plugin without unloading it.
    pub async fn disable_plugin(&self, name: &str) -> Result<(), String> {
        let mut plugins = self.plugins.write().await;
        let plugin = plugins
            .get_mut(name)
            .ok_or_else(|| format!("Plugin not found: {name}"))?;

        plugin.status = PluginStatus::Disabled;
        info!(name, "Plugin disabled");
        Ok(())
    }

    /// Enable a disabled plugin.
    pub async fn enable_plugin(&self, name: &str) -> Result<(), String> {
        let mut plugins = self.plugins.write().await;
        let plugin = plugins
            .get_mut(name)
            .ok_or_else(|| format!("Plugin not found: {name}"))?;

        plugin.status = PluginStatus::Loaded;
        plugin.error = None;
        info!(name, "Plugin enabled");
        Ok(())
    }

    /// Get a plugin by name.
    pub async fn get_plugin(&self, name: &str) -> Option<LoadedPlugin> {
        self.plugins.read().await.get(name).cloned()
    }

    /// Get all loaded plugins.
    pub async fn get_all_plugins(&self) -> Vec<LoadedPlugin> {
        self.plugins.read().await.values().cloned().collect()
    }

    /// Get all enabled plugins.
    pub async fn get_enabled_plugins(&self) -> Vec<LoadedPlugin> {
        self.plugins
            .read()
            .await
            .values()
            .filter(|p| p.status == PluginStatus::Loaded)
            .cloned()
            .collect()
    }

    /// Get all disabled plugins.
    pub async fn get_disabled_plugins(&self) -> Vec<LoadedPlugin> {
        self.plugins
            .read()
            .await
            .values()
            .filter(|p| p.status == PluginStatus::Disabled)
            .cloned()
            .collect()
    }

    /// Get all tool names from all enabled plugins.
    pub async fn get_all_tool_names(&self) -> Vec<String> {
        self.plugins
            .read()
            .await
            .values()
            .filter(|p| p.status == PluginStatus::Loaded)
            .flat_map(|p| p.manifest.tools.clone())
            .collect()
    }

    /// Get all command names from all enabled plugins.
    pub async fn get_all_command_names(&self) -> Vec<String> {
        self.plugins
            .read()
            .await
            .values()
            .filter(|p| p.status == PluginStatus::Loaded)
            .flat_map(|p| p.manifest.commands.clone())
            .collect()
    }

    /// Get all errors from plugin loading.
    pub async fn get_errors(&self) -> Vec<PluginError> {
        let mut errors = self.errors.read().await.clone();

        // Also collect errors from loaded plugins
        for plugin in self.plugins.read().await.values() {
            if let Some(ref err) = plugin.error {
                errors.push(err.clone());
            }
        }

        errors
    }

    /// Reload all plugins from registered directories.
    pub async fn reload_all(&self) -> Result<usize, String> {
        let dirs = self.plugin_dirs.read().await.clone();
        let mut total_loaded = 0;

        // Clear existing plugins
        self.plugins.write().await.clear();

        for dir in &dirs {
            match self.scan_dir(dir).await {
                Ok(count) => total_loaded += count,
                Err(e) => {
                    warn!(dir = %dir.display(), error = %e, "Failed to scan plugin directory");
                }
            }
        }

        info!(total_loaded, "All plugins reloaded");
        Ok(total_loaded)
    }

    /// Get plugin count.
    pub async fn plugin_count(&self) -> usize {
        self.plugins.read().await.len()
    }

    /// Get enabled plugin count.
    pub async fn enabled_count(&self) -> usize {
        self.plugins
            .read()
            .await
            .values()
            .filter(|p| p.status == PluginStatus::Loaded)
            .count()
    }

    /// Get registered directory count.
    pub async fn dir_count(&self) -> usize {
        self.plugin_dirs.read().await.len()
    }
}

impl Default for PluginService {
    fn default() -> Self {
        Self::new()
    }
}
