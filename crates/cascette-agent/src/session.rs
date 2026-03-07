//! In-memory game session tracker.
//!
//! Sessions are transient (they don't survive agent restarts), so no SQLite
//! persistence is needed. The tracker coordinates with `process_detection` to
//! detect stale sessions from crashed processes.

use std::collections::HashMap;
use std::sync::Arc;

use chrono::{DateTime, Utc};
use serde::Serialize;
use tokio::sync::RwLock;

use crate::process_detection;

/// A tracked game session.
#[derive(Debug, Clone, Serialize)]
pub struct GameSession {
    /// Product code (e.g., "wow_classic_era").
    pub product_code: String,
    /// OS process ID of the running game.
    pub pid: Option<u32>,
    /// When the session started.
    pub started_at: DateTime<Utc>,
}

/// Tracks active game sessions in memory.
#[derive(Debug, Clone)]
pub struct SessionTracker {
    sessions: Arc<RwLock<HashMap<String, GameSession>>>,
}

impl SessionTracker {
    /// Create a new session tracker.
    #[must_use]
    pub fn new() -> Self {
        Self {
            sessions: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    /// Register a new game session.
    pub async fn start_session(&self, product_code: &str, pid: Option<u32>) {
        let session = GameSession {
            product_code: product_code.to_string(),
            pid,
            started_at: Utc::now(),
        };
        self.sessions
            .write()
            .await
            .insert(product_code.to_string(), session);
    }

    /// Remove a game session.
    pub async fn end_session(&self, product_code: &str) -> Option<GameSession> {
        self.sessions.write().await.remove(product_code)
    }

    /// Check whether a product has an active session.
    pub async fn is_active(&self, product_code: &str) -> bool {
        self.sessions.read().await.contains_key(product_code)
    }

    /// Get a session for a specific product.
    pub async fn get(&self, product_code: &str) -> Option<GameSession> {
        self.sessions.read().await.get(product_code).cloned()
    }

    /// List all active sessions.
    pub async fn list(&self) -> Vec<GameSession> {
        self.sessions.read().await.values().cloned().collect()
    }

    /// Number of active sessions.
    pub async fn count(&self) -> usize {
        self.sessions.read().await.len()
    }

    /// Remove sessions whose PIDs are no longer running.
    ///
    /// Uses `process_detection::game_pids` to cross-reference tracked PIDs
    /// against live processes.
    pub async fn cleanup_dead_processes(&self) {
        let mut sessions = self.sessions.write().await;
        sessions.retain(|product_code, session| {
            if let Some(pid) = session.pid {
                // Check if the tracked PID is still among live game processes
                let live_pids = process_detection::game_pids(product_code);
                live_pids.contains(&pid)
            } else {
                // No PID tracked -- fall back to process detection
                process_detection::is_game_running(product_code)
            }
        });
    }
}

impl Default for SessionTracker {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_session_lifecycle() {
        let tracker = SessionTracker::new();

        assert!(!tracker.is_active("wow").await);
        assert_eq!(tracker.count().await, 0);

        tracker.start_session("wow", Some(1234)).await;
        assert!(tracker.is_active("wow").await);
        assert_eq!(tracker.count().await, 1);

        let session = tracker.get("wow").await.unwrap();
        assert_eq!(session.product_code, "wow");
        assert_eq!(session.pid, Some(1234));

        tracker.end_session("wow").await;
        assert!(!tracker.is_active("wow").await);
        assert_eq!(tracker.count().await, 0);
    }

    #[tokio::test]
    async fn test_list_sessions() {
        let tracker = SessionTracker::new();
        tracker.start_session("wow", Some(100)).await;
        tracker.start_session("wow_classic_era", Some(200)).await;

        let sessions = tracker.list().await;
        assert_eq!(sessions.len(), 2);
    }
}
