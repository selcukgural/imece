# Turn to turn in a conversation

`[ ]` in the conversation window (2026-10-31, backlog C6 in #56, issue
#91, from cmux and Warp, which jump from prompt to prompt): a
conversation where one side writes many lines in a row (a worker's
`progress` lines, a leader's several assigns) took one `j` per line to
get past.

A turn is the lines one party says before the other answers:
`ui::turn_starts` gives the positions in the conversation where the
speaker (`Said::from`) is not the one of the line before, and the first
line. A line that is no conversation (a window opened on another event)
has no speaker, so it is one turn. `]` selects the next turn's first
line, `[` the start of the turn the selection is in, or of the one
before when it is at a start already; neither moves past the first or
last turn. The drawing scrolls to the line, as `j k` do.

The keys are the ones the diff view uses to go file to file, and the
two share `app::jump`. In the salon `[ ]` cycle the panels; the
conversation window keeps them while it is open, like the diff view.
Movement, so no `:` name (record 41). The window's hint row and the
help's `[  ]` line name both uses, in both languages. Pure screen
logic: the demo's conversations show it as live's do.

Not done (C6's other half, now C6b in #56): bookmarks. A mark the
person sets would want to outlive the run, so it is a store column
and a decision of its own.
