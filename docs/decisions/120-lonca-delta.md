# The Δ on Lonca

The first half of record 115's step 5, on Lonca `d317b7e`. A worker in a
workspace now has a Δ: what it proposed waits in WAITING FOR YOU, read from
the store, and `a` and `r` are the person's word to Lonca. What is left of
the step is `done`'s gate: a worker that says `done` with work it never
proposed is not refused yet.

**The store is the truth, and imece reads it.** Nothing here hangs on `done`.
Every look (`LOOK_EVERY` 5 s, and at once when a workspace's turn ends or a job
ends) brings each repository's pending proposals and where every change stands;
`World.changes` is a view of that. So a solo, which has no `done`, is the same
path; a restart finds its Δs at the first look; two imeces on one daemon agree;
and `lonca approve` in the person's own shell closes the item as `a` does.
`World.proposals` says which proposal a change is, memory only: the next look
finds it. Only the journal and the inbox row are saved, so a start closes a Δ row
with no change behind it (`World::close_empty_changes`), the look brings back what
still waits, and an open Δ's cost does not come back.

**One Δ per worker, its newest proposal.** Probed: a second `propose` from one
workspace is a new pending change on the same base that holds the first; the
first stays pending. Approve the second and trunk has both; approve the first
afterwards and it integrates too, as an empty commit. Left alone it would come
up as the worker's change the moment the newest was decided. So the older ones
are counted on the Δ (`· holds 2 older`), and a decision turns them down first
(`replaced by proposal 9`), whichever the word: a refused step stops there and
the newest still waits. Each is a signature, so with a Secure Enclave key each
is a finger; lonca#107 asks for a new proposal of a workspace to replace its
older ones, and then this goes.

**Not before its turn is over.** A proposal is a Δ once `turns[id] == 0`. Mid-turn
it may be followed by a better one, and an approval then would race a worker
still editing. The look at the turn's end (`look_now`) brings it. An older Δ
stays on screen while its worker works on; a look after the turn replaces it.

**What is read, and how often.** `status` as ever. `change.diff` once per
proposal, kept in the look thread (`Kept.diffs`, behind an `Arc`, dropped when it
no longer waits, 2 MiB each at most: the daemon's cut, said on the Δ, as redacted
files are). The policy once per commit (`Kept.held`, an answer only). A diff the
daemon can not give is left for the next look: the proposal is not shown, the
count on the project's line still says it.

**`unobserved_is_tainted`.** Record 119 says it: off, nothing would wait for a
person, and a change would integrate unseen. So the look reads the policy commit
`status.policy` names (`tree` with `paths`, the file's bytes in base64, decoded
here since `imece-agent` takes no dependency for it) and anything but a plain
`true` under `[observation]` closes the project, `StoreDown::Unguarded`, as a
daemon gone does: no hire, no Δ decision, `⌀ no lonca` and why. An inline table
or a dotted key is not read, so it is off: a setting imece can not see is one it
does not trust. No file means Lonca's defaults, which hold. Not covered: a session
whose level is above `unobserved` (root-managed hooks) integrates a clean change
unseen with the flag on. imece refuses a hardened daemon (record 116) and
`level_for` gives `unobserved` otherwise, so that needs root on the person's own
machine; when Lonca's hardened mode comes in, this check has to look at the level
too.

**`a`.** `StoreJob::Decide` runs `lonca approve SEQ --repo=R --json` as a child,
`DECIDE_WAIT` 120 s, one job at a time, in the project's checkout. `Live.applying`
stops a second press while it runs. A success changes nothing on screen: the look
it triggers says what came of it, so there is one path for everyone's decisions.
A refusal (`no finger`, `nothing_pending`) is `Event::StoreFailed` with
`StoreAct::Decide`, and the Δ still waits. With `required` above 1 an approval
leaves it pending and the Δ stays; imece does not count approvals.

**`r`.** Lonca takes no rejection without a reason (1 to 4096 bytes; cut on a
letter). Record 11's order, reject then write, can not hold, so `r` opens
`@name ` with `App.rejecting`, and the line sent is both the reason and the
message: `submit` rejects first, and a rejection refused stops the message, so
the worker is not told of a rejection that did not go. A line that is no reason
for that worker (`:`, another name, nothing) rejects nothing and says so, and so
does `esc`. `n` is the same with the notes as the reason. The job can still fail
after the line went (a finger not given); the journal says it, the Δ waits, and
the worker has been told, which is the one lie left here. Waiting for the job
before the message would hold a line back behind a finger with nothing on screen.

**What came of it** (`Live::land`) is said whoever decided: `Landed::Queued` (the
item closes; the change waits in `Watch.queued` for the queue's word),
`Integrated`, `Conflicted`, `Rejected`, `Withdrawn`. An approval integrates before
the call returns on a repository with no verification, so `Queued` is seen only
with a queue that takes time. The worker hears it as a note: on trunk at its next
prompt, sent back at once (`lost`, which now also means this, within `MAX_WOKEN`
and the pot): `conflicted` and a `rejected` it was not told by the person say
sync and propose again. Reasons are cut to `REASON_TOLD` 400. A rejection worded by
the person is not told twice (`Watch.worded`). The cost is carried (record 113)
until a proposal is on trunk.

**`x` and `b`.** A proposal is approved whole; `x` says so. `b` finds no clash,
the queue tells the worker. `:commit`, the partial apply and the clash send-back
stay for a worktree: the git path goes once a team runs on Lonca end to end
(record 115).

**The demo** (record 31). A worker in a workspace proposes (`Sim::ready`), every
fourth proposal holding an older one; `a` lands on the table's trunk or, one in
eight, comes back `Conflicted` and the worker takes it up; `r` asks why. What can
not be faked: a daemon, a signature, a real queue.

**Concurrency.** The daemon is the sequencer. Two imeces can decide one proposal;
the second gets `nothing_pending`, said in the journal. A look reads while a job
runs: at worst it sees the change a moment before the job's end says so. **Read
cost.** `status` lists every change the repository ever had, and the look moves
that list to the screen's thread every 5 s (no copy; a hash lookup per Δ).
**Growth.** Memory: one patch per pending proposal in `Kept`, one per Δ in
`World.changes`; the journal gains a line per outcome. **Ten times the scale.**
`status` at 10,000 changes is what breaks first; Lonca would need a `since`.
**Migration.** Nothing in the schema; `Event::Landed` is a new journal line, an
older build can not read a store that has one, as with every new event.
**Failure.** A daemon gone: the Δ stays, decisions are refused, nothing is lost,
the proposal is in the store. A crash between the job and the look: the next run's
first look finds it decided, and since the Δ is gone from `World` too the worker
is told nothing; its `sync` says.

**Tested on a real daemon** (`--ignored live_a_real_daemon_answers_as_the_protocol_says`,
`live_a_proposal_is_turned_down_and_its_newer_one_lands`, by hand; CI has none):
`change.diff`, `status.changes`, `status.policy`, the default `lonca.toml` read as
held, `reject`, `approve`, `nothing_pending`.
