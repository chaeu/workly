//! Typed views of workspace files. Read-only: writes go through `patch`.

use serde::{Deserialize, Deserializer, Serialize};
use serde_json::Value;
use std::collections::BTreeMap;

/// Fields the app does not know. Kept so nothing is hidden, never written back.
pub type Extra = BTreeMap<String, serde_yaml::Value>;

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct Task {
    pub id: String,
    #[serde(default)]
    pub title: String,
    #[serde(default)]
    pub status: String,
    pub priority: Option<u8>,
    pub due: Option<String>,
    #[serde(default)]
    pub tags: Vec<String>,
    pub focus: Option<String>,
    pub focus_order: Option<u8>,
    /// Manual position inside a board column; missing = after the ordered ones.
    pub order: Option<i64>,
    pub created: Option<String>,
    pub done_at: Option<String>,
    pub agent: Option<AgentState>,
    #[serde(flatten)]
    pub extra: Extra,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct AgentState {
    #[serde(default)]
    pub ready: bool,
    pub runner: Option<String>,
    pub model: Option<String>,
    pub effort: Option<String>,
    pub active: Option<String>,
    pub since: Option<String>,
    pub commit: Option<String>,
    pub last_run: Option<String>,
    #[serde(flatten)]
    pub extra: Extra,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct Project {
    pub key: String,
    #[serde(default)]
    pub title: String,
    #[serde(default)]
    pub status: String,
    pub color: Option<String>,
    pub order: Option<i64>,
    #[serde(default)]
    pub repos: Vec<String>,
    #[serde(default)]
    pub links: Vec<Link>,
    pub created: Option<String>,
    pub usecase: Option<UseCase>,
    #[serde(flatten)]
    pub extra: Extra,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct Link {
    pub label: Option<String>,
    pub url: String,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct UseCase {
    #[serde(rename = "type")]
    pub kind: Option<String>,
    pub area: Option<String>,
    pub step: Option<String>,
    pub step_since: Option<String>,
    pub status: Option<String>,
    pub blocked_by: Option<String>,
    pub next_step: Option<String>,
    pub current_state: Option<String>,
    #[serde(default)]
    pub decisions: Vec<Decision>,
    /// Activities the use case saves time on. Hours and FTE are computed in the app, never stored.
    #[serde(default, deserialize_with = "lenient_savings")]
    pub savings: Vec<Saving>,
    pub savings_note: Option<String>,
    /// Current assessment (K.O. questions, 1-3 points per criterion). Value, feasibility
    /// and quadrant are computed in the app, never stored.
    #[serde(default, deserialize_with = "lenient_assessment")]
    pub assessment: Option<UseCaseAssessment>,
    #[serde(flatten)]
    pub extra: Extra,
}

pub const SAVING_PER: [&str; 4] = ["year", "month", "week", "day"];

/// One entry of `usecase.savings`. A hand-written field that is missing or
/// invalid reads as `None`, so the use case still loads; the scan reports it.
#[derive(Debug, Clone, Default, Serialize)]
pub struct Saving {
    pub what: Option<String>,
    pub count: Option<f64>,
    pub per: Option<String>,
    pub minutes: Option<f64>,
    /// Keys the app does not know, sent back unchanged when the list is rewritten.
    #[serde(flatten)]
    pub extra: serde_json::Map<String, Value>,
}

impl Saving {
    fn from_value(v: &Value) -> Saving {
        let num = |k: &str| v.get(k).and_then(Value::as_f64).filter(|x| *x >= 0.0);
        let mut extra = v.as_object().cloned().unwrap_or_default();
        extra.retain(|k, _| !["what", "count", "per", "minutes"].contains(&k.as_str()));
        Saving {
            what: v.get("what").and_then(Value::as_str).filter(|s| !s.trim().is_empty()).map(String::from),
            count: num("count"),
            per: v.get("per").and_then(Value::as_str).filter(|p| SAVING_PER.contains(p)).map(String::from),
            minutes: num("minutes"),
            extra,
        }
    }
}

/// What is wrong with a `usecase.savings` value; empty = valid. Checked on write and on scan.
pub fn saving_problems(v: &Value) -> Vec<String> {
    let items = match v {
        Value::Null => return Vec::new(),
        Value::Array(items) => items,
        _ => return vec!["usecase.savings must be a list".into()],
    };
    let mut out = Vec::new();
    for (i, item) in items.iter().enumerate() {
        let s = Saving::from_value(item);
        let n = i + 1;
        if s.what.is_none() {
            out.push(format!("usecase.savings {n}: what must be a non-empty text"));
        }
        for (field, ok) in [("count", s.count.is_some()), ("minutes", s.minutes.is_some())] {
            if !ok {
                out.push(format!("usecase.savings {n}: {field} must be a number >= 0"));
            }
        }
        if s.per.is_none() {
            out.push(format!("usecase.savings {n}: per must be one of {}", SAVING_PER.join(", ")));
        }
    }
    out
}

fn lenient_savings<'de, D: Deserializer<'de>>(d: D) -> Result<Vec<Saving>, D::Error> {
    let v = Option::<Value>::deserialize(d)?;
    Ok(v.as_ref().and_then(Value::as_array).map(|a| a.iter().map(Saving::from_value).collect()).unwrap_or_default())
}

pub const KO_VALUES: [&str; 3] = ["pass", "fail", "open"];
/// Keys of `usecase.assessment` the app writes.
pub const ASSESSMENT_KEYS: [&str; 4] = ["date", "ko", "scores", "note"];

/// `usecase.assessment`. Invalid hand-written entries are left out, so the use
/// case still loads; the scan reports them. A missing K.O. or score = open.
#[derive(Debug, Clone, Default, Serialize)]
pub struct UseCaseAssessment {
    pub date: Option<String>,
    /// K.O. question id -> pass | fail | open
    pub ko: BTreeMap<String, String>,
    /// Criterion id -> 1..3
    pub scores: BTreeMap<String, u8>,
    pub note: Option<String>,
}

fn score(v: &Value) -> Option<u8> {
    v.as_u64().filter(|n| (1..=3).contains(n)).map(|n| n as u8)
}

fn lenient_assessment<'de, D: Deserializer<'de>>(d: D) -> Result<Option<UseCaseAssessment>, D::Error> {
    let v = Option::<Value>::deserialize(d)?;
    let Some(v) = v.filter(Value::is_object) else { return Ok(None) };
    let entries = |k: &str| v.get(k).and_then(Value::as_object).into_iter().flatten();
    Ok(Some(UseCaseAssessment {
        date: v.get("date").and_then(Value::as_str).map(String::from),
        ko: entries("ko").filter_map(|(id, x)| x.as_str().filter(|x| KO_VALUES.contains(x)).map(|x| (id.clone(), x.into()))).collect(),
        scores: entries("scores").filter_map(|(id, x)| score(x).map(|n| (id.clone(), n))).collect(),
        note: v.get("note").and_then(Value::as_str).map(String::from),
    }))
}

/// What is wrong with a `usecase.assessment` value; empty = valid. Checked on write
/// and on scan. Ids are checked against `method` when there is one.
pub fn assessment_problems(v: &Value, method: Option<&Assessment>) -> Vec<String> {
    let map = match v {
        Value::Null => return Vec::new(),
        Value::Object(map) => map,
        _ => return vec!["usecase.assessment must be a map".into()],
    };
    let mut out = Vec::new();
    if let Some(d) = map.get("date").filter(|d| !d.is_null())
        && !d.as_str().is_some_and(|d| chrono::NaiveDate::parse_from_str(d, "%Y-%m-%d").is_ok())
    {
        out.push(format!("usecase.assessment: date must be a date (YYYY-MM-DD), got {d}"));
    }
    if map.get("note").is_some_and(|n| !n.is_null() && !n.is_string()) {
        out.push("usecase.assessment: note must be a text".into());
    }
    let ko_ids: Option<Vec<&str>> = method.map(|m| m.ko.iter().map(|k| k.id.as_str()).collect());
    let criteria_ids: Option<Vec<&str>> = method.map(|m| m.criteria.iter().map(|c| c.id.as_str()).collect());
    let ko_ok: fn(&Value) -> bool = |x| x.as_str().is_some_and(|x| KO_VALUES.contains(&x));
    let checks = [
        ("ko", "K.O. question", ko_ids, ko_ok, "pass, fail or open"),
        ("scores", "criterion", criteria_ids, |x| score(x).is_some(), "1, 2 or 3"),
    ];
    for (key, what, ids, ok, allowed) in checks {
        match map.get(key) {
            None | Some(Value::Null) => {}
            Some(Value::Object(entries)) => {
                for (id, x) in entries {
                    if ids.as_ref().is_some_and(|ids| !ids.contains(&id.as_str())) {
                        out.push(format!("usecase.assessment: unknown {what} '{id}' (not in process.yml)"));
                    } else if !ok(x) {
                        out.push(format!("usecase.assessment: {key}.{id} must be {allowed}, got {x}"));
                    }
                }
            }
            Some(_) => out.push(format!("usecase.assessment: {key} must be a map")),
        }
    }
    out
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct Decision {
    pub date: Option<String>,
    pub gate: Option<String>,
    pub text: String,
}

/// Parse error located in the file (1-based line), never fatal for a scan.
#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct ParseError {
    pub path: String,
    pub line: Option<usize>,
    pub message: String,
}

/// Parse a file's frontmatter into `T`. `path` is only used for the error.
pub fn parse_file<T: serde::de::DeserializeOwned>(path: &str, src: &str) -> Result<T, ParseError> {
    let parts = crate::frontmatter::split(src);
    if !parts.has_frontmatter() {
        return Err(ParseError { path: path.into(), line: Some(1), message: "missing frontmatter".into() });
    }
    // Lines before the YAML: the opening fence.
    let offset = 1;
    serde_yaml::from_str(parts.frontmatter).map_err(|e| ParseError {
        path: path.into(),
        line: e.location().map(|l| l.line() + offset),
        message: e.to_string(),
    })
}

// ---------------------------------------------------------------- config.yml

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(default)]
pub struct Config {
    pub version: u32,
    pub new_projects_dir: String,
    pub inbox_dir: String,
    pub scan_exclude: Vec<String>,
    pub task_statuses: Vec<TaskStatus>,
    pub agent_defaults: AgentDefaults,
}

impl Default for Config {
    fn default() -> Self {
        let status = |id: &str, label: &str| TaskStatus { id: id.into(), label: label.into(), wip_limit: None };
        Config {
            version: 1,
            new_projects_dir: "projects".into(),
            inbox_dir: "inbox".into(),
            scan_exclude: [".git", "node_modules", ".obsidian", ".workly", "_templates"].map(String::from).into(),
            task_statuses: vec![
                status("backlog", "Backlog"),
                status("todo", "To do"),
                status("doing", "Doing"),
                status("review", "Review"),
                status("done", "Done"),
            ],
            agent_defaults: AgentDefaults::default(),
        }
    }
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct TaskStatus {
    pub id: String,
    pub label: String,
    pub wip_limit: Option<u32>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(default)]
pub struct AgentDefaults {
    pub runner: String,
    pub model: String,
    pub effort: String,
}

impl Default for AgentDefaults {
    fn default() -> Self {
        AgentDefaults { runner: "auto".into(), model: "auto".into(), effort: "auto".into() }
    }
}

// --------------------------------------------------------------- process.yml

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct Process {
    #[serde(default)]
    pub lanes: Vec<Lane>,
    #[serde(default)]
    pub phases: Vec<Phase>,
    #[serde(default)]
    pub steps: Vec<Step>,
    #[serde(default)]
    pub phase_default_step: BTreeMap<String, String>,
    #[serde(default)]
    pub board_gates: BTreeMap<String, String>,
    #[serde(default)]
    pub edges: Vec<Edge>,
    #[serde(default)]
    pub statuses: Vec<Labelled>,
    #[serde(default)]
    pub types: Vec<Labelled>,
    #[serde(default)]
    pub areas: Vec<String>,
    pub stale_after_days: Option<u32>,
    /// Use-case assessment method; missing = no assessment in the app.
    pub assessment: Option<Assessment>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct Assessment {
    #[serde(default)]
    pub ko: Vec<Labelled>,
    #[serde(default)]
    pub criteria: Vec<Criterion>,
}

pub const AXES: [&str; 2] = ["value", "feasibility"];

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct Criterion {
    pub id: String,
    /// value | feasibility
    pub axis: String,
    pub label: String,
    /// What 1, 2 and 3 points mean.
    #[serde(default)]
    pub anchors: Vec<String>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct Lane {
    pub id: String,
    pub label: String,
    pub sub: Option<String>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct Phase {
    pub id: String,
    pub name: String,
    pub desc: Option<String>,
    #[serde(default)]
    pub optional: bool,
    #[serde(default)]
    pub parked: bool,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct Step {
    pub id: String,
    /// box | gate | term
    pub kind: String,
    pub col: u32,
    pub lane: String,
    pub phase: Option<String>,
    pub label: String,
    pub sub: Option<String>,
    pub code: Option<String>,
    pub hint: Option<String>,
    #[serde(default)]
    pub optional: bool,
    #[serde(default)]
    pub parked: bool,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct Edge {
    pub from: String,
    pub to: String,
    pub route: String,
    pub label: Option<String>,
    pub label_dx: Option<i32>,
    pub offset: Option<i32>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct Labelled {
    pub id: String,
    pub label: String,
}

impl Process {
    /// Every dangling reference as a readable message. Empty = valid.
    pub fn validate(&self) -> Vec<String> {
        let has = |list: &[&str], id: &str| list.contains(&id);
        let steps: Vec<&str> = self.steps.iter().map(|s| s.id.as_str()).collect();
        let phases: Vec<&str> = self.phases.iter().map(|p| p.id.as_str()).collect();
        let lanes: Vec<&str> = self.lanes.iter().map(|l| l.id.as_str()).collect();
        let mut errors = Vec::new();
        for s in &self.steps {
            if !has(&lanes, &s.lane) {
                errors.push(format!("step '{}' uses unknown lane '{}'", s.id, s.lane));
            }
            if let Some(p) = s.phase.as_deref().filter(|p| !has(&phases, p)) {
                errors.push(format!("step '{}' uses unknown phase '{p}'", s.id));
            }
            if !["box", "gate", "term"].contains(&s.kind.as_str()) {
                errors.push(format!("step '{}' has unknown kind '{}' (box | gate | term)", s.id, s.kind));
            }
        }
        for (i, e) in self.edges.iter().enumerate() {
            if !["h", "hv", "hvh", "vu", "over"].contains(&e.route.as_str()) {
                errors.push(format!("edge {} ({} -> {}) has unknown route '{}' (h | hv | hvh | vu | over)", i + 1, e.from, e.to, e.route));
            }
            for end in [&e.from, &e.to] {
                if !has(&steps, end) {
                    errors.push(format!("edge {} ({} -> {}) references unknown step '{end}'", i + 1, e.from, e.to));
                }
            }
        }
        for (name, map) in [("phase_default_step", &self.phase_default_step), ("board_gates", &self.board_gates)] {
            for (phase, step) in map {
                if !has(&phases, phase) {
                    errors.push(format!("{name}: unknown phase '{phase}'"));
                }
                if !has(&steps, step) {
                    errors.push(format!("{name}: phase '{phase}' points to unknown step '{step}'"));
                }
            }
        }
        if let Some(a) = &self.assessment {
            let mut seen = Vec::new();
            for id in a.ko.iter().map(|k| &k.id).chain(a.criteria.iter().map(|c| &c.id)) {
                if seen.contains(&id) {
                    errors.push(format!("assessment: id '{id}' is used twice"));
                }
                seen.push(id);
            }
            for c in &a.criteria {
                if !AXES.contains(&c.axis.as_str()) {
                    errors.push(format!("assessment: criterion '{}' has unknown axis '{}' (value | feasibility)", c.id, c.axis));
                }
                if c.anchors.len() != 3 {
                    errors.push(format!("assessment: criterion '{}' needs exactly 3 anchors, has {}", c.id, c.anchors.len()));
                }
            }
            for axis in AXES {
                if !a.criteria.iter().any(|c| c.axis == axis) {
                    errors.push(format!("assessment: no criterion on the {axis} axis"));
                }
            }
        }
        errors
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn task_with_unknown_fields() {
        let src = "---\nid: A-1\ntitle: T\npriority: 2\nestimate: 3h\nagent:\n  ready: true\n  x: 1\n---\n";
        let t: Task = parse_file("a.md", src).unwrap();
        assert_eq!(t.priority, Some(2));
        assert!(t.extra.contains_key("estimate"));
        let agent = t.agent.unwrap();
        assert!(agent.ready && agent.extra.contains_key("x"));
    }

    #[test]
    fn dates_stay_strings() {
        let t: Task = parse_file("a.md", "---\nid: A-1\ndue: 2026-10-10\n---\n").unwrap();
        assert_eq!(t.due.as_deref(), Some("2026-10-10"));
    }

    #[test]
    fn invalid_yaml_has_file_line() {
        let src = "---\nid: IN-3\ntitle: \"Unclosed quote\nstatus: todo\n---\n";
        let e = parse_file::<Task>("inbox/IN-3.md", src).unwrap_err();
        assert_eq!(e.path, "inbox/IN-3.md");
        assert!(matches!(e.line, Some(3..=5)), "{e:?}");
    }

    #[test]
    fn process_validation_messages() {
        let p: Process = serde_yaml::from_str(
            "lanes: [{id: a, label: A}]\nphases: [{id: p, name: P}]\n\
             steps: [{id: s, kind: box, col: 0, lane: a, phase: p, label: S}]\n\
             edges: [{from: s, to: nope, route: zz}]\nphase_default_step: {p: gone, q: s}\n",
        )
        .unwrap();
        assert_eq!(
            p.validate(),
            [
                "edge 1 (s -> nope) has unknown route 'zz' (h | hv | hvh | vu | over)",
                "edge 1 (s -> nope) references unknown step 'nope'",
                "phase_default_step: phase 'p' points to unknown step 'gone'",
                "phase_default_step: unknown phase 'q'",
            ]
        );
        let p: Process = serde_yaml::from_str(
            "assessment:\n  ko: [{id: a, label: A}]\n  criteria:\n    - {id: a, axis: value, label: X, anchors: [1, 2, 3]}\n    - {id: b, axis: cost, label: Y, anchors: [1, 2]}\n",
        )
        .unwrap();
        assert_eq!(
            p.validate(),
            [
                "assessment: id 'a' is used twice",
                "assessment: criterion 'b' has unknown axis 'cost' (value | feasibility)",
                "assessment: criterion 'b' needs exactly 3 anchors, has 2",
                "assessment: no criterion on the feasibility axis",
            ]
        );
    }
}
