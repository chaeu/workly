//! Watcher: external edits arrive fast, own writes are suppressed.

use serde_json::json;
use std::fs;
use std::path::Path;
use std::sync::mpsc;
use std::time::{Duration, Instant};
use workly_core::Workspace;
use workly_core::watch::Change;

const FIXTURE: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/../../fixtures/workspace");

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

fn drain(rx: &mpsc::Receiver<Vec<Change>>, wait: Duration) -> Vec<Change> {
    let mut all = Vec::new();
    while let Ok(batch) = rx.recv_timeout(wait) {
        all.extend(batch);
    }
    all
}

#[test]
fn external_changes_arrive_own_writes_do_not() {
    let tmp = tempfile::tempdir().unwrap();
    let root = tmp.path().join("ws");
    copy_dir(Path::new(FIXTURE), &root);
    let mut ws = Workspace::open(&root).unwrap();
    let (tx, rx) = mpsc::channel();
    let _watcher = ws.watch(move |changes| tx.send(changes).unwrap()).unwrap();
    drain(&rx, Duration::from_millis(400)); // settle events from the copy

    // External edit, as another editor would do it.
    let file = ws.root().join("projects/website-relaunch/tasks/WR-1-setup-repo.md");
    let start = Instant::now();
    fs::write(&file, fs::read_to_string(&file).unwrap().replace("Done.", "Done!")).unwrap();
    let batch = rx.recv_timeout(Duration::from_secs(1)).expect("no event within 1 s");
    eprintln!("external change seen after {:?}", start.elapsed());
    assert_eq!(batch, [Change::Changed("projects/website-relaunch/tasks/WR-1-setup-repo.md".into())]);

    // Own writes of every kind: nothing comes back.
    ws.update_task_field("WR-8", "status", &json!("doing"), "app").unwrap();
    ws.create_task("Fresh", Some("WR"), "app").unwrap();
    ws.move_task("WR-5", Some("IE"), "app").unwrap();
    ws.delete_task("WR-7", "app").unwrap();
    ws.restore("projects/website-relaunch/tasks/WR-7-lighthouse-audit.md", "app").unwrap();
    assert_eq!(drain(&rx, Duration::from_millis(600)), []);

    // Deleting a file outside the app is reported as removal.
    fs::remove_file(ws.root().join("inbox/IN-1-workshop-date.md")).unwrap();
    let batch = rx.recv_timeout(Duration::from_secs(1)).expect("no removal event");
    assert!(batch.contains(&Change::Removed("inbox/IN-1-workshop-date.md".into())), "{batch:?}");

    // A file we wrote earlier still reports external edits.
    let wr8 = ws.root().join("projects/website-relaunch/tasks/WR-8-content-migration.md");
    fs::write(&wr8, fs::read_to_string(&wr8).unwrap().replace("status: doing", "status: review")).unwrap();
    let batch = rx.recv_timeout(Duration::from_secs(1)).expect("external edit after own write not seen");
    assert!(batch.contains(&Change::Changed("projects/website-relaunch/tasks/WR-8-content-migration.md".into())), "{batch:?}");
}
