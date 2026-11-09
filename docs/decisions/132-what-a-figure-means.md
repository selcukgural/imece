# What a figure means

G2 of #60 (user, 2026-11-09): a cost ledger per attempt, the cost of each
applied change, what subscriptions would have cost at API prices, each
price with its source and date, and an unknown price never shown as $0.
Asked first; every answer the recommended one: a `:ledger` window (one
row per card attempt, totals per project, applied apart from wasted, the
leader's own spend apart) with the attempts on the card's log tab; `≈` on
every figure of a plan's login; Claude Code's own figure as the source
(its version, the turn's date, the per-model split of `modelUsage`), no
price table of imece's; what was never priced shown as "at least", the
pot's limit working on what is known. This record is the first of two
parts: what a figure means. The ledger itself is the second, record
133.

The source. Every dollar imece shows from an agent is Claude Code's
`total_cost_usd`, which it computes from its own price table, plan or
key. imece keeps no table of its own (record 109: Claude Code alone): one
would go stale, and the person would have two figures that disagree.

`≈` on a plan. `claude auth status --json` (asked at every start, record
75) says `authMethod: "claude.ai"` for a plan's login and something else
for an API key, which Claude Code prefers when it is in the environment
even with a plan's login stored (`subscriptionType` is then still there:
not read). `imece_agent::Login::Yes { subscription }`; `App::take_plan`
sets `World.estimated` (memory only) from it at each look, the demo's at
its start; not signed in or not said leaves it as it was. `World::money`
(`model::money`) words a figure: `≈` first while estimated. What is
estimated: every agent's total, a change's spend, a project's, the pot's
spent and its rate, what is left of it, `:commit`'s and `:review`'s cost
in the journal, the farewell. Not: the budget the person set, a plan's
figure (the leader's own guess, record 114). The welcome window's line
says `signed in, a plan · ≈ API prices`.

Never $0 for what is not known. Three things leave an agent's spend
unpriced: a result without `total_cost_usd`; a result whose `modelUsage`
has a model that used tokens (input, output, cache read or creation) at
`costUSD` 0, one Claude Code has no price for (a model with no `costUSD`
at all is not taken as one: an older Claude Code may say only the
window); and a session that ended mid-turn, whose cut turn never reports.
The parser (`claude::unpriced`) sends `Event::Unpriced(Option<model>)`
before the `TurnDone`, which carries the last total known: a missing
figure is no longer read as $0, which also counted as a reset and booked
the session's next total twice (a bug until now). A `/clear` after an
unpriced turn keeps every later turn of that process unpriced
(`cleared_unpriced`: no later total covers it). On the screen
`Agent.unpriced: Unpriced { said, session, gaps }`:
`World::turn_unpriced` sets `said` (and traces `cost unknown: …`),
`World::turn_priced` at the `TurnDone` moves it to `session`, before the
change is settled, so the next whole figure, the session's whole, covers
an earlier unpriced turn of the same session; `World::session_over` from
`Live::end` makes a gap for good of a session that ended unpriced, or
mid-turn in a running process (`turns > 0` and `Slot::Running`; a
process still starting spent nothing; the turn at the cap before
`renew` and the failed turn before `fall_back` said their cost, so those
two count it out first; a resume refused before `init` ran nothing). A
turn cut by a quit is counted at the next load, from the cut prompts the
store keeps (`interrupted`), so a quit again before the agent wakes
counts it again: the count is only ever compared, never shown.
`Unpriced::any` (a session gap or `gaps > 0`) makes the agent's figure
`$2.29+?`, `?` alone when it is $0; a round of a change is unpriced when
its agent's session is, or its agent or a helper with a round open has
more gaps than when the round opened (`Task.gaps`); `Spend.unpriced`;
`World::unpriced` over every agent makes the pot's spent `+?`;
`World::project_spent` its project's. The run's farewell is unpriced
when the gaps grew this run (`App.unpriced_at_start`,
`World::unpriced_count`) or an agent is mid-turn as imece quits
(`App::spent_this_run`). `:commit`'s and `:review`'s one-shots
(`claude::once`) have `Said.cost: None` when unpriced, and keep what they
reserved, as one that stopped without a figure does.

The pot. It goes on counting what is known: a limit cannot count what it
does not know, and a session's own `--max-budget-usd` is Claude Code's
and counts what Claude Code counts. The person reads `+?` beside it.

Growth: three small fields per agent, memory only but `gaps`, which the
store keeps as `agents.unpriced` (schema 33; a session unpriced at a save
counted as one more gap, `Unpriced::count`, since a resumed session's
figure may or may not cover it); `Task.gaps` and `Spend.unpriced` in
their JSON columns (left out when zero or false, so an older row reads
as priced). Read cost: `World::unpriced` and `project_spent` go over the
agents, as `spent` does, once a frame. Migration: an older file gets the
column at 0: nothing was known unpriced then (a missing figure was $0
and is not told apart now). Failure: an `Event::Unpriced` without its
`TurnDone` (the process gone between them) leaves `said`, which the
session's end counts as a gap.

Demo (record 31): the demo signs in through a plan (`sim::seen`, taken by
`take_plan` in `main`), so its figures read `≈`; every `UNPRICED_EVERY`
(5th) change it reads ends a turn that said no figure, through the same
`turn_unpriced`/`turn_priced`, until that worker's next. `--demo-live`'s
stand-in always says its figure and does not look (no `≈`).

Not done here: the ledger and the `:ledger` window (part two); the
per-model split and Claude Code's version beside each figure, which go
with the ledger's rows; whether a resumed session's total covers a turn
cut in its earlier process is not probed, so such a turn stays a gap
(pessimistic, never low).
