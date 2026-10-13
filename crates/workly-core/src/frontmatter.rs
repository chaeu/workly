//! Split a Markdown file into frontmatter and body without changing a byte,
//! plus body helpers.
//!
//! Task body layout: `## Description`, `## Acceptance` (old name `## Acceptance
//! criteria`), `## Updates`. A body without `## Description` is the old layout:
//! the description is the text before the first Acceptance or Updates heading.
//! A section ends at the next heading of level 1 or 2.

use serde::Serialize;
use std::ops::Range;

/// `head + frontmatter + fence + body` is always exactly the source.
#[derive(Debug, PartialEq, Eq, Clone, Copy)]
pub struct Parts<'a> {
    /// Optional BOM plus the opening `---` line. Empty when there is no frontmatter.
    pub head: &'a str,
    /// YAML text between the fences, including its final newline.
    pub frontmatter: &'a str,
    /// The closing `---` line including its line break (if any).
    pub fence: &'a str,
    pub body: &'a str,
}

impl Parts<'_> {
    pub fn has_frontmatter(&self) -> bool {
        !self.head.is_empty()
    }
}

/// Unclosed frontmatter counts as no frontmatter, like Obsidian does.
pub fn split(src: &str) -> Parts<'_> {
    let none = Parts { head: "", frontmatter: "", fence: "", body: src };
    let bom = if src.starts_with('\u{feff}') { '\u{feff}'.len_utf8() } else { 0 };
    let rest = &src[bom..];
    let open = if rest.starts_with("---\n") {
        4
    } else if rest.starts_with("---\r\n") {
        5
    } else {
        return none;
    };
    let fm_start = bom + open;
    let mut i = fm_start;
    while i < src.len() {
        let end = src[i..].find('\n').map_or(src.len(), |n| i + n + 1);
        if src[i..end].trim_end_matches(['\n', '\r']) == "---" {
            return Parts {
                head: &src[..fm_start],
                frontmatter: &src[fm_start..i],
                fence: &src[i..end],
                body: &src[end..],
            };
        }
        i = end;
    }
    none
}

/// The line ending a file uses: CRLF if it has any, else LF.
pub fn eol_of(src: &str) -> &'static str {
    if src.contains("\r\n") { "\r\n" } else { "\n" }
}

/// Append `- <text>` as the last item under `## Updates`, creating the section
/// at the end if missing. Further lines of `text` become continuation lines
/// indented by two spaces (still one list item). Everything else stays
/// byte-identical, including a missing final newline.
pub fn append_update(body: &str, text: &str) -> String {
    let eol = eol_of(body);
    let item = format!("- {}", text.lines().collect::<Vec<_>>().join(&format!("{eol}  ")));
    let lines: Vec<&str> = body.split_inclusive('\n').collect();
    let Some(head) = updates_head(&lines) else {
        let mut out = body.to_string();
        if !out.is_empty() {
            if !out.ends_with('\n') {
                out.push_str(eol);
            }
            out.push_str(eol);
        }
        out.push_str(&format!("## Updates{eol}{item}{eol}"));
        return out;
    };
    append_in_section(&lines, head, &item, eol)
}

/// `item` as the last line of the section under `head`, after its last non-blank line.
fn append_in_section(lines: &[&str], head: usize, item: &str, eol: &str) -> String {
    let end = section_end(lines, head);
    let last = (head..end).rev().find(|&i| !content(lines[i]).trim().is_empty()).unwrap_or(head);
    let mut out: String = lines[..=last].concat();
    if out.ends_with('\n') {
        out.push_str(item);
        out.push_str(eol);
    } else {
        // Last line had no newline: keep "no final newline".
        out.push_str(eol);
        out.push_str(item);
    }
    out.push_str(&lines[last + 1..].concat());
    out
}

fn content(l: &str) -> &str {
    l.trim_end_matches(['\n', '\r'])
}

