# wly – Workly CLI

`wly` reads and changes tasks in a Workly workspace. Coding agents use it as their only way to change task state; humans can use it too. It runs on the same core as the app and works while the app is open: the app picks up every change through its file watcher.

## Install

```
cargo install --path crates/wly      # -> ~/.cargo/bin/wly
```

or in the app: Settings → Agent features → **Install CLI**. That links `~/.local/bin/wly` to the app binary (no sudo), which runs the CLI when it is started as `wly`. An existing `~/.local/bin/wly` that is not a link is never replaced.

## Global options

| Option | Env | Meaning |
| --- | --- | --- |
| `--workspace <path>` | `WORKLY_WORKSPACE` | Workspace folder. Default: the active workspace from the app settings (`~/Library/Application Support/Workly/settings.json`). `~/` is expanded. |
| `--agent <name>` | `WORKLY_AGENT` | Acting agent. The log records `agent:<name>`, otherwise `cli`. 1–32 of `A-Z a-z 0-9 - _ .` |
| `--json` | | JSON on stdout; errors as JSON on stderr. |

Options can stand anywhere on the line: `wly task show WR-5 --json`.

## Commands

```
wly task list [--project WR] [--status todo] [--json]
wly task show WR-12 [--json]                  # task + project + agent context
wly task start WR-12 [--agent codex]          # -> doing; with an agent also agent.active, agent.since
wly task note WR-12 "text"                    # appends "- YYYY-MM-DD HH:MM · <who>: text" under ## Updates
wly task review WR-12 [--commit a3f9c1e]      # -> review; clears agent.active/since, stamps agent.last_run
wly task add "title" [--project WR | --inbox] [--priority 1-3]   # prints the new id
wly task done WR-12                           # humans only, see below
wly project list [--json]
```

`<who>` in a note is the agent name, or `cli` without one.

### Transitions

| Command | Allowed from | Else |
| --- | --- | --- |
| `start` | backlog, todo, review | exit 3 |
| `review` | doing | exit 3 |
| `done` | any status, actor `cli` only | exit 3 when `--agent` / `WORKLY_AGENT` is set |

Agents never set `done`. `wly task done` is not in the spec's command list; it exists so a human can close a task from the terminal and so the refusal for agents is testable. It is a rule for well-behaved agents, not a security boundary: anything that can run `wly` can also edit the files.

`start` and `review` each write the task file once (atomically) and log one line per changed field.

## Exit codes

| Code | Meaning |
| --- | --- |
| 0 | ok |
| 1 | usage error: unknown command or option, invalid value, no workspace configured |
| 2 | not found: task, project, or the path is not a workspace |
| 3 | invalid transition |

Errors go to stderr as `wly: <message>`, with `--json` as:

```json
{"error":{"code":2,"message":"task WR-99 not found"}}
```

## JSON output

Field names are stable. All `path` fields are absolute. Missing values are `null`, never left out (except `body`, which only `task show` has).

### Task (`task list` items; `start`, `note`, `review`, `add`, `done` with `--json`)

```json
{
  "id": "WR-7",
  "title": "Lighthouse audit and fixes",
  "status": "review",
  "priority": 2,
  "due": "2026-10-03",
  "tags": ["perf"],
  "project": "WR",
  "path": "/Users/me/Workspace/projects/website-relaunch/tasks/WR-7-lighthouse-audit.md",
  "created": "2026-09-28",
  "done_at": null,
  "agent": {
    "ready": false,
    "runner": "codex",
    "model": "auto",
    "effort": "low",
    "active": null,
    "since": null,
    "commit": "a3f9c1e",
    "last_run": "2026-10-02T15:10:00Z"
  }
}
```

`project` is the project key, `null` for inbox tasks. `agent` is `null` when the task has no `agent:` block.

### Project (`project list` items)

```json
{ "key": "WR", "title": "Website Relaunch", "status": "active", "path": "/Users/me/Workspace/projects/website-relaunch", "repos": ["~/repos/website"] }
```

`repos` are as written in `_project.md` (`~/…`).

### `task show --json`

```json
{
  "task": { "...": "task fields as above", "body": "Markdown body without frontmatter, including ## Updates" },
  "project": { "...": "project fields as above, or null for inbox tasks" },
  "context": {
    "global":  { "path": "/…/.workly/agent/AGENTS.md", "content": "…" },
    "project": { "path": "/…/projects/website-relaunch/agent/AGENTS.md", "content": "…" },
    "skills": [
      { "name": "release-check", "description": "Checks a build before release (links, Lighthouse, sitemap).", "path": "/…/agent/skills/release-check/SKILL.md" }
    ]
  }
}
```

`global` and `project` are `null` when the file does not exist. Skills come from `<project>/agent/skills/<folder>/SKILL.md`; `name` and `description` from its frontmatter, `name` falls back to the folder name.

Without `--json`, `task show` prints the same as Markdown: task, project, workspace rules, project instructions and the skill list. That text is meant to be read by an agent as its working brief.
