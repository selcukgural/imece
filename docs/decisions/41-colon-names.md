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
