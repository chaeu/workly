<script lang="ts">
  import { goto } from "$app/navigation";
  import { page } from "$app/state";
  import AgentActivity from "$lib/components/AgentActivity.svelte";
  import AgentOverview from "$lib/components/AgentOverview.svelte";
  import AgentSetup from "$lib/components/AgentSetup.svelte";
  import TaskDetail from "$lib/components/TaskDetail.svelte";
  import { agentsEnabled, workspace } from "$lib/stores/workspace.svelte";
  import { agentInbox, today } from "$lib/tasks.svelte";

  // Agent features off: the view does not exist.
  $effect(() => {
    if (!agentsEnabled()) goto("/tasks", { replaceState: true });
  });

  const TABS = ["overview", "activity", "setup"] as const;
  type Tab = (typeof TABS)[number];
  const tab = $derived(TABS.find((t) => t === page.url.searchParams.get("tab")) ?? "overview");
  const go = (t: Tab) => goto(t === "overview" ? "/agents" : `/agents?tab=${t}`, { replaceState: true });

  const inbox = $derived(agentInbox());

  let openId = $state<string | null>(null);
  const detailMode = $derived(workspace.settings?.task_detail ?? "popup");
  const focusIds = $derived(
    (workspace.index?.tasks ?? [])
      .filter((t) => t.focus?.slice(0, 10) === today())
      .sort((a, b) => (a.focus_order ?? 9) - (b.focus_order ?? 9))
      .map((t) => t.id),
  );
  const open = (id: string) => (openId = id);
</script>

{#if agentsEnabled()}
  <header class="w-page-head">
    <div>
      <h1 class="w-h1">Agents</h1>
      <p class="w-sub">{inbox.count} {inbox.count === 1 ? "needs" : "need"} you · {inbox.running.length} running</p>
    </div>
  </header>

  <!-- Same place and height as the focus strip on /tasks and the project tabs. -->
  <section class="w-bar" aria-label="Agents">
    <div class="w-seg" role="group" aria-label="View">
      <button type="button" aria-pressed={tab === "overview"} onclick={() => go("overview")}
        >Overview{#if inbox.count}<span class="w-mono n">{inbox.count}</span>{/if}</button
      >
      <button type="button" aria-pressed={tab === "activity"} onclick={() => go("activity")}>Activity</button>
      <button type="button" aria-pressed={tab === "setup"} onclick={() => go("setup")}>Setup</button>
    </div>
  </section>

  {#if tab === "overview"}
    <AgentOverview onopen={open} />
  {:else if tab === "activity"}
    <AgentActivity onopen={open} />
  {:else}
    <AgentSetup />
  {/if}

  {#if openId}
    <TaskDetail id={openId} mode={detailMode} {focusIds} onclose={() => (openId = null)} onopen={open} />
  {/if}
{/if}

<style>
  .w-page-head p {
    margin: 4px 0 0;
  }
  .n {
    margin-left: 6px;
    color: var(--w-accent);
  }
</style>
