# The fleet and "done, not seen"

Two backlog items of #55 (2026-10-31): B2 "done, not yet
seen" (from herdr) and B3 the always-on fleet count (from openharness).

Done, not seen: an agent that was mid-turn at one step (`World::mid_turn`,
record 34) and is idle at the next finished a turn; `App::watch_turns`
puts it in `App::unseen`. It leaves the set when the person opens it
(zoomed in on, focus, raw stream or its change's diff, or a party of the
open conversation: `App::opened`, checked after every key and every
step, so a turn that ends while the person looks is seen at once) and
when it is mid-turn again (the earlier turn is no news any more). An
agent that ends asking a teammate, blocked or waiting for the person did
not finish. `App::done_unseen` also needs nothing of the agent waiting
in WAITING FOR YOU: an answer, a question or a change says more, in its
own row and the inbox. The salon row then shows `✓` (a leader keeps `◆`)
and "done, not seen yet" in place of its status. Kept for the run only,
like the put-off items of record 33.

The fleet replaces the header's "N agents working": `● 2 working  ✓ 1
done  ✕ 1 stuck`, counts of none left out, "all idle" with none at all.
Working is `mid_turn`; done is `done_unseen`; stuck is blocked or stopped
(the session ended without an answer). What waits for the person is not
in the fleet: the header's `! N waiting for you` and the title count it.
Gone agents are not counted. The glyphs are the salon's, so they differ
by shape and read in the mono theme; the help's legend has `✓` and `■`.

Demo: the same model and screen, so `--demo` shows both; its workers
mostly end a card with an item for the person, so the first plain
"done, not seen" comes after a few hundred steps (a test steps the demo
to it). B1, the full state ladder with a folded group showing its loudest
member, is record 61; the `✓` of a done, not seen row is its rung's
glyph there (`rung_glyph`).
