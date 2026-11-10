# A worker that stops is its leader's to solve, and nothing that stops development stays silent

Issue #455 (user, 2026-11-10): whatever stops the harness or keeps
workers from working never stays silent; the leader always tries to
solve it, brings the person the safest choices when it cannot, and gets
the worker going again with the answer. The person took the mock-up's
proposals as a whole (A · S1 · R1 · E, 2026-11-10).

## The inventory

Read from the code at 376df1b, before any of this was built. LOUD: an
item in TO DECIDE or a red row; QUIET: a journal line, a header word or a
row that looks idle; SILENT: nowhere.

- The whole team: the pot spent (QUIET: the header's `pot spent`; mail,
  wakes, retries and offers skipped without a word); `MAX_WOKEN` (12
  message turns since the person spoke: SILENT, only the sender's tool
  error); Lonca down, frozen or signed out (QUIET: `⌀ no lonca`, one
  journal line; workers go on and their `done` is refused); the leader's
  own session ended (LOUD, but every send of its workers, `done`
  included, is refused behind it); the leader's turn hung or failed
  (SILENT: health watches no leader); the usage limit at 100% (LOUD, late:
  three retries per agent, then one item per agent).
- One worker: its session not starting, ended, or stopped by the check of
  what it runs with (LOUD for the person; its leader never told, and its
  send to the worker refused; queued prompts dropped; the next start a
  blank session); a turn failed with no fault (QUIET and wrong: journaled
  `session ended` while the session lived); a passing API fault (LOUD,
  heals itself); a setup script that failed (QUIET); a permission ask
  Claude Code gave up on (stale: the item stays and holds the worker's
  mail; how long Claude Code waits is not in the code).
- Waiting for what never comes: a teammate's `ask` with no answer
  (QUIET; `Status::Asking` is never set live, so health's `Waiting`
  never fires there); a proposal Lonca sent back (QUIET, the worker
  alone told); a card after a failed one (QUIET, the board only); a
  posted card nobody claims (SILENT); cards open and nobody working
  (SILENT, no such state); a refused `hire` (QUIET); a solo's hung or
  failed turn (SILENT: health watches no solo).

## The rule

Every stop has an owner. What the leader can solve (a worker's session
ended, a failed turn, an unanswered ask, a proposal sent back, cards
waiting on a failed card, a card nobody takes) is told to the leader,
which is woken, and it takes one of record 143's three ends: it solves
it (the person hears nothing, the journal says so), it asks the person
with choices, or it lets it lie and imece brings the stop to the person.
What only the person can lift (the pot, `MAX_WOKEN`, Lonca, an account
fault, the leader itself, a solo) goes straight to the person, with
imece's own fixed choices as digits (S1). `stall off` silences what goes
to the leader, never what goes to the person. imece itself lifts
nothing: it never raises the pot or starts a session on its own.

The work goes in four pull requests, each useful alone:

1. the truth and the leader told (this record's first part, built);
2. what stops the whole team, to the person: a `■ STOPPED` line atop
   Home (look A), a `stopped` group first in TO DECIDE, its choices as
   digits;
3. what waits for what never comes;
4. the stale permission ask, after probing how long Claude Code waits.

## Part 1, as built

**A stop is a block.** `blocked::What` says what a block is of: `Said`
(record 143's `blocked`), `Ended` (the session ended) or `Failed` (a turn
failed with nothing imece tries again, its session alive). `World::stood`
opens one when a worker's session ends (`live::sessions::apply` of
`AgentEvent::Stopped`, every path: the start thread, a prompt that could
not be written, `stop_unheard`, the process gone) or when
`World::turn_failed` has no fault to retry or bring to the person. It
takes a number of its own (`World::number`, as a message would), so the
leader names it in an `ask` to the person with `re` as for a `blocked`.
None opens for a leader, a solo, a helper, a worker put away or whose
leader is gone or stopped: those stops are the person's as before. Nor
for a session imece stopped because it did not run with what was asked
(its words start with `claude::CHECK_STOPPED`): whatever differed is the
person's to look at, never a leader's to start again.

