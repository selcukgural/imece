//! What imece remembers between runs: the world as the screen shows it
//! (projects, agents, cards and their stories, the journal, the inbox, the
//! traces) and what the engine keeps beside it (each agent's model, mail,
//! notes and unfinished prompts, the board's posts and offers, workers'
//! changes), in one SQLite file for every project.
//!
//! A save writes only what changed since the last one: new journal, story
//! and trace lines are appended, and a row of a growing table (cards, the
//! inbox) is written again only when its hash moved. Rows are numbered by
//! their position in the world, so ids stay stable across runs. The file
//! is the person's alone (0600) and one imece at a time has it (a lock
//! next to it); a second one is refused, not silently shared.
//!
//! Growth: the stories are loaded whole (the screen holds them in memory
//! anyway) and the journal's newest lines; the read paths are the primary
//! keys, so a per-project page or a cap per card comes without a new
//! index. Closed projects and gone agents keep their rows. A tidy at the
//! start (`tidy`) lets go of journal lines nobody loads any more and packs
//! the file when a quarter of it is air.
//!
//! The person's screen choices (`prefs`) are in the same file's `meta`
//! table, but go through a connection of their own without the world's
//! lock (`settings`, `set_settings`).

use std::collections::{HashMap, HashSet, VecDeque};
use std::fs::OpenOptions;
use std::hash::{DefaultHasher, Hash, Hasher};
use std::os::unix::fs::OpenOptionsExt;
use std::path::{Path, PathBuf};

use imece_agent::worktree;
use rusqlite::{Connection, OptionalExtension, params};
use serde::{Serialize, de::DeserializeOwned};

use crate::model::*;

/// The file's layout; a file from a newer imece is refused.
pub const SCHEMA_VERSION: i64 = 2;
/// Trace lines kept per agent, as the world keeps them.
const TRACE_KEEP: usize = 200;
/// The file's name under the state directory.
pub const FILE: &str = "imece.sqlite";
/// Journal lines older than this many days go at a tidy, except the newest
/// ones a load takes, which stay whatever their age.
pub const JOURNAL_DAYS: u64 = 90;
/// Keys of `meta` that are the world's, not the person's settings.
const WORLD_KEYS: [&str; 4] = ["schema_version", "started_at", "budget", "saved_at"];

const SCHEMA: &str = "
CREATE TABLE IF NOT EXISTS meta (key TEXT PRIMARY KEY, value TEXT NOT NULL) WITHOUT ROWID;
CREATE TABLE IF NOT EXISTS projects (
    id INTEGER PRIMARY KEY, name TEXT NOT NULL, leader INTEGER NOT NULL,
    closed INTEGER NOT NULL, repo TEXT NOT NULL);
CREATE TABLE IF NOT EXISTS agents (
    id INTEGER PRIMARY KEY, project INTEGER NOT NULL, name TEXT NOT NULL,
    vendor TEXT NOT NULL, role TEXT NOT NULL, parent INTEGER, card INTEGER,
    cost INTEGER NOT NULL, taint TEXT NOT NULL, gone INTEGER NOT NULL,
    changed_at INTEGER NOT NULL, traced INTEGER NOT NULL, commands TEXT NOT NULL,
    model TEXT, tiered INTEGER NOT NULL, woken INTEGER NOT NULL, posted INTEGER NOT NULL,
    session TEXT, session_spent INTEGER NOT NULL DEFAULT 0);
CREATE TABLE IF NOT EXISTS cards (
    id INTEGER PRIMARY KEY, project INTEGER NOT NULL, title TEXT NOT NULL,
    owner INTEGER, col TEXT NOT NULL, started INTEGER NOT NULL, finished INTEGER NOT NULL);
-- Read whole, in key order (`load`): no index beyond the key.
CREATE TABLE IF NOT EXISTS story (
    card INTEGER NOT NULL, seq INTEGER NOT NULL, tick INTEGER NOT NULL,
    kind TEXT NOT NULL, data TEXT NOT NULL, PRIMARY KEY (card, seq)) WITHOUT ROWID;
-- Read newest first with a cap (`load`), which the rowid order gives.
CREATE TABLE IF NOT EXISTS journal (
    seq INTEGER PRIMARY KEY, tick INTEGER NOT NULL, project INTEGER NOT NULL,
    agent INTEGER NOT NULL, kind TEXT NOT NULL, data TEXT NOT NULL);
CREATE TABLE IF NOT EXISTS inbox (
    id INTEGER PRIMARY KEY, kind TEXT NOT NULL, agent INTEGER NOT NULL,
    detail TEXT NOT NULL, researcher INTEGER, open INTEGER NOT NULL, tick INTEGER NOT NULL);
-- Read per agent, newest first with a cap (`load`): the key serves it.
CREATE TABLE IF NOT EXISTS trace (
    agent INTEGER NOT NULL, seq INTEGER NOT NULL, line TEXT NOT NULL,
    PRIMARY KEY (agent, seq)) WITHOUT ROWID;
CREATE TABLE IF NOT EXISTS mail (
    agent INTEGER NOT NULL, seq INTEGER NOT NULL, sender INTEGER NOT NULL,
    kind TEXT NOT NULL, body TEXT NOT NULL, PRIMARY KEY (agent, seq)) WITHOUT ROWID;
CREATE TABLE IF NOT EXISTS notes (
    agent INTEGER NOT NULL, seq INTEGER NOT NULL, text TEXT NOT NULL,
    PRIMARY KEY (agent, seq)) WITHOUT ROWID;
CREATE TABLE IF NOT EXISTS inflight (
    agent INTEGER NOT NULL, seq INTEGER NOT NULL, text TEXT NOT NULL,
    PRIMARY KEY (agent, seq)) WITHOUT ROWID;
CREATE TABLE IF NOT EXISTS posts (card INTEGER PRIMARY KEY, by_agent INTEGER NOT NULL, body TEXT NOT NULL);
CREATE TABLE IF NOT EXISTS offers (
    card INTEGER NOT NULL, seq INTEGER NOT NULL, agent INTEGER NOT NULL,
    PRIMARY KEY (card, seq)) WITHOUT ROWID;
CREATE TABLE IF NOT EXISTS changes (
    agent INTEGER PRIMARY KEY, path TEXT NOT NULL, tree TEXT NOT NULL,
    files TEXT NOT NULL, patch TEXT NOT NULL);
";

/// What the engine keeps beside the world, borrowed for a save: by
/// project, by agent, and by card.
pub struct Kept<'a> {
    /// Each project's checkout.
    pub repos: Vec<&'a Path>,
    /// Each agent's model (none: its vendor's default) and whether a tier
    /// chose it.
    pub models: &'a [Option<String>],
    pub tiered: &'a [bool],
    /// Turns each agent took on messages, and cards it posted, since the
    /// person last spoke.
    pub woken: &'a [u32],
    pub posted: &'a [u32],
    pub mail: &'a [Vec<Mail>],
    /// What imece has to tell each agent at its next turn.
    pub notes: &'a [Vec<String>],
    /// Prompts each agent was given and has not finished.
    pub inflight: &'a [Vec<String>],
    /// The vendor session each agent had last, to go on with.
    pub sessions: &'a [Option<Resumable>],
    pub posts: &'a HashMap<CardId, Post>,
    pub offered: &'a HashMap<CardId, Vec<AgentId>>,
    pub changes: &'a HashMap<AgentId, worktree::Change>,
}

/// What the engine kept, as it comes back from the file.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Found {
    pub repos: Vec<PathBuf>,
    pub models: Vec<Option<String>>,
    pub tiered: Vec<bool>,
    pub woken: Vec<u32>,
    pub posted: Vec<u32>,
    pub mail: Vec<Vec<Mail>>,
    pub notes: Vec<Vec<String>>,
    pub inflight: Vec<Vec<String>>,
    pub sessions: Vec<Option<Resumable>>,
    pub posts: HashMap<CardId, Post>,
    pub offered: HashMap<CardId, Vec<AgentId>>,
    pub changes: HashMap<AgentId, worktree::Change>,
}

/// What a tidy let go of.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Tidied {
    /// Journal lines deleted.
    pub journal: usize,
    /// Whether the file was written anew (`VACUUM`).
    pub vacuumed: bool,
}

/// A world as it was saved, with what the engine kept, and how many
/// journal or story lines could not be read (written by another version
/// of imece).
pub struct Loaded {
    pub world: World,
    pub found: Found,
    pub skipped: usize,
    /// When the file was last written (seconds since the Unix epoch): the
    /// earlier run's last change, or its end when it quit in order. None
    /// for a file an older imece wrote.
    pub saved_at: Option<u64>,
}

/// What is in the file already, so a save writes only what moved.
#[derive(Default)]
struct Saved {
    meta: Option<u64>,
    projects: Vec<u64>,
    agents: Vec<u64>,
    cards: Vec<u64>,
    /// Story lines saved per card.
    story: Vec<usize>,
    /// Journal lines in the world that are saved, and the last sequence
    /// number in the file (older lines may not be loaded).
    journal: usize,
    journal_seq: i64,
    inbox: Vec<u64>,
    /// Each agent's `traced` at the last save.
    trace: Vec<u64>,
    mail: Vec<u64>,
    notes: Vec<u64>,
    inflight: Vec<u64>,
    posts: HashSet<CardId>,
    offered: HashMap<CardId, u64>,
    /// Each change by the tree it makes, which names its content.
    changes: HashMap<AgentId, String>,
}

pub struct Store {
    conn: Connection,
    saved: Saved,
    /// The lock file, held open: its lease is imece's run.
    _lock: std::fs::File,
}

/// Seconds since the Unix epoch.
fn unix_now() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| d.as_secs())
}

fn hash<T: Hash + ?Sized>(t: &T) -> u64 {
    let mut h = DefaultHasher::new();
    t.hash(&mut h);
    h.finish()
}

/// A small enum as one word (`Leader`), for a column.
fn tag<T: Serialize>(t: &T) -> String {
    serde_json::to_value(t)
        .ok()
        .and_then(|v| v.as_str().map(str::to_string))
        .unwrap_or_default()
}

fn untag<T: DeserializeOwned>(s: &str) -> rusqlite::Result<T> {
    serde_json::from_value(serde_json::Value::String(s.to_string()))
        .map_err(|e| bad(format!("{s:?}: {e}")))
}

/// A file that does not hold what imece wrote, as an error.
fn bad(msg: String) -> rusqlite::Error {
    rusqlite::Error::ToSqlConversionFailure(msg.into())
}

/// An event as JSON, and its variant's name.
fn event_json<T: Serialize>(e: &T) -> (String, String) {
    let v = serde_json::to_value(e).unwrap_or_default();
    let kind = match &v {
        serde_json::Value::Object(m) => m.keys().next().cloned().unwrap_or_default(),
        serde_json::Value::String(s) => s.clone(),
        _ => String::new(),
    };
    (kind, v.to_string())
}

