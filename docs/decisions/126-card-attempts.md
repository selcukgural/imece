# A card's attempts

F8, F9 and F10 of backlog #59 (epic #286; the person, 2026-11-08): a Δ
must provably come from its card's current attempt, and a card must be
provably finished. Three pieces, one pull request each: attempts and
outcomes (F8, below, built), a circuit breaker on tries and rounds (F9,
below, built), dependencies between cards (F10, below, built).

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
the rounds of one card stay one card; a card done or failed makes the
next task a card of its own, as a done one always did (the person's own
line opens it only once a change shows: "The person's line opens no
card", below). A card in review
whose change still waits (not a Lonca proposal) goes back to work with
the person's word (`World::word_goes_on`, from `Live::prompt` when
`Opening.person`, after the wake briefing has said where the card stood;
`Sim::told` the same): the acceptance run of 2026-11-09 wrote to a worker
whose change for #1 waited and imece opened #2 for it, so the next change
(#1's work and the follow-up) was headed `#2` and flagged two files
outside #2. It is not a send-back: no round is counted against the card
(`breaker.sent_back`, the ledger's `sent_back`), and the change is not
decided; it waits on until the worker's next change, which holds it, takes
its place (`change_ready`'s `carry_spend` carries what it cost), and an
apply of it meanwhile leaves the card at work (`change_applied` moves only
cards in review). A proposal is left to Lonca: its cards follow what Lonca
does with it.

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
card does not move). The `board` tool listed only posted cards; F10
widened it to every card.

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

## Dependencies (F10)

A card that needs another card's work first names it: `after` (the
cards it waits on, by their numbers on the board) on an `assign` or a
`post`, and on a `plan`'s cards by their place in the plan. The person's
choice of four, as decided: no new message type, the envelope carries
it, the body is never interpreted.

`Card.after: Vec<CardId>` is set when the card opens and never changes.
`bus::after_of` takes numbers from 1, each once, sorted, at most
`AFTER_MAX` (8); `bus::depends` takes only cards of the project the
person did not fail for good (a card waiting on one would never begin).
A new card's number is past every card it can name, so no card waits on
itself through others: a load refuses a file where one does (`card N
waits on a later card`). `World::ready` is every card it waits on done;
`waits_on` the ones not done; `blocked_on` the first that failed;
`dependents` the cards not through that wait on one.

- **A posted card waits on the board.** `Live::open` (what is offered)
  takes only ready cards nobody holds, and `claim` refuses one that is
  not (`#6 waits on #2, #5; pick a card that is ready (board with ready:
  true)`) or that an assign holds (`#6 is assigned to claude-1`).
- **An assign waits on its card.** With `after` the card opens when the
  assign is sent (`Live::opens`: the worker's, `Waiting`, attempt 0,
  titled by the first line, owning the `paths`), not at the turn it
  starts, and the mail carries it (`Mail.card`, as a card handed again
  does). Not ready: `World::hold` (`StoryEvent::Held`, `Event::Held`,
  the answer `held as message 14 on #6: it waits on …`). The mail stays
  in the worker's queue: `Live::deliver` takes the rest and leaves an
  assign whose card is not ready (`Live::waits`); once it is, the next
  delivery begins it (`World::begin`: at work, its first try, `started`
  now, `StoryEvent::Took`, `Event::Released`) before the prompt is
  written, so the heading says `[assign 14 · #6, try 1]`. A held assign
  does not wake a sleeper. An assign naming a card that is not ready
  (`card: 6`) holds it for the new worker instead of handing it
  (`Live::hand`): no try counted, and whoever it was held for loses the
  assign that waited (`the assign for it that waited for you is
  withdrawn`).
- **A failed card holds back what waits on it** until it is handed again
  and done; one the person failed for good, for ever. Either way the
  leader hears which cards wait on it with its next prompt
  (`Live::blocks`, from a failed `done` and from the person's no at the
  breaker).
- **The `board` tool lists every card**, posted or assigned (it listed
  only posted ones, the gap F8 named), the open ones apart, each with
  what it waits on (`; waits on #2, #5`, `, and #2 failed`), a held one
  as `assigned to claude-1`. `ready: true` lists only the open cards a
  worker may take now and counts the rest.
- **On screen.** The board window's TO DO marks a waiting card `⧗ after
  #2` (dim) or `✕ #2 failed` (bad), and a held one `→ claude-1`
  (`board::waiting_spans`); `r` there (`App.board_ready`, memory only)
  shows only the ready cards and the title says how many it hides
  (`board::hidden`); the help has the row, place `[board]`. A card's
  screen lists what it waits on, `✓` `✕` `⧗` (`card::after_spans`). The
  plan window says `after 1 above` (`Planned.after`, places from 1,
  earlier cards only: `hiring::planned_after`). The leader's
  instructions say to name `after` and that an assign with it waits, so
  a whole order can be assigned at once.

The demo mirrors it (record 31): a plan the person gives the leader
posts its tests and its docs after the work itself; one card in
`AFTER_ONE_IN` (3) the leader posts waits on the newest card not
through; the leader takes and assigns only ready cards, and one tick in
`HOLD_ONE_IN` (4) assigns a waiting card early, held for a free worker
(`Sim::hold`), begun once ready and the worker free (`Taken::Released`);
the demo's plan names the places of what its cards wait on.

- **Growth**: a short JSON column a card; one story line and one
  journal line per hold and release. Nothing that grows with time.
- **Concurrency**: the bus answers on the main loop; a card is done and
  a held assign delivered on the same loop, so nothing races.
- **Read cost**: `ready` walks a card's few dependencies; the post
  office's delivery and the offer ask it per mail or card, not per
  frame; the board window asks it per card drawn.
- **Ten times the scale**: at most eight dependencies a card, so the
  same.
- **Migration**: schema 30: `cards.waits_on` (`[]`); `Held`/`Released`
  are new serde variants; `Planned.after` is left out of a plan's JSON
  when empty, so a plan saved before reads as before. The column is not
  called `after`, a word of SQLite's.
- **Failure**: a crash while an assign is held keeps the card (held,
  its owner) and the mail (with its card), both saved; at the start the
  post office leaves it until the card is ready, as before.

What holds a card back is found through the cards it waits on
(`World::blocked_on`: breadth first, the nearest first, a done card not
looked through), and `dependents` are the cards that wait on one through
others too (one pass in number order, since a card waits only on older
ones): the board says `✕ #2 failed` on a card two steps away, and the
leader hears every card the failure holds back. The wake briefing says
what waits (`blocked_words`): an assign held for the agent and what it
waits on, and for a leader its project's cards that wait on others.

Gaps. What a card waits on is fixed when it opens: a card cannot be
made to wait on another later, nor freed of one.

## Withdrawing a card

The person's choice (2026-11-08): a card nobody is to do any more is
withdrawn, a state of its own, through like a done or failed one. It is
the leader's tool, `withdraw(card, why)`, a board tool beside `post` and
`claim`, not a `cancel` naming a card: a `cancel` goes to someone, and a
posted card nobody took has nobody to go to. No new message type.

- `Column::Withdrawn` (`Card::over`), `World::withdraw` (`finished`
  now, `StoryEvent::Withdrawn(why)`, `Event::Withdrawn`); `why` one line
  of at most 120 characters, the person reads it on the card.
- What may be withdrawn: a card waiting (posted, or held for a worker),
  at work, or failed. Not one whose change waits for the person (they
  reject it themselves), is in the merge queue or is done.
- Whoever held it at work or waiting is told in its next prompt and
  loses an assign that waited for it; a failed card's last owner is not
  told, it is through already. `World::done_for` makes a `done` for a
  withdrawn card late: it closes nothing and shows no change, its
  heading `[done 12 · late: #3 was withdrawn, it closes nothing]`, its
  sender told `#3 was withdrawn since your turn took it`.
- What waits on it never begins: the answer names it (`#3 is withdrawn;
  #6, #7 wait on it and never begin: withdraw them too, or tell the
  person`), the board says `✕ #3 withdrawn`, and no `assign`, `claim` or
  `after` takes it any more. A card the person failed for good tells
  the leader to withdraw what waits on it.
- On screen: with the done ones, marked `–` dim; WITHDRAWN on its
  screen, `– withdrawn` in the story pane, `✕` in a card's list of what
  it waits on; the journal `lead withdrew #3 Legacy parser: covered by
  #5`, and a late `done` for it `which was withdrawn`.

The demo mirrors it: every `WITHDRAW_EVERY` (300) ticks, by the clock so
the dice and the scenarios stay, its leader withdraws the newest card
nobody took, and it withdraws every card waiting on one never to be done
(withdrawn, or failed for good).

- **Growth**: one story line and one journal line a card withdrawn.
- **Concurrency**: on the main loop, as every tool.
- **Read cost**: `blocked_on` and `dependents` are linear in the cards;
  the board window asks `blocked_on` per waiting card drawn.
- **Migration**: schema 31: the `Withdrawn` column, story and event are
  new serde variants; no new column.
- **Failure**: a crash after a withdrawal keeps the card withdrawn and
  the held assign gone, both saved.

Gaps. A Lonca proposal of a card withdrawn while it was at work still
shows as its worker's Δ, as one handed away does (F8's gap): the person
rejects it.

