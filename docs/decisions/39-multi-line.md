# Writing over lines

Backlog item C10 of #56 (2026-10-31, from cmux, Warp and Claude Code;
issue #77): a message to an agent can run over lines. Before, the
command line had one line, and enter sent it.

The key is shift-enter, the person's choice: Claude Code uses it, and
people who code are used to it. Most terminals send shift-enter as a
plain enter (`\r`), so imece cannot tell them apart unless the terminal
speaks the kitty keyboard protocol and is asked to. The start-up probe
(record 15) adds the protocol's query, `CSI ? u`, before the cursor
position report it already waits for; a terminal that speaks it answers
`CSI ? flags u` first (kitty, Ghostty, WezTerm with
`enable_kitty_keyboard`, foot, Alacritty, iTerm2). Then, while imece has
the screen, it pushes the flags with disambiguate only (`CSI > 1 u`):
shift-enter comes as `CSI 13 ; 2 u`, which crossterm reads as enter with
shift; plain keys and text are sent as before, and no release events
are asked for. The flags are popped (`CSI < 1 u`) before the alternate
screen is left, whenever the terminal is handed back (the end, a panic,
a `!` command), so the shell and the command get the keys they know.
The probe's answer is kept in `screen` (for those hand-overs) and as
`App::shift_enter` (for what the screen says).

Where the terminal cannot tell them apart, the fallback is Claude
Code's: enter just after a `\` turns the `\` into a new line instead of
sending. It works in every terminal, and for a `!` command it is what a
shell does anyway. Alt-enter starts a new line too, where Option is
Meta. Ctrl-j was not taken: it is enter in readline, and two fallbacks
are enough to remember. `?` lists the new-line key after the writing
keys, the one this terminal has (`help_newline`, two lines in each
language), and the footer's hint names it: `⇧enter new line` or
`\ enter new line`.

Editing (`edit.rs`): the keys that go or cut to an end (ctrl-a, ctrl-e,
home, end, ctrl-u, ctrl-k) stop at the ends of the line the cursor is
on, as in Claude Code and zsh's multi-line buffer; for a text of one
line nothing changes. `↑` `↓` go a line up or down, keeping the column
in characters (a shorter line's end when it has fewer), and stay put on
the first or last line; while the completion menu is shown they still
step through it. Backspace at a line's start joins it to the one above;
the word keys cross lines, a new line being a space to them. The menu
is offered for a text of one line only: names and commands are on one.
`@name` ends at a space or a new line, so `@name` shift-enter text goes
to that agent.

Drawing: the footer takes one row per line of the text, at most eight
and a third of the screen. The first row carries who the text goes to,
the others are indented under it, and the send / cancel hint follows
the last row shown. When not all lines fit, the cursor's line is kept
in view (rows above it go out of view first) and the hint starts with
where the cursor is, `3/12`. Each line is drawn through `clean` with
the cursor as before (record 37).

Paste: bracketed paste is on while imece has the screen (`CSI ? 2004 h`,
off when the terminal is handed back). Without it a pasted text came as
key presses, and its first line end, an enter, sent the first line as
a message, spending a turn on half a request. Now a paste is one event:
while writing it goes into the line at the cursor whole, line ends
(`\r`, `\r\n`) as new lines, tabs kept, any other control character (an
escape sequence's ESC) left out. A paste while not writing, or with a
window over the line (help, while you were away, where you left off,
the quit list), is left out, and the footer says to start writing
first: its characters would otherwise be taken as keys.

The demo shows it as it is: the keys, the probe and the paste are the
screen's own, the same in `--demo` and live; the simulation takes a
text of several lines as live does (a card's title is its first line).

Probe facts (2026-10-31, `imece --demo` in a pty answering as a
terminal would, read back with pyte): with `CSI ? 0 u` answered, imece
pushed `CSI > 1 u`, `CSI 13 ; 2 u` started a new line, a bracketed
paste of `one\rtwo` went in as two lines, enter sent all of it, and
quitting popped the flags and turned bracketed paste off. Without the
answer it pushed nothing, and `\` enter started the new line.

Gaps: a line wider than the footer is still cut at the right edge
(record 37); long lines are not wrapped. The journal and the salon show
a message of several lines on one line (`clean` turns a new line into a
space); the conversation (`v`) shows it with its lines. A terminal
multiplexer (tmux, screen) that does not answer the query gets the
fallback, `\` enter, even when the terminal around it could do more;
not tried yet.
