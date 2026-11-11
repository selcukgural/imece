# 2026-11-11 shelf, tally, plain · the acceptance run for v0.1.202611111627

- Setup: the whole catalogue on the candidate 77385ef (record 147,
  report `docs/acceptance/runs/v0.1.202611111627.md`): 57 scripted cases
  on the stand-in (129 s at 6 jobs), 12 real cases on Claude Code 2.1.322
  with a plan's login (102 s at 3 jobs, about $0.40), Lonca 0.2.3 with one
  daemon per Lonca case, the five auto cases. 74 of 74 passed; then the
  tag, the release, the installer, the Homebrew tap and crates.io.
- What rubbed:
  - (the report tool, no seam of imece's) TEAM-2's row said it spent `?`.
    imece's header said `≈$0.04+? of $2`, rightly: the worker's last turn,
    cut by the script's `stop`, had no figure (record 132). `drive.py`'s
    pattern took only the `?`, so the row and the run's total lost the
    known part. The fix could not land before the tag: after the run's
    commit the gate allows only `runs/`.
  - [install] Publishing to crates.io from the tag: the stored token was
    refused (403) and a new publish-update token was made by hand. The
    Homebrew tap's update was started by hand too.
  - (CI, after the run) Two macOS tests failed on a loaded runner, neither
    in imece's code: the signed sleeper of `procs`' test took longer than
    5 s to run the first time, and the Lonca ear test counted watches in a
    fixed 600 ms.
- What was done:
  - `?`: #487 (`drive.py` reads `≈$0.04+?` whole; the total adds the known
    part and counts the cases with a turn that has no figure).
  - crates.io and the tap: left to the person (Trusted Publishing and the
    `PUBLISH_CRATES` variable, record 69).
  - The two tests: #488.
