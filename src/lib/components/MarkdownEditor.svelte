<script lang="ts">
  import { onMount } from "svelte";
  import { beforeNavigate, goto } from "$app/navigation";
  import { ask } from "@tauri-apps/plugin-dialog";
  import { Prec } from "@codemirror/state";
  import { EditorView, keymap } from "@codemirror/view";
  import { fileText, markdownState } from "$lib/editor/setup";
  import { readMarkdown, setBody, workspace } from "$lib/stores/workspace.svelte";

  /**
   * Edits the body of one Markdown file; the frontmatter is never shown or touched.
   * ⌘S saves and keeps editing, Esc / Cancel closes (asks when there are unsaved changes).
   * `dirty` is bindable so the parent can ask before switching files.
   */
  let {
    path,
    onclose,
    template,
    dirty = $bindable(false),
  }: { path: string; onclose: () => void; template?: () => Promise<string>; dirty?: boolean } = $props();

  let host: HTMLDivElement;
  let view: EditorView | null = null;
  /** The body on disk when loaded or last saved; set_body refuses when the file has another one. */
  let expected = "";
  let empty = $state(false);
  let conflict = $state(false);
  let saving = false;

  const extras = [
    Prec.high(
      keymap.of([
        { key: "Mod-s", run: () => (save(), true) },
        { key: "Escape", run: () => (cancel(), true) },
      ]),
    ),
    EditorView.updateListener.of((u) => u.docChanged && sync()),
  ];

  function sync() {
    const text = view ? fileText(view.state) : expected;
    dirty = text !== expected;
    empty = text.trim() === "";
  }

  function load(text: string) {
    expected = text;
    conflict = false;
    const state = markdownState(text, extras);
    if (view) view.setState(state);
    else view = new EditorView({ parent: host, state });
    sync();
    view.focus();
  }

  onMount(() => {
    readMarkdown(path).then(load, (e) => (workspace.error = String(e)));
    return () => view?.destroy();
  });

  async function save() {
    if (!view || saving || conflict) return;
    const text = fileText(view.state);
    if (text === expected) return;
    saving = true;
    const ok = await setBody(path, text, expected);
    saving = false;
    if (ok) expected = text;
    else if (ok === false) conflict = true;
    sync();
  }

  async function cancel() {
    if (dirty && !(await ask("Discard your changes to this file?", { title: "Unsaved changes", kind: "warning", okLabel: "Discard" }))) return;
    dirty = false;
    onclose();
  }

  /** Conflict: drop my edit and show the file as it is now. */
  const reload = () => readMarkdown(path).then(load, (e) => (workspace.error = String(e)));

  /** Conflict: save my text over the file's current body. */
  async function overwrite() {
    expected = await readMarkdown(path);
    conflict = false;
    await save();
  }

  async function insertTemplate() {
    if (!view || !template) return;
    const text = await template();
    view.dispatch({ changes: { from: 0, to: view.state.doc.length, insert: text } });
    view.focus();
  }

  // Leaving the page with unsaved changes asks first.
  let leaving = false;
  beforeNavigate((nav) => {
    if (!dirty || leaving || !nav.to) return;
    nav.cancel();
    const to = nav.to.url;
    ask("Discard your changes to this file?", { title: "Unsaved changes", kind: "warning", okLabel: "Discard" }).then((ok) => {
      if (!ok) return;
      leaving = true;
      goto(to);
    });
  });
</script>