/// A heading of level 1 or 2: ends a section.
pub fn is_section_end(l: &str) -> bool {
    l.starts_with("# ") || l.starts_with("## ")
}

fn is_description(l: &str) -> bool {
    l.trim_end() == "## Description"
}

fn is_acceptance(l: &str) -> bool {
    matches!(l.trim_end(), "## Acceptance" | "## Acceptance criteria")
}

fn is_updates(l: &str) -> bool {
    l.trim_end() == "## Updates"
}

/// Line index of the `## Updates` heading.
fn updates_head(lines: &[&str]) -> Option<usize> {
    lines.iter().position(|l| is_updates(l))
}

/// End (exclusive line index) of the section under `head`.
fn section_end(lines: &[&str], head: usize) -> usize {
    lines[head + 1..].iter().position(|l| is_section_end(l)).map_or(lines.len(), |n| head + 1 + n)
}

fn offset(lines: &[&str], i: usize) -> usize {
    lines[..i].iter().map(|l| l.len()).sum()
}

/// Byte range of the description in `body`, and whether the body has a
/// `## Description` section (else it is the old layout).
pub fn description_range(body: &str) -> (Range<usize>, bool) {
    let lines: Vec<&str> = body.split_inclusive('\n').collect();
    match lines.iter().position(|l| is_description(l)) {
        Some(d) => (offset(&lines, d + 1)..offset(&lines, section_end(&lines, d)), true),
        None => {
            let end = lines.iter().position(|l| is_acceptance(l) || is_updates(l)).unwrap_or(lines.len());
            (0..offset(&lines, end), false)
        }
    }
}

/// A line the description cannot contain, because it would end the description.
pub fn ends_description(line: &str, sectioned: bool) -> bool {
    if sectioned { is_section_end(line) } else { is_acceptance(line) || is_updates(line) }
}

/// One acceptance criterion as the app shows it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, serde::Deserialize)]
pub struct AcceptanceItem {
    pub text: String,
    pub done: bool,
}

/// `- [ ] a`, `- [x] a` or a plain `- a` (open). Empty items are no criteria.
fn acceptance_item(line: &str) -> Option<AcceptanceItem> {
    let c = content(line);
    let rest = c.strip_prefix("- ").or_else(|| c.strip_prefix("* "))?;
    let (done, text) = match rest.get(..3) {
        Some("[ ]") => (false, &rest[3..]),
        Some("[x]" | "[X]") => (true, &rest[3..]),
        _ => (false, rest),
    };
    let text = text.trim();
    (!text.is_empty()).then(|| AcceptanceItem { text: text.into(), done })
}

/// Criteria of the first Acceptance section with their line index.
fn acceptance_lines(lines: &[&str]) -> Vec<(usize, AcceptanceItem)> {
    let Some(head) = lines.iter().position(|l| is_acceptance(l)) else { return Vec::new() };
    (head + 1..section_end(lines, head)).filter_map(|i| Some((i, acceptance_item(lines[i])?))).collect()
}

/// Criteria of the Acceptance section; empty without one.
pub fn acceptance(body: &str) -> Vec<AcceptanceItem> {
    let lines: Vec<&str> = body.split_inclusive('\n').collect();
    acceptance_lines(&lines).into_iter().map(|(_, a)| a).collect()
}

/// Flip criterion `n`: exactly one line changes (`- [ ]` <-> `- [x]`, a plain
/// `- a` becomes `- [x] a`). `None` when item `n` is not `shown` any more.
pub fn toggle_acceptance(body: &str, n: usize, shown: &AcceptanceItem) -> Option<String> {
    let lines: Vec<&str> = body.split_inclusive('\n').collect();
    let (i, item) = acceptance_lines(&lines).into_iter().nth(n)?;
    if &item != shown {
        return None;
    }
    let c = content(lines[i]);
    let (marker, rest) = c.split_at(2);
    let line = match rest.get(..3) {
        Some("[ ]") => format!("{marker}[x]{}", &rest[3..]),
        Some("[x]" | "[X]") => format!("{marker}[ ]{}", &rest[3..]),
        _ => format!("{marker}[x] {rest}"),
    };
    Some([&lines[..i].concat(), line.as_str(), &lines[i][c.len()..], &lines[i + 1..].concat()].concat())
}

