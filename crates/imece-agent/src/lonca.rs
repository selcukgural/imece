//! A client of Lonca's daemon (record 116), written against its
//! `docs/protocol.md`, protocol number [`PROTOCOL`]: one JSON object a line
//! over a Unix socket. imece reads through it and never writes to the store
//! (record 115: the daemon is the only writer). Everything here fails
//! closed: a socket that is not the person's, a daemon of another protocol
//! number, an answer that does not fit, and no token leaves before the
//! daemon was checked. Its words are data: what comes from the store reaches
//! the screen only through `clean`.
//!
//! What must be signed with the person's key is not done here: the `lonca`
//! program does it, as a child: a login or an import when the person asks
//! (record 117), a worker's seat when it starts (record 119), the policy of
//! a repository made just now (record 121).

use std::ffi::{OsStr, OsString};
use std::fs::{self, DirBuilder, OpenOptions};
use std::io::{self, BufRead, BufReader, Read, Write};
use std::os::unix::fs::{DirBuilderExt, FileTypeExt, MetadataExt, OpenOptionsExt};
use std::os::unix::net::UnixStream;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use serde_json::{Map, Value, json};

use crate::child;

/// The protocol number this client was written for: a daemon that says
/// another one is refused (the protocol's own rule).
pub const PROTOCOL: u64 = 1;
/// The longest line read: the daemon's own `MAX_LINE`.
const MAX_LINE: u64 = 96 << 20;
/// How long the daemon gets to answer. Its calls take a millisecond; one
/// that hangs must not hang who asked.
const TIMEOUT: Duration = Duration::from_secs(5);
/// The longest `wait_ms` the daemon holds a `log` request.
pub const LOG_WAIT_MOST: u64 = 30_000;
/// A session file is a token and a time; anything longer is not one.
const SESSION_MOST: u64 = 4096;
/// What a `lonca` child sees of our environment besides [`child::BASE_ENV`]:
/// where the daemon is and who the person is, the same as this client.
const CLI_ENV: &[&str] = &[
    "LONCA_HOME",
    "LONCA_SOCKET",
    "LONCA_HUMANS",
    "LONCA_HUMAN",
    "LONCA_SE",
];
/// How long `human login` gets: a Secure Enclave key waits for a finger.
const LOGIN_WAIT: Duration = Duration::from_secs(60);
/// How long a worker's seat gets: signed by the person, so the same finger,
/// and a whole tree written out.
const SEAT_WAIT: Duration = Duration::from_secs(120);
/// How long a session lasts: Lonca's own most, `sessions.max_ttl_minutes`;
/// a repository that allows less refuses it, in its words.
const SEAT_TTL: &str = "8h";
/// The harness a seat names in its signed `session.start` and its hook
/// reads events with: Lonca grants an observation level only to the
/// harness a session names, so a seat that named none would be
/// `unobserved` even where the harness's hooks are locked.
pub const HARNESS: &str = "claude-code";
/// How long a decision gets: signed by the person, so the same finger.
const DECIDE_WAIT: Duration = Duration::from_secs(120);
/// The longest reason `reject` takes, in bytes.
const REASON_MOST: usize = 4096;
/// How long a heartbeat gets: a millisecond call, or no daemon.
const BEAT_WAIT: Duration = Duration::from_secs(5);
/// What `lonca diff` says of a workspace as its base left it.
const NO_CHANGES: &str = "no local changes";
/// The file that makes a directory a workspace.
const WORKSPACE_FILE: &str = ".lonca/workspace.json";
/// The most of it read: it lists every tracked file of the tree.
const WORKSPACE_MOST: u64 = 64 << 20;
/// The longest agent name Lonca takes.
const AGENT_MOST: usize = 64;
/// How long an import gets: the daemon fetches the branch's whole history.
const IMPORT_WAIT: Duration = Duration::from_secs(600);
/// How long a fetch gets: a bundle of what the checkout lacks.
const FETCH_WAIT: Duration = Duration::from_secs(120);
/// Where `lonca fetch` keeps trunk in a git checkout.
pub const TRUNK_REF: &str = "refs/lonca/trunk";
/// The longest refusal kept of what `lonca` said, in characters.
const SAID_MOST: usize = 300;
/// The most warnings kept of an import.
const WARNINGS_MOST: usize = 8;

/// Why the store can not be read. Every one of them closes a project on
/// Lonca to hires and Δ decisions.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Down {
    /// Nothing answers on the socket.
    NoDaemon,
    /// Something is there, but not a daemon of the person's: not a socket,
    /// or another user's.
    NotTheDaemon,
    /// It answered outside the protocol, or with another protocol number.
    Protocol,
    /// No human identity, no session, or an ended one: `lonca human login`.
    SignedOut,
    /// A session the repository does not take: the person is not enrolled
    /// in it. The daemon says this of a repository it does not have too,
    /// checking the session first.
    Forbidden,
    /// Nothing of that name: what the daemon says past the session's check.
    NoRepo,
    /// The daemon refused for a reason of its own.
    Refused,
}

/// Why a `lonca` command of the person's did not go through.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Failed {
    /// No such program, or it ran out of time: nothing was said.
    NoAnswer,
    /// It refused, and said why: its last line, cut.
    Said(String),
}

/// Where the daemon and the person's identity are, as `lonca` finds them,
/// and the program that signs for the person.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Place {
    pub socket: PathBuf,
    pub humans: PathBuf,
    /// `LONCA_HUMAN`; none: the `default` file among the humans.
    pub human: Option<String>,
    /// The `lonca` program, found on `PATH`.
    pub program: String,
}

impl Place {
    /// From this process's environment.
    pub fn here() -> Place {
        Place::of(|key| std::env::var_os(key))
    }

    fn of(get: impl Fn(&str) -> Option<OsString>) -> Place {
        let path = |key: &str| get(key).map(PathBuf::from);
        let home = path("HOME").unwrap_or_else(|| PathBuf::from("."));
        let lonca = path("LONCA_HOME").unwrap_or_else(|| home.join(".lonca"));
        Place {
            socket: path("LONCA_SOCKET").unwrap_or_else(|| lonca.join("lonca.sock")),
            humans: path("LONCA_HUMANS").unwrap_or_else(|| home.join(".config/lonca/humans")),
            human: get("LONCA_HUMAN").and_then(|h| h.into_string().ok()),
            program: "lonca".to_string(),
        }
    }

    /// What `lonca ARGS` printed in `dir`; a refusal is its last line.
    /// Stdin is closed: nothing here can wait on a terminal.
    fn run(&self, dir: &Path, args: &[&OsStr], wait: Duration) -> Result<String, Failed> {
        let mut cmd = child::command(&self.program, dir, CLI_ENV);
        cmd.args(args);
        let (status, said) = child::ask(cmd, wait).ok_or(Failed::NoAnswer)?;
        if status.success() {
            Ok(said)
        } else {
            Err(Failed::Said(refusal(&said)))
        }
    }

    /// `lonca human login`: the person's session, which every read needs;
    /// the name it was opened for. The token stays in its file. With a
    /// Secure Enclave key this waits for a finger, so it runs only when
    /// the person asks and never on the screen's thread.
    pub fn login(&self, dir: &Path) -> Result<String, Failed> {
        let args = ["human", "login", "--json"].map(OsStr::new);
        let said = self.run(dir, &args, LOGIN_WAIT)?;
        Ok(signed_in(&said).unwrap_or_default())
    }

    /// `lonca init` of `repo` from `branch` of the checkout: Lonca's store
    /// gets the branch and its history. What it warned of comes back: a
    /// file too large for a proposal, for one. The caller has the person's
    /// yes; nothing can take this back.
    pub fn import(&self, repo: &str, checkout: &Path, branch: &str) -> Result<Vec<String>, Failed> {
        let args = [
            OsStr::new("init"),
            OsStr::new("--import"),
            checkout.as_os_str(),
            OsStr::new("--branch"),
            OsStr::new(branch),
            // A name may start with a dash, and is no option then.
            OsStr::new("--"),
            OsStr::new(repo),
        ];
        Ok(warnings(&self.run(checkout, &args, IMPORT_WAIT)?))
    }

    /// Has `repo`, made just now, take a change to a critical path to the
    /// person instead of refusing it: `unobserved_critical` and imece's bus
    /// in `mcp_servers` (see [`critical_opened`]), signed with the person's
    /// key under `lonca policy set`, which the same yes as the import covers
    /// (record 121). Lonca's rules go back as they were. A
    /// policy that is not the default one a new repository has, or can not
    /// be read, is left alone: no words, what stands is what was.
    pub fn critical_waits(&self, repo: &str, dir: &Path) -> Result<(), Failed> {
        let unread = |_| Failed::NoAnswer;
        let mut client = self.open().map_err(unread)?;
        let policy = client.status(repo).map_err(unread)?.policy;
        let config = (client.policy_file(repo, &policy, POLICY_FILE)).map_err(unread)?;
        let rules = (client.policy_file(repo, &policy, RULES_FILE)).map_err(unread)?;
        let (Some(config), Some(rules)) = (config.as_deref().and_then(critical_opened), rules)
        else {
            return Err(Failed::NoAnswer);
        };
        let files = [(POLICY_FILE, config.as_str()), (RULES_FILE, rules.as_str())];
        let set = with_files(&files, |at| {
            let named = format!("--repo={repo}");
            let config = format!("--config={}", at.join(POLICY_FILE).display());
            let rules = format!("--policy={}", at.join(RULES_FILE).display());
            let args = ["policy", "set", &config, &rules, &named].map(OsStr::new);
            self.run(dir, &args, DECIDE_WAIT).map(|_| ())
        });
        set.map_err(|e| Failed::Said(e.to_string()))?
    }

    /// `lonca fetch`: trunk of `repo` into [`TRUNK_REF`] of the checkout,
    /// through the daemon. Nothing else of the checkout moves.
    pub fn fetch(&self, repo: &str, checkout: &Path) -> Result<(), Failed> {
        // With `=`: a name may start with a dash.
        let named = format!("--repo={repo}");
        let args = [
            OsStr::new("fetch"),
            OsStr::new(&named),
            OsStr::new("--into"),
            checkout.as_os_str(),
        ];
        self.run(checkout, &args, FETCH_WAIT).map(|_| ())
    }

    /// `lonca approve`: the operation numbered `seq` of `repo` goes on.
    /// Signed by the person, who just said so: a Secure Enclave key waits
    /// for a finger.
    pub fn approve(&self, repo: &str, dir: &Path, seq: u64) -> Result<(), Failed> {
        let (seq, named) = (seq.to_string(), format!("--repo={repo}"));
        let args = ["approve", &seq, &named, "--json"].map(OsStr::new);
        self.run(dir, &args, DECIDE_WAIT).map(|_| ())
    }

    /// `lonca reject`: the operation numbered `seq` of `repo` does not go
    /// on, and its requester reads `reason`, cut to what Lonca takes.
    pub fn reject(&self, repo: &str, dir: &Path, seq: u64, reason: &str) -> Result<(), Failed> {
        let (seq, named) = (seq.to_string(), format!("--repo={repo}"));
        // With `=`: a reason may start with a dash.
        let why = format!("--reason={}", cut_bytes(reason, REASON_MOST));
        let args = ["reject", &seq, &named, &why, "--json"].map(OsStr::new);
        self.run(dir, &args, DECIDE_WAIT).map(|_| ())
    }

    /// `lonca session start`: a session of `seat.agent` on `seat.repo` and
    /// a workspace of trunk in `seat.workspace`, which must not be there
    /// yet; the token is in the workspace, and goes nowhere else. Signed
    /// by the person: a Secure Enclave key waits for a finger.
    pub fn sit(&self, seat: &Seat) -> Result<(), Failed> {
        self.seat(seat, OsStr::new("--workspace"))
    }

    /// `lonca session start --continue`: a session of `seat.agent` takes
    /// over the workspace, files, base and proposal as they stand; the
    /// session it had ends and takes its leases along. A taint stays.
    pub fn sit_again(&self, seat: &Seat) -> Result<(), Failed> {
        self.seat(seat, OsStr::new("--continue"))
    }

    fn seat(&self, seat: &Seat, how: &OsStr) -> Result<(), Failed> {
        let dir = seat.workspace.parent().unwrap_or(Path::new("/"));
        std::fs::create_dir_all(dir).map_err(|e| Failed::Said(e.to_string()))?;
        // With `=`: names may start with a dash.
        let repo = format!("--repo={}", seat.repo);
        let agent = format!("--agent={}", agent_word(seat.agent));
        let mut args: Vec<&OsStr> = [
            "session",
            "start",
            &repo,
            &agent,
            "--paths=**",
            "--ttl",
            SEAT_TTL,
            "--harness",
            HARNESS,
            // imece brings the hooks and the MCP entry itself
            // (`claude::hook_settings`), so Lonca writes none in the workspace.
            "--no-setup",
            "--json",
        ]
        .map(OsStr::new)
        .into();
        args.extend([how, seat.workspace.as_os_str()]);
        if let Some(scratch) = seat.scratch {
            args.extend([OsStr::new("--scratch"), scratch.as_os_str()]);
        }
        self.run(dir, &args, SEAT_WAIT).map(|_| ())
    }

