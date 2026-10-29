//! Claude Code as an agent: one long-lived `claude -p` session per agent
//! (never a fresh process per turn), prompts as stream-json on stdin, events
//! as stream-json on stdout.
//!
//! The session runs `--restricted` (no code-running tools, no user or
//! project settings, so no hooks; file tools confined to the agent's
//! directory), with no MCP server but imece's own, exactly the tools asked
//! for and a spending cap. What the vendor reports back in its `init` event is checked
//! against that on every turn; anything else stops the session.

use std::collections::{BTreeSet, HashMap};
use std::io;
use std::path::{Path, PathBuf};

use serde_json::Value;

use crate::Event;
use crate::child::{self, Child, Out};

/// Variables Claude Code may need besides [`child::BASE_ENV`]: an API key
/// (a subscription login lives in the keychain), its config directory, and
/// proxy and CA settings of corporate networks.
pub const ENV: &[&str] = &[
    "ANTHROPIC_API_KEY",
    "CLAUDE_CONFIG_DIR",
    "XDG_CONFIG_HOME",
    "HTTPS_PROXY",
    "HTTP_PROXY",
    "NO_PROXY",
    "https_proxy",
    "http_proxy",
    "no_proxy",
    "NODE_EXTRA_CA_CERTS",
    "SSL_CERT_FILE",
];

/// Edits inside the agent's directory need nobody's approval; everything
/// that would ask is denied (`--permission-prompts none`).
const PERMISSION_MODE: &str = "acceptEdits";

/// Claude Code names a tool of an MCP server `mcp__<server>__<tool>`, which
/// is the name the screen uses for every vendor.
pub use crate::{MCP_SERVER, Mcp, mcp_tool};

/// The program on PATH.
pub const PROGRAM: &str = "claude";

/// Claude Code's model for a tier: its own aliases, which follow the
/// newest model of each size. `Standard` is Claude Code's own default.
pub fn model(tier: crate::Tier) -> Option<&'static str> {
    match tier {
        crate::Tier::Cheap => Some("haiku"),
        crate::Tier::Standard => None,
        crate::Tier::Strong => Some("opus"),
    }
}

/// Tools that read files.
pub const READ_TOOLS: &[&str] = &["Glob", "Grep", "Read"];
/// Tools that read and change files.
pub const EDIT_TOOLS: &[&str] = &["Edit", "Glob", "Grep", "Read", "Write"];

#[derive(Clone, Debug)]
pub struct Spec {
    /// `claude`, or a stand-in in tests.
    pub program: String,
    /// The agent's directory; its file tools can not leave it.
    pub cwd: PathBuf,
    pub name: String,
    pub tools: Vec<String>,
    pub model: Option<String>,
    /// The session's spending cap, in millionths of a dollar.
    pub budget: u64,
    /// Appended to Claude Code's own system prompt.
    pub system: String,
    /// imece's MCP server, if the agent gets its tools.
    pub mcp: Option<Mcp>,
}

/// The command line for `spec`. Prompts never go here; they go to stdin.
pub fn args(spec: &Spec) -> Vec<String> {
    let mut a: Vec<String> = [
        "-p",
        "--input-format",
        "stream-json",
        "--output-format",
        "stream-json",
        "--verbose",
        "--restricted",
        "--strict-mcp-config",
        "--no-chrome",
        "--permission-mode",
        PERMISSION_MODE,
        "--permission-prompts",
        "none",
    ]
    .map(String::from)
    .into();
    a.push("--tools".into());
    // An empty list is no tools at all, never the default set.
    a.push(spec.tools.join(","));
    a.push("--max-budget-usd".into());
    a.push(format!(
        "{}.{:06}",
        spec.budget / 1_000_000,
        spec.budget % 1_000_000
    ));
    a.push("--name".into());
    a.push(spec.name.clone());
    a.push("--append-system-prompt".into());
    a.push(spec.system.clone());
    if let Some(m) = &spec.model {
        a.push("--model".into());
        a.push(m.clone());
    }
    if let Some(mcp) = &spec.mcp {
        let config = serde_json::json!({ "mcpServers": { MCP_SERVER: {
            "type": "stdio",
            "command": mcp.command,
            "args": mcp.args,
        }}});
        a.push("--mcp-config".into());
        a.push(config.to_string());
        if !mcp.tools.is_empty() {
            // Exact names only: never a pattern, never a built-in tool.
            a.push("--allowedTools".into());
            a.push(
                mcp.tools
                    .iter()
                    .map(|t| mcp_tool(t))
                    .collect::<Vec<_>>()
                    .join(","),
            );
        }
    }
    a
}

