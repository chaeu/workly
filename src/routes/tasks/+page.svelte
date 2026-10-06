<script lang="ts">
  import TaskCard from "$lib/components/TaskCard.svelte";
  import TaskDetail from "$lib/components/TaskDetail.svelte";
  import QuickAdd from "$lib/components/QuickAdd.svelte";
  import {
    workspace,
    projColor,
    saveSettings,
    updateTaskField,
    moveTask,
    reorderTasks,
    setFocus,
    type TaskEntry,
  } from "$lib/stores/workspace.svelte";
  import { byBoardOrder, endOfWeek, isOverdue, projectOf, shortDate, startClock, today } from "$lib/tasks.svelte";

  startClock();

  const statuses = $derived(workspace.index?.config.task_statuses ?? []);
  const projects = $derived(workspace.index?.projects.filter((p) => p.status !== "archived") ?? []);
  // Tasks of archived projects stay off the board.
  const tasks = $derived.by(() => {
    const live = new Set(projects.map((p) => p.path));
    return (workspace.index?.tasks ?? []).filter((t) => !t.project || live.has(t.project)).sort(byBoardOrder);
  });
  const lanes = $derived([
    ...projects.map((p) => ({ key: p.key, title: p.title, color: projColor(p.color) })),
    { key: "", title: "Inbox", color: "var(--w-line)" },
  ]);
  const laneOf = (t: TaskEntry) => projectOf(t)?.key ?? "";

  let group = $state<"status" | "project">("status");
  let openId = $state<string | null>(null);
  let adding = $state(false);
  const detailMode = $derived(workspace.settings?.task_detail ?? "popup");

  // ------------------------------------------------------------- filters

  let query = $state("");
  let fProject = $state("");
  let fPrio = $state<number | null>(null);
  let fTag = $state("");
  let fDue = $state<"" | "overdue" | "week">("");
  let searchEl: HTMLInputElement;
  const allTags = $derived([...new Set(tasks.flatMap((t) => t.tags))].sort());
  const filtering = $derived(!!(query.trim() || fProject || fPrio || fTag || fDue));

  function matches(t: TaskEntry) {
    const q = query.trim().toLowerCase();
    if (q && !`${t.id} ${t.title}`.toLowerCase().includes(q)) return false;
    if (fProject && (fProject === "inbox" ? laneOf(t) !== "" : laneOf(t) !== fProject)) return false;
    if (fPrio && t.priority !== fPrio) return false;
    if (fTag && !t.tags.includes(fTag)) return false;
    if (fDue === "overdue" && !isOverdue(t)) return false;
    if (fDue === "week" && !(t.due && t.status !== "done" && t.due.slice(0, 10) >= today() && t.due.slice(0, 10) <= endOfWeek())) return false;
    return true;
  }
  const matchCount = $derived(tasks.filter(matches).length);
  const clearFilters = () => ([query, fProject, fPrio, fTag, fDue] = ["", "", null, "", ""]);

  // --------------------------------------------------------------- focus

  const focus = $derived(
    tasks.filter((t) => t.focus?.slice(0, 10) === today()).sort((a, b) => (a.focus_order ?? 9) - (b.focus_order ?? 9)),
  );
  const focusIds = $derived(focus.map((t) => t.id));
  const focusHidden = $derived(workspace.settings?.focus_hidden ?? false);
  const plural = (n: number, word: string) => `${n} ${word}${n === 1 ? "" : "s"}`;

  // --------------------------------------------------------------- board

  /** Drop target key: `<status>|<lane>`; lane `*` = any project (status view), `` = inbox. */
  const cellKey = (status: string, lane = "*") => `${status}|${lane}`;
  const keyOf = (t: TaskEntry) => cellKey(t.status, group === "project" ? laneOf(t) : "*");
  function cellTasks(key: string) {
    const [status, lane] = key.split("|");
    return tasks.filter((t) => t.status === status && (lane === "*" || laneOf(t) === lane));
  }
  function countLabel(s: { id: string; wip_limit: number | null }) {
    const n = tasks.filter((t) => t.status === s.id).length;
    return s.wip_limit ? `${n} / ${s.wip_limit}` : `${n}`;
  }
  const overLimit = (s: { id: string; wip_limit: number | null }) => !!s.wip_limit && tasks.filter((t) => t.status === s.id).length > s.wip_limit;
  const openCount = $derived(tasks.filter((t) => t.status !== "done").length);
  const subline = $derived(
    [`${openCount} open`, ...statuses.filter((s) => s.id === "doing" || s.id === "review").map((s) => `${cellTasks(cellKey(s.id)).length} ${s.label.toLowerCase()}`)].join(" · "),
  );

  // --------------------------------------------------------- drag & drop
  // Pointer events like the reference: 5 px threshold, a click opens the card.
  // Nothing moves until the file changed: the board re-renders from the index.

  let pending: { id: string; el: HTMLElement; sx: number; sy: number; fromFocus: boolean } | null = null;
  let ghost: HTMLElement | null = null;
  let offset = { x: 0, y: 0 };
  let dragId = $state<string | null>(null);
  let dragFromFocus = $state(false);
  let over = $state<{ key: string; index: number } | null>(null);

  // Reordering inside the source cell shows an insertion line; other targets light up.
  const sameCell = $derived.by(() => {
    const t = dragId ? tasks.find((x) => x.id === dragId) : undefined;
    return !!t && !!over && over.key === keyOf(t);
  });
  const lineBefore = $derived.by(() => {
    if (!sameCell || !over) return null;
    return cellTasks(over.key).filter((t) => t.id !== dragId)[over.index]?.id ?? "end";
  });
  const isOver = (key: string) => over?.key === key && !sameCell && !(key === "focus" && dragFromFocus);

  function onpointerdown(e: PointerEvent) {
    const target = e.target as HTMLElement;
    const el = target.closest<HTMLElement>("[data-id]");
    if (e.button !== 0 || !el || target.closest("button")) return;
    pending = { id: el.dataset.id!, el, sx: e.clientX, sy: e.clientY, fromFocus: !!el.closest(".w-focus") };
  }

  function insertAt(target: HTMLElement, e: PointerEvent) {
    const across = target.dataset.drop === "focus";
    const items = [...target.querySelectorAll<HTMLElement>("[data-id]")].filter((x) => x.dataset.id !== dragId);
    const i = items.findIndex((x) => {
      const r = x.getBoundingClientRect();
      return across ? e.clientX < r.left + r.width / 2 : e.clientY < r.top + r.height / 2;
    });
    return i < 0 ? items.length : i;
  }

  function onpointermove(e: PointerEvent) {
    if (!pending) return;
    if (!ghost) {
      if (Math.hypot(e.clientX - pending.sx, e.clientY - pending.sy) < 5) return;
      const r = pending.el.getBoundingClientRect();
      offset = { x: pending.sx - r.left, y: pending.sy - r.top };
      ghost = pending.el.cloneNode(true) as HTMLElement;
      ghost.classList.add("w-drag-ghost");
      ghost.style.width = `${r.width}px`;
      document.body.appendChild(ghost);
      dragId = pending.id;
      dragFromFocus = pending.fromFocus;
    }
    ghost.style.transform = `translate(${e.clientX - offset.x}px, ${e.clientY - offset.y}px)`;
    const target = document.elementFromPoint(e.clientX, e.clientY)?.closest<HTMLElement>("[data-drop]");
    // Focus items only reorder inside the strip.
    if (!target || (dragFromFocus && target.dataset.drop !== "focus")) over = null;
    else over = { key: target.dataset.drop!, index: insertAt(target, e) };
  }

  function onpointerup() {
    const p = pending;
    pending = null;
    if (!p) return;
    if (!ghost) {
      openId = p.id;
      return;
    }
    const o = over;
    endDrag();
    if (o) drop(p.id, o.key, o.index);
  }

  function endDrag() {
    pending = null;
    ghost?.remove();
    ghost = null;
    dragId = null;
    over = null;
  }

  async function drop(id: string, key: string, index: number) {
    const t = tasks.find((x) => x.id === id);
    if (!t) return;
    if (key === "focus") {
      const ids = focusIds.filter((x) => x !== id);
      if (ids.length >= 3) {
        workspace.error = "The focus strip holds three tasks. Remove one first.";
        return;
      }
      ids.splice(index, 0, id);
      if (ids.join() !== focusIds.join()) await setFocus(ids);
      return;
    }
    if (key === keyOf(t)) {
      const ids = cellTasks(key).map((x) => x.id).filter((x) => x !== id);
      ids.splice(index, 0, id);
      if (ids.join() !== cellTasks(key).map((x) => x.id).join()) await reorderTasks(ids);
      return;
    }
    // Another column changes only the status; another lane moves the file first.
    const [status, lane] = key.split("|");
    if (lane !== "*" && lane !== laneOf(t)) await moveTask(id, lane || null);
    if (status !== t.status) await updateTaskField(id, "status", status);
  }

  // ------------------------------------------------------------ keyboard

  function onkeydown(e: KeyboardEvent) {
    const meta = e.metaKey && !e.shiftKey && !e.altKey && !e.ctrlKey;
    if (meta && e.key === "n") {
      e.preventDefault();
      adding = true;
    } else if (meta && e.key === "k") {
      e.preventDefault();
      searchEl.focus();
      searchEl.select();
    } else if (e.key === "Escape") {
      if (ghost) endDrag();
      else if (adding) adding = false;
      else if (openId) openId = null;
      else if (document.activeElement === searchEl) query = "";
    } else if (e.key === "Enter" || e.key === " ") {
      const el = (e.target as HTMLElement).closest?.<HTMLElement>(".w-card, .w-focus-item");
      if (el?.dataset.id && e.target === el) {
        e.preventDefault();
        openId = el.dataset.id;
      }
    }
  }
