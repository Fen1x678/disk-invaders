import type { Backend } from "../api";
import { backdrop, h, type Screen } from "../dom";
import { formatBytes } from "../format";
import { t } from "../i18n";
import { sfx } from "../sound";
import { sprite } from "../sprites";
import type { SystemInfo, Volume } from "../types";

export type Scope = "whole" | "home" | "custom";

export interface TitleState {
  scope: Scope;
  customPath: string | null;
  findDuplicates: boolean;
}

interface TitleOptions {
  info: SystemInfo;
  backend: Backend;
  state: TitleState;
  /** `expectedBytes` lets the scan screen show a real percentage when it is known. */
  onStart(roots: string[], expectedBytes: number | null): void;
}

const FULL_DISK_ACCESS = "x-apple.systempreferences:com.apple.preference.security?Privacy_AllFiles";

export function titleScreen({ info, backend, state, onStart }: TitleOptions): Screen {
  const scopeBox = h("div", { class: "scope-options", role: "radiogroup", "aria-label": t("title.area") });

  const option = (scope: Scope, label: string, hint: string) =>
    h(
      "button",
      {
        class: `scope-option ${state.scope === scope ? "active" : ""}`,
        role: "radio",
        "aria-checked": String(state.scope === scope),
        onclick: async () => {
          if (scope === "custom") {
            const picked = await backend.pickFolder();
            if (!picked) return;
            state.customPath = picked;
          }
          state.scope = scope;
          sfx.blip();
          renderScopes();
        },
      },
      h("span", { class: "scope-marker" }, state.scope === scope ? "▶" : ""),
      h("span", { class: "scope-label" }, label),
      h("span", { class: "scope-hint" }, hint),
    );

  function renderScopes() {
    scopeBox.replaceChildren(
      option("whole", t("scope.whole"), t("scope.wholeHint")),
      option("home", t("scope.home"), info.home ?? "~"),
      option("custom", t("scope.custom"), state.customPath ?? t("scope.customHint")),
    );
  }
  renderScopes();

  const dupes = h("input", { type: "checkbox", checked: state.findDuplicates });
  dupes.addEventListener("change", () => {
    state.findDuplicates = dupes.checked;
    sfx.blip();
  });

  async function start() {
    let roots: string[];
    let expected: number | null = null;
    if (state.scope === "whole") {
      roots = info.wholeDiskRoots;
      const used = info.volumes.filter((v) => roots.includes(v.mount)).reduce((sum, v) => sum + v.total - v.available, 0);
      expected = used > 0 ? used : null;
    } else if (state.scope === "home" && info.home) {
      roots = [info.home];
    } else if (state.customPath) {
      roots = [state.customPath];
    } else {
      const picked = await backend.pickFolder();
      if (!picked) return;
      roots = [(state.customPath = picked)];
    }
    onStart(roots, expected);
  }

  const onKey = (e: KeyboardEvent) => {
    if (e.key === "Enter" && !(e.target instanceof HTMLButtonElement || e.target instanceof HTMLInputElement)) {
      e.preventDefault();
      void start();
    }
  };
  document.addEventListener("keydown", onKey);

  const invaders = h(
    "div",
    { class: "invader-row" },
    (["squid", "crab", "octopus", "crab", "squid"] as const).map((name, i) => sprite(name, `animate tint-${i % 3}`)),
  );

  const el = h(
    "section",
    { class: "screen title-screen" },
    backdrop(),
    h(
      "div",
      { class: "title-content" },
      invaders,
      h("h1", { class: "logo" }, h("span", { class: "logo-top" }, "DISK"), h("span", { class: "logo-bottom" }, "INVADERS")),
      h("p", { class: "tagline" }, t("app.tagline")),
      h(
        "div",
        { class: "title-panels" },
        h("div", { class: "panel" }, h("h2", {}, t("title.disks")), info.volumes.map(volumeRow)),
        h(
          "div",
          { class: "panel" },
          h("h2", {}, t("title.area")),
          scopeBox,
          h("label", { class: "check" }, dupes, h("span", {}, t("title.dupes"))),
        ),
      ),
      h("button", { class: "start-button", onclick: () => void start() }, "▶ ", t("title.start")),
      h("p", { class: "hint" }, t("title.startHint"), " · ", t("title.howto")),
      info.platform === "macos" && !backend.demo
        ? h(
            "p",
            { class: "tip" },
            t("title.fda"),
            " ",
            h("button", { class: "link", onclick: () => void backend.openUrl(FULL_DISK_ACCESS) }, t("title.fdaButton")),
          )
        : null,
      backend.demo ? h("p", { class: "demo-banner" }, t("app.demo")) : null,
    ),
  );

  return { el, destroy: () => document.removeEventListener("keydown", onKey) };
}

function volumeRow(volume: Volume): HTMLElement {
  const used = volume.total - volume.available;
  const ratio = volume.total ? used / volume.total : 0;
  const cells = 24;
  const filled = Math.round(ratio * cells);
  const level = ratio > 0.9 ? "critical" : ratio > 0.75 ? "warning" : "ok";
  return h(
    "div",
    { class: "volume" },
    h("div", { class: "volume-head" }, h("span", { class: "volume-name" }, volume.name), h("span", { class: "volume-pct" }, `${Math.round(ratio * 100)}%`)),
    h(
      "div",
      { class: `cells ${level}`, role: "meter", "aria-valuenow": Math.round(ratio * 100), "aria-valuemin": 0, "aria-valuemax": 100 },
      Array.from({ length: cells }, (_, i) => h("span", { class: i < filled ? "on" : "" })),
    ),
    h("div", { class: "volume-free" }, t("title.diskFree", { free: formatBytes(volume.available), total: formatBytes(volume.total) })),
  );
}
