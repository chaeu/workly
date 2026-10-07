<script lang="ts">
  import { open as openDialog } from "@tauri-apps/plugin-dialog";
  import {
    workspace,
    saveSettings,
    openWorkspace,
    chooseWorkspace,
    createWorkspace,
    tilde,
    expandHome,
    agentsEnabled,
    refreshCli,
    installCli,
  } from "$lib/stores/workspace.svelte";
  import { getVersion } from "@tauri-apps/api/app";

  const themes = ["system", "light", "dark"] as const;
  const detailModes = [["popup", "Popup"], ["panel", "Side panel"]] as const;
  const settings = $derived(workspace.settings);
  // The open workspace can differ from `active` while WORKLY_WORKSPACE is set (dev).
  const openRoot = $derived(workspace.index?.root ?? null);

  async function chooseReposDir() {
    const start = settings?.repos_dir;
    const path = await openDialog({ directory: true, title: "Folder with your repos", defaultPath: start ? expandHome(start) : undefined });
    if (typeof path === "string") await saveSettings({ repos_dir: tilde(path) });
  }

  const cli = $derived(workspace.cli);
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

  let version = $state("");
  getVersion().then((v) => (version = v));

  const remove = (path: string) => saveSettings({ workspaces: settings!.workspaces.filter((w) => w.path !== path) });
</script>

<header class="w-page-head">
  <div>
    <h1 class="w-h1">Settings</h1>
    <p class="w-sub">Stored on this Mac in ~/Library/Application Support/Workly/settings.json</p>
  </div>
</header>

{#if !settings}
  <section class="panel" role="alert">
    <h2 class="w-h2">Settings could not be read</h2>
    <p class="w-sub">{workspace.error ?? "Unknown error."}</p>
    <p class="w-sub">Fix or delete the file, then restart Workly. Nothing is saved until it can be read, so it is never overwritten.</p>
  </section>
{:else}
  <section class="panel">
    <div class="panel-head">
      <h2 class="w-h2">Workspaces</h2>
      <div class="w-toolbar">
        <button class="w-btn" onclick={chooseWorkspace}>Add folder…</button>
        <button class="w-btn" onclick={createWorkspace}>Create new…</button>
      </div>
    </div>
    {#each settings.workspaces as w (w.path)}
      <div class="row">
        <span class="name">{w.name}</span>
        <span class="w-mono path" title={w.path}>{tilde(w.path)}</span>
        {#if w.path === openRoot || (w.path === settings.active && !openRoot)}
          <span class="w-sub current">Open</span>
        {:else}
          <button class="w-btn w-btn--quiet" onclick={() => openWorkspace(w.path)}>Switch</button>
          <button class="w-btn w-btn--quiet" title="Remove from this list. The folder stays." onclick={() => remove(w.path)}>Remove</button>
        {/if}
      </div>
    {:else}
      <p class="w-sub">No workspace yet.</p>
    {/each}
    {#if openRoot && !settings.workspaces.some((w) => w.path === openRoot)}
      <p class="w-sub">Open now: <span class="w-mono">{tilde(openRoot)}</span> (from WORKLY_WORKSPACE, not saved)</p>
    {/if}
  </section>

  <section class="panel">
    <div class="panel-head">
      <div>
        <h2 class="w-h2">Repos folder</h2>
        <p class="w-sub">Where the repo picker starts when you link a repo to a project.</p>
      </div>
      <div class="w-toolbar">
        {#if settings.repos_dir}
          <span class="w-mono">{settings.repos_dir}</span>
          <button class="w-btn w-btn--quiet" onclick={() => saveSettings({ repos_dir: null })}>Clear</button>
        {/if}
        <button class="w-btn" onclick={chooseReposDir}>Choose…</button>
      </div>
    </div>
  </section>

  <section class="panel">
    <div class="panel-head">
      <h2 class="w-h2">Theme</h2>
      <div class="w-toolbar">
        <div class="w-seg" role="group" aria-label="Theme">
          {#each themes as t (t)}
            <button type="button" aria-pressed={(settings.theme ?? "system") === t} onclick={() => saveSettings({ theme: t })}
              >{t[0].toUpperCase() + t.slice(1)}</button
            >
          {/each}
        </div>
      </div>
    </div>
  </section>

  <section class="panel">
    <div class="panel-head">
      <h2 class="w-h2">Detail cards</h2>
      <div class="w-toolbar">
        <div class="w-seg" role="group" aria-label="Detail cards">
          {#each detailModes as [mode, label] (mode)}
            <button type="button" aria-pressed={(settings.task_detail ?? "popup") === mode} onclick={() => saveSettings({ task_detail: mode })}>{label}</button>
          {/each}
        </div>
      </div>
    </div>
  </section>

  <section class="panel">
    <div class="panel-head">
      <div>
        <h2 class="w-h2">Agent features</h2>
        <p class="w-sub">Agents view, agent fields on cards and tasks, agent files in projects. Off hides them; files and wly keep working.</p>
      </div>
      <div class="w-toolbar">
        <div class="w-seg" role="group" aria-label="Agent features">
          <button type="button" aria-pressed={agentsEnabled()} onclick={() => saveSettings({ agents_enabled: true })}>On</button>
          <button type="button" aria-pressed={!agentsEnabled()} onclick={() => saveSettings({ agents_enabled: false })}>Off</button>
        </div>
      </div>
    </div>
    {#if agentsEnabled()}
      <div class="row">
        <span class="name">wly</span>
        <span class="w-mono path" class:broken={cli && !cli.ok} title={cli?.target}
          >{!cli
            ? "Not installed. Links ~/.local/bin/wly to this app, no admin rights needed."
            : cli.ok
              ? `~/.local/bin/wly → ${tilde(cli.target)}`
              : `Broken: ~/.local/bin/wly → ${tilde(cli.target)}, which no longer exists`}</span
        >
        <button class="w-btn" onclick={install}>{cli ? "Reinstall CLI" : "Install CLI"}</button>
      </div>
      {#if cliError}<p class="w-sub cli-error">{cliError}</p>{/if}
    {/if}
  </section>

  <p class="w-sub version">Workly <span class="w-mono">{version}</span> · About: Workly menu → About Workly · Shortcuts: press ?</p>
{/if}

<style>
  .panel {
    display: flex;
    flex-direction: column;
    gap: var(--w-s-2);
    padding: var(--w-s-4) var(--w-s-5);
    background: var(--w-surface);
    border-radius: var(--w-r-lg);
    box-shadow: var(--w-shadow-panel);
    max-width: 760px;
  }
  .panel-head {
    display: flex;
    align-items: center;
    gap: var(--w-s-3);
  }
  .panel p {
    margin: 0;
  }
  .row {
    display: flex;
    align-items: center;
    gap: var(--w-s-3);
    min-height: 32px;
    border-top: 1px solid var(--w-line);
    padding-top: var(--w-s-2);
  }
  .name {
    font-weight: 500;
  }
  .path {
    flex: 1;
    min-width: 0;
    color: var(--w-muted);
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .cli-error,
  .broken {
    color: var(--w-danger);
  }
  .version {
    margin: 0;
  }
  .current {
    color: var(--w-accent);
    padding: 0 14px;
  }
</style>
