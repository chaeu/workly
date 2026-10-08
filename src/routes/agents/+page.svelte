<script lang="ts">
  import { goto } from "$app/navigation";
  import TaskDetail from "$lib/components/TaskDetail.svelte";
  import { agentsEnabled, tilde, workspace, type TaskEntry } from "$lib/stores/workspace.svelte";
  import { agentBadge, agentName, byBoardOrder, projectOf, today } from "$lib/tasks.svelte";

  // Agent features off: the view does not exist.
  $effect(() => {
    if (!agentsEnabled()) goto("/tasks", { replaceState: true });
  });

  // ponytail: activity list + test guide for the wly trial week; the v1.5 queue view replaces it if M7 happens.
  const tasks = $derived(workspace.index?.tasks ?? []);
  const groups = $derived([
    { title: "Working", list: tasks.filter((t) => t.agent?.active) },
    { title: "Back for review", list: tasks.filter((t) => t.status === "review" && (t.agent?.last_run || t.agent?.commit)) },
    { title: "Ready for an agent", list: tasks.filter((t) => t.agent?.ready && !t.agent.active && !["doing", "review", "done"].includes(t.status)) },
  ].map((g) => ({ ...g, list: g.list.sort(byBoardOrder) })));
  const active = $derived(groups.some((g) => g.list.length));

  const stateOf = (t: TaskEntry) =>
    agentBadge(t) ?? (t.agent?.commit ? `commit ${t.agent.commit.slice(0, 7)}` : t.status === "review" ? "review" : t.agent?.runner && t.agent.runner !== "auto" ? agentName(t.agent.runner) : "ready");

  // A real id for the examples: a todo task in a project with a repo, else any task.
  const example = $derived(
    (tasks.find((t) => t.status === "todo" && projectOf(t)?.repos.length) ?? tasks.find((t) => t.status !== "done") ?? tasks[0])?.id ?? "WR-5",
  );
  const root = $derived(workspace.index?.root ?? "");
  const cli = $derived(workspace.cli);

  let openId = $state<string | null>(null);
  const detailMode = $derived(workspace.settings?.task_detail ?? "popup");
  const focusIds = $derived(
    tasks
      .filter((t) => t.focus?.slice(0, 10) === today())
      .sort((a, b) => (a.focus_order ?? 9) - (b.focus_order ?? 9))
      .map((t) => t.id),
  );
</script>

