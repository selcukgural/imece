//! Codex as an agent: one long-lived `codex app-server` per agent (never a
//! process per turn), JSON-RPC over stdio. imece's requests go in on stdin;
//! Codex's responses, notifications and requests come out on stdout, one
//! JSON object per line.
//!
//! Codex has nothing like `--restricted` and no way to choose its built-in
//! tools: it always has a sandboxed shell and `apply_patch`. What imece can
//! set, it sets with config overrides (`-c`) on the command line and again
//! on the thread: approvals are never asked (what the sandbox refuses just
//! fails), the sandbox is read-only for a leader and workspace-write for a
//! worker (writes stay in its directory, no network), web search is off,
//! and there are no plugins, apps, hooks, sub-agents or memories, no session
//! files on disk, and no MCP server but imece's own. Codex loads the
//! person's own config as well, and some of what comes with it (MCP servers
//! that plugins put there, the system skills) can only be turned off by
//! name: a session that finds such things when it first starts is started
//! a second time with them named off. What Codex then reports it runs
//! with (`config/read`, `plugin/installed`, `skills/list`, `hooks/list`,
//! `mcpServerStatus/list`, and the thread's own settings) is checked
//! against all that before the thread starts, the MCP servers again before
//! every turn; anything else stops the session.
//!
//! Codex reports tokens, not dollars. On a ChatGPT plan the tokens cost
//! nothing beyond the plan, so a session books no cost and says so once; an
//! API-key login would cost money imece can not count yet, so it is refused.

use std::collections::{BTreeSet, VecDeque};
use std::io;
use std::path::{Path, PathBuf};
use std::sync::mpsc::{self, Receiver, RecvTimeoutError, Sender};
use std::thread;
use std::time::{Duration, Instant};

use serde_json::{Value, json};

use crate::child::{self, Child, Out};
use crate::{Event, MCP_SERVER, Mcp, mcp_tool};

/// Variables Codex may need besides [`child::BASE_ENV`]: its home (login,
/// config), an API key, and proxy and CA settings of corporate networks.
pub const ENV: &[&str] = &[
    "CODEX_HOME",
    "OPENAI_API_KEY",
    "CODEX_API_KEY",
    "HTTPS_PROXY",
    "HTTP_PROXY",
    "NO_PROXY",
    "https_proxy",
    "http_proxy",
    "no_proxy",
    "SSL_CERT_FILE",
    "SSL_CERT_DIR",
];

/// The program on PATH.
pub const PROGRAM: &str = "codex";

/// Codex's tiers are reasoning efforts on its default model, since the
/// default is already its fast model and effort is what Codex itself turns:
/// `[MODEL][@EFFORT]`, so `--tier codex:strong=gpt-5.6-terra@xhigh` names
/// both. `Standard` is Codex's own default model at its default effort.
pub fn model(tier: crate::Tier) -> Option<&'static str> {
    match tier {
        crate::Tier::Cheap => Some("@low"),
        crate::Tier::Standard => None,
        crate::Tier::Strong => Some("@high"),
    }
}

/// The model and the reasoning effort in a `[MODEL][@EFFORT]` choice; none
/// for Codex's own default.
pub fn choice(s: &str) -> (Option<String>, Option<String>) {
    let (model, effort) = s.split_once('@').unwrap_or((s, ""));
    let some = |s: &str| (!s.is_empty()).then(|| s.to_string());
    (some(model), some(effort))
}

/// Config every session gets (`-c key=value`, TOML values) and that
/// `config/read` must echo. Features and the sandbox come on top.
const OVERRIDES: &[(&str, &str)] = &[
    ("approval_policy", "\"never\""),
    ("web_search", "\"disabled\""),
    ("notify", "[]"),
    ("history.persistence", "\"none\""),
    ("check_for_update_on_startup", "false"),
    ("sandbox_workspace_write.network_access", "false"),
];

/// Features turned off: other tools and other agents, and what reaches
/// out of the session.
const FEATURES_OFF: &[&str] = &[
    "apps",
    "plugins",
    "browser_use",
    "computer_use",
    "hooks",
    "image_generation",
    "multi_agent",
    "view_image",
    "memories",
    "goals",
    "tool_suggest",
];

/// Notifications nobody here reads: streamed deltas and account noise.
const QUIET: &[&str] = &[
    "item/agentMessage/delta",
    "item/reasoning/summaryTextDelta",
    "item/reasoning/summaryPartAdded",
    "item/reasoning/textDelta",
    "item/commandExecution/outputDelta",
    "item/fileChange/outputDelta",
    "item/plan/delta",
    "turn/diff/updated",
    "account/rateLimits/updated",
    "remoteControl/status/changed",
    "thread/readState/changed",
];

/// How long an answer to one of imece's requests may take. A turn itself
/// has no limit; its `turn/start` is answered at once.
const REQUEST_WAIT: Duration = Duration::from_secs(60);
const POLL: Duration = Duration::from_millis(500);

/// The names the shell and the patch tool have on the screen.
pub const SHELL: &str = "shell";
pub const PATCH: &str = "apply_patch";

/// Codex's approval mode for an MCP server's tools that asks nothing
/// (`default_tools_approval_mode`; the others are auto, prompt, writes).
const APPROVED: &str = "approve";

#[derive(Clone, Debug)]
pub struct Spec {
    /// `codex`, or a stand-in in tests.
    pub program: String,
    /// The agent's directory: the thread's `cwd`, and what the sandbox lets
    /// a worker write.
    pub cwd: PathBuf,
    /// `[MODEL][@EFFORT]` (see [`choice`]), or none for Codex's own default.
    pub choice: Option<String>,
    /// Whether the agent changes files (workspace-write) or only reads them
    /// (read-only).
    pub write: bool,
    /// The thread's developer instructions: the agent's role in imece.
    pub system: String,
    /// imece's MCP server, if the agent gets its tools.
    pub mcp: Option<Mcp>,
}

/// What a first start found in the person's config that only a name turns
/// off: MCP servers (by name) and skills (by path).
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Off {
    pub servers: Vec<String>,
    pub skills: Vec<String>,
}

impl Off {
    fn is_empty(&self) -> bool {
        self.servers.is_empty() && self.skills.is_empty()
    }
}

fn sandbox(write: bool) -> &'static str {
    if write {
        "workspace-write"
    } else {
        "read-only"
    }
}

/// `s` as a TOML basic string.
fn toml_str(s: &str) -> String {
    let mut out = String::with_capacity(s.len() + 2);
    out.push('"');
    for c in s.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            c if c.is_control() => out.push_str(&format!("\\u{:04X}", c as u32)),
            c => out.push(c),
        }
    }
    out.push('"');
    out
}

/// `s` as one segment of a TOML dotted key: bare when it can be.
fn toml_key(s: &str) -> String {
    if !s.is_empty()
        && s.bytes()
            .all(|b| b.is_ascii_alphanumeric() || b == b'-' || b == b'_')
    {
        s.to_string()
    } else {
        toml_str(s)
    }
}

/// The command line for `spec`, with `off` named off. Prompts never go
/// here; they go to the thread.
pub fn args(spec: &Spec, off: &Off) -> Vec<String> {
    let mut a = vec!["app-server".to_string()];
    let mut set = |k: String, v: String| {
        a.push("-c".into());
        a.push(format!("{k}={v}"));
    };
    for (k, v) in OVERRIDES {
        set(k.to_string(), v.to_string());
    }
    for f in FEATURES_OFF {
        set(format!("features.{f}"), "false".into());
    }
    set("sandbox_mode".into(), toml_str(sandbox(spec.write)));
    if let Some(m) = &spec.mcp {
        set(
            format!("mcp_servers.{MCP_SERVER}.command"),
            toml_str(&m.command),
        );
        // imece's tools need nobody's approval: they act inside imece,
        // which decides for itself. Without this, Codex would ask, and
        // under `never` every call would fail.
        set(
            format!("mcp_servers.{MCP_SERVER}.default_tools_approval_mode"),
            toml_str(APPROVED),
        );
        set(
            format!("mcp_servers.{MCP_SERVER}.args"),
            format!(
                "[{}]",
                m.args
                    .iter()
                    .map(|s| toml_str(s))
                    .collect::<Vec<_>>()
                    .join(",")
            ),
        );
    }
    for s in &off.servers {
        set(
            format!("mcp_servers.{}.enabled", toml_key(s)),
            "false".into(),
        );
    }
    if !off.skills.is_empty() {
        set(
            "skills.config".into(),
            format!(
                "[{}]",
                off.skills
                    .iter()
                    .map(|p| format!("{{path={},enabled=false}}", toml_str(p)))
                    .collect::<Vec<_>>()
                    .join(",")
            ),
        );
    }
    a
}

/// What the driver does with what the protocol decided.
#[derive(Clone, Debug, PartialEq)]
pub enum Act {
    Event(Event),
    /// One line for Codex's stdin.
    Send(String),
    /// Start Codex again with these named off, and check again.
    Restart(Off),
}

/// The answer the protocol waits for.
#[derive(Clone, Copy, Debug, PartialEq)]
enum Wait {
    Initialize,
    Account,
    Config,
    Skills,
    Plugins,
    Hooks,
    Servers,
    Thread,
    /// The MCP servers, before a turn.
    Recheck,
    Turn,
}

/// Turns stdout lines into [`Act`]s and checks the settings Codex reports.
pub struct Protocol {
    cwd: PathBuf,
    write: bool,
    model: Option<String>,
    effort: Option<String>,
    system: String,
    mcp: Option<Mcp>,
    /// Tools imece's server offers, as Codex names them.
    tools: BTreeSet<String>,
    /// What a first start found and this start has named off, if this is
    /// the second start: nothing of it may be left.
    off: Option<Off>,
    found: Off,
    next_id: u64,
    awaiting: Option<(u64, Wait)>,
    thread: Option<String>,
    started: bool,
    queue: VecDeque<String>,
    busy: bool,
    /// The turn so far: the agent's last message, the last error, refused
    /// tool calls, and the session's tokens (input, cached, output).
    text: String,
    error: Option<String>,
    denied: usize,
    tokens: Option<(u64, u64, u64)>,
}

