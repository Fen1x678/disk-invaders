//! # INVADER BRAIN
//!
//! A tiny neural network that learns what junk looks like. It never sees file contents:
//! only the path, whether it's a folder, its size and its age ([`Sample`]).
//!
//! It learns from two places:
//! - **school**: `brain/dataset.csv` in the repository, labeled examples anyone can improve with a PR,
//! - **you**: every time you delete or keep something, it becomes a personal lesson (stored locally).
//!
//! Training takes milliseconds, so the brain is simply retrained whenever it learns something new.

mod features;
mod net;

use std::collections::{HashMap, HashSet};
use std::io::Write;
use std::path::Path;

use serde::{Deserialize, Serialize};

use features::{encode, encode_word};
pub use features::{normalize_path, tokens, Sample};
use net::{Example, Mlp};

/// The school book, compiled into the app.
pub const SCHOOL_CSV: &str = include_str!("../../../../brain/dataset.csv");
/// The final exam: paths that are *not* in the school book. Never used for training.
pub const EXAM_CSV: &str = include_str!("../../../../brain/exam.csv");

/// Personal lessons count this many times more than school ones: it's your computer.
const PERSONAL_WEIGHT: f32 = 4.0;
/// XP needed for each level. Level 1 starts at 0.
const LEVELS: [u32; 12] = [0, 250, 500, 800, 1200, 1700, 2300, 3000, 4000, 5500, 7500, 10_000];
const SEED: u64 = 0x1984;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Label {
    Delete,
    Review,
    Keep,
}

impl Label {
    pub const ALL: [Label; 3] = [Label::Delete, Label::Review, Label::Keep];

    pub fn index(self) -> usize {
        self as usize
    }

    pub fn parse(text: &str) -> Option<Label> {
        match text.trim() {
            "delete" => Some(Label::Delete),
            "review" => Some(Label::Review),
            "keep" => Some(Label::Keep),
            _ => None,
        }
    }

    pub fn as_str(self) -> &'static str {
        match self {
            Label::Delete => "delete",
            Label::Review => "review",
            Label::Keep => "keep",
        }
    }
}

/// A labeled example: "this kind of thing should be deleted / reviewed / kept".
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Lesson {
    #[serde(flatten)]
    pub sample: Sample,
    pub label: Label,
}

/// The brain's opinion about one item. Probabilities add up to 1.
#[derive(Debug, Clone, Copy, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Score {
    pub delete: f32,
    pub review: f32,
    pub keep: f32,
    /// `true` when you already told the brain about this exact item.
    pub remembered: bool,
}

impl Score {
    pub fn label(&self) -> Label {
        let p = [self.delete, self.review, self.keep];
        let best = (0..3).max_by(|&a, &b| p[a].total_cmp(&p[b])).unwrap_or(1);
        Label::ALL[best]
    }
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BrainStatus {
    pub level: u32,
    pub xp: u32,
    /// XP at which the current level started.
    pub level_xp: u32,
    /// XP needed for the next level (`None` at max level).
    pub next_level_xp: Option<u32>,
    pub school_lessons: usize,
    pub personal_lessons: usize,
    /// Share of the final exam (`brain/exam.csv`, paths it never trained on) answered right (0..1).
    pub accuracy: f32,
    /// Words the brain associates with junk / with precious things, most confident first.
    pub junk_words: Vec<String>,
    pub precious_words: Vec<String>,
}

pub struct Brain {
    net: Mlp,
    memory: HashMap<String, Label>,
    status: BrainStatus,
}

impl Brain {
    /// Trains a brain on the built-in school book plus personal lessons.
    pub fn new(personal: &[Lesson]) -> Self {
        Self::train(&school_lessons(), personal)
    }

