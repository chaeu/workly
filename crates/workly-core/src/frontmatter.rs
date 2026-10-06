//! Split a Markdown file into frontmatter and body without changing a byte,
//! plus body helpers.

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
/// at the end if missing. Everything else stays byte-identical, including a
/// missing final newline.
pub fn append_update(body: &str, text: &str) -> String {
    let eol = eol_of(body);
    let item = format!("- {text}");
    let lines: Vec<&str> = body.split_inclusive('\n').collect();
    let content = |l: &str| l.trim_end_matches(['\n', '\r']).to_string();
    let Some(head) = lines.iter().position(|l| content(l).trim_end() == "## Updates") else {
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
    // Section ends at the next heading of level 1 or 2, or at EOF.
    let end = lines[head + 1..]
        .iter()
        .position(|l| l.starts_with("# ") || l.starts_with("## "))
        .map_or(lines.len(), |n| head + 1 + n);
    let last = (head..end).rev().find(|&i| !content(lines[i]).trim().is_empty()).unwrap_or(head);
    let mut out: String = lines[..=last].concat();
    if out.ends_with('\n') {
        out.push_str(&item);
        out.push_str(eol);
    } else {
        // Last line had no newline: keep "no final newline".
        out.push_str(eol);
        out.push_str(&item);
    }
    out.push_str(&lines[last + 1..].concat());
    out
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
    fn append_update_empty_section() {
        assert_eq!(append_update("## Updates\n\n## Next\n", "a"), "## Updates\n- a\n\n## Next\n");
    }
}
