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
imece's own paid calls (a suggested commit message, record 56) are paid
from the same pot (`World::paid`, in `World::spent`).

The rate (2026-10-30, after the first run on Kule burnt ~$15/h with three
Claude sessions): the header's `$/h` is what the last ten minutes cost
(`RATE_WINDOW`), not the average since the run began, which an idle hour
pulled down and a first turn pushed up. `World::watch_spending`, called
on every step, notes the team's total each time it changes and keeps
only what the window starts from; this run alone counts (`run_spent`).
Next to it the header says how long the pot lasts at that rate
(`~2 h 10 min left`); under `POT_LOW_MINUTES` (30) the whole budget
turns the attention colour and the journal says once, with the rate
and the minutes, that the pot runs low (`Event::PotLow`, under the
first open project's leader); `:budget` makes it say so again. It is a
journal line, not a WAITING FOR YOU item: nothing in it waits for an
answer, and the pot stops every turn on its own when it is spent.
Known gaps: costs arrive at turn ends, so the rate moves in steps; a
run younger than ten minutes counts the minutes before it as idle;
there is no rate per agent (each agent's own total is in its salon
row).
