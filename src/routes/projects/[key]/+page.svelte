<script lang="ts">
  import { goto } from "$app/navigation";
  import { page } from "$app/state";
  import ProjectFiles from "$lib/components/ProjectFiles.svelte";
  import ProjectForm from "$lib/components/ProjectForm.svelte";
  import TaskBoard from "$lib/components/TaskBoard.svelte";
  import { workspace, stepLabel, projColor, openInObsidian, openProjectInVscode, reveal } from "$lib/stores/workspace.svelte";

  const project = $derived(workspace.index?.projects.find((p) => p.key === page.params.key) ?? null);
  // `/projects/WR` = tasks, `?tab=files` = files; links and the sidebar land on tasks.
  const tab = $derived(page.url.searchParams.get("tab") === "files" ? "files" : "tasks");

  let selected = $state<string | null>(null);
  let editing = $state(false);
</script>

{#if !project}
  <header class="w-page-head">
    <h1 class="w-h1">Project not found</h1>
  </header>
  <p><a href="/projects">Back to projects</a></p>
{:else}
  {#snippet title()}
    <div class="title-row">
      <span class="w-proj-mark big" style:--c={projColor(project.color)}></span>
      <h1 class="w-h1">{project.title}</h1>
      <div class="w-seg" role="group" aria-label="View">
        <button type="button" aria-pressed={tab === "tasks"} onclick={() => goto(`/projects/${project.key}`)}>Tasks</button>
        <button type="button" aria-pressed={tab === "files"} onclick={() => goto(`/projects/${project.key}?tab=files`)}>Files</button>
      </div>
    </div>
  {/snippet}

  {#if tab === "tasks"}
    <!-- A fresh board per project: filters, search and an open card do not carry over. -->
    {#key project.key}
      <TaskBoard {project} {title} />
    {/key}
  {:else}
    <header class="w-page-head">
      <div>
        {@render title()}
        <div class="w-sub">
          <span class="w-mono">{project.key}</span> · <span class="w-mono">{project.path}</span> · {project.status}{#if stepLabel(project)}
            · {stepLabel(project)}{/if}
        </div>
      </div>
      <div class="w-toolbar">
        <button class="w-btn" onclick={() => openInObsidian(`${project.path}/_project.md`)}>Open in Obsidian</button>
        <button class="w-btn" onclick={() => openProjectInVscode(project.key)}>Open in VS Code</button>
        <button class="w-btn" onclick={() => reveal(project.path)}>Reveal in Finder</button>
        <button class="w-btn w-btn--primary" onclick={() => (editing = true)}>Edit</button>
      </div>
    </header>
    <ProjectFiles {project} bind:selected />
  {/if}

  {#if editing}
    <ProjectForm {project} onclose={() => (editing = false)} />
  {/if}
{/if}

<style>
  /* Fixed to the title's line height, so the tabs do not make this head taller than /tasks. */
  .title-row {
    display: flex;
    align-items: center;
    gap: var(--w-s-3);
    height: calc(var(--w-fs-display) * 1.1);
  }
  .title-row .w-seg {
    margin-left: var(--w-s-2);
  }
  .big {
    width: 14px;
    height: 14px;
    border-radius: var(--w-r-sm);
  }
</style>
