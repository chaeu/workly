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
    let id = ws.create_task("Collect volume: numbers", Some("ST"), None, "app").unwrap();
    assert_eq!(id, "ST-1");
    let src = fs::read_to_string(root.join("projects/support-triage/tasks/ST-1-collect-volume-numbers.md")).unwrap();
    assert!(src.starts_with("---\nid: ST-1\ntitle: \"Collect volume: numbers\"\nstatus: todo\n"), "{src}");
    assert_eq!(ws.create_task("Inbox thing", None, None, "app").unwrap(), "IN-4");
    assert_eq!(ws.create_task("Next", Some("WR"), None, "app").unwrap(), "WR-10");
    assert!(matches!(ws.create_task("x", Some("NOPE"), None, "app"), Err(Error::NotFound(_))));
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

// ---------------------------------------------------------------- projects (M2)

use workly_core::project::{NewProject, init_workspace, is_workspace};

#[test]
fn empty_folder_becomes_workspace() {
    let tmp = tempfile::tempdir().unwrap();
    let root = tmp.path().join("new ws");
    assert!(!is_workspace(&root));
    init_workspace(&root).unwrap();
    assert!(is_workspace(&root));
    for p in ["_templates/project.md", "_templates/task.md", "inbox", "projects", "knowledge", ".workly/process.yml", ".workly/agent/AGENTS.md", ".workly/trash"] {
        assert!(root.join(p).exists(), "{p}");
    }
    assert_eq!(fs::read_to_string(root.join(".workly/config.yml")).unwrap(), fs::read_to_string(Path::new(FIXTURE).join(".workly/config.yml")).unwrap());
    // Existing files are never overwritten.
    fs::write(root.join(".workly/agent/AGENTS.md"), "mine").unwrap();
    init_workspace(&root).unwrap();
    assert_eq!(fs::read_to_string(root.join(".workly/agent/AGENTS.md")).unwrap(), "mine");

    let mut ws = Workspace::open(&root).unwrap();
    assert!(ws.index.errors.is_empty() && ws.index.projects.is_empty(), "{:?}", ws.index.errors);
    assert_eq!(ws.suggest_key("Test Alpha"), "TA");
    let new = NewProject { title: "Test Alpha".into(), key: "TA".into(), color: "proj-1".into(), repos: vec![] };
    assert_eq!(ws.create_project(&new, "app").unwrap(), "projects/test-alpha");
    assert_eq!(ws.create_task("First", Some("TA"), None, "app").unwrap(), "TA-1");
}

#[test]
fn create_project_builds_structure() {
    let (_tmp, root) = fixture();
    let mut ws = Workspace::open(&root).unwrap();
    let repo = workly_core::settings::home().join("repos/alpha");
    let new = NewProject { title: "Über: Alpha".into(), key: "UA".into(), color: "proj-3".into(), repos: vec!["~/repos/alpha".into()] };
    let dir = ws.create_project(&new, "app").unwrap();
    assert_eq!(dir, "projects/ueber-alpha");
    let d = root.join(&dir);
    for sub in ["tasks", "docs", "notes", "decisions", "agent/AGENTS.md"] {
        assert!(d.join(sub).exists(), "{sub}");
    }
    let p = &ws.index.project("UA").unwrap().project;
    assert_eq!((p.title.as_str(), p.status.as_str(), p.color.as_deref(), p.order), ("Über: Alpha", "active", Some("proj-3"), Some(7)));
    assert_eq!(p.repos, ["~/repos/alpha"]);
    let src = fs::read_to_string(d.join("_project.md")).unwrap();
    assert!(src.contains("title: \"Über: Alpha\"\n") && src.contains("repos: [~/repos/alpha]\n") && src.contains("## Goal"), "{src}");
    let cw: Value = serde_json::from_str(&fs::read_to_string(d.join("UA.code-workspace")).unwrap()).unwrap();
    assert_eq!(cw["folders"], json!([{ "path": "." }, { "path": repo }]));
    assert_eq!(log_lines(&root).pop().unwrap()["kind"], "project.create");

    let mut bad = |key: &str, title: &str, color: &str| {
        let n = NewProject { title: title.into(), key: key.into(), color: color.into(), repos: vec![] };
        matches!(ws.create_project(&n, "app"), Err(Error::Invalid(_)))
    };
    assert!(bad("WR", "Other", "proj-1"), "key taken");
    assert!(bad("IN", "Other", "proj-1"), "reserved");
    assert!(bad("ab", "Other", "proj-1"), "lowercase");
    assert!(bad("ZZ", "Über: Alpha", "proj-1"), "folder exists");
    assert!(bad("ZZ", "  ", "proj-1"), "no title");
    assert!(bad("ZZ", "Other", "#fff"), "colour");
}

