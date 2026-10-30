# Code map v1

Code map v1 (2026-10-29, user: the first one looked a mess): one row
per file, one cell per lease in fixed columns (`ui/code.rs`,
`Widths::of`): the holder's status glyph (the salon's `glyph`, one
cell, never doubled), its name in its vendor colour, the short symbol,
each padded to the widest in the PROJECT (agents' names, all cards'
symbols, capped 14/18; file names capped 28) so the columns do not
jump when a lease comes or goes. A file nobody holds is dim with
nothing after it; a file several agents hold ends with `⇆ N`
(`shared()`). The legend is not fixed: it words only the glyphs on
the map right now (`GLYPHS` in code.rs, `code_glyphs` in i18n, one
short word per status in the salon's order) and says what `⇆` means
only when a file is shared (user, 2026-10-29: the fixed legend missed
the glyphs the map had started to use). Tests check the columns line
up on the demo (the other project's names must not widen them), the
badge count, the dim rows, the directory headers and that the legend
names exactly the glyphs shown.
