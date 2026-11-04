# Claude Code only

The person's decision (2026-11-04): supporting every vendor strains the
application, so imece runs Claude Code alone for now. Codex and Gemini CLI
leave `main`; their code stays on the branch `multi-vendor`, with the tag
`multi-vendor-last` on the same commit. `main` does not depend on that
branch and owes it nothing: coming back to several vendors is a new
decision.

What went. `crates/imece-agent/src/codex.rs` and `gemini.rs`; the `Vendor`
type with all that hung on it; `--lead`, `:lead`, the who-leads question
(`ui/pick.rs`) and the primary vendor; the `V:` prefix of `--model` and
`--tier`; `hire`'s `vendor`; sending to a group by vendor; the vendor step
of the `:solo` pick screen; the colour per vendor and the vendor word on
rows; and the session's own wire for asking (`Agent::decide`, `Event::Asks`,
`Event::Withdrawn`), which only those two used: Claude Code asks through
imece's socket.

What stayed in another form. `--solo claude[:MODEL][@EFFORT]` and the
`:solo` word keep the literal `claude`, which tells a choice from a path or
a name. Workers are still `claude-N`. `:review` (record 106) stays: with no
other vendor, a fresh Claude Code session with no tools and none of the
worker's context reads the change. That is a second reading, no longer a
second vendor's. The confine dial (record 28) stays with its three levels.

Migration. Schema 22 drops `agents.vendor`. Before it does, a head (leader
or solo) of another vendor goes on as Claude Code: its session, its spent
floor and its model are cleared, since neither means anything to Claude
Code, and it wakes in a new session told where it left off. Any other agent
of those vendors counts as gone, its session cleared; its worktree stays,
for `:tidy`. Journal lines may still say `"by": "Codex"`: an unknown field
is ignored, so they load. An old `lead` in the store's `meta` is ignored.
A file of schema 22 is refused by an older imece, as any newer file is. A
Gemini private directory (`<worktrees root>-gemini`) stays on disk unused:
imece never removes what it no longer knows, and the person may.

Read cost and scale. Less of both: one program, one ladder, no vendor
dispatch; nothing new grows. Failure: Claude Code missing from PATH stops
the start with what to install; a session's checks fail closed as before.

The tour videos still name the vendors until they are rendered again
(`website/video/README.md`). Records 03, 05, 06, 07, 08, 21, 22, 30, 48, 59,
63, 64, 75, 79, 94, 105, 106 and others describe the application as it was
with several vendors; the parts about Codex and Gemini are history, and
records 06, 07, 08, 21 and 22 are replaced whole.
