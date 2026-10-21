import { t } from "./i18n";

// macOS Finder counts 1 GB = 1000³ bytes, Windows Explorer counts 1024³. Match the OS so numbers agree.
let base = 1024;

export function useDecimalUnits(decimal: boolean): void {
  base = decimal ? 1000 : 1024;
}

const UNITS = ["B", "KB", "MB", "GB", "TB", "PB"];

export function formatBytes(bytes: number): string {
  if (bytes < base) return `${bytes} B`;
  let value = bytes;
  let unit = 0;
  while (value >= base && unit < UNITS.length - 1) {
    value /= base;
    unit++;
  }
  const digits = value >= 100 ? 0 : value >= 10 ? 1 : 2;
  return `${value.toFixed(digits)} ${UNITS[unit]}`;
}

export function formatCount(n: number): string {
  return n.toLocaleString("en-US");
}

export function formatDuration(ms: number): string {
  const s = Math.round(ms / 1000);
  if (s < 60) return `${s}s`;
  return `${Math.floor(s / 60)}m ${String(s % 60).padStart(2, "0")}s`;
}

/** "12d", "5mo", "3y" (localized). */
export function formatAge(modified: number | null): string {
  if (modified === null) return "";
  const days = Math.max(0, Math.floor((Date.now() / 1000 - modified) / 86400));
  if (days < 1) return t("age.today");
  if (days < 60) return t("age.days", { n: days });
  if (days < 730) return t("age.months", { n: Math.floor(days / 30) });
  return t("age.years", { n: Math.floor(days / 365) });
}

/** Score-style zero padded number: 0042.7 */
export function scoreDigits(bytes: number): { value: string; unit: string } {
  const [value, unit] = formatBytes(bytes).split(" ");
  const [int, frac] = value.split(".");
  return { value: int.padStart(4, "0") + (frac ? `.${frac}` : ""), unit };
}

const SEPARATOR = /[\\/]/;

export function baseName(path: string): string {
  const parts = path.split(SEPARATOR).filter(Boolean);
  return parts.length ? parts[parts.length - 1] : path;
}

/** "/a/b/c" -> "/a/b", "/a" -> "/", "C:\\a" -> "C:\\". */
export function parentPath(path: string): string {
  const trimmed = path.replace(/[\\/]+$/, "");
  const i = Math.max(trimmed.lastIndexOf("/"), trimmed.lastIndexOf("\\"));
  if (i < 0) return "";
  if (i === 0) return trimmed[0];
  const parent = trimmed.slice(0, i);
  return /^[A-Za-z]:$/.test(parent) ? parent + "\\" : parent;
}

/** Replaces the home folder with "~" so paths fit on screen. */
export function shortenHome(path: string, home: string | null): string {
  if (home && (path === home || path.startsWith(home + "/") || path.startsWith(home + "\\"))) {
    return "~" + path.slice(home.length);
  }
  return path;
}

/** Is `path` inside `folder` (or equal to it)? Works with both / and \ separators. */
export function isInside(path: string, folder: string): boolean {
  if (path === folder) return true;
  const withSep = /[\\/]$/.test(folder) ? folder : folder + (folder.includes("\\") ? "\\" : "/");
  return path.startsWith(withSep);
}