    pub fn train(school: &[Lesson], personal: &[Lesson]) -> Self {
        let net = fit(school, personal);
        // The final exam: unseen paths, so the score is honest.
        let exam = parse_csv(EXAM_CSV).unwrap_or_default();
        let correct = exam.iter().filter(|l| argmax(net.predict(&encode(&l.sample))) == l.label.index()).count();
        let accuracy = if exam.is_empty() { 0.0 } else { correct as f32 / exam.len() as f32 };
        let xp = school.len() as u32 + 10 * personal.len() as u32;
        let level = LEVELS.iter().filter(|&&needed| xp >= needed).count() as u32;
        let (junk_words, precious_words) = favourite_words(&net, school.iter().chain(personal));

        Self {
            memory: personal.iter().map(|l| (l.sample.path.to_lowercase(), l.label)).collect(),
            status: BrainStatus {
                level,
                xp,
                level_xp: LEVELS[level as usize - 1],
                next_level_xp: LEVELS.get(level as usize).copied(),
                school_lessons: school.len(),
                personal_lessons: personal.len(),
                accuracy,
                junk_words,
                precious_words,
            },
            net,
        }
    }

    /// What does the brain think about this item? (`sample.path` must already be normalized.)
    pub fn score(&self, sample: &Sample) -> Score {
        if let Some(label) = self.memory.get(&sample.path.to_lowercase()) {
            let mut p = [0.015; 3];
            p[label.index()] = 0.97;
            return Score { delete: p[0], review: p[1], keep: p[2], remembered: true };
        }
        let [delete, review, keep] = self.net.predict(&encode(sample));
        Score { delete, review, keep, remembered: false }
    }

    pub fn status(&self) -> &BrainStatus {
        &self.status
    }
}

fn fit(school: &[Lesson], personal: &[Lesson]) -> Mlp {
    // Rare labels count more, so the brain doesn't just answer "delete" to everything.
    let mut counts = [0usize; 3];
    for lesson in school.iter().chain(personal) {
        counts[lesson.label.index()] += 1;
    }
    let total: usize = counts.iter().sum();
    let balance = counts.map(|c| if c == 0 { 1.0 } else { total as f32 / (3.0 * c as f32) });

    // Data augmentation: show every lesson again with a different size, so the brain learns
    // that the *name* matters more than how big something is.
    let mut rng = net::Rng(SEED);
    let mut examples = Vec::new();
    for (lesson, weight) in school.iter().map(|l| (l, 1.0)).chain(personal.iter().map(|l| (l, PERSONAL_WEIGHT))) {
        let class = lesson.label.index();
        let weight = weight * balance[class];
        examples.push(Example { input: encode(&lesson.sample), class, weight });
        let mut other = lesson.sample.clone();
        other.size_mb *= 10f32.powf(rng.uniform(1.0));
        examples.push(Example { input: encode(&other), class, weight });
    }
    let mut net = Mlp::new(SEED);
    net.train(&examples, 30, 0.05, SEED);
    net
}

fn argmax(p: [f32; 3]) -> usize {
    (0..3).max_by(|&a, &b| p[a].total_cmp(&p[b])).unwrap_or(1)
}

/// Asks the network about every word it has seen and keeps the ones it feels strongest about.
fn favourite_words<'a>(net: &Mlp, lessons: impl Iterator<Item = &'a Lesson>) -> (Vec<String>, Vec<String>) {
    let mut seen: HashMap<String, usize> = HashMap::new();
    for lesson in lessons {
        let unique: HashSet<String> =
            tokens(&lesson.sample.path).into_iter().filter(|t| t.starts_with("nw:") || t.starts_with("w:")).collect();
        for token in unique {
            *seen.entry(token).or_default() += 1;
        }
    }
    let mut scored: Vec<(String, f32)> = seen
        .into_iter()
        .filter(|(token, count)| *count >= 3 && !token.contains('#'))
        .map(|(token, _)| {
            let p = net.predict(&encode_word(&token));
            let word = token.split_once(':').map(|(_, w)| w.to_string()).unwrap_or(token);
            (word, p[0] - p[2])
        })
        .collect();
    scored.sort_by(|a, b| b.1.total_cmp(&a.1).then_with(|| a.0.cmp(&b.0)));
    scored.dedup_by(|a, b| a.0 == b.0);
    let junk = scored.iter().filter(|(_, s)| *s > 0.0).take(10).map(|(w, _)| w.clone()).collect();
    let precious = scored.iter().rev().filter(|(_, s)| *s < 0.0).take(10).map(|(w, _)| w.clone()).collect();
    (junk, precious)
}

