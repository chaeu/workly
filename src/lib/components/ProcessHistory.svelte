<script lang="ts">
  // Vertical phase track that doubles as the history: past phases with their time span,
  // decisions and status episodes (from .workly/log), the current phase, the next one,
  // and the rest folded into one line.
  import { workspace, usecaseHistory, type PhaseSpan, type Process } from "$lib/stores/workspace.svelte";
  import { daysBetween, daysInStep, fmtDay, fmtSpan, isStale, label, phaseIndex, statusColor, stepName, stepOf, type UC } from "$lib/usecases.svelte";

  let { u, process: p }: { u: UC; process: Process } = $props();

  let spans = $state<PhaseSpan[]>([]);
  $effect(() => {
    void workspace.reloads;
    const key = u.key;
    usecaseHistory(key).then(
      (h) => key === u.key && (spans = h),
      (e) => (workspace.error = String(e)),
    );
  });

  const step = $derived(stepOf(p, u.usecase.step));
  const lane = $derived(p.lanes.find((l) => l.id === step?.lane));
  const track = $derived(p.phases.filter((x) => !x.parked));
  const cur = $derived(track.findIndex((x) => x.id === step?.phase));
  const parked = $derived(step?.phase ? p.phases[phaseIndex(p, step.phase)] : undefined);
  const now = $derived(cur >= 0 ? track[cur] : parked?.parked ? parked : undefined);
  const spanOf = (id: string) => spans.find((s) => s.phase === id);
  // Parked: everything with history counts as done, nothing comes next.
  const done = $derived(cur >= 0 ? track.slice(0, cur) : track.filter((ph) => spanOf(ph.id)));
  const later = $derived(cur >= 0 ? track.slice(cur + 1) : now ? [] : track);
  const nowStart = $derived((now && spanOf(now.id)?.start) || u.usecase.step_since?.slice(0, 10) || null);
  const days = $derived(daysInStep(u));
  const firstDay = $derived(spans.map((s) => s.start).filter((d): d is string => !!d).sort()[0] ?? nowStart);
  // Active is the normal case; only the detours and decisions are worth a line.
  const events = (id: string) => spanOf(id)?.events.filter((e) => e.kind === "decision" || e.status !== "active") ?? [];
</script>

{#snippet eventList(id: string)}
  {#each events(id) as e, i (i)}
    <div class="ev">
      <span class="w-mono">{fmtDay(e.date)}</span>
      {#if e.kind === "decision"}
        <span><i class="dia"></i>{e.gate ? `${e.gate} · ` : ""}{e.text}</span>
      {:else}
        <span
          ><i class="bar" style:--c={statusColor(e.status)}></i>{label(p.statuses, e.status)}
          {e.until ? `${daysBetween(e.date, e.until)} d` : `since ${fmtDay(e.date)}`}{e.text ? ` · ${e.text}` : ""}</span
        >
      {/if}
    </div>
  {/each}
{/snippet}

<div class="head">
  <h3 class="w-caps">Process</h3>
  {#if firstDay}<span class="w-sub">running {daysBetween(firstDay, null)} days</span>{/if}
</div>
{#if !step}
  <p class="stale">Unknown step “{u.usecase.step ?? "none"}”, pick one under Classification.</p>
{/if}
<ol class="vt">
  {#each done as ph (ph.id)}
    {@const s = spanOf(ph.id)}
    <li class="done">
      <div class="row">
        <span>{ph.name}</span>
        <span class="d">{s?.start ? `${fmtSpan(s.start, s.end)} · ${daysBetween(s.start, s.end)} d` : ph.optional && !s ? "skipped" : "–"}</span>
      </div>
      {@render eventList(ph.id)}
    </li>
  {/each}
  {#if now}
    <li class="cur" class:parked={now.parked}>
      <div class="row">
        <b>{now.name}</b>
        {#if nowStart}<span class="d">since {fmtDay(nowStart)} · {daysBetween(nowStart, null)} d</span>{/if}
      </div>
      {#if step}
        <div class="where">
          {stepName(step)} · Owner <b>{lane?.label ?? step.lane}</b>{#if days !== null}{" · "}<span class:stale={isStale(p, u)}>{days} d in step</span>{/if}
        </div>
      {/if}
      {@render eventList(now.id)}
    </li>
  {/if}
  {#if later.length}
    <li class="next"><div class="row"><span>{later[0].name}</span><span class="d">next</span></div></li>
  {/if}
  {#if later.length > 1}
    <li class="rest"><div class="row">then {later.slice(1).map((ph) => ph.name).join(" · ")}</div></li>
  {/if}
</ol>

<style>
  .head {
    display: flex;
    align-items: baseline;
    justify-content: space-between;
  }
  h3 {
    margin: 0;
  }
  .vt {
    list-style: none;
    margin: 0;
    padding: 0;
    position: relative;
    display: grid;
    gap: 2px;
  }
  .vt::before {
    content: "";
    position: absolute;
    left: 5px;
    top: 10px;
    bottom: 10px;
    width: 2px;
    background: var(--w-line);
  }
  li {
    position: relative;
    padding: 3px 0 3px 24px;
    display: grid;
    gap: 2px;
  }
  li::before {
    content: "";
    position: absolute;
    left: 0;
    top: 6px;
    width: 12px;
    height: 12px;
    box-sizing: border-box;
    border-radius: 50%;
    border: 2px solid var(--w-line);
    background: var(--w-surface);
  }
  li.done::before {
    background: var(--w-accent);
    border-color: var(--w-accent);
  }
  li.cur::before {
    border-color: var(--w-accent);
    box-shadow: 0 0 0 4px var(--w-accent-soft);
  }
  li.cur.parked::before,
  li.rest::before {
    border-style: dashed;
    border-color: var(--w-muted);
    box-shadow: none;
  }
  .row {
    display: flex;
    align-items: baseline;
    gap: var(--w-s-2);
    font-size: var(--w-fs-small);
  }
  .row b {
    font-weight: 600;
  }
  .d {
    margin-left: auto;
    font-size: var(--w-fs-caption);
    color: var(--w-muted);
    white-space: nowrap;
  }
  .next .row,
  .rest .row {
    color: var(--w-muted);
  }
  .rest .row {
    font-size: var(--w-fs-caption);
  }
  .where,
  .ev {
    font-size: var(--w-fs-caption);
    color: var(--w-muted);
  }
  .where b {
    color: var(--w-ink);
    font-weight: 500;
  }
  .ev {
    display: flex;
    gap: var(--w-s-2);
    align-items: baseline;
  }
  .ev .w-mono {
    flex: none;
    width: 46px;
    white-space: nowrap;
  }
  .dia {
    display: inline-block;
    width: 7px;
    height: 7px;
    margin-right: 6px;
    transform: rotate(45deg);
    background: var(--w-accent);
  }
  .bar {
    display: inline-block;
    width: 12px;
    height: 4px;
    margin-right: 6px;
    border-radius: var(--w-r-sm);
    vertical-align: middle;
    background: var(--c);
  }
  .stale {
    margin: 0;
    color: var(--w-warn);
    font-size: var(--w-fs-small);
  }
  .where .stale {
    font-size: inherit;
    font-weight: 500;
  }
</style>
