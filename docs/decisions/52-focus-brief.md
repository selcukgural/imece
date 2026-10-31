# What happened, as soon as you are back

A brief when the focus returns (2026-10-31, backlog A3 in #54, from
openharness): the "while you were away" window came on the first key
after twenty seconds without one, so a person back from another window
first had to press something, which the window then swallowed.

The terminal reports when it loses and gets back the focus (focus
reporting is on since record 15, for the colour probe). `App::focus`
keeps when the focus went and the world's tick then; when it comes
back after `FOCUS_AWAY` (three minutes) or more, the same digest the
key would bring (`Digest::since` the tick of the blur: merges, blocks,
assumptions, conversations, what waits, the important lines; built from
the journal, no model asked) opens at once. Shorter absences show
nothing, and the key-driven one stays as it was for a person who stayed
in the window. It does not open over what the person is writing, the
help, the quit question or a window already open (another digest,
"where you left off"); the blur is forgotten then. With nothing new in
the journal there is nothing to show.

Pure screen logic over focus events: the demo shows it as live does,
in a terminal that reports focus. A terminal that does not report focus
(some multiplexers without `focus-events on`) only gets the key-driven
digest.
