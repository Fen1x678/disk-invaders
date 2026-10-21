// Shared state of the brain's voice (Ollama + a chosen model).

import type { Backend } from "./api";
import { DEFAULT_VOICE, VOICES } from "./brain";
import type { VoiceStatus } from "./types";

const KEY = "disk-invaders:voice";

export const voice = {
  status: null as VoiceStatus | null,
  model: (() => {
    try {
      return localStorage.getItem(KEY) || DEFAULT_VOICE;
    } catch {
      return DEFAULT_VOICE;
    }
  })(),
};

export function setVoiceModel(model: string): void {
  voice.model = model;
  try {
    localStorage.setItem(KEY, model);
  } catch {
    // not remembered, that's fine
  }
}

export async function refreshVoice(backend: Backend): Promise<VoiceStatus> {
  voice.status = await backend.voiceStatus().catch(() => ({ running: false, version: null, models: [] }));
  return voice.status;
}

/** The model to talk with: the chosen one if downloaded, else any of ours, else anything installed. */
export function readyModel(): string | null {
  const installed = voice.status?.running ? voice.status.models : [];
  if (installed.includes(voice.model)) return voice.model;
  return VOICES.map((v) => v.model).find((m) => installed.includes(m)) ?? installed[0] ?? null;
}
