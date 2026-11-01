---
title: Commands
description: Every `:` command imece takes, with its arguments, the key it mirrors, its short name, whether it is demo-only, whether `$` marks it as raising the next bill, and what its menu note shows.
---

# Commands

A line that starts with `:` is a command to imece, not a message to an agent. You type one in the command line (`:` opens it), `tab` offers the matches with their values, `enter` runs it. Every key that *acts* (not the ones that only move) has a `:` name too, so anything can be done from anywhere by typing it; the key handlers and the commands call the same code, so they cannot drift apart.

## How the `:` line works

- `:` then `tab` lists the commands. The menu shows after each command what it stands at now or would act on, in brackets: `budget (spent $3.10 of $20.00)`, `theme (dark)`, `lead (claude)`, `close (kule)`, `forget (claude-2)`, `next (2 waiting)`, `needs (off)`. The note is drawn, never inserted.
- `$` after a command marks one that raises the next bill: `:open` (a new leader session), `:solo` (a new session), `:forget` (a new session that pays the whole briefing again, without the cache) and `:commit` (a paid call). No `:` command starts a paid turn by itself; `:budget` shows its value instead of a mark.
- Short names: `:b` budget, `:c` close, `:f` forget, `:o` open, `:q` quit, `:t` theme. A short name typed whole is a command already, so the menu offers nothing and `enter` runs it (`:q` then `enter` asks at once).
- Arguments complete too: after `:lang` the languages, after `:lead` the vendors, after `:notify` its three words, after `:theme` the built-in names, after `:close` and `:pin` the open projects, after `:goto` nothing (the `&` line has its own menu), after `:forget`, `:talk` and `:trace` the agents of open projects (`@` or not), after `:open` and `:export` directories (`~` for your home, hidden ones only after a `.`, `tab` steps into the chosen one).
- A name imece does not know: `no command xyz; : and tab list them`. A demo-only command typed live: `:pause is for the demo: live agents run in real time`.
- `!cmd` runs a command in your own shell and `/cmd` is the agent's own command line; neither is a `:` command ([command line](../screen/command-line.md)).

## All commands

| Command | Key | Short | Demo only | `$` | Menu note |
| --- | --- | --- | --- | --- | --- |
| `:awake` | — | | | | on / off |
| `:board` | `2` | | | | |
| `:budget USD` | — | `:b` | | | spent X of Y |
| `:close [NAME]` | — | `:c` | | | the project at hand |
| `:code` | `3` | | | | |
| `:commit` | — | | | `$` | |
| `:copy [PATH]` | — | | | | the list, when there is one |
| `:defer` | `S` | | | | |
| `:effort LEVEL` | — | | | | the solo at hand's effort, or `default` |
| `:export [PATH]` | — | | | | `the conversation` or `#12` |
| `:faster` | `+` | | yes | | the speed, `×2` |
| `:fold` | `zM` | | | | |
| `:forget [@NAME]` | — | `:f` | | `$` | the agent at hand |
| `:goto [NAME]` | `&` | | | | |
| `:help` | `?` | | | | |
| `:lang [en\|tr]` | `L` | | | | `English` / `Türkçe` |
| `:lead VENDOR` | — | | | | the primary vendor, or `ask` |
| `:mark` | `m` | | | | |
| `:model NAME` | — | | | | the solo at hand's model, or `default` |
| `:needs` | `f` | | | | on / off |
| `:next` | `W` | | | | N waiting |
| `:notify [off\|bell\|text]` | — | | | | off / bell / text |
| `:open PATH [NAME]` | — | `:o` | | `$` | |
| `:pause` | `space` | | yes | | on / off |
| `:pending` | `q s` | | | | |
| `:pin [NAME]` | — | | | | the project at hand, `▴` when pinned |
| `:quit` | `q` | `:q` | | | |
| `:rename NAME` | — | | | | the project at hand |
| `:salon` | `1` | | | | |
| `:slower` | `-` | | yes | | the speed, `×2` |
| `:solo [PATH] [CHOICE] [NAME]` | — | | | `$` | |
| `:talk [@NAME]` | `v` | | | | the agent at hand |
| `:theme [NAME\|FILE]` | `T` | `:t` | | | the theme in use |
| `:trace [@NAME]` | `t` | | | | the agent at hand |
| `:unfold` | `zR` | | | | |
| `:waiting` | `w` | | | | N waiting |

