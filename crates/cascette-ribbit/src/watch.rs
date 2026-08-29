//! Hot-reload watcher for the builds.json database.
//!
//! Spawns a background task using the `notify` crate to watch the file for
//! modifications and trigger [`AppState::reload_database`] when a change is
//! detected. The server continues serving the previous data while the reload
//! is in progress.

use crate::server::AppState;
use notify::{Config, Event, EventKind, RecommendedWatcher, RecursiveMode, Watcher};
use std::sync::Arc;
use tokio::sync::mpsc;

/// Watch `builds.json` and reload [`AppState`] whenever the file is modified.
///
/// This function runs until the task is cancelled (e.g. on server shutdown).
/// Errors from the watcher are logged and ignored; the previous database
/// snapshot remains active.
pub async fn watch_builds(state: Arc<AppState>) {
    let builds_path = state.builds_path().to_path_buf();

    let (tx, mut rx) = mpsc::channel::<notify::Result<Event>>(16);

    let mut watcher = match RecommendedWatcher::new(
        move |res| {
            // notify callbacks are synchronous; use try_send to avoid blocking.
            let _ = tx.try_send(res);
        },
        Config::default(),
    ) {
        Ok(w) => w,
        Err(e) => {
            tracing::warn!("Could not start builds.json watcher: {e}");
            return;
        }
    };

    if let Err(e) = watcher.watch(&builds_path, RecursiveMode::NonRecursive) {
        tracing::warn!("Could not watch {:?}: {e}", builds_path);
        return;
    }

    tracing::info!("Hot-reload watcher active for {:?}", builds_path);

    while let Some(event_result) = rx.recv().await {
        match event_result {
            Ok(event) => {
                // Only act on data-modifying events (write, rename, create).
                let is_write = matches!(
                    event.kind,
                    EventKind::Create(_) | EventKind::Modify(_) | EventKind::Remove(_)
                );
                if is_write {
                    tracing::debug!("builds.json changed ({:?}), reloading", event.kind);
                    state.reload_database().await;
                }
            }
            Err(e) => {
                tracing::warn!("File watcher error: {e}");
            }
        }
    }
}
