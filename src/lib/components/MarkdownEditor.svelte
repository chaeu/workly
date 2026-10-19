<script lang="ts">
  import { onMount, untrack } from "svelte";
  import { beforeNavigate, goto } from "$app/navigation";
  import { ask } from "@tauri-apps/plugin-dialog";
  import { Prec } from "@codemirror/state";
  import { EditorView, keymap } from "@codemirror/view";
  import { fileText, markdownState } from "$lib/editor/setup";
  import { readMarkdown, setBody, workspace } from "$lib/stores/workspace.svelte";

  /**
   * Edits the body of one Markdown file, like Obsidian: always editable, saved on its own
   * (a short pause after typing, leaving the editor, switching away, ⌘S). The frontmatter is
   * never shown or touched. A file changed outside shows up live while there is nothing
   * unsaved; otherwise the save stops with Reload / Overwrite. `unsaved` (bindable) is true
   * only then, so the parent can ask before switching files.
   */
  let {
    path,
    template,
    autofocus = false,
    unsaved = $bindable(false),
  }: { path: string; template?: () => Promise<string>; autofocus?: boolean; unsaved?: boolean } = $props();

  /** Idle time after the last keystroke before saving. */
  const SAVE_AFTER_MS = 800;

  let host: HTMLDivElement;
  let view: EditorView | null = null;
  /** The body on disk when loaded or last saved; set_body refuses when the file has another one. */
  let expected = "";
  let empty = $state(false);
  let conflict = $state(false);
  let saving = false;
  let again = false;
  let timer: ReturnType<typeof setTimeout> | undefined;

  const extras = [
    Prec.high(keymap.of([{ key: "Mod-s", run: () => (save(), true) }])),
    EditorView.updateListener.of((u) => {
      if (u.docChanged) {
        sync();
        clearTimeout(timer);
        timer = setTimeout(save, SAVE_AFTER_MS);
      }
      if (u.focusChanged && !u.view.hasFocus) save();
    }),
  ];

  const text = () => (view ? fileText(view.state) : expected);

  function sync() {
    empty = text().trim() === "";
    unsaved = conflict && text() !== expected;
  }

  function load(body: string) {
    expected = body;
    conflict = false;
    const state = markdownState(body, extras);
    if (view) view.setState(state);
    else view = new EditorView({ parent: host, state });
    sync();
    if (autofocus) view.focus();
  }

  onMount(() => {
    readMarkdown(path).then(load, (e) => (workspace.error = String(e)));
    return () => {
      save(); // reads the text before the view goes away
      view?.destroy();
    };
  });

  async function save() {
    clearTimeout(timer);
    if (!view || conflict) return;
    if (saving) {
      again = true;
      return;
    }
    const body = text();
    if (body === expected) return;
    saving = true;
    const ok = await setBody(path, body, expected);
    saving = false;
    if (ok) expected = body;
    else if (ok === false) conflict = true;
    sync();
    if (again) {
      again = false;
      save();
    }
  }

  // Changed outside (Obsidian, git): show it, as long as nothing here is unsaved.
  $effect(() => {
    void workspace.reloads;
    untrack(refresh);
  });

  async function refresh() {
    if (!view || saving || conflict) return;
    const disk = await readMarkdown(path).catch(() => null);
    if (disk === null || !view || saving || disk === expected || text() !== expected) return;
    // Replace only the changed middle, so the cursor stays where it was.
    const old = expected;
    let a = 0;
    while (a < old.length && a < disk.length && old[a] === disk[a]) a++;
    let b = 0;
    while (b < old.length - a && b < disk.length - a && old[old.length - 1 - b] === disk[disk.length - 1 - b]) b++;
    expected = disk;
    view.dispatch({ changes: { from: a, to: old.length - b, insert: disk.slice(a, disk.length - b) } });
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
    const body = await template();
    view.dispatch({ changes: { from: 0, to: view.state.doc.length, insert: body } });
    view.focus();
  }

  // Leaving the page while a conflict holds unsaved text asks first.
  let leaving = false;
  beforeNavigate((nav) => {
    if (!unsaved || leaving || !nav.to) return;
    nav.cancel();
    const to = nav.to.url;
    ask("Your changes to this file are not saved. Discard them?", { title: "Unsaved changes", kind: "warning", okLabel: "Discard" }).then((ok) => {
      if (!ok) return;
      leaving = true;
      goto(to);
    });
  });
</script>

<div class="editor">
  {#if conflict}
    <div class="bar conflict" role="alert">
      Changed outside Workly while you were typing – not saved.
      <button type="button" class="w-btn w-btn--quiet" onclick={reload}>Reload</button>
      <button type="button" class="w-btn w-btn--quiet" onclick={overwrite}>Overwrite</button>
    </div>
  {:else if empty && template}
    <div class="bar"><button type="button" class="w-btn w-btn--quiet" onclick={insertTemplate}>Insert template</button></div>
  {/if}
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
  .conflict {
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
