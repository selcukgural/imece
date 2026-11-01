---
title: "Coming back"
description: "Quitting with agents mid-turn and starting again the next day: the left-off window, agents asleep, how each vendor resumes, the wake briefing, forgetting a session, the away digest, a word in another window, marks and export."
---

# Coming back

imece remembers between runs: projects, agents, cards, the journal, what waits for you and the pot live in one SQLite file, and every agent sleeps until spoken to. You meet this the second time you start imece in a checkout. This scenario quits in the evening with two agents mid-turn and comes back the next morning.

## 1. Quitting mid-turn

Press `q`. When something is in flight the question counts it:

```text
 quit imece? agents mid-turn: 2, changes not reviewed: 1 · q again quits · s shows them · any other key stays
```

Press `s` to see them:

```text
 BEFORE YOU GO
 Mid-turn: quitting cuts these turns short
    claude-1   editing src/parser.rs
    codex-1    running cargo test
 Changes not reviewed
    Δ 18:40 gemini-1 finished: config loader (2 files, +30 −4)
 q quits · W opens the next waiting · any other key: back
```

`q` quits at once; `W` opens the next waiting item instead (review the change first, then quit); any other key goes back. With nothing in flight `q s` opens nothing and the question is the plain one. `:quit` (`:q`) asks the same way, `:pending` opens the list.

Press `q`. The sessions end; the prompts of the turns that were under way are kept as cut-short, not to be run again. Everything else is saved; the state is written after every step anyway, so a crash loses little.

## 2. The next `imece`

```sh
cd ~/code/parser
imece
```

Before anything moves, one window says where you left off:

```text
 WHERE YOU LEFT OFF · 14 h 20 min ago
 $12.40 of $50.00 spent; every agent sleeps until spoken to
 parser: 4 agents, 2 open cards
 1 wait for you:
   Δ 18:40 gemini-1 finished: config loader (2 files, +30 −4)
 turns cut short when imece closed: claude-1, codex-1
 the last lines before:
   ─ 2026-10-31
   18:40 ! gemini-1 finished: 2 files, +30 −4 → waits for you
   18:41 › you ran `cargo test` → exit 0 · 12 s
 any key: close
```

The pot is where it was: `--budget` unset keeps it, `--budget 80` would set it. Any key closes the window and does nothing else. It opens only when there is something to tell; the demo never has one.

The journal has a line for each cut turn, and the salon shows everyone asleep:

```text
 09:00 ✕ claude-1's turn was cut short when imece closed; it hears of it when it wakes
 09:00 ✕ codex-1's turn was cut short when imece closed; it hears of it when it wakes
```

```text
   ◦ claude-1   #3   parser: nested lists   ◦ asleep since the last run, write to it to wake it
   ◦ codex-1    #5   config: defaults       ◦ asleep since the last run, write to it to wake it
   ◦ gemini-1   #4   config loader          ◦ asleep since the last run, write to it to wake it
```

Asleep means no process and no cost. Three things wake an agent: you write to it, a teammate's message of a waking type arrives, or a card posted in this run is offered to it. Mail and cards that waited through the restart wake nobody. A journal line older than today carries a `─ YYYY-MM-DD` row above it, and so does an item in WAITING FOR YOU of another day.

## 3. Writing wakes it: how each vendor resumes

Type `@claude-1 go on` and `enter`. imece looks for the session the agent had and resumes it when it can; the journal says which way it went.

**Claude Code.** The remembered session id must be a UUID and its transcript must still be where Claude Code keeps it for the worktree (`~/.claude/projects/<the worktree path with every non-alphanumeric as ->/<id>.jsonl`). Then the session starts with `--resume ID`, and:

```text
 09:02 ⚑ claude-1 goes on in the session it had, with all it knew
```

A resumed Claude session reports the whole session's cost, so imece starts counting at what it had already booked (the cost floor) and books only what lies above it. The briefing is short, since the session knows its story: "imece started again since your last turn; this session goes on where it was, so you remember everything up to then. Since then:" then the cut turn's prompt ("A turn of yours was cut short when imece closed, before it ended; it is not run again. Its prompt was: …") and how many messages wait, or "Nothing; go on."

**Codex.** Threads are not ephemeral, so Codex keeps the rollout under `~/.codex/sessions/`; the start sends `thread/resume` with the thread id, and the answer must carry that id. A thread whose rollout is gone is refused by Codex before any turn. A resumed Codex thread keeps the instructions it was started with; what changed since (a rename, new teammates) reaches it only through imece's notes.

**Gemini.** `session/load` with the session id replays the chat Gemini kept under `~/.gemini/tmp/`; the history streams back before the first turn (at most 50 messages, so a long session resumes with its tail).

**When a resume is not possible** (no transcript, a rollout gone, an id of another shape, or `--fresh`), the agent starts a new session and is told where it left off:

