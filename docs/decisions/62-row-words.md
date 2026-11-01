# What each row says it does now, and for how long

Backlog item B4 of #55 (2026-11-01, from openharness and cmux; issue
#138): each row says what the agent does now (its question, its last
progress, what it said at the end of its last turn, why it failed) and how
long it has been in that state; a fixed row height and a stable order.

One line per agent. Before, a row that talked grew a second, dim line
`⇄ claude-2 "which parser?"` for eight to twelve ticks and lost it again,
so the tree jumped under the eye; the order was already stable (record
38). Now `Row::Talk` is gone and the exchange lives on the agent's own
row, where a row is one line whatever it says.

The status carries its own text where it has one (`ui::status_spans`): the
question waiting for the person (`? → you "which index?"`), the answer
they have not read (`answered you "use the enum"`), what a tool call
waits to do (`⚑ waits for your yes "cargo test"`, the lines joined with
`↵`), what a blocked worker waits for (`BLOCKED "JWT clock skew" ←
researcher-1`, the topic in the room the wait leaves), and why a stopped
session stopped (`■ stopped: the process exited with code 1`). The reason
is the first line of what the session said, kept in `Agent.topic` (what a
researcher researches, what a blocked worker waits for: the same field,
read only for those statuses; not saved between runs, so a stopped agent
remembered from an earlier run shows the plain hint). A failed turn sets
nothing: the agent is idle after it, and the journal has the why.

Otherwise the status ends with the agent's newest exchange
(`salon::quote_spans`): `Talk` got `mine`, so the arrow says who said it,
`→ lead "parser fixed"` or `← you "fix the parser"`; a message on the bus
sets both rows, the sender's as its own; an `Asking` row's own question
to the one it waits for is quoted alone (`? → claude-2 "which parser?"`),
the status having named them. The talk is no longer cleared when `until`
passes (`Live::step`, `Sim::step`): it stays until the next exchange
replaces it, so a row says what the agent last did; `until` now only says
how long the row counts as news, which is what keeps a worker from
folding (`salon::rows`' `calm`). A status that says a text of its own
(`status_says`: an item's text, a tool in hand, a topic, a why) quotes
nothing more, nor does one whose words fill the column (`■ stopped, write
to it to start again`, `◦ asleep …`).

How long: `App::since` is the tick the state began, the oldest open item
of the agent's in WAITING FOR YOU when there is one (how long it has
waited for the person), else its last status change; the row says
` · 3 min` (`words::ago`) from a minute on, nothing under it, so rows do
not tick. A starting session, a done not seen and a working agent all
count from their status change.

Width: the status with its quote keeps to `STATUS_WIDTH` (40 columns) so
the columns after it stay put, a quote under `QUOTE_MIN` (8 with its
quotes) is left out rather than shown as `"…"`, and `agent_line` now takes
the row's width: the time and then the quote take only what the row has
left once its cost, stand, helpers and link are on it, so what was on the
row before this change never moves off it; a narrow pane drops the quote
first, the time next. The card's screen passes its width less the part
column.

Mirrors: the demo sets `mine` on its sends, answers and turn ends, and
`STOPPED` as the why of a worker it stops, so the demo's rows say what
live's do. Done since: the to-do list (B7) is record 63 (`≡ 3/7` on the row, which
gives way before the time) and the context-window fill (B8) record 64
(`◔ 72%`, which stays longer than the mark). Not done: the status
column's forty columns are a constant, not a share of the pane.
