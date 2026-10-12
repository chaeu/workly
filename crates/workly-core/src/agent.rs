//! The agent side of tasks: the start and review transitions, and the context
//! an agent works with (global rules, project instructions, skills).

use crate::ids::split_id;
use crate::model::parse_file;
use crate::project::AGENTS_TEMPLATE;
use crate::scan::rel;
use crate::write::{LogEntry, atomic_write, log};
use crate::{Error, Result, Workspace};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::BTreeMap;
use std::fs;
use std::io::Write;
use std::path::PathBuf;

/// Workspace-wide agent rules.
pub const GLOBAL_AGENTS_MD: &str = ".workly/agent/AGENTS.md";
/// `start` is allowed from these statuses.
pub const STARTABLE: [&str; 3] = ["backlog", "todo", "review"];

/// A Markdown file and its content. `path` is relative to the workspace.
#[derive(Debug, Clone, Serialize)]
pub struct AgentFile {
    pub path: String,
    pub content: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct Skill {
    pub name: String,
    pub description: String,
    /// `<project>/agent/skills/<name>/SKILL.md`, relative to the workspace.
    pub path: String,
}

/// What an agent reads before it works on a task.
#[derive(Debug, Clone, Serialize)]
pub struct AgentContext {
    pub global: Option<AgentFile>,
    pub project: Option<AgentFile>,
    pub skills: Vec<Skill>,
}

/// One line of `.workly/runs/<ID>/log.jsonl`. `run` is `agent.since` of the
/// run the note belongs to, `null` when no agent was active.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RunEntry {
    pub ts: String,
    pub agent: String,
    pub run: Option<String>,
    pub text: String,
}

#[derive(Deserialize)]
struct SkillMeta {
    name: Option<String>,
    description: Option<String>,
}

/// Agent names end up in the badge, the log and `## Updates` lines (`· codex: ...`).
pub fn check_name(name: &str) -> Result<()> {
    let ok = (1..=32).contains(&name.len()) && name.chars().all(|c| c.is_ascii_alphanumeric() || "-_.".contains(c));
    if !ok {
        return Err(Error::Invalid(format!("agent name '{name}': 1-32 letters, digits, '-', '_' or '.'")));
    }
    Ok(())
}

fn now() -> Value {
    Value::from(chrono::Utc::now().format("%Y-%m-%dT%H:%M:%SZ").to_string())
}

impl Workspace {
    /// Global rules, plus the project's AGENTS.md and skills when `project` (a key) is set.
    pub fn agent_context(&self, project: Option<&str>) -> Result<AgentContext> {
        let file = |path: String| fs::read_to_string(self.root.join(&path)).ok().map(|content| AgentFile { path, content });
        let mut ctx = AgentContext { global: file(GLOBAL_AGENTS_MD.into()), project: None, skills: Vec::new() };
        let Some(key) = project else { return Ok(ctx) };
        let dir = self.project_dir(key)?;
        ctx.project = file(rel(&self.root, &dir.join("agent/AGENTS.md")));
        let mut dirs: Vec<_> = fs::read_dir(dir.join("agent/skills")).into_iter().flatten().flatten().map(|e| e.path()).collect();
        dirs.sort();
        for d in dirs {
            let skill_file = d.join("SKILL.md");
            let Ok(src) = fs::read_to_string(&skill_file) else { continue };
            let path = rel(&self.root, &skill_file);
            // A broken header still lists the skill under its folder name.
            let meta = parse_file::<SkillMeta>(&path, &src).ok();
            let folder = d.file_name().unwrap_or_default().to_string_lossy().into_owned();
            ctx.skills.push(Skill {
                name: meta.as_ref().and_then(|m| m.name.clone()).unwrap_or(folder),
                description: meta.and_then(|m| m.description).unwrap_or_default(),
                path,
            });
        }
        Ok(ctx)
    }

    /// `<project>/agent/AGENTS.md` from the template. Fails if it exists.
    pub fn create_agents_md(&mut self, key: &str, actor: &str) -> Result<String> {
        let dir = self.project_dir(key)?;
        let file = dir.join("agent/AGENTS.md");
        let path = rel(&self.root, &file);
        if file.exists() {
            return Err(Error::Invalid(format!("{path} already exists")));
        }
        let title = &self.index.project(key).unwrap().project.title;
        fs::create_dir_all(dir.join("agent"))?;
        atomic_write(&file, &AGENTS_TEMPLATE.replace("{{title}}", title), &self.own)?;
        log(&self.root, &LogEntry::new(actor, "project.agents_md", key, None, Value::Null, path.clone().into()))?;
        self.rescan();
        Ok(path)
    }

    /// Status `doing` from backlog, todo or review. With an agent name, also
    /// `agent.active` and `agent.since` (now). One write.
    pub fn start_task(&mut self, id: &str, agent: Option<&str>, actor: &str) -> Result<()> {
        let status = self.status_of(id)?;
        if !STARTABLE.contains(&status.as_str()) {
            return Err(Error::InvalidTransition(format!("{id} is {status}; start works from {}", STARTABLE.join(", "))));
        }
        let mut changes = vec![("status", Value::from("doing"))];
        if let Some(name) = agent {
            check_name(name)?;
            changes.push(("agent.active", Value::from(name)));
            changes.push(("agent.since", now()));
        }
        self.set_task_fields(id, &changes, actor)
    }

