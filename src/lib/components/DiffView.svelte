<script lang="ts">
  /** A unified diff as `change_diff` returns it (core cuts it at 400 lines). `max` = CSS max-height, "none" = grow with the page. */
  let { text, max = "320px" }: { text: string; max?: string } = $props();

  const lineClass = (l: string) =>
    /^(\+\+\+|---|diff |index |new file|deleted file)/.test(l) ? "meta" : l.startsWith("@@") ? "hunk" : l[0] === "+" ? "add" : l[0] === "-" ? "del" : "";
</script>

<pre class="diff" style:max-height={max}>{#each text.split("\n") as l, i (i)}<span class={lineClass(l)}>{l}</span>{/each}</pre>

<style>
  .diff {
    margin: 0;
    overflow: auto;
    border-radius: var(--w-r-sm);
    padding: var(--w-s-2);
    background: var(--w-sunk);
    font-family: var(--w-font-mono);
    font-size: var(--w-fs-micro);
    user-select: text;
    -webkit-user-select: text;
  }
  span {
    display: block;
    min-height: 1lh;
    white-space: pre;
  }
  .add {
    color: var(--w-ok);
  }
  .del {
    color: var(--w-danger);
  }
  .hunk {
    color: var(--w-info);
  }
  .meta {
    color: var(--w-muted);
  }
</style>
