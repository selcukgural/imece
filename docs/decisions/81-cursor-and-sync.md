# The terminal's cursor, and each frame as one update

The first part of the motion and signals plan (design talk of
2026-11-02: what moves on the screen, sound, notices, the cursor, the
tab; its rule is the one of the record on drawing less in another
window: drawn on a change, never on a timer). This part is what the
terminal draws by itself, so it costs no frame.

**The cursor is the terminal's own.** Until now imece hid the terminal's
cursor and drew one of its own: the character under it reversed, a `█`
at the end of the line, a `▏` after the help's and the archives' `/`
filter. Now the terminal's cursor stands where the next character goes
(`Frame::set_cursor_position` from `ui::draw`) and none of those is
drawn. `App::caret() -> Option<(Field, Caret)>` says where and what:
`Field::Line` (the command line), `Field::Help`, `Field::Archives`;
`Caret::Write` (a message, a note, a `!` command), `Caret::Command` (a
`:` command), `Caret::Find` (`^r`, `#`, `&`, the two filters), in the
order the line's heading names what is written (`ui::writing`: a note,
`^r`, `!`, `#` `&`, `:`). The window on top decides: the solo pick
hides it, the archives and the help have it while their filter is typed
(else hidden), the news, welcome, away digest, where you left off and
the quit question hide it; an archive's view has its own. A conversation
or a peek window drawn over the footer hides a cursor that would stand
under it (a footer of several lines under the conversation).
`ui::caret_line` counts the columns before the cursor in cells (`ç`
one, `中` two) and puts it at the end of a line `clean` changed (a
hidden token, as before); `ui::caret_in` keeps it on the area's last
column when the line runs past the edge (the line is cut there) and
gives none on a row the area does not have.

**Its shape says what is typed.** `ui::look` turns the caret into a
`screen::Look`: a blinking bar (`CSI 5 q`) to write, a steady bar
(`CSI 6 q`) in the theme's accent (`OSC 12;#rrggbb`) for a `:` command,
a blinking underline (`CSI 3 q`) to find; the others keep the terminal's
colour (`OSC 112`). Only an RGB accent is sent: a palette colour (the
`terminal` theme, base16 schemes rounded to 256, `mono`) is the
terminal's to choose and has no `#rrggbb` imece could name, so the `:`
cursor keeps the terminal's colour there. `screen::Cursor` writes the
look only when it changes (like the title), and forgets it when the
terminal comes back from a `!` command. The terminal blinks the cursor:
no frame, no timer, nothing for the blurred window to save.

**Given back on every way out.** `screen::give_back`, which the end, the
panic hook (`screen::restore`) and the `!` command's step aside all go
through, now starts with `CSI ?2026l` (a frame cut short by a panic no
longer holds what follows), `CSI 0 q` and `OSC 112` (the shape and the
colour of the terminal's settings) and `CSI ?25h` (shown; the step
aside's own `cursor::Show` went with it). A signal (TERM, HUP, INT) ends
the loop and leaves through the same restore.

**Each frame as one synchronized update.** `screen::synced` writes
`CSI ?2026h` before a drawing and `CSI ?2026l` after it whatever the
drawing returned, then flushes; the main loop's frame (with the cursor's
look), the lead question and the probing screen all go through it. A
terminal with mode 2026 (iTerm2, kitty, WezTerm, Ghostty, Windows
Terminal, foot, Alacritty) shows the frame at once, never half drawn; one
without it ignores both (Terminal.app). 16 bytes a frame.

**Points.** Nothing grows: one look kept, two windows' rectangles per
frame. Read cost: `App::caret` reads a few flags per frame and per
drawing of the look. Failure: a terminal that does not know DECSCUSR or
OSC 12 ignores them and keeps its own cursor; tmux keeps a cursor
shape per pane and passes it on where it knows the outer terminal's
(`Ss`), unverified here; a terminal that does
not answer mode 2026 (none is asked: it is set, not queried) costs
nothing. The demo draws through the same `ui::draw` and `main::run`, so
it shows the same cursor; there is nothing the demo fakes here.

**Not done.** The terminals were not probed one by one (the plan's
support table is from their documentation; Terminal.app's DECSCUSR and
tmux's mode 2026 are unverified). The cursor's colour does not follow a
palette accent. The vim-style cursor shapes of other places (a block
while choosing lines to copy) were left out: choosing has its own `▶`
and reversed words, and no text is typed there. The rest of the plan
(the tab's progress `OSC 9;4`, `OSC 777`/kitty `OSC 99` notices,
`:motion`, the activity bars) comes in its own PRs.
