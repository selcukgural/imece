---
title: Messages and the board
description: How agents talk on imece's bus - the typed envelope, the ten message types, mail caps, numbered messages and replies, questions to you - and how the board hands out cards.
---

# Messages and the board

Agents talk to each other and to you over imece's bus with one tool, `send`, and share work nobody owns yet through the board (`post`, `board`, `claim`). You meet the messages in a card's story, in a conversation and in the journal; you meet the board in its window (`:board`).

## The envelope

Every message is a typed envelope plus a free-text body: `send(to, type, body)`, and optionally `re`. Only the envelope drives routing, waking and cost. imece never interprets the body: it is delivered as written (cleaned when it is drawn), and nothing in it can make imece do anything.

| Type | Meaning | Wakes the receiver? |
| --- | --- | --- |
| `assign` | a task; the body's first line becomes the card's title | yes |
| `ask` | a question that needs an answer | yes |
| `answer` | the answer to an `ask` | yes |
| `inform` | something worth knowing, no action | no |
| `handoff` | a task passed on to another agent | yes |
| `done` | the task is finished; a worker's `done` becomes a change | yes |
| `blocked` | cannot go on, and why; a worker's to its leader is the leader's to solve first ([a worker that is blocked](#a-worker-that-is-blocked)) | yes |
| `progress` | where the work stands | no |
| `notice` | a short note to the team | no |
| `cancel` | the task is withdrawn | yes |

There is no `ack`: a waking message starts the receiver's next turn, which is the acknowledgement.

Mail waits per agent. A waking type starts the receiver's next turn as soon as it is free; `inform`, `progress` and `notice` ride along with whatever starts the next turn. The receiver reads its mail at the start of that turn, each message headed with its number and sender:

```text
[assign 1] from lead:
#12 parser: tokens
Fix the tokeniser in src/parser.rs; done when `cargo test -p parser` passes.
```

## Caps

Caps keep one agent from flooding another and from running up the pot on its own:

| Cap | Value |
| --- | --- |
| Body | 4000 characters |
| Unread mail per agent | 20 (a `send` to a full box is refused: "wait for it") |
| Message-driven turns per agent | 12 since you last spoke to the team; then waking mail is refused until you write, and while something waits on it the team's stop is yours on [Home](../screen/home.md#when-a-whole-team-stops) (`1` lets it go on) |
| A stopped agent | gets no waking mail |
| Budget spent | nobody gets a waking message |

A refused `send` tells the sender why, so the model can decide what to do; nothing is queued silently.

## Numbers and replies

Every message one agent sends another gets a number, in order from 1 across the whole world and kept between runs. `send` answers with it ("delivered as message 7"). A reply may name the message it answers with `re`; the receiver sees it in the heading:

```text
[answer 8, re 7] from claude-1:
The new one.
```

`re` is checked, deny by default: it must be the number of a message the journal has, that was sent to the sender, by the one the reply goes to. Anything else is refused with the reason and takes no number. `re` links; it never routes, wakes or costs anything.

On the screen a reply wears `↩` after its type: on its journal line, in a card's story and in a conversation, where the first line of what it answers shows dim under it (`↩ re 7: Which parser?`). `%` jumps from a reply to what it answers and from a message to its first reply ([conversations](../screen/conversations.md)).

## Messages to you

The bus reaches you as `you`: `send(to: "you", type, body)`.

- An `ask` to you becomes a question in TO DECIDE (`?`, "claude-1 asks you: …"). While it is open the agent is that row, and is not listed under AT WORK. A newer question from the same agent takes the older's place. You answer by writing to the agent (`@claude-1 …`, or `a`/`r` on the item, which open the composer), and that starts its next turn.
- `inform`, `progress`, `notice`, `done`, `blocked` and `answer` to you only inform: a line in the conversation and the story, no item, nothing wakes. A worker's `done` to you shows its change like any `done`.
- `assign`, `handoff` and `cancel` to you are refused.
- Messages to you take no number and no `re`, but for the leader's `ask` about a worker's blocked, which names it ([a worker that is blocked](#a-worker-that-is-blocked)); your own messages have none. Nothing to you counts against the mail caps or the pot.

The prompts say when to ask: the leader may ask you mid-task; a worker asks its leader first and sends you only what only you can decide; a helper only what no teammate can answer. More on [approvals and questions](../concepts/approvals-and-questions.md).

## A worker that is blocked

A worker that cannot go on sends its leader `blocked`. The leader tries to solve it itself first: it reads the code, searches, and sends the worker the way on, or hands the card to another worker. You hear of it only when the leader asks you, or when the leader lets it lie.

- **While the leader has it** the worker stays under AT WORK with a `■` in the attention colour: `blocked "the retry test needs a clock" · lead is on it`. It is not your turn: it is not counted as stuck and nothing rings. Its preview reads the worker's message whole and what the leader has done about it so far.
- **Solved.** Any waking message of the leader's to that worker (an `answer`, a new `assign`) ends the block, and the journal says `lead unblocked claude-2 after 3 min: World::at is there for tests`. Handing its card to another worker or withdrawing it, the worker's own `done`, and your word to the worker end it too.
- **Asked.** The leader asks you only when the way on is risky, or when it found none. Risky is: deleting or overwriting work; reaching outside the repository (credentials, installs, network services, money); widening what the card owns or its task; dropping a requirement or skipping a test; changing something you decided. Such an `ask` names the blocked it is about (`re`: its number) and carries what the leader tried (`tried`) and why it is risky (`risk`, left out when it could not solve it). On Home the question is the blocked work's row, `claude-2 is blocked · lead asks: Let it into live/mod.rs?`, with the answers it offers under it. Its preview and its detail read `lead asks you · risky: it widens what #6 owns` (or `· could not solve it`), then the worker's words, what the leader tried, the question and the answers. Your answer goes to the leader, which sends the worker the way on; until you answer, the worker's row says `· lead asked you`.
- **Let lie.** A turn of the leader's that ends with a blocked it read neither answered nor asked about puts the block before you: a row under questions, `blocked: the retry test needs a clock · lead did not solve it`, where `a` writes to the worker and `enter` opens it. So does a block older than [`:health stall`](../reference/commands.md#health-stallidleloop-value) allows (ten minutes by default: `lead has not solved it in time`), and one whose leader stopped. This one is your turn: it is counted, it rings in another window, and the worker's mail waits with it. Your word to the worker ends it, and so does the leader's after all; the leader hears with its next prompt that you have it.

A `blocked` to a teammate that is not the worker's leader, a helper's to its worker and a `blocked` to `you` are plain messages, as before. A block the leader still had is not kept between runs (the [health notice](team.md#health-notices) for a stalled card finds that worker again); one that came to you is.

## A worker that stopped

A worker whose session ended (the process exited, Claude Code could not start, its worktree or Lonca session could not be made) or whose turn failed with nothing imece tries again (out of turns, an error that is no API fault) is its leader's to solve the same way as a `blocked`. Nothing that stops a worker stays silent.

- **Its leader hears it at once**, and is woken for it: what happened, on which card, and what it may do. The worker's row stays under AT WORK, `■` in the attention colour: `session ended "exited with code 1: out of memory" · lead is on it`, or `turn failed "error_max_turns" · lead is on it`. It is not your turn yet: nothing rings.
- **Started again.** The leader's waking message to a worker whose session ended starts it again, once, where it left off (its session resumed, told where it stopped); the journal says `lead started claude-2 again after 1 min: Start again; run the tests one file at a time`. A worker that stops again before a turn of its ends well is not started a second time: the leader hands its card to another worker or asks you. A turn that failed is ended the same way (`lead got claude-2 going again after 1 min: …`); its session never stopped.
- **Asked or let lie** go as for a `blocked`: the leader's question is the stopped work's row (`claude-2 stopped · lead asks: Give its card to another worker?`), and a stop the leader let lie comes to you as `session ended: … · lead did not solve it` (or `turn failed: …`), where your word to the worker starts it.
- **Yours from the start**: a session imece itself stopped because it did not run with what was asked (the tools, the permission mode, the hooks), a leader's or a solo's own stop, and a worker's whose leader is stopped too. Those stay red `■ session ended: …` rows, counted as stuck, as before.

A worker's row, the journal and a stop's item say a turn that failed apart from a session that ended: `claude-2: turn failed: error_max_turns` is not `session ended`.

## A worker that waits on an answer

A worker that sends a teammate an `ask` waits on its answer between its turns: its row says `? → claude-3`. Any message of that teammate's to it ends the wait, whatever its type, and so does the worker's own `done`; a newer `ask` takes the older's place.

An answer that does not come is its leader's to solve, as a `blocked` is, once the worker on a card has waited as long as [`:health stall`](../reference/commands.md#health-stallidleloop-value) allows (ten minutes by default):

- **Its leader hears it**, and is woken for it: who was asked, on which card, the question, and the ways on (answer it itself, tell the worker how to go on without it, get the teammate to answer, hand the card, ask you). The row stays under AT WORK, `■` in the attention colour: `no answer from claude-3 "Which port does the stub use?" · lead is on it`.
- **Ended** by the leader's waking word to the worker (the way on without the answer), by the teammate's answer at last (`claude-3 answered claude-2 after 12 min: 8080, the stub's default`), by your word to the worker, or by its card going elsewhere.
- **Asked or let lie** go as for a `blocked`: let lie, it comes to you as `no answer to: Which port does the stub use? · lead did not solve it`.

With `stall off` nothing is said: the leader is not told, and nothing comes to you. A leader's and a helper's asks are not followed this way.

## Mail waits with an agent that waits on you

An agent with something open that only you can settle does not go on because a teammate wrote: a question of its in TO DECIDE, a tool call waiting for your yes, an API error, a setup script, a sensitive change or a research report. Its mail stays queued and starts no turn, and no board card is offered to it. The sender hears `held as message 7; …`, and the agent's row and `:why` say how many messages wait: `✉ 2 queued`.

When you write to the agent, the mail goes in that same turn, after your words, if your word ends the wait. When you settle it another way, the mail starts a turn of its own. A `cancel` is never held: it starts a turn at once, and the mail that waited goes with it, in order. The mail caps count held mail as any other.

Two things in TO DECIDE hold nothing: an answer waiting to be read, and a worker's change. A worker with a change waiting is finished and needs no answer to go on, so it can take the next card while you review.

## You on the bus

Plain text in the command line goes to the leader of the project at hand; `@name text` goes to one agent. A message to an idle worker opens no card by itself: a card opens, titled by it, only when its work comes to a change, and that change closes it ([a card's tries](#a-cards-tries)); a line that comes to nothing (a question, a hello) is never a card, so the board and its counts hold work alone. A teammate's message that is no `assign` is the same; the leader's `assign`, `post` and plan open cards as ever. Your messages have no number. An agent's answer to you waits in TO DECIDE ("claude-1 answered you: …") until you open it (`enter`), dismiss it (`r`) or just write back.

## The board

The board is for work nobody has yet. For work one teammate should do, the leader sends `assign`.

| Tool | Who | What |
| --- | --- | --- |
| `post(title, body)` | everyone | puts a card on the project's board; `after` names the cards it waits on |
| `board()` | everyone | lists every card of the project, posted or assigned, the open ones apart, each with what it waits on; `ready: true` lists only the open cards a worker may take now |
| `claim(card)` | workers only | takes an open card; the answer is its task |

- An open card is **offered** to one idle worker at a time, as a turn of its own, with the title only (the body comes with the claim). It goes to the next worker only after that one's turn ended without taking it, so a card wakes each worker at most once, never a broadcast. An offer counts as a message-driven turn.
- One card at a time per worker. The worker sends `done` to whoever posted the card.
- A card posted before a restart does not wake a sleeping worker; one posted in this run does.

Caps: a title is one line of at most 120 characters (the screen shows its first 20 next to the worker, so it should start with what tells it from the other cards: an issue number, a file, a symbol); 20 open cards per project; 10 posts per agent until you speak again.

## The paths a card owns

An `assign` or a `post` names the paths its card owns: `paths`, at most 20 git pathspecs relative to the repository (`src/parser.rs`, a directory `docs`, a pattern `tests/*_test.rs`, where `*` stands for anything, `/` included). The card keeps them, between runs too; the worker's task ends with `This card owns src/parser.rs, docs (shared): stay in them; what else needs changing goes in your done, or ask for the card to be widened`, and a card's screen lists them (`owns`). A card without `paths` has no bounds.

- **No two open cards of a project own the same path.** An `assign` or a `post` whose paths an open card (waiting or working) already owns is refused with which card owns what: `#3 Fix the parser owns "src/parser.rs", which "src" names too; give this card other paths, or end one with the word shared (src shared)`. A path either card names the other way round clashes too (`src` and `src/a.rs`, `src/*` and `src/a.rs`). The card the receiving worker holds now is not counted: its next card may go on in the same files. A done card owns nothing any more.
- **A path every card touches is `shared`.** `CHANGELOG.md shared` names the path without owning it: another card may name it too, and the worker may write it. The leader is told to give a file most tasks touch to one card and tell the others to say in their `done` what it should get.
- **A worker that writes outside its paths strays, and is told.** Its session's `Edit`, `Write` and `MultiEdit` calls show the file; the first time a file outside the card's paths is written, the card's story says `changed src/other.rs, outside the card's paths`, the journal `claude-1 changed src/other.rs, which #3 does not own`, and the worker's next prompt starts with `you changed src/other.rs, which #3 does not own (it owns src/parser.rs); say so in your done, or ask lead to widen the card`. Once per file per card; never stopped, never refused: the person reads the change with the files outside the card counted above its patch ([changes](../screen/changes.md#what-is-above-the-patch)).

imece has no symbol leases. A path is what the leader can name when it plans, and what a change can be checked against without reading the files; on a project on Lonca the store's leases are shown on the [code map](../screen/board-and-code-map.md).

## The checks a card asks for

On a project on Lonca an `assign`, a `post` or a card of a plan may also name checks: `checks`, at most 32 names of checks the repository's Lonca policy defines (`verification.checks` in its `lonca.toml`, such as `api` or `e2e`). Lonca's own checks (`build`, `tests`) run anyway and are no names here. The card keeps them, sorted, between runs too; the worker's task ends with `This card asks for Lonca's checks api, e2e: once the person says yes, Lonca's merge queue runs them on your change too, and a failure comes back to you`, and a card's screen lists them (`checks`).

Nothing runs them before you decide. The card's change says them on its verdict line (`after your yes Lonca runs: build, tests, api · the card also asks for api`), and your `a` asks Lonca for them (`lonca approve 5 --also=api`): only a person signs an approval, so no agent asks Lonca for one itself. Lonca keeps them for the change's later versions.

A name the policy lacks is refused where it is written, once Lonca's `status` says which checks the policy in force defines (`policy_checks`, a Lonca newer than 0.2.0): the leader's `assign`, `post` or plan card gets `"apii" is not a check of Lonca's policy; it has api, e2e. Did you mean api?` and fixes it in the same turn, nothing sent. A card written before a newer policy dropped one of its checks says so on its change's verdict line, `Lonca's policy no longer has smoke: :checks changes what the card asks for`, and the change is not ready. With an older daemon, or a policy Lonca can not read, the yes is where a name is judged. There a name the policy lacks stops the yes: `your word on the proposal did not reach Lonca: Lonca's policy has no api check · :checks NAMES changes what the card asks for`, and the change still waits; change the card's checks (below), add the check to the policy (`lonca policy set`), or reject the change. Off Lonca nothing would run them, so `checks` is refused (`checks run in Lonca's merge queue, and this project is not on Lonca: leave checks out, and say in the body what to run`); [the repository's check](changes.md#the-repositorys-check) is what runs there. The demo's second project is on Lonca, and every third card of it asks for `api`.

A card's checks can change after it was made, by two hands, each in the journal and the card's story (`lead set #3's checks: api, e2e`, `you set #3's checks: none`):

- **The leader**, handing the card again (`assign` with `card`): `checks` given there take the place of the card's, `[]` for none; left out, the card keeps its own. The new worker's task names them.
- **You**, at the yes: `:checks api e2e` (commas or spaces between) on the change at hand sets what your yes asks Lonca for, `:checks none` takes them away, and a bare `:checks` says them. So a leader's typo need not turn a good change into a reject: drop or fix the name and say yes. Once Lonca says the policy's checks, the menu after `:checks ` offers them. Refused off Lonca, for a change made for no card, for a name Lonca would not take, and for one the policy lacks (`Lonca's policy has no apii; it has api, e2e · did you mean api?`).

The verdict line follows at once. imece adds and drops no check by itself, not even one the policy no longer has: you or the leader do. The demo's leader, handing again a card of the project on Lonca that asked for nothing, has it ask for `api`. The demo's policy defines `api` and `e2e`; its seventh card, and every ninth after, also asks for `smoke`, which the policy dropped, and a yes to its change is refused until `:checks api` takes `smoke` away.

## The issue a card closes

Work often comes from an issue of the repository. The leader says so on the card: `closes: 58` on an `assign`, a `post` or a plan card, the issue's number and nothing else (digits; not `#58`, another repository or an address: `closes must be the number of an issue of this repository, digits only`). One issue a card; a card handed again keeps its own (`a card handed again closes what it did; leave closes out`). The leader never closes it and never has a worker close it: imece does, after your yes, so the leader stays an agent that writes nowhere and no model's turn is spent on it.

The number is a model's word, so imece reads the issue once with your `gh` (`gh issue view`) and shows what it found wherever the number shows: the card's screen (`closes #58 “A 529 from the API stops the worker” · open`), the plan window (`closes #58`), the change's row on Home (`· closes #58`), and the change itself, in a line of its own under the verdict:

```
closes #58 “A 529 from the API stops the worker” · when it lands
```

A wrong number shows there before your yes: `closes #58 “…”: closed already · nothing will close`, `closes #58: that number is a pull request · nothing will close` (`gh issue` answers for a pull request's number too; imece closes only what a look found to be an open issue), `closes #580: this repository has no such issue · nothing will close`. Without `gh`, or signed out, the number stands alone. The title is the text of whoever wrote the issue: it is cleaned as an agent's text is, and no agent is told it.

What happens after your yes depends on where the change goes:

- **On Lonca** the issue closes when the change is integrated, not at the yes: a change that fails the merge queue closes nothing. With [`:closes on`](../reference/commands.md#closes-onoff) (the default) imece runs `gh issue close 58` in the checkout, and the journal says `closed #58 “…” · card #6 landed`; the leader gets a note with its next prompt, no turn of its own. With `:closes off` an item `!` waits in [TO DECIDE](../screen/waiting.md): `a` closes it, `r` leaves it open for good.
- **On the git path** your yes leaves the change staged, not committed, so imece closes nothing itself: [`:commit`](../reference/commands.md#commit)'s message ends with `Closes #58`, and GitHub closes the issue when that commit reaches the default branch. With `:closes off` the line is left out.

Nothing closes for a card that failed or was withdrawn, or one whose change is applied only in part. An issue that could not be closed is said once (`#58 was not closed: gh is not signed in · it stays open`) and not tried again; the leader hears that it stays open. `gh` gets the number as the only argument that comes from a model, and runs in the checkout, so it is always the checkout's own repository (`GH_REPO` is not passed on). Only GitHub issues for now. In the demo every third card names an issue; its board is made up (nothing leaves the machine), and every seventh issue does not close.

## A card's tries

A card follows its change, not its worker's turns. It is at work until the worker's `done`; it waits for you to review while its change (`Δ`) is in TO DECIDE; it goes back to work when you send the change back (a reject, notes, a clash), and your words to the worker go on with that card, a round more; your words while its change still waits take that card back to work too (the change waits on until the worker's next one, which holds it, takes its place), rather than opening a card of their own; it is done once the change is applied whole, or when the `done` left nothing to take. On Lonca it is verifying in the merge queue after your yes, done on trunk, and back at work when the queue or a reviewer says no.

- **A try.** A card counts the times it was taken: the `assign` that opened it is the first, a `claim` from the board is one, and the leader may hand a card again with `assign` and `card: 3` (one that failed, whose worker stopped, or that waits on the board), its next try. The card keeps its title and its paths; the new worker's mail is headed `[assign 14 · #3, try 2]`, and whoever had it is told `Card #3 (…) went to claude-2. Stop work on it; a done from you for it closes nothing now.` A card in review, in the queue or done cannot be handed: `#3's change waits for the person; it comes back if they send it back`.
- **A late `done`.** A `done` from a turn that began before its card was handed again closes nothing and shows no change: the journal says `claude-1: a late done for #3, which went to another try; it closes nothing`, and the leader's mail is headed `[done 12 · late: #3 is claude-2's now, it closes nothing]`. imece knows which turn a `done` came from; the agent never writes its try.
- **A failed card.** A worker that cannot do a task as asked sends its `done` with `outcome: "failed"` and why. The card fails (`✕` on the board, FAILED on its screen), no change is shown, and the leader's mail is headed `[done 12 · #3 failed]`; the leader is told to hand it again with what to do differently.

- **A card withdrawn.** The leader withdraws a card nobody is to do any more with `withdraw(card, why)`: one on the board, one it assigned (at work, or waiting on other cards), or one that failed. The card is through: it stands with the done ones marked `–`, WITHDRAWN on its screen, the journal says `lead withdrew #3 Legacy parser: covered by #5`. Whoever had it at work or waiting is told `Card #3 (…) was withdrawn by lead: stop work on it; a done from you for it closes nothing now.`, and loses an assign that waited for it; its later `done` closes nothing and shows no change (`[done 12 · late: #3 was withdrawn, it closes nothing]`). The answer names the cards that wait on it, through others too: `#3 is withdrawn; #6, #7 wait on it and never begin: withdraw them too, or tell the person`. A card whose change waits for you, is in the merge queue or is done cannot be withdrawn: you reject a change yourself.
- **A circuit breaker.** A card has three tries, and its change three rounds. The leader's `assign` naming a card that failed its third try is refused (`#3 failed 3 tries: the person decides whether it gets another`), and the card waits for you in TO DECIDE with `⊘`: `failed 3 tries: one more, or it fails for good`. On Lonca, a change the queue (or a reviewer there) sends back a third time does not wake its worker: it waits for you the same way, `its change came back 3 times: one more round, or it fails for good`, and holds the worker's mail meanwhile. `a` allows one more try or round (the leader, or the worker, hears it); `r` fails the card for good: nobody hands it again, its worker leaves it, and its leader is told. Your own reject, notes and clash sent back always pass: you are deciding already. The `board` tool says `its tries are out` or `the person failed it for good` for such a card.

The demo shows all four: every seventh first try fails, and its leader hands a failed card to another worker; such a card fails each try handed by itself, so its fourth waits for you, and the try you allow goes through. A change whose tests failed in the demo's queue fails them each round until you allow one more. Now and then the leader takes a card that has run long from its worker, whose turn then ends with a late `done`.

## Cards that wait on others

A card that needs another card's work first says so: `after: [2, 5]` on an `assign` or a `post` names the cards it waits on (at most 8, older cards of the same project). Until each of them is done:

- **A posted card is not offered and cannot be claimed.** `claim` answers `#6 waits on #2, #5; pick a card that is ready (board with ready: true)`, and the `board` tool lists it as `#6 Tests (posted by lead; waits on #2, #5)`.
- **An `assign` waits on its card.** The card opens at once, the worker's, untried; the leader hears `held as message 14 on #6: it waits on #2, #5, and claude-1 gets it once they are done`, and the journal `claude-1 gets #6 Tests after #2, #5`. Its other mail goes past it. Once the last of them is done, the assign goes like any (`[assign 14 · #6, try 1]`) and the journal says `claude-1 begins #6 Tests: what it waited on is done`. So the leader can assign a whole order at once. A card handed again while it waits (`card: 6`) is held for the new worker, and the old one's waiting assign is withdrawn.
- **A failed card holds back what waits on it**, and what waits on those, until it is handed again and done: the board window marks the card `✕ #2 failed`, the `board` tool says `waits on #2, #5, and #2 failed` (or `and #1 failed` when #1 is further down the chain), and the leader hears `#6, #7 wait on #2, which failed; they begin once it is done: hand it again, or tell the person.` A card you failed for good, or one the leader withdrew, holds them back for ever: the board says `✕ #2 withdrawn`, the leader is told to withdraw them or tell you, and no new card may wait on it.

The board window marks a waiting card `⧗ after #2` (or `✕ #2 failed`), and one held for a worker `→ claude-1`; `r` there shows only the cards a worker may take now (`ready only: 3 waiting hidden`), `r` again all. A card's screen lists what it waits on, each `✓` done, `✕` failed or `⧗` not yet. In a plan, a card names the cards of the same plan it waits on by their place (`after: [1]`), and the plan's window says `after 1 above`.

The demo shows it: a plan the person gives the leader posts its tests and its docs after the work itself, one card in three the leader posts waits on the newest card not through, and now and then the leader assigns a waiting card early, held for its worker until it may begin. Every 300 ticks its leader withdraws the newest card nobody took, and it withdraws any card that waits on one never to be done.

## The plan

The first worker is cheap and you asked for work; before the leader hires a second one, you see what it means to do. It calls `plan(cards, cost)`: the cards it means to assign or post, each with a title, a tier, the paths it owns and a body, and what it expects them all to cost in dollars. The plan comes to TO DECIDE as one `▤` item (`lead: plans 3 cards for $1.20; enter reads it`), the journal says `lead planned 3 cards for $1.20; waits for your yes`, and the leader is told to end its turn; while the plan waits, the leader's mail waits with it ([mail waits](#mail-waits-with-an-agent-that-waits-on-you)) and `hire` answers `the person is reading your plan; end your turn, their yes starts your next`.

- `enter` opens the plan whole: each card's number, title and tier, the paths it owns, the checks it asks for, the cards of the plan it waits on, its body; the expected cost beside what is left in the pot. Every word is the leader's, cleaned, never read as Markdown. A line longer than the window wraps within it, under its card's indent; `j k` scroll the wrapped lines.
- `a` says yes: the leader hears `The person said yes to your plan: hire and assign as planned` with its next prompt (at once when its turn is over) and may hire until its next plan; a new plan takes the yes back.
- `r` says no: the plan is dropped and the leader hears `The person said no to your plan: ask them what they want, or make another plan`.
- `n` writes to the leader instead: what you send closes the plan unanswered, and `hire` says so until a new plan has your yes (`the person answered your plan in words, not with a yes: make a new plan from what they said`). Any word of yours to the leader does the same.

A new plan replaces the one waiting. The plan is checked as cards would be: a title of one line, a body, a tier, paths, and no two cards of the plan owning the same path (`cards 1 and 3 both own a path: "src" and "src/a.rs"`); the envelope drives it and the body is never interpreted. One to twenty cards. The first worker needs no plan, nor does a leader hiring with no worker left; a solo never plans. `:plan off` turns the gate off for a project that trusts its leader (kept in the store, on by default); a plan sent then is not kept and the leader is told to hire as it sees fit. The demo's leader plans before its second worker too; the person's yes there comes from `a` as live.

## Cards and their stories

A card is a unit of work with a story: `intent`, `assumption`, `decision`, `talk`, `evidence`, `change`, `you` (what you said or decided). The board window shows cards in columns (TO DO, IN PROGRESS, TO REVIEW, VERIFYING, DONE; a failed card stands with the done ones, marked `✕`); `#N` or `#word` opens a card's own screen with who was on it, its story, and its own inbox and journal lines ([cards](../screen/cards.md)).

## The wake briefing

An agent asleep since the last run wakes on your message, a teammate's waking message or a card posted in this run. Unless its session resumes with its memory intact ([sessions](../concepts/sessions.md)), imece briefs the new session ahead of the first prompt: the card and its last 12 story lines, the prompt that was cut short (not run again), how many messages wait, the worktree's uncommitted changes, an assign held for it and what it waits on (`#6 Tests is yours once what it waits on is done (#2); its assign comes then.`), the leader's cards that wait on others and what holds them back, and the board's open cards. Every line goes through `clean`; a line is at most 300 characters, the briefing 6000.

## See also

- [Message types](../reference/message-types.md): the reference table.
- [Conversations](../screen/conversations.md): reading an exchange, `[ ]` turns, `%`.
- [Board and code map](../screen/board-and-code-map.md).
- [Cards](../screen/cards.md).
- [Changes](../concepts/changes.md): what a worker's `done` turns into.
