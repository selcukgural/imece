# Motion on the screen

Part 4 of the motion and signals plan (#200). The design talk
(2026-11-02) decided: `:motion off|calm|full`, calm by default; a
working row's `●` breathes in its colour (two tones, 600 ms), its shape
kept, as the state ladder (record 61) wants every rung its own shape;
no turning marker on the rows. The rule stays record 77's: drawn on a
change, never on a timer.

**What moves** (`motion.rs`, `Motion`; the screen's words in `i18n.rs`):

- calm: a working row's glyph (a worker's, helper's, solo's or
  researcher's; never a leader's `◆`) takes the DIM attribute every
  other 600 ms (`App::breath`, `ui::glyph_style`), also as the loudest
  member of a folded group; `starting…` counts its dots, `starting   `
  `starting.  ` `starting.. ` `starting...` (`App::dots`,
  `salon::dotted`, the word less its `…`, so both languages); the bottom
  line shows `⠋ asking … for a commit message …` while a
  `:commit` waits (`App::asking`, set with the flash, let go by the step
  that sees the engine no longer asking: `Live::asking`, `Sim::asking`),
  and the first-run window spins by a vendor still `Checking`
  (`App::spin`, `…` when nothing moves); a row that came to wait for the
  person (`Rung::Waiting`) and each new item of WAITING FOR YOU blink
  reversed three times (`Glow::Arrive`); a row whose turn is done
  (`Rung::Done`) or whose session started (from `Rung::Starting`)
  settles (`Glow::Settle`): a background of the theme's `ok` blended 35,
  20, 8 % into its background where both are RGB, else reversed, then the
  changed rows' look (`ui::lit_style`). A selected row keeps its
  selection; a lit row shows over the "changed" look.
- full: the same, and a spinner before the header's fleet while anyone is
  mid-turn (`App::head_spin`), the spinners at 10 frames a second.
- off: the screen as before this record; `App::new` starts off, `main`
  sets the saved choice (calm when none). `:motion WORD` or bare (the
  next one round), menu note and completion as `:notify`; kept in the
  store's `meta` as `motion` (`prefs::keep_motion`, written when it
  changes, read with the rest; a word it does not know reads as none).

Over SSH (`SSH_CONNECTION`, `SSH_CLIENT` or `SSH_TTY` set, `motion::over_ssh`)
full runs as calm (`Motion::under`): every frame crosses the network. In
another window (`App::blurred`) nothing moves (`App::moving` is off):
the frame stands, and nothing lights for what happens meanwhile, so the
way back is not a burst of blinks (record 52's digest says what
happened).

**When a frame is drawn.** `App::next_motion(now)`: the soonest moment
something shown changes its look, none when motion is off, the terminal
has no focus, an archive's view is open, or nothing on the screen moves
(breath and dots only on the salon's screen, for agents not gone or put
away, and the breath on the board for the owners of the cards taken
(in progress, verifying, done) in sight; the spinner only while the bottom line is free: no flash, no
countdown, not writing; the header's only in full and while someone is
mid-turn; each glow to its end). `App::drawn` keeps it as
`drawn_motion`; `App::draw_due` draws when the clock passes it;
`App::motion_wait` shortens the main loop's poll to it, so the demo's
500 ms tick does not hold a frame back, unless the tick comes at most
`motion::LATE` (100 ms) after it (`App::wait`): then the tick's wake
draws it, a little late, as one frame, not two. Live ticks every
100 ms, so there what moves never wakes the loop by itself. Losing the focus forgets it, so
a blurred loop never spins on a frame that is past. Glows end and go
(`drawn` keeps a glow until its end, one not yet begun too).