**The leader is told at once.** `World.stops` (memory) lists the stops
not told yet; `Live::look_stops` (each step, after `look_blocks`) puts a
note in the leader's next prompt (`stop_note`: what happened, the card,
the stop's number, and the ways on: its message starts the worker again
once, `assign` with `card` hands the card, an `ask` to `you` with `re`)
and sets `lost`, so an idle leader is prompted `GO_ON` as for a health
sign (counted in `MAX_WOKEN`, refused over budget). `World::stops_read`
marks the leader's stops read with that prompt, so a turn of its that
ends with neither a word nor a question lets them lie (record 143's
`blocks_unsolved`), and `look_blocks` brings one older than
`:health stall`.

**The leader may start it again, once (R1).** The bus refused every
send to a stopped agent; now a waking message from the leader to its
worker whose session ended goes (`World::may_revive`), ends the block
(`Event::Unblocked { what: Ended }`, `lead started claude-2 again after
1 min: …`), puts the worker back to idle so the post office delivers it,
and sets `Agent.revived` (memory) until a turn of the worker's ends well
(`apply` of `TurnDone { ok: true }`) or the person writes to it (`World::said`). A worker that stops again with
`revived` set is told to its leader as stopped again, and the bus refuses
the leader's word with the ways left (hand the card, ask the person).
An inform, progress or notice starts nothing and is refused as such;
anyone else's send is refused as before. A card handed elsewhere or
withdrawn ends the block and puts a stopped worker to sleep (`Asleep`).

**Where it left off.** A session that ended sets `remembered`, so
whoever starts it again (the leader or the person) resumes the session
it had, with the briefing (`Live::brief`); before, a start within a run
was a blank session.

**The truth.** A failed turn with its session alive is journaled
`Event::TurnFailed` (`claude-2: turn failed: error_max_turns`), never
`Stopped` (`session ended`). The row, the item, the notice, the preview
and the detail word a block by what it is of (`What::of`): `session
ended "…" · lead is on it`, `turn failed "…"`, `claude-2 stopped · lead
asks: …`, `claude-2's session ended and its leader did not solve it`.
A stopped worker its leader has is drawn as a blocked one (`■` in the
attention colour) and is not `Rung::Stuck`.

**The demo.** `Sim::stop` makes every third (`STOP_FAILS`) stop a turn
that fails with no fault (`max_turns`), and opens the block for the
rest; `blocks_due` takes `World.stops` as live's `look_stops` does and
the leader takes each the three ways of `way_of`: it starts the worker
again (`stop_way`, `failed_way`), asks the person (`stop_ask` with two
answers, no risk: it could not solve it), or lets it lie; a worker
started again that stops again is asked about, never started twice.

## Part 2, as built

Part 2 itself went in two: 2a (this) is the stop of a whole team and
four of its causes; the usage limit as one stop of the team and the
leader's or a solo's hung or failed turn are 2b.

**A stop of a whole team** (`halt.rs`): `Halt` Pot, Woken, Store, Lead,
one `Stop` per project and halt (`World.halts`, memory only), with when
it began and who waits behind it (`Behind`: an agent and its `Waits`:
mail not delivered, a wake held, a retry, messages the bus refused, or
that it can not report). The engine says each step what it holds back
and why (`Held`), for only it sees its mail and wakes:
`Live::held_back` (`live/halts.rs`) and `Sim::held_back`.
`World::look_halts` makes the stops of that and of the world: the pot
spent and something held by it; an agent at `MAX_WOKEN` (moved to
`halt.rs`) with a wake held or a message refused; the project on Lonca
with its store `Down` (not while still asking); the leader `Stopped`.
A stop that begins is journaled (`Event::Halted`: `shop-api: the team
stopped: the pot is spent · 3 wait behind it`), one that ends too
(`Event::Unhalted`, with how long); one of a project closed goes unsaid.

