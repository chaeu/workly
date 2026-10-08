<script lang="ts">
  import UseCaseMap from "$lib/components/UseCaseMap.svelte";
  import UseCaseList from "$lib/components/UseCaseList.svelte";
  import ProcessError from "$lib/components/ProcessError.svelte";
  import UseCaseDetail from "$lib/components/UseCaseDetail.svelte";
  import ProjectForm from "$lib/components/ProjectForm.svelte";
  import { workspace, moveUseCase, updateProjectField, saveSettings, openInVscode } from "$lib/stores/workspace.svelte";
  import { startClock } from "$lib/tasks.svelte";
  import {
    ui,
    PROCESS,
    daysInStep,
    fmtFte,
    savedFte,
    isStale,
    knownAreas,
    label,
    moveMessage,
    phaseOf,
    statusColor,
    stepName,
    stepOf,
    stepTip,
    fteTip,
    ucTip,
    type UC,
  } from "$lib/usecases.svelte";
  import { tip } from "$lib/tip";

  startClock();

  const p = $derived(workspace.index?.process ?? null);
  const processErrors = $derived(workspace.index?.errors.filter((e) => e.path === PROCESS) ?? []);
  const ucs = $derived((workspace.index?.projects.filter((x) => x.usecase && x.status !== "archived") ?? []) as UC[]);
  const unplaced = $derived(p ? ucs.filter((u) => !phaseOf(p, u)) : []);
  const detailMode = $derived(workspace.settings?.task_detail ?? "popup");
  let openKey = $state<string | null>(null);
  const compact = $derived(workspace.settings?.usecase_compact ?? false);

  // A new use case opens once the reloaded index has it (the reload is async).
  let creating = $state(false);
  let openWhenLoaded = $state<string | null>(null);
  $effect(() => {
    if (openWhenLoaded && ucs.some((u) => u.key === openWhenLoaded)) {
      openKey = openWhenLoaded;
      openWhenLoaded = null;
    }
  });

  // --------------------------------------------------------------- lanes

  const LANE_MODES = [
    ["area", "Area"],
    ["type", "Type"],
    ["status", "Status"],
  ] as const;
  const laneField = $derived(ui.lanes);
  const laneValue = (u: UC) => u.usecase[laneField] ?? "";
  /** Configured values first, then values only the files use, so no card goes missing. Areas are free text: only those in use get a lane. */
  const lanes = $derived.by(() => {
    if (!p) return [];
    const used = new Set(ucs.map(laneValue));
    const configured =
      ui.lanes === "area" ? p.areas.filter((a) => used.has(a)).map((a) => ({ id: a, label: a })) : ui.lanes === "type" ? p.types : p.statuses;
    const extra = [...used].filter((v) => !configured.some((c) => c.id === v));
    return [...configured, ...extra.map((v) => ({ id: v, label: v || `No ${ui.lanes}` }))];
  });

  // ------------------------------------------------------------- filters

  let fType = $state("");
  let fArea = $state("");
  let fStatus = $state<string[]>([]);
  let fStale = $state(false);
  let query = $state("");
  const areas = $derived(p ? knownAreas(p) : []);
  const filtering = $derived(!!(fType || fArea || fStatus.length || fStale || query.trim()));
  function matches(u: UC) {
    const q = query.trim().toLowerCase();
    if (q && !`${u.key} ${u.title}`.toLowerCase().includes(q)) return false;
    if (fType && u.usecase.type !== fType) return false;
    if (fArea && u.usecase.area !== fArea) return false;
    if (fStatus.length && !fStatus.includes(u.usecase.status ?? "")) return false;
    return !fStale || (!!p && isStale(p, u));
  }
  const dim = (u: UC) => filtering && !matches(u);
  const toggleStatus = (id: string) => (fStatus = fStatus.includes(id) ? fStatus.filter((x) => x !== id) : [...fStatus, id]);

  // ---------------------------------------------------------------- moves

  let toast = $state("");
  let toastTimer: ReturnType<typeof setTimeout>;
  function say(msg: string) {
    toast = msg;
    clearTimeout(toastTimer);
    toastTimer = setTimeout(() => (toast = ""), 2800);
  }

  /** Board drag, map drag, step select and decision buttons all end here. */
  async function move(u: UC, step: string) {
    if (!p || step === u.usecase.step) return;
    const from = u.usecase.step;
    if (await moveUseCase(u.key, step)) say(moveMessage(p, u.key, from, step));
  }

  async function dropOnCell(u: UC, phase: string, lane: string) {
    if (!p) return;
    if (phase !== phaseOf(p, u)) {
      const step = p.phase_default_step[phase];
      if (!step) {
        workspace.error = `process.yml has no phase_default_step for '${phase}'.`;
        return;
      }
      await move(u, step);
    }
    if (lane !== laneValue(u)) {
      try {
        await updateProjectField(u.key, `usecase.${laneField}`, lane || null);
        say(`${u.key}: ${LANE_MODES.find(([m]) => m === ui.lanes)![1].toLowerCase()} now ${lanes.find((l) => l.id === lane)?.label ?? lane}`);
      } catch (e) {
        workspace.error = String(e);
      }
    }
  }

  // --------------------------------------------------------- drag & drop
  // Same pointer handling as the task board: 5 px threshold, a click opens the card.

  let pending: { key: string; el: HTMLElement; sx: number; sy: number; scroller: HTMLElement | null } | null = null;
  let ghost: HTMLElement | null = null;
  let offset = { x: 0, y: 0 };
  let dragKey = $state<string | null>(null);
  let over = $state<string | null>(null);
  let overEl: HTMLElement | null = null;

  function onpointerdown(e: PointerEvent) {
    const el = (e.target as HTMLElement).closest<HTMLElement>("[data-key]");
    if (e.button !== 0 || !el) return;
    pending = { key: el.dataset.key!, el, sx: e.clientX, sy: e.clientY, scroller: el.closest(".scroll") };
  }

  function onpointermove(e: PointerEvent) {
    if (!pending) return;
    if (!ghost) {
      if (Math.hypot(e.clientX - pending.sx, e.clientY - pending.sy) < 5) return;
      const r = pending.el.getBoundingClientRect();
      offset = { x: pending.sx - r.left, y: pending.sy - r.top };
      ghost = pending.el.cloneNode(true) as HTMLElement;
      ghost.classList.add("w-drag-ghost");
      ghost.classList.remove("is-dim");
      ghost.style.width = `${Math.max(r.width, 170)}px`;
      document.body.appendChild(ghost);
      dragKey = pending.key;
    }
    e.preventDefault();
    ghost.style.transform = `translate(${e.clientX - offset.x}px, ${e.clientY - offset.y}px)`;
    overEl = document.elementFromPoint(e.clientX, e.clientY)?.closest<HTMLElement>("[data-drop]") ?? null;
    over = !overEl ? null : overEl.dataset.drop === "step" ? `step:${overEl.dataset.step}` : `${overEl.dataset.phase}|${overEl.dataset.lane}`;
    // Scroll the board or map sideways near its edges.
    const s = pending.scroller;
    if (s) {
      const r = s.getBoundingClientRect();
      if (e.clientX > r.right - 48) s.scrollLeft += 16;
      else if (e.clientX < r.left + 48 + 140) s.scrollLeft -= 16;
    }
  }

  function onpointerup() {
    const d = pending;
    const target = overEl;
    if (!d) return;
    if (!ghost) {
      pending = null;
      openKey = d.key;
      return;
    }
    endDrag();
    const u = ucs.find((x) => x.key === d.key);
    if (!u || !target) return;
    if (target.dataset.drop === "step") move(u, target.dataset.step!);
    else dropOnCell(u, target.dataset.phase!, target.dataset.lane!);
  }

  function endDrag() {
    pending = null;
    ghost?.remove();
    ghost = null;
    dragKey = null;
    over = null;
    overEl = null;
  }

  function onkeydown(e: KeyboardEvent) {
    if (e.key === "Escape") {
      if (ghost) endDrag();
      else if (openKey) openKey = null;
    } else if (e.key === "Enter" || e.key === " ") {
      const el = e.target as HTMLElement;
      if (el.dataset?.key && el.getAttribute("role") === "button") {
        e.preventDefault();
        openKey = el.dataset.key;
      }
    }
  }
