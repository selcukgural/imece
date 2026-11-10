//! A stand-in for Claude Code (record 130): `imece stand-in ARGS…` takes
//! the command line imece gives `claude` ([`crate::claude::args`]), speaks
//! stream-json on stdin and stdout as Claude Code does, reaches the MCP
//! servers its `--mcp-config` names as their client (imece's bus, through
//! `imece mcp-relay`), and does a small, fixed piece of work per role: a
//! leader hires a worker and assigns it the person's words, a worker
//! changes a file of its directory as a task list says ([`work`]) and says
//! done. Sometimes a turn fails as Claude Code says an API error, and the
//! next does the prompt that failed. It asks no model and spends nothing;
//! the cost it reports is made up, the same every run.
//!
//! It is what the demo runs on the real `Live` (record 111, `--demo-live`
//! for now), and what tests run the real `Live` on end to end. Under
//! `CLAUDE_CONFIG_DIR` it keeps a transcript per session where Claude Code
//! keeps its own ([`crate::claude::transcript`]), so a `--resume` goes on
//! from what the session had learned and spent; without it, nothing. It is no sandbox and checks
//! nothing: imece's checks of what a session reports run on it as on
//! Claude Code.

use std::collections::BTreeMap;
use std::io::{self, BufRead, BufReader, Write};
use std::path::{Path, PathBuf};
use std::process::{Child, ChildStdin, ChildStdout, Command, Stdio};

use serde_json::{Value, json};

use crate::claude::{EDIT_TOOLS, SHELL_TOOL, SUBAGENT_TOOLS, TASK_TOOLS, WEB_TOOLS};

pub mod work;
pub use work::Habits;

/// What `--version` says: no Claude Code's number, so nothing mistakes it
/// for one.
pub const VERSION: &str = "0.0.0 (imece stand-in)";

/// The context window every turn reports, in tokens.
pub const WINDOW: u64 = 200_000;

/// The tokens a session's context starts with, and what each turn adds.
pub const CONTEXT_START: u64 = 2_000;
pub const CONTEXT_TURN: u64 = 1_500;

/// What a turn costs, in millionths of a dollar, and what each tool call
/// adds to it.
pub const TURN_COST: u64 = 10_000;
pub const CALL_COST: u64 = 5_000;

/// The model a session's background work runs on, as Claude Code's own
/// does (record 134), and its part of what the session spent: an eighth.
pub const BACKGROUND: &str = "claude-haiku-5-5";
pub const BACKGROUND_PART: u64 = 8;

/// The longest task a leader hands on, in characters.
pub const TASK_MAX: usize = 200;

/// The directory a worker writes its file in, inside its own, when it has
/// nothing to change.
pub const OUT_DIR: &str = "stand-in";

/// What a tool call that needs a yes gets in a session that has nobody
/// to ask, as Claude Code words it.
pub const NOT_GRANTED: &str =
    "Claude requested permissions to use {}, but you haven't granted it yet.";

/// What a turn that failed at Claude's API says, as Claude Code does
/// (record 79's probe): an overloaded API after its own three tries, and
/// a refused request for a bill unpaid, at once.
pub const OVERLOADED: &str = "API Error: Repeated 529 Overloaded errors. The API is at capacity.";
pub const NO_CREDIT: &str = "Credit balance is too low";

/// The tries Claude Code makes of an overloaded API before it gives up,
/// of the most it would make, and how long it waits before the first.
pub const TRIES: u64 = 3;
pub const TRIES_MOST: u64 = 10;
pub const TRY_WAIT_MS: u64 = 500;

/// The stand-in as a program: `args` without the program's name; what it
/// exits with.
pub fn main(args: &[String]) -> i32 {
    let cwd = std::env::current_dir().unwrap_or_default();
    let home = home_of(std::env::var_os("CLAUDE_CONFIG_DIR"));
    let habits = Habits::of(|k| std::env::var(k).ok());
    answer(
        args,
        &cwd,
        home.as_deref(),
        &habits,
        io::stdin().lock(),
        io::stdout().lock(),
    )
}

/// Where the transcripts go, from `CLAUDE_CONFIG_DIR`'s value: nowhere
/// when it is not set or empty (never the person's `~/.claude`).
pub fn home_of(var: Option<std::ffi::OsString>) -> Option<PathBuf> {
    var.filter(|d| !d.is_empty()).map(PathBuf::from)
}

/// What the stand-in does with `args` in `cwd`, `input` its stdin and
/// `out` its stdout, its transcripts under `home` (none: kept nowhere),
/// working with `habits`; what it exits with.
pub fn answer(
    args: &[String],
    cwd: &Path,
    home: Option<&Path>,
    habits: &Habits,
    input: impl BufRead,
    mut out: impl Write,
) -> i32 {
    match args {
        [v] if v == "--version" || v == "-v" => {
            let _ = writeln!(out, "{VERSION}");
            0
        }
        [a, s, j] if a == "auth" && s == "status" && j == "--json" => {
            let status = json!({ "loggedIn": true, "authMethod": "stand-in" });
            let _ = writeln!(out, "{status}");
            0
        }
        _ => match Args::parse(args) {
            Ok(a) => {
                run(&a, cwd, home, habits, input, out);
                0
            }
            Err(why) => {
                eprintln!("imece stand-in: {why}");
                2
            }
        },
    }
}

/// Writes `dir/claude`, a program that runs `imece stand-in` with its
/// arguments, `imece` being imece's own binary, its transcripts under
/// `home` (as `CLAUDE_CONFIG_DIR`, which a session's cleared environment
/// would not pass on otherwise), working with `habits`: what a session's
/// program is (one path) when the stand-in plays Claude Code. Gives its
/// path.
pub fn install(dir: &Path, imece: &Path, home: &Path, habits: &Habits) -> io::Result<PathBuf> {
    use std::os::unix::fs::PermissionsExt;
    let quoted = |p: &Path| format!("'{}'", p.to_string_lossy().replace('\'', r"'\''"));
    let path = dir.join("claude");
    let (imece, home, vars) = (quoted(imece), quoted(home), habits.vars());
    std::fs::write(
        &path,
        format!("#!/bin/sh\nCLAUDE_CONFIG_DIR={home} {vars} exec {imece} stand-in \"$@\"\n"),
    )?;
    std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o700))?;
    Ok(path)
}

/// What of Claude Code's command line the stand-in reads.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Args {
    /// `--tools`: a list, or `default`.
    pub tools: Vec<String>,
    /// `--disallowedTools`.
    pub disallowed: Vec<String>,
    /// `--allowedTools`: run without asking.
    pub allowed: Vec<String>,
    pub permission_mode: String,
    /// `--permission-prompt-tool`: hidden from the model, as Claude Code
    /// hides it.
    pub prompt_tool: Option<String>,
    /// `--mcp-config`'s servers: name, command, arguments.
    pub servers: Vec<(String, String, Vec<String>)>,
    pub model: Option<String>,
    pub resume: Option<String>,
    /// `--fork-session` with `--session-id`.
    pub session_id: Option<String>,
    pub fork: bool,
    /// `--replay-user-messages`: each prompt taken is echoed back.
    pub replay: bool,
    /// `--name`: what imece calls the session (`imece claude-1`).
    pub name: String,
}

impl Args {
    pub fn parse(args: &[String]) -> Result<Args, String> {
        let mut a = Args::default();
        let mut it = args.iter();
        let list = |s: &str| -> Vec<String> {
            s.split(',')
                .map(str::trim)
                .filter(|t| !t.is_empty())
                .map(String::from)
                .collect()
        };
        while let Some(flag) = it.next() {
            let mut value = || {
                it.next()
                    .cloned()
                    .ok_or_else(|| format!("{flag} needs a value"))
            };
            match flag.as_str() {
                "--tools" => a.tools = list(&value()?),
                "--disallowedTools" => a.disallowed = list(&value()?),
                "--allowedTools" => a.allowed = list(&value()?),
                "--permission-mode" => a.permission_mode = value()?,
                "--permission-prompt-tool" => a.prompt_tool = Some(value()?),
                "--model" => a.model = Some(value()?),
                "--resume" => a.resume = Some(value()?),
                "--session-id" => a.session_id = Some(value()?),
                "--fork-session" => a.fork = true,
                "--replay-user-messages" => a.replay = true,
                "--mcp-config" => a.servers = servers_of(&value()?)?,
                "--name" => a.name = value()?,
                // Flags that take a value the stand-in does not read.
                "--input-format"
                | "--output-format"
                | "--permission-prompts"
                | "--add-dir"
                | "--append-system-prompt"
                | "--system-prompt"
                | "--max-budget-usd"
                | "--effort"
                | "--settings" => {
                    value()?;
                }
                _ => {}
            }
        }
        Ok(a)
    }

    /// The built-in tools the session has: its list, or Claude Code's
    /// default set less what is disallowed (and less the tools that start
    /// agents, which the stand-in has none of).
    pub fn built_in(&self) -> Vec<String> {
        if self.tools != ["default"] {
            return self.tools.clone();
        }
        let mut t: Vec<String> = (EDIT_TOOLS.iter().chain(WEB_TOOLS).chain(TASK_TOOLS))
            .chain(std::iter::once(&SHELL_TOOL))
            .map(|t| t.to_string())
            .filter(|t| !self.disallowed.contains(t) && !SUBAGENT_TOOLS.contains(&t.as_str()))
            .collect();
        t.sort_unstable();
        t
    }

    /// The session's id: the forked one, else the resumed one, else `new`.
    pub fn session(&self, new: impl FnOnce() -> String) -> String {
        match (&self.resume, &self.session_id) {
            (Some(_), Some(id)) if self.fork => id.clone(),
            (Some(id), _) => id.clone(),
            (None, Some(id)) => id.clone(),
            (None, None) => new(),
        }
    }
}

/// The servers of an `--mcp-config` text, in its order.
fn servers_of(text: &str) -> Result<Vec<(String, String, Vec<String>)>, String> {
    let v: Value = serde_json::from_str(text).map_err(|e| format!("--mcp-config: {e}"))?;
    let servers = v["mcpServers"]
        .as_object()
        .ok_or("--mcp-config: no mcpServers")?;
    Ok(servers
        .iter()
        .map(|(name, s)| {
            let args = (s["args"].as_array().into_iter().flatten())
                .filter_map(Value::as_str)
                .map(String::from)
                .collect();
            let command = s["command"].as_str().unwrap_or_default().to_string();
            (name.clone(), command, args)
        })
        .collect())
}

/// A stdio MCP server the stand-in is the client of.
pub struct Server {
    child: Option<Child>,
    to: Option<ChildStdin>,
    from: Option<BufReader<ChildStdout>>,
    next: u64,
    /// Its tools, by the names it lists; empty when it never answered.
    pub tools: Vec<String>,
}

impl Server {
    /// Starts `command` in `cwd` and asks it for its tools; a server that
    /// does not start or answer is kept as failed (no tools).
    pub fn start(command: &str, args: &[String], cwd: &Path) -> Server {
        let mut s = Server {
            child: None,
            to: None,
            from: None,
            next: 0,
            tools: Vec::new(),
        };
        let spawned = Command::new(command)
            .args(args)
            .current_dir(cwd)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .spawn();
        let Ok(mut child) = spawned else { return s };
        s.to = child.stdin.take();
        s.from = child.stdout.take().map(BufReader::new);
        s.child = Some(child);
        let hello = json!({
            "protocolVersion": "2025-06-18",
            "capabilities": {},
            "clientInfo": { "name": "imece-stand-in", "version": VERSION },
        });
        if s.request("initialize", hello).is_err() {
            return s;
        }
        s.notify("notifications/initialized");
        if let Ok(list) = s.request("tools/list", json!({})) {
            s.tools = (list["tools"].as_array().into_iter().flatten())
                .filter_map(|t| t["name"].as_str())
                .map(String::from)
                .collect();
        }
        s
    }

    /// A server whose tools are `tools` and that answers nothing: for the
    /// tests of what the stand-in reports.
    pub fn listing(tools: &[&str]) -> Server {
        Server {
            child: None,
            to: None,
            from: None,
            next: 0,
            tools: tools.iter().map(|t| t.to_string()).collect(),
        }
    }

    /// Its process, while it runs.
    pub fn pid(&self) -> Option<u32> {
        self.child.as_ref().map(Child::id)
    }

    /// Whether it answered `initialize` and `tools/list`.
    pub fn connected(&self) -> bool {
        !self.tools.is_empty()
    }

