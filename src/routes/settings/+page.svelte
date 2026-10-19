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
    backupNow,
    backupInfo,
    enableBackup,
    addWaitingStatus,
    openInVscode,
    BACKUP_DEFAULTS,
    type BackupInfo,
    type BackupSettings,
  } from "$lib/stores/workspace.svelte";
  import { getVersion } from "@tauri-apps/api/app";

  const themes = ["system", "light", "dark"] as const;
  const detailModes = [["popup", "Popup"], ["panel", "Side panel"]] as const;
  const settings = $derived(workspace.settings);
  // The open workspace can differ from `active` while WORKLY_WORKSPACE is set (dev).
  const openRoot = $derived(workspace.index?.root ?? null);

  let newApp = $state("");
  const apps = $derived(settings?.open_with ?? []);
  function addApp(e: SubmitEvent) {
    e.preventDefault();
    const name = newApp.trim();
    if (name && !apps.includes(name)) saveSettings({ open_with: [...apps, name] });
    newApp = "";
  }
  function moveApp(i: number, by: number) {
    const next = [...apps];
    [next[i], next[i + by]] = [next[i + by], next[i]];
    saveSettings({ open_with: next });
  }

  async function chooseReposDir() {
    const start = settings?.repos_dir;
    const path = await openDialog({ directory: true, title: "Folder with your repos", defaultPath: start ? expandHome(start) : undefined });
    if (typeof path === "string") await saveSettings({ repos_dir: tilde(path) });
  }

  let version = $state("");
  getVersion().then((v) => (version = v));

  // Backup of the open workspace; interval and push live in its entry of the workspace list.
  const intervals = [["off", "Off"], ["15m", "15 min"], ["hourly", "Hourly"], ["daily", "Daily"]] as const;
  const backup = $derived(workspace.backup);
  const entry = $derived(settings?.workspaces.find((w) => w.path === openRoot));
  const cfg = $derived(entry?.backup ?? BACKUP_DEFAULTS);
  let info = $state<BackupInfo | null>(null);
  let backupError = $state<string | null>(null);
  // Re-read after every run too: the upstream can change in Git outside Workly.
  $effect(() => {
    void backup;
    if (openRoot) backupInfo().then((i) => (info = i), (e) => (backupError = String(e)));
  });
  const saveBackup = (patch: Partial<BackupSettings>) =>
    saveSettings({ workspaces: settings!.workspaces.map((w) => (w.path === entry?.path ? { ...w, backup: { ...cfg, ...patch } } : w)) });
  async function enable() {
    try {
      await enableBackup();
      info = await backupInfo();
      backupError = null;
    } catch (e) {
      backupError = String(e);
    }
  }

  // Existing workspaces keep their own config.yml; this adds the column on request.
  const WAITING_LINE = "- { id: waiting, label: Waiting }";
  const noWaiting = $derived(!!workspace.index && !workspace.index.config.task_statuses.some((s) => s.id === "waiting"));
  let waitingError = $state(false);
  async function addWaiting() {
    try {
      await addWaitingStatus();
    } catch {
      waitingError = true;
    }
  }

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

  {#if openRoot}
    <section class="panel" id="backup">
      <div class="panel-head">
        <div class="grow">
          <h2 class="w-h2">Backup</h2>
          <p class="w-sub">Commits this workspace to its Git repo and pushes to the branch's upstream. Workly never sets up or changes remotes.</p>
        </div>
        {#if info?.repo}
          <div class="w-toolbar">
            <button class="w-btn" disabled={backup?.state === "running"} onclick={backupNow}>{backup?.state === "running" ? "Backing up…" : "Back up now"}</button>
          </div>
        {/if}
      </div>
      {#if info && !info.repo}
        <div class="row">
          {#if info.cloud}
            <span class="w-sub grow">This folder is synced by OneDrive or iCloud, which breaks Git repos. Move the workspace to a local folder to back it up.</span>
          {:else}
            <span class="w-sub grow">Not a Git repo yet. Creates one in the workspace folder, with a .gitignore.</span>
            <button class="w-btn" onclick={enable}>Enable backup</button>
          {/if}
        </div>
      {:else if info}
        {#if entry}
          <div class="row">
            <span class="name grow">Interval</span>
            <div class="w-seg" role="group" aria-label="Backup interval">
              {#each intervals as [id, label] (id)}
                <button type="button" aria-pressed={cfg.interval === id} onclick={() => saveBackup({ interval: id })}>{label}</button>
              {/each}
            </div>
          </div>
          <div class="row">
            <span class="name grow">Push</span>
            <div class="w-seg" role="group" aria-label="Push">
              <button type="button" aria-pressed={cfg.push} onclick={() => saveBackup({ push: true })}>On</button>
              <button type="button" aria-pressed={!cfg.push} onclick={() => saveBackup({ push: false })}>Off</button>
            </div>
          </div>
        {:else}
          <p class="w-sub">Not in the workspace list (WORKLY_WORKSPACE): hourly, push on.</p>
        {/if}
        <div class="row">
          <span class="name">Pushes to</span>
          <span class="w-mono path" title={info.url ?? undefined}
            >{info.upstream ? `${info.upstream}${info.url ? ` · ${info.url}` : ""}` : "No upstream. Commits stay on this Mac."}</span
          >
        </div>
        {#if backup?.state === "failed" && backup.error}
          <pre class="w-mono backup-error">{backup.error}</pre>
        {/if}
      {/if}
      {#if backupError}<p class="w-sub cli-error">{backupError}</p>{/if}
    </section>
  {/if}

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
      <div class="grow">
        <h2 class="w-h2">Open files with</h2>
        <p class="w-sub">Buttons on files and task cards, in this order. Name as in Applications, e.g. Zed.</p>
      </div>
      <form class="w-toolbar" onsubmit={addApp}>
        <input class="app-name" bind:value={newApp} placeholder="App name" aria-label="App name" />
        <button class="w-btn" disabled={!newApp.trim()}>Add</button>
      </form>
    </div>
    {#each apps as app, i (app)}
      <div class="row">
        <span class="name grow">{app}</span>
        <button class="w-btn w-btn--quiet" aria-label="Move {app} up" disabled={i === 0} onclick={() => moveApp(i, -1)}>↑</button>
        <button class="w-btn w-btn--quiet" aria-label="Move {app} down" disabled={i === apps.length - 1} onclick={() => moveApp(i, 1)}>↓</button>
        <button class="w-btn w-btn--quiet" onclick={() => saveSettings({ open_with: apps.filter((a) => a !== app) })}>Remove</button>
      </div>
    {:else}
      <p class="w-sub">No apps. Files open in Finder only.</p>
    {/each}
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

  {#if noWaiting}
    <section class="panel">
      <div class="panel-head">
        <div class="grow">
          <h2 class="w-h2">Waiting column</h2>
          <p class="w-sub">This workspace has no Waiting column. Adds one line to task_statuses in .workly/config.yml, before Review.</p>
        </div>
        <button type="button" class="w-btn" onclick={addWaiting}>Add Waiting column</button>
      </div>
      {#if waitingError}
        <div class="row">
          <span class="grow">The list in config.yml has a shape Workly does not edit. Add this line under task_statuses by hand:</span>
          <button type="button" class="w-btn w-btn--quiet" onclick={() => openInVscode(".workly/config.yml")}>Open config.yml</button>
        </div>
        <code class="w-mono waiting-line">{WAITING_LINE}</code>
      {/if}
    </section>
  {/if}

  <section class="panel">
    <div class="panel-head">
      <div>
        <h2 class="w-h2">Hours per FTE per year</h2>
        <p class="w-sub">Turns the savings of a use case into FTE. Per Mac.</p>
      </div>
      <div class="w-toolbar">
        {#key settings.fte_hours_per_year}
          <input
            class="fte-hours"
            type="number"
            min="1"
            step="1"
            value={settings.fte_hours_per_year ?? 1720}
            aria-label="Hours per FTE per year"
            onchange={(e) => {
              const n = e.currentTarget.valueAsNumber;
              if (n > 0) saveSettings({ fte_hours_per_year: n });
              else e.currentTarget.value = String(settings.fte_hours_per_year ?? 1720);
            }}
          />
        {/key}
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
      <p class="w-sub">The agent list, the <span class="w-mono">wly</span> CLI and the checks live in <a href="/agents?tab=setup">Agents → Setup</a>.</p>
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
  .cli-error {
    color: var(--w-danger);
  }
  .version {
    margin: 0;
  }
  .grow {
    flex: 1;
    min-width: 0;
  }
  .backup-error {
    margin: 0;
    padding: var(--w-s-2) var(--w-s-3);
    background: var(--w-tray);
    border-radius: var(--w-r-md);
    color: var(--w-warn);
    white-space: pre-wrap;
    overflow-wrap: anywhere;
    max-height: 240px;
    overflow: auto;
    user-select: text;
  }
  .current {
    color: var(--w-accent);
    padding: 0 14px;
  }
  .waiting-line {
    padding: var(--w-s-2) var(--w-s-3);
    background: var(--w-tray);
    border-radius: var(--w-r-md);
    user-select: text;
  }
  .fte-hours,
  .app-name {
    width: 90px;
    box-sizing: border-box;
    border: 1px solid var(--w-line);
    border-radius: var(--w-r-sm);
    padding: 4px 8px;
    background: var(--w-surface);
    font-size: var(--w-fs-small);
    text-align: right;
    font-variant-numeric: tabular-nums;
  }
  .fte-hours:focus,
  .app-name:focus {
    border-color: var(--w-accent);
    outline: none;
  }
  .app-name {
    width: 160px;
    text-align: left;
  }
</style>
