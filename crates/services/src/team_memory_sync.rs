use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;

use serde::{Deserialize, Serialize};
use tokio::sync::RwLock;
use tracing::{debug, info, warn};

/// A shared memory entry that can be synced between teammates.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SharedMemoryEntry {
    pub key: String,
    pub value: String,
    pub author: String,
    pub timestamp: chrono::DateTime<chrono::Utc>,
    pub version: u64,
}

/// Sync status for a memory entry.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum SyncStatus {
    Synced,
    Pending,
    Conflict,
    Failed,
}

/// Sync event for tracking memory synchronization.
#[derive(Debug, Clone)]
pub enum SyncEvent {
    EntryAdded { key: String, author: String },
    EntryUpdated { key: String, author: String, version: u64 },
    EntryDeleted { key: String },
    ConflictDetected { key: String, authors: Vec<String> },
    SyncCompleted { entries_synced: usize },
    SyncFailed { error: String },
}

/// Team memory sync service — shared memory synchronization between teammates.
pub struct TeamMemorySyncService {
    /// Local memory store.
    local_store: RwLock<HashMap<String, SharedMemoryEntry>>,
    /// Team name.
    team_name: String,
    /// Agent ID of this instance.
    agent_id: String,
    /// Sync event broadcast channel.
    event_tx: tokio::sync::broadcast::Sender<SyncEvent>,
    /// Path to the shared memory file (for file-based sync).
    sync_file_path: Option<PathBuf>,
    /// Sync status tracking.
    sync_status: RwLock<HashMap<String, SyncStatus>>,
}

impl TeamMemorySyncService {
    pub fn new(team_name: &str, agent_id: &str) -> Self {
        let (event_tx, _) = tokio::sync::broadcast::channel(64);
        Self {
            local_store: RwLock::new(HashMap::new()),
            team_name: team_name.to_string(),
            agent_id: agent_id.to_string(),
            event_tx,
            sync_file_path: None,
            sync_status: RwLock::new(HashMap::new()),
        }
    }

    /// Set the sync file path for file-based synchronization.
    pub fn set_sync_file(&mut self, path: PathBuf) {
        self.sync_file_path = Some(path);
    }

    /// Subscribe to sync events.
    pub fn subscribe(&self) -> tokio::sync::broadcast::Receiver<SyncEvent> {
        self.event_tx.subscribe()
    }

    /// Add a memory entry to the local store.
    pub async fn add_entry(&self, key: &str, value: &str) -> Result<(), String> {
        let mut store = self.local_store.write().await;

        let entry = SharedMemoryEntry {
            key: key.to_string(),
            value: value.to_string(),
            author: self.agent_id.clone(),
            timestamp: chrono::Utc::now(),
            version: 1,
        };

        store.insert(key.to_string(), entry);

        let _ = self.event_tx.send(SyncEvent::EntryAdded {
            key: key.to_string(),
            author: self.agent_id.clone(),
        });

        // Update sync status
        self.sync_status.write().await.insert(key.to_string(), SyncStatus::Pending);

        debug!(key, "Memory entry added");
        Ok(())
    }

    /// Update an existing memory entry.
    pub async fn update_entry(&self, key: &str, value: &str) -> Result<(), String> {
        let mut store = self.local_store.write().await;

        let entry = store.get_mut(key).ok_or_else(|| format!("Entry not found: {key}"))?;

        // Check for conflicts
        if entry.author != self.agent_id {
            warn!(key, "Updating entry authored by another agent");
        }

        entry.value = value.to_string();
        entry.version += 1;
        entry.timestamp = chrono::Utc::now();
        entry.author = self.agent_id.clone();

        let version = entry.version;

        let _ = self.event_tx.send(SyncEvent::EntryUpdated {
            key: key.to_string(),
            author: self.agent_id.clone(),
            version,
        });

        self.sync_status.write().await.insert(key.to_string(), SyncStatus::Pending);

        debug!(key, version, "Memory entry updated");
        Ok(())
    }

