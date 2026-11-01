# A word in another window

`:notify` (2026-11-01, backlog A2 in #54, from herdr, cmux, Terax, Orca
and Warp): the terminal's title counts what waits (record 16) and the
digest tells what happened once the person is back (record 52), but
nothing fetched them from another window while a change sat for an hour.

Opt-in, off by default: `:notify bell` rings the terminal's bell (BEL;
every terminal has a setting for it, a multiplexer passes it on, tmux's
`monitor-bell`), `:notify text` sends a desktop notification with
imece's words (OSC 9, which iTerm2, WezTerm and Ghostty show; a terminal
that does not know the sequence ignores it, so `bell` is the choice
there; OSC 777 and kitty's OSC 99 are not sent, one sequence rather
than two toasts on a terminal that knows both), `:notify off` neither;
bare `:notify` goes round. The value shows in the `:` menu and is
remembered in the store's `meta` (`prefs`, key `notify`), unlike
`--awake`, since it changes nothing on the machine.

What rings (`notify.rs`): a new open item in WAITING FOR YOU, or an
agent whose need is `Stuck` (blocked, session stopped; one that waits
in the inbox counts once), when it came while the terminal had told
imece it lost the focus (`App::blurred`, the same reports record 52
uses) and is still there `AFTER` (five seconds) later, once each.
`Notices::due` keeps `known` (what was on screen when the person was in
the window, and what rang) and `pending` (seen while away, with when);
back in the window the pending ones are dropped, since they are on
screen now, and whatever goes before its wait is over (a question
answered by the next line, a change the leader took at once) never
rings. The note is imece's words and the agent's name (`words::notice`:
"imece · claude-1 finished: a change waits for you", "… asks you",
"… answered you", "… waits for your yes", "… something sensitive waits
for you", "… is stuck"), never the agent's text; several due at once
are one bell, or one note with the count. The sequences carry no
control character (`notify::ring`, as the title). The main loop asks
`App::notices` after every poll and writes what it gets to stdout.

Pure screen logic over focus events and the model: the demo shows it as
live does. A terminal or multiplexer that does not report focus (tmux
without `focus-events on`) never rings, and never rings wrongly either:
without a blur nothing is pending. Not done: a notification that opens
the terminal when clicked (OSC 777/99 have no such thing portable), a
per-kind choice (a bell for questions only), a sound of imece's own.
