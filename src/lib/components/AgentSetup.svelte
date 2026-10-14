<script lang="ts">
  import AgentsEditor from "$lib/components/AgentsEditor.svelte";
  import {
    workspace,
    agentChecks,
    agentContext,
    copyText,
    createAgentsMd,
    inDailyWork,
    installCli,
    openInVscode,
    projColor,
    refreshCli,
    tilde,
    type AgentChecks,
    type AgentContext,
    type ProjectEntry,
  } from "$lib/stores/workspace.svelte";

  const GLOBAL = ".workly/agent/AGENTS.md";

  let checks = $state<AgentChecks | null>(null);
  let checking = $state(false);
  async function runChecks() {
    checking = true;
    await refreshCli();
    checks = await agentChecks().catch((e) => ((workspace.error = String(e)), null));
    checking = false;
  }
  runChecks();

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
  let copied = $state(false);
  async function copyBuiltin() {
    if (checks && (await copyText(checks.builtin_rules))) {
      copied = true;
      setTimeout(() => (copied = false), 1600);
    }
  }

  const cli = $derived(workspace.cli);
  const projects = $derived((workspace.index?.projects ?? []).filter(inDailyWork));
  const repos = $derived(projects.flatMap((p) => p.repos));
  const missing = $derived(workspace.index?.missing_repos ?? []);
  /** What passed, as one line; everything else gets its own row. */
  const ok = $derived(
    [
      cli?.ok && "wly",
      checks?.codex && `Codex${checks.codex.version ? ` ${checks.codex.version.replace(/^codex(-cli)?\s*/i, "")}` : ""}`,
      checks?.vscode && `VS Code ${checks.vscode.version ?? ""}`.trim(),
      repos.length && !missing.length && `${repos.length} of ${repos.length} repos`,
      checks?.rules === "builtin" && "built-in rules",
    ].filter(Boolean) as string[],
  );

  // Rules and skills: what `wly task show` hands to an agent, per project.
  let contexts = $state<{ global: AgentContext | null; projects: Record<string, AgentContext> }>({ global: null, projects: {} });
  $effect(() => {
    const keys = projects.map((p) => p.key);
    void workspace.index;
    Promise.all([agentContext(null), ...keys.map((k) => agentContext(k))]).then(
      ([global, ...rest]) => (contexts = { global, projects: Object.fromEntries(keys.map((k, i) => [k, rest[i]])) }),
      (e) => (workspace.error = String(e)),
    );
  });
  const ctxOf = (p: ProjectEntry) => contexts.projects[p.key];
</script>

