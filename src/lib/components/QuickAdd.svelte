<script lang="ts">
  import { tick } from "svelte";
  import { grow } from "$lib/grow";
  import { workspace, createTask, inDailyWork, agentsEnabled } from "$lib/stores/workspace.svelte";
  import { agentList } from "$lib/tasks.svelte";

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
  let due = $state("");
  // svelte-ignore state_referenced_locally
  let parent = $state(initialParent ?? "");
  let tags = $state("");
  let description = $state("");
  let criteria = $state<string[]>([""]);
  // Index into the Agent menu; "" = the default agent (no fields written).
  let agent = $state("");
  let busy = $state(false);
  // `autofocus` does not fire when the layout mounts this later.
  let input: HTMLInputElement;
  $effect(() => input.focus());

  async function save(e?: SubmitEvent) {
    e?.preventDefault();
    if (!title.trim() || busy) return;
    busy = true;
    const pick = agent === "" ? undefined : agentList()[+agent];
    const id = await createTask({
      title: title.trim(),
      project: project || null,
      priority,
      due: due || null,
      parent: parent.trim() || null,
      tags: tags.split(/[,\s]+/).map((x) => x.replace(/^#/, "")).filter(Boolean),
      description,
      acceptance: criteria.map((c) => c.trim()).filter(Boolean),
      runner: pick?.runner ?? null,
      model: pick?.model ?? null,
      effort: pick?.effort ?? null,
      permissions: pick?.permissions ?? null,
    });
    busy = false;
    if (id) onclose(id);
  }

  // ⌘↵ saves from any field (Enter in the title too; in the criteria it adds a line).
  function onFormKey(e: KeyboardEvent) {
    if (e.key === "Enter" && e.metaKey) {
      e.preventDefault();
      save();
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
    <div class="head">
      <h2 class="meta" id="qa-title">{initialParent ? `New sub-task of ${initialParent}` : "New task"}</h2>
      <input bind:this={input} bind:value={title} class="title" placeholder="What needs doing?" aria-label="Title" />
    </div>

    <div class="grid">
      <label for="qa-desc" class="top">Description</label>
      <textarea id="qa-desc" class="field" bind:value={description} placeholder="What and why, in Markdown" use:grow></textarea>

      <span class="label top" id="qa-acc">Acceptance</span>
      <div class="crit" role="group" aria-labelledby="qa-acc">
        {#each criteria as _, i (i)}
          <label class="field crit-row">
            <span class="box" aria-hidden="true"></span>
            <input
              bind:this={rows[i]}
              bind:value={criteria[i]}
              placeholder={i === 0 ? "Done when … (Enter adds the next)" : "Next criterion"}
              aria-label="Acceptance criterion {i + 1}"
              onkeydown={(e) => onCriterionKey(e, i)}
            />
          </label>
        {/each}
      </div>

      <label for="qa-project">Project</label>
      <span class="select">
        <select id="qa-project" bind:value={project}>
          <option value="">Inbox</option>
          {#each projects as p (p.key)}<option value={p.key}>{p.title}</option>{/each}
        </select>
      </span>

      <span class="label" id="qa-prio">Priority</span>
      <div class="w-seg prio" role="group" aria-labelledby="qa-prio">
        {#each [1, 2, 3] as p (p)}
          <button type="button" aria-pressed={priority === p} onclick={() => (priority = p)}>P{p}</button>
        {/each}
      </div>

      <label for="qa-due">Due</label>
      <input id="qa-due" class="field" class:empty={!due} type="date" bind:value={due} />

      <label for="qa-parent">Parent</label>
      <input id="qa-parent" class="field" list="qa-parents" bind:value={parent} placeholder="None – or a task id like WR-12" />
      <datalist id="qa-parents">
        {#each parents as t (t.id)}<option value={t.id}>{t.title}</option>{/each}
      </datalist>

      <label for="qa-tags">Tags</label>
      <input id="qa-tags" class="field" bind:value={tags} placeholder="frontend, ux" />
    </div>

    {#if agentsEnabled()}
      <details>
        <summary>Agent settings</summary>
        <div class="grid">
          <label for="qa-agent">Agent</label>
          <span class="select">
            <select id="qa-agent" bind:value={agent}>
              <option value="">Default{agentList()[0] ? ` · ${agentList()[0].name}` : ""}</option>
              {#each agentList() as a, i (a.name)}<option value={String(i)}>{a.name}</option>{/each}
            </select>
          </span>
        </div>
      </details>
    {/if}

    <div class="foot">
      <kbd class="w-sub">⌘↵</kbd>
      <button type="submit" class="w-btn w-btn--primary" disabled={!title.trim() || busy}>Add task</button>
    </div>
  </form>
</div>

<style>
  /* Centred like the other modals (they translate by -50%); scrolls when taller than the window. */
  .w-modal {
    width: min(600px, calc(100% - 32px));
    max-height: calc(100% - 48px);
    overflow-y: auto;
    /* One box for every control: height, label column. */
    --qa-h: 32px;
    --qa-label: 92px;
  }
  form {
    display: flex;
    flex-direction: column;
    gap: var(--w-s-4);
    padding: var(--w-s-5) var(--w-s-6);
  }
  /* Same head as the detail card: meta line, then the title as the one large element. */
  .head {
    display: grid;
    gap: 6px;
    padding-bottom: var(--w-s-4);
    border-bottom: 1px solid var(--w-line);
  }
  .meta {
    margin: 0;
    font-size: var(--w-fs-small);
    font-weight: 400;
    color: var(--w-muted);
  }
  .title {
    margin: 0 -6px;
    padding: 2px 6px;
    border: 0;
    border-radius: var(--w-r-sm);
    background: none;
    font-family: var(--w-font-label);
    font-size: var(--w-fs-headline);
    font-weight: 600;
    line-height: 1.2;
  }
  .title::placeholder {
    color: var(--w-muted);
    font-weight: 500;
  }
  .title:focus {
    outline: none;
    background: var(--w-sunk);
    box-shadow: 0 0 0 2px var(--w-accent-soft);
  }
  .grid {
    display: grid;
    grid-template-columns: var(--qa-label) minmax(0, 1fr);
    gap: var(--w-s-2) var(--w-s-3);
    align-items: center;
  }
  .grid > label,
  .label {
    font-size: var(--w-fs-small);
    color: var(--w-muted);
  }
  /* Multi-line controls: the label sits on the first line. */
  .top {
    align-self: start;
    line-height: var(--qa-h);
  }
  .field,
  .select select {
    box-sizing: border-box;
    width: 100%;
    min-width: 0;
    height: var(--qa-h);
    margin: 0;
    border: 0;
    border-radius: var(--w-r-md);
    padding: 0 10px;
    background: var(--w-tray);
    font-size: var(--w-fs-small);
    line-height: var(--qa-h);
    outline: none;
  }
  .field::placeholder {
    color: var(--w-muted);
  }
  .field:focus,
  .field:focus-within,
  .select select:focus {
    box-shadow: 0 0 0 2px var(--w-accent-soft);
  }
  textarea.field {
    height: auto;
    min-height: calc(var(--qa-h) * 2.5);
    padding: 7px 10px;
    line-height: 1.45;
    resize: none;
    overflow: hidden;
  }
  /* Native select without its own height and arrow; the chevron is drawn in currentColor. */
  .select {
    position: relative;
    display: block;
    min-width: 0;
  }
  .select select {
    appearance: none;
    -webkit-appearance: none;
    padding-right: 28px;
    cursor: pointer;
  }
  .select::after {
    content: "";
    position: absolute;
    right: 12px;
    top: 50%;
    width: 6px;
    height: 6px;
    border: solid var(--w-muted);
    border-width: 0 1.5px 1.5px 0;
    transform: translateY(-70%) rotate(45deg);
    pointer-events: none;
  }
  /* WebKit draws empty date parts in its own colour; keep them in the field's colours. */
  input[type="date"]::-webkit-datetime-edit,
  input[type="date"]::-webkit-datetime-edit-fields-wrapper,
  input[type="date"]::-webkit-datetime-edit-text,
  input[type="date"]::-webkit-datetime-edit-day-field,
  input[type="date"]::-webkit-datetime-edit-month-field,
  input[type="date"]::-webkit-datetime-edit-year-field {
    color: inherit;
    padding: 0;
  }
  input[type="date"].empty {
    color: var(--w-muted);
  }
  .prio {
    justify-self: start;
    box-sizing: border-box;
    height: var(--qa-h);
  }
  .prio button {
    padding: 0 14px;
  }
  .crit {
    display: grid;
    gap: 6px;
  }
  .crit-row {
    display: flex;
    align-items: center;
    gap: var(--w-s-2);
    cursor: text;
  }
  .crit-row input {
    flex: 1;
    min-width: 0;
    height: 100%;
    border: 0;
    padding: 0;
    background: none;
    font-size: var(--w-fs-small);
    outline: none;
  }
  .crit-row input::placeholder {
    color: var(--w-muted);
  }
  .box {
    flex: none;
    width: 12px;
    height: 12px;
    box-sizing: border-box;
    border: 1.5px solid var(--w-edge);
    border-radius: var(--w-r-xs);
  }
  details > .grid {
    margin-top: var(--w-s-2);
  }
  summary {
    width: max-content;
    font-size: var(--w-fs-small);
    color: var(--w-muted);
    cursor: pointer;
  }
  summary:hover {
    color: var(--w-ink);
  }
  .foot {
    display: flex;
    align-items: center;
    justify-content: flex-end;
    gap: var(--w-s-3);
    padding-top: var(--w-s-4);
    border-top: 1px solid var(--w-line);
  }
  kbd {
    font: inherit;
  }
  .w-btn--primary:disabled {
    opacity: 0.5;
    cursor: default;
  }
</style>
