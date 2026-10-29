//! Gemini CLI as an agent: one long-lived `gemini --acp` per agent (never a
//! process per turn), the Agent Client Protocol as JSON-RPC over stdio.
//! imece's requests go in on stdin; Gemini's answers, notifications and
//! requests come out on stdout, one JSON object per line.
//!
//! Gemini CLI has nothing like `--restricted`: its settings come from the
//! person's own `~/.gemini/settings.json` and from the `.gemini` directory
//! of the folder it runs in, and only a root-owned system settings file
//! could override them. What imece can set, it sets on the command line:
//! its own POLICY file (the user tier's only policy once `--policy` names
//! it; just root-owned system policies rank above it) denies every tool
//! and allows exactly the file tools of the role and imece's own MCP
//! tools, so nothing is ever asked (what would ask is denied instead, and
//! imece denies every permission request anyway); extensions are off, no
//! MCP server but imece's is allowed; the approval mode is `default`.
//! Gemini runs in a private directory imece owns (its working directory,
//! holding the policy and a `GEMINI.md` with the agent's instructions, the
//! only way ACP has to give instructions) and gets the agent's files as an
//! included directory: a repository's `.gemini` settings, hooks and `.env`
//! never become this session's. The folder is trusted (an untrusted one
//! starts no MCP server at all), so Gemini's own path checks keep the file
//! tools inside the workspace. What Gemini then reports (`initialize`,
//! `session/new`, and its `/about`, `/extensions list` and `/memory list`
//! commands, which cost no tokens) is checked before the first turn;
//! anything else stops the session.
//!
//! Gemini reports tokens, not dollars. An API-key login is booked from the
//! list prices imece carries (a model it does not know stops the session);
//! a Google login runs on the person's plan and books nothing, said once.

use std::collections::{BTreeSet, HashMap, VecDeque};
use std::io;
use std::path::{Path, PathBuf};
use std::sync::mpsc::{self, Receiver, RecvTimeoutError, Sender};
use std::thread;
use std::time::{Duration, Instant};

use serde_json::{Value, json};

use crate::child::{self, Child, Out};
use crate::{Event, MCP_SERVER, Mcp, mcp_tool};

/// Variables Gemini CLI may need besides [`child::BASE_ENV`]: an API key
/// (a key entered in the CLI lives in the keychain), its config home, and
/// proxy and CA settings of corporate networks.
pub const ENV: &[&str] = &[
    "GEMINI_API_KEY",
    "GOOGLE_API_KEY",
    "GOOGLE_CLOUD_PROJECT",
    "GOOGLE_CLOUD_LOCATION",
    "GEMINI_CLI_HOME",
    "HTTPS_PROXY",
    "HTTP_PROXY",
    "NO_PROXY",
    "https_proxy",
    "http_proxy",
    "no_proxy",
    "NODE_EXTRA_CA_CERTS",
    "SSL_CERT_FILE",
];

/// Variables every session gets: the private directory is trusted (an
/// untrusted one starts no MCP server), telemetry is off, and a login that
/// is missing fails instead of opening a browser.
pub const FIXED_ENV: &[(&str, &str)] = &[
    ("GEMINI_CLI_TRUST_WORKSPACE", "true"),
    ("GEMINI_TELEMETRY_ENABLED", "false"),
    ("NO_BROWSER", "true"),
];

/// The program on PATH.
pub const PROGRAM: &str = "gemini";

/// The ACP version imece speaks.
const PROTOCOL_VERSION: u64 = 1;

/// Gemini CLI's model for a tier: its own aliases, which follow the newest
/// model of each size. `Standard` is Gemini CLI's own default (`auto`,
/// which picks a model per request).
pub fn model(tier: crate::Tier) -> Option<&'static str> {
    match tier {
        crate::Tier::Cheap => Some("flash"),
        crate::Tier::Standard => None,
        crate::Tier::Strong => Some("pro"),
    }
}

/// What Gemini CLI's `/about` reports as the model when none was named.
const AUTO: &str = "auto";

/// Tools that read files, as Gemini names them.
pub const READ_TOOLS: &[&str] = &["read_file", "list_directory", "glob", "grep_search"];
/// Tools that read and change files.
pub const EDIT_TOOLS: &[&str] = &[
    "read_file",
    "list_directory",
    "glob",
    "grep_search",
    "write_file",
    "replace",
];

/// The file imece's instructions go in, the one Gemini reads as context.
const INSTRUCTIONS: &str = "GEMINI.md";
/// The policy directory inside the private directory.
const POLICY_DIR: &str = "policy";

/// List prices in thousandths of a dollar per million tokens: model name
/// prefix, input, output, cached input. Read from
/// ai.google.dev/gemini-api/docs/pricing on 2026-10-29 (paid tier, prompts
/// up to 200K tokens; the 3.8 and 3.6 Flash prices double on 2027-01-01).
/// Thought tokens are output. The longest prefix wins.
const PRICES: &[(&str, u64, u64, u64)] = &[
    ("gemini-3.8-flash", 750, 3750, 75),
    ("gemini-3.6-flash", 750, 3750, 75),
    ("gemini-3.5-flash-lite", 300, 2500, 30),
    ("gemini-3.1-pro-preview", 2000, 12000, 200),
    ("gemini-3.1-flash-lite", 250, 1500, 25),
    ("gemini-3-flash-preview", 500, 3000, 50),
    ("gemini-2.5-pro", 1250, 10000, 125),
    ("gemini-2.5-flash-lite", 100, 400, 10),
    ("gemini-2.5-flash", 300, 2500, 30),
];

/// The list price of `model` (input, output, cached input), if imece knows
/// it.
pub fn price(model: &str) -> Option<(u64, u64, u64)> {
    PRICES
        .iter()
        .filter(|(prefix, ..)| model.starts_with(prefix))
        .max_by_key(|(prefix, ..)| prefix.len())
        .map(|&(_, i, o, c)| (i, o, c))
}

/// What `input` tokens (of which `cached` were read from the cache) and
/// `output` tokens (thoughts included) cost on `model`, in millionths of a
/// dollar, rounded up; none for a model imece can not price.
pub fn cost(model: &str, input: u64, cached: u64, output: u64) -> Option<u64> {
    let (i, o, c) = price(model)?;
    let fresh = input.saturating_sub(cached);
    let thousandths = fresh * i + cached * c + output * o;
    Some(thousandths.div_ceil(1000))
}

/// How long an answer to one of imece's requests may take, a turn apart.
const REQUEST_WAIT: Duration = Duration::from_secs(60);
const POLL: Duration = Duration::from_millis(500);
/// How long a refused write waits for the process's own end to be
/// reported, which says why (an exit code and its last words) where the
/// write error only says "broken pipe".
const END_WAIT: Duration = Duration::from_secs(2);

#[derive(Clone, Debug)]
pub struct Spec {
    /// `gemini`, or a stand-in in tests.
    pub program: String,
    /// The agent's files: the checkout or its worktree, an included
    /// directory of the session's workspace.
    pub cwd: PathBuf,
    /// The private directory Gemini runs in: imece's policy and
    /// instructions live here, nothing else. Created on start.
    pub home: PathBuf,
    /// A model name or alias, or none for Gemini CLI's own default.
    pub model: Option<String>,
    /// Whether the agent changes files or only reads them.
    pub write: bool,
    /// The agent's instructions: its role in imece.
    pub system: String,
    /// imece's MCP server, if the agent gets its tools.
    pub mcp: Option<Mcp>,
}

