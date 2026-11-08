---
title: Commands
description: Every `:` command imece takes, with its arguments, the key it mirrors, its short name, whether it is demo-only, whether `$` marks it as raising the next bill, and what its menu note shows.
---

# Commands

A line that starts with `:` is a command to imece, not a message to an agent. You type one in the command line (`:` opens it), `tab` offers the matches with their values, `enter` runs it. Every key that *acts* (not the ones that only move) has a `:` name too, so anything can be done from anywhere by typing it; the key handlers and the commands call the same code, so they cannot drift apart.

## How the `:` line works

- `:` then `tab` lists the commands. The menu shows after each command what it stands at now or would act on, in brackets: `budget (spent $3.10 of $20.00)`, `theme (dark)`, `close (kule)`, `forget (claude-2)`, `next (2 waiting)`, `needs (off)`. The note is drawn, never inserted.
- `$` after a command marks one that raises the next bill: `:open` (a new leader session), `:solo` (a new session), `:forget` (a new session that pays the whole briefing again, without the cache), `:commit` and `:review` (paid calls). No `:` command starts a paid turn by itself; `:budget` shows its value instead of a mark.
- Short names: `:b` budget, `:c` close, `:f` forget, `:o` open, `:q` quit, `:t` theme. A short name typed whole is a command already, so the menu offers nothing and `enter` runs it (`:q` then `enter` asks at once).
- Arguments complete too: after `:lang` the languages, after `:notify` its four words, after `:motion` its three, after `:review` and `:plan` `on off`, after `:away` `15m 30m 1h off`, after `:clear` and `:unclear` `inbox journal`, after `:theme` the built-in names, after `:close` and `:pin` the open projects, after `:restore` the archives' titles (newest first, each noted with its project's name and the day it was archived), after `:goto` nothing (the `&` line has its own menu), after `:history` the lines sent, after `:forget`, `:talk`, `:trace` and `:peek` the agents of open projects (`@` or not), after `:open` and `:export` directories (`~` for your home, hidden ones only after a `.`, `tab` steps into the chosen one).
- A name imece does not know: `no command xyz; : and tab list them`. A demo-only command typed live: `:pause is for the demo: live agents run in real time`.
- `!cmd` runs a command in your own shell and `/cmd` is the agent's own command line; neither is a `:` command ([command line](../screen/command-line.md)).

## All commands

| Command | Key | Short | Demo only | `$` | Menu note |
| --- | --- | --- | --- | --- | --- |
| `:archive [TITLE]` | — | | | | |
| `:archives` | `A` | | | | |
| `:allow [PROGRAM [WORD]]` | `A` (on an ask) | | | | the project's yes from now on, when there is one |
| `:awake` | — | | | | on / off |
| `:away [TIME\|off]` | — | | | | `30m` / `1h` / `off` |
| `:board` | — | | | | |
| `:budget USD` | — | `:b` | | | spent X of Y |
| `:checks [NAMES\|none]` | — | | | | the checks the yes to the change at hand asks for |
| `:clear [inbox\|journal]` | — | | | | |
| `:close [NAME]` | — | `:c` | | | the project at hand |
| `:code` | — | | | | |
| `:commit` | — | | | `$` | |
| `:copy [PATH]` | — | | | | the list, when there is one |
| `:defer` | `S` | | | | |
| `:digest` | — | | | | |
| `:edit [TEXT]` | `^x^e` (writing) | | | | the editor that opens |
| `:effort LEVEL` | — | | | | the solo at hand's effort, or `default` |
| `:export [PATH]` | — | | | | `the conversation` or `#12` |
| `:faster` | `+` | | yes | | the speed, `×2` |
| `:fold` | `zM` | | | | |
| `:forget [@NAME]` | — | `:f` | | `$` | the agent at hand |
| `:fork [@NAME]` | — | | | `$` | the agent at hand |
| `:goto [NAME]` | `&` | | | | |
| `:history [WORDS]` | `^r` | | | | |
| `:help` | `?` | | | | |
| `:keys` | — | | | | how many keys your file gives |
| `:lang [en\|tr]` | `L` | | | | `English` / `Türkçe` |
| `:lonca [NAME\|off\|login\|import [NAME]]` | — | | | | the project's repository, when there is one |
| `:mark` | `m` | | | | |
| `:model NAME` | — | | | | the solo at hand's model, or `default` |
| `:motion [off\|calm\|full]` | — | | | | off / calm / full |
| `:news` | — | | | | this version |
| `:next` | `W` | | | | N waiting |
| `:notify [off\|bell\|text\|push]` | — | | | | off / bell / text / push |
| `:now` | `U` | | | | |
| `:open PATH [NAME]` | — | `:o` | | `$` | |
| `:output` | `O` | | | | the command whose output it quotes |
| `:pause` | `space` | | yes | | on / off |
| `:peek [@NAME]` | `K` | | | | the agent at hand |
| `:pending` | `q s` | | | | |
| `:pin [NAME]` | — | | | | the project at hand, `▴` when pinned |
| `:plan [on\|off]` | — | | | | on / off |
| `:quit` | `q` | `:q` | | | |
| `:recall [WORDS]` | `H` | | | | |
| `:rename NAME` | — | | | | the project at hand |
| `:restore [NAME]` | — | | | | |
| `:review [on\|off]` | `R` (in a change: whole) | | | `$` | on / off |
| `:salon` | — | | | | |
| `:settle` | `s` | | | | the selected row's agent or project |
| `:slower` | `-` | | yes | | the speed, `×2` |
| `:solo [PATH] [CHOICE] [NAME]` | — | | | `$` | |
| `:story` | — | | | | on / off |
| `:sync` | — | | | | how far behind trunk (`↓3`) |
| `:talk [@NAME]` | `v` | | | | the agent at hand |
| `:theme [NAME\|FILE]` | `T` | `:t` | | | the theme in use |
| `:tidy` | — | | | | |
| `:timeline` | — | | | | |
| `:home` | — | | | | |
| `:journal` | `J` | | | | |
| `:trace [@NAME]` | `t` | | | | the agent at hand |
| `:unclear [inbox\|journal]` | — | | | | |
| `:undo` | `u` | | | | |
| `:unfold` | `zR` | | | | |
| `:unsettle` | — | | | | how many are hidden |
| `:waiting` | — | | | | N waiting |
| `:welcome` | — | | | | |
| `:why [@NAME]` | — | | | | the agent at hand |
| `:yes [@NAME]` | `e` | | | | the agent at hand |

Live, `:pause`, `:faster` and `:slower` are left out of the menu. Keys that move (`j k`, `gg G`, `enter`, `esc`, `tab`) and keys that act on the selected row (`a r c n x`) have no `:` name: typing them would be slower than the key, and the row would have to be named.

