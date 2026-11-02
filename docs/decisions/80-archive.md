# Archived sessions: :archive

The person's ask (2026-11-02): a finished epic's project should go out of
sight with all it had, a clean one should take its place, and the old one
must stay to be looked at again, or carried on, when they choose. Until
now the only way to start clean was to move the store file away by hand,
which took every project with it.

**A session is a project.** The person decided: what is archived is one
project (its head, workers, helpers, cards, journal lines, inbox items,
conversations), not the whole screen; the other projects go on. One
pot stays: archiving spends nothing and resets nothing, and the daily caps
of imece's own paid calls stay global (record 56). Looking at an archive
(read-only, nothing wakes) and bringing it back (its agents go on in their
vendor sessions) are the next two steps of the plan, `:restore` and the `A`
browser; this entry is the first.

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

**Not done (the next steps):** looking at an archive and the `A` browser,
`:restore`, deleting an archive, renaming one.