/// The built-in school book.
pub fn school_lessons() -> Vec<Lesson> {
    parse_csv(SCHOOL_CSV).expect("brain/dataset.csv must be valid (cargo test checks it)")
}

/// Reads lessons from CSV: `path,kind,size_mb,age_days,label[,note]` with a header line.
pub fn parse_csv(text: &str) -> Result<Vec<Lesson>, String> {
    let mut lessons = Vec::new();
    for (n, line) in text.lines().enumerate().skip(1) {
        if line.trim().is_empty() {
            continue;
        }
        let fields = split_csv_line(line);
        let err = |what: &str| format!("line {}: {what}: {line}", n + 1);
        if fields.len() < 5 {
            return Err(err("expected path,kind,size_mb,age_days,label"));
        }
        let is_dir = match fields[1].trim() {
            "dir" => true,
            "file" => false,
            _ => return Err(err("kind must be file or dir")),
        };
        let size_mb = fields[2].trim().parse::<f32>().map_err(|_| err("size_mb is not a number"))?;
        let age_days = fields[3].trim().parse::<f32>().map_err(|_| err("age_days is not a number"))?;
        let label = Label::parse(&fields[4]).ok_or_else(|| err("label must be delete, review or keep"))?;
        lessons.push(Lesson { sample: Sample { path: fields[0].clone(), is_dir, size_mb, age_days }, label });
    }
    Ok(lessons)
}

/// Writes lessons in the same CSV format as `brain/dataset.csv`, ready to contribute.
pub fn to_csv(lessons: &[Lesson]) -> String {
    let quote =
        |s: &str| if s.contains([',', '"']) { format!("\"{}\"", s.replace('"', "\"\"")) } else { s.to_string() };
    let mut out = String::from("path,kind,size_mb,age_days,label,note\n");
    for l in lessons {
        let s = &l.sample;
        let kind = if s.is_dir { "dir" } else { "file" };
        out += &format!(
            "{},{kind},{:.1},{:.0},{},from a player\n",
            quote(&s.path),
            s.size_mb,
            s.age_days,
            l.label.as_str()
        );
    }
    out
}

fn split_csv_line(line: &str) -> Vec<String> {
    let mut fields = vec![String::new()];
    let mut quoted = false;
    let mut chars = line.chars().peekable();
    while let Some(c) = chars.next() {
        match c {
            '"' if quoted && chars.peek() == Some(&'"') => {
                chars.next();
                fields.last_mut().unwrap().push('"');
            }
            '"' => quoted = !quoted,
            ',' if !quoted => fields.push(String::new()),
            _ => fields.last_mut().unwrap().push(c),
        }
    }
    fields
}

/// Loads personal lessons (one JSON object per line). Later lessons about the same path win.
pub fn load_lessons(file: &Path) -> Vec<Lesson> {
    let Ok(text) = std::fs::read_to_string(file) else { return Vec::new() };
    let mut by_path: Vec<Lesson> = Vec::new();
    for lesson in text.lines().filter_map(|line| serde_json::from_str::<Lesson>(line).ok()) {
        by_path.retain(|l| !l.sample.path.eq_ignore_ascii_case(&lesson.sample.path));
        by_path.push(lesson);
    }
    by_path
}

pub fn save_lessons(file: &Path, lessons: &[Lesson]) -> std::io::Result<()> {
    if let Some(dir) = file.parent() {
        std::fs::create_dir_all(dir)?;
    }
    let mut out = std::fs::File::create(file)?;
    for lesson in lessons {
        writeln!(out, "{}", serde_json::to_string(lesson).map_err(std::io::Error::other)?)?;
    }
    Ok(())
}

