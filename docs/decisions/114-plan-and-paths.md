# A plan the person sees, and the paths a card owns

The last piece of record 111's first round (#290, 2026-11-04): F20, a plan
the person reads before the leader hires a second worker, and F16, the
paths a card owns, together, since a plan is made of paths. Paths come
first, in this entry; the plan gate is the next pull request and is added
here when it lands.

## The paths a card owns (F16)

What. An `assign` or a `post` takes `paths`: at most 20 git pathspecs
relative to the repository, each at most 200 characters, no control
character, no `..` component, not starting with `/`; a leading `./` and a
trailing `/` are dropped; one ending in the word `shared` is named but not
owned (`paths.rs`: `Owned { path, shared }`, `parse`). A path names a
target when it is the same path, a directory the target is under, or a
pattern the target matches whole or as a directory (`*` stands for
anything, `/` included, as git's does; `names`, `glob` as a table over the
pattern's characters and the target's lengths, never backtracking, so a
pattern of two hundred stars costs its length times the target's and no
more). The card keeps them (`Card.paths`; an `assign`'s ride in
`Mail.paths` until it is delivered, since a card opens only then), the
worker's task ends with them (`bus::owns`: `This card owns …: stay in
them; what else needs changing goes in your done, or ask for the card to
be widened`), `claim`'s answer too, and the card's screen lists them. A
card without paths has no bounds.

Overlap. No two open cards of a project own the same path: an `assign` or
a `post` is refused when a waiting or working card of the project owns a
path that names, or is named by, one of the new ones (`paths::overlap`,
either way round, shared paths on either side clashing with nothing;
`Live::owned_elsewhere`), with which card owns what and the way out
(`#3 Fix the parser owns "src/parser.rs", which "src" names too; give
this card other paths, or end one with the word shared (src shared)`).
The card the receiving worker holds now is not counted: its next card
opens only once that one is done, and may go on in the same files. A done
card owns nothing. The leader's instructions say to name paths on every
assign, to give a file most tasks touch to one card, and to end a file
every card must touch with `shared`.

Strays. A worker's session shows what it writes (`Edit`, `Write`,
`MultiEdit`: `sessions::writes`, the target relative to its directory).
The first time a file outside its card's paths is written
(`paths::strays`: the card owns paths and none, shared ones included,
names the file), `World::strayed` puts `StoryEvent::Strayed(path)` on the
card and `Event::Strayed { agent, card, path }` in the journal, and
`Live::strayed` (from the step loop, after `apply`) queues a note for the
worker's next prompt: `you changed X, which #3 does not own (it owns …);
say so in your done, or ask lead to widen the card`. Once per file per
card; never stopped and never refused: the person is the gate, and the Δ
header counts the files of the change the card does not own under the
kinds line (`diff::outside_line`: `2 outside #12: src/other.rs,
Cargo.toml`, in the warning colour; the card is the worker's last, done
or not, since a change is read after its `done`). Only workers are
bounded: a leader writes nothing, a helper writes in its worker's
worktree on what the worker gave it.

Why paths and not leases. Record 17's symbol leases need a code map imece
does not have live; a path is what the leader can name when it plans
(the plan gate is made of them), what the bus can check at `assign`
without reading a file, and what a change can be held against from its
file list alone. Overlap is refused, not serialised (voff serialises
overlapping work; `docs/neighbours.md`): the leader narrows a card or
says `shared`, and the person sees what strayed. Leases come only if
paths prove not enough.

Growth: a card's paths are a short list on the card (`cards.paths`, JSON)
and on an undelivered assign (`mail.paths`), schema 25; a stray is one
story line and one journal line per file per card. Concurrency: the
check at `assign`/`post` and the stray at a session's event both run on
imece's own thread over the world. Read cost: `owned_elsewhere` scans the
project's cards once per `assign`/`post`, each path against each
(≤ 20 × 20 × a glob of the two lengths); `outside_line` reads the
change's file list against the card's paths each frame of the diff view
(bounded by the files shown). Ten times the scale: the overlap scan grows
with a project's open cards (capped at 20 on the board, plus a card per
worker). Migration: an older file gets the two columns as `[]`; an older
imece refuses the file as any newer file. Failure: a `paths` that does
not read is refused whole with why, nothing sent; a stray noted after a
session ended rides the next prompt as any note.

Not done: `paths` on `handoff` (a card handed on keeps its own); the
`board` tool's lines do not list a card's paths; a path's overlap is
checked only between patterns and plain paths each read against the
other, so `src/*` and `*/a.rs` do not clash though both may name
`src/a.rs`; helpers are not bounded by their worker's card.

Demo (record 31): a taken card owns the files of its leases
(`Sim::take`); every fourth card's worker, forty percent into its work,
edits the first file of the code map its card does not own, once
(`Sim::stray`): the trace shows the edit and the note live would put in
the next prompt, the story and the journal say it strayed, and its done's
change holds the file, so the Δ header counts it.

## The plan gate (F20)

What. Before the leader hires a second worker (the first is cheap and
the person asked for work), its plan comes to the person: the `plan`
tool (the leader's only, `tools::plan_tool`) takes `cards` (1 to 20, each
a title of one line, a body, a tier, `paths`) and `cost` (dollars,
rounded to ten-thousandths, saturating), checked as cards would be
(`Live::plan`: the `assign`'s rules, plus no two cards of the plan owning
the same path by `paths::overlap`); `World::propose` closes the plan
that waited (a new one replaces it), takes the yes back
(`PlanGate.approved = false`), keeps the plan on the project
(`Project.plan: PlanGate { on, approved, proposal }`, `projects.plan`
JSON, schema 26) and puts one `InboxKind::Plan` item (`▤`, the leader's,
detail the first card's title) in WAITING FOR YOU, journaled as
`Event::Planned`. The item holds mail as any item but a reply or a
change (record 107): the leader was told to end its turn, and its mail
waits for the answer.

The gate. `World::plan_hold(project)`: nothing when the gate is off, no
worker is there (gone ones do not count) or the last answer was yes;
else `NoPlan`, `Reading` (the item is open) or `Unanswered` (closed by
the person's word). `Live::hire_room` turns it into the leader's refusal
after the cap check (a full team is said first), so `hire` and `whoami`'s
`may hire` say the same words. A solo never passes through the leader's
arm, so it never plans.

The answer. `a` (`App::answer_plan`, on the item or in its window) →
`World::decide_plan`: the item closes, `approved` is set, `PlanAnswered`
journaled; `Live::decide_plan` queues `PLAN_YES` as a note and sets
`lost[leader]`, the same wake as a lost helper's: the leader is prompted
`Go on with your task.` under the note as soon as its turn is over, so
the yes starts its next turn. `r` drops the plan and queues `PLAN_NO`
(the leader asks the person or plans again). `n` opens the composer to
the leader; the person's word closes the plan unanswered
(`InboxKind::Plan.closes_on_word`, `World::close_plans` in `said`), and
`hire` says to plan again from what they said. `enter` opens the window
(`App::plan_window`, `overlay::plan` on the script window's model: the
head, the expected cost beside the pot, `overlay::plan_lines` for the
cards, `j k` and the pages over them, the keys at the foot), every word
of it the leader's through `clean`. `:plan [on|off]` sets the project's
gate, bare says where it stands; on by default. The leader's
instructions say to plan before a second worker and end the turn.

Growth: one `PlanGate` per project with at most one plan of twenty
cards; a closed item per plan made (the inbox's, as any). Concurrency:
all on imece's thread over the world. Read cost: `plan_hold` counts the
project's workers at each `hire`/`whoami`; the window reads the plan
from the project. Migration: an older file has no column (`''` → the
default, on); an older imece refuses the file. Failure: a plan refused
whole with why, nothing kept; a yes while the leader's session is gone
rides its next prompt as any note.

Not done: the plan's cards are not turned into `assign`s by imece (the
leader does that, which is the point: the body is never interpreted);
the cost is the leader's word, not checked against tiers; `n` quotes
nothing (the leader knows its own plan).

Demo (record 31): the leader proposes instead of hiring when a worker is
there (`Sim::propose`: a card per waiting card, each owning one file of
the code map, a flat `PLANNED_COST` a card), hires on the next chance
after the yes, and asks the person what should change after a no
(`Sim::decide_plan`).

Field fix (2026-11-09): the plan window cut long lines at its edge (a card's
title and body, the head line). Every line wraps within it (`ui::wrap`; a
card's lines under its three-cell indent, `overlay::hung`); the drawing
keeps its width in `App.plan_width`, so `j k` and the pages scroll the
wrapped lines (`plan_lines(app, p, width)`).

**The reply that came with the plan (2026-11-09).** The leader's closing words
of the turn that proposed ("I wait for your yes before I hire") stayed in TO
DECIDE after the yes. One open reply per agent held already (`World::replied`
closes the older); now `decide_plan` closes the leader's open reply too: the
decision answers it.
