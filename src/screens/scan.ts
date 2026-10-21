import { backdrop, h, type Screen } from "../dom";
import { formatBytes, formatCount } from "../format";
import { t } from "../i18n";
import { sprite } from "../sprites";
import type { Progress } from "../types";

export interface ScanView extends Screen {
  update(progress: Progress): void;
}

interface ScanOptions {
  /** Bytes we expect to find (used space of the disk), or `null` if unknown. */
  expectedBytes: number | null;
  onAbort(): void;
}

export function scanScreen({ expectedBytes, onAbort }: ScanOptions): ScanView {
  const counter = (label: string) => {
    const value = h("span", { class: "counter-value" }, "0");
    return { value, el: h("div", { class: "counter" }, h("span", { class: "counter-label" }, label), value) };
  };
  const files = counter(t("scan.files"));
  const folders = counter(t("scan.folders"));
  const size = counter(t("scan.size"));
  const locked = counter(t("scan.locked"));

  const heading = h("h2", { class: "scan-title" }, t("scan.walking"));
  const fill = h("div", { class: "bar-fill" });
  const bar = h("div", { class: `bar ${expectedBytes ? "" : "indeterminate"}` }, fill);
  const percent = h("span", { class: "bar-label" }, expectedBytes ? "0%" : "");
  const current = h("div", { class: "terminal-line" }, "> ");
  const abort = h("button", { class: "btn danger" }, t("scan.abort"));

  const doAbort = () => {
    abort.setAttribute("disabled", "");
    onAbort();
  };
  abort.addEventListener("click", doAbort);
  const onKey = (e: KeyboardEvent) => {
    if (e.key === "Escape") doAbort();
  };
  document.addEventListener("keydown", onKey);

  const fleet = h(
    "div",
    { class: "fleet" },
    Array.from({ length: 3 }, (_, row) =>
      h("div", { class: "fleet-row" }, Array.from({ length: 8 }, () => sprite((["squid", "crab", "octopus"] as const)[row], `animate tint-${row}`))),
    ),
  );

  const el = h(
    "section",
    { class: "screen scan-screen" },
    backdrop(),
    h(
      "div",
      { class: "scan-content panel" },
      heading,
      fleet,
      h("div", { class: "counters" }, files.el, folders.el, size.el, locked.el),
      h("div", { class: "bar-wrap" }, bar, percent),
      current,
      h("div", { class: "scan-actions" }, abort),
    ),
  );

  return {
    el,
    destroy: () => document.removeEventListener("keydown", onKey),
    update(p) {
      files.value.textContent = formatCount(p.files);
      folders.value.textContent = formatCount(p.dirs);
      size.value.textContent = formatBytes(p.bytes);
      locked.value.textContent = formatCount(p.errors);
      if (p.current) current.textContent = `> ${p.current.length > 80 ? "…" + p.current.slice(-79) : p.current}`;

      let ratio: number | null = null;
      if (p.phase === "duplicates") {
        heading.textContent = t("scan.dupes");
        bar.classList.remove("indeterminate");
        ratio = p.hashTotal ? p.hashed / p.hashTotal : 0;
        current.textContent = `> ${t("scan.checked", { done: formatCount(p.hashed), total: formatCount(p.hashTotal) })}`;
      } else if (expectedBytes) {
        ratio = Math.min(0.99, p.bytes / expectedBytes);
      }
      if (ratio !== null) {
        fill.style.width = `${(ratio * 100).toFixed(1)}%`;
        percent.textContent = `${Math.floor(ratio * 100)}%`;
      }
    },
  };
}
