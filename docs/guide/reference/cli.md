---
title: Command line
description: Every flag of imece with its default and range, the environment variables it reads, what a wrong flag says, and what is remembered between runs.
---

# Command line

`imece` takes a handful of flags: the checkout, the caps of the team, the models, the pot, and a few switches for the screen. You meet them once, at the start; most of what they set can also be changed from the screen with a `:` command while agents run ([commands](commands.md)).

## Synopsis

```text
usage: imece [--repo DIR] [--agents N] [--helpers N]
             [--solo claude[:MODEL][@EFFORT]] [--model NAME] [--tier T=NAME]...
             [--budget USD] [--fresh] [--awake] [--confine standard|strict|open]
             [--lang en|tr] [--theme NAME|FILE]
       imece --demo [--projects N] [--agents N] [--researchers N] [--seed N]
       imece doctor [--repo DIR] [--verbose] [--json]
       imece --version
```

```sh
imece                                              # the checkout you are in, the leader alone
imece --agents 8 --budget 10 --model sonnet
imece --demo --projects 3 --agents 30              # fake agents, to see the screen at work
imece --demo --lang tr                             # or IMECE_LANG=tr
imece doctor                                       # what a start needs, and what is wrong
```

## How flags are read

- Every flag but `--demo`, `--fresh` and `--awake` takes one value, the next word. A flag without one is refused: `--agents needs a value`, followed by the usage text.
- Flags may come in any order. `--model` and `--tier` may be repeated; the others take their last value.
- A number is checked against its range; outside it imece says `--helpers: 9 is not in 0-8` (the range of the flag).
- `--projects`, `--researchers` and `--seed` belong to `--demo`. Given without it, imece says `--projects is for --demo` and prints the usage.
- `-h` or `--help` prints the usage text. It leaves with exit code 2, like every other refused command line.
- `--version` (or `-V`) first prints `imece 0.1.0` on stdout and leaves with exit code 0; the installer checks the binary it put in place this way.
- `imece doctor` prints one report of what a start needs and what to do about what is wrong, without the screen, an agent or a cent spent; it takes `--repo DIR`, `--verbose` (`-v`), `--json` and `--lang` only ([imece doctor](../getting-started/doctor.md)).
- `--live` is accepted and ignored: it is what live agents were asked for with before they became the default.
- `imece mcp-relay SOCKET` is not for you: Claude Code starts imece this way as its MCP server, and the process only carries bytes between it and imece's socket for that agent ([message types](message-types.md)).
- `imece stand-in ARGS…` is not for you either: it plays Claude Code for imece's own tests. Run by a `claude` script on PATH that calls `imece stand-in "$@"`, it takes the flags imece gives `claude` (and `--version`, which prints `0.0.0 (imece stand-in)`, and `auth status --json`, which says it is signed in), talks to imece's bus as Claude Code's MCP client does, and streams the same events: a leader hires one worker and assigns it your last paragraph, a worker writes `stand-in/<name>.md` and sends `done`, each turn at a made-up cent; told to, it also says the plan's usage windows from what its sessions spent and compacts a context that fills. Words in brackets in your task make it do a thing when asked, not by chance: `[ask]` runs a command that waits for your yes, `[question]` asks you with answers, `[helper]` hires a helper, `[stray]` changes a file its card does not own, `[cards N]` plans N cards, `[fault]` and `[account]` fail the turn at Claude's API; imece's scripted acceptance cases use them. No model is asked and nothing is spent. imece's end-to-end test runs the real team on it.

## Exit codes

