# Peek and settle

Backlog item C16 of #56 (from herdr), issue #160. Two things the person
had to leave the view for: looking at one worker up close (`enter` takes
the whole body, so the salon, board or card behind it is gone until
`esc`), and a finished worker or project that keeps its row although
nothing of it needs them. The person chose the keys (`K` and `s`) and
that settling lasts the run only (2026-11-01).

**Peek.** `K` (`:peek [@NAME]`) sets `App::peek` to the agent at hand
(`agent_at_hand`: zoomed in on, selected in the salon, behind the inbox
item or the journal line); `ui::overlay::peek` draws `focus::draw` for
it in a window eight columns and six rows smaller than the screen, the
rest shaded (record 29), after the conversation window and before the
help. Nothing else changes: the view, the zoom, the pane and the
selection stay, so `esc` leaves the person where they were. While it is
open `App::peek_key` takes every key (after the help, the digest and
the writing, before a chord): `j k` and the arrows step through the
agents of `ui::salon::rows` at full height (`peek_step`; the first
agent when the one peeked at has no row, itself at either end), `enter`
opens its whole focus (the zoom, over the same view), `@` starts a
message to it, `?` the help (which closes back to the peek), `esc` or
`K` close; anything else does nothing, so a stray `q` or `2` does not
act behind the window. `App::opened` counts the agent peeked at, so a
peek sees a finished turn as the focus does. `K` works with a zoom
open too: in a change, a peek at its worker's focus (its plan, its
context) is worth having; the zoom stays under it. The footer shows the
peek's keys. Nothing of it is saved.

**Settle.** `App::settled` (agents) and `App::settled_projects`. `s` in
the salon (`:settle`, which goes to the salon first) acts on the
selection: a worker's, helper's or researcher's row settles that agent;
a project's header or its leader's row settles the project (a leader is
the face of its project, never settled alone). `App::row_group` is
what a row stands for: the agent, its helpers and the researcher
working for it (`Status::Researching { for_agent }`), which hang under
it and go with it; `project_group` is every agent of a project.
Settling is refused while any of the group needs more than `Need::Done`
(waiting or stuck), naming that agent: what needs the person is never
put out of sight. A done-not-seen turn is fine; settling removes the
group from `unseen`, as looking at it would. The selection goes to the
nearest row after it still shown, else before it (`split_at`, no index
arithmetic), and stays when nothing is left.

`ui::salon::rows` leaves out settled agents (through `kept`, so a
settled helper or researcher goes alone; and a settled worker is left
out even when its helpers are shown, so they go with it) and settled
projects (with their header), whatever `f` says. The pane title adds
`· N settled (:unsettle)`. The fleet in the header, `&`'s list, the
folded project line's loudest member, `W` and the notices still count
every agent: settling hides rows, not state.

**Coming back.** `App::wake_settled`, in every step right after
`watch_turns`, keeps a settled agent only while none of its row's group
needs anything (`Need::Nothing`) and it is not gone; a project only
while none of its agents needs anything and it is open. So a question,
a change, a stuck session or a new finished turn brings the row back by
itself; working does not (a settled worker given new work comes back
when it finishes). `go_agent` (`&`, `:goto`) drops the agent's project
and every settled row whose group holds it (a helper brings back the
worker it hangs under); `:goto PROJECT` drops the project. `:unsettle`
drops everything and says how many. Run only, like `f` and `S`: a new
run shows everything, and the store has no table for it.

**Demo.** Both are screen state over `World`, the same code for the
demo and live; the demo's workers finish turns, ask and get stuck, so
settling and coming back show there as they do live.

**Help.** `K` and `s` joined existing rows (`enter  K`, `f  s`), so the
help keeps its 55 rows and the forty-row window.

**Not done.** A settled set kept between runs; `:settle NAME`; settling
from the board or a card's screen (only the salon's selection).
