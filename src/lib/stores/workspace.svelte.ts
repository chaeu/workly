import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { homeDir } from "@tauri-apps/api/path";
import { open as openDialog, save as saveDialog } from "@tauri-apps/plugin-dialog";

// Mirrors workly-core's Index (crates/workly-core/src/scan.rs) plus the Snapshot in src-tauri.
export type TaskEntry = {
  path: string;
  project: string | null;
  id: string;
  title: string;
  status: string;
  priority: number | null;
  due: string | null;
  tags: string[];
  focus: string | null;
  focus_order: number | null;
  order: number | null;
  created: string | null;
  done_at: string | null;
  /** Only while the status is `waiting`. */
  waiting_for: string | null;
  /** Parent task id; one level. The parent may be gone (trash). */
  parent: string | null;
  /** Items of the body's `## Acceptance` section, in file order (the index the toggle uses). */
  acceptance: AcceptanceItem[];
  agent: AgentState | null;
  [field: string]: unknown;
};
export type AcceptanceItem = { text: string; done: boolean };
/** Mirrors workly-core's NewTask: only `title` is required. */
export type NewTask = {
  title: string;
  project: string | null;
  priority?: number | null;
  due?: string | null;
  tags?: string[];
  parent?: string | null;
  description?: string;
  acceptance?: string[];
  runner?: string | null;
  model?: string | null;
  effort?: string | null;
  permissions?: string | null;
};
export type AgentState = {
  /** Deprecated (B2): still loaded from old files, never written. */
  ready: boolean;
  runner: string | null;
  model: string | null;
  effort: string | null;
  /** Codex only: ask | auto | full (B11); null = ask. */
  permissions?: string | null;
  active: string | null;
  since: string | null;
  commit: string | null;
  last_run: string | null;
  /** Repo paths the agent changed, set at review (B8); what Commit commits. */
  changes: string[] | null;
  /** Repo of `changes`, `~/…`. */
  repo: string | null;
};
export type Link = { label: string | null; url: string };
export type ProjectEntry = {
  path: string;
  key: string;
  title: string;
  status: string;
  color: string | null;
  order: number | null;
  repos: string[];
  links: Link[];
  created: string | null;
  /** false = a use case that is not a project (only in the cockpit); missing = true. */
  tracked: boolean | null;
  usecase: UseCase | null;
  [field: string]: unknown;
};
export type Decision = { date: string | null; gate: string | null; text: string };
/** A hand-written field that is missing or invalid arrives as null (listed under Problems). Unknown keys ride along. */
export type Saving = { what: string | null; count: number | null; per: string | null; minutes: number | null; [key: string]: unknown };
export type UseCase = {
  type: string | null;
  area: string | null;
  step: string | null;
  step_since: string | null;
  status: string | null;
  blocked_by: string | null;
  next_step: string | null;
  current_state: string | null;
  decisions: Decision[];
  savings: Saving[];
  savings_note: string | null;
  /** Invalid hand-written entries are left out (listed under Problems); missing = open. */
  assessment: UseCaseAssessment | null;
};
export type KoValue = "pass" | "fail" | "open";
export type UseCaseAssessment = { date: string | null; ko: Record<string, KoValue>; scores: Record<string, number>; note: string | null };
export type ParseError = { path: string; line: number | null; message: string };
// Mirrors model::Process: `.workly/process.yml`.
export type Labelled = { id: string; label: string };
export type Lane = { id: string; label: string; sub: string | null };
export type Phase = { id: string; name: string; desc: string | null; optional: boolean; parked: boolean };
export type Step = {
  id: string;
  kind: string; // box | gate | term
  col: number;
  lane: string;
  phase: string | null;
  label: string;
  sub: string | null;
  code: string | null;
  hint: string | null;
  optional: boolean;
  parked: boolean;
};
export type Edge = { from: string; to: string; route: string; label: string | null; label_dx: number | null; offset: number | null };
export type Process = {
  lanes: Lane[];
  phases: Phase[];
  steps: Step[];
  phase_default_step: Record<string, string>;
  board_gates: Record<string, string>;
  edges: Edge[];
  statuses: Labelled[];
  types: Labelled[];
  areas: string[];
  stale_after_days: number | null;
  /** Assessment method; null = no assessment anywhere in the app. */
  assessment: { ko: Labelled[]; criteria: Criterion[] } | null;
};
export type AgentPreset = { name: string; runner: string; model?: string | null; effort?: string | null; permissions?: string | null };
export type Criterion = { id: string; axis: "value" | "feasibility"; label: string; anchors: string[]; bonus: boolean; fte: [number, number] | null };
export type Snapshot = {
  root: string;
  config: {
    task_statuses: { id: string; label: string; wip_limit: number | null }[];
    agent_defaults?: { runner: string; model: string; effort: string; stale_after_minutes: number };
    /** The Agent menu from config.yml (B10); the first entry is the default. */
    agents?: AgentPreset[];
  };
  /** Last activity per task with an active agent: newest run-log entry of the run, else agent.since. */
  agent_activity: Record<string, string>;
  /** Tasks whose active agent stopped with a BLOCKED note: the note, by id. */
  agent_blocked: Record<string, string>;
  process: Process | null;
  /** Repos of live projects that are not folders on this Mac. */
  missing_repos: { key: string; repo: string }[];
  projects: ProjectEntry[];
  tasks: TaskEntry[];
  trash: string[];
  errors: ParseError[];
};
/** Per workspace and Mac (settings.json). Missing = hourly, push on. */
export type BackupSettings = { interval: "off" | "15m" | "hourly" | "daily"; push: boolean };
export const BACKUP_DEFAULTS: BackupSettings = { interval: "hourly", push: true };
/** Mirrors BackupStatus in src-tauri: runtime only, `at` in ms since the epoch. */
export type BackupStatus = { state: "off" | "running" | "ok" | "local" | "failed"; at: number | null; upstream: string | null; error: string | null };
/** Mirrors backup::BackupInfo; `url` without credentials. */
export type BackupInfo = { repo: boolean; branch: string | null; upstream: string | null; url: string | null; cloud: boolean };
export type Settings = {
  workspaces: { name: string; path: string; backup?: BackupSettings }[];
  active: string | null;
  repos_dir: string | null;
  theme: "system" | "light" | "dark" | null;
  /** App names for the file buttons; the core fills the default. */
  open_with: string[];
  focus_hidden?: boolean;
  /** Task board: hide tasks that have sub-tasks. */
  hide_parents?: boolean;
  task_detail?: "popup" | "panel";
  usecase_compact?: boolean;
  /** Missing = on. */
  agents_enabled?: boolean;
  /** Missing = 1720. */
  fte_hours_per_year?: number;
  [key: string]: unknown;
};
// Mirrors agent::AgentContext; paths relative to the workspace.
export type AgentFile = { path: string; content: string };
/** One line of `.workly/runs/<ID>/log.jsonl`. */
export type RunEntry = { ts: string; agent: string; run: string | null; text: string };
/** One event of the Agents timeline (core feed.rs), newest first. */
export type FeedEvent = { ts: string; kind: "start" | "note" | "blocked" | "review" | "commit"; agent: string; id: string; text: string };
/** Agents → Setup: what a start needs on this Mac. */
export type AgentChecks = {
  codex: { path: string; version: string | null } | null;
  vscode: { path: string; version: string | null } | null;
  rules: "missing" | "builtin" | "custom";
  builtin_rules: string;
};
/** One path of agent.changes as it is now; `drifted` = edited after the review. */
export type FileChange = { path: string; kind: "new" | "modified" | "deleted" | "clean"; drifted: boolean };
export type Skill = { name: string; description: string; path: string };
export type AgentContext = { global: AgentFile | null; project: AgentFile | null; skills: Skill[] };
// Mirrors trash::TrashItem; `path` is relative to the trash and to the workspace.
export type TrashItem = { path: string; kind: "task" | "project" | "file"; id: string; title: string; deleted_at: string | null };
/** `~/.local/bin/wly`: null = no link; ok = false when its target is gone. */
export type CliLink = { target: string; ok: boolean } | null;

