//! The last line of defense: decides whether a path may be deleted at all.
//!
//! Every delete request goes through [`Guard::check`], no matter what the UI says.
//! When in doubt, block. A cleaner that breaks the OS gets uninstalled (and un-starred).

use crate::platform::{is_within, Platform};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Blocked {
    /// Relative path or a path with `..` in it.
    InvalidPath,
    /// A drive/filesystem root or a folder directly inside it (`/`, `C:\`, `/Users`, `C:\Windows`).
    TopLevel,
    /// Inside an operating system folder.
    System,
    /// The home folder itself, or one of its parents.
    Home,
    /// A standard user folder such as Documents or Desktop (its contents are fine, the folder is not).
    UserFolder,
    /// Keys, keychains, registry hives and similar.
    Sensitive,
}

impl Blocked {
    /// Stable code sent to the UI, which translates it.
    pub fn code(self) -> &'static str {
        match self {
            Blocked::InvalidPath => "invalid_path",
            Blocked::TopLevel => "top_level",
            Blocked::System => "system",
            Blocked::Home => "home",
            Blocked::UserFolder => "user_folder",
            Blocked::Sensitive => "sensitive",
        }
    }
}

const MAC_SYSTEM: &[&str] = &[
    "/System",
    "/bin",
    "/sbin",
    "/usr",
    "/etc",
    "/var",
    "/private",
    "/dev",
    "/Library/Apple",
    "/Library/Extensions",
    "/Library/Keychains",
    "/Library/LaunchAgents",
    "/Library/LaunchDaemons",
    "/Library/Security",
    "/Library/SystemExtensions",
];
const LINUX_SYSTEM: &[&str] = &[
    "/bin", "/sbin", "/usr", "/lib", "/lib32", "/lib64", "/libx32", "/etc", "/var", "/boot", "/dev", "/proc", "/sys",
    "/run", "/snap",
];
/// Places inside system folders that users legitimately clean up.
const UNIX_EXCEPTIONS: &[&str] = &["/usr/local", "/private/tmp", "/var/tmp"];
const WINDOWS_SYSTEM_DIRS: &[&str] = &[
    "Windows",
    "Program Files",
    "Program Files (x86)",
    "ProgramData",
    "Recovery",
    "Boot",
    "System Volume Information",
];

/// Folders (relative to home) that must never be deleted as a whole.
const USER_FOLDERS: &[&str] = &[
    "Desktop",
    "Documents",
    "Downloads",
    "Pictures",
    "Music",
    "Movies",
    "Videos",
    "Public",
    "Applications",
    "Library",
    "Library/Application Support",
    "Library/Caches",
    "Library/Logs",
    "Library/Containers",
    "Library/Group Containers",
    "Library/Mobile Documents",
    "Library/Developer",
    "AppData",
    "AppData/Local",
    "AppData/LocalLow",
    "AppData/Roaming",
    "AppData/Local/Temp",
    "OneDrive",
    "Dropbox",
    "Google Drive",
    ".config",
    ".local",
    ".local/share",
    ".cache",
    ".Trash",
];
/// Folders (relative to home) that must not be touched at all, including their contents.
const SENSITIVE: &[&str] = &[
    ".ssh",
    ".gnupg",
    ".aws",
    ".kube",
    ".password-store",
    "Library/Keychains",
    "AppData/Roaming/Microsoft/Crypto",
    "AppData/Roaming/Microsoft/Protect",
];

#[derive(Debug, Clone)]
pub struct Guard {
    platform: Platform,
    home: Option<Vec<String>>,
    system: Vec<Vec<String>>,
    exceptions: Vec<Vec<String>>,
    user_folders: Vec<Vec<String>>,
    sensitive: Vec<Vec<String>>,
}