<div class="content">
  <AgentsEditor />

  <div class="cols">
    <section class="panel" aria-label="Checks">
      <div class="head">
        <h2 class="w-h2">Checks</h2>
        <button type="button" class="w-btn w-btn--quiet right" disabled={checking} onclick={runChecks}>{checking ? "Checking…" : "Run again"}</button>
      </div>
      {#if ok.length}
        <div class="allok"><span class="dot"></span>{ok.length} {ok.length === 1 ? "check" : "checks"} OK<span class="w-sub">{ok.join(" · ")}</span></div>
      {/if}

      {#if cli !== undefined && !cli?.ok}
        <div class="check">
          <span class="ctext"
            ><span class="ctitle warn">{cli ? "wly CLI is broken" : "wly CLI is not installed"}</span><span class="w-sub"
              >{cli ? `~/.local/bin/wly → ${tilde(cli.target)}, which no longer exists` : "Agents report through wly. Links ~/.local/bin/wly to this app, no admin rights."}</span
            ></span
          >
          <button type="button" class="w-btn" onclick={install}>{cli ? "Reinstall CLI" : "Install CLI"}</button>
        </div>
        {#if cliError}<p class="w-sub err">{cliError}</p>{/if}
      {/if}
      {#if checks && !checks.codex}
        <div class="check">
          <span class="ctext"
            ><span class="ctitle warn">Codex not found</span><span class="w-sub"
              >Not in ~/.local/bin, Homebrew, npm, nvm or the VS Code extension. Start agent asks your login shell, so it may still work.</span
            ></span
          >
        </div>
      {/if}
      {#if checks && !checks.vscode}
        <div class="check">
          <span class="ctext"><span class="ctitle warn">VS Code 1.98 or newer not found</span><span class="w-sub">Copilot starts through VS Code's code chat.</span></span>
        </div>
      {/if}
      {#if missing.length}
        <div class="check">
          <span class="ctext"
            ><span class="ctitle warn">{missing.length} {missing.length === 1 ? "repo is" : "repos are"} not on this Mac</span><span class="w-sub w-mono"
              >{missing.map((m) => `${m.key}: ${m.repo}`).join(" · ")}</span
            ></span
          >
        </div>
      {/if}
      {#if checks && checks.rules !== "builtin"}
        <div class="check">
          <span class="ctext"
            ><span class="ctitle" class:warn={checks.rules === "missing"}>{checks.rules === "missing" ? "No global rules" : "Your own global rules"}</span><span class="w-sub"
              >{checks.rules === "missing"
                ? `${GLOBAL} is missing. The prompt still carries the short rules.`
                : `${GLOBAL} differs from the rules this version of Workly ships. Compare when agents miss a newer rule.`}</span
            ></span
          >
          <span class="acts">
            {#if checks.rules === "custom"}<button type="button" class="w-btn w-btn--quiet" onclick={() => openInVscode(GLOBAL)}>Open</button>{/if}
            <button type="button" class="w-btn" onclick={copyBuiltin}>{copied ? "Copied" : "Copy built-in rules"}</button>
          </span>
        </div>
      {/if}
      {#if !checks && checking}<p class="w-sub">Checking…</p>{/if}
    </section>

    <section class="panel" aria-label="Rules and skills">
      <div class="head"><h2 class="w-h2">Rules &amp; skills</h2><span class="w-sub">What wly task show hands to every agent</span></div>
      <div class="rrow">
        <span class="w-proj-mark" style:--c="var(--w-line)"></span>
        <span class="grow"><span>Global rules</span><span class="w-mono path">{GLOBAL}</span></span>
        {#if contexts.global?.global}<button type="button" class="w-btn w-btn--quiet" onclick={() => openInVscode(GLOBAL)}>Open</button>{/if}
      </div>
      {#each projects as p (p.key)}
        {@const ctx = ctxOf(p)}
        <div class="rrow">
          <span class="w-proj-mark" style:--c={projColor(p.color)}></span>
          <span class="grow">
            <span>{p.title}</span>
            <span class="w-mono path">{ctx?.project ? ctx.project.path.slice(p.path.length + 1) : "no agent/AGENTS.md"}{p.repos.length ? "" : " · no repo"}</span>
            {#if ctx?.skills.length}
              <span class="skills">
                {#each ctx.skills as s (s.path)}<button type="button" class="skill" title={s.description} onclick={() => openInVscode(s.path)}>skill · {s.name}</button>{/each}
              </span>
            {/if}
          </span>
          {#if ctx?.project}
            <button type="button" class="w-btn w-btn--quiet" onclick={() => openInVscode(ctx.project!.path)}>Open</button>
          {:else if ctx}
            <button type="button" class="w-btn w-btn--quiet" onclick={() => createAgentsMd(p.key)}>Create</button>
          {/if}
        </div>
      {/each}
    </section>
  </div>
</div>

<style>
  .content {
    max-width: 1080px;
    display: flex;
    flex-direction: column;
    gap: 18px;
  }
  .cols {
    display: grid;
    grid-template-columns: repeat(auto-fit, minmax(380px, 1fr));
    gap: 18px;
    align-items: start;
  }
  .panel {
    display: flex;
    flex-direction: column;
    gap: var(--w-s-2);
    min-width: 0;
    padding: var(--w-s-4) var(--w-s-5);
    background: var(--w-surface);
    border-radius: var(--w-r-lg);
    box-shadow: var(--w-shadow-panel);
  }
  .panel p {
    margin: 0;
  }
  .head {
    display: flex;
    flex-wrap: wrap;
    align-items: baseline;
    gap: var(--w-s-1) var(--w-s-3);
  }
  .right {
    margin-left: auto;
  }
  .allok {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: var(--w-s-1) var(--w-s-3);
    padding: 10px 12px;
    background: var(--w-sunk);
    border-radius: var(--w-r-md);
    font-size: var(--w-fs-small);
  }
  .dot {
    width: 8px;
    height: 8px;
    border-radius: 50%;
    background: var(--w-ok);
  }
  .check {
    display: flex;
    align-items: center;
    gap: var(--w-s-3);
    padding-top: var(--w-s-2);
  }
  .ctext,
  .grow {
    display: flex;
    flex: 1;
    flex-direction: column;
    gap: 2px;
    min-width: 0;
  }
  .ctitle {
    display: flex;
    align-items: center;
    gap: var(--w-s-2);
    font-weight: 500;
  }
  .ctitle::before {
    content: "";
    width: 8px;
    height: 8px;
    flex: none;
    border-radius: 50%;
    background: var(--w-hold);
  }
  .ctitle.warn::before {
    background: var(--w-warn);
  }
  .ctext .w-sub {
    padding-left: 16px;
    overflow-wrap: anywhere;
  }
  .acts {
    display: flex;
    gap: var(--w-s-2);
  }
  .err {
    color: var(--w-danger);
  }
  .rrow {
    display: flex;
    align-items: center;
    gap: var(--w-s-3);
    min-width: 0;
    padding: 10px 0;
    border-top: 1px solid var(--w-line);
  }
  .head + .rrow {
    border-top: 0;
  }
  .path {
    color: var(--w-muted);
    overflow-wrap: anywhere;
  }
  .skills {
    display: flex;
    flex-wrap: wrap;
    gap: 6px;
    padding-top: 3px;
  }
  .skill {
    padding: 3px 7px;
    border: 0;
    border-radius: var(--w-r-sm);
    background: var(--w-sunk);
    color: var(--w-muted);
    font-family: var(--w-font-mono);
    font-size: var(--w-fs-micro);
    cursor: pointer;
  }
  .skill:hover {
    color: var(--w-ink);
  }
</style>
