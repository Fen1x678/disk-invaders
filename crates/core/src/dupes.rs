//! Duplicate finder ("clone hunter").
//!
//! 1. Group files by exact length (free, we already know it).
//! 2. Hash the first and last 16 KiB of each file in a group (cheap, rules out almost everything).
//! 3. Fully hash whatever is still tied with BLAKE3. Only byte-identical files end up in a group.

use std::collections::HashMap;
use std::fs::File;
use std::io::{Read, Seek, SeekFrom};

use rayon::prelude::*;

use crate::model::{Category, Item, Risk};
use crate::progress::Progress;

const CHUNK: u64 = 16 * 1024;

#[derive(Debug, Clone)]
pub struct Candidate {
    pub path: String,
    /// Length in bytes (what must match).
    pub len: u64,
    /// Bytes on disk (what deleting it frees).
    pub size: u64,
    pub modified: Option<i64>,
}

/// Returns groups of identical files (each group has 2+ files).
pub fn find(candidates: Vec<Candidate>, progress: &Progress) -> Vec<Vec<Candidate>> {
    let mut by_len: HashMap<u64, Vec<Candidate>> = HashMap::new();
    for c in candidates {
        by_len.entry(c.len).or_default().push(c);
    }
    let groups: Vec<Vec<Candidate>> = by_len.into_values().filter(|g| g.len() > 1).collect();

    let groups = refine(groups, progress, partial_hash);
    // For small files the partial hash already covered every byte.
    let (done, unsure): (Vec<_>, Vec<_>) = groups.into_iter().partition(|g| g[0].len <= 2 * CHUNK);
    let mut confirmed = refine(unsure, progress, full_hash);
    confirmed.extend(done);
    confirmed
}

/// Splits every group by `hash`, keeping only sub-groups that still have 2+ files.
fn refine(
    groups: Vec<Vec<Candidate>>,
    progress: &Progress,
    hash: fn(&Candidate) -> std::io::Result<[u8; 32]>,
) -> Vec<Vec<Candidate>> {
    progress.add_hash_work(groups.iter().map(|g| g.len() as u64).sum());
    groups
        .into_par_iter()
        .flat_map_iter(|group| {
            let mut by_hash: HashMap<[u8; 32], Vec<Candidate>> = HashMap::new();
            for candidate in group {
                if progress.is_cancelled() {
                    return Vec::new();
                }
                // Unreadable files are simply left out.
                if let Ok(h) = hash(&candidate) {
                    by_hash.entry(h).or_default().push(candidate);
                }
                progress.hashed_one();
            }
            by_hash.into_values().filter(|g| g.len() > 1).collect::<Vec<_>>()
        })
        .collect()
}

fn partial_hash(c: &Candidate) -> std::io::Result<[u8; 32]> {
    let mut file = File::open(&c.path)?;
    let mut hasher = blake3::Hasher::new();
    let mut buf = vec![0u8; CHUNK as usize];
    let n = read_up_to(&mut file, &mut buf)?;
    hasher.update(&buf[..n]);
    if c.len > 2 * CHUNK {
        file.seek(SeekFrom::End(-(CHUNK as i64)))?;
        let n = read_up_to(&mut file, &mut buf)?;
        hasher.update(&buf[..n]);
    } else if c.len > CHUNK {
        let mut rest = Vec::new();
        file.read_to_end(&mut rest)?;
        hasher.update(&rest);
    }
    Ok(*hasher.finalize().as_bytes())
}

fn full_hash(c: &Candidate) -> std::io::Result<[u8; 32]> {
    let mut hasher = blake3::Hasher::new();
    hasher.update_reader(File::open(&c.path)?)?;
    Ok(*hasher.finalize().as_bytes())
}

fn read_up_to(file: &mut File, buf: &mut [u8]) -> std::io::Result<usize> {
    let mut filled = 0;
    while filled < buf.len() {
        match file.read(&mut buf[filled..])? {
            0 => break,
            n => filled += n,
        }
    }
    Ok(filled)
}