fn n(v: usize) -> i64 {
    i64::try_from(v).unwrap_or(i64::MAX)
}

fn opt(v: Option<usize>) -> Option<i64> {
    v.map(n)
}

fn u(v: i64) -> usize {
    usize::try_from(v).unwrap_or_default()
}

fn uopt(v: Option<i64>) -> Option<usize> {
    v.map(u)
}

/// A tick or a cost for SQLite, which has signed integers only.
fn big(v: u64) -> i64 {
    i64::try_from(v).unwrap_or(i64::MAX)
}

fn ubig(v: i64) -> u64 {
    u64::try_from(v).unwrap_or_default()
}

impl Store {
    /// Opens (or makes) the file at `path`, the person's alone, held
    /// against a second imece.
    pub fn open(path: &Path) -> Result<Store, String> {
        if let Some(dir) = path.parent() {
            std::fs::create_dir_all(dir).map_err(|e| format!("{}: {e}", dir.display()))?;
        }
        let mut lock_path = path.as_os_str().to_owned();
        lock_path.push(".lock");
        let lock = OpenOptions::new()
            .create(true)
            .truncate(false)
            .write(true)
            .mode(0o600)
            .open(&lock_path)
            .map_err(|e| format!("{}: {e}", path.display()))?;
        if let Err(e) = lock_file(&lock) {
            return Err(if e.kind() == std::io::ErrorKind::WouldBlock {
                format!(
                    "another imece is running on {}; open the other checkout from it with :open",
                    path.display()
                )
            } else {
                format!("{}: could not lock: {e}", lock_path.to_string_lossy())
            });
        }
        make_private(path)?;
        let conn = Connection::open(path).map_err(|e| format!("{}: {e}", path.display()))?;
        let mut store = Store {
            conn,
            saved: Saved::default(),
            _lock: lock,
        };
        store
            .prepare()
            .map_err(|e| format!("{}: {e}", path.display()))?;
        Ok(store)
    }

    /// The connection itself, for tests that look at or break the file.
    #[cfg(test)]
    pub(crate) fn raw(&self) -> &Connection {
        &self.conn
    }

    /// One in memory, for tests.
    #[cfg(test)]
    pub fn in_memory() -> Store {
        let mut store = Store {
            conn: Connection::open_in_memory().unwrap(),
            saved: Saved::default(),
            _lock: std::fs::File::open("/dev/null").unwrap(),
        };
        store.prepare().unwrap();
        store
    }

    fn prepare(&mut self) -> rusqlite::Result<()> {
        self.conn.busy_timeout(std::time::Duration::from_secs(5))?;
        self.conn.pragma_update(None, "journal_mode", "WAL")?;
        self.conn.pragma_update(None, "synchronous", "NORMAL")?;
        self.conn.execute_batch(SCHEMA)?;
        // Schema 2 (2026-10-30) added the session an agent can go on with.
        if !self.has_column("agents", "session")? {
            self.conn.execute_batch(
                "ALTER TABLE agents ADD COLUMN session TEXT;
                 ALTER TABLE agents ADD COLUMN session_spent INTEGER NOT NULL DEFAULT 0;",
            )?;
        }
        let version: i64 = self
            .conn
            .query_row(
                "SELECT value FROM meta WHERE key = 'schema_version'",
                [],
                |r| r.get::<_, String>(0),
            )
            .optional()?
            .and_then(|v| v.parse().ok())
            .unwrap_or(0);
        if version > SCHEMA_VERSION {
            return Err(bad(format!(
                "the file was written by a newer imece (schema {version}, this one knows \
                 {SCHEMA_VERSION})"
            )));
        }
        self.conn.execute(
            "INSERT OR REPLACE INTO meta (key, value) VALUES ('schema_version', ?1)",
            params![SCHEMA_VERSION.to_string()],
        )?;
        Ok(())
    }

    /// Lets go of what nobody reads any more, once at the start after the
    /// load: journal lines older than [`JOURNAL_DAYS`] that lie past the
    /// newest `journal_keep` (the ones a load takes, whatever their age),
    /// and the file's free pages when they are a quarter of it or more
    /// (`VACUUM` writes the file anew). Stories stay with their cards and
    /// items in the inbox, both numbered by position. A file without a
    /// world has nothing to tidy.
    pub fn tidy(&mut self, now: u64, journal_keep: usize) -> rusqlite::Result<Tidied> {
        let started_at: Option<u64> = self
            .conn
            .query_row("SELECT value FROM meta WHERE key = 'started_at'", [], |r| {
                r.get::<_, String>(0)
            })
            .optional()?
            .and_then(|v| v.parse().ok());
        let Some(started_at) = started_at else {
            return Ok(Tidied::default());
        };
        // The tick JOURNAL_DAYS ago; 0 while the world is younger than that.
        let cutoff = now
            .saturating_sub(JOURNAL_DAYS * 86_400)
            .saturating_sub(started_at)
            / SECONDS_PER_TICK;
        let newest: i64 =
            self.conn
                .query_row("SELECT COALESCE(MAX(seq), 0) FROM journal", [], |r| {
                    r.get(0)
                })?;
        // `seq <= ?1` walks the primary key up to there: the newer lines,
        // the bulk of a file that is tidied every start, are not read.
        let journal = self.conn.execute(
            "DELETE FROM journal WHERE seq <= ?1 AND tick < ?2",
            params![newest - n(journal_keep), big(cutoff)],
        )?;
        let free: i64 = self
            .conn
            .query_row("PRAGMA freelist_count", [], |r| r.get(0))?;
        let pages: i64 = self.conn.query_row("PRAGMA page_count", [], |r| r.get(0))?;
        let vacuumed = needs_vacuum(ubig(free), ubig(pages));
        if vacuumed {
            self.conn.execute_batch("VACUUM")?;
        }
        Ok(Tidied { journal, vacuumed })
    }

    fn has_column(&self, table: &str, column: &str) -> rusqlite::Result<bool> {
        let mut st = self
            .conn
            .prepare(&format!("SELECT name FROM pragma_table_info('{table}')"))?;
        let names: Vec<String> = st.query_map([], |r| r.get(0))?.collect::<Result<_, _>>()?;
        Ok(names.iter().any(|n| n == column))
    }

    /// Writes what moved since the last save, in one transaction. Says
    /// whether anything was written.
    pub fn save(&mut self, w: &World, k: &Kept) -> rusqlite::Result<bool> {
        self.save_at(w, k, unix_now())
    }

    /// [`save`](Self::save) at `now` (seconds since the Unix epoch), which
    /// the file remembers as its last writing when anything is written.
    pub fn save_at(&mut self, w: &World, k: &Kept, now: u64) -> rusqlite::Result<bool> {
        let plan = self.plan(w, k);
        if plan.is_empty() {
            return Ok(false);
        }
        let tx = self.conn.transaction()?;
        tx.execute(
            "INSERT OR REPLACE INTO meta (key, value) VALUES ('saved_at', ?1)",
            params![now.to_string()],
        )?;
        if plan.meta {
            for (key, value) in [
                ("started_at", w.started_at.to_string()),
                ("budget", w.budget.to_string()),
            ] {
                tx.execute(
                    "INSERT OR REPLACE INTO meta (key, value) VALUES (?1, ?2)",
                    params![key, value],
                )?;
            }
        }
        for &p in &plan.projects {
            let q = &w.projects[p];
            tx.execute(
                "INSERT OR REPLACE INTO projects (id, name, leader, closed, repo) \
                 VALUES (?1, ?2, ?3, ?4, ?5)",
                params![
                    n(q.id),
                    q.name,
                    n(q.leader),
                    q.closed,
                    k.repos
                        .get(p)
                        .map_or(String::new(), |r| r.display().to_string())
                ],
            )?;
        }
        for &id in &plan.agents {
            let a = &w.agents[id];
            tx.execute(
                "INSERT OR REPLACE INTO agents (id, project, name, vendor, role, parent, card, \
                 cost, taint, gone, changed_at, traced, commands, model, tiered, woken, posted, \
                 session, session_spent) \
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?16, \
                 ?17, ?18, ?19)",
                params![
                    n(a.id),
                    n(a.project),
                    a.name,
                    tag(&a.vendor),
                    tag(&a.role),
                    opt(a.parent),
                    opt(a.card),
                    big(a.cost),
                    tag(&a.taint),
                    a.gone,
                    big(a.changed_at),
                    big(a.traced),
                    serde_json::to_string(&a.commands).unwrap_or_default(),
                    k.models.get(id).cloned().flatten(),
                    k.tiered.get(id).copied().unwrap_or(false),
                    k.woken.get(id).copied().unwrap_or(0),
                    k.posted.get(id).copied().unwrap_or(0),
                    k.sessions
                        .get(id)
                        .and_then(|s| s.as_ref().map(|s| s.session.clone())),
                    k.sessions
                        .get(id)
                        .and_then(|s| s.as_ref().map(|s| big(s.spent)))
                        .unwrap_or(0),
                ],
            )?;
        }
        for &c in &plan.cards {
            let card = &w.cards[c];
            tx.execute(
                "INSERT OR REPLACE INTO cards (id, project, title, owner, col, started, finished) \
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
                params![
                    n(card.id),
                    n(card.project),
                    card.title,
                    opt(card.owner),
                    tag(&card.column),
                    big(card.started),
                    big(card.finished)
                ],
            )?;
        }
        for &c in &plan.story {
            let from = self.saved.story.get(c).copied().unwrap_or(0);
            for (seq, s) in w.cards[c].story.iter().enumerate().skip(from) {
                let (kind, data) = event_json(&s.event);
                tx.execute(
                    "INSERT OR REPLACE INTO story (card, seq, tick, kind, data) \
                     VALUES (?1, ?2, ?3, ?4, ?5)",
                    params![n(c), n(seq), big(s.tick), kind, data],
                )?;
            }
        }
        let mut seq = self.saved.journal_seq;
        for e in &w.journal[self.saved.journal.min(w.journal.len())..] {
            seq += 1;
            let (kind, data) = event_json(&e.event);
            let agent = e.event.agent();
            tx.execute(
                "INSERT OR REPLACE INTO journal (seq, tick, project, agent, kind, data) \
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
                params![
                    seq,
                    big(e.tick),
                    n(w.agents[agent].project),
                    n(agent),
                    kind,
                    data
                ],
            )?;
        }
        for &i in &plan.inbox {
            let it = &w.inbox[i];
            tx.execute(
                "INSERT OR REPLACE INTO inbox (id, kind, agent, detail, researcher, open, tick) \
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
                params![
                    n(i),
                    tag(&it.kind),
                    n(it.agent),
                    it.detail,
                    opt(it.researcher),
                    it.open,
                    big(it.tick)
                ],
            )?;
        }
        for &id in &plan.trace {
            let a = &w.agents[id];
            let from = self.saved.trace.get(id).copied().unwrap_or(0);
            // The kept lines are the last `len` of `traced`.
            let first = a.traced - a.trace.len() as u64;
            for (i, line) in a.trace.iter().enumerate() {
                let seq = first + i as u64;
                if seq >= from {
                    tx.execute(
                        "INSERT OR REPLACE INTO trace (agent, seq, line) VALUES (?1, ?2, ?3)",
                        params![n(id), big(seq), line],
                    )?;
                }
            }
            tx.execute(
                "DELETE FROM trace WHERE agent = ?1 AND seq < ?2",
                params![n(id), big(first)],
            )?;
        }
        for &id in &plan.mail {
            tx.execute("DELETE FROM mail WHERE agent = ?1", params![n(id)])?;
            for (seq, m) in k.mail[id].iter().enumerate() {
                tx.execute(
                    "INSERT INTO mail (agent, seq, sender, kind, body) VALUES (?1, ?2, ?3, ?4, ?5)",
                    params![n(id), n(seq), n(m.from), tag(&m.kind), m.body],
                )?;
            }
        }
        for (table, ids, texts) in [
            ("notes", &plan.notes, k.notes),
            ("inflight", &plan.inflight, k.inflight),
        ] {
            for &id in ids {
                tx.execute(
                    &format!("DELETE FROM {table} WHERE agent = ?1"),
                    params![n(id)],
                )?;
                for (seq, text) in texts[id].iter().enumerate() {
                    tx.execute(
                        &format!("INSERT INTO {table} (agent, seq, text) VALUES (?1, ?2, ?3)"),
                        params![n(id), n(seq), text],
                    )?;
                }
            }
        }
        for &c in &plan.posts_gone {
            tx.execute("DELETE FROM posts WHERE card = ?1", params![n(c)])?;
        }
        for &c in &plan.posts {
            let p = &k.posts[&c];
            tx.execute(
                "INSERT OR REPLACE INTO posts (card, by_agent, body) VALUES (?1, ?2, ?3)",
                params![n(c), n(p.by), p.body],
            )?;
        }
        for &c in &plan.offered_gone {
            tx.execute("DELETE FROM offers WHERE card = ?1", params![n(c)])?;
        }
        for &c in &plan.offered {
            tx.execute("DELETE FROM offers WHERE card = ?1", params![n(c)])?;
            for (seq, &agent) in k.offered[&c].iter().enumerate() {
                tx.execute(
                    "INSERT INTO offers (card, seq, agent) VALUES (?1, ?2, ?3)",
                    params![n(c), n(seq), n(agent)],
                )?;
            }
        }
        for &id in &plan.changes_gone {
            tx.execute("DELETE FROM changes WHERE agent = ?1", params![n(id)])?;
        }
        for &id in &plan.changes {
            let c = &k.changes[&id];
            let files: Vec<serde_json::Value> = c
                .files
                .iter()
                .map(|f| {
                    serde_json::json!({
                        "path": f.path, "added": f.added, "removed": f.removed, "binary": f.binary
                    })
                })
                .collect();
            tx.execute(
                "INSERT OR REPLACE INTO changes (agent, path, tree, files, patch) \
                 VALUES (?1, ?2, ?3, ?4, ?5)",
                params![
                    n(id),
                    c.path.display().to_string(),
                    c.tree,
                    serde_json::Value::Array(files).to_string(),
                    c.patch
                ],
            )?;
        }
        tx.commit()?;
        self.saved = plan.after;
        Ok(true)
    }

