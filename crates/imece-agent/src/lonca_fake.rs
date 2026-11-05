//! A daemon that speaks the protocol, for tests here and in the screen
//! (feature `fake`; never in a build of imece).

use std::io::{BufRead, BufReader, Write};
use std::os::unix::fs::PermissionsExt;
use std::os::unix::net::UnixListener;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::thread;

use serde_json::{Value, json};

use crate::lonca::Place;

/// A `lonca` program that writes where it runs and its arguments to
/// [`seen`], then runs the shell text `body`; the place that finds it, in
/// a directory of its own, which the test removes.
pub fn program(body: &str) -> (Place, PathBuf) {
    static NEXT: AtomicU64 = AtomicU64::new(0);
    let n = NEXT.fetch_add(1, Ordering::Relaxed);
    let dir = std::env::temp_dir().join(format!("imece-cli-{}-{n}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let dir = dir.canonicalize().unwrap();
    let bin = dir.join("lonca");
    let script = format!(
        "#!/bin/sh\necho \"$PWD|$*|\" > \"{}/seen\"\n{body}\n",
        dir.display()
    );
    std::fs::write(&bin, script).unwrap();
    std::fs::set_permissions(&bin, std::fs::Permissions::from_mode(0o755)).unwrap();
    let place = Place {
        socket: dir.join("s"),
        humans: dir.join("humans"),
        human: None,
        program: bin.display().to_string(),
    };
    (place, dir)
}

/// What the [`program`] in `dir` was last run with.
pub fn seen(dir: &Path) -> String {
    std::fs::read_to_string(dir.join("seen")).unwrap()
}

/// A trunk the fake answers with.
pub const TRUNK: &str = "693d62952cd8f7ce58b0de780cf5d998b2f5c1a6";

/// The policy commit [`status`] names: it holds no settings file.
pub const POLICY: &str = "7dcb2a6f0e8c1d3b5a497f6e2c8d1b0a3e5f7c9d";

/// A daemon on a socket of its own: `answer` is called with each
/// request but `ping` (answered with `protocol`) and the reading of
/// [`POLICY`], and gives the `result`, or an error code. It ends with its
/// last connection.
pub struct Daemon {
    pub dir: PathBuf,
    pub place: Place,
    /// Every request it took, whole.
    pub heard: Arc<Mutex<Vec<Value>>>,
}

static NEXT: AtomicUsize = AtomicUsize::new(0);

impl Daemon {
    pub fn start(
        protocol: u64,
        answer: impl Fn(&Value) -> Result<Value, &'static str> + Send + Sync + 'static,
    ) -> Daemon {
        let n = NEXT.fetch_add(1, Ordering::Relaxed);
        let dir = std::env::temp_dir().join(format!("il{}-{n}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).expect("the fake's folder");
        let socket = dir.join("s");
        let listener = UnixListener::bind(&socket).expect("the fake's socket");
        let heard = Arc::new(Mutex::new(Vec::new()));
        let told = heard.clone();
        let answer = Arc::new(answer);
        thread::spawn(move || {
            for conn in listener.incoming().flatten() {
                let (told, answer) = (told.clone(), answer.clone());
                thread::spawn(move || {
                    let reader = BufReader::new(conn.try_clone().expect("a clone"));
                    let mut conn = conn;
                    for line in reader.lines().map_while(Result::ok) {
                        let asked: Value = serde_json::from_str(&line).expect("json");
                        let id = asked["id"].clone();
                        let reply = if asked["method"] == "ping" {
                            json!({"id": id, "ok": true, "result": {"protocol": protocol}})
                        } else if asked["method"] == "tree" && asked["params"]["commit"] == POLICY {
                            json!({"id": id, "ok": true, "result": []})
                        } else {
                            told.lock().expect("heard").push(asked.clone());
                            match answer(&asked) {
                                Ok(result) => json!({"id": id, "ok": true, "result": result}),
                                Err(code) => json!({"id": id, "ok": false,
                                    "error": {"code": code, "message": "no"}}),
                            }
                        };
                        let bytes = format!("{reply}\n");
                        if conn.write_all(bytes.as_bytes()).is_err() {
                            return;
                        }
                    }
                });
            }
        });
        let place = Place {
            socket,
            humans: dir.clone(),
            human: Some("ayse".into()),
            program: "lonca".into(),
        };
        Daemon { dir, place, heard }
    }

    /// A session file for `ayse` beside the socket, as `lonca human
    /// login` keeps it.
    pub fn sign_in(&self) {
        write_session(&self.dir.join("ayse.session"), i64::MAX);
    }
}

impl Drop for Daemon {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.dir);
    }
}

/// A session file at `path`, ending at `expires_ms`, mode 0600.
pub fn write_session(path: &Path, expires_ms: i64) {
    let text = json!({"token": "lonca_h_secret", "expires_ms": expires_ms});
    std::fs::write(path, text.to_string()).expect("the session");
    let only_me = std::fs::Permissions::from_mode(0o600);
    std::fs::set_permissions(path, only_me).expect("its mode");
}

/// A `status` result with `pending` entries of the protocol's shape.
pub fn status(pending: &[u64]) -> Value {
    let pending: Vec<Value> = (pending.iter())
        .map(|seq| {
            json!({"seq": seq, "created_ms": 5, "actor": "agent:a/s", "op": "change.propose",
                "reasons": ["unobserved_session"], "approvals": [], "required": 1, "change": "kkkk", "base": TRUNK,
                "intent": "fix", "agent": "a", "task": "T-1", "paths": ["src/a.rs"]})
        })
        .collect();
    json!({"repo": "demo", "trunk": TRUNK, "policy": POLICY, "next_seq": 9, "pending": pending,
        "changes": [], "sessions": [], "now_ms": 1000})
}
