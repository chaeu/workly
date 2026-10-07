<script lang="ts">
  import { ask } from "@tauri-apps/plugin-dialog";
  import { workspace, trashItems, restore, emptyTrash, type TrashItem } from "$lib/stores/workspace.svelte";

  let items = $state<TrashItem[]>([]);
  // Reload on every index change: deletes and restores elsewhere show up here.
  $effect(() => {
    void workspace.reloads;
    trashItems().then(
      (t) => (items = t),
      (e) => (workspace.error = String(e)),
    );
  });

  const deleted = (iso: string | null) =>
    iso ? new Date(iso).toLocaleString("en-GB", { day: "numeric", month: "short", hour: "2-digit", minute: "2-digit" }) : "";
  const folder = (path: string) => path.split("/").slice(0, -1).join("/") || ".";

  async function empty() {
    const n = items.length;
    const ok = await ask(
      `Move ${n} ${n === 1 ? "item" : "items"} to the macOS Trash? They leave the workspace but stay recoverable in Finder until you empty the Trash there. Their ids stay taken.`,
      { title: "Empty trash", kind: "warning", okLabel: "Move to Trash" },
    );
    if (ok) await emptyTrash();
  }
</script>

<header class="w-page-head">
  <div>
    <h1 class="w-h1">Trash</h1>
    <p class="w-sub">
      {items.length
        ? `${items.length} ${items.length === 1 ? "item" : "items"} in .workly/trash/. Restore puts each back where it was.`
        : "Empty. Deleted tasks and projects wait here until you restore them or empty the trash."}
    </p>
  </div>
  {#if items.length}
    <div class="w-toolbar">
      <button class="w-btn" onclick={empty}>Empty trash…</button>
    </div>
  {/if}
</header>

{#if items.length}
  <div class="w-tray list">
    {#each items as it (it.path)}
      <div class="row">
        <span class="w-mono id">{it.id}</span>
        <div class="text">
          <span class="title">{it.title}{#if it.kind === "project"}<span class="w-type">Project</span>{/if}</span>
          <span class="w-mono where" title={it.path}>{it.kind === "project" ? it.path : folder(it.path)}</span>
        </div>
        <span class="w-mono when">{deleted(it.deleted_at)}</span>
        <button class="w-btn" onclick={() => restore(it.path)}>Restore</button>
      </div>
    {/each}
  </div>
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
    padding: 8px 8px 8px 12px;
    border-radius: var(--w-r-md);
    background: var(--w-surface);
    box-shadow: var(--w-shadow-card);
  }
  .id {
    width: 7ch;
    flex: none;
    color: var(--w-muted);
  }
  .text {
    flex: 1;
    min-width: 0;
    display: flex;
    flex-direction: column;
    gap: 2px;
  }
  .title {
    display: flex;
    align-items: center;
    gap: var(--w-s-2);
    font-weight: 500;
  }
  .where {
    color: var(--w-muted);
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
    user-select: text;
  }
  .when {
    color: var(--w-muted);
    white-space: nowrap;
  }
</style>