    /// Whether the session of `workspace` is alive: a heartbeat in it. One
    /// over is `false`, and a word of [`sit_again`](Self::sit_again) away;
    /// no answer says nothing of the session.
    pub fn seated(&self, workspace: &Path) -> Result<bool, Failed> {
        let args = ["session", "heartbeat", "--json"].map(OsStr::new);
        match self.run(workspace, &args, BEAT_WAIT) {
            Ok(_) => Ok(true),
            Err(Failed::Said(said)) if session_over(&said) => Ok(false),
            Err(other) => Err(other),
        }
    }

    /// `lonca session end` in `workspace`: its worker left. A proposal of
    /// its stays, and what it pinned; its token and its leases go.
    pub fn stand_up(&self, workspace: &Path) -> Result<(), Failed> {
        let args = ["session", "end", "--json"].map(OsStr::new);
        self.run(workspace, &args, BEAT_WAIT).map(|_| ())
    }

    /// How many files `workspace` has that differ from its base. A
    /// session that ended with work no proposal carries left its count with
    /// the daemon, which says it without the session's token. Else as
    /// `lonca diff` counts them: no change is told without the daemon; what
    /// changed is counted by it under the session's token, so a workspace
    /// with changes and its session over says none (and the daemon notes a
    /// dead token): it holds something, and nobody can say what.
    pub fn left_in(&self, workspace: &Path) -> Option<usize> {
        let ended = self.session_at(workspace).filter(|s| !s.active);
        if let Some(total) = ended.and_then(|s| s.unproposed) {
            return Some(total);
        }
        let said = self.run(workspace, &[OsStr::new("diff")], BEAT_WAIT).ok()?;
        if said.trim() == NO_CHANGES {
            return Some(0);
        }
        let facts: Value = serde_json::from_str(&said).ok()?;
        let lists = ["added_files", "modified_files", "deleted_files"];
        (lists.iter()).try_fold(0, |n, list| Some(n + facts[list].as_array()?.len()))
    }

    /// Whether the daemon says the session of `workspace` is over: then its
    /// token is dead, and a command under it only fails. `false` when it can
    /// not say.
    pub fn ended(&self, workspace: &Path) -> bool {
        self.session_at(workspace).is_some_and(|s| !s.active)
    }

    /// What the daemon says, to the person, of the session the workspace
    /// at `dir` names; none when it can not say. The file names the
    /// session and proves nothing: the daemon's word is read.
    fn session_at(&self, dir: &Path) -> Option<SessionEnd> {
        let file = workspace_file(dir)?;
        let (id, repo) = (file["session"].as_str()?, file["repo"].as_str()?);
        let status = self.open().ok()?.status(repo).ok()?;
        (status.sessions.into_iter()).find(|s| s.id.as_deref() == Some(id))
    }

    /// Who the person is here: a name that can be a file's.
    fn who(&self) -> Option<String> {
        let name = match &self.human {
            Some(h) => h.clone(),
            None => {
                let read = fs::read_to_string(self.humans.join("default")).ok()?;
                read.trim().to_string()
            }
        };
        valid_human(&name).then_some(name)
    }

    /// A client of the daemon: the socket checked, the protocol's number
    /// read with nothing sent, then the person's session on every call.
    pub fn open(&self) -> Result<Client, Down> {
        // SAFETY: `geteuid` takes nothing and can not fail.
        let me = unsafe { libc::geteuid() };
        let mut client = Client::of(connect(&self.socket, me)?);
        client.check_protocol()?;
        let human = self.who().ok_or(Down::SignedOut)?;
        let file = self.humans.join(format!("{human}.session"));
        client.token = Some(read_session(&file, me, now_ms()).ok_or(Down::SignedOut)?);
        Ok(client)
    }
}

/// A worker's place on Lonca: what [`Place::sit`] asks for.
#[derive(Clone, Copy, Debug)]
pub struct Seat<'a> {
    pub repo: &'a str,
    /// The agent's name, as imece has it.
    pub agent: &'a str,
    pub workspace: &'a Path,
    /// A directory of the agent's own outside the workspace: writes there
    /// are no part of a proposal.
    pub scratch: Option<&'a Path>,
}

/// `name` as Lonca takes an agent's: ASCII letters, digits, `_ - .`, cut to
/// [`AGENT_MOST`]; what else a solo's name holds becomes `-`.
pub fn agent_word(name: &str) -> String {
    (name.chars())
        .map(|c| match c {
            c if c.is_ascii_alphanumeric() || matches!(c, '_' | '-' | '.') => c,
            _ => '-',
        })
        .take(AGENT_MOST)
        .collect()
}

/// Whether what `lonca` said of a heartbeat is the session being over.
fn session_over(said: &str) -> bool {
    said.contains("session_inactive") || said.contains("unauthenticated")
}

/// Whether `dir` is a Lonca workspace: no link, no directory in its place.
pub fn is_workspace(dir: &Path) -> bool {
    std::fs::symlink_metadata(dir.join(WORKSPACE_FILE)).is_ok_and(|m| m.is_file())
}

/// The repository the workspace `dir` is of; none when it is no workspace or
/// says no such thing. The file is within an agent's reach, so this tells
/// which session to ask for, and proves nothing: the daemon decides by token.
pub fn workspace_repo(dir: &Path) -> Option<String> {
    workspace_file(dir)?["repo"].as_str().map(str::to_string)
}

/// The change the workspace at `dir` last proposed: Lonca keeps it there.
pub fn workspace_submitted(dir: &Path) -> Option<String> {
    workspace_file(dir)?["submitted"][0]
        .as_str()
        .map(str::to_string)
}

/// What the workspace at `dir` says of itself.
fn workspace_file(dir: &Path) -> Option<Value> {
    if !is_workspace(dir) {
        return None;
    }
    let file = std::fs::File::open(dir.join(WORKSPACE_FILE)).ok()?;
    serde_json::from_reader(file.take(WORKSPACE_MOST)).ok()
}

/// The refusal in what `lonca` said: its last line, without the program's
/// name, cut to [`SAID_MOST`].
fn refusal(said: &str) -> String {
    let last = (said.lines().rev()).find(|l| !l.trim().is_empty());
    let line = last.unwrap_or_default().trim();
    let line = line.strip_prefix("lonca: ").unwrap_or(line);
    line.chars().take(SAID_MOST).collect()
}

/// The most of `text` that fits `most` bytes, on a character's edge.
fn cut_bytes(text: &str, most: usize) -> &str {
    let end = (0..=most.min(text.len()))
        .rev()
        .find(|&i| text.is_char_boundary(i));
    &text[..end.unwrap_or(0)]
}

/// Who `human login --json` signed in: stdout comes first, one object.
fn signed_in(said: &str) -> Option<String> {
    let line = said.lines().next()?;
    let told: Value = serde_json::from_str(line).ok()?;
    told["human"].as_str().map(str::to_string)
}

/// The warnings in what `lonca init` said, at most [`WARNINGS_MOST`].
fn warnings(said: &str) -> Vec<String> {
    (said.lines())
        .filter_map(|l| l.strip_prefix("warning: "))
        .take(WARNINGS_MOST)
        .map(str::to_string)
        .collect()
}

/// Whether `name` is a name that stays in its directory.
fn valid_human(name: &str) -> bool {
    !name.is_empty()
        && !name.starts_with('.')
        && (name.chars()).all(|c| c.is_ascii_alphanumeric() || matches!(c, '.' | '_' | '-'))
}

fn now_ms() -> i64 {
    let since = SystemTime::now().duration_since(UNIX_EPOCH);
    since.map_or(0, |d| i64::try_from(d.as_millis()).unwrap_or(i64::MAX))
}

/// The token of the session file at `path`, when it is `me`'s plain file,
/// nobody else can read it and it has not ended at `now`.
fn read_session(path: &Path, me: u32, now: i64) -> Option<String> {
    let file = OpenOptions::new()
        .read(true)
        .custom_flags(libc::O_NOFOLLOW | libc::O_NONBLOCK)
        .open(path)
        .ok()?;
    let meta = file.metadata().ok()?;
    if !meta.is_file() || meta.uid() != me || meta.mode() & 0o077 != 0 {
        return None;
    }
    let mut text = String::new();
    file.take(SESSION_MOST).read_to_string(&mut text).ok()?;
    let session: Value = serde_json::from_str(&text).ok()?;
    let ends = session["expires_ms"].as_i64()?;
    (ends > now)
        .then(|| session["token"].as_str().map(str::to_string))
        .flatten()
}

/// The socket at `path`, when it is a socket of `me`'s and `me` is who
/// listens on it: the token goes nowhere else.
fn connect(path: &Path, me: u32) -> Result<UnixStream, Down> {
    let meta = fs::symlink_metadata(path).map_err(|_| Down::NoDaemon)?;
    if !meta.file_type().is_socket() || meta.uid() != me {
        return Err(Down::NotTheDaemon);
    }
    let conn = UnixStream::connect(path).map_err(|_| Down::NoDaemon)?;
    if peer_uid(&conn) != Some(me) {
        return Err(Down::NotTheDaemon);
    }
    let timed = conn
        .set_read_timeout(Some(TIMEOUT))
        .and(conn.set_write_timeout(Some(TIMEOUT)));
    timed.map_err(|_| Down::NoDaemon)?;
    Ok(conn)
}

/// The user listening on the other end of `conn`, as the system tells it
/// (macOS `getpeereid`, Linux `SO_PEERCRED`).
fn peer_uid(conn: &UnixStream) -> Option<u32> {
    use std::os::fd::AsRawFd;
    let fd = conn.as_raw_fd();
    #[cfg(target_os = "macos")]
    let (got, uid) = {
        let (mut uid, mut gid) = (0, 0);
        // SAFETY: both pointers are to live integers of the right type.
        let got = unsafe { libc::getpeereid(fd, &raw mut uid, &raw mut gid) };
        (got, uid)
    };
    #[cfg(target_os = "linux")]
    let (got, uid) = {
        let mut cred = libc::ucred {
            pid: 0,
            uid: 0,
            gid: 0,
        };
        let mut len = libc::socklen_t::try_from(size_of::<libc::ucred>()).ok()?;
        let at = (&raw mut cred).cast();
        // SAFETY: `cred` and `len` are live and `len` says `cred`'s size.
        let got =
            unsafe { libc::getsockopt(fd, libc::SOL_SOCKET, libc::SO_PEERCRED, at, &raw mut len) };
        (got, cred.uid)
    };
    peer_told(got, uid)
}

/// The user the system told, when it answered `got`.
fn peer_told(got: libc::c_int, uid: u32) -> Option<u32> {
    (got == 0).then_some(uid)
}

/// One connection to the daemon. The token is never printed: there is no
/// `Debug` here.
pub struct Client {
    conn: BufReader<UnixStream>,
    token: Option<String>,
    id: u64,
}

impl Client {
    fn of(conn: UnixStream) -> Client {
        Client {
            conn: BufReader::new(conn),
            token: None,
            id: 0,
        }
    }

    fn check_protocol(&mut self) -> Result<(), Down> {
        let told = self.call("ping", None, json!({}))?;
        if told["protocol"].as_u64() == Some(PROTOCOL) {
            Ok(())
        } else {
            Err(Down::Protocol)
        }
    }

    /// `method` on `repo`, with the token when there is one; what the
    /// daemon answered, or why not.
    pub fn call(&mut self, method: &str, repo: Option<&str>, params: Value) -> Result<Value, Down> {
        self.id += 1;
        let mut line = Map::new();
        line.insert("id".into(), json!(self.id));
        line.insert("method".into(), json!(method));
        if let Some(repo) = repo {
            line.insert("repo".into(), json!(repo));
        }
        if let Some(token) = &self.token {
            line.insert("token".into(), json!(token));
        }
        line.insert("params".into(), params);
        let mut bytes = Value::Object(line).to_string().into_bytes();
        bytes.push(b'\n');
        let conn = self.conn.get_mut();
        conn.write_all(&bytes).map_err(|_| Down::NoDaemon)?;
        let mut answer = Vec::new();
        (&mut self.conn)
            .take(MAX_LINE)
            .read_until(b'\n', &mut answer)
            .map_err(|_| Down::NoDaemon)?;
        answered(&answer, self.id)
    }

    /// The repository's state: trunk and what waits for a human.
    pub fn status(&mut self, repo: &str) -> Result<Status, Down> {
        Status::of(&self.call("status", Some(repo), json!({}))?).ok_or(Down::Protocol)
    }