impl Protocol {
    /// For `spec`; `off` is what the first start found, on the second.
    pub fn new(spec: &Spec, off: Option<Off>) -> Protocol {
        let (model, effort) = spec.choice.as_deref().map_or((None, None), choice);
        Protocol {
            cwd: spec.cwd.clone(),
            write: spec.write,
            model,
            effort,
            system: spec.system.clone(),
            mcp: spec.mcp.clone(),
            tools: spec
                .mcp
                .iter()
                .flat_map(|m| m.tools.iter().map(|t| mcp_tool(t)))
                .collect(),
            off,
            found: Off::default(),
            next_id: 0,
            awaiting: None,
            thread: None,
            started: false,
            queue: VecDeque::new(),
            busy: false,
            text: String::new(),
            error: None,
            denied: 0,
            tokens: None,
        }
    }

    /// Whether an answer from Codex is due.
    pub fn waiting(&self) -> bool {
        self.awaiting.is_some()
    }

    /// The first request: `initialize`.
    pub fn open(&mut self) -> Vec<Act> {
        vec![self.request(
            "initialize",
            json!({
                "clientInfo": {
                    "name": "imece",
                    "title": "imece",
                    "version": env!("CARGO_PKG_VERSION"),
                },
                "capabilities": { "optOutNotificationMethods": QUIET },
            }),
            Wait::Initialize,
        )]
    }

    fn request(&mut self, method: &str, params: Value, wait: Wait) -> Act {
        self.next_id += 1;
        self.awaiting = Some((self.next_id, wait));
        Act::Send(
            json!({ "jsonrpc": "2.0", "id": self.next_id, "method": method, "params": params })
                .to_string(),
        )
    }

    /// Queues a prompt; it starts a turn as soon as the thread is free.
    pub fn prompt(&mut self, text: &str) -> Vec<Act> {
        self.queue.push_back(text.to_string());
        self.next_turn()
    }

    /// Starts the next queued turn, if the thread is free: the MCP servers
    /// are checked first.
    fn next_turn(&mut self) -> Vec<Act> {
        if !self.started || self.busy || self.awaiting.is_some() || self.queue.is_empty() {
            return Vec::new();
        }
        vec![self.request("mcpServerStatus/list", json!({}), Wait::Recheck)]
    }

    /// The acts for one stdout line, or why the session must stop.
    pub fn line(&mut self, line: &str) -> Result<Vec<Act>, String> {
        if line.trim().is_empty() {
            return Ok(Vec::new());
        }
        let v: Value =
            serde_json::from_str(line).map_err(|e| format!("spoke no JSON-RPC ({e})"))?;
        match (v.get("method").and_then(Value::as_str), v.get("id")) {
            (Some(method), Some(id)) => self.asked(id.clone(), method, &v["params"]),
            (Some(method), None) => self.notified(method, &v["params"]),
            (None, Some(id)) => self.answered(id, &v),
            (None, None) => Err("spoke no JSON-RPC (neither method nor id)".into()),
        }
    }

    /// Codex answered request `id`.
    fn answered(&mut self, id: &Value, v: &Value) -> Result<Vec<Act>, String> {
        let Some((want, wait)) = self.awaiting else {
            return Err("answered a request nobody made".into());
        };
        if id.as_u64() != Some(want) {
            return Err(format!("answered request {id}, not {want}"));
        }
        self.awaiting = None;
        if let Some(e) = v.get("error") {
            let msg = e["message"].as_str().unwrap_or("unknown error");
            return Err(format!("refused {wait:?}: {msg}"));
        }
        let r = &v["result"];
        Ok(match wait {
            Wait::Initialize => vec![self.request("account/read", json!({}), Wait::Account)],
            Wait::Account => {
                match r["account"]["type"].as_str() {
                    Some("chatgpt") => {}
                    Some(other) => {
                        return Err(format!(
                            "is logged in with {other}, whose cost imece can not count yet; \
                             log in with ChatGPT (codex login)"
                        ));
                    }
                    None => return Err("is not logged in; run codex login".into()),
                }
                vec![self.request("config/read", json!({}), Wait::Config)]
            }
            Wait::Config => {
                self.found.servers = self.check_config(&r["config"])?;
                vec![self.request("skills/list", json!({}), Wait::Skills)]
            }
            Wait::Skills => {
                self.found.skills = enabled_skills(r);
                if !self.found.is_empty() {
                    if let Some(off) = &self.off {
                        return Err(format!(
                            "still has MCP servers {:?} and skills {:?} on after they were \
                             turned off ({:?})",
                            self.found.servers, self.found.skills, off
                        ));
                    }
                    return Ok(vec![Act::Restart(std::mem::take(&mut self.found))]);
                }
                vec![self.request("plugin/installed", json!({}), Wait::Plugins)]
            }
            Wait::Plugins => {
                let on: Vec<&str> = r["marketplaces"]
                    .as_array()
                    .into_iter()
                    .flatten()
                    .flat_map(|m| m["plugins"].as_array().into_iter().flatten())
                    .filter(|p| p["enabled"] == true)
                    .map(|p| p["id"].as_str().unwrap_or("?"))
                    .collect();
                if !on.is_empty() {
                    return Err(format!("has plugins on: {on:?}"));
                }
                vec![self.request("hooks/list", json!({}), Wait::Hooks)]
            }
            Wait::Hooks => {
                let hooks = r["data"]
                    .as_array()
                    .into_iter()
                    .flatten()
                    .flat_map(|d| d["hooks"].as_array().into_iter().flatten())
                    .count();
                if hooks > 0 {
                    return Err(format!("has {hooks} hooks"));
                }
                vec![self.request("mcpServerStatus/list", json!({}), Wait::Servers)]
            }
            Wait::Servers => {
                self.check_servers(r)?;
                let mut params = json!({
                    "cwd": self.cwd,
                    "approvalPolicy": "never",
                    "sandbox": sandbox(self.write),
                    "developerInstructions": self.system,
                    "ephemeral": true,
                });
                if let Some(m) = &self.model {
                    params["model"] = json!(m);
                }
                vec![self.request("thread/start", params, Wait::Thread)]
            }
            Wait::Thread => {
                self.check_thread(r)?;
                self.thread = Some(r["thread"]["id"].as_str().unwrap_or_default().to_string());
                self.started = true;
                let mut acts = vec![
                    Act::Event(Event::Started {
                        session: self.thread.clone().unwrap_or_default(),
                        model: r["model"]
                            .as_str()
                            .or(r["thread"]["model"].as_str())
                            .unwrap_or_default()
                            .to_string(),
                    }),
                    Act::Event(Event::Note(
                        "codex runs on your ChatGPT plan: its turns cost the budget nothing".into(),
                    )),
                ];
                acts.extend(self.next_turn());
                acts
            }
            Wait::Recheck => {
                self.check_servers(r)?;
                let text = self.queue.pop_front().unwrap_or_default();
                self.busy = true;
                self.text.clear();
                self.error = None;
                self.denied = 0;
                let mut params = json!({
                    "threadId": self.thread,
                    "input": [{ "type": "text", "text": text }],
                });
                if let Some(e) = &self.effort {
                    params["effort"] = json!(e);
                }
                vec![self.request("turn/start", params, Wait::Turn)]
            }
            Wait::Turn => Vec::new(),
        })
    }

    /// The `config/read` result must echo what was set; the MCP servers
    /// still on besides imece's are returned, to be named off.
    fn check_config(&self, c: &Value) -> Result<Vec<String>, String> {
        let want = [
            ("approval_policy", json!("never")),
            ("web_search", json!("disabled")),
            ("notify", json!([])),
            ("check_for_update_on_startup", json!(false)),
            ("sandbox_mode", json!(sandbox(self.write))),
        ];
        for (k, v) in &want {
            if &c[k] != v {
                return Err(format!("runs with {k} = {}", c[k]));
            }
        }
        if c["history"]["persistence"] != "none" {
            return Err(format!(
                "runs with history.persistence = {}",
                c["history"]["persistence"]
            ));
        }
        if self.write && c["sandbox_workspace_write"]["network_access"] != false {
            return Err("runs with network access in the sandbox".into());
        }
        for f in FEATURES_OFF {
            if c["features"][f] != false {
                return Err(format!("runs with the {f} feature on"));
            }
        }
        let mut on = Vec::new();
        for (name, s) in c["mcp_servers"].as_object().into_iter().flatten() {
            if name == MCP_SERVER {
                match &self.mcp {
                    Some(m)
                        if s["command"] == m.command
                            && s["args"] == json!(m.args)
                            && s["default_tools_approval_mode"] == APPROVED => {}
                    Some(_) => return Err("has another imece MCP server".into()),
                    None => return Err("has an MCP server it was not given".into()),
                }
            } else if s["enabled"] != false {
                on.push(name.clone());
            }
        }
        if self.mcp.is_some()
            && !c["mcp_servers"]
                .get(MCP_SERVER)
                .is_some_and(|s| s.is_object())
        {
            return Err("has no imece MCP server".into());
        }
        Ok(on)
    }

    /// `mcpServerStatus/list` must show imece's server with exactly its
    /// tools (so it is connected), and no tools anywhere else.
    fn check_servers(&self, r: &Value) -> Result<(), String> {
        let mut ours = false;
        for s in r["data"].as_array().into_iter().flatten() {
            let name = s["name"].as_str().unwrap_or_default();
            let tools: BTreeSet<String> = s["tools"]
                .as_object()
                .into_iter()
                .flatten()
                .map(|(k, _)| mcp_tool(k))
                .collect();
            if name == MCP_SERVER && self.mcp.is_some() {
                if tools != self.tools {
                    return Err(format!(
                        "imece's MCP server offers {tools:?}, not {:?}",
                        self.tools
                    ));
                }
                ours = true;
            } else if !tools.is_empty() {
                return Err(format!(
                    "has MCP server {name:?} with {} tools",
                    tools.len()
                ));
            }
        }
        if self.mcp.is_some() && !ours {
            return Err("has no connected imece MCP server".into());
        }
        Ok(())
    }

