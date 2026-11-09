# The checks a card asks for

Step 5 of the Lonca catch-up (user, 2026-11-08, accepting the proposal): a card may
ask Lonca's merge queue for checks of the repository's policy beyond its own, and
the person's yes is what asks for them. Built on Lonca `becbb44` (lonca#191 →
lonca#196: `approve.checks`, log format 25).

**What Lonca offers.** `[verification] checks = { api = "…", e2e = "…" }` names
commands that run only for a change that owes them; `checks_by_path` makes a path owe
one. A human owes a change more at approval: `lonca approve SEQ --also api,e2e`
(`approve.checks`: only on a pending `change.propose`, else `checks_not_a_change`;
only names the policy in force defines, else `unknown_check:<name>`; sorted, each
once, at most 32, else `invalid_checks`). A name is `[a-z0-9_-]`, 1 to 32, not one of
Lonca's own (`build test tests extended syntax scope claim_diff merge
test_integrity`); a dash may come first. A later version of the change keeps them,
and `runs` on `status.pending[]` lists every named check a change owes, asked or by
path (record 122). Only a human signs an approval, so no agent adds or drops a check.
A denied approval exits 1 and its last line is `denied: <reasons, joined by ", ">`.

**Who names them.** The leader, as it names paths (record 114): `checks` on `assign`,
`post` and a card of `plan` (`checks::parse`: an array of names, ≤ 32, each
`imece_agent::lonca::is_check_name`, none twice, sorted). Refused on another type
(`checks go with an assign: …`) and on a project not on Lonca (`bus::checks_run`: nothing would
run them; deny by default). Whether the policy defines a name is Lonca's to say at the
yes: imece does not read `[verification]` (record 122 stopped that). `Card.checks`
(given when the card is made; changed only as **Changing them** says) and `Mail.checks` (an assign's until it
is delivered) are schema 32 (`cards.checks`, `mail.checks`, `'[]'` by default; a load
refuses a column that is not names Lonca takes, since they reach a command line). The
worker's task ends with `This card asks for Lonca's checks api, e2e: once the person
says yes, …` (`bus::asks`, also on `claim` and a card handed again); the leader's
system prompt says checks exist only on Lonca (`prompts::lonca_checks`); a card's
screen and the plan window list them.

**The yes.** `World::asked_checks` is the checks of the card the change was made for
(`change_card`); `a` on a proposal sends `Decision::Approve(checks)`, and
`Place::approve(repo, dir, seq, also)` adds one argument `--also=api,e2e` (so a name
that starts with a dash is no option), refusing before it runs anything a list Lonca
would refuse (`lonca::checks_asked`). The verdict carries `asks`: those of the card's
checks `runs` does not list (all of them when Lonca did not say what runs); the line
adds them to what runs after the yes and says `the card also asks for api`, so
`nothing runs after your yes` never shows while the yes asks for something. Not a
reason to look first. A denial is said in words (`words::denied_why` →
`refused_why`: `Lonca's policy has no api check`, `the card's checks are not a list
Lonca takes`, `checks go only with a proposed change`); the proposal still waits.
What ran is the journal's `Verified` line, a named check by its name (record 122).

**Changing them** (user, 2026-11-08: "close the gaps with smart behaviour", without
Lonca, whose release was under way). Two hands, through one `World::set_checks(card,
checks, by)`: nothing noted when the list stays as it was, else `StoryEvent::ChecksSet
{ by, checks }` and `Event::ChecksSet { agent, card, checks, you }` (`lead set #3's
checks: api, e2e`, `you set #3's checks: none`); the save sum of a card already holds
its checks, so no schema change.
- The leader, handing a card again (`assign` with `card`): `checks` given there take
  the card's place, `[]` for none (the key there and not null); left
  out, the card keeps its own. The same refusals as a new card's; the assign's mail
  carries none (the card's are the card's), and the prompt names the new ones.
- The person, at the yes: `:checks [NAMES|none]` (`App::checks_command`, the change at
  hand as `:review` finds it: `App::change_at_hand`) sets the card's list (`checks_named`:
  commas or spaces, `is_check_name`, sorted, each once, at most `CHECKS_MOST`), `none`
  clears it, bare says it; refused off Lonca, for a change made for no card, a name
  Lonca would not take, with the list kept. The menu note is the list.