{#if agentsEnabled()}
  <header class="w-page-head">
    <div>
      <h1 class="w-h1">Agents</h1>
      <p class="w-sub">Agents work in VS Code and report through <span class="w-mono">wly</span>. Their status shows up here and on the cards.</p>
    </div>
  </header>

  <section class="w-tray list" aria-label="Agent activity">
    {#if active}
      {#each groups as g (g.title)}
        {#if g.list.length}
          <div class="w-caps group">{g.title} · {g.list.length}</div>
          {#each g.list as t (t.id)}
            {@const p = projectOf(t)}
            <button type="button" class="row" onclick={() => (openId = t.id)}>
              <span class="w-mono id">{t.id}</span>
              <span class="text">
                <span class="title">{t.title}</span>
                <span class="w-sub where">{p?.title ?? "Inbox"}</span>
              </span>
              <span class="state">{stateOf(t)}</span>
            </button>
          {/each}
        {/if}
      {/each}
    {:else}
      <p class="w-sub empty">No agent activity yet. Hand a task over with <b>Copy prompt</b> in its detail card – see the guide below.</p>
    {/if}
  </section>

  <section class="panel guide" aria-label="Test guide">
    <h2 class="w-h2">Trying wly this week</h2>
    <p class="w-sub">
      The goal: find out whether handing tasks to Copilot or Codex through <span class="w-mono">wly</span> works in everyday use, and whether you miss a one-click
      “Start agent”. That decides M7.
    </p>

    <h3>1 · Get wly into VS Code's terminal</h3>
    <p>
      {#if cli?.ok}
        Installed: <span class="w-mono">~/.local/bin/wly → {tilde(cli.target)}</span>.
      {:else}
        Not installed yet: <a href="/settings">Settings → Install CLI</a> links <span class="w-mono">~/.local/bin/wly</span> to this app.
      {/if}
      Then, in a VS Code terminal:
    </p>
    <pre>wly --version</pre>
    <p>“command not found”: add the folder to your PATH once, then open a new terminal.</p>
    <pre>echo 'export PATH="$HOME/.local/bin:$PATH"' >> ~/.zshrc</pre>
    <p>wly uses the workspace that is active in Workly; no environment variables needed.</p>

    <h3>2 · Try it by hand (2 min)</h3>
    <p>Keep this window next to VS Code and watch the card change after each line.</p>
    <pre>wly task list --status todo
wly task show {example}
wly task start {example} --agent test
wly task note {example} "trying wly" --agent test
wly task review {example} --agent test
wly task done {example} --agent test     # refused, exit 3: only you set done</pre>
    <p>Afterwards set the task back to its old status in the app.</p>

    <h3>3 · Hand a task to Copilot</h3>
    <ol>
      <li>Open the task. Under <b>Agent</b>: Runner <b>Copilot</b>, then <b>Open repo</b> and <b>Copy prompt</b>.</li>
      <li>In VS Code open Copilot Chat in <b>Agent</b> mode, paste, send.</li>
      <li>Allow the <span class="w-mono">wly</span> commands when Copilot asks. To skip the prompts, auto-approve them in the VS Code settings (<span class="w-mono"
          >chat.tools.terminal.autoApprove</span
        >, entry <span class="w-mono">"wly": true</span>).</li>
    </ol>

    <h3>4 · Hand a task to Codex</h3>
    <ol>
      <li>Same as Copilot with Runner <b>Codex</b>; paste the prompt into the Codex panel in VS Code.</li>
      <li>
        Codex's sandbox only writes inside the open folder, but <span class="w-mono">wly</span> writes to the Workly workspace. Approve the command when Codex
        asks, or allow the workspace once in <span class="w-mono">~/.codex/config.toml</span>:
      </li>
    </ol>
    <pre>[sandbox_workspace_write]
writable_roots = ["{root}"]</pre>

    <h3>5 · What to watch</h3>
    <ul>
      <li>Did the agent run <span class="w-mono">wly task show</span> first and follow the rules?</li>
      <li>Did the card update live: badge, notes under Updates, Review column?</li>
      <li>Were the notes useful when you came back?</li>
      <li>Did it ever try <span class="w-mono">done</span>, or edit the task file directly?</li>
      <li>How often did you wish for a one-click “Start agent” instead of copy and paste?</li>
    </ul>
    <p>Note findings as you go, e.g. <span class="w-mono">wly task add "wly: …" --inbox</span>.</p>
  </section>

  {#if openId}
    <TaskDetail id={openId} mode={detailMode} {focusIds} onclose={() => (openId = null)} />
  {/if}
{/if}

<style>
  .w-page-head p {
    margin: 4px 0 0;
  }
  .list,
  .guide {
    box-sizing: border-box;
    max-width: 900px;
  }
  .group {
    padding: 6px 8px 0;
  }
  .empty {
    margin: 0;
    padding: 6px 8px;
  }
  .row {
    display: flex;
    align-items: center;
    gap: var(--w-s-3);
    padding: 8px 12px;
    border: 0;
    border-radius: var(--w-r-md);
    background: var(--w-surface);
    box-shadow: var(--w-shadow-card);
    color: inherit;
    font: inherit;
    text-align: left;
    cursor: pointer;
  }
  .row:hover {
    box-shadow: var(--w-shadow-card), 0 0 0 1px var(--w-line);
  }
  .id {
    width: 7ch;
    flex: none;
    color: var(--w-muted);
  }
  .text {
    flex: 1;
    min-width: 0;
    display: flex;
    flex-direction: column;
    gap: 2px;
  }
  .title {
    font-weight: 500;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .state {
    font-size: var(--w-fs-caption);
    font-weight: 500;
    color: var(--w-accent);
    white-space: nowrap;
  }
  .panel {
    display: flex;
    flex-direction: column;
    gap: var(--w-s-2);
    padding: var(--w-s-4) var(--w-s-5);
    background: var(--w-surface);
    border-radius: var(--w-r-lg);
    box-shadow: var(--w-shadow-panel);
    font-size: var(--w-fs-small);
    line-height: 1.5;
    user-select: text;
  }
  .panel p,
  .panel ol,
  .panel ul {
    margin: 0;
  }
  .panel ol,
  .panel ul {
    padding-left: var(--w-s-5);
    display: grid;
    gap: var(--w-s-1);
  }
  h3 {
    margin: var(--w-s-3) 0 0;
    font-family: var(--w-font-label);
    font-size: var(--w-fs-small);
    font-weight: 600;
  }
  pre {
    margin: 0;
    padding: var(--w-s-2) var(--w-s-3);
    background: var(--w-sunk);
    border-radius: var(--w-r-md);
    font-family: var(--w-font-mono);
    font-size: var(--w-fs-micro);
    line-height: 1.6;
    overflow-x: auto;
  }
  a {
    color: var(--w-accent);
  }
</style>
