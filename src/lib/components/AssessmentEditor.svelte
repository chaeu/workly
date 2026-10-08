<script lang="ts">
  // Assessment of one use case against the method in process.yml. A local draft; Save writes the whole
  // block at once (one write, one log line), Clear removes it.
  import { ask } from "@tauri-apps/plugin-dialog";
  import AssessmentPill from "$lib/components/AssessmentPill.svelte";
  import { workspace, updateProjectField, type KoValue, type Process, type UseCaseAssessment } from "$lib/stores/workspace.svelte";
  import { today } from "$lib/tasks.svelte";
  import { assess, scoreLine, type UC } from "$lib/usecases.svelte";

  let { key, process: p, onclose }: { key: string; process: Process; onclose: () => void } = $props();

  const u = $derived(workspace.index?.projects.find((x) => x.key === key && x.usecase) as UC | undefined);
  $effect(() => {
    if (workspace.index && !u) onclose();
  });

  // ponytail: assumes p.assessment exists; the callers only open the editor when it does.
  const m = $derived(p.assessment!);
  // Only ids the method knows: the core refuses unknown ones. Every K.O. question is written, open by default.
  // svelte-ignore state_referenced_locally
  const cur = u?.usecase.assessment;
  // svelte-ignore state_referenced_locally
  let draft = $state<UseCaseAssessment>({
    date: null,
    ko: Object.fromEntries(p.assessment!.ko.map((k) => [k.id, cur?.ko[k.id] ?? "open"])),
    scores: Object.fromEntries(p.assessment!.criteria.filter((c) => cur?.scores[c.id] != null).map((c) => [c.id, cur!.scores[c.id]])),
    note: cur?.note ?? "",
  });
  const r = $derived(assess(p, draft));

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
  const save = () => write({ ...$state.snapshot(draft), date: today(), note: draft.note?.trim() || null });
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
      {#each AXES as [axis, name] (axis)}
        <section>
          <h3 class="w-caps">{name}</h3>
          {#each m.criteria.filter((c) => c.axis === axis) as c (c.id)}
            {@const s = draft.scores[c.id] ?? null}
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
              <div class="w-seg" role="group" aria-label={c.label}>
                {#each [1, 2, 3, null] as n (n)}
                  <button type="button" aria-pressed={s === n} title={n === null ? "Open" : c.anchors[n - 1]} onclick={() => setScore(c.id, n)}>{n ?? "?"}</button>
                {/each}
              </div>
            </div>
          {/each}
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
        <span>{scoreLine(r)}</span>
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
  .result {
    flex: 1;
    display: flex;
    align-items: center;
    gap: var(--w-s-3);
    font-size: var(--w-fs-small);
    font-variant-numeric: tabular-nums;
  }
</style>
