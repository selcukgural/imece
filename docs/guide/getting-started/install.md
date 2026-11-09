---
title: Install
description: Install imece with one line (the installer, Homebrew or cargo; or build it from source), put Claude Code on PATH, and know where it keeps its state.
---

# Install

imece runs on macOS and Linux, x86_64 and arm64 (on Windows: inside WSL). It needs git (a solo also runs without it, in a folder that is no checkout) and Claude Code on PATH, logged in. This page is what to have in place before the [first run](first-run.md).

## With the installer

```sh
curl -fsSL https://selcukgural.github.io/imece/install.sh | sh
```

The script downloads the prebuilt imece for your system from the [releases](https://github.com/selcukgural/imece/releases) (the Linux builds are static, so any distribution takes them) and checks it against the release's `SHA256SUMS`: when it does not match, nothing is installed. It puts `imece` in `~/.local/bin` and, when that is not on PATH yet, adds it once to your shell's startup file: `~/.zshrc`, `~/.bashrc` (`~/.bash_profile` on macOS), fish's `conf.d/imece.fish`, else `~/.profile`, under a `# imece` line. Open a new terminal, or run the line it prints, and `imece --version` answers. It also gives imece's tab its mark where the terminal shows an icon per program (see [terminals](#terminals)). On Linux it puts the mark in your icon theme (`~/.local/share/icons/hicolor/scalable/apps/imece.svg`), and imece names it to Konsole for its tab while it runs. On macOS with iTerm2 it does it the way iTerm2 shows Claude Code's: iTerm2 draws the icon of the program in a tab from its own table of programs, and imece is not in the one it ships, so its tab would show a letter in a circle. The script puts the mark (`graphic_imece.png`) and an `imece` entry in iTerm2's own tables (`graphic_icons.json`, `graphic_colors.json` in `~/Library/Application Support/iTerm2`), from iTerm2's next start; the tab icon shows when *Settings → Profiles → General → Icon* is not *No icon*. Entries of yours in those tables stay as they are; a table that already names imece is not touched, and one that is not a JSON object (or is a link) is left alone and said. It ends by saying which coding agents it found on PATH. Running it again updates imece; a running imece keeps its file until it ends.

| Variable | Effect |
| --- | --- |
| `IMECE_VERSION=0.1.0` | that version instead of the latest |
| `IMECE_INSTALL_DIR=DIR` | where imece goes (default `~/.local/bin`) |
| `IMECE_NO_MODIFY_PATH=1` | no startup file is touched |
| `IMECE_NO_ICON=1` | no icon for imece's tab (iTerm2's tables and the icon theme left alone) |
| `IMECE_DOWNLOAD_URL=URL` | where the release files are, for a mirror: an `https://` address, a `file://` directory or a directory's absolute path; anything else is refused |

```sh
curl -fsSL https://selcukgural.github.io/imece/install.sh | IMECE_VERSION=0.1.0 sh
```

To read the script before it runs: `curl -fsSL https://selcukgural.github.io/imece/install.sh -o install.sh`, read it, then `sh install.sh`. Each archive also carries a build provenance attestation; `gh attestation verify imece-aarch64-apple-darwin.tar.gz --repo selcukgural/imece` checks that this repository's release workflow built it from its tag.

To uninstall: `rm ~/.local/bin/imece`, and the lines under `# imece` in the startup file; for the tab icon, on macOS `graphic_imece.png` and the `imece` entries in iTerm2's two tables, on Linux `~/.local/share/icons/hicolor/scalable/apps/imece.svg`. The state directory (below) stays until you remove it.

## With Homebrew

```sh
brew tap selcukgural/imece
brew trust --formula selcukgural/imece/imece
brew install imece
```

The middle line is for Homebrew 7 and newer, which loads a formula of a tap that is not its own only once you have said you trust it; without it `brew install` stops with `Refusing to load formula selcukgural/imece/imece from untrusted tap`. An older Homebrew has no `brew trust` and does not need it: leave the line out.