    /// Marks the file as written at `now` without writing anything else:
    /// the last word when imece quits in order, so the next start knows
    /// when this run ended and not only when it last changed something.
    pub fn leave(&mut self, now: u64) -> rusqlite::Result<()> {
        self.conn.execute(
            "INSERT OR REPLACE INTO meta (key, value) VALUES ('saved_at', ?1)",
            params![now.to_string()],
        )?;
        Ok(())
    }

    /// What a save has to write, and what is saved once it did.
    fn plan(&self, w: &World, k: &Kept) -> Plan {
        let s = &self.saved;
        let mut plan = Plan::default();
        let mut after = Saved::default();
        let meta = hash(&(w.started_at, w.budget));
        plan.meta = s.meta != Some(meta);
        after.meta = Some(meta);
        for (p, q) in w.projects.iter().enumerate() {
            let h = hash(&(
                &q.name,
                q.leader,
                q.closed,
                k.repos.get(p).map(|r| r.as_os_str()),
            ));
            if s.projects.get(p) != Some(&h) {
                plan.projects.push(p);
            }
            after.projects.push(h);
        }
        for (id, a) in w.agents.iter().enumerate() {
            let h = hash(&(
                hash(&(
                    a.project, &a.name, a.vendor, a.role, a.parent, a.card, a.cost, a.taint,
                )),
                hash(&(a.gone, a.changed_at, a.traced, &a.commands)),
                hash(&(
                    k.models.get(id),
                    k.tiered.get(id),
                    k.woken.get(id),
                    k.posted.get(id),
                    k.sessions.get(id),
                )),
            ));
            if s.agents.get(id) != Some(&h) {
                plan.agents.push(id);
            }
            after.agents.push(h);
            if s.trace.get(id).copied().unwrap_or(0) != a.traced {
                plan.trace.push(id);
            }
            after.trace.push(a.traced);
        }
        for (lists, saved, out, after) in [
            (
                k.mail.iter().map(hash).collect::<Vec<u64>>(),
                &s.mail,
                &mut plan.mail,
                &mut after.mail,
            ),
            (
                k.notes.iter().map(hash).collect(),
                &s.notes,
                &mut plan.notes,
                &mut after.notes,
            ),
            (
                k.inflight.iter().map(hash).collect(),
                &s.inflight,
                &mut plan.inflight,
                &mut after.inflight,
            ),
        ] {
            for (id, h) in lists.into_iter().enumerate() {
                if saved.get(id) != Some(&h) {
                    out.push(id);
                }
                after.push(h);
            }
        }
        for (c, card) in w.cards.iter().enumerate() {
            let h = hash(&(
                card.project,
                &card.title,
                card.owner,
                card.column,
                card.started,
                card.finished,
            ));
            if s.cards.get(c) != Some(&h) {
                plan.cards.push(c);
            }
            after.cards.push(h);
            let saved = s.story.get(c).copied().unwrap_or(0);
            if card.story.len() > saved {
                plan.story.push(c);
            }
            after.story.push(card.story.len());
        }
        let new = w.journal.len().saturating_sub(s.journal);
        plan.journal = new > 0;
        after.journal = w.journal.len();
        after.journal_seq = s.journal_seq + n(new);
        for (i, it) in w.inbox.iter().enumerate() {
            let h = hash(&(
                it.kind,
                it.agent,
                &it.detail,
                it.researcher,
                it.open,
                it.tick,
            ));
            if s.inbox.get(i) != Some(&h) {
                plan.inbox.push(i);
            }
            after.inbox.push(h);
        }
        for &c in k.posts.keys() {
            if !s.posts.contains(&c) {
                plan.posts.push(c);
            }
            after.posts.insert(c);
        }
        plan.posts_gone = s.posts.difference(&after.posts).copied().collect();
        for (&c, agents) in k.offered {
            let h = hash(agents);
            if s.offered.get(&c) != Some(&h) {
                plan.offered.push(c);
            }
            after.offered.insert(c, h);
        }
        plan.offered_gone = s
            .offered
            .keys()
            .filter(|c| !after.offered.contains_key(c))
            .copied()
            .collect();
        for (&id, c) in k.changes {
            if s.changes.get(&id) != Some(&c.tree) {
                plan.changes.push(id);
            }
            after.changes.insert(id, c.tree.clone());
        }
        plan.changes_gone = s
            .changes
            .keys()
            .filter(|id| !after.changes.contains_key(id))
            .copied()
            .collect();
        plan.after = after;
        plan
    }

