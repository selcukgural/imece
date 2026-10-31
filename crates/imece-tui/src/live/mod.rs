//! Live agents: real Claude Code sessions feed the model the simulation
//! feeds. Projects, each a checkout with a leader that reads it and
//! workers that each change their own git worktree of it: the one imece
//! runs in to begin with, more as the person opens them (`:open`).
//!
//! The team starts as the leader alone. The leader hires workers as the
//! work needs them (`hire`), up to the cap the person set, choosing a tier
//! (cheap, standard, strong) rather than a model name; each vendor maps the
//! tiers to its models. A worker whose tier model turns out not to be
//! available here is restarted once on the vendor's default.
//!
//! Money is one pot for the whole team: once what all agents spent reaches
//! the budget, nobody is prompted again. Each session also gets what was
//! left of the budget when it started as its hard cap (`--max-budget-usd`),
//! so a session that runs away on its own can never pass the budget either.
//! When the person raises the budget (`:budget`), a session that hit its
//! old cap is started again with the new remainder.
//!
//! Agents message each other through imece's `send` tool (an MCP server,
//! one socket per agent, so the sender is the socket, never a claim). A
//! message waits in the recipient's mail; a waking one starts its next turn
//! as soon as it is free, the others ride along with that turn.
//!
//! Agents may also put cards on the board (`post`); workers take them
//! (`claim`). An open card is offered to one idle worker at a time, as a
//! turn of its own; it goes to the next only when that worker's turn ended
//! without taking it, so a card wakes each worker at most once.

use std::collections::{HashMap, VecDeque};
use std::io;
use std::path::{Path, PathBuf};
use std::sync::mpsc::{self, Receiver, Sender};
use std::thread;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use imece_agent::mcp::{Answer, Server, Tool};
use imece_agent::{
    APPROVE_TOOL, Agent as Session, Ask, Confine, Event as AgentEvent, Mcp, Tier, child, claude,
    codex, gemini, run, worktree,
};
use serde_json::{Value, json};

use crate::i18n::{Lang, clean};
use crate::model::*;
use crate::store::{self, Found, Kept, Store};
use crate::ui::words;

mod bus;
mod changes;
mod hiring;
mod projects;
mod prompts;
mod remember;
mod sessions;
#[cfg(test)]
pub(crate) mod testing;
mod tools;

pub(crate) use self::prompts::MAX_NOTES_KIB;
pub use self::sessions::apply;
use self::{changes::*, hiring::*, projects::*, prompts::*, tools::*};

pub struct Options {
    pub repo: PathBuf,
    /// Where imece keeps what it remembers between runs: the worktrees
    /// and the store.
    pub state: PathBuf,
    /// Workers the leader may hire, at most.
    pub max_workers: usize,
    /// Helpers a worker may have at once, at most.
    pub max_helpers: usize,
    /// The leader's vendor.
    pub lead: Vendor,
    /// Models that stand in for the vendors' own, by vendor and tier; no
    /// vendor means the leader's. `--model NAME` is the standard tier.
    pub tiers: Vec<(Option<Vendor>, Tier, String)>,
    /// Ten-thousandths of a dollar, for all agents together; none keeps
    /// the pot as it was left, or [`DEFAULT_BUDGET`] the first time.
    pub budget: Option<u64>,
    /// Forget every agent's remembered session at the start (`--fresh`):
    /// each wakes in a new one, told where it left off, and what the
    /// vendors kept of the old ones goes.
    pub fresh: bool,
    /// Programs workers and helpers may run with the run tool, next to
    /// the built-in cargo and git commands.
    pub run_programs: Vec<String>,
    /// How long one run may take.
    pub run_timeout: Duration,
    /// How much of its vendor each agent's session gets (`--confine`).
    pub confine: Confine,
}

