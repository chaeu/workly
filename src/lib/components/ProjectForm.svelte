<script lang="ts">
  // Create (project = null) or edit a project. Every edit is one field write in the core.
  import { ask } from "@tauri-apps/plugin-dialog";
  import { goto } from "$app/navigation";
  import { knownAreas } from "$lib/usecases.svelte";
  import {
    workspace,
    COLORS,
    PROJECT_STATUSES,
    suggestKey,
    createProject,
    makeUseCase,
    updateProjectField,
    deleteProject,
    nextColor,
    pickRepo,
    type ProjectEntry,
    type Link,
  } from "$lib/stores/workspace.svelte";

  // `usecase`: create as a use case (type + area); `oncreated` replaces the jump to the project page.
  let {
    project = null,
    usecase = false,
    onclose,
    oncreated,
  }: { project?: ProjectEntry | null; usecase?: boolean; onclose: () => void; oncreated?: (key: string) => void } = $props();

  // Form state starts as a copy of the project; nothing is written until Save.
  // svelte-ignore state_referenced_locally
  const p = project;
  let title = $state(p?.title ?? "");
  let key = $state("");
  let keyEdited = $state(false);
  let color = $state(p?.color ?? nextColor());
  let status = $state(p?.status ?? "active");
  let repos = $state<string[]>([...(p?.repos ?? [])]);
  let links = $state<{ label: string; url: string }[]>((p?.links ?? []).map((l) => ({ label: l.label ?? "", url: l.url })));
  const process = $derived(workspace.index?.process);
  let ucType = $state(workspace.index?.process?.types[0]?.id ?? "");
  let ucArea = $state("");
  // Edit form: "Make use case…" reveals type + area; Save then writes the usecase block.
  let makeUc = $state(false);
  const ucFields = $derived(!!process && (p ? makeUc : usecase));
  // Title whose folder already exists without _project.md: offer to use it.
  let existsFor = $state<string | null>(null);
  let error = $state<string | null>(null);
  let busy = $state(false);

  // Key follows the title until the user types one.
  $effect(() => {
    const t = title;
    if (p || keyEdited) return;
    if (!t.trim()) key = "";
    else suggestKey(t).then((k) => !keyEdited && (key = k));
  });

  const keyProblem = $derived.by(() => {
    if (p || !key) return null;
    if (!/^[A-Z][A-Z0-9]{1,5}$/.test(key)) return "2-6 uppercase letters or digits, starting with a letter";
    if (key === "IN") return "IN is reserved for the inbox";
    if (workspace.index?.projects.some((x) => x.key === key)) return "Already used";
    return null;
  });
  const canSave = $derived(!!title.trim() && (p || (key && !keyProblem)) && !busy);

  async function addRepo() {
    const repo = await pickRepo();
    if (repo && !repos.includes(repo)) repos = [...repos, repo];
  }

  // A link without label is written as `{ url: ... }`.
  const linkValue = (ls: { label: string | null; url: string }[]): Link[] =>
    ls.filter((l) => l.url.trim()).map((l) => (l.label?.trim() ? { label: l.label.trim(), url: l.url.trim() } : { url: l.url.trim() }) as Link);

  const ucValue = () => ({ type: ucType, area: ucArea.trim() || null });

  async function create(adopt: boolean) {
    const dir = await createProject({ title: title.trim(), key, color, repos: $state.snapshot(repos), usecase: usecase ? ucValue() : null, adopt });
    if (dir === null) {
      existsFor = title.trim();
      return;
    }
    onclose();
    if (oncreated) oncreated(key);
    else goto(`/projects/${key}`);
  }

  async function save(e: SubmitEvent | null, adopt = false) {
    e?.preventDefault();
    if (!canSave) return;
    busy = true;
    error = null;
    try {
      if (!p) {
        await create(adopt);
        return;
      }
      const changes: [string, unknown, unknown][] = [
        ["title", p.title, title.trim()],
        ["color", p.color, color],
        ["status", p.status, status],
        ["repos", p.repos, $state.snapshot(repos)],
        ["links", linkValue(p.links), linkValue(links)],
      ];
      for (const [field, before, after] of changes) {
        if (JSON.stringify(before) !== JSON.stringify(after)) await updateProjectField(p.key, field, after);
      }
      if (makeUc) await makeUseCase(p.key, ucValue());
      onclose();
    } catch (err) {
      error = String(err);
    } finally {
      busy = false;
    }
  }

  async function remove() {
    if (!p) return;
    const ok = await ask(`Move "${p.title}" with all its tasks and files to .workly/trash/?`, {
      title: "Delete project",
      kind: "warning",
      okLabel: "Delete",
    });
    if (!ok) return;
    await deleteProject(p.key);
    onclose();
    goto("/projects");
  }

  function onkeydown(e: KeyboardEvent) {
    if (e.key === "Escape") onclose();
  }
