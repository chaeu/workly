//! An agent's changes in the code repo (B8). Start writes a baseline of the repo's
//! dirty files, review records what changed since, and an explicit click commits
//! exactly those paths. Never pushes, never commits outside `agent.changes`.
//! Git through the backup's helper: args array, no shell, never interactive.

use crate::backup::{git, git_timeout};
use crate::settings::tilde;
use crate::write::{LogEntry, atomic_write, log};
use crate::{Error, Result, Workspace};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::collections::BTreeMap;
use std::fs;
use std::path::{Component, Path, PathBuf};
use std::time::Duration;

const BASELINE: &str = "baseline.json";
const REVIEW: &str = "review.json";
/// A commit may run the repo's pre-commit hook (lint, tests).
const COMMIT_TIMEOUT: Duration = Duration::from_secs(120);
/// Diff lines shown per file.
const DIFF_LINES: usize = 400;

/// The repo's dirty files at one moment: `.workly/runs/<ID>/baseline.json` at start,
/// `review.json` (only the changed paths) at review. A hash is `git hash-object` of
/// the file, `null` = gone.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct RepoState {
    pub ts: String,
    /// Repo toplevel on this Mac, absolute.
    pub repo: String,
    /// `null` before the first commit.
    pub head: Option<String>,
    pub files: BTreeMap<String, Option<String>>,
    /// review.json: the review summary, body of the commit message.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub summary: Option<String>,
}

/// One path of `agent.changes` as it is now.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct FileChange {
    pub path: String,
    /// new | modified | deleted | clean (committed or reverted since)
    pub kind: &'static str,
    /// Other content than at review: the user edited it afterwards. Commit asks first.
    pub drifted: bool,
}

/// A checked commit, run by `CommitPlan::run` without the workspace: a hook may take long.
#[derive(Debug, Clone)]
pub struct CommitPlan {
    pub id: String,
    pub repo: PathBuf,
    /// Paths of `agent.changes` that differ from HEAD.
    pub paths: Vec<String>,
    message: PathBuf,
}

type Git<T> = std::result::Result<T, String>;

/// A path from `agent.changes` before it reaches git: relative, inside the repo, no `..`.
pub fn check_path(p: &str) -> Result<()> {
    if !p.is_empty() && !p.contains('\0') && Path::new(p).components().all(|c| matches!(c, Component::Normal(_))) {
        Ok(())
    } else {
        Err(Error::Invalid(format!("'{p}' is not a path inside the repo")))
    }
}

/// Toplevel of the repo `dir` is in; None when it is none (or git is missing).
fn toplevel(dir: &Path) -> Option<PathBuf> {
    git(dir, &["rev-parse", "--show-toplevel"]).ok().map(PathBuf::from)
}

/// Dirty paths as (`git status` code, path): untracked files one by one, a rename as delete + add.
fn status(repo: &Path) -> Git<Vec<(String, String)>> {
    let out = git(repo, &["status", "--porcelain", "-z", "--untracked-files=all", "--no-renames"])?;
    Ok(out.split('\0').filter(|e| e.len() > 3).map(|e| (e[..2].to_string(), e[3..].to_string())).collect())
}

/// `git hash-object` per path; `None` for a path that is gone.
// ponytail: every path in one argv; --stdin-paths if repos get thousands of dirty files. Submodules hash as None.
fn hashes(repo: &Path, paths: &[String]) -> Git<BTreeMap<String, Option<String>>> {
    let files: Vec<&str> = paths.iter().filter(|p| repo.join(p).is_file()).map(String::as_str).collect();
    let mut out: BTreeMap<String, Option<String>> = paths.iter().map(|p| (p.clone(), None)).collect();
    if !files.is_empty() {
        let hashed = git(repo, &[&["hash-object", "--"][..], &files].concat())?;
        for (p, h) in files.iter().zip(hashed.lines()) {
            out.insert(p.to_string(), Some(h.to_string()));
        }
    }
    Ok(out)
}

/// The repo's dirty files now.
fn snapshot(repo: &Path) -> Git<RepoState> {
    let paths: Vec<String> = status(repo)?.into_iter().map(|(_, p)| p).collect();
    Ok(RepoState {
        ts: crate::agent::now().as_str().unwrap_or_default().into(),
        repo: repo.to_string_lossy().into(),
        head: git(repo, &["rev-parse", "-q", "--verify", "HEAD"]).ok(),
        files: hashes(repo, &paths)?,
        summary: None,
    })
}

/// Dirty now and either clean at the baseline or with other content than there.
fn changed(base: &RepoState, now: &RepoState) -> Vec<String> {
    now.files.iter().filter(|(p, h)| base.files.get(*p) != Some(*h)).map(|(p, _)| p.clone()).collect()
}

/// At most `DIFF_LINES` lines.
fn truncate(text: String) -> String {
    let n = text.lines().count();
    if n <= DIFF_LINES {
        return text;
    }
    let head: Vec<&str> = text.lines().take(DIFF_LINES).collect();
    format!("{}\n… {} more lines", head.join("\n"), n - DIFF_LINES)
}

