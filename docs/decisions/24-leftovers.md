# Leftovers v0

Leftovers v0 (2026-10-30, persistence PR 6: what the earlier entries
left). DAYS: `model::start_clock_at(seconds, day)` takes the local day
of the world's first start as well (`live::local_clock`; the civil date
math `days_from_civil` / `civil_from_days` is pure, in model.rs);
`day(tick)`, `date(day)` = `YYYY-MM-DD`, `stamp(tick)` = both,
`when(tick, today)` = the clock alone on today's lines. The journal
pane and a conversation draw a dim `─ YYYY-MM-DD` row above the first
line of a day other than the line above it (today for the first line:
a journal whose newest line is yesterday starts with its date, one all
of today has none; `first_shown` in salon.rs counts the day rows so the
selection stays in view); a WAITING FOR YOU item of another day carries
its date; copied text is always stamped (it leaves the screen). The
clock follows the start, so a daylight-saving change inside a world's
life shifts it by an hour (accepted). The demo runs at 14:00 on today's
date. RETENTION: `Store::tidy(now, JOURNAL_KEEP)` at every live start
after the load deletes journal lines older than `JOURNAL_DAYS` (90)
that lie past the newest 5000 (`seq <= newest - keep AND tick <
cutoff`, a primary-key range, in the `plans` test), and runs `VACUUM`
when the free pages are a quarter of the file or more (`needs_vacuum`);
stories and inbox rows stay (numbered by position); a failure is
ignored and the next start tries again. PREFS: the `screen` file is
gone; side, theme and lead live in the store's `meta` table under
their own keys, through `store::settings` / `set_settings`: a
connection of their own without the world's lock (the theme is read
before the store is taken, a demo beside a live imece may change it,
SQLite serialises the writers), the file made 0600 before SQLite opens
it, the world's keys (schema_version, started_at, budget) refused; a
`screen` file found at load is moved in and removed. Known gaps: no
retention for stories of long-closed projects or closed inbox items; a
tidy's result is shown nowhere; the clock ignores daylight-saving
changes (above).

Record 66 adds a second retention: sent lines past the newest 500 go at
the tidy.
