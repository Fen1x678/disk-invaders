//! The walker: visits every folder in parallel, builds the size tree and collects junk.

use std::cmp::Reverse;
use std::collections::{BTreeMap, BinaryHeap, HashSet};
use std::fs::{self, Metadata};
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::time::{Instant, SystemTime, UNIX_EPOCH};

use rayon::prelude::*;

use crate::dupes::{self, Candidate};
use crate::model::{Category, CategorySummary, Item, Risk, ScanOptions, ScanReport, MB};
use crate::platform::Platform;
use crate::progress::{Phase, Progress};
use crate::rules::{self, KnownDirs, Markers, PathRule, Rules, Spec};
use crate::safety::Guard;
use crate::tree::{DirNode, FileLeaf, ScanTree, TREE_FILE_MIN};

/// Folders that are never entered: virtual file systems, other disks mounted inside the root,
/// and macOS system volume mirrors that would count the same bytes twice.
fn skipped_dirs(platform: Platform, roots: &[PathBuf]) -> HashSet<String> {
    let fixed: &[&str] = match platform {
        Platform::MacOs => &[
            "/System/Volumes",
            "/Volumes",
            "/dev",
            "/net",
            "/home",
            "/Network",
            "/.Spotlight-V100",
            "/.fseventsd",
            "/.DocumentRevisions-V100",
            "/.MobileBackups",
            "/.vol",
        ],
        Platform::Linux => &["/proc", "/sys", "/dev", "/run", "/snap", "/mnt", "/media", "/lost+found"],
        Platform::Windows => &[],
    };
    let mut skip: HashSet<String> = fixed.iter().map(|p| platform.key(p)).collect();
    if platform == Platform::Windows {
        for root in roots {
            skip.insert(platform.key(&root.join("System Volume Information").to_string_lossy()));
        }
    }
    skip
}

/// Scans `roots` on this computer. Blocks until done or cancelled via [`Progress::cancel`].
pub fn scan(roots: &[PathBuf], options: &ScanOptions, progress: &Progress) -> (ScanReport, ScanTree) {
    let rules = Rules::new(Platform::current(), &KnownDirs::for_this_computer());
    scan_with(roots, options, progress, &rules, &Guard::for_this_computer())
}

/// Same as [`scan`] with explicit rules and guard (handy for tests).
pub fn scan_with(
    roots: &[PathBuf],
    options: &ScanOptions,
    progress: &Progress,
    rules: &Rules,
    guard: &Guard,
) -> (ScanReport, ScanTree) {
    let started = Instant::now();
    let walker = Walker {
        options,
        progress,
        rules,
        guard,
        skip: skipped_dirs(rules.platform(), roots),
        now: unix_now(),
        items: Mutex::default(),
        large: Mutex::default(),
        candidates: Mutex::default(),
        seen_links: Mutex::default(),
    };

    // File systems are slow and threads are cheap: oversubscribe the CPU a little while waiting on I/O.
    let cores = std::thread::available_parallelism().map(|n| n.get()).unwrap_or(4);
    let pool = rayon::ThreadPoolBuilder::new()
        .num_threads((cores * 2).clamp(4, 32))
        .stack_size(32 * 1024 * 1024) // deeply nested node_modules are a thing
        .build()
        .expect("failed to start scanner threads");

    let nodes: Vec<(PathBuf, DirNode)> =
        pool.install(|| roots.par_iter().map(|root| (root.clone(), walker.scan_root(root))).collect());

    let mut items = std::mem::take(&mut *walker.items.lock().unwrap());
    items.extend(walker.large_items());
    if options.find_duplicates && !progress.is_cancelled() {
        progress.set_phase(Phase::Duplicates);
        let candidates = std::mem::take(&mut *walker.candidates.lock().unwrap());
        let groups = pool.install(|| dupes::find(candidates, progress));
        items.extend(dupes::to_items(groups));
    }
    let (summaries, items) = summarize(items, options.max_items_per_category, guard);
    progress.set_phase(Phase::Done);

    let report = ScanReport {
        roots: roots.iter().map(|r| r.to_string_lossy().into_owned()).collect(),
        files: nodes.iter().map(|(_, n)| n.files).sum(),
        dirs: progress.snapshot().dirs,
        bytes: nodes.iter().map(|(_, n)| n.size).sum(),
        errors: progress.snapshot().errors,
        elapsed_ms: started.elapsed().as_millis() as u64,
        cancelled: progress.is_cancelled(),
        summaries,
        items,
    };
    (report, ScanTree::new(nodes))
}

