// The BRAIN screen: level, what it learned, its voice, and a chat.

import type { Backend } from "../api";
import { answerOffline, buildChat, levelTitle, OLLAMA_DOWNLOAD, VOICES } from "../brain";
import { h } from "../dom";
import { formatCount } from "../format";
import { t, tk } from "../i18n";
import { sfx } from "../sound";
import { sprite } from "../sprites";
import type { BrainStatus, ChatMessage, Item } from "../types";
import { readyModel, refreshVoice, setVoiceModel, voice } from "../voice";
import { confirmDialog, toast } from "./dialog";

export interface ChatLine {
  role: "user" | "assistant";
  text: string;
  offline?: boolean;
}

interface BrainViewOptions {
  backend: Backend;
  status: BrainStatus | null;
  chat: ChatLine[];
  items: Item[];
  home: string | null;
  onStatus(status: BrainStatus): void;
}

export function brainView(o: BrainViewOptions): HTMLElement {
  const left = h("div", { class: "brain-side" });
  const voiceBox = h("div", { class: "voice-box" });
  const log = h("div", { class: "chat-log", "aria-live": "polite" });
  const input = h("input", { class: "chat-input", type: "text", placeholder: t("chat.placeholder"), "aria-label": t("chat.placeholder") });
  const send = h("button", { class: "btn primary" }, t("chat.send"));
  const voiceStat = h("span", { class: "stat-value" }, readyModel() ?? t("brain.voiceOff"));
  let busy = false;

  // ---------- left: stats ----------
  const s = o.status;
  if (s) {
    const span = (s.nextLevelXp ?? s.xp) - s.levelXp || 1;
    const filled = s.nextLevelXp ? Math.round(((s.xp - s.levelXp) / span) * 20) : 20;
    left.append(
      h("p", { class: "brain-about" }, t("brain.about", { school: formatCount(s.schoolLessons) })),
      h(
        "div",
        { class: "xp" },
        h("div", { class: "xp-head" }, h("span", {}, t("brain.level", { n: s.level }), " · ", levelTitle(s.level)), h("span", {}, s.nextLevelXp ? t("brain.xp", { xp: s.xp, next: s.nextLevelXp }) : t("brain.maxLevel"))),
        h("div", { class: "cells ok" }, Array.from({ length: 20 }, (_, i) => h("span", { class: i < filled ? "on" : "" }))),
      ),
      h(
        "div",
        { class: "brain-stats" },
        stat(t("brain.exam"), `${Math.round(s.accuracy * 100)}%`),
        stat(t("brain.school"), formatCount(s.schoolLessons)),
        stat(t("brain.yours"), formatCount(s.personalLessons)),
        h("div", { class: "stat" }, h("span", { class: "stat-label" }, t("brain.voice")), voiceStat),
      ),
      words(t("brain.junkWords"), s.junkWords, "junk"),
      words(t("brain.preciousWords"), s.preciousWords, "precious"),
    );
  }
  left.append(
    voiceBox,
    h(
      "div",
      { class: "brain-actions" },
      h("button", { class: "btn small", onclick: () => void exportLessons() }, t("brain.export")),
      h("button", { class: "btn small danger", onclick: () => void forget() }, t("brain.forget")),
    ),
  );

  // ---------- voice setup ----------
  async function renderVoice(check = false) {
    if (check || !voice.status) {
      voiceBox.replaceChildren(h("p", { class: "muted" }, "…"));
      await refreshVoice(o.backend);
    }
    const status = voice.status!;
    voiceStat.textContent = readyModel() ?? t("brain.voiceOff");
    if (!status.running) {
      voiceBox.replaceChildren(
        h("h3", {}, t("voice.offTitle")),
        h("p", { class: "muted" }, t("voice.offText")),
        h(
          "div",
          { class: "voice-buttons" },
          h("button", { class: "btn small primary", onclick: () => void o.backend.openUrl(OLLAMA_DOWNLOAD) }, t("voice.getOllama")),
          h("button", { class: "btn small", onclick: () => void renderVoice(true) }, t("voice.check")),
        ),
      );
      return;
    }
    const options = VOICES.map((v) =>
      h(
        "button",
        {
          class: `voice-option ${voice.model === v.model ? "active" : ""}`,
          onclick: () => {
            setVoiceModel(v.model);
            sfx.blip();
            void renderVoice();
          },
        },
        h("span", {}, tk(v.tier)),
        h("small", {}, `${v.model} · ${status.models.includes(v.model) ? "✓" : v.size}`),
      ),
    );
    const chosen = VOICES.find((v) => v.model === voice.model);
    const needsDownload = chosen && !status.models.includes(chosen.model);
    voiceBox.replaceChildren(
      h("h3", {}, t("voice.pick")),
      h("div", { class: "voice-options" }, options),
      needsDownload ? h("button", { class: "btn small primary", onclick: () => void download(chosen.model) }, t("voice.download", { size: chosen.size })) : h("p", { class: "voice-ready" }, t("voice.ready", { model: readyModel() ?? "?" })),
    );
  }

  async function download(model: string) {
    const fill = h("div", { class: "bar-fill" });
    const label = h("p", { class: "muted" }, t("voice.downloading", { pct: 0 }));
    voiceBox.replaceChildren(h("h3", {}, t("voice.pick")), h("div", { class: "bar" }, fill), label);
    try {
      await o.backend.voicePull(model, (p) => {
        const pct = p.total ? Math.floor((p.completed / p.total) * 100) : 0;
        fill.style.width = `${pct}%`;
        label.textContent = t("voice.downloading", { pct });
      });
      sfx.coin();
    } catch (error) {
      toast(t("voice.failed", { error: String(error) }), "bad");
    }
    await renderVoice(true);
  }

  // ---------- chat ----------
  function renderLog() {
    log.replaceChildren(
      ...o.chat.map((line) =>
        h(
          "div",
          { class: `bubble ${line.role}` },
          line.role === "assistant" ? sprite("brain", "bubble-face") : null,
          h("div", {}, h("p", {}, line.text || t("brain.thinking")), line.offline ? h("small", { class: "muted" }, t("chat.offline")) : null),
        ),
      ),
    );
    log.scrollTop = log.scrollHeight;
  }

  async function ask(question: string) {
    question = question.trim();
    if (!question || busy) return;
    busy = true;
    input.value = "";
    const history: ChatMessage[] = o.chat.slice(1).filter((l) => !l.offline).map((l) => ({ role: l.role, content: l.text }));
    o.chat.push({ role: "user", text: question });
    const answer: ChatLine = { role: "assistant", text: "" };
    o.chat.push(answer);
    renderLog();
    sfx.blip();
    const model = readyModel();
    try {
      if (!model) throw new Error("no voice");
      send.textContent = t("chat.stop");
      await o.backend.voiceChat(model, buildChat(question, history, o.items, o.home), (piece) => {
        answer.text += piece;
        renderLog();
      });
    } catch {
      answer.text = answerOffline(question, o.items);
      answer.offline = true;
    }
    send.textContent = t("chat.send");
    busy = false;
    renderLog();
  }

  send.addEventListener("click", () => (busy ? void o.backend.voiceStop() : void ask(input.value)));
  input.addEventListener("keydown", (e) => {
    if (e.key === "Enter") {
      e.preventDefault();
      void ask(input.value);
    }
  });
  const suggestions = h(
    "div",
    { class: "chat-suggest" },
    (["chat.suggest1", "chat.suggest2", "chat.suggest3"] as const).map((key) => h("button", { class: "chip", onclick: () => void ask(t(key)) }, t(key))),
  );

  async function exportLessons() {
    sfx.blip();
    const n = await o.backend.brainExport().catch(() => null);
    if (n !== null) toast(t("brain.exported", { n }), "good");
  }

  async function forget() {
    if (!(await confirmDialog({ title: t("brain.forget"), lines: [t("brain.forgetConfirm")], danger: true }))) return;
    o.onStatus(await o.backend.brainForget());
    toast(t("brain.forgot"));
  }

  if (!o.chat.length) o.chat.push({ role: "assistant", text: t("chat.hello") });
  renderLog();
  void renderVoice();

  return h(
    "div",
    { class: "brain-view" },
    left,
    h("div", { class: "chat" }, log, suggestions, h("div", { class: "chat-bar" }, input, send)),
  );
}

function stat(label: string, value: string): HTMLElement {
  return h("div", { class: "stat" }, h("span", { class: "stat-label" }, label), h("span", { class: "stat-value" }, value));
}

function words(title: string, list: string[], kind: string): HTMLElement {
  return h("div", { class: "words" }, h("h3", {}, title), h("div", { class: "chips" }, list.map((w) => h("span", { class: `chip ${kind}` }, w))));
}
