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
| `:archive [TITLE]` | puts the project at hand away with its agents asleep and all they had (sessions, mail, changes, cards, journal lines), out of sight, and opens a fresh project of the same name, head and `:copy` list in its place. Ten seconds of grace, `u` keeps it. See [archives](sessions.md#archives). |
| `:restore [NAME]` | brings an archive back as it was put away, its agents asleep until written to; the team open on its checkout gives way (archived, or closed when it did nothing). See [archives](sessions.md#archives). |
| `A`  `:archives` | the archives as a tree, by checkout, with what each spent, its cards done and its agents; `/` filters, `enter` opens one read-only, `r` brings one back. |
| `:rename NAME` | renames the project at hand and its leader to `lead-NAME` (the first project's too). Sessions go on; every live agent of it gets a note at its next prompt, and a `send` to the old name answers with the team list. |

- A closed project keeps its id; opening the same checkout again reuses it, with a new leader (a closed solo is reused by the next solo on that checkout, never by a team). Its pin is kept for then. An archived project is never reused this way.
- One checkout holds at most one open team (`that checkout is open already, as kule`) and any number of solos. The directory's name takes a number when a project has it: `kule`, `kule-2`.
- Names are global: `@lead-api` reaches that project's leader from anywhere; the completion after `@` lists the project at hand first.
- A PATH with spaces is not parsed (no quoting). `:open ` completes directories from the filesystem (hidden ones after a `.`, `~` for home); `:close ` and `:pin ` complete the open projects' names.
- The journal: `you opened api at /src/api`, `you closed api`, `you renamed api to gateway`, `you archived api as “v1”; a fresh api takes its place`, `you brought back api “v1”; the one open there is archived as “api 2026-11-02”`.
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

### `.worktreeinclude`: the checkout names them itself

A checkout may name what to copy itself, in a `.worktreeinclude` file at its top, in gitignore's syntax (agent-deck and Claude Code's desktop app read the same file):

```gitignore
# copied into every worktree imece makes
.env*
node_modules/
```

At each worker's start imece reads it beside the `:copy` list. Git lists what its patterns match among the paths it does not track (a directory they match whole is one path), and of those only what git ignores is copied: after the list's paths, under the same rules, within the same 2 GiB. A path the list has already counts as the list's. Nothing needs your yes: the file only names paths of your own checkout to copy into imece's worktrees.

- **Refused, and said in the journal at each start:** the file when it is a link, not a plain file, larger than 16 KiB or not text; a line with a control character, longer than 200 bytes, with a `..` part or naming `.git` (the rest of the file is followed); the whole file when its patterns match more than 100 paths git ignores, or git does not say in 10 s what they match (then nothing of it is copied).
- The journal says the file's copies in a line of their own, `copied node_modules into claude-1's worktree (310 MiB), as .worktreeinclude names`; a skipped one as `node_modules (from .worktreeinclude) was not copied into claude-1's worktree: …`; a refused line as `line 3 of .worktreeinclude (../shared) was not followed for claude-1's worktree: reaches outside the checkout`.
- `:copy` alone ends with `; and what its .worktreeinclude names` when the checkout has one; it does not list what the file matches.
- In the demo, the second project's checkout acts as if it had one: it names `node_modules`, and a line of it that reaches outside is refused at every start.

Known limits: a file you change in the checkout later is not copied again (the worktree has it); copying is per worker, not shared; the 2 GiB budget, the entry cap and the 100 paths of `.worktreeinclude` are constants; `.worktreeinclude` is read from the checkout as it is on disk, so an edit not committed counts.

## A worktree setup script

A copy is not enough when a worktree needs work done in it: installing dependencies, running migrations. Put a shell script at `.imece/worktree-setup.sh` in your checkout, and imece runs it in each new worktree, after the copies and before the agent's session starts, **once you said yes to its exact bytes**:

```sh
#!/bin/sh
set -e
npm ci --no-audit
npm run db:migrate
```

- **When:** a worker's or a solo's worktree made just now (a helper works in its worker's, a leader in your checkout; a worktree that was there already is not set up again). Without git there is no worktree, so no script.
- **Your yes:** the first new worktree that finds a script you have not said yes to puts an ask in WAITING FOR YOU (`▶ claude-1: run the worktree setup script .imece/worktree-setup.sh in its new worktree? enter shows it`); the worker does not wait for it and starts without. `enter` opens the script as imece read and hashed it, a numbered line each, with what runs and how (`SETUP SCRIPT · claude-1 · sha256 1a2b…`); `j` `k`, `ctrl-d` `ctrl-u`, `g` `G` move, `a` says yes, `r` no, `esc` closes. `a` and `r` work on the selected ask too. A yes runs the script at once in the worktrees that went without it, and from then on in every new one before its session; a no holds for this run (a later start asks again). Workers that start while the ask waits are counted on the same ask.
- **What the yes is bound to:** the bytes' SHA-256, the file's real path (links resolved) and how it runs (`/bin/sh`), kept in imece's store, never in the repository. A changed byte is a new script and is asked about again. What runs is a private copy of the bytes you read, written into imece's state directory (`0700`, the copy `0600`), never the file again, so nothing can change it between your yes and the run.
- **How it runs:** `/bin/sh` on that copy in the new worktree, with imece's cleared environment plus `IMECE_REPO` (your checkout) and `IMECE_WORKTREE` (the worktree), in a process group of its own, at most 60 seconds (then the group is killed). At most 64 KiB of script.
- **What you see:** the journal says how it ended and its last line (`claude-1: worktree setup ran, exit 0, 4.2 s · migrations: 7 applied`, or `failed`); one that could not be read or started says why (`worktree setup did not run: …`). The worker starts either way.
- **Its teardown:** `.imece/worktree-teardown.sh` in your checkout runs in a worktree before `:tidy` removes it (below), on the same terms: your yes to its exact bytes, a private copy, `/bin/sh`, `IMECE_REPO` and `IMECE_WORKTREE`, its own process group, 60 seconds. It undoes what setup made outside the worktree (a database, a container, a port). Not done yet: removing a worker's own worktree, so a teardown runs only for worktrees no agent owns.

## A sparse checkout

On a large repository you may keep only part of the tree checked out (`git sparse-checkout`). A worktree imece makes from such a checkout is as sparse as it: imece adds it with nothing checked out, gives it your checkout's patterns (`git sparse-checkout set`, cone mode or not, as yours is), and only then checks it out, so what you left out is never written into it. The patterns are the worktree's own: changing them there does not change yours, and changing yours later does not change a worktree made before. A worktree that cannot be made as sparse (git older than 2.35) is removed again and the agent does not start; the journal says why, so a monorepo is never written out whole behind your back.

## Worktrees no agent owns

Each worker's worktree stays on disk between runs, so its work is never thrown away. One whose agent imece no longer knows stays too: after a crash, a removed store, a fork that never started. `:tidy` lists them in a window, each with its checkout and what it holds:

```text
 WORKTREES NO AGENT OWNS · 2
▶ claude-31  ~/src/shop  clean
  codex-32   ~/src/shop  files changed: 3 · commits of its own: 1
 a remove the clean ones   d remove this one   j k move   esc back
```

- **What counts:** a directory under imece's state (`worktrees/<checkout>-<hash>/<name>`) that git knows as a worktree added to a checkout, and that no agent of any project has: open, closed or archived; a helper's is its worker's. Links, files, Gemini's private directories and what git does not know are passed over.
- **What it holds:** files changed or not tracked (`git status`; what git ignores, such as `target/`, is not counted and goes with it), and commits of its own that no branch, tag or remote has (imece's own commits for changes you applied are in your checkout already and are not counted). Clean means neither.
- **Removing:** `a` removes the clean ones, `d` the one at hand. One that holds work goes only after a `y` for it, with `git worktree remove --force`; a clean one with `git worktree remove`, which refuses again if something changed meanwhile. Removal runs off the screen, and the window looks again after it.
- **The teardown script first:** when the worktree's checkout has `.imece/worktree-teardown.sh`, it runs in the worktree just before the removal. One you have not answered opens over the window first, as the setup script's does (`TEARDOWN SCRIPT · shop · sha256 1a2b…`, its lines numbered, `j` `k` `ctrl-d` `ctrl-u` `g` `G` scroll): `a` says yes (kept in imece's store with the setup scripts' yeses, so a changed byte asks again) and the removal goes on, `r` says no for this run and removes without it, `esc` removes nothing. The flash says how it went (`removed claude-31 · teardown in claude-31: exit 0, 0.4 s · dropped database`); a teardown that fails or cannot be read is said, and the removal still decides on its own: one that left a file in the worktree keeps it.
- **Never an agent's:** who owns what is read again when a removal is asked; a worker hired since, whose name is the old worktree's (it would take it over), keeps it (`not removed: claude-1 (an agent works in it now)`). A worker that starts while its worktree is being removed waits for it to go and gets a new one.
- **At start:** imece looks once, off the screen, and says `2 worktrees no agent owns: :tidy lists them` when it found any; it removes nothing by itself.
- The demo has two made-up ones, one clean and one with work, which `:tidy` removes as live does.

## The demo's copies

`--demo --projects N` (1-4, default 2) shows several fake projects at once; `:open PATH [NAME]` there adds a fake project with a leader, two workers of random vendors and three cards, `:close` and `:rename` work the same, `:pin` pins without a store, and `:copy` keeps a list that a demo worker's start "copies" with made-up sizes, now and then skipping one as gone. A worker the demo's leader hires gets a new worktree, so the demo's own setup script (`npm ci`, `npm run db:migrate`) is asked about, shown with `enter`, and on a yes "runs" with made-up output, as live does.

## See also

- [The salon](../screen/salon.md): folding, the attention order, `f`.
- [Many projects](../scenarios/many-projects.md): a worked example.
- [Sessions](../concepts/sessions.md): what the store keeps per project.
- [Commands](../reference/commands.md).
