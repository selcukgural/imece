# What each model spent

G2 of #60, part three (user, 2026-11-09; the questions and answers are
in record 132, the ledger is record 133): a ledger row says what each
model spent of it, from Claude Code's own figures, so the person sees
what the worker's model, its helpers' cheaper tier and Claude Code's
background model each came to.

Claude Code's result says, per model the session ran on, its
`modelUsage[*].costUSD`: the session's figure for that model, which
starts again with a `/clear` and which a resumed or forked session
reports whole, as `total_cost_usd` does. So the counting is the
adapter's, beside the total's (`claude::Parser`): `models_last` holds
each model's figure as the last result said it; a result's figures
(`by_model`: names with a `costUSD`, in millionths, a negative one
nothing) give what each grew since; after a reset (the total went
down) every model's figure is new spending, and so is one model's that
went down with the total not. `Event::Models { said, grew }` comes
before the `TurnDone` (as `Unpriced` does), `grew` without the models
that did not grow; a result with no total or no model's cost says
nothing by model and moves nothing. A resume counts each model on from
the figures the old session last said: `Resume.models`, kept in
`Resumable.models` (`Started` sets it from the resumed attempt, or
empty for a new session or `/clear`'s; `Models` sets it to `said`). A
resume of a session whose figures by model were not kept (a save
before this) takes its first result's figures as the floor and says
none of that turn grew (`models_unknown`): that turn is not split
rather than counted twice.

Booking (record 133's `World::book`): `World::models_grew` adds what
grew to the agent's `Meter.models` (millionths, memory only), and the
booking after the `TurnDone` moves each model's whole ten-thousandths
onto the row its worker spends on (`Attempt.models`), keeping the rest
for the next; a helper's go on its worker's row under their own names,
a leader's or a researcher's are dropped with their figure. What of a
row's cost no model's figure said is `Attempt::unsplit` (cost less the
models' sum, never below nothing); `World::models_of(p)` sums a
project's rows and their unsplit parts.

What the person sees: in `:ledger`, a dim line under each project's
totals and under the row at hand (`ui::ledger::by_model`): `by model:
claude-opus-5-5 $0.70 · claude-haiku-5-5 $0.12 · not split $0.02`, the
most first, `not split` from a cent on, figures as `World::money` says
them (`≈` on a plan); none when no model's figure was said. The card's
screen has the line under each of its tries. Turkish `modele göre`,
`ayrılmamış`.

Growth: a few names per row (the worker's model, a helper's, the
background's), in the row's `models` column as JSON; a session's in
`agents.session_models`. Read cost: the window sums a project's rows
once a frame while open, as it does for the totals. Migration: schema
35 adds `ledger.models` (`'{}'`: an older row is all not split) and
`agents.session_models` (NULL: not known, see above); a column that
holds no figures fails the load, as other columns do. Failure: the
figures are saved with the rows and the session they came from; a
crash before a save loses them together with the figure they split.

Demo (record 31): each busy tick splits its cost, an eighth to
`claude-haiku-5-5` (`sim::DEMO_BACKGROUND`), the rest to the session's
model, through the same `models_grew` and `book`; the stand-in
(record 130) says its spending the same way in its results'
`modelUsage` (`stand_in::BACKGROUND`, its session's whole), so
`--demo-live` books it through the real adapter.

Not done: the leader's spending by model (it is no row's; the window
says the leader's total only); a model's figure per turn in the trace.