/// A coding agent imece knows: its program on PATH, and whether imece can
/// drive it yet.
pub type Known = (Vendor, &'static str, bool);

/// The coding agents imece knows.
pub const KNOWN: &[Known] = &[
    (Vendor::Claude, claude::PROGRAM, true),
    (Vendor::Codex, codex::PROGRAM, true),
    (Vendor::Gemini, gemini::PROGRAM, true),
];

/// The models a vendor's tiers stand for; none is the vendor's own default.
#[derive(Clone, Debug, PartialEq)]
struct Ladder {
    cheap: Option<String>,
    standard: Option<String>,
    strong: Option<String>,
}

/// A vendor imece can drive, found on PATH.
#[derive(Clone, Debug)]
struct Hireable {
    vendor: Vendor,
    program: String,
    ladder: Ladder,
}

/// Why a message could not reach an agent.
/// Claude Code's result subtype when a session passed its `--max-budget-usd`.
const CAPPED: &str = "error_max_budget_usd";

#[derive(Debug, PartialEq)]
pub enum Refusal {
    Budget,
    Start(String),
    /// The agent's session took no such command: why.
    Command(String),
}

/// A project's checkout and where its workers' worktrees live.
struct Place {
    repo: PathBuf,
    root: PathBuf,
}

/// An agent's session, if it has one.
enum Slot {
    None,
    /// Being started on its own thread (a first worktree checks out the
    /// whole tree); the person's messages wait here.
    Starting(Vec<String>),
    Running(Box<dyn Session>),
}

/// A started session (and where its worktree stands, when it has one), or
/// why it could not start. Last, for a woken worker's first prompt: the
/// line saying what its worktree holds and where it goes in the prompt,
/// read on the session's thread (see [`Live::brief`]).
type Ready = Result<
    (
        Box<dyn Session>,
        PathBuf,
        Option<worktree::Placed>,
        Option<(usize, String)>,
    ),
    String,
>;

/// A tool call of an agent (tool name, arguments), and where its answer
/// goes.
type Call = (AgentId, String, Value, Sender<Answer>);

/// What all agents together may spend until the person says otherwise,
/// in ten-thousandths of a dollar.
pub const DEFAULT_BUDGET: u64 = 50 * 10_000;

/// Journal lines loaded from the store at the start.
const JOURNAL_KEEP: usize = 5000;

/// The longest a woken agent's briefing gets, in characters, and the
/// longest one line of it.
const MAX_BRIEFING: usize = 6000;

const MAX_BRIEFING_LINE: usize = 300;

/// What imece's notes to an agent start with, ahead of the first.
const FROM_IMECE: &str = "From imece (not from the person or a teammate):\n- ";

/// The longest message body, in characters.
const MAX_BODY: usize = 4000;

/// Messages that may wait for one agent; more are refused until it reads.
const MAX_MAIL: usize = 20;

/// Turns an agent takes on its teammates' messages before the person has
/// to speak again: agents can not keep each other busy forever.
const MAX_WOKEN: u32 = 12;

/// The longest card title, in characters.
const MAX_TITLE: usize = 120;

/// Open cards one project's board may hold; more are refused.
const MAX_OPEN: usize = 20;

/// Cards one agent may post before the person has to speak again.
const MAX_POSTS: u32 = 10;

/// How long a tool call waits for the screen's loop to answer it.
const CALL_WAIT: Duration = Duration::from_secs(30);

/// How long a session's ask waits for the person's yes: the person may be
/// away for hours, and the session waits with them.
const APPROVE_WAIT: Duration = Duration::from_secs(24 * 3600);

/// How long one run may take unless `--run-timeout` says otherwise.
pub const DEFAULT_RUN_TIMEOUT: Duration = Duration::from_secs(600);

/// How long a session's process is given to be gone after it was asked to
/// stop, before what its vendor kept of the session is removed: a child
/// gets two grace periods of 1.5 s before it is killed for good.
const SETTLE: Duration = Duration::from_secs(4);

/// The longest command line the run tool takes, in characters.
const MAX_COMMAND: usize = 1000;

/// The largest change (as a patch) the person is shown.
const MAX_PATCH: usize = 4 << 20;

/// The most output of one run that goes back to the agent, in bytes.
const MAX_RUN_OUTPUT: usize = 48 << 10;
/// How often a worker's row learns where its worktree stands while it or
/// a helper of its works; between turns it learns at each turn's end.
const STAND_EVERY: Duration = Duration::from_secs(15);

/// Runs remembered with the tree they ran on, for the changes read after
/// them (see [`Change::runs`]); the oldest go first.
const RAN_KEEP: usize = 64;

/// imece's MCP servers, one socket per agent in a private directory.
struct Bus {
    dir: PathBuf,
    /// imece's own binary; sessions start it as `mcp-relay <socket>`.
    relay: String,
    servers: Vec<Server>,
}

impl Drop for Bus {
    fn drop(&mut self) {
        self.servers.clear();
        let _ = std::fs::remove_dir_all(&self.dir);
    }
}

/// What an earlier run left in the store, as the screen tells it at the
/// start.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct LeftBehind {
    /// Journal lines the earlier runs wrote; the lines after them are this
    /// start's (turns found cut short, sessions forgotten).
    pub journal: usize,
    /// Seconds since the earlier run last wrote the file; none for a file
    /// an older imece wrote.
    pub ago: Option<u64>,
}

/// A job for the git thread (see [`Live::git`]).
enum GitJob {
    /// Read worker `agent`'s change in `path` for its done titled
    /// `title`. `said` is what the worker is told before what the change
    /// is, on `reply`.
    Read {
        agent: AgentId,
        path: PathBuf,
        title: String,
        said: String,
        reply: Sender<Answer>,
    },
    /// Apply worker `agent`'s change to the checkout `repo`.
    Apply {
        agent: AgentId,
        repo: PathBuf,
        change: worktree::Change,
        title: String,
    },
    /// Move worker `agent`'s worktree `path` to the checkout `repo`'s
    /// HEAD if it can (see [`worktree::place`]), for the session of
    /// `generation`.
    Place {
        agent: AgentId,
        generation: u64,
        repo: PathBuf,
        path: PathBuf,
    },
    /// Read where worker `agent`'s worktree `path` stands next to the
    /// checkout `repo` (see [`worktree::stand`]).
    Stand {
        agent: AgentId,
        repo: PathBuf,
        path: PathBuf,
    },
}

/// What a git job came to, back on the screen's loop.
enum GitDone {
    Read {
        agent: AgentId,
        title: String,
        said: String,
        reply: Sender<Answer>,
        read: io::Result<Option<worktree::Change>>,
    },
    /// `mark` is the worktree's mark commit, when `applied` is ok.
    Applied {
        agent: AgentId,
        title: String,
        applied: io::Result<()>,
        mark: io::Result<String>,
    },
    Placed {
        agent: AgentId,
        generation: u64,
        placed: io::Result<worktree::Placed>,
    },
    Stood {
        agent: AgentId,
        stand: io::Result<worktree::Stand>,
    },
}

/// What a session waits on the person's yes for, and how the answer
/// reaches it.
struct Pending {
    what: Ask,
    reply: Reply,
}

/// How the person's answer reaches the session that asked.
enum Reply {
    /// A Claude Code session asked through imece's socket: the call
    /// waits for its answer, with the tool's input to give back.
    Socket { input: Value, to: Sender<Answer> },
    /// The session asked on its vendor's own wire (Codex): answered
    /// through it, by request.
    Session(String),
}

