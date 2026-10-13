//! Runs the `wly` binary against a temp copy of the fixture.

use serde_json::Value;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};

const FIXTURE: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/../../fixtures/workspace");
const WR5: &str = "projects/website-relaunch/tasks/WR-5-navigation.md";
const WR3: &str = "projects/website-relaunch/tasks/WR-3-header-component.md";

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

/// Fresh fixture copy; never write to `fixtures/` itself.
fn fixture() -> (tempfile::TempDir, PathBuf) {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path().join("ws");
    copy_dir(Path::new(FIXTURE), &root);
    (dir, root)
}

/// `wly` with a clean environment: no agent, workspace only from `ws`, settings in an empty HOME.
fn wly(ws: Option<&Path>, args: &[&str]) -> Output {
    let home = tempfile::tempdir().unwrap();
    let mut cmd = Command::new(env!("CARGO_BIN_EXE_wly"));
    cmd.args(args).env_remove("WORKLY_AGENT").env_remove("WORKLY_WORKSPACE").env("HOME", home.path());
    if let Some(ws) = ws {
        cmd.env("WORKLY_WORKSPACE", ws);
    }
    cmd.output().unwrap()
}

/// WR-3's agent works in the website repo, which blocks agent starts there (one agent per repo).
fn free_website_repo(root: &Path) {
    let file = root.join(WR3);
    fs::write(&file, fs::read_to_string(&file).unwrap().replace("  active: codex\n", "  active: null\n")).unwrap();
}

fn code(o: &Output) -> i32 {
    o.status.code().unwrap()
}

fn stdout_json(o: &Output) -> Value {
    assert_eq!(code(o), 0, "stderr: {}", String::from_utf8_lossy(&o.stderr));
    serde_json::from_slice(&o.stdout).unwrap()
}

fn log_actors(root: &Path) -> Vec<String> {
    let mut out = Vec::new();
    for e in fs::read_dir(root.join(".workly/log")).unwrap().flatten() {
        if e.path().extension().is_some_and(|x| x == "jsonl") {
            for l in fs::read_to_string(e.path()).unwrap().lines() {
                out.push(serde_json::from_str::<Value>(l).unwrap()["actor"].as_str().unwrap().to_string());
            }
        }
    }
    out
}

#[test]
fn list_and_show_json() {
    let (_tmp, root) = fixture();
    let list = stdout_json(&wly(Some(&root), &["task", "list", "--project", "WR", "--status", "review", "--json"]));
    let list = list.as_array().unwrap();
    assert_eq!(list.len(), 1);
    let t = &list[0];
    assert_eq!((&t["id"], &t["project"], &t["agent"]["commit"]), (&"WR-7".into(), &"WR".into(), &"a3f9c1e".into()));
    assert!(t["path"].as_str().unwrap().starts_with(root.canonicalize().unwrap().to_str().unwrap()));
    assert!(t.get("body").is_none());

    let show = stdout_json(&wly(Some(&root), &["task", "show", "WR-5", "--json"]));
    assert_eq!(show["task"]["status"], "todo");
    assert_eq!(show["task"]["body"].as_str().unwrap().trim(), "Burger menu below 720 px, keyboard accessible.");
    assert_eq!(show["project"]["key"], "WR");
    let ctx = &show["context"];
    assert!(ctx["global"]["content"].as_str().unwrap().contains("wly task start"));
    assert!(ctx["project"]["path"].as_str().unwrap().ends_with("website-relaunch/agent/AGENTS.md"));
    assert_eq!(ctx["skills"][0]["name"], "release-check");
    assert!(Path::new(ctx["skills"][0]["path"].as_str().unwrap()).is_file());

    let inbox = stdout_json(&wly(Some(&root), &["task", "show", "IN-1", "--json"]));
    assert!(inbox["project"].is_null() && inbox["context"]["project"].is_null() && inbox["context"]["global"].is_object());

    let projects = stdout_json(&wly(Some(&root), &["project", "list", "--json"]));
    assert!(projects.as_array().unwrap().iter().any(|p| p["key"] == "WR" && p["repos"][0] == "~/repos/website"));
}