/// What the walker knows about the folder it is in.
#[derive(Debug, Clone, Copy, Default)]
struct Ctx {
    /// Inside something already reported as a whole (e.g. inside `node_modules`).
    claimed: bool,
    /// Every direct child of this folder is reported with this spec.
    each_child: Option<Spec>,
    is_downloads: bool,
    in_home: bool,
    in_app_data: bool,
    hidden: bool,
    bundle: bool,
    system: bool,
}

/// Results from one folder, merged into the shared lists in one go to keep locking rare.
#[derive(Default)]
struct Found {
    items: Vec<Item>,
    large: Vec<Large>,
    candidates: Vec<Candidate>,
}

/// (size, path, modified): ordered by size so a min-heap can keep the N biggest.
type Large = (u64, String, Option<i64>);

struct Walker<'a> {
    options: &'a ScanOptions,
    progress: &'a Progress,
    rules: &'a Rules,
    guard: &'a Guard,
    skip: HashSet<String>,
    now: i64,
    items: Mutex<Vec<Item>>,
    large: Mutex<BinaryHeap<Reverse<Large>>>,
    candidates: Mutex<Vec<Candidate>>,
    /// (device, inode) of hard-linked files already counted.
    #[cfg_attr(not(unix), allow(dead_code))]
    seen_links: Mutex<HashSet<(u64, u64)>>,
}

