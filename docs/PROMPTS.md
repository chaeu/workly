# Workly – Bauanleitung mit Claude Code

Pro Meilenstein eine frische Claude-Code-Session, Prompt unten reinkopieren, Plan prüfen, bauen lassen, abnehmen, committen. Nicht zwei Meilensteine in einer Session – der Kontext wird sonst zäh und die Abnahme unscharf.

## Setup (einmalig, ~15 min)

1. Voraussetzungen: Xcode Command Line Tools (`xcode-select --install`), Rust (`rustup`), Node 22+, pnpm, Claude Code.
2. Paket entpacken nach z. B. `~/dev/workly`, dann:
   ```
   cd ~/dev/workly
   git init && git add . && git commit -m "Docs, design, fixtures"
   claude
   ```
3. `/init` **nicht** ausführen – `CLAUDE.md` existiert schon und soll so bleiben.
4. Optional: privates GitHub-Repo anlegen und pushen, dann ist der spätere Umzug auf den Arbeits-Mac ein `git clone`.

## Modell und Modus

| Meilenstein | Modell | Warum |
| --- | --- | --- |
| M1 Core, M4 Cockpit, M6 Agent-Start | stärkstes verfügbares (Opus) | Verlustfreies Schreiben, Kantenrouting, Prozess-Spawning: hier kosten Fehler später am meisten |
| M0, M2, M3, M5, M7 | Sonnet reicht, Opus schadet nicht | Gut spezifiziert, viel Fleißarbeit |

Jeden Prompt im **Plan-Modus** starten (Shift+Tab). Den Plan lesen, Einwände direkt zurückgeben, dann freigeben. Bei UI-Meilensteinen nach dem Bau Screenshots machen lassen oder selbst neben `docs/screenshots/` legen.

## Abnahme-Routine (nach jedem Meilenstein)

```
cargo test --workspace && cargo clippy --workspace -- -D warnings && pnpm check
pnpm tauri dev
```
Dann die Checkliste des Meilensteins durchgehen. Erst wenn alles passt: `git commit`, Session schließen.

---

## M0 – Gerüst

```
Read CLAUDE.md, docs/SPEC.md (sections 1-2) and docs/design/DESIGN.md.

Milestone M0: project skeleton.
- Tauri 2 + SvelteKit (adapter-static, ssr off, Svelte 5, TS strict) with pnpm.
- Cargo workspace with crates/workly-core (empty lib with one smoke test), crates/wly (clap, `wly --version`), src-tauri.
- Bundle id dev.chaeu.workly, product name Workly, minimum macOS 13, window 1280x800, min 960x600, native title bar with overlay style if it looks clean.
- Copy docs/design/tokens.css and components.css to src/lib/styles and load them globally. Self-host IBM Plex Sans and Mono (woff2, only the weights the design uses) – no Google Fonts at runtime.
- App icon: run `pnpm tauri icon docs/design/icon/workly-icon-1024.png` (do not redraw it). Brand mark in the sidebar = docs/design/icon/workly-mark.svg (inline SVG, 24 px) next to "Workly".
- App shell from the reference prototypes: sidebar (nav: Tasks, Projects, Use cases, Agents; workspace name at top), main area with page title. Empty pages for each view, routing via SvelteKit.
- Theme: follow system, plus a manual override (light/dark/system) kept in memory for now.
- Keyboard: ⌘1-4 switch views.
- Fill in the Commands section of CLAUDE.md.

Acceptance: `pnpm tauri dev` shows the empty shell in light and dark, fonts are Plex, sidebar matches docs/screenshots/task-board-light.png in spacing and colours. `pnpm tauri build` produces a .app. All checks green.
Plan first, then build. Stop after M0 and report.
```

**Checkliste:** App startet, hell/dunkel stimmt, Schrift ist Plex, ⌘1–4 wechselt, `.app` liegt unter `src-tauri/target/release/bundle/macos/`, Leerlauf-CPU ≈ 0 % (Aktivitätsanzeige).

---

## M1 – Core (Opus)