/// Adds lessons, replacing older lessons about the same paths.
pub fn merge_lessons(existing: &mut Vec<Lesson>, new: Vec<Lesson>) {
    for lesson in new {
        existing.retain(|l| !l.sample.path.eq_ignore_ascii_case(&lesson.sample.path));
        existing.push(lesson);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample(path: &str, is_dir: bool, size_mb: f32, age_days: f32) -> Sample {
        Sample { path: path.into(), is_dir, size_mb, age_days }
    }

    #[test]
    fn school_book_is_valid_and_balanced() {
        let lessons = school_lessons();
        assert!(lessons.len() >= 300, "the brain needs enough examples");
        for label in Label::ALL {
            assert!(lessons.iter().filter(|l| l.label == label).count() >= 50, "{label:?} needs 50+ examples");
        }
    }

    #[test]
    fn brain_passes_its_exam() {
        // Community PRs to brain/dataset.csv must keep this green.
        let brain = Brain::new(&[]);
        let status = brain.status();
        assert!(status.accuracy >= 0.85, "exam score dropped to {:.0}%", status.accuracy * 100.0);
        assert!(status.level >= 2);
        assert!(!status.junk_words.is_empty() && !status.precious_words.is_empty());
    }

    #[test]
    fn brain_generalizes_to_new_paths() {
        let brain = Brain::new(&[]);
        let label = |path: &str, is_dir: bool, mb: f32, days: f32| brain.score(&sample(path, is_dir, mb, days)).label();
        assert_eq!(label("~/code/brand-new-project/node_modules", true, 700.0, 20.0), Label::Delete);
        assert_eq!(label("~/Library/Caches/com.example.newapp", true, 400.0, 3.0), Label::Delete);
        assert_eq!(label("~/Documents/Taxes/2025 tax return.pdf", false, 2.0, 100.0), Label::Keep);
        assert_eq!(label("~/.ssh/id_rsa", false, 0.01, 900.0), Label::Keep);
        assert_ne!(label("~/Movies/graduation-party.mov", false, 8000.0, 400.0), Label::Delete);
    }

    #[test]
    fn personal_lessons_are_remembered() {
        let path = "~/code/legacy-but-precious/node_modules";
        let lesson = Lesson { sample: sample(path, true, 900.0, 600.0), label: Label::Keep };
        let brain = Brain::new(std::slice::from_ref(&lesson));
        let score = brain.score(&lesson.sample);
        assert!(score.remembered);
        assert_eq!(score.label(), Label::Keep);
        assert_eq!(brain.status().personal_lessons, 1);
        assert_eq!(brain.status().xp, school_lessons().len() as u32 + 10);
    }

    #[test]
    fn csv_round_trip_and_errors() {
        let lessons = vec![Lesson { sample: sample("~/a, \"b\"/c", true, 12.0, 3.0), label: Label::Review }];
        assert_eq!(parse_csv(&to_csv(&lessons)).unwrap(), lessons);
        let bad = "path,kind,size_mb,age_days,label\n~/x,dir,1,2,maybe\n";
        assert!(parse_csv(bad).unwrap_err().contains("line 2"));
    }

    #[test]
    fn lessons_file_round_trip() {
        let dir = tempfile::tempdir().unwrap();
        let file = dir.path().join("brain/lessons.jsonl");
        let mut lessons = vec![Lesson { sample: sample("~/x", false, 1.0, 1.0), label: Label::Delete }];
        merge_lessons(&mut lessons, vec![Lesson { sample: sample("~/X", false, 1.0, 1.0), label: Label::Keep }]);
        assert_eq!(lessons.len(), 1, "same path (ignoring case) is replaced");
        save_lessons(&file, &lessons).unwrap();
        assert_eq!(load_lessons(&file), lessons);
        assert!(load_lessons(&dir.path().join("missing.jsonl")).is_empty());
    }
}