    /// How many integrations trunk has had since `base` was trunk; none
    /// when it never was: a commit of the person's own, for one.
    pub fn distance(&mut self, repo: &str, base: &str) -> Result<Option<u64>, Down> {
        let told = self.call("trunk.distance", Some(repo), json!({"base": base}))?;
        match &told["behind"] {
            Value::Null if told.get("behind").is_some() => Ok(None),
            behind => behind.as_u64().map(Some).ok_or(Down::Protocol),
        }
    }

    /// The files of `commit`, without their contents (the manifest).
    pub fn tree(&mut self, repo: &str, commit: &str) -> Result<Value, Down> {
        let params = json!({"commit": commit, "with_content": false, "paths": null});
        self.call("tree", Some(repo), params)
    }

    /// The record of `change`.
    pub fn change(&mut self, repo: &str, change: &str) -> Result<Value, Down> {
        self.call("change.get", Some(repo), json!({"change": change}))
    }

    /// Where `change` stands.
    pub fn standing(&mut self, repo: &str, change: &str) -> Result<Standing, Down> {
        Standing::of(&self.change(repo, change)?["status"]).ok_or(Down::Protocol)
    }

    /// What `change` does, as the daemon works it out in the store.
    pub fn diff(&mut self, repo: &str, change: &str) -> Result<Diff, Down> {
        let params = json!({"change": change, "format": "unified"});
        Diff::of(&self.call("change.diff", Some(repo), params)?).ok_or(Down::Protocol)
    }

    /// Whether the policy at commit `policy` keeps what a session nothing
    /// observes proposes for a human. Its `lonca.toml` is read from the
    /// store; a policy that has none has Lonca's defaults, which do.
    pub fn holds_unobserved(&mut self, repo: &str, policy: &str) -> Result<bool, Down> {
        let text = self.policy_file(repo, policy, POLICY_FILE)?;
        Ok(text.is_none_or(|t| unobserved_held(&t)))
    }

    /// The file `path` of the policy commit `policy`, as text; none when
    /// the commit has no such file.
    fn policy_file(
        &mut self,
        repo: &str,
        policy: &str,
        path: &str,
    ) -> Result<Option<String>, Down> {
        let params = json!({"commit": policy, "with_content": true, "paths": [path]});
        let told = self.call("tree", Some(repo), params)?;
        let files = told.as_array().ok_or(Down::Protocol)?;
        let Some(file) = files.iter().find(|f| f["path"] == path) else {
            return Ok(None);
        };
        let text = (file["content_b64"].as_str())
            .and_then(from_base64)
            .and_then(|b| String::from_utf8(b).ok());
        text.map(Some).ok_or(Down::Protocol)
    }

    /// The checks of `change`'s last integration attempt: none before one.
    pub fn evidence(&mut self, repo: &str, change: &str) -> Result<Vec<Evidence>, Down> {
        let told = self.change(repo, change)?;
        let all = told["evidence"].as_array().ok_or(Down::Protocol)?;
        (all.iter().map(Evidence::of))
            .collect::<Option<_>>()
            .ok_or(Down::Protocol)
    }

    /// The commands policy `policy` of `repo` runs on a change it has
    /// approved, by their keys: none, nothing does.
    pub fn verifies(&mut self, repo: &str, policy: &str) -> Result<Vec<String>, Down> {
        let text = self.policy_file(repo, policy, POLICY_FILE)?;
        Ok(text.map_or_else(Vec::new, |t| commands_in(&t)))
    }

    /// The log from `from`, at most `limit` entries. With `wait` (at most
    /// [`LOG_WAIT_MOST`]) the daemon holds the request until an entry
    /// comes; an empty array means the time ran out.
    pub fn log(
        &mut self,
        repo: &str,
        from: u64,
        limit: Option<u64>,
        wait: Option<Duration>,
    ) -> Result<Value, Down> {
        let wait_ms = wait.map(|d| log_wait(d.as_millis()));
        let params = json!({"from": from, "limit": limit, "wait_ms": wait_ms});
        let conn = self.conn.get_ref();
        (conn.set_read_timeout(Some(log_timeout(wait_ms)))).map_err(|_| Down::NoDaemon)?;
        let told = self.call("log", Some(repo), params);
        let conn = self.conn.get_ref();
        (conn.set_read_timeout(Some(TIMEOUT))).map_err(|_| Down::NoDaemon)?;
        told
    }
}

/// `millis` within what the daemon holds.
fn log_wait(millis: u128) -> u64 {
    u64::try_from(millis).map_or(LOG_WAIT_MOST, |m| m.min(LOG_WAIT_MOST))
}

/// How long a `log` that may be held `wait_ms` gets: the wait and the
/// daemon's usual time on top of it.
fn log_timeout(wait_ms: Option<u64>) -> Duration {
    TIMEOUT + Duration::from_millis(wait_ms.unwrap_or(0))
}

/// What the daemon's line `answer` to request `id` says. A line cut short
/// (no end: the cap or the connection) was no answer.
fn answered(answer: &[u8], id: u64) -> Result<Value, Down> {
    let Some((b'\n', line)) = answer.split_last() else {
        return Err(Down::NoDaemon);
    };
    let told: Value = serde_json::from_slice(line).map_err(|_| Down::Protocol)?;
    if told["id"].as_u64() != Some(id) {
        return Err(Down::Protocol);
    }
    match told["ok"].as_bool() {
        Some(true) => Ok(told["result"].clone()),
        Some(false) => Err(down_of(told["error"]["code"].as_str())),
        None => Err(Down::Protocol),
    }
}

/// What an error code of the daemon's means here; no code, the protocol.
fn down_of(code: Option<&str>) -> Down {
    match code {
        Some("unauthenticated") => Down::SignedOut,
        Some("forbidden") => Down::Forbidden,
        Some("not_found") => Down::NoRepo,
        Some(_) => Down::Refused,
        None => Down::Protocol,
    }
}

/// The file of a policy commit that holds the repository's settings.
const POLICY_FILE: &str = "lonca.toml";
/// The rules beside it.
const RULES_FILE: &str = "policy.cedar";
/// The keys of `[verification]` that name a command.
const COMMAND_KEYS: &[&str] = &["build", "test", "on_signature_change"];
/// What a critical path costs a session nobody fully observes.
const CRITICAL_KEY: &str = "unobserved_critical";
/// The MCP servers whose tools do not taint a session.
const UNTAINTED_KEY: &str = "mcp_servers";
/// What the log calls a proposal: [`Pending::op`].
pub const PROPOSE: &str = "change.propose";

/// The bytes `text` stands for in base64; none for a letter it has not.
fn from_base64(text: &str) -> Option<Vec<u8>> {
    let mut out = Vec::with_capacity(text.len() / 4 * 3);
    let (mut bits, mut held) = (0u32, 0u32);
    for b in text.bytes().filter(|&b| b != b'=') {
        let value = match b {
            b'A'..=b'Z' => b - b'A',
            b'a'..=b'z' => b - b'a' + 26,
            b'0'..=b'9' => b - b'0' + 52,
            b'+' => 62,
            b'/' => 63,
            _ => return None,
        };
        bits = (bits << 6) + u32::from(value);
        held += 6;
        if held >= 8 {
            held -= 8;
            out.push((bits >> held) as u8);
        }
    }
    Some(out)
}

/// The keys of `[verification]` in the settings `toml` that name a command:
/// a line of its own, not a comment.
fn commands_in(toml: &str) -> Vec<String> {
    let mut section = "";
    let mut found = Vec::new();
    for line in toml.lines() {
        let line = line.split('#').next().unwrap_or_default().trim();
        if let Some(name) = line.strip_prefix('[') {
            section = name.trim_end_matches(']').trim();
        } else if section == "verification"
            && let Some((key, _)) = line.split_once('=')
            && COMMAND_KEYS.contains(&key.trim())
        {
            found.push(key.trim().to_string());
        }
    }
    found
}

/// The settings `toml` with a critical path waiting for a human: for an
/// unobserved session, and imece's bus no longer tainting one, or the taint
/// would refuse the same change (record 121). None when they do not say what
/// Lonca's default says, its own lines, which a comment or another spelling
/// is not.
fn critical_opened(toml: &str) -> Option<String> {
    let line = |key: &str, value: &str| format!("\n{key} = {value}\n");
    let bus = format!("[\"{}\"]", crate::MCP_SERVER);
    [
        (
            line(CRITICAL_KEY, "\"deny\""),
            line(CRITICAL_KEY, "\"escalate\""),
        ),
        (line(UNTAINTED_KEY, "[]"), line(UNTAINTED_KEY, &bus)),
    ]
    .iter()
    .try_fold(toml.to_string(), |text, (from, to)| {
        (text.contains(from)).then(|| text.replacen(from, to, 1))
    })
}

/// `then` with `files` (name, text) in a directory of their own, only the
/// person's, gone afterwards.
fn with_files<T>(files: &[(&str, &str)], then: impl FnOnce(&Path) -> T) -> io::Result<T> {
    static NEXT: AtomicU64 = AtomicU64::new(0);
    let n = NEXT.fetch_add(1, Ordering::Relaxed);
    let name = format!("imece-policy-{}-{n}", std::process::id());
    let dir = std::env::temp_dir().join(name);
    // Not there before: never a directory someone else made ready.
    DirBuilder::new().mode(0o700).create(&dir)?;
    let wrote = (files.iter()).try_for_each(|(name, text)| fs::write(dir.join(name), text));
    let done = wrote.map(|()| then(&dir));
    let _ = fs::remove_dir_all(&dir);
    done
}

/// Whether the settings `toml` leave `unobserved_is_tainted` on, Lonca's
/// default. Off, nothing waits for a person: a change would integrate
/// unseen (record 120). So anything but a plain `true` for it is off, and a
/// spelling this does not know (a table written inline) is off too: only
/// the key's own line under `[observation]` is read as the setting.
fn unobserved_held(toml: &str) -> bool {
    const KEY: &str = "unobserved_is_tainted";
    let mut section = String::new();
    let mut held = true;
    for line in toml.lines() {
        let line = line.split('#').next().unwrap_or_default().trim();
        if let Some(name) = line.strip_prefix('[') {
            section = name.trim_end_matches(']').trim().to_string();
        } else if line.contains(KEY) || (line.starts_with("observation") && section.is_empty()) {
            let setting = (line.split_once('='))
                .filter(|(key, _)| key.trim() == KEY && section == "observation");
            held &= setting.is_some_and(|(_, value)| value.trim() == "true");
        }
    }
    held
}

/// What a change does, read from the store.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Diff {
    /// The change's commit.
    pub commit: String,
    pub files: Vec<DiffFile>,
    /// Files left out of `patch` because a blob of theirs was redacted.
    pub redacted: Vec<String>,
    /// The unified diff.
    pub patch: String,
    /// The daemon cut `patch` (at 2 MiB, on a line).
    pub cut: bool,
}

/// One file of a [`Diff`].
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DiffFile {
    pub path: String,
    /// Lines added and removed; none for a binary file.
    pub lines: Option<(u32, u32)>,
}

impl Diff {
    /// Fails closed, as [`Status::of`].
    fn of(v: &Value) -> Option<Diff> {
        let texts = |key: &str| -> Option<Vec<String>> {
            (v[key].as_array()?.iter())
                .map(|p| p.as_str().map(str::to_string))
                .collect()
        };
        Some(Diff {
            commit: v["commit"].as_str()?.to_string(),
            files: (v["files"].as_array()?.iter())
                .map(DiffFile::of)
                .collect::<Option<_>>()?,
            redacted: texts("redacted")?,
            patch: v["diff"].as_str()?.to_string(),
            cut: v["truncated"].as_bool()?,
        })
    }
}

impl DiffFile {
    fn of(v: &Value) -> Option<DiffFile> {
        let count = |key: &str| u32::try_from(v[key].as_u64()?).ok();
        Some(DiffFile {
            path: v["path"].as_str()?.to_string(),
            lines: count("added").zip(count("removed")),
        })
    }
}

/// What `status` says of a repository, the part imece reads.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Status {
    /// Trunk's commit: 40 hex digits.
    pub trunk: String,
    /// The commit that holds the repository's policy.
    pub policy: String,
    /// What is held now.
    pub leases: Vec<Lease>,
    /// The number the next log entry gets: a change here means something
    /// happened since the last look.
    pub next_seq: u64,
    pub pending: Vec<Pending>,
    /// Every change the repository has had and where each stands.
    pub changes: Vec<(String, Standing)>,
    /// The sessions the repository has had, ended ones too: the log keeps
    /// them, so one agent may have several.
    pub sessions: Vec<SessionEnd>,
    /// The daemon's clock.
    pub now_ms: i64,
}

/// Where a change stands in the store.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Standing {
    /// It waits for a human.
    Pending,
    /// Approved or clean, in the merge queue.
    Queued,
    /// On trunk, as this commit.
    Integrated(String),
    /// The queue could not merge these symbols.
    Conflicted(Vec<String>),
    /// A human or the verifier said no, and why.
    Rejected(Vec<String>),
    /// Its proposer took it back.
    Withdrawn,
    /// A state of a newer Lonca's: nothing is done on it.
    Other,
}

