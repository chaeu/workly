<script lang="ts">
  import { ask } from "@tauri-apps/plugin-dialog";
  import { SvelteSet } from "svelte/reactivity";
  import MarkdownEditor from "./MarkdownEditor.svelte";
  import { flatFiles, isWebLink, linkTarget, renderMarkdown } from "$lib/markdown";
  import {
    workspace,
    projectFiles,
    readMarkdown,
    createFile,
    createFolder,
    deletePath,
    projectTemplate,
    type FileNode,
    openWith,
    openWithApps,
    appLabel,
    openRepo,
    openUrl,
    reveal,
    agentsEnabled,
    agentContext,
    createAgentsMd,
    type AgentContext,
    type ProjectEntry,
  } from "$lib/stores/workspace.svelte";

  /** `selected` lives in the page, so switching tabs keeps the open file. */
  let { project, selected = $bindable(null) }: { project: ProjectEntry; selected?: string | null } = $props();

  const overview = $derived(`${project.path}/_project.md`);

  // Everything in the project except tasks/ and agent/ (own section below, behind the agent switch).
  let files = $state<FileNode[]>([]);
  const flat = $derived(flatFiles(files));
  /** Folders the user collapsed; all others are open. */
  const closed = new SvelteSet<string>();
  /** The open editor holds text it could not save (conflict); switching files asks first. */
  let unsaved = $state(false);
  /** A file just created opens with the cursor in it. */
  let created = $state<string | null>(null);
  /** Inline name input for a new file or folder in `dir`. */
  let adding = $state<{ dir: string; kind: "file" | "folder" } | null>(null);
  let addName = $state("");
  let addError = $state<string | null>(null);
  let agent = $state<AgentContext | null>(null);
  let html = $state("");
  let previewError = $state<string | null>(null);

  // Selection from another project, or an agent file with agent features off, falls back to the overview.
  const current = $derived(
    selected?.startsWith(project.path + "/") && (agentsEnabled() || !selected.startsWith(project.path + "/agent/")) ? selected : overview,
  );
  const isMarkdown = $derived(current.endsWith(".md"));
  // agent/ files stay read-only here; _project.md is edited (body only) but deleted with the project.
  const isAgent = $derived(current.startsWith(project.path + "/agent/"));
  const canEdit = $derived(isMarkdown && !isAgent);
  const canDelete = $derived(current !== overview && !isAgent);

  /** Switch the open file; asks first when the editor has unsaved changes. */
  async function select(path: string) {
    if (path === current) return;
    if (unsaved && !(await ask("Your changes to this file are not saved. Discard them?", { title: "Unsaved changes", kind: "warning", okLabel: "Discard" }))) return;
    unsaved = false;
    selected = path;
  }

  function startAdd(dir: string, kind: "file" | "folder") {
    closed.delete(dir);
    adding = { dir, kind };
    addName = "";
    addError = null;
  }

  async function add() {
    if (!adding || !addName.trim()) return;
    const { dir, kind } = adding;
    try {
      const path = kind === "file" ? await createFile(project.key, dir, addName) : await createFolder(project.key, dir, addName);
      adding = null;
      if (kind === "file") {
        created = path;
        await select(path);
      }
    } catch (e) {
      addError = String(e);
    }
  }

  function onAddKey(e: KeyboardEvent) {
    if (e.key === "Enter") {
      e.preventDefault();
      add();
    } else if (e.key === "Escape") {
      e.preventDefault();
      adding = null;
    }
  }

  async function remove(path: string, node?: FileNode) {
    const name = path.split("/").pop();
    const n = node?.children ? flatFiles(node.children).length : 0;
    const what = node?.children ? `the folder “${name}” with ${n} ${n === 1 ? "file" : "files"}` : `“${name}”`;
    const ok = await ask(`Move ${what} to the trash? You can restore it from Trash.`, { title: "Delete", kind: "warning", okLabel: "Move to trash" });
    if (!ok) return;
    await deletePath(path);
    if (current === path || current.startsWith(path + "/")) selected = overview;
  }

  // Reload tree and preview on every index change, so edits in Obsidian show up live.
  $effect(() => {
    void workspace.reloads;
    projectFiles(project.key).then(
      (f) => (files = f),
      () => (files = []),
    );
  });

  $effect(() => {
    void workspace.reloads;
    if (!agentsEnabled()) return;
    agentContext(project.key).then(
      (c) => (agent = c),
      () => (agent = null),
    );
  });

  async function createAgents() {
    const path = await createAgentsMd(project.key);
    if (path) selected = path;
  }

  $effect(() => {
    void workspace.reloads;
    const path = current;
    if (!path || !path.endsWith(".md") || canEdit) {
      html = "";
      return;
    }
    readMarkdown(path).then(
      (src) => {
        if (path !== current) return;
        html = renderMarkdown(src);
        previewError = null;
      },
      (e) => (previewError = String(e)),
    );
  });

  const shortPath = (f: string) => f.slice(project.path.length + 1);

  // Links in the preview must never navigate the app window.
  function onPreviewClick(e: MouseEvent) {
    const a = (e.target as HTMLElement).closest("a");
    if (!a) return;
    e.preventDefault();
    openLink(a.getAttribute("href") ?? "", false);
  }

  /** A link in the preview or editor: web links in the browser, project files here. */
  function openLink(href: string, wiki: boolean) {
    if (isWebLink(href)) return openUrl(href);
    const target = linkTarget(current, href, wiki, [overview, ...flat]);
    if (target) select(target);
  }
