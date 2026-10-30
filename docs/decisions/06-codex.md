# Codex v0

Codex v0 (2026-10-29): `crates/imece-agent/src/codex.rs` drives Codex
CLI (tested against 0.165.0) as one long-lived `codex app-server`
(JSON-RPC over stdio) per agent; `imece_agent::Agent` is the trait both
vendors' sessions implement, `Event::Note` carries vendor remarks (tokens,
whose plan pays) to the trace. Codex has no tool allowlist and always a
shell, so the SANDBOX confines: `read-only` for a leader,
`workspace-write` (cwd only, `network_access=false`) for workers and
helpers, `approval_policy="never"` (a refused command just fails, never
a prompt). Set with `-c` overrides AND on the thread: web search off,
features apps/plugins/browser_use/computer_use/hooks/image_generation/
multi_agent/view_image/memories/goals/tool_suggest off, `notify=[]`,
`history.persistence="none"` (Codex's global message log; the thread's
own rollout is kept, see Codex resume). Codex loads the
person's own `~/.codex` (a private CODEX_HOME was tried and rejected:
no auth, and the ChatGPT account re-installs remote plugins into it),
so MCP servers and skills that config brings can only be turned off BY
NAME: a first start reads them (`config/read`, `skills/list`) and, if
any are on, the process is started again with
`mcp_servers.<n>.enabled=false` / `skills.config=[{path,enabled=false}]`,
once; still on → stop. Checked before the thread starts, fail closed:
`account/read` (ChatGPT login only; an API key is refused because Codex
reports tokens, not dollars, and imece can not price them yet),
`config/read` echoes every override, `plugin/installed` none enabled,
`hooks/list` empty, `mcpServerStatus/list` exactly imece's server with
exactly its tools (tools listed = connected) and no tools elsewhere;
imece's server carries `default_tools_approval_mode="approve"` (checked
in the echo too): Codex asks approval for MCP tool calls, and under
`never` every unapproved call fails with "MCP tool call requires
approval" (the live smoke test found this);
`thread/start`'s answer echoes approvalPolicy/sandbox/cwd; the MCP
servers are checked again before every `turn/start`. Server requests:
any approval request or elicitation or dynamic tool call stops the
session; `item/tool/requestUserInput` is answered empty (the person is
not there; the leader is, via send). Items: `commandExecution` →
Tool `shell`, `fileChange` → Tool `apply_patch` per path (counts as a
change), `mcpToolCall` must be imece's server and an offered tool,
`webSearch`/`collabAgentToolCall`/`subAgentActivity`/`dynamicToolCall`
stop the session. Cost booked is 0 on the ChatGPT plan (a Note says so
once, and token totals per turn). Tiers are EFFORTS on Codex's default
model (`@low` / default / `@high`), a tier value is `[MODEL][@EFFORT]`;
`--model [V:]NAME` and `--tier [V:]T=NAME` take a vendor prefix, bare
ones mean the leader's vendor (`team_vendors` in live.rs). Known gaps:
git in a worktree writes to the main `.git` outside the sandbox (agents
are told not to commit anyway); the read-only sandbox lets a leader read
the whole disk, unlike Claude's `--restricted`; no per-session dollar cap
for Codex (the pot still stops new turns). An ignored test
(`IMECE_LIVE_CODEX=1 cargo test -p imece-agent -- --ignored live_codex`)
runs one real turn with `send` over `nc -U`.
