# Posture change

Posture change (2026-10-30, user's decisions after PR #32; the "Posture"
line at the top says why): the default posture moves from restricted
tools to full capability, in this order, each its own PR: (a) DONE in
PR #33: the principles above rewritten, the sandbox wording corrected
(imece sandboxes nothing), the instruction cut journaled. (b) DONE in
PR #34: agents may ask the person (entry below). (c) DONE in the
confine PR: the CONFINE DIAL, `--confine open|standard|strict`, default
`standard` = the vendor's full coding toolset for workers and helpers
(read, write, shell, web search and fetch); the vendor's own sub-agent
tool stays off at every level (it would skip the worktree, the pot, the
bus and the screen); `strict` = the tool lists of the entries above; the
fail-closed checks verify the chosen level and nothing weakens silently;
one start-up line says isolation is not imece's when not strict (entry
"Confine dial v0" below). (d) Vendor
APPROVAL REQUESTS routed to WAITING FOR YOU instead of denied or
stopping the session: Claude `--permission-prompt-tool` served by
imece's MCP server (DONE in the approvals PR, entry "Approval routing
v0", with routine commands allowed on their own), Codex approval
requests (DONE in the Codex approvals PR, same record), Gemini
`session/request_permission` (DONE in the Gemini approvals PR, same
record): (d) is done.
Then dogfooding (imece writes imece) in standard mode, with an
`AGENTS.md` under the 16 KiB cap and a known-good binary kept as
`imece-stable`.
