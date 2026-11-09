---
title: imece doctor
description: One report of what a start needs (Claude Code signed in, git, the checkout's tests, Lonca, the terminal, your files), with what to do about what is wrong.
---

# imece doctor

`imece doctor` looks at what a start needs and says what is wrong and how to mend it, before any agent spends a cent. Run it after installing imece, or when a team does not start.

```sh
cd your/checkout
imece doctor
```

```text
Doctor summary (imece 0.1.0, checkout ~/code/shelf)

[✓] imece: 0.1.0, state in ~/.local/state/imece
[✗] Claude Code
    ✓ claude --version: 2.1.321 (Claude Code)
    ✗ not signed in
      run `claude auth login`
    ✓ takes every option imece passes
[!] git
    ✓ git version 2.56.0
    ! ~/code/shelf is on no branch (a detached HEAD)
      agents work as ever; :lonca import needs a branch (`git switch NAME`)
[✓] This checkout's tests: python3 -m pytest … · python3 -m unittest … · pytest …
[✓] Lonca: lonca 0.2.2, this checkout is not on Lonca
[✓] Terminal: iterm2 3.6.6, 120×40
[✓] Your files: ^x ^e opens nvim

✗ Doctor found issues in 2 categories.
```

## How to read it

Each line is a category, marked by its worst finding:

| Mark | Meaning |
| --- | --- |
| `[✓]` | nothing to mend; the line says what was found |
| `[!]` | it works, but something will be refused or is not as good as it could be |
| `[✗]` | a start, or the part named, does not work |

A category with an issue lists every finding, and under each one that is not right, one line of what to do. Inside a category `•` marks a note: something to know that needs no mending (no key file, a narrow terminal). A note never makes a category an issue.

The last line counts the categories with issues. The exit code is 0, or 1 when a category is `[✗]`, so a script can stop on it.

## What it looks at

| Category | Findings |
| --- | --- |
| **imece** | its version; the state directory (there, or can be made, and yours to write to); free space there; the store (its schema, a file a newer imece wrote, one that can not be read); another imece running on the store; how many worktrees are kept |
| **Claude Code** | on PATH and its version; signed in (a plan or an API key); whether its `--help` still names every option imece passes |
| **git** | its version (a sparse checkout's worktree needs 2.35); the folder: a checkout, with a commit, on a branch; its `.worktreeinclude` when there is one |
| **This checkout's tests** | the test commands imece found from the checkout's files (cargo, Python, Node, Go), which agents run without asking and which count on a change's verdict; a hint when none |
| **Lonca** | only when `lonca` is on PATH or the project is on Lonca: its version, the daemon and your session, the project's repository and its policy |
| **Terminal** | its name and version, its size, 24-bit colour, the kitty keyboard protocol (shift-enter), progress on the tab, how notices go, tmux, SSH |
| **Your files** | `instructions.md`, the key file, the push file, the editor `^x ^e` opens, the tool `:awake` holds the machine's sleep with |

Without a git checkout with a commit, the tests are not looked for. When the output is piped or redirected, the terminal is not asked.

## Options

| Option | What it does |
| --- | --- |
| `--repo DIR` | the checkout to look at (default: the current directory) |
| `--verbose`, `-v` | lists every finding of every category, also where nothing is wrong |
| `--json` | the report as JSON, for scripts |
| `--lang en\|tr` | the report's language (default `en`, or `IMECE_LANG`) |

The marks are in colour on a terminal; `NO_COLOR` turns that off.

### `--json`

```json
{
  "imece": "0.1.0",
  "issues": 1,
  "failed": true,
  "categories": [
    {
      "id": "claude",
      "title": "Claude Code",
      "status": "fail",
      "items": [
        { "id": "claude.login", "status": "fail", "text": "not signed in", "fix": "run `claude auth login`" }
      ]
    }
  ]
}
```

A finding's `id` is the same in every language; its `status` is `ok`, `note`, `warn` or `fail`, and a category's is its worst (`ok` when it has only notes). `text` and `fix` are in the report's language.

## On the screen: `:doctor`

While imece runs, `:doctor` opens the same report in a window over the screen, for the project at hand. imece asks on a thread of its own, so agents go on working; `v` lists every finding, `r` looks again, `esc` closes ([commands](../reference/commands.md#doctor)). The welcome window points at it when Claude Code is not on PATH or not signed in.

## What it does not do

- It calls no model and spends nothing.
- It writes nothing: the store is read without being taken or changed, so it is safe to run beside a running imece. Nothing is made, not even the state directory.
- It asks Claude Code and Lonca through their own commands (`claude --version`, `claude auth status`, `claude --help`, `lonca --version`, the daemon's socket) and never reads their files.
- It does not mend. Each finding says what to run.

Every refusal imece itself gives at a start is in [troubleshooting](../troubleshooting.md).
