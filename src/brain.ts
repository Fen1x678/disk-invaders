// What the brain knows and how it talks.
//
// - The neural network itself lives in Rust (crates/core/src/brain) and gives each item a score.
// - The knowledge cards (brain/knowledge.json) explain common folders in plain words.
// - With a voice (a free local model in Ollama), the cards and the scan become the model's context,
//   so it can answer free-form questions. Without one, the brain answers from its cards.

import knowledge from "../brain/knowledge.json";
import { baseName, formatAge, formatBytes, shortenHome } from "./format";
import { getLang, t, tk } from "./i18n";
import type { BrainScore, ChatMessage, Item, Verdict } from "./types";

export interface Card {
  id: string;
  match: string[];
  verdict: Verdict;
  en: string;
  ru: string;
}

export const CARDS = knowledge as Card[];

/** Free voices (Apache 2.0 licensed models), smallest first. */
export const VOICES = [
  { model: "qwen3:0.6b", size: "0.5 GB", tier: "voice.lite" },
  { model: "qwen3:1.7b", size: "1.4 GB", tier: "voice.smart" },
  { model: "qwen3:4b", size: "2.5 GB", tier: "voice.genius" },
] as const;
export const DEFAULT_VOICE = "qwen3:1.7b";
export const OLLAMA_DOWNLOAD = "https://ollama.com/download";

const LEVEL_TITLES = [
  "NEWBORN", "LARVA", "CADET", "SCOUT", "PILOT", "SNIPER",
  "ACE", "COMMANDER", "ADMIRAL", "MASTERMIND", "ORACLE", "GALAXY BRAIN",
];

export function levelTitle(level: number): string {
  return LEVEL_TITLES[Math.min(Math.max(level, 1), LEVEL_TITLES.length) - 1];
}

/** Below this confidence the brain admits it isn't sure. */
const SURE = 0.6;

export function confidence(score: BrainScore): number {
  return Math.max(score.delete, score.review, score.keep);
}

export function verdictOf(score?: BrainScore): Verdict | "unsure" | null {
  if (!score) return null;
  const options: [Verdict, number][] = [
    ["delete", score.delete],
    ["review", score.review],
    ["keep", score.keep],
  ];
  const [best, p] = options.reduce((a, b) => (b[1] > a[1] ? b : a));
  return p >= SURE ? best : "unsure";
}

export const verdictName = (verdict: Verdict | "unsure" | null) => (verdict ? tk(`verdict.${verdict}`) : "");

/** Items the brain would shoot, biggest first. */
export function brainPicks(items: Item[]): Item[] {
  const seen = new Set<string>();
  return items
    .filter((i) => i.brain && !i.brain.remembered && verdictOf(i.brain) === "delete" && i.brain.delete >= 0.75)
    .filter((i) => (seen.has(i.path) ? false : (seen.add(i.path), true)))
    .sort((a, b) => b.size - a.size);
}

// ---------- knowledge ----------

const normalize = (text: string) => text.toLowerCase().replace(/\\/g, "/");

/** Knowledge cards that match a path or a question, most specific first. */
export function findCards(text: string, limit = 2): Card[] {
  const haystack = normalize(text);
  return CARDS.map((card) => ({ card, hit: Math.max(0, ...card.match.filter((m) => haystack.includes(m)).map((m) => m.length)) }))
    .filter((x) => x.hit > 0)
    .sort((a, b) => b.hit - a.hit)
    .slice(0, limit)
    .map((x) => x.card);
}

export const cardText = (card: Card) => (getLang() === "ru" ? card.ru : card.en);

/** The brain's opinion in words, without a voice. Pass the path with home shortened to `~`. */
export function explainOffline(path: string, score?: BrainScore): string {
  const parts: string[] = [];
  const verdict = verdictOf(score);
  if (score?.remembered) parts.push(t("brain.youTold", { verdict: verdictName(verdict) }));
  else if (score && verdict === "unsure") parts.push(t("brain.notSure"));
  else if (score) parts.push(t("brain.iThink", { verdict: verdictName(verdict), pct: Math.round(confidence(score) * 100) }));
  const card = findCards(path, 1)[0];
  parts.push(card ? cardText(card) : t("offline.unknownItem"));
  return parts.join(" ");
}

