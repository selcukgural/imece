# A worker that stops is its leader's to solve, and nothing that stops development stays silent

Issue #455 (user, 2026-11-10): whatever stops the harness or keeps
workers from working never stays silent; the leader always tries to
solve it, brings the person the safest choices when it cannot, and gets
the worker going again with the answer. The person took the mock-up's
proposals as a whole (A · S1 · R1 · E, 2026-11-10).

## The inventory

Read from the code at 376df1b, before any of this was built. LOUD: an
item in TO DECIDE or a red row; QUIET: a journal line, a header word or a
row that looks idle; SILENT: nowhere.

- The whole team: the pot spent (QUIET: the header's `pot spent`; mail,
  wakes, retries and offers skipped without a word); `MAX_WOKEN` (12
  message turns since the person spoke: SILENT, only the sender's tool
  error); Lonca down, frozen or signed out (QUIET: `⌀ no lonca`, one
  journal line; workers go on and their `done` is refused); the leader's
  own session ended (LOUD, but every send of its workers, `done`
  included, is refused behind it); the leader's turn hung or failed
  (SILENT: health watches no leader); the usage limit at 100% (LOUD, late:
  three retries per agent, then one item per agent).
- One worker: its session not starting, ended, or stopped by the check of
  what it runs with (LOUD for the person; its leader never told, and its
  send to the worker refused; queued prompts dropped; the next start a
  blank session); a turn failed with no fault (QUIET and wrong: journaled
  `session ended` while the session lived); a passing API fault (LOUD,
  heals itself); a setup script that failed (QUIET); a permission ask
  Claude Code gave up on (stale: the item stays and holds the worker's
  mail; how long Claude Code waits is not in the code).
- Waiting for what never comes: a teammate's `ask` with no answer
  (QUIET; `Status::Asking` is never set live, so health's `Waiting`
  never fires there); a proposal Lonca sent back (QUIET, the worker
  alone told); a card after a failed one (QUIET, the board only); a
  posted card nobody claims (SILENT); cards open and nobody working
  (SILENT, no such state); a refused `hire` (QUIET); a solo's hung or
  failed turn (SILENT: health watches no solo).

## The rule

Every stop has an owner. What the leader can solve (a worker's session
ended, a failed turn, an unanswered ask, a proposal sent back, cards
waiting on a failed card, a card nobody takes) is told to the leader,
which is woken, and it takes one of record 143's three ends: it solves
it (the person hears nothing, the journal says so), it asks the person
with choices, or it lets it lie and imece brings the stop to the person.
What only the person can lift (the pot, `MAX_WOKEN`, Lonca, an account
fault, the leader itself, a solo) goes straight to the person, with
imece's own fixed choices as digits (S1). `stall off` silences what goes
to the leader, never what goes to the person. imece itself lifts
nothing: it never raises the pot or starts a session on its own.

The work goes in four pull requests, each useful alone:

1. the truth and the leader told (this record's first part, built);
2. what stops the whole team, to the person: a `■ STOPPED` line atop
   Home (look A), a `stopped` group first in TO DECIDE, its choices as
   digits;
3. what waits for what never comes;
4. the stale permission ask, after probing how long Claude Code waits.

## Part 1, as built

**A stop is a block.** `blocked::What` says what a block is of: `Said`
(record 143's `blocked`), `Ended` (the session ended) or `Failed` (a turn
failed with nothing imece tries again, its session alive). `World::stood`
opens one when a worker's session ends (`live::sessions::apply` of
`AgentEvent::Stopped`, every path: the start thread, a prompt that could
not be written, `stop_unheard`, the process gone) or when
`World::turn_failed` has no fault to retry or bring to the person. It
takes a number of its own (`World::number`, as a message would), so the
leader names it in an `ask` to the person with `re` as for a `blocked`.
None opens for a leader, a solo, a helper, a worker put away or whose
leader is gone or stopped: those stops are the person's as before. Nor
for a session imece stopped because it did not run with what was asked
(its words start with `claude::CHECK_STOPPED`): whatever differed is the
person's to look at, never a leader's to start again.

