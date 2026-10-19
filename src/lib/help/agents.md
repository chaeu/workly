## Agents

Coding agents (Codex, GitHub Copilot) can work on your tasks. They run in their own tools, in Terminal and VS Code. Workly hands them the task, shows what they are doing and lets you review and commit the result. You switch this part on or off in Settings → Agent features; off hides every agent button, and the files and `wly` keep working.

**The `wly` CLI** is how an agent reads its task and reports back. Install it in [Agents → Setup](/agents?tab=setup) with "Install CLI": it links `~/.local/bin/wly` to Workly. An agent runs `wly task show WR-12` for its brief, `wly task start` (the task moves to Doing), `wly task note` for progress and `wly task review --summary "…"` when it is finished (the task moves to Review). You can use `wly` in Terminal yourself; `wly --help` lists the commands.

**Starting an agent.** The task's project needs a repo (Edit → Repos). In the task's detail card, pick an agent from the Agent menu and click "Start …":

- **Codex** opens in Terminal, in the project's first repo.
- **Copilot** opens VS Code (1.98 or later) with the prompt in Copilot Chat, in agent mode.

"Copy prompt" gives you the same prompt to paste wherever you like. The agents in the menu, with their model, effort and permissions (Ask, Auto, Full for Codex), are set in Agents → Setup. Only one agent works in a repo at a time.

**While it works** the card shows "Codex working · 14 min" and its notes appear under Agent activity. After 30 minutes without a sign of life (the default) the card says "Agent inactive?" and "Clear agent" resets it. An agent that cannot go on writes a note starting with BLOCKED.

**Review.** The Agents view collects what needs you: blocked agents and finished work. A review shows the agent's summary and the changed files with their diffs. "Commit" commits exactly those files to the repo (never pushes); "Commit & Done" also closes the task.

**Rules agents follow.** They get the global rules from `.workly/agent/AGENTS.md`, the project's `agent/AGENTS.md` and its skills (`agent/skills/`); you find all of them in Agents → Setup. The built-in rules say: change task status only through `wly`, work only in the repo they were started in, never commit, push or create repos, and finish with one review. **Agents never set a task to Done** – `wly` refuses it. You decide when the work is done.

[Open Agents → Setup](/agents?tab=setup)
