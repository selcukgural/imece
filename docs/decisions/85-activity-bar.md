# The activity bar

Part 5, the last, of the motion and signals plan (#200). The design talk
(2026-11-02) named it as the motion that carries information: a short
bar on each row saying how busy the agent has been lately, `▁▃▅▂▁`,
changing only as events come, so it stays inside record 77's rule (drawn
on a change, never on a timer) and needs no frame of its own.

**What it counts.** Every line an agent's trace gets (`Agent::trace`):
its texts, tool calls and failures, plans, context notes, messages on
the bus, imece's notes about it (a retry, a session start). The trace is
where live (`live/sessions.rs`) and the demo (`sim.rs`) already write
every event, so the demo shows the bar for the same reasons live does
(record 31) without a path of its own, and nothing new is asked of a
vendor. `Agent.stirs: Stirs` keeps `[u16; STIR_TICKS]` (5) counts, the
oldest first, and the tick of the newest column; `Stirs::add(tick)`
moves the columns left by the ticks gone (at most five) and counts the
line in the newest column; a line stamped before the newest tick counts
as the newest (the trace never stamps one, and looking back would only
add arithmetic for no case). A column saturates at `u16::MAX`.
`Stirs::at(tick)` gives the columns as they stand at `tick`, empty ones
for the ticks since the newest line. A tick is 5 s live, so the bar
covers 25 s; the demo's clock calls its ticks 5 s too.

**How it is drawn.** `salon::stir_spans` after the quote: `  ▁▂▃▄▅`, dim,
one glyph per column on one scale for every row so rows compare
(`salon::stir_glyph`: 0 `▁`, 1 `▂`, 2 `▃`, 3–4 `▄`, 5–6 `▅`, 7–9 `▆`,
10–14 `▇`, more `█`; a log-like scale, since a Claude turn writes from
one line a tick while it thinks to a dozen while it edits). A row whose
columns are all empty shows nothing, so a calm salon stays as it was and
the bar of an agent that stopped is gone five ticks later. It is the
first of the row's parts to give way: the context mark, the to-do mark
and the time get their room, then the quote, then the bar takes what is
left (`salon::fits`, the closure it replaced made a function so the
quote's columns can be taken between); a row never grows past its
width. `:motion off` leaves it out: off is the screen as it was before
the motion plan, and the person who asked for a still screen gets one;
calm and full show it the same (it does not animate, so full has
nothing to add). The `?` legend names it (`▁▃▅ what it did, 5 s a
column`), both languages.

**Read cost.** One `[u16; 5]` copy per agent row drawn; nothing scans
the trace. The bar changes when a line is traced (the step already says
the screen changed) or when the tick moves (already in the screen's
`Clock`), so no frame is drawn for it alone, focused or not.

**Growth.** Ten bytes and a tick per agent, memory only, not saved:
after a restart the bar starts empty, which is what happened in the last
25 s of a new run anyway.

**Ten times the scale.** At a hundred agents it is a hundred copies of
ten bytes a frame; the rows that fold away are not drawn. A busy team
makes many rows show `█`, which says "busy" and nothing finer: the scale
is fixed on purpose, so it does not rescale to the busiest agent and
make a quiet row look loud.

**On the focus** (2026-11-02, item 7 of the motion talk). `Stirs`
keeps `STIR_LONG` (24) columns, two minutes live; `Stirs::long(tick)`
gives them all, `Stirs::at(tick)` the row's last five as before, so the
row did not change. The focus (and the peek, which draws it) has a line
after the context line, `activity: ▁▁▂▄▁…█▃ · trace lines in 2 min:
21`, dim, on the row's scale (`salon::stir_glyph`) so the two read
alike, the count summed in `u32` (24 saturated columns pass `u16`);
none while every column is empty or `:motion off`, as on the row
(`focus::activity_line`). Growth: 48 bytes and a tick per agent in
memory, as before not saved. Read cost: one copy of 24 counts per
focus drawn.

**Not done.** The bar does not tell kinds of lines apart (tool calls
from notes), which would need the trace typed: every `Agent::trace`
caller (live's sessions, the demo, imece's own notes) says what a line
is, a change across both engines for a distinction the trace view
(`enter` twice) already shows line by line.
