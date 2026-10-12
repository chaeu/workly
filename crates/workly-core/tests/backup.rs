//! Workspace backup against temp copies of the fixture, with a temp bare repo as upstream.

use serde_json::Value;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use workly_core::backup::{IGNORE, backup, info, init_backup};
use workly_core::settings::home;

const FIXTURE: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/../../fixtures/workspace");
const WR8: &str = "projects/website-relaunch/tasks/WR-8-content-migration.md";

fn copy_dir(from: &Path, to: &Path) {
    fs::create_dir_all(to).unwrap();
    for e in fs::read_dir(from).unwrap().flatten() {
        let dest = to.join(e.file_name());
        if e.file_type().unwrap().is_dir() {
            copy_dir(&e.path(), &dest);
        } else {
            fs::copy(e.path(), dest).unwrap();
        }
    }
}

fn sh(dir: &Path, args: &[&str]) -> String {
    let o = Command::new("git").args(args).current_dir(dir).output().unwrap();
    assert!(o.status.success(), "git {args:?}: {}", String::from_utf8_lossy(&o.stderr));
    String::from_utf8_lossy(&o.stdout).trim().to_string()
}

/// Fixture copy, not a repo yet.
fn fixture() -> (tempfile::TempDir, PathBuf) {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path().join("ws");
    copy_dir(Path::new(FIXTURE), &root);
    (dir, root)
}

/// Fixture copy as a repo, with a local identity so the user's git config does not matter.
fn repo() -> (tempfile::TempDir, PathBuf) {
    let (dir, root) = fixture();
    init_backup(&root).unwrap();
    sh(&root, &["config", "user.name", "Test"]);
    sh(&root, &["config", "user.email", "test@example.com"]);
    sh(&root, &["config", "commit.gpgsign", "false"]);
    (dir, root)
}

/// A bare repo next to the workspace as `origin`, current branch tracking it.
fn with_upstream(dir: &Path, root: &Path) -> PathBuf {
    let bare = dir.join("remote.git");
    sh(dir, &["init", "-q", "--bare", bare.to_str().unwrap()]);
    sh(root, &["remote", "add", "origin", bare.to_str().unwrap()]);
    sh(root, &["commit", "-q", "--allow-empty", "-m", "start"]);
    sh(root, &["push", "-q", "-u", "origin", "HEAD"]);
    bare
}

fn edit(root: &Path) {
    let p = root.join(WR8);
    fs::write(&p, fs::read_to_string(&p).unwrap() + "\nedited\n").unwrap();
}

fn backup_lines(root: &Path) -> Vec<Value> {
    let mut lines = Vec::new();
    for e in fs::read_dir(root.join(".workly/log")).unwrap().flatten() {
        for l in fs::read_to_string(e.path()).unwrap().lines() {
            let v: Value = serde_json::from_str(l).unwrap();
            if v["kind"] == "backup" {
                lines.push(v);
            }
        }
    }
    lines
}

#[test]
fn init_new_repo_and_gitignore() {
    let (_dir, root) = fixture();
    init_backup(&root).unwrap();
    assert!(root.join(".git").is_dir());
    assert_eq!(fs::read_to_string(root.join(".gitignore")).unwrap(), IGNORE.join("\n") + "\n");
    // No commit yet: the first backup makes it.
    assert!(Command::new("git").args(["log"]).current_dir(&root).output().unwrap().status.code() != Some(0));
    init_backup(&root).unwrap();
    assert_eq!(fs::read_to_string(root.join(".gitignore")).unwrap(), IGNORE.join("\n") + "\n");
}

#[test]
fn init_appends_missing_lines_only() {
    let (_dir, root) = fixture();
    fs::write(root.join(".gitignore"), "# mine\n.DS_Store\nnode_modules").unwrap();
    init_backup(&root).unwrap();
    assert_eq!(
        fs::read_to_string(root.join(".gitignore")).unwrap(),
        "# mine\n.DS_Store\nnode_modules\n.workly/runs/\n.obsidian/workspace*.json\n*.workly-tmp\n"
    );
}

#[test]
fn existing_repo_with_upstream_is_used_as_is() {
    let (dir, root) = repo();
    let bare = with_upstream(dir.path(), &root);
    let head = sh(&root, &["rev-parse", "HEAD"]);
    let branch = sh(&root, &["symbolic-ref", "--short", "HEAD"]);
    init_backup(&root).unwrap();
    assert_eq!(sh(&root, &["rev-parse", "HEAD"]), head);
    assert_eq!(sh(&root, &["remote", "get-url", "origin"]), bare.to_str().unwrap());
    let i = info(&root);
    assert!(i.repo && !i.cloud);
    assert_eq!(i.branch.as_deref(), Some(branch.as_str()));
    assert_eq!(i.upstream, Some(format!("origin/{branch}")));
}

