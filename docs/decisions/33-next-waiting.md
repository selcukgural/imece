# The next thing waiting

`W` and `S` (2026-10-31, backlog A4 in #54, from cmux and openharness):
one key from anywhere to what waits for the person, one after the other.

`W` works on every screen (the board, the code map, a card's screen, a
diff, a conversation, a raw stream): it closes what is open over the
salon, selects the next open inbox item in the waiting panel and opens
it as `enter` does (a change's diff, a question's conversation, a reply's
conversation, which reads it, an agent's focus for the rest), so `a` and
`r` answer it at once. Pressed again it goes on to the item after, and
round to the first after the last. Nothing waiting: it says so ("Nothing
needs you right now") and nothing moves.

`S` puts the item at hand off and opens the next one: put-off items come
after all the others, in the order they were put off, not by age, and
their rows say "later ·" ("sonra ·") after the time, up front, since
a long row is cut at the panel's edge. Putting off one already put off
moves it to the very end. A closed item is never put off, and the list
forgets items that closed.

"At hand" is the item `W` last opened, open or closed since (opening a
reply reads it, an answer closes it, and the panel's selection slides
onto the next row when an item above it closes, so the selection alone
would skip one); when the person moves the panel's selection by hand
(`j k`, `gg G`), that is what is at hand, and `W` goes on from there.
"Next" is by the item's place before it was put off, so `S` moves on to
what came after it rather than back to the top.

Choices: `W` opens rather than only selects, since selecting alone is
what `w` and `j` already do. Put-off items are kept for the run only
(not in the store): putting off is about this sitting, and a fresh start
shows everything in age order again. Pure screen logic, so the demo
shows it exactly as live does.
