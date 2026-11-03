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

## `.worktreeinclude` (#244, 2026-11-03)

From the study of agent-deck; the person asked for it. agent-deck and
Claude Code's desktop app read a `.worktreeinclude` file at the top of
the repository, in gitignore's syntax, for the same thing. imece reads it
too (`copy::bring`, on the session's start thread in place of
`copy::into`): what it names is copied after the list's paths, under the
same rules and the same budget, the list's own coming first and a path
in both counting as the list's (`Brought.listed`). It needs no yes: the
file only names paths of the person's own checkout to copy into imece's
worktrees, so it can not reach anything the person's `:copy` could not.
It is not added to the stored list: the file is read at every start, so
an edit to it counts at the next one and a removed line stops copying.

How a pattern is matched: by git, not by imece (no glob crate, and
gitignore's rules are git's). `copy::named` runs `git ls-files -z
--others --ignored --directory --exclude=P…` (patterns given with
`--exclude=`, so negation works among them and none is read from a file
git would follow), then keeps only what git ignores (`git check-ignore
-z --stdin`, one call): a path the file matches that git does not ignore
is untracked work of the person's, not copied and not said. Probe
(git 2.56): with `--directory`, a directory the patterns match whole is
listed alone, but an untracked directory that only *holds* a match is
listed too, beside the match (`.env` lists `build/sub/` and
`build/sub/.env`); so `copy::whole` drops a directory with a path under it
listed (sorted, the next one starts with it), and the rest lose their
`/`. `--no-empty-directory` is not used: it drops the match instead.

What is refused and said (`Event::IncludeRefused { line, why }`, kind
Failed, at each start since the file is read at each): the file when it
is a link (`O_NOFOLLOW`), not a plain file, over `INCLUDE_MAX` 16 KiB or
not UTF-8; a line over 200 bytes, with a control character, a `..` part
or a `.git` part (git would match nothing outside, but such a line says
it wants to; the rest of the file is followed); the whole file when it
matches more than `INCLUDE_PATHS` 100 paths git ignores, when git's
answer reaches `child::ANSWER_MAX` (64 KiB, checked before the exit
status: git ends on the closed pipe), or when git does not answer in
10 s. Comment and blank lines are skipped; `\r\n` ends are fine.

The journal says the file's copies apart: `Event::Copied` and
`CopySkipped` carry `included` (`#[serde(default)]`, so older rows read
as the list's), worded `…, as .worktreeinclude names` and `… (from
.worktreeinclude) …`. `:copy` alone ends with `; and what its
.worktreeinclude names` when the checkout has one (`copy::has_include`,
`Live::includes`); the menu note is the list alone.

The demo: the second project's checkout acts as if it had one
(`Sim::includes`), naming `node_modules` (copied with a size made up from
the worker's number, taking nothing of the demo's chance so the second
project plays as before), with line 2 `../shared/.env` refused at every
start.

Known gaps: a start whose session then fails to start has copied, but
the journal does not say so (the next start finds the paths there and
says nothing); the budget and the entry cap are constants; a file the
person changes in the checkout later is not copied again (the worktree
has it); copying is per worker, not shared.
