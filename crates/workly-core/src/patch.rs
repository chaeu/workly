//! Lossless frontmatter patcher. Edits lines in place and never round-trips
//! YAML through a serializer: unknown fields, key order, comments, blank lines
//! and the quoting of untouched values stay byte-identical.
//!
//! Supported paths: up to three keys through nested block maps (`status`,
//! `agent.active`, `usecase.assessment.ko`). Values are JSON scalars, lists of
//! scalars or flat maps, and flat maps of scalars (written as `{ a: b }`).

use crate::frontmatter::{eol_of, split};
use serde_json::Value;

/// Set `path` to `value`. The result is re-parsed and checked: if anything but
/// the target changed, nothing is returned.
pub fn set_field(src: &str, path: &[&str], value: &Value) -> Result<String, String> {
    check_path(path)?;
    let text = emit(value)?;
    let before = parse(src)?;
    let out = patch(src, |doc| set_in(doc, Scope::top(doc), path, value, &text))?;
    let mut expected = before;
    json_set(&mut expected, path, Some(value.clone()));
    verify(&out, &expected)?;
    Ok(out)
}

/// Remove `path`. Removing a missing key is a no-op.
pub fn remove_field(src: &str, path: &[&str]) -> Result<String, String> {
    check_path(path)?;
    let before = parse(src)?;
    let out = patch(src, |doc| remove_in(doc, Scope::top(doc), path))?;
    let mut expected = before;
    json_set(&mut expected, path, None);
    verify(&out, &expected)?;
    Ok(out)
}

/// Current value at `path` (null when missing).
pub fn get_field(src: &str, path: &[&str]) -> Result<Value, String> {
    let mut v = &parse(src)?;
    for key in path {
        match v.get(key) {
            Some(next) => v = next,
            None => return Ok(Value::Null),
        }
    }
    Ok(v.clone())
}

/// Frontmatter as JSON; an empty frontmatter is an empty object.
fn parse(src: &str) -> Result<Value, String> {
    let parts = split(src);
    if !parts.has_frontmatter() {
        return Err("file has no frontmatter".into());
    }
    let yaml: serde_yaml::Value = serde_yaml::from_str(parts.frontmatter).map_err(|e| format!("invalid frontmatter: {e}"))?;
    match serde_json::to_value(yaml).map_err(|e| e.to_string())? {
        Value::Null => Ok(Value::Object(Default::default())),
        v @ Value::Object(_) => Ok(v),
        _ => Err("frontmatter is not a map".into()),
    }
}

fn verify(out: &str, expected: &Value) -> Result<(), String> {
    let got = parse(out)?;
    if &got != expected {
        return Err(format!("patch check failed, nothing written: got {got}, expected {expected}"));
    }
    Ok(())
}

/// Apply a change to the JSON view. A map emptied by removal reads back as null.
fn json_set(v: &mut Value, path: &[&str], value: Option<Value>) {
    if !v.is_object() {
        *v = Value::Object(Default::default());
    }
    let map = v.as_object_mut().unwrap();
    if path.len() == 1 {
        match value {
            Some(x) => map.insert(path[0].into(), x),
            None => map.remove(path[0]),
        };
        return;
    }
    if value.is_none() && !map.contains_key(path[0]) {
        return;
    }
    let child = map.entry(path[0]).or_insert(Value::Null);
    json_set(child, &path[1..], value);
    if child.as_object().is_some_and(|m| m.is_empty()) {
        *child = Value::Null;
    }
}

fn check_path(path: &[&str]) -> Result<(), String> {
    let ok = |k: &str| !k.is_empty() && k.chars().all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-');
    if path.is_empty() || path.len() > 3 || !path.iter().all(|k| ok(k)) {
        return Err(format!("unsupported field path '{}'", path.join(".")));
    }
    Ok(())
}

// ------------------------------------------------------------------ emitting

/// YAML text for a value, quoted only where needed.
pub fn emit(v: &Value) -> Result<String, String> {
    Ok(match v {
        Value::Null => "null".into(),
        Value::Bool(b) => b.to_string(),
        Value::Number(n) => n.to_string(),
        Value::String(s) => scalar(s, false),
        Value::Array(items) => {
            let items: Result<Vec<_>, _> = items.iter().map(emit_item(true)).collect();
            format!("[{}]", items?.join(", "))
        }
        Value::Object(_) => emit_item(true)(v)?,
    })
}

