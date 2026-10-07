<script lang="ts">
  import { goto } from "$app/navigation";
  import { grow } from "$lib/grow";
  import { renderMarkdown } from "$lib/markdown";
  import { workspace, readMarkdown, updateProjectField, updateTaskField, createTask, type Process, type Saving } from "$lib/stores/workspace.svelte";
  import {
    daysInStep,
    fmtFte,
    fmtHours,
    fteHours,
    isStale,
    knownAreas,
    label,
    nextMoves,
    PER_YEAR,
    phaseIndex,
    savedHours,
    savingHours,
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
  }: { key: string; process: Process; mode: "popup" | "panel"; onmove: (u: UC, step: string) => void; onclose: () => void } = $props();

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
    if (workspace.index && !u) onclose();
  });

  // Description = the body of _project.md, re-read on every index change.
  let body = $state("");
  $effect(() => {
    const path = u?.path;
    void workspace.index;
    if (path) readMarkdown(`${path}/_project.md`).then((b) => (body = b), (e) => (workspace.error = String(e)));
  });

  async function save(field: string, value: unknown) {
    try {
      await updateProjectField(key, field, value);
    } catch (e) {
      workspace.error = String(e);
    }
  }
  const text = (field: string) => (e: Event & { currentTarget: HTMLTextAreaElement | HTMLInputElement }) =>
    save(field, e.currentTarget.value.trim() || null);

  // Savings: the whole list is written on every change. A new row stays a local draft until it is complete.
  const PERS = Object.keys(PER_YEAR);
  let draft = $state<Saving | null>(null);
  const complete = (s: Saving) => !!s.what?.trim() && s.count != null && s.count >= 0 && s.minutes != null && s.minutes >= 0 && !!s.per;
  const hours = $derived(u ? savedHours(u) : 0);
  const saveSavings = (list: Saving[]) => save("usecase.savings", list);
  /** Cell edit of row `i` (or the draft when i < 0). Invalid input falls back to the file's value. */
  function cell(i: number, field: "what" | "count" | "per" | "minutes") {
    return (e: Event & { currentTarget: HTMLInputElement | HTMLSelectElement }) => {
      const el = e.currentTarget;
      const value = field === "what" || field === "per" ? el.value.trim() : el.value === "" ? null : Number(el.value);
      const row = { ...(i < 0 ? draft! : uc!.savings[i]), [field]: value };
      if (i < 0) {
        draft = row;
        if (complete(row)) {
          saveSavings([...uc!.savings, row]);
          draft = null;
        }
      } else if (complete(row)) {
        saveSavings(uc!.savings.map((s, j) => (j === i ? row : s)));
      } else {
        el.value = String(uc!.savings[i][field] ?? "");
      }
    };
  }

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

