<script lang="ts">
  // Hidden debug view (⌘0): raw index, live reloads, every core command.
  import { workspace, startWorkspace, updateTaskField, createTask, moveTask, deleteTask, restore } from "$lib/stores/workspace.svelte";
  import type { TaskEntry } from "$lib/stores/workspace.svelte";

  startWorkspace();

  const statuses = ["backlog", "todo", "doing", "review", "done"];
  const nextStatus = (s: string) => statuses[(statuses.indexOf(s) + 1) % statuses.length];

  let newTitle = $state("");

  const index = $derived(workspace.index);
  const groups = $derived(
    index
      ? [
          ...index.projects.map((p) => ({ key: p.key as string | null, label: `${p.key} · ${p.title}`, sub: `${p.path} · ${p.status}`, path: p.path as string | null })),
          { key: null, label: "Inbox", sub: "inbox", path: null },
        ]
      : [],
  );
  const tasksOf = (path: string | null): TaskEntry[] => index?.tasks.filter((t) => t.project === path) ?? [];

  async function add(key: string | null) {
    if (!newTitle.trim()) return;
    await createTask(newTitle.trim(), key);
    newTitle = "";
  }
</script>

<header class="w-page-head">
  <h1 class="w-h1">Debug</h1>
  <div class="w-toolbar w-mono">reloads {workspace.reloads} · {workspace.loadedAt}</div>
</header>

{#if workspace.error}
  <p class="err">{workspace.error}</p>
{/if}

{#if index}
  <label class="w-search"><input placeholder="Title for a new task" bind:value={newTitle} /></label>

  {#if index.errors.length}
    <section>
      <h2 class="w-h2">Errors</h2>
      {#each index.errors as e (e.path + e.message)}
        <div class="row err"><span class="w-mono">{e.path}{e.line ? `:${e.line}` : ""}</span><span>{e.message}</span></div>
      {/each}
    </section>
  {/if}

  {#each groups as g (g.label)}
    <section>
      <div class="head">
        <h2 class="w-h2">{g.label}</h2>
        <span class="w-sub">{g.sub}</span>
        <button class="w-btn w-btn--quiet" onclick={() => add(g.key)}>Add task</button>
      </div>
      {#each tasksOf(g.path) as t (t.id)}
        <div class="row">
          <span class="w-mono">{t.id}</span>
          <span class="title">{t.title}</span>
          {#if t.agent?.active}<span class="w-mono agent">{t.agent.active}</span>{/if}
          <button class="w-btn w-btn--quiet" title="Next status" onclick={() => updateTaskField(t.id, "status", nextStatus(t.status))}>{t.status}</button>
          <select value="" onchange={(e) => moveTask(t.id, e.currentTarget.value || null)}>
            <option value="" disabled>Move…</option>
            {#each index.projects as p (p.key)}<option value={p.key}>{p.key}</option>{/each}
            <option value="">Inbox</option>
          </select>
          <button class="w-btn w-btn--quiet" onclick={() => deleteTask(t.id)}>Delete</button>
        </div>
      {/each}
    </section>
  {/each}

  <section>
    <h2 class="w-h2">Trash</h2>
    {#each index.trash as path (path)}
      <div class="row">
        <span class="w-mono title">{path}</span>
        <button class="w-btn w-btn--quiet" onclick={() => restore(path)}>Restore</button>
      </div>
    {:else}
      <p class="w-sub">Empty</p>
    {/each}
  </section>
{/if}

<style>
  section {
    display: flex;
    flex-direction: column;
    gap: var(--w-s-1);
  }
  .head,
  .row {
    display: flex;
    align-items: center;
    gap: var(--w-s-3);
  }
  .row {
    padding: var(--w-s-1) var(--w-s-2);
    border-radius: var(--w-r-sm);
    background: var(--w-surface);
    font-size: var(--w-fs-small);
  }
  .title {
    flex: 1;
    min-width: 0;
  }
  .agent {
    color: var(--w-accent);
  }
  .err {
    color: var(--w-danger);
    background: var(--w-danger-soft);
  }
  select {
    font: inherit;
    color: var(--w-ink);
    background: var(--w-tray);
    border: 0;
    border-radius: var(--w-r-sm);
  }
</style>
