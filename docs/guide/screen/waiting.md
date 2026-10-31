---
title: WAITING FOR YOU
description: The panel that holds every decision only you can take - changes to review, tool calls to allow, questions to answer - and the keys that work through it.
---

# WAITING FOR YOU

WAITING FOR YOU is the top side panel of the salon. Only decisions that need you land there; everything else stays in the journal. You meet it the first time a worker sends `done`: its change (`Δ`) appears, the header counts `! 1 waiting for you`, and the terminal's title says `imece · 1 waiting`.

```text
┌ WAITING FOR YOU (3) ───────────────────────────────────────────────┐
│Δ 13:58 claude-1 finished: parser (3 files, +41 −6) · ✓ ran clean on this tree: cargo test
│  enter see the diff · a apply to the checkout · r reject            │
│⚑ 14:01 codex-2 waits for your yes: Bash: npm install left-pad       │
│? 14:03 later · gemini-3 asks you: Keep the old CLI flag or drop it? │
└─────────────────────────────────────────────────────────────────────┘
```

## What lands there

| Mark | Item | Where it comes from | `a` | `r` |
| --- | --- | --- | --- | --- |
| `Δ` | a change | a worker's `done`: its worktree's whole change against your HEAD, read by imece. The row says the done's first line, the files and lines, and what imece's `run` tool ran on exactly that tree (`✓ ran clean`, `✗ failed`, or `no run on this tree`). | apply it to your checkout, staged | reject it; the composer opens `@worker ` so you say why |
| `⚑` | an ask | a tool call the agent's own vendor would ask permission for: a shell command Claude Code would ask about, a Codex `on-request` ask, a Gemini shell. Routine commands (the build, the tests, git's reading commands) run on their own and never get here. The selected ask is shown whole under its row, its lines joined with ` ↵ `, never read as Markdown: what you allow is what runs. | allow | refuse; the agent is told it was your decision |
| `?` | a question | an agent's `send(to: "you", type: ask)`. A newer question of the same agent takes the older's place. | both open the composer `@agent `; the item closes when your answer goes | |
| `»` | a reply | an agent answered you and waits for you to read it | `enter` reads it (the conversation opens) and closes it; `r` dismisses it | |

The row carries the time, `later ·` when you put it off, and the project's name when several are open. An item of another day carries its date.

A change (`Δ`) can also go back to its worker with notes (`n` in the diff) or be applied in part (`x` then `a`); see [the change view](changes.md). An item keeps the card its agent worked on when it came, so it shows on that [card's screen](cards.md) too.

## Keys

| Key | What it does |
| --- | --- |
| `tab` `⇧tab` | cycle the panels: salon, waiting, journal |
| `w` (`:waiting`) | straight to the waiting panel, from any panel of the salon |
| `↑` `↓` / `j` `k`, `gg` `G` | select an item |
| `enter` | open the item: a change's diff, a question's or reply's conversation, an agent's focus for the rest |
| `a` / `r` | approve / apply / allow · reject / refuse the selected item, as the table above says |
| `W` (`:next`) | from anywhere: open the next item waiting |
| `S` (`:defer`) | put this item off and open the next |
| `esc` | from the diff back to the panel |

Under the selected item a dim row names its keys (`enter see the diff · a apply to the checkout · r reject`; `a allow · r refuse · enter focus` for an ask; `enter read all · r dismiss · or just write back` for a reply).

## `W` and `S`: one after the other

`W` works on every screen: the board, the code map, a card's screen, a diff, a conversation, a raw stream. It closes what is open over the salon, selects the next open item in the panel and opens it as `enter` would, so `a` and `r` answer it at once. Pressed again it goes on to the item after, and round to the first after the last. With nothing waiting it says `Nothing needs you right now` and nothing moves.

`S` puts the item at hand off and opens the next one. Put-off items come after all the others, in the order they were put off, and their rows say `later ·` up front. Putting off one already put off moves it to the very end. The list forgets items that close, and it lasts the run only: a fresh start shows everything in age order again.

"At hand" is the item `W` last opened; when you move the panel's selection yourself (`j` `k`, `gg` `G`), that is what is at hand, and `W` goes on from there. "Next" is by the item's place before it was put off, so `S` moves on to what came after it rather than back to the top.

## The `!` on a row

An agent whose item waits shows it on its own salon row: `? → you` while a question waits, `⚑ waits for your yes` while an ask does, `answered you` while a reply does; an agent with the status "waiting for you" wears `!` as its glyph. The project header counts them in the attention colour, `! 2 waiting`, even when the project is folded. `f` in the salon shows only the agents that need you, these first.

## Quitting with items open

`q` asks before quitting and counts what quitting now would leave: `quit imece? agents mid-turn: 2, changes not reviewed: 1 · q again quits · s shows them · any other key stays`. Agents mid-turn are the ones spending tokens; changes not reviewed are the open `Δ` items. Asks and questions are not counted here: they are in the title and the header already, and quitting cuts none of them. `q` `s` (`:pending`) opens BEFORE YOU GO with the agents mid-turn, each with what its row says it does, and the changes as the panel shows them; there `q` quits at once, `W` opens the next item waiting, any other key goes back. With nothing in flight the question is the plain one.

## See also

- [Approvals and questions](../concepts/approvals-and-questions.md)
- [Changes](../concepts/changes.md)
- [The change view](changes.md)
- [The salon](salon.md)
- [Keyboard](keyboard.md)
