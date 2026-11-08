//! A stand-in for Claude Code (record 130): `imece stand-in ARGS…` takes
//! the command line imece gives `claude` ([`crate::claude::args`]), speaks
//! stream-json on stdin and stdout as Claude Code does, reaches the MCP
//! servers its `--mcp-config` names as their client (imece's bus, through
//! `imece mcp-relay`), and does a small, fixed piece of work per role: a
//! leader hires a worker and assigns it the person's words, a worker
//! writes one file in its directory and says done. It asks no model and
//! spends nothing; the cost it reports is made up, the same every run.
//!
//! It is what the demo will run on the real `Live` (record 111), and what
//! tests run the real `Live` on end to end. It is no sandbox and checks
//! nothing: imece's checks of what a session reports run on it as on
//! Claude Code.

use std::collections::BTreeMap;
use std::io::{self, BufRead, BufReader, Write};
use std::path::{Path, PathBuf};
use std::process::{Child, ChildStdin, ChildStdout, Command, Stdio};

use serde_json::{Value, json};

use crate::claude::{EDIT_TOOLS, SHELL_TOOL, SUBAGENT_TOOLS, TASK_TOOLS, WEB_TOOLS};

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

/// The longest task a leader hands on, in characters.
pub const TASK_MAX: usize = 200;

/// The directory a worker writes its file in, inside its own.
pub const OUT_DIR: &str = "stand-in";

/// The stand-in as a program: `args` without the program's name; what it
/// exits with.
pub fn main(args: &[String]) -> i32 {
    let cwd = std::env::current_dir().unwrap_or_default();
    answer(args, &cwd, io::stdin().lock(), io::stdout().lock())
}

/// What the stand-in does with `args` in `cwd`, `input` its stdin and
/// `out` its stdout; what it exits with.
pub fn answer(args: &[String], cwd: &Path, input: impl BufRead, mut out: impl Write) -> i32 {
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
                run(&a, cwd, input, out);
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
/// arguments, `imece` being imece's own binary: what a session's program
/// is (one path) when the stand-in plays Claude Code. Gives its path.
pub fn install(dir: &Path, imece: &Path) -> io::Result<PathBuf> {
    use std::os::unix::fs::PermissionsExt;
    let quoted = format!("'{}'", imece.to_string_lossy().replace('\'', r"'\''"));
    let path = dir.join("claude");
    std::fs::write(&path, format!("#!/bin/sh\nexec {quoted} stand-in \"$@\"\n"))?;
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
                "--permission-mode" => a.permission_mode = value()?,
                "--permission-prompt-tool" => a.prompt_tool = Some(value()?),
                "--model" => a.model = Some(value()?),
                "--resume" => a.resume = Some(value()?),
                "--session-id" => a.session_id = Some(value()?),
                "--fork-session" => a.fork = true,
                "--mcp-config" => a.servers = servers_of(&value()?)?,
                // Flags that take a value the stand-in does not read.
                "--input-format"
                | "--output-format"
                | "--permission-prompts"
                | "--name"
                | "--add-dir"
                | "--append-system-prompt"
                | "--system-prompt"
                | "--max-budget-usd"
                | "--allowedTools"
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
        writeln!(to, "{line}")
            .and_then(|()| to.flush())
            .map_err(|e| format!("the server is gone: {e}"))?;
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

/// What the stand-in's mind works with: the tools of imece's bus, the
/// files of its directory, and lines of text to say.
pub trait Hands {
    /// Calls the bus's tool `tool` (its bare name).
    fn call(&mut self, tool: &str, args: Value) -> Result<String, String>;
    /// Writes `text` to `path`, relative to the session's directory.
    fn write(&mut self, path: &str, text: &str) -> Result<(), String>;
    fn say(&mut self, text: &str);
}

/// What the stand-in has learned in its session.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Mind {
    /// `whoami`'s answer, by its `key: value` lines; asked once.
    pub me: BTreeMap<String, String>,
    /// The workers a leader hired.
    pub hired: Vec<String>,
}

impl Mind {
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

    fn work(&mut self, hands: &mut impl Hands, prompt: &str) -> String {
        let Some(task) = body_of(prompt, "assign") else {
            return "Nothing assigned to me.".into();
        };
        let me = self.me.get("name").cloned().unwrap_or_default();
        let path = format!("{OUT_DIR}/{me}.md");
        if let Err(why) = hands.write(&path, &format!("# {}\n\n{task}\n", first_line(&task))) {
            return format!("I could not write {path}: {why}");
        }
        let lead = (self.me.get("you answer to"))
            .and_then(|o| o.strip_prefix("the leader "))
            .unwrap_or_default()
            .to_string();
        let body = format!("wrote {path}");
        match hands.call("send", json!({ "to": lead, "type": "done", "body": body })) {
            Ok(_) => format!("Done: {body}."),
            Err(why) => format!("{body}, but {lead} did not hear: {why}"),
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

    fn tool(&mut self, name: &str, input: &Value, said: &Result<String, String>) {
        self.calls += 1;
        let id = format!("toolu_{}", self.calls);
        self.assistant(json!([{ "type": "tool_use", "id": id, "name": name, "input": input }]));
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
            "modelUsage": { self.model.clone(): { "contextWindow": WINDOW } },
        }));
    }
}