impl Spec {
    fn tools(&self) -> &'static [&'static str] {
        if self.write { EDIT_TOOLS } else { READ_TOOLS }
    }

    fn policy_dir(&self) -> PathBuf {
        self.home.join(POLICY_DIR)
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

/// The policy for `spec`: the role's file tools and imece's own tools
/// allowed by exact name, every other tool denied (and so not even shown
/// to the model). The highest priority wins in Gemini's policy engine, so
/// the allows rank first, the denial right under them, and both above
/// anything the person's settings add.
pub fn policy(spec: &Spec) -> String {
    let list = |names: &[String]| {
        format!(
            "[{}]",
            names
                .iter()
                .map(|n| toml_str(n))
                .collect::<Vec<_>>()
                .join(", ")
        )
    };
    let tools: Vec<String> = spec.tools().iter().map(|t| t.to_string()).collect();
    let mut out = format!(
        "# Written by imece for this session; imece replaces it on every start.\n\
         [[rule]]\n\
         toolName = {}\n\
         decision = \"allow\"\n\
         priority = 999\n\
         \n\
         [[rule]]\n\
         toolName = \"*\"\n\
         decision = \"deny\"\n\
         priority = 998\n\
         denyMessage = \"imece: this tool is not available in this session\"\n",
        list(&tools)
    );
    if let Some(m) = &spec.mcp
        && !m.tools.is_empty()
    {
        // By the full name the model calls them by: the policy's own
        // `mcpName` form matches when the model is shown the tools, but
        // not when one is called.
        let names: Vec<String> = m.tools.iter().map(|t| gemini_name(t)).collect();
        out.push_str(&format!(
            "\n[[rule]]\ntoolName = {}\ndecision = \"allow\"\npriority = 999\n",
            list(&names)
        ));
    }
    out
}

/// The instructions file: the agent's role, and where its files are.
pub fn instructions(spec: &Spec) -> String {
    format!(
        "{}\n\nYour files are in {}, an included directory of your workspace; name them by \
         their absolute path under it. The directory you run in holds only imece's own files; \
         leave it alone.\n",
        spec.system.trim_end(),
        spec.cwd.display()
    )
}

/// Makes the private directory ready: the policy and the instructions,
/// written fresh on every start.
pub fn prepare(spec: &Spec) -> io::Result<()> {
    use std::os::unix::fs::DirBuilderExt;
    let mut b = std::fs::DirBuilder::new();
    b.recursive(true).mode(0o700);
    b.create(&spec.home)?;
    b.create(spec.policy_dir())?;
    std::fs::write(spec.policy_dir().join("imece.toml"), policy(spec))?;
    std::fs::write(spec.home.join(INSTRUCTIONS), instructions(spec))?;
    Ok(())
}

/// The command line for `spec`. Prompts never go here; they go to the
/// session.
pub fn args(spec: &Spec) -> Vec<String> {
    let mut a: Vec<String> = vec![
        "--acp".into(),
        "--policy".into(),
        spec.policy_dir().to_string_lossy().into_owned(),
        "--approval-mode".into(),
        "default".into(),
        "--include-directories".into(),
        spec.cwd.to_string_lossy().into_owned(),
        // No extensions, and no MCP server but imece's own, whatever the
        // person's settings bring.
        "-e".into(),
        "none".into(),
        "--allowed-mcp-server-names".into(),
        MCP_SERVER.into(),
    ];
    if let Some(m) = &spec.model {
        a.push("-m".into());
        a.push(m.clone());
    }
    a
}

/// What the driver does with what the protocol decided.
#[derive(Clone, Debug, PartialEq)]
pub enum Act {
    Event(Event),
    /// One line for Gemini's stdin.
    Send(String),
}

/// The answer the protocol waits for.
#[derive(Clone, Copy, Debug, PartialEq)]
enum Wait {
    Initialize,
    Session,
    About,
    Extensions,
    Memory,
    Turn,
}

/// How the session's turns are paid for.
#[derive(Clone, Copy, Debug, PartialEq)]
enum Pay {
    /// An API key: list prices.
    Priced,
    /// A Google login: the person's plan.
    Plan,
}

/// Tool kinds a leader's session must never show; a worker's may edit.
const NEVER: &[&str] = &["execute", "fetch", "switch_mode"];
const WRITES: &[&str] = &["edit", "delete", "move"];

/// Turns stdout lines into [`Act`]s and checks what Gemini reports.
pub struct Protocol {
    cwd: PathBuf,
    home: PathBuf,
    model: Option<String>,
    write: bool,
    mcp: Option<Mcp>,
    /// Tools imece's server offers, as Gemini names them to the model.
    tools: BTreeSet<String>,
    next_id: u64,
    awaiting: Option<(u64, Wait)>,
    session: Option<String>,
    started: bool,
    pay: Option<Pay>,
    queue: VecDeque<Queued>,
    busy: bool,
    /// The agent's text since the last turn or command started.
    text: String,
    /// Names of the tool calls under way, by id.
    calls: HashMap<String, String>,
    denied: usize,
    /// What the session has cost so far, in millionths of a dollar.
    cost: u64,
    /// The session's tokens so far: input, cached, output.
    tokens: (u64, u64, u64),
}

impl Protocol {
    pub fn new(spec: &Spec) -> Protocol {
        Protocol {
            cwd: spec.cwd.clone(),
            home: spec.home.clone(),
            model: spec.model.clone(),
            write: spec.write,
            mcp: spec.mcp.clone(),
            tools: spec
                .mcp
                .iter()
                .flat_map(|m| m.tools.iter().map(|t| gemini_name(t)))
                .collect(),
            next_id: 0,
            awaiting: None,
            session: None,
            started: false,
            pay: None,
            queue: VecDeque::new(),
            busy: false,
            text: String::new(),
            calls: HashMap::new(),
            denied: 0,
            cost: 0,
            tokens: (0, 0, 0),
        }
    }

    /// Whether an answer from Gemini is due soon. A turn's answer comes
    /// when the turn ends, which may take long.
    pub fn waiting(&self) -> bool {
        self.awaiting.is_some_and(|(_, w)| w != Wait::Turn)
    }

    /// The first request: `initialize`.
    pub fn open(&mut self) -> Vec<Act> {
        vec![self.request(
            "initialize",
            json!({
                "protocolVersion": PROTOCOL_VERSION,
                "clientInfo": {
                    "name": "imece",
                    "title": "imece",
                    "version": env!("CARGO_PKG_VERSION"),
                },
                "clientCapabilities": {
                    "fs": { "readTextFile": false, "writeTextFile": false },
                    "terminal": false,
                },
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

    /// A prompt to the session: a command (`/about`) or a turn.
    fn say(&mut self, text: &str, wait: Wait) -> Act {
        self.text.clear();
        let params = json!({
            "sessionId": self.session,
            "prompt": [{ "type": "text", "text": text }],
        });
        self.request("session/prompt", params, wait)
    }

    /// Queues a prompt; it starts a turn as soon as the session is free.
    pub fn prompt(&mut self, text: &str) -> Vec<Act> {
        self.queue.push_back(Queued::Prompt(text.to_string()));
        self.next_turn()
    }

    /// Queues one of Gemini's own commands (`/clear`, `/compact`, `/stats`),
    /// sent as it is: a turn like any other.
    pub fn command(&mut self, line: &str) -> Vec<Act> {
        self.queue
            .push_back(Queued::Command(line.trim().to_string()));
        self.next_turn()
    }

    /// Starts the next queued turn, if the session is free. Once started,
    /// a session waits for nothing but its turns.
    fn next_turn(&mut self) -> Vec<Act> {
        if !self.started || self.busy {
            return Vec::new();
        }
        let Some(text) = self.queue.pop_front() else {
            return Vec::new();
        };
        self.busy = true;
        self.denied = 0;
        self.calls.clear();
        let text = match text {
            Queued::Prompt(t) => plain(&t),
            Queued::Command(line) => line,
        };
        vec![self.say(&text, Wait::Turn)]
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

    /// Gemini answered request `id`.
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
            if wait == Wait::Turn {
                return Ok(self.turn_failed(msg.to_string()));
            }
            return Err(format!("refused {wait:?}: {msg}"));
        }
        let r = &v["result"];
        Ok(match wait {
            Wait::Initialize => {
                if r["protocolVersion"].as_u64() != Some(PROTOCOL_VERSION) {
                    return Err(format!("speaks ACP {}", r["protocolVersion"]));
                }
                if r["agentInfo"]["name"] != "gemini-cli" {
                    return Err(format!("is {}, not gemini-cli", r["agentInfo"]["name"]));
                }
                let servers = match &self.mcp {
                    Some(m) => json!([{
                        "name": MCP_SERVER,
                        "command": m.command,
                        "args": m.args,
                        "env": [],
                    }]),
                    None => json!([]),
                };
                vec![self.request(
                    "session/new",
                    json!({ "cwd": self.home, "mcpServers": servers }),
                    Wait::Session,
                )]
            }
            Wait::Session => {
                if r["modes"]["currentModeId"] != "default" {
                    return Err(format!(
                        "started in mode {}, not default",
                        r["modes"]["currentModeId"]
                    ));
                }
                let want = self.model.as_deref().unwrap_or(AUTO);
                if r["models"]["currentModelId"] != want {
                    return Err(format!(
                        "started on model {}, not {want}",
                        r["models"]["currentModelId"]
                    ));
                }
                let id = r["sessionId"].as_str().unwrap_or_default();
                if id.is_empty() {
                    return Err("started a session without an id".into());
                }
                self.session = Some(id.to_string());
                vec![self.say("/about", Wait::About)]
            }
            Wait::About => {
                let about = std::mem::take(&mut self.text);
                let field = |name: &str| {
                    about
                        .lines()
                        .find_map(|l| l.trim().strip_prefix(&format!("- {name}:")))
                        .map(str::trim)
                };
                let want = self.model.as_deref().unwrap_or(AUTO);
                match field("Model") {
                    Some(m) if m == want => {}
                    Some(m) => return Err(format!("runs on model {m:?}, not {want:?}")),
                    None => return Err("did not say which model it runs on".into()),
                }
                self.pay = Some(match field("Auth Type") {
                    Some("gemini-api-key") => Pay::Priced,
                    Some("oauth-personal") => Pay::Plan,
                    Some(other) => {
                        return Err(format!(
                            "is logged in with {other:?}, whose cost imece can not count yet; \
                             use a Gemini API key or log in with Google"
                        ));
                    }
                    None => return Err("did not say how it is logged in".into()),
                });
                vec![self.say("/extensions list", Wait::Extensions)]
            }
            Wait::Extensions => {
                let text = std::mem::take(&mut self.text);
                let on = active_extensions(&text)?;
                if !on.is_empty() {
                    return Err(format!("has extensions on: {on:?}"));
                }
                vec![self.say("/memory list", Wait::Memory)]
            }
            Wait::Memory => {
                let text = std::mem::take(&mut self.text);
                let ours = self.home.join(INSTRUCTIONS);
                let real = ours.canonicalize().unwrap_or_else(|_| ours.clone());
                let listed = text.lines().map(str::trim).any(|l| {
                    let p = Path::new(l);
                    p == ours || p == real
                });
                if !listed {
                    return Err(format!(
                        "does not read imece's instructions ({}); the context file name may \
                         be changed in your Gemini settings",
                        ours.display()
                    ));
                }
                self.started = true;
                let mut acts = vec![
                    Act::Event(Event::Started {
                        session: self.session.clone().unwrap_or_default(),
                        model: self.model.clone().unwrap_or_else(|| AUTO.into()),
                    }),
                    Act::Event(Event::Note(match self.pay {
                        Some(Pay::Plan) => {
                            "gemini runs on your Google account: its turns cost the budget \
                             nothing"
                                .into()
                        }
                        _ => {
                            "gemini runs on your API key: its turns are booked at list price".into()
                        }
                    })),
                ];
                acts.extend(self.next_turn());
                acts
            }
            Wait::Turn => self.turn_done(r)?,
        })
    }

    /// A turn Gemini refused outright (a model it lacks, a rate limit).
    fn turn_failed(&mut self, why: String) -> Vec<Act> {
        self.busy = false;
        let mut acts = vec![Act::Event(Event::TurnDone {
            ok: false,
            text: why,
            cost: self.cost,
            denied: self.denied,
        })];
        acts.extend(self.next_turn());
        acts
    }

    /// A turn ended: its cost is booked from what it used.
    fn turn_done(&mut self, r: &Value) -> Result<Vec<Act>, String> {
        self.busy = false;
        let reason = r["stopReason"].as_str().unwrap_or("unknown");
        let u = &r["usage"];
        let n = |k: &str| u[k].as_u64().unwrap_or(0);
        let (input, cached) = (n("inputTokens"), n("cachedReadTokens"));
        let output = n("outputTokens") + n("thoughtTokens");
        self.tokens.0 += input;
        self.tokens.1 += cached;
        self.tokens.2 += output;
        if self.pay == Some(Pay::Priced) {
            // The models the turn ran on; `auto` may use several. Each
            // token is booked at the dearest of them: never too little.
            let models: Vec<&str> = r["_meta"]["quota"]["model_usage"]
                .as_array()
                .into_iter()
                .flatten()
                .filter_map(|m| m["model"].as_str())
                .collect();
            let mut dearest = 0;
            for m in &models {
                let c = cost(m, input, cached, output).ok_or_else(|| {
                    format!(
                        "ran on {m}, whose price imece does not know; name a model it knows \
                         with --model or --tier"
                    )
                })?;
                dearest = dearest.max(c);
            }
            if models.is_empty() && (input > 0 || output > 0) {
                return Err("used tokens without saying on which model".into());
            }
            self.cost += dearest;
        }
        let text = self.text.trim().to_string();
        let mut acts = Vec::new();
        if !text.is_empty() {
            acts.push(Act::Event(Event::Text(text.clone())));
        }
        acts.push(Act::Event(Event::TurnDone {
            ok: reason == "end_turn",
            text: if reason == "end_turn" {
                text
            } else {
                format!("stopped: {reason}")
            },
            cost: self.cost,
            denied: self.denied,
        }));
        let (i, c, o) = self.tokens;
        acts.push(Act::Event(Event::Note(format!(
            "tokens so far: {i} in ({c} cached) · {o} out"
        ))));
        acts.extend(self.next_turn());
        Ok(acts)
    }

    /// Gemini asked something of imece (a request with an id).
    fn asked(&mut self, id: Value, method: &str, params: &Value) -> Result<Vec<Act>, String> {
        match method {
            // Nothing may be asked: what would ask is denied.
            "session/request_permission" => {
                self.denied += 1;
                let title = params["toolCall"]["title"].as_str().unwrap_or_default();
                Ok(vec![
                    Act::Event(Event::Note(format!(
                        "refused: {}",
                        one_line(if title.is_empty() {
                            "a permission"
                        } else {
                            title
                        })
                    ))),
                    Act::Send(
                        json!({ "jsonrpc": "2.0", "id": id, "result": {
                            "outcome": { "outcome": "selected", "optionId": "cancel" } } })
                        .to_string(),
                    ),
                ])
            }
            _ => Ok(vec![Act::Send(
                json!({ "jsonrpc": "2.0", "id": id, "error": {
                    "code": -32601, "message": format!("imece does not answer {method}") } })
                .to_string(),
            )]),
        }
    }

    /// Gemini told something (a request without an id).
    fn notified(&mut self, method: &str, p: &Value) -> Result<Vec<Act>, String> {
        if method != "session/update" {
            return Ok(Vec::new());
        }
        let u = &p["update"];
        match u["sessionUpdate"].as_str().unwrap_or_default() {
            "agent_message_chunk" => {
                if let Some(t) = u["content"]["text"].as_str() {
                    self.text.push_str(t);
                }
                Ok(Vec::new())
            }
            "tool_call" => {
                if !self.started {
                    return Err("used a tool before its settings were checked".into());
                }
                let kind = u["kind"].as_str().unwrap_or("other");
                if NEVER.contains(&kind) || (!self.write && WRITES.contains(&kind)) {
                    return Err(format!("used a {kind} tool it was not given"));
                }
                let title = u["title"].as_str().unwrap_or_default();
                let id = u["toolCallId"].as_str().unwrap_or_default();
                let name = self.tool_name(id, kind, title)?;
                let target = u["locations"][0]["path"]
                    .as_str()
                    .map(|p| relative(p, &self.cwd))
                    .unwrap_or_else(|| {
                        if kind == "other" {
                            mcp_target(title)
                        } else {
                            one_line(title)
                        }
                    });
                if !id.is_empty() {
                    self.calls.insert(id.to_string(), name.clone());
                }
                Ok(vec![Act::Event(Event::Tool { name, target })])
            }
            "tool_call_update" => {
                if u["status"] != "failed" {
                    return Ok(Vec::new());
                }
                let id = u["toolCallId"].as_str().unwrap_or_default();
                let name = match self.calls.remove(id) {
                    Some(n) => n,
                    // Refused before it started: by the policy, or by a
                    // path outside the workspace.
                    None => {
                        self.denied += 1;
                        let kind = u["kind"].as_str().unwrap_or("other");
                        self.tool_name(id, kind, u["title"].as_str().unwrap_or_default())
                            .unwrap_or_else(|_| kind.to_string())
                    }
                };
                Ok(vec![Act::Event(Event::ToolFailed { name })])
            }
            "current_mode_update" => Err(format!("changed its mode to {}", u["currentModeId"])),
            // Gemini lists its own command line (and again when it
            // changes); the screen completes with it.
            "available_commands_update" => {
                let commands = u["availableCommands"]
                    .as_array()
                    .into_iter()
                    .flatten()
                    .filter_map(|c| c["name"].as_str())
                    .map(|n| format!("/{}", n.trim_start_matches('/')))
                    .collect();
                Ok(vec![Act::Event(Event::Commands(commands))])
            }
            // Thoughts, plans, usage meters: nothing the screen needs.
            _ => Ok(Vec::new()),
        }
    }

    /// The name a tool call gets on the screen: imece's own tools by their
    /// name, Gemini's tools by theirs. Gemini's call ids carry the tool's
    /// name (`write_file__call_7`); without one, the title and the kind
    /// have to do. A tool the role was not given stops the session.
    fn tool_name(&self, id: &str, kind: &str, title: &str) -> Result<String, String> {
        let called = id
            .split_once("__call_")
            .map(|(name, _)| name)
            .filter(|n| !n.is_empty())
            .map(str::to_string)
            .unwrap_or_else(|| {
                if kind == "other" {
                    gemini_name(title.split('(').next().unwrap_or_default().trim())
                } else {
                    String::new()
                }
            });
        if called.is_empty() {
            return Ok(kind.to_string());
        }
        if self.tools.contains(&called) {
            return Ok(mcp_tool(called.trim_start_matches(&prefix())));
        }
        let own = if self.write { EDIT_TOOLS } else { READ_TOOLS };
        if own.contains(&called.as_str()) {
            return Ok(called);
        }
        Err(format!(
            "used tool {} it was not given",
            one_line(if called.starts_with(&prefix()) {
                title
            } else {
                &called
            })
        ))
    }
}

/// How Gemini names a tool of imece's server to the model.
fn gemini_name(tool: &str) -> String {
    format!("{}{tool}", prefix())
}

fn prefix() -> String {
    format!("mcp_{MCP_SERVER}_")
}

/// A prompt Gemini would run as a command of its own (`/about`, `$x`) is
/// said to be text.
fn plain(text: &str) -> String {
    let t = text.trim_start();
    if t.starts_with('/') || t.starts_with('$') {
        format!("Text:\n{text}")
    } else {
        text.to_string()
    }
}

/// The extensions `/extensions list` says are on.
fn active_extensions(text: &str) -> Result<Vec<String>, String> {
    let t = text.trim();
    if t.is_empty() || t == "[]" {
        return Ok(Vec::new());
    }
    let v: Value = serde_json::from_str(t).map_err(|_| {
        format!(
            "listed its extensions in a way imece does not read: {}",
            one_line(t)
        )
    })?;
    Ok(v.as_array()
        .into_iter()
        .flatten()
        .filter(|e| e["isActive"] == true)
        .map(|e| e["name"].as_str().unwrap_or("?").to_string())
        .collect())
}

/// What an MCP tool's title says it was called with: the first argument.
fn mcp_target(title: &str) -> String {
    let inner = title
        .split_once('(')
        .map(|(_, rest)| rest.trim_end_matches(')'))
        .unwrap_or_default();
    let first = inner.split(',').next().unwrap_or_default();
    one_line(first.split_once(':').map_or(first, |(_, v)| v).trim())
}

/// Text in a few characters: its first line, cut short.
fn one_line(text: &str) -> String {
    let line = text.lines().next().unwrap_or_default().trim();
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
    Line(String),
    End(String),
    Prompt(String),
    Command(String),
    Terminate,
}

/// What waits for the session's next free moment.
#[derive(Clone, Debug, PartialEq, Eq)]
enum Queued {
    /// Text for the model, said to be text (see [`plain`]).
    Prompt(String),
    /// A line of Gemini's own command line, sent as it is.
    Command(String),
}

/// A running Gemini session.
pub struct Session {
    tx: Sender<Msg>,
}

fn spawn(spec: &Spec, tx: Sender<Msg>) -> io::Result<Child> {
    prepare(spec)?;
    let mut cmd = child::command(&spec.program, &spec.home, ENV);
    for (k, v) in FIXED_ENV {
        cmd.env(k, v);
    }
    cmd.args(args(spec));
    Child::spawn(cmd, move |out| {
        tx.send(match out {
            Out::Line(l) => Msg::Line(l.to_string()),
            Out::End(why) => Msg::End(why),
        })
        .is_ok()
    })
}

impl Session {
    /// Starts the session. `on` gets its events on the driver thread; the
    /// last one is [`Event::Stopped`], unless the session was terminated.
    pub fn start(spec: &Spec, on: impl FnMut(Event) + Send + 'static) -> io::Result<Session> {
        let (tx, rx) = mpsc::channel();
        let child = spawn(spec, tx.clone())?;
        let spec = spec.clone();
        thread::spawn(move || drive(spec, child, rx, on));
        Ok(Session { tx })
    }

    /// Gives the agent a prompt. A busy session takes it after the running
    /// turn.
    pub fn prompt(&mut self, text: &str) -> io::Result<()> {
        self.tx
            .send(Msg::Prompt(text.to_string()))
            .map_err(|_| io::Error::new(io::ErrorKind::BrokenPipe, "the session is over"))
    }

    /// Gives the session one of Gemini CLI's own commands, as the person
    /// typed it.
    pub fn command(&mut self, line: &str) -> io::Result<()> {
        if !line.trim().starts_with('/') {
            return Err(io::Error::new(
                io::ErrorKind::Unsupported,
                "a command of gemini's starts with /",
            ));
        }
        self.tx
            .send(Msg::Command(line.to_string()))
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
}

/// Why a session whose process refused a write is over: the process's
/// own end when it is reported within `END_WAIT` (a process that has
/// exited refuses input before its reader says so), else the write error.
fn refused(rx: &Receiver<Msg>, e: &io::Error) -> String {
    let deadline = Instant::now() + END_WAIT;
    loop {
        match rx.recv_timeout(deadline.saturating_duration_since(Instant::now())) {
            Ok(Msg::End(why)) => return why,
            Ok(_) => continue,
            Err(_) => return format!("stopped: gemini took no input ({e})"),
        }
    }
}

/// Runs the protocol against the Gemini process.
fn drive(spec: Spec, mut child: Child, rx: Receiver<Msg>, mut on: impl FnMut(Event)) {
    let mut protocol = Protocol::new(&spec);
    let mut acts = protocol.open();
    let mut deadline: Option<Instant> = None;
    loop {
        for act in acts.drain(..) {
            match act {
                Act::Event(e) => on(e),
                Act::Send(line) => {
                    if let Err(e) = child.write_line(&line) {
                        on(Event::Stopped(refused(&rx, &e)));
                        return;
                    }
                }
            }
        }
        deadline = protocol
            .waiting()
            .then(|| deadline.unwrap_or_else(|| Instant::now() + REQUEST_WAIT));
        match rx.recv_timeout(POLL) {
            Ok(Msg::Line(line)) => {
                deadline = None;
                match protocol.line(&line) {
                    Ok(a) => acts = a,
                    Err(why) => {
                        on(Event::Stopped(format!("stopped: the session {why}")));
                        return;
                    }
                }
            }
            Ok(Msg::End(why)) => {
                on(Event::Stopped(why));
                return;
            }
            Ok(Msg::Prompt(text)) => acts = protocol.prompt(&text),
            Ok(Msg::Command(line)) => acts = protocol.command(&line),
            Ok(Msg::Terminate) | Err(RecvTimeoutError::Disconnected) => return,
            Err(RecvTimeoutError::Timeout) => {
                if deadline.is_some_and(|d| Instant::now() >= d) {
                    on(Event::Stopped(
                        "stopped: gemini did not answer in time".to_string(),
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
            program: "gemini".into(),
            cwd: PathBuf::from("/work/w1"),
            home: PathBuf::from("/state/gemini/gemini-1"),
            model: None,
            write,
            system: "You are gemini-1.".into(),
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
    fn the_tiers_are_gemini_s_own_aliases() {
        assert_eq!(model(crate::Tier::Cheap), Some("flash"));
        assert_eq!(model(crate::Tier::Standard), None);
        assert_eq!(model(crate::Tier::Strong), Some("pro"));
        assert_eq!(PROGRAM, "gemini");
    }

    #[test]
    fn prices_match_the_longest_known_prefix_and_cost_rounds_up() {
        assert_eq!(price("gemini-2.5-pro"), Some((1250, 10000, 125)));
        assert_eq!(
            price("gemini-3.1-pro-preview-customtools"),
            Some((2000, 12000, 200))
        );
        assert_eq!(price("gemini-2.5-flash-lite"), Some((100, 400, 10)));
        assert_eq!(price("gemini-2.5-flash"), Some((300, 2500, 30)));
        assert_eq!(price("gemini-3-pro-preview"), None);
        assert_eq!(price("auto"), None);
        assert_eq!(price(""), None);
        // 600 fresh input at 1.25, 400 cached at 0.125, 100 output at 10.
        assert_eq!(cost("gemini-2.5-pro", 1000, 400, 100), Some(1_800));
        assert_eq!(cost("gemini-2.5-flash-lite", 1, 0, 0), Some(1));
        assert_eq!(cost("gemini-2.5-flash-lite", 0, 0, 0), Some(0));
        // Cached tokens beyond the input never go negative.
        assert_eq!(cost("gemini-2.5-flash-lite", 10, 20, 0), Some(1));
        assert_eq!(cost("gemini-9-ultra", 1, 0, 0), None);
    }

    #[test]
    fn the_policy_allows_the_role_s_tools_by_name_and_denies_the_rest() {
        let p = policy(&with_bus(false));
        let allow = p.find("decision = \"allow\"").unwrap();
        let deny = p.find("decision = \"deny\"").unwrap();
        assert!(allow < deny, "the allows come first:\n{p}");
        assert!(p.contains("toolName = [\"read_file\", \"list_directory\", \"glob\", \"grep_search\"]\ndecision = \"allow\"\npriority = 999"), "{p}");
        assert!(
            p.contains("toolName = \"*\"\ndecision = \"deny\"\npriority = 998"),
            "{p}"
        );
        assert!(p.contains("toolName = [\"mcp_imece_send\", \"mcp_imece_post\"]\ndecision = \"allow\"\npriority = 999"), "{p}");
        assert!(!p.contains("write_file"));
        let w = policy(&with_bus(true));
        assert!(w.contains("\"write_file\", \"replace\"]"), "{w}");
        let alone = policy(&spec(true));
        assert!(
            !alone.contains("mcp_imece"),
            "no bus, no MCP rule:\n{alone}"
        );
        // The allow rules' names are the ones the tools have on the wire.
        for t in EDIT_TOOLS {
            assert!(w.contains(&format!("\"{t}\"")), "{t}");
        }
    }

    #[test]
    fn toml_strings_are_quoted_as_toml_wants() {
        assert_eq!(toml_str(r#"a"b\c"#), r#""a\"b\\c""#);
        assert_eq!(toml_str("tab\there"), r#""tab\u0009here""#);
    }

    #[test]
    fn the_instructions_carry_the_role_and_where_the_files_are() {
        let i = instructions(&spec(true));
        assert!(i.starts_with("You are gemini-1.\n\n"), "{i}");
        assert!(i.contains("/work/w1"), "{i}");
    }

    fn after<'a>(a: &'a [String], key: &str) -> Option<&'a str> {
        a.iter()
            .position(|x| x == key)
            .and_then(|i| a.get(i + 1))
            .map(String::as_str)
    }

    #[test]
    fn the_command_line_locks_the_session_down_and_carries_no_prompt() {
        let a = args(&with_bus(true));
        assert_eq!(a[0], "--acp");
        assert_eq!(after(&a, "--policy"), Some("/state/gemini/gemini-1/policy"));
        assert_eq!(after(&a, "--approval-mode"), Some("default"));
        assert_eq!(after(&a, "--include-directories"), Some("/work/w1"));
        assert_eq!(after(&a, "-e"), Some("none"));
        assert_eq!(after(&a, "--allowed-mcp-server-names"), Some("imece"));
        assert_eq!(after(&a, "-m"), None);
        assert!(
            !a.iter()
                .any(|x| x.contains("You are") || x.contains("yolo"))
        );
        let mut s = spec(false);
        s.model = Some("pro".into());
        assert_eq!(after(&args(&s), "-m"), Some("pro"));
        // The MCP server goes to the session, never to argv.
        assert!(!a.iter().any(|x| x.contains("/opt/imece")));
        for (k, v) in FIXED_ENV {
            assert!(!k.is_empty() && !v.is_empty());
        }
        assert!(FIXED_ENV.contains(&("GEMINI_CLI_TRUST_WORKSPACE", "true")));
    }

    #[test]
    fn preparing_writes_the_policy_and_the_instructions_in_a_private_directory() {
        use std::os::unix::fs::PermissionsExt;
        let dir = std::env::temp_dir().join(format!("imece-gemini-prep-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let mut s = with_bus(true);
        s.home = dir.join("gemini-1");
        prepare(&s).unwrap();
        let mode = std::fs::metadata(&s.home).unwrap().permissions().mode() & 0o777;
        assert_eq!(mode, 0o700);
        assert_eq!(
            std::fs::read_to_string(s.home.join("policy/imece.toml")).unwrap(),
            policy(&s)
        );
        assert_eq!(
            std::fs::read_to_string(s.home.join("GEMINI.md")).unwrap(),
            instructions(&s)
        );
        // A second start rewrites both for the new role.
        s.write = false;
        prepare(&s).unwrap();
        assert!(
            !std::fs::read_to_string(s.home.join("policy/imece.toml"))
                .unwrap()
                .contains("write_file")
        );
        let _ = std::fs::remove_dir_all(&dir);
    }

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

    fn chunk(text: &str) -> String {
        json!({ "jsonrpc": "2.0", "method": "session/update", "params": {
            "sessionId": "s1",
            "update": { "sessionUpdate": "agent_message_chunk", "content": { "type": "text", "text": text } },
        }})
        .to_string()
    }

    fn update(u: Value) -> String {
        json!({ "jsonrpc": "2.0", "method": "session/update", "params": {
            "sessionId": "s1", "update": u } })
        .to_string()
    }

    fn command_done() -> Value {
        json!({ "stopReason": "end_turn", "usage": { "inputTokens": 0, "outputTokens": 0, "totalTokens": 0 } })
    }

    fn init() -> Value {
        json!({ "protocolVersion": 1, "agentInfo": { "name": "gemini-cli", "version": "0.65.0" } })
    }

    fn session(model: &str) -> Value {
        json!({
            "sessionId": "s1",
            "modes": { "currentModeId": "default" },
            "models": { "currentModelId": model },
        })
    }

    fn about(model: &str, auth: &str) -> String {
        format!(
            "Gemini CLI Info:\n- Version: 0.65.0\n- OS: darwin\n- Model: {model}\n- Auth Type: {auth}\n- Tier: Unknown"
        )
    }

    fn memory(spec: &Spec) -> String {
        format!(
            "There are 2 GEMINI.md file(s) in use:\n\n/home/me/.gemini/GEMINI.md\n{}",
            spec.home.join("GEMINI.md").display()
        )
    }

    /// Walks the protocol through the start, answering as a clean Gemini
    /// would, up to the started session.
    fn started(spec: &Spec) -> Protocol {
        let mut p = Protocol::new(spec);
        let open = p.open();
        let init_req = &sent(&open)[0];
        assert_eq!(init_req["method"], "initialize");
        assert_eq!(init_req["id"], 1);
        assert_eq!(init_req["params"]["protocolVersion"], 1);
        assert_eq!(
            init_req["params"]["clientCapabilities"]["fs"]["readTextFile"],
            false
        );
        assert_eq!(
            init_req["params"]["clientCapabilities"]["fs"]["writeTextFile"],
            false
        );
        assert!(p.waiting());

        let acts = p.line(&reply(1, init())).unwrap();
        let new = &sent(&acts)[0];
        assert_eq!(new["method"], "session/new");
        assert_eq!(new["params"]["cwd"], json!(spec.home));
        match &spec.mcp {
            Some(m) => {
                let s = &new["params"]["mcpServers"][0];
                assert_eq!(s["name"], "imece");
                assert_eq!(s["command"], m.command);
                assert_eq!(s["args"], json!(m.args));
                assert_eq!(new["params"]["mcpServers"].as_array().unwrap().len(), 1);
            }
            None => assert_eq!(new["params"]["mcpServers"], json!([])),
        }

        let want = spec.model.clone().unwrap_or_else(|| "auto".into());
        let acts = p.line(&reply(2, session(&want))).unwrap();
        let q = &sent(&acts)[0];
        assert_eq!(q["method"], "session/prompt");
        assert_eq!(q["params"]["sessionId"], "s1");
        assert_eq!(q["params"]["prompt"][0]["text"], "/about");
        assert!(p.waiting(), "a command is answered soon");

        assert!(
            p.line(&chunk(&about(&want, "gemini-api-key")))
                .unwrap()
                .is_empty()
        );
        let acts = p.line(&reply(3, command_done())).unwrap();
        assert_eq!(
            sent(&acts)[0]["params"]["prompt"][0]["text"],
            "/extensions list"
        );
        assert!(p.line(&chunk("[]")).unwrap().is_empty());
        let acts = p.line(&reply(4, command_done())).unwrap();
        assert_eq!(
            sent(&acts)[0]["params"]["prompt"][0]["text"],
            "/memory list"
        );
        assert!(p.line(&chunk(&memory(spec))).unwrap().is_empty());
        let acts = p.line(&reply(5, command_done())).unwrap();
        assert_eq!(
            events(&acts),
            [
                Event::Started {
                    session: "s1".into(),
                    model: want
                },
                Event::Note(
                    "gemini runs on your API key: its turns are booked at list price".into()
                ),
            ]
        );
        assert!(
            sent(&acts).is_empty(),
            "nothing to say until a prompt comes"
        );
        assert!(!p.waiting());
        p
    }

    #[test]
    fn a_clean_gemini_passes_the_checks_and_the_session_starts() {
        started(&spec(false));
        started(&with_bus(true));
        let mut s = with_bus(false);
        s.model = Some("pro".into());
        started(&s);
    }

    /// Answers the start up to `upto` (the step whose answer `result`
    /// stands in for; a command's text is `text`), returning what the
    /// protocol made of it.
    fn start_until(spec: &Spec, upto: &str, text: &str, result: Value) -> Result<Vec<Act>, String> {
        let mut p = Protocol::new(spec);
        p.open();
        let want = spec.model.clone().unwrap_or_else(|| "auto".into());
        let steps: Vec<(&str, String, Value)> = vec![
            ("initialize", String::new(), init()),
            ("session/new", String::new(), session(&want)),
            ("/about", about(&want, "gemini-api-key"), command_done()),
            ("/extensions list", "[]".into(), command_done()),
            ("/memory list", memory(spec), command_done()),
        ];
        let mut id = 0;
        for (step, good_text, good) in steps {
            id += 1;
            let (t, r) = if step == upto {
                (text.to_string(), result.clone())
            } else {
                (good_text, good)
            };
            if !t.is_empty() {
                p.line(&chunk(&t))?;
            }
            let acts = p.line(&reply(id, r))?;
            if step == upto {
                return Ok(acts);
            }
        }
        unreachable!("{upto} is not a start step");
    }

    #[test]
    fn what_gemini_reports_must_be_what_was_asked_for() {
        let s = spec(false);
        let e = start_until(
            &s,
            "initialize",
            "",
            json!({ "protocolVersion": 2, "agentInfo": { "name": "gemini-cli" } }),
        )
        .unwrap_err();
        assert!(e.contains("speaks ACP 2"), "{e}");
        let e = start_until(
            &s,
            "initialize",
            "",
            json!({ "protocolVersion": 1, "agentInfo": { "name": "other" } }),
        )
        .unwrap_err();
        assert!(e.contains("not gemini-cli"), "{e}");
        let mut bad = session("auto");
        bad["modes"]["currentModeId"] = json!("yolo");
        let e = start_until(&s, "session/new", "", bad).unwrap_err();
        assert!(e.contains("mode \"yolo\""), "{e}");
        let e = start_until(&s, "session/new", "", session("gemini-2.5-pro")).unwrap_err();
        assert!(e.contains("model \"gemini-2.5-pro\", not auto"), "{e}");
        let mut bad = session("auto");
        bad["sessionId"] = json!("");
        let e = start_until(&s, "session/new", "", bad).unwrap_err();
        assert!(e.contains("without an id"), "{e}");
        let e = start_until(
            &s,
            "/about",
            &about("gemini-2.5-pro", "gemini-api-key"),
            command_done(),
        )
        .unwrap_err();
        assert!(
            e.contains("runs on model \"gemini-2.5-pro\", not \"auto\""),
            "{e}"
        );
        let e = start_until(&s, "/about", "nothing useful", command_done()).unwrap_err();
        assert!(e.contains("which model"), "{e}");
        let e = start_until(&s, "/about", &about("auto", "vertex-ai"), command_done()).unwrap_err();
        assert!(
            e.contains("\"vertex-ai\"") && e.contains("can not count"),
            "{e}"
        );
        let e = start_until(&s, "/about", "- Model: auto\n", command_done()).unwrap_err();
        assert!(e.contains("how it is logged in"), "{e}");
        let on = r#"[{"name": "grafana", "isActive": true}, {"name": "redis", "isActive": false}]"#;
        let e = start_until(&s, "/extensions list", on, command_done()).unwrap_err();
        assert!(e.contains("extensions on: [\"grafana\"]"), "{e}");
        let e = start_until(&s, "/extensions list", "not a list", command_done()).unwrap_err();
        assert!(e.contains("does not read"), "{e}");
        let e = start_until(
            &s,
            "/memory list",
            "There are 1 GEMINI.md file(s) in use:\n\n/home/me/.gemini/GEMINI.md",
            command_done(),
        )
        .unwrap_err();
        assert!(
            e.contains("instructions") && e.contains("/state/gemini/gemini-1/GEMINI.md"),
            "{e}"
        );
        // A refused request stops the session, except a turn's.
        let p = Protocol::new(&s);
        let mut p = p;
        p.open();
        let e = p
            .line(&json!({ "jsonrpc": "2.0", "id": 1, "error": { "code": -32000, "message": "Authentication required." } }).to_string())
            .unwrap_err();
        assert!(
            e.contains("refused Initialize: Authentication required."),
            "{e}"
        );
    }

    #[test]
    fn a_google_login_books_nothing_and_says_so() {
        let s = spec(false);
        let acts = start_until(
            &s,
            "/about",
            &about("auto", "oauth-personal"),
            command_done(),
        )
        .unwrap();
        assert_eq!(
            sent(&acts)[0]["params"]["prompt"][0]["text"],
            "/extensions list"
        );
        let mut p = Protocol::new(&s);
        p.open();
        p.line(&reply(1, init())).unwrap();
        p.line(&reply(2, session("auto"))).unwrap();
        p.line(&chunk(&about("auto", "oauth-personal"))).unwrap();
        p.line(&reply(3, command_done())).unwrap();
        p.line(&chunk("[]")).unwrap();
        p.line(&reply(4, command_done())).unwrap();
        p.line(&chunk(&memory(&s))).unwrap();
        let acts = p.line(&reply(5, command_done())).unwrap();
        assert!(matches!(&events(&acts)[1], Event::Note(n) if n.contains("Google account")));
        p.prompt("hi");
        p.line(&chunk("pong")).unwrap();
        let acts = p
            .line(&reply(6, turn_result("gemini-2.5-pro", 1000, 0, 10, 0)))
            .unwrap();
        assert!(
            events(&acts).contains(&Event::TurnDone {
                ok: true,
                text: "pong".into(),
                cost: 0,
                denied: 0
            }),
            "{acts:?}"
        );
    }

    #[test]
    fn answers_must_match_the_question_and_nothing_counts_before_the_checks() {
        let s = with_bus(true);
        let mut p = Protocol::new(&s);
        let e = p.line(&reply(1, init())).unwrap_err();
        assert!(e.contains("nobody made"), "{e}");
        p.open();
        let e = p.line(&reply(7, init())).unwrap_err();
        assert!(e.contains("answered request 7, not 1"), "{e}");
        let mut p = Protocol::new(&s);
        p.open();
        let e = p.line("not json").unwrap_err();
        assert!(e.contains("no JSON-RPC"), "{e}");
        let e = p.line(r#"{"jsonrpc":"2.0"}"#).unwrap_err();
        assert!(e.contains("neither method nor id"), "{e}");
        assert!(p.line("   ").unwrap().is_empty());
        let e = p
            .line(&update(json!({ "sessionUpdate": "tool_call", "toolCallId": "c1", "title": "x", "kind": "read" })))
            .unwrap_err();
        assert!(e.contains("before its settings were checked"), "{e}");
        // Prompts wait for the start, before and during it.
        assert!(p.prompt("hi").is_empty());
        let mut p = Protocol::new(&s);
        assert!(
            p.prompt("hi").is_empty(),
            "nothing is said before initialize"
        );
    }

    #[test]
    fn the_instructions_file_may_be_listed_by_its_real_path() {
        // Gemini lists GEMINI.md files by their canonical path, which may
        // differ from the one imece gave it (a symlinked state directory).
        let dir = std::env::temp_dir().join(format!("imece-gemini-real-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(dir.join("real")).unwrap();
        std::os::unix::fs::symlink(dir.join("real"), dir.join("link")).unwrap();
        let mut s = spec(false);
        s.home = dir.join("link").join("gemini-1");
        prepare(&s).unwrap();
        let real = dir
            .join("real")
            .join("gemini-1")
            .join("GEMINI.md")
            .canonicalize()
            .unwrap();
        assert_ne!(real, s.home.join("GEMINI.md"));
        let listing = format!(
            "There are 1 GEMINI.md file(s) in use:\n\n{}",
            real.display()
        );
        let acts = start_until(&s, "/memory list", &listing, command_done()).unwrap();
        assert!(
            matches!(&events(&acts)[0], Event::Started { .. }),
            "{acts:?}"
        );
        let _ = std::fs::remove_dir_all(&dir);
    }

    fn turn_result(model: &str, input: u64, cached: u64, output: u64, thought: u64) -> Value {
        json!({
            "stopReason": "end_turn",
            "usage": {
                "inputTokens": input, "outputTokens": output, "cachedReadTokens": cached,
                "thoughtTokens": thought, "totalTokens": input + output,
            },
            "_meta": { "quota": { "model_usage": [{ "model": model, "token_count": {} }] } },
        })
    }

    #[test]
    fn a_prompt_starts_a_turn_the_next_waits_and_the_turn_is_booked() {
        let s = with_bus(true);
        let mut p = started(&s);
        let acts = p.prompt("hello");
        let q = &sent(&acts)[0];
        assert_eq!(q["method"], "session/prompt");
        assert_eq!(q["id"], 6);
        assert_eq!(
            q["params"]["prompt"],
            json!([{ "type": "text", "text": "hello" }])
        );
        assert!(!p.waiting(), "a turn takes as long as it takes");
        assert!(p.prompt("later").is_empty(), "the session is busy");
        p.line(&chunk("po")).unwrap();
        p.line(&chunk("ng")).unwrap();
        // Thoughts and meters are not the screen's business.
        assert!(p.line(&update(json!({ "sessionUpdate": "agent_thought_chunk", "content": { "type": "text", "text": "hm" } }))).unwrap().is_empty());
        // The command list is: the screen completes with it.
        let acts = p
            .line(&update(json!({ "sessionUpdate": "available_commands_update", "availableCommands": [
                { "name": "clear", "description": "x" }, { "name": "/stats", "description": "y", "input": { "hint": "z" } }
            ] })))
            .unwrap();
        assert_eq!(
            events(&acts),
            [Event::Commands(vec!["/clear".into(), "/stats".into()])]
        );
        let acts = p
            .line(&update(
                json!({ "sessionUpdate": "available_commands_update", "availableCommands": [] }),
            ))
            .unwrap();
        assert_eq!(events(&acts), [Event::Commands(vec![])]);
        assert!(
            p.line(&update(
                json!({ "sessionUpdate": "usage_update", "used": 5, "size": 9 })
            ))
            .unwrap()
            .is_empty()
        );
        assert!(
            p.line(
                &json!({ "jsonrpc": "2.0", "method": "something/else", "params": {} }).to_string()
            )
            .unwrap()
            .is_empty()
        );
        let acts = p
            .line(&reply(6, turn_result("gemini-2.5-pro", 1000, 400, 50, 50)))
            .unwrap();
        assert_eq!(
            events(&acts),
            [
                Event::Text("pong".into()),
                Event::TurnDone {
                    ok: true,
                    text: "pong".into(),
                    cost: 1_800,
                    denied: 0
                },
                Event::Note("tokens so far: 1000 in (400 cached) · 100 out".into()),
            ],
            "{acts:?}"
        );
        // The waiting prompt goes out now, and the cost keeps adding up.
        let q = &sent(&acts)[0];
        assert_eq!(q["params"]["prompt"][0]["text"], "later");
        assert_eq!(q["id"], 7);
        let acts = p
            .line(&reply(7, turn_result("gemini-2.5-pro", 1000, 400, 50, 50)))
            .unwrap();
        assert!(
            events(&acts).contains(&Event::TurnDone {
                ok: true,
                text: String::new(),
                cost: 3_600,
                denied: 0
            }),
            "{acts:?}"
        );
        assert!(
            !events(&acts).iter().any(|e| matches!(e, Event::Text(_))),
            "no text, no Text event"
        );
    }

    #[test]
    fn a_turn_on_several_models_is_booked_at_the_dearest_and_an_unknown_one_stops_it() {
        let s = spec(false);
        let mut p = started(&s);
        p.prompt("hello");
        let mut r = turn_result("gemini-2.5-flash-lite", 1000, 0, 100, 0);
        r["_meta"]["quota"]["model_usage"] = json!([
            { "model": "gemini-2.5-flash-lite", "token_count": {} },
            { "model": "gemini-2.5-pro", "token_count": {} },
        ]);
        let acts = p.line(&reply(6, r)).unwrap();
        // 1000 at 1.25 plus 100 at 10, the pro price, not the lite one.
        assert!(
            events(&acts).contains(&Event::TurnDone {
                ok: true,
                text: String::new(),
                cost: 2_250,
                denied: 0
            }),
            "{acts:?}"
        );
        p.prompt("again");
        let e = p
            .line(&reply(7, turn_result("gemini-9-ultra", 10, 0, 1, 0)))
            .unwrap_err();
        assert!(e.contains("gemini-9-ultra") && e.contains("--tier"), "{e}");
        for (input, output) in [(10, 1), (1, 0), (0, 1)] {
            let mut p = started(&s);
            p.prompt("hello");
            let mut r = turn_result("x", input, 0, output, 0);
            r["_meta"] = json!({});
            let e = p.line(&reply(6, r)).unwrap_err();
            assert!(e.contains("without saying on which model"), "{e}");
        }
        // No tokens, no model needed: a command-like turn.
        let mut p = started(&s);
        p.prompt("hello");
        let mut r = turn_result("x", 0, 0, 0, 0);
        r["_meta"] = json!({});
        assert!(p.line(&reply(6, r)).is_ok());
    }

    #[test]
    fn a_turn_that_fails_or_stops_early_says_so_and_the_session_goes_on() {
        let s = spec(false);
        let mut p = started(&s);
        p.prompt("hello");
        p.prompt("then");
        let acts = p
            .line(&json!({ "jsonrpc": "2.0", "id": 6, "error": { "code": 500, "message": "This model is no longer available" } }).to_string())
            .unwrap();
        assert_eq!(
            events(&acts),
            [Event::TurnDone {
                ok: false,
                text: "This model is no longer available".into(),
                cost: 0,
                denied: 0
            }]
        );
        assert_eq!(sent(&acts)[0]["params"]["prompt"][0]["text"], "then");
        let mut r = turn_result("gemini-2.5-pro", 10, 0, 1, 0);
        r["stopReason"] = json!("max_turn_requests");
        let acts = p.line(&reply(7, r)).unwrap();
        assert!(
            events(&acts).contains(&Event::TurnDone {
                ok: false,
                text: "stopped: max_turn_requests".into(),
                cost: 23,
                denied: 0
            }),
            "{acts:?}"
        );
    }

    #[test]
    fn a_command_of_gemini_s_own_goes_as_it_is_and_runs_as_a_turn() {
        let s = with_bus(true);
        let mut p = started(&s);
        let acts = p.command(" /clear ");
        let q = &sent(&acts)[0];
        assert_eq!(q["method"], "session/prompt");
        assert_eq!(q["params"]["prompt"][0]["text"], "/clear");
        assert!(p.prompt("/stats").is_empty(), "the session is busy");
        let acts = p
            .line(&reply(6, turn_result("gemini-2.5-pro", 10, 0, 1, 0)))
            .unwrap();
        assert!(
            events(&acts)
                .iter()
                .any(|e| matches!(e, Event::TurnDone { ok: true, .. })),
            "{acts:?}"
        );
        // The prompt that waited is text, not a command.
        assert_eq!(
            sent(&acts)[0]["params"]["prompt"][0]["text"],
            "Text:\n/stats"
        );
    }

    #[test]
    fn a_session_takes_a_command_as_typed_and_refuses_what_is_not_one() {
        let (tx, rx) = mpsc::channel();
        // Through the trait, as the screen drives every vendor.
        let mut s: Box<dyn crate::Agent> = Box::new(Session { tx });
        s.command(" /clear ").unwrap();
        assert!(matches!(rx.try_recv(), Ok(Msg::Command(l)) if l == " /clear "));
        for line in ["clear", "", "fix a/b"] {
            let e = s.command(line).unwrap_err();
            assert_eq!(e.kind(), io::ErrorKind::Unsupported, "{line:?}");
        }
        assert!(rx.try_recv().is_err(), "a refused command sends nothing");
        drop(rx);
        let e = s.command("/stats").unwrap_err();
        assert_eq!(e.kind(), io::ErrorKind::BrokenPipe);
    }

    #[test]
    fn a_prompt_gemini_would_run_as_a_command_is_said_to_be_text() {
        assert_eq!(plain("/about"), "Text:\n/about");
        assert_eq!(plain("  $x"), "Text:\n  $x");
        assert_eq!(plain("hello /about"), "hello /about");
        assert_eq!(plain(""), "");
        let mut p = started(&spec(false));
        let acts = p.prompt("/help");
        assert_eq!(
            sent(&acts)[0]["params"]["prompt"][0]["text"],
            "Text:\n/help"
        );
    }

    fn tool_call(id: &str, kind: &str, title: &str, path: Option<&str>) -> String {
        let mut u = json!({ "sessionUpdate": "tool_call", "toolCallId": id, "title": title, "kind": kind, "status": "in_progress" });
        if let Some(p) = path {
            u["locations"] = json!([{ "path": p }]);
        }
        update(u)
    }

    #[test]
    fn tools_failures_and_refusals_become_events() {
        let s = with_bus(true);
        let mut p = started(&s);
        p.prompt("go");
        let acts = p
            .line(&tool_call(
                "c1",
                "read",
                "hello.txt",
                Some("/work/w1/src/hello.txt"),
            ))
            .unwrap();
        assert_eq!(
            events(&acts),
            [Event::Tool {
                name: "read".into(),
                target: "src/hello.txt".into()
            }]
        );
        let acts = p
            .line(&tool_call(
                "c2",
                "edit",
                "note.txt",
                Some("/elsewhere/note.txt"),
            ))
            .unwrap();
        assert_eq!(
            events(&acts),
            [Event::Tool {
                name: "edit".into(),
                target: "/elsewhere/note.txt".into()
            }]
        );
        let acts = p.line(&tool_call("c3", "search", "*.rs", None)).unwrap();
        assert_eq!(
            events(&acts),
            [Event::Tool {
                name: "search".into(),
                target: "*.rs".into()
            }]
        );
        let acts = p
            .line(&tool_call(
                "c4",
                "other",
                "send(to: lead, type: inform, body: pong)",
                None,
            ))
            .unwrap();
        assert_eq!(
            events(&acts),
            [Event::Tool {
                name: "mcp__imece__send".into(),
                target: "lead".into()
            }]
        );
        let acts = p.line(&tool_call("c5", "other", "post", None)).unwrap();
        assert_eq!(
            events(&acts),
            [Event::Tool {
                name: "mcp__imece__post".into(),
                target: String::new()
            }]
        );
        // A tool that ran and failed.
        let acts = p.line(&update(json!({ "sessionUpdate": "tool_call_update", "toolCallId": "c4", "status": "failed", "kind": "other" }))).unwrap();
        assert_eq!(
            events(&acts),
            [Event::ToolFailed {
                name: "mcp__imece__send".into()
            }]
        );
        assert!(p.line(&update(json!({ "sessionUpdate": "tool_call_update", "toolCallId": "c1", "status": "completed" }))).unwrap().is_empty());
        // One denied by the policy before it started: nobody saw it start.
        let acts = p.line(&update(json!({ "sessionUpdate": "tool_call_update", "toolCallId": "c9", "status": "failed", "kind": "execute", "title": "ls" }))).unwrap();
        assert_eq!(
            events(&acts),
            [Event::ToolFailed {
                name: "execute".into()
            }]
        );
        // A permission request is refused and counted.
        let acts = p
            .line(&json!({ "jsonrpc": "2.0", "id": "r1", "method": "session/request_permission", "params": {
                "sessionId": "s1", "options": [{ "optionId": "proceed_once", "kind": "allow_once" }, { "optionId": "cancel", "kind": "reject_once" }],
                "toolCall": { "toolCallId": "c7", "title": "rm -rf /" } } }).to_string())
            .unwrap();
        assert_eq!(events(&acts), [Event::Note("refused: rm -rf /".into())]);
        let answer = &sent(&acts)[0];
        assert_eq!(answer["id"], "r1");
        assert_eq!(
            answer["result"]["outcome"],
            json!({ "outcome": "selected", "optionId": "cancel" })
        );
        // Other questions are not answered, politely.
        let acts = p
            .line(
                &json!({ "jsonrpc": "2.0", "id": 3, "method": "fs/read_text_file", "params": {} })
                    .to_string(),
            )
            .unwrap();
        assert_eq!(sent(&acts)[0]["error"]["code"], -32601);
        let acts = p
            .line(&reply(6, turn_result("gemini-2.5-pro", 10, 0, 1, 0)))
            .unwrap();
        assert!(
            events(&acts).contains(&Event::TurnDone {
                ok: true,
                text: String::new(),
                cost: 23,
                denied: 2
            }),
            "{acts:?}"
        );
        // The next turn starts its count afresh.
        let acts = p.prompt("more");
        assert_eq!(sent(&acts).len(), 1);
        let acts = p
            .line(&reply(7, turn_result("gemini-2.5-pro", 10, 0, 1, 0)))
            .unwrap();
        assert!(
            events(&acts).contains(&Event::TurnDone {
                ok: true,
                text: String::new(),
                cost: 46,
                denied: 0
            }),
            "{acts:?}"
        );
    }

    #[test]
    fn gemini_s_call_ids_name_the_tools() {
        let s = with_bus(true);
        let mut p = started(&s);
        p.prompt("go");
        let acts = p
            .line(&tool_call(
                "write_file__call_1",
                "edit",
                "note.txt",
                Some("/work/w1/note.txt"),
            ))
            .unwrap();
        assert_eq!(
            events(&acts),
            [Event::Tool {
                name: "write_file".into(),
                target: "note.txt".into()
            }]
        );
        let acts = p
            .line(&tool_call(
                "read_file__call_2",
                "read",
                "../.../w1/a.rs",
                Some("/work/w1/a.rs"),
            ))
            .unwrap();
        assert_eq!(
            events(&acts),
            [Event::Tool {
                name: "read_file".into(),
                target: "a.rs".into()
            }]
        );
        let acts = p
            .line(&tool_call(
                "mcp_imece_send__call_3",
                "other",
                "send(to: lead, type: inform, body: pong)",
                None,
            ))
            .unwrap();
        assert_eq!(
            events(&acts),
            [Event::Tool {
                name: "mcp__imece__send".into(),
                target: "lead".into()
            }]
        );
        // A write refused for its path fails before anyone saw it start.
        let acts = p.line(&update(json!({ "sessionUpdate": "tool_call_update", "toolCallId": "write_file__call_9", "status": "failed",
            "content": [{ "type": "content", "content": { "type": "text", "text": "Path not in workspace" } }] }))).unwrap();
        assert_eq!(
            events(&acts),
            [Event::ToolFailed {
                name: "write_file".into()
            }]
        );
        let e = p
            .line(&tool_call(
                "save_memory__call_5",
                "other",
                "save_memory(fact: x)",
                None,
            ))
            .unwrap_err();
        assert!(e.contains("used tool save_memory it was not given"), "{e}");
        let mut p = started(&with_bus(false));
        p.prompt("go");
        let e = p
            .line(&tool_call("write_file__call_1", "other", "note.txt", None))
            .unwrap_err();
        assert!(
            e.contains("used tool write_file it was not given"),
            "a leader: {e}"
        );
        let acts = p
            .line(&reply(6, turn_result("gemini-2.5-pro", 10, 0, 1, 0)))
            .unwrap_or_default();
        let _ = acts;
    }

    #[test]
    fn what_the_agent_was_not_given_stops_the_session() {
        let s = with_bus(true);
        for (kind, title) in [
            ("execute", "ls"),
            ("fetch", "https://x"),
            ("switch_mode", "plan"),
        ] {
            let mut p = started(&s);
            p.prompt("go");
            let e = p.line(&tool_call("c1", kind, title, None)).unwrap_err();
            assert!(e.contains(&format!("used a {kind} tool")), "{e}");
        }
        for kind in ["edit", "delete", "move"] {
            let mut p = started(&with_bus(false));
            p.prompt("go");
            let e = p.line(&tool_call("c1", kind, "x", None)).unwrap_err();
            assert!(e.contains(&format!("used a {kind} tool")), "a leader: {e}");
        }
        let mut p = started(&s);
        p.prompt("go");
        let e = p
            .line(&tool_call("c1", "other", "hire(tier: cheap)", None))
            .unwrap_err();
        assert!(
            e.contains("used tool hire(tier: cheap) it was not given"),
            "{e}"
        );
        let mut p = started(&spec(false));
        p.prompt("go");
        let e = p
            .line(&tool_call("c1", "other", "send(to: lead)", None))
            .unwrap_err();
        assert!(e.contains("not given"), "no bus: {e}");
        let mut p = started(&s);
        p.prompt("go");
        let e = p
            .line(&update(
                json!({ "sessionUpdate": "current_mode_update", "currentModeId": "yolo" }),
            ))
            .unwrap_err();
        assert!(e.contains("changed its mode to \"yolo\""), "{e}");
    }

    #[test]
    fn titles_and_targets_are_shown_in_one_short_line() {
        assert_eq!(
            mcp_target("send(to: lead, type: inform, body: pong)"),
            "lead"
        );
        assert_eq!(
            mcp_target("post(title: Fix the tests, body: ...)"),
            "Fix the tests"
        );
        assert_eq!(mcp_target("board"), "");
        assert_eq!(one_line("a\nb"), "a");
        let long = "x".repeat(130);
        assert_eq!(one_line(&long).chars().count(), 121);
        let exact = "y".repeat(120);
        assert_eq!(one_line(&exact), exact, "120 fit as they are");
        assert_eq!(relative("/work/w1/a/b.rs", Path::new("/work/w1")), "a/b.rs");
        assert_eq!(
            relative("/other/b.rs", Path::new("/work/w1")),
            "/other/b.rs"
        );
        assert_eq!(active_extensions("").unwrap(), Vec::<String>::new());
        assert_eq!(active_extensions("[]").unwrap(), Vec::<String>::new());
        assert_eq!(active_extensions(r#"[{"isActive": true}]"#).unwrap(), ["?"]);
    }

    fn fake(dir: &Path, name: &str, body: &str) -> String {
        std::fs::create_dir_all(dir).unwrap();
        let path = dir.join(name);
        std::fs::write(&path, format!("#!/bin/sh\n{body}\n")).unwrap();
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o755)).unwrap();
        path.to_string_lossy().into_owned()
    }

    /// Shell that answers the n-th request with the lines of `stepN.txt`
    /// in its working directory (the request's id put in for `__ID__`),
    /// and exits when there are no more.
    const FAKE: &str = r#"printf '%s\n' "$@" > args.txt
env > env.txt
n=0
while IFS= read -r line; do
  n=$((n+1))
  id=$(printf '%s' "$line" | sed -E 's/.*"id":([0-9]+).*/\1/')
  f="step$n.txt"
  [ -f "$f" ] || exit 0
  sed "s/__ID__/$id/" "$f"
  [ -f "step$((n+1)).txt" ] || exit 0
done"#;

    /// A JSON-RPC line whose id the stand-in fills in: a number, not a
    /// string.
    fn unquoted_id(v: Value) -> String {
        v.to_string().replace("\"__ID__\"", "__ID__")
    }

    /// The lines a clean start answers with, step by step.
    fn clean_steps(spec: &Spec) -> Vec<String> {
        let want = spec.model.clone().unwrap_or_else(|| "auto".into());
        let result =
            |r: Value| unquoted_id(json!({ "jsonrpc": "2.0", "id": "__ID__", "result": r }));
        vec![
            result(init()),
            result(session(&want)),
            format!(
                "{}\n{}",
                chunk(&about(&want, "gemini-api-key")),
                result(command_done())
            ),
            format!("{}\n{}", chunk("[]"), result(command_done())),
            format!("{}\n{}", chunk(&memory(spec)), result(command_done())),
        ]
    }

    fn run(spec: &Spec, steps: &[String], prompts: &[&str]) -> Vec<Event> {
        std::fs::create_dir_all(&spec.home).unwrap();
        for (i, s) in steps.iter().enumerate() {
            std::fs::write(
                spec.home.join(format!("step{}.txt", i + 1)),
                format!("{s}\n"),
            )
            .unwrap();
        }
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

    #[test]
    fn a_session_starts_runs_a_turn_and_reports_events() {
        let dir = std::env::temp_dir().join(format!("imece-gemini-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let mut s = with_bus(false);
        s.cwd = dir.join("repo");
        s.home = dir.join("home");
        std::fs::create_dir_all(&s.cwd).unwrap();
        s.program = fake(&dir, "fake-gemini", FAKE);
        let mut steps = clean_steps(&s);
        let turn = format!(
            "{}\n{}\n{}",
            chunk("pong"),
            update(
                json!({ "sessionUpdate": "tool_call", "toolCallId": "c1", "title": "README.md", "kind": "read",
                "locations": [{ "path": s.cwd.join("README.md") }] })
            ),
            unquoted_id(
                json!({ "jsonrpc": "2.0", "id": "__ID__", "result": turn_result("gemini-2.5-flash", 1000, 0, 100, 0) })
            )
        );
        steps.push(turn);
        let got = run(&s, &steps, &["ping"]);
        assert_eq!(
            got,
            [
                Event::Started {
                    session: "s1".into(),
                    model: "auto".into()
                },
                Event::Note(
                    "gemini runs on your API key: its turns are booked at list price".into()
                ),
                Event::Tool {
                    name: "read".into(),
                    target: "README.md".into()
                },
                Event::Text("pong".into()),
                // 1000 at 0.30 and 100 at 2.50.
                Event::TurnDone {
                    ok: true,
                    text: "pong".into(),
                    cost: 550,
                    denied: 0
                },
                Event::Note("tokens so far: 1000 in (0 cached) · 100 out".into()),
                // The stand-in has no more answers and exits.
                Event::Stopped("exited with code 0".into()),
            ],
            "{got:?}"
        );
        let args = std::fs::read_to_string(s.home.join("args.txt")).unwrap();
        assert!(args.starts_with("--acp\n"), "{args}");
        assert!(
            args.contains(&format!("--include-directories\n{}\n", s.cwd.display())),
            "{args}"
        );
        let env = std::fs::read_to_string(s.home.join("env.txt")).unwrap();
        assert!(env.contains("GEMINI_CLI_TRUST_WORKSPACE=true"), "{env}");
        assert!(env.contains("GEMINI_TELEMETRY_ENABLED=false"), "{env}");
        assert!(env.contains("NO_BROWSER=true"), "{env}");
        assert!(s.home.join("policy/imece.toml").exists() && s.home.join("GEMINI.md").exists());
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn a_session_whose_settings_differ_is_stopped_before_its_events_count() {
        let dir = std::env::temp_dir().join(format!("imece-gemini-bad-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let mut s = spec(false);
        s.cwd = dir.join("repo");
        s.home = dir.join("home");
        std::fs::create_dir_all(&s.cwd).unwrap();
        s.program = fake(&dir, "fake-gemini", FAKE);
        let mut steps = clean_steps(&s);
        steps[2] = format!(
            "{}\n{}",
            chunk(&about("auto", "vertex-ai")),
            unquoted_id(json!({ "jsonrpc": "2.0", "id": "__ID__", "result": command_done() }))
        );
        let got = run(&s, &steps, &["ping"]);
        assert_eq!(got.len(), 1, "{got:?}");
        assert!(
            matches!(&got[0], Event::Stopped(w) if w.contains("vertex-ai") && w.starts_with("stopped: the session")),
            "{got:?}"
        );
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn a_session_that_gets_no_answer_in_time_is_stopped() {
        // Not run against the clock: REQUEST_WAIT is a minute. The driver's
        // deadline logic is exercised by its waiting flag instead.
        let mut p = Protocol::new(&spec(true));
        assert!(!p.waiting());
        p.open();
        assert!(p.waiting());
        p.line(&reply(1, init())).unwrap();
        assert!(p.waiting(), "the next question is out");
    }

    #[test]
    fn a_prompt_to_a_session_that_is_over_fails() {
        let dir = std::env::temp_dir().join(format!("imece-gemini-over-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let mut s = spec(true);
        s.cwd = dir.join("repo");
        s.home = dir.join("home");
        std::fs::create_dir_all(&s.cwd).unwrap();
        s.program = fake(&dir, "fake-gemini", "exit 3");
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
        // The driver is gone with the process; a prompt has nowhere to go.
        assert!(rx.recv_timeout(Duration::from_secs(5)).is_err());
        assert!(session.prompt("hi").is_err() || session.prompt("hi").is_err());
        session.terminate();
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn a_refused_write_reports_the_process_end_when_it_comes_in_time() {
        let (tx, rx) = mpsc::channel();
        let e = io::Error::from(io::ErrorKind::BrokenPipe);
        tx.send(Msg::Line("late words".into())).unwrap();
        tx.send(Msg::End("exited with code 3: no auth".into()))
            .unwrap();
        assert_eq!(refused(&rx, &e), "exited with code 3: no auth");
        tx.send(Msg::Prompt("hi".into())).unwrap();
        drop(tx);
        let why = refused(&rx, &e);
        assert!(why.starts_with("stopped: gemini took no input ("), "{why}");
        assert!(why.contains("broken pipe"), "{why}");
    }

    /// A stand-in that answers nothing and never exits on its own.
    fn sleeper(dir: &Path) -> Spec {
        let mut s = spec(true);
        s.cwd = dir.join("repo");
        s.home = dir.join("home");
        std::fs::create_dir_all(&s.cwd).unwrap();
        s.program = fake(dir, "fake-gemini", "cat > /dev/null");
        s
    }

    #[test]
    fn terminating_a_session_ends_its_driver_and_its_process() {
        let dir = std::env::temp_dir().join(format!("imece-gemini-term-{}", std::process::id()));
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
        let dir = std::env::temp_dir().join(format!("imece-gemini-drop-{}", std::process::id()));
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
        let dir = std::env::temp_dir().join(format!("imece-gemini-slow-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let mut s = spec(false);
        s.cwd = dir.join("repo");
        s.home = dir.join("home");
        std::fs::create_dir_all(&s.cwd).unwrap();
        // Slower than the driver's poll, well within its patience.
        s.program = fake(&dir, "fake-gemini", &format!("sleep 1.5\n{FAKE}"));
        let steps = clean_steps(&s);
        let got = run(&s, &steps, &[]);
        assert!(matches!(&got[0], Event::Started { .. }), "{got:?}");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    #[ignore = "runs a real Gemini CLI session: IMECE_LIVE_GEMINI=1 cargo test -p imece-agent -- --ignored live_gemini"]
    fn live_gemini_runs_one_turn_with_imece_s_tool() {
        use std::sync::{Arc, Mutex};
        if std::env::var_os("IMECE_LIVE_GEMINI").is_none() {
            return;
        }
        let dir = std::env::temp_dir().join(format!("imece-live-gemini-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let cwd = dir.join("repo");
        std::fs::create_dir_all(&cwd).unwrap();
        std::fs::write(
            cwd.join("README.md"),
            "# scratch\n\nA scratch repository.\n",
        )
        .unwrap();
        let sock = dir.join("a.sock");
        let calls = Arc::new(Mutex::new(Vec::<Value>::new()));
        let seen = Arc::clone(&calls);
        let _server = crate::mcp::Server::bind(
            &sock,
            vec![crate::mcp::Tool {
                name: "send".into(),
                description: "Sends a message to a teammate.".into(),
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
            home: dir.join("home"),
            model: std::env::var("IMECE_LIVE_GEMINI_MODEL").ok(),
            write: false,
            system: "You are gemini-1, a coding agent in imece, a team led by lead. \
                     You can read files but not change them or run commands."
                .into(),
            mcp: Some(Mcp {
                command: "nc".into(),
                args: vec!["-U".into(), sock.to_string_lossy().into_owned()],
                tools: vec!["send".into()],
            }),
        };
        let (tx, rx) = mpsc::channel();
        let mut session = Session::start(&s, move |e| {
            let _ = tx.send(e);
        })
        .unwrap();
        session
            .prompt(
                "With the send tool, send lead an inform message whose body is the \
                 first line of README.md. Then reply with the single word: pong",
            )
            .unwrap();
        let mut got = Vec::new();
        loop {
            let e = rx
                .recv_timeout(Duration::from_secs(300))
                .expect("gemini answers within five minutes");
            eprintln!("live gemini: {e:?}");
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
