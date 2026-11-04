# Codex v0

Codex v0 (2026-10-29): `crates/imece-agent/src/codex.rs` drives Codex
CLI (tested against 0.165.0) as one long-lived `codex app-server`
(JSON-RPC over stdio) per agent; `imece_agent::Agent` is the trait both
vendors' sessions implement, `Event::Note` carries vendor remarks (tokens,
whose plan pays) to the trace. Codex has no tool allowlist and always a
shell, so the SANDBOX confines: `read-only` for a leader,
`workspace-write` (cwd only, `network_access=false`) for workers and
helpers, `approval_policy="never"` (a refused command just fails, never
a prompt; since Approval routing v0 part 2 a writer under standard and
open runs `on-request` and asks the person, see
`30-approval-routing.md`). Set with `-c` overrides AND on the thread: web search off,
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
once; still on → stop. Before Codex starts at all, under strict and
standard: no execpolicy rules file (`codex::rules_files`: any `*.rules`
in `$CODEX_HOME/rules`, else `~/.codex/rules`, and in the `.codex/rules`
of the session's folder and every folder above it; a `rules` folder
imece cannot read counts as one), else the session is refused with the
file's name (`codex::rules_refusal`, 2026-11-03). Probe facts (codex-cli
0.165.0): a `prefix_rule(..., decision="allow")` runs the command it
matches outside the sandbox and without asking, read-only and
`approval_policy="never"` too; a project's rules load when the person
trusts the project in their `config.toml`, and a git worktree of a
trusted checkout is trusted too; `workspace-write` cannot write
`.codex/`, so a worker cannot add one; nothing in `config/read` or the
thread says what rules there are, and app-server has no way to turn
them off (`codex exec --ignore-rules` exists, `-c ignore_rules=true`
does nothing). A private CODEX_HOME was rejected again: the login's
refresh token rotates, so a copy of `auth.json` would sign the person
out. imece never answers an approval with an execpolicy amendment
(only accept or decline). Under open the person's rules stand, like the
rest of their setup. Checked before the thread starts, fail closed:
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
an approval request of a session that may not ask (a leader, strict),
the old protocol's approvals, an elicitation or a dynamic tool call
stop the session; `item/tool/requestUserInput` is answered empty (the person is
not there; the leader is, via send). Items: `commandExecution` →
Tool `shell`, `fileChange` → Tool `apply_patch` per path (counts as a
change), `mcpToolCall` must be imece's server and an offered tool,
`webSearch`/`collabAgentToolCall`/`subAgentActivity`/`dynamicToolCall`
stop the session. Cost booked is 0 on the ChatGPT plan (a Note says so
once, and token totals per turn). Tiers are EFFORTS on Codex's default
model (`@low` / default / `@high`), a tier value is `[MODEL][@EFFORT]`;
`--model [V:]NAME` and `--tier [V:]T=NAME` take a vendor prefix, bare
ones mean the leader's vendor (`team_vendors` in `live/hiring.rs`). Known gaps:
git in a worktree writes to the main `.git` outside the sandbox (agents
are told not to commit anyway); the read-only sandbox lets a leader read
the whole disk, unlike Claude's `--restricted`; no per-session dollar cap
for Codex (the pot still stops new turns). An ignored test
(`IMECE_LIVE_CODEX=1 cargo test -p imece-agent -- --ignored live_codex`)
runs one real turn with `send` over `nc -U`.

Record 63 (2026-11-01) adds one override to every session: `tools.update_plan.enabled=true`, Codex's `update_plan` tool for the agent's own to-do list, off in an app-server thread unless asked for; `config/read` does not echo it, so `check_config` leaves it unchecked (it changes no file).

Record 105 (2026-11-04) adds `sandbox_workspace_write.writable_roots` = exactly the agent's scratch directory (none for a reader), replacing the person's own roots; `config/read` and the started thread's `sandbox.writableRoots` must echo it for a writer, or the session stops.

Record 106 (2026-11-04) adds a one-shot call, `codex::once`: a session on a `Spec` whose `ephemeral` asks `thread/start` for a thread left off the disk, checked as Codex echoes it (`thread.ephemeral`; a session that is not one-shot still must be kept). The probe of that day (codex-cli 0.165.0) found `ephemeral: true`, `path: null` and no rollout under `~/.codex/sessions`. A resumed or forked thread is never ephemeral. Every answer and failure of it costs nothing (the ChatGPT plan).