</script>

<svelte:window {onkeydown} {onpointermove} {onpointerup} onpointercancel={endDrag} />

{#snippet card(u: UC)}
  {@const st = stepOf(p!, u.usecase.step)}
  {@const status = u.usecase.status}
  {@const days = daysInStep(u)}
  {@const fte = savedFte(u)}
  {@const next = status === "blocked" && u.usecase.blocked_by ? u.usecase.blocked_by : u.usecase.next_step}
  <div
    class="card"
    class:blocked={status === "blocked"}
    class:hold={status === "on_hold"}
    class:is-dim={dim(u)}
    class:is-dragging={dragKey === u.key}
    role="button"
    tabindex="0"
    data-key={u.key}
    aria-label="{u.key} {u.title}, {label(p!.statuses, status)}"
    use:tip={ucTip(p!, u)}
  >
    <div class="c-top">
      <span class="w-mono c-id">{u.key}</span>
      {#if fte}<span class="fte w-mono" use:tip={fteTip(u)}>{fmtFte(fte)} FTE</span>{/if}
      {#if u.usecase.type}<span class="w-type" class:w-type--ki={u.usecase.type === "ai"} class:hybrid={u.usecase.type === "hybrid"}
          >{label(p!.types, u.usecase.type)}</span
        >{/if}
    </div>
    <div class="c-title">{u.title}</div>
    {#if st?.kind === "gate"}
      <div class="c-step"><i></i>waiting at {stepName(st)}</div>
    {:else if st && st.phase && st.id !== p!.phase_default_step[st.phase]}
      <div class="c-step">{st.label}</div>
    {/if}
    <div class="c-status">
      <span class="w-dot" class:w-dot--hollow={status === "on_hold"} style:--c={statusColor(status)}></span>{label(p!.statuses, status)}
      {#if days !== null}<span class="age w-mono" class:stale={isStale(p!, u)}>{days} d</span>{/if}
    </div>
    {#if next}<div class="c-next">{status === "blocked" ? "" : "→ "}{next}</div>{/if}
  </div>
{/snippet}

<header class="w-page-head">
  <div>
    <h1 class="w-h1">Use-case cockpit</h1>
    <div class="w-sub">
      {ui.view === "board"
        ? "Phases left to right, lanes by choice. Drag a card to change phase or lane; click opens the details."
        : ui.view === "list"
          ? "All use cases in one table. Click a column to sort, a row to open the details."
          : "Lanes show who has the ball; diamonds are decisions. Drag a use case onto the step it is in."}
    </div>
  </div>
  <div class="w-toolbar">
    <label class="w-search">
      <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" aria-hidden="true" class="lens"
        ><circle cx="11" cy="11" r="7" /><path d="m20 20-3.5-3.5" /></svg
      >
      <input bind:value={query} type="search" placeholder="Search key or title" aria-label="Search use cases" />
    </label>
    <div class="w-seg" role="group" aria-label="View">
      <button type="button" aria-pressed={ui.view === "list"} onclick={() => (ui.view = "list")}>List</button>
      <button type="button" aria-pressed={ui.view === "board"} onclick={() => (ui.view = "board")}>Board</button>
      <button type="button" aria-pressed={ui.view === "map"} onclick={() => (ui.view = "map")}>Process map</button>
    </div>
    <button type="button" class="w-btn w-btn--quiet" title="Open {PROCESS} in VS Code" onclick={() => openInVscode(PROCESS)}>Edit process</button>
    {#if p && !processErrors.length}
      <button type="button" class="w-btn w-btn--primary" onclick={() => (creating = true)}>+ New use case</button>
    {/if}
  </div>
</header>

{#if !p || processErrors.length}
  <ProcessError />
{:else}
  <div class="toolbar">
    {#if ui.view === "board"}
      <div class="group">
        <span class="w-caps">Lanes</span>
        <div class="w-seg" role="group" aria-label="Lanes">
          {#each LANE_MODES as [mode, text] (mode)}
            <button type="button" aria-pressed={ui.lanes === mode} onclick={() => (ui.lanes = mode)}>{text}</button>
          {/each}
        </div>
      </div>
    {/if}
    <div class="group">
      <span class="w-caps">Type</span>
      <div class="w-seg" role="group" aria-label="Type">
        <button type="button" aria-pressed={fType === ""} onclick={() => (fType = "")}>All</button>
        {#each p.types as t (t.id)}
          <button type="button" aria-pressed={fType === t.id} onclick={() => (fType = t.id)}>{t.label}</button>
        {/each}
      </div>
    </div>
    {#if ui.view === "board"}
      <div class="w-seg" role="group" aria-label="Density">
        <button type="button" aria-pressed={!compact} onclick={() => saveSettings({ usecase_compact: false })}>Detailed</button>
        <button type="button" aria-pressed={compact} onclick={() => saveSettings({ usecase_compact: true })}>Compact</button>
      </div>
    {/if}
    <select class="w-chip" class:on={fArea} bind:value={fArea} aria-label="Area">
      <option value="">All areas</option>
      {#each areas as a (a)}<option value={a}>{a}</option>{/each}
    </select>
    <div class="group details">
      <span class="w-caps">Details</span>
      <div class="w-seg" role="group" aria-label="Detail cards">
        <button type="button" aria-pressed={detailMode === "popup"} onclick={() => saveSettings({ task_detail: "popup" })}>Popup</button>
        <button type="button" aria-pressed={detailMode === "panel"} onclick={() => saveSettings({ task_detail: "panel" })}>Side panel</button>
      </div>
    </div>
  </div>

  <div class="summary" role="group" aria-label="Status filter">
    <span class="w-chip w-chip--static"><b>{ucs.length}</b> use cases</span>
    {#each p.statuses as s (s.id)}
      <button type="button" class="w-chip" aria-pressed={fStatus.includes(s.id)} onclick={() => toggleStatus(s.id)}
        ><span class="w-dot" class:w-dot--hollow={s.id === "on_hold"} style:--c={statusColor(s.id)}></span><b
          >{ucs.filter((u) => u.usecase.status === s.id).length}</b
        >
        {s.label}</button
      >
    {/each}
    {#if p.stale_after_days != null}
      <button type="button" class="w-chip" aria-pressed={fStale} onclick={() => (fStale = !fStale)}
        ><b>{ucs.filter((u) => isStale(p, u)).length}</b> {p.stale_after_days}+ days in the same step</button
      >
    {/if}
    {#if filtering}
      <button
        type="button"
        class="w-btn w-btn--quiet clear"
        onclick={() => {
          [fType, fArea, fStatus, fStale, query] = ["", "", [], false, ""];
        }}>Clear filters</button
      >
    {/if}
  </div>

  {#if unplaced.length}
    <p class="unplaced">
      Not on the board, the step is not in process.yml:
      {#each unplaced as u, i (u.key)}{i ? ", " : ""}<button type="button" class="link" onclick={() => (openKey = u.key)}
          >{u.key} ({u.usecase.step ?? "no step"})</button
        >{/each}
    </p>
  {/if}

  {#if ui.view === "list"}
    <UseCaseList process={p} ucs={ucs.filter(matches)} onopen={(key) => (openKey = key)} />
  {:else}
  <!-- svelte-ignore a11y_no_static_element_interactions -->
  <div class="scroll" class:map-wrap={ui.view === "map"} {onpointerdown}>
    {#if ui.view === "board"}
      <div class="board" class:compact style:--n={p.phases.length}>
        <div class="corner w-caps">{LANE_MODES.find(([m]) => m === ui.lanes)![1]} / phase</div>
        {#each p.phases as ph, i (ph.id)}
          <div class="ph" class:parked={ph.parked} class:optional={ph.optional}>
            {#if !ph.parked}<span class="num w-mono">{i + 1}</span>{/if}
            <div>
              <strong>{ph.name}{#if ph.optional}<span class="opt">optional</span>{/if}</strong>
              {#if ph.desc}<small>{ph.desc}</small>{/if}
            </div>
            <span class="cnt w-mono">{ucs.filter((u) => phaseOf(p, u) === ph.id).length}</span>
          </div>
        {/each}
        <div class="g-row corner-g w-caps">Gates</div>
        {#each p.phases as ph (ph.id)}
          {@const g = stepOf(p, p.board_gates[ph.id])}
          <div class="g-row">
            {#if g}<div class="bgate w-gate" use:tip={stepTip(g)}><code>{g.code ?? g.id}</code><span>{g.label}</span></div>{/if}
          </div>
        {/each}
        {#each lanes as l, li (l.id)}
          {@const last = li === lanes.length - 1}
          <div class="b-lane" class:last>
            <strong>{l.label}</strong><small>{ucs.filter((u) => laneValue(u) === l.id).length}</small>
          </div>
          {#each p.phases as ph, pi (ph.id)}
            <div
              class="cell"
              class:odd={pi % 2}
              class:after-gate={pi > 0 && !!p.board_gates[p.phases[pi - 1].id]}
              class:parked={ph.parked}
              class:optional={ph.optional}
              class:last
              class:over={over === `${ph.id}|${l.id}`}
              data-drop="cell"
              data-phase={ph.id}
              data-lane={l.id}
            >
              {#each ucs.filter((u) => phaseOf(p, u) === ph.id && laneValue(u) === l.id) as u (u.key)}{@render card(u)}{/each}
            </div>
          {/each}
        {/each}
      </div>
    {:else}
      <UseCaseMap process={p} {ucs} {dim} {dragKey} {over} />
    {/if}
  </div>
  {/if}

  <div class="legend">
    {#each p.statuses as s (s.id)}
      <span><span class="w-dot" class:w-dot--hollow={s.id === "on_hold"} style:--c={statusColor(s.id)}></span>{s.label}</span>
    {/each}
    <span
      >Number = days in the current step{p.stale_after_days != null ? `, orange from ${p.stale_after_days}` : ""}. Diamond = decision (gate).</span
    >
  </div>

  {#if creating}
    <ProjectForm usecase onclose={() => (creating = false)} oncreated={(key) => (openWhenLoaded = key)} />
  {/if}

  {#if openKey}
    {#key openKey}
      <UseCaseDetail key={openKey} process={p} mode={detailMode} onmove={move} onclose={() => (openKey = null)} />
    {/key}
  {/if}
{/if}

{#if toast}<div class="toast" role="status">{toast}</div>{/if}

<style>
  .lens {
    color: var(--w-muted);
    flex: none;
  }
  .toolbar {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: 10px 18px;
    margin-bottom: 12px;
  }
  .group {
    display: flex;
    align-items: center;
    gap: 8px;
  }
  .details {
    margin-left: auto;
  }
  select.w-chip {
    padding-right: 6px;
  }
  .w-chip.on {
    box-shadow: inset 0 0 0 1.5px var(--w-ink);
  }
  .summary {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: 8px;
    margin-bottom: 14px;
  }
  .clear {
    padding: 4px 8px;
  }
  .unplaced {
    margin: 0 0 12px;
    font-size: var(--w-fs-small);
    color: var(--w-warn);
  }
  .link {
    border: 0;
    background: none;
    padding: 0;
    color: inherit;
    font: inherit;
    text-decoration: underline;
    cursor: pointer;
  }

  /* ---------- shared scroll container ---------- */
  .scroll {
    --lane-w: 140px;
    --col-w: 192px;
    overflow-x: auto;
    border-radius: var(--w-r-lg);
    background: var(--w-tray);
  }
  .map-wrap {
    background: var(--w-surface);
    box-shadow: var(--w-shadow-panel);
  }

  /* ---------- board ---------- */
  .board {
    display: grid;
    grid-template-columns: var(--lane-w) repeat(var(--n), minmax(var(--col-w), 1fr));
    min-width: calc(var(--lane-w) + var(--n) * var(--col-w));
  }
  .corner,
  .b-lane,
  .corner-g {
    position: sticky;
    left: 0;
    z-index: 3;
    background: var(--w-tray);
    border-right: 1px solid var(--w-bg);
  }
  .corner {
    padding: 12px;
    display: flex;
    align-items: flex-end;
    border-bottom: 1px solid var(--w-bg);
  }
  .ph {
    padding: 12px 12px 10px;
    display: flex;
    gap: 9px;
    align-items: flex-start;
    border-bottom: 1px solid var(--w-bg);
    min-width: 0;
  }
  .ph .num {
    color: var(--w-accent);
    box-shadow: inset 0 0 0 1px currentColor;
    border-radius: var(--w-r-sm);
    padding: 0 5px;
    margin-top: 2px;
  }
  .ph strong {
    display: block;
    font-family: var(--w-font-label);
    font-size: var(--w-fs-title);
    font-weight: 600;
  }
  .ph small {
    display: block;
    color: var(--w-muted);
    font-size: var(--w-fs-caption);
    line-height: 1.35;
  }
  .ph .cnt {
    margin-left: auto;
    color: var(--w-muted);
  }
  .ph.parked strong {
    color: var(--w-muted);
  }
  .ph.optional .num {
    box-shadow: none;
    outline: 1px dashed currentColor;
    outline-offset: -1px;
  }
  .opt {
    font-family: var(--w-font-mono);
    font-size: var(--w-fs-micro);
    font-weight: 500;
    color: var(--w-accent);
    outline: 1px dashed var(--w-accent);
    outline-offset: -1px;
    border-radius: var(--w-r-sm);
    padding: 0 4px;
    margin-left: 6px;
    vertical-align: 2px;
  }
  .g-row {
    height: 34px;
    position: relative;
    border-bottom: 1px solid var(--w-bg);
    background: color-mix(in srgb, var(--w-line) 55%, var(--w-tray));
  }
  .corner-g {
    padding: 9px 12px;
    background: color-mix(in srgb, var(--w-line) 55%, var(--w-tray));
  }
  .bgate {
    position: absolute;
    right: 0;
    top: 50%;
    transform: translate(50%, -50%);
    z-index: 2;
    background: color-mix(in srgb, var(--w-line) 55%, var(--w-tray));
    padding: 2px 6px;
    border-radius: var(--w-r-sm);
    cursor: help;
  }
  .b-lane {
    padding: 12px;
    border-bottom: 1px solid var(--w-bg);
  }
  .b-lane strong {
    display: block;
    font-family: var(--w-font-label);
    font-weight: 600;
    font-size: var(--w-fs-body);
  }
  .b-lane small {
    color: var(--w-muted);
    font-size: var(--w-fs-caption);
    font-variant-numeric: tabular-nums;
  }
  .cell {
    min-height: 104px;
    padding: 8px;
    display: flex;
    flex-direction: column;
    gap: 8px;
    border-left: 1px solid var(--w-bg);
    border-bottom: 1px solid var(--w-bg);
    min-width: 0;
    transition: background var(--w-dur) var(--w-ease);
  }
  .cell.odd {
    background: color-mix(in srgb, var(--w-surface) 22%, var(--w-tray));
  }
  .cell.optional {
    background: color-mix(in srgb, var(--w-accent-soft) 40%, var(--w-tray));
  }
  .cell.after-gate {
    border-left: 1.5px dashed var(--w-accent);
  }
  .cell.parked {
    background: repeating-linear-gradient(135deg, transparent 0 8px, color-mix(in srgb, var(--w-line) 60%, transparent) 8px 9px);
  }
  .cell.over {
    background: var(--w-accent-soft);
    box-shadow: inset 0 0 0 2px var(--w-accent);
  }
  .cell.last,
  .b-lane.last {
    border-bottom: 0;
  }

  /* ---------- card ---------- */
  .card {
    background: var(--w-surface);
    border-radius: var(--w-r-md);
    box-shadow: var(--w-shadow-card);
    padding: 8px 9px 9px;
    cursor: grab;
    user-select: none;
    -webkit-user-select: none;
    touch-action: none;
    display: grid;
    gap: 4px;
    min-width: 0;
    transition:
      opacity var(--w-dur) var(--w-ease),
      box-shadow var(--w-dur) var(--w-ease);
  }
  .card > :global(*) {
    min-width: 0;
  }
  .card:hover {
    box-shadow:
      var(--w-shadow-card),
      0 0 0 1px var(--w-line);
  }
  .c-top {
    display: flex;
    align-items: center;
    gap: 6px;
  }
  .c-id {
    color: var(--w-muted);
    margin-right: auto;
  }
  .fte {
    font-size: var(--w-fs-micro);
    color: var(--w-muted);
    box-shadow: inset 0 0 0 1px var(--w-line);
    border-radius: var(--w-r-pill);
    padding: 0 6px;
    white-space: nowrap;
  }
  .w-type.hybrid {
    color: var(--w-accent);
    box-shadow: inset 0 0 0 1px var(--w-accent);
  }
  .c-title {
    font-weight: 500;
    font-size: var(--w-fs-small);
    line-height: 1.3;
    overflow-wrap: break-word;
    hyphens: auto;
  }
  .c-step {
    font-size: var(--w-fs-caption);
    color: var(--w-accent);
    font-family: var(--w-font-label);
    font-weight: 600;
    display: flex;
    align-items: center;
    gap: 5px;
  }
  .c-step i {
    flex: none;
    width: 8px;
    height: 8px;
    transform: rotate(45deg);
    border: 1.5px solid currentColor;
  }
  .c-status {
    display: flex;
    align-items: center;
    gap: 6px;
    font-size: var(--w-fs-caption);
    color: var(--w-muted);
  }
  .age {
    margin-left: auto;
    color: var(--w-muted);
  }
  .age.stale {
    color: var(--w-warn);
    font-weight: 500;
  }
  .c-next {
    font-size: var(--w-fs-caption);
    color: var(--w-muted);
    line-height: 1.35;
    display: -webkit-box;
    -webkit-line-clamp: 2;
    line-clamp: 2;
    -webkit-box-orient: vertical;
    overflow: hidden;
  }
  .compact .c-next {
    display: none;
  }
  .card.blocked {
    background: var(--w-danger-soft);
    box-shadow: inset 0 0 0 1px var(--w-danger);
  }
  .card.blocked .c-status {
    color: var(--w-danger);
    font-weight: 500;
  }
  .card.hold {
    background: transparent;
    box-shadow: inset 0 0 0 1px var(--w-line);
  }
  .card.hold .c-title {
    color: var(--w-muted);
  }
  .is-dim {
    opacity: 0.2;
  }
  .is-dragging {
    opacity: 0.3;
  }

  .legend {
    display: flex;
    flex-wrap: wrap;
    gap: 6px 18px;
    margin-top: 12px;
    color: var(--w-muted);
    font-size: var(--w-fs-caption);
  }
  .legend > span {
    display: inline-flex;
    align-items: center;
    gap: 6px;
  }
  .toast {
    position: fixed;
    left: 50%;
    bottom: 20px;
    transform: translateX(-50%);
    z-index: 60;
    max-width: calc(100% - 32px);
    background: var(--w-ink);
    color: var(--w-bg);
    padding: 8px 14px;
    border-radius: var(--w-r-md);
    font-size: var(--w-fs-small);
    box-shadow: var(--w-shadow-pop);
  }
</style>
