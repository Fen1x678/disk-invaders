// The bridge to the Rust side. In a normal browser (npm run dev) a fake backend is used instead,
// so you can work on the UI without building the app, and nothing on your disk is touched.

import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { open, save } from "@tauri-apps/plugin-dialog";
import { openUrl, revealItemInDir } from "@tauri-apps/plugin-opener";

import type {
  BrainScore,
  BrainStatus,
  ChatMessage,
  DeleteOutcome,
  Progress,
  PullProgress,
  ScanOptions,
  ScanReport,
  SystemInfo,
  Target,
  Teaching,
  Thing,
  TreeEntry,
  VoiceStatus,
} from "./types";

export interface Backend {
  /** `true` for the in-browser demo. */
  readonly demo: boolean;
  systemInfo(): Promise<SystemInfo>;
  startScan(roots: string[], options: ScanOptions, onProgress: (p: Progress) => void): Promise<ScanReport>;
  cancelScan(): Promise<void>;
  /** Folder contents from the last scan. `null` lists the scanned roots. */
  listDir(path: string | null): Promise<TreeEntry[]>;
  deleteItems(targets: Target[], permanent: boolean): Promise<DeleteOutcome[]>;
  reveal(path: string): Promise<void>;
  pickFolder(): Promise<string | null>;
  openUrl(url: string): Promise<void>;

  // INVADER BRAIN: the built-in neural network
  brainStatus(): Promise<BrainStatus>;
  brainScore(things: Thing[]): Promise<BrainScore[]>;
  /** Learns from the player's decisions and returns the new status. */
  brainTeach(lessons: Teaching[]): Promise<BrainStatus>;
  /** Asks where to save, writes personal lessons as CSV, returns how many (or null if cancelled). */
  brainExport(): Promise<number | null>;
  brainForget(): Promise<BrainStatus>;

  // Voice: a free local language model in Ollama
  voiceStatus(): Promise<VoiceStatus>;
  voicePull(model: string, onProgress: (p: PullProgress) => void): Promise<void>;
  voiceChat(model: string, messages: ChatMessage[], onToken: (piece: string) => void): Promise<string>;
  voiceStop(): Promise<void>;
}

export const isTauri = "__TAURI_INTERNALS__" in window;

export async function createBackend(): Promise<Backend> {
  if (isTauri) return tauriBackend;
  const { mockBackend } = await import("./mock");
  return mockBackend;
}

const tauriBackend: Backend = {
  demo: false,
  systemInfo: () => invoke<SystemInfo>("system_info"),
  async startScan(roots, options, onProgress) {
    const unlisten = await listen<Progress>("scan-progress", (event) => onProgress(event.payload));
    try {
      return await invoke<ScanReport>("start_scan", { roots, options });
    } finally {
      unlisten();
    }
  },
  cancelScan: () => invoke("cancel_scan"),
  listDir: (path) => invoke<TreeEntry[]>("list_dir", { path }),
  deleteItems: (targets, permanent) =>
    invoke<DeleteOutcome[]>("delete_items", { items: targets.map(({ path, size }) => ({ path, size })), permanent }),
  reveal: (path) => revealItemInDir(path),
  async pickFolder() {
    const picked = await open({ directory: true, multiple: false });
    return typeof picked === "string" ? picked : null;
  },
  openUrl: (url) => openUrl(url),

  brainStatus: () => invoke<BrainStatus>("brain_status"),
  brainScore: (things) => invoke<BrainScore[]>("brain_score", { things }),
  brainTeach: (lessons) => invoke<BrainStatus>("brain_teach", { lessons }),
  async brainExport() {
    const path = await save({ defaultPath: "my-brain-lessons.csv", filters: [{ name: "CSV", extensions: ["csv"] }] });
    return path ? invoke<number>("brain_export", { path }) : null;
  },
  brainForget: () => invoke<BrainStatus>("brain_forget"),

  voiceStatus: () => invoke<VoiceStatus>("voice_status"),
  async voicePull(model, onProgress) {
    const unlisten = await listen<PullProgress>("voice-pull", (event) => onProgress(event.payload));
    try {
      await invoke("voice_pull", { model });
    } finally {
      unlisten();
    }
  },
  async voiceChat(model, messages, onToken) {
    const unlisten = await listen<string>("voice-token", (event) => onToken(event.payload));
    try {
      return await invoke<string>("voice_chat", { model, messages });
    } finally {
      unlisten();
    }
  },
  voiceStop: () => invoke("voice_stop"),
};