export const workspace = $state({
  index: null as Snapshot | null,
  settings: null as Settings | null,
  home: "",
  /** Last failed command; shown as a banner until dismissed. */
  error: null as string | null,
  /** True once the first load finished, so the UI does not flash the onboarding. */
  ready: false,
  /** Why no workspace is open, e.g. the saved folder is gone. */
  openError: null as string | null,
  /** undefined until checked. */
  cli: undefined as CliLink | undefined,
  reloads: 0,
  loadedAt: "",
  backup: null as BackupStatus | null,
});

async function load() {
  try {
    workspace.index = await invoke<Snapshot>("get_index");
    workspace.reloads++;
    workspace.loadedAt = new Date().toLocaleTimeString();
  } catch (e) {
    // No workspace open: the layout shows the onboarding, with the reason if there is one.
    workspace.index = null;
    workspace.openError = String(e) === "No workspace open." ? null : String(e);
  }
  workspace.ready = true;
}

let started = false;
export async function startWorkspace() {
  if (started) return;
  started = true;
  workspace.home = await homeDir();
  try {
    workspace.settings = await invoke<Settings>("get_settings");
  } catch (e) {
    workspace.error = String(e);
  }
  // Never fail silently: a rejected promise nobody caught shows as the banner.
  window.addEventListener("unhandledrejection", (e) => (workspace.error = String(e.reason)));
  await load();
  listen("workspace-changed", load);
  listen<BackupStatus>("backup-status", (e) => (workspace.backup = e.payload));
  workspace.backup = await invoke<BackupStatus>("backup_status");
  refreshCli();
}

