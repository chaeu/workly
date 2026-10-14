<script lang="ts">
  import { workspace, agentFeed, type FeedEvent } from "$lib/stores/workspace.svelte";
  import { agentName, blockedReason, isoDate, shortDate, today } from "$lib/tasks.svelte";

  let { onopen }: { onopen: (id: string) => void } = $props();

  // Re-read with the index: notes arrive via the watcher, transitions with the task writes.
  let events = $state<FeedEvent[]>([]);
  let error = $state("");
  $effect(() => {
    void workspace.index;
    agentFeed().then(
      (e) => ((events = e), (error = "")),
      (e) => (error = String(e)),
    );
  });

  /** null = all agents; "important" = reviews, blocks and commits only. */
  let agent = $state<string | null>(null);
  let important = $state(false);
  const agents = $derived([...new Set(events.map((e) => e.agent))].sort());
  const IMPORTANT = new Set(["review", "blocked", "commit"]);
  const shown = $derived(events.filter((e) => (!agent || e.agent === agent) && (!important || IMPORTANT.has(e.kind))));
  let limit = $state(100);

  const who = (a: string) => (a === "me" ? "You" : agentName(a));
  const time = (iso: string) => new Date(iso).toLocaleTimeString("en-GB", { hour: "2-digit", minute: "2-digit" });
  function dayLabel(day: string) {
    const yesterday = new Date(Date.parse(`${today()}T00:00`) - 864e5);
    if (day === today()) return "Today";
    if (day === isoDate(yesterday)) return "Yesterday";
    return new Date(`${day}T00:00`).toLocaleDateString("en-US", { weekday: "short" }) + ", " + shortDate(day);
  }
  /** Local days, newest first, each with its events. */
  const days = $derived.by(() => {
    const out: { day: string; events: FeedEvent[] }[] = [];
    for (const e of shown.slice(0, limit)) {
      const day = isoDate(new Date(e.ts));
      if (out.at(-1)?.day !== day) out.push({ day, events: [] });
      out.at(-1)!.events.push(e);
    }
    return out;
  });
  const titleOf = (id: string) => workspace.index?.tasks.find((t) => t.id === id)?.title ?? null;
</script>