    /// The world as it was saved, or none when the file holds no world yet.
    /// Every agent comes back asleep (gone ones idle), with nothing in
    /// hand; the journal's newest `journal_keep` lines come.
    pub fn load(&mut self, journal_keep: usize) -> rusqlite::Result<Option<Loaded>> {
        let meta = |key: &str| -> rusqlite::Result<Option<String>> {
            self.conn
                .query_row("SELECT value FROM meta WHERE key = ?1", params![key], |r| {
                    r.get(0)
                })
                .optional()
        };
        let Some(started_at) = meta("started_at")?.and_then(|v| v.parse().ok()) else {
            return Ok(None);
        };
        let budget = meta("budget")?.and_then(|v| v.parse().ok()).unwrap_or(0);
        let saved_at = meta("saved_at")?.and_then(|v| v.parse().ok());
        let mut w = World {
            tick: 0,
            started_at,
            run_tick: 0,
            run_spent: 0,
            projects: Vec::new(),
            agents: Vec::new(),
            cards: Vec::new(),
            journal: Vec::new(),
            inbox: Vec::new(),
            budget,
            max_researchers: 0,
            research_queue: VecDeque::new(),
            plans: Vec::new(),
            changes: Vec::new(),
        };
        let mut found = Found::default();
        let mut saved = Saved::default();
        let mut skipped = 0;
        let conn = &self.conn;
        let mut st =
            conn.prepare("SELECT id, name, leader, closed, repo FROM projects ORDER BY id")?;
        for row in st.query_map([], |r| {
            Ok((
                u(r.get(0)?),
                r.get::<_, String>(1)?,
                u(r.get(2)?),
                r.get::<_, bool>(3)?,
                r.get::<_, String>(4)?,
            ))
        })? {
            let (id, name, leader, closed, repo) = row?;
            if id != w.projects.len() {
                return Err(bad(format!("project {id} is out of order")));
            }
            w.projects.push(Project {
                id,
                name,
                leader,
                trunk: None,
                repo: &[],
                closed,
            });
            found.repos.push(PathBuf::from(repo));
        }
        let mut st = conn.prepare(
            "SELECT id, project, name, vendor, role, parent, card, cost, taint, gone, \
             changed_at, traced, commands, model, tiered, woken, posted, session, session_spent \
             FROM agents ORDER BY id",
        )?;
        for row in st.query_map([], |r| {
            let id = u(r.get(0)?);
            let mut a = Agent::new(
                id,
                u(r.get(1)?),
                r.get(2)?,
                untag(&r.get::<_, String>(3)?)?,
                untag(&r.get::<_, String>(4)?)?,
                Status::Asleep,
                ubig(r.get(10)?),
            );
            a.parent = uopt(r.get(5)?);
            a.card = uopt(r.get(6)?);
            a.cost = ubig(r.get(7)?);
            a.taint = untag(&r.get::<_, String>(8)?)?;
            a.gone = r.get(9)?;
            a.traced = ubig(r.get(11)?);
            a.commands = serde_json::from_str(&r.get::<_, String>(12)?).unwrap_or_default();
            if a.gone {
                a.status = Status::Idle;
            }
            Ok((
                a,
                r.get::<_, Option<String>>(13)?,
                r.get::<_, bool>(14)?,
                r.get::<_, u32>(15)?,
                r.get::<_, u32>(16)?,
                r.get::<_, Option<String>>(17)?.map(|session| Resumable {
                    session,
                    spent: ubig(r.get(18).unwrap_or(0)),
                }),
            ))
        })? {
            let (a, model, tiered, woken, posted, session) = row?;
            if a.id != w.agents.len() {
                return Err(bad(format!("agent {} is out of order", a.id)));
            }
            w.agents.push(a);
            found.models.push(model);
            found.tiered.push(tiered);
            found.woken.push(woken);
            found.posted.push(posted);
            found.sessions.push(session);
        }
        let mut traces =
            conn.prepare("SELECT line FROM trace WHERE agent = ?1 ORDER BY seq DESC LIMIT ?2")?;
        let mut mails =
            conn.prepare("SELECT sender, kind, body FROM mail WHERE agent = ?1 ORDER BY seq")?;
        let mut texts = [
            conn.prepare("SELECT text FROM notes WHERE agent = ?1 ORDER BY seq")?,
            conn.prepare("SELECT text FROM inflight WHERE agent = ?1 ORDER BY seq")?,
        ];
        for a in &mut w.agents {
            let lines: Vec<String> = traces
                .query_map(params![n(a.id), n(TRACE_KEEP)], |r| r.get(0))?
                .collect::<Result<_, _>>()?;
            a.trace = lines.into_iter().rev().collect();
            saved.trace.push(a.traced);
            let mut mail = Vec::new();
            for m in mails.query_map(params![n(a.id)], |r| {
                Ok(Mail {
                    from: u(r.get(0)?),
                    kind: untag(&r.get::<_, String>(1)?)?,
                    body: r.get(2)?,
                })
            })? {
                mail.push(m?);
            }
            found.mail.push(mail);
            for (st, out) in texts
                .iter_mut()
                .zip([&mut found.notes, &mut found.inflight])
            {
                let lines: Vec<String> = st
                    .query_map(params![n(a.id)], |r| r.get(0))?
                    .collect::<Result<_, _>>()?;
                out.push(lines);
            }
        }
        let mut st = conn.prepare(
            "SELECT id, project, title, owner, col, started, finished FROM cards ORDER BY id",
        )?;
        for row in st.query_map([], |r| {
            Ok(Card {
                id: u(r.get(0)?),
                project: u(r.get(1)?),
                title: r.get(2)?,
                owner: uopt(r.get(3)?),
                column: untag(&r.get::<_, String>(4)?)?,
                story: Vec::new(),
                symbols: Vec::new(),
                started: ubig(r.get(5)?),
                finished: ubig(r.get(6)?),
            })
        })? {
            let c = row?;
            if c.id != w.cards.len() {
                return Err(bad(format!("card {} is out of order", c.id)));
            }
            w.cards.push(c);
        }
        let mut st = conn.prepare("SELECT card, tick, data FROM story ORDER BY card, seq")?;
        for row in st.query_map([], |r| {
            Ok((u(r.get(0)?), ubig(r.get(1)?), r.get::<_, String>(2)?))
        })? {
            let (card, tick, data) = row?;
            let Some(c) = w.cards.get_mut(card) else {
                continue;
            };
            match serde_json::from_str(&data) {
                Ok(event) => c.story.push(Story { tick, event }),
                Err(_) => skipped += 1,
            }
        }
        let mut st =
            conn.prepare("SELECT seq, tick, data FROM journal ORDER BY seq DESC LIMIT ?1")?;
        let mut entries = Vec::new();
        for row in st.query_map(params![n(journal_keep)], |r| {
            Ok((r.get::<_, i64>(0)?, ubig(r.get(1)?), r.get::<_, String>(2)?))
        })? {
            let (seq, tick, data) = row?;
            saved.journal_seq = saved.journal_seq.max(seq);
            match serde_json::from_str(&data) {
                Ok(event) => entries.push(Entry { tick, event }),
                Err(_) => skipped += 1,
            }
        }
        entries.reverse();
        w.journal = entries;
        let mut st = conn.prepare(
            "SELECT id, kind, agent, detail, researcher, open, tick FROM inbox ORDER BY id",
        )?;
        for row in st.query_map([], |r| {
            Ok((
                u(r.get(0)?),
                InboxItem {
                    kind: untag(&r.get::<_, String>(1)?)?,
                    agent: u(r.get(2)?),
                    detail: r.get(3)?,
                    researcher: uopt(r.get(4)?),
                    open: r.get(5)?,
                    tick: ubig(r.get(6)?),
                },
            ))
        })? {
            let (id, it) = row?;
            if id != w.inbox.len() {
                return Err(bad(format!("inbox item {id} is out of order")));
            }
            w.inbox.push(it);
        }
        let mut st = conn.prepare("SELECT card, by_agent, body FROM posts")?;
        for row in st.query_map([], |r| {
            Ok((
                u(r.get(0)?),
                Post {
                    by: u(r.get(1)?),
                    body: r.get(2)?,
                },
            ))
        })? {
            let (c, p) = row?;
            found.posts.insert(c, p);
        }
        let mut st = conn.prepare("SELECT card, agent FROM offers ORDER BY card, seq")?;
        for row in st.query_map([], |r| Ok((u(r.get(0)?), u(r.get(1)?))))? {
            let (c, agent) = row?;
            found.offered.entry(c).or_default().push(agent);
        }
        let mut st = conn.prepare("SELECT agent, path, tree, files, patch FROM changes")?;
        for row in st.query_map([], |r| {
            Ok((
                u(r.get(0)?),
                r.get::<_, String>(1)?,
                r.get::<_, String>(2)?,
                r.get::<_, String>(3)?,
                r.get::<_, String>(4)?,
            ))
        })? {
            let (agent, path, tree, files, patch) = row?;
            let files: Vec<serde_json::Value> = serde_json::from_str(&files).unwrap_or_default();
            let files = files
                .iter()
                .map(|f| worktree::FileChange {
                    path: f["path"].as_str().unwrap_or_default().to_string(),
                    added: f["added"].as_u64().unwrap_or(0) as u32,
                    removed: f["removed"].as_u64().unwrap_or(0) as u32,
                    binary: f["binary"].as_bool().unwrap_or(false),
                })
                .collect();
            found.changes.insert(
                agent,
                worktree::Change {
                    path: PathBuf::from(path),
                    tree,
                    files,
                    patch,
                },
            );
        }
        // What is loaded is saved: the next save writes only what moves.
        let kept = Kept {
            repos: found.repos.iter().map(PathBuf::as_path).collect(),
            models: &found.models,
            tiered: &found.tiered,
            woken: &found.woken,
            posted: &found.posted,
            mail: &found.mail,
            notes: &found.notes,
            inflight: &found.inflight,
            sessions: &found.sessions,
            posts: &found.posts,
            offered: &found.offered,
            changes: &found.changes,
        };
        let mut after = self.plan(&w, &kept).after;
        after.journal_seq = saved.journal_seq;
        self.saved = after;
        Ok(Some(Loaded {
            world: w,
            found,
            skipped,
            saved_at,
        }))
    }

    /// The query plans of the loading reads that grow with time: none may
    /// sort in a temporary b-tree, and a read by key must use the key.
    #[cfg(test)]
    fn plans(&self) -> Vec<(String, String)> {
        [
            "SELECT line FROM trace WHERE agent = 1 ORDER BY seq DESC LIMIT 200",
            "SELECT sender, kind, body FROM mail WHERE agent = 1 ORDER BY seq",
            "SELECT card, tick, data FROM story ORDER BY card, seq",
            "SELECT seq, tick, data FROM journal ORDER BY seq DESC LIMIT 5000",
            "SELECT id, kind, agent, detail, researcher, open, tick FROM inbox ORDER BY id",
            "DELETE FROM journal WHERE seq <= 10 AND tick < 20",
        ]
        .iter()
        .map(|q| {
            let mut st = self
                .conn
                .prepare(&format!("EXPLAIN QUERY PLAN {q}"))
                .unwrap();
            let steps: Vec<String> = st
                .query_map([], |r| r.get::<_, String>(3))
                .unwrap()
                .map(Result::unwrap)
                .collect();
            (q.to_string(), steps.join(" | "))
        })
        .collect()
    }
}

/// What one save writes.
#[derive(Default)]
struct Plan {
    meta: bool,
    projects: Vec<ProjectId>,
    agents: Vec<AgentId>,
    cards: Vec<CardId>,
    story: Vec<CardId>,
    journal: bool,
    inbox: Vec<usize>,
    trace: Vec<AgentId>,
    mail: Vec<AgentId>,
    notes: Vec<AgentId>,
    inflight: Vec<AgentId>,
    posts: Vec<CardId>,
    posts_gone: Vec<CardId>,
    offered: Vec<CardId>,
    offered_gone: Vec<CardId>,
    changes: Vec<AgentId>,
    changes_gone: Vec<AgentId>,
    after: Saved,
}

impl Plan {
    /// Nothing to write. A new trace line always comes with its agent's
    /// row (`traced` is in it), so the trace is not asked.
    fn is_empty(&self) -> bool {
        !self.meta
            && !self.journal
            && self.projects.is_empty()
            && self.agents.is_empty()
            && self.cards.is_empty()
            && self.story.is_empty()
            && self.inbox.is_empty()
            && self.mail.is_empty()
            && self.notes.is_empty()
            && self.inflight.is_empty()
            && self.posts.is_empty()
            && self.posts_gone.is_empty()
            && self.offered.is_empty()
            && self.offered_gone.is_empty()
            && self.changes.is_empty()
            && self.changes_gone.is_empty()
    }
}

/// Whether a file of `pages` pages with `free` of them unused is worth
/// writing anew: a quarter or more of it is air.
fn needs_vacuum(free: u64, pages: u64) -> bool {
    free > 0 && free * 4 >= pages
}

/// Makes the file at `path` the person's alone (0600) before SQLite opens
/// it, which would make it with the umask; the WAL files inherit its mode.
fn make_private(path: &Path) -> Result<(), String> {
    OpenOptions::new()
        .create(true)
        .truncate(false)
        .write(true)
        .mode(0o600)
        .open(path)
        .map(drop)
        .map_err(|e| format!("{}: {e}", path.display()))
}

/// A connection of its own to the file at `path` for the person's
/// settings, without the world's lock: the theme is read before the store
/// is taken, and a demo beside a live imece changes it too (SQLite
/// serialises the writers; a busy file is waited for a second).
fn settings_conn(path: &Path) -> Result<Connection, String> {
    let conn = Connection::open(path).map_err(|e| format!("{}: {e}", path.display()))?;
    conn.busy_timeout(std::time::Duration::from_secs(1))
        .map_err(|e| e.to_string())?;
    Ok(conn)
}

