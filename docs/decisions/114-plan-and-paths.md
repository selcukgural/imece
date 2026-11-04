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

The next pull request.
