//! Use-case history for the use-case page: per phase its time span and what happened in
//! it (status episodes, decisions). Built from `.workly/log`, so it only knows changes made
//! through Workly; hand edits leave no trace beyond the current `step` / `step_since`.

use crate::{Error, Result, Workspace};
use serde::Serialize;
use serde_json::Value;
use std::fs;

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct PhaseSpan {
    pub phase: String,
    /// First day in the phase; None when it started before the log.
    pub start: Option<String>,
    /// Day it was left; None while it is the current phase.
    pub end: Option<String>,
    pub events: Vec<Event>,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Event {
    pub date: String,
    /// "status": the use case had `status` from `date` to `until` (None = still).
    /// "decision": an entry of `usecase.decisions`.
    pub kind: &'static str,
    pub status: Option<String>,
    pub until: Option<String>,
    pub gate: Option<String>,
    /// Decision text, or the last "blocked by" of a blocked episode.
    pub text: Option<String>,
}

impl Workspace {
    pub fn usecase_history(&self, key: &str) -> Result<Vec<PhaseSpan>> {
        let p = self.process.as_ref().ok_or_else(|| Error::Invalid("no valid .workly/process.yml".into()))?;
        let uc = self.index.project(key).and_then(|e| e.project.usecase.as_ref()).ok_or_else(|| Error::NotFound(format!("use case {key} not found")))?;

        // (step, first day) in time order; a step ends where the next one starts.
        let mut steps: Vec<(String, Option<String>)> = Vec::new();
        let mut statuses: Vec<(String, String)> = Vec::new();
        let mut reasons: Vec<(String, String)> = Vec::new();
        for l in self.log_lines(key)? {
            let date = l["ts"].as_str().unwrap_or_default().chars().take(10).collect::<String>();
            let text = |v: &Value| v.as_str().map(String::from);
            match (l["kind"].as_str().unwrap_or_default(), l["field"].as_str().unwrap_or_default()) {
                ("usecase.create", _) => {
                    steps = text(&l["to"]["step"]).map(|s| (s, Some(date.clone()))).into_iter().collect();
                    statuses = text(&l["to"]["status"]).map(|s| (date.clone(), s)).into_iter().collect();
                    reasons.clear();
                }
                ("usecase.remove", _) => {
                    steps.clear();
                    statuses.clear();
                    reasons.clear();
                }
                ("usecase.move", _) => {
                    if let (true, Some(from)) = (steps.is_empty(), text(&l["from"])) {
                        steps.push((from, None));
                    }
                    if let Some(to) = text(&l["to"]) {
                        steps.push((to, Some(date)));
                    }
                }
                ("project.update", "usecase.status") => statuses.extend(text(&l["to"]).map(|s| (date, s))),
                ("project.update", "usecase.blocked_by") => reasons.extend(text(&l["to"]).map(|s| (date, s))),
                _ => {}
            }
        }
        // The file wins over the log (hand edits, use cases older than the log).
        if let Some(step) = &uc.step {
            match steps.last_mut() {
                Some((s, start)) if s == step => {
                    if start.is_none() {
                        *start = uc.step_since.clone();
                    }
                }
                _ => steps.push((step.clone(), uc.step_since.clone())),
            }
        }

        let phase_of = |step: &str| p.steps.iter().find(|s| s.id == step).and_then(|s| s.phase.clone());
        // Phase the use case was in on `date`: the last step that started on or before it.
        let phase_at = |date: &str| {
            let i = steps.iter().rposition(|(_, start)| start.as_deref().is_none_or(|s| s <= date)).unwrap_or(0);
            steps.get(i).and_then(|(s, _)| phase_of(s))
        };

        let mut spans: Vec<PhaseSpan> = Vec::new();
        for (i, (step, start)) in steps.iter().enumerate() {
            let Some(phase) = phase_of(step) else { continue };
            // The current step has no end; the others end where the next one starts.
            let end = steps.get(i + 1).and_then(|(_, s)| s.clone()).filter(|_| i + 1 < steps.len());
            match spans.iter_mut().find(|s| s.phase == phase) {
                // ponytail: a phase visited twice (rework) shows as one span from first entry to last exit.
                Some(s) => s.end = end,
                None => spans.push(PhaseSpan { phase, start: start.clone(), end, events: Vec::new() }),
            }
        }
        let span = |spans: &mut Vec<PhaseSpan>, phase: String| -> usize {
            spans.iter().position(|s| s.phase == phase).unwrap_or_else(|| {
                spans.push(PhaseSpan { phase, start: None, end: None, events: Vec::new() });
                spans.len() - 1
            })
        };

        for (j, (date, status)) in statuses.iter().enumerate() {
            let until = statuses.get(j + 1).map(|(d, _)| d.clone());
            // ponytail: the reason is the last "blocked by" written before the episode ended.
            let text = (status == "blocked")
                .then(|| reasons.iter().rev().find(|(d, _)| until.as_ref().is_none_or(|u| d <= u)).map(|(_, r)| r.clone()))
                .flatten();
            if let Some(phase) = phase_at(date) {
                let k = span(&mut spans, phase);
                spans[k].events.push(Event { date: date.clone(), kind: "status", status: Some(status.clone()), until, gate: None, text });
            }
        }
        for d in &uc.decisions {
            let Some(date) = d.date.clone() else { continue };
            // A gate decision belongs to the gate's phase; others to the phase on that day.
            let gate_phase = d.gate.as_deref().and_then(|g| p.steps.iter().find(|s| s.kind == "gate" && (s.code.as_deref() == Some(g) || s.id == g))).and_then(|s| s.phase.clone());
            if let Some(phase) = gate_phase.or_else(|| phase_at(&date)) {
                let k = span(&mut spans, phase);
                spans[k].events.push(Event { date, kind: "decision", status: None, until: None, gate: d.gate.clone(), text: Some(d.text.clone()) });
            }
        }

        let order = |id: &str| p.phases.iter().position(|ph| ph.id == id).unwrap_or(usize::MAX);
        spans.sort_by_key(|s| order(&s.phase));
        for s in &mut spans {
            s.events.sort_by(|a, b| a.date.cmp(&b.date));
        }
        Ok(spans)
    }

    /// Log lines about `key`, oldest first. Unreadable lines are skipped.
    fn log_lines(&self, key: &str) -> Result<Vec<Value>> {
        let dir = self.root.join(".workly/log");
        let Ok(entries) = fs::read_dir(&dir) else { return Ok(Vec::new()) };
        let mut files: Vec<_> = entries.flatten().map(|e| e.path()).filter(|p| p.extension().is_some_and(|x| x == "jsonl")).collect();
        files.sort();
        let mut out = Vec::new();
        for f in files {
            for line in fs::read_to_string(f)?.lines() {
                if let Ok(v) = serde_json::from_str::<Value>(line)
                    && v["id"] == key
                {
                    out.push(v);
                }
            }
        }
        Ok(out)
    }
}
