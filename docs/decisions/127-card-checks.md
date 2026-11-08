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
(`checks go with an assign: …`), on a card handed again (`a card handed again keeps
its own checks; …`), and on a project not on Lonca (`bus::checks_run`: nothing would
run them; deny by default). Whether the policy defines a name is Lonca's to say at the
yes: imece does not read `[verification]` (record 122 stopped that). `Card.checks`
(given when the card is made, never changed) and `Mail.checks` (an assign's until it
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

**Points.** Growth: at most 32 names of 32 bytes a card, gone with the card's row.
Read cost: the verdict reads one card's list per frame. Concurrency: the yes is one
store job at a time, as before. Migration: an older file gets the columns empty; an
older imece refuses schema 32 as ever. Failure: a name the policy lacks stops the
yes; the person adds it with `lonca policy set` or rejects the change. Security: the
names are checked at the bus, at load and before `lonca approve`; the boundary list
gets `checks.rs`.

**Gaps.** A card's checks cannot be changed (withdraw it and post another). imece does
not check a name against the policy before the yes, so a leader's typo is found only
then. The git path has no named checks (`.imece/check` is per repository). The worker
hears that the checks run, not their commands. `status.changes[].checks` (asked at
earlier approvals) is not read; `runs` already holds them.

**The demo** (record 31): every third card of the project on Lonca asks for `api`;
its change's verdict line says `the card also asks for api`, and its landing's
journal line has `✓ api` after `build` and `tests`. What can not be faked: a real
`--also`.
