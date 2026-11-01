# Own shell

Own shell (2026-10-29, user's ask: `!` as in Claude Code): `!cmd` in
the composer (key `!` opens it prefilled, like `:`) runs `cmd` in the
person's own shell (`$SHELL`, else `/bin/sh`) in the checkout, vim's
`:!` way (`shell.rs`): the main loop steps the screen aside (raw mode
off, alternate screen left), the shell inherits the terminal, SIGINT
is ignored in imece and reset to default in the child (`pre_exec`) so
ctrl-c stops the command and not imece, a worded line says how it
ended and one key press (raw mode, so no newline) brings the screen
back (a fresh `Terminal`, not `Terminal::clear`, which asks the
terminal for the cursor and waits 2 s on a plain pty). `Event::Shell { leader,
command, ended, took }` journals it under the project at hand
(`EntryKind::Person`); a shell that could not start is only flashed.
How long it ran (2026-10-31, backlog C22 in #56, from Warp): `run_in`
times the child from the spawn to its end (`shell::Ran { ended, took
}`), the journal keeps it in milliseconds (`took: Option<u64>`, none
in a line saved before, which loads as it was and is worded without
it) and says it after how it ended in the largest units that say it
(`40 ms`, `2 s`, `1 min 5 s`, `1 h 2 min`; `words::took_words`), as do
the flash and the line under the output (`[imece] exit 2 · 3 s · any
key …`). The exit code was there from the start. The demo runs `!`
in the same shell, so it shows the same.
The output is not captured (no pty) and never reaches an agent; it
stays on the terminal's main screen. (Replaced 2026-11-01 by record 68
`own-shell-output`: the command runs on a pty of imece's own, its keys
passed through with the person's terminal in raw mode, its output
copied to the screen and its end kept in memory for `O`, which quotes
it in a message the person sends; the rest of this record stands.) `w` took over "straight to
waiting for you". The terminal dance is excluded from mutants;
`run_in` (the spawn, the cwd, exit code vs signal, the SIGINT swap)
is tested, with the tests that swap the handler taking turns.
