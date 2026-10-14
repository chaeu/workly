//! The Agents view's timeline: run-log notes plus the agent transitions from
//! `.workly/log` (start, review, commit), newest first. Read only.

use crate::{Workspace, who};
use serde::Serialize;
use serde_json::Value;
use std::fs;

/// Events older than this are left out.
const FEED_DAYS: i64 = 30;
// ponytail: fixed cap, no paging; a "load older" call if 30 days ever hold more.
const FEED_MAX: usize = 300;

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct FeedEvent {
    pub ts: String,
    /// start | note | blocked | review | commit
    pub kind: &'static str,
    /// Agent name, `me` for the app.
    pub agent: String,
    pub id: String,
    /// The note; `<sha> · N files` for a commit; empty for start and review.
    pub text: String,
}

/// A note that says the agent stopped and needs the user (prompt rule 5: `BLOCKED: <reason>`).
pub fn is_blocked(text: &str) -> bool {
    text.trim_start().starts_with("BLOCKED")
}

impl Workspace {
    /// Agent activity of the last 30 days, newest first, at most 300 events.
    pub fn agent_feed(&self) -> Vec<FeedEvent> {
        let cutoff = (chrono::Utc::now() - chrono::Duration::days(FEED_DAYS)).format("%Y-%m-%dT%H:%M:%SZ").to_string();
        let mut out = Vec::new();
        for t in &self.index.tasks {
            let id = &t.task.id;
            for e in self.run_log(id).unwrap_or_default() {
                let kind = if is_blocked(&e.text) { "blocked" } else { "note" };
                out.push(FeedEvent { ts: e.ts, kind, agent: e.agent, id: id.clone(), text: e.text });
            }
        }
        out.extend(self.log_events(&cutoff[..7]));
        out.retain(|e| e.ts >= cutoff);
        out.sort_by(|a, b| b.ts.cmp(&a.ts));
        out.truncate(FEED_MAX);
        out
    }

    /// Start, review and commit lines of the monthly logs from `month` (`YYYY-MM`) on.
    fn log_events(&self, month: &str) -> Vec<FeedEvent> {
        let mut files: Vec<_> = (fs::read_dir(self.root.join(".workly/log")).into_iter().flatten().flatten())
            .map(|e| e.path())
            .filter(|p| p.extension().is_some_and(|x| x == "jsonl") && p.file_stem().is_some_and(|s| s.to_string_lossy().as_ref() >= month))
            .collect();
        files.sort();
        let mut out = Vec::new();
        for f in files {
            for v in fs::read_to_string(f).unwrap_or_default().lines().filter_map(|l| serde_json::from_str::<Value>(l).ok()) {
                let s = |k: &str| v[k].as_str().unwrap_or_default();
                let event = |kind, agent: &str, text: String| FeedEvent { ts: s("ts").into(), kind, agent: agent.into(), id: s("id").into(), text };
                match (s("kind"), s("field")) {
                    ("task.update", "agent.active") if v["to"].is_string() => out.push(event("start", s("to"), String::new())),
                    // Moving a card to review in the app is not an agent's review.
                    ("task.update", "status") if s("to") == "review" && s("from") == "doing" && s("actor") != "app" => {
                        out.push(event("review", who(s("actor")), String::new()))
                    }
                    ("commit", _) => {
                        let sha: String = v["to"]["sha"].as_str().unwrap_or_default().chars().take(7).collect();
                        let files = v["to"]["files"].as_u64().unwrap_or(0);
                        out.push(event("commit", who(s("actor")), format!("{sha} · {files} {}", if files == 1 { "file" } else { "files" })));
                    }
                    _ => {}
                }
            }
        }
        out
    }
}