**What it holds back.** The bus counts each sender's waking messages
refused for the pot or for the turns (`Live.held_sends`, reset when the
person speaks): that sender waits behind the stop. When the pot has
room again (`after_halts`) or the team may go on (`Live::go_on`, also
what the person's word does), each such sender is woken (`lost`) to go
on, for the message never went. Before, it stayed refused and nobody
woke it.

**Home** (A): `■ STOPPED  shop-api · the pot is spent · 3 wait behind
it · 2 min` on the first line (`ui/halt.rs` `banner`, above record 145's
line), and `HomeRow::Stop` rows first in TO DECIDE under the group
`stopped` (`HomeGroup::Stopped`, the heading counts `1 stopped`); the
row at hand ends with imece's choices as digits (S1, `Halt::choices`,
the one that changes least first): the pot `1 raise by $50.00  2 raise
by $20.00  3 rest here`, the turns `1 let it go on (12 more)`, Lonca `1
look again now  2 open the Lonca window`, the leader `1 start lead
again, where it left off` (`LEAD_AGAIN` to it as the person's word,
which resumes its session). `3 rest here` and `s` put the row after
everything else waiting (`App.rested`, forgotten when the stop is over);
the line stays. `enter` opens what stopped: the Lonca window, else the
leader's work. The preview names each agent behind it with what it
waits with, and what each choice does. A leader that is the stop has no
stuck row in AT WORK. The header puts ` ■` after the project and counts
stops in `! N to decide` (`World::to_decide`, the title and the
farewell too); the tab is stuck. A stop rings as an item does
(`Note::Halt`, loud: `imece · shop-api stopped: the pot is spent`), a
click lands on its row. The help's `W  S` row names `1-9`.

The mock-up's `start lead fresh` is not built: a resume Claude Code
refuses already starts afresh (record 20). The usage limit's line is
2b.

