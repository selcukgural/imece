# Confine dial v0

Confine dial v0 (2026-10-30, posture PR c; user's decision on `open`:
the person's own setup): `imece_agent::Confine { Open, Standard
(default), Strict }`, `--confine LEVEL`, `Options.confine`,
`Live.confine`, every vendor `Spec` carries it. The tool lists of the
entries above are STRICT. STANDARD: Claude keeps `--restricted` (no
settings, no hooks, file tools inside the directory) and NAMES `Bash`
(writers) and `WebFetch`, `WebSearch` in `--tools` (`--restricted`
removes them "unless --tools names them", probed on 2.1.314: a named,
allowed Bash ran `echo` with no denial) and puts them in
`--allowedTools` beside imece's tools (`ALLOWED_OUTRIGHT`; under
`--permission-prompts none` an unallowed Bash call would just be
denied); `claude::tools(write, confine)` gives the list, `init` is
still exact. Codex: `web_search="live"` and
`sandbox_workspace_write.network_access=true` (a worker's thread echoes
`networkAccess: true`, probed on 0.165.0; a leader's read-only sandbox
never has the network); features still all off. Gemini: the policy
allows `google_web_search`, `web_fetch` (everyone) and
`run_shell_command` (writers) by name next to the file tools, deny `*`
under them as before (a user-tier allow at 999 ranks above every
default policy, yolo's `allow *` included, and an ALLOW decision runs
with no prompt); `allowed_kind` lets `execute` (writers) and `fetch`
through from standard on. OPEN: Claude without `--restricted` and
`--strict-mcp-config` (the person's settings, hooks and MCP servers
load), a writer gets `--tools default --disallowedTools Agent,Task`
(probed: `--disallowedTools` takes the tool out of `init`'s list), a
reader keeps its list; `init` may report more tools and other servers,
but every tool of the level, imece's server once and connected, and no
sub-agent tool. Codex: only `features.multi_agent=false`, the person's
MCP servers, skills, plugins and hooks stay (no second start, not
checked), `web_search` the person's; approval never, sandbox and
`history.persistence="none"` as always. Gemini: no `-e none`, no
`--allowed-mcp-server-names`; the policy allows the role's tools and
imece's and denies by name only `invoke_agent`, `codebase_investigator`,
`generalist` (plus a reader's `run_shell_command`, `write_file`,
`replace`), the rest runs under the person's own policies (what asks is
still cancelled); `/extensions list` is not asked; an unknown tool
shows by its name instead of stopping the session, the kind checks
stay. Journal: `Event::Confine { leader, level }` (Assumption, worded
`ev_confine_standard` / `ev_confine_open`) once per start that is not
strict, on the project imece runs in. `abilities()` takes the level.
Gemini's shell under a policy allow was probed live on 0.65.0
(headless, `--approval-mode default`, imece's policy: one
`run_shell_command`, decision accept, no prompt). Known gaps: the ACP
path with the shell was not run end to end (free-tier latency, ~4 min
a turn); a Bash under standard needed no approval at all until (d) routed
Claude's prompts to the person (entry "Approval routing v0"); a Codex leader has web search but no network in
its shell (read-only sandbox); Claude's `NotebookEdit` and the like
come only with open.
