# A worker's local address as a link

A worker or helper that starts a dev server says where it runs
(2026-10-31, backlog B9 in #55, issue #93, from Terax): the newest
loopback URL it writes shows once at the end of its salon row,
`↗ localhost:5173/`, as an OSC 8 hyperlink the terminal opens.

What counts is strict, as the person clicks it (`model::local_url`):
the scheme `http` or `https`, the host `localhost`, `127.0.0.1` or
`[::1]` and nothing else (`localhost.evil.com`, `localhost@evil.com`,
`127.0.0.1.nip.io` are not it), a port 1–65535 of at most five digits,
a path of ASCII letters, digits and `-._~:/?#[]@!$&*+,=%`. Quotes,
brackets, `;`, `\`, spaces, non-ASCII and control characters end it,
so nothing an agent writes can end the terminal's sequence early;
sentence punctuation after it is left out. The query and fragment are
dropped (a token in `?token=` is not kept on the screen) and a URL
longer than 60 characters is not shown at all, so what is clicked is
all on the row. A path cleaning would change (`i18n::clean` hides one
that looks like a secret) is not shown either: the text is the link.
Only the agent's own text counts (`Text`, a turn's closing text):
tool output does not reach the screen (record 02). The address goes
when the session ends (`Live::end`), as a server it started goes with
it; it is not saved in the store.

ratatui counts every byte of a cell's symbol as width, so the link is
not part of the row's text. The row is laid out as usual; then, in the
buffer, every cell of the URL gets `ESC ]8;id=imece-N;URL ESC \`, its own
symbol, and `ESC ]8;; ESC \`, with `CellDiffOption::ForcedWidth(1)`
(`ui::hyperlink`). Each cell opens and closes its own link: when only
one cell changes, ratatui redraws only that one, and a link opened in
one cell and closed in another would stay open over whatever is drawn
next. The `id` makes the terminal light the cells up as one link. Only
a URL the row shows whole is linked (`salon::link_cells`); in a narrow
pane the cut text shows with no link. A terminal without OSC 8 shows
the plain text (iTerm2, kitty, WezTerm, Ghostty, foot and VTE ones open
it; tmux passes it with its `hyperlinks` feature).

The leader, which writes nothing and reads in the person's checkout,
and researchers get no address on their row. The `?` legend says what
`↗` is, in both languages. The demo's workers on the projects that
serve (shop-api, web, billing) say where the app runs after a build,
so their rows show it; a stopped session drops it there too. Each
runs its own copy in its worktree, so each has a port of its own, as
live workers would (2026-10-31, with record 46): the project's first
port plus how many of its workers and helpers came before it, gone
ones too, so a port stays its worker's when another leaves
(`sim::serving_before`).

Not done: the card screen draws the same row (`card.rs`) with the
address as plain text, not linked; not tried in a real terminal yet,
only through crossterm's backend into a buffer.
