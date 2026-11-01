# A key file of the person's own

Remappable keys (2026-11-02, backlog C9 in #56, from Orca, openharness
and Warp; issue #171). Before this, every key was matched where it acts
(`App::key`, `thread_key`, `diff_key`, `copy_key`, `peek_key`, the
pickers), and someone whose fingers know other keys had no way to say so.

**The file.** `$XDG_CONFIG_HOME/imece/keys`, else `~/.config/imece/keys`
(`keys::file`; a relative `XDG_CONFIG_HOME` is ignored, as the spec
says). A file the person writes by hand is a config file, not a pref:
the screen choices live in the store because imece writes them; this one
imece only reads. Each line is `KEY = KEY` (three words, the middle one
`=`), a line starting with `#` is a comment, blank lines are nothing. The
parser is imece's own (`Keymap::read`): no TOML, no new dependency for
nine words of grammar. Names are the help's (`j`, `G`, `^n`, `⌥b`,
`⇧tab`, `enter`, `esc`, `tab`, `space`, `↑ ↓ ← →`, `home`, `end`, `pgup`,
`pgdn`, `f1`–`f12`, `backspace`, `del`, `insert`) or words (`ctrl-`,
`alt-`, `shift-` prefixes, `up`, `escape`, `return`, `pageup`, `delete`,
`backtab`); `hash` is `#`, which cannot start a line. `shift-g` is `G`;
a shift on a digit or sign is refused, since the character it gives
depends on the keyboard. Keys are told apart by code and ctrl/alt/shift
(`keys::Key`): a character carries its shift (`G`), `⇧tab` is BackTab,
`^N` is `^n`, super and the rest are not kept.

**One step, like `noremap` (user, 2026-11-02: key → key, not key → `:`
name).** A line makes a key act as another key of imece's; the key that
comes out is not looked up again, so `j = k` and `k = j` swap the two,
and a key the file does not name is itself. This was chosen over binding
keys to `:` names because movement and row keys (`j k`, `enter`, `a r`)
have no names (record 41), and because the translation sits in one place
in front of every handler (`Keymap::turn` at the top of `App::key`; `Keymap::alone` and `Keymap::pair` since C9b),
leaving all the context logic as it was. The key keeps its kind and
state; it gets the target's code and modifiers, so `x = ^o` goes back and
`^n = 2` is a plain `2`. It is not applied where keys are text: writing,
the help's `/` filter and the hint letters (whose labels are letters).
`^c` is checked before it and always quits; it cannot be given on either
side. The lead question at start (`ui::pick`, in `main::ask`) takes the
file's keys too.

**Refused whole, the last good kept.** A file is taken whole or not at
all; the first wrong line says why (`keys::Why`): not `KEY = KEY`, a name
that is no key (cut at 24 characters, through `fill` so cleaned), a
target imece does nothing with outside writing (`Key::used`: the plain
keys and `^d ^u ^f ^b ^o ^r` that some handler matches), a key given twice
(with the line that gave it), `^c`, more than 200 keys, a file over 64
KiB (checked before reading) or not UTF-8, a folder in its place. A
refused file leaves the keys in use. The text of the last good file is
kept in the store's `meta` as `keys` (`prefs::keys`/`keep_keys`, a
setting, no schema change), so a start whose file is refused still has
those keys (`keys::Start`) and says why in the flash; a kept text that no
longer reads is dropped. No file means imece's own keys, and the kept
text is forgotten: deleting the file is how to go back.

**Read at start, on `:keys` and when it changes (user, 2026-11-02).** `:keys`
reads the file again and flashes `PATH: keys taken (N)`, or `no key file;
KEY = KEY lines in PATH give you keys of your own`, or the refusal with
`the keys you had stay`. A good file whose lines take a key of imece's
away without giving that meaning to another key adds `; nothing does j
now` (`Keymap::lost`); that is a warning, not a refusal, since moving a
key away can be what the person wants. The menu note is how many keys the
file gives. `App::keys_new` tells the main loop to keep the new text.

