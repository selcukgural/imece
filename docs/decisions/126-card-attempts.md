# A card's attempts

F8, F9 and F10 of backlog #59 (epic #286; the person, 2026-11-08): a Δ
must provably come from its card's current attempt, and a card must be
provably finished. Three pieces, one pull request each: attempts and
outcomes (F8, below, built), a circuit breaker on tries and rounds (F9,
below, built), dependencies between cards (F10, decided, to come).

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

## A circuit breaker (F9)

Three tries a card (`TRIES`) and three rounds its change (`ROUNDS`).
Past them what would go on by itself stops, and an item waits for the
person: one more, or the card fails for good. The person's own reject
passes, and so do their notes and a clash they send back: they are
deciding already.

`Card.breaker: Breaker` counts and keeps what the person said:
`sent_back` (each time the card's change came back to work: the
person's reject, notes or clash, or Lonca's queue or a human there
saying no or finding a conflict, `World::sent_back` from
`change_rejected` and `cards_landed`; a worker that withdrew its own
proposal was sent back by nobody), `more_tries` and `more_rounds` (the
person's yeses past the caps) and `stopped` (failed for good).
`Card::tries_out` is `attempt >= TRIES + more_tries`, `Card::rounds_out`
is `sent_back >= ROUNDS + more_rounds`: the third try and the third
time back are the last that go by themselves.

What stops (`Cap`):

- **Tries.** The leader's `assign` naming a card past its last try is
  refused (`bus::tries_left`): one still at work is its worker's to
  finish (`#3 is on try 3, its last`), nothing asked; a failed one opens
  an `InboxKind::Tries` item for the leader (`World::capped`, one open a
  card and cap) and the leader hears it is the person's now. That item
  holds no mail: the leader's team goes on.
- **Rounds.** Lonca's queue (or a human there) sending back a change for
  the third time does not wake its worker (`tell_landed`): what it was
  told waits in its notes, its wake is set, and an `InboxKind::Rounds`
  item holds it (it holds the worker's mail, record 107, so neither the
  wake nor the leader's next message starts a turn). A rejection the
  person worded through imece passes: it is theirs.
- **The board's offer** never meets it: a claimed card does not go back
  to the board, so a card there was never tried.

The person's answer (`a` one more, `r` fail it; Home's row says
`failed 3 tries: one more, or it fails for good` / `its change came back
3 times: …`, the footer `a  one more   r  fail it`, glyph `⊘`, group
"questions"): `World::decide_cap` closes the item; yes adds one to
`more_tries` or `more_rounds`, no sets `stopped` and the card `Failed`
now. `Live::decide_cap` tells whoever waited with its next prompt: a
leader hears it may hand the card once more, or that nobody hands it
again, and wakes for it; a worker allowed one more round wakes to what
it was told of its change; a worker whose card failed is not woken (its
wake dropped, a note that it leaves the card) and its leader is, told
the card failed after N times back. A stopped card is refused to any
`assign` (`the person failed #3 for good`), and the `board` tool says
`its tries are out: another waits for the person's yes` or `the person
failed it for good` where it said `the leader may hand it again`.
Journal: `Event::Capped` (blocked) and `Event::CapAnswered` (the
person's); the story `StoryEvent::Capped`/`CapAnswered`; both in both
languages.

The demo mirrors it (record 31): a card that failed fails each try its
leader hands it by itself (its story has a failure and its try is at
most `TRIES`), so the leader's fourth hand meets the breaker; the try the
person allows goes through. A card whose change failed its tests in the
queue (each fifth landing) fails them each round after while the person
allowed none (`Extras.hard`), so its third time back waits for them; the
round they allow lands. `Sim::decide_cap` as live's; the demo's leader
goes on with other work while a card waits at its breaker.

- **Growth**: one JSON column a card, an item and two journal lines a
  breaker; nothing that grows with time.
- **Concurrency**: the bus answers on the main loop; the Lonca look's
  result is applied there too, so a cap and a hand never race.
- **Read cost**: `rounds_out_of` walks the cards once per proposal that
  came back, not per frame.
- **Ten times the scale**: the same walks over ten times the cards, per
  decision.
- **Migration**: schema 29: `cards.breaker` (`{}`, every card with its
  tries and rounds whole); `Tries`/`Rounds` and the two events are new
  serde variants. A card's save sum now takes its attempt and breaker
  too: a yes past the breaker changes neither column nor owner, and
  would not have been saved (nor would a try handed back to its own
  worker at work, from F8).
- **Failure**: a crash after a cap keeps the item and the card's count;
  a worker held at its rounds is not woken again at the next start (its
  wake is memory): the person's yes wakes it.

Gaps. A worker whose card the person failed may still send a `done` for
it: its change shows and the person rejects it as any (the card stays
failed). The git path has no automatic round: every change sent back
there is the person's, so only the tries cap applies to it.

## Dependencies (F10, to come)

Decided: `after: [N]` on `assign`, `post` and `plan`. A card whose
dependencies are not done is not offered and cannot be claimed; an
`assign` to it waits on the card and goes when they are; one whose
dependency failed says `✕ waits on #2`. `board` takes `ready`, and the
board window filters to the ready ones.
