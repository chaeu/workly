<script lang="ts">
  import { goto } from "$app/navigation";
  import { page } from "$app/state";
  import ProjectFiles from "$lib/components/ProjectFiles.svelte";
  import ProjectForm from "$lib/components/ProjectForm.svelte";
  import ProcessError from "$lib/components/ProcessError.svelte";
  import UseCaseDetail from "$lib/components/UseCaseDetail.svelte";
  import TaskBoard from "$lib/components/TaskBoard.svelte";
  import { renderMarkdown, resolveLink } from "$lib/markdown";
  import {
    workspace,
    stepLabel,
    projColor,
    openTasks,
    openInObsidian,
    openProjectInVscode,
    openRepo,
    openUrl,
    readMarkdown,
    reveal,
    moveUseCase,
  } from "$lib/stores/workspace.svelte";
  import { fmtFte, label, PROCESS, savedFte, type UC } from "$lib/usecases.svelte";

  const TABS = ["overview", "usecase", "tasks", "files"] as const;
  type Tab = (typeof TABS)[number];

  const project = $derived(workspace.index?.projects.find((p) => p.key === page.params.key) ?? null);
  // No parameter = tasks, so older links keep working. The sidebar link remembers the last tab (layout).
  const asked = $derived(TABS.find((t) => t === page.url.searchParams.get("tab")) ?? "tasks");
  // Use case only while the project has a usecase block; otherwise (e.g. after "Remove from cockpit") Overview.
  const tab = $derived(asked === "usecase" && !project?.usecase ? "overview" : asked);
  const tabHref = (t: Tab) => `/projects/${project?.key}${t === "tasks" ? "" : `?tab=${t}`}`;
  $effect(() => {
    if (project && tab !== asked) goto(tabHref(tab), { replaceState: true });
  });

  let selected = $state<string | null>(null);
  let editing = $state(false);

  // Overview: body of _project.md, reloaded on every index change so edits in Obsidian show up live.
  const overviewPath = $derived(project ? `${project.path}/_project.md` : "");
  let html = $state("");
  let descError = $state<string | null>(null);
  $effect(() => {
    void workspace.reloads;
    const path = overviewPath;
    if (tab !== "overview" || !path) return;
    readMarkdown(path).then(
      (src) => {
        if (path !== overviewPath) return;
        html = src.trim() ? renderMarkdown(src) : "";
        descError = null;
      },
      (e) => (descError = String(e)),
    );
  });

  // Same rule as the Files preview: links never navigate the app window; a project file opens in the Files tab.
  function onDescClick(e: MouseEvent) {
    const a = (e.target as HTMLElement).closest("a");
    if (!a || !project) return;
    e.preventDefault();
    const href = a.getAttribute("href") ?? "";
    if (/^(https?|mailto):/i.test(href)) openUrl(href);
    else if (href && !href.includes(":") && !href.startsWith("#")) {
      const target = resolveLink(overviewPath, href);
      if (target.startsWith(project.path + "/")) {
        selected = target;
        goto(tabHref("files"));
      }
    }
  }

  const uc = $derived(project?.usecase ? (project as UC) : null);
  const fte = $derived(uc ? savedFte(uc) : null);
  const proc = $derived(workspace.index?.process ?? null);
  const procBroken = $derived(!proc || !!workspace.index?.errors.some((e) => e.path === PROCESS));
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
        <button type="button" aria-pressed={tab === "overview"} onclick={() => goto(tabHref("overview"))}>Overview</button>
        {#if project.usecase}
          <button type="button" aria-pressed={tab === "usecase"} onclick={() => goto(tabHref("usecase"))}>Use case</button>
        {/if}
        <button type="button" aria-pressed={tab === "tasks"} onclick={() => goto(tabHref("tasks"))}>Tasks</button>
        <button type="button" aria-pressed={tab === "files"} onclick={() => goto(tabHref("files"))}>Files</button>
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
          <span class="w-mono">{project.key}</span> · <span class="w-mono">{project.path}</span> · {project.status}{stepLabel(project) ? ` · ${stepLabel(project)}` : ""}
        </div>
      </div>
      <div class="w-toolbar">
        <button class="w-btn" onclick={() => openInObsidian(overviewPath)}>Open in Obsidian</button>
        <button class="w-btn" onclick={() => openProjectInVscode(project.key)}>Open in VS Code</button>
        <button class="w-btn" onclick={() => reveal(project.path)}>Reveal in Finder</button>
        <button class="w-btn w-btn--primary" onclick={() => (editing = true)}>Edit</button>
      </div>
    </header>
    {@render bar()}
    {#if tab === "files"}
      <ProjectFiles {project} bind:selected />
    {:else if tab === "usecase"}
      {#if proc && !procBroken}
        <!-- No toast here: the new step shows right away. -->
        <UseCaseDetail key={project.key} process={proc} mode="page" onmove={(u, step) => step !== u.usecase.step && moveUseCase(u.key, step)} />
      {:else}
        <ProcessError />
      {/if}
    {:else}
      <div class="overview">
        <section class="desc" aria-label="Description">
          {#if descError}
            <p class="w-sub">{descError}</p>
          {:else if html}
            <!-- Sanitised by DOMPurify in renderMarkdown. -->
            <!-- eslint-disable-next-line svelte/no-at-html-tags -->
            <!-- svelte-ignore a11y_click_events_have_key_events, a11y_no_static_element_interactions -->
            <div class="md" onclick={onDescClick}>{@html html}</div>
          {:else}
            <p class="w-sub">No description in _project.md.</p>
          {/if}
        </section>

        <aside class="props" aria-label="Properties">
          <dl>
            <dt class="w-caps">Key</dt>
            <dd class="w-mono">{project.key}</dd>
            <dt class="w-caps">Status</dt>
            <dd>{project.status}</dd>
            <dt class="w-caps">Colour</dt>
            <dd><span class="w-proj-mark swatch" style:--c={projColor(project.color)}></span>{project.color ?? "–"}</dd>
            <dt class="w-caps">Path</dt>
            <dd class="w-mono small">{project.path}</dd>
            <dt class="w-caps">Created</dt>
            <dd>{project.created ?? "–"}</dd>
            <dt class="w-caps">Open tasks</dt>
            <dd><a href={tabHref("tasks")}>{openTasks(project)}</a></dd>
            <dt class="w-caps">Repos</dt>
            <dd>
              {#each project.repos as r (r)}
                {#if workspace.index?.missing_repos.some((m) => m.key === project.key && m.repo === r)}
                  <span class="item w-mono small missing" title="This folder does not exist on this Mac">{r} · not found</span>
                {:else}
                  <button class="item link w-mono small" title="Open in VS Code" onclick={() => openRepo(r)}>{r}</button>
                {/if}
              {:else}
                <span class="w-sub">–</span>
              {/each}
            </dd>
            <dt class="w-caps">Links</dt>
            <dd>
              {#each project.links as l (l.url)}
                <button class="item link" title={l.url} onclick={() => openUrl(l.url)}>{l.label ?? l.url}</button>
              {:else}
                <span class="w-sub">–</span>
              {/each}
            </dd>
          </dl>

          <div class="uc">
            {#if uc}
              <div class="w-caps">Use case</div>
              <dl>
                <dt>Step</dt>
                <dd>{stepLabel(uc)}</dd>
                <dt>Status</dt>
                <dd>{workspace.index?.process ? label(workspace.index.process.statuses, uc.usecase.status) : (uc.usecase.status ?? "–")}</dd>
                <dt>Manual effort</dt>
                <dd>{fte === null ? "–" : `${fmtFte(fte)} FTE`}</dd>
              </dl>
              <a href={tabHref("usecase")}>Open use case</a>
            {:else}
              <button class="w-btn" onclick={() => (editing = true)}>Make use case…</button>
            {/if}
          </div>
        </aside>
      </div>
    {/if}
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
  /* Fills the page below the bar like the Files tab; both columns scroll on their own. */
  .overview {
    flex: 1;
    min-height: 300px;
    display: grid;
    grid-template-columns: minmax(0, 1fr) 300px;
    grid-template-rows: minmax(0, 1fr);
    gap: var(--w-s-4);
  }
  .desc,
  .props {
    background: var(--w-surface);
    border-radius: var(--w-r-lg);
    box-shadow: var(--w-shadow-panel);
    overflow-y: auto;
  }
  .desc {
    padding: var(--w-s-3) var(--w-s-6) var(--w-s-6);
  }
  .props {
    padding: var(--w-s-4) var(--w-s-5);
    display: flex;
    flex-direction: column;
    gap: var(--w-s-5);
  }
  dl {
    margin: 0;
    display: grid;
    grid-template-columns: auto minmax(0, 1fr);
    gap: var(--w-s-2) var(--w-s-4);
    align-items: baseline;
  }
  dt {
    color: var(--w-muted);
  }
  dd {
    margin: 0;
    min-width: 0;
    font-size: var(--w-fs-small);
    overflow-wrap: anywhere;
  }
  .swatch {
    display: inline-block;
    margin-right: var(--w-s-2);
  }
  .small {
    font-size: var(--w-fs-micro);
  }
  .item {
    display: block;
  }
  .link {
    padding: 0;
    border: 0;
    background: none;
    color: var(--w-accent);
    text-align: left;
    cursor: pointer;
  }
  .link:hover {
    text-decoration: underline;
  }
  .missing {
    color: var(--w-warn);
  }
  a {
    color: var(--w-accent);
  }
  .uc {
    display: flex;
    flex-direction: column;
    align-items: flex-start;
    gap: var(--w-s-2);
    padding-top: var(--w-s-4);
    border-top: 1px solid var(--w-line);
  }
  .uc dl {
    align-self: stretch;
  }
  .uc dt {
    font-size: var(--w-fs-small);
  }
</style>