- A yes Lonca refused for the card's checks (`unknown_check:`, `invalid_checks`; not
  `checks_not_a_change`) ends with ` · :checks NAMES changes what the card asks for`
  (`words::denied_why`), so a typo is fixed and the yes said again instead of a
  reject.
imece never adds or drops a check by itself: either the leader names them or the person
does, both journaled, and the verdict line says what the yes will ask before it is
said. Demo: a card of the project on Lonca handed again that asked for nothing asks for
`api` (the leader's `ChecksSet`), one that asks keeps its own; `:checks` runs the same
path.

**The policy's names** (#390, on lonca#203 → lonca#210: `status.policy_checks`, the
sorted names of `verification.checks` in the policy in force, `null` when it can not be
read; names only, never a command; protocol 1, a field added, in Lonca after 0.2.0).
`lonca::Status.policy_checks` (`policy_checks_of`: a list of check names, sorted, each
once; missing, `null` or anything else is `None`) goes with each look of a repository
that holds a change for a person into `OnLonca.policy_checks` (memory only, through
`World::store_checks`, gone with a store that is down; `World::policy_checks`). Where it
is said:
- `bus::checks_run` (an `assign`, a `post`, a plan card, a card handed again) refuses a
  name it lacks with `checks::against`: `"apii" is not a check of Lonca's policy; it has
  api, e2e. Did you mean api?` (`checks::nearest`: the fewest edits, at most `NEAR_EDITS`
  2 and fewer than the name has letters; several lacking name each nearest, `api for
  "apii"`), and a policy with none `… which names none: leave checks out, and say in the
  body what to run`. Nothing is sent, so the leader fixes it in the same turn.
- `Verdict.unknown`: the card's checks the policy no longer defines (a newer policy
  signed since the card was written); they leave `asks`, the line ends in red with
  `Lonca's policy no longer has smoke: :checks changes what the card asks for`, and the
  change is not ready (Lonca would refuse the yes).
- `:checks NAMES` refuses one the policy lacks (`checks_known`: `Lonca's policy has no
  apii; it has api, e2e · did you mean api?`, the nearest only when one is lacking), and
  the menu after `:checks ` offers the policy's names not named yet, `none` first
  (`complete::check_words`).
`None` (an older daemon, a policy Lonca can not read) leaves everything as before: the
yes judges the names. imece still drops and adds no check by itself, not even one the
policy no longer has: the verdict says it and the person or the leader changes the card.

**Points.** Growth: at most 32 names of 32 bytes a card, gone with the card's row.
Read cost: the verdict reads one card's list per frame. Concurrency: the yes is one
store job at a time, as before. Migration: an older file gets the columns empty; an
older imece refuses schema 32 as ever. Failure: a name the policy lacks stops the
yes; the person adds it with `lonca policy set` or rejects the change. Security: the
names are checked at the bus, at load and before `lonca approve`; the boundary list
gets `checks.rs`.

**Gaps.** With a daemon that does not say `policy_checks` (0.2.0 and older) a leader's
typo is still found at the yes (and fixed with `:checks`); reading `lonca.toml` by hand
is what record 122 stopped. A policy signed between a look and the yes is seen at the
next look (5 s), else at the yes. The leader's prompt does not list the policy's
names: the refusal does, when one is wrong. The leader changes a card's checks only by handing it
again, not while its change waits for the person (the person's `:checks` is there
then). The git path has no named checks (`.imece/check` is per repository). The worker
hears that the checks run, not their commands. `status.changes[].checks` (asked at
earlier approvals) is not read; `runs` already holds them.

**The demo** (record 31): every third card of the project on Lonca asks for `api`;
its change's verdict line says `the card also asks for api`, and its landing's
journal line has `✓ api` after `build` and `tests`. Its daemon's policy defines `api`
and `e2e` (`DEMO_POLICY`, said at each look, so `:checks` completes and refuses as
live); the seventh card and each ninth after also ask for `smoke`, which it dropped
(`DROPPED_EVERY`, `DROPPED_FROM`): the verdict says so, and a yes is refused with
`denied: unknown_check:smoke` (`Sim::unknown_refused`) until `:checks api`. What can
not be faked: a real `--also`, and the bus's refusal (the demo's leader calls no
tool).