/// The hands of a running session: its bus, its directory, its stream.
struct Busy<'a, W: Write> {
    stream: &'a mut Stream<W>,
    servers: &'a mut [(String, Server)],
    cwd: &'a Path,
}

impl<W: Write> Hands for Busy<'_, W> {
    fn call(&mut self, tool: &str, args: Value) -> Result<String, String> {
        // A session without imece's server has none of its tools to call.
        let bus = self
            .servers
            .iter_mut()
            .find(|(n, _)| n == crate::claude::MCP_SERVER);
        let Some((_, server)) = bus else {
            return Err("no imece MCP server".into());
        };
        let said = server.call(tool, &args);
        self.stream
            .tool(&mcp_name(crate::claude::MCP_SERVER, tool), &args, &said);
        said
    }

    fn write(&mut self, path: &str, text: &str) -> Result<(), String> {
        let full: PathBuf = self.cwd.join(path);
        let input = json!({ "file_path": full.to_string_lossy(), "content": text });
        let done = full
            .parent()
            .map_or(Ok(()), std::fs::create_dir_all)
            .and_then(|()| std::fs::write(&full, text))
            .map(|()| format!("File created successfully at: {}", full.display()))
            .map_err(|e| e.to_string());
        self.stream.tool("Write", &input, &done);
        done.map(|_| ())
    }

    fn say(&mut self, text: &str) {
        self.stream
            .assistant(json!([{ "type": "text", "text": text }]));
    }
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