The file is also watched (C9c, #173): `App::watch_keys`, called by the
main loop, looks at it every `keys::EVERY` (2 s), and at once after the
terminal gets the focus back (`App::focus` forgets when it last looked),
so an edit in another window is taken by the time the person is back.
It compares `keys::stamp`, the file's modified time, length and inode
through a link (no read, one `stat`), with the stamp taken before the
file was last read (by the start in `main`, by `:keys`, by a watch), so
a change while it is read is seen the next time. A different stamp reads
the file as `:keys` does and says the same words; the same stamp says
nothing, so a bad edit is said once and its keys stay, not every two
seconds. A removed file gives imece's own keys back, as on `:keys`; an
editor that saves by renaming over the file leaves no gap a two-second
look is likely to fall in. A save within the clock's step that keeps the
length and the inode is not seen until the next change (`:keys` still
reads it). Polling, not `kqueue`/`inotify`: no new crate, one `stat`
every two seconds, and the same code on macOS and Linux.

**Keys of two (C9b, #175, 2026-11-02).** A side of a line is one key or
two (`keys::Keys`, at most `keys::STROKES`): `gt = G` gives a key of two
strokes, `x = gg` turns one key into two, `g enter = ^o` and `^t = z M`
mix them. A word that is a key's name is that key (`^n`, `f1`, `up`);
any other word of two characters is two plain keys (`gt`, `zM`); keys
can also be written apart, which is the only way for two keys with ctrl,
alt or a name, or for `f 1` and `u p`. `Keys::name` writes them back the
same way (together unless that reads as another key). The decisions,
made while the person was away and to go over together:

- The first key of two is held (`App::keys_held`, with when) and waits
  for the second, at most `keys::WAIT` (1 s, vim's `timeoutlen`); the
  main loop's `App::keys_due` lets it through alone when the time is up,
  through the same `around` as a key (the place left is recorded). The
  loop wakes every tick (100 ms live, 500 ms in the demo), so the wait is
  up to half a second longer in the demo.
- A second key that does not make a line with it lets the first act
  alone first, then the second is taken afresh (it may start a hold of
  its own, as in vim). It is taken where the first left the screen: a
  held `/` that turns out alone opens writing, and the second key is
  then a letter.
- A key is not held while imece's own chord waits for its second key
  (`App::pending`): with `gt = G`, `g g` is still `gg` at once, not after
  a second; only the first `g` waits.
- A key may not be given alone and also start two keys (`Why::Starts`):
  alone it would act at once and the two could never be typed. Vim
  allows it with the timeout; refusing is the plainer rule. Three keys
  or more on a side are refused (`Why::Long`).
- The keys that come out (`Keymap::alone`, `Keymap::pair`) are acted one
  after the other (`App::act_all`) and not turned again; the away digest
  is only for the first. The lead question at start takes one-key lines
  only through `alone` (a key given as two still works there, each key
  in turn); two-key lines are not held there.
- The help: a two-key line is in the "your keys" rows as written
  (`gt  your key: as G`, `x  your key: as gg`); `Keymap::acting_as` takes
  the keys of a help word (one, or two such as `gg`) and gives the lines
  that give exactly those first, then the keys themselves the way the
  person reaches them (two keys each by its first one-key way, unless a
  line gives those two another meaning). `Keymap::lost` also names
  imece's chords (`gg za zM zR qq qs`) a line took and nothing gives.

**The help (user, 2026-11-02: in place and a section).** `?` lists the
file's keys first (`^n  your key: as j`), then every key line with each
key the file moved written as the keys that do it now
(`Keymap::shown`): the person's keys in the file's order, then the key
itself unless the file gave it another meaning (`j/^n`), `–` when
nothing does it. A word of two keys (`gg`, `zM`) shows each one's first
key. The rows whose keys are written or are none of imece's
(`overlay::TYPED`: `$`, the three writing rows, the new-line row) are
shown as they are. The `/` filter goes over both. Nothing changes when
there is no file. The help's own row stays 55 lines: `:keys` is named in
the `?  :help` row's text.

**The demo.** The keymap is screen logic: the demo reads the same file
the same way, so there is nothing to fake.

**Gaps.** Three keys or more on a side cannot be given. A held key is
not shown anywhere while it waits (vim's `showcmd`). Keys are not per
context (the same file holds in the salon and in a change; C9d).
