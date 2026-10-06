import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";

// Mirrors workly-core's Index (crates/workly-core/src/scan.rs).
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
export type ProjectEntry = { path: string; key: string; title: string; status: string; color: string | null; [field: string]: unknown };
export type ParseError = { path: string; line: number | null; message: string };
export type Index = { projects: ProjectEntry[]; tasks: TaskEntry[]; trash: string[]; errors: ParseError[] };

export const workspace = $state({
  index: null as Index | null,
  error: null as string | null,
  reloads: 0,
  loadedAt: "",
});

async function load() {
  try {
    workspace.index = await invoke<Index>("get_index");
    workspace.error = null;
    workspace.reloads++;
    workspace.loadedAt = new Date().toLocaleTimeString();
  } catch (e) {
    workspace.error = String(e);
  }
}

let started = false;
export function startWorkspace() {
  if (started) return;
  started = true;
  load();
  listen("workspace-changed", load);
}

/** Run a command; errors land in `workspace.error`. The index reloads via the event. */
async function run<T>(cmd: string, args: Record<string, unknown>): Promise<T | undefined> {
  try {
    return await invoke<T>(cmd, args);
  } catch (e) {
    workspace.error = String(e);
  }
}

export const updateTaskField = (id: string, field: string, value: unknown) => run("update_task_field", { id, field, value });
export const createTask = (title: string, project: string | null) => run<string>("create_task", { title, project });
export const moveTask = (id: string, project: string | null) => run("move_task", { id, project });
export const deleteTask = (id: string) => run("delete_task", { id });
export const restore = (path: string) => run("restore", { path });
