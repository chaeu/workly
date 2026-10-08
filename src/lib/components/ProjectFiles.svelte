<script lang="ts">
  import { renderMarkdown, resolveLink } from "$lib/markdown";
  import {
    workspace,
    projectFiles,
    readMarkdown,
    openInObsidian,
    openInVscode,
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

  // agent/ has its own section below, behind the agent switch.
  const DOC_DIRS = ["docs", "notes", "decisions"];

  const overview = $derived(`${project.path}/_project.md`);

  let files = $state<string[]>([]);
  let agent = $state<AgentContext | null>(null);
  let html = $state("");
  let previewError = $state<string | null>(null);

  // Selection from another project, or an agent file with agent features off, falls back to the overview.
  const current = $derived(
    selected?.startsWith(project.path + "/") && (agentsEnabled() || !selected.startsWith(project.path + "/agent/")) ? selected : overview,
  );
  const isMarkdown = $derived(current.endsWith(".md"));

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
    if (!path || !path.endsWith(".md")) {
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

  const group = (dir: string) => files.filter((f) => f.startsWith(`${project.path}/${dir}/`));
  const inGroup = (dir: string, f: string) => f.slice(`${project.path}/${dir}/`.length);
  const shortPath = (f: string) => f.slice(project.path.length + 1);

  // Links in the preview must never navigate the app window.
  function onPreviewClick(e: MouseEvent) {
    const a = (e.target as HTMLElement).closest("a");
    if (!a) return;
    e.preventDefault();
    const href = a.getAttribute("href") ?? "";
    if (/^(https?|mailto):/i.test(href)) openUrl(href);
    else if (href && !href.includes(":") && !href.startsWith("#")) {
      const target = resolveLink(current, href);
      if (target === overview || files.includes(target)) selected = target;
    }
  }
</script>

<div class="layout">
  <nav class="w-tray side" aria-label="Project files">
    <button class="file" aria-current={current === overview ? "true" : undefined} onclick={() => (selected = overview)}>Overview</button>
    {#each DOC_DIRS as dir (dir)}
      <div class="w-caps group">{dir}</div>
      {#each group(dir) as f (f)}
        <button class="file" aria-current={current === f ? "true" : undefined} title={f} onclick={() => (selected = f)}>{inGroup(dir, f)}</button>
      {/each}
    {/each}

    {#if agentsEnabled() && agent}
      <div class="w-caps group">Agent</div>
      {#if agent.project}
        {@const md = agent.project.path}
        <button class="file" aria-current={current === md ? "true" : undefined} title={md} onclick={() => (selected = md)}>AGENTS.md</button>
      {:else}
        <button class="file add" onclick={createAgents}>Create AGENTS.md</button>
      {/if}
      {#each agent.skills as s (s.path)}
        <button class="file skill" aria-current={current === s.path ? "true" : undefined} title={s.path} onclick={() => (selected = s.path)}
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
      <button class="w-btn w-btn--quiet" onclick={() => openInObsidian(current)}>Obsidian</button>
      <button class="w-btn w-btn--quiet" onclick={() => openInVscode(current)}>VS Code</button>
      <button class="w-btn w-btn--quiet" onclick={() => reveal(current)}>Finder</button>
    </div>
    {#if previewError}
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
