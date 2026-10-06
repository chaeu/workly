<script lang="ts">
  import { projColor, type TaskEntry } from "$lib/stores/workspace.svelte";
  import { agentBadge, agentName, isOverdue, projectOf, shortDate } from "$lib/tasks.svelte";

  let { task: t, showProject = true, dim = false, dragging = false }: { task: TaskEntry; showProject?: boolean; dim?: boolean; dragging?: boolean } =
    $props();

  const done = $derived(t.status === "done");
  const badge = $derived(agentBadge(t));
  const project = $derived(projectOf(t));
  const commit = $derived(t.agent?.commit ? `commit ${t.agent.commit.slice(0, 7)}` : null);
  const commitBy = $derived(t.agent?.runner && t.agent.runner !== "auto" ? ` · ${agentName(t.agent.runner)}` : "");
</script>

<div
  class="w-card"
  class:w-card--done={done}
  class:w-card--active={!!badge}
  class:is-dim={dim}
  class:is-dragging={dragging}
  role="button"
  tabindex="0"
  data-id={t.id}
  aria-label="{t.id} {t.title}"
>
  <div class="w-card-meta w-mono">
    <span>{t.id}{done && t.done_at ? ` · ${shortDate(t.done_at)}` : ""}</span>
    {#if t.priority}<span class="w-prio w-prio--{t.priority}">P{t.priority}</span>{/if}
  </div>
  <div class="w-card-title">{t.title}</div>
  {#if t.tags.length}<div class="w-tag">{t.tags.map((x) => `#${x}`).join("  ")}</div>{/if}
  {#if badge}<div class="w-card-agent">{badge}</div>{/if}
  {#if commit}<div class="w-card-code">{commit}{commitBy}</div>{/if}
  {#if showProject || (t.due && !done)}
    <div class="w-card-foot">
      {#if showProject}
        <span class="w-proj-mark" style:--c={project ? projColor(project.color) : "var(--w-line)"}></span>
        <span class="w-card-proj">{project?.title ?? "Inbox"}</span>
      {/if}
      {#if t.due && !done}<span class="w-mono" class:overdue={isOverdue(t)}>due {shortDate(t.due)}</span>{/if}
    </div>
  {/if}
</div>

<style>
  .overdue {
    color: var(--w-warn);
  }
</style>
