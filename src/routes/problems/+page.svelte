<script lang="ts">
  import {
    workspace,
    agentsEnabled,
    installCli,
    refreshCli,
    openInVscode,
    reveal,
    problemCount,
    projColor,
    tilde,
  } from "$lib/stores/workspace.svelte";

  const errors = $derived(workspace.index?.errors ?? []);
  const repos = $derived(workspace.index?.missing_repos ?? []);
  const cli = $derived(workspace.cli);
  const cliProblem = $derived(agentsEnabled() && cli !== undefined && !cli?.ok);
  const project = (key: string) => workspace.index?.projects.find((p) => p.key === key);

  let cliError = $state<string | null>(null);
  async function install() {
    try {
      await installCli();
      await refreshCli();
      cliError = null;
    } catch (e) {
      cliError = String(e);
    }
  }
</script>

<header class="w-page-head">
  <div>
    <h1 class="w-h1">Problems</h1>
    <p class="w-sub">{problemCount() ? "Workly skips what it cannot read and keeps going. Fix the file and save; this list updates on its own." : "Nothing to fix."}</p>
  </div>
</header>

{#if errors.length}
  <section class="group">
    <h2 class="w-h2">Files Workly cannot read</h2>
    <div class="w-tray list">
      {#each errors as e, i (i)}
        <div class="row">
          <div class="text">
            <span class="w-mono path">{e.path}{e.line ? `:${e.line}` : ""}</span>
            <span class="msg">{e.message}</span>
          </div>
          <button class="w-btn" onclick={() => openInVscode(e.path, e.line)}>Open in VS Code</button>
          <button class="w-btn w-btn--quiet" onclick={() => reveal(e.path)}>Finder</button>
        </div>
      {/each}
    </div>
  </section>
{/if}

{#if repos.length}
  <section class="group">
    <h2 class="w-h2">Repos not found on this Mac</h2>
    <div class="w-tray list">
      {#each repos as r (r.key + r.repo)}
        {@const p = project(r.key)}
        <div class="row">
          <span class="w-proj-mark" style:--c={projColor(p?.color ?? null)}></span>
          <div class="text">
            <span class="w-mono path">{r.repo}</span>
            <span class="msg">Linked in {p?.title ?? r.key}. Clone it there, or change the project's repos.</span>
          </div>
          <a class="w-btn" href="/projects/{r.key}?tab=overview">Open project</a>
        </div>
      {/each}
    </div>
  </section>
{/if}

{#if cliProblem}
  <section class="group">
    <h2 class="w-h2">Command line</h2>
    <div class="w-tray list">
      <div class="row">
        <div class="text">
          <span class="w-mono path">~/.local/bin/wly</span>
          <span class="msg"
            >{cli
              ? `Points to ${tilde(cli.target)}, which no longer exists.`
              : "Not installed. Agents report progress through wly."} Agent features are on, so agents need it.</span
          >
          {#if cliError}<span class="msg err">{cliError}</span>{/if}
        </div>
        <button class="w-btn" onclick={install}>{cli ? "Reinstall CLI" : "Install CLI"}</button>
      </div>
    </div>
  </section>
{/if}

<style>
  .w-page-head p {
    margin: 4px 0 0;
  }
  .group {
    display: flex;
    flex-direction: column;
    gap: var(--w-s-2);
    max-width: 900px;
  }
  .row {
    display: flex;
    align-items: center;
    gap: var(--w-s-3);
    padding: 8px 8px 8px 12px;
    border-radius: var(--w-r-md);
    background: var(--w-surface);
    box-shadow: var(--w-shadow-card);
  }
  .text {
    flex: 1;
    min-width: 0;
    display: flex;
    flex-direction: column;
    gap: 2px;
    user-select: text;
  }
  .path {
    color: var(--w-ink);
    overflow-wrap: anywhere;
  }
  .msg {
    font-size: var(--w-fs-small);
    color: var(--w-muted);
    overflow-wrap: anywhere;
  }
  .err {
    color: var(--w-danger);
  }
  a.w-btn {
    text-decoration: none;
  }
</style>