</script>

{#snippet addRow(depth: number)}
  <div class="add-row" style:--depth={depth}>
    <!-- Names stay as typed: no macOS auto-capitalisation or autocorrect. -->
    <input
      class="name"
      autocapitalize="off" autocorrect="off" spellcheck="false"
      aria-label={adding?.kind === "file" ? "New file name" : "New folder name"}
      placeholder={adding?.kind === "file" ? "File name" : "Folder name"}
      bind:value={addName}
      onkeydown={onAddKey}
      onblur={() => !addName.trim() && (adding = null)}
      {@attach (el) => el.focus()}
    />
    {#if addError}<div class="add-error" role="alert">{addError}</div>{/if}
  </div>
{/snippet}

{#snippet tree(nodes: FileNode[], depth: number)}
  {#each nodes as n (n.path)}
    {#if n.children}
      {@const open = !closed.has(n.path)}
      <div class="folder-row" style:--depth={depth}>
        <button class="file folder" aria-expanded={open} title={n.path} onclick={() => (open ? closed.add(n.path) : closed.delete(n.path))}
          ><span class="caret" aria-hidden="true">{open ? "▾" : "▸"}</span>{n.name}</button
        >
        <span class="acts">
          <button type="button" class="act" title="New file in {n.name}" onclick={() => startAdd(n.path, "file")}>+ File</button>
          <button type="button" class="act" title="New folder in {n.name}" onclick={() => startAdd(n.path, "folder")}>+ Folder</button>
          <button type="button" class="act" title="Delete {n.name}" onclick={() => remove(n.path, n)}>Delete</button>
        </span>
      </div>
      {#if open}
        {#if adding?.dir === n.path}{@render addRow(depth + 1)}{/if}
        {@render tree(n.children, depth + 1)}
      {/if}
    {:else}
      <button class="file leaf" style:--depth={depth} aria-current={current === n.path ? "true" : undefined} title={n.path} onclick={() => select(n.path)}
        >{n.name}</button
      >
    {/if}
  {/each}
{/snippet}

<div class="layout">
  <nav class="w-tray side" aria-label="Project files">
    <button class="file" aria-current={current === overview ? "true" : undefined} onclick={() => select(overview)}>Overview</button>
    <div class="w-caps group head">
      <span>Files</span>
      <button type="button" class="act" onclick={() => startAdd(project.path, "file")}>New file</button>
      <button type="button" class="act" onclick={() => startAdd(project.path, "folder")}>New folder</button>
    </div>
    {#if adding?.dir === project.path}{@render addRow(0)}{/if}
    {@render tree(files, 0)}

    {#if agentsEnabled() && agent}
      <div class="w-caps group">Agent</div>
      {#if agent.project}
        {@const md = agent.project.path}
        <button class="file" aria-current={current === md ? "true" : undefined} title={md} onclick={() => select(md)}>AGENTS.md</button>
      {:else}
        <button class="file add" onclick={createAgents}>Create AGENTS.md</button>
      {/if}
      {#each agent.skills as s (s.path)}
        <button class="file skill" aria-current={current === s.path ? "true" : undefined} title={s.path} onclick={() => select(s.path)}
          >{s.name}{#if s.description}<span class="desc">{s.description}</span>{/if}</button
        >
      {/each}
    {/if}

    {#if project.repos.length}
      <div class="w-caps group">Repos</div>
      {#each project.repos as r (r)}
        {#if workspace.index?.missing_repos.some((m) => m.key === project.key && m.repo === r)}
          <div class="file w-mono missing" title="This folder does not exist on this Mac">{r}<span class="desc">Not found on this Mac</span></div>
        {:else}
          <button class="file w-mono" title="Open in VS Code" onclick={() => openRepo(r)}>{r}</button>
        {/if}
      {/each}
    {/if}
    {#if project.links.length}
      <div class="w-caps group">Links</div>
      {#each project.links as l (l.url)}
        <button class="file" title={l.url} onclick={() => openUrl(l.url)}>{l.label ?? l.url}</button>
      {/each}
    {/if}
  </nav>

  <section class="preview">
    <div class="preview-head">
      <span class="w-mono path">{shortPath(current)}</span>
      {#if canDelete}<button class="w-btn w-btn--quiet" onclick={() => remove(current)}>Delete</button>{/if}
      {#each openWithApps() as app (app)}
        <button class="w-btn w-btn--quiet" onclick={() => openWith(app, current)}>{appLabel(app)}</button>
      {/each}
      <button class="w-btn w-btn--quiet" onclick={() => reveal(current)}>Finder</button>
    </div>
    {#if canEdit}
      <!-- Markdown opens straight in the editor, like Obsidian; agent/ files stay a read-only preview. -->
      {#key current}
        <MarkdownEditor path={current} bind:unsaved onlink={openLink} autofocus={current === created} template={current === overview ? projectTemplate : undefined} />
      {/key}
    {:else if previewError}
      <p class="w-sub">{previewError}</p>
    {:else if isMarkdown}
      <!-- Sanitised by DOMPurify in renderMarkdown. -->
      <!-- eslint-disable-next-line svelte/no-at-html-tags -->
      <!-- svelte-ignore a11y_click_events_have_key_events, a11y_no_static_element_interactions -->
      <div class="md" onclick={onPreviewClick}>{@html html}</div>
    {:else}
      <p class="w-sub">No preview for this file type.</p>
    {/if}
  </section>
</div>

<style>
  /* Fills the page below the bar; tree and preview scroll on their own. */
  .layout {
    flex: 1;
    min-height: 300px;
    display: grid;
    grid-template-columns: 250px minmax(0, 1fr);
    grid-template-rows: minmax(0, 1fr);
    gap: var(--w-s-4);
  }
  .side {
    gap: 2px;
    overflow-y: auto;
  }
  .group {
    padding: var(--w-s-3) 8px var(--w-s-1);
  }
  .file {
    display: block;
    width: 100%;
    min-height: 28px;
    padding: 5px 8px;
    border: 0;
    border-radius: var(--w-r-md);
    background: none;
    text-align: left;
    font-size: var(--w-fs-small);
    cursor: pointer;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .missing {
    cursor: default;
    color: var(--w-muted);
  }
  .file {
    padding-left: calc(8px + var(--depth, 0) * 14px);
  }
  .head {
    display: flex;
    align-items: center;
    gap: var(--w-s-1);
  }
  .head span {
    flex: 1;
  }
  .act {
    border: 0;
    background: none;
    padding: 2px 4px;
    border-radius: var(--w-r-sm);
    color: var(--w-muted);
    font: 500 var(--w-fs-micro) var(--w-font-label);
    text-transform: none;
    letter-spacing: 0;
    cursor: pointer;
    white-space: nowrap;
  }
  .act:hover {
    background: var(--w-surface);
    color: var(--w-ink);
  }
  .folder-row {
    display: flex;
    align-items: center;
  }
  .folder-row .file {
    flex: 1;
    min-width: 0;
  }
  /* Files line up with the folder names, past the caret. */
  .leaf {
    padding-left: calc(20px + var(--depth, 0) * 14px);
  }
  .folder {
    color: var(--w-muted);
    font-weight: 500;
  }
  .caret {
    display: inline-block;
    width: 12px;
    font-size: var(--w-fs-micro);
  }
  /* Folder actions show on hover or keyboard focus. */
  .folder-row .acts {
    display: none;
  }
  .folder-row:hover .acts,
  .folder-row:focus-within .acts {
    display: flex;
  }
  .add-row {
    padding: 2px 4px 2px calc(20px + var(--depth, 0) * 14px);
  }
  .name {
    width: 100%;
    padding: 4px 6px;
    border: 1px solid var(--w-line);
    border-radius: var(--w-r-sm);
    background: var(--w-surface);
    color: var(--w-ink);
    font: var(--w-fs-small) var(--w-font-sans);
  }
  .name:focus {
    outline: none;
    border-color: var(--w-accent);
  }
  .add-error {
    padding-top: 2px;
    color: var(--w-danger);
    font-size: var(--w-fs-micro);
  }
  .file:hover {
    background: color-mix(in srgb, var(--w-surface) 55%, transparent);
  }
  .file[aria-current="true"] {
    background: var(--w-surface);
    box-shadow: var(--w-shadow-raised);
    font-weight: 500;
  }
  .add {
    color: var(--w-accent);
  }
  .skill .desc,
  .missing .desc {
    display: block;
    overflow: hidden;
    text-overflow: ellipsis;
    color: var(--w-muted);
    font-size: var(--w-fs-micro);
  }
  .missing .desc {
    color: var(--w-warn);
  }
  .file.w-mono {
    font-size: var(--w-fs-micro);
  }
  .preview {
    background: var(--w-surface);
    border-radius: var(--w-r-lg);
    box-shadow: var(--w-shadow-panel);
    padding: var(--w-s-3) var(--w-s-6) var(--w-s-6);
    overflow-y: auto;
  }
  .preview-head {
    display: flex;
    align-items: center;
    gap: var(--w-s-1);
    padding-bottom: var(--w-s-2);
    border-bottom: 1px solid var(--w-line);
  }
  .preview-head .path {
    flex: 1;
    color: var(--w-muted);
  }
  .preview-head .w-btn {
    padding: 4px 8px;
    font-size: var(--w-fs-caption);
  }
</style>
