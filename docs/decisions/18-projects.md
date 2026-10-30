# Projects from the screen v0

Projects from the screen v0 (2026-10-29, user's ask: open and manage
projects without CLI flags): live `imece` starts with the checkout it
runs in as project 0 (leader `lead`); `:open PATH [NAME]` (`:o`) opens
the git checkout at PATH as a project of its own with a new leader named
`lead-NAME` on the team's lead vendor (same model and tiers), NAME
defaulting to the directory's name made safe (1-24 of `[A-Za-z0-9._-]`,
the rest `-`); `:close [NAME]` (`:c`, no name = the project at hand)
ends its agents' sessions and runs, drops their mail, notes, offers and
changes, closes their inbox items, marks the project `closed` (ids stay;
the salon skips it; the same checkout opened again reuses the id with a
new leader), worktrees stay on disk, the last open project refuses;
`:rename NAME` renames the project at hand and its leader to `lead-NAME`
(project 0's too), sessions go on and every live agent of it gets a note
at its next prompt (`send` to the old name answers with the team list).
One budget pot, one bus (sockets in agent order, like `hire`), global
names; `Live.places` holds each project's checkout and worktrees root
(`root_for`: `<state>/worktrees/<dir>-<fnv>`), `Live.running` is per
agent so a closed project's runs die alone; a leader's prompt lists only
its own project's workers (was every worker); `offer` skips gone
workers; `App::current_project` falls back to the first open project
when a journal line leads to a closed one. The demo has the same three
commands (user, 2026-10-30): `:open PATH [NAME]` adds a fake project
named NAME (else the path's last part), with a leader, two workers of
random vendors and three cards, on one of the sim's four code maps; a
closed project of that name opens again in place; `:close` sends its
agents away and drops its plans and research requests (the sim's step
skips closed projects); `:rename` as in live. `ProjectRefusal`,
`valid_project_name` and `project_name_of` live in model.rs for both
engines.
Completion: `:close ` offers open project names, `:open ` completes
DIRECTORIES from the filesystem (`complete::dirs`, hidden ones only
after a `.`, `~` = `App.home`, tail `/`); `:close` + enter with a
trailing space takes the menu, so the project at hand closes with a
bare `:close` or `:c`. Journal: `Opened` / `Closed` / `Renamed`
(`EntryKind::Person`, worded by the project's CURRENT name). Known
gaps: a PATH with spaces is not parsed (no quoting), opened projects
are not remembered across starts (comes with session persistence), the
project-name prefixes on journal and inbox lines count closed projects,
the lead vendor can not be chosen per project.
