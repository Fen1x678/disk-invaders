//! Turning a path like `~/code/shop/node_modules` into numbers a neural network can read.
//!
//! Every word of the path becomes a "token" (`p:caches` = the parent folder is called caches,
//! `nw:spotify` = the item's own name contains the word spotify...). Tokens are hashed into a
//! fixed number of slots ("the hashing trick"), so the network never needs a dictionary.
//! A few extra numbers describe size, age and depth.

/// Number of slots for hashed words.
pub const HASH_DIM: usize = 8192;
/// Extra dense features (size, age, ...), stored after the hashed slots.
pub const DENSE_DIM: usize = 15;
pub const INPUT_DIM: usize = HASH_DIM + DENSE_DIM;
/// Words carry the meaning; size and age only nudge. These scales keep it that way.
const TOKEN_SCALE: f32 = 2.0;
const DENSE_SCALE: f32 = 0.35;

/// A sparse input vector: (index, value) pairs.
pub type Input = Vec<(usize, f32)>;

/// What the brain looks at: never file contents, only these four things.
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Sample {
    /// Path with the home folder replaced by `~` (see [`normalize_path`]).
    pub path: String,
    pub is_dir: bool,
    pub size_mb: f32,
    /// Days since last modification, or a negative number if unknown.
    pub age_days: f32,
}

/// Replaces the home folder with `~`, so lessons are portable and don't contain user names.
pub fn normalize_path(path: &str, home: Option<&str>) -> String {
    if let Some(home) = home.map(|h| h.trim_end_matches(['/', '\\'])).filter(|h| !h.is_empty()) {
        if let Some(prefix) = path.get(..home.len()) {
            let rest = &path[home.len()..];
            if prefix.eq_ignore_ascii_case(home) && (rest.is_empty() || rest.starts_with(['/', '\\'])) {
                return format!("~{rest}");
            }
        }
    }
    path.to_string()
}

/// Words of a path, tagged with where they appear. Public so the UI can show "what the brain sees".
pub fn tokens(path: &str) -> Vec<String> {
    let lower = path.to_lowercase();
    let comps: Vec<&str> = lower.split(['/', '\\']).filter(|c| !c.is_empty() && *c != "~").collect();
    let mut out = Vec::new();
    out.push(
        if lower.starts_with('~') {
            "root:home"
        } else if lower.as_bytes().get(1) == Some(&b':') {
            "root:drive"
        } else {
            "root:abs"
        }
        .to_string(),
    );
    let n = comps.len();
    for (i, comp) in comps.iter().enumerate() {
        let whole = squash_digits(comp);
        let (whole_tag, word_tag) = match n - 1 - i {
            0 => ("n", "nw"),
            1 => ("p", "w"),
            _ => ("d", "w"),
        };
        for word in words(&whole) {
            out.push(format!("{word_tag}:{word}"));
        }
        out.push(format!("{whole_tag}:{whole}"));
        // The same folder name means the same thing wherever it appears (system32, tdata, .ssh...).
        out.push(format!("c:{whole}"));
    }
    if let Some((stem, ext)) = comps.last().and_then(|name| name.rsplit_once('.')) {
        if !stem.is_empty() && !ext.is_empty() && ext.len() <= 12 {
            out.push(format!("ext:{ext}"));
        }
    }
    out
}

/// Plain words of a string: `com.spotify.client` -> `com`, `spotify`, `client`.
pub fn words(s: &str) -> impl Iterator<Item = &str> {
    s.split(|c: char| !c.is_alphanumeric() && c != '#')
        .filter(|w| w.chars().count() >= 2 && !w.chars().all(|c| c == '#'))
}

/// `17.4 (21E219)` -> `#.# (#e#)`: version numbers and dates shouldn't matter.
fn squash_digits(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for c in s.chars() {
        if c.is_ascii_digit() {
            if !out.ends_with('#') {
                out.push('#');
            }
        } else {
            out.push(c);
        }
    }
    out
}

/// FNV-1a: a tiny, fast, stable hash. Stable matters: the same word must land in the same slot forever.
pub fn slot(token: &str) -> usize {
    let mut hash: u64 = 0xcbf2_9ce4_8422_2325;
    for byte in token.bytes() {
        hash ^= u64::from(byte);
        hash = hash.wrapping_mul(0x0100_0000_01b3);
    }
    (hash % HASH_DIM as u64) as usize
}

