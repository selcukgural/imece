---
title: Approvals and questions
description: What an agent must ask you before doing, how a routine command runs on its own, how a ⚑ ask and a ? question wait in TO DECIDE, and the run tool with its allow list.
---

# Approvals and questions

Two kinds of things stop an agent until you answer: an **approval** (`⚑`), when Claude Code would ask before a tool call and imece hands the question to you, and a **question** (`?`), when the agent itself asks you something with `send(to: "you", type: ask)`. Both wait in TO DECIDE; `a` and `r` answer the first, writing back answers the second.

## Approvals (`⚑`)

Under `--confine standard` and `open`, what a session would ask before doing goes to you instead of being refused. Under `strict` nothing asks: what would ask is refused, and commands go through imece's `run` tool.

Claude Code runs `--permission-prompts host --permission-prompt-tool mcp__imece__approve`: it calls imece's `approve` tool itself before a tool call that needs a yes (its own read-only rules decide what never asks). The tool is hidden from the model.

### Routine commands run at once

A shell command is **routine** when it is one line, one program with its arguments (nothing a shell would read otherwise: no pipes, redirections, `&&`, `$`, globs), and the `run` tool's policy would let the role run it (below). A routine command is allowed on its own, with a trace line "allowed on its own"; the build, the tests and git's reading commands never wait for you. That holds for a Python, JavaScript or Go checkout's own tests too ([below](#the-tests-a-checkout-calls-for)): `python3 -m unittest -v` runs at once. A routine command may have its output cut at its end, to its last or first lines: `cargo test 2>&1 | tail -20`, `python3 -m unittest | head -n 5` and a bare `| tail` run at once as well, since the cut only shortens what the agent reads; `tail -f`, `tail -c`, a second pipe or anything after the count still ask.

### Everything else waits

Anything else waits in TO DECIDE, under **permissions** on Home: one row per ask, led by the card its agent works on, saying the whole ask in one place, `run Bash: curl … | head?`, wrapped under its column, its lines joined with ` ↵ ` and never read as markdown: what you allow is what runs. The row at hand ends with its keys, and the status line says them in full:

```text
 permissions                                                                                           one key each
▸! #6   Bulk e-mail queue         run WebFetch:                                                    a  yes   r  no
                                  https://learn.microsoft.com/dotnet/api/system.text.json?
 …
 home · shop-api · #6 Bulk e-mail queue    a  allow once   r  refuse   e  yes this turn   ?  help
```

While its ask waits, the agent is that row: it is not listed again under AT WORK. A yes or a no settles the row and puts the next one at hand, so a run of permissions is a run of keys.

- `a` allows; the call runs as asked and the journal says `you allowed claude-1: Bash: …`.
- `r` refuses; the agent is told "The person refused this. Do not try it another way; if you need it, ask them (send to you, type ask) and go on with what does not depend on it." and the journal says `you refused …`. A refusal is a decision, not an error; the turn goes on.
- A call waits up to a day. One call at a time per connection, so a waiting ask holds that session's other imece tools (it could not use them mid-ask anyway).
- When the session ends, every pending ask is answered "the session ended" and its items close. An ask the store kept from an earlier run is closed at the start: the process that asked is gone.

What an ask shows: for a shell command the command; for another tool what it touches (the path, address, query or pattern), or its input as JSON cut at 200 characters. A yes holds once; for a yes from now on, see below.

### A yes from now on

`A` on an ask says yes from now on to its commands, for every agent of that project: `npm run build` gives `npm run …` (the program and its first argument), `make` gives `make`. A Python interpreter's `-m` is read with its module: `python3 -m pip install x` gives `python3 -m pip …`, which holds for that module with any words after it and for no other module, nor for `python3` itself (`python3 -c …`, `python3 x.py` still ask). The ask goes on, so does every other open ask of the project it now covers, and a later ask that only runs such commands (and routine ones) goes on without asking you; its trace says `allowed by your yes from now on: Bash: …`. A line of several commands (`&&`, `||`, `;`, `|`) goes on when each is routine or has its yes. After a yes's two words a line may hold `$NAME`, a glob or a `$(…)`, and what a `$(…)` runs needs its own yes: `A` on `gh pr view "$(git branch --show-current)"` gives `gh pr …` and `git branch …`. `2>&1` and `>/dev/null` change nothing. Those words never count for a routine command or for git, whose denied words a shell could hide (`cargo $X`, `git grep $X`). A line no such yes can hold (an option first as in `rm -rf target`, a redirection to a file, a backtick) gets a yes to that line alone, shown `` `rm -rf target` ``: only those bytes go on, `rm -rf /` still asks. A file edit or a fetch can have none: `A` says why and `a` allows it once. The list is kept with the project between runs; `:allow` shows it, `:allow npm run` adds one by name (before you step away), and `:allow npm run` again takes it out; `:allow rm -rf target` takes out the yes to that line. The journal says `shop-api: you say yes from now on to npm run …`.

