# imece

**One terminal for many coding agents from many vendors.**

imece (Turkish: neighbours finishing a job together) runs a guild of coding
agents (Claude Code, Codex, Gemini and any agent that speaks
[ACP](https://agentclientprotocol.com)) on one codebase, on one screen. A
leader agent splits the work into cards and hands them out; the agents talk
to each other, ask a capped pool of researchers, and lease the symbols they
change so several of them can work in the same file.

> Status: early. imece runs real Claude Code and Codex agents on one project
> (a leader that hires workers and talks to you),
> or a seeded simulation with `--demo`.

## Run it

```sh
cd your/checkout
cargo run -p imece-tui                       # or, once installed: imece
cargo run -p imece-tui -- --agents 8 --budget 10 --model sonnet
cargo run -p imece-tui -- --lead codex                   # Codex leads; it may still hire claude workers
cargo run -p imece-tui -- --lead gemini --tier gemini:strong=gemini-2.5-pro
```

You do not pick a team size. `lead` starts alone, reads the checkout (it
can not change anything), and hires workers as the work needs them, up to
`--agents` (default 4). The workers, `claude-1`, `claude-2`, … each work in
their own git worktree under `~/.local/state/imece/worktrees/`, detached at
your HEAD. Your checkout is never touched by an agent. When a worker sends
its `done`, what its worktree holds shows up in WAITING FOR YOU as a
change (`Δ`): `enter` opens the whole diff (`j k` scroll, `[ ]` file to
file), `a` applies it to your checkout, staged, for you to look over and
commit, `r` rejects it and opens the composer so you say why. A change
that does not fit (you edited the same file, or the same lines moved on)
is refused with git's words and nothing is half applied. An applied
change is committed in the worktree as a mark (author `imece`), so the
worker's next `done` shows only what came after, and the worktree still
follows your HEAD once you commit. The trace (`enter` twice) shows where
each worktree is. A worktree is kept between runs: one found clean at an
older commit is moved to your HEAD when its agent next starts and between
turns; one with uncommitted changes or commits of its own stays where it
is, and the journal says so either way.

- Plain text goes to the lead, `@claude-2 text` to one agent. A message to
  an idle worker opens a card; its answer closes it.
- A line that starts with `/` is a command of the agent's own CLI, not a
  message: `/clear`, `@claude-2 /compact`, `/cost`, `/context`, whatever
  the vendor's command line knows. It runs as a turn and its answer comes
  back like a reply. Claude Code and Gemini CLI take their commands as
  they are; Codex's app server has no command line, so only `/compact`
  works there, and `/clear` on a vendor without one of its own ends the
  session (the next message starts a fresh one).
- Every agent gets the repository's notes for coding agents, its
  `AGENTS.md` or `CLAUDE.md` (at most 16 KiB), with its instructions: the
  vendors' CLIs are started so that nothing of the repository loads, which
  would drop them too. Codex reads `AGENTS.md` on its own.
- Every agent has a `run` tool: one program and its arguments, no shell
  (pipes, redirections and variables are refused), in the agent's own
  directory, killed after `--run-timeout` seconds (default 600) and with
  imece. Allowed: `cargo` without the commands that change dependencies
  (`add`, `remove`, `install`, `publish`, …; a leader also without the
  ones that change files, so `fmt --check` but not `fmt`), git's reading
  commands (`status`, `diff`, `log`, `show`, `blame`, `grep`, `ls-files`,
  `rev-parse`), and for workers and helpers the programs you name with
  `--run npm,make`. Output comes back tidied: colours stripped, lines of
  passing tests dropped, repeats collapsed, the middle cut past 48 KiB.
  Each worktree builds in its own `target/` (a shared one would make the
  workers rebuild each other's changes in turn).
- `:budget 80` raises (or lowers) the budget while agents run. A turn that
  is under way finishes on its old cap; the next one starts on the new
  remainder, in a new session if the old one had hit its cap.
- Writing completes as a shell does: after `@` the agents' names (the
  project at hand first), after `:` imece's commands and, after `:lead`,
  the vendors, after `/` (or `@name /`) that agent's own command line.
  The matches show above the footer as you type; `tab` puts the first in
  and then steps on, `⇧tab` back, `↑` `↓` and ctrl-n / ctrl-p step too,
  the text following; `enter` takes the highlighted one when it is not in
  yet, else sends; `esc` hides the menu, and cancels the writing after.
- `!cargo test` runs a command of your own in your own shell (`$SHELL`),
  in the checkout, as `:!` does in vim: the screen steps aside, the
  command has the terminal to itself (so `git log`, `less` and `vim`
  work, and ctrl-c stops the command, not imece), a line says how it
  ended, and any key brings the screen back. The journal keeps the
  command and its exit code; the output stays on the terminal's main
  screen, where you see it again when imece quits. Nothing of it reaches
  an agent.
- Models: every agent runs on its vendor's own default (what you set up in
  Claude Code or Codex), or on `--model`. When the lead hires, it picks a
  tier, not a model: `cheap` for small mechanical work, `standard` (the
  default) for ordinary fixes, `strong` for cross-cutting design. Claude's
  tiers are haiku, its default, opus; Codex's are reasoning efforts on its
  default model, `@low`, its default, `@high`; Gemini's are its own aliases
  `flash`, its default (`auto`), `pro`. Change what a tier means
  with `--tier strong=opus[1m]` or `--tier codex:strong=gpt-5.6-terra@xhigh`
  (`MODEL@EFFORT`); without a vendor prefix, `--model` and `--tier` speak
  of the leader's vendor. A tier model your account does not have fails
  its first turn; imece then restarts that worker on the standard model
  and says so in the journal.
- Vendors: imece looks for `claude`, `codex` and `gemini` on PATH and
  drives all three; `--lead` names the leader's vendor, and the lead may
  hire workers from any of them (`hire` takes a vendor).
- `--confine standard|strict|open` says how much of its vendor each
  agent gets. `standard` (the default) gives workers and helpers the
  vendor's full coding toolset (read, write, shell, web search and fetch)
  and the lead the web, with nothing of your own vendor setup loaded;
  `strict` gives file tools only, with commands through imece's `run`
  tool; `open` runs the vendor as you have it set up (your settings,
  hooks, MCP servers, skills, extensions). At every level the lead never
  writes (it works in your checkout, where a write would skip the review)
  and the vendor's own sub-agent tool stays off (a sub-agent spawned
  inside the CLI would skip the worktree, the budget, the bus and the
  screen). imece sandboxes nothing: it switches on each vendor's own
  restrictions for the level and checks, fail closed, that the session
  runs with them; isolation is a container's job. The journal says so at
  every start that is not strict.