/// A wake that goes on with the agent's earlier session: what to start
/// over with, briefed, if the vendor can not find it.
struct Attempt {
    session: String,
    /// The first prompt as it was given, without imece's notes.
    text: String,
    /// imece's notes as they waited before the wake.
    notes: Vec<String>,
}

pub struct Live {
    /// Where the worktrees and the store live.
    state: PathBuf,
    /// What is remembered between runs; none in tests without a file.
    store: Option<Store>,
    /// What the store had of earlier runs, for the screen's "where you
    /// left off": how many journal lines were theirs (the ones after
    /// are this start's) and how many seconds ago the last of them wrote
    /// the file; none when nothing was remembered.
    left_off: Option<LeftBehind>,
    /// The project of the checkout imece was started in.
    home: ProjectId,
    /// Cards there were when the world was loaded: older ones are not
    /// offered to a sleeping worker, newer ones wake it.
    loaded_cards: usize,
    /// Agents remembered from an earlier run that have not started this
    /// run: the first prompt tells them where they left off.
    remembered: Vec<bool>,
    /// The prompts an earlier run's turn did not finish, for that briefing.
    interrupted: Vec<Vec<String>>,
    /// The vendor session each agent had last, to go on with at its wake;
    /// kept up to date from what the session reports.
    sessions: Vec<Option<Resumable>>,
    /// What the agent's current session had cost before this run resumed
    /// it (millionths of a dollar); 0 for a new session.
    floor: Vec<u64>,
    /// Wakes that go on with the earlier session, until the vendor
    /// confirms it or fails to find it.
    resuming: Vec<Option<Attempt>>,
    /// Claude Code's configuration directory, where its transcripts are.
    claude_home: PathBuf,
    /// Gemini CLI's configuration directory, where its chats are.
    gemini_config: PathBuf,
    /// How long a forgotten session's process, just asked to stop, is
    /// given to be gone before what it kept is removed.
    settle: Duration,
    /// Whether the sessions the store remembers are forgotten at the
    /// start (`--fresh`).
    fresh: bool,
    /// Events tagged with the agent and the session's generation, so a late
    /// event of an ended session never touches the next one.
    tx: Sender<(AgentId, u64, AgentEvent)>,
    rx: Receiver<(AgentId, u64, AgentEvent)>,
    ready_tx: Sender<(AgentId, u64, Ready)>,
    ready_rx: Receiver<(AgentId, u64, Ready)>,
    slots: Vec<Slot>,
    generation: Vec<u64>,
    /// What each agent's earlier sessions cost; a session reports only its
    /// own total.
    spent_before: Vec<u64>,
    /// The vendors the leader may hire from.
    vendors: Vec<Hireable>,
    /// Workers the leader may hire, at most.
    max_workers: usize,
    /// Helpers a worker may have at once, at most.
    max_helpers: usize,
    /// The vendor every leader runs on.
    lead: Vendor,
    /// Each project's checkout and worktrees root, by project id.
    places: Vec<Place>,
    /// The model each agent runs on; none for its vendor's default.
    models: Vec<Option<String>>,
    /// Whether the agent's model came from a tier, so a first turn that
    /// fails means the model is not available here and the vendor's
    /// standard model is tried instead.
    tiered: Vec<bool>,
    /// Whether the agent's current session has finished a turn.
    turned: Vec<bool>,
    /// The prompts each agent was given and has not finished, to give
    /// again after a fallback.
    inflight: Vec<Vec<String>>,
    /// Whether the agent's session was renewed after hitting its cap and
    /// has not finished a turn since: a second cap in a row is final.
    renewed: Vec<bool>,
    /// Helpers that sent done to their worker: each leaves when its turn
    /// ends, so the turn's cost is still reported.
    leaving: Vec<bool>,
    /// What imece itself has to tell each agent, at the start of its next
    /// turn: the person took or rejected its change.
    notes: Vec<Vec<String>>,
    /// Workers' changes waiting for the person, as git knows them; the
    /// screen's copy is in the world.
    changes: HashMap<AgentId, worktree::Change>,
    /// Git work on changes (reading a done's change, applying one) runs
    /// on a thread of its own, one job at a time in the order given: the
    /// screen never waits on git, two applies never meet in the checkout,
    /// and a change is read only after the apply asked before it.
    git: Sender<GitJob>,
    git_back: Receiver<GitDone>,
    /// Reads of each worker's change under way: a change is not applied
    /// while a newer one is being read.
    reading: Vec<usize>,
    /// Whether each worker's change is being applied.
    applying: Vec<bool>,
    /// The prompts each worker's session waits to be given while git
    /// moves its worktree to HEAD (see [`Live::follow`]); none when it
    /// is not moving.
    placing: Vec<Option<Vec<String>>>,
    /// Whether git reads where each worker's worktree stands; `true`
    /// when the worktree may have changed since that read was asked, so
    /// another follows it.
    standing: Vec<Option<bool>>,
    /// When each worker's stand was last asked for.
    stood_at: Vec<Option<Instant>>,
    /// Workers' and helpers' runs that ran on one tree from start to end,
    /// with that tree, oldest first, at most [`RAN_KEEP`]. They come from
    /// the runs' threads on `ran_back`, each before its run is answered,
    /// so a change read after the answer finds it there.
    ran: VecDeque<(String, Ran)>,
    ran_tx: Sender<(String, Ran)>,
    ran_back: Receiver<(String, Ran)>,
    bus: Option<Bus>,
    calls_tx: Sender<Call>,
    calls_rx: Receiver<Call>,
    /// Messages waiting for each agent's next turn.
    mail: Vec<Vec<Mail>>,
    /// Prompts each session was given and has not finished.
    turns: Vec<usize>,
    /// Agents the person spoke to whose answer has not come yet.
    asked: Vec<bool>,
    /// Turns each agent took on messages since the person last spoke.
    woken: Vec<u32>,
    /// The board's cards, by the card they made.
    posts: HashMap<CardId, Post>,
    /// Workers each open card was offered to, in order.
    offered: HashMap<CardId, Vec<AgentId>>,
    /// Cards each agent posted since the person last spoke.
    posted: Vec<u32>,
    /// The directory each agent's running session is in: where its runs go.
    cwds: Vec<Option<PathBuf>>,
    /// Each agent's runs under way, killed with its project or with imece.
    running: Vec<run::Running>,
    /// Programs workers and helpers may run, next to cargo and git.
    run_programs: Vec<String>,
    run_timeout: Duration,
    /// How much of its vendor each session gets; see [`Confine`].
    confine: Confine,
    /// What each agent's session waits on the person's yes for, in the
    /// order its approval items are in the inbox.
    pending: Vec<Vec<Pending>>,
}

