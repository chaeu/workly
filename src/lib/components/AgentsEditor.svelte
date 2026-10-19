<script lang="ts">
  import { workspace, openInVscode, setAgents, type AgentPreset } from "$lib/stores/workspace.svelte";
  import { EFFORTS, PERMISSIONS, RUNNERS, agentName } from "$lib/tasks.svelte";

  // Agents (B10): the Agent menu in config.yml. Every complete change writes the whole list.
  const agents = $derived(workspace.index?.config.agents ?? []);
  let draft = $state<AgentPreset | null>(null);
  let agentsError = $state<string | null>(null);
  async function saveAgents(list: AgentPreset[]) {
    try {
      await setAgents(list);
      agentsError = null;
      draft = null;
    } catch (e) {
      agentsError = String(e);
    }
  }
  /** r = row in the list, -1 = the new row (saved once it has a name). */
  const agentCell = (r: number, field: keyof AgentPreset) => (e: Event) => {
    const value = (e.currentTarget as HTMLInputElement | HTMLSelectElement).value.trim();
    const row: AgentPreset = { ...(r < 0 ? draft! : agents[r]), [field]: value || (field === "name" ? "" : null) };
    if (row.runner === "copilot") Object.assign(row, { model: null, effort: null, permissions: null });
    if (r < 0) {
      draft = row;
      if (!row.name) return;
    }
    saveAgents(r < 0 ? [...agents, row] : agents.map((a, j) => (j === r ? row : a)));
  };
  function moveAgent(r: number, by: number) {
    const list = [...agents];
    [list[r], list[r + by]] = [list[r + by], list[r]];
    saveAgents(list);
  }
</script>

