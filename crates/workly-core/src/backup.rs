//! Workspace backup: Workly commits the workspace repo (never a code repo) and pushes
//! to the current branch's upstream. System git with an args array, never a shell,
//! never interactive. Never forces, never touches other branches, never rewrites history.

use crate::settings::{home, tilde};
use crate::write::{LogEntry, OwnWrites, atomic_write, log};
use crate::{Error, Result};
use serde::Serialize;
use serde_json::{Value, json};
use std::fs;
use std::io::{self, Read};
use std::path::Path;
use std::process::{Command, Stdio};
use std::thread;
use std::time::{Duration, Instant};

/// Lines every workspace `.gitignore` gets.
pub const IGNORE: [&str; 4] = [".workly/runs/", ".DS_Store", ".obsidian/workspace*.json", "*.workly-tmp"];
/// Per git call; also catches a credential helper or gpg waiting for input.
const TIMEOUT: Duration = Duration::from_secs(30);
/// stderr lines kept for the UI.
const TAIL: usize = 20;
/// Only the log changed: that is our own backup line, it rides along with the next real backup.
const OWN_LOG: &str = ".workly/log/";
/// `git status --porcelain` codes of unmerged paths.
const CONFLICT: [&str; 7] = ["DD", "AU", "UD", "UA", "DU", "AA", "UU"];

#[derive(Debug, Clone, Default, PartialEq, Serialize)]
pub struct Backup {
    /// The new commit; None when nothing changed or the run was skipped.
    pub sha: Option<String>,
    pub files: usize,
    /// True when HEAD is on the upstream after the run.
    pub pushed: bool,
    /// `origin/main`; None = local commits only.
    pub upstream: Option<String>,
    /// Why the run was skipped or the push failed, with git's stderr tail.
    pub error: Option<String>,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize)]
pub struct BackupInfo {
    /// The workspace folder itself is a repo (has `.git`).
    pub repo: bool,
    pub branch: Option<String>,
    pub upstream: Option<String>,
    /// Remote URL, credentials removed.
    pub url: Option<String>,
    /// Under OneDrive or iCloud: `init_backup` refuses.
    pub cloud: bool,
}

/// OneDrive, Dropbox & co. (`~/Library/CloudStorage`) and iCloud Drive break `.git` folders.
pub fn in_cloud_storage(path: &Path) -> bool {
    let h = home();
    let real = path.canonicalize().unwrap_or_else(|_| path.to_path_buf());
    !h.as_os_str().is_empty() && ["Library/CloudStorage", "Library/Mobile Documents"].iter().any(|d| real.starts_with(h.join(d)))
}

/// The workspace folder itself is a repo. A workspace inside another repo is not,
/// so Workly never commits a surrounding code repo.
pub fn is_repo(root: &Path) -> bool {
    root.join(".git").exists()
}

/// `git init` unless the folder is a repo, and the ignore lines missing from `.gitignore`.
/// An existing repo, its branches and remotes are used as they are.
pub fn init_backup(root: &Path) -> Result<()> {
    if in_cloud_storage(root) {
        return Err(Error::Invalid(format!(
            "{} is synced by OneDrive or iCloud, which breaks Git repos. Move the workspace to a local folder to enable backup.",
            tilde(root)
        )));
    }
    if !is_repo(root) {
        git(root, &["init", "-q"]).map_err(|e| Error::Invalid(format!("git init failed: {e}")))?;
    }
    let file = root.join(".gitignore");
    let old = match fs::read_to_string(&file) {
        Err(e) if e.kind() == io::ErrorKind::NotFound => String::new(),
        r => r?,
    };
    let have: Vec<&str> = old.lines().map(str::trim).collect();
    let missing: Vec<&str> = IGNORE.into_iter().filter(|l| !have.contains(l)).collect();
    if !missing.is_empty() {
        let mut new = old.clone();
        if !new.is_empty() && !new.ends_with('\n') {
            new.push('\n');
        }
        for l in missing {
            new.push_str(l);
            new.push('\n');
        }
        atomic_write(&file, &new, &OwnWrites::default())?;
    }
    Ok(())
}

