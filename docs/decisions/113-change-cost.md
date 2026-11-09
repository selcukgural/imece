# What a change cost

The second piece of record 111's first round (#289, 2026-11-04): every
change that reaches the Δ gate says what the worker spent on it, so the
person can judge the leader's hiring (a strong-tier worker on a rename) or
a worker that burnt $4 on a one-line fix, and the leader learns what its
tiers cost. The pot (record 02) says what the team spent; this cuts the
same spend by change, nothing paid twice.

The figure. `Spend { cost, ticks, rounds }`: the cost in ten-thousandths
of a dollar (as `Agent.cost`), the ticks worked, and the rounds. It is
the worker's spend, its helpers' counted in, from its first prompt after
its last change to this `done`. The round is the unit: `Agent.task:
Task { round, carried, last }`. A round opens at a prompt when none is
open (`World::round_opens`, from `Live::prompt` for every role but the
leader, whose spend is nobody's change; the demo at an agent's first busy
tick), noting the agent's cost and the tick; it closes when a change of
the agent's is settled (`World::spend_of`), which takes the cost since it
opened, the ticks since (the worker's own: helpers' time runs beside it),
one round, and the helpers' open rounds' cost; the helpers' rounds close
with it, a helper hired mid-round counts from its first prompt. A round
not closed (a question answered between changes) counts into the next
change. Not the time the change waited for the person: a round opens at
the next prompt, after the wait.

When. A worker's `done` comes mid-turn, and Claude Code says a turn's cost
only at its end, so `changes::read` puts the change before the person
with `Change.spend: None` and `World::settle_change` fills it at the
worker's `TurnDone` (`sessions::apply`, ok or not), with
`Event::ChangeCost`; a change read with its agent's turn over (a solo's at
its turn's end, the rest of a part applied, the demo's done, which sets
the agent idle first) is settled in `change_ready` itself and
`Event::ChangeReady` carries the figure, one journal line. A round's
spend not yet in a change is lost only when no change is ever read for it.

Rounds. A change that leaves unapplied carries its figure into the
agent's next (`Task.carried`, set by `carry_spend` on `change_rejected`,
on `change_ready` replacing an older change, on `change_applied` of a
part): the next settle adds its own round, `$1.20 · 25 min · 3 rounds`.
Applied whole, `carried` is dropped; a worktree that holds nothing
(`change_none`: a done with no change, no directory to read, the rest of
a part applied being empty) drops it too and takes the newest figure
with it. `Task.last` is the newest change settled, for the leader's mail.

What the person sees. The Δ row in WAITING FOR YOU ends ` · $0.84 · 12
min`, dim, after the check mark and the runs (`ui::inbox_line`,
`words::spend`: whole seconds under a minute, else minutes, hours and
minutes, ` · N rounds` when more than one); above the diff, before the
kinds and files, `cost · $0.84 · 12 min` (`ui::diff::spend_line`); the
journal `claude-1 finished: 3 files, +34 −8 · $0.84 · 12 min → waits for
you` when known at the read, else `claude-1's change cost $0.84 · 12 min`
when the turn ends (`EntryKind::Assumption`, the agent's, so the card's
screen and `:why` have it). Nothing until it is known.

The leader. A worker's `done` is headed `[done 12 · $0.84]`
(`prompts::heading`, from `Task.last`), and its delivery waits for the
worker's turn end: `Live::post_office` holds a recipient's delivery while
its mail has a worker's `done` whose sender is mid-turn (`settling`:
role Worker, not gone, `turns[from] > 0`); the worker hears `delivered as
message N; lead gets it when this turn of yours ends, headed with what it
cost`. A helper's `done` to its worker is not held (its cost is its
worker's). A `done` whose worktree held nothing has no figure. A leader
busy for long enough that the worker settled a newer change reads the
newer figure on the older `done`: accepted, the mail carries no figure of
its own.

Not done, by choice for now: a price per applied
change across a project (G2's ledger, #60; what a figure means, `≈` on a
plan and `+?` for what was never priced, is record 132); the figure in the `board`
tool's lines; re-settling a change for a helper whose last turn ends
after its worker's (its cost goes to the next round).

Growth: three fields per agent and one `Option<Spend>` per waiting
change; the journal's lines as any event's; the store adds one text
column to `changes` (`spend`, JSON or `''`) and one to `agents` (`task`),
schema 24; the save plan keys a change's row on tree, check and spend,
and an agent's row on its task, so a settle or an opened round is
written. Concurrency: settled on imece's own thread at the session's
event; the git thread's read and the turn's end come in either order and
both paths settle once (`spend.is_none()`). Read cost: `inbox_line` and
the diff header read one field; `settling` scans a recipient's mail
once a step. Ten times the scale: nothing grows with it beyond the
agents. Migration: an older file has no columns (`''` → no spend, a
default task); an older imece refuses the file as any newer file.
Failure: a session that ends mid-turn after a `done` leaves the change
unsettled until a later turn's end (then settled with the whole since
the round opened); the leader's `done` is delivered when `turns` reset
with the session's end, headed without a figure.

Demo (record 31): the fake workers' spending runs by tick; a round opens
at an agent's first busy tick, the done settles as it is read (the
demo's turn is over then), a rejected change's rounds add up, a helper's
cost counts, an apply drops the carried figure.

Since record 126 (2026-11-08) the card moves with the change as well: a
change sent back puts its card back to work, so a card's rounds are the
rounds of the change that carries it.
