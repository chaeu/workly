<script lang="ts">
  import type { Snippet } from "svelte";
  import { goto } from "$app/navigation";
  import AssessmentEditor from "$lib/components/AssessmentEditor.svelte";
  import AssessmentPill from "$lib/components/AssessmentPill.svelte";
  import EffortEditor from "$lib/components/EffortEditor.svelte";
  import ProcessHistory from "$lib/components/ProcessHistory.svelte";
  import { grow } from "$lib/grow";
  import { tip } from "$lib/tip";
  import { workspace, updateProjectField, updateTaskField, createTask, type Process } from "$lib/stores/workspace.svelte";
  import {
    assess,
    daysInStep,
    fmtFte,
    fmtHours,
    fteHours,
    fteTip,
    isStale,
    knownAreas,
    label,
    nextMoves,
    phaseIndex,
    savedHours,
    savingHours,
    scoreLine,
    statusColor,
    stepName,
    stepOf,
    type UC,
  } from "$lib/usecases.svelte";

  let {
    key,
    process: p,
    mode,
    onmove,
    onclose,
    description,
  }: {
    key: string;
    process: Process;
    mode: "popup" | "panel" | "page";
    onmove: (u: UC, step: string) => void;
    onclose?: () => void;
    /** Page only: the rendered _project.md body (the project page owns it, with its link handling). */
    description?: Snippet;
  } = $props();
  const pageHref = $derived(`/projects/${key}?tab=usecase`);

  const u = $derived(workspace.index?.projects.find((x) => x.key === key && x.usecase) as UC | undefined);
  const uc = $derived(u?.usecase);
  const step = $derived(stepOf(p, uc?.step));
  const lane = $derived(p.lanes.find((l) => l.id === step?.lane));
  const track = $derived(p.phases.filter((x) => !x.parked));
  const cur = $derived(track.findIndex((x) => x.id === step?.phase));
  const parked = $derived(!!step?.phase && !!p.phases[phaseIndex(p, step.phase)]?.parked);
  const moves = $derived(uc ? nextMoves(p, uc.step) : []);
  const days = $derived(u ? daysInStep(u) : null);
  const tasks = $derived(workspace.index?.tasks.filter((t) => t.project === u?.path) ?? []);
  const doneCount = $derived(tasks.filter((t) => t.status === "done").length);
  const areas = $derived(knownAreas(p));

  // Gone (deleted, archived away or no longer a use case): nothing to show.
  $effect(() => {
    if (workspace.index && !u) onclose?.();
  });

  // ⌘↩ in the card opens the full page.
  function onkeydown(e: KeyboardEvent) {
    if (mode !== "page" && e.key === "Enter" && e.metaKey) {
      e.preventDefault();
      goto(pageHref);
    }
  }

  async function save(field: string, value: unknown) {
    try {
      await updateProjectField(key, field, value);
    } catch (e) {
      workspace.error = String(e);
    }
  }
  const text = (field: string) => (e: Event & { currentTarget: HTMLTextAreaElement | HTMLInputElement }) =>
    save(field, e.currentTarget.value.trim() || null);

  const hours = $derived(u ? savedHours(u) : 0);
  let editingEffort = $state(false);
  // Hidden everywhere when process.yml has no assessment block.
  const assessed = $derived(p.assessment && uc?.assessment ? assess(p, uc.assessment) : null);
  let editingAssessment = $state(false);
  const OPEN_MAX = 5;

  // ponytail: unticking reopens as `todo` (or the first status); the task board covers other statuses.
  const reopen = $derived(workspace.index?.config.task_statuses.find((s) => s.id === "todo")?.id ?? workspace.index?.config.task_statuses[0]?.id ?? "todo");
  let newTask = $state("");
  async function addTask() {
    if (!newTask.trim()) return;
    if (await createTask(newTask.trim(), key)) newTask = "";
  }
</script>

