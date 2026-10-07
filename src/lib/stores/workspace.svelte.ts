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
  done_at: string | null;
  agent: AgentState | null;
  [field: string]: unknown;
};
export type AgentState = {
  ready: boolean;
  runner: string | null;
  model: string | null;
  effort: string | null;
  active: string | null;
  since: string | null;
  commit: string | null;
  last_run: string | null;
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
};
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
};
export type Snapshot = {
  root: string;
  config: { task_statuses: { id: string; label: string; wip_limit: number | null }[] };
  process: Process | null;
  /** Repos of live projects that are not folders on this Mac. */
  missing_repos: { key: string; repo: string }[];
  projects: ProjectEntry[];
  tasks: TaskEntry[];
  trash: string[];
  errors: ParseError[];
};
export type Settings = {
  workspaces: { name: string; path: string }[];
  active: string | null;
  repos_dir: string | null;
  theme: "system" | "light" | "dark" | null;
  focus_hidden?: boolean;
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
export const createTask = (title: string, project: string | null, priority: number | null = null) =>
  run<string>("create_task", { title, project, priority });
export const reorderTasks = (ids: string[]) => run("reorder_tasks", { ids });
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

export type NewProject = { title: string; key: string; color: string; repos: string[]; usecase?: { type: string; area: string | null } | null };

export const suggestKey = (title: string) => invoke<string>("suggest_key", { title });
/** Throws, so the form can show the error next to its fields. */
export const createProject = (project: NewProject) => invoke<string>("create_project", { project });
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

export const openTasks = (p: ProjectEntry) =>
  workspace.index?.tasks.filter((t) => t.project === p.path && t.status !== "done").length ?? 0;

export const stepLabel = (p: ProjectEntry) => {
  const step = p.usecase?.step;
  if (!step) return null;
  return workspace.index?.process?.steps.find((s) => s.id === step)?.label ?? step;
};

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
export const reveal = (path: string) => run("reveal", { path });
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
    await invoke("open_workspace", { path });
    workspace.settings = await invoke<Settings>("get_settings");
    workspace.error = null;
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