/** Run a command; errors land in `workspace.error`. The index reloads via the event. */
async function run<T>(cmd: string, args: Record<string, unknown> = {}): Promise<T | undefined> {
  try {
    return await invoke<T>(cmd, args);
  } catch (e) {
    workspace.error = String(e);
  }
}

// ------------------------------------------------------------------ tasks

/** False when the write was refused, so an input can fall back to the file's value. */
export async function updateTaskField(id: string, field: string, value: unknown) {
  try {
    await invoke("update_task_field", { id, field, value });
    return true;
  } catch (e) {
    workspace.error = String(e);
    return false;
  }
}
export const createTask = (task: NewTask) => run<string>("create_task", { task });
/** `shown` is the item as displayed; core refuses the toggle when the file has something else there. */
export const toggleAcceptance = (id: string, n: number, shown: AcceptanceItem) => run("toggle_acceptance", { id, n, shown });
export const addAcceptance = (id: string, text: string) => run("add_acceptance", { id, text });
export const reorderTasks = (ids: string[]) => run("reorder_tasks", { ids });
/** Inserts the Waiting line into .workly/config.yml; throws when the list has a shape it cannot edit. */
export const addWaitingStatus = () => invoke("add_waiting_status");
export const setFocus = (ids: string[]) => run("set_focus", { ids });
export const addTaskUpdate = (id: string, text: string) => run("add_task_update", { id, text });
/** true = saved, false = changed outside since `expected` was read (nothing written). */
export const setDescription = (id: string, text: string, expected: string) =>
  run<boolean>("set_description", { id, text, expected });
export const moveTask = (id: string, project: string | null) => run("move_task", { id, project });
export const deleteTask = (id: string) => run("delete_task", { id });
export const restore = (path: string) => run("restore", { path });
export const trashItems = () => invoke<TrashItem[]>("trash_items");
/** Moves everything to the macOS Trash; returns the item count. */
export const emptyTrash = () => run<number>("empty_trash");

// --------------------------------------------------------------- projects

export const COLORS = ["proj-1", "proj-2", "proj-3", "proj-4", "proj-5", "proj-6"];
export const PROJECT_STATUSES = ["active", "paused", "archived"];

export type NewUseCase = { type: string; area: string | null };
export type NewProject = { title: string; key: string; color: string; repos: string[]; usecase?: NewUseCase | null; adopt?: boolean; tracked?: boolean };

export const suggestKey = (title: string) => invoke<string>("suggest_key", { title });
/** Throws, so the form can show the error next to its fields. Null = the folder exists without
 *  _project.md; nothing was written, `adopt: true` uses it. */
export const createProject = (project: NewProject) => invoke<string | null>("create_project", { project });
export const makeUseCase = (key: string, usecase: NewUseCase) => invoke("make_usecase", { key, usecase });
export const removeUseCase = (key: string) => run("remove_usecase", { key });
export const updateProjectField = (key: string, field: string, value: unknown) =>
  invoke("update_project_field", { key, field, value });
