# The person's own instructions for every project

**Changed by [record 110](110-claude-s-own-loader.md): the notes go through Claude Code's own loader, whole; what is below about the 16 KiB cap and `InstructionsCut` is history.**

Backlog F22 (#59, after Terax and cmux). Each vendor has a file of its
own for a person's instructions (Claude Code's `~/.claude/CLAUDE.md`,
Codex's `~/.codex/AGENTS.md`, Gemini's `~/.gemini/GEMINI.md`), and
imece's starts promise none of them: Claude runs `--restricted` (no user
settings), Gemini in imece's private directory, Codex on the person's own
`~/.codex`. Whether each vendor still reads its own file was probed on
2026-11-04 (Claude Code 2.1.318, Codex 0.165.0, Gemini CLI 0.65.0):
Claude under `--restricted` does not, neither `~/.claude/CLAUDE.md` nor a
project's (and `--setting-sources` does not bring them back); Codex reads
`~/.codex/AGENTS.md` and Gemini `~/.gemini/GEMINI.md` under standard and
open alike, and Gemini also a `GEMINI.md` at the top of its included
directory.
A person with three vendors would keep three files in step anyway. So
imece reads one of its own, the same for every vendor:
`instructions.md` in its configuration directory, where the key file
lives (`keys::config_file`: `$XDG_CONFIG_HOME/imece`, a relative one
ignored, else `~/.config/imece`). `main` names it in
`Options::instructions`; tests name their own file or none, so they never
read the person's. Where a vendor also reads its own file, the agent gets
both.

`prompts::personal_notes` reads it when each agent's session starts and
puts it in that agent's instructions after imece's own and before the
repository's notes (`repo_notes`), headed `The person's own instructions
for every project, from PATH:`. It is the person's own file, so it is
trusted as their words are; it goes to every role, the leader too, and
to every vendor (none of them reads it itself). The repository's notes
come after it, so a project's own rule is the later word.

Bounds, as for the repository's notes: at most `MAX_NOTES` (16 KiB) goes
in; a longer file goes in as its head, with
`[cut: the file goes on]`, and the start journals
`Event::InstructionsCut { agent, path, size }` (an assumption, as
`NotesCut`), worded `claude-1: your instructions file PATH is 20 KiB;
only its first 16 KiB went into the instructions`. No more than the cap
is read, whatever the file's size; a character the cap cuts in two is
left out whole (`from_utf8`'s error for an end cut short), any other
byte that is not UTF-8 is replaced. The file is opened
without waiting (`O_NONBLOCK`) and looked at as opened: only a plain file
is read, so a pipe nobody writes to never holds a start and a device
(`/dev/zero`) is passed over. Missing, not a plain file, or empty: nothing
goes in and nothing is said.

Read cost: one small read per session start, on the main thread as the
repository's notes are. Growth: none; the journal line once per start
that cut it. A change to the file is read at each agent's next start
(a running session keeps what it started with). Migration: a new event
in the journal; an older imece skips a journal line it cannot read
(`store`), so the file stays readable both ways. The demo says
`InstructionsCut` once a run as a worker starts (a roll of 4, as the
repository's notes' cut on 2), for `~/.config/imece/instructions.md`.

Not done: instructions per project kept outside the repository (the
backlog's wording allows either; one file for every project was the
plainer first step).
