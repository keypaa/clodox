use std::sync::{Arc, RwLock};

pub use cc_core::state::AppState;

/// Shared state wrapper.
pub type SharedState = Arc<RwLock<AppState>>;

/// Create a new shared state.
pub fn create_state() -> SharedState {
    Arc::new(RwLock::new(AppState::default()))
}

/// Read lock helper.
pub fn read_state(state: &SharedState) -> std::sync::RwLockReadGuard<AppState> {
    state.read().expect("State lock poisoned")
}

/// Write lock helper.
pub fn write_state(state: &SharedState) -> std::sync::RwLockWriteGuard<AppState> {
    state.write().expect("State lock poisoned")
}

/// Clone state for snapshotting (used during render).
pub fn snapshot_state(state: &SharedState) -> AppState {
    read_state(state).clone()
}
