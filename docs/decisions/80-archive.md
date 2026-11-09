# Archived sessions: :archive, :restore and the A browser

The person's ask (2026-11-02): a finished epic's project should go out of
sight with all it had, a clean one should take its place, and the old one
must stay to be looked at again, or carried on, when they choose. Until
now the only way to start clean was to move the store file away by hand,
which took every project with it.

**A session is a project.** The person decided: what is archived is one
project (its head, workers, helpers, cards, journal lines, inbox items,
conversations), not the whole screen; the other projects go on. One
pot stays: archiving spends nothing and resets nothing, and the daily caps
of imece's own paid calls stay global (record 56). Bringing it back (`:restore`, its agents go on
in their vendor sessions) is the second step, below; looking at one
read-only from the `A` browser is the third.

**`:archive [TITLE]`** puts the project at hand away as TITLE (its first
line, trimmed, at most `ARCHIVE_TITLE_MAX` 60 characters; else
`World::archive_title`: its name and the days of its oldest journal line
in memory and of now, `kule 2026-10-30 → 2026-11-02`). It waits out the
grace of record 78 (`Undoable::Archive(p)`, the title in
`App::archive_title`; `u` keeps it: `kule stays as it is`). The bottom
line counts down `◷ archiving kule as “first epic” in 7 s · u keeps it`,
and when agents of it are mid-turn says how many turns it cuts short
(`grace_archive_cut`). There is no refusal for a busy project: an archive
cuts a turn short as a quit does, and the cut prompts are kept.

**What an archive is.** `Project.archived: Option<Archived { at, title }>`
with `closed = true`, so every filter of a closed project (the salon,
`open_projects`, `#` cards, `&`, `:goto`, `@`) already leaves it out. Its
agents are NOT gone: `World::archive_project` puts them `Asleep`, drops
an activity and a waiting retry, and leaves their inbox items open.
`Live::archive_project` ends each agent as a quit does: runs killed, the
session terminated, `end`, and keeps what it had: the remembered vendor
session, mail, notes, its change, posts and offers, and the prompts a
turn was cut short in (`interrupted` ++ `inflight` ++ what waited in
`Slot::Starting`, kept in `inflight`, which the store saves). `remembered`
is set, so the next session is briefed.

**A fresh project takes its place**, the same kind, name, pin and `:copy`
list, its head on the same vendor and model and with the same name (`lead`
stays `lead`), on the same checkout and worktrees root; `home` moves to it.
`World::archive_project` notes one line, `Event::Archived { leader, was,
title }`, whose subject is the FRESH head, so the line stays in sight
(`you archived kule as “first epic”; a fresh kule takes its place`); no
`Opened` line (`World::add_project` is `open_project` without it).

**Out of sight, out of reach.** `World::put_away(id)` (closed or
archived) and `World::in_archive(id)`:
- `agent_by_name` skips put-away agents, so the bus, `@` and completion
  reach the fresh `lead`, not the archived one with the lower id;
  `name_taken` (hiring's and the demo's free names) still counts them,
  so a new worker is never called like an archived one (its worktree is
  `root/<name>`). Workers were numbered over the whole world already.
- `open_inbox` hides an archive's items (still `open`): WAITING FOR YOU,
  the quit question and `open_changes` follow. A closed project's own
  items are closed by its close, as before.
- `ui::journal_rows` hides an archive's lines; the load is unchanged, so
  the store's numbering of new lines and `tidy`'s bound stay as they
  were. A closed project's lines still show, as before.
- `World::shown_cards` (the board, the queue, the header's `N/M cards`,
  `cards_done`) leaves an archive's cards out; the board names projects
  only when more than one in sight has cards.
- `Live::offer` offers no card to an agent put away, and a tool call from
  one (asked before the archive and still queued) is answered `your
  project was put away; nothing was done` and traced.
- `open_project` (live and demo) reuses a closed project in place, never
  an archived one.
- `--fresh` skips archived agents: their sessions are what a restore goes
  on with.

