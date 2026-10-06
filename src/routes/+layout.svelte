<script lang="ts">
  import "@fontsource/ibm-plex-sans/latin-400.css";
  import "@fontsource/ibm-plex-sans/latin-500.css";
  import "@fontsource/ibm-plex-sans/latin-600.css";
  import "@fontsource/ibm-plex-mono/latin-400.css";
  import "@fontsource/ibm-plex-mono/latin-500.css";
  import "$lib/styles/tokens.css";
  import "$lib/styles/components.css";
  import "$lib/styles/markdown.css";
  import { goto } from "$app/navigation";
  import { page } from "$app/state";
  import { getCurrentWindow } from "@tauri-apps/api/window";
  import {
    workspace,
    startWorkspace,
    openTasks,
    projColor,
    tilde,
    chooseWorkspace,
    createWorkspace,
    openWorkspace,
    agentsEnabled,
  } from "$lib/stores/workspace.svelte";

  startWorkspace();

  let { children } = $props();

  const ALL_VIEWS = [
    { href: "/tasks", label: "Tasks" },
    { href: "/projects", label: "Projects" },
    { href: "/use-cases", label: "Use cases" },
    { href: "/agents", label: "Agents" },
  ];
  // Agent features off: no Agents item and no ⌘4.
  const views = $derived(ALL_VIEWS.filter((v) => v.href !== "/agents" || agentsEnabled()));

  const theme = $derived(workspace.settings?.theme ?? "system");
  const sidebarProjects = $derived(workspace.index?.projects.filter((p) => p.status !== "archived") ?? []);

  $effect(() => {
    const root = document.documentElement;
    if (theme === "system") delete root.dataset.theme;
    else root.dataset.theme = theme;
    // Native title bar and traffic lights follow the window appearance.
    getCurrentWindow().setTheme(theme === "system" ? null : theme);
  });

  function onkeydown(e: KeyboardEvent) {
    if (!e.metaKey || e.shiftKey || e.altKey || e.ctrlKey) return;
    // Hidden debug view.
    if (e.key === "0") {
      e.preventDefault();
      goto("/debug");
      return;
    }
    const view = views[Number(e.key) - 1];
    if (view) {
      e.preventDefault();
      goto(view.href);
    }
  }
</script>

<svelte:window {onkeydown} />

<div class="titlebar" data-tauri-drag-region></div>

<div class="w-app">
  <nav class="w-sidebar" aria-label="Navigation">
    <div class="w-brand">
      <svg class="w-brand-mark" viewBox="160 160 704 704" aria-hidden="true"
        ><rect x="272" y="272" width="480" height="480" rx="64" fill="var(--w-accent)" transform="rotate(45 512 512)" /><path
          d="M336 436 L424 624 L512 500 L600 624 L688 436"
          fill="none"
          stroke="var(--w-on-accent)"
          stroke-width="84"
          stroke-linecap="round"
          stroke-linejoin="round"
        /></svg
      >Workly
    </div>
    <div class="w-nav">
      {#each views as view (view.href)}
        <a
          class="w-nav-item"
          href={view.href}
          aria-current={page.url.pathname.startsWith(view.href) ? "page" : undefined}>{view.label}</a
        >
      {/each}
    </div>
    {#if sidebarProjects.length}
      <div class="w-nav">
        <div class="w-caps side-label">Projects</div>
        {#each sidebarProjects as p (p.key)}
          <a class="w-nav-item" href="/projects/{p.key}" aria-current={page.url.pathname === `/projects/${p.key}` ? "page" : undefined}
            ><span class="w-proj-mark" style:--c={projColor(p.color)}></span><span class="side-title">{p.title}</span><span
              class="w-count w-mono">{openTasks(p)}</span
            ></a
          >
        {/each}
      </div>
    {/if}
    <div class="w-sidebar-foot">
      <a class="w-nav-item" href="/settings" aria-current={page.url.pathname === "/settings" ? "page" : undefined}>Settings</a>
      <div class="w-mono foot-path" title={workspace.index?.root}>{workspace.index ? tilde(workspace.index.root) : "No workspace"}</div>
    </div>
  </nav>

  <main class="w-main">
    {#if workspace.error}
      <div class="banner" role="alert">
        <span>{workspace.error}</span>
        <button class="w-btn w-btn--quiet" onclick={() => (workspace.error = null)}>Dismiss</button>
      </div>
    {/if}
    {#if !workspace.ready}
      <!-- first load, avoids flashing the onboarding -->
    {:else if workspace.index || page.url.pathname === "/settings"}
      {@render children()}
    {:else}
      <header class="w-page-head">
        <div>
          <h1 class="w-h1">Welcome to Workly</h1>
          <p class="w-sub">Pick a folder with your Markdown files, or an empty one. Workly adds _templates/ and .workly/ to it.</p>
        </div>
      </header>
      <div class="onboarding">
        <button class="w-btn" onclick={chooseWorkspace}>Choose folder…</button>
        <button class="w-btn w-btn--primary" onclick={createWorkspace}>Create new workspace…</button>
      </div>
      {#if workspace.settings?.workspaces.length}
        <div class="known">
          <div class="w-caps">Known workspaces</div>
          {#each workspace.settings.workspaces as w (w.path)}
            <button class="w-btn w-btn--quiet known-row" onclick={() => openWorkspace(w.path)}
              >{w.name} <span class="w-mono">{tilde(w.path)}</span></button
            >
          {/each}
        </div>
      {/if}
    {/if}
  </main>
</div>

<style>
  :global(html, body) {
    height: 100%;
    margin: 0;
    background: var(--w-bg);
  }
  :global(.w-app) {
    height: 100%;
    flex-wrap: nowrap;
    cursor: default;
    -webkit-user-select: none;
    user-select: none;
  }
  :global(.w-main) {
    overflow: auto;
  }
  /* Room for the overlay traffic lights above the brand. */
  .w-sidebar {
    padding-top: 52px;
    overflow-y: auto;
  }
  .w-brand-mark {
    width: 24px;
    height: 24px;
  }
  .w-sidebar-foot {
    display: flex;
    flex-direction: column;
    gap: var(--w-s-3);
    padding: 0;
  }
  .foot-path {
    padding: 0 10px;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .side-label {
    padding: 0 10px var(--w-s-2);
  }
  .side-title {
    min-width: 0;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .banner {
    display: flex;
    align-items: center;
    gap: var(--w-s-3);
    padding: var(--w-s-2) var(--w-s-3);
    border-radius: var(--w-r-md);
    background: var(--w-danger-soft);
    color: var(--w-danger);
    font-size: var(--w-fs-small);
    user-select: text;
  }
  .banner span {
    flex: 1;
  }
  .onboarding {
    display: flex;
    gap: var(--w-s-2);
  }
  .known {
    display: flex;
    flex-direction: column;
    align-items: flex-start;
    gap: var(--w-s-1);
  }
  .known-row {
    display: flex;
    gap: var(--w-s-3);
  }
  .titlebar {
    position: fixed;
    inset: 0 0 auto 0;
    height: 28px;
    z-index: 100;
  }
</style>
