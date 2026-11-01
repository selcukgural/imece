---
title: Scenarios
description: Seven step-by-step walks through imece, each key and command named with what the screen shows in reply.
---

# Scenarios

The concept and screen pages say what each part of imece is. The scenarios put the parts in order: a developer sits down with a task, and every step names the key or command pressed and what the screen answers. Read them when you know what imece does and want to see how a day with it goes.

Every scenario is true of the real imece: the keys are the ones the `?` help lists, the journal lines are the words the screen uses, the numbers (caps, prices, defaults) are the ones in the code. Drawn screens are short and plausible, not a transcript of a real run.

## The seven

| Scenario | One line |
| --- | --- |
| [A failing test, one worker](one-fix.md) | From `/ fix it` to `!git commit`: the leader hires `claude-1`, a `⚑` ask, a `Δ`, apply, a suggested commit message. |
| [Codex leads, Claude and Gemini work](mixed-vendors.md) | `--lead codex`, hiring across vendors, what a tier means per vendor, tokens against dollars, the header's usage limits, `--confine strict`. |
| [Sending a change back, then taking half of it](review-with-notes.md) | Notes with `c` and `n`, the worker's answer with `↩` and `%`, hunks chosen with `x`, `>` to quote one, `r` to reject the rest. |
| [Three checkouts, one screen](many-projects.md) | `:open`, folding, `:pin`, `f`, `:rename`, `:copy .env`, `:close`, and the one pot every project draws from. |
| [A solo beside the team](solo-beside-the-team.md) | `:solo claude:opus@xhigh`, a session you drive yourself with no leader, the `Δ` after every answer, a second solo on Codex, `--solo` the next morning. |
| [Coming back](next-morning.md) | Quitting mid-turn, the "where you left off" window, agents asleep, how each vendor resumes, `:forget`, the away digest, `:notify`, marks, `:export`. |
| [Your shell next to the team](own-shell-and-commit.md) | `!cargo test`, `!vim`, ctrl-c, `:commit` and its ceilings, `:awake`, `:budget 80`, `:theme nord`, `L`. |

## Trying them for free

Everything a scenario does on the screen can be tried with `imece --demo`: fake agents play the same model, so the salon, WAITING FOR YOU, the diff view, notes, chosen hunks, `:open`, `:pin`, `:copy`, marks and `:export` all work the same way and cost nothing. What the demo cannot show is the part that needs an earlier run and a store: the "where you left off" window, turns cut short, agents asleep from the start. The [demo page](../getting-started/demo.md) says what else differs.

## A reading order

1. [A failing test, one worker](one-fix.md): the whole loop once, with one agent.
2. [Sending a change back, then taking half of it](review-with-notes.md): the review keys in depth.
3. [Your shell next to the team](own-shell-and-commit.md): the person's own tools beside the agents.
4. [Codex leads, Claude and Gemini work](mixed-vendors.md): when more than one vendor is on PATH.
5. [Three checkouts, one screen](many-projects.md): when more than one project is open.
6. [A solo beside the team](solo-beside-the-team.md): when you want to drive one agent yourself.
7. [Coming back](next-morning.md): the second day.

## See also

- [How imece works](../concepts/how-imece-works.md)
- [The keyboard](../screen/keyboard.md)
- [Commands](../reference/commands.md)
