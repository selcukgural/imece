# Board v0

Board v0 (2026-10-28): `post(title, body)` and `board()` for everyone,
`claim(card)` for workers only (the leader's socket does not offer it).
An open card is offered to one idle worker at a time as a turn of its
own (titles only; the body comes with the claim); it goes to the next
worker only after that one's turn ended without taking it, so a card
wakes each worker at most once, never a broadcast. One card at a time
per worker; the worker sends `done` to whoever posted it. Caps: title one
line of 120 chars, 20 open cards per project, 10 posts per agent until
the person speaks again; offers count as message-driven turns.