/// The coding agents among `known` (see [`KNOWN`]) on PATH, run from the
/// checkout `repo` is in, with whether imece drives them.
pub fn probe(repo: &Path, known: &[Known]) -> Result<Vec<(Vendor, bool)>, String> {
    let repo = worktree::top(repo).map_err(|e| format!("{}: {e}", repo.display()))?;
    Ok(known
        .iter()
        .filter(|(_, program, _)| on_path(program, &repo))
        .map(|&(vendor, _, drives)| (vendor, drives))
        .collect())
}

/// The vendors among `present` that could lead: the ones imece drives.
pub fn leaders(present: &[(Vendor, bool)]) -> Vec<Vendor> {
    present
        .iter()
        .filter(|(_, drives)| *drives)
        .map(|(v, _)| *v)
        .collect()
}

/// Builds the team for the checkout `repo` is in, from the coding agents
/// `present` on PATH (see [`probe`]): what the store remembers of earlier
/// runs (every agent asleep), and the checkout at hand as a project if it
/// is not one yet, led by its leader alone, who hires the rest.
pub fn world(o: Options, present: &[(Vendor, bool)]) -> Result<(World, Live), String> {
    let repo = worktree::top(&o.repo).map_err(|e| format!("{}: {e}", o.repo.display()))?;
    let vendors = team_vendors(hireable(present, o.lead)?, o.lead, &o.tiers);
    let mut store = Store::open(&o.state.join(store::FILE))?;
    let loaded = store
        .load(JOURNAL_KEEP)
        .map_err(|e| format!("{}: {e}", o.state.join(store::FILE).display()))?;
    let now = now();
    // Old journal lines past what was just loaded go, and the file is
    // packed when it is mostly air. Nothing is lost when this fails: the
    // next start tries again, and there is nobody to tell yet.
    let _ = store.tidy(now, JOURNAL_KEEP);
    let left_off = loaded.as_ref().map(|l| LeftBehind {
        journal: l.world.journal.len(),
        ago: l.saved_at.map(|at| now.saturating_sub(at)),
    });
    let (mut w, found) = match loaded {
        Some(l) => (l.world, Some((l.found, l.skipped))),
        None => (
            World {
                marks: Default::default(),
                tick: 0,
                started_at: now,
                run_spent: 0,
                spending: Spending::default(),
                projects: Vec::new(),
                agents: Vec::new(),
                cards: Vec::new(),
                journal: Vec::new(),
                inbox: Vec::new(),
                budget: DEFAULT_BUDGET,
                max_researchers: 0,
                research_queue: Default::default(),
                plans: Vec::new(),
                changes: Vec::new(),
            },
            None,
        ),
    };
    if let Some(b) = o.budget {
        w.budget = b;
    }
    w.tick = now.saturating_sub(w.started_at) / SECONDS_PER_TICK;
    w.run_spent = w.spent();
    let mut live = Live::bare(vendors, o.lead);
    live.state = o.state;
    live.max_workers = o.max_workers;
    live.max_helpers = o.max_helpers;
    live.run_programs = o.run_programs;
    live.run_timeout = o.run_timeout;
    live.confine = o.confine;
    live.fresh = o.fresh;
    live.left_off = left_off;
    if let Some((found, skipped)) = found {
        live.adopt(&mut w, found, skipped);
    }
    let at = |q: &Project| live.places.get(q.id).is_some_and(|pl| pl.repo == repo);
    let home = w.open_projects().find(|q| at(q)).map(|q| q.id);
    live.home = match home {
        Some(p) => p,
        None => live
            .open_project(&mut w, &repo, None)
            .map_err(|e| e.describe())?,
    };
    // Said once per start: the sessions reach out of their directories
    // (a shell, the web), and what keeps them in is not imece.
    if live.confine != Confine::Strict {
        w.note(Event::Confine {
            leader: w.projects[live.home].leader,
            level: live.confine.wire().to_string(),
        });
    }
    let relay = std::env::current_exe().map_err(|e| format!("imece's own path: {e}"))?;
    live.open_bus(&w, relay.to_string_lossy().into_owned())
        .map_err(|e| format!("the agents' message bus: {e}"))?;
    live.store = Some(store);
    Ok((w, live))
}

/// Seconds since the Unix epoch.
fn now() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |d| d.as_secs())
}

impl Live {
    /// The checkout of project `p`.
    pub fn repo_of(&self, p: ProjectId) -> &Path {
        &self.places[p].repo
    }