<section class="panel" aria-label="Agents">
  <div class="head">
    <h2 class="w-h2">Agents</h2>
    <span class="w-sub">The Agent menu on every task. The first one is the default.</span>
    <span class="w-mono file">.workly/config.yml</span>
  </div>
  {#if agents.length || draft}
    <div class="scroll">
      <table class="agents">
        <thead>
          <tr><th>Name</th><th class="runner">Runner</th><th>Model</th><th class="effort">Effort</th><th class="perm">Permissions</th><th></th></tr>
        </thead>
        <tbody>
          {#each [...agents, ...(draft ? [draft] : [])] as a, i (i)}
            {@const r = i < agents.length ? i : -1}
            {@const copilot = a.runner === "copilot"}
            <!-- {#key} resets the inputs to the file's values after every reload. -->
            {#key workspace.reloads}
              <tr>
                <td>
                  <input
                    value={a.name}
                    placeholder="e.g. Codex Sol"
                    aria-label="Name"
                    onchange={agentCell(r, "name")}
                    {@attach (el) => {
                      if (r < 0 && !a.name) el.focus();
                    }}
                  />
                </td>
                <td class="runner">
                  <select value={a.runner} aria-label="Runner" onchange={agentCell(r, "runner")}>
                    {#each RUNNERS as x (x)}<option value={x}>{agentName(x)}</option>{/each}
                  </select>
                </td>
                <td>
                  <input
                    class="w-mono"
                    autocapitalize="off" autocorrect="off" spellcheck="false"
                    value={a.model ?? ""}
                    disabled={copilot}
                    placeholder={copilot ? "Picked in VS Code" : "Codex default"}
                    aria-label="Model"
                    onchange={agentCell(r, "model")}
                  />
                </td>
                <td class="effort">
                  <select value={a.effort ?? ""} disabled={copilot} aria-label="Effort" onchange={agentCell(r, "effort")}>
                    {#each EFFORTS as x (x)}<option value={x}>{x || "default"}</option>{/each}
                    {#if a.effort && !EFFORTS.includes(a.effort)}<option value={a.effort}>{a.effort}</option>{/if}
                  </select>
                </td>
                <td class="perm">
                  <select
                    value={a.permissions ?? "ask"}
                    class:full={a.permissions === "full"}
                    disabled={copilot}
                    aria-label="Permissions"
                    onchange={agentCell(r, "permissions")}
                  >
                    {#each PERMISSIONS as x (x)}<option value={x}>{x.charAt(0).toUpperCase() + x.slice(1)}</option>{/each}
                  </select>
                </td>
                <td class="acts">
                  {#if i === 0}<span class="w-caps default">Default</span>{/if}
                  <button type="button" class="rm" aria-label="Move up" disabled={r <= 0} onclick={() => moveAgent(r, -1)}>↑</button>
                  <button type="button" class="rm" aria-label="Move down" disabled={r < 0 || r === agents.length - 1} onclick={() => moveAgent(r, 1)}>↓</button>
                  <button
                    type="button"
                    class="rm"
                    aria-label="Remove agent"
                    onclick={() => (r < 0 ? (draft = null) : saveAgents(agents.filter((_, j) => j !== r)))}>✕</button
                  >
                </td>
              </tr>
            {/key}
          {/each}
        </tbody>
      </table>
    </div>
  {:else}
    <p class="w-sub">No agents yet. Tasks start the runner from agent_defaults in config.yml.</p>
  {/if}
  <div>
    <button type="button" class="w-btn w-btn--quiet add" disabled={!!draft} onclick={() => (draft = { name: "", runner: "codex", model: null, effort: null })}
      >+ Add agent</button
    >
  </div>
  {#if agentsError}
    <div class="error">
      <span class="grow">{agentsError}</span>
      <button type="button" class="w-btn w-btn--quiet" onclick={() => openInVscode(".workly/config.yml")}>Open config.yml</button>
    </div>
  {/if}
  <div class="levels">
    <div class="level"><b>Ask</b>Codex asks before risky steps.</div>
    <div class="level"><b>Auto</b>Never asks. Stays in its sandbox, no network.</div>
    <div class="level full"><b>Full</b>No sandbox: the agent can do anything you can on this Mac.</div>
  </div>
  <p class="w-sub">All three may write to the repo, the project folder and .workly/. Copilot: model and approvals are set in VS Code.</p>
</section>

<style>
  .panel {
    display: flex;
    flex-direction: column;
    gap: var(--w-s-2);
    padding: var(--w-s-4) var(--w-s-5);
    background: var(--w-surface);
    border-radius: var(--w-r-lg);
    box-shadow: var(--w-shadow-panel);
    min-width: 0;
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
  .file {
    margin-left: auto;
    color: var(--w-muted);
  }
  .scroll {
    overflow-x: auto;
  }
  .add {
    padding-left: 0;
    color: var(--w-accent);
  }
  .error {
    display: flex;
    align-items: center;
    gap: var(--w-s-3);
    color: var(--w-danger);
  }
  .grow {
    flex: 1;
    min-width: 0;
  }
  .levels {
    display: grid;
    grid-template-columns: repeat(auto-fit, minmax(180px, 1fr));
    gap: var(--w-s-2);
    margin-top: var(--w-s-2);
  }
  .level {
    display: flex;
    flex-direction: column;
    gap: 2px;
    padding: var(--w-s-2) var(--w-s-3);
    background: var(--w-sunk);
    border-radius: var(--w-r-md);
    font-size: var(--w-fs-caption);
    color: var(--w-muted);
  }
  .level b {
    font-size: var(--w-fs-small);
    color: var(--w-ink);
  }
  .level.full b {
    color: var(--w-warn);
  }
  .agents {
    width: 100%;
    border-collapse: collapse;
    font-size: var(--w-fs-small);
  }
  .agents th {
    text-align: left;
    font-weight: 500;
    font-size: var(--w-fs-caption);
    color: var(--w-muted);
    padding: 0 4px 2px;
    border-bottom: 1px solid var(--w-line);
  }
  .agents td {
    padding: 2px 0;
    border-bottom: 1px solid var(--w-line);
  }
  .agents .runner {
    width: 110px;
  }
  .agents .effort {
    width: 90px;
  }
  .agents .perm {
    width: 100px;
  }
  .agents select.full {
    color: var(--w-warn);
  }
  .agents .acts {
    padding-left: var(--w-s-3);
    width: 1%;
    white-space: nowrap;
    text-align: right;
  }
  .agents input,
  .agents select {
    width: 100%;
    min-width: 80px;
    box-sizing: border-box;
    border: 1px solid transparent;
    border-radius: var(--w-r-sm);
    padding: 4px 6px;
    background: none;
    font-size: var(--w-fs-small);
  }
  .agents input:hover:not(:disabled),
  .agents select:hover:not(:disabled) {
    border-color: var(--w-line);
  }
  .agents input:focus,
  .agents select:focus {
    border-color: var(--w-accent);
    background: var(--w-surface);
    outline: none;
  }
  .agents :disabled {
    color: var(--w-muted);
  }
  .default {
    color: var(--w-accent);
    margin-right: var(--w-s-2);
  }
  .rm {
    border: 0;
    background: none;
    color: var(--w-muted);
    cursor: pointer;
    border-radius: var(--w-r-sm);
    width: 24px;
    height: 24px;
  }
  .rm:hover:not(:disabled) {
    background: var(--w-tray);
    color: var(--w-ink);
  }
  .rm:disabled {
    opacity: 0.3;
    cursor: default;
  }
</style>