```
Read CLAUDE.md and docs/SPEC.md (sections 2-3) carefully. Look at every file in fixtures/workspace, including .workly/.

Milestone M1: workly-core. No UI work in this milestone except one debug view.

Build in this order, with tests for each step:
1. Frontmatter split: separate frontmatter text and body without changing either. Handle: no frontmatter, empty frontmatter, missing final newline, CRLF, BOM.
2. Read: parse frontmatter with serde_yaml (or yaml-rust2) into typed structs (Project, UseCase, Task, AgentState) plus a map of unknown fields. Invalid YAML returns a structured error with path and line; it never aborts the scan.
3. Lossless patcher: set/remove a top-level key or a key inside one nested map (agent.*, usecase.*) by editing lines, not by re-serialising. Preserve comments on the same line, key order, indentation, blank lines, quoting style of untouched values. New keys are appended at the end of their map. Values are emitted with minimal quoting. Lists (tags) are written in flow style `[a, b]` only if the original was flow style or absent; block lists are rewritten as block lists.
   Golden tests: patch every field of WR-8-content-migration.md one at a time and assert that the diff is exactly the changed line(s). A property test (proptest) that patch(x -> y -> x) is byte-identical.
4. Body helpers: append a line under `## Updates` (create section at end if missing).
5. Scan: walk the workspace (respect scan_exclude from config.yml), find _project.md, tasks/*.md per project, inbox/*.md. Build an in-memory index. Measure: scanning the fixture must take < 20 ms in release.
6. IDs: next id = max number for that key across workspace AND .workly/trash + 1 (fixture: next WR is WR-10). Key suggestion as in the spec; IN reserved; collision appends a digit. Moving a task between projects keeps its id and filename.
7. Writes: atomic (temp file + rename), log line to .workly/log/YYYY-MM.jsonl {ts, actor, kind, id, field, from, to}.
8. Trash: delete moves to .workly/trash/<relative path>, restore moves back (error if target exists).
9. Watcher: notify crate with debounce (~150 ms). Emits typed change events. Writes made by the core itself are suppressed (track path + mtime/hash of own writes).
10. Config + process: load config.yml and process.yml into typed structs; validate process (edges reference existing steps, phase_default_step points to existing steps) and return readable errors.

Expose from src-tauri: get_index, update_task_field, create_task, move_task, delete_task, restore, plus an event "workspace-changed". Add a hidden debug page in the app that lists projects and tasks from the index and updates live.

Acceptance: cargo test green including golden roundtrip and proptest; IN-3 shows up as a parse error, everything else loads; editing a file in another editor updates the debug page in < 1 s; the app's own writes do not trigger a reload loop.
Plan first (list the crates you want to use and why), then build. Stop after M1 and report, including any spec ambiguities you hit.
```

**Checkliste:** WR-8 in VS Code öffnen, in der Debug-Ansicht Status ändern lassen, `git diff` zeigt genau eine Zeile. Datei in Obsidian/VS Code ändern → Debug-Seite aktualisiert sich. `IN-3` erscheint als Fehler.

---

## M2 – Projects und Settings

```
Read CLAUDE.md, docs/SPEC.md (sections 3, 4, 6) and the projects part of docs/design/DESIGN.md. Check docs/screenshots/task-board-projekte.png.

Milestone M2: Settings and Projects.
Settings
- Device settings at ~/Library/Application Support/Workly/settings.json: list of workspaces (name, path), active workspace, repos folder, theme. Switching workspace rebuilds the index.
- First start without workspace: choose an existing folder or create a new one. Creating initialises _templates/, inbox/, projects/, knowledge/ and .workly/ (config.yml, process.yml, agent/AGENTS.md) from built-in defaults – use the fixture files as the defaults.
- Folder pickers via the Tauri dialog plugin.

Projects
- List with colour square, key, title, open task count, use-case step if any. Active first, archived collapsed. Sort by drag (writes `order`).
- Create from _templates/project.md: title, key (suggested, editable, validated, then fixed), colour (next free proj-n), optional repos. Creates the folder structure tasks/, docs/, notes/, decisions/, agent/AGENTS.md and <key>.code-workspace (project folder + repos).
- Edit title, colour, status, repos, links. Archive. Delete (to trash, with confirmation).
- Project detail: overview (rendered body), folder tree of the project (docs, notes, decisions, agent), Markdown preview of the selected file (read-only, use a small renderer such as markdown-it or marked; sanitise). Buttons: Open in Obsidian (obsidian://open?path=...), Open in VS Code (the .code-workspace), Reveal in Finder. Repo links open in VS Code.

Acceptance as in the spec: new project creates the structure; changes made in Obsidian appear without restart; all open buttons hit the right target. Light and dark.
Plan first, then build. Stop after M2 and report.
```

**Checkliste:** Leeren Ordner als Workspace anlegen, Projekt „Test Alpha“ → Key-Vorschlag `TA`, Ordnerstruktur da. `_project.md` in Obsidian ändern → Liste aktualisiert. Obsidian-, VS-Code- und Finder-Button testen.

---

## M3 – Tasks

```
Read CLAUDE.md, docs/SPEC.md (sections 3, 6) and docs/design/DESIGN.md fully. Open docs/reference/task-board.html in a browser and use it as the behavioural reference; match docs/screenshots/task-board-*.png and task-detail.png.

Milestone M3: Tasks view.
- Board with columns from config.yml task_statuses, WIP limit indicator. Toggle grouping by project (swimlanes) as in the reference.
- Card anatomy exactly as DESIGN.md: id (mono), priority, title, project square + name, due date, tags, agent badge when agent.active is set ("Codex working · 14 min", minutes tick live).
- Drag and drop between columns and within a column; dropping into another project lane moves the task file (id unchanged).
- Focus strip: up to three tasks for today (focus == today, ordered by focus_order), show/hide toggle remembered in device settings, add/remove/reorder.
- Quick add (⌘N): title, project (or inbox), priority; next id from core.
- Filters: project, priority, tag, due (overdue/this week), text search (⌘K) over id and title.
- Detail card as popup or side panel (user setting), fields: status, priority, due, tags, description (rendered, "Edit in Obsidian/VS Code" button), updates list, agent section (ready, runner, model, effort – editable; active/since/commit read-only). Delete with confirmation.
- Every edit goes through the core patcher; no optimistic state that can drift from the file.

Acceptance: every change lands losslessly in the file (verify with git diff on the fixture copy); external edits show up live; side-by-side with the reference screenshots in light and dark.
Plan first, then build. Stop after M3 and report with screenshots.
```

**Checkliste:** Karte ziehen → `git diff` eine Zeile. Fokus setzen/entfernen. Detailkarte als Popup und als Seitenleiste. Task in anderes Projekt ziehen → Datei verschoben, ID gleich. Screenshots neben Referenz.

---

## M4 – Use-Case-Cockpit (Opus)

```
Read CLAUDE.md, docs/SPEC.md (sections 3, 6) and docs/design/DESIGN.md. Open docs/reference/use-case-cockpit.html in a browser, read its source (process data, moveTo/moveUseCase, edge routing) and match docs/screenshots/cockpit-*.png.

Milestone M4: Use-case cockpit, driven entirely by .workly/process.yml (no step, lane or label hardcoded).
- Board: one column per phase, gate markers on column borders (board_gates), parked column. Cards show type, area, status pill, days in step (stale after stale_after_days), next step. Drag a card into a column = move to phase_default_step.
- Process map: swimlanes x columns grid from steps (col, lane), boxes, gates (diamond), terminals; edges with the routes h, hv, hvh, vu, over, offset, label_dx exactly as in the prototype; "No ..." labels styled as negative. Use-case chips sit on their current step. Recompute edges on resize.
- Detail card (popup and side panel like tasks): progress bar over phases, status pills, next step, current state, decision buttons generated from outgoing edges of the current step (negative branch as secondary button), decision history (usecase.decisions), linked tasks of the project, open project.
- One move function for all paths (drag on board, drag on map, decision button): updates usecase.step, step_since, appends to usecase.decisions when leaving a gate, logs the change.
- Status is a property, not a position: changing status never moves the card.
- Filters: type, area, status, search.

Acceptance: behaves like the prototype; editing process.yml (rename a label, add a step) updates the views without rebuild; invalid process.yml shows a readable error instead of a blank view.
Plan first, then build. Stop after M4 and report with screenshots.
```

**Checkliste:** Use Case per Drag und per Entscheidungsbutton bewegen → `_project.md` zeigt neuen `step`, `decisions` wächst. Label in `process.yml` ändern → sofort sichtbar. Kaputte `process.yml` → Fehlermeldung.

---

## M5 – CLI `wly` und Agent-Dateien

```
Read CLAUDE.md and docs/SPEC.md (section 5). Look at fixtures/workspace/.workly/agent/AGENTS.md and projects/website-relaunch/agent/.

Milestone M5: the wly CLI on top of workly-core.
- Commands exactly as in the spec: task list/show/start/note/review/add, project list. --json on every read command, stable field names, documented in docs/CLI.md.
- Workspace resolution: --workspace, then WORKLY_WORKSPACE, then active workspace from device settings.
- Actor: --agent <name> or WORKLY_AGENT sets actor agent:<name> in the log; otherwise cli.
- `task show --json` returns task, project, and the assembled agent context: global AGENTS.md, project AGENTS.md, list of skills (name, description, path).
- Transitions: start allowed from backlog/todo/review; review allowed from doing. Setting done is impossible for agents (exit 3). Exit codes 0/1/2/3 as in the spec, errors on stderr, JSON errors with --json.
- `wly` must work while the app is running; the app picks up changes via the watcher.
- Install path: `cargo install --path crates/wly`, plus a Settings button "Install CLI" that symlinks the bundled binary to ~/.local/bin/wly (no sudo).
- App: show agent files in project detail (AGENTS.md preview, skills list); "Create AGENTS.md" from template if missing.
- Integration tests run the binary against a temp copy of the fixture.

Acceptance: an agent (Codex CLI or Claude Code) given only "Work on WR-5 in this repo, follow the Workly rules from `wly task show WR-5`" starts the task, writes notes and sets it to review via wly; the board shows it live.
Plan first, then build. Stop after M5 and report.
```

**Checkliste:** In einem Terminal `wly task start WR-5 --agent codex` → Karte zeigt Badge. `wly task review WR-5` → Review. `wly task ... done` als Agent → Exit 3.

---

## M6 – „Start agent“ (Opus)

```
Read CLAUDE.md and docs/SPEC.md (section 5, Runner-Adapter).

Milestone M6: start an agent from a task.
- Runner adapter trait in workly-core: build_prompt(task) -> String, start(task, prompt) -> Result. Prompt = global rules + project AGENTS.md + skill list + task (id, title, body, acceptance criteria) + the exact wly commands to use.
- Adapter codex: open a new Terminal window (AppleScript via osascript; fall back to iTerm if configured) in the first repo of the project, run `codex` with the prompt (pass via a temp file, not shell-escaped inline text), env WORKLY_WORKSPACE and WORKLY_AGENT=codex set. Model/effort from the task's agent fields or config defaults, mapped to codex flags.
- Adapter copilot: open <key>.code-workspace in VS Code (`code` CLI or `open -a`), copy the prompt to the clipboard, show a toast "Prompt copied – paste into Copilot Chat". Status updates come from Copilot calling wly in the terminal, or the user.
- Adapter selection: task agent.runner, else config default; "auto" = config default.
- UI: "Start agent" button in the task detail card and in the card's context menu, disabled without repo (with reason). Shows runner, model, effort before start; "Copy prompt" always available.
- Live status: card badge from agent.active/since, updates via watcher; a task stuck in doing with an agent for > 2 h shows a warning.
- Never block the UI thread; spawn errors appear as toasts with the stderr tail.

Acceptance: one click goes from task to a running Codex session in the right repo with the full prompt; the agent's wly calls show up on the card; Copilot path opens the workspace and puts the prompt in the clipboard.
Plan first, then build. Stop after M6 and report.
```

**Checkliste:** Start agent bei WR-5 → Terminal im Repo, Codex läuft mit Prompt, Badge erscheint, Review kommt zurück. Copilot-Pfad: VS Code öffnet richtigen Workspace, Prompt in Zwischenablage.

---

## M7 – Feinschliff und Installation

```
Read CLAUDE.md and docs/SPEC.md (section 6, non-functional).

Milestone M7: polish and release.
- Shortcuts: ⌘N quick add, ⌘K search, ⌘1-4 views, Esc closes cards, ⌘⌫ delete with confirmation, ⌘, settings. Shortcut overlay on ?.
- Empty states (no workspace, no projects, empty board) calm and minimal as in DESIGN.md principle 6.
- Error states: parse errors list (file, line, open in editor), unreachable repo, missing CLI. Never a blank screen.
- Trash view: list, restore, empty trash.
- Performance: measure start and scan with 500 generated tasks (script in scripts/), keep start < 1 s, idle CPU ~0. Report numbers.
- About window with version and the app icon (docs/design/icon/).
- Release build, ad-hoc signed, scripts/install.sh copies Workly.app to /Applications and installs wly. Document in README how to move to another Mac (clone repo, build, point settings to the workspace).

Acceptance: one day of real use without touching the files by hand except in Obsidian/VS Code.
Plan first, then build. Stop after M7 and report.
```

---

## Wenn etwas schiefläuft

- **Claude Code weicht vom Design ab:** Screenshot der App plus Referenz-Screenshot reingeben: „Match this reference. List the differences first, then fix them.“
- **Verlustfreies Schreiben bricht:** sofort stoppen, fehlschlagenden Fall als neue Fixture-Datei ergänzen, Test schreiben lassen, dann fixen.
- **Kontext wird lang:** `/compact` oder neue Session mit „Continue milestone Mx. Read CLAUDE.md and the last commit messages first.“
- **Spec-Lücke:** Entscheidung treffen, in `docs/SPEC.md` nachtragen, committen. Die Datei bleibt die Wahrheit.