/// A prompt Claude Code would run as a slash command of its own (`/clear`,
/// `/compact`, a skill) is said to be text: only [`Session::command`]
/// sends one on purpose. It takes the command from a content block too,
/// so the prefix is the one guard there is.
fn plain(text: &str) -> String {
    if text.trim_start().starts_with('/') {
        format!("Text:\n{text}")
    } else {
        text.to_string()
    }
}

/// The stdin line that gives the agent a prompt.
pub fn prompt_line(text: &str) -> String {
    serde_json::json!({
        "type": "user",
        "message": { "role": "user", "content": text },
    })
    .to_string()
}

/// Turns stdout lines into [`Event`]s and checks the settings the vendor
/// reports.
pub struct Parser {
    cwd: PathBuf,
    /// Built-in tools and imece's MCP tools, as the session names them.
    tools: BTreeSet<String>,
    mcp: bool,
    started: bool,
    /// Tool names by tool-use id, to name a failed result.
    calls: HashMap<String, String>,
    /// The session's cost counter starts again after `/clear`; the screen
    /// gets one that never goes down: what the earlier conversations
    /// cost, and the last total seen.
    cost_base: u64,
    cost_last: u64,
    /// The slash commands the last `init` listed, to report a change.
    commands: Vec<String>,
}

impl Parser {
    pub fn new(spec: &Spec) -> Parser {
        Parser {
            cwd: spec.cwd.clone(),
            tools: spec
                .tools
                .iter()
                .cloned()
                .chain(
                    spec.mcp
                        .iter()
                        .flat_map(|m| m.tools.iter().map(|t| mcp_tool(t))),
                )
                .collect(),
            mcp: spec.mcp.is_some(),
            started: false,
            calls: HashMap::new(),
            commands: Vec::new(),
            cost_base: 0,
            cost_last: 0,
        }
    }

    /// The events in one stdout line, or why the session must stop.
    pub fn line(&mut self, line: &str) -> Result<Vec<Event>, String> {
        if line.trim().is_empty() {
            return Ok(Vec::new());
        }
        let v: Value = serde_json::from_str(line).map_err(|e| format!("not stream-json ({e})"))?;
        let kind = v["type"].as_str().unwrap_or_default();
        if kind == "system" && v["subtype"] == "init" {
            return self.init(&v);
        }
        if !self.started && matches!(kind, "assistant" | "user" | "result") {
            return Err(format!("{kind} event before the session's settings"));
        }
        match kind {
            "assistant" => self.assistant(&v),
            "user" => Ok(self.results(&v)),
            "result" => Ok(vec![self.result(&v)]),
            // Rate limits, retries, partial chunks, hook events, the reset
            // `/clear` announces (its new `init` is checked like the first):
            // nothing the screen needs yet.
            _ => Ok(Vec::new()),
        }
    }

    fn result(&mut self, v: &Value) -> Event {
        let mut e = result(v);
        if let Event::TurnDone { cost, .. } = &mut e {
            if *cost < self.cost_last {
                self.cost_base += self.cost_last;
            }
            self.cost_last = *cost;
            *cost += self.cost_base;
        }
        e
    }