```text
 09:02 ⚑ codex-1 starts a new session, told where it left off
```

The wake briefing goes ahead of the first prompt as a note from imece: "imece started again since your last turn, so this is a new session that remembers nothing of the old one. Where you left off:", then its card ("Your task: #5 config: defaults (not finished). Its story, latest last:" with the last 12 story lines), the cut turn's prompt, how many messages wait, its worktree's uncommitted changes ("Your worktree holds changes the person has not applied: 2 files, +12 −3: src/a.rs, src/b.rs"), and the board's open cards. Every line is cleaned and cut (300 characters a line, 6000 in all). A worker told again goes on with its unfinished card.

Either way the worktree is checked before the first prompt: a clean one is moved to your HEAD (`worktree moved from … to HEAD …`), one with changes stays and the journal says so.

## 4. Forgetting a session on purpose

A session that has gone wrong (or grown long) can be dropped. `:forget @claude-1` (`:f`, completion offers the agents):

```text
 claude-1 forgot its session; the next message starts a new one, told where it left off
```

```text
 09:30 › you made claude-1 forget its session; it starts a new one, told where it left off
```

The running session ends (its run killed, unfinished prompts kept as cut-short), and what the vendor kept of it is removed on a thread of imece's own: Claude's transcript file, Codex's thread (`thread/delete`), Gemini's chat files. The agent sleeps until spoken to; its next message starts a fresh session with the full briefing (`Restarted`). The `:` menu marks `:forget` with `$`: the new session pays the whole briefing again, without the cache. An agent with nothing to forget is refused (`claude-1 keeps its session: …`). `imece --fresh` does the same for every remembered agent at the start.

## 5. The away digest

Switch to another window for a while. When the terminal gets the focus back after three minutes or more, the digest of what happened opens at once, without a key:

```text
 WHILE YOU WERE AWAY · last 12 min
  ✓ 0 cards merged
  ! 1 decisions wait for you
  ✕ 0 blocks
  ⚑ 1 assumptions made
  ⇄ 4 conversations
  09:33 ⚑ codex-1: worktree moved from 3f2a9c1 to HEAD 9b1d2e0
  09:41 ! codex-1 finished: 1 files, +18 −2 → waits for you
 any key: close
```

The counts are the new journal lines by kind and what waits for you now; under them the important lines since (applies, changes ready, blocks, failures, assumptions; the last eight). It is built from the journal since the focus went, no model asked. It does not open over what you are writing, the help, the quit question or another window; a shorter absence shows nothing, and the first key after twenty seconds without one still brings the key-driven digest. The terminal must report focus (tmux needs `focus-events on`).

The digest tells you when you are back; `:notify` fetches you. `:notify bell` rings the terminal's bell, `:notify text` sends a desktop notification, when something came to wait for you while you were in another window and is still there five seconds later: a change, a question, an ask for your yes, an agent stuck, each once. What was on screen when you left stays quiet, and so does whatever came and went within the wait.

```text
 imece · codex-1 finished: a change waits for you
```

Several at once are one bell, or one note with the count. `:notify` bare goes round: off, bell, text, off; the choice is remembered. It needs the same focus reports as the digest, and a terminal that knows OSC 9 for the text (iTerm2, WezTerm, Ghostty; elsewhere `bell`).

## 6. Keep a place with marks

In the journal (or an open conversation) select a line you will want again, the message with the plan say, and press `m`: `marked: ' comes back to it`. The line wears `✱` after its clock. `'` selects the next marked line, round to the first: in the open conversation, else in the journal; from anywhere else it goes to the salon's journal first. `m` again takes the mark away (`mark taken away`).

```text
 09:41✱ ⇄ lead → codex-1 [assign]: #5 config: defaults, src/config.rs
```

Marks are yours: kept in the store between runs, never folded under a newer line, and kept by the retention tidy however old (the journal otherwise keeps 90 days past its newest 5000 lines).

## 7. Take a conversation with you

Open the conversation with `v` on `codex-1`'s row (its talk with you) or `enter` on a journal line (two agents' talk), then `:export`:

```text
 written to /Users/you/code/parser/imece-conversation-20261101-0945.md
```

The file is `# Conversation: lead ⇄ codex-1`, each message a heading with its day and time, who to whom and its kind (`### 2026-11-01 09:41 codex-1 → lead [done]`) over its text. With a card's screen open (`#5`) instead, the file is the card: its WHO, STORY, what waits on it and its whole journal, unfolded. `:export ~/notes/` names a file in that directory; `:export ~/notes/config.md` names the file. Nothing is ever written over (`could not write …`). Every line is cleaned as the screen cleans it; the marks do not show in the file.

## See also

- [Sessions](../concepts/sessions.md): the store, resume per vendor, hygiene
- [The journal](../screen/journal.md): days, marks, retention
- [Files](../reference/files.md): where the store and the vendors' files live
- [A failing test, one worker](one-fix.md)
