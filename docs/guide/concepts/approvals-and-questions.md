---
title: Approvals and questions
description: What an agent must ask you before doing, how a routine command runs on its own, how a ⚑ ask and a ? question wait in WAITING FOR YOU, and the run tool with its allow list.
---

# Approvals and questions

Two kinds of things stop an agent until you answer: an **approval** (`⚑`), when its vendor would ask before a tool call and imece hands the question to you, and a **question** (`?`), when the agent itself asks you something with `send(to: "you", type: ask)`. Both wait in WAITING FOR YOU; `a` and `r` answer the first, writing back answers the second.

## Approvals (`⚑`)

Under `--confine standard` and `open`, what a session would ask before doing goes to you instead of being refused. Under `strict` nothing asks: what would ask is refused, and commands go through imece's `run` tool.

| Vendor | How the ask reaches imece |
| --- | --- |
| Claude Code | `--permission-prompts host --permission-prompt-tool mcp__imece__approve`: Claude Code calls imece's `approve` tool itself before a tool call that needs a yes (its own read-only rules decide what never asks). The tool is hidden from the model. |
| Codex | writers (workers, helpers) run `approval_policy = "on-request"`: what needs more than the sandbox gives (a write outside the directory, a command outside the sandbox) comes as a request. A leader, and everything under strict, stays `never`: what the sandbox refuses simply fails. |
| Gemini CLI | the policy file makes a writer's `run_shell_command` ask (`ask_user`) before every command; the other tools stay allowed without a prompt. Under open your own policies decide. |

### Routine commands run at once

A shell command is **routine** when it is one line, one program with its arguments (nothing a shell would read otherwise: no pipes, redirections, `&&`, `$`, globs), and the `run` tool's policy would let the role run it (below). A routine command is allowed on its own, with a trace line "allowed on its own"; the build, the tests and git's reading commands never wait for you. For Codex the shell it wraps a command in (`/bin/zsh -lc '…'`) is taken off first and the inner command judged as it is, so `&&` inside is not routine. For Gemini only a command that runs in the agent's directory may be routine.

### Everything else waits

Anything else is a `⚑` item in WAITING FOR YOU: "claude-1 waits for your yes: Bash: curl … | head". Each ask is an item of its own, and the selected one is shown **whole** under its row, its lines joined with ` ↵ ` and never read as markdown: what you allow is what runs. The agent's salon line reads `⚑ waits for your yes`.

```text
 WAITING FOR YOU (1)
 ⚑ 14:10 claude-1 waits for your yes: Bash: curl -s https://example.org/schema.json | head -40
    curl -s https://example.org/schema.json | head -40
  a allow · r refuse · enter focus
```

- `a` allows; the call runs as asked and the journal says `you allowed claude-1: Bash: …`.
- `r` refuses; the agent is told "The person refused this. Do not try it another way; if you need it, ask them (send to you, type ask) and go on with what does not depend on it." and the journal says `you refused …`. A refusal is a decision, not an error; the turn goes on.
- A call waits up to a day. One call at a time per connection, so a waiting ask holds that session's other imece tools (it could not use them mid-ask anyway).
- When the session ends, every pending ask is answered "the session ended" and its items close. An ask the store kept from an earlier run is closed at the start: the process that asked is gone. Codex and Gemini withdraw an ask when the turn ends; the item closes without a decision.

What an ask shows: for a shell command the command; for another tool what it touches (the path, address, query or pattern), or its input as JSON cut at 200 characters. Codex file changes show the files the item named and `writes under <root>` for a granted root. There is no "allow for the session": every non-routine ask waits.

## Questions (`?`)

An agent asks you with `send(to: "you", type: "ask", body)`. It becomes a `?` item, "codex-1 asks you: keep the old CLI flag?", and the agent's salon line reads `? → you` while it is idle with the question open. A newer question from the same agent replaces the older.

- `enter` opens the conversation at the question.
- `a` or `r` open the composer `@codex-1 ` (there is no "answered without words"); anything you write to that agent closes the question and starts its next turn.
- A question of an agent that is gone or stopped stays open until you write to it.

Who asks when: the leader may ask you mid-task; a worker asks its leader first and sends you only what only you can decide; a helper only what no teammate can answer. Questions cost nothing against the mail caps or the pot ([messages](../concepts/messages-and-board.md)).

## Getting to what waits

- `W` from anywhere opens the next item waiting for you (a change's diff, a question's conversation, an approval's focus), so `a`/`r` answer it at once; again goes on to the next, round to the first. Nothing waiting: "Nothing needs you right now".
- `S` puts the item at hand off and opens the next: put-off items come after all the others, in the order they were put off, their rows marked `later ·`. Putting off is for this run only.
- The salon sorts each project's workers by what they need of you: waiting for you (an open item or waiting status) first, then stuck (blocked or stopped), then done and not yet seen, then the rest, equals keeping their order. `f` shows only those that need something. The header's fleet (`● working ✓ done ✕ stuck`) and the terminal's title (`imece · 2 waiting`) count the same.

## The `run` tool

Every agent has a `run` tool on the bus: one program and its arguments, no shell, in the agent's own directory (the checkout for the leader, the worktree for workers and helpers).

| | Allowed |
| --- | --- |
| `cargo` | every command except the ones that change dependencies or reach outside the project: `add`, `remove`, `rm`, `install`, `uninstall`, `publish`, `login`, `logout`, `owner`, `yank`. A leader also without the ones that change files (`fmt` without `--check`, `fix`, `clean`, `init`, `new`). |
| `git` | reading commands only: `blame`, `diff`, `grep`, `log`, `ls-files`, `rev-parse`, `show`, `status`. No option before the command (`-c`, `-C`, `--git-dir`), none that starts a pager or writes a file (`-O…`, `--open-files-in-pager…`, `--output…`). |
| `--run PROG[,..]` | the programs you name (`--run npm,make`), for workers and helpers only; repeatable. |
| anything else | refused, with the list of what is allowed and how you add to it. |

- The program is named alone, on PATH, never as a path. The command line is at most 1000 characters; `| & ; < > ( ) $ \` # * ? ~ {` are refused.
- A run is killed after `--run-timeout` seconds (10-3600, default 600), and with imece. The answer waits for the program, not the screen's loop.
- Output comes back tidied: colours stripped, lines of passing tests dropped, repeats collapsed, the middle cut past 48 KiB. Each worktree builds in its own `target/`: a shared one would make the workers rebuild each other's changes in turn.
- `run` has no network block of its own; cargo may reach the registry when the lockfile is off.

The same policy decides which shell commands are routine, so under `standard` a worker's `cargo test` runs whether it goes through `run` or its vendor's shell; only a `run` ties its result to the tree it ran on and shows on the change ([changes](../concepts/changes.md)).

## See also

- [Vendors](../concepts/vendors.md): the confine levels and what each vendor asks.
- [WAITING FOR YOU](../screen/waiting.md): the panel, `W`, `S`.
- [Messages and the board](../concepts/messages-and-board.md).
- [Security](../concepts/security.md): you are the last gate, not a tool list.
