<script lang="ts">
  // Assessment of one use case against the method in process.yml. A local draft; Save writes the whole
  // block at once (one write, one log line), Clear removes it.
  import { ask } from "@tauri-apps/plugin-dialog";
  import AssessmentPill from "$lib/components/AssessmentPill.svelte";
  import { workspace, updateProjectField, type KoValue, type Process, type UseCaseAssessment } from "$lib/stores/workspace.svelte";
  import { today } from "$lib/tasks.svelte";
  import { assess, axisCalc, fmtFte, fmtScore, fteScore, savedFte, type UC } from "$lib/usecases.svelte";

  let { key, process: p, onclose }: { key: string; process: Process; onclose: () => void } = $props();

  const u = $derived(workspace.index?.projects.find((x) => x.key === key && x.usecase) as UC | undefined);
  $effect(() => {
    if (workspace.index && !u) onclose();
  });

  // ponytail: assumes p.assessment exists; the callers only open the editor when it does.
  const m = $derived(p.assessment!);
  // Only ids the method knows (the core refuses unknown ones), so re-assessing an old assessment drops ids
  // that left process.yml, and a bonus only as 3 (an old reuse: 1 or 2 means no). Every K.O. question is written, open by default.
  // svelte-ignore state_referenced_locally
  const cur = u?.usecase.assessment;
  // svelte-ignore state_referenced_locally
  let draft = $state<UseCaseAssessment>({
    date: null,
    ko: Object.fromEntries(p.assessment!.ko.map((k) => [k.id, cur?.ko[k.id] ?? "open"])),
    scores: Object.fromEntries(
      p.assessment!.criteria.filter((c) => cur?.scores[c.id] != null && (!c.bonus || cur.scores[c.id] === 3)).map((c) => [c.id, cur!.scores[c.id]]),
    ),
    note: cur?.note ?? "",
  });
  // A criterion with `fte` takes its points from savings while there is any (D12): fixed here, never written.
  const fte = $derived(u ? savedFte(u) : null);
  const r = $derived(assess(p, draft, fte));

  const KO: KoValue[] = ["pass", "fail", "open"];
  const AXES = [
    ["value", "Value"],
    ["feasibility", "Feasibility"],
  ] as const;
  function setScore(id: string, n: number | null) {
    if (n === null) delete draft.scores[id];
    else draft.scores[id] = n;
  }

  let busy = $state(false);
  async function write(value: UseCaseAssessment | null) {
    busy = true;
    try {
      await updateProjectField(key, "usecase.assessment", value);
      onclose();
    } catch (e) {
      workspace.error = String(e);
    } finally {
      busy = false;
    }
  }
  function save() {
    const d = $state.snapshot(draft);
    const scores = Object.fromEntries(Object.entries(d.scores).filter(([id]) => !m.criteria.some((c) => c.id === id && fteScore(c, fte) != null)));
    write({ ...d, scores, date: today(), note: d.note?.trim() || null });
  }
  async function clear() {
    const ok = await ask(`Remove the assessment of ${key}? The log keeps the old one.`, { title: "Clear assessment", kind: "warning", okLabel: "Clear" });
    if (ok) await write(null);
  }

  function onkeydown(e: KeyboardEvent) {
    if (e.key === "Escape") onclose();
  }
</script>

<svelte:window {onkeydown} />

