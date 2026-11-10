# A worker that finished says so on its row

Issue #451 (user, 2026-11-10): on Home a worker whose change was applied
and a worker never given anything both read `idle`; only the card's title
on the row told them apart. The look was decided on mock-ups
(words, a glyph that stays, a count); the person took the words.

## What is built

An idle worker's or solo's row on Home says, beside `idle`, what became of
the work it last had:

| the row says | when |
| --- | --- |
| `idle · finished, applied` | its change was applied (the checkout, or trunk on Lonca) |
| `idle · finished` | done with no change to apply |
| `idle · #12 failed` | its try at the card failed |
| `idle · #12 withdrawn` | the leader withdrew the card |
| `idle · no work yet` | it was never given anything |
| `idle` | work in hand still (a card not through, the person's line with no card yet), or a card handed on |

Then how long, as before. `finished` is green, `failed` red, the rest
dim. The AT WORK heading counts the finished apart:
`AT WORK 4 · finished 1 · idle 1`; a failed or withdrawn card is not
finished work and counts as idle.

A row too narrow says it in fewer words, in this order: without how long,
then without `idle`, then cut (`Tail::Fits` in `ui/home.rs`: the first way
that fits is drawn).

## How

Nothing new is stored. `World::last_work` (`ledger.rs`, `Last`) reads how
the agent's newest row in the cost ledger (record 133) ended:
`Outcome::Applied`, `Done`, `Failed`, `Withdrawn`; an open row, one handed
on or late, and the person's line that has no card yet (`task.own`) say
nothing; no row, no card and no line is `Nothing`. The ledger is saved, so
the row reads the same after a restart.

Only workers and solos say it (`ui::home::rest_of`): a helper's work is
its worker's and books on its worker's row, a leader does no card.

The five glyphs stay as they are (`○` idle; `✓` is still "done, not seen"
of record 35): the words carry it, so the mono theme reads the same.

`done` was not the word: the screen uses it for "done, not seen".

## The demo

The demo's workers go through the same model (`World::change_applied`,
`change_none`, `card_failed`, `withdraw`), so their rows say the same; a
test in `sim.rs` holds it.

## Gaps

- The focus, the `&` jump list and `:why` still say `idle` alone.
- A worker that finished several cards says only the last one's end;
  a count (`3 cards`) was a mock-up not taken.
