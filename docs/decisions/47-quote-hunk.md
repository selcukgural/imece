# Quote a hunk to its worker

`>` in a change (2026-10-31, backlog C15 in #56, from Warp, which
attaches a block of output to a prompt): the person reading a Δ saw a
hunk they wanted changed and had to tell the worker where it was in
words.

`>` in the diff view starts a message to the change's worker, as `@`
does there, with the hunk at hand quoted under the line the person
writes on: `@worker ` with the cursor after it, then a `diff` fence
holding the hunk's file's `---` and `+++` lines and the hunk itself
(`ui::quote`). The hunk at hand is the one the view's top row is in;
on a file's header (or a binary file, which has none) it is the next
one; past the last, the last. The file's names are looked for between
its `diff --git` line and its first hunk only, where no line of a hunk
(a removed `-- x` reads `--- x`) can pass for one. At most 40 lines of
the hunk go in (`QUOTE_LINES`), the rest counted as `… +N`, so a huge
hunk does not make the message huge; the worker has the whole of it
in its worktree. Every line goes through `clean`: the patch is the
worker's text and the line it goes into is drawn (control characters
out, secret-looking words hidden, also in what the worker gets back).
With no hunk to quote the flash says so and nothing is written. The
person can edit all of it before `enter`; the message is a plain one
to the worker, journaled as said.

In the diff view `>` quotes and no longer narrows the side panel; `.`
(the same key without shift) still does. The footer names `>`, the
help has its line, in both languages. The demo's changes (record 31)
quote the same way and the demo worker gets the message.

Not done (C15's other half, now C15b in #56): the last `!cmd` output.
Record 13 runs the person's command in their own terminal with the
screen aside and does not capture its output, so there is nothing to
attach yet; capturing it is a decision of its own (a pty, a size cap,
secrets in it).
