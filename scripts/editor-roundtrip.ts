// Run with `pnpm test:editor` (plain node, no test framework; outside src/ so svelte-check skips it).
// Loading a file into the editor and saving without edits must give the identical string.
import assert from "node:assert/strict";
import { readFileSync, readdirSync, statSync } from "node:fs";
import { join } from "node:path";
import { fileText, markdownState } from "../src/lib/editor/setup.ts";
import { linkAt, toggleTask } from "../src/lib/editor/livePreview.ts";
import { linkTarget } from "../src/lib/markdown.ts";

function mdFiles(dir: string): string[] {
  return readdirSync(dir).flatMap((n) => {
    const p = join(dir, n);
    return statSync(p).isDirectory() ? mdFiles(p) : n.endsWith(".md") ? [p] : [];
  });
}

const samples = [
  ...mdFiles("fixtures/workspace").map((p) => [p, readFileSync(p, "utf8")]),
  ["crlf", "# Title\r\n\r\n- [ ] a\r\n- [x] b\r\n"],
  ["no trailing newline", "text **bold** [[wiki]]"],
  ["tabs and spaces", "\t- item\n    code\n\n| a | b |\n|---|---|\n| 1 | 2 |\n\nSetext\n===\n"],
  ["empty", ""],
];
for (const [name, text] of samples) {
  assert.equal(fileText(markdownState(text)), text, name);
}

// The checkbox changes exactly one character.
const text = "- [ ] open\n- [x] done\n";
let state = markdownState(text);
state = state.update({ changes: toggleTask("[ ]", 2) }).state;
assert.equal(fileText(state), "- [x] open\n- [x] done\n");
state = state.update({ changes: toggleTask("[x]", 13) }).state;
assert.equal(fileText(state), "- [x] open\n- [ ] done\n");

// An edit in a CRLF file keeps CRLF.
const crlf = markdownState("a\r\nb\r\n");
assert.equal(fileText(crlf.update({ changes: { from: 1, insert: "\r\nx" } }).state), "a\r\nx\r\nb\r\n");

// Links: Markdown link target, bare URL, wiki name; plain text is no link.
const doc = "See [brief](docs/a.md) and https://x.org and [[kickoff|Kick]].\nplain";
const links = markdownState(doc);
const at = (needle: string) => linkAt(links, doc.indexOf(needle) + 1);
assert.deepEqual(at("brief"), { href: "docs/a.md", wiki: false });
assert.deepEqual(at("x.org"), { href: "https://x.org", wiki: false });
assert.deepEqual(at("kickoff"), { href: "kickoff|Kick", wiki: true });
assert.equal(at("plain"), null);
const files = ["p/_project.md", "p/docs/a.md", "p/notes/kickoff.md"];
assert.equal(linkTarget("p/notes/x.md", "../docs/a.md", false, files), "p/docs/a.md");
assert.equal(linkTarget("p/notes/x.md", "kickoff|Kick", true, files), "p/notes/kickoff.md");
assert.equal(linkTarget("p/notes/x.md", "notes/kickoff#Top", true, files), "p/notes/kickoff.md");
assert.equal(linkTarget("p/notes/x.md", "missing", true, files), null);

console.log(`editor round trip ok (${samples.length} samples)`);
