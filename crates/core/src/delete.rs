//! Deleting things, carefully. Trash by default, permanent only when asked, [`Guard`] always.

use std::fs;
use std::io::ErrorKind;

use serde::Serialize;

use crate::safety::Guard;

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DeleteOutcome {
    pub path: String,
    pub ok: bool,
    /// Machine-readable reason, e.g. `blocked:system`, `not_found`, `permission_denied`.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

/// Moves `paths` to the Trash / Recycle Bin, or deletes them for good when `permanent` is set.
/// Each path is handled on its own: one failure never stops the rest.
pub fn delete_paths(paths: &[String], permanent: bool, guard: &Guard) -> Vec<DeleteOutcome> {
    #[allow(unused_mut)]
    let mut trash = trash::TrashContext::default();
    #[cfg(target_os = "macos")]
    {
        // The default (asking Finder via AppleScript) pops up an extra permission prompt.
        use trash::macos::{DeleteMethod, TrashContextExtMacos};
        trash.set_delete_method(DeleteMethod::NsFileManager);
    }
    paths
        .iter()
        .map(|path| {
            let result = delete_one(path, permanent, guard, &trash);
            DeleteOutcome { path: path.clone(), ok: result.is_ok(), error: result.err() }
        })
        .collect()
}

fn delete_one(path: &str, permanent: bool, guard: &Guard, trash: &trash::TrashContext) -> Result<(), String> {
    guard.check(path).map_err(|blocked| format!("blocked:{}", blocked.code()))?;
    let meta = fs::symlink_metadata(path).map_err(io_error)?;
    if !permanent {
        return trash.delete(path).map_err(|e| format!("trash_failed:{e:?}"));
    }
    let result = if meta.is_dir() { fs::remove_dir_all(path) } else { fs::remove_file(path) };
    result.map_err(io_error)
}

fn io_error(e: std::io::Error) -> String {
    match e.kind() {
        ErrorKind::NotFound => "not_found".into(),
        ErrorKind::PermissionDenied => "permission_denied".into(),
        _ => format!("io:{e}"),
    }
}
