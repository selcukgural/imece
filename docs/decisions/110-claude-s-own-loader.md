# CLAUDE.md through Claude Code's own loader

The person's decision (2026-11-04), after asking why imece took only 16 KiB
of some files when Claude Code loads them by itself in a plain session. It
changes records 10 and 88. The 16 KiB cap (`MAX_NOTES`, commit 20c0b5d) had
no reason on record; `--restricted`, which imece wants for hooks and settings,
dropped `CLAUDE.md` as a side effect, and imece had put the file's head back
by hand.

Probes (2026-11-04, Claude Code 2.1.318, `-p --restricted`, haiku, a code word
in each file):

- `CLAUDE_CODE_ADDITIONAL_DIRECTORIES_CLAUDE_MD=1` with `--add-dir DIR` loads
  DIR's `CLAUDE.md` whole (a 40 KB tail was seen), its `@imports` inside DIR
  or the working directory, `.claude/CLAUDE.md`, `.claude/rules`; not
  `CLAUDE.local.md`. Without the variable: nothing. A hook in DIR's
  `.claude/settings.json` does not run; skills and commands do not change.
- DIR may be the working directory itself, so a worker gets its notes from its
  own worktree and its file tools reach nothing new. The person's checkout is
  never added: a worker's file tools would reach it past the Δ gate. Nor
  `~/.claude`, for the same reason.
- `@/abs/path` and `@../path` out of those directories are not loaded, from the
  working directory's file or another's: a repository cannot pull in a file of
  the person's that way.
- `--resume` with the same flags sees the file as it is now, not as it was.

What runs. `claude::session_args` gives a session `--add-dir` for its own
directory (not under open: Claude Code reads it there anyway) and for imece's
notes directory when there is one; `claude::loads_notes` sets the variable.
`child::command` clears the environment, so imece sets it itself and it is not
in `claude::ENV`. A one-shot call (`:commit`, `:review`, `claude::once`) reads
no notes: it pays by the token and answers about one diff.

The notes directory, `<worktrees>-notes/<name>` (`Live::notes_of`), is imece's.
Its `CLAUDE.md` (0600, the directory 0700, `make_private`) is rewritten at
every start by `prompts::notes_text` and `sessions::write_notes` and holds, in
this order and whole: the person's `~/.claude/CLAUDE.md`
(`claude::own_notes`, `CLAUDE_CONFIG_DIR` first; not under open, where Claude
Code reads it), imece's `instructions.md` (record 88), and the repository's
notes the loader would not find in the agent's directory: the checkout's
`CLAUDE.md` when the tree has none in either place (git ignores it, or a
worker started where it is not), else its `AGENTS.md`. None of them, no
directory and no flag.

One directory per agent, so what an agent writes there reaches no other; a
writer's file tools reach it, as they reach its scratch. So the file is removed
and made new (`create_new`), never written through a link left in its place.
`AGENTS.md` and `CLAUDE.md` of the checkout count only inside it (`canonicalize`
+ `starts_with`): a link out of it is the repository asking for a file of the
person's. `Live::drop_scratch` removes the directory too: `:forget`, `--fresh`,
an agent that leaves.

`instructions.md` moved out of `--append-system-prompt` with the rest: Linux
allows one argument 128 KiB, so uncapped text on the command line would fail
to start. `Event::NotesCut`, `Event::InstructionsCut`, their words and the
demo's two cases are gone: nothing is cut. A journal line of either is passed
over at load, as any kind the build no longer has.

Known gaps. An `@import` in `~/.claude/CLAUDE.md` is not followed in the copy:
one relative to `~/.claude` names nothing there, one elsewhere is out of bounds.
`CLAUDE.local.md` is not loaded, and the person's skills, commands and settings
stay out under standard and strict. A worker reads the `CLAUDE.md` of its
worktree, so a change still uncommitted in the checkout reaches it after it is
committed and the worktree moves. Whether the loader saw the files is not
checked: `init` does not list them.