#[test]
fn commit_message_count_and_log_line() {
    let (_dir, root) = repo();
    let b = backup(&root, true);
    assert_eq!(b.error, None);
    let sha = b.sha.clone().unwrap();
    let tracked = sh(&root, &["ls-files"]).lines().count();
    assert_eq!(b.files, tracked);
    assert!(!sh(&root, &["ls-files"]).contains(".DS_Store"));
    let msg = sh(&root, &["log", "-1", "--format=%s"]);
    assert!(msg.starts_with("backup: ") && msg.ends_with(&format!(" ({tracked} files)")), "{msg}");
    assert_eq!(msg.len(), "backup: 2026-10-12 14:05 ()".len() + format!("{tracked} files").len());

    let lines = backup_lines(&root);
    assert_eq!(lines.len(), 1);
    assert_eq!(lines[0]["to"]["sha"], sha.as_str());
    assert_eq!(lines[0]["to"]["files"], tracked);
    assert_eq!(lines[0]["to"]["pushed"], false);
    assert_eq!(lines[0]["to"]["error"], Value::Null);
}

#[test]
fn nothing_to_commit_and_own_log_line_waits() {
    let (_dir, root) = repo();
    let first = backup(&root, true).sha.unwrap();
    // Only our own log line changed since: no commit, no new log line.
    let b = backup(&root, true);
    assert_eq!((b.sha, b.error), (None, None));
    assert_eq!(sh(&root, &["rev-parse", "HEAD"]), first);
    assert_eq!(backup_lines(&root).len(), 1);
    // A real change takes the waiting log line along.
    edit(&root);
    let b = backup(&root, true);
    assert_eq!(b.files, 2);
    assert!(sh(&root, &["log", "-1", "--format=%s"]).ends_with("(2 files)"));
}

#[test]
fn push_to_upstream() {
    let (dir, root) = repo();
    let bare = with_upstream(dir.path(), &root);
    let branch = sh(&root, &["symbolic-ref", "--short", "HEAD"]);
    edit(&root);
    // Push off: local commit only.
    let b = backup(&root, false);
    assert!(b.sha.is_some() && !b.pushed && b.error.is_none());
    assert_eq!(b.upstream, Some(format!("origin/{branch}")));
    // Push on, nothing new: the commit still ahead is pushed.
    let b = backup(&root, true);
    assert!(b.sha.is_none() && b.pushed && b.error.is_none());
    assert_eq!(sh(&bare, &["rev-parse", &branch]), sh(&root, &["rev-parse", "HEAD"]));
    edit(&root);
    let b = backup(&root, true);
    assert!(b.pushed && b.error.is_none());
    assert_eq!(sh(&bare, &["rev-parse", &branch]), b.sha.unwrap());
}

#[test]
fn failed_push_keeps_the_commit() {
    let (dir, root) = repo();
    with_upstream(dir.path(), &root);
    sh(&root, &["remote", "set-url", "origin", dir.path().join("gone.git").to_str().unwrap()]);
    edit(&root);
    let b = backup(&root, true);
    assert!(b.sha.is_some() && !b.pushed);
    assert!(b.error.as_deref().unwrap().starts_with("git push failed"), "{:?}", b.error);
    assert!(backup_lines(&root).last().unwrap()["to"]["error"].is_string());
}

#[test]
fn no_upstream_is_a_local_commit_without_error() {
    let (_dir, root) = repo();
    let b = backup(&root, true);
    assert!(b.sha.is_some() && !b.pushed);
    assert_eq!((b.upstream, b.error), (None, None));
}

#[test]
fn detached_head_and_merge_are_skipped() {
    let (_dir, root) = repo();
    backup(&root, true);
    edit(&root);
    let head = sh(&root, &["rev-parse", "HEAD"]);

    sh(&root, &["checkout", "-q", "--detach"]);
    let b = backup(&root, true);
    assert!(b.sha.is_none() && b.error.as_deref().unwrap().starts_with("Detached HEAD"));
    sh(&root, &["checkout", "-q", "-"]);

    fs::write(root.join(".git/MERGE_HEAD"), format!("{head}\n")).unwrap();
    let b = backup(&root, true);
    assert!(b.sha.is_none() && b.error.as_deref().unwrap().starts_with("A merge is in progress"));
    assert_eq!(sh(&root, &["rev-parse", "HEAD"]), head);
    assert_eq!(backup_lines(&root).len(), 1);
}

#[test]
fn not_a_repo_and_repo_around_it() {
    // A workspace inside another repo is not backed up: that repo could be a code repo.
    let (dir, root) = fixture();
    sh(dir.path(), &["init", "-q"]);
    let b = backup(&root, true);
    assert!(b.sha.is_none() && b.error.as_deref().unwrap().contains("not a Git repo"));
    assert!(!info(&root).repo);
}

#[test]
fn cloud_storage_is_refused() {
    let root = home().join("Library/CloudStorage/workly-test-does-not-exist/ws");
    let e = init_backup(&root).unwrap_err().to_string();
    assert!(e.contains("OneDrive or iCloud"), "{e}");
    assert!(!root.exists());
}
