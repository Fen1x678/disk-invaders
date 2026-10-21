// A fake computer for the in-browser demo. Lets you play with (and screenshot) the UI without Rust.

import type { Backend } from "./api";
import { answerOffline, explainOffline, findCards } from "./brain";
import type {
  BrainScore,
  BrainStatus,
  Category,
  DeleteOutcome,
  Item,
  Progress,
  Risk,
  ScanReport,
  SystemInfo,
  Teaching,
  TreeEntry,
  Verdict,
  VoiceStatus,
} from "./types";
import { CATEGORIES } from "./types";

const HOME = "/Users/player";
const GB = 1e9;
const MB = 1e6;
const DAY = 86400;
const now = Math.floor(Date.now() / 1000);

type Row = [path: string, size: number, label: string, risk?: Risk, ageDays?: number, isDir?: boolean];

const ROWS: Record<Category, Row[]> = {
  caches: [
    ["~/Library/Caches/com.spotify.client", 2.1 * GB, "App cache"],
    ["~/Library/Caches/Google", 1.4 * GB, "App cache"],
    ["~/Library/Caches/Yarn", 1.2 * GB, "App cache"],
    ["~/Library/Caches/Homebrew", 890 * MB, "App cache"],
    ["~/Library/Caches/JetBrains", 640 * MB, "App cache"],
    ["~/Library/Caches/pip", 310 * MB, "App cache"],
    ["~/Library/Caches/com.apple.HomeKit", 2 * MB, "App cache"],
    ["~/Library/Application Support/Slack/Cache", 520 * MB, "Browser / Electron app cache"],
    ["~/Library/Application Support/discord/Cache", 410 * MB, "Browser / Electron app cache"],
    ["~/Library/Application Support/Code/Code Cache", 160 * MB, "Browser / Electron app cache"],
    ["~/.npm/_cacache", 1.8 * GB, "npm cache"],
    ["~/.gradle/caches", 2.3 * GB, "Gradle cache"],
  ],
  dev_junk: [
    ["~/Library/Developer/Xcode/DerivedData", 9.8 * GB, "Xcode DerivedData"],
    ["~/Library/Developer/Xcode/iOS DeviceSupport/17.4 (21E219)", 4.1 * GB, "iOS DeviceSupport"],
    ["~/Library/Developer/Xcode/iOS DeviceSupport/16.6 (20G75)", 3.2 * GB, "iOS DeviceSupport"],
    ["~/code/game-jam/target", 3.4 * GB, "Rust build output"],
    ["~/code/ml-course/.venv", 2.2 * GB, "Python virtualenv", "review"],
    ["~/code/hackathon-2024/node_modules", 1.1 * GB, "node_modules (npm / yarn / pnpm)"],
    ["~/code/android-app/build", 780 * MB, "Gradle build output"],
    ["~/code/portfolio/node_modules", 612 * MB, "node_modules (npm / yarn / pnpm)"],
    ["~/code/portfolio/.next", 340 * MB, "JavaScript framework cache"],
    ["~/code/telegram-bot/__pycache__", 3 * MB, "Python bytecode cache"],
  ],
  logs: [
    ["~/java_pid4242.hprof", 2.1 * GB, "Crash / memory dump", "safe", 120, false],
    ["~/code/api/server.log", 820 * MB, "Log file", "safe", 30, false],
    ["~/Library/Logs/JetBrains", 340 * MB, "App logs"],
    ["~/Library/Logs/DiagnosticReports", 120 * MB, "App logs"],
  ],
  trash: [
    ["~/.Trash/Untitled.mov", 3.2 * GB, "In the Trash", "safe", 12, false],
    ["~/.Trash/old-project", 640 * MB, "In the Trash", "safe", 40],
  ],
  installers: [
    ["~/Downloads/ubuntu-24.04-desktop-amd64.iso", 5.8 * GB, "Installer / disk image", "review", 210, false],
    ["~/Downloads/Xcode_15.4.xip", 3.0 * GB, "Installer / disk image", "review", 150, false],
    ["~/Downloads/Docker.dmg", 610 * MB, "Installer / disk image", "review", 90, false],
    ["~/Downloads/Figma.dmg", 210 * MB, "Installer / disk image", "review", 400, false],
    ["~/Downloads/zoomusInstallerFull.pkg", 90 * MB, "Installer / disk image", "review", 600, false],
  ],
  old_downloads: [
    ["~/Downloads/lecture-recordings", 7.4 * GB, "Old download", "review", 380],
    ["~/Downloads/photos-2022.zip", 2.6 * GB, "Old download", "review", 1100, false],
    ["~/Downloads/thesis-final-FINAL-v3.pdf", 12 * MB, "Old download", "review", 760, false],
    ["~/Downloads/Untitled design.png", 4 * MB, "Old download", "review", 240, false],
  ],
  backups: [
    ["~/Library/Application Support/MobileSync/Backup/00008110-001A2B3C4D5E", 48.2 * GB, "iPhone / iPad backup", "review", 730],
    ["~/Library/Application Support/MobileSync/Backup/00008030-000E1D2A3B4C", 31 * GB, "iPhone / iPad backup", "review", 150],
  ],
  duplicates: [],
  large_files: [
    ["~/Library/Containers/com.docker.docker/Data/vms/0/data/Docker.raw", 38 * GB, "Virtual machine disk", "review", 2, false],
    ["~/Movies/obs-recording-2024-03-01.mkv", 7.7 * GB, "Video", "review", 230, false],
    ["~/Downloads/ubuntu-24.04-desktop-amd64.iso", 5.8 * GB, "Disk image / installer", "review", 210, false],
    ["~/models/llama-3-8b-instruct.Q4_K_M.gguf", 4.9 * GB, "AI model / binary blob", "review", 60, false],
    ["~/Movies/trip-to-sochi.mov", 4.2 * GB, "Video", "review", 500, false],
    ["~/Desktop/trip-to-sochi copy.mov", 4.2 * GB, "Video", "review", 20, false],
    ["~/Music/Logic/backing-tracks.wav", 1.3 * GB, "Audio", "review", 300, false],
  ],
};

