//! Runs the `wly` binary against a temp copy of the fixture.

use serde_json::Value;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};

const FIXTURE: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/../../fixtures/workspace");
const WR5: &str = "projects/website-relaunch/tasks/WR-5-navigation.md";

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
    let out = wly(ws, &["task", "start", "WR-5", "--agent", "codex"]);
    assert_eq!((code(&out), String::from_utf8_lossy(&out.stdout).trim()), (0, "WR-5 doing"));
    let started = fs::read_to_string(root.join(WR5)).unwrap();
    assert!(started.contains("status: doing\n") && started.contains("  active: codex\n  since: 20"));
    // Untouched lines stay.
    assert!(started.contains("  effort: high\n  active: codex\n") && started.ends_with(before.split("---\n").last().unwrap()));

    assert_eq!(code(&wly(ws, &["task", "note", "WR-5", "--agent", "codex", "Burger", "menu", "done"])), 0);
    let t = stdout_json(&wly(ws, &["task", "review", "WR-5", "--agent", "codex", "--commit", "b4d2e9f", "--json"]));
    assert_eq!((&t["status"], &t["agent"]["active"], &t["agent"]["commit"]), (&"review".into(), &Value::Null, &"b4d2e9f".into()));
    let file = fs::read_to_string(root.join(WR5)).unwrap();
    assert!(file.trim_end().ends_with(" · codex: Burger menu done"), "{file}");

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
}