### A yes for this turn

`e` on an ask (in TO DECIDE, or on its agent's log tab), or `:yes [@NAME]`, says yes to everything that agent asks until its turn ends: the ask goes on, so do its other open ones, and every later ask of that agent goes on without asking you until its turn (or its session) ends, whatever it is: a command with a `$(…)` or an option first, an edit, a fetch. It is for one agent at work on something you started, when the asks are many and you are watching; a yes from now on (`A`) is for the commands a project always runs. Its trace says `allowed by your yes for this turn: …`, its row and focus say `✓ turn` while it holds, and the journal says when you gave it and, at the turn's end, how many asks it let go on. Another agent's asks still wait for you. A new turn asks again.

## Questions (`?`)

An agent asks you with `send(to: "you", type: "ask", body)`, and may offer the answers to pick from (`options`: at most four, each a short line, the one it would take first). It becomes a `?` item under **questions**, "claude-2 asks you: keep the old CLI flag?" in the journal, and the agent is that row while the question is open. A newer question from the same agent replaces the older.

On [Home](../screen/home.md) the row says the question and, under it, each answer offered with the key that picks it:

```text
▸! #12 Refunds     Should a refund go back to the card or to store credit?   1-3 pick   a answer
                    1  back to the card
                    2  store credit
                    3  whichever it was paid with
```

- `1`-`9` on that row sends the answer it names to the agent as your message: the question closes and the agent's next turn starts with it. The digits pick an answer on Home and on the question's detail; anywhere else they do nothing (the views they once opened are the `:board`, `:code` and `:timeline` windows).
- `/` on the row, or `a` or `r`, open the composer `@claude-2 ` for an answer in your own words; anything you write to that agent closes the question and starts its next turn.
- `enter` opens the conversation at the question. The preview beside the list (from 120 columns) reads the question whole, its answers, and what the agent did so far: its status, its to-do list and its newest trace lines.
- A question of an agent that is gone or stopped stays open until you write to it.

Who asks when: the leader may ask you mid-task; a worker asks its leader first and sends you only what only you can decide; a helper only what no teammate can answer. Questions cost nothing against the mail caps or the pot ([messages](../concepts/messages-and-board.md)).

## Getting to what waits

- `W` from anywhere opens the next item waiting for you (a change's diff, a question's conversation, an approval's agent on its log tab), so `a`/`r` answer it at once; again goes on to the next, round to the first. Nothing waiting: "Nothing needs you right now".
- `S` puts the item at hand off and opens the next: put-off items come after all the others, in the order they were put off, their rows marked `later ·`. Putting off is for this run only.
- Home puts what waits for you above, in TO DECIDE, and sorts each project's workers in AT WORK by what they need of you: stuck (blocked or stopped) first, then at work, then the quiet ones, equals keeping their order. The AT WORK heading (`stuck 2 · idle 1`), the header (`! 2 to decide`) and the terminal's title count the same.

## The `run` tool

Every agent has a `run` tool on the bus: one program and its arguments, no shell, in the agent's own directory (the checkout for the leader, the worktree for workers and helpers).

| | Allowed |
| --- | --- |
| `cargo` | every command except the ones that change dependencies or reach outside the project: `add`, `remove`, `rm`, `install`, `uninstall`, `publish`, `login`, `logout`, `owner`, `yank`. A leader also without the ones that change files (`fmt` without `--check`, `fix`, `clean`, `init`, `new`). |
| `git` | reading commands only: `blame`, `diff`, `grep`, `log`, `ls-files`, `rev-parse`, `show`, `status`. No option before the command (`-c`, `-C`, `--git-dir`), none that starts a pager or writes a file (`-O…`, `--open-files-in-pager…`, `--output…`), also when short options go together (`-iO…`) or a long name is cut short (`--open=…`). |
| the checkout's tests | the test commands its files call for ([below](#the-tests-a-checkout-calls-for)), for every role, as `cargo test` is. |
| `--run PROG[,..]` | the programs you name (`--run npm,make`), for workers and helpers only; repeatable. |
| anything else | refused, with the list of what is allowed and how you add to it. |

- The program is named alone, on PATH, never as a path. The command line is at most 1000 characters; `| & ; < > ( ) $ \` # * ? ~ {` are refused.
- A run is killed after `--run-timeout` seconds (10-3600, default 600), and with imece. The answer waits for the program, not the screen's loop.
- Output comes back tidied: colours stripped, lines of passing tests dropped, repeats collapsed, the middle cut past 48 KiB. Each worktree builds in its own `target/`: a shared one would make the workers rebuild each other's changes in turn.
- `run` has no network block of its own; cargo may reach the registry when the lockfile is off.

The same policy decides which shell commands are routine, so under `standard` a worker's `cargo test` runs whether it goes through `run` or Claude Code's shell; only a `run` ties its result to the tree it ran on and shows on the change ([changes](../concepts/changes.md)).

### The tests a checkout calls for

imece reads which tests a project has from the files of your checkout, as git lists them with its files (on its own thread, at the project's opening and again while agents work), and runs those test commands as it runs `cargo test`: routine, through `run`, so a run counts on the change's verdict. Only these shapes, never the program as a whole:

| The checkout has | Runs at once |
| --- | --- |
| `pyproject.toml`, `setup.py`, `setup.cfg`, `tox.ini`, `pytest.ini` or a `requirements*.txt` at its top, or a `test_*.py` / `*_test.py` anywhere | `python3 -m pytest …`, `python3 -m unittest …` (or with `python`), `pytest …` |
| `package.json` at its top | `npm test`, `npm run test`, each with words for the test script after `--`; `pnpm` for a `pnpm-lock.yaml`, `yarn` for a `yarn.lock` instead of `npm` |
| `go.mod` at its top | `go test …` |

What comes after the shape is checked too: pytest and unittest take only the options that choose, show or stop tests (`-k`, `-m`, `-x`, `-v`, `-q`, `-s`, `--lf`, `--tb`, `--maxfail`, `-n`, `discover -s -p -t`, …), and `go test` only `-run`, `-count`, `-race`, `-v`, `-timeout`, `-short`, `-cover`, `-json`, `-tags` and the like, with packages as `./…`. An option that loads a plugin, reads options or arguments from a file, writes a file or runs another program is refused (`pytest -p`, `-c`, `-o`, `@args`, `--basetemp`, `--junitxml`; `go test -exec`, `-o`, `-toolexec`, `-coverprofile`, `-args`), and so is anything of the package manager's own before `--` (`npm test --prefix …`). No word may name a path outside the agent's directory: `/…`, `~…` or a `..` part, also after `=`. `python3 -c …`, `python3 script.py`, `npm run build`, `node x.js`, `go build` still ask; a yes from now on (`A`) holds them.

The journal says what a project's files call for when imece first reads it and whenever it changes (`web's files call for these tests, which now run without asking you, through the run tool too: npm test, npm run test`), `whoami` names them for an agent, and each agent's instructions name them, so it runs them as one program instead of through a pipe. These tests run the repository's code (a `conftest.py`, a test script in `package.json`), as `cargo test` runs a `build.rs`: an agent that can write the repository can already make them do anything, and the yes that counts stays yours, on the change. The commands are read from your checkout, so a worker's own new `package.json` unlocks nothing until you apply it. A folder without git reads none.

## See also

- [Claude Code](../concepts/claude-code.md): the confine levels.
- [Home](../screen/home.md): TO DECIDE, its groups, `W`, `S`.
- [Messages and the board](../concepts/messages-and-board.md).
- [Security](../concepts/security.md): you are the last gate, not a tool list.
