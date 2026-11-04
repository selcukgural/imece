---
title: The timeline
description: View 4 - one lane per agent over the last hour, what each did in every column, the messages under each project, the scale and the keys.
---

# The timeline

The timeline shows the team over time: one lane per agent of the open projects, time going right to now, each column what the agent was doing then. What works side by side, who sat idle, where something got stuck and since when, read at a glance. `4` (`:timeline`) opens it in the salon's place; `1` comes back. It is at its best with a small team; with many agents the lanes scroll.

```text
┌ TIMELINE · a column is 5 s · [ ] scale ─────────────────────────────────────────────┐
│shop-api            14:00                    14:04                     now           │
│◆ lead-shop-api     ··········································  coordinating         │
│! claude-1          ◌◌◌░░✕✕✕✕✕✕✕✕✕✕✕✕✕✕⚑⚑⚑⚑⚑⚑⚑⚑⚑⚑⚑⚑⚑⚑⚑⚑⚑⚑⚑⚑  waiting for you      │
│● claude-2          ◌◌◌░░░░░░░▒▒▒▒▒▒▒▒▒▒░░░░▒▒▒⚑░░░░░░▒▒▒▒▒▒▒██  ▸ Run the tests      │
│●   claude-2.1                       ▒▒▒▒████▒▒✓·············  idle                │
│! claude-3          ·◌◌◌░░⚑⚑⚑⚑⚑⚑▒▒▒▒▒░░░░???░░░░░░░░▒▒▒▒⚑⚑░░?  ▓░░░░ ? → claude-2   │
│  messages          ▁▁▂▁▂▁▁▁▁▁▁▁▁▁▁▂▁▁▂▁▁▁▁▂▂▁▁▁▁▁▁▁▂▁▁▁▁▁▁▂▁                        │
│ █ writing  ▒ reading, running  ░ thinking  ◌ starting  ? asking a teammate  ⚑ waiting for you  ✕ stuck  ✓ turn done  · idle
└─────────────────────────────────────────────────────────────────────────────────────┘
```

Each project starts with its name and the time of its first column, its middle one and `now`. Then its leader, its workers and each worker's helpers (indented), each row the agent's state glyph and name, its lane, and what its salon row says now. Under the project, `messages`: how many messages its agents sent each other in each column, on the activity bar's scale (`▁` none, `▂` one, `▃` two, up to `█`).

## What a column says

| Glyph | The agent was |
| --- | --- |
| `█` | writing files (an edit, a write, a patch) |
| `▒` | reading, searching or running something, or researching |
| `░` | thinking: in a turn, no tool call in hand |
| `◌` | starting: its session coming up |
| `?` | asking a teammate and waiting for the answer |
| `⚑` | waiting for you: a tool call waiting for your yes, a question to you, or, out of a turn, something of its in WAITING FOR YOU |
| `✕` | stuck: blocked, or its session stopped |
| `✓` | the tick a turn ended |
| `·` | idle |
| (blank) | nothing known: before imece first saw it |

A column shows the most of what its ticks saw, in that order from the bottom of the table up: a short edit in a column of reading shows as `█`, a column with a stop in it as `✕`. While an agent works, its lane says what it does even when an older change of its waits for you; the salon's `!` still says that.

## Scale and keys

A column is a tick, 5 seconds; `]` makes it 30 seconds, then a minute, `[` back. The lanes keep the last hour; they live in memory only, so a new run starts them empty.

| Key | What it does |
| --- | --- |
| `4`  `:timeline` | open the timeline |
| `[`  `]` | a column of 5 s · 30 s · 1 min |
| `j` `k`  `gg` `G` | select an agent; the selected row is reversed and kept in view |
| `enter` | the selected agent's focus; `esc` comes back |
| `K` `t` `@` | peek, its raw stream, write to it: the selected agent is the one at hand |

The demo fills the lanes the same way, so `imece --demo` shows the timeline as a live run does. See [the decision](https://github.com/selcukgural/imece/blob/main/docs/decisions/100-timeline.md).