<div class="editor">
  <div class="bar">
    {#if conflict}
      <div class="conflict" role="alert">
        Changed outside Workly since you opened it.
        <button type="button" class="w-btn w-btn--quiet" onclick={reload}>Reload</button>
        <button type="button" class="w-btn w-btn--quiet" onclick={overwrite}>Overwrite</button>
      </div>
    {:else if empty && template}
      <button type="button" class="w-btn w-btn--quiet" onclick={insertTemplate}>Insert template</button>
    {/if}
    <span class="spacer"></span>
    <button type="button" class="w-btn w-btn--quiet" onclick={cancel}>{dirty ? "Cancel" : "Close"}</button>
    <button type="button" class="w-btn w-btn--primary" disabled={!dirty || conflict} onclick={save}>Save</button>
  </div>
  <div class="host" bind:this={host}></div>
</div>

<style>
  .editor {
    display: flex;
    flex-direction: column;
    gap: var(--w-s-2);
    min-height: 0;
  }
  .bar {
    display: flex;
    align-items: center;
    gap: var(--w-s-1);
  }
  .bar .w-btn {
    padding: 4px 10px;
    font-size: var(--w-fs-caption);
  }
  .bar .w-btn:disabled {
    opacity: 0.5;
    cursor: default;
  }
  .spacer {
    flex: 1;
  }
  .conflict {
    display: flex;
    align-items: center;
    gap: var(--w-s-1);
    color: var(--w-warn);
    font-size: var(--w-fs-small);
  }

  /* CodeMirror: same type scale as the rendered preview (markdown.css). */
  .host :global(.cm-editor) {
    max-width: 72ch;
    font-size: var(--w-fs-body);
    color: var(--w-ink);
    background: transparent;
  }
  .host :global(.cm-editor.cm-focused) {
    outline: none;
  }
  .host :global(.cm-scroller) {
    font-family: var(--w-font-sans);
    line-height: 1.55;
  }
  .host :global(.cm-content) {
    caret-color: var(--w-accent);
    min-height: 240px;
  }
  .host :global(.cm-cursor) {
    border-left-color: var(--w-accent);
  }
  .host :global(.cm-selectionBackground),
  .host :global(.cm-focused .cm-selectionBackground) {
    /* CodeMirror's base theme is more specific. */
    background: var(--w-accent-soft) !important;
  }
  .host :global(.cm-selectionMatch) {
    background: var(--w-accent-soft);
  }
  .host :global(.cm-h1),
  .host :global(.cm-h2),
  .host :global(.cm-h3) {
    font-family: var(--w-font-label);
    font-weight: 600;
  }
  .host :global(.cm-h1) {
    font-size: var(--w-fs-heading);
  }
  .host :global(.cm-h2) {
    font-size: var(--w-fs-title);
  }
  .host :global(.tok-mark) {
    color: var(--w-muted);
  }
  .host :global(.tok-strong) {
    font-weight: 600;
  }
  .host :global(.tok-em) {
    font-style: italic;
  }
  .host :global(.tok-strike),
  .host :global(.cm-done) {
    text-decoration: line-through;
    color: var(--w-muted);
  }
  .host :global(.tok-link),
  .host :global(.cm-wiki) {
    color: var(--w-accent);
  }
  .host :global(.tok-code) {
    font-family: var(--w-font-mono);
    font-size: var(--w-fs-small);
    background: var(--w-sunk);
    border-radius: var(--w-r-sm);
  }
  .host :global(.tok-quote) {
    color: var(--w-muted);
  }
  .host :global(.cm-bullet) {
    color: var(--w-muted);
  }
  .host :global(.cm-check) {
    margin: 0 4px 0 0;
    vertical-align: -2px;
    accent-color: var(--w-accent);
    cursor: pointer;
  }
  /* Search panel (⌘F). */
  .host :global(.cm-panels) {
    background: var(--w-sunk);
    color: var(--w-ink);
    border-radius: var(--w-r-md);
  }
  .host :global(.cm-panels-bottom) {
    border-top: 1px solid var(--w-line);
  }
  .host :global(.cm-panel.cm-search) {
    font-family: var(--w-font-sans);
    font-size: var(--w-fs-caption);
  }
  .host :global(.cm-panel.cm-search input),
  .host :global(.cm-panel.cm-search button) {
    font: inherit;
    color: var(--w-ink);
    background: var(--w-surface);
    border: 1px solid var(--w-line);
    border-radius: var(--w-r-sm);
  }
  .host :global(.cm-searchMatch) {
    background: var(--w-accent-soft);
    outline: 1px solid var(--w-accent);
  }
</style>
