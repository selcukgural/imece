---
title: The team
description: Who is on an imece team - the leader, workers, helpers and the researchers that are not built yet - how the leader hires with tiers, who leads, and how agents are named.
---

# The team

An imece team is a leader that talks to you and hires workers as the work needs them, workers that may hire helpers for parts of their task, and (one day) researchers. You never pick a team size; you set caps. You meet the team in the salon, grouped by project.

## Roles

| Role | Where it works | What it may do on the bus |
| --- | --- | --- |
| Leader (`lead`) | Your checkout, read only | `send`, `post`, `board`, `run`, `hire` (workers) |
| Worker (`claude-1`, `codex-2`, …) | Its own worktree, detached at your HEAD | `send`, `post`, `board`, `run`, `claim`, `hire` (helpers) |
| Helper (`claude-1.1`, …) | Its worker's worktree, the files its worker names | `send`, `post`, `board`, `run` |
| Solo (`solo`, `solo-notes`) | Its own worktree, in a project with no leader | `send`, `run`, `hire` (helpers) |
| Researcher | not built yet | — |

- **The leader** reads the checkout, splits your request into cards and hands them out with `send` (type `assign`), or puts them on the board for any worker to take. It never changes a file, at every `--confine` level: it works in your checkout, where a write would skip the review. It gets its vendor's read tools and, from `standard` on, the web.
- **A worker** changes files in its own worktree, checks its work with the `run` tool, and sends `done` to whoever gave it the task. Its `done` becomes a change for you to review ([changes](../concepts/changes.md)). One card at a time per worker.
- **A helper** is to a worker what a worker is to the leader. A worker hires one for a part that can run beside its own work, in files it is not changing. A helper is not offered cards, hires nobody (no helpers of helpers), and is not counted against `--agents`. Its `done` to its worker marks it leaving; it leaves when that turn ends, so the turn's cost is still reported. The journal says `finished and left`. A worker that stops takes its helpers with it.
- **A solo** is a worker without a leader: you drive it yourself, on the vendor, model and effort you chose (`:solo`, `--solo`), in a project of its own. It has no board and no cards; every turn that ends with an answer shows you its worktree's diff. It may hire helpers as a worker does ([solo sessions](solo.md)).
- **Researchers** are a planned layer (a capped pool that answers questions, with graded taint and trusted sources). They are not built; only the demo shows them (`--researchers N`).

The vendor's own sub-agent tool (Claude Code's `Agent`/`Task`, Codex's `multi_agent`, Gemini's `invoke_agent` and the like) is off at every level: a sub-agent spawned inside a CLI would skip the worktree, the pot, the bus and the screen. Helpers are imece's answer to that need.

## Hiring

```sh
imece                              # the leader alone; it hires as needed
imece --agents 8 --helpers 3       # caps: workers per team, helpers per worker
```

- Bare `imece` starts the leader alone. It hires workers with the `hire` tool as the work needs them, up to `--agents` (0-16, default 4). A worker hires helpers the same way, up to `--helpers` (0-8, default 2) at once.
- `hire` takes a `tier` and a `vendor`. The vendor defaults to the hirer's own; the leader may hire from any vendor imece found on PATH (`claude`, `codex`, `gemini`). Only vendors imece has an adapter for are hireable.
- A socket on the bus is bound for the new agent before it exists. Hiring is for work that can run at once, not in advance: one worker takes several tasks in turn.
- There is no budget share per agent; the whole team spends from one pot ([budget](../concepts/budget-and-limits.md)).

## Tiers, not models

The leader picks a tier for a worker, never a model name. Each vendor maps tiers to its own models:

| Tier | For | Claude | Codex | Gemini |
| --- | --- | --- | --- | --- |
| `cheap` | small, well-specified mechanical work (renames, tests of known behaviour, docs) | `haiku` | `@low` (its default model at low effort) | `flash` |
| `standard` (default) | ordinary fixes and features | the vendor's own default | the default | the default (`auto`) |
| `strong` | design that cuts across the code, hard debugging, review | `opus` | `@high` | `pro` |

- The leader itself runs on its vendor's default, or on `--model`.
- `--tier [V:]T=NAME` changes what a tier means: `--tier strong=opus[1m]`, `--tier strong=opus@xhigh`, `--tier codex:strong=gpt-5.6-terra@xhigh` (a value is `[MODEL][@EFFORT]`; the efforts are the vendor's own, Gemini has none). `--model [V:]NAME` sets the model agents of vendor V run on. Without a vendor prefix both speak of the leader's vendor.
- **Fallback:** a tier model your account does not have fails its first turn (Claude Code reports it as a session that starts and then errors at zero cost). imece then replaces that worker once with the standard model, with the same prompts, and the journal says `model X is not available here, using the default`. One attempt, never a loop.

## Who leads

There is no silent default.

1. `--lead VENDOR` wins (`claude`, `codex`, `gemini`).
2. Else the primary vendor you set, if it is on PATH and driveable.
3. Else the only driveable vendor on PATH.
4. Else a question before the salon, WHO LEADS?: `j k` choose, `enter` this time, `p` makes it the primary, `esc` quits.

`--lead ask` forces the question. `:lead VENDOR` sets the primary from the next start on; `:lead ask` forgets it. A primary that is not on PATH is said on the question screen, never replaced silently. The leader's vendor is also what bare `--model` and `--tier` refer to.

## Names

- The leader of the project imece started in is `lead`; a project opened with `:open PATH NAME` gets `lead-NAME`, and `:rename NAME` renames the leader with its project.
- Workers are `<vendor>-N`: `claude-1`, `claude-2`, `codex-1`, `gemini-1`. The number counts that vendor's workers, those that left included, and skips a name still taken.
- Helpers are `<worker>.N`: `claude-1.1` is the first helper of `claude-1`.
- You write to one agent with `@name text`; `you` is how an agent addresses you (`send(to: "you")`).

Agents address each other by these names on the bus; a `send` to a name that is not on the same project's team is refused with the team list.

## On the screen

```text
 SALON
 ◆ lead            coordinating
 ● claude-1        #12 parser: tokens      ⎇ ↑1 +34 −8   ▾ 1 helper
   └ ● claude-1.1  helper of claude-1
 ○ codex-1         idle
 ✓ gemini-1        done, not seen yet
```

Workers that need you come first (stuck, then waiting for you, then done and not seen: the [state ladder](../screen/salon.md#the-state-ladder)); helpers sit under their worker and fold with `h` `l`. The header counts the fleet: `● 2 working ✓ 1 done ✕ 1 stuck`.

## See also

- [How imece works](../concepts/how-imece-works.md)
- [Messages and the board](../concepts/messages-and-board.md)
- [Vendors](../concepts/vendors.md): what each tier runs as, and the confine levels.
- [Projects](../concepts/projects.md): a leader per project.
- [The salon](../screen/salon.md) and [the CLI reference](../reference/cli.md).
