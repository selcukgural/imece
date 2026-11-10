# The leader solves a worker's blocked first; the person hears only when it is risky, unsolved or let lie

Issue #452 (user, 2026-11-10): when the leader hears from a worker that it
is stuck, the leader first tries to solve it itself, and asks the person
only when the solution is risky or it cannot solve it. Until now a
worker's `blocked` was mail like any other: the leader's turn started on
it, and nothing on the screen said that the worker was blocked, that the
leader had it, or what became of it.

## What was decided (user, on the mock-ups)

- **Three ends.** Solved: the person hears nothing, the journal has a
  line. Asked: the leader puts a question before the person, only when
  the way on is risky or it found none. Let lie: imece brings the block
  to the person itself. No block waits in silence (the sibling rule of
  #455).
- **While the leader has it** the worker stays in AT WORK, a `■` in the
  attention colour, `blocked "…" · lead is on it`; it is not the
  person's turn, is not counted as stuck and rings nothing (look A; a
  group of its own, B, was dropped: a row that asks nothing would sit
  under the decisions and the list would move when it is solved).
- **The question** is the blocked work's row and reads in parts: why the
  leader asks, the worker's words, what the leader tried, what it asks,
  the answers (look B; today's bare question row, A, left all of that to
  the leader's sentence).
- **The limit of silence**: the leader's turn ending with no word, and
  `:health stall`; no setting of its own.
- **Risky** is: deleting or overwriting work; reaching outside the
  repository (credentials, installs, network services, money); widening
  what the card owns or its task; dropping a requirement or skipping a
  test; changing something the person decided.

## What is built

**The block.** `blocked.rs`: `Agent.block: Option<Block>` (the message's
number, the worker's words whole, its card, its leader, the tick, `read`,
`Stage`). `World::blocked` opens one when a worker sends `blocked` to the
leader of its project (`opens_block`; a teammate's, a helper's to its
worker and one to `you` open none). Only the envelope decides: the body
is never read for meaning. `Stage::Leader` → `Asked { tried, risk }` →
`Yours(Unsolved)`, `Unsolved` = `TurnEnded` / `TooLong` / `LeaderStopped`.
`World::held_of` says who has it for the row (`Held`): the person the
leader asked only while that question is open, the leader again once
they answered.

**What ends it.** The leader's waking message to the worker
(`World::unblocked`, `Event::Unblocked { agent, by, text, minutes }`: the
journal's `lead unblocked claude-2 after 3 min: …`); the worker's own
`done`; the person's word to the worker (`World::said`); its card handed
to another or withdrawn (`blocks_on` from `World::hand` and `withdraw`);
the worker leaving. All but the first say nothing of their own: what did
it is in the journal already.

**The ask.** The `send` tool takes `tried` (≤ 600 characters) and `risk`
(≤ 120), and `re` on an `ask` to `you` when it is the number of an open
blocked of a worker of the sender's (`live::bus`: `words_of`, `Asked`;
refused on any other message, with no block it could be about, as a
non-string, too long, or with a control character; a `re` to the person
that names anything else is refused as before). `World::block_asked`
ties the question to the block: by `re`, else to the only block the
leader has read and not settled (a leader that names none means the one
it has; with two, none is guessed and the question stays a plain one).
The question's item takes the worker's card, so its row and its detail
are the blocked work's. The leader's system prompt says to solve first,
what is risky, and how to ask (`prompts.rs`).

**Let lie.** `Live::deliver` marks the blocks a turn of the leader's
takes as read. When a turn of the leader's ends and no prompt is left to
it (`Live::step`, `turns == 0`), `World::blocks_unsolved` moves the
blocks it had read and neither answered nor asked about to the person;
so for one it asked about once the person has answered and the next turn
passed nothing on. A block that came while a turn ran was not read by
that turn and does not count against it. `World::look_blocks` (each
step) brings a block older than `:health stall`, read or not, unless the
person has the leader's question; `stall off` switches that off. A
leader whose session stops leaves every block of its to the person. Each
is `InboxKind::Blocked` (questions group, `a` writes to the worker,
`enter` opens, holds the worker's mail, closes on the person's word or
the leader's after all, rings loud in another window) and
`Event::StillBlocked`. The leader hears with its next prompt that the
person has it (`live/blocked.rs`), and is not woken for that.

**The screen.** `ui::glyph` and `block_spans`: `■` in the attention
colour while the leader has it, in the stuck colour once the person
does; `blocked "first line" · lead is on it` / `· lead asked you`.
`App::rungs` keeps such a worker off `done, not seen`; `home_rows_in`
lists it with those at work. Home's question row for a block:
`claude-2 is blocked · lead asks: …`, called by the worker's work
(`title_of`); the preview (`block_question_lines`) and the detail's
question (`detail::question_lines`): `lead asks you · risky: …` or
`· could not solve it`, then `blocked`, `lead tried`, `asks`, the
answers, and what the leader did so far. A blocked worker's own preview:
its words whole, then the leader's newest trace lines.

**Health.** A blocked worker is no stalled one (`health::quiet`): its
leader heard from the worker itself.

**The demo.** `--demo`: a working worker tells its leader it is blocked
(`Sim::tell_leader`, a fifth of what used to go to a researcher's
question; the researcher path keeps the rest, record 01's follow-up
removes it). The leader takes the `n`th block one of three ways
(`way_of`): it sends the way on, asks the person about every third
(`BLOCK_ASKS`) with what it tried, the risk and answers to pick, and
lets every fifth lie (`BLOCK_SILENT`). The person's answer to the leader
is passed on to the worker (`block_answered`, no plan comes of that
word). `--demo-live` (the stand-in on the real `Live`) does not block
yet: it comes with the stand-in's next behaviours (record 130).

## The points of the list

- **Growth**: one `Option<Block>` per agent, a message of at most 4,000
  characters and 720 of the leader's; gone with the block. One inbox
  item per block let lie, closed with it.
- **Read cost**: a row asks `held_of` (one inbox scan) and, for a
  leader's question, `asked_block` (one pass over the agents): small at
  16 agents a project, and only for rows drawn.
- **Ten times the scale**: at 160 agents the per-row passes are still
  thousands of comparisons a frame; the leader's attention is what
  breaks first, and the stall limit is what catches that.
- **Migration**: no schema change. `InboxKind::Blocked` and the two
  events are new names in saved rows; an older imece cannot read a store
  that holds them, as with every new kind.
- **Failure**: the block is memory only. After a restart a block the
  leader still had is forgotten, and health's stalled notice (the worker
  idle on its card) tells the leader again after `stall`; one that came
  to the person is an inbox item and comes back, without the leader's
  `tried` and the worker's words past their first line.

## Gaps

- The leader may answer a block badly and so end it: imece does not
  judge an answer, by the rule that only the envelope drives. A worker
  still stuck says `blocked` again.
- A `blocked` from a helper to its worker is not followed.
- With two read blocks and an ask that names neither, the question is a
  plain one and both blocks come to the person at the turn's end.
- `--demo-live` does not show it (above).