The [tap](https://github.com/selcukgural/homebrew-imece) installs the same prebuilt imece as the installer, for macOS (arm64, x86_64) and Linux (x86_64, arm64). Its formula is written by a workflow for the latest release: every archive is checked against the release's `SHA256SUMS` and its build provenance attestation, and the formula is installed and tested on macOS and Linux before it is committed. `brew upgrade imece` updates it, `brew uninstall imece` removes it. Homebrew does not set the tab's icon; the installer does.

## From crates.io

```sh
cargo install --locked imece
```

Builds imece from the [crate](https://crates.io/crates/imece) with your own Rust (1.99 or newer) and puts it in `~/.cargo/bin`. `--locked` builds with the dependency versions the release was tested with. The same command again updates it; `cargo uninstall imece` removes it.

Whichever way it came, `imece --version` says which one runs, and [`imece doctor`](doctor.md) what else a start needs. Install one way only: two copies on PATH are two versions sooner or later.

## From source

The repository pins its compiler in `rust-toolchain.toml` (channel `1.99.1`, with `clippy` and `rustfmt`). With [rustup](https://rustup.rs) installed, `cargo` picks that toolchain up on its own the first time you build; a new compiler comes in only by a change to that file.

```sh
git clone https://github.com/selcukgural/imece.git
cd imece
export PATH="$HOME/.cargo/bin:$PATH"
cargo install --locked --path crates/imece-tui   # the crate and its binary are called imece
```

Or run it from the checkout without installing:

```sh
cargo run -p imece -- --help
cargo run -p imece -- --demo
```

An imece put in `~/.cargo/bin` this way and one the installer put in `~/.local/bin` are two copies; the installer says so when the other comes first on PATH.

The crate is `imece` (in `crates/imece-tui`), as its binary is. SQLite is bundled into the build, so no system library is needed for the store. Git must be on PATH: imece reads and applies changes with git itself, and the three-way apply was probed on git 2.56.

## Claude Code

imece looks for `claude` on PATH when it starts and drives it. Without it, it stops and says to install Claude Code (https://claude.com/claude-code) and log in.

It runs one long-lived `claude -p` stream-json session per agent, under `--restricted` unless `--confine open`. It needs a login; costs are reported in dollars and go to the pot. Probed on Claude Code 2.1.314.

Each session is started so that your Claude Code setup does not load (no hooks, no MCP servers but imece's own, no settings, no commands of yours), only the notes below, under the default `--confine standard` and under `strict`; `--confine open` runs Claude Code as you have it set up. In every case the leader never writes and Claude Code's own sub-agent tool stays off. What a session reports it runs with is checked, fail closed; see [Claude Code](../concepts/claude-code.md) and [security](../concepts/security.md).

Every agent reads the repository's `CLAUDE.md` and your `~/.claude/CLAUDE.md` whole, through Claude Code's own loader; a repository with only an `AGENTS.md` has that read instead. Instructions for imece's agents alone go in `~/.config/imece/instructions.md` (or under `$XDG_CONFIG_HOME/imece/`). See [files](../reference/files.md).

## Terminals

imece draws in the alternate screen and reads keys through crossterm; any terminal works. A few things depend on the terminal:

- **`⇧enter` for a new line while writing** needs the kitty keyboard protocol, which imece asks for at start. Terminals that speak it: kitty, Ghostty, WezTerm (with `enable_kitty_keyboard`), foot, Alacritty, iTerm2. Elsewhere `⇧enter` is a plain `enter` and sends; type `\` then `enter` for a new line (it works everywhere), or `⌥enter` where Option is Meta. The `?` help and the footer say which one your terminal has. A terminal multiplexer (tmux, screen) that does not answer the query gets the `\` `enter` fallback even when the terminal around it could do more.
- **Alt keys on macOS** (`⌥b` `⌥f` `⌥d` `⌥backspace` `⌥enter`) need the terminal to send Option as Meta: iTerm2 → Profiles → Keys → Left Option key: *Esc+*; Terminal.app → *Use Option as Meta key*. Without it `⌥b` types `∫`.
- **The `terminal` theme** (the default) uses your terminal's own 16 colours and asks it, before the screen starts, for its background and foreground (OSC 11 / OSC 10) to know dark from light; iTerm2, Ghostty, kitty, WezTerm, Alacritty, foot and tmux answer. A terminal that does not answer is taken as dark, or as `COLORFGBG` says. Without `COLORTERM=truecolor` (or `24bit`) a scheme's RGB colours are rounded to the 256-colour cube. The theme follows your terminal turning light or dark when the window comes back into focus; under tmux that needs `focus-events on`. See [themes and language](../screen/themes-and-language.md).
- **The tab's icon.** A program cannot send a terminal a picture for its tab; where a terminal shows one per program, imece's mark is there: **iTerm2** takes it from its own table, which the installer fills (from iTerm2's next start, with *Settings → Profiles → General → Icon* not *No icon*); **Konsole** (and Yakuake) is told the icon's name (`OSC 32`) while imece runs, and gets the default profile's back after (a profile with an icon of its own is not asked for it, so it gets the default too). **Windows Terminal** takes a tab's icon only from the profile it was opened with. Terminal.app, Ghostty, kitty, WezTerm, GNOME Terminal, Ptyxis, Tilix, xfce4-terminal, Alacritty and foot show no icon per program. Everywhere, the title starts with imece's mark `⁙` while all is calm. `IMECE_TAB_ICON=on` or `off` decides the Konsole icon yourself; never inside tmux.
- **The title** (`imece · 2 to decide`) uses xterm's title stack, so xterm, iTerm2, kitty, WezTerm and Ghostty give your shell's title back when imece ends; a terminal without the stack shows its own default.

A mouse is never needed and never captured, so your terminal's own text selection keeps working.

## Where state lives

Everything imece remembers is yours alone (files are made `0600`) and lives under one directory: `$XDG_STATE_HOME/imece`, or `~/.local/state/imece` when `XDG_STATE_HOME` is not set (an absolute path is required).

| Path | What it is |
| --- | --- |
| `<state>/imece.sqlite` | the store: projects, agents, cards and their stories, the journal, the inbox, mail, the board, the pot, your screen preferences (theme, notify, motion). WAL mode, so `imece.sqlite-wal` and `imece.sqlite-shm` sit next to it while it is open |
| `<state>/imece.sqlite.lock` | held with `flock` while an imece runs; a second imece on the same file is refused and told to use `:open` from the first |
| `<state>/worktrees/<dir>-<hash>/<agent>` | one git worktree per worker (`claude-1`, `claude-2`, …), detached at your HEAD, under a directory per project (the checkout's directory name and an 8-digit hash of its path). Kept between runs. Each has its own `target/` |

Outside that directory, Claude Code keeps what it always keeps: its transcripts under `~/.claude/projects/`. `--fresh` and `:forget` remove those for the sessions imece forgets; see [sessions](../concepts/sessions.md). imece's MCP sockets, one per agent, live in a private directory under the system's temporary directory for the run.

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
