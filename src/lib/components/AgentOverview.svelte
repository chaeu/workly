<script lang="ts">
  import {
    workspace,
    changeDiff,
    clearAgent,
    commitTask,
    expandHome,
    inDailyWork,
    projColor,
    readMarkdown,
    runLog,
    taskChanges,
    updateTaskField,
    type FileChange,
    type TaskEntry,
  } from "$lib/stores/workspace.svelte";
  import DiffView from "$lib/components/DiffView.svelte";
  import { agentBadge, agentInbox, blockedReason, agentStale, isoDate, projectOf, shortDate, today, updatesOf } from "$lib/tasks.svelte";

  let { onopen }: { onopen: (id: string) => void } = $props();

  const inbox = $derived(agentInbox());
  const blockedNote = $derived(workspace.index?.agent_blocked ?? {});
  const repoOf = (t: TaskEntry) => projectOf(t)?.repos[0] ?? null;
  const repoName = (repo: string | null) => repo?.replace(/\/+$/, "").split("/").pop() ?? "";

  // Last note of the current run per running task; re-read with the index (the watcher reports .workly/runs/).
  let lastNote = $state<Record<string, string>>({});
  $effect(() => {
    void workspace.index;
    for (const t of inbox.running) {
      runLog(t.id).then(
        (r) => (lastNote[t.id] = r.filter((e) => e.run === t.agent?.since).at(-1)?.text ?? ""),
        () => {},
      );
    }
  });

  // One agent per repo: the first repos of live projects that no running agent holds.
  const free = $derived.by(() => {
    const busy = new Set(inbox.running.map(repoOf).filter(Boolean).map((r) => expandHome(r!)));
    const missing = new Set(workspace.index?.missing_repos.map((m) => m.repo));
    const repos = (workspace.index?.projects ?? []).filter(inDailyWork).map((p) => p.repos[0]).filter((r): r is string => !!r && !missing.has(r));
    return [...new Set(repos)].filter((r) => !busy.has(expandHome(r)));
  });

  /** "14:10" today, "Oct 12" before. */
  function when(iso: string | null | undefined) {
    if (!iso) return "";
    const d = new Date(iso);
    if (isNaN(d.getTime())) return "";
    return isoDate(d) === today() ? d.toLocaleTimeString("en-GB", { hour: "2-digit", minute: "2-digit" }) : shortDate(isoDate(d));
  }

  // Expanded review rows: the summary from ## Updates, the files as they are now and their diffs, like a pull request.
  let open = $state<Record<string, boolean>>({});
  let detail = $state<Record<string, { summary: string; files: FileChange[]; diffs: Record<string, string> }>>({});
  async function loadDetail(t: TaskEntry) {
    const [body, files] = await Promise.all([readMarkdown(t.path).catch(() => ""), t.agent?.changes?.length ? taskChanges(t.id).catch(() => []) : []]);
    const summary = updatesOf(body).at(-1)?.text ?? "";
    const diffs = await Promise.all(files.map((f) => changeDiff(t.id, f.path).then((d) => d || "No difference to the last commit", String)));
    detail[t.id] = { summary, files, diffs: Object.fromEntries(files.map((f, i) => [f.path, diffs[i]])) };
  }
  /** Collapsed file diffs, by `<id> <path>`. */
  let shut = $state<Record<string, boolean>>({});
  function toggle(t: TaskEntry) {
    open[t.id] = !open[t.id];
    if (open[t.id]) loadDetail(t);
  }
  $effect(() => {
    void workspace.index;
    for (const t of inbox.review) if (open[t.id]) loadDetail(t);
  });

  const KIND = { new: "A", modified: "M", deleted: "D", clean: "·" } as const;

  let busy = $state<string | null>(null);
  /** Files edited after the review: the task card shows their diffs and asks there. */
  async function commit(t: TaskEntry, done: boolean) {
    if (busy) return;
    busy = t.id;
    try {
      if ((await taskChanges(t.id)).some((f) => f.drifted)) {
        onopen(t.id);
        workspace.error = `${t.id}: files changed after the review. Check the diffs in the task, then commit there.`;
        return;
      }
      await commitTask(t.id, false);
      if (done) await updateTaskField(t.id, "status", "done");
    } catch (e) {
      workspace.error = String(e);
    } finally {
      busy = null;
    }
  }
  const setDone = (t: TaskEntry) => updateTaskField(t.id, "status", "done");