**The demo** mirrors it by the clock, so its dice and its scenarios stay
as they were: every `CAP_EVERY` (900) ticks each team has taken its
turns and its leader hands out nothing (`Extras.capped`, cleared by the
person's word or `1`); every `LEAD_STOP_EVERY` (1,300) its leader's
session ends; a spent pot tries no retry and holds the leader that
would hand out and the free workers that would take a card. Two seeded
tests took a new seed for it.

## Part 2b, as built

**The usage limit is one stop of the team** (`Halt::Limit`). A turn
that fails with an account's fault whose words are the plan's limit
(`imece_agent::is_usage_limit`: `usage limit`, `hit your limit`, `limit
reached`) no longer opens an item per agent: `World::limit_hit` keeps
one `Limited` in `World.halts` (memory) with the tick it began, the
agents whose turn it cut and the tick it is tried again: when Claude
Code's windows say a window at 100% opens (`opens`), else
`LIMIT_AGAIN` (15 minutes) after the last failure. Each cut turn is a
retry with `attempt` 0, at that tick; its row says `■ usage limit ·
again in 12 min`. While it holds (`World::limit_holds`, up to the tick,
not on it) live delivers no mail, takes no wake or retry and offers no
card (`deliver`, `offer`, the step's wakes and retries), and says so in
`held_back` (the pot first); the person's own word still goes. A turn
that goes through anywhere (`TurnDone { ok: true }`) lifts it
(`World::limit_lifted`) and the cut turns go at once. The stop holds
while anyone waits behind it or a cut agent is there: `■ STOPPED
shop-api · your Claude usage limit is reached · opens 17:00 · 4 wait
behind it`, the row's choice `1 try again now` (`World::try_now`). A
quota, a bill or a sign-in is the account's item as before.

**The head's own turn** (`Halt::Hung`, `Halt::Failed`). Health watches
no head, and a head's failed turn was a journal line alone. Now a head
(the project's leader, or a solo) mid-turn whose newest trace line (or
its status change, whichever is later) is as old as `:health stall` is
its team's stop: `lead's turn gives no sign of life · last: Bash "cargo
test"` (`World::hung_since`; not while it starts, waits on the person or
for a retry). `stall off` makes it `HUNG_MINUTES` (10), never silent:
that knob silences what goes to the leader only. `1 end the turn, go on
where it left off` (`Live::end_turn`: the session ends, cutting the
turn, and resumes on `HUNG_AGAIN`, which says not to run what hung the
same way again; refused over budget as a word is). A head's turn that
fails with nothing imece tries again (`World::head_failed`, from
`turn_failed`) is `lead's turn failed · error_max_turns` until the
head's next turn, `1 tell lead to go on` (`FAILED_AGAIN`).
`Halt::Lead` now covers a solo too, started with `SOLO_AGAIN`.

**The demo** mirrors them by the clock, each once its leader is idle
and in turn (`Extras.head_due`): every `HEAD_FAIL_EVERY` (1,100) ticks
its turn fails; every `LIMIT_EVERY` (1,700) the plan's five hours are
full for `LIMIT_TICKS` (60) and the leader meets the limit, which opens
by itself (an answer lifts it only after its time); every `HUNG_EVERY`
(2,300) it hangs (`Sim::hang`, kept hung each tick by `still_hung`)
until the person ends the turn (`Sim::end_turn`). imece's words to a
head (`halt::is_again`) post no cards in the demo. The tests' person
(`answer_all`) takes the first digit of these three stops; one seeded
test took a new seed (1).

## Part 3a, as built

Part 3 went in two as well: 3a (this) is what one worker waits for;
the cards that wait for what never comes (a card after a failed one, a
posted card nobody takes, cards open and nobody working, a refused
`hire`) are 3b.

**An ask with no answer** (`What::Unanswered`). A worker's `ask` to a
teammate is `Agent.awaits` (`blocked::Awaits`: whom, the number, the
first line; memory only), set by the bus (`World::asks`, a worker's
only; a leader's and a helper's are not followed). Any message of the
teammate's to it ends the wait, whatever its type (`World::answered`,
traced `answered by message N`), and so do its own `done` and its
leaving (`World::awaits_over`); a newer ask takes the older's place.
Between its turns it is now `Status::Asking { to, until: u64::MAX }`
live, set where a good turn ends (`apply` of `TurnDone { ok: true }`):
the row's `? → claude-3`, the lane's asking, and health's `Waiting`,
which never fired live. That sign is now a stop, not a notice: once the
worker on a card has waited `:health stall`, `look_health` opens a
block through `World::stood` and journals no `Health`; with no leader
that runs, the notice is said as before. The leader is told by
`look_stops` (`stop_note`: whom it asked, the card, the question, and
the ways on: answer it, tell it how to go on without, get the teammate
to answer, hand the card, ask the person) and woken; record 143's three
ends follow. The leader's waking word (`unblocked`), the person's word
and the card going elsewhere (`block_over`) end the wait with the
block; the teammate's answer at last ends both and is journaled as
`claude-3 answered claude-2 after 12 min: …` (`Event::Unblocked` by the
one asked). The row says `no answer from claude-3 "…" · lead is on it`
(`unanswered_from`, a block drawn on an asking row too); let lie, `no
answer to: … · lead did not solve it`. `What::of` takes four words.
`stall off` says nothing, by the rule: it silences what goes to the
leader, and this goes to the leader first.

**A proposal Lonca sent back** (a merge it could not make, a
refusal). The worker is told and woken as before; now its leader hears
it too with its next prompt, and why, and is not woken
(`live::lonca::sent_back`): `Lonca sent claude-2's proposal on #4 back:
verification_failed:tests. claude-2 was told and puts it right; nothing
is yours to do unless it says blocked.` One held back (a quota, a
freeze) says the worker will say it is blocked; one whose rounds are out
says the person decides.

**A setup script that failed** (a non-zero end, or one that could not
run). The worker started without it and heard nothing; now it hears
with its next prompt that what the script sets up may be missing, with
its last lines (`setup::failed_note`), and its leader hears that the
worker was told (`failed_told`); neither is woken. The script is the
person's, approved by its bytes: nobody is asked to mend it.