#[test]
fn agent_flow_start_note_review() {
    let (_tmp, root) = fixture();
    let ws = Some(root.as_path());
    let before = fs::read_to_string(root.join(WR5)).unwrap();
    // One agent per repo: refused with the other task's id, nothing written.
    let out = wly(ws, &["task", "start", "WR-5", "--agent", "codex"]);
    assert_eq!(code(&out), 3);
    assert!(String::from_utf8_lossy(&out.stderr).contains("WR-3 already has an agent working in ~/repos/website"));
    assert_eq!(fs::read_to_string(root.join(WR5)).unwrap(), before);
    free_website_repo(&root);
    let out = wly(ws, &["task", "start", "WR-5", "--agent", "codex"]);
    assert_eq!((code(&out), String::from_utf8_lossy(&out.stdout).trim()), (0, "WR-5 doing"));
    let started = fs::read_to_string(root.join(WR5)).unwrap();
    assert!(started.contains("status: doing\n") && started.contains("  active: codex\n  since: 20"));
    // Untouched lines stay.
    assert!(started.contains("  effort: high\n  active: codex\n") && started.ends_with(before.split("---\n").last().unwrap()));

    // A note goes to the run log of the current run; the task file stays byte-identical.
    assert_eq!(code(&wly(ws, &["task", "note", "WR-5", "--agent", "codex", "Burger", "menu", "done"])), 0);
    assert_eq!(fs::read_to_string(root.join(WR5)).unwrap(), started);
    let run_log = fs::read_to_string(root.join(".workly/runs/WR-5/log.jsonl")).unwrap();
    let entry: Value = serde_json::from_str(run_log.trim_end()).unwrap();
    let since = started.split("  since: ").nth(1).unwrap().lines().next().unwrap();
    assert_eq!((&entry["agent"], &entry["run"], &entry["text"]), (&"codex".into(), &since.into(), &"Burger menu done".into()));
    // --to-body: the old behaviour, a line under ## Updates.
    assert_eq!(code(&wly(ws, &["task", "note", "WR-5", "--to-body", "--agent", "codex", "Into", "the", "body"])), 0);
    assert!(fs::read_to_string(root.join(WR5)).unwrap().trim_end().ends_with(" · codex: Into the body"));

    let t = stdout_json(&wly(ws, &["task", "review", "WR-5", "--agent", "codex", "--commit", "b4d2e9f", "--summary", "Burger menu\nVerify: resize", "--json"]));
    assert_eq!((&t["status"], &t["agent"]["active"], &t["agent"]["commit"]), (&"review".into(), &Value::Null, &"b4d2e9f".into()));
    let file = fs::read_to_string(root.join(WR5)).unwrap();
    assert!(file.ends_with(" · codex: Burger menu\n  Verify: resize\n"), "{file}");
    // No agent active any more: notes belong to no run.
    assert_eq!(code(&wly(ws, &["task", "note", "WR-5", "late"])), 0);
    let last = fs::read_to_string(root.join(".workly/runs/WR-5/log.jsonl")).unwrap().lines().last().unwrap().to_string();
    let entry: Value = serde_json::from_str(&last).unwrap();
    assert_eq!((&entry["agent"], &entry["run"]), (&"cli".into(), &Value::Null));

    // Review twice, start a done task, agents setting done: invalid transitions.
    assert_eq!(code(&wly(ws, &["task", "review", "WR-5", "--agent", "codex"])), 3);
    assert_eq!(code(&wly(ws, &["task", "start", "WR-1"])), 3);
    assert_eq!(code(&wly(ws, &["task", "done", "WR-5", "--agent", "codex"])), 3);
    assert!(fs::read_to_string(root.join(WR5)).unwrap().contains("status: review\n"));
    // A human may.
    assert_eq!(code(&wly(ws, &["task", "done", "WR-5"])), 0);

    let actors = log_actors(&root);
    assert!(actors.iter().filter(|a| *a == "agent:codex").count() >= 6);
    assert_eq!(actors.last().unwrap(), "cli");
}

