---
title: "A new epic"
description: "Finishing a piece of work and starting the next on a clean screen: :archive, its grace and u, what the archive keeps, what the fresh project starts with, and what stays shared."
---

# A new epic

A week of work on `kule` is done: the issues are fixed, the changes applied, the leader has nothing left to do. The salon still shows the two workers who did it, the journal is full of their talk, the board of their cards. The next piece of work is unrelated, and you want to start it on a clean screen without throwing the old one away.

## 1. Archive the project

With a row of `kule` selected, type:

```text
:archive v1 fixes
```

Nothing happens yet. For ten seconds the bottom line counts down:

```text
 ◷ archiving kule as “v1 fixes” in 7 s · u keeps it
```

`u` (or `:undo`) in that time leaves everything as it was: `kule stays as it is`. If an agent of `kule` is still mid-turn the line says what you are about to cut short:

```text
 ◷ archiving kule as “v1 fixes” in 7 s, cutting 1 turns short · u keeps it
```

An archive stops its agents as a quit does: the turn under way ends and its prompt is kept. Without a title the archive is called by its name and its days, `kule 2026-10-26 → 2026-11-02`.

## 2. A clean kule

When the count ends:

```text
 archived kule as “v1 fixes”: its agents sleep with all they had; a fresh one opens
```

The salon shows `kule` again, with its `lead` alone and idle: a fresh project on the same checkout, with the same name, the same pin and the same `:copy` list, its leader on the same vendor and model. The journal has one new line, `› you archived kule as “v1 fixes”; a fresh kule takes its place`; the old lines, the old cards on the board and in the queue, the old items in WAITING FOR YOU are gone from the screen. `@lead` reaches the new leader.

Write to it as on the first day:

```text
/ the next epic: move the store to its own crate
```

The leader starts a new session. Workers it hires are numbered after the old ones (`claude-3`, not `claude-1` again), so their worktrees never meet the archive's.

## 3. What the archive keeps

Nothing of the archive was thrown away:

- its leader and workers, asleep, each with its vendor session (to go on with), its mail, notes and change;
- its cards with their stories, its journal lines (the tidy keeps them whatever their age), its items that waited for you.

None of it wakes or spends: no card is offered to its workers and nobody reaches them by name. `imece --fresh` leaves their sessions alone. The vendors keep the sessions themselves, and Claude Code removes old transcripts after a while (30 days by default); a session gone by then starts anew, told where it left off.

For now the archive is kept, not shown: looking at it and bringing it back are the next steps.

## 4. What stays shared

The pot is one: archiving spends nothing and resets nothing. The header still says what all projects spent, and the daily ceilings of `:commit` still count the day. Other open projects are not touched; an archive is one project's.

## See also

- [Sessions and persistence: archives](../concepts/sessions.md#archives)
- [`:archive`](../reference/commands.md#archive-title)
- [Projects](../concepts/projects.md)
- [Three checkouts, one screen](many-projects.md)
