<script lang="ts">
  import { goto } from "$app/navigation";
  import { agentsEnabled } from "$lib/stores/workspace.svelte";

  // Agent features off: the view does not exist.
  $effect(() => {
    if (!agentsEnabled()) goto("/tasks", { replaceState: true });
  });
</script>

{#if agentsEnabled()}
  <header class="w-page-head">
    <h1 class="w-h1">Agents</h1>
  </header>
{/if}
