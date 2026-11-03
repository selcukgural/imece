# Clearing the screen without deleting

At start imece brings back what earlier sessions left (record 25): the
journal's lines and what waits in WAITING FOR YOU. The person wanted to
clear what the salon shows with one command, and the journal and WAITING
FOR YOU one by one, while the store keeps everything for `:export`, the
archives, a card's screen and later runs (#232, 2026-11-03).

`:clear` takes both, `:clear inbox` and `:clear journal` one
(`app::Clear`). It waits record 78's grace (`Undoable::Clear`, the bottom
line counts down, `u` keeps the screen as it is), then `App::clear_now`
sets `World.cleared` (`model::Cleared`):

- the journal by a point, `journal`: the lines before that index are not
  listed by `ui::journal_rows`, but for the marked ones (a mark is never
  left out, record 46);
- the inbox by item, `inbox`: the items open at the clear leave the list
  (`World::shown_inbox`, what `App::inbox_rows` lists outside a card's
  screen). What waits for the person must not be lost silently (the
  person's answer): an item cleared stays open, its agent still waits,
  WAITING FOR YOU's title counts it (`+N hidden`, `World::hidden_inbox`),
  as do the terminal's title, the tab's progress and notices, which read
  `open_inbox`; `W` reaches it and puts it back on the list as it opens
  it (`App::open_waiting`). A set and not a point, so one can come back
  alone.

What comes after shows as usual. `:unclear [inbox|journal]` brings back
at once and says how much came back.

The clear holds across runs (the person's answer): the store keeps it in
`meta`, not in the schema: `cleared_journal` is the `seq` of the last
line taken off (a load of the newest N lines finds its point by
`seq`, so a line of the same tick after the clear still shows),
`cleared_inbox` the item ids (`3,7,12`; ids past the inbox are
dropped). Both are world keys, not settings. The demo has no store, so
its clear ends with the run; otherwise it runs the same path. An
archive's view starts with nothing cleared.