const DUPLICATE_GROUPS: [size: number, paths: string[]][] = [
  [4.2 * GB, ["~/Movies/trip-to-sochi.mov", "~/Desktop/trip-to-sochi copy.mov"]],
  [380 * MB, ["~/Music/Albums/synthwave-mix.flac", "~/Downloads/synthwave-mix.flac"]],
  [64 * MB, ["~/Documents/Courses/rust-book.pdf", "~/Downloads/rust-book(1).pdf", "~/Desktop/rust-book.pdf"]],
  [18 * MB, ["~/Pictures/wallpaper-neon.png", "~/Downloads/wallpaper-neon.png", "~/Desktop/wallpaper-neon (2).png"]],
];

/** Big stuff that isn't junk, so the Explorer has something to show. */
const OTHER_SPACE: [string, number][] = [
  ["/System", 17.8 * GB],
  ["/Applications/Xcode.app", 12.4 * GB],
  ["/Applications/Adobe Photoshop 2025", 4.6 * GB],
  ["/Applications/Steam.app", 1.1 * GB],
  ["/Applications/Spotify.app", 0.4 * GB],
  ["/Library/Application Support", 6.2 * GB],
  ["/private/var", 9.4 * GB],
  ["/usr", 2.1 * GB],
  ["/opt/homebrew", 5.3 * GB],
  ["~/Pictures/Photos Library.photoslibrary", 61.7 * GB],
  ["~/Documents/University", 14.2 * GB],
  ["~/Documents/Personal", 3.1 * GB],
  ["~/Music/Music Library", 8.6 * GB],
  ["~/Library/Mail", 6.4 * GB],
  ["~/Library/Messages", 4.8 * GB],
  ["~/Library/Containers/com.docker.docker/Data/log", 0.2 * GB],
  ["~/code/game-jam/src", 0.05 * GB],
  ["~/code/portfolio/src", 0.02 * GB],
  ["~/Steam/steamapps/common/Hades", 15.2 * GB],
  ["~/Steam/steamapps/common/Cyberpunk 2077", 70.3 * GB],
];

