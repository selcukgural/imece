# Helpers

Helpers (2026-10-29, user's decision): a worker is to imece what an
agent is to Claude Code, so it splits its task the same way: the
worker's socket offers `hire` too, and a worker hires HELPERS (role
`Helper`, `parent` = the worker, named `<worker>.N`), `--helpers N` is
the cap per worker (0-8, default 2). A helper runs in its worker's
worktree (same files; the worker says which), with the edit tools and
send/post/board only (no claim, no hire: no helpers of helpers), is not
offered cards and is not counted against `--agents`. Its `done` to its
worker marks it leaving; it leaves when that turn ends (so the turn's
cost is still reported): session ended, mail dropped, `gone`, journaled
as `Left`. A worker that stops takes its helpers with it. The vendor's
own sub-agent tool stays off the `--tools` list on purpose: a sub-agent
spawned inside the CLI would skip the worktree, the budget, the bus and
the screen. Salon: helpers sit under their worker at depth 3 (`├`/`└`),
the worker line ends with `▾ 2 helpers` / `▸ 2 helpers`; `h`/`l`/`za`
on a worker with helpers (or on a helper) fold its helpers before the
project, `zM`/`zR` fold and unfold workers too.

## A helper lost before its done (2026-11-03, backlog F5 in #59)

A worker that gave a helper a part waited for its `done`; when the
helper's session ended, or its last turn failed for good (an account
fault, the budget, a vendor error imece gave up retrying), nothing told
the worker, which then waited on a helper that would never answer.
Now `Live::helper_lost` (called from `step` for a helper's `Stopped`,
and for a failed `TurnDone` that was its last queued turn and left no
retry) puts a note for the worker: `Your helper claude-1.1 stopped before
it sent you done (WHY). The part you gave it is not done: do it
yourself, or hire another helper for it.` (WHY: the first non-blank line,
at most `LOST_SAID` 200 characters). The note goes with the worker's next
prompt; one that is idle when nothing else wakes it (`Live::lost`) is
woken with `GO_ON` once its turn count is 0, unless it is stopped, asleep,
gone, at `MAX_WOKEN` or over the budget; it counts as a message-driven
turn. A note read with any prompt drops the wake (`with_notes`). A helper
that sent `done` (leaving), a worker's or leader's own end, or a worker
gone tells nobody. The journal already has the helper's `Stopped`; the
worker's trace says `told: claude-1.1 stopped`.

"Helpers end with their worker" was already so: a worker's `Stopped`
releases its helpers, a closed project ends everyone, and nothing else
makes a worker leave. A helper's `blocked` goes to its worker on the bus
as any message does. The demo mirrors it (`Sim::helper_lost`): a helper's
failed turn without a retry, or its stop, puts the trace line on its
worker and puts an idle worker back to work.