{#if u}
  <div class="w-scrim" role="presentation" onclick={onclose}></div>
  <div class="w-modal" role="dialog" aria-modal="true" aria-labelledby="ae-title">
    <div class="head">
      <span class="w-mono">{u.key}</span>
      <h2 class="w-h2" id="ae-title">Assessment</h2>
      <button type="button" class="x" aria-label="Close" onclick={onclose}>✕</button>
    </div>
    <div class="body">
      {#if m.ko.length}
        <section>
          <h3 class="w-caps">K.O. questions</h3>
          {#each m.ko as k (k.id)}
            <div class="row">
              <span class="lbl">{k.label}</span>
              <div class="w-seg" role="group" aria-label={k.label}>
                {#each KO as v (v)}
                  <button type="button" class:fail={v === "fail"} aria-pressed={draft.ko[k.id] === v} onclick={() => (draft.ko[k.id] = v)}>{v}</button>
                {/each}
              </div>
            </div>
          {/each}
        </section>
      {/if}
      {#each AXES as [axis, name] (axis)}
        {@const bonus = m.criteria.filter((c) => c.axis === axis && c.bonus)}
        <section>
          <h3 class="w-caps">{name}</h3>
          {#each m.criteria.filter((c) => c.axis === axis && !c.bonus) as c (c.id)}
            {@const derived = fteScore(c, fte)}
            {@const s = derived ?? draft.scores[c.id] ?? null}
            <div class="row crit">
              <div>
                <span class="lbl">{c.label}</span>
                <!-- The anchors stay readable while scoring: that is the method. -->
                <ol class="anchors">
                  {#each c.anchors as a, i (i)}
                    <li class:on={s === i + 1}><span class="w-mono">{i + 1}</span>{a}</li>
                  {/each}
                </ol>
              </div>
              {#if derived !== null}
                <span class="derived">From savings: {fmtFte(fte ?? 0)} FTE → <b class="w-mono">{fmtScore(derived)}</b></span>
              {:else}
                <div class="w-seg" role="group" aria-label={c.label}>
                  {#each [1, 2, 3, null] as n (n)}
                    <button type="button" aria-pressed={s === n} title={n === null ? "Open" : c.anchors[n - 1]} onclick={() => setScore(c.id, n)}>{n ?? "?"}</button>
                  {/each}
                </div>
              {/if}
            </div>
          {/each}
          {#if bonus.length}
            <!-- Yes/no (D13): checked = 3, unchecked = not counted and not open. -->
            <h4 class="w-caps bonus-h">Bonus <span>only raises the value</span></h4>
            {#each bonus as c (c.id)}
              <label class="bonus">
                <input type="checkbox" checked={draft.scores[c.id] === 3} onchange={(e) => setScore(c.id, e.currentTarget.checked ? 3 : null)} />
                <span class="bonus-txt"><span class="lbl">{c.label}</span><span class="w-sub">{c.anchors[0]}</span></span>
              </label>
            {/each}
          {/if}
        </section>
      {/each}
      <label class="field">
        <span class="w-caps">Note</span>
        <input class="note" bind:value={draft.note} placeholder="What the scores depend on" />
      </label>
    </div>
    <div class="foot">
      <div class="result">
        <AssessmentPill result={r} />
        <!-- The arithmetic, so a score can be explained: mean of the scored criteria per axis. -->
        <span>Value <span class="w-mono calc">{axisCalc(p, r.scores, "value")}</span></span>
        <span>Feasibility <span class="w-mono calc">{axisCalc(p, r.scores, "feasibility")}</span></span>
        <span class="w-sub">{r.open.length} open</span>
      </div>
      {#if u.usecase.assessment}<button type="button" class="w-btn w-btn--quiet" disabled={busy} onclick={clear}>Clear…</button>{/if}
      <button type="button" class="w-btn" onclick={onclose}>Cancel</button>
      <button type="button" class="w-btn w-btn--primary" disabled={busy} onclick={save}>Save</button>
    </div>
  </div>
{/if}

<style>
  .head {
    display: flex;
    align-items: baseline;
    gap: var(--w-s-3);
    padding: 18px 22px 6px;
  }
  .head .w-mono {
    color: var(--w-muted);
  }
  .head h2 {
    margin: 0;
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
  .body {
    overflow-y: auto;
    padding: 8px 22px 18px;
    display: grid;
    gap: 18px;
  }
  section {
    display: grid;
  }
  h3 {
    margin: 0 0 4px;
  }
  .row {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: var(--w-s-4);
    padding: 7px 0;
    border-bottom: 1px solid var(--w-line);
  }
  .row.crit {
    align-items: flex-start;
  }
  .lbl {
    font-size: var(--w-fs-small);
    font-weight: 500;
  }
  .w-seg {
    flex: none;
  }
  .w-seg button {
    min-width: 34px;
    padding: 4px 10px;
  }
  .w-seg button {
    text-transform: capitalize;
  }
  .w-seg button.fail[aria-pressed="true"] {
    color: var(--w-danger);
  }
  .derived {
    flex: none;
    display: flex;
    align-items: baseline;
    gap: 8px;
    font-size: var(--w-fs-caption);
    color: var(--w-muted);
  }
  .derived b {
    font-size: var(--w-fs-small);
    color: var(--w-ink);
  }
  .bonus-h {
    margin: 12px 0 2px;
  }
  .bonus-h span {
    font-weight: 400;
    text-transform: none;
    letter-spacing: 0;
  }
  .bonus {
    display: flex;
    align-items: baseline;
    gap: 8px;
    padding: 4px 0;
    cursor: pointer;
  }
  .bonus input {
    accent-color: var(--w-accent);
  }
  .bonus-txt {
    display: grid;
  }
  .bonus .w-sub {
    font-size: var(--w-fs-caption);
  }
  .anchors {
    list-style: none;
    margin: 4px 0 0;
    padding: 0;
    display: grid;
    gap: 1px;
    font-size: var(--w-fs-caption);
    color: var(--w-muted);
  }
  .anchors li {
    justify-self: start;
    display: flex;
    gap: 8px;
    padding: 1px 6px;
    margin-left: -6px;
    border-radius: var(--w-r-sm);
  }
  .anchors li.on {
    color: var(--w-ink);
    font-weight: 500;
    background: var(--w-accent-soft);
  }
  .field {
    display: grid;
    gap: 4px;
  }
  .note {
    box-sizing: border-box;
    width: 100%;
    border: 1px solid var(--w-line);
    border-radius: var(--w-r-sm);
    padding: 5px 8px;
    background: none;
    font-size: var(--w-fs-small);
  }
  .note:focus {
    border-color: var(--w-accent);
    background: var(--w-surface);
    outline: none;
  }
  .foot {
    display: flex;
    align-items: center;
    gap: var(--w-s-2);
    padding: 12px 22px;
    border-top: 1px solid var(--w-line);
    background: var(--w-sunk);
  }
  .calc {
    font-size: var(--w-fs-caption);
  }
  .result {
    flex: 1;
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: 2px var(--w-s-3);
    font-size: var(--w-fs-small);
    font-variant-numeric: tabular-nums;
  }
</style>
