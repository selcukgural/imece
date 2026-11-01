//! Codex as an agent: one long-lived `codex app-server` per agent (never a
//! process per turn), JSON-RPC over stdio. imece's requests go in on stdin;
//! Codex's responses, notifications and requests come out on stdout, one
//! JSON object per line.
//!
//! Codex has nothing like `--restricted` and no way to choose its built-in
//! tools: it always has a sandboxed shell and `apply_patch`. What imece can
//! set, it sets with config overrides (`-c`) on the command line and again
//! on the thread: the sandbox is read-only for a leader and workspace-write
//! for a worker (writes stay in its directory). A worker from standard on
//! asks (`on-request`): what needs more than the sandbox gives (a write
//! outside its directory, a command run outside the sandbox) comes as an
//! [`Event::Asks`] and waits for [`Session::decide`]; a leader, and every
//! session under strict, never asks (what the sandbox refuses just
//! fails). The confine dial sets the rest:
//! under strict the sandbox has no network and web search is off; under
//! standard a worker's sandbox reaches the network and web search is live;
//! both have no plugins, apps, hooks, sub-agents or memories, and no MCP
//! server but imece's own. Codex loads the person's own config as well,
//! and some of what comes with it (MCP servers that plugins put there,
//! the system skills) can only be turned off by name: a session that
//! finds such things when it first starts is started a second time with
//! them named off. Under open the person's config stands (their MCP
//! servers, skills, plugins, hooks, features), with only the sub-agent
//! feature off. What Codex then reports it runs with (`config/read`,
//! `plugin/installed`, `skills/list`, `hooks/list`, `mcpServerStatus/list`,
//! and the thread's own settings) is checked against all that before the
//! thread starts, the MCP servers again before every turn; anything else
//! stops the session.
//!
//! Codex reports tokens, not dollars. On a ChatGPT plan the tokens cost
//! nothing beyond the plan, so a session books no cost and says so once; an
//! API-key login would cost money imece can not count yet, so it is refused.
//!
//! A thread is kept on disk (Codex's rollout under `~/.codex/sessions`, its
//! message history still off), so an agent woken in a later run goes on in
//! the thread it had: `thread/resume` instead of `thread/start`, answered
//! with the same settings to check, and refused ("no rollout found") when
//! the rollout is gone, which stops the session before any turn so the
//! screen starts a new one. A resumed thread keeps the developer
//! instructions it was started with; the ones passed again are not taken.

use std::collections::{BTreeSet, HashMap, VecDeque};
use std::io;
use std::path::{Path, PathBuf};
use std::sync::mpsc::{self, Receiver, RecvTimeoutError, Sender};
use std::thread;
use std::time::{Duration, Instant};

use serde_json::{Value, json};

use crate::child::{self, Child, Out};
use crate::{Ask, Confine, Event, Limit, MCP_SERVER, Mcp, Step, StepState, mcp_tool};

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

/// Codex has no model aliases: a model is named in full, or left to
/// Codex's own default.
pub const ALIASES: &[&str] = &[];

/// The reasoning efforts Codex takes on a turn, least first. One it does
/// not know is refused before the session starts.
pub const EFFORTS: &[&str] = &["low", "medium", "high", "xhigh"];

/// Config every session gets at every level (`-c key=value`, TOML values)
/// and that `config/read` must echo. The approval policy, web search,
/// the sandbox's network, the features and the sandbox come on top, by
/// role and level.
const OVERRIDES: &[(&str, &str)] = &[
    ("notify", "[]"),
    ("history.persistence", "\"none\""),
    ("check_for_update_on_startup", "false"),
    (PLAN_TOOL, "true"),
];

/// Codex's `update_plan` tool, off in an app-server thread unless asked
/// for: the agent's own to-do list, which changes no file, read as
/// [`Event::Plan`] from `turn/plan/updated`. `config/read` does not echo
/// it (0.165 echoes `tools.web_search` alone), so it is the one override
/// not checked: a thread without it only has no list to show.
const PLAN_TOOL: &str = "tools.update_plan.enabled";

/// Codex's approval policy for a session: a writer from standard on asks
/// (`on-request`: what needs more than the sandbox gives waits for the
/// person's yes); a leader, and every session under strict, never asks.
fn approval_policy(write: bool, confine: Confine) -> &'static str {
    if write && confine.full() {
        "on-request"
    } else {
        "never"
    }
}

/// Codex's web search at a level: off under strict, live under standard,
/// the person's own under open (not set, not checked).
fn web_search(confine: Confine) -> Option<&'static str> {
    match confine {
        Confine::Strict => Some("disabled"),
        Confine::Standard => Some("live"),
        Confine::Open => None,
    }
}

/// The feature that runs agents of Codex's own: off at every level.
const SUBAGENT_FEATURE: &str = "multi_agent";

/// The features turned off at a level: under open only the sub-agents,
/// else all of [`FEATURES_OFF`].
fn features_off(confine: Confine) -> &'static [&'static str] {
    if confine == Confine::Open {
        &[SUBAGENT_FEATURE]
    } else {
        FEATURES_OFF
    }
}

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
/// (`account/rateLimits/updated` is read: the plan's usage windows.)
const QUIET: &[&str] = &[
    "item/agentMessage/delta",
    "item/reasoning/summaryTextDelta",
    "item/reasoning/summaryPartAdded",
    "item/reasoning/textDelta",
    "item/commandExecution/outputDelta",
    "item/fileChange/outputDelta",
    "item/plan/delta",
    "turn/diff/updated",
    "remoteControl/status/changed",
    "thread/readState/changed",
];

/// How long an answer to one of imece's requests may take. A turn itself
/// has no limit; its `turn/start` is answered at once.
const REQUEST_WAIT: Duration = Duration::from_secs(60);
const POLL: Duration = Duration::from_millis(500);
/// How long a refused write waits for the process's own end to be
/// reported, which says why (an exit code and its last words) where the
/// write error only says "broken pipe".
const END_WAIT: Duration = Duration::from_secs(2);

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
    /// How much of Codex the session gets; see [`Confine`].
    pub confine: Confine,
    /// The thread's developer instructions: the agent's role in imece.
    pub system: String,
    /// imece's MCP server, if the agent gets its tools.
    pub mcp: Option<Mcp>,
    /// The thread of an earlier run to go on with, if the agent had one.
    pub resume: Option<String>,
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
    set(
        "approval_policy".into(),
        toml_str(approval_policy(spec.write, spec.confine)),
    );
    if let Some(w) = web_search(spec.confine) {
        set("web_search".into(), toml_str(w));
    }
    set(
        "sandbox_workspace_write.network_access".into(),
        spec.confine.full().to_string(),
    );
    for f in features_off(spec.confine) {
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
    /// `thread/compact/start`, the one command of its own Codex has.
    Compact,
}

/// How an approval Codex asked for is answered.
#[derive(Clone, Debug, PartialEq)]
enum Asked {
    /// `accept` or `decline`: a command, or a file change.
    Decision,
    /// The permissions asked for, granted for the turn, or none.
    Permissions(Value),
}

/// The longest permissions ask shown, in characters.
const MAX_PERMISSIONS: usize = 200;

/// What waits for the thread's next free moment.
#[derive(Clone, Debug, PartialEq, Eq)]
enum Queued {
    Prompt(String),
    Compact,
}

/// Turns stdout lines into [`Act`]s and checks the settings Codex reports.
pub struct Protocol {
    cwd: PathBuf,
    write: bool,
    confine: Confine,
    /// See [`approval_policy`].
    policy: &'static str,
    model: Option<String>,
    effort: Option<String>,
    system: String,
    mcp: Option<Mcp>,
    /// The thread asked for instead of a new one; see [`Spec::resume`].
    resume: Option<String>,
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
    queue: VecDeque<Queued>,
    busy: bool,
    /// The turn so far: the agent's last message, the last error, refused
    /// tool calls, and the session's tokens (input, cached, output).
    text: String,
    error: Option<String>,
    denied: usize,
    tokens: Option<(u64, u64, u64)>,
    /// What Codex asked and waits on the person's yes for, in the order
    /// asked: the request (its JSON-RPC id as text), the id to answer,
    /// and how.
    asks: Vec<(String, Value, Asked)>,
    /// The files each file change of the turn touches, by item id: what
    /// its approval shows.
    changes: HashMap<String, Vec<String>>,
}

