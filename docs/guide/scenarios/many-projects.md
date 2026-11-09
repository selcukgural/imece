---
title: "Three checkouts, one screen"
description: "Several projects in one imece, each with its own leader: opening, folding, pinning, filtering, renaming, copying ignored paths, closing, and the pot they share."
---

# Three checkouts, one screen

One imece runs several checkouts at once. Each is a project in the team with a leader of its own; they share one pot, one bus and one screen. You meet this when the work spans repositories, or when you keep a long-running project open beside the one you are in. This scenario opens two more projects beside the one imece started in, and uses the keys that keep three teams readable.

imece was started in `~/code/cli`; its project is named after the directory, `cli`, with the leader `lead`.

## 1. Open two more

Type `:open ~/code/api api` and press `enter`. The flash says `opened api at /Users/you/code/api; write to lead-api`; the journal has `› you opened api at /Users/you/code/api`. `~` stands for your home directory. The second project's path is relative to where imece was started, and its name is left to the directory:

```text
 › command: :open ../web
```

```text
 15:00 › you opened web at /Users/you/code/web
```

`:open PATH [NAME]` opens the git checkout at PATH; NAME defaults to the directory's name made safe (1 to 24 of letters, digits, `.`, `_`, `-`). The new project's leader is `lead-NAME`, with the same model and tiers. `:open ` completes directories as you type (hidden ones after a `.`, `~` works). The `:` menu marks `:open` with `$`: a new leader is a new session that costs when you first write to it. A path that is not a checkout, a name taken or a checkout already open is refused with why (`api is open already, as api`). `:o` is short for it.

The header names the open projects. Home's AT WORK groups its rows by project, under a heading each, and a row carries its project's name when more than one project is open. A project with nobody on the list has no heading: a leader with nothing to do has no row, so `api` and `web` show only in the header until their leaders work:

```text
 imece  cli · api · web │ $2.10 of $50.00  15:00
 AT WORK 1
▸▾ cli           ▓▓▓▓░░░░ 1/2  1 working  $2.10  ~/code/cli
 ● cli         #2   docs: man page            writing docs/imece.1 · 4 min
```

`/` writes to the leader of the project at hand: narrow Home to `api` with `]` (step 4), or select one of its rows, then `/`. `@lead-web text` writes to one leader from anywhere.

## 2. Fold and unfold

Later `lead-web` hires a worker, and `web` has a heading of its own. With a row of `web` at hand (its heading or one of its agents), press `h` (or `←`). The project folds to its heading; `l` unfolds it. `za` toggles, `zM` folds every project, `zR` unfolds all (`:fold`, `:unfold`). `enter` on a heading opens the leader's detail instead. A folded project is quiet, not deaf: its heading still counts what waits for you, and what is to decide stays in TO DECIDE above.

```text
 ▸ web            ! 1 waiting   1 working  $0.80
```

`s` on a heading hides the whole project from AT WORK until it needs you (`:unsettle` brings it back); the AT WORK heading counts what is hidden.

## 3. Pin the one you live in

Type `:pin api` and press `enter`: `api is pinned to the top of the team`. Its heading wears `▴` and it stays first whatever the order of opening; it also folds last. `:pin` alone pins the project at hand; `:pin` again unpins (`api is not pinned any more`). The pin is kept in the store across runs, and a closed project keeps its pin for when it is opened again.

```text
 ▾ ▴ api         2 working  $3.10
```

A colour per project is not built yet.

## 4. One project at a time

Press `]`. Home narrows to one project: the header lights its name, and TO DECIDE and AT WORK show only its rows. `]` again goes to the next project, and after the last back to all of them; `[` goes the other way. The narrowing is for this run only. `W` still opens the next item to decide from anywhere.

## 5. Rename

With a row of `api` selected, `:rename backend` and `enter`: `api is now backend; write to lead-backend`. The leader is renamed with it, sessions go on, and every live agent of the project gets a note at its next prompt; a `send` to the old name answers with the team list. The journal: `› you renamed api to backend`. Project 0 can be renamed too (its leader becomes `lead-NAME`).

## 6. Copy what git ignores

A worker's worktree is a fresh checkout, so what git ignores is not there: `.env`, `node_modules`, `target/`. For `web`, select one of its rows and type `:copy .env`:

```text
 .env is copied into web's worktrees that lack it, at a worker's start
```

The list is per project and empty by default. `:copy .env` again drops the path; `:copy` alone says the list (`copied into web's worktrees: .env, node_modules`, or `nothing is copied into web's worktrees; :copy PATH adds a path git ignores`). The `:` menu shows the list after the command. A path must be relative, literal (no glob), present in the checkout, not a link, inside the checkout once resolved, and ignored by git; otherwise it is refused with why:

```text
 src/main.rs can not be copied: git does not ignore it: a tracked file is in the worktree already
```

When `lead-web` hires a worker, the listed paths the new worktree lacks are copied before the session starts (`cp -R -P -p`, an APFS clone on macOS, a reflink where the volume has it), all of one start within 2 GiB. The journal says what moved and what was skipped; a path already there says nothing:

```text
 15:12 ⚑ copied .env, node_modules into claude-2's worktree (412 MiB)
 15:12 ■ target was not copied into claude-2's worktree: more than what is left of the copy budget (1715470336 bytes)
```

Helpers share their worker's worktree and get the same; a leader works in the checkout and never gets a copy. A file you change in the checkout later is not copied again.

## 7. Researchers

A researcher layer (agents that read the outside world for the team, under a cap shared by every project) is designed but not built: live imece shows no researcher count in the header. `imece --demo --researchers N` previews it: in the demo the cap is one pool for all projects.

## 8. Close one

Type `:close web` and press `enter`:

```text
 closed web: its agents left; their worktrees stay on disk
```

Its agents' sessions and runs end, their mail, offers and changes are dropped, their inbox items close; the worktrees stay under `~/.local/state/imece/worktrees/`. `:close` alone (or `:c`) closes the project at hand. The last open project refuses: `cli is the last open project; q q quits`. The same checkout opened again later reuses its place with a new leader. The journal: `› you closed web`.

## 9. One pot for all

The header has one pot for every project: `$5.90 of $50.00`. With less than half an hour left at the current rate it turns the attention colour and adds how long it lasts (`~25 min left`); the rate itself is in the `:` menu's note on `:budget` (`$5.90 of $50.00 · $8.20/h, ~5 h 20 min left`). `:budget 80` raises it for all of them; each project's heading on Home shows what its own agents spent. When the pot is nearly spent, the journal says so once, under the first open project's leader:

```text
 15:40 ! at $8.20/h the budget lasts about 25 min more; :budget USD raises it
```

Once the pot is spent, nobody in any project takes another turn. One imece at a time holds the store; a second one started elsewhere is told to use `:open` from the first.

## See also

- [Projects](../concepts/projects.md)
- [Budget and limits](../concepts/budget-and-limits.md)
- [Home](../screen/home.md): TO DECIDE, AT WORK, folding, hiding
- [Commands](../reference/commands.md)
- [Files](../reference/files.md): where worktrees and the store live
