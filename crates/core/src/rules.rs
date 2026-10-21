//! Junk rules: how the scanner recognises caches, build output, logs and friends.
//!
//! Want to teach Disk Invaders a new kind of junk? This is the file. Most rules are one line:
//! add a match arm to [`match_dir_name`] or an entry to [`Rules::new`], then add a test below.

use std::collections::HashMap;
use std::path::Path;

use crate::model::{Category, Risk};
use crate::platform::Platform;

/// What to call a match and how to treat it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Spec {
    pub category: Category,
    pub risk: Risk,
    pub label: &'static str,
}

const fn spec(category: Category, risk: Risk, label: &'static str) -> Spec {
    Spec { category, risk, label }
}

const fn dev(label: &'static str) -> Spec {
    spec(Category::DevJunk, Risk::Safe, label)
}

/// How a known folder is reported.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PathRule {
    /// The folder is a single item.
    Whole(Spec),
    /// Every direct child of the folder is a separate item (e.g. one entry per app in `~/Library/Caches`).
    EachChild(Spec),
}

/// Project marker files seen in a folder. A `target` folder is only junk next to a `Cargo.toml`.
#[derive(Debug, Default, Clone, Copy)]
pub struct Markers {
    pub cargo: bool,
    pub gradle: bool,
    pub maven: bool,
    pub podfile: bool,
    pub dotnet: bool,
    pub mix: bool,
    pub pubspec: bool,
    pub package_json: bool,
}

impl Markers {
    pub fn observe(&mut self, file_name: &str) {
        match file_name {
            "Cargo.toml" => self.cargo = true,
            "build.gradle" | "build.gradle.kts" | "settings.gradle" | "settings.gradle.kts" => self.gradle = true,
            "pom.xml" => self.maven = true,
            "Podfile" => self.podfile = true,
            "mix.exs" => self.mix = true,
            "pubspec.yaml" => self.pubspec = true,
            "package.json" => self.package_json = true,
            name if [".csproj", ".fsproj", ".vbproj", ".sln"].iter().any(|ext| name.ends_with(ext)) => {
                self.dotnet = true
            }
            _ => {}
        }
    }
}

/// Recognises junk folders by name. `markers` describes the parent folder.
///
/// Inside app data (`~/Library`, `AppData`, `~/.config`) only cache folders count: a `node_modules`
/// there belongs to an installed app, and deleting it would break that app.
pub fn match_dir_name(name: &str, markers: &Markers, path: &Path, in_app_data: bool) -> Option<Spec> {
    if in_app_data {
        return match name {
            "Cache" | "Code Cache" | "GPUCache" | "DawnCache" | "DawnGraphiteCache" | "DawnWebGPUCache"
            | "GrShaderCache" | "ShaderCache" | "CacheStorage" | "ScriptCache" => {
                Some(spec(Category::Caches, Risk::Safe, "Browser / Electron app cache"))
            }
            _ => None,
        };
    }
    let found = match name {
        "node_modules" if markers.package_json => dev("node_modules (npm / yarn / pnpm)"),
        "target" if markers.cargo => dev("Rust build output"),
        "target" if markers.maven => dev("Maven build output"),
        "build" if markers.gradle => dev("Gradle build output"),
        ".gradle" if markers.gradle => dev("Gradle project cache"),
        "build" if markers.pubspec => dev("Flutter build output"),
        ".dart_tool" => dev("Dart tool cache"),
        "__pycache__" => dev("Python bytecode cache"),
        ".pytest_cache" | ".mypy_cache" | ".ruff_cache" | ".tox" | ".nox" => dev("Python tool cache"),
        ".venv" | "venv" | "env" | ".virtualenv" if path.join("pyvenv.cfg").is_file() => {
            spec(Category::DevJunk, Risk::Review, "Python virtualenv")
        }
        ".next" | ".nuxt" | ".svelte-kit" | ".parcel-cache" | ".turbo" | ".angular" | ".expo" | ".docusaurus" => {
            dev("JavaScript framework cache")
        }
        "Pods" if markers.podfile => dev("CocoaPods dependencies"),
        "bin" | "obj" if markers.dotnet => dev(".NET build output"),
        "_build" | "deps" if markers.mix => dev("Elixir build output"),
        ".stack-work" | "dist-newstyle" => dev("Haskell build output"),
        "zig-cache" | ".zig-cache" => dev("Zig build cache"),
        ".terraform" => dev("Terraform providers"),
        n if n.starts_with("cmake-build-") => dev("CMake build (CLion)"),
        _ => return None,
    };
    Some(found)
}

