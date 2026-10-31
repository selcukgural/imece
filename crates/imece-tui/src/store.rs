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

use std::collections::{BTreeSet, HashMap, HashSet, VecDeque};
use std::fs::OpenOptions;
use std::hash::{DefaultHasher, Hash, Hasher};
use std::os::unix::fs::OpenOptionsExt;
use std::path::{Path, PathBuf};

use imece_agent::worktree;
use rusqlite::{Connection, OptionalExtension, params};
use serde::{Serialize, de::DeserializeOwned};

use crate::model::*;

/// The file's layout; a file from a newer imece is refused.
/// 3 (2026-10-30): the `Question` inbox kind and the `ToYou` events (no
/// table changed; an older imece would not know the tags).
/// 4 (2026-10-30): the `Approval` inbox kind and the `Decided` events,
/// the same way.
/// 5 (2026-10-30): the runs on a change's tree (`changes.runs`).
/// 6 (2026-10-31): the card an inbox item came on (`inbox.card`).
/// 7 (2026-10-31): whether the person pinned a project (`projects.pinned`).
/// 8 (2026-10-31): the journal lines the person marked (`journal.marked`).
/// 9 (2026-10-31): imece's own paid calls, by day (`paid`).
/// 10 (2026-11-01): the paths copied into a project's worktrees
/// (`projects.copies`).
pub const SCHEMA_VERSION: i64 = 10;
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
    closed INTEGER NOT NULL, repo TEXT NOT NULL, pinned INTEGER NOT NULL DEFAULT 0);
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
-- Read newest first with a cap (`load`), which the rowid order gives; the
-- marked lines older than those through `journal_marked` (`prepare`).
CREATE TABLE IF NOT EXISTS journal (
    seq INTEGER PRIMARY KEY, tick INTEGER NOT NULL, project INTEGER NOT NULL,
    agent INTEGER NOT NULL, kind TEXT NOT NULL, data TEXT NOT NULL,
    marked INTEGER NOT NULL DEFAULT 0);
CREATE TABLE IF NOT EXISTS inbox (
    id INTEGER PRIMARY KEY, kind TEXT NOT NULL, agent INTEGER NOT NULL,
    detail TEXT NOT NULL, researcher INTEGER, open INTEGER NOT NULL, tick INTEGER NOT NULL,
    card INTEGER);
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
    files TEXT NOT NULL, patch TEXT NOT NULL, runs TEXT NOT NULL DEFAULT '[]');
CREATE TABLE IF NOT EXISTS paid (
    day INTEGER PRIMARY KEY, calls INTEGER NOT NULL, cost INTEGER NOT NULL) WITHOUT ROWID;
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
    /// The runs on each change's tree, as the world had them.
    pub runs: HashMap<AgentId, Vec<Ran>>,
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
    /// The journal lines that are saved marked.
    marks: BTreeSet<usize>,
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
    /// Where the world's journal lines are in the file.
    seqs: Seqs,
    /// The lock file, held open: its lease is imece's run.
    _lock: std::fs::File,
}

/// Where each of the world's journal lines is in the file (its `seq`): the
/// lines a load brought, which may have gaps (a line this imece could not
/// read, the old ones only a mark brought), and after them the lines this
/// run wrote, one after the other from the last one the file had.
#[derive(Default)]
struct Seqs {
    loaded: Vec<i64>,
    /// The file's last `seq` at the load.
    last: i64,
}

