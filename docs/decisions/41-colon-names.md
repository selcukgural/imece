# Command names and values

The `:` menu and the names of keys (2026-10-31, backlog C4 in #56, from
Warp and cmux; issue #81). Before this, the `:` menu listed seven words
and told nothing about them, and what a key did could be done only with
that key, in the view where it works.

Every key that acts has a `:` name (user, 2026-10-31: actions, not
movement): `:salon` `:board` `:code` (`1 2 3`), `:lang` (`L`; `en` or
`tr` after it sets one), `:theme` alone (`T`, the next built-in one),
`:help` (`?`), `:quit` (`q`: it asks, a `q` after it quits), `:pending`
(`q s`), `:next` `:defer` (`W S`), `:waiting` (`w`), `:needs` (`f`),
`:fold` `:unfold` (`zM zR`), `:trace` (`t`), `:talk` (`v`), and in the
demo `:pause` `:faster` `:slower` (`space + -`). A name does what its
key does from wherever the person is: the ones whose key works only in
the salon's panels (`:waiting`, `:needs`, `:fold`, `:unfold`) go to that
panel first. `:trace` and `:talk` take `@NAME` (or `NAME`) like
`:forget`, completed the same way; none named is the agent at hand.
Movement keys (`j k`, `gg G`, `enter`, `esc`, `tab`, `, .`) and the
keys that act on the selected row (`a r c`) have none: typing them would
be slower than the key and the row would have to be named. Live,
`:pause` `:faster` `:slower` are left out of the menu and typed anyway
say they are for the demo. The key handlers and the commands call the
same `App` methods, so the two cannot drift apart. Short names (`b c f
o t`, and now `q`) are one table, `complete::SHORT`; one typed whole is
a command already, so the menu offers nothing and `enter` runs it (`:q`
⏎ asks at once; before, `:b` ⏎ took `budget` from the menu first).

The menu shows each command's value after it, in brackets, drawn but
never inserted (`Menu::notes`, filled by `App::command_note`, so
`complete.rs` stays pure): `budget (spent $3.10 of $20.00)`, `lead
(claude)` or `(ask)`, `theme (dark)`, `lang (English)`, `close` and
`rename` the project at hand, `forget` `talk` `trace` the agent at
hand, `next` and `waiting` how many items wait, `needs` and `pause` on
or off, `faster` `slower` the speed (`×2`). Names in a note go through
`clean` with the rest of the row, and the window that keeps the chosen
item in view counts the notes.

`$` marks what raises the next bill (user, 2026-10-31): `:open` (a new
leader session) and `:forget` (the next message starts a new session
that pays the whole briefing again, without the cache). No `:` command
starts a paid turn by itself; `:budget` shows its value instead of a
mark. The `?` help gives `:` and `$` a line each, and every key's line
names its `:` name (in the key column where it fits, else in the text),
in both languages. Pure screen logic: the demo shows it as live does.

Record 65 adds `&` = `:goto [NAME]`.
Record 66 adds `^r` = `:history [WORDS]`.

Record 76 adds `:keys`.

**What is out of view, and what a command does (the person, 2026-11-04:
"there are many `:` commands and not all show; one does not know the
others are there unless tabbing on by chance"; and "after `tab`, let one
see what the command does, as a hint, gone once one writes").** The
menu's row (`ui::completions`) shows every item when all fit, else a
window around the highlighted one with `‹ N` before it (how many are
before the window) and `+N more ›` after it (`menu_more`, `+N tane daha
›`), both counted into the room so nothing is cut; the line under it
adds where the highlight is, `· 3 of 63` (`menu_at`, `3/63`), for every
menu of more than one item. Once `tab` (or a step) put a `:` command on
the line (`Menu.inserted`, `start == 1`, the line starts with `:`), what
it does follows the keys in the info colour (`ui::command_hint` from
`Text::command_hints`, one line per command of `complete::COMMANDS` in
its order, both languages, a test holds the order and that none is
missing); writing on makes the menu again with nothing put in, so it
goes. An argument's menu, `@` and the rest have no such line. Pure
screen logic: the demo shows it as live does.

Acceptance fixes (2026-11-09). The `?` help leaves the demo's keys out
live as the `:` menu leaves out their names: `overlay::DEMO_KEYS` (the
rows `space` and `+ -`), `help_rows(.., demo)` from `App::demo`. A
command's whole name with the menu's highlight on it runs on `enter`, as
a short name does (`App::whole_command`: the menu not put in, `start` 1,
the highlighted item equal to the word typed): `:archive` runs though
`archives` is listed too; a highlight moved to another name, a word only
begun or an argument's menu takes the entry as before. A name nothing
else begins with (`:news`, `:commit`, `:welcome`) shows no menu at all
(`complete::menu` offers none for a single item equal to the word), so
`enter` ran it already; the acceptance run's `:news` that took the entry
first was not reproduced by a test.