    /// Status `review` from doing. Clears `agent.active`/`agent.since` and stamps
    /// `agent.last_run` when an agent was working; `commit` goes to `agent.commit`.
    /// `summary` becomes one item under `## Updates` (first line after the
    /// stamp, further lines as continuation lines), in the same write.
    pub fn review_task(&mut self, id: &str, commit: Option<&str>, summary: Option<&str>, actor: &str) -> Result<()> {
        let status = self.status_of(id)?;
        if status != "doing" {
            return Err(Error::InvalidTransition(format!("{id} is {status}; review works from doing")));
        }
        let mut changes = vec![("status", Value::from("review"))];
        if let Some(sha) = commit {
            if !(4..=40).contains(&sha.len()) || !sha.chars().all(|c| c.is_ascii_hexdigit()) {
                return Err(Error::Invalid(format!("commit '{sha}' is not a git hash")));
            }
            changes.push(("agent.commit", Value::from(sha.to_ascii_lowercase())));
        }
        let update = match summary {
            Some(text) => {
                let lines: Vec<&str> = text.lines().map(str::trim).filter(|l| !l.is_empty()).collect();
                if lines.is_empty() {
                    return Err(Error::Invalid("a summary needs text".into()));
                }
                let stamp = chrono::Local::now().format("%Y-%m-%d %H:%M");
                Some(format!("{stamp} · {}: {}", crate::who(actor), lines.join("\n")))
            }
            None => None,
        };
        if self.index.task(id).and_then(|t| t.task.agent.as_ref()).is_some_and(|a| a.active.is_some()) {
            changes.push(("agent.active", Value::Null));
            changes.push(("agent.since", Value::Null));
            changes.push(("agent.last_run", now()));
        }
        self.set_task_fields_with_update(id, &changes, update.as_deref(), actor)
    }

    /// Append a note to the task's run log; the task file stays untouched.
    pub fn add_run_note(&self, id: &str, text: &str, actor: &str) -> Result<RunEntry> {
        let text = text.split_whitespace().collect::<Vec<_>>().join(" ");
        if text.is_empty() {
            return Err(Error::Invalid("a note needs text".into()));
        }
        let file = self.run_log_path(id)?;
        let entry = RunEntry { ts: now().as_str().unwrap().into(), agent: crate::who(actor).into(), run: self.current_run(id), text };
        fs::create_dir_all(file.parent().unwrap())?;
        let line = serde_json::to_string(&entry).map_err(std::io::Error::other)? + "\n";
        fs::OpenOptions::new().create(true).append(true).open(&file)?.write_all(line.as_bytes())?;
        Ok(entry)
    }

    /// The task's run log, oldest first. Missing file = empty; broken lines are skipped.
    pub fn run_log(&self, id: &str) -> Result<Vec<RunEntry>> {
        let src = fs::read_to_string(self.run_log_path(id)?).unwrap_or_default();
        Ok(src.lines().filter_map(|l| serde_json::from_str(l).ok()).collect())
    }

    /// While an agent is active: the newest run-log entry of the current run,
    /// else `agent.since`. `None` when no agent is active.
    pub fn last_activity(&self, id: &str) -> Result<Option<String>> {
        let Some(run) = self.current_run(id) else { return Ok(None) };
        let last = self.run_log(id)?.into_iter().rev().find(|e| e.run.as_ref() == Some(&run));
        Ok(Some(last.map_or(run, |e| e.ts)))
    }

    /// Last activity of every task with an active agent, by id.
    pub fn agent_activity(&self) -> BTreeMap<String, String> {
        (self.index.tasks.iter())
            .filter_map(|t| Some((t.task.id.clone(), self.last_activity(&t.task.id).ok()??)))
            .collect()
    }

    /// Clear `agent.active` and `agent.since`, e.g. after an agent stopped
    /// without `review`. The status stays. One write, logged.
    pub fn clear_agent(&mut self, id: &str, actor: &str) -> Result<()> {
        self.status_of(id)?;
        self.set_task_fields(id, &[("agent.active", Value::Null), ("agent.since", Value::Null)], actor)
    }

    /// `agent.since` while `agent.active` is set.
    fn current_run(&self, id: &str) -> Option<String> {
        let agent = self.index.task(id)?.task.agent.as_ref()?;
        agent.active.as_ref().and(agent.since.clone())
    }

    fn run_log_path(&self, id: &str) -> Result<PathBuf> {
        self.status_of(id)?;
        // The id comes from frontmatter: only a plain `KEY-N` may become a path segment.
        if split_id(id).is_none_or(|(key, n)| format!("{key}-{n}") != id) {
            return Err(Error::Invalid(format!("task id '{id}' cannot have a run log")));
        }
        Ok(self.root.join(".workly/runs").join(id).join("log.jsonl"))
    }

    fn status_of(&self, id: &str) -> Result<String> {
        Ok(self.index.task(id).ok_or_else(|| Error::NotFound(format!("task {id} not found")))?.task.status.clone())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn names() {
        for ok in ["codex", "claude-code", "gpt_5.1"] {
            assert!(check_name(ok).is_ok(), "{ok}");
        }
        for bad in ["", "a b", "x: y", "a\nb", &"x".repeat(33)] {
            assert!(check_name(bad).is_err(), "{bad}");
        }
    }
}