impl Protocol {
    /// For `spec`; `off` is what the first start found, on the second.
    pub fn new(spec: &Spec, off: Option<Off>) -> Protocol {
        let crate::Choice { model, effort } = spec
            .choice
            .as_deref()
            .map(crate::Choice::parse)
            .unwrap_or_default();
        Protocol {
            cwd: spec.cwd.clone(),
            write: spec.write,
            confine: spec.confine,
            policy: approval_policy(spec.write, spec.confine),
            model,
            effort,
            system: spec.system.clone(),
            mcp: spec.mcp.clone(),
            resume: spec.resume.clone(),
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
            asks: Vec::new(),
            changes: HashMap::new(),
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
        self.queue.push_back(Queued::Prompt(text.to_string()));
        self.next_turn()
    }

    /// Queues a compaction of the thread (`/compact`); it runs as a turn
    /// of its own when the thread is free.
    pub fn compact(&mut self) -> Vec<Act> {
        self.queue.push_back(Queued::Compact);
        self.next_turn()
    }

    /// Starts the next queued turn, if the thread is free: the MCP servers
    /// are checked first (a compaction uses no tools and goes at once).
    fn next_turn(&mut self) -> Vec<Act> {
        if !self.started || self.busy || self.awaiting.is_some() {
            return Vec::new();
        }
        match self.queue.front() {
            None => Vec::new(),
            Some(Queued::Compact) => {
                self.queue.pop_front();
                self.busy = true;
                let params = json!({ "threadId": self.thread });
                vec![self.request("thread/compact/start", params, Wait::Compact)]
            }
            Some(Queued::Prompt(_)) => {
                vec![self.request("mcpServerStatus/list", json!({}), Wait::Recheck)]
            }
        }
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
            // A refused compaction is a turn that failed, not a broken
            // session: the thread is as it was and the next turn runs.
            // Everything else (the start-up checks above all) fails closed.
            if wait != Wait::Compact {
                return Err(format!("refused {wait:?}: {msg}"));
            }
            self.busy = false;
            let mut acts = vec![Act::Event(Event::TurnDone {
                ok: false,
                text: format!("compaction refused: {msg}"),
                cost: 0,
                denied: 0,
            })];
            acts.extend(self.next_turn());
            return Ok(acts);
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
                // Under open the person's skills stay, like their servers.
                if self.confine != Confine::Open {
                    self.found.skills = enabled_skills(r);
                }
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
                if !on.is_empty() && self.confine != Confine::Open {
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
                if hooks > 0 && self.confine != Confine::Open {
                    return Err(format!("has {hooks} hooks"));
                }
                vec![self.request("mcpServerStatus/list", json!({}), Wait::Servers)]
            }
            Wait::Servers => {
                self.check_servers(r)?;
                let mut params = json!({
                    "cwd": self.cwd,
                    "approvalPolicy": self.policy,
                    "sandbox": sandbox(self.write),
                    "developerInstructions": self.system,
                });
                if let Some(m) = &self.model {
                    params["model"] = json!(m);
                }
                // The thread is kept, to go on with in a later run; a
                // resumed one needs no history sent back (its turns are
                // in Codex, not on the screen).
                let method = match &self.resume {
                    Some(id) => {
                        params["threadId"] = json!(id);
                        params["excludeTurns"] = json!(true);
                        "thread/resume"
                    }
                    None => {
                        params["ephemeral"] = json!(false);
                        "thread/start"
                    }
                };
                vec![self.request(method, params, Wait::Thread)]
            }
            Wait::Thread => {
                self.check_thread(r)?;
                let id = r["thread"]["id"].as_str().unwrap_or_default();
                if let Some(asked) = &self.resume
                    && id != asked
                {
                    return Err(format!("resumed thread {id:?}, not {asked:?}"));
                }
                self.thread = Some(id.to_string());
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
                    // Codex has no command line; `command` maps this one.
                    Act::Event(Event::Commands(vec!["/compact".into()])),
                ];
                acts.extend(self.next_turn());
                acts
            }
            Wait::Recheck => {
                self.check_servers(r)?;
                let text = match self.queue.pop_front() {
                    Some(Queued::Prompt(t)) => t,
                    _ => String::new(),
                };
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
            Wait::Compact => {
                self.busy = false;
                let mut acts = vec![Act::Event(Event::TurnDone {
                    ok: true,
                    text: "compacted".into(),
                    cost: 0,
                    denied: 0,
                })];
                acts.extend(self.next_turn());
                acts
            }
        })
    }

    /// The `config/read` result must echo what was set; the MCP servers
    /// still on besides imece's are returned, to be named off (none under
    /// open, where the person's stay).
    fn check_config(&self, c: &Value) -> Result<Vec<String>, String> {
        let mut want = vec![
            ("approval_policy", json!(self.policy)),
            ("notify", json!([])),
            ("check_for_update_on_startup", json!(false)),
            ("sandbox_mode", json!(sandbox(self.write))),
        ];
        if let Some(w) = web_search(self.confine) {
            want.push(("web_search", json!(w)));
        }
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
        let network = self.confine.full();
        if self.write && c["sandbox_workspace_write"]["network_access"] != network {
            return Err(format!(
                "runs {} network access in the sandbox",
                if network { "without" } else { "with" }
            ));
        }
        for f in features_off(self.confine) {
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
            } else if s["enabled"] != false && self.confine != Confine::Open {
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
    /// tools (so it is connected), and no tools anywhere else (the
    /// person's servers under open excepted).
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
            } else if !tools.is_empty() && self.confine != Confine::Open {
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
        if r["approvalPolicy"] != self.policy {
            return Err(format!(
                "started a thread with approval policy {}",
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
        // A writer's sandbox reaches the network from standard on; a
        // reader's never does.
        let network = self.write && self.confine.full();
        if (r["sandbox"]["networkAccess"] == true) != network {
            return Err(format!(
                "started a thread {} network access",
                if network { "without" } else { "with" }
            ));
        }
        if r["cwd"].as_str().map(Path::new) != Some(self.cwd.as_path()) {
            return Err(format!("started a thread in {}", r["cwd"]));
        }
        // Kept on disk, or there is nothing to go on with next time.
        if r["thread"]["ephemeral"] != false {
            return Err("started a thread that is not kept".into());
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
            | "item/permissions/requestApproval" => self.approval(id, method, params),
            "applyPatchApproval" | "execCommandApproval" => {
                Err(format!("asked for an approval the old way ({method})"))
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

    /// Codex asks the person's yes before a command, a file change or
    /// more permissions: it waits, as an [`Event::Asks`], for
    /// [`Protocol::decide`]. A session that may not ask, or asks before
    /// its settings were checked, is stopped.
    fn approval(&mut self, id: Value, method: &str, p: &Value) -> Result<Vec<Act>, String> {
        if self.policy == "never" {
            return Err("asked for an approval although none may be asked".into());
        }
        if !self.started {
            return Err(format!("sent {method} before its settings were checked"));
        }
        let text = |k: &str| p[k].as_str().unwrap_or_default().to_string();
        let (what, how) = match method {
            "item/commandExecution/requestApproval" => {
                let command = text("command");
                let what = if p["kind"] == "writeStdin" {
                    Ask::Other {
                        tool: format!("{SHELL} stdin"),
                        detail: command,
                    }
                } else if let Some(host) = p["networkApprovalContext"]["host"].as_str() {
                    Ask::Other {
                        tool: SHELL.into(),
                        detail: format!("{command} (network: {host})"),
                    }
                } else {
                    Ask::Command {
                        tool: SHELL.into(),
                        command: unwrapped(&command),
                    }
                };
                (what, Asked::Decision)
            }
            "item/fileChange/requestApproval" => {
                let mut detail = self
                    .changes
                    .get(&text("itemId"))
                    .map(|c| c.join(", "))
                    .unwrap_or_default();
                if let Some(root) = p["grantRoot"].as_str() {
                    if !detail.is_empty() {
                        detail.push_str("; ");
                    }
                    detail.push_str(&format!("writes under {root}"));
                }
                let what = Ask::Other {
                    tool: PATCH.into(),
                    detail,
                };
                (what, Asked::Decision)
            }
            _ => {
                let asked = p["permissions"].to_string();
                let mut detail: String = asked.chars().take(MAX_PERMISSIONS).collect();
                if asked.chars().count() > MAX_PERMISSIONS {
                    detail.push('…');
                }
                let what = Ask::Other {
                    tool: "permissions".into(),
                    detail,
                };
                (what, Asked::Permissions(p["permissions"].clone()))
            }
        };
        let request = id.to_string();
        self.asks.push((request.clone(), id, how));
        Ok(vec![Act::Event(Event::Asks { request, what })])
    }

    /// The person's answer to what Codex asked with `request`: a command
    /// or a file change is accepted or declined (the turn goes on either
    /// way), permissions are granted for the turn or none are. An ask
    /// that is gone (settled, answered) is answered no more.
    pub fn decide(&mut self, request: &str, allow: bool) -> Vec<Act> {
        let Some(at) = self.asks.iter().position(|(r, ..)| r == request) else {
            return Vec::new();
        };
        let (_, id, how) = self.asks.remove(at);
        let result = match how {
            Asked::Decision => json!({ "decision": if allow { "accept" } else { "decline" } }),
            Asked::Permissions(asked) => json!({
                "permissions": if allow { asked } else { json!({}) },
                "scope": "turn",
            }),
        };
        vec![Act::Send(
            json!({ "jsonrpc": "2.0", "id": id, "result": result }).to_string(),
        )]
    }

    /// What Codex asked with `request` waits no more.
    fn withdraw(&mut self, request: &str) -> Vec<Act> {
        let Some(at) = self.asks.iter().position(|(r, ..)| r == request) else {
            return Vec::new();
        };
        self.asks.remove(at);
        vec![Act::Event(Event::Withdrawn {
            request: request.to_string(),
        })]
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
            // An ask Codex settled itself (the turn was stopped); one
            // imece answered is gone already.
            "serverRequest/resolved" => Ok(self.withdraw(&p["requestId"].to_string())),
            // The plan's usage windows; sparse, so a window not named is as
            // it was.
            "account/rateLimits/updated" => Ok(limits(&p["rateLimits"])
                .map(|l| Act::Event(Event::Limits(l)))
                .into_iter()
                .collect()),
            // Its `update_plan` tool: the whole list every time.
            "turn/plan/updated" => Ok(vec![Act::Event(Event::Plan(plan(&p["plan"])))]),
            "thread/tokenUsage/updated" => {
                let t = &p["tokenUsage"]["total"];
                let n = |k: &str| t[k].as_u64().unwrap_or(0);
                self.tokens = Some((n("inputTokens"), n("cachedInputTokens"), n("outputTokens")));
                // The last request's tokens are what the context holds,
                // its reasoning not kept; the window's size when said.
                let last = &p["tokenUsage"]["last"];
                let l = |k: &str| last[k].as_u64().unwrap_or(0);
                let used = match last["totalTokens"].as_u64() {
                    Some(total) => total.saturating_sub(l("reasoningOutputTokens")),
                    None => l("inputTokens") + l("cachedInputTokens") + l("outputTokens"),
                };
                Ok(vec![Act::Event(Event::Context {
                    used,
                    size: p["tokenUsage"]["modelContextWindow"].as_u64(),
                })])
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
                // What the turn asked can not run after it.
                let asked: Vec<String> = self.asks.iter().map(|(r, ..)| r.clone()).collect();
                let mut acts: Vec<Act> = asked.iter().flat_map(|r| self.withdraw(r)).collect();
                self.changes.clear();
                acts.push(Act::Event(Event::TurnDone {
                    ok,
                    text,
                    cost: 0,
                    denied: self.denied,
                }));
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
                let id = item["id"].as_str().unwrap_or_default().to_string();
                if completed {
                    failed(PATCH, &mut out);
                    if status == "declined" {
                        self.denied += 1;
                    }
                    self.changes.remove(&id);
                } else {
                    let mut paths = Vec::new();
                    for c in item["changes"].as_array().into_iter().flatten() {
                        let target = relative(c["path"].as_str().unwrap_or_default(), &self.cwd);
                        paths.push(target.clone());
                        out.push(Act::Event(Event::Tool {
                            name: PATCH.into(),
                            target,
                        }));
                    }
                    self.changes.insert(id, paths);
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

/// The steps of a `turn/plan/updated`, in Codex's order; a status it
/// does not name is not begun.
fn plan(steps: &Value) -> Vec<Step> {
    steps
        .as_array()
        .into_iter()
        .flatten()
        .map(|s| Step {
            text: one_line(s["step"].as_str().unwrap_or_default()),
            state: match s["status"].as_str().unwrap_or_default() {
                "inProgress" => StepState::Doing,
                "completed" => StepState::Done,
                _ => StepState::Todo,
            },
        })
        .collect()
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

/// The shells Codex wraps a command in, by name, at the places a system
/// keeps them.
const SHELLS: &[&str] = &[
    "sh",
    "bash",
    "zsh",
    "/bin/sh",
    "/bin/bash",
    "/bin/zsh",
    "/usr/bin/sh",
    "/usr/bin/bash",
    "/usr/bin/zsh",
];

/// The command a shell runs when Codex wraps it in one (`/bin/zsh -lc
/// '…'`): what the person reads and what is checked for routine, as the
/// agent wrote it. Anything else (another program, a shell elsewhere,
/// more words) stays as Codex sent it.
fn unwrapped(command: &str) -> String {
    match crate::run::split(command).as_deref() {
        Ok([shell, flag, inner])
            if SHELLS.contains(&shell.as_str()) && matches!(flag.as_str(), "-c" | "-lc") =>
        {
            inner.clone()
        }
        _ => command.to_string(),
    }
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
    Compact,
    /// The person's answer to an ask: the request, and whether it runs.
    Decide(String, bool),
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
        let choice = spec.choice.as_deref().map(crate::Choice::parse);
        if let Err(why) = choice.unwrap_or_default().check(PROGRAM, EFFORTS) {
            return Err(io::Error::new(io::ErrorKind::InvalidInput, why));
        }
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

    /// Codex's app server has no command line; `/compact` is the one
    /// command it has a method for. (`/clear` is imece's: a new session.)
    pub fn command(&mut self, line: &str) -> io::Result<()> {
        if line.trim() != "/compact" {
            return Err(io::Error::new(
                io::ErrorKind::Unsupported,
                "codex has no command line of its own; it knows /compact, and /clear \
                 starts a new session",
            ));
        }
        self.tx
            .send(Msg::Compact)
            .map_err(|_| io::Error::new(io::ErrorKind::BrokenPipe, "the session is over"))
    }

    /// Answers what the session asked with `request` (an
    /// [`Event::Asks`]): `allow` lets it run, else it is refused.
    pub fn decide(&mut self, request: &str, allow: bool) -> io::Result<()> {
        self.tx
            .send(Msg::Decide(request.to_string(), allow))
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

    fn command(&mut self, line: &str) -> io::Result<()> {
        Session::command(self, line)
    }

    fn terminate(&mut self) {
        Session::terminate(self)
    }

    fn decide(&mut self, request: &str, allow: bool) -> io::Result<()> {
        Session::decide(self, request, allow)
    }
}

/// How long deleting a thread may take: a process of its own starts.
const DELETE_WAIT: Duration = Duration::from_secs(30);

/// Deletes thread `id` where Codex keeps it (its rollout file and its row
/// in Codex's own thread index), through an app server started for it:
/// `initialize`, `thread/delete`, done. Blocks for the answer, at most
/// [`DELETE_WAIT`]; an error answer (a thread Codex does not know, too)
/// is an error. Only an id shaped like a UUID goes to Codex.
pub fn delete(program: &str, id: &str) -> io::Result<()> {
    if !crate::valid_uuid(id) {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            format!("{id:?} is not a thread id"),
        ));
    }
    let (tx, rx) = mpsc::channel();
    let mut cmd = child::command(program, &std::env::temp_dir(), ENV);
    cmd.arg("app-server");
    let mut child = Child::spawn(cmd, move |out| {
        tx.send(match out {
            Out::Line(l) => Ok(l.to_string()),
            Out::End(why) => Err(why),
        })
        .is_ok()
    })?;
    let deadline = Instant::now() + DELETE_WAIT;
    let requests = [
        (
            "initialize",
            json!({
                "clientInfo": {
                    "name": "imece",
                    "title": "imece",
                    "version": env!("CARGO_PKG_VERSION"),
                },
                "capabilities": { "optOutNotificationMethods": QUIET },
            }),
        ),
        ("thread/delete", json!({ "threadId": id })),
    ];
    for (n, (method, params)) in requests.into_iter().enumerate() {
        let n = n + 1;
        child.write_line(
            &json!({ "jsonrpc": "2.0", "id": n, "method": method, "params": params }).to_string(),
        )?;
        answer(&rx, deadline, n).map_err(|why| io::Error::other(format!("{method}: {why}")))?;
    }
    Ok(())
}

/// The answer to request `n` from `rx`, by `deadline`: its result, or the
/// error Codex gave, or why there was none.
fn answer(
    rx: &Receiver<Result<String, String>>,
    deadline: Instant,
    n: usize,
) -> Result<Value, String> {
    loop {
        let left = deadline.saturating_duration_since(Instant::now());
        let line = match rx.recv_timeout(left) {
            Ok(Ok(line)) => line,
            Ok(Err(why)) => return Err(format!("codex ended first ({why})")),
            Err(_) => return Err("codex did not answer in time".to_string()),
        };
        let Ok(v) = serde_json::from_str::<Value>(&line) else {
            continue;
        };
        if v["id"] != json!(n) {
            continue;
        }
        if let Some(e) = v.get("error") {
            let message = e["message"].as_str().unwrap_or("error");
            return Err(message.to_string());
        }
        return Ok(v["result"].clone());
    }
}

/// Why a session whose process refused a write is over: the process's
/// own end (of this generation) when it is reported within `END_WAIT`
/// (a process that has exited refuses input before its reader says
/// so), else the write error.
fn refused(rx: &Receiver<Msg>, generation: u64, e: &io::Error) -> String {
    let deadline = Instant::now() + END_WAIT;
    loop {
        match rx.recv_timeout(deadline.saturating_duration_since(Instant::now())) {
            Ok(Msg::End(g, why)) if g == generation => return why,
            Ok(_) => continue,
            Err(_) => return format!("stopped: codex took no input ({e})"),
        }
    }
}

/// Runs the protocol against the Codex process, starting it a second time
/// when the first start found things to name off.
fn drive(spec: Spec, child: Child, rx: Receiver<Msg>, tx: Sender<Msg>, mut on: impl FnMut(Event)) {
    // A local drops before the parameters: the commands' receiver goes
    // before the events' sender, so once the events end a command finds
    // nobody to take it (#74).
    let rx = rx;
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
                        on(Event::Stopped(refused(&rx, generation, &e)));
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
                            on(Event::Stopped(refused(&rx, generation, &e)));
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
            Ok(Msg::Compact) => acts = protocol.compact(),
            Ok(Msg::Decide(request, allow)) => acts = protocol.decide(&request, allow),
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

/// The windows an `account/rateLimits/updated` names: `primary` and
/// `secondary`, each with its length in minutes and the percent used
/// (`resetsAt` in seconds). One without a length or a percent is left
/// out; none when nothing is left.
fn limits(snapshot: &Value) -> Option<Vec<Limit>> {
    let window = |w: &Value| -> Option<Limit> {
        let minutes = u32::try_from(w["windowDurationMins"].as_u64()?).ok()?;
        let used = w["usedPercent"].as_f64()?.round().clamp(0.0, 100.0) as u8;
        Some(Limit {
            minutes,
            used,
            resets_at: w["resetsAt"].as_u64(),
        })
    };
    let out: Vec<Limit> = ["primary", "secondary"]
        .iter()
        .filter_map(|k| window(&snapshot[*k]))
        .collect();
    (!out.is_empty()).then_some(out)
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
            confine: Confine::Strict,
            system: "You are codex-1.".into(),
            mcp: None,
            resume: None,
        }
    }

    /// [`spec`] for a worker with a model of its own, remembered with
    /// thread `t1` from an earlier run.
    fn resumed() -> Spec {
        let mut s = spec(true);
        s.choice = Some("gpt-6-luna@high".into());
        s.resume = Some("t1".into());
        s
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
        assert_eq!(EFFORTS, ["low", "medium", "high", "xhigh"]);
        assert!(ALIASES.is_empty(), "Codex names models in full");
        let p = |c: &str| {
            let mut s = spec(true);
            s.choice = Some(c.into());
            let p = Protocol::new(&s, None);
            (p.model, p.effort)
        };
        assert_eq!(p("@low"), (None, Some("low".into())));
        assert_eq!(p("gpt-5.6-terra"), (Some("gpt-5.6-terra".into()), None));
        assert_eq!(
            p("gpt-5.6-terra@xhigh"),
            (Some("gpt-5.6-terra".into()), Some("xhigh".into()))
        );
        assert_eq!(p(""), (None, None));
        assert_eq!(
            (
                Protocol::new(&spec(true), None).model,
                Protocol::new(&spec(true), None).effort
            ),
            (None, None)
        );
    }

    #[test]
    fn an_effort_codex_does_not_have_never_starts_a_session() {
        // Before the program is looked for.
        let mut s = spec(true);
        s.program = "/nonexistent/codex".into();
        for bad in ["@max", "gpt-5.6-terra@bogus", "@High"] {
            s.choice = Some(bad.into());
            let e = Session::start(&s, |_| {}).err().expect("refused");
            assert_eq!(e.kind(), io::ErrorKind::InvalidInput, "{bad:?}");
            assert!(
                e.to_string().contains("codex has no effort")
                    && e.to_string().contains("low, medium, high, xhigh"),
                "{bad:?}: {e}"
            );
        }
        for fine in ["@xhigh", "gpt-5.6-terra", ""] {
            s.choice = Some(fine.into());
            let e = Session::start(&s, |_| {}).err().expect("refused");
            assert_ne!(e.kind(), io::ErrorKind::InvalidInput, "{fine:?}: {e}");
        }
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
        assert_eq!(after(&a, "tools.update_plan.enabled"), Some("true"));
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

    /// [`with_bus`] at `confine`.
    fn at(confine: Confine, write: bool) -> Spec {
        let mut s = with_bus(write);
        s.confine = confine;
        s
    }

    #[test]
    fn standard_turns_web_search_on_and_lets_the_sandbox_reach_the_network() {
        let a = args(&at(Confine::Standard, true), &Off::default());
        let after = |k: &str| {
            a.iter()
                .find_map(|x| x.strip_prefix(&format!("{k}=")))
                .map(str::to_string)
        };
        assert_eq!(after("web_search").as_deref(), Some("\"live\""));
        assert_eq!(
            after("sandbox_workspace_write.network_access").as_deref(),
            Some("true")
        );
        assert_eq!(
            after("sandbox_mode").as_deref(),
            Some("\"workspace-write\"")
        );
        for f in FEATURES_OFF {
            assert_eq!(
                after(&format!("features.{f}")).as_deref(),
                Some("false"),
                "{f}"
            );
        }
        // A writer asks what needs more than the sandbox; a reader never.
        assert_eq!(after("approval_policy").as_deref(), Some("\"on-request\""));
        let reader = args(&at(Confine::Standard, false), &Off::default());
        assert!(reader.contains(&"approval_policy=\"never\"".to_string()));
        // Strict: no web, no network, no asking, as before.
        let a = args(&at(Confine::Strict, true), &Off::default());
        assert!(a.contains(&"approval_policy=\"never\"".to_string()));
        assert!(a.contains(&"web_search=\"disabled\"".to_string()));
        assert!(a.contains(&"sandbox_workspace_write.network_access=false".to_string()));
        // Open: the person's web search, the sub-agents off and nothing else.
        let a = args(&at(Confine::Open, true), &Off::default());
        assert!(!a.iter().any(|x| x.starts_with("web_search=")), "{a:?}");
        assert!(a.contains(&"features.multi_agent=false".to_string()));
        assert!(!a.iter().any(|x| x.starts_with("features.apps=")), "{a:?}");
        assert!(!a.iter().any(|x| x.starts_with("features.hooks=")), "{a:?}");
        assert!(a.contains(&"sandbox_workspace_write.network_access=true".to_string()));
        assert!(a.contains(&"history.persistence=\"none\"".to_string()));
        assert!(a.contains(&"approval_policy=\"on-request\"".to_string()));
        assert_eq!(features_off(Confine::Open), ["multi_agent"]);
        assert_eq!(features_off(Confine::Standard), FEATURES_OFF);
        assert_eq!(features_off(Confine::Strict), FEATURES_OFF);
    }

    #[test]
    fn a_writer_asks_from_standard_on_and_a_reader_never() {
        assert_eq!(approval_policy(true, Confine::Open), "on-request");
        assert_eq!(approval_policy(true, Confine::Standard), "on-request");
        assert_eq!(approval_policy(true, Confine::Strict), "never");
        assert_eq!(approval_policy(false, Confine::Open), "never");
        assert_eq!(approval_policy(false, Confine::Standard), "never");
        assert_eq!(approval_policy(false, Confine::Strict), "never");
        // What config/read and the thread report is checked against it.
        let mut c = config_at(true, &[], false, Confine::Standard);
        c["approval_policy"] = json!("never");
        let e = start_until(
            &at(Confine::Standard, true),
            "config/read",
            json!({ "config": c }),
        )
        .unwrap_err();
        assert!(e.contains("runs with approval_policy = \"never\""), "{e}");
        let mut p = Protocol::new(&at(Confine::Standard, true), None);
        p.started = true;
        let mut t = thread_at(true, "/work/w1", Confine::Standard);
        assert!(p.check_thread(&t).is_ok());
        t["approvalPolicy"] = json!("never");
        let e = p.check_thread(&t).unwrap_err();
        assert!(e.contains("approval policy \"never\""), "{e}");
    }

    #[test]
    fn the_config_echo_is_checked_against_the_level() {
        // Standard: web search must be live, the sandbox must reach the network.
        let mut c = config_at(true, &[], true, Confine::Standard);
        assert!(
            start_until(
                &at(Confine::Standard, true),
                "config/read",
                json!({ "config": c.clone() })
            )
            .is_ok()
        );
        c["web_search"] = json!("disabled");
        let e = start_until(
            &at(Confine::Standard, true),
            "config/read",
            json!({ "config": c.clone() }),
        )
        .unwrap_err();
        assert!(e.contains("web_search = \"disabled\""), "{e}");
        c["web_search"] = json!("live");
        c["sandbox_workspace_write"]["network_access"] = json!(false);
        let e = start_until(
            &at(Confine::Standard, true),
            "config/read",
            json!({ "config": c.clone() }),
        )
        .unwrap_err();
        assert!(e.contains("runs without network access"), "{e}");
        // A reader's config need not say anything about the write sandbox.
        let mut r = config_at(false, &[], true, Confine::Standard);
        r["sandbox_workspace_write"]["network_access"] = json!(false);
        assert!(
            start_until(
                &at(Confine::Standard, false),
                "config/read",
                json!({ "config": r })
            )
            .is_ok()
        );
        // Strict: the network on is a weakening.
        let mut c = config_at(true, &[], true, Confine::Strict);
        c["sandbox_workspace_write"]["network_access"] = json!(true);
        let e = start_until(
            &at(Confine::Strict, true),
            "config/read",
            json!({ "config": c }),
        )
        .unwrap_err();
        assert!(e.contains("runs with network access"), "{e}");
        // Open: the person's features, servers and web search stand; only
        // the sub-agents must be off.
        let mut c = config_at(true, &[("github", true)], true, Confine::Open);
        c["features"]["apps"] = json!(true);
        c["features"]["hooks"] = json!(true);
        c["web_search"] = json!("live");
        let acts = start_until(
            &at(Confine::Open, true),
            "config/read",
            json!({ "config": c.clone() }),
        )
        .unwrap();
        assert_eq!(
            sent(&acts)[0]["method"],
            "skills/list",
            "no restart for the person's server"
        );
        c["features"]["multi_agent"] = json!(true);
        let e = start_until(
            &at(Confine::Open, true),
            "config/read",
            json!({ "config": c }),
        )
        .unwrap_err();
        assert!(e.contains("multi_agent feature on"), "{e}");
        // Standard still names the person's servers off.
        let c = config_at(true, &[("github", true)], true, Confine::Standard);
        let mut p = Protocol::new(&at(Confine::Standard, true), None);
        assert_eq!(p.check_config(&c).unwrap(), ["github"]);
        p.confine = Confine::Open;
        assert_eq!(p.check_config(&c).unwrap(), Vec::<String>::new());
    }

    #[test]
    fn under_open_the_person_s_skills_plugins_hooks_and_servers_stay() {
        let spec = at(Confine::Open, true);
        let skills =
            json!({ "data": [{ "skills": [{ "name": "x", "enabled": true, "path": "/x" }] }] });
        let acts = start_until(&spec, "skills/list", skills.clone()).unwrap();
        assert_eq!(
            sent(&acts)[0]["method"],
            "plugin/installed",
            "no restart: {acts:?}"
        );
        assert!(matches!(
            start_until(&at(Confine::Standard, true), "skills/list", skills).unwrap()[0],
            Act::Restart(_)
        ));
        let plugins = json!({ "marketplaces": [{ "plugins": [{ "id": "p", "enabled": true }] }] });
        let acts = start_until(&spec, "plugin/installed", plugins.clone()).unwrap();
        assert_eq!(sent(&acts)[0]["method"], "hooks/list");
        let e = start_until(&at(Confine::Standard, true), "plugin/installed", plugins).unwrap_err();
        assert!(e.contains("plugins on"), "{e}");
        let hooks = json!({ "data": [{ "hooks": [{ "event": "x" }] }] });
        let acts = start_until(&spec, "hooks/list", hooks.clone()).unwrap();
        assert_eq!(sent(&acts)[0]["method"], "mcpServerStatus/list");
        let e = start_until(&at(Confine::Standard, true), "hooks/list", hooks).unwrap_err();
        assert!(e.contains("has 1 hooks"), "{e}");
        // Other servers may have tools; imece's must still have exactly its own.
        let acts = start_until(
            &spec,
            "mcpServerStatus/list",
            servers(true, &[("github", 3)]),
        )
        .unwrap();
        assert_eq!(sent(&acts)[0]["method"], "thread/start");
        let e = start_until(
            &at(Confine::Standard, true),
            "mcpServerStatus/list",
            servers(true, &[("github", 3)]),
        )
        .unwrap_err();
        assert!(e.contains("has MCP server \"github\" with 3 tools"), "{e}");
        let mut bad = servers(true, &[("github", 3)]);
        bad["data"][0]["tools"] = json!({ "send": {} });
        let e = start_until(&spec, "mcpServerStatus/list", bad).unwrap_err();
        assert!(e.contains("imece's MCP server offers"), "{e}");
    }

    #[test]
    fn the_thread_s_network_access_must_be_the_level_s() {
        let p = |c, write| {
            let mut p = Protocol::new(&at(c, write), None);
            p.started = true;
            p
        };
        let net = |c, on| {
            let mut t = thread_at(true, "/work/w1", c);
            t["sandbox"]["networkAccess"] = json!(on);
            t
        };
        assert!(
            p(Confine::Strict, true)
                .check_thread(&net(Confine::Strict, false))
                .is_ok()
        );
        let e = p(Confine::Strict, true)
            .check_thread(&net(Confine::Strict, true))
            .unwrap_err();
        assert!(e.contains("with network access"), "{e}");
        assert!(
            p(Confine::Standard, true)
                .check_thread(&net(Confine::Standard, true))
                .is_ok()
        );
        assert!(
            p(Confine::Open, true)
                .check_thread(&net(Confine::Open, true))
                .is_ok()
        );
        let e = p(Confine::Standard, true)
            .check_thread(&net(Confine::Standard, false))
            .unwrap_err();
        assert!(e.contains("without network access"), "{e}");
        // A reader never reaches the network, whatever the level.
        let mut reader = thread_at(false, "/work/w1", Confine::Standard);
        assert!(p(Confine::Standard, false).check_thread(&reader).is_ok());
        reader["sandbox"]["networkAccess"] = json!(true);
        let e = p(Confine::Open, false).check_thread(&reader).unwrap_err();
        assert!(e.contains("with network access"), "{e}");
        // The whole start at standard and open passes with their echoes.
        started(&at(Confine::Standard, true));
        started(&at(Confine::Open, true));
        started(&at(Confine::Open, false));
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
        config_at(write, servers, bus, Confine::Strict)
    }

    /// [`good_config`] as a session at `confine` sets it.
    fn config_at(write: bool, servers: &[(&str, bool)], bus: bool, confine: Confine) -> Value {
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
            "approval_policy": approval_policy(write, confine),
            "web_search": web_search(confine).unwrap_or("cached"),
            "notify": [],
            "check_for_update_on_startup": false,
            "sandbox_mode": sandbox(write),
            "sandbox_workspace_write": { "network_access": confine.full() },
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
        thread_at(write, cwd, Confine::Strict)
    }

    /// The thread Codex reports for a session at `confine`.
    fn thread_at(write: bool, cwd: &str, confine: Confine) -> Value {
        json!({
            "thread": { "id": "t1", "model": "gpt-6-luna", "ephemeral": false },
            "model": "gpt-6-luna",
            "approvalPolicy": approval_policy(write, confine),
            "cwd": cwd,
            "sandbox": if write {
                json!({ "type": "workspaceWrite", "networkAccess": confine.full(), "writableRoots": [] })
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
                json!({ "config": config_at(spec.write, &[], bus, spec.confine) }),
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
            (
                "thread/start",
                thread_at(spec.write, "/work/w1", spec.confine),
            ),
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
                assert_eq!(
                    t["params"]["approvalPolicy"],
                    approval_policy(spec.write, spec.confine)
                );
                assert_eq!(t["params"]["sandbox"], sandbox(spec.write));
                assert_eq!(t["params"]["cwd"], "/work/w1");
                assert_eq!(t["params"]["ephemeral"], false, "kept, to resume");
                assert_eq!(t["params"]["developerInstructions"], "You are codex-1.");
                assert!(t["params"].get("threadId").is_none());
                assert!(t["params"].get("excludeTurns").is_none());
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
                Event::Commands(vec!["/compact".into()]),
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
                json!({ "config": config_at(spec.write, &[], bus, spec.confine) }),
            ),
            ("skills/list", json!({ "data": [] })),
            ("plugin/installed", json!({ "marketplaces": [] })),
            ("hooks/list", json!({ "data": [] })),
            ("mcpServerStatus/list", servers(bus, &[])),
            (
                if spec.resume.is_some() {
                    "thread/resume"
                } else {
                    "thread/start"
                },
                thread_at(spec.write, "/work/w1", spec.confine),
            ),
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
        // A thread that is not kept could not be resumed next time.
        let mut t = thread(true, "/work/w1");
        t["thread"]["ephemeral"] = json!(true);
        let e = start_until(&spec(true), "thread/start", t).unwrap_err();
        assert!(e.contains("not kept"), "{e}");
        let mut t = thread(true, "/work/w1");
        t["thread"].as_object_mut().unwrap().remove("ephemeral");
        assert!(start_until(&spec(true), "thread/start", t).is_err());
    }

    #[test]
    fn a_remembered_thread_is_resumed_with_the_same_settings_to_check() {
        let mut p = Protocol::new(&resumed(), None);
        p.open();
        let steps = [
            json!({ "userAgent": "codex" }),
            json!({ "account": { "type": "chatgpt" } }),
            json!({ "config": good_config(true, &[], false) }),
            json!({ "data": [] }),
            json!({ "marketplaces": [] }),
            json!({ "data": [] }),
            servers(false, &[]),
        ];
        let mut acts = Vec::new();
        for (i, r) in steps.into_iter().enumerate() {
            acts = p.line(&reply(i as u64 + 1, r)).unwrap();
        }
        let asked = sent(&acts);
        assert_eq!(asked.len(), 1, "{acts:?}");
        let t = &asked[0];
        assert_eq!(t["id"], 8);
        assert_eq!(t["method"], "thread/resume");
        assert_eq!(t["params"]["threadId"], "t1");
        assert_eq!(t["params"]["cwd"], "/work/w1");
        assert_eq!(t["params"]["approvalPolicy"], "never");
        assert_eq!(t["params"]["sandbox"], "workspace-write");
        assert_eq!(t["params"]["developerInstructions"], "You are codex-1.");
        assert_eq!(t["params"]["model"], "gpt-6-luna");
        assert_eq!(t["params"]["excludeTurns"], true, "its turns stay in codex");
        assert!(t["params"].get("ephemeral").is_none());
        assert!(
            t["params"].get("effort").is_none(),
            "effort goes on the turn"
        );
        // The thread that comes back is the one asked for, checked like
        // a new one, and the session goes on from it.
        let acts = p.line(&reply(8, thread(true, "/work/w1"))).unwrap();
        assert_eq!(
            events(&acts)[0],
            Event::Started {
                session: "t1".into(),
                model: "gpt-6-luna".into()
            }
        );
        assert_eq!(p.thread.as_deref(), Some("t1"));
        assert!(p.started);
        let acts = p.prompt("go on");
        assert_eq!(sent(&acts)[0]["method"], "mcpServerStatus/list");
        let acts = p.line(&reply(9, servers(false, &[]))).unwrap();
        let turn = &sent(&acts)[0];
        assert_eq!(turn["method"], "turn/start");
        assert_eq!(turn["params"]["threadId"], "t1");
        assert_eq!(turn["params"]["effort"], "high");
    }

    #[test]
    fn a_resume_must_come_back_as_the_thread_asked_for_and_kept() {
        let mut t = thread(true, "/work/w1");
        t["thread"]["id"] = json!("t2");
        let e = start_until(&resumed(), "thread/resume", t).unwrap_err();
        assert!(e.contains("\"t2\"") && e.contains("\"t1\""), "{e}");
        // The settings are checked on a resumed thread as on a new one.
        let t = thread(false, "/work/w1");
        assert!(start_until(&resumed(), "thread/resume", t).is_err());
        let t = thread(true, "/elsewhere");
        assert!(start_until(&resumed(), "thread/resume", t).is_err());
        let mut t = thread(true, "/work/w1");
        t["thread"]["ephemeral"] = json!(true);
        assert!(start_until(&resumed(), "thread/resume", t).is_err());
        // A rollout that is gone is refused by codex: the session stops
        // before any turn, and the screen starts a new one.
        let mut p = Protocol::new(&resumed(), None);
        p.open();
        let steps = [
            json!({ "userAgent": "codex" }),
            json!({ "account": { "type": "chatgpt" } }),
            json!({ "config": good_config(true, &[], false) }),
            json!({ "data": [] }),
            json!({ "marketplaces": [] }),
            json!({ "data": [] }),
            servers(false, &[]),
        ];
        for (i, r) in steps.into_iter().enumerate() {
            p.line(&reply(i as u64 + 1, r)).unwrap();
        }
        let e = p
            .line(
                &json!({ "jsonrpc": "2.0", "id": 8, "error": {
                    "code": -32600, "message": "no rollout found for thread id t1" } })
                .to_string(),
            )
            .unwrap_err();
        assert_eq!(e, "refused Thread: no rollout found for thread id t1");
        assert!(!p.started);
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
    fn a_rate_limits_update_reports_the_windows_it_names() {
        let mut p = in_turn(&with_bus(true));
        let update = |limits: Value| {
            notify(
                "account/rateLimits/updated",
                json!({ "rateLimits": limits }),
            )
        };
        let both = json!({ "limitId": "codex", "planType": "plus",
            "primary": { "usedPercent": 48.4, "windowDurationMins": 300, "resetsAt": 1_700_000_000u64 },
            "secondary": { "usedPercent": 12.5, "windowDurationMins": 10_080, "resetsAt": null } });
        assert_eq!(
            events(&p.line(&update(both)).unwrap()),
            [Event::Limits(vec![
                Limit {
                    minutes: 300,
                    used: 48,
                    resets_at: Some(1_700_000_000)
                },
                Limit {
                    minutes: 10_080,
                    used: 13,
                    resets_at: None
                },
            ])]
        );
        // Sparse: only the window it names; one past full is full.
        let one = json!({ "primary": null, "secondary": { "usedPercent": 140.0, "windowDurationMins": 10_080 } });
        assert_eq!(
            events(&p.line(&update(one)).unwrap()),
            [Event::Limits(vec![Limit {
                minutes: 10_080,
                used: 100,
                resets_at: None
            }])]
        );
        // A window without a length or a use says nothing.
        for none in [
            json!({ "primary": null, "secondary": null }),
            json!({ "primary": { "usedPercent": 5.0, "windowDurationMins": null } }),
            json!({ "primary": { "windowDurationMins": 300 } }),
            json!({ "primary": { "usedPercent": 5.0, "windowDurationMins": -1 } }),
        ] {
            assert!(p.line(&update(none.clone())).unwrap().is_empty(), "{none}");
        }
        // It is asked for: not among the notifications turned off.
        assert!(!QUIET.contains(&"account/rateLimits/updated"));
    }

    #[test]
    fn a_plan_update_is_the_agent_s_to_do_list_whole() {
        use crate::StepState::*;
        let step = |text: &str, state| Step {
            text: text.into(),
            state,
        };
        let mut p = in_turn(&with_bus(true));
        let update = json!({ "threadId": "t1", "turnId": "u1", "explanation": null, "plan": [
            { "step": "Read the parser", "status": "completed" },
            { "step": "Change it\nin two files", "status": "inProgress" },
            { "step": "Test", "status": "pending" },
            { "step": "Odd", "status": "someday" } ] });
        assert_eq!(
            events(&p.line(&notify("turn/plan/updated", update)).unwrap()),
            [Event::Plan(vec![
                step("Read the parser", Done),
                step("Change it", Doing),
                step("Test", Todo),
                step("Odd", Todo),
            ])]
        );
        let none = json!({ "threadId": "t1", "turnId": "u1", "plan": [] });
        assert_eq!(
            events(&p.line(&notify("turn/plan/updated", none)).unwrap()),
            [Event::Plan(Vec::new())]
        );
        // It is asked for; the proposed-plan deltas are not.
        assert!(!QUIET.contains(&"turn/plan/updated"));
        assert!(QUIET.contains(&"item/plan/delta"));
        // Before the settings are checked, like every turn notification.
        let mut p = Protocol::new(&with_bus(true), None);
        let e = p
            .line(&notify("turn/plan/updated", json!({ "plan": [] })))
            .unwrap_err();
        assert!(e.contains("before its settings were checked"), "{e}");
    }

    #[test]
    fn a_token_usage_update_says_how_full_the_context_is() {
        let mut p = in_turn(&with_bus(true));
        // The last request's tokens less its reasoning, and the window.
        let full = json!({ "threadId": "t1", "turnId": "u1", "tokenUsage": {
            "total": { "inputTokens": 9000, "cachedInputTokens": 8000, "outputTokens": 300, "reasoningOutputTokens": 100, "totalTokens": 17_300 },
            "last": { "inputTokens": 5000, "cachedInputTokens": 4000, "outputTokens": 200, "reasoningOutputTokens": 150, "totalTokens": 9200 },
            "modelContextWindow": 272_000 } });
        assert_eq!(
            events(&p.line(&notify("thread/tokenUsage/updated", full)).unwrap()),
            [Event::Context {
                used: 9050,
                size: Some(272_000)
            }]
        );
        // Without a total: the parts; without a window: none.
        let parts = json!({ "threadId": "t1", "turnId": "u1", "tokenUsage": {
            "total": { "inputTokens": 1, "cachedInputTokens": 1, "outputTokens": 1 },
            "last": { "inputTokens": 5000, "cachedInputTokens": 4000, "outputTokens": 200 },
            "modelContextWindow": null } });
        assert_eq!(
            events(&p.line(&notify("thread/tokenUsage/updated", parts)).unwrap()),
            [Event::Context {
                used: 9200,
                size: None
            }]
        );
        // The turn's note still counts the whole thread's tokens.
        let acts = p
            .line(&notify(
                "turn/completed",
                json!({ "turn": { "id": "u1", "status": "completed" } }),
            ))
            .unwrap();
        assert!(
            events(&acts)
                .iter()
                .any(|e| matches!(e, Event::Note(n) if n.contains("tokens"))),
            "{acts:?}"
        );
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
    fn compact_is_the_one_command_and_runs_as_a_turn_without_the_servers_check() {
        let s = with_bus(true);
        let mut p = started(&s);
        let acts = p.compact();
        let q = sent(&acts);
        assert_eq!(q.len(), 1);
        assert_eq!(q[0]["method"], "thread/compact/start");
        assert_eq!(q[0]["params"], json!({ "threadId": "t1" }));
        assert_eq!(q[0]["id"], 9);
        assert!(p.prompt("next").is_empty(), "the thread is busy");
        assert!(p.compact().is_empty());
        let acts = p.line(&reply(9, json!({}))).unwrap();
        assert_eq!(
            events(&acts),
            [Event::TurnDone {
                ok: true,
                text: "compacted".into(),
                cost: 0,
                denied: 0
            }]
        );
        // The prompt that waited goes next, through the servers check; the
        // second compaction after it.
        let q = sent(&acts);
        assert_eq!(q.len(), 1);
        assert_eq!(q[0]["method"], "mcpServerStatus/list");
        let acts = p.line(&reply(10, servers(true, &[]))).unwrap();
        assert_eq!(sent(&acts)[0]["params"]["input"][0]["text"], "next");
        p.line(&reply(11, json!({ "turn": { "id": "u1" } })))
            .unwrap();
        let acts = p
            .line(&notify(
                "turn/completed",
                json!({ "turn": { "id": "u1", "status": "completed" } }),
            ))
            .unwrap();
        assert_eq!(sent(&acts)[0]["method"], "thread/compact/start");
        // A compaction Codex refuses is a failed turn, not a stopped
        // session: the prompt queued behind it still runs.
        assert!(p.prompt("after").is_empty(), "queued behind the compaction");
        let acts = p
            .line(&json!({ "jsonrpc": "2.0", "id": 12, "error": { "message": "no" } }).to_string())
            .unwrap();
        assert!(
            matches!(
                &acts[0],
                Act::Event(Event::TurnDone { ok: false, text, cost: 0, denied: 0 })
                    if text == "compaction refused: no"
            ),
            "{acts:?}"
        );
        let q = sent(&acts);
        assert_eq!(q.len(), 1);
        assert_eq!(q[0]["method"], "mcpServerStatus/list");
        let acts = p.line(&reply(13, servers(true, &[]))).unwrap();
        assert_eq!(sent(&acts)[0]["params"]["input"][0]["text"], "after");
    }

    #[test]
    fn compact_is_the_one_command_a_session_takes_and_the_rest_is_refused() {
        let (tx, rx) = mpsc::channel();
        // Through the trait, as the screen drives every vendor.
        let mut s: Box<dyn crate::Agent> = Box::new(Session { tx });
        s.command(" /compact ").unwrap();
        assert!(matches!(rx.try_recv(), Ok(Msg::Compact)));
        for line in ["/clear", "/cost", "compact", ""] {
            let e = s.command(line).unwrap_err();
            assert_eq!(e.kind(), io::ErrorKind::Unsupported, "{line:?}");
            assert!(e.to_string().contains("/compact"), "{e}");
        }
        assert!(rx.try_recv().is_err(), "a refused command sends nothing");
        drop(rx);
        let e = s.command("/compact").unwrap_err();
        assert_eq!(e.kind(), io::ErrorKind::BrokenPipe);
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

    fn ask(id: Value, method: &str, params: Value) -> String {
        json!({ "jsonrpc": "2.0", "id": id, "method": method, "params": params }).to_string()
    }

    /// The one [`Event::Asks`] in `acts`.
    fn asks(acts: &[Act]) -> (String, Ask) {
        match events(acts).as_slice() {
            [Event::Asks { request, what }] => (request.clone(), what.clone()),
            e => panic!("not one ask: {e:?}"),
        }
    }

    #[test]
    fn a_writer_s_approvals_wait_for_the_person_and_are_answered_once() {
        let mut p = in_turn(&at(Confine::Standard, true));
        let cmd = |command: &str| json!({ "itemId": "c1", "command": command, "threadId": "t1", "turnId": "u1" });
        let acts = p
            .line(&ask(
                json!(42),
                "item/commandExecution/requestApproval",
                cmd("cargo publish"),
            ))
            .unwrap();
        assert!(
            sent(&acts).is_empty(),
            "nothing is answered before the person"
        );
        assert_eq!(
            asks(&acts),
            (
                "42".to_string(),
                Ask::Command {
                    tool: "shell".into(),
                    command: "cargo publish".into()
                }
            )
        );
        let acts = p
            .line(&ask(
                json!(43),
                "item/commandExecution/requestApproval",
                cmd("rm -rf target"),
            ))
            .unwrap();
        assert_eq!(asks(&acts).0, "43");
        // Answered by request, in any order, each once.
        let r = &sent(&p.decide("43", false))[0];
        assert_eq!(r["id"], 43);
        assert_eq!(r["result"], json!({ "decision": "decline" }));
        assert!(p.decide("43", true).is_empty());
        let r = &sent(&p.decide("42", true))[0];
        assert_eq!(r["id"], 42);
        assert_eq!(r["result"], json!({ "decision": "accept" }));
        assert!(p.decide("42", true).is_empty());
        assert!(p.decide("7", true).is_empty(), "never asked");
        // An id that is text goes back as the same text.
        let acts = p
            .line(&ask(
                json!("a-1"),
                "item/commandExecution/requestApproval",
                cmd("ls"),
            ))
            .unwrap();
        assert_eq!(asks(&acts).0, "\"a-1\"");
        assert_eq!(sent(&p.decide("\"a-1\"", true))[0]["id"], "a-1");
    }

    #[test]
    fn what_codex_asks_is_shown_as_what_it_would_run_or_touch() {
        let mut p = in_turn(&at(Confine::Open, true));
        let what = |p: &mut Protocol, method: &str, params: Value| {
            asks(&p.line(&ask(json!(50), method, params)).unwrap()).1
        };
        let other = |tool: &str, detail: &str| Ask::Other {
            tool: tool.into(),
            detail: detail.into(),
        };
        let command = "item/commandExecution/requestApproval";
        // Input to a running command is not a command to run.
        assert_eq!(
            what(
                &mut p,
                command,
                json!({ "itemId": "c", "kind": "writeStdin", "command": "y" })
            ),
            other("shell stdin", "y")
        );
        assert_eq!(
            what(
                &mut p,
                command,
                json!({ "itemId": "c", "kind": "command", "command": "make" })
            ),
            Ask::Command {
                tool: "shell".into(),
                command: "make".into()
            }
        );
        assert_eq!(
            what(
                &mut p,
                command,
                json!({ "itemId": "c", "command": "curl x", "networkApprovalContext": { "host": "example.com", "protocol": "https" } })
            ),
            other("shell", "curl x (network: example.com)")
        );
        assert_eq!(
            what(&mut p, command, json!({ "itemId": "c" })),
            Ask::Command {
                tool: "shell".into(),
                command: String::new()
            }
        );
        // A file change shows the files its item touches.
        let change = json!({ "item": { "type": "fileChange", "id": "f1", "status": "inProgress", "changes": [
            { "path": "/work/w1/src/a.rs" }, { "path": "/etc/hosts" } ] } });
        p.line(&notify("item/started", change)).unwrap();
        let file = "item/fileChange/requestApproval";
        assert_eq!(
            what(&mut p, file, json!({ "itemId": "f1" })),
            other("apply_patch", "src/a.rs, /etc/hosts")
        );
        assert_eq!(
            what(&mut p, file, json!({ "itemId": "f1", "grantRoot": "/etc" })),
            other("apply_patch", "src/a.rs, /etc/hosts; writes under /etc")
        );
        assert_eq!(
            what(&mut p, file, json!({ "itemId": "f2", "grantRoot": "/etc" })),
            other("apply_patch", "writes under /etc")
        );
        assert_eq!(
            what(&mut p, file, json!({ "itemId": "f2" })),
            other("apply_patch", "")
        );
        // A finished change is forgotten.
        let done = json!({ "item": { "type": "fileChange", "id": "f1", "status": "completed", "changes": [] } });
        p.line(&notify("item/completed", done)).unwrap();
        assert_eq!(
            what(&mut p, file, json!({ "itemId": "f1" })),
            other("apply_patch", "")
        );
        // Permissions show what they would grant, cut short.
        let perms = "item/permissions/requestApproval";
        assert_eq!(
            what(
                &mut p,
                perms,
                json!({ "itemId": "p", "permissions": { "network": { "enabled": true } } })
            ),
            other("permissions", r#"{"network":{"enabled":true}}"#)
        );
        // `{"w":"…"}` is 8 characters around the text.
        let long = |n: usize| json!({ "itemId": "p", "permissions": { "w": "x".repeat(n - 8) } });
        let Ask::Other { detail, .. } = what(&mut p, perms, long(200)) else {
            panic!()
        };
        assert_eq!(detail.chars().count(), 200);
        assert!(!detail.ends_with('…'));
        let Ask::Other { detail, .. } = what(&mut p, perms, long(201)) else {
            panic!()
        };
        assert_eq!(detail.chars().count(), 201);
        assert!(detail.ends_with("x\"…"), "{detail}");
    }

    #[test]
    fn permissions_are_granted_for_the_turn_or_none_are() {
        let mut p = in_turn(&at(Confine::Standard, true));
        let perms = json!({ "network": { "enabled": true } });
        let params = json!({ "itemId": "p", "cwd": "/work/w1", "permissions": perms });
        p.line(&ask(
            json!(60),
            "item/permissions/requestApproval",
            params.clone(),
        ))
        .unwrap();
        p.line(&ask(json!(61), "item/permissions/requestApproval", params))
            .unwrap();
        let r = &sent(&p.decide("60", true))[0];
        assert_eq!(
            r["result"],
            json!({ "permissions": perms, "scope": "turn" })
        );
        let r = &sent(&p.decide("61", false))[0];
        assert_eq!(r["result"], json!({ "permissions": {}, "scope": "turn" }));
    }

    #[test]
    fn an_ask_codex_settles_or_whose_turn_ends_is_withdrawn() {
        let mut p = in_turn(&at(Confine::Standard, true));
        let cmd = json!({ "itemId": "c1", "command": "make" });
        for id in [70, 71, 72] {
            p.line(&ask(
                json!(id),
                "item/commandExecution/requestApproval",
                cmd.clone(),
            ))
            .unwrap();
        }
        let acts = p
            .line(&notify(
                "serverRequest/resolved",
                json!({ "requestId": 71, "threadId": "t1" }),
            ))
            .unwrap();
        assert_eq!(
            events(&acts),
            [Event::Withdrawn {
                request: "71".into()
            }]
        );
        assert!(p.decide("71", true).is_empty(), "gone");
        // One that was answered, or never asked, is nothing to withdraw.
        assert!(!p.decide("70", true).is_empty());
        for id in [70, 99] {
            let acts = p
                .line(&notify(
                    "serverRequest/resolved",
                    json!({ "requestId": id }),
                ))
                .unwrap();
            assert!(acts.is_empty(), "{id}");
        }
        p.line(&ask(
            json!(73),
            "item/commandExecution/requestApproval",
            cmd,
        ))
        .unwrap();
        let acts = p
            .line(&notify(
                "turn/completed",
                json!({ "turn": { "id": "u1", "status": "interrupted" } }),
            ))
            .unwrap();
        let e = events(&acts);
        assert_eq!(
            e[..2],
            [
                Event::Withdrawn {
                    request: "72".into()
                },
                Event::Withdrawn {
                    request: "73".into()
                }
            ]
        );
        assert!(matches!(e[2], Event::TurnDone { ok: false, .. }), "{e:?}");
        assert!(p.decide("72", true).is_empty());
        assert!(p.decide("73", true).is_empty());
    }

    #[test]
    fn an_approval_that_may_not_be_asked_stops_the_session() {
        let methods = [
            "item/commandExecution/requestApproval",
            "item/fileChange/requestApproval",
            "item/permissions/requestApproval",
        ];
        // A leader, and every session under strict, never asks.
        for spec in [at(Confine::Standard, false), at(Confine::Strict, true)] {
            for m in methods {
                let mut p = in_turn(&spec);
                let e = p.line(&ask(json!(1), m, json!({}))).unwrap_err();
                assert!(e.contains("although none may be asked"), "{m}: {e}");
            }
        }
        // Nor before the settings are checked.
        for m in methods {
            let mut p = Protocol::new(&at(Confine::Standard, true), None);
            p.open();
            let e = p.line(&ask(json!(1), m, json!({}))).unwrap_err();
            assert!(e.contains("before its settings were checked"), "{m}: {e}");
        }
        // The old protocol's approvals are not answered.
        for m in ["applyPatchApproval", "execCommandApproval"] {
            let mut p = in_turn(&at(Confine::Standard, true));
            let e = p.line(&ask(json!(1), m, json!({}))).unwrap_err();
            assert!(e.contains("the old way"), "{m}: {e}");
        }
    }

    #[test]
    fn a_command_codex_wraps_in_a_shell_is_asked_for_as_the_agent_wrote_it() {
        assert_eq!(unwrapped("/bin/zsh -lc 'cargo test'"), "cargo test");
        assert_eq!(unwrapped("bash -c 'git status'"), "git status");
        assert_eq!(unwrapped("/usr/bin/sh -lc \"ls -la\""), "ls -la");
        // What is inside stays whole, to be judged as it is.
        assert_eq!(
            unwrapped("/bin/bash -lc 'cargo test && rm -rf x'"),
            "cargo test && rm -rf x"
        );
        for kept in [
            "/tmp/zsh -lc 'cargo test'",
            "/bin/zsh -i 'cargo test'",
            "/bin/zsh -lc 'cargo test' extra",
            "/bin/zsh -lc",
            "cargo test",
            "/bin/zsh -lc \"echo $HOME\"",
            "",
        ] {
            assert_eq!(unwrapped(kept), kept);
        }
        let mut p = in_turn(&at(Confine::Standard, true));
        let acts = p
            .line(&ask(
                json!(1),
                "item/commandExecution/requestApproval",
                json!({ "itemId": "c", "command": "/bin/zsh -lc 'cargo test'" }),
            ))
            .unwrap();
        assert_eq!(
            asks(&acts).1,
            Ask::Command {
                tool: "shell".into(),
                command: "cargo test".into()
            }
        );
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
                Event::Commands(vec!["/compact".into()]),
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
    fn the_person_s_answer_reaches_codex_through_the_session() {
        let dir = std::env::temp_dir().join(format!("imece-codex-ask-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let cwd = dir.canonicalize().unwrap();
        let mut s = spec(true);
        s.confine = Confine::Standard;
        s.cwd = cwd.clone();
        let cwd_text = cwd.to_string_lossy();
        let mut answers = clean_answers(true, &cwd_text);
        answers[2] = json!({ "config": config_at(true, &[], false, Confine::Standard) });
        answers[7] = thread_at(true, &cwd_text, Confine::Standard);
        let turn = [ask(
            json!(77),
            "item/commandExecution/requestApproval",
            json!({ "itemId": "c1", "command": "cargo publish" }),
        )];
        // After the ask, the fake waits for the answer, keeps it, and ends
        // the turn.
        let body = fake_body(&answers, &turn).replace(
            "\n    exit 0\n",
            &format!(
                "\n    read -r answer; printf '%s\\n' \"$answer\" > answer.txt\n    echo '{}'\n    exit 0\n",
                notify(
                    "turn/completed",
                    json!({ "turn": { "id": "u1", "status": "completed" } })
                )
            ),
        );
        s.program = fake(&dir, "fake-codex", &body);
        let (tx, rx) = mpsc::channel();
        let mut session: Box<dyn crate::Agent> = Box::new(
            Session::start(&s, move |e| {
                let _ = tx.send(e);
            })
            .unwrap(),
        );
        session.prompt("publish").unwrap();
        let mut got = Vec::new();
        loop {
            let e = rx.recv_timeout(Duration::from_secs(20)).expect("in time");
            if let Event::Asks { request, .. } = &e {
                session.decide(request, false).unwrap();
            }
            let end = matches!(e, Event::Stopped(_));
            got.push(e);
            if end {
                break;
            }
        }
        assert!(
            got.contains(&Event::Asks {
                request: "77".into(),
                what: Ask::Command {
                    tool: "shell".into(),
                    command: "cargo publish".into()
                }
            }),
            "{got:?}"
        );
        assert!(
            got.iter()
                .any(|e| matches!(e, Event::TurnDone { ok: true, .. }))
        );
        let answer: Value =
            serde_json::from_str(&std::fs::read_to_string(cwd.join("answer.txt")).unwrap())
                .unwrap();
        assert_eq!(
            answer,
            json!({ "jsonrpc": "2.0", "id": 77, "result": { "decision": "decline" } })
        );
        // A session that is over takes no answer, once its driver ended:
        // the stop is told just before that, so a decide in between still
        // reaches the channel.
        drop(rx);
        let deadline = Instant::now() + Duration::from_secs(5);
        let e = loop {
            match session.decide("77", true) {
                Ok(()) => assert!(Instant::now() < deadline, "the driver ended"),
                Err(e) => break e,
            }
            thread::sleep(Duration::from_millis(20));
        };
        let _ = std::fs::remove_dir_all(&dir);
        assert_eq!(e.kind(), io::ErrorKind::BrokenPipe);
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
            matches!(&got[3], Event::TurnDone { ok: true, .. }),
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

    #[test]
    fn a_refused_write_reports_the_process_end_of_its_generation_in_time() {
        let (tx, rx) = mpsc::channel();
        let e = io::Error::from(io::ErrorKind::BrokenPipe);
        tx.send(Msg::End(0, "the first process".into())).unwrap();
        tx.send(Msg::Line(1, "late words".into())).unwrap();
        tx.send(Msg::End(1, "exited with code 3: no auth".into()))
            .unwrap();
        assert_eq!(refused(&rx, 1, &e), "exited with code 3: no auth");
        // An end reported a moment after the refused write still counts.
        let late = tx.clone();
        thread::spawn(move || {
            thread::sleep(Duration::from_millis(200));
            late.send(Msg::End(1, "exited with code 1: late".into()))
                .unwrap();
        });
        assert_eq!(refused(&rx, 1, &e), "exited with code 1: late");
        drop(tx);
        let why = refused(&rx, 1, &e);
        assert!(why.starts_with("stopped: codex took no input ("), "{why}");
        assert!(why.contains("broken pipe"), "{why}");
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
            confine: Confine::Strict,
            system: "You are codex-1, a coding agent in imece, a team led by lead. \
                     You can read files and run commands that only read."
                .into(),
            mcp: Some(Mcp {
                command: "nc".into(),
                args: vec!["-U".into(), sock.to_string_lossy().into_owned()],
                tools: vec!["send".into()],
            }),
            resume: None,
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

    /// `IMECE_LIVE_CODEX=1 cargo test -p imece-agent -- --ignored live_codex_reports --nocapture`
    ///
    /// Codex's `update_plan` tool is on in an app-server thread without
    /// being asked for: its plan comes as `turn/plan/updated`.
    #[test]
    #[ignore = "drives the real codex; set IMECE_LIVE_CODEX=1"]
    fn live_codex_reports_its_plan() {
        if std::env::var_os("IMECE_LIVE_CODEX").is_none() {
            return;
        }
        let dir =
            std::env::temp_dir().join(format!("imece-live-codex-plan-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let cwd = dir.canonicalize().unwrap();
        let s = Spec {
            program: PROGRAM.into(),
            cwd: cwd.clone(),
            choice: Some("@low".into()),
            write: false,
            confine: Confine::Strict,
            system: "You are codex-1, a coding agent in imece.".into(),
            mcp: None,
            resume: None,
        };
        let (tx, rx) = mpsc::channel();
        let mut session = Session::start(&s, move |e| {
            let _ = tx.send(e);
        })
        .unwrap();
        session
            .prompt(
                "With your update_plan tool, write a plan of exactly three steps and mark \
                 the first one in progress. Then reply with the single word: pong",
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
        let plans: Vec<&Vec<Step>> = got
            .iter()
            .filter_map(|e| match e {
                Event::Plan(p) => Some(p),
                _ => None,
            })
            .collect();
        assert!(
            plans
                .iter()
                .any(|p| p.len() == 3 && p.iter().any(|s| s.state == crate::StepState::Doing)),
            "a plan of three with one in hand: {got:?}"
        );
        assert!(
            matches!(got.last(), Some(Event::TurnDone { ok: true, .. })),
            "{got:?}"
        );
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// `IMECE_LIVE_CODEX=1 cargo test -p imece-agent -- --ignored live_codex_asks --nocapture`
    ///
    /// A worker under standard asks before it writes outside its
    /// directory; refused, the file is not made, allowed, it is.
    #[test]
    #[ignore = "drives the real codex; set IMECE_LIVE_CODEX=1"]
    fn live_codex_asks_before_a_write_outside_its_directory_and_does_what_it_hears() {
        if std::env::var_os("IMECE_LIVE_CODEX").is_none() {
            return;
        }
        let dir = std::env::temp_dir().join(format!("imece-live-codex-ask-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let cwd = dir.canonicalize().unwrap();
        // Outside the directory and outside the temporary ones, which the
        // sandbox lets a worker write.
        let outside = std::env::home_dir()
            .unwrap()
            .join(format!(".imece-live-codex-{}", std::process::id()));
        std::fs::create_dir_all(&outside).unwrap();
        let s = Spec {
            program: PROGRAM.into(),
            cwd: cwd.clone(),
            choice: Some("@low".into()),
            write: true,
            confine: Confine::Standard,
            system: "You are codex-1, a coding agent in imece. You can read and change \
                     files here and run commands; what needs more than the sandbox gives \
                     waits for the person's yes. A refusal is the person's decision."
                .into(),
            mcp: None,
            resume: None,
        };
        let (tx, rx) = mpsc::channel();
        let mut session = Session::start(&s, move |e| {
            let _ = tx.send(e);
        })
        .unwrap();
        let turn = |session: &mut Session, file: &str, allow: bool| {
            let target = outside.join(file);
            session
                .prompt(&format!(
                    "Run the shell command `touch {}`. The file is outside your directory, \
                     so ask to run it outside the sandbox (escalated permissions) right \
                     away. If you are refused, reply with the single word: refused. \
                     Else reply with the single word: done.",
                    target.display()
                ))
                .unwrap();
            let mut got = Vec::new();
            loop {
                let e = rx
                    .recv_timeout(Duration::from_secs(300))
                    .expect("codex answers within five minutes");
                eprintln!("live codex: {e:?}");
                if let Event::Asks { request, .. } = &e {
                    session.decide(request, allow).unwrap();
                }
                let end = matches!(e, Event::TurnDone { .. } | Event::Stopped(_));
                got.push(e);
                if end {
                    return (got, target.exists());
                }
            }
        };
        let (no, made) = turn(&mut session, "no.txt", false);
        let (yes, made_too) = turn(&mut session, "yes.txt", true);
        drop(session);
        let _ = std::fs::remove_dir_all(&outside);
        let _ = std::fs::remove_dir_all(&dir);
        let asked = |got: &[Event]| got.iter().any(|e| matches!(e, Event::Asks { .. }));
        assert!(asked(&no), "{no:?}");
        assert!(!made, "a refused write was made: {no:?}");
        assert!(
            matches!(no.last(), Some(Event::TurnDone { ok: true, .. })),
            "{no:?}"
        );
        assert!(asked(&yes), "{yes:?}");
        assert!(made_too, "an allowed write was not made: {yes:?}");
    }

    /// `IMECE_LIVE_CODEX=1 cargo test -p imece-agent -- --ignored live_codex_resumes --nocapture`
    ///
    /// Three processes: one that takes a turn in a new thread, one that
    /// resumes that thread and must remember the turn, one that asks for a
    /// thread there is no rollout of and must stop before any turn. The
    /// rollouts stay under `~/.codex/sessions` (imece deletes none yet).
    #[test]
    #[ignore = "drives the real codex; set IMECE_LIVE_CODEX=1"]
    fn live_codex_resumes_its_thread_and_stops_on_one_that_is_gone() {
        if std::env::var_os("IMECE_LIVE_CODEX").is_none() {
            return;
        }
        let dir = std::env::temp_dir().join(format!("imece-live-codex-r-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let cwd = dir.canonicalize().unwrap();
        let mut spec = Spec {
            program: PROGRAM.into(),
            cwd: cwd.clone(),
            choice: Some("@low".into()),
            write: false,
            confine: Confine::Strict,
            system: "You are codex-1, a coding agent in imece. Answer briefly.".into(),
            mcp: None,
            resume: None,
        };
        // One process, one prompt: every event until the turn or the
        // session ends.
        let run = |spec: &Spec, prompt: &str| -> Vec<Event> {
            let (tx, rx) = mpsc::channel();
            let mut session = Session::start(spec, move |e| {
                let _ = tx.send(e);
            })
            .unwrap();
            session.prompt(prompt).unwrap();
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
            got
        };
        let first = run(&spec, "Reply with exactly the word: marigold");
        let Some(Event::Started {
            session: thread, ..
        }) = first.first().cloned()
        else {
            panic!("the checks passed: {first:?}");
        };
        assert!(crate::valid_uuid(&thread), "{thread}");
        assert!(
            matches!(first.last(), Some(Event::TurnDone { ok: true, .. })),
            "{first:?}"
        );
        spec.resume = Some(thread.clone());
        let again = run(
            &spec,
            "Which word did I ask you to reply with before? Answer with that word only.",
        );
        assert_eq!(
            again
                .first()
                .map(|e| matches!(e, Event::Started { session, .. } if *session == thread)),
            Some(true),
            "the same thread: {again:?}"
        );
        assert!(
            matches!(again.last(), Some(Event::TurnDone { ok: true, text, .. }) if text.to_lowercase().contains("marigold")),
            "{again:?}"
        );
        spec.resume = Some("01a18b35-6d6d-70f2-94d1-5ac2cf796999".into());
        let gone = run(&spec, "unused");
        assert_eq!(gone.len(), 1, "{gone:?}");
        assert!(
            matches!(&gone[0], Event::Stopped(why) if why.contains("no rollout found")),
            "{gone:?}"
        );
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// A stand-in app server at `path`: logs every request to `log`,
    /// answers `initialize`, and answers `thread/delete` with success,
    /// with Codex's error for `gone`, or by exiting for `mute`.
    fn fake_app_server(dir: &Path, gone: &str, mute: &str) -> (String, PathBuf) {
        use std::os::unix::fs::PermissionsExt;
        let path = dir.join("fake-codex");
        let log = dir.join("requests.log");
        let script = format!(
            "#!/bin/sh\n\
             [ \"$1\" = app-server ] || exit 9\n\
             printf '%s\\n' 'not json' '{{\"jsonrpc\":\"2.0\",\"method\":\"noise\"}}'\n\
             while IFS= read -r line; do\n\
               printf '%s\\n' \"$line\" >> {log}\n\
               case \"$line\" in\n\
                 *'\"method\":\"initialize\"'*) printf '%s\\n' '{{\"jsonrpc\":\"2.0\",\"id\":1,\"result\":{{\"userAgent\":\"codex\"}}}}' ;;\n\
                 *'\"method\":\"thread/delete\"'*)\n\
                   case \"$line\" in\n\
                     *{gone}*) printf '%s\\n' '{{\"jsonrpc\":\"2.0\",\"id\":2,\"error\":{{\"code\":-32600,\"message\":\"no rollout found for thread id {gone}\"}}}}' ;;\n\
                     *{mute}*) exit 0 ;;\n\
                     *) printf '%s\\n' '{{\"jsonrpc\":\"2.0\",\"id\":7,\"result\":{{}}}}' '{{\"jsonrpc\":\"2.0\",\"id\":2,\"result\":{{}}}}' ;;\n\
                   esac ;;\n\
               esac\n\
             done\n",
            log = log.display(),
        );
        std::fs::write(&path, script).unwrap();
        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o700)).unwrap();
        (path.to_string_lossy().into_owned(), log)
    }

    #[test]
    fn delete_asks_an_app_server_of_its_own_to_delete_the_thread_and_reports_its_answer() {
        let dir = std::env::temp_dir().join(format!("imece-codex-delete-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let id = "01a14673-5104-70b2-b314-a45e688f44da";
        let gone = "01a14673-0000-70b2-b314-a45e688f44da";
        let mute = "01a14673-1111-70b2-b314-a45e688f44da";
        let (program, log) = fake_app_server(&dir, gone, mute);

        delete(&program, id).unwrap();
        let requests = std::fs::read_to_string(&log).unwrap();
        let lines: Vec<Value> = requests
            .lines()
            .map(|l| serde_json::from_str(l).unwrap())
            .collect();
        assert_eq!(lines.len(), 2, "{requests}");
        assert_eq!(lines[0]["method"], "initialize");
        assert_eq!(lines[0]["id"], 1);
        assert_eq!(lines[0]["params"]["clientInfo"]["name"], "imece");
        assert_eq!(lines[1]["method"], "thread/delete");
        assert_eq!(lines[1]["id"], 2);
        assert_eq!(lines[1]["params"], json!({ "threadId": id }));
        std::fs::remove_file(&log).unwrap();

        // Codex's error is imece's, with the method in front.
        let e = delete(&program, gone).unwrap_err();
        assert!(
            e.to_string()
                .starts_with("thread/delete: no rollout found for thread id"),
            "{e}"
        );
        assert_eq!(
            std::fs::read_to_string(&log).unwrap().lines().count(),
            2,
            "both requests were made"
        );
        std::fs::remove_file(&log).unwrap();

        // A server that ends without answering is an error, not a wait.
        let e = delete(&program, mute).unwrap_err();
        assert!(e.to_string().contains("codex ended first"), "{e}");

        // Only a thread id goes to Codex: nothing is started for another.
        let e = delete(&program, "t1; rm -rf /").unwrap_err();
        assert_eq!(e.kind(), io::ErrorKind::InvalidInput);
        assert_eq!(
            std::fs::read_to_string(&log).unwrap().lines().count(),
            2,
            "the muted call's requests only"
        );
        // A program that is not there is an error too.
        assert!(delete("/nonexistent/codex", id).is_err());
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn an_answer_is_the_line_with_the_request_s_id_whatever_comes_before() {
        let (tx, rx) = mpsc::channel();
        let soon = Instant::now() + Duration::from_secs(5);
        for l in [
            "garbage",
            r#"{"jsonrpc":"2.0","method":"thread/started","params":{}}"#,
            r#"{"jsonrpc":"2.0","id":3,"result":{"other":true}}"#,
            r#"{"jsonrpc":"2.0","id":2,"result":{"ok":true}}"#,
        ] {
            tx.send(Ok(l.to_string())).unwrap();
        }
        assert_eq!(answer(&rx, soon, 2), Ok(json!({ "ok": true })));
        tx.send(Ok(
            r#"{"jsonrpc":"2.0","id":4,"error":{"code":1,"message":"nope"}}"#.to_string(),
        ))
        .unwrap();
        assert_eq!(answer(&rx, soon, 4), Err("nope".to_string()));
        tx.send(Ok(
            r#"{"jsonrpc":"2.0","id":5,"error":{"code":1}}"#.to_string()
        ))
        .unwrap();
        assert_eq!(answer(&rx, soon, 5), Err("error".to_string()));
        tx.send(Err("exit 1".to_string())).unwrap();
        assert_eq!(
            answer(&rx, soon, 6),
            Err("codex ended first (exit 1)".to_string())
        );
        // Nothing comes: the deadline, not forever.
        assert_eq!(
            answer(&rx, Instant::now() + Duration::from_millis(50), 7),
            Err("codex did not answer in time".to_string())
        );
    }
}