impl Guard {
    /// A guard configured for the computer we are running on.
    pub fn for_this_computer() -> Self {
        let platform = Platform::current();
        let home = dirs::home_dir().map(|h| h.to_string_lossy().into_owned());
        let mut system_roots = Vec::new();
        if platform == Platform::Windows {
            for var in ["SystemRoot", "windir", "ProgramFiles", "ProgramFiles(x86)", "ProgramW6432", "ProgramData"] {
                if let Ok(value) = std::env::var(var) {
                    system_roots.push(value);
                }
            }
            let drive = std::env::var("SystemDrive").unwrap_or_else(|_| "C:".into());
            system_roots.extend(WINDOWS_SYSTEM_DIRS.iter().map(|d| format!(r"{drive}\{d}")));
        }
        Self::new(platform, home.as_deref(), &system_roots)
    }

    /// Builds a guard for any platform. Used by tests to check Windows rules on a Mac and vice versa.
    pub fn new(platform: Platform, home: Option<&str>, extra_system: &[String]) -> Self {
        let parse = |p: &str| platform.components(p);
        let (system, exceptions): (&[&str], &[&str]) = match platform {
            Platform::MacOs => (MAC_SYSTEM, UNIX_EXCEPTIONS),
            Platform::Linux => (LINUX_SYSTEM, UNIX_EXCEPTIONS),
            Platform::Windows => (&[], &[]),
        };
        let home_comps = home.map(parse);
        let under_home = |rel: &&str| home_comps.as_ref().map(|h| [h.clone(), parse(rel)].concat());

        Self {
            platform,
            system: system.iter().map(|p| parse(p)).chain(extra_system.iter().map(|p| parse(p))).collect(),
            exceptions: exceptions.iter().map(|p| parse(p)).collect(),
            user_folders: USER_FOLDERS.iter().filter_map(under_home).collect(),
            sensitive: SENSITIVE.iter().filter_map(under_home).collect(),
            home: home_comps,
        }
    }

    pub fn platform(&self) -> Platform {
        self.platform
    }

    /// `Ok(())` if the path may be deleted, otherwise the reason it is blocked.
    pub fn check(&self, path: &str) -> Result<(), Blocked> {
        if !self.platform.is_absolute(path) {
            return Err(Blocked::InvalidPath);
        }
        let comps = self.platform.components(path);
        if comps.iter().any(|c| c == "..") {
            return Err(Blocked::InvalidPath);
        }
        let top_level_depth = if self.platform == Platform::Windows { 2 } else { 1 };
        if comps.len() <= top_level_depth {
            return Err(Blocked::TopLevel);
        }
        if let Some(home) = &self.home {
            if is_within(home, &comps) {
                return Err(Blocked::Home);
            }
            let in_home_root = comps.len() == home.len() + 1 && is_within(&comps, home);
            if self.platform == Platform::Windows && in_home_root && comps[home.len()].starts_with("ntuser") {
                return Err(Blocked::Sensitive);
            }
        }
        if self.sensitive.iter().any(|s| is_within(&comps, s)) {
            return Err(Blocked::Sensitive);
        }
        if self.user_folders.contains(&comps) {
            return Err(Blocked::UserFolder);
        }
        // Inside a system folder, or a parent of one.
        if self.is_system(&comps) || self.system.iter().any(|s| is_within(s, &comps)) {
            return Err(Blocked::System);
        }
        Ok(())
    }

    /// `true` for paths inside OS folders. The scanner uses this to avoid suggesting them.
    pub fn is_system_path(&self, path: &str) -> bool {
        self.is_system(&self.platform.components(path))
    }

