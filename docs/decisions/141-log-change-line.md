# Enter on a change line of the log opens the change at its lines

Issue #458 (user, 2026-11-10): on an agent's log (its focus) a line such
as `Δ change  changed docs/guide/concepts/team.md` says a file changed,
and `enter` on it opened the raw stream as on any other line. The person
wants to see that change: the lines the log's line is about, at first
glance, lit for a moment without disturbing.

## What is built

`enter` on a `changed FILE` line of a focus (in or outside the card's
paths: `StoryEvent::Touched`, `Strayed`) opens the worker's change, the
diff tab of the same detail, at that file:

- the view starts at the file's header (`diff --git`), the file unfolded
  if it was folded;
- the header says ` ← the log's line`, in the accent, for as long as the
  person stays on that change;
- the file's lines are lit for 1.5 s: a background blended from the
  theme's `accent` into its background at 30, 24, 18, 12, 6 %, a step
  every beat (300 ms), then nothing. Never reversed, never a blink. The
  line at hand keeps its own mark over the light;
- `esc` goes back to the log, at the line it was opened from (a change
  opened any other way goes back to Home, as before).

The footer says `enter  its lines` on such a line, before `a` `r` when
the change waits; an ask's three keys leave it to the help.

## What was to decide

- **What is lit:** the file's whole part of the patch. The log has one
  line per file however often it was edited (`sessions::event` records a
  `Touched` once per file per card), and imece keeps no hunk per edit, so
  "the hunks that one edit made" is not known.
- **How long, how it looks:** `Glow::Point`, `motion::POINT` 1.5 s, five
  steps of `ui::POINT_SHADES`; the accent, the colour of the bar that
  marks the log's line at hand, not `ok` (green under added lines would
  hide them).
- **Without motion** (`:motion off`, another window; over SSH calm still
  moves) **and where colours cannot be blended** (mono, the terminal's
  own colours): nothing is lit; the words on the header are the mark.
  They are there with motion too, so the light is never the only sign.
- **A line whose file is not in the change:** a flash and the log stays:
  `NAME shows no change yet: FILE opens here once one waits` with no
  change waiting (mid-turn, or applied), `FILE is not in the change NAME
  shows now` when the change has no such file. The raw stream is `t`
  there; `enter` on every other line opens it as before.

## How

`App::change_line` (the file of the entry at hand; none on a card's
screen or under a conversation), `App::open_change_line` (finds the
header whose `ui::head_path` is the file, unfolds it, sets `diff_scroll`,
`App.pointed: Pointed { worker, patch, file }` and lights
`GlowOn::Lines`), `App::pointed_in` (only for that worker's change with
that patch). `App::on_key` drops `pointed` once the zoom is not that
change; `keep_reading` keeps the log's place meanwhile.
`ui::diff::Pointing` carries the file's lines and the light into
`patch_rows`; `ui::lit_style` words `Lit::Point`.

## The demo

The demo's workers write `Touched` lines for the files of their cards'
leases and their changes are of those files, so `enter` works there as in
live; a test in `app/focus.rs` holds it.

## Gaps

- While a worker is mid-turn nothing waits, so the line only says so; a
  look at the worktree's diff before `done` is not built.
- On Lonca the change is the pending proposal; a file Lonca redacted or
  cut out of the patch reads as not in the change.
- One line per file: a second edit of a file has no line of its own.
