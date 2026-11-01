---
title: Command line
description: Every flag of imece with its default and range, the vendor prefix rule, the environment variables it reads, what a wrong flag says, and what is remembered between runs.
---

# Command line

`imece` takes a handful of flags: the checkout, the caps of the team, who leads, the models, the pot, and a few switches for the screen. You meet them once, at the start; most of what they set can also be changed from the screen with a `:` command while agents run ([commands](commands.md)).

## Synopsis

```text
usage: imece [--repo DIR] [--agents N] [--helpers N] [--lead VENDOR]
             [--solo V[:MODEL][@EFFORT]] [--model [V:]NAME]... [--tier [V:]T=NAME]...
             [--budget USD] [--fresh] [--awake] [--confine standard|strict|open]
             [--lang en|tr] [--theme NAME|FILE]
       imece --demo [--projects N] [--agents N] [--researchers N] [--seed N]
```

```sh
imece                                              # the checkout you are in, the leader alone
imece --agents 8 --budget 10 --model sonnet
imece --lead codex                                 # Codex leads; it may still hire claude workers
imece --lead gemini --tier gemini:strong=gemini-2.5-pro
imece --demo --projects 3 --agents 30              # fake agents, to see the screen at work
imece --demo --lang tr                             # or IMECE_LANG=tr
```

## How flags are read

- Every flag but `--demo`, `--fresh` and `--awake` takes one value, the next word. A flag without one is refused: `--agents needs a value`, followed by the usage text.
- Flags may come in any order. `--model` and `--tier` may be repeated; the others take their last value.
- A number is checked against its range; outside it imece says `--helpers: 9 is not in 0-8` (the range of the flag).
- `--projects`, `--researchers` and `--seed` belong to `--demo`. Given without it, imece says `--projects is for --demo` and prints the usage.
- `-h` or `--help` prints the usage text. It leaves with exit code 2, like every other refused command line.
- `--live` is accepted and ignored: it is what live agents were asked for with before they became the default.
- `imece mcp-relay SOCKET` is not for you: a vendor's CLI starts imece this way as its MCP server, and the process only carries bytes between the CLI and imece's socket for that agent ([message types](message-types.md)).

### The `[V:]` vendor prefix

`--model` and `--tier` take an optional vendor in front of their value, separated by a colon: `--model codex:gpt-5.6-terra`, `--tier gemini:strong=gemini-2.5-pro`. A model name never holds a colon, so a colon always means a vendor. Without the prefix both speak of the leader's vendor. The vendor must be one imece drives (`claude`, `codex`, `gemini`); anything else is refused: `--tier: "cursor" is not a vendor imece drives (claude, codex, gemini)`.

## Exit codes

| Code | When | What you see |
| --- | --- | --- |
| 0 | A normal quit (`q q`, `:quit`, or SIGTERM, SIGHUP and SIGINT from outside, which imece turns into a normal quit so sessions stop and the bus's sockets are removed) | the shell's title comes back |
| 1 | The start failed after the flags were read: no coding agent on PATH, `--repo` is not a git checkout, another imece holds the store, the store cannot be read, a vendor's session could not start | `imece: <reason>` on stderr, after the screen is restored |
| 2 | A flag is wrong, or `--help` | the message on stderr; for a missing value, an unknown option and a demo flag given live, the usage text follows |

Some messages of exit code 1, quoted: `no coding agent on PATH (looked for claude, codex, gemini); install Claude Code (https://claude.com/claude-code) and log in`; `codex is not on PATH (found claude); install it, or pick the leader with --lead`; `another imece is running on <path>/imece.sqlite; open the other checkout from it with :open`.

## Remembered between runs, or not

| Remembered (in the store, [files](files.md)) | Per run |
| --- | --- |
| the theme (`--theme`, `T`, `:theme`; a flag or `IMECE_THEME` wins over the saved one) | the language (`--lang`, `IMECE_LANG`, else English; `L` changes it for this run) |
| the pot (`--budget` sets it; without the flag it stays as it was left; $50 the first time) | `--agents`, `--helpers`, `--confine`, `--run`, `--run-timeout` |
| the primary vendor (`p` on the question screen, `:lead`) | `--model` and `--tier` |
| the side panel's width (`,` `.` `<` `>`), how you are told in another window (`:notify`) | `--awake` (by choice: it changes the machine's power behaviour) |
| projects, agents, cards, the journal, mail, the board, pins, marks, the copy list, each agent's vendor session | the vendors' usage windows in the header (a session says them again on its first turn) |

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

### `--lead VENDOR`

```sh
imece --lead codex      # claude, codex, gemini or ask
```

The leader's coding agent. Without it: your primary vendor (set with `p` on the question screen, or with `:lead VENDOR`) when it is on PATH and imece drives it; else the only such vendor on PATH; else the question WHO LEADS? before the salon. `--lead ask` forces the question. A vendor named here that is not on PATH gives the honest error when the team is built, not a silent replacement. Anything else: `--lead: "cursor" is not claude, codex, gemini or ask`. See [vendors](../concepts/vendors.md).

