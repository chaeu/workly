//! Workspace operations against a temp copy of the fixture.

use serde_json::{Value, json};
use std::fs;
use std::path::{Path, PathBuf};
use std::time::Instant;
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

/// Fresh fixture copy; never write to `fixtures/` itself.
fn fixture() -> (tempfile::TempDir, PathBuf) {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path().join("ws");
    copy_dir(Path::new(FIXTURE), &root);
    (dir, root)
}

fn log_lines(root: &Path) -> Vec<Value> {
    let mut lines = Vec::new();
    for e in fs::read_dir(root.join(".workly/log")).unwrap().flatten() {
        if e.path().extension().is_some_and(|x| x == "jsonl") {
            lines.extend(fs::read_to_string(e.path()).unwrap().lines().map(|l| serde_json::from_str::<Value>(l).unwrap()));
        }
    }
    lines
}

#[test]
fn loads_fixture_with_one_parse_error() {
    let ws = Workspace::open(FIXTURE).unwrap();
    let idx = &ws.index;
    let keys: Vec<&str> = idx.projects.iter().map(|p| p.project.key.as_str()).collect();
    assert_eq!(keys, ["WR", "IE", "OPS", "NA", "ST", "OI"]);
    assert_eq!(idx.tasks.len(), 12);
    assert_eq!(idx.errors.len(), 1, "{:?}", idx.errors);
    let e = &idx.errors[0];
    assert_eq!(e.path, "inbox/IN-3-broken-frontmatter.md");
    assert!(e.line.is_some());
    assert_eq!(idx.trash, ["projects/website-relaunch/tasks/WR-9-old-banner.md"]);
    let oi = idx.task("OI-1").unwrap();
    assert_eq!(oi.project.as_deref(), Some("projects/archive/old-intranet"));
    assert_eq!(idx.task("IN-1").unwrap().project, None);
    assert_eq!(idx.project("NA").unwrap().project.usecase.as_ref().unwrap().decisions.len(), 2);
    assert_eq!(idx.task("WR-3").unwrap().task.agent.as_ref().unwrap().active.as_deref(), Some("codex"));
    let process = ws.process.as_ref().unwrap();
    assert_eq!(process.steps.len(), 17);
    assert!(process.validate().is_empty());
    assert_eq!(ws.config.task_statuses[2].wip_limit, Some(3));
}

#[test]
fn scan_fixture_is_fast() {
    let mut ws = Workspace::open(FIXTURE).unwrap();
    let start = Instant::now();
    ws.rescan();
    let took = start.elapsed();
    let limit = if cfg!(debug_assertions) { 200 } else { 20 };
    assert!(took.as_millis() < limit, "scan took {took:?}");
    eprintln!("scan: {took:?}");
}

#[test]
fn next_ids_include_trash_and_broken_files() {
    let ws = Workspace::open(FIXTURE).unwrap();
    assert_eq!(ws.next_id("WR"), "WR-10");
    assert_eq!(ws.next_id("IN"), "IN-4");
    assert_eq!(ws.next_id("IE"), "IE-5");
    assert_eq!(ws.next_id("ST"), "ST-1");
}

#[test]
fn update_changes_one_line_and_logs() {
    let (_tmp, root) = fixture();
    let before = fs::read_to_string(root.join(WR8)).unwrap();
    let mut ws = Workspace::open(&root).unwrap();
    ws.update_task_field("WR-8", "status", &json!("doing"), "app").unwrap();
    let after = fs::read_to_string(root.join(WR8)).unwrap();
    assert_eq!(after, before.replace("status: backlog\n", "status: doing\n"));
    assert_eq!(ws.index.task("WR-8").unwrap().task.status, "doing");

    let last = log_lines(&root).pop().unwrap();
    assert_eq!(last["actor"], "app");
    assert_eq!(last["kind"], "task.update");
    assert_eq!((&last["id"], &last["field"], &last["from"], &last["to"]), (&json!("WR-8"), &json!("status"), &json!("backlog"), &json!("doing")));
    let keys: Vec<&String> = last.as_object().unwrap().keys().collect();
    assert_eq!(keys.len(), 7);

    // Same value: no write, no log line.
    let n = log_lines(&root).len();
    ws.update_task_field("WR-8", "status", &json!("doing"), "app").unwrap();
    assert_eq!(log_lines(&root).len(), n);
    // No temp files left behind.
    let dir = root.join("projects/website-relaunch/tasks");
    assert!(fs::read_dir(dir).unwrap().flatten().all(|e| !e.file_name().to_string_lossy().starts_with('.')));
}