/** The one move for every path (board, map, decision button). False when refused. */
export async function moveUseCase(key: string, step: string) {
  try {
    await invoke("move_usecase", { key, step });
    return true;
  } catch (e) {
    workspace.error = String(e);
    return false;
  }
}
export const reorderProjects = (keys: string[]) => run("reorder_projects", { keys });
export const deleteProject = (key: string) => run("delete_project", { key });
export const projectFiles = (key: string) => invoke<string[]>("project_files", { key });
export const readMarkdown = (path: string) => invoke<string>("read_markdown", { path });
/** Per phase: time span and events, from .workly/log (core `history.rs`). */
export type PhaseSpan = {
  phase: string;
  start: string | null;
  end: string | null;
  events: { date: string; kind: "status" | "decision"; status: string | null; until: string | null; gate: string | null; text: string | null }[];
};
export const usecaseHistory = (key: string) => invoke<PhaseSpan[]>("usecase_history", { key });

export const openTasks = (p: ProjectEntry) =>
  workspace.index?.tasks.filter((t) => t.project === p.path && t.status !== "done").length ?? 0;

export const stepLabel = (p: ProjectEntry) => {
  const step = p.usecase?.step;
  if (!step) return null;
  return workspace.index?.process?.steps.find((s) => s.id === step)?.label ?? step;
};

/** Sidebar, Projects table and task pickers: not archived and tracked as a project
 *  (`tracked: false` = a use case that lives only in the cockpit). */
export const inDailyWork = (p: ProjectEntry) => p.status !== "archived" && p.tracked !== false;

/** First colour no active project uses; round robin when all are taken. */
export function nextColor() {
  const live = workspace.index?.projects.filter((p) => p.status !== "archived") ?? [];
  return COLORS.find((c) => !live.some((p) => p.color === c)) ?? COLORS[live.length % COLORS.length];
}

/** CSS colour for a project's `color` field; unknown values fall back to muted. */
export const projColor = (color: string | null) => (color && COLORS.includes(color) ? `var(--w-${color})` : "var(--w-muted)");

// ----------------------------------------------------------------- agents

/** The one switch every agent UI checks (device setting `agents_enabled`, default off). */
export const agentsEnabled = () => workspace.settings?.agents_enabled === true;

export const agentContext = (key: string | null) => invoke<AgentContext>("agent_context", { key });
export const createAgentsMd = (key: string) => run<string>("create_agents_md", { key });
export const runLog = (id: string) => invoke<RunEntry[]>("run_log", { id });
export const agentFeed = () => invoke<FeedEvent[]>("agent_feed");
export const agentChecks = () => invoke<AgentChecks>("agent_checks");
export const clearAgent = (id: string) => run("clear_agent", { id });
/** The whole agents list of config.yml; throws so Settings can keep the edit and show why. */
export const setAgents = (agents: AgentPreset[]) => invoke("set_agents", { agents: $state.snapshot(agents) });
/** A task's agent.runner/model/effort from one entry; null = the default agent. */
export const setTaskAgent = (id: string, agent: AgentPreset | null) => run("set_task_agent", { id, agent: $state.snapshot(agent) });
/** Throw, so Start agent can stop on a refused start and fall back to the clipboard on a failed launch. */
export const startAgentTask = (id: string, agent: string) => invoke("start_agent_task", { id, agent });
export const launchAgent = (id: string, runner: string, prompt: string) => invoke("launch_agent", { id, runner, prompt });
/** Copy prompt on a task in doing: the repo baseline for review, unless it has one. */
export const ensureBaseline = (id: string) => invoke<boolean>("ensure_baseline", { id }).catch(() => false);
export const taskChanges = (id: string) => invoke<FileChange[]>("task_changes", { id });
export const changeDiff = (id: string, path: string) => invoke<string>("change_diff", { id, path });
/** Throws with git's message; returns the short sha. */
export const commitTask = (id: string, acceptDrift: boolean) => invoke<string>("commit_task", { id, acceptDrift });
export async function refreshCli() {
  workspace.cli = await invoke<CliLink>("cli_link").catch(() => null);
}
/** Throws, so Settings can show the reason next to the button. */
export const installCli = () => invoke<string>("install_cli");

