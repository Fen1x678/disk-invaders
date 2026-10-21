import type { Lang } from "./i18n";

export const THEMES = ["synthwave", "phosphor", "amber", "c64"] as const;
export type Theme = (typeof THEMES)[number];

export interface Prefs {
  theme: Theme;
  sound: boolean;
  crt: boolean;
  lang: Lang | null;
  /** Best amount of bytes freed in a single session. */
  hiScore: number;
  findDuplicates: boolean;
}

const KEY = "disk-invaders:prefs";

const DEFAULTS: Prefs = {
  theme: "synthwave",
  sound: true,
  crt: true,
  lang: null,
  hiScore: 0,
  findDuplicates: true,
};

export function loadPrefs(): Prefs {
  try {
    const saved = JSON.parse(localStorage.getItem(KEY) ?? "{}") as Partial<Prefs>;
    const prefs = { ...DEFAULTS, ...saved };
    if (!THEMES.includes(prefs.theme)) prefs.theme = DEFAULTS.theme;
    return prefs;
  } catch {
    return { ...DEFAULTS };
  }
}

export function savePrefs(prefs: Prefs): void {
  try {
    localStorage.setItem(KEY, JSON.stringify(prefs));
  } catch {
    // Private mode or storage disabled: settings just won't stick.
  }
}

export function applyPrefs(prefs: Prefs): void {
  document.documentElement.dataset.theme = prefs.theme;
  document.documentElement.dataset.crt = prefs.crt ? "on" : "off";
}
