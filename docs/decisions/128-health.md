# Health notices to the leader

F6 of backlog #59, in epic #286 (record 111's list after F8-F10). The
person's answers (2026-11-08): a serious sign wakes an idle leader, an idle
worker waits for its next turn; the minutes are the person's to set while
imece runs, ten by default; on a loop imece says so and cuts nothing, the
count the person's too, three by default.

What is seen (`health.rs`, on `World`, so live and the demo share it):

- **stalled**: a worker whose card is `Working` and its own has been
  `Idle` (`Sign::Stalled`) or `Asking` (`Sign::Waiting`, with whom) for
  `stall` minutes since its status changed;
- **silent**: an agent other than the leader `Working` with no trace line
  for `stall` minutes (`Sign::Silent`, with its card and `activity`, its
  last call): a hung session, or a command that runs long, which the
  words to the leader say both;
- **free**: a worker `Idle` with no card, or one over (done, failed,
  withdrawn), for `idle` minutes (`Sign::Free`); a card held for it, in
  review or in Lonca's queue is work;
- **looping**: one call (`tool target`, the target the newest `Tool` of
  that tool named) failed `fails` times in one turn (`Sign::Looping`), at
  exactly the count, once a turn and call; the turn's end (`TurnDone`) or
  the session's (`Live::end`) starts the counts again (`Watch::turn_over`).

Nobody is told what waits on the person (`waits_on_person`: a question, a
permission, a held item), a session starting, a turn imece will retry,
an agent gone or in a closed project, a worker whose helpers work, or
anything in a project with no leader to tell (a solo's: `told_of`). Each
quiet is told once, keyed on the tick it began (`Watch.told`: the status
change, or the last trace line); moving again starts a new one. Loops are
kept on the agent (`Watch.due`) until the next look.

`World::look_health` puts each sign in the journal (`Event::Health { agent,
sign }`: `Blocked` for what wakes, `Assumption` for a free worker; worded
with who heard it, `· lead told` / `· lead hears at its next turn`, a
leader's own loop without) and returns them; `Live::look_health` (each
step, after the post office, before the wakes) puts the leader's note
(`live/health.rs`, its own words, a call cut to 120 characters) in
`notes`, and sets `lost` for all but a free worker, so an idle leader is
prompted `GO_ON` with it, counted in `MAX_WOKEN` and refused over budget
as every wake is. The demo calls the same look each step and traces
`told: health of NAME` on the leader; its failed test runs are calls of
`run` that fail, every fourth (`LOOP_EVERY`) as many times as a loop
takes, and a worker's done ends its turn.

Mail addressed to no one is never dropped: a helper that leaves
(`Live::release`) with mail it never read tells each sender still on the
team which message went unread (`Live::unread`), with its next prompt.
Mail to a name not on the team was already refused to its sender; mail
queued before an agent stopped waits for its next session, and a new send
to a stopped agent is refused but for its leader's (record 146); a closed project's agents all
go together, so nobody is left to tell.

`:health [stall|idle|loop VALUE]` (`Knob`, `Health::with`): minutes or
hours from one minute to a day, failures from 2 to 20, `off`; bare, the
flash says all three; the `:` menu notes them and completes knob then
value; `World.health` is memory, prefs keep it as one line (`health`,
`stall 10m idle 10m loop 3`, `Health::name`/`parse`; a bad line reads as
nothing, the defaults).

Read cost: one pass over the agents a step (a card lookup each), nothing
that grows; the calls and failures of a turn are one entry per tool and
per failing call, cleared at its end. Ten times the agents is ten times a
short loop. Migration: none (prefs key new, the journal's event new;
an older imece reading a newer journal line it does not know drops it as
it does any). Failure: a note to a leader whose session stops is lost
with the session's notes, as every note is; the journal keeps it.

Gaps: a loop is seen only through failures (a call that succeeds the same
way over and over is not), and on the bus not at all (`MAX_WOKEN` caps
turns on messages); an idle leader that does nothing with a stall is told
nothing more until something moves; the minutes are one setting for every
project.

**Later (record 143).** A worker with an open block is no stalled one:
its leader heard from the worker itself. `stall` is also how long a block
may stay with the leader before it comes to the person.