/// List items: scalars, or flat maps of scalars as `{ label: a, url: b }`.
fn emit_item(in_flow: bool) -> impl Fn(&Value) -> Result<String, String> {
    move |v| match v {
        Value::String(s) => Ok(scalar(s, in_flow)),
        Value::Object(map) => {
            let fields: Result<Vec<_>, String> = map
                .iter()
                .map(|(k, v)| match v {
                    Value::Array(_) | Value::Object(_) => Err("maps may only hold scalars".into()),
                    _ => Ok(format!("{}: {}", scalar(k, true), emit_item(true)(v)?)),
                })
                .collect();
            Ok(format!("{{ {} }}", fields?.join(", ")))
        }
        Value::Array(_) => Err("lists may only contain scalars or flat maps".into()),
        _ => emit(v),
    }
}

fn scalar(s: &str, in_flow: bool) -> String {
    if plain_ok(s, in_flow) { s.to_string() } else { quote(s) }
}

/// Plain is fine when the YAML parser reads it back as the same string.
fn plain_ok(s: &str, in_flow: bool) -> bool {
    // YAML 1.1 booleans: still read as bool by PyYAML and friends.
    const YAML11: [&str; 6] = ["y", "n", "yes", "no", "on", "off"];
    // Number-like strings ("007", "1e3") read as numbers in other parsers.
    if s.is_empty()
        || s != s.trim()
        || s.chars().any(char::is_control)
        || YAML11.contains(&s.to_lowercase().as_str())
        || s.parse::<f64>().is_ok()
    {
        return false;
    }
    if in_flow {
        return !s.contains([',', '[', ']', '{', '}'])
            && serde_yaml::from_str::<Vec<serde_yaml::Value>>(&format!("[{s}]"))
                .is_ok_and(|v| v.len() == 1 && v[0].as_str() == Some(s));
    }
    serde_yaml::from_str::<serde_yaml::Value>(s).is_ok_and(|v| v.as_str() == Some(s))
}

fn quote(s: &str) -> String {
    let mut out = String::from("\"");
    for c in s.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            c if c.is_control() => out.push_str(&format!("\\u{:04x}", c as u32)),
            c => out.push(c),
        }
    }
    out.push('"');
    out
}

// --------------------------------------------------------------- line model

struct Doc {
    /// Frontmatter lines, each with its own line ending.
    lines: Vec<String>,
    eol: &'static str,
}

/// A block map: lines `start..end`, keys at `indent`.
#[derive(Clone, Copy)]
struct Scope {
    start: usize,
    end: usize,
    indent: usize,
}

impl Scope {
    fn top(doc: &Doc) -> Scope {
        Scope { start: 0, end: doc.lines.len(), indent: 0 }
    }
}

fn patch(src: &str, f: impl FnOnce(&mut Doc) -> Result<(), String>) -> Result<String, String> {
    let parts = split(src);
    let mut doc = Doc { lines: parts.frontmatter.split_inclusive('\n').map(String::from).collect(), eol: eol_of(src) };
    f(&mut doc)?;
    Ok([parts.head, &doc.lines.concat(), parts.fence, parts.body].concat())
}

fn content(l: &str) -> &str {
    l.strip_suffix('\n').map_or(l, |l| l.strip_suffix('\r').unwrap_or(l))
}

fn indent(l: &str) -> usize {
    let c = content(l);
    c.len() - c.trim_start_matches(' ').len()
}

fn is_blank(l: &str) -> bool {
    content(l).trim().is_empty()
}

fn is_item(l: &str) -> bool {
    let t = content(l).trim_start();
    t == "-" || t.starts_with("- ")
}

/// `key: value  # comment` split into byte offsets of `content(line)`.
struct KeyLine {
    colon: usize,
    value_start: usize,
    value_end: usize,
}

impl KeyLine {
    fn has_value(&self) -> bool {
        self.value_end > self.value_start
    }
}

