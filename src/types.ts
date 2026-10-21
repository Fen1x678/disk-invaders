// Mirrors the Rust types in crates/core/src (serialized with camelCase field names).

export type Category =
  | "caches"
  | "dev_junk"
  | "logs"
  | "trash"
  | "installers"
  | "old_downloads"
  | "backups"
  | "duplicates"
  | "large_files";

export const CATEGORIES: Category[] = [
  "caches",
  "dev_junk",
  "logs",
  "trash",
  "installers",
  "old_downloads",
  "backups",
  "duplicates",
  "large_files",
];

export type Risk = "safe" | "review";

export interface Item {
  path: string;
  size: number;
  isDir: boolean;
  /** Seconds since the Unix epoch. */
  modified: number | null;
  category: Category;
  risk: Risk;
  label: string;
  group?: number;
  brain?: BrainScore;
}

export interface CategorySummary {
  category: Category;
  count: number;
  bytes: number;
  shown: number;
}

export interface ScanReport {
  roots: string[];
  files: number;
  dirs: number;
  bytes: number;
  errors: number;
  elapsedMs: number;
  cancelled: boolean;
  summaries: CategorySummary[];
  items: Item[];
}

export interface ScanOptions {
  largeFileMinBytes?: number;
  oldDownloadDays?: number;
  findDuplicates?: boolean;
  duplicateMinBytes?: number;
  maxItemsPerCategory?: number;
}

export interface Progress {
  files: number;
  dirs: number;
  bytes: number;
  errors: number;
  hashed: number;
  hashTotal: number;
  phase: "walk" | "duplicates" | "done";
  current: string;
}

export interface Volume {
  name: string;
  mount: string;
  total: number;
  available: number;
  removable: boolean;
}

export type PlatformName = "macos" | "windows" | "linux";

export interface SystemInfo {
  platform: PlatformName;
  home: string | null;
  volumes: Volume[];
  wholeDiskRoots: string[];
}

export interface TreeEntry {
  name: string;
  path: string | null;
  size: number;
  files: number;
  kind: "dir" | "file" | "small_files";
  brain?: BrainScore;
}

export interface DeleteOutcome {
  path: string;
  ok: boolean;
  error?: string;
}

export interface Target {
  path: string;
  size: number;
}

// ---------- INVADER BRAIN ----------

export type Verdict = "delete" | "review" | "keep";

/** The brain's opinion: probabilities that add up to 1. */
export interface BrainScore {
  delete: number;
  review: number;
  keep: number;
  /** The player already told the brain about this exact item. */
  remembered: boolean;
}

export interface BrainStatus {
  level: number;
  xp: number;
  levelXp: number;
  nextLevelXp: number | null;
  schoolLessons: number;
  personalLessons: number;
  /** Final exam score, 0..1. */
  accuracy: number;
  junkWords: string[];
  preciousWords: string[];
}

/** Something on disk the brain can learn from. */
export interface Thing {
  path: string;
  isDir: boolean;
  size: number;
  modified: number | null;
}

export interface Teaching extends Thing {
  label: Verdict;
}

export interface VoiceStatus {
  running: boolean;
  version: string | null;
  models: string[];
}

export interface PullProgress {
  status: string;
  completed: number;
  total: number;
}

export interface ChatMessage {
  role: "system" | "user" | "assistant";
  content: string;
}
