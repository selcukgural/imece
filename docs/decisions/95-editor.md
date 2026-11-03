# A long message in the person's editor

Issue #243 (from the study of agent-deck, which opens its draft in
`$VISUAL` / `$EDITOR` with `Ctrl+X E`). The command line takes several
lines (record 39), but a long message to the leader, a plan or a review,
is easier to write where the person writes everything else.

`^x ^e` while writing (bash's and zsh's `edit-and-execute-command` /
`edit-command-line` key) and `:edit [TEXT]` (record 41: every key that
acts has a `:` name; its draft is `TEXT`, nothing when bare) hand the
draft to the main loop (`App.editor`), which runs `editor::run`: the
screen steps aside as for a `!` command (`shell::step_aside`, record 13),
the editor runs in the person's own terminal in the project's checkout
(`App::cwd`), and the screen comes back (`shell::come_back`) whatever
happened. `App::edited` puts what the editor left on the line, the
cursor at its end, cleaned as a paste is (`pasted`: line ends kept,
control characters out); nothing is sent, nothing is kept in the lines
sent until `enter` sends it. `^x` followed by any key but `^e` is
dropped and the key does what it does alone (`App.ctrl_x`, taken at
every key while writing).

The editor: `$VISUAL`, else `$EDITOR`, else `vi` (`editor::chosen`, one
set to blanks counts as unset), as git and crontab pick it; the `:` menu
notes the one that would open. It is a line of the person's shell words
(`code --wait`, `emacsclient -t`), so it runs as `/bin/sh -c '<editor>
"$1"' imece-edit FILE`, as git runs `core.editor`: the file's name is an
argument, never spliced into the line. While it runs imece ignores
SIGINT and SIGQUIT and the child puts them back to the default before
exec (as `system(3)` does), so ctrl-c in an editor that leaves the
terminal cooked ends the editor, not imece. ctrl-z is the editor's: an
editor that suspends itself stops imece's process group with it, as it
does under git; `fg` brings both back.

The draft's file: `state/drafts/draft-PID-N.md` (`.md` so the editor
highlights Markdown), the directory made 0700, the file created new
(`create_new`, so never a file or link already there) with mode 0600,
the draft written with a final line end, removed when the round ends
(`editor::Draft`'s drop), on every path. What comes back: at most
`editor::MAX` (1 MiB) of UTF-8, the line ends at its end taken off.

Failure keeps the draft as it was on the line. Why is said on the
terminal, under what the editor left there (its own complaint, such as
`sh: nvim: not found`), until a key brings the screen back
(`App::editor_back`, `shell::wait_for_key`), as a `!` command's end is:
while writing, the bottom line shows the line, never a flash, so a
flash would go unseen. The reasons (`words::editor_why`): a non-zero
exit (vim's `:cq` is the way to give up), with 127 said as no such
editor and `$VISUAL` / `$EDITOR` named; a signal; more than 1 MiB; not
text; the file or directory not made. Success says nothing: the text on
the line is the answer. No file is left behind.

Security: the text is the person's, typed into their own editor; it
reaches an agent only when they send it, like a paste. The file sits in
imece's state directory (the person's only), never in the worktree or a
shared temp directory, and lives only while the editor runs. The editor
command is the person's own environment, run as their shell would.

Not in an archive's view (`read_only`; `edit` is not in
`VIEW_COMMANDS`), whose line sends nothing. The demo runs the same main
loop, so it opens the same editor. Not done: a key outside writing, a
draft kept between runs.
