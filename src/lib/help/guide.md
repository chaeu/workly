# Workly

Workly keeps your projects, tasks and use cases in plain Markdown files in one folder, your **workspace**. The files are the truth: Workly has no database and no server, and nothing leaves your Mac.

You can open the same folder in Obsidian and VS Code at the same time. A change made there shows up in Workly within a second, and Workly's changes show up there. Workly only changes the lines it has to: unknown fields, comments and the order of your frontmatter stay as they are.

## Workspace

A workspace is a folder. Pick or create it in [Settings](/settings); you can keep several and switch between them. Workly adds what is missing and never overwrites your files:

```
_templates/      templates for new projects, tasks, notes, decisions
inbox/           tasks without a project
knowledge/       general notes that belong to no project
projects/
  website-relaunch/
    _project.md  the project: fields on top, overview below
    tasks/       one file per task, e.g. WR-12-navigation.md
    docs/  notes/  decisions/
.workly/         Workly's own files: config.yml, process.yml, log, trash
```

**Settings live in two places.** Things that belong to this Mac (workspaces, theme, apps for files, backup) are in Settings. Rules for the workspace (task statuses, WIP limits, folders) are in `.workly/config.yml`, the use-case process in `.workly/process.yml`. Edit those two in VS Code.

**Backup.** The workspace is its own Git repo. Workly commits it on start, every hour (change it in Settings → Backup), on "Back up now" and when you quit, and pushes to the branch's upstream if there is one. Workly never sets up a remote: add one yourself with `git remote add` if you want an offsite copy.

**Keep the workspace out of OneDrive and iCloud.** Their sync breaks Git repos, so Workly does not back up a folder under `~/Library/CloudStorage/` or `~/Library/Mobile Documents/`. A local folder plus a Git remote is the safe setup.

**Deleting** moves things to `.workly/trash/`. Restore them from Trash in the sidebar. "Empty trash" moves them to the macOS Trash.

## Projects and tasks

Every folder with a `_project.md` is a project. Create one with **+ New project** on Projects. Its key (e.g. `WR`) is suggested from the title and fixed once created; task ids follow it: `WR-1`, `WR-2`, … A project page has the tabs Overview, Tasks and Files (plus Use case for use cases).

**Tasks** move through the statuses of your workspace, by default *Backlog → To do → Doing → Waiting → Review → Done*. Drag a card to change its status, or drag it within a column to reorder. Dragging to Waiting asks what you are waiting for. The board groups by status or by project and filters by project, priority, tag and due date.

**Quick Add.** Press ⌘N anywhere. Type the title, Tab to the description, Tab to the first acceptance criterion; Enter adds the next criterion, ⌘↩ creates the task. Inside a project, the task lands in that project; otherwise pick one, or leave it in the Inbox.

**Acceptance.** Criteria are a checklist under `## Acceptance` in the task file. Tick them in the detail card ("2/5" on the card); "+ Add criterion" adds one. Rewording or deleting a criterion happens in the file. Rule of thumb: steps under a day are criteria, real work packages are sub-tasks ("+ Add sub-task", one level deep).

**Focus.** The strip above the board holds up to three tasks for today. Drag cards up, or use the star in the detail card. It starts empty every day; "Hide" folds it away.

**Updates.** The field at the bottom of the detail card appends a dated line under `## Updates` in the task file: decisions, progress, a closing note.

## Use cases

A use case is a project with a `usecase:` block: an idea for an automation that moves through a fixed process. Create one with **+ New use case** in Use cases, or tick "Use case" in a project's Edit form. Untick "Project" to keep an idea in the cockpit only.

**Process.** Phases, steps and **gates** come from `.workly/process.yml` ("Edit process" opens it). A gate is a decision such as "Pilot?". On the use case's page, the buttons under Next or Decision move it on; a gate decision is recorded with its date. The cockpit shows all use cases as a list, a board, a process map or a matrix.

**Assessment.** "Assess…" scores the use case from 1 to 3 per criterion on two axes, **value** and **feasibility**. The averages place it in a quadrant: Quick win, Big bet, Fill-in or Drop. Criteria and their anchors come from `process.yml`. Assess before the pilot and again after it.

**Savings potential.** The manual work the use case would take over: per activity how often (per day, week, month or year) and how many minutes. Workly turns that into hours per year and FTE (hours per FTE are set in [Settings](/settings)). It is an estimate, not money saved.

## Editing files

In a project's **Files** tab, every Markdown file opens straight in the editor, like Obsidian: formatting shows rendered except on the line you are typing in, and the file saves itself as you type (⌘S saves at once). ⌘F searches. Workly edits only the text below the frontmatter; change fields like tags in Obsidian or VS Code.

"New file" and "New folder" create them anywhere in the project except `tasks/` and `agent/`. "Delete" moves them to the Trash. Links open on click: web links in the browser, links to other files of the project right here. Renaming and moving files is done in Obsidian or Finder.

If a file changes somewhere else while you have unsaved edits, Workly asks before it writes: "Reload" takes the other version, "Overwrite" keeps yours.

## Open with

Files and task cards have buttons to open the file in other apps, by default Obsidian and VS Code, plus Finder. Choose the apps and their order in Settings → Open files with, by their name in Applications (e.g. Zed). A whole project or repo still opens in VS Code.

## Requirements

- macOS 13 or later.
- Git, for the backup: run `xcode-select --install` in Terminal if `git --version` asks for it. Without Git, Workly works but does not back up.
- Optional: Obsidian and VS Code for the files and fields Workly does not edit.

## Keyboard shortcuts

⌘N new task, ⌘K search, ⌘1, ⌘2, … switch views, ⌘, Settings, ⌘? this page. [See all shortcuts](#shortcuts), or press ? anywhere.
