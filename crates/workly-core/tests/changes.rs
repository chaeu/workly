//! B8: baseline at start, changes at review, commit on a click. Temp copies of the
//! fixture with a temp code repo as the first repo of project WR.

use serde_json::{Value, json};
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use workly_core::{Error, Workspace};

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

fn write(repo: &Path, path: &str, text: &str) {
    let p = repo.join(path);
    fs::create_dir_all(p.parent().unwrap()).unwrap();
    fs::write(p, text).unwrap();
}

/// Workspace copy plus a code repo with a.txt … e.txt committed; project WR points at it.
fn setup() -> (tempfile::TempDir, PathBuf, PathBuf, Workspace) {
    let dir = tempfile::tempdir().unwrap();
    let (root, repo) = (dir.path().join("ws"), dir.path().join("code repo"));
    copy_dir(Path::new(FIXTURE), &root);
    fs::create_dir_all(&repo).unwrap();
    sh(&repo, &["init", "-q"]);
    sh(&repo, &["config", "user.name", "Test"]);
    sh(&repo, &["config", "user.email", "test@example.com"]);
    sh(&repo, &["config", "commit.gpgsign", "false"]);
    for f in ["a.txt", "b.txt", "c.txt", "d.txt", "gone.txt"] {
        write(&repo, f, &format!("{f}\n"));
    }
    sh(&repo, &["add", "-A"]);
    sh(&repo, &["commit", "-q", "-m", "init"]);
    let repo = repo.canonicalize().unwrap();
    let mut ws = Workspace::open(&root).unwrap();
    ws.update_project_field("WR", "repos", &json!([repo.to_str().unwrap()]), "app").unwrap();
    ws.clear_agent("WR-3", "app").unwrap(); // WR-3's agent works in the same repo
    (dir, root, repo, ws)
}

fn agent(ws: &Workspace, id: &str) -> Value {
    serde_json::to_value(ws.index.task(id).unwrap().task.agent.clone()).unwrap()
}

fn log_lines(root: &Path) -> Vec<Value> {
    let dir = root.join(".workly/log");
    let mut out = Vec::new();
    for e in fs::read_dir(dir).unwrap().flatten() {
        out.extend(fs::read_to_string(e.path()).unwrap().lines().map(|l| serde_json::from_str::<Value>(l).unwrap()));
    }
    out
}

/// The user's own edits before the start, then the agent's run, then review.
fn agent_run(ws: &mut Workspace, repo: &Path) {
    write(repo, "a.txt", "user\n"); // dirty before, untouched by the agent
    write(repo, "b.txt", "user\n"); // dirty before, changed again by the agent
    write(repo, "u.txt", "untracked\n"); // untracked before, untouched
    ws.start_task("WR-8", Some("codex"), "agent:codex").unwrap();
    write(repo, "b.txt", "user\nagent\n");
    write(repo, "c.txt", "agent\n");
    write(repo, "new dir/n.txt", "new\n");
    fs::remove_file(repo.join("gone.txt")).unwrap();
    fs::rename(repo.join("d.txt"), repo.join("e.txt")).unwrap();
    ws.review_task("WR-8", None, Some("Migrated pages\nVerify: open /blog"), "agent:codex").unwrap();
}

