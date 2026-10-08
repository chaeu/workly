// Use-case cockpit helpers. Every step, lane, phase and label comes from
// `.workly/process.yml`; only the spec's fixed status and type ids pick colours.
import { workspace, type Process, type ProjectEntry, type Saving, type Step, type UseCase } from "$lib/stores/workspace.svelte";
import { today } from "$lib/tasks.svelte";

export type UC = ProjectEntry & { usecase: UseCase };

export const PROCESS = ".workly/process.yml";

/** View state that survives switching pages (not saved: no file needs it). */
export const ui = $state({ view: "list" as "list" | "board" | "map", lanes: "area" as "area" | "type" | "status" });

export const stepOf = (p: Process, id: string | null | undefined): Step | undefined => p.steps.find((s) => s.id === id);
export const stepName = (s: Step) => (s.kind === "gate" && s.code ? `${s.code} ${s.label}` : s.label);
export const phaseIndex = (p: Process, id: string | null | undefined) => p.phases.findIndex((x) => x.id === id);
export const phaseOf = (p: Process, u: UC) => {
  const s = stepOf(p, u.usecase.step);
  return s && s.kind !== "term" ? s.phase : null;
};

/** Area suggestions: process.yml areas first, then every other area a use case already has. Areas are free text. */
export function knownAreas(p: Process) {
  const used = (workspace.index?.projects ?? []).map((x) => x.usecase?.area?.trim()).filter((a): a is string => !!a);
  return [...new Set([...p.areas, ...used.sort((a, b) => a.localeCompare(b))])];
}

/** "No" or "No, rework": the negative branch of a decision. */
export const isNegative = (label: string | null) => /^No\b/.test(label ?? "");

// Status ids are fixed by the spec (active | waiting | blocked | on_hold | stable).
const STATUS_COLOR: Record<string, string> = {
  active: "var(--w-info)",
  waiting: "var(--w-warn)",
  blocked: "var(--w-danger)",
  on_hold: "var(--w-hold)",
  stable: "var(--w-ok)",
};
export const statusColor = (id: string | null) => STATUS_COLOR[id ?? ""] ?? "var(--w-muted)";
export const label = (list: { id: string; label: string }[], id: string | null) => list.find((x) => x.id === id)?.label ?? id ?? "–";

/** Whole days since `step_since`; null when unknown. */
export function daysInStep(u: UC) {
  const since = u.usecase.step_since?.slice(0, 10);
  if (!since) return null;
  const d = Math.round((Date.parse(`${today()}T00:00`) - Date.parse(`${since}T00:00`)) / 86_400_000);
  return isNaN(d) ? null : d;
}
/** Whole days from `a` to `b` (today when null). Dates are YYYY-MM-DD. */
export const daysBetween = (a: string, b: string | null) => Math.round((Date.parse(`${b ?? today()}T00:00`) - Date.parse(`${a}T00:00`)) / 86_400_000);
const MONTHS = ["Jan", "Feb", "Mar", "Apr", "May", "Jun", "Jul", "Aug", "Sep", "Oct", "Nov", "Dec"];
/** "24 Sep" from YYYY-MM-DD; always three letters, so date columns line up. */
export const fmtDay = (d: string) => `${d.slice(8, 10)} ${MONTHS[Number(d.slice(5, 7)) - 1] ?? "?"}`;
/** "01 – 24 Sep", "27 Aug – 10 Sep"; open end = "since 24 Sep". */
export function fmtSpan(a: string, b: string | null) {
  if (!b) return `since ${fmtDay(a)}`;
  const [x, y] = [fmtDay(a), fmtDay(b)];
  return a.slice(0, 7) === b.slice(0, 7) ? `${x.slice(0, 2)} – ${y}` : `${x} – ${y}`;
}
export const isStale = (p: Process, u: UC) => {
  const d = daysInStep(u);
  return d !== null && p.stale_after_days != null && d >= p.stale_after_days && u.usecase.status !== "stable";
};

/** Decision buttons: outgoing edges of the current step, end points left out. */
export function nextMoves(p: Process, stepId: string | null) {
  return p.edges
    .filter((e) => e.from === stepId && stepOf(p, e.to) && stepOf(p, e.to)!.kind !== "term")
    .map((e) => ({ to: e.to, verdict: e.label ? e.label.split(",")[0] : "Next", text: stepName(stepOf(p, e.to)!), neg: isNegative(e.label) }));
}