## The person's line opens no card

The person's choice (2026-11-09): a line the person writes to a worker
opens no card by itself; a card comes only when the work produces a
change, or, as ever, from the leader's `assign`, `post` and plan and the
board. A real run had written `@claude-4 are you there? Answer in one
word.` and imece opened card #12 "are you there? Answ…": on the board,
in the story, in every count of cards. imece never reads the line to
decide (the body is never interpreted): it waits for what the work
shows.

- The line to a worker with no card at work (none, or one that is through
  or handed on) is the worker's own task, `Task.own: Option<Own>` (its
  first line and the tick), set by `World::own_line` from `Live::prompt`
  (`Opening.lazy`) and `Sim::told`. A teammate's mail that is no `assign`
  (an ask, an answer, an inform, a handoff, a helper's done…) waking a
  worker between cards is the same (`deliver`'s `lazy`, the user,
  2026-11-09): titled by its body's first line once a change of it shows;
  the leader's `assign`, `post`, plan and the board open cards as ever. The card it last had is not its own
  now: `Agent.card` goes, and its spending goes on a ledger row with no
  card again (record 133). A line while one waits is part of it: the
  first line's title stays. A worker with a card at work (or whose change
  waits, which the word takes back to work, above) goes on with that card,
  as before.
- When a change of that worker shows the person (`World::change_ready`:
  the git path's `done` read by `changes::read`, a Lonca proposal through
  `change_proposed`), the card opens then (`World::own_card`): titled by
  the line, the worker's, its first try, `started` the line's tick, a
  `Took` in its story, and straight into `Review` with the change. What
  the worker spent with no card (its open row with no card,
  `World::row_to_card`) is that try's row, and its round (record 113)
  opened at the line, so the Δ's cost counts from there. A card of its
  own at work already holds a change; the line is part of it then, and
  no other card opens. A card taken (`World::takes`: an assign, a claim,
  a card handed) takes the line in too.
