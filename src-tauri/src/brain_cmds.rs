//! Commands for the INVADER BRAIN (the built-in neural network) and its voice (a local language model).

use std::path::PathBuf;
use std::sync::atomic::Ordering;
use std::time::{SystemTime, UNIX_EPOCH};

use invaders_core::brain::{self, Brain, BrainStatus, Label, Lesson, Sample, Score};
use invaders_core::voice::{self, ChatMessage, VoiceStatus, OLLAMA_ADDR};
use serde::Deserialize;
use tauri::{AppHandle, Emitter, Manager, State};

use crate::commands::AppState;

/// Something on disk the brain can look at.
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Thing {
    path: String,
    is_dir: bool,
    size: u64,
    modified: Option<i64>,
}

#[derive(Debug, Deserialize)]
pub struct Teaching {
    #[serde(flatten)]
    thing: Thing,
    label: Label,
}

/// Builds what the brain sees: home folder as `~`, size in MB, age in days.
pub fn sample(path: &str, is_dir: bool, size: u64, modified: Option<i64>) -> Sample {
    let home = dirs::home_dir().map(|h| h.to_string_lossy().into_owned());
    let now = SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_secs() as i64).unwrap_or(0);
    Sample {
        path: brain::normalize_path(path, home.as_deref()),
        is_dir,
        size_mb: size as f32 / 1_048_576.0,
        age_days: modified.map(|m| (now - m) as f32 / 86_400.0).unwrap_or(-1.0),
    }
}

fn lessons_file(app: &AppHandle) -> Result<PathBuf, String> {
    app.path().app_data_dir().map(|dir| dir.join("brain-lessons.jsonl")).map_err(|e| e.to_string())
}

/// Runs `f` with the brain, waking it up (training it) on first use.
pub fn with_brain<R>(app: &AppHandle, state: &AppState, f: impl FnOnce(&Brain) -> R) -> R {
    let mut slot = state.brain.lock().unwrap();
    let brain = slot.get_or_insert_with(|| {
        let personal = lessons_file(app).map(|file| brain::load_lessons(&file)).unwrap_or_default();
        Brain::new(&personal)
    });
    f(brain)
}

#[tauri::command]
pub fn brain_status(app: AppHandle, state: State<'_, AppState>) -> BrainStatus {
    with_brain(&app, &state, |b| b.status().clone())
}

#[tauri::command]
pub fn brain_score(app: AppHandle, state: State<'_, AppState>, things: Vec<Thing>) -> Vec<Score> {
    with_brain(&app, &state, |b| {
        things.iter().map(|t| b.score(&sample(&t.path, t.is_dir, t.size, t.modified))).collect()
    })
}

/// Learns from the player's decisions and retrains (takes a fraction of a second).
#[tauri::command]
pub async fn brain_teach(
    app: AppHandle,
    state: State<'_, AppState>,
    lessons: Vec<Teaching>,
) -> Result<BrainStatus, String> {
    let file = lessons_file(&app)?;
    let new: Vec<Lesson> = lessons
        .iter()
        .map(|t| Lesson {
            sample: sample(&t.thing.path, t.thing.is_dir, t.thing.size, t.thing.modified),
            label: t.label,
        })
        .collect();
    let trained = tauri::async_runtime::spawn_blocking(move || {
        let mut all = brain::load_lessons(&file);
        brain::merge_lessons(&mut all, new);
        brain::save_lessons(&file, &all).map_err(|e| e.to_string())?;
        Ok::<_, String>(Brain::new(&all))
    })
    .await
    .map_err(|e| e.to_string())??;
    let status = trained.status().clone();
    *state.brain.lock().unwrap() = Some(trained);
    Ok(status)
}

/// Saves personal lessons as CSV (same format as `brain/dataset.csv`) so they can be shared in a PR.
#[tauri::command]
pub fn brain_export(app: AppHandle, path: String) -> Result<usize, String> {
    let lessons = brain::load_lessons(&lessons_file(&app)?);
    std::fs::write(&path, brain::to_csv(&lessons)).map_err(|e| e.to_string())?;
    Ok(lessons.len())
}

/// Forgets personal lessons. School lessons stay.
#[tauri::command]
pub fn brain_forget(app: AppHandle, state: State<'_, AppState>) -> Result<BrainStatus, String> {
    let file = lessons_file(&app)?;
    if file.exists() {
        std::fs::remove_file(&file).map_err(|e| e.to_string())?;
    }
    let fresh = Brain::new(&[]);
    let status = fresh.status().clone();
    *state.brain.lock().unwrap() = Some(fresh);
    Ok(status)
}

#[tauri::command]
pub async fn voice_status() -> VoiceStatus {
    tauri::async_runtime::spawn_blocking(|| voice::status(OLLAMA_ADDR)).await.unwrap_or(VoiceStatus {
        running: false,
        version: None,
        models: Vec::new(),
    })
}

/// Downloads a voice model through Ollama, emitting `voice-pull` progress events.
#[tauri::command]
pub async fn voice_pull(app: AppHandle, model: String) -> Result<(), String> {
    tauri::async_runtime::spawn_blocking(move || {
        voice::pull(OLLAMA_ADDR, &model, |progress| {
            let _ = app.emit("voice-pull", progress);
        })
    })
    .await
    .map_err(|e| e.to_string())?
}

/// Asks the voice model, emitting each piece of the answer as a `voice-token` event.
#[tauri::command]
pub async fn voice_chat(
    app: AppHandle,
    state: State<'_, AppState>,
    model: String,
    messages: Vec<ChatMessage>,
) -> Result<String, String> {
    let stop = state.voice_stop.clone();
    stop.store(false, Ordering::Relaxed);
    tauri::async_runtime::spawn_blocking(move || {
        voice::chat(OLLAMA_ADDR, &model, &messages, &stop, |piece| {
            let _ = app.emit("voice-token", piece);
        })
    })
    .await
    .map_err(|e| e.to_string())?
}

#[tauri::command]
pub fn voice_stop(state: State<'_, AppState>) {
    state.voice_stop.store(true, Ordering::Relaxed);
}
