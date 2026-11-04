# A standing yes for a project's commands

Issue #229 (the person, 2026-11-03: "pressing a for every bash command is
annoying; you cannot leave the computer"). Before it, every command an
agent asked about that was not routine (record 30) waited for the
person's `a`, one by one, and nothing held.

**Decided (the person, 2026-11-03):** a yes from now on is matched by the
command's program and its first argument, like Claude Code's
`Bash(npm run:*)`, and it is the project's (every agent of it), saved in
the store, listed and taken back with `:allow`.

- **A rule** (`model::Standing { program, sub }`, `live::standing::rule_of`):
  the program and its first argument when that is not an option (`npm
  run`, `dotnet ef`, `rm target`), the program alone when the command is
  only that (`make`). A command whose first argument is an option (`rm
  -rf target`, `sh -c …`) can have none (`Why::OptionFirst`): an option
  before what a program works on can change what it runs (`npm --prefix
  /x run`, `git --exec-path=… log`), so it never matches a rule either.
  What follows the first argument is not looked at: `npm run …` is every
  `npm run`, so a `find .` rule covers `find . -delete` too; the person
  chooses what they say yes to, the ask shows the whole command.
- **Matching** (`standing::matches`): the same program (as written: `./x`
  is not `x`) and the same first argument (none: the program alone); for
  git none of the options that start a program (`tools::git_denied`).
- **A line of several commands** (`standing::commands`): split outside
  quotes at `&&`, `||`, `;`, `|` and a new line, each part through
  `run::split`. A line is covered (`standing::covered`) when every command
  is routine for the role (`tools::allowed`, as the run tool) or has its
  rule; so a chain of routine commands (`cargo build && cargo test`) goes
  on too. `cd` is not special: `cd x && …` asks unless `cd x` has a rule.
- **What a shell reads in a line** (the person, 2026-11-04: `A` refused
  every such line, so `a` was needed each time). A rule already takes
  whatever follows its two words, so an expansion there adds nothing to
  it: `$NAME`, `${NAME}`, an unquoted glob (`*`, `?`, `[`) and `$(…)` are
  kept in a word as `standing::EXPANDED`, and the commands a `$(…)` runs
  (at most `MAX_NESTED` 4 deep) count as commands of the line, so they need
  a rule or to be routine too. Three limits, each because a list of what is
  denied reads the words and an expansion hides them: never in a rule's
  own two words (`rule_of`), never for what is routine (`covered`'s
  `known`: `cargo $X` may be `cargo publish`), never for a rule of git's
  (`matches`: `git grep $X` may be `-Ovim`, a file too). A redirection that
  leaves the disk as it was (`2>&1`, `1>&2`, `>&2`, `N>/dev/null`,
  `&>/dev/null`) is dropped (`past_no_effect`). The rest of what a shell
  reads (a backtick, a redirection to a file, `<`, `(`, a lone `&`,
  `${X:-…}`, `$?`, an open quote, an empty part) cannot be parsed:
- **A yes to one whole line** (`Standing.whole`, `Standing::exactly`):
  when a command of the line can have no rule that would hold (it cannot
  be parsed, an option first, an expansion in the two words, git's denied
  options), `A` says yes to that line alone, byte for byte but for the
  spaces round it (`needed`), shown `` `rm -rf target` ``. Only that line
  matches (`said_whole`), never as a command: `'foo bar'` runs another
  program than the line `foo bar`. It is the person's word on what they
  read, so it stands for what git's list would refuse as written; what the
  line does may still change with the files it reads, as `npm run x` does.
  `program` holds the line, so the field adds no schema: an older imece
  reads it as a rule no command matches.
- **`A` on an ask** in WAITING FOR YOU (`App::answer_always`, only on an
  approval at hand; elsewhere `A` still opens the archives): a rule for
  each command of it not yet routine or allowed (`standing::needed`)
  goes into the project's list (`World::stand`: sorted, once each,
  journaled `shop-api: you say yes from now on to npm run …` as
  `Event::Standing`), the ask is allowed (`Live::decide`), and so is
  every other open ask of the project the list now covers
  (`Live::go_on_allowed`). An ask that is not a command (an edit, a
  fetch: `Why::NotACommand`) can have none: the flash says why and that
  `a` allows it once; nothing is decided.
- **Later asks** (`Live::ask`, after the routine check):
  `Live::standing_yes` lets a covered one go on at once, traced `allowed
  by your yes from now on: Bash: …`; it is not journaled (as a routine
  one is not). Claude's, Codex's and Gemini's commands all come through
  here (`Ask::Command`).
- **`:allow [PROGRAM [WORD]]`** (`App::allow`): bare, the list of the
  project at hand (`shop-api: yes from now on to make, npm run …`, or how
  to add one); with words (`standing::named`: one command of at most two
  words), it is added (the project's open asks it covers go on) or, named
  again, taken out (`World::unstand`, journaled `no standing yes any more
  to …`); a yes to one whole line is named by that line, as listed or
  bare, and only taken out. The `:` menu shows the list. In an archive's
  view the list is read only.
- **Store** schema 18: `projects.allowed` (JSON, `[]` for older files);
  a list that is not one refuses the file. Part of the project's hash,
  so a change alone is saved.
- **Demo** (`Sim::always`, `Sim::go_on_allowed`, `Sim::approval`): its
  asks are `Bash: …` lines of the project's kit, one with a `$(…)` and one
  with a redirection to a file among them; one its list covers is not
  asked, traced the same way; `A` and `:allow` work as live's.

On a focus `a`, `r` and `A` decide that agent's own ask (record 103),
and waiting for you's row now names `A` (`a yes · A from now on · r
no`).

A yes for one agent until its turn ends is record 104 (`e`, `:yes`).

Not done: a rule for one agent or for this run only, an expiry, a rule
for a file edit or a fetch, rules across projects, a wider match than
program + first argument.