    /// The project of the checkout imece was started in.
    pub fn home(&self) -> ProjectId {
        self.home
    }

    #[cfg(test)]
    pub(crate) fn set_home(&mut self, p: ProjectId) {
        self.home = p;
    }

    /// No projects and no agents yet; every leader will run on `lead`.
    fn bare(vendors: Vec<Hireable>, lead: Vendor) -> Live {
        let (tx, rx) = mpsc::channel();
        let (ready_tx, ready_rx) = mpsc::channel();
        let (calls_tx, calls_rx) = mpsc::channel();
        let (git, jobs) = mpsc::channel();
        let (done, git_back) = mpsc::channel();
        let (ran_tx, ran_back) = mpsc::channel();
        thread::spawn(move || git_thread(jobs, done));
        Live {
            state: std::env::temp_dir(),
            store: None,
            left_off: None,
            home: 0,
            loaded_cards: 0,
            remembered: Vec::new(),
            interrupted: Vec::new(),
            sessions: Vec::new(),
            floor: Vec::new(),
            resuming: Vec::new(),
            claude_home: claude::config_dir(),
            gemini_config: gemini::config_dir(),
            settle: SETTLE,
            fresh: false,
            tx,
            rx,
            ready_tx,
            ready_rx,
            slots: Vec::new(),
            generation: Vec::new(),
            spent_before: Vec::new(),
            vendors,
            max_workers: usize::MAX,
            max_helpers: usize::MAX,
            lead,
            places: Vec::new(),
            models: Vec::new(),
            tiered: Vec::new(),
            turned: Vec::new(),
            inflight: Vec::new(),
            renewed: Vec::new(),
            leaving: Vec::new(),
            notes: Vec::new(),
            changes: HashMap::new(),
            git,
            git_back,
            reading: Vec::new(),
            applying: Vec::new(),
            placing: Vec::new(),
            standing: Vec::new(),
            stood_at: Vec::new(),
            ran: VecDeque::new(),
            ran_tx,
            ran_back,
            bus: None,
            calls_tx,
            calls_rx,
            mail: Vec::new(),
            turns: Vec::new(),
            asked: Vec::new(),
            woken: Vec::new(),
            posts: HashMap::new(),
            offered: HashMap::new(),
            posted: Vec::new(),
            cwds: Vec::new(),
            running: Vec::new(),
            run_programs: Vec::new(),
            run_timeout: DEFAULT_RUN_TIMEOUT,
            confine: Confine::default(),
            pending: Vec::new(),
        }
    }

    /// The private directory a Gemini session of agent `name` of project
    /// `project` runs in: a sibling of the worktrees, kept between runs
    /// like them. A leader's is `lead` whatever the project is called, so
    /// a rename (`lead-NAME`) does not leave its session behind.
    fn gemini_home(&self, project: ProjectId, role: Role, name: &str) -> PathBuf {
        let mut dir = self.places[project].root.clone().into_os_string();
        dir.push("-gemini");
        let name = if role == Role::Leader { "lead" } else { name };
        PathBuf::from(dir).join(name)
    }

    /// Gives every agent a socket for imece's tools. Sessions started
    /// before have none.
    fn open_bus(&mut self, w: &World, relay: String) -> io::Result<()> {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_or(0, |d| d.subsec_nanos());
        // Short (socket paths are limited to about 100 bytes), private, and
        // new: an existing directory is refused, not reused.
        let dir = std::env::temp_dir().join(format!("imece-{}-{nanos:x}", std::process::id()));
        std::os::unix::fs::DirBuilderExt::mode(&mut std::fs::DirBuilder::new(), 0o700)
            .create(&dir)?;
        self.bus = Some(Bus {
            dir,
            relay,
            servers: Vec::new(),
        });
        for id in 0..self.slots.len() {
            self.bind(id, w.agents[id].role, w.agents[id].vendor)?;
        }
        Ok(())
    }

    /// Binds the socket of agent `id`, the next one to get a socket, with
    /// the tools of `role`, and on `vendor` Claude the tool its session
    /// asks through: the other vendors ask on their own wire, and their
    /// sessions must report exactly imece's tools. Without a bus (before
    /// it opens) there is nothing to bind.
    fn bind(&mut self, id: AgentId, role: Role, vendor: Vendor) -> io::Result<()> {
        let mut tools = tools_of(
            role,
            &self.vendor_names(),
            &self.run_programs,
            self.run_timeout,
        );
        if vendor == Vendor::Claude {
            tools.push(approve_tool());
        }
        let Some(bus) = &mut self.bus else {
            return Ok(());
        };
        assert_eq!(bus.servers.len(), id, "sockets are bound in agent order");
        let tx = self.calls_tx.clone();
        let run_timeout = self.run_timeout;
        let server = Server::bind(&bus.dir.join(format!("a{id}.sock")), tools, {
            move |name, args| {
                let (reply, answer) = mpsc::channel();
                tx.send((id, name.to_string(), args.clone(), reply))
                    .map_err(|_| "imece is closing".to_string())?;
                answer
                    .recv_timeout(call_wait(name, run_timeout))
                    .map_err(|_| "imece did not answer in time; try again".to_string())?
            }
        })?;
        bus.servers.push(server);
        Ok(())
    }

