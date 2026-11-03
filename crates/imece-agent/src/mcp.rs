//! A small MCP server that offers tools to agents: line-delimited JSON-RPC
//! over a Unix socket, one socket per agent.
//!
//! The agent's CLI starts MCP servers itself, as child processes on stdio.
//! It starts [`relay`] (imece's own binary), which only copies bytes between
//! its stdio and the agent's socket; the tools run here, in imece. Who calls
//! is the socket the call came through, never something the agent says.

use std::io::{self, BufWriter, Read, Write};
use std::net::Shutdown;
use std::os::unix::net::{UnixListener, UnixStream};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::thread;

use serde_json::{Value, json};

use crate::child;

/// The longest request read. A longer one ends the connection.
pub const MAX_LINE: usize = 1 << 20;
/// Connections one socket serves at once; more are closed. A session needs
/// one; a restarting session briefly two.
const MAX_CONNECTIONS: usize = 4;
/// Protocol versions answered as asked; any other gets the newest.
const VERSIONS: &[&str] = &["2025-11-25", "2025-06-18", "2025-03-26", "2024-11-05"];

#[derive(Clone, Debug)]
pub struct Tool {
    pub name: String,
    pub description: String,
    /// JSON Schema of the tool's arguments.
    pub input: Value,
}

/// What a tool call gives back to the agent: text, or an error it reads.
pub type Answer = Result<String, String>;

/// Answers the requests read from `r` on `w` until either side closes.
pub fn serve(
    r: impl Read,
    w: impl Write,
    tools: &[Tool],
    mut call: impl FnMut(&str, &Value) -> Answer,
) {
    let mut w = BufWriter::new(w);
    child::read_lines(r, MAX_LINE, |line| {
        let Some(line) = line else { return false };
        let Some(reply) = answer(line, tools, &mut call) else {
            return true;
        };
        writeln!(w, "{reply}").and_then(|()| w.flush()).is_ok()
    });
}

/// The reply to one line, or nothing for a notification.
fn answer(
    line: &str,
    tools: &[Tool],
    call: &mut impl FnMut(&str, &Value) -> Answer,
) -> Option<Value> {
    if line.trim().is_empty() {
        return None;
    }
    let Ok(v) = serde_json::from_str::<Value>(line) else {
        return Some(error(Value::Null, -32700, "parse error"));
    };
    let id = v.get("id").cloned();
    let Some(method) = v["method"].as_str() else {
        return id.map(|id| error(id, -32600, "invalid request"));
    };
    // Notifications (`notifications/initialized`, cancellations) need no
    // reply.
    let id = id?;
    let result = match method {
        "initialize" => {
            let asked = v["params"]["protocolVersion"].as_str().unwrap_or_default();
            let version = VERSIONS
                .iter()
                .find(|&&s| s == asked)
                .unwrap_or(&VERSIONS[0]);
            json!({
                "protocolVersion": version,
                "capabilities": { "tools": {} },
                "serverInfo": { "name": "imece", "version": env!("CARGO_PKG_VERSION") },
            })
        }
        "ping" => json!({}),
        "tools/list" => json!({
            "tools": tools.iter().map(|t| json!({
                "name": t.name,
                "description": t.description,
                "inputSchema": t.input,
            })).collect::<Vec<_>>(),
        }),
        "tools/call" => {
            let name = v["params"]["name"].as_str().unwrap_or_default();
            let empty = json!({});
            let args = v["params"].get("arguments").unwrap_or(&empty);
            let got = if tools.iter().any(|t| t.name == name) {
                call(name, args)
            } else {
                Err(format!("there is no tool {name:?}"))
            };
            let (text, failed) = match got {
                Ok(t) => (t, false),
                Err(t) => (t, true),
            };
            json!({ "content": [{ "type": "text", "text": text }], "isError": failed })
        }
        _ => return Some(error(id, -32601, "method not found")),
    };
    Some(json!({ "jsonrpc": "2.0", "id": id, "result": result }))
}

fn error(id: Value, code: i64, message: &str) -> Value {
    json!({ "jsonrpc": "2.0", "id": id, "error": { "code": code, "message": message } })
}

