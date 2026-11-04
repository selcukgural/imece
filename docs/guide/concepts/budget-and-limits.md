---
title: Budget and limits
description: "The one pot the whole team spends from, how it is set and changed, what the header says about the burn rate, Claude Code's usage windows, what costs money in the : menu, and the ceilings of :commit and :review."
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
 imece  3/7 cards │ ● 2 working ✓ 1 done │ spent $3.10 of $20.00 ($2.40/h, ~2 h 10 min left) │ claude 5h: 48% left, resets in 1 h 59 min
```

- `$/h` is what the **last ten minutes** cost per hour, not the average since the start (an idle hour would pull that down and a first turn push it up). Costs arrive at turn ends, so the rate moves in steps. Only this run counts.
- `~… left` is how long the pot lasts at that rate. Under half an hour the whole budget turns the **attention colour** and the journal says once, with the rate and the minutes, that the pot runs low: `at $12.00/h the budget lasts about 25 min more; :budget USD raises it`. `:budget` makes it say so again. It is a journal line, not an item in WAITING FOR YOU: nothing in it needs an answer, and the pot stops turns on its own.
- Each agent's own total is on its salon row; there is no rate per agent.

## Usage windows

A team on a Claude subscription can be stopped by the plan's five-hour or weekly limit long before the pot runs dry. Claude Code sessions report their account's windows. imece keeps one per window length and shows the tightest after the pot, `claude 7d: 12% left`, in the attention colour from 90% used, with the time to the reset while it is ahead. Nothing stops a turn before a window is full: Claude Code refuses it, and imece shows the refusal. Windows are not kept between runs.

## What counts and how

A turn costs the pot the dollars Claude Code reports (`total_cost_usd`); a resumed session reports the whole session's, and imece counts only what lies above what was already booked.

What was spent is read back from the store at every start, agents and imece's own calls alike.

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

## The demo

`--demo` spends a made-up pot the same way: the header, the rate, the low-pot line, `:budget`, the usage windows (filled from spending), what each change cost and the few cents of `:commit` and `:review` all show as live does.

## See also

- [Claude Code](../concepts/claude-code.md): the confine levels and usage limits.
- [Sessions](../concepts/sessions.md): the pot in the store.
- [Own shell and commit](../scenarios/own-shell-and-commit.md): `:commit` in use.
- [Commands](../reference/commands.md): `:budget`, `:commit`.