**The demo**: every `UNANSWERED_EVERY` (4th) ask between workers gets
no answer (`until` never comes; the world's `asks` and `answered` as
live's), its leader takes the stop when health opens it and answers with
`unanswered_way` (never asks the person about it); every `SETUP_FAILS`
(3rd) run of the setup script fails (`npm ERR! network timeout`) and is
traced to the worker and its leader in live's words; a proposal the
demo's queue sends back is traced to the leader (`Sim::leader_hears`).
Two seeded tests took new seeds (1 and 2).

## Part 3b, as built

The person took look K1 of the mock-up (2026-11-10): a card that waits
is a row of Home's AT WORK while its leader is on it.

**A card's wait** (`card_wait.rs`, `World.card_waits`, memory only):
`Why::After(d)` (a card it waits on, or one those wait on, failed or was
withdrawn: `blocked_on`) or `Why::Untaken` (posted, ready, nobody's, on
the board `:health stall` minutes since it was posted or since the last
card it waited on was done). `World::look_card_waits`, each step after
`look_stops` (live) and `blocks_due` (the demo), ends the waits whose
reason is gone (`still_waits`: taken, begun, handed again, withdrawn,
through; `Event::CardWaitOver` with who has it) and opens the new ones
(`Event::CardWaits`), each with a number of its own (`World::number`),
told to its leader: none while the project is closed or archived, its
head is no leader that runs, a stop of the whole team or the usage
limit holds, or the leader waits on the person (`told_of_card`). `stall
off` opens no `Untaken`: that knob silences what goes to the leader.
"Cards open and nobody working" and a refused `hire` are no wait of
their own: a card left open is one of these two, or behind another
stop.

**The leader is told and woken** (`Live::look_card_waits`,
`wait_note`): the card, what it waits on, the stop's number and the
ways on (hand the failed card again, withdraw the card, assign it, hire,
ask the person with `re`). `Live::blocks` (record 126's note, never
woken) is gone: the wait says it. The person failing a card for good
still tells the leader that what waits on it never begins
(`failed_for_good`). `stops_read` takes the waits too (`waits_read`).
Record 143's three ends follow: solved (the journal's
`CardWaitOver`), asked (`World::wait_asked` from the bus's ask with `re`
of the wait's number, or, naming none, the only wait read when no
worker's block is: the question stands on the card; the bus takes a
wait's number as `re` and `tried`/`risk` for it, `wait_for`), or let
lie: a turn of the leader's that ends with it read (`waits_unsolved`,
also when the leader stops) or one older than the stall
(`waits_too_long`, from `Live::look_blocks`) is `InboxKind::Waits` on
the leader, with the card (`Event::CardStillWaits`); it holds no mail
and closes on the person's word to the leader (`waits_said`: the wait
is the leader's again from then) or when the wait ends.

**Home** (K1): `HomeRow::Card` in `HomeGroup::Waits`, after a project's
stuck agents, while the leader has it or asked the person about it
(`waits_with_leader`): `■ #21 Import from CSV  waits on #20 which
failed · lead is on it · 3 min`, `nobody took it · posted 14 min ago`,
`lead asked you`; `enter` opens the card; the preview says since when,
who has it, what waits on it and what the leader does. The leader's
question reads `waits on #20 which failed · lead asks: …`; the item
`waits on #20 which failed · lead did not solve it` with `a  answer`.
The journal, the inbox line and the notice word it (`wait_why`,
`wait_words`), English and Turkish.

**The demo**: every `UNTAKEN_EVERY` (4th) card a leader posts by itself
no worker takes by itself (`Extras.shunned`); the demo's leaders take
each wait up at once (`card_waits_due`) and take the three ways of
`way_of`: solved (`solve_wait`: the card to a free worker, the failed
card handed again once someone is free and it may be, else the card
withdrawn), asked (`wait_ask` with two answers), or let lie; the
person's word solves it (`wait_answered`). The demo's cards now take the
tick they are posted at as `started`, as live's do. The tests' person
(`answer_all`) leaves an item an earlier answer closed; one seeded test
took a new seed (1).

## The points of the list

