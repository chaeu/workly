/** Grow a textarea with its content (no field-sizing in every WebKit yet). */
export function grow(el: HTMLTextAreaElement) {
  const fit = () => {
    el.style.height = "auto";
    el.style.height = `${el.scrollHeight}px`;
  };
  fit();
  el.addEventListener("input", fit);
  return { destroy: () => el.removeEventListener("input", fit) };
}
