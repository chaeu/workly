/**
 * `use:outside={fn}` calls fn on a pointer down outside the element (side panel: a click elsewhere closes it).
 * Clicks inside other dialogs (editors opened from the panel, quick add) do not count. A null fn does nothing.
 * Opening another card still works: the panel closes on pointer down, the card opens on click/pointer up.
 */
type Fn = (() => void) | null | undefined;

export function outside(el: HTMLElement, fn: Fn) {
  const down = (e: PointerEvent) => {
    const t = e.target as Element | null;
    if (!fn || !t?.isConnected || el.contains(t) || t.closest(".w-modal, .w-scrim")) return;
    fn();
  };
  window.addEventListener("pointerdown", down, true);
  return {
    update: (next: Fn) => (fn = next),
    destroy: () => window.removeEventListener("pointerdown", down, true),
  };
}