- A Claude Code session is a long-lived `claude -p`, under standard and
  strict in `--restricted` mode: no user or project settings (so no
  hooks), no MCP servers but imece's own, file access confined to the
  agent's directory. Strict names the file tools only and refuses what
  would ask for permission; standard names the shell and the web tools
  too and allows the web outright; a shell command Claude Code would ask
  about comes to imece, which runs a routine one at once (one program
  and its arguments, no shell syntax, what the `run` tool would run:
  the build, the tests, git's reading commands) and puts any other in
  WAITING FOR YOU (`⚑`, shown whole) for you to allow or refuse; open
  drops `--restricted` and gives a
  worker Claude Code's default toolset less the sub-agent tool, with your
  own settings, hooks and MCP servers loaded. What Claude Code reports it
  runs with is checked on every turn; anything else stops the session.
- A Codex session is a long-lived `codex app-server` (JSON-RPC on stdio).
  Codex always has a shell, so the sandbox does the confining: read-only
  for the lead, workspace-write (the agent's directory) for workers and
  helpers; approvals are never asked, so what the sandbox refuses simply
  fails. Under strict the sandbox has no network and web search is off;
  under standard a worker's sandbox reaches the network and web search is
  live; both turn plugins, apps, hooks, sub-agents and memories off, write
  no session files, and name off the MCP servers and skills your own Codex
  config brings along (imece starts Codex a second time to do that when it
  has to); open keeps your config as it is, with only the sub-agent
  feature off. What Codex reports it runs with, from the config to the
  connected MCP servers to the thread's own sandbox, is checked before the
  thread starts and the MCP servers again before every turn; anything else
  stops the session.
  Codex reports tokens, not dollars: on a ChatGPT plan its turns cost the
  budget nothing (the trace says so, and shows the tokens); an API-key
  login is refused, since imece can not count its cost yet.
- A Gemini session is a long-lived `gemini --acp` (the Agent Client
  Protocol, JSON-RPC on stdio). Gemini CLI has nothing like `--restricted`
  and takes its settings from your own `~/.gemini` and from the `.gemini`
  directory of the folder it runs in, so imece runs it in a private
  directory of its own (under the worktrees, next to them) and gives it the
  agent's files as an included directory: a repository's `.gemini`
  settings, hooks and `.env` never reach the session. A policy file imece
  writes there allows exactly the tools of the role and the level (read
  tools for the lead, read and edit tools for workers and helpers; from
  standard on the web for everyone and a shell for workers and helpers,
  all without a prompt) and imece's own tools, and under strict and
  standard denies every other tool (a denied tool is not even shown to
  the model); extensions are off, no MCP server but imece's is allowed,
  and every permission request is refused. Under open your extensions and
  MCP servers load and run under your own policies (what would ask is
  still refused); imece denies by name only Gemini's agent tools and, for
  the lead, the writing tools. The agent's instructions go in a
  `GEMINI.md` in that private directory, the only way ACP has to give
  them. What Gemini reports (the session's mode and model, and its
  `/about`, `/extensions list` and `/memory list`, which cost no tokens)
  is checked before the first turn; anything else stops the session.
  Gemini reports tokens, not dollars: with an API key its turns are booked
  from the list prices imece carries (prompts up to 200K tokens, thought
  tokens as output; a model imece can not price stops the session, so name
  one it knows with `--model` or `--tier`); with a Google login they cost
  the budget nothing, and the trace says so. Gemini keeps its chat files
  under `~/.gemini/tmp` as it always does.
- The budget (`--budget`, $50 the first time) is one pot for the whole
  team, kept between runs: once all agents together have spent it, nobody
  takes another turn. Each Claude Code session also gets what was left of
  the pot when it started as its own hard cap, so a session running away
  on its own cannot pass it either.
- What imece knows is remembered between runs in one SQLite file
  (`$XDG_STATE_HOME/imece/imece.sqlite`, yours alone): projects, agents,
  cards and their stories, the journal, the inbox, mail that waits and the
  board. The next `imece` shows the same screen, every agent asleep: no
  process and no cost until you write to one, or a teammate's message or
  a new card wakes it. A woken Claude agent goes on in the session it
  had, with everything it knew, when Claude Code still has its transcript
  for that directory; otherwise (and for the other vendors, for now) it
  starts a fresh session and is told where it left off (its card's story,
  a turn that was cut short, its worktree's changes, the board). The
  journal says which. One imece at a time has the file; a second one is
  told to use `:open` from the first.
- Child processes get a cleared environment (plus what the vendor needs to
  log in), their own process group (killed on exit), and capped pipes.

## Run the simulation

```sh
cargo run -p imece-tui -- --demo --projects 3 --agents 30   # --researchers N, --seed N
cargo run -p imece-tui -- --demo --lang tr                  # or IMECE_LANG=tr; English by default
```

Each project is a group in the salon with its own leader. A folded project
is one line, and that line still shows what waits for you.

| Key | What it does |
| --- | --- |
| `/` | write to the selected project's leader; it splits the request into cards |
| `@name text` | write to one agent |
| `/cmd`, `@name /cmd` | the agent's own command line (`/clear`, `/compact`, `/cost` …) |
| `:budget USD` | raise or lower the budget while agents run (`:b 80`) |
| `!cmd` | run `cmd` in your own shell; the screen steps aside until a key |
| `tab` `⇧tab` `↑` `↓` | while writing: complete an agent's name (`@`), a command (`:`, `/`) |
| `1` `2` `3` | salon · board · code map |
| `↑` `↓` / `j` `k` | select |
| `←` `→` / `h` `l` | fold / unfold a project (`enter` on its header too) |
| `L` | English / Türkçe |
| `enter` / `esc` | look deeper (focus, then the raw stream) / go back |
| `tab`, `w` | salon ↔ the items waiting for you · straight to them |
| `a` / `r` | approve / apply / allow · reject / refuse the selected item |
| `space`, `+` `-` | pause, simulation speed (`--demo` only) |
| `?`, `q` | help, quit |

## Screen principles

1. **Calm by default.** Nothing scrolls unless it means something; a change
   stands out for a moment, then calms down.
2. **You talk to the leader**, not to thirty agents.
3. **Three depths everywhere:** the guild, one card's story (intent,
   decisions, assumptions, conversations, evidence), the raw stream.
4. **An attention budget:** only decisions that need you reach the inbox;
   calm agents fold into one line.
5. **Coming back is cheap:** after a pause, the first key shows what happened
   while you were away.

## Security

Please report security problems privately; see [SECURITY.md](SECURITY.md).

## License

Licensed under the [Apache License 2.0](LICENSE).