Live, `:pause`, `:faster` and `:slower` are left out of the menu. Keys that move (`j k`, `gg G`, `enter`, `esc`, `tab`) and keys that act on the selected row (`a r c n x`) have no `:` name: typing them would be slower than the key, and the row would have to be named.

## The commands

### `:awake`

Turns keeping the machine awake on or off (the same as `--awake`). While it is on and any agent is mid-turn, imece runs `caffeinate` (macOS) or `systemd-inhibit` (Linux) as a child and lets it go when nothing is mid-turn. The flash says `the machine stays awake while an agent is mid-turn` or `the machine may sleep again`; a tool that cannot start is said once: `could not keep the machine awake: …`. Not remembered between runs.

### `:board`

The board view, as `2`. See [board and code map](../screen/board-and-code-map.md).

### `:budget USD`

Raises or lowers the pot while agents run (`:b 80`). The value is dollars with at most two decimals, up to four digits before the point (`80`, `2.50`); anything else: `80$ is not dollars like 80 or 2.50`. The flash says `the budget is $80.00 now`, the journal `you set the budget to $80.00 (was $50.00)`, and the warning that the pot runs low may sound again. A turn under way finishes on its old cap; the next starts on the new remainder, in a new session if the old one had hit its cap. The pot is one for all projects and is kept between runs. The menu note: `(spent $3.10 of $20.00)`. See [budget and limits](../concepts/budget-and-limits.md).

### `:close [NAME]`

Closes the project `NAME`, or the one at hand (`:c`). Its agents' sessions and runs end, their mail, notes, offers and changes are dropped, their items in WAITING FOR YOU close, and the project is marked closed: the salon skips it, the same checkout opened again takes its place with a new leader. The worktrees stay on disk. The flash: `closed kule: its agents left; their worktrees stay on disk`. The last open project refuses: `kule is the last open project; q q quits`; a name nobody has: `no open project named x`. In the demo, the project's agents leave the same way. See [projects](../concepts/projects.md).

### `:code`

The code map, as `3`. Live it says there is no shared trunk yet (each agent works in its own worktree, so there are no leases to show); symbol leases are not built. See [board and code map](../screen/board-and-code-map.md).

### `:commit`

Asks Claude Code's cheap tier (`haiku`, or your `--tier claude:cheap=…`) for a commit message for what is staged in the project at hand's checkout, and puts it on the command line as `!git commit -m '…'` for you to edit and run with `enter` in your own shell, or drop with `esc`. imece commits nothing. The flash says `asking haiku for a commit message for what is staged in kule`; the journal says each suggestion with its model and cost, and each failure with why.

It costs: each call at most $0.05 (the call's own hard cap), a day (UTC) at most 20 calls and $1.00 over all projects, reserved from the pot before the call and settled after. Refused with `no commit message: …` when nothing is staged, the day is full, the pot has less than the cap, or Claude Code is not on PATH (it is the one vendor whose call takes a spending cap; Codex and Gemini leads need Claude Code for this). The same staged tree asked again costs nothing: the last suggestion comes back. The call reads `git diff --cached --stat`, the patch (at most 24 KiB) and the eight newest commit subjects; it runs with no tools, no MCP server, its own short system prompt, nothing saved. The demo answers from the titles of the changes applied in that project, as `git commit --dry-run`. See [own shell and commit](../scenarios/own-shell-and-commit.md).

### `:copy [PATH]`

The list of paths git ignores that a worker's start copies into its worktree (`.env`, `node_modules`, `target`): a worktree is a fresh checkout, so what git ignores is not there. `:copy PATH` adds a path of the project at hand, `:copy PATH` again drops it (a trailing `/` is the same path), `:copy` alone says the list: `copied into kule's worktrees: .env, target` or `nothing is copied into kule's worktrees; :copy PATH adds a path git ignores`.

A path must be relative, literal (no glob, no `..`, not in `.git`, at most 200 bytes), there in the checkout, not a link, inside the checkout once resolved, and ignored by git (a tracked file is in the worktree already); else `x can not be copied: …`. The copy happens at each worker's start (helpers share their worker's worktree; never for the leader), only for paths the worktree lacks, all paths of one start within 2 GiB, as an APFS clone or reflink where the volume can. The journal says what was copied and what was skipped. Kept with the project in the store. See [files](files.md) and [projects](../concepts/projects.md).