impl CommitPlan {
    /// `git add -- <paths>`, `git commit -F <message file>`: (short sha, full sha).
    /// The repo's hooks run; nothing is pushed.
    pub fn run(&self) -> Result<(String, String)> {
        let add = [&["add", "--"][..], &self.paths.iter().map(String::as_str).collect::<Vec<_>>()].concat();
        git(&self.repo, &add).map_err(Error::Invalid)?;
        let message = self.message.to_string_lossy();
        git_timeout(&self.repo, &["commit", "-q", "-F", &message], COMMIT_TIMEOUT)
            .map_err(|e| Error::Invalid(format!("{e}\nThe files of {} are staged now.", self.id)))?;
        let sha = |short: bool| git(&self.repo, &if short { vec!["rev-parse", "--short", "HEAD"] } else { vec!["rev-parse", "HEAD"] });
        Ok((sha(true).map_err(Error::Invalid)?, sha(false).map_err(Error::Invalid)?))
    }
}

impl Workspace {
    fn read_state(&self, id: &str, name: &str) -> Result<Option<RepoState>> {
        match fs::read_to_string(self.run_dir(id)?.join(name)) {
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(None),
            r => serde_json::from_str(&r?).map(Some).map_err(|e| Error::Invalid(format!(".workly/runs/{id}/{name}: {e}"))),
        }
    }

    fn write_state(&self, id: &str, name: &str, state: &RepoState) -> Result<()> {
        let file = self.run_dir(id)?.join(name);
        fs::create_dir_all(file.parent().unwrap())?;
        let json = serde_json::to_string_pretty(state).map_err(std::io::Error::other)? + "\n";
        Ok(atomic_write(&file, &json, &self.own)?)
    }

    /// Snapshot of the task's repo (first repo of its project) as its baseline.
    /// No repo, no git repo or git failing: no baseline, `false`.
    pub fn write_baseline(&self, id: &str) -> Result<bool> {
        let Some(repo) = self.repo_dir(id).ok().and_then(|d| toplevel(&d)) else { return Ok(false) };
        let Ok(state) = snapshot(&repo) else { return Ok(false) };
        self.write_state(id, BASELINE, &state)?;
        Ok(true)
    }

    /// A baseline unless the task has one: Copy prompt on a task in doing, a rework from review.
    pub fn ensure_baseline(&self, id: &str) -> Result<bool> {
        if self.run_dir(id)?.join(BASELINE).exists() {
            return Ok(true);
        }
        self.write_baseline(id)
    }

    /// At review: the changed paths since the baseline and the review record (their
    /// hashes now plus the summary). None without a baseline or when git fails.
    pub(crate) fn review_changes(&self, id: &str, summary: Option<&str>) -> Result<Option<(Vec<String>, RepoState)>> {
        let Some(base) = self.read_state(id, BASELINE).ok().flatten() else { return Ok(None) };
        let Ok(mut now) = snapshot(Path::new(&base.repo)) else { return Ok(None) };
        let paths = changed(&base, &now);
        now.files.retain(|p, _| paths.contains(p));
        now.summary = summary.map(String::from);
        Ok(Some((paths, now)))
    }

    /// Store the review record once the review is written.
    pub(crate) fn write_review(&self, id: &str, review: &RepoState) -> Result<()> {
        self.write_state(id, REVIEW, review)
    }

    /// `agent.changes` (each path checked) and the review record.
    fn review_of(&self, id: &str) -> Result<(Vec<String>, RepoState)> {
        let t = self.index.task(id).ok_or_else(|| Error::NotFound(format!("task {id} not found")))?;
        let paths = t.task.agent.as_ref().and_then(|a| a.changes.clone()).unwrap_or_default();
        paths.iter().try_for_each(|p| check_path(p))?;
        let review = (self.read_state(id, REVIEW)?)
            .ok_or_else(|| Error::Invalid(format!("{id} has no review record (.workly/runs/{id}/review.json) on this Mac")))?;
        if !Path::new(&review.repo).is_dir() {
            return Err(Error::Invalid(format!("Repo folder not found: {}", tilde(Path::new(&review.repo)))));
        }
        Ok((paths, review))
    }

    /// `agent.changes` as they are now, for the detail card.
    pub fn task_changes(&self, id: &str) -> Result<Vec<FileChange>> {
        let (paths, review) = self.review_of(id)?;
        let repo = Path::new(&review.repo);
        let codes: BTreeMap<String, String> = status(repo).map_err(Error::Invalid)?.into_iter().map(|(c, p)| (p, c)).collect();
        let now = hashes(repo, &paths).map_err(Error::Invalid)?;
        Ok(paths
            .iter()
            .map(|p| {
                let kind = match codes.get(p).map(String::as_str) {
                    None => "clean",
                    Some(c) if c.contains('D') => "deleted",
                    Some(c) if c == "??" || c.starts_with('A') => "new",
                    Some(_) => "modified",
                };
                FileChange { path: p.clone(), kind, drifted: kind != "clean" && review.files.get(p) != now.get(p) }
            })
            .collect())
    }

