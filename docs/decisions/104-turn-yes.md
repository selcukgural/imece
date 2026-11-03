# A yes for one agent until its turn ends

The person, 2026-11-04, answering record 103's open question: asks kept
coming that no yes from now on (record 89) could cover, `sqlite3
-readonly …` (an option first) and `gh issue list --repo "$(git … )"`
(a `$(…)`). Of the two ways offered (a yes for one agent until its turn
ends, or `$(…)` covered when its inner command is), the person chose
the first.

**Decided.** `e` on an ask, in waiting for you (`App::ask_at_hand`) or on
its agent's focus or raw stream (`App::focus_ask`, record 103), or `:yes
[@NAME]` (the agent at hand, `App::agent_named`), says yes to everything
that agent asks until its turn ends (`App::answer_turn`):

- **Given** (`Live::yes_turn`, `Sim::yes_turn`): only on an open approval
  (`Live::ask_at`; anything else changes nothing and says `NAME asks for
  no yes now`). `World::yes_this_turn` sets `Agent.turn_yes = Some(0)` and
  journals `Event::TurnYes` (once while it holds); the ask is allowed
  (`decide`), and so is every other open ask of that agent (counted).
- **Holds** (`Live::ask`, after the routine check and the standing yes,
  which do not count): `World::allowed_this_turn` lets any ask of that
  agent go on, a command of any shape, an edit, a fetch, whatever its
  vendor asks, counted; traced `allowed by your yes for this turn: …`.
  Another agent's asks still wait. The demo's asks (`Sim::approval`) go
  the same way.
- **Ends** (`World::turn_over`): live, at the turn's end in `Live::step`
  (after the cases where the same turn runs again: a tier fallback, a
  renewed cap) and at the session's end (`Live::end`: a stop, `:forget`,
  a re-choice); the demo, when the agent's work stops (not `busy`) or it
  is gone. The journal says `NAME's turn ended, and your yes for it: it
  let N asks go on` (`Event::TurnYesEnded`). A new turn asks again.
- **Seen**: `✓ turn` first on the agent's status (`ui::status_spans`, so
  on its salon row and focus, never cut), the footer's `e yes this turn`
  on a focus with an ask, waiting for you's row `a yes · e this turn · A
  from now on · r no`, the `?` help's `a A e r` row.

**Why this is not weaker than it looks.** It is the same power `a` has,
said once for a turn instead of once per call: the person is the gate
(record 26) and chooses it on a visible ask of a named agent; it is per
agent and per turn, never a project's or a run's; and it is said on the
row while it holds and in the journal with its count when it ends. A
leader's yes lets its commands run in the person's checkout as `a` would.

- **Growth**: one counter per agent, memory only; two journal lines per
  yes.
- **Concurrency**: the bus's calls, the sessions' asks and the person's
  key are all taken on the main loop (`Live::step`), so a yes and an ask
  are never decided at once.
- **Failure**: a crash forgets it (memory only): the next run asks again,
  the safe side.

Not done: a yes for a turn of every agent of a project; a cap on how many
asks it lets go.
