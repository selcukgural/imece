---
title: Home
description: View 5 - what is your turn across projects, what is stuck and what is at work, in one list with what is at hand read beside it.
---

# Home

Home is the screen imece opens on, the one for deciding; `5` or `:home` comes back to it. The salon shows the team; Home shows what is your turn. One list across the open projects, and beside it whatever row is at hand, read without pressing a key.

```text
 all   shop-api !2 ●1   mobile-app !3 ✕1 ●2                                  / writes to lead-shop-api
 lonca mobile-app 9d3f1c2 · 3 pending
 YOUR TURN 5                                        │#19 Paging in product search  shop-api · claude-1 · $0.90
▸ Δ shop-api    #19  Paging in product… +41 −6 ✓ 38 min│
  ? shop-api    #31  Refund rules         question 12 min│why      Paging in product search
  ⚑ mobile-app  #52  Notification sett…     allow? 4 min│proof    ✓ ran clean on this tree: dotnet test
  Δ mobile-app  #21  Currency support   +6 −3 ✕ 41 min│cost     $0.90 · 11 min · 2 rounds
                                                     │
 STUCK 1                                             │files    src/Catalog/ProductQuery.cs    +19 −6
  ■ mobile-app  #14  Price list cache   stopped 52 min│         tests/PagingTests.cs           +18 −0
                                                     │
 AT WORK 3                                           │@@ -11,3 +11,4 @@
  ● shop-api    #33  Order validation  Write the tests 2 min│-    var rows = db.Skip(q.Offset);
```

The first row names every open project with what it holds: `!` waits for you, `✕` stuck, `●` at work. The project it is narrowed to is lit, and its end says whom `/` writes to. The second row, for projects on Lonca, says how each stands.

## The list

| Group | What is in it |
| --- | --- |
| YOUR TURN | everything of WAITING FOR YOU, in the order `W` takes it |
| STUCK | agents blocked or stopped |
| AT WORK | agents mid-turn or asking a teammate. A worker whose change waits and who has gone on to other work is here too: two pieces of work. One waiting on a yes or an answer of yours is that item instead |
| QUIET | the rest, but for leaders with nothing to do |

A row is the piece of work first: its mark, project (when several are open), card and title, then what it comes to and how long it has stood there, from a minute on. A change ends with its lines and how its runs went; a question says `question`; an agent says the step it is on. The agent's name stands in the preview.

## The preview

What is at hand, whole: a question's or an ask's words, a change's title, proof, cost, gate, files and the top of its patch without git's own lines, an agent's status, to-do list and newest trace lines.

From 100 columns on it stands beside the list. On a narrower screen, beside an editor, the list takes the width and the row at hand opens under itself.

## Keys

| Key | What it does |
| --- | --- |
| `j` `k` / `↑` `↓`, `gg` `G` | move; the preview follows |
| `[` `]` | all projects, then each one, round |
| `enter` | open it: a change's diff, a question's conversation, an agent's focus. `esc` comes back here |
| `a` `r` | yes and no, as in [WAITING FOR YOU](waiting.md). On a question `a` writes back |
| `e` `A` | on an ask: yes for this turn · yes from now on |
| `s` | later: the item goes after the rest and you stay on the list |
| `/` | write to the lead of the project at hand; the first row says who |
| `@` | write to the agent at hand |
| `W` | open the next item |

Every other key works as everywhere; the status line's keys are the row at hand's.

A decision never lands on what you did not read: when the row at hand is gone by the time you press `a` or `r`, the key does nothing, a line says so, and the row now in its place is at hand.

The salon (with the journal, where a start's notes land), the board, the code map and the timeline stay, one key away. A key file's `[salon]` part holds on Home too.
