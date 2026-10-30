# Change review v0

Change review v0 (2026-10-29): a worker's `done` (role Worker only;
a helper's done is its worker's business, a leader changes nothing)
makes imece read its worktree through a private index file
(`worktree::change`: `read-tree HEAD`, `add -A`, `write-tree`, `diff
--cached --binary` against HEAD, untracked files included, the
agent's own index untouched; a patch over 4 MiB is refused and the
worker told). The result is `InboxKind::Change` in WAITING FOR YOU
(`Δ`, detail = the done's first line) plus `World.changes` (one per
worker, a newer done supersedes); the send answer tells the worker
the person sees it. Screen: `enter` → `Zoom::Diff` (`ui/diff.rs`,
every line through `clean`, +/- coloured, `j k` / pages / `gg G` /
`[ ]` file headers), `a` → `Live::apply_change`: `git apply --index`
in the checkout (atomic; `update-index --refresh` first so a touched
but unchanged file is not a mismatch; refused with git's error lines
on an unstaged edit or a patch git can not read, journaled
`ApplyFailed`, the item stays; see "Three-way apply" below for a
checkout that moved on under the patch), then `worktree::applied` commits the reviewed tree in the
worktree as `imece <imece@localhost>` and `git reset -q` to it (files
untouched, index = HEAD) so the next change is incremental; `r` →
`reject_change` (item closes, the change stays in the worktree, the
composer opens `@worker ` for the reasons). Either way the worker
gets a note from imece at its next prompt (`Live::notes`, "From imece
(not from the person or a teammate)"). `worktree::place` lets a clean
worktree whose own commits are all imece's follow the checkout (it
stays while the checkout has not moved past them), and `Live::follow`
runs it before every prompt of a worker that is between turns (its
helpers too), journaled `WorktreeMoved`. The sim has no changes.
Known gaps: the wake briefing's `worktree::change` (once per
remembered worker per run, at its first prompt) still runs on the UI
thread; no partial apply.

Three-way apply (2026-10-30, the first Kule run: two workers both added
to CHANGELOG, the second Δ did not apply): when the plain `git apply
--index` fails for any reason but the person's own unstaged edits
("does not match index", returned as it is: no merge goes over them),
the patch is merged three-way into a COPY of the checkout's index
(`<git dir>/imece-apply-index`, `git apply --cached --3way`, removed
after), which touches no file; only when that is clean does `git apply
--index --3way` run for real. Git's own three-way on a conflict leaves
markers in the files and the other files applied (probed on git 2.56),
so it never runs unchecked. A conflict is refused as `conflicts with
the checkout in a, b` (unmerged paths of the copy, each once); a patch
that does not merge at all keeps the plain error. The patch's blob ids
are full (`--binary` implies `--full-index`) and the worktree shares
the checkout's objects, so the preimages are there. The same patch
applied again now merges into what it made and changes nothing. The
prompts: the leader gives a file most tasks touch (a changelog, a
version) to one worker at a time and tells the others to leave it out
and say in their done what it should get; a worker leaves such files
out unless its task names them. Symbol leases are still the real fix
for two workers in one file.

Git off the screen's loop (2026-10-30; on a Kule clone reading a change
took ~250 ms, a frame the screen stood still): reading a done's change
and applying one are jobs for a git thread of Live's own
(`changes::git_thread`), one at a time in the order given, their
outcomes taken in by `Live::take_git` at the start of `step`. One
thread, not one per job: two applies never meet in the person's
checkout, and a done sent after an apply is read only once that apply
has marked the worktree, so the new change holds only what came after.
The worker's `send` answer waits for the read (`Live::send` takes the
reply; `message` is the part before git), as a `run` waits for its
command. `a` only starts the apply: the flash says it is under way,
the journal says how it went (`Applied` / `ApplyFailed`). Refused, with
why: `a` while that change is being applied or while a newer one of
the worker's is still being read (`reading`), `r` while it is being
applied (`applying`; the flash `reject_failed`). Without its thread (it
can not end while Live lives, but a send can fail) a done is answered
"imece could not read your change" and an apply is refused.

`follow` on the git thread too (2026-10-30; `place` is ~110 ms and ran
before every worker prompt): a worker between turns gets a
`GitJob::Place` for the session's generation, and its prompts wait in
`Live::placing` (the session stays `Slot::Running`, so approvals,
commands and the end still reach it) until `GitDone::Placed` comes
back; then they go in order. A session that ended meanwhile (its
generation moved on, `end` empties the queue) hears nothing and
nothing is journaled; a prompt the session refuses then stops it, as
a refused prompt does anywhere. The bookkeeping (turn count, card,
status Working) happens when the prompt is given, not when it is
sent, so nothing offers or delivers to the worker twice meanwhile.

Done needs evidence (2026-10-30, the user's decisions: only imece's own
`run` counts, tied to the tree; shown, never a gate): a run of a
worker or helper (their directory is the worker's worktree; the
leader's runs are not kept) takes `worktree::tree` before and after
it, on the run's own thread (`read-tree HEAD`, `add -A`, `write-tree`
through an index file of its own per call, so untracked files count
and the agent's index is untouched). Only when the two are the same
is it kept (`Live::ran`, the newest 64), sent on `ran_back` before the
agent gets its answer, so a done that came after it finds it. A run
that changed the tree (a formatter, a test that writes a file git
sees) counts for none. When git has read a done's change, the runs on
exactly its tree, the last of each command line, go with it
(`Change::runs`, saved in `changes.runs`, schema 5). The screen: the
`Δ` line ends with `✓ ran clean on this tree: …` (green), `✗ failed on
this tree: …` (the failed ones, red) or `no run on this tree` (warn);
the diff starts with one line per run (`✓ line`, `✗ line: exit N`,
`killed at the deadline`, `killed`) or says that whether it builds and
passes its tests is not known. Nothing more: the done is not refused,
the worker is not told after the fact, and `a` applies a change with
no run as before; the person decides. A vendor's own shell does not
count: imece does not see what tree it ran on, Claude's and Gemini's
events carry no exit code, and "failed" means something else per
vendor. So the worker's instructions say to check with the run tool
after the last edit, and that a check in its own shell does not show.
Known gaps: a run takes two tree reads more (each the `add -A` of
reading a change, without its diff); a run that edits a
file and puts it back between the reads counts; ignored files
(`.env`, `target/`) are not part of the tree, so a run that depends on
them is not told apart.
