//! Data types shared between the scanner and the UI (serialized to JSON for the frontend).

use serde::{Deserialize, Serialize};

/// A "level" in the UI: a bucket of things that can be cleaned.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Category {
    Caches,
    DevJunk,
    Logs,
    Trash,
    Installers,
    OldDownloads,
    Backups,
    Duplicates,
    LargeFiles,
}

impl Category {
    pub const ALL: [Category; 9] = [
        Category::Caches,
        Category::DevJunk,
        Category::Logs,
        Category::Trash,
        Category::Installers,
        Category::OldDownloads,
        Category::Backups,
        Category::Duplicates,
        Category::LargeFiles,
    ];
}

/// How careful the user should be before deleting an item.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Risk {
    /// Regenerated automatically by apps/tools (caches, build output, logs).
    Safe,
    /// Might be something the user cares about. Look before you shoot.
    Review,
}

/// A cleanable thing: a single file or a whole folder.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Item {
    pub path: String,
    /// Bytes actually occupied on disk.
    pub size: u64,
    pub is_dir: bool,
    /// Last modification time, seconds since the Unix epoch.
    pub modified: Option<i64>,
    pub category: Category,
    pub risk: Risk,
    /// Short human-readable explanation, e.g. "Rust build output".
    pub label: String,
    /// Duplicate group id (only for [`Category::Duplicates`]).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub group: Option<u32>,
    /// What the [brain](crate::brain) thinks, filled in by the app after the scan.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub brain: Option<crate::brain::Score>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CategorySummary {
    pub category: Category,
    /// Number of items found (before truncation).
    pub count: usize,
    /// Bytes that can be freed. For duplicates: everything except one copy per group.
    pub bytes: u64,
    /// Number of items included in the report.
    pub shown: usize,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ScanReport {
    pub roots: Vec<String>,
    pub files: u64,
    pub dirs: u64,
    pub bytes: u64,
    /// Folders/files that could not be read (usually permission denied).
    pub errors: u64,
    pub elapsed_ms: u64,
    pub cancelled: bool,
    pub summaries: Vec<CategorySummary>,
    pub items: Vec<Item>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct ScanOptions {
    pub large_file_min_bytes: u64,
    pub old_download_days: u64,
    pub find_duplicates: bool,
    pub duplicate_min_bytes: u64,
    pub max_items_per_category: usize,
}

impl Default for ScanOptions {
    fn default() -> Self {
        Self {
            large_file_min_bytes: 100 * MB,
            old_download_days: 180,
            find_duplicates: true,
            duplicate_min_bytes: MB,
            max_items_per_category: 2000,
        }
    }
}

pub const MB: u64 = 1024 * 1024;
