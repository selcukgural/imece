# A card's attempts

F8, F9 and F10 of backlog #59 (epic #286; the person, 2026-11-08): a Δ
must provably come from its card's current attempt, and a card must be
provably finished. Three pieces, one pull request each: attempts and
outcomes (F8, below, built), a circuit breaker on tries and rounds (F9,
decided, to come), dependencies between cards (F10, decided, to come).

## Attempts and outcomes (F8)

A card follows its change (the person's choice of three). Until now a
live card was done at its owner's every ok turn end (`sessions::apply`),
so a worker that asked something and ended its turn finished its card.
Now `Column` has two more states, and the change moves the card
(`World::move_cards`, from the same `World` calls live and the demo
make, so both move it alike):

- `Working` until the owner's `done`;
- `Review` while its change waits for the person (`change_ready`);
- back to `Working` when it is sent back (`change_rejected`: a reject,
  notes, a clash sent back), a round more;
- `Verifying` in Lonca's merge queue (`proposal_left` with `Queued`),
  `Done` on trunk, `Working` when the queue or a human said no or the
  worker withdrew (`proposal_left` for a change shown, `proposal_landed`
  for one in the queue, each moving only its own column);
- `Done` once the change is applied whole (a part applied keeps it in
  review: the rest is read again) or the `done` left nothing to take
  (`change_none`);
- `Failed` when the `done` says so (below).

A worker's worktree holds the work of every card it owns that is not
through, so its change is theirs: `move_cards` moves every card of the
worker's standing in the column it moves from, not only the newest. A
card through (`Card::over`: done or failed) has its `finished` tick. The
person's words to a worker whose card was sent back go on with that card
(`unfinished` is now a card at work that is still the agent's own), so
the rounds of one card stay one card; a card in review, done or failed
makes the next task a card of its own, as a done one always did.

Attempts. `Card.attempt` counts the times a card was taken: the turn an
`assign` opened (1), a `claim` (+1, so a posted card starts at 0), or an
`assign` naming the card (`card: N`, +1). `Agent.attempt` is the try its
turn took (`World::takes`, at the prompt that takes it; memory only, a
load sets it from the card, since the turns before ended with the run).
`World::done_for` tells what a `done` is for: the card its agent holds
when it still owns it at that try (`DoneFor::Card`), else a late one
(`DoneFor::Late`), or nothing. The agent never writes its attempt:
imece knows which turn the `done` came from, and an agent's own number
could be wrong or stale.

Handing a card again. `send` takes `card` with an `assign` (the leader's
tool; refused on any other type, with `paths`, for a card of another
project, to a non-worker, and for a card in review, in the queue or done,
each refusal in words). `World::hand` takes it from whoever held it at
once, when the assign is sent, not when it is delivered: owner, next
attempt, at work, `finished` cleared, `StoryEvent::Handed` and
`Event::Handed`. Whoever had it hears it in its next prompt (`card #3
went to claude-2; a done from you for it closes nothing now`). The mail
carries the card (`Mail.card`), and the turn it starts takes it instead
of a new one (`Opening.card`); its heading names it and its try
(`[assign 14 · #3, try 2]`) and the card's own paths ride along.

A late `done` (its card handed again since its turn took it) closes
nothing and shows no change: no snapshot, no Lonca gate (`gate_of`),
`StoryEvent::LateDone`, `Event::LateDone`; it still reaches the leader,
headed `[done 12 · late: #3 is claude-2's now, it closes nothing]`, and
its sender is told why. The worktree keeps what it wrote.

A failed outcome. `send` takes `outcome: "failed"` with a `done` (`done`,
the default, hands the change in; anything else is refused, and
`outcome` on another type too). It fails the card (`World::card_failed`:
`Failed`, `finished`, `StoryEvent::Failed` with the done's first line,
`Event::CardFailed`) and shows no change; the leader's mail is headed
`[done 12 · #3 failed]` (no cost: nothing was settled) and the leader is
told to hand the card again with what to do differently. The field is
in the envelope, not the body: the body is never interpreted (record
03), and no eleventh message type was needed.

On screen. The board puts `Review` in TO REVIEW (a card saved done with
its change still waiting, from before, still stands there) and `Failed`
with the done ones, marked `✕`; a card on its second try or later says
` · try 2` on the board, its screen and an export, whose state word is
FAILED for one that failed. The story pane heads a failed card `✕
failed`. The wake briefing says where its card stands (handed to whom,
in review, in the queue, failed). The `board` tool says `failed by …,
try N` and `done by …, its change waits`. The journal says each of the
three events in both languages.

Leases. A card in review lets its leases go, as the demo's done always
did; one sent back to work has lost what another card took meanwhile,
as a lapsed lease in Lonca, and keeps the paths it owns (record 114).
Only the demo has symbols; live's leases are Lonca's (record 123).

The demo mirrors it (record 31): every seventh first try a run finishes
fails (`FAIL_EVERY`, a count, not the dice, so tests stay put), its
leader hands a failed first try to another free worker once
(`Taken::Handed`), and a first try at work `SLOW_TICKS` (100) goes,
one tick in `SLOW_ONE_IN` (10), to a free worker; the worker it was
taken from ends its turn with a late `done`.

- **Growth**: one story line and one journal line per hand, failure and
  late done; two small columns per mail row while it waits. Nothing new
  that grows with time.
- **Concurrency**: the bus answers on the main loop, as before; a hand
  happens when the assign is sent, so a `done` racing it is late the
  moment the assign is in.
- **Read cost**: `move_cards` walks the cards once per change settled,
  not per frame.
- **Ten times the scale**: the walk is over all cards, ten times as
  many is still a walk per decision.
- **Migration**: schema 28: `cards.attempt` (1, or 0 for a card waiting
  on the board), `mail.card`, `mail.outcome` (`"Ok"`); the column and
  event names are new serde variants, so an older imece refuses the file
  by its schema number, as every bump does.
- **Failure**: a crash between a hand and its delivery leaves the card
  handed and the assign in the mail, both saved; a load sets each
  agent's attempt from its card, so a resumed worker's next `done` is
  for its card.

Gaps. A Lonca proposal a worker makes after its card was handed away
still becomes its Δ (the proposal is the store's; imece shows it, the
card does not move). The `board` tool still lists only posted cards;
F10's ready filter widens it.

## A circuit breaker (F9, to come)

Decided: three attempts and three rounds a card. At the cap the
automatic paths stop (Lonca waking a worker whose proposal came back,
the leader's `assign` naming the card, the board's offer) and an item
waits for the person: one more try or round, or the card fails. The
person's own reject passes: they are deciding already.

## Dependencies (F10, to come)

Decided: `after: [N]` on `assign`, `post` and `plan`. A card whose
dependencies are not done is not offered and cannot be claimed; an
`assign` to it waits on the card and goes when they are; one whose
dependency failed says `✕ waits on #2`. `board` takes `ready`, and the
board window filters to the ready ones.