const abs = (path: string) => path.replace(/^~/, HOME);

function buildItems(): Item[] {
  const items: Item[] = [];
  for (const category of CATEGORIES) {
    for (const [path, size, label, risk = "safe", ageDays = 45, isDir = true] of ROWS[category]) {
      items.push({ path: abs(path), size: Math.round(size), isDir, modified: now - ageDays * DAY, category, risk, label });
    }
  }
  DUPLICATE_GROUPS.forEach(([size, paths], group) =>
    paths.forEach((path, i) =>
      items.push({
        path: abs(path),
        size,
        isDir: false,
        modified: now - (500 - i * 200) * DAY,
        category: "duplicates",
        risk: "review",
        label: `${paths.length} identical copies`,
        group,
      }),
    ),
  );
  return items;
}

// ---------- fake folder tree ----------

interface Node {
  size: number;
  files: number;
  children: Map<string, Node>;
  isDir: boolean;
}

const newNode = (isDir = true): Node => ({ size: 0, files: 0, children: new Map(), isDir });
const tree = newNode();

function insert(path: string, size: number, isDir: boolean): void {
  const parts = path.split("/").filter(Boolean);
  const files = isDir ? Math.max(1, Math.round(size / 2e6)) : 1;
  let node = tree;
  node.size += size;
  node.files += files;
  for (const [i, part] of parts.entries()) {
    let child = node.children.get(part);
    if (!child) {
      child = newNode(i < parts.length - 1 || isDir);
      node.children.set(part, child);
    }
    child.size += size;
    child.files += files;
    node = child;
  }
}

function find(path: string): Node | undefined {
  let node: Node | undefined = tree;
  for (const part of path.split("/").filter(Boolean)) node = node?.children.get(part);
  return node;
}

function removeFromTree(path: string): void {
  const parts = path.split("/").filter(Boolean);
  const target = find(path);
  if (!target) return;
  let node = tree;
  for (const part of parts) {
    node.size -= target.size;
    node.files -= target.files;
    if (part === parts[parts.length - 1] && node.children.get(part) === target) {
      node.children.delete(part);
      return;
    }
    node = node.children.get(part)!;
  }
}

const seen = new Set<string>();
for (const item of buildItems()) {
  if (!seen.has(item.path)) insert(item.path, item.size, item.isDir);
  seen.add(item.path);
}
for (const [path, size] of OTHER_SPACE) insert(abs(path), Math.round(size), true);

// ---------- the backend ----------

const sleep = (ms: number) => new Promise((resolve) => setTimeout(resolve, ms));
let cancelled = false;
let lastRoots = ["/"];

const FAKE_PATHS = [
  "/Applications/Xcode.app/Contents/Developer/Platforms",
  `${HOME}/Library/Caches/com.spotify.client/Data`,
  `${HOME}/code/hackathon-2024/node_modules/@babel/core/lib`,
  `${HOME}/Pictures/Photos Library.photoslibrary/originals/A`,
  `${HOME}/Library/Developer/Xcode/DerivedData/Game-abc/Build`,
  "/System/Library/Frameworks/AppKit.framework",
  `${HOME}/Steam/steamapps/common/Cyberpunk 2077/archive/pc`,
  `${HOME}/Library/Application Support/MobileSync/Backup`,
  `${HOME}/Downloads/lecture-recordings/week-07`,
  "/private/var/db/uuidtext",
];

// ---------- fake brain: uses the knowledge cards instead of the real neural network ----------

