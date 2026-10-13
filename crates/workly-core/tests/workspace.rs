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
    assert_eq!(idx.tasks.len(), 14);
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
    assert_eq!(process.steps.len(), 19);
    assert!(process.validate().is_empty());
    assert_eq!(ws.config.task_statuses[2].wip_limit, Some(3));
    let ids: Vec<&str> = ws.config.task_statuses.iter().map(|s| s.id.as_str()).collect();
    assert_eq!(ids, ["backlog", "todo", "doing", "waiting", "review", "done"]);
    let ops3 = &idx.task("OPS-3").unwrap().task;
    assert_eq!((ops3.waiting_for.as_deref(), ops3.parent.as_deref()), (Some("IT: read access to the job database"), Some("OPS-2")));
}

#[test]
fn default_config_matches_the_fixture() {
    let ws = Workspace::open(FIXTURE).unwrap();
    let ids = |c: &workly_core::model::Config| c.task_statuses.iter().map(|s| s.id.clone()).collect::<Vec<_>>();
    assert_eq!(ids(&workly_core::model::Config::default()), ids(&ws.config));
}

#[test]
fn leaving_waiting_removes_waiting_for() {
    let (_tmp, root) = fixture();
    let file = root.join("projects/ops-dashboard/tasks/OPS-3-job-db-connection.md");
    let before = fs::read_to_string(&file).unwrap();
    let mut ws = Workspace::open(&root).unwrap();
    // Changing another field keeps it.
    ws.update_task_field("OPS-3", "priority", &json!(2), "app").unwrap();
    assert!(ws.index.task("OPS-3").unwrap().task.waiting_for.is_some());
    ws.update_task_field("OPS-3", "status", &json!("todo"), "app").unwrap();
    let after = fs::read_to_string(&file).unwrap();
    let expected = before
        .replace("priority: 1\n", "priority: 2\n")
        .replace("status: waiting\n", "status: todo\n")
        .replace("waiting_for: \"IT: read access to the job database\"\n", "");
    assert_eq!(after, expected);
    let last = log_lines(&root).pop().unwrap();
    assert_eq!((&last["field"], &last["from"], &last["to"]), (&json!("waiting_for"), &json!("IT: read access to the job database"), &Value::Null));
    // `wly task start` works from waiting and clears it too.
    ws.update_task_field("OPS-3", "status", &json!("waiting"), "app").unwrap();
    ws.update_task_field("OPS-3", "waiting_for", &json!("Legal"), "app").unwrap();
    ws.start_task("OPS-3", Some("codex"), "cli").unwrap();
    let t = &ws.index.task("OPS-3").unwrap().task;
    assert_eq!((t.status.as_str(), t.waiting_for.as_deref()), ("doing", None));
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
    assert!(matches!(ws.update_task_field("WR-5", "title", &json!(" "), "app"), Err(Error::Invalid(_))));
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
    assert!(src.starts_with("---\nid: ST-1\ntitle: \"Collect volume: numbers\"\nstatus: backlog\n"), "{src}");
    assert!(src.ends_with("---\n## Description\n\n## Acceptance\n\n## Updates\n"), "{src}");
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
    assert_eq!(msgs[1], ".workly/process.yml: edge 23 (run -> nowhere) references unknown step 'nowhere'");
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
    let new = NewProject { title: "Test Alpha".into(), key: "TA".into(), color: "proj-1".into(), repos: vec![], usecase: None, adopt: false, tracked: true };
    assert_eq!(ws.create_project(&new, "app").unwrap(), "projects/test-alpha");
    assert_eq!(ws.create_task("First", Some("TA"), None, "app").unwrap(), "TA-1");
}

#[test]
fn create_project_builds_structure() {
    let (_tmp, root) = fixture();
    let mut ws = Workspace::open(&root).unwrap();
    let repo = workly_core::settings::home().join("repos/alpha");
    let new = NewProject { title: "Über: Alpha".into(), key: "UA".into(), color: "proj-3".into(), repos: vec!["~/repos/alpha".into()], usecase: None, adopt: false, tracked: true };
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
        let n = NewProject { title: title.into(), key: key.into(), color: color.into(), repos: vec![], usecase: None, adopt: false, tracked: true };
        matches!(ws.create_project(&n, "app"), Err(Error::Invalid(_)))
    };
    assert!(bad("WR", "Other", "proj-1"), "key taken");
    assert!(bad("IN", "Other", "proj-1"), "reserved");
    assert!(bad("ab", "Other", "proj-1"), "lowercase");
    assert!(bad("ZZ", "Über: Alpha", "proj-1"), "folder exists");
    assert!(bad("ZZ", "  ", "proj-1"), "no title");
    assert!(bad("ZZ", "Other", "#fff"), "colour");
}

/// Every file under `dir` with its bytes.
fn snapshot(dir: &Path) -> Vec<(PathBuf, Vec<u8>)> {
    let mut out = Vec::new();
    for e in fs::read_dir(dir).unwrap().flatten() {
        if e.file_type().unwrap().is_dir() {
            out.extend(snapshot(&e.path()));
        } else {
            out.push((e.path(), fs::read(e.path()).unwrap()));
        }
    }
    out.sort();
    out
}

