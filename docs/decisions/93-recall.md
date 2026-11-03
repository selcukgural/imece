# Recall: `H` and `:recall` search every journal and trace line

Issue #241 (2026-11-03, from the study of agent-deck's Recall; backlog
item C13 of #56): `:history` (record 66) searches only what the person
sent. Recall searches what everyone did: every journal line and trace
line imece keeps, of every project and archive, newest first.

## What is searched

- **Journal lines** (the events): the name of the agent the line is
  about and every text the event carries (a message's first line, a
  topic, a reason, a command, a path, a standing yes's rules), not its
  ids or numbers (`recall::words_of`: the string leaves of the event's
  JSON). What the screen words around them (`asked`, `answered`) is not
  searched: it depends on the language, the data does not.
- **Trace lines** (the raw stream): the agent's name and the line, as
  written (its clock first). The store keeps the last 200 per agent
  (`TRACE_KEEP`), so recall finds those.
- **Mail**: a message between agents is journaled as `Sent` with its
  first line, so that line is found as a journal line. The whole body is
  kept only while unread (`mail` table) and is not indexed: an unread
  body is a briefing's, not a record.

## Words

What is typed is folded as the help filter folds it (`ui::fold`: case and
the Turkish letters, `ı İ ş ğ ç ö ü` as their plain Latin ones) and cut
into terms at every character that is no letter or digit
(`recall::terms`). A line has them when every term starts one of its
words (`deploy fix` finds `fixed the deploy script`; `ploy` does not):
the full-text index matches words by their start, not inside them, and
the demo matches the same way (`recall::has`). No terms (only
punctuation): nothing is listed.

## The index (schema 19)

- `recall`: an FTS5 table with no copy of the text (`content=''`,
  `contentless_delete=1`), tokenizer `unicode61 remove_diacritics 2` over
  text folded in Rust first, so the file grows by the index only. A
  query is each term quoted as a prefix (`"deplo"*`), all of them
  needed; terms hold only letters and digits, so nothing typed is ever
  the query language's (quotes doubled all the same).
- `recall_rows`: where each row's line is (`kind` 0 journal by `seq`, 1
  trace by agent and `seq`) and when it was written; a unique index on
  `(kind, agent, seq)`. A trace line keeps no tick of its own in the
  world, so its row takes the tick of the save that wrote it: within a
  step of the truth, since a save follows every step.
- Written with the lines, in the save's transaction (`store::index`: a
  line in the index already stays as it is). A line deleted takes its
  rows with it, by triggers on `journal` and `trace`: the tidy's 90 days
  (record 24) and a trace past its 200 are the index's retention too.
  An archive's lines are kept by the tidy, so they stay findable.
- An older file is indexed once, at its first open by this version
  (`fill_recall`, in one transaction): lines in the order they were
  written, a trace line by its agent's last change (the closest the file
  knows). A line that does not read (a later imece's) is left out.
- Reads by key only (`plans`, EXPLAIN QUERY PLAN test): the match in
  `rowid` order newest first with a `LIMIT` (`recall::SHOWN`, 50), then
  each row's line by its primary key.

## On the screen

`H` (from anywhere outside writing) and `:recall [WORDS]` open the
command line with the words, lead ` › recall: `, and the completion menu
lists what has them as `^r` does (`App::searching` with
`App::recalling`): each line as the journal words it (`words::event`,
through `fill`) or the trace line through `clean`, on one line, noted
`agent · project · when` (`model::when`: the clock today, the date before;
a trace line the demo holds has no time: `agent · project`). `tab` `↑`
`↓` step, `esc` closes the menu then the line. The words are not kept
for `↑` (`:recall` is a line that only goes somewhere, as `:history`).

`enter` goes to the line (`App::go_recalled`):

- a journal line the screen holds: its conversation (`open_thread`, as
  `enter` on a journal line), the line selected in the salon's journal
  when it shows there (`show_line`);
- a line of an archive: the archive's read-only view (record 80) with
  the line's conversation open in it (found there by its tick and event),
  or the agent's raw stream for a trace line;
- a trace line of an open project: the agent selected, its raw stream;
- a line the screen does not hold (an old journal line past the 5000 a
  start loads, or a closed project's trace): `recall_away` is said.

Nothing has the words: `no journal or trace line has …`. The store
failing: `recall could not read the store: …`. In an archive's view `H`
is refused as read-only and `:recall` is not one of its commands
(`VIEW_COMMANDS`): the view looks at one archive, recall at everything.

The help row `A  H  :recall` (merged with `:archives` so the help stays
40 rows).

## The demo

The demo has no store: it searches what its world holds
(`recall::in_world`), journal lines newest first, then each agent's
trace newest first, with the same terms and matching. Live without a
store (tests) does the same.

## Not done

- Jumping to the very trace line in the raw stream: the stream opens at
  its newest lines.
- A journal line older than what a start loads opens nowhere; showing
  it would need the journal paged from the store (as an archive's view
  reads it).
- Stories (a card's own lines) are not indexed: their events are the
  journal's too.
- Searching inside words (`ploy` for `deploy`): FTS5's trigram tokenizer
  would do it at about three times the index.
