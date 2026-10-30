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
Known gaps: git runs on the UI thread (a huge diff stalls a frame);
no partial apply; the `done needs evidence` line (last run after the
last change) is not yet shown on the item.

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