#[test]
fn update_project_is_lossless() {
    let (_tmp, root) = fixture();
    let file = root.join("projects/website-relaunch/_project.md");
    let before = fs::read_to_string(&file).unwrap();
    let mut ws = Workspace::open(&root).unwrap();
    ws.update_project_field("WR", "title", &json!("Site Relaunch"), "app").unwrap();
    ws.update_project_field("WR", "status", &json!("paused"), "app").unwrap();
    let after = fs::read_to_string(&file).unwrap();
    assert_eq!(after, before.replace("title: Website Relaunch\n", "title: Site Relaunch\n").replace("status: active\n", "status: paused\n"));
    let last = log_lines(&root).pop().unwrap();
    assert_eq!((&last["kind"], &last["id"], &last["field"]), (&json!("project.update"), &json!("WR"), &json!("status")));

    ws.update_project_field("WR", "repos", &json!(["~/repos/website", "/opt/api"]), "app").unwrap();
    let cw = fs::read_to_string(root.join("projects/website-relaunch/WR.code-workspace")).unwrap();
    assert!(cw.contains("/opt/api"), "{cw}");

    for (field, value) in [("key", json!("XX")), ("status", json!("done")), ("color", json!("red")), ("title", json!(""))] {
        assert!(matches!(ws.update_project_field("WR", field, &value, "app"), Err(Error::Invalid(_))), "{field}");
    }
}

#[test]
fn reorder_writes_order() {
    let (_tmp, root) = fixture();
    let mut ws = Workspace::open(&root).unwrap();
    let keys: Vec<String> = ["IE", "WR", "OPS", "NA", "ST", "OI"].map(String::from).into();
    let n = log_lines(&root).len();
    ws.reorder_projects(&keys, "app").unwrap();
    assert_eq!(log_lines(&root).len(), n + 2, "only IE and WR changed");
    let order: Vec<&str> = ws.index.projects.iter().map(|p| p.project.key.as_str()).collect();
    assert_eq!(order, ["IE", "WR", "OPS", "NA", "ST", "OI"]);
}

#[test]
fn delete_project_moves_folder_to_trash() {
    let (_tmp, root) = fixture();
    let mut ws = Workspace::open(&root).unwrap();
    ws.delete_project("OI", "app").unwrap();
    assert!(!root.join("projects/archive/old-intranet").exists());
    assert!(root.join(".workly/trash/projects/archive/old-intranet/_project.md").is_file());
    assert!(ws.index.project("OI").is_none() && ws.index.task("OI-1").is_none());
    // Ids keep counting through the trash.
    assert_eq!(ws.next_id("OI"), "OI-2");
    assert_eq!(log_lines(&root).pop().unwrap()["kind"], "project.delete");
}

#[test]
fn project_files_and_preview() {
    let (_tmp, root) = fixture();
    let ws = Workspace::open(&root).unwrap();
    assert_eq!(
        ws.project_files("WR").unwrap(),
        [
            "projects/website-relaunch/agent/AGENTS.md",
            "projects/website-relaunch/agent/skills/release-check/SKILL.md",
            "projects/website-relaunch/decisions/0001-hosting.md",
            "projects/website-relaunch/docs/architecture.md",
            "projects/website-relaunch/notes/2026-09-15-kickoff.md",
        ]
    );
    assert!(ws.project_files("ST").unwrap().is_empty());
    let body = ws.read_markdown("projects/website-relaunch/_project.md").unwrap();
    assert!(body.starts_with("Relaunch of the portfolio site"), "{body}");
    assert!(matches!(ws.read_markdown("../outside.md"), Err(Error::NotFound(_))));
    fs::write(root.parent().unwrap().join("outside.md"), "x").unwrap();
    assert!(matches!(ws.read_markdown("../outside.md"), Err(Error::Invalid(_))));
    assert!(matches!(ws.read_markdown("projects/../../outside.md"), Err(Error::Invalid(_))));
}

#[test]
fn done_stamps_and_clears_done_at() {
    let (_tmp, root) = fixture();
    let before = fs::read_to_string(root.join(WR8)).unwrap();
    let mut ws = Workspace::open(&root).unwrap();
    ws.update_task_field("WR-8", "status", &json!("done"), "app").unwrap();
    let today = workly_core::today();
    let done = before.replace("status: backlog\n", "status: done\n").replace("done_at: null\n", &format!("done_at: {today}\n"));
    assert_eq!(fs::read_to_string(root.join(WR8)).unwrap(), done);
    ws.update_task_field("WR-8", "status", &json!("backlog"), "app").unwrap();
    assert_eq!(fs::read_to_string(root.join(WR8)).unwrap(), before);
}