/// Key and colon offset of a `key: ...` line.
fn key_of(l: &str) -> Option<(&str, usize)> {
    let c = content(l);
    let ind = indent(l);
    let t = &c[ind..];
    if t.is_empty() || t.starts_with(['#', '-', '"', '\'', '{', '[', '?']) {
        return None;
    }
    let colon = t.char_indices().find(|&(i, ch)| ch == ':' && matches!(t[i + 1..].chars().next(), None | Some(' ' | '\t')))?.0;
    Some((&t[..colon], ind + colon))
}

/// Where the value ends and a trailing comment starts. Instead of a hand-written
/// YAML scanner, the parser decides: the value is the shortest cut before a
/// ` #` that reads the same as the whole rest of the line.
fn key_line(l: &str, colon: usize) -> KeyLine {
    let c = content(l);
    let after = &c[colon + 1..];
    let start = colon + 1 + (after.len() - after.trim_start_matches([' ', '\t']).len());
    let rest = &c[start..];
    if rest.is_empty() || rest.starts_with('#') {
        return KeyLine { colon, value_start: colon + 1, value_end: colon + 1 };
    }
    let parse = |s: &str| serde_yaml::from_str::<serde_yaml::Value>(s).ok();
    let full = parse(rest);
    let mut cuts: Vec<usize> = rest.match_indices([' ', '\t']).map(|(i, _)| i).filter(|&i| rest[i + 1..].starts_with('#')).collect();
    cuts.push(rest.len());
    let cut = cuts.into_iter().find(|&i| full.is_some() && parse(&rest[..i]) == full).unwrap_or(rest.len());
    KeyLine { colon, value_start: start, value_end: start + rest[..cut].trim_end().len() }
}

fn find_key(doc: &Doc, scope: Scope, key: &str) -> Option<(usize, KeyLine)> {
    (scope.start..scope.end).find_map(|i| {
        let (k, colon) = key_of(&doc.lines[i])?;
        (indent(&doc.lines[i]) == scope.indent && k == key).then(|| (i, key_line(&doc.lines[i], colon)))
    })
}

/// End (exclusive) of the block under the key at `i`, trailing blank lines excluded.
fn block_end(doc: &Doc, i: usize, scope: Scope, has_value: bool) -> usize {
    let mut last = i + 1;
    for j in i + 1..scope.end {
        let l = &doc.lines[j];
        if is_blank(l) {
            continue;
        }
        // Block sequences may sit at the key's own indent.
        let child = indent(l) > scope.indent || (!has_value && indent(l) == scope.indent && is_item(l));
        if !child {
            break;
        }
        last = j + 1;
    }
    last
}

/// The key line with its value replaced (or cleared), trailing comment kept.
fn with_value(line: &str, kl: &KeyLine, new: Option<&str>) -> String {
    let c = content(line);
    let eol = &line[c.len()..];
    let (head, tail) = match new {
        None => (c[..=kl.colon].to_string(), &c[kl.value_end..]),
        Some(v) if !kl.has_value() => (format!("{} {v}", &c[..=kl.colon]), &c[kl.value_end..]),
        Some(v) => (format!("{}{v}", &c[..kl.value_start]), &c[kl.value_end..]),
    };
    format!("{head}{tail}{eol}")
}

