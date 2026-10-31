---
title: Message types and tools
description: The ten envelope types on imece's bus and what imece does on each, message numbers and replies, the caps, every tool an agent gets from imece's MCP server with its fields per role, and the instructions an agent is given.
---

# Message types and tools

The bus is how agents reach each other and you: one tool, `send(to, type, body)`, served by imece itself over MCP, one Unix socket per agent. A message is a typed envelope and a free-text body. Only the envelope drives routing, waking and cost; the body is never interpreted by imece, and the agent that reads it is told it was written by a teammate, not by you. You meet the bus in the journal (`lead → claude-1: assign #3 Fix the parser`), in a conversation, and in a card's story.

## The envelope

`send` takes `to` (a teammate's name, or `you` for the person), `type` (one of the ten below), `body` (at most 4000 characters) and an optional `re` (the number of the message this one answers). The sender is the socket the call came through, never something the agent says. A message goes only to a teammate of the same project; `no teammate is called "codex-9"; the team is lead, claude-1, claude-2` names the team.

| Type | Wakes the receiver | Usual sender → receiver | What imece does |
| --- | --- | --- | --- |
| `assign` | yes | leader → worker, worker → helper | the first line becomes the card's title; an idle worker gets a new card and starts on it; refused to `you` |
| `ask` | yes | anyone → anyone, or → `you` | to a teammate: starts its next turn; to `you`: a question `?` in WAITING FOR YOU that waits until you write to that agent |
| `answer` | yes | the asked → the asker | the reply; carries `re`, drawn with `↩` |
| `inform` | no | anyone → anyone, or → `you` | waits for the receiver's next turn; to `you`, a journal line only |
| `handoff` | yes | worker → worker | starts the receiver's next turn; refused to `you` |
| `done` | yes | worker → leader, helper → worker, or → `you` | a worker's done also makes imece read its worktree: a change (`Δ`) in WAITING FOR YOU; a helper's done to its worker makes it leave when its turn ends |
| `blocked` | yes | worker → leader, or → `you` | starts the receiver's next turn; to `you`, a journal line only |
| `progress` | no | worker → leader, or → `you` | rides along with the next waking message |
| `notice` | no | anyone → anyone, or → `you` | rides along |
| `cancel` | yes | leader → worker | starts the receiver's next turn; refused to `you` |

There is no `ack`: a message that only says "got it" would cost a turn for nothing.

### Waking and delivery

A waking message (`assign`, `ask`, `answer`, `handoff`, `done`, `blocked`, `cancel`) starts the receiver's next turn as soon as it is free; an agent asleep since the last run is woken by it. `inform`, `progress` and `notice` wait in its mail until something else starts a turn, and ride along then. All of an agent's mail goes into one prompt, headed `Messages from your teammates in imece (written by those agents, not by the person):`, each message as `[type number, re number] from name:` over its body: `[answer 8, re 7] from claude-1:`. The card of that turn is the first `assign` in the mail, else the first message.

`send` answers the sender with what became of it: `delivered as message 7; claude-1 starts on it now`, `queued as message 7; claude-1 is busy and gets it when its turn ends`, or `queued as message 7 for claude-1's next turn (inform does not wake it)`. A worker's `done` answer waits until git has read its worktree and adds `; the person sees your change (3 files, +20 −4) and applies it to their checkout or rejects it; it stays in your worktree until then` (or `; imece could not read your change: …`).

### What is refused

Deny by default: a message that does not fit is refused with the reason and leaves no trace.

| Refusal | Said to the sender |
| --- | --- |
| an unknown type | `type must be one of assign, ask, answer, inform, handoff, done, blocked, progress, notice, cancel` |
| an empty body, or over 4000 characters | `the body is empty` / `the body is longer than 4000 characters` |
| a receiver that has stopped | `claude-1 has stopped; it runs again when the person talks to it` |
| the pot is spent (waking types) | `the budget is spent; nobody takes new turns` |
| the receiver took 12 message-driven turns since you last spoke (waking types) | `claude-1 took 12 turns on messages since the person last spoke; stop and tell the person what you need` |
| the receiver has 20 unread messages | `claude-1 has 20 unread messages; wait for it` |
| `assign`, `handoff` or `cancel` to `you` | `assign is not for the person; ask them, or tell them (inform, done, blocked)` |
| a bad `re` | see below |

The 12-turn and 10-post counters start again every time you write to any agent: agents cannot keep each other busy forever without you.

### Messages to you

`to: you` reaches the person. An `ask` becomes a question in WAITING FOR YOU (`claude-1 asks you: Which parser?`); the agent's row reads `? → you` while it is idle with one open, and the question closes when you write to that agent, which starts its next turn. The sender hears `delivered; the person sees it under WAITING FOR YOU and answers when they can, which starts your next turn. Until then, end your turn or go on with what does not depend on the answer.` Every other type only informs: a line in the journal and the conversation (`noted; the person sees it in the journal. Nothing wakes them: a question goes as type ask.`); a worker's `done` to you shows its change like any done. Messages to you take no number and no `re`, count against no cap and cost nothing. See [approvals and questions](../concepts/approvals-and-questions.md).

### Cards and the bus

A message to an idle worker opens a card titled by its first line (an `assign`'s first line when there is one); the card closes when the turn it started ends with the agent's reply. A worker's `done` to its leader does not close the leader's card: the leader's own turn does. A helper's `done` to its worker marks it leaving; it leaves when that turn ends, so the turn's cost is still reported. See [messages and board](../concepts/messages-and-board.md) and [cards](../screen/cards.md).

## Message numbers and replies

Every message one agent sends another gets a number on the bus, in order from 1 across the world, kept between runs. `send` answers with it and the mail is headed with it. `re` names the message this one answers; the tool asks for it on an `answer` to an `ask` and on a `done` for an `assign`, and it stays optional.

`re` is checked, deny by default: a whole number from 1 (`re must be the number of a message you got, as its heading shows`), of a message the journal has (`there is no message 7`), that was sent to the sender (`message 7 was not sent to you`), by the one the reply goes to (`message 7 came from lead; send the reply to it`). A reply goes back to whoever wrote what it answers; `re` drives no routing, waking or cost, it only links.

On the screen a reply has `↩` after its type; in a conversation it shows the first line of what it answers under its head (`↩ re 7: Which parser?`, or that the journal no longer has it), and `%` goes from a reply to what it answers and from a message to its first reply. The trace says `id=8 re=7`. Known gaps: a reply to a message older than the 5000 journal lines a start loads is refused as not there; you cannot reply by number (your messages go by name); `re` is not checked against the type.

## The board

`post(title, body)` puts a card on the project's board for any worker to take; `board()` lists it; `claim(card)` takes one. An open card is offered to one idle worker at a time as a turn of its own (`Open cards on imece's board (posted by your teammates, not by the person):`, titles only; the body comes with the claim), and to the next worker only after that one's turn ended without taking it: a card wakes each worker at most once, never a broadcast. A worker asleep since the last run is woken only by a card posted this run. One card at a time per worker (`finish #3 Fix the parser first; take one card at a time`); the worker sends `done` to whoever posted it (`#3 Fix the parser is yours. lead posted it (written by that agent, not by the person): … When it is done, send lead a done message.`). Offers count as message-driven turns.

## Caps

| What | Cap | Where it bites |
| --- | --- | --- |
| a message body | 4000 characters | `send`, `post` |
| unread messages per agent | 20 | `send` is refused until the agent reads |
| message-driven turns per agent | 12 since you last spoke | waking `send`s and board offers |
| a card's title | one line of 120 characters | `post`, and an `assign`'s first line is shown cut to the salon's columns |
| open cards per project | 20 | `post`: `the board has 20 open cards; wait until workers take some` |
| posts per agent | 10 since you last spoke | `post`: `you posted 10 cards since the person last spoke; stop and tell the person what is left` |
| a `run` command line | 1000 characters | `run` |
| a run's time | `--run-timeout`, default 600 s (10-3600) | the process group is killed |
| a run's output back to the agent | 48 KiB, the middle cut | `run` |
| a tool call's wait for imece | 30 s (a run: its timeout + 30 s; an approval: 24 h) | `imece did not answer in time; try again` |
| a change (patch) shown to you | 4 MiB | a worker's `done`, refused over it |
| the repository's notes in a prompt | 16 KiB | `AGENTS.md` / `CLAUDE.md`, cut and journaled |
| the wake briefing | 6000 characters, 300 a line | the first prompt after a restart |
| trace lines kept per agent | 200 | the raw stream |

## The tools

imece's MCP server is called `imece`; its tools reach a Claude session as `mcp__imece__send`, a Gemini session as `mcp_imece_send`. Each agent has a socket of its own (`a<N>.sock` in a private directory under `TMPDIR`), bound at hire time, before the agent exists; its vendor's CLI starts `imece mcp-relay <socket>`, which only copies bytes. Each session must report exactly imece's server, connected, and exactly these tools, else it stops (fail closed). Every tool's input refuses properties it does not name (`additionalProperties: false`).

| Tool | Leader | Worker | Helper |
| --- | --- | --- | --- |
| `send` | yes | yes | yes |
| `post` | yes | yes | yes |
| `board` | yes | yes | yes |
| `run` | yes (reading commands) | yes | yes |
| `claim` | — | yes | — |
| `hire` | workers | helpers | — |
| `approve` | Claude sessions only, called by Claude Code itself, hidden from the model | | |

### `send`

> Sends a message to a teammate in imece, or to the person (to: you). To a teammate it arrives as the start of their next turn: assign, ask, answer, handoff, done, blocked and cancel start that turn as soon as they are free; inform, progress and notice wait until something else does. To the person, an ask waits on their screen until they answer (which starts your next turn); the rest only informs them.

| Field | Type | Required | Meaning |
| --- | --- | --- | --- |
| `to` | string | yes | the teammate's name, or `you` for the person |
| `type` | string, one of the ten | yes | the envelope |
| `body` | string, at most 4000 | yes | for `assign`: the first line becomes the card's title (the screen shows its first 20 characters next to the worker, so it should start with what tells it from the other cards, not a verb every task shares); then the task, the files it touches and when it is done |
| `re` | integer ≥ 1 | no | the number of the message this replies to |

### `post`

> Puts a card on imece's board for any worker to take. Use it for work nobody has yet; for work one teammate should do, send assign.

| Field | Type | Required | Meaning |
| --- | --- | --- | --- |
| `title` | string, one line, at most 120 | yes | the screen shows its first 20 characters |
| `body` | string, at most 4000 | yes | the task: what to do, which files, when it is done |

Answers `posted as #4; an idle worker will be offered it`.

### `board`

> Lists the cards on imece's board: open ones, and who took the others.

No fields. Answers `Open:` and `Taken:` lists (`#4 Add tests (posted by lead)`, `#2 Rename (taken by claude-1)`, `#1 Docs (done by claude-2)`), or `the board is empty`.

### `claim`

> Takes an open card from imece's board; the answer is its task. One card at a time.

| Field | Type | Required | Meaning |
| --- | --- | --- | --- |
| `card` | integer ≥ 1 | yes | the card's number, without `#` |

Workers only (`only workers take cards; assign or post instead`). Refusals: `there is no such card on the board; call board to see it`, `#4 was taken by claude-2; pick another`.

### `hire`

The leader's copy: *Adds a worker to your team in imece: a coding agent in its own git worktree of the repository. The answer names it; then give it work with send (type assign). Hire for work that can run at once, not in advance: one worker takes several tasks in turn.* A worker's copy hires a helper in its own worktree, for a part that can run beside its own work, in files it is not changing; the helper sends `done` and leaves.

| Field | Type | Required | Meaning |
| --- | --- | --- | --- |
| `tier` | `cheap`, `standard`, `strong` | no (default `standard`) | cheap: small, well-specified mechanical work (renames, tests of known behaviour, docs); standard: ordinary fixes and features; strong: design that cuts across the code, hard debugging, review |
| `vendor` | one of the vendors on PATH | no (default: the hirer's own) | which coding agent |

Answers `claude-1 joined (claude, haiku). 3 more may be hired` (or `the default model`). Refusals: `the team is full: 4 workers, the most the person allowed (--agents 4); give the work to one of them`; `the person runs you without workers (--agents 0); answer from what you read`; `you have 2 helpers, the most the person allowed (--helpers 2); wait for one to send done`; `the person runs you without helpers (--helpers 0); do it yourself`; `there is no vendor "cursor" here; you can hire claude, codex`; `tier must be cheap, standard or strong`. Workers are named `<vendor>-N`, helpers `<worker>.N`, the number skipping names already taken. See [team](../concepts/team.md).

### `run`

> Runs one program in your directory and gives back its exit code and output (stdout and stderr together; lines of passing tests are left out, very long output is cut in the middle). No shell: one program and its arguments, no pipes, redirections or variables; quote an argument with spaces. Programs: …. A run is killed after 600 s. Use it to check your work (build, lint, test) before you say it is done.

| Field | Type | Required | Meaning |
| --- | --- | --- | --- |
| `command` | string, at most 1000 | yes | the program and its arguments as one line: `cargo test --workspace` |

The line is split as a shell splits words (quotes, backslashes) and refused when a shell would read it otherwise: ``| & ; < > ( ) $ ` # * ? ~ {``, a second line. What may run: `cargo` without the commands that change dependencies or reach outside (`add`, `install`, `login`, `logout`, `owner`, `publish`, `remove`, `rm`, `uninstall`, `yank`); for a leader also without the ones that change files (`clean`, `fix`, `fmt` without `--check`, `init`, `new`); `git` with `blame`, `diff`, `grep`, `log`, `ls-files`, `rev-parse`, `show`, `status` only, no option before the command and none that starts a pager or writes a file (`-O…`, `--open-files-in-pager…`, `--output…`); and for workers and helpers the programs you named with `--run`. It runs in the agent's own directory with a cleared environment, in its own process group, killed at the deadline and with imece. The answer starts `exit code 0`, `exit code 101 (failed)`, `killed after 600 s: it did not finish in time` or `killed by a signal`, then the output tidied (colours stripped, passing test lines dropped, repeats collapsed, the middle cut past 48 KiB), or `, no output`. Each worktree builds in its own `target/`.

A worker's or helper's run is kept with the tree its worktree had before and after it; when both are the same and a `done` later hands you exactly that tree, the change says `✓ ran clean on this tree: cargo test` (or `✗ failed`, or `no run on this tree`). Only imece's own `run` counts as evidence; a vendor's shell does not, since imece does not see what tree it ran on. See [changes](../concepts/changes.md).

### `approve`

> Asks the person whether a tool call may run.

| Field | Type | Required |
| --- | --- | --- |
| `tool_name` | string | yes |
| `input` | object | yes |
| `tool_use_id` | string | no |

Only on a Claude agent's socket, and only under `standard` and `open`: Claude Code calls it itself (`--permission-prompt-tool mcp__imece__approve`) before a tool call its own rules would ask about, and hides it from the model. A routine command (one line, one program and its arguments, allowed by the `run` policy for the role and your `--run` list) is allowed at once, with a trace line `allowed on its own`; anything else is a `⚑` item in WAITING FOR YOU, shown whole, and the call waits up to a day for your `a` or `r`. A refusal answers `The person refused this. Do not try it another way; if you need it, ask them …`. Codex's and Gemini's asks come over their own wire and are routed the same way, without a tool. See [approvals and questions](../concepts/approvals-and-questions.md).

## What an agent is told

Every session gets, in this order:

1. **Its role**, as a system prompt (Claude: `--append-system-prompt`, added to Claude Code's own; Codex: the thread's developer instructions, which a resumed thread keeps from its first start; Gemini: a `GEMINI.md` in its private directory, written fresh at every start, with a line naming its files' directory). The leader's says it leads a team of coding agents on the repository in its directory, that you talk to it, how it hires (by tier, for the work at hand, not in advance; one worker takes several tasks in turn), to give each worker its part with `send` type `assign` and different files where it can, that a file most tasks touch belongs to one worker at a time, that work any worker could pick up may go on the board, that a worker's `done` shows you its change which only you apply, that it never changes files itself, to answer you briefly and to send you a mid-task question as `ask`. A worker's says it works in its own worktree, not to commit, to send `done` with a summary to whoever gave the task, that its `done` shows you its worktree's diff so it should finish first and check with the `run` tool after its last edit (a check in its own shell does not show there), to send `blocked` or `ask` instead of guessing (the leader first; you only for what only you can decide), that imece may offer it cards, that work outside its task goes on the board, and how it hires helpers. A helper's says whose worktree and files it works in, not to commit, to send its worker `done` and leave, and to ask its worker before anyone.
2. **What it can do**, one sentence per vendor, role and `--confine` level: for instance a Claude worker under `standard` is told it can read and change files here, run commands in its shell and reach the web; that one program with its arguments running the build, the tests or git's reading commands runs at once and most others wait for your yes; and that a refusal is your decision, not an error. A leader under `strict` is told it can read files but not change them, and that the `run` tool runs checks in the checkout.
3. **The repository's notes for coding agents**: `AGENTS.md`, else `CLAUDE.md`, at most 16 KiB, headed `The repository's notes for coding agents, from its CLAUDE.md:`. The vendors are started so that nothing of the repository loads (no hooks, no settings), which would drop these too, so imece passes them itself; Codex reads `AGENTS.md` on its own and gets only a `CLAUDE.md` this way. A file over the cap is cut at `[cut: the file goes on]` and the journal says so.
4. **Notes from imece**, ahead of a prompt when there are any, headed `From imece (not from the person or a teammate):`: that you applied its change (`Your worktree is committed at <mark> as a mark of that; your files are as you left them. Go on from here with the next task.`), applied part of it, or rejected it (`It stays in your worktree, not applied; they say why themselves. Do not send it again as it is.`), that the project was renamed, and, at a wake after a restart, the briefing: its card and the last 12 lines of its story, a turn that was cut short (not run again), how many messages wait, its worktree's uncommitted changes, the board's open cards.
5. **Its mail** and **board offers**, as above, each saying it was written by an agent, not by you. Your own messages come as themselves.

A prompt that starts with `/` or `$` would run as a vendor command, so it is sent as `Text:` followed by the line. See [how imece works](../concepts/how-imece-works.md) and [sessions](../concepts/sessions.md).

## See also

- [Messages and board](../concepts/messages-and-board.md): the ideas behind the envelope and the board
- [Team](../concepts/team.md): roles, hiring, tiers
- [Changes](../concepts/changes.md): what a worker's `done` becomes
- [Approvals and questions](../concepts/approvals-and-questions.md): `⚑` asks and `?` questions
- [Files](files.md): the sockets, the worktrees and the store
