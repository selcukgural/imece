//! Claude Code as an agent: one long-lived `claude -p` session per agent
//! (never a fresh process per turn), prompts as stream-json on stdin, events
//! as stream-json on stdout.
//!
//! The session runs `--restricted` (no code-running tools, no user or
//! project settings, so no hooks; file tools confined to the agent's
//! directory), without MCP servers, with exactly the tools asked for and a
//! spending cap. What the vendor reports back in its `init` event is checked
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
        "--disable-slash-commands",
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
    a
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
    tools: BTreeSet<String>,
    started: bool,
    /// Tool names by tool-use id, to name a failed result.
    calls: HashMap<String, String>,
}

impl Parser {
    pub fn new(spec: &Spec) -> Parser {
        Parser {
            cwd: spec.cwd.clone(),
            tools: spec.tools.iter().cloned().collect(),
            started: false,
            calls: HashMap::new(),
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
            "result" => Ok(vec![result(&v)]),
            // Rate limits, retries, partial chunks, hook events: nothing the
            // screen needs yet.
            _ => Ok(Vec::new()),
        }
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
        if v["mcp_servers"].as_array().is_none_or(|s| !s.is_empty()) {
            return Err("has MCP servers".into());
        }
        if self.started {
            return Ok(Vec::new());
        }
        self.started = true;
        Ok(vec![Event::Started {
            session: v["session_id"].as_str().unwrap_or_default().to_string(),
            model: v["model"].as_str().unwrap_or_default().to_string(),
        }])
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
    input["pattern"].as_str().unwrap_or_default().to_string()
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
        self.child.write_line(&prompt_line(text))
    }

    /// Asks the session to stop without waiting; dropping it finishes the
    /// job.
    pub fn terminate(&mut self) {
        self.child.terminate();
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
        }
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
        for flag in [
            "--restricted",
            "--strict-mcp-config",
            "--disable-slash-commands",
            "--no-chrome",
        ] {
            assert!(a.iter().any(|x| x == flag), "missing {flag}");
        }
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
        let mut s = Session::start(spec, move |e| {
            let _ = tx.send(e);
        })
        .unwrap();
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
}