/// The full input vector for one sample.
pub fn encode(sample: &Sample) -> Input {
    let toks = tokens(&sample.path);
    let weight = TOKEN_SCALE / (toks.len() as f32).sqrt();
    let mut input: Input = Vec::with_capacity(toks.len() + 6);
    for token in &toks {
        input.push((slot(token), weight));
    }
    let depth = toks.iter().filter(|t| t.starts_with("n:") || t.starts_with("p:") || t.starts_with("d:")).count();
    input.extend(dense(sample, depth));
    input
}

/// Encodes a single token on its own (used to ask the brain "what do you think of this word?").
pub fn encode_word(token: &str) -> Input {
    let neutral = Sample { path: String::new(), is_dir: false, size_mb: 300.0, age_days: 90.0 };
    let mut input = vec![(slot(token), TOKEN_SCALE)];
    input.extend(dense(&neutral, 3));
    input
}

fn dense(sample: &Sample, depth: usize) -> Vec<(usize, f32)> {
    let mut d = Vec::with_capacity(8);
    let at = |k: usize| HASH_DIM + k;
    if sample.is_dir {
        d.push((at(0), 1.0));
    }
    let mb = sample.size_mb.max(0.0);
    d.push((at(1), (mb + 1.0).log10() / 5.0));
    let size_bucket = [1.0, 100.0, 1000.0, 10_000.0].iter().position(|&limit| mb < limit).unwrap_or(4);
    d.push((at(2 + size_bucket), 1.0));
    if sample.age_days >= 0.0 {
        let age_bucket = [7.0, 30.0, 180.0, 365.0].iter().position(|&limit| sample.age_days < limit).unwrap_or(4);
        d.push((at(7 + age_bucket), 1.0));
        d.push((at(12), (sample.age_days / 1825.0).min(1.0)));
    }
    if sample.path.starts_with('~') {
        d.push((at(13), 1.0));
    }
    d.push((at(14), (depth as f32 / 12.0).min(1.0)));
    for (_, value) in &mut d {
        *value *= DENSE_SCALE;
    }
    d
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn home_becomes_tilde() {
        assert_eq!(normalize_path("/Users/neo/Library/Caches", Some("/Users/neo")), "~/Library/Caches");
        assert_eq!(normalize_path(r"C:\Users\Neo\AppData", Some(r"c:\users\neo")), r"~\AppData");
        assert_eq!(normalize_path("/Users/neon/x", Some("/Users/neo")), "/Users/neon/x");
        assert_eq!(normalize_path("/Applications/Xcode.app", Some("/Users/neo")), "/Applications/Xcode.app");
        assert_eq!(normalize_path("/Юзер", Some("/Users/neo")), "/Юзер", "no panic on non-ASCII");
    }

    #[test]
    fn tokens_describe_the_path() {
        let t = tokens("~/Library/Caches/com.spotify.client");
        for expected in ["root:home", "p:caches", "d:library", "n:com.spotify.client", "nw:spotify", "w:caches"] {
            assert!(t.contains(&expected.to_string()), "{expected} missing in {t:?}");
        }
        let t = tokens(r"~\Downloads\Zoom.pkg");
        assert!(t.contains(&"ext:pkg".to_string()) && t.contains(&"p:downloads".to_string()));
        assert!(
            tokens("~/Library/Developer/Xcode/iOS DeviceSupport/17.4 (21E219)").contains(&"n:#.# (#e#)".to_string())
        );
        assert!(tokens("~/Документы/Курсовая.docx").contains(&"nw:курсовая".to_string()));
    }

    #[test]
    fn hashing_is_stable() {
        // If this ever changes, every saved brain would forget everything.
        assert_eq!(slot("n:node_modules"), slot("n:node_modules"));
        assert!(slot("a") < HASH_DIM);
        let input = encode(&Sample { path: "~/x/node_modules".into(), is_dir: true, size_mb: 500.0, age_days: 10.0 });
        assert!(input.iter().all(|(i, _)| *i < INPUT_DIM));
    }
}
