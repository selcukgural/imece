# imece's mark on the screen, in the title and at quit

The person, 2026-11-03: imece did not show its own face. Claude Code
opens with its mark, its version and where it runs; imece had a mark on
its site (`website/static/img/imece-mark.svg`, five amber dots in a ring
round a white one: five hands round one job) and none in the terminal. The
proposals were shown as a page and accepted as they were: the mark beside
imece's words in the welcome and news windows and in an empty salon, its
dots coming one a beat, the mark in the title, a line left at quit; a
coloured tab (iTerm2's OSC 6) was left out.

The mark is drawn in half blocks (`ui::brand`): the SVG's six circles
scaled to a 12 × 8 grid of pixels half a cell high, each dot 2 × 2, so it
is 12 columns by 4 rows (`cells`: `▀` a cell's top pixel, `▄` its bottom
one, `█` both; no two dots share a cell, so a cell has one colour). The
hands are the theme's `accent`, the job its `strong`; in `mono` both are
the text's. No glyph of a font is needed beyond the block elements every
terminal font has. Beside it three lines: `imece v<version>`, the tagline
(`one terminal, many agents, one leader` / `tek terminal, birçok agent,
bir lider`) and the project at hand with who heads it (`shop-api · claude
leads`, a solo `… alone`), cut with the rest by `fit`. Where the mark and
the words do not fit side by side (`lines`: indent + 12 + 3 + the longest
line), the words stand alone, each cut to the room.

Where it shows: at the top of the welcome and news windows, with a blank
row above and below, when the screen has the window's rows and these six
(`overlay::branded`), else the window as before; and in the salon, in the
middle of the rows left free, while every row is a project or a leader
(no project has a worker yet: a team's first minutes, a solo) and `f` is
off, when four rows and one on each side are free. The first worker's row
takes its place.

Its coming (record 84's rules): the first drawing with the mark after one
without it (`mark_drawn`, `mark_before`, set by every `ui::draw`) notes
the next beat (`App::mark_shown` → `mark_from`, `motion::on_beat`); from
there `App::mark_dots` shows one dot more each `BEAT` (the hands clockwise
from the top, the job last, 1.8 s for all six), and `moving_parts` has
`Moves::Mark` until then, so its frames fall on the beat with the rest
and a drawing that moves only the breath is not patched while it comes.
Any key (`App::on_key`) makes it whole at once (`mark_from` none), as do
`:motion off`, another window and SSH (`mark_dots` checks `ssh` itself:
full runs as calm there, and even calm's six frames are not worth the
network). Nothing waits for it: the words beside it are there from the
first frame, and keys act as always. It comes again only when it went
from the screen and came back (`:welcome` again, a salon that had a
worker and has none).

The title: calm (`screen::Progress::Calm`) starts with `⁙ ` (U+2059 FIVE
DOT PUNCTUATION, five dots as the hands) where it had nothing; the other
rungs keep `✕ ` `! ` `● `. A terminal cannot be sent a picture for its
tab (the crab in the person's screenshot is drawn by the app that hosts
the terminal, which knows the process), so a character is all a title
carries; `✻` was too close to Claude Code's `✳`, `⁘` has four dots, `❂`
draws large in some fonts. Menlo has no `⁙`; macOS and Linux draw it from
a fallback font.

At quit, after `screen::restore` (the alternate screen gone), one line on
stdout (`words::farewell`): `⁙ imece`, what waits for the person (`2
waiting for you`), the agents that sleep until the next start (`3 agents
asleep`; those of projects not put away and not gone; never in the demo),
what this run spent (`World::spent` less `App::spent_at_start`, read when
the run began; left out at nothing), and `resume: imece` (the demo: `the
demo keeps nothing`). It is read after `persist` and before `leave`. A
failed write (the terminal went away, record 86) is let go: it is no fault
of the run's.

Growth: none; three cells and an instant in `App`, nothing saved. Read
cost: the salon tests the rows it already built; the words read the
project at hand's head, one agent. Failure: the farewell's write is the
only new output, its error ignored. The demo goes the same way: `:welcome`
in it brings the mark's dots, and its quit leaves its line. Not done: an
animation of the mark anywhere else (the header keeps its `imece` badge),
a colour per project, the tour videos (re-rendered with the next round of
them; `website/video/README.md`).
