<script lang="ts">
  import { goto } from "$app/navigation";
  import ProjectForm from "$lib/components/ProjectForm.svelte";
  import TaskDetail from "$lib/components/TaskDetail.svelte";
  import { workspace, openTasks, stepLabel, projColor, reorderProjects, type ProjectEntry, type TaskEntry } from "$lib/stores/workspace.svelte";
  import { dueTip, isOverdue, nextUp, openQuickAdd, shortDate, startClock, today } from "$lib/tasks.svelte";
  import { daysInStep, isStale, label, phaseIndex, phaseOf, statusColor, stepOf, type UC } from "$lib/usecases.svelte";
  import { clipped, tip } from "$lib/tip";

  startClock();

  let creating = $state(false);
  let openId = $state<string | null>(null);
  const detailMode = $derived(workspace.settings?.task_detail ?? "popup");

  const projects = $derived(workspace.index?.projects ?? []);
  const live = $derived(projects.filter((p) => p.status !== "archived"));
  const archived = $derived(projects.filter((p) => p.status === "archived"));
  const statuses = $derived(workspace.index?.config.task_statuses ?? []);
  const proc = $derived(workspace.index?.process ?? null);

  // ------------------------------------------------------------ per project

  // Bar colours for the spec's task statuses; any other status is muted.
  const BAR: Record<string, string> = {
    backlog: "color-mix(in srgb, var(--w-edge) 45%, transparent)",
    todo: "var(--w-hold)",
    doing: "var(--w-info)",
    review: "var(--w-accent)",
    done: "var(--w-ok)",
  };
  const barColor = (id: string) => BAR[id] ?? "var(--w-muted)";

  const byProject = $derived.by(() => {
    const m = new Map<string, TaskEntry[]>();
    for (const t of workspace.index?.tasks ?? []) if (t.project) m.set(t.project, [...(m.get(t.project) ?? []), t]);
    return m;
  });
  const info = $derived.by(() => {
    const day = today();
    return new Map(
      projects.map((p) => {
        const tasks = byProject.get(p.path) ?? [];
        const next = nextUp(tasks);
        const overdue = tasks.filter(isOverdue).length;
        return [
          p.key,
          {
            next,
            open: openTasks(p),
            counts: statuses.map((s) => ({ ...s, n: tasks.filter((t) => t.status === s.id).length })).filter((s) => s.n),
            moreOverdue: overdue - (next && isOverdue(next) ? 1 : 0),
            overdue: overdue > 0,
            dueToday: tasks.some((t) => t.status !== "done" && t.due?.slice(0, 10) === day),
            focus: tasks.some((t) => t.focus?.slice(0, 10) === day),
          },
        ];
      }),
    );
  });
  const of = (p: ProjectEntry) => info.get(p.key)!;
  const repoName = (p: ProjectEntry) => p.repos[0]?.split("/").filter(Boolean).pop() ?? null;

  // Same list of three as the focus strip, so the detail card's focus toggle works here too.
  const focusIds = $derived(
    (workspace.index?.tasks ?? [])
      .filter((t) => t.focus?.slice(0, 10) === today())
      .sort((a, b) => (a.focus_order ?? 9) - (b.focus_order ?? 9))
      .map((t) => t.id),
  );

  // --------------------------------------------------------------- filters

  const CHIPS = [
    { id: "overdue", text: "Overdue", color: "var(--w-danger)", test: (p: ProjectEntry) => of(p).overdue },
    { id: "today", text: "Due today", color: "var(--w-warn)", test: (p: ProjectEntry) => of(p).dueToday },
    { id: "focus", text: "In focus", color: "var(--w-accent)", test: (p: ProjectEntry) => of(p).focus },
    { id: "idle", text: "No open tasks", color: "var(--w-muted)", test: (p: ProjectEntry) => of(p).open === 0 },
  ];
  let query = $state("");
  let fChips = $state<string[]>([]);
  const filtering = $derived(!!(query.trim() || fChips.length));
  const toggleChip = (id: string) => (fChips = fChips.includes(id) ? fChips.filter((x) => x !== id) : [...fChips, id]);

  // Chips are OR, like the cockpit's status chips.
  function matches(p: ProjectEntry) {
    const q = query.trim().toLowerCase();
    if (q && !`${p.key} ${p.title}`.toLowerCase().includes(q)) return false;
    return !fChips.length || CHIPS.some((c) => fChips.includes(c.id) && c.test(p));
  }

  // ------------------------------------------------------------------ sort

  const ucRank = (p: ProjectEntry) => {
    if (!p.usecase || !proc) return null;
    const ph = phaseIndex(proc, phaseOf(proc, p as UC));
    return (ph < 0 ? 999 : ph) * 1000 + (stepOf(proc, p.usecase.step)?.col ?? 999);
  };
  const COLS = [
    ["key", "ID", (p: ProjectEntry) => p.key],
    ["title", "Title", (p: ProjectEntry) => p.title.toLowerCase()],
    ["tasks", "Tasks", (p: ProjectEntry) => of(p).open],
    ["next", "Next up", null],
    ["due", "Due", (p: ProjectEntry) => of(p).next?.due?.slice(0, 10) ?? null],
    ["usecase", "Use case", ucRank],
    ["repo", "Repo", (p: ProjectEntry) => repoName(p)?.toLowerCase() ?? null],
  ] as const;
  type Col = (typeof COLS)[number][0];

  // No sort = the manual order (`order`), the only one that can be dragged.
  let sort = $state<{ col: Col; dir: 1 | -1 } | null>(null);
  const by = (col: Col) => (sort = sort?.col !== col ? { col, dir: 1 } : sort.dir === 1 ? { col, dir: -1 } : null);

  function sorted(list: ProjectEntry[]) {
    if (!sort) return list;
    const { col, dir } = sort;
    const get = COLS.find(([c]) => c === col)![2] as (p: ProjectEntry) => string | number | null;
    return [...list].sort((a, b) => {
      const [x, y] = [get(a), get(b)];
      // Empty values go last in both directions.
      if (x === null || y === null) return x === y ? a.key.localeCompare(b.key) : x === null ? 1 : -1;
      const c = typeof x === "number" ? x - (y as number) : x.localeCompare(y as string);
      return (c || a.key.localeCompare(b.key)) * dir;
    });
  }

  // ------------------------------------------------------------------ drag

  // `draft` is the order shown while dragging, written as `order` on drop.
  let dragKey = $state<string | null>(null);
  let draft = $state<string[] | null>(null);
  let dropped = false;
  const ordered = $derived(draft ? draft.map((k) => live.find((p) => p.key === k)).filter((p) => p !== undefined) : live);
  const shown = $derived(sorted(ordered).filter(matches));
  const canDrag = $derived(!sort && !filtering);

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

  // A click on the row opens the overview; links and buttons inside it do their own thing.
  function rowClick(e: MouseEvent, p: ProjectEntry) {
    if (!(e.target as Element).closest("a, button")) goto(`/projects/${p.key}?tab=overview`);
  }
