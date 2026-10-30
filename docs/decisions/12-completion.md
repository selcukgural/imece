# Completion

Completion (2026-10-29, user's ask: terminal-like, names and commands,
several steppable): `complete.rs`, pure `suggest(input, world,
project, live) -> Option<Menu>` the app calls after every change of
the input (`App::menu`): `@word` → agent names (project at hand first,
gone ones out, prefix match case aside, tail space), `@name /c` and
`/c` → `Agent.commands`, the command line the agent's SESSION reported
(`imece_agent::Event::Commands`: Claude from `init`'s `slash_commands`,
first seen and on change; Gemini from ACP `available_commands_update`;
Codex the adapter's own `/compact` mapping; live.rs adds imece's
`/clear` where missing), never a static list (user, 2026-10-29: a
vendor's new command must not need a release), so empty until a
session says and in the demo,
`:w` → `budget`/`lead`, `:lead w` → claude/codex/gemini/ask (no tail);
nothing for `!` or plain text; one match equal to the word = no menu.
Keys (`type_key`): tab inserts the highlighted item then steps,
⇧tab/↑/↓/ctrl-n/ctrl-p step with the text following (zsh menu style;
`Menu::inserted`), enter takes the item when not in yet else sends,
esc hides the menu then cancels. Drawn as one row above the footer
(`completions` in ui/mod.rs: window that keeps the chosen item in
view), footer hint `pick_cancel`. No path completion after `!`.