impl Standing {
    /// `v` is a change's `status` object.
    fn of(v: &Value) -> Option<Standing> {
        let list = |key: &str| -> Option<Vec<String>> {
            (v[key].as_array()?.iter())
                .map(|s| s.as_str().map(str::to_string))
                .collect()
        };
        Some(match v["status"].as_str()? {
            "pending" => Standing::Pending,
            "queued" => Standing::Queued,
            "integrated" => Standing::Integrated(v["commit"].as_str()?.to_string()),
            "conflicted" => Standing::Conflicted(list("symbols")?),
            "rejected" => Standing::Rejected(list("reasons")?),
            "withdrawn" => Standing::Withdrawn,
            _ => Standing::Other,
        })
    }
}

/// One check of a change's last integration attempt, as `change.get` has
/// it: the merge queue's, which a client never runs (record 122).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Evidence {
    /// `syntax`, `build`, `tests`, ...: Lonca's word.
    pub check: String,
    /// `pass`, `fail`, `error` or `skipped`.
    pub result: String,
    pub exit: Option<i64>,
    pub duration_ms: Option<u64>,
    /// The tail of its output.
    pub log: Option<String>,
}

impl Evidence {
    /// Fails closed on what the protocol promises; the rest may be null.
    fn of(v: &Value) -> Option<Evidence> {
        Some(Evidence {
            check: v["check"].as_str()?.to_string(),
            result: v["result"].as_str()?.to_string(),
            exit: v["exit"].as_i64(),
            duration_ms: v["duration_ms"].as_u64(),
            log: v["log"].as_str().map(str::to_string),
        })
    }
}

/// A symbol held now, as `status` lists it: live or pinned to a change.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct Lease {
    pub file: String,
    /// The symbol in the file; `<file>` for the whole of it.
    pub symbol: String,
    /// Its holder's name as Lonca took it; none for a session `status` does
    /// not list.
    pub agent: Option<String>,
}

/// What `v`, a `status`, says is held, each symbol of a holder once, by
/// file and symbol. Fails closed: a lease with no session or a symbol that
/// is no text.
fn leases_of(v: &Value) -> Option<Vec<Lease>> {
    let agent_of = |session: &str| {
        let sessions = v["sessions"].as_array()?.iter();
        sessions
            .filter(|s| s["id"] == session)
            .find_map(|s| s["agent"].as_str().map(str::to_string))
    };
    let mut found = Vec::new();
    for lease in v["leases"].as_array()? {
        let agent = agent_of(lease["session"].as_str()?);
        let pinned = lease["pinned"].as_array().into_iter().flatten();
        let lists = std::iter::once(&lease["items"]).chain(pinned.map(|p| &p["items"]));
        for list in lists {
            for item in list.as_array()? {
                let symbol = item["symbol"].as_str()?;
                let (file, symbol) = symbol.split_once('#')?;
                found.push(Lease {
                    file: file.to_string(),
                    symbol: symbol.to_string(),
                    agent: agent.clone(),
                });
            }
        }
    }
    found.sort();
    found.dedup();
    Some(found)
}

/// When a session of an agent's ends.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SessionEnd {
    /// Its id, which a workspace names.
    pub id: Option<String>,
    /// Its name, as Lonca took it: see [`agent_word`].
    pub agent: String,
    pub expires_ms: i64,
    /// Whether it is alive. One not said is taken as alive: its token is
    /// then tried, as before Lonca said it.
    pub active: bool,
    /// How many files the workspace held, when the session last stopped
    /// or ended, that no proposal carries; none when nothing was recorded
    /// (nothing left, or nobody reported it).
    pub unproposed: Option<usize>,
}

/// An operation waiting for a human.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Pending {
    /// The number `approve` and `reject` name.
    pub seq: u64,
    pub created_ms: i64,
    /// What it is, as the log names it ([`PROPOSE`]).
    pub op: String,
    /// Why it waits, as the policy said it (`unobserved_session`).
    pub reasons: Vec<String>,
    /// The change, its base and intent: only a proposal has them.
    pub change: Option<String>,
    pub base: Option<String>,
    pub intent: Option<String>,
    /// The agent and its task: only an agent session has them.
    pub agent: Option<String>,
    pub task: Option<String>,
    pub paths: Vec<String>,
}

impl Status {
    /// Fails closed: a field the protocol promises and is not there, or a
    /// trunk that is no commit id, is no answer.
    fn of(v: &Value) -> Option<Status> {
        let trunk = v["trunk"].as_str()?;
        if trunk.len() != 40 || !trunk.bytes().all(|b| b.is_ascii_hexdigit()) {
            return None;
        }
        let pending = v["pending"].as_array()?;
        let changes = v["changes"].as_array()?.iter();
        Some(Status {
            trunk: trunk.to_string(),
            policy: v["policy"].as_str()?.to_string(),
            next_seq: v["next_seq"].as_u64()?,
            pending: pending.iter().map(Pending::of).collect::<Option<_>>()?,
            changes: changes
                .map(|c| {
                    Some((
                        c["change"].as_str()?.to_string(),
                        Standing::of(&c["status"])?,
                    ))
                })
                .collect::<Option<_>>()?,
            sessions: (v["sessions"].as_array()?.iter())
                .map(SessionEnd::of)
                .collect::<Option<_>>()?,
            leases: leases_of(v)?,
            now_ms: v["now_ms"].as_i64()?,
        })
    }

    /// Each agent and how long its newest session has left, in
    /// milliseconds; below zero for one over.
    pub fn time_left(&self) -> Vec<(String, i64)> {
        let mut newest: Vec<(String, i64)> = Vec::new();
        for s in &self.sessions {
            let left = s.expires_ms - self.now_ms;
            match newest.iter_mut().find(|(agent, _)| *agent == s.agent) {
                Some((_, kept)) => *kept = left.max(*kept),
                None => newest.push((s.agent.clone(), left)),
            }
        }
        newest
    }
}

impl SessionEnd {
    fn of(v: &Value) -> Option<SessionEnd> {
        Some(SessionEnd {
            id: v["id"].as_str().map(str::to_string),
            agent: v["agent"].as_str()?.to_string(),
            expires_ms: v["expires_ms"].as_i64()?,
            active: v["active"].as_bool() != Some(false),
            unproposed: (v["unproposed"]["total"].as_u64()).and_then(|n| usize::try_from(n).ok()),
        })
    }
}