</script>

{#snippet row(p: ProjectEntry, draggable: boolean)}
  {@const i = of(p)}
  {@const t = i.next}
  {@const uc = p.usecase}
  {@const repo = repoName(p)}
  <!-- svelte-ignore a11y_click_events_have_key_events, a11y_no_noninteractive_element_interactions -->
  <tr
    class:is-dragging={dragKey === p.key}
    class:paused={p.status === "paused"}
    {draggable}
    onclick={(e) => rowClick(e, p)}
    ondragstart={(e) => dragstart(e, p)}
    ondragover={(e) => draggable && dragover(e, p)}
    ondragend={dragend}
  >
    <td class="id"><span class="w-proj-mark" style:--c={projColor(p.color)}></span><span class="w-mono">{p.key}</span></td>
    <!-- The title link carries keyboard access; a mouse click anywhere on the row opens it too. -->
    <td class="title">
      <a href="/projects/{p.key}?tab=overview" draggable="false" use:tip={clipped(p.title)}>{p.title}</a>
      {#if p.status === "paused"}<span class="badge">Paused</span>{/if}
    </td>
    <td>
      <span class="tasks" use:tip={i.counts.map((s) => `${s.n} ${s.label.toLowerCase()}`).join(" · ") || "No tasks"}>
        <span class="bar">{#each i.counts as s (s.id)}<i style:flex={s.n} style:background={barColor(s.id)}></i>{/each}</span>
        <span class="w-mono muted">{i.open} open</span>
      </span>
    </td>
    <td class="next">
      {#if t}
        <span class="w-mono muted">{t.id}</span>
        <button type="button" use:tip={clipped(t.title)} onclick={() => (openId = t.id)}>{t.title}</button>
      {:else if p.status === "archived"}
        <span class="muted">–</span>
      {:else}
        <span class="muted">No open tasks ·</span>
        <button type="button" class="add" onclick={() => openQuickAdd(p.key)}>+ Add task</button>
      {/if}
    </td>
    <td>
      {#if t?.due}
        {@const d = t.due.slice(0, 10)}
        <span class="w-mono" class:overdue={isOverdue(t)} class:today={d === today()} use:tip={dueTip(t.due)}>{d === today() ? "today" : shortDate(d)}</span>
      {:else}<span class="muted">–</span>{/if}
      {#if i.moreOverdue}<span class="badge badge--danger">+{i.moreOverdue} overdue</span>{/if}
    </td>
    <td>
      {#if uc}
        {@const stale = proc && isStale(proc, p as UC)}
        <span class="uc" class:blocked={uc.status === "blocked"} use:tip={proc && label(proc.statuses, uc.status)}
          ><span class="w-dot" class:w-dot--hollow={uc.status === "on_hold"} style:--c={statusColor(uc.status)}></span>{stepLabel(p) ?? "–"}
          {#if stale}<span class="w-mono stale">{daysInStep(p as UC)}d</span>{/if}</span
        >
      {:else}<span class="muted">–</span>{/if}
    </td>
    <td>
      {#if repo}<span class="w-mono muted repo" use:tip={p.repos.join("\n")}>{repo}{#if p.repos.length > 1}&nbsp;+{p.repos.length - 1}{/if}</span>
      {:else}<span class="muted">–</span>{/if}
    </td>
  </tr>
{/snippet}

{#snippet table(list: ProjectEntry[], live: boolean)}
  <div class="list">
    <table ondragover={(e) => dragKey && e.preventDefault()} ondrop={drop}>
      <thead>
        <tr>
          {#each COLS as [col, text, get] (col)}
            {#if get && live}
              <th aria-sort={sort?.col === col ? (sort.dir === 1 ? "ascending" : "descending") : "none"}>
                <button type="button" onclick={() => by(col)}>{text}{#if sort?.col === col}<span class="arrow">{sort.dir === 1 ? "↑" : "↓"}</span>{/if}</button>
              </th>
            {:else}
              <th><span>{text}</span></th>
            {/if}
          {/each}
        </tr>
      </thead>
      <tbody>
        {#each list as p (p.key)}
          {@render row(p, live && canDrag)}
        {:else}
          <tr><td class="empty">No project matches the filters.</td></tr>
        {/each}
      </tbody>
    </table>
  </div>
{/snippet}

<header class="w-page-head">
  <div>
    <h1 class="w-h1">Projects</h1>
    <div class="w-sub">All projects in one table. Click a column to sort, a row to open the overview.</div>
  </div>
  <div class="w-toolbar">
    <label class="w-search">
      <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" aria-hidden="true" class="lens"
        ><circle cx="11" cy="11" r="7" /><path d="m20 20-3.5-3.5" /></svg
      >
      <input bind:value={query} type="search" placeholder="Search key or title" aria-label="Search projects" />
    </label>
    <button class="w-btn w-btn--primary" onclick={() => (creating = true)}>+ New project</button>
  </div>
</header>

{#if !projects.length}
  <p class="w-sub">No projects yet. A project is a folder with a _project.md, made here or from the template in Obsidian.</p>
{/if}

{#if live.length}
  <div class="summary" role="group" aria-label="Filter">
    <span class="w-chip w-chip--static"><b>{live.length}</b> projects</span>
    {#each CHIPS as c (c.id)}
      <button type="button" class="w-chip" aria-pressed={fChips.includes(c.id)} onclick={() => toggleChip(c.id)}
        ><span class="w-dot" class:w-dot--hollow={c.id === "idle"} style:--c={c.color}></span><b>{live.filter(c.test).length}</b> {c.text}</button
      >
    {/each}
    {#if filtering}
      <button type="button" class="w-btn w-btn--quiet" onclick={() => ([query, fChips] = ["", []])}>Clear filters</button>
    {/if}
  </div>

  {@render table(shown, true)}

  <div class="legend">
    {#each statuses as s (s.id)}<span><i style:background={barColor(s.id)}></i>{s.label}</span>{/each}
    <span>Next up = overdue first, then today's focus, priority, due date. Without a sorted column, drag a row to change the order.</span>
  </div>
{/if}

{#if archived.length}
  <details class="archive">
    <summary class="w-caps">Archived · {archived.length}</summary>
    {@render table(archived, false)}
  </details>
{/if}

{#if creating}
  <ProjectForm onclose={() => (creating = false)} />
{/if}

{#if openId}
  <TaskDetail id={openId} mode={detailMode} {focusIds} onclose={() => (openId = null)} />
{/if}

<style>
  .summary {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: var(--w-s-2);
  }
  .list {
    overflow: auto;
    border-radius: var(--w-r-lg);
    background: var(--w-surface);
    box-shadow: var(--w-shadow-panel);
    container-type: inline-size;
  }
  /* Grid table: Title, Tasks and Next up have their full width from a ~1600px table up and shrink
     proportionally below it; Due takes any extra width, so Use case and Repo stay at the right edge. */
  table {
    display: grid;
    grid-template-columns: max-content min(260px, 16%) min(194px, 12%) min(476px, 30%) minmax(max-content, 1fr) max-content fit-content(160px);
    width: 100%;
    font-size: var(--w-fs-small);
  }
  thead,
  tbody {
    display: contents;
  }
  tr {
    display: grid;
    grid-template-columns: subgrid;
    grid-column: 1 / -1;
  }
  th,
  td {
    display: flex;
    align-items: center;
    gap: 6px;
    min-width: 0;
    padding: 0 12px;
    white-space: nowrap;
    overflow: hidden;
  }
  /* The gap before Next up shrinks with the table too: 64px at ~1600px. */
  th:nth-child(4),
  td:nth-child(4) {
    padding-left: min(calc(2 * var(--w-s-8)), 4cqw);
  }
  th {
    background: var(--w-tray);
    border-bottom: 1px solid var(--w-line);
    font-family: var(--w-font-label);
    font-size: var(--w-fs-caption);
    font-weight: 600;
    text-transform: uppercase;
    letter-spacing: 0.06em;
    color: var(--w-muted);
  }
  th > span {
    padding: 9px 0;
  }
  th button {
    all: unset;
    box-sizing: border-box;
    padding: 9px 0;
    cursor: pointer;
  }
  th button:hover,
  th[aria-sort="ascending"] button,
  th[aria-sort="descending"] button {
    color: var(--w-ink);
  }
  th button:focus-visible {
    outline: 2px solid var(--w-accent);
    outline-offset: 2px;
  }
  .arrow {
    margin-left: 4px;
  }
  td {
    height: 38px;
    box-sizing: border-box;
    border-bottom: 1px solid var(--w-line);
  }
  tbody tr:last-child td {
    border-bottom: 0;
  }
  tbody tr {
    cursor: pointer;
    transition: background var(--w-dur) var(--w-ease);
  }
  tbody tr:hover {
    background: color-mix(in srgb, var(--w-tray) 60%, var(--w-surface));
  }
  tr.is-dragging {
    opacity: 0.3;
  }
  .muted {
    color: var(--w-muted);
  }
  .id {
    gap: 8px;
  }
  .id .w-mono {
    color: var(--w-muted);
  }
  .title a,
  .next button,
  .repo {
    min-width: 0;
    overflow: hidden;
    text-overflow: ellipsis;
  }
  .title a {
    color: var(--w-ink);
    text-decoration: none;
    font-size: var(--w-fs-body);
    font-weight: 500;
  }
  .title a:focus-visible,
  .next button:focus-visible {
    outline: 2px solid var(--w-accent);
    outline-offset: 2px;
    border-radius: var(--w-r-sm);
  }
  tr.paused .title a {
    color: var(--w-muted);
    font-weight: 400;
  }
  .tasks {
    display: flex;
    align-items: center;
    gap: 10px;
    width: 100%;
  }
  .bar {
    flex: 1;
    display: flex;
    gap: 2px;
    height: 6px;
    border-radius: var(--w-r-pill);
    overflow: hidden;
    background: var(--w-sunk);
  }
  .next button {
    all: unset;
    min-width: 0;
    overflow: hidden;
    text-overflow: ellipsis;
    color: var(--w-ink);
    cursor: pointer;
  }
  .next button:hover {
    text-decoration: underline;
    text-decoration-color: var(--w-edge);
  }
  .next button.add {
    color: var(--w-accent);
  }
  .overdue {
    color: var(--w-danger);
  }
  .today {
    color: var(--w-warn);
  }
  .badge {
    flex: none;
    font-size: var(--w-fs-caption);
    font-weight: 500;
    padding: 1px 8px;
    border-radius: var(--w-r-pill);
    color: var(--w-muted);
    box-shadow: inset 0 0 0 1px var(--w-line);
  }
  .badge--danger {
    color: var(--w-danger);
    background: var(--w-danger-soft);
    box-shadow: none;
  }
  /* Repo may shrink to a short stub; Use case keeps its width. */
  td:last-child {
    min-width: 72px;
  }
  .uc {
    display: flex;
    align-items: center;
    gap: 7px;
  }
  .uc.blocked {
    color: var(--w-danger);
  }
  .stale {
    color: var(--w-warn);
  }
  .uc.blocked .stale {
    color: inherit;
  }
  .empty {
    grid-column: 1 / -1;
    justify-content: center;
    height: auto;
    padding: 28px;
    color: var(--w-muted);
    cursor: default;
  }
  .legend {
    display: flex;
    flex-wrap: wrap;
    gap: 14px;
    font-size: var(--w-fs-caption);
    color: var(--w-muted);
  }
  .legend span {
    display: inline-flex;
    align-items: center;
    gap: 6px;
  }
  .legend i {
    width: 9px;
    height: 9px;
    border-radius: 2px;
  }
  .archive summary {
    cursor: pointer;
    padding: var(--w-s-1) 0;
    margin-bottom: var(--w-s-2);
  }
  .archive .list {
    background: transparent;
    box-shadow: inset 0 0 0 1px var(--w-line);
  }
  .archive .title a {
    color: var(--w-muted);
  }
</style>
