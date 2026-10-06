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
  agent: { active: string | null; ready: boolean } | null;
  [field: string]: unknown;
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
  usecase: { step: string | null; status: string | null } | null;
  [field: string]: unknown;
};
export type ParseError = { path: string; line: number | null; message: string };
export type Step = { id: string; label: string; kind: string };
export type Snapshot = {
  root: string;
  config: { task_statuses: { id: string; label: string; wip_limit: number | null }[] };
  process: { steps: Step[] } | null;
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
  [key: string]: unknown;
};

export const workspace = $state({
  index: null as Snapshot | null,
  settings: null as Settings | null,
  home: "",
  /** Last failed command; shown as a banner until dismissed. */
  error: null as string | null,
  /** True once the first load finished, so the UI does not flash the onboarding. */
  ready: false,
  reloads: 0,
  loadedAt: "",
});

async function load() {
  try {
    workspace.index = await invoke<Snapshot>("get_index");
    workspace.reloads++;
    workspace.loadedAt = new Date().toLocaleTimeString();
  } catch {
    // No workspace open: the layout shows the onboarding.
    workspace.index = null;
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
  await load();
  listen("workspace-changed", load);
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

export const updateTaskField = (id: string, field: string, value: unknown) => run("update_task_field", { id, field, value });
export const createTask = (title: string, project: string | null) => run<string>("create_task", { title, project });
export const moveTask = (id: string, project: string | null) => run("move_task", { id, project });
export const deleteTask = (id: string) => run("delete_task", { id });
export const restore = (path: string) => run("restore", { path });

// --------------------------------------------------------------- projects

export const COLORS = ["proj-1", "proj-2", "proj-3", "proj-4", "proj-5", "proj-6"];
export const PROJECT_STATUSES = ["active", "paused", "archived"];

export type NewProject = { title: string; key: string; color: string; repos: string[] };

export const suggestKey = (title: string) => invoke<string>("suggest_key", { title });
/** Throws, so the form can show the error next to its fields. */
export const createProject = (project: NewProject) => invoke<string>("create_project", { project });
export const updateProjectField = (key: string, field: string, value: unknown) =>
  invoke("update_project_field", { key, field, value });
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

// ------------------------------------------------------- open elsewhere

export const openUrl = (url: string) => run("open_url", { url });
export const openInVscode = (path: string) => run("open_in_vscode", { path });
export const openProjectInVscode = (key: string) => run("open_project_in_vscode", { key });
export const openRepo = (repo: string) => run("open_repo", { repo });
export const reveal = (path: string) => run("reveal", { path });
export const openInObsidian = (path: string) =>
  openUrl(`obsidian://open?path=${encodeURIComponent(`${workspace.index?.root}/${path}`)}`);

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