/// The process on the other end of `conn`, as the system tells it when
/// it connected (macOS `LOCAL_PEERPID`, Linux `SO_PEERCRED`).
pub fn peer_pid(conn: &UnixStream) -> Option<u32> {
    use std::os::fd::AsRawFd;
    #[cfg(target_os = "macos")]
    let (level, name, mut pid) = (libc::SOL_LOCAL, libc::LOCAL_PEERPID, 0 as libc::pid_t);
    #[cfg(target_os = "linux")]
    let (level, name, mut cred) = (
        libc::SOL_SOCKET,
        libc::SO_PEERCRED,
        libc::ucred {
            pid: 0,
            uid: 0,
            gid: 0,
        },
    );
    #[cfg(target_os = "macos")]
    let (ptr, size) = ((&raw mut pid).cast(), std::mem::size_of::<libc::pid_t>());
    #[cfg(target_os = "linux")]
    let (ptr, size) = ((&raw mut cred).cast(), std::mem::size_of::<libc::ucred>());
    let mut len = libc::socklen_t::try_from(size).ok()?;
    let got = unsafe { libc::getsockopt(conn.as_raw_fd(), level, name, ptr, &raw mut len) };
    #[cfg(target_os = "linux")]
    let pid = cred.pid;
    peer_told(got, pid)
}

/// The peer `getsockopt` told, when it answered `got`: none on an error
/// or a pid no process has.
fn peer_told(got: libc::c_int, pid: libc::pid_t) -> Option<u32> {
    (got == 0 && pid > 0).then(|| pid.cast_unsigned())
}

/// Serves tools on a Unix socket until dropped.
pub struct Server {
    path: PathBuf,
    closing: Arc<AtomicBool>,
}

impl Server {
    /// Listens on `path`, which must not exist yet. Every connection is
    /// served on its own thread with `call`, which also gets the process
    /// on the other end ([`peer_pid`]; none when the system will not
    /// say), for the caller to decide whose it is.
    pub fn bind(
        path: &Path,
        tools: Vec<Tool>,
        call: impl Fn(&str, &Value, Option<u32>) -> Answer + Send + Sync + 'static,
    ) -> io::Result<Server> {
        let listener = UnixListener::bind(path)?;
        let closing = Arc::new(AtomicBool::new(false));
        let open = Arc::new(AtomicUsize::new(0));
        let (tools, call) = (Arc::new(tools), Arc::new(call));
        let stop = Arc::clone(&closing);
        thread::spawn(move || {
            for conn in listener.incoming() {
                if stop.load(Ordering::SeqCst) {
                    return;
                }
                let Ok(conn) = conn else { continue };
                if open.fetch_add(1, Ordering::SeqCst) >= MAX_CONNECTIONS {
                    open.fetch_sub(1, Ordering::SeqCst);
                    continue;
                }
                let (tools, call, open) =
                    (Arc::clone(&tools), Arc::clone(&call), Arc::clone(&open));
                thread::spawn(move || {
                    let peer = peer_pid(&conn);
                    if let Ok(w) = conn.try_clone() {
                        serve(&conn, w, &tools, |name, args| call(name, args, peer));
                    }
                    let _ = conn.shutdown(Shutdown::Both);
                    open.fetch_sub(1, Ordering::SeqCst);
                });
            }
        });
        Ok(Server {
            path: path.to_path_buf(),
            closing,
        })
    }

    pub fn path(&self) -> &Path {
        &self.path
    }
}

impl Drop for Server {
    fn drop(&mut self) {
        self.closing.store(true, Ordering::SeqCst);
        // Wakes the accept loop so it sees it is closing.
        let _ = UnixStream::connect(&self.path);
        let _ = std::fs::remove_file(&self.path);
    }
}

