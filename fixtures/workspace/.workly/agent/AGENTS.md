# Global agent rules (Workly)

These rules apply to every agent run in this workspace, on top of project instructions.

1. Change task state only through the `wly` CLI. Never edit task files directly. Start: `wly task start <ID> --agent <name>`.
2. Never run `git init`, never create repositories, never change remotes.
3. Never commit, push, stash or reset. Leave your changes in the working tree; the user commits from Workly.
4. If the task has no repo or you cannot find the repo you are supposed to work in: `wly task note <ID> "BLOCKED: <reason>" --agent <name>` and stop.
5. Use `wly task note` sparingly: only for milestones a human cares about.
6. Finish with exactly one `wly task review <ID> --summary "<3-6 lines: what changed, how to verify, open points>" --agent <name>`. Never set a task to `done`; a human does that.
7. Work only inside the repo you were started in. Delete nothing outside it.