    fn notify(&mut self, method: &str) {
        if let Some(to) = &mut self.to {
            let line = json!({ "jsonrpc": "2.0", "method": method });
            let _ = writeln!(to, "{line}").and_then(|()| to.flush());
        }
    }

    /// One request and its result; notifications and answers to other
    /// ids in between are passed over.
    fn request(&mut self, method: &str, params: Value) -> Result<Value, String> {
        self.next += 1;
        let id = self.next;
        let (Some(to), Some(from)) = (&mut self.to, &mut self.from) else {
            return Err("the server is not running".into());
        };
        let line = json!({ "jsonrpc": "2.0", "id": id, "method": method, "params": params });
        // A server that ended may be seen so here (a broken pipe) or at the
        // read below: the same either way.
        writeln!(to, "{line}")
            .and_then(|()| to.flush())
            .map_err(|_| "the server is gone".to_string())?;
        let mut buf = String::new();
        loop {
            buf.clear();
            match from.read_line(&mut buf) {
                Ok(0) | Err(_) => return Err("the server is gone".into()),
                Ok(_) => {}
            }
            let Ok(v) = serde_json::from_str::<Value>(&buf) else {
                continue;
            };
            if v["id"] != id {
                continue;
            }
            if let Some(e) = v.get("error") {
                return Err(e["message"].as_str().unwrap_or("error").to_string());
            }
            return Ok(v["result"].clone());
        }
    }

    /// Calls tool `name`: its text, or the text of its error.
    pub fn call(&mut self, name: &str, args: &Value) -> Result<String, String> {
        let r = self.request("tools/call", json!({ "name": name, "arguments": args }))?;
        let text = (r["content"].as_array().into_iter().flatten())
            .filter_map(|c| c["text"].as_str())
            .collect::<Vec<_>>()
            .join("\n");
        if r["isError"] == true {
            Err(text)
        } else {
            Ok(text)
        }
    }
}

impl Drop for Server {
    fn drop(&mut self) {
        // Its stdin closed, a relay ends by itself; the kill is for one
        // that would not.
        self.to = None;
        if let Some(mut c) = self.child.take() {
            let _ = c.kill();
            let _ = c.wait();
        }
    }
}

/// The name a tool of server `server` goes by in the session.
pub fn mcp_name(server: &str, tool: &str) -> String {
    format!("mcp__{server}__{tool}")
}

/// The `init` line: what the session runs with, as Claude Code reports
/// it.
pub fn init(a: &Args, session: &str, servers: &[(String, Server)]) -> Value {
    let hidden = a.prompt_tool.as_deref();
    let mut tools = a.built_in();
    for (name, s) in servers {
        tools.extend(
            (s.tools.iter())
                .map(|t| mcp_name(name, t))
                .filter(|t| Some(t.as_str()) != hidden),
        );
    }
    let status = |s: &Server| if s.connected() { "connected" } else { "failed" };
    json!({
        "type": "system",
        "subtype": "init",
        "session_id": session,
        "model": a.model.clone().unwrap_or_else(|| "stand-in".into()),
        "permissionMode": a.permission_mode,
        "tools": tools,
        "mcp_servers": servers.iter().map(|(n, s)| json!({ "name": n, "status": status(s) })).collect::<Vec<_>>(),
        "slash_commands": ["clear", "compact"],
    })
}

/// What the stand-in's mind works with: the tools of imece's bus, Claude
/// Code's own tools on the files of its directory, lines of text to say,
/// and its habits.
pub trait Hands {
    /// Calls the bus's tool `tool` (its bare name).
    fn call(&mut self, tool: &str, args: Value) -> Result<String, String>;
    /// Whether the session has the built-in tool `tool`.
    fn has(&self, tool: &str) -> bool;
    /// Uses the built-in tool `tool` (its paths relative to the session's
    /// directory), asking first when it needs a yes.
    fn tool(&mut self, tool: &str, input: Value) -> Result<String, String>;
    /// The files of the session's directory ([`work::files_in`]).
    fn files(&self) -> Vec<String>;
    fn say(&mut self, text: &str);
    /// Waits between two steps, as long as the habits say.
    fn pause(&mut self);
    fn habits(&self) -> &Habits;
}

/// What the stand-in has learned in its session.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Mind {
    /// `whoami`'s answer, by its `key: value` lines; asked once.
    pub me: BTreeMap<String, String>,
    /// The workers a leader hired.
    pub hired: Vec<String>,
    /// The assigns a worker (or a helper) has worked on.
    pub worked: u64,
    /// The task a worker waits on, for the person's answer or its
    /// helper's done.
    pub waiting: Option<work::Waiting>,
    /// The prompts the session has taken.
    pub turns: u64,
    /// The prompt whose turn failed at Claude's API: still in the
    /// session, as Claude Code keeps it, for the next turn to do.
    pub failed: Option<Failed>,
}

/// A prompt whose turn failed at Claude's API, and whether on the
/// account's fault (the person mends it) or a passing one.
#[derive(Clone, Debug, PartialEq)]
pub struct Failed {
    pub prompt: String,
    pub account: bool,
}

impl Failed {
    /// The prompts the next turn does after this failure, the next prompt
    /// being `prompt`: imece's try again is the failed prompt alone; after
    /// the account's fault, the person's words are what lets it go on
    /// (imece holds mail while it waits); any other prompt comes after the
    /// failed one, in the same turn.
    pub fn then<'a>(&'a self, prompt: &'a str) -> Vec<&'a str> {
        let again = prompt == crate::RETRY_PROMPT;
        if again || (self.account && !prompt.starts_with(MAIL)) {
            vec![&self.prompt]
        } else {
            vec![&self.prompt, prompt]
        }
    }
}

impl Mind {
    /// The mind as a transcript keeps it.
    pub fn to_json(&self) -> Value {
        let waiting = self.waiting.as_ref().map(work::Waiting::to_json);
        let failed =
            (self.failed.as_ref()).map(|f| json!({ "prompt": f.prompt, "account": f.account }));
        json!({
            "me": self.me,
            "hired": self.hired,
            "worked": self.worked,
            "waiting": waiting,
            "turns": self.turns,
            "failed": failed,
        })
    }

    /// A mind a transcript kept; what is not text is left out.
    pub fn from_json(v: &Value) -> Mind {
        let text = |v: &Value| v.as_str().map(String::from);
        Mind {
            me: (v["me"].as_object().into_iter().flatten())
                .filter_map(|(k, v)| Some((k.clone(), text(v)?)))
                .collect(),
            hired: (v["hired"].as_array().into_iter().flatten())
                .filter_map(text)
                .collect(),
            worked: v["worked"].as_u64().unwrap_or(0),
            waiting: work::Waiting::from_json(&v["waiting"]),
            turns: v["turns"].as_u64().unwrap_or(0),
            failed: text(&v["failed"]["prompt"]).map(|prompt| Failed {
                prompt,
                account: v["failed"]["account"] == true,
            }),
        }
    }

    /// One turn on prompt `prompt`: what it ends with.
    pub fn turn(&mut self, hands: &mut impl Hands, prompt: &str) -> String {
        if self.me.is_empty()
            && let Ok(said) = hands.call("whoami", json!({}))
        {
            self.me = said
                .lines()
                .filter_map(|l| l.split_once(": "))
                .map(|(k, v)| (k.to_string(), v.to_string()))
                .collect();
        }
        let role = self.me.get("role").map_or("", String::as_str);
        match role {
            "the leader" => self.lead(hands, prompt),
            "a worker" => self.work(hands, prompt),
            "a helper" => self.help(hands, prompt),
            _ => "Nothing for me to do here.".into(),
        }
    }

    fn lead(&mut self, hands: &mut impl Hands, prompt: &str) -> String {
        if prompt.starts_with(MAIL) {
            let done = headed(prompt, "done");
            return if done.is_empty() {
                "Read my mail; nothing to do.".into()
            } else {
                format!("{} said done.", done.join(", "))
            };
        }
        let task = task_of(prompt);
        let name = match self.hired.first() {
            Some(n) => n.clone(),
            None => match hands.call("hire", json!({})) {
                Ok(said) => {
                    let name = said.split(' ').next().unwrap_or_default().to_string();
                    self.hired.push(name.clone());
                    name
                }
                Err(why) => return format!("I could not hire: {why}"),
            },
        };
        let sent = hands.call(
            "send",
            json!({ "to": name, "type": "assign", "body": task }),
        );
        match sent {
            Ok(_) => {
                hands.say(&format!("I gave {name} the work."));
                format!("{name} has it: {}", first_line(&task))
            }
            Err(why) => format!("{name} did not get it: {why}"),
        }
    }
}

/// How the prompt that hands an agent its mail begins (`mail_prompt` in
/// the screen's crate).
pub const MAIL: &str = "Messages from your teammates in imece";

/// Who sent mail of type `kind` in a mail prompt, in its order.
fn headed(prompt: &str, kind: &str) -> Vec<String> {
    (prompt.lines())
        .filter_map(|l| l.strip_prefix('['))
        .filter(|l| l.starts_with(&format!("{kind} ")) || l.starts_with(&format!("{kind}]")))
        .filter_map(|l| l.split_once("] from "))
        .map(|(_, who)| who.trim_end_matches(':').to_string())
        .collect()
}

/// The body of the first message of type `kind` in a mail prompt: the
/// lines after its heading, up to the next heading.
fn body_of(prompt: &str, kind: &str) -> Option<String> {
    let mut lines = prompt.lines();
    lines.find(|l| {
        l.strip_prefix('[').is_some_and(|l| {
            l.starts_with(&format!("{kind} ")) || l.starts_with(&format!("{kind}]"))
        })
    })?;
    let body: Vec<&str> = lines.take_while(|l| !l.starts_with('[')).collect();
    let body = body.join("\n").trim().to_string();
    (!body.is_empty()).then_some(body)
}

/// What a leader hands on of the person's words: the prompt's last
/// paragraph (imece puts its own notes first), at most [`TASK_MAX`]
/// characters.
fn task_of(prompt: &str) -> String {
    let last = prompt
        .split("\n\n")
        .map(str::trim)
        .filter(|p| !p.is_empty())
        .last()
        .unwrap_or("Look around.");
    last.chars().take(TASK_MAX).collect()
}

fn first_line(text: &str) -> &str {
    text.lines().next().unwrap_or_default()
}

/// A session's stream: the lines it writes, and what it has spent and
/// holds.
struct Stream<W: Write> {
    out: W,
    /// In millionths of a dollar, since the session's process started.
    spent: u64,
    context: u64,
    calls: u64,
    /// The tasks its task tools made.
    tasks: u64,
    model: String,
}

impl<W: Write> Stream<W> {
    fn line(&mut self, v: &Value) {
        let _ = writeln!(self.out, "{v}").and_then(|()| self.out.flush());
    }

    fn assistant(&mut self, content: Value) {
        self.context += CONTEXT_TURN / 10;
        let usage = json!({
            "input_tokens": self.context,
            "cache_creation_input_tokens": 0,
            "cache_read_input_tokens": 0,
            "output_tokens": 0,
        });
        self.line(&json!({
            "type": "assistant",
            "message": { "role": "assistant", "content": content, "usage": usage },
        }));
    }

    /// A call of tool `name` begins: its id.
    fn tool_use(&mut self, name: &str, input: &Value) -> String {
        self.calls += 1;
        let id = format!("toolu_{}", self.calls);
        self.assistant(json!([{ "type": "tool_use", "id": id, "name": name, "input": input }]));
        id
    }

    /// The call `id` ends with `said`.
    fn tool_result(&mut self, id: &str, said: &Result<String, String>) {
        let (text, error) = match said {
            Ok(t) => (t.as_str(), false),
            Err(t) => (t.as_str(), true),
        };
        self.spent += CALL_COST;
        self.line(&json!({
            "type": "user",
            "message": { "role": "user", "content": [{
                "type": "tool_result", "tool_use_id": id, "content": text, "is_error": error,
            }]},
        }));
    }

    /// The turn's end, `cost` more spent on it.
    fn result(&mut self, text: &str, cost: u64) {
        self.spent += cost;
        let usd = self.spent as f64 / 1_000_000.0;
        self.line(&json!({
            "type": "result",
            "subtype": "success",
            "is_error": false,
            "result": text,
            "total_cost_usd": usd,
            "permission_denials": [],
            "modelUsage": self.usage(),
        }));
    }

