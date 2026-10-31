# Notes on a change's lines

Notes on diff lines (2026-10-31, backlog D1 in #57, from Orca and Warp,
where a review is comments on lines sent as one batch): a change that
was nearly right had two answers, apply it or reject it and say why in
a message of one's own, with the places typed out by hand.

The diff view's top row is the line at hand now (drawn as a selection
is), the line `c` writes a note on and the one `>` takes its hunk from
(record 47); `j k` and the pages move the lines under it. `c` on a line
of a hunk (context, added or removed; not a header, a `@@` line or "\ No
newline") opens the command line for the note, labelled with its place
("note on src/p.rs:12 ▸"); `enter` keeps it, `esc` gives it up, `c` on
a line with a note writes it again, and an empty one takes it away. No
names or commands are offered while a note is written. Off a hunk's
lines `c` says where a note goes. A note shows under its line in the
diff, each of its lines after `✎`, cleaned.

A place is the file's new name and the line's number there, or for a
removed line its number in the old file with a minus sign
(`src/p.rs:−7`), counted from the hunk's `@@ -a,b +c,d @@` (`ui::place`;
context lines move both sides on, added the new, removed the old). The
hunk a line is in is the nearest `@@` above it with no `diff --git`
between, so a removed `-- x` (which reads `--- x`) is a line, not a
file's header.

`n` sends the change's notes (in line order) to its worker in one
message (`ui::notes_message`): a line saying the change stays in its
worktree, not applied, to change as they say and send done again; then
each note under its place and the line itself in a `diff` fence. The
change goes back as `r` sends it back (`reject_change`: the inbox item
closes, the worktree keeps it, the worker's next `done` is a new Δ), so
the journal says it was rejected and the message follows as said to
the worker; the flash counts the notes. With no notes `n` says how to
write one. The demo's changes take notes the same way, and its worker
takes the message as its next card, as a rejection's reasons.

Notes live in the screen's memory only (`App::diff_notes`), each kept
against its change's patch by hash: a newer Δ from the worker has other
lines, so older notes are not shown or sent with it; sending drops the
worker's notes. Not kept between runs.

While a note is written the footer says "enter keep", not "send".

Not done: notes on several changes are not listed anywhere but on
their diff.