    /// Read-only diff of one changed path against HEAD; a new file as its content.
    /// At most `DIFF_LINES` lines.
    pub fn change_diff(&self, id: &str, path: &str) -> Result<String> {
        let (paths, review) = self.review_of(id)?;
        if !paths.iter().any(|p| p == path) {
            return Err(Error::Invalid(format!("{path} is not in the changes of {id}")));
        }
        let repo = Path::new(&review.repo);
        let diff = git(repo, &["diff", "--no-color", "--no-ext-diff", "HEAD", "--", path]).unwrap_or_default();
        if !diff.is_empty() {
            return Ok(truncate(diff));
        }
        Ok(match fs::read(repo.join(path)) {
            Ok(b) if b.contains(&0) => "Binary file".into(),
            Ok(b) => truncate(String::from_utf8_lossy(&b).lines().map(|l| format!("+{l}")).collect::<Vec<_>>().join("\n")),
            Err(_) => String::new(),
        })
    }

    /// Check a commit of the task's changes: status review, every path inside the
    /// repo, no staged changes outside the task, nothing edited since the review
    /// unless `accept_drift`. Writes the message file `<ID>: <title>` + summary.
    pub fn prepare_commit(&self, id: &str, accept_drift: bool) -> Result<CommitPlan> {
        let t = &self.index.task(id).ok_or_else(|| Error::NotFound(format!("task {id} not found")))?.task;
        if t.status != "review" {
            return Err(Error::InvalidTransition(format!("{id} is {}; commit works from review", t.status)));
        }
        let (paths, review) = self.review_of(id)?;
        if paths.is_empty() {
            return Err(Error::Invalid(format!("{id} has no changes to commit")));
        }
        let repo = PathBuf::from(&review.repo);
        let staged = git(&repo, &["diff", "--cached", "--name-only", "-z"]).map_err(Error::Invalid)?;
        let foreign: Vec<&str> = staged.split('\0').filter(|p| !p.is_empty() && !paths.iter().any(|x| x == p)).collect();
        if !foreign.is_empty() {
            return Err(Error::Conflict(format!(
                "The repo already has staged changes that are not part of {id}: {}. Commit or unstage them first; Workly commits only the task's files.",
                foreign.join(", ")
            )));
        }
        let files = self.task_changes(id)?;
        let drifted: Vec<&str> = files.iter().filter(|f| f.drifted).map(|f| f.path.as_str()).collect();
        if !drifted.is_empty() && !accept_drift {
            return Err(Error::Conflict(format!("Changed after the review: {}. Check the diff, then commit again.", drifted.join(", "))));
        }
        let paths: Vec<String> = files.into_iter().filter(|f| f.kind != "clean").map(|f| f.path).collect();
        if paths.is_empty() {
            return Err(Error::Invalid(format!("Nothing to commit: the files of {id} match the last commit")));
        }
        let mut message = format!("{id}: {}\n", t.title);
        if let Some(summary) = review.summary.as_deref().filter(|s| !s.trim().is_empty()) {
            message += &format!("\n{summary}\n");
        }
        let file = self.run_dir(id)?.join("commit-msg.txt");
        atomic_write(&file, &message, &self.own)?;
        Ok(CommitPlan { id: id.into(), repo, paths, message: file })
    }

    /// After `CommitPlan::run`: `agent.commit` = the short sha, plus one `commit` log line.
    pub fn record_commit(&mut self, plan: &CommitPlan, sha: (&str, &str), actor: &str) -> Result<()> {
        self.set_task_fields(&plan.id, &[("agent.commit", Value::from(sha.0))], actor)?;
        let to = json!({ "sha": sha.1, "repo": tilde(&plan.repo), "files": plan.paths.len() });
        Ok(log(&self.root, &LogEntry::new(actor, "commit", &plan.id, None, Value::Null, to))?)
    }

    /// Commit the task's changes in its repo: prepare, run, record. Returns the short sha.
    pub fn commit_task(&mut self, id: &str, accept_drift: bool, actor: &str) -> Result<String> {
        let plan = self.prepare_commit(id, accept_drift)?;
        let (short, full) = plan.run()?;
        self.record_commit(&plan, (&short, &full), actor)?;
        Ok(short)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn paths_stay_inside_the_repo() {
        for ok in ["a.ts", "src/a b.ts", "-rf", "*.rs", ":(top)x"] {
            assert!(check_path(ok).is_ok(), "{ok}");
        }
        for bad in ["", "/etc/passwd", "../x", "a/../../x", "./a", "a\0b"] {
            assert!(check_path(bad).is_err(), "{bad:?}");
        }
    }

    #[test]
    fn long_diffs_are_cut() {
        let text = (0..DIFF_LINES + 5).map(|i| i.to_string()).collect::<Vec<_>>().join("\n");
        assert!(truncate(text).ends_with("\n… 5 more lines"));
        assert_eq!(truncate("a\nb".into()), "a\nb");
    }
}
