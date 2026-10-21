// Pixel-art sprites drawn as crisp SVG. Two frames each, swapped by CSS for the classic waddle.

import type { Category } from "./types";

const SPRITES = {
  crab: [
    ["..X.....X..", "...X...X...", "..XXXXXXX..", ".XX.XXX.XX.", "XXXXXXXXXXX", "X.XXXXXXX.X", "X.X.....X.X", "...XX.XX..."],
    ["..X.....X..", "X..X...X..X", "X.XXXXXXX.X", "XXX.XXX.XXX", "XXXXXXXXXXX", ".XXXXXXXXX.", "..X.....X..", ".X.......X."],
  ],
  squid: [
    ["...XX...", "..XXXX..", ".XXXXXX.", "XX.XX.XX", "XXXXXXXX", "..X..X..", ".X.XX.X.", "X.X..X.X"],
    ["...XX...", "..XXXX..", ".XXXXXX.", "XX.XX.XX", "XXXXXXXX", ".X.XX.X.", "X......X", ".X....X."],
  ],
  octopus: [
    ["....XXXX....", ".XXXXXXXXXX.", "XXXXXXXXXXXX", "XXX..XX..XXX", "XXXXXXXXXXXX", "...XX..XX...", "..XX.XX.XX..", "XX........XX"],
    ["....XXXX....", ".XXXXXXXXXX.", "XXXXXXXXXXXX", "XXX..XX..XXX", "XXXXXXXXXXXX", "..XXX..XXX..", ".XX..XX..XX.", "..XX....XX.."],
  ],
  ufo: [
    [".....XXXXXX.....", "...XXXXXXXXXX...", "..XXXXXXXXXXXX..", ".XX.XX.XX.XX.XX.", "XXXXXXXXXXXXXXXX", "..XXX..XX..XXX..", "...X........X..."],
    [".....XXXXXX.....", "...XXXXXXXXXX...", "..XXXXXXXXXXXX..", ".X.XX.XX.XX.XX.X", "XXXXXXXXXXXXXXXX", "..XXX..XX..XXX..", "..X..........X.."],
  ],
  ship: [[".....X.....", "....XXX....", "....XXX....", ".XXXXXXXXX.", "XXXXXXXXXXX", "XXXXXXXXXXX", "XXXXXXXXXXX"]],
  folder: [["XXX.....", "XXXXXXXX", "X......X", "X......X", "X......X", "XXXXXXXX"]],
  file: [["XXXX..", "X..XX.", "X...XX", "X....X", "X....X", "X....X", "XXXXXX"]],
  eye: [["..XXXX..", ".X....X.", "X..XX..X", "X..XX..X", ".X....X.", "..XXXX.."]],
  brain: [
    ["..XXX.XXX..", ".X..XX..XX.", "X.XX..XX..X", "X...XX..X.X", ".XX...XX.X.", "..XXXXXXX..", "...X...X...", "..X.....X.."],
    ["..XXX.XXX..", ".X..XX..XX.", "X.XX..XX..X", "X...XX..X.X", ".XX...XX.X.", "..XXXXXXX..", "..X.....X..", "...X...X..."],
  ],
  heart: [[".XX.XX.", "XXXXXXX", "XXXXXXX", ".XXXXX.", "..XXX..", "...X..."]],
  ask: [[".XXX.", "X...X", "....X", "..XX.", "..X..", ".....", "..X.."]],
} as const;

export type SpriteName = keyof typeof SPRITES;

const SVG_NS = "http://www.w3.org/2000/svg";

function pathFor(rows: readonly string[]): string {
  let d = "";
  rows.forEach((row, y) => {
    for (let x = 0; x < row.length; x++) if (row[x] === "X") d += `M${x} ${y}h1v1h-1z`;
  });
  return d;
}

export function sprite(name: SpriteName, className = ""): SVGSVGElement {
  const frames = SPRITES[name];
  const svg = document.createElementNS(SVG_NS, "svg");
  svg.setAttribute("viewBox", `0 0 ${frames[0][0].length} ${frames[0].length}`);
  // Single-frame sprites would just blink if "animated".
  const classes = frames.length > 1 ? className : className.replace(/\banimate\b/, "");
  svg.setAttribute("class", `sprite sprite-${name} ${classes}`.trim());
  svg.setAttribute("shape-rendering", "crispEdges");
  svg.setAttribute("aria-hidden", "true");
  frames.forEach((frame, i) => {
    const path = document.createElementNS(SVG_NS, "path");
    path.setAttribute("d", pathFor(frame));
    path.setAttribute("class", `frame frame-${i}`);
    path.setAttribute("fill", "currentColor");
    svg.appendChild(path);
  });
  return svg;
}

/** Every level has its own invader. */
export const CATEGORY_SPRITE: Record<Category, SpriteName> = {
  caches: "squid",
  dev_junk: "crab",
  logs: "octopus",
  trash: "ufo",
  installers: "crab",
  old_downloads: "squid",
  backups: "octopus",
  duplicates: "crab",
  large_files: "ufo",
};
