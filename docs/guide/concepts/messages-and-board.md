---
title: Messages and the board
description: How agents talk on imece's bus - the typed envelope, the ten message types, mail caps, numbered messages and replies, questions to you - and how the board hands out cards.
---

# Messages and the board

Agents talk to each other and to you over imece's bus with one tool, `send`, and share work nobody owns yet through the board (`post`, `board`, `claim`). You meet the messages in a card's story, in a conversation and in the journal; you meet the board as the second view (`2`).

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
| `blocked` | cannot go on, and why | yes |
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
| Message-driven turns per agent | 12 since you last spoke to the team; then waking mail is refused until you write |
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

- An `ask` to you becomes a question in TO DECIDE (`?`, "claude-1 asks you: …"). The agent's team line reads `? → you` while it is idle with one open. A newer question from the same agent takes the older's place. You answer by writing to the agent (`@claude-1 …`, or `a`/`r` on the item, which open the composer), and that starts its next turn.
- `inform`, `progress`, `notice`, `done`, `blocked` and `answer` to you only inform: a line in the conversation and the story, no item, nothing wakes. A worker's `done` to you shows its change like any `done`.
- `assign`, `handoff` and `cancel` to you are refused.
- Messages to you take no number and no `re`; your own messages have none. Nothing to you counts against the mail caps or the pot.

The prompts say when to ask: the leader may ask you mid-task; a worker asks its leader first and sends you only what only you can decide; a helper only what no teammate can answer. More on [approvals and questions](../concepts/approvals-and-questions.md).

## Mail waits with an agent that waits on you

An agent with something open that only you can settle does not go on because a teammate wrote: a question of its in TO DECIDE, a tool call waiting for your yes, an API error, a setup script, a sensitive change or a research report. Its mail stays queued and starts no turn, and no board card is offered to it. The sender hears `held as message 7; …`, and the agent's row and `:why` say how many messages wait: `✉ 2 queued`.

When you write to the agent, the mail goes in that same turn, after your words, if your word ends the wait. When you settle it another way, the mail starts a turn of its own. A `cancel` is never held: it starts a turn at once, and the mail that waited goes with it, in order. The mail caps count held mail as any other.

Two things in TO DECIDE hold nothing: an answer waiting to be read, and a worker's change. A worker with a change waiting is finished and needs no answer to go on, so it can take the next card while you review.

## You on the bus

Plain text in the command line goes to the selected project's leader; `@name text` goes to one agent. A message to an idle worker opens a card; its answer closes it. Your messages have no number. An agent's answer to you waits in TO DECIDE ("claude-1 answered you: …") until you open it (`enter`), dismiss it (`r`) or just write back.

## The board

The board is for work nobody has yet. For work one teammate should do, the leader sends `assign`.

| Tool | Who | What |
| --- | --- | --- |
| `post(title, body)` | everyone | puts a card on the project's board |
| `board()` | everyone | lists the open cards, and who took the others |
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

This is the cheap stand-in for symbol leases (see [the code map](../screen/board-and-code-map.md)): a path is what the leader can name when it plans, and what a change can be checked against without reading the files. Leases come only if paths prove not enough.

## The plan

The first worker is cheap and you asked for work; before the leader hires a second one, you see what it means to do. It calls `plan(cards, cost)`: the cards it means to assign or post, each with a title, a tier, the paths it owns and a body, and what it expects them all to cost in dollars. The plan comes to TO DECIDE as one `▤` item (`lead: plans 3 cards for $1.20; enter reads it`), the journal says `lead planned 3 cards for $1.20; waits for your yes`, and the leader is told to end its turn; while the plan waits, the leader's mail waits with it ([mail waits](#mail-waits-with-an-agent-that-waits-on-you)) and `hire` answers `the person is reading your plan; end your turn, their yes starts your next`.

- `enter` opens the plan whole: each card's number, title and tier, the paths it owns, its body; the expected cost beside what is left in the pot. Every word is the leader's, cleaned, never read as Markdown.
- `a` says yes: the leader hears `The person said yes to your plan: hire and assign as planned` with its next prompt (at once when its turn is over) and may hire until its next plan; a new plan takes the yes back.
- `r` says no: the plan is dropped and the leader hears `The person said no to your plan: ask them what they want, or make another plan`.
- `n` writes to the leader instead: what you send closes the plan unanswered, and `hire` says so until a new plan has your yes (`the person answered your plan in words, not with a yes: make a new plan from what they said`). Any word of yours to the leader does the same.

A new plan replaces the one waiting. The plan is checked as cards would be: a title of one line, a body, a tier, paths, and no two cards of the plan owning the same path (`cards 1 and 3 both own a path: "src" and "src/a.rs"`); the envelope drives it and the body is never interpreted. One to twenty cards. The first worker needs no plan, nor does a leader hiring with no worker left; a solo never plans. `:plan off` turns the gate off for a project that trusts its leader (kept in the store, on by default); a plan sent then is not kept and the leader is told to hire as it sees fit. The demo's leader plans before its second worker too; the person's yes there comes from `a` as live.

## Cards and their stories

A card is a unit of work with a story: `intent`, `assumption`, `decision`, `talk`, `evidence`, `change`, `you` (what you said or decided). The board view shows cards in columns (WAITING, IN PROGRESS, VERIFYING, DONE); `#N` or `#word` opens a card's own screen with who was on it, its story, and its own inbox and journal lines ([cards](../screen/cards.md)).

## The wake briefing

An agent asleep since the last run wakes on your message, a teammate's waking message or a card posted in this run. Unless its session resumes with its memory intact ([sessions](../concepts/sessions.md)), imece briefs the new session ahead of the first prompt: the card and its last 12 story lines, the prompt that was cut short (not run again), how many messages wait, the worktree's uncommitted changes, and the board's open cards. Every line goes through `clean`; a line is at most 300 characters, the briefing 6000.

## See also

- [Message types](../reference/message-types.md): the reference table.
- [Conversations](../screen/conversations.md): reading an exchange, `[ ]` turns, `%`.
- [Board and code map](../screen/board-and-code-map.md).
- [Cards](../screen/cards.md).
- [Changes](../concepts/changes.md): what a worker's `done` turns into.
