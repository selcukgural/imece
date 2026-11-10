---
title: "The demo"
description: "`imece --demo` runs the whole screen on fake agents, at no cost, mirroring what live does."
---

# The demo

`imece --demo` runs the screen on a seeded simulation instead of real agents: projects with leaders and workers that post, claim and assign cards, talk on the bus, ask you questions, wait for your yes, hire helpers and finish with changes that wait for you. Claude Code is not started and nothing costs money. You meet it when you want to see the screen at work before spending a cent, or when you want to learn the keys.

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

`--projects`, `--researchers` and `--seed` belong to the demo alone: live imece refuses them with `--projects is for --demo`. The demo's clock starts at 14:00 on today's date. The demo shares the store's screen settings (theme, notify, motion) with live but writes nothing else: no projects, no journal, no pot.

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

- **The leader's work**: it posts cards (at the start too), answers your request with the cards it posted (a reply in TO DECIDE), assigns a card to a free worker or lets one claim it from the board, hires a worker when two cards wait and nobody is free (up to six), and a tiered hire falls back to the default model now and then.
- **Rounds**: a leader hands out twelve cards by itself, then none until all of them are through and nothing waits for you, so Home comes to say `◐ TEAM FINISHED` and, after your last yes, `✓ ALL DONE` ([when the work is done](../screen/home.md#when-the-work-is-done)); two minutes later the next round begins. What you ask a leader for is posted whenever you ask. Left alone, a demo comes to rest with its changes waiting for you, and a worker whose session stopped holds its round until you write to it.
- **Workers**: typed messages on the bus (`assign`, `ask`, `answer`, `progress`, `done`), what they assume told to you as `inform`, what they touch shown in the card's story, a helper hired for the tests that sends its `done` and leaves. Your `@claude-2 …` to a worker between cards opens no card: it answers, and every second answer comes with a change, which opens its card, titled by your line; the others leave nothing to show, and the line ends there.
- **Approvals (`⚑`)**: a worker's session asks your yes for one of the project's sensitive steps; the item is held until you decide (`a` allow, `r` refuse) or given up on after 100 seconds, as Claude Code's own prompt is.
- **Questions (`?`)**: a worker asks you something; it waits a while, then goes on with what does not depend on the answer. `enter` opens the conversation at the question; writing to the worker answers it.
- **Changes (`Δ`)**: a card ends with imece's `run` of the build and the tests (one test run in six fails and the worker goes back), a `done` to the leader and a change in TO DECIDE with a patch made from the card's files and the runs on that tree. `enter` opens the diff; `a` applies it on the next step (one apply in eight fails with git's words, as live's can), `r` rejects it and your reasons become the worker's next card; `c` `n` notes, `x` partial apply, `>` quoting and `w` wrapping all work as live.
- **Sessions**: a session starts once (with Claude Code's `/` commands; a worker's may say its worktree was moved or kept), stops now and then (`■ session ended · write to it to restart`), is forgotten with `:forget` or `/clear` and restarted when told; `/cost` and `/compact` answer as a turn.
- **The pot and the limits**: spending per agent and per hour, the pot running low, a spent pot refusing words and commands; the usage window in the `:budget` note and, nearly full, in the header, filled from what the agents spent.
- **Projects**: `:open PATH [NAME]` adds a fake project with a leader, two workers and three cards; `:close` sends its agents away; `:rename` as live. `:pin` too.
- **Cards and their tries**: a card follows its change through the board's columns; every seventh first try fails and its leader hands the card to another worker, a card that keeps failing meets the breaker at its fourth try and waits for you (`a` one more, `r` fail it), a card that has run long is taken from its worker, whose turn then ends with a late `done`, some cards wait on others (`⧗ after #2`), and now and then the leader withdraws a card nobody took.
- **The repository's check**: every checkout of the demo has one; the first change asks for your yes to it, and it then fails on every third change.
- **The issue a card closes**: every third card names a made-up issue (`closes #41`); its change says so under the verdict, with the issue's title. On the project on Lonca it closes when the change lands (every seventh does not: `gh did not answer`), or asks you first after `:closes off`; elsewhere `:commit`'s message ends with `Closes #41`. Nothing leaves the machine.
- **Health**: every fourth failed test run goes round, and the leader is told of the loop. A worker that is blocked tells its leader: the leader sends most of them the way on, asks you about every third as a risky one (with what it tried and answers to pick), and lets every fifth lie so that it comes to you. A worker whose session ends now and then (every third such stop is a turn that fails instead) is its leader's the same way: the leader starts it again where it left off, asks you whether to give its card to another worker, or lets it lie; one that stops again after being started is not started a second time.
- **A whole team that stops**: by the clock, every 900 ticks (75 demo minutes) each team has taken its turns on messages since you last spoke, and hands out nothing until you write or press `1` on its stop; every 1,300 ticks its leader's session ends, and `1` starts it again where it left off. `:budget` under what was spent shows the pot's stop with the leader and the retries behind it. Each comes first on Home as `■ STOPPED` ([when a whole team stops](../screen/home.md#when-a-whole-team-stops)).
- **Processes**: `:ps` lists made-up processes under each session (its test program while it works), ends one after your `y`, and lists two left from an earlier run.
- **A project on Lonca**: with two projects or more, the second stands on a fake Lonca repository. Its workers propose; a stranger's proposal (`ayse`) waits beside theirs, one proposal in three is based before trunk, every fifth landing fails its tests in the queue, and every third card asks for the `api` check, which your yes asks for. Its line says how the repository stands, `:lonca` opens its window, `:sync` brings the checkout up (every third finds commits of your own), and the daemon stops answering for 6 ticks in 60.
- **The ledger**: `:ledger` fills as cards are applied, fail, are handed on and withdrawn; every figure reads `≈`, the demo's login being a plan's.
- **The whole screen**: Home with TO DECIDE by group and AT WORK by project, folding and hiding, the detail's tabs, the windows (`:board`, `:code` with leases, `:timeline`, `J`), cards (`#N`), conversations with turns and replies (`↩`), marks, `W` `S`, the quit question, the help, themes, both languages, the readline keys, multi-line writing and paste. `Confine` is said at the start as live says it.

## The demo mirrors live

Every fix, feature or change to what live does is shown by the demo in the same change, with its test; a change that cannot fake something says so. That is a standing rule of the project, so what you see in the demo is what live does, drawn by the same code. Where the demo once had things live did not (a merge queue, trunk numbers, a verifying status), they were taken out; its project on Lonca now shows what live does on Lonca.

## What cannot be faked

- **The "where you left off" window**, turns cut short by a close, and agents asleep from a start: they need a store and an earlier run, and the demo keeps none.
- **Researchers**: the demo shows them as a preview; live has none, and they are closed, not to be built (record 01).
- **The code map of a project in worktrees**: the demo draws leases from its cards on every project, as a preview; live has leases only on Lonca, where the map shows the daemon's, and a project in worktrees says `no shared trunk yet: each agent works in its own worktree, so there is nothing to show of who edits what`. The demo's Lonca project lists no held symbols in `:lonca`.
- **The board's VERIFYING column**: live cards stand there while Lonca's merge queue holds their change; the demo lands a proposal at once, so its column stays empty.

## See also

- [First run](first-run.md)
- [Keyboard](../screen/keyboard.md)
- [Home](../screen/home.md)
- [Command line reference](../reference/cli.md)
- [Videos](../videos.mdx)
