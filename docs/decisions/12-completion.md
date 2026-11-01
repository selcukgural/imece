# Completion

Completion (2026-10-29, user's ask: terminal-like, names and commands,
several steppable): `complete.rs`, pure `suggest(input, world,
project, live) -> Option<Menu>` the app calls after every change of
the input (`App::menu`): `@word` → agent names (project at hand first,
gone ones out, prefix match case aside, tail space), `@name /c` and
`/c` → `Agent.commands`, the command line the agent's SESSION reported
(`imece_agent::Event::Commands`: Claude from `init`'s `slash_commands`,
first seen and on change; Gemini from ACP `available_commands_update`;
Codex the adapter's own `/compact` mapping; `live/sessions.rs` adds imece's
`/clear` where missing), never a static list (user, 2026-10-29: a
vendor's new command must not need a release), so empty until a
session says (the demo's agents get their vendor's list when their
session starts, decision 31),
`:w` → imece's commands (with their values and `$`, decision
`colon-names`; a short name typed whole, `:b` `:q`, offers none so
`enter` runs it), `:lead w` → claude/codex/gemini/ask (no tail);
nothing for `!` or plain text; one match equal to the word = no menu.
Keys (`type_key`): tab inserts the highlighted item then steps,
⇧tab/↑/↓/ctrl-n/ctrl-p step with the text following (zsh menu style;
`Menu::inserted`), enter takes the item when not in yet else sends,
esc hides the menu then cancels. Since readline's keys
(decision 37) move the cursor, the menu is offered only with the
cursor at the end (a completion replaces the word the line ends with):
moving off the end hides it, `tab` there does nothing, coming back
offers it again. Drawn as one row above the footer
(`completions` in ui/mod.rs: window that keeps the chosen item in
view), footer hint `pick_cancel`. No path completion after `!`.

A list that never comes (2026-10-31, #53): what is fixed in code is
the wire format, so a vendor that renames `slash_commands` or stops
sending `available_commands_update` would leave only imece's `/clear`
with nothing said. `imece_agent::Event::CommandsMissing` tells a
missing list from an empty one, once a session: Claude on an `init`
whose `slash_commands` is not a list at all (absent, `null`, a
string; `[]` is a list and stays quiet), Gemini when a turn ends (or is
refused) and no `available_commands_update` came yet (a clean Gemini
sends it as the session starts; a loaded one with its history). Codex
lists none by design and says nothing. The screen puts a line in the
agent's trace and `CommandsMissing` in the journal ("{agent}'s session
listed no commands of its own; {vendor}'s format may have changed, so
only /clear completes", both languages; the trace is the raw stream
and stays English, so the worded line is the journal's). The demo
shows it on the first Gemini session of a run, whose list is then
only `/clear`.

Record 65 adds the `&` line's menu on the same `Menu`: the places to
go, numbered as a list frozen when the line opened, with notes.

Record 67 adds files and leased symbols for any word of a message.
