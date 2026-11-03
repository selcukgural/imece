# A row says the step, and what needs you has a header

Part A of #257 (2026-11-03). The person said they could not tell what
the agents were doing. A 160×48 demo screen showed why: 23 of 25 rows
wore the same `●`, a working row's status was its last tool call (`Read
src/Auth/TokenService.cs`, `Grep "…"`), which says how the agent goes
about something but not what it is after, while the agent's own to-do
list (record 63) sat behind `≡ 3/5`; and the workers that need the person
came first (record 38) with nothing to show where they ended. Five ideas
were drawn and talked over (A this record, E a pulse line, B a story
panel with D the wait chains, C a timeline); the person took the order
A, E, B, C.

The step. `Agent::step` is the first step of the to-do list in
progress, none when no step is or its text is blank. `ui::on_step` is
the step a working row says: the agent is `Working`, its session is not
starting and no tool call of it waits for a yes (those say so instead,
as before). Such a row's status is `▸ Run the tests` (`ui::step_span`),
and in the quote's place, dim, what it does for it now: its tool call or
`thinking…` (`ui::doing_spans`). What the agent is after comes first,
how it goes about it second. The step takes at most `STEP_WIDTH` (24)
columns and never more than the row's fixed parts (cost, stand, helpers,
link) leave it, but at least `STEP_MIN` (8): the first build gave it a
fixed width and the tail of a narrow row went off its edge. The tool
call takes what `STATUS_WIDTH` and the row leave after the step, nothing
under `DOING_MIN` (8). A step row quotes no exchange (`status_says`).
The focus's status line has both, the tool call in up to the 38 columns
the status leaves. Without a step (Gemini's `write_todos` carries none;
a list all done; no list) the row is as before. `▸` was already the
focus's mark for the step in progress; it is in the `?` legend now.

The headers. In an open project that shows more than one worker, when
some of them need the person, each need's workers sit under a
`Row::Group` header: `✕ stuck · 1`, `! waits for you · 2`, `✓ done, not
seen · 1`, then the rest under one that counts them, `● 8 working  ○ 1
idle` (idle includes asleep; a count of none left out). The workers were
already sorted by `Need`, so each need is one run (`chunk_by`) and
nothing moves that did not move before; the calm ones still keep their
order. A header is not selectable (`Row::sel` is none), not a hint
target, and closes its branch: the last worker under each header draws
`└`. The leader stays above them. The headers give way first: they are
shown only when every row fits with them; otherwise none is and the
salon folds calm workers exactly as before, so a crowded screen keeps
its rows and no header ever stands over an empty group. `f` shows them
by the same rule.

A fix on the way: the first key of a chord (`g…`, `z…`) was drawn at the
end of the footer, past the 140th column in the demo, so it was cut off;
a test passed only because a row's `thinking…` contains `g…`. It is the
footer's first part now.

Read cost: per frame, the headers add one pass over each open project's
sorted workers (already built for the order) and one count per header;
nothing that grows is scanned. Ten times the agents: the headers are
the first thing dropped, so they cost nothing on a crowded screen.
Mirrors: the step comes from the to-do list the demo already fills
(record 63) and the headers are screen logic, so the demo shows both as
live does.

Not done: a second, dim line under the selected row for its tool call
(drawn in the discussion) was left out, since rows keep one height
(record 62). The rest of #257 is E, B (with D) and C.
