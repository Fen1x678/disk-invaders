//! # invaders-core
//!
//! Everything Disk Invaders does that isn't pixels:
//!
//! - [`scanner`] walks the disk in parallel and builds a size tree,
//! - [`rules`] recognise caches, build output, logs, installers...,
//! - [`dupes`] finds byte-identical files,
//! - [`safety`] refuses to delete anything that could break the computer,
//! - [`delete`] moves things to the Trash (or deletes them for good),
//! - [`brain`] is a tiny neural network, written from scratch, that learns what junk looks like,
//! - [`voice`] lets the brain talk through a free local language model (Ollama).
//!
//! No UI code lives here, so it can be tested with plain `cargo test -p invaders-core`.

pub mod brain;
pub mod delete;
pub mod dupes;
pub mod model;
pub mod platform;
pub mod progress;
pub mod rules;
pub mod safety;
pub mod scanner;
pub mod tree;
pub mod voice;

pub use delete::{delete_paths, DeleteOutcome};
pub use model::{Category, CategorySummary, Item, Risk, ScanOptions, ScanReport};
pub use platform::Platform;
pub use progress::{Phase, Progress, ProgressSnapshot};
pub use safety::Guard;
pub use scanner::scan;
pub use tree::{ScanTree, TreeEntry};
