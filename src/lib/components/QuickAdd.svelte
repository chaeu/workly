<script lang="ts">
  import { workspace, createTask } from "$lib/stores/workspace.svelte";

  let { project: initial, onclose }: { project: string | null; onclose: (id?: string) => void } = $props();

  const projects = $derived(workspace.index?.projects.filter((p) => p.status !== "archived") ?? []);
  let title = $state("");
  // svelte-ignore state_referenced_locally
  let project = $state(initial ?? "");
  let priority = $state(2);
  let busy = $state(false);

  async function save(e: SubmitEvent) {
    e.preventDefault();
    if (!title.trim() || busy) return;
    busy = true;
    const id = await createTask(title.trim(), project || null, priority);
    busy = false;
    if (id) onclose(id);
  }
</script>

<div class="w-scrim" role="presentation" onclick={() => onclose()}></div>
<div class="w-modal" role="dialog" aria-modal="true" aria-labelledby="qa-title">
  <form onsubmit={save}>
    <h2 class="w-h2" id="qa-title">New task</h2>
    <!-- svelte-ignore a11y_autofocus -->
    <input bind:value={title} autofocus placeholder="What needs doing?" aria-label="Title" />
    <div class="row">
      <select bind:value={project} aria-label="Project">
        <option value="">Inbox</option>
        {#each projects as p (p.key)}<option value={p.key}>{p.title}</option>{/each}
      </select>
      <div class="w-seg" role="group" aria-label="Priority">
        {#each [1, 2, 3] as p (p)}
          <button type="button" aria-pressed={priority === p} onclick={() => (priority = p)}>P{p}</button>
        {/each}
      </div>
      <button type="submit" class="w-btn w-btn--primary" disabled={!title.trim() || busy}>Add task</button>
    </div>
  </form>
</div>

<style>
  .w-modal {
    width: min(520px, calc(100% - 32px));
    top: 28%;
  }
  form {
    display: flex;
    flex-direction: column;
    gap: var(--w-s-3);
    padding: var(--w-s-5) var(--w-s-6);
  }
  .row {
    display: flex;
    align-items: center;
    gap: var(--w-s-2);
  }
  input,
  select {
    border: 0;
    outline: none;
    background: var(--w-tray);
    border-radius: var(--w-r-md);
    padding: 7px 11px;
    font-size: var(--w-fs-small);
    min-width: 0;
  }
  input {
    font-size: var(--w-fs-body);
  }
  select {
    flex: 1;
  }
  input:focus,
  select:focus {
    box-shadow: 0 0 0 2px var(--w-accent-soft);
  }
  .w-btn--primary:disabled {
    opacity: 0.5;
    cursor: default;
  }
</style>
