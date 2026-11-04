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

use std::collections::{HashMap, HashSet, VecDeque};
use std::io;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};
use std::sync::mpsc::{self, Receiver, RecvTimeoutError, Sender};
use std::thread;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use imece_agent::mcp::{Answer, Server, Tool};
use imece_agent::{
    APPROVE_TOOL, Agent as Session, Ask, Choice, Confine, Event as AgentEvent, Login, Mcp, Tier,
    child, claude, codex, copy, gemini, run, worktree,
};
use serde_json::{Value, json};

use crate::i18n::{Lang, clean};
use crate::model::*;
use crate::store::{self, Found, Kept, Store};
use crate::ui::words;
use crate::welcome::{Answers, Seen};

mod bus;
mod changes;
mod hiring;
mod paid;
mod projects;
mod prompts;
mod remember;
mod review;
mod sessions;
pub(crate) mod setup;
pub(crate) mod standing;
mod suggest;
#[cfg(test)]
pub(crate) mod testing;
mod tidy;
mod tools;

pub(crate) use self::bus::numbers;
pub use self::projects::Open;
pub(crate) use self::prompts::{INSTRUCTIONS, MAX_NOTES_KIB};
pub(crate) use self::review::{REVIEW_DAY_CALLS, REVIEW_DAY_COST, review_cap, reviewer_of};
pub use self::sessions::apply;
pub(crate) use self::suggest::{SUGGEST_CAP, SUGGEST_DAY_CALLS, SUGGEST_DAY_COST};
pub use self::tidy::Refusal as TidyRefusal;
use self::{changes::*, hiring::*, projects::*, prompts::*, suggest::*, tools::*};

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
    /// The person's own instructions for every project, put in every
    /// agent's instructions when the file is there (`keys::config_file`).
    pub instructions: Option<PathBuf>,
    /// How much of its vendor each agent's session gets (`--confine`).
    pub confine: Confine,
    /// `--solo`: start with a solo session on the checkout, on this vendor
    /// and `[MODEL][@EFFORT]` choice (none: the vendor's standard rung);
    /// an open one of the same vendor and choice is gone on with.
    pub solo: Option<(Vendor, Option<String>)>,
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

/// The efforts vendor `v`'s CLI can be asked for; none for one with no
/// such setting.
pub fn efforts(v: Vendor) -> &'static [&'static str] {
    match v {
        Vendor::Codex => codex::EFFORTS,
        Vendor::Gemini => gemini::EFFORTS,
        _ => claude::EFFORTS,
    }
}

/// Whether the choice `s` (`[MODEL][@EFFORT]`) asks vendor `v` only for an
/// effort it has; else why not, in words for the person.
pub fn valid_choice(v: Vendor, s: &str) -> Result<(), String> {
    Choice::parse(s).check(v.label(), efforts(v))
}

/// The model aliases vendor `v`'s CLI knows, for a choice screen and the
/// completion; a full model name is always taken too.
pub fn aliases(v: Vendor) -> Vec<&'static str> {
    match v {
        Vendor::Codex => codex::ALIASES.to_vec(),
        Vendor::Gemini => gemini::alias_names().collect(),
        _ => claude::ALIASES.to_vec(),
    }
}

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
    /// A git checkout with a first commit, which worktrees are made of;
    /// a folder without (a solo's only) has its agents work straight in
    /// it, and nothing of git runs on it.
    git: bool,
}

/// The folder a project at `path` lives in, and whether it is a git
/// checkout with a first commit: git's top directory then (also for a
/// checkout with nothing committed yet, which is no such checkout), else
/// `path` itself, when it is a directory.
pub(crate) fn checkout_of(path: &Path) -> Result<(PathBuf, bool), String> {
    match worktree::top(path) {
        Ok(top) => {
            let git = worktree::born(&top);
            Ok((top, git))
        }
        Err(_) if path.is_dir() => {
            let folder = path.canonicalize().unwrap_or_else(|_| path.to_path_buf());
            Ok((folder, false))
        }
        Err(e) => Err(format!("{}: {e}", path.display())),
    }
}