### `:defer`

Puts the item at hand in WAITING FOR YOU off and opens the next one, as `S`. See [waiting for you](../screen/waiting.md).

### `:effort LEVEL`

Sets how deep the solo session at hand thinks, from its next turn: one of its vendor's efforts (Claude Code: `low`, `medium`, `high`, `xhigh`, `max`; Codex: `low`, `medium`, `high`, `xhigh`), or `default` for the vendor's own. The solo at hand is the selected agent when it is a solo, else the project at hand's solo. Its running session ends and the next message starts a new one on the same transcript, told where it left off, so nothing is lost but the process; refused mid-turn (`could not change solo-notes: solo-notes is mid-turn; wait for its answer`), for a vendor without efforts (`gemini has no effort setting; …`), for a level the vendor lacks (`claude has no effort "turbo"; one of …`) and when no solo is at hand (`no solo session at hand: select one, or open one with :solo`). The flash: `solo-notes runs on claude opus@max from its next turn; its session starts again on it`; the journal: `you set solo-notes to run on claude opus@max`. The menu note shows the effort now, `(xhigh)` or `(default)`; after `:effort ` the levels complete. The model part of the choice stays. See [solo sessions](../concepts/solo.md).

### `:export [PATH]`

Writes what is open as a Markdown file: the open conversation (`# Conversation: lead ⇄ claude-1`, each message a heading with its day, time, who to whom and its type), else the card's screen (`# #12 title`, its project, column and minutes, WHO, STORY, what waits for you on it, and its journal lines, oldest first, none folded). Nothing open: `nothing to export: open a conversation (v) or a card (#N) first`.

`PATH` is a file, or a directory to name one in; none is the directory imece was started from. The name says what and when: `imece-conversation-20261031-1400.md`, `imece-card-12-20261031-1400.md`. A file that is there already is never written over; the flash says `written to …` or `could not write …: …`. Every time has its day, every line goes through the same cleaning as the screen. The menu note says what it would write: `(the conversation)` or `(#12)`.

### `:faster`

Demo only, as `+`: the simulation runs faster. The menu note shows the speed (`×2`).

### `:fold`

Folds every project and every worker's helpers in the salon, as `zM`; from another view it goes to the salon first. See [salon](../screen/salon.md).

### `:forget [@NAME]`

