# Usage limits beside the pot

Vendor rate-limit windows in the header (2026-10-31, backlog G1 in #60,
from Orca and openharness): a team on Claude or ChatGPT subscriptions
can be stopped by the plan's five-hour or weekly limit long before the
pot runs dry, and nothing on the screen said how near that was.

What the vendors send (read offline, no session run): Claude's
stream-json `rate_limit_event` carries `rate_limit_info` (from the
Claude Code 2.x binary's own schema): `status`
(`allowed`/`allowed_warning`/`rejected`), `resetsAt` (seconds),
`rateLimitType` (`five_hour`, `seven_day`, `seven_day_opus`,
`seven_day_sonnet`, `seven_day_overage_included`, `overage`),
`utilization` (a fraction) and `unifiedWindows` with `five_hour`,
`seven_day` and `seven_day_overage_included`, each `{utilization,
resetsAt}`. Codex's app server (`codex app-server generate-ts`, 0.165)
sends `account/rateLimits/updated` with `rateLimits.primary` and
`.secondary`, each `{usedPercent, windowDurationMins, resetsAt}`, sparse:
a window not named keeps its value. imece had it among the
notifications it turned off (`QUIET`); it is asked for now.

`imece_agent::Event::Limits(Vec<Limit>)`, a `Limit` being the window's
length in minutes, the percent used (whole, at most 100) and when it
starts again (seconds since the epoch, if said). Claude: each of
`unifiedWindows` that has a fixed length (five hours, a week; one
model's week and overage are left out), else the one `rateLimitType`
names; a window without `utilization` says nothing. Codex: `primary`
and `secondary` with a length and a percent. Gemini reports none.

The windows are the account's, not the agent's: `World.limits` keeps
one per vendor and window (`World::limit` replaces a window said again,
which is what a sparse update needs), and `World::tightest` gives each
vendor's window nearest to full (the most used; the shorter one on a
tie). The header puts them after the pot, one per vendor:
`claude 5h: 48% left, resets in 1 h 59 min` (`words::limit`, both
languages; the time only while the reset is ahead), in the attention
colour from 90% used (`LIMIT_LOUD`). The reset is measured against the
world's clock (`World::now`: its first start and the ticks since).
They are not kept between runs: a session says them again on its
first turn.

The demo mirrors it for the vendors whose sessions run: every minute of
its clock, what a vendor's agents spent fills its five-hour window (a
percent for each $0.02) and a quarter as much of its week, each
starting again at the next whole window of the clock.

Not done: nothing stops or warns before a turn when a window is full
(the vendor refuses the turn itself, and imece shows that refusal);
Claude's `status`, `seven_day_opus`/`sonnet` and overage are read by
nobody; the formats were read from the vendors' own code, not from a
live session yet.
