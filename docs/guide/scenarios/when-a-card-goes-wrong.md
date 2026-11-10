---
title: "When a card goes wrong"
description: "What imece does when work does not go as planned: a card that waits on another, a card that fails and is tried again, the breaker after three tries, a withdrawn card, a health notice to the leader, a hung command ended with :ps, and what it all cost in :ledger."
---

# When a card goes wrong

Most cards go from `assign` to an approved change. This scenario is about the others: a card that cannot start until another is done, a card its worker cannot finish, a card nobody needs any more, a worker that goes round in circles, a test run that hangs. imece does not hide any of it, and it stops what would otherwise repeat by itself until you have said yes. At the end, `:ledger` shows what each try cost and what was spent for nothing.

Everything here can be watched in the demo, which plays all of it within an hour of its clock:

```sh
imece --demo --projects 1 --agents 3
```

The demo's project is `shop-api`, led by `lead`, with three workers. Its figures start with `≈` because the demo signs in as a plan's login (on an API key the figures have no `≈`).

## 1. A card that waits on another

The leader can say that a card needs another card's work first: `after` on an `assign` or a `post` names the cards it waits on, by number (at most eight, only cards of the project that were not failed for good). An assign with `after` does not wait in the leader's head: it is sent at once, its card opens on the board, and the worker it is for gets it only once what it waits on is done. The journal says so:

```text
 14:07 → claude-3 gets #6 Paging in product search after #5
```

`:board` shows it under TO DO, marked with what it waits on and who it is held for:

```text
 ┌ TO DO (10) ────────┐
 │#6  Paging in produ…│
 │    ⧗ after #5 → cla│
 │#21 Order validatio…│
 │    ✕ #20 failed → c│
```

`⧗ after #5` is dim: it is only waiting. `✕ #20 failed` is in the attention colour: what it waits on failed, so it will not begin until that card is handed again and done. `r` in the board shows only the cards that are ready to take, and the title says how many it hides (`· ready only: N waiting hidden · r all`); `r` again shows all. `esc` closes the board.

`#6` on the command line opens the card. Under its column it lists what it waits on, `✓` done, `✕` failed, `⧗` not yet:

```text
 shop-api · TO DO · 75 min
 after  #5 ⧗
```

When `#5` is done, the next delivery begins `#6` (`claude-3 begins #6 Paging in product search: what it waited on is done`), and the worker's mail is headed `[assign 14 · #6, try 1]`. A held assign does not wake a sleeping worker, and a worker cannot `claim` a card from the board that is not ready.

## 2. A card that fails

A worker that cannot finish its card says so: `done` with `outcome: "failed"`. No change is shown, the card goes to the done column marked `✕`, and the journal has the worker's first line:

```text
 14:34 ■ claude-1: #9 Currency support failed: Could not finish it: the callers need a change outside the card's paths.
```

The leader's mail is headed `[done 12 · #9 failed]`, and the leader is told to hand the card again with what to do differently. It does, to another worker:

```text
 14:35 → claude-3 got #9 Currency support from claude-1, try 2
```

The board says ` · try 2` on the card, and its screen counts the tries. If `claude-1` was still at work on `#9` and sends a `done` for it after it was handed on, that `done` is late: it closes nothing, shows no change, and its worktree keeps what it wrote:

```text
 15:58 ⚑ claude-2: a late done for #50, which went to another try; it closes nothing
```

In the demo, every seventh first try fails, and a failed card fails again each time its leader hands it on by itself, so the next step comes quickly.

## 3. The breaker: three tries, then you decide

A card has three tries and its change three rounds. Past them nothing goes on by itself: the leader's next `assign` for the card is refused, and the card waits for you in TO DECIDE, under questions:

```text
 TO DECIDE 1 permission · 1 question · 3 changes
 ...
 questions
▸! #9   Currency support          failed 3 tries: one more, or it fails  a  one more   r  fail it
                                  for good
```

```text
 15:00 ■ #9 Currency support failed 3 tries: another waits for your yes
```