<div class="content">
  <div class="filters" role="group" aria-label="Filter">
    <button type="button" class="w-chip" aria-pressed={!agent} onclick={() => (agent = null)}>All <span class="w-mono">{events.length}</span></button>
    {#each agents as a (a)}
      <button type="button" class="w-chip" aria-pressed={agent === a} onclick={() => (agent = agent === a ? null : a)}
        >{who(a)} <span class="w-mono">{events.filter((e) => e.agent === a).length}</span></button
      >
    {/each}
    <button type="button" class="w-chip" aria-pressed={important} onclick={() => (important = !important)}>Reviews &amp; blocks</button>
  </div>

  {#if error}
    <p class="w-sub">{error}</p>
  {:else if !shown.length}
    <p class="w-sub">No agent activity in the last 30 days.</p>
  {:else}
    <section class="timeline" aria-label="Activity">
      {#each days as d (d.day)}
        <div class="day"><span class="w-caps">{dayLabel(d.day)}</span><span class="w-mono count">{d.events.length}</span></div>
        {#each d.events as e, i (i)}
          {@const title = titleOf(e.id)}
          <div class="ev" class:last={i === d.events.length - 1}>
            <span class="w-mono t">{time(e.ts)}</span>
            <span class="rail"><span class="pin {e.kind}"></span></span>
            <span class="who">{who(e.agent)}</span>
            <span class="body">
              <span class="task">
                <span class="w-mono">{e.id}</span>
                {#if title}<button type="button" class="link" onclick={() => onopen(e.id)}>{title}</button>{:else}<span class="gone">(deleted)</span>{/if}
                {#if e.kind !== "note"}<span class="kind {e.kind}">{e.kind}</span>{/if}
              </span>
              {#if e.text}<span class="x">{e.kind === "blocked" ? blockedReason(e.text) : e.text}</span>{/if}
            </span>
          </div>
        {/each}
      {/each}
      {#if shown.length > limit}
        <button type="button" class="more" onclick={() => (limit += 100)}>Show {Math.min(100, shown.length - limit)} more</button>
      {/if}
    </section>
  {/if}
</div>

<style>
  .content {
    max-width: 960px;
    display: flex;
    flex-direction: column;
    gap: var(--w-s-3);
  }
  .filters {
    display: flex;
    flex-wrap: wrap;
    gap: var(--w-s-2);
  }
  .w-chip .w-mono {
    color: var(--w-muted);
  }
  .timeline {
    padding: 6px 0 10px;
    background: var(--w-surface);
    border-radius: var(--w-r-lg);
    box-shadow: var(--w-shadow-panel);
  }
  .day {
    display: flex;
    align-items: center;
    gap: var(--w-s-2);
    padding: 14px 18px 6px;
  }
  .count {
    color: var(--w-muted);
  }
  .ev {
    display: grid;
    grid-template-columns: 44px 14px 64px minmax(0, 1fr);
    gap: 0 var(--w-s-3);
    padding: 0 18px;
  }
  .t {
    padding-top: 12px;
    color: var(--w-muted);
  }
  .rail {
    position: relative;
    display: flex;
    justify-content: center;
  }
  .rail::before {
    content: "";
    position: absolute;
    top: 0;
    bottom: 0;
    width: 1px;
    background: var(--w-line);
  }
  .pin {
    position: relative;
    width: 9px;
    height: 9px;
    margin-top: 14px;
    border-radius: 50%;
    background: var(--w-surface);
    box-shadow: inset 0 0 0 2px var(--w-edge);
  }
  .pin.review {
    background: var(--w-accent);
    box-shadow: none;
  }
  .pin.blocked {
    background: var(--w-danger);
    box-shadow: none;
  }
  .pin.commit {
    background: var(--w-ok);
    box-shadow: none;
  }
  .who {
    padding-top: 11px;
    font-size: var(--w-fs-caption);
    color: var(--w-muted);
  }
  .body {
    display: flex;
    flex-direction: column;
    gap: 3px;
    min-width: 0;
    padding: 9px 0;
    border-bottom: 1px solid var(--w-line);
  }
  .ev.last .body {
    border-bottom: 0;
  }
  .task {
    display: flex;
    align-items: center;
    gap: var(--w-s-2);
    min-width: 0;
    font-size: var(--w-fs-caption);
    color: var(--w-muted);
  }
  .link {
    min-width: 0;
    padding: 0;
    border: 0;
    background: none;
    color: var(--w-ink);
    font: inherit;
    font-size: var(--w-fs-small);
    font-weight: 500;
    text-align: left;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
    cursor: pointer;
  }
  .link:hover {
    color: var(--w-accent);
  }
  .gone {
    font-style: italic;
  }
  .x {
    font-size: var(--w-fs-small);
    overflow-wrap: anywhere;
    user-select: text;
  }
  .kind {
    padding: 0 6px;
    border-radius: 4px;
    font-family: var(--w-font-label);
    font-size: var(--w-fs-micro);
    font-weight: 600;
    letter-spacing: 0.04em;
    text-transform: uppercase;
    white-space: nowrap;
    color: var(--w-muted);
    box-shadow: inset 0 0 0 1px var(--w-line);
  }
  .kind.review {
    color: var(--w-accent);
    background: var(--w-accent-soft);
    box-shadow: none;
  }
  .kind.blocked {
    color: var(--w-danger);
    background: var(--w-danger-soft);
    box-shadow: none;
  }
  .kind.commit {
    color: var(--w-ok);
    box-shadow: inset 0 0 0 1px var(--w-ok);
  }
  .more {
    margin: 8px 0 0 calc(18px + 44px + 14px + 2 * var(--w-s-3));
    padding: 4px 0;
    border: 0;
    background: none;
    color: var(--w-accent);
    font: inherit;
    font-size: var(--w-fs-small);
    cursor: pointer;
  }
</style>