/// Installer and disk image extensions. Most people never delete these after installing.
pub const INSTALLER_EXTENSIONS: &[&str] = &["dmg", "pkg", "mpkg", "msi", "msix", "iso", "xip"];
/// Executable installers: only reported inside Downloads, they might be portable apps elsewhere.
pub const DOWNLOAD_ONLY_INSTALLERS: &[&str] = &["exe", "appimage", "deb", "rpm", "apk"];

/// Classifies loose files (logs, dumps, temp files) that are worth showing.
pub fn match_file(name: &str, ext: &str, size: u64) -> Option<Spec> {
    const MB: u64 = 1024 * 1024;
    let logs = Category::Logs;
    match ext {
        "log" if size >= MB => Some(spec(logs, Risk::Safe, "Log file")),
        "dmp" | "mdmp" | "hprof" | "crash" | "ips" if size >= 256 * 1024 => {
            Some(spec(logs, Risk::Safe, "Crash / memory dump"))
        }
        "tmp" | "temp" if size >= MB => Some(spec(logs, Risk::Safe, "Temporary file")),
        _ if name.starts_with("core.") && size >= MB && ext.chars().all(|c| c.is_ascii_digit()) => {
            Some(spec(logs, Risk::Safe, "Core dump"))
        }
        _ => None,
    }
}

/// A friendly name for a large file based on its extension.
pub fn kind_of(ext: &str) -> &'static str {
    match ext {
        "mp4" | "mov" | "mkv" | "avi" | "m4v" | "webm" | "wmv" | "mts" | "braw" | "r3d" | "prores" => "Video",
        "zip" | "rar" | "7z" | "tar" | "gz" | "tgz" | "bz2" | "xz" | "zst" => "Archive",
        "dmg" | "iso" | "img" | "pkg" | "msi" | "xip" => "Disk image / installer",
        "vdi" | "vmdk" | "vhd" | "vhdx" | "qcow2" | "raw" | "utm" | "hdd" | "ova" => "Virtual machine disk",
        "wav" | "flac" | "aiff" | "aif" | "mp3" | "m4a" => "Audio",
        "psd" | "psb" | "tif" | "tiff" | "dng" | "cr2" | "cr3" | "nef" | "arw" | "heic" | "png" | "jpg" | "jpeg" => {
            "Image"
        }
        "pdf" | "docx" | "pptx" | "xlsx" | "key" => "Document",
        "safetensors" | "gguf" | "ckpt" | "pt" | "pth" | "onnx" | "bin" => "AI model / binary blob",
        "sqlite" | "db" | "sqlite3" => "Database",
        "log" => "Log",
        _ => "File",
    }
}

/// Folder extensions that are really "documents" (app bundles, photo libraries).
/// Files inside them are never suggested one by one: deleting one would corrupt the whole thing.
pub fn is_bundle(name: &str) -> bool {
    const BUNDLES: &[&str] = &[
        ".app",
        ".framework",
        ".photoslibrary",
        ".musiclibrary",
        ".tvlibrary",
        ".fcpbundle",
        ".imovielibrary",
        ".logicx",
        ".band",
        ".lrdata",
        ".aplibrary",
        ".xcarchive",
        ".bundle",
        ".plugin",
        ".kext",
    ];
    let lower = name.to_ascii_lowercase();
    BUNDLES.iter().any(|ext| lower.ends_with(ext))
}

