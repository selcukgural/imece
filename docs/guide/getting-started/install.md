---
title: Install
description: Build imece from source, put the vendor CLIs on PATH, and know where it keeps its state.
---

# Install

imece is installed from source with `cargo`; there are no packages yet. It needs git, a Rust toolchain, and at least one vendor's command-line agent on PATH, logged in. This page is what to have in place before the [first run](first-run.md).

## Rust toolchain

The repository pins its compiler in `rust-toolchain.toml` (channel `1.99.1`, with `clippy` and `rustfmt`). With [rustup](https://rustup.rs) installed, `cargo` picks that toolchain up on its own the first time you build; a new compiler comes in only by a change to that file.

```sh
git clone https://github.com/selcukgural/imece.git
cd imece
export PATH="$HOME/.cargo/bin:$PATH"
cargo install --path crates/imece-tui      # the binary is called imece
```

Or run it from the checkout without installing:

```sh
cargo run -p imece-tui -- --help
cargo run -p imece-tui -- --demo
```

The crate is `imece-tui`; its binary is `imece`. SQLite is bundled into the build, so no system library is needed for the store. Git must be on PATH: imece reads and applies changes with git itself, and the three-way apply was probed on git 2.56.

## The vendor CLIs

imece looks for three programs on PATH when it starts (`looking for coding agents on PATH…`) and drives whichever are there: `claude`, `codex` and `gemini`. One is enough. With none, it stops with `no coding agent on PATH (looked for claude, codex, gemini); install Claude Code (https://claude.com/claude-code) and log in`.

| Program | What imece runs | What it must have |
| --- | --- | --- |
| `claude` (Claude Code) | one long-lived `claude -p` stream-json session per agent, under `--restricted` unless `--confine open` | a login; costs are reported in dollars and go to the pot. Probed on Claude Code 2.1.314 |
| `codex` (Codex CLI) | one long-lived `codex app-server` (JSON-RPC on stdio) per agent | a **ChatGPT login** (`codex login`). An API-key login is refused: Codex reports tokens, not dollars, and imece cannot price them yet. On a ChatGPT plan a Codex turn costs the pot nothing; the trace says so and shows the tokens. Probed on Codex 0.165.0 |
| `gemini` (Gemini CLI) | one long-lived `gemini --acp` (Agent Client Protocol) per agent, in a private directory imece owns | an API key or a Google login. With an API key, turns are booked from the list prices imece carries (a model it cannot price stops the session; name one it knows with `--model` or `--tier`). With a Google login they cost the pot nothing. Vertex AI and gateway logins are refused. Probed on Gemini CLI 0.65.0 |

Each vendor's session is started so that nothing of your own vendor setup loads (no hooks, no MCP servers but imece's own, no repository settings) under the default `--confine standard` and under `strict`; `--confine open` runs the vendor as you have it set up. In every case the leader never writes and the vendor's own sub-agent tool stays off. What a session reports it runs with is checked, fail closed; see [vendors](../concepts/vendors.md) and [security](../concepts/security.md).

Every agent also gets your repository's notes for coding agents, its `AGENTS.md` or else `CLAUDE.md`, up to 16 KiB; a longer file goes in cut and the journal says so.

## Who leads

With one vendor on PATH it leads. With more than one, imece asks at the start (the `WHO LEADS?` screen) unless `--lead claude|codex|gemini` says, or you answered the question with `p` once, which makes that vendor the primary from then on. `--lead ask` asks anyway. Details on the [first run](first-run.md) page.

## Terminals

imece draws in the alternate screen and reads keys through crossterm; any terminal works. A few things depend on the terminal:

