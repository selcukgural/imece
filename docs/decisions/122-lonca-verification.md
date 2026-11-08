# The queue's checks, on the journal and the Δ

The fifth step of record 115. Verification on Lonca is the merge queue's: the
repository's `[verification]` in `lonca.toml` (signed, so a change to it is a policy
change) runs on the merged tree after a human's approval, in a sandbox, and the
evidence goes in the log. imece runs nothing. What it adds is that the person and the
worker hear it.

**The order is the other way round from `.imece/check`.** Record 112 runs before the
person reads. The queue runs after: `change.get.evidence` is empty until the change's
last `change.integrate`. Probed on Lonca `9bf19e0`, with a one-second `build` and a
`test`: `approve` returns at once with `released`; `status.changes` says `queued`
with the change in `queue`; a second later `integrated`, evidence `syntax`, `build`
(`exit`, `duration_ms`, `sandbox: seatbelt`, `log`), `tests`. A failing `build`:
`rejected`, reasons `verification_failed:build`, `verification_missing:build`,
`verification_missing:tests` (the last two say what it never got a pass for), `tests`
`skipped`, the log `lonca verify: …` then its output.

**Before the yes.** The Δ says what will run: `after your yes Lonca runs: build,
tests, api`, `nothing runs after your yes`, or `Lonca did not say what runs after
your yes` (`verdict_spans`, `StoreProposal.verifies: Runs`). A person approving knows
whether anything stands behind the yes. Since lonca#196 (`e7463d1`) the store says it
itself: each `status.pending[]` entry carries `runs`, what the queue will run for the
change under the policy in force, in its evidence's words (`build`, `tests`,
`extended` for a change that reaches others, then each named check it owes), and
`null` when the policy can not be read. `Pending.runs` takes it (a field missing, from
an older daemon, or not a list of words is none), the offer carries the newest
proposal's (`Runs::These`, else `Runs::Unsaid`, said in the warning colour, since
nothing then promises a check). imece no longer reads `[verification]` itself: the
earlier `Client::verifies` / `commands_in` read the keys of `lonca.toml` and could not
know what `on_signature_change` or a named check owes one change. A line, not a gate.
A named check's evidence (`check: "named"`, `details.name`) goes by its name in the
journal line, as the Δ said it would run.

**After it.** A look carries the changes imece waits on (`Asked`: those shown and those
queued), and for each that `status` calls `integrated` or `rejected` it reads
`change.get` once (`settled`, `Client::evidence`). A conflict or a withdrawal ran no
check worth saying. `attested` makes `model::Attested` of it, a skipped check no check:
how it ended (`exit 0, 41.2 s`) and, for one that failed, the last `ATTEST_LINES` 6
lines of output, the verifier's own header left out. `Live::land` gives that to
`World::verified`: `Event::Verified`, a journal line `claude-1: Lonca's checks · ✓
build exit 0, 12.4 s · ✗ tests exit 1, 2.0 s · its last line`. A rejection worded by
the person ran no check, so it says nothing.

**The worker** hears what failed in the same note as the reasons, up to `OUTPUT_TOLD`
1,000 characters: it is what it has to put right, and without it `verification_failed:
build` tells it nothing. This is the output of a command on the merged tree, read by a
model as a test's output is. The conflict's note now points at the markers `sync`
leaves in the files (lonca#108), not at `.trunk` copies.

**`.imece/check`** does not run on a project on Lonca: it hangs on reading a worktree's
change, which a workspace has not (`tree_at`). It stays for a worktree and goes with
the git path (record 115). A repository that had one writes the same commands in
`[verification]` and signs it with `lonca policy set`; `docs/guide` says how, with the
toolchain's homes the empty environment needs (`env_passthrough`). imece does not write
that policy: it can not know the commands.

**Failure.** A daemon gone between the two: the status never said it settled, the next
look does. A `change.get` that fails: the change lands with no checks line, and the
worker hears the reasons. A crash between the look and the journal loses the line; the
evidence stays in the log. **Read cost.** One `change.get` per settled change, on the
look's thread; the watched list is what is on screen and in the queue. **Growth.** One
journal line per landing, six short lines at most. **Ten times the scale.** `settled`
walks `status.changes`, the list record 120 already says breaks first. **Concurrency.**
A change decided from another imece is watched only by the one showing it; the other
sees it land and says what its own look read. **Migration.** `Event::Verified` is a new
journal line, as ever an older build can not read a store that has one.

**The demo** (record 31): a proposal says `build, test`, its landing says the checks,
and each fifth merges and fails its tests, the worker told. What can not be faked: a
sandbox, a real log.
