// Use-case cockpit helpers. Every step, lane, phase and label comes from
// `.workly/process.yml`; only the spec's fixed status and type ids pick colours.
import type { Process, ProjectEntry, Step, UseCase } from "$lib/stores/workspace.svelte";
import { today } from "$lib/tasks.svelte";

export type UC = ProjectEntry & { usecase: UseCase };

/** View state that survives switching pages (not saved: no file needs it). */
export const ui = $state({ view: "list" as "list" | "board" | "map", lanes: "area" as "area" | "type" | "status" });

export const stepOf = (p: Process, id: string | null | undefined): Step | undefined => p.steps.find((s) => s.id === id);
export const stepName = (s: Step) => (s.kind === "gate" && s.code ? `${s.code} ${s.label}` : s.label);
export const phaseIndex = (p: Process, id: string | null | undefined) => p.phases.findIndex((x) => x.id === id);
export const phaseOf = (p: Process, u: UC) => {
  const s = stepOf(p, u.usecase.step);
  return s && s.kind !== "term" ? s.phase : null;
};

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
  return `${key} → ${stepName(to)}, ball with ${p.lanes.find((l) => l.id === to.lane)?.label ?? to.lane}`;
}
