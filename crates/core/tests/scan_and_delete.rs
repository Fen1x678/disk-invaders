//! End-to-end: build a fake home folder full of junk, scan it, delete some of it.

use std::fs;
use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime};

use invaders_core::rules::{KnownDirs, Rules};
use invaders_core::scanner::scan_with;
use invaders_core::{delete_paths, Category, Guard, Phase, Platform, Progress, ScanOptions};

const MB: usize = 1024 * 1024;

fn s(p: &Path) -> String {
    p.to_string_lossy().into_owned()
}

/// Writes `len` bytes; files with the same `seed` and length are identical.
fn write(path: &Path, len: usize, seed: u8) {
    fs::create_dir_all(path.parent().unwrap()).unwrap();
    let data: Vec<u8> = (0..len).map(|i| (i as u8).wrapping_mul(31).wrapping_add(seed)).collect();
    fs::write(path, data).unwrap();
}

/// A temp folder outside the OS temp dir (`/var/folders` counts as a system folder on macOS).
fn sandbox() -> tempfile::TempDir {
    tempfile::Builder::new().prefix("sandbox").tempdir_in(env!("CARGO_TARGET_TMPDIR")).unwrap()
}

struct FakeHome {
    _dir: tempfile::TempDir,
    home: PathBuf,
    rules: Rules,
    guard: Guard,
}

fn fake_home() -> FakeHome {
    let dir = sandbox();
    let home = dir.path().join("neo");
    write(&home.join("code/web/package.json"), 10, 1);
    write(&home.join("code/web/node_modules/react/index.js"), 2 * MB, 2);
    write(&home.join("code/game/Cargo.toml"), 10, 3);
    write(&home.join("code/game/target/debug/game"), 3 * MB, 4);
    write(&home.join("code/notes/target/keep.txt"), 10, 5); // no Cargo.toml: not junk
    write(&home.join(".vscode/extensions/x/package.json"), 10, 6);
    write(&home.join(".vscode/extensions/x/node_modules/a.js"), MB, 7); // belongs to an extension
    write(&home.join("Downloads/Zoom.pkg"), 2 * MB, 8);
    write(&home.join("Downloads/ancient.zip"), MB, 9);
    write(&home.join("Downloads/fresh.pdf"), MB, 10);
    write(&home.join("Movies/trip.mov"), 5 * MB, 11);
    write(&home.join("Movies/trip copy.mov"), 5 * MB, 11);
    write(&home.join(".cache/pip/http/blob"), MB, 12);
    write(&home.join("server.log"), 2 * MB, 13);

    let old = SystemTime::now() - Duration::from_secs(400 * 86_400);
    fs::File::options().write(true).open(home.join("Downloads/ancient.zip")).unwrap().set_modified(old).unwrap();

    let platform = Platform::current();
    let known = KnownDirs { home: Some(s(&home)), downloads: Some(s(&home.join("Downloads"))), ..Default::default() };
    FakeHome { rules: Rules::new(platform, &known), guard: Guard::new(platform, Some(&s(&home)), &[]), home, _dir: dir }
}

#[test]
fn finds_every_kind_of_junk() {
    let fake = fake_home();
    let options = ScanOptions { large_file_min_bytes: 4 * MB as u64, ..Default::default() };
    let progress = Progress::new();
    let (report, tree) = scan_with(std::slice::from_ref(&fake.home), &options, &progress, &fake.rules, &fake.guard);

    let rel = |p: &str| s(Path::new(p).strip_prefix(&fake.home).unwrap()).replace('\\', "/");
    let found = |category: Category| {
        let mut paths: Vec<String> =
            report.items.iter().filter(|i| i.category == category).map(|i| rel(&i.path)).collect();
        paths.sort();
        paths
    };

    assert_eq!(found(Category::DevJunk), ["code/game/target", "code/web/node_modules"]);
    assert_eq!(found(Category::Installers), ["Downloads/Zoom.pkg"]);
    assert_eq!(found(Category::OldDownloads), ["Downloads/ancient.zip"]);
    assert_eq!(found(Category::Duplicates), ["Movies/trip copy.mov", "Movies/trip.mov"]);
    assert_eq!(found(Category::LargeFiles), ["Movies/trip copy.mov", "Movies/trip.mov"]);
    assert_eq!(found(Category::Caches), [".cache/pip"]);
    assert_eq!(found(Category::Logs), ["server.log"]);

    let dev_junk = report.summaries.iter().find(|s| s.category == Category::DevJunk).unwrap();
    assert!(dev_junk.bytes >= 5 * MB as u64, "folder items carry the size of everything inside");
    let dupes = report.summaries.iter().find(|s| s.category == Category::Duplicates).unwrap();
    assert!(dupes.bytes >= 5 * MB as u64 && dupes.bytes < 6 * MB as u64, "one copy is kept");

    assert_eq!(report.files, 14);
    assert_eq!(report.errors, 0);
    assert!(!report.cancelled);
    assert_eq!(progress.phase(), Phase::Done);
    let root = &tree.children(None).unwrap()[0];
    assert_eq!(root.size, report.bytes);
    let top = tree.children(Some(&s(&fake.home))).unwrap();
    assert_eq!(top[0].name, "Movies", "largest folder first");
}

#[test]
fn cancelled_scan_stops_early() {
    let fake = fake_home();
    let progress = Progress::new();
    progress.cancel();
    let (report, _) =
        scan_with(std::slice::from_ref(&fake.home), &ScanOptions::default(), &progress, &fake.rules, &fake.guard);
    assert!(report.cancelled);
    assert_eq!(report.files, 0);
}

#[test]
fn permanent_delete_respects_the_guard() {
    let fake = fake_home();
    let junk = fake.home.join("code/web/node_modules");
    let missing = fake.home.join("nope");
    let outcomes = delete_paths(&[s(&junk), s(&fake.home), s(&missing)], true, &fake.guard);

    assert!(outcomes[0].ok, "{:?}", outcomes[0]);
    assert!(!junk.exists());
    assert_eq!(outcomes[1].error.as_deref(), Some("blocked:home"));
    assert!(fake.home.exists());
    assert_eq!(outcomes[2].error.as_deref(), Some("not_found"));
}
