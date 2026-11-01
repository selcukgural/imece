# The jump line: `&` and `:goto`

Backlog item C1 of #56 (2026-11-01, from herdr, Orca, OH and Warp; issue
#144): a jump palette over agents, cards, projects and commands, what
needs the person first, digit keys, the order frozen while open.

What was built. `&` opens a jump line on the completion menu that `@`,
`#` and `:` already use (record 12): no new widget. The line's menu lists
the places to go, each `&N name` with a note: an agent's rung glyph and
its row's status words (record 62, done-and-not-seen worded as the row
does, cut to thirty characters), a project's the word `project`. The list
(`App::jumps`, from `App::jump_list`) is made when the line opens and
held until it closes, so the numbers do not shift under the person while
they type and the world moves on: the agents of open projects by `Need`
(record 38: waiting, stuck, done-unseen, nothing), then by `Rung` (record
61: working before idle), then the salon's project order and the agent's
id; then the open projects in the salon's order. Typing after `&` filters
(`complete::jumps`): digits match the start of a number, `!` first keeps
only what needed the person when the list was made, anything else is
words that must all occur inside the name, case and Turkish letters
folded as the help's filter does (record 40, `ui::fold`). A digit `1`-`9`
right after `&` goes at once; `#` right after `&` turns the line into the
card line of record 32; `0` only filters. `enter` takes the item under
the cursor (the menu inserts `&N name` and the line is submitted at
once, as the card line is), else what was typed: `complete::jump_named`
reads the number when the first word is all digits, the words otherwise.
A number past the list's end says `nothing called "9" to go to` and the
line closes; a bare `&` and `enter` does nothing.

`:goto [NAME]` is the same by name (`App::goto`; `@` before the name is
dropped): the agent of that name among open projects, else the open
project; alone it opens the `&` line. Going to an agent (`go_agent`)
puts the salon up with its row selected, unfolds its project and, for a
helper, its worker's helpers, and turns `f` off when the agent needs
nothing (record 38 would hide the row otherwise); a project selects its
header. The footer over the line says `› go to:` (`typing_jump`). The
help row `&  :goto` was added and the `n p` row merged into the `#N
#word` row, so the help keeps its 55 rows and the forty-row window test.
Pure screen logic: the demo shows it as live does (record 31).

Not done, from C1's line: the `@` prefix (a message to the agent is `@name`
already, record 3), the `>` prefix over commands (`:` and `tab` list them,
record 41), cards on the `&` list (`#` is one key away) and state filters
beyond `!`. A card on the list would need a card's `Need`, which only an
agent has.
