<script lang="ts">
  import TaskBoard from "$lib/components/TaskBoard.svelte";
</script>

<TaskBoard>
  {#snippet title()}<h1 class="w-h1">All tasks</h1>{/snippet}
</TaskBoard>