impl Walker<'_> {
    fn scan_root(&self, root: &Path) -> DirNode {
        let key = self.rules.key(root);
        let text = root.to_string_lossy();
        let comps = self.rules.platform().components(&text);
        let mut ctx = Ctx {
            is_downloads: self.rules.is_downloads_key(&key),
            in_home: self.rules.is_inside_home_key(&key),
            in_app_data: self.rules.is_inside_app_data_key(&key),
            hidden: self.rules.is_hidden_in_home_key(&key),
            bundle: comps.iter().any(|c| rules::is_bundle(c)),
            system: self.guard.is_system_path(&text),
            ..Ctx::default()
        };
        if let Some(PathRule::EachChild(spec)) = self.rules.rule_for_key(&key) {
            ctx.each_child = Some(spec);
        }
        self.scan_dir(root, text.into_owned(), ctx)
    }

    fn scan_dir(&self, path: &Path, name: String, ctx: Ctx) -> DirNode {
        let mut node = DirNode { name, ..DirNode::default() };
        if self.progress.is_cancelled() {
            return node;
        }
        self.progress.visit_dir(path);
        let Ok(entries) = fs::read_dir(path) else {
            self.progress.add_error();
            return node;
        };

        let mut markers = Markers::default();
        let mut subdirs = Vec::new();
        let mut found = Found::default();
        let (mut files, mut bytes) = (0u64, 0u64);

        for entry in entries {
            let Ok(entry) = entry else {
                self.progress.add_error();
                continue;
            };
            let Ok(file_type) = entry.file_type() else { continue };
            if file_type.is_symlink() {
                continue; // never follow links: no loops, no counting things twice
            }
            let file_name = entry.file_name().to_string_lossy().into_owned();
            if file_type.is_dir() {
                subdirs.push((entry.path(), file_name));
                continue;
            }
            let Ok(meta) = entry.metadata() else {
                self.progress.add_error();
                continue;
            };
            if !self.first_sighting(&meta) {
                continue;
            }
            let size = disk_size(&meta);
            files += 1;
            bytes += size;
            markers.observe(&file_name);
            self.classify_file(path, &file_name, size, &meta, &ctx, &mut found);
            if size >= TREE_FILE_MIN {
                node.big_files.push(FileLeaf { name: file_name, size });
            } else {
                node.small_size += size;
                node.small_count += 1;
            }
        }
        self.progress.add_files(files, bytes);
        self.flush(found);

        let children: Vec<_> = subdirs
            .into_iter()
            .filter_map(|(child_path, child_name)| {
                let key = self.rules.key(&child_path);
                if self.skip.contains(&key) {
                    return None;
                }
                let (child_ctx, claim) = self.child_ctx(&ctx, &child_path, &key, &child_name, &markers, &node.name);
                Some((child_path, child_name, child_ctx, claim))
            })
            .collect();

        node.dirs = children
            .into_par_iter()
            .map(|(child_path, child_name, child_ctx, claim)| {
                let child = self.scan_dir(&child_path, child_name, child_ctx);
                if let Some(spec) = claim {
                    let modified = fs::symlink_metadata(&child_path).ok().and_then(|m| mtime(&m));
                    let item = make_item(child_path.to_string_lossy().into_owned(), child.size, true, modified, spec);
                    self.items.lock().unwrap().push(item);
                }
                child
            })
            .collect();

        node.size = bytes + node.dirs.iter().map(|d| d.size).sum::<u64>();
        node.files = files + node.dirs.iter().map(|d| d.files).sum::<u64>();
        node
    }

    /// Works out the context of a subfolder and whether the subfolder itself is junk.
    fn child_ctx(
        &self,
        parent: &Ctx,
        path: &Path,
        key: &str,
        name: &str,
        markers: &Markers,
        parent_name: &str,
    ) -> (Ctx, Option<Spec>) {
        let mut ctx = Ctx {
            claimed: parent.claimed || parent.each_child.is_some(),
            each_child: None,
            is_downloads: self.rules.is_downloads_key(key),
            in_home: parent.in_home || self.rules.is_home_key(key),
            in_app_data: parent.in_app_data || self.rules.is_app_data_key(key),
            hidden: parent.hidden || name.starts_with('.'),
            bundle: parent.bundle || rules::is_bundle(name),
            system: parent.system || self.guard.is_system_path(&path.to_string_lossy()),
        };
        if let Some(spec) = parent.each_child {
            return (ctx, Some(spec));
        }
        if ctx.claimed || ctx.system || ctx.bundle {
            return (ctx, None);
        }
        match self.rules.rule_for_key(key) {
            Some(PathRule::Whole(spec)) => {
                ctx.claimed = true;
                return (ctx, Some(spec));
            }
            Some(PathRule::EachChild(spec)) => {
                ctx.each_child = Some(spec);
                return (ctx, None);
            }
            None => {}
        }
        if self.rules.platform() == Platform::Windows && parent_name.eq_ignore_ascii_case("$Recycle.Bin") {
            ctx.each_child = Some(Spec { category: Category::Trash, risk: Risk::Safe, label: "In the Recycle Bin" });
            return (ctx, None);
        }
        // Projects hidden in dot-folders (~/.vscode/extensions, ~/.nvm) belong to tools: hands off.
        if !parent.hidden {
            if let Some(mut spec) = rules::match_dir_name(name, markers, path, ctx.in_app_data) {
                // Outside home a node_modules is more likely an installed tool than a forgotten project.
                if !ctx.in_home && spec.category == Category::DevJunk {
                    spec.risk = Risk::Review;
                }
                ctx.claimed = true;
                return (ctx, Some(spec));
            }
        }
        if parent.is_downloads && self.is_old(fs::symlink_metadata(path).ok().as_ref().and_then(mtime)) {
            ctx.claimed = true;
            return (ctx, Some(old_download()));
        }
        (ctx, None)
    }

    fn classify_file(&self, dir: &Path, name: &str, size: u64, meta: &Metadata, ctx: &Ctx, found: &mut Found) {
        if ctx.system || ctx.bundle {
            return;
        }
        let modified = mtime(meta);
        let ext = extension(name);
        let full_path = || dir.join(name).to_string_lossy().into_owned();

        if let Some(spec) = ctx.each_child {
            found.items.push(make_item(full_path(), size, false, modified, spec));
        } else if !ctx.claimed {
            let installer = rules::INSTALLER_EXTENSIONS.contains(&ext.as_str())
                || (ctx.is_downloads && rules::DOWNLOAD_ONLY_INSTALLERS.contains(&ext.as_str()));
            let spec = if installer && (ctx.is_downloads || (ctx.in_home && !ctx.in_app_data && size >= MB)) {
                Some(Spec { category: Category::Installers, risk: Risk::Review, label: "Installer / disk image" })
            } else if ctx.is_downloads && self.is_old(modified) {
                Some(old_download())
            } else {
                rules::match_file(name, &ext, size)
            };
            if let Some(spec) = spec {
                found.items.push(make_item(full_path(), size, false, modified, spec));
            }
        }

        if size >= self.options.large_file_min_bytes {
            found.large.push((size, full_path(), modified));
        }
        let personal = ctx.in_home && !ctx.in_app_data && !ctx.hidden && !ctx.claimed;
        if self.options.find_duplicates && personal && size >= self.options.duplicate_min_bytes {
            found.candidates.push(Candidate { path: full_path(), len: meta.len(), size, modified });
        }
    }

    fn flush(&self, found: Found) {
        if !found.items.is_empty() {
            self.items.lock().unwrap().extend(found.items);
        }
        if !found.large.is_empty() {
            let keep = self.options.max_items_per_category.max(1);
            let mut heap = self.large.lock().unwrap();
            for large in found.large {
                heap.push(Reverse(large));
                if heap.len() > keep {
                    heap.pop();
                }
            }
        }
        if !found.candidates.is_empty() {
            self.candidates.lock().unwrap().extend(found.candidates);
        }
    }

    fn large_items(&self) -> Vec<Item> {
        let heap = std::mem::take(&mut *self.large.lock().unwrap());
        heap.into_iter()
            .map(|Reverse((size, path, modified))| {
                let label = rules::kind_of(&extension(&path));
                make_item(
                    path,
                    size,
                    false,
                    modified,
                    Spec { category: Category::LargeFiles, risk: Risk::Review, label },
                )
            })
            .collect()
    }

    fn is_old(&self, modified: Option<i64>) -> bool {
        let max_age = self.options.old_download_days as i64 * 86_400;
        modified.is_some_and(|m| self.now - m > max_age)
    }

    /// Hard links share their bytes: count each (device, inode) once.
    #[cfg(unix)]
    fn first_sighting(&self, meta: &Metadata) -> bool {
        use std::os::unix::fs::MetadataExt;
        meta.nlink() <= 1 || self.seen_links.lock().unwrap().insert((meta.dev(), meta.ino()))
    }

    #[cfg(not(unix))]
    fn first_sighting(&self, _meta: &Metadata) -> bool {
        true
    }
}

