// Obsidian-style live preview for CodeMirror: Markdown syntax is hidden on lines
// without a cursor. Decorations only – the document is always the raw file text.
import { type ChangeSpec, type Range } from "@codemirror/state";
import { Decoration, type DecorationSet, EditorView, ViewPlugin, type ViewUpdate, WidgetType } from "@codemirror/view";
import { syntaxTree } from "@codemirror/language";

/** The change that checks or unchecks the task marker `[ ]` / `[x]` at `pos`: one character. */
export function toggleTask(marker: string, pos: number): ChangeSpec {
  return { from: pos + 1, to: pos + 2, insert: marker[1] === " " ? "x" : " " };
}

class Check extends WidgetType {
  marker: string;
  pos: number;
  constructor(marker: string, pos: number) {
    super();
    this.marker = marker;
    this.pos = pos;
  }
  eq(o: Check) {
    return o.marker === this.marker && o.pos === this.pos;
  }
  toDOM(view: EditorView) {
    const i = document.createElement("input");
    i.type = "checkbox";
    i.checked = this.marker[1] !== " ";
    i.className = "cm-check";
    i.onmousedown = (e) => {
      e.preventDefault();
      view.dispatch({ changes: toggleTask(this.marker, this.pos) });
    };
    return i;
  }
  ignoreEvent() {
    return false;
  }
}

class Bullet extends WidgetType {
  eq() {
    return true;
  }
  toDOM() {
    const s = document.createElement("span");
    s.className = "cm-bullet";
    s.textContent = "•";
    return s;
  }
}

const hide = Decoration.replace({});
const bullet = Decoration.replace({ widget: new Bullet() });
const done = Decoration.mark({ class: "cm-done" });
const wiki = Decoration.mark({ class: "cm-wiki" });
const HEADING = [1, 2, 3, 4, 5, 6].map((n) => Decoration.line({ class: `cm-h${Math.min(n, 3)}` }));
const MARKS = new Set(["HeaderMark", "EmphasisMark", "StrikethroughMark", "QuoteMark"]);
const WIKI = /\[\[([^\[\]\n]+)\]\]/g;

function build(view: EditorView): DecorationSet {
  const { doc, selection } = view.state;
  const active = new Set(selection.ranges.flatMap((r) => [doc.lineAt(r.from).number, doc.lineAt(r.to).number]));
  const out: Range<Decoration>[] = [];
  // Wiki-links [[x]] first; Markdown marks inside them are left alone.
  const wikis: [number, number][] = [];
  for (const { from, to } of view.visibleRanges) {
    for (let pos = from; pos <= to; ) {
      const line = doc.lineAt(pos);
      if (!active.has(line.number)) {
        for (const m of line.text.matchAll(WIKI)) {
          const a = line.from + m.index!;
          const b = a + m[0].length;
          wikis.push([a, b]);
          out.push(hide.range(a, a + 2), wiki.range(a + 2, b - 2), hide.range(b - 2, b));
        }
      }
      pos = line.to + 1;
    }
  }
  const inWiki = (a: number) => wikis.some(([f, t]) => a >= f && a < t);

  for (const { from, to } of view.visibleRanges) {
    syntaxTree(view.state).iterate({
      from,
      to,
      enter: (n) => {
        if (n.name === "Table" || n.name === "FencedCode" || n.name === "CodeBlock") return false; // stay raw
        const heading = /^(ATX|Setext)Heading(\d)$/.exec(n.name);
        if (heading) {
          const line = doc.lineAt(n.from);
          out.push(HEADING[+heading[2] - 1].range(line.from));
          if (heading[1] === "Setext") return false; // underline stays raw
        }
        const line = doc.lineAt(n.from);
        const on = active.has(line.number);
        if (n.name === "TaskMarker") {
          const marker = doc.sliceString(n.from, n.to);
          if (!on) out.push(Decoration.replace({ widget: new Check(marker, n.from) }).range(n.from, n.to));
          if (marker[1] !== " " && n.to < line.to) out.push(done.range(n.to, line.to));
          return;
        }
        if (on || inWiki(n.from)) return;
        const parent = n.node.parent?.name;
        if (n.name === "ListMark" && /^[-*+]$/.test(doc.sliceString(n.from, n.to))) {
          // A task item hides "- " (the checkbox stands in); a plain item gets a bullet.
          if (/^\s*[-*+] \[[ xX]\]/.test(line.text)) out.push(hide.range(n.from, Math.min(n.to + 1, line.to)));
          else out.push(bullet.range(n.from, n.to));
        } else if (MARKS.has(n.name)) {
          // "## " hides the space after the marks too.
          const end = n.name === "HeaderMark" && doc.sliceString(n.to, n.to + 1) === " " ? n.to + 1 : n.to;
          if (end > n.from) out.push(hide.range(n.from, end));
        } else if (n.name === "CodeMark" && parent === "InlineCode") {
          out.push(hide.range(n.from, n.to));
        } else if (parent === "Link" && (n.name === "LinkMark" || n.name === "URL" || n.name === "LinkTitle")) {
          // "](url)" hides the link target; bare URLs and autolinks stay.
          out.push(hide.range(n.from, n.to));
        }
      },
    });
  }
  return Decoration.set(out, true);
}

export const livePreview = ViewPlugin.fromClass(
  class {
    decorations: DecorationSet;
    constructor(view: EditorView) {
      this.decorations = build(view);
    }
    update(u: ViewUpdate) {
      if (u.docChanged || u.viewportChanged || u.selectionSet || syntaxTree(u.startState) !== syntaxTree(u.state)) this.decorations = build(u.view);
    }
  },
  { decorations: (p) => p.decorations },
);
