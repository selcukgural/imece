# When the terminal goes away

Found 2026-11-02 while measuring record 84: an imece whose terminal went
away (its tab closed, `tmux kill-server`) did not end. It spun one core
at 99% for as long as nobody killed it (the person's own imece on Kule:
7 hours) and ignored SIGHUP and SIGTERM, so its sessions were never
stopped and the bus's directory stayed in `$TMPDIR`.

**Why.** crossterm 0.29's reader on Unix (`event/source/unix/mio.rs`)
reads the terminal in an inner loop that ends only on an event or on
"would block". A hung-up terminal answers a read with 0 bytes or EIO,
neither of which ends it, and the loop never looks at the clock or comes
back to the caller, so the main loop never saw the flag its signal
handler had set. crossterm's `use-dev-tty` reader is better (EIO is an
error, the end goes back to `poll` until the timeout) but still spins
for each whole wait on a terminal that says it can be read, and its
feature would reach crossterm past ratatui, which keeps the two in step.

**What imece does.** It asks first (`input.rs`). `input::wait(fd,
wake, timeout)` polls the terminal's descriptor itself: nothing within the
wait is `Quiet`; `POLLHUP`, `POLLERR` or `POLLNVAL` is `Gone`; readable
with nothing to read (`FIONREAD` 0, or EIO asking it) is `Gone` too,
since in raw mode there is no end-of-file key that makes an empty read;
readable with bytes is `Ready`. crossterm is asked (`event::poll(0)`)
only when bytes are there, or while it may still hold events it read
together with others (`input::Guard.held`: its last answer was yes);
then the terminal is asked first with a zero wait, so a terminal gone
in between is never handed to crossterm. `main::poll_terminal` does this
in the main loop, in the who-leads question and while a kitty click
report is read. `Gone` is a quit like SIGHUP's: the loop ends, every
agent's state is kept, the sessions stop, the bus's directory is
removed. The descriptor is stdin when it is a terminal
(`input::terminal`, the one crossterm reads); otherwise crossterm opens
`/dev/tty`, which macOS's `poll` can not wait on, and imece waits through
crossterm as before. The wait is rounded up to the millisecond, so a
short wait does not become a busy return.

**Checked.** `tmux -L … kill-server` under imece 0.1.1: the process at
99% CPU (`Rs+`) until SIGKILL; with this change it ends in about
300 ms. The tests open a pty, make its slave raw like imece's terminal
and close the master: `Gone` at once, not after the wait; a key written
is `Ready` with its byte waiting; nothing is `Quiet` after the wait.
`Guard` is tested with a fake terminal and a fake crossterm, for which
waits and reads it asks. The demo runs the same main loop, so it ends
the same way (record 31).

**A resize (#457, 2026-11-10).** Asking first had a cost found eight
days later: a change of the terminal's size writes nothing to its
input. crossterm hears it as SIGWINCH and gives `Event::Resize` only
when it is asked, and it was asked only with bytes there, so a resize
on a quiet screen woke nothing. imece drew again only when something
else changed (a key, an agent's event, the clock's step), and until
then the terminal showed the old frame as it had moved it about: in
iTerm2 the status line half way down a taller window, every row below
it empty, the preview's lines running off the right edge (the person's
screenshot on the issue). Measured on a pty with the demo paused and
motion off: 0 bytes in the 1.5 s after the size changed. Now the wait
also watches a pipe (`input::Wake::on_resize`: a `UnixStream` pair, both
ends non-blocking, registered for SIGWINCH through signal-hook, the
crate crossterm registers the signal with, so both hear it); a byte
there ends the wait as `Quiet` and is read away. The main loop looks at
the terminal's size on every wake (`screen::Sizes::seen`), so a change
stirs a drawing at once, and once the size has not changed for
`screen::SETTLE` (250 ms) the screen is erased row by row and drawn
whole once more (`Terminal::clear` in the same synchronized frame): a
terminal may move its cells after the frame for the new size came, and
a frame drawn as a difference from the last never touches a cell it
believes blank. The same run measured after: the frame at the new size
and the settle frame, each erasing all 45 rows. The kitty click report's
short waits watch no wake, so a resize never cuts one into keys. When
the pipe cannot be made, the size is still looked at on every wake, a
tick late at most.

**Failure.** One window is left: bytes read by crossterm, then the
terminal gone before crossterm's own loop reads again and finds half an
escape sequence; only then does crossterm's loop spin. It is a
microsecond after a key, never while imece sits waiting, which is when
terminals are closed.

## The `!cmd`'s key, too (2026-11-10)

The acceptance run's scripted cases (record 147) found imece at full CPU,
deaf to SIGTERM, five minutes after its tmux pane had closed: the pane
went while imece waited for "any key: back to the screen" after a
`!cmd` (record 68). `shell::wait_for_key`, which the editor's failure
screen (record 95) shares, read with crossterm's `event::read`, the loop
that spins on a terminal gone. It now waits as the main loop does
(`key_pressed` over `input::Guard` and `input::wait`, a look every
`KEY_WAIT` 250 ms): a press, the terminal gone or a signal (`EINTR`)
ends it, and the main loop then quits as on its own screen. The case
`SESS-8` closes the terminal at exactly that wait, and the runner fails
any case that leaves a process working in its directory five seconds
after its terminal went.
