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
 * priority, due date, id. Backlog and waiting count only when nothing else is open.
 */
export function nextUp(tasks: TaskEntry[]) {
  const open = tasks.filter((t) => t.status !== "done");
  const later = (t: TaskEntry) => t.status === "backlog" || t.status === "waiting";
  const pool = open.some((t) => !later(t)) ? open.filter((t) => !later(t)) : open;
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

/**
 * True while an agent is active but has not reported for `stale_after_minutes`
 * (last activity = newest run-log entry of the run, else `agent.since`).
 */
export function agentStale(t: TaskEntry) {
  if (!t.agent?.active) return false;
  const last = Date.parse(workspace.index?.agent_activity?.[t.id] ?? t.agent.since ?? "");
  const limit = workspace.index?.config.agent_defaults?.stale_after_minutes ?? 30;
  return !isNaN(last) && clock.now - last > limit * 60_000;
}

/** "Codex working · 14 min" while `agent.active` is set, "Agent inactive?" when stale. */
export function agentBadge(t: TaskEntry) {
  const a = t.agent;
  if (!a?.active) return null;
  if (agentStale(t)) return "Agent inactive?";
  const since = a.since ? Date.parse(a.since) : NaN;
  if (isNaN(since)) return `${agentName(a.active)} working`;
  const min = Math.max(0, Math.floor((clock.now - since) / 60_000));
  const took = min < 60 ? `${min} min` : `${Math.floor(min / 60)} h ${min % 60} min`;
  return `${agentName(a.active)} working · ${took}`;
}

/** Agent name for a task: its runner, else the workspace default, else "agent". */
export function runnerOf(t: TaskEntry) {
  const pick = (r: string | null | undefined) => (r && r !== "auto" ? r : null);
  return pick(t.agent?.runner) ?? pick(workspace.index?.config.agent_defaults?.runner) ?? "agent";
}

/**
 * What you paste into Copilot Chat or Codex: the context comes from `wly task show`, so the agent goes through the CLI.
 * Carries the global rules too: existing workspaces keep their own .workly/agent/AGENTS.md.
 */
export function agentPrompt(t: TaskEntry, body = "", started = false) {
  const name = runnerOf(t);
  const id = t.id;
  const description = bodyView(body).trim();
  const acceptance = t.acceptance.map((a) => `- [${a.done ? "x" : " "}] ${a.text}`);
  return [
    `Work on Workly task ${id} ("${t.title}") in this repo. Report through wly as agent "${name}".`,
    `1. Run \`wly task show ${id}\` first. Follow the workspace rules, project instructions and skills it prints.`,
    t.status === "doing" || started
      ? `2. The task is already in progress, so skip \`wly task start\`. Change task state only through wly.`
      : `2. Start with \`wly task start ${id} --agent ${name}\`. Change task state only through wly.`,
    `3. Never run git init, never create repositories, never change remotes.`,
    `4. Never commit, push, stash or reset. Leave your changes in the working tree; the user commits from Workly.`,
    `5. No repo, or you cannot find the repo you are supposed to work in: \`wly task note ${id} "BLOCKED: <reason>" --agent ${name}\` and stop.`,
    `6. Use \`wly task note ${id} "<text>" --agent ${name}\` sparingly: only for milestones a human cares about.`,
    `7. Finish with exactly one \`wly task review ${id} --summary "<3-6 lines: what changed, how to verify, open points>" --agent ${name}\`. Never set the task to done.`,
    `8. Work only inside this repo. Delete nothing outside it.`,
    ...(description ? ["", "Task description:", description] : []),
    ...(acceptance.length ? ["", "Acceptance:", ...acceptance] : []),
  ].join("\n");
}

/** Column order: `order`, then priority, then id (`WR-9` before `WR-10`). Missing values go last. */
export const byBoardOrder = (a: TaskEntry, b: TaskEntry) =>
  (a.order ?? Infinity) - (b.order ?? Infinity) ||
  (a.priority ?? 9) - (b.priority ?? 9) ||
  a.id.localeCompare(b.id, "en", { numeric: true });

/** Owning project of a task; `undefined` for the inbox. */
export const projectOf = (t: TaskEntry): ProjectEntry | undefined =>
  t.project ? workspace.index?.projects.find((p) => p.path === t.project) : undefined;

/** The `## Updates` items of a task body; continuation lines join the item above. */
export function updatesOf(body: string) {
  const lines = body.split(/\r?\n/);
  const head = lines.findIndex(isUpdates);
  const updates: { meta: string; text: string }[] = [];
  if (head < 0) return updates;
  let end = lines.findIndex((l, i) => i > head && isSectionEnd(l));
  if (end < 0) end = lines.length;
  for (const l of lines.slice(head + 1, end)) {
    if (/^[-*] /.test(l)) {
      const item = l.slice(2);
      const colon = item.indexOf(": ");
      updates.push(colon > 0 ? { meta: item.slice(0, colon), text: item.slice(colon + 2) } : { meta: "", text: item });
    } else if (/^ {2}\S/.test(l) && updates.length) {
      // Continuation line of the item above, e.g. a multi-line review summary.
      updates[updates.length - 1].text += "\n" + l.trim();
    }
  }
  return updates;
}

// Body sections, same rules as workly-core's frontmatter.rs: a section ends at the next heading of level 1 or 2.
const isSectionEnd = (l: string) => /^#{1,2} /.test(l);
const isDescription = (l: string) => /^## Description\s*$/.test(l);
const isAcceptance = (l: string) => /^## Acceptance( criteria)?\s*$/.test(l);
const isUpdates = (l: string) => /^## Updates\s*$/.test(l);

/**
 * The editable description, raw (same rule as core's description_range): the `## Description` section,
 * or in the old layout the text before the first Acceptance or Updates heading. `sectioned` = the body has the section.
 */
export function descriptionOf(body: string) {
  const lines = body.split("\n");
  const d = lines.findIndex(isDescription);
  const end = (from: number, stop: (l: string) => boolean) => {
    const i = lines.findIndex((l, n) => n >= from && stop(l));
    return i < 0 ? lines.length : i;
  };
  if (d >= 0) return { text: lines.slice(d + 1, end(d + 1, isSectionEnd)).join("\n").trimEnd(), sectioned: true };
  return { text: lines.slice(0, end(0, (l) => isAcceptance(l) || isUpdates(l))).join("\n").trimEnd(), sectioned: false };
}

/** The body as the description view shows it: without the Description heading and without the Acceptance and Updates sections (shown on their own). */
export function bodyView(body: string) {
  let skip = false;
  return body
    .split(/\r?\n/)
    .filter((l) => {
      if (isSectionEnd(l)) skip = isAcceptance(l) || isUpdates(l);
      return !skip && !isDescription(l);
    })
    .join("\n");
}

/** Whether the body has the B3 layout; "Add sections" is offered otherwise. */
export const hasSections = (body: string) => {
  const lines = body.split(/\r?\n/);
  return lines.some(isDescription) && lines.some(isAcceptance);
};

export const RUNNERS = ["auto", "codex", "copilot", "claude-code"];
export const EFFORTS = ["auto", "low", "medium", "high"];

/** Quick add (⌘N) lives in the layout. `hint` is the project the board filter suggests; `parent` opens it as a sub-task. */
export const quickAdd = $state({ open: false, project: null as string | null, parent: null as string | null, hint: null as string | null });
export function openQuickAdd(project: string | null = quickAdd.hint, parent: string | null = null) {
  quickAdd.project = project;
  quickAdd.parent = parent;
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
