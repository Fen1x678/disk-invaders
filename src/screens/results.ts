import type { Backend } from "../api";
import { brainPicks, confidence, levelTitle, verdictName, verdictOf } from "../brain";
import { askBrain } from "../components/ask";
import { brainView, type ChatLine } from "../components/brainView";
import { confirmDialog, messageDialog, toast } from "../components/dialog";
import type { Hud } from "../components/hud";
import { treemap } from "../components/treemap";
import { floatText, h, type Screen } from "../dom";
import { baseName, formatAge, formatBytes, formatCount, formatDuration, isInside, parentPath, shortenHome } from "../format";
import { t, tk } from "../i18n";
import { Selection, type Selected } from "../selection";
import { sfx } from "../sound";
import { CATEGORY_SPRITE, sprite, type SpriteName } from "../sprites";
import {
  CATEGORIES,
  type BrainScore,
  type BrainStatus,
  type Category,
  type DeleteOutcome,
  type Item,
  type ScanReport,
  type SystemInfo,
  type Teaching,
  type Thing,
  type TreeEntry,
} from "../types";

const PAGE = 150;

/** Lives outside the screen so switching language (which re-renders) keeps everything. */
export interface ResultsState {
  report: ScanReport;
  selection: Selection;
  view: Category | "explorer" | "brain";
  filter: string;
  limit: number;
  explorerPath: string | null;
  permanent: boolean;
  oldDownloadDays: number;
  brain: BrainStatus | null;
  chat: ChatLine[];
}

/** Levels where everything is re-created automatically: a friendly place to start. */
const SAFE_LEVELS: Category[] = ["dev_junk", "caches", "logs", "trash"];

export function createResultsState(report: ScanReport, oldDownloadDays: number): ResultsState {
  const found = report.summaries.filter((s) => s.count > 0).sort((a, b) => b.bytes - a.bytes);
  const start = found.find((s) => SAFE_LEVELS.includes(s.category)) ?? found[0];
  return {
    report,
    selection: new Selection(),
    view: start?.category ?? "explorer",
    filter: "",
    limit: PAGE,
    explorerPath: null,
    permanent: false,
    oldDownloadDays,
    brain: null,
    chat: [],
  };
}

interface ResultsOptions {
  state: ResultsState;
  info: SystemInfo;
  backend: Backend;
  hud: Hud;
  onNewGame(): void;
}