**The leader is told at once.** `World.stops` (memory) lists the stops
not told yet; `Live::look_stops` (each step, after `look_blocks`) puts a
note in the leader's next prompt (`stop_note`: what happened, the card,
the stop's number, and the ways on: its message starts the worker again
once, `assign` with `card` hands the card, an `ask` to `you` with `re`)
and sets `lost`, so an idle leader is prompted `GO_ON` as for a health
sign (counted in `MAX_WOKEN`, refused over budget). `World::stops_read`
marks the leader's stops read with that prompt, so a turn of its that
ends with neither a word nor a question lets them lie (record 143's
`blocks_unsolved`), and `look_blocks` brings one older than
`:health stall`.

**The leader may start it again, once (R1).** The bus refused every
send to a stopped agent; now a waking message from the leader to its
worker whose session ended goes (`World::may_revive`), ends the block
(`Event::Unblocked { what: Ended }`, `lead started claude-2 again after
1 min: …`), puts the worker back to idle so the post office delivers it,
and sets `Agent.revived` (memory) until a turn of the worker's ends well
(`apply` of `TurnDone { ok: true }`) or the person writes to it (`World::said`). A worker that stops again with
`revived` set is told to its leader as stopped again, and the bus refuses
the leader's word with the ways left (hand the card, ask the person).
An inform, progress or notice starts nothing and is refused as such;
anyone else's send is refused as before. A card handed elsewhere or
withdrawn ends the block and puts a stopped worker to sleep (`Asleep`).

**Where it left off.** A session that ended sets `remembered`, so
whoever starts it again (the leader or the person) resumes the session
it had, with the briefing (`Live::brief`); before, a start within a run
was a blank session.

**The truth.** A failed turn with its session alive is journaled
`Event::TurnFailed` (`claude-2: turn failed: error_max_turns`), never
`Stopped` (`session ended`). The row, the item, the notice, the preview
and the detail word a block by what it is of (`What::of`): `session
ended "…" · lead is on it`, `turn failed "…"`, `claude-2 stopped · lead
asks: …`, `claude-2's session ended and its leader did not solve it`.
A stopped worker its leader has is drawn as a blocked one (`■` in the
attention colour) and is not `Rung::Stuck`.

**The demo.** `Sim::stop` makes every third (`STOP_FAILS`) stop a turn
that fails with no fault (`max_turns`), and opens the block for the
rest; `blocks_due` takes `World.stops` as live's `look_stops` does and
the leader takes each the three ways of `way_of`: it starts the worker
again (`stop_way`, `failed_way`), asks the person (`stop_ask` with two
answers, no risk: it could not solve it), or lets it lie; a worker
started again that stops again is asked about, never started twice.

## The points of the list

- **Growth**: one `Option<Block>` per agent (record 143's), one `bool`
  and the short `World.stops`, emptied each step. No new item kind.
- **Read cost**: `stood` and `stops_read` pass over the agents once,
  at a stop or a leader's prompt, never per frame.
- **Ten times the scale**: at 160 agents a leader may be told several
  stops in one prompt; `MAX_WOKEN` and the pot still bound its turns, and
  a stop it lets lie still comes to the person.
- **Migration**: no schema change. `Event::TurnFailed` is a new name in
  saved rows, and `Unblocked`/`StillBlocked` carry `what` with a default
  (`Said`), so older rows read as before; an older imece cannot read a
  store that holds the new event.
- **Failure**: the block and `revived` are memory only. After a restart
  a stopped worker is asleep with its session remembered, as before, and
  its leader is not told again; one that came to the person is an inbox
  item and comes back, worded as a `blocked` (its `what` is gone).

## Gaps (part 1)

- Parts 2-4 above are not built.
- A helper's or a researcher's stop is not followed here: a helper's is
  told to its worker as before (`helper_lost`).
- Prompts queued while a session started and then failed are still
  dropped; the resume and the briefing say where it stood, not what the
  dropped mail said.
- `--demo-live` (the stand-in) does not stop sessions yet.
