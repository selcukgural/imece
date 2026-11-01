# A solo without git

The person tried `imece --solo claude` in an empty folder, to have the
agent start the project (even run `git init`), and imece stopped:
`git rev-parse --show-toplevel: fatal: not a git repository`. They asked
that imece run without git; the whole question (teams too, a shadow git
repo of imece's own) is issue #162, kept for later. For now, their choice
(2026-11-01): a solo runs without git.

**Probe.** An empty `git init` is no help either: a worktree is
`git worktree add --detach … HEAD`, and without a first commit there is
no HEAD (`invalid reference: HEAD`). Codex's app-server in a folder with
no git at all started, wrote a file with its shell and ended its turn
(codex-cli 0.165.0, `--ignored live_codex_writes_outside_a_git_checkout`);
Claude Code and Gemini CLI never needed git.

**What is decided.** `live::checkout_of(path)`: git's top directory and
whether it has a first commit (`worktree::born`), else the folder itself
when it is a directory, else git's words. A checkout with nothing
committed counts as no checkout. `Place::git` keeps it per project, found
at each open (`open_project`, a reused closed solo too) and at each start
(`adopt`: only the folder's own git, compared canonically, counts, not a
checkout around it). In a folder without:

- a solo and its helpers work **straight in the folder** (`cwd_of`, and
  `start` skips `worktree::ensure` and `:copy`'s copies): what they write
  is in the person's files at once;
- nothing of git runs on it (`Live::tree_at` is none, so no `Δ` at a turn
  end, no stand on the row, no following HEAD; `files` lists nothing;
  `:commit` says `this folder is not a git checkout with a commit`);
- `Project::no_git` (not saved; found again) puts `⌀ no git` on the
  header in the warning colour, and `Event::NoGit` says in the journal,
  at the open and at every start, that the solo works straight in the
  folder and that a first commit gives it a worktree from the next start;
- the solo's and its helpers' prompts say so, and that `git init` or a
  commit is for when the person asks;
- a **team** is refused, at the start (`no_git_team`) and by `:open`
  (`ProjectRefusal::NoGit`), with what works instead; a stored team whose
  folder lost its git closes at the next start, a solo stays.

**The Δ gate.** Record 26 makes the person the last gate through the Δ
review. A solo without git has no Δ: it writes as the vendor's own CLI
would in that folder. That is the person's choice for this case, said on
the header, in the journal and to the agent, never silent. With a first
commit the gate is back.

**Demo.** `:solo PATH` in the demo with a real folder without git marks
the project as live does (`checkout_of`, the header, the journal line) and
its answers come without a change; the demo's made-up paths stay
checkouts.

**Not done.** Teams without git and a shadow repo (issue #162); file
completion by walking a folder; moving a running solo to a worktree when
the agent commits mid-run (it moves at the next start or reopen).