{#snippet area(field: string, title: string, value: string | null, placeholder: string)}
  <section class="txt">
    <label for="f-{field}" class="w-caps">{title}</label>
    {#key value}
      <textarea id="f-{field}" rows="1" {placeholder} value={value ?? ""} use:grow onchange={text(`usecase.${field}`)}></textarea>
    {/key}
  </section>
{/snippet}

<svelte:window {onkeydown} />

{#if u && uc}
  <!-- Parts shared by the popup / panel (light version) and the page (everything editable). -->
  {#snippet position()}
    <div class="d-pos">
      {#if parked}
        <span class="parked-badge">{p.phases[phaseIndex(p, step?.phase)].name}</span>
      {:else}
        <ol class="track" aria-label="Phase">
          {#each track as ph, i (ph.id)}
            <li class:done={i < cur} class:cur={i === cur} class:opt={ph.optional}><i></i><span>{ph.name}</span></li>
          {/each}
        </ol>
      {/if}
      <div class="where">
        {#if step}
          Step <b>{stepName(step)}</b> · Owner <b>{lane?.label ?? step.lane}</b>
        {:else}
          <span class="stale">Unknown step “{uc.step ?? "none"}”, pick one below</span>
        {/if}
        {#if days !== null}· <span class:stale={isStale(p, u)}>{days} {days === 1 ? "day" : "days"} in step</span>{/if}
      </div>
    </div>
  {/snippet}

  {#snippet statuses()}
    <div class="status-row" role="group" aria-label="Status">
      {#each p.statuses as s (s.id)}
        <button type="button" aria-pressed={uc.status === s.id} onclick={() => save("usecase.status", s.id)}
          ><span class="w-dot" class:w-dot--hollow={s.id === "on_hold"} style:--c={statusColor(s.id)}></span>{s.label}</button
        >
      {/each}
    </div>
  {/snippet}

  {#snippet blocked()}
    {#if uc.status === "blocked"}
      <div class="callout">{@render area("blocked_by", "Blocked by", uc.blocked_by, "What or who blocks it?")}</div>
    {/if}
  {/snippet}

  {#snippet texts()}
    {@render area("current_state", "Current state", uc.current_state, "Where does it stand?")}
    {@render area("next_step", "Next step", uc.next_step, "What happens next?")}
  {/snippet}

  {#snippet moveButtons()}
    {#if moves.length}
      <span class="w-caps">{step?.kind === "gate" ? "Decision" : "Next"}</span>
      {#each moves as m (m.to)}
        <!-- Page: quiet, so current state and next step keep the attention. -->
        <button
          type="button"
          class="w-btn"
          class:w-btn--primary={!m.neg && mode !== "page"}
          class:w-btn--quiet={mode === "page"}
          class:pos={!m.neg}
          onclick={() => onmove(u, m.to)}>{m.verdict} → {m.text}</button
        >
      {/each}
    {/if}
  {/snippet}

  {#snippet classification()}
    <dl class="kv">
        <dt><label for="f-step">Step</label></dt>
        <dd>
          {#key uc.step}
            <select id="f-step" value={uc.step ?? ""} onchange={(e) => onmove(u, e.currentTarget.value)}>
              {#if !step}<option value={uc.step ?? ""}>–</option>{/if}
              {#each p.phases as ph (ph.id)}
                <optgroup label={ph.name}>
                  {#each p.steps.filter((s) => s.phase === ph.id && s.kind !== "term") as s (s.id)}<option value={s.id}>{stepName(s)}</option>{/each}
                </optgroup>
              {/each}
            </select>
          {/key}
        </dd>
        <dt><label for="f-area">Area</label></dt>
        <dd>
          <input id="f-area" list="f-area-list" value={uc.area ?? ""} placeholder="–" autocomplete="off" onchange={text("usecase.area")} />
          <datalist id="f-area-list">{#each areas as a (a)}<option value={a}></option>{/each}</datalist>
        </dd>
        <dt><label for="f-type">Type</label></dt>
        <dd>
          <select id="f-type" value={uc.type ?? ""} onchange={(e) => save("usecase.type", e.currentTarget.value)}>
            {#if !uc.type}<option value="">–</option>{/if}
            {#each p.types as t (t.id)}<option value={t.id}>{t.label}</option>{/each}
          </select>
        </dd>
      </dl>
  {/snippet}

  {#snippet taskList()}
    <section>
      <h3 class="w-caps">Tasks <span class="w-mono prog">{doneCount}/{tasks.length}</span></h3>
      {#if tasks.length}<div class="bar"><i style:width="{(doneCount / tasks.length) * 100}%"></i></div>{/if}
      <ul class="tasks">
        {#each tasks as t (t.id)}
          <li class:done={t.status === "done"}>
            <input type="checkbox" id="t-{t.id}" checked={t.status === "done"} onchange={(e) => updateTaskField(t.id, "status", e.currentTarget.checked ? "done" : reopen)} />
            <label for="t-{t.id}"><span class="w-mono">{t.id}</span> {t.title}</label>
          </li>
        {/each}
      </ul>
      <input class="add-task" bind:value={newTask} placeholder="+ Add task" aria-label="Add task" onkeydown={(e) => e.key === "Enter" && addTask()} />
    </section>
  {/snippet}

  {#snippet side()}
    <section>
      <h3 class="w-caps">Classification</h3>
      {@render classification()}
    </section>
    {#if p.assessment}
      <section>
        <h3 class="w-caps">Assessment</h3>
        {#if assessed}
          <AssessmentPill result={assessed} />
        {:else}
          <span class="sum">Not assessed · <a href={pageHref}>Open page</a></span>
        {/if}
      </section>
    {/if}
    <!-- Light version: the total only; the list and the editor are on the page. -->
    <section>
      <h3 class="w-caps">Manual effort</h3>
      {#if uc.savings.length}
        <span class="sum" use:tip={fteTip(u)}><b>≈ {fmtFte(hours / fteHours())} FTE</b> · {fmtHours(hours)} h/yr</span>
      {:else}
        <span class="sum">No effort recorded · <a href={pageHref}>Open page</a></span>
      {/if}
    </section>
    {@render taskList()}
    <section>
      <h3 class="w-caps">Decisions <span class="w-mono prog">{uc.decisions.length}</span></h3>
      {#each uc.decisions as d, i (i)}
        <div class="decision">
          <span class="w-mono">{[d.date, d.gate].filter(Boolean).join(" · ")}</span>{d.text}
        </div>
      {:else}
        <p class="w-sub">None yet.</p>
      {/each}
    </section>
  {/snippet}

  {#if mode === "page"}
    <!-- Page: process flow on the left (status, texts, gates, history, tasks), content on the right. -->
    <div class="page">
      <div class="flow-col">
        <div class="card flow">
          {@render statuses()}
          {@render blocked()}
          {@render texts()}
          {#if moves.length}<div class="moves">{@render moveButtons()}</div>{/if}
          <div class="sec">{@render taskList()}</div>
        </div>
        <!-- The one part that keeps growing: its own card, scrolling inside. -->
        <section class="card flow proc"><ProcessHistory {u} process={p} /></section>
      </div>
      <div class="content">
        <div class="card desc">
          <div class="cls">{@render classification()}</div>
          {@render description?.()}
        </div>
        <!-- Assessment and effort: always in the same place below the description, however long that gets. -->
        {#if p.assessment}
          {@const a = uc.assessment}
          <section class="card effort" aria-label="Assessment">
            <div class="eff-sum">
              <span class="w-caps">Assessment</span>
              {#if assessed && a}
                <AssessmentPill result={assessed} />
                <span class="w-sub">{scoreLine(assessed)}</span>
                {#if a.date}<span class="w-sub">{a.date}</span>{/if}
              {:else}
                <span class="w-sub">Not assessed</span>
              {/if}
            </div>
            <div class="eff-list">
              {#if assessed}
                {#each assessed.failed as f (f)}<p class="ko">K.O.: {f}</p>{/each}
                {#if assessed.open.length}
                  <span class="w-caps sub-h">Open for pilot</span>
                  {#each assessed.open.slice(0, OPEN_MAX) as o (o)}<div class="eff-row"><span>{o}</span></div>{/each}
                  {#if assessed.open.length > OPEN_MAX}<span class="w-sub more">+{assessed.open.length - OPEN_MAX}</span>{/if}
                {/if}
                {#if a?.note}<p class="w-sub">{a.note}</p>{/if}
              {/if}
            </div>
            <button type="button" class="w-btn" onclick={() => (editingAssessment = true)}>{a ? "Re-assess…" : "Assess…"}</button>
          </section>
        {/if}
        <section class="card effort" aria-label="Manual effort">
          <div class="eff-sum">
            <span class="w-caps">Manual effort today</span>
            {#if uc.savings.length}
              <b class="big" use:tip={fteTip(u)}>≈ {fmtFte(hours / fteHours())} FTE</b>
              <span class="w-sub">{fmtHours(hours)} h/yr</span>
            {:else}
              <span class="w-sub">No effort recorded</span>
            {/if}
          </div>
          <div class="eff-list">
            {#each uc.savings as s, i (i)}
              {@const h = savingHours(s)}
              <div class="eff-row"><span>{s.what ?? "–"}</span><span class="w-mono">{h == null ? "–" : `${fmtHours(h)} h`}</span></div>
            {/each}
            {#if uc.savings_note}<p class="w-sub">{uc.savings_note}</p>{/if}
          </div>
          <button type="button" class="w-btn" onclick={() => (editingEffort = true)}>{uc.savings.length ? "Edit…" : "Add…"}</button>
        </section>
      </div>
    </div>
    {#if editingEffort}<EffortEditor {key} onclose={() => (editingEffort = false)} />{/if}
    {#if editingAssessment && p.assessment}<AssessmentEditor {key} process={p} onclose={() => (editingAssessment = false)} />{/if}
  {:else}
    <!-- Popup and side panel: a click on the scrim only closes, it never opens what lies beneath. -->
    <div class="w-scrim" onclick={onclose} aria-hidden="true"></div>
    <div
      class={mode === "popup" ? "w-modal" : "panel"}
      role="dialog"
      aria-modal={mode === "popup"}
      aria-labelledby="u-title"
    >
      <div class="d-top">
        <div class="d-meta">
          <span class="w-mono">{u.key}</span>
          {#if uc.type}<span class="w-type" class:w-type--ki={uc.type === "ai"} class:hybrid={uc.type === "hybrid"}>{label(p.types, uc.type)}</span>{/if}
          {#if uc.area}<span class="pill">{uc.area}</span>{/if}
          <button type="button" class="w-btn w-btn--quiet open" title="Open page (⌘↩)" onclick={() => goto(pageHref)}>Open page ↗</button>
          <button type="button" class="x" onclick={onclose} aria-label="Close">✕</button>
        </div>
        {#key u.title}
          <input
            id="u-title"
            class="d-title"
            value={u.title}
            aria-label="Title"
            onchange={(e) => (e.currentTarget.value.trim() ? save("title", e.currentTarget.value.trim()) : (e.currentTarget.value = u.title))}
          />
        {/key}
        {@render position()}
        {@render statuses()}
      </div>

      <div class="d-body">
        {@render blocked()}
        <div class="d-main">
          {@render texts()}
        </div>
        <aside class="d-side">{@render side()}</aside>
      </div>

      {#if moves.length}<div class="d-foot">{@render moveButtons()}</div>{/if}
    </div>
  {/if}
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
  @keyframes slide {
    from {
      transform: translateX(16px);
      opacity: 0;
    }
  }
  .panel .d-top {
    padding-top: 36px;
  }
  .d-top {
    padding: 16px 22px 14px;
    border-bottom: 1px solid var(--w-line);
    display: grid;
    gap: 10px;
  }
  .d-meta {
    display: flex;
    align-items: center;
    gap: var(--w-s-2);
    color: var(--w-muted);
  }
  .w-type.hybrid {
    color: var(--w-accent);
    box-shadow: inset 0 0 0 1px var(--w-accent);
  }
  .pill {
    font-size: var(--w-fs-caption);
    color: var(--w-muted);
    box-shadow: inset 0 0 0 1px var(--w-line);
    border-radius: var(--w-r-pill);
    padding: 0 8px;
  }
  .x {
    width: 30px;
    height: 30px;
    border: 0;
    border-radius: var(--w-r-sm);
    background: none;
    color: var(--w-muted);
    cursor: pointer;
  }
  .x:hover {
    background: var(--w-tray);
    color: var(--w-ink);
  }
  .d-title {
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
  .d-title:hover,
  .d-title:focus {
    outline: none;
    background: var(--w-sunk);
  }
  .d-pos {
    display: grid;
    gap: 6px;
  }
  .track {
    list-style: none;
    margin: 0;
    padding: 0;
    display: flex;
  }
  .track li {
    flex: 1;
    min-width: 0;
    display: flex;
    flex-direction: column;
    align-items: center;
    gap: 4px;
    position: relative;
    font-size: var(--w-fs-micro);
    color: var(--w-muted);
  }
  .track li::before {
    content: "";
    position: absolute;
    top: 5px;
    left: -50%;
    right: 50%;
    height: 2px;
    background: var(--w-line);
  }
  .track li:first-child::before {
    display: none;
  }
  .track i {
    position: relative;
    width: 12px;
    height: 12px;
    box-sizing: border-box;
    border-radius: 50%;
    border: 2px solid var(--w-line);
    background: var(--w-surface);
  }
  .track li.opt i {
    border-style: dashed;
  }
  .track li.done i {
    background: var(--w-accent);
    border-color: var(--w-accent);
  }
  .track li.done::before,
  .track li.cur::before {
    background: var(--w-accent);
  }
  .track li.cur i {
    border-color: var(--w-accent);
    box-shadow: 0 0 0 4px var(--w-accent-soft);
  }
  .track li.cur span {
    color: var(--w-ink);
    font-weight: 600;
  }
  .track span {
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
    max-width: 100%;
  }
  .parked-badge {
    justify-self: start;
    font-family: var(--w-font-label);
    font-weight: 600;
    font-size: var(--w-fs-caption);
    color: var(--w-muted);
    border: 1px dashed var(--w-muted);
    border-radius: var(--w-r-sm);
    padding: 1px 8px;
  }
  .where {
    font-size: var(--w-fs-small);
    color: var(--w-muted);
  }
  .where b {
    color: var(--w-ink);
    font-weight: 500;
  }
  .stale {
    color: var(--w-warn);
    font-weight: 500;
  }
  .status-row {
    display: flex;
    flex-wrap: wrap;
    gap: 6px;
  }
  .status-row button {
    display: inline-flex;
    align-items: center;
    gap: 6px;
    border: 0;
    background: var(--w-surface);
    box-shadow: var(--w-shadow-raised);
    border-radius: var(--w-r-pill);
    padding: 3px 11px 3px 9px;
    font-size: var(--w-fs-small);
    cursor: pointer;
    color: var(--w-muted);
  }
  .status-row button[aria-pressed="true"] {
    color: var(--w-ink);
    font-weight: 600;
    box-shadow: inset 0 0 0 1.5px var(--w-ink);
  }
  .w-modal {
    width: min(var(--w-uc-detail-w), calc(100% - 32px));
    min-height: min(var(--w-uc-detail-min-h), calc(100% - 48px));
  }
  .d-body {
    flex: 1;
    overflow-y: auto;
    padding: 18px 22px 20px;
    display: grid;
    grid-template-columns: minmax(0, 1.5fr) minmax(0, 1fr);
    gap: 18px 28px;
    align-content: start;
  }
  .panel .d-body {
    grid-template-columns: minmax(0, 1fr);
  }
  .callout {
    grid-column: 1 / -1;
    background: var(--w-danger-soft);
    box-shadow: inset 0 0 0 1px var(--w-danger);
    border-radius: var(--w-r-md);
    padding: 10px 12px;
  }
  .callout :global(label) {
    color: var(--w-danger);
  }
  .d-main {
    display: grid;
    gap: 16px;
    align-content: start;
    min-width: 0;
  }
  .txt {
    display: grid;
    gap: 3px;
  }
  #f-next_step {
    font-size: var(--w-fs-title);
    font-weight: 500;
  }
  textarea {
    width: 100%;
    box-sizing: border-box;
    min-height: 1.9em;
    border: 1px solid transparent;
    border-radius: var(--w-r-sm);
    padding: 4px 6px;
    margin-left: -7px;
    background: none;
    resize: none;
    overflow: hidden;
    line-height: 1.45;
    font-size: var(--w-fs-body);
    user-select: text;
    -webkit-user-select: text;
  }
  textarea:hover {
    border-color: var(--w-line);
  }
  textarea:focus {
    border-color: var(--w-accent);
    background: var(--w-surface);
    outline: none;
  }
  .sum {
    font-size: var(--w-fs-small);
    color: var(--w-muted);
    justify-self: start;
  }
  .sum b {
    color: var(--w-ink);
    font-weight: 600;
  }
  .sum a {
    color: var(--w-accent);
  }
  .d-side {
    display: grid;
    gap: 18px;
    align-content: start;
    min-width: 0;
    border-left: 1px solid var(--w-line);
    padding-left: 24px;
  }
  .panel .d-side {
    border-left: 0;
    padding-left: 0;
    border-top: 1px solid var(--w-line);
    padding-top: var(--w-s-4);
  }
  .d-side section {
    display: grid;
    gap: 8px;
  }
  .d-side h3 {
    display: flex;
    justify-content: space-between;
    margin: 0;
  }
  .prog {
    font-weight: 400;
    letter-spacing: 0;
    color: var(--w-muted);
  }
  .bar {
    height: 4px;
    border-radius: var(--w-r-sm);
    background: var(--w-tray);
    overflow: hidden;
  }
  .bar i {
    display: block;
    height: 100%;
    background: var(--w-accent);
  }
  .tasks {
    display: grid;
    gap: 6px;
    margin: 0;
    padding: 0;
    list-style: none;
  }
  .tasks li {
    display: flex;
    gap: 8px;
    align-items: flex-start;
    font-size: var(--w-fs-small);
    line-height: 1.35;
  }
  .tasks input {
    margin: 2px 0 0;
    flex: none;
  }
  .tasks .w-mono {
    color: var(--w-muted);
  }
  .tasks li.done label {
    text-decoration: line-through;
    color: var(--w-muted);
  }
  .add-task {
    border: 0;
    background: var(--w-tray);
    border-radius: var(--w-r-sm);
    padding: 5px 8px;
    font-size: var(--w-fs-small);
  }
  .add-task:focus {
    outline: none;
    box-shadow: 0 0 0 2px var(--w-accent-soft);
  }
  .kv {
    display: grid;
    grid-template-columns: auto 1fr;
    gap: 4px 12px;
    margin: 0;
    align-items: center;
  }
  .kv dt {
    font-size: var(--w-fs-small);
    color: var(--w-muted);
  }
  .kv dd {
    margin: 0;
    min-width: 0;
  }
  .kv select,
  .kv input {
    width: 100%;
    border: 1px solid transparent;
    border-radius: var(--w-r-sm);
    padding: 3px 4px;
    background: none;
    font-size: var(--w-fs-small);
    cursor: pointer;
  }
  .kv select:hover,
  .kv select:focus,
  .kv input:hover,
  .kv input:focus {
    border-color: var(--w-line);
  }
  .decision {
    background: var(--w-sunk);
    border-radius: var(--w-r-md);
    padding: 6px 10px;
    font-size: var(--w-fs-small);
    line-height: 1.4;
  }
  .decision .w-mono {
    display: block;
    color: var(--w-muted);
  }
  .d-foot {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: 8px;
    padding: 12px 22px;
    border-top: 1px solid var(--w-line);
    background: var(--w-sunk);
  }
  .d-foot .w-caps {
    margin-right: 4px;
  }
  .open {
    margin-left: auto;
  }
  /* Page: fills the tab below the bar like Overview; every card scrolls on its own. */
  .page {
    flex: 1;
    min-height: 300px;
    display: grid;
    grid-template-columns: 420px minmax(0, 1fr);
    grid-template-rows: minmax(0, 1fr);
    gap: var(--w-s-4);
  }
  .card {
    background: var(--w-surface);
    border-radius: var(--w-r-lg);
    box-shadow: var(--w-shadow-panel);
    overflow-y: auto;
  }
  .flow-col {
    display: flex;
    flex-direction: column;
    gap: var(--w-s-4);
    min-height: 0;
  }
  .flow {
    flex: 0 1 auto;
    min-height: 0;
    padding: var(--w-s-5) 22px;
    display: flex;
    flex-direction: column;
    gap: 14px;
  }
  .flow.proc {
    flex: 1 1 200px;
    min-height: 160px;
    gap: 8px;
  }
  .flow .status-row {
    gap: 4px;
  }
  .flow .status-row button {
    padding: 3px 9px 3px 7px;
    font-size: var(--w-fs-caption);
  }
  .flow .callout {
    flex: none;
  }
  .sec,
  .sec section {
    display: grid;
    gap: 8px;
  }
  .sec {
    border-top: 1px solid var(--w-line);
    padding-top: 14px;
  }
  .sec h3 {
    display: flex;
    justify-content: space-between;
    margin: 0;
  }
  .moves {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: 4px;
  }
  .moves .w-btn {
    padding: 3px 8px;
    font-size: var(--w-fs-caption);
  }
  .moves .w-btn.pos {
    color: var(--w-accent);
  }
  .content {
    display: flex;
    flex-direction: column;
    gap: var(--w-s-4);
    min-height: 0;
  }
  .desc {
    flex: 1;
    min-height: 0;
    padding: var(--w-s-4) 30px var(--w-s-6);
    display: flex;
    flex-direction: column;
    gap: var(--w-s-4);
  }
  .cls .kv {
    grid-template-columns: auto minmax(0, 1.4fr) auto minmax(0, 1fr) auto minmax(0, 1fr);
    padding-bottom: var(--w-s-3);
    border-bottom: 1px solid var(--w-line);
  }
  /* Natural height, at most a third of the column; a long list scrolls inside. */
  .effort {
    flex: none;
    max-height: 33%;
    padding: var(--w-s-4) 30px;
    display: grid;
    grid-template-columns: 170px minmax(0, 1fr) auto;
    gap: 6px var(--w-s-6);
    align-items: start;
  }
  .eff-sum {
    display: grid;
    gap: 2px;
  }
  .big {
    justify-self: start;
    font-family: var(--w-font-label);
    font-size: var(--w-fs-heading);
    font-weight: 600;
  }
  .eff-list {
    display: grid;
    min-width: 0;
  }
  .eff-row {
    display: flex;
    justify-content: space-between;
    gap: var(--w-s-3);
    padding: 5px 0;
    border-bottom: 1px solid var(--w-line);
    font-size: var(--w-fs-small);
  }
  .eff-row .w-mono {
    color: var(--w-muted);
    white-space: nowrap;
  }
  .eff-list p {
    margin: 6px 0 0;
  }
  .eff-list .sub-h {
    margin-top: 2px;
  }
  .eff-list .ko {
    margin: 0 0 6px;
    font-size: var(--w-fs-small);
    font-weight: 500;
    color: var(--w-danger);
  }
  .more {
    padding-top: 4px;
  }
  @media (max-width: 680px) {
    .page {
      grid-template-columns: 1fr;
      grid-template-rows: none;
    }
    .effort {
      grid-template-columns: 1fr;
    }
    .d-body {
      grid-template-columns: 1fr;
    }
    .d-side {
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