const lessons = new Map<string, Teaching>();
const LEVELS = [0, 250, 500, 800, 1200, 1700, 2300, 3000, 4000, 5500, 7500, 10_000];
let lastItems: Item[] = [];
let stopVoice = false;
const voiceState: VoiceStatus = { running: true, version: "demo", models: ["qwen3:1.7b"] };

function brainStatus(): BrainStatus {
  const xp = 693 + lessons.size * 10;
  const level = LEVELS.filter((needed) => xp >= needed).length;
  return {
    level,
    xp,
    levelXp: LEVELS[level - 1],
    nextLevelXp: LEVELS[level] ?? null,
    schoolLessons: 693,
    personalLessons: lessons.size,
    accuracy: 1,
    junkWords: ["cache", "caches", "logs", "temp", "build", "deriveddata", "node_modules", "trash", "dmg", "setup"],
    preciousWords: ["documents", "photos", "git", "src", "ssh", "keychains", "saves", "tdata", "thesis", "passport"],
  };
}

function brainScore(path: string): BrainScore {
  const p = { delete: 0.34, review: 0.33, keep: 0.33 };
  const told = lessons.get(path)?.label;
  const verdict: Verdict | undefined = told ?? findCards(path, 1)[0]?.verdict;
  if (verdict) {
    const wobble = ([...path].reduce((h, c) => (h * 31 + c.charCodeAt(0)) % 997, 7) % 14) / 100;
    p.delete = p.review = p.keep = told ? 0.015 : (0.12 + wobble) / 2;
    p[verdict] = told ? 0.97 : 0.88 - wobble;
  }
  return { ...p, remembered: Boolean(told) };
}