fn set_in(doc: &mut Doc, scope: Scope, path: &[&str], value: &Value, text: &str) -> Result<(), String> {
    let eol = doc.eol;
    let Some((i, kl)) = find_key(doc, scope, path[0]) else {
        // New keys go to the end of their map.
        let at = (scope.start..scope.end).rev().find(|&j| !is_blank(&doc.lines[j])).map_or(scope.start, |j| j + 1);
        let new = path.iter().enumerate().map(|(d, key)| {
            let pad = " ".repeat(scope.indent + 2 * d);
            if d + 1 == path.len() { format!("{pad}{key}: {text}{eol}") } else { format!("{pad}{key}:{eol}") }
        });
        doc.lines.splice(at..at, new);
        return Ok(());
    };
    let end = block_end(doc, i, scope, kl.has_value());

    if path.len() > 1 {
        if kl.has_value() {
            // Only `key: null` / `key: ~` may turn into a block map.
            let v = &content(&doc.lines[i])[kl.value_start..kl.value_end];
            if !matches!(v, "null" | "~" | "Null" | "NULL") {
                return Err(format!("'{}' is not a block map", path[0]));
            }
            doc.lines[i] = with_value(&doc.lines[i], &kl, None);
        }
        let first = (i + 1..end).find(|&j| !is_blank(&doc.lines[j]) && !content(&doc.lines[j]).trim_start().starts_with('#'));
        if first.is_some_and(|j| is_item(&doc.lines[j])) {
            return Err(format!("'{}' is a list, not a map", path[0]));
        }
        let child_indent = first.map_or(scope.indent + 2, |j| indent(&doc.lines[j]));
        return set_in(doc, Scope { start: i + 1, end, indent: child_indent }, &path[1..], value, text);
    }

    // Block lists stay block lists; unchanged item lines are reused verbatim.
    let items: Vec<usize> = (i + 1..end).filter(|&j| is_item(&doc.lines[j])).collect();
    if let (Value::Array(new_items), Some(&first)) = (value, items.first())
        && !kl.has_value()
        && !new_items.is_empty()
    {
        let pad = " ".repeat(indent(&doc.lines[first]));
        let mut old: Vec<Option<(String, Value)>> = items
            .iter()
            .map(|&j| {
                let line = doc.lines[j].clone();
                let item = content(&line).trim_start()[1..].trim_start().to_string();
                let parsed = serde_yaml::from_str::<serde_yaml::Value>(&item).ok().and_then(|v| serde_json::to_value(v).ok());
                parsed.map(|v| (line, v))
            })
            .collect();
        let mut lines = Vec::new();
        for item in new_items {
            let reused = old.iter_mut().find(|o| o.as_ref().is_some_and(|(_, v)| v == item)).and_then(Option::take);
            lines.push(match reused {
                Some((line, _)) => line,
                None => format!("{pad}- {}{eol}", emit_item(false)(item)?),
            });
        }
        doc.lines.splice(i + 1..end, lines);
        return Ok(());
    }

    doc.lines[i] = with_value(&doc.lines[i], &kl, Some(text));
    doc.lines.drain(i + 1..end);
    Ok(())
}

