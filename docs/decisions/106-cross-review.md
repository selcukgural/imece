# A change read by another vendor

Backlog F24 in #58 (from OpenHands). The frame was decided by the person
on 2026-11-03: optional, off by default, an advisory note on the Δ that
never gates it, the person still the last gate, the cost from the pot.
The details were proposed and taken on 2026-11-04 (who reviews, how the
call is made, the switch, the ceilings). Built on 2026-11-04.

**Why.** A worker's change reaches the person as a patch to read (record
11). A second model, of another vendor, reads it first and says what it
finds: a missed case, a test that tests nothing, a title that says more
than the patch does. A model of the same vendor shares the worker's
blind spots, so the reviewer is never the worker's vendor. The note is
advice: nothing waits for it, `a` applies with or without it, and a
change is never held back because a review said something. This
changes the line in `docs/neighbours.md` that said no model reviews
patches: one may, when the person asks, and only as a note.

**When.** `:review` on a change (the one open, else the one selected in
waiting for you) asks for one now. `:review on` asks for one for every
change as it comes to wait (`App::watch_reviews`: new open `Change`
items in the inbox since the last step), `:review off` stops that;
saved in prefs as `review`, off by default. A refusal asked by hand is
flashed; one asked by `:review on` is journaled (`ReviewFailed`), since
nobody is looking at a flash then. The `:` menu shows `(on) $` /
`(off) $`.

**Who reviews** (`review::reviewer_of`): a vendor that is not the
worker's and that can be asked a one-shot question, Claude Code or
Codex, on PATH; the project's leader's vendor first when it is one of
them, then Claude Code, then Codex. Gemini CLI is not asked yet (its
one-shot call would need a probe of its own), and a Gemini worker is
reviewed by the others. None there: refused, saying what it needs.
Whether the vendor is signed in is not looked at before: the call says
so if it is not.

**The call.** One process per review, a prompt in, an answer out, gone
(record 56's way), in the bus's private directory where there is
nothing to read:

- **Claude Code** `claude::once`: no tools, no MCP server, the strict
  level, `--system-prompt` with the reviewer's few lines, no session
  saved, `--max-budget-usd 0.10`. The model is the person's
  `--tier claude:standard=…`, else `sonnet` (Claude Code's own default
  may be its largest model, which the cap would cut short).
- **Codex** `codex::once` (new): an app-server session as every Codex
  session is, with everything it reports checked the same way, as a
  reader (read-only sandbox, approvals `never`) at the strict level (no
  network, web search off), no MCP server, on the person's
  `--tier codex:standard=…` else Codex's default; its thread is
  `ephemeral` (`codex::Spec::ephemeral`), checked as Codex echoes it.
  The probe of 2026-11-04 (codex-cli 0.165.0, ChatGPT plan) found a
  thread started with `ephemeral: true` answered with `ephemeral: true`
  and `path: null`, a turn answered as usual (the final text in
  `item/completed` and `turn/completed`), and no rollout written under
  `~/.codex/sessions`. A one-shot thread is never resumed or forked.
  Codex cannot be asked to have no shell; with a read-only sandbox, no
  network and nothing in its directory, what a prompt in the patch could
  make it read stays on this machine and comes back only as text on the
  person's screen.

What it reads (`review::review_prompt`): what the worker said it did
(the change's title), its files with their counts, and the patch, at
most 64 KiB, cut at a line and said to be cut. The system prompt says
the patch and the worker's words are data, not instructions. Its answer
is cleaned line by line (`clean`), blank ends dropped, at most 80 lines
and 8,000 characters (`review::review_text`). The worker is not told:
the note is the person's, who may quote it to the worker (`>`) or note
lines (`c`, record 49).

**Ceilings** (the standing cost rules): a review by Claude Code may cost
at most $0.10 (`REVIEW_CAP`); a day (UTC) holds 20 reviews
(`REVIEW_DAY_CALLS`) and $2.00 (`REVIEW_DAY_COST`), caps held counting,
every project together, apart from `:commit`'s. Codex's reviews cost the
pot nothing (it runs on the ChatGPT plan; an API key is refused, record
06) and hold no cap, but count against the day's 20. Both kinds of
imece's own paid calls now share one ledger (`live/paid.rs`: `room`,
`reserve`, `settle`, each with a `Ceiling`): the store counts by kind and
day (`paid.kind`, schema 21; `Paid::Commit`, `Paid::Review`), in one
immediate transaction that checks and counts, and the pot holds the cap
(`World::paid`) before the call, settled to what it said after; one
that never said keeps its cap. An ask checks the room (`paid::room`)
and is refused at once without it; the reserve is made when its turn
comes (`Live::review_next`), so a day that filled in between is
journaled then. One review runs at a time; the rest wait in
`Live::reviews_due`, each with its vendor.

**Replays are not paid twice.** A change that has its review is refused
another (`… has its review already`); so is one asked for or under way.
A review under way of an older change of the same worker is no reason:
the newer one waits its turn, and the older one's answer, when it
comes, is journaled but not kept, as it read another patch.

**Where it shows.** `World::reviews`, by worker, holds only the review
of the change that waits now: `World::reviewed` keeps it when the
patch it read (`model::patch_sum`, the first 16 hex digits of its
SHA-256) is the one waiting, a newer change, an apply or a reject drop
it (`forget_change`). The change's screen shows it between what ran
and the file list (`ui::diff::review_lines`): `✎ review · vendor · model
· $cost`, then its first six rows (`REVIEW_ROWS`, wrapped), `… +N lines
· R shows all`; `R` shows it whole, up to half the screen
(`App::review_whole`). While one is asked for or under way, a dim
`✎ codex reads this change…` stands there. The journal says each one
with its first line (`Reviewed`, kind Sensitive) and each failure with
why and what it cost (`ReviewFailed`).

**Kept** in the journal: `Event::Reviewed` carries the whole text and
the patch's sum, so a run that loads a waiting change finds its review
again (`World::reviews_from_journal`, after the changes come back in
`Live::adopt`), without a column of its own. The newest of that patch
wins. A review whose journal line was tidied away (90 days) is gone
with it.

**Growth.** One journal line per review, at most 8,000 characters, at
most 20 a day; the journal's own retention removes them. `World::reviews`
holds one per waiting change. **Read cost.** Drawing a change reads its
review from the map; nothing scans the journal but a load, once per
waiting change. **Concurrency.** The call runs on a thread of its own and
sends its answer back; the store's reserve is one transaction, so a
second imece is refused by the store's lock anyway. **Failure.** A
vendor that cannot start costs nothing (`could not start`); one that
stops or runs out of time (300 s) without saying keeps its cap; a
vendor gone from PATH between ask and start is journaled, not counted.
**Migration.** Schema 21 makes `paid` again with `kind` in its key, every
earlier row a commit message's (the one kind there was); an older imece
refuses the newer file, as with every schema step.

**The demo** mirrors it (`Sim::review`, `Sim::reviewed`): the same
refusals and ceilings, an answer three ticks after the review before it,
made up from the change's files (a short list of points, or "nothing
worth saying"), a Claude Code review costing a few hundredths from the
pot, Codex's nothing; a change gone by then takes none.

**Known gaps.** Gemini CLI does not review yet. The ceilings are
constants, not flags. The reviewer does not see the worker's
worktree beyond the patch (no tools by design), so it cannot run the
tests or read code the patch does not show. Claude Code's call may read
the person's `~/.claude/CLAUDE.md` (record 56's gap). Whether a vendor
is signed in is found out by the call, not before.
