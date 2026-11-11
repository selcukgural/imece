//! A daemon that speaks the protocol, for tests here and in the screen
//! (feature `fake`; never in a build of imece).

use std::io::{BufRead, BufReader, Write};
use std::os::unix::fs::PermissionsExt;
use std::os::unix::net::UnixListener;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::{Duration, Instant};

use serde_json::{Value, json};

use crate::lonca::Place;
use lonca_client::Trust;

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
        trust: Some(Trust::cooperative()),
    };
    (place, dir)
}

/// What the [`program`] in `dir` was last run with.
pub fn seen(dir: &Path) -> String {
    std::fs::read_to_string(dir.join("seen")).unwrap()
}

/// The version the fake's `ping` says: the oldest imece takes.
pub const VERSION: &str = "0.2.1";

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
    /// Every request it took, whole, but `events.watch`.
    pub heard: Arc<Mutex<Vec<Value>>>,
    /// Every `events.watch` it took, whole.
    pub watched: Arc<Mutex<Vec<Value>>>,
    /// What the next `events.watch` answers, oldest first: an answer
    /// waits up to [`WATCH_HOLD`] for one, then says nothing happened.
    pub news: Arc<Mutex<Vec<Value>>>,
}

/// The longest the fake holds an `events.watch`: a test does not wait the
/// daemon's 30 s, and an answer before the time is still the protocol's.
pub const WATCH_HOLD: Duration = Duration::from_millis(200);

/// The first version whose daemon knows `events.watch`.
pub const WATCHES: &str = "0.2.3";

static NEXT: AtomicUsize = AtomicUsize::new(0);

impl Daemon {
    pub fn start(
        protocol: u64,
        answer: impl Fn(&Value) -> Result<Value, &'static str> + Send + Sync + 'static,
    ) -> Daemon {
        Self::start_at(protocol, Some(VERSION), answer)
    }

    /// As [`Daemon::start`], its `ping` saying `version` (none: no field).
    pub fn start_at(
        protocol: u64,
        version: Option<&'static str>,
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
        let watched = Arc::new(Mutex::new(Vec::new()));
        let news: Arc<Mutex<Vec<Value>>> = Arc::new(Mutex::new(Vec::new()));
        let (watches, waiting) = (watched.clone(), news.clone());
        let knows_watch = (version.and_then(crate::lonca::version_of))
            .is_some_and(|v| Some(v) >= crate::lonca::version_of(WATCHES));
        let answer = Arc::new(answer);
        thread::spawn(move || {
            for conn in listener.incoming().flatten() {
                let (told, answer) = (told.clone(), answer.clone());
                let (watches, waiting) = (watches.clone(), waiting.clone());
                thread::spawn(move || {
                    let reader = BufReader::new(conn.try_clone().expect("a clone"));
                    let mut conn = conn;
                    for line in reader.lines().map_while(Result::ok) {
                        let asked: Value = serde_json::from_str(&line).expect("json");
                        let id = asked["id"].clone();
                        let reply = if asked["method"] == "ping" {
                            json!({"id": id, "ok": true, "result": {"protocol": protocol, "version": version}})
                        } else if asked["method"] == "tree" && asked["params"]["commit"] == POLICY {
                            json!({"id": id, "ok": true, "result": []})
                        } else if asked["method"] == "events.watch" {
                            watches.lock().expect("watched").push(asked.clone());
                            if knows_watch {
                                let result = watch_answer(&asked, &waiting);
                                json!({"id": id, "ok": true, "result": result})
                            } else {
                                json!({"id": id, "ok": false,
                                    "error": {"code": "unknown_method", "message": "no"}})
                            }
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
                        // As the daemon does: after these it reads nothing
                        // more on the connection.
                        if ["closed", "timeout"]
                            .contains(&reply["error"]["code"].as_str().unwrap_or(""))
                        {
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
            // The person's own daemon, whatever this machine's install says.
            trust: Some(Trust::cooperative()),
        };
        Daemon {
            dir,
            place,
            heard,
            watched,
            news,
        }
    }

    /// The next `events.watch` answers `answer`.
    pub fn tell(&self, answer: Value) {
        self.news.lock().expect("news").push(answer);
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

/// What the fake says to an `events.watch`: the oldest of `news` once
/// there is one within [`WATCH_HOLD`] (or the request's `wait_ms`, when
/// less), else nothing new since its `from`.
fn watch_answer(asked: &Value, news: &Mutex<Vec<Value>>) -> Value {
    let wait = asked["params"]["wait_ms"].as_u64().unwrap_or(0);
    let until = Instant::now() + WATCH_HOLD.min(Duration::from_millis(wait));
    loop {
        let mut queued = news.lock().expect("news");
        if !queued.is_empty() {
            return queued.remove(0);
        }
        drop(queued);
        if Instant::now() >= until {
            let next = asked["params"]["from"].as_u64().unwrap_or(9);
            return json!({"events": [], "next": next, "reserved": [], "ended": []});
        }
        thread::sleep(Duration::from_millis(5));
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
                "intent": "fix", "agent": "a", "task": "T-1", "paths": ["src/a.rs"],
                "checks": [], "runs": ["build", "tests", "api"]})
        })
        .collect();
    json!({"repo": "demo", "trunk": TRUNK, "policy": POLICY, "next_seq": 9, "pending": pending,
        "changes": [], "sessions": [], "leases": [], "now_ms": 1000})
}