- **Growth**: one `Option<Block>` per agent (record 143's), one `bool`
  and the short `World.stops`, emptied each step. No new item kind. Part
  2: at most seven stops per open project, each listing its agents; two
  counters per agent in `Live.held_sends`; one `Limited` with the agents
  it cut, and one entry per failed head.
- **Read cost**: `stood` and `stops_read` pass over the agents once,
  at a stop or a leader's prompt, never per frame. Part 2: `held_back`
  and `look_halts` pass over the agents and the projects once a step;
  a frame reads `World.halts` only. 2b: `hung_since` is one look at the
  head per project a step.
  3a: one `Option<Awaits>` per agent; the bus sets and clears it per
  message, health reads it once per sign. 3b: one `CardWait` per card
  that waits; `look_card_waits` passes over the cards once a step (a
  column test for most, `blocked_on` for a waiting card with `after`),
  with a flag per card for the open waits; a frame reads the waits of
  the projects it draws.
- **Ten times the scale**: at 160 agents a leader may be told several
  stops in one prompt; `MAX_WOKEN` and the pot still bound its turns, and
  a stop it lets lie still comes to the person. 3a: more asks only
  mean more waits, each bound by its teammate's next word or the
  stall's minutes. 3b: a chain of cards after one that failed opens a
  wait each, told in one prompt; the stall still brings the rest.
- **Migration**: no schema change. `Event::TurnFailed` is a new name in
  saved rows, and `Unblocked`/`StillBlocked` carry `what` with a default
  (`Said`), so older rows read as before; an older imece cannot read a
  store that holds the new event. Part 2: `Event::Halted` and
  `Event::Unhalted` are new names too, with the same rule; nothing else
  is saved. 2b: three new `Halt` names in those rows, the same rule.
  3a: `What::Unanswered` is a new name in `Unblocked`/`StillBlocked`
  rows, the same rule; `Status::Asking` is not saved (a start puts every
  agent asleep). 3b: `InboxKind::Waits` and the three card events are
  new names in saved rows, the same rule.
- **Failure**: the block and `revived` are memory only. After a restart
  a stopped worker is asleep with its session remembered, as before, and
  its leader is not told again; one that came to the person is an inbox
  item and comes back, worded as a `blocked` (its `what` is gone).

## Gaps

- Part 4 is not built.
- Part 3b: a wait is memory only; after a start it is found again and
  its leader told once more. Who a card nobody took was offered to is
  the bus's and not said. A card whose holder (an `assign` held for it)
  left is not followed. The demo's solos post no cards; `--demo-live`
  (the stand-in) leaves no card on the board.
- Part 3a: a wait is memory only; after a start the asker does not
  wait any more (its session remembers the ask). A wait without a card
  is not told (health watches a card's worker). A teammate that left
  with the ask unread is told to the asker's senders as before (`unread`),
  and the asker's wait goes on until the stall. Lonca's send-back is a
  note, no stop: a worker that does not put it right is found by
  health's stall as any idle card.
- Part 2b: the usage limit and a failed head are memory only; after a
  start a cut turn is not tried again by itself (the stop is gone with
  the run, the session remembered). A turn of another agent already
  under way when the limit is met is not stopped; it fails on its own.
  The limit's time is read from Claude Code's windows, never from the
  error's words (`resets 3pm`). The demo mirrors the three stops for
  leaders only, not for a solo; `--demo-live` (the stand-in) meets no
  usage limit and never hangs.
- Part 2a: the stops are memory only; after a start the engine finds
  them again at its first step. `W` does not come to a stop (it walks
  the inbox); Home lists it first. A card offer skipped for the turns is
  not counted behind the stop. `--demo-live` (the stand-in) does not
  reach the pot or the turns by itself.
- A helper's or a researcher's stop is not followed here: a helper's is
  told to its worker as before (`helper_lost`).
- Prompts queued while a session started and then failed are still
  dropped; the resume and the briefing say where it stood, not what the
  dropped mail said.
- `--demo-live` (the stand-in) does not stop sessions yet.
