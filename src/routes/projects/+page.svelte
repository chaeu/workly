<script lang="ts">
  import ProjectForm from "$lib/components/ProjectForm.svelte";
  import { workspace, openTasks, stepLabel, projColor, reorderProjects, type ProjectEntry } from "$lib/stores/workspace.svelte";

  let creating = $state(false);

  const projects = $derived(workspace.index?.projects ?? []);
  const live = $derived(projects.filter((p) => p.status !== "archived"));
  const archived = $derived(projects.filter((p) => p.status === "archived"));
  const count = (s: string) => projects.filter((p) => p.status === s).length;

  // Drag to sort: `draft` is the order shown while dragging, written as `order` on drop.
  let dragKey = $state<string | null>(null);
  let draft = $state<string[] | null>(null);
  let dropped = false;
  const shown = $derived(draft ? draft.map((k) => live.find((p) => p.key === k)).filter((p) => p !== undefined) : live);

  // A new index (after our write or an external change) replaces the draft.
  $effect(() => {
    void workspace.index;
    draft = null;
  });

  function dragstart(e: DragEvent, p: ProjectEntry) {
    dragKey = p.key;
    dropped = false;
    e.dataTransfer!.effectAllowed = "move";
    e.dataTransfer!.setData("text/plain", p.key);
  }

  function dragover(e: DragEvent, p: ProjectEntry) {
    if (!dragKey) return;
    e.preventDefault();
    if (p.key === dragKey) return;
    const keys = (draft ?? live.map((x) => x.key)).filter((k) => k !== dragKey);
    const box = (e.currentTarget as HTMLElement).getBoundingClientRect();
    const at = keys.indexOf(p.key) + (e.clientY > box.top + box.height / 2 ? 1 : 0);
    keys.splice(at, 0, dragKey);
    draft = keys;
  }

  function drop(e: DragEvent) {
    if (!dragKey) return;
    e.preventDefault();
    dropped = true;
    if (draft) reorderProjects([...draft, ...archived.map((p) => p.key)]);
  }

  function dragend() {
    dragKey = null;
    if (!dropped) draft = null;
  }
</script>

{#snippet row(p: ProjectEntry, sortable: boolean)}
  <a
    class="row"
    class:is-dragging={dragKey === p.key}
    href="/projects/{p.key}"
    draggable={sortable}
    ondragstart={(e) => dragstart(e, p)}
    ondragover={(e) => sortable && dragover(e, p)}
    ondragend={dragend}
  >
    <span class="w-proj-mark" style:--c={projColor(p.color)}></span>
    <span class="w-mono key">{p.key}</span>
    <span class="title">{p.title}</span>
    {#if p.status === "paused"}<span class="w-sub">Paused</span>{/if}
    {#if stepLabel(p)}<span class="w-sub step">{stepLabel(p)}</span>{/if}
    <span class="w-mono open">{openTasks(p)} open</span>
  </a>
{/snippet}

<header class="w-page-head">
  <div>
    <h1 class="w-h1">Projects</h1>
    <p class="w-sub">
      {count("active")} active{#if count("paused")} · {count("paused")} paused{/if}{#if archived.length} · {archived.length} archived{/if}
    </p>
  </div>
  <div class="w-toolbar">
    <button class="w-btn w-btn--primary" onclick={() => (creating = true)}>+ New project</button>
  </div>
</header>

{#if !projects.length}
  <p class="w-sub">No projects yet. A project is a folder with a _project.md, made here or from the template in Obsidian.</p>
{/if}

{#if live.length}
  <div class="w-tray list" role="list" ondragover={(e) => dragKey && e.preventDefault()} ondrop={drop}>
    {#each shown as p (p.key)}
      {@render row(p, true)}
    {/each}
  </div>
{/if}

{#if archived.length}
  <details class="archive">
    <summary class="w-caps">Archived · {archived.length}</summary>
    <div class="w-tray list">
      {#each archived as p (p.key)}
        {@render row(p, false)}
      {/each}
    </div>
  </details>
{/if}

{#if creating}
  <ProjectForm onclose={() => (creating = false)} />
{/if}

<style>
  .w-page-head p {
    margin: 4px 0 0;
  }
  .list {
    max-width: 900px;
  }
  .row {
    display: flex;
    align-items: center;
    gap: var(--w-s-3);
    padding: 10px 12px;
    min-height: 28px;
    border-radius: var(--w-r-md);
    background: var(--w-surface);
    box-shadow: var(--w-shadow-card);
    color: var(--w-ink);
    text-decoration: none;
    transition: box-shadow var(--w-dur) var(--w-ease);
  }
  .row:hover {
    box-shadow:
      var(--w-shadow-card),
      0 0 0 1px var(--w-line);
  }
  .row.is-dragging {
    opacity: 0.3;
  }
  .key {
    width: 6ch;
    color: var(--w-muted);
  }
  .title {
    flex: 1;
    min-width: 0;
    font-weight: 500;
  }
  .step {
    white-space: nowrap;
  }
  .open {
    width: 8ch;
    text-align: right;
    color: var(--w-muted);
  }
  .archive {
    display: flex;
    flex-direction: column;
    gap: var(--w-s-2);
  }
  .archive summary {
    cursor: pointer;
    padding: var(--w-s-1) 0;
    margin-bottom: var(--w-s-2);
  }
  .archive .row {
    background: transparent;
    box-shadow: inset 0 0 0 1px var(--w-line);
  }
  .archive .title {
    color: var(--w-muted);
  }
</style>
