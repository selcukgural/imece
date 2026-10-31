# Apply part of a change

Hunk by hunk (2026-10-31, backlog D2 in #57, from Terax and Warp; the
known gap of record 11): a change with one good hunk and one wrong one
could only be applied whole or rejected whole.

`x` in the diff view chooses the hunk at hand (the one the top row is
in, or on a file's header the next one, as `>` finds it,
`ui::hunk_at`); `x` again lets it go. A chosen hunk's `@@` line wears
`✓`, and the flash counts them ("1 of 3 hunks chosen; a applies only
them, the rest waits"). With hunks chosen, `a` applies only those; with
none, the whole change, as before. The choice is kept in the screen's
memory against the patch's hash (`App::chosen`), so a newer Δ starts
with none. The footer's `a x  apply · choose a hunk` and a help line
name it, in both languages.

On imece's git thread the part becomes a change of its own before
anything is applied: `worktree::subset` keeps the chosen hunks under
their file's header (a file with none is left out, and so is a binary
file, which has no hunks), and `worktree::part` reads the tree the
worktree's HEAD makes with only that applied, through an index file of
imece's own (`git apply --cached` on `read-tree HEAD`), so the agent's
index and files are not touched; a part that does not fit is refused
like a change that does not apply, and the change still waits. That
part goes through the same apply as a whole change (record 11, three-way
when clean) and the same mark: the worktree is committed as imece with
only the part, and `git reset` leaves the files as they were, so the
rest is what the worktree holds against its new HEAD. That rest is read
again at once (`Live::snapshot`, nobody waiting on its answer) and waits
in WAITING FOR YOU as a change again, under the same title. The worker
hears that part of its change was applied ("1 of its hunks"), that its
worktree is marked with only that part, and that the rest waits.

The demo mirrors it: its applied change's rest (`subset` of the hunks
not chosen) is a change again, the worker's row counting its lines.

Not done: choosing single lines inside a hunk; the choice does not
outlive the run.
