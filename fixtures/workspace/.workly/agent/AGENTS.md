# Global agent rules (Workly)

These rules apply to every agent run in this workspace, on top of project instructions.

1. Change task state only through the `wly` CLI. Never edit task files directly.
2. Start: `wly task start <ID> --agent <name>`.
3. Progress: `wly task note <ID> "<short update>"` after each meaningful step.
4. Finish: `wly task review <ID> --commit <sha>`. Never set a task to `done`; a human does that.
5. Work only inside the repo or worktree you were started in. Delete nothing outside it.
6. If blocked, write a note starting with `BLOCKED:` and stop.
