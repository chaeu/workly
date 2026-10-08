<script lang="ts">
  import { grow } from "$lib/grow";
  import { tip } from "$lib/tip";
  import { renderMarkdown } from "$lib/markdown";
  import {
    workspace,
    agentsEnabled,
    projColor,
    readMarkdown,
    updateTaskField,
    addTaskUpdate,
    setDescription,
    moveTask,
    setFocus,
    openUrl,
    openInObsidian,
    openInVscode,
  } from "$lib/stores/workspace.svelte";
  import { agentName, confirmDeleteTask, projectOf, rawDescription, shortDate, splitBody, today } from "$lib/tasks.svelte";

  let { id, mode, focusIds, onclose }: { id: string; mode: "popup" | "panel"; focusIds: string[]; onclose: () => void } = $props();

  const RUNNERS = ["auto", "codex", "copilot", "claude-code"];
  const EFFORTS = ["auto", "low", "medium", "high"];

  const t = $derived(workspace.index?.tasks.find((x) => x.id === id));
  const project = $derived(t && projectOf(t));
  const projects = $derived(workspace.index?.projects.filter((p) => p.status !== "archived" || p.key === project?.key) ?? []);
  const statuses = $derived(workspace.index?.config.task_statuses ?? []);
  const focusAt = $derived(focusIds.indexOf(id));

  // Gone (deleted or broken in another editor): nothing left to show.
  $effect(() => {
    if (workspace.index && !t) onclose();
  });

  // Body from the file; re-read on every index change so external edits show.
  let body = $state("");
  $effect(() => {
    const path = t?.path;
    void workspace.index;
    if (path) readMarkdown(path).then((b) => (body = b), (e) => (workspace.error = String(e)));
  });
  const parts = $derived(splitBody(body));
  const html = $derived(renderMarkdown(parts.description));

  /** Write one field; a refused write puts the file's value back into the input. */
  async function save(field: string, value: unknown, input?: HTMLInputElement, shown = "") {
    const ok = await updateTaskField(id, field, value);
    if (!ok && input) input.value = shown;
  }

  const tagsText = $derived(t?.tags.join(", ") ?? "");
  const parseTags = (s: string) =>
    s
      .split(/[,\s]+/)
      .map((x) => x.replace(/^#/, ""))
      .filter(Boolean);

  let note = $state("");
  async function saveNote() {
    if (!note.trim()) return;
    await addTaskUpdate(id, note);
    if (!workspace.error) note = "";
  }

  async function remove() {
    if (t && (await confirmDeleteTask(t))) onclose();
  }

  /** Move the file; the select then shows whatever the index says, also after a refused move. */
  async function changeProject(e: Event & { currentTarget: HTMLSelectElement }) {
    const select = e.currentTarget;
    await moveTask(id, select.value || null);
    select.value = project?.key ?? "";
  }

  const focusFull = $derived(focusAt < 0 && focusIds.length >= 3);
  const toggleFocus = () => !focusFull && setFocus(focusAt >= 0 ? focusIds.filter((x) => x !== id) : [...focusIds, id]);

  // Links in the description open outside; they never navigate the app window.
  // Any other click (not ending a text selection) edits the description.
  function onDescClick(e: MouseEvent) {
    const a = (e.target as HTMLElement).closest("a");
    if (!a) {
      if (!getSelection()?.toString()) editDesc();
      return;
    }
    e.preventDefault();
    const href = a.getAttribute("href") ?? "";
    if (/^(https?|mailto):/i.test(href)) openUrl(href);
  }

  // Description edit: raw Markdown in a textarea. `expected` is the file's
  // description when editing started; core refuses the save if it changed since.
  let draft = $state<string | null>(null);
  let expected = "";
  let conflict = $state(false);
  let saving = false;
  function editDesc() {
    expected = rawDescription(body);
    draft = expected.replace(/\r\n/g, "\n");
    conflict = false;
  }
  function cancelDesc() {
    draft = null;
    conflict = false;
  }
  async function saveDesc() {
    if (draft === null || conflict || saving) return;
    if (draft.trimEnd() === expected.replace(/\r\n/g, "\n")) return cancelDesc();
    saving = true;
    const ok = await setDescription(id, draft, expected);
    saving = false;
    if (ok) draft = null;
    else if (ok === false) conflict = true;
  }
  function onDescKey(e: KeyboardEvent) {
    if (e.key === "Escape") {
      // Cancel the edit only; the card stays open.
      e.preventDefault();
      e.stopPropagation();
      cancelDesc();
    } else if (e.key === "Enter" && e.metaKey) {
      e.preventDefault();
      saveDesc();
    }
  }
  /** Caret at the end, where most edits add text. */
  function focus(el: HTMLTextAreaElement) {
    el.focus();
    el.setSelectionRange(el.value.length, el.value.length);
  }

  const stamp = (iso: string | null) =>
    iso ? new Date(iso).toLocaleString("en-GB", { day: "numeric", month: "short", hour: "2-digit", minute: "2-digit" }) : "–";
</script>

{#if t}
  <!-- Side panel: a clear scrim, so a click outside only closes it and never opens what lies beneath. -->
  <div class="w-scrim" class:w-scrim--clear={mode === "panel"} onclick={onclose} aria-hidden="true"></div>
  <div
    class={mode === "popup" ? "w-modal" : "panel"}
    role="dialog"
    aria-modal={mode === "popup"}
    aria-labelledby="t-title"
  >
    <div class="t-head">
      <div class="t-meta">
        <span class="w-mono">{t.id}</span>
        <span class="w-proj-mark" style:--c={project ? projColor(project.color) : "var(--w-line)"}></span>
        {#key t.path}
          <select class="project" value={project?.key ?? ""} aria-label="Project" onchange={changeProject}>
            <option value="">Inbox</option>
            {#each projects as p (p.key)}<option value={p.key}>{p.title}</option>{/each}
          </select>
        {/key}
        <!-- aria-disabled, not disabled: WebKit shows no tooltip on a disabled button. -->
        <button
          type="button"
          class="t-star"
          class:on={focusAt >= 0}
          aria-pressed={focusAt >= 0}
          aria-disabled={focusFull}
          aria-label={focusAt >= 0 ? `Focus #${focusAt + 1}, remove from focus` : "Add to today's focus"}
          use:tip={focusAt >= 0 ? `Focus #${focusAt + 1}` : focusFull ? "The focus strip already has three tasks" : "Add to today's focus"}
          onclick={toggleFocus}
          ><svg width="16" height="16" viewBox="0 0 24 24" fill={focusAt >= 0 ? "currentColor" : "none"} stroke="currentColor" stroke-width="1.8" stroke-linejoin="round" aria-hidden="true"
            ><path d="m12 3 2.8 5.7 6.2.9-4.5 4.4 1 6.2L12 17.3 6.5 20.2l1-6.2L3 9.6l6.2-.9z" /></svg
          ></button
        >
        <button type="button" class="t-x" onclick={onclose} aria-label="Close">✕</button>
      </div>
      {#key t.title}
        <input
          id="t-title"
          class="t-title"
          value={t.title}
          aria-label="Title"
          onchange={(e) => save("title", e.currentTarget.value.trim(), e.currentTarget, t.title)}
        />
      {/key}
      <div class="t-status" role="group" aria-label="Status">
        {#each statuses as s (s.id)}
          <button type="button" aria-pressed={t.status === s.id} onclick={() => save("status", s.id)}>{s.label}</button>
        {/each}
      </div>
    </div>

    <div class="t-body">
      <div class="t-desc">
        {#if draft !== null}
          <textarea
            class="t-desc-edit"
            bind:value={draft}
            aria-label="Description (Markdown)"
            placeholder="Description and acceptance criteria"
            use:grow
            use:focus
            onblur={saveDesc}
            onkeydown={onDescKey}
          ></textarea>
          {#if conflict}
            <div class="t-conflict" role="alert">
              Changed outside – reload?
              <button type="button" class="w-btn w-btn--quiet" onclick={cancelDesc}>Reload</button>
            </div>
          {/if}
        {:else if parts.description.trim()}
          <!-- Sanitised by DOMPurify in renderMarkdown. -->
          <!-- svelte-ignore a11y_no_noninteractive_tabindex, a11y_no_noninteractive_element_interactions -->
          <div
            class="md t-desc-view"
            role="group"
            aria-label="Description, click or press Enter to edit"
            tabindex="0"
            title="Click to edit"
            onclick={onDescClick}
            onkeydown={(e) => e.key === "Enter" && e.target === e.currentTarget && editDesc()}
          >
            {@html html}
          </div>
        {:else}
          <button type="button" class="t-desc-add" onclick={editDesc}>Add description</button>
        {/if}
      </div>

      <div class="t-side">
        <dl class="t-kv">
          <dt>Priority</dt>
          <dd>
            <div class="w-seg" role="group" aria-label="Priority">
              {#each [null, 1, 2, 3] as p (p)}
                <button type="button" aria-pressed={t.priority === p} onclick={() => save("priority", p)}>{p ? `P${p}` : "–"}</button>
              {/each}
            </div>
          </dd>
          <dt>Due</dt>
          <dd>
            {#key t.due}
              <input
                type="date"
                class="field"
                value={t.due ?? ""}
                aria-label="Due date"
                onchange={(e) => save("due", e.currentTarget.value || null, e.currentTarget, t.due ?? "")}
              />
            {/key}
          </dd>
          <dt>Tags</dt>
          <dd>
            {#key tagsText}
              <input
                class="field w-mono"
                value={tagsText}
                placeholder="frontend, ux"
                aria-label="Tags, comma separated"
                onchange={(e) => save("tags", parseTags(e.currentTarget.value), e.currentTarget, tagsText)}
              />
            {/key}
          </dd>
          {#if t.status === "done" && t.done_at}
            <dt>Done</dt>
            <dd class="w-mono">{shortDate(t.done_at)}</dd>
          {/if}
        </dl>

        {#if agentsEnabled()}
          <div class="t-group">
            <span class="w-caps">Agent</span>
            <dl class="t-kv">
              <dt>Ready</dt>
              <dd>
                <input type="checkbox" checked={t.agent?.ready ?? false} aria-label="Ready for an agent" onchange={(e) => save("agent.ready", e.currentTarget.checked)} />
              </dd>
              <dt>Runner</dt>
              <dd>
                <select class="field" value={t.agent?.runner ?? "auto"} aria-label="Runner" onchange={(e) => save("agent.runner", e.currentTarget.value)}>
                  {#each RUNNERS as r (r)}<option value={r}>{r === "auto" ? "auto" : agentName(r)}</option>{/each}
                </select>
              </dd>
              <dt>Model</dt>
              <dd>
                {#key t.agent?.model}
                  <input
                    class="field"
                    value={t.agent?.model ?? "auto"}
                    aria-label="Model"
                    onchange={(e) => save("agent.model", e.currentTarget.value.trim() || "auto", e.currentTarget, t.agent?.model ?? "auto")}
                  />
                {/key}
              </dd>
              <dt>Effort</dt>
              <dd>
                <select class="field" value={t.agent?.effort ?? "auto"} aria-label="Effort" onchange={(e) => save("agent.effort", e.currentTarget.value)}>
                  {#each EFFORTS as x (x)}<option value={x}>{x}</option>{/each}
                </select>
              </dd>
              <dt>Active</dt>
              <dd>{t.agent?.active ? agentName(t.agent.active) : "–"}</dd>
              <dt>Since</dt>
              <dd class="w-mono">{stamp(t.agent?.since ?? null)}</dd>
              <dt>Commit</dt>
              <dd class="w-mono">{t.agent?.commit ?? "–"}</dd>
            </dl>
          </div>
        {/if}

        <div class="t-group">
          <span class="w-caps">Updates · {parts.updates.length}</span>
          {#each parts.updates as u, i (i)}
            <div class="t-update"><span class="w-mono">{u.meta}</span>{u.text}</div>
          {/each}
          <textarea
            class="t-compose"
            bind:value={note}
            aria-label="Add an update"
            placeholder="Update, decision or closing note"
            onkeydown={(e) => e.key === "Enter" && e.metaKey && saveNote()}
          ></textarea>
        </div>
      </div>
    </div>

    <div class="t-foot">
      <button type="button" class="w-btn w-btn--quiet danger" onclick={remove}>Delete…</button>
      <button type="button" class="w-btn w-btn--quiet" onclick={() => openInObsidian(t.path)}>Obsidian</button>
      <button type="button" class="w-btn w-btn--quiet" onclick={() => openInVscode(t.path)}>VS Code</button>
      <button type="button" class="w-btn w-btn--primary" disabled={!note.trim()} onclick={saveNote}>Save update</button>
    </div>
  </div>
{/if}

<style>
  .panel {
    position: fixed;
    top: 0;
    right: 0;
    bottom: 0;
    z-index: 40;
    width: min(var(--w-panel-w), 100%);
    display: flex;
    flex-direction: column;
    background: var(--w-surface);
    box-shadow: var(--w-shadow-pop);
    color: var(--w-ink);
    animation: slide var(--w-dur) var(--w-ease);
  }
  .w-modal {
    width: min(var(--w-detail-w), calc(100% - 32px));
    min-height: min(var(--w-detail-min-h), calc(100% - 48px));
  }
  @keyframes slide {
    from {
      transform: translateX(16px);
      opacity: 0;
    }
  }
  /* Room for the overlay traffic lights / drag region. */
  .panel .t-head {
    padding-top: 36px;
  }
  .t-head {
    padding: 16px 22px 14px;
    display: grid;
    gap: 10px;
    border-bottom: 1px solid var(--w-line);
  }
  .t-meta {
    display: flex;
    align-items: center;
    gap: var(--w-s-2);
    color: var(--w-muted);
  }
  .project {
    border: 0;
    border-radius: var(--w-r-sm);
    background: none;
    color: var(--w-muted);
    font-size: var(--w-fs-small);
    padding: 2px 4px;
    margin-left: -4px;
    cursor: pointer;
  }
  .project:hover {
    background: var(--w-tray);
    color: var(--w-ink);
  }
  .t-star {
    margin-left: auto;
    display: grid;
    place-items: center;
    width: 30px;
    height: 30px;
    border: 0;
    border-radius: var(--w-r-sm);
    background: none;
    color: var(--w-muted);
    cursor: pointer;
  }
  .t-star:hover {
    background: var(--w-tray);
    color: var(--w-ink);
  }
  .t-star.on {
    color: var(--w-accent);
  }
  .t-star[aria-disabled="true"] {
    opacity: 0.4;
    cursor: default;
  }
  .t-star[aria-disabled="true"]:hover {
    background: none;
    color: var(--w-muted);
  }
  .t-x {
    width: 30px;
    height: 30px;
    border: 0;
    border-radius: var(--w-r-sm);
    background: none;
    color: var(--w-muted);
    cursor: pointer;
  }
  .t-x:hover {
    background: var(--w-tray);
    color: var(--w-ink);
  }
  .t-title {
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
  .t-title:hover {
    background: var(--w-sunk);
  }
  .t-title:focus {
    outline: none;
    background: var(--w-sunk);
    box-shadow: 0 0 0 2px var(--w-accent-soft);
  }
  .t-status {
    display: flex;
    flex-wrap: wrap;
    gap: 6px;
  }
  .t-status button {
    border: 0;
    border-radius: var(--w-r-pill);
    padding: 3px 11px;
    font-size: var(--w-fs-small);
    cursor: pointer;
    background: var(--w-tray);
    color: var(--w-muted);
  }
  .t-status button[aria-pressed="true"] {
    background: var(--w-ink);
    color: var(--w-bg);
    font-weight: 600;
  }
  .t-body {
    flex: 1;
    overflow-y: auto;
    padding: 18px 22px 20px;
    display: grid;
    grid-template-columns: minmax(0, 1.6fr) minmax(0, 1fr);
    gap: 18px 28px;
  }
  .panel .t-body {
    grid-template-columns: minmax(0, 1fr);
    align-content: start;
  }
  .t-desc {
    display: flex;
    flex-direction: column;
    gap: var(--w-s-3);
    min-width: 0;
  }
  .t-desc .md :global(:first-child) {
    margin-top: 0;
  }
  .t-desc-view {
    margin: -4px -8px;
    padding: 4px 8px;
    border-radius: var(--w-r-sm);
    cursor: text;
  }
  .t-desc-view:hover {
    background: var(--w-sunk);
  }
  .t-desc-view:focus-visible {
    outline: none;
    box-shadow: 0 0 0 2px var(--w-accent-soft);
  }
  .t-desc-add {
    align-self: flex-start;
    margin-left: -8px;
    border: 0;
    border-radius: var(--w-r-sm);
    padding: 4px 8px;
    background: none;
    color: var(--w-muted);
    font-size: var(--w-fs-body);
    cursor: text;
  }
  .t-desc-add:hover {
    background: var(--w-sunk);
    color: var(--w-ink);
  }
  .t-desc-edit {
    width: calc(100% + 16px);
    box-sizing: border-box;
    margin: -4px -8px;
    min-height: 120px;
    border: 0;
    border-radius: var(--w-r-sm);
    padding: 4px 8px;
    background: var(--w-sunk);
    box-shadow: 0 0 0 2px var(--w-accent-soft);
    resize: none;
    overflow: hidden;
    font-size: var(--w-fs-body);
    line-height: 1.5;
    user-select: text;
    -webkit-user-select: text;
  }
  .t-desc-edit:focus {
    outline: none;
  }
  .t-conflict {
    display: flex;
    align-items: center;
    gap: var(--w-s-2);
    margin-top: var(--w-s-1);
    color: var(--w-warn);
    font-size: var(--w-fs-small);
  }
  .t-side {
    display: grid;
    gap: var(--w-s-4);
    align-content: start;
    border-left: 1px solid var(--w-line);
    padding-left: 24px;
  }
  .panel .t-side {
    border-left: 0;
    padding-left: 0;
    border-top: 1px solid var(--w-line);
    padding-top: var(--w-s-4);
  }
  .t-group {
    display: grid;
    gap: var(--w-s-2);
  }
  .t-kv {
    display: grid;
    grid-template-columns: auto 1fr;
    gap: 6px 12px;
    margin: 0;
    font-size: var(--w-fs-small);
    align-items: center;
  }
  .t-kv dt {
    color: var(--w-muted);
  }
  .t-kv dd {
    margin: 0;
    display: flex;
    align-items: center;
    gap: 7px;
    min-width: 0;
    min-height: 28px;
  }
  .t-kv .w-seg button {
    padding: 2px 9px;
  }
  .field {
    width: 100%;
    min-width: 0;
    box-sizing: border-box;
    border: 0;
    border-radius: var(--w-r-sm);
    background: var(--w-tray);
    padding: 4px 8px;
    font-size: var(--w-fs-small);
  }
  .field.w-mono {
    font-size: var(--w-fs-micro);
  }
  .field:focus {
    outline: none;
    box-shadow: 0 0 0 2px var(--w-accent-soft);
  }
  .t-update {
    background: var(--w-sunk);
    border-radius: var(--w-r-md);
    padding: 8px 10px;
    font-size: var(--w-fs-small);
    line-height: 1.45;
    user-select: text;
    -webkit-user-select: text;
  }
  .t-update .w-mono {
    display: block;
    color: var(--w-muted);
    margin-bottom: 2px;
  }
  .t-compose {
    width: 100%;
    box-sizing: border-box;
    border: 0;
    border-radius: var(--w-r-md);
    background: var(--w-tray);
    padding: 8px 10px;
    resize: vertical;
    min-height: 60px;
    font-size: var(--w-fs-small);
  }
  .t-compose:focus {
    outline: none;
    box-shadow: 0 0 0 2px var(--w-accent-soft);
  }
  .t-foot {
    display: flex;
    align-items: center;
    gap: var(--w-s-2);
    padding: 12px 22px;
    background: var(--w-sunk);
    border-top: 1px solid var(--w-line);
  }
  .t-foot .w-btn--primary {
    margin-left: auto;
  }
  .w-btn:disabled {
    opacity: 0.5;
    cursor: default;
  }
  .danger:hover {
    color: var(--w-danger);
    background: var(--w-danger-soft);
  }
  @media (max-width: 680px) {
    .t-body {
      grid-template-columns: 1fr;
    }
    .t-side {
      border-left: 0;
      padding-left: 0;
    }
  }
  @media (prefers-reduced-motion: reduce) {
    .panel {
      animation: none;
    }
  }
</style>