**Gemini's leader directory.** Every team's leader on a checkout ran in
`<root>-gemini/lead`; a fresh team would share it with its archive, and
closing the fresh one (`forget` whole) would remove the archive's chats.
`gemini_home` now gives `lead` to the first team on a checkout and
`lead-p<project id>` to a team after it (a solo before it does not count;
a solo's is its tree, `solo-<id>`, as before). It is derived, not saved:
before archives a checkout never held two team rows (a closed one was
reused in place), so every existing leader keeps `lead`.

**Store (schema 15).** `projects.archived_at`, `projects.archived_title`
(NULL: not archived; an older file gets them by `ALTER`); the index
`journal_project ON journal (project)` for reading one archive's lines
(the plan test reads through it); `tidy` keeps an archive's lines whatever
their age (`AND project NOT IN (SELECT id FROM projects WHERE archived_at
IS NOT NULL)`). At a load an archived agent comes back `Asleep` and
`remembered` with its cut prompts still in `inflight`: no `Interrupted`
line and no story entry at each start; they are told when it is brought
back.

**The demo** archives the same way (`Sim::archive_project`): the
project's plans, research requests, replies to come and hunks being
applied go, the fresh project gets the same code map and solo choice, and
the sim's step leaves agents put away alone.

**Growth.** An archive grows no more: its agents sleep and nothing
reaches them. Its journal lines are kept for good (the tidy skips them),
so the file grows by what each archived epic said, once; deleting an
archive is not done yet. At a start the load still reads every project
and agent row and the newest 5000 journal lines; an archive's lines among
them are hidden, not skipped, which costs one filter per drawn frame over
the lines in memory (`in_archive` is two lookups).

**Concurrency.** The archive is one step on the main thread: the killed
runs and terminated sessions are the same calls as `:close` and a quit.
A session's late events carry the old generation and are dropped.

**Failure.** The fresh head's socket is bound first; a failure there
refuses the archive with nothing changed. A crash between the archive and
the next save loses the archive (the project is open again at the next
start, its agents asleep, as after a crash), the safe side.

## `:restore [NAME]` (the second step)

**Which archive.** `World::archive_named(arg, at_hand)`: the newest one
titled `arg` exactly, else the newest archive of the project called
`arg`; a bare `:restore` takes the newest of the name the project at hand
has, so it undoes an `:archive` whose grace is over. Newest: archived
last (`Archived.at`), then the higher id. `World::archives` lists them in
that order for the menu after `:restore ` (`complete::archives`: the
titles starting with what is typed, case aside, each noted `kule ·
2026-11-02`, its project's name and the day it was archived). Nothing
named: `no archive is titled or named x`, `kule has no archive`.

**Refusals, nothing changed** (`Live::restore_project`): the folder is
gone (`ProjectRefusal::Gone`); a team's folder is no git checkout with a
first commit of its own any more (`NoGit`; `own_git`, the same test as a
start's, now shared with `adopt`); another open project has its name
(`NameTaken`): the person renames or closes that one first. A solo's
folder without git is fine: the solo works straight in it, as at a start
(`set_no_git`).

**What gives way.** One checkout holds one open team, so a team archive
swaps out the open team on its checkout, whatever it is called; a solo
archive swaps out the open solo of its own name there (other solos stay).
The one giving way is archived in its turn under `World::archive_title`
when it did anything (`World::did_anything`: a card, an agent besides its
head, a cent spent, an inbox item, or a journal line of its agents other
than its opening, archiving, bringing back, renaming, closing, `NoGit`
and `Confine`; live adds a session one of its agents had), else closed
as `:close` does (`Live::shut`, its own `Closed` line), so empty archives
do not pile up. Archiving it is `Live::put_away` (the per-agent end of
`archive_project`) and `World::shelve` (closed, asleep, `archived` set)
with no fresh project. `home` follows the archive when it was the one
giving way.

**Coming back.** `World::restore_project` clears `archived` and `closed`
and notes `Event::Restored { leader, title, swapped }` about the
archive's head (`you brought back kule “first epic”`, and `; the one open
there is archived as “…”` when one was): the archive's inbox items,
cards and journal lines are in sight again by themselves (every filter
was `in_archive`). Its agents stay `Asleep` and `remembered`: nothing
wakes or spends until the person writes. Each agent gets a note (`The
person had put this project away (archived as "…") and has brought it
back now`) and the prompts the archive cut short move from `inflight` to
`interrupted`, so its wake briefing says that turn was cut short and is
not run again; no `Interrupted` line (the person saw the count in the
grace line). Its vendor sessions are the ones kept (`sessions`), resumed
at the wake when the vendor still has them (record 20). The screen
selects the archive's head; what pointed into the one that gave way (its
focus, card, unseen, settled rows) goes, as after an `:archive`
(`App::put_away`). No grace: a restore throws nothing away.

**Store (schema 16).** No table changed; the `Restored` events are new,
which an older imece would not know. A restore saves the project row
with `archived_at` NULL again.

**The demo** (`Sim::restore_project`) swaps the open project of the
archive's name and kind (it has no checkouts), refuses when one of the
other kind has the name, and drops what was to come of the one giving
way (`Sim::leave_be`, shared with `:close` and `:archive`, which now also
drops its hunks being applied). Its folders are its own: never gone,
never without git.

**Known gap.** A project brought back and archived again (by `:archive`
or by giving way) takes a new title: the old one is not kept.

## `A` / `:archives`: the browser (the third step, first half)

**What it lists.** `archives::groups` groups `World::archives` (newest
first) by checkout: live `Live::repo_of` written with `~` for the home
(`App::checkout_shown`), the demo's the project's name (its stand-in
for a checkout, as its `:restore` swap uses). The checkout of the
project at hand goes first, the others in the order of their newest
archive. Each group has the projects open on it now (`● … open now`,
by id) above its archives. A checkout with no archive is left out. The
groups are frozen when the browser opens: only a restore changes an
archive, and a restore closes the browser.

**Rows** (`archives::Browser::rows`): checkout, open projects, archives,
and under an unfolded archive its agents that are not gone
(`archives::agents_of`; a helper that finished is gone). An archive's
row: its title, what all its agents spent (gone ones too), its cards in
Done, its agents, the day it was archived. Titles, names and checkouts
go through `clean`. The rows scroll to keep the selected one in sight;
at least one row is drawn on any screen.

**Keys** (`Browser::on_key`, the browser takes every key, as the solo
pick does; no `keys::Scope` of its own, so the key file does not reach
into it): `j k ↑ ↓`, `gg G home end`, `l →` unfold (a checkout's
archives, an archive's agents), `h ←` fold (on an agent: its archive,
selected; on a folded archive or an open project: its checkout,
selected), `za` toggles the row at hand, and so does `enter` on a
checkout; `enter` on an archive or one of its agents opens its read-only
view (below, `Act::View`, the agent selected), `/` filters (`overlay::fold`: every
word in the title, project name, checkout or an agent's name; `enter`
keeps, `esc` drops, `backspace` on nothing stops typing; a paste goes in
only while typing), `r` on an archive or one of its agents restores it
through the same path as `:restore` (`App::restore_at`) and closes,
`esc q A` close (`esc` drops a kept filter first). `A` with nothing
archived flashes `no_archives`. An `:archive` counting down acts first
(`end_grace`), so it is listed.

**Help.** One row, `A  :archives`, after `esc u`: 57 key rows make 29
in two columns, so the help fills the 40-row screen with none to spare.
A row more scrolls it there (record 40), so no rows need merging.

**The read-only view.** `enter` on an archive opens it over the screen
as a screen of its own: `App.viewing: Option<Box<App>>`, an `App::with`
of `World::view_of(p, journal, marks)` and `Engine::View(Viewed { project,
title, at })`, which never steps. `view_of` keeps every project, agent
and card (the journal's events name them by id), opens `p` (not closed,
not archived, so the salon, the journal and the names show it) and puts
every other project away as an archive is (closed, `archived` set: the
salon, the board, the journal, WAITING FOR YOU and `agent_by_name` leave
them out); the pot, the clock and the lines sent are this world's; no
limits, queue or suggestion. Its journal is all the store keeps of `p`
(`Store::project_journal`: `WHERE project = ?1 ORDER BY seq` through the
`journal_project` index, a mark by its place in the lines, a line no
imece reads left out; `Live::project_journal`, the world's without a
store), more than the newest lines the start loaded; the demo's comes
from memory (`World::journal_of`). A store that fails says so
(`view_failed`) and opens nothing. The view takes the person's keys,
side, shift-enter, home and fold choice; its language starts as the
screen's (`L` there changes only the view's).

Keys and drawing go to the view; the screen behind goes on stepping and
saving. `esc` or `q` (plain) where nothing is open in it (`App::at_rest`:
not writing, no help, chord, held key, hints, note, card, zoom, window,
peek or conversation) closes it; `:quit` and its `q` close it too
(the view's `quit`); `ctrl-c` quits imece. What the view copies goes to
the clipboard through the screen behind (`after_view`), a paste and a
held key (`keys_due`) go to it, its flash is the screen's clock, and the
away digest does not open over it (`focus`). The top row is a banner in
place of its header (`ui::view_banner`): `ARCHIVE · name “title” ·
archived day · N waiting for you now · esc back`, N from the world behind.

**What it refuses.** Deny by default: of the `:` commands only those in
`VIEW_COMMANDS` run (they look, go, or change how the screen looks:
`goto history export salon board code lang theme help quit next defer
waiting needs fold unfold trace peek hints news settle unsettle talk`);
every other known command flashes `read_only` (`an archive is read-only ·
:restore brings it back · esc goes back`), an unknown one is still
`no_command`, and a command added later is refused there until it is
listed. `:restore` asks the screen behind to bring the archive back
(`restore_asked`, then `restore_at`) and closes the view. A message, an
`!cmd`, `m`, `A`, `a`/`r` in WAITING FOR YOU and every `match` on the
engine (tell, apply, reject, notes, decide, copy, commit, open, solo,
re-choose, close, archive, restore, forget, rename) refuse the same and
call no engine; `space + -` are the demo's only (`App::demo`).
`:export`, `v` copying, conversations, the card screen, the focus, the
board and the code map work as anywhere.

**Not done:** deleting an archive, renaming one. The view is the
archive as it was when it opened; it is not built again while open.

**The banner's count (2026-11-09).** `N to decide now` counted replies; it
counts `World::open_decisions` and says replies apart (`2 to decide, 1 reply to
read now`). The browser's `1 agent`, `1 card done` count one as one.
