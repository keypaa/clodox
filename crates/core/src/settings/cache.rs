use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::RwLock;

use serde::Serialize;

use crate::types::SettingSource;

/// Cached settings file with modification time tracking.
#[derive(Debug, Clone)]
struct CacheEntry {
    path: PathBuf,
    value: serde_json::Value,
    mtime: std::time::SystemTime,
}

/// Settings cache for efficient repeated access.
#[derive(Debug)]
pub struct SettingsCache {
    cache: RwLock<HashMap<PathBuf, CacheEntry>>,
}

impl SettingsCache {
    pub fn new() -> Self {
        Self {
            cache: RwLock::new(HashMap::new()),
        }
    }

    /// Get settings from a file, using cache if valid.
    pub fn get(&self, path: &PathBuf) -> Result<serde_json::Value, crate::settings::SettingsError> {
        // Check cache
        {
            let cache = self.cache.read().unwrap();
            if let Some(entry) = cache.get(path) {
                // Check if file has been modified
                if let Ok(metadata) = std::fs::metadata(path) {
                    if let Ok(current_mtime) = metadata.modified() {
                        if current_mtime == entry.mtime {
                            return Ok(entry.value.clone());
                        }
                    }
                }
            }
        }

        // Cache miss or stale — reload
        let value = super::sources::parse_settings_file(path)?;

        // Update cache
        if let Ok(mtime) = std::fs::metadata(path).and_then(|m| m.modified()) {
            let mut cache = self.cache.write().unwrap();
            cache.insert(
                path.clone(),
                CacheEntry {
                    path: path.clone(),
                    value: value.clone(),
                    mtime,
                },
            );
        }

        Ok(value)
    }

    /// Invalidate all cached entries.
    pub fn reset(&self) {
        let mut cache = self.cache.write().unwrap();
        cache.clear();
    }

    /// Invalidate a specific cached entry.
    pub fn invalidate(&self, path: &PathBuf) {
        let mut cache = self.cache.write().unwrap();
        cache.remove(path);
    }
}

impl Default for SettingsCache {
    fn default() -> Self {
        Self::new()
    }
}

/// Global settings cache instance.
pub static GLOBAL_CACHE: std::sync::LazyLock<SettingsCache> =
    std::sync::LazyLock::new(SettingsCache::new);

/// Reset all settings caches.
pub fn reset_settings_cache() {
    GLOBAL_CACHE.reset();
}

/// Get settings for a single source (cached).
pub fn get_settings_for_source(source: SettingSource) -> Result<serde_json::Value, crate::settings::SettingsError> {
    let path = match source {
        SettingSource::UserSettings => super::sources::user_settings_path(),
        SettingSource::ProjectSettings => super::sources::project_settings_path(),
        SettingSource::LocalSettings => super::sources::local_settings_path(),
        _ => return Err(crate::settings::SettingsError::InvalidSource),
    };

    match path {
        Some(p) if p.exists() => GLOBAL_CACHE.get(&p),
        _ => Ok(serde_json::Value::Object(serde_json::Map::new())),
    }
}
