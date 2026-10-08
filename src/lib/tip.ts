/**
 * Quick tooltip: `use:tip={text}`. One shared `.w-tip` element (position: fixed, so scroll containers never clip it),
 * shown ~300 ms after hover or keyboard focus. Lines are split on "\n"; with several, the first is the heading.
 * A function is evaluated on show, e.g. `clipped(text)` shows the text only when the element cuts it off.
 * The tip only describes: the element keeps its own accessible name.
 */
type Tip = string | null | undefined | false | ((el: HTMLElement) => string | null | undefined | false);

const DELAY = 300;
const tips = new WeakMap<Element, Tip>();
let box: HTMLDivElement | null = null;
let owner: HTMLElement | null = null;
let timer = 0;
let hiddenAt = 0;

/** Innermost element with a tip, so a pill inside a card gets its own. */
function ownerOf(n: EventTarget | null) {
  let el = n instanceof Element ? n : null;
  while (el && !tips.has(el)) el = el.parentElement;
  return el as HTMLElement | null;
}

function hide() {
  clearTimeout(timer);
  if (box?.classList.contains("is-on")) hiddenAt = Date.now();
  box?.classList.remove("is-on");
  owner?.removeAttribute("aria-describedby");
  owner = null;
}

function schedule(el: HTMLElement | null) {
  if (el === owner) return;
  hide();
  if (!el) return;
  owner = el;
  // Moving straight from one tip to the next shows it at once, like native tooltips.
  timer = window.setTimeout(show, Date.now() - hiddenAt < DELAY ? 0 : DELAY);
}

function show() {
  if (!owner?.isConnected) return hide();
  const t = tips.get(owner);
  const text = (typeof t === "function" ? t(owner) : t) || "";
  const lines = text.split("\n").filter((l) => l.trim());
  if (!lines.length) return box?.classList.remove("is-on");
  if (!box) {
    box = document.createElement("div");
    box.id = "w-tip";
    box.className = "w-tip";
    box.setAttribute("role", "tooltip");
    document.body.append(box);
  }
  // textContent only: titles and hints come from workspace files.
  box.replaceChildren(
    ...lines.map((l, i) => {
      const d = document.createElement(i === 0 && lines.length > 1 ? "strong" : "div");
      d.textContent = l;
      return d;
    }),
  );
  // Below the element, above when there is no room; kept 8 px inside the window.
  const r = owner.getBoundingClientRect();
  const { offsetWidth: w, offsetHeight: h } = box;
  const below = r.bottom + 6 + h <= innerHeight - 8 || r.top - 6 - h < 8;
  box.style.left = `${Math.max(8, Math.min(r.left + r.width / 2 - w / 2, innerWidth - w - 8))}px`;
  box.style.top = `${below ? r.bottom + 6 : r.top - 6 - h}px`;
  box.classList.add("is-on");
  owner.setAttribute("aria-describedby", box.id);
}

let listening = false;
function listen() {
  listening = true;
  // No tips while a button is held: a drag passes over other cards.
  document.addEventListener("pointerover", (e) => (e.buttons ? hide() : schedule(ownerOf(e.target))));
  document.addEventListener("pointerout", (e) => !e.relatedTarget && hide());
  document.addEventListener("focusin", (e) => (e.target as Element).matches?.(":focus-visible") && schedule(ownerOf(e.target)));
  document.addEventListener("focusout", hide);
  document.addEventListener("pointerdown", hide, true);
  document.addEventListener("scroll", hide, true);
  document.addEventListener("keydown", (e) => e.key === "Escape" && hide());
  window.addEventListener("blur", hide);
}

export function tip(el: HTMLElement, text: Tip) {
  if (!listening) listen();
  tips.set(el, text);
  return {
    update(t: Tip) {
      tips.set(el, t);
      if (owner === el && box?.classList.contains("is-on")) show();
    },
    destroy() {
      tips.delete(el);
      if (owner === el) hide();
    },
  };
}

/** The text, but only while the element cuts it off (ellipsis or line clamp). */
export const clipped = (text: string | null | undefined) => (el: HTMLElement) =>
  (el.scrollWidth > el.clientWidth || el.scrollHeight > el.clientHeight) && text;
