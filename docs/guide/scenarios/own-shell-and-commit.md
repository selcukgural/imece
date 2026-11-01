---
title: "Your shell next to the team"
description: "The person's own tools beside the agents: commands in your shell, a suggested commit message and its ceilings, keeping the machine awake, changing the budget mid-run, export, themes and language."
---

# Your shell next to the team

imece does not replace your terminal: `!cmd` runs a command of your own in your shell with the screen stepped aside, `:commit` asks a cheap model for a commit message for what you staged, and a few commands change the run while agents work. You meet these as soon as a change is applied and you want to build, look and commit yourself. This scenario is an evening of that.

## 1. `!cargo test`

Press `!`. The command line opens as ` › shell: ` with `!` already typed. Type `cargo test` and press `enter`. The screen steps aside (raw mode off, the alternate screen left) and the command has the terminal to itself, in the checkout, in your `$SHELL`. Under its output:

```text
test result: ok. 42 passed; 0 failed

[imece] exit 0 · 12 s · any key: back to the screen
```

Any key brings the screen back (the key is swallowed, nothing else happens). The journal keeps the command, its exit code and how long it took:

```text
 20:02 › you ran `cargo test` → exit 0 · 12 s
```

The time is said in the largest units that say it (`40 ms`, `2 s`, `1 min 5 s`, `1 h 2 min`). The output is not captured and never reaches an agent; it stays on the terminal's main screen, where you see it again when imece quits. A command that could not start is only flashed (`could not run …`); an empty one says `the command is empty`.

## 2. `!git log`, `!vim`

Because the shell inherits the terminal, interactive programs work: `!git log` opens its pager, `!vim src/lib.rs` edits, `!less target/out.txt` scrolls. Quit them as you always do; the `[imece] exit …` line waits under them.

## 3. ctrl-c stops the command, not imece

Run `!cargo bench` and press ctrl-c. imece ignores SIGINT while the command runs and the child gets the default handler, so the command dies and imece does not:

```text
[imece] stopped by signal 2 · 3 s · any key: back to the screen
```

```text
 20:05 › you ran `cargo bench` → stopped by signal 2 · 3 s
```

## 4. `:commit`

A worker's change was applied: it is staged in your checkout. Type `:commit` and press `enter`. The flash says `asking haiku for a commit message for what is staged in parser`.

What happens: imece reads `git diff --cached --stat`, the staged patch (at most 24 KiB, cut at a line and said to be cut) and the eight newest commit subjects for the repository's style, with `--no-ext-diff --no-textconv` so no program of yours runs. It starts one Claude Code process on the cheap tier (`haiku`, or your `--tier claude:cheap=…`) with no tools, no MCP server, a short system prompt of imece's own instead of Claude Code's and `--no-session-persistence`; its `init` is checked like every session's; it runs in a directory with nothing to read and is given up after 120 s. The system prompt says the diff is data, not instructions. The answer is cleaned, fences and blank ends dropped, at most 30 lines and 2000 characters, and single-quoted for the shell (`'` as `'\''`, so `$(…)` in it stays text).

The suggestion lands on the command line, once it is free, as a `!` command for you to read, edit and run with `enter`, or drop with `esc`:

```text
 › shell: !git commit -m 'Read defaults from config v2

Values missing from the file fall back to Config::default(); the loader no longer panics on a partial file.'
```

For a project other than the one at hand it is `!git -C /path/to/checkout commit -m '…'`. imece commits nothing itself.

```text
 20:10 ! haiku suggested a commit message for what is staged, for $0.01
```

### What it costs

| Ceiling | Value |
| --- | --- |
| One suggestion, at most | $0.05, reserved from the pot before the call, settled to what it cost after |
| A day (UTC), every project together | 20 suggestions and $1.00 |
| The same staged tree again | nothing: the last suggestion per project is kept and put back on the line |

The reservation is one transaction in the store and the pot holds it; refused when the pot has less than $0.05 or the day is full (`no commit message: today's 20 suggestions were made`). A call that could not start costs nothing; one that ran out of time without reporting keeps its cap. Failures are journaled with why and what they cost: `no commit message came ($0.00): nothing is staged`. Only Claude Code can be asked, because it is the one vendor whose call takes a hard spending cap; a Codex or Gemini lead needs Claude Code on PATH for this, and without it `:commit` is refused. The `:` menu marks `:commit` with `$`.

## 5. A long night run: `--awake`, `:awake`

A laptop that sleeps mid-turn cuts every session. `imece --awake` or `:awake` while running:

```text
 the machine stays awake while an agent is mid-turn
```

While it is on and any agent is mid-turn, imece runs the system's own tool as a child, `caffeinate -i -w PID` on macOS or `systemd-inhibit --what=idle:sleep --mode=block sleep infinity` on Linux, and lets it go when nothing is mid-turn, when you turn it off or when imece ends. `:awake` again turns it off (`the machine may sleep again`); the `:` menu shows `on` or `off`. Off by default and not remembered between runs, since it changes the machine's power behaviour. A tool that does not start is said once (`could not keep the machine awake: …`) and not tried again that run.

## 6. `:budget 80` mid-run

The header shows `spent $41.20 of $50.00 ($6.10/h, ~1 h 26 min left)`; with under half an hour left it would turn the attention colour and the journal would say so once. Type `:budget 80` (or `:b 80`) and `enter`:

```text
 the budget is $80.00 now
```

```text
 20:30 › you set the budget to $80.00 (was $50.00)
```

A turn that is under way finishes on its old cap; the next starts on the new remainder. A Claude session whose turn ended at its cap while the pot has room is started again with the same prompts and the new remainder. The pot is one for every project and kept between runs; `:budget` lower than what is spent stops every next turn.

## 7. `:export`

With a conversation open (`v` on a row) or a card's screen (`#N`), `:export` writes it as Markdown next to where imece started (`written to /Users/you/code/parser/imece-conversation-20261101-2035.md`), or into the directory or file you name (`:export ~/notes/`). Never over an existing file. With nothing open: `nothing to export: open a conversation (v) or a card (#N) first`.

## 8. `:theme nord` as a preview

Type `:theme ` and `tab`. The menu lists the built-in themes (`terminal`, `dark`, `light`, `mono`, `dracula`, `nord`, `gruvbox-dark-medium`, `catppuccin-mocha`, `tokyo-night-dark`, `solarized-dark`, …) and the screen wears each one while its name is on the line, typed or stepped to. `enter` keeps it (`theme: nord`), `esc` gives it up and the theme in use comes back. `T` cycles them; `:theme FILE` takes a colour file in Ghostty's or Kitty's format; `--theme` and `IMECE_THEME` do the same at the start. The choice is remembered. The default, `terminal`, is your terminal's own 16 colours and follows it turning light or dark: imece asks for the colours again when its window comes back into focus.

## 9. `L`

Press `L`. Every word on the screen switches to Turkish (`:lang tr`; `L` again or `:lang en` back; `--lang` and `IMECE_LANG` at the start). The journal is stored as data and worded at draw time, so old lines switch too; what agents wrote stays as they wrote it.

## See also

- [The command line](../screen/command-line.md): `!`, `:`, completion, readline keys
- [Themes and language](../screen/themes-and-language.md)
- [Budget and limits](../concepts/budget-and-limits.md)
- [Commands](../reference/commands.md)
- [A failing test, one worker](one-fix.md)