### `--solo V[:MODEL][@EFFORT]`

```sh
imece --solo claude                    # Claude Code, its own model and effort
imece --solo claude:opus@xhigh         # opus, thinking at xhigh
imece --solo codex:gpt-5.6-terra@high
```

Starts with a solo session on the checkout instead of a team: one agent you drive yourself, no leader, on vendor `V` (`claude`, `codex`, `gemini`), model `MODEL` (default: the vendor's own, or what `--model` names) and effort `EFFORT` (the vendor's own levels, see `--model`; Gemini has none). The project is named after the directory, the agent `solo` (`solo-NAME` elsewhere); it writes in its own worktree and every turn that ends with an answer shows you the diff. An open solo on this checkout with the same vendor and choice is gone on with, so the same command every morning continues the same session; a different choice opens another solo beside it, named `-2`; a team open here stays, asleep until you write to it. Without `--lead`, `V` leads the teams `:open` adds, so no WHO LEADS? question comes. Refused: `--solo: "x" is not VENDOR[:MODEL][@EFFORT], like claude:opus@xhigh`, `--solo: imece does not drive qwen`, `--solo: claude has no effort "bogus"; …`, and at the start `--solo: codex is not on PATH here, or imece does not drive it` (exit code 1). The demo takes it too. See [solo sessions](../concepts/solo.md).

### `--model [V:]NAME`

```sh
imece --model sonnet
imece --model opus@xhigh                 # Claude's opus, thinking at xhigh
imece --model claude:@max                # Claude's default model at its deepest effort
imece --model codex:gpt-5.6-terra@high
```

The model agents of vendor `V` run on; without `V`, the leader's vendor. It is the `standard` tier: the leader runs on it, and so does every worker hired at the standard tier. Default: the vendor's own default (what you set up in Claude Code, Codex or Gemini CLI). `NAME` is `[MODEL][@EFFORT]`, each part optional: a model name as the vendor takes it (`sonnet`, `claude-opus-5-5`, `opus[1m]`, `gpt-5.6-terra`), an effort after `@`, or both. The efforts are the vendor's own: Claude Code's `low`, `medium`, `high`, `xhigh`, `max` (its `--effort`); Codex's `low`, `medium`, `high`, `xhigh` (the reasoning effort of every turn); Gemini CLI has none. The whole is 1-64 characters of letters, digits and `-._[]@`, not starting with `-`; anything else: `--model: "x y" is not a model name`. An effort the vendor lacks is refused on the command line when the vendor is named (`--model: claude has no effort "bogus"; one of low, medium, high, xhigh, max`, `--model: gemini has no effort setting; name only a model, not @high`) and, for a bare value, when the team is built and the leader's vendor is known (`--model/--tier "opus@bogus": …`, exit code 1); Claude Code itself would only warn and run at its default effort, so imece checks first. May be repeated, once per vendor.

### `--tier [V:]T=NAME`

```sh
imece --tier strong=opus[1m]
imece --tier codex:strong=gpt-5.6-terra@xhigh
imece --tier gemini:cheap=gemini-2.5-flash-lite
```

What a tier stands for, `T` being `cheap`, `standard` or `strong`. The leader hires by tier, never by model; each vendor has its own ladder (Claude: `haiku`, the default, `opus`; Codex: `@low`, the default, `@high`, an effort on its default model; Gemini: `flash`, the default `auto`, `pro`). This flag replaces one rung; `NAME` is `[MODEL][@EFFORT]` as for `--model`, so `--tier strong=opus@xhigh` makes the strong tier Claude's opus thinking at xhigh. Refusals: `--tier: "strong" is not TIER=MODEL, like strong=opus`, `--tier: "huge" is not cheap, standard or strong`, `--tier: "…" is not a model name`, and the effort refusals of `--model`. A tier model your account lacks fails its first turn; imece then restarts that worker on the standard model and the journal says so ([team](../concepts/team.md)).

### `--budget USD`

```sh
imece --budget 10       # dollars, at most two decimals, at least 0.01
```

The most all agents together may spend: one pot for the whole team, kept between runs. The flag sets it; without the flag the pot stays as the last run left it, $50 the first time. Up to four digits before the point and two after; `--budget 0` and anything that is not a number are refused: `--budget: abc is not dollars like 5 or 2.50`. `:budget USD` changes it while agents run. See [budget and limits](../concepts/budget-and-limits.md).

### `--fresh`

```sh
imece --fresh
```

Forgets every agent's remembered vendor session when the store is taken in: each agent wakes in a new session, told where it left off, and what the vendors kept of the old ones is removed (Claude's transcript, Codex's thread, Gemini's chat files; see [files](files.md)). The journal says `you made X forget its session; it starts a new one, told where it left off` for every agent that is not gone. `:forget @NAME` does the same for one agent while imece runs. The next session pays the whole briefing again, without the cache.

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

