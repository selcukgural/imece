# Leases on the code map

The first half of record 115's seventh step. The code map (record 17) has shown the
demo's cards since it was drawn, and nothing live: "no shared trunk yet". A project on
Lonca has one, and its daemon knows who holds what, so its map is that.

**What is read.** `status.leases`, on every look (`lonca::leases_of`): each lease set's
`items` and the items of its `pinned` changes, a symbol of a holder once, by file and
symbol. A lease names a session; the holder's name comes from `status.sessions`, and a
session not listed there reads `?`. `path#Qualified.name` is cut at the first `#`.
Fails closed as the rest of `Status::of` does: a lease with no session or a symbol that
is no text is no answer. So a pinned item counts as held, which it is: another session
is denied it until the change leaves the queue (record 120).

**What is kept.** `OnLonca.leases: Vec<StoreLease>`, memory only, what the last look
read (`World::store_leases`, the look says the screen changed only when it moved), and
gone with a store that goes down: a lease the daemon can not vouch for is not drawn.
`Live::owner` makes a name an agent of the project, the same reading as a proposal's;
a helper works under its worker's session and shows as its worker.

**What is drawn** (`ui/code.rs`, `map_of`). On Lonca, only the files held: a repository
can have 20,000 files and the map has no scroll, and what the person asks of it is
where work is. An agent's cell as ever, glyph and colour; anyone else's `· name`, dim:
a second imece, the person's own `lonca` session. `⇆ N` counts holders of either kind.
Nothing held says `nobody holds a lease in NAME now`. A project off Lonca says what it
said, and the demo keeps its map of cards, on Lonca or not: its table has no leases,
and `Project.repo` is what the check looks at. What cannot be faked: a real daemon's
list; the rows are the same rows.

**Read cost.** Nothing new on the wire: the look's `status`. One `Vec` compared per
look. **Growth.** A project's leases, replaced whole; a few per agent. **Ten times the
scale.** `leases_of` walks `sessions` per lease, small lists both; a map longer than
the screen is cut, the legend still counting. **Concurrency.** The daemon's list as of
the look, up to 5 s old: a line to read, never a decision. **Failure.** A look that
fails leaves the last list; a daemon down clears it. **Migration.** Nothing saved.

Not done: `file::name` completion from these (record 67), and the owned paths as a
session's paths, the second half of the step.
