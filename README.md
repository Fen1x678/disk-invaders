<p align="center"><img src="docs/icon.png" width="120" alt="Disk Invaders icon"></p>

<h1 align="center">DISK INVADERS</h1>
<p align="center"><b>The 8-bit cleaner for your 64-bit disk.</b><br>
Find caches, build junk, forgotten downloads, duplicates and huge files on macOS &amp; Windows, then blast them away arcade-style.</p>

<p align="center"><img src="docs/demo.gif" alt="Demo: scan, pick targets, fire" width="760"></p>

> 🇷🇺 [Читать на русском](README.ru.md)

## Why?

Disk cleaners are either paid, closed-source, or look like a tax form. Disk Invaders is free, open source, fast (Rust + Tauri, ~10 MB app) and fun: freed gigabytes are your **SCORE**.

## Features

- **Scans everything**: the whole computer, your home folder, or any folder/drive. Parallel Rust walker: hundreds of thousands of files per second.
- **9 levels of junk**: app caches, dev junk (`node_modules`, Rust `target`, Xcode DerivedData, Gradle, `.venv`…), logs &amp; crash dumps, Trash, installers (`.dmg`, `.pkg`, `.msi`, `.iso`), old downloads, iPhone backups, duplicates, big files.
- **Duplicate hunter**: size → partial hash → full BLAKE3 hash. Only byte-identical files are reported.
- **Explorer** with a treemap: "where did my space go?"
- **🧠 INVADER BRAIN**: a tiny neural network written from scratch (no ML libraries, no cloud) rates every item *delete / check / keep*, learns from every choice you make and levels up. Give it a free local **voice** (Ollama) and it chats about your disk. [How it works](docs/BRAIN.md).
- **Safe by design**: everything goes to the Trash by default (NUKE mode deletes forever). A Rust guard refuses system folders, your home folder, user folders, SSH keys and keychains, no matter what the UI asks.
- **Retro 80s look**: synthwave, green phosphor, amber and Commodore 64 themes, CRT scanlines, chip-tune sound effects, keyboard controls.
- **English and Russian** UI (add yours, it's one file).

| | |
|---|---|
| ![Title](docs/screenshots/title.png) | ![Results](docs/screenshots/results.png) |
| ![Explorer](docs/screenshots/explorer.png) | ![Duplicates](docs/screenshots/duplicates.png) |
| ![Brain](docs/screenshots/brain.png) | ![Brain says](docs/screenshots/brain-says.png) |
| ![Phosphor](docs/screenshots/theme-phosphor.png) | ![C64](docs/screenshots/theme-c64.png) |

## Download

Grab the latest `.dmg` (macOS) or `.msi` / `.exe` (Windows) from **[Releases](../../releases)**.

- **macOS:** builds are not notarized yet. Right-click the app → *Open* the first time. For a complete scan, allow *System Settings → Privacy &amp; Security → Full Disk Access*.
- **Windows:** SmartScreen may warn about an unknown publisher → *More info* → *Run anyway*.

## Build from source

You need [Node.js 20+](https://nodejs.org), [Rust 1.90+](https://rustup.rs) and the [Tauri prerequisites](https://v2.tauri.app/start/prerequisites/) for your OS.

```sh
npm install
npm run app          # run the desktop app in dev mode
npm run app:build    # build an installer into target/release/bundle
```

UI only, no Rust needed: `npm run dev` opens a **demo mode** in the browser with a fake disk.

Scanner only, no UI: `cargo run --release -p invaders-core --example scan -- ~/`

Brain only: `cargo run --release -p invaders-core --example brain -- "~/code/shop/node_modules"` (without arguments it takes its final exam)

## How it works

```
src/                    TypeScript UI (no framework, ~2k lines)
  screens/              title, scan and results screens
  api.ts / mock.ts      bridge to Rust / fake backend for the browser demo
  i18n.ts               all UI strings
  styles/main.css       themes are just CSS variables
src-tauri/              the desktop shell (Tauri 2): commands the UI calls
crates/core/            the engine, plain Rust, fully unit-tested
  scanner.rs            parallel walk, size tree, junk detection
  rules.rs              junk rules  ← easiest place to contribute
  dupes.rs              duplicate finder
  safety.rs             what may never be deleted
  delete.rs             Trash / permanent delete
  brain/                the neural network, from scratch
  voice.rs              talks to a local language model (Ollama)
brain/                  what the brain learns from  ← teach it with a PR
  dataset.csv           school book (labeled examples)
  exam.csv              final exam (never trained on)
  knowledge.json        plain-language knowledge cards (EN/RU)
```

## Roadmap

- 🧠 **Invader-1.7B**: fine-tune the brain's voice on Disk Invaders knowledge and publish it as our own open model. See [BRAIN.md](docs/BRAIN.md#next-level-an-invader-model-of-our-own).
- Achievements, sorting, an "Empty Trash" button, NTFS MFT turbo scan on Windows: see [good first issues](docs/GOOD_FIRST_ISSUES.md).

## Contributing

PRs are very welcome, including your first one. See [CONTRIBUTING.md](CONTRIBUTING.md) and [good first issues](docs/GOOD_FIRST_ISSUES.md). Popular starters: **teach the brain** (add lessons to `brain/dataset.csv`), a new junk rule, a new theme, a new language.

## License

[MIT](LICENSE)
