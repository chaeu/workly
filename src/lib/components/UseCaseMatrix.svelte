<script lang="ts">
  // Feasibility matrix: assessed use cases by value (up) and feasibility (right), with a list by quadrant beside it.
  // Read-only; scores change in the editor (D5). Advanced adds bubble size by manual effort and quadrant colours.
  import { projColor, type Process } from "$lib/stores/workspace.svelte";
  import {
    assess,
    assessTip,
    fmtFte,
    fmtScore,
    phaseIndex,
    phaseOf,
    QUADRANT_COLOR,
    QUADRANTS,
    savedFte,
    scoreLine,
    stepName,
    stepOf,
    type AssessResult,
    type UC,
  } from "$lib/usecases.svelte";
  import { tip } from "$lib/tip";

  let { process: p, ucs, advanced, onopen }: { process: Process; ucs: UC[]; advanced: boolean; onopen: (key: string) => void } = $props();

  const rated = $derived(ucs.filter((u) => u.usecase.assessment).map((u) => ({ u, r: assess(p, u.usecase.assessment!) })));
  const ko = $derived(rated.filter((x) => x.r.ko === "fail"));
  // Incomplete (one axis unscored) has no position, so it waits with the unassessed ones.
  const placed = $derived(
    rated
      .filter((x) => x.r.ko !== "fail" && x.r.quadrant)
      .map((x) => ({ ...x, q: x.r.quadrant!, fte: savedFte(x.u), x: ((x.r.feasibility! - 1) / 2) * 100, y: (1 - (x.r.value! - 1) / 2) * 100 })),
  );
  const unplaced = $derived([...rated.filter((x) => x.r.ko !== "fail" && !x.r.quadrant), ...ucs.filter((u) => !u.usecase.assessment).map((u) => ({ u, r: null }))]);
  type Dot = (typeof placed)[number];

  /** Best first: closest to the top-right corner. */
  const byQuadrant = $derived(
    QUADRANTS.map((q) => {
      const list = placed.filter((d) => d.q === q).sort((a, b) => Math.hypot(3 - a.r.value!, 3 - a.r.feasibility!) - Math.hypot(3 - b.r.value!, 3 - b.r.feasibility!));
      return { q, list, fte: list.reduce((s, d) => s + (d.fte ?? 0), 0) };
    }),
  );

  // Advanced: bubble diameter grows with the square root of FTE, so area tracks effort.
  const size = (d: Dot) => (advanced ? 14 + Math.sqrt(d.fte ?? 0) * 26 : 11);
  // Still in the first phase (Idea) = dashed bubble.
  const early = (d: Dot) => phaseIndex(p, phaseOf(p, d.u)) === 0;

  let hover = $state<string | null>(null);
  const dimmed = (d: Dot) => !!hover && hover !== d.u.key;

  const dotTip = (d: Dot) =>
    [
      `${d.u.key} ${d.u.title}`,
      `${d.q} · ${scoreLine(d.r)}${d.fte != null ? ` · ${fmtFte(d.fte)} FTE` : ""}`,
      ...(d.r.open.length ? ["Open for pilot:", ...d.r.open.map((x) => `· ${x}`)] : []),
    ].join("\n");
  const chipTip = (u: UC, r: AssessResult | null) => `${u.key} ${u.title}\n${r ? assessTip(r) : "Not assessed"}`;

  // ------------------------------------------------------------ labels
  // Greedy placement: label beside its point (right, left near the right edge). On a collision with a placed label or
  // another point it moves up or down in steps and gets a leader line. Same spot for several use cases = one leader each.
  // ponytail: greedy in value order, no global optimum; good for the few dozen use cases a cockpit holds.
  let plot = $state<HTMLElement>();
  let lines = $state<{ x1: number; y1: number; x2: number; y2: number }[]>([]);

  // How far a label may reach past the plot box: the field margin minus the axis label left and the corner names on top.
  const EDGE = { x: 40, top: 12, bottom: 18 };
  const inside = (b: { l: number; r: number; t: number; b: number }, W: number, H: number) =>
    b.l >= -EDGE.x && b.r <= W + EDGE.x && b.t >= -EDGE.top && b.b <= H + EDGE.bottom;

  function place() {
    if (!plot) return;
    const [W, H] = [plot.clientWidth, plot.clientHeight];
    const labs = [...plot.querySelectorAll<HTMLElement>(".lab")];
    const pt = (el: HTMLElement) => [(+el.dataset.x! / 100) * W, (+el.dataset.y! / 100) * H, +el.dataset.r!];
    type Box = { l: number; r: number; t: number; b: number; own?: HTMLElement };
    const taken: Box[] = labs.map((el) => {
      const [x, y, r] = pt(el);
      return { l: x - r, r: x + r, t: y - r, b: y + r, own: el };
    });
    const out: typeof lines = [];
    for (const el of labs.sort((a, b) => +a.dataset.y! - +b.dataset.y!)) {
      const [px, py, r] = pt(el);
      const [w, h, off] = [el.offsetWidth, el.offsetHeight, r + 5];
      const sides = [px + off, px - off - w];
      let best: { box: Box; dy: number } | null = null;
      for (const dy of [0, -22, 22, -44, 44, -66, 66, -88, 88]) {
        for (const l of sides) {
          const box = { l, r: l + w, t: py - h / 2 + dy, b: py + h / 2 + dy };
          if (!inside(box, W, H)) continue;
          if (!taken.some((o) => o.own !== el && box.l < o.r + 3 && o.l < box.r + 3 && box.t < o.b + 1 && o.t < box.b + 1)) {
            best = { box, dy };
            break;
          }
        }
        if (best) break;
      }
      if (!best) {
        const l = px + off + w <= W + EDGE.x ? px + off : px - off - w;
        best = { box: { l, r: l + w, t: py - h / 2, b: py + h / 2 }, dy: 0 };
      }
      taken.push(best.box);
      el.style.left = `${best.box.l}px`;
      el.style.top = `${best.box.t}px`;
      if (best.dy) out.push({ x1: px, y1: py, x2: best.box.l < px ? best.box.r : best.box.l, y2: py + best.dy });
    }
    lines = out;
  }

  $effect(() => {
    void [placed, advanced];
    place();
  });
  $effect(() => {
    if (!plot) return;
    const ro = new ResizeObserver(() => place());
    ro.observe(plot);
    return () => ro.disconnect();
  });
