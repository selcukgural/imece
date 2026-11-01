---
title: "A solo beside the team"
description: "Opening a session you drive yourself next to the team: :solo with a vendor, model and effort, the Δ after every answer, a second solo on Codex, helpers, and --solo the next morning."
---

# A solo beside the team

Sometimes you do not want a leader between you and the work: one agent, the model and thinking depth you choose, and you say what to do. A **solo** is that, inside imece: it keeps the worktree, the `Δ` review, the pot and the salon, and leaves out the board and the hiring. This scenario opens one next to a running team, then a second on another vendor, and comes back to it the next day.

imece was started in `~/code/kule` as usual: the team is `lead` and the two workers it hired. Claude Code and Codex are on PATH.

## 1. Open a solo

Type the command, with the vendor, model and effort in one word:

```text
 › :solo claude:opus@xhigh notes
```

The words may come in any order; imece tells them apart by shape. `claude:opus@xhigh` starts with a vendor's name, so it is the choice: Claude Code on the `opus` alias, thinking at `xhigh`. `notes` is neither a path (no `/`, `~` or `.`) nor a vendor, so it is the project's name. No path means the checkout of the project at hand, `kule`. The flash answers:

```text
 opened notes at /Users/me/code/kule as a solo session; write to solo-notes (claude opus@xhigh)
```

The salon shows a new group beside the team, with one agent, and it is selected:

```text
 ▾ kule
   ◆ lead             claude          coordinating                    $0.42
   ● claude-1         #3 limiter: config v2     editing src/limit.rs
   ○ codex-1          #4 limiter tests          idle
 ▾ notes
   ○ solo-notes       claude opus@xhigh         idle
```

Where a leader's row says its vendor, a solo's says its vendor and choice. The glyph is its status, not the leader's `◆`: a solo works, it does not coordinate. Nothing costs anything yet: like every agent, it has no process until you write to it.

## 2. Tell it what to do

Press `/`. The command line is addressed to the selected project's head, which here is the solo itself:

```text
 › to solo-notes: write docs/limits.md: what the rate limiter does, from src/limit.rs
```

Its first turn starts. Before it, imece makes `solo-notes` a worktree of `kule` under `~/.local/state/imece/worktrees/`, named `solo-3` after the project's id, detached at your HEAD. The solo gets what a worker gets under `--confine standard`: Claude Code's read and edit tools, its shell (what would ask comes to you as a `⚑`), the web, and imece's `run` tool. Its row shows what it does:

```text
   ● solo-notes       claude opus@xhigh         reading src/limit.rs          $0.31
```

## 3. The answer, and the Δ with it

The turn ends with an answer. It goes to the journal and WAITING FOR YOU as any agent's answer does, and, because this is a solo, imece reads its worktree at the same moment:

```text
 WAITING FOR YOU
 ↩ solo-notes answered: Wrote docs/limits.md from src/limit.rs: the two windows, the per-key…
 Δ solo-notes: Wrote docs/limits.md from src/limit.rs   no run on this tree
```

Nobody sends `done` for a solo. Every turn that ends with an answer shows you what its worktree holds, titled by the answer's first line. `enter` on the `Δ` opens the diff; `a` applies it to your checkout, `r` rejects it and opens `@solo-notes ` for your reasons, `c` and `n` send notes on lines, `x` chooses hunks ([changes](../concepts/changes.md)). A turn that answers a question without touching a file reads the same tree again: the `Δ` stays, retitled.

You read the diff and send a note instead of applying:

```text
 › @solo-notes the burst window is 10 s, not 60; fix and shorten the intro
```

The next answer brings the next `Δ`, with the fix. `a` applies it. The journal says `you applied solo-notes's change`, and the worktree is marked so its next change holds only what comes after.

## 4. A second solo, on Codex

A different question, a different model. This time a path and no name:

```text
 › :solo codex:gpt-5.6-terra@high ~/code/kule
```

`~/code/kule` is a path (it has a `/`), the choice names Codex at `high` reasoning effort, and the name defaults to the directory's: `kule` is taken by the team, so the project is `kule-2` and its agent `solo-kule-2`:

```text
 ▾ kule
   ◆ lead             claude          idle
   ○ claude-1         #3 limiter: config v2     idle
   ○ codex-1          #4 limiter tests          idle
 ▾ notes
   ○ solo-notes       claude opus@xhigh         idle                          $0.58
 ▾ kule-2
   ○ solo-kule-2      codex gpt-5.6-terra@high  idle
```

Three groups on one checkout: a team and two solos, each in its own worktree, your checkout untouched by all of them. One pot pays for all; the header's `$/h` counts them together. A second *team* on the same checkout would be refused (`that checkout is open already, as kule`): one team per checkout, any number of solos.

A bare `:solo` asks instead of guessing: a screen lists the vendors imece can run here, then the model, then the effort; `j k` and `enter` pick, `esc` goes back a step. And a running solo can be re-tuned: `:effort max` makes `solo-notes` think at `max` from its next turn (the flash: `solo-notes runs on claude opus@max from its next turn; its session starts again on it`), `:model sonnet` moves it to a cheaper model for the small remaining fixes; the session starts again on the same transcript, nothing of the conversation is lost.

A choice imece cannot honour is refused before anything starts. `:solo gemini:pro@high` says `could not open: gemini has no effort setting; name only a model, not @high`, and `:solo claude@turbo` says `could not open: claude has no effort "turbo"; one of low, medium, high, xhigh, max`.

## 5. A helper

A solo may split its work as a worker does. Told to also add the tests, `solo-notes` calls `hire` and the journal says so:

```text
 15:02 ◆ solo-notes hired solo-notes.1 (default model)
 15:02 ⇄ solo-notes → solo-notes.1 [assign]: tests for the burst window in tests/limit.rs
```

The helper works in the solo's worktree, on the files the solo named, and leaves when its `done` turn ends; its row hangs under the solo's, as a worker's helper does. Up to `--helpers` (default 2) at once. What a solo does not have is the board: no `post`, `board` or `claim`, so nothing of the team's cards reaches it and nothing of its work reaches the team.

## 6. The next morning

Quit with `q q`. The solos are in the store like every agent, with their sessions and their choices. Tomorrow:

```sh
cd ~/code/kule
imece --solo claude:opus@xhigh
```

`--solo` looks for an open solo on this checkout with the same vendor and choice, finds `notes`, and makes it the project at hand: the same session continues, told where it left off. The team and `kule-2` come back asleep beside it, until you write to them. Had you typed `--solo claude:sonnet`, a third solo would have opened, `kule-3`. Had you typed a bare `imece`, the team would have been the project at hand: solos do not stand in for it.

Without `--lead`, the solo's vendor leads the teams `:open` adds, so no WHO LEADS? question comes before the salon.

## What this scenario used

| Key or command | What it did |
| --- | --- |
| `:solo claude:opus@xhigh notes` | a solo on Claude's opus at xhigh, named `notes`, on the checkout at hand |
| `:solo codex:gpt-5.6-terra@high ~/code/kule` | a second solo, named after the directory with a number |
| `/`, `@solo-notes` | writing to the solo, like to a leader |
| `Δ`, `enter`, `a`, `r`, `c`, `n`, `x` | the change after every answer, reviewed as a worker's |
| `imece --solo claude:opus@xhigh` | the next morning: the same solo, continued |

## See also

- [Solo sessions](../concepts/solo.md)
- [Changes](../concepts/changes.md)
- [Command line](../reference/cli.md): `--solo`
- [Three checkouts, one screen](many-projects.md): teams side by side