/// Commit everything that changed and push to the upstream (when `push`). Never fails:
/// a skipped run or a failed push comes back in `error`. A new commit is logged.
pub fn backup(root: &Path, push: bool) -> Backup {
    let mut b = Backup::default();
    if let Err(e) = run(root, push, &mut b) {
        b.error = Some(e);
    }
    if let Some(sha) = &b.sha {
        let to = json!({ "sha": sha, "files": b.files, "pushed": b.pushed, "error": b.error });
        if let Err(e) = log(root, &LogEntry::new("app", "backup", "", None, Value::Null, to)) {
            b.error.get_or_insert(format!("Backup committed, but the log line failed: {e}"));
        }
    }
    b
}

fn run(root: &Path, push: bool, b: &mut Backup) -> std::result::Result<(), String> {
    if !is_repo(root) {
        return Err("The workspace is not a Git repo. Settings → Backup → Enable backup.".into());
    }
    let Ok(branch) = git(root, &["symbolic-ref", "--short", "-q", "HEAD"]) else {
        return Err("Detached HEAD. Check out a branch; backup resumes afterwards.".into());
    };
    let git_dir = git(root, &["rev-parse", "--absolute-git-dir"])?;
    let busy = [("MERGE_HEAD", "merge"), ("rebase-merge", "rebase"), ("rebase-apply", "rebase"), ("CHERRY_PICK_HEAD", "cherry-pick"), ("REVERT_HEAD", "revert")];
    if let Some((_, what)) = busy.iter().find(|(p, _)| Path::new(&git_dir).join(p).exists()) {
        return Err(format!("A {what} is in progress. Finish or abort it in Git; backup resumes afterwards."));
    }
    let status = git(root, &["status", "--porcelain"])?;
    if status.lines().any(|l| CONFLICT.contains(&l.get(..2).unwrap_or_default())) {
        return Err("Conflicts in the index. Resolve them in Git; backup resumes afterwards.".into());
    }
    if status.lines().any(|l| !l.get(3..).unwrap_or_default().trim_matches('"').starts_with(OWN_LOG)) {
        git(root, &["add", "-A"])?;
        let files = git(root, &["diff", "--cached", "--name-only"])?.lines().count();
        if files > 0 {
            let when = chrono::Local::now().format("%Y-%m-%d %H:%M");
            let msg = format!("backup: {when} ({files} {})", if files == 1 { "file" } else { "files" });
            git(root, &["commit", "-q", "-m", &msg])?;
            b.sha = Some(git(root, &["rev-parse", "HEAD"])?);
            b.files = files;
        }
    }

    let Some((remote, merge)) = upstream(root, &branch) else { return Ok(()) };
    b.upstream = Some(format!("{remote}/{}", merge.trim_start_matches("refs/heads/")));
    // Unknown (tracking ref never fetched) counts as ahead.
    let ahead = git(root, &["rev-list", "--count", "@{u}..HEAD"]).map_or(1, |n| n.parse().unwrap_or(1));
    if ahead == 0 {
        b.pushed = true;
    } else if push {
        git(root, &["push", "-q", &remote, &format!("HEAD:{merge}")])?;
        b.pushed = true;
    }
    Ok(())
}

/// (remote, merge ref) of the branch; None without upstream or for a local one (`.`).
fn upstream(root: &Path, branch: &str) -> Option<(String, String)> {
    let remote = git(root, &["config", &format!("branch.{branch}.remote")]).ok()?;
    let merge = git(root, &["config", &format!("branch.{branch}.merge")]).ok()?;
    (remote != ".").then_some((remote, merge))
}

/// What Settings shows: repo, branch, upstream and its URL.
pub fn info(root: &Path) -> BackupInfo {
    let mut i = BackupInfo { repo: is_repo(root), cloud: in_cloud_storage(root), ..Default::default() };
    if !i.repo {
        return i;
    }
    i.branch = git(root, &["symbolic-ref", "--short", "-q", "HEAD"]).ok();
    if let Some((remote, merge)) = i.branch.as_deref().and_then(|b| upstream(root, b)) {
        i.url = git(root, &["remote", "get-url", &remote]).ok().map(|u| redact(&u));
        i.upstream = Some(format!("{remote}/{}", merge.trim_start_matches("refs/heads/")));
    }
    i
}