    fn init(&mut self, v: &Value) -> Result<Vec<Event>, String> {
        let mode = v["permissionMode"].as_str().unwrap_or_default();
        if mode != PERMISSION_MODE {
            return Err(format!("runs in permission mode {mode:?}"));
        }
        let tools: BTreeSet<String> = v["tools"]
            .as_array()
            .ok_or("reports no tool list")?
            .iter()
            .map(|t| t.as_str().unwrap_or_default().to_string())
            .collect();
        if tools != self.tools {
            let extra: Vec<&String> = tools.difference(&self.tools).collect();
            let missing: Vec<&String> = self.tools.difference(&tools).collect();
            return Err(format!(
                "tools differ: extra {extra:?}, missing {missing:?}"
            ));
        }
        let servers = v["mcp_servers"]
            .as_array()
            .ok_or("reports no MCP server list")?;
        match (self.mcp, servers.as_slice()) {
            (false, []) => {}
            (true, [s]) if s["name"] == MCP_SERVER => {
                if s["status"] != "connected" {
                    return Err(format!("has imece's MCP server {}", s["status"]));
                }
            }
            _ => return Err("has other MCP servers".into()),
        }
        // `init` lists the session's slash commands (without the slash);
        // they are reported when first seen and when they change.
        let commands: Vec<String> = v["slash_commands"]
            .as_array()
            .into_iter()
            .flatten()
            .filter_map(Value::as_str)
            .map(|c| format!("/{}", c.trim_start_matches('/')))
            .collect();
        let mut out = Vec::new();
        if !self.started {
            self.started = true;
            out.push(Event::Started {
                session: v["session_id"].as_str().unwrap_or_default().to_string(),
                model: v["model"].as_str().unwrap_or_default().to_string(),
            });
        }
        if commands != self.commands {
            self.commands = commands.clone();
            out.push(Event::Commands(commands));
        }
        Ok(out)
    }

    fn assistant(&mut self, v: &Value) -> Result<Vec<Event>, String> {
        let mut out = Vec::new();
        for block in v["message"]["content"].as_array().into_iter().flatten() {
            match block["type"].as_str().unwrap_or_default() {
                "text" => {
                    let text = block["text"].as_str().unwrap_or_default().trim();
                    if !text.is_empty() {
                        out.push(Event::Text(text.to_string()));
                    }
                }
                "tool_use" => {
                    let name = block["name"].as_str().unwrap_or_default().to_string();
                    if !self.tools.contains(&name) {
                        return Err(format!("used tool {name:?} it was not given"));
                    }
                    if let Some(id) = block["id"].as_str() {
                        self.calls.insert(id.to_string(), name.clone());
                    }
                    let target = target(&block["input"], &self.cwd);
                    out.push(Event::Tool { name, target });
                }
                _ => {}
            }
        }
        Ok(out)
    }

    fn results(&mut self, v: &Value) -> Vec<Event> {
        let mut out = Vec::new();
        for block in v["message"]["content"].as_array().into_iter().flatten() {
            if block["type"] != "tool_result" {
                continue;
            }
            let name = block["tool_use_id"]
                .as_str()
                .and_then(|id| self.calls.remove(id))
                .unwrap_or_default();
            if block["is_error"] == true {
                out.push(Event::ToolFailed { name });
            }
        }
        out
    }
}

fn result(v: &Value) -> Event {
    let ok = v["subtype"] == "success" && v["is_error"] != true;
    let text = match v["result"].as_str() {
        Some(r) if ok => r.trim().to_string(),
        _ => v["subtype"].as_str().unwrap_or("error").to_string(),
    };
    let usd = v["total_cost_usd"].as_f64().unwrap_or(0.0);
    let cost = if usd.is_finite() && usd > 0.0 {
        (usd * 1_000_000.0).round() as u64
    } else {
        0
    };
    Event::TurnDone {
        ok,
        text,
        cost,
        denied: v["permission_denials"].as_array().map_or(0, Vec::len),
    }
}

/// What a tool call touches, in a few characters: a path relative to the
/// agent's directory, or a search pattern.
fn target(input: &Value, cwd: &Path) -> String {
    if let Some(p) = input["file_path"].as_str().or(input["path"].as_str()) {
        let p = Path::new(p);
        return p
            .strip_prefix(cwd)
            .unwrap_or(p)
            .to_string_lossy()
            .into_owned();
    }
    input["pattern"]
        .as_str()
        .or(input["command"].as_str())
        .unwrap_or_default()
        .to_string()
}

/// A running Claude Code session.
pub struct Session {
    child: Child,
}

impl Session {
    /// Starts the session. `on` gets its events on a reader thread; the
    /// last one is always [`Event::Stopped`].
    pub fn start(spec: &Spec, mut on: impl FnMut(Event) + Send + 'static) -> io::Result<Session> {
        let mut cmd = child::command(&spec.program, &spec.cwd, ENV);
        cmd.args(args(spec));
        let mut parser = Parser::new(spec);
        let mut stopped = false;
        let child = Child::spawn(cmd, move |out| match out {
            Out::Line(line) => match parser.line(line) {
                Ok(events) => {
                    events.into_iter().for_each(&mut on);
                    true
                }
                Err(why) => {
                    stopped = true;
                    on(Event::Stopped(format!("stopped: the session {why}")));
                    false
                }
            },
            Out::End(why) => {
                if !stopped {
                    on(Event::Stopped(why));
                }
                false
            }
        })?;
        Ok(Session { child })
    }

