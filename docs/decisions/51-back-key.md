# Back where you were

`^o` (2026-10-31, backlog A5 in #54, from cmux and Warp; vim's jump
list): following what waits (`W`), a card (`#323`), a diff and a
conversation, the person had no way back but retracing each step.

A place is the view, the zoom, the salon's panel and the card whose
screen is open (`app::Place`). `App::on_key` takes the place before a
key and after it; when the key moved the person (and was not `^o`
itself), the place left goes on `App::been`, the newest last, fifty at
most (the oldest goes first). `^o` pops places until one is still there
and is not where the person is: a gone agent's focus or stream, a
change applied or rejected since, a card that is not, are passed over;
it puts the view, zoom, panel and card back (the journal panel with a
row selected). An open conversation is an overlay, not a place: `^o`
closes it first. With nothing left the flash says so. There is no
forward key: `^i` is `tab`, which cycles the panels.

Movement, so no `:` name (record 41). The help's `^o` line and the
README name it, in both languages. Pure screen logic: the demo goes
back as live does.

Not done: the selection inside a place (the salon's row, the inbox
item, the diff's scroll) is not part of it.
