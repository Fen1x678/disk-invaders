//! Live counters shared between the scanning threads and the UI.

use std::path::Path;
use std::sync::atomic::{AtomicBool, AtomicU64, AtomicU8, Ordering::Relaxed};
use std::sync::Mutex;

use serde::Serialize;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Phase {
    /// Walking the folder tree.
    Walk,
    /// Hashing same-size files to find identical copies.
    Duplicates,
    Done,
}

#[derive(Debug, Default)]
pub struct Progress {
    files: AtomicU64,
    dirs: AtomicU64,
    bytes: AtomicU64,
    errors: AtomicU64,
    hashed: AtomicU64,
    hash_total: AtomicU64,
    phase: AtomicU8,
    cancelled: AtomicBool,
    current: Mutex<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProgressSnapshot {
    pub files: u64,
    pub dirs: u64,
    pub bytes: u64,
    pub errors: u64,
    pub hashed: u64,
    pub hash_total: u64,
    pub phase: Phase,
    pub current: String,
}

impl Progress {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn cancel(&self) {
        self.cancelled.store(true, Relaxed);
    }

    pub fn is_cancelled(&self) -> bool {
        self.cancelled.load(Relaxed)
    }

    pub fn phase(&self) -> Phase {
        match self.phase.load(Relaxed) {
            0 => Phase::Walk,
            1 => Phase::Duplicates,
            _ => Phase::Done,
        }
    }

    pub fn snapshot(&self) -> ProgressSnapshot {
        ProgressSnapshot {
            files: self.files.load(Relaxed),
            dirs: self.dirs.load(Relaxed),
            bytes: self.bytes.load(Relaxed),
            errors: self.errors.load(Relaxed),
            hashed: self.hashed.load(Relaxed),
            hash_total: self.hash_total.load(Relaxed),
            phase: self.phase(),
            current: self.current.lock().map(|c| c.clone()).unwrap_or_default(),
        }
    }

    pub(crate) fn set_phase(&self, phase: Phase) {
        self.phase.store(phase as u8, Relaxed);
    }

    pub(crate) fn visit_dir(&self, path: &Path) {
        let n = self.dirs.fetch_add(1, Relaxed);
        // Updating the "current folder" text for every folder would make threads fight over the lock.
        if n.is_multiple_of(64) {
            if let Ok(mut current) = self.current.try_lock() {
                *current = path.to_string_lossy().into_owned();
            }
        }
    }

    pub(crate) fn add_files(&self, files: u64, bytes: u64) {
        self.files.fetch_add(files, Relaxed);
        self.bytes.fetch_add(bytes, Relaxed);
    }

    pub(crate) fn add_error(&self) {
        self.errors.fetch_add(1, Relaxed);
    }

    pub(crate) fn add_hash_work(&self, files: u64) {
        self.hash_total.fetch_add(files, Relaxed);
    }

    pub(crate) fn hashed_one(&self) {
        self.hashed.fetch_add(1, Relaxed);
    }
}