/// Append `- [ ] <text>` to the Acceptance section; the section is created above
/// `## Updates` (else at the end) when missing.
pub fn append_acceptance(body: &str, text: &str) -> String {
    let eol = eol_of(body);
    let item = format!("- [ ] {text}");
    let lines: Vec<&str> = body.split_inclusive('\n').collect();
    match lines.iter().position(|l| is_acceptance(l)) {
        Some(head) => append_in_section(&lines, head, &item, eol),
        None => insert_above_updates(body, &format!("## Acceptance{eol}{item}{eol}")),
    }
}

/// "Add sections" for an old-layout body: `## Description` on top (the old text
/// becomes the description) and an empty `## Acceptance` above `## Updates`,
/// each only when missing. Nothing else changes.
pub fn add_sections(body: &str) -> String {
    let eol = eol_of(body);
    let lines: Vec<&str> = body.split_inclusive('\n').collect();
    let mut out = body.to_string();
    if !lines.iter().any(|l| is_description(l)) {
        out = format!("## Description{eol}{out}");
    }
    if !lines.iter().any(|l| is_acceptance(l)) {
        out = insert_above_updates(&out, &format!("## Acceptance{eol}"));
    }
    out
}

/// `block` (whole lines) above the `## Updates` heading with a blank line on
/// both sides, else at the end after a blank line.
fn insert_above_updates(body: &str, block: &str) -> String {
    let eol = eol_of(body);
    let lines: Vec<&str> = body.split_inclusive('\n').collect();
    let (mut out, rest) = match updates_head(&lines) {
        Some(u) => (lines[..u].concat(), Some(lines[u..].concat())),
        None => (body.to_string(), None),
    };
    if !out.is_empty() && !out.ends_with('\n') {
        out.push_str(eol);
    }
    if !out.trim().is_empty() && !out.ends_with(&eol.repeat(2)) {
        out.push_str(eol);
    }
    out.push_str(block);
    if let Some(rest) = rest {
        out.push_str(eol);
        out.push_str(&rest);
    }
    out
}

/// The description as the user sees it: LF line ends, no trailing whitespace.
pub fn normalize_description(text: &str) -> String {
    text.replace("\r\n", "\n").trim_end().to_string()
}

