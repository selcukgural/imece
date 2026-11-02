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
timeout)` polls the terminal's descriptor itself: nothing within the
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

**Failure.** One window is left: bytes read by crossterm, then the
terminal gone before crossterm's own loop reads again and finds half an
escape sequence; only then does crossterm's loop spin. It is a
microsecond after a key, never while imece sits waiting, which is when
terminals are closed.
