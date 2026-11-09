---
title: Budget and limits
description: "The one pot the whole team spends from, how it is set and changed, what the header says about the burn rate, Claude Code's usage windows, what costs money in the : menu, the ceilings of :commit and :review, and what each attempt at a card cost (:ledger)."
---

# Budget and limits

The budget is one pot for the whole team: once all agents together have spent it, nobody takes another turn. You meet it in the header on every screen, in `:budget`, and in the `$` marks of the `:` menu.

## The pot

- `--budget USD` sets it; the first time it is $50. The pot is **kept between runs**: a start without `--budget` goes on with what was left. `--budget` on a later start sets it anew.
- Every agent's turns, of every project, come out of it, and so do imece's own paid calls (`:commit`, `:review`). There is no share per agent.
- When it is spent, no agent gets a waking message and no card is offered; a `send` that would wake someone is refused with "the budget is spent".
- **A per-session hard cap:** each Claude Code session also gets what was left of the pot when it started as its own `--max-budget-usd`, so a session running away on its own cannot pass it either. It is a last stop, not a share.

### `:budget USD`

`:budget 80` (`:b 80`) raises or lowers the pot while agents run. A turn under way finishes on its old cap; the next starts on the new remainder. A Claude session whose turn ended at its old cap while the pot has room is ended and started again with the same prompts and the new remainder, once per session until a turn finishes. The journal says `you set the budget to $80.00 (was $50.00)`. Anything that is not dollars like `80` or `2.50` is refused.

## The header

```text
 imece  ● 2 working ✓ 1 done │ spent $3.10 of $20.00  14:02
```

The header carries the pot alone; the `:budget` menu's note carries the rest: `(spent $3.10 of $20.00 · $2.40/h, ~2 h 10 min left · claude 5h: 48% left)`.

- `$/h` is what the **last ten minutes** cost per hour, not the average since the start (an idle hour would pull that down and a first turn push it up). Costs arrive at turn ends, so the rate moves in steps. Only this run counts.
- `~… left` is how long the pot lasts at that rate. Under half an hour the header's pot turns the **attention colour** and says it, `spent $19.60 of $20.00, ~25 min left`, and the journal says once, with the rate and the minutes, that the pot runs low: `at $12.00/h the budget lasts about 25 min more; :budget USD raises it`. `:budget` makes it say so again. It is a journal line, not an item in TO DECIDE: nothing in it needs an answer, and the pot stops turns on its own.
- Each agent's own total is on its team row; there is no rate per agent.

## Usage windows

A team on a Claude subscription can be stopped by the plan's five-hour or weekly limit long before the pot runs dry. Claude Code sessions report their account's windows. imece keeps one per window length and shows the tightest after the pot, `claude 7d: 12% left`, in the attention colour from 90% used, with the time to the reset while it is ahead. Nothing stops a turn before a window is full: Claude Code refuses it, and imece shows the refusal. Windows are not kept between runs.

## What counts and how

A turn costs the pot the dollars Claude Code reports (`total_cost_usd`); a resumed session reports the whole session's, and imece counts only what lies above what was already booked.

What was spent is read back from the store at every start, agents and imece's own calls alike.

## What a figure means

Every dollar figure is Claude Code's own: it prices each turn from its own price table, and imece books what it says. Two things change how a figure reads:

- **`≈` on a plan.** Signed in through a claude.ai plan (Pro, Max, Team), Claude Code still prices turns at API rates, but what you pay is the plan. Then every figure Claude Code gave starts with `≈`, API-equivalent: `≈$3.10 of $20.00`, `≈$0.84 · 12 min`. The budget itself, which you set, stays plain. imece reads the sign-in at every start (`claude auth status --json`: `authMethod` `claude.ai`); an API key in the environment pays by the token, so its figures are plain. `:welcome` says which: `signed in, a plan · ≈ API prices`.
- **`+?` for what was never priced.** A figure is never `$0.00` for spending nobody priced. A turn whose result says no cost, a model Claude Code priced at nothing though it used tokens (one it has no price for), and a session that ended mid-turn (a quit, `:close`, `:forget`, a crash) leave part of an agent's spending unknown. Its figure then reads `$2.29+?`, what it spent at least, or `?` alone when none of it is known; the change it worked on, its project's total, the pot and the farewell's `spent this run` read the same way. A turn's own result without a figure is covered again by the session's next one, which says the whole session; a session that ended mid-turn stays `+?` for good, kept in the store. The agent's trace says why: `cost unknown: the turn's result said none`. The pot's limit works on what is known.
- `:commit` and `:review` that do not say all they cost keep what they reserved, as one that stopped without a figure does.

## `$` in the `:` menu

The `:` menu shows each command's value after it, `budget (spent $3.10 of $20.00)`, and marks with `$` the commands that raise the next bill:

- `:open` starts a new leader session.
- `:forget` makes the next message start a new session that pays the whole briefing again, without the cache.
- `:commit` and `:review` are paid model calls.

No `:` command starts a paid turn by itself; `:budget` shows its value instead of a mark.

## `:commit` and its ceilings

