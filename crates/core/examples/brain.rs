//! Ask the brain from the terminal:
//!
//! ```sh
//! cargo run -p invaders-core --example brain -- "~/code/shop/node_modules" "~/Documents/thesis.docx"
//! ```
//!
//! Without arguments it takes the final exam and shows every answer.

use std::time::Instant;

use invaders_core::brain::{parse_csv, Brain, Sample, EXAM_CSV};

fn main() {
    let started = Instant::now();
    let brain = Brain::new(&[]);
    let status = brain.status();
    println!(
        "INVADER BRAIN · LVL {} · {} lessons · exam accuracy {:.0}% · trained in {} ms",
        status.level,
        status.school_lessons,
        status.accuracy * 100.0,
        started.elapsed().as_millis()
    );
    println!("junk words:     {}", status.junk_words.join(", "));
    println!("precious words: {}\n", status.precious_words.join(", "));

    if std::env::args().len() == 1 {
        println!("final exam (brain/exam.csv):");
        for lesson in parse_csv(EXAM_CSV).unwrap() {
            let score = brain.score(&lesson.sample);
            let mark = if score.label() == lesson.label { "  ok " } else { "MISS " };
            println!(
                "{mark} {:<7} (should be {:<6}) d{:>3.0}% r{:>3.0}% k{:>3.0}%  {}",
                score.label().as_str(),
                lesson.label.as_str(),
                score.delete * 100.0,
                score.review * 100.0,
                score.keep * 100.0,
                lesson.sample.path
            );
        }
    }

    for path in std::env::args().skip(1) {
        let is_dir = !path.rsplit(['/', '\\']).next().unwrap_or("").contains('.');
        let score = brain.score(&Sample { path: path.clone(), is_dir, size_mb: 500.0, age_days: 120.0 });
        println!(
            "{:<7} delete {:>3.0}%  review {:>3.0}%  keep {:>3.0}%  {path}",
            score.label().as_str().to_uppercase(),
            score.delete * 100.0,
            score.review * 100.0,
            score.keep * 100.0
        );
    }
}