## The commands

### `:archive [TITLE]`

Puts the project at hand away as `TITLE` and opens a fresh one in its place, ten seconds after it is asked: the bottom line counts down (`◷ archiving kule as “first epic” in 7 s · u keeps it`, and when agents of it are mid-turn `…, cutting 2 turns short …`), and `u` ([`:undo`](#undo)) leaves it as it is. Without a title it is called by its name and its days (`kule 2026-10-30 → 2026-11-02`); a title is one line, at most 60 characters.

The archived project's agents stop as at a quit (a turn under way is cut short and its prompt kept) and sleep with all they had: their sessions, mail, notes, changes, cards and items in TO DECIDE. All of it leaves the screen: the team, TO DECIDE, the journal, the board and the queue show only the fresh project, which has the same name, pin and `:copy` list, and a head of the same name and model. Nothing is spent, and the pot is the same one. The flash: `archived kule as “first epic”: its agents sleep with all they had; a fresh one opens`; the journal: `you archived kule as “first epic”; a fresh kule takes its place`. `--fresh` leaves an archive's sessions alone. The demo archives the same way. See [sessions](../concepts/sessions.md#archives) and the scenario [a new epic](../scenarios/new-epic.md).

### `:archives`

Opens the archives as a tree, as `A`: each checkout that has one (the one at hand first, then by its newest archive), under it the projects open on it now (`● kule · open now · 1 agents`), then its archives, newest first, each with what its agents spent, its cards done, its agents and the day it was archived (`▸ first epic · $4.20 · 3 cards done · 4 agents · archived 2026-11-02`). Nothing in it wakes or is spent.

In it `j` `k` move, `gg` `G` go to the first / last row, `l` (or `→`) shows an archive's agents (each with what it spent, its head first and in bold) and opens a folded checkout, `h` (or `←`) folds them again (on an archive already folded, its checkout), `za` folds or unfolds the row at hand, and so does `enter` on a checkout. `/` filters: only the archives with every word typed, in their title, project name, checkout or an agent's name, case and Turkish letters folded; `enter` keeps the filter, `esc` drops it. `r` on an archive or one of its agents brings it back, as [`:restore`](#restore-name) does, and closes the window; `esc`, `q` or `A` close it (`esc` drops a filter first). With nothing archived: `nothing is archived yet; :archive puts the project at hand away`. An `:archive` still counting down acts first, so it is listed. The demo lists its archives the same way, each under its project's name.

`enter` on an archive (or one of its agents, then selected) opens it read-only over the screen, without bringing it back: the team, the board, TO DECIDE and the journal show that archive alone, as it was put away, its journal all the store keeps of it. The top row says which it is and what waits for you meanwhile: `ARCHIVE · kule “v1 fixes” · archived 2026-11-02 · 1 waiting for you now · esc back`. Nothing in it wakes or spends, and the screen behind goes on. Conversations (`v` copies), the card screens, the focus, `:export` and the `:` commands that only look or go work there; a message, `!cmd`, `m`, `A`, an answer in TO DECIDE and every other command say `an archive is read-only · :restore brings it back · esc goes back`. `:restore` in it brings that archive back. `esc` or `q` (with nothing open in the view) goes back, and so does `:quit`; `ctrl-c` quits imece. If the store cannot be read: `the archive's journal could not be read: …`.

### `:awake`

Turns keeping the machine awake on or off (the same as `--awake`). While it is on and any agent is mid-turn, imece runs `caffeinate` (macOS) or `systemd-inhibit` (Linux) as a child and lets it go when nothing is mid-turn. The flash says `the machine stays awake while an agent is mid-turn` or `the machine may sleep again`; a tool that cannot start is said once: `could not keep the machine awake: …`. Not remembered between runs.

### `:away [TIME|off]`

How long you are away before WHILE YOU WERE AWAY opens by itself: back in the terminal after that long in another window, or the first key after that long without one. Minutes (`45m`, or `45`) or hours (`2h`), from one minute to a day; `off` never opens it (`:digest` still does). Half an hour by default. Bare `:away` goes to the next one round: 15m, 30m, 1h, off; a time of your own goes to the next longer one. The flash: `back after 30m or more away, what happened shows at once`, `what happened never shows by itself; :digest shows it`; another word: `3s is not off, minutes (30m) or hours (1h), up to a day`. The menu note shows the value; it is remembered in the store. See [journal](../screen/journal.md#while-you-were-away).

### `:board`

The board, in a window over the screen; `esc` closes it. See [board and code map](../screen/board-and-code-map.md).

### `:budget USD`

Raises or lowers the pot while agents run (`:b 80`). The value is dollars with at most two decimals, up to four digits before the point (`80`, `2.50`); anything else: `80$ is not dollars like 80 or 2.50`. The flash says `the budget is $80.00 now`, the journal `you set the budget to $80.00 (was $50.00)`, and the warning that the pot runs low may sound again. A turn under way finishes on its old cap; the next starts on the new remainder, in a new session if the old one had hit its cap. The pot is one for all projects and is kept between runs. The menu note: `(spent $3.10 of $20.00)`. See [budget and limits](../concepts/budget-and-limits.md).

### `:checks [NAMES|none]`

On a project on Lonca, the checks of the repository's policy that your yes to the change at hand asks Lonca's merge queue for: its card's (the change open, else the one `W` opened, else Home's row). `:checks api e2e` (commas or spaces between) sets them, sorted and each once; `:checks none` takes them away; bare it says them: `the yes to claude-1's change also asks Lonca for api, e2e`. The journal and the card's story say `you set #3's checks: api, e2e`, and the verdict line follows. Refused, the list kept: `the checks stay as they were: no change at hand: open one, or select it on Home`, `… kule is not on Lonca, and only Lonca's merge queue runs a card's checks`, `… claude-1's change was made for no card`, `… Build is not a check's name: a-z, 0-9, _ and -, at most 32, and not one of Lonca's own`, `… a card asks for at most 32 checks`. The menu note: the card's checks, or `none`. See [the checks a card asks for](../concepts/messages-and-board.md#the-checks-a-card-asks-for).

### `:clear [inbox|journal]`

Takes what TO DECIDE lists and the journal's lines off the team's screen: bare both, `inbox` or `journal` one. Nothing is deleted: the store keeps it all, and `:export`, a card's screen and the archives still show it. It waits ten seconds first, as `:close` does (`◷ clearing TO DECIDE and the journal off the screen in 7 s · u keeps them`; `u` leaves the screen as it is), then says what went: `2 waiting and 40 journal lines off the screen; :unclear brings them back`. What still waits for you is not lost: it stays open, its agent still waits, the title counts it (`TO DECIDE (0 · +2 hidden)`), the terminal's title and tab count it too, and `W` opens the next one and puts it back on the list. A marked line (`m`) stays in the journal. What comes after shows as usual. The clear is kept in the store, so the next start begins clean too. With nothing to clear it says `nothing on the screen to clear`; another word: `all is not inbox or journal`. See [journal](../screen/journal.md#clearing-the-screen).

### `:close [NAME]`

Closes the project `NAME`, or the one at hand (`:c`), ten seconds after it is asked: the bottom line counts down, and `u` ([`:undo`](#undo)) keeps it open. Its agents' sessions and runs end, their mail, notes, offers and changes are dropped, their items in TO DECIDE close, and the project is marked closed: the team skips it, the same checkout opened again takes its place with a new leader. The worktrees stay on disk. The flash: `closed kule: its agents left; their worktrees stay on disk`. The last open project refuses: `kule is the last open project; q q quits`; a name nobody has: `no open project named x`. In the demo, the project's agents leave the same way. See [projects](../concepts/projects.md).

### `:code`

The code map, in a window over the screen; `esc` closes it. Live it says there is no shared trunk yet (each agent works in its own worktree, so there is nothing to show of who edits what); symbol leases are not built. See [board and code map](../screen/board-and-code-map.md).

### `:commit`

Asks Claude Code's cheap tier (`haiku`, or your `--tier claude:cheap=…`) for a commit message for what is staged in the project at hand's checkout, and puts it on the command line as `!git commit -m '…'` for you to edit and run with `enter` in your own shell, or drop with `esc`. imece commits nothing. The flash says `asking haiku for a commit message for what is staged in kule`; the journal says each suggestion with its model and cost, and each failure with why.

It costs: each call at most $0.05 (the call's own hard cap), a day (UTC) at most 20 calls and $1.00 over all projects, reserved from the pot before the call and hidden after. Refused with `no commit message: …` when nothing is staged, the day is full, or the pot has less than the cap. The same staged tree asked again costs nothing: the last suggestion comes back. The call reads `git diff --cached --stat`, the patch (at most 24 KiB) and the eight newest commit subjects; it runs with no tools, no MCP server, its own short system prompt, nothing saved. The demo answers from the titles of the changes applied in that project, as `git commit --dry-run`. See [own shell and commit](../scenarios/own-shell-and-commit.md).

### `:allow [PROGRAM [WORD]]`

The project at hand's yes from now on: commands its agents run without asking you (see [approvals](../concepts/approvals-and-questions.md#a-yes-from-now-on)). `:allow` alone says the list (`kule: yes from now on to make, npm run …`, or `kule: no yes from now on; A on an ask, or :allow PROGRAM [WORD]`); `:allow npm run` adds one (the project's open asks it covers go on), and the same again takes it out (`kule: no yes from now on to npm run … any more`). A program and at most one word, not an option first: `:allow rm -rf` is refused. `A` on an ask in TO DECIDE adds what that ask runs, or that line alone when no such yes can hold it; `:allow` with the line takes that one out. Kept with the project between runs.

### `:copy [PATH]`

The list of paths git ignores that a worker's start copies into its worktree (`.env`, `node_modules`, `target`): a worktree is a fresh checkout, so what git ignores is not there. `:copy PATH` adds a path of the project at hand, `:copy PATH` again drops it (a trailing `/` is the same path), `:copy` alone says the list: `copied into kule's worktrees: .env, target` or `nothing is copied into kule's worktrees; :copy PATH adds a path git ignores`.

A path must be relative, literal (no glob, no `..`, not in `.git`, at most 200 bytes), there in the checkout, not a link, inside the checkout once resolved, and ignored by git (a tracked file is in the worktree already); else `x can not be copied: …`. The copy happens at each worker's start (helpers share their worker's worktree; never for the leader), only for paths the worktree lacks, all paths of one start within 2 GiB, as an APFS clone or reflink where the volume can. The journal says what was copied and what was skipped. Kept with the project in the store. Beside the list, what the checkout's own `.worktreeinclude` (gitignore syntax) matches among the paths git ignores is copied too, said apart in the journal; `:copy` alone then ends with `; and what its .worktreeinclude names` (see [projects](../concepts/projects.md#worktreeinclude-the-checkout-names-them-itself)). See [files](files.md) and [projects](../concepts/projects.md).

### `:defer`

Puts the item at hand in TO DECIDE off and opens the next one, as `S`. See [waiting for you](../screen/waiting.md).

### `:digest`

Opens WHILE YOU WERE AWAY now: what happened since you last left the terminal's window this run, or since the run began. With nothing new: `nothing new since you last left`. Any key closes it, as when it opens by itself.

### `:edit [TEXT]`

Opens `TEXT` (nothing when bare) in your editor, as `^x ^e` does with the line you are writing: `$VISUAL`, else `$EDITOR`, else `vi`. The screen steps aside as for a `!` command, the editor runs in the project's checkout, and what you save comes back on the command line, the cursor at its end, not sent. An editor that exits with an error (vim's `:cq`), is stopped, leaves more than 1 MiB or what is not text gives the draft back as it was, and a line under what it left on the terminal says why until a key brings the screen back: `` [imece] `vim` ended with 1: the draft is kept · any key: back to the screen ``; one not found says `` no editor `nvim`: set $VISUAL or $EDITOR; the draft is kept ``. The draft is a file of its own under imece's state (only you may read it), removed when the editor closes. The menu note: the editor that opens, `(vim)`. Not in an archive's view. See [command line](../screen/command-line.md#your-editor).

### `:effort LEVEL`

Sets how deep the solo session at hand thinks, from its next turn: one of Claude Code's efforts (`low`, `medium`, `high`, `xhigh`, `max`), or `default` for Claude Code's own. The solo at hand is the selected agent when it is a solo, else the project at hand's solo. Its running session ends and the next message starts a new one on the same transcript, told where it left off, so nothing is lost but the process; refused mid-turn (`could not change solo-notes: solo-notes is mid-turn; wait for its answer`), for any other level (`claude has no effort "turbo"; one of …`) and when no solo is at hand (`no solo session at hand: select one, or open one with :solo`). The flash: `solo-notes runs on claude opus@max from its next turn; its session starts again on it`; the journal: `you set solo-notes to run on claude opus@max`. The menu note shows the effort now, `(xhigh)` or `(default)`; after `:effort ` the levels complete. The model part of the choice stays. See [solo sessions](../concepts/solo.md).

### `:export [PATH]`

Writes what is open as a Markdown file: the open conversation (`# Conversation: lead ⇄ claude-1`, each message a heading with its day, time, who to whom and its type), else the card's screen (`# #12 title`, its project, column and minutes, WHO, STORY, what waits for you on it, and its journal lines, oldest first, none folded). Nothing open: `nothing to export: open a conversation (v) or a card (#N) first`.

`PATH` is a file, or a directory to name one in; none is the directory imece was started from. The name says what and when: `imece-conversation-20261031-1400.md`, `imece-card-12-20261031-1400.md`. A file that is there already is never written over; the flash says `written to …` or `could not write …: …`. Every time has its day, every line goes through the same cleaning as the screen. The menu note says what it would write: `(the conversation)` or `(#12)`.

### `:faster`

Demo only, as `+`: the simulation runs faster. The menu note shows the speed (`×2`).

### `:fold`

Folds every project and every worker's helpers in the team, as `zM`; on Home it folds the projects that head its rows and stays; from another screen it goes to the team first. With a conversation open it folds every message of it to one row instead, and the conversations you open after it in this run start folded. See [team](../screen/salon.md) and [conversations](../screen/conversations.md).

### `:forget [@NAME]`

Makes the agent forget its session (`:f`), ten seconds after it is asked: the bottom line counts down, and `u` ([`:undo`](#undo)) keeps it. Then the running session ends (its run killed, an unfinished prompt kept as interrupted, told at the wake and not run again), the remembered session goes with its transcript, the agent sleeps until spoken to, and its next message starts a new session with the full briefing. No name is the agent at hand. The flash: `claude-2 forgot its session; the next message starts a new one, told where it left off`; refused for an agent with nothing to forget or one that is gone: `claude-2 keeps its session: it has no session to forget`. The journal: `you made claude-2 forget its session; it starts a new one, told where it left off`. `--fresh` does it for every agent at the start. `$`: the new session pays the whole briefing again. See [sessions](../concepts/sessions.md).

### `:fork [@NAME]`

Makes a new worker that goes on from the worker `NAME` (else the one at hand) to try another way, while that worker goes on in its own: the new one has its conversation (a fork of its session: Claude Code's `--fork-session`) and its uncommitted change, untracked files included, in a worktree of its own made at that worker's HEAD. It is named as a hired worker is (`claude-3`), runs on the same model, counts in `--agents`, and its turns come out of the pot (Claude Code counts what the first session had cost too; imece counts only the fork's turns). Its row is selected and the command line opens with `@claude-3 `, flashing `claude-3 is a fork of claude-1: write what it should try`; what you write is its first turn, and only then does the fork happen: the session and the change are taken as they are at that moment. The journal says `claude-3 forked from claude-1: …`; its change is a Δ of its own. Refused, with why: anything but a worker, a worker with no session yet or mid-turn, a full team, a worktree that is not there, or a worker in a Lonca workspace. A session that could not be forked goes on as a new one, told where it is; the journal says why. The demo forks too, without the `--agents` cap.

### `:goto [NAME]`

Goes to the agent called `NAME` (an `@` before it is fine), else to the open project of that name: the team with its row selected, the project unfolded, a helper's worker unfolded, and `f` let go when it would hide the row; a project's name selects its header. Case and Turkish letters do not matter. Nothing of the name: `nothing called "zzz" to go to: an agent or a project`, and nothing moves. Alone it opens the `&` line: the places as they stand now, what needs you first (stuck, then waiting, then done and not seen), the rest by their state (working before idle), then the open projects, each agent with its glyph and status words. The list is frozen while the line is open, so its numbers hold while you type; a digit `1`-`9` right after `&` goes at once, `#` right after `&` turns to the card line, `!` keeps only what needs you, digits match the start of a number, anything else matches inside the names. `enter` goes to the item under the cursor, or to the name typed. See [team](../screen/salon.md).

### `:history [WORDS]`

Searches the lines you sent, as `^r` does: the command line opens with `WORDS` on it and the completion menu lists the lines sent that have every one of them (case and Turkish letters aside), newest first, each line once with how long ago it was sent; nothing typed lists them all. `tab`, `↑` and `↓` step through them, `enter` puts the highlighted line on the command line, whole (a line of several lines comes back as it was), to send with `enter` again or to change first; with nothing matched, `enter` keeps the words and ends the search. `esc` closes the menu, then the line. While writing, `^r` takes what is written as the words. Nothing sent yet: `nothing sent yet to bring back`. Every line sent is kept (a message, a `:` command, a `!cmd`; not `#12`, `&3`, `:goto` and `:history`, which only go somewhere), between runs too: the newest 500 come back at the next start. `↑` and `↓` at the text's first or last line step through the same lines without a search. See [keyboard](../screen/keyboard.md) and [files](files.md).

### `:help`

The `?` help, filtered to the screen you are on (`?` in it: every key); the legend of glyphs; `/` in it searches every key by every word. See [keyboard](../screen/keyboard.md).

### `:journal`

The journal in a window over the screen, as `J`: its lines newest first, or the story by card while that is on (`J` in the window turns it); `j` `k` `gg` `G` and the pages move, `m` marks, `'` goes to the next mark, `c` copies, `enter` opens the line's conversation in the window's place, `esc` or `q` closes it. See [the journal](../screen/journal.md).

### `:keys`

Reads your key file (`~/.config/imece/keys`) again and says what came of it: `…/imece/keys: keys taken (5)`, with `; nothing does j now` when a key of imece's lost its meaning to no other key; `no key file; KEY = KEY lines in … give you keys of your own` (imece's own keys are back); or why the file was refused, the keys you had staying: `…/imece/keys: line 3: write it as KEY = KEY; the keys you had stay`. The menu note is how many keys the file gives. imece also reads the file by itself when it changes, with the same words, so `:keys` is only needed to hear them again. See [your own keys](../screen/keyboard.md#your-own-keys).

### `:lang [en|tr]`

The screen's language, as `L`. Alone it switches to the other language; `:lang en` or `:lang tr` sets one. Anything else: `de is not a language: en or tr`. The menu note shows the language in use. For this run only; the next start reads `--lang` or `IMECE_LANG`.

### `:lonca [NAME|off|login|import]`

The [Lonca](https://github.com/selcukgural/lonca) repository of the project at hand. `:lonca NAME` names it (a name is 1 to 64 letters, digits, `.`, `_`, `-`) and asks the daemon at once: `kule is on Lonca repository kule now; imece asks its daemon`. `:lonca off` leaves it. Bare, it opens the Lonca window: how the project stands at the top (`kule is on Lonca repository kule: trunk 693d629, 2 waiting for a human`, or why it can not be read), then `WAITING FOR A HUMAN · 2`, a row for every proposal waiting in the repository as the last look read it, whoever made it: its number, who (a worker of the project by its name here, anyone else as Lonca names them), the paths it changes (three and the rest counted), `Δ shown` when it is a worker's change waiting for you, `based before trunk` when trunk moved on since its base; then `HELD · N`, every symbol held (file, symbol, holder). `j` `k` `g` `G` move over the proposals, `enter` on one shown as a Δ opens that change (on Home it becomes the row at hand too; another says `proposal 12 is no change shown here: nobody's of this project, or its worker's turn goes on`), `esc` or `q` closes, `?` the help. Off Lonca a flash says so: `kule is on no Lonca repository; :lonca NAME names one, :lonca import makes one`. The repository is kept in the store per project; a project archived hands it to the fresh one. The demo's second project shows the window with a stranger's proposal on its table; its daemon lists no held symbols.

`:lonca login` signs you in: imece runs `lonca human login` once, while you go on (with a Secure Enclave key it waits for your finger), and the journal says `signed in to Lonca as NAME` or why not. A session lasts a day at most.

`:lonca import [NAME]` makes a repository of the checkout, as NAME or the project's name. A window shows the checkout, the branch and its commit and the repository it becomes; `a` imports, `esc` or `q` closes it and nothing goes, `?` the help. imece can not take it back. It runs on the side and the journal says how it went, with the first thing Lonca warned of. The repository starts from your Lonca install's own profile, which imece never weakens: on a hardened install the journal says so, and a key that can not prove presence is refused in Lonca's words. Refused at once: a project already on a repository, a folder without git, a detached HEAD, a `lonca` command under way. On a new daemon the first import signs you in too, since a daemon with no repository can sign nobody in. It signs the repository's policy as well, so a worker's change to a critical path waits for you instead of being refused; if that does not go through the journal says so and the import stands. `:lonca login` before it shows the daemon's refusal.

While a project on Lonca has not had an answer from its daemon it is closed: nobody is hired, helpers included, `:fork` is refused and no Δ is applied, rejected or sent back. Its line says `⌀ no lonca` and the journal says why once. A daemon that is only busy and asks to be asked again closes nothing until it has said so three looks in a row (about 15 seconds). A repository Lonca froze (by hand, or on an integrity alarm) is closed the same way until a person unfreezes it; `lonca status` says why. See [projects](../concepts/projects.md#on-lonca). So is one whose policy would let changes in unseen (`unobserved_is_tainted = false`). Workers hired from then on work in Lonca workspaces, and what they propose is their Δ; a worker already in a worktree stays there, and its Δ lands in your checkout. The demo's second project is on Lonca, and its daemon stops now and then; `:lonca import` and `:lonca login` work on its table, and a name already there is refused as a daemon refuses it.

### `:mark`

Marks the line at hand to come back to, as `m`: in the open conversation, else in the journal; again unmarks. A marked line shows `✱`, is kept between runs and is never folded, tidied or left out of a load; `'` goes to the next marked line. The flash: `marked: ' comes back to it`. See [journal](../screen/journal.md).

### `:model NAME`

Sets the model the solo session at hand runs on, from its next turn: an alias (`opus`, `sonnet`, `haiku`, `fable`) or a full name as Claude Code takes it, or `default` for its own. The same rules, refusals, flash and journal line as `:effort`; a name that is not a model name (`a b`, `-x`) is refused. After `:model ` the aliases complete; a full name is typed. The effort part of the choice stays.

### `:motion [off|calm|full]`

How much moves on the screen. `calm` (the default): a working row's glyph breathes, its colour dimmed every other 0.6 s, never its shape (on the board too, a working owner's under its card); `starting` counts its dots; the bottom line shows a spinner while imece waits for an answer (`:commit`), and so does Claude Code while it is still being checked in the first-run window; a row or an item that comes to wait for you blinks three times; a row whose turn is done, or whose session started, hides from a green background to its own colours (a theme of the terminal's own colours: reversed, then the changed rows' look); a row wears its activity bar, `▁▃▅▂▁`, what the agent did in each of the last five ticks (it changes only with what the agent does and as the ticks pass); imece's mark, where it comes on the screen (the welcome and news windows, a team with no worker yet), brings its dots one a beat, any key showing it whole. `full` adds a spinner by the header's fleet while anyone works, and spins faster; over SSH it runs as calm (every frame crosses the network). `off`: nothing moves, no row wears an activity bar and the mark is drawn whole, the screen as it was before. Nothing moves in another window, and what happens there does not blink when you come back; everything falls on one beat (0.3 s), so several things moving cost no more frames than one, and a frame is drawn only when what it shows moves (see [the decision](https://github.com/selcukgural/imece/blob/main/docs/decisions/84-motion.md)). Bare `:motion` goes to the next one round: off, calm, full, off. The flash says what the new one does (`calm motion: a working row breathes, what comes to wait blinks`, `full motion: spinners too, and faster`, `full motion runs as calm over SSH`, `nothing moves on the screen`); another word: `wild is not off, calm or full`. The menu note shows the value; it is remembered in the store.

### `:news`

What is new in the latest versions (three at most), newest first, in a window any key closes. The first start of a new version shows it once by itself, with what is new since the version that started before. The menu note is this build's version. See [first run](../getting-started/first-run.md#whats-new).

### `:next`

Opens the next item waiting for you from anywhere, as `W`. The menu note says how many wait: `(2 waiting)`.

### `:notify [off|bell|text|push]`

How you are told, in another window, that something came to wait for you: `bell` rings the terminal's bell (every terminal has a setting for it: a sound, a badge on the tab, a flash; tmux's `monitor-bell`), `text` sends a desktop notification with imece's words, the way your terminal shows it best (it is asked its name at start): kitty 0.36+ gets its own (OSC 99) with `imece` as the title, louder and with an error sound for something stuck or an API error, a question sound otherwise, and **clicking it opens what it was about** (the item that waits, the stuck agent, or for several the next item waiting, as `W`); Ghostty gets OSC 777 with `imece` as the title; inside tmux it goes through tmux's passthrough, which needs `set -g allow-passthrough on` in your tmux config (else tmux drops it; `bell` always gets through); any other terminal gets OSC 9 (iTerm2, WezTerm; one that does not know the sequence ignores it, so choose `bell` there), `off` (the default) neither. Bare `:notify` goes to the next one round: off, bell, text, off. The flash says `the bell rings when something waits and you are in another window`, `a desktop note when something waits and you are in another window` or `no word in another window`; another word: `beep is not off, bell, text or push`. The menu note shows the value; it is remembered in the store.

What rings: a new item in TO DECIDE (a change, a question, an answer, an ask for your yes, something sensitive) or an agent that got stuck (blocked, its session stopped), when it came while the terminal did not have the focus and is still there five seconds later, once each. A note names the kind and the agent, never the agent's words: `imece · claude-1 finished: a change waits for you`, `imece · claude-2 asks you`, `imece · claude-3 is stuck`; several due at once are one bell, or one note with the count (`imece · 3 things wait for you`). What was on screen when you left never rings, and back in the window what was about to ring is dropped: it is on screen now. The terminal must report focus (tmux needs `focus-events on`); one that does not never rings. See [TO DECIDE](../screen/waiting.md).

`push` sends what comes to wait to your phone through [ntfy](https://ntfy.sh) instead, for when you are away from the machine: it is said when the terminal lost the focus, or when no key was pressed for `:away`'s time (30 minutes by default; `:away off` leaves only the focus), since a terminal left on the desk keeps its focus. It is taken only by name (never in the bare round), and only when your settings hold: a file `push` in imece's config directory (`$XDG_CONFIG_HOME/imece/push`, else `~/.config/imece/push`):

```text
# where pushes go
server = https://ntfy.sh
topic = imece-alerts-7f3k9q2m
# token = ~/.config/imece/ntfy-token
```

`server` is the public `https://ntfy.sh` when left out; one of your own must be `https` (`http` only on this machine), without a user, a query or a fragment. `topic` is letters, digits, `-` and `_`, at most 64; on the public server anyone who knows a topic reads it, so one shorter than 16 or without a digit is refused. `token` names a file holding the token your own server wants: a plain file of yours that no one else may read or write (`chmod 600`), one word; it is sent as an `Authorization` header, never in the URL, on a command line or in a log. A push is `curl` (it must be installed) reading where, the words and the token on its stdin, nothing of your own `.curlrc`, no redirect followed, ten seconds at most. It says what a note says and the project, never an agent's words or a command: `imece · claude-2 asks you · shop`, `imece · 3 things wait for you`; something stuck goes at high priority. Each thing is pushed once; the journal says each push (`pushed to your phone: …`); one that fails is said once (`the push to your phone did not go: …`) and not tried again, and the next thing that waits is pushed as its own. The flash says `a push to your phone through https://ntfy.sh when something waits and you are away`, or what is wrong with the settings (`push settings: on ntfy.sh a topic of at least 16 with a digit, …`); the settings are read again at each start, and when they no longer hold nothing is pushed (`:notify off`). The demo reads no settings and sends nothing: each push is a journal line, `a push to your phone would say (the demo sends nothing): …`.

### `:now`

Does the `:close`, `:forget`, `:archive` or `:clear` counting down at once, as `U`, instead of after its ten seconds: the same as when its time is up. The bottom line says so while it counts (`◷ closing kule in 7 s · u keeps it open · U now`); with nothing counting down it says `nothing counts down`. `u` ([`:undo`](#undo)) is still the way back. Both the demo and live sessions.

### `:open PATH [NAME]`

Opens the git checkout at `PATH` as a project of its own (`:o`), with a new leader named `lead-NAME` (same model and tiers). `NAME` defaults to the directory's name made safe: 1-24 of letters, digits, `.`, `_`, `-` (the rest become `-`); a wrong name is refused: `"x y" can not name a project: 1-24 of letters, digits, . _ -`. `~` stands for your home; a path with spaces is not parsed. The flash: `opened kule at /Users/me/src/kule; write to lead-kule`. Refusals: `:open PATH [NAME] opens the git checkout at PATH as a project` (no path), `a project named kule is open already`, `kule is open already, as k2`, `could not open: …` (not a checkout). A closed project with the same checkout opens again in its place. Opened projects are remembered between runs; a second imece on another checkout is told to use `:open` from the first. `$`: a new leader session. The demo adds a fake project with a leader, two workers and three cards. See [projects](../concepts/projects.md). A folder that is not a git checkout with a commit is refused: `… is not a git checkout with a commit: a team works in git worktrees …`; `:solo` opens there.

### `:output`

Quotes the last `!cmd` of the project at hand in a message, as `O`: the command line opens to the agent at hand (else the project's head) with how the command ran and the last 40 lines it showed, cleaned, in a `text` fence, under the line you write on. Nothing is sent until you press `enter`. The output is kept in memory only, the newest per project. None yet: `no !command has run in this project yet`. The menu note: the command, `(cargo test)`. See [command line](../screen/command-line.md).

### `:pause`

Demo only, as `space`: pauses or resumes the simulation. The menu note: `(on)` / `(off)`.

### `:peek [@NAME]`

The agent's focus in a window over the view, as `K`; no name is the agent at hand (selected in the team, behind the inbox item or the journal line, or the one zoomed in on). The view under it does not move. In the window `j` `k` (or the arrows) step to the next / previous agent in the team's order, `enter` opens its whole focus, `@` starts a message to it, `?` the help, `esc` or `K` close it; other keys do nothing. Looking at an agent this way counts as seeing its finished turn. See [team](../screen/salon.md).

### `:pending`

Before quitting, the list `q s` shows: the agents mid-turn and the changes not reviewed; `q` there quits, `W` opens the next waiting.

### `:pin [NAME]`

Pins the project `NAME`, or the one at hand, to the top of the team; again unpins. Pinned projects come first, in the order they were opened, and fold last when the team is short; a pinned header wears `▴`. The flash: `kule is pinned to the top of the team` / `kule is not pinned any more`. Kept in the store; a closed project keeps its pin for when it opens again. The menu note shows the project it would act on, `▴ kule` when pinned already. A colour per project is not built.

### `:plan [on|off]`

The plan gate of the project at hand ([the plan](../concepts/messages-and-board.md#the-plan)): `on` (the default) has the leader put its plan before you, as a `▤` item in TO DECIDE, before it hires a second worker; `off` lets it hire as it sees fit, and a plan it still sends is answered `the person turned the plan gate off for this project` and not kept. Bare, it says where the gate stands: `plan gate on for kule: a plan with your yes before the second worker` / `plan gate off for kule: the leader hires as it sees fit`. Kept in the store per project. The menu note shows on / off.

### `:quit`

Quits, as `q` (`:q`): it asks first (`quit imece? q again to quit · any other key to stay`), counting the agents mid-turn and the changes not reviewed when there are any; `s` lists them.

### `:recall [WORDS]`

Searches what everyone did, as `H` does: every journal line and every trace line imece keeps, of every project and archive, earlier runs too. The command line opens with `WORDS` on it (lead `› recall:`) and the completion menu lists the lines that have every word typed at the start of one of their words (case and Turkish letters aside: `deploy fix` finds `fixed the deploy script`, `ploy` does not), the newest first, at most 50, each on one line as the journal words it, noted with the agent, the project and when (the clock today, the date before). A journal line is found by the texts it carries (a message's first line, a reason, a command, a path) and its agent's name; a trace line by itself and its agent's name. `tab`, `↑` and `↓` step; `enter` goes to the highlighted line: a journal line's conversation, the line selected in the journal; a line of an archive in the archive's read-only view; a trace line's agent, its raw stream open. A line the screen does not hold (older than the journal lines a start loads, or a closed project's trace) is said instead: `that line is not on the screen: …`. Nothing found: `no journal or trace line has WORDS`. `esc` closes the menu, then the line. Not in an archive's view, which looks at its archive only. The demo searches what it holds in memory. Kept in the store's full-text index ([files](files.md)), which forgets a line when the line goes (90 days, 200 trace lines per agent).

### `:rename NAME`

Renames the project at hand and its leader to `lead-NAME` (project 0's `lead` too). Sessions go on; every live agent of the project gets a note at its next prompt, and a `send` to the old name answers with the team list. The same name rule as `:open`. The flash: `kule is now k2; write to lead-k2`.

### `:restore [NAME]`

Brings an archive back: the newest one titled `NAME`, else the newest archive of the project called `NAME`; without a name, the newest archive of the project at hand's name (so `:restore` right after an `:archive` takes it back once the grace is over). The menu after `:restore ` offers the archives' titles, newest first, each noted with its project's name and the day it was archived (`first epic   kule · 2026-11-02`).

The archive opens again as it was put away: its agents asleep with their sessions, mail, notes and changes, its cards, items in TO DECIDE and journal lines back in sight, its head selected. Nothing wakes and nothing is spent until you write: each agent then goes on in the session it had (when Claude Code still keeps it, else a new one, told where it left off), told first that the project was put away and brought back, and of the turn the archive cut short, which is not run again.

A checkout holds one open team, so the team open on the archive's checkout gives way (for a solo archive: the open solo of its name). It is archived in its turn under its name and days when it did anything (a card, a hired agent, something spent or said, a session), or closed when it did not, so empty archives do not pile up. The flash: `brought kule “first epic” back: its agents sleep until you write to them`; the journal: `you brought back kule “first epic”; the one open there is archived as “kule 2026-11-02”` (or a `you closed kule` line before it).

Refused, with nothing changed: no archive of that title or name (`no archive is titled or named x`, `kule has no archive`), its folder is gone (`/src/kule is not there any more`), a team's folder is no git checkout with a first commit any more, or another open project has its name (`a project named kule is open already`: rename or close that one first). An archive brought back and later archived again takes a new title. The demo brings back the same way (its folders are its own, never gone). See [sessions](../concepts/sessions.md#archives) and the scenario [a new epic](../scenarios/new-epic.md).

### `:review [on|off]`

Has a fresh Claude Code session read a change and say what it finds, as a note above the change's files. Bare, it asks for the change at hand (the one open, else the one selected in TO DECIDE) now; `on` asks for one for every change as it comes to wait, `off` stops that (remembered in the store, off by default); bare with no change at hand, it says which it is. When it comes, the change's screen shows `✎ review · sonnet · $0.04` and its first six rows (`R` shows it whole); the journal says each review with its first line, and each failure with why and what it cost.

The note is advice: nothing waits for it and it never holds a change back; you still apply or reject. Who reads: a session of its own, which knows nothing of the worker's, on your `--tier standard=…`, else `sonnet`, with no tools and a hard cap of $0.10 a review. The reader sees what the worker said it did, the list of files and the patch (at most 64 KiB); nothing else. A day (UTC) holds 20 reviews and $2.00, apart from `:commit`'s, reserved from the pot before each call and hidden after; one runs at a time, the rest wait their turn. Refused with `no review: …` when no change is at hand, it has its review already, one of it is under way, the pot has less than the cap, or the day is full. A newer change of the same worker gets a review of its own. See [changes](../concepts/changes.md#a-second-reading). The demo reviews the same way, answering from the change's files a few ticks later.

### `:salon`

The team: Home's AT WORK, as `gw` (record 125: the team is Home's lower half).

### `:settle`

Hides the row at hand's agent (with its helpers and researcher) or, on a project's heading, the whole project, out of AT WORK until it needs you again, as `s` there; from another screen it goes to Home first. Refused while something of it waits for you or is stuck: `not hidden: claude-2 needs you`. Its finished turns count as seen. The flash: `claude-2 hidden: back when it needs you`. The menu note names the row. See [team](../screen/salon.md).

### `:slower`

Demo only, as `-`: the simulation runs slower.

### `:solo [PATH] [CHOICE] [NAME]`

Opens a solo session: one agent you drive yourself, with no leader and no team, as a project of its own ([solo sessions](../concepts/solo.md)). The three words may come in any order: a word with a `/`, or starting with `~` or `.`, is the PATH of the checkout (default: the project at hand's); a word that starts with `claude` is the CHOICE, `claude[:MODEL][@EFFORT]` (`claude`, `claude:opus@xhigh`, `claude@high`); the word left is the NAME (default: the directory's name, numbered `-2`, `-3` when taken). The agent is `solo-NAME`, writes in its own worktree, and every turn that ends with an answer shows you its worktree's diff as a `Δ`. Any number of solos may share a checkout with its team. The flash: `opened notes at ~/src/kule as a solo session; write to solo-notes (claude opus@xhigh)`. Refusals: the usage line when two words want the same place, `a project named notes is open already`, `"a:b" can not name a project: …`, `could not open: claude has no effort "bogus"; one of low, medium, high, xhigh, max`. `$`: a new session. Without a CHOICE, a screen asks first: the model (`default`, Claude Code's own, or one of its aliases; a full name is typed after `:solo claude:`), then the effort (`default` or one of its levels); `j k` and `1`-`9` choose, `enter` goes on, `esc` back a step (at the first, it gives up), `q` gives up. A PATH or NAME typed with the bare `:solo` waits for the choice. After `:solo ` the completion offers `claude`, then, after `claude@`, the efforts, and directories for a word that looks like a path. The demo opens a fake solo that answers and shows a change now and then. In a folder that is not a git checkout with a commit, the solo works straight in the folder, with no worktree and no `Δ`, and its header says `⌀ no git` ([in a folder without git](../concepts/solo.md#in-a-folder-without-git)).

### `:story`

Opens the journal's window and turns the story on, as `J` there: what happened, by card, newest first, with whom you hold up on top (see [the story](../screen/journal.md#the-story)); again, the lines. The menu note: `(on)` / `(off)`; remembered in the store, on by default.

### `:sync`

Brings the trunk of the project's [Lonca](../concepts/projects.md#on-lonca) repository to your checkout, on the side: `lonca fetch` puts it in `refs/lonca/trunk`, then your branch moves forward when that is a fast-forward. The journal says how it stood: `kule moved forward to trunk of kule`, `kule is level with trunk of kule`, or `kule has commits of its own: trunk of kule is in refs/lonca/trunk, nothing moved`. imece never merges, rebases or resets your branch, and work in the way stops it with git's words. Refused at once off Lonca, without git, while the store does not answer, or with a `lonca` command under way. The menu note says how far behind you are: `(↓3)`, `(↓?)` when your commit never was trunk and nothing was fetched yet, nothing when level. The demo keeps the same count on its table, and every third `:sync` finds commits of your own.

### `:talk [@NAME]`

The agent's conversation with you, as `v`; no name is the agent at hand (`no conversation with claude-2 yet` when there is none). See [conversations](../screen/conversations.md).

### `:theme [NAME|FILE]`

The theme, as `T` (`:t`). Alone: the next built-in one, around the end. With a name: one of `terminal`, `dark`, `light`, `mono`, `dracula`, `nord`, `gruvbox-dark-medium`, `gruvbox-light-medium`, `catppuccin-mocha`, `catppuccin-latte`, `tokyo-night-dark`, `solarized-dark`, `solarized-light`, `onedark`, `rose-pine`, `everforest`, `kanagawa`, `github`, `monokai`, or a colour file in Ghostty's or Kitty's format (anything with a `/`, or that is a file). While a name is written or stepped to with `tab`, the screen wears it; `enter` keeps it, `esc` gives it up. The flash: `theme: nord`; a wrong one: `theme solar: not one of terminal|dark|… and no such file`. Remembered for the next run. See [themes and language](../screen/themes-and-language.md).

### `:tidy`

The worktrees under imece's state that no agent owns (a crash, a removed store, a fork that never started), in a window: `WORKTREES NO AGENT OWNS · 2`, a row each with its name, its checkout and what it holds (`clean`, or `files changed: 3 · commits of its own: 1`). `j` `k` `g` `G` move, `a` removes the clean ones, `d` the one at hand (one that holds work asks first: `y` removes it, any other key keeps it), `esc` closes. When the worktree's checkout has a teardown script (`.imece/worktree-teardown.sh`), it runs in the worktree first; one not yet answered opens over the window (`a` yes, `r` no for the run, `esc` removes nothing). The flash says how it went: `removed claude-31`, `not removed: claude-32 (…)`. A worker's Lonca workspace left behind is listed too. With its session over, what it holds is what Lonca recorded when the session ended (the files no proposal carries, or `clean`), read without running anything under the dead token; a session that expired without saying is `a workspace that may hold work: its session is over` and goes only on a `y`. Proposals it made stay in Lonca. An agent's worktree is never listed or removed. At each start imece looks once and says `2 worktrees no agent owns: :tidy lists them` when it finds any. See [projects](../concepts/projects.md#worktrees-no-agent-owns).

### `:home`

Home: what is your turn across projects, what is stuck and what is at work, with what is at hand read beside it. See [Home](../screen/home.md).

### `:timeline`

The timeline, in a window over the screen (`esc` closes it): one lane per agent of the open projects over the last hour, each column what it was doing, the messages under each project; `[` `]` change a column's length. See [the timeline](../screen/timeline.md).

### `:trace [@NAME]`

The raw stream of the agent, as `t`: every line its session said, as data, up to 200 lines kept. No name is the agent at hand.

### `:unclear [inbox|journal]`

Brings back what `:clear` took off the screen: bare both, `inbox` or `journal` one. It says how much came back (`back on the screen: 2 waiting, 40 journal lines`), or `nothing is cleared off the screen`.

### `:undo`

Keeps a `:close`, `:forget`, `:archive` or `:clear` from happening, as `u`; `U` ([`:now`](#now)) does it at once instead. Both wait ten seconds before they act, and the bottom line counts down meanwhile: `◷ closing kule in 7 s · u keeps it open`, `◷ claude-2 forgets its session in 7 s · u keeps it`. `u` in that time leaves everything as it was (`kule stays open`, `claude-2 keeps its session`); with nothing counting down it says `nothing to undo`. One waits at a time: another `:close` or `:forget` makes the one waiting act first. Quitting while one counts down lets it act. A refusal (the last open project, no session to forget, a name nobody has) is said at once and waits for nothing. Both the demo and live sessions wait.

### `:unfold`

Unfolds everything in the team, as `zR` (on Home it stays); with a conversation open, every message of it, and the conversations opened after it start open again.

### `:unsettle`

Brings back everything hidden with `s` or `:settle`; Home stays. The flash: `3 back in the team`. The menu note says how many are hidden.

### `:waiting`

Home, its first row to decide. The menu note says how many wait.

### `:welcome`

The first-run window: what imece is, Claude Code with its version and whether it is signed in (or the command that signs it in), and the keys to start with. It is asked again, without the network; a line says `checking…` until its answer comes. It opens by itself on the first start ever and on a start where Claude Code is not signed in. Any key closes it. See [first run](../getting-started/first-run.md#the-welcome-window).

### `:why [@NAME]`

What put the agent in its state, on the bottom line: its rung of the ladder (idle, done and not seen, starting, working, needs you, stuck), since when, and the journal's line that put it there, in the journal's words: `claude-1: stuck · since 14:02 · claude-1 stopped: the session ended`. An agent with mail waiting on you ends with how many: `· messages queued until you answer: 2`. No name is the agent at hand, as for `:peek`. "Since" is when its oldest item began waiting for you, or else when its state last changed; the line is its newest journal line from then or before. When the journal no longer has one (tidied, or the agent came from an earlier run), it says so. It only looks, so it runs in an archive's view too.

### `:yes [@NAME]`

Says yes to everything the agent asks until its turn ends, as `e` on its ask in TO DECIDE or on its focus; no name is the agent at hand. The ask it waits on and its other open ones go on now, and so does every later ask of that agent until its turn ends (or its session does): commands, edits, fetches, whatever Claude Code asks about. Its trace says `allowed by your yes for this turn: Bash: …`, its row and focus say `✓ turn` while the yes holds, and the journal says when it was given and when it ended, with how many asks it let go on (`claude-1's turn ended, and your yes for it: it let 7 asks go on`). Another agent's asks still wait for you. With no ask of the agent waiting it says so (`claude-1 asks for no yes now: …`). Both the demo and live sessions. See [approvals](../concepts/approvals-and-questions.md#a-yes-for-this-turn).

## See also

- [Command line](cli.md): the flags the same things start with
- [The command line](../screen/command-line.md): writing, completion, readline keys, `!cmd` and `/cmd`
- [Keyboard](../screen/keyboard.md): every key
- [Files](files.md): where `:export`, `:copy`, `:forget` and `:open` read and write
