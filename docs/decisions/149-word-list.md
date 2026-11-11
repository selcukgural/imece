# Words while writing, in a list

The person asked on 2026-11-07 for word suggestions while writing, as an
IDE gives them (#361; `docs/field/2026-11-11-demo-word-list.md`). Until then
`tab` completed names after `@`, commands after `:` and cards after `#`
(record 12) and a message's last word to a file (record 67); an ordinary
word was not known. The person settled the shape on 2026-11-07 (a
drop-down list, not ghost text; no model) and the rest on 2026-11-11 from
a mock-up.

**The list.** Any word of a message or a `!cmd` line that is not a name
(`@`), a command (`/`, `:`), a card (`#`) or a jump (`&`) has a drop-down
list: `Menu.list`, drawn by `ui::footer::word_list` over the screen above
the line, at the column the word starts in (the cursor less the word's
cells), slid left at the right edge, from the row above the line up and
never over the header. There is no frame: the rows have a ground of their
own (`Theme.selection`; nothing in mono), because a box would change its
width with every key and take two rows more; the person chose this over
the boxed mock-up. Its width is the widest row's, at least `LIST_MIN` 24
cells so it does not change shape at every key, at most `LIST_MAX` 48, a
longer name cut with `…`; `LIST_ROWS` 6 rows at most, scrolled to keep the
word at hand, the last row counting `+N` more. Each row says where its
word comes from (`said`, `code`, `file`, or the holder of a leased file),
worded when the list is made, as the `:` menu's notes are. The row menu
of `@ : # & ^r :recall` stays as it was: those replace the line's start,
a list mid-line has nothing to add to them. A drawing that holds the list
patches no breath (`App.breathing` is cleared), since a breathing glyph
may lie under it.

**The keys.** It opens by itself from the second letter (`pool::TYPED`,
the person's choice over three). `tab` takes the word at hand and closes
the list (`App::take_word`); nothing follows a word taken, since a
Turkish suffix may (`retry_policy'yi`). `↓ ↑ ^n ^p ⇧tab` move in it
(`Menu.moved`) and put nothing in. `enter` sends unless the person moved:
a list that opens by itself must not hold back a send, so only a word
chosen by moving is taken by `enter`. `esc` closes the list first
(`App.word_closed`, the word's start): it does not open again on that
word, only on a word begun where it began or on the next one, or by
`tab`; a second `esc` leaves writing as before. While no list is open,
`↑ ↓` recall the lines sent (record 66) as before. `tab` with no list
opens one from the first letter, the files by any part of their path
when no word starts so (record 67's `complete::files`), and takes a word
that is the only one.

**Where the words come from** (`pool.rs`, pure): the conversation at
hand's messages, then the rest said in the project at hand (the journal's
said lines whose speaker or listener is of it, among its last
`JOURNAL_BACK` 2,000 lines), then the lines sent, `SAID_BACK` 200 texts
of each, newest first and a text's words last to first; then the names in
the code (below), the most used first; then the files of the project
whose name starts with the word, shorter paths first. A word with a `/`
lists the files by any part of their path and the symbols leased as
`file::name`, as record 67 did; so does a word with a `.` when some
path holds it (`shelf.p`, `config.toml`): a word with a dot is a file's
name more often than not, and the word after the dot would find the
code's names first. No path holding it (`e.g`, `self.world`), the word
after the dot is looked for as any other. A word is a run of letters, digits and
`_` starting with no digit, `SHORTEST` 4 to `LONGEST` 48 characters;
matching folds case and Turkish letters (`ui::fold`); the word typed is
never offered, and a word is offered once (its first spelling). At most
`SHOWN` 30 picks. Every text is cleaned (`i18n::clean`) before its words
are read, so a secret it holds never enters the list, and a name from the
code that `clean` would change is left out. No model is asked: the list
is local and free.

**The names in the code** (`names.rs`) are read on the git thread with
the files (`GitJob::Files`, so at a project's opening, after an apply and
every `FILES_EVERY` 60 s while one of its agents is mid-turn) into
`Project.names` (memory only, kept when a listing fails): the words of
every file git lists, in its order, counted, the most used first, at most
`KEPT` 10,000. Left out: a file larger than `FILE_MOST` 1 MiB (generated
or data more often than code), the files after `READ_MOST` 8 MiB read in
all, a file with a NUL byte or not UTF-8, lock files, minified or
generated code (`ui::diff::generated`, the change view's kinds) and prose
or data by extension (`md txt rst adoc org csv tsv svg …`). A file is
read only as a plain file inside the checkout: its real path must stand
under the checkout's, it is opened `O_NOFOLLOW | O_NONBLOCK` and looked at
as opened, so a link to a file outside or a pipe is never read. Every
file's text is cleaned (`i18n::clean`) before its words are counted, and
`pool::picks` cleans a name again when it matches, so a secret the code
holds is never offered. On imece's own checkout (626 files, about 7 MiB
of code) a read takes 0.18 s in a release build, on the git thread. The
demo's names are the words of its code map's symbols (`World::names_of`:
`Rates.Quote` is `Rates` and `Quote`).

**Read cost.** Each key that changes the line reads at most 2,000 journal
lines and 600 texts and looks at the 10,000 names; a text that does not
hold the word folded is not cleaned or split, a name that does not start
with it is not cleaned. Nothing is kept between keys, so nothing grows; the
list lives as long as the word. At ten times the talk the caps hold the
cost where it is; what goes first is the oldest talk, which is the least
likely word.

**Gaps.** The names are words, not a language's symbols: a word in a
comment counts as one in the code, and nothing tells a function from a
local. A file changed is read again only with the whole listing (no cache
by file). A list with the
cursor mid-line or on a text of several lines needs `Menu::insert` to
keep what follows the cursor: the one after. The demo shows the list
through the same `App` on its own talk and files (`Project.repo`).
