# A narrow screen

Backlog C20 (#56, after herdr): imece on a small terminal, ssh from a
phone. Record 111 takes no more features from neighbours; the person
chose this one anyway (2026-11-09), so it is built as the screen's
fit to a size, not as a feature of its own: nothing new to learn, the
same keys, the same screens.

What a phone gives: in portrait about 40-50 columns and 20-30 rows
(Blink, Termius, a-Shell at their default fonts), in landscape about
90-110 by 18-22. Record 125 designs at 100 by 40; this record is what
changes below that. Looked at on the demo at 100×40, 60×20 and 48×18
before the change (2026-11-09):

- Home's entries keep a 20-cell name column (glyph, card, a title of
  `TITLE_MIN` 10 at least), so at 48 columns a permission takes four
  rows and four entries fill the screen;
- the status line kept the place (`home · shop-api · #1 Shipping
  cost`) and dropped the keys that decide, though record 124 says the
  edge never cuts those;
- the help, in one column, is 13 + 44 + 4 cells whatever the screen,
  so under 61 columns its words are cut at the frame;
- a piece of work's head line is cut (its time and cost go first);
- the conversation over a detail leaves a column of the screen under
  it on either side.

## What is decided

**A floor.** Under `MIN_COLUMNS` 40 or `MIN_ROWS` 12 imece draws one
line in the middle (`too_small`: this terminal's size and the floor,
both languages) and no key acts but `ctrl-c`, a paste neither: what
the person cannot see they do not decide. The size is the last one
drawn (`App.drawn_size`, set by `ui::draw`), so a key acts on what was
on the screen when it was pressed; a terminal made larger draws again
at once (a resize is a terminal event, record 77) and the keys come
back.

**The status line.** The screen's keys first, the place in what they
leave (`ui::status_parts`): cut with `…` while it keeps `PLACE_MIN` 12
cells, else gone; only then do keys go, from the end, never `esc` or
`?`. Record 124 meant this ("the keys that decide are never what the
edge cuts"), but the place stayed while any one key fit beside it.

**The header.** Its parts go as before (`fit_header`, the lowest `keep`
first, never `KEEP`); when what is left is still wider than the screen,
the ` imece ` brand goes too, so what to decide and the pot are whole
at 40 columns.

**Narrow.** Under `NARROW` 80 columns:

- Home's entries stand stacked: glyph, card and title on the first
  line, the title as wide as the row lets it be; the ask, the verdict,
  the status under it from where the card starts, the answers a
  question offers on lines of their own. A row that says only what it
  does (an agent at work) keeps one line when title and words fit. The
  row at hand carries no keys of its own: the status line has them,
  whole (above).
- The help's text column takes what the frame and the key leave
  (`help_widths`), down to `HELP_NARROW` 20; at the floor that is 23,
  so a key never needs a line of its own for want of room. The legend
  wraps by its entries (`legend_wrap`), at every width: it was cut at
  the frame in one column before.
- A piece of work's head line, when it is wider than the screen,
  breaks in two (`detail::head_lines`: the card and title; project,
  agent, time, cost, files), each wrapped at its blanks.
- The conversation over a detail takes the whole body.

Read cost: one comparison of the area's width per frame; the stacked
rows are the same words wrapped another way. Growth: nothing kept.
Failure: a size the terminal reports as 0 is under the floor and draws
nothing but the line. The demo runs the same drawing, so it shows
what live shows.

Not done (to come, after a pass over each window at 40×12): the other
windows (board, code map, timeline, journal, peek, script, plan,
tidy, ps, Lonca, archives, welcome, news, digest, left off, the quit
question) shrink to the screen as before and cut what does not fit;
the diff does not wrap by itself on a narrow screen (`w` does).