    /// The thread Codex started must be the one asked for.
    fn check_thread(&self, r: &Value) -> Result<(), String> {
        if r["approvalPolicy"] != "never" {
            return Err(format!(
                "started a thread asking approvals ({})",
                r["approvalPolicy"]
            ));
        }
        let kind = if self.write {
            "workspaceWrite"
        } else {
            "readOnly"
        };
        if r["sandbox"]["type"] != kind {
            return Err(format!("started a thread in sandbox {}", r["sandbox"]));
        }
        if r["sandbox"]["networkAccess"] == true {
            return Err("started a thread with network access".into());
        }
        if r["cwd"].as_str().map(Path::new) != Some(self.cwd.as_path()) {
            return Err(format!("started a thread in {}", r["cwd"]));
        }
        Ok(())
    }

    /// Codex asked something of imece (a request with an id).
    fn asked(&mut self, id: Value, method: &str, params: &Value) -> Result<Vec<Act>, String> {
        match method {
            // The model asked the person a question through its tool: the
            // person is not here; its leader is, through send.
            "item/tool/requestUserInput" => {
                let answers: serde_json::Map<String, Value> = params["questions"]
                    .as_array()
                    .into_iter()
                    .flatten()
                    .filter_map(|q| q["id"].as_str())
                    .map(|q| (q.to_string(), json!({ "answers": [] })))
                    .collect();
                Ok(vec![Act::Send(
                    json!({ "jsonrpc": "2.0", "id": id, "result": { "answers": answers } })
                        .to_string(),
                )])
            }
            "item/commandExecution/requestApproval"
            | "item/fileChange/requestApproval"
            | "item/permissions/requestApproval"
            | "applyPatchApproval"
            | "execCommandApproval" => {
                Err("asked for an approval although none may be asked".into())
            }
            "mcpServer/elicitation/request" => Err("had an MCP server ask something".into()),
            "item/tool/call" => Err("called a tool it was not given".into()),
            _ => Ok(vec![Act::Send(
                json!({ "jsonrpc": "2.0", "id": id, "error": {
                    "code": -32601, "message": format!("imece does not answer {method}") } })
                .to_string(),
            )]),
        }
    }

    /// Codex told something (a request without an id).
    fn notified(&mut self, method: &str, p: &Value) -> Result<Vec<Act>, String> {
        if !self.started && (method.starts_with("item/") || method.starts_with("turn/")) {
            return Err(format!("sent {method} before its settings were checked"));
        }
        match method {
            "item/started" => self.item(&p["item"], false),
            "item/completed" => self.item(&p["item"], true),
            "error" => {
                if p["willRetry"] != true
                    && let Some(m) = p["error"]["message"].as_str()
                {
                    self.error = Some(m.to_string());
                }
                Ok(Vec::new())
            }
            "thread/tokenUsage/updated" => {
                let t = &p["tokenUsage"]["total"];
                let n = |k: &str| t[k].as_u64().unwrap_or(0);
                self.tokens = Some((n("inputTokens"), n("cachedInputTokens"), n("outputTokens")));
                Ok(Vec::new())
            }
            "turn/completed" => {
                let status = p["turn"]["status"].as_str().unwrap_or("unknown");
                let ok = status == "completed";
                let text = if ok {
                    self.text.trim().to_string()
                } else {
                    p["turn"]["error"]["message"]
                        .as_str()
                        .map(str::to_string)
                        .or_else(|| self.error.take())
                        .unwrap_or_else(|| status.to_string())
                };
                self.busy = false;
                let mut acts = vec![Act::Event(Event::TurnDone {
                    ok,
                    text,
                    cost: 0,
                    denied: self.denied,
                })];
                if let Some((input, cached, output)) = self.tokens {
                    acts.push(Act::Event(Event::Note(format!(
                        "tokens so far: {input} in ({cached} cached) · {output} out"
                    ))));
                }
                acts.extend(self.next_turn());
                Ok(acts)
            }
            _ => Ok(Vec::new()),
        }
    }

    /// One item of the turn, as it starts or as it ends.
    fn item(&mut self, item: &Value, completed: bool) -> Result<Vec<Act>, String> {
        let status = item["status"].as_str().unwrap_or_default();
        let failed = |name: &str, out: &mut Vec<Act>| {
            if matches!(status, "failed" | "declined") {
                out.push(Act::Event(Event::ToolFailed { name: name.into() }));
            }
        };
        let mut out = Vec::new();
        match item["type"].as_str().unwrap_or_default() {
            "agentMessage" => {
                if completed {
                    let text = item["text"].as_str().unwrap_or_default().trim();
                    if !text.is_empty() {
                        self.text = text.to_string();
                        out.push(Act::Event(Event::Text(text.to_string())));
                    }
                }
            }
            "commandExecution" => {
                if completed {
                    failed(SHELL, &mut out);
                    if status == "declined" {
                        self.denied += 1;
                    }
                } else {
                    out.push(Act::Event(Event::Tool {
                        name: SHELL.into(),
                        target: one_line(item["command"].as_str().unwrap_or_default()),
                    }));
                }
            }
            "fileChange" => {
                if completed {
                    failed(PATCH, &mut out);
                    if status == "declined" {
                        self.denied += 1;
                    }
                } else {
                    for c in item["changes"].as_array().into_iter().flatten() {
                        out.push(Act::Event(Event::Tool {
                            name: PATCH.into(),
                            target: relative(c["path"].as_str().unwrap_or_default(), &self.cwd),
                        }));
                    }
                }
            }
            "mcpToolCall" => {
                let server = item["server"].as_str().unwrap_or_default();
                let name = mcp_tool(item["tool"].as_str().unwrap_or_default());
                if server != MCP_SERVER || !self.tools.contains(&name) {
                    return Err(format!(
                        "used tool {}/{} it was not given",
                        server,
                        item["tool"].as_str().unwrap_or_default()
                    ));
                }
                if completed {
                    failed(&name, &mut out);
                    // Why imece's own tool failed is worth a line: the
                    // call may never have reached imece.
                    let why = match &item["error"] {
                        Value::String(s) => s.clone(),
                        Value::Null => String::new(),
                        e => e["message"]
                            .as_str()
                            .map_or_else(|| e.to_string(), str::to_string),
                    };
                    if status == "failed" && !why.trim().is_empty() {
                        out.push(Act::Event(Event::Note(format!(
                            "{name} failed: {}",
                            one_line(&why)
                        ))));
                    }
                } else {
                    let a = &item["arguments"];
                    let target = a["to"]
                        .as_str()
                        .or(a["title"].as_str())
                        .map(str::to_string)
                        .or_else(|| a["card"].as_u64().map(|n| n.to_string()))
                        .unwrap_or_default();
                    out.push(Act::Event(Event::Tool { name, target }));
                }
            }
            "webSearch" => return Err("searched the web".into()),
            "collabAgentToolCall" | "subAgentActivity" => {
                return Err("started an agent of its own".into());
            }
            "dynamicToolCall" => return Err("called a tool it was not given".into()),
            // Reasoning, plans, the user's own message, compaction: nothing
            // the screen needs.
            _ => {}
        }
        Ok(out)
    }
}

/// Paths of the skills Codex would offer the model.
fn enabled_skills(r: &Value) -> Vec<String> {
    r["data"]
        .as_array()
        .into_iter()
        .flatten()
        .flat_map(|d| d["skills"].as_array().into_iter().flatten())
        .filter(|s| s["enabled"] == true)
        .filter_map(|s| s["path"].as_str())
        .map(str::to_string)
        .collect()
}

/// A command in a few characters: its first line, cut short.
fn one_line(command: &str) -> String {
    let line = command.lines().next().unwrap_or_default().trim();
    let mut out: String = line.chars().take(120).collect();
    if line.chars().count() > 120 {
        out.push('…');
    }
    out
}

/// `path` relative to the agent's directory, when it is inside.
fn relative(path: &str, cwd: &Path) -> String {
    let p = Path::new(path);
    p.strip_prefix(cwd)
        .unwrap_or(p)
        .to_string_lossy()
        .into_owned()
}

/// What reaches the driver thread.
enum Msg {
    /// A stdout line of the Codex process of this generation.
    Line(u64, String),
    /// The Codex process of this generation is gone.
    End(u64, String),
    Prompt(String),
    Terminate,
}

/// A running Codex session.
pub struct Session {
    tx: Sender<Msg>,
}

fn spawn(spec: &Spec, off: &Off, tx: Sender<Msg>, generation: u64) -> io::Result<Child> {
    let mut cmd = child::command(&spec.program, &spec.cwd, ENV);
    cmd.args(args(spec, off));
    Child::spawn(cmd, move |out| {
        tx.send(match out {
            Out::Line(l) => Msg::Line(generation, l.to_string()),
            Out::End(why) => Msg::End(generation, why),
        })
        .is_ok()
    })
}

impl Session {
    /// Starts the session. `on` gets its events on the driver thread; the
    /// last one is [`Event::Stopped`], unless the session was terminated.
    pub fn start(spec: &Spec, on: impl FnMut(Event) + Send + 'static) -> io::Result<Session> {
        let (tx, rx) = mpsc::channel();
        let child = spawn(spec, &Off::default(), tx.clone(), 0)?;
        let spec = spec.clone();
        let driver = tx.clone();
        thread::spawn(move || drive(spec, child, rx, driver, on));
        Ok(Session { tx })
    }

    /// Gives the agent a prompt. A busy thread takes it after the running
    /// turn.
    pub fn prompt(&mut self, text: &str) -> io::Result<()> {
        self.tx
            .send(Msg::Prompt(text.to_string()))
            .map_err(|_| io::Error::new(io::ErrorKind::BrokenPipe, "the session is over"))
    }

    /// Asks the session to stop without waiting.
    pub fn terminate(&mut self) {
        let _ = self.tx.send(Msg::Terminate);
    }
}