#[test]
fn baseline_and_change_detection() {
    let (_tmp, root, repo, mut ws) = setup();
    agent_run(&mut ws, &repo);
    let base: Value = serde_json::from_str(&fs::read_to_string(root.join(".workly/runs/WR-8/baseline.json")).unwrap()).unwrap();
    assert_eq!(base["repo"], json!(repo.to_str().unwrap()));
    assert_eq!(base["head"], json!(sh(&repo, &["rev-parse", "HEAD"])));
    let keys: Vec<&String> = base["files"].as_object().unwrap().keys().collect();
    assert_eq!(keys, ["a.txt", "b.txt", "u.txt"]);

    let a = agent(&ws, "WR-8");
    assert_eq!(a["changes"], json!(["b.txt", "c.txt", "d.txt", "e.txt", "gone.txt", "new dir/n.txt"]));
    assert_eq!(a["repo"], json!(repo.to_str().unwrap()));
    assert_eq!((&a["active"], &a["commit"]), (&Value::Null, &Value::Null));
    let review: Value = serde_json::from_str(&fs::read_to_string(root.join(".workly/runs/WR-8/review.json")).unwrap()).unwrap();
    assert_eq!(review["summary"], json!("Migrated pages\nVerify: open /blog"));
    assert_eq!(review["files"]["gone.txt"], Value::Null);
    assert_eq!(review["files"].as_object().unwrap().len(), 6);

    let kinds: Vec<(String, &str, bool)> = ws.task_changes("WR-8").unwrap().into_iter().map(|f| (f.path, f.kind, f.drifted)).collect();
    let want = [("b.txt", "modified"), ("c.txt", "modified"), ("d.txt", "deleted"), ("e.txt", "new"), ("gone.txt", "deleted"), ("new dir/n.txt", "new")];
    assert_eq!(kinds, want.map(|(p, k)| (p.to_string(), k, false)));
    assert!(ws.change_diff("WR-8", "c.txt").unwrap().contains("-c.txt\n+agent"));
    assert_eq!(ws.change_diff("WR-8", "new dir/n.txt").unwrap(), "+new");
    assert!(matches!(ws.change_diff("WR-8", "a.txt"), Err(Error::Invalid(_))));
}

#[test]
fn commit_contains_exactly_the_task_paths() {
    let (_tmp, root, repo, mut ws) = setup();
    agent_run(&mut ws, &repo);
    let before = sh(&repo, &["rev-parse", "HEAD"]);
    let short = ws.commit_task("WR-8", false, "app").unwrap();
    assert_eq!(sh(&repo, &["rev-parse", "HEAD~1"]), before);
    let files = sh(&repo, &["show", "--name-only", "--no-renames", "--format=", "HEAD"]);
    assert_eq!(files.lines().collect::<Vec<_>>(), ["b.txt", "c.txt", "d.txt", "e.txt", "gone.txt", "new dir/n.txt"]);
    assert_eq!(sh(&repo, &["log", "-1", "--format=%B"]), "WR-8: Write: content migration plan\n\nMigrated pages\nVerify: open /blog");
    // The user's own changes stay where they were.
    assert_eq!(sh(&repo, &["status", "--porcelain", "--untracked-files=all"]), "M a.txt\n?? u.txt");
    assert_eq!(agent(&ws, "WR-8")["commit"], json!(short));
    assert!(sh(&repo, &["rev-parse", "HEAD"]).starts_with(&short));
    let last = log_lines(&root).pop().unwrap();
    assert_eq!((&last["kind"], &last["id"], &last["to"]["files"]), (&json!("commit"), &json!("WR-8"), &json!(6)));
    assert_eq!(last["to"]["sha"], json!(sh(&repo, &["rev-parse", "HEAD"])));
    // Committed: the list stays, the files are clean, a second commit has nothing to do.
    assert!(ws.task_changes("WR-8").unwrap().iter().all(|f| f.kind == "clean" && !f.drifted));
    assert!(matches!(ws.commit_task("WR-8", false, "app"), Err(Error::Invalid(_))));
}

#[test]
fn refuses_foreign_staged_changes() {
    let (_tmp, _root, repo, mut ws) = setup();
    agent_run(&mut ws, &repo);
    sh(&repo, &["add", "a.txt"]);
    let head = sh(&repo, &["rev-parse", "HEAD"]);
    let err = ws.commit_task("WR-8", false, "app").unwrap_err();
    assert!(matches!(&err, Error::Conflict(m) if m.contains("a.txt")), "{err}");
    assert_eq!(sh(&repo, &["rev-parse", "HEAD"]), head);
    assert_eq!(agent(&ws, "WR-8")["commit"], Value::Null);
}