/** Answers a chat question from the knowledge cards and the scan, without a voice. */
export function answerOffline(question: string, items: Item[]): string {
  const cards = findCards(question);
  if (cards.length) return cards.map(cardText).join("\n\n");
  if (/(удал|delete|remove|clean|free|освобод|почист|мусор|junk|space|мест|what first|сначала)/i.test(question)) {
    return topPicks(items);
  }
  return t("offline.dontKnow");
}

function topPicks(items: Item[]): string {
  const picks = brainPicks(items).slice(0, 5);
  if (!picks.length) return t("offline.nothing");
  const total = picks.reduce((sum, i) => sum + i.size, 0);
  const lines = picks.map((i) => `• ${baseName(i.path)} — ${formatBytes(i.size)}`);
  return `${t("offline.top", { size: formatBytes(total) })}\n${lines.join("\n")}`;
}

// ---------- the voice's prompt ----------

const SYSTEM_PROMPT = `You are INVADER BRAIN, the friendly assistant inside Disk Invaders, a retro arcade disk cleaner for macOS and Windows.
You help the player decide what to delete to free disk space. You only see file and folder names, sizes and ages, never their contents.
Rules:
- Be brief: at most 5 short sentences or a short list. Mention sizes.
- Explain WHY something is safe or not (a cache that rebuilds itself, an old installer, personal photos...).
- Never recommend deleting system files, keys or passwords, photos, documents or source code.
- If you are not sure, say "check it first". Never invent files that are not in the SCAN.
- Use the FACTS. A brain verdict is the opinion of a small neural network: trust it for typical junk, double-check surprises.`;

function verdictTag(score?: BrainScore): string {
  if (!score) return "?";
  const verdict = verdictOf(score);
  if (score.remembered) return `PLAYER SAID ${verdict?.toUpperCase()}`;
  return verdict === "unsure" ? "UNSURE" : `${verdict?.toUpperCase()} ${Math.round(confidence(score) * 100)}%`;
}

export interface Focus {
  path: string;
  size: number;
  label?: string;
  modified?: number | null;
  brain?: BrainScore;
}

/** Builds the conversation for the local model: rules + facts + what the scan found + the question. */
export function buildChat(question: string, history: ChatMessage[], items: Item[], home: string | null, focus?: Focus): ChatMessage[] {
  const unique = new Map<string, Item>();
  items.forEach((i) => unique.set(i.path, i));
  const top = [...unique.values()].sort((a, b) => b.size - a.size).slice(0, 40);
  const scan = top.map((i) => `[${verdictTag(i.brain)}] ${formatBytes(i.size)} · ${formatAge(i.modified) || "?"} · ${i.label} · ${shortenHome(i.path, home)}`);

  const cards = new Map<string, Card>();
  const near = (path: string) => shortenHome(path, home);
  [...findCards(question, 3), ...(focus ? findCards(near(focus.path), 3) : []), ...top.slice(0, 15).flatMap((i) => findCards(near(i.path), 1))]
    .forEach((c) => cards.set(c.id, c));
  const facts = [...cards.values()].slice(0, 10).map((c) => `- ${c.en} (usual verdict: ${c.verdict})`);

  const language = getLang() === "ru" ? "Russian" : "English";
  let system = `${SYSTEM_PROMPT}\nAlways answer in ${language}.\n\nFACTS:\n${facts.join("\n") || "- (none)"}\n\nSCAN (biggest items found, brain verdict in brackets):\n${scan.join("\n") || "(nothing scanned yet)"}`;
  if (focus) {
    system += `\n\nTHE PLAYER IS ASKING ABOUT: ${shortenHome(focus.path, home)} · ${formatBytes(focus.size)} · ${formatAge(focus.modified ?? null) || "?"} · ${focus.label ?? ""} · brain: ${verdictTag(focus.brain)}`;
  }
  return [{ role: "system", content: system }, ...history.slice(-6), { role: "user", content: question }];
}
