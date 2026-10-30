# Where you left off v0

Where you left off v0 (2026-10-30, the plan's last step; the fork
command was dropped for now, user's decision: no clear use yet): a live
start that remembers an earlier run opens one window before anything
moves (`ui/overlay.rs::left_off`, `app::LeftOff::of`, pure and tested):
the pot (`spent of budget`) and that every agent sleeps, one line per
open project (agents still there, cards not done), the oldest open
inbox items as the inbox lists them (`ui::inbox_line`, shared with the
salon; at most `LEFT_SHOWN` 6, then `… and N more`), the turns the
earlier run cut short (the `Interrupted` lines this start journaled),
and the earlier runs' last 6 journal lines, oldest first with a day
row where the day changes (as a conversation does). The title says how
long ago (`words::ago`: minutes, then `h min`, then `d h`) from the
store's `saved_at` (`meta`; written with every save that writes
anything and by `Store::leave` at an orderly quit, `App::leave` after
the last persist in main; an older file has none and the title says
nothing). Any key closes it and does nothing else; the help wins over
it. Nothing to tell (no lines from before, nothing waiting) → no
window; the demo never has one. `Live::left_off()` carries the journal
count before `adopt` and the seconds since. `saved_at` is a world key
(refused as a setting).
