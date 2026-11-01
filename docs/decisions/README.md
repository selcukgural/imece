# Decisions

One file per decision, numbered in the order they were made; the text is
what was decided and why, with the facts the probes found and the gaps
left. An entry that says "above" or "the entries above" means one with a
lower number, "below" one with a higher number; entries name each other by
their titles. `CLAUDE.md` keeps the standing rules and a one-line index of
these files.

A new decision gets the next number (`NN-short-name.md`, `# Title` first,
then the entry as one paragraph or a few) and a line in the index in
`CLAUDE.md`. A later change to a decision goes into its file; a decision
that replaces one says so in both.

Before a decision is written down, go over this list and say in the
entry what each point comes to where it applies (a point that does not
apply is left out, not answered with "n/a"):

- **Growth**: what grows (rows, files, memory, a list on screen), how
  fast, and what removes it (retention, a cap, the session's end).
- **Concurrency**: who else touches the same state at the same time
  (threads, another imece, the vendor's own process) and what keeps
  them apart.
- **Read cost**: what each frame, key or tick reads, and that it does
  not scan what grows.
- **Ten times the scale**: what breaks first at ten times the agents,
  projects, lines or files, and what would warn of it before it does.
- **Migration**: what an earlier store, file or flag becomes, and what a
  later version can still read.
- **Failure**: what happens when it fails halfway (a crash, a refused
  call, a vendor gone), and that it fails closed where a boundary is.
