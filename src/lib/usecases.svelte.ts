// Use-case cockpit helpers. Every step, lane, phase and label comes from
// `.workly/process.yml`; only the spec's fixed status and type ids pick colours.
import { workspace, type Process, type ProjectEntry, type Saving, type Step, type UseCase, type UseCaseAssessment } from "$lib/stores/workspace.svelte";
import { today } from "$lib/tasks.svelte";

export type UC = ProjectEntry & { usecase: UseCase };

export const PROCESS = ".workly/process.yml";

/** View state that survives switching pages (not saved: no file needs it). */
export const ui = $state({ view: "list" as "list" | "board" | "map" | "matrix", lanes: "area" as "area" | "type" | "status", matrixAdvanced: false });

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

// ---------------------------------------------------------------- assessment

/** Value and feasibility at or above this are "high" (docs/ASSESSMENT.md, D3). */
export const ASSESS_THRESHOLD = 2.0;
export const QUADRANTS = ["Quick win", "Big bet", "Fill-in", "Drop"] as const;
export type Quadrant = (typeof QUADRANTS)[number];
export const QUADRANT_COLOR: Record<Quadrant | "K.O.", string> = {
  "Quick win": "var(--w-ok)",
  "Big bet": "var(--w-info)",
  "Fill-in": "var(--w-warn)",
  Drop: "var(--w-hold)",
  "K.O.": "var(--w-danger)",
};

/**
 * Result of one assessment against the method in process.yml. Computed, never stored.
 * value / feasibility: mean of the scored criteria per axis, null when none is scored.
 * ko: fail if any fails, else open if any is open or missing, else pass.
 * open: labels of open K.O. questions, then unscored criteria (what the pilot has to answer).
 */
export function assess(p: Process, a: UseCaseAssessment) {
  const m = p.assessment!;
  const mean = (axis: string) => {
    const pts = m.criteria.filter((c) => c.axis === axis && a.scores[c.id] != null).map((c) => a.scores[c.id]);
    return pts.length ? pts.reduce((x, y) => x + y, 0) / pts.length : null;
  };
  const [value, feasibility] = [mean("value"), mean("feasibility")];
  const quadrant: Quadrant | null =
    value === null || feasibility === null
      ? null
      : value >= ASSESS_THRESHOLD
        ? feasibility >= ASSESS_THRESHOLD ? "Quick win" : "Big bet"
        : feasibility >= ASSESS_THRESHOLD ? "Fill-in" : "Drop";
  const koOf = (id: string) => a.ko[id] ?? "open";
  const failed = m.ko.filter((k) => koOf(k.id) === "fail").map((k) => k.label);
  const open = [...m.ko.filter((k) => koOf(k.id) === "open").map((k) => k.label), ...m.criteria.filter((c) => a.scores[c.id] == null).map((c) => c.label)];
  const ko = failed.length ? "fail" : m.ko.some((k) => koOf(k.id) === "open") ? "open" : "pass";
  return { value, feasibility, quadrant, ko, failed, open };
}
export type AssessResult = ReturnType<typeof assess>;

/** What the pill says: K.O. beats the quadrant; scored on one axis only = "Incomplete". */
export const verdict = (r: AssessResult) => (r.ko === "fail" ? "K.O." : (r.quadrant ?? "Incomplete"));
/** List sort: Quick win, Big bet, Fill-in, Drop, K.O., incomplete; null = not assessed (sorts last). */
export function assessRank(p: Process, u: UC) {
  if (!p.assessment || !u.usecase.assessment) return null;
  const v = verdict(assess(p, u.usecase.assessment));
  return v === "K.O." ? 5 : v === "Incomplete" ? 6 : QUADRANTS.indexOf(v) + 1;
}
const scoreFmt = new Intl.NumberFormat(undefined, { minimumFractionDigits: 1, maximumFractionDigits: 1 });
export const fmtScore = (n: number | null) => (n === null ? "–" : scoreFmt.format(n));
export const scoreLine = (r: AssessResult) => `Value ${fmtScore(r.value)} · Feasibility ${fmtScore(r.feasibility)}`;
export const assessTip = (r: AssessResult) =>
  [verdict(r), scoreLine(r), ...r.failed.map((x) => `K.O.: ${x}`), ...(r.open.length ? ["Open for pilot:", ...r.open.map((x) => `· ${x}`)] : [])].join("\n");

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
