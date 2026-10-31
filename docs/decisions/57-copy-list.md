# Paths copied into worktrees

Backlog E13 in #58 (2026-11-01, from Orca; the person decided the
shape on 2026-10-31): a worker's worktree is a fresh checkout, so what
git ignores is not there: `.env`, `node_modules`, `target/`. A worker
then fails to run the app, or builds everything again. Some projects
need those files in each worktree, and only the person knows which.

An explicit list per project, empty by default. `:copy PATH` adds a
path of the project at hand, `:copy PATH` again drops it (as `:pin`
does), and `:copy` alone says the list; a trailing `/` is the same
path. The `:` menu shows the list. It is kept with the project in the
store (`projects.copies`, JSON, schema 10); a list that does not read
is refused like any row the store does not know (record 19).

What may be listed (`copy::literal`, `copy::checkable`, both when it is
added and again at each copy): a relative path of at most 200 bytes,
literal (no glob, backslash or control character, no `.` or `..`, not
in `.git`); there in the checkout; not a link itself, and inside the
checkout once its directories are resolved (a directory that links out
is refused); and ignored by git (`git check-ignore`: a tracked file is
in the worktree already). Live asks git when the path is added; the
demo checks only the first part.

When (`copy::into`, on the session's start thread, right after
`worktree::ensure`): each worker's start, helpers too (they share
their worker's worktree); never for a leader, which works in the
checkout. A path the worktree has already, as a file, directory or
link, is left as it is: imece never writes over the worker's copy, so
a start after the first copies only what was added to the list since.
A path is measured first (files counted by size, links as themselves
and never followed, at most 100 000 entries), and all paths of one
start together stay within 2 GiB (`COPY_BUDGET`); a path that would
pass what is left is skipped. The copy is `cp -R -P -p`, on macOS with
`-c` (an APFS clone, almost free) and again without it where the
volume can not clone, elsewhere with `--reflink=auto`; a copy that
fails leaves nothing half done. `cp` runs without a shell, with
imece's cleared environment.

The journal says what was copied in one line (`Copied`: the paths,
their size, `words::size`) and each path skipped with why
(`CopySkipped`); a path that was there says nothing, so a usual start
says nothing.

The demo mirrors it: `:copy` keeps the list, and a demo worker's start
copies the listed paths it lacks with made-up sizes (small for a
dotfile, large for a directory), now and then skipping one as gone from
the checkout.

Known gaps: a start whose session then fails to start has copied, but
the journal does not say so (the next start finds the paths there and
says nothing); the budget and the entry cap are constants; a file the
person changes in the checkout later is not copied again (the worktree
has it); copying is per worker, not shared.
