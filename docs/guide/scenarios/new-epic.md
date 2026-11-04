---
title: "A new epic"
description: "Finishing a piece of work and starting the next on a clean screen: :archive, its grace and u, what the archive keeps and looking at it read-only, what the fresh project starts with, bringing it back with :restore, and what stays shared."
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

The salon shows `kule` again, with its `lead` alone and idle: a fresh project on the same checkout, with the same name, the same pin and the same `:copy` list, its leader on the same model. The journal has one new line, `› you archived kule as “v1 fixes”; a fresh kule takes its place`; the old lines, the old cards on the board and in the queue, the old items in WAITING FOR YOU are gone from the screen. `@lead` reaches the new leader.

Write to it as on the first day:

```text
/ the next epic: move the store to its own crate
```

The leader starts a new session. Workers it hires are numbered after the old ones (`claude-3`, not `claude-1` again), so their worktrees never meet the archive's.

## 3. What the archive keeps

Nothing of the archive was thrown away:

- its leader and workers, asleep, each with its session (to go on with), its mail, notes and change;
- its cards with their stories, its journal lines (the tidy keeps them whatever their age), its items that waited for you.

None of it wakes or spends: no card is offered to its workers and nobody reaches them by name. `imece --fresh` leaves their sessions alone. Claude Code keeps the sessions itself, and removes old transcripts after a while (30 days by default); a session gone by then starts anew, told where it left off.

To look at it again without bringing it back, press `A` and `enter` on `▸ v1 fixes`: the archive opens read-only, alone on the screen as it was put away, its whole journal there, under

```text
 ARCHIVE · kule “v1 fixes” · archived 2026-11-02 · 0 waiting for you now · esc back
```

Its conversations, cards and changes open as anywhere and `:export` writes one out; writing to its agents says `an archive is read-only · :restore brings it back · esc goes back`. `esc` goes back to the fresh `kule`, which went on meanwhile.

## 4. Bringing it back

A month later a bug in the old work turns up, and `kule`'s old team knew that code. Type:

```text
:restore v1 fixes
```

(`:restore ` and `tab` list the archives, newest first: `v1 fixes   kule · 2026-11-02`. Or press `A`: the archives as a tree, `kule`'s checkout first, with `● kule · open now` above `▸ v1 fixes · $6.10 · 4 cards done · 3 agents · archived 2026-11-02`; `l` shows its agents, `/fix` filters, `r` brings the one at hand back.) The archive opens again as it was put away, its `lead` selected, its cards, journal lines and waiting items back on the screen:

```text
 brought kule “v1 fixes” back: its agents sleep until you write to them
```

The `kule` that was open gives way: it did something (it had a session and a card), so it is archived in its turn under its name and days, and the journal says so: `› you brought back kule “v1 fixes”; the one open there is archived as “kule 2026-11-02 → 2026-12-02”`. A fresh `kule` that had done nothing would have been closed instead.

Nothing woke yet. Write to the old leader and it goes on in the session it had, told first that the project was put away and brought back, and of the turn the archive cut short (not run again). A session Claude Code removed meanwhile starts anew, told where it left off. To go back to the newer work, `:restore` it the same way.

## 5. What stays shared

The pot is one: archiving spends nothing and resets nothing. The header still says what all projects spent, and the daily ceilings of `:commit` still count the day. Other open projects are not touched; an archive is one project's.

## See also

- [Sessions and persistence: archives](../concepts/sessions.md#archives)
- [`:archive`](../reference/commands.md#archive-title)
- [`:restore`](../reference/commands.md#restore-name)
- [Projects](../concepts/projects.md)
- [Three checkouts, one screen](many-projects.md)
