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

- An `ask` to you becomes a question in WAITING FOR YOU (`?`, "claude-1 asks you: …"). The agent's salon line reads `? → you` while it is idle with one open. A newer question from the same agent takes the older's place. You answer by writing to the agent (`@claude-1 …`, or `a`/`r` on the item, which open the composer), and that starts its next turn.
- `inform`, `progress`, `notice`, `done`, `blocked` and `answer` to you only inform: a line in the conversation and the story, no item, nothing wakes. A worker's `done` to you shows its change like any `done`.
- `assign`, `handoff` and `cancel` to you are refused.
- Messages to you take no number and no `re`; your own messages have none. Nothing to you counts against the mail caps or the pot.

The prompts say when to ask: the leader may ask you mid-task; a worker asks its leader first and sends you only what only you can decide; a helper only what no teammate can answer. More on [approvals and questions](../concepts/approvals-and-questions.md).

## You on the bus

Plain text in the command line goes to the selected project's leader; `@name text` goes to one agent. A message to an idle worker opens a card; its answer closes it. Your messages have no number. An agent's answer to you waits in WAITING FOR YOU ("claude-1 answered you: …") until you open it (`enter`), dismiss it (`r`) or just write back.

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

## Cards and their stories

A card is a unit of work with a story: `intent`, `assumption`, `decision`, `talk`, `evidence`, `change`, `you` (what you said or decided). The board view shows cards in columns (WAITING, IN PROGRESS, VERIFYING, DONE); `#N` or `#word` opens a card's own screen with who was on it, its story, and its own inbox and journal lines ([cards](../screen/cards.md)).

## The wake briefing

An agent asleep since the last run wakes on your message, a teammate's waking message or a card posted in this run. Unless its vendor session resumes with its memory intact ([sessions](../concepts/sessions.md)), imece briefs the new session ahead of the first prompt: the card and its last 12 story lines, the prompt that was cut short (not run again), how many messages wait, the worktree's uncommitted changes, and the board's open cards. Every line goes through `clean`; a line is at most 300 characters, the briefing 6000.

## See also

- [Message types](../reference/message-types.md): the reference table.
- [Conversations](../screen/conversations.md): reading an exchange, `[ ]` turns, `%`.
- [Board and code map](../screen/board-and-code-map.md).
- [Cards](../screen/cards.md).
- [Changes](../concepts/changes.md): what a worker's `done` turns into.
