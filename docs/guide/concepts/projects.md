---
title: Projects
description: Several checkouts in one imece - :open, :close, :rename, a leader per project, groups in the salon that fold, :pin, and the per-project :copy list of ignored paths a worker's worktree gets.
---

# Projects

A project is one git checkout with its own leader and workers. imece starts with the checkout it runs in as the first project (leader `lead`); you open more from the screen, and each is a group in the salon. One pot and one bus serve them all.

## Opening, closing, renaming

| Command | What it does |
| --- | --- |
| `:open PATH [NAME]` (`:o`) | opens the git checkout at PATH as a project of its own, with a new leader `lead-NAME` on the team's lead vendor (same model and tiers). NAME defaults to the directory's name made safe: 1-24 of letters, digits, `.`, `_`, `-`, the rest replaced by `-`. The `:` menu marks it `$`: a new leader session. |
| `:solo [PATH] [CHOICE] [NAME]` | opens a solo session as a project of its own: one agent, `solo-NAME`, that you drive yourself, on the vendor, model and effort you name, with no leader ([solo sessions](solo.md)). Any number may share a checkout with its team. `$`: a new session. |
| `:close [NAME]` (`:c`) | ends the project's agents' sessions and runs, drops their mail, notes, offers and changes, closes their inbox items, marks the project closed. Worktrees stay on disk. No name: the project at hand. The last open project refuses to close (`q q` quits). |
| `:rename NAME` | renames the project at hand and its leader to `lead-NAME` (the first project's too). Sessions go on; every live agent of it gets a note at its next prompt, and a `send` to the old name answers with the team list. |

- A closed project keeps its id; opening the same checkout again reuses it, with a new leader (a closed solo is reused by the next solo on that checkout, never by a team). Its pin is kept for then.
- One checkout holds at most one open team (`that checkout is open already, as kule`) and any number of solos. The directory's name takes a number when a project has it: `kule`, `kule-2`.
- Names are global: `@lead-api` reaches that project's leader from anywhere; the completion after `@` lists the project at hand first.
- A PATH with spaces is not parsed (no quoting). `:open ` completes directories from the filesystem (hidden ones after a `.`, `~` for home); `:close ` and `:pin ` complete the open projects' names.
- The journal: `you opened api at /src/api`, `you closed api`, `you renamed api to gateway`.
- Projects are remembered in the store; a remembered project whose checkout is gone loads closed.

The lead vendor cannot be chosen per project: every leader runs on the team's lead vendor. A solo's vendor, model and effort are yours to pick, per solo.

## Groups in the salon

```text
 SALON
 ▴ api        ! 1 waiting
   ◆ lead-api        coordinating
   ● claude-1        #4 auth: refresh      +12 −3
 ▸ shop       ● 2 working
 ▾ imece
   ◆ lead            idle
   ✓ codex-1         done, not seen yet
```

- Each project is a group with its own leader. `h` `l` (or `←` `→`, `enter` on the header) fold and unfold a group; `za` toggles, `zM` folds every worker, `zR` unfolds all (`:fold`, `:unfold`).
- A folded project is one line that still shows what waits for you (`! 1 waiting`) or what it does (`● 2 working`).
- When the salon is too short, calm workers fold from the bottom up into `… N agents doing fine`; the workers that need you fold last, and a pinned project last of all.
- The journal and inbox lines carry the project's name before the agent's when several are open.
- Plain text in the command line goes to the **selected** project's leader. The selection decides who `/` writes to.

## `:pin [NAME]`

`:pin` pins the named project, else the one at hand, to the top of the salon; `:pin` again unpins. Pinned projects come first, then the rest, each group in the order it was opened. A pinned project's header wears `▴`. The pin is yours and outlives the run (kept in the store). The `:` menu shows which project `:pin` would act on, with `▴` when it is pinned already.

A colour per project is not built: the themes name colours by role and order their speaker colours differently, so a colour chosen by name would need a named palette in every theme first.

## `:copy PATH`: ignored paths a worktree needs

A worker's worktree is a fresh checkout, so what git ignores is not there: `.env`, `node_modules`, `target/`. Some projects need those in every worktree, and only you know which.

- `:copy PATH` adds a path of the project at hand to its list; `:copy PATH` again drops it; `:copy` alone says the list, and so does the `:` menu. A trailing `/` is the same path. The list is empty by default and kept with the project in the store.
- **What may be listed** (checked when added and again at each copy): a relative path of at most 200 bytes, literal (no glob, backslash or control character, no `.` or `..`, not inside `.git`); present in the checkout; not a link, and inside the checkout once its directories are resolved (a directory that links out is refused); and ignored by git (`git check-ignore`: a tracked file is in the worktree already).
- **When:** at each worker's start, right after its worktree is made or placed; helpers share their worker's worktree; never for a leader, which works in the checkout.
- **How:** a path the worktree already has (file, directory or link) is left as it is; imece never writes over the worker's copy, so a later start copies only what was added to the list since. Each path is measured first (links as themselves, never followed; at most 100 000 entries), and all paths of one start together stay within 2 GiB; a path that would pass what is left is skipped. The copy is `cp -R -P -p`: on macOS with `-c` (an APFS clone, almost free) and again without it where the volume cannot clone, elsewhere with `--reflink=auto`. A copy that fails leaves nothing half done; `cp` runs without a shell, with imece's cleared environment.
- The journal says what was copied in one line, `copied .env, target into claude-1's worktree (1.2 GiB)`, and each skipped path with why. A path that was there says nothing.

Known limits: a file you change in the checkout later is not copied again (the worktree has it); copying is per worker, not shared; the 2 GiB budget and the entry cap are constants.

## The demo's copies

`--demo --projects N` (1-4, default 2) shows several fake projects at once; `:open PATH [NAME]` there adds a fake project with a leader, two workers of random vendors and three cards, `:close` and `:rename` work the same, `:pin` pins without a store, and `:copy` keeps a list that a demo worker's start "copies" with made-up sizes, now and then skipping one as gone.

## See also

- [The salon](../screen/salon.md): folding, the attention order, `f`.
- [Many projects](../scenarios/many-projects.md): a worked example.
- [Sessions](../concepts/sessions.md): what the store keeps per project.
- [Commands](../reference/commands.md).
