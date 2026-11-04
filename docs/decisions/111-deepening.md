# Deepening

The person's decision (2026-11-04), after a review of where imece stood:
101k lines of Rust, 1,361 tests, 110 records, 40 guide pages in two
languages, 194 pull requests merged in eight days, and two real sessions
(Kule on 2026-10-30, imece on itself since 2026-11-04). Most of those pull
requests were screen features taken from the neighbour studies
(`docs/neighbours.md`, backlog #56–#60). The loop imece exists for (the
leader splits, a worker does, its `done` becomes a Δ, the person reads and
applies) did not deepen at the same rate: the leader hires and forwards
and has no plan the person sees before the money is spent; a card is open,
claimed or done and nothing more; whether a Δ builds depends on the worker
remembering the `run` tool; a Δ says nothing of what it cost.

The decision, in one sentence: imece's product is the Δ gate, not the
screen. The coming period goes to making every change that reaches that
gate more trustworthy before the person reads it, turning the leader into
a visible planner, and making the code workable by imece's own team. A new
screen feature comes only out of a real session's friction.

What this changes in how work is chosen:

- No more features imported from neighbour studies. A line of #56–#60
  that is not in the first round below stays a line until a field entry
  asks for it.
- Every real session (a team or a solo on Kule, on imece, anywhere) leaves
  an entry in `docs/field/`: what rubbed, which seam it came from (the
  leader, the bus and board, the Δ gate, sessions, the screen), what was
  done about it. New work comes from those entries or from the epic
  (#286).
- "Demo mirrors live" (record 31) and the other standing rules hold; the
  demo's cost is a reason to put it on the real `Live` later (below), not
  to drop it.

The first round, each its own pull request and record:

1. The split (#287). `app.rs` (16.6k lines, 7.6k of them code, one `impl
   App` of 6.4k lines, a struct of 84 fields) and `ui/mod.rs` (13.6k) are
   the files every change touches; the leader's own instructions say such a
   file belongs to one worker at a time, so two imece workers cannot work
   on imece. The struct stays in `app/mod.rs`; the `impl App` blocks move
   to `app/<concern>.rs` with their tests, in slices of about 800 lines a
   pull request. Behaviour does not change. Meant for imece's own
   cheap-tier workers: the issue is the module map.
2. Evidence on a Δ (#288). A project's check (`.imece/check`, run on the
   setup script's terms, record 92) runs in the worker's worktree when its
   `done` makes a Δ, and the Δ's header says what passed and what failed
   with the tail of the failure. A note, never a gate: the person still
   decides. voff's "done needs evidence" (`docs/neighbours.md`), unbuilt
   until now.
3. Cost on a Δ (#289). What the worker spent between the assign and the
   `done`, on the Δ's header and in the journal line. The pot says what the
   team spent; only a price per Δ lets the person judge the leader's hiring.
4. A plan the person sees (#290): F20 and F16 of #59. Before the leader
   hires more than one worker, its plan (the cards, each worker's tier, the
   paths each owns, the cost it expects) comes to WAITING FOR YOU; `assign`
   carries the paths a card owns, and a worker that writes outside them is
   told so. Owned paths are the cheap stand-in for symbol leases (record
   17's code map, `docs/neighbours.md`): leases come only if paths prove
   not enough.

After the first round, in the epic and in this order unless a field entry
says otherwise: a worker told at once when the checkout's HEAD moves under
its waiting Δ, not only when the apply fails (record 108); card
dependencies, attempt numbers on `done` and a cap on revision rounds (F8,
F9, F10); health notices to the leader (F6: a stalled card, an idle
worker, a loop); what Claude Code alone allows, each probed first (its own
sandbox switched on for workers and verified fail closed, its sub-agents
shown as rows under the worker in place of or beside helpers, a
repository's skills under `standard`); the demo on the real `Live` with a
`claude` stand-in that speaks stream-json, so `Engine::Sim` and its 52
arms in `app.rs` can go; the researchers record (01) closed, since
workers read the web themselves since record 26.

What is not touched: the `Agent` trait and `Event` stay as they are, the
seam a later model would go through; nothing is built behind it now. The
tiers stay: Claude Code's aliases follow its newest models.

Growth: `docs/field/` grows one short file a session; nothing reads it but
people. Ten times the scale: a hundred records and a hundred field
entries are still files in two directories; the index in `CLAUDE.md` is
what grows on an agent's context, and record 110 made the whole file
reach agents, so a long index costs tokens every session: when it passes
what a session can carry, the index moves to a file of its own and
`CLAUDE.md` keeps the rules.
