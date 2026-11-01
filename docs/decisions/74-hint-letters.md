# Hint letters

Backlog item C8 of #56 (from cmux, vimium's link hints): every row the
person can act on, a key and a label away instead of a walk with `j k`
and `tab`. The person chose the key (`F`, as vimium's `f`; `f` is only
what needs you) and what a label does: lowercase selects, the last
letter in capitals opens too (2026-11-01).

**Where.** `F` (`:hints`, record 41) works where the salon's screen is
drawn (`pane_keys`: the salon view, no zoom) with no conversation over
it: the salon's rows (project headers and agents; on a card's screen,
its people), the items of WAITING FOR YOU and the journal lines shown.
The board and the code map have no selection, a focus or a change has
its own keys; there `F` flashes `no_hints`. The targets are
`app::Target`: `Row(Sel)`, `Waiting(inbox index)`, `Line(journal
index)`, the same things `j k` and `tab` reach.

**Where a row is.** The drawing knows where it put each row; nothing
else does. `ui::draw` empties `App::hintable` first, and
`ui::salon::salon`, `side` and `ui::card::draw` push `(Target, x, y)`
for each such row as they lay it out (`ui::hintable`, which drops a row
at or past its area's foot: one not drawn gets no label). The order is
the drawing's: salon, then the inbox, then the journal, top down.

**Labels.** `app::hint_labels(n)`, over `HINT_LETTERS` (home row first,
26 letters): as many labels of one letter as there can be, and the last
letters turned into starts of two-letter labels, as few as hold the
rest (each start turned gives 26 labels for its one: `(n - 26) / 25`
rounded up), at most 26 × 26 rows. A letter that starts a label of two
is never a label itself, so typing never has to wait to see whether a
second letter comes. The first rows drawn (the salon's, then what
waits) get the short ones.

**Open.** `open_hints` gives each target of the last drawing its label
in `App::hints: Option<Hints { labels, typed }>`, frozen while they are
open, as `&`'s list is (record 65). The labels follow their targets,
not their places: each drawing paints a label over the start of the
cell where its target is now (`ui::hint_labels`, after the footer and
before the windows: the letters typed as a key, the rest as attention),
so a journal line pushed down by a new one keeps its label, and one
that left the screen shows none. Only labels the typed letters start
are drawn.

**Keys.** `App::hint_key` takes every key while they are open (after
the help, the digest, the writing and a peek, before a chord): a letter
(lowercased) that completes a label acts on its target, one that starts
labels narrows them, one that starts none does nothing (so a stray `q`
neither asks to quit nor waits); backspace takes a letter back, `?`
opens the help over them, `esc` closes them. Only the last letter's
case counts: `mS` opens, `Ms` selects.

**Acting.** `hint_go` selects the target in its panel as `j k` would:
a row (`pane = Salon`, `selected`), an item (`pane = Inbox`, its
position among `inbox_rows`, `waiting_at` dropped as a pick of the
person's), a line (`pane = Journal`, `journal_sel`); opening is
`deeper`, the same `enter`. A target no longer there (the row left
`shown`, the item was answered, the line is not among `journal_rows`)
flashes `hint_gone` and nothing moves.

**Demo.** Screen state over `World`; the demo and live draw the same
rows, so the labels work the same in both. Nothing is saved.

**Help.** `F` joined the `f  s` row (`f  s  F`), so the help keeps its
55 rows.

**Not done.** Labels on the board's cards, the code map's files, the
lines of a conversation or a change (they have no selection of their
own to land on yet); a remappable alphabet (C9's key file).
