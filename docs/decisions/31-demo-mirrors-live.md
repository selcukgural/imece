# The demo mirrors live

The demo mirrors live (user, 2026-10-30): every fix, feature or change to
what live does is shown by the demo (`--demo`, `sim.rs`) in the same PR,
and a PR that cannot fake something says so. `sim.rs` was rewritten to
produce live's events through the same `World` calls live uses:
the leader posts cards (`Posted`, at the start too) and answers a request
with the cards it posted (`Replied`, a Reply item); a free worker takes a
card by the leader's `assign` or claims it from the board (`Claimed`);
workers talk in typed messages (`Sent` assign/ask/answer/progress/done)
and tell the person what they assume (`ToYou` inform); a working worker
shows what it does (`activity`, `Touched` in the story); its session
asks the person's yes for one of its project's sensitive steps (`⚑`
Approval, held until decided or given up on after 100 s as Claude's
prompt is); it asks the person (`ToYou` ask, a Question item; it waits
40 ticks, then goes on with what does not depend on the answer); a card
ends with imece's `run` of the build and the tests (one test run in six
fails and the worker goes back), a `done` to the leader and a change in
WAITING FOR YOU with a patch made from the card's leases and its runs;
`a` applies it on the next step (`Applied` with a mark, one in eight
`ApplyFailed`), `r` rejects it and the person's reasons become the
worker's next card, as live's prompt does. The leader hires a worker when
two cards wait and nobody is free (up to six), a worker hires a helper
for its tests that sends done and leaves (`Hired`, `Left`), a tiered hire
falls back now and then (`Fallback`); a session starts once (its vendor's
`/` commands; a worker's may say `WorktreeMoved`, `WorktreeKept` or, once
a run, `NotesCut`), stops now and then (`Stopped`, resumed with
`Resumed` when told), and is forgotten with `:forget` or `/clear`
(`Forgotten`, asleep, `Restarted` when told); `/cost` and `/compact`
answer as a turn; a spent pot refuses words and commands and starts no
work; `Confine` is said at the start.
What the demo had and live does not was replaced: the merge queue (trunk
numbers, `Merged`/`Failed`, the verifying status) and the sensitive-change
item; `Status::Planning` and `Status::Verifying`, which nothing made any
more, were removed with their glyphs and words. The model keeps the old
events (the store's format). What stays as a preview of what is planned:
researchers (01), symbol leases and the code map, the Qwen and Local
vendors. What the demo cannot show without a store and an earlier run:
the "where you left off" window (25), `Interrupted`, `Asleep` from a
start. Known gaps: the salon's QUEUE line and the board's VERIFYING
column are shared screen parts that nothing fills any more.
