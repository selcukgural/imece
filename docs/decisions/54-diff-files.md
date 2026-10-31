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

Not done (now D3b in #57): folding every file to its header and
unfolding them (`zM zR` in the salon), which needs the scroll to count
what is shown, not the patch.