/// Whether the folder `repo` a project was kept on is a git checkout
/// with a first commit of its own (not a folder inside another
/// checkout): found again at each start and at a `:restore`, since a
/// folder may have got its first commit since, or lost its git.
pub(super) fn own_git(repo: &Path) -> bool {
    let own = repo.canonicalize().unwrap_or_else(|_| repo.to_path_buf());
    checkout_of(repo).is_ok_and(|(top, born)| born && top == own)
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
/// why it could not start. Then, for a woken worker's first prompt: the
/// line saying what its worktree holds and where it goes in the prompt,
/// read on the session's thread (see [`Live::brief`]). Last, what became
/// of the paths the person listed to copy into the worktree, and of those
/// the checkout's `.worktreeinclude` names.
type Ready = Result<
    (
        Box<dyn Session>,
        PathBuf,
        Option<worktree::Placed>,
        Option<(usize, String)>,
        copy::Brought,
        setup::Done,
    ),
    String,
>;

/// A tool call of an agent (tool name, arguments), where its answer goes,
/// and who has it: see [`answer_of`].
type Call = (AgentId, String, Value, Sender<Answer>, Arc<AtomicBool>);

/// The answer to a call on the bus, waited for `wait`. Either imece takes
/// the call or the caller gives up on it, never both: whichever turns
/// `taken` first. A caller that gave up hears "try again" and imece drops
/// the call when it gets to it (it was busy: a `!cmd` holds the screen),
/// so a call tried again is not done twice (two workers hired, a message
/// sent twice). A call imece took is waited for once more: it was done,
/// What a socket's call from outside its agent's session is passed on
/// as, to be said in that agent's trace; no tool has the name.
const NOT_ITS_SESSION: &str = "(outside its session)";

/// What the caller from outside hears.
const NOT_ITS_SESSION_SAID: &str = "this socket takes calls from its own agent's session only";

/// Whether the process `peer` that called on a socket is the session's
/// process `owner` or one it started. None running (0), or a caller the
/// system will not name, is a no.
fn its_session(peer: Option<u32>, owner: u32) -> bool {
    owner != 0 && peer.is_some_and(|p| imece_agent::child::descends(p, owner))
}

/// and the caller must not do it again.
fn answer_of(answer: &Receiver<Answer>, taken: &AtomicBool, wait: Duration) -> Answer {
    let late = match answer.recv_timeout(wait) {
        Ok(a) => return a,
        Err(RecvTimeoutError::Disconnected) => return Err("imece is closing".into()),
        Err(RecvTimeoutError::Timeout) => wait,
    };
    if !taken.swap(true, Ordering::SeqCst) {
        return Err("imece did not answer in time; try again".into());
    }
    match answer.recv_timeout(late) {
        Ok(a) => a,
        Err(RecvTimeoutError::Disconnected) => Err("imece is closing".into()),
        Err(RecvTimeoutError::Timeout) => Err("imece took this call but its answer is late: \
                                               it was done, do not make it again"
            .into()),
    }
}

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

/// What a failed turn tried again says: the failed prompt is still in
/// the session (every vendor keeps it), so it is not sent again.
pub(super) const RETRY_PROMPT: &str = "From imece (not from the person or a teammate): your \
     last turn stopped on an error at your vendor's API (overloaded, a server error or a lost \
     connection). Carry on from where you stopped.";

/// What wakes a worker whose helper stopped: the note before it says
/// what happened.
pub(super) const GO_ON: &str = "Go on with your task.";

/// The most of a helper's last words its worker is told.
pub(super) const LOST_SAID: usize = 200;

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

/// The most bytes copied into one worktree at a session's start, all the
/// listed paths together (`:copy`): a clone costs little on APFS, a
/// whole copy does elsewhere.
pub const COPY_BUDGET: u64 = 2 << 30;

/// The most output of one run that goes back to the agent, in bytes.
const MAX_RUN_OUTPUT: usize = 48 << 10;
/// How often a worker's row learns where its worktree stands while it or
/// a helper of its works; between turns it learns at each turn's end.
const STAND_EVERY: Duration = Duration::from_secs(15);

/// How often a project's file list (for completion in a message) is read
/// again while one of its agents is mid-turn; between turns it is read
/// when a change is applied.
const FILES_EVERY: Duration = Duration::from_secs(60);

/// Runs remembered with the tree they ran on, for the changes read after
/// them (see [`Change::runs`]); the oldest go first.
const RAN_KEEP: usize = 64;

/// imece's MCP servers, one socket per agent in a private directory.
struct Bus {
    dir: PathBuf,
    /// imece's own binary; sessions start it as `mcp-relay <socket>`.
    relay: String,
    servers: Vec<Server>,
    /// Each socket's session process now (0: none running), by agent:
    /// a tool call is taken only from it or a process it started
    /// (`imece_agent::child::descends`), never from another agent's.
    owners: Vec<Arc<AtomicU32>>,
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
        /// The hunks the person chose, by their `@@` lines; none: all.
        hunks: Vec<usize>,
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
    /// Read what is staged in project `project`'s checkout `repo`, for a
    /// commit message (see [`worktree::staged_change`]).
    Staged { project: ProjectId, repo: PathBuf },
    /// List the files of project `project`'s checkout `repo`, for
    /// completion in a message (see [`worktree::files`]).
    Files { project: ProjectId, repo: PathBuf },
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
        /// How many hunks were applied, when only some were.
        part: Option<usize>,
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
    Staged {
        project: ProjectId,
        staged: io::Result<Option<worktree::Staged>>,
    },
    /// The files of project `project`'s checkout, or why not.
    Files {
        project: ProjectId,
        files: io::Result<(Vec<String>, bool)>,
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
    /// A fork's new session (record 94): the id Claude Code is told to
    /// give it (Codex gives its own); none for a resume.
    fork: Option<String>,
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
    /// What the vendor said when it would not go on with an agent's old
    /// session, for the journal's line on the new one.
    refused: Vec<Option<String>>,
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
    /// The worker each fork goes on from, until its own session starts
    /// (`:fork`, record 94).
    forks: Vec<Option<AgentId>>,
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
    /// Whether each agent has a note that a helper of its stopped before
    /// it sent done, not read yet: an idle one is woken for it.
    lost: Vec<bool>,
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
    /// When each project's file list was last asked for; a project not
    /// here has never been read (or was closed) and is read at once.
    filed_at: HashMap<ProjectId, Instant>,
    /// The projects whose file list git is reading now.
    filing: HashSet<ProjectId>,
    /// The projects told once that their checkout has more files than
    /// completion keeps.
    files_cut: HashSet<ProjectId>,
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
    /// Worktree setup scripts asked about (record 92), by their inbox
    /// item: the bytes asked about and the agents whose new worktrees
    /// went without them while the ask waited.
    setup_asks: Vec<setup::Ask>,
    /// The setup scripts the person said no to this run, by key.
    setup_refused: HashSet<String>,
    /// Setup runs after a yes, each on a thread of its own, ending here.
    setup_tx: Sender<(AgentId, Result<run::Outcome, String>)>,
    setup_rx: Receiver<(AgentId, Result<run::Outcome, String>)>,
    /// Programs workers and helpers may run, next to cargo and git.
    run_programs: Vec<String>,
    run_timeout: Duration,
    /// The person's own instructions file; see [`Options::instructions`].
    instructions: Option<PathBuf>,
    /// How much of its vendor each session gets; see [`Confine`].
    confine: Confine,
    /// What each agent's session waits on the person's yes for, in the
    /// order its approval items are in the inbox.
    pending: Vec<Vec<Pending>>,
    /// The project a commit message is being suggested for: one at a
    /// time, from the person's ask until it answered or was refused.
    suggesting: Option<ProjectId>,
    /// The last suggestion of each project, with the staged tree it was
    /// for: the same tree asked again gets it again without a call.
    suggestions: HashMap<ProjectId, (String, String)>,
    /// The day the ceilings count (see [`suggest::SUGGEST_DAY_CALLS`]),
    /// with its suggestions and their cost as the store has them.
    paid_day: (u64, u32, u64),
    said_tx: Sender<Said>,
    said_rx: Receiver<Said>,
    /// The reviews asked for (`:review`) that wait their turn, each with
    /// the vendor that reviews, and the one under way with the patch it
    /// reads ([`patch_sum`]): one at a time.
    reviews_due: VecDeque<(AgentId, Vendor)>,
    reviewing: Option<(AgentId, Vendor, String)>,
    /// The day the reviews' ceilings count (see
    /// [`review::REVIEW_DAY_CALLS`]), as [`Live::paid_day`] is for
    /// suggestions.
    review_day: paid::Count,
    reviewed_tx: Sender<review::Reviewed>,
    reviewed_rx: Receiver<review::Reviewed>,
    /// Looks for worktrees no agent owns and their removals (`:tidy`).
    tidy: tidy::Tidy,
}

/// The coding agents among `known` (see [`KNOWN`]) on PATH, run from the
/// checkout `repo` is in, with whether imece drives them.
pub fn probe(repo: &Path, known: &[Known]) -> Result<Vec<(Vendor, bool)>, String> {
    let repo = checkout_of(repo)?.0;
    Ok(known
        .iter()
        .filter(|(_, program, _)| on_path(program, &repo))
        .map(|&(vendor, _, drives)| (vendor, drives))
        .collect())
}

/// Asks each of `known`'s CLIs whose vendor is among `which`, each on a
/// thread of its own, for its version and whether it is signed in, outside
/// any checkout and without the network; the answers come one by one.
/// Gemini CLI has no such question: its sign-in is [`Login::Unknown`].
pub fn look(known: &[Known], which: &[Vendor]) -> Answers {
    let (tx, rx) = mpsc::channel();
    for &(vendor, program, _) in known.iter().filter(|k| which.contains(&k.0)) {
        let tx = tx.clone();
        thread::spawn(move || {
            let seen = match imece_agent::version(program, env_of(vendor)) {
                None => Seen::Missing,
                Some(version) => Seen::Here {
                    version,
                    login: match vendor {
                        Vendor::Claude => claude::login(program),
                        Vendor::Codex => codex::login(program),
                        _ => Login::Unknown,
                    },
                },
            };
            let _ = tx.send((vendor, seen));
        });
    }
    rx
}

/// The variables vendor `v`'s CLI gets besides the base ones: its login,
/// config and proxies.
fn env_of(v: Vendor) -> &'static [&'static str] {
    match v {
        Vendor::Codex => codex::ENV,
        Vendor::Gemini => gemini::ENV,
        _ => claude::ENV,
    }
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
    let (repo, git) = checkout_of(&o.repo)?;
    if !git && o.solo.is_none() {
        return Err(no_git_team(&repo.display().to_string()));
    }
    let vendors = team_vendors(hireable(present, o.lead)?, o.lead, &o.tiers);
    check_ladders(&vendors)?;
    let file = o.state.join(store::FILE);
    let mut store = Store::open(&file)?;
    let loaded = store
        .load(JOURNAL_KEEP)
        .map_err(|e| store::unreadable(&file, e))?;
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
                cleared: Default::default(),
                setup_approved: Vec::new(),
                sent: Vec::new(),
                limits: Vec::new(),
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
                paid: 0,
                suggested: None,
                reviews: Default::default(),
                orphans: crate::model::Orphans::default(),
                messages: 0,
            },
            None,
        ),
    };
    if let Some(b) = o.budget {
        w.budget = b;
    }
    w.tick = now.saturating_sub(w.started_at) / SECONDS_PER_TICK;
    // imece's own calls are in the pot as the agents' turns are.
    w.paid = store
        .paid_total()
        .map_err(|e| store::unreadable(&file, e))?;
    w.run_spent = w.spent();
    let mut live = Live::bare(vendors, o.lead);
    live.state = o.state;
    live.max_workers = o.max_workers;
    live.max_helpers = o.max_helpers;
    live.run_programs = o.run_programs;
    live.run_timeout = o.run_timeout;
    live.instructions = o.instructions;
    live.confine = o.confine;
    live.fresh = o.fresh;
    live.left_off = left_off;
    if let Some((found, skipped)) = found {
        live.adopt(&mut w, found, skipped);
    }
    // The project of the checkout at hand: with `--solo`, the solo on the
    // asked vendor and choice, opened unless one is open already; else the
    // team, opened unless one is open already (a solo alone here does not
    // stand in for it).
    let at = |q: &Project, live: &Live| live.places.get(q.id).is_some_and(|pl| pl.repo == repo);
    live.home = match o.solo {
        Some((vendor, choice)) => {
            let Some(h) = live.vendors.iter().find(|h| h.vendor == vendor) else {
                return Err(format!(
                    "--solo: {} is not on PATH here, or imece does not drive it",
                    vendor.label()
                ));
            };
            let model = choice.clone().or_else(|| h.ladder.standard.clone());
            match live.solo_at(&w, &repo, vendor, &model) {
                Some(p) => p,
                None => live
                    .open_project(
                        &mut w,
                        &repo,
                        None,
                        Open::Solo {
                            vendor: Some(vendor),
                            choice,
                        },
                    )
                    .map_err(|e| format!("--solo: {}", e.describe()))?,
            }
        }
        None => {
            let team = (w.open_projects())
                .find(|q| at(q, &live) && !w.is_solo(q.id))
                .map(|q| q.id);
            match team {
                Some(p) => p,
                None => live
                    .open_project(&mut w, &repo, None, Open::Team)
                    .map_err(|e| e.describe())?,
            }
        }
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

    /// The `[MODEL][@EFFORT]` choice agent `id` runs on, none for its
    /// vendor's own default; what a solo's row shows.
    pub fn model_of(&self, id: AgentId) -> Option<&str> {
        self.models[id].as_deref()
    }

    /// The vendors imece can run here, the lead vendor first: what a solo
    /// may be opened on.
    pub fn vendors(&self) -> Vec<Vendor> {
        let mut v: Vec<Vendor> = self.vendors.iter().map(|h| h.vendor).collect();
        if let Some(i) = v.iter().position(|&x| x == self.lead) {
            v.swap(0, i);
        }
        v
    }

    /// The person changes what solo `id` runs on (`:model`, `:effort`):
    /// `choice` is the whole `[MODEL][@EFFORT]`, none for the vendor's own.
    /// Its running session ends (the next message starts a new one on the
    /// same transcript, told where it left off); refused mid-turn, so no
    /// answer is cut, and for anyone but a solo. Says what it runs on now.
    pub fn rechoose(
        &mut self,
        w: &mut World,
        id: AgentId,
        choice: Option<String>,
    ) -> Result<String, String> {
        let a = &w.agents[id];
        if a.role != Role::Solo {
            return Err(format!("{} is not a solo session", a.name));
        }
        if let Some(c) = &choice {
            valid_choice(a.vendor, c)?;
        }
        if self.turns[id] > 0 {
            return Err(format!("{} is mid-turn; wait for its answer", a.name));
        }
        self.models[id] = choice.clone();
        self.tiered[id] = false;
        let runs = match &choice {
            Some(c) => format!("{} {c}", a.vendor.label()),
            None => a.vendor.label().to_string(),
        };
        if matches!(self.slots[id], Slot::Running(_)) {
            self.end(w, id);
            w.agents[id].trace(
                w.tick,
                format!("the person chose {runs}; the session starts again on it"),
            );
        } else {
            w.agents[id].trace(w.tick, format!("the person chose {runs}"));
        }
        w.note(Event::Chose {
            agent: id,
            choice: runs.clone(),
        });
        Ok(runs)
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
        let (said_tx, said_rx) = mpsc::channel();
        let (reviewed_tx, reviewed_rx) = mpsc::channel();
        let (setup_tx, setup_rx) = mpsc::channel();
        thread::spawn(move || git_thread(jobs, done));
        Live {
            state: std::env::temp_dir(),
            store: None,
            left_off: None,
            home: 0,
            loaded_cards: 0,
            remembered: Vec::new(),
            refused: Vec::new(),
            interrupted: Vec::new(),
            sessions: Vec::new(),
            floor: Vec::new(),
            resuming: Vec::new(),
            forks: Vec::new(),
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
            lost: Vec::new(),
            notes: Vec::new(),
            changes: HashMap::new(),
            git,
            git_back,
            reading: Vec::new(),
            applying: Vec::new(),
            placing: Vec::new(),
            standing: Vec::new(),
            stood_at: Vec::new(),
            filed_at: HashMap::new(),
            filing: HashSet::new(),
            files_cut: HashSet::new(),
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
            setup_asks: Vec::new(),
            setup_refused: HashSet::new(),
            setup_tx,
            setup_rx,
            run_programs: Vec::new(),
            run_timeout: DEFAULT_RUN_TIMEOUT,
            instructions: None,
            confine: Confine::default(),
            pending: Vec::new(),
            suggesting: None,
            suggestions: HashMap::new(),
            // No day yet: the first ask reads the day's count.
            paid_day: (u64::MAX, 0, 0),
            said_tx,
            said_rx,
            reviews_due: VecDeque::new(),
            reviewing: None,
            review_day: (u64::MAX, 0, 0),
            reviewed_tx,
            reviewed_rx,
            tidy: tidy::Tidy::default(),
        }
    }

    /// The private directory a Gemini session of agent `name` of project
    /// `project` runs in: a sibling of the worktrees, kept between runs
    /// like them. A leader's is `lead` whatever the project is called, so
    /// a rename (`lead-NAME`) does not leave its session behind; a team's
    /// on a checkout that had a team before (one archived, or closed) is
    /// `lead-p<project id>`, so the two never share one.
    fn gemini_home(&self, w: &World, project: ProjectId, role: Role, name: &str) -> PathBuf {
        let root = &self.places[project].root;
        let mut dir = root.clone().into_os_string();
        dir.push("-gemini");
        let dir = PathBuf::from(dir);
        if role != Role::Leader {
            return dir.join(name);
        }
        let before = (0..project)
            .any(|q| self.places.get(q).is_some_and(|pl| pl.root == *root) && !w.is_solo(q));
        if before {
            dir.join(format!("lead-p{project}"))
        } else {
            dir.join("lead")
        }
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
            owners: Vec::new(),
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
        let owner = Arc::new(AtomicU32::new(0));
        let mine = Arc::clone(&owner);
        let server = Server::bind(&bus.dir.join(format!("a{id}.sock")), tools, {
            move |name, args, peer| {
                if !its_session(peer, mine.load(Ordering::SeqCst)) {
                    // Said in the trace of the agent whose socket it is.
                    let (reply, _) = mpsc::channel();
                    let said = json!({ "tool": name });
                    let _ = tx.send((id, NOT_ITS_SESSION.into(), said, reply, Default::default()));
                    return Err(NOT_ITS_SESSION_SAID.into());
                }
                let (reply, answer) = mpsc::channel();
                let taken = Arc::new(AtomicBool::new(false));
                tx.send((id, name.to_string(), args.clone(), reply, taken.clone()))
                    .map_err(|_| "imece is closing".to_string())?;
                answer_of(&answer, &taken, call_wait(name, run_timeout))
            }
        })?;
        bus.servers.push(server);
        bus.owners.push(owner);
        Ok(())
    }

    /// Tells each socket which process its agent's session runs in now:
    /// a session started, ended, or started its vendor again.
    fn own_sockets(&self) {
        let Some(bus) = &self.bus else {
            return;
        };
        for (slot, owner) in self.slots.iter().zip(&bus.owners) {
            let pid = match slot {
                Slot::Running(s) => s.pid().unwrap_or(0),
                _ => 0,
            };
            owner.store(pid, Ordering::SeqCst);
        }
    }

    /// Moves the clock and applies what the sessions reported. Whether
    /// anything came the screen may show: the clock's step, a session
    /// starting or saying something, what git finished, a suggestion, a
    /// call on the bus (the screen is drawn again only then).
    pub fn step(&mut self, w: &mut World) -> bool {
        let tick = now().saturating_sub(w.started_at) / SECONDS_PER_TICK;
        let moved = tick != w.tick;
        w.tick = tick;
        let mut started = false;
        while let Ok((id, generation, ready)) = self.ready_rx.try_recv() {
            started = true;
            if generation == self.generation[id] {
                self.ready(w, id, ready);
            }
        }
        // Before any call is taken: a session that just started answers
        // on its socket from now on.
        self.own_sockets();
        let git = self.take_git(w);
        let said = self.take_said(w);
        let reviewed = self.take_reviews(w);
        self.review_next(w);
        let tidied = self.take_tidy(w);
        let set_up = self.take_setups(w);
        self.stand_busy(w, Instant::now());
        self.files_due(w, Instant::now());
        let mut heard = false;
        while let Ok((id, generation, e)) = self.rx.try_recv() {
            heard = true;
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
                // What the vendor said the turn failed on: back on the
                // agent for its end below, unless the turn runs again.
                let fault = w.agents[id].fault.take();
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
                // A vendor's fault is no missing model: it is tried again.
                if failed && !self.turned[id] && self.tiered[id] && fault.is_none() {
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
                w.agents[id].fault = fault;
                // The person's yes for this turn ends with it.
                w.turn_over(id);
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
                    if let Some(a) = self.resuming[id].take() {
                        self.interrupted[id].clear();
                        // A fork was journaled when it was made; its
                        // session is its own from now on.
                        if a.fork.is_none() {
                            w.note(Event::Resumed { agent: id });
                        }
                        self.forks[id] = None;
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
            // What a helper's end, or the last turn it had failing, takes
            // from its worker.
            let lost = match &e {
                AgentEvent::Stopped(why) => Some(why.clone()),
                AgentEvent::TurnDone {
                    ok: false, text, ..
                } if self.turns[id] == 1 => Some(text.clone()),
                _ => None,
            };
            if stopped {
                self.end(w, id);
            }
            apply(w, id, e, self.spent_before[id]);
            if let Some(why) = lost
                && w.agents[id].retry.is_none()
            {
                self.helper_lost(w, id, &why);
            }
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
                    if w.agents[id].role == Role::Solo && !text.is_empty() {
                        // Nobody sends done for a solo: every turn that
                        // ends with an answer shows the person what its
                        // worktree holds (a command's turn says nothing).
                        let title = text.lines().next().unwrap_or_default().to_string();
                        let (nobody, _) = mpsc::channel();
                        self.snapshot(w, id, title, String::new(), nobody);
                    }
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
        let mut called = false;
        while let Ok((from, tool, args, reply, taken)) = self.calls_rx.try_recv() {
            called = true;
            if tool == NOT_ITS_SESSION {
                let tool = clean(args["tool"].as_str().unwrap_or_default());
                let said =
                    format!("{tool}: called on this socket from outside its session, refused");
                w.agents[from].trace(w.tick, said);
                continue;
            }
            if taken.swap(true, Ordering::SeqCst) {
                // Its caller gave up on it and was told to try again.
                let late = format!("{tool}: given up before imece got to it, not done");
                w.agents[from].trace(w.tick, late);
                continue;
            }
            if w.put_away(from) {
                // Asked before its project was archived or closed: what it
                // asked for would land where nobody looks.
                w.agents[from].trace(w.tick, format!("{tool}: its project is put away, not done"));
                let _ = reply.send(Err("your project was put away; nothing was done".into()));
                continue;
            }
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
                "whoami" => Ok(self.whoami(w, from)),
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
        for id in 0..self.lost.len() {
            let a = &w.agents[id];
            if self.lost[id]
                && self.turns[id] == 0
                && !matches!(a.status, Status::Stopped | Status::Asleep)
                && !a.gone
                && self.woken[id] < MAX_WOKEN
                && !self.over_budget(w)
            {
                self.woken[id] += 1;
                if let Err(why) = self.prompt(w, id, GO_ON.into(), None) {
                    apply(w, id, AgentEvent::Stopped(why), self.spent_before[id]);
                }
            }
        }
        for id in w.retries_due() {
            if self.turns[id] == 0 && !self.over_budget(w) {
                self.retry(w, id);
            }
        }
        self.offer(w);
        moved || started || git || said || reviewed || tidied || set_up || heard || called
    }

    /// Tries agent `id`'s failed turn again: its session is told to go on.
    fn retry(&mut self, w: &mut World, id: AgentId) {
        let Some(r) = w.agents[id].retry else {
            return;
        };
        let t = w.tick;
        w.agents[id].trace(
            t,
            format!(
                "retry {}/{} after a vendor error",
                r.attempt,
                RETRY_TICKS.len()
            ),
        );
        if let Err(why) = self.prompt(w, id, RETRY_PROMPT.into(), None) {
            apply(w, id, AgentEvent::Stopped(why), self.spent_before[id]);
        }
    }

    /// Whether a leader's finished turn is its report to the person: the
    /// team's work it led has come back and nobody works or has mail. A
    /// solo's turn is its report the same way, once its helpers are quiet.
    fn reports(&self, w: &World, id: AgentId) -> bool {
        let a = &w.agents[id];
        matches!(a.role, Role::Leader | Role::Solo)
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
        self.tidy.running.kill_all();
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
    // `time_t` by inference: naming it is deprecated on musl, whose
    // `time_t` is changing to 64 bits.
    let secs = at as _;
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

    /// What a step says when `put` has put something in, on a step that
    /// left the clock where it was (the clock moving says so by itself).
    fn says(live: &mut Live, w: &mut World, put: impl Fn(&mut Live)) -> bool {
        for _ in 0..3 {
            live.step(w);
            let before = w.tick;
            put(live);
            let came = live.step(w);
            if w.tick == before {
                return came;
            }
        }
        panic!("the clock moved on every try");
    }

    #[test]
    fn a_call_is_taken_by_imece_or_given_up_by_its_caller_never_both() {
        let ms = Duration::from_millis;
        // Answered in time.
        let (reply, answer) = mpsc::channel();
        let taken = AtomicBool::new(false);
        reply.send(Ok("joined".into())).unwrap();
        assert_eq!(answer_of(&answer, &taken, ms(10)), Ok("joined".into()));
        assert!(!taken.load(Ordering::SeqCst));
        // Not taken in time: given up, and imece will drop it.
        let got = answer_of(&answer, &taken, ms(10)).unwrap_err();
        assert!(got.contains("try again"), "{got}");
        assert!(taken.load(Ordering::SeqCst), "given up");
        // Taken by imece, answered late: the answer is waited for.
        let (reply, answer) = mpsc::channel();
        let taken = AtomicBool::new(true);
        let late = thread::spawn(move || {
            thread::sleep(ms(750));
            reply.send(Ok("joined".into())).unwrap();
            reply
        });
        assert_eq!(answer_of(&answer, &taken, ms(500)), Ok("joined".into()));
        // Taken and never answered: done, not to be made again.
        let _reply = late.join().unwrap();
        let got = answer_of(&answer, &taken, ms(10)).unwrap_err();
        assert!(got.contains("do not make it again"), "{got}");
        // imece going away.
        let (reply, answer) = mpsc::channel::<Answer>();
        drop(reply);
        let got = answer_of(&answer, &AtomicBool::new(false), ms(10));
        assert_eq!(got, Err("imece is closing".into()));
    }

    #[test]
    fn a_call_its_caller_gave_up_on_is_not_done_when_imece_gets_to_it() {
        let mut w = team(1);
        let mut live = idle_live(2);
        let hire = |live: &mut Live, gave_up: bool| {
            let (reply, answer) = mpsc::channel();
            let taken = Arc::new(AtomicBool::new(gave_up));
            let call = (0, "hire".into(), json!({}), reply, taken.clone());
            live.calls_tx.send(call).unwrap();
            (answer, taken)
        };
        let (answer, _) = hire(&mut live, true);
        live.step(&mut w);
        assert_eq!(w.agents.len(), 2, "nobody hired");
        assert!(answer.try_recv().is_err(), "nobody to answer");
        let said = w.agents[0].trace.back().unwrap();
        assert!(
            said.ends_with(" hire: given up before imece got to it, not done"),
            "{said}"
        );
        // Tried again: done once, and taken, so its caller waits for it.
        let (answer, taken) = hire(&mut live, false);
        live.step(&mut w);
        assert_eq!(w.agents.len(), 3);
        assert!(
            answer
                .try_recv()
                .unwrap()
                .unwrap()
                .starts_with("claude-2 joined")
        );
        assert!(taken.load(Ordering::SeqCst));
    }

    #[test]
    fn a_step_says_whether_anything_came_the_screen_may_show() {
        let mut w = team(1);
        let mut live = idle_live(2);
        assert!(!says(&mut live, &mut w, |_| {}), "nothing came");
        // The clock alone.
        w.tick = 1_000;
        assert!(live.step(&mut w), "the clock moved");
        // A session's report, even of an old generation.
        let (ready, ready_rx) = mpsc::channel();
        live.ready_rx = ready_rx;
        let came = says(&mut live, &mut w, |_| {
            ready.send((1, 99, Err("old".into()))).unwrap();
        });
        assert!(came, "a start");
        // What git finished.
        let (git, git_back) = mpsc::channel();
        live.git_back = git_back;
        let came = says(&mut live, &mut w, |_| {
            let files = Ok((vec!["a.rs".into()], false));
            git.send(GitDone::Files { project: 0, files }).unwrap();
        });
        assert!(came, "git");
        assert_eq!(w.projects[0].files, ["a.rs"]);
        // A suggestion's answer.
        let came = says(&mut live, &mut w, |live| {
            let unsaid = imece_agent::Unsaid {
                why: "no".into(),
                cost: None,
            };
            (live.said_tx.send(Said {
                project: 0,
                day: 5,
                tree: "t".into(),
                model: "haiku".into(),
                said: Err(unsaid),
            }))
            .unwrap();
        });
        assert!(came, "a suggestion");
        // A session's word.
        let came = says(&mut live, &mut w, |live| {
            let text = AgentEvent::Text("hello".into());
            live.tx.send((1, 0, text)).unwrap();
        });
        assert!(came, "a session's word");
        // A call on the bus.
        let (reply, answer) = mpsc::channel();
        let came = says(&mut live, &mut w, |live| {
            let call = (
                1,
                "board".into(),
                json!({}),
                reply.clone(),
                Default::default(),
            );
            live.calls_tx.send(call).unwrap();
        });
        assert!(came, "a call");
        assert!(answer.try_recv().is_ok(), "answered");
        assert!(!says(&mut live, &mut w, |_| {}), "and quiet again");
    }

    #[test]
    fn a_look_asks_the_vendors_named_for_their_version_and_sign_in() {
        let dir = std::env::temp_dir().join(format!("imece-look-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let fake = |name: &str, script: &str| -> &'static str {
            let path = dir.join(name);
            std::fs::write(&path, format!("#!/bin/sh\n{script}\n")).unwrap();
            use std::os::unix::fs::PermissionsExt;
            std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o755)).unwrap();
            Box::leak(path.display().to_string().into_boxed_str())
        };
        let claude = fake(
            "claude",
            r#"case "$*" in
--version) echo "2.1.3 (Claude Code)";;
"auth status --json") echo '{"loggedIn": true}';;
*) exit 9;;
esac"#,
        );
        let codex = fake(
            "codex",
            r#"case "$*" in
--version) echo "codex-cli 0.9.1";;
"login status") echo "Not logged in" >&2; exit 1;;
*) exit 9;;
esac"#,
        );
        let gemini = fake("gemini", r#"[ "$1" = --version ] && echo 0.65.0"#);
        let known = [
            (Vendor::Claude, claude, true),
            (Vendor::Codex, codex, true),
            (Vendor::Gemini, gemini, true),
        ];
        let answers = |which: &[Vendor]| {
            let rx = look(&known, which);
            let mut got: Vec<_> = rx.iter().collect();
            got.sort_by_key(|(v, _)| v.label());
            got
        };
        let here = |version: &str, login| Seen::Here {
            version: version.into(),
            login,
        };
        assert_eq!(
            answers(&[Vendor::Claude, Vendor::Codex, Vendor::Gemini]),
            [
                (Vendor::Claude, here("2.1.3 (Claude Code)", Login::Yes)),
                (Vendor::Codex, here("codex-cli 0.9.1", Login::No)),
                (Vendor::Gemini, here("0.65.0", Login::Unknown)),
            ]
        );
        // Only the ones named are asked.
        assert_eq!(
            answers(&[Vendor::Codex]),
            [(Vendor::Codex, here("codex-cli 0.9.1", Login::No))]
        );
        // Each CLI gets its own vendor's variables (its login lives there).
        assert!(env_of(Vendor::Claude).contains(&"ANTHROPIC_API_KEY"));
        assert!(env_of(Vendor::Codex).contains(&"CODEX_HOME"));
        assert!(env_of(Vendor::Gemini).contains(&"GEMINI_API_KEY"));
        let gone: &'static str = Box::leak(dir.join("gone").display().to_string().into_boxed_str());
        let rx = look(&[(Vendor::Claude, gone, true)], &[Vendor::Claude]);
        assert_eq!(
            rx.iter().collect::<Vec<_>>(),
            [(Vendor::Claude, Seen::Missing)]
        );
        std::fs::remove_dir_all(&dir).unwrap();
    }

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
