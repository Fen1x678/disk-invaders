# Contributing to Disk Invaders

Thanks for joining the fleet! First PR ever? Perfect, this project is made for that.

## Setup

```sh
npm install
npm run dev                      # UI in the browser with a fake disk (no Rust needed)
npm run app                      # the real desktop app
cargo test -p invaders-core      # engine tests (fast, no UI)
```

Before opening a PR: `npm run build`, `cargo fmt --all`, `cargo clippy --workspace`, `cargo test -p invaders-core`.

## Add a junk rule (easiest Rust PR)

Open `crates/core/src/rules.rs`.

- A folder recognised **by name** (like `node_modules`): add an arm to `match_dir_name`. If it is only junk next to a project file, add that file to `Markers` (see `target` + `Cargo.toml`).
- A **known location** (like `~/Library/Caches`): add a line in `Rules::new`. `Whole` = one item, `EachChild` = one item per child.
- Add a test at the bottom of the file.

Rule of thumb: if a tool re-creates it automatically, it's `Risk::Safe`. If a human might miss it, it's `Risk::Review`.

## Teach the brain (no Rust needed!)

The INVADER BRAIN learns from `brain/dataset.csv`. Each line is a lesson:

```csv
path,kind,size_mb,age_days,label,note
~/Library/Caches/com.example.app,dir,350,10,delete,app cache
~/Documents/Thesis/final.docx,file,2,30,keep,document
```

- `~` is the home folder; Windows paths with `\` are fine.
- `label` is `delete` (safe junk), `review` (look first) or `keep` (precious).
- Teach *kinds* of things, not your personal files. Vary names, sizes and ages.
- If you add a new kind of junk, also add one or two **exam** questions to `brain/exam.csv` with different names (never copy a lesson into the exam).
- Explain the thing in plain words in `brain/knowledge.json` (`en` + `ru`).

Check your lessons: `cargo test -p invaders-core brain` (the exam must stay ≥ 85%) and `cargo run -p invaders-core --example brain` to see every answer. Players can also press **EXPORT MY LESSONS** in the app and share the CSV.

## Add a theme (easiest CSS PR)

Copy a `[data-theme="..."]` block in `src/styles/main.css`, change the colours, add the name to `THEMES` in `src/prefs.ts`. Add a screenshot to your PR!

## Add a language

Copy the `ru` object in `src/i18n.ts`, translate the values, add it to `LANGUAGES`.

## Safety first

Never weaken `crates/core/src/safety.rs` without a test that explains why. A cleaner that deletes the wrong thing once is uninstalled forever.
