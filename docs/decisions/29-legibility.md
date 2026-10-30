# Legibility v0

Legibility v0 (2026-10-30, the first Kule run's screenshots): a window
over the salon (a conversation, the help, the digest, where you left
off) DIMS everything around it (`ui::shade`, every cell outside the
window gets the DIM modifier before `clear`), so the salon tree and the
journal's clipped line ends read as background; the journal pane and a
question in the inbox show a message as one row of PLAIN words
(`ui::plain`: cleaned, lines joined with a space, heading and list
markers off, inline marks replaced by what they cover, a link by its
label; `ui::marks` is the inline parser both `inline` and `plain` use);
the trace (`focus::trace_lines`) colours by the line's first word: the
agent's own `text "…"` unquoted in its vendor colour, one row per line
(the line the person must not miss: "four tests fail without the fix"),
`tool_use` with the tool's name in the info colour, `run:` ends by exit
code (ok / bad), `stopped` and `run refused` bad, the bus words in the
accent, the rest as written. The `post` title and the `send` body
descriptions tell the model the first line is the card's title as the
salon shows it (20 cells: lead with what tells the cards apart, not
"Fix the …"; two cards had read `Fix Gi` and `Fix Gi`). A trace line
that runs over rows (a Bash heredoc in a `tool_use`) keeps them: each
cleaned on its own, indented under the first, at most `TRACE_ROWS` (6),
then `… +N` dim; before, `clean` made the newlines spaces and the
heredoc wrapped as one long paragraph. Known gap: the trace's
vocabulary is read back from strings, a new producer word is plain
until named in `trace_lines`. An open
conversation window FOLLOWS the talk (`App::follow_thread`, after every
key and step): journal lines of its pair that came since are appended;
the person's own line (written from the window with `@`) is selected,
as is a line that came while the last one was selected; a selection
further up stays (was: close and open again to see what was written).
