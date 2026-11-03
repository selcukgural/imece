# The header by what matters, the last ten minutes, a project's cards

Part E of #257 (2026-11-03), after record 97. The header was one line in a
fixed order: cards, the fleet, the pot, each vendor's window, the
researchers, then what waits for the person and the clock. In the demo at
160 columns the line ran past the edge, and what got cut was the end of
it: `! N waiting for you`, the one part the person must not miss. Nothing
said what had happened lately (the person's "ben bakmazken ne oldu?"),
and a project's own progress was only in the board.

The header is now a list of `ui::HeadPart`s, most wanted first: what
waits for the person, the fleet (with the full-motion spinner), the last
ten minutes, the cards, the pot (its rate glued to it), each vendor's
tightest window, the researchers, the clock (the pause or the speed glued
to it). Each part has a `keep`; `ui::fit_header` takes, while the line is
wider than the screen, the part with the lowest `keep` away, the last of
equals first, and never one at `KEEP`: what waits, the pause, a pot
running low (`pot_low`, spent and rate as one part), a window used from
`LIMIT_LOUD`. The order of going is the researchers and calm windows (1),
the rate (2), the clock and speed (3), the pot (4), the cards (5), the
ten minutes (6), the fleet (8). A glued part goes before the one it is
glued to, so it is never left glued to another. The brand is not a part.

The last ten minutes: `World::pulse` counts, over `RATE_WINDOW` (the
rate's ten minutes), the cards done (`finished` in the window, column
done), the changes that came for review (`Event::ChangeReady`) and the
messages on the bus (`Event::Sent`), of projects not put away. It reads
the journal from its newest line back while lines are in the window
(`take_while`), so the cost per frame is what happened in ten minutes,
not the journal's length; the cards were already scanned for the
header's count. `ui::pulse_spans` says `10 min: 3 cards done · 2 Δ · 14
messages`, a count of none left out, nothing when all are none. Both
the demo and live write those events and set `finished`, so the demo
shows it as live does.

A project's cards: its header row has `▓▓▓░░░░░ 3/8` after the name
(`salon::cards_spans`, `BAR_CELLS` 8, the done share filled down to the
cell, so a full bar is all done), nothing while it has no cards. A
leader that posts more cards moves the bar back: that is what happened.

Not done: who holds up whom (D) goes with the story panel (B), not the
header; spending in the last ten minutes is the rate already there.