</script>

<svelte:window {onkeydown} />

<div class="w-scrim" role="presentation" onclick={onclose}></div>
<div class="w-modal" role="dialog" aria-modal="true" aria-labelledby="pf-title">
  <form onsubmit={save}>
    <div class="body">
      <h2 class="w-h2" id="pf-title">{p ? `Edit ${p.key}` : usecase ? "New use case" : "New project"}</h2>

      <label class="field">
        <span class="w-caps">Title</span>
        <!-- svelte-ignore a11y_autofocus -->
        <input bind:value={title} autofocus placeholder="Website Relaunch" />
      </label>

      {#if !p}
        <label class="field">
          <span class="w-caps">Key</span>
          <input
            class="w-mono key"
            value={key}
            maxlength="6"
            oninput={(e) => {
              keyEdited = true;
              key = e.currentTarget.value.toUpperCase();
            }}
          />
          <span class="hint" class:bad={keyProblem}>{keyProblem ?? "Task ids start with it, e.g. " + (key || "WR") + "-1. Fixed after creation."}</span>
        </label>
      {/if}

      <div class="field">
        <span class="w-caps">Colour</span>
        <div class="swatches" role="radiogroup" aria-label="Colour">
          {#each COLORS as c, i (c)}
            <button
              type="button"
              class="swatch"
              role="radio"
              aria-checked={color === c}
              aria-label="Colour {i + 1}"
              style:--c="var(--w-{c})"
              onclick={() => (color = c)}
            ></button>
          {/each}
        </div>
      </div>

      {#if p && !p.usecase && process && !makeUc}
        <div class="field">
          <span class="w-caps">Use case</span>
          <div><button type="button" class="w-btn" onclick={() => (makeUc = true)}>Make use case…</button></div>
        </div>
      {/if}

      {#if ucFields && process}
        <div class="field">
          <span class="w-caps">Type</span>
          <div class="w-seg" role="group" aria-label="Type">
            {#each process.types as t (t.id)}
              <button type="button" aria-pressed={ucType === t.id} onclick={() => (ucType = t.id)}>{t.label}</button>
            {/each}
          </div>
        </div>
        <label class="field">
          <span class="w-caps">Area</span>
          <input list="new-area-list" bind:value={ucArea} placeholder="Type or pick an area" autocomplete="off" />
          <datalist id="new-area-list">{#each knownAreas(process) as a (a)}<option value={a}></option>{/each}</datalist>
        </label>
      {/if}

      {#if p}
        <div class="field">
          <span class="w-caps">Status</span>
          <div class="w-seg" role="group" aria-label="Status">
            {#each PROJECT_STATUSES as s (s)}
              <button type="button" aria-pressed={status === s} onclick={() => (status = s)}>{s[0].toUpperCase() + s.slice(1)}</button>
            {/each}
          </div>
        </div>
      {/if}

      <div class="field">
        <span class="w-caps">Repos</span>
        {#each repos as r, i (r)}
          <div class="list-row">
            <span class="w-mono grow">{r}</span>
            <button type="button" class="w-btn w-btn--quiet" onclick={() => repos.splice(i, 1)}>Remove</button>
          </div>
        {/each}
        <div><button type="button" class="w-btn" onclick={addRepo}>Add repo…</button></div>
      </div>

      {#if p}
        <div class="field">
          <span class="w-caps">Links</span>
          {#each links as l, i (i)}
            <div class="list-row">
              <input class="label" bind:value={l.label} placeholder="Label" aria-label="Link label" />
              <input class="grow" bind:value={l.url} placeholder="https://" aria-label="Link URL" />
              <button type="button" class="w-btn w-btn--quiet" onclick={() => links.splice(i, 1)}>Remove</button>
            </div>
          {/each}
          <div><button type="button" class="w-btn" onclick={() => links.push({ label: "", url: "" })}>Add link</button></div>
        </div>
      {/if}

      {#if !p && existsFor && existsFor === title.trim()}
        <div class="exists" role="alert">
          <p class="hint">A folder for “{existsFor}” already exists and is not a project yet. Existing files stay untouched.</p>
          <div><button type="button" class="w-btn" disabled={!canSave} onclick={() => save(null, true)}>Use existing folder</button></div>
        </div>
      {/if}
      {#if error}<p class="hint bad" role="alert">{error}</p>{/if}
    </div>

    <div class="actions">
      {#if p}
        <button type="button" class="w-btn w-btn--quiet danger" onclick={remove}>Delete…</button>
      {/if}
      <span class="grow"></span>
      <button type="button" class="w-btn w-btn--quiet" onclick={onclose}>Cancel</button>
      <button type="submit" class="w-btn w-btn--primary" disabled={!canSave}>{p ? "Save" : usecase ? "Create use case" : "Create project"}</button>
    </div>
  </form>
</div>

<style>
  .w-modal {
    width: min(560px, calc(100% - 32px));
  }
  form {
    display: flex;
    flex-direction: column;
    min-height: 0;
  }
  .body {
    display: flex;
    flex-direction: column;
    gap: var(--w-s-4);
    padding: var(--w-s-5) var(--w-s-6);
    overflow-y: auto;
  }
  .field {
    display: flex;
    flex-direction: column;
    gap: var(--w-s-2);
  }
  input {
    border: 0;
    outline: none;
    background: var(--w-tray);
    border-radius: var(--w-r-md);
    padding: 7px 11px;
    font-size: var(--w-fs-small);
    min-width: 0;
  }
  input:focus {
    box-shadow: 0 0 0 2px var(--w-accent-soft);
  }
  .key {
    width: 9ch;
    font-size: var(--w-fs-small);
  }
  .hint {
    margin: 0;
    font-size: var(--w-fs-caption);
    color: var(--w-muted);
  }
  .bad,
  .danger {
    color: var(--w-danger);
  }
  .exists {
    display: flex;
    flex-direction: column;
    gap: var(--w-s-2);
  }
  .swatches {
    display: flex;
    gap: var(--w-s-2);
  }
  .swatch {
    width: 28px;
    height: 28px;
    border: 0;
    border-radius: var(--w-r-sm);
    background: var(--c);
    cursor: pointer;
  }
  .swatch[aria-checked="true"] {
    box-shadow:
      0 0 0 2px var(--w-surface),
      0 0 0 4px var(--w-ink);
  }
  .list-row {
    display: flex;
    align-items: center;
    gap: var(--w-s-2);
  }
  .label {
    width: 130px;
  }
  .grow {
    flex: 1;
    min-width: 0;
  }
  .actions {
    display: flex;
    align-items: center;
    gap: var(--w-s-2);
    padding: var(--w-s-3) var(--w-s-6);
    background: var(--w-sunk);
  }
  .w-btn--primary:disabled {
    opacity: 0.5;
    cursor: default;
  }
</style>
