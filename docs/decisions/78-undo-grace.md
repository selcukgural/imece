# A grace before :close and :forget

Backlog C23 in #56 (from Warp; issue #181, 2026-11-02, made while the
person was away: to go over together). `:close` ended a project's
sessions and dropped its mail, offers and changes at once, and `:forget`
killed a session and removed what the vendor kept of it; a slip of the
menu (`:c` and `:f` are short names) could not be taken back.

**A wait, not an undo after the fact.** What the two do cannot be put
back: a killed run, a vendor's transcript removed. So neither acts when
asked: `App::grace` holds what was asked (`Undoable::Close(p)` or
`Undoable::Forget(id)`) and when it acts, `UNDO_GRACE` (10 s) on. The
main loop calls `App::grace_due` every turn; when the time is up the old
code runs (`close_now`, `forget_now`, through the engine as before) and
the store is written. Until then nothing changes: the agents go on, the
project is open.

**What the person sees.** The flash is cleared and the bottom line counts
down in the warning colour (`◷ closing kule in 7 s · u keeps it open`,
`◷ claude-2 forgets its session in 7 s · u keeps it`, seconds rounded up,
`App::grace_left`); a flash said meanwhile has the line for its three
seconds, then the count is back. While writing, the bottom line is the
writing, as for a flash. `◷` and not `⌛`: an hourglass is drawn two
cells wide in most terminals.

**`u` and `:undo` (user's rule: every key has a `:` name).** `u`, free
until now, drops what waits and says it (`kule stays open`, `claude-2
keeps its session`), else `nothing to undo: …`. It is listed on the `esc`
row of the help (`esc  u`), which keeps the help at 55 rows, and it is a
used key for the key file (`keys::USED`).

**One at a time.** A second `:close` or `:forget` makes the one waiting
act first (`App::end_grace`), then is checked and waits itself: two
closes in a row close both, the second after its own ten seconds. Being
checked after means a `:close` of the only other open project, with one
close waiting, is refused as the last one.

**Refusals at once.** What would be refused is said when asked, not ten
seconds later: the last open project (fewer than two open), a name nobody
has, an agent with no session (`Live::can_forget`, `Sim::can_forget`,
the checks the old functions began with, which they still call). A
refusal waits for nothing.

**Quitting.** Quitting with one waiting lets it act (`main` calls
`App::end_grace` before the last `persist`): the person asked for it and
did not take it back (a SIGTERM or SIGHUP quits the same way). A crash or a
SIGKILL loses it, so the project stays
open and the session kept, the safe side.

**The demo** waits the same way: the grace lives in `App`, in front of
both engines.

**Not done.** No grace for other actions that drop something (`r` on a
change, `:copy` off); no setting for the length.
