# Quiet erase

Quiet erase (2026-10-29, found by the user in iTerm2): `screen.rs`
wraps the crossterm backend so the whole screen is only ever erased
row by row (`CSI 2 K` per row), never with `CSI 2 J` or `CSI J`: iTerm2
(profile option "Save lines to scrollback in alternate screen mode",
on by default) copies an erased alternate screen into the scrollback,
and ratatui erases on every change of size, so each resize left a copy
of imece above the shell's history, reflowed into staircases of frame
bars. Standard terminals do not do this; the row-by-row erase is
harmless everywhere. The other leak is a write past the right edge of
the bottom row (a frame drawn for one width landing in a terminal that
has just become narrower while the window is dragged): it wraps,
scrolls, and the top row goes to the scrollback. So auto-wrap (DECAWM,
`CSI ?7 l`) is off while the screen is up and on again whenever the
terminal is handed back (`screen::take_over` / `give_back`, used by
`init`, `restore` and the `!` dance in shell.rs). `screen::init` and
`screen::restore` replace ratatui's. Found with the demo in a scripted
iTerm2 window and iTerm2's source (`eraseInDisplay` calls
`scrollScreenIntoHistory`; a resize moves the alternate screen through
a private buffer and never into the scrollback): `CSI 2 J` and
`CSI H CSI J` leak, `CSI K` per row does not, the cursor's row makes
no difference.