    /// A turn that failed at Claude's API on `fault`, in session
    /// `session`: Claude Code's own tries of an overloaded API, `wait`
    /// called between them, then a result that is an API error; nothing
    /// spent, as no answer came. The account's fault is a bill unpaid,
    /// refused at once.
    fn failure(&mut self, fault: crate::Fault, session: &str, mut wait: impl FnMut()) {
        let (status, text) = match fault {
            crate::Fault::Account => (400, NO_CREDIT),
            crate::Fault::Passing => {
                for attempt in 1..=TRIES {
                    self.line(&json!({
                        "type": "system",
                        "subtype": "api_retry",
                        "attempt": attempt,
                        "max_retries": TRIES_MOST,
                        "retry_delay_ms": TRY_WAIT_MS << (attempt - 1),
                        "error_status": 529,
                        "error": "overloaded",
                        "session_id": session,
                    }));
                    wait();
                }
                (529, OVERLOADED)
            }
        };
        let usd = self.spent as f64 / 1_000_000.0;
        self.line(&json!({
            "type": "result",
            "subtype": "success",
            "is_error": true,
            "terminal_reason": "api_error",
            "api_error_status": status,
            "result": text,
            "total_cost_usd": usd,
            "permission_denials": [],
            "modelUsage": self.usage(),
        }));
    }

    /// The session's cost by model, as a result's `modelUsage` says it:
    /// its own model's and the background's part, both the session's
    /// whole (record 134).
    fn usage(&self) -> Value {
        let usd = |m: u64| m as f64 / 1_000_000.0;
        if self.model == BACKGROUND {
            let whole = json!({ "contextWindow": WINDOW, "costUSD": usd(self.spent) });
            return json!({ BACKGROUND: whole });
        }
        let background = self.spent / BACKGROUND_PART;
        json!({
            self.model.clone(): {
                "contextWindow": WINDOW,
                "costUSD": usd(self.spent - background),
            },
            BACKGROUND: { "contextWindow": WINDOW, "costUSD": usd(background) },
        })
    }
}

/// The hands of a running session: its bus, its directory, its stream,
/// what its command line gives it and its habits.
struct Busy<'a, W: Write> {
    stream: &'a mut Stream<W>,
    servers: &'a mut [(String, Server)],
    cwd: &'a Path,
    args: &'a Args,
    /// Its built-in tools ([`Args::built_in`]).
    tools: &'a [String],
    habits: &'a Habits,
}

impl<W: Write> Busy<'_, W> {
    /// Calls tool `tool` of server `server`, nothing streamed.
    fn server_call(&mut self, server: &str, tool: &str, args: &Value) -> Result<String, String> {
        let found = self.servers.iter_mut().find(|(n, _)| n == server);
        let Some((_, s)) = found else {
            return Err(format!("no {server} MCP server"));
        };
        s.call(tool, args)
    }

    /// Whether built-in tool `tool` asks before it runs: the shell and the
    /// web unless allowed outright; reading, the task tools and edits
    /// (imece runs Claude Code under `acceptEdits`) never.
    fn needs_yes(&self, tool: &str) -> bool {
        (tool == SHELL_TOOL || WEB_TOOLS.contains(&tool))
            && !self.args.allowed.iter().any(|t| t == tool)
    }

    /// The yes for call `id` of `tool` on `input`, from the permission
    /// prompt tool, as Claude Code asks it (hidden from the stream).
    fn permit(&mut self, tool: &str, input: &Value, id: &str) -> Result<(), String> {
        let refused = || NOT_GRANTED.replace("{}", tool);
        let prompt = self.args.prompt_tool.clone();
        let Some((server, name)) = prompt
            .as_deref()
            .and_then(|p| p.strip_prefix("mcp__"))
            .and_then(|p| p.split_once("__"))
        else {
            return Err(refused());
        };
        let asked = json!({ "tool_name": tool, "input": input, "tool_use_id": id });
        let said = self.server_call(server, name, &asked)?;
        let v: Value = serde_json::from_str(&said).map_err(|_| refused())?;
        if v["behavior"] == "allow" {
            Ok(())
        } else {
            Err(v["message"].as_str().map_or_else(refused, String::from))
        }
    }

    /// `path` in the session's directory; a path outside it is refused.
    fn inside(&self, path: &str) -> Result<PathBuf, String> {
        let p = Path::new(path);
        // A whole path stays as it is.
        let full = self.cwd.join(p);
        let up = p.components().any(|c| c == std::path::Component::ParentDir);
        if up || !full.starts_with(self.cwd) {
            return Err(format!("{path} is outside the session's directory"));
        }
        Ok(full)
    }

    /// What built-in tool `tool` does with `input`: files read, written
    /// and edited in the session's directory, a command's made-up answer,
    /// a task made or changed.
    fn run_tool(&mut self, tool: &str, input: &Value) -> Result<String, String> {
        let text = |k: &str| input[k].as_str().unwrap_or_default().to_string();
        let file = || -> Result<PathBuf, String> { self.inside(&text("file_path")) };
        match tool {
            "Read" => {
                let was = std::fs::read_to_string(file()?).map_err(|e| e.to_string())?;
                Ok(work::numbered(&was))
            }
            "Write" => {
                let full = file()?;
                (full.parent())
                    .map_or(Ok(()), std::fs::create_dir_all)
                    .and_then(|()| std::fs::write(&full, text("content")))
                    .map(|()| format!("File created successfully at: {}", full.display()))
                    .map_err(|e| e.to_string())
            }
            "Edit" => {
                let full = file()?;
                let was = std::fs::read_to_string(&full).map_err(|e| e.to_string())?;
                let now = work::edited(&was, &text("old_string"), &text("new_string"))?;
                std::fs::write(&full, now)
                    .map(|()| format!("The file {} has been updated successfully.", full.display()))
                    .map_err(|e| e.to_string())
            }
            SHELL_TOOL => Ok(work::ran(&text("command")).to_string()),
            "TaskCreate" => {
                self.stream.tasks += 1;
                Ok(format!(
                    "Task #{} created successfully: {}",
                    self.stream.tasks,
                    text("subject")
                ))
            }
            "TaskUpdate" => Ok(format!("Updated task #{} status", text("taskId"))),
            _ => Err(format!("the stand-in does not run {tool}")),
        }
    }
}

impl<W: Write> Hands for Busy<'_, W> {
    fn call(&mut self, tool: &str, args: Value) -> Result<String, String> {
        // A session without imece's server has none of its tools to call.
        if !self
            .servers
            .iter()
            .any(|(n, _)| n == crate::claude::MCP_SERVER)
        {
            return Err("no imece MCP server".into());
        }
        let id = (self.stream).tool_use(&mcp_name(crate::claude::MCP_SERVER, tool), &args);
        let said = self.server_call(crate::claude::MCP_SERVER, tool, &args);
        self.stream.tool_result(&id, &said);
        said
    }

    fn has(&self, tool: &str) -> bool {
        self.tools.iter().any(|t| t == tool)
    }

    fn tool(&mut self, tool: &str, mut input: Value) -> Result<String, String> {
        if !self.has(tool) {
            return Err(format!("No such tool available: {tool}"));
        }
        // Claude Code names a file by its whole path.
        if let Some(path) = input["file_path"].as_str() {
            let full = self.cwd.join(path);
            input["file_path"] = json!(full.to_string_lossy());
        }
        let id = self.stream.tool_use(tool, &input);
        let said = if self.needs_yes(tool) {
            self.permit(tool, &input, &id)
        } else {
            Ok(())
        };
        let said = said.and_then(|()| self.run_tool(tool, &input));
        self.stream.tool_result(&id, &said);
        said
    }

    fn files(&self) -> Vec<String> {
        work::files_in(self.cwd)
    }

    fn say(&mut self, text: &str) {
        self.stream
            .assistant(json!([{ "type": "text", "text": text }]));
    }

    fn pause(&mut self) {
        if !self.habits.pause.is_zero() {
            std::thread::sleep(self.habits.pause);
        }
    }

    fn habits(&self) -> &Habits {
        self.habits
    }
}

/// The echo of a prompt taken, as Claude Code writes it under
/// `--replay-user-messages`.
fn replayed(prompt: &str, session: &str) -> Value {
    serde_json::json!({
        "type": "user",
        "message": { "role": "user", "content": prompt },
        "session_id": session,
        "parent_tool_use_id": null,
        "isReplay": true,
    })
}

/// The text of a stdin line that gives a prompt.
fn prompt_of(line: &str) -> Option<String> {
    let v: Value = serde_json::from_str(line).ok()?;
    if v["type"] != "user" {
        return None;
    }
    let c = &v["message"]["content"];
    match c.as_str() {
        Some(t) => Some(t.to_string()),
        None => Some(
            (c.as_array().into_iter().flatten())
                .filter_map(|b| b["text"].as_str())
                .collect::<Vec<_>>()
                .join("\n"),
        ),
    }
}

/// What a transcript keeps of a session after each of its turns: its
/// mind, what it has spent (a resumed session reports its whole cost, as
/// Claude Code's does) and its context.
#[derive(Clone, Debug, PartialEq)]
pub struct Kept {
    pub mind: Mind,
    pub spent: u64,
    pub context: u64,
}

impl Kept {
    fn fresh() -> Kept {
        Kept {
            mind: Mind::default(),
            spent: 0,
            context: CONTEXT_START,
        }
    }

    fn line(&self) -> Value {
        json!({
            "type": "stand-in",
            "mind": self.mind.to_json(),
            "spent": self.spent,
            "context": self.context,
        })
    }
}

/// The last of what session `id`'s transcript under `home` kept, in
/// whichever directory it ran (a fork goes on from another's).
pub fn kept(home: &Path, id: &str) -> Option<Kept> {
    let file = format!("{id}.jsonl");
    let text = (std::fs::read_dir(home.join("projects")).ok()?)
        .flatten()
        .find_map(|d| std::fs::read_to_string(d.path().join(&file)).ok())?;
    let v: Value = serde_json::from_str(text.lines().last()?).ok()?;
    Some(Kept {
        mind: Mind::from_json(&v["mind"]),
        spent: v["spent"].as_u64()?,
        context: v["context"].as_u64()?,
    })
}

/// Adds `k` to session `id`'s transcript under `home` for `cwd`. A
/// transcript that can not be written leaves a resume to start afresh.
fn keep(home: &Path, cwd: &Path, id: &str, k: &Kept) {
    let file = crate::claude::transcript(home, cwd, id);
    let wrote = file
        .parent()
        .map_or(Ok(()), std::fs::create_dir_all)
        .and_then(|()| {
            std::fs::OpenOptions::new()
                .create(true)
                .append(true)
                .open(&file)
        })
        .and_then(|mut f| writeln!(f, "{}", k.line()));
    if let Err(e) = wrote {
        eprintln!("imece stand-in: {}: {e}", file.display());
    }
}