impl Pending {
    fn of(v: &Value) -> Option<Pending> {
        let text = |key: &str| v[key].as_str().map(str::to_string);
        let texts = |key: &str| -> Option<Vec<String>> {
            (v[key].as_array()?.iter())
                .map(|p| p.as_str().map(str::to_string))
                .collect()
        };
        Some(Pending {
            seq: v["seq"].as_u64()?,
            created_ms: v["created_ms"].as_i64()?,
            op: text("op")?,
            reasons: texts("reasons")?,
            change: text("change"),
            base: text("base"),
            intent: text("intent"),
            agent: text("agent"),
            task: text("task"),
            paths: texts("paths")?,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::lonca_fake::{Daemon, POLICY, TRUNK, program, seen, status, write_session};

    #[test]
    fn a_login_says_who_signed_in_and_never_waits_on_stdin() {
        let (place, dir) = program(
            "read -r line || true\n\
             printf '{\"human\":\"ayse\",\"expires_ms\":9}\\n'\n\
             echo note >&2",
        );
        assert_eq!(place.login(&dir).as_deref(), Ok("ayse"));
        let told = seen(&dir);
        assert!(told.contains("|human login --json|"), "{told}");
        assert!(told.starts_with(&format!("{}|", dir.display())), "{told}");
        fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn a_login_that_prints_no_name_still_went_through() {
        let (place, dir) = program("echo signed in");
        assert_eq!(place.login(&dir).as_deref(), Ok(""));
        fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn an_import_names_the_checkout_and_branch_and_keeps_the_warnings() {
        let (place, dir) = program(
            "echo '#0 allow'\n\
             echo 'warning: big.bin is 3 bytes, above the cap' >&2\n\
             echo 'a line of no matter' >&2",
        );
        let warned = place.import("-demo", &dir, "feature/x").unwrap();
        assert_eq!(warned, ["big.bin is 3 bytes, above the cap"]);
        let args = format!(
            "|init --import {} --branch feature/x -- -demo|",
            dir.display()
        );
        assert!(seen(&dir).contains(&args), "{}", seen(&dir));
        fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn a_refused_command_says_its_last_line_without_the_programs_name() {
        let (place, dir) =
            program("echo 'one' >&2\necho 'lonca: conflict: it exists' >&2\necho >&2\nexit 1");
        assert_eq!(
            place.import("demo", &dir, "main"),
            Err(Failed::Said("conflict: it exists".into()))
        );
        fs::remove_dir_all(dir).unwrap();
    }

    /// A daemon whose policy commit `P` holds `lonca.toml` and `policy.cedar`
    /// (base64 of `a=1` and `r`), and the program it is asked through.
    fn policy_daemon(config_b64: &'static str) -> (Daemon, PathBuf) {
        let mut d = Daemon::start(PROTOCOL, move |asked| {
            Ok(match asked["method"].as_str() {
                Some("status") => {
                    let mut v = status(&[]);
                    v["policy"] = json!("P");
                    v
                }
                _ => match asked["params"]["paths"][0].as_str() {
                    Some("lonca.toml") if asked["params"]["commit"] == "P" => {
                        json!([{"path": "lonca.toml", "content_b64": config_b64}])
                    }
                    Some("policy.cedar") if asked["params"]["commit"] == "P" => {
                        json!([{"path": "policy.cedar", "content_b64": "cg=="}])
                    }
                    _ => json!([]),
                },
            })
        });
        d.sign_in();
        let (fake, dir) = program(
            "for a; do case \"$a\" in\n\
             --config=*) cp \"${a#--config=}\" \"$(dirname \"$0\")/config\";;\n\
             --policy=*) cp \"${a#--policy=}\" \"$(dirname \"$0\")/rules\";;\n\
             esac; done",
        );
        d.place.program = fake.program;
        (d, dir)
    }

    /// Lonca's two default lines between two others, base64.
    const DEFAULT_B64: &str =
        "eAp1bm9ic2VydmVkX2NyaXRpY2FsID0gImRlbnkiCm1jcF9zZXJ2ZXJzID0gW10KeQo=";

    #[test]
    fn critical_waits_signs_the_settings_with_only_that_line_changed() {
        let (d, dir) = policy_daemon(DEFAULT_B64);
        assert_eq!(d.place.critical_waits("-demo", &dir), Ok(()));
        let args = seen(&dir);
        assert!(args.contains("|policy set --config="), "{args}");
        assert!(args.ends_with(" --repo=-demo|\n"), "{args}");
        assert_eq!(
            fs::read_to_string(dir.join("config")).unwrap(),
            "x\nunobserved_critical = \"escalate\"\nmcp_servers = [\"imece\"]\ny\n"
        );
        assert_eq!(fs::read_to_string(dir.join("rules")).unwrap(), "r");
        // Its copies are gone with the command.
        let at = args.split("--config=").nth(1).unwrap();
        let at = Path::new(at.split(' ').next().unwrap());
        assert!(!at.parent().unwrap().exists(), "{}", at.display());
        fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn critical_waits_touches_a_policy_it_does_not_know() {
        // `unobserved_critical = "escalate"` already, in base64.
        let (d, dir) = policy_daemon("CnVub2JzZXJ2ZWRfY3JpdGljYWwgPSAiZXNjYWxhdGUiCg==");
        assert_eq!(d.place.critical_waits("demo", &dir), Err(Failed::NoAnswer));
        assert!(!dir.join("seen").exists(), "no program ran");
        fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn critical_waits_needs_both_files_and_a_daemon() {
        let (d, dir) = policy_daemon(DEFAULT_B64);
        // No such commit: status names another policy.
        let d2 = Daemon::start(PROTOCOL, |_| Ok(status(&[])));
        d2.sign_in();
        let place = Place {
            program: d.place.program.clone(),
            ..d2.place.clone()
        };
        assert_eq!(place.critical_waits("demo", &dir), Err(Failed::NoAnswer));
        // A daemon gone is no answer either.
        let gone = Place {
            socket: dir.join("no-socket"),
            ..d.place.clone()
        };
        assert_eq!(gone.critical_waits("demo", &dir), Err(Failed::NoAnswer));
        assert!(!dir.join("seen").exists(), "no program ran");
        fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn a_refused_policy_says_why() {
        let (d, dir) = policy_daemon(DEFAULT_B64);
        let refused = program("echo 'lonca: denied: quorum' >&2\nexit 1");
        let place = Place {
            program: refused.0.program,
            ..d.place.clone()
        };
        assert_eq!(
            place.critical_waits("demo", &dir),
            Err(Failed::Said("denied: quorum".into()))
        );
        fs::remove_dir_all(dir).unwrap();
        fs::remove_dir_all(refused.1).unwrap();
    }

    #[test]
    fn the_critical_lines_are_read_only_as_lonca_writes_them() {
        let deny = "unobserved_critical = \"deny\"";
        let none = "mcp_servers = []";
        assert_eq!(
            critical_opened(&format!("a\n{deny}\nb\n{none}\nc\n")).as_deref(),
            Some("a\nunobserved_critical = \"escalate\"\nb\nmcp_servers = [\"imece\"]\nc\n")
        );
        assert_eq!(
            critical_opened(&format!("a\n{deny}\n{none}\nc\n")).as_deref(),
            Some("a\nunobserved_critical = \"escalate\"\nmcp_servers = [\"imece\"]\nc\n"),
            "side by side"
        );
        for other in [
            "".to_string(),
            format!("a\n{deny}\nb\n"),
            format!("a\n{none}\nb\n"),
            format!("a\nunobserved_critical = \"escalate\"\n{none}\nb\n"),
            format!("a\n# {deny}\n{none}\nb\n"),
            format!("a\n{deny}\nmcp_servers = [\"x\"]\nb\n"),
            format!("a\nnot_{deny}\n{none}\nb\n"),
        ] {
            assert_eq!(critical_opened(&other), None, "{other:?}");
        }
    }

    #[test]
    fn files_stand_for_the_call_in_a_directory_of_the_persons_alone() {
        let mut seen_at = PathBuf::new();
        let mode = with_files(&[("a", "one"), ("b", "two")], |at| {
            seen_at = at.to_path_buf();
            assert_eq!(fs::read_to_string(at.join("a")).unwrap(), "one");
            assert_eq!(fs::read_to_string(at.join("b")).unwrap(), "two");
            fs::metadata(at).unwrap().mode() & 0o777
        });
        assert_eq!(mode.unwrap(), 0o700);
        assert!(!seen_at.exists());
    }

    #[test]
    fn evidence_is_read_from_the_change_and_fails_closed() {
        let d = Daemon::start(PROTOCOL, |asked| {
            Ok(match asked["params"]["change"].as_str() {
                Some("two") => json!({"evidence": [
                    {"check": "syntax", "result": "pass", "exit": null, "duration_ms": null,
                     "log": null},
                    {"check": "build", "result": "fail", "exit": 3, "duration_ms": 1036,
                     "log": "checking"},
                ]}),
                Some("none") => json!({"evidence": []}),
                Some("short") => json!({"evidence": [{"check": "build"}]}),
                Some("nameless") => json!({"evidence": [{"result": "pass"}]}),
                _ => json!({}),
            })
        });
        d.sign_in();
        let mut c = d.place.open().expect("opens");
        assert_eq!(
            c.evidence("demo", "two").unwrap(),
            [
                Evidence {
                    check: "syntax".into(),
                    result: "pass".into(),
                    exit: None,
                    duration_ms: None,
                    log: None,
                },
                Evidence {
                    check: "build".into(),
                    result: "fail".into(),
                    exit: Some(3),
                    duration_ms: Some(1036),
                    log: Some("checking".into()),
                },
            ]
        );
        assert_eq!(c.evidence("demo", "none"), Ok(vec![]));
        for wrong in ["short", "nameless", "gone"] {
            assert_eq!(c.evidence("demo", wrong), Err(Down::Protocol), "{wrong}");
        }
    }

    #[test]
    fn the_commands_of_a_policy_are_those_of_its_verification() {
        let d = Daemon::start(PROTOCOL, |asked| {
            Ok(match asked["method"].as_str() {
                // `[verification]\ntest = "t"\n`
                _ if asked["params"]["commit"] == "P" => json!([{
                    "path": "lonca.toml", "content_b64": "W3ZlcmlmaWNhdGlvbl0KdGVzdCA9ICJ0Igo="
                }]),
                _ => json!([]),
            })
        });
        d.sign_in();
        let mut c = d.place.open().expect("opens");
        assert_eq!(c.verifies("demo", "P"), Ok(vec!["test".to_string()]));
        assert_eq!(c.verifies("demo", "other"), Ok(vec![]), "no settings file");
    }

    #[test]
    fn a_command_is_a_key_of_verification_and_no_comment() {
        let toml = "[a]\nbuild = \"no\"\n[verification]\n# build = \"x\"\n\
                    test = \"t\" # note\nrequired = true\n  build = \"b\"\n\
                    on_signature_change = \"o\"\nbare\n[b]\ntest = \"no\"\n";
        assert_eq!(commands_in(toml), ["test", "build", "on_signature_change"]);
        assert_eq!(commands_in(""), Vec::<String>::new());
    }

    #[test]
    fn a_fetch_names_the_repository_and_the_checkout() {
        let (place, dir) = program("exit 0");
        assert_eq!(place.fetch("-demo", &dir), Ok(()));
        let args = format!("|fetch --repo=-demo --into {}|", dir.display());
        assert!(seen(&dir).contains(&args), "{}", seen(&dir));
        fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn a_refused_fetch_says_why() {
        let (place, dir) = program("echo 'lonca: redacted' >&2\nexit 1");
        assert_eq!(
            place.fetch("demo", &dir),
            Err(Failed::Said("redacted".into()))
        );
        fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn a_distance_is_a_number_a_commit_that_never_was_trunk_or_no_answer() {
        let d = Daemon::start(PROTOCOL, |asked| match asked["params"]["base"].as_str() {
            Some("three") => Ok(json!({"behind": 3})),
            Some("own") => Ok(json!({"behind": null})),
            Some("word") => Ok(json!({"behind": "many"})),
            _ => Ok(json!({})),
        });
        d.sign_in();
        let mut c = d.place.open().unwrap();
        assert_eq!(c.distance("demo", "three"), Ok(Some(3)));
        assert_eq!(c.distance("demo", "own"), Ok(None));
        assert_eq!(c.distance("demo", "word"), Err(Down::Protocol));
        assert_eq!(c.distance("demo", "nothing"), Err(Down::Protocol));
    }

    /// A seat of `claude-1` on a repository whose name starts with a dash.
    fn seat_in(dir: &Path) -> Seat<'_> {
        Seat {
            repo: "-kule",
            agent: "claude-1",
            workspace: dir,
            scratch: None,
        }
    }

    #[test]
    fn a_seat_is_a_session_of_the_whole_tree_for_as_long_as_lonca_allows() {
        let (place, dir) = program("echo '{}'");
        let at = dir.join("trees/claude-1");
        let scratch = dir.join("scratch");
        let mut seat = seat_in(&at);
        seat.scratch = Some(&scratch);
        place.sit(&seat).unwrap();
        assert_eq!(
            seen(&dir).trim_end(),
            format!(
                "{}/trees|session start --repo=-kule --agent=claude-1 --paths=** --ttl 8h \
                 --harness claude-code --no-setup --json --workspace {} --scratch {}|",
                dir.display(),
                at.display(),
                scratch.display()
            )
        );
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn a_seat_taken_again_names_the_workspace_and_may_have_no_scratch() {
        let (place, dir) = program("echo '{}'");
        let at = dir.join("trees/claude-1");
        place.sit_again(&seat_in(&at)).unwrap();
        let said = seen(&dir);
        // A session that goes on names its harness as a new one does.
        assert!(
            said.contains(" --ttl 8h --harness claude-code --no-setup --json "),
            "{said}"
        );
        assert!(
            said.trim_end()
                .ends_with(&format!("--json --continue {}|", at.display())),
            "{said}"
        );
        assert!(!said.contains("--scratch"), "{said}");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn a_refused_seat_says_why() {
        let (place, dir) = program("echo 'lonca: denied: session_ttl' >&2; exit 1");
        let at = dir.join("w");
        assert_eq!(
            place.sit(&seat_in(&at)),
            Err(Failed::Said("denied: session_ttl".into()))
        );
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn a_seat_where_no_folder_can_be_says_so() {
        let (place, dir) = program("echo '{}'");
        std::fs::write(dir.join("file"), "").unwrap();
        let at = dir.join("file/w");
        assert!(matches!(place.sit(&seat_in(&at)), Err(Failed::Said(_))));
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn a_heartbeat_tells_a_session_over_from_a_daemon_gone() {
        let (place, dir) = program("echo '{}'");
        assert_eq!(place.seated(&dir), Ok(true));
        assert_eq!(
            seen(&dir).trim_end(),
            format!("{}|session heartbeat --json|", dir.display())
        );
        let _ = std::fs::remove_dir_all(&dir);
        for (said, alive) in [
            (
                "session_inactive: this session has ended or expired",
                Ok(false),
            ),
            (
                "session.heartbeat: unauthenticated: no such token",
                Ok(false),
            ),
            (
                "cannot reach the Lonca daemon at /s",
                Err(Failed::Said("cannot reach the Lonca daemon at /s".into())),
            ),
        ] {
            let (place, dir) = program(&format!("echo 'lonca: {said}' >&2; exit 1"));
            assert_eq!(place.seated(&dir), alive, "{said}");
            let _ = std::fs::remove_dir_all(&dir);
        }
    }

    #[test]
    fn a_worker_that_left_ends_its_session_in_its_workspace() {
        let (place, dir) = program("echo '{}'");
        assert_eq!(place.stand_up(&dir), Ok(()));
        assert_eq!(
            seen(&dir).trim_end(),
            format!("{}|session end --json|", dir.display())
        );
        let _ = std::fs::remove_dir_all(&dir);
        let (place, dir) = program("echo 'lonca: unauthenticated: gone' >&2; exit 1");
        assert!(matches!(place.stand_up(&dir), Err(Failed::Said(_))));
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// A workspace in a folder of its own naming session `ses_1` of `demo`,
    /// a daemon whose status lists `sessions`, and a `lonca` program that
    /// says `body`.
    fn ended_at(sessions: Value, body: &str) -> (Place, PathBuf, crate::lonca_fake::Daemon) {
        let d = crate::lonca_fake::Daemon::start(PROTOCOL, move |_| {
            let mut v = crate::lonca_fake::status(&[]);
            v["sessions"] = sessions.clone();
            Ok(v)
        });
        d.sign_in();
        let (mut place, dir) = program(body);
        place.socket = d.place.socket.clone();
        place.humans = d.place.humans.clone();
        place.human = d.place.human.clone();
        std::fs::create_dir_all(dir.join(".lonca")).unwrap();
        let file = json!({"repo": "demo", "session": "ses_1"});
        std::fs::write(dir.join(WORKSPACE_FILE), file.to_string()).unwrap();
        (place, dir, d)
    }

    fn session_json(id: &str, active: bool, unproposed: Option<u64>) -> Value {
        let unproposed = unproposed
            .map(|total| json!({"paths": ["a"], "total": total, "rounds": 1, "seen_ms": 5}));
        json!({"id": id, "agent": "w", "expires_ms": 9, "active": active,
            "unproposed": unproposed})
    }

    #[test]
    fn an_ended_session_s_record_is_its_workspace_s_count_and_runs_nothing() {
        let says = r#"echo '{"added_files":["a"],"modified_files":[],"deleted_files":[]}'"#;
        let (place, dir, _d) = ended_at(json!([session_json("ses_1", false, Some(3))]), says);
        assert_eq!(place.left_in(&dir), Some(3));
        assert!(
            !dir.join("seen").exists(),
            "no lonca diff under a dead token"
        );
        assert!(place.ended(&dir));
        let _ = std::fs::remove_dir_all(&dir);
        // No record, another session's record, or one alive: lonca diff counts.
        for sessions in [
            json!([session_json("ses_1", false, None)]),
            json!([session_json("ses_2", false, Some(3))]),
            json!([session_json("ses_1", true, Some(3))]),
            json!([{"agent": "w", "expires_ms": 9}]),
        ] {
            let (place, dir, _d) = ended_at(sessions.clone(), says);
            assert_eq!(place.left_in(&dir), Some(1), "{sessions}");
            assert_eq!(seen(&dir).trim_end(), format!("{}|diff|", dir.display()));
            let alive = sessions[0]["active"] != json!(false) || sessions[0]["id"] != "ses_1";
            assert_eq!(place.ended(&dir), !alive, "{sessions}");
            let _ = std::fs::remove_dir_all(&dir);
        }
        // A daemon that can not be asked says nothing of the session.
        let (mut place, dir, _d) = ended_at(json!([session_json("ses_1", false, Some(3))]), says);
        place.socket = dir.join("nobody");
        assert_eq!(place.left_in(&dir), Some(1));
        assert!(!place.ended(&dir));
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn what_a_workspace_holds_is_lonca_s_count_or_nothing_said() {
        let (place, dir) = program(
            r#"echo '{"added_files":["a","b"],"modified_files":["c"],"deleted_files":["d","e","f"]}'"#,
        );
        assert_eq!(place.left_in(&dir), Some(6));
        assert_eq!(seen(&dir).trim_end(), format!("{}|diff|", dir.display()));
        let _ = std::fs::remove_dir_all(&dir);
        let (place, dir) = program("echo 'no local changes'");
        assert_eq!(place.left_in(&dir), Some(0));
        let _ = std::fs::remove_dir_all(&dir);
        for body in [
            "echo 'lonca: facts.preview: unauthenticated: unknown' >&2; exit 1",
            "echo 'no json'",
            "echo 'no local changes here'",
            r#"echo '{"added_files":[],"modified_files":[]}'"#,
        ] {
            let (place, dir) = program(body);
            assert_eq!(place.left_in(&dir), None, "{body}");
            let _ = std::fs::remove_dir_all(&dir);
        }
    }

    #[test]
    fn an_agents_name_goes_as_lonca_takes_it() {
        assert_eq!(agent_word("claude-1_a.b"), "claude-1_a.b");
        assert_eq!(agent_word("solo-şu ad"), "solo--u-ad");
        assert_eq!(agent_word(&"x".repeat(70)).len(), AGENT_MOST);
        assert_eq!(agent_word(&"x".repeat(AGENT_MOST)).len(), 64);
    }

    #[test]
    fn a_workspace_is_a_directory_with_lonca_s_file_in_it() {
        let dir = std::env::temp_dir().join(format!("imece-ws-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(dir.join(".lonca")).unwrap();
        assert!(!is_workspace(&dir), "no file");
        assert_eq!(workspace_repo(&dir), None);

        let file = dir.join(WORKSPACE_FILE);
        std::fs::write(&file, r#"{"repo":"kule","base":"b"}"#).unwrap();
        assert!(is_workspace(&dir));
        assert_eq!(workspace_repo(&dir).as_deref(), Some("kule"));

        assert_eq!(workspace_submitted(&dir), None, "proposed nothing");
        std::fs::write(&file, r#"{"repo":"kule","submitted":null}"#).unwrap();
        assert_eq!(workspace_submitted(&dir), None, "as Lonca writes that");
        std::fs::write(&file, r#"{"submitted":["kkkk","c1"]}"#).unwrap();
        assert_eq!(workspace_submitted(&dir).as_deref(), Some("kkkk"));

        std::fs::write(&file, r#"{"base":"b"}"#).unwrap();
        assert_eq!(workspace_repo(&dir), None, "says no repository");
        std::fs::write(&file, "not json").unwrap();
        assert_eq!(workspace_repo(&dir), None);

        // A link is no workspace: this file is within an agent's reach.
        std::fs::remove_file(&file).unwrap();
        let elsewhere = dir.join("elsewhere");
        std::fs::write(&elsewhere, r#"{"repo":"kule"}"#).unwrap();
        std::os::unix::fs::symlink(&elsewhere, &file).unwrap();
        assert!(!is_workspace(&dir));
        assert_eq!(workspace_repo(&dir), None);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn no_program_is_no_answer() {
        let (mut place, dir) = program("exit 0");
        place.program = dir.join("nothing").display().to_string();
        assert_eq!(place.login(&dir), Err(Failed::NoAnswer));
        fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn what_lonca_said_is_read_within_its_caps() {
        assert_eq!(refusal(""), "");
        assert_eq!(refusal("  lonca: x  \n \n"), "x");
        assert_eq!(refusal("no name here"), "no name here");
        assert_eq!(
            refusal(&"é".repeat(SAID_MOST + 9)).chars().count(),
            SAID_MOST
        );
        assert_eq!(refusal(&"é".repeat(SAID_MOST)).chars().count(), SAID_MOST);
        assert_eq!(signed_in("not json\n{\"human\":\"a\"}"), None);
        assert_eq!(signed_in("{\"who\":\"a\"}"), None);
        assert_eq!(signed_in(""), None);
        let many = "warning: x\n".repeat(WARNINGS_MOST + 3);
        assert_eq!(warnings(&many).len(), WARNINGS_MOST);
        assert_eq!(warnings("nothing\nwarnings: no\n"), Vec::<String>::new());
    }

    fn me() -> u32 {
        // SAFETY: as in `Place::open`.
        unsafe { libc::geteuid() }
    }

    fn up() -> Daemon {
        let d = Daemon::start(PROTOCOL, |_| Ok(status(&[])));
        d.sign_in();
        d
    }

    #[test]
    fn the_place_is_found_as_lonca_finds_it() {
        let of = |vars: &'static [(&str, &str)]| {
            Place::of(move |key| {
                let found = vars.iter().find(|(k, _)| *k == key);
                found.map(|(_, v)| OsString::from(v))
            })
        };
        let bare = of(&[("HOME", "/h")]);
        assert_eq!(bare.socket, Path::new("/h/.lonca/lonca.sock"));
        assert_eq!(bare.humans, Path::new("/h/.config/lonca/humans"));
        assert_eq!(bare.program, "lonca");
        assert_eq!(bare.human, None);
        let home = of(&[("HOME", "/h"), ("LONCA_HOME", "/l")]);
        assert_eq!(home.socket, Path::new("/l/lonca.sock"));
        let all = of(&[
            ("HOME", "/h"),
            ("LONCA_HOME", "/l"),
            ("LONCA_SOCKET", "/s"),
            ("LONCA_HUMANS", "/u"),
            ("LONCA_HUMAN", "ayse"),
        ]);
        assert_eq!(all.socket, Path::new("/s"), "the socket wins over the home");
        assert_eq!(all.humans, Path::new("/u"));
        assert_eq!(all.human.as_deref(), Some("ayse"));
        assert_eq!(of(&[]).socket, Path::new("./.lonca/lonca.sock"));
    }

    #[test]
    fn a_human_is_a_name_that_stays_in_its_folder() {
        for good in ["ayse", "a.b_c-1"] {
            assert!(valid_human(good), "{good}");
        }
        for bad in ["", ".hidden", "../x", "a/b", "a b"] {
            assert!(!valid_human(bad), "{bad}");
        }
        let d = up();
        let mut place = d.place.clone();
        place.human = Some("../ayse".into());
        assert_eq!(place.who(), None, "not out of the folder");
        place.human = None;
        assert_eq!(place.who(), None, "no default file");
        fs::write(d.dir.join("default"), "ayse\n").expect("default");
        assert_eq!(place.who().as_deref(), Some("ayse"), "the file, trimmed");
    }

    #[test]
    fn nothing_on_the_socket_is_no_daemon() {
        let d = up();
        let mut place = d.place.clone();
        place.socket = d.dir.join("nobody");
        assert_eq!(place.open().err(), Some(Down::NoDaemon));
        // A socket nobody listens on any more.
        let gone = d.dir.join("gone");
        drop(std::os::unix::net::UnixListener::bind(&gone).expect("bind"));
        place.socket = gone;
        assert_eq!(place.open().err(), Some(Down::NoDaemon));
    }

    #[test]
    fn only_a_socket_of_the_persons_gets_a_word() {
        let d = up();
        // A plain file there is no daemon of theirs.
        let plain = d.dir.join("plain");
        fs::write(&plain, "x").expect("plain");
        assert_eq!(connect(&plain, me()).err(), Some(Down::NotTheDaemon));
        // A link to the real one is not the socket either.
        let link = d.dir.join("link");
        std::os::unix::fs::symlink(&d.place.socket, &link).expect("link");
        assert_eq!(connect(&link, me()).err(), Some(Down::NotTheDaemon));
        // Another user's socket: refused before connecting.
        let other = connect(&d.place.socket, me() + 1);
        assert_eq!(other.err(), Some(Down::NotTheDaemon));
        assert!(connect(&d.place.socket, me()).is_ok());
    }

    #[test]
    fn a_peer_is_who_the_system_says_when_it_says() {
        assert_eq!(peer_told(0, 5), Some(5));
        assert_eq!(peer_told(-1, 5), None);
    }

    #[test]
    fn the_peer_is_who_listens() {
        let d = up();
        let conn = connect(&d.place.socket, me()).expect("connects");
        assert_eq!(peer_uid(&conn), Some(me()));
        let (a, _b) = UnixStream::pair().expect("a pair");
        assert_eq!(peer_uid(&a), Some(me()));
    }

    #[test]
    fn no_token_goes_to_a_daemon_of_another_protocol() {
        let d = Daemon::start(PROTOCOL + 1, |_| Ok(status(&[])));
        d.sign_in();
        assert_eq!(d.place.open().err(), Some(Down::Protocol));
        assert!(d.heard.lock().expect("heard").is_empty());
    }

    #[test]
    fn an_answer_with_no_result_is_nothing() {
        assert_eq!(answered(b"{\"id\":1,\"ok\":true}\n", 1), Ok(Value::Null));
    }

    #[test]
    fn without_a_session_nothing_is_asked() {
        let d = Daemon::start(PROTOCOL, |_| Ok(status(&[])));
        assert_eq!(d.place.open().err(), Some(Down::SignedOut), "no file");
        // The moment that matters is now: this ended long ago.
        write_session(&d.dir.join("ayse.session"), 2);
        assert_eq!(d.place.open().err(), Some(Down::SignedOut), "ended");
        d.sign_in();
        let mut c = d.place.open().expect("opens");
        c.status("demo").expect("reads");
        let heard = d.heard.lock().expect("heard");
        assert_eq!(heard[0]["token"], "lonca_h_secret");
        assert_eq!(heard[0]["repo"], "demo");
        assert_eq!(heard[0]["method"], "status");
        assert_eq!(heard[0]["id"], 2, "after the ping");
        assert_eq!(heard[0]["params"], json!({}));
    }

    #[test]
    fn a_session_file_is_the_persons_own_plain_file() {
        let d = up();
        let file = d.dir.join("mine.session");
        write_session(&file, 100);
        assert_eq!(
            read_session(&file, me(), 99).as_deref(),
            Some("lonca_h_secret")
        );
        assert_eq!(read_session(&file, me(), 100), None, "ends at its time");
        assert_eq!(read_session(&file, me() + 1, 99), None, "someone else's");
        for mode in [0o640, 0o604, 0o610, 0o601] {
            let wide = fs::Permissions::from_mode(mode);
            fs::set_permissions(&file, wide).expect("mode");
            assert_eq!(read_session(&file, me(), 99), None, "{mode:o}");
        }
        fs::set_permissions(&file, fs::Permissions::from_mode(0o600)).expect("mode");
        let link = d.dir.join("link.session");
        std::os::unix::fs::symlink(&file, &link).expect("link");
        assert_eq!(read_session(&link, me(), 99), None, "a link");
        assert_eq!(read_session(&d.dir, me(), 99), None, "a folder");
        fs::write(
            &file,
            format!("{{\"token\":\"{}\",\"expires_ms\":100}}", "t".repeat(5000)),
        )
        .expect("long");
        assert_eq!(read_session(&file, me(), 99), None, "too long to be one");
        fs::write(&file, "{\"expires_ms\":100}").expect("short");
        assert_eq!(read_session(&file, me(), 99), None, "no token");
        fs::write(&file, "{\"token\":\"t\"}").expect("short");
        assert_eq!(read_session(&file, me(), 99), None, "no end");
    }

    use std::os::unix::fs::PermissionsExt;

    #[test]
    fn an_answer_is_read_by_the_protocol() {
        let line = |s: &str| format!("{s}\n").into_bytes();
        let good = line(r#"{"id":3,"ok":true,"result":{"a":1}}"#);
        assert_eq!(answered(&good, 3), Ok(json!({"a": 1})));
        assert_eq!(answered(&good, 4), Err(Down::Protocol), "another's id");
        assert_eq!(
            answered(&good[..good.len() - 1], 3),
            Err(Down::NoDaemon),
            "cut"
        );
        assert_eq!(answered(b"", 3), Err(Down::NoDaemon), "closed");
        assert_eq!(answered(&line("not json"), 3), Err(Down::Protocol));
        assert_eq!(
            answered(&line(r#"{"id":3}"#), 3),
            Err(Down::Protocol),
            "no ok"
        );
        for (code, down) in [
            ("unauthenticated", Down::SignedOut),
            ("forbidden", Down::Forbidden),
            ("not_found", Down::NoRepo),
            ("frozen", Down::Refused),
        ] {
            let e = format!(r#"{{"id":3,"ok":false,"error":{{"code":"{code}"}}}}"#);
            assert_eq!(answered(&line(&e), 3), Err(down), "{code}");
        }
        let bare = line(r#"{"id":3,"ok":false}"#);
        assert_eq!(
            answered(&bare, 3),
            Err(Down::Protocol),
            "an error of no code"
        );
    }

    #[test]
    fn status_reads_trunk_and_what_waits() {
        let d = Daemon::start(PROTOCOL, |_| Ok(status(&[7])));
        d.sign_in();
        let got = d
            .place
            .open()
            .expect("opens")
            .status("demo")
            .expect("reads");
        assert_eq!(got.trunk, TRUNK);
        assert_eq!(got.next_seq, 9);
        let want = Pending {
            seq: 7,
            created_ms: 5,
            op: PROPOSE.into(),
            reasons: vec!["unobserved_session".into()],
            change: Some("kkkk".into()),
            base: Some(TRUNK.into()),
            intent: Some("fix".into()),
            agent: Some("a".into()),
            task: Some("T-1".into()),
            paths: vec!["src/a.rs".into()],
        };
        assert_eq!(got.pending, [want]);
        assert_eq!(got.policy, POLICY);
    }

    #[test]
    fn every_way_a_change_can_stand_is_read() {
        let of = |v: Value| Standing::of(&v);
        let word = |w: &str| of(json!({"status": w}));
        assert_eq!(word("pending"), Some(Standing::Pending));
        assert_eq!(word("queued"), Some(Standing::Queued));
        assert_eq!(word("withdrawn"), Some(Standing::Withdrawn));
        assert_eq!(word("a newer word"), Some(Standing::Other));
        assert_eq!(
            of(json!({"status": "integrated", "commit": "c1"})),
            Some(Standing::Integrated("c1".into()))
        );
        assert_eq!(
            of(json!({"status": "conflicted", "symbols": ["a#b"]})),
            Some(Standing::Conflicted(vec!["a#b".into()]))
        );
        assert_eq!(
            of(json!({"status": "rejected", "reasons": ["rejected: no"]})),
            Some(Standing::Rejected(vec!["rejected: no".into()]))
        );
        assert_eq!(word("integrated"), None, "no commit");
        assert_eq!(word("conflicted"), None, "no symbols");
        assert_eq!(word("rejected"), None, "no reasons");
        assert_eq!(of(json!({"status": "rejected", "reasons": [1]})), None);
        assert_eq!(of(json!({})), None);
    }

    #[test]
    fn where_a_change_stands_is_read_from_its_record() {
        let d = Daemon::start(PROTOCOL, |asked| match asked["params"]["change"].as_str() {
            Some("kkkk") => Ok(json!({"status": {"status": "queued"}})),
            Some("odd") => Ok(json!({})),
            _ => Err("not_found"),
        });
        d.sign_in();
        let mut c = d.place.open().expect("opens");
        assert_eq!(c.standing("demo", "kkkk"), Ok(Standing::Queued));
        assert_eq!(c.standing("demo", "odd"), Err(Down::Protocol));
        assert!(c.standing("demo", "nobody").is_err());
    }

    #[test]
    fn a_status_lists_every_change_with_where_it_stands() {
        let mut v = status(&[]);
        v["changes"] = json!([{"change": "a", "status": {"status": "queued"}},
            {"change": "b", "status": {"status": "withdrawn"}}]);
        let got = Status::of(&v).expect("reads");
        let want = [
            ("a".to_string(), Standing::Queued),
            ("b".to_string(), Standing::Withdrawn),
        ];
        assert_eq!(got.changes, want);
        v["changes"] = json!([{"status": {"status": "queued"}}]);
        assert_eq!(Status::of(&v), None, "a change with no id");
        v["changes"] = json!([{"change": "a"}]);
        assert_eq!(Status::of(&v), None, "a change standing nowhere");
    }

    #[test]
    fn a_diff_is_read_whole_or_not_at_all() {
        let whole = json!({"change": "kkkk", "base": TRUNK, "commit": "c2", "format": "unified",
            "files": [{"path": "src/a.rs", "status": "modified", "added": 3, "removed": 1,
                       "symbols": {}},
                      {"path": "b.png", "status": "added", "added": null, "removed": null,
                       "symbols": {}}],
            "redacted": ["secret.txt"], "diff": "diff --git a b\n", "truncated": true});
        let d = Daemon::start(PROTOCOL, {
            let whole = whole.clone();
            move |_| Ok(whole.clone())
        });
        d.sign_in();
        let got = (d.place.open().expect("opens"))
            .diff("demo", "kkkk")
            .expect("reads");
        let file = |path: &str, lines| DiffFile {
            path: path.into(),
            lines,
        };
        let want = Diff {
            commit: "c2".into(),
            files: vec![file("src/a.rs", Some((3, 1))), file("b.png", None)],
            redacted: vec!["secret.txt".into()],
            patch: "diff --git a b\n".into(),
            cut: true,
        };
        assert_eq!(got, want);
        let heard = d.heard.lock().expect("heard");
        assert_eq!(heard[0]["method"], "change.diff");
        assert_eq!(heard[0]["params"]["change"], "kkkk");
        assert_eq!(heard[0]["params"]["format"], "unified");
        for key in ["commit", "files", "redacted", "diff", "truncated"] {
            let mut v = whole.clone();
            v.as_object_mut().expect("object").remove(key);
            assert_eq!(Diff::of(&v), None, "{key}");
        }
        let mut v = whole.clone();
        v["files"][0]["path"] = json!(1);
        assert_eq!(Diff::of(&v), None, "a path that is no text");
        v = whole;
        v["files"][0]["added"] = json!(u64::from(u32::MAX) + 1);
        assert_eq!(Diff::of(&v).expect("reads").files[0].lines, None);
    }

    #[test]
    fn base64_comes_back_to_its_bytes() {
        let of = |text: &str| from_base64(text).map(|b| String::from_utf8_lossy(&b).into_owned());
        assert_eq!(of(""), Some(String::new()));
        assert_eq!(of("Zg=="), Some("f".into()));
        assert_eq!(of("Zm8="), Some("fo".into()));
        assert_eq!(of("Zm9v"), Some("foo".into()));
        assert_eq!(of("Zm9vYmFy"), Some("foobar".into()));
        assert_eq!(
            from_base64("AZaz09+/"),
            Some(vec![1, 150, 179, 211, 223, 191])
        );
        for wrong in ["Zm9v!", "Zm 9v", "Zm9v\n"] {
            assert_eq!(from_base64(wrong), None, "{wrong:?}");
        }
    }

    #[test]
    fn a_policy_holds_a_change_unless_it_is_seen_to_be_told_not_to() {
        for held in [
            "",
            "[paths]\ncritical = []\n",
            "[observation]\nnetwork = \"taint\"\n",
            "[observation]\nunobserved_is_tainted = true\n",
            "[observation]\nunobserved_is_tainted = true  # the default\n",
            "# unobserved_is_tainted = false\n",
            "version = 1\n",
            "[observation]\nobservation_note = 1\n",
            "[other]\nthing = 1\n[observation]\n  unobserved_is_tainted=true\n",
        ] {
            assert!(unobserved_held(held), "{held:?}");
        }
        for open in [
            "[observation]\nunobserved_is_tainted = false\n",
            "[observation]\nunobserved_is_tainted = \"true\"\n",
            "[observation]\nunobserved_is_tainted\n",
            "observation.unobserved_is_tainted = true\n",
            "observation = { unobserved_is_tainted = false }\n",
            "observation = { network = \"taint\" }\n",
            "[other]\nunobserved_is_tainted = true\n",
            "[observation]\nunobserved_is_tainted = true\nunobserved_is_tainted = false\n",
        ] {
            assert!(!unobserved_held(open), "{open:?}");
        }
    }

    #[test]
    fn the_policy_is_read_from_its_commit_in_the_store() {
        let answers = |result: Value| {
            let d = Daemon::start(PROTOCOL, move |_| Ok(result.clone()));
            d.sign_in();
            d
        };
        let file = |b64: &str| {
            json!([{"path": "policy.cedar", "content_b64": "AAAA"},
            {"path": "lonca.toml", "content_b64": b64}])
        };
        // `[observation]\nunobserved_is_tainted = false\n`
        let off = "W29ic2VydmF0aW9uXQp1bm9ic2VydmVkX2lzX3RhaW50ZWQgPSBmYWxzZQo=";
        let holds = |d: &Daemon| {
            d.place
                .open()
                .expect("opens")
                .holds_unobserved("demo", "pol")
        };
        let d = answers(file(off));
        assert_eq!(holds(&d), Ok(false));
        let heard = d.heard.lock().expect("heard");
        assert_eq!(heard[0]["method"], "tree");
        assert_eq!(heard[0]["params"]["commit"], "pol");
        assert_eq!(heard[0]["params"]["with_content"], true);
        assert_eq!(heard[0]["params"]["paths"], json!(["lonca.toml"]));
        assert_eq!(heard[0]["repo"], "demo");
        assert_eq!(holds(&answers(file("Cg=="))), Ok(true), "a file of nothing");
        assert_eq!(
            holds(&answers(json!([]))),
            Ok(true),
            "no file: the defaults"
        );
        assert_eq!(holds(&answers(file("!"))), Err(Down::Protocol));
        assert_eq!(
            holds(&answers(file("/w=="))),
            Err(Down::Protocol),
            "no text"
        );
        assert_eq!(holds(&answers(json!({}))), Err(Down::Protocol));
        let none = json!([{"path": "lonca.toml", "content_b64": null}]);
        assert_eq!(
            holds(&answers(none)),
            Err(Down::Protocol),
            "a redacted blob"
        );
    }

    #[test]
    fn a_decision_names_its_number_and_repository() {
        let (place, dir) = program("exit 0");
        assert_eq!(place.approve("-demo", &dir, 5), Ok(()));
        assert!(
            seen(&dir).contains("|approve 5 --repo=-demo --json|"),
            "{}",
            seen(&dir)
        );
        assert_eq!(place.reject("-demo", &dir, 7, "-not this"), Ok(()));
        assert!(
            seen(&dir).contains("|reject 7 --repo=-demo --reason=-not this --json|"),
            "{}",
            seen(&dir)
        );
        fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn a_refused_decision_says_why() {
        let (place, dir) = program("echo 'lonca: denied: nothing_pending' >&2\nexit 1");
        let why = Err(Failed::Said("denied: nothing_pending".into()));
        assert_eq!(place.approve("demo", &dir, 5), why);
        assert_eq!(place.reject("demo", &dir, 5, "no"), why);
        fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn a_reason_stays_within_what_lonca_takes_and_whole_letters() {
        assert_eq!(cut_bytes("abc", 5), "abc");
        assert_eq!(cut_bytes("abc", 3), "abc");
        assert_eq!(cut_bytes("abc", 2), "ab");
        assert_eq!(cut_bytes("aç", 2), "a", "ç takes two bytes");
        assert_eq!(cut_bytes("aç", 3), "aç");
        assert_eq!(cut_bytes("ç", 1), "");
        let (place, dir) = program("exit 0");
        let long = "x".repeat(REASON_MOST + 1);
        place.reject("demo", &dir, 1, &long).unwrap();
        let reason = "x".repeat(REASON_MOST);
        assert!(seen(&dir).contains(&format!("--reason={reason} --json")));
        fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn a_pending_that_is_no_proposal_has_no_change() {
        let v = json!({"seq": 1, "created_ms": 2, "op": "policy.update", "reasons": [], "change": null,
            "base": null, "intent": null, "agent": null, "task": null, "paths": []});
        let p = Pending::of(&v).expect("reads");
        assert_eq!((p.change, p.agent, p.paths.len()), (None, None, 0));
    }

    #[test]
    fn what_is_held_is_read_with_its_holder_pinned_or_not() {
        let mut v = status(&[]);
        v["sessions"] = json!([
            {"id": "s1", "agent": "a", "expires_ms": 1},
            {"id": "s2", "agent": "b", "expires_ms": 1},
        ]);
        v["leases"] = json!([
            {"session": "s2", "items": [{"symbol": "z.rs#T.m", "kind": "edit"}], "pinned": []},
            {"session": "s1",
             "items": [{"symbol": "a.rs#f", "kind": "edit"}],
             "pinned": [{"change": "c", "items": [
                 {"symbol": "a.rs#f", "kind": "edit"},
                 {"symbol": "a.rs#<file>", "kind": "insert"},
             ]}]},
            {"session": "gone", "items": [{"symbol": "b.rs#g", "kind": "edit"}]},
        ]);
        let held = |file: &str, symbol: &str, agent: Option<&str>| Lease {
            file: file.into(),
            symbol: symbol.into(),
            agent: agent.map(str::to_string),
        };
        assert_eq!(
            Status::of(&v).unwrap().leases,
            [
                held("a.rs", "<file>", Some("a")),
                held("a.rs", "f", Some("a")),
                held("b.rs", "g", None),
                held("z.rs", "T.m", Some("b")),
            ]
        );
    }

    #[test]
    fn a_lease_short_of_the_protocol_is_no_answer() {
        for wrong in [
            json!([{"items": []}]),
            json!([{"session": "s"}]),
            json!([{"session": "s", "items": [{"kind": "edit"}]}]),
            json!([{"session": "s", "items": [{"symbol": "no-hash"}]}]),
            json!([{"session": "s", "items": [], "pinned": [{"change": "c"}]}]),
        ] {
            let mut v = status(&[]);
            v["leases"] = wrong.clone();
            assert_eq!(Status::of(&v), None, "{wrong}");
        }
    }

    #[test]
    fn an_agents_time_left_is_its_newest_sessions() {
        let mut v = status(&[]);
        v["now_ms"] = json!(1000);
        v["sessions"] = json!([
            {"agent": "a", "expires_ms": 1500},
            {"agent": "b", "expires_ms": 900},
            {"agent": "a", "expires_ms": 4000},
            {"agent": "a", "expires_ms": 1200},
        ]);
        let told = Status::of(&v).unwrap();
        assert_eq!(told.now_ms, 1000);
        assert_eq!(
            told.time_left(),
            vec![("a".to_string(), 3000), ("b".to_string(), -100)]
        );
        // A session short of the protocol is no answer.
        v["sessions"] = json!([{"agent": "a"}]);
        assert_eq!(Status::of(&v), None);
        v["sessions"] = json!([{"expires_ms": 1}]);
        assert_eq!(Status::of(&v), None);
    }

    #[test]
    fn a_status_short_of_the_protocol_is_no_answer() {
        let whole = status(&[7]);
        assert!(Status::of(&whole).is_some());
        for key in [
            "trunk", "policy", "next_seq", "pending", "changes", "sessions", "leases", "now_ms",
        ] {
            let mut v = whole.clone();
            v.as_object_mut().expect("object").remove(key);
            assert_eq!(Status::of(&v), None, "{key}");
        }
        for trunk in ["", "abc", "z93d62952cd8f7ce58b0de780cf5d998b2f5c1a6"] {
            let mut v = whole.clone();
            v["trunk"] = json!(trunk);
            assert_eq!(Status::of(&v), None, "{trunk}");
        }
        for key in ["seq", "created_ms", "op", "reasons", "paths"] {
            let mut v = whole.clone();
            v["pending"][0].as_object_mut().expect("object").remove(key);
            assert_eq!(Status::of(&v), None, "{key}");
        }
        let mut v = whole;
        v["pending"][0]["paths"] = json!([1]);
        assert_eq!(Status::of(&v), None, "a path that is no text");
    }

    #[test]
    fn the_other_reads_ask_what_the_protocol_says() {
        let d = Daemon::start(PROTOCOL, |asked| Ok(asked["params"].clone()));
        d.sign_in();
        let mut c = d.place.open().expect("opens");
        assert_eq!(
            c.tree("demo", "abc").expect("tree"),
            json!({"commit": "abc", "with_content": false, "paths": null})
        );
        assert_eq!(
            c.change("demo", "kkkk").expect("change"),
            json!({"change": "kkkk"})
        );
        assert_eq!(
            c.log("demo", 4, Some(2), None).expect("log"),
            json!({"from": 4, "limit": 2, "wait_ms": null})
        );
        assert_eq!(
            c.log("demo", 4, None, Some(Duration::from_millis(250)))
                .expect("log"),
            json!({"from": 4, "limit": null, "wait_ms": 250})
        );
        let heard = d.heard.lock().expect("heard");
        let methods: Vec<&str> = heard.iter().filter_map(|h| h["method"].as_str()).collect();
        assert_eq!(methods, ["tree", "change.get", "log", "log"]);
        assert!(heard.iter().all(|h| h["repo"] == "demo"));
    }

    #[test]
    fn a_wait_stays_within_what_the_daemon_holds() {
        assert_eq!(log_wait(250), 250);
        assert_eq!(log_wait(30_000), 30_000);
        assert_eq!(log_wait(30_001), 30_000);
        assert_eq!(log_wait(u128::MAX), 30_000);
    }

    #[test]
    fn a_log_wait_is_given_its_time_and_taken_back() {
        assert_eq!(log_timeout(None), TIMEOUT);
        assert_eq!(log_timeout(Some(2000)), Duration::from_secs(7));
        // Afterwards the read timeout is the plain one again.
        let d = Daemon::start(PROTOCOL, |_| Ok(json!([])));
        d.sign_in();
        let mut c = d.place.open().expect("opens");
        let wait = Some(Duration::from_secs(2));
        c.log("demo", 0, None, wait).expect("log");
        let timeout = c.conn.get_ref().read_timeout().expect("timeout");
        assert_eq!(timeout, Some(TIMEOUT));
    }

    #[test]
    fn a_daemon_gone_is_said_on_the_next_call() {
        let d = up();
        let mut c = d.place.open().expect("opens");
        c.conn
            .get_ref()
            .shutdown(std::net::Shutdown::Both)
            .expect("shuts");
        assert_eq!(c.status("demo"), Err(Down::NoDaemon));
    }

    /// The real program against a real daemon, as `:lonca import`, `:lonca
    /// login` and `:sync` run it: `LONCA_SOCKET`, `LONCA_HUMANS`,
    /// `LONCA_HOME`, `lonca` on `PATH`, and a git checkout in
    /// `IMECE_LONCA_CHECKOUT`, on a daemon with no repository yet.
    #[test]
    #[ignore = "needs a running lonca daemon and the lonca program"]
    fn live_the_program_imports_signs_in_and_fetches() {
        let checkout = PathBuf::from(std::env::var("IMECE_LONCA_CHECKOUT").expect("a checkout"));
        let place = Place::here();
        let (branch, head) = crate::worktree::on_branch(&checkout).expect("on a branch");
        assert!(matches!(place.open(), Err(Down::SignedOut)), "no session");
        assert!(
            matches!(place.login(&checkout), Err(Failed::Said(_))),
            "no repository yet"
        );
        assert_eq!(place.import("-odd.name", &checkout, &branch), Ok(vec![]));
        assert_eq!(place.login(&checkout), Ok("ayse".to_string()));
        assert_eq!(place.critical_waits("-odd.name", &checkout), Ok(()));
        assert_eq!(
            place.critical_waits("-odd.name", &checkout),
            Err(Failed::NoAnswer),
            "said once"
        );
        let mut c = place.open().expect("opens");
        let status = c.status("-odd.name").expect("status");
        assert_eq!(status.trunk, head);
        assert_eq!(c.distance("-odd.name", &head), Ok(Some(0)));
        assert_eq!(c.distance("-odd.name", &"0".repeat(40)), Ok(None));
        assert_eq!(place.fetch("-odd.name", &checkout), Ok(()));
        assert_eq!(crate::worktree::near(&checkout, TRUNK_REF), Some(head));
        assert_eq!(
            crate::worktree::fast_forward(&checkout, TRUNK_REF).unwrap(),
            crate::worktree::Forward::Even
        );
        assert!(matches!(
            place.import("-odd.name", &checkout, &branch),
            Err(Failed::Said(why)) if why.contains("exists")
        ));
    }

    /// A worker's seat as a real program gives it: `LONCA_SOCKET`,
    /// `LONCA_HUMANS`, `LONCA_HUMAN`, `lonca` on `PATH`, a repository in
    /// `IMECE_LONCA_REPO` and an empty directory for it in
    /// `IMECE_LONCA_SEATS`; run alone, it signs as the person.
    #[test]
    #[ignore = "needs a running lonca daemon and the lonca program"]
    fn live_a_seat_is_taken_kept_and_taken_again() {
        let repo = std::env::var("IMECE_LONCA_REPO").expect("a repository");
        let seats = PathBuf::from(std::env::var("IMECE_LONCA_SEATS").expect("a directory"));
        let (tree, scratch) = (seats.join("claude-1"), seats.join("scratch"));
        fs::create_dir_all(&scratch).unwrap();
        let place = Place::here();
        let seat = Seat {
            repo: &repo,
            agent: "solo-şu ad",
            workspace: &tree,
            scratch: Some(&scratch),
        };
        assert_eq!(place.sit(&seat), Ok(()));
        assert!(is_workspace(&tree));
        assert_eq!(workspace_repo(&tree).as_deref(), Some(repo.as_str()));
        assert_eq!(place.seated(&tree), Ok(true));
        assert!(
            matches!(place.sit(&seat), Err(Failed::Said(why)) if why.contains("not empty")),
            "a workspace is made once"
        );
        let token = fs::read_to_string(tree.join(".lonca/session")).unwrap();
        // The same scope is no widening, its scratch included.
        assert_eq!(place.sit_again(&seat), Ok(()));
        assert_ne!(
            token,
            fs::read_to_string(tree.join(".lonca/session")).unwrap()
        );
        assert_eq!(place.seated(&tree), Ok(true));
        assert_eq!(place.left_in(&tree), Some(0));
        fs::write(tree.join("new.txt"), "x").unwrap();
        assert_eq!(place.left_in(&tree), Some(1));
        assert_eq!(place.stand_up(&tree), Ok(()));
        assert_eq!(place.seated(&tree), Ok(false));
        assert_eq!(place.left_in(&tree), None, "a session over can not count");
        assert!(place.stand_up(&tree).is_err(), "twice");
        fs::remove_file(tree.join("new.txt")).unwrap();
        assert_eq!(place.left_in(&tree), Some(0), "no change needs no session");
    }

    /// Against a real daemon, the fake being only what this client thinks
    /// the protocol is: `LONCA_SOCKET`, `LONCA_HUMANS`, `LONCA_HUMAN` and
    /// a repository in `IMECE_LONCA_REPO`, with a pending proposal on it.
    #[test]
    #[ignore = "needs a running lonca daemon"]
    fn live_a_real_daemon_answers_as_the_protocol_says() {
        let repo = std::env::var("IMECE_LONCA_REPO").expect("a repository");
        let mut c = Place::here().open().expect("opens");
        let status = c.status(&repo).expect("status");
        // A human session is checked against the repository first: to it
        // one that does not exist is one it may not read.
        assert_eq!(c.status("no-such-repo"), Err(Down::Forbidden));
        let tree = c.tree(&repo, &status.trunk).expect("tree");
        assert!(tree[0]["blob"].is_string() && tree[0]["path"].is_string());
        let log = c.log(&repo, 0, Some(1), None).expect("log");
        assert_eq!(log[0]["entry"]["seq"], 0);
        let held = Some(Duration::from_millis(50));
        let none = c.log(&repo, status.next_seq, None, held).expect("held log");
        assert_eq!(none, json!([]));
        let held = status
            .leases
            .first()
            .expect("a pending proposal pins its lease");
        assert!(held.agent.is_some() && !held.file.is_empty() && !held.symbol.is_empty());
        let pending = status.pending.first().expect("a pending proposal");
        let change = pending.change.as_deref().expect("a proposal");
        let got = c.change(&repo, change).expect("change");
        assert_eq!(got["change"], change);
        assert_eq!(pending.op, "change.propose");
        assert!(!pending.paths.is_empty() && pending.intent.is_some());
        assert!(pending.agent.is_some() && pending.base.is_some());
        assert!(!pending.reasons.is_empty());
        let diff = c.diff(&repo, change).expect("diff");
        assert_eq!(got["commit"], diff.commit.as_str());
        assert!(diff.patch.starts_with("diff --git "), "{}", diff.patch);
        assert!(diff.files[0].lines.is_some() && !diff.cut);
        let listed = (status.changes.iter()).find(|(id, _)| id == change);
        assert_eq!(listed.map(|(_, s)| s), Some(&Standing::Pending));
        assert_eq!(c.holds_unobserved(&repo, &status.policy), Ok(true));
    }

    /// A worker's proposals decided by the real program: as above, with
    /// two proposals of one workspace waiting, and `lonca` on `PATH`.
    #[test]
    #[ignore = "needs a running lonca daemon and the lonca program"]
    fn live_a_proposal_is_turned_down_and_its_newer_one_lands() {
        let repo = std::env::var("IMECE_LONCA_REPO").expect("a repository");
        let dir = std::env::temp_dir();
        let place = Place::here();
        let mut c = place.open().expect("opens");
        let status = c.status(&repo).expect("status");
        let [older, newer] = &status.pending[..] else {
            panic!("two waiting: {:?}", status.pending);
        };
        let standing = |c: &mut Client, change: &Option<String>| {
            let changes = c.status(&repo).expect("status").changes;
            let found = changes
                .into_iter()
                .find(|(id, _)| Some(id) == change.as_ref());
            found.map(|(_, s)| s)
        };
        assert_eq!(place.reject(&repo, &dir, older.seq, "replaced"), Ok(()));
        let rejected = Standing::Rejected(vec!["rejected: replaced".into()]);
        assert_eq!(standing(&mut c, &older.change), Some(rejected));
        assert_eq!(place.approve(&repo, &dir, newer.seq), Ok(()));
        let trunk = c.status(&repo).expect("status").trunk;
        assert_eq!(
            standing(&mut c, &newer.change),
            Some(Standing::Integrated(trunk))
        );
        let change = newer.change.as_deref().expect("a proposal");
        let checks = c.evidence(&repo, change).expect("evidence");
        assert!(checks.iter().any(|e| e.check == "syntax"), "{checks:?}");
        assert_eq!(c.verifies(&repo, &status.policy), Ok(vec![]), "the default");
        assert!(matches!(
            place.approve(&repo, &dir, newer.seq),
            Err(Failed::Said(why)) if why.contains("nothing_pending")
        ));
    }
}
