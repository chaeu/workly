<script lang="ts">
  import type { Process } from "$lib/stores/workspace.svelte";
  import { daysInStep, isStale, label, phaseIndex, phaseOf, statusColor, stepName, stepOf, type UC } from "$lib/usecases.svelte";

  let { process: p, ucs, onopen }: { process: Process; ucs: UC[]; onopen: (key: string) => void } = $props();

  // Step sorts by position in the process: phase, then column, then key.
  const stepRank = (u: UC) => {
    const s = stepOf(p, u.usecase.step);
    const ph = phaseIndex(p, phaseOf(p, u));
    return (ph < 0 ? 999 : ph) * 1000 + (s?.col ?? 999);
  };
  const COLS = [
    ["key", "ID", (u: UC) => u.key],
    ["title", "Title", (u: UC) => u.title.toLowerCase()],
    ["type", "Type", (u: UC) => label(p.types, u.usecase.type)],
    ["area", "Area", (u: UC) => u.usecase.area ?? "~"],
    ["step", "Phase / step", stepRank],
    ["status", "Status", (u: UC) => p.statuses.findIndex((s) => s.id === u.usecase.status)],
    ["days", "Days", (u: UC) => daysInStep(u) ?? -1],
  ] as const;
  type Col = (typeof COLS)[number][0];

  let sort = $state<{ col: Col; dir: 1 | -1 }>({ col: "step", dir: 1 });
  const rows = $derived.by(() => {
    const get = COLS.find(([c]) => c === sort.col)![2] as (u: UC) => string | number;
    return [...ucs].sort((a, b) => {
      const [x, y] = [get(a), get(b)];
      const c = typeof x === "number" ? x - (y as number) : x.localeCompare(y as string);
      return (c || a.key.localeCompare(b.key, undefined, { numeric: true })) * sort.dir;
    });
  });
  const by = (col: Col) => (sort = { col, dir: sort.col === col ? (-sort.dir as 1 | -1) : 1 });
</script>