- A turn that ends with nothing to show ends the line (the user,
  2026-11-09: "are you there?" then "fix the footer" names the card by the
  footer): the answer goes to the person as ever (TO DECIDE's replies),
  and no card ever opens for that line. At the end of the turn the line
  started or joined (`Live::step`, `World::own_ended`) imece asks off the
  screen's thread whether the turn left anything: on the git path the
  stand read at every turn's end, `worktree::Stand.holds` (the tree
  against its HEAD, the mark, as `worktree::change` reads it; a binary
  file counts) in `changes::stood`, only the last read asked after the
  turn's end; on Lonca the next look runs `Place::left_in` for the
  worker's workspace once its turn is over (`Live::lonca_lines`, the
  look's `left_in`: a session over says `status.sessions[].unproposed`,
  else `lonca diff`), and `Live::left_in` takes nothing there with no
  proposal of the worker's waiting (`proposal_of`, the look's `pending`)
  as nothing to show; Lonca not answering (`None`) keeps the line asked
  until a look does. Something there keeps the line (`own_checked`), so a
  follow-up mid-turn, or a turn that wrote and said no done yet, keeps the
  first line's title. A line written while the answer is out
  (`Own.later`) is part of the line when something was there, else the
  task in its place, asked about at its own turn's end. A `done` with
  nothing to propose on Lonca comes to the same once the look says so. A
  `done` with nothing in the
  worktree (`change_none`) or one that says it failed with no card
  (`DoneFor::Nothing`, `Outcome::Failed(None)`) ends the line: no card
  ever. A `done` from such a worker is for nothing (`DoneFor::Nothing`),
  never a late one, since its old card is not its own any more.
- Solos never had cards (record 59): neither a line nor its Δ opens one;
  `own_line` passes over every role but a worker.
- Home's row of a worker with no card is called by the line (`ui::home`
  `title_of`: change, card, the line, the name).
- Restart: `Own` lives in `Task`, which the store keeps whole in
  `agents.task` (JSON, record 113), with the round it opened: a run
  that ends between the line and the change opens the card at the next
  run's change, titled and costed alike. No schema change: an older
  `task` reads with no line (`serde(default)`).

The demo mirrors it: `Sim::told` keeps the line, the worker answers and
goes idle as a solo does once no other answer of it is due, and every
`OWN_CHANGE_EVERY` (2)th such answer comes with a change, which opens its
card; the rest have nothing to show and end the line. The demo's mail
opens no cards (its cards come from the leader), as before.

- **Growth**: one `Own` a worker at most, gone when a card opens or the
  line ends.
- **Concurrency**: on the main loop, as every tool.
- **Migration**: `Task` gains a field skipped when empty; nothing else.
  `Own.ended` and `Own.later` are memory only: a restart between a
  turn's end and its answer keeps the line, asked again at the next
  turn's end.
- **Read cost**: one more `rev-parse` in the stand read a turn's end
  already makes; on Lonca one `lonca diff` a look per worker whose line
  waits for its answer, on the look's thread.
- **Failure**: a read that fails, or Lonca down, keeps the line.

A worker woken after a restart with a line and no card is briefed with
it (`remember::brief`: `Your task: fix the footer (no card yet; one opens
when a change of it shows).`), not `You had no task in hand`.
