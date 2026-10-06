//! Golden and property tests for the lossless patcher.

use proptest::prelude::*;
use serde_json::{Value, json};
use workly_core::patch::{emit, get_field, remove_field, set_field};

const WR8: &str = include_str!("../../../fixtures/workspace/projects/website-relaunch/tasks/WR-8-content-migration.md");

/// The single changed region between two texts: (removed lines, added lines).
fn diff(a: &str, b: &str) -> (Vec<String>, Vec<String>) {
    let a: Vec<&str> = a.split_inclusive('\n').collect();
    let b: Vec<&str> = b.split_inclusive('\n').collect();
    let pre = a.iter().zip(&b).take_while(|(x, y)| x == y).count();
    let max_suf = a.len().min(b.len()) - pre;
    let suf = a.iter().rev().zip(b.iter().rev()).take(max_suf).take_while(|(x, y)| x == y).count();
    let lines = |v: &[&str]| v.iter().map(|l| l.trim_end_matches('\n').to_string()).collect();
    (lines(&a[pre..a.len() - suf]), lines(&b[pre..b.len() - suf]))
}

fn path(p: &str) -> Vec<&str> {
    p.split('.').collect()
}

#[test]
fn golden_wr8_every_field() {
    let cases: &[(&str, Value, &[&str], &[&str])] = &[
        ("id", json!("WR-80"), &["id: WR-8"], &["id: WR-80"]),
        (
            "title",
            json!("Content plan"),
            &["title: \"Write: content migration plan\"   # quoted because of the colon"],
            &["title: Content plan   # quoted because of the colon"],
        ),
        (
            "title",
            json!("New: plan"),
            &["title: \"Write: content migration plan\"   # quoted because of the colon"],
            &["title: \"New: plan\"   # quoted because of the colon"],
        ),
        ("status", json!("doing"), &["status: backlog"], &["status: doing"]),
        ("priority", json!(1), &["priority: 3"], &["priority: 1"]),
        ("due", json!("2026-10-20"), &["due: null"], &["due: 2026-10-20"]),
        ("tags", json!(["content", "needs-input", "seo"]), &[], &["  - seo"]),
        ("tags", json!(["content"]), &["  - \"needs-input\""], &[]),
        (
            "estimate",
            json!("5h"),
            &["estimate: 3h            # unknown field, must survive"],
            &["estimate: 5h            # unknown field, must survive"],
        ),
        ("custom.owner_note", json!("done"), &["  owner_note: ask client about legacy URLs"], &["  owner_note: done"]),
        ("custom.links", json!(["c"]), &["  links: [a, b]"], &["  links: [c]"]),
        ("custom.new", json!(1), &[], &["  new: 1"]),
        ("created", json!("2026-10-02"), &["created: 2026-10-01"], &["created: 2026-10-02"]),
        ("done_at", json!("2026-10-06"), &["done_at: null"], &["done_at: 2026-10-06"]),
        ("focus", json!("2026-10-06"), &[], &["focus: 2026-10-06"]),
        ("agent.ready", json!(true), &[], &["agent:", "  ready: true"]),
    ];
    for (p, v, removed, added) in cases {
        let out = set_field(WR8, &path(p), v).unwrap_or_else(|e| panic!("{p}: {e}"));
        assert_eq!(diff(WR8, &out), (removed.iter().map(|s| s.to_string()).collect(), added.iter().map(|s| s.to_string()).collect()), "{p}");
        assert_eq!(&get_field(&out, &path(p)).unwrap(), v, "{p}");
    }
}

#[test]
fn golden_wr8_remove() {
    let out = remove_field(WR8, &["estimate"]).unwrap();
    assert_eq!(diff(WR8, &out), (vec!["estimate: 3h            # unknown field, must survive".into()], vec![]));
    let out = remove_field(WR8, &["custom"]).unwrap();
    assert_eq!(diff(WR8, &out).0.len(), 3);
    let out = remove_field(WR8, &["tags"]).unwrap();
    assert_eq!(diff(WR8, &out).0, ["tags:", "  - content", "  - \"needs-input\""]);
}

