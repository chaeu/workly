<script lang="ts">
  // Manual effort of one use case as a table. Every complete change writes the whole list right away.
  import { workspace, updateProjectField, type Saving } from "$lib/stores/workspace.svelte";
  import { fmtFte, fmtHours, fteHours, PER_YEAR, savedHours, savingHours, type UC } from "$lib/usecases.svelte";

  let { key, onclose }: { key: string; onclose: () => void } = $props();

  const u = $derived(workspace.index?.projects.find((x) => x.key === key && x.usecase) as UC | undefined);
  const uc = $derived(u?.usecase);
  const hours = $derived(u ? savedHours(u) : 0);
  $effect(() => {
    if (workspace.index && !u) onclose();
  });

  async function save(field: string, value: unknown) {
    try {
      await updateProjectField(key, field, value);
    } catch (e) {
      workspace.error = String(e);
    }
  }

  // A new row stays a local draft until it is complete. Opening with no rows starts one.
  const PERS = Object.keys(PER_YEAR);
  const blank = (): Saving => ({ what: "", count: null, per: "month", minutes: null });
  // svelte-ignore state_referenced_locally
  let draft = $state<Saving | null>(uc?.savings.length ? null : blank());
  const complete = (s: Saving) => !!s.what?.trim() && s.count != null && s.count >= 0 && s.minutes != null && s.minutes >= 0 && !!s.per;
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

  function onkeydown(e: KeyboardEvent) {
    if (e.key === "Escape") onclose();
  }
</script>

<svelte:window {onkeydown} />

{#if u && uc}
  <div class="w-scrim" role="presentation" onclick={onclose}></div>
  <div class="w-modal" role="dialog" aria-modal="true" aria-labelledby="ee-title">
    <div class="head">
      <span class="w-mono">{u.key}</span>
      <h2 class="w-h2" id="ee-title">Manual effort today</h2>
      <button type="button" class="rm x" aria-label="Close" onclick={onclose}>✕</button>
    </div>
    <div class="body">
      <table>
        <thead>
          <tr><th>Activity</th><th class="num">Count</th><th class="per">Per</th><th class="num">Minutes</th><th class="num">h/yr</th><th></th></tr>
        </thead>
        <tbody>
          {#each [...uc.savings, ...(draft ? [draft] : [])] as s, i (i)}
            {@const r = i < uc.savings.length ? i : -1}
            {@const h = savingHours(s)}
            <!-- {#key} resets the inputs to the file's values after every reload. -->
            {#key workspace.reloads}
              <tr>
                <td><input value={s.what ?? ""} title={s.what ?? ""} placeholder="What is done by hand?" aria-label="Activity" onchange={cell(r, "what")} {@attach (el) => { if (r < 0 && !s.what) el.focus(); }} /></td>
                <td class="num"><input type="number" min="0" step="any" value={s.count ?? ""} aria-label="Count" onchange={cell(r, "count")} /></td>
                <td class="per">
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
        <button type="button" class="w-btn w-btn--quiet" onclick={() => (draft ??= blank())}>+ Add activity</button>
        <span><b>≈ {fmtFte(hours / fteHours())} FTE</b> · {fmtHours(hours)} h/yr</span>
      </div>
      <label class="field">
        <span class="w-caps">Where do the numbers come from?</span>
        {#key uc.savings_note}
          <input class="note" value={uc.savings_note ?? ""} placeholder="Source, sample, date" onchange={(e) => save("usecase.savings_note", e.currentTarget.value.trim() || null)} />
        {/key}
      </label>
      <p class="w-sub hint">Per year: month × {PER_YEAR.month}, week × {PER_YEAR.week}, day × {PER_YEAR.day} · 1 FTE = {fmtHours(fteHours())} h (Settings)</p>
    </div>
    <div class="foot">
      <span class="w-sub">Each change is saved to _project.md right away.</span>
      <button type="button" class="w-btn w-btn--primary" onclick={onclose}>Done</button>
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
  }
  .body {
    overflow-y: auto;
    padding: 8px 22px 18px;
    display: grid;
    gap: 10px;
  }
  table {
    width: 100%;
    border-collapse: collapse;
    font-size: var(--w-fs-small);
  }
  th {
    text-align: left;
    font-weight: 500;
    font-size: var(--w-fs-caption);
    color: var(--w-muted);
    padding: 0 4px 2px;
    border-bottom: 1px solid var(--w-line);
  }
  td {
    padding: 2px 0;
    border-bottom: 1px solid var(--w-line);
  }
  .num {
    text-align: right;
    width: 72px;
  }
  .per {
    width: 92px;
  }
  td.w-mono {
    padding-right: 4px;
    color: var(--w-muted);
  }
  td:last-child {
    width: 26px;
  }
  td input,
  td select,
  .note {
    width: 100%;
    box-sizing: border-box;
    border: 1px solid transparent;
    border-radius: var(--w-r-sm);
    padding: 4px 6px;
    background: none;
    font-size: var(--w-fs-small);
    font-variant-numeric: tabular-nums;
  }
  .num input {
    text-align: right;
  }
  td input:hover,
  td select:hover,
  .note {
    border-color: var(--w-line);
  }
  td input:focus,
  td select:focus,
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
    width: 24px;
    height: 24px;
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
  .field {
    display: grid;
    gap: 4px;
    margin-top: 6px;
  }
  .hint {
    margin: 0;
    font-size: var(--w-fs-caption);
  }
  .foot {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: var(--w-s-3);
    padding: 12px 22px;
    border-top: 1px solid var(--w-line);
    background: var(--w-sunk);
  }
</style>
