# The quit question

The quit question (2026-10-31, backlog A7 in #54, from Warp and cmux)
counts what quitting now would leave behind: one `q` asks, and when
something is in flight the question says how much ("quit imece? agents
mid-turn: 2, changes not reviewed: 1 · q again quits · s shows them ·
any other key stays"); with nothing in flight it is the plain question
as before. A count of zero is left out.

Mid-turn is `World::mid_turn`: agents not gone whose status spends
tokens (`Status::busy`: working, researching), the lead included; an
agent asking a teammate, blocked, waiting for the person, stopped or
asleep is not mid-turn, since quitting cuts no turn of theirs. Not
reviewed is `World::open_changes`: the open Δ items in WAITING FOR YOU.
Other waiting items are not counted here: they are in the title and the
header already, and quitting cuts none of them.

`q s` opens "BEFORE YOU GO": the agents mid-turn, each with what its
salon row says it does (and its project when there are several), and
the changes not reviewed as the waiting panel shows them. There `q`
quits at once, `W` goes to the next item waiting (record 33), any other
key goes back. With nothing in flight `q s` opens nothing. The window is
drawn from the world as it is: a turn that ends while it is open
drops off the list. Pure screen logic: the demo shows it as live does.
