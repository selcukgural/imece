# What needs you first, and only that

Backlog item B6 of #55 (2026-10-31, from herdr and Warp): sort the
salon by attention, or show only what needs the person.

Both were taken, small. What an agent needs of the person is one rank
(`App::needs`, `Need`), most first: something of it waits in WAITING
FOR YOU (an open item, or the `WaitingHuman` status), it is stuck
(blocked, or its session stopped), it finished a turn the person has
not looked at (record 35's done, not seen), nothing. An agent that left
needs nothing. Before, workers with a status that needs the person
(blocked, waiting for them, stopped) came first in their project and
the rest kept their order; now the workers sort by that rank, equals
keeping their order, so the screen still does not jump among the calm
ones. Projects keep their order. When the rows do not fit, calm workers
still fold from the bottom up, so those that need the person more fold
last; an idle worker whose answer or change waits may still fold into
"N agents doing fine", as before: the inbox and `W` reach it, and a
crowded salon must fit.

`f` (salon pane, no zoom, no card's screen) shows only the agents that
need something, each under its project's header; a worker comes along
for a helper or researcher of its that needs something, and the lead
only when it needs something itself, with its talk. A project with
nothing is left out, a folded one with something is its header alone,
nothing folds into "doing fine", and with nothing at all the salon says
"Nothing needs you right now · f shows everyone". The title says the
filter is on (`SALON · only what needs you (f)`), and `f` flashes what
it switched to. While it is on, a selection whose row left (the agent
needs nothing any more) moves to the first row of its project still
shown, else to the first row, after every key and every step; not while
the person writes, since the selection decides who `/` writes to.

Choices: `f` for filter (free in every pane; vim's find-char has no
meaning here). The filter is kept for the run only, like the put-off
items of record 33: a fresh start that hid the team would confuse. The
rank is the start of B1's state ladder (error > needs you > working >
starting > done-unseen > idle, with a folded group showing its loudest
member), which is still to come. Pure screen logic, so the demo shows
it exactly as live does.