/// The person's settings `keys` from the file's `meta` table, `None` for
/// one the file does not have, all `None` for a file that is not there.
pub fn settings(path: &Path, keys: &[&str]) -> Result<Vec<Option<String>>, String> {
    if !path.exists() {
        return Ok(vec![None; keys.len()]);
    }
    let conn = settings_conn(path)?;
    keys.iter()
        .map(|key| {
            conn.query_row("SELECT value FROM meta WHERE key = ?1", params![key], |r| {
                r.get(0)
            })
            .optional()
            .map_err(|e| e.to_string())
        })
        .collect()
}

/// Writes the settings `set` and drops `unset` in the file's `meta` table,
/// making the file if it is not there (the person's alone, like `open`).
/// The world's own keys are refused.
pub fn set_settings(path: &Path, set: &[(&str, &str)], unset: &[&str]) -> Result<(), String> {
    if let Some(key) = set
        .iter()
        .map(|(k, _)| *k)
        .chain(unset.iter().copied())
        .find(|k| WORLD_KEYS.contains(k))
    {
        return Err(format!("{key} is the world's, not a setting"));
    }
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir).map_err(|e| format!("{}: {e}", dir.display()))?;
    }
    make_private(path)?;
    let conn = settings_conn(path)?;
    let write = || -> rusqlite::Result<()> {
        conn.pragma_update(None, "journal_mode", "WAL")?;
        conn.execute_batch(
            "CREATE TABLE IF NOT EXISTS meta (key TEXT PRIMARY KEY, value TEXT NOT NULL)              WITHOUT ROWID;",
        )?;
        for (key, value) in set {
            conn.execute(
                "INSERT OR REPLACE INTO meta (key, value) VALUES (?1, ?2)",
                params![key, value],
            )?;
        }
        for key in unset {
            conn.execute("DELETE FROM meta WHERE key = ?1", params![key])?;
        }
        Ok(())
    };
    write().map_err(|e| format!("{}: {e}", path.display()))
}

/// Takes the file's lock for this process: `WouldBlock` when another
/// process has it. A lock that is only being let go of (an imece that is
/// just quitting, or a child of this process between fork and exec, which
/// holds every descriptor for a moment) is given a short while.
fn lock_file(f: &std::fs::File) -> std::io::Result<()> {
    use std::os::unix::io::AsRawFd;
    // Exclusive and without waiting. Added, not or-ed: the flags share no
    // bit, and a slip here (LOCK_UN, say) must fail, not lock.
    let flags = libc::LOCK_EX + libc::LOCK_NB;
    let mut held = std::io::Error::from(std::io::ErrorKind::WouldBlock);
    for _ in 0..LOCK_TRIES {
        // SAFETY: flock on a descriptor this process owns.
        if unsafe { libc::flock(f.as_raw_fd(), flags) } == 0 {
            return Ok(());
        }
        held = std::io::Error::last_os_error();
        if held.kind() != std::io::ErrorKind::WouldBlock {
            break;
        }
        std::thread::sleep(LOCK_WAIT);
    }
    Err(held)
}

/// How often, and how long apart, the lock is tried before another imece
/// is declared to hold it.
const LOCK_TRIES: u32 = 50;
const LOCK_WAIT: std::time::Duration = std::time::Duration::from_millis(20);

#[cfg(test)]
mod tests {
    use std::os::unix::fs::PermissionsExt;

    use super::*;