/// Well-known places on this computer.
#[derive(Debug, Clone, Default)]
pub struct KnownDirs {
    pub home: Option<String>,
    pub downloads: Option<String>,
    /// Windows `%LOCALAPPDATA%`.
    pub local_app_data: Option<String>,
    /// Windows `%APPDATA%`.
    pub roaming_app_data: Option<String>,
    pub temp: Option<String>,
}

impl KnownDirs {
    pub fn for_this_computer() -> Self {
        let s = |p: std::path::PathBuf| p.to_string_lossy().into_owned();
        let env = |var: &str| std::env::var(var).ok().filter(|v| !v.is_empty());
        let windows = cfg!(windows);
        let downloads = dirs::download_dir().or_else(|| dirs::home_dir().map(|h| h.join("Downloads")));
        Self {
            home: dirs::home_dir().map(s),
            downloads: downloads.map(s),
            local_app_data: if windows { env("LOCALAPPDATA") } else { None },
            roaming_app_data: if windows { env("APPDATA") } else { None },
            temp: if windows { env("TEMP") } else { None },
        }
    }
}

/// Absolute-path rules, built once per scan for the current platform.
pub struct Rules {
    platform: Platform,
    by_path: HashMap<String, PathRule>,
    home: Option<String>,
    downloads: Option<String>,
    app_data_roots: Vec<String>,
}

impl Rules {
    pub fn new(platform: Platform, dirs: &KnownDirs) -> Self {
        use Category::*;
        use PathRule::{EachChild, Whole};
        let mut rules: Vec<(String, PathRule)> = Vec::new();
        let mut add = |base: &Option<String>, rel: &str, rule: PathRule| {
            if let Some(base) = base {
                rules.push((format!("{base}/{rel}"), rule));
            }
        };
        let home = &dirs.home;
        let cache = |label| spec(Caches, Risk::Safe, label);

        // Everywhere
        add(home, ".npm/_cacache", Whole(cache("npm cache")));
        add(home, ".yarn/berry/cache", Whole(cache("Yarn cache")));
        add(home, ".pnpm-store", Whole(cache("pnpm store")));
        add(home, ".gradle/caches", Whole(cache("Gradle cache")));
        add(home, ".m2/repository", Whole(cache("Maven repository cache")));
        add(home, ".cargo/registry", Whole(cache("Cargo registry cache")));
        add(home, "go/pkg/mod", Whole(cache("Go module cache")));
        add(home, ".nuget/packages", Whole(cache("NuGet package cache")));
        add(home, ".pub-cache", Whole(cache("Dart pub cache")));
        add(home, ".android/avd", EachChild(spec(DevJunk, Risk::Review, "Android emulator")));
        add(home, ".cache", EachChild(cache("Cache (~/.cache)")));

        match platform {
            Platform::MacOs => {
                add(home, ".Trash", EachChild(spec(Trash, Risk::Safe, "In the Trash")));
                add(home, "Library/Caches", EachChild(cache("App cache")));
                add(home, "Library/Logs", EachChild(spec(Logs, Risk::Safe, "App logs")));
                add(home, "Library/Developer/Xcode/DerivedData", Whole(dev("Xcode DerivedData")));
                add(home, "Library/Developer/Xcode/iOS DeviceSupport", EachChild(dev("iOS DeviceSupport")));
                add(home, "Library/Developer/Xcode/watchOS DeviceSupport", EachChild(dev("watchOS DeviceSupport")));
                add(home, "Library/Developer/Xcode/Archives", EachChild(spec(DevJunk, Risk::Review, "Xcode archive")));
                add(home, "Library/Developer/CoreSimulator/Caches", Whole(dev("iOS Simulator cache")));
                add(
                    home,
                    "Library/Application Support/MobileSync/Backup",
                    EachChild(spec(Backups, Risk::Review, "iPhone / iPad backup")),
                );
                rules.push(("/cores".into(), EachChild(spec(Logs, Risk::Safe, "Core dump"))));
            }
            Platform::Windows => {
                let local = &dirs.local_app_data;
                add(&dirs.temp, "", EachChild(spec(Logs, Risk::Safe, "Temporary files")));
                add(local, "CrashDumps", EachChild(spec(Logs, Risk::Safe, "Crash dump")));
                add(local, "npm-cache", Whole(cache("npm cache")));
                add(local, "pip/Cache", Whole(cache("pip cache")));
                add(local, "Yarn/Cache", Whole(cache("Yarn cache")));
                add(local, "Microsoft/Windows/INetCache", Whole(cache("Internet cache")));
                add(local, "D3DSCache", Whole(cache("DirectX shader cache")));
                add(local, "NVIDIA/DXCache", Whole(cache("NVIDIA shader cache")));
                add(local, "NVIDIA/GLCache", Whole(cache("NVIDIA shader cache")));
                add(local, "AMD/DxCache", Whole(cache("AMD shader cache")));
                add(
                    &dirs.roaming_app_data,
                    "Apple Computer/MobileSync/Backup",
                    EachChild(spec(Backups, Risk::Review, "iPhone / iPad backup")),
                );
            }
            Platform::Linux => {
                add(home, ".local/share/Trash/files", EachChild(spec(Trash, Risk::Safe, "In the Trash")));
            }
        }

        let key = |p: &str| platform.key(p);
        let mut app_data_roots: Vec<String> = match platform {
            Platform::MacOs => ["Library"].iter().filter_map(|r| home.as_ref().map(|h| format!("{h}/{r}"))).collect(),
            Platform::Windows => ["AppData"].iter().filter_map(|r| home.as_ref().map(|h| format!("{h}/{r}"))).collect(),
            Platform::Linux => {
                [".config", ".local"].iter().filter_map(|r| home.as_ref().map(|h| format!("{h}/{r}"))).collect()
            }
        };
        app_data_roots.extend(dirs.local_app_data.iter().chain(&dirs.roaming_app_data).cloned());

        Self {
            platform,
            by_path: rules.iter().map(|(p, r)| (key(p), *r)).collect(),
            home: home.as_deref().map(key),
            downloads: dirs.downloads.as_deref().map(key),
            app_data_roots: app_data_roots.iter().map(|p| key(p)).collect(),
        }
    }

