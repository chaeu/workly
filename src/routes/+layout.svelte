<script lang="ts">
  import "@fontsource/ibm-plex-sans/latin-400.css";
  import "@fontsource/ibm-plex-sans/latin-500.css";
  import "@fontsource/ibm-plex-sans/latin-600.css";
  import "@fontsource/ibm-plex-mono/latin-400.css";
  import "@fontsource/ibm-plex-mono/latin-500.css";
  import "$lib/styles/tokens.css";
  import "$lib/styles/components.css";
  import { goto } from "$app/navigation";
  import { page } from "$app/state";
  import { getCurrentWindow } from "@tauri-apps/api/window";

  let { children } = $props();

  const views = [
    { href: "/tasks", label: "Tasks" },
    { href: "/projects", label: "Projects" },
    { href: "/use-cases", label: "Use cases" },
    { href: "/agents", label: "Agents" },
  ];

  type Theme = "system" | "light" | "dark";
  const themes: Theme[] = ["system", "light", "dark"];
  // ponytail: in memory only, persisted with device settings later
  let theme = $state<Theme>("system");

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
    <div class="w-sidebar-foot">
      <div class="w-seg" role="group" aria-label="Theme">
        {#each themes as t (t)}
          <button type="button" aria-pressed={theme === t} onclick={() => (theme = t)}>{t[0].toUpperCase() + t.slice(1)}</button>
        {/each}
      </div>
      <!-- ponytail: placeholder until M1 loads a workspace -->
      <div class="w-mono">No workspace</div>
    </div>
  </nav>

  <main class="w-main">
    {@render children()}
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
  .w-sidebar-foot .w-mono {
    padding: 0 10px;
  }
  .titlebar {
    position: fixed;
    inset: 0 0 auto 0;
    height: 28px;
    z-index: 100;
  }
</style>