/// Replace the description (see `description_range`) with `text`; everything
/// else stays byte-identical. The whitespace after the old description (blank
/// line before the next section, final newline or none) is kept.
pub fn replace_description(body: &str, text: &str, eol: &str) -> String {
    let (range, sectioned) = description_range(body);
    let (before, old, rest) = (&body[..range.start], &body[range.clone()], &body[range.end..]);
    let mut before = before.to_string();
    if !before.is_empty() && !before.ends_with('\n') {
        before.push_str(eol);
    }
    let text = normalize_description(text).replace('\n', eol);
    if text.is_empty() {
        // An empty section keeps one blank line before the next heading.
        let blank = if sectioned && !rest.is_empty() { eol } else { "" };
        return format!("{before}{blank}{rest}");
    }
    let kept = old.trim_end();
    let gap = if !kept.is_empty() {
        old[kept.len()..].to_string()
    } else if rest.is_empty() {
        eol.to_string()
    } else {
        eol.repeat(2)
    };
    format!("{before}{text}{gap}{rest}")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn join(p: Parts) -> String {
        [p.head, p.frontmatter, p.fence, p.body].concat()
    }

    #[test]
    fn split_cases() {
        let cases = [
            ("---\nid: A-1\n---\nbody\n", "---\n", "id: A-1\n", "---\n", "body\n"),
            ("no frontmatter\n", "", "", "", "no frontmatter\n"),
            ("---\n---\nbody", "---\n", "", "---\n", "body"),
            ("---\nid: A-1\n---", "---\n", "id: A-1\n", "---", ""),
            ("---\r\nid: A-1\r\n---\r\nb\r\n", "---\r\n", "id: A-1\r\n", "---\r\n", "b\r\n"),
            ("\u{feff}---\nid: A-1\n---\nb", "\u{feff}---\n", "id: A-1\n", "---\n", "b"),
            ("---\nid: A-1\nnever closed\n", "", "", "", "---\nid: A-1\nnever closed\n"),
            ("", "", "", "", ""),
            ("----\nx\n", "", "", "", "----\nx\n"),
        ];
        for (src, head, fm, fence, body) in cases {
            let p = split(src);
            assert_eq!((p.head, p.frontmatter, p.fence, p.body), (head, fm, fence, body), "{src:?}");
            assert_eq!(join(p), src);
        }
    }

    #[test]
    fn append_update_existing_section() {
        let body = "Text\n\n## Updates\n- one\n\n## Other\nx\n";
        assert_eq!(append_update(body, "two"), "Text\n\n## Updates\n- one\n- two\n\n## Other\nx\n");
    }

    #[test]
    fn append_update_section_at_eof_without_newline() {
        assert_eq!(append_update("## Updates\n- one", "two"), "## Updates\n- one\n- two");
    }

    #[test]
    fn append_update_creates_section() {
        assert_eq!(append_update("", "a"), "## Updates\n- a\n");
        assert_eq!(append_update("Text\n", "a"), "Text\n\n## Updates\n- a\n");
        assert_eq!(append_update("Text", "a"), "Text\n\n## Updates\n- a\n");
        assert_eq!(append_update("T\r\n", "a"), "T\r\n\r\n## Updates\r\n- a\r\n");
    }

    #[test]
    fn replace_description_cases() {
        let r = |b, t| replace_description(b, t, "\n");
        assert_eq!(r("Old\n\n## Updates\n- a\n", "New\nlines \n\n"), "New\nlines\n\n## Updates\n- a\n");
        assert_eq!(r("Old\n## Updates\n- a", "New"), "New\n## Updates\n- a");
        assert_eq!(r("## Updates\n- a\n", "New"), "New\n\n## Updates\n- a\n");
        assert_eq!(r("Old\n\n## Updates\n- a\n", ""), "## Updates\n- a\n");
        assert_eq!(r("Old", "New"), "New");
        assert_eq!(r("Old\n", "New"), "New\n");
        assert_eq!(r("", "New"), "New\n");
        assert_eq!(r("Old\n", " "), "");
        let crlf = "Old\r\n\r\n## Updates\r\n- a\r\n";
        assert_eq!(replace_description(crlf, "A\nB", "\r\n"), "A\r\nB\r\n\r\n## Updates\r\n- a\r\n");
        assert_eq!(description_range(crlf), (0..7, false));
    }

    const SECTIONS: &str = "## Description\nOld text\n\n## Acceptance\n- [x] a\n- [ ] b\n- plain\n- \n  - nested\n\n## Updates\n- 2026 · me: x\n";

    #[test]
    fn description_in_sections() {
        let r = |b, t| replace_description(b, t, "\n");
        assert_eq!(r(SECTIONS, "New\nlines"), SECTIONS.replace("Old text\n", "New\nlines\n"));
        assert_eq!(r(SECTIONS, ""), SECTIONS.replace("Old text\n\n", "\n"));
        let empty = "## Description\n\n## Acceptance\n\n## Updates\n";
        assert_eq!(r(empty, "X"), "## Description\nX\n\n## Acceptance\n\n## Updates\n");
        let once = r(empty, "X");
        assert_eq!(r(&once, ""), empty);
        assert_eq!(r("## Description", "X"), "## Description\nX\n");
        // Old layout with an Acceptance section: the description ends there.
        let old = "Free text\n\n## Acceptance criteria\n- a\n";
        assert_eq!(r(old, "New"), "New\n\n## Acceptance criteria\n- a\n");
        assert!(ends_description("## Notes", true) && !ends_description("## Notes", false));
        assert!(ends_description("## Acceptance criteria", false));
    }

    #[test]
    fn acceptance_items() {
        let item = |t: &str, done| AcceptanceItem { text: t.into(), done };
        assert_eq!(acceptance(SECTIONS), [item("a", true), item("b", false), item("plain", false)]);
        assert_eq!(acceptance("Text\n- [ ] not in a section\n"), []);
        assert_eq!(acceptance("## Acceptance criteria\n- Header ok\n* [X] Star\n"), [item("Header ok", false), item("Star", true)]);
    }

    #[test]
    fn toggle_changes_one_line() {
        let item = |t: &str, done| AcceptanceItem { text: t.into(), done };
        assert_eq!(toggle_acceptance(SECTIONS, 0, &item("a", true)).unwrap(), SECTIONS.replace("- [x] a", "- [ ] a"));
        assert_eq!(toggle_acceptance(SECTIONS, 1, &item("b", false)).unwrap(), SECTIONS.replace("- [ ] b", "- [x] b"));
        assert_eq!(toggle_acceptance(SECTIONS, 2, &item("plain", false)).unwrap(), SECTIONS.replace("- plain", "- [x] plain"));
        // Not what the app showed: refused.
        assert_eq!(toggle_acceptance(SECTIONS, 1, &item("b", true)), None);
        assert_eq!(toggle_acceptance(SECTIONS, 1, &item("c", false)), None);
        assert_eq!(toggle_acceptance(SECTIONS, 3, &item("x", false)), None);
        let crlf = "## Acceptance\r\n- [ ] a\r\n";
        assert_eq!(toggle_acceptance(crlf, 0, &item("a", false)).unwrap(), "## Acceptance\r\n- [x] a\r\n");
    }

    #[test]
    fn append_acceptance_cases() {
        assert_eq!(append_acceptance(SECTIONS, "c"), SECTIONS.replace("  - nested\n", "  - nested\n- [ ] c\n"));
        assert_eq!(append_acceptance("Text\n\n## Updates\n- u\n", "c"), "Text\n\n## Acceptance\n- [ ] c\n\n## Updates\n- u\n");
        assert_eq!(append_acceptance("Text\n## Updates\n", "c"), "Text\n\n## Acceptance\n- [ ] c\n\n## Updates\n");
        assert_eq!(append_acceptance("Text", "c"), "Text\n\n## Acceptance\n- [ ] c\n");
        assert_eq!(append_acceptance("", "c"), "## Acceptance\n- [ ] c\n");
    }

    #[test]
    fn add_sections_cases() {
        assert_eq!(add_sections("Text\n\n## Updates\n- u\n"), "## Description\nText\n\n## Acceptance\n\n## Updates\n- u\n");
        assert_eq!(add_sections("Text\n"), "## Description\nText\n\n## Acceptance\n");
        assert_eq!(add_sections(""), "## Description\n\n## Acceptance\n");
        let old = "Text\n\n## Acceptance criteria\n- a\n";
        assert_eq!(add_sections(old), format!("## Description\n{old}"));
        assert_eq!(add_sections(SECTIONS), SECTIONS);
    }

    #[test]
    fn append_update_continuation_lines() {
        assert_eq!(append_update("## Updates\n- one\n", "two\nthree"), "## Updates\n- one\n- two\n  three\n");
        assert_eq!(append_update("T\r\n", "a\nb"), "T\r\n\r\n## Updates\r\n- a\r\n  b\r\n");
    }

    #[test]
    fn append_update_empty_section() {
        assert_eq!(append_update("## Updates\n\n## Next\n", "a"), "## Updates\n- a\n\n## Next\n");
    }
}
