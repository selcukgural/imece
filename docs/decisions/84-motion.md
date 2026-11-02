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
away; the spinner only while the bottom line is free: no flash, no
countdown, not writing; the header's only in full and while someone is
mid-turn; each glow to its end). `App::drawn` keeps it as
`drawn_motion`; `App::draw_due` draws when the clock passes it;
`App::motion_wait` shortens the main loop's poll to it, so the demo's
500 ms tick does not hold a frame back. Losing the focus forgets it, so
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

**Measured** (`tools/measure.py --motion off calm full`, 15 s, M3 Max,
demo on a 160 × 48 pty, two runs each, the level typed 2 s in):

| | off | calm | full |
|---|---|---|---|
| demo's speed, focused | 0.03 s CPU, 0.10 MiB | 0.04–0.05 s, 0.12 MiB | 0.09–0.13 s, 0.13 MiB |
| top speed, focused | 0.13 s, 0.65 MiB | 0.15–0.16 s, 0.71 MiB | 0.18 s, 0.72 MiB |
| top speed, blurred | 0.03 s, 0.17 MiB | 0.03 s, 0.17 MiB | |

Before the beat, calm at top speed was 0.26 s. The design's mark,
"focused CPU at most 10 % over off", holds at neither speed in relative
terms (calm +15–25 % at top speed, +0.01–0.02 s at the demo's own speed,
where off is 0.2 % of a core); in absolute terms calm is a tenth of a
percent of one core. Blurred, motion costs nothing. The demo's world
moves on every step and lights a row every few seconds, so this is the
upper end; a live team between turns only breathes.

**Points.** Growth: `glows` holds at most one per agent and per item,
each gone at its end (1.8 s); `glow_rungs` one per agent. Read cost: a
step computes the rungs once more and walks the inbox from the last
length seen; a drawn frame computes `next_motion` (the rungs once, the
agents twice). Ten times the agents: the same per frame, since the
frames per second do not grow with what moves. Failure: a frame that is
late is drawn at the next look (the loop's poll at most a tick).

**Not done.** The design's breathing in RGB tones of the theme (DIM is
used: one attribute, every theme, mono too). Nothing animates on the
board or the code map. No reduced-motion hint is read from the system
(a terminal does not pass one); `:motion off` is the person's switch.
The activity bar per row is part 5 of #200, in the activity bar below;
`off` leaves it out too.