#[test]
fn wr8_there_and_back() {
    let cases = [
        ("id", json!("X-1")),
        ("title", json!("Plain")),
        ("status", json!("doing")),
        ("priority", json!(1)),
        ("due", json!("2026-10-20")),
        ("tags", json!(["content", "needs-input", "z"])),
        ("estimate", json!(null)),
        ("custom.owner_note", json!("x: y")),
        ("custom.links", json!("scalar")),
        ("created", json!(true)),
        ("done_at", json!("2026-10-06")),
    ];
    for (p, y) in cases {
        let x = get_field(WR8, &path(p)).unwrap();
        let there = set_field(WR8, &path(p), &y).unwrap();
        assert_eq!(set_field(&there, &path(p), &x).unwrap(), WR8, "{p}");
    }
}

// ------------------------------------------------------------------ proptest

fn scalar() -> impl Strategy<Value = Value> {
    prop_oneof![
        "[a-zA-Z0-9 :#,'\"!&*?|>@%\\[\\]{}\\-äü]{0,10}".prop_map(Value::from),
        any::<i32>().prop_map(Value::from),
        any::<bool>().prop_map(Value::from),
        Just(Value::Null),
    ]
}

fn list() -> impl Strategy<Value = Value> {
    prop::collection::vec("[a-z0-9 :#,\"\\-]{0,6}".prop_map(Value::from), 1..4).prop_map(Value::from)
}

#[derive(Debug, Clone)]
enum Field {
    Scalar(Value, Option<String>),
    Flow(Value),
    Block(Value, usize),
}

fn field() -> impl Strategy<Value = Field> {
    prop_oneof![
        (scalar(), prop::option::of("[a-z ]{0,6}")).prop_map(|(v, c)| Field::Scalar(v, c)),
        list().prop_map(Field::Flow),
        (list(), 0..3usize).prop_map(|(v, i)| Field::Block(v, i)),
    ]
}

/// A frontmatter document plus the paths that can be patched and whether the
/// field is a block list (those only round-trip through non-empty lists).
fn document() -> impl Strategy<Value = (String, Vec<(Vec<String>, bool)>)> {
    (
        prop::collection::vec((field(), any::<bool>(), any::<bool>()), 1..6),
        prop::option::of(prop::collection::vec(scalar(), 1..4)),
        any::<bool>(),
    )
        .prop_map(|(fields, agent, crlf)| {
            let mut doc = String::from("---\n# header comment\n");
            let mut paths = Vec::new();
            for (i, (f, blank_before, comment_before)) in fields.into_iter().enumerate() {
                if blank_before {
                    doc.push('\n');
                }
                if comment_before {
                    doc.push_str("# about\n");
                }
                let key = format!("k{i}");
                match f {
                    Field::Scalar(v, c) => {
                        doc.push_str(&format!("{key}: {}", emit(&v).unwrap()));
                        if let Some(c) = c {
                            doc.push_str(&format!("   # {c}"));
                        }
                        doc.push('\n');
                    }
                    Field::Flow(v) => doc.push_str(&format!("{key}: {}\n", emit(&v).unwrap())),
                    Field::Block(v, ind) => {
                        doc.push_str(&format!("{key}:\n"));
                        for item in v.as_array().unwrap() {
                            doc.push_str(&format!("{}- {}\n", " ".repeat(ind), emit(item).unwrap()));
                        }
                    }
                }
                let block = doc.ends_with('\n') && doc.lines().last().is_some_and(|l| l.trim_start().starts_with('-'));
                paths.push((vec![key], block));
            }
            if let Some(children) = agent {
                doc.push_str("agent:\n");
                for (i, v) in children.iter().enumerate() {
                    doc.push_str(&format!("  c{i}: {}\n", emit(v).unwrap()));
                    paths.push((vec!["agent".into(), format!("c{i}")], false));
                }
            }
            doc.push_str("---\nbody  \n\n- [ ] x");
            if crlf {
                doc = doc.replace('\n', "\r\n");
            }
            (doc, paths)
        })
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(512))]

    #[test]
    fn patch_there_and_back_is_byte_identical(
        (doc, paths) in document(),
        pick in any::<prop::sample::Index>(),
        y_scalar in scalar(),
        y_list in list(),
        use_list in any::<bool>(),
    ) {
        let (p, block) = pick.get(&paths);
        let p: Vec<&str> = p.iter().map(String::as_str).collect();
        let y = if *block || use_list { y_list } else { y_scalar };
        let x = get_field(&doc, &p).unwrap();
        let there = set_field(&doc, &p, &y).unwrap();
        prop_assert_eq!(get_field(&there, &p).unwrap(), y);
        let back = set_field(&there, &p, &x).unwrap();
        prop_assert_eq!(back, doc);
    }
}