</script>

<svelte:window {onkeydown} {onpointermove} {onpointerup} onpointercancel={endDrag} />

{#snippet cards(key: string, showProject: boolean)}
  {#each cellTasks(key) as t (t.id)}
    {#if lineBefore === t.id && over?.key === key}<div class="drop-line"></div>{/if}
    <TaskCard task={t} {showProject} dim={filtering && !matches(t)} dragging={!dragFromFocus && dragId === t.id} />
  {/each}
  {#if lineBefore === "end" && over?.key === key}<div class="drop-line"></div>{/if}
{/snippet}

{#snippet colHead(s: { id: string; label: string; wip_limit: number | null })}
  <div class="w-col-head">
    <h2 class="w-h2">{s.label}</h2>
    <span class="w-mono" class:over-limit={overLimit(s)} title={s.wip_limit ? `WIP limit ${s.wip_limit}` : undefined}>{countLabel(s)}</span>
  </div>
{/snippet}

<header class="w-page-head">
  <div>
    <h1 class="w-h1">All tasks</h1>
    <div class="w-sub">{subline}</div>
  </div>
  <div class="w-toolbar">
    <label class="w-search">
      <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" aria-hidden="true" class="lens"
        ><circle cx="11" cy="11" r="7" /><path d="m20 20-3.5-3.5" /></svg
      >
      <input bind:this={searchEl} bind:value={query} type="search" placeholder="Search id or title" aria-label="Search tasks (⌘K)" />
      <kbd class="w-mono">⌘K</kbd>
    </label>
    <div class="w-seg" role="group" aria-label="Grouping">
      <button type="button" aria-pressed={group === "status"} onclick={() => (group = "status")}>By status</button>
      <button type="button" aria-pressed={group === "project"} onclick={() => (group = "project")}>By project</button>
    </div>
    {#if focusHidden}
      <button type="button" class="w-btn" onclick={() => saveSettings({ focus_hidden: false })}>Show focus · {focus.length}</button>
    {/if}
    <button type="button" class="w-btn w-btn--primary" onclick={() => (adding = true)}>+ New task</button>
  </div>
</header>

<!-- svelte-ignore a11y_no_static_element_interactions -->
<div class="board-area" {onpointerdown}>
  {#if !focusHidden}
    <section class="w-focus" class:is-over={isOver("focus")} data-drop="focus" aria-label="Focus today">
      <div class="w-focus-head">
        <span class="w-caps">Focus today</span>
        <span class="w-sub hint">{focus.length ? `${plural(focus.length, "task")}, in this order` : "Drag up to three cards here"}</span>
        <button type="button" class="w-btn w-btn--quiet" onclick={() => saveSettings({ focus_hidden: true })}>Hide</button>
      </div>
      {#if focus.length}
        <ol class="w-focus-list">
          {#each focus as t, i (t.id)}
            {@const p = projectOf(t)}
            <!-- svelte-ignore a11y_no_noninteractive_element_to_interactive_role -->
            <li class="w-focus-item" class:is-dragging={dragFromFocus && dragId === t.id} data-id={t.id} role="button" tabindex="0" aria-label="{i + 1}. {t.id} {t.title}">
              <span class="w-focus-num">{i + 1}</span>
              <div class="fi-text">
                <strong>{t.title}</strong>
                <span class="w-mono"
                  >{[t.id, t.priority && `P${t.priority}`, t.due && `due ${shortDate(t.due)}`, p?.title ?? "Inbox"].filter(Boolean).join(" · ")}</span
                >
              </div>
              <button type="button" class="fi-x" aria-label="Remove {t.id} from focus" onclick={() => setFocus(focusIds.filter((x) => x !== t.id))}
                >✕</button
              >
            </li>
          {/each}
        </ol>
      {/if}
    </section>
  {/if}

  <div class="filters" role="group" aria-label="Filters">
    <select class="w-chip" class:on={fProject} bind:value={fProject} aria-label="Project">
      <option value="">All projects</option>
      {#each projects as p (p.key)}<option value={p.key}>{p.title}</option>{/each}
      <option value="inbox">Inbox</option>
    </select>
    {#each [1, 2, 3] as p (p)}
      <button type="button" class="w-chip" aria-pressed={fPrio === p} onclick={() => (fPrio = fPrio === p ? null : p)}
        ><span class="w-prio w-prio--{p}">P{p}</span></button
      >
    {/each}
    <select class="w-chip" class:on={fTag} bind:value={fTag} aria-label="Tag">
      <option value="">All tags</option>
      {#each allTags as tag (tag)}<option value={tag}>#{tag}</option>{/each}
    </select>
    <button type="button" class="w-chip" aria-pressed={fDue === "overdue"} onclick={() => (fDue = fDue === "overdue" ? "" : "overdue")}
      ><span class="w-dot" style:--c="var(--w-warn)"></span>Overdue</button
    >
    <button type="button" class="w-chip" aria-pressed={fDue === "week"} onclick={() => (fDue = fDue === "week" ? "" : "week")}>Due this week</button>
    {#if filtering}
      <span class="w-sub">{matchCount} {matchCount === 1 ? "match" : "matches"}</span>
      <button type="button" class="w-btn w-btn--quiet clear" onclick={clearFilters}>Clear</button>
    {/if}
  </div>

  <div class="w-board-scroll">
    {#if group === "status"}
      <div class="w-board" style:--cols={statuses.length}>
        {#each statuses as s (s.id)}
          {@const key = cellKey(s.id)}
          <section class="w-tray w-col" class:w-col--done={s.id === "done"} class:is-over={isOver(key)} data-drop={key} aria-label={s.label}>
            {@render colHead(s)}
            {@render cards(key, true)}
          </section>
        {/each}
      </div>
    {:else}
      <div class="w-board lanes" style:--cols={statuses.length}>
        <div class="w-group-cells heads">
          {#each statuses as s (s.id)}{@render colHead(s)}{/each}
        </div>
        {#each lanes as lane (lane.key)}
          <section class="w-tray" aria-label={lane.title}>
            <div class="w-group-head">
              <span class="w-proj-mark" style:--c={lane.color}></span>
              <h2 class="w-h2">{lane.title}</h2>
              <span class="w-sub">{tasks.filter((t) => laneOf(t) === lane.key && t.status !== "done").length} open</span>
            </div>
            <div class="w-group-cells">
              {#each statuses as s (s.id)}
                {@const key = cellKey(s.id, lane.key)}
                <div class="w-group-cell" class:is-over={isOver(key)} data-drop={key}>{@render cards(key, false)}</div>
              {/each}
            </div>
          </section>
        {/each}
      </div>
    {/if}
  </div>
</div>

{#if openId}
  {#key openId}
    <TaskDetail id={openId} mode={detailMode} {focusIds} onclose={() => (openId = null)} />
  {/key}
{/if}

{#if adding}
  <QuickAdd project={fProject && fProject !== "inbox" ? fProject : null} onclose={() => (adding = false)} />
{/if}

<style>
  .board-area {
    display: flex;
    flex-direction: column;
    gap: 18px;
  }
  .lens {
    color: var(--w-muted);
    flex: none;
  }
  kbd {
    color: var(--w-muted);
  }
  .hint {
    font-size: var(--w-fs-caption);
  }
  .w-focus.is-over {
    box-shadow:
      var(--w-shadow-panel),
      inset 0 0 0 2px var(--w-accent);
  }
  .w-focus-item {
    position: relative;
    cursor: grab;
  }
  .w-focus-item.is-dragging {
    opacity: 0.3;
  }
  .fi-text {
    min-width: 0;
    display: flex;
    flex-direction: column;
    gap: 2px;
  }
  .fi-text .w-mono {
    color: var(--w-muted);
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .fi-x {
    margin-left: auto;
    flex: none;
    width: 28px;
    height: 28px;
    margin: -6px -6px 0 auto;
    border: 0;
    border-radius: var(--w-r-sm);
    background: none;
    color: var(--w-muted);
    cursor: pointer;
    opacity: 0;
    transition: opacity var(--w-dur) var(--w-ease);
  }
  .w-focus-item:hover .fi-x,
  .fi-x:focus-visible {
    opacity: 1;
  }
  .fi-x:hover {
    background: var(--w-tray);
    color: var(--w-ink);
  }
  .filters {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: var(--w-s-2);
    margin-bottom: -6px;
  }
  .clear {
    padding: 4px 8px;
  }
  select.w-chip {
    padding-right: 6px;
  }
  .w-chip.on {
    box-shadow: inset 0 0 0 1.5px var(--w-ink);
  }
  .w-chip .w-prio {
    margin: 0;
  }
  .over-limit {
    color: var(--w-warn) !important;
    font-weight: 500;
  }
  .lanes {
    display: flex;
    flex-direction: column;
    gap: 12px;
  }
  .heads {
    padding: 0 8px;
  }
  .drop-line {
    height: 2px;
    margin: -5px 2px;
    border-radius: var(--w-r-pill);
    background: var(--w-accent);
  }
</style>