#[test]
fn edits_after_review_need_a_confirmation() {
    let (_tmp, _root, repo, mut ws) = setup();
    agent_run(&mut ws, &repo);
    write(&repo, "c.txt", "agent\nuser after review\n");
    let drifted: Vec<String> = ws.task_changes("WR-8").unwrap().into_iter().filter(|f| f.drifted).map(|f| f.path).collect();
    assert_eq!(drifted, ["c.txt"]);
    assert!(matches!(ws.commit_task("WR-8", false, "app"), Err(Error::Conflict(m)) if m.contains("c.txt")));
    ws.commit_task("WR-8", true, "app").unwrap();
    assert_eq!(sh(&repo, &["show", "HEAD:c.txt"]), "agent\nuser after review");
}

#[test]
fn paths_from_the_file_are_checked() {
    let (_tmp, root, repo, mut ws) = setup();
    agent_run(&mut ws, &repo);
    let file = root.join(WR8);
    let src = fs::read_to_string(&file).unwrap();
    fs::write(&file, src.replace("    - c.txt\n", "    - ../../etc/passwd\n")).unwrap();
    ws.rescan();
    let head = sh(&repo, &["rev-parse", "HEAD"]);
    assert!(matches!(ws.commit_task("WR-8", false, "app"), Err(Error::Invalid(m)) if m.contains("../../etc/passwd")));
    assert_eq!(sh(&repo, &["rev-parse", "HEAD"]), head);
}

#[test]
fn review_writes_changes_losslessly() {
    let (_tmp, root, repo, mut ws) = setup();
    // Without an agent name only the status changes, so the agent block is review's own
    // (no `commit: null` line: a missing field already reads as null).
    ws.start_task("WR-8", None, "cli").unwrap();
    write(&repo, "c.txt", "agent\n");
    let before = fs::read_to_string(root.join(WR8)).unwrap();
    ws.review_task("WR-8", None, None, "cli").unwrap();
    let after = fs::read_to_string(root.join(WR8)).unwrap();
    let agent_block = format!("agent:\n  changes:\n    - c.txt\n  repo: {}\n", repo.display());
    let expected = before.replace("status: doing\n", "status: review\n").replace("done_at: null\n---", &format!("done_at: null\n{agent_block}---"));
    assert_eq!(after, expected);
}

#[test]
fn rework_keeps_the_first_baseline() {
    let (_tmp, _root, repo, mut ws) = setup();
    ws.start_task("WR-8", Some("codex"), "agent:codex").unwrap();
    write(&repo, "a.txt", "run 1\n");
    write(&repo, "b.txt", "run 1\n");
    ws.review_task("WR-8", None, None, "agent:codex").unwrap();
    ws.start_task("WR-8", Some("codex"), "agent:codex").unwrap(); // back from review
    write(&repo, "a.txt", "run 2\n");
    ws.review_task("WR-8", None, None, "agent:codex").unwrap();
    assert_eq!(agent(&ws, "WR-8")["changes"], json!(["a.txt", "b.txt"]));
}

#[test]
fn without_a_repo_start_and_review_still_work() {
    let (_tmp, root, _repo, mut ws) = setup();
    ws.start_task("NA-1", Some("codex"), "agent:codex").unwrap();
    assert!(!root.join(".workly/runs/NA-1/baseline.json").exists());
    ws.review_task("NA-1", None, None, "agent:codex").unwrap();
    assert_eq!(agent(&ws, "NA-1")["changes"], Value::Null);
    // A folder that is no git repo: no baseline either.
    let plain = root.parent().unwrap().join("plain");
    fs::create_dir_all(&plain).unwrap();
    ws.update_project_field("WR", "repos", &json!([plain.to_str().unwrap()]), "app").unwrap();
    ws.start_task("WR-8", Some("codex"), "agent:codex").unwrap();
    assert!(!root.join(".workly/runs/WR-8/baseline.json").exists());
    assert!(ws.ensure_baseline("WR-8").is_ok_and(|b| !b));
}
