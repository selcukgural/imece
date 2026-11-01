---
title: Solo sessions
description: A session you drive yourself, with no leader - :solo and --solo, the vendor, model and effort you pick, its own worktree, the Δ after every answer, helpers, and how solos and teams share a checkout.
---

# Solo sessions

A team is imece's usual shape: a leader reads your checkout, hires workers, and their changes come to you for review. A **solo** is the other shape: one agent you drive yourself, with no leader and no team, on the vendor, model and effort you name. It keeps what makes imece worth it (the worktree, the `Δ` gate, the pot, the salon, the store), and leaves out what a single session does not need (the board, cards, hiring workers). Open as many as you like, next to teams: team, solo, team, solo, solo, each a group in the salon.

## Opening one

| How | What it does |
| --- | --- |
| `:solo [PATH] [CHOICE] [NAME]` | from the screen, while imece runs. The words may come in any order: a word with a `/`, or starting with `~` or `.`, is the checkout's path (default: the project at hand's); a word that starts with a vendor's name is the choice; the word left is the project's name (default: the directory's, numbered `-2`, `-3` when taken). Without a choice, a screen asks the vendor, the model and the effort, with `j k enter`. |
| `:effort LEVEL`, `:model NAME` | change what the solo at hand runs on, from its next turn; `default` is the vendor's own. Its session starts again on the same transcript. |
| `imece --solo V[:MODEL][@EFFORT]` | from the command line: imece starts with a solo on the checkout instead of a team. An open solo of the same vendor and choice is gone on with, so the same command every morning continues the same session; a different choice opens another solo beside it. Without `--lead`, the solo's vendor leads the teams `:open` adds, so no WHO LEADS? question comes. |

The **choice** is `VENDOR[:MODEL][@EFFORT]`, each part after the vendor optional:

| Choice | Means |
| --- | --- |
| `claude` | Claude Code on its own model and effort (or what `--model` named for it) |
| `claude:opus` | the `opus` alias |
| `claude@xhigh` | its default model, thinking at `xhigh` |
| `claude:opus@max` | both |
| `codex:gpt-5.6-terra@high` | Codex on that model at `high` reasoning effort |
| `gemini:pro` | Gemini CLI on `pro`; Gemini has no effort setting, `gemini:pro@high` is refused |

The efforts are each vendor's own: Claude Code's `low`, `medium`, `high`, `xhigh`, `max`; Codex's `low`, `medium`, `high`, `xhigh`. An effort the vendor lacks is refused before anything starts, in words: `claude has no effort "bogus"; one of low, medium, high, xhigh, max` ([command line](../reference/cli.md)). A vendor that is not on PATH is refused too.

The flash says what opened: `opened notes at ~/src/kule as a solo session; write to solo-notes (claude opus@xhigh)`. The new solo is selected, so what you type next goes to it.

## What a solo is

- **A project of its own.** The salon shows it as a group with one agent, `solo-NAME` (`solo` when it is the project imece started in). Its row names the vendor and the choice where a leader's row names the vendor: `○ solo-notes  claude opus@xhigh  idle`. `:close`, `:rename` (the agent becomes `solo-NEWNAME`), `:pin` and `:copy` work as for any project.
- **Its own worktree.** Like a worker, a solo writes in a git worktree of the checkout under imece's state directory, never in your checkout. Two solos on one checkout, and your own edits, never collide. The worktree is named after the project's id (`solo-3`), so renaming the project keeps it. Its row shows where the worktree stands (`⎇ branch ↑own ↓checkout +N −M`) as a worker's does.
- **The worker's tools.** Under the confine dial a solo gets what a worker gets: read, write, shell, web search and fetch at `standard`, file tools and imece's `run` at `strict`, your own setup at `open` ([vendors](vendors.md)). On the bus it has `send` (to you, and to its helpers), `run`, and `hire` for helpers. No `post`, `board` or `claim`: the board is a team's.
- **The Δ after every answer.** Nobody sends `done` for a solo. Every turn that ends with an answer to you has imece read its worktree; when the tree differs from your HEAD, the change waits in WAITING FOR YOU as a `Δ` titled by the answer's first line, and `a` applies it to your checkout, `r` rejects it, `c` and `n` send notes, `x` chooses hunks, exactly as for a worker ([changes](changes.md)). A newer answer's change takes the older one's place. A turn that ends with nothing to say (a `/command`) reads nothing.
- **Helpers.** A solo may hire helpers (`solo-notes.1`), up to `--helpers`, as a worker does: a coding agent in its worktree, for a part that can run beside its own work. They are told they work for an agent that works alone with the person.
- **The one pot.** A solo's turns are paid from the same pot as every team's, and its vendor's usage window shows in the header like any session's ([budget and limits](budget-and-limits.md)).
- **Remembered.** A solo, its session and its choice are in the store like any agent: it comes back asleep, told where it left off ([sessions](sessions.md)).

## Solos and teams on one checkout

One checkout holds at most one open **team** (a second `:open` of it says `that checkout is open already, as kule`) and any number of **solos**. The directory's name takes a number when a project has it already: `kule`, `kule-2`, `kule-3`. A closed solo is reused by the next solo on that checkout, never by a team, and the other way round.

A bare `imece` opens a team on the checkout when none is open there, even when solos are: solos do not stand in for the team. `imece --solo …` opens or continues a solo and leaves an open team as it is, asleep until you write to it.

## What is not there yet

- A solo's answer while one of its helpers is still mid-turn stays in its story (as a leader's does) and comes to you, with its `Δ`, when the next answer does.

## See also

- [Team](team.md): the roles, and what a solo shares with a worker
- [Projects](projects.md): groups in the salon, `:close`, `:rename`, `:pin`
- [Changes](changes.md): the `Δ`, apply, reject, notes
- [Command line](../reference/cli.md): `--solo`, `--model`, `--tier`
- [Commands](../reference/commands.md): `:solo`
- [A solo beside the team](../scenarios/solo-beside-the-team.md): a walk through it
