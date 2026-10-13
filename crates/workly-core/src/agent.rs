//! The agent side of tasks: the start and review transitions, and the context
//! an agent works with (global rules, project instructions, skills).

use crate::ids::split_id;
use crate::model::{AgentPreset, Config, parse_file};
use crate::patch;
use crate::project::AGENTS_TEMPLATE;
use crate::scan::rel;
use crate::settings::expand_home;
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
pub const STARTABLE: [&str; 4] = ["backlog", "todo", "waiting", "review"];
/// Runners Workly can start (B9); entries of `agents:` use one of these.
pub const AGENT_RUNNERS: [&str; 2] = ["codex", "copilot"];

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

/// One shell word in single quotes (`'` becomes `'\''`): every path written into a script goes through here.
pub fn shell_quote(s: &str) -> String {
    format!("'{}'", s.replace('\'', r"'\''"))
}

pub(crate) fn now() -> Value {
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

    /// Status `doing` from backlog, todo, waiting or review. With an agent name, also
    /// `agent.active` and `agent.since` (now); from `doing` only those. One write.
    /// One agent per repo: refused while another task of a project with the
    /// same first repo has an active agent.
    pub fn start_task(&mut self, id: &str, agent: Option<&str>, actor: &str) -> Result<()> {
        let status = self.status_of(id)?;
        let from_doing = status == "doing" && agent.is_some();
        if !STARTABLE.contains(&status.as_str()) && !from_doing {
            return Err(Error::InvalidTransition(format!("{id} is {status}; start works from {}", STARTABLE.join(", "))));
        }
        let mut changes = if from_doing { vec![] } else { vec![("status", Value::from("doing"))] };
        if let Some(name) = agent {
            check_name(name)?;
            if let Some((other, repo)) = self.agent_in_repo(id) {
                return Err(Error::InvalidTransition(format!("{other} already has an agent working in {repo}")));
            }
            changes.push(("agent.active", Value::from(name)));
            changes.push(("agent.since", now()));
        }
        self.set_task_fields(id, &changes, actor)?;
        self.left_waiting(id, &Value::from(status.as_str()), actor)?;
        // B8: the repo before the agent works. A rework (from doing or review) keeps the
        // first run's baseline, so uncommitted changes of that run stay in agent.changes.
        // ponytail: best effort; the start stands even if the baseline cannot be written.
        let _ = if from_doing || status == "review" { self.ensure_baseline(id) } else { self.write_baseline(id) };
        Ok(())
    }

    /// First repo of the task's project, if the project has one.
    pub fn task_repo(&self, id: &str) -> Option<String> {
        let folder = self.index.task(id)?.project.as_ref()?;
        let p = self.index.projects.iter().find(|p| &p.path == folder)?;
        p.project.repos.first().cloned()
    }

    /// Another task with an active agent in the same first repo: its id and the repo.
    fn agent_in_repo(&self, id: &str) -> Option<(String, String)> {
        let repo = self.task_repo(id)?;
        let other = (self.index.tasks.iter())
            .filter(|t| t.task.id != id && t.task.agent.as_ref().is_some_and(|a| a.active.is_some()))
            .find(|t| self.task_repo(&t.task.id).is_some_and(|r| expand_home(&r) == expand_home(&repo)))?;
        Some((other.task.id.clone(), repo))
    }

    /// Status `review` from doing. Clears `agent.active`/`agent.since` and stamps
    /// `agent.last_run` when an agent was working; `commit` goes to `agent.commit`.
    /// `summary` becomes one item under `## Updates` (first line after the
    /// stamp, further lines as continuation lines), in the same write.
    /// With a baseline: `agent.changes`, `agent.repo` and `agent.commit: null` (unless
    /// `commit` is given) too, and `.workly/runs/<ID>/review.json`.
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
        let lines: Option<String> = match summary {
            Some(text) => {
                let lines: Vec<&str> = text.lines().map(str::trim).filter(|l| !l.is_empty()).collect();
                if lines.is_empty() {
                    return Err(Error::Invalid("a summary needs text".into()));
                }
                Some(lines.join("\n"))
            }
            None => None,
        };
        let stamp = chrono::Local::now().format("%Y-%m-%d %H:%M");
        let update = lines.as_ref().map(|l| format!("{stamp} · {}: {l}", crate::who(actor)));
        let review = self.review_changes(id, lines.as_deref())?;
        if let Some((paths, state)) = &review {
            changes.push(("agent.changes", Value::from(paths.clone())));
            changes.push(("agent.repo", Value::from(crate::settings::tilde(std::path::Path::new(&state.repo)))));
            if commit.is_none() {
                changes.push(("agent.commit", Value::Null));
            }
        }
        if self.index.task(id).and_then(|t| t.task.agent.as_ref()).is_some_and(|a| a.active.is_some()) {
            changes.push(("agent.active", Value::Null));
            changes.push(("agent.since", Value::Null));
            changes.push(("agent.last_run", now()));
        }
        self.set_task_fields_with_update(id, &changes, update.as_deref(), actor)?;
        match review {
            Some((_, state)) => self.write_review(id, &state),
            None => Ok(()),
        }
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

    /// What starts for a task: its own agent fields when it names a runner, else the
    /// first `agents` entry. Without `agents`, missing fields come from `agent_defaults`
    /// (pre-B10 workspaces). "auto" = unset; `None` = no flag, the runner's own default.
    pub fn agent_choice(&self, id: &str) -> AgentPreset {
        let set = |v: Option<&String>| v.filter(|v| !v.is_empty() && *v != "auto").cloned();
        let agent = self.index.task(id).and_then(|t| t.task.agent.as_ref());
        let mut choice = match (agent.and_then(|a| set(a.runner.as_ref())), self.config.agents.first()) {
            (Some(runner), _) => {
                let agent = agent.unwrap();
                AgentPreset { name: runner.clone(), runner, model: set(agent.model.as_ref()), effort: set(agent.effort.as_ref()) }
            }
            (None, Some(first)) => first.clone(),
            (None, None) => AgentPreset { name: String::new(), runner: self.config.agent_defaults.runner.clone(), model: None, effort: None },
        };
        if self.config.agents.is_empty() {
            let d = &self.config.agent_defaults;
            choice.model = choice.model.or_else(|| set(Some(&d.model)));
            choice.effort = choice.effort.or_else(|| set(Some(&d.effort)));
        }
        choice
    }

    /// Pick an agent for a task: `agent.runner`, `agent.model`, `agent.effort` in one
    /// write; `None` (the default agent) or a missing model/effort writes `null`.
    pub fn set_task_agent(&mut self, id: &str, agent: Option<&AgentPreset>, actor: &str) -> Result<()> {
        self.status_of(id)?;
        let v = |f: fn(&AgentPreset) -> Option<&String>| agent.and_then(f).map_or(Value::Null, |s| Value::from(s.as_str()));
        if let Some(a) = agent
            && !AGENT_RUNNERS.contains(&a.runner.as_str())
        {
            return Err(Error::Invalid(format!("runner must be one of {}", AGENT_RUNNERS.join(", "))));
        }
        let changes = [("agent.runner", v(|a| Some(&a.runner))), ("agent.model", v(|a| a.model.as_ref())), ("agent.effort", v(|a| a.effort.as_ref()))];
        self.set_task_fields(id, &changes, actor)
    }

    /// Replace `agents:` in `.workly/config.yml` with `agents` (block list of flow maps,
    /// first = default). Lossless: every other line stays as it is. Refused, nothing
    /// written, when an entry is invalid or the file has a shape the patcher cannot edit.
    pub fn set_agents(&mut self, agents: &[AgentPreset], actor: &str) -> Result<()> {
        let clean = |v: &Option<String>| v.as_deref().map(str::trim).filter(|v| !v.is_empty()).map(String::from);
        let agents: Vec<AgentPreset> = (agents.iter())
            .map(|a| AgentPreset { name: a.name.trim().into(), runner: a.runner.clone(), model: clean(&a.model), effort: clean(&a.effort) })
            .collect();
        for (i, a) in agents.iter().enumerate() {
            if a.name.is_empty() {
                return Err(Error::Invalid("every agent needs a name".into()));
            }
            if agents[..i].iter().any(|b| b.name == a.name) {
                return Err(Error::Invalid(format!("two agents are called {}", a.name)));
            }
            if !AGENT_RUNNERS.contains(&a.runner.as_str()) {
                return Err(Error::Invalid(format!("{}: runner must be one of {}", a.name, AGENT_RUNNERS.join(", "))));
            }
        }
        let path = self.root.join(".workly/config.yml");
        let src = fs::read_to_string(&path).unwrap_or_default();
        let refuse = || Error::Invalid("agents in .workly/config.yml has a shape Workly cannot edit safely. Edit it by hand.".into());
        // The patcher works on frontmatter: wrap the file in fences, unwrap after.
        if src.lines().any(|l| l.trim_end() == "---") {
            return Err(refuse());
        }
        let eol = if src.contains("\r\n") { "\r\n" } else { "\n" };
        let body = if src.is_empty() || src.ends_with('\n') { src.clone() } else { format!("{src}{eol}") };
        let wrapped = format!("---{eol}{body}---{eol}");
        let value = serde_json::to_value(&agents).map_err(|e| Error::Invalid(e.to_string()))?;
        let patched = if agents.is_empty() { patch::set_field(&wrapped, &["agents"], &value) } else { patch::set_block_list(&wrapped, &["agents"], &value) };
        let patched = patched.map_err(|_| refuse())?;
        let p = crate::frontmatter::split(&patched);
        let out = p.frontmatter.to_string();
        if serde_yaml::from_str::<Config>(&out).map(|c| c.agents).ok().as_deref() != Some(&agents[..]) {
            return Err(refuse());
        }
        if out == src {
            return Ok(());
        }
        let from = serde_json::to_value(&self.config.agents).unwrap_or(Value::Null);
        atomic_write(&path, &out, &self.own)?;
        log(&self.root, &LogEntry::new(actor, "config.update", "", Some("agents"), from, value))?;
        self.rescan();
        Ok(())
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
        Ok(self.run_dir(id)?.join("log.jsonl"))
    }

    /// `.workly/runs/<ID>/`: run log, prompt and start script of a task.
    pub(crate) fn run_dir(&self, id: &str) -> Result<PathBuf> {
        self.status_of(id)?;
        // The id comes from frontmatter: only a plain `KEY-N` may become a path segment.
        if split_id(id).is_none_or(|(key, n)| format!("{key}-{n}") != id) {
            return Err(Error::Invalid(format!("task id '{id}' cannot have a run log")));
        }
        Ok(self.root.join(".workly/runs").join(id))
    }

    /// The first repo of the task's project as a folder on this Mac.
    pub fn repo_dir(&self, id: &str) -> Result<PathBuf> {
        let repo = self.task_repo(id).ok_or_else(|| Error::Invalid(format!("{id}: the project has no repo")))?;
        let dir = expand_home(&repo);
        if !dir.is_dir() {
            return Err(Error::Invalid(format!("Repo folder not found: {repo}")));
        }
        Ok(dir)
    }

    /// The agent prompt as `.workly/runs/<ID>/prompt.md`; agents get the path, never the text in argv.
    pub fn write_run_prompt(&self, id: &str, prompt: &str) -> Result<PathBuf> {
        let file = self.run_dir(id)?.join("prompt.md");
        fs::create_dir_all(file.parent().unwrap())?;
        atomic_write(&file, prompt, &self.own)?;
        Ok(file)
    }

    /// `prompt.md` plus `.workly/runs/<ID>/start.command` (mode 700): Terminal runs it in the
    /// user's login shell, so `codex` resolves through their own PATH, else the VS Code
    /// extension's binary. Model and effort from `agent_choice`; unset = no flag.
    pub fn write_codex_start(&self, id: &str, prompt: &str) -> Result<PathBuf> {
        use std::os::unix::fs::PermissionsExt;
        let repo = self.repo_dir(id)?;
        let prompt = self.write_run_prompt(id, prompt)?;
        let choice = self.agent_choice(id);
        let mut flags = String::new();
        if let Some(model) = &choice.model {
            flags += &format!(" -m {}", shell_quote(model));
        }
        if let Some(effort) = &choice.effort {
            flags += &format!(" -c {}", shell_quote(&format!("model_reasoning_effort={effort}")));
        }
        let q = |p: &std::path::Path| shell_quote(&p.to_string_lossy());
        let script = format!(
            r#"#!/bin/sh
# Written by Workly: Codex on {id} in the project's first repo, with prompt.md next to this file.
cd {repo} || exit 1
export WORKLY_WORKSPACE={root}
export WORKLY_AGENT=codex
# ponytail: newest extension by install time, Apple Silicon only.
codex=$(command -v codex) || codex=$(ls -td "$HOME"/.vscode/extensions/openai.chatgpt-*/bin/macos-aarch64/codex 2>/dev/null | head -1)
if [ -z "$codex" ]; then
  echo "Codex not found. Install the Codex CLI or the Codex extension for VS Code, then start the agent again."
  exit 1
fi
exec "$codex"{flags} "$(cat {prompt})"
"#,
            repo = q(&repo),
            root = q(&self.root),
            prompt = q(&prompt),
        );
        let file = self.run_dir(id)?.join("start.command");
        atomic_write(&file, &script, &self.own)?;
        fs::set_permissions(&file, fs::Permissions::from_mode(0o700))?;
        Ok(file)
    }

    fn status_of(&self, id: &str) -> Result<String> {
        Ok(self.index.task(id).ok_or_else(|| Error::NotFound(format!("task {id} not found")))?.task.status.clone())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn quoting() {
        assert_eq!(shell_quote("a b"), "'a b'");
        assert_eq!(shell_quote("it's"), r"'it'\''s'");
        assert_eq!(shell_quote(""), "''");
    }

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