#[test]
fn adopt_existing_folder_keeps_every_file() {
    let (_tmp, root) = fixture();
    let wr8 = fs::read(root.join(WR8)).unwrap();
    let dir = root.join("projects/foo");
    fs::create_dir_all(dir.join("notes")).unwrap();
    fs::create_dir_all(dir.join("agent")).unwrap();
    fs::write(dir.join("notes/idea.md"), "# Idea\n\nLoose note.\n").unwrap();
    fs::write(dir.join("README.md"), "readme\r\nno newline").unwrap();
    fs::write(dir.join("agent/AGENTS.md"), "my rules\n").unwrap();
    fs::write(dir.join("FOO.code-workspace"), "{ \"folders\": [] }").unwrap();
    let before = snapshot(&dir);
    let mut ws = Workspace::open(&root).unwrap();
    let mut new = NewProject { title: "Foo".into(), key: "FOO".into(), color: "proj-2".into(), repos: vec![], usecase: None, adopt: false, tracked: true };

    // Without adopt: a distinguishable refusal, nothing written.
    assert!(matches!(ws.create_project(&new, "app"), Err(Error::Conflict(_))));
    assert_eq!(snapshot(&dir), before);

    new.adopt = true;
    assert_eq!(ws.create_project(&new, "app").unwrap(), "projects/foo");
    let after = snapshot(&dir);
    for file in &before {
        assert!(after.contains(file), "{} changed", file.0.display());
    }
    let added: Vec<String> = after.iter().filter(|f| !before.contains(f)).map(|f| f.0.strip_prefix(&dir).unwrap().display().to_string()).collect();
    assert_eq!(added, ["_project.md"]);
    for sub in ["tasks", "docs", "decisions"] {
        assert!(dir.join(sub).is_dir(), "{sub}");
    }
    assert_eq!(ws.index.project("FOO").unwrap().project.title, "Foo");
    assert_eq!((&log_lines(&root).pop().unwrap()["kind"]).as_str(), Some("project.adopt"));
    assert_eq!(fs::read(root.join(WR8)).unwrap(), wr8);

    // A folder with _project.md already is a project, adopt or not.
    let wr = root.join("projects/website-relaunch/_project.md");
    let wr_before = fs::read(&wr).unwrap();
    let n = NewProject { title: "Website Relaunch".into(), key: "WEB".into(), color: "proj-1".into(), repos: vec![], usecase: None, adopt: true, tracked: true };
    assert!(matches!(ws.create_project(&n, "app"), Err(Error::Invalid(m)) if m.contains("already a project")));
    assert_eq!(fs::read(&wr).unwrap(), wr_before);
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
fn set_description_replaces_only_the_description() {
    let (_tmp, root) = fixture();
    let before = fs::read_to_string(root.join(WR8)).unwrap();
    let old = "Body with trailing spaces   \nand **markdown**.\n\n- [ ] list a\n- [x] list b";
    let (fm, desc) = before.split_at(before.find("Body with").unwrap());
    assert_eq!(desc, old);
    let mut ws = Workspace::open(&root).unwrap();

    // No Updates section, no final newline: stays without one.
    ws.set_description("WR-8", "New plan\n\n- step one\n", old, "app").unwrap();
    assert_eq!(fs::read_to_string(root.join(WR8)).unwrap(), format!("{fm}New plan\n\n- step one"));
    let last = log_lines(&root).pop().unwrap();
    assert_eq!((&last["kind"], &last["field"], &last["from"]), (&json!("task.update"), &json!("description"), &json!(old.trim_end())));

    // With an Updates section: frontmatter and Updates stay byte-identical.
    ws.add_task_update("WR-8", "Asked", "app").unwrap();
    let with_updates = fs::read_to_string(root.join(WR8)).unwrap();
    let updates = &with_updates[with_updates.find("## Updates").unwrap()..];
    ws.set_description("WR-8", "Shorter", "New plan\n\n- step one", "app").unwrap();
    assert_eq!(fs::read_to_string(root.join(WR8)).unwrap(), format!("{fm}Shorter\n\n{updates}"));

    // Unchanged text: no write, no log line.
    let n = log_lines(&root).len();
    ws.set_description("WR-8", "Shorter\n", "Shorter", "app").unwrap();
    assert_eq!(log_lines(&root).len(), n);
}

#[test]
fn set_description_on_empty_body_and_back() {
    let (_tmp, root) = fixture();
    let file = root.join("inbox/IN-1-workshop-date.md");
    let before = fs::read_to_string(&file).unwrap();
    let mut ws = Workspace::open(&root).unwrap();
    ws.set_description("IN-1", "Ask for Tuesday", "", "app").unwrap();
    assert_eq!(fs::read_to_string(&file).unwrap(), format!("{before}Ask for Tuesday\n"));
    ws.set_description("IN-1", "  \n", "Ask for Tuesday", "app").unwrap();
    assert_eq!(fs::read_to_string(&file).unwrap(), before);
}

#[test]
fn set_description_conflict_writes_nothing() {
    let (_tmp, root) = fixture();
    let before = fs::read_to_string(root.join(WR8)).unwrap();
    let mut ws = Workspace::open(&root).unwrap();
    let n = log_lines(&root).len();
    let r = ws.set_description("WR-8", "Mine", "What the card showed before", "app");
    assert!(matches!(r, Err(Error::Conflict(_))), "{r:?}");
    let r = ws.set_description("WR-8", "a\n## Updates\n- fake", &before[before.find("Body").unwrap()..], "app");
    assert!(matches!(r, Err(Error::Invalid(_))), "{r:?}");
    assert_eq!(fs::read_to_string(root.join(WR8)).unwrap(), before);
    assert_eq!(log_lines(&root).len(), n);
}

#[test]
fn set_description_keeps_crlf() {
    let (_tmp, root) = fixture();
    let file = root.join("inbox/IN-7-crlf.md");
    let fm = "---\r\nid: IN-7\r\ntitle: Windows file\r\nstatus: todo\r\n---\r\n";
    let updates = "## Updates\r\n- 2026-10-01 09:00 · me: x\r\n";
    fs::write(&file, format!("{fm}Old\r\ntext\r\n\r\n{updates}")).unwrap();
    let mut ws = Workspace::open(&root).unwrap();
    // The textarea hands back LF; `expected` may still carry the file's CRLF.
    ws.set_description("IN-7", "New\nlines", "Old\r\ntext", "app").unwrap();
    assert_eq!(fs::read_to_string(&file).unwrap(), format!("{fm}New\r\nlines\r\n\r\n{updates}"));
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

// --------------------------------------------------------------- use cases (M4)

#[test]
fn move_usecase_writes_step_since_and_decision() {
    let (_tmp, root) = fixture();
    let file = root.join("projects/invoice-extraction/_project.md");
    let before = fs::read_to_string(&file).unwrap();
    let mut ws = Workspace::open(&root).unwrap();
    let today = workly_core::today();

    // Into a gate: no decision yet.
    ws.move_usecase("IE", "g1", "app").unwrap();
    let at_gate = before.replace("  step: pilot\n  step_since: 2026-09-24\n", &format!("  step: g1\n  step_since: {today}\n"));
    assert_eq!(fs::read_to_string(&file).unwrap(), at_gate);

    // Out of a gate along an edge: the edge label becomes the decision, old items stay verbatim.
    ws.move_usecase("IE", "wd", "app").unwrap();
    let decided = at_gate.replace("  step: g1\n", "  step: wd\n").replace(
        "    - { date: 2026-09-10, gate: null, text: \"Start pilot, timebox 3 weeks\" }\n",
        &format!("    - {{ date: 2026-09-10, gate: null, text: \"Start pilot, timebox 3 weeks\" }}\n    - {{ date: {today}, gate: G1, text: \"Yes\" }}\n"),
    );
    assert_eq!(fs::read_to_string(&file).unwrap(), decided);
    let uc = ws.index.project("IE").unwrap().project.usecase.clone().unwrap();
    assert_eq!((uc.step.as_deref(), uc.decisions.len()), (Some("wd"), 2));

    let last = log_lines(&root).pop().unwrap();
    assert_eq!((&last["kind"], &last["id"], &last["field"], &last["from"], &last["to"]), (&json!("usecase.move"), &json!("IE"), &json!("usecase.step"), &json!("g1"), &json!("wd")));

    // Same step: nothing written.
    ws.move_usecase("IE", "wd", "app").unwrap();
    assert_eq!(fs::read_to_string(&file).unwrap(), decided);
}

#[test]
fn move_usecase_without_decisions_and_off_edge() {
    let (_tmp, root) = fixture();
    let file = root.join("projects/support-triage/_project.md");
    let before = fs::read_to_string(&file).unwrap();
    let mut ws = Workspace::open(&root).unwrap();
    let today = workly_core::today();
    ws.move_usecase("ST", "g3", "app").unwrap();
    // No edge g3 -> parked: the decision says where it went.
    ws.move_usecase("ST", "parked", "app").unwrap();
    let want = before.replace("  step: need\n  step_since: 2026-10-02\n", &format!("  step: parked\n  step_since: {today}\n")).replace(
        "    note: Ticket export only as a sample so far\n",
        &format!("    note: Ticket export only as a sample so far\n  decisions: [{{ date: {today}, gate: G3, text: Moved to Parked }}]\n"),
    );
    assert_eq!(fs::read_to_string(&file).unwrap(), want);
}

#[test]
fn move_usecase_rules() {
    let (_tmp, root) = fixture();
    let mut ws = Workspace::open(&root).unwrap();
    for (key, step) in [("IE", "start"), ("IE", "nowhere"), ("WR", "pilot")] {
        assert!(matches!(ws.move_usecase(key, step, "app"), Err(Error::Invalid(_))), "{key} {step}");
    }
    assert!(matches!(ws.move_usecase("XX", "pilot", "app"), Err(Error::NotFound(_))));
    // Moves take one path; status and type follow process.yml.
    for (field, value) in [("usecase.step", json!("g1")), ("usecase.decisions", json!([])), ("usecase.status", json!("done")), ("usecase.type", json!("ml"))] {
        assert!(matches!(ws.update_project_field("IE", field, &value, "app"), Err(Error::Invalid(_))), "{field}");
    }
    // Status is a property: it never moves the use case.
    ws.update_project_field("IE", "usecase.status", &json!("blocked"), "app").unwrap();
    let uc = ws.index.project("IE").unwrap().project.usecase.clone().unwrap();
    assert_eq!((uc.status.as_deref(), uc.step.as_deref()), (Some("blocked"), Some("pilot")));
    // Areas are free text: one that no list has is accepted as typed.
    ws.update_project_field("IE", "usecase.area", &json!("Underwriting"), "app").unwrap();
    assert_eq!(ws.index.project("IE").unwrap().project.usecase.clone().unwrap().area.as_deref(), Some("Underwriting"));
}

#[test]
fn create_usecase_starts_at_first_phase() {
    let (_tmp, root) = fixture();
    let mut ws = Workspace::open(&root).unwrap();
    let uc = Some(workly_core::project::NewUseCase { kind: "ai".into(), area: Some("Finance".into()) });
    let new = NewProject { title: "Mail Sorting".into(), key: "MS".into(), color: "proj-1".into(), repos: vec![], usecase: uc, adopt: false, tracked: true };
    let dir = ws.create_project(&new, "app").unwrap();
    let src = fs::read_to_string(root.join(dir).join("_project.md")).unwrap();
    let today = workly_core::today();
    let block = format!(
        "created: {today}\nusecase:\n  type: ai\n  area: Finance\n  step: need\n  step_since: {today}\n  status: active\n  blocked_by: null\n  next_step: null\n  current_state: null\n---\n"
    );
    assert!(src.contains(&block), "{src}");
    assert_eq!(ws.index.project("MS").unwrap().project.usecase.as_ref().unwrap().step.as_deref(), Some("need"));

    let bad = Some(workly_core::project::NewUseCase { kind: "ml".into(), area: None });
    let n = NewProject { title: "Other".into(), key: "OT".into(), color: "proj-2".into(), repos: vec![], usecase: bad, adopt: false, tracked: true };
    assert!(matches!(ws.create_project(&n, "app"), Err(Error::Invalid(_))));
    assert!(!root.join("projects/other").exists());
}

// ------------------------------------------------ project <-> use case (P5)

#[test]
fn make_usecase_appends_block_and_remove_restores_bytes() {
    let (_tmp, root) = fixture();
    let file = root.join("projects/website-relaunch/_project.md");
    let before = fs::read_to_string(&file).unwrap();
    let wr8 = fs::read(root.join(WR8)).unwrap();
    let mut ws = Workspace::open(&root).unwrap();
    let uc = workly_core::project::NewUseCase { kind: "hybrid".into(), area: Some(" Marketing ".into()) };
    ws.make_usecase("WR", &uc, "app").unwrap();
    let today = workly_core::today();
    let block = format!(
        "usecase:\n  type: hybrid\n  area: Marketing\n  step: need\n  step_since: {today}\n  status: active\n  blocked_by: null\n  next_step: null\n  current_state: null\n"
    );
    let after = fs::read_to_string(&file).unwrap();
    assert_eq!(after, before.replacen("created: 2026-09-15\n", &format!("created: 2026-09-15\n{block}"), 1));
    assert_eq!(ws.index.project("WR").unwrap().project.usecase.as_ref().unwrap().step.as_deref(), Some("need"));
    let last = log_lines(&root).pop().unwrap();
    assert_eq!((&last["kind"], &last["field"], &last["from"], &last["to"]["step"]), (&json!("usecase.create"), &json!("usecase"), &Value::Null, &json!("need")));

    // Already a use case / unknown type: refused, nothing written.
    assert!(matches!(ws.make_usecase("WR", &uc, "app"), Err(Error::Invalid(_))));
    let bad = workly_core::project::NewUseCase { kind: "ml".into(), area: None };
    let ops = root.join("projects/ops-dashboard/_project.md");
    let ops_before = fs::read(&ops).unwrap();
    assert!(matches!(ws.make_usecase("OPS", &bad, "app"), Err(Error::Invalid(_))));
    assert_eq!(fs::read(&ops).unwrap(), ops_before);

    // And back: the file is byte-identical to before.
    ws.remove_usecase("WR", "app").unwrap();
    assert_eq!(fs::read_to_string(&file).unwrap(), before);
    assert!(ws.index.project("WR").unwrap().project.usecase.is_none());
    assert_eq!(fs::read(root.join(WR8)).unwrap(), wr8);
}

#[test]
fn remove_usecase_keeps_project_tasks_and_logs_the_block() {
    let (_tmp, root) = fixture();
    let file = root.join("projects/invoice-extraction/_project.md");
    let before = fs::read_to_string(&file).unwrap();
    let mut ws = Workspace::open(&root).unwrap();
    let old = ws.index.project("IE").unwrap().project.usecase.clone().unwrap();
    let tasks = ws.index.tasks.iter().filter(|t| t.task.id.starts_with("IE-")).count();
    assert!(tasks > 0);
    ws.remove_usecase("IE", "app").unwrap();

    let start = before.find("usecase:\n").unwrap();
    let end = before.rfind("---\n").unwrap();
    assert_eq!(fs::read_to_string(&file).unwrap(), format!("{}{}", &before[..start], &before[end..]));
    let p = &ws.index.project("IE").unwrap().project;
    assert!(p.usecase.is_none() && p.title == "Invoice Extraction");
    assert_eq!(ws.index.tasks.iter().filter(|t| t.task.id.starts_with("IE-")).count(), tasks);
    let last = log_lines(&root).pop().unwrap();
    assert_eq!((&last["kind"], &last["field"], &last["to"]), (&json!("usecase.remove"), &json!("usecase"), &Value::Null));
    assert_eq!(last["from"]["decisions"][0]["text"], json!(old.decisions[0].text));
    assert_eq!(last["from"]["savings_note"], json!("Volume from the finance team, sample 09/2026"));

    assert!(matches!(ws.remove_usecase("IE", "app"), Err(Error::Invalid(_))));
    assert!(matches!(ws.remove_usecase("NOPE", "app"), Err(Error::NotFound(_))));
}

// ---------------------------------------------------- tracked (A5)

#[test]
fn create_untracked_usecase_writes_tracked_false() {
    let (_tmp, root) = fixture();
    let mut ws = Workspace::open(&root).unwrap();
    let uc = Some(workly_core::project::NewUseCase { kind: "ai".into(), area: None });
    let new = NewProject { title: "Mail Sorting".into(), key: "MS".into(), color: "proj-1".into(), repos: vec![], usecase: uc, adopt: false, tracked: false };
    let dir = ws.create_project(&new, "app").unwrap();
    let src = fs::read_to_string(root.join(dir).join("_project.md")).unwrap();
    assert!(src.contains(&format!("created: {}\ntracked: false\nusecase:\n", workly_core::today())), "{src}");
    assert_eq!(ws.index.project("MS").unwrap().project.tracked, Some(false));

    // A plain project cannot start untracked.
    let plain = NewProject { title: "Other".into(), key: "OT".into(), color: "proj-2".into(), repos: vec![], usecase: None, adopt: false, tracked: false };
    assert!(matches!(ws.create_project(&plain, "app"), Err(Error::Invalid(_))));
    assert!(!root.join("projects/other").exists());
}

#[test]
fn tracked_toggle_is_lossless_and_guarded() {
    let (_tmp, root) = fixture();
    let file = root.join("projects/invoice-extraction/_project.md");
    let before = fs::read_to_string(&file).unwrap();
    let wr8 = fs::read(root.join(WR8)).unwrap();
    let mut ws = Workspace::open(&root).unwrap();
    assert_eq!(ws.index.project("IE").unwrap().project.tracked, None);

    // Off: one line at the end of the frontmatter, the rest byte-identical.
    ws.update_project_field("IE", "tracked", &json!(false), "app").unwrap();
    let end = before.rfind("---\n").unwrap();
    assert_eq!(fs::read_to_string(&file).unwrap(), format!("{}tracked: false\n{}", &before[..end], &before[end..]));
    assert_eq!(ws.index.project("IE").unwrap().project.tracked, Some(false));
    let last = log_lines(&root).pop().unwrap();
    assert_eq!((&last["field"], &last["from"], &last["to"]), (&json!("tracked"), &Value::Null, &json!(false)));

    // While untracked, removing the use case is refused: it would vanish everywhere.
    assert!(matches!(ws.remove_usecase("IE", "app"), Err(Error::Invalid(_))));

    // On again removes the line: back to the original bytes; on when missing writes nothing.
    ws.update_project_field("IE", "tracked", &json!(true), "app").unwrap();
    assert_eq!(fs::read_to_string(&file).unwrap(), before);
    let lines = log_lines(&root).len();
    ws.update_project_field("IE", "tracked", &json!(true), "app").unwrap();
    assert_eq!(log_lines(&root).len(), lines);

    // A plain project cannot be untracked; a non-bool is refused.
    let wr = root.join("projects/website-relaunch/_project.md");
    let wr_before = fs::read(&wr).unwrap();
    assert!(matches!(ws.update_project_field("WR", "tracked", &json!(false), "app"), Err(Error::Invalid(_))));
    assert!(matches!(ws.update_project_field("IE", "tracked", &json!("no"), "app"), Err(Error::Invalid(_))));
    assert_eq!(fs::read(&wr).unwrap(), wr_before);
    assert_eq!(fs::read(root.join(WR8)).unwrap(), wr8);
}

#[test]
fn fixture_support_triage_is_untracked() {
    let (_tmp, root) = fixture();
    let ws = Workspace::open(&root).unwrap();
    assert_eq!(ws.index.project("ST").unwrap().project.tracked, Some(false));
}

// ----------------------------------------------------------- savings (P4)

#[test]
fn savings_write_is_lossless_and_logged() {
    let (_tmp, root) = fixture();
    let file = root.join("projects/invoice-extraction/_project.md");
    let before = fs::read_to_string(&file).unwrap();
    let mut ws = Workspace::open(&root).unwrap();
    let old = json!([
        { "what": "Capture invoice header", "count": 1200, "per": "month", "minutes": 6 },
        { "what": "Clarify queries with suppliers", "count": 80, "per": "month", "minutes": 15 },
    ]);
    // Unchanged row stays verbatim, the edited one is rewritten, the new one appended. Key order as sent.
    let new = json!([
        old[0],
        { "what": "Clarify queries with suppliers", "count": 80, "per": "month", "minutes": 20 },
        { "what": "Post, archive", "count": 2.5, "per": "day", "minutes": 30 },
    ]);
    ws.update_project_field("IE", "usecase.savings", &new, "app").unwrap();
    let want = before.replace(
        "    - { what: Clarify queries with suppliers, count: 80, per: month, minutes: 15 }\n",
        "    - { what: Clarify queries with suppliers, count: 80, per: month, minutes: 20 }\n    - { what: \"Post, archive\", count: 2.5, per: day, minutes: 30 }\n",
    );
    assert_eq!(fs::read_to_string(&file).unwrap(), want);
    let uc = ws.index.project("IE").unwrap().project.usecase.clone().unwrap();
    assert_eq!((uc.savings.len(), uc.savings[2].count, uc.savings[2].per.as_deref()), (3, Some(2.5), Some("day")));
    let last = log_lines(&root).pop().unwrap();
    assert_eq!((&last["kind"], &last["field"], &last["from"], &last["to"]), (&json!("project.update"), &json!("usecase.savings"), &old, &new));

    // A use case without savings gets the list appended to its block.
    let file = root.join("projects/support-triage/_project.md");
    let before = fs::read_to_string(&file).unwrap();
    ws.update_project_field("ST", "usecase.savings", &json!([{ "what": "Route ticket", "count": 300, "per": "week", "minutes": 2 }]), "app").unwrap();
    let want = before.replace(
        "    note: Ticket export only as a sample so far\n",
        "    note: Ticket export only as a sample so far\n  savings: [{ what: Route ticket, count: 300, per: week, minutes: 2 }]\n",
    );
    assert_eq!(fs::read_to_string(&file).unwrap(), want);
}

#[test]
fn savings_are_validated_on_write() {
    let (_tmp, root) = fixture();
    let file = root.join("projects/invoice-extraction/_project.md");
    let before = fs::read_to_string(&file).unwrap();
    let mut ws = Workspace::open(&root).unwrap();
    let row = |what: Value, count: Value, per: Value, minutes: Value| json!([{ "what": what, "count": count, "per": per, "minutes": minutes }]);
    for bad in [
        row(json!(" "), json!(1), json!("day"), json!(1)),
        row(json!("X"), json!(-1), json!("day"), json!(1)),
        row(json!("X"), json!("12"), json!("day"), json!(1)),
        row(json!("X"), json!(1), json!("quarter"), json!(1)),
        row(json!("X"), json!(1), json!("day"), Value::Null),
        json!("about 0.5 FTE"),
    ] {
        assert!(matches!(ws.update_project_field("IE", "usecase.savings", &bad, "app"), Err(Error::Invalid(_))), "{bad}");
    }
    assert_eq!(fs::read_to_string(&file).unwrap(), before);
    // Empty is fine; so is a zero.
    ws.update_project_field("IE", "usecase.savings", &row(json!("X"), json!(0), json!("year"), json!(0)), "app").unwrap();
    ws.update_project_field("IE", "usecase.savings", &json!([]), "app").unwrap();
    assert!(fs::read_to_string(&file).unwrap().contains("  savings: []\n"));
}

#[test]
fn hand_written_bad_savings_are_problems_not_breakage() {
    let (_tmp, root) = fixture();
    let ie = root.join("projects/invoice-extraction/_project.md");
    let src = fs::read_to_string(&ie).unwrap()
        .replace("count: 1200, per: month, minutes: 6 }", "count: 1200, per: month, minutes: 6, source: SAP }")
        .replace("count: 80, per: month", "count: lots, per: month");
    fs::write(&ie, src).unwrap();
    let st = root.join("projects/support-triage/_project.md");
    let src = fs::read_to_string(&st).unwrap().replace("  current_state: Need reported, no pilot yet\n", "  current_state: Need reported, no pilot yet\n  savings: about 0.5 FTE\n");
    fs::write(&st, src).unwrap();

    let ws = Workspace::open(&root).unwrap();
    let uc = ws.index.project("IE").unwrap().project.usecase.clone().unwrap();
    assert_eq!((uc.savings.len(), uc.savings[1].count, uc.savings[1].minutes), (2, None, Some(15.0)));
    // Unknown keys ride along, so rewriting the list keeps them.
    assert_eq!(serde_json::to_value(&uc.savings[0]).unwrap()["source"], json!("SAP"));
    assert!(ws.index.project("ST").unwrap().project.usecase.as_ref().unwrap().savings.is_empty());
    let msgs: Vec<(&str, &str)> = ws.index.errors.iter().map(|e| (e.path.as_str(), e.message.as_str())).filter(|(_, m)| m.contains("savings")).collect();
    assert_eq!(
        msgs,
        [
            ("projects/invoice-extraction/_project.md", "usecase.savings 2: count must be a number >= 0"),
            ("projects/support-triage/_project.md", "usecase.savings must be a list"),
        ]
    );
}

// ------------------------------------------------------------- assessment (A1)

const ST_ASSESSMENT: &str = "  assessment:\n    date: 2026-10-06\n    scores: { volume: 3, quality: 2, access: 1, maturity: 2 }\n    note: Ticket export only as a sample so far\n";

/// The template has no K.O. questions (ASSESSMENT.md D8); the mechanism stays, so tests add some back.
fn with_ko(root: &Path) {
    let p = root.join(".workly/process.yml");
    let src = fs::read_to_string(&p).unwrap();
    let ko = "assessment:\n  ko:\n    - { id: owner, label: Business owner named }\n    - { id: risk, label: Not high-risk }\n    - { id: data_use, label: Data may be used }\n";
    fs::write(&p, src.replacen("assessment:\n", ko, 1)).unwrap();
}

#[test]
fn fixture_assessments_load() {
    let ws = Workspace::open(FIXTURE).unwrap();
    let a = ws.index.project("ST").unwrap().project.usecase.as_ref().unwrap().assessment.clone().unwrap();
    assert_eq!((a.date.as_deref(), a.ko.len(), a.scores.len(), a.scores.get("integration")), (Some("2026-10-06"), 0, 4, None));
    // IE's volume comes from Manual effort (fte on the criterion), so it is not stored.
    let ie = ws.index.project("IE").unwrap().project.usecase.as_ref().unwrap().assessment.clone().unwrap();
    assert_eq!((ie.scores.len(), ie.scores.get("volume")), (5, None));
    let method = ws.process.as_ref().unwrap().assessment.as_ref().unwrap();
    assert_eq!((method.ko.len(), method.criteria.len()), (0, 6));
    assert_eq!(method.criteria[0].fte.as_deref(), Some(&[0.5, 2.0][..]));
    assert!(ws.process.as_ref().unwrap().validate().is_empty());
    assert!(ws.index.project("NA").unwrap().project.usecase.as_ref().unwrap().assessment.is_none());
}

#[test]
fn assessment_write_is_lossless_and_logged() {
    let (_tmp, root) = fixture();
    with_ko(&root);
    let file = root.join("projects/support-triage/_project.md");
    // Hand-written: comments, a block-style ko, an unknown key in the block.
    let hand = "  assessment:   # G0\n    date: 2026-10-06\n    ko:\n      owner: pass   # Mia\n      risk: open\n    scores: { volume: 3, quality: 2, access: 1, maturity: 2 }\n    source: workshop\n    note: Ticket export only as a sample so far\n";
    let before = fs::read_to_string(&file).unwrap().replace(ST_ASSESSMENT, hand);
    fs::write(&file, &before).unwrap();
    let wr8 = fs::read(root.join(WR8)).unwrap();
    let mut ws = Workspace::open(&root).unwrap();
    let n = log_lines(&root).len();
    let old = json!({ "date": "2026-10-06", "ko": { "owner": "pass", "risk": "open" }, "scores": { "volume": 3, "quality": 2, "access": 1, "maturity": 2 }, "source": "workshop", "note": "Ticket export only as a sample so far" });
    let new = json!({ "date": "2026-10-08", "ko": { "owner": "pass", "risk": "pass", "data_use": "pass" }, "scores": { "volume": 3, "quality": 2, "access": 2, "integration": 3, "maturity": 2 }, "note": "Output stays internal" });
    ws.update_project_field("ST", "usecase.assessment", &new, "app").unwrap();
    let want = before.replace(
        "    date: 2026-10-06\n    ko:\n      owner: pass   # Mia\n      risk: open\n    scores: { volume: 3, quality: 2, access: 1, maturity: 2 }\n",
        "    date: 2026-10-08\n    ko: { owner: pass, risk: pass, data_use: pass }\n    scores: { volume: 3, quality: 2, access: 2, integration: 3, maturity: 2 }\n",
    ).replace("    note: Ticket export only as a sample so far\n", "    note: Output stays internal\n");
    assert_eq!(fs::read_to_string(&file).unwrap(), want);
    let lines = log_lines(&root);
    assert_eq!(lines.len(), n + 1);
    let last = lines.last().unwrap();
    assert_eq!((&last["kind"], &last["field"], &last["from"], &last["to"]), (&json!("project.update"), &json!("usecase.assessment"), &old, &new));
    let a = ws.index.project("ST").unwrap().project.usecase.as_ref().unwrap().assessment.clone().unwrap();
    assert_eq!((a.ko["risk"].as_str(), a.scores["integration"], a.note.as_deref()), ("pass", 3, Some("Output stays internal")));

    // Same value again: no write, no log line.
    ws.update_project_field("ST", "usecase.assessment", &new, "app").unwrap();
    assert_eq!(log_lines(&root).len(), n + 1);

    // No note, nothing scored: those keys go; null removes the block, unknown key and all.
    let bare = json!({ "date": "2026-10-09", "ko": { "owner": "fail" }, "scores": {}, "note": null });
    ws.update_project_field("ST", "usecase.assessment", &bare, "app").unwrap();
    assert!(fs::read_to_string(&file).unwrap().contains("  assessment:   # G0\n    date: 2026-10-09\n    ko: { owner: fail }\n    source: workshop\n---"));
    ws.update_project_field("ST", "usecase.assessment", &Value::Null, "app").unwrap();
    let original = fs::read_to_string(Path::new(FIXTURE).join("projects/support-triage/_project.md")).unwrap();
    assert_eq!(fs::read_to_string(&file).unwrap(), original.replace(ST_ASSESSMENT, ""));
    assert_eq!(log_lines(&root).len(), n + 3);
    assert_eq!(fs::read(root.join(WR8)).unwrap(), wr8);
}

#[test]
fn assessment_appended_to_use_case_without_one() {
    let (_tmp, root) = fixture();
    let file = root.join("projects/newsletter-automation/_project.md");
    let before = fs::read_to_string(&file).unwrap();
    let mut ws = Workspace::open(&root).unwrap();
    // No K.O. questions in the method: an empty ko map is not written.
    let value = json!({ "date": "2026-10-08", "ko": {}, "scores": { "volume": 1 }, "note": "Rare: 2 newsletters a month" });
    ws.update_project_field("NA", "usecase.assessment", &value, "app").unwrap();
    let want = before.replace(
        "Demand needed: touches CRM\" }\n",
        "Demand needed: touches CRM\" }\n  assessment:\n    date: 2026-10-08\n    scores: { volume: 1 }\n    note: \"Rare: 2 newsletters a month\"\n",
    );
    assert_eq!(fs::read_to_string(&file).unwrap(), want);
    // A hand-written flow-style block is replaced whole.
    let flow = fs::read_to_string(&file).unwrap().replace(&want[want.find("  assessment:").unwrap()..want.find("---\nDraft").unwrap()], "  assessment: { date: 2026-10-01, scores: { volume: 9 } }\n");
    fs::write(&file, flow).unwrap();
    ws.rescan();
    ws.update_project_field("NA", "usecase.assessment", &value, "app").unwrap();
    assert_eq!(fs::read_to_string(&file).unwrap(), want);
}

#[test]
fn assessment_is_validated_on_write() {
    let (_tmp, root) = fixture();
    with_ko(&root);
    let file = root.join("projects/support-triage/_project.md");
    let before = fs::read_to_string(&file).unwrap();
    let mut ws = Workspace::open(&root).unwrap();
    let n = log_lines(&root).len();
    let err = |ws: &mut Workspace, key: &str, v: Value| match ws.update_project_field(key, "usecase.assessment", &v, "app") {
        Err(Error::Invalid(m)) => m,
        other => panic!("{v}: {other:?}"),
    };
    let cases = [
        (json!({ "date": "2026-10-08", "ko": { "owner": "yes" } }), "usecase.assessment: ko.owner must be pass, fail or open, got \"yes\""),
        (json!({ "date": "2026-10-08", "scores": { "access": 4 } }), "usecase.assessment: scores.access must be 1, 2 or 3, got 4"),
        (json!({ "date": "2026-10-08", "scores": { "access": 2.5 } }), "usecase.assessment: scores.access must be 1, 2 or 3, got 2.5"),
        (json!({ "date": "2026-10-08", "scores": { "access": "2" } }), "usecase.assessment: scores.access must be 1, 2 or 3, got \"2\""),
        (json!({ "date": "2026-10-08", "scores": { "cost": 2 } }), "usecase.assessment: unknown criterion 'cost' (not in process.yml)"),
        (json!({ "date": "2026-10-08", "ko": { "budget": "pass" } }), "usecase.assessment: unknown K.O. question 'budget' (not in process.yml)"),
        (json!({ "date": "2026-10-08", "ko": ["owner"] }), "usecase.assessment: ko must be a map"),
        (json!({ "date": "2026-10-08", "note": 3 }), "usecase.assessment: note must be a text"),
        (json!({ "date": "8.10.2026" }), "usecase.assessment: date must be a date (YYYY-MM-DD), got \"8.10.2026\""),
        (json!({ "ko": {} }), "usecase.assessment: date is required"),
        (json!({ "date": "2026-10-08", "value": 2.3 }), "usecase.assessment: unknown key 'value'"),
        (json!("quick win"), "usecase.assessment must be a map"),
    ];
    for (v, msg) in cases {
        assert_eq!(err(&mut ws, "ST", v), msg);
    }
    assert_eq!(err(&mut ws, "WR", json!({ "date": "2026-10-08" })), "WR is not a use case");
    // Without the method in process.yml there is nothing to score against.
    let p = root.join(".workly/process.yml");
    let src = fs::read_to_string(&p).unwrap();
    fs::write(&p, &src[..src.find("\n# Use-case assessment").unwrap()]).unwrap();
    ws.rescan();
    assert_eq!(err(&mut ws, "ST", json!({ "date": "2026-10-08" })), "process.yml has no assessment block");
    assert_eq!(fs::read_to_string(&file).unwrap(), before);
    assert_eq!(log_lines(&root).len(), n);
}

#[test]
fn hand_written_bad_assessment_is_problems_not_breakage() {
    let (_tmp, root) = fixture();
    let st = root.join("projects/support-triage/_project.md");
    let src = fs::read_to_string(&st).unwrap()
        .replace("    scores: { volume: 3, quality: 2,", "    ko: { budget: pass }\n    scores: { volume: 5, quality: 2, cost: 1,");
    fs::write(&st, src).unwrap();
    let ie = root.join("projects/invoice-extraction/_project.md");
    let src = fs::read_to_string(&ie).unwrap().replace("    date: 2026-09-08\n", "    date: 2026-09-08\n    note: [a, b]\n").replace("    note: Header fields only, line items later\n", "");
    fs::write(&ie, src).unwrap();
    let na = root.join("projects/newsletter-automation/_project.md");
    let src = fs::read_to_string(&na).unwrap().replace("  current_state:", "  assessment: quick win\n  current_state:");
    fs::write(&na, src).unwrap();

    let ws = Workspace::open(&root).unwrap();
    let a = ws.index.project("ST").unwrap().project.usecase.as_ref().unwrap().assessment.clone().unwrap();
    // Bad values load as open; unknown ids ride along (the app ignores them).
    assert_eq!((a.ko.get("budget").map(String::as_str), a.scores.get("volume"), a.scores.get("cost")), (Some("pass"), None, Some(&1)));
    assert_eq!(ws.index.project("IE").unwrap().project.usecase.as_ref().unwrap().assessment.as_ref().unwrap().note, None);
    let na_uc = ws.index.project("NA").unwrap().project.usecase.clone().unwrap();
    assert!(na_uc.assessment.is_none() && na_uc.status.as_deref() == Some("blocked"));
    let msgs: Vec<(&str, &str)> = ws.index.errors.iter().map(|e| (e.path.as_str(), e.message.as_str())).filter(|(_, m)| m.contains("assessment")).collect();
    assert_eq!(
        msgs,
        [
            ("projects/invoice-extraction/_project.md", "usecase.assessment: note must be a text"),
            ("projects/newsletter-automation/_project.md", "usecase.assessment must be a map"),
            ("projects/support-triage/_project.md", "usecase.assessment: unknown K.O. question 'budget' (not in process.yml)"),
            ("projects/support-triage/_project.md", "usecase.assessment: scores.volume must be 1, 2 or 3, got 5"),
            ("projects/support-triage/_project.md", "usecase.assessment: unknown criterion 'cost' (not in process.yml)"),
        ]
    );
}

// ------------------------------------------------------------------ agents (M5)

#[test]
fn start_and_review_are_lossless_and_logged() {
    let (_tmp, root) = fixture();
    let before = fs::read_to_string(root.join(WR8)).unwrap();
    let mut ws = Workspace::open(&root).unwrap();
    ws.clear_agent("WR-3", "app").unwrap(); // WR-3's agent works in the same repo
    let n = log_lines(&root).len();
    ws.start_task("WR-8", Some("codex"), "agent:codex").unwrap();
    let after = fs::read_to_string(root.join(WR8)).unwrap();
    // Only status changes in place; the new agent block is appended to the frontmatter.
    let (head, tail) = after.split_once("agent:\n").unwrap();
    assert_eq!(head, before.split_once("---\nBody").unwrap().0.replace("status: backlog\n", "status: doing\n"));
    assert!(tail.starts_with("  active: codex\n  since: 20") && tail.contains("---\nBody with trailing spaces   \n"), "{tail}");
    let a = ws.index.task("WR-8").unwrap().task.agent.clone().unwrap();
    assert_eq!(a.active.as_deref(), Some("codex"));
    let lines = log_lines(&root);
    assert_eq!(lines.len(), n + 3);
    assert!(lines[n..].iter().all(|l| l["actor"] == "agent:codex" && l["id"] == "WR-8"));

    ws.review_task("WR-8", Some("A3F9C1E"), None, "agent:codex").unwrap();
    let t = ws.index.task("WR-8").unwrap().task.clone();
    let a = t.agent.unwrap();
    assert_eq!((t.status.as_str(), a.active, a.since, a.commit.as_deref()), ("review", None, None, Some("a3f9c1e")));
    assert!(a.last_run.is_some());
    let reviewed = fs::read_to_string(root.join(WR8)).unwrap();
    assert!(reviewed.ends_with("- [x] list b") && reviewed.contains("# Kept by hand, do not reorder\n"));
}

#[test]
fn review_summary_is_one_updates_item() {
    let (_tmp, root) = fixture();
    let mut ws = Workspace::open(&root).unwrap();
    // Without an agent name start changes only the status, so review does too.
    ws.start_task("WR-8", None, "agent:codex").unwrap();
    let before = fs::read_to_string(root.join(WR8)).unwrap();
    let n = log_lines(&root).len();
    let summary = "Migrated 12 pages\n\n   Verify: open /blog  \nOpen: legacy URLs";
    ws.review_task("WR-8", None, Some(summary), "agent:codex").unwrap();
    let after = fs::read_to_string(root.join(WR8)).unwrap();
    let stamp = after.split("## Updates\n- ").nth(1).unwrap().split(" · ").next().unwrap();
    assert_eq!(stamp.len(), 16, "{stamp}");
    let expected = before.replace("status: doing\n", "status: review\n")
        + &format!("\n\n## Updates\n- {stamp} · codex: Migrated 12 pages\n  Verify: open /blog\n  Open: legacy URLs\n");
    assert_eq!(after, expected);
    let lines = log_lines(&root);
    assert_eq!(lines.len(), n + 2);
    assert_eq!((&lines[n + 1]["kind"], &lines[n + 1]["field"]), (&json!("task.summary"), &json!("updates")));
    // An empty summary writes nothing.
    ws.start_task("WR-8", None, "cli").unwrap();
    let doing = fs::read_to_string(root.join(WR8)).unwrap();
    assert!(matches!(ws.review_task("WR-8", None, Some(" \n "), "cli"), Err(Error::Invalid(_))));
    assert_eq!(fs::read_to_string(root.join(WR8)).unwrap(), doing);
}

#[test]
fn run_notes_leave_the_task_file_alone() {
    let (_tmp, root) = fixture();
    let mut ws = Workspace::open(&root).unwrap();
    ws.clear_agent("WR-3", "app").unwrap(); // WR-3's agent works in the same repo
    ws.start_task("WR-8", Some("codex"), "agent:codex").unwrap();
    let before = fs::read_to_string(root.join(WR8)).unwrap();
    let n = log_lines(&root).len();
    let since = ws.index.task("WR-8").unwrap().task.agent.clone().unwrap().since.unwrap();

    let e = ws.add_run_note("WR-8", "Migrated\n 12 pages", "agent:codex").unwrap();
    assert_eq!((e.agent.as_str(), e.run.as_deref(), e.text.as_str()), ("codex", Some(since.as_str()), "Migrated 12 pages"));
    assert_eq!(fs::read_to_string(root.join(WR8)).unwrap(), before);
    assert_eq!(log_lines(&root).len(), n, "the run log is its own record");
    let src = fs::read_to_string(root.join(".workly/runs/WR-8/log.jsonl")).unwrap();
    assert_eq!(src.lines().count(), 1);
    assert_eq!(ws.run_log("WR-8").unwrap(), [e]);

    assert!(matches!(ws.add_run_note("WR-8", " \n ", "cli"), Err(Error::Invalid(_))));
    assert!(matches!(ws.add_run_note("XX-1", "x", "cli"), Err(Error::NotFound(_))));
    assert!(ws.run_log("WR-5").unwrap().is_empty());
}

#[test]
fn last_activity_of_the_current_run() {
    let (_tmp, root) = fixture();
    let mut ws = Workspace::open(&root).unwrap();
    // WR-3: codex active since 2026-10-06T07:48:00Z, two notes in the fixture run log.
    assert_eq!(ws.last_activity("WR-3").unwrap().as_deref(), Some("2026-10-06T08:20:00Z"));
    assert_eq!(ws.last_activity("WR-7").unwrap(), None);
    // Without a run log: agent.since.
    fs::remove_file(root.join(".workly/runs/WR-3/log.jsonl")).unwrap();
    assert_eq!(ws.last_activity("WR-3").unwrap().as_deref(), Some("2026-10-06T07:48:00Z"));
    // Entries of another run and broken lines do not count.
    let dir = root.join(".workly/runs/WR-3");
    fs::create_dir_all(&dir).unwrap();
    fs::write(
        dir.join("log.jsonl"),
        concat!(
            r#"{"ts":"2026-10-06T08:10:00Z","agent":"codex","run":"2026-10-06T07:48:00Z","text":"a"}"#, "\n",
            r#"{"ts":"2026-10-07T09:00:00Z","agent":"codex","run":"2026-10-07T08:00:00Z","text":"old run"}"#, "\n",
            "not json\n",
        ),
    )
    .unwrap();
    assert_eq!(ws.last_activity("WR-3").unwrap().as_deref(), Some("2026-10-06T08:10:00Z"));
    assert_eq!(ws.run_log("WR-3").unwrap().len(), 2);
    // No agent active: no activity.
    assert_eq!(ws.last_activity("WR-5").unwrap(), None);
    assert_eq!(ws.agent_activity().into_iter().collect::<Vec<_>>(), [("WR-3".to_string(), "2026-10-06T08:10:00Z".to_string())]);
    ws.review_task("WR-3", None, None, "agent:codex").unwrap();
    assert_eq!(ws.last_activity("WR-3").unwrap(), None);
    assert!(ws.agent_activity().is_empty());
}

#[test]
fn clear_agent_keeps_the_status() {
    let (_tmp, root) = fixture();
    let mut ws = Workspace::open(&root).unwrap();
    assert_eq!(ws.config.agent_defaults.stale_after_minutes, 30);
    ws.clear_agent("WR-3", "app").unwrap(); // WR-3's agent works in the same repo
    ws.start_task("WR-8", Some("codex"), "agent:codex").unwrap();
    let before = fs::read_to_string(root.join(WR8)).unwrap();
    let n = log_lines(&root).len();
    ws.clear_agent("WR-8", "app").unwrap();
    let after = fs::read_to_string(root.join(WR8)).unwrap();
    let since = before.split("  since: ").nth(1).unwrap().lines().next().unwrap();
    assert_eq!(after, before.replace("  active: codex\n", "  active: null\n").replace(&format!("  since: {since}\n"), "  since: null\n"));
    let t = ws.index.task("WR-8").unwrap().task.clone();
    assert_eq!((t.status.as_str(), t.agent.unwrap().active), ("doing", None));
    let lines = log_lines(&root);
    assert_eq!(lines.len(), n + 2);
    assert!(lines[n..].iter().all(|l| l["actor"] == "app" && l["kind"] == "task.update"));
    // Nothing active: nothing written.
    ws.clear_agent("WR-8", "app").unwrap();
    assert_eq!(log_lines(&root).len(), n + 2);
    assert!(matches!(ws.clear_agent("XX-1", "app"), Err(Error::NotFound(_))));
}

#[test]
fn run_log_needs_a_plain_id() {
    let (_tmp, root) = fixture();
    let file = root.join("inbox/IN-1-sketch.md");
    let src = fs::read_dir(root.join("inbox")).unwrap().flatten().find(|e| e.file_name().to_string_lossy().starts_with("IN-1-")).unwrap().path();
    let text = fs::read_to_string(&src).unwrap().replacen("id: IN-1", "id: IN-1/../../x", 1);
    fs::remove_file(&src).unwrap();
    fs::write(&file, text).unwrap();
    let ws = Workspace::open(&root).unwrap();
    assert!(matches!(ws.add_run_note("IN-1/../../x", "x", "cli"), Err(Error::Invalid(_))));
    assert!(!root.join(".workly/x").exists());
}

#[test]
fn transition_rules() {
    let (_tmp, root) = fixture();
    let mut ws = Workspace::open(&root).unwrap();
    // WR-3 is doing without an agent name, WR-1 done: no start. WR-5 is todo: no review.
    assert!(matches!(ws.start_task("WR-3", None, "cli"), Err(Error::InvalidTransition(_))));
    assert!(matches!(ws.start_task("WR-1", None, "cli"), Err(Error::InvalidTransition(_))));
    assert!(matches!(ws.review_task("WR-5", None, None, "cli"), Err(Error::InvalidTransition(_))));
    assert!(matches!(ws.start_task("XX-1", None, "cli"), Err(Error::NotFound(_))));
    assert!(matches!(ws.start_task("WR-5", Some("a b"), "cli"), Err(Error::Invalid(_))));
    assert!(matches!(ws.review_task("WR-3", Some("not-a-sha"), None, "cli"), Err(Error::Invalid(_))));
    // Without an agent name only the status changes; review from review restarts.
    ws.start_task("WR-7", None, "cli").unwrap();
    let t = &ws.index.task("WR-7").unwrap().task;
    assert_eq!((t.status.as_str(), t.agent.as_ref().unwrap().active.as_deref()), ("doing", None));
    ws.review_task("WR-3", None, None, "agent:codex").unwrap();
    assert_eq!(ws.index.task("WR-3").unwrap().task.agent.as_ref().unwrap().active, None);
}

#[test]
fn one_agent_per_repo() {
    let (_tmp, root) = fixture();
    let mut ws = Workspace::open(&root).unwrap();
    // WR-3: codex active in ~/repos/website, the first repo of website-relaunch.
    let before = fs::read_to_string(root.join(WR8)).unwrap();
    let Err(Error::InvalidTransition(msg)) = ws.start_task("WR-8", Some("copilot"), "app") else { panic!("not refused") };
    assert_eq!(msg, "WR-3 already has an agent working in ~/repos/website");
    assert_eq!(fs::read_to_string(root.join(WR8)).unwrap(), before);
    // Without an agent there is no check; restarting WR-3 itself is fine.
    ws.start_task("WR-5", None, "cli").unwrap();
    ws.start_task("WR-3", Some("copilot"), "app").unwrap();
    // Another repo is not affected.
    ws.start_task("IE-2", Some("codex"), "app").unwrap();
    // Free again after review.
    ws.review_task("WR-3", None, None, "app").unwrap();
    ws.start_task("WR-8", Some("copilot"), "app").unwrap();
}

#[test]
fn start_from_doing_sets_only_the_agent() {
    let (_tmp, root) = fixture();
    let mut ws = Workspace::open(&root).unwrap();
    ws.clear_agent("WR-3", "app").unwrap(); // WR-3's agent works in the same repo
    ws.start_task("WR-8", None, "cli").unwrap();
    let before = fs::read_to_string(root.join(WR8)).unwrap();
    let n = log_lines(&root).len();
    ws.start_task("WR-8", Some("copilot"), "app").unwrap();
    let after = fs::read_to_string(root.join(WR8)).unwrap();
    // The status line stays, the agent block is appended to the frontmatter, the body stays.
    let (head, tail) = after.split_once("agent:\n").unwrap();
    assert_eq!(head, before.split_once("---\nBody").unwrap().0);
    assert!(tail.starts_with("  active: copilot\n  since: 20") && after.ends_with(before.split_once("---\nBody").unwrap().1), "{tail}");
    assert_eq!(log_lines(&root).len(), n + 2);
}

#[test]
fn codex_start_script_runs_with_quoted_paths() {
    use std::os::unix::fs::PermissionsExt;
    use std::process::Command;
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path().join("it's a ws");
    copy_dir(Path::new(FIXTURE), &root);
    let repo = dir.path().join("my 'repo' $HOME");
    fs::create_dir_all(&repo).unwrap();
    let edit = |rel: &str, from: &str, to: &str| {
        let f = root.join(rel);
        let src = fs::read_to_string(&f).unwrap();
        assert!(src.contains(from), "{rel}");
        fs::write(&f, src.replacen(from, to, 1)).unwrap();
    };
    edit("projects/website-relaunch/_project.md", "repos: [~/repos/website]", &format!("repos: [\"{}\"]", repo.display()));
    edit("projects/website-relaunch/tasks/WR-5-navigation.md", "  model: auto\n", "  model: gpt-5\n");
    let ws = Workspace::open(&root).unwrap();
    let prompt = "Work on WR-5.\nIt's \"quoted\", $(not run) and `nor this`.";
    let script = ws.write_codex_start("WR-5", prompt).unwrap();
    assert_eq!(script, ws.root().join(".workly/runs/WR-5/start.command"));
    assert_eq!(fs::read_to_string(ws.root().join(".workly/runs/WR-5/prompt.md")).unwrap(), prompt);
    assert!(!fs::read_to_string(&script).unwrap().contains("quoted"), "the prompt never goes into the script");
    assert_eq!(fs::metadata(&script).unwrap().permissions().mode() & 0o777, 0o700);

    // A fake codex prints its arguments, the folder and the env.
    let fake = "#!/bin/sh\nfor a in \"$@\"; do printf '[%s]\\n' \"$a\"; done\npwd\necho \"$WORKLY_WORKSPACE|$WORKLY_AGENT\"\n";
    let install = |dir: &Path| {
        fs::create_dir_all(dir).unwrap();
        fs::write(dir.join("codex"), fake).unwrap();
        fs::set_permissions(dir.join("codex"), fs::Permissions::from_mode(0o755)).unwrap();
    };
    let run = |path: &str, home: &Path| Command::new("/bin/sh").arg(&script).env("PATH", path).env("HOME", home).output().unwrap();
    let expected = format!("[-m]\n[gpt-5]\n[-c]\n[model_reasoning_effort=high]\n[{prompt}]\n{}\n{}|codex\n", repo.display(), ws.root().display());

    // From PATH.
    let bin = dir.path().join("bin");
    install(&bin);
    let empty = dir.path().join("empty home");
    fs::create_dir_all(&empty).unwrap();
    let out = run(&format!("{}:/usr/bin:/bin", bin.display()), &empty);
    assert_eq!(String::from_utf8_lossy(&out.stdout), expected);
    // Not on PATH: the VS Code extension's binary.
    let home = dir.path().join("home");
    install(&home.join(".vscode/extensions/openai.chatgpt-26.1007.1-darwin-arm64/bin/macos-aarch64"));
    let out = run("/usr/bin:/bin", &home);
    assert_eq!(String::from_utf8_lossy(&out.stdout), expected);
    // Neither: a readable message and exit 1.
    let out = run("/usr/bin:/bin", &empty);
    assert_eq!(out.status.code(), Some(1));
    assert!(String::from_utf8_lossy(&out.stdout).starts_with("Codex not found."));

    // Defaults: auto everywhere = no flags. No repo = refused.
    edit("projects/website-relaunch/tasks/WR-5-navigation.md", "  model: gpt-5\n  effort: high\n", "  model: auto\n  effort: auto\n");
    edit(".workly/config.yml", "  effort: medium\n", "  effort: auto\n");
    let ws = Workspace::open(&root).unwrap();
    ws.write_codex_start("WR-5", prompt).unwrap();
    let out = run(&format!("{}:/usr/bin:/bin", bin.display()), &empty);
    assert!(String::from_utf8_lossy(&out.stdout).starts_with(&format!("[{prompt}]\n")));
    assert!(matches!(ws.write_codex_start("NA-1", prompt), Err(Error::Invalid(_))));
}

#[test]
fn agent_context_and_agents_md() {
    let (_tmp, root) = fixture();
    let mut ws = Workspace::open(&root).unwrap();
    let ctx = ws.agent_context(Some("WR")).unwrap();
    assert!(ctx.global.unwrap().content.starts_with("# Global agent rules"));
    assert_eq!(ctx.project.unwrap().path, "projects/website-relaunch/agent/AGENTS.md");
    assert_eq!(ctx.skills.len(), 1);
    let s = &ctx.skills[0];
    assert_eq!((s.name.as_str(), s.path.as_str()), ("release-check", "projects/website-relaunch/agent/skills/release-check/SKILL.md"));
    assert!(s.description.starts_with("Checks a build"));
    // Inbox task: global rules only.
    let inbox = ws.agent_context(None).unwrap();
    assert!(inbox.global.is_some() && inbox.project.is_none() && inbox.skills.is_empty());

    let ie = ws.agent_context(Some("IE")).unwrap();
    assert!(ie.project.is_none() && ie.skills.is_empty());
    let path = ws.create_agents_md("IE", "app").unwrap();
    assert_eq!(path, "projects/invoice-extraction/agent/AGENTS.md");
    assert!(fs::read_to_string(root.join(&path)).unwrap().starts_with("# Invoice Extraction"));
    assert!(matches!(ws.create_agents_md("IE", "app"), Err(Error::Invalid(_))));
    assert_eq!(log_lines(&root).pop().unwrap()["kind"], "project.agents_md");
}

#[test]
fn trash_lists_tasks_and_project_folders() {
    let (_tmp, root) = fixture();
    let mut ws = Workspace::open(&root).unwrap();
    let items = ws.trash_items();
    assert_eq!(items.len(), 1);
    assert_eq!((items[0].kind, items[0].id.as_str()), ("task", "WR-9"));

    ws.delete_project("OI", "app").unwrap();
    ws.delete_task("WR-8", "app").unwrap();
    let items = ws.trash_items();
    let kinds: Vec<(&str, &str)> = items.iter().map(|i| (i.kind, i.id.as_str())).collect();
    assert_eq!(kinds.len(), 3, "{kinds:?}");
    assert_eq!(kinds[0], ("task", "WR-8"), "newest deletion first");
    assert!(kinds.contains(&("project", "OI")), "a project folder is one item, not its files");
    assert!(ws.index.trash.iter().any(|p| p.ends_with("OI-1-legacy-export.md") || p.contains("old-intranet/tasks/")));
}

#[test]
fn restore_project_folder_and_refuse_orphan_task() {
    let (_tmp, root) = fixture();
    let mut ws = Workspace::open(&root).unwrap();
    let task = ws.index.task("OI-1").unwrap().path.clone();
    ws.delete_task("OI-1", "app").unwrap();
    ws.delete_project("OI", "app").unwrap();
    // Its project folder is gone, so the task cannot go back on its own.
    assert!(matches!(ws.restore(&task, "app"), Err(Error::Invalid(_))));

    ws.restore("projects/archive/old-intranet", "app").unwrap();
    assert!(ws.index.project("OI").is_some());
    assert_eq!(log_lines(&root).pop().unwrap()["kind"], "project.restore");
    ws.restore(&task, "app").unwrap();
    assert!(ws.index.task("OI-1").is_some());
    assert_eq!(ws.trash_items().len(), 1, "only WR-9 left");
}

#[test]
fn ids_stay_taken_after_the_trash_is_gone() {
    let (_tmp, root) = fixture();
    let mut ws = Workspace::open(&root).unwrap();
    let id = ws.create_task("Short lived", Some("WR"), None, "app").unwrap();
    assert_eq!(id, "WR-10");
    ws.delete_task(&id, "app").unwrap();
    // As after emptying the trash: the file is gone, the log remembers.
    fs::remove_dir_all(root.join(".workly/trash/projects")).unwrap();
    ws.rescan();
    assert_eq!(ws.next_id("WR"), "WR-11");
}

#[test]
fn missing_repos_skip_archived_and_existing() {
    let (tmp, root) = fixture();
    let mut ws = Workspace::open(&root).unwrap();
    let repo = tmp.path().join("repo");
    fs::create_dir(&repo).unwrap();
    ws.update_project_field("WR", "repos", &json!([repo.display().to_string(), "~/no/such/repo"]), "app").unwrap();
    let missing: Vec<(String, String)> = ws.missing_repos().into_iter().map(|m| (m.key, m.repo)).collect();
    assert!(missing.contains(&("WR".into(), "~/no/such/repo".into())));
    assert!(!missing.iter().any(|(_, r)| r == &repo.display().to_string()));
    assert!(!missing.iter().any(|(k, _)| k == "OI"));
}

#[test]
fn usecase_history_from_the_log() {
    let (_dir, root) = fixture();
    // Without log lines: the current phase from step_since, the decision placed by date.
    let ws = Workspace::open(&root).unwrap();
    let h = serde_json::to_value(ws.usecase_history("IE").unwrap()).unwrap();
    assert_eq!(h[0]["phase"], "pilot");
    assert_eq!(h[0]["start"], "2026-09-24");
    assert_eq!(h[0]["end"], Value::Null);
    assert_eq!(h[0]["events"][0]["kind"], "decision");

    // A removed and re-made use case starts over; other ids are ignored.
    let lines = [
        json!({"ts":"2026-08-01T08:00:00Z","kind":"usecase.create","id":"IE","field":"usecase","from":null,"to":{"step":"pilot","status":"active"}}),
        json!({"ts":"2026-08-20T08:00:00Z","kind":"usecase.remove","id":"IE","field":"usecase","from":{},"to":null}),
        json!({"ts":"2026-09-01T08:00:00Z","kind":"usecase.create","id":"IE","field":"usecase","from":null,"to":{"step":"need","status":"active"}}),
        json!({"ts":"2026-09-03T08:00:00Z","kind":"project.update","id":"IE","field":"usecase.blocked_by","from":null,"to":"numbers missing"}),
        json!({"ts":"2026-09-03T08:01:00Z","kind":"project.update","id":"IE","field":"usecase.status","from":"active","to":"blocked"}),
        json!({"ts":"2026-09-07T08:00:00Z","kind":"project.update","id":"IE","field":"usecase.status","from":"blocked","to":"active"}),
        json!({"ts":"2026-09-08T08:00:00Z","kind":"project.update","id":"WR","field":"usecase.status","from":"active","to":"blocked"}),
        json!({"ts":"2026-09-24T08:00:00Z","kind":"usecase.move","id":"IE","field":"usecase.step","from":"need","to":"pilot"}),
        json!({"ts":"2026-09-26T08:00:00Z","kind":"project.update","id":"IE","field":"usecase.status","from":"active","to":"waiting"}),
    ];
    let text: String = lines.iter().map(|l| format!("{l}\n")).collect();
    fs::write(root.join(".workly/log/2026-09.jsonl"), text + "not json\n").unwrap();
    let h = serde_json::to_value(ws.usecase_history("IE").unwrap()).unwrap();
    let phases: Vec<&str> = h.as_array().unwrap().iter().map(|s| s["phase"].as_str().unwrap()).collect();
    assert_eq!(phases, ["idea", "pilot"]);
    assert_eq!((&h[0]["start"], &h[0]["end"]), (&json!("2026-09-01"), &json!("2026-09-24")));
    let idea = &h[0]["events"];
    assert_eq!(idea[0]["status"], "active");
    assert_eq!((&idea[1]["status"], &idea[1]["until"], &idea[1]["text"]), (&json!("blocked"), &json!("2026-09-07"), &json!("numbers missing")));
    // The 2026-09-10 decision has no gate: it lands in the phase it was in on that day.
    assert_eq!(idea[3]["kind"], "decision");
    let pilot = &h[1];
    assert_eq!((&pilot["start"], &pilot["end"]), (&json!("2026-09-24"), &Value::Null));
    assert_eq!((&pilot["events"][0]["status"], &pilot["events"][0]["until"]), (&json!("waiting"), &Value::Null));
}

#[test]
fn add_waiting_column_is_a_one_line_diff() {
    let (_tmp, root) = fixture();
    let path = root.join(".workly/config.yml");
    let with = fs::read_to_string(&path).unwrap();
    let without = with.replace("  - { id: waiting, label: Waiting }\n", "");
    assert_ne!(with, without);
    fs::write(&path, &without).unwrap();
    let mut ws = Workspace::open(&root).unwrap();
    assert!(!ws.config.task_statuses.iter().any(|s| s.id == "waiting"));
    ws.add_waiting_status("app").unwrap();
    assert_eq!(fs::read_to_string(&path).unwrap(), with);
    assert_eq!(ws.config.task_statuses[3].id, "waiting");
    let last = log_lines(&root).pop().unwrap();
    assert_eq!((&last["kind"], &last["field"]), (&json!("config.update"), &json!("task_statuses")));
    assert_eq!(last["to"], json!(["backlog", "todo", "doing", "waiting", "review", "done"]));
    // Twice: nothing happens.
    let n = log_lines(&root).len();
    ws.add_waiting_status("app").unwrap();
    assert_eq!(log_lines(&root).len(), n);
    // A shape it cannot edit: refused, file untouched.
    let flow = "task_statuses: [{ id: todo, label: To do }, { id: done, label: Done }]\n";
    fs::write(&path, flow).unwrap();
    ws.rescan();
    assert!(matches!(ws.add_waiting_status("app"), Err(Error::Invalid(_))));
    assert_eq!(fs::read_to_string(&path).unwrap(), flow);
}
