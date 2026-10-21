//! The commands the UI can call (`invoke("start_scan", ...)` in `src/api.ts`).

use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use invaders_core::brain::{Brain, Score};
use invaders_core::tree::EntryKind;
use invaders_core::{delete_paths, DeleteOutcome, Guard, Progress, ScanOptions, ScanReport, ScanTree, TreeEntry};
use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Emitter, State};

use crate::brain_cmds::{sample, with_brain};
use crate::system::SystemInfo;

#[derive(Default)]
pub struct AppState {
    /// Progress of the running scan, if any.
    progress: Mutex<Option<Arc<Progress>>>,
    /// Folder tree of the last finished scan.
    tree: Mutex<ScanTree>,
    /// The neural network, trained on first use.
    pub(crate) brain: Mutex<Option<Brain>>,
    /// Set to stop the voice mid-sentence.
    pub(crate) voice_stop: Arc<AtomicBool>,
}

#[tauri::command]
pub fn system_info() -> SystemInfo {
    crate::system::system_info()
}

/// Scans `roots`, emitting `scan-progress` events about 10 times per second.
#[tauri::command]
pub async fn start_scan(
    app: AppHandle,
    state: State<'_, AppState>,
    roots: Vec<String>,
    options: ScanOptions,
) -> Result<ScanReport, String> {
    let progress = Arc::new(Progress::new());
    {
        let mut running = state.progress.lock().unwrap();
        if running.is_some() {
            return Err("busy".into());
        }
        *running = Some(progress.clone());
    }

    let stop = Arc::new(AtomicBool::new(false));
    let ticker = {
        let (app, progress, stop) = (app.clone(), progress.clone(), stop.clone());
        std::thread::spawn(move || {
            while !stop.load(Ordering::Relaxed) {
                let _ = app.emit("scan-progress", progress.snapshot());
                std::thread::sleep(Duration::from_millis(100));
            }
        })
    };

    let worker = progress.clone();
    let roots: Vec<PathBuf> = roots.into_iter().map(PathBuf::from).collect();
    let result = tauri::async_runtime::spawn_blocking(move || invaders_core::scan(&roots, &options, &worker)).await;

    stop.store(true, Ordering::Relaxed);
    let _ = ticker.join();
    state.progress.lock().unwrap().take();

    let (mut report, tree) = result.map_err(|e| format!("scan crashed: {e}"))?;
    *state.tree.lock().unwrap() = tree;
    with_brain(&app, &state, |brain| {
        for item in &mut report.items {
            item.brain = Some(brain.score(&sample(&item.path, item.is_dir, item.size, item.modified)));
        }
    });
    Ok(report)
}

#[tauri::command]
pub fn cancel_scan(state: State<'_, AppState>) {
    if let Some(progress) = state.progress.lock().unwrap().as_ref() {
        progress.cancel();
    }
}

#[derive(Serialize)]
pub struct ScoredEntry {
    #[serde(flatten)]
    entry: TreeEntry,
    #[serde(skip_serializing_if = "Option::is_none")]
    brain: Option<Score>,
}

/// Contents of a folder from the last scan (`None` = the scanned roots), with the brain's opinion.
#[tauri::command]
pub fn list_dir(app: AppHandle, state: State<'_, AppState>, path: Option<String>) -> Result<Vec<ScoredEntry>, String> {
    let entries = state.tree.lock().unwrap().children(path.as_deref()).ok_or_else(|| "not_found".to_string())?;
    Ok(with_brain(&app, &state, |brain| {
        entries
            .into_iter()
            .map(|entry| {
                let brain = entry
                    .path
                    .as_deref()
                    .map(|p| brain.score(&sample(p, entry.kind == EntryKind::Dir, entry.size, None)));
                ScoredEntry { entry, brain }
            })
            .collect()
    }))
}

#[derive(Debug, Deserialize)]
pub struct DeleteRequest {
    path: String,
    size: u64,
}

/// Moves items to the Trash (or deletes them for good) and keeps the folder tree in sync.
#[tauri::command]
pub async fn delete_items(
    state: State<'_, AppState>,
    items: Vec<DeleteRequest>,
    permanent: bool,
) -> Result<Vec<DeleteOutcome>, String> {
    let paths: Vec<String> = items.iter().map(|i| i.path.clone()).collect();
    let outcomes =
        tauri::async_runtime::spawn_blocking(move || delete_paths(&paths, permanent, &Guard::for_this_computer()))
            .await
            .map_err(|e| e.to_string())?;

    let mut tree = state.tree.lock().unwrap();
    for (outcome, item) in outcomes.iter().zip(&items) {
        if outcome.ok {
            tree.remove(&item.path, item.size);
        }
    }
    Ok(outcomes)
}