    fn dir(name: &str) -> PathBuf {
        let d = std::env::temp_dir().join(format!("imece-store-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        d
    }

    fn agent(id: AgentId, project: ProjectId, name: &str, vendor: Vendor, role: Role) -> Agent {
        Agent::new(id, project, name.into(), vendor, role, Status::Working, 0)
    }

    /// A world with something in every table, with values that tell the
    /// columns apart.
    fn world() -> World {
        let mut w = World {
            tick: 77,
            started_at: 1_700_000_000,
            run_tick: 70,
            run_spent: 5,
            projects: vec![
                Project {
                    id: 0,
                    name: "one".into(),
                    leader: 0,
                    trunk: None,
                    repo: &[],
                    closed: false,
                },
                Project {
                    id: 1,
                    name: "two".into(),
                    leader: 3,
                    trunk: None,
                    repo: &[],
                    closed: true,
                },
            ],
            agents: vec![
                agent(0, 0, "lead", Vendor::Claude, Role::Leader),
                agent(1, 0, "codex-1", Vendor::Codex, Role::Worker),
                agent(2, 0, "codex-1.1", Vendor::Gemini, Role::Helper),
                agent(3, 1, "lead-two", Vendor::Claude, Role::Leader),
            ],
            cards: vec![
                Card {
                    id: 0,
                    project: 0,
                    title: "fix the parser".into(),
                    owner: Some(1),
                    column: Column::Working,
                    story: vec![
                        Story {
                            tick: 10,
                            event: StoryEvent::Took { agent: 1 },
                        },
                        Story {
                            tick: 11,
                            event: StoryEvent::Sent {
                                from: 1,
                                to: 0,
                                kind: MsgKind::Progress,
                                text: "half way".into(),
                            },
                        },
                        Story {
                            tick: 12,
                            event: StoryEvent::Interrupted,
                        },
                    ],
                    symbols: Vec::new(),
                    started: 10,
                    finished: 0,
                },
                Card {
                    id: 1,
                    project: 0,
                    title: "write docs".into(),
                    owner: None,
                    column: Column::Waiting,
                    story: vec![Story {
                        tick: 20,
                        event: StoryEvent::Posted {
                            by: 0,
                            text: "the README".into(),
                        },
                    }],
                    symbols: Vec::new(),
                    started: 20,
                    finished: 0,
                },
                Card {
                    id: 2,
                    project: 1,
                    title: "done one".into(),
                    owner: Some(3),
                    column: Column::Done,
                    story: Vec::new(),
                    symbols: Vec::new(),
                    started: 5,
                    finished: 9,
                },
            ],
            journal: vec![
                Entry {
                    tick: 1,
                    event: Event::Opened {
                        leader: 0,
                        path: "/work/one".into(),
                    },
                },
                Entry {
                    tick: 2,
                    event: Event::Hired {
                        by: 0,
                        agent: 1,
                        model: Some("gpt".into()),
                    },
                },
                Entry {
                    tick: 3,
                    event: Event::Sent {
                        from: 0,
                        to: 1,
                        kind: MsgKind::Assign,
                        text: "fix the parser".into(),
                    },
                },
                Entry {
                    tick: 4,
                    event: Event::Shell {
                        leader: 0,
                        command: "ls".into(),
                        ended: Ended::Signal(2),
                    },
                },
                Entry {
                    tick: 5,
                    event: Event::Interrupted { agent: 1 },
                },
            ],
            inbox: vec![
                InboxItem {
                    kind: InboxKind::Reply,
                    agent: 0,
                    detail: "read it".into(),
                    researcher: None,
                    open: false,
                    tick: 3,
                },
                InboxItem {
                    kind: InboxKind::Change,
                    agent: 1,
                    detail: "Changed a".into(),
                    researcher: Some(2),
                    open: true,
                    tick: 6,
                },
            ],
            budget: 123_400,
            max_researchers: 0,
            research_queue: VecDeque::new(),
            plans: Vec::new(),
            changes: Vec::new(),
        };
        w.agents[1].cost = 4_321;
        w.agents[1].card = Some(0);
        w.agents[1].taint = Taint::Soft;
        w.agents[1].commands = vec!["/compact".into(), "/clear".into()];
        w.agents[1].changed_at = 66;
        w.agents[2].parent = Some(1);
        w.agents[2].gone = true;
        w.agents[2].taint = Taint::Full;
        w.agents[3].cost = 100;
        for i in 0..250 {
            w.agents[1].trace(i, format!("line {i}"));
        }
        w.agents[0].trace(7, "TOKEN=abc");
        w
    }

    #[derive(Clone, Debug, PartialEq, Eq)]
    struct Owned {
        repos: Vec<PathBuf>,
        models: Vec<Option<String>>,
        tiered: Vec<bool>,
        woken: Vec<u32>,
        posted: Vec<u32>,
        mail: Vec<Vec<Mail>>,
        notes: Vec<Vec<String>>,
        inflight: Vec<Vec<String>>,
        sessions: Vec<Option<Resumable>>,
        posts: HashMap<CardId, Post>,
        offered: HashMap<CardId, Vec<AgentId>>,
        changes: HashMap<AgentId, worktree::Change>,
    }

    impl Owned {
        fn kept(&self) -> Kept<'_> {
            Kept {
                repos: self.repos.iter().map(PathBuf::as_path).collect(),
                models: &self.models,
                tiered: &self.tiered,
                woken: &self.woken,
                posted: &self.posted,
                mail: &self.mail,
                notes: &self.notes,
                inflight: &self.inflight,
                sessions: &self.sessions,
                posts: &self.posts,
                offered: &self.offered,
                changes: &self.changes,
            }
        }

        fn found(&self) -> Found {
            Found {
                repos: self.repos.clone(),
                models: self.models.clone(),
                tiered: self.tiered.clone(),
                woken: self.woken.clone(),
                posted: self.posted.clone(),
                mail: self.mail.clone(),
                notes: self.notes.clone(),
                inflight: self.inflight.clone(),
                sessions: self.sessions.clone(),
                posts: self.posts.clone(),
                offered: self.offered.clone(),
                changes: self.changes.clone(),
            }
        }
    }

    fn kept() -> Owned {
        Owned {
            repos: vec!["/work/one".into(), "/work/two".into()],
            models: vec![None, Some("gpt".into()), Some("flash".into()), None],
            tiered: vec![false, true, false, false],
            woken: vec![0, 3, 0, 1],
            posted: vec![2, 0, 0, 0],
            mail: vec![
                Vec::new(),
                vec![
                    Mail {
                        from: 0,
                        kind: MsgKind::Assign,
                        body: "fix the parser\nin src".into(),
                    },
                    Mail {
                        from: 2,
                        kind: MsgKind::Done,
                        body: "tests pass".into(),
                    },
                ],
                Vec::new(),
                Vec::new(),
            ],
            notes: vec![
                vec!["the person applied it".into()],
                Vec::new(),
                Vec::new(),
                Vec::new(),
            ],
            inflight: vec![
                Vec::new(),
                vec!["go on".into(), "then stop".into()],
                Vec::new(),
                Vec::new(),
            ],
            sessions: vec![
                Some(Resumable {
                    session: "686c6d27-d50b-4ae2-ad6a-159bf2984671".into(),
                    spent: 524,
                }),
                None,
                Some(Resumable {
                    session: "thread_9".into(),
                    spent: 0,
                }),
                None,
            ],
            posts: HashMap::from([(
                1,
                Post {
                    by: 0,
                    body: "the README".into(),
                },
            )]),
            offered: HashMap::from([(1, vec![1, 2])]),
            changes: HashMap::from([(
                1,
                worktree::Change {
                    path: "/trees/codex-1".into(),
                    tree: "abc123".into(),
                    files: vec![worktree::FileChange {
                        path: "a.txt".into(),
                        added: 3,
                        removed: 1,
                        binary: false,
                    }],
                    patch: "diff --git a/a.txt b/a.txt\n".into(),
                },
            )]),
        }
    }

    /// The fields the store keeps of an agent, side by side.
    fn agent_view(a: &Agent) -> impl PartialEq + std::fmt::Debug {
        (
            (
                a.id,
                a.project,
                a.name.clone(),
                a.vendor,
                a.role,
                a.parent,
                a.card,
            ),
            (a.cost, a.taint, a.gone, a.changed_at, a.traced),
            (a.commands.clone(), a.trace.clone()),
        )
    }

    fn card_view(c: &Card) -> impl PartialEq + std::fmt::Debug {
        (
            c.id,
            c.project,
            c.title.clone(),
            c.owner,
            c.column,
            c.story
                .iter()
                .map(|s| (s.tick, s.event.clone()))
                .collect::<Vec<_>>(),
            c.started,
            c.finished,
        )
    }

    fn inbox_view(i: &InboxItem) -> impl PartialEq + std::fmt::Debug {
        (
            i.kind,
            i.agent,
            i.detail.clone(),
            i.researcher,
            i.open,
            i.tick,
        )
    }

    fn same(w: &World, back: &World) {
        assert_eq!((back.started_at, back.budget), (w.started_at, w.budget));
        assert_eq!(
            back.projects
                .iter()
                .map(|p| (p.id, p.name.clone(), p.leader, p.closed))
                .collect::<Vec<_>>(),
            w.projects
                .iter()
                .map(|p| (p.id, p.name.clone(), p.leader, p.closed))
                .collect::<Vec<_>>()
        );
        for (a, b) in w.agents.iter().zip(&back.agents) {
            assert_eq!(agent_view(b), agent_view(a), "agent {}", a.id);
        }
        assert_eq!(back.agents.len(), w.agents.len());
        for (a, b) in w.cards.iter().zip(&back.cards) {
            assert_eq!(card_view(b), card_view(a), "card {}", a.id);
        }
        assert_eq!(back.cards.len(), w.cards.len());
        assert_eq!(
            back.journal
                .iter()
                .map(|e| (e.tick, e.event.clone()))
                .collect::<Vec<_>>(),
            w.journal
                .iter()
                .map(|e| (e.tick, e.event.clone()))
                .collect::<Vec<_>>()
        );
        assert_eq!(
            back.inbox.iter().map(inbox_view).collect::<Vec<_>>(),
            w.inbox.iter().map(inbox_view).collect::<Vec<_>>()
        );
    }

    #[test]
    fn everything_saved_comes_back_with_the_agents_asleep() {
        let (w, k) = (world(), kept());
        let mut store = Store::in_memory();
        assert!(store.save(&w, &k.kept()).unwrap());
        assert!(!store.save(&w, &k.kept()).unwrap(), "nothing moved");
        let back = store.load(100).unwrap().expect("a world");
        assert_eq!(back.skipped, 0);
        same(&w, &back.world);
        assert_eq!(back.found, k.found());
        let b = &back.world;
        assert_eq!(
            b.agents.iter().map(|a| a.status).collect::<Vec<_>>(),
            [Status::Asleep, Status::Asleep, Status::Idle, Status::Asleep],
            "every agent sleeps, a gone one is idle"
        );
        assert!(
            b.agents
                .iter()
                .all(|a| a.talk.is_none() && a.activity.is_none())
        );
        assert_eq!(b.agents[1].trace.len(), 200, "the kept tail");
        assert_eq!(b.agents[1].trace[0], w.agents[1].trace[0]);
        assert_eq!(b.agents[1].traced, 250);
        // Lines are numbered by `traced`: the kept 200 of 250 are 50..249.
        let span = |agent: i64| -> (i64, i64) {
            store
                .raw()
                .query_row(
                    "SELECT min(seq), max(seq) FROM trace WHERE agent = ?1",
                    params![agent],
                    |r| Ok((r.get(0)?, r.get(1)?)),
                )
                .unwrap()
        };
        assert_eq!(span(1), (50, 249));
        assert_eq!(span(0), (0, 0));
        assert_eq!((b.tick, b.run_tick, b.run_spent), (0, 0, 0), "this run's");
        assert_eq!(b.spent(), 4_421);
        // Loaded is saved: nothing to write until something moves.
        assert!(!store.save(&back.world, &back.found().kept()).unwrap());
    }

    impl Loaded {
        fn found(&self) -> Owned {
            let f = &self.found;
            Owned {
                repos: f.repos.clone(),
                models: f.models.clone(),
                tiered: f.tiered.clone(),
                woken: f.woken.clone(),
                posted: f.posted.clone(),
                mail: f.mail.clone(),
                notes: f.notes.clone(),
                inflight: f.inflight.clone(),
                sessions: f.sessions.clone(),
                posts: f.posts.clone(),
                offered: f.offered.clone(),
                changes: f.changes.clone(),
            }
        }
    }

    #[test]
    fn an_empty_file_holds_no_world() {
        let mut store = Store::in_memory();
        assert!(store.load(100).unwrap().is_none());
    }

    /// Each change to one table alone is seen, written, and read back.
    #[test]
    fn a_save_writes_what_moved_and_only_that() {
        let (mut w, mut k) = (world(), kept());
        let mut store = Store::in_memory();
        store.save(&w, &k.kept()).unwrap();
        type Move = Box<dyn Fn(&mut World, &mut Owned)>;
        let moves: Vec<(&str, Move)> = vec![
            ("budget", Box::new(|w, _| w.budget += 1)),
            ("started_at", Box::new(|w, _| w.started_at += 1)),
            (
                "project name",
                Box::new(|w, _| w.projects[1].name = "three".into()),
            ),
            (
                "project closed",
                Box::new(|w, _| w.projects[1].closed = false),
            ),
            ("project leader", Box::new(|w, _| w.projects[1].leader = 2)),
            (
                "project repo",
                Box::new(|_, k| k.repos[1] = "/elsewhere".into()),
            ),
            ("agent cost", Box::new(|w, _| w.agents[3].cost += 1)),
            ("agent card", Box::new(|w, _| w.agents[3].card = Some(2))),
            ("agent gone", Box::new(|w, _| w.agents[3].gone = true)),
            (
                "agent name",
                Box::new(|w, _| w.agents[3].name = "lead-three".into()),
            ),
            ("agent project", Box::new(|w, _| w.agents[3].project = 0)),
            (
                "agent vendor",
                Box::new(|w, _| w.agents[3].vendor = Vendor::Gemini),
            ),
            (
                "agent role",
                Box::new(|w, _| w.agents[3].role = Role::Worker),
            ),
            (
                "agent parent",
                Box::new(|w, _| w.agents[3].parent = Some(0)),
            ),
            (
                "agent taint",
                Box::new(|w, _| w.agents[3].taint = Taint::Full),
            ),
            (
                "agent changed_at",
                Box::new(|w, _| w.agents[3].changed_at = 9),
            ),
            (
                "agent commands",
                Box::new(|w, _| w.agents[3].commands = vec!["/x".into()]),
            ),
            (
                "agent model",
                Box::new(|_, k| k.models[3] = Some("opus".into())),
            ),
            ("agent tiered", Box::new(|_, k| k.tiered[3] = true)),
            ("agent woken", Box::new(|_, k| k.woken[3] += 1)),
            ("agent posted", Box::new(|_, k| k.posted[3] += 1)),
            ("trace", Box::new(|w, _| w.agents[3].trace(80, "again"))),
            (
                "trace past the cap",
                Box::new(|w, _| {
                    for i in 0..300 {
                        w.agents[3].trace(81, format!("more {i}"));
                    }
                }),
            ),
            (
                "card column",
                Box::new(|w, _| w.cards[1].column = Column::Working),
            ),
            ("card owner", Box::new(|w, _| w.cards[1].owner = Some(1))),
            (
                "card title",
                Box::new(|w, _| w.cards[1].title = "write the docs".into()),
            ),
            ("card project", Box::new(|w, _| w.cards[1].project = 1)),
            ("card started", Box::new(|w, _| w.cards[1].started = 21)),
            ("card finished", Box::new(|w, _| w.cards[1].finished = 30)),
            (
                "story",
                Box::new(|w, _| {
                    w.cards[1].story.push(Story {
                        tick: 31,
                        event: StoryEvent::Took { agent: 1 },
                    })
                }),
            ),
            (
                "new card",
                Box::new(|w, _| {
                    w.cards.push(Card {
                        id: 3,
                        project: 0,
                        title: "new".into(),
                        owner: None,
                        column: Column::Waiting,
                        story: vec![Story {
                            tick: 40,
                            event: StoryEvent::Decision("yes".into()),
                        }],
                        symbols: Vec::new(),
                        started: 40,
                        finished: 0,
                    })
                }),
            ),
            (
                "journal",
                Box::new(|w, _| {
                    w.journal.push(Entry {
                        tick: 50,
                        event: Event::Left { agent: 2 },
                    })
                }),
            ),
            ("inbox open", Box::new(|w, _| w.inbox[1].open = false)),
            (
                "inbox detail",
                Box::new(|w, _| w.inbox[0].detail = "read that".into()),
            ),
            (
                "inbox kind",
                Box::new(|w, _| w.inbox[0].kind = InboxKind::Sensitive),
            ),
            ("inbox agent", Box::new(|w, _| w.inbox[0].agent = 3)),
            (
                "inbox researcher",
                Box::new(|w, _| w.inbox[0].researcher = Some(0)),
            ),
            ("inbox tick", Box::new(|w, _| w.inbox[0].tick = 4)),
            (
                "new inbox item",
                Box::new(|w, _| {
                    w.inbox.push(InboxItem {
                        kind: InboxKind::Research,
                        agent: 0,
                        detail: "x".into(),
                        researcher: None,
                        open: true,
                        tick: 60,
                    })
                }),
            ),
            (
                "mail",
                Box::new(|_, k| {
                    k.mail[0].push(Mail {
                        from: 1,
                        kind: MsgKind::Ask,
                        body: "which".into(),
                    })
                }),
            ),
            ("mail read", Box::new(|_, k| k.mail[1].clear())),
            ("notes", Box::new(|_, k| k.notes[1].push("rejected".into()))),
            (
                "inflight",
                Box::new(|_, k| {
                    k.inflight[1].remove(0);
                }),
            ),
            (
                "post",
                Box::new(|_, k| {
                    k.posts.insert(
                        2,
                        Post {
                            by: 3,
                            body: "b".into(),
                        },
                    );
                }),
            ),
            (
                "post gone",
                Box::new(|_, k| {
                    k.posts.remove(&1);
                }),
            ),
            (
                "offered",
                Box::new(|_, k| k.offered.get_mut(&1).unwrap().push(3)),
            ),
            (
                "offer gone",
                Box::new(|_, k| {
                    k.offered.remove(&1);
                }),
            ),
            (
                "change replaced",
                Box::new(|_, k| {
                    let c = k.changes.get_mut(&1).unwrap();
                    c.tree = "def456".into();
                    c.patch = "other".into();
                    c.files[0].added = 9;
                    c.files[0].binary = true;
                    c.path = "/trees/elsewhere".into();
                }),
            ),
            (
                "change gone",
                Box::new(|_, k| {
                    k.changes.remove(&1);
                }),
            ),
        ];
        for (what, change) in moves {
            change(&mut w, &mut k);
            assert!(store.save(&w, &k.kept()).unwrap(), "{what}: not seen");
            assert!(!store.save(&w, &k.kept()).unwrap(), "{what}: written twice");
            let back = store.load(100).unwrap().unwrap();
            same(&w, &back.world);
            assert_eq!(back.found, k.found(), "{what}");
            assert_eq!(
                back.world.agents[3].trace.len(),
                w.agents[3].trace.len(),
                "{what}: the trace tail"
            );
            let kept: i64 = store
                .raw()
                .query_row("SELECT count(*) FROM trace WHERE agent = 3", [], |r| {
                    r.get(0)
                })
                .unwrap();
            assert!(kept <= 200, "{what}: {kept} trace rows kept");
        }
    }

    #[test]
    fn the_file_is_the_persons_alone_and_one_imece_has_it() {
        let d = dir("lock");
        let path = d.join(FILE);
        let (w, k) = (world(), kept());
        {
            let mut first = Store::open(&path).unwrap();
            assert!(first.save(&w, &k.kept()).unwrap());
            let mode = |p: &Path| std::fs::metadata(p).unwrap().permissions().mode() & 0o777;
            assert_eq!(mode(&path), 0o600);
            assert_eq!(mode(&d.join("imece.sqlite.lock")), 0o600);
            let e = Store::open(&path).err().expect("refused");
            assert!(e.contains("another imece is running"), "{e}");
        }
        // Gone with the first: the next run reads what it wrote.
        let mut second = Store::open(&path).unwrap();
        let back = second.load(100).unwrap().unwrap();
        same(&w, &back.world);
        assert_eq!(back.found, k.found());
        let _ = std::fs::remove_dir_all(&d);
    }

    #[test]
    fn a_lock_that_is_being_let_go_of_is_waited_for() {
        let d = dir("letgo");
        let path = d.join(FILE);
        let first = Store::open(&path).unwrap();
        let letting_go = std::thread::spawn(move || {
            std::thread::sleep(std::time::Duration::from_millis(150));
            drop(first);
        });
        // Taken within the tries, once the first is gone.
        let second = Store::open(&path);
        assert!(second.is_ok(), "{:?}", second.err());
        letting_go.join().unwrap();
        let _ = std::fs::remove_dir_all(&d);
    }

    #[test]
    fn a_file_from_a_newer_imece_is_refused() {
        let d = dir("newer");
        let path = d.join(FILE);
        {
            let store = Store::open(&path).unwrap();
            store
                .raw()
                .execute(
                    "UPDATE meta SET value = '99' WHERE key = 'schema_version'",
                    [],
                )
                .unwrap();
        }
        let e = Store::open(&path).err().expect("refused");
        assert!(e.contains("newer imece") && e.contains("schema 99"), "{e}");
        let _ = std::fs::remove_dir_all(&d);
    }

    #[test]
    fn the_journal_loads_its_newest_lines_and_goes_on_from_the_last() {
        let (mut w, k) = (world(), kept());
        let mut store = Store::in_memory();
        store.save(&w, &k.kept()).unwrap();
        let back = store.load(2).unwrap().unwrap();
        assert_eq!(
            back.world
                .journal
                .iter()
                .map(|e| e.tick)
                .collect::<Vec<_>>(),
            [4, 5],
            "the newest two, oldest first"
        );
        // Saved from the cut world: the new line follows the old ones.
        let mut cut = back.world;
        cut.journal.push(Entry {
            tick: 6,
            event: Event::Left { agent: 2 },
        });
        assert!(store.save(&cut, &k.kept()).unwrap());
        // And the next ones follow that, each save counting on from the
        // file's last line, not from the lines it has in memory.
        for tick in [7, 8] {
            cut.journal.push(Entry {
                tick,
                event: Event::Left { agent: 2 },
            });
            assert!(store.save(&cut, &k.kept()).unwrap());
        }
        let all = store.load(100).unwrap().unwrap();
        assert_eq!(
            all.world.journal.iter().map(|e| e.tick).collect::<Vec<_>>(),
            [1, 2, 3, 4, 5, 6, 7, 8]
        );
        // The kinds are columns too, for the file's other readers.
        let kinds: Vec<String> = store
            .raw()
            .prepare("SELECT kind FROM journal ORDER BY seq")
            .unwrap()
            .query_map([], |r| r.get(0))
            .unwrap()
            .map(Result::unwrap)
            .collect();
        assert_eq!(
            kinds,
            [
                "Opened",
                "Hired",
                "Sent",
                "Shell",
                "Interrupted",
                "Left",
                "Left",
                "Left"
            ]
        );
        let kinds: Vec<String> = store
            .raw()
            .prepare("SELECT kind FROM story WHERE card = 0 ORDER BY seq")
            .unwrap()
            .query_map([], |r| r.get(0))
            .unwrap()
            .map(Result::unwrap)
            .collect();
        assert_eq!(kinds, ["Took", "Sent", "Interrupted"]);
        let (project, agent): (i64, i64) = store
            .raw()
            .query_row(
                "SELECT project, agent FROM journal WHERE seq = 3",
                [],
                |r| Ok((r.get(0)?, r.get(1)?)),
            )
            .unwrap();
        assert_eq!((project, agent), (0, 0), "the sender's project");
        w.journal.clear();
    }

    #[test]
    fn lines_another_imece_wrote_are_skipped_and_counted() {
        let (w, k) = (world(), kept());
        let mut store = Store::in_memory();
        store.save(&w, &k.kept()).unwrap();
        store
            .raw()
            .execute_batch(
                "INSERT INTO journal (seq, tick, project, agent, kind, data) \
                 VALUES (99, 1, 0, 0, 'Future', '{\"Future\":{}}');
                 INSERT INTO story (card, seq, tick, kind, data) \
                 VALUES (0, 99, 1, 'Future', 'not json');
                 INSERT INTO story (card, seq, tick, kind, data) \
                 VALUES (77, 0, 1, 'Took', '{\"Took\":{\"agent\":1}}');",
            )
            .unwrap();
        let back = store.load(100).unwrap().unwrap();
        assert_eq!(
            back.skipped, 2,
            "a story of a card that is not there is not a line"
        );
        same(&w, &back.world);
    }

    #[test]
    fn rows_out_of_order_are_a_broken_file() {
        for (table, row) in [
            (
                "projects (id, name, leader, closed, repo)",
                "(5, 'x', 0, 0, '/x')",
            ),
            (
                "agents (id, project, name, vendor, role, parent, card, cost, taint, gone, \
                 changed_at, traced, commands, model, tiered, woken, posted)",
                "(5, 0, 'x', 'Claude', 'Leader', NULL, NULL, 0, 'Clean', 0, 0, 0, '[]', NULL, 0, 0, 0)",
            ),
            (
                "cards (id, project, title, owner, col, started, finished)",
                "(5, 0, 'x', NULL, 'Waiting', 0, 0)",
            ),
            (
                "inbox (id, kind, agent, detail, researcher, open, tick)",
                "(5, 'Reply', 0, 'x', NULL, 1, 0)",
            ),
        ] {
            let mut store = Store::in_memory();
            store
                .raw()
                .execute_batch(&format!(
                    "INSERT INTO meta VALUES ('started_at', '1'); INSERT INTO {table} VALUES {row};"
                ))
                .unwrap();
            let e = store.load(100).err().expect("refused").to_string();
            assert!(e.contains("5 is out of order"), "{table}: {e}");
        }
        // A word the file holds that imece does not know.
        let mut store = Store::in_memory();
        store
            .raw()
            .execute_batch(
                "INSERT INTO meta VALUES ('started_at', '1'); \
                 INSERT INTO agents VALUES (0, 0, 'x', 'Clod', 'Leader', NULL, NULL, 0, 'Clean', \
                 0, 0, 0, '[]', NULL, 0, 0, 0, NULL, 0);",
            )
            .unwrap();
        let e = store.load(100).err().expect("refused").to_string();
        assert!(e.contains("\"Clod\""), "{e}");
    }

    #[test]
    fn a_tidy_lets_old_journal_lines_go_but_keeps_what_a_load_takes() {
        let (w, k) = (world(), kept());
        let mut store = Store::in_memory();
        store.save(&w, &k.kept()).unwrap();
        store.raw().execute("DELETE FROM journal", []).unwrap();
        // One hundred days on: lines older than ninety days are the ones
        // before tick 172_800 (ten days of five-second ticks).
        let now = w.started_at + 100 * 86_400;
        let cutoff = 10 * 86_400 / SECONDS_PER_TICK;
        assert_eq!(cutoff, 172_800);
        let rows = [
            (1, 0),          // old, past the newest three: goes
            (2, cutoff - 1), // old by one tick: goes
            (3, cutoff),     // at the cutoff: stays
            (4, 0),          // old: goes
            (5, cutoff + 1), // young: stays
            (6, cutoff * 2), // young: stays
            (7, 0),          // old, the last one past the newest three: goes
            (8, 0),          // old, but among the newest three: stays
            (9, 0),
            (10, 0),
        ];
        for (seq, tick) in rows {
            store
                .raw()
                .execute(
                    "INSERT INTO journal (seq, tick, project, agent, kind, data) \
                     VALUES (?1, ?2, 0, 0, 'Failed', '{\"Failed\":{\"agent\":0}}')",
                    params![seq, big(tick)],
                )
                .unwrap();
        }
        let tidied = store.tidy(now, 3).unwrap();
        assert_eq!(tidied.journal, 4);
        let left: Vec<i64> = store
            .raw()
            .prepare("SELECT seq FROM journal ORDER BY seq")
            .unwrap()
            .query_map([], |r| r.get(0))
            .unwrap()
            .map(Result::unwrap)
            .collect();
        assert_eq!(left, [3, 5, 6, 8, 9, 10]);
        // Nothing more to let go of; a young world has nothing at all.
        assert_eq!(store.tidy(now, 3).unwrap(), Tidied::default());
        assert_eq!(
            store.tidy(w.started_at + 89 * 86_400, 0).unwrap(),
            Tidied::default(),
            "nothing is ninety days old yet"
        );
        assert_eq!(
            store.tidy(w.started_at + 90 * 86_400, 0).unwrap(),
            Tidied::default(),
            "the first tick is ninety days old to the second, not older"
        );
        assert_eq!(
            store
                .tidy(w.started_at + 90 * 86_400 + SECONDS_PER_TICK, 0)
                .unwrap()
                .journal,
            3,
            "one tick more: the lines of the first tick go"
        );
        // Lines 3, 5 and 6 are left. The load goes on from the last line
        // kept, so the file's numbering stays one sequence.
        let l = store.load(10).unwrap().unwrap();
        assert_eq!(l.world.journal.len(), 3);
        let mut w2 = l.world;
        w2.journal.push(Entry {
            tick: 5,
            event: Event::Failed { agent: 0 },
        });
        let k2 = l.found;
        store
            .save(
                &w2,
                &Kept {
                    repos: k2.repos.iter().map(PathBuf::as_path).collect(),
                    models: &k2.models,
                    tiered: &k2.tiered,
                    woken: &k2.woken,
                    posted: &k2.posted,
                    mail: &k2.mail,
                    notes: &k2.notes,
                    inflight: &k2.inflight,
                    sessions: &k2.sessions,
                    posts: &k2.posts,
                    offered: &k2.offered,
                    changes: &k2.changes,
                },
            )
            .unwrap();
        let newest: i64 = store
            .raw()
            .query_row("SELECT MAX(seq) FROM journal", [], |r| r.get(0))
            .unwrap();
        assert_eq!(newest, 7);
        // A file without a world is left alone.
        let mut empty = Store::in_memory();
        assert_eq!(empty.tidy(now, 3).unwrap(), Tidied::default());
    }

    #[test]
    fn a_tidy_packs_a_file_that_is_mostly_air() {
        let (w, k) = (world(), kept());
        let mut store = Store::in_memory();
        store.save(&w, &k.kept()).unwrap();
        let page: i64 = store
            .raw()
            .query_row("PRAGMA page_size", [], |r| r.get(0))
            .unwrap();
        // Enough old lines to fill many pages, then let them all go.
        let filler = "x".repeat(page as usize);
        for seq in 100..400 {
            store
                .raw()
                .execute(
                    "INSERT INTO journal (seq, tick, project, agent, kind, data) \
                     VALUES (?1, 0, 0, 0, 'Failed', ?2)",
                    params![seq, filler],
                )
                .unwrap();
        }
        let before: i64 = store
            .raw()
            .query_row("PRAGMA page_count", [], |r| r.get(0))
            .unwrap();
        assert!(before > 300, "{before} pages");
        let tidied = store.tidy(w.started_at + 100 * 86_400, 0).unwrap();
        assert!(tidied.journal >= 300, "{tidied:?}");
        assert!(tidied.vacuumed, "{tidied:?}");
        let (free, after): (i64, i64) = store
            .raw()
            .query_row(
                "SELECT * FROM pragma_freelist_count, pragma_page_count",
                [],
                |r| Ok((r.get(0)?, r.get(1)?)),
            )
            .unwrap();
        assert_eq!(free, 0, "packed");
        assert!(after < before / 4, "{after} of {before} pages left");
        assert!(!store.tidy(w.started_at + 100 * 86_400, 0).unwrap().vacuumed);
        // The threshold: a quarter of the file, free pages or none.
        assert!(!needs_vacuum(0, 0));
        assert!(!needs_vacuum(0, 4));
        assert!(needs_vacuum(1, 4), "exactly a quarter");
        assert!(!needs_vacuum(1, 5));
        assert!(needs_vacuum(3, 12));
        assert!(!needs_vacuum(2, 9));
        assert!(needs_vacuum(9, 9), "all air");
    }

    #[test]
    fn the_growing_tables_are_read_by_their_keys() {
        let (w, k) = (world(), kept());
        let mut store = Store::in_memory();
        store.save(&w, &k.kept()).unwrap();
        for (query, plan) in store.plans() {
            assert!(!plan.contains("TEMP B-TREE"), "{query}: {plan}");
            if query.contains("WHERE") {
                assert!(
                    plan.contains("SEARCH") && plan.contains("PRIMARY KEY"),
                    "{query}: {plan}"
                );
            }
        }
    }

    #[test]
    fn numbers_cross_into_sqlite_and_back() {
        assert_eq!(n(7), 7);
        assert_eq!(n(usize::MAX), i64::MAX);
        assert_eq!(u(7), 7);
        assert_eq!(u(-1), 0);
        assert_eq!(opt(Some(3)), Some(3));
        assert_eq!(opt(None), None);
        assert_eq!(uopt(Some(3)), Some(3));
        assert_eq!(big(u64::MAX), i64::MAX);
        assert_eq!(big(9), 9);
        assert_eq!(ubig(-5), 0);
        assert_eq!(ubig(9), 9);
        assert_eq!(tag(&Role::Helper), "Helper");
        assert_eq!(untag::<Role>("Helper").unwrap(), Role::Helper);
        assert!(untag::<Role>("Boss").is_err());
        let (kind, data) = event_json(&StoryEvent::Interrupted);
        assert_eq!(
            (kind.as_str(), data.as_str()),
            ("Interrupted", "\"Interrupted\"")
        );
        let (kind, data) = event_json(&Event::Left { agent: 4 });
        assert_eq!(
            (kind.as_str(), data.as_str()),
            ("Left", "{\"Left\":{\"agent\":4}}")
        );
        assert_ne!(hash(&1u8), hash(&2u8));
        assert_eq!(hash(&"a"), hash(&"a"));
    }

    #[test]
    fn a_session_that_changes_is_written_again() {
        let (w, mut k) = (world(), kept());
        let mut store = Store::in_memory();
        store.save(&w, &k.kept()).unwrap();
        k.sessions[1] = Some(Resumable {
            session: "aaaaaaaa-0000-4000-8000-000000000000".into(),
            spent: 7,
        });
        assert!(store.save(&w, &k.kept()).unwrap(), "a new session moved");
        k.sessions[0].as_mut().unwrap().spent = 600;
        assert!(store.save(&w, &k.kept()).unwrap(), "more spent moved");
        k.sessions[2] = None;
        assert!(store.save(&w, &k.kept()).unwrap(), "a forgotten one moved");
        assert!(!store.save(&w, &k.kept()).unwrap());
        let back = store.load(100).unwrap().unwrap();
        assert_eq!(back.found.sessions, k.sessions);
        assert_eq!(back.found.sessions[0].as_ref().unwrap().spent, 600);
        assert_eq!(back.found.sessions[2], None);
    }

    #[test]
    fn a_file_of_schema_1_gets_the_session_columns() {
        let d = dir("schema1");
        std::fs::create_dir_all(&d).unwrap();
        let path = d.join(FILE);
        {
            // The agents table as schema 1 had it, with one agent in it.
            let conn = Connection::open(&path).unwrap();
            conn.execute_batch(
                "CREATE TABLE meta (key TEXT PRIMARY KEY, value TEXT NOT NULL) WITHOUT ROWID;
                 INSERT INTO meta VALUES ('schema_version', '1'), ('started_at', '1700000000'),
                     ('budget', '500000');
                 CREATE TABLE projects (
                     id INTEGER PRIMARY KEY, name TEXT NOT NULL, leader INTEGER NOT NULL,
                     closed INTEGER NOT NULL, repo TEXT NOT NULL);
                 INSERT INTO projects VALUES (0, 'one', 0, 0, '/repo/one');
                 CREATE TABLE agents (
                     id INTEGER PRIMARY KEY, project INTEGER NOT NULL, name TEXT NOT NULL,
                     vendor TEXT NOT NULL, role TEXT NOT NULL, parent INTEGER, card INTEGER,
                     cost INTEGER NOT NULL, taint TEXT NOT NULL, gone INTEGER NOT NULL,
                     changed_at INTEGER NOT NULL, traced INTEGER NOT NULL, commands TEXT NOT NULL,
                     model TEXT, tiered INTEGER NOT NULL, woken INTEGER NOT NULL,
                     posted INTEGER NOT NULL);
                 INSERT INTO agents VALUES (0, 0, 'lead', 'Claude', 'Leader', NULL, NULL, 12,
                     'Clean', 0, 3, 0, '[]', NULL, 0, 0, 0);",
            )
            .unwrap();
        }
        let mut store = Store::open(&path).unwrap();
        let version: String = store
            .raw()
            .query_row(
                "SELECT value FROM meta WHERE key = 'schema_version'",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(version, "2");
        let back = store.load(100).unwrap().expect("a world");
        assert_eq!(back.world.agents[0].name, "lead");
        assert_eq!(back.found.sessions, [None]);
        assert_eq!(back.saved_at, None, "an older file does not say when");
        // Saved with a session, read back with it.
        let (mut w, mut found) = (back.world, back.found);
        found.sessions[0] = Some(Resumable {
            session: "686c6d27-d50b-4ae2-ad6a-159bf2984671".into(),
            spent: 524,
        });
        w.agents[0].cost = 13;
        let kept = Kept {
            repos: found.repos.iter().map(PathBuf::as_path).collect(),
            models: &found.models,
            tiered: &found.tiered,
            woken: &found.woken,
            posted: &found.posted,
            mail: &found.mail,
            notes: &found.notes,
            inflight: &found.inflight,
            sessions: &found.sessions,
            posts: &found.posts,
            offered: &found.offered,
            changes: &found.changes,
        };
        assert!(store.save(&w, &kept).unwrap());
        drop(store);
        // Opening again does not add the columns twice.
        let mut store = Store::open(&path).unwrap();
        let back = store.load(100).unwrap().unwrap();
        assert_eq!(back.found.sessions, found.sessions);
        let _ = std::fs::remove_dir_all(&d);
    }

    /// The file says when it was last written: with every save that writes
    /// something (not one that has nothing to write), and at a leave,
    /// which writes nothing else.
    #[test]
    fn the_file_remembers_when_it_was_last_written() {
        let (mut w, k) = (world(), kept());
        let mut store = Store::in_memory();
        let saved_at = |store: &mut Store| store.load(100).unwrap().unwrap().saved_at;
        assert!(store.save_at(&w, &k.kept(), 1_700_000_500).unwrap());
        assert_eq!(saved_at(&mut store), Some(1_700_000_500));
        assert!(!store.save_at(&w, &k.kept(), 1_700_000_600).unwrap());
        assert_eq!(
            saved_at(&mut store),
            Some(1_700_000_500),
            "nothing moved, nothing written"
        );
        w.budget += 1;
        assert!(store.save_at(&w, &k.kept(), 1_700_000_600).unwrap());
        assert_eq!(saved_at(&mut store), Some(1_700_000_600));
        store.leave(1_700_000_700).unwrap();
        assert_eq!(saved_at(&mut store), Some(1_700_000_700));
        let rows: i64 = store
            .raw()
            .query_row("SELECT count(*) FROM meta", [], |r| r.get(0))
            .unwrap();
        assert_eq!(rows, 4, "schema_version, started_at, budget, saved_at");
        // `save` takes the time from the clock.
        w.budget += 1;
        assert!(store.save(&w, &k.kept()).unwrap());
        assert!(saved_at(&mut store).unwrap().abs_diff(unix_now()) <= 2);
    }
}
