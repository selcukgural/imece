# The timeline: one lane per agent over the last hour

Part C of #257 (2026-11-03), after records 97-99. The salon says what
each agent does now and the story what happened by card; neither shows
time: what worked side by side, who sat idle, where something got stuck
and for how long. The person said they usually run 2-4 workers, where a
lane per agent reads at a glance, so C was built.

The lane: `Agent.lane: model::Lane`, memory only, a `LaneCell` per tick
for the last `LANE_TICKS` (720, an hour), newest at `at`. `Lane::mark`
(tick, cell): the ticks between the last seen and this one take the last
cell (live's clock is the wall's and can jump), a turn's end
(`LaneCell::Done`) only its own tick (`after`); within one tick the most
is kept, by `LaneCell`'s order (gap < idle < starting < thinking <
reading < writing < asking < done < waiting < stuck), so a short edit in
a tick of reading shows as writing; a tick older than the newest counts
as the newest; past an hour the oldest goes. `Lane::at(tick, n)` gives n
ticks up to `tick`, gaps before it began and after it was last seen.

What a cell is (`app::cell_of`, from the status, not the state ladder):
blocked or stopped → stuck; `WaitingHuman`, or working with an approval
open → waiting; working: starting, writing (`app::writes`: the first word
`Edit`, `Write`, `MultiEdit`, `NotebookEdit` or Codex's `apply_patch`, as
live names a tool call), reading (any other tool call; researching too),
thinking (no tool in hand); asking; idle or asleep: waiting when an open
inbox item is the agent's, else done when its lane's last cell was busy
(`LaneCell::busy`), else idle. The first build took the state ladder
(record 61), whose waiting rung is above working: a worker busy on its
next card while its last change waited showed `⚑` for minutes. The lane
says what the agent does; the salon's `!` still says what waits.
`App::watch_lanes` marks every agent not gone at every step (`App::step`,
the demo's and live's), after `watch_turns`.

The view (until record 125's step 5): `View::Timeline`, `4`, `:timeline` (allowed in an archive's
view), key-file place `[timeline]` (`keys::Scope::Timeline`), the
footer's fourth view. `ui/timeline.rs`: per open project in the salon's
order (not closed, not settled) its name and an axis (`axis`: the first
column's clock, the middle one's, `now`; only `now` under 24 columns),
then its leader, its workers and their helpers indented (`agents`; gone
and settled left out), each `rung_glyph`, name, lane, status
(`status_spans`); then `messages`, the project's `Sent` lines per column
on the activity bar's scale (`messages`: the journal read back only
through the lanes' span). `lane_width`: name 20, lane at least 10, status
28; narrower, the status goes. Scale `[` `]` over `SCALES` (1, 6, 12
ticks: 5 s, 30 s, 1 min), a column the most of its ticks (`columns`).
`j k gg G` select (`App.timeline_sel`), reversed and kept in view;
`enter` the agent's focus; the selected agent is the one at hand (`K`,
`t`, `@`). Legend on the last row.

Growth and cost: 720 bytes and a deque per agent, nothing saved; a frame
reads each shown lane's columns and the journal back through at most an
hour. Ten times the agents: the rows scroll; the view says so in its
page. Mirrors: the demo's agents change status and activity as live's
do and `App::step` marks both, so `--demo` shows the timeline the same.

Not done: scrolling back in time (the view always ends at now), lanes
kept between runs, a row for the person.

Since record 125's step 5 (2026-11-07) the timeline is no view: `:timeline`
alone opens it in a window over the screen (`App.window`,
`overlay::view`; `App::window_key` has its `j k g G [ ] enter @`, the
window takes every other key), `esc` or `q` closes it; `4` does nothing,
`K` and `t` are not its.

**A reply is no wait (2026-11-09).** `watch_lanes` counted any open item as the
agent waiting, so an idle agent with an unread reply drew `⚑` all along its
lane; a reply is only read, and its agent is drawn idle.
