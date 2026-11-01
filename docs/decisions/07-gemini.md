# Gemini v0

Gemini v0 (2026-10-29): `crates/imece-agent/src/gemini.rs` drives Gemini
CLI (tested against 0.65.0) as one long-lived `gemini --acp` (Agent
Client Protocol v1, JSON-RPC over stdio) per agent. Gemini has no
`--restricted`; its settings come from `~/.gemini/settings.json` and
the `.gemini` directory of the folder it runs in, a system settings
file counts only from a root-owned directory, and an UNTRUSTED folder
starts no MCP server at all (so imece's bus would be gone). So: Gemini
runs in a PRIVATE directory imece owns (`<worktrees root>-gemini/<agent>`,
trusted via `GEMINI_CLI_TRUST_WORKSPACE=true`) and gets the checkout or
worktree as `--include-directories`; the repo's `.gemini` settings,
hooks and `.env` never load. Confinement is a POLICY file imece writes
there (`--policy DIR`, the user tier's only policies once named; just
root-owned system policies rank above): allow the role's file tools
(read_file, list_directory, glob, grep_search; workers and helpers also
write_file, replace; everyone write_todos since record 63, Gemini's own
to-do list, whose steps ACP 0.65 does not carry) and imece's MCP tools by their FULL names
(`mcp_imece_send`; the `mcpName` form matches when tools are shown to
the model but not when one is called) at priority 999, deny `*` at 998
(the highest matching rule wins, and a denied tool is hidden from the
model). Argv: `--approval-mode default`, `-e none` (extensions off),
`--allowed-mcp-server-names imece`, `-m MODEL`; env
`GEMINI_TELEMETRY_ENABLED=false`, `NO_BROWSER=true`. The MCP server goes
in `session/new` (`mcpServers`, stdio). Instructions: ACP has no system
prompt, so the agent's role goes in `<private dir>/GEMINI.md` (project
memory), plus a line naming its files' directory. Checks before the
first turn, fail closed: `initialize` (ACP 1, `gemini-cli`),
`session/new` (mode `default`, the model asked for), then the
zero-token commands `/about` (model, auth type), `/extensions list`
(none active) and `/memory list` (imece's GEMINI.md in use). A
`session/request_permission` is answered `cancel` and counted as
denied under strict (since Approval routing v0 part 3, under standard
and open it waits for the person and a worker's shell is `ask_user`,
see `30-approval-routing.md`); a `tool_call` of kind execute/fetch/switch_mode (or edit/delete/
move for a leader), an MCP tool not offered, or a `current_mode_update`
stops the session. A prompt starting with `/` or `$` would run as a
Gemini command, so it is prefixed with `Text:`. Cost: `session/prompt`'s
answer carries usage (input, cached, output, thought) and
`_meta.quota.model_usage`; API key (`gemini-api-key`) → list-price
table in gemini.rs (per-1M prices in thousandths of a dollar, <=200K
tier, thoughts as output, cached at the cached rate, a turn on several
models booked at the dearest), unknown model → session stops; a
turn named by the alias asked for (`auto`, `pro`, `flash`,
`flash-lite`: Gemini CLI 0.65.0 keeps the alias when no `model_info`
came, seen live on a turn with a tool call) is booked at the dearest
model the alias may mean; Google
login (`oauth-personal`) → 0 with a Note; vertex-ai/gateway refused.
Tool calls: a `tool_call`'s id carries the tool's name
(`write_file__call_7`, `mcp_imece_send__call_3`), its `title` is the
path (file tools) or `send(to: lead, …)` (MCP), `locations` the paths;
a write outside the workspace fails before it starts
(`tool_call_update` failed, "Path not in workspace"), which counts as
denied. Confirmed live on 2026-10-29 (API key, free tier): the checks,
hidden tools (a lead's model sees no shell or write tool, a worker's
no shell), `send` reaching imece's server, the outside write refused,
a repo hook in `.gemini/settings.json` not running.
Tiers: cheap=`flash`, standard=default (`auto`), strong=`pro`. Known
gaps: Gemini writes chat files under `~/.gemini/tmp`; the person's own
`~/.gemini/GEMINI.md` and user-level hooks still apply (hooks only turn
off in settings files imece can not write); the free tier's rate limits
(5 requests/min) make turns slow and retried. An ignored test
(`IMECE_LIVE_GEMINI=1 cargo test -p imece-agent -- --ignored live_gemini`)
runs one real turn with `send` over `nc -U`.