/** Toast after a move, from the process data only. */
export function moveMessage(p: Process, key: string, fromId: string | null, toId: string) {
  const from = stepOf(p, fromId);
  const to = stepOf(p, toId);
  if (!to) return `${key} moved`;
  const edge = p.edges.find((e) => e.from === fromId && e.to === toId && e.label);
  if (edge) return `${key}: ${edge.label} → ${stepName(to)}`;
  if (to.parked) return `${key} parked`;
  const [a, b] = [phaseIndex(p, from?.phase), phaseIndex(p, to.phase)];
  if (from && !from.parked && b > a) {
    const crossed = Object.entries(p.board_gates)
      .filter(([ph]) => phaseIndex(p, ph) >= a && phaseIndex(p, ph) < b)
      .sort(([x], [y]) => phaseIndex(p, x) - phaseIndex(p, y))
      .map(([, g]) => stepOf(p, g)?.code ?? g);
    if (crossed.length) return `${key} passed ${crossed.join(", ")} → ${stepName(to)}`;
  }
  if (to.kind === "gate") return `${key} waits at ${stepName(to)}`;
  return `${key} → ${stepName(to)}, owner ${p.lanes.find((l) => l.id === to.lane)?.label ?? to.lane}`;
}

// ---------------------------------------------------------------- savings

// ponytail: working weeks/days per year as constants; move them to process.yml if another organisation counts differently.
export const PER_YEAR: Record<string, number> = { year: 1, month: 12, week: 46, day: 220 };
export const fteHours = () => workspace.settings?.fte_hours_per_year || 1720;

/** Hours per year of one activity; null while a field is missing. */
export const savingHours = (s: Saving) =>
  s.count != null && s.minutes != null && s.per && PER_YEAR[s.per] ? (s.count * PER_YEAR[s.per] * s.minutes) / 60 : null;
export const savedHours = (u: UC) => u.usecase.savings.reduce((sum, s) => sum + (savingHours(s) ?? 0), 0);
/** FTE a use case saves; null when it has no savings. */
export const savedFte = (u: UC) => (u.usecase.savings.length ? savedHours(u) / fteHours() : null);

const fteFmt = new Intl.NumberFormat(undefined, { minimumFractionDigits: 1, maximumFractionDigits: 1 });
const hoursFmt = new Intl.NumberFormat(undefined, { maximumFractionDigits: 0 });
export const fmtFte = (fte: number) => fteFmt.format(fte);
export const fmtHours = (h: number) => hoursFmt.format(h);

// Tooltip texts (use:tip): first line is the heading.
/** Card or map dot: who, where it stands, how long, what comes next. */
export function ucTip(p: Process, u: UC) {
  const st = stepOf(p, u.usecase.step);
  const days = daysInStep(u);
  const status = label(p.statuses, u.usecase.status) + (u.usecase.status === "blocked" && u.usecase.blocked_by ? `: ${u.usecase.blocked_by}` : "");
  const where = st ? (st.kind === "gate" ? `waiting at ${stepName(st)}` : st.label) : null;
  return [`${u.key} ${u.title}`, [status, where, days !== null && `${days} d in step`].filter(Boolean).join(" · "), u.usecase.next_step && `→ ${u.usecase.next_step}`]
    .filter(Boolean)
    .join("\n");
}
export const stepTip = (s: Step) => [stepName(s), s.kind === "gate" ? s.hint : s.sub, s.optional && "Optional"].filter(Boolean).join("\n");
/** Manual effort: one line per activity, then the conversion. */
export const fteTip = (u: UC) =>
  [
    "Manual effort",
    ...u.usecase.savings.map((s) => {
      const h = savingHours(s);
      return `${s.what ?? "–"} · ${s.count ?? "–"}/${s.per ?? "–"} × ${s.minutes ?? "–"} min = ${h == null ? "–" : `${fmtHours(h)} h`}`;
    }),
    `${fmtHours(savedHours(u))} h per year · ${fmtFte(savedFte(u) ?? 0)} FTE at ${fmtHours(fteHours())} h`,
  ].join("\n");