    /// Moves the clock and applies what the sessions reported.
    pub fn step(&mut self, w: &mut World) {
        w.tick = now().saturating_sub(w.started_at) / SECONDS_PER_TICK;
        let t = w.tick;
        for a in &mut w.agents {
            if a.talk.as_ref().is_some_and(|k| k.until <= t) {
                a.talk = None;
            }
        }
        while let Ok((id, generation, ready)) = self.ready_rx.try_recv() {
            if generation == self.generation[id] {
                self.ready(w, id, ready);
            }
        }
        self.take_git(w);
        self.stand_busy(w, Instant::now());
        while let Ok((id, generation, e)) = self.rx.try_recv() {
            if generation != self.generation[id] {
                continue;
            }
            let e = match e {
                AgentEvent::Asks { request, what } => {
                    self.ask(w, id, what, Reply::Session(request));
                    continue;
                }
                AgentEvent::Withdrawn { request } => {
                    self.withdrawn(w, id, &request);
                    continue;
                }
                e => e,
            };
            let turn_done = matches!(e, AgentEvent::TurnDone { .. });
            if turn_done {
                // A tier model that is not available here fails the first
                // turn before anything is spent: the standard model gets
                // the same prompts.
                let failed = matches!(
                    e,
                    AgentEvent::TurnDone {
                        ok: false,
                        cost: 0,
                        ..
                    }
                );
                if failed && !self.turned[id] && self.tiered[id] {
                    self.fall_back(w, id);
                    continue;
                }
                if let AgentEvent::TurnDone {
                    ok: false,
                    text,
                    cost,
                    ..
                } = &e
                    && text == CAPPED
                {
                    // The cap was what was left of the budget when the
                    // session started. Its last turn still counts.
                    w.agents[id].cost = self.spent_before[id] + cost / 100;
                    if !self.over_budget(w) && !self.renewed[id] {
                        self.renew(w, id);
                        continue;
                    }
                }
                if let AgentEvent::TurnDone { ok: true, .. } = e {
                    self.renewed[id] = false;
                }
                self.turned[id] = true;
                if !self.inflight[id].is_empty() {
                    self.inflight[id].remove(0);
                }
            }
            let answer = match &e {
                AgentEvent::TurnDone { ok: true, text, .. } => Some(text.clone()),
                _ => None,
            };
            match &e {
                // The session the vendor runs is the one to go on with
                // next time; a wake that asked for the old one has it.
                AgentEvent::Started { session, .. } => {
                    self.sessions[id] = Some(Resumable {
                        session: session.clone(),
                        spent: self.floor[id],
                    });
                    if self.resuming[id].take().is_some() {
                        self.interrupted[id].clear();
                        w.note(Event::Resumed { agent: id });
                    }
                }
                AgentEvent::TurnDone { cost, .. } => {
                    if let Some(s) = &mut self.sessions[id] {
                        s.spent = self.floor[id] + cost;
                    }
                }
                AgentEvent::Stopped(why) => {
                    if let Some(a) = self.resuming[id].take() {
                        self.restart(w, id, a, why);
                        continue;
                    }
                }
                _ => {}
            }
            let stopped = matches!(e, AgentEvent::Stopped(_));
            if stopped {
                self.end(w, id);
            }
            apply(w, id, e, self.spent_before[id]);
            if stopped {
                // Helpers work for their worker alone: none without it.
                let helpers: Vec<AgentId> = w.helpers_of(id).map(|h| h.id).collect();
                for h in helpers {
                    self.release(w, h);
                }
            }
            if turn_done {
                self.stand(w, id);
                self.turns[id] = self.turns[id].saturating_sub(1);
                if self.turns[id] == 0 && self.leaving[id] {
                    self.release(w, id);
                    continue;
                }
                if self.turns[id] > 0 {
                    // The session goes on with a prompt it already has.
                    w.agents[id].set_status(Status::Working, w.tick);
                } else if let Some(text) = answer.filter(|_| self.asked[id] || self.reports(w, id))
                {
                    // The person waits for this: it goes to them and into
                    // the journal. The closing text of a turn that mail or
                    // a board offer started restates what the agent sent
                    // (which the journal has) and stays in its story. A
                    // command may have nothing to say (`/clear`).
                    self.asked[id] = false;
                    if text.is_empty() {
                        w.agents[id].trace(w.tick, "nothing to say");
                    } else {
                        w.note(Event::Replied {
                            agent: id,
                            text: text.clone(),
                        });
                        w.replied(id, &text);
                    }
                }
            }
        }
        while let Ok((from, tool, args, reply)) = self.calls_rx.try_recv() {
            if tool == "run" {
                self.run(w, from, &args, reply);
                continue;
            }
            if tool == APPROVE_TOOL {
                self.approve(w, from, &args, reply);
                continue;
            }
            if tool == "send" {
                self.send(w, from, &args, reply);
                continue;
            }
            let answer = match tool.as_str() {
                "post" => self.post(w, from, &args),
                "board" => Ok(self.board(w, from)),
                "claim" => self.claim(w, from, &args),
                "hire" => self.hire(w, from, &args),
                _ => Err(format!("there is no tool {tool:?}")),
            };
            let _ = reply.send(answer);
        }
        for id in 0..self.mail.len() {
            let waking = self.mail[id].iter().any(|m| m.kind.wakes());
            let a = &w.agents[id];
            if waking
                && self.turns[id] == 0
                && !matches!(a.status, Status::Stopped | Status::Asleep)
                && !a.gone
            {
                self.deliver(w, id);
            }
        }
        self.offer(w);
    }

    /// Whether a leader's finished turn is its report to the person: the
    /// team's work it led has come back and nobody works or has mail.
    fn reports(&self, w: &World, id: AgentId) -> bool {
        let a = &w.agents[id];
        a.role == Role::Leader
            && w.agents.iter().all(|o| {
                o.project != a.project || (self.turns[o.id] == 0 && self.mail[o.id].is_empty())
            })
    }