/// Copies stdin to the socket at `path` and the socket to stdout, until
/// either side closes. This is what the agent's CLI starts as imece's MCP
/// server.
pub fn relay(path: &Path) -> io::Result<()> {
    let sock = UnixStream::connect(path)?;
    let mut up = sock.try_clone()?;
    thread::spawn(move || {
        let _ = io::copy(&mut io::stdin().lock(), &mut up);
        let _ = up.shutdown(Shutdown::Write);
    });
    io::copy(&mut &sock, &mut io::stdout().lock())?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::{BufRead, BufReader};

    fn tools() -> Vec<Tool> {
        vec![Tool {
            name: "send".into(),
            description: "Sends a message.".into(),
            input: json!({ "type": "object" }),
        }]
    }

    fn run(lines: &[&str]) -> Vec<Value> {
        let input = lines.join("\n");
        let mut out = Vec::new();
        serve(
            input.as_bytes(),
            &mut out,
            &tools(),
            |name, args| match args["to"].as_str() {
                Some("nobody") => Err("no such agent".into()),
                to => Ok(format!("{name} to {}", to.unwrap_or_default())),
            },
        );
        String::from_utf8(out)
            .unwrap()
            .lines()
            .map(|l| serde_json::from_str(l).unwrap())
            .collect()
    }

    #[test]
    fn a_session_lists_and_calls_tools() {
        let got = run(&[
            r#"{"jsonrpc":"2.0","id":"probe","method":"server/discover","params":{}}"#,
            r#"{"jsonrpc":"2.0","id":0,"method":"initialize","params":{"protocolVersion":"2025-11-25"}}"#,
            r#"{"jsonrpc":"2.0","method":"notifications/initialized"}"#,
            r#"{"jsonrpc":"2.0","id":1,"method":"tools/list"}"#,
            r#"{"jsonrpc":"2.0","id":2,"method":"tools/call","params":{"name":"send","arguments":{"to":"claude-1"}}}"#,
            r#"{"jsonrpc":"2.0","id":3,"method":"tools/call","params":{"name":"send","arguments":{"to":"nobody"}}}"#,
            r#"{"jsonrpc":"2.0","id":4,"method":"tools/call","params":{"name":"Bash","arguments":{}}}"#,
        ]);
        assert_eq!(got.len(), 6, "the notification gets no reply: {got:?}");
        // An unknown method is refused, so a newer client falls back.
        assert_eq!(got[0]["id"], "probe");
        assert_eq!(got[0]["error"]["code"], -32601);
        assert_eq!(got[1]["result"]["protocolVersion"], "2025-11-25");
        assert_eq!(got[2]["result"]["tools"][0]["name"], "send");
        assert_eq!(
            got[2]["result"]["tools"][0]["inputSchema"]["type"],
            "object"
        );
        assert_eq!(got[3]["result"]["content"][0]["text"], "send to claude-1");
        assert_eq!(got[3]["result"]["isError"], false);
        assert_eq!(got[4]["result"]["content"][0]["text"], "no such agent");
        assert_eq!(got[4]["result"]["isError"], true);
        assert_eq!(got[5]["result"]["isError"], true, "only offered tools run");
    }

    #[test]
    fn broken_requests_get_errors_not_a_crash() {
        let got = run(&[
            "not json",
            r#"{"jsonrpc":"2.0","id":7}"#,
            r#"{"jsonrpc":"2.0","id":8,"method":"initialize","params":{"protocolVersion":"1999-01-01"}}"#,
        ]);
        assert_eq!(got[0]["error"]["code"], -32700);
        assert_eq!(got[1]["error"]["code"], -32600);
        assert_eq!(got[2]["result"]["protocolVersion"], VERSIONS[0]);
    }

    #[test]
    fn a_request_over_the_cap_ends_the_connection() {
        let long = format!(
            r#"{{"id":1,"method":"ping","x":"{}"}}"#,
            "a".repeat(MAX_LINE)
        );
        let got = run(&[&long, r#"{"jsonrpc":"2.0","id":2,"method":"ping"}"#]);
        assert!(got.is_empty(), "{got:?}");
    }

    #[test]
    fn the_process_on_the_other_end_is_told() {
        let (a, b) = UnixStream::pair().unwrap();
        assert_eq!(peer_pid(&a), Some(std::process::id()));
        assert_eq!(peer_pid(&b), Some(std::process::id()));
        assert_eq!(peer_told(0, 7), Some(7));
        assert_eq!(peer_told(0, 1), Some(1));
        assert_eq!(peer_told(0, 0), None, "no process is 0");
        assert_eq!(peer_told(-1, 7), None, "an error tells nothing");
        assert_eq!(peer_told(-1, 0), None);
    }

    #[test]
    fn the_socket_serves_calls_and_goes_away_when_dropped() {
        let dir = std::env::temp_dir().join(format!("imece-mcp-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("a1.sock");
        let _ = std::fs::remove_file(&path);
        let server = Server::bind(&path, tools(), |_, args, peer| {
            // The caller is this test's own process.
            if peer != Some(std::process::id()) {
                return Err(format!("called from {peer:?}"));
            }
            Ok(format!("hi {}", args["to"].as_str().unwrap_or_default()))
        })
        .unwrap();
        assert!(Server::bind(&path, tools(), |_, _, _| Ok(String::new())).is_err());

        let mut sock = UnixStream::connect(&path).unwrap();
        writeln!(
            sock,
            r#"{{"jsonrpc":"2.0","id":1,"method":"tools/call","params":{{"name":"send","arguments":{{"to":"lead"}}}}}}"#
        )
        .unwrap();
        let mut line = String::new();
        BufReader::new(&sock).read_line(&mut line).unwrap();
        let v: Value = serde_json::from_str(&line).unwrap();
        assert_eq!(v["result"]["content"][0]["text"], "hi lead");

        drop(server);
        assert!(!path.exists());
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn a_ping_is_answered_with_nothing_in_it() {
        let got = run(&[r#"{"jsonrpc":"2.0","id":5,"method":"ping"}"#]);
        assert_eq!(got.len(), 1);
        assert_eq!(got[0]["id"], 5);
        assert_eq!(got[0]["result"], serde_json::json!({}));
    }
}