- **`⇧enter` for a new line while writing** needs the kitty keyboard protocol, which imece asks for at start. Terminals that speak it: kitty, Ghostty, WezTerm (with `enable_kitty_keyboard`), foot, Alacritty, iTerm2. Elsewhere `⇧enter` is a plain `enter` and sends; type `\` then `enter` for a new line (it works everywhere), or `⌥enter` where Option is Meta. The `?` help and the footer say which one your terminal has. A terminal multiplexer (tmux, screen) that does not answer the query gets the `\` `enter` fallback even when the terminal around it could do more.
- **Alt keys on macOS** (`⌥b` `⌥f` `⌥d` `⌥backspace` `⌥enter`) need the terminal to send Option as Meta: iTerm2 → Profiles → Keys → Left Option key: *Esc+*; Terminal.app → *Use Option as Meta key*. Without it `⌥b` types `∫`.
- **The `terminal` theme** (the default) uses your terminal's own 16 colours and asks it, before the screen starts, for its background and foreground (OSC 11 / OSC 10) to know dark from light; iTerm2, Ghostty, kitty, WezTerm, Alacritty, foot and tmux answer. A terminal that does not answer is taken as dark, or as `COLORFGBG` says. Without `COLORTERM=truecolor` (or `24bit`) a scheme's RGB colours are rounded to the 256-colour cube. The theme follows your terminal turning light or dark when the window comes back into focus; under tmux that needs `focus-events on`. See [themes and language](../screen/themes-and-language.md).
- **The title** (`imece · 2 waiting`) uses xterm's title stack, so xterm, iTerm2, kitty, WezTerm and Ghostty give your shell's title back when imece ends; a terminal without the stack shows its own default.

A mouse is never needed and never captured, so your terminal's own text selection keeps working.

## Where state lives

Everything imece remembers is yours alone (files are made `0600`) and lives under one directory: `$XDG_STATE_HOME/imece`, or `~/.local/state/imece` when `XDG_STATE_HOME` is not set (an absolute path is required).

| Path | What it is |
| --- | --- |
| `<state>/imece.sqlite` | the store: projects, agents, cards and their stories, the journal, the inbox, mail, the board, the pot, your screen preferences (side panel width, theme, primary vendor). WAL mode, so `imece.sqlite-wal` and `imece.sqlite-shm` sit next to it while it is open |
| `<state>/imece.sqlite.lock` | held with `flock` while an imece runs; a second imece on the same file is refused and told to use `:open` from the first |
| `<state>/worktrees/<dir>-<hash>/<agent>` | one git worktree per worker (`claude-1`, `codex-2`, …), detached at your HEAD, under a directory per project (the checkout's directory name and an 8-digit hash of its path). Kept between runs. Each has its own `target/` |
| `<state>/worktrees/<dir>-<hash>-gemini/<agent>` | the private directory a Gemini session runs in (its policy file and `GEMINI.md`); a leader's is `lead` whatever the project is called |

Outside that directory, the vendors keep what they always keep: Claude Code its transcripts under `~/.claude/projects/`, Codex its thread rollouts under `~/.codex`, Gemini its chat files under `~/.gemini/tmp`. `--fresh` and `:forget` remove those for the sessions imece forgets; see [sessions](../concepts/sessions.md). imece's MCP sockets, one per agent, live in a private directory under the system's temporary directory for the run.

Your checkout itself gets nothing but the changes you apply, staged, and a private index file under its `.git` while a three-way apply is checked.

## `--awake`

`--awake` (or `:awake` while imece runs) keeps the machine from sleeping while an agent is mid-turn. It runs the system's own tool as a child of imece: `caffeinate -i -w PID` on macOS (part of the OS) and `systemd-inhibit --what=idle:sleep --mode=block sleep infinity` on Linux (needs systemd). Elsewhere there is nothing to run. It is off by default and not remembered between runs, since it changes the machine's power behaviour. A program that does not start is said once in the flash (`could not keep the machine awake: …`) and not tried again that run.

## Environment variables

| Variable | Effect |
| --- | --- |
| `IMECE_LANG=en\|tr` | the screen's language (`--lang` wins; `L` switches while running) |
| `IMECE_THEME=NAME\|FILE` | the theme (`--theme` wins; the last choice is remembered in the store) |
| `XDG_STATE_HOME` | where the state directory goes |
| `COLORTERM` | `truecolor` or `24bit` for RGB colours |

## See also

- [First run](first-run.md)
- [The demo](demo.md)
- [Command line reference](../reference/cli.md)
- [Files](../reference/files.md)
- [Troubleshooting](../troubleshooting.md)
