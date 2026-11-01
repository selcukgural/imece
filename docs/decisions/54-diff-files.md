# The files of a change, and its long lines

Backlog D3 in #57 (2026-10-31, from Orca and Warp), its first half: a
change's diff showed the number of files in its title and the files
only as they came in the patch, and a long line (a minified file, a
long string) was cut at the screen's edge with no way to read the rest
but the worktree.

A change of two files or more lists them between the runs on its tree
and its patch (`ui::diff::file_lines`): each on a line with its lines
added and removed (`+12 −3`) or "binary", cleaned, six at most and the
rest counted ("… 2 more files"); a change of one file shows none, its
title says it. `[ ]` still jump from file to file in the patch.

`w` in a change wraps its long lines: a line goes on in the rows under
it (the same wrapping a conversation uses, by cell width), the line at
hand marked on all of its rows; `w` again cuts them at the edge as
before (`App::diff_wrap`, for every change until turned off; not kept
between runs). The scroll still counts patch lines, so `j k` move a
whole line however many rows it takes. In the view, `w` is the diff's,
not the salon's "waiting" key. The footer's `j k  w  scroll · wrap`
and a help line name it, in both languages.

**Files folded (D3b, 2026-11-02, made while the person was away).** `h`
or `←` folds the file of the line at hand to its header, `l` or `→`
opens it, `za` turns it, `zM` / `:fold` folds every file and `zR` /
`:unfold` opens them all: the keys of the salon and of a conversation.
`App::diff_folds` holds the folds (`Folds`, by the file's number in the
patch) with the worker and the patch's hash, so a new patch starts open,
as the notes and the chosen hunks do. `ui::diff::Shown::of` gives the
lines shown (a folded file: its `diff --git` line only, which says
`▸ N lines`; lines before the first header always) and `Shown::step`
moves over them: the scroll stays a patch line number, always one shown,
so `c`, `>`, `x`, the notes and `[ ]` work on it as before. `j k`, the
pages, `G` and `gg` step over the lines shown; folding the file of a
hidden line puts the line at hand on its header (`App::fold_files`). `v`
opens every file first, since lines are chosen from the whole patch. A
conversation over a change keeps the fold keys for itself
(`App::diff_zoom`). The help's `← →  h l` row names files too; the
demo's changes fold the same way.
