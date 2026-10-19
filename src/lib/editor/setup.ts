// Editor state for Markdown files: one setup for MarkdownEditor.svelte and the round-trip test.
import { EditorState, type Extension } from "@codemirror/state";
import { EditorView, drawSelection, highlightSpecialChars, keymap } from "@codemirror/view";
import { defaultKeymap, history, historyKeymap, indentWithTab } from "@codemirror/commands";
import { HighlightStyle, LanguageSupport, syntaxHighlighting } from "@codemirror/language";
import { markdownKeymap, markdownLanguage } from "@codemirror/lang-markdown";
import { highlightSelectionMatches, searchKeymap } from "@codemirror/search";
import { tags as t } from "@lezer/highlight";
import { livePreview } from "./livePreview.ts";

// Colours live in MarkdownEditor.svelte, from tokens.
const style = HighlightStyle.define([
  { tag: [t.processingInstruction, t.meta, t.contentSeparator], class: "tok-mark" },
  { tag: t.strong, class: "tok-strong" },
  { tag: t.emphasis, class: "tok-em" },
  { tag: [t.link, t.url], class: "tok-link" },
  { tag: t.monospace, class: "tok-code" },
  { tag: t.quote, class: "tok-quote" },
  { tag: t.strikethrough, class: "tok-strike" },
]);

/** State for `text`, joined back with its own line break so saving without edits gives the same bytes. */
export function markdownState(text: string, extra: Extension[] = []): EditorState {
  return EditorState.create({
    doc: text,
    extensions: [
      EditorState.lineSeparator.of(text.includes("\r\n") ? "\r\n" : "\n"),
      history(),
      drawSelection(),
      highlightSpecialChars(),
      highlightSelectionMatches(),
      keymap.of([indentWithTab, ...markdownKeymap, ...defaultKeymap, ...historyKeymap, ...searchKeymap]),
      // GFM without markdown(): that would also bundle the HTML, CSS and JS parsers for embedded code.
      new LanguageSupport(markdownLanguage),
      syntaxHighlighting(style),
      EditorView.lineWrapping,
      livePreview,
      ...extra,
    ],
  });
}

/** The file text of a state, with the file's own line breaks (doc.toString() would always use "\n"). */
export const fileText = (state: EditorState) => state.sliceDoc();
