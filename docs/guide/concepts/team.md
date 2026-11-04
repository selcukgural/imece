---
title: The team
description: Who is on an imece team - the leader, workers, helpers and the researchers that are not built yet - how the leader hires with tiers, and how agents are named.
---

# The team

An imece team is a leader that talks to you and hires workers as the work needs them, workers that may hire helpers for parts of their task, and (one day) researchers. You never pick a team size; you set caps. You meet the team in the salon, grouped by project.

## Roles

| Role | Where it works | What it may do on the bus |
| --- | --- | --- |
| Leader (`lead`) | Your checkout, read only | `send`, `post`, `board`, `run`, `whoami`, `hire` (workers) |
| Worker (`claude-1`, `claude-2`, …) | Its own worktree, detached at your HEAD | `send`, `post`, `board`, `run`, `whoami`, `claim`, `hire` (helpers) |
| Helper (`claude-1.1`, …) | Its worker's worktree, the files its worker names | `send`, `post`, `board`, `run`, `whoami` |
| Solo (`solo`, `solo-notes`) | Its own worktree, in a project with no leader | `send`, `run`, `hire` (helpers) |
| Researcher | not built yet | — |

- **The leader** reads the checkout, splits your request into cards and hands them out with `send` (type `assign`), or puts them on the board for any worker to take. It never changes a file, at every `--confine` level: it works in your checkout, where a write would skip the review. It gets Claude Code's read tools and, from `standard` on, the web.
- **A worker** changes files in its own worktree, checks its work with the `run` tool, and sends `done` to whoever gave it the task. Its `done` becomes a change for you to review ([changes](../concepts/changes.md)). One card at a time per worker.
- **A helper** is to a worker what a worker is to the leader. A worker hires one for a part that can run beside its own work, in files it is not changing. A helper is not offered cards, hires nobody (no helpers of helpers), and is not counted against `--agents`. Its `done` to its worker marks it leaving; it leaves when that turn ends, so the turn's cost is still reported. The journal says `finished and left`. A worker that stops takes its helpers with it. A helper that stops before its `done` (its session ends, or its last turn fails for good: a refused key, a spent budget) is told to its worker by imece at the worker's next turn, "the part you gave it is not done: do it yourself, or hire another helper", and an idle worker is woken for it, so it does not wait on a helper that will not answer; an API error imece tries again is not a stop yet.
- **A solo** is a worker without a leader: you drive it yourself, on the model and effort you chose (`:solo`, `--solo`), in a project of its own. It has no board and no cards; every turn that ends with an answer shows you its worktree's diff. It may hire helpers as a worker does ([solo sessions](solo.md)).
- **Researchers** are a planned layer (a capped pool that answers questions, with graded taint and trusted sources). They are not built; only the demo shows them (`--researchers N`).

Claude Code's own sub-agent tool (`Agent`/`Task`) is off at every level: a sub-agent spawned inside a session would skip the worktree, the pot, the bus and the screen. Helpers are imece's answer to that need.

## Hiring

```sh
imece                              # the leader alone; it hires as needed
imece --agents 8 --helpers 3       # caps: workers per team, helpers per worker
```

- Bare `imece` starts the leader alone. It hires workers with the `hire` tool as the work needs them, up to `--agents` (0-16, default 4). A worker hires helpers the same way, up to `--helpers` (0-8, default 2) at once.
- `hire` takes a `tier`.
- **Where a worker starts.** A worker's worktree starts at your HEAD and follows it. Ask the leader for work on another point ("review PR 41", "fix this on release-0.2") and it hires with `from`: a branch, tag or commit of your checkout, or `#41` for a pull request of `origin` (fetched when the worker starts). That worktree starts there and stays there: it never follows your HEAD, its change is against that point, and you apply it to your checkout as any other (three-way when it does not fit cleanly). The journal says `lead hired claude-2 (haiku), starting from #41`, the row `⎇ #41`, and the point is kept between runs. Its helpers work in the same worktree.
- A socket on the bus is bound for the new agent before it exists. Hiring is for work that can run at once, not in advance: one worker takes several tasks in turn.
- There is no budget share per agent; the whole team spends from one pot ([budget](../concepts/budget-and-limits.md)).

## Tiers, not models

The leader picks a tier for a worker, never a model name:

| Tier | For | Model |
| --- | --- | --- |
| `cheap` | small, well-specified mechanical work (renames, tests of known behaviour, docs) | `haiku` |
| `standard` (default) | ordinary fixes and features | Claude Code's own default |
| `strong` | design that cuts across the code, hard debugging, review | `opus` |

- The leader itself runs on Claude Code's default, or on `--model`.
- `--tier T=NAME` changes what a tier means: `--tier strong=opus[1m]`, `--tier strong=opus@xhigh` (a value is `[MODEL][@EFFORT]`). `--model NAME` sets the model agents run on.
- **Fallback:** a tier model your account does not have fails its first turn (Claude Code reports it as a session that starts and then errors at zero cost). imece then replaces that worker once with the standard model, with the same prompts, and the journal says `model X is not available here, using the default`. One attempt, never a loop.

## Names

- The leader of the project imece started in is `lead`; a project opened with `:open PATH NAME` gets `lead-NAME`, and `:rename NAME` renames the leader with its project.
- Workers are `claude-N`: `claude-1`, `claude-2`. The number counts the workers, those that left included, and skips a name still taken.
- Helpers are `<worker>.N`: `claude-1.1` is the first helper of `claude-1`.
- You write to one agent with `@name text`; `you` is how an agent addresses you (`send(to: "you")`).

Agents address each other by these names on the bus; a `send` to a name that is not on the same project's team is refused with the team list.

## On the screen

```text
 SALON
 ◆ lead            coordinating
 ● claude-1        #12 parser: tokens      ⎇ ↑1 +34 −8   ▾ 1 helper
   └ ● claude-1.1  helper of claude-1
 ○ claude-2         idle
 ✓ claude-3        done, not seen yet
```

Workers that need you come first (stuck, then waiting for you, then done and not seen: the [state ladder](../screen/salon.md#the-state-ladder)); helpers sit under their worker and fold with `h` `l`. The header counts the fleet: `● 2 working ✓ 1 done ✕ 1 stuck`.

## See also

- [How imece works](../concepts/how-imece-works.md)
- [Messages and the board](../concepts/messages-and-board.md)
- [Claude Code](../concepts/claude-code.md): the confine levels and what a session is.
- [Projects](../concepts/projects.md): a leader per project.
- [The salon](../screen/salon.md) and [the CLI reference](../reference/cli.md).