| Code | When | What you see |
| --- | --- | --- |
| 0 | `--version`, `imece doctor` with nothing that does not work (warnings too), or a normal quit (`q q`, `:quit`, or SIGTERM, SIGHUP and SIGINT from outside, which imece turns into a normal quit so sessions stop and the bus's sockets are removed; the same when the terminal itself goes away, its tab or window closed or its tmux server ended) | the shell's title comes back |
| 1 | `imece doctor` found something that does not work (a `[✗]` category). Or the start failed after the flags were read: no coding agent on PATH, `--repo` is not a git checkout, another imece holds the store, the store cannot be read, a session could not start | `imece: <reason>` on stderr, after the screen is restored |
| 2 | A flag is wrong, or `--help` | the message on stderr; for a missing value, an unknown option and a demo flag given live, the usage text follows |

Some messages of exit code 1, quoted: `another imece is running on <path>/imece.sqlite; open the other checkout from it with :open`.

## Remembered between runs, or not

| Remembered (in the store, [files](files.md)) | Per run |
| --- | --- |
| the theme (`--theme`, `T`, `:theme`; a flag or `IMECE_THEME` wins over the saved one) | the language (`--lang`, `IMECE_LANG`, else English; `L` changes it for this run) |
| the pot (`--budget` sets it; without the flag it stays as it was left; $50 the first time) | `--agents`, `--helpers`, `--confine`, `--run`, `--run-timeout` |
| | `--model` and `--tier` |
| how you are told in another window (`:notify`), how much moves on the screen (`:motion`) | `--awake` (by choice: it changes the machine's power behaviour) |
| projects, agents, cards, the journal, mail, the board, pins, marks, the copy list, each agent's session | the usage windows in the header (a session says them again on its first turn) |

## The flags

### `--repo DIR`

```sh
imece --repo ~/src/kule
```

The checkout imece runs on. Default: the current directory. imece looks for the top of the git checkout `DIR` is in; a directory that is not in one is refused at the start (`<DIR>: …`, exit code 1). The checkout becomes a project, led by `lead`; more checkouts open from the screen with `:open PATH [NAME]`.

### `--agents N`

```sh
imece --agents 8        # live: 0-16, default 4
imece --demo --agents 30 # demo: 1-200, default 24
```

Live: the most workers a leader may hire (every project's leader has the same cap). `0` runs the leader alone; it is told so and answers from what it reads. Above 16 imece says `--agents: a leader hires at most 16`. Helpers do not count against it. The demo takes the same flag for the workers next to its leaders; `--demo --agents 0` is refused: `--agents: the demo needs at least 1`.

### `--helpers N`

```sh
imece --helpers 3       # 0-8, default 2
```

The most helpers one worker may have at once. `0` means a worker does everything itself and is told so when it tries to hire.

### `--solo claude[:MODEL][@EFFORT]`

```sh
imece --solo claude                    # Claude Code, its own model and effort
imece --solo claude:opus@xhigh         # opus, thinking at xhigh
```

Starts with a solo session on the checkout instead of a team: one agent you drive yourself, no leader, on model `MODEL` (default: Claude Code's own, or what `--model` names) and effort `EFFORT` (see `--model`). The project is named after the directory, the agent `solo` (`solo-NAME` elsewhere); it writes in its own worktree and every turn that ends with an answer shows you the diff. An open solo on this checkout with the same choice is gone on with, so the same command every morning continues the same session; a different choice opens another solo beside it, named `-2`; a team open here stays, asleep until you write to it. Anything else is refused, in words. The demo takes it too. See [solo sessions](../concepts/solo.md).

### `--model NAME`

```sh
imece --model sonnet
imece --model opus@xhigh                 # Claude's opus, thinking at xhigh
imece --model claude:@max                # Claude's default model at its deepest effort
```

The model agents run on. It is the `standard` tier: the leader runs on it, and so does every worker hired at the standard tier. Default: Claude Code's own default (what you set up in it). `NAME` is `[MODEL][@EFFORT]`, each part optional: a model name as Claude Code takes it (`sonnet`, `claude-opus-5-5`, `opus[1m]`), an effort after `@`, or both. The efforts are Claude Code's own: `low`, `medium`, `high`, `xhigh`, `max` (its `--effort`). The whole is 1-64 characters of letters, digits and `-._[]@`, not starting with `-`; anything else: `--model: "x y" is not a model name`. Any other effort is refused before anything starts; Claude Code itself would only warn and run at its default effort, so imece checks first.

### `--tier T=NAME`

```sh
imece --tier strong=opus[1m]
```

What a tier stands for, `T` being `cheap`, `standard` or `strong`. The leader hires by tier, never by model; the rungs are `haiku`, Claude Code's default and `opus`. This flag replaces one rung; `NAME` is `[MODEL][@EFFORT]` as for `--model`, so `--tier strong=opus@xhigh` makes the strong tier Claude's opus thinking at xhigh. Refusals: `--tier: "strong" is not TIER=MODEL, like strong=opus`, `--tier: "huge" is not cheap, standard or strong`, `--tier: "…" is not a model name`, and the effort refusals of `--model`. A tier model your account lacks fails its first turn; imece then restarts that worker on the standard model and the journal says so ([team](../concepts/team.md)).

### `--budget USD`

```sh
imece --budget 10       # dollars, at most two decimals, at least 0.01
```

The most all agents together may spend: one pot for the whole team, kept between runs. The flag sets it; without the flag the pot stays as the last run left it, $50 the first time. Up to four digits before the point and two after; `--budget 0` and anything that is not a number are refused: `--budget: abc is not dollars like 5 or 2.50`. `:budget USD` changes it while agents run. See [budget and limits](../concepts/budget-and-limits.md).

### `--fresh`

```sh
imece --fresh
```

Forgets every agent's remembered session when the store is taken in: each agent wakes in a new session, told where it left off, and the transcripts of the old ones are removed (see [files](files.md)). The journal says `you made X forget its session; it starts a new one, told where it left off` for every agent that is not gone. `:forget @NAME` does the same for one agent while imece runs. The next session pays the whole briefing again, without the cache. An archived project's sessions are kept: they are what bringing it back goes on with (see [`:archive`](commands.md#archive-title)).

### `--awake`

```sh
imece --awake
```

Keeps the machine from sleeping while any agent is mid-turn: `caffeinate -i -w PID` on macOS, `systemd-inhibit --what=idle:sleep --mode=block sleep infinity` on Linux, nothing elsewhere, as a child in its own process group that goes when nothing is mid-turn or imece ends. Off by default and not remembered; `:awake` turns it on or off while imece runs. A tool that does not start is said once in the flash and not tried again that run. The demo takes the flag too.

### `--confine LEVEL`

```sh
imece --confine standard   # the default
imece --confine strict
imece --confine open
```

How much of Claude Code each agent's session gets:

| Level | Workers and helpers | The leader | Your own Claude Code setup |
| --- | --- | --- | --- |
| `standard` (default) | Claude Code's full coding toolset: read, write, shell, web search and fetch | read and the web | not loaded |
| `strict` | file tools only; commands through imece's `run` tool | read | not loaded |
| `open` | Claude Code as you have it set up, less its sub-agent tool | read, your setup | loaded: settings, hooks, MCP servers, skills |

At every level the leader never writes (it works in your checkout, where a write would skip the Δ review) and Claude Code's own sub-agent tool stays off. imece sandboxes nothing: it switches on Claude Code's own restrictions for the level and checks, fail closed, that the session runs with them; isolation is a container's job. The journal says so at every start that is not strict. Anything else: `--confine: "loose" is not standard, strict or open`. See [security](../concepts/security.md) and [approvals and questions](../concepts/approvals-and-questions.md).

### `--run PROG[,..]`

```sh
imece --run npm,make
imece --run npm --run pnpm       # repeatable; duplicates dropped
```

Programs workers and helpers may run with their `run` tool, next to `cargo`, git's reading commands and the test commands a Python, JavaScript or Go checkout's files call for (`python3 -m pytest`, `npm test`, `go test`: those need no `--run`, see [the tests a checkout calls for](../concepts/approvals-and-questions.md#the-tests-a-checkout-calls-for)). A program named here runs whole, with any arguments, so name one only when its tests' shapes are not enough. Names on PATH only; a path is refused: `--run: "./bin/x" is a path; name a program on PATH`. The leader never gets them. Under `standard` and `open` the same list also decides which shell commands a session may run without asking you (the routine rule, [approvals](../concepts/approvals-and-questions.md)).

### `--run-timeout S`

```sh
imece --run-timeout 1200   # 10-3600 seconds, default 600
```

How long one `run` may take before its process group is killed. The tool's description tells the agent the number; an answer that came back late says `killed after 600 s: it did not finish in time`.

### `--lang en|tr`

```sh
imece --lang tr
IMECE_LANG=tr imece
```

The screen's language. Default English, or `IMECE_LANG`. `L` or `:lang` switches while imece runs; the choice is per run. Anything else: `--lang: "de" is not en or tr` (and `IMECE_LANG: "de" is not en or tr` for the variable). The agents are always spoken to in English.

### `--theme NAME|FILE`

```sh
imece --theme dark
imece --theme nord
imece --theme ~/.config/ghostty/themes/mine
IMECE_THEME=catppuccin-mocha imece
```

`terminal` (the default: your terminal's own colours, asked for at the start and again when the window comes back into focus, so a terminal that turns light or dark is followed), `dark`, `light`, `mono`, one of the built-in schemes `dracula`, `nord`, `gruvbox-dark-medium`, `gruvbox-light-medium`, `catppuccin-mocha`, `catppuccin-latte`, `tokyo-night-dark`, `solarized-dark`, `solarized-light`, `onedark`, `rose-pine`, `everforest`, `kanagawa`, `github`, `monokai`, or a colour file in Ghostty's (`palette = 4=#rrggbb`, `background = #rrggbb`) or Kitty's (`color4 #rrggbb`, `background #rrggbb`) format; a value with a `/` in it, or that is a file, is read as a file. The last choice (`T`, `:theme`) is remembered; a flag or `IMECE_THEME` wins over it. A wrong name: `--theme: "solar": not one of terminal|dark|light|mono|dracula|…|monokai and no such file`; a file that no longer exists at the next start leaves the default silently. See [themes and language](../screen/themes-and-language.md).

### `--demo`

```sh
imece --demo
```

Fake agents instead of real sessions, to see the screen at work: a seeded simulation (`sim.rs`) feeding the same model as live. Nothing is started, nothing is spent, nothing is saved but the screen choices. The demo mirrors live: every feature live has shows in the demo too (and the demo has a few of its own: `space`, `+`, `-`, researchers). See [the demo](../getting-started/demo.md).

### `--projects N`

```sh
imece --demo --projects 3   # 1-4, default 2
```

Demo only: projects worked on at once, each with its own leader.

### `--researchers N`

```sh
imece --demo --researchers 8   # 1-32, default 4
```

Demo only: researchers that may run at once. Researchers are not built live; only the demo shows them.

### `--seed N`

```sh
imece --demo --seed 42   # any whole number, default 7
```

Demo only: the same seed replays the same fake scenario.

## Environment variables

Read by imece itself:

| Variable | What it does |
| --- | --- |
| `IMECE_LANG` | `en` or `tr`, the screen's language when `--lang` is not given; a wrong value is refused like the flag |
| `IMECE_THEME` | a theme name or colour file, as `--theme`; checked the same way |
| `IMECE_PROGRESS` | `on` or `off`: whether the tab's progress bar (OSC 9;4) is sent; without it, to kitty 0.39+, Ghostty 1.2+ and iTerm2 3.6.6+ by the name the terminal gives at start, and inside tmux (`TMUX`); a terminal that does not answer is taken from `WT_SESSION` (Windows Terminal), `TERM_PROGRAM` and `TERM_PROGRAM_VERSION`, or `LC_TERMINAL` and `LC_TERMINAL_VERSION`; see [the team](../screen/salon.md) |
| `TMUX` | inside tmux: notifications go through its passthrough (`allow-passthrough on`), the tab's progress plainly |
| `XDG_STATE_HOME` | where the state directory `imece/` goes (the store, the worktrees); must be absolute; else `~/.local/state/imece`, and without `HOME` the system's temporary directory |
| `HOME` | the fallback above, `~` in a path typed after `:open`, `:export` or `--theme`, and Claude Code's default directory |
| `COLORTERM` | `truecolor` or `24bit` makes imece draw RGB as it is; else the scheme's colours are brought to the nearest of the 256 |
| `COLORFGBG` | a first guess whether the terminal is dark or light, before it is asked |
| `TERM` | `dumb` (or no tty) skips asking the terminal for its colours and the kitty keyboard protocol |
| `SHELL` | what `!cmd` runs in; `/bin/sh` without it |
| `TMPDIR` | where the bus's socket directory `imece-<pid>-<hex>/` is made (0700, removed at exit) |
| `WAYLAND_DISPLAY`, `DISPLAY` | on Linux: whether `wl-copy`, then `xclip`/`xsel`, are tried for `c` `C` `y` `Y` copies (macOS uses `/usr/bin/pbcopy`); when no tool takes the text, the terminal's OSC 52 escape does |
| `CLAUDE_CONFIG_DIR` | where Claude Code keeps its sessions, so imece finds the transcript to resume or remove ([files](files.md)) |

Passed to children: Claude Code, `cp`, `git` and the `run` tool's program start with a cleared environment. They get `HOME`, `USER`, `LOGNAME`, `PATH`, `SHELL`, `TMPDIR`, `LANG`, `LC_ALL`, `LC_CTYPE` when set, plus what Claude Code needs to log in and reach the network: `ANTHROPIC_API_KEY`, `CLAUDE_CONFIG_DIR`, `XDG_CONFIG_HOME`, the proxy variables (`HTTPS_PROXY`, `HTTP_PROXY`, `NO_PROXY` and their lowercase forms), `NODE_EXTRA_CA_CERTS` and `SSL_CERT_FILE`. A `run` also gets `CARGO_HOME`, `RUSTUP_HOME`, `RUSTUP_TOOLCHAIN`, `CARGO_TARGET_DIR`, `CARGO_BUILD_JOBS`, and is given `NO_COLOR=1`, `CARGO_TERM_COLOR=never`, `TERM=dumb`. Nothing else of your environment reaches an agent.

## See also

- [Commands](commands.md): what the same things are called from the screen
- [Files](files.md): the state directory the flags and variables point at
- [Install](../getting-started/install.md) and [first run](../getting-started/first-run.md)
- [Claude Code](../concepts/claude-code.md), [budget and limits](../concepts/budget-and-limits.md), [security](../concepts/security.md)