// ------------------------------------------------------- open elsewhere

export const openUrl = (url: string) => run("open_url", { url });
export const openInVscode = (path: string, line: number | null = null) => run("open_in_vscode", { path, line });
export const openProjectInVscode = (key: string) => run("open_project_in_vscode", { key });
export const openRepo = (repo: string) => run("open_repo", { repo });
/** False when pbcopy failed; the reason goes to workspace.error like other commands. */
export const copyText = (text: string) =>
  invoke("copy_text", { text }).then(
    () => true,
    (e) => ((workspace.error = String(e)), false),
  );
export const reveal = (path: string) => run("reveal", { path });
/** Apps for the file buttons; the default only until settings have loaded. */
export const openWithApps = () => workspace.settings?.open_with ?? ["Obsidian", "Visual Studio Code"];
export const appLabel = (app: string) => (app === "Visual Studio Code" ? "VS Code" : app);
export const openWith = (app: string, path: string) => run("open_with", { app, path });
export const openInObsidian = (path: string) =>
  openUrl(`obsidian://open?path=${encodeURIComponent(`${workspace.index?.root}/${path}`)}`);

// --------------------------------------------------------------- problems

/** Broken files, repos missing on this Mac, and a missing or broken CLI while agent features are on. */
export function problemCount() {
  const idx = workspace.index;
  if (!idx) return 0;
  const cli = agentsEnabled() && workspace.cli !== undefined && !workspace.cli?.ok ? 1 : 0;
  return idx.errors.length + idx.missing_repos.length + cli;
}

// --------------------------------------------------------------- settings

/** `/Users/me/x` -> `~/x` for display and for paths stored in shared files. */
export const tilde = (path: string) =>
  workspace.home && (path === workspace.home || path.startsWith(workspace.home + "/")) ? "~" + path.slice(workspace.home.length) : path;

export const expandHome = (path: string) => (path.startsWith("~/") ? workspace.home + path.slice(1) : path);

export async function saveSettings(patch: Partial<Settings>) {
  // Settings failed to load (broken file): never overwrite it with a partial copy.
  if (!workspace.settings) return;
  const next = { ...workspace.settings, ...patch };
  try {
    await invoke("save_settings", { settings: next });
    workspace.settings = next;
  } catch (e) {
    workspace.error = String(e);
  }
}

export async function openWorkspace(path: string) {
  try {
    // A warning: the workspace opened, but backup could not be set up (OneDrive, iCloud).
    const warning = await invoke<string | null>("open_workspace", { path });
    workspace.settings = await invoke<Settings>("get_settings");
    workspace.error = warning;
    workspace.openError = null;
  } catch (e) {
    workspace.error = String(e);
  }
}

/** Pick an existing folder; one without `.workly/` is initialised. */
export async function chooseWorkspace() {
  const path = await openDialog({ directory: true, title: "Choose a workspace folder" });
  if (typeof path === "string") await openWorkspace(path);
}

/** Name a new folder; it is created and initialised. */
export async function createWorkspace() {
  const path = await saveDialog({ title: "Create a workspace", defaultPath: "Workspace" });
  if (path) await openWorkspace(path);
}

/** Folder picker for a repo; result in `~/` form. */
export async function pickRepo() {
  const start = workspace.settings?.repos_dir;
  const path = await openDialog({ directory: true, title: "Choose a repo", defaultPath: start ? expandHome(start) : undefined });
  return typeof path === "string" ? tilde(path) : null;
}

// ----------------------------------------------------------------- backup

/** Starts a run; the result arrives as `workspace.backup`. */
export const backupNow = () => run("backup_now");
export const backupInfo = () => invoke<BackupInfo>("backup_info");
/** Throws, so Settings can show the reason next to the button. */
export const enableBackup = () => invoke("enable_backup");

/** "12 min ago" for the backup status; `now` from a ticking clock. */
export function ago(at: number, now: number) {
  const min = Math.floor((now - at) / 60000);
  if (min < 1) return "just now";
  if (min < 60) return `${min} min ago`;
  if (min < 24 * 60) return `${Math.floor(min / 60)} h ago`;
  return `${Math.floor(min / (24 * 60))} d ago`;
}