impl Drop for Session {
    fn drop(&mut self) {
        self.terminate();
    }
}

impl crate::Agent for Session {
    fn prompt(&mut self, text: &str) -> io::Result<()> {
        Session::prompt(self, text)
    }

    fn terminate(&mut self) {
        Session::terminate(self)
    }
}

/// Runs the protocol against the Codex process, starting it a second time
/// when the first start found things to name off.
fn drive(spec: Spec, child: Child, rx: Receiver<Msg>, tx: Sender<Msg>, mut on: impl FnMut(Event)) {
    let mut generation = 0;
    let mut child = Some(child);
    let mut protocol = Protocol::new(&spec, None);
    let mut acts = protocol.open();
    let mut deadline: Option<Instant> = None;
    loop {
        for act in acts.drain(..) {
            match act {
                Act::Event(e) => on(e),
                Act::Send(line) => {
                    let sent = child.as_mut().map_or(Ok(()), |c| c.write_line(&line));
                    if let Err(e) = sent {
                        on(Event::Stopped(format!(
                            "stopped: codex took no input ({e})"
                        )));
                        return;
                    }
                }
                Act::Restart(off) => {
                    // Dropping the first process kills it; its last words
                    // arrive with its generation and are ignored.
                    drop(child.take());
                    generation += 1;
                    // The prompts given so far wait for the second start.
                    let queue = std::mem::take(&mut protocol.queue);
                    protocol = Protocol::new(&spec, Some(off.clone()));
                    protocol.queue = queue;
                    match spawn(&spec, &off, tx.clone(), generation) {
                        Ok(c) => child = Some(c),
                        Err(e) => {
                            on(Event::Stopped(format!("{}: {e}", spec.program)));
                            return;
                        }
                    }
                    let open = protocol.open();
                    for a in open {
                        if let Act::Send(line) = a
                            && let Err(e) = child.as_mut().unwrap().write_line(&line)
                        {
                            on(Event::Stopped(format!(
                                "stopped: codex took no input ({e})"
                            )));
                            return;
                        }
                    }
                }
            }
        }
        deadline = protocol
            .waiting()
            .then(|| deadline.unwrap_or_else(|| Instant::now() + REQUEST_WAIT));
        match rx.recv_timeout(POLL) {
            Ok(Msg::Line(g, line)) if g == generation => {
                deadline = None;
                match protocol.line(&line) {
                    Ok(a) => acts = a,
                    Err(why) => {
                        on(Event::Stopped(format!("stopped: the session {why}")));
                        return;
                    }
                }
            }
            Ok(Msg::End(g, why)) if g == generation => {
                on(Event::Stopped(why));
                return;
            }
            Ok(Msg::Line(..) | Msg::End(..)) => {}
            Ok(Msg::Prompt(text)) => acts = protocol.prompt(&text),
            Ok(Msg::Terminate) | Err(RecvTimeoutError::Disconnected) => return,
            Err(RecvTimeoutError::Timeout) => {
                if deadline.is_some_and(|d| Instant::now() >= d) {
                    on(Event::Stopped(
                        "stopped: codex did not answer in time".to_string(),
                    ));
                    return;
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn spec(write: bool) -> Spec {
        Spec {
            program: "codex".into(),
            cwd: PathBuf::from("/work/w1"),
            choice: None,
            write,
            system: "You are codex-1.".into(),
            mcp: None,
        }
    }

    fn with_bus(write: bool) -> Spec {
        let mut s = spec(write);
        s.mcp = Some(Mcp {
            command: "/opt/imece".into(),
            args: vec!["mcp-relay".into(), "/tmp/i/a1.sock".into()],
            tools: vec!["send".into(), "post".into()],
        });
        s
    }

    #[test]
    fn the_tiers_are_efforts_on_codex_s_default_model() {
        assert_eq!(model(crate::Tier::Cheap), Some("@low"));
        assert_eq!(model(crate::Tier::Standard), None);
        assert_eq!(model(crate::Tier::Strong), Some("@high"));
        assert_eq!(PROGRAM, "codex");
        assert_eq!(choice("@low"), (None, Some("low".into())));
        assert_eq!(
            choice("gpt-5.6-terra"),
            (Some("gpt-5.6-terra".into()), None)
        );
        assert_eq!(
            choice("gpt-5.6-terra@xhigh"),
            (Some("gpt-5.6-terra".into()), Some("xhigh".into()))
        );
        assert_eq!(choice(""), (None, None));
    }

    #[test]
    fn toml_strings_and_keys_are_quoted_as_toml_wants() {
        assert_eq!(toml_str(r#"a"b\c"#), r#""a\"b\\c""#);
        assert_eq!(toml_str("tab\there"), r#""tab\u0009here""#);
        assert_eq!(toml_key("node_repl"), "node_repl");
        assert_eq!(toml_key("computer-use"), "computer-use");
        assert_eq!(toml_key("my server"), r#""my server""#);
        assert_eq!(toml_key(""), r#""""#);
    }

    fn after<'a>(a: &'a [String], key: &str) -> Option<&'a str> {
        a.iter()
            .filter_map(|x| x.strip_prefix(&format!("{key}=")))
            .next()
    }

    #[test]
    fn the_command_line_locks_the_session_down_and_carries_no_prompt() {
        let a = args(&with_bus(true), &Off::default());
        assert_eq!(a[0], "app-server");
        assert_eq!(after(&a, "approval_policy"), Some("\"never\""));
        assert_eq!(after(&a, "sandbox_mode"), Some("\"workspace-write\""));
        assert_eq!(after(&a, "web_search"), Some("\"disabled\""));
        assert_eq!(after(&a, "notify"), Some("[]"));
        assert_eq!(after(&a, "history.persistence"), Some("\"none\""));
        assert_eq!(
            after(&a, "sandbox_workspace_write.network_access"),
            Some("false")
        );
        for f in FEATURES_OFF {
            assert_eq!(after(&a, &format!("features.{f}")), Some("false"), "{f}");
        }
        assert_eq!(
            after(&a, "mcp_servers.imece.command"),
            Some("\"/opt/imece\"")
        );
        assert_eq!(
            after(&a, "mcp_servers.imece.default_tools_approval_mode"),
            Some("\"approve\"")
        );
        assert_eq!(
            after(&a, "mcp_servers.imece.args"),
            Some(r#"["mcp-relay","/tmp/i/a1.sock"]"#)
        );
        // Every override is a -c pair.
        assert_eq!(
            a[1..].iter().filter(|x| *x == "-c").count(),
            (a.len() - 1) / 2
        );
        assert!(
            !a.iter()
                .any(|x| x.contains("dangerously") || x.contains("You are"))
        );

        let reader = args(&spec(false), &Off::default());
        assert_eq!(after(&reader, "sandbox_mode"), Some("\"read-only\""));
        assert!(after(&reader, "mcp_servers.imece.command").is_none());
    }

    #[test]
    fn what_a_first_start_found_is_named_off_on_the_second() {
        let off = Off {
            servers: vec!["node_repl".into(), "my server".into()],
            skills: vec!["/h/.codex/skills/.system/imagegen/SKILL.md".into()],
        };
        let a = args(&spec(true), &off);
        assert_eq!(after(&a, "mcp_servers.node_repl.enabled"), Some("false"));
        assert_eq!(
            after(&a, "mcp_servers.\"my server\".enabled"),
            Some("false")
        );
        assert_eq!(
            after(&a, "skills.config"),
            Some(r#"[{path="/h/.codex/skills/.system/imagegen/SKILL.md",enabled=false}]"#)
        );
        assert!(after(&args(&spec(true), &Off::default()), "skills.config").is_none());
    }

    // --- The protocol, one answer at a time. ---

    fn sent(acts: &[Act]) -> Vec<Value> {
        acts.iter()
            .filter_map(|a| match a {
                Act::Send(l) => Some(serde_json::from_str(l).unwrap()),
                _ => None,
            })
            .collect()
    }

    fn events(acts: &[Act]) -> Vec<Event> {
        acts.iter()
            .filter_map(|a| match a {
                Act::Event(e) => Some(e.clone()),
                _ => None,
            })
            .collect()
    }

    fn reply(id: u64, result: Value) -> String {
        json!({ "jsonrpc": "2.0", "id": id, "result": result }).to_string()
    }

    fn notify(method: &str, params: Value) -> String {
        json!({ "jsonrpc": "2.0", "method": method, "params": params }).to_string()
    }

    /// The config Codex reports with every override in place; `servers`
    /// are extra MCP servers in the person's config.
    fn good_config(write: bool, servers: &[(&str, bool)], bus: bool) -> Value {
        let mut features = serde_json::Map::new();
        for f in FEATURES_OFF {
            features.insert(f.to_string(), json!(false));
        }
        features.insert("shell_tool".into(), json!(true));
        let mut mcp = serde_json::Map::new();
        for (name, on) in servers {
            mcp.insert(name.to_string(), json!({ "command": "x", "enabled": on }));
        }
        if bus {
            mcp.insert(
                "imece".into(),
                json!({
                    "command": "/opt/imece",
                    "args": ["mcp-relay", "/tmp/i/a1.sock"],
                    "default_tools_approval_mode": "approve",
                }),
            );
        }
        json!({
            "approval_policy": "never",
            "web_search": "disabled",
            "notify": [],
            "check_for_update_on_startup": false,
            "sandbox_mode": sandbox(write),
            "sandbox_workspace_write": { "network_access": false },
            "history": { "persistence": "none" },
            "features": features,
            "mcp_servers": mcp,
        })
    }

    fn servers(bus: bool, others: &[(&str, usize)]) -> Value {
        let mut data = Vec::new();
        if bus {
            data.push(json!({ "name": "imece", "tools": { "send": {}, "post": {} } }));
        }
        for (name, n) in others {
            let tools: serde_json::Map<String, Value> =
                (0..*n).map(|i| (format!("t{i}"), json!({}))).collect();
            data.push(json!({ "name": name, "tools": tools }));
        }
        json!({ "data": data })
    }

    fn thread(write: bool, cwd: &str) -> Value {
        json!({
            "thread": { "id": "t1", "model": "gpt-6-luna" },
            "model": "gpt-6-luna",
            "approvalPolicy": "never",
            "cwd": cwd,
            "sandbox": if write {
                json!({ "type": "workspaceWrite", "networkAccess": false, "writableRoots": [] })
            } else {
                json!({ "type": "readOnly", "networkAccess": false })
            },
        })
    }

    /// Walks the protocol through the start, answering as a clean Codex
    /// would, up to the started thread.
    fn started(spec: &Spec) -> Protocol {
        let mut p = Protocol::new(spec, None);
        let open = p.open();
        let init = &sent(&open)[0];
        assert_eq!(init["method"], "initialize");
        assert_eq!(init["id"], 1);
        assert_eq!(init["params"]["clientInfo"]["name"], "imece");
        let bus = spec.mcp.is_some();
        // What each of imece's requests is answered with, in order.
        let steps: Vec<(&str, Value)> = vec![
            ("initialize", json!({ "userAgent": "codex" })),
            (
                "account/read",
                json!({ "account": { "type": "chatgpt", "planType": "go" } }),
            ),
            (
                "config/read",
                json!({ "config": good_config(spec.write, &[], bus) }),
            ),
            (
                "skills/list",
                json!({ "data": [{ "skills": [{ "name": "x", "enabled": false, "path": "/x" }] }] }),
            ),
            (
                "plugin/installed",
                json!({ "marketplaces": [{ "plugins": [{ "id": "p", "enabled": false }] }] }),
            ),
            ("hooks/list", json!({ "data": [{ "hooks": [] }] })),
            ("mcpServerStatus/list", servers(bus, &[("node_repl", 0)])),
            ("thread/start", thread(spec.write, "/work/w1")),
        ];
        let mut id = 0;
        let mut acts = Vec::new();
        for (method, result) in steps {
            id += 1;
            acts = p.line(&reply(id, result)).unwrap();
            let asked = sent(&acts);
            if method != "thread/start" {
                assert_eq!(asked.len(), 1, "after {method}: {acts:?}");
                assert_eq!(asked[0]["id"], id + 1);
            }
            if method == "mcpServerStatus/list" {
                let t = &asked[0];
                assert_eq!(t["method"], "thread/start");
                assert_eq!(t["params"]["approvalPolicy"], "never");
                assert_eq!(t["params"]["sandbox"], sandbox(spec.write));
                assert_eq!(t["params"]["cwd"], "/work/w1");
                assert_eq!(t["params"]["ephemeral"], true);
                assert_eq!(t["params"]["developerInstructions"], "You are codex-1.");
            }
        }
        assert_eq!(
            events(&acts),
            [
                Event::Started {
                    session: "t1".into(),
                    model: "gpt-6-luna".into()
                },
                Event::Note(
                    "codex runs on your ChatGPT plan: its turns cost the budget nothing".into()
                ),
            ]
        );
        assert!(!p.waiting());
        p
    }

    #[test]
    fn a_clean_codex_passes_the_checks_and_the_thread_starts() {
        started(&spec(false));
        started(&with_bus(true));
    }

    /// Answers the start up to `upto` (the method whose answer `result`
    /// stands in for), returning what the protocol made of it.
    fn start_until(spec: &Spec, upto: &str, result: Value) -> Result<Vec<Act>, String> {
        let mut p = Protocol::new(spec, None);
        p.open();
        let bus = spec.mcp.is_some();
        let steps: Vec<(&str, Value)> = vec![
            ("initialize", json!({ "userAgent": "codex" })),
            (
                "account/read",
                json!({ "account": { "type": "chatgpt", "planType": "go" } }),
            ),
            (
                "config/read",
                json!({ "config": good_config(spec.write, &[], bus) }),
            ),
            ("skills/list", json!({ "data": [] })),
            ("plugin/installed", json!({ "marketplaces": [] })),
            ("hooks/list", json!({ "data": [] })),
            ("mcpServerStatus/list", servers(bus, &[])),
            ("thread/start", thread(spec.write, "/work/w1")),
        ];
        let mut id = 0;
        for (method, good) in steps {
            id += 1;
            let r = if method == upto { result.clone() } else { good };
            let acts = p.line(&reply(id, r))?;
            if method == upto {
                return Ok(acts);
            }
        }
        unreachable!("{upto} is not a start step");
    }

    #[test]
    fn an_api_key_or_no_login_is_refused() {
        let e = start_until(
            &spec(false),
            "account/read",
            json!({ "account": { "type": "apiKey" } }),
        )
        .unwrap_err();
        assert!(e.contains("apiKey") && e.contains("codex login"), "{e}");
        let e = start_until(&spec(false), "account/read", json!({ "account": null })).unwrap_err();
        assert!(e.contains("not logged in"), "{e}");
    }

    #[test]
    fn every_setting_codex_reports_must_be_the_one_asked_for() {
        let mut bad: Vec<(&str, Value)> = Vec::new();
        for (k, v) in [
            ("approval_policy", json!("on-request")),
            ("web_search", json!("live")),
            ("notify", json!(["/bin/notify"])),
            ("check_for_update_on_startup", json!(true)),
            ("sandbox_mode", json!("danger-full-access")),
            ("sandbox_workspace_write", json!({ "network_access": true })),
            ("history", json!({ "persistence": "save-all" })),
        ] {
            let mut c = good_config(true, &[], false);
            c[k] = v;
            bad.push((k, c));
        }
        let mut c = good_config(true, &[], false);
        c["features"]["multi_agent"] = json!(true);
        bad.push(("features.multi_agent", c));
        let mut c = good_config(true, &[], false);
        c["mcp_servers"]["imece"] = json!({ "command": "/opt/imece" });
        bad.push(("an imece server it was not given", c));
        for (what, c) in bad {
            let e = start_until(&spec(true), "config/read", json!({ "config": c }));
            assert!(e.is_err(), "accepted {what}");
        }
        // With the bus, imece's server must be there and be ours.
        let e = start_until(
            &with_bus(true),
            "config/read",
            json!({ "config": good_config(true, &[], false) }),
        );
        assert!(e.unwrap_err().contains("no imece MCP server"));
        let mut c = good_config(true, &[], true);
        c["mcp_servers"]["imece"]["args"] = json!(["mcp-relay", "/tmp/other.sock"]);
        let e = start_until(&with_bus(true), "config/read", json!({ "config": c }));
        assert!(e.unwrap_err().contains("another imece"));
        // imece's tools must be pre-approved, or every call would fail.
        for mode in [json!("prompt"), json!("auto"), json!("writes"), Value::Null] {
            let mut c = good_config(true, &[], true);
            c["mcp_servers"]["imece"]["default_tools_approval_mode"] = mode.clone();
            let e = start_until(&with_bus(true), "config/read", json!({ "config": c }));
            assert!(e.unwrap_err().contains("another imece"), "{mode}");
        }
        // A reader's config need not say anything about the write sandbox.
        let mut c = good_config(false, &[], false);
        c["sandbox_workspace_write"] = Value::Null;
        assert!(start_until(&spec(false), "config/read", json!({ "config": c })).is_ok());
    }

    #[test]
    fn plugins_hooks_and_other_servers_tools_stop_the_session() {
        let e = start_until(
            &spec(true),
            "plugin/installed",
            json!({ "marketplaces": [{ "plugins": [{ "id": "browser@x", "enabled": true }] }] }),
        );
        assert!(e.unwrap_err().contains("browser@x"));
        let e = start_until(
            &spec(true),
            "hooks/list",
            json!({ "data": [{ "hooks": [{}] }] }),
        );
        assert!(e.unwrap_err().contains("1 hooks"));
        let e = start_until(
            &spec(true),
            "mcpServerStatus/list",
            servers(false, &[("node_repl", 4)]),
        );
        assert!(e.unwrap_err().contains("node_repl"));
        // imece's server must be connected with exactly its tools.
        let e = start_until(&with_bus(true), "mcpServerStatus/list", servers(false, &[]));
        assert!(e.unwrap_err().contains("no connected imece"));
        let e = start_until(
            &with_bus(true),
            "mcpServerStatus/list",
            json!({ "data": [{ "name": "imece", "tools": { "send": {} } }] }),
        );
        assert!(e.unwrap_err().contains("offers"));
        let e = start_until(
            &with_bus(true),
            "mcpServerStatus/list",
            json!({ "data": [{ "name": "imece", "tools": { "send": {}, "post": {}, "exec": {} } }] }),
        );
        assert!(e.is_err());
        // Without the bus, a connected imece server is as unwelcome as any.
        let e = start_until(&spec(true), "mcpServerStatus/list", servers(true, &[]));
        assert!(e.is_err());
    }

    #[test]
    fn the_thread_must_be_the_one_asked_for() {
        let mut t = thread(true, "/work/w1");
        t["approvalPolicy"] = json!("on-request");
        assert!(start_until(&spec(true), "thread/start", t).is_err());
        let mut t = thread(true, "/work/w1");
        t["sandbox"] = json!({ "type": "dangerFullAccess" });
        assert!(start_until(&spec(true), "thread/start", t).is_err());
        let t = thread(false, "/work/w1");
        assert!(
            start_until(&spec(true), "thread/start", t).is_err(),
            "read-only for a writer"
        );
        let t = thread(true, "/work/w1");
        assert!(
            start_until(&spec(false), "thread/start", t).is_err(),
            "writable for a reader"
        );
        let mut t = thread(true, "/work/w1");
        t["sandbox"]["networkAccess"] = json!(true);
        assert!(start_until(&spec(true), "thread/start", t).is_err());
        let t = thread(true, "/elsewhere");
        assert!(start_until(&spec(true), "thread/start", t).is_err());
    }

    #[test]
    fn servers_and_skills_that_are_on_are_named_off_in_a_second_start_once() {
        let mut p = Protocol::new(&spec(true), None);
        p.open();
        p.line(&reply(1, json!({}))).unwrap();
        p.line(&reply(2, json!({ "account": { "type": "chatgpt" } })))
            .unwrap();
        let c = good_config(true, &[("node_repl", true), ("cua_repl", false)], false);
        p.line(&reply(3, json!({ "config": c }))).unwrap();
        let acts = p
            .line(&reply(
                4,
                json!({ "data": [{ "skills": [
                    { "name": "imagegen", "enabled": true, "path": "/h/.system/imagegen/SKILL.md" },
                    { "name": "off", "enabled": false, "path": "/h/off/SKILL.md" }] }] }),
            ))
            .unwrap();
        let off = Off {
            servers: vec!["node_repl".into()],
            skills: vec!["/h/.system/imagegen/SKILL.md".into()],
        };
        assert_eq!(acts, [Act::Restart(off.clone())]);

        // The second start finds them still on: that is the end.
        let mut p = Protocol::new(&spec(true), Some(off));
        p.open();
        p.line(&reply(1, json!({}))).unwrap();
        p.line(&reply(2, json!({ "account": { "type": "chatgpt" } })))
            .unwrap();
        let c = good_config(true, &[("node_repl", true)], false);
        p.line(&reply(3, json!({ "config": c }))).unwrap();
        let e = p.line(&reply(4, json!({ "data": [] }))).unwrap_err();
        assert!(e.contains("still has") && e.contains("node_repl"), "{e}");
    }

    #[test]
    fn answers_must_match_the_question_and_errors_stop_the_session() {
        let mut p = Protocol::new(&spec(true), None);
        p.open();
        assert!(p.line(&reply(7, json!({}))).is_err(), "another id");
        let mut p = Protocol::new(&spec(true), None);
        p.open();
        assert!(p.line(&reply(1, json!({}))).is_ok());
        assert!(p.line(&reply(1, json!({}))).is_err(), "answered twice");
        let mut p = Protocol::new(&spec(true), None);
        p.open();
        let e = p
            .line(
                &json!({ "jsonrpc": "2.0", "id": 1, "error": { "code": 1, "message": "no" } })
                    .to_string(),
            )
            .unwrap_err();
        assert!(e.contains("Initialize") && e.contains("no"), "{e}");
        let mut p = Protocol::new(&spec(true), None);
        assert!(p.line(&reply(1, json!({}))).is_err(), "nothing was asked");
        assert!(p.line("not json").is_err());
        assert!(p.line(r#"{"jsonrpc":"2.0"}"#).is_err());
        assert_eq!(p.line("  ").unwrap(), []);
    }

    #[test]
    fn nothing_of_a_turn_counts_before_the_settings_are_checked() {
        let mut p = Protocol::new(&spec(true), None);
        p.open();
        let item = notify(
            "item/completed",
            json!({ "item": { "type": "agentMessage", "text": "hi" } }),
        );
        assert!(p.line(&item).is_err());
        assert!(
            p.line(&notify(
                "turn/completed",
                json!({ "turn": { "status": "completed" } })
            ))
            .is_err()
        );
        // Account noise is fine at any time.
        assert_eq!(p.line(&notify("account/updated", json!({}))).unwrap(), []);
    }

    /// A started thread that was given one prompt: the servers were
    /// checked again and the turn started.
    fn in_turn(spec: &Spec) -> Protocol {
        let mut p = started(spec);
        let acts = p.prompt("fix it");
        let q = sent(&acts);
        assert_eq!(q.len(), 1);
        assert_eq!(q[0]["method"], "mcpServerStatus/list");
        assert_eq!(q[0]["id"], 9);
        let acts = p.line(&reply(9, servers(spec.mcp.is_some(), &[]))).unwrap();
        let q = sent(&acts);
        assert_eq!(q[0]["method"], "turn/start");
        assert_eq!(q[0]["params"]["threadId"], "t1");
        assert_eq!(
            q[0]["params"]["input"],
            json!([{ "type": "text", "text": "fix it" }])
        );
        let acts = p
            .line(&reply(10, json!({ "turn": { "id": "u1" } })))
            .unwrap();
        assert!(acts.is_empty());
        p
    }

    #[test]
    fn a_prompt_checks_the_servers_then_starts_a_turn_and_the_next_waits() {
        let mut p = in_turn(&with_bus(true));
        // A second prompt during the turn waits.
        assert!(p.prompt("and this").is_empty());
        let acts = p
            .line(&notify(
                "item/completed",
                json!({ "item": { "type": "agentMessage", "text": " Done. " } }),
            ))
            .unwrap();
        assert_eq!(events(&acts), [Event::Text("Done.".into())]);
        p.line(&notify(
            "thread/tokenUsage/updated",
            json!({ "tokenUsage": { "total": { "inputTokens": 1200, "cachedInputTokens": 1000, "outputTokens": 34 } } }),
        ))
        .unwrap();
        let acts = p
            .line(&notify(
                "turn/completed",
                json!({ "turn": { "id": "u1", "status": "completed" } }),
            ))
            .unwrap();
        assert_eq!(
            events(&acts),
            [
                Event::TurnDone {
                    ok: true,
                    text: "Done.".into(),
                    cost: 0,
                    denied: 0
                },
                Event::Note("tokens so far: 1200 in (1000 cached) · 34 out".into()),
            ]
        );
        // The waiting prompt goes next, after the servers are checked again.
        let q = sent(&acts);
        assert_eq!(q.len(), 1);
        assert_eq!(q[0]["method"], "mcpServerStatus/list");
        let acts = p.line(&reply(11, servers(true, &[]))).unwrap();
        assert_eq!(sent(&acts)[0]["params"]["input"][0]["text"], "and this");
    }

    #[test]
    fn a_turn_s_effort_and_model_come_from_the_choice() {
        let mut s = with_bus(true);
        s.choice = Some("gpt-5.6-terra@xhigh".into());
        // The thread starts on the model; the effort rides on each turn.
        let acts = start_until(&s, "mcpServerStatus/list", servers(true, &[])).unwrap();
        assert_eq!(sent(&acts)[0]["params"]["model"], "gpt-5.6-terra");
        let mut p = started(&s);
        p.prompt("go");
        let acts = p.line(&reply(9, servers(true, &[]))).unwrap();
        assert_eq!(sent(&acts)[0]["params"]["effort"], "xhigh");
        let mut s2 = with_bus(true);
        s2.choice = Some("@low".into());
        let acts = start_until(&s2, "mcpServerStatus/list", servers(true, &[])).unwrap();
        assert!(sent(&acts)[0]["params"].get("model").is_none());
    }

    #[test]
    fn the_effort_rides_on_every_turn() {
        let mut s = spec(false);
        s.choice = Some("@low".into());
        let mut p = started(&s);
        p.prompt("go");
        let acts = p.line(&reply(9, servers(false, &[]))).unwrap();
        assert_eq!(sent(&acts)[0]["params"]["effort"], "low");
        let mut p = started(&spec(false));
        p.prompt("go");
        let acts = p.line(&reply(9, servers(false, &[]))).unwrap();
        assert!(sent(&acts)[0]["params"].get("effort").is_none());
    }

    #[test]
    fn tools_failures_and_refusals_become_events() {
        let mut p = in_turn(&with_bus(true));
        let acts = p
            .line(&notify(
                "item/started",
                json!({ "item": { "type": "commandExecution", "command": "rg -n main src\nsecond line", "status": "inProgress" } }),
            ))
            .unwrap();
        assert_eq!(
            events(&acts),
            [Event::Tool {
                name: "shell".into(),
                target: "rg -n main src".into()
            }]
        );
        let acts = p
            .line(&notify(
                "item/completed",
                json!({ "item": { "type": "commandExecution", "command": "rm -rf /", "status": "declined" } }),
            ))
            .unwrap();
        assert_eq!(
            events(&acts),
            [Event::ToolFailed {
                name: "shell".into()
            }]
        );
        let acts = p
            .line(&notify(
                "item/started",
                json!({ "item": { "type": "fileChange", "status": "inProgress", "changes": [
                    { "path": "/work/w1/src/main.rs", "kind": { "type": "update" } },
                    { "path": "/work/w1/README.md", "kind": { "type": "add" } }] } }),
            ))
            .unwrap();
        assert_eq!(
            events(&acts),
            [
                Event::Tool {
                    name: "apply_patch".into(),
                    target: "src/main.rs".into()
                },
                Event::Tool {
                    name: "apply_patch".into(),
                    target: "README.md".into()
                },
            ]
        );
        // A patch that went in is no event and no refusal.
        let acts = p
            .line(&notify(
                "item/completed",
                json!({ "item": { "type": "fileChange", "status": "completed", "changes": [] } }),
            ))
            .unwrap();
        assert_eq!(events(&acts), []);
        // A patch the sandbox refused counts as a refusal too.
        let acts = p
            .line(&notify(
                "item/completed",
                json!({ "item": { "type": "fileChange", "status": "declined", "changes": [] } }),
            ))
            .unwrap();
        assert_eq!(
            events(&acts),
            [Event::ToolFailed {
                name: "apply_patch".into()
            }]
        );
        let acts = p
            .line(&notify(
                "item/completed",
                json!({ "item": { "type": "fileChange", "status": "failed", "changes": [] } }),
            ))
            .unwrap();
        assert_eq!(
            events(&acts),
            [Event::ToolFailed {
                name: "apply_patch".into()
            }]
        );
        let acts = p
            .line(&notify(
                "item/started",
                json!({ "item": { "type": "mcpToolCall", "server": "imece", "tool": "send", "arguments": { "to": "lead", "type": "done" }, "status": "inProgress" } }),
            ))
            .unwrap();
        assert_eq!(
            events(&acts),
            [Event::Tool {
                name: "mcp__imece__send".into(),
                target: "lead".into()
            }]
        );
        let acts = p
            .line(&notify(
                "item/completed",
                json!({ "item": { "type": "mcpToolCall", "server": "imece", "tool": "post", "arguments": {}, "status": "failed" } }),
            ))
            .unwrap();
        assert_eq!(
            events(&acts),
            [Event::ToolFailed {
                name: "mcp__imece__post".into()
            }]
        );
        // A failure with a reason says it: the call may never have reached
        // imece.
        let acts = p
            .line(&notify(
                "item/completed",
                json!({ "item": { "type": "mcpToolCall", "server": "imece", "tool": "send", "arguments": {}, "status": "failed",
                    "error": { "message": "MCP tool call requires approval, but approval policy is never" } } }),
            ))
            .unwrap();
        assert_eq!(
            events(&acts),
            [
                Event::ToolFailed {
                    name: "mcp__imece__send".into()
                },
                Event::Note(
                    "mcp__imece__send failed: MCP tool call requires approval, but approval policy is never".into()
                ),
            ]
        );
        let acts = p
            .line(&notify(
                "item/completed",
                json!({ "item": { "type": "mcpToolCall", "server": "imece", "tool": "send", "arguments": {}, "status": "failed", "error": "socket closed" } }),
            ))
            .unwrap();
        assert_eq!(
            events(&acts)[1],
            Event::Note("mcp__imece__send failed: socket closed".into())
        );
        let reasoning = notify(
            "item/completed",
            json!({ "item": { "type": "reasoning", "summary": [] } }),
        );
        assert_eq!(p.line(&reasoning).unwrap(), []);
        // The turn fails with the error's words and counts both refusals.
        p.line(&notify(
            "error",
            json!({ "error": { "message": "model overloaded" }, "willRetry": false }),
        ))
        .unwrap();
        let acts = p
            .line(&notify(
                "turn/completed",
                json!({ "turn": { "id": "u1", "status": "failed", "error": null } }),
            ))
            .unwrap();
        assert_eq!(
            events(&acts),
            [Event::TurnDone {
                ok: false,
                text: "model overloaded".into(),
                cost: 0,
                denied: 2
            }]
        );
    }

    #[test]
    fn a_turn_that_was_interrupted_says_so() {
        let mut p = in_turn(&spec(false));
        let acts = p
            .line(&notify(
                "turn/completed",
                json!({ "turn": { "id": "u1", "status": "interrupted" } }),
            ))
            .unwrap();
        assert!(
            matches!(&events(&acts)[0], Event::TurnDone { ok: false, text, .. } if text == "interrupted")
        );
        let mut p = in_turn(&spec(false));
        let acts = p
            .line(&notify(
                "turn/completed",
                json!({ "turn": { "id": "u1", "status": "failed", "error": { "message": "usage limit" } } }),
            ))
            .unwrap();
        assert!(
            matches!(&events(&acts)[0], Event::TurnDone { ok: false, text, .. } if text == "usage limit")
        );
    }

    #[test]
    fn what_the_agent_was_not_given_stops_the_session() {
        for item in [
            json!({ "type": "webSearch", "query": "rust" }),
            json!({ "type": "collabAgentToolCall", "tool": "spawn_agent" }),
            json!({ "type": "subAgentActivity", "kind": "spawn" }),
            json!({ "type": "dynamicToolCall", "tool": "x" }),
            json!({ "type": "mcpToolCall", "server": "node_repl", "tool": "run", "status": "inProgress" }),
            json!({ "type": "mcpToolCall", "server": "imece", "tool": "exec", "status": "inProgress" }),
        ] {
            let mut p = in_turn(&with_bus(true));
            assert!(
                p.line(&notify("item/started", json!({ "item": item })))
                    .is_err(),
                "accepted {item}"
            );
        }
        // Without the bus, even imece's tools are not its.
        let mut p = in_turn(&spec(true));
        let item = json!({ "type": "mcpToolCall", "server": "imece", "tool": "send", "status": "inProgress" });
        assert!(
            p.line(&notify("item/started", json!({ "item": item })))
                .is_err()
        );
    }

    #[test]
    fn codex_s_own_questions_are_answered_or_refused() {
        let mut p = in_turn(&spec(true));
        let ask = |id: u64, method: &str, params: Value| {
            json!({ "jsonrpc": "2.0", "id": id, "method": method, "params": params }).to_string()
        };
        // The model asked the person: nobody is there; it gets no answer.
        let acts = p
            .line(&ask(
                40,
                "item/tool/requestUserInput",
                json!({ "questions": [{ "id": "q1" }, { "id": "q2" }] }),
            ))
            .unwrap();
        let r = &sent(&acts)[0];
        assert_eq!(r["id"], 40);
        assert_eq!(r["result"]["answers"]["q1"]["answers"], json!([]));
        assert_eq!(r["result"]["answers"]["q2"]["answers"], json!([]));
        // Something imece does not do gets a JSON-RPC error, and life goes on.
        let acts = p.line(&ask(41, "attestation/generate", json!({}))).unwrap();
        assert_eq!(sent(&acts)[0]["error"]["code"], -32601);
        // An approval request means the settings are not what was asked.
        for method in [
            "item/commandExecution/requestApproval",
            "item/fileChange/requestApproval",
            "item/permissions/requestApproval",
            "applyPatchApproval",
            "execCommandApproval",
            "mcpServer/elicitation/request",
            "item/tool/call",
        ] {
            let mut p = in_turn(&spec(true));
            assert!(p.line(&ask(42, method, json!({}))).is_err(), "{method}");
        }
    }

    #[test]
    fn a_command_is_shown_in_one_short_line() {
        assert_eq!(one_line("  ls -la \n rm x"), "ls -la");
        let long = "x".repeat(130);
        let shown = one_line(&long);
        assert_eq!(shown.chars().count(), 121);
        assert!(shown.ends_with('…'));
        assert_eq!(one_line(&"y".repeat(120)).chars().count(), 120);
        assert_eq!(relative("/work/w1/a/b.rs", Path::new("/work/w1")), "a/b.rs");
        assert_eq!(relative("/else/b.rs", Path::new("/work/w1")), "/else/b.rs");
    }

    // --- The session, against a stand-in for codex. ---

    /// A stand-in for `codex app-server` that answers imece's requests in
    /// order from `answers` (one JSON result per line, ids filled in) and
    /// plays `turn` after the turn starts.
    fn fake(dir: &Path, name: &str, body: &str) -> String {
        std::fs::create_dir_all(dir).unwrap();
        let path = dir.join(name);
        std::fs::write(&path, format!("#!/bin/sh\n{body}\n")).unwrap();
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o755)).unwrap();
        path.to_string_lossy().into_owned()
    }

    /// Shell that reads requests and answers each with the next line of
    /// `answers` (its id taken from the request); on `turn/start` it also
    /// plays the turn's notifications.
    fn fake_body(answers: &[Value], turn: &[String]) -> String {
        let answers = answers
            .iter()
            .map(|a| a.to_string().replace('\'', "'\\''"))
            .collect::<Vec<_>>()
            .join("\n");
        let turn = turn
            .iter()
            .map(|t| format!("echo '{}'", t.replace('\'', "'\\''")))
            .collect::<Vec<_>>()
            .join("\n");
        format!(
            r#"printf '%s\n' "$@" > args.txt
cat > answers.txt <<'IMECE_EOF'
{answers}
IMECE_EOF
n=0
while IFS= read -r line; do
  n=$((n+1))
  id=$(printf '%s' "$line" | sed -E 's/.*"id":([0-9]+).*/\1/')
  result=$(sed -n "${{n}}p" answers.txt)
  [ -z "$result" ] && exit 0
  printf '{{"jsonrpc":"2.0","id":%s,"result":%s}}\n' "$id" "$result"
  case "$line" in *'"method":"turn/start"'*)
{turn}
    exit 0
  ;; esac
done"#
        )
    }

    fn run(spec: &Spec, prompts: &[&str]) -> Vec<Event> {
        let (tx, rx) = mpsc::channel();
        // Through the trait, as the screen drives every vendor.
        let mut s: Box<dyn crate::Agent> = Box::new(
            Session::start(spec, move |e| {
                let _ = tx.send(e);
            })
            .unwrap(),
        );
        for p in prompts {
            s.prompt(p).unwrap();
        }
        let mut out = Vec::new();
        loop {
            let e = rx.recv_timeout(Duration::from_secs(20)).expect("in time");
            let end = matches!(e, Event::Stopped(_));
            out.push(e);
            if end {
                return out;
            }
        }
    }

    fn clean_answers(write: bool, cwd: &str) -> Vec<Value> {
        vec![
            json!({ "userAgent": "fake" }),
            json!({ "account": { "type": "chatgpt", "planType": "go" } }),
            json!({ "config": good_config(write, &[], false) }),
            json!({ "data": [] }),
            json!({ "marketplaces": [] }),
            json!({ "data": [] }),
            servers(false, &[]),
            thread(write, cwd),
            servers(false, &[]),
            json!({ "turn": { "id": "u1" } }),
        ]
    }

    #[test]
    fn a_session_starts_a_thread_runs_a_turn_and_reports_events() {
        let dir = std::env::temp_dir().join(format!("imece-codex-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let cwd = dir.canonicalize().unwrap();
        let turn = [
            notify(
                "item/completed",
                json!({ "item": { "type": "agentMessage", "text": "pong" } }),
            ),
            notify(
                "turn/completed",
                json!({ "turn": { "id": "u1", "status": "completed" } }),
            ),
        ];
        let mut s = spec(false);
        s.cwd = cwd.clone();
        s.program = fake(
            &dir,
            "fake-codex",
            &fake_body(&clean_answers(false, &cwd.to_string_lossy()), &turn),
        );
        let got = run(&s, &["ping"]);
        assert_eq!(
            got,
            [
                Event::Started {
                    session: "t1".into(),
                    model: "gpt-6-luna".into()
                },
                Event::Note(
                    "codex runs on your ChatGPT plan: its turns cost the budget nothing".into()
                ),
                Event::Text("pong".into()),
                Event::TurnDone {
                    ok: true,
                    text: "pong".into(),
                    cost: 0,
                    denied: 0
                },
                // The stand-in has no more answers and exits.
                Event::Stopped("exited with code 0".into()),
            ],
            "{got:?}"
        );
        let args = std::fs::read_to_string(dir.join("args.txt")).unwrap();
        assert!(args.starts_with("app-server\n"));
        assert!(args.contains("sandbox_mode=\"read-only\""));
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn a_session_whose_settings_differ_is_stopped_before_its_events_count() {
        let dir = std::env::temp_dir().join(format!("imece-codex-bad-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let cwd = dir.canonicalize().unwrap();
        let mut answers = clean_answers(true, &cwd.to_string_lossy());
        answers[2] = json!({ "config": good_config(false, &[], false) });
        let mut s = spec(true);
        s.cwd = cwd;
        s.program = fake(&dir, "fake-codex", &fake_body(&answers, &[]));
        let got = run(&s, &["ping"]);
        assert_eq!(got.len(), 1, "{got:?}");
        assert!(
            matches!(&got[0], Event::Stopped(w) if w.contains("sandbox_mode")),
            "{got:?}"
        );
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn a_first_start_that_finds_servers_on_starts_again_with_them_off() {
        let dir = std::env::temp_dir().join(format!("imece-codex-twice-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let cwd = dir.canonicalize().unwrap();
        // The stand-in tells the two starts apart by its arguments: the
        // second has node_repl named off, and then reports it off.
        let on = clean_answers(false, &cwd.to_string_lossy());
        let mut first = on.clone();
        first[2] = json!({ "config": good_config(false, &[("node_repl", true)], false) });
        // After its last answer the first start also blurts out a line that
        // would stop the session if the second start took it as its own.
        let body = format!(
            r#"case "$*" in *'mcp_servers.node_repl.enabled=false'*) exec "$0.second" "$@";; esac
{}"#,
            fake_body(&first, &[]).replace(
                r#""$id" "$result""#,
                r#""$id" "$result"
  [ "$n" = 4 ] && echo '{"jsonrpc":"2.0","id":99,"result":{}}'"#
            )
        );
        assert!(body.contains(r#""id":99"#), "the blurt is in: {body}");
        let program = fake(&dir, "fake-codex", &body);
        let turn = [notify(
            "turn/completed",
            json!({ "turn": { "id": "u1", "status": "completed" } }),
        )];
        fake(&dir, "fake-codex.second", &fake_body(&on, &turn));
        let mut s = spec(false);
        s.cwd = cwd;
        s.program = program;
        let got = run(&s, &["ping"]);
        assert!(
            matches!(&got[0], Event::Started { .. }),
            "the second start succeeded: {got:?}"
        );
        assert!(
            matches!(&got[2], Event::TurnDone { ok: true, .. }),
            "{got:?}"
        );
        let args = std::fs::read_to_string(dir.join("args.txt")).unwrap();
        assert!(
            args.contains("mcp_servers.node_repl.enabled=false"),
            "{args}"
        );
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn a_session_that_gets_no_answer_in_time_is_stopped() {
        // Not run against the clock: REQUEST_WAIT is a minute. The driver's
        // deadline logic is exercised by its waiting flag instead.
        let mut p = Protocol::new(&spec(true), None);
        assert!(!p.waiting());
        p.open();
        assert!(p.waiting());
        p.line(&reply(1, json!({}))).unwrap();
        assert!(p.waiting(), "the next question is out");
    }

    #[test]
    fn a_prompt_to_a_session_that_is_over_fails() {
        let dir = std::env::temp_dir().join(format!("imece-codex-over-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let mut s = spec(true);
        s.cwd = dir.clone();
        s.program = fake(&dir, "fake-codex", "exit 3");
        let (tx, rx) = mpsc::channel();
        let mut session = Session::start(&s, move |e| {
            let _ = tx.send(e);
        })
        .unwrap();
        let e = rx.recv_timeout(Duration::from_secs(20)).unwrap();
        assert!(
            matches!(&e, Event::Stopped(w) if w.contains("code 3")),
            "{e:?}"
        );
        // The driver goes away right after its last word; give it a moment
        // (a bounded one), then nothing can be sent any more.
        let refused = (0..100).any(|_| {
            thread::sleep(Duration::from_millis(50));
            session.prompt("late").is_err()
        });
        assert!(refused, "the session still took prompts after it was over");
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// A stand-in that answers nothing and never exits on its own.
    fn sleeper(dir: &Path) -> Spec {
        let mut s = spec(true);
        s.cwd = dir.to_path_buf();
        s.program = fake(dir, "fake-codex", "cat > /dev/null");
        s
    }

    #[test]
    fn terminating_a_session_ends_its_driver_and_its_process() {
        let dir = std::env::temp_dir().join(format!("imece-codex-term-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let (tx, rx) = mpsc::channel();
        let mut session: Box<dyn crate::Agent> = Box::new(
            Session::start(&sleeper(&dir), move |e| {
                let _ = tx.send(e);
            })
            .unwrap(),
        );
        assert!(session.prompt("hello").is_ok(), "the driver is there");
        session.terminate();
        // The driver returns and lets go of the events channel.
        assert_eq!(
            rx.recv_timeout(Duration::from_secs(10)).unwrap_err(),
            RecvTimeoutError::Disconnected
        );
        assert!(session.prompt("late").is_err());
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn dropping_a_session_ends_its_driver_too() {
        let dir = std::env::temp_dir().join(format!("imece-codex-drop-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let (tx, rx) = mpsc::channel();
        let session = Session::start(&sleeper(&dir), move |e| {
            let _ = tx.send(e);
        })
        .unwrap();
        drop(session);
        assert_eq!(
            rx.recv_timeout(Duration::from_secs(10)).unwrap_err(),
            RecvTimeoutError::Disconnected
        );
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn an_answer_that_takes_a_moment_is_waited_for() {
        let dir = std::env::temp_dir().join(format!("imece-codex-slow-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let cwd = dir.canonicalize().unwrap();
        // Slower than the driver's poll, well within its patience.
        let body = format!(
            "sleep 1.5\n{}",
            fake_body(&clean_answers(false, &cwd.to_string_lossy()), &[])
        );
        let mut s = spec(false);
        s.cwd = cwd;
        s.program = fake(&dir, "fake-codex", &body);
        let (tx, rx) = mpsc::channel();
        let _session = Session::start(&s, move |e| {
            let _ = tx.send(e);
        })
        .unwrap();
        let e = rx.recv_timeout(Duration::from_secs(20)).unwrap();
        assert!(matches!(e, Event::Started { .. }), "{e:?}");
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// The real Codex, on the person's ChatGPT plan: one short turn in a
    /// throwaway checkout, with imece's `send` tool on a socket (`nc -U`
    /// stands in for `imece mcp-relay`). Run by hand:
    /// `IMECE_LIVE_CODEX=1 cargo test -p imece-agent -- --ignored live_codex --nocapture`
    #[test]
    #[ignore = "drives the real codex; set IMECE_LIVE_CODEX=1"]
    fn live_codex_takes_a_turn_and_reaches_the_bus() {
        if std::env::var_os("IMECE_LIVE_CODEX").is_none() {
            return;
        }
        use std::sync::{Arc, Mutex};
        let dir = std::env::temp_dir().join(format!("imece-live-codex-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let cwd = dir.canonicalize().unwrap();
        std::fs::write(cwd.join("README.md"), "# scratch\n").unwrap();
        let git = |args: &[&str]| {
            let out = std::process::Command::new("git")
                .args(args)
                .current_dir(&cwd)
                .output()
                .unwrap();
            assert!(out.status.success(), "git {args:?}");
        };
        git(&["init", "--quiet"]);
        let calls: Arc<Mutex<Vec<Value>>> = Arc::default();
        let seen = Arc::clone(&calls);
        let sock = cwd.join("a.sock");
        let _server = crate::mcp::Server::bind(
            &sock,
            vec![crate::mcp::Tool {
                name: "send".into(),
                description: "Sends a message to a teammate in imece.".into(),
                input: json!({
                    "type": "object",
                    "properties": {
                        "to": { "type": "string" },
                        "type": { "type": "string" },
                        "body": { "type": "string" },
                    },
                    "required": ["to", "type", "body"],
                }),
            }],
            move |_, args| {
                seen.lock().unwrap().push(args.clone());
                Ok("delivered".into())
            },
        )
        .unwrap();
        let s = Spec {
            program: PROGRAM.into(),
            cwd: cwd.clone(),
            choice: Some("@low".into()),
            write: false,
            system: "You are codex-1, a coding agent in imece, a team led by lead. \
                     You can read files and run commands that only read."
                .into(),
            mcp: Some(Mcp {
                command: "nc".into(),
                args: vec!["-U".into(), sock.to_string_lossy().into_owned()],
                tools: vec!["send".into()],
            }),
        };
        let (tx, rx) = mpsc::channel();
        let session = Session::start(&s, move |e| {
            let _ = tx.send(e);
        })
        .unwrap();
        let mut session = session;
        session
            .prompt(
                "With the send tool, send lead an inform message whose body is the \
                 first line of README.md. Then reply with the single word: pong",
            )
            .unwrap();
        let mut got = Vec::new();
        loop {
            let e = rx
                .recv_timeout(Duration::from_secs(180))
                .expect("codex answers within three minutes");
            eprintln!("live codex: {e:?}");
            let end = matches!(e, Event::TurnDone { .. } | Event::Stopped(_));
            got.push(e);
            if end {
                break;
            }
        }
        drop(session);
        assert!(
            matches!(&got[0], Event::Started { .. }),
            "the checks passed: {got:?}"
        );
        let done = got.last().unwrap();
        assert!(
            matches!(done, Event::TurnDone { ok: true, text, .. } if text.to_lowercase().contains("pong")),
            "{got:?}"
        );
        let calls = calls.lock().unwrap();
        assert_eq!(calls.len(), 1, "one send: {calls:?}");
        assert_eq!(calls[0]["to"], "lead");
        assert!(
            calls[0]["body"]
                .as_str()
                .unwrap_or_default()
                .contains("scratch"),
            "{calls:?}"
        );
        let _ = std::fs::remove_dir_all(&dir);
    }
}
