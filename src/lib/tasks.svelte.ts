import { ask } from "@tauri-apps/plugin-dialog";
import { workspace, deleteTask, type TaskEntry, type ProjectEntry } from "$lib/stores/workspace.svelte";

/** Ticks once a minute for the agent badge; nothing else runs while idle. */
export const clock = $state({ now: Date.now() });
let ticking = false;
export function startClock() {
  if (ticking) return;
  ticking = true;
  setInterval(() => (clock.now = Date.now()), 60_000);
}

const pad = (n: number) => String(n).padStart(2, "0");
/** Local date as `YYYY-MM-DD`, like the core writes it. */
export const isoDate = (d: Date) => `${d.getFullYear()}-${pad(d.getMonth() + 1)}-${pad(d.getDate())}`;
export const today = () => isoDate(new Date(clock.now));

/** Sunday of the current week (weeks start on Monday). */
export function endOfWeek() {
  const d = new Date(clock.now);
  d.setDate(d.getDate() + ((7 - d.getDay()) % 7));
  return isoDate(d);
}

/** `2026-10-10` -> `Oct 10`; anything else is shown as written. */
export function shortDate(iso: string | null) {
  if (!iso) return "";
  const d = new Date(`${iso.slice(0, 10)}T00:00`);
  return isNaN(d.getTime()) ? iso : d.toLocaleDateString("en-US", { month: "short", day: "numeric" });
}

/** Tooltip for a due date: the full date and how far away it is. */
export function dueTip(iso: string) {
  const days = Math.round((Date.parse(iso.slice(0, 10)) - Date.parse(today())) / 864e5);
  const d = new Date(`${iso.slice(0, 10)}T00:00`);
  if (isNaN(days) || isNaN(d.getTime())) return `Due ${iso}`;
  const when = days < 0 ? `${-days} ${-days === 1 ? "day" : "days"} overdue` : days === 0 ? "Today" : days === 1 ? "Tomorrow" : `In ${days} days`;
  return `Due ${d.toLocaleDateString("en-US", { weekday: "short", month: "short", day: "numeric", year: "numeric" })}\n${when}`;
}

export const isOverdue = (t: TaskEntry) => !!t.due && t.status !== "done" && t.due.slice(0, 10) < today();

/**
 * The one open task to do next: overdue (oldest due first), then in today's focus (focus order),
 * priority, due date, id. Backlog counts only when nothing else is open.
 */
export function nextUp(tasks: TaskEntry[]) {
  const open = tasks.filter((t) => t.status !== "done");
  const pool = open.some((t) => t.status !== "backlog") ? open.filter((t) => t.status !== "backlog") : open;
  const day = today();
  // "~" sorts after every date, so a missing value goes last.
  const rank = (t: TaskEntry) => [
    isOverdue(t) ? t.due!.slice(0, 10) : "~",
    t.focus?.slice(0, 10) === day ? (t.focus_order ?? 9) : 99,
    t.priority ?? 9,
    t.due?.slice(0, 10) ?? "~",
  ];
  const cmp = (a: TaskEntry, b: TaskEntry) => {
    const [x, y] = [rank(a), rank(b)];
    const i = x.findIndex((v, n) => v !== y[n]);
    return i >= 0 ? (x[i] < y[i] ? -1 : 1) : a.id.localeCompare(b.id, "en", { numeric: true });
  };
  return pool.sort(cmp)[0] ?? null;
}

const AGENT_NAMES: Record<string, string> = { codex: "Codex", copilot: "Copilot", "claude-code": "Claude Code" };
export const agentName = (id: string) => AGENT_NAMES[id] ?? id.charAt(0).toUpperCase() + id.slice(1);

/** "Codex working · 14 min" while `agent.active` is set. */
export function agentBadge(t: TaskEntry) {
  const a = t.agent;
  if (!a?.active) return null;
  const since = a.since ? Date.parse(a.since) : NaN;
  if (isNaN(since)) return `${agentName(a.active)} working`;
  const min = Math.max(0, Math.floor((clock.now - since) / 60_000));
  const took = min < 60 ? `${min} min` : `${Math.floor(min / 60)} h ${min % 60} min`;
  return `${agentName(a.active)} working · ${took}`;
}

/** Column order: `order`, then priority, then id (`WR-9` before `WR-10`). Missing values go last. */
export const byBoardOrder = (a: TaskEntry, b: TaskEntry) =>
  (a.order ?? Infinity) - (b.order ?? Infinity) ||
  (a.priority ?? 9) - (b.priority ?? 9) ||
  a.id.localeCompare(b.id, "en", { numeric: true });

/** Owning project of a task; `undefined` for the inbox. */
export const projectOf = (t: TaskEntry): ProjectEntry | undefined =>
  t.project ? workspace.index?.projects.find((p) => p.path === t.project) : undefined;

/** Description and `## Updates` items of a task body. */
export function splitBody(body: string) {
  const lines = body.split(/\r?\n/);
  const head = lines.findIndex((l) => l.trimEnd() === "## Updates");
  if (head < 0) return { description: body, updates: [] as { meta: string; text: string }[] };
  let end = lines.findIndex((l, i) => i > head && /^#{1,2} /.test(l));
  if (end < 0) end = lines.length;
  const updates = lines
    .slice(head + 1, end)
    .filter((l) => /^[-*] /.test(l))
    .map((l) => {
      const item = l.slice(2);
      const colon = item.indexOf(": ");
      return colon > 0 ? { meta: item.slice(0, colon), text: item.slice(colon + 2) } : { meta: "", text: item };
    });
  return { description: [...lines.slice(0, head), ...lines.slice(end)].join("\n"), updates };
}

/** The editable description: body before the `## Updates` line, raw (same rule as core's description_end). */
export function rawDescription(body: string) {
  const m = /^## Updates[^\S\n]*$/m.exec(body);
  return (m ? body.slice(0, m.index) : body).trimEnd();
}

/** Quick add (⌘N) lives in the layout. `hint` is the project the board filter suggests. */
export const quickAdd = $state({ open: false, project: null as string | null, hint: null as string | null });
export function openQuickAdd(project: string | null = quickAdd.hint) {
  quickAdd.project = project;
  quickAdd.open = true;
}

/** Ask, then move the task to the trash. True when deleted. */
export async function confirmDeleteTask(t: TaskEntry) {
  const ok = await ask(`Move ${t.id} "${t.title}" to .workly/trash/?`, { title: "Delete task", kind: "warning", okLabel: "Delete" });
  if (ok) await deleteTask(t.id);
  return ok;
}

/** Typing in a field: single keys and ⌘⌫ belong to the field, not to shortcuts. */
export const isTyping = (e: KeyboardEvent) => {
  const el = e.target as HTMLElement;
  return el.isContentEditable || ["INPUT", "TEXTAREA", "SELECT"].includes(el.tagName);
};
