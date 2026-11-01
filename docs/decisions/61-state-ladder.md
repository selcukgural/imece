# One ladder of states, and what a folded group shows

Backlog item B1 of #55 (2026-10-31, from cmux, herdr and openharness):
a state ladder per agent row, states that differ by shape and not only
by colour, a folded group that shows its loudest member.

The ladder is `Rung` (`app.rs`), lowest first: idle (also asleep, also
gone) < done, not seen (record 35) < starting < working (mid-turn, asking
a teammate, researching) < waiting (something of the agent in WAITING
FOR YOU, or the `WaitingHuman` status) < stuck (blocked, or its session
stopped). `App::rungs` gives every agent's rung; `Need` (record 38) is
now the ladder's top with the calm rungs folded into `Nothing`
(`Rung::need`), so the workers still sort only by what they need of the
person and an agent going from idle to working and back does not move.
Stuck went above waiting: an agent that goes nowhere until the person
writes to it is the loudest thing in its project, as the backlog line
had it (error > needs you); record 38 ranked them the other way and now
says so. `App::loudest` picks a group's loudest member: the highest
rung, the first in order among equals.

Shapes: every rung has a glyph of its own shape, so the mono theme tells
them apart. `rung_glyph` draws the row: a leader always `◆` (record 35);
stuck as its status says, `✕` blocked or `■` stopped (two things to do
about them); `!` when something of the agent waits for the person,
whatever it does meanwhile (before, an idle worker whose change waited
showed `○` and a working one whose tool call waited showed `●`); `✓`
done, not seen; `◌` starting; else the status's own glyph (`●` `◐` `◇`,
`○` `◦`). The `?` legend has `◌ starting` on its first row; the help
still fits forty rows, so no row was added.

Starting: `Agent.starting`, set when live starts a session on the first
prompt to it (`Slot::Starting`: the process, a first worktree's
checkout) and cleared when the session is up (`ready`) or ended (`end`);
the row says `◌ starting…` in place of `thinking…`. The demo sets it on a
worker's first card (nothing spent yet, as the `--agents` workers at a
start) and on a helper's hire, for `STARTING` (3) ticks; the work under
it goes on as before, so the demo's course (its seeded random numbers)
is unchanged and every scenario test still holds.

Folded groups: a folded project's header ends with its loudest member
(`▸ kule  2 working  $1.20   ! 1 waiting  ! codex-2`), the leader a
member too; the header also counts `■N` stopped next to `✕N` blocked. A
worker whose helpers are folded ends with `▸ 2 helpers  ● claude-1.1`;
the calm workers folded away (record 38) read `… 2 agents folded,
loudest  ● codex-2` (`Row::Folded { n, loud }`). The member is the glyph
in its rung's colour and the name in the vendor's: the one to open.

The notices of record 60 note a stuck agent whose item waits once, as
before, now by checking the inbox rather than the rank.

Not done: the fleet in the header (record 35) still counts working, done
and stuck only, no `◌`; a per-rung count on an open project's header was
not added (its rows are right under it). B4 (what each row does now and
for how long, one line per agent) is record 62.