    pub fn platform(&self) -> Platform {
        self.platform
    }

    /// Normalized key for a path; pass it to the `*_key` methods below.
    pub fn key(&self, path: &Path) -> String {
        self.platform.key(&path.to_string_lossy())
    }

    pub fn rule_for_key(&self, key: &str) -> Option<PathRule> {
        self.by_path.get(key).copied()
    }

    pub fn is_home_key(&self, key: &str) -> bool {
        self.home.as_deref() == Some(key)
    }

    pub fn is_downloads_key(&self, key: &str) -> bool {
        self.downloads.as_deref() == Some(key)
    }

    pub fn is_app_data_key(&self, key: &str) -> bool {
        self.app_data_roots.iter().any(|root| root == key)
    }

    /// Like [`Rules::is_home_key`], but also `true` for anything inside home. Used for scan roots.
    pub fn is_inside_home_key(&self, key: &str) -> bool {
        self.home.as_deref().is_some_and(|home| inside(key, home))
    }

    /// `true` if `key` is inside a dot-folder somewhere below home (e.g. `~/.vscode/extensions`).
    pub fn is_hidden_in_home_key(&self, key: &str) -> bool {
        match &self.home {
            Some(home) if inside(key, home) => key[home.len()..].split('/').any(|c| c.starts_with('.')),
            _ => false,
        }
    }

    pub fn is_inside_app_data_key(&self, key: &str) -> bool {
        self.app_data_roots.iter().any(|root| inside(key, root))
    }
}

