<script lang="ts">
  import type { Process } from "$lib/stores/workspace.svelte";
  import { daysInStep, isNegative, isStale, label, statusColor, stepOf, type UC } from "$lib/usecases.svelte";

  let { process: p, ucs, dim, dragKey, over }: { process: Process; ucs: UC[]; dim: (u: UC) => boolean; dragKey: string | null; over: string | null } =
    $props();

  // Column widths as in the prototype: gates 140, start/end only 88, boxes 176.
  const columns = $derived.by(() => {
    const last = Math.max(0, ...p.steps.map((s) => s.col));
    return Array.from({ length: last + 1 }, (_, c) => {
      const here = p.steps.filter((s) => s.col === c);
      if (!here.length) return "40px";
      if (here.some((s) => s.kind === "gate")) return "140px";
      return here.every((s) => s.kind === "term") ? "88px" : "176px";
    }).join(" ");
  });
  const row = (lane: string) => p.lanes.findIndex((l) => l.id === lane) + 1;
  // ponytail: lane tints by position from the four lane tokens; a fifth lane repeats the first.
  const TINTS = ["--w-lane-fb", "--w-lane-me", "--w-lane-gov", "--w-lane-it"];
  const tint = (i: number) => `var(${TINTS[i % TINTS.length]})`;
  const at = (step: string) => ucs.filter((u) => u.usecase.step === step);
  /** A terminal nothing leads into is the start. */
  const isStart = (id: string) => !p.edges.some((e) => e.to === id);

  // ---------------------------------------------------------------- edges
  // Routing copied from docs/reference/use-case-cockpit.html (drawEdges).

  type Drawn = { d: string; head: string; label: string | null; x: number; y: number; anchor: string; neg: boolean };
  let flow: HTMLDivElement;
  let drawn = $state<Drawn[]>([]);
  let size = $state({ w: 0, h: 0 });

  function draw() {
    if (!flow) return;
    const f = flow.getBoundingClientRect();
    const box = (id: string) => {
      const el = flow.querySelector(`.shape[data-node="${CSS.escape(id)}"]`);
      if (!el) return null;
      const r = el.getBoundingClientRect();
      const l = r.left - f.left;
      const t = r.top - f.top;
      return { l, t, r: l + r.width, b: t + r.height, cx: l + r.width / 2, ay: t + 28 };
    };
    const A = 6;
    const head = (x: number, y: number, dir: "r" | "d" | "u") =>
      ({
        r: `${x},${y} ${x - A * 1.4},${y - A / 1.3} ${x - A * 1.4},${y + A / 1.3}`,
        d: `${x},${y} ${x - A / 1.3},${y - A * 1.4} ${x + A / 1.3},${y - A * 1.4}`,
        u: `${x},${y} ${x - A / 1.3},${y + A * 1.4} ${x + A / 1.3},${y + A * 1.4}`,
      })[dir];
    const out: Drawn[] = [];
    for (const e of p.edges) {
      const s = box(e.from);
      const t = box(e.to);
      if (!s || !t) continue;
      const off = e.offset ?? 0;
      let d = "";
      let ah = "";
      let x = 0;
      let y = 0;
      let anchor = "start";
      if (e.route === "h") {
        d = `M${s.r} ${s.ay}H${t.l}`;
        ah = head(t.l, t.ay, "r");
        x = s.r + 3 + (e.label_dx ?? 0);
        y = s.ay - 7;
      } else if (e.route === "hvh") {
        const x1 = s.r + 12;
        d = `M${s.r} ${s.ay}H${x1}V${t.ay}H${t.l}`;
        ah = head(t.l, t.ay, "r");
        x = x1 + 6;
        y = (s.ay + t.ay) / 2;
      } else if (e.route === "hv") {
        const tx = t.cx + off;
        const right = tx > s.cx;
        const x0 = right ? s.r : s.l;
        const down = t.t > s.ay;
        const y1 = down ? t.t : t.b;
        d = `M${x0} ${s.ay}H${tx}V${y1}`;
        ah = head(tx, y1, down ? "d" : "u");
        x = right ? x0 + 6 : x0 - 6;
        y = s.ay - 7;
        anchor = right ? "start" : "end";
      } else if (e.route === "vu") {
        d = `M${s.cx} ${s.t}V${t.b}`;
        ah = head(s.cx, t.b, "u");
        x = s.cx + 8;
        y = s.t - 12;
      } else if (e.route === "over") {
        const top = s.t - 18;
        d = `M${s.cx} ${s.t}V${top}H${t.cx + off}V${t.t}`;
        ah = head(t.cx + off, t.t, "d");
        x = (s.cx + t.cx) / 2;
        // A gate in the first lane leaves no room above the loop: label goes under it.
        y = top < 20 ? top + 13 : top - 5;
        anchor = "middle";
      }
      out.push({ d, head: ah, label: e.label, x, y, anchor, neg: isNegative(e.label) });
    }
    size = { w: flow.scrollWidth, h: flow.scrollHeight };
    drawn = out;
  }

  // New data (a step renamed or added, a chip moved) changes box sizes: redraw after the DOM update.
  $effect(() => {
    void p;
    void ucs;
    const id = requestAnimationFrame(draw);
    return () => cancelAnimationFrame(id);
  });
  $effect(() => {
    const ro = new ResizeObserver(() => draw());
    ro.observe(flow);
    document.fonts?.ready.then(draw);
    return () => ro.disconnect();
  });
