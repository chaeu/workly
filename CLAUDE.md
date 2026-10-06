# Workly

Lightweight local macOS app to manage tasks, projects and use cases on top of plain Markdown files, plus a CLI (`wly`) that coding agents use to report progress.

## Read first

- `docs/SPEC.md` – the specification. It is the source of truth. If code and spec disagree, ask; do not silently deviate.
- `docs/design/DESIGN.md`, `docs/design/tokens.css`, `docs/design/components.css` – the design system.
- `docs/reference/*.html` – working HTML prototypes (task board, use-case cockpit). Open them in a browser to see behaviour. `docs/screenshots/` shows the target look.
- `docs/PROMPTS.md` – the milestone plan. Work on one milestone at a time.

The spec and design docs are written in German; code, comments, commits, UI labels and identifiers are English.

## Stack

- Tauri 2, SvelteKit with `adapter-static` (SPA, `ssr = false`), Svelte 5 runes, TypeScript strict.
- Rust Cargo workspace: `crates/workly-core` (all file logic), `src-tauri` (thin command layer), `crates/wly` (CLI).
- pnpm. No UI component library, no Tailwind. Plain CSS with the design tokens.
- No database, no server, no network access at runtime.

## Layout

```
crates/workly-core/   domain, scan, parse, lossless write, index, watcher, ids, process, log, trash
crates/wly/           CLI (clap), JSON output
src-tauri/            Tauri app, commands + events only
src/                  SvelteKit frontend
  lib/styles/         tokens.css, components.css (copied from docs/design)
  lib/components/     Svelte components
  lib/stores/         state (runes), talks to Tauri commands
fixtures/workspace/   sample workspace for dev and tests
docs/                 spec, design, prototypes, prompts
```

## Hard rules

1. **Files are the source of truth.** The app never keeps state that is not in the workspace or in device settings.
2. **Lossless writes.** Changing a field rewrites only that field's line(s). Unknown fields, key order, comments, blank lines and the body stay byte-identical. Do not round-trip frontmatter through a YAML serializer. Every write path needs a test against `fixtures/workspace/projects/website-relaunch/tasks/WR-8-content-migration.md`.
3. **All file logic lives in `workly-core`.** `src-tauri` and `wly` only call it. The frontend never touches the filesystem directly.
4. **Design tokens only.** No hex colors, no ad-hoc px values for colors, radii, shadows or fonts in components. Use `var(--w-*)`. If a token is missing, add it to `tokens.css` and say so.
5. **Atomic writes.** Write to a temp file in the same folder, then rename. Ignore the watcher event caused by our own write.
6. **Delete = move to `.workly/trash/`** with the relative path preserved. Never `rm`.
7. **Log every change** to `.workly/log/YYYY-MM.jsonl`.
8. **Never write to `fixtures/workspace` in tests.** Copy it to a temp dir first (`tempfile`).
9. **No company data.** Only neutral sample data in the repo.
10. **Agents never set `done`.** The CLI refuses it for actors other than the human user (exit code 3).

## Working style

- Before starting a milestone, read its section in `docs/PROMPTS.md` and post a short plan (files, steps, open questions). Then build.
- Stop at the end of the milestone and report: what was built, how to verify, what deviates from spec, what is left. Do not start the next milestone on your own.
- Prefer small, boring code. No abstractions for features that are not in the current milestone.
- Commit per logical step with the milestone prefix, e.g. `M1: lossless frontmatter patcher`.

## Commands

```
pnpm install
pnpm tauri dev                          # run the app on $WORKLY_WORKSPACE (settings arrive in M2)
pnpm dev:fixture                        # run the app on a fresh copy of the fixture in /tmp/workly-dev; ⌘0 = debug view
pnpm check                              # svelte-kit sync + svelte-check
cargo test --workspace                  # core + cli tests
cargo clippy --workspace -- -D warnings
cargo run -p wly -- --version
pnpm tauri build                        # -> target/release/bundle/macos/Workly.app
pnpm tauri icon docs/design/icon/workly-icon-1024.png  # regenerate icons (delete non-macOS output)
```

Cargo uses one workspace `target/` at the repo root, not `src-tauri/target/`.

Keep this section up to date when scripts change.

## Definition of done (every milestone)

- `cargo test --workspace`, `cargo clippy --workspace -- -D warnings`, `pnpm check` are green.
- New behaviour has tests (core) or a manual check list (UI) in the milestone report.
- UI matches the reference screenshots in light and dark mode.
- No console errors, no idle CPU load.
