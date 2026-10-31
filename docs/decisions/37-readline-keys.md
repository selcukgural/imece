# Readline keys in the command line

Backlog item C2 of #56 (2026-10-31, from herdr and openharness): the
command line edits as a shell's does. Before it, the line only grew at
the end and shrank by backspace; a typo at the start of a long message
meant deleting it all.

The keys are readline's emacs set, the one bash, zsh, psql and most
REPLs share, so they need no learning: ctrl-a / ctrl-e (and home / end)
to the start / end, ctrl-b / ctrl-f (and `←` `→`) a character, alt-b /
alt-f (and alt or ctrl with `←` `→`) a word back / on, ctrl-w cuts back
to the space before, alt-backspace / alt-d cut a word back / on,
ctrl-u / ctrl-k cut what is before / after the cursor, ctrl-y puts the
last cut back at the cursor, ctrl-d and delete take the character under
the cursor. A word for alt-b, alt-f, alt-d and alt-backspace is letters
and digits (`~/src/imece` is three); for ctrl-w it is anything up to a
space, as in bash. There is one cut, not a ring: each cut that takes
something replaces it, one that takes nothing keeps it. Ctrl-d does
not leave the line when it is empty (imece has esc for that). Vim's
normal mode in the line was not taken: the line is short, and esc
already means "back".

How: `edit.rs` is pure, `key(k, text, at, cut) -> Option<usize>`, the
cursor after the key or `None` for a key that is not an editing key
(enter, esc, tab, the menu's keys stay in `App::type_key`). Positions
are byte offsets on a character's edge; moves go by characters, so
`ç` and `ş` are one step. The app keeps the cursor as `App::after`, the
bytes after it, so 0 is the end: every place that puts text in the line
(`:`, `@name `, `#323`, the answer to a question, a rejected change)
starts with the cursor at the end without knowing about it.
`App::cursor` turns it into an offset and moves a stale one to the next
character edge. Esc and sending set it back to 0.

Completion and the cursor: a completion replaces the word the line ends
with (record 12), so the menu is offered only with the cursor at the
end. A key that changes the line or moves the cursor asks for the menu
again when the cursor is at the end, else hides it; a key that does
nothing (`→` at the end) leaves the menu as it was, so the highlighted
item stays. Tab with the cursor inside the line does nothing.

Drawing: the character under the cursor is drawn in reverse, the block
`█` stays for the end. The line is the person's own text, but a pasted
token would still be split by the cursor past `clean`'s hiding, so a
line `clean` changes is drawn cleaned whole with the block at the end.

The demo shows it as it is: the keys are the screen's own, the same in
`--demo` and live.

Gaps: a line wider than the footer is cut at the right edge, and the
cursor can go past what is shown (the footer does not scroll yet). On
macOS the alt keys need the terminal to send Option as Meta (iTerm2:
Esc+; Terminal.app: "Use Option as Meta key"); without it Option-b types
`∫`. Kill-ring cycling (alt-y), undo (ctrl-_), transposing (ctrl-t) and
multi-line writing (C10, Shift+Enter) are not here.
