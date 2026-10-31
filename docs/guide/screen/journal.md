---
title: The journal
description: The journal pane - what imece writes down, how lines fold and carry their day, marks, the away digest, where you left off, and opening a conversation from a line.
---

# The journal

The journal is the lower side panel of the salon: what happened, newest first, one line per event. Everything meaningful an agent or you did is there; routine assignments are left to the board and the leader's trace. It is kept in the store, so the next `imece` opens with the same journal, and the away digest and the "where you left off" window are built from it.

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

## What is journaled

Each line has the time, a glyph for its kind, the project's name when several are open, and the event in words.

| Glyph | Lines |
| --- | --- |
| `⇄` | a message between two parties, `from → to [type]: first line`; `↩` after the type when it replies to another message. Your own messages, `you → claude-1: …`, and an agent's closing text to you, `claude-1 answered you: …` |
| `◆` | the leader's acts: a card posted, a worker hired (`lead hired claude-1 (default model)`), a worker that finished and left. Assignments and claims are not shown in the pane (the board and the card's screen have them) |
| `!` | something for you: a worker finished (`claude-1 finished: 3 files, +41 −6 → waits for you`), `:commit`'s suggestion came, the pot runs low |
| `✓` | you applied a change: `you applied claude-1's change (3 files) to the checkout, staged; worktree mark 7f3a2c1` |
| `⚑` | what an agent assumed, and the session's notes: `goes on in the session it had`, `starts a new session, told where it left off`, a model that was not available (`using the default`), the confine level, a worktree moved to your HEAD or kept where it is, a path copied into a worktree, the repository notes cut at 16 KiB, a session that listed no commands |
| `✕` | blocked, stopped (`claude-1 stopped: <why>`), a change that did not apply, a turn cut short when imece closed, a copy that was skipped, no commit message |
| `›` | you: what you said, ran, approved, rejected, allowed or refused, set (`you set the budget to $80.00 (was $50.00)`), opened, closed, renamed, made forget |

Among them:

- **Replied.** A turn's closing text is journaled as `answered you` only when it reaches you: when you asked, or a leader's report once the team is quiet. The closing text of a turn that mail or a board offer started restates what the agent sent, which the journal has already as the message; it stays in the card's story and the trace. Without this rule conversations read as repeating themselves.
- **Changes.** `finished … → waits for you`, `you applied …, staged; worktree mark <sha>`, `… did not apply: <git's words>`, `rejected: claude-1`; after a partial apply the rest comes back as a new `finished`.
- **Sessions.** `goes on in the session it had`, `starts a new session, told where it left off`, `you made claude-1 forget its session`, `stopped: …`, `'s turn was cut short when imece closed; it hears of it when it wakes`, a session that listed no commands of its own, the confine level said at every start that is not strict, the repository notes cut at 16 KiB.
- **Worktrees.** `worktree moved from <sha> to HEAD <sha>`, or kept where it is because it has uncommitted changes or commits of its own.
- **Copies.** `copied .env into claude-1's worktree (1.2 KiB)`, and what was not copied and why (`:copy`).
- **Your shell.** `` you ran `cargo test` → exit 0 · 12 s ``, or `stopped by signal 2`, with the time in the largest units that say it (`40 ms`, `2 s`, `1 min 5 s`).
- **The pot.** `you set the budget to …`, and once, when under half an hour is left at the current rate, `at $4.00/h the budget lasts about 25 min more; :budget USD raises it`.
- **Asks.** `you allowed codex-2: npm install left-pad`, `you refused …`.
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

Twenty seconds without a key, and the first key you press opens WHILE YOU WERE AWAY · last N min instead of acting: cards merged, decisions that wait for you, blocks, assumptions made, conversations, each counted, then the important lines since. Built from the journal, no model asked. Any key closes it.

Back in the terminal after three minutes or more in another window, the same digest opens at once, without a key: the terminal reports when it loses and regains the focus. Shorter absences show nothing. It does not open over what you are writing, the help, the quit question or another window, and with nothing new in the journal there is nothing to show. A terminal or multiplexer that does not report focus (tmux without `focus-events on`) gets only the key-driven one.

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