#[test]
fn agent_from_env_and_add() {
    let (_tmp, root) = fixture();
    let out = Command::new(env!("CARGO_BIN_EXE_wly"))
        .args(["task", "add", "Follow-up: alt text", "--project", "WR", "--priority", "1"])
        .env("WORKLY_WORKSPACE", &root)
        .env("WORKLY_AGENT", "claude-code")
        .output()
        .unwrap();
    assert_eq!((code(&out), String::from_utf8_lossy(&out.stdout).trim()), (0, "WR-10"));
    assert_eq!(log_actors(&root).last().unwrap(), "agent:claude-code");
    assert!(root.join("projects/website-relaunch/tasks/WR-10-follow-up-alt-text.md").is_file());
    let t = stdout_json(&wly(Some(&root), &["task", "add", "Inbox thing", "--json"]));
    assert_eq!((&t["id"], &t["project"]), (&"IN-4".into(), &Value::Null));
}

#[test]
fn errors_and_exit_codes() {
    let (_tmp, root) = fixture();
    let ws = Some(root.as_path());
    // Not found.
    let out = wly(ws, &["task", "show", "WR-99"]);
    assert_eq!(code(&out), 2);
    assert!(out.stdout.is_empty() && String::from_utf8_lossy(&out.stderr).starts_with("wly: task WR-99 not found"));
    let out = wly(ws, &["task", "show", "WR-99", "--json"]);
    let err: Value = serde_json::from_slice(&out.stderr).unwrap();
    assert_eq!((code(&out), &err["error"]["code"]), (2, &2.into()));
    assert_eq!(code(&wly(ws, &["task", "list", "--project", "ZZ"])), 2);
    // Usage errors, also as JSON.
    assert_eq!(code(&wly(ws, &["task", "frobnicate"])), 1);
    assert_eq!(code(&wly(ws, &["task", "add", "x", "--priority", "7"])), 1);
    assert_eq!(code(&wly(ws, &["task", "add", "x", "--project", "WR", "--inbox"])), 1);
    assert_eq!(code(&wly(ws, &["task", "list", "--status", "nope"])), 1);
    assert_eq!(code(&wly(ws, &["task", "start", "WR-5", "--agent", "bad name"])), 1);
    let out = wly(ws, &["task", "nope", "--json"]);
    assert_eq!(serde_json::from_slice::<Value>(&out.stderr).unwrap()["error"]["code"], 1);
    // Help and version are not errors.
    assert_eq!(code(&wly(ws, &["--version"])), 0);
    // No workspace anywhere (empty HOME), or a folder that is none.
    assert_eq!(code(&wly(None, &["task", "list"])), 1);
    assert_eq!(code(&wly(None, &["--workspace", root.join("projects").to_str().unwrap(), "task", "list"])), 2);
}

/// What the open app sees: the CLI's write arrives through the watcher, as an external change.
#[test]
fn app_watcher_sees_cli_writes() {
    use std::sync::mpsc;
    use std::time::Duration;
    let (_tmp, root) = fixture();
    free_website_repo(&root);
    let mut app = workly_core::Workspace::open(&root).unwrap();
    let (tx, rx) = mpsc::channel();
    let _watcher = app.watch(move |changes| tx.send(changes).unwrap()).unwrap();
    while rx.recv_timeout(Duration::from_millis(400)).is_ok() {} // settle events from the copy

    assert_eq!(code(&wly(Some(&root), &["task", "start", "WR-5", "--agent", "codex"])), 0);
    let batch = rx.recv_timeout(Duration::from_secs(1)).expect("no watcher event within 1 s");
    assert_eq!(batch, [workly_core::watch::Change::Changed(WR5.into())]);
    app.rescan();
    let t = &app.index.task("WR-5").unwrap().task;
    assert_eq!((t.status.as_str(), t.agent.as_ref().unwrap().active.as_deref()), ("doing", Some("codex")));

    // A note only touches the run log; the event says so, the app skips the rescan.
    assert_eq!(code(&wly(Some(&root), &["task", "note", "WR-5", "--agent", "codex", "hi"])), 0);
    let mut seen = Vec::new();
    while let Ok(batch) = rx.recv_timeout(Duration::from_millis(600)) {
        seen.extend(batch);
    }
    assert!(seen.iter().any(|c| *c == workly_core::watch::Change::Changed(".workly/runs/WR-5/log.jsonl".into())), "{seen:?}");
    assert!(seen.iter().all(|c| c.is_run_log()), "{seen:?}");
}