fn remove_in(doc: &mut Doc, scope: Scope, path: &[&str]) -> Result<(), String> {
    let Some((i, kl)) = find_key(doc, scope, path[0]) else { return Ok(()) };
    let end = block_end(doc, i, scope, kl.has_value());
    if path.len() > 1 {
        if kl.has_value() || end == i + 1 {
            return Ok(());
        }
        let child_indent = (i + 1..end).find(|&j| !is_blank(&doc.lines[j])).map_or(0, |j| indent(&doc.lines[j]));
        return remove_in(doc, Scope { start: i + 1, end, indent: child_indent }, &path[1..]);
    }
    doc.lines.drain(i..end);
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn set(src: &str, path: &str, v: Value) -> String {
        set_field(src, &path.split('.').collect::<Vec<_>>(), &v).unwrap()
    }

    #[test]
    fn minimal_quoting() {
        let cases = [
            ("todo", "todo"),
            ("2026-10-10", "2026-10-10"),
            ("Write: plan", "\"Write: plan\""),
            ("a #b", "\"a #b\""),
            ("#x", "\"#x\""),
            ("", "\"\""),
            ("3", "\"3\""),
            ("007", "\"007\""),
            ("null", "\"null\""),
            ("yes", "\"yes\""),
            ("true", "\"true\""),
            ("- x", "\"- x\""),
            ("*x", "\"*x\""),
            (" pad", "\" pad\""),
            ("two\nlines", "\"two\\nlines\""),
            ("say \"hi\"", "say \"hi\""),
            ("Navigation überarbeiten", "Navigation überarbeiten"),
        ];
        for (s, want) in cases {
            assert_eq!(emit(&json!(s)).unwrap(), want, "{s:?}");
        }
        assert_eq!(emit(&json!(["a", "b,c", 3, null])).unwrap(), "[a, \"b,c\", 3, null]");
    }

    #[test]
    fn keeps_comment_and_quoting_rules() {
        let src = "---\ntitle: \"A: b\"   # why\nx: 1\n---\n";
        assert_eq!(set(src, "title", json!("C")), "---\ntitle: C   # why\nx: 1\n---\n");
        assert_eq!(set(src, "title", json!("C: d")), "---\ntitle: \"C: d\"   # why\nx: 1\n---\n");
    }

    #[test]
    fn nested_and_new_keys() {
        let src = "---\nid: A-1\nagent:\n  ready: false\n  active: null\n---\nbody";
        assert_eq!(set(src, "agent.active", json!("codex")), "---\nid: A-1\nagent:\n  ready: false\n  active: codex\n---\nbody");
        assert_eq!(set(src, "agent.since", json!("x")), "---\nid: A-1\nagent:\n  ready: false\n  active: null\n  since: x\n---\nbody");
        assert_eq!(set("---\nid: A-1\n---\n", "agent.ready", json!(true)), "---\nid: A-1\nagent:\n  ready: true\n---\n");
        assert_eq!(set("---\nagent: null\n---\n", "agent.ready", json!(true)), "---\nagent:\n  ready: true\n---\n");
        assert_eq!(set("---\n---\n", "id", json!("A-1")), "---\nid: A-1\n---\n");
    }

    #[test]
    fn three_levels_and_flow_maps() {
        let src = "---\nid: A-1\nuc:\n  step: x   # here\n---\n";
        let added = set(src, "uc.a.ko", json!({ "owner": "pass", "data_use": "open" }));
        assert_eq!(added, "---\nid: A-1\nuc:\n  step: x   # here\n  a:\n    ko: { owner: pass, data_use: open }\n---\n");
        // A hand-written block map is replaced by one flow line; a trailing comment stays.
        let block = "---\nuc:\n  a:\n    ko:   # k.o.\n      owner: pass\n      risk: maybe\n    note: n\n---\n";
        assert_eq!(set(block, "uc.a.ko", json!({ "risk": "open" })), "---\nuc:\n  a:\n    ko: { risk: open }   # k.o.\n    note: n\n---\n");
        assert_eq!(remove_field(block, &["uc", "a", "ko"]).unwrap(), "---\nuc:\n  a:\n    note: n\n---\n");
        assert_eq!(emit(&json!({ "a": "x, y", "b": 3 })).unwrap(), "{ a: \"x, y\", b: 3 }");
    }

    #[test]
    fn lists() {
        assert_eq!(set("---\ntags: [a]\n---\n", "tags", json!(["a", "b"])), "---\ntags: [a, b]\n---\n");
        assert_eq!(set("---\nid: x\n---\n", "tags", json!(["a"])), "---\nid: x\ntags: [a]\n---\n");
        let block = "---\ntags:\n  - a\n  - \"b\"\nx: 1\n---\n";
        assert_eq!(set(block, "tags", json!(["b", "c"])), "---\ntags:\n  - \"b\"\n  - c\nx: 1\n---\n");
        assert_eq!(set(block, "tags", json!([])), "---\ntags: []\nx: 1\n---\n");
        let compact = "---\ntags:\n- a\nx: 1\n---\n";
        assert_eq!(set(compact, "tags", json!(["a", "b"])), "---\ntags:\n- a\n- b\nx: 1\n---\n");
    }

    #[test]
    fn crlf_and_remove() {
        let src = "---\r\nid: A-1\r\nagent:\r\n  ready: true\r\n---\r\nb\r\n";
        assert_eq!(set(src, "due", json!(null)), "---\r\nid: A-1\r\nagent:\r\n  ready: true\r\ndue: null\r\n---\r\nb\r\n");
        assert_eq!(remove_field(src, &["agent"]).unwrap(), "---\r\nid: A-1\r\n---\r\nb\r\n");
        assert_eq!(remove_field(src, &["agent", "ready"]).unwrap(), "---\r\nid: A-1\r\nagent:\r\n---\r\nb\r\n");
        assert_eq!(remove_field(src, &["missing"]).unwrap(), src);
    }

    #[test]
    fn rejects_bad_input() {
        let src = "---\nagent: codex\ntags: [a]\n---\n";
        assert!(set_field(src, &["agent", "x"], &json!(1)).is_err());
        assert!(set_field(src, &["tags", "x"], &json!(1)).is_err());
        assert!(set_field(src, &["a: b"], &json!(1)).is_err());
        assert!(set_field(src, &["x"], &json!({"a": {"b": 1}})).is_err());
        assert!(set_field(src, &["a", "b", "c", "d"], &json!(1)).is_err());
        assert!(set_field("no frontmatter", &["x"], &json!(1)).is_err());
        assert!(set_field("---\nid: \"broken\n---\n", &["x"], &json!(1)).is_err());
    }
}
