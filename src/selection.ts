import { parentPath } from "./format";
import type { Category, Risk, Target } from "./types";

export interface Selected extends Target {
  risk: Risk;
  /** `null` when picked in the Explorer. */
  category: Category | null;
  isDir: boolean;
  modified: number | null;
}

/** The set of targets the player has picked, keyed by path. Shared by every level and the Explorer. */
export class Selection {
  private items = new Map<string, Selected>();
  private listeners = new Set<() => void>();

  has(path: string): boolean {
    return this.items.has(path);
  }

  get count(): number {
    return this.items.size;
  }

  paths(): string[] {
    return [...this.items.keys()];
  }

  add(entries: Selected[]): void {
    for (const entry of entries) this.items.set(entry.path, entry);
    this.emit();
  }

  remove(paths: Iterable<string>): void {
    for (const path of paths) this.items.delete(path);
    this.emit();
  }

  clear(): void {
    this.items.clear();
    this.emit();
  }

  onChange(listener: () => void): () => void {
    this.listeners.add(listener);
    return () => this.listeners.delete(listener);
  }

  /** What will actually be deleted: anything inside an already selected folder goes with it. */
  effective(): Selected[] {
    return [...this.items.values()].filter((entry) => !this.hasSelectedAncestor(entry.path));
  }

  totalBytes(): number {
    return this.effective().reduce((sum, entry) => sum + entry.size, 0);
  }

  private hasSelectedAncestor(path: string): boolean {
    for (let p = parentPath(path); p && p !== path; ) {
      if (this.items.has(p)) return true;
      const next = parentPath(p);
      if (next === p) break;
      p = next;
    }
    return false;
  }

  private emit(): void {
    this.listeners.forEach((listener) => listener());
  }
}