#[test]
fn parent_waiting_and_acceptance() {
    let (_tmp, root) = fixture();
    let ws = Some(root.as_path());
    let t = stdout_json(&wly(ws, &["task", "add", "Slow job alert", "--project", "OPS", "--parent", "OPS-2", "--json"]));
    assert_eq!((&t["id"], &t["parent"], &t["status"]), (&"OPS-4".into(), &"OPS-2".into(), &"backlog".into()));
    // One level: OPS-3 already has a parent.
    let out = wly(ws, &["task", "add", "Deeper", "--project", "OPS", "--parent", "OPS-3"]);
    assert_eq!(code(&out), 1);
    assert!(String::from_utf8_lossy(&out.stderr).contains("one level"));
    let list = stdout_json(&wly(ws, &["task", "list", "--parent", "OPS-2", "--json"]));
    let ids: Vec<&str> = list.as_array().unwrap().iter().map(|t| t["id"].as_str().unwrap()).collect();
    assert_eq!(ids, ["OPS-3", "OPS-4"]);
    let ops3 = &list[0];
    assert_eq!(ops3["waiting_for"], "IT: read access to the job database");
    assert_eq!(ops3["acceptance"], serde_json::json!([{ "text": "Query returns the jobs of the last 24 h", "done": false }]));
    assert_eq!(code(&wly(ws, &["task", "list", "--parent", "OPS-99"])), 2);
    // start works from waiting and drops waiting_for.
    let t = stdout_json(&wly(ws, &["task", "start", "OPS-3", "--agent", "codex", "--json"]));
    assert_eq!((&t["status"], &t["waiting_for"]), (&"doing".into(), &Value::Null));
}

#[test]
fn start_and_review_record_the_changed_files() {
    let (tmp, root) = fixture();
    let ws = Some(root.as_path());
    let repo = tmp.path().join("code");
    fs::create_dir_all(&repo).unwrap();
    let git = |args: &[&str]| assert!(Command::new("git").args(args).current_dir(&repo).output().unwrap().status.success(), "git {args:?}");
    git(&["init", "-q"]);
    fs::write(repo.join("mine.txt"), "dirty before the start\n").unwrap();
    let repo = repo.canonicalize().unwrap();
    let project = root.join("projects/website-relaunch/_project.md");
    let src = fs::read_to_string(&project).unwrap().replace("repos: [~/repos/website]", &format!("repos: [\"{}\"]", repo.display()));
    fs::write(&project, src).unwrap();
    free_website_repo(&root);

    assert_eq!(code(&wly(ws, &["task", "start", "WR-5", "--agent", "codex"])), 0);
    assert!(root.join(".workly/runs/WR-5/baseline.json").exists());
    fs::write(repo.join("agent.txt"), "new\n").unwrap();
    let t = stdout_json(&wly(ws, &["task", "review", "WR-5", "--agent", "codex", "--summary", "Added agent.txt", "--json"]));
    assert_eq!((&t["agent"]["changes"], &t["agent"]["repo"]), (&serde_json::json!(["agent.txt"]), &repo.to_str().unwrap().into()));
    assert!(root.join(".workly/runs/WR-5/review.json").exists());
}
