import DOMPurify from "dompurify";
import { marked } from "marked";

/** Markdown to sanitised HTML for read-only previews. */
export function renderMarkdown(src: string): string {
  return DOMPurify.sanitize(marked.parse(src, { async: false, gfm: true }));
}

/** Relative link in a previewed file to a workspace path: `a/b/c.md` + `../d.md` -> `a/d.md`. */
export function resolveLink(from: string, href: string) {
  const parts = from.split("/").slice(0, -1);
  for (const seg of decodeURIComponent(href.split("#")[0]).split("/")) {
    if (seg === "..") parts.pop();
    else if (seg && seg !== ".") parts.push(seg);
  }
  return parts.join("/");
}