`:commit` asks Claude Code's cheap tier (`haiku`, or your `--tier claude:cheap=…`) for a commit message for what is staged in the project at hand and puts it on the command line as `!git commit -m '…'`. It is asked for, not run on every apply, because you often apply several changes and commit once. Its ceilings follow the standing cost rules: a daily ceiling and a cap per call, reserved before the call.

| Ceiling | Value |
| --- | --- |
| One suggestion | at most $0.05 (the call's own `--max-budget-usd`) |
| One day (UTC), every project together | 20 suggestions and $1.00 |
| Running at once | one |

Before the call the store counts it at its cap in one transaction and the pot holds the cap; it is refused when the pot has less than the cap or the day is full. When the call ends, what it reported replaces the cap; one that could not start cost nothing; one that stopped without reporting keeps its cap. The same staged tree asked again costs nothing: the last suggestion per project is kept. The journal says each suggestion with its model and cost, and each failure with why.

## `:review` and its ceilings

`:review` has a fresh Claude Code session read a change and say what it finds ([changes](changes.md#a-second-reading)). Its ceilings are its own, apart from `:commit`'s, and are kept the same way: counted in the store by kind and day, reserved before the call, settled after.

| Ceiling | Value |
| --- | --- |
| One review | at most $0.10 (the call's own `--max-budget-usd`) |
| One day (UTC), every project together | 20 reviews and $2.00 |
| Running at once | one; the rest wait their turn |

A change that has its review, or one asked for, is not reviewed again.

## What a change cost

The pot says what the team spent; each change says what it alone cost ([changes](changes.md#what-it-cost)): the worker's spend, its helpers' counted in, from its first prompt after its last change to its `done`, with the time it worked and, for a change sent back and sent again, its rounds: `$0.84 · 12 min`, `$1.20 · 25 min · 3 rounds`. It stands on the `Δ` row, above the diff, in the journal and in the leader's copy of the `done` (`[done 12 · $0.84]`), so the leader learns what its tiers cost. It is the same spend the pot counts, cut by change; nothing is paid twice.

## The ledger

The pot says what the team spent, a change what it cost; `:ledger` says what each **attempt** at a card cost and what became of it, so you see what was applied and what was spent for nothing.

```text
┌LEDGER · 4 attempts · API-equivalent───────────────────────────────────────────────────────┐
│shop-api  applied ≈$1.30 · wasted ≈$0.91+? · at work ≈$0.22 · lead ≈$0.48                  │
│▶ #4 try 1  claude-2  claude-sonnet-5-5  1 Δ  ≈$0.22  6 min  at work  2026-11-09 · 2.1.4    │
│  #3 try 2  claude-1  claude-opus-5-5  2 Δ · 1 sent back  ≈$1.30  31 min  applied  …       │
│  #3 try 1  claude-3  claude-sonnet-5-5  0 Δ  ≈$0.84  12 min  handed on  …                 │
│  #2 try 1  claude-2  claude-sonnet-5-5  0 Δ  ≈$0.07+?  2 min  failed  …                   │
│ j k  move   enter  open the card   esc  back                                              │
└───────────────────────────────────────────────────────────────────────────────────────────┘
```

- **A row is an attempt**: a worker's try at a card (a card handed again is a try more), with its worker, the model Claude Code said its session runs on, the changes it put before you (`Δ`) and how many came back (sent back by you or by Lonca), what it cost (its helpers' counted in), how long it took, how it ended, the day it last spent and Claude Code's version then. A worker's work with no card (a solo, a worker you wrote to directly) is a `no card` row until a change of it is applied.
- **How it ended**: `at work` (or its change waits), `applied` (on trunk, on Lonca), `done, no change`, `failed` (its worker said so, or you said no to one more try), `withdrawn`, `handed on` (the card went to another try while this one worked), `late done` (handed on, and its worker still sent a `done`). What a worker spends after its card failed, was withdrawn or handed on is counted on that try: it was spent for nothing.
- **A project's line** adds its rows up by how they ended: `applied` (applied, done), `wasted` (failed, withdrawn, handed on, late), `at work`, and the leader's own spend apart, since a leader's turns are nobody's card. Every dollar of a worker or a helper is on exactly one row.
- The figures read as everywhere else: `≈` on a plan (the title says `API-equivalent`), `+?` for a part that was never priced.
- `enter` opens the row's card; a card's own screen lists its tries under **ATTEMPTS**. The ledger is kept in the store, archives too; what was spent before this version has no rows.

## The demo

`--demo` spends a made-up pot the same way: the header, the rate, the low-pot line, `:budget`, the usage windows (filled from spending), what each change cost and the few cents of `:commit` and `:review` all show as live does. The demo signs in through a plan, so its figures read `≈`, and every fifth change it reads ends a turn that said no cost (`+?`) until that worker's next one. `:ledger` fills as its cards are applied, fail, are handed on and withdrawn.

## See also

- [Claude Code](../concepts/claude-code.md): the confine levels and usage limits.
- [Sessions](../concepts/sessions.md): the pot in the store.
- [Own shell and commit](../scenarios/own-shell-and-commit.md): `:commit` in use.
- [Commands](../reference/commands.md): `:budget`, `:commit`, `:ledger`.