    /// Delete a memory entry.
    pub async fn delete_entry(&self, key: &str) -> Result<(), String> {
        let mut store = self.local_store.write().await;

        if store.remove(key).is_some() {
            let _ = self.event_tx.send(SyncEvent::EntryDeleted {
                key: key.to_string(),
            });

            self.sync_status.write().await.remove(key);

            debug!(key, "Memory entry deleted");
            Ok(())
        } else {
            Err(format!("Entry not found: {key}"))
        }
    }

    /// Get a memory entry by key.
    pub async fn get_entry(&self, key: &str) -> Option<SharedMemoryEntry> {
        self.local_store.read().await.get(key).cloned()
    }

    /// Get all memory entries.
    pub async fn get_all_entries(&self) -> Vec<SharedMemoryEntry> {
        self.local_store.read().await.values().cloned().collect()
    }

    /// Get entries by author.
    pub async fn get_entries_by_author(&self, author: &str) -> Vec<SharedMemoryEntry> {
        self.local_store
            .read()
            .await
            .values()
            .filter(|e| e.author == author)
            .cloned()
            .collect()
    }

    /// Merge entries from another teammate (sync operation).
    pub async fn merge_entries(&self, remote_entries: Vec<SharedMemoryEntry>) -> Vec<SyncEvent> {
        let mut events = Vec::new();
        let mut store = self.local_store.write().await;
        let mut sync_status = self.sync_status.write().await;

        let mut synced_count = 0;

        for remote in remote_entries {
            let key = remote.key.clone();

            if let Some(local) = store.get(&key) {
                // Conflict detection: compare versions
                if remote.version > local.version {
                    // Remote is newer, accept it
                    store.insert(key.clone(), remote.clone());
                    sync_status.insert(key, SyncStatus::Synced);
                    synced_count += 1;
                } else if remote.version < local.version {
                    // Local is newer, keep it
                    sync_status.insert(key.clone(), SyncStatus::Synced);
                    synced_count += 1;
                } else if remote.value != local.value {
                    // Same version but different content — conflict
                    sync_status.insert(key.clone(), SyncStatus::Conflict);
                    events.push(SyncEvent::ConflictDetected {
                        key: key.clone(),
                        authors: vec![local.author.clone(), remote.author.clone()],
                    });
                }
            } else {
                // New entry from remote
                store.insert(key.clone(), remote.clone());
                sync_status.insert(key, SyncStatus::Synced);
                synced_count += 1;
            }
        }

        if synced_count > 0 {
            events.push(SyncEvent::SyncCompleted {
                entries_synced: synced_count,
            });
        }

        info!(synced = synced_count, "Memory sync completed");
        events
    }

