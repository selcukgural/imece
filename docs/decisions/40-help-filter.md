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

## Scrolling (2026-11-02)

The person asked whether the help could scroll instead of every new row
having to fit 40 rows. It also had a gap: the window was only centred and
cut, so on a screen shorter than the help (or a narrow one, where it is one
column of 64 lines) its foot could not be reached at all, `/` aside.

Now the lines above the help's last line scroll where they do not fit:
the last line stays and starts with how many lines are below
(`help_more`, `↓ 12 more · j k scroll ·`) or, at the foot, `help_top`
(`↑ j k scroll ·`). The drawing records what it had room for
(`App::help_room`: lines in sight, the first line at most) and the keys
clamp to it (`App::help_scroll`): `j k ↓ ↑` a line, `^d ^u` half the room
(at least one), `^f ^b pgdn pgup` the room, `gg home` the top, `G end` the
foot. These keys never close the help, even where it fits: before, `j`
closed it, and that one habit changes. Every other key still closes it,
ctrl ones too. The help opens at its top, and a change to the filter
shows it from the top. At least one line is in sight on any screen.

So the 40-row test now only says the help fits 40 rows whole on a wide
screen; a row more makes it scroll there instead of being cut.