</script>

<div class="content">
  <section class="section" aria-label="Running">
    <div class="shead"><h2 class="w-h2">Running</h2><span class="w-sub">one agent per repo</span></div>
    {#if inbox.running.length}
      <div class="tiles">
        {#each inbox.running as t (t.id)}
          {@const stale = agentStale(t)}
          <article class="tile" class:active={!stale}>
            <div class="tline">
              <span class="w-card-agent" class:w-card-agent--stale={stale}>{agentBadge(t)}</span>
              <span class="w-mono repo">{repoName(repoOf(t))}</span>
            </div>
            <button type="button" class="ttitle" onclick={() => onopen(t.id)}><span class="w-mono">{t.id}</span> {t.title}</button>
            {#if lastNote[t.id]}<div class="last" title={lastNote[t.id]}>{lastNote[t.id]}</div>{/if}
            {#if stale}
              <div class="tacts">
                <button type="button" class="w-btn sm" onclick={() => clearAgent(t.id)}>Clear agent</button>
              </div>
            {/if}
          </article>
        {/each}
      </div>
    {:else}
      <p class="w-sub quiet">No agent running.</p>
    {/if}
    {#if free.length}
      <p class="w-sub quiet">Free: <span class="w-mono">{free.map(repoName).join(" · ")}</span></p>
    {/if}
  </section>

  <section class="section" aria-label="Needs you">
    <div class="shead"><h2 class="w-h2">Needs you</h2><span class="w-sub">blocked first, then the oldest review</span></div>
    {#if inbox.blocked.length || inbox.review.length}
      <div class="list">
        {#each inbox.blocked as t (t.id)}
          {@const p = projectOf(t)}
          <article class="nrow">
            <div class="nline">
              <span class="kind blocked">Blocked</span>
              <span class="w-mono id">{t.id}</span>
              <button type="button" class="ntext" onclick={() => onopen(t.id)}>
                <span class="ntitle">{t.title}</span>
                <span class="nsub"
                  ><span class="w-proj-mark" style:--c={projColor(p?.color ?? null)}></span>{p?.title ?? "Inbox"} · {when(workspace.index?.agent_activity[t.id])}</span
                >
              </button>
              <span class="nacts"><button type="button" class="w-btn sm" onclick={() => clearAgent(t.id)}>Clear agent</button></span>
            </div>
            <p class="ndetail note">{blockedReason(blockedNote[t.id])}</p>
          </article>
        {/each}
        {#each inbox.review as t (t.id)}
          {@const p = projectOf(t)}
          {@const n = t.agent?.changes?.length ?? 0}
          {@const d = detail[t.id]}
          <article class="nrow" class:open={open[t.id]}>
            <div class="nline">
              <span class="kind review">Review</span>
              <span class="w-mono id">{t.id}</span>
              <button type="button" class="ntext" aria-expanded={!!open[t.id]} onclick={() => toggle(t)}>
                <span class="ntitle">{t.title}</span>
                <span class="nsub"
                  ><span class="w-proj-mark" style:--c={projColor(p?.color ?? null)}></span>{p?.title ?? "Inbox"}{t.agent?.last_run ? ` · ${when(t.agent.last_run)}` : ""}{n
                    ? ` · ${n} ${n === 1 ? "file" : "files"}`
                    : ""}{t.agent?.commit ? ` · commit ${t.agent.commit}` : ""}</span
                >
              </button>
              <span class="nacts">
                {#if n && !t.agent?.commit}
                  <button type="button" class="w-btn sm" disabled={!!busy} onclick={() => commit(t, false)}>Commit</button>
                  <button type="button" class="w-btn sm" disabled={!!busy} onclick={() => commit(t, true)}>Commit &amp; Done</button>
                {:else}
                  <button type="button" class="w-btn sm" onclick={() => setDone(t)}>Done</button>
                {/if}
              </span>
              <button type="button" class="chev" aria-label={open[t.id] ? "Collapse" : "Expand"} aria-expanded={!!open[t.id]} onclick={() => toggle(t)}
                ><svg width="14" height="14" viewBox="0 0 14 14" aria-hidden="true"
                  ><path d={open[t.id] ? "M3 9l4-4 4 4" : "M3 5l4 4 4-4"} fill="none" stroke="currentColor" stroke-width="1.6" stroke-linecap="round" stroke-linejoin="round" /></svg
                ></button
              >
            </div>
            {#if open[t.id]}
              <div class="ndetail">
                {#if d?.summary}<p class="summary">{d.summary}</p>{/if}
                {#each d?.files ?? [] as f (f.path)}
                  {@const key = `${t.id} ${f.path}`}
                  <div class="file">
                    <button type="button" class="fhead" aria-expanded={!shut[key]} onclick={() => (shut[key] = !shut[key])}>
                      <b class="k {f.kind}">{KIND[f.kind]}</b><span class="w-mono path">{f.path}</span>{#if f.drifted}<span class="drift">edited</span>{/if}
                      <svg class="fchev" width="12" height="12" viewBox="0 0 14 14" aria-hidden="true"
                        ><path d={shut[key] ? "M3 5l4 4 4-4" : "M3 9l4-4 4 4"} fill="none" stroke="currentColor" stroke-width="1.6" stroke-linecap="round" stroke-linejoin="round" /></svg
                      >
                    </button>
                    {#if !shut[key] && d!.diffs[f.path]}<DiffView text={d!.diffs[f.path]} max="none" />{/if}
                  </div>
                {/each}
                <div><button type="button" class="w-btn w-btn--quiet sm" onclick={() => onopen(t.id)}>Open task</button></div>
              </div>
            {/if}
          </article>
        {/each}
      </div>
    {:else}
      <p class="w-sub quiet">Nothing needs you.</p>
    {/if}
  </section>
</div>

<style>
  .content {
    max-width: 1080px;
    display: flex;
    flex-direction: column;
    gap: 22px;
  }
  .section {
    display: flex;
    flex-direction: column;
    gap: 10px;
  }
  .shead {
    display: flex;
    align-items: baseline;
    gap: var(--w-s-3);
    padding: 0 2px;
  }
  .quiet {
    margin: 0;
    padding: 0 2px;
  }
  .sm {
    padding: 4px 10px;
    font-size: var(--w-fs-caption);
  }
  .tiles {
    display: grid;
    grid-template-columns: repeat(auto-fill, minmax(280px, 1fr));
    gap: var(--w-s-3);
  }
  .tile {
    display: flex;
    flex-direction: column;
    gap: 6px;
    min-width: 0;
    padding: 12px 14px;
    background: var(--w-surface);
    border-radius: var(--w-r-md);
    box-shadow: var(--w-shadow-card);
  }
  .tile.active {
    box-shadow: var(--w-ring-active);
  }
  .tline {
    display: flex;
    align-items: center;
    gap: var(--w-s-2);
    min-width: 0;
  }
  .repo {
    margin-left: auto;
    color: var(--w-muted);
    white-space: nowrap;
  }
  .ttitle {
    border: 0;
    background: none;
    padding: 0;
    color: inherit;
    font: inherit;
    font-weight: 500;
    line-height: 1.3;
    text-align: left;
    cursor: pointer;
  }
  .ttitle .w-mono {
    color: var(--w-muted);
    margin-right: 4px;
  }
  .last {
    font-size: var(--w-fs-small);
    color: var(--w-muted);
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .tacts {
    display: flex;
    gap: 6px;
    padding-top: 2px;
  }
  .list {
    display: flex;
    flex-direction: column;
    overflow: hidden;
    background: var(--w-surface);
    border-radius: var(--w-r-lg);
    box-shadow: var(--w-shadow-panel);
  }
  .nrow {
    display: flex;
    flex-direction: column;
    border-top: 1px solid var(--w-line);
  }
  .nrow:first-child {
    border-top: 0;
  }
  .nrow.open {
    background: var(--w-sunk);
  }
  .nline {
    display: grid;
    grid-template-columns: 78px 56px minmax(0, 1fr) auto 32px;
    gap: var(--w-s-3);
    align-items: center;
    padding: 10px 14px 10px 16px;
  }
  .kind {
    justify-self: start;
    padding: 1px 7px;
    border-radius: 4px;
    font-family: var(--w-font-label);
    font-size: var(--w-fs-micro);
    font-weight: 600;
    letter-spacing: 0.04em;
    text-transform: uppercase;
  }
  .kind.review {
    color: var(--w-accent);
    background: var(--w-accent-soft);
  }
  .kind.blocked {
    color: var(--w-danger);
    background: var(--w-danger-soft);
  }
  .id {
    color: var(--w-muted);
  }
  .ntext {
    display: flex;
    flex-direction: column;
    gap: 2px;
    min-width: 0;
    border: 0;
    background: none;
    padding: 0;
    color: inherit;
    font: inherit;
    text-align: left;
    cursor: pointer;
  }
  .ntitle {
    font-weight: 500;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .nsub {
    display: flex;
    align-items: center;
    gap: 7px;
    min-width: 0;
    font-size: var(--w-fs-caption);
    color: var(--w-muted);
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }
  .nacts {
    display: flex;
    gap: 6px;
    align-items: center;
  }
  .chev {
    display: grid;
    place-items: center;
    width: 28px;
    height: 28px;
    padding: 0;
    border: 0;
    border-radius: var(--w-r-sm);
    background: none;
    color: var(--w-muted);
    cursor: pointer;
  }
  .chev:hover {
    background: var(--w-tray);
    color: var(--w-ink);
  }
  .ndetail {
    display: flex;
    flex-direction: column;
    gap: 10px;
    margin: 0;
    padding: 0 16px 14px calc(16px + 78px + 56px + 2 * var(--w-s-3));
  }
  .note,
  .summary {
    max-width: 720px;
    font-size: var(--w-fs-small);
    line-height: 1.5;
    white-space: pre-line;
    user-select: text;
  }
  .summary {
    margin: 0;
  }
  .file {
    display: flex;
    flex-direction: column;
    gap: var(--w-s-1);
    padding: var(--w-s-1);
    background: var(--w-surface);
    border-radius: var(--w-r-md);
    box-shadow: inset 0 0 0 1px var(--w-line);
    min-width: 0;
  }
  .fhead {
    display: flex;
    align-items: center;
    gap: var(--w-s-2);
    padding: 4px 6px;
    border: 0;
    border-radius: var(--w-r-sm);
    background: none;
    color: inherit;
    font: inherit;
    text-align: left;
    cursor: pointer;
  }
  .fhead:hover {
    background: var(--w-sunk);
  }
  .fchev {
    flex: none;
    margin-left: auto;
    color: var(--w-muted);
  }
  .drift + .fchev {
    margin-left: 0;
  }
  .k {
    width: 12px;
    font-family: var(--w-font-mono);
    font-size: var(--w-fs-micro);
  }
  .k.new {
    color: var(--w-ok);
  }
  .k.modified {
    color: var(--w-info);
  }
  .k.deleted {
    color: var(--w-danger);
  }
  .path {
    min-width: 0;
    overflow-wrap: anywhere;
  }
  .drift {
    margin-left: auto;
    font-size: var(--w-fs-caption);
    color: var(--w-warn);
  }
  @media (max-width: 820px) {
    .nline {
      grid-template-columns: 70px minmax(0, 1fr) 32px;
    }
    .nline .id,
    .nacts {
      display: none;
    }
    .ndetail {
      padding-left: 16px;
    }
  }
</style>
