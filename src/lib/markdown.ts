import type { FileNode } from "$lib/stores/workspace.svelte";
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

/**
 * Project file a link in `from` points to, or null: a relative Markdown link, or a wiki-link
 * `[[name]]` / `[[folder/name|alias]]` matched by file name (".md" optional) among `files`.
 */
export function linkTarget(from: string, href: string, wiki: boolean, files: string[]): string | null {
  if (!wiki) {
    if (!href || href.includes(":") || href.startsWith("#")) return null;
    const target = resolveLink(from, href);
    return files.includes(target) ? target : null;
  }
  const name = href.split(/[|#]/)[0].trim();
  const want = name.endsWith(".md") ? name : `${name}.md`;
  return files.find((f) => f === want || f.endsWith(`/${want}`)) ?? null;
}

/** Paths of all files in a project tree. */
export const flatFiles = (nodes: FileNode[]): string[] => nodes.flatMap((n) => (n.children ? flatFiles(n.children) : [n.path]));

/** Plain http(s) or mailto link, for the system browser or mail app. */
export const isWebLink = (href: string) => /^(https?|mailto):/i.test(href);