fn old_download() -> Spec {
    Spec { category: Category::OldDownloads, risk: Risk::Review, label: "Old download" }
}

fn make_item(path: String, size: u64, is_dir: bool, modified: Option<i64>, spec: Spec) -> Item {
    Item {
        path,
        size,
        is_dir,
        modified,
        category: spec.category,
        risk: spec.risk,
        label: spec.label.to_string(),
        group: None,
        brain: None,
    }
}

/// Sorts, filters and trims items per category. Anything the [`Guard`] would refuse is dropped here,
/// so the UI never even offers it.
fn summarize(items: Vec<Item>, max_per_category: usize, guard: &Guard) -> (Vec<CategorySummary>, Vec<Item>) {
    let mut by_category: BTreeMap<Category, Vec<Item>> = BTreeMap::new();
    for item in items.into_iter().filter(|i| guard.check(&i.path).is_ok()) {
        by_category.entry(item.category).or_default().push(item);
    }
    let mut summaries = Vec::new();
    let mut out = Vec::new();
    for category in Category::ALL {
        let mut list = by_category.remove(&category).unwrap_or_default();
        let bytes = if category == Category::Duplicates {
            list = dupes::drop_lonely(list);
            dupes::reclaimable(&list)
        } else {
            list.sort_by_key(|i| Reverse(i.size));
            list.iter().map(|i| i.size).sum()
        };
        let count = list.len();
        let cut = if category == Category::Duplicates {
            dupes::group_boundary(&list, max_per_category)
        } else {
            max_per_category
        };
        list.truncate(cut);
        summaries.push(CategorySummary { category, count, bytes, shown: list.len() });
        out.extend(list);
    }
    (summaries, out)
}

/// Bytes the file really occupies on disk (sparse files and cloud placeholders take less than their length).
#[cfg(unix)]
fn disk_size(meta: &Metadata) -> u64 {
    use std::os::unix::fs::MetadataExt;
    meta.blocks() * 512
}

#[cfg(windows)]
fn disk_size(meta: &Metadata) -> u64 {
    use std::os::windows::fs::MetadataExt;
    const FILE_ATTRIBUTE_OFFLINE: u32 = 0x1000;
    const FILE_ATTRIBUTE_RECALL_ON_DATA_ACCESS: u32 = 0x0040_0000;
    // OneDrive "online-only" files: listed, but not on this disk.
    if meta.file_attributes() & (FILE_ATTRIBUTE_OFFLINE | FILE_ATTRIBUTE_RECALL_ON_DATA_ACCESS) != 0 {
        0
    } else {
        meta.len()
    }
}

#[cfg(not(any(unix, windows)))]
fn disk_size(meta: &Metadata) -> u64 {
    meta.len()
}

fn mtime(meta: &Metadata) -> Option<i64> {
    let since_epoch = meta.modified().ok()?.duration_since(UNIX_EPOCH).ok()?;
    Some(since_epoch.as_secs() as i64)
}

fn unix_now() -> i64 {
    SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_secs() as i64).unwrap_or(0)
}

/// Lowercase extension without the dot. `".bashrc"` has none.
fn extension(name: &str) -> String {
    let file_name = name.rsplit(['/', '\\']).next().unwrap_or(name);
    match file_name.rsplit_once('.') {
        Some((stem, ext)) if !stem.is_empty() => ext.to_ascii_lowercase(),
        _ => String::new(),
    }
}

#[cfg(test)]
mod tests {
    use super::extension;

    #[test]
    fn extensions() {
        assert_eq!(extension("Movie.MKV"), "mkv");
        assert_eq!(extension("/a/b.c/archive.tar.gz"), "gz");
        assert_eq!(extension(".bashrc"), "");
        assert_eq!(extension("README"), "");
    }
}
