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
command, ended }` journals it under the project at hand
(`EntryKind::Person`); a shell that could not start is only flashed.
The output is not captured (no pty) and never reaches an agent; it
stays on the terminal's main screen. `w` took over "straight to
waiting for you". The terminal dance is excluded from mutants;
`run_in` (the spawn, the cwd, exit code vs signal, the SIGINT swap)
is tested, with the tests that swap the handler taking turns.