The status line says `a  one more   r  fail it`. `#9` opens the card's screen, which shows why: each try under ATTEMPTS, with its worker, model and what it cost, and the story's `failed 3 tries: the person decides`.

- `a` gives one more try. The flash: `one more try for #9: the leader hears it`; the journal: `› you allowed #9 one more try`. The leader is woken, hands the card again, and the board says `try 4`.
- `r` fails the card for good. The flash: `#9 failed for good`; the journal: `› you failed #9: nobody hands it again`. No `assign` takes it any more, and the leader hears which cards wait on it, to withdraw them or tell you.

On a project on [Lonca](../concepts/projects.md#on-lonca) the rounds count too: when Lonca's queue sends a change back for the third time, its worker is not woken, and the row says `its change came back 3 times: one more round, or it fails for good`. Your own send-back, your notes and a clash you send back always pass: you are deciding already.

## 4. A card nobody needs

When the leader finds a card is not needed any more, it withdraws it with its `withdraw` tool and one line of why:

```text
 15:15 → lead withdrew #34 Notification settings: not needed any more: another card covers it
```

A card waiting, at work or failed can be withdrawn; one whose change waits for you, is in Lonca's queue or is done cannot (you send its change back yourself). Whoever held it is told at its next prompt, a `done` it still sends is late, and the board puts the card with the done ones, marked `–` and dim. Its screen says WITHDRAWN. A card that waits on a withdrawn one never begins: the board says `✕ #34 withdrawn` on it and the leader is told to withdraw it too.

## 5. A health notice to the leader

imece watches every agent for four signs and tells the leader, not you, since the leader can act on them:

| Sign | When | Journal line |
| --- | --- | --- |
| stalled | a worker's card is at work, the worker idle for 10 min | `claude-2 has been idle 10 min on #9 Currency support without a done` |
| silent | an agent other than the leader is mid-turn with no trace line for 10 min | `claude-2 has said nothing for 10 min in its turn; last: Bash dotnet test` |
| free | a worker with no card is idle for 10 min | `claude-3 has been idle 10 min with no card` |
| looping | one call fails 3 times in one turn | `claude-2 failed 3 times this turn on run dotnet test` |

A worker that has waited 10 minutes on a teammate's answer is no notice: it is its leader's to solve, as a `blocked` is, and its row says `no answer from claude-3 "…" · lead is on it` ([a worker that waits on an answer](../concepts/messages-and-board.md#a-worker-that-waits-on-an-answer)).

The demo shows the loop: every fourth failing test run fails three times in a row.

```text
 15:05 ■ claude-2 failed 3 times this turn on run dotnet test · lead told
```

`· lead told` says the leader was woken for it. A free worker is a `⚑` line ending `· lead hears at its next turn` and wakes nobody (an idle worker costs nothing). The leader's note says what to do, in imece's words: `claude-2's call run dotnet test failed 3 times this turn: it may be going round. Cancel it, or tell it another way.` Each quiet is told once; moving again starts a new one. Nothing is told about an agent that waits on you, is starting, has a retry coming or has helpers at work, or about a solo, which has no leader.

`:health` alone says the settings: `health notices to the leader: stall 10m · idle 10m · loop 3 (stall and idle wait minutes; loop counts failures of one call in a turn)`. `:health stall 20m`, `:health idle 1h`, `:health loop 5` or `off` change one; they are remembered.

## 6. A test run that hangs: `:ps`

A live worker runs `dotnet test` through Claude Code's shell, and it hangs. After ten minutes the leader hears `claude-2 has said nothing for 10 min in its turn; last: Bash dotnet test · lead told`. To see what runs, type `:ps`:

```text
 ┌ PROCESSES · 9 ────────────────────────────────────────────────────────┐
 │each agent's session and what runs under it, as ps reports it: CPU of …│
 │                                                                       │
 │claude-1 · 75.5% · 716 MiB                                             │
 │  claude      41010    1.5%  287 MiB                                   │
 │    imece     41011    0.0%  9 MiB                                     │
 │    dotnet    61010   74.0%  420 MiB                                   │
 │claude-2 · 75.8% · 723 MiB                                             │
 │  claude      41020    1.8%  294 MiB                                   │
 │    imece     41021    0.0%  9 MiB                                     │
 │▶   dotnet    62010   74.0%  420 MiB                                   │
 │                                                                       │
 │ d  end this one   j k  move   esc  back                               │
 └───────────────────────────────────────────────────────────────────────┘
```

Each agent has a line with its sum, then its session's process (`claude`), imece's relay under it (`imece`), and what the session started. The window reads `ps` again every three seconds while it is open. `j` `k` move to the hung `dotnet`; `d` asks:

```text
 end dotnet (62010) under claude-2? y ends it, any other key keeps it
```

`y` sends it `SIGTERM`, and `SIGKILL` two seconds later if it is still there. The flash and the journal: `you ended dotnet (62010) under claude-2`. Only that process ends; the session goes on with its turn, the command's failure its to read. imece checks again, just before each signal, that the process is still under that session. The session's own process is refused (`that is claude-2's session itself: :forget or a stop ends it, not :ps`), and so is imece's relay.

A run of imece that ended without ending its sessions (a crash, a closed terminal) can leave them running. A start says how many it finds (`4 processes left from an earlier run: :ps lists them`), and `:ps` lists them under `LEFT FROM AN EARLIER RUN · 4`, each tree under the worktree it ran in. `d` ends one, `a` all of them, each after `y`. Nothing is ever ended without your `y`.

The demo's processes are made up: each session a `claude` with `imece` under it and, while it works, the project's test program, plus a crashed run's leftovers.

## 7. What it all cost: `:ledger`

Every try at a card is a row in the ledger, booked from what Claude Code says each session spent. `:ledger` opens it:

```text
 LEDGER · 30 attempts · API-equivalent
 shop-api  applied ≈$0.00 · wasted ≈$0.52 · at work ≈$1.26+? · lead ≈$0.00
   by model: claude-opus-5-5 ≈$0.95 · claude-sonnet-5-5 ≈$0.61 · claude-haiku-5-5 ≈$0.22
 ▶ #30 try 3  claude-2  claude-sonnet-5-5  0 Δ  ≈<$0.01+?  15 s  at work  2026-11-09 · 2.1.0
     by model: claude-sonnet-5-5 ≈<$0.01 · claude-haiku-5-5 ≈<$0.01
   #35 try 1  claude-3  claude-opus-5-5  0 Δ  ≈<$0.01  25 s  at work  2026-11-09 · 2.1.0
   #30 try 2  claude-3  claude-opus-5-5  0 Δ  ≈$0.07  6 min  failed  2026-11-09 · 2.1.0
   #33 try 1  claude-2  claude-sonnet-5-5  1 Δ  ≈$0.07  8 min  at work  2026-11-09 · 2.1.0
```

The project's line splits what was spent: `applied` on tries whose change you applied (or that ended done with no change), `wasted` on tries that failed, were withdrawn or handed on, `at work` on tries still open, `lead` the leader's own spending, which is no card's. Each row: the card and its try, the worker, the model its session ran on, the changes it put before you (`Δ`, and ` · N sent back`), what it cost, how long, how it ended (at work, applied, done with no change, failed, withdrawn, handed on, late done), the day and Claude Code's version. `by model` splits a row by the models that spent on it: the worker's, its helpers' and Claude Code's own background model.

`≈` marks API-equivalent figures on a plan's login. `+?` marks a figure with a part Claude Code never priced (a turn without a cost, a model priced at nothing, a session that ended mid-turn): what is shown is at least what was spent, never `$0` for what is not known.

`j` `k` move, `enter` opens the row's card, `esc` closes. A card's screen lists its own tries under ATTEMPTS.

## See also

- [Messages and the board](../concepts/messages-and-board.md): `assign`, `post`, `done`
- [Message types](../reference/message-types.md): the envelope's `after`, `outcome` and `card`
- [Budget and limits](../concepts/budget-and-limits.md#the-ledger): the ledger, `≈` and `+?`
- [Commands](../reference/commands.md): `:board`, `:health`, `:ps`, `:ledger`
