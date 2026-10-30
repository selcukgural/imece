# Live v0

Live v0 (2026-10-28): one long-lived `claude -p` stream-json session per
agent (never a process per turn), `--restricted`, exact `--tools`, no MCP
server but imece's own, `--permission-prompts none`; the `init` event is
checked every turn and a mismatch stops the session (fail closed). Leader: read tools, the
checkout itself. Workers: edit tools, one detached worktree each under
`$XDG_STATE_HOME/imece/worktrees`, kept between runs. Budget (default
$50, user 2026-10-29) is one pot for the team: no turn once it is spent;
each session's `--max-budget-usd` is what was left when it started, a
last stop against a runaway session, not a share. `:budget USD` (key `:`
opens a command line, `:b` is short) changes the pot while running: a
session whose turn ends at its old cap (`error_max_budget_usd`) while
the pot has room is ended and started again with the same prompts and
the new remainder, once per session until a turn finishes (`renewed`).
Sessions start off the UI
thread; events carry a session generation so late ones are dropped.
