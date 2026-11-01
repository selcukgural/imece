# Solo sessions

Solo sessions (2026-11-01, user's ask and decisions): next to its team
projects, imece opens LEADERLESS sessions where the person picks the
vendor, the model and the thinking effort themselves, as many of each kind
as they like (team, solo, team, solo, solo), each a group in the salon.
The design, agreed before any code: a solo is `Role::Solo`, a project whose
`Project.leader` IS the solo agent (the field keeps its one meaning, the
agent the project's command line goes to, so the salon header, the budget
and confine notes, `:rename`, `:close` and `:pin` go on unchanged). A solo
gets the worker's toolset under the confine dial (read, write, shell, web),
works in its OWN worktree, and at every turn end imece reads its tree: a
difference is a Δ in WAITING FOR YOU, applied, rejected or noted as any
worker's (record 11); the person's own checkout is never written, so two
solos and the person's own edits on one checkout do not collide. A solo may
hire helpers (`--helpers`); it has no board, no claim and hires no workers.
`:solo [PATH] [CHOICE] [NAME]` opens one (PATH: the checkout at hand,
CHOICE: the pick screen, `ui/pick.rs`: vendor, model, effort), `imece
--solo CHOICE` starts leaderless; one checkout may hold one team project
and any number of solos (two teams on one checkout are still refused);
the agent is `solo-NAME` (`solo` at home), as a leader is `lead-NAME`;
`:effort LEVEL` and `:model NAME` change the solo at hand (Codex: from the
next turn, the effort rides on the turn; Claude: the session starts again
on the same transcript, `Live::restart`). Store: `agents.effort`, role
`Solo` (schema 12). The demo opens solos too (record 31). Three PRs, in
this order: (1) the choice syntax below; (2) the role, the commands, the Δ
at turn end, the store, the salon, the demo, the pages; (3) the pick
screen, completion, `:effort` and `:model`.

The choice (PR 1, the `choice` branch): one form for every vendor,
`VENDOR[:MODEL][@EFFORT]` on the command line and `[MODEL][@EFFORT]` where
the vendor is known, `imece_agent::Choice` (`parse`, `wire`, `check`):
Codex's `[MODEL][@EFFORT]` of record 06 generalised, so `--model
opus@xhigh`, `--tier strong=opus@xhigh`, `--model claude:@max`. The
efforts are each vendor's own (`claude::EFFORTS` low, medium, high, xhigh,
max: Claude Code 2.1.316's `--effort`; `codex::EFFORTS` low, medium, high,
xhigh, on every turn as before; `gemini::EFFORTS` empty: Gemini CLI 0.65.0
has no such setting, so `gemini:pro@high` is refused). Checked fail
closed, in words for the person: on the command line when the vendor is
named (`--model: claude has no effort "bogus"; one of …`), in
`live::world` for a bare value once the leader's vendor is known
(`check_ladders`, `--model/--tier "opus@bogus": …`, exit code 1), and
again in each vendor's `Session::start` before anything is spawned or
written (Gemini's private directory is not even made). Probe facts
(2026-11-01, `claude -p`): `--effort` is taken in print mode with
stream-json; an UNKNOWN value is only WARNED about (`Warning: Unknown
--effort value 'bogus' — ignoring it and using the default effort`) and the
session runs, which is why imece refuses first; the `init` event carries
`per_turn_effort_active: true` but not the level, so the effort asked for
is not verified afterwards (a known gap; the model is). `claude::Spec`
gained `effort`; a Claude agent's choice is split in `Live::start`
(`sessions.rs`). Nothing of this shows in the demo: it has no command line
for models. Known gaps: the leader's own effort can only come through
`--model`/`--tier` of the standard tier (a `--lead claude:opus@high` form
is for later); the `hire` tool still picks tiers, never efforts.

Solo sessions v0 (PR 2, the `solo` branch, 2026-11-01): `Role::Solo` in
`model.rs`; `World::head_name(project, name, role)` (`lead`/`solo` at home,
`lead-NAME`/`solo-NAME` elsewhere; `World::head_stem`), `World::open_project`
takes the head's role, `rename_project` keeps the stem, `World::is_solo(p)`,
`free_project_name` (`kule`, `kule-2`, cut to a project name). Live:
`Live::open_project(w, path, name, Open)` with `Open::Team` or `Open::Solo {
vendor, choice }` (`live/projects.rs`): a team refuses a checkout that has an
open team (`PathOpen`), solos never; the directory's name is numbered when a
project has it (teams too, now); a closed project is reused only by its own
kind; the vendor must be hireable here (`ProjectRefusal::Vendor`) and the
choice valid for it (`ProjectRefusal::Choice`); the solo's model is the
choice, else the vendor's standard rung, never tiered. `Options.solo` and
`imece --solo V[:MODEL][@EFFORT]` (`main.rs`, `parse_solo`, `vendor_choice`
in `model.rs`; without `--lead` the solo's vendor leads, no question):
`live::world` goes on with an open solo of the same vendor and choice at the
checkout (`Live::solo_at`, through `worktree::top` so paths compare as git
gives them), else opens one; a bare start opens a team unless one is open,
solos do not stand in for it. A solo is a writer: `Live::start` gives it a
worktree named `solo-<project id>` (`Live::tree_of`, also its helpers'; a
rename renames the agent, not the tree), the worker's toolset under the
confine dial, `tools_of(Solo)` = send, run, hire (no post, board, claim: the
board is the team's), the Solo prompt and a solo's helper's prompt
(`prompts.rs`), `hire` lets it hire helpers (`solo-notes.1`), `follow`,
`stand`, `stand_busy` and the row's link treat it as a worker, `reports`
as a leader. The Δ: in `Live::step`, a solo's turn that ends with an answer
the person gets (`asked` or `reports`) calls `snapshot` with the answer's
first line as the title and nobody to answer; an empty answer (a command's
turn) reads nothing; a worker's turn end still reads nothing, its `done`
does. Screen: `:solo [PATH] [CHOICE] [NAME]` (`App::solo_project`, the
words placed by shape: `/` `~` `.` → path, a vendor's name → choice, the rest
the name; the flash `solo_opened`; `$` in the menu), `App::model_of` from
the engine for the row; the salon draws a solo with its vendor and choice
where a leader shows its vendor, the status glyph (not ◆), its stand and
link, its helpers under it at depth 2, and counts it as working in the
header; `:solo CHOICE` in the `?` help (`WRITING_KEYS` 20). Store: schema 12
with no DDL change (an older imece can not read the `Solo` role tag, so the
file says it is newer). Demo: `Sim::open_solo` (`:solo`, numbered names,
reopened by name), a told solo works and answers; about every other answer
shows a change of its code map (`Sim::models` for the row); `--solo` is
not taken by the demo (it has no vendors to pick from) and the default demo
world has no solo, so the tour videos stand. Known gaps: no pick screen for
a bare `:solo` (it takes the lead vendor's standard rung; PR 3), no
completion after `:solo `, no `:effort`/`:model` (PR 3); a solo's Gemini
private directory follows its tree name (`solo-N`), its helpers' their own
names; a solo's turn that ends while a helper is mid-turn is not a report
(as a leader's), so its answer waits in its story and the Δ comes with the
next answer; the two kinds of project are told apart only by the head's
role.