#[test]
fn reorder_tasks_adds_one_line() {
    let (_tmp, root) = fixture();
    let before = fs::read_to_string(root.join(WR8)).unwrap();
    let mut ws = Workspace::open(&root).unwrap();
    ws.reorder_tasks(&["WR-8".into(), "NA-1".into()], "app").unwrap();
    assert_eq!(fs::read_to_string(root.join(WR8)).unwrap(), before.replace("done_at: null\n---", "done_at: null\norder: 1\n---"));
    assert_eq!(ws.index.task("NA-1").unwrap().task.order, Some(2));
    // Unchanged orders are not rewritten.
    let n = log_lines(&root).len();
    ws.reorder_tasks(&["WR-8".into()], "app").unwrap();
    assert_eq!(log_lines(&root).len(), n);
    assert!(matches!(ws.reorder_tasks(&["XX-1".into()], "app"), Err(Error::NotFound(_))));
}

#[test]
fn focus_sets_and_removes_lines() {
    let (_tmp, root) = fixture();
    let before = fs::read_to_string(root.join(WR8)).unwrap();
    let mut ws = Workspace::open(&root).unwrap();
    let today = workly_core::today();
    let ids = |v: &[&str]| v.iter().map(|s| s.to_string()).collect::<Vec<_>>();
    ws.set_focus(&ids(&["IN-1", "WR-8", "IN-2"]), "app").unwrap();
    let focused = before.replace("done_at: null\n---", &format!("done_at: null\nfocus: {today}\nfocus_order: 2\n---"));
    assert_eq!(fs::read_to_string(root.join(WR8)).unwrap(), focused);
    // Fixture tasks focused on another day are not today's strip.
    let strip = |ws: &Workspace| {
        let mut t: Vec<_> = ws.index.tasks.iter().filter(|t| t.task.focus.as_deref() == Some(today.as_str())).collect();
        t.sort_by_key(|t| t.task.focus_order);
        t.iter().map(|t| t.task.id.clone()).collect::<Vec<_>>()
    };
    let fixture_today: Vec<String> = Workspace::open(FIXTURE).unwrap().index.tasks.iter()
        .filter(|t| t.task.focus.as_deref() == Some(today.as_str())).map(|t| t.task.id.clone()).collect();
    if fixture_today.is_empty() {
        assert_eq!(strip(&ws), ["IN-1", "WR-8", "IN-2"]);
    }
    // Reorder, then drop WR-8: its lines go, the file is as before.
    ws.set_focus(&ids(&["WR-8", "IN-1"]), "app").unwrap();
    assert_eq!(fs::read_to_string(root.join(WR8)).unwrap(), focused.replace("focus_order: 2", "focus_order: 1"));
    ws.set_focus(&ids(&["IN-1"]), "app").unwrap();
    assert_eq!(fs::read_to_string(root.join(WR8)).unwrap(), before);
    assert_eq!(strip(&ws), ["IN-1"]);
    assert!(!fs::read_to_string(root.join("inbox/IN-2-read-article.md")).unwrap().contains("focus"));
    assert!(matches!(ws.set_focus(&ids(&["IN-1", "IN-2", "WR-8", "WR-5"]), "app"), Err(Error::Invalid(_))));
    assert!(matches!(ws.set_focus(&ids(&["IN-1", "IN-1"]), "app"), Err(Error::Invalid(_))));
    assert!(matches!(ws.set_focus(&ids(&["XX-1"]), "app"), Err(Error::NotFound(_))));
}

#[test]
fn update_note_appends_to_body_only() {
    let (_tmp, root) = fixture();
    let before = fs::read_to_string(root.join(WR8)).unwrap();
    let mut ws = Workspace::open(&root).unwrap();
    let notes = |root: &Path| log_lines(root).iter().filter(|l| l["kind"] == "task.note").count();
    let n = notes(&root);
    ws.add_task_update("WR-8", "Asked the client\nabout URLs", "app").unwrap();
    let after = fs::read_to_string(root.join(WR8)).unwrap();
    let added = after.strip_prefix(before.as_str()).expect("original bytes kept");
    assert!(added.starts_with("\n\n## Updates\n- 20") && added.ends_with(" · me: Asked the client about URLs\n"), "{added:?}");
    ws.add_task_update("WR-7", "Checked", "agent:codex").unwrap();
    assert!(fs::read_to_string(root.join("projects/website-relaunch/tasks/WR-7-lighthouse-audit.md")).unwrap().trim_end().ends_with(" · codex: Checked"));
    assert!(matches!(ws.add_task_update("WR-8", "  ", "app"), Err(Error::Invalid(_))));
    assert_eq!(notes(&root), n + 2);
}

#[test]
fn create_task_with_priority() {
    let (_tmp, root) = fixture();
    let mut ws = Workspace::open(&root).unwrap();
    let id = ws.create_task("  Urgent thing ", Some("WR"), Some(1), "app").unwrap();
    let t = ws.index.task(&id).unwrap();
    assert_eq!((t.task.title.as_str(), t.task.priority), ("Urgent thing", Some(1)));
    assert!(t.path.ends_with("WR-10-urgent-thing.md"));
    assert!(matches!(ws.create_task(" ", None, None, "app"), Err(Error::Invalid(_))));
    assert!(matches!(ws.create_task("x", None, Some(5), "app"), Err(Error::Invalid(_))));
}
