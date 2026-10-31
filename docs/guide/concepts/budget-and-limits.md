---
title: Budget and limits
description: "The one pot the whole team spends from, how it is set and changed, what the header says about the burn rate, the vendors' usage windows, what costs money in the : menu, and :commit's own ceilings."
---

# Budget and limits

The budget is one pot for the whole team: once all agents together have spent it, nobody takes another turn. You meet it in the header on every screen, in `:budget`, and in the `$` marks of the `:` menu.

## The pot

- `--budget USD` sets it; the first time it is $50. The pot is **kept between runs**: a start without `--budget` goes on with what was left. `--budget` on a later start sets it anew.
- Every agent's turns, of every project, come out of it, and so do imece's own paid calls (`:commit`). There is no share per agent.
- When it is spent, no agent gets a waking message and no card is offered; a `send` that would wake someone is refused with "the budget is spent".
- **A per-session hard cap:** each Claude Code session also gets what was left of the pot when it started as its own `--max-budget-usd`, so a session running away on its own cannot pass it either. It is a last stop, not a share. Codex and Gemini have no such cap; the pot still stops their next turn.

### `:budget USD`

`:budget 80` (`:b 80`) raises or lowers the pot while agents run. A turn under way finishes on its old cap; the next starts on the new remainder. A Claude session whose turn ended at its old cap while the pot has room is ended and started again with the same prompts and the new remainder, once per session until a turn finishes. The journal says `you set the budget to $80.00 (was $50.00)`. Anything that is not dollars like `80` or `2.50` is refused.

## The header

```text
 imece  3/7 cards │ ● 2 working ✓ 1 done │ spent $3.10 of $20.00 ($2.40/h, ~2 h 10 min left) │ claude 5h: 48% left, resets in 1 h 59 min
```

- `$/h` is what the **last ten minutes** cost per hour, not the average since the start (an idle hour would pull that down and a first turn push it up). Costs arrive at turn ends, so the rate moves in steps. Only this run counts.
- `~… left` is how long the pot lasts at that rate. Under half an hour the whole budget turns the **attention colour** and the journal says once, with the rate and the minutes, that the pot runs low: `at $12.00/h the budget lasts about 25 min more; :budget USD raises it`. `:budget` makes it say so again. It is a journal line, not an item in WAITING FOR YOU: nothing in it needs an answer, and the pot stops turns on its own.
- Each agent's own total is on its salon row; there is no rate per agent.

## Vendor usage windows

A team on Claude or ChatGPT subscriptions can be stopped by the plan's five-hour or weekly limit long before the pot runs dry. Claude Code and Codex sessions report their account's windows; Gemini does not. imece keeps one per vendor and window length and shows each vendor's tightest after the pot, `codex 7d: 12% left`, in the attention colour from 90% used, with the time to the reset while it is ahead. Nothing stops a turn before a window is full: the vendor refuses it, and imece shows the refusal. Windows are not kept between runs.

## What counts and how

| Vendor | What a turn costs the pot |
| --- | --- |
| Claude Code | the dollars it reports (`total_cost_usd`); a resumed session reports the whole session's, and imece counts only what lies above what was already booked |
| Codex | nothing on a ChatGPT plan (tokens shown in the trace); an API-key login is refused, since imece cannot price it yet |
| Gemini CLI | with an API key, the list prices imece carries (prompts up to 200K tokens, thought tokens as output, cached tokens at the cached rate, several models at the dearest); nothing with a Google login; a model it cannot price stops the session |

What was spent is read back from the store at every start, agents and imece's own calls alike.

## `$` in the `:` menu

The `:` menu shows each command's value after it, `budget (spent $3.10 of $20.00)`, and marks with `$` the commands that raise the next bill:

- `:open` starts a new leader session.
- `:forget` makes the next message start a new session that pays the whole briefing again, without the cache.
- `:commit` is a paid model call.

No `:` command starts a paid turn by itself; `:budget` shows its value instead of a mark.

## `:commit` and its ceilings

`:commit` asks Claude Code's cheap tier (`haiku`, or your `--tier claude:cheap=…`) for a commit message for what is staged in the project at hand and puts it on the command line as `!git commit -m '…'`. It is asked for, not run on every apply, because you often apply several changes and commit once. Its ceilings follow the standing cost rules: a daily ceiling and a cap per call, reserved before the call.

| Ceiling | Value |
| --- | --- |
| One suggestion | at most $0.05 (the call's own `--max-budget-usd`) |
| One day (UTC), every project together | 20 suggestions and $1.00 |
| Running at once | one |

Before the call the store counts it at its cap in one transaction and the pot holds the cap; it is refused when the pot has less than the cap or the day is full. When the call ends, what it reported replaces the cap; one that could not start cost nothing; one that stopped without reporting keeps its cap. The same staged tree asked again costs nothing: the last suggestion per project is kept. The journal says each suggestion with its model and cost, and each failure with why. Only Claude Code: it is the one vendor whose call takes a hard spending cap; without it on PATH `:commit` is refused.

## The demo

`--demo` spends a made-up pot the same way: the header, the rate, the low-pot line, `:budget`, the usage windows (filled from spending) and `:commit`'s few cents all show as live does.

## See also

- [Vendors](../concepts/vendors.md): tokens versus dollars per vendor.
- [Sessions](../concepts/sessions.md): the pot in the store.
- [Own shell and commit](../scenarios/own-shell-and-commit.md): `:commit` in use.
- [Commands](../reference/commands.md): `:budget`, `:commit`.
