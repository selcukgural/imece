# Why an agent is where it is

Backlog F19 (#59, after herdr). The salon's row says what state an agent
is in (record 61's rung glyph, record 62's words), not what put it there;
finding that meant reading the journal back for its lines.

`:why [@NAME]` (no name: the agent at hand, as `:peek`) says it on the
bottom line, in one flash: the agent's name, its rung in words
(`Text::rungs`, the ladder's order: idle, done and not seen, starting,
working, waiting for you, stuck), since when, and the journal's line
that put it there in the journal's own words (`words::event`), or that
the journal does not say (`why_unsaid`). `App::why` gives the data: the
rung from `App::rungs`, "since" from `App::since` (the oldest open item
waiting for the person, else `Agent.changed_at`, the same time the row's
`· 3 min` counts from), and the newest journal line of the agent
(`Event::agent()`) from that tick or before. `words::why` words it when
it is said; the flash is a string, so a `:lang` after it does not re-word
it (as every flash).

Why the journal line and not a reason kept on the agent: the journal
already holds every cause the screen shows (stopped and why, a question,
a change, a retry, a reply), kept and saved; a field of its own would be
a second copy that could disagree with it. The line before "since" is a
choice: a later line (a trace note, a message it sent while still
stuck) is not what put it there.

Read cost: one pass back over the journal for each `:why`, only when it
is typed; nothing per frame. Growth: none, a flash. Ten times the scale:
the journal is kept to its retention (record 24), the pass stops at the
first line found. Failure: a name nobody has says `no_agent`, as the
other agent commands; the journal tidied past the cause says it does not
say. The demo runs the same code on its own journal, so it shows what
live shows.

`:why` only looks, so an archive's view runs it (`VIEW_COMMANDS`); its
name completes after `:why ` like `:peek`'s. Not done: a window with the
agent's last few state changes, which the card's screen (record 32) and
the focus already come close to.
