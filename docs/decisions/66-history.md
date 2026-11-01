# The lines sent: `↑` `↓` recall, `^r` search, `:history`

Backlog item C5 of #56 (2026-11-01, from Warp; issue #146): a history of
everything the person sent, with a ctrl-r search.

What is kept. Every line the person sends from the command line (a
message, a `:` command, a `!cmd`), as typed and trimmed, with the tick:
`World.sent: Vec<Sent>`, `World::recall_line`, which does not keep the
last line again (a shell keeps one of a run of the same line). Not the
lines that only go somewhere: `#12`, `&3`, `:goto`, `:history`
(`app::recalled`). The store writes the new ones at each save (`sent`,
schema 13, numbered on from the file's last), a load brings the newest
`SENT_KEEP` (500) in order, and the tidy (record 24) deletes the ones
past the newest 500, which a load would never read. The demo keeps them
for its run; the store is live's.

`↑` and `↓` while writing, when no menu is up: at the text's first line
`↑` brings the newest line sent, then older ones; `↓` goes the other way
and, past the newest, brings back what was being written (`App::recall`:
the place shown and the draft). Before the oldest it stays. Writing on a
line brought back makes it the person's own again (`moved` drops the
recall), so `↑` starts from the newest once more and `↓` after it brings
the written line. Between the lines of a text of several lines `↑` and
`↓` move as record 39 says; only at the ends do they recall.

`^r`, from anywhere or while writing, and `:history [WORDS]`: the line
opens (or stays) with the words and `App::searching` puts the lines sent
on the completion menu (`complete::history`): the ones with every word
in them, case and Turkish letters folded as the help's filter (record
40), newest first, each text once at its newest, on one line (`⏎` between
its lines, cut to sixty characters: `complete::one_line`), with how long
ago (`words::ago`). The footer says `› history:`. `tab`, `↑` and `↓`
step (tab does not insert: the item shown is not the text); `enter`
puts the line chosen on the command line whole (`take_sent`, through
`complete::history_picks` so a line of several lines comes back as it
was) and ends the search, to send with `enter` again or to change first;
with nothing matched `enter` keeps the words and ends the search, sending
nothing. `esc` closes the menu, then the line. `↑` with no menu leaves
the search for the newest line. Nothing sent yet: `no_history` flashed,
the line not opened. The help row `tab ↑ ↓ ^r` carries it (the `⇧tab`
of the old row dropped from the key column to fit the thirteen columns;
`⇧tab` still steps back).

Not done: a search over what agents said (that is C13, full-text search);
a per-agent history (`@name` is a word to search for); the time of a
line in the menu as a clock rather than an age; the age on a line from
an earlier run is measured in the world's ticks, which stand still while
imece is closed (record 24: the clock knows days, the ticks do not).
