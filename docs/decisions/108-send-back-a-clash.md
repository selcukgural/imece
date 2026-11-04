# Send a clash back

A change that clashes goes back with what clashes (2026-11-04, backlog
D5 in #57): when git refused a Δ, the person could reject it and write
out by hand which files did not fit, or read git's words and pass them
on. The worker is the one who can resolve it; it only needs to know
what to resolve.

When an apply fails, the git thread (the same job, so nothing else moves
the checkout meanwhile) asks `worktree::clashes` what of the whole
change does not fit, the hunks chosen or not: file by file, and in a
file that does not fit hunk by hunk, each with `git apply --cached
--check` and, when that fails, `--cached --3way`, the same two steps
`apply` takes. A part only a merge takes is no clash. It runs on a copy
of the checkout's index (`<git dir>/imece-clash-<pid>-N`, gone after,
the merge's stages thrown away after each try), so the person's index
and files are not touched. A file that does not fit while each of its
hunks does, and a file with no hunks (a binary one), is named whole. The
why of each is git's own words on the plain try (`patch failed:
log.md:26; log.md: patch does not apply`).

The copy is of the index and not of HEAD, though the worker can only
reach HEAD: what stops a change is often another worker's change,
applied and staged and not committed yet. Those files are named
(`Clashes.staged`), and the message gives the command that shows what is
staged there, `git -C <checkout> diff --cached`.

What is found stays beside the change, `World.clashes` by worker, in
memory only (no schema change; after a restart `a` finds it again), and
goes with the change (`forget_change`: applied, rejected, or a newer Δ).
`clashes_of` says nothing without a change that waits. A failed apply
that finds nothing takes away what an earlier one found: that was about
a checkout gone by. Nothing found is kept as nothing: the person's own
unstaged edits (`does not match index`) stop an apply and clash with no
hunk, and they are the person's to settle, not a worker's; so are a
chosen part that does not fit its worktree and a check git could not
run (traced `clashes: …`, as when the checkout has no index).

The screen says so in three places while it is kept: the failure's
journal line ends `· b in the change sends it back`, decided at draw time
(`words::event`, no change to the event); a red line above the
change names HEAD and each file once (`ui::clash_line`, cleaned by
`fill`); the footer gets `b  send back` after `r`. `b` was free in the
change view. The help keeps its size: it shares the `x` row (`x  b`),
since a help row more pushes the legend out of a window that fits it now. It has no `:` name, as `a r c n x` have none
(record 41): they act on the change at hand.

`b` (`App::send_back`) goes the way `n` does (`App::goes_back`, shared;
record 49): `reject_change`, so the item closes, the journal says
rejected and the worktree keeps the change, and one message as the
person's (`ui::clash_message`): the first line says it does not apply and
HEAD's short id (a card's title in the demo), then what to do, then each
clash under its file with its `@@` line in a `diff` fence and git's words.
Every word from git goes through `clean`. The worker is told to bring its
worktree up to that commit with its change on top and still not
committed: imece reads a change against the worktree's HEAD, so a commit
of its own would hide it. A solo is told to end its turn instead of
sending done. Neither imece nor the message resolves anything, and the
worktree is not moved for it: `follow` leaves a worktree with work in it
where it is. Its next `done` is a new Δ.

Before anything goes back, `b` refuses and leaves the Δ as it is when
nobody would hear: a worker that is gone or `Stopped` (`back_gone`; a
stopped one would start again on a message, but the person should choose
that), or a spent pot (`budget_spent`). `n` does not check these. While
it is being applied, `reject_change` refuses as for `r`, and no message
goes.

Limits: `clashes` tries at most `CLASH_TRIES` (200) hunks and the message
then says it stopped looking, since a hunk that fits costs a process and
git's thread does nothing else meanwhile; only files that do not fit are
gone through, so this takes a very large change to a file the checkout
also changed. So at most 200 entries are kept per worker, and the message
is bounded with them. A stale answer is possible: what `b` sends is what
the last `a` found; the checkout may have moved since, and `a` again finds
it afresh. The check runs only after a failed apply, which the person
started; nothing reads per frame but the map lookup of `clashes_of`.

The demo mirrors it: its one apply in eight that fails names the
change's first file and hunk, a made-up HEAD, and git's words; `b` sends
it back the same way and the worker takes it as its next card, as a
rejection's reasons. What it cannot fake: a real merge, and staged files
(`staged` stays empty there).

Superseded in part by "Lonca as the store" (record 115): on Lonca a change
that does not merge is `conflicted` in the queue, and the worker's `sync`
writes trunk's version beside its file; imece's part becomes showing that
status and telling the worker to sync and propose again. `b` and the
clash search stay until a team runs on Lonca.
