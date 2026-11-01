# A `!cmd`'s output: caught on a pty, quoted with `O`

Backlog item C15b of #56 (2026-11-01, from Warp; issue #151): one key
attaches the last `!cmd` output to a message. Record 13 left the output
uncaught (the shell inherited the terminal), so catching it came first.

How it is caught. `shell::run_in` runs the command on a pty of imece's
own, as `script(1)` does: `libc::openpty` (libc links libutil on Linux),
both ends close-on-exec (a vendor CLI another thread spawns meanwhile
holds neither), the child's `pre_exec` calls `login_tty` (a session of
its own, the slave its controlling terminal and its stdin, stdout and
stderr) and sets SIGINT back to the default. The pty starts with the
person's terminal settings, taken before raw mode (`termios_of`), less
suspend: ctrl-z (and macOS's delayed ctrl-y) is `_POSIX_VDISABLE` on it
(`no_suspend`), since imece waits for the command to end and a stopped
one would hold the screen with nothing to bring it back. Its size is the
screen's and follows it (TIOCSWINSZ when the screen's changes; the
kernel tells the command). The person's terminal is in raw mode while
the command runs, so every key, ctrl-c too, is a byte the pty cooks:
ctrl-c becomes the command's SIGINT, never imece's (SIGINT is still
ignored in imece while the command runs, for one sent to its group).
`pump` is one `poll` loop over the master and the keys (20 ms ticks): the
output is copied to the screen as it comes and its last `KEPT` (64 KiB)
kept, its newlines counted; keys go to the master; an input that ends is
no longer read. It ends two ways, so that a loop never hangs if one is
never seen: the command's end (`try_wait`) and then `DRAIN` (300 ms) for
what was on its way or a child left holding the pty, or the pty's end
(EOF or EIO once no one holds the slave), after which the master is
closed (a command still running is hung up) and the command waited for.

What is kept. `shell::Output::of` turns the bytes into what a terminal
would show (`screen_lines`): CSI, OSC, DCS/APC/PM strings and charset
escapes dropped, `\r` back to the line's start and backspace one cell
back (what a progress bar wrote over is gone), `ESC [ K` / `ESC [ 2 K`
honoured, control characters dropped, a tab a space, every line through
`i18n::clean` (secret-looking words hidden) and trimmed at the end, blank
lines at the end dropped, bytes that are no UTF-8 (a cut at the front)
`�`. It keeps the last `ui::QUOTE_LINES` (40, as `>` on a hunk) and how
many lines came before them (`above`: the newlines seen in the whole
run, so lines cut from the front of the 64 KiB count too). `App.outputs`
holds the newest per project (`current_project` when it ran), in memory
only: never in the store, since it may hold what the person would not
write down; gone when imece ends. A shell that could not start leaves
the last one as it was.

The quote. `O` from anywhere (a change and an open conversation leave it to
the rest), or `:output` (its menu note is the command): the command line
opens as `@NAME ` to the agent at hand when it is of that project (else
the project's head) with, on the lines under the cursor,
`words::output_quote`: `I ran `cmd` → exit N · 12 s` (the person speaking,
so not the journal's "you ran") and, when there were lines, a `text`
fence one backtick longer than the longest run of backticks in them
(three at least), `… lines above: N` first when there were more.
Nothing is sent until the person sends it; `esc` drops it. None in the
project: `no_output`. `O` was free; `>` is the hunk quote in a change and
resizes the panels elsewhere, so it could not be the key here. The `?`
help has it on the `!cmd` row (`!cmd  O`), so the help keeps its rows.

The demo runs `!` through the same main-loop path, so it catches and
quotes the same; nothing is faked. Tests: the pty (a terminal on all
three descriptors, 80 by 24 without a screen, the screen's size and a
change of it, keys in, ctrl-c as a key, an input that ends, a child left
holding the pty, 64 KiB kept of more and all of it shown, the settings
and close-on-exec of a new pty), `keep`, `screen_lines`, `Output::of`,
`output_quote` and `O`/`:output` in `app`. The stepping aside and the raw
mode around `run_in` stay out of mutants (`run`), as before.

Gaps. A command that changes the pty's settings and dies leaves them for
nothing (the pty is closed); the person's own terminal is restored by
imece's raw-mode switch either way. Output a full-screen program drew
(`vim`, `less`) is caught as its escape sequences, so its lines say little;
`O` is for commands that print. A job the command left in the background
keeps writing to a pty no one reads after `DRAIN`, and is hung up when
the master closes.
