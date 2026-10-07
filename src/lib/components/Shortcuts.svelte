<script lang="ts">
  let { views, onclose }: { views: string[]; onclose: () => void } = $props();

  const keys = $derived([
    ["⌘N", "New task"],
    ["⌘K", "Search"],
    ...views.map((v, i) => [`⌘${i + 1}`, v]),
    ["⌘,", "Settings"],
    ["Enter", "Open the focused card"],
    ["Esc", "Close card or dialog"],
    ["⌘⌫", "Delete the open or focused task"],
    ["?", "This list"],
  ]);

  let el: HTMLElement;
  $effect(() => el.focus());
</script>

<div class="w-scrim" role="presentation" onclick={onclose}></div>
<!-- Esc closes only this dialog, not a card below it. -->
<div
  bind:this={el}
  class="w-modal"
  role="dialog"
  aria-modal="true"
  aria-labelledby="sc-title"
  tabindex="-1"
  onkeydown={(e) => {
    if (e.key === "Escape" || e.key === "?") {
      e.stopPropagation();
      onclose();
    }
  }}
>
  <div class="body">
    <h2 class="w-h2" id="sc-title">Keyboard shortcuts</h2>
    <dl>
      {#each keys as [key, what] (key)}
        <dt class="w-mono">{key}</dt>
        <dd>{what}</dd>
      {/each}
    </dl>
  </div>
</div>

<style>
  .w-modal {
    width: min(420px, calc(100% - 32px));
    outline: none;
  }
  .body {
    display: flex;
    flex-direction: column;
    gap: var(--w-s-4);
    padding: var(--w-s-5) var(--w-s-6) var(--w-s-6);
  }
  dl {
    display: grid;
    grid-template-columns: auto 1fr;
    gap: var(--w-s-2) var(--w-s-4);
    margin: 0;
  }
  dt {
    justify-self: start;
    padding: 2px 7px;
    border-radius: var(--w-r-sm);
    background: var(--w-sunk);
    color: var(--w-ink);
  }
  dd {
    margin: 0;
    font-size: var(--w-fs-small);
  }
</style>
