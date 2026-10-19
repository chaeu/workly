<script lang="ts">
  import { goto } from "$app/navigation";
  import guide from "$lib/help/guide.md?raw";
  import agents from "$lib/help/agents.md?raw";
  import { isWebLink, renderMarkdown } from "$lib/markdown";
  import { shortcuts } from "$lib/tasks.svelte";
  import { agentsEnabled, openUrl } from "$lib/stores/workspace.svelte";

  // Agent help sits behind the agent switch (rule 11).
  const html = $derived(renderMarkdown(agentsEnabled() ? `${guide}\n\n${agents}` : guide));

  // Links never navigate the window: app routes via goto, web links in the browser.
  function onclick(e: MouseEvent) {
    const a = (e.target as HTMLElement).closest("a");
    if (!a) return;
    e.preventDefault();
    const href = a.getAttribute("href") ?? "";
    if (href === "#shortcuts") shortcuts.open = true;
    else if (href.startsWith("/")) goto(href);
    else if (isWebLink(href)) openUrl(href);
  }
</script>

<header class="w-page-head">
  <div>
    <h1 class="w-h1">Help</h1>
    <p class="w-sub">How Workly works, in a few minutes of reading.</p>
  </div>
</header>

<!-- Our own text, sanitised by DOMPurify in renderMarkdown anyway. -->
<!-- svelte-ignore a11y_click_events_have_key_events, a11y_no_static_element_interactions -->
<div class="md help" {onclick}>{@html html}</div>

<style>
  .help {
    padding-bottom: var(--w-s-6);
  }
  .help :global(h2:first-child) {
    margin-top: 0;
  }
</style>