Makes the agent forget its vendor session (`:f`): the running session ends (its run killed, an unfinished prompt kept as interrupted, told at the wake and not run again), the remembered session goes with what the vendor kept of it (Claude's transcript, Codex's thread, Gemini's chat files), the agent sleeps until spoken to, and its next message starts a new session with the full briefing. No name is the agent at hand. The flash: `claude-2 forgot its session; the next message starts a new one, told where it left off`; refused for an agent with nothing to forget or one that is gone: `claude-2 keeps its session: it has no session to forget`. The journal: `you made claude-2 forget its session; it starts a new one, told where it left off`. `--fresh` does it for every agent at the start. `$`: the new session pays the whole briefing again. See [sessions](../concepts/sessions.md).

### `:goto [NAME]`

Goes to the agent called `NAME` (an `@` before it is fine), else to the open project of that name: the salon with its row selected, the project unfolded, a helper's worker unfolded, and `f` let go when it would hide the row; a project's name selects its header. Case and Turkish letters do not matter. Nothing of the name: `nothing called "zzz" to go to: an agent or a project`, and nothing moves. Alone it opens the `&` line: the places as they stand now, what needs you first (stuck, then waiting, then done and not seen), the rest by their state (working before idle), then the open projects, each agent with its glyph and status words. The list is frozen while the line is open, so its numbers hold while you type; a digit `1`-`9` right after `&` goes at once, `#` right after `&` turns to the card line, `!` keeps only what needs you, digits match the start of a number, anything else matches inside the names. `enter` goes to the item under the cursor, or to the name typed. See [salon](../screen/salon.md).

### `:help`

The `?` help: every key and command in both languages, the legend of glyphs; `/` in it filters by every word. See [keyboard](../screen/keyboard.md).

### `:lang [en|tr]`

The screen's language, as `L`. Alone it switches to the other language; `:lang en` or `:lang tr` sets one. Anything else: `de is not a language: en or tr`. The menu note shows the language in use. For this run only; the next start reads `--lang` or `IMECE_LANG`.

### `:lead VENDOR`

Who leads from the next start on, without asking: `claude`, `codex` or `gemini` makes that vendor the primary one (`claude leads from the next start on, without asking`); `ask` forgets it (`the next start asks who leads`). Anything else: `x is not claude, codex, gemini or ask`. The primary vendor is kept in the store; `--lead` on the command line wins over it for one run, and a primary that is not on PATH is said on the question screen, never replaced silently. The menu note: `(claude)` or `(ask)`. See [vendors](../concepts/vendors.md).

### `:mark`

Marks the line at hand to come back to, as `m`: in the open conversation, else in the journal; again unmarks. A marked line shows `✱`, is kept between runs and is never folded, tidied or left out of a load; `'` goes to the next marked line. The flash: `marked: ' comes back to it`. See [journal](../screen/journal.md).

### `:model NAME`

Sets the model the solo session at hand runs on, from its next turn: an alias (`opus`, `sonnet`, `haiku`, `fable` for Claude Code; `auto`, `pro`, `flash`, `flash-lite` for Gemini CLI) or a full name as the vendor takes it (`gpt-5.6-terra`), or `default` for the vendor's own. The same rules, refusals, flash and journal line as `:effort`; a name that is not a model name (`a b`, `-x`) is refused. After `:model ` the aliases complete; a full name is typed. The effort part of the choice stays.

### `:needs`

The salon shows only what needs you (waiting for you, stuck, done and not seen), as `f`; again, everyone. Goes to the salon first. The menu note: `(on)` / `(off)`.

### `:next`

Opens the next item waiting for you from anywhere, as `W`. The menu note says how many wait: `(2 waiting)`.

### `:notify [off|bell|text]`

How you are told, in another window, that something came to wait for you: `bell` rings the terminal's bell (every terminal has a setting for it: a sound, a badge on the tab, a flash; tmux's `monitor-bell`), `text` sends a desktop notification with imece's words (OSC 9: iTerm2, WezTerm, Ghostty; a terminal that does not know the sequence ignores it, so choose `bell` there), `off` (the default) neither. Bare `:notify` goes to the next one round: off, bell, text, off. The flash says `the bell rings when something waits and you are in another window`, `a desktop note when something waits and you are in another window` or `no word in another window`; another word: `beep is not off, bell or text`. The menu note shows the value; it is remembered in the store.

What rings: a new item in WAITING FOR YOU (a change, a question, an answer, an ask for your yes, something sensitive) or an agent that got stuck (blocked, its session stopped), when it came while the terminal did not have the focus and is still there five seconds later, once each. A note names the kind and the agent, never the agent's words: `imece · claude-1 finished: a change waits for you`, `imece · codex-2 asks you`, `imece · gemini-1 is stuck`; several due at once are one bell, or one note with the count (`imece · 3 things wait for you`). What was on screen when you left never rings, and back in the window what was about to ring is dropped: it is on screen now. The terminal must report focus (tmux needs `focus-events on`); one that does not never rings. See [WAITING FOR YOU](../screen/waiting.md).

### `:open PATH [NAME]`

Opens the git checkout at `PATH` as a project of its own (`:o`), with a new leader named `lead-NAME` on the team's lead vendor (same model and tiers). `NAME` defaults to the directory's name made safe: 1-24 of letters, digits, `.`, `_`, `-` (the rest become `-`); a wrong name is refused: `"x y" can not name a project: 1-24 of letters, digits, . _ -`. `~` stands for your home; a path with spaces is not parsed. The flash: `opened kule at /Users/me/src/kule; write to lead-kule`. Refusals: `:open PATH [NAME] opens the git checkout at PATH as a project` (no path), `a project named kule is open already`, `kule is open already, as k2`, `could not open: …` (not a checkout). A closed project with the same checkout opens again in its place. Opened projects are remembered between runs; a second imece on another checkout is told to use `:open` from the first. `$`: a new leader session. The demo adds a fake project with a leader, two workers and three cards. See [projects](../concepts/projects.md).

### `:pause`

Demo only, as `space`: pauses or resumes the simulation. The menu note: `(on)` / `(off)`.

### `:pending`

Before quitting, the list `q s` shows: the agents mid-turn and the changes not reviewed; `q` there quits, `W` opens the next waiting.

### `:pin [NAME]`

