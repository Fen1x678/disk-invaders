import { h } from "../dom";
import { t } from "../i18n";
import { sfx } from "../sound";

interface DialogOptions {
  title: string;
  lines: string[];
  /** Monospace list under the text (paths, errors). */
  list?: string[];
  danger?: boolean;
}

/** Retro "ARE YOU SURE? Y/N" box. Resolves to `true` for yes. Keys: Y / N / Enter / Esc. */
export function confirmDialog(options: DialogOptions): Promise<boolean> {
  return new Promise((resolve) => {
    const yes = h("button", { class: `btn ${options.danger ? "danger" : "primary"}` }, t("confirm.yes"));
    const no = h("button", { class: "btn" }, t("confirm.no"));
    const close = open(options, [no, yes], (answer) => resolve(answer));
    yes.addEventListener("click", () => close(true));
    no.addEventListener("click", () => close(false));
    // Make the safe answer the default for scary actions.
    (options.danger ? no : yes).focus();
  });
}

export function messageDialog(options: DialogOptions): Promise<void> {
  return new Promise((resolve) => {
    const ok = h("button", { class: "btn primary" }, t("done.ok"));
    const close = open(options, [ok], () => resolve());
    ok.addEventListener("click", () => close(true));
    ok.focus();
  });
}

function open(options: DialogOptions, buttons: HTMLElement[], done: (answer: boolean) => void): (answer: boolean) => void {
  const box = h(
    "div",
    { class: `dialog ${options.danger ? "dialog-danger" : ""}`, role: "dialog", "aria-modal": "true", "aria-label": options.title },
    h("h2", {}, options.title),
    options.lines.map((line) => h("p", {}, line)),
    options.list?.length ? h("ul", { class: "dialog-list" }, options.list.map((item) => h("li", {}, item))) : null,
    h("div", { class: "dialog-buttons" }, buttons),
  );
  const overlay = h("div", { class: "overlay" }, box);
  document.body.appendChild(overlay);

  const onKey = (e: KeyboardEvent) => {
    const key = e.key.toLowerCase();
    if (key === "escape" || key === "n") close(false);
    else if (key === "y") close(true);
    else return;
    e.preventDefault();
    e.stopPropagation();
  };
  document.addEventListener("keydown", onKey, true);

  function close(answer: boolean) {
    document.removeEventListener("keydown", onKey, true);
    overlay.remove();
    if (!answer) sfx.blip();
    done(answer);
  }
  return close;
}

/** Message that slides in at the bottom and goes away by itself. */
export function toast(text: string, kind: "info" | "good" | "bad" = "info"): void {
  const el = h("div", { class: `toast toast-${kind}`, role: "status" }, text);
  el.style.marginTop = `${document.querySelectorAll(".toast").length * 58}px`;
  document.body.appendChild(el);
  setTimeout(() => el.classList.add("leaving"), 3500);
  setTimeout(() => el.remove(), 4000);
}
