# Copy mode

`v` in a conversation or a change (2026-11-01, backlog C7 in #56, from
herdr's keyboard copy mode). The terminal's own selection keeps working,
since imece never captures the mouse (the keyboard-first rule), but
over imece's windows it takes the border and the gutter with the text,
cannot reach past what is on screen, and cuts a wrapped line in two. `c`
and `y` copied one whole message, `C` and `Y` the whole conversation, and
a change could not be copied at all.

**State.** `Copying { anchor, at }` (`app.rs`): the line the choice
started on and the cursor; `span`, `len`, `holds` read it either way
round. A conversation keeps its own in `Thread::copy`, so it closes with
the window. A change's is `App::diff_copy`, kept against the worker and
the patch's hash like notes and chosen hunks (records 49, 50): a newer Δ
has other lines, so `diff_copy_of` gives nothing for it. Leaving the
change view (any key that leaves `Zoom::Diff`, `^o` too) drops it.

**Lines.** In a conversation a line is a drawn line before it wraps:
a message's head, the `↩` line under a reply, each line of its text
(`ui::thread_lines`, one `CopyLine { entry, text }` each). `markdown_src`
draws the Markdown as before and keeps, for every drawn line, the source
line it came from; that is what is copied. A fence is never drawn, so
the cursor never lands on one and a code block copies as its code; a
table's rows copy as their Markdown, its rule as the separator line it
stands for. Heads and `↩` lines copy as read (the drawn spans' text).
`ui::copy_lines` joins lines with a newline and messages with a blank
line. In a change a line is a patch line, the same index as
`diff_scroll`.

**Keys.** `v` opens a choice on the selected message's head, or on the
change's line at hand. While choosing (`App::copy_key`, before the
conversation's and the change's own keys): `j k ↑ ↓` a line, the page
keys (`pgup pgdn space ^d ^u ^f ^b`, through `page`) and `gg G home end`
(through `jump`) move the cursor, no further than the last line; `y`
copies and leaves; `Y` in a change copies the new side; `v` and `esc`
leave (a second `esc` then closes the conversation). Every other key
does nothing, so a key meant for the view (`a` applies a change, `c`
copies a message) is not taken by mistake; `?` and `q` still work. In a
conversation the cursor's message becomes the selection (the title's
`4/5` follows) and the drawing keeps the cursor's line in view rather
than the whole message, which may be taller than the window. In a change
the view scrolls as little as it can (`ui::in_view` on the cursor's
line), so where the choice started stays at the top while it fits.

**What is copied.** `y` in a change copies the lines as the patch has
them. `Y` copies the new side (`ui::copy_patch(.., new_side)`): only the
hunks' added and kept lines, without their mark. Whether a line is in a
hunk is read from the patch's start, not from the choice, so a `+++ b/x`
head is never taken for an added line; a `diff ` line ends a hunk, an
`@@` line starts one, `\ No newline` is left out, an empty line in a
hunk stays. A choice with nothing on the new side says so and stays
chosen. Every copied line goes through `clean`: the clipboard gets no
control characters (a pasted escape could drive the next terminal it
lands in) and no secret-looking words, the same as `c`. The copy itself
is the one `c` already used (`clip::copy`: the system tool, else OSC
52) and its flash.

**Screen.** The conversation's title becomes `COPY · N chosen` and its
key bar copy mode's keys; chosen rows carry `▌` and the selection
background, the cursor's `▶`. A change's title becomes `COPY · N chosen
· y as written · Y the new side · v esc leave`, its footer copy mode's
keys and `Y`; chosen rows the background, the cursor's bold. The
conversation's key bar lost `pgup pgdn scroll` and merged `c`/`C` into
one hint so `v` fits next to `esc close` in a 120-column window; the
`?` help keeps its 55 rows (the copy row became `c y  C Y  v`).

**Changed.** `v` in a change used to fall through to the main screen's
`v`, the worker's conversation with the person; there it is copy mode
now. `:talk` still opens that conversation from the change.

**Demo.** Copy mode works on the screen's own data, so the demo's
conversations and changes copy the same way (record 31); nothing is
faked.

Not done: a copy mode over the journal or a card's screen (`c` copies a
journal line whole); a copy of the rows as drawn, with list bullets and
the table's padding. Choosing within a line came with conversation folds
and notes on words (record 71): word by word, not by column; there `c`
while choosing writes a note on the words, no longer nothing.