    fn is_system(&self, comps: &[String]) -> bool {
        self.system.iter().any(|s| is_within(comps, s))
            && !self.exceptions.iter().any(|e| comps.len() > e.len() && is_within(comps, e))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn mac() -> Guard {
        Guard::new(Platform::MacOs, Some("/Users/neo"), &[])
    }

    fn windows() -> Guard {
        let system = [r"C:\Windows", r"C:\Program Files", r"C:\ProgramData"].map(String::from);
        Guard::new(Platform::Windows, Some(r"C:\Users\Neo"), &system)
    }

    #[test]
    fn mac_allows_normal_junk() {
        let g = mac();
        assert_eq!(g.check("/Users/neo/Library/Caches/com.spotify.client"), Ok(()));
        assert_eq!(g.check("/Users/neo/code/app/node_modules"), Ok(()));
        assert_eq!(g.check("/Users/neo/Downloads/Zoom.pkg"), Ok(()));
        assert_eq!(g.check("/Applications/Old Game.app"), Ok(()));
        assert_eq!(g.check("/usr/local/Cellar/python@3.9"), Ok(()));
    }

    #[test]
    fn mac_blocks_dangerous_paths() {
        let g = mac();
        assert_eq!(g.check("/"), Err(Blocked::TopLevel));
        assert_eq!(g.check("/Users"), Err(Blocked::TopLevel));
        assert_eq!(g.check("/Applications"), Err(Blocked::TopLevel));
        assert_eq!(g.check("/Users/neo"), Err(Blocked::Home));
        assert_eq!(g.check("/users/NEO"), Err(Blocked::Home), "APFS ignores case");
        assert_eq!(g.check("/System/Library/Kernels"), Err(Blocked::System));
        assert_eq!(g.check("/usr/local"), Err(Blocked::System));
        assert_eq!(g.check("/usr/bin/python3"), Err(Blocked::System));
        assert_eq!(g.check("/Users/neo/Documents"), Err(Blocked::UserFolder));
        assert_eq!(g.check("/Users/neo/Library/Caches"), Err(Blocked::UserFolder));
        assert_eq!(g.check("/Users/neo/.ssh/id_ed25519"), Err(Blocked::Sensitive));
        assert_eq!(g.check("/Users/neo/Library/Keychains/login.keychain-db"), Err(Blocked::Sensitive));
        assert_eq!(g.check("Library/Caches/foo"), Err(Blocked::InvalidPath));
        assert_eq!(g.check("/Users/neo/Downloads/../../../System"), Err(Blocked::InvalidPath));
    }

    #[test]
    fn windows_rules() {
        let g = windows();
        assert_eq!(g.check(r"C:\Users\Neo\AppData\Local\Temp\setup123"), Ok(()));
        assert_eq!(g.check(r"D:\Games\Old\setup.exe"), Ok(()));
        assert_eq!(g.check(r"C:\"), Err(Blocked::TopLevel));
        assert_eq!(g.check(r"C:\pagefile.sys"), Err(Blocked::TopLevel));
        assert_eq!(g.check(r"D:\Movies"), Err(Blocked::TopLevel));
        assert_eq!(g.check(r"c:\windows\system32\drivers"), Err(Blocked::System));
        assert_eq!(g.check(r"C:\Program Files\Steam\steam.exe"), Err(Blocked::System));
        assert_eq!(g.check(r"C:\Users\neo"), Err(Blocked::Home));
        assert_eq!(g.check(r"C:\Users\Neo\NTUSER.DAT"), Err(Blocked::Sensitive));
        assert_eq!(g.check(r"C:\Users\Neo\AppData"), Err(Blocked::UserFolder));
        assert_eq!(g.check(r"Users\Neo\Downloads"), Err(Blocked::InvalidPath));
        assert!(g.is_system_path(r"C:\Windows\WinSxS"));
        assert!(!g.is_system_path(r"C:\Users\Neo"));
    }

    #[test]
    fn linux_is_case_sensitive() {
        let g = Guard::new(Platform::Linux, Some("/home/neo"), &[]);
        assert_eq!(g.check("/home/neo/.cache/pip"), Ok(()));
        assert_eq!(g.check("/home/neo/.cache"), Err(Blocked::UserFolder));
        assert_eq!(g.check("/home/NEO"), Ok(()), "different folder on a case-sensitive FS");
        assert_eq!(g.check("/var/tmp/big.iso"), Ok(()));
        assert_eq!(g.check("/var/lib/dpkg"), Err(Blocked::System));
    }
}