impl Seqs {
    fn of(&self, i: usize) -> i64 {
        match self.loaded.get(i) {
            Some(&seq) => seq,
            None => self.last + n(i - self.loaded.len()) + 1,
        }
    }
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

/// Why the file at `path` was refused (`e`), and what the person can do:
/// imece never moves, mends or writes over a file it cannot read, since
/// what it holds may still be wanted (a newer imece reads it).
pub fn unreadable(path: &Path, e: impl std::fmt::Display) -> String {
    let p = path.display();
    format!(
        "{p}: {e}\nimece left the file as it was. To start afresh, move it somewhere else \
         with its {p}-wal and {p}-shm files, if they are there; the next start makes a new one."
    )
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
        let conn = Connection::open(path).map_err(|e| unreadable(path, e))?;
        let mut store = Store {
            conn,
            saved: Saved::default(),
            seqs: Seqs::default(),
            _lock: lock,
        };
        store.prepare().map_err(|e| unreadable(path, e))?;
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
            seqs: Seqs::default(),
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
        if !self.has_column("changes", "runs")? {
            self.conn
                .execute_batch("ALTER TABLE changes ADD COLUMN runs TEXT NOT NULL DEFAULT '[]';")?;
        }
        if !self.has_column("inbox", "card")? {
            self.conn
                .execute_batch("ALTER TABLE inbox ADD COLUMN card INTEGER;")?;
        }
        if !self.has_column("projects", "pinned")? {
            self.conn.execute_batch(
                "ALTER TABLE projects ADD COLUMN pinned INTEGER NOT NULL DEFAULT 0;",
            )?;
        }
        if !self.has_column("projects", "copies")? {
            self.conn.execute_batch(
                "ALTER TABLE projects ADD COLUMN copies TEXT NOT NULL DEFAULT '[]';",
            )?;
        }
        if !self.has_column("journal", "marked")? {
            self.conn.execute_batch(
                "ALTER TABLE journal ADD COLUMN marked INTEGER NOT NULL DEFAULT 0;",
            )?;
        }
        // The few marked lines, without reading the many others (`load`);
        // after the column, which an older file gets just above.
        self.conn.execute_batch(
            "CREATE INDEX IF NOT EXISTS journal_marked ON journal (seq) WHERE marked = 1;",
        )?;
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
        // the bulk of a file that is tidied every start, are not read. A
        // line the person marked stays.
        let journal = self.conn.execute(
            "DELETE FROM journal WHERE seq <= ?1 AND tick < ?2 AND marked = 0",
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
                "INSERT OR REPLACE INTO projects (id, name, leader, closed, repo, pinned, \
                 copies) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
                params![
                    n(q.id),
                    q.name,
                    n(q.leader),
                    q.closed,
                    k.repos
                        .get(p)
                        .map_or(String::new(), |r| r.display().to_string()),
                    q.pinned,
                    serde_json::to_string(&q.copies).unwrap_or_else(|_| "[]".into()),
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
        let from = self.saved.journal.min(w.journal.len());
        for (i, e) in w.journal.iter().enumerate().skip(from) {
            let (kind, data) = event_json(&e.event);
            let agent = e.event.agent();
            tx.execute(
                "INSERT OR REPLACE INTO journal (seq, tick, project, agent, kind, data, marked) \
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
                params![
                    self.seqs.of(i),
                    big(e.tick),
                    n(w.agents[agent].project),
                    n(agent),
                    kind,
                    data,
                    w.marks.contains(&i)
                ],
            )?;
        }
        for &(i, marked) in &plan.marks {
            tx.execute(
                "UPDATE journal SET marked = ?1 WHERE seq = ?2",
                params![marked, self.seqs.of(i)],
            )?;
        }
        for &i in &plan.inbox {
            let it = &w.inbox[i];
            tx.execute(
                "INSERT OR REPLACE INTO inbox (id, kind, agent, detail, researcher, open, tick, \
                 card) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
                params![
                    n(i),
                    tag(&it.kind),
                    n(it.agent),
                    it.detail,
                    opt(it.researcher),
                    it.open,
                    big(it.tick),
                    opt(it.card)
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
            let runs = w.change_of(id).map_or(&[][..], |c| &c.runs);
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
                "INSERT OR REPLACE INTO changes (agent, path, tree, files, patch, runs) \
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
                params![
                    n(id),
                    c.path.display().to_string(),
                    c.tree,
                    serde_json::Value::Array(files).to_string(),
                    c.patch,
                    serde_json::to_string(runs).unwrap_or_default()
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

    /// imece's own paid calls on `day` (days since the epoch, UTC): how
    /// many, and what they cost or hold reserved, in ten-thousandths of a
    /// dollar.
    pub fn paid_on(&self, day: u64) -> rusqlite::Result<(u32, u64)> {
        self.conn
            .query_row(
                "SELECT calls, cost FROM paid WHERE day = ?1",
                params![big(day)],
                |r| Ok((r.get::<_, u32>(0)?, ubig(r.get(1)?))),
            )
            .optional()
            .map(Option::unwrap_or_default)
    }

    /// What all of imece's own paid calls cost, every day together: part
    /// of the pot, as what the agents spent is.
    pub fn paid_total(&self) -> rusqlite::Result<u64> {
        self.conn
            .query_row("SELECT COALESCE(SUM(cost), 0) FROM paid", [], |r| {
                Ok(ubig(r.get(0)?))
            })
    }

    /// Reserves one paid call on `day` that may cost up to `cap`, before
    /// it is made, when the day's calls stay at most `calls` and their
    /// cost at most `cost` with it; whether it did. One transaction: what
    /// is counted is what was checked.
    pub fn reserve(&mut self, day: u64, cap: u64, calls: u32, cost: u64) -> rusqlite::Result<bool> {
        let tx = self
            .conn
            .transaction_with_behavior(rusqlite::TransactionBehavior::Immediate)?;
        let (had_calls, had_cost): (u32, u64) = tx
            .query_row(
                "SELECT calls, cost FROM paid WHERE day = ?1",
                params![big(day)],
                |r| Ok((r.get(0)?, ubig(r.get(1)?))),
            )
            .optional()?
            .unwrap_or_default();
        if had_calls >= calls || had_cost + cap > cost {
            return Ok(false);
        }
        tx.execute(
            "INSERT OR REPLACE INTO paid (day, calls, cost) VALUES (?1, ?2, ?3)",
            params![big(day), had_calls + 1, big(had_cost + cap)],
        )?;
        tx.commit()?;
        Ok(true)
    }

    /// A call reserved on `day` with `cap` cost `spent`: the day keeps
    /// that instead of the cap.
    pub fn settle(&mut self, day: u64, cap: u64, spent: u64) -> rusqlite::Result<()> {
        self.conn.execute(
            "UPDATE paid SET cost = MAX(cost - ?2, 0) + ?3 WHERE day = ?1",
            params![big(day), big(cap), big(spent)],
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
                q.pinned,
                &q.copies,
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
        // A line whose mark came or went is written again (a new line goes
        // in with its mark, and the writing again comes after it).
        plan.marks = (s.marks.symmetric_difference(&w.marks))
            .map(|&i| (i, w.marks.contains(&i)))
            .collect();
        after.marks = w.marks.clone();
        for (i, it) in w.inbox.iter().enumerate() {
            let h = hash(&(
                it.kind,
                it.agent,
                &it.detail,
                it.researcher,
                it.open,
                it.tick,
                it.card,
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
            marks: Default::default(),
            limits: Vec::new(),
            tick: 0,
            started_at,
            run_spent: 0,
            spending: Spending::default(),
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
            paid: 0,
            suggested: None,
        };
        let mut found = Found::default();
        let mut saved = Saved::default();
        let mut skipped = 0;
        let conn = &self.conn;
        let mut st = conn.prepare(
            "SELECT id, name, leader, closed, repo, pinned, copies FROM projects ORDER BY id",
        )?;
        for row in st.query_map([], |r| {
            Ok((
                u(r.get(0)?),
                r.get::<_, String>(1)?,
                u(r.get(2)?),
                r.get::<_, bool>(3)?,
                r.get::<_, String>(4)?,
                r.get::<_, bool>(5)?,
                r.get::<_, String>(6)?,
            ))
        })? {
            let (id, name, leader, closed, repo, pinned, copies) = row?;
            let copies: Vec<String> = serde_json::from_str(&copies)
                .map_err(|e| bad(format!("project {id}'s copies: {e}")))?;
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
                pinned,
                copies,
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
        // The newest lines, and the marked ones older than those: a mark
        // keeps its line within reach whatever its age.
        let mut st =
            conn.prepare("SELECT seq, tick, data, marked FROM journal ORDER BY seq DESC LIMIT ?1")?;
        let mut newest = Vec::new();
        let mut oldest = i64::MAX;
        for row in st.query_map(params![n(journal_keep)], |r| {
            Ok((
                r.get::<_, i64>(0)?,
                ubig(r.get(1)?),
                r.get::<_, String>(2)?,
                r.get::<_, bool>(3)?,
            ))
        })? {
            let (seq, tick, data, marked) = row?;
            saved.journal_seq = saved.journal_seq.max(seq);
            oldest = oldest.min(seq);
            match serde_json::from_str(&data) {
                Ok(event) => newest.push((seq, Entry { tick, event }, marked)),
                Err(_) => skipped += 1,
            }
        }
        newest.reverse();
        let mut st = conn.prepare(
            "SELECT seq, tick, data FROM journal WHERE marked = 1 AND seq < ?1 ORDER BY seq",
        )?;
        let mut lines = Vec::new();
        for row in st.query_map(params![oldest], |r| {
            Ok((r.get::<_, i64>(0)?, ubig(r.get(1)?), r.get::<_, String>(2)?))
        })? {
            let (seq, tick, data) = row?;
            match serde_json::from_str(&data) {
                Ok(event) => lines.push((seq, Entry { tick, event }, true)),
                Err(_) => skipped += 1,
            }
        }
        lines.extend(newest);
        let mut seqs = Vec::with_capacity(lines.len());
        for (seq, entry, marked) in lines {
            if marked {
                w.marks.insert(w.journal.len());
            }
            seqs.push(seq);
            w.journal.push(entry);
        }
        let mut st = conn.prepare(
            "SELECT id, kind, agent, detail, researcher, open, tick, card FROM inbox ORDER BY id",
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
                    card: uopt(r.get(7)?),
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
        let mut st = conn.prepare("SELECT agent, path, tree, files, patch, runs FROM changes")?;
        for row in st.query_map([], |r| {
            Ok((
                u(r.get(0)?),
                r.get::<_, String>(1)?,
                r.get::<_, String>(2)?,
                r.get::<_, String>(3)?,
                r.get::<_, String>(4)?,
                r.get::<_, String>(5)?,
            ))
        })? {
            let (agent, path, tree, files, patch, runs) = row?;
            let runs: Vec<Ran> = serde_json::from_str(&runs).unwrap_or_default();
            if !runs.is_empty() {
                found.runs.insert(agent, runs);
            }
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
        self.seqs = Seqs {
            loaded: seqs,
            last: saved.journal_seq,
        };
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
            "SELECT seq, tick, data, marked FROM journal ORDER BY seq DESC LIMIT 5000",
            "SELECT id, kind, agent, detail, researcher, open, tick, card FROM inbox ORDER BY id",
            "DELETE FROM journal WHERE seq <= 10 AND tick < 20 AND marked = 0",
            "SELECT seq, tick, data FROM journal WHERE marked = 1 AND seq < 10 ORDER BY seq",
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
    /// Journal lines whose mark changed, with the mark they have now.
    marks: Vec<(usize, bool)>,
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
            && self.marks.is_empty()
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
            marks: Default::default(),
            limits: Vec::new(),
            tick: 77,
            started_at: 1_700_000_000,
            run_spent: 5,
            spending: Spending::default(),
            projects: vec![
                Project {
                    id: 0,
                    name: "one".into(),
                    leader: 0,
                    trunk: None,
                    repo: &[],
                    closed: false,
                    pinned: false,
                    copies: Vec::new(),
                },
                Project {
                    id: 1,
                    name: "two".into(),
                    leader: 3,
                    trunk: None,
                    repo: &[],
                    closed: true,
                    pinned: false,
                    copies: Vec::new(),
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
                        took: Some(1_250),
                    },
                },
                Entry {
                    tick: 5,
                    event: Event::Interrupted { agent: 1 },
                },
                Entry {
                    tick: 6,
                    event: Event::ToYou {
                        from: 1,
                        kind: MsgKind::Ask,
                        text: "which?".into(),
                    },
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
                    card: None,
                },
                InboxItem {
                    kind: InboxKind::Change,
                    agent: 1,
                    detail: "Changed a".into(),
                    researcher: Some(2),
                    open: true,
                    tick: 6,
                    card: Some(0),
                },
                InboxItem {
                    kind: InboxKind::Question,
                    agent: 1,
                    detail: "which?".into(),
                    researcher: None,
                    open: true,
                    tick: 6,
                    card: None,
                },
                InboxItem {
                    kind: InboxKind::Approval,
                    agent: 1,
                    detail: "Bash: make".into(),
                    researcher: None,
                    open: true,
                    tick: 6,
                    card: None,
                },
            ],
            budget: 123_400,
            max_researchers: 0,
            research_queue: VecDeque::new(),
            plans: Vec::new(),
            changes: Vec::new(),
            paid: 0,
            suggested: None,
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
        w.changes.push(Change {
            agent: 1,
            title: "done".into(),
            files: Vec::new(),
            patch: String::new(),
            runs: runs(),
        });
        w
    }

    /// The runs on agent 1's change in [`world`]: one clean, one failed,
    /// one the deadline ended.
    fn runs() -> Vec<Ran> {
        vec![
            Ran {
                line: "cargo test".into(),
                code: Some(0),
                timed_out: false,
            },
            Ran {
                line: "cargo clippy".into(),
                code: Some(101),
                timed_out: false,
            },
            Ran {
                line: "make".into(),
                code: None,
                timed_out: true,
            },
        ]
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
        runs: HashMap<AgentId, Vec<Ran>>,
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
                runs: self.runs.clone(),
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
            runs: HashMap::from([(1, runs())]),
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
            i.card,
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
        assert_eq!((b.tick, b.run_spent), (0, 0), "this run's");
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
                runs: f.runs.clone(),
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
            ("inbox card", Box::new(|w, _| w.inbox[0].card = Some(0))),
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
                        card: None,
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
                Box::new(|w, k| {
                    let runs = vec![runs()[1].clone()];
                    w.changes[0].runs = runs.clone();
                    k.runs.insert(1, runs);
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
                Box::new(|w, k| {
                    w.changes.clear();
                    k.changes.remove(&1);
                    k.runs.remove(&1);
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
        assert!(e.contains("imece left the file as it was"), "{e}");
        let _ = std::fs::remove_dir_all(&d);
    }

    #[test]
    fn a_file_imece_cannot_read_is_left_as_it_was_and_the_person_told_how_to_start_afresh() {
        let d = dir("unreadable");
        let path = d.join(FILE);
        let junk = b"not a database, but somebody's file all the same".repeat(200);
        std::fs::create_dir_all(&d).unwrap();
        std::fs::write(&path, &junk).unwrap();
        let e = Store::open(&path).err().expect("refused");
        let p = path.display();
        assert_eq!(
            e.lines().nth(1),
            Some(
                format!(
                    "imece left the file as it was. To start afresh, move it somewhere else \
                     with its {p}-wal and {p}-shm files, if they are there; the next start \
                     makes a new one."
                )
                .as_str()
            ),
            "{e}"
        );
        assert!(e.starts_with(&format!("{p}: ")), "{e}");
        assert_eq!(std::fs::read(&path).unwrap(), junk, "written over");
        // Again: still refused, still untouched.
        assert!(Store::open(&path).is_err());
        assert_eq!(std::fs::read(&path).unwrap(), junk);
        let _ = std::fs::remove_dir_all(&d);
    }

    /// The journal lines the file has marked, by their `seq`.
    fn marked_seqs(store: &Store) -> Vec<i64> {
        store
            .raw()
            .prepare("SELECT seq FROM journal WHERE marked = 1 ORDER BY seq")
            .unwrap()
            .query_map([], |r| r.get(0))
            .unwrap()
            .map(Result::unwrap)
            .collect()
    }

    #[test]
    fn a_mark_is_saved_with_its_line_and_comes_back() {
        let (mut w, k) = (world(), kept());
        // Marked before the line was ever saved: it goes in marked.
        w.marks.insert(1);
        let mut store = Store::in_memory();
        store.save(&w, &k.kept()).unwrap();
        assert_eq!(marked_seqs(&store), [2]);
        // Marked and unmarked once saved: written again, and only that.
        w.marks.insert(4);
        w.marks.remove(&1);
        assert!(store.save(&w, &k.kept()).unwrap());
        assert_eq!(marked_seqs(&store), [5]);
        assert!(!store.save(&w, &k.kept()).unwrap(), "nothing moved");
        let back = store.load(100).unwrap().unwrap();
        assert_eq!(back.world.marks, BTreeSet::from([4]));
        assert_eq!(back.world.journal, w.journal);
    }

    #[test]
    fn a_marked_line_loads_past_the_newest_and_keeps_its_place() {
        let (mut w, k) = (world(), kept());
        // Ticks 1 to 6; the first and the third marked.
        w.marks.extend([0, 2]);
        let mut store = Store::in_memory();
        store.save(&w, &k.kept()).unwrap();
        let back = store.load(2).unwrap().unwrap();
        let ticks = |w: &World| w.journal.iter().map(|e| e.tick).collect::<Vec<_>>();
        assert_eq!(
            ticks(&back.world),
            [1, 3, 5, 6],
            "the marked, then the newest two"
        );
        assert_eq!(back.world.marks, BTreeSet::from([0, 1]));
        // Lines loaded with a gap between them are found where they are.
        let mut cut = back.world;
        cut.mark(1);
        cut.mark(3);
        for tick in [7, 8] {
            cut.journal.push(Entry {
                tick,
                event: Event::Left { agent: 2 },
            });
        }
        cut.mark(5);
        assert!(store.save(&cut, &k.kept()).unwrap());
        assert_eq!(marked_seqs(&store), [1, 6, 8]);
        let all = store.load(100).unwrap().unwrap();
        assert_eq!(ticks(&all.world), [1, 2, 3, 4, 5, 6, 7, 8]);
        assert_eq!(all.world.marks, BTreeSet::from([0, 5, 7]));
    }

    #[test]
    fn a_line_that_cannot_be_read_leaves_the_marks_where_they_are() {
        let (w, k) = (world(), kept());
        let mut store = Store::in_memory();
        store.save(&w, &k.kept()).unwrap();
        // The second line, from another imece, and the fourth marked.
        store
            .raw()
            .execute(
                "UPDATE journal SET data = '{\"Gone\":{}}' WHERE seq = 2",
                [],
            )
            .unwrap();
        store
            .raw()
            .execute("UPDATE journal SET marked = 1 WHERE seq IN (2, 4)", [])
            .unwrap();
        let back = store.load(100).unwrap().unwrap();
        assert_eq!(back.skipped, 1);
        assert_eq!(
            back.world.marks,
            BTreeSet::from([2]),
            "the fourth is third now"
        );
        let mut cut = back.world;
        cut.mark(2);
        cut.mark(3);
        store.save(&cut, &k.kept()).unwrap();
        assert_eq!(marked_seqs(&store), [2, 5], "the unread line keeps its own");
        // An old marked line that cannot be read counts as one too.
        let back = store.load(1).unwrap().unwrap();
        assert_eq!(back.skipped, 1);
        assert_eq!(back.world.journal.len(), 2);
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
            [5, 6],
            "the newest two, oldest first"
        );
        // Saved from the cut world: the new line follows the old ones.
        let mut cut = back.world;
        cut.journal.push(Entry {
            tick: 7,
            event: Event::Left { agent: 2 },
        });
        assert!(store.save(&cut, &k.kept()).unwrap());
        // And the next ones follow that, each save counting on from the
        // file's last line, not from the lines it has in memory.
        for tick in [8, 9] {
            cut.journal.push(Entry {
                tick,
                event: Event::Left { agent: 2 },
            });
            assert!(store.save(&cut, &k.kept()).unwrap());
        }
        let all = store.load(100).unwrap().unwrap();
        assert_eq!(
            all.world.journal.iter().map(|e| e.tick).collect::<Vec<_>>(),
            [1, 2, 3, 4, 5, 6, 7, 8, 9]
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
                "ToYou",
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
    fn what_the_person_decided_of_an_ask_is_kept() {
        let (mut w, k) = (world(), kept());
        w.journal.push(Entry {
            tick: 9,
            event: Event::Decided {
                agent: 1,
                what: "Bash: make\nmake check".into(),
                allowed: false,
            },
        });
        let mut store = Store::in_memory();
        store.save(&w, &k.kept()).unwrap();
        let back = store.load(100).unwrap().unwrap();
        same(&w, &back.world);
        let kind: String = store
            .raw()
            .query_row(
                "SELECT kind FROM journal ORDER BY seq DESC LIMIT 1",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(kind, "Decided");
        assert!(
            back.world
                .inbox
                .iter()
                .any(|i| i.kind == InboxKind::Approval && i.open),
            "the store keeps it open; the start closes it"
        );
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
    fn a_shell_line_saved_before_its_time_was_kept_loads_without_one() {
        let (w, k) = (world(), kept());
        let mut store = Store::in_memory();
        store.save(&w, &k.kept()).unwrap();
        store
            .raw()
            .execute_batch(
                "INSERT INTO journal (seq, tick, project, agent, kind, data) \
                 VALUES (99, 9, 0, 0, 'Shell', \
                 '{\"Shell\":{\"leader\":0,\"command\":\"make\",\"ended\":{\"Exit\":2}}}');",
            )
            .unwrap();
        let back = store.load(100).unwrap().unwrap();
        assert_eq!(back.skipped, 0);
        let last = &back.world.journal.last().unwrap().event;
        assert_eq!(
            *last,
            Event::Shell {
                leader: 0,
                command: "make".into(),
                ended: Ended::Exit(2),
                took: None,
            }
        );
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
            (4, 0),          // old, but marked: stays
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
                    "INSERT INTO journal (seq, tick, project, agent, kind, data, marked) \
                     VALUES (?1, ?2, 0, 0, 'Failed', '{\"Failed\":{\"agent\":0}}', ?3)",
                    params![seq, big(tick), seq == 4],
                )
                .unwrap();
        }
        let tidied = store.tidy(now, 3).unwrap();
        assert_eq!(tidied.journal, 3);
        let left: Vec<i64> = store
            .raw()
            .prepare("SELECT seq FROM journal ORDER BY seq")
            .unwrap()
            .query_map([], |r| r.get(0))
            .unwrap()
            .map(Result::unwrap)
            .collect();
        assert_eq!(left, [3, 4, 5, 6, 8, 9, 10]);
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
        // Lines 3, 4 (marked), 5 and 6 are left. The load goes on from the
        // last line kept, so the file's numbering stays one sequence.
        let l = store.load(10).unwrap().unwrap();
        assert_eq!(l.world.journal.len(), 4);
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
            // The marked lines have an index of their own, which holds
            // only them.
            let key = match query.contains("marked = 1") {
                true => "USING INDEX journal_marked",
                false => "PRIMARY KEY",
            };
            if query.contains("WHERE") {
                assert!(
                    plan.contains("SEARCH") && plan.contains(key),
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
    fn a_pin_is_saved_on_its_own_and_a_file_of_schema_6_gets_the_column() {
        let d = dir("schema6");
        std::fs::create_dir_all(&d).unwrap();
        let path = d.join(FILE);
        let mut w = world();
        {
            let mut store = Store::open(&path).unwrap();
            assert!(store.save(&w, &kept().kept()).unwrap());
            assert!(!store.save(&w, &kept().kept()).unwrap(), "nothing new");
            // A pin alone is something to save.
            w.projects[0].pinned = true;
            assert!(store.save(&w, &kept().kept()).unwrap());
            let back = store.load(100).unwrap().unwrap();
            let pins: Vec<bool> = back.world.projects.iter().map(|p| p.pinned).collect();
            assert_eq!(pins, [true, false]);
            // As schema 6 wrote it: no pins.
            store
                .raw()
                .execute_batch(
                    "ALTER TABLE projects DROP COLUMN pinned;
                     UPDATE meta SET value = '6' WHERE key = 'schema_version';",
                )
                .unwrap();
        }
        let mut store = Store::open(&path).unwrap();
        let back = store.load(100).unwrap().expect("a world");
        assert!(back.world.projects.iter().all(|p| !p.pinned));
        assert!(store.save(&w, &kept().kept()).unwrap());
        assert!(store.load(100).unwrap().unwrap().world.projects[0].pinned);
    }

    #[test]
    fn a_projects_copies_are_saved_on_their_own_and_schema_9_gets_the_column() {
        let d = dir("schema9");
        std::fs::create_dir_all(&d).unwrap();
        let path = d.join(FILE);
        let mut w = world();
        {
            let mut store = Store::open(&path).unwrap();
            assert!(store.save(&w, &kept().kept()).unwrap());
            // The list alone is something to save.
            w.projects[1].copies = vec![".env".into(), "target".into()];
            assert!(store.save(&w, &kept().kept()).unwrap());
            assert!(!store.save(&w, &kept().kept()).unwrap(), "nothing new");
            let back = store.load(100).unwrap().unwrap();
            assert!(back.world.projects[0].copies.is_empty());
            assert_eq!(back.world.projects[1].copies, [".env", "target"]);
            // As schema 9 wrote it: no copies.
            store
                .raw()
                .execute_batch(
                    "ALTER TABLE projects DROP COLUMN copies;
                     UPDATE meta SET value = '9' WHERE key = 'schema_version';",
                )
                .unwrap();
        }
        let mut store = Store::open(&path).unwrap();
        let back = store.load(100).unwrap().expect("a world");
        assert!(back.world.projects.iter().all(|p| p.copies.is_empty()));
        // A list that is not one is refused, never read as empty.
        store
            .raw()
            .execute_batch("UPDATE projects SET copies = 'x' WHERE id = 1;")
            .unwrap();
        let e = store.load(100).err().expect("refused").to_string();
        assert!(e.contains("project 1's copies"), "{e}");
    }

    #[test]
    fn a_file_of_schema_5_gets_the_inbox_card_column() {
        let d = dir("schema5");
        std::fs::create_dir_all(&d).unwrap();
        let path = d.join(FILE);
        {
            // A whole file as schema 5 wrote it, its inbox without cards.
            let mut store = Store::open(&path).unwrap();
            assert!(store.save(&world(), &kept().kept()).unwrap());
            store
                .raw()
                .execute_batch(
                    "ALTER TABLE inbox DROP COLUMN card;
                     UPDATE meta SET value = '5' WHERE key = 'schema_version';",
                )
                .unwrap();
        }
        let mut store = Store::open(&path).unwrap();
        let back = store.load(100).unwrap().expect("a world");
        let cards: Vec<Option<CardId>> = back.world.inbox.iter().map(|i| i.card).collect();
        assert_eq!(cards, [None; 4], "no card was kept");
        assert_eq!(back.world.inbox.len(), world().inbox.len());
        // The next save keeps them.
        let mut w = back.world;
        w.inbox[1].card = Some(0);
        assert!(store.save(&w, &kept().kept()).unwrap());
        let again = store.load(100).unwrap().unwrap();
        assert_eq!(again.world.inbox[1].card, Some(0));
    }

    #[test]
    fn a_file_of_schema_4_gets_the_runs_column() {
        let d = dir("schema4");
        std::fs::create_dir_all(&d).unwrap();
        let path = d.join(FILE);
        {
            // A whole file as schema 4 wrote it, its change without runs.
            let mut store = Store::open(&path).unwrap();
            assert!(store.save(&world(), &kept().kept()).unwrap());
            store
                .raw()
                .execute_batch(
                    "ALTER TABLE changes DROP COLUMN runs;
                     UPDATE meta SET value = '4' WHERE key = 'schema_version';",
                )
                .unwrap();
        }
        let mut store = Store::open(&path).unwrap();
        let back = store.load(100).unwrap().expect("a world");
        assert_eq!(back.found.changes, kept().changes);
        assert_eq!(back.found.runs, HashMap::new(), "no runs were kept");
        let runs: String = store
            .raw()
            .query_row("SELECT runs FROM changes WHERE agent = 1", [], |r| r.get(0))
            .unwrap();
        assert_eq!(runs, "[]");
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
                     'Clean', 0, 3, 0, '[]', NULL, 0, 0, 0);
                 CREATE TABLE journal (
                     seq INTEGER PRIMARY KEY, tick INTEGER NOT NULL, project INTEGER NOT NULL,
                     agent INTEGER NOT NULL, kind TEXT NOT NULL, data TEXT NOT NULL);
                 INSERT INTO journal VALUES (1, 2, 0, 0, 'Left', '{\"Left\":{\"agent\":0}}');",
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
        assert_eq!(version, SCHEMA_VERSION.to_string());
        let back = store.load(100).unwrap().expect("a world");
        assert_eq!(back.world.agents[0].name, "lead");
        assert!(!back.world.projects[0].pinned, "pinned came in later");
        assert_eq!(back.world.journal.len(), 1);
        assert!(back.world.marks.is_empty(), "marks came in later");
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
        // `save` takes the time from the clock (the clock itself, not what
        // the store thinks it is).
        w.budget += 1;
        assert!(store.save(&w, &k.kept()).unwrap());
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs();
        assert!(saved_at(&mut store).unwrap().abs_diff(now) <= 2);
        assert!(unix_now().abs_diff(now) <= 2);
    }

    #[test]
    fn a_paid_call_is_reserved_only_under_both_of_the_days_ceilings() {
        let mut st = Store::in_memory();
        assert_eq!(st.paid_on(7).unwrap(), (0, 0));
        // Two calls a day, 0.10 a day; each may cost 0.04.
        assert!(st.reserve(7, 400, 2, 1_000).unwrap());
        assert_eq!(st.paid_on(7).unwrap(), (1, 400));
        assert!(st.reserve(7, 400, 2, 1_000).unwrap());
        assert_eq!(st.paid_on(7).unwrap(), (2, 800));
        // The count is full: refused, nothing counted.
        assert!(!st.reserve(7, 100, 2, 1_000).unwrap());
        assert_eq!(st.paid_on(7).unwrap(), (2, 800));
        // Another day starts from nothing.
        assert!(
            st.reserve(8, 1_000, 3, 1_000).unwrap(),
            "up to the cost is room"
        );
        assert!(!st.reserve(8, 1, 3, 1_000).unwrap(), "one more is past it");
        assert_eq!(st.paid_on(8).unwrap(), (1, 1_000));
        assert_eq!(st.paid_total().unwrap(), 1_800);
    }

    #[test]
    fn a_settled_call_keeps_what_it_cost_instead_of_its_cap() {
        let mut st = Store::in_memory();
        assert_eq!(st.paid_total().unwrap(), 0);
        assert!(st.reserve(3, 500, 9, 9_000).unwrap());
        assert!(st.reserve(3, 500, 9, 9_000).unwrap());
        st.settle(3, 500, 37).unwrap();
        assert_eq!(st.paid_on(3).unwrap(), (2, 537));
        // A day that is not there takes nothing.
        st.settle(4, 500, 37).unwrap();
        assert_eq!(st.paid_on(4).unwrap(), (0, 0));
        assert_eq!(st.paid_total().unwrap(), 537);
    }

    #[test]
    fn paid_calls_stay_in_the_file() {
        let dir = std::env::temp_dir().join(format!("imece-paid-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join(FILE);
        {
            let mut st = Store::open(&path).unwrap();
            assert!(st.reserve(5, 200, 1, 1_000).unwrap());
            st.settle(5, 200, 12).unwrap();
        }
        let mut st = Store::open(&path).unwrap();
        assert_eq!(st.paid_on(5).unwrap(), (1, 12));
        assert!(
            !st.reserve(5, 1, 1, 1_000).unwrap(),
            "the day's one call was made"
        );
        drop(st);
        let _ = std::fs::remove_dir_all(&dir);
    }
}