Pins the project `NAME`, or the one at hand, to the top of the salon; again unpins. Pinned projects come first, in the order they were opened, and fold last when the salon is short; a pinned header wears `▴`. The flash: `kule is pinned to the top of the salon` / `kule is not pinned any more`. Kept in the store; a closed project keeps its pin for when it opens again. The menu note shows the project it would act on, `▴ kule` when pinned already. A colour per project is not built.

### `:quit`

Quits, as `q` (`:q`): it asks first (`quit imece? q again to quit · any other key to stay`), counting the agents mid-turn and the changes not reviewed when there are any; `s` lists them.

### `:rename NAME`

Renames the project at hand and its leader to `lead-NAME` (project 0's `lead` too). Sessions go on; every live agent of the project gets a note at its next prompt, and a `send` to the old name answers with the team list. The same name rule as `:open`. The flash: `kule is now k2; write to lead-k2`.

### `:salon`

The salon, as `1`.

### `:slower`

Demo only, as `-`: the simulation runs slower.

### `:solo [PATH] [CHOICE] [NAME]`

Opens a solo session: one agent you drive yourself, with no leader and no team, as a project of its own ([solo sessions](../concepts/solo.md)). The three words may come in any order: a word with a `/`, or starting with `~` or `.`, is the PATH of the checkout (default: the project at hand's); a word that starts with a vendor's name is the CHOICE, `VENDOR[:MODEL][@EFFORT]` (`claude`, `claude:opus@xhigh`, `codex@high`, `gemini:pro`); the word left is the NAME (default: the directory's name, numbered `-2`, `-3` when taken). The agent is `solo-NAME`, writes in its own worktree, and every turn that ends with an answer shows you its worktree's diff as a `Δ`. Any number of solos may share a checkout with its team. The flash: `opened notes at ~/src/kule as a solo session; write to solo-notes (claude opus@xhigh)`. Refusals: the usage line when two words want the same place, `a project named notes is open already`, `"a:b" can not name a project: …`, `could not open: codex is not on PATH here, or imece does not drive it`, `could not open: claude has no effort "bogus"; one of low, medium, high, xhigh, max`. `$`: a new session. Without a CHOICE, a screen asks first: the vendor (the ones imece can run here, the lead vendor first), then the model (`default`, the vendor's own, or one of its aliases; a full name is typed after `:solo VENDOR:`), then the effort (`default` or one of the vendor's levels; skipped for Gemini, which has none); `j k` and `1`-`9` choose, `enter` goes on, `esc` back a step (at the first, it gives up), `q` gives up. A PATH or NAME typed with the bare `:solo` waits for the choice. After `:solo ` the completion offers the vendors, then, after `claude@`, that vendor's efforts, and directories for a word that looks like a path. The demo opens a fake solo that answers and shows a change now and then.

### `:talk [@NAME]`

The agent's conversation with you, as `v`; no name is the agent at hand (`no conversation with claude-2 yet` when there is none). See [conversations](../screen/conversations.md).

### `:theme [NAME|FILE]`

The theme, as `T` (`:t`). Alone: the next built-in one, around the end. With a name: one of `terminal`, `dark`, `light`, `mono`, `dracula`, `nord`, `gruvbox-dark-medium`, `gruvbox-light-medium`, `catppuccin-mocha`, `catppuccin-latte`, `tokyo-night-dark`, `solarized-dark`, `solarized-light`, `onedark`, `rose-pine`, `everforest`, `kanagawa`, `github`, `monokai`, or a colour file in Ghostty's or Kitty's format (anything with a `/`, or that is a file). While a name is written or stepped to with `tab`, the screen wears it; `enter` keeps it, `esc` gives it up. The flash: `theme: nord`; a wrong one: `theme solar: not one of terminal|dark|… and no such file`. Remembered for the next run. See [themes and language](../screen/themes-and-language.md).

### `:trace [@NAME]`

The raw stream of the agent, as `t`: every line its session said, as data, up to 200 lines kept. No name is the agent at hand.

### `:unfold`

Unfolds everything in the salon, as `zR`.

### `:waiting`

Straight to WAITING FOR YOU, as `w`. The menu note says how many wait.

## See also

- [Command line](cli.md): the flags the same things start with
- [The command line](../screen/command-line.md): writing, completion, readline keys, `!cmd` and `/cmd`
- [Keyboard](../screen/keyboard.md): every key
- [Files](files.md): where `:export`, `:copy`, `:forget` and `:open` read and write