    /// Whether the team as a whole has spent the budget.
    fn over_budget(&self, w: &World) -> bool {
        w.spent() >= w.budget
    }
}

impl Drop for Live {
    fn drop(&mut self) {
        for running in &self.running {
            running.kill_all();
        }
        // Ask every session to stop first, so they wind down together.
        for s in &mut self.slots {
            if let Slot::Running(s) = s {
                s.terminate();
            }
        }
    }
}

/// The name an agent writes to on the bus to reach the person.
pub(crate) const YOU: &str = "you";

/// A repository's notes for coding agents, as they go into an agent's
/// instructions.
#[derive(Debug, PartialEq)]
struct Notes {
    /// The text to append to the instructions.
    text: String,
    /// The file the notes came from (`AGENTS.md` or `CLAUDE.md`).
    file: &'static str,
    /// The file's size in bytes, trimmed.
    size: usize,
    /// Whether the file was longer than [`MAX_NOTES`] and only its head
    /// went in; the journal says so, since the agent reads a file the
    /// person believes it has whole.
    cut: bool,
}

/// FNV-1a: tells checkouts with the same name apart. Not for security.
fn fnv(bytes: &[u8]) -> u32 {
    bytes.iter().fold(0x811c_9dc5, |h, &b| {
        (h ^ u32::from(b)).wrapping_mul(0x0100_0193)
    })
}

/// The longest prefix of `s` within `max` characters, at a character
/// boundary, with a mark when something was cut.
fn cut(s: &str, max: usize) -> &str {
    match s.char_indices().nth(max) {
        Some((i, _)) => &s[..i],
        None => s,
    }
}

/// The local time of day at `at` (seconds since the Unix epoch) and its
/// local day counted from 1970-01-01: what the screen's clock starts at
/// (`start_clock_at`) for a world begun then. Without a local time (no
/// zone the C library knows) the clock is UTC.
pub fn local_clock(at: u64) -> (u64, u64) {
    let secs = at as libc::time_t;
    // SAFETY: `tm` is plain data that localtime_r fills; it keeps no pointer.
    let mut tm: libc::tm = unsafe { std::mem::zeroed() };
    if unsafe { libc::localtime_r(&secs, &mut tm) }.is_null() {
        return utc_clock(at);
    }
    let seconds = (tm.tm_hour as u64) * 3600 + (tm.tm_min as u64) * 60 + tm.tm_sec as u64;
    let day = days_from_civil(
        i64::from(tm.tm_year) + 1900,
        (tm.tm_mon + 1) as u32,
        tm.tm_mday as u32,
    );
    (seconds, day.max(0) as u64)
}

/// `at` as a time of day and a day in UTC: the clock without a zone.
fn utc_clock(at: u64) -> (u64, u64) {
    (at % 86_400, at / 86_400)
}