</script>

<div class="wrap" class:advanced>
  <div class="panel">
    <div class="field" role="group" aria-label="Feasibility matrix">
      {#each [["Big bet", "tl"], ["Quick win", "tr"], ["Drop", "bl"], ["Fill-in", "br"]] as const as [q, pos] (q)}
        <div class="quad {pos}" style:--c={QUADRANT_COLOR[q]}><span class="w-caps">{q}</span></div>
      {/each}
      <div class="plot" bind:this={plot}>
        <svg class="leads" aria-hidden="true">
          {#each lines as l, i (i)}<line {...l} />{/each}
        </svg>
        <span class="axis y w-caps">Value → more</span>
        <span class="axis x w-caps">Feasibility → easier</span>
        {#each placed as d (d.u.key)}
          {@const s = size(d)}
          <button
            type="button"
            class="dot"
            class:bubble={advanced}
            class:early={advanced && early(d)}
            class:dim={dimmed(d)}
            style:left="{d.x}%"
            style:top="{d.y}%"
            style:--d="{s}px"
            style:--c={advanced ? QUADRANT_COLOR[d.q] : projColor(d.u.color)}
            aria-label="{d.u.key} {d.u.title}, {d.q}, {scoreLine(d.r)}"
            use:tip={dotTip(d)}
            onclick={() => onopen(d.u.key)}
            onpointerenter={() => (hover = d.u.key)}
            onpointerleave={() => (hover = null)}
          ></button>
          <button
            type="button"
            class="lab"
            class:dim={dimmed(d)}
            class:hi={hover === d.u.key}
            data-x={d.x}
            data-y={d.y}
            data-r={s / 2}
            tabindex="-1"
            use:tip={dotTip(d)}
            onclick={() => onopen(d.u.key)}
            onpointerenter={() => (hover = d.u.key)}
            onpointerleave={() => (hover = null)}><span class="w-mono">{d.u.key}</span><span class="t">{d.u.title}</span></button
          >
        {/each}
      </div>
    </div>

    {#if advanced}
      <div class="legend">
        <span><i class="lg"></i>Pilot or later</span><span><i class="lg early"></i>Still in {p.phases[0]?.name ?? "the first phase"}</span>
        <span>Size = manual effort (FTE)</span><span>Colour = quadrant</span>
      </div>
    {/if}

    {#each [["K.O.", ko], ["Not assessed", unplaced]] as const as [name, list] (name)}
      <div class="row">
        <span class="w-caps">{name} ({list.length})</span>
        {#each list as { u, r } (u.key)}
          <button type="button" class="chip" use:tip={chipTip(u, r)} onclick={() => onopen(u.key)}
            ><span class="w-proj-mark" style:--c={projColor(u.color)}></span><span class="w-mono">{u.key}</span><span class="t">{u.title}</span>
            {#if r?.failed.length}<span class="why ko">✕ {r.failed[0]}</span>{:else if r}<span class="why">Incomplete</span>{/if}</button
          >
        {/each}
      </div>
    {/each}
  </div>

  <aside class="panel side" aria-label="Use cases by quadrant">
    <div class="scroll">
      {#each byQuadrant as g (g.q)}
        <h3>
          <span class="pill" style:--c={QUADRANT_COLOR[g.q]}>{g.q}</span><b>{g.list.length}</b>
          {#if g.list.length}<small class="w-mono">Σ {fmtFte(g.fte)} FTE</small>{/if}
        </h3>
        {#each g.list as d (d.u.key)}
          {@const st = stepOf(p, d.u.usecase.step)}
          <button
            type="button"
            class="li"
            class:hi={hover === d.u.key}
            onclick={() => onopen(d.u.key)}
            onpointerenter={() => (hover = d.u.key)}
            onpointerleave={() => (hover = null)}
          >
            <span class="w-mono key">{d.u.key}</span>
            <span class="t">{d.u.title}</span>
            <span class="w-mono fte">{d.fte != null ? `${fmtFte(d.fte)} FTE` : ""}</span>
            <span class="meta"
              ><span class="w-mono vf">V {fmtScore(d.r.value)} · F {fmtScore(d.r.feasibility)}</span>{#if st}<span>{stepName(st)}</span>{/if}{#if d.r.open.length}<span
                  class="open">{d.r.open.length} open</span
                >{/if}</span
            >
          </button>
        {:else}
          <p class="none">–</p>
        {/each}
      {/each}
    </div>
  </aside>
</div>
{#if !ucs.length}<p class="empty">No use case matches the filters.</p>{/if}

<style>
  .wrap {
    display: grid;
    grid-template-columns: minmax(0, 1fr) 340px;
    gap: 14px;
  }
  .panel {
    border-radius: var(--w-r-lg);
    background: var(--w-surface);
    box-shadow: var(--w-shadow-panel);
    padding: 14px;
    min-width: 0;
  }

  /* ---------- field ---------- */
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
  /* Score 1..3 maps onto this inset box, so points at 1 or 3 keep clear of the edges and the corner names. */
  .plot {
    position: absolute;
    inset: 40px 56px;
  }
  /* Room for the biggest bubble (3 FTE ≈ 60 px) below the corner names. */
  .advanced .plot {
    inset: 56px 56px;
  }
  .leads {
    position: absolute;
    inset: 0;
    width: 100%;
    height: 100%;
    overflow: visible;
    pointer-events: none;
  }
  .leads line {
    stroke: var(--w-edge);
    stroke-width: 1;
  }
  .axis {
    position: absolute;
    color: var(--w-muted);
  }
  .axis.y {
    left: -48px;
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
    width: var(--d);
    height: var(--d);
    transform: translate(-50%, -50%);
    border: 0;
    padding: 0;
    border-radius: 50%;
    background: var(--c);
    box-shadow: 0 0 0 2px var(--w-surface);
    cursor: pointer;
    transition:
      opacity var(--w-dur) var(--w-ease),
      width var(--w-dur) var(--w-ease),
      height var(--w-dur) var(--w-ease);
  }
  .dot.bubble {
    background: color-mix(in srgb, var(--c) 28%, transparent);
    box-shadow: inset 0 0 0 1.5px var(--c);
  }
  .dot.early {
    background: transparent;
    box-shadow: none;
    outline: 1.5px dashed var(--c);
    outline-offset: -1.5px;
  }
  .dot:hover {
    z-index: 2;
    box-shadow:
      inset 0 0 0 1.5px var(--c),
      var(--w-ring-active);
  }
  .lab {
    position: absolute;
    left: 0;
    top: 0;
    z-index: 1;
    display: flex;
    align-items: center;
    gap: 6px;
    max-width: 200px;
    border: 0;
    padding: 2px 8px;
    border-radius: var(--w-r-sm);
    background: var(--w-surface);
    box-shadow: var(--w-shadow-raised);
    color: var(--w-ink);
    font-size: var(--w-fs-caption);
    white-space: nowrap;
    cursor: pointer;
    transition: opacity var(--w-dur) var(--w-ease);
  }
  .lab .t {
    overflow: hidden;
    text-overflow: ellipsis;
  }
  .lab .w-mono {
    color: var(--w-muted);
  }
  .lab.hi {
    z-index: 3;
    box-shadow: var(--w-ring-active);
  }
  .dim {
    opacity: 0.25;
  }

  .legend {
    display: flex;
    flex-wrap: wrap;
    gap: 6px 16px;
    margin-top: 10px;
    color: var(--w-muted);
    font-size: var(--w-fs-caption);
  }
  .legend span {
    display: inline-flex;
    align-items: center;
    gap: 6px;
  }
  .lg {
    width: 12px;
    height: 12px;
    border-radius: 50%;
    box-shadow: inset 0 0 0 1.5px var(--w-muted);
  }
  .lg.early {
    box-shadow: none;
    outline: 1.5px dashed var(--w-muted);
    outline-offset: -1.5px;
  }

  /* ---------- K.O. / not assessed ---------- */
  .row {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: 6px;
    margin-top: 10px;
    min-height: 26px;
  }
  .row > .w-caps {
    min-width: 128px;
  }
  .chip {
    display: inline-flex;
    align-items: center;
    gap: 6px;
    max-width: 100%;
    border: 0;
    background: none;
    box-shadow: inset 0 0 0 1px var(--w-line);
    border-radius: var(--w-r-pill);
    padding: 2px 10px 2px 8px;
    color: var(--w-ink);
    font-size: var(--w-fs-caption);
    cursor: pointer;
  }
  .chip:hover {
    background: var(--w-surface);
    box-shadow: var(--w-shadow-raised);
  }
  .chip .w-mono {
    color: var(--w-muted);
  }
  .chip .t {
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .why {
    color: var(--w-muted);
    font-size: var(--w-fs-micro);
    white-space: nowrap;
  }
  .why.ko {
    color: var(--w-danger);
  }

  /* ---------- list beside the field: same height, scrolls inside ---------- */
  .side {
    position: relative;
    padding: 0;
  }
  .scroll {
    position: absolute;
    inset: 0;
    overflow-y: auto;
    padding: 12px 10px 12px 14px;
    scrollbar-width: thin;
    scrollbar-color: var(--w-line) transparent;
    scrollbar-gutter: stable;
  }
  h3 {
    margin: 14px 0 4px;
    display: flex;
    align-items: center;
    gap: 8px;
    font-family: var(--w-font-label);
    font-size: var(--w-fs-small);
    font-weight: 600;
  }
  h3:first-child {
    margin-top: 0;
  }
  h3 small {
    margin-left: auto;
    color: var(--w-muted);
    font-weight: 400;
  }
  .pill {
    font-family: var(--w-font-label);
    font-size: var(--w-fs-caption);
    font-weight: 600;
    white-space: nowrap;
    color: var(--c);
    box-shadow: inset 0 0 0 1px var(--c);
    border-radius: var(--w-r-pill);
    padding: 0 8px;
  }
  .li {
    width: 100%;
    display: grid;
    grid-template-columns: 38px minmax(0, 1fr) auto;
    gap: 2px 8px;
    padding: 6px 8px;
    border: 0;
    border-radius: var(--w-r-md);
    background: none;
    color: var(--w-ink);
    text-align: left;
    cursor: pointer;
    transition: opacity var(--w-dur) var(--w-ease);
  }
  .li:hover,
  .li.hi {
    background: var(--w-sunk);
  }
  .li .key {
    color: var(--w-muted);
    padding-top: 2px;
  }
  .li .t {
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
    font-size: var(--w-fs-small);
    font-weight: 500;
  }
  .li .fte {
    color: var(--w-muted);
    padding-top: 2px;
  }
  .meta {
    grid-column: 2 / 4;
    display: flex;
    flex-wrap: wrap;
    gap: 0 10px;
    color: var(--w-muted);
    font-size: var(--w-fs-caption);
  }
  .vf {
    color: var(--w-ink);
  }
  .open {
    color: var(--w-warn);
  }
  .none {
    margin: 0 8px;
    color: var(--w-muted);
    font-size: var(--w-fs-caption);
  }

  .empty {
    margin: 10px 0 0;
    color: var(--w-muted);
    font-size: var(--w-fs-small);
  }

  /* Narrow window: the list goes below the field with its own height. */
  @media (max-width: 1100px) {
    .wrap {
      grid-template-columns: minmax(0, 1fr);
    }
    .scroll {
      position: static;
      max-height: 360px;
    }
  }
</style>
