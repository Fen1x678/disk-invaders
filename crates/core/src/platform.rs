//! Platform-aware path comparison.
//!
//! Rules and safety checks compare paths as lists of components, so the same logic (and the
//! same unit tests) work for `C:\Users\Bob` and `/Users/bob` on any host OS.

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Platform {
    MacOs,
    Windows,
    Linux,
}

impl Platform {
    pub fn current() -> Self {
        if cfg!(target_os = "macos") {
            Platform::MacOs
        } else if cfg!(windows) {
            Platform::Windows
        } else {
            Platform::Linux
        }
    }

    /// Default file systems on macOS (APFS) and Windows (NTFS) ignore case.
    pub fn case_insensitive(self) -> bool {
        matches!(self, Platform::MacOs | Platform::Windows)
    }

    pub fn is_absolute(self, path: &str) -> bool {
        match self {
            Platform::Windows => {
                let b = path.as_bytes();
                path.starts_with(r"\\")
                    || (b.len() >= 3 && b[0].is_ascii_alphabetic() && b[1] == b':' && (b[2] == b'\\' || b[2] == b'/'))
            }
            _ => path.starts_with('/'),
        }
    }

    /// Splits a path into normalized components (lowercased on case-insensitive platforms).
    /// `C:\Users\Bob` -> `["c:", "users", "bob"]`, `/Users/bob` -> `["users", "bob"]`.
    pub fn components(self, path: &str) -> Vec<String> {
        let is_sep = |c: char| c == '/' || (self == Platform::Windows && c == '\\');
        path.split(is_sep)
            .filter(|c| !c.is_empty() && *c != ".")
            .map(|c| if self.case_insensitive() { c.to_lowercase() } else { c.to_string() })
            .collect()
    }

    /// A normalized string key for a path, suitable for hash lookups.
    pub fn key(self, path: &str) -> String {
        self.components(path).join("/")
    }
}

/// `true` if `inner` equals `outer` or lives somewhere below it.
pub fn is_within(inner: &[String], outer: &[String]) -> bool {
    inner.len() >= outer.len() && inner[..outer.len()] == *outer
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn windows_components_ignore_case_and_separators() {
        let p = Platform::Windows;
        assert_eq!(p.components(r"C:\Users\Bob/Downloads\"), ["c:", "users", "bob", "downloads"]);
        assert_eq!(p.key(r"C:\Users\Bob"), p.key("c:/users/BOB"));
        assert!(p.is_absolute(r"D:\games"));
        assert!(p.is_absolute(r"\\nas\share"));
        assert!(!p.is_absolute(r"games\doom"));
    }

    #[test]
    fn linux_is_case_sensitive_and_keeps_backslashes() {
        let p = Platform::Linux;
        assert_ne!(p.key("/home/Bob"), p.key("/home/bob"));
        assert_eq!(p.components(r"/tmp/odd\name"), ["tmp", r"odd\name"]);
    }

    #[test]
    fn within() {
        let p = Platform::MacOs;
        let home = p.components("/Users/bob");
        assert!(is_within(&p.components("/Users/Bob/Library"), &home));
        assert!(is_within(&home, &home));
        assert!(!is_within(&p.components("/Users/bobby"), &home));
        assert!(!is_within(&p.components("/Users"), &home));
    }
}
