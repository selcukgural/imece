# imece

**One terminal for many coding agents from many vendors.**

imece (Turkish: neighbours finishing a job together) runs a guild of coding
agents (Claude Code, Codex, Gemini and any agent that speaks
[ACP](https://agentclientprotocol.com)) on one codebase, on one screen. A
leader agent splits the work into cards and hands them out; the agents talk
to each other, ask a capped pool of researchers, and lease the symbols they
change so several of them can work in the same file.

> Status: early. The screen runs on a seeded simulation, or on real Claude
> Code agents with `--live` (one project, a leader and workers you talk to
> yourself; agents do not message each other yet).

## Live agents

```sh
cd your/checkout
cargo run -p imece-tui -- --live --agents 3 --model sonnet --budget 5
```

- `lead` reads the checkout (it can not change anything); `claude-1`,
  `claude-2`, … each work in their own git worktree under
  `~/.local/state/imece/worktrees/`, detached at your HEAD. Your checkout
  is never touched; review an agent's work with `git -C <worktree> diff`.
  The trace (`enter` twice) shows where each worktree is.
- Plain text goes to the lead, `@claude-2 text` to one agent. A message to
  an idle worker opens a card; its answer closes it.
- Each session is a long-lived `claude -p` in `--restricted` mode: file
  tools only (no shell, no web), no MCP servers, no user or project
  settings (so no hooks), file access confined to the agent's directory,
  nothing that would ask for permission is allowed. What Claude Code
  reports it runs with is checked on every turn; anything else stops the
  session.
- The budget is split evenly and each share is the session's hard cap, so
  all agents together never spend more than `--budget` dollars.
- Child processes get a cleared environment (plus what Claude Code needs to
  log in), their own process group (killed on exit), and capped pipes.

## Run the simulation

```sh
cargo run -p imece-tui -- --projects 3 --agents 30   # --researchers N, --seed N
cargo run -p imece-tui -- --lang tr                  # or IMECE_LANG=tr; English by default
```

Each project is a group in the salon with its own leader. A folded project
is one line, and that line still shows what waits for you.

| Key | What it does |
| --- | --- |
| `/` | write to the selected project's leader; it splits the request into cards |
| `@name text` | write to one agent |
| `1` `2` `3` | salon · board · code map |
| `↑` `↓` / `j` `k` | select |
| `←` `→` / `h` `l` | fold / unfold a project (`enter` on its header too) |
| `L` | English / Türkçe |
| `enter` / `esc` | look deeper (focus, then the raw stream) / go back |
| `tab` | salon ↔ the items waiting for you |
| `a` / `r` | approve / reject the selected item |
| `space`, `+` `-` | pause, simulation speed (not in `--live`) |
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