/// The local day today, as [`local_clock`] counts it.
pub fn today() -> u64 {
    local_clock(now()).1
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::live::testing::*;

    #[test]
    fn the_known_vendors_are_the_ones_the_leader_may_be() {
        for (vendor, program, _) in KNOWN {
            assert_eq!(Vendor::parse(program), Some(*vendor));
            assert_eq!(vendor.label(), *program);
        }
        assert_eq!(Vendor::parse("qwen"), None);
        let drives = |vendor| KNOWN.iter().any(|(v, _, d)| *v == vendor && *d);
        assert!(drives(Vendor::Claude) && drives(Vendor::Codex));
        assert!(drives(Vendor::Gemini));
    }

    #[test]
    fn what_an_agent_can_do_depends_on_its_vendor_role_and_confine_level() {
        use Confine::*;
        let strict = |v, r| abilities(v, r, Strict);
        assert!(
            strict(Vendor::Claude, Role::Leader)
                .contains("not change them; the run tool runs checks")
        );
        assert!(
            strict(Vendor::Claude, Role::Worker).contains("the run tool runs the build, the tests")
        );
        assert!(
            strict(Vendor::Claude, Role::Helper).contains("the run tool runs the build, the tests")
        );
        assert!(
            strict(Vendor::Gemini, Role::Worker).contains("the run tool runs the build, the tests")
        );
        assert!(strict(Vendor::Codex, Role::Leader).contains("refuses writes and the network"));
        assert!(!strict(Vendor::Codex, Role::Leader).contains("search the web"));
        assert!(!strict(Vendor::Claude, Role::Worker).contains("the person's yes"));
        assert!(!strict(Vendor::Codex, Role::Worker).contains("the person's yes"));
        assert!(!strict(Vendor::Gemini, Role::Worker).contains("the person's yes"));
        assert!(
            strict(Vendor::Codex, Role::Worker)
                .contains("inside this directory and refuses the network")
        );
        assert!(
            strict(Vendor::Codex, Role::Helper)
                .contains("inside this directory and refuses the network")
        );
        // Standard and open say the same: a shell and the web for workers
        // and helpers, the web for a leader; Codex's sandbox stays.
        for level in [Standard, Open] {
            let full = |v, r| abilities(v, r, level);
            assert!(
                full(Vendor::Claude, Role::Leader)
                    .contains("read files and the web but not change files")
            );
            assert!(
                full(Vendor::Gemini, Role::Leader)
                    .contains("read files and the web but not change files")
            );
            assert!(
                full(Vendor::Claude, Role::Worker)
                    .contains("run commands in your shell and reach the web")
            );
            assert!(
                full(Vendor::Gemini, Role::Helper)
                    .contains("run commands in your shell and reach the web")
            );
            assert!(!full(Vendor::Claude, Role::Worker).contains("nothing else"));
            // Claude's shell asks the person about what is not routine.
            for r in [Role::Worker, Role::Helper] {
                assert!(
                    full(Vendor::Claude, r).contains("most others wait for the person's yes"),
                    "{r:?}"
                );
            }
            // So does what needs more than Codex's sandbox.
            for r in [Role::Worker, Role::Helper] {
                assert!(
                    full(Vendor::Codex, r).contains("sandbox gives waits for the person's yes"),
                    "{r:?}"
                );
            }
            // And Gemini's, run in the agent's directory.
            for r in [Role::Worker, Role::Helper] {
                let g = full(Vendor::Gemini, r);
                assert!(g.contains("most others wait for the person's yes"), "{r:?}");
                assert!(g.contains("your files' directory as its dir_path"), "{r:?}");
            }
            assert!(!full(Vendor::Gemini, Role::Leader).contains("the person's yes"));
            assert!(!full(Vendor::Qwen, Role::Worker).contains("the person's yes"));
            assert!(
                full(Vendor::Qwen, Role::Worker)
                    .contains("run commands in your shell and reach the web")
            );
            assert!(!full(Vendor::Claude, Role::Leader).contains("the person's yes"));
            assert!(!full(Vendor::Codex, Role::Leader).contains("the person's yes"));
            assert!(
                full(Vendor::Codex, Role::Leader)
                    .contains("search the web and run commands that only read")
            );
            assert!(full(Vendor::Codex, Role::Leader).contains("refuses writes and the network"));
            assert!(full(Vendor::Codex, Role::Worker).contains("run commands and reach the web"));
            assert!(
                full(Vendor::Codex, Role::Worker).contains("keeps writes inside this directory.")
            );
            assert!(!full(Vendor::Codex, Role::Helper).contains("refuses the network"));
        }
        let mut w = team(0);
        w.agents[0].vendor = Vendor::Codex;
        assert!(system_prompt(&w, 0, 4, 2, Strict).contains("run commands that only read"));
        assert!(system_prompt(&w, 0, 4, 2, Standard).contains("search the web and run commands"));
    }

    #[test]
    fn the_clock_starts_at_the_local_time_and_day() {
        let out = std::process::Command::new("date")
            .arg("+%H %M %S %Y %m %d")
            .output()
            .unwrap();
        let p: Vec<u64> = String::from_utf8_lossy(&out.stdout)
            .split_whitespace()
            .map(|n| n.parse().unwrap())
            .collect();
        let expect = p[0] * 3600 + p[1] * 60 + p[2];
        let at = now();
        let (got, day) = local_clock(at);
        let apart = got.abs_diff(expect).min(86_400 - got.abs_diff(expect));
        assert!(apart <= 2, "{got} vs {expect}");
        let expect_day = days_from_civil(p[3] as i64, p[4] as u32, p[5] as u32) as u64;
        // The second between `date` and `now()` may be midnight's.
        assert!(
            day == expect_day || (apart > 0 && day.abs_diff(expect_day) == 1),
            "{day} vs {expect_day}"
        );
        assert_eq!(today().abs_diff(day), 0);
        // A known instant: 2026-10-30 12:00 UTC is the 30th in every zone
        // west of +12 and the 31st east of it (where the clock reads before
        // 02:00); the day is the local clock's, not the epoch's.
        let noon = 1_793_361_600;
        let (seconds, day) = local_clock(noon);
        assert_eq!(utc_clock(noon), (12 * 3600, 20_756), "without a zone");
        assert_eq!(utc_clock(noon + 1), (12 * 3600 + 1, 20_756));
        let utc_day = noon / 86_400;
        assert!(
            day == utc_day || (day == utc_day + 1 && seconds < 2 * 3600),
            "{day} at {seconds}"
        );
    }

    #[test]
    fn a_loopback_url_an_agent_writes_stays_on_its_row_until_its_session_ends() {
        let mut w = team(1);
        let mut live = Live::new(2, claude_at("x"), PathBuf::new(), PathBuf::new());
        let local = |w: &World| w.agents[1].local.clone();
        let text = AgentEvent::Text("dev server: http://localhost:5173/".into());
        live.tx.send((1, 0, text)).unwrap();
        live.step(&mut w);
        assert_eq!(local(&w).as_deref(), Some("http://localhost:5173/"));
        let done = AgentEvent::TurnDone {
            ok: true,
            text: "Moved it to http://127.0.0.1:8080.".into(),
            cost: 0,
            denied: 0,
        };
        live.tx.send((1, 0, done)).unwrap();
        live.step(&mut w);
        assert_eq!(local(&w).as_deref(), Some("http://127.0.0.1:8080"));
        live.tx
            .send((1, 0, AgentEvent::Stopped("gone".into())))
            .unwrap();
        live.step(&mut w);
        assert_eq!(local(&w), None, "it went with the session");
    }

    #[test]
    fn a_late_event_of_an_ended_session_is_ignored() {
        let mut w = team(1);
        let mut live = Live::new(2, claude_at("x"), PathBuf::new(), PathBuf::new());
        live.tx
            .send((1, 0, AgentEvent::Stopped("gone".into())))
            .unwrap();
        live.step(&mut w);
        assert_eq!(w.agents[1].status, Status::Stopped);
        let journal = w.journal.len();
        // Generation 0 is over: its late events do not count.
        live.tx
            .send((
                1,
                0,
                AgentEvent::TurnDone {
                    ok: true,
                    text: "late".into(),
                    cost: 9_000_000,
                    denied: 0,
                },
            ))
            .unwrap();
        live.step(&mut w);
        assert_eq!(w.journal.len(), journal);
        assert_eq!(w.agents[1].cost, 0);
    }
}
