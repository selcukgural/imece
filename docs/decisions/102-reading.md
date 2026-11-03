# Reading an agent up close: the focus and the raw stream scroll, an entry opens, a table fits

The person's report (2026-11-04): on a focus they could not move up or
down or fold anything, and a long message ended in `… +20` that nothing
showed; a Markdown table in a message came out broken in the focus and
in the raw stream, which did not scroll either. They asked the focus to
work like the conversation window.

What was there: `focus::draw` drew the story with `tail_linked`, the
newest rows that fit and nothing above them; an agent's message of more
than `STORY_ROWS` (8) lines was cut there with `… +N` for good. `j k` in
a focus moved the selection of the panel behind it (the salon or the
waiting list) without anything on screen saying so. The raw stream drew
the same way. A table (`ui::table`) was laid out at its natural width,
its widest cell (a link) setting a column wider than the screen, and the
rows wrapped as prose, header cells under one another.

What it does now (`App.reading: app::Reading`):

- The focus has an entry at hand: one entry of the card's story, marked
  `▌` in a two-cell gutter on every row of it and on the selection's
  background, as the conversation marks its message. `Reading.sel` none
  means the newest, followed as more come (the newest at the foot, as
  before); `j k ↑ ↓` step (from the one before the newest, `j` goes back
  to following), `gg Home` the first, `G End` the newest. The drawing
  keeps the entry at hand in view as little as it can (`ui::in_view`,
  `Reading::focus_top`).
- Pages scroll without moving the entry: `PgUp PgDn`, `^d ^u` half a
  page, `^f ^b` a page (a page is the rows less one). A page from the
  newest followed fixes the entry at hand there, so what comes does not
  move the view.
- Each entry is cut to its first `STORY_ROWS` lines (`Reading.folds`,
  all cut at first: `Folds::every(true)`); the line under it says how
  many more and the key, `… +20 · l shows all`. `l →` shows the entry at
  hand whole, `h ←` cuts it again, `za` toggles, `zR` shows all, `zM`
  cuts all. A card's screen and `:export` keep the cut lines with `… +N`
  (`focus::story_lines`); the focus builds on `focus::story_told`, an
  entry a `Told` (head, the text under it, its indent).
- The raw stream scrolls: `Reading.trace_top` none follows the newest at
  the foot; `j k ↑ ↓` a row, `PgUp PgDn ^d ^u ^f ^b` pages, `gg Home`
  the first row, `G End` the newest again; a step down onto the last
  page follows again (`Reading::trace_by`). A trace line's own rows (a
  heredoc) are still cut at `TRACE_ROWS` (6).
- The reading is kept for one agent (`Reading.agent`): `App::around`
  calls `keep_reading` after every key, so another agent's focus or
  stream starts afresh and leaving both (the salon, a change) forgets
  it; going from a focus to its raw stream and back keeps both.
  `focus_key` and `trace_key` take only their keys and leave the rest
  (`@ enter esc v t K`, views, help, writing) as before; with a
  conversation over the focus, the conversation takes the keys. The
  peek (`K`) draws the focus as read only when it is the same agent,
  else fresh.
- A table fits its room: `markdown_src` and `markdown_in` take `room`
  (cells, the indent counted), which only a table heeds.
  `ui::column_widths` keeps the columns that fit as they are; else no
  column is wider than the most that lets the row fit (searched over a
  range, one cell at least each), so narrow columns keep their width and
  the widest give way. A cell wraps in its column (`wrap` on the cell),
  so a row may take several lines; a web address wider than its column
  loses the link style (a link of part of it would open somewhere else).
  A table that fits copies as its source (the rule as the separator);
  one that wraps copies its lines as read (`ui::line_text`), so a row is
  not copied twice. The focus passes its width less the gutter, the
  trace its width, the conversation its text's width
  (`Thread.room`, set by the drawing, so copy mode's lines, `thread_lines`,
  are the lines drawn); a card's screen its body, `:export` none.

Footer: the focus says `j k step · h l cut · whole · enter trace ·
esc back · @ …`, the raw stream `j k scroll · esc back · @ …`. The help's
rows for `j k`, `gg G`, `^d ^u ^f ^b`, `h l` and `za zM zR` already named
these keys; the `h l` row now names the focus's entry.

Read cost: a frame lays out every entry of the card's story (as before,
whole now for the opened ones) and every trace line of the agent (the
trace is capped by `TRACE_KEEP`); nothing new grows. Memory only: nothing
is kept between runs.

Mirrors: pure screen logic; the demo's focus and trace are drawn by the
same code over the demo's cards and traces.

Not done: a focus has no copy mode (`v` opens the conversation with the
agent); a trace line's heredoc rows stay cut at six; the scroll position
is not shown as a bar.
