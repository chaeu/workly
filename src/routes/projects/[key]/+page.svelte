<script lang="ts">
  import { goto } from "$app/navigation";
  import { page } from "$app/state";
  import ProjectFiles from "$lib/components/ProjectFiles.svelte";
  import ProjectForm from "$lib/components/ProjectForm.svelte";
  import TaskBoard from "$lib/components/TaskBoard.svelte";
  import { workspace, stepLabel, projColor, openInObsidian, openProjectInVscode, reveal } from "$lib/stores/workspace.svelte";

  const project = $derived(workspace.index?.projects.find((p) => p.key === page.params.key) ?? null);
  // `/projects/WR` = tasks, `?tab=files` = files. The sidebar link remembers the last tab (layout).
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
    <h1 class="w-h1"><span class="w-proj-mark big" style:--c={projColor(project.color)}></span>{project.title}</h1>
  {/snippet}

  <!-- Same place and height as the focus strip on /tasks, so nothing moves between views. -->
  {#snippet bar()}
    <section class="w-bar" aria-label="Project">
      <div class="w-seg" role="group" aria-label="View">
        <button type="button" aria-pressed={tab === "tasks"} onclick={() => goto(`/projects/${project.key}`)}>Tasks</button>
        <button type="button" aria-pressed={tab === "files"} onclick={() => goto(`/projects/${project.key}?tab=files`)}>Files</button>
      </div>
    </section>
  {/snippet}

  {#if tab === "tasks"}
    <!-- A fresh board per project: filters, search and an open card do not carry over. -->
    {#key project.key}
      <TaskBoard {project} {title} {bar} />
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
    {@render bar()}
    <ProjectFiles {project} bind:selected />
  {/if}

  {#if editing}
    <ProjectForm {project} onclose={() => (editing = false)} />
  {/if}
{/if}

<style>
  .big {
    display: inline-block;
    width: 14px;
    height: 14px;
    margin-right: var(--w-s-3);
    border-radius: var(--w-r-sm);
    vertical-align: middle;
  }
  .w-bar .w-seg {
    align-self: flex-start;
  }
</style>
