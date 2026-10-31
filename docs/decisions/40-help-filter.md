# Filtering the help

`/` in the `?` help (2026-10-31, backlog C3 in #56, from herdr; issue
#79): the help lists over forty keys and four legend lines, and finding
one meant reading them all.

In the help window `/` starts a filter, shown on the window's last line
with a cursor. Every key typed goes into it (`?`, `q` and `/` too), so
the help stays open while the person types; backspace takes a letter
back, `^u` empties it. A key line or a legend line stays when it has
every word of the filter, in either column (`zM` finds the fold keys,
`first last` finds `gg G`). Case and the Turkish letters fold to plain
Latin ones (`calis` finds `çalış`, `BÜTÇE` finds `bütçe`), so the
Turkish help can be searched from a keyboard without them. Nothing
matching says so in the first row.

`enter` keeps the filter and ends the typing: the last line then says
`/` changes it and any other key closes, as the unfiltered help does.
`esc` drops the filter and the whole help is back; backspace on an empty
filter leaves it too. The help closes only after one of these, so a
filter is never typed into the screen behind it. A paste while the
filter is typed goes into it, on one line. The filter starts empty each
time the help opens.

The window keeps the size of the whole help while it is filtered (the
rows that went are blank), so it does not jump around while the person
types. `?` is now listed in the help itself, with the `/`. Pure screen
logic: the demo shows it exactly as live does.