    /// Gives the agent a prompt. A busy session queues it for after the
    /// running turn.
    pub fn prompt(&mut self, text: &str) -> io::Result<()> {
        self.child.write_line(&prompt_line(&plain(text)))
    }

    /// Gives the session one of Claude Code's own slash commands, as the
    /// person typed it. It runs as a turn: `/clear` answers with an empty
    /// result and starts the cost counter again (the parser keeps the
    /// screen's total going up).
    pub fn command(&mut self, line: &str) -> io::Result<()> {
        let line = line.trim();
        if !line.starts_with('/') {
            return Err(io::Error::new(
                io::ErrorKind::Unsupported,
                "a command of claude's starts with /",
            ));
        }
        self.child.write_line(&prompt_line(line))
    }

    /// Asks the session to stop without waiting; dropping it finishes the
    /// job.
    pub fn terminate(&mut self) {
        self.child.terminate();
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

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::mpsc;
    use std::time::Duration;

    fn spec(tools: &[&str]) -> Spec {
        Spec {
            program: "claude".into(),
            cwd: PathBuf::from("/work/w1"),
            name: "claude-1".into(),
            tools: tools.iter().map(|t| t.to_string()).collect(),
            model: None,
            budget: 1_250_000,
            system: "You are claude-1.".into(),
            mcp: None,
        }
    }

    fn with_bus(tools: &[&str]) -> Spec {
        let mut s = spec(tools);
        s.mcp = Some(Mcp {
            command: "/opt/imece".into(),
            args: vec!["mcp-relay".into(), "/tmp/i/a1.sock".into()],
            tools: vec!["send".into()],
        });
        s
    }

    const BUS: &str = r#"[{"name":"imece","status":"connected","source":"dynamic"}]"#;
    const BUS_INIT: &str = r#"["Edit","Glob","Grep","Read","Write","mcp__imece__send"]"#;

    #[test]
    fn the_tiers_are_claude_code_s_aliases_and_its_own_default() {
        assert_eq!(model(crate::Tier::Cheap), Some("haiku"));
        assert_eq!(model(crate::Tier::Standard), None);
        assert_eq!(model(crate::Tier::Strong), Some("opus"));
        assert_eq!(PROGRAM, "claude");
    }

    #[test]
    fn imece_s_tools_are_allowed_by_exact_name_and_nothing_else_is() {
        let a = args(&with_bus(EDIT_TOOLS));
        let after = |f: &str| a[a.iter().position(|x| x == f).unwrap() + 1].clone();
        assert_eq!(after("--tools"), "Edit,Glob,Grep,Read,Write");
        assert_eq!(after("--allowedTools"), "mcp__imece__send");
        assert!(a.iter().any(|x| x == "--strict-mcp-config"));
        let config: Value = serde_json::from_str(&after("--mcp-config")).unwrap();
        let servers = config["mcpServers"].as_object().unwrap();
        assert_eq!(servers.len(), 1);
        assert_eq!(servers["imece"]["command"], "/opt/imece");
        assert_eq!(servers["imece"]["args"][1], "/tmp/i/a1.sock");
        let plain = args(&spec(EDIT_TOOLS));
        assert!(
            !plain
                .iter()
                .any(|x| x == "--mcp-config" || x == "--allowedTools")
        );
    }

    #[test]
    fn a_session_with_the_bus_must_report_exactly_imece_s_server_and_tools() {
        let mut p = Parser::new(&with_bus(EDIT_TOOLS));
        assert!(p.line(&init(BUS_INIT, "acceptEdits", BUS)).is_ok());
        let use_ = r#"{"type":"assistant","message":{"content":[{"type":"tool_use","id":"t","name":"mcp__imece__send","input":{"to":"lead"}}]}}"#;
        assert!(p.line(use_).is_ok());

        let bad = [
            // The server did not come up, or is not ours, or not alone.
            init(BUS_INIT, "acceptEdits", "[]"),
            init(
                BUS_INIT,
                "acceptEdits",
                r#"[{"name":"imece","status":"failed"}]"#,
            ),
            init(
                BUS_INIT,
                "acceptEdits",
                r#"[{"name":"other","status":"connected"}]"#,
            ),
            init(
                BUS_INIT,
                "acceptEdits",
                r#"[{"name":"imece","status":"connected"},{"name":"x","status":"connected"}]"#,
            ),
            // A tool imece's server does not offer.
            init(
                r#"["Edit","Glob","Grep","Read","Write","mcp__imece__send","mcp__imece__exec"]"#,
                "acceptEdits",
                BUS,
            ),
            init(GOOD_INIT, "acceptEdits", BUS),
        ];
        for line in bad {
            let mut p = Parser::new(&with_bus(EDIT_TOOLS));
            assert!(p.line(&line).is_err(), "accepted {line}");
        }
        // Without the bus, imece's server is as unwelcome as any.
        let mut p = Parser::new(&spec(EDIT_TOOLS));
        assert!(p.line(&init(BUS_INIT, "acceptEdits", BUS)).is_err());
    }

    fn init(tools: &str, mode: &str, mcp: &str) -> String {
        format!(
            r#"{{"type":"system","subtype":"init","session_id":"s1","model":"m1","tools":{tools},"permissionMode":"{mode}","mcp_servers":{mcp}}}"#
        )
    }

    const GOOD_INIT: &str = r#"["Edit","Glob","Grep","Read","Write"]"#;

    #[test]
    fn the_command_line_is_locked_down_and_carries_no_prompt() {
        let a = args(&spec(EDIT_TOOLS));
        for flag in ["--restricted", "--strict-mcp-config", "--no-chrome"] {
            assert!(a.iter().any(|x| x == flag), "missing {flag}");
        }
        // The person's own commands reach the session; a prompt that
        // looks like one is guarded in `plain`.
        assert!(!a.iter().any(|x| x == "--disable-slash-commands"));
        let after = |f: &str| a[a.iter().position(|x| x == f).unwrap() + 1].clone();
        assert_eq!(after("--tools"), "Edit,Glob,Grep,Read,Write");
        assert_eq!(after("--permission-mode"), "acceptEdits");
        assert_eq!(after("--permission-prompts"), "none");
        assert_eq!(after("--max-budget-usd"), "1.250000");
        assert!(
            !a.iter()
                .any(|x| x.contains("dangerously") || x == "bypassPermissions")
        );
    }

    #[test]
    fn no_tools_means_none_not_the_default_set() {
        let a = args(&spec(&[]));
        let i = a.iter().position(|x| x == "--tools").unwrap();
        assert_eq!(a[i + 1], "");
    }

    #[test]
    fn a_prompt_is_one_json_line_even_with_newlines() {
        let l = prompt_line("fix it\nnow \"please\"");
        assert!(!l.contains('\n'));
        let v: Value = serde_json::from_str(&l).unwrap();
        assert_eq!(v["message"]["content"], "fix it\nnow \"please\"");
    }

    #[test]
    fn the_reported_settings_must_match_what_was_asked() {
        let mut p = Parser::new(&spec(EDIT_TOOLS));
        assert_eq!(
            p.line(&init(GOOD_INIT, "acceptEdits", "[]")).unwrap(),
            [Event::Started {
                session: "s1".into(),
                model: "m1".into()
            }]
        );
        // Every later turn repeats init and is checked again, quietly.
        assert_eq!(p.line(&init(GOOD_INIT, "acceptEdits", "[]")).unwrap(), []);
        // The slash commands it lists are reported once, and again only
        // when they change, as `/name`.
        let with = |cmds: &str| {
            format!(
                r#"{{"type":"system","subtype":"init","session_id":"s1","model":"m1","tools":{GOOD_INIT},"permissionMode":"acceptEdits","mcp_servers":[],"slash_commands":{cmds}}}"#
            )
        };
        assert_eq!(
            p.line(&with(r#"["compact","cost","/context"]"#)).unwrap(),
            [Event::Commands(vec![
                "/compact".into(),
                "/cost".into(),
                "/context".into()
            ])]
        );
        assert_eq!(
            p.line(&with(r#"["compact","cost","/context"]"#)).unwrap(),
            []
        );
        assert_eq!(
            p.line(&with(r#"["compact"]"#)).unwrap(),
            [Event::Commands(vec!["/compact".into()])]
        );
        assert_eq!(p.line(&with(r#"[]"#)).unwrap(), [Event::Commands(vec![])]);
        // A first init that lists them says so right after Started.
        let mut p = Parser::new(&spec(EDIT_TOOLS));
        assert_eq!(
            p.line(&with(r#"["clear"]"#)).unwrap(),
            [
                Event::Started {
                    session: "s1".into(),
                    model: "m1".into()
                },
                Event::Commands(vec!["/clear".into()])
            ]
        );

        let bad = [
            init(
                r#"["Bash","Edit","Glob","Grep","Read","Write"]"#,
                "acceptEdits",
                "[]",
            ),
            init(r#"["Read"]"#, "acceptEdits", "[]"),
            init(GOOD_INIT, "bypassPermissions", "[]"),
            init(GOOD_INIT, "acceptEdits", r#"[{"name":"x"}]"#),
            init(GOOD_INIT, "acceptEdits", "null"),
            r#"{"type":"system","subtype":"init","permissionMode":"acceptEdits"}"#.to_string(),
        ];
        for line in bad {
            let mut p = Parser::new(&spec(EDIT_TOOLS));
            assert!(p.line(&line).is_err(), "accepted {line}");
        }
    }

    #[test]
    fn nothing_counts_before_the_settings_are_checked() {
        let mut p = Parser::new(&spec(EDIT_TOOLS));
        let text = r#"{"type":"assistant","message":{"content":[{"type":"text","text":"hi"}]}}"#;
        assert!(p.line(text).is_err());
        assert!(p.line("not json").is_err());
    }

    #[test]
    fn tools_texts_failures_and_turns_become_events() {
        let mut p = Parser::new(&spec(EDIT_TOOLS));
        p.line(&init(GOOD_INIT, "acceptEdits", "[]")).unwrap();
        let use_ = r#"{"type":"assistant","message":{"content":[
            {"type":"thinking","thinking":"hmm"},
            {"type":"text","text":"  Reading it.  "},
            {"type":"tool_use","id":"t1","name":"Edit","input":{"file_path":"/work/w1/src/main.rs"}},
            {"type":"tool_use","id":"t2","name":"Grep","input":{"pattern":"fn main"}}]}}"#
            .replace('\n', "");
        assert_eq!(
            p.line(&use_).unwrap(),
            [
                Event::Text("Reading it.".into()),
                Event::Tool {
                    name: "Edit".into(),
                    target: "src/main.rs".into()
                },
                Event::Tool {
                    name: "Grep".into(),
                    target: "fn main".into()
                },
            ]
        );
        let res = r#"{"type":"user","message":{"content":[{"type":"tool_result","tool_use_id":"t1","is_error":true,"content":"no"},{"type":"tool_result","tool_use_id":"t2","content":"ok"}]}}"#;
        assert_eq!(
            p.line(res).unwrap(),
            [Event::ToolFailed {
                name: "Edit".into()
            }]
        );
        let done = r#"{"type":"result","subtype":"success","is_error":false,"result":" Done. ","total_cost_usd":0.0123456,"permission_denials":[{}]}"#;
        assert_eq!(
            p.line(done).unwrap(),
            [Event::TurnDone {
                ok: true,
                text: "Done.".into(),
                cost: 12_346,
                denied: 1
            }]
        );
        let broke = r#"{"type":"result","subtype":"error_max_budget_usd","is_error":true,"total_cost_usd":1.3}"#;
        assert_eq!(
            p.line(broke).unwrap(),
            [Event::TurnDone {
                ok: false,
                text: "error_max_budget_usd".into(),
                cost: 1_300_000,
                denied: 0
            }]
        );
        assert_eq!(p.line(r#"{"type":"rate_limit_event"}"#).unwrap(), []);
    }

    #[test]
    fn a_tool_it_was_not_given_stops_the_session() {
        let mut p = Parser::new(&spec(READ_TOOLS));
        p.line(&init(r#"["Glob","Grep","Read"]"#, "acceptEdits", "[]"))
            .unwrap();
        let line = r#"{"type":"assistant","message":{"content":[{"type":"tool_use","id":"t","name":"Write","input":{}}]}}"#;
        assert!(p.line(line).is_err());
    }

    /// A stand-in for `claude` that checks its stdin and prints a session.
    fn fake(dir: &Path, body: &str) -> String {
        std::fs::create_dir_all(dir).unwrap();
        let path = dir.join("fake-claude");
        std::fs::write(&path, format!("#!/bin/sh\n{body}\n")).unwrap();
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o755)).unwrap();
        path.to_string_lossy().into_owned()
    }

    fn events(spec: &Spec, prompt: &str) -> Vec<Event> {
        let (tx, rx) = mpsc::channel();
        // Through the trait, as the screen drives every vendor.
        let mut s: Box<dyn crate::Agent> = Box::new(
            Session::start(spec, move |e| {
                let _ = tx.send(e);
            })
            .unwrap(),
        );
        s.prompt(prompt).unwrap();
        let mut out = Vec::new();
        loop {
            let e = rx.recv_timeout(Duration::from_secs(10)).expect("in time");
            let end = matches!(e, Event::Stopped(_));
            out.push(e);
            if end {
                return out;
            }
        }
    }

    #[test]
    fn a_session_reads_its_prompt_from_stdin_and_reports_events() {
        let dir = std::env::temp_dir().join(format!("imece-fake-{}", std::process::id()));
        let init = init(GOOD_INIT, "acceptEdits", "[]");
        let body = format!(
            r#"read line
case "$line" in *'"content":"hello"'*) ;; *) echo "bad stdin: $line" >&2; exit 9;; esac
echo '{init}'
echo '{{"type":"result","subtype":"success","result":"hi","total_cost_usd":0.5}}'"#
        );
        let mut s = spec(EDIT_TOOLS);
        s.program = fake(&dir, &body);
        s.cwd = dir.clone();
        let got = events(&s, "hello");
        assert_eq!(
            got,
            [
                Event::Started {
                    session: "s1".into(),
                    model: "m1".into()
                },
                Event::TurnDone {
                    ok: true,
                    text: "hi".into(),
                    cost: 500_000,
                    denied: 0
                },
                Event::Stopped("exited with code 0".into()),
            ]
        );
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn terminating_through_the_trait_stops_the_process() {
        let dir = std::env::temp_dir().join(format!("imece-fake-term-{}", std::process::id()));
        let mut s = spec(EDIT_TOOLS);
        s.program = fake(&dir, "read line\nsleep 30");
        s.cwd = dir.clone();
        let (tx, rx) = mpsc::channel();
        let mut agent: Box<dyn crate::Agent> = Box::new(
            Session::start(&s, move |e| {
                let _ = tx.send(e);
            })
            .unwrap(),
        );
        agent.terminate();
        let e = rx.recv_timeout(Duration::from_secs(10)).expect("in time");
        assert!(
            matches!(&e, Event::Stopped(w) if w.contains("signal 15")),
            "{e:?}"
        );
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn a_session_with_other_settings_is_stopped_before_its_events_count() {
        let dir = std::env::temp_dir().join(format!("imece-fake-bad-{}", std::process::id()));
        let init = init(r#"["Bash","Read"]"#, "acceptEdits", "[]");
        let body = format!(
            r#"read line
echo '{init}'
echo '{{"type":"result","subtype":"success","result":"hi","total_cost_usd":0.5}}'
sleep 5"#
        );
        let mut s = spec(EDIT_TOOLS);
        s.program = fake(&dir, &body);
        s.cwd = dir.clone();
        let got = events(&s, "hello");
        assert_eq!(got.len(), 1, "{got:?}");
        assert!(
            matches!(&got[0], Event::Stopped(w) if w.contains("tools differ")),
            "{got:?}"
        );
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn a_result_is_ok_only_when_successful_and_not_flagged_and_cost_is_never_negative() {
        use serde_json::json;
        let ok = result(&json!({"subtype":"success","result":" fine ","total_cost_usd":0.25}));
        assert_eq!(
            ok,
            Event::TurnDone {
                ok: true,
                text: "fine".into(),
                cost: 250_000,
                denied: 0
            }
        );
        let flagged = result(&json!({"subtype":"success","is_error":true,"result":"text"}));
        assert!(
            matches!(&flagged, Event::TurnDone { ok: false, text, .. } if text == "success"),
            "{flagged:?}"
        );
        let failed = result(&json!({"subtype":"error_max_turns","result":"ignored"}));
        assert!(
            matches!(&failed, Event::TurnDone { ok: false, text, .. } if text == "error_max_turns"),
            "{failed:?}"
        );
        for cost in [json!(0.0), json!(-1.0), json!("x"), json!(null)] {
            let e = result(&json!({"subtype":"success","result":"r","total_cost_usd":cost}));
            assert!(
                matches!(e, Event::TurnDone { cost: 0, .. }),
                "{cost}: {e:?}"
            );
        }
    }

    #[test]
    fn a_prompt_that_looks_like_a_slash_command_is_said_to_be_text() {
        assert_eq!(plain("/clear"), "Text:\n/clear");
        assert_eq!(plain("  /compact now"), "Text:\n  /compact now");
        assert_eq!(plain("fix a/b"), "fix a/b");
        assert_eq!(plain(""), "");
        assert_eq!(
            prompt_line(&plain("/clear")),
            r#"{"message":{"content":"Text:\n/clear","role":"user"},"type":"user"}"#
        );
    }

    #[test]
    fn a_command_of_claude_s_own_goes_as_it_is_and_the_rest_is_refused() {
        let dir = std::env::temp_dir().join(format!("imece-claude-cmd-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let out = dir.join("first-line.txt");
        let mut s = spec(EDIT_TOOLS);
        s.cwd = dir.clone();
        // Reads one line, keeps it, ends.
        s.program = fake(
            &dir,
            &format!("IFS= read -r l; printf '%s\\n' \"$l\" > {}", out.display()),
        );
        let (tx, rx) = mpsc::channel();
        let mut session: Box<dyn crate::Agent> = Box::new(
            Session::start(&s, move |e| {
                let _ = tx.send(e);
            })
            .unwrap(),
        );
        for not_a_command in ["cost", "", "clear please"] {
            let e = session.command(not_a_command).unwrap_err();
            assert_eq!(e.kind(), io::ErrorKind::Unsupported, "{not_a_command:?}");
        }
        session.command(" /cost ").unwrap();
        let e = rx.recv_timeout(Duration::from_secs(10)).expect("in time");
        assert!(matches!(e, Event::Stopped(_)), "{e:?}");
        assert_eq!(
            std::fs::read_to_string(&out).unwrap(),
            format!("{}\n", prompt_line("/cost")),
            "the command goes as typed, not as text"
        );
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn the_cost_keeps_going_up_across_a_clear() {
        use serde_json::json;
        let mut p = Parser::new(&spec(EDIT_TOOLS));
        p.line(&init(GOOD_INIT, "acceptEdits", "[]")).unwrap();
        let done = |usd: f64| {
            json!({"type":"result","subtype":"success","result":"","total_cost_usd":usd})
                .to_string()
        };
        let cost = |p: &mut Parser, line: &str| match p.line(line).unwrap().as_slice() {
            [Event::TurnDone { cost, .. }] => *cost,
            other => panic!("{other:?}"),
        };
        assert_eq!(cost(&mut p, &done(0.0007)), 700);
        assert_eq!(cost(&mut p, &done(0.0012)), 1200);
        // `/clear`: a reset is announced, the settings come again, the
        // counter starts from nothing.
        assert_eq!(
            p.line(r#"{"type":"conversation_reset","trigger":"clear"}"#)
                .unwrap(),
            []
        );
        assert_eq!(p.line(&init(GOOD_INIT, "acceptEdits", "[]")).unwrap(), []);
        assert_eq!(cost(&mut p, &done(0.0)), 1200);
        assert_eq!(cost(&mut p, &done(0.0003)), 1500);
        assert_eq!(cost(&mut p, &done(0.0003)), 1500);
        // The settings after a reset are checked like the first.
        assert!(p.line(&init(r#"["Bash"]"#, "acceptEdits", "[]")).is_err());
    }

    #[test]
    fn harmless_events_pass_before_the_settings_and_real_ones_pass_after() {
        let mut p = Parser::new(&spec(EDIT_TOOLS));
        assert_eq!(p.line(r#"{"type":"rate_limit_event"}"#).unwrap(), []);
        p.line(&init(GOOD_INIT, "acceptEdits", "[]")).unwrap();
        let done = r#"{"type":"result","subtype":"success","result":"ok"}"#;
        assert!(matches!(
            p.line(done).unwrap()[..],
            [Event::TurnDone { .. }]
        ));
    }
}
