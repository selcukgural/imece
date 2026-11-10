# When all the work the leader handed out is done, the screen says so

Issue #454 (user, 2026-11-10): when every piece of work the leader
assigned is finished, the person sees that plainly, one clear state, not
something to work out from each row. Until now three idle rows reading
`finished, applied` and a calm title were all there was: the same title
as a session nobody gave any work.

## What was decided (user, on the mock-ups: K1 · A · Y)

- **Two steps** (K1). When the team is through and something of it still
  waits for the person: `◐ TEAM FINISHED … 2 wait for you`. When nothing
  waits either: `✓ ALL DONE`. Saying it only once everything landed (K2)
  leaves "the team stopped, the rest is mine" to be worked out again;
  saying all done as soon as the team stops (K3) says done of work not
  yet on trunk. A card that failed or was withdrawn holds neither step
  back and is always counted on the line, a failed one in red.
- **Home's first line, and a `✓` beside the project in the header** (A).
  The leader's line (B) would hide the leader's newest words; the header
  alone (C) has no room for the figures.
- **Away from the screen: the tab's title and one notice** (Y), through
  `:notify` as it is set, once per round.
- A round (accepted with the above): the figures are of the cards opened
  since the project last had none open, so last week's cards are not in
  today's `5 cards`.

## What is built

**The round** (`round.rs`, on `World`). `round_from(p)`: the newest card
of the project that came (`born`: its first story line, or the start of
its work when that was earlier) after every earlier card was over and had
ended; the project's first card when none did. Nothing is stored: the
same cards give the same round after a start, and a card handed again
(no longer over) takes the round back to where it belongs.
`finished(p) -> Option<Finished>`: none while a card of the round is
`Waiting`, `Working` or `Verifying`, while anybody of the project not
gone is busy, starting or about to retry a failed turn, for a project
with no card, without a leader (a solo), closed or archived. Else
`Team(Round)` when something waits (`waits` = the project's open items
that are not replies, at least its cards in `Review`), `All(Round)` when
nothing does. `Round`: cards, landed (`Done`), failed, withdrawn, waits,
what the round's ledger rows cost (record 133; the leader's own spending
is on no row) and whether part was unpriced, and the ticks from the
first card's coming to the last one's end (to now while something
waits). `finished_of(projects)` / `finished_all()` take several as one:
none when a team with cards is not finished, all done only when each is;
projects with no card and solos say nothing either way.

**Home** (`ui/round.rs`, `banner`): the first lines of the list, for the
project Home is narrowed to, else the open ones together (`N projects`):
mark, word, whose, the figures that are not nought, then cost and time
dim. Under `ALL DONE` what is next (`round_next`), then a blank line.
Figures that do not fit go on under the word, whole, never cut: what
waits is among the last. `list_drawn` puts it above the headings and
moves the row at hand and the breathing glyphs down by its height; with
nothing listed under an all done, Home's `nothing needs you` line is
left out. The header's project gets ` ✓` when its round is all done
(`header::projects_spans`).

**The title**: `words::title(.., done)` reads `✓ imece · all done`
(`· N failed`) when `App::all_done()` (every open project, as
`finished_all`) and nothing waits and the ladder is calm; the tab's bar
is cleared as for calm.

**The notice**: `notify::Note::Done(project)`. `App::rounds_done`: the
open projects all done whose round (`rounds_told`: project and first
card, memory only) the person does not know of; one on screen while the
person is in the window counts as told. It goes through `Notices::due`
like the rest (still all done after `AFTER`), and a round that rang is
kept in `rounds_told` until a card opens the next, so a leader that takes
a turn and hands out nothing does not ring it again. Words name the
project and the counts, nothing an agent wrote; a push says the same
(the project once); not loud; a click goes Home.

**The demo**: its leader kept a card waiting for every free worker for
ever, so it never came to an end. It now hands out `ROUND_CARDS` (12)
cards by itself, then none until the round is all done and `ROUND_REST`
(24 ticks, 2 min) have passed (`Sim::hands_out`); what the person asks
for is posted whenever they ask. With nobody kept busy by new cards, a
failed card could wait for ever on the worker that failed it being the
first one free: the leader now hands it to the first free worker that
may take it.

## The list

- **Growth**: nothing kept but `rounds_told`, at most one entry per open
  project, dropped when its round is over.
- **Read cost**: `finished` reads the cards, the agents, the open inbox
  and the ledger once each. Home asks per project in view and the header
  per open project each frame, the title once per loop: with tens of
  thousands of cards that is what would show first, and a round's first
  card kept on the project would end it.
- **Migration**: none; no schema change.
- **Failure**: a wrong answer here says a word too early or too late on
  the screen; it gates nothing.

## Known gaps

- Mail that waits for a leader that is between turns is not looked at
  (it lives in `Live`, not the world): between a worker's last `done` and
  the leader's turn that reads it, the line can show for a moment and go
  again. The notice waits `AFTER` (5 s) and is not fooled by it.
- A card that waits on another whose change waits for the person keeps
  the team from being "finished", though only the person can move it: TO
  DECIDE shows the change, the line says nothing.
- No journal line and nothing in the away digest says a round ended.
- The `?` help's legend does not list `◐`; the line's own words say what
  it is.
- `--demo-live`'s stand-in comes to all done by itself (one worker, one
  card); it has no rounds of its own.
- The tour videos do not show it.
