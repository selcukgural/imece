# Bookmarks

`m` and `'` (2026-10-31, backlog C6b in #56, from cmux and Warp, which
let the person mark a place in the output and come back to it): a line
the person wants again (a decision, a question answered, the message
with the plan) got lost in a journal that grows by hundreds of lines a
day.

`m` (`:mark`, it acts) marks the line at hand: the selected line of the
open conversation, else the selected row of the journal (the card's, on
a card's screen); `m` again takes the mark away, and a flash says which
happened. With no line at hand (another panel, a focus, the board) the
flash says where `m` works. `'` (as vim's go-to-mark; movement, so no
`:` name, record 41) selects the next marked line after the one at hand,
round to the first: among the open conversation's own lines when one is
open, else in the journal's rows in the order they are listed (newest
first), and from anywhere else it goes to the salon's journal first.
None there: the selection stays on a row and the flash says so. The
diff view keeps its own keys; `'` from it leaves for the journal.

A mark is the person's, so it outlives the run and is kept where the
line is: `World.marks` (journal indexes) on screen, `journal.marked` in
the store (schema 8; a file of schema 7 gets the column, nothing
marked). A mark alone is a change to save: the line is written again
by its `seq`, which `store::Seqs` knows for every line in the world
(the lines a load brought may have gaps, the ones this run wrote follow
the file's last). The load takes the newest lines as before and, older
than those, the marked ones, through a partial index
(`journal_marked ON journal (seq) WHERE marked = 1`: it holds only the
few marked lines, so the many others cost nothing to it; the `plans`
test checks the read uses it). The tidy never lets a marked line go,
whatever its age: a mark keeps its line within reach.

On screen a marked line wears `✱` (neutral width, a shape the mono
theme keeps): in the journal in the space after its clock, so marked
and unmarked rows stay aligned; in a conversation in the second column
of the first row's gutter, beside the selection's `▌`. The journal
never folds a marked line under a newer line of its conversation: it
keeps a row of its own for `'` to land on (it still folds the older
ones under it). The help lists `m :mark` and `'`, its legend `✱`, the
journal's key row `m`, in both languages. The conversation window's
hint row does not: at 120 columns it no longer fit and `esc` fell off.

The demo marks and goes to marks as live does; it has no store, so its
marks end with the run.

Not done: `:export` does not show the marks; there is no list of all
marks across projects (the journal with `'` stands for it).
