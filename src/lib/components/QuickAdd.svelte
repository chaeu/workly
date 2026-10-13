<script lang="ts">
  import { tick } from "svelte";
  import { grow } from "$lib/grow";
  import { workspace, createTask, inDailyWork, agentsEnabled } from "$lib/stores/workspace.svelte";
  import { EFFORTS, RUNNERS, agentName } from "$lib/tasks.svelte";

  let {
    project: initial,
    parent: initialParent = null,
    onclose,
  }: { project: string | null; parent?: string | null; onclose: (id?: string) => void } = $props();

  const projects = $derived(workspace.index?.projects.filter(inDailyWork) ?? []);
  // One level: only tasks without a parent can be parents.
  const parents = $derived(workspace.index?.tasks.filter((t) => !t.parent && t.status !== "done") ?? []);
  let title = $state("");
  // svelte-ignore state_referenced_locally
  let project = $state(initial ?? "");
  let priority = $state(2);
  // svelte-ignore state_referenced_locally
  let more = $state(!!initialParent);
  let due = $state("");
  // svelte-ignore state_referenced_locally
  let parent = $state(initialParent ?? "");
  let tags = $state("");
  let description = $state("");
  let criteria = $state<string[]>([""]);
  let runner = $state("");
  let model = $state("");
  let effort = $state("");
  let busy = $state(false);
  // `autofocus` does not fire when the layout mounts this later.
  let input: HTMLInputElement;
  $effect(() => input.focus());

  async function save(e?: SubmitEvent) {
    e?.preventDefault();
    if (!title.trim() || busy) return;
    busy = true;
    const id = await createTask({
      title: title.trim(),
      project: project || null,
      priority,
      ...(more && {
        due: due || null,
        parent: parent.trim() || null,
        tags: tags.split(/[,\s]+/).map((x) => x.replace(/^#/, "")).filter(Boolean),
        description,
        acceptance: criteria.map((c) => c.trim()).filter(Boolean),
        runner: runner || null,
        model: model.trim() || null,
        effort: effort || null,
      }),
    });
    busy = false;
    if (id) onclose(id);
  }

  // ⌘↵: first expands, then saves.
  function onFormKey(e: KeyboardEvent) {
    if (e.key === "Enter" && e.metaKey) {
      e.preventDefault();
      if (more) save();
      else more = true;
    }
  }

  // Acceptance list: Enter adds a line below, Backspace on an empty line removes it.
  let rows: HTMLInputElement[] = $state([]);
  async function focusRow(i: number) {
    await tick();
    rows[i]?.focus();
  }
  function onCriterionKey(e: KeyboardEvent, i: number) {
    if (e.metaKey) return;
    if (e.key === "Enter") {
      e.preventDefault();
      criteria.splice(i + 1, 0, "");
      focusRow(i + 1);
    } else if (e.key === "Backspace" && !criteria[i] && criteria.length > 1) {
      e.preventDefault();
      criteria.splice(i, 1);
      focusRow(Math.max(0, i - 1));
    }
  }
</script>

<div class="w-scrim" role="presentation" onclick={() => onclose()}></div>
<!-- Esc closes only this dialog, not a card below it. -->
<div
  class="w-modal"
  class:more
  role="dialog"
  aria-modal="true"
  aria-labelledby="qa-title"
  tabindex="-1"
  onkeydown={(e) => {
    if (e.key === "Escape") {
      e.stopPropagation();
      onclose();
    } else onFormKey(e);
  }}
>
  <form onsubmit={save}>
    <h2 class="w-h2" id="qa-title">{initialParent ? `New sub-task of ${initialParent}` : "New task"}</h2>
    <input bind:this={input} bind:value={title} placeholder="What needs doing?" aria-label="Title" />
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
      {#if !more}
        <button type="button" class="w-btn w-btn--quiet" title="⌘↵" onclick={() => (more = true)}>More</button>
      {/if}
      <button type="submit" class="w-btn w-btn--primary" disabled={!title.trim() || busy}>Add task</button>
    </div>

    {#if more}
      <div class="kv">
        <label for="qa-due">Due</label>
        <input id="qa-due" type="date" bind:value={due} />
        <label for="qa-parent">Parent</label>
        <input id="qa-parent" class="w-mono" list="qa-parents" bind:value={parent} placeholder="WR-12" />
        <datalist id="qa-parents">
          {#each parents as t (t.id)}<option value={t.id}>{t.title}</option>{/each}
        </datalist>
      </div>
      <textarea bind:value={description} aria-label="Description (Markdown)" placeholder="Description" use:grow></textarea>
      <div class="crit">
        <span class="w-caps">Acceptance</span>
        {#each criteria as _, i (i)}
          <label class="crit-row">
            <span class="box" aria-hidden="true"></span>
            <input
              bind:this={rows[i]}
              bind:value={criteria[i]}
              placeholder={i === 0 ? "Criterion – Enter adds the next" : ""}
              aria-label="Acceptance criterion {i + 1}"
              onkeydown={(e) => onCriterionKey(e, i)}
            />
          </label>
        {/each}
      </div>
      <details>
        <summary class="w-caps">Tags</summary>
        <input class="w-mono" bind:value={tags} placeholder="frontend, ux" aria-label="Tags, comma separated" />
      </details>
      {#if agentsEnabled()}
        <details>
          <summary class="w-caps">Agent</summary>
          <div class="kv">
            <label for="qa-runner">Runner</label>
            <select id="qa-runner" bind:value={runner}>
              <option value="">Default</option>
              {#each RUNNERS as r (r)}<option value={r}>{r === "auto" ? "auto" : agentName(r)}</option>{/each}
            </select>
            <label for="qa-model">Model</label>
            <input id="qa-model" bind:value={model} placeholder="auto" />
            <label for="qa-effort">Effort</label>
            <select id="qa-effort" bind:value={effort}>
              <option value="">Default</option>
              {#each EFFORTS as x (x)}<option value={x}>{x}</option>{/each}
            </select>
          </div>
        </details>
      {/if}
      <p class="w-sub hint">⌘↵ adds the task</p>
    {/if}
  </form>
</div>

<style>
  .w-modal {
    width: min(520px, calc(100% - 32px));
    top: 28%;
  }
  .w-modal.more {
    width: min(600px, calc(100% - 32px));
    top: 12%;
    max-height: 80%;
    overflow-y: auto;
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
  select,
  textarea {
    border: 0;
    outline: none;
    background: var(--w-tray);
    border-radius: var(--w-r-md);
    padding: 7px 11px;
    font-size: var(--w-fs-small);
    min-width: 0;
    box-sizing: border-box;
  }
  input:first-of-type {
    font-size: var(--w-fs-body);
  }
  .row select {
    flex: 1;
  }
  textarea {
    min-height: 72px;
    resize: none;
    overflow: hidden;
    line-height: 1.5;
  }
  input:focus,
  select:focus,
  textarea:focus {
    box-shadow: 0 0 0 2px var(--w-accent-soft);
  }
  .kv {
    display: grid;
    grid-template-columns: auto 1fr;
    gap: 6px 12px;
    align-items: center;
    font-size: var(--w-fs-small);
  }
  .kv label {
    color: var(--w-muted);
  }
  .crit {
    display: grid;
    gap: 4px;
  }
  .crit-row {
    display: flex;
    align-items: center;
    gap: var(--w-s-2);
  }
  .crit-row input {
    flex: 1;
    padding: 4px 8px;
  }
  .box {
    flex: none;
    width: 12px;
    height: 12px;
    border: 1.5px solid var(--w-edge);
    border-radius: var(--w-r-sm);
  }
  details {
    display: grid;
    gap: var(--w-s-2);
  }
  details[open] > summary {
    margin-bottom: var(--w-s-2);
  }
  summary {
    cursor: pointer;
    color: var(--w-muted);
  }
  details > input {
    width: 100%;
  }
  .hint {
    margin: 0;
    text-align: right;
  }
  .w-btn--primary:disabled {
    opacity: 0.5;
    cursor: default;
  }
</style>