/// Trimmed stdout, or a readable error with the last stderr lines. Shared with
/// the code-repo commit (changes.rs).
pub(crate) fn git(root: &Path, args: &[&str]) -> std::result::Result<String, String> {
    git_timeout(root, args, TIMEOUT)
}

/// `git` with its own timeout, e.g. for a commit whose pre-commit hook runs tests.
pub(crate) fn git_timeout(root: &Path, args: &[&str], timeout: Duration) -> std::result::Result<String, String> {
    let mut child = Command::new("git")
        .args(args)
        .current_dir(root)
        // Never ask: a missing credential fails instead of hanging the app.
        .env("GIT_TERMINAL_PROMPT", "0")
        .env("GIT_SSH_COMMAND", "ssh -o BatchMode=yes -o ConnectTimeout=15")
        .env("GCM_INTERACTIVE", "never")
        // Paths are paths: `*.rs` or `:(top)` never expand to more files.
        .env("GIT_LITERAL_PATHSPECS", "1")
        // Only the workspace repo, whatever the app was started with.
        .env_remove("GIT_DIR")
        .env_remove("GIT_WORK_TREE")
        .env_remove("GIT_INDEX_FILE")
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|e| format!("Could not run git: {e}"))?;
    let read = |mut r: Box<dyn Read + Send>| {
        thread::spawn(move || {
            let mut s = String::new();
            let _ = r.read_to_string(&mut s);
            s
        })
    };
    let out = read(Box::new(child.stdout.take().expect("piped")));
    let err = read(Box::new(child.stderr.take().expect("piped")));
    let deadline = Instant::now() + timeout;
    let status = loop {
        match child.try_wait() {
            Ok(Some(s)) => break s,
            Ok(None) if Instant::now() < deadline => thread::sleep(Duration::from_millis(20)),
            // ponytail: the readers are left behind; ssh (a grandchild) may still hold the pipes.
            _ => {
                let _ = child.kill();
                let _ = child.wait();
                return Err(format!("git {} timed out after {} s.", args[0], timeout.as_secs()));
            }
        }
    };
    let (out, err) = (out.join().unwrap_or_default(), err.join().unwrap_or_default());
    if status.success() {
        return Ok(out.trim_end().to_string());
    }
    let lines: Vec<&str> = err.lines().filter(|l| !l.trim().is_empty()).collect();
    let tail = lines[lines.len().saturating_sub(TAIL)..].join("\n");
    Err(redact(format!("git {} failed ({status}).\n{tail}", args[0]).trim_end()))
}

/// `https://user:token@host/x` -> `https://***@host/x`: URLs can carry credentials,
/// and errors end up in the log, which is committed and pushed.
pub fn redact(s: &str) -> String {
    let mut out = String::new();
    let mut rest = s;
    while let Some(i) = rest.find("://") {
        let (head, tail) = rest.split_at(i + 3);
        out.push_str(head);
        let end = tail.find(|c: char| c == '/' || c.is_whitespace()).unwrap_or(tail.len());
        match tail[..end].rfind('@') {
            Some(at) => {
                out.push_str("***");
                rest = &tail[at..];
            }
            None => rest = tail,
        }
    }
    out.push_str(rest);
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn redacts_credentials() {
        assert_eq!(redact("https://u:t0k@github.com/a/b.git"), "https://***@github.com/a/b.git");
        assert_eq!(redact("fatal: 'https://x@h/r' and ssh://git@h:22/r"), "fatal: 'https://***@h/r' and ssh://***@h:22/r");
        assert_eq!(redact("git@github.com:a/b.git https://h/r"), "git@github.com:a/b.git https://h/r");
    }

    #[test]
    fn cloud_paths() {
        assert!(in_cloud_storage(&home().join("Library/CloudStorage/OneDrive-X/ws")));
        assert!(in_cloud_storage(&home().join("Library/Mobile Documents/com~apple~CloudDocs/ws")));
        assert!(!in_cloud_storage(&home().join("Workspaces/ws")));
    }
}
