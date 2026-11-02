# Notice styles, a click that opens, and the terminal's own name

Part 3 of the motion and signals plan (#200). Record 60's `:notify text`
sent OSC 9 everywhere. The design talk decided: notices only as escape
sequences (OSC 9, 777, 99), no `notify-send` or `osascript`; sound only
through the terminal (kitty's sound names); a click on a kitty notice
opens what it was about, in the first version.

**The terminal's own name.** `probe` adds XTVERSION (`CSI > 0 q`) to the
questions asked at start, before the cursor position report it already
waits for, so the answer never reaches the key stream.
`probe::xtversion` reads `DCS > | text ST` (or BEL);
`screen::Who::parse` makes it a lower-case name and three numbers
(`kitty(0.49.2)`, `ghostty 1.4.0`, `iTerm2 3.7.6`, `tmux 3.8`). Probed on
2026-11-02: kitty 0.49.2 answers `kitty(0.49.2)`. The environment is not
the terminal: a kitty started from an iTerm2 shell had
`TERM_PROGRAM=iTerm.app 3.7.6`. Record 82's allow-list now goes by `Who`
first.

**One style per terminal** (`notify::Style::of`, the answer and the
environment), so no terminal gets two notices for one (Ghostty shows both
OSC 9 and 777):

- inside tmux (`Who` says tmux, or `TMUX`): `Tmux`, OSC 9 in tmux's
  passthrough (`DCS tmux; …ESC doubled… ST`). Probed on a pty with tmux
  3.8: plain OSC 9 and 777 never leave tmux; the wrapped one does with
  `allow-passthrough on` and is dropped without it. The terminal outside
  is not known, so OSC 9, which iTerm2, Ghostty, kitty and WezTerm all
  show. A click cannot come back through tmux.
- kitty from 0.36 (urgency since 0.35, sound and the support query
  since 0.36, its own docs): `Kitty`, OSC 99 in two parts with one id,
  `i=imece-N:d=0:u=U:s=S:a=focus,report:p=title;imece`, then
  `i=imece-N:p=body;words`. `u=2` and the `error` sound for something
  stuck or a vendor error (`Word.loud`), else `u=1` and `question`. The
  sound name is base64 (`ZXJyb3I=`, `cXVlc3Rpb24=`), as kitty's protocol
  says (a plain name was a mistake of the first probe). kitty 0.49.2's
  answer to `p=?`: `a=focus,report`, `u=0,1,2`, `s=system,silent,error,
  info,question,warn,warning`.
- Ghostty: `Osc777`, `777;notify;imece;words` (`;` in the words becomes
  `,`). Ghostty 1.4.0's own docs: notices by OSC 9 or 777; no OSC 99.
- else `Osc9`, as before.

The words keep imece's `imece · ` in OSC 9; a style with a title drops
it from the body (`TITLE`). `Notify::Bell` is the same BEL everywhere
(tmux passes the bell on).

**The click.** kitty sends `ESC ] 99 ; i=ID ; ST` when a notice with
`a=report` is clicked (its docs; a button's number after the `;`, none
for the notice as a whole; no close report unless `c=1` is asked).
crossterm has no OSC input: it reads `ESC ]` as alt-`]` and every
character after as a key, so the report would type (the pty test
without the filter: `i` opened the command line). With the `Kitty` style
only, the main loop reads what comes with an alt-`]` (`REPORT_WAIT` 20 ms
between events, at most `REPORT_MAX` 64), and `notify::report` takes a
report whole: alt-`]`, `99;`, characters with no ctrl or alt, then alt-`\`
or ctrl-g (BEL). Any report is swallowed; only `i=imece-N;` with nothing
after acts. Anything else (another start, a key with ctrl or alt, an
event that is no key, no end within 64) is keys, put back in order
(`queued`). A person cannot type alt-`]` and the rest within 20 ms of
each.

`App::notices` numbers each notice (`notice_count`) and keeps the last
`NOTICED` (16) with what they were about (`noticed`). `App::clicked(id)`
closes the windows that would hide it (an archive's view, help,
archives, peek, hints, news, welcome, the digest, where you left off,
the quit question) and opens: the one item still open
(`open_waiting`, the tail of `W`), the one stuck agent (`go_agent`), else
the next waiting item (`W`). An id imece did not send, or one of the
16 gone, does nothing.

**Checked.** A pty playing kitty (answering XTVERSION as `kitty(0.49.2)`)
ran `imece --demo` with `:notify text`, lost the focus, got
`ESC]99;i=imece-1:d=0:u=2:s=ZXJyb3I=:a=focus,report:p=title;imece` and
`…p=body;qwen-1 is stuck`, sent kitty's report back, and nothing was
typed. The same bytes to imece with no answer to XTVERSION opened the
command line.

**Points.** Growth: 16 notices kept, a number that only counts up. Read
cost: one alt-`]` reads at most 64 events at 20 ms each. Failure: a
report split across reads more than 20 ms apart would be typed (a
terminal writes it at once); a terminal that answers XTVERSION late is
still read before the screen (the cursor report ends the reading).

**Not done.** Nobody clicked a real kitty notice here (the screen was
the person's, no desktop control): the report's bytes are kitty's
documentation's, and crossterm's reading of them was run. Ghostty's and
kitty's notices were not seen on a screen. WezTerm keeps OSC 9 (its
`TERM_PROGRAM` aside, no style of its own). The terminal outside tmux is
not asked (tmux answers for itself).
