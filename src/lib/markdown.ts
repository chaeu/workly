import DOMPurify from "dompurify";
import { marked } from "marked";

/** Markdown to sanitised HTML for read-only previews. */
export function renderMarkdown(src: string): string {
  return DOMPurify.sanitize(marked.parse(src, { async: false, gfm: true }));
}