<div class="list">
  <table>
    <thead>
      <tr>
        {#each COLS as [col, text] (col)}
          <th aria-sort={sort.col === col ? (sort.dir === 1 ? "ascending" : "descending") : "none"} class:num={col === "days"}>
            <button type="button" onclick={() => by(col)}>{text}{#if sort.col === col}<span class="arrow">{sort.dir === 1 ? "↑" : "↓"}</span>{/if}</button>
          </th>
        {/each}
        <th>Next step</th>
      </tr>
    </thead>
    <tbody>
      {#each rows as u (u.key)}
        {@const st = stepOf(p, u.usecase.step)}
        {@const ph = p.phases.find((x) => x.id === phaseOf(p, u))}
        {@const status = u.usecase.status}
        {@const days = daysInStep(u)}
        {@const next = status === "blocked" && u.usecase.blocked_by ? u.usecase.blocked_by : u.usecase.next_step}
        <!-- svelte-ignore a11y_click_events_have_key_events, a11y_no_noninteractive_element_interactions -->
        <tr class:blocked={status === "blocked"} class:hold={status === "on_hold"} onclick={() => onopen(u.key)}>
          <td class="w-mono id">{u.key}</td>
          <!-- The title button carries keyboard access; a mouse click anywhere on the row opens it too. -->
          <td class="title"><button type="button">{u.title}</button></td>
          <td>
            {#if u.usecase.type}<span class="w-type" class:w-type--ki={u.usecase.type === "ai"} class:hybrid={u.usecase.type === "hybrid"}
                >{label(p.types, u.usecase.type)}</span
              >{/if}
          </td>
          <td class="muted">{u.usecase.area ?? "–"}</td>
          <td>
            <span class="phase">{ph?.name ?? "–"}</span>
            <!-- Like the board card: the step shows only when it says more than the phase. -->
            {#if !st}<span class="step warn">{u.usecase.step ?? "no step"}</span>
            {:else if st.kind === "gate" || !ph || (st.id !== p.phase_default_step[ph.id] && st.label !== ph.name)}<span class="step" class:gate={st.kind === "gate"}
                >{#if st.kind === "gate"}<i></i>{/if}{stepName(st)}</span
              >{/if}
          </td>
          <td>
            <span class="status"
              ><span class="w-dot" class:w-dot--hollow={status === "on_hold"} style:--c={statusColor(status)}></span>{label(p.statuses, status)}</span
            >
          </td>
          <td class="num w-mono" class:stale={isStale(p, u)}>{days ?? "–"}</td>
          <td class="next">{next ?? ""}</td>
        </tr>
      {:else}
        <tr><td class="empty" colspan={COLS.length + 1}>No use case matches the filters.</td></tr>
      {/each}
    </tbody>
  </table>
</div>

<style>
  .list {
    overflow: auto;
    border-radius: var(--w-r-lg);
    background: var(--w-surface);
    box-shadow: var(--w-shadow-panel);
  }
  table {
    width: 100%;
    border-collapse: collapse;
    font-size: var(--w-fs-small);
  }
  th {
    position: sticky;
    top: 0;
    z-index: 1;
    background: var(--w-tray);
    text-align: left;
    padding: 0;
    border-bottom: 1px solid var(--w-line);
    white-space: nowrap;
  }
  th button {
    all: unset;
    box-sizing: border-box;
    width: 100%;
    padding: 9px 12px;
    cursor: pointer;
    font-family: var(--w-font-label);
    font-size: var(--w-fs-caption);
    font-weight: 600;
    text-transform: uppercase;
    letter-spacing: 0.06em;
    color: var(--w-muted);
  }
  th:last-child {
    padding: 9px 12px;
    font-family: var(--w-font-label);
    font-size: var(--w-fs-caption);
    font-weight: 600;
    text-transform: uppercase;
    letter-spacing: 0.06em;
    color: var(--w-muted);
  }
  th button:hover,
  th[aria-sort="ascending"] button,
  th[aria-sort="descending"] button {
    color: var(--w-ink);
  }
  th button:focus-visible {
    outline: 2px solid var(--w-accent);
    outline-offset: -2px;
  }
  .arrow {
    margin-left: 4px;
  }
  td {
    padding: 8px 12px;
    border-bottom: 1px solid var(--w-line);
    vertical-align: top;
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
  .id,
  .muted {
    color: var(--w-muted);
    white-space: nowrap;
  }
  .title {
    min-width: 200px;
  }
  .title button {
    all: unset;
    font-weight: 500;
    cursor: pointer;
  }
  .title button:focus-visible {
    outline: 2px solid var(--w-accent);
    outline-offset: 2px;
    border-radius: var(--w-r-sm);
  }
  .w-type.hybrid {
    color: var(--w-accent);
    box-shadow: inset 0 0 0 1px var(--w-accent);
  }
  .phase {
    display: block;
    font-family: var(--w-font-label);
    font-weight: 600;
    white-space: nowrap;
  }
  .step {
    display: flex;
    align-items: center;
    gap: 5px;
    color: var(--w-muted);
    font-size: var(--w-fs-caption);
    white-space: nowrap;
  }
  .step.gate {
    color: var(--w-accent);
  }
  .step.warn {
    color: var(--w-warn);
  }
  .step i {
    flex: none;
    width: 7px;
    height: 7px;
    transform: rotate(45deg);
    border: 1.5px solid currentColor;
  }
  .status {
    display: flex;
    align-items: center;
    gap: 6px;
    white-space: nowrap;
  }
  .num {
    text-align: right;
    color: var(--w-muted);
  }
  th.num button {
    text-align: right;
  }
  td.stale {
    color: var(--w-warn);
    font-weight: 500;
  }
  .next {
    color: var(--w-muted);
    font-size: var(--w-fs-caption);
    min-width: 180px;
  }
  tr.blocked .status,
  tr.blocked .next {
    color: var(--w-danger);
    font-weight: 500;
  }
  tr.blocked .id {
    box-shadow: inset 3px 0 0 var(--w-danger);
  }
  tr.hold .title button {
    color: var(--w-muted);
  }
  .empty {
    text-align: center;
    color: var(--w-muted);
    padding: 28px;
    cursor: default;
  }
</style>