export const mockBackend: Backend = {
  demo: true,

  async systemInfo(): Promise<SystemInfo> {
    return {
      platform: "macos",
      home: HOME,
      volumes: [
        { name: "Macintosh HD", mount: "/", total: 494.4 * GB, available: 41.7 * GB, removable: false },
        { name: "RETRO-BACKUP", mount: "/Volumes/RETRO-BACKUP", total: 2000 * GB, available: 1240 * GB, removable: true },
      ],
      wholeDiskRoots: ["/"],
    };
  },

  async startScan(roots, _options, onProgress): Promise<ScanReport> {
    cancelled = false;
    lastRoots = roots;
    const started = Date.now();
    const scope = roots[0] === "/" ? tree.size : find(roots[0])?.size ?? tree.size / 3;
    const totalFiles = Math.round(scope / 330_000);
    const walkMs = 3600;
    let p: Progress = { files: 0, dirs: 0, bytes: 0, errors: 0, hashed: 0, hashTotal: 0, phase: "walk", current: "" };
    for (let t = 0; t < walkMs && !cancelled; t += 80) {
      const k = Math.pow(t / walkMs, 0.8);
      p = {
        ...p,
        files: Math.round(totalFiles * k),
        dirs: Math.round((totalFiles / 9) * k),
        bytes: Math.round(scope * k),
        errors: Math.round(37 * k),
        current: FAKE_PATHS[Math.floor(t / 360) % FAKE_PATHS.length],
      };
      onProgress(p);
      await sleep(80);
    }
    for (let i = 0; i <= 20 && !cancelled; i++) {
      onProgress({ ...p, phase: "duplicates", hashTotal: 4210, hashed: Math.round(4210 * (i / 20)) });
      await sleep(60);
    }

    const items = buildItems().filter((item) => roots.some((root) => item.path.startsWith(root)));
    items.forEach((item) => (item.brain = brainScore(item.path)));
    lastItems = items;
    const summaries = CATEGORIES.map((category) => {
      const list = items.filter((i) => i.category === category);
      let bytes = list.reduce((sum, i) => sum + i.size, 0);
      if (category === "duplicates") bytes -= DUPLICATE_GROUPS.reduce((sum, [size]) => sum + size, 0);
      return { category, count: list.length, bytes: Math.max(0, bytes), shown: list.length };
    });
    return {
      roots,
      files: totalFiles,
      dirs: Math.round(totalFiles / 9),
      bytes: Math.round(scope),
      errors: 37,
      elapsedMs: Date.now() - started,
      cancelled,
      summaries,
      items,
    };
  },

  async cancelScan() {
    cancelled = true;
  },

  async listDir(path): Promise<TreeEntry[]> {
    if (path === null) {
      return lastRoots.map((root) => ({ name: root, path: root, size: find(root)?.size ?? 0, files: find(root)?.files ?? 0, kind: "dir", brain: brainScore(root) }));
    }
    const node = find(path);
    if (!node) throw new Error("not_found");
    if (node.children.size === 0) {
      return [{ name: "", path: null, size: node.size, files: Math.max(1, node.files), kind: "small_files" }];
    }
    const base = path.endsWith("/") ? path : `${path}/`;
    return [...node.children.entries()]
      .map(([name, child]): TreeEntry => ({
        name,
        path: base + name,
        size: child.size,
        files: child.files,
        kind: child.isDir ? "dir" : "file",
        brain: brainScore(base + name),
      }))
      .sort((a, b) => b.size - a.size);
  },

  async deleteItems(targets): Promise<DeleteOutcome[]> {
    await sleep(500);
    return targets.map(({ path }) => {
      // Show off the failure dialog: macOS protects some Apple caches.
      if (path.includes("com.apple.")) return { path, ok: false, error: "permission_denied" };
      if (path === HOME || path === "/" || path.split("/").length <= 2) return { path, ok: false, error: "blocked:top_level" };
      removeFromTree(path);
      return { path, ok: true };
    });
  },

  async reveal() {},
  async pickFolder() {
    return `${HOME}/code`;
  },
  async openUrl(url) {
    window.open(url, "_blank", "noopener");
  },

  async brainStatus() {
    return brainStatus();
  },
  async brainScore(things) {
    return things.map((thing) => brainScore(thing.path));
  },
  async brainTeach(newLessons) {
    await sleep(150);
    newLessons.forEach((lesson) => lessons.set(lesson.path, lesson));
    return brainStatus();
  },
  async brainExport() {
    const rows = [...lessons.values()].map((l) => `${l.path},${l.isDir ? "dir" : "file"},${(l.size / 1048576).toFixed(1)},0,${l.label},from a player`);
    const blob = new Blob([["path,kind,size_mb,age_days,label,note", ...rows].join("\n")], { type: "text/csv" });
    const link = Object.assign(document.createElement("a"), { href: URL.createObjectURL(blob), download: "my-brain-lessons.csv" });
    link.click();
    return lessons.size;
  },
  async brainForget() {
    lessons.clear();
    return brainStatus();
  },

  async voiceStatus() {
    return { ...voiceState, models: [...voiceState.models] };
  },
  async voicePull(model, onProgress) {
    for (let done = 0; done <= 100; done += 5) {
      onProgress({ status: "downloading", completed: done, total: 100 });
      await sleep(60);
    }
    voiceState.models.push(model);
  },
  async voiceChat(_model, messages, onToken) {
    // The demo has no language model: it "speaks" the brain's notes, word by word.
    stopVoice = false;
    const question = messages[messages.length - 1]?.content ?? "";
    const focus = /THE PLAYER IS ASKING ABOUT: (.+?) ·/.exec(messages[0]?.content ?? "")?.[1];
    const answer = focus ? explainOffline(focus, brainScore(focus.replace(/^~/, HOME))) : answerOffline(question, lastItems);
    await sleep(500);
    let said = "";
    for (const word of answer.match(/\S+\s*/g) ?? []) {
      if (stopVoice) break;
      said += word;
      onToken(word);
      await sleep(35);
    }
    return said;
  },
  async voiceStop() {
    stopVoice = true;
  },
};
