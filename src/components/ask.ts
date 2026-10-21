// "BRAIN SAYS": the brain explains one item. With a voice it streams an answer, without one it reads its notes.

import type { Backend } from "../api";
import { buildChat, explainOffline, type Focus } from "../brain";
import { h } from "../dom";
import { baseName, shortenHome } from "../format";
import { t } from "../i18n";
import { sfx } from "../sound";
import { sprite } from "../sprites";
import type { Item } from "../types";
import { readyModel, refreshVoice, voice } from "../voice";

export async function askBrain(backend: Backend, focus: Focus, items: Item[], home: string | null): Promise<void> {
  const text = h("div", { class: "says-text" });
  const note = h("p", { class: "says-note muted" });
  const stop = h("button", { class: "btn" }, t("chat.stop"));
  const ok = h("button", { class: "btn primary" }, t("done.ok"));
  const box = h(
    "div",
    { class: "dialog says", role: "dialog", "aria-modal": "true", "aria-label": t("brain.says") },
    h("div", { class: "says-head" }, sprite("brain", "animate big"), h("div", {}, h("h2", {}, t("brain.says")), h("p", { class: "muted" }, baseName(focus.path)))),
    text,
    note,
    h("div", { class: "dialog-buttons" }, stop, ok),
  );
  const overlay = h("div", { class: "overlay" }, box);
  document.body.appendChild(overlay);

  let open = true;
  const close = () => {
    if (!open) return;
    open = false;
    void backend.voiceStop();
    document.removeEventListener("keydown", onKey, true);
    overlay.remove();
  };
  const onKey = (e: KeyboardEvent) => {
    if (e.key === "Escape" || e.key === "Enter") {
      e.preventDefault();
      close();
    }
  };
  document.addEventListener("keydown", onKey, true);
  ok.addEventListener("click", close);
  stop.addEventListener("click", () => void backend.voiceStop());
  ok.focus();
  sfx.blip();

  const offline = () => {
    text.textContent = explainOffline(shortenHome(focus.path, home), focus.brain);
    note.textContent = t("chat.offline");
    stop.remove();
  };

  if (!voice.status) {
    text.textContent = t("brain.thinking");
    await refreshVoice(backend);
  }
  const model = readyModel();
  if (!model) return offline();
  text.textContent = t("brain.thinking");
  text.classList.add("thinking");
  let started = false;
  const question = t("brain.askAbout", { name: baseName(focus.path) });
  backend
    .voiceChat(model, buildChat(question, [], items, home, focus), (piece) => {
      if (!open) return;
      if (!started) {
        started = true;
        text.textContent = "";
        text.classList.remove("thinking");
      }
      text.textContent += piece;
      text.scrollTop = text.scrollHeight;
    })
    .then(() => {
      stop.remove();
      note.textContent = t("voice.ready", { model });
    })
    .catch((error) => {
      if (!open) return;
      text.classList.remove("thinking");
      offline();
      note.textContent = t("voice.failed", { error: String(error) });
    });
}