/// A session: its servers started in `cwd`, `init`, then a turn per
/// prompt until stdin ends; its transcript under `home`, and a
/// `--resume` going on from the one there (one not found: afresh).
pub fn run(
    a: &Args,
    cwd: &Path,
    home: Option<&Path>,
    habits: &Habits,
    input: impl BufRead,
    out: impl Write,
) {
    let mut servers: Vec<(String, Server)> = (a.servers.iter())
        .map(|(n, c, args)| (n.clone(), Server::start(c, args, cwd)))
        .collect();
    let new = || crate::new_uuid().unwrap_or_else(|_| "stand-in".into());
    let mut session = a.session(new);
    let was = (home.zip(a.resume.as_deref()))
        .and_then(|(h, id)| kept(h, id))
        .unwrap_or_else(Kept::fresh);
    let mut stream = Stream {
        out,
        spent: was.spent,
        context: was.context,
        calls: 0,
        tasks: 0,
        model: a.model.clone().unwrap_or_else(|| "stand-in".into()),
    };
    let tools = a.built_in();
    stream.line(&init(a, &session, &servers));
    let mut mind = was.mind;
    let kept_now = |session: &str, mind: &Mind, stream: &Stream<_>| {
        if let Some(h) = home {
            let k = Kept {
                mind: mind.clone(),
                spent: stream.spent,
                context: stream.context,
            };
            keep(h, cwd, session, &k);
        }
    };
    kept_now(&session, &mind, &stream);
    for line in input.lines() {
        let Ok(line) = line else { break };
        let Some(prompt) = prompt_of(&line) else {
            continue;
        };
        if prompt.trim() == "/clear" {
            // A new conversation: a new session, an empty context, the
            // cost counter from nothing, as Claude Code's `/clear` does.
            session = new();
            (stream.spent, stream.context) = (0, CONTEXT_START);
            mind = Mind::default();
            stream.line(&init(a, &session, &servers));
            stream.result("", 0);
            kept_now(&session, &mind, &stream);
            continue;
        }
        if a.replay {
            // Claude Code echoes a prompt as it takes it; its own
            // commands (`/clear` above) are not echoed.
            stream.line(&replayed(&prompt, &session));
        }
        let roll = work::roll(habits.seed, &a.name, mind.turns);
        mind.turns += 1;
        // A turn fails at Claude's API before any work; the one after a
        // failure goes through, so imece's tries are never used up.
        if mind.failed.is_none()
            && let Some(fault) = habits.fault_now(roll)
        {
            stream.failure(fault, &session, || std::thread::sleep(habits.pause));
            mind.failed = Some(Failed {
                prompt,
                account: fault == crate::Fault::Account,
            });
            kept_now(&session, &mind, &stream);
            continue;
        }
        let failed = mind.failed.take();
        let prompts = failed
            .as_ref()
            .map_or(vec![prompt.as_str()], |f| f.then(&prompt));
        let mut hands = Busy {
            stream: &mut stream,
            servers: &mut servers,
            cwd,
            args: a,
            tools: &tools,
            habits,
        };
        let texts: Vec<String> = (prompts.into_iter())
            .map(|p| mind.turn(&mut hands, p))
            .collect();
        let text = texts.join("\n\n");
        // Claude Code's last message is the turn's text, then its result.
        hands.say(&text);
        stream.context += CONTEXT_TURN;
        stream.result(&text, TURN_COST);
        kept_now(&session, &mind, &stream);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    pub(super) fn strings(s: &[&str]) -> Vec<String> {
        s.iter().map(|s| s.to_string()).collect()
    }

    /// Hands on files in memory: the bus answers what `answers` says
    /// (else nothing), the built-in tools in `tools` work on `files` as
    /// the session's do ([`work::numbered`], [`work::edited`]), a tool in
    /// `refuse` fails with its words.
    #[derive(Default)]
    pub(super) struct Fake {
        pub calls: Vec<(String, Value)>,
        /// The built-in tools used, with their input.
        pub used: Vec<(String, Value)>,
        pub said: Vec<String>,
        pub answers: BTreeMap<String, Result<String, String>>,
        pub tools: Vec<String>,
        pub files: BTreeMap<String, String>,
        pub refuse: BTreeMap<String, String>,
        pub habits: Habits,
        pub pauses: usize,
        tasks: u64,
    }

    impl Hands for Fake {
        fn call(&mut self, tool: &str, args: Value) -> Result<String, String> {
            self.calls.push((tool.to_string(), args));
            self.answers
                .get(tool)
                .cloned()
                .unwrap_or_else(|| Ok(String::new()))
        }
        fn has(&self, tool: &str) -> bool {
            self.tools.iter().any(|t| t == tool)
        }
        fn tool(&mut self, tool: &str, input: Value) -> Result<String, String> {
            assert!(self.has(tool), "{tool} is not the session's");
            self.used.push((tool.to_string(), input.clone()));
            if let Some(why) = self.refuse.get(tool) {
                return Err(why.clone());
            }
            let text = |k: &str| input[k].as_str().unwrap_or_default().to_string();
            let path = text("file_path");
            match tool {
                "Read" => (self.files.get(&path))
                    .map(|t| work::numbered(t))
                    .ok_or_else(|| "no such file".into()),
                "Write" => {
                    self.files.insert(path, text("content"));
                    Ok("written".into())
                }
                "Edit" => {
                    let was = self.files.get(&path).cloned().unwrap_or_default();
                    let now = work::edited(&was, &text("old_string"), &text("new_string"))?;
                    self.files.insert(path, now);
                    Ok("edited".into())
                }
                "Bash" => Ok(work::ran(&text("command")).into()),
                "TaskCreate" => {
                    self.tasks += 1;
                    Ok(format!("Task #{} created successfully: x", self.tasks))
                }
                _ => Ok(String::new()),
            }
        }
        fn files(&self) -> Vec<String> {
            self.files.keys().cloned().collect()
        }
        fn say(&mut self, text: &str) {
            self.said.push(text.to_string());
        }
        fn pause(&mut self) {
            self.pauses += 1;
        }
        fn habits(&self) -> &Habits {
            &self.habits
        }
    }

    pub(super) fn fake(whoami: &str) -> Fake {
        let mut f = Fake::default();
        f.answers.insert("whoami".into(), Ok(whoami.into()));
        f
    }

    const LEADER: &str = "name: lead\nrole: the leader\nproject: repo\nyou answer to: the person\n";
    pub(super) const WORKER: &str =
        "name: claude-1\nrole: a worker\nproject: repo\nyou answer to: the leader lead\n";

    #[test]
    fn its_command_line_is_read_as_claude_code_s() {
        let config = r#"{"mcpServers":{"imece":{"type":"stdio","command":"/bin/imece","args":["mcp-relay","/s/a0.sock"]}}}"#;
        let a = Args::parse(&strings(&[
            "-p",
            "--input-format",
            "stream-json",
            "--permission-mode",
            "acceptEdits",
            "--permission-prompts",
            "host",
            "--permission-prompt-tool",
            "mcp__imece__approve",
            "--tools",
            "Read,Write",
            "--name",
            "imece lead",
            "--append-system-prompt",
            "--model",
            "--model",
            "haiku",
            "--resume",
            "old",
            "--fork-session",
            "--session-id",
            "new",
            "--mcp-config",
            config,
            "--disallowedTools",
            "Agent, Task",
            "--allowedTools",
            "mcp__imece__send,WebFetch",
        ]))
        .unwrap();
        assert_eq!(
            a,
            Args {
                tools: strings(&["Read", "Write"]),
                disallowed: strings(&["Agent", "Task"]),
                allowed: strings(&["mcp__imece__send", "WebFetch"]),
                permission_mode: "acceptEdits".into(),
                prompt_tool: Some("mcp__imece__approve".into()),
                servers: vec![(
                    "imece".into(),
                    "/bin/imece".into(),
                    strings(&["mcp-relay", "/s/a0.sock"])
                )],
                model: Some("haiku".into()),
                resume: Some("old".into()),
                session_id: Some("new".into()),
                fork: true,
                replay: false,
                name: "imece lead".into(),
            },
            "a value that looks like a flag is a value"
        );
    }

    #[test]
    fn a_flag_without_its_value_and_a_bad_config_are_refused() {
        assert_eq!(
            Args::parse(&strings(&["--tools"])),
            Err("--tools needs a value".into())
        );
        assert!(Args::parse(&strings(&["--mcp-config", "{"])).is_err());
        assert_eq!(
            Args::parse(&strings(&["--mcp-config", "{}"])),
            Err("--mcp-config: no mcpServers".into())
        );
        assert_eq!(
            Args::parse(&strings(&["--verbose", "--no-chrome"])),
            Ok(Args::default()),
            "flags without a value pass"
        );
    }

    #[test]
    fn the_session_is_the_fork_s_else_the_resumed_one_else_new() {
        let new = || "fresh".to_string();
        let mut a = Args::default();
        assert_eq!(a.session(new), "fresh");
        a.session_id = Some("asked".into());
        assert_eq!(a.session(new), "asked", "--session-id alone");
        a.resume = Some("old".into());
        assert_eq!(a.session(new), "old", "a resume that is no fork");
        a.fork = true;
        assert_eq!(a.session(new), "asked");
        a.session_id = None;
        assert_eq!(a.session(new), "old", "a fork without its new id");
    }

    #[test]
    fn the_default_tools_are_claude_code_s_less_the_disallowed_and_the_agents() {
        let a = Args {
            tools: strings(&["default"]),
            disallowed: strings(&["Agent", "Task", "WebFetch"]),
            ..Args::default()
        };
        assert_eq!(
            a.built_in(),
            strings(&[
                "Bash",
                "Edit",
                "Glob",
                "Grep",
                "Read",
                "TaskCreate",
                "TaskGet",
                "TaskList",
                "TaskUpdate",
                "WebSearch",
                "Write"
            ])
        );
        let listed = Args {
            tools: strings(&["Read"]),
            disallowed: strings(&["Read"]),
            ..Args::default()
        };
        assert_eq!(listed.built_in(), strings(&["Read"]), "a list is the list");
    }

    #[test]
    fn init_reports_the_tools_and_servers_and_hides_the_prompt_tool() {
        let a = Args {
            tools: strings(&["Read"]),
            permission_mode: "acceptEdits".into(),
            prompt_tool: Some("mcp__imece__approve".into()),
            model: Some("haiku".into()),
            ..Args::default()
        };
        let servers = vec![
            ("imece".to_string(), Server::listing(&["send", "approve"])),
            ("other".to_string(), Server::listing(&[])),
        ];
        let v = init(&a, "s1", &servers);
        assert_eq!(v["type"], "system");
        assert_eq!(v["subtype"], "init");
        assert_eq!(v["session_id"], "s1");
        assert_eq!(v["model"], "haiku");
        assert_eq!(v["permissionMode"], "acceptEdits");
        assert_eq!(v["tools"], json!(["Read", "mcp__imece__send"]));
        assert_eq!(
            v["mcp_servers"],
            json!([
                { "name": "imece", "status": "connected" },
                { "name": "other", "status": "failed" },
            ])
        );
        assert!(v["slash_commands"].is_array());
        assert_eq!(init(&Args::default(), "s", &[])["model"], "stand-in");
    }

    #[test]
    fn a_leader_hires_once_and_assigns_the_person_s_last_paragraph() {
        let mut f = fake(LEADER);
        f.answers.insert(
            "hire".into(),
            Ok("claude-1 joined (Claude Code, haiku).".into()),
        );
        let mut m = Mind::default();
        let said = m.turn(&mut f, "From imece: notes\n\nWrite a greeting\nin French");
        assert_eq!(said, "claude-1 has it: Write a greeting");
        assert_eq!(m.hired, strings(&["claude-1"]));
        assert_eq!(f.said, strings(&["I gave claude-1 the work."]));
        assert_eq!(
            f.calls,
            vec![
                ("whoami".into(), json!({})),
                ("hire".into(), json!({})),
                (
                    "send".into(),
                    json!({ "to": "claude-1", "type": "assign", "body": "Write a greeting\nin French" })
                ),
            ]
        );
        m.turn(&mut f, "And another");
        assert_eq!(f.calls.len(), 4, "no second whoami, no second hire");
        assert_eq!(f.calls[3].1["body"], "And another");
    }

    #[test]
    fn a_leader_that_can_not_hire_or_send_says_so() {
        let mut f = fake(LEADER);
        f.answers
            .insert("hire".into(), Err("the pot is empty".into()));
        assert_eq!(
            Mind::default().turn(&mut f, "go"),
            "I could not hire: the pot is empty"
        );
        let mut f = fake(LEADER);
        f.answers
            .insert("hire".into(), Ok("claude-2 joined".into()));
        f.answers.insert("send".into(), Err("no such agent".into()));
        assert_eq!(
            Mind::default().turn(&mut f, "go"),
            "claude-2 did not get it: no such agent"
        );
    }

    #[test]
    fn a_leader_reads_its_mail_and_names_who_is_done() {
        let mut f = fake(LEADER);
        let mail = format!(
            "{MAIL} (written by those agents, not by the person):\n\n[done 4 · $0.02] from claude-1:\nwrote it\n\n[inform 5] from claude-2:\nhi\n\n[done] from claude-3:\nok"
        );
        let mut m = Mind::default();
        assert_eq!(m.turn(&mut f, &mail), "claude-1, claude-3 said done.");
        let none = format!("{MAIL}:\n\n[inform 5] from claude-2:\nhi");
        assert_eq!(m.turn(&mut f, &none), "Read my mail; nothing to do.");
        assert_eq!(f.calls.len(), 1, "only whoami");
    }

    #[test]
    fn a_worker_with_nothing_to_change_writes_its_file_and_tells_its_leader() {
        let mut f = fake(WORKER);
        f.tools = strings(&["Write"]);
        f.files.insert("data.json".into(), "{}".into());
        let mail = format!(
            "{MAIL}:\n\n[assign 3] from lead:\nWrite a greeting\nin French\n\n[inform 4] from lead:\nthanks"
        );
        let said = Mind::default().turn(&mut f, &mail);
        assert_eq!(said, "Done: wrote stand-in/claude-1.md.");
        assert_eq!(
            f.files["stand-in/claude-1.md"],
            "# Write a greeting\n\nWrite a greeting\nin French\n"
        );
        assert_eq!(f.used.len(), 1, "one write, no list");
        assert_eq!(
            f.calls[1],
            (
                "send".to_string(),
                json!({ "to": "lead", "type": "done", "body": "wrote stand-in/claude-1.md" })
            )
        );
    }

    #[test]
    fn a_worker_without_work_or_a_place_to_write_says_so() {
        let mut f = fake(WORKER);
        let mut m = Mind::default();
        assert_eq!(m.turn(&mut f, "hello"), "Nothing assigned to me.");
        let empty = format!("{MAIL}:\n\n[assign 3] from lead:\n\n[inform] from lead:\nx");
        assert_eq!(m.turn(&mut f, &empty), "Nothing assigned to me.");
        f.tools = strings(&["Write"]);
        f.refuse.insert("Write".into(), "read-only".into());
        let mail = format!("{MAIL}:\n\n[assign] from lead:\ndo it");
        assert_eq!(
            m.turn(&mut f, &mail),
            "I could not write stand-in/claude-1.md: read-only"
        );
        f.refuse.clear();
        f.answers.insert("send".into(), Err("gone".into()));
        assert_eq!(
            m.turn(&mut f, &mail),
            "wrote stand-in/claude-1.md, but lead did not hear: gone"
        );
    }

    #[test]
    fn others_and_a_session_without_a_bus_do_nothing() {
        let mut f = fake("name: r\nrole: a researcher\n");
        assert_eq!(
            Mind::default().turn(&mut f, "x"),
            "Nothing for me to do here."
        );
        let mut f = Fake::default();
        f.answers.insert("whoami".into(), Err("no bus".into()));
        let mut m = Mind::default();
        assert_eq!(m.turn(&mut f, "x"), "Nothing for me to do here.");
        assert!(m.me.is_empty());
    }

    #[test]
    fn the_task_is_the_last_paragraph_cut() {
        assert_eq!(task_of("a\n\n b \n\n\n"), "b");
        assert_eq!(task_of("  \n\n "), "Look around.");
        let long = "x".repeat(TASK_MAX + 5);
        assert_eq!(task_of(&long).chars().count(), TASK_MAX);
    }

    #[test]
    fn a_prompt_line_is_read_as_imece_writes_it() {
        assert_eq!(
            prompt_of(&crate::claude::prompt_line("hi\nthere")).as_deref(),
            Some("hi\nthere")
        );
        let blocks = r#"{"type":"user","message":{"content":[{"type":"text","text":"a"},{"type":"text","text":"b"}]}}"#;
        assert_eq!(prompt_of(blocks).as_deref(), Some("a\nb"));
        assert_eq!(prompt_of(r#"{"type":"control"}"#), None);
        assert_eq!(prompt_of("not json"), None);
    }

    /// The stand-in's whole stream, as imece's parser reads it.
    fn parsed(lines: &str, a: &Args) -> Vec<crate::Event> {
        let spec = crate::claude::Spec {
            program: "imece".into(),
            cwd: PathBuf::from("/work"),
            name: "imece lead".into(),
            tools: a.tools.clone(),
            confine: crate::Confine::Standard,
            write: true,
            model: None,
            effort: None,
            budget: 1_000_000,
            system: String::new(),
            mcp: None,
            resume: None,
            scratch: None,
            notes: None,
            store: None,
        };
        let mut p = crate::claude::Parser::new(&spec);
        lines
            .lines()
            .flat_map(|l| p.line(l).unwrap_or_else(|e| panic!("{e}: {l}")))
            .collect()
    }

    #[test]
    fn transcripts_go_under_the_config_dir_only_when_it_is_named() {
        assert_eq!(home_of(Some("/h".into())), Some(PathBuf::from("/h")));
        assert_eq!(home_of(Some("".into())), None);
        assert_eq!(home_of(None), None);
    }

    #[test]
    fn a_mind_is_kept_as_json_and_read_back_without_what_is_not_text() {
        let mind = Mind {
            me: BTreeMap::from([("role".into(), "the leader".into())]),
            hired: vec!["claude-1".into(), "claude-2".into()],
            worked: 3,
            waiting: Some(work::Waiting {
                task: "Round up".into(),
                list: vec![Some(6), None, Some(8)],
                helper: Some("claude-1.1".into()),
                said: Some("behind a flag".into()),
            }),
            turns: 4,
            failed: Some(Failed {
                prompt: "Round up".into(),
                account: true,
            }),
        };
        assert_eq!(Mind::from_json(&mind.to_json()), mind);
        let passing = Mind {
            failed: Some(Failed {
                prompt: "p".into(),
                account: false,
            }),
            ..mind.clone()
        };
        assert_eq!(Mind::from_json(&passing.to_json()), passing);
        let odd = json!({ "me": { "role": 3, "name": "x" }, "hired": ["a", 4], "worked": "2",
            "waiting": { "task": "t", "list": ["1", 2], "helper": 1, "said": ["x"] } });
        let read = Mind::from_json(&odd);
        assert_eq!(read.me, BTreeMap::from([("name".into(), "x".into())]));
        assert_eq!(read.hired, ["a"]);
        assert_eq!(read.worked, 0);
        assert_eq!((read.turns, read.failed), (0, None));
        let odd_failed = json!({ "turns": "3", "failed": { "prompt": "x", "account": "yes" } });
        let read = Mind::from_json(&odd_failed);
        assert_eq!(read.turns, 0);
        assert_eq!(
            read.failed,
            Some(Failed {
                prompt: "x".into(),
                account: false
            }),
            "only true is the account's"
        );
        assert_eq!(
            Mind::from_json(&json!({ "failed": { "prompt": 1, "account": true } })).failed,
            None,
            "no prompt, no failure"
        );
        let read = Mind::from_json(&odd);
        let waiting = read.waiting.unwrap();
        assert_eq!(
            (waiting.task.as_str(), waiting.list),
            ("t", vec![None, Some(2)])
        );
        assert_eq!((waiting.helper, waiting.said), (None, None));
        let no_task = json!({ "waiting": { "task": 1, "list": [1] } });
        assert_eq!(Mind::from_json(&no_task).waiting, None);
        assert_eq!(
            Mind {
                waiting: None,
                ..mind.clone()
            }
            .to_json()["waiting"],
            json!(null)
        );
        assert_eq!(
            Mind {
                failed: None,
                ..mind
            }
            .to_json()["failed"],
            json!(null)
        );
        assert_eq!(Mind::from_json(&json!(null)), Mind::default());
    }

    /// Runs a session on `args` in `cwd` with `prompts`, its transcripts
    /// under `home`: its session id and the costs its turns reported.
    fn session_in(home: &Path, cwd: &Path, args: &[&str], prompts: &[&str]) -> (String, Vec<u64>) {
        let a = Args::parse(&strings(args)).unwrap();
        let input: String = (prompts.iter())
            .map(|p| crate::claude::prompt_line(p) + "\n")
            .collect();
        let mut out = Vec::new();
        run(
            &a,
            cwd,
            Some(home),
            &Habits::default(),
            input.as_bytes(),
            &mut out,
        );
        let lines: Vec<Value> = (String::from_utf8(out).unwrap().lines())
            .map(|l| serde_json::from_str(l).unwrap())
            .collect();
        let session = lines[0]["session_id"].as_str().unwrap().to_string();
        let costs = (lines.iter())
            .filter(|l| l["type"] == "result")
            .map(|l| (l["total_cost_usd"].as_f64().unwrap() * 1_000_000.0).round() as u64)
            .collect();
        (session, costs)
    }

    #[test]
    fn a_prompt_is_echoed_as_taken_under_replay_and_a_command_never() {
        let lines = |args: &[&str]| -> Vec<Value> {
            let a = Args::parse(&strings(args)).unwrap();
            let input: String = (["one", "/clear", "two"].iter())
                .map(|p| crate::claude::prompt_line(p) + "\n")
                .collect();
            let mut out = Vec::new();
            run(
                &a,
                Path::new("/work"),
                None,
                &Habits::default(),
                input.as_bytes(),
                &mut out,
            );
            (String::from_utf8(out).unwrap().lines())
                .map(|l| serde_json::from_str(l).unwrap())
                .collect()
        };
        let echoed = |v: &[Value]| -> Vec<String> {
            (v.iter())
                .filter(|l| l["isReplay"] == true)
                .map(|l| l["message"]["content"].as_str().unwrap().to_string())
                .collect()
        };
        let all = lines(&["--replay-user-messages"]);
        assert_eq!(echoed(&all), ["one", "two"]);
        // Each echo comes before its turn's result.
        let at = |content: &str| all.iter().position(|l| l["message"]["content"] == content);
        let first_result = all.iter().position(|l| l["type"] == "result").unwrap();
        assert!(at("one").unwrap() < first_result);
        assert!(
            Args::parse(&strings(&["--replay-user-messages"]))
                .unwrap()
                .replay
        );
        assert!(echoed(&lines(&[])).is_empty());
        // imece's parser reads the echo as a prompt taken.
        let a = Args::parse(&strings(&[
            "--replay-user-messages",
            "--permission-mode",
            "acceptEdits",
        ]))
        .unwrap();
        let input = crate::claude::prompt_line("one") + "\n";
        let mut out = Vec::new();
        run(
            &a,
            Path::new("/work"),
            None,
            &Habits::default(),
            input.as_bytes(),
            &mut out,
        );
        let events = parsed(&String::from_utf8(out).unwrap(), &a);
        assert_eq!(
            events.iter().filter(|e| **e == crate::Event::Took).count(),
            1
        );
    }

    #[test]
    fn a_session_keeps_a_transcript_a_resume_and_a_fork_go_on_from() {
        let home = std::env::temp_dir().join(format!("imece-stand-in-home-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&home);
        let (here, there) = (Path::new("/work/here"), Path::new("/work/there"));
        let (old, costs) = session_in(&home, here, &[], &["one", "two"]);
        assert_eq!(costs, [TURN_COST, 2 * TURN_COST]);
        // Where Claude Code keeps its own: a line at the start, one a turn.
        let file = crate::claude::transcript(&home, here, &old);
        let lines = || std::fs::read_to_string(&file).unwrap().lines().count();
        assert_eq!(lines(), 3);
        let k = kept(&home, &old).unwrap();
        assert_eq!(k.spent, 2 * TURN_COST);
        let context = CONTEXT_START + 2 * (CONTEXT_TURN / 10 + CONTEXT_TURN);
        assert_eq!(k.context, context);
        // A resume reports the whole session's cost, as Claude Code's does,
        // and goes on in the same transcript.
        let (again, costs) = session_in(&home, here, &["--resume", &old], &["three"]);
        assert_eq!(again, old);
        assert_eq!(costs, [3 * TURN_COST]);
        assert_eq!(lines(), 5);
        assert_eq!(
            kept(&home, &old).unwrap().context,
            context + CONTEXT_TURN / 10 + CONTEXT_TURN
        );
        // A fork, in another directory: a transcript of its own there, the
        // old one as it was.
        let fork = "11111111-2222-4333-8444-555555555555";
        let args = ["--resume", &old, "--fork-session", "--session-id", fork];
        let (forked, costs) = session_in(&home, there, &args, &["four"]);
        assert_eq!(forked, fork);
        assert_eq!(costs, [4 * TURN_COST]);
        assert_eq!(lines(), 5);
        assert!(crate::claude::transcript(&home, there, fork).is_file());
        assert_eq!(kept(&home, fork).unwrap().spent, 4 * TURN_COST);
        // `/clear`: a new session from nothing, kept apart.
        let (_, costs) = session_in(&home, here, &["--resume", &old], &["/clear", "five"]);
        assert_eq!(costs, [0, TURN_COST]);
        assert_eq!(lines(), 6, "the old one took no more than its start");
        // One not found starts afresh; nothing is kept without a home.
        let none = "99999999-2222-4333-8444-555555555555";
        let (_, costs) = session_in(&home, here, &["--resume", none], &["six"]);
        assert_eq!(costs, [TURN_COST]);
        assert_eq!(kept(&home, "not-there"), None);
        std::fs::write(&file, "not json\n").unwrap();
        assert_eq!(kept(&home, &old), None, "a last line not understood");
        let _ = std::fs::remove_dir_all(&home);
    }

    #[test]
    fn a_session_streams_what_imece_s_parser_takes() {
        let a = Args {
            tools: strings(&["Read", "Write"]),
            permission_mode: "acceptEdits".into(),
            ..Args::default()
        };
        let input = format!(
            "{}\nnot a prompt\n{}\n{}\n",
            crate::claude::prompt_line("hello"),
            crate::claude::prompt_line("/clear"),
            crate::claude::prompt_line("again"),
        );
        let mut out = Vec::new();
        run(
            &a,
            Path::new("/work"),
            None,
            &Habits::default(),
            input.as_bytes(),
            &mut out,
        );
        let text = String::from_utf8(out).unwrap();
        let events = parsed(&text, &a);
        let done: Vec<(bool, String, u64)> = (events.iter())
            .filter_map(|e| match e {
                crate::Event::TurnDone { ok, text, cost, .. } => Some((*ok, text.clone(), *cost)),
                _ => None,
            })
            .collect();
        // Without a bus nobody says who it is: nothing to do, a turn's
        // cost each; `/clear` costs nothing and starts the counter again
        // (the parser keeps the total going up).
        let nothing = "Nothing for me to do here.".to_string();
        assert_eq!(
            done,
            vec![
                (true, nothing.clone(), TURN_COST),
                (true, String::new(), TURN_COST),
                (true, nothing.clone(), 2 * TURN_COST),
            ]
        );
        let started = (events.iter())
            .filter(|e| matches!(e, crate::Event::Started { .. }))
            .count();
        assert_eq!(started, 2, "the first init and the one /clear brings");
        let said = CONTEXT_START + CONTEXT_TURN / 10;
        let context: Vec<&crate::Event> = (events.iter())
            .filter(|e| matches!(e, crate::Event::Context { .. }))
            .collect();
        assert_eq!(
            context,
            [
                &crate::Event::Context {
                    used: said,
                    size: None
                },
                &crate::Event::Context {
                    used: said,
                    size: Some(WINDOW)
                },
                &crate::Event::Context {
                    used: 0,
                    size: Some(WINDOW)
                },
                &crate::Event::Context {
                    used: said,
                    size: Some(WINDOW)
                },
            ],
            "the turn's text, the window its result names, a cleared context"
        );
        assert!(events.contains(&crate::Event::Text(nothing)));
        assert!(
            !events
                .iter()
                .any(|e| matches!(e, crate::Event::Tool { .. })),
            "no tool of a bus it has not got"
        );
    }

    #[test]
    fn the_context_grows_by_a_turn_and_the_cost_adds_up() {
        let a = Args {
            permission_mode: "acceptEdits".into(),
            ..Args::default()
        };
        let input = format!(
            "{}\n{}\n",
            crate::claude::prompt_line("one"),
            crate::claude::prompt_line("two"),
        );
        let mut out = Vec::new();
        run(
            &a,
            Path::new("/work"),
            None,
            &Habits::default(),
            input.as_bytes(),
            &mut out,
        );
        let events = parsed(&String::from_utf8(out).unwrap(), &a);
        let said = CONTEXT_START + CONTEXT_TURN / 10;
        assert!(events.contains(&crate::Event::Context {
            used: said + CONTEXT_TURN + CONTEXT_TURN / 10,
            size: Some(WINDOW)
        }));
        let costs: Vec<u64> = (events.iter())
            .filter_map(|e| match e {
                crate::Event::TurnDone { cost, .. } => Some(*cost),
                _ => None,
            })
            .collect();
        assert_eq!(costs, [TURN_COST, 2 * TURN_COST]);
        // Its model and the background's, an eighth, each turn (record 134).
        let grew: Vec<Vec<(String, u64)>> = (events.iter())
            .filter_map(|e| match e {
                crate::Event::Models { grew, .. } => Some(grew.clone().into_iter().collect()),
                _ => None,
            })
            .collect();
        let turn = vec![
            (BACKGROUND.to_string(), TURN_COST / 8),
            ("stand-in".to_string(), TURN_COST - TURN_COST / 8),
        ];
        assert_eq!(grew, [turn.clone(), turn]);
    }

    #[test]
    fn after_a_failure_a_try_again_does_the_failed_prompt_and_other_words_come_after_it() {
        let passing = Failed {
            prompt: "Write a greeting".into(),
            account: false,
        };
        let account = Failed {
            account: true,
            ..passing.clone()
        };
        let mail = format!("{MAIL}:\n\n[done 3] from claude-1: ok");
        for f in [&passing, &account] {
            assert_eq!(f.then(crate::RETRY_PROMPT), ["Write a greeting"]);
            assert_eq!(f.then(&mail), ["Write a greeting", mail.as_str()]);
        }
        // The person's words: new work after a passing fault, the word to
        // go on after the account's.
        assert_eq!(
            passing.then("And in French"),
            ["Write a greeting", "And in French"]
        );
        assert_eq!(account.then("Paid it, go on"), ["Write a greeting"]);
        // Only imece's words are its try again.
        let near = format!("{} ", crate::RETRY_PROMPT);
        assert_eq!(passing.then(&near), ["Write a greeting", near.as_str()]);
    }

    #[test]
    fn a_failure_streams_claude_code_s_tries_then_an_api_error_and_spends_nothing() {
        let mut s = stream();
        s.spent = 20_000;
        let mut waited = 0;
        s.failure(crate::Fault::Passing, "s1", || waited += 1);
        let lines: Vec<Value> = (String::from_utf8(std::mem::take(&mut s.out)).unwrap())
            .lines()
            .map(|l| serde_json::from_str(l).unwrap())
            .collect();
        assert_eq!(waited, TRIES);
        assert_eq!(lines.len(), 4);
        let tries: Vec<(u64, u64)> = (lines[..3].iter())
            .map(|l| {
                assert_eq!(
                    (
                        &l["subtype"],
                        &l["max_retries"],
                        &l["error_status"],
                        &l["error"]
                    ),
                    (
                        &json!("api_retry"),
                        &json!(10),
                        &json!(529),
                        &json!("overloaded")
                    )
                );
                assert_eq!(l["session_id"], "s1");
                (
                    l["attempt"].as_u64().unwrap(),
                    l["retry_delay_ms"].as_u64().unwrap(),
                )
            })
            .collect();
        assert_eq!(tries, [(1, 500), (2, 1_000), (3, 2_000)]);
        let end = &lines[3];
        assert_eq!(
            (
                &end["type"],
                &end["subtype"],
                &end["is_error"],
                &end["terminal_reason"]
            ),
            (
                &json!("result"),
                &json!("success"),
                &json!(true),
                &json!("api_error")
            )
        );
        assert_eq!(
            (&end["api_error_status"], &end["result"]),
            (&json!(529), &json!(OVERLOADED))
        );
        assert_eq!(
            end["total_cost_usd"],
            json!(0.02),
            "what was spent before, no more"
        );
        assert_eq!(end["modelUsage"]["m"]["contextWindow"], json!(WINDOW));
        assert_eq!(s.spent, 20_000);
        // The account's: refused at once, no tries.
        s.failure(crate::Fault::Account, "s1", || waited += 1);
        let out = String::from_utf8(s.out).unwrap();
        let lines: Vec<Value> = out
            .lines()
            .map(|l| serde_json::from_str(l).unwrap())
            .collect();
        assert_eq!((lines.len(), waited), (1, TRIES));
        assert_eq!(
            (
                &lines[0]["api_error_status"],
                &lines[0]["result"],
                &lines[0]["is_error"]
            ),
            (&json!(400), &json!(NO_CREDIT), &json!(true))
        );
    }

    #[test]
    fn a_turn_fails_as_its_habits_say_and_the_one_after_a_failure_goes_through() {
        use crate::{Event, Fault};
        let name = "imece lead";
        let habits = |seed| Habits {
            faults: 1,
            seed,
            ..Habits::default()
        };
        // Seeds whose first and third turns fail on a passing fault, and
        // on the account's.
        let seed_of = |kind| {
            let fault = |s, n| habits(s).fault_now(work::roll(s, name, n));
            (0..1_000u64)
                .find(|&s| fault(s, 0) == Some(kind) && fault(s, 2) == Some(kind))
                .unwrap()
        };
        let run_on = |seed, home: &Path, prompts: &[&str]| {
            let a = Args::parse(&strings(&[
                "--name",
                name,
                "--permission-mode",
                "acceptEdits",
            ]))
            .unwrap();
            let input: String = (prompts.iter())
                .map(|p| crate::claude::prompt_line(p) + "\n")
                .collect();
            let mut out = Vec::new();
            let cwd = Path::new("/work");
            run(
                &a,
                cwd,
                Some(home),
                &habits(seed),
                input.as_bytes(),
                &mut out,
            );
            let session = (String::from_utf8(out.clone()).unwrap().lines())
                .find_map(|l| {
                    serde_json::from_str::<Value>(l).ok()?["session_id"]
                        .as_str()
                        .map(String::from)
                })
                .unwrap();
            (
                parsed(&String::from_utf8(out).unwrap(), &a),
                kept(home, &session).unwrap(),
            )
        };
        let home =
            std::env::temp_dir().join(format!("imece-stand-in-faults-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&home);
        let ends = |events: &[Event]| -> Vec<(bool, String, u64)> {
            (events.iter())
                .filter_map(|e| match e {
                    Event::TurnDone { ok, text, cost, .. } => Some((*ok, text.clone(), *cost)),
                    _ => None,
                })
                .collect()
        };
        let (events, k) = run_on(seed_of(Fault::Passing), &home, &["one", "two", "three"]);
        let failed = (false, OVERLOADED.to_string(), 0);
        let worked = (
            true,
            "Nothing for me to do here.\n\nNothing for me to do here.".to_string(),
            TURN_COST,
        );
        // Every turn fails but the one after a failure, which does the
        // failed prompt and the new one.
        assert_eq!(
            ends(&events),
            [
                failed.clone(),
                worked,
                (false, OVERLOADED.into(), TURN_COST)
            ]
        );
        let notes = (events.iter())
            .filter(|e| matches!(e, Event::Note(n) if n.contains("api error 529")))
            .count();
        assert_eq!(
            notes,
            2 * TRIES as usize,
            "Claude Code's own tries, as trace notes"
        );
        assert_eq!(
            events
                .iter()
                .filter(|e| **e == Event::Failed(Fault::Passing))
                .count(),
            2
        );
        // The transcript keeps the failed prompt for a resume.
        assert_eq!(k.mind.turns, 3);
        assert_eq!(
            k.mind.failed,
            Some(Failed {
                prompt: "three".into(),
                account: false
            })
        );
        let (events, k) = run_on(seed_of(Fault::Account), &home, &["one"]);
        assert_eq!(ends(&events), [(false, NO_CREDIT.to_string(), 0)]);
        assert!(events.contains(&Event::Failed(Fault::Account)));
        assert!(
            !events
                .iter()
                .any(|e| matches!(e, Event::Note(n) if n.contains("api error")))
        );
        assert!(k.mind.failed.is_some_and(|f| f.account));
        // Without the habit, nothing fails.
        let a = Args::parse(&strings(&[
            "--name",
            name,
            "--permission-mode",
            "acceptEdits",
        ]))
        .unwrap();
        let mut out = Vec::new();
        let input = crate::claude::prompt_line("one") + "\n";
        run(
            &a,
            Path::new("/work"),
            None,
            &Habits::default(),
            input.as_bytes(),
            &mut out,
        );
        assert_eq!(
            ends(&parsed(&String::from_utf8(out).unwrap(), &a)),
            [(true, "Nothing for me to do here.".to_string(), TURN_COST)]
        );
        let _ = std::fs::remove_dir_all(&home);
    }

    #[test]
    fn a_session_on_the_background_s_model_says_its_whole_cost_as_that_model_s() {
        let mut s = stream();
        s.spent = 16_000;
        let usage = s.usage();
        assert_eq!(usage["m"]["costUSD"], json!(0.014));
        assert_eq!(usage[BACKGROUND]["costUSD"], json!(0.002));
        assert_eq!(usage["m"]["contextWindow"], json!(WINDOW));
        s.model = BACKGROUND.into();
        let usage = s.usage();
        assert_eq!(usage.as_object().unwrap().len(), 1);
        assert_eq!(usage[BACKGROUND]["costUSD"], json!(0.016));
        assert_eq!(usage[BACKGROUND]["contextWindow"], json!(WINDOW));
    }

    fn stream() -> Stream<Vec<u8>> {
        Stream {
            out: Vec::new(),
            spent: 0,
            context: 0,
            calls: 0,
            tasks: 0,
            model: "m".into(),
        }
    }

    #[test]
    fn its_hands_write_in_its_directory_and_stream_the_call() {
        let dir = std::env::temp_dir().join(format!("imece-stand-in-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let mut stream = stream();
        let (args, tools, habits) = (Args::default(), strings(&["Write"]), Habits::default());
        let mut hands = Busy {
            stream: &mut stream,
            servers: &mut [],
            cwd: &dir,
            args: &args,
            tools: &tools,
            habits: &habits,
        };
        let write = |h: &mut Busy<_>, path: &str, text: &str| {
            h.tool("Write", json!({ "file_path": path, "content": text }))
        };
        write(&mut hands, "a/b.md", "hi").unwrap();
        assert_eq!(std::fs::read_to_string(dir.join("a/b.md")).unwrap(), "hi");
        std::fs::write(dir.join("file"), "").unwrap();
        assert!(
            write(&mut hands, "file/x", "no").is_err(),
            "a file is no directory"
        );
        hands.say("words");
        let text = String::from_utf8(stream.out.clone()).unwrap();
        let lines: Vec<Value> = text
            .lines()
            .map(|l| serde_json::from_str(l).unwrap())
            .collect();
        assert_eq!(lines.len(), 5, "two calls of two lines, one text");
        assert_eq!(lines[0]["message"]["content"][0]["name"], "Write");
        assert_eq!(
            lines[0]["message"]["content"][0]["input"]["file_path"],
            dir.join("a/b.md").to_string_lossy().as_ref()
        );
        assert_eq!(lines[1]["message"]["content"][0]["tool_use_id"], "toolu_1");
        assert_eq!(lines[1]["message"]["content"][0]["is_error"], false);
        assert_eq!(lines[3]["message"]["content"][0]["is_error"], true);
        assert_eq!(lines[4]["message"]["content"][0]["text"], "words");
        assert_eq!(stream.spent, 2 * CALL_COST);
        assert_eq!(stream.calls, 2);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn install_writes_a_program_that_runs_imece_s_stand_in() {
        use std::os::unix::fs::PermissionsExt;
        let dir = std::env::temp_dir().join(format!("imece-stand-in-at-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        // An "imece" with a quote and a space in its path, that says what
        // it was given.
        let odd = dir.join("it's here");
        std::fs::create_dir_all(&odd).unwrap();
        let imece = odd.join("imece");
        std::fs::write(
            &imece,
            "#!/bin/sh\nprintf '%s|' \"$CLAUDE_CONFIG_DIR\" \"$@\"\n",
        )
        .unwrap();
        std::fs::set_permissions(&imece, std::fs::Permissions::from_mode(0o700)).unwrap();
        let home = odd.join("it's home");
        let habits = Habits {
            pause: std::time::Duration::from_millis(250),
            asks: 3,
            questions: 5,
            helpers: 6,
            faults: 8,
            seed: 7,
        };
        let claude = install(&dir, &imece, &home, &habits).unwrap();
        assert_eq!(claude, dir.join("claude"));
        let mode = std::fs::metadata(&claude).unwrap().permissions().mode();
        assert_eq!(mode & 0o777, 0o700);
        let out = Command::new(&claude).args(["-p", "a b"]).output().unwrap();
        assert_eq!(
            String::from_utf8_lossy(&out.stdout),
            format!("{}|stand-in|-p|a b|", home.display())
        );
        // Its habits reach the stand-in.
        std::fs::write(
            &imece,
            "#!/bin/sh\nprintf '%s|' \"$IMECE_STAND_IN_PAUSE_MS\" \"$IMECE_STAND_IN_ASKS\" \"$IMECE_STAND_IN_QUESTIONS\" \"$IMECE_STAND_IN_HELPERS\" \"$IMECE_STAND_IN_FAULTS\" \"$IMECE_STAND_IN_SEED\"\n",
        )
        .unwrap();
        let out = Command::new(&claude).output().unwrap();
        assert_eq!(String::from_utf8_lossy(&out.stdout), "250|3|5|6|8|7|");
        let _ = std::fs::remove_dir_all(&dir);
    }

    fn said(args: &[&str], input: &str) -> (i32, String) {
        let mut out = Vec::new();
        let code = answer(
            &strings(args),
            Path::new("/"),
            None,
            &Habits::default(),
            input.as_bytes(),
            &mut out,
        );
        (code, String::from_utf8(out).unwrap())
    }

    #[test]
    fn it_answers_for_its_version_and_its_login_and_runs_a_session_else() {
        assert_eq!(said(&["--version"], ""), (0, format!("{VERSION}\n")));
        assert_eq!(said(&["-v"], ""), (0, format!("{VERSION}\n")));
        let (code, login) = said(&["auth", "status", "--json"], "");
        assert_eq!(code, 0);
        let login: Value = serde_json::from_str(&login).unwrap();
        assert_eq!(login["loggedIn"], true);
        // Three words that are not those three, or one that is not the
        // version: a session, whose first line is its `init`.
        for args in [
            &["--verbose", "status", "--json"][..],
            &["auth", "--verbose", "--json"],
            &["auth", "status", "--verbose"],
            &["--verbose"],
        ] {
            let (code, out) = said(args, "");
            assert_eq!(code, 0);
            let first: Value = serde_json::from_str(out.lines().next().unwrap()).unwrap();
            assert_eq!(first["subtype"], "init", "{args:?}");
        }
        assert_eq!(said(&["--tools"], "").0, 2, "a line it can not read");
        assert_eq!(said(&["--tools"], "").1, "");
    }

    /// Runs `f` on a thread of its own and fails when it takes longer
    /// than `WITHIN`: a scripted server that waits on a line the stand-in
    /// did not send, while the stand-in waits on its answer, fails the
    /// test instead of holding it (a mutant of the exchange would).
    fn within<T: Send + 'static>(f: impl FnOnce() -> T + Send + 'static) -> T {
        const WITHIN: std::time::Duration = std::time::Duration::from_secs(10);
        let (tx, rx) = std::sync::mpsc::channel();
        std::thread::spawn(move || {
            let _ = tx.send(f());
        });
        match rx.recv_timeout(WITHIN) {
            Ok(t) => t,
            Err(std::sync::mpsc::RecvTimeoutError::Timeout) => {
                panic!("the exchange with the server did not end")
            }
            // The thread panicked: its message is above.
            Err(std::sync::mpsc::RecvTimeoutError::Disconnected) => panic!("it failed"),
        }
    }

    /// A server that answers `initialize` and `tools/list` with tool
    /// `echo`, then each call with `body` (a shell line that reads the
    /// call and writes its answer), in a directory of its own.
    fn script_server(name: &str, body: &str) -> (PathBuf, PathBuf) {
        use std::os::unix::fs::PermissionsExt;
        let dir =
            std::env::temp_dir().join(format!("imece-stand-in-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let script = dir.join("server");
        let text = format!(
            "#!/bin/sh\nread -r l\necho '{{\"jsonrpc\":\"2.0\",\"id\":1,\"result\":{{}}}}'\nread -r l\nread -r l\necho '{{\"jsonrpc\":\"2.0\",\"id\":2,\"result\":{{\"tools\":[{{\"name\":\"echo\"}}]}}}}'\n{body}\n"
        );
        std::fs::write(&script, text).unwrap();
        std::fs::set_permissions(&script, std::fs::Permissions::from_mode(0o700)).unwrap();
        (dir, script)
    }

    #[test]
    fn a_server_that_would_not_end_is_ended_with_the_stand_in() {
        within(move || {
            let (dir, script) = script_server("stays", "sleep 30");
            let s = Server::start(&script.to_string_lossy(), &[], &dir);
            assert!(s.connected());
            let pid = s.pid().unwrap() as libc::pid_t;
            assert_eq!(unsafe { libc::kill(pid, 0) }, 0, "it runs");
            drop(s);
            assert_eq!(unsafe { libc::kill(pid, 0) }, -1, "killed and reaped");
            assert!(Server::listing(&[]).pid().is_none());
            let _ = std::fs::remove_dir_all(&dir);
        });
    }

    #[test]
    fn the_bus_is_the_server_named_imece_and_its_calls_are_streamed() {
        within(move || {
            let answer = r#"read -r l; echo '{"jsonrpc":"2.0","id":3,"result":{"content":[{"type":"text","text":"name: lead"}]}}'; read -r l"#;
            let (dir, script) = script_server("bus", answer);
            let mut servers = vec![
                ("other".to_string(), Server::listing(&["whoami"])),
                (
                    crate::claude::MCP_SERVER.to_string(),
                    Server::start(&script.to_string_lossy(), &[], &dir),
                ),
            ];
            let mut stream = stream();
            let (args, habits) = (Args::default(), Habits::default());
            let mut hands = Busy {
                stream: &mut stream,
                servers: &mut servers,
                cwd: &dir,
                args: &args,
                tools: &[],
                habits: &habits,
            };
            assert_eq!(
                hands.call("whoami", json!({ "a": 1 })),
                Ok("name: lead".into())
            );
            let text = String::from_utf8(stream.out.clone()).unwrap();
            let lines: Vec<Value> = text
                .lines()
                .map(|l| serde_json::from_str(l).unwrap())
                .collect();
            let call = &lines[0]["message"]["content"][0];
            assert_eq!(call["name"], "mcp__imece__whoami");
            assert_eq!(call["input"], json!({ "a": 1 }));
            assert_eq!(lines[1]["message"]["content"][0]["content"], "name: lead");
            let mut alone = vec![("other".to_string(), Server::listing(&["whoami"]))];
            let mut hands = Busy {
                stream: &mut stream,
                servers: &mut alone,
                cwd: &dir,
                args: &args,
                tools: &[],
                habits: &habits,
            };
            assert_eq!(
                hands.call("whoami", json!({})),
                Err("no imece MCP server".into())
            );
            assert_eq!(stream.calls, 1, "nothing streamed without the bus");
            drop(servers);
            let _ = std::fs::remove_dir_all(&dir);
        });
    }

    #[test]
    fn a_tool_that_needs_a_yes_asks_the_prompt_tool_hidden_from_the_stream() {
        within(move || {
            let answer = |id: u32, text: &str| {
                format!(
                    r#"read -r l; echo "$l" >> asked.txt; echo '{{"jsonrpc":"2.0","id":{id},"result":{{"content":[{{"type":"text","text":"{text}"}}]}}}}'"#
                )
            };
            let body = [
                answer(3, r#"{\"behavior\":\"allow\",\"updatedInput\":{}}"#),
                answer(4, r#"{\"behavior\":\"deny\",\"message\":\"no thanks\"}"#),
                answer(5, "not json"),
                answer(6, r#"{\"behavior\":\"deny\"}"#),
                "read -r l".to_string(),
            ]
            .join("\n");
            let (dir, script) = script_server("approve", &body);
            let mut servers = vec![(
                crate::claude::MCP_SERVER.to_string(),
                Server::start(&script.to_string_lossy(), &[], &dir),
            )];
            let mut stream = stream();
            let args = Args {
                prompt_tool: Some("mcp__imece__approve".into()),
                allowed: strings(&["WebFetch"]),
                ..Args::default()
            };
            let tools = strings(&["Bash", "WebFetch", "WebSearch", "Read", "Edit"]);
            let habits = Habits::default();
            let mut hands = Busy {
                stream: &mut stream,
                servers: &mut servers,
                cwd: &dir,
                args: &args,
                tools: &tools,
                habits: &habits,
            };
            assert!(hands.needs_yes("Bash") && hands.needs_yes("WebSearch"));
            assert!(!hands.needs_yes("WebFetch"), "allowed outright");
            assert!(!hands.needs_yes("Read") && !hands.needs_yes("Edit"));
            let bash = |h: &mut Busy<_>, c: &str| h.tool("Bash", json!({ "command": c }));
            assert_eq!(bash(&mut hands, "make test"), Ok(work::PASSED.into()));
            assert_eq!(bash(&mut hands, "make"), Err("no thanks".into()));
            let refused = NOT_GRANTED.replace("{}", "Bash");
            assert_eq!(
                bash(&mut hands, "make"),
                Err(refused.clone()),
                "not understood"
            );
            assert_eq!(bash(&mut hands, "make"), Err(refused.clone()), "no words");
            drop(servers);
            let asked: Vec<Value> = (std::fs::read_to_string(dir.join("asked.txt")).unwrap())
                .lines()
                .map(|l| serde_json::from_str(l).unwrap())
                .collect();
            assert_eq!(asked.len(), 4);
            assert_eq!(
                asked[0]["params"],
                json!({ "name": "approve", "arguments": {
                    "tool_name": "Bash", "input": { "command": "make test" }, "tool_use_id": "toolu_1",
                }})
            );
            assert_eq!(asked[1]["params"]["arguments"]["tool_use_id"], "toolu_2");
            // The stream has the calls and their answers, not the asking.
            let lines: Vec<Value> = (String::from_utf8(stream.out.clone()).unwrap())
                .lines()
                .map(|l| serde_json::from_str(l).unwrap())
                .collect();
            assert_eq!(lines.len(), 8);
            assert_eq!(lines[0]["message"]["content"][0]["name"], "Bash");
            assert_eq!(lines[1]["message"]["content"][0]["is_error"], false);
            assert_eq!(lines[3]["message"]["content"][0]["content"], "no thanks");
            assert_eq!(lines[3]["message"]["content"][0]["is_error"], true);
            assert_eq!(stream.spent, 4 * CALL_COST);
            // Nobody to ask: Claude Code's words, nothing asked.
            for prompt in [None, Some("approve"), Some("mcp__imece")] {
                let args = Args {
                    prompt_tool: prompt.map(String::from),
                    ..Args::default()
                };
                let mut hands = Busy {
                    stream: &mut stream,
                    servers: &mut [],
                    cwd: &dir,
                    args: &args,
                    tools: &tools,
                    habits: &habits,
                };
                assert_eq!(bash(&mut hands, "x"), Err(refused.clone()), "{prompt:?}");
                let web = hands.tool("WebSearch", json!({ "query": "q" }));
                assert_eq!(web, Err(NOT_GRANTED.replace("{}", "WebSearch")));
            }
            let args = Args {
                prompt_tool: Some("mcp__other__approve".into()),
                ..Args::default()
            };
            let mut hands = Busy {
                stream: &mut stream,
                servers: &mut [],
                cwd: &dir,
                args: &args,
                tools: &tools,
                habits: &habits,
            };
            assert_eq!(bash(&mut hands, "x"), Err("no other MCP server".into()));
            let _ = std::fs::remove_dir_all(&dir);
        });
    }

    #[test]
    fn a_worker_s_session_works_with_its_tools_and_habits() {
        within(move || {
            // `printf`, not `echo`: macOS's sh reads `\n` in an echo.
            let answer = |id: u32, text: &str| {
                format!(
                    r#"read -r l; printf '%s\n' '{{"jsonrpc":"2.0","id":{id},"result":{{"content":[{{"type":"text","text":"{text}"}}]}}}}'"#
                )
            };
            let whoami = WORKER.replace('\n', "\\n");
            let body = [answer(3, &whoami), answer(4, "sent"), "read -r l".into()].join("\n");
            let (dir, script) = script_server("session", &body);
            let cwd = dir.join("tree");
            std::fs::create_dir_all(&cwd).unwrap();
            std::fs::write(cwd.join("lib.rs"), "fn a() {}\n").unwrap();
            let config = json!({ "mcpServers": { "imece": {
                "type": "stdio", "command": script.to_string_lossy(), "args": [],
            }}});
            let a = Args::parse(&strings(&[
                "--tools",
                "default",
                "--mcp-config",
                &config.to_string(),
            ]))
            .unwrap();
            let input =
                crate::claude::prompt_line(&format!("{MAIL}:\n\n[assign 1] from lead:\nTidy lib"));
            let habits = Habits {
                pause: std::time::Duration::from_millis(5),
                ..Habits::default()
            };
            let mut out = Vec::new();
            let start = std::time::Instant::now();
            run(
                &a,
                &cwd,
                None,
                &habits,
                format!("{input}\n").as_bytes(),
                &mut out,
            );
            assert!(start.elapsed() >= 6 * habits.pause, "a pause a step");
            let text = String::from_utf8(out).unwrap();
            let named: Vec<String> = (text.lines())
                .map(|l| serde_json::from_str::<Value>(l).unwrap())
                .filter_map(|v| {
                    v["message"]["content"][0]["name"]
                        .as_str()
                        .map(String::from)
                })
                .collect();
            let creates = named.iter().filter(|n| *n == "TaskCreate").count();
            assert_eq!(creates, 5, "{named:?}");
            assert!(named.contains(&"Edit".to_string()));
            assert!(named.contains(&"Bash".to_string()), "cargo test");
            assert_eq!(
                std::fs::read_to_string(cwd.join("lib.rs")).unwrap(),
                "fn a() {}\n// Tidy lib\n"
            );
            let _ = std::fs::remove_dir_all(&dir);
        });
    }

    #[test]
    fn its_own_tools_work_on_its_directory_only() {
        let dir = std::env::temp_dir().join(format!("imece-stand-in-own-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(dir.join("src")).unwrap();
        std::fs::write(dir.join("src/a.rs"), "one\ntwo\n").unwrap();
        let mut stream = stream();
        let args = Args::default();
        let tools = strings(&["Read", "Edit", "TaskCreate", "TaskUpdate", "Glob", "Bash"]);
        let habits = Habits {
            pause: std::time::Duration::from_millis(40),
            ..Habits::default()
        };
        let mut hands = Busy {
            stream: &mut stream,
            servers: &mut [],
            cwd: &dir,
            args: &args,
            tools: &tools,
            habits: &habits,
        };
        let read = hands.tool("Read", json!({ "file_path": "src/a.rs" }));
        assert_eq!(read, Ok(work::numbered("one\ntwo\n")));
        let edit = |h: &mut Busy<_>, path: &str, old: &str| {
            h.tool(
                "Edit",
                json!({ "file_path": path, "old_string": old, "new_string": "2" }),
            )
        };
        let full = dir.join("src/a.rs");
        assert_eq!(
            edit(&mut hands, "src/a.rs", "two"),
            Ok(format!(
                "The file {} has been updated successfully.",
                full.display()
            ))
        );
        assert_eq!(std::fs::read_to_string(&full).unwrap(), "one\n2\n");
        assert_eq!(
            edit(&mut hands, &full.to_string_lossy(), "nine"),
            Err("String to replace not found in file.".into()),
            "a whole path inside"
        );
        let outside = |p: &str| Err(format!("{p} is outside the session's directory"));
        let up = dir.join("../x").to_string_lossy().into_owned();
        assert_eq!(edit(&mut hands, "../x", "a"), outside(&up));
        assert_eq!(edit(&mut hands, "/etc/hosts", "a"), outside("/etc/hosts"));
        assert_eq!(
            hands
                .tool("Read", json!({ "file_path": "src/none" }))
                .unwrap_err(),
            "No such file or directory (os error 2)"
        );
        // Its tasks by number, a session's own.
        let task = |h: &mut Busy<_>, s: &str| h.tool("TaskCreate", json!({ "subject": s }));
        assert_eq!(
            task(&mut hands, "a"),
            Ok("Task #1 created successfully: a".into())
        );
        assert_eq!(
            task(&mut hands, "b"),
            Ok("Task #2 created successfully: b".into())
        );
        assert_eq!(
            hands.tool(
                "TaskUpdate",
                json!({ "taskId": "2", "status": "completed" })
            ),
            Ok("Updated task #2 status".into())
        );
        // The shell is made up, and allowed outright here.
        let args_yes = Args {
            allowed: strings(&["Bash"]),
            ..Args::default()
        };
        hands.args = &args_yes;
        assert_eq!(
            hands.tool("Bash", json!({ "command": "rm -rf /" })),
            Ok(work::RAN.into())
        );
        assert_eq!(
            hands.tool("Glob", json!({})),
            Err("the stand-in does not run Glob".into())
        );
        let calls = hands.stream.calls;
        assert_eq!(
            hands.tool("Grep", json!({})),
            Err("No such tool available: Grep".into())
        );
        assert_eq!(
            hands.stream.calls, calls,
            "a tool it has not got is not streamed"
        );
        assert!(hands.has("Glob") && !hands.has("Grep"));
        assert_eq!(hands.files(), ["src/a.rs"]);
        assert_eq!(hands.habits(), &habits);
        let start = std::time::Instant::now();
        hands.pause();
        assert!(start.elapsed() >= habits.pause);
        // The stream names each file by its whole path.
        let lines: Vec<Value> = (String::from_utf8(stream.out.clone()).unwrap())
            .lines()
            .map(|l| serde_json::from_str(l).unwrap())
            .collect();
        assert_eq!(
            lines[0]["message"]["content"][0]["input"]["file_path"],
            full.to_string_lossy().as_ref()
        );
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn a_server_that_does_not_start_is_failed() {
        let s = Server::start("/nonexistent/server", &[], Path::new("/"));
        assert!(!s.connected());
        let mut s = Server::start("/usr/bin/true", &[], Path::new("/"));
        assert!(!s.connected(), "one that ends at once answers nothing");
        // Gone at the write or at the read, whichever comes first.
        let why = s.call("send", &json!({})).unwrap_err();
        assert!(why.starts_with("the server is gone"), "{why}");
    }

    #[test]
    fn a_server_is_asked_its_tools_and_called_over_stdio() {
        within(move || {
            // A server that answers in order, with a notification and an
            // answer to another id first, as a server may send them: the
            // stand-in waits for the answer to its own request.
            use std::os::unix::fs::PermissionsExt;
            let dir =
                std::env::temp_dir().join(format!("imece-stand-in-mcp-{}", std::process::id()));
            let _ = std::fs::remove_dir_all(&dir);
            std::fs::create_dir_all(&dir).unwrap();
            let script = dir.join("server");
            let body = r#"#!/bin/sh
read -r l; echo "$l" > asked.txt
echo '{"jsonrpc":"2.0","method":"notifications/message"}'
echo 'not json'
echo '{"jsonrpc":"2.0","id":1,"result":{}}'
read -r l; echo "$l" >> asked.txt
read -r l; echo "$l" >> asked.txt
echo '{"jsonrpc":"2.0","id":9,"result":{"tools":[]}}'
echo '{"jsonrpc":"2.0","id":2,"result":{"tools":[{"name":"echo"},{"name":"approve"}]}}'
read -r l; echo "$l" >> asked.txt
echo '{"jsonrpc":"2.0","id":3,"result":{"content":[{"type":"text","text":"a"},{"type":"text","text":"b"}],"isError":false}}'
read -r l
echo '{"jsonrpc":"2.0","id":4,"result":{"content":[{"type":"text","text":"no"}],"isError":true}}'
read -r l
echo '{"jsonrpc":"2.0","id":5,"error":{"code":-32601,"message":"method not found"}}'
read -r l
echo '{"jsonrpc":"2.0","id":6,"error":{}}'
"#;
            std::fs::write(&script, body).unwrap();
            std::fs::set_permissions(&script, std::fs::Permissions::from_mode(0o700)).unwrap();
            let mut s = Server::start(&script.to_string_lossy(), &[], &dir);
            assert_eq!(s.tools, strings(&["echo", "approve"]));
            assert!(s.connected());
            assert_eq!(s.call("echo", &json!({ "word": "hi" })), Ok("a\nb".into()));
            assert_eq!(s.call("echo", &json!({})), Err("no".into()));
            assert_eq!(s.call("echo", &json!({})), Err("method not found".into()));
            assert_eq!(s.call("echo", &json!({})), Err("error".into()));
            assert_eq!(s.call("echo", &json!({})), Err("the server is gone".into()));
            drop(s);
            let asked = std::fs::read_to_string(dir.join("asked.txt")).unwrap();
            let asked: Vec<Value> = asked
                .lines()
                .map(|l| serde_json::from_str(l).unwrap())
                .collect();
            assert_eq!(asked[0]["method"], "initialize");
            assert_eq!(asked[0]["id"], 1);
            assert_eq!(asked[0]["params"]["protocolVersion"], "2025-06-18");
            assert_eq!(
                asked[1],
                json!({ "jsonrpc": "2.0", "method": "notifications/initialized" })
            );
            assert_eq!(asked[2]["method"], "tools/list");
            assert_eq!(asked[3]["method"], "tools/call");
            assert_eq!(
                asked[3]["params"],
                json!({ "name": "echo", "arguments": { "word": "hi" } })
            );
            let _ = std::fs::remove_dir_all(&dir);
        });
    }
}
