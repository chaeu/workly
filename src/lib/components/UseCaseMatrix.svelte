<script lang="ts">
  // Feasibility matrix: assessed use cases by value (up) and feasibility (right). Read-only; scores change in the editor (D5).
  import { projColor, type Process } from "$lib/stores/workspace.svelte";
  import { ASSESS_THRESHOLD, assess, assessTip, fmtScore, QUADRANT_COLOR, scoreLine, verdict, type AssessResult, type UC } from "$lib/usecases.svelte";
  import { tip } from "$lib/tip";

  let { process: p, ucs, onopen }: { process: Process; ucs: UC[]; onopen: (key: string) => void } = $props();

  const rated = $derived(ucs.filter((u) => u.usecase.assessment).map((u) => ({ u, r: assess(p, u.usecase.assessment!) })));
  const ko = $derived(rated.filter((x) => x.r.ko === "fail"));
  // Incomplete (one axis unscored) has no position, so it waits with the unassessed ones.
  const placed = $derived(rated.filter((x) => x.r.ko !== "fail" && x.r.quadrant));
  const unplaced = $derived([
    ...rated.filter((x) => x.r.ko !== "fail" && !x.r.quadrant),
    ...ucs.filter((u) => !u.usecase.assessment).map((u) => ({ u, r: null })),
  ]);

  // Same shown scores = same spot: stack those vertically so every key stays readable. The stack grows toward the middle
  // of its own quadrant, so a stack on the 2.0 line or the field edge never crosses the line or leaves the field.
  // ponytail: only identical (rounded) scores are spread; near neighbours may still touch, a collision pass if that bites.
  const dots = $derived.by(() => {
    const groups = new Map<string, typeof placed>();
    for (const x of placed) {
      const k = `${fmtScore(x.r.value)}|${fmtScore(x.r.feasibility)}`;
      groups.set(k, [...(groups.get(k) ?? []), x]);
    }
    return [...groups.values()].flatMap((g) => {
      const v = g[0].r.value!;
      const down = v >= (v >= ASSESS_THRESHOLD ? 2.5 : 1.5);
      return g
        .sort((a, b) => a.u.key.localeCompare(b.u.key, undefined, { numeric: true }))
        .map((x, i) => ({ ...x, x: ((x.r.feasibility! - 1) / 2) * 100, y: (1 - (v - 1) / 2) * 100, shift: down ? i : i - (g.length - 1) }));
    });
  });

  const dotTip = (u: UC, r: AssessResult) =>
    [`${u.key} ${u.title}`, scoreLine(r), ...(r.open.length ? ["Open for pilot:", ...r.open.map((x) => `· ${x}`)] : [])].join("\n");
  const chipTip = (u: UC, r: AssessResult | null) => `${u.key} ${u.title}\n${r ? assessTip(r) : "Not assessed"}`;

  const CORNERS = [
    ["Big bet", "tl"],
    ["Quick win", "tr"],
    ["Drop", "bl"],
    ["Fill-in", "br"],
  ] as const;
</script>

<div class="wrap">
  <div class="field" role="group" aria-label="Feasibility matrix">
    {#each CORNERS as [q, pos] (q)}
      <div class="quad {pos}" style:--c={QUADRANT_COLOR[q]}><span class="w-caps">{q}</span></div>
    {/each}
    <div class="plot">
      <span class="axis y w-caps">Value →</span>
      <span class="axis x w-caps">Feasibility →</span>
      {#each dots as d (d.u.key)}
        <button
          type="button"
          class="dot"
          class:flip={d.x > 85}
          style:left="{d.x}%"
          style:top="{d.y}%"
          style:--shift={d.shift}
          style:--c={projColor(d.u.color)}
          aria-label="{d.u.key} {d.u.title}, {verdict(d.r)}, {scoreLine(d.r)}"
          use:tip={dotTip(d.u, d.r)}
          onclick={() => onopen(d.u.key)}><i></i><span class="w-mono">{d.u.key}</span></button
        >
      {/each}
    </div>
  </div>

  {#each [["K.O.", ko], ["Not assessed", unplaced]] as const as [name, list] (name)}
    <div class="row">
      <span class="w-caps">{name} ({list.length})</span>
      {#each list as { u, r } (u.key)}
        <button type="button" class="w-chip" use:tip={chipTip(u, r)} onclick={() => onopen(u.key)}
          ><span class="w-proj-mark" style:--c={projColor(u.color)}></span><span class="w-mono">{u.key}</span></button
        >
      {/each}
    </div>
  {/each}
  {#if !ucs.length}<p class="empty">No use case matches the filters.</p>{/if}
</div>

<style>
  .wrap {
    border-radius: var(--w-r-lg);
    background: var(--w-surface);
    box-shadow: var(--w-shadow-panel);
    padding: 14px;
  }
  /* Scales with the window; the rows below stay in view at the reference size. */
  .field {
    position: relative;
    height: clamp(320px, calc(100vh - 400px), 1000px);
    display: grid;
    grid-template: 1fr 1fr / 1fr 1fr;
    gap: 2px;
    border-radius: var(--w-r-md);
    overflow: hidden;
  }
  .quad {
    background: color-mix(in srgb, var(--c) 7%, var(--w-surface));
    padding: 8px 12px;
    display: flex;
  }
  .quad span {
    color: var(--c);
  }
  .tr,
  .br {
    justify-content: flex-end;
  }
  .bl,
  .br {
    align-items: flex-end;
  }
  /* Score 1..3 maps onto this inset box, so dots at 1 or 3 keep clear of the edges and the corner names. */
  .plot {
    position: absolute;
    inset: 34px 48px;
  }
  .axis {
    position: absolute;
    color: var(--w-muted);
  }
  .axis.y {
    left: -40px;
    top: 50%;
    writing-mode: vertical-rl;
    transform: translateY(-50%) rotate(180deg);
  }
  .axis.x {
    left: 50%;
    bottom: -28px;
    transform: translateX(-50%);
  }
  .dot {
    position: absolute;
    display: flex;
    align-items: center;
    gap: 5px;
    border: 0;
    background: none;
    padding: 1px 3px;
    border-radius: var(--w-r-sm);
    color: var(--w-ink);
    font-size: var(--w-fs-caption);
    cursor: pointer;
    white-space: nowrap;
    /* The dot (not the label) sits on the score; stacked keys keep one label height apart. */
    transform: translate(-8px, calc(-50% + var(--shift) * 20px));
  }
  .dot.flip {
    flex-direction: row-reverse;
    transform: translate(calc(-100% + 8px), calc(-50% + var(--shift) * 20px));
  }
  .dot i {
    width: 10px;
    height: 10px;
    border-radius: 50%;
    background: var(--c);
    box-shadow: 0 0 0 2px var(--w-surface);
    flex: none;
  }
  .dot:hover,
  .dot:focus-visible {
    background: var(--w-surface);
    box-shadow: var(--w-shadow-raised);
    z-index: 1;
  }
  .row {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: 6px;
    margin-top: 10px;
    min-height: 26px;
  }
  .row .w-caps {
    color: var(--w-muted);
    margin-right: 4px;
  }
  .row .w-chip {
    padding: 2px 9px 2px 7px;
    font-size: var(--w-fs-caption);
    box-shadow: inset 0 0 0 1px var(--w-line);
    background: none;
  }
  .row .w-chip:hover {
    box-shadow: var(--w-shadow-raised);
  }
  .empty {
    margin: 10px 0 0;
    color: var(--w-muted);
    font-size: var(--w-fs-small);
  }
</style>