/// A session: its servers started in `cwd`, `init`, then a turn per
/// prompt until stdin ends.
pub fn run(a: &Args, cwd: &Path, input: impl BufRead, out: impl Write) {
    let mut servers: Vec<(String, Server)> = (a.servers.iter())
        .map(|(n, c, args)| (n.clone(), Server::start(c, args, cwd)))
        .collect();
    let new = || crate::new_uuid().unwrap_or_else(|_| "stand-in".into());
    let mut session = a.session(new);
    let mut stream = Stream {
        out,
        spent: 0,
        context: CONTEXT_START,
        calls: 0,
        model: a.model.clone().unwrap_or_else(|| "stand-in".into()),
    };
    stream.line(&init(a, &session, &servers));
    let mut mind = Mind::default();
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
            continue;
        }
        let mut hands = Busy {
            stream: &mut stream,
            servers: &mut servers,
            cwd,
        };
        let text = mind.turn(&mut hands, &prompt);
        // Claude Code's last message is the turn's text, then its result.
        hands.say(&text);
        stream.context += CONTEXT_TURN;
        stream.result(&text, TURN_COST);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn strings(s: &[&str]) -> Vec<String> {
        s.iter().map(|s| s.to_string()).collect()
    }

    #[derive(Default)]
    struct Fake {
        calls: Vec<(String, Value)>,
        writes: Vec<(String, String)>,
        said: Vec<String>,
        answers: BTreeMap<String, Result<String, String>>,
        refuse_writes: bool,
    }

    impl Hands for Fake {
        fn call(&mut self, tool: &str, args: Value) -> Result<String, String> {
            self.calls.push((tool.to_string(), args));
            self.answers
                .get(tool)
                .cloned()
                .unwrap_or_else(|| Ok(String::new()))
        }
        fn write(&mut self, path: &str, text: &str) -> Result<(), String> {
            if self.refuse_writes {
                return Err("read-only".into());
            }
            self.writes.push((path.to_string(), text.to_string()));
            Ok(())
        }
        fn say(&mut self, text: &str) {
            self.said.push(text.to_string());
        }
    }

    fn fake(whoami: &str) -> Fake {
        let mut f = Fake::default();
        f.answers.insert("whoami".into(), Ok(whoami.into()));
        f
    }

    const LEADER: &str = "name: lead\nrole: the leader\nproject: repo\nyou answer to: the person\n";
    const WORKER: &str =
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
        ]))
        .unwrap();
        assert_eq!(
            a,
            Args {
                tools: strings(&["Read", "Write"]),
                disallowed: strings(&["Agent", "Task"]),
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
    fn a_worker_writes_its_file_and_tells_its_leader() {
        let mut f = fake(WORKER);
        let mail = format!(
            "{MAIL}:\n\n[assign 3] from lead:\nWrite a greeting\nin French\n\n[inform 4] from lead:\nthanks"
        );
        let said = Mind::default().turn(&mut f, &mail);
        assert_eq!(said, "Done: wrote stand-in/claude-1.md.");
        assert_eq!(
            f.writes,
            vec![(
                "stand-in/claude-1.md".to_string(),
                "# Write a greeting\n\nWrite a greeting\nin French\n".to_string()
            )]
        );
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
        f.refuse_writes = true;
        let mail = format!("{MAIL}:\n\n[assign] from lead:\ndo it");
        assert_eq!(
            m.turn(&mut f, &mail),
            "I could not write stand-in/claude-1.md: read-only"
        );
        f.refuse_writes = false;
        f.answers.insert("send".into(), Err("gone".into()));
        assert_eq!(
            m.turn(&mut f, &mail),
            "wrote stand-in/claude-1.md, but lead did not hear: gone"
        );
    }

    #[test]
    fn others_and_a_session_without_a_bus_do_nothing() {
        let mut f = fake("name: claude-1.1\nrole: a helper\n");
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
        run(&a, Path::new("/work"), input.as_bytes(), &mut out);
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
        run(&a, Path::new("/work"), input.as_bytes(), &mut out);
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
    }

    #[test]
    fn its_hands_write_in_its_directory_and_stream_the_call() {
        let dir = std::env::temp_dir().join(format!("imece-stand-in-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let mut stream = Stream {
            out: Vec::new(),
            spent: 0,
            context: 0,
            calls: 0,
            model: "m".into(),
        };
        let mut hands = Busy {
            stream: &mut stream,
            servers: &mut [],
            cwd: &dir,
        };
        hands.write("a/b.md", "hi").unwrap();
        assert_eq!(std::fs::read_to_string(dir.join("a/b.md")).unwrap(), "hi");
        std::fs::write(dir.join("file"), "").unwrap();
        assert!(
            hands.write("file/x", "no").is_err(),
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
        std::fs::write(&imece, "#!/bin/sh\nprintf '%s|' \"$@\"\n").unwrap();
        std::fs::set_permissions(&imece, std::fs::Permissions::from_mode(0o700)).unwrap();
        let claude = install(&dir, &imece).unwrap();
        assert_eq!(claude, dir.join("claude"));
        let mode = std::fs::metadata(&claude).unwrap().permissions().mode();
        assert_eq!(mode & 0o777, 0o700);
        let out = Command::new(&claude).args(["-p", "a b"]).output().unwrap();
        assert_eq!(String::from_utf8_lossy(&out.stdout), "stand-in|-p|a b|");
        let _ = std::fs::remove_dir_all(&dir);
    }

    fn said(args: &[&str], input: &str) -> (i32, String) {
        let mut out = Vec::new();
        let code = answer(&strings(args), Path::new("/"), input.as_bytes(), &mut out);
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
            let mut stream = Stream {
                out: Vec::new(),
                spent: 0,
                context: 0,
                calls: 0,
                model: "m".into(),
            };
            let mut hands = Busy {
                stream: &mut stream,
                servers: &mut servers,
                cwd: &dir,
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
