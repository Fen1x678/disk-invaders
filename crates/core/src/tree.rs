//! The folder tree built during a scan. Powers the Explorer view ("where did my space go?").

use std::path::{Path, PathBuf};

use serde::Serialize;

/// Files smaller than this are folded into a single "small files" row per folder.
/// Keeps memory low on disks with millions of tiny files.
pub const TREE_FILE_MIN: u64 = 1024 * 1024;

#[derive(Debug, Default)]
pub struct DirNode {
    pub name: String,
    /// Total bytes of everything inside (recursive).
    pub size: u64,
    /// Total number of files inside (recursive).
    pub files: u64,
    pub dirs: Vec<DirNode>,
    pub big_files: Vec<FileLeaf>,
    pub small_size: u64,
    pub small_count: u64,
}

#[derive(Debug)]
pub struct FileLeaf {
    pub name: String,
    pub size: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum EntryKind {
    Dir,
    File,
    SmallFiles,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TreeEntry {
    pub name: String,
    /// `None` for the aggregated "small files" row.
    pub path: Option<String>,
    pub size: u64,
    pub files: u64,
    pub kind: EntryKind,
}

#[derive(Debug, Default)]
pub struct ScanTree {
    roots: Vec<(PathBuf, DirNode)>,
}

impl ScanTree {
    pub fn new(roots: Vec<(PathBuf, DirNode)>) -> Self {
        Self { roots }
    }

    /// Entries inside `path`, largest first. `None` lists the scanned roots.
    pub fn children(&self, path: Option<&str>) -> Option<Vec<TreeEntry>> {
        let Some(path) = path else {
            let roots = self.roots.iter().map(|(root, node)| TreeEntry {
                name: root.to_string_lossy().into_owned(),
                path: Some(root.to_string_lossy().into_owned()),
                size: node.size,
                files: node.files,
                kind: EntryKind::Dir,
            });
            return Some(roots.collect());
        };
        let base = Path::new(path);
        let node = self.find(base)?;
        let child_path = |name: &str| Some(base.join(name).to_string_lossy().into_owned());
        let mut entries: Vec<TreeEntry> = node
            .dirs
            .iter()
            .map(|d| TreeEntry {
                name: d.name.clone(),
                path: child_path(&d.name),
                size: d.size,
                files: d.files,
                kind: EntryKind::Dir,
            })
            .chain(node.big_files.iter().map(|f| TreeEntry {
                name: f.name.clone(),
                path: child_path(&f.name),
                size: f.size,
                files: 1,
                kind: EntryKind::File,
            }))
            .collect();
        if node.small_count > 0 {
            entries.push(TreeEntry {
                name: String::new(),
                path: None,
                size: node.small_size,
                files: node.small_count,
                kind: EntryKind::SmallFiles,
            });
        }
        entries.sort_by_key(|e| std::cmp::Reverse(e.size));
        Some(entries)
    }

    /// Forgets a deleted file or folder and shrinks all of its parents.
    /// `size` is used for small files, which are not stored one by one.
    pub fn remove(&mut self, path: &str, size: u64) -> bool {
        let path = Path::new(path);
        for (root, node) in &mut self.roots {
            if let Ok(rel) = path.strip_prefix(&*root) {
                let names: Vec<String> =
                    rel.components().map(|c| c.as_os_str().to_string_lossy().into_owned()).collect();
                return !names.is_empty() && remove_in(node, &names, size).is_some();
            }
        }
        false
    }

    fn find(&self, path: &Path) -> Option<&DirNode> {
        let (root, top) = self.roots.iter().find(|(root, _)| path.starts_with(root))?;
        let mut node = top;
        for comp in path.strip_prefix(root).ok()?.components() {
            let name = comp.as_os_str().to_string_lossy();
            node = node.dirs.iter().find(|d| d.name == name)?;
        }
        Some(node)
    }
}

/// Removes `names` (a path relative to `node`) and returns the (bytes, files) it took away.
fn remove_in(node: &mut DirNode, names: &[String], size_hint: u64) -> Option<(u64, u64)> {
    let (first, rest) = names.split_first()?;
    let removed = if !rest.is_empty() {
        let child = node.dirs.iter_mut().find(|d| d.name == *first)?;
        remove_in(child, rest, size_hint)?
    } else if let Some(i) = node.dirs.iter().position(|d| d.name == *first) {
        let dir = node.dirs.swap_remove(i);
        (dir.size, dir.files)
    } else if let Some(i) = node.big_files.iter().position(|f| f.name == *first) {
        (node.big_files.swap_remove(i).size, 1)
    } else if node.small_count > 0 {
        let size = size_hint.min(node.small_size);
        node.small_size -= size;
        node.small_count -= 1;
        (size, 1)
    } else {
        return None;
    };
    node.size = node.size.saturating_sub(removed.0);
    node.files = node.files.saturating_sub(removed.1);
    Some(removed)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample() -> ScanTree {
        let movies = DirNode {
            name: "Movies".into(),
            size: 5_000,
            files: 2,
            big_files: vec![FileLeaf { name: "a.mkv".into(), size: 4_000 }],
            small_size: 1_000,
            small_count: 1,
            ..Default::default()
        };
        let home = DirNode { name: "neo".into(), size: 5_000, files: 2, dirs: vec![movies], ..Default::default() };
        ScanTree::new(vec![(PathBuf::from("/home/neo"), home)])
    }

    #[test]
    fn lists_children_largest_first() {
        let tree = sample();
        let roots = tree.children(None).unwrap();
        assert_eq!(roots[0].size, 5_000);
        let movies = tree.children(Some("/home/neo/Movies")).unwrap();
        assert_eq!(movies.len(), 2);
        assert_eq!(movies[0].kind, EntryKind::File);
        assert_eq!(movies[0].path.as_deref(), Some("/home/neo/Movies/a.mkv"));
        assert_eq!(movies[1].kind, EntryKind::SmallFiles);
        assert!(tree.children(Some("/home/neo/Nope")).is_none());
    }

    #[test]
    fn remove_updates_parents() {
        let mut tree = sample();
        assert!(tree.remove("/home/neo/Movies/a.mkv", 4_000));
        assert_eq!(tree.children(None).unwrap()[0].size, 1_000);
        assert!(tree.remove("/home/neo/Movies/tiny.txt", 1_000));
        assert_eq!(tree.children(None).unwrap()[0].size, 0);
        assert!(!tree.remove("/elsewhere/file", 10));
    }
}