**One beat.** The first build drew far more than it moved: each glow
counted from its own moment, so two blinks a quarter-second apart made
twice the frames, and the dots (500 ms) fell between the breaths
(600 ms). Now everything calm falls on `BEAT` (300 ms from the screen's
start): breath and dots 600 ms, the calm spinner, a blink, a settle step
300 ms each (three blinks 1.8 s, a settle 0.9 s), and a glow starts on
the beat at or after it is lit (`motion::on_beat`; lit at once until the
beat after that). Full's spinner (100 ms) divides the beat. So several
things moving cost no more frames than one: at most 3.3 frames a second
from motion while something glows, 1.7 while only rows breathe. The
blinks stay under three a second (WCAG's flash threshold). SGR blink is
never used (it cannot be switched off and terminals differ).

**A frame of the breath alone is patched.** Most of what calm adds is
the breath: about 1.7 frames a second, each a whole drawing (1–2 ms on
the salon) for a change of one attribute on a few cells. The salon
notes where it put each glyph that breathes (`ui::Glyph`, in
`App::breathing`, through `salon::note_glyph`: the span's columns from
the spans before it; the row's own glyph, a folded project's member).
`ui::draw` empties the note first and drops it (none) when anything
is drawn over the salon (a window, a conversation, a peek, hint
letters, an archive's view) or a glyph is cut by the list's right
edge. While the note holds a glyph, the main loop keeps a copy of what
it drew (`keep`, 20 µs for 160 × 48, the allocation reused). A frame
due only for the breath (`App::patch_due`: nothing from the terminal,
nothing changed, the same clock, no glow, and the breath all that moves:
`App::moving_parts`, no dots or spinner) is `ui::patch`: the copy with
DIM set or cleared on each noted glyph's cells and the cursor where it
was; another size draws whole. The test draws the demo whole in one
tone, patches it to the other where the main loop would, and compares
it cell by cell, cursor included, with the frame drawn whole (120 demo
steps, folded projects, a selected row, the person writing, a narrow
screen). Glows, dots and spinners are drawn whole.

**Measured** (`tools/measure.py`, 30 s, M3 Max, demo on a 160 × 48 pty,
the level typed 2 s in, two runs each; CPU seconds; before = main
before the merging and the patch):

| | off | calm | full |
|---|---|---|---|
| demo's speed, before | 0.07–0.12 | 0.18–0.26 | 0.41–0.67 |
| demo's speed, now | 0.11–0.16 | 0.15–0.21 | 0.44–0.49 |
| top speed, before | 0.82–1.02 | 0.96–1.04 | 1.27–1.31 |
| top speed, now | 0.76–0.91 | 0.70–0.79 | 0.84–0.87 |
| paused 8 s in (`--pause`), before | 0.04 | 0.07–0.10 | 0.37–0.46 |
| paused 8 s in, now | 0.03–0.04 | 0.05–0.06 | 0.39–0.45 |
| top speed, blurred, now | 0.06–0.09 | 0.06–0.08 | |

Run to run, the same build differs by up to a third, so the rows say
more than any one cell. At top speed calm now costs what off costs:
each frame of what moves falls on a tick's. Paused (rows still
working, nothing new: a live team between events) calm's surcharge
halves, and most of what is left comes before the pause. At the
demo's own speed calm is still above off by more than the design's
10 %: the demo lights a row every few seconds and starts sessions
often, and glows (six frames in 1.8 s) and dots are drawn whole. Full
spins in the header ten times a second while anyone is mid-turn, drawn
whole: full is the person's choice of more motion. (Before the beat,
calm at top speed was 0.26 s in 15 s, against 0.13 off.)

**Points.** Growth: `glows` holds at most one per agent and per item,
each gone at its end (1.8 s); `glow_rungs` one per agent. Read cost: a
step computes the rungs once more and walks the inbox from the last
length seen; a drawn frame computes `next_motion` (the rungs once, the
agents twice) and `patch_due` the same again; the note of glyphs holds
one per salon row, the kept copy one screen. Ten times the agents: the same per frame, since the
frames per second do not grow with what moves. Failure: a frame that is
late is drawn at the next look (the loop's poll at most a tick).

**Not done.** The design's breathing in RGB tones of the theme (DIM is
used: one attribute, every theme, mono too). Glows and dots patched
like the breath (a lit row drawn again alone): the demo's speed would
then meet the 10 %. Nothing animates on the code map. A spinner while a
change loads was looked at (2026-11-02) and is not needed: a change is
read on the git thread when its worker says `done`, before its Δ is in
WAITING FOR YOU, so opening it never waits; an apply says so at once
(`applying …`, the flash) and the journal when it is in. No reduced-motion hint is read from the system
(a terminal does not pass one); `:motion off` is the person's switch.
The activity bar per row is part 5 of #200, in the activity bar below;
`off` leaves it out too.

**On the board** (2026-11-02, item 5 of the motion talk). A card's owner
under its title (every column but WAITING, which shows none) wears the
same glyph as before, now styled by `ui::glyph_style`: it breathes when
`ui::breathes` says so on the salon (rung working, not a leader), so the
two screens agree on who is at work. `ui::note_glyph` (moved from the
salon, shared) records where it is, so a frame where only the breath
changed is patched on the board too; a card below the column's foot is
not noted. `App::moving_parts` gives the breath on the board when a card
in sight (`shown_cards`), not waiting, has an owner not gone who
breathes; under a zoom or on the code map nothing breathes. The demo
draws the same board, so it shows it (record 31). Read cost: the rungs
once per frame and the cards in sight, as the board already read them.