fn inside(key: &str, base: &str) -> bool {
    key == base || (key.starts_with(base) && key.as_bytes().get(base.len()) == Some(&b'/'))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn markers(files: &[&str]) -> Markers {
        let mut m = Markers::default();
        files.iter().for_each(|f| m.observe(f));
        m
    }

    #[test]
    fn build_folders_need_their_project_file() {
        let p = Path::new("/x");
        assert!(match_dir_name("target", &markers(&["Cargo.toml"]), p, false).is_some());
        assert!(match_dir_name("target", &markers(&["notes.txt"]), p, false).is_none());
        assert!(match_dir_name("build", &markers(&["build.gradle.kts"]), p, false).is_some());
        assert!(match_dir_name("bin", &markers(&["Game.csproj"]), p, false).is_some());
        assert!(match_dir_name("bin", &markers(&[]), p, false).is_none());
        let js = markers(&["package.json"]);
        assert_eq!(match_dir_name("node_modules", &js, p, false).unwrap().category, Category::DevJunk);
        assert!(match_dir_name("node_modules", &markers(&[]), p, false).is_none(), "global npm folder");
    }

    #[test]
    fn app_data_only_has_caches() {
        let p = Path::new("/x");
        assert!(match_dir_name("Code Cache", &Markers::default(), p, true).is_some());
        assert!(match_dir_name("Cache", &Markers::default(), p, false).is_none());
        let electron_app = markers(&["package.json"]);
        assert!(match_dir_name("node_modules", &electron_app, p, true).is_none(), "would break the app");
    }

    #[test]
    fn virtualenv_needs_pyvenv_cfg() {
        let dir = tempfile::tempdir().unwrap();
        let venv = dir.path().join(".venv");
        std::fs::create_dir(&venv).unwrap();
        assert!(match_dir_name(".venv", &Markers::default(), &venv, false).is_none());
        std::fs::write(venv.join("pyvenv.cfg"), "home = /usr/bin").unwrap();
        assert_eq!(match_dir_name(".venv", &Markers::default(), &venv, false).unwrap().risk, Risk::Review);
    }

    #[test]
    fn loose_files() {
        assert_eq!(match_file("server.log", "log", 5 << 20).unwrap().label, "Log file");
        assert!(match_file("server.log", "log", 10).is_none(), "tiny logs are not worth a row");
        assert_eq!(match_file("java_pid42.hprof", "hprof", 900 << 20).unwrap().label, "Crash / memory dump");
        assert_eq!(match_file("core.12345", "12345", 2 << 20).unwrap().label, "Core dump");
        assert!(match_file("core.js", "js", 2 << 20).is_none());
    }

    #[test]
    fn path_rules_are_platform_specific_and_case_insensitive() {
        let mac = Rules::new(
            Platform::MacOs,
            &KnownDirs {
                home: Some("/Users/neo".into()),
                downloads: Some("/Users/neo/Downloads".into()),
                ..Default::default()
            },
        );
        let key = mac.key(Path::new("/Users/Neo/Library/Caches"));
        assert!(matches!(mac.rule_for_key(&key), Some(PathRule::EachChild(s)) if s.category == Category::Caches));
        assert!(mac.is_downloads_key(&mac.key(Path::new("/users/neo/downloads"))));
        assert!(mac.is_app_data_key(&mac.key(Path::new("/Users/neo/Library"))));

        let win = Rules::new(
            Platform::Windows,
            &KnownDirs {
                home: Some(r"C:\Users\Neo".into()),
                local_app_data: Some(r"C:\Users\Neo\AppData\Local".into()),
                temp: Some(r"C:\Users\Neo\AppData\Local\Temp".into()),
                ..Default::default()
            },
        );
        let temp = win.key(Path::new(r"C:\Users\Neo\AppData\Local\Temp"));
        assert!(matches!(win.rule_for_key(&temp), Some(PathRule::EachChild(s)) if s.category == Category::Logs));
        let npm = win.key(Path::new(r"C:\Users\Neo\AppData\Local\npm-cache"));
        assert!(matches!(win.rule_for_key(&npm), Some(PathRule::Whole(_))));
        assert!(win.rule_for_key(&win.key(Path::new("/Users/Neo/Library/Caches"))).is_none());
    }

    #[test]
    fn bundles() {
        assert!(is_bundle("Xcode.app"));
        assert!(is_bundle("Photos Library.photoslibrary"));
        assert!(!is_bundle("apps"));
    }
}
