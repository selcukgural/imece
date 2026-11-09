# 2026-11-09 shelf · find ignores case and matches authors; then lending, on Lonca

- Setup: live, 120×40, on `~/code/shelf`, a fresh small Python repository
  tested with the standard library's `unittest`. The first task to the
  leader ("find should ignore case and match authors") went to one worker
  on the cheap tier (haiku) in a git worktree; its change was applied
  staged and committed with `:commit`. Then `:lonca import`, and a lending
  feature the leader planned as two cards: the plan gate, two workers on
  the standard tier (sonnet) in Lonca workspaces, the code map with real
  leases, both proposals approved, `:sync`, 24 tests passing in the
  checkout. About $0.65 in all.
- What worked: the whole loop, from the first message to an approved
  change on Lonca and the checkout synced, on a repository imece had never
  seen. `:code` on Lonca showed the leases the workers really held (a
  worker, its file and symbol): the map at its best, and good material for
  a video.
- What rubbed:
  - [sessions] A message to a worker mid-turn counted as a second turn,
    while Claude Code folds it into the turn under way and ends it with one
    result. The count never came back to zero: the worker's row kept the
    demo's progress label (`● ░░░░░ plan`) for good, its Lonca proposal
    was never offered as a change, and its `done` never reached the
    leader. Both workers written to mid-turn stuck this way; the one left
    alone did not. After a restart, WHERE YOU LEFT OFF named both as
    turns cut short when imece closed, though they had finished.
  - [sessions] After the restart the pot read `≈$0.65+?`.
  - [Δ] Python's tests were not routine: `python3 -m unittest`, the test
    command the repository's notes give, asked for a yes, so the first
    task stopped on a permission at once, and every test run of every
    worker asked again.
  - [Δ] `A` on `python3 -m unittest 2>&1 | tail -8` said yes to that line
    alone, so the same run with `| tail -4` asked again; a standing yes
    can never cover `python3 -m`, since its first word is an option.
  - [Δ] The first change's verdict said `! look first · nothing ran`
    although the worker had run the tests (through Bash, after a yes): the
    `run` tool knows `cargo` unless `--run python3` is given, and the
    worker's own why said so. Every change of a project that is not Rust
    would say "nothing ran".
  - [screen] An empty Home at 120 columns drew its "nothing needs you"
    words twice, in the list and in the preview, cut in both; its status
    line offered `@ write to this agent` with no agent there.
  - [screen] The leader's trace line `commands …` listed every slash
    command and skill of the person's own Claude Code: noise, and private
    in a recording.
  - [screen] The leader's closing reply sat under TO DECIDE's questions
    with `! 1 to decide`; a reply is not something to decide.
  - [screen] A card's title was cut at about 20 characters while the row
    had 30 columns and more free.
  - [screen] imece's mark was drawn in Home's free rows while a worker
    existed (idle, its change waiting).
  - [screen] The change preview's why was the worker's text as one
    paragraph, its `- ` bullets run together.
  - [screen] When the plan arrived, Home scrolled so that the TO DECIDE
    heading and the plans group were off the top, the selection left on
    the idle worker's row.
  - [screen] The plan window cut long lines at its right edge (a card's
    text, the first line of the explanation) instead of wrapping them;
    `j` `k` did not bring them into view.
  - [screen] The import window showed the checkout as a full path under
    the home directory, not `~/code/shelf` as everywhere else.
  - [screen] After the yes to the plan, the leader's earlier reply that
    hiring the second worker waited on that yes stayed at the top.
  - [leader] After the yes to the plan the leader hired two new workers
    and left the first one, idle with its card done, unused.
  - [leader] The plan ran its second card (the command line and storage)
    beside the first (the loan model) without `after`. On Lonca the second
    workspace starts from trunk without the model, so its tests could not
    pass, and its `done` said so.
  - [Lonca] Both workers in Lonca workspaces wrote their files through
    Bash (`cat > file <<EOF`, a `python3 -` script), right after taking
    their lease, never trying Edit; the haiku worker in a git worktree used
    Edit. One such ask was 180 lines, unreadable on Home with its lines
    joined. It was refused and the worker told to use Edit.
  - [Lonca] Two proposals waited for a human while the header said one,
    and no change row showed: both workers were still mid-turn.
  - [Lonca] A worker's lease intent named card #4 while the screen called
    its card #3.
  - [Lonca] The verdict said `nothing runs after your yes`: a fresh import
    has no `[verification]`, and nothing tells the person how to make
    Lonca run the tests.
  - [Lonca] A second `a` right after the first approval did nothing that
    could be seen (a `lonca` command still under way?); pressed again, it
    worked.
- What was done:
  - Being fixed today: the turn count of a message sent mid-turn (the
    stuck label, the proposal not offered, the `done` held, WHERE YOU LEFT
    OFF's cut turns); the test runners read from the repository for
    Python, JavaScript and Go (the asks for every test run, the yes that
    cannot stand, "nothing ran"); the screen fixes on Home, the preview,
    the plan window and the trace (the empty Home, its status line, the
    `commands` line, the reply under questions, the cut card title, the
    mark beside a worker, the why's bullets, the scroll on a plan's
    arrival, the plan window's cut lines).
  - Left: the pot's `+?` (to look at again once the turn count is
    fixed), the import window's path, the reply that outlived the plan's
    yes, the idle worker passed over, the missing `after`, the Lonca
    workers' Bash writes (the prompt, Lonca's hook or the model: to find
    out), the pending count on the header, the lease's card number,
    telling the person how Lonca would run the tests, the second `a`.
