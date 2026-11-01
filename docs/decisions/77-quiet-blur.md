# Drawing less in another window

Backlog C18 in #56 (from Terax; issue #179, 2026-11-02). The main loop
drew the whole screen on every turn: every tick (100 ms live, 500 ms /
speed in the demo) and after every event, also while the person worked
in another window. ratatui sends only the cells that changed, so the
cost was already small, but the terminal still parses every spinner and
clock step, and imece builds every frame.

**What changed.** `App::draw_due(now)`: drawn always while the terminal
has the focus or never says (`App::blurred` is none: no focus reports,
tmux without `focus-events on`); without the focus, drawn when something
came from the terminal since the last drawing (`App::stirred`, set by the
main loop for any event: a key, a paste, a resize, the focus report
itself) or `BLURRED_DRAW` (1 s) after it. `App::drawn(now)` records the
drawing. Everything else in the loop runs as before: the vendors' answers,
the title (which counts what waits), the notices of record 60, the key
file's watch, the ticks and the agents' turns. Only the drawing waits, so
what changed shows at most a second late in a window beside the one the
person works in, and at once on the way back (the focus report stirs).

**The measurement.** `tools/measure.py` (stdlib only, `python3 -I`) runs
`target/release/imece --demo` on a pty of 160 × 48 with a fresh state and
config folder, closes the first-start window, optionally reports the
focus lost (`CSI O`, as a terminal does), and after `--seconds` quits
with `q q`; `--fast` presses `+` three times (speed 8, a tick of 62 ms,
closer to live's 100 ms than the demo's 500). It reads the pty all along
(a pty not drained keeps an exiting imece in the kernel's tty close on
macOS, see the note on killed test processes) and ends whatever did not
quit, TERM then KILL. It prints the CPU time (user + system, from
`wait4`), the peak RSS and the bytes the terminal was sent, per run.

On an M3 Max MacBook Pro, 15 s with `--fast`, two runs each:

| | before | after |
|---|---|---|
| focused | 0.50–0.56 s CPU, 0.64 MiB drawn | 0.52–0.56 s, 0.64 MiB |
| another window | 0.54–0.56 s CPU, 0.64 MiB drawn | 0.10 s, 0.17 MiB |

Memory is 6 MiB either way. At the demo's own speed the figures are
0.09 s and 0.10 MiB in 15 s, too small to tell apart.

**Not done.** The measurement is not in CI: a shared runner's CPU time
varies more than the difference it would guard, and the figures are for
the person to run where they care. Live sessions are not measured (they
need signed-in vendors); their loop is the same, at 100 ms. The poll
timeout is not lengthened while blurred: the ticks drive the agents'
turns and the notices, not only the drawing.
