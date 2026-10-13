<script lang="ts">
  import { ask } from "@tauri-apps/plugin-dialog";
  import { grow } from "$lib/grow";
  import { tip } from "$lib/tip";
  import { renderMarkdown } from "$lib/markdown";
  import {
    workspace,
    agentsEnabled,
    projColor,
    readMarkdown,
    updateTaskField,
    addTaskUpdate,
    setDescription,
    moveTask,
    setFocus,
    openUrl,
    openInObsidian,
    openInVscode,
    openRepo,
    copyText,
    inDailyWork,
    runLog,
    clearAgent,
    setTaskAgent,
    startAgentTask,
    launchAgent,
    ensureBaseline,
    taskChanges,
    changeDiff,
    commitTask,
    openProjectInVscode,
    toggleAcceptance,
    addAcceptance,
    addSections,
    type AcceptanceItem,
    type RunEntry,
    type FileChange,
  } from "$lib/stores/workspace.svelte";
  import {
    agentChoice,
    agentLabel,
    agentList,
    agentPick,
    agentName,
    agentPrompt,
    agentStale,
    bodyView,
    confirmDeleteTask,
    descriptionOf,
    hasSections,
    openQuickAdd,
    projectOf,
    runnerOf,
    shortDate,
    updatesOf,
    waitingFocus,
  } from "$lib/tasks.svelte";

  let {
    id,
    mode,
    focusIds,
    onclose,
    onopen,
  }: { id: string; mode: "popup" | "panel"; focusIds: string[]; onclose: () => void; onopen?: (id: string) => void } = $props();

  const t = $derived(workspace.index?.tasks.find((x) => x.id === id));
  const project = $derived(t && projectOf(t));
  const projects = $derived(workspace.index?.projects.filter((p) => inDailyWork(p) || p.key === project?.key) ?? []);
  const statuses = $derived(workspace.index?.config.task_statuses ?? []);
  const focusAt = $derived(focusIds.indexOf(id));

  // Gone (deleted or broken in another editor): nothing left to show.
  $effect(() => {
    if (workspace.index && !t) onclose();
  });

  // Body from the file; re-read on every index change so external edits show.
  let body = $state("");
  $effect(() => {
    const path = t?.path;
    void workspace.index;
    if (path) readMarkdown(path).then((b) => (body = b), (e) => (workspace.error = String(e)));
  });
  const view = $derived(bodyView(body));
  const updates = $derived(updatesOf(body));

  // Agent activity: the run log, newest first; re-read with the index (the watcher reports .workly/runs/).
  let runs = $state<RunEntry[]>([]);
  let runsOpen = $state(false);
  let runOpen = $state(-1);
  $effect(() => {
    void workspace.index;
    if (t && agentsEnabled()) runLog(id).then((r) => (runs = r.reverse()), () => (runs = []));
  });
  const shownRuns = $derived(runsOpen ? runs : runs.slice(0, 1));

  // Changes (B8): agent.changes as they are now and a diff per file on demand; re-read with the index.
  const KIND = { new: "A", modified: "M", deleted: "D", clean: "·" } as const;
  const paths = $derived(t?.agent?.changes ?? []);
  let files = $state<FileChange[]>([]);
  let filesError = $state("");
  let diffs = $state<Record<string, string>>({});
  let committing = $state(false);
  $effect(() => {
    void id;
    diffs = {};
  });
  $effect(() => {
    void workspace.index;
    if (!agentsEnabled() || !paths.length) return;
    taskChanges(id).then(
      (f) => ((files = f), (filesError = ""), Object.keys(diffs).forEach(loadDiff)),
      (e) => ((files = []), (filesError = String(e))),
    );
  });
  /** Without a review record on this Mac the paths from the file, without status. */
  const shownFiles = $derived(files.length ? files : paths.map((path) => ({ path, kind: null, drifted: false })));
  async function loadDiff(path: string) {
    diffs[path] = await changeDiff(id, path).then((d) => d || "No difference to the last commit", String);
  }
  const toggleDiff = (path: string) => (path in diffs ? delete diffs[path] : loadDiff(path));
  const lineClass = (l: string) =>
    /^(\+\+\+|---|diff |index |new file|deleted file)/.test(l) ? "meta" : l.startsWith("@@") ? "hunk" : l[0] === "+" ? "add" : l[0] === "-" ? "del" : "";
  /** Edited after the review: show those diffs and ask first. Commit & Done then sets done. */
  async function commit(done: boolean) {
    if (!t || committing) return;
    const drifted = files.filter((f) => f.drifted);
    if (drifted.length) {
      await Promise.all(drifted.map((f) => loadDiff(f.path)));
      const names = drifted.map((f) => f.path).join(", ");
      const msg = `${names} changed after the review. The ${drifted.length === 1 ? "diff is" : "diffs are"} open in the task. Commit as it is now?`;
      if (!(await ask(msg, { title: `Commit ${id}`, kind: "warning", okLabel: "Commit" }))) return;
    }
    committing = true;
    try {
      await commitTask(id, drifted.length > 0);
      if (done) await updateTaskField(id, "status", "done");
    } catch (e) {
      workspace.error = String(e);
    }
    committing = false;
  }
  const stale = $derived(!!t && agentStale(t));
  const html = $derived(renderMarkdown(view));

  /** Write one field; a refused write puts the file's value back into the input. */
  async function save(field: string, value: unknown, input?: HTMLInputElement, shown = "") {
    const ok = await updateTaskField(id, field, value);
    if (!ok && input) input.value = shown;
  }

  // Waiting: the field shows while waiting (or when a value is left); moving into waiting focuses it once.
  let waitingInput = $state<HTMLInputElement>();
  $effect(() => {
    if (t?.status === "waiting" && waitingFocus.id === id && waitingInput) {
      waitingInput.focus();
      waitingFocus.id = null;
    }
  });
  function setStatus(status: string) {
    if (status === "waiting") waitingFocus.id = id;
    save("status", status);
  }

  // Acceptance: real checkboxes. The box shows the file's state until the index changes.
  const checked = $derived(t?.acceptance.filter((a) => a.done).length ?? 0);
  function toggle(e: Event & { currentTarget: HTMLInputElement }, n: number, a: AcceptanceItem) {
    e.currentTarget.checked = a.done;
    toggleAcceptance(id, n, a);
  }
  let criterion = $state("");
  async function addCriterion() {
    if (!criterion.trim()) return;
    await addAcceptance(id, criterion);
    if (!workspace.error) criterion = "";
  }

  // Parent, one level: a parent has no parent; a task with sub-tasks gets none.
  const tasks = $derived(workspace.index?.tasks ?? []);
  const children = $derived(tasks.filter((x) => x.parent === id));
  const childrenDone = $derived(children.filter((x) => x.status === "done").length);
  const parentTask = $derived(t?.parent ? tasks.find((x) => x.id === t.parent) : undefined);
  const parentCandidates = $derived(tasks.filter((x) => !x.parent && x.id !== id && x.status !== "done"));
  const statusLabel = (s: string) => statuses.find((x) => x.id === s)?.label ?? s;

  const tagsText = $derived(t?.tags.join(", ") ?? "");
  const parseTags = (s: string) =>
    s
      .split(/[,\s]+/)
      .map((x) => x.replace(/^#/, ""))
      .filter(Boolean);

  let note = $state("");

  // Agent hand-off: Start agent launches Copilot (code chat) or Codex (Terminal); Copy prompt is the manual way.
  const repo = $derived(project?.repos[0] ?? null);
  const repoMissing = $derived(!!repo && !!workspace.index?.missing_repos.some((m) => m.repo === repo));
  const runner = $derived(t ? runnerOf(t) : "agent");
  // The core refuses too (one agent per repo); this only explains the disabled button.
  const busyIn = $derived(
    workspace.index?.tasks.find((x) => x.id !== id && x.agent?.active && repo && projectOf(x)?.repos[0] === repo),
  );
  const startBlock = $derived(
    !t
      ? "No task"
      : !repo
        ? "The project has no repo"
        : repoMissing
          ? `Not on this Mac: ${repo}`
          : t.status === "done"
            ? "The task is done"
            : busyIn
              ? `${busyIn.id} already has an agent working in this repo`
              : t.agent?.active
                ? `${agentName(t.agent.active)} is already working on this task`
                : runner !== "copilot" && runner !== "codex"
                  ? "Pick an agent that runs Copilot or Codex"
                  : null,
  );
  // Agent menu (B10): one entry writes runner, model and effort; "default" clears them.
  const pick = $derived(t ? agentPick(t) : -1);
  const pickValue = $derived(pick === null ? "custom" : pick < 0 ? "default" : String(pick));
  const choice = $derived(t ? agentChoice(t) : null);
  const access = (p: string | null | undefined) => (p === "full" ? "full access" : p === "auto" ? "no prompts" : null);
  const choiceText = $derived(choice ? [agentName(choice.runner), choice.model, choice.effort, access(choice.permissions)].filter(Boolean).join(" · ") : "");
  const choiceDetail = $derived(
    !choice
      ? ""
      : choice.runner === "copilot"
        ? "Model picked in VS Code"
        : choice.runner === "codex"
          ? [choice.model ?? "default model", choice.effort ?? "default effort", access(choice.permissions)].filter(Boolean).join(" · ")
          : choiceText,
  );
  function pickAgent(value: string) {
    if (value === "custom") return;
    setTaskAgent(id, value === "default" ? null : agentList()[+value]);
  }
  let copied = $state(false);
  async function copyPrompt() {
    if (!t) return;
    // In doing the prompt has no `wly task start`, so the baseline for review is taken here.
    if (t.status === "doing") await ensureBaseline(id);
    if (!(await copyText(agentPrompt(t, body)))) return;
    copied = true;
    setTimeout(() => (copied = false), 1600);
  }
  let starting = $state(false);
  async function startAgent() {
    if (!t || startBlock || starting) return;
    starting = true;
    const prompt = agentPrompt(t, body, true);
    const key = project?.key;
    try {
      await startAgentTask(id, runner);
    } catch (e) {
      workspace.error = String(e);
      starting = false;
      return;
    }
    try {
      await launchAgent(id, runner, prompt);
    } catch (e) {
      // Fallback: the prompt to paste and the workspace in VS Code; the task stays started.
      const ok = await copyText(prompt);
      if (key) await openProjectInVscode(key);
      workspace.error = `${e}. ${ok ? "Prompt copied – paste into the chat." : "Use Copy prompt."}`;
    }
    starting = false;
  }
  async function saveNote() {
    if (!note.trim()) return;
    await addTaskUpdate(id, note);
    if (!workspace.error) note = "";
  }

  async function remove() {
    if (t && (await confirmDeleteTask(t))) onclose();
  }

  /** Move the file; the select then shows whatever the index says, also after a refused move. */
  async function changeProject(e: Event & { currentTarget: HTMLSelectElement }) {
    const select = e.currentTarget;
    await moveTask(id, select.value || null);
    select.value = project?.key ?? "";
  }

  const focusFull = $derived(focusAt < 0 && focusIds.length >= 3);
  const toggleFocus = () => !focusFull && setFocus(focusAt >= 0 ? focusIds.filter((x) => x !== id) : [...focusIds, id]);

  // Links in the description open outside; they never navigate the app window.
  // Any other click (not ending a text selection) edits the description.
  function onDescClick(e: MouseEvent) {
    const a = (e.target as HTMLElement).closest("a");
    if (!a) {
      if (!getSelection()?.toString()) editDesc();
      return;
    }
    e.preventDefault();
    const href = a.getAttribute("href") ?? "";
    if (/^(https?|mailto):/i.test(href)) openUrl(href);
  }

  // Description edit: raw Markdown in a textarea. `expected` is the file's
  // description when editing started; core refuses the save if it changed since.
  let draft = $state<string | null>(null);
  let expected = "";
  let conflict = $state(false);
  let saving = false;
  function editDesc() {
    expected = descriptionOf(body).text;
    draft = expected.replace(/\r\n/g, "\n");
    conflict = false;
  }
  function cancelDesc() {
    draft = null;
    conflict = false;
  }
  async function saveDesc() {
    if (draft === null || conflict || saving) return;
    if (draft.trimEnd() === expected.replace(/\r\n/g, "\n")) return cancelDesc();
    saving = true;
    const ok = await setDescription(id, draft, expected);
    saving = false;
    if (ok) draft = null;
    else if (ok === false) conflict = true;
  }
  function onDescKey(e: KeyboardEvent) {
    if (e.key === "Escape") {
      // Cancel the edit only; the card stays open.
      e.preventDefault();
      e.stopPropagation();
      cancelDesc();
    } else if (e.key === "Enter" && e.metaKey) {
      e.preventDefault();
      saveDesc();
    }
  }
  /** Caret at the end, where most edits add text. */
  function focus(el: HTMLTextAreaElement) {
    el.focus();
    el.setSelectionRange(el.value.length, el.value.length);
  }

  const stamp = (iso: string | null) =>
    iso ? new Date(iso).toLocaleString("en-GB", { day: "numeric", month: "short", hour: "2-digit", minute: "2-digit" }) : "–";
</script>

{#if t}
  <!-- Popup and side panel: a click on the scrim only closes, it never opens what lies beneath. -->
  <div class="w-scrim" onclick={onclose} aria-hidden="true"></div>
  <div
    class={mode === "popup" ? "w-modal" : "panel"}
    role="dialog"
    aria-modal={mode === "popup"}
    aria-labelledby="t-title"
  >
    <div class="t-head">
      <div class="t-meta">
        <span class="w-mono">{t.id}</span>
        <span class="w-proj-mark" style:--c={project ? projColor(project.color) : "var(--w-line)"}></span>
        {#key t.path}
          <select class="project" value={project?.key ?? ""} aria-label="Project" onchange={changeProject}>
            <option value="">Inbox</option>
            {#each projects as p (p.key)}<option value={p.key}>{p.title}</option>{/each}
          </select>
        {/key}
        <!-- aria-disabled, not disabled: WebKit shows no tooltip on a disabled button. -->
        <button
          type="button"
          class="t-star"
          class:on={focusAt >= 0}
          aria-pressed={focusAt >= 0}
          aria-disabled={focusFull}
          aria-label={focusAt >= 0 ? `Focus #${focusAt + 1}, remove from focus` : "Add to today's focus"}
          use:tip={focusAt >= 0 ? `Focus #${focusAt + 1}` : focusFull ? "The focus strip already has three tasks" : "Add to today's focus"}
          onclick={toggleFocus}
          ><svg width="16" height="16" viewBox="0 0 24 24" fill={focusAt >= 0 ? "currentColor" : "none"} stroke="currentColor" stroke-width="1.8" stroke-linejoin="round" aria-hidden="true"
            ><path d="m12 3 2.8 5.7 6.2.9-4.5 4.4 1 6.2L12 17.3 6.5 20.2l1-6.2L3 9.6l6.2-.9z" /></svg
          ></button
        >
        <button type="button" class="t-x" onclick={onclose} aria-label="Close">✕</button>
      </div>
      {#key t.title}
        <input
          id="t-title"
          class="t-title"
          value={t.title}
          aria-label="Title"
          onchange={(e) => save("title", e.currentTarget.value.trim(), e.currentTarget, t.title)}
        />
      {/key}
      <div class="t-status" role="group" aria-label="Status">
        {#each statuses as s (s.id)}
          <button type="button" aria-pressed={t.status === s.id} onclick={() => setStatus(s.id)}>{s.label}</button>
        {/each}
      </div>
    </div>

    <div class="t-body">
      <div class="t-desc">
        {#if draft !== null}
          <textarea
            class="t-desc-edit"
            bind:value={draft}
            aria-label="Description (Markdown)"
            placeholder="Description (Markdown)"
            use:grow
            use:focus
            onblur={saveDesc}
            onkeydown={onDescKey}
          ></textarea>
          {#if conflict}
            <div class="t-conflict" role="alert">
              Changed outside – reload?
              <button type="button" class="w-btn w-btn--quiet" onclick={cancelDesc}>Reload</button>
            </div>
          {/if}
        {:else if view.trim()}
          <!-- Sanitised by DOMPurify in renderMarkdown. -->
          <!-- svelte-ignore a11y_no_noninteractive_tabindex, a11y_no_noninteractive_element_interactions -->
          <div
            class="md t-desc-view"
            role="group"
            aria-label="Description, click or press Enter to edit"
            tabindex="0"
            title="Click to edit"
            onclick={onDescClick}
            onkeydown={(e) => e.key === "Enter" && e.target === e.currentTarget && editDesc()}
          >
            {@html html}
          </div>
        {:else}
          <button type="button" class="t-desc-add" onclick={editDesc}>Add description</button>
        {/if}

        {#if t.acceptance.length || hasSections(body)}
          <div class="t-group">
            <span class="w-caps">Acceptance{t.acceptance.length ? ` · ${checked}/${t.acceptance.length}` : ""}</span>
            <ul class="t-acc">
              {#each t.acceptance as a, n (n)}
                <li class:done={a.done}>
                  <input type="checkbox" id="acc-{n}" checked={a.done} onchange={(e) => toggle(e, n, a)} />
                  <label for="acc-{n}">{a.text}</label>
                </li>
              {/each}
            </ul>
            <input
              class="t-acc-add"
              bind:value={criterion}
              placeholder="+ Add criterion"
              aria-label="Add acceptance criterion"
              onkeydown={(e) => e.key === "Enter" && addCriterion()}
            />
          </div>
        {/if}
        {#if !hasSections(body)}
          <button
            type="button"
            class="t-desc-add"
            use:tip={"Puts ## Description above the text and an empty ## Acceptance above ## Updates"}
            onclick={() => addSections(id)}>Add sections</button
          >
        {/if}
      </div>

      <div class="t-side">
        <dl class="t-kv">
          <dt>Priority</dt>
          <dd>
            <div class="w-seg" role="group" aria-label="Priority">
              {#each [null, 1, 2, 3] as p (p)}
                <button type="button" aria-pressed={t.priority === p} onclick={() => save("priority", p)}>{p ? `P${p}` : "–"}</button>
              {/each}
            </div>
          </dd>
          <dt>Due</dt>
          <dd>
            {#key t.due}
              <input
                type="date"
                class="field"
                value={t.due ?? ""}
                aria-label="Due date"
                onchange={(e) => save("due", e.currentTarget.value || null, e.currentTarget, t.due ?? "")}
              />
            {/key}
          </dd>
          <dt>Tags</dt>
          <dd>
            {#key tagsText}
              <input
                class="field w-mono"
                value={tagsText}
                placeholder="frontend, ux"
                aria-label="Tags, comma separated"
                onchange={(e) => save("tags", parseTags(e.currentTarget.value), e.currentTarget, tagsText)}
              />
            {/key}
          </dd>
          {#if t.status === "waiting" || t.waiting_for}
            <dt>Waiting for</dt>
            <dd>
              {#key t.waiting_for}
                <input
                  bind:this={waitingInput}
                  class="field waiting"
                  value={t.waiting_for ?? ""}
                  placeholder="Who or what?"
                  aria-label="Waiting for"
                  onchange={(e) => save("waiting_for", e.currentTarget.value.trim() || null, e.currentTarget, t.waiting_for ?? "")}
                />
              {/key}
            </dd>
          {/if}
          {#if !children.length}
            <dt>Parent</dt>
            <dd>
              {#key t.parent}
                <input
                  class="field w-mono"
                  list="t-parents"
                  value={t.parent ?? ""}
                  placeholder="–"
                  aria-label="Parent task id"
                  onchange={(e) => save("parent", e.currentTarget.value.trim() || null, e.currentTarget, t.parent ?? "")}
                />
              {/key}
              {#if t.parent}
                <button
                  type="button"
                  class="t-link"
                  disabled={!parentTask}
                  use:tip={parentTask ? parentTask.title : `${t.parent} is in the trash`}
                  onclick={() => parentTask && onopen?.(parentTask.id)}>{parentTask ? "↑ Open" : "(deleted)"}</button
                >
              {/if}
              <datalist id="t-parents">
                {#each parentCandidates as x (x.id)}<option value={x.id}>{x.title}</option>{/each}
              </datalist>
            </dd>
          {/if}
          {#if t.status === "done" && t.done_at}
            <dt>Done</dt>
            <dd class="w-mono">{shortDate(t.done_at)}</dd>
          {/if}
        </dl>

        {#if !t.parent}
          <div class="t-group">
            <span class="w-caps">Sub-tasks{children.length ? ` · ${childrenDone}/${children.length} done` : ""}</span>
            {#each children as c (c.id)}
              <button type="button" class="t-child" class:done={c.status === "done"} onclick={() => onopen?.(c.id)}>
                <span class="w-mono">{c.id}</span>
                <span class="t-child-title">{c.title}</span>
                <span class="t-child-status">{statusLabel(c.status)}</span>
              </button>
            {/each}
            <button type="button" class="t-more" onclick={() => openQuickAdd(project?.key ?? null, id)}>+ Add sub-task</button>
          </div>
        {/if}

        {#if agentsEnabled()}
          <div class="t-group">
            <span class="w-caps">Agent</span>
            <dl class="t-kv">
              <dt>Agent</dt>
              <dd class="t-agent-pick">
                <select class="field" value={pickValue} aria-label="Agent" onchange={(e) => pickAgent(e.currentTarget.value)}>
                  <option value="default">Default{agentList()[0] ? ` · ${agentList()[0].name}` : ""}</option>
                  {#each agentList() as a, i (a.name)}<option value={String(i)}>{a.name}</option>{/each}
                  {#if pickValue === "custom"}<option value="custom">{choiceText}</option>{/if}
                </select>
                <span class="t-agent-sub w-sub">{choiceDetail}</span>
              </dd>
              <dt>Active</dt>
              <dd>{t.agent?.active ? agentName(t.agent.active) : "–"}</dd>
              <dt>Since</dt>
              <dd class="w-mono">{stamp(t.agent?.since ?? null)}</dd>
              <dt>Commit</dt>
              <dd class="w-mono">{t.agent?.commit ?? "–"}</dd>
            </dl>
            <div class="t-agent-acts">
              <button
                type="button"
                class="w-btn w-btn--primary"
                disabled={!!startBlock || starting}
                use:tip={startBlock ?? (runner === "codex" ? "Codex in a new Terminal window, in the repo" : "Copilot agent chat in the project's VS Code workspace")}
                onclick={startAgent}>{starting ? "Starting…" : `Start ${agentLabel(t)}`}</button
              >
              <button type="button" class="w-btn" onclick={copyPrompt}>{copied ? "Copied" : "Copy prompt"}</button>
              {#if repo}
                <button type="button" class="w-btn w-btn--quiet" disabled={repoMissing} use:tip={repoMissing ? `Not on this Mac: ${repo}` : repo} onclick={() => openRepo(repo)}
                  >Open repo</button
                >
              {/if}
            </div>
          </div>
        {/if}

        {#if agentsEnabled() && paths.length}
          <div class="t-group">
            <span class="w-caps">Changes · {paths.length}</span>
            {#if filesError}<div class="t-files-error">{filesError}</div>{/if}
            {#each shownFiles as f (f.path)}
              <button type="button" class="t-file" aria-expanded={f.path in diffs} onclick={() => toggleDiff(f.path)}>
                <span class="w-mono t-kind t-kind--{f.kind}" use:tip={f.kind ?? "unknown"}>{f.kind ? KIND[f.kind] : "?"}</span>
                <span class="w-mono t-file-path" use:tip={f.path}>{f.path}</span>
                {#if f.drifted}<span class="t-drift" use:tip={"Edited after the review: Commit shows the diff and asks first"}>edited</span>{/if}
              </button>
              {#if f.path in diffs}
                <pre class="t-diff">{#each diffs[f.path].split("\n") as l, i (i)}<span class={lineClass(l)}>{l}</span>{/each}</pre>
              {/if}
            {/each}
            {#if t.status === "review" && !t.agent?.commit}
              <div class="t-agent-acts">
                <button
                  type="button"
                  class="w-btn w-btn--primary"
                  disabled={committing || !files.length}
                  use:tip={`Commits exactly these files in ${t.agent?.repo ?? "the repo"} as "${id}: …". Never pushes.`}
                  onclick={() => commit(false)}>{committing ? "Committing…" : "Commit"}</button
                >
                <button type="button" class="w-btn" disabled={committing || !files.length} onclick={() => commit(true)}>Commit & Done</button>
              </div>
            {/if}
          </div>
        {/if}

        {#if agentsEnabled() && (runs.length || stale)}
          <div class="t-group">
            <span class="w-caps">Agent activity · {runs.length}</span>
            {#if stale && t.agent?.active}
              <div class="t-stale" role="status">
                <span>Agent inactive? {agentName(t.agent.active)} has not reported since {stamp(workspace.index?.agent_activity?.[t.id] ?? t.agent.since)}.</span>
                <button type="button" class="w-btn w-btn--quiet" use:tip={"Clears agent.active and agent.since; the status stays"} onclick={() => clearAgent(id)}
                  >Clear agent</button
                >
              </div>
            {/if}
            {#each shownRuns as e, i (i)}
              <button type="button" class="t-run" class:open={runOpen === i} aria-expanded={runOpen === i} onclick={() => (runOpen = runOpen === i ? -1 : i)}>
                <span class="w-mono">{stamp(e.ts)}</span>
                <span class="t-run-text">{e.agent}: {e.text}</span>
              </button>
            {/each}
            {#if runs.length > 1}
              <button type="button" class="t-more" onclick={() => ((runsOpen = !runsOpen), (runOpen = -1))}>{runsOpen ? "Show less" : `${runs.length - 1} more`}</button>
            {/if}
          </div>
        {/if}

        <div class="t-group">
          <span class="w-caps">Updates · {updates.length}</span>
          {#each updates as u, i (i)}
            <div class="t-update"><span class="w-mono">{u.meta}</span>{u.text}</div>
          {/each}
          <textarea
            class="t-compose"
            bind:value={note}
            aria-label="Add an update"
            placeholder="Update, decision or closing note"
            onkeydown={(e) => e.key === "Enter" && e.metaKey && saveNote()}
          ></textarea>
        </div>
      </div>
    </div>

    <div class="t-foot">
      <button type="button" class="w-btn w-btn--quiet danger" onclick={remove}>Delete…</button>
      <button type="button" class="w-btn w-btn--quiet" onclick={() => openInObsidian(t.path)}>Obsidian</button>
      <button type="button" class="w-btn w-btn--quiet" onclick={() => openInVscode(t.path)}>VS Code</button>
      <button type="button" class="w-btn w-btn--primary" disabled={!note.trim()} onclick={saveNote}>Save update</button>
    </div>
  </div>
{/if}

<style>
  .panel {
    position: fixed;
    top: 0;
    right: 0;
    bottom: 0;
    z-index: 40;
    width: min(var(--w-panel-w), 100%);
    display: flex;
    flex-direction: column;
    background: var(--w-surface);
    box-shadow: var(--w-shadow-pop);
    color: var(--w-ink);
    animation: slide var(--w-dur) var(--w-ease);
  }
  .w-modal {
    width: min(var(--w-detail-w), calc(100% - 32px));
    min-height: min(var(--w-detail-min-h), calc(100% - 48px));
  }
  @keyframes slide {
    from {
      transform: translateX(16px);
      opacity: 0;
    }
  }
  /* Room for the overlay traffic lights / drag region. */
  .panel .t-head {
    padding-top: 36px;
  }
  .t-head {
    padding: 16px 22px 14px;
    display: grid;
    gap: 10px;
    border-bottom: 1px solid var(--w-line);
  }
  .t-meta {
    display: flex;
    align-items: center;
    gap: var(--w-s-2);
    color: var(--w-muted);
  }
  .project {
    border: 0;
    border-radius: var(--w-r-sm);
    background: none;
    color: var(--w-muted);
    font-size: var(--w-fs-small);
    padding: 2px 4px;
    margin-left: -4px;
    cursor: pointer;
  }
  .project:hover {
    background: var(--w-tray);
    color: var(--w-ink);
  }
  .t-star {
    margin-left: auto;
    display: grid;
    place-items: center;
    width: 30px;
    height: 30px;
    border: 0;
    border-radius: var(--w-r-sm);
    background: none;
    color: var(--w-muted);
    cursor: pointer;
  }
  .t-star:hover {
    background: var(--w-tray);
    color: var(--w-ink);
  }
  .t-star.on {
    color: var(--w-accent);
  }
  .t-star[aria-disabled="true"] {
    opacity: 0.4;
    cursor: default;
  }
  .t-star[aria-disabled="true"]:hover {
    background: none;
    color: var(--w-muted);
  }
  .t-x {
    width: 30px;
    height: 30px;
    border: 0;
    border-radius: var(--w-r-sm);
    background: none;
    color: var(--w-muted);
    cursor: pointer;
  }
  .t-x:hover {
    background: var(--w-tray);
    color: var(--w-ink);
  }
  .t-title {
    margin: 0 -6px;
    padding: 2px 6px;
    border: 0;
    border-radius: var(--w-r-sm);
    background: none;
    font-family: var(--w-font-label);
    font-size: var(--w-fs-headline);
    font-weight: 600;
    line-height: 1.2;
  }
  .t-title:hover {
    background: var(--w-sunk);
  }
  .t-title:focus {
    outline: none;
    background: var(--w-sunk);
    box-shadow: 0 0 0 2px var(--w-accent-soft);
  }
  .t-status {
    display: flex;
    flex-wrap: wrap;
    gap: 6px;
  }
  .t-status button {
    border: 0;
    border-radius: var(--w-r-pill);
    padding: 3px 11px;
    font-size: var(--w-fs-small);
    cursor: pointer;
    background: var(--w-tray);
    color: var(--w-muted);
  }
  .t-status button[aria-pressed="true"] {
    background: var(--w-ink);
    color: var(--w-bg);
    font-weight: 600;
  }
  .t-body {
    flex: 1;
    overflow-y: auto;
    padding: 18px 22px 20px;
    display: grid;
    grid-template-columns: minmax(0, 1.6fr) minmax(0, 1fr);
    gap: 18px 28px;
  }
  .panel .t-body {
    grid-template-columns: minmax(0, 1fr);
    align-content: start;
  }
  .t-desc {
    display: flex;
    flex-direction: column;
    gap: var(--w-s-3);
    min-width: 0;
  }
  .t-desc .md :global(:first-child) {
    margin-top: 0;
  }
  .t-desc-view {
    margin: -4px -8px;
    padding: 4px 8px;
    border-radius: var(--w-r-sm);
    cursor: text;
  }
  .t-desc-view:hover {
    background: var(--w-sunk);
  }
  .t-desc-view:focus-visible {
    outline: none;
    box-shadow: 0 0 0 2px var(--w-accent-soft);
  }
  .t-desc-add {
    align-self: flex-start;
    margin-left: -8px;
    border: 0;
    border-radius: var(--w-r-sm);
    padding: 4px 8px;
    background: none;
    color: var(--w-muted);
    font-size: var(--w-fs-body);
    cursor: text;
  }
  .t-desc-add:hover {
    background: var(--w-sunk);
    color: var(--w-ink);
  }
  .t-desc-edit {
    width: calc(100% + 16px);
    box-sizing: border-box;
    margin: -4px -8px;
    min-height: 120px;
    border: 0;
    border-radius: var(--w-r-sm);
    padding: 4px 8px;
    background: var(--w-sunk);
    box-shadow: 0 0 0 2px var(--w-accent-soft);
    resize: none;
    overflow: hidden;
    font-size: var(--w-fs-body);
    line-height: 1.5;
    user-select: text;
    -webkit-user-select: text;
  }
  .t-desc-edit:focus {
    outline: none;
  }
  .t-conflict {
    display: flex;
    align-items: center;
    gap: var(--w-s-2);
    margin-top: var(--w-s-1);
    color: var(--w-warn);
    font-size: var(--w-fs-small);
  }
  .t-side {
    display: grid;
    gap: var(--w-s-4);
    align-content: start;
    border-left: 1px solid var(--w-line);
    padding-left: 24px;
  }
  .panel .t-side {
    border-left: 0;
    padding-left: 0;
    border-top: 1px solid var(--w-line);
    padding-top: var(--w-s-4);
  }
  .t-group {
    display: grid;
    gap: var(--w-s-2);
  }
  .t-kv {
    display: grid;
    grid-template-columns: auto 1fr;
    gap: 6px 12px;
    margin: 0;
    font-size: var(--w-fs-small);
    align-items: center;
  }
  .t-agent-acts {
    display: flex;
    flex-wrap: wrap;
    gap: var(--w-s-2);
  }
  .t-agent-acts .w-btn {
    white-space: nowrap;
  }
  .t-kv dt {
    color: var(--w-muted);
  }
  .t-kv dd {
    margin: 0;
    display: flex;
    align-items: center;
    gap: 7px;
    min-width: 0;
    min-height: 28px;
  }
  .t-kv dd.t-agent-pick {
    flex-direction: column;
    align-items: stretch;
    gap: 2px;
  }
  .t-agent-sub {
    padding-left: 8px;
  }
  .t-kv .w-seg button {
    padding: 2px 9px;
  }
  .field {
    width: 100%;
    min-width: 0;
    box-sizing: border-box;
    border: 0;
    border-radius: var(--w-r-sm);
    background: var(--w-tray);
    padding: 4px 8px;
    font-size: var(--w-fs-small);
  }
  .field.w-mono {
    font-size: var(--w-fs-micro);
  }
  .field:focus {
    outline: none;
    box-shadow: 0 0 0 2px var(--w-accent-soft);
  }
  .field.waiting {
    color: var(--w-warn);
  }
  .t-link {
    flex: none;
    border: 0;
    padding: 0;
    background: none;
    color: var(--w-muted);
    font: inherit;
    font-size: var(--w-fs-small);
    cursor: pointer;
    white-space: nowrap;
  }
  .t-link:hover:not(:disabled) {
    color: var(--w-ink);
  }
  .t-link:disabled {
    cursor: default;
  }
  .t-acc {
    display: grid;
    gap: 6px;
    margin: 0;
    padding: 0;
    list-style: none;
  }
  .t-acc li {
    display: flex;
    gap: 8px;
    align-items: flex-start;
    font-size: var(--w-fs-small);
    line-height: 1.35;
  }
  .t-acc input {
    margin: 2px 0 0;
    flex: none;
  }
  .t-acc li.done label {
    text-decoration: line-through;
    color: var(--w-muted);
  }
  .t-acc-add {
    border: 0;
    background: var(--w-tray);
    border-radius: var(--w-r-sm);
    padding: 5px 8px;
    font-size: var(--w-fs-small);
  }
  .t-acc-add:focus {
    outline: none;
    box-shadow: 0 0 0 2px var(--w-accent-soft);
  }
  .t-child {
    display: flex;
    align-items: baseline;
    gap: var(--w-s-2);
    min-width: 0;
    border: 0;
    border-radius: var(--w-r-sm);
    padding: 2px 0;
    background: none;
    color: inherit;
    font: inherit;
    font-size: var(--w-fs-small);
    text-align: left;
    cursor: pointer;
  }
  .t-child .w-mono {
    flex: none;
    color: var(--w-muted);
  }
  .t-child-title {
    flex: 1;
    min-width: 0;
    overflow: hidden;
    white-space: nowrap;
    text-overflow: ellipsis;
  }
  .t-child:hover .t-child-title {
    text-decoration: underline;
  }
  .t-child.done .t-child-title {
    text-decoration: line-through;
    color: var(--w-muted);
  }
  .t-child-status {
    flex: none;
    color: var(--w-muted);
  }
  .t-update {
    background: var(--w-sunk);
    border-radius: var(--w-r-md);
    padding: 8px 10px;
    font-size: var(--w-fs-small);
    line-height: 1.45;
    white-space: pre-line;
    user-select: text;
    -webkit-user-select: text;
  }
  .t-update .w-mono {
    display: block;
    color: var(--w-muted);
    margin-bottom: 2px;
  }
  .t-stale {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: var(--w-s-2);
    color: var(--w-warn);
    font-size: var(--w-fs-small);
  }
  .t-stale .w-btn {
    flex: none;
    white-space: nowrap;
  }
  .t-run {
    display: flex;
    gap: var(--w-s-2);
    min-width: 0;
    border: 0;
    border-radius: var(--w-r-sm);
    padding: 2px 0;
    background: none;
    color: inherit;
    font: inherit;
    font-size: var(--w-fs-small);
    text-align: left;
    cursor: pointer;
  }
  .t-run .w-mono {
    flex: none;
    color: var(--w-muted);
  }
  .t-run-text {
    min-width: 0;
    overflow: hidden;
    white-space: nowrap;
    text-overflow: ellipsis;
  }
  .t-run.open .t-run-text {
    white-space: normal;
    user-select: text;
    -webkit-user-select: text;
  }
  .t-file {
    display: flex;
    align-items: center;
    gap: var(--w-s-2);
    min-width: 0;
    border: 0;
    padding: 2px 0;
    background: none;
    color: inherit;
    font: inherit;
    font-size: var(--w-fs-small);
    text-align: left;
    cursor: pointer;
  }
  .t-file-path {
    min-width: 0;
    overflow: hidden;
    white-space: nowrap;
    text-overflow: ellipsis;
  }
  .t-kind {
    flex: none;
    width: 1.2em;
    color: var(--w-muted);
  }
  .t-kind--new,
  .t-diff .add {
    color: var(--w-ok);
  }
  .t-kind--deleted,
  .t-diff .del {
    color: var(--w-danger);
  }
  .t-drift,
  .t-files-error {
    color: var(--w-warn);
    font-size: var(--w-fs-small);
  }
  .t-drift {
    flex: none;
    margin-left: auto;
  }
  .t-diff {
    margin: 0;
    max-height: 320px;
    overflow: auto;
    border-radius: var(--w-r-sm);
    padding: var(--w-s-2);
    background: var(--w-sunk);
    font-family: var(--w-font-mono);
    font-size: var(--w-fs-micro);
    user-select: text;
    -webkit-user-select: text;
  }
  .t-diff span {
    display: block;
    min-height: 1lh;
    white-space: pre;
  }
  .t-diff .hunk {
    color: var(--w-info);
  }
  .t-diff .meta {
    color: var(--w-muted);
  }
  .t-more {
    justify-self: start;
    border: 0;
    padding: 0;
    background: none;
    color: var(--w-muted);
    font: inherit;
    font-size: var(--w-fs-small);
    cursor: pointer;
  }
  .t-more:hover {
    color: var(--w-ink);
  }
  .t-compose {
    width: 100%;
    box-sizing: border-box;
    border: 0;
    border-radius: var(--w-r-md);
    background: var(--w-tray);
    padding: 8px 10px;
    resize: vertical;
    min-height: 60px;
    font-size: var(--w-fs-small);
  }
  .t-compose:focus {
    outline: none;
    box-shadow: 0 0 0 2px var(--w-accent-soft);
  }
  .t-foot {
    display: flex;
    align-items: center;
    gap: var(--w-s-2);
    padding: 12px 22px;
    background: var(--w-sunk);
    border-top: 1px solid var(--w-line);
  }
  .t-foot .w-btn--primary {
    margin-left: auto;
  }
  .w-btn:disabled {
    opacity: 0.5;
    cursor: default;
  }
  .danger:hover {
    color: var(--w-danger);
    background: var(--w-danger-soft);
  }
  @media (max-width: 680px) {
    .t-body {
      grid-template-columns: 1fr;
    }
    .t-side {
      border-left: 0;
      padding-left: 0;
    }
  }
  @media (prefers-reduced-motion: reduce) {
    .panel {
      animation: none;
    }
  }
</style>
