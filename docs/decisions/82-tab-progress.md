# The tab's progress and the ladder in the title

Part 2 of the motion and signals plan (#200; the cursor and synchronized
output were part 1, the record on the terminal's cursor). What the
terminal draws by itself, so it costs no frame: a tab in the background,
or the task bar, says whether anything in imece is stuck, waits for the
person, or works.

**What the ladder's top is.** `App::progress() -> screen::Progress`
(`Calm < Working < Waiting < Stuck`): the highest rung (`App::rungs`,
the state ladder record) over the agents of projects that are not closed
(`World::put_away`; an archived project is closed too), so a blocked
agent of a project put away does not keep the tab red. Waiting is also
anything in `World::open_inbox`, as the title counts it, so the count and
the glyph never disagree. Starting counts as working. Done, not seen, is
calm: it is news for the screen, not a reason to look at a tab.

**The title.** `words::title(lang, waiting, progress)` puts the glyph
first, in the glyphs of the header's fleet and the rows: `✕ ` stuck, `! `
waiting, `● ` working, none when calm (`● imece`, `! imece · 2 waiting`,
`✕ imece · 1 waiting`). The glyph is the same in both languages. The
title is still written only when it changes (`screen::Title`); work
starting and ending changes it, at most once per change of the ladder's
top, not per agent.

**The tab's progress (OSC 9;4).** ConEmu's sequence, shown by Windows
Terminal (the task bar too), Ghostty 1.2 and later on the tab, iTerm2
3.6.6 and later. `Progress::osc`: calm `9;4;0` (no bar), working `9;4;3`
(indeterminate), waiting `9;4;4;100` (paused, a full yellow bar), stuck
`9;4;2;100` (error, a full red bar; the 100 so a terminal that draws the
error at its value draws it whole). Only the ladder, never the pot (the
person's decision in the design talk). `screen::Tab` writes on a change
only and forgets after a `!` command; `give_back` (the end, a panic, the
`!` command's step aside) clears it with `9;4;0`, but only on a terminal
that was sent it.

**Only an allow-list gets it.** OSC 9 is a notice in older terminals
(iTerm2 before 3.6.6, and others): `4;3` would pop up as a notice every
time work starts. `screen::shows_progress(env)`: `IMECE_PROGRESS=on|off`
says it outright (over everything, tmux included); else never inside tmux
(`TMUX`: tmux does not pass an unknown OSC on without passthrough, the
plan's part 3); Windows Terminal by `WT_SESSION`; else `TERM_PROGRAM`
with `TERM_PROGRAM_VERSION`, or when there is none `LC_TERMINAL` with
`LC_TERMINAL_VERSION` (iTerm2 sends these over ssh): `ghostty` from
1.2.0, `iTerm.app`/`iTerm2` from 3.6.6. A version is its first three
numbers (`screen::version_of`: `1.2.0-main+abc` is 1.2.0, `3.6.6beta1`
3.6.6, a word ends it; none when it does not start with a number, and
then no bar). Read once at start; the demo goes through the same loop,
so it shows the same bar and title.

**Points.** Read cost: `App::progress` reads every agent's rung and the
open inbox once per turn of the main loop (100 ms live), as the title's
count already read the open inbox; at ten times the agents this is still
a few thousand comparisons a second, and the inbox scan is the part that
grows with a long run (the same as the title's count before). Nothing is
kept but the last state written. Failure: a terminal on the list that
does not show it ignores it; one not on the list never sees it.

**Not done.** None of the terminals were probed here: the list is from
their documentation (Windows Terminal, Ghostty 1.2's release notes,
iTerm2 3.6.6 per the plan's sources). WezTerm and kitty are left out
until checked; ConEmu (`ConEmuANSI`) too. tmux passthrough is part 3 of
the plan. The bar does not show the pot.

**Changed by record 83 (notice styles).** The allow-list goes by the
terminal's own answer to XTVERSION first (`screen::Who`, asked by
`probe`): a kitty started from an iTerm2 shell inherited
`TERM_PROGRAM=iTerm.app` (seen in the probe of 2026-11-02), so the
environment alone could send the bar to a terminal not on the list. Now
kitty from 0.39 is on it (its changelog: 0.38 stopped taking `9;4` for a
notice, 0.39 shows it), and a terminal that answered with another name is
not; the environment is read only when nothing answered. Inside tmux the
bar is sent plainly: tmux 3.8 shows it again to Ghostty and iTerm2, which
it knows by their XTVERSION (`terminal-features` `progressbar`; probed on
a pty answering as each), and drops it for kitty and WezTerm, as an older
tmux drops any OSC it does not know.
