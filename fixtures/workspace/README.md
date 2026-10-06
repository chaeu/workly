# Fixture workspace

Neutral sample data for development and tests. Never put real company data here.

What it covers:
- 6 projects, 4 of them use cases (ai, rule, hybrid, ai) in different steps and statuses
- nested project folder (`projects/archive/old-intranet`), archived
- project without `tasks/` (`support-triage`)
- inbox tasks, one with broken frontmatter (`IN-3`) -> must surface as an error, not crash
- `WR-8`: roundtrip fixture with comments, unknown fields, blank line, no final newline
- `WR-9` only in `.workly/trash/` -> next WR id is `WR-10`
- agent states: WR-3 running (codex), WR-5 ready, WR-7 in review with commit
- focus strip for 2026-10-06: WR-5, WR-3, IE-2

Tests copy this folder to a temp dir before writing. Never write to it in place.