export function resultsScreen({ state, info, backend, hud, onNewGame }: ResultsOptions): Screen {
  const { report, selection } = state;
  const short = (path: string) => shortenHome(path, info.home);
  const summaryOf = (c: Category) => report.summaries.find((s) => s.category === c)!;

  const sidebar = h("aside", { class: "levels panel" });
  const arena = h("div", { class: "arena panel" });
  const firebar = h("footer", { class: "firebar" });
  const el = h("section", { class: "screen results-screen" }, sidebar, arena, firebar);

  // ---------- sidebar ----------

  function renderSidebar() {
    sidebar.replaceChildren(
      h(
        "div",
        { class: "scan-summary" },
        h("div", {}, t("results.summary", { files: formatCount(report.files), size: formatBytes(report.bytes), time: formatDuration(report.elapsedMs) })),
        report.errors ? h("div", { class: "muted" }, t("results.locked", { n: formatCount(report.errors) })) : null,
        report.cancelled ? h("div", { class: "warn" }, t("results.cancelled")) : null,
      ),
      h("h2", {}, t("results.levels")),
      h(
        "nav",
        { class: "level-list" },
        CATEGORIES.map((c) => {
          const s = summaryOf(c);
          return levelButton(c, tk(`cat.${c}`), CATEGORY_SPRITE[c], s.count ? formatBytes(s.bytes) : "—", s.count === 0);
        }),
        levelButton("explorer", t("results.explorer"), "eye", formatBytes(report.bytes), false),
        levelButton("brain", t("results.brain"), "brain", state.brain ? t("brain.level", { n: state.brain.level }) : "…", false),
      ),
      h("button", { class: "btn new-game", onclick: onNewGame }, "↺ ", t("results.newGame")),
    );
  }

  function levelButton(view: ResultsState["view"], label: string, icon: SpriteName, size: string, empty: boolean) {
    const active = state.view === view;
    return h(
      "button",
      { class: `level level-${view} ${active ? "active" : ""} ${empty ? "is-empty" : ""}`, "aria-current": active ? "true" : null, onclick: () => go(view) },
      sprite(icon, active ? "animate" : ""),
      h("span", { class: "level-name" }, label),
      h("span", { class: "level-size" }, size),
    );
  }

  function go(view: ResultsState["view"]) {
    state.view = view;
    state.filter = "";
    state.limit = PAGE;
    sfx.blip();
    renderSidebar();
    renderArena();
  }

  function renderArena() {
    if (state.view === "explorer") void renderExplorer();
    else if (state.view === "brain") renderBrain();
    else renderLevel(state.view);
  }

  // ---------- the brain ----------

  function renderBrain() {
    const level = state.brain ? `${t("brain.level", { n: state.brain.level })} · ${levelTitle(state.brain.level)}` : "";
    arena.replaceChildren(
      h(
        "header",
        { class: "arena-head" },
        h("div", { class: "arena-title" }, sprite("brain", "animate big"), h("h2", {}, t("brain.title")), h("span", { class: "arena-total" }, level)),
      ),
      brainView({
        backend,
        status: state.brain,
        chat: state.chat,
        items: report.items,
        home: info.home,
        onStatus(status) {
          state.brain = status;
          void rescore().then(() => {
            renderSidebar();
            renderArena();
          });
        },
      }),
    );
  }

  const thingOf = (x: Thing): Thing => ({ path: x.path, isDir: x.isDir, size: x.size, modified: x.modified });

  /** Asks the brain again about everything (after it learned something). */
  async function rescore() {
    const scores = await backend.brainScore(report.items.map(thingOf));
    report.items.forEach((item, k) => (item.brain = scores[k]));
  }

  /** Teaches the brain, then celebrates. Learning is best-effort: cleaning works without it. */
  async function teach(lessons: Teaching[], message: string) {
    if (!lessons.length) return;
    const before = state.brain?.level ?? 0;
    try {
      state.brain = await backend.brainTeach(lessons);
      await rescore();
      toast(message, "good");
      const level = state.brain.level;
      if (before && level > before) {
        setTimeout(() => {
          sfx.victory();
          floatText(t("brain.levelUp", { n: level }), window.innerWidth / 2, window.innerHeight / 3, "float-text stage");
        }, 1600);
      }
    } catch (error) {
      console.warn("the brain could not learn", error);
    }
  }

  async function keep(thing: Thing) {
    sfx.select();
    selection.remove([thing.path]);
    await teach([{ ...thingOf(thing), label: "keep" }], t("brain.learnedKeep"));
    renderSidebar();
    renderArena();
  }

  function brainChip(score?: BrainScore): HTMLElement {
    const verdict = verdictOf(score);
    if (!score || !verdict) return h("span", { class: "brain-chip none" });
    const pct = (p: number) => Math.round(p * 100);
    const title = t("brain.chip", { d: pct(score.delete), r: pct(score.review), k: pct(score.keep) }) + (score.remembered ? ` · ${t("brain.remembered")}` : "");
    return h(
      "span",
      { class: `brain-chip v-${verdict} ${score.remembered ? "remembered" : ""}`, title },
      h("small", {}, verdictName(verdict)),
      h("b", {}, score.remembered ? "★" : verdict === "unsure" ? "?" : `${pct(confidence(score))}%`),
    );
  }

  function rowActions(thing: Thing, label: string, score?: BrainScore): HTMLElement {
    const ask = () => void askBrain(backend, { path: thing.path, size: thing.size, label, modified: thing.modified, brain: score }, report.items, info.home);
    return h(
      "span",
      { class: "row-actions" },
      iconButton("ask", t("brain.ask"), ask),
      iconButton("heart", t("brain.keep"), () => void keep(thing)),
      iconButton("eye", t("list.reveal"), () => void backend.reveal(thing.path)),
    );
  }

  // ---------- one level (category) ----------

  function renderLevel(c: Category) {
    const summary = summaryOf(c);
    const visible = () => {
      const needle = state.filter.toLowerCase();
      const all = report.items.filter((i) => i.category === c);
      return needle ? all.filter((i) => i.path.toLowerCase().includes(needle) || i.label.toLowerCase().includes(needle)) : all;
    };
    const pick = (items: Item[]) => {
      selection.add(items.map(toSelected));
      sfx.select();
      renderList();
    };

    const filter = h("input", { class: "filter", type: "search", placeholder: t("list.filter"), value: state.filter, "aria-label": t("list.filter") });
    let timer = 0;
    filter.addEventListener("input", () => {
      clearTimeout(timer);
      timer = window.setTimeout(() => {
        state.filter = filter.value;
        state.limit = PAGE;
        renderList();
      }, 150);
    });

    const toolbar = h(
      "div",
      { class: "toolbar" },
      h("button", { class: "btn small", onclick: () => pick(visible().filter((i) => i.risk === "safe")) }, t("list.selectSafe")),
      h("button", { class: "btn small", onclick: () => pick(visible()) }, t("list.selectAll")),
      c === "duplicates" ? h("button", { class: "btn small", onclick: keepOneOfEach }, t("list.keepOne")) : null,
      h(
        "button",
        {
          class: "btn small brain-btn",
          onclick: () => {
            const picks = brainPicks(visible());
            if (picks.length) pick(picks);
            else {
              sfx.error();
              toast(t("brain.picksNone"));
            }
          },
        },
        sprite("brain"),
        t("brain.picks"),
      ),
      h("button", { class: "btn small", onclick: () => { selection.clear(); sfx.deselect(); renderList(); } }, t("list.clear")),
      filter,
    );

    const list = h("div", { class: "list" });
    arena.replaceChildren(
      h(
        "header",
        { class: "arena-head" },
        h("div", { class: "arena-title" }, sprite(CATEGORY_SPRITE[c], "animate big"), h("h2", {}, tk(`cat.${c}`)), h("span", { class: "arena-total" }, formatBytes(summary.bytes))),
        h("p", { class: "desc" }, tk(`desc.${c}`, { days: state.oldDownloadDays })),
        toolbar,
      ),
      list,
    );
    renderList();

    function keepOneOfEach() {
      const items = visible();
      const firsts = new Set<number | undefined>();
      const extra = items.filter((i) => (firsts.has(i.group) ? true : (firsts.add(i.group), false)));
      selection.remove(items.filter((i) => !extra.includes(i)).map((i) => i.path));
      pick(extra);
    }

    function renderList() {
      const items = visible();
      if (!items.length) {
        list.replaceChildren(h("div", { class: "empty" }, sprite("ufo", "animate big"), h("p", {}, t("list.empty"))));
        return;
      }
      const max = Math.max(...items.map((i) => i.size));
      const groupSizes = new Map<number | undefined, number>();
      items.forEach((i) => groupSizes.set(i.group, (groupSizes.get(i.group) ?? 0) + 1));

      const rows: Node[] = [];
      let lastGroup: number | undefined = -1;
      for (const item of items.slice(0, state.limit)) {
        if (c === "duplicates" && item.group !== lastGroup) {
          lastGroup = item.group;
          rows.push(h("div", { class: "group-head" }, t("list.group", { n: (item.group ?? 0) + 1, count: groupSizes.get(item.group) ?? 0, size: formatBytes(item.size) })));
        }
        rows.push(itemRow(item, max));
      }
      if (items.length > state.limit) {
        const more = Math.min(PAGE, items.length - state.limit);
        rows.push(h("button", { class: "btn more", onclick: () => { state.limit += PAGE; renderList(); } }, t("list.more", { n: more })));
      }
      if (!state.filter && summary.count > summary.shown) {
        rows.push(h("p", { class: "muted not-shown" }, t("list.notShown", { n: formatCount(summary.count - summary.shown) })));
      }
      list.replaceChildren(...rows);
    }
  }

  function itemRow(item: Item, max: number): HTMLElement {
    const box = h("input", { type: "checkbox", checked: selection.has(item.path), "aria-label": baseName(item.path) });
    const risk = item.risk === "safe" ? "safe" : "review";
    const row = h(
      "label",
      { class: `row risk-${risk} ${box.checked ? "selected" : ""}`, "data-path": item.path },
      box,
      sprite(item.isDir ? "folder" : "file", "row-icon"),
      h(
        "span",
        { class: "row-main" },
        h("span", { class: "row-name" }, baseName(item.path)),
        h("span", { class: "row-path" }, `${short(parentPath(item.path))} · ${item.label}`),
      ),
      h("span", { class: `risk-badge ${risk}`, title: t(risk === "safe" ? "risk.safeHint" : "risk.reviewHint") }, t(risk === "safe" ? "risk.safe" : "risk.review")),
      brainChip(item.brain),
      h("span", { class: "row-age" }, formatAge(item.modified)),
      h("span", { class: "row-size" }, formatBytes(item.size), meter(item.size / max)),
      rowActions(item, item.label, item.brain),
    );
    box.addEventListener("change", () => {
      toggleTarget(box.checked, toSelected(item));
      row.classList.toggle("selected", box.checked);
    });
    return row;
  }

  // ---------- explorer ----------

  async function renderExplorer() {
    let entries: TreeEntry[];
    try {
      entries = await backend.listDir(state.explorerPath);
    } catch {
      state.explorerPath = null;
      entries = await backend.listDir(null);
    }
    if (state.view !== "explorer") return; // navigated away while loading
    const total = entries.reduce((sum, e) => sum + e.size, 0);
    const blocks = entries.map((e) => ({ label: entryName(e), size: e.size, clickable: e.kind === "dir" }));
    const up = state.explorerPath !== null ? h("button", { class: "row up", onclick: goUp }, "▲ ", t("explorer.up")) : null;

    arena.replaceChildren(
      h(
        "header",
        { class: "arena-head" },
        h("div", { class: "arena-title" }, sprite("eye", "big"), h("h2", {}, t("explorer.title")), h("span", { class: "arena-total" }, formatBytes(total))),
        breadcrumbs(),
        h("p", { class: "desc" }, t("explorer.hint")),
      ),
      treemap(blocks, (i) => openDir(entries[i])),
      h("div", { class: "list" }, up, entries.map((e) => explorerRow(e, total || 1))),
    );
  }

  function entryName(e: TreeEntry): string {
    if (e.kind === "small_files") return t("explorer.small", { n: formatCount(e.files) });
    return e.name;
  }

  function explorerRow(e: TreeEntry, total: number): HTMLElement {
    const path = e.path;
    const name = entryName(e);
    const box = path ? h("input", { type: "checkbox", checked: selection.has(path), "aria-label": name }) : h("span", { class: "box-spacer" });
    const nameEl =
      e.kind === "dir"
        ? h("button", { class: "row-main link", onclick: () => openDir(e) }, h("span", { class: "row-name" }, name))
        : h("span", { class: "row-main" }, h("span", { class: "row-name" }, name));
    const thing: Thing | null = path ? { path, isDir: e.kind === "dir", size: e.size, modified: null } : null;
    const row = h(
      "div",
      { class: `row explorer-row ${path && selection.has(path) ? "selected" : ""}`, "data-path": path ?? "" },
      box,
      sprite(e.kind === "dir" ? "folder" : "file", "row-icon"),
      nameEl,
      brainChip(e.brain),
      h("span", { class: "row-age" }, `${((e.size / total) * 100).toFixed(1)}%`),
      h("span", { class: "row-size" }, formatBytes(e.size), meter(e.size / total)),
      thing ? rowActions(thing, "", e.brain) : h("span", { class: "box-spacer" }),
    );
    if (thing && box instanceof HTMLInputElement) {
      box.addEventListener("change", () => {
        toggleTarget(box.checked, { ...thing, risk: "review", category: null });
        row.classList.toggle("selected", box.checked);
      });
    }
    return row;
  }

  function openDir(e: TreeEntry) {
    if (e.kind !== "dir" || !e.path) return;
    state.explorerPath = e.path;
    sfx.blip();
    void renderExplorer();
  }

  function goUp() {
    const path = state.explorerPath;
    if (path === null) return;
    state.explorerPath = report.roots.includes(path) ? null : parentPath(path) || null;
    sfx.blip();
    void renderExplorer();
  }

  function breadcrumbs(): HTMLElement {
    const nav = h("nav", { class: "crumbs" });
    const crumb = (label: string, target: string | null) =>
      h("button", { class: "crumb", onclick: () => { state.explorerPath = target; sfx.blip(); void renderExplorer(); } }, label);
    nav.append(crumb(t("explorer.roots"), null));
    const path = state.explorerPath;
    if (path) {
      const root = report.roots.find((r) => isInside(path, r)) ?? path;
      const sep = root.includes("\\") ? "\\" : "/";
      nav.append(" / ", crumb(short(root), root));
      let acc = root;
      for (const part of path.slice(root.length).split(/[\\/]/).filter(Boolean)) {
        acc = acc.endsWith(sep) ? acc + part : acc + sep + part;
        nav.append(" / ", crumb(part, acc));
      }
    }
    return nav;
  }

  // ---------- shared bits ----------

  function toSelected(item: Item): Selected {
    return { ...thingOf(item), risk: item.risk, category: item.category };
  }

  function toggleTarget(on: boolean, entry: Selected) {
    if (on) {
      selection.add([entry]);
      sfx.select();
    } else {
      selection.remove([entry.path]);
      sfx.deselect();
    }
  }

  function iconButton(icon: SpriteName, title: string, onClick: () => void): HTMLElement {
    return h(
      "button",
      {
        class: `icon-btn icon-${icon}`,
        title,
        "aria-label": title,
        onclick: (e: Event) => {
          e.preventDefault();
          e.stopPropagation();
          sfx.blip();
          onClick();
        },
      },
      sprite(icon),
    );
  }

  function meter(ratio: number): HTMLElement {
    const fill = h("span", { class: "meter-fill" });
    fill.style.width = `${Math.max(2, Math.min(100, ratio * 100))}%`;
    return h("span", { class: "meter" }, fill);
  }

  // ---------- fire! ----------

  function renderFirebar() {
    const targets = selection.effective();
    const bytes = targets.reduce((sum, x) => sum + x.size, 0);
    const mode = (permanent: boolean, label: string) =>
      h(
        "button",
        {
          class: `mode-option ${permanent ? "nuke" : ""} ${state.permanent === permanent ? "active" : ""}`,
          role: "radio",
          "aria-checked": String(state.permanent === permanent),
          onclick: () => {
            state.permanent = permanent;
            sfx.blip();
            renderFirebar();
          },
        },
        label,
      );
    firebar.replaceChildren(
      h(
        "div",
        { class: "fire-status" },
        sprite("ship", "ship"),
        h(
          "div",
          {},
          h("div", { class: "fire-label" }, t("fire.targets")),
          h("div", { class: "fire-count" }, targets.length ? `${formatCount(targets.length)} · ${formatBytes(bytes)}` : t("fire.none")),
        ),
      ),
      h("div", { class: "mode", role: "radiogroup", title: t("fire.modeHint"), "aria-label": t("fire.modeHint") }, mode(false, t("fire.trash")), mode(true, t("fire.nuke"))),
      h("button", { class: `fire-button ${state.permanent ? "nuke" : ""}`, disabled: targets.length === 0, onclick: () => void fire() }, t("fire.button")),
    );
  }

  async function fire() {
    const targets = selection.effective();
    if (!targets.length) {
      sfx.error();
      return;
    }
    const bytes = targets.reduce((sum, x) => sum + x.size, 0);
    const review = targets.filter((x) => x.risk === "review").length;
    const inTrash = targets.filter((x) => x.category === "trash");
    const lines = [t(state.permanent ? "confirm.nuke" : "confirm.trash", { size: formatBytes(bytes) })];
    if (!state.permanent && inTrash.length) lines.push(t("confirm.inTrash"));
    if (review) lines.push(t("confirm.review", { n: review }));
    const preview = targets.slice(0, 6).map((x) => `${short(x.path)}  ${formatBytes(x.size)}`);
    if (targets.length > 6) preview.push(t("confirm.more", { n: targets.length - 6 }));

    sfx.blip();
    const sure = await confirmDialog({ title: t("confirm.title", { n: targets.length, s: targets.length === 1 ? "" : "S" }), lines, list: preview, danger: state.permanent || review > 0 });
    if (!sure) return;

    sfx.laser();
    el.classList.add("firing");
    const forever = state.permanent ? targets : inTrash;
    const toTrash = state.permanent ? [] : targets.filter((x) => x.category !== "trash");
    let outcomes: DeleteOutcome[] = [];
    try {
      if (toTrash.length) outcomes = outcomes.concat(await backend.deleteItems(toTrash, false));
      if (forever.length) outcomes = outcomes.concat(await backend.deleteItems(forever, true));
    } catch (error) {
      const done = new Set(outcomes.map((o) => o.path));
      outcomes = outcomes.concat(targets.filter((x) => !done.has(x.path)).map((x) => ({ path: x.path, ok: false, error: String(error) })));
    }
    el.classList.remove("firing");

    const sizeOf = new Map(targets.map((x) => [x.path, x.size]));
    const gone = outcomes.filter((o) => o.ok).map((o) => o.path);
    const goneSet = new Set(gone);
    const lessons: Teaching[] = targets.filter((x) => goneSet.has(x.path)).map((x) => ({ ...thingOf(x), label: "delete" }));
    const failed = outcomes.filter((o) => !o.ok);
    if (gone.length) {
      const freed = gone.reduce((sum, path) => sum + (sizeOf.get(path) ?? 0), 0);
      await explode(gone);
      forget(gone, freed);
      hud.addScore(freed);
      sfx.explode();
      setTimeout(sfx.coin, 250);
      const box = firebar.getBoundingClientRect();
      floatText(`+${formatBytes(freed)}`, box.left + box.width / 2, box.top - 30, "float-text big");
      if (state.view !== "explorer" && !report.items.some((i) => i.category === state.view)) {
        setTimeout(() => {
          sfx.victory();
          floatText(t("done.stage"), window.innerWidth / 2, window.innerHeight / 2.5, "float-text stage");
        }, 500);
      }
      if (toTrash.some((x) => gone.includes(x.path))) toast(t("done.trashTip"));
    }
    renderSidebar();
    renderArena();
    renderFirebar();
    // Every shot is a lesson: the brain learns what you consider junk.
    void teach(lessons, t("brain.learned", { n: lessons.length, xp: lessons.length * 10 })).then(renderSidebar);
    if (failed.length) {
      sfx.error();
      await messageDialog({
        title: t("done.failed", { n: failed.length, s: failed.length === 1 ? "" : "S" }),
        lines: [],
        list: failed.slice(0, 12).map((f) => `${short(f.path)}: ${errorText(f.error)}`),
      });
    }
  }

  function errorText(code?: string): string {
    if (!code) return t("err.unknown");
    const known = tk(`err.${code}`);
    return known === `err.${code}` ? `${t("err.unknown")} (${code})` : known;
  }

  /** Flash and shrink the rows that are about to disappear. */
  function explode(paths: string[]): Promise<void> {
    const doomed = new Set(paths);
    arena.querySelectorAll<HTMLElement>(".row[data-path]").forEach((row) => {
      if (doomed.has(row.dataset.path ?? "")) row.classList.add("boom");
    });
    return new Promise((resolve) => setTimeout(resolve, matchMedia("(prefers-reduced-motion: reduce)").matches ? 0 : 450));
  }

  /** Removes deleted things (and everything inside them) from the report and the selection. */
  function forget(gone: string[], freed: number) {
    const isGone = (path: string) => gone.some((g) => isInside(path, g));
    for (const item of report.items.filter((i) => isGone(i.path))) {
      const s = summaryOf(item.category);
      s.count = Math.max(0, s.count - 1);
      s.shown = Math.max(0, s.shown - 1);
      s.bytes = Math.max(0, s.bytes - item.size);
    }
    report.items = report.items.filter((i) => !isGone(i.path));
    report.bytes = Math.max(0, report.bytes - freed);

    // A "duplicate" with no twin left is not a duplicate any more.
    const members = new Map<number | undefined, Item[]>();
    report.items.filter((i) => i.category === "duplicates").forEach((i) => members.set(i.group, [...(members.get(i.group) ?? []), i]));
    const lonely = new Set([...members.values()].filter((g) => g.length < 2).flat());
    report.items = report.items.filter((i) => !lonely.has(i));
    const dupes = summaryOf("duplicates");
    const groups = [...members.values()].filter((g) => g.length >= 2);
    dupes.shown = dupes.count = groups.reduce((sum, g) => sum + g.length, 0);
    dupes.bytes = groups.reduce((sum, g) => sum + g[0].size * (g.length - 1), 0);

    selection.remove(selection.paths().filter(isGone));
  }

  // ---------- wiring ----------

  const unsubscribe = selection.onChange(renderFirebar);
  const onKey = (e: KeyboardEvent) => {
    const typing = e.target instanceof HTMLInputElement && e.target.type !== "checkbox";
    if ((e.key === "Delete" || (e.key === "Backspace" && (e.metaKey || e.ctrlKey))) && !typing) {
      e.preventDefault();
      void fire();
    }
  };
  document.addEventListener("keydown", onKey);

  renderSidebar();
  renderArena();
  renderFirebar();
  if (!state.brain) {
    backend
      .brainStatus()
      .then((status) => {
        state.brain = status;
        renderSidebar();
        if (state.view === "brain") renderArena();
      })
      .catch((error) => console.warn("the brain is asleep", error));
  }

  return {
    el,
    destroy() {
      unsubscribe();
      document.removeEventListener("keydown", onKey);
    },
  };
}
