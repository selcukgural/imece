# The cost ledger

G2 of #60, part two (user, 2026-11-09; part one, what a figure means, is
record 132, where the questions and answers are): what each attempt at
a card cost and what became of it, so the person sees what was applied
and what was spent for nothing, card by card, and the leader's own
spend apart.

A row is an attempt (`ledger::Attempt`): a worker's try at a card, the
card's `attempt` (record 126) at the time; or a worker's spending with
no card (a solo, a worker told directly), one row per worker open until
a change of it is applied. A worker's row with no card becomes the first
try of the card its change opens (record 126, "The person's line opens
no card": `World::row_to_card`). It keeps the project, the card and try, the
worker, the model Claude Code said its session runs on (`init`'s
`model`, `Agent.meter.model`), what it spent (its helpers' counted in,
in ten-thousandths of a dollar), whether part of that was never priced
(record 132), the changes it put before the person (`rounds`) and those
that came back (`sent_back`: the person's no, Lonca's conflict or no),
the tick it began and ended, Claude Code's version when it last spent
(`World.version` from `take_plan`, record 75's look), and its
`Outcome`: `Open` (at work, or its change waits), `Applied` (applied
whole; on trunk on Lonca), `Done` (its card done with no change),
`Failed` (its worker's `outcome: "failed"`, or the person's no to one
more try or round), `Withdrawn`, `HandedOn` (the card handed to another
try while this one was at work), `Late` (handed on, and its worker still
sent a `done` for it). A row ends once: how it ended stays.

Booking. Every dollar of a worker or a helper is on exactly one row, the
leader's on none. Claude Code says what a session spent at a turn's end,
so imece books what an agent's figure grew since it last booked
(`World::book`: `Agent.meter.booked`, set at the figure at a load) on
the row its worker spends on now (`Agent.meter.row`, a helper's its
worker's), after `turn_priced` and before a `done`'s change settles; a
gap (`session_over`, a turn unpriced, more gaps than at the last
booking) marks the row. The row spent on is the one `World::takes`
gave the worker (its try at the card, opened there when it has none,
`row_takes`), else its open row with no card, else a new one. A try that
ends kept (`Applied`, `Done`) lets go of its worker, whose next spending
goes on a row with no card until it takes a card; one that ends
otherwise keeps it, so what a worker spends after its card was failed,
withdrawn or handed on is counted as that try's (it was spent for
nothing). A figure that went down (a session's reset the parser did not
see) books nothing and counts on from it.

Where a try ends: `change_applied` whole (the cards it moved to done),
`cards_landed` for `Integrated`, `change_none` (only when it moved a
card: a solo's turn that changed nothing ends no row), `card_failed`,
`decide_cap`'s no, `withdraw`, `hand` (before the try is counted up),
`late_done` (`HandedOn` to `Late`). A round is counted at `change_ready`
on the worker's cards in review, else on the row at hand; one back at
`change_rejected` and at a Lonca conflict or no.

What the person sees. `:ledger` opens a window over the screen
(`App.ledger_win`, the row at hand; `ui/ledger.rs`): its title counts
the attempts and says ` · API-equivalent` on a plan's login, then per
open project in the salon's order a line `applied $3.20 · wasted $1.10 ·
at work $0.40 · lead $0.90` (`World::totals`: rows kept, rows that ended
otherwise, rows open, the project's leaders' figures; `+?` on each part
that has an unpriced row) and its rows newest first: `#12 try 2
claude-3  claude-opus-5-5  3 Δ · 2 sent back  $0.84  12 min  late done
2026-11-09 · 2.1.4`, or `nothing spent on a card yet`. `j k g G` (the
arrows, Home, End) move, `enter` opens the row's card (`open_card`), a
row with no card its worker's focus, `esc` `q` close, `?` the help; the
rest do nothing. The card's screen lists its own tries under WHO
(`ATTEMPTS`). It is allowed in an archive's view (`VIEW_COMMANDS`): it
only looks. The Turkish title is `HARCAMALAR`: `DEFTER` is the
journal's.

Growth: one row per try at a card and one per worker's stretch with no
card, kept for good with the project (archives too); a long-lived
project of a thousand cards is a few thousand small rows. The store's
`ledger` table (schema 34), read whole at a load in key order, written
by its key when its hash moved (the save plan's `ledger`); no index
beyond the key. Read cost: the window and a card's screen go over the
rows once a frame, only while open; `book` reads the worker's row by
index, and looks among the rows only when it has none (a row with no
card, newest first). Ten times the scale: the window's list grows; a
filter or a per-project cut would be the first thing to add. Migration:
an older file gets the table empty: what was spent before has no rows
(the agents' figures are booked as they are at the load, so nothing old
is counted again). A row that names an agent or a card not there, or
out of order, fails the load, as a card would. Failure: the rows are
written with the rest of a save; a crash before it loses the rows'
latest growth while the agents' figures it came from are lost with
them, so the two stay in step.

Demo (record 31): the demo books every busy tick through the same
`book`, its agents' sessions saying `claude-sonnet-5-5` or
`claude-opus-5-5` (`sim::DEMO_MODELS`), and its cards fail, are handed
on, withdrawn and applied through the same calls, so the window and the
cards' screens fill as live's do.

The per-model split of a row (`modelUsage[*].costUSD`) is part three,
record 134 (What each model spent): it is per session in Claude Code's
result and resets with a `/clear` and a resume's floor as the total
does, so it is counted in the Claude Code adapter; `model` stays the
model the worker's session ran on, and `models` says what each model,
its helpers' and the background's too, spent of the row.