{#if u && uc}
  {#if mode === "popup"}<div class="w-scrim" onclick={onclose} aria-hidden="true"></div>{/if}
  <div class={mode === "popup" ? "w-modal" : "panel"} role="dialog" aria-modal={mode === "popup"} aria-labelledby="u-title">
    <div class="d-top">
      <div class="d-meta">
        <span class="w-mono">{u.key}</span>
        {#if uc.type}<span class="w-type" class:w-type--ki={uc.type === "ai"} class:hybrid={uc.type === "hybrid"}>{label(p.types, uc.type)}</span>{/if}
        {#if uc.area}<span class="pill">{uc.area}</span>{/if}
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
            Step <b>{stepName(step)}</b> · Ball with <b>{lane?.label ?? step.lane}</b>
          {:else}
            <span class="stale">Unknown step “{uc.step ?? "none"}”, pick one below</span>
          {/if}
          {#if days !== null}· <span class:stale={isStale(p, u)}>{days} {days === 1 ? "day" : "days"} in step</span>{/if}
        </div>
      </div>
      <div class="status-row" role="group" aria-label="Status">
        {#each p.statuses as s (s.id)}
          <button type="button" aria-pressed={uc.status === s.id} onclick={() => save("usecase.status", s.id)}
            ><span class="w-dot" class:w-dot--hollow={s.id === "on_hold"} style:--c={statusColor(s.id)}></span>{s.label}</button
          >
        {/each}
      </div>
    </div>

    <div class="d-body">
      {#if uc.status === "blocked"}
        <div class="callout">{@render area("blocked_by", "Blocked by", uc.blocked_by, "What or who blocks it?")}</div>
      {/if}
      <div class="d-main">
        {@render area("next_step", "Next step", uc.next_step, "What happens next?")}
        {@render area("current_state", "Current state", uc.current_state, "Where does it stand?")}
        <section class="savings">
          <span class="w-caps">Savings</span>
          {#if uc.savings.length || draft}
            <table>
              <thead>
                <tr><th>Activity</th><th class="num">Count</th><th>Per</th><th class="num">Minutes</th><th class="num">h/yr</th><th></th></tr>
              </thead>
              <tbody>
                {#each [...uc.savings, ...(draft ? [draft] : [])] as s, i (i)}
                  {@const r = i < uc.savings.length ? i : -1}
                  {@const h = savingHours(s)}
                  <!-- {#key} resets the inputs to the file's values after every reload. -->
                  {#key workspace.reloads}
                    <tr>
                      <td><input value={s.what ?? ""} placeholder="What is done by hand?" aria-label="Activity" onchange={cell(r, "what")} {@attach (el) => { if (r < 0 && !s.what) el.focus(); }} /></td>
                      <td class="num"><input type="number" min="0" step="any" value={s.count ?? ""} aria-label="Count" onchange={cell(r, "count")} /></td>
                      <td>
                        <select value={s.per ?? ""} aria-label="Per" onchange={cell(r, "per")}>
                          {#if !s.per}<option value="">–</option>{/if}
                          {#each PERS as per (per)}<option value={per}>{per}</option>{/each}
                        </select>
                      </td>
                      <td class="num"><input type="number" min="0" step="any" value={s.minutes ?? ""} aria-label="Minutes" onchange={cell(r, "minutes")} /></td>
                      <td class="num w-mono">{h == null ? "–" : fmtHours(h)}</td>
                      <td>
                        <button type="button" class="rm" aria-label="Remove activity" onclick={() => (r < 0 ? (draft = null) : saveSavings(uc.savings.filter((_, j) => j !== r)))}>✕</button>
                      </td>
                    </tr>
                  {/key}
                {/each}
              </tbody>
            </table>
            <div class="total">
              <span><b>≈ {fmtFte(hours / fteHours())} FTE</b> · {fmtHours(hours)} h/yr</span>
              <button type="button" class="w-btn w-btn--quiet" onclick={() => (draft ??= { what: "", count: null, per: "month", minutes: null })}>+ Add activity</button>
            </div>
            <p class="w-sub hint">
              Per year: month × {PER_YEAR.month}, week × {PER_YEAR.week}, day × {PER_YEAR.day} · 1 FTE = {fmtHours(fteHours())} h (Settings)
            </p>
          {:else}
            <button type="button" class="add-act" onclick={() => (draft = { what: "", count: null, per: "month", minutes: null })}>+ Add activity</button>
          {/if}
          {#if uc.savings.length || uc.savings_note}
            {#key uc.savings_note}
              <input class="note" value={uc.savings_note ?? ""} placeholder="Where do the numbers come from?" aria-label="Savings note" onchange={text("usecase.savings_note")} />
            {/key}
          {/if}
        </section>
        <section class="txt">
          <span class="w-caps">Description</span>
          {#if body.trim()}
            <!-- Sanitised by DOMPurify in renderMarkdown. -->
            <div class="md">{@html renderMarkdown(body)}</div>
          {:else}
            <p class="w-sub">No description in _project.md.</p>
          {/if}
        </section>
      </div>
      <aside class="d-side">
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
        <section>
          <h3 class="w-caps">Classification</h3>
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
        </section>
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
      </aside>
    </div>

    <div class="d-foot">
      {#if moves.length}
        <span class="w-caps">{step?.kind === "gate" ? "Decision" : "Next"}</span>
        {#each moves as m (m.to)}
          <button type="button" class="w-btn" class:w-btn--primary={!m.neg} onclick={() => onmove(u, m.to)}>{m.verdict} → {m.text}</button>
        {/each}
      {/if}
      <button type="button" class="w-btn w-btn--quiet open" onclick={() => goto(`/projects/${u.key}`)}>Open project</button>
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
    margin-left: auto;
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
  .d-body {
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
  .txt:first-child textarea {
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
  .savings {
    display: grid;
    gap: 6px;
  }
  .savings table {
    width: 100%;
    border-collapse: collapse;
    font-size: var(--w-fs-small);
  }
  .savings th {
    text-align: left;
    font-weight: 500;
    font-size: var(--w-fs-caption);
    color: var(--w-muted);
    padding: 0 4px 2px;
    border-bottom: 1px solid var(--w-line);
  }
  .savings td {
    padding: 1px 0;
    border-bottom: 1px solid var(--w-line);
  }
  .savings .num {
    text-align: right;
    width: 64px;
  }
  .savings td.w-mono {
    padding-right: 4px;
    color: var(--w-muted);
  }
  .savings td:last-child {
    width: 24px;
  }
  .savings input,
  .savings select,
  .note {
    width: 100%;
    box-sizing: border-box;
    border: 1px solid transparent;
    border-radius: var(--w-r-sm);
    padding: 3px 4px;
    background: none;
    font-size: var(--w-fs-small);
    font-variant-numeric: tabular-nums;
  }
  .savings .num input {
    text-align: right;
  }
  .savings input:hover,
  .savings select:hover,
  .note:hover {
    border-color: var(--w-line);
  }
  .savings input:focus,
  .savings select:focus,
  .note:focus {
    border-color: var(--w-accent);
    background: var(--w-surface);
    outline: none;
  }
  .rm {
    border: 0;
    background: none;
    color: var(--w-muted);
    cursor: pointer;
    border-radius: var(--w-r-sm);
    width: 22px;
    height: 22px;
  }
  .rm:hover {
    background: var(--w-tray);
    color: var(--w-ink);
  }
  .total {
    display: flex;
    align-items: center;
    justify-content: space-between;
    font-size: var(--w-fs-small);
    color: var(--w-muted);
  }
  .total b {
    color: var(--w-ink);
    font-weight: 600;
  }
  .hint {
    margin: 0;
    font-size: var(--w-fs-caption);
  }
  .add-act {
    justify-self: start;
    border: 0;
    background: var(--w-tray);
    border-radius: var(--w-r-sm);
    padding: 5px 8px;
    font-size: var(--w-fs-small);
    color: var(--w-muted);
    cursor: pointer;
  }
  .add-act:hover {
    color: var(--w-ink);
  }
  .note {
    margin-left: -4px;
    color: var(--w-muted);
  }
  .md {
    font-size: var(--w-fs-body);
    user-select: text;
    -webkit-user-select: text;
  }
  .md :global(:first-child) {
    margin-top: 0;
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
  @media (max-width: 680px) {
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
