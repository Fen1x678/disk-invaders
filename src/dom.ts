// A tiny element builder. File names can contain anything (even "<script>"),
// so text always goes in through textContent, never innerHTML.

type Child = Node | string | number | null | undefined | false;
type Attrs = Record<string, string | number | boolean | EventListener | null | undefined>;

export function h<K extends keyof HTMLElementTagNameMap>(
  tag: K,
  attrs: Attrs = {},
  ...children: (Child | Child[])[]
): HTMLElementTagNameMap[K] {
  const el = document.createElement(tag);
  for (const [key, value] of Object.entries(attrs)) {
    if (value === null || value === undefined || value === false) continue;
    if (key.startsWith("on") && typeof value === "function") {
      el.addEventListener(key.slice(2).toLowerCase(), value);
    } else if (key === "class") {
      el.className = String(value);
    } else if (value === true) {
      el.setAttribute(key, "");
    } else {
      el.setAttribute(key, String(value));
    }
  }
  append(el, children);
  return el;
}

export function append(parent: Node, children: (Child | Child[])[]): void {
  for (const child of children.flat()) {
    if (child === null || child === undefined || child === false) continue;
    parent.appendChild(child instanceof Node ? child : document.createTextNode(String(child)));
  }
}

/** Short-lived floating text, e.g. "+3.2 GB". */
export function floatText(text: string, x: number, y: number, className = "float-text"): void {
  const el = h("div", { class: className }, text);
  el.style.left = `${x}px`;
  el.style.top = `${y}px`;
  document.body.appendChild(el);
  el.addEventListener("animationend", () => el.remove());
}

/** A full-window view. `destroy` removes global listeners when another screen takes over. */
export interface Screen {
  el: HTMLElement;
  destroy?: () => void;
}

/** Synthwave sky: sun + moving grid (purely decorative). */
export function backdrop(): HTMLElement {
  return h("div", { class: "backdrop", "aria-hidden": "true" }, h("div", { class: "sun" }), h("div", { class: "horizon" }, h("div", { class: "grid" })));
}
