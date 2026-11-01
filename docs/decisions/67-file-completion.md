# Files and symbols completed in a message

Backlog item C11 of #56 (2026-11-01, from Warp and Terax; issue #149):
completion of repo files and symbols, from the code map, in messages.

What was built. While the person writes a message (to the leader, or
`@name …`) or a `!cmd` line, with the cursor at the end, the last word
can complete to a file of the project at hand (`App::file_menu`,
`complete::files`): the menu opens by itself only when the word has a
`/` in it, so ordinary writing is not interrupted; `tab` asks for any
word (`App::suggest_with(forced)`, the one place that passes `true`).
The files whose name starts with the word come first, then the ones with
it anywhere in the path; among equals the shorter path, then by name;
case and Turkish letters folded as the help's filter (record 40); thirty
at most (`FILES_SHOWN`). The item is the whole path, which replaces the
word with a space after it, on the completion menu of record 12 (no new
widget); `tab`, `↑` and `↓` step as they do for names. Not for a word
that starts with `@` (a name) or `/` (a command), not on the `:` `#` `&`
lines (their own menus), not over several lines (the menu is drawn on
one). After the files come the symbols leased on the code map (the
cards of the project that hold leases, record 17) whose name starts with
the word, as `file::name`; a word with `::` names the file's part and
the symbol's (`auth.rs::` lists that file's). A file an agent holds
shows the holder as the note (`● claude-1`: the salon's glyph and the
name, `ui::glyph`), its symbols too.

Where the files come from. The demo's are its code map's (`Project.repo`,
`World::files_of`). Live's are what git lists in the checkout
(`worktree::files`: `git ls-files -z --cached --others
--exclude-standard`, sorted, each once), read on the git thread of
record 36 (`GitJob::Files`, `Live::files`, `Live::filed`) into
`Project.files`, not saved (git is the source). When: at once for a
project never read or opened again (`Live::files_due`, `filed_at`
forgotten at `:close` and at an opening), after each change the person
applies (`applied`), and every `FILES_EVERY` (60 s) while an agent of
the project is mid-turn, when new files may come; one listing at a time
per project (`filing`). At most `FILES_MAX` (20,000) paths are kept; a
checkout with more is said once per project in the journal
(`Event::FilesCut`, `files_cut`), a listing that fails keeps the last
list and is traced on the leader (`files: …`).

Not done, said plainly: live has no symbols, since imece has no leases
yet (the Lonca code is not copied in; the code map is empty live), so
the live menu is files only, and the holder note is the demo's; a
symbol index of the repository (ctags, tree-sitter) was not built: the
item says what the map knows. The menu over several lines (record 39)
and completion for the `:copy` and `:export` arguments beyond
directories are left as they were.
