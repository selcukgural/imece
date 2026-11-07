---
title: Solo sessions
description: A session you drive yourself, with no leader - :solo and --solo, the model and effort you pick, its own worktree, the Δ after every answer, helpers, a folder without git, and how solos and teams share a checkout.
---

# Solo sessions

A team is imece's usual shape: a leader reads your checkout, hires workers, and their changes come to you for review. A **solo** is the other shape: one agent you drive yourself, with no leader and no team, on the model and effort you name. It keeps what makes imece worth it (the worktree, the `Δ` gate, the pot, the team, the store), and leaves out what a single session does not need (the board, cards, hiring workers). Open as many as you like, next to teams: team, solo, team, solo, solo, each a group in the team.

## Opening one

| How | What it does |
| --- | --- |
| `:solo [PATH] [CHOICE] [NAME]` | from the screen, while imece runs. The words may come in any order: a word with a `/`, or starting with `~` or `.`, is the checkout's path (default: the project at hand's); a word that starts with `claude` is the choice; the word left is the project's name (default: the directory's, numbered `-2`, `-3` when taken). Without a choice, a screen asks the model and the effort, with `j k enter`. |
| `:effort LEVEL`, `:model NAME` | change what the solo at hand runs on, from its next turn; `default` is Claude Code's own. Its session starts again on the same transcript. |
| `imece --solo V[:MODEL][@EFFORT]` | from the command line: imece starts with a solo on the checkout instead of a team. An open solo of the same choice is gone on with, so the same command every morning continues the same session; a different choice opens another solo beside it. |

The **choice** is `claude[:MODEL][@EFFORT]`, each part after `claude` optional:

| Choice | Means |
| --- | --- |
| `claude` | Claude Code on its own model and effort (or what `--model` named for it) |
| `claude:opus` | the `opus` alias |
| `claude@xhigh` | its default model, thinking at `xhigh` |
| `claude:opus@max` | both |

The efforts are Claude Code's own: `low`, `medium`, `high`, `xhigh`, `max`. Any other is refused before anything starts, in words: `claude has no effort "bogus"; one of low, medium, high, xhigh, max` ([command line](../reference/cli.md)).

The flash says what opened: `opened notes at ~/src/kule as a solo session; write to solo-notes (claude opus@xhigh)`. The new solo is selected, so what you type next goes to it.

## What a solo is

- **A project of its own.** The team shows it as a group with one agent, `solo-NAME` (`solo` when it is the project imece started in). Its row names the choice: `○ solo-notes  opus@xhigh  idle`. `:close`, `:rename` (the agent becomes `solo-NEWNAME`), `:pin` and `:copy` work as for any project.
- **Its own worktree.** Like a worker, a solo writes in a git worktree of the checkout under imece's state directory, never in your checkout. Two solos on one checkout, and your own edits, never collide. The worktree is named after the project's id (`solo-3`), so renaming the project keeps it. Its row shows where the worktree stands (`⎇ branch ↑own ↓checkout +N −M`) as a worker's does.
- **The worker's tools.** Under the confine dial a solo gets what a worker gets: read, write, shell, web search and fetch at `standard`, file tools and imece's `run` at `strict`, your own setup at `open` ([Claude Code](claude-code.md)). On the bus it has `send` (to you, and to its helpers), `run`, and `hire` for helpers. No `post`, `board` or `claim`: the board is a team's.
- **The Δ after every answer.** Nobody sends `done` for a solo. Every turn that ends with an answer to you has imece read its worktree; when the tree differs from your HEAD, the change waits in TO DECIDE as a `Δ` titled by the answer's first line, and `a` applies it to your checkout, `r` rejects it, `c` and `n` send notes, `x` chooses hunks, exactly as for a worker ([changes](changes.md)). A newer answer's change takes the older one's place. A turn that ends with nothing to say (a `/command`) reads nothing.
- **Helpers.** A solo may hire helpers (`solo-notes.1`), up to `--helpers`, as a worker does: a coding agent in its worktree, for a part that can run beside its own work. They are told they work for an agent that works alone with the person.
- **The one pot.** A solo's turns are paid from the same pot as every team's, and Claude Code's usage window shows in the header like any session's ([budget and limits](budget-and-limits.md)).
- **Remembered.** A solo, its session and its choice are in the store like any agent: it comes back asleep, told where it left off ([sessions](sessions.md)).

## In a folder without git

A solo also runs in a folder that is not a git checkout, or one where `git init` ran but nothing is committed yet: a new project before its first file, or a folder you keep without git. There it works **straight in the folder**: no worktree, so what it writes is in your files at once, and no `Δ` comes after its answers (there is no base to compare with, and nothing to apply). Its helpers work there too. The project's header says it in the warning colour, `▾ notes  ⌀ no git`, and the journal says it at the open and at every start: `solo-notes works straight in ~/src/notes: no git here, so its edits are not reviewed as a Δ (with a first commit, the next start gives it a worktree)`. The agent is told the same, and to run `git init` or commit only when you ask. Nothing of git runs on such a folder: no worktree stand on its row, no file completion, `:commit` says `this folder is not a git checkout with a commit`, and `:copy` has nothing to copy into. It does not need git installed at all.

Once the folder has a first commit (the agent can make it when you ask: `git init`, then a commit), the next start, or closing the solo and opening it again, gives it a worktree and the `Δ` like any solo.

A **team** does not start in such a folder: `imece` there says `~/src/notes is not a git checkout with a commit: a team works in git worktrees. A solo works here without git (--solo claude, :solo); or run git init && git commit --allow-empty -m init first`, and `:open` says the same. A team stored on a folder that lost its git closes at the next start. Running teams without git is [issue #162](https://github.com/selcukgural/imece/issues/162).

## Solos and teams on one checkout

One checkout holds at most one open **team** (a second `:open` of it says `that checkout is open already, as kule`) and any number of **solos**. The directory's name takes a number when a project has it already: `kule`, `kule-2`, `kule-3`. A closed solo is reused by the next solo on that checkout, never by a team, and the other way round.

A bare `imece` opens a team on the checkout when none is open there, even when solos are: solos do not stand in for the team. `imece --solo …` opens or continues a solo and leaves an open team as it is, asleep until you write to it.

## What is not there yet

- A solo's answer while one of its helpers is still mid-turn stays in its story (as a leader's does) and comes to you, with its `Δ`, when the next answer does.

## See also

- [Team](team.md): the roles, and what a solo shares with a worker
- [Projects](projects.md): groups in the team, `:close`, `:rename`, `:pin`
- [Changes](changes.md): the `Δ`, apply, reject, notes
- [Command line](../reference/cli.md): `--solo`, `--model`, `--tier`
- [Commands](../reference/commands.md): `:solo`
- [A solo beside the team](../scenarios/solo-beside-the-team.md): a walk through it
