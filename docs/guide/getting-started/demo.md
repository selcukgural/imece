---
title: "The demo"
description: "`imece --demo` runs the whole screen on fake agents, at no cost, mirroring what live does."
---

# The demo

`imece --demo` runs the screen on a seeded simulation instead of real agents: projects with leaders and workers from several vendors that post, claim and assign cards, talk on the bus, ask you questions, wait for your yes, hire helpers and finish with changes that wait for you. No vendor CLI is started and nothing costs money. You meet it when you want to see the screen at work before spending a cent, or when you want to learn the keys.

```sh
imece --demo
imece --demo --projects 3 --agents 30 --seed 12
imece --demo --lang tr --theme nord
```

## Flags

| Flag | Range (default) | What it does |
| --- | --- | --- |
| `--demo` | | fake agents instead of live ones |
| `--projects N` | 1-4 (2) | projects worked on at once, each with a leader of its own |
| `--agents N` | 1-200 (24) | workers next to the leaders, spread over the projects |
| `--researchers N` | 1-32 (4) | researchers that may run at once, a pool shared by all projects (a preview: researchers are not built live) |
| `--seed N` | (7) | replays the same scenario; a different seed, a different story |
| `--lang en\|tr` | (en, or `IMECE_LANG`) | the screen's language; `L` switches while running |
| `--theme NAME\|FILE` | (terminal) | a theme, as live takes it |
| `--awake` | off | holds the machine's sleep while a fake agent is mid-turn, as live does |

`--projects`, `--researchers` and `--seed` belong to the demo alone: live imece refuses them with `--projects is for --demo`. The demo's clock starts at 14:00 on today's date. The demo shares the store's screen settings (theme, side panel width, the primary vendor) with live but writes nothing else: no projects, no journal, no pot.

## Speed

Live agents run in real time; the simulation has a clock you can hold.

| Key | `:` name | What it does |
| --- | --- | --- |
| `space` | `:pause` | pause the simulation; again: resume. The header shows `‖ PAUSED` |
| `+` | `:faster` | faster |
| `-` | `:slower` | slower |

The `:` menu shows the speed after the command (`faster (×2)`) and whether it is paused. Typed live, these three say `:pause is for the demo: live agents run in real time`; they are left out of the live `:` menu.

## What it shows

The simulation produces live's events through the same model calls live uses, so the screen is the real screen:

- **The leader's work**: it posts cards (at the start too), answers your request with the cards it posted (a reply `»` in WAITING FOR YOU), assigns a card to a free worker or lets one claim it from the board, hires a worker when two cards wait and nobody is free (up to six), and a tiered hire falls back to the default model now and then.
- **Workers**: typed messages on the bus (`assign`, `ask`, `answer`, `progress`, `done`), what they assume told to you as `inform`, what they touch shown in the card's story, a helper hired for the tests that sends its `done` and leaves.
- **Approvals (`⚑`)**: a worker's session asks your yes for one of the project's sensitive steps; the item is held until you decide (`a` allow, `r` refuse) or given up on after 100 seconds, as Claude Code's own prompt is.
- **Questions (`?`)**: a worker asks you something; it waits a while, then goes on with what does not depend on the answer. `enter` opens the conversation at the question; writing to the worker answers it.
- **Changes (`Δ`)**: a card ends with imece's `run` of the build and the tests (one test run in six fails and the worker goes back), a `done` to the leader and a change in WAITING FOR YOU with a patch made from the card's files and the runs on that tree. `enter` opens the diff; `a` applies it on the next step (one apply in eight fails with git's words, as live's can), `r` rejects it and your reasons become the worker's next card; `c` `n` notes, `x` partial apply, `>` quoting and `w` wrapping all work as live.
- **Sessions**: a session starts once (with its vendor's `/` commands; a worker's may say its worktree was moved or kept, or that the repository's notes were cut), stops now and then (`■ stopped, write to it to start again`), is forgotten with `:forget` or `/clear` and restarted when told; `/cost` and `/compact` answer as a turn; a Codex agent's second `/compact` with no turn between is refused as a failed turn.
- **The pot and the limits**: spending per agent and per hour, the pot running low, a spent pot refusing words and commands; each vendor's usage window in the header, filled from what its agents spent.
- **Projects**: `:open PATH [NAME]` adds a fake project with a leader, two workers of random vendors and three cards; `:close` sends its agents away; `:rename` as live. `:pin` too.
- **The whole screen**: the salon, folding, `f`, the board (`2`), the code map (`3`) with leases, cards (`#N`), conversations with turns and replies (`↩`), marks, the journal, `W` `S`, the quit question, the help, themes, both languages, the readline keys, multi-line writing and paste. `Confine` is said at the start as live says it.

## The demo mirrors live

Every fix, feature or change to what live does is shown by the demo in the same change, with its test; a change that cannot fake something says so. That is a standing rule of the project, so what you see in the demo is what live does, drawn by the same code. Where the demo once had things live did not (a merge queue, trunk numbers, a verifying status), they were taken out.

## What cannot be faked

- **The "where you left off" window**, turns cut short by a close, and agents asleep from a start: they need a store and an earlier run, and the demo keeps none.
- **Researchers, symbol leases and the code map's leases, and the Qwen and Local vendors**: the demo shows them as a preview of what is planned; live has none of them yet. Live's code map says `no shared trunk yet: each agent works in its own worktree, so there are no leases to show`.
- **Two parts of the screen nothing fills any more**: the salon's QUEUE line and the board's VERIFYING column stayed when the merge queue left.

## See also

- [First run](first-run.md)
- [Keyboard](../screen/keyboard.md)
- [The salon](../screen/salon.md)
- [Command line reference](../reference/cli.md)
- [Videos](../videos.mdx)