</script>

{#snippet chip(u: UC)}
  {@const st = u.usecase.status}
  {@const days = daysInStep(u)}
  <div
    class="uc"
    class:blocked={st === "blocked"}
    class:hold={st === "on_hold"}
    class:is-dim={dim(u)}
    class:is-dragging={dragKey === u.key}
    role="button"
    tabindex="0"
    data-key={u.key}
    title="{u.title} · {label(p.statuses, st)}{st === 'blocked' && u.usecase.blocked_by ? `: ${u.usecase.blocked_by}` : ''}"
  >
    <span class="w-dot" class:w-dot--hollow={st === "on_hold"} style:--c={statusColor(st)}></span>
    <span class="t">
      <span class="id w-mono"
        >{u.key}{#if u.usecase.type === "ai" || u.usecase.type === "hybrid"}<span class="ki">{label(p.types, u.usecase.type)}</span>{/if}</span
      >
      <span class="nm">{u.title}</span>
    </span>
    {#if days !== null}<span class="age w-mono" class:stale={isStale(p, u)}>{days}d</span>{/if}
  </div>
{/snippet}

<div class="flow" bind:this={flow} style:grid-template-columns="var(--lane-w) {columns}">
  {#each p.lanes as l, i (l.id)}
    {@const here = ucs.filter((u) => {
      const s = stepOf(p, u.usecase.step);
      return s?.lane === l.id && !s.parked;
    })}
    {@const blocked = here.filter((u) => u.usecase.status === "blocked").length}
    <div class="band" class:last={i === p.lanes.length - 1} style:grid-row={i + 1} style:background={tint(i)}></div>
    <div class="f-lane" style:grid-row={i + 1} style:--tint={tint(i)}>
      <strong>{l.label}</strong>
      {#if l.sub}<small>{l.sub}</small>{/if}
      <span class="ball">Ball here: <b>{here.length}</b>{#if blocked}{" · "}<span class="bl">{blocked} blocked</span>{/if}</span>
    </div>
  {/each}

  {#each p.steps as n (n.id)}
    {#if n.kind === "term"}
      <div class="node" style:grid-row={row(n.lane)} style:grid-column={n.col + 2}>
        <div class="term shape" class:start={isStart(n.id)} data-node={n.id}><span>{n.label}</span></div>
      </div>
    {:else if n.kind === "gate"}
      <div class="node" class:over={over === `step:${n.id}`} data-drop="step" data-step={n.id} style:grid-row={row(n.lane)} style:grid-column={n.col + 2}>
        <div class="gate shape" class:optional={n.optional} data-node={n.id} title={n.hint ?? undefined}>
          <svg viewBox="0 0 112 56" aria-hidden="true"><polygon points="56,1 111,28 56,55 1,28" /></svg>
          <div class="gi"><code>{n.code ?? ""}</code><span>{n.label}</span></div>
        </div>
        {#if at(n.id).length}
          <div class="gate-ucs">
            <span class="wait">Waiting for decision</span>
            {#each at(n.id) as u (u.key)}{@render chip(u)}{/each}
          </div>
        {/if}
      </div>
    {:else}
      <div class="node" class:over={over === `step:${n.id}`} data-drop="step" data-step={n.id} style:grid-row={row(n.lane)} style:grid-column={n.col + 2}>
        <div class="box shape" class:parked={n.parked} class:optional={n.optional} data-node={n.id}>
          <div class="hd">
            <strong>{n.label}{#if n.optional}<span class="opt">optional</span>{/if}</strong>
            {#if n.sub}<small>{n.sub}</small>{/if}
          </div>
          {#if at(n.id).length}
            <div class="ucs">{#each at(n.id) as u (u.key)}{@render chip(u)}{/each}</div>
          {/if}
        </div>
      </div>
    {/if}
  {/each}

  <svg class="edges" width={size.w} height={size.h} aria-hidden="true">
    {#each drawn as e, i (i)}
      <path d={e.d} />
      <polygon class="ah" points={e.head} />
      {#if e.label}<text x={e.x} y={e.y} text-anchor={e.anchor} class:neg={e.neg}>{e.label}</text>{/if}
    {/each}
  </svg>
</div>

<style>
  .flow {
    position: relative;
    display: grid;
    width: max-content;
  }
  .band {
    grid-column: 1 / -1;
    position: relative;
    z-index: 0;
    border-bottom: 1px solid var(--w-line);
  }
  .band.last {
    border-bottom: 0;
  }
  .f-lane {
    grid-column: 1;
    position: sticky;
    left: 0;
    z-index: 4;
    padding: 14px 12px;
    border-right: 1px solid var(--w-line);
    background: var(--w-surface);
    display: flex;
    flex-direction: column;
    gap: 2px;
    align-self: stretch;
  }
  .f-lane::before {
    content: "";
    position: absolute;
    inset: 0;
    background: var(--tint);
    z-index: -1;
  }
  .f-lane strong {
    font-family: var(--w-font-label);
    font-size: var(--w-fs-title);
    font-weight: 600;
  }
  .f-lane small {
    color: var(--w-muted);
    font-size: var(--w-fs-caption);
    line-height: 1.35;
  }
  .ball {
    margin-top: 8px;
    font-size: var(--w-fs-caption);
    font-variant-numeric: tabular-nums;
    color: var(--w-muted);
  }
  .ball b {
    color: var(--w-ink);
    font-weight: 600;
  }
  .ball .bl {
    color: var(--w-danger);
    font-weight: 600;
  }
  .node {
    position: relative;
    z-index: 2;
    padding: 34px 8px 18px;
    align-self: start;
    display: flex;
    flex-direction: column;
    align-items: center;
    min-height: 136px;
    box-sizing: border-box;
  }
  .edges {
    position: absolute;
    left: 0;
    top: 0;
    z-index: 1;
    pointer-events: none;
    overflow: visible;
  }
  .edges path {
    fill: none;
    stroke: var(--w-edge);
    stroke-width: 1.4;
  }
  .edges .ah {
    fill: var(--w-edge);
  }
  .edges text {
    font-family: var(--w-font-sans);
    font-size: var(--w-fs-caption);
    fill: var(--w-muted);
    paint-order: stroke;
    stroke: var(--w-surface);
    stroke-width: 4px;
    stroke-linejoin: round;
  }
  .edges text.neg {
    fill: var(--w-warn);
  }
  .box {
    width: 100%;
    background: var(--w-surface);
    border: 1.25px solid var(--w-edge);
    border-radius: var(--w-r-md);
    box-shadow: var(--w-shadow-card);
    transition:
      box-shadow var(--w-dur) var(--w-ease),
      border-color var(--w-dur) var(--w-ease);
  }
  .hd {
    height: 56px;
    padding: 0 10px;
    display: flex;
    flex-direction: column;
    justify-content: center;
    text-align: center;
    min-width: 0;
  }
  .hd strong {
    font-family: var(--w-font-label);
    font-size: var(--w-fs-body);
    font-weight: 600;
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }
  /* Up to two lines: a long sub still fits the fixed 56px header. */
  .hd small {
    color: var(--w-muted);
    font-size: var(--w-fs-caption);
    line-height: 1.25;
    display: -webkit-box;
    -webkit-line-clamp: 2;
    line-clamp: 2;
    -webkit-box-orient: vertical;
    overflow: hidden;
  }
  /* Chips sit in an inset well, so they read as items inside the step, not as part of it. */
  .ucs {
    display: flex;
    flex-direction: column;
    gap: 5px;
    margin: 0 6px 6px;
    padding: 6px;
    border-radius: var(--w-r-sm);
    background: var(--w-sunk);
    box-shadow: inset 0 0 0 1px var(--w-line);
  }
  .box.parked {
    border-style: dashed;
    background: transparent;
    box-shadow: none;
  }
  .box.parked .hd strong {
    color: var(--w-muted);
  }
  .box.optional {
    border-style: dashed;
    border-color: var(--w-accent);
  }
  .opt {
    display: inline-block;
    font-family: var(--w-font-mono);
    font-size: var(--w-fs-micro);
    color: var(--w-accent);
    box-shadow: inset 0 0 0 1px var(--w-accent);
    border-radius: var(--w-r-sm);
    padding: 0 4px;
    margin-left: 5px;
    vertical-align: 1px;
    font-weight: 500;
  }
  .gate {
    position: relative;
    width: 112px;
    height: 56px;
    cursor: help;
  }
  .gate svg {
    position: absolute;
    inset: 0;
    width: 100%;
    height: 100%;
    overflow: visible;
  }
  .gate polygon {
    fill: var(--w-surface);
    stroke: var(--w-accent);
    stroke-width: 1.5;
  }
  .gate.optional polygon {
    stroke-dasharray: 4 3;
  }
  .gi {
    position: absolute;
    inset: 0 18px;
    display: flex;
    flex-direction: column;
    align-items: center;
    justify-content: center;
    text-align: center;
    line-height: 1.1;
  }
  .gi code {
    font-family: var(--w-font-mono);
    font-size: var(--w-fs-micro);
    color: var(--w-accent);
  }
  .gi span {
    font-family: var(--w-font-label);
    font-size: var(--w-fs-micro);
    font-weight: 600;
    line-height: 1.05;
  }
  .gate-ucs {
    width: 100%;
    display: flex;
    flex-direction: column;
    gap: 5px;
    margin-top: 10px;
  }
  .wait {
    font-family: var(--w-font-label);
    font-size: var(--w-fs-micro);
    letter-spacing: 0.05em;
    text-transform: uppercase;
    color: var(--w-muted);
    text-align: center;
  }
  .term {
    height: 56px;
    display: flex;
    align-items: center;
  }
  .term span {
    display: inline-block;
    padding: 6px 16px;
    border-radius: var(--w-r-pill);
    border: 1.25px solid var(--w-edge);
    background: var(--w-surface);
    font-family: var(--w-font-label);
    font-weight: 600;
    font-size: var(--w-fs-small);
  }
  .term.start span {
    border-color: var(--w-accent);
    color: var(--w-accent);
  }
  .node.over .box {
    border-color: var(--w-accent);
    box-shadow: 0 0 0 3px var(--w-accent-soft);
  }
  .node.over .gate polygon {
    fill: var(--w-accent-soft);
    stroke-width: 2.5;
  }
  .uc {
    display: grid;
    grid-template-columns: auto 1fr auto;
    align-items: center;
    gap: 6px;
    background: var(--w-surface);
    border-radius: var(--w-r-sm);
    box-shadow: var(--w-shadow-card);
    padding: 5px 7px;
    cursor: grab;
    user-select: none;
    -webkit-user-select: none;
    touch-action: none;
    min-width: 0;
    transition: opacity var(--w-dur) var(--w-ease);
  }
  .uc:hover {
    box-shadow:
      var(--w-shadow-card),
      0 0 0 1px var(--w-line);
  }
  .t {
    min-width: 0;
    display: flex;
    flex-direction: column;
  }
  .id {
    font-size: var(--w-fs-micro);
    color: var(--w-muted);
    line-height: 1.2;
  }
  .nm {
    font-size: var(--w-fs-caption);
    font-weight: 500;
    line-height: 1.25;
    display: -webkit-box;
    -webkit-line-clamp: 2;
    line-clamp: 2;
    -webkit-box-orient: vertical;
    overflow: hidden;
    hyphens: auto;
  }
  .ki {
    font-family: var(--w-font-label);
    font-size: var(--w-fs-micro);
    font-weight: 600;
    color: var(--w-accent);
    letter-spacing: 0.04em;
    text-transform: uppercase;
    margin-left: 4px;
  }
  .age {
    font-size: var(--w-fs-micro);
    color: var(--w-muted);
  }
  .age.stale {
    color: var(--w-warn);
    font-weight: 500;
  }
  .uc.blocked {
    background: var(--w-danger-soft);
    box-shadow: inset 0 0 0 1px var(--w-danger);
  }
  .uc.hold {
    background: transparent;
    box-shadow: inset 0 0 0 1px var(--w-line);
  }
  .uc.hold .nm {
    color: var(--w-muted);
  }
  .is-dim {
    opacity: 0.2;
  }
  .is-dragging {
    opacity: 0.3;
  }
</style>
