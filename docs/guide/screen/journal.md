---
title: The journal
description: The journal pane - what imece writes down, how lines fold and carry their day, marks, the away digest, where you left off, and opening a conversation from a line.
---

# The journal

The journal is the lower side panel of the salon (told as [the story](#the-story) by default): what happened, newest first, one line per event. Everything meaningful an agent or you did is there; routine assignments are left to the board and the leader's trace. It is kept in the store, so the next `imece` opens with the same journal, and the away digest and the "where you left off" window are built from it.

```text
┌ JOURNAL ────────────────────────────────────────────────┐
│14:07 › you → claude-1: the empty-file check belongs in …│
│14:05 ✓ you applied claude-1's change (3 files) to the checkout, staged; worktree mark 7f3a2c1
│13:58 ! claude-1 finished: 3 files, +41 −6 → waits for you
│13:57 ⇄ claude-1 → lead [ask] ↩: Which parser? There …  +2
│  enter open the conversation · c copy the message · m mark
│13:40✱› you ran `cargo test` → exit 0 · 12 s             │
│─ 2026-10-30                                              │
│18:12 ◆ lead hired claude-1 (default model)               │
└──────────────────────────────────────────────────────────┘
```

## The story

By default the lower side panel does not show the journal line by line: it tells the story, the same lines grouped by card. `J` (or `:story`) switches between the two; the choice is remembered, and the journal's title then says the key back, `JOURNAL · J story`. On a card's screen the panel is always that card's journal.

```text
┌ STORY · J journal ─────────────────────────────────────────┐
│! waiting on you (3): claude-2, claude-3, claude-1 via cl…  │
│#1 Payment retry policy · claude-2 ≡ 4/5                    │
│  14:00 ⇄ lead-shop-api → claude-2 [assign]: #1 Payment re… │
│  14:01 ⇄ claude-4 → claude-2 [ask]: Zone codes an enum?    │
│  14:06 ⇄ claude-1 → claude-2 [ask]: Retry count from config?  no answer · 2 min
│  now ⚑ waits for your yes "WebFetch: https://…" · 4 min    │
│#7 Payment retry policy · claude-3  ✓ done · Δ waits for you│
│  14:05 ⇄ claude-3 → lead-shop-api [done]: Backoff capped…  │
│elsewhere                                                   │
│  14:00 ⚑ copied node_modules into claude-5's worktree  ×12 │
└────────────────────────────────────────────────────────────┘
```

- **Whom you hold up**, on top when anyone: every agent something of which waits for you (an item in WAITING FOR YOU, or a question to you), then every agent asking a teammate already held up, with the one it waits for (`claude-1 via claude-2`). Answering the first frees the rest.
- **A block per card** the journal lately spoke of, the card with the newest line first. Its header: the number and title, its owner, then how far it is (`≡ 4/5` its to-do list, `✓ done`, `Δ waits for you` when a change of that card waits). Under it the card's three newest lines, oldest first: what was said about it, its owner's change, a stop, a failure, an apply. A line belongs to the card its agent was on when it was written, so a worker that moved on to another card does not take its old lines along; something said belongs to the card of whoever asked. An ask that got no reply says so: `no answer yet`, then `no answer · 2 min`. Last, `now` and what the owner does now, its step and how long, while the card is not done.
- **Elsewhere**: lines of no card (the leader hiring, a worktree being set up, an API call retrying), each kind once, its newest line, with how many there were (`×12`). An assignment names its card, so it is told in that card's block.

The story reads at most the 2,000 newest journal lines per frame and shows as many cards as fill the panel; for older ones, `J` and the journal. Lines `:clear` took off are not told (a marked one is), and an archive's lines wait in the archive.

| Key | What it does |
| --- | --- |
| `j` `k`  `gg` `G` | select a card's header or a line of elsewhere |
| `enter` | a card: its screen ([cards](cards.md)) · a line of elsewhere: the journal, at that line, for now (`JOURNAL · esc story`; `esc` or `J` back to the story) |
| `#` | write the selected card's number |
| `J` | the journal |

## What is journaled

Each line has the time, a glyph for its kind, the project's name when several are open, and the event in words.

| Glyph | Lines |
| --- | --- |
| `⇄` | a message between two parties, `from → to [type]: first line`; `↩` after the type when it replies to another message. Your own messages, `you → claude-1: …`, and an agent's closing text to you, `claude-1 answered you: …` |
| `◆` | the leader's acts: a card posted, a worker hired (`lead hired claude-1 (default model)`), a worker that finished and left. Assignments and claims are not shown in the pane (the board and the card's screen have them) |
| `!` | something for you: a worker finished (`claude-1 finished: 3 files, +41 −6 → waits for you`), `:commit`'s suggestion came, the pot runs low |
| `✓` | you applied a change: `you applied claude-1's change (3 files) to the checkout, staged; worktree mark 7f3a2c1` |
| `⚑` | what an agent assumed, and the session's notes: `goes on in the session it had`, `starts a new session, told where it left off`, a model that was not available (`using the default`), the confine level, a worktree moved to your HEAD or kept where it is, a path copied into a worktree, a session that listed no commands |
| `✕` | blocked, stopped (`claude-1 stopped: <why>`), a change that did not apply, a turn cut short when imece closed, a copy that was skipped, no commit message |
| `›` | you: what you said, ran, approved, rejected, allowed or refused, set (`you set the budget to $80.00 (was $50.00)`), opened, closed, renamed, made forget |

Among them:

- **Replied.** A turn's closing text is journaled as `answered you` only when it reaches you: when you asked, or a leader's report once the team is quiet. The closing text of a turn that mail or a board offer started restates what the agent sent, which the journal has already as the message; it stays in the card's story and the trace. Without this rule conversations read as repeating themselves.
- **Changes.** `finished … → waits for you`, `you applied …, staged; worktree mark <sha>`, `… did not apply: <git's words>`, `rejected: claude-1`; after a partial apply the rest comes back as a new `finished`.
- **Sessions.** `goes on in the session it had`, `starts a new session, told where it left off`, `you made claude-1 forget its session`, `stopped: …`, `'s turn was cut short when imece closed; it hears of it when it wakes`, a session that listed no commands of its own, the confine level said at every start that is not strict.
- **Worktrees.** `worktree moved from <sha> to HEAD <sha>`, or kept where it is because it has uncommitted changes or commits of its own.
- **Copies.** `copied .env into claude-1's worktree (1.2 KiB)`, and what was not copied and why (`:copy`); what the checkout's `.worktreeinclude` names in lines of its own (`…, as .worktreeinclude names`), and a line of it not followed with why.
- **Your shell.** `` you ran `cargo test` → exit 0 · 12 s ``, or `stopped by signal 2`, with the time in the largest units that say it (`40 ms`, `2 s`, `1 min 5 s`).
- **The pot.** `you set the budget to …`, and once, when under half an hour is left at the current rate, `at $4.00/h the budget lasts about 25 min more; :budget USD raises it`.
- **Asks.** `you allowed claude-4: npm install left-pad`, `you refused …`.
- **Commit messages.** `:commit`'s suggestion and the model that made it, or why none came.

### Folding and days

Older lines of one conversation fold under its newest: `+2` at the end of a row says two more are under it. A marked line is never folded: it keeps a row for `'` to land on.

A dim `─ 2026-10-30` row stands above the first line of a day other than the line above it; a journal whose newest line is yesterday starts with its date, one all of today has none. Copied and exported text always carries the day, since it leaves the screen. The clock follows the day imece first started (a daylight-saving change inside a world's life shifts it by an hour).

Lines older than 90 days and past the newest 5000 are tidied at a live start; a marked line is never tidied.

## Keys in the journal

| Key | What it does |
| --- | --- |
| `tab` `⇧tab` | to the journal panel (salon → waiting → journal) |
| `↑` `↓` / `j` `k`, `gg` `G`, `^d` `^u` `^f` `^b` | select a line |
| `enter` | open the conversation the line belongs to, with the line selected; an event that is no conversation opens as EVENT |
| `c` / `y` | copy the selected message (its text alone; an event line is copied with its day and time) |
| `m` (`:mark`) | mark the line, again unmark; `✱` shows after its clock |
| `'` | the next marked line after the one at hand, newest first, round to the first; from anywhere else `'` comes to the journal first |
| `#` | the card the selected line names or first mentions, on its screen |

Under the selected line a dim row names its keys: `enter open the conversation · c copy the message · m mark`.

## Marks

`m` marks the line at hand to come back to: the selected line of the open conversation, else the selected journal line (the card's, on a card's screen); `m` again takes the mark away, and the flash says which. With no line at hand (another panel, a focus, the board) the flash says where `m` works. `'` goes to the next marked line, round to the first; with none it says so. A mark is yours, so it outlives the run: it is kept with the line in the store, a load always brings the marked lines however old, and the tidy never lets one go. The demo has no store, so its marks end with the run. Marks do not show in `:export`, and there is no list of all marks: the journal with `'` stands for it.

## While you were away

Half an hour without a key, and the first key you press opens WHILE YOU WERE AWAY · last N min instead of acting: cards merged, decisions that wait for you, blocks, assumptions made, conversations, each counted, then the important lines since. Built from the journal, no model asked. Any key closes it.

Back in the terminal after half an hour or more in another window, the same digest opens at once, without a key: the terminal reports when it loses and regains the focus. Shorter absences show nothing. It does not open over what you are writing, the help, the quit question or another window, and with nothing new in the journal there is nothing to show. A terminal or multiplexer that does not report focus (tmux without `focus-events on`) gets only the key-driven one.

`:away` sets the time for both: `:away 10m`, `:away 2h`, from a minute to a day; `:away off` never opens it by itself. It is remembered in the store. `:digest` opens it whenever you ask: what happened since you last left the window, or since the run began.

## Clearing the screen

`:clear` takes the journal's lines and what WAITING FOR YOU lists off the salon's screen; `:clear journal` and `:clear inbox` take one. It waits ten seconds first, and `u` keeps the screen as it is. Nothing is deleted: the store keeps every line, `:export`, a card's screen and the archives still show them, and the clear itself is kept, so the next start begins clean too. A marked line stays. What still waits for you stays open and counted in the title, `WAITING FOR YOU (0 · +2 hidden)`, and `W` opens the next one, back on the list. `:unclear` (or `:unclear journal`, `:unclear inbox`) brings it all back.

## Where you left off

A live start that remembers an earlier run opens WHERE YOU LEFT OFF · 2 h 10 min ago before anything moves: the pot (`$3.10 of $20.00 spent; every agent sleeps until spoken to`), one line per open project (agents, open cards), the oldest items waiting for you as the panel lists them (six at most, then `… and N more`), the turns the earlier run cut short, and the earlier run's last six journal lines with a day row where the day changes. Any key closes it and does nothing else. With nothing to tell there is no window; the demo never has one.

## Export

`:export` writes the open conversation, else the open card's screen with its journal lines, as a Markdown file. See [conversations](conversations.md) and [cards](cards.md).

## See also

- [Conversations](conversations.md)
- [A card's screen](cards.md)
- [Sessions](../concepts/sessions.md)
- [The next morning](../scenarios/next-morning.md)
- [Keyboard](keyboard.md)
