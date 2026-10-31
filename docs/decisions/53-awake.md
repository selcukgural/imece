# Keep the machine awake mid-turn

`--awake` and `:awake` (2026-10-31, backlog A6 in #54, from Orca and
cmux): a laptop that went to sleep mid-turn stopped every agent's
session in the middle of its work, and the person came back to cut
turns.

Opt-in: off unless `--awake` is given (the demo takes it too) or
`:awake` turns it on; `:awake` again turns it off, and its value shows
in the `:` menu. While it is on and any agent is mid-turn
(`World::mid_turn`), imece runs the system's own tool as a child
(`awake.rs`): `caffeinate -i -w PID` on macOS, which also ends with
imece's process, or `systemd-inhibit --what=idle:sleep --mode=block
sleep infinity` on Linux; nothing elsewhere. It is checked after every
step (`App::keep_awake`) and when `:awake` changes: the child starts
when something is mid-turn and is let go when nothing is, or the
person turns it off, or imece ends (dropped). It runs in a process
group of its own, and letting go signals the whole group, so
systemd-inhibit's `sleep` goes with it. A program that does not start,
or ends by itself, is said once in the flash ("could not keep the
machine awake: …") and not tried again in that run.

Not done: a kill -9 of imece leaves systemd-inhibit's lock until its
`sleep` is killed (caffeinate's `-w` follows imece's pid by itself);
not remembered between runs (a flag each time, by choice: it changes
the machine's power behaviour).
