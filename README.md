# imece

**One terminal for many coding agents from many vendors.**

imece (Turkish: neighbours finishing a job together) runs a guild of coding
agents (Claude Code, Codex, Gemini and any agent that speaks
[ACP](https://agentclientprotocol.com)) on one codebase, on one screen. A
leader agent splits the work into cards and hands them out; the agents talk
to each other, ask a capped pool of researchers, and lease the symbols they
change so several of them can work in the same file.

> Status: early. imece runs real Claude Code and Codex agents on one project
> (a leader that hires workers and talks to you; a Gemini adapter is next),
> or a seeded simulation with `--demo`.

## Run it

```sh
cd your/checkout
cargo run -p imece-tui                       # or, once installed: imece
cargo run -p imece-tui -- --agents 8 --budget 10 --model sonnet
cargo run -p imece-tui -- --lead codex                   # Codex leads; it may still hire claude workers
```

You do not pick a team size. `lead` starts alone, reads the checkout (it
can not change anything), and hires workers as the work needs them, up to
`--agents` (default 4). The workers, `claude-1`, `claude-2`, … each work in
their own git worktree under `~/.local/state/imece/worktrees/`, detached at
your HEAD. Your checkout is never touched; review an agent's work with
`git -C <worktree> diff`. The trace (`enter` twice) shows where each
worktree is.

- Plain text goes to the lead, `@claude-2 text` to one agent. A message to
  an idle worker opens a card; its answer closes it.
- `:budget 80` raises (or lowers) the budget while agents run. A turn that
  is under way finishes on its old cap; the next one starts on the new
  remainder, in a new session if the old one had hit its cap.
- Models: every agent runs on its vendor's own default (what you set up in
  Claude Code or Codex), or on `--model`. When the lead hires, it picks a
  tier, not a model: `cheap` for small mechanical work, `standard` (the
  default) for ordinary fixes, `strong` for cross-cutting design. Claude's
  tiers are haiku, its default, opus; Codex's are reasoning efforts on its
  default model, `@low`, its default, `@high`. Change what a tier means
  with `--tier strong=opus[1m]` or `--tier codex:strong=gpt-5.6-terra@xhigh`
  (`MODEL@EFFORT`); without a vendor prefix, `--model` and `--tier` speak
  of the leader's vendor. A tier model your account does not have fails
  its first turn; imece then restarts that worker on the standard model
  and says so in the journal.
- Vendors: imece looks for `claude`, `codex` and `gemini` on PATH. It
  drives Claude Code and Codex today; `--lead` names the leader's vendor,
  and the lead may hire workers from either (`hire` takes a vendor).
- A Claude Code session is a long-lived `claude -p` in `--restricted`
  mode: file tools only (no shell, no web), no MCP servers but imece's own,
  no user or project settings (so no hooks), file access confined to the
  agent's directory, nothing that would ask for permission is allowed.
  What Claude Code reports it runs with is checked on every turn; anything
  else stops the session.
- A Codex session is a long-lived `codex app-server` (JSON-RPC on stdio).
  Codex always has a shell, so the sandbox does the confining: read-only
  for the lead, workspace-write (the agent's directory, no network) for
  workers and helpers; approvals are never asked, so what the sandbox
  refuses simply fails. Web search, plugins, apps, hooks, sub-agents and
  memories are turned off, no session files are written, and the MCP
  servers and skills your own Codex config brings along are named off
  (imece starts Codex a second time to do that when it has to). What Codex
  reports it runs with, from the config to the connected MCP servers to
  the thread's own sandbox, is checked before the thread starts and the
  MCP servers again before every turn; anything else stops the session.
  Codex reports tokens, not dollars: on a ChatGPT plan its turns cost the
  budget nothing (the trace says so, and shows the tokens); an API-key
  login is refused, since imece can not count its cost yet.
- The budget (`--budget`, default $50) is one pot for the whole team: once
  all agents together have spent it, nobody takes another turn. Each
  Claude Code session also gets what was left of the pot when it started
  as its own hard cap, so a session running away on its own cannot pass
  it either.
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
| `:budget USD` | raise or lower the budget while agents run (`:b 80`) |
| `1` `2` `3` | salon · board · code map |
| `↑` `↓` / `j` `k` | select |
| `←` `→` / `h` `l` | fold / unfold a project (`enter` on its header too) |
| `L` | English / Türkçe |
| `enter` / `esc` | look deeper (focus, then the raw stream) / go back |
| `tab` | salon ↔ the items waiting for you |
| `a` / `r` | approve / reject the selected item |
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
