// Squarified treemap: every block's area is proportional to its size.
// Bruls, Huizing & van Wijk, "Squarified Treemaps" (2000).

import { h } from "../dom";
import { formatBytes } from "../format";

export interface Rect {
  x: number;
  y: number;
  w: number;
  h: number;
}

/** Lays out `values` (sorted largest first, all > 0) inside `bounds`. */
export function squarify(values: number[], bounds: Rect): Rect[] {
  const total = values.reduce((a, b) => a + b, 0);
  if (total <= 0) return [];
  const scale = (bounds.w * bounds.h) / total;
  const areas = values.map((v) => v * scale);
  const out: Rect[] = [];
  let free = { ...bounds };
  let i = 0;
  while (i < areas.length) {
    const side = Math.min(free.w, free.h);
    const row = [areas[i]];
    let j = i + 1;
    while (j < areas.length && worst([...row, areas[j]], side) <= worst(row, side)) row.push(areas[j++]);

    const rowArea = row.reduce((a, b) => a + b, 0);
    if (free.w >= free.h) {
      const width = rowArea / free.h;
      let y = free.y;
      for (const area of row) {
        out.push({ x: free.x, y, w: width, h: area / width });
        y += area / width;
      }
      free = { x: free.x + width, y: free.y, w: free.w - width, h: free.h };
    } else {
      const height = rowArea / free.w;
      let x = free.x;
      for (const area of row) {
        out.push({ x, y: free.y, w: area / height, h: height });
        x += area / height;
      }
      free = { x: free.x, y: free.y + height, w: free.w, h: free.h - height };
    }
    i = j;
  }
  return out;
}

/** Worst aspect ratio in a row: lower is squarer. */
function worst(row: number[], side: number): number {
  const sum = row.reduce((a, b) => a + b, 0);
  const max = Math.max(...row);
  const min = Math.min(...row);
  return Math.max((side * side * max) / (sum * sum), (sum * sum) / (side * side * min));
}

export interface Block {
  label: string;
  size: number;
  clickable: boolean;
}

const WIDTH = 1000;
const HEIGHT = 300;
const COLORS = ["var(--accent)", "var(--accent-2)", "var(--review)", "var(--safe)", "var(--danger)"];

export function treemap(blocks: Block[], onPick: (index: number) => void): HTMLElement {
  const shown = blocks.map((b, index) => ({ ...b, index })).filter((b) => b.size > 0).slice(0, 48);
  const rects = squarify(shown.map((b) => b.size), { x: 0, y: 0, w: WIDTH, h: HEIGHT });
  const el = h("div", { class: "treemap" });
  rects.forEach((r, i) => {
    const block = shown[i];
    const tile = h(
      block.clickable ? "button" : "div",
      { class: `tile ${block.clickable ? "clickable" : ""}`, title: `${block.label} · ${formatBytes(block.size)}` },
      r.w > 70 && r.h > 34 ? h("span", { class: "tile-label" }, block.label) : null,
      r.w > 70 && r.h > 60 ? h("span", { class: "tile-size" }, formatBytes(block.size)) : null,
    );
    tile.style.left = `${(r.x / WIDTH) * 100}%`;
    tile.style.top = `${(r.y / HEIGHT) * 100}%`;
    tile.style.width = `${(r.w / WIDTH) * 100}%`;
    tile.style.height = `${(r.h / HEIGHT) * 100}%`;
    tile.style.setProperty("--tile", COLORS[i % COLORS.length]);
    if (block.clickable) tile.addEventListener("click", () => onPick(block.index));
    el.appendChild(tile);
  });
  return el;
}