/// Turns groups into items: biggest waste first, oldest copy first inside each group.
pub fn to_items(mut groups: Vec<Vec<Candidate>>) -> Vec<Item> {
    groups.sort_by_key(|g| std::cmp::Reverse(g[0].size * (g.len() as u64 - 1)));
    let mut items = Vec::new();
    for (id, mut group) in groups.into_iter().enumerate() {
        group.sort_by(|a, b| a.modified.cmp(&b.modified).then_with(|| a.path.len().cmp(&b.path.len())));
        let label = format!("{} identical copies", group.len());
        items.extend(group.into_iter().map(|c| Item {
            path: c.path,
            size: c.size,
            is_dir: false,
            modified: c.modified,
            category: Category::Duplicates,
            risk: Risk::Review,
            label: label.clone(),
            group: Some(id as u32),
            brain: None,
        }));
    }
    items
}

/// Removes groups that shrank to a single file (e.g. after filtering out protected paths).
pub fn drop_lonely(items: Vec<Item>) -> Vec<Item> {
    let mut counts: HashMap<Option<u32>, usize> = HashMap::new();
    for item in &items {
        *counts.entry(item.group).or_default() += 1;
    }
    items.into_iter().filter(|i| counts[&i.group] > 1).collect()
}

/// Bytes freed by keeping exactly one copy of every group.
pub fn reclaimable(items: &[Item]) -> u64 {
    let mut seen = std::collections::HashSet::new();
    items.iter().filter(|i| !seen.insert(i.group)).map(|i| i.size).sum()
}

/// First index >= `max` that starts a new group, so a group is never cut in half.
pub fn group_boundary(items: &[Item], max: usize) -> usize {
    (max..items.len()).find(|&i| i == 0 || items[i].group != items[i - 1].group).unwrap_or(items.len())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn finds_identical_files_only() {
        let dir = tempfile::tempdir().unwrap();
        let write = |name: &str, data: &[u8]| {
            let path = dir.path().join(name);
            std::fs::write(&path, data).unwrap();
            Candidate {
                path: path.to_string_lossy().into_owned(),
                len: data.len() as u64,
                size: data.len() as u64,
                modified: None,
            }
        };
        let big: Vec<u8> = (0..200_000u32).map(|i| (i % 251) as u8).collect();
        let mut almost = big.clone();
        almost[100_000] ^= 1; // same length, same head and tail, different middle
        let candidates = vec![
            write("a.bin", &big),
            write("b.bin", &big),
            write("c.bin", &almost),
            write("small1.txt", b"hello"),
            write("small2.txt", b"hello"),
            write("small3.txt", b"world"),
        ];
        let mut groups = find(candidates, &Progress::new());
        groups.sort_by_key(|g| g[0].len);
        assert_eq!(groups.len(), 2);
        assert_eq!(groups[0].len(), 2, "two 'hello' files");
        assert_eq!(groups[1].len(), 2, "a.bin and b.bin, but not the almost-identical c.bin");
        assert!(groups[1].iter().all(|c| !c.path.ends_with("c.bin")));
    }

    #[test]
    fn group_helpers() {
        let item = |group, size| Item {
            path: String::new(),
            size,
            is_dir: false,
            modified: None,
            category: Category::Duplicates,
            risk: Risk::Review,
            label: String::new(),
            group: Some(group),
            brain: None,
        };
        let items = vec![item(0, 10), item(0, 10), item(0, 10), item(1, 5), item(1, 5), item(2, 1)];
        assert_eq!(reclaimable(&items), 25);
        assert_eq!(drop_lonely(items.clone()).len(), 5);
        assert_eq!(group_boundary(&items, 2), 3);
        assert_eq!(group_boundary(&items, 3), 3);
        assert_eq!(group_boundary(&items, 100), 6);
    }
}
