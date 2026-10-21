//! Scan from the terminal, no UI needed:
//!
//! ```sh
//! cargo run --release -p invaders-core --example scan -- ~/
//! ```

use std::path::PathBuf;

use invaders_core::{scan, Progress, ScanOptions};

fn main() {
    let roots: Vec<PathBuf> = std::env::args().skip(1).map(PathBuf::from).collect();
    if roots.is_empty() {
        eprintln!("usage: scan <folder> [folder...]");
        std::process::exit(2);
    }
    let progress = Progress::new();
    let (report, _tree) = scan(&roots, &ScanOptions::default(), &progress);

    let gb = |b: u64| b as f64 / 1e9;
    println!(
        "{} files, {} folders, {:.2} GB in {:.1}s ({} unreadable)\n",
        report.files,
        report.dirs,
        gb(report.bytes),
        report.elapsed_ms as f64 / 1000.0,
        report.errors
    );
    for summary in report.summaries.iter().filter(|s| s.count > 0) {
        println!("{:<14} {:>9.2} GB  {:>6} items", format!("{:?}", summary.category), gb(summary.bytes), summary.count);
        for item in report.items.iter().filter(|i| i.category == summary.category).take(3) {
            println!("    {:>9.2} GB  {}  ({})", gb(item.size), item.path, item.label);
        }
    }
}
