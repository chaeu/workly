<script lang="ts">
  import { workspace, openInVscode } from "$lib/stores/workspace.svelte";
  import { PROCESS } from "$lib/usecases.svelte";

  // Shown instead of a use-case view while process.yml is missing or broken.
  const errors = $derived(workspace.index?.errors.filter((e) => e.path === PROCESS) ?? []);
</script>

<section class="proc-error" role="alert">
  <h2 class="w-h2">{workspace.index?.process || errors.length ? `${PROCESS} has a problem` : `This workspace has no ${PROCESS}`}</h2>
  {#if errors.length}
    <ul>
      {#each errors as e, i (i)}
        <li>{#if e.line}<span class="w-mono">line {e.line}</span>{/if}{e.message}</li>
      {/each}
    </ul>
  {/if}
  <p class="w-sub">The cockpit is drawn from this file. Fix it and save; the views update on their own.</p>
  {#if errors.length}
    <div><button type="button" class="w-btn" onclick={() => openInVscode(PROCESS)}>Open in VS Code</button></div>
  {/if}
</section>

<style>
  .proc-error {
    background: var(--w-danger-soft);
    box-shadow: inset 0 0 0 1px var(--w-danger);
    border-radius: var(--w-r-lg);
    padding: 16px 20px;
    display: grid;
    gap: 10px;
    user-select: text;
    -webkit-user-select: text;
  }
  .proc-error h2 {
    margin: 0;
    color: var(--w-danger);
  }
  .proc-error ul {
    margin: 0;
    padding-left: 18px;
    display: grid;
    gap: 4px;
    font-size: var(--w-fs-small);
  }
  .proc-error li .w-mono {
    margin-right: 8px;
    color: var(--w-muted);
  }
  .proc-error p {
    margin: 0;
  }
</style>