How much of its vendor each agent's session gets:

| Level | Workers and helpers | The leader | Your own vendor setup |
| --- | --- | --- | --- |
| `standard` (default) | the vendor's full coding toolset: read, write, shell, web search and fetch | read and the web | not loaded |
| `strict` | file tools only; commands through imece's `run` tool | read | not loaded |
| `open` | the vendor as you have it set up, less its sub-agent tool | read, your setup | loaded: settings, hooks, MCP servers, skills, extensions |

At every level the leader never writes (it works in your checkout, where a write would skip the Δ review) and the vendor's own sub-agent tool stays off. imece sandboxes nothing: it switches on each vendor's own restrictions for the level and checks, fail closed, that the session runs with them; isolation is a container's job. The journal says so at every start that is not strict. Anything else: `--confine: "loose" is not standard, strict or open`. See [security](../concepts/security.md) and [approvals and questions](../concepts/approvals-and-questions.md).

### `--run PROG[,..]`

```sh
imece --run npm,make
imece --run npm --run pnpm       # repeatable; duplicates dropped
```

Programs workers and helpers may run with their `run` tool, next to `cargo` and git's reading commands. Names on PATH only; a path is refused: `--run: "./bin/x" is a path; name a program on PATH`. The leader never gets them. Under `standard` and `open` the same list also decides which shell commands a Claude, Codex or Gemini session may run without asking you (the routine rule, [approvals](../concepts/approvals-and-questions.md)).

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
| `XDG_STATE_HOME` | where the state directory `imece/` goes (the store, the worktrees); must be absolute; else `~/.local/state/imece`, and without `HOME` the system's temporary directory |
| `HOME` | the fallback above, `~` in a path typed after `:open`, `:export` or `--theme`, and the vendors' default directories |
| `COLORTERM` | `truecolor` or `24bit` makes imece draw RGB as it is; else the scheme's colours are brought to the nearest of the 256 |
| `COLORFGBG` | a first guess whether the terminal is dark or light, before it is asked |
| `TERM` | `dumb` (or no tty) skips asking the terminal for its colours and the kitty keyboard protocol |
| `SHELL` | what `!cmd` runs in; `/bin/sh` without it |
| `TMPDIR` | where the bus's socket directory `imece-<pid>-<hex>/` is made (0700, removed at exit) |
| `WAYLAND_DISPLAY`, `DISPLAY` | on Linux: whether `wl-copy`, then `xclip`/`xsel`, are tried for `c` `C` `y` `Y` copies (macOS uses `/usr/bin/pbcopy`); when no tool takes the text, the terminal's OSC 52 escape does |
| `CLAUDE_CONFIG_DIR`, `CODEX_HOME`, `GEMINI_CLI_HOME` | where the vendors keep their sessions, so imece finds the transcript to resume or remove ([files](files.md)) |

Passed to children: every vendor CLI, `cp`, `git` and the `run` tool's program start with a cleared environment. They get `HOME`, `USER`, `LOGNAME`, `PATH`, `SHELL`, `TMPDIR`, `LANG`, `LC_ALL`, `LC_CTYPE` when set, plus what the vendor needs to log in and reach the network: Claude `ANTHROPIC_API_KEY`, `CLAUDE_CONFIG_DIR`, `XDG_CONFIG_HOME`; Codex `CODEX_HOME`, `OPENAI_API_KEY`, `CODEX_API_KEY`, `SSL_CERT_DIR`; Gemini `GEMINI_API_KEY`, `GOOGLE_API_KEY`, `GOOGLE_CLOUD_PROJECT`, `GOOGLE_CLOUD_LOCATION`, `GEMINI_CLI_HOME`; all three the proxy variables (`HTTPS_PROXY`, `HTTP_PROXY`, `NO_PROXY` and their lowercase forms), `NODE_EXTRA_CA_CERTS` (Claude, Gemini) and `SSL_CERT_FILE`. A `run` also gets `CARGO_HOME`, `RUSTUP_HOME`, `RUSTUP_TOOLCHAIN`, `CARGO_TARGET_DIR`, `CARGO_BUILD_JOBS`, and is given `NO_COLOR=1`, `CARGO_TERM_COLOR=never`, `TERM=dumb`. Gemini sessions are given `GEMINI_CLI_TRUST_WORKSPACE=true`, `GEMINI_TELEMETRY_ENABLED=false`, `NO_BROWSER=true`. Nothing else of your environment reaches an agent.

## See also

- [Commands](commands.md): what the same things are called from the screen
- [Files](files.md): the state directory the flags and variables point at
- [Install](../getting-started/install.md) and [first run](../getting-started/first-run.md)
- [Vendors](../concepts/vendors.md), [budget and limits](../concepts/budget-and-limits.md), [security](../concepts/security.md)