#[test]
fn update_rules() {
    let (_tmp, root) = fixture();
    let mut ws = Workspace::open(&root).unwrap();
    assert!(matches!(ws.update_task_field("WR-5", "status", &json!("done"), "agent:codex"), Err(Error::InvalidTransition(_))));
    assert!(matches!(ws.update_task_field("WR-5", "status", &json!("nope"), "app"), Err(Error::Invalid(_))));
    assert!(matches!(ws.update_task_field("WR-5", "priority", &json!(7), "app"), Err(Error::Invalid(_))));
    assert!(matches!(ws.update_task_field("WR-5", "id", &json!("WR-50"), "app"), Err(Error::Invalid(_))));
    assert!(matches!(ws.update_task_field("XX-1", "status", &json!("todo"), "app"), Err(Error::NotFound(_))));
    ws.update_task_field("WR-5", "status", &json!("done"), "app").unwrap();
    ws.update_task_field("WR-5", "agent.active", &json!("codex"), "cli").unwrap();
    assert_eq!(ws.index.task("WR-5").unwrap().task.agent.as_ref().unwrap().active.as_deref(), Some("codex"));
}

#[test]
fn create_tasks() {
    let (_tmp, root) = fixture();
    let mut ws = Workspace::open(&root).unwrap();
    // Project without a tasks/ folder.
    let id = ws.create_task("Collect volume: numbers", Some("ST"), "app").unwrap();
    assert_eq!(id, "ST-1");
    let src = fs::read_to_string(root.join("projects/support-triage/tasks/ST-1-collect-volume-numbers.md")).unwrap();
    assert!(src.starts_with("---\nid: ST-1\ntitle: \"Collect volume: numbers\"\nstatus: todo\n"), "{src}");
    assert_eq!(ws.create_task("Inbox thing", None, "app").unwrap(), "IN-4");
    assert_eq!(ws.create_task("Next", Some("WR"), "app").unwrap(), "WR-10");
    assert!(matches!(ws.create_task("x", Some("NOPE"), "app"), Err(Error::NotFound(_))));
    assert_eq!(log_lines(&root).iter().filter(|l| l["kind"] == "task.create").count(), 3);
}

#[test]
fn move_keeps_id_and_file_name() {
    let (_tmp, root) = fixture();
    let mut ws = Workspace::open(&root).unwrap();
    let before = fs::read_to_string(root.join(WR8)).unwrap();
    ws.move_task("WR-8", Some("ST"), "app").unwrap();
    let moved = root.join("projects/support-triage/tasks/WR-8-content-migration.md");
    assert_eq!(fs::read_to_string(&moved).unwrap(), before);
    let t = ws.index.task("WR-8").unwrap();
    assert_eq!(t.project.as_deref(), Some("projects/support-triage"));
    ws.move_task("WR-8", None, "app").unwrap();
    assert!(root.join("inbox/WR-8-content-migration.md").exists());
    assert_eq!(ws.next_id("WR"), "WR-10");
}

#[test]
fn delete_and_restore_round_trip() {
    let (_tmp, root) = fixture();
    let mut ws = Workspace::open(&root).unwrap();
    let before = fs::read_to_string(root.join(WR8)).unwrap();
    ws.delete_task("WR-8", "app").unwrap();
    assert!(!root.join(WR8).exists());
    assert!(ws.index.task("WR-8").is_none());
    let trashed = fs::read_to_string(root.join(".workly/trash").join(WR8)).unwrap();
    assert!(trashed.contains("\ndeleted_at: "));
    assert!(ws.index.trash.contains(&WR8.to_string()));

    ws.restore(WR8, "app").unwrap();
    assert_eq!(fs::read_to_string(root.join(WR8)).unwrap(), before);
    assert!(ws.index.task("WR-8").is_some());

    // Restore refuses to overwrite.
    ws.delete_task("WR-8", "app").unwrap();
    fs::write(root.join(WR8), "someone else's file").unwrap();
    ws.rescan();
    assert!(matches!(ws.restore(WR8, "app"), Err(Error::Invalid(_))));
    assert!(matches!(ws.restore("nope.md", "app"), Err(Error::NotFound(_))));
}

#[test]
fn broken_config_and_process_are_reported() {
    let (_tmp, root) = fixture();
    fs::write(root.join(".workly/config.yml"), "scan_exclude: [unclosed\n").unwrap();
    let p = root.join(".workly/process.yml");
    let src = fs::read_to_string(&p).unwrap().replace("{ from: run,     to: end,", "{ from: run,     to: nowhere,");
    fs::write(&p, src).unwrap();
    let ws = Workspace::open(&root).unwrap();
    let msgs: Vec<String> = ws.index.errors.iter().map(|e| format!("{}: {}", e.path, e.message)).collect();
    assert!(msgs[0].starts_with(".workly/config.yml: "), "{msgs:?}");
    assert_eq!(msgs[1], ".workly/process.yml: edge 20 (run -> nowhere) references unknown step 'nowhere'");
    // Defaults keep the app usable.
    assert_eq!(ws.index.projects.len(), 6);
}
