<script lang="ts">
  // Quadrant or K.O. of one assessment; the tooltip carries V/F and the open items.
  import { tip } from "$lib/tip";
  import { assessTip, QUADRANT_COLOR, verdict, type AssessResult } from "$lib/usecases.svelte";

  let { result: r }: { result: AssessResult } = $props();
  const v = $derived(verdict(r));
</script>

<span class="aq" style:--c={v === "Incomplete" ? "var(--w-muted)" : QUADRANT_COLOR[v]} use:tip={assessTip(r)}>{v}</span>

<style>
  .aq {
    justify-self: start;
    font-family: var(--w-font-label);
    font-size: var(--w-fs-caption);
    font-weight: 600;
    white-space: nowrap;
    color: var(--c);
    box-shadow: inset 0 0 0 1px var(--c);
    border-radius: var(--w-r-pill);
    padding: 0 8px;
  }
</style>