    /// Save local store to sync file.
    pub async fn save_to_file(&self) -> Result<(), String> {
        let path = self
            .sync_file_path
            .as_ref()
            .ok_or_else(|| "No sync file path configured".to_string())?;

        let store = self.local_store.read().await;
        let entries: Vec<&SharedMemoryEntry> = store.values().collect();

        let content = serde_json::to_string_pretty(&entries)
            .map_err(|e| format!("Failed to serialize: {}", e))?;

        // Ensure parent directory exists
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)
                .map_err(|e| format!("Failed to create directory: {}", e))?;
        }

        std::fs::write(path, content)
            .map_err(|e| format!("Failed to write sync file: {}", e))?;

        // Mark all as synced
        for key in store.keys() {
            self.sync_status.write().await.insert(key.clone(), SyncStatus::Synced);
        }

        info!(path = %path.display(), count = entries.len(), "Memory saved to file");
        Ok(())
    }

    /// Load entries from sync file.
    pub async fn load_from_file(&self) -> Result<Vec<SharedMemoryEntry>, String> {
        let path = self
            .sync_file_path
            .as_ref()
            .ok_or_else(|| "No sync file path configured".to_string())?;

        if !path.exists() {
            return Ok(Vec::new());
        }

        let content = std::fs::read_to_string(path)
            .map_err(|e| format!("Failed to read sync file: {}", e))?;

        let entries: Vec<SharedMemoryEntry> = serde_json::from_str(&content)
            .map_err(|e| format!("Failed to parse sync file: {}", e))?;

        info!(path = %path.display(), count = entries.len(), "Memory loaded from file");
        Ok(entries)
    }

    /// Get sync status for all entries.
    pub async fn get_sync_status(&self) -> HashMap<String, SyncStatus> {
        self.sync_status.read().await.clone()
    }

    /// Get pending entries (not yet synced).
    pub async fn get_pending_entries(&self) -> Vec<SharedMemoryEntry> {
        let status = self.sync_status.read().await;
        let store = self.local_store.read().await;

        store
            .values()
            .filter(|e| status.get(&e.key) == Some(&SyncStatus::Pending))
            .cloned()
            .collect()
    }

    /// Get conflict entries.
    pub async fn get_conflict_entries(&self) -> Vec<SharedMemoryEntry> {
        let status = self.sync_status.read().await;
        let store = self.local_store.read().await;

        store
            .values()
            .filter(|e| status.get(&e.key) == Some(&SyncStatus::Conflict))
            .cloned()
            .collect()
    }

    /// Get entry count.
    pub async fn entry_count(&self) -> usize {
        self.local_store.read().await.len()
    }

    /// Get team name.
    pub fn team_name(&self) -> &str {
        &self.team_name
    }

    /// Get agent ID.
    pub fn agent_id(&self) -> &str {
        &self.agent_id
    }

    /// Clear all local entries.
    pub async fn clear(&self) {
        self.local_store.write().await.clear();
        self.sync_status.write().await.clear();
        info!("Local memory cleared");
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_new_service_is_empty() {
        let service = TeamMemorySyncService::new("test-team", "agent-1");
        assert_eq!(service.entry_count().await, 0);
        assert_eq!(service.team_name(), "test-team");
        assert_eq!(service.agent_id(), "agent-1");
    }

    #[tokio::test]
    async fn test_add_and_get_entry() {
        let service = TeamMemorySyncService::new("test-team", "agent-1");
        service.add_entry("key1", "value1").await.unwrap();
        let entry = service.get_entry("key1").await;
        assert!(entry.is_some());
        let entry = entry.unwrap();
        assert_eq!(entry.key, "key1");
        assert_eq!(entry.value, "value1");
        assert_eq!(entry.author, "agent-1");
        assert_eq!(entry.version, 1);
    }

    #[tokio::test]
    async fn test_get_nonexistent_entry() {
        let service = TeamMemorySyncService::new("test-team", "agent-1");
        assert!(service.get_entry("nonexistent").await.is_none());
    }

    #[tokio::test]
    async fn test_update_entry() {
        let service = TeamMemorySyncService::new("test-team", "agent-1");
        service.add_entry("key1", "value1").await.unwrap();
        service.update_entry("key1", "value2").await.unwrap();
        let entry = service.get_entry("key1").await.unwrap();
        assert_eq!(entry.value, "value2");
        assert_eq!(entry.version, 2);
    }

    #[tokio::test]
    async fn test_update_nonexistent_entry() {
        let service = TeamMemorySyncService::new("test-team", "agent-1");
        let result = service.update_entry("nonexistent", "value").await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_delete_entry() {
        let service = TeamMemorySyncService::new("test-team", "agent-1");
        service.add_entry("key1", "value1").await.unwrap();
        service.delete_entry("key1").await.unwrap();
        assert!(service.get_entry("key1").await.is_none());
    }

    #[tokio::test]
    async fn test_delete_nonexistent_entry() {
        let service = TeamMemorySyncService::new("test-team", "agent-1");
        let result = service.delete_entry("nonexistent").await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_get_all_entries() {
        let service = TeamMemorySyncService::new("test-team", "agent-1");
        service.add_entry("key1", "value1").await.unwrap();
        service.add_entry("key2", "value2").await.unwrap();
        let entries = service.get_all_entries().await;
        assert_eq!(entries.len(), 2);
    }

    #[tokio::test]
    async fn test_get_entries_by_author() {
        let service = TeamMemorySyncService::new("test-team", "agent-1");
        service.add_entry("key1", "value1").await.unwrap();
        service.add_entry("key2", "value2").await.unwrap();
        let entries = service.get_entries_by_author("agent-1").await;
        assert_eq!(entries.len(), 2);
        let entries = service.get_entries_by_author("agent-2").await;
        assert!(entries.is_empty());
    }

    #[tokio::test]
    async fn test_merge_new_entries() {
        let service = TeamMemorySyncService::new("test-team", "agent-1");
        let remote = vec![
            SharedMemoryEntry {
                key: "key1".to_string(),
                value: "value1".to_string(),
                author: "agent-2".to_string(),
                timestamp: chrono::Utc::now(),
                version: 1,
            },
        ];
        let events = service.merge_entries(remote).await;
        assert!(events.iter().any(|e| matches!(e, SyncEvent::SyncCompleted { .. })));
        assert_eq!(service.entry_count().await, 1);
    }

    #[tokio::test]
    async fn test_merge_remote_newer_version() {
        let service = TeamMemorySyncService::new("test-team", "agent-1");
        service.add_entry("key1", "local").await.unwrap();
        let remote = vec![
            SharedMemoryEntry {
                key: "key1".to_string(),
                value: "remote".to_string(),
                author: "agent-2".to_string(),
                timestamp: chrono::Utc::now(),
                version: 5,
            },
        ];
        let _ = service.merge_entries(remote).await;
        let entry = service.get_entry("key1").await.unwrap();
        assert_eq!(entry.value, "remote");
    }

    #[tokio::test]
    async fn test_merge_local_newer_version() {
        let service = TeamMemorySyncService::new("test-team", "agent-1");
        service.add_entry("key1", "local").await.unwrap();
        // Bump local version
        service.update_entry("key1", "local-updated").await.unwrap();
        let remote = vec![
            SharedMemoryEntry {
                key: "key1".to_string(),
                value: "remote".to_string(),
                author: "agent-2".to_string(),
                timestamp: chrono::Utc::now(),
                version: 1,
            },
        ];
        let _ = service.merge_entries(remote).await;
        let entry = service.get_entry("key1").await.unwrap();
        assert_eq!(entry.value, "local-updated");
    }

    #[tokio::test]
    async fn test_clear() {
        let service = TeamMemorySyncService::new("test-team", "agent-1");
        service.add_entry("key1", "value1").await.unwrap();
        service.clear().await;
        assert_eq!(service.entry_count().await, 0);
    }

    #[tokio::test]
    async fn test_get_pending_and_conflict_entries() {
        let service = TeamMemorySyncService::new("test-team", "agent-1");
        service.add_entry("key1", "value1").await.unwrap();
        let pending = service.get_pending_entries().await;
        assert_eq!(pending.len(), 1);
        let conflicts = service.get_conflict_entries().await;
        assert!(conflicts.is_empty());
    }

    #[tokio::test]
    async fn test_get_sync_status() {
        let service = TeamMemorySyncService::new("test-team", "agent-1");
        service.add_entry("key1", "value1").await.unwrap();
        let status = service.get_sync_status().await;
        assert!(status.contains_key("key1"));
        assert_eq!(status["key1"], SyncStatus::Pending);
    }

    #[tokio::test]
    async fn test_save_to_file_no_path() {
        let service = TeamMemorySyncService::new("test-team", "agent-1");
        let result = service.save_to_file().await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_load_from_file_no_path() {
        let service = TeamMemorySyncService::new("test-team", "agent-1");
        let result = service.load_from_file().await;
        assert!(result.is_err());
    }
}
