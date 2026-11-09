//! What the screen shows: projects, agents, cards, the journal, the inbox and
//! the merge queue. Fake events (`sim`) or live agents (`live`) fill it.
//!
//! Events are stored as data, not sentences: the screen words them in the
//! person's language when it draws them.

use std::collections::{BTreeSet, HashMap, VecDeque};
use std::path::Path;
use std::sync::atomic::{AtomicU64, Ordering};

pub use imece_agent::worktree::{Clash, Clashes};
use serde::{Deserialize, Serialize};

pub use crate::paths::Owned;

pub type AgentId = usize;
pub type CardId = usize;
pub type ProjectId = usize;

/// Seconds per tick.
pub const SECONDS_PER_TICK: u64 = 5;
/// How many of a checkout's files are kept for completion.
pub const FILES_MAX: usize = 20_000;
/// How far back the spending rate looks, in ticks: long enough for a few
/// turns, short enough that an idle hour does not hide a busy team.
pub const RATE_WINDOW: u64 = 10 * 60 / SECONDS_PER_TICK;
/// Minutes left in the pot, at the rate, below which the header warns and
/// the journal says so once.
pub const POT_LOW_MINUTES: u64 = 30;
/// The clock at tick 0: seconds after midnight (14:00 for the simulation,
/// the local time of the world's first start for live agents) and the day
/// (local, counted from 1970-01-01), so a tick has a date as well as a
/// time. Both follow the start, not the terminal's clock: a tick is five
/// seconds whatever the day, so a change of daylight-saving time inside a
/// world's life shifts its clock by an hour.
static START_SECONDS: AtomicU64 = AtomicU64::new(14 * 3600);
static START_DAY: AtomicU64 = AtomicU64::new(0);

pub fn start_clock_at(seconds_after_midnight: u64, day: u64) {
    START_SECONDS.store(seconds_after_midnight % 86_400, Ordering::Relaxed);
    START_DAY.store(day, Ordering::Relaxed);
}

fn start_seconds() -> u64 {
    START_SECONDS.load(Ordering::Relaxed)
}

/// The day of a tick, counted from 1970-01-01 in the clock's time zone.
pub fn day(tick: u64) -> u64 {
    START_DAY.load(Ordering::Relaxed) + (start_seconds() + tick * SECONDS_PER_TICK) / 86_400
}

/// A day (as [`day`] counts them) as `YYYY-MM-DD`.
pub fn date(day: u64) -> String {
    let (y, m, d) = civil_from_days(day as i64);
    format!("{y:04}-{m:02}-{d:02}")
}

/// Days since 1970-01-01 of a date of the (proleptic) Gregorian calendar;
/// `m` and `d` count from 1. Howard Hinnant's `days_from_civil`.
pub fn days_from_civil(y: i64, m: u32, d: u32) -> i64 {
    let y = if m <= 2 { y - 1 } else { y };
    let era = if y >= 0 { y } else { y - 399 } / 400;
    let yoe = y - era * 400;
    let mp = i64::from(if m > 2 { m - 3 } else { m + 9 });
    let doy = (153 * mp + 2) / 5 + i64::from(d) - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    era * 146_097 + doe - 719_468
}

/// The date of a day counted from 1970-01-01; the inverse of
/// [`days_from_civil`] (Hinnant's `civil_from_days`).
fn civil_from_days(z: i64) -> (i64, u32, u32) {
    let z = z + 719_468;
    let era = if z >= 0 { z } else { z - 146_096 } / 146_097;
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = (doy - (153 * mp + 2) / 5 + 1) as u32;
    let m = if mp < 10 { mp + 3 } else { mp - 9 } as u32;
    (yoe + era * 400 + i64::from(m <= 2), m, d)
}
/// Trace lines kept per agent.
const TRACE_KEEP: usize = 200;

/// The word for Claude Code on the screen and in worker names (`claude-2`).
pub const CLAUDE: &str = "claude";

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Role {
    Leader,
    /// A session the person drives alone, in a project of its own with
    /// no leader: it writes in its own worktree as a worker does, and
    /// every turn end shows the person what changed. It may hire helpers.
    Solo,
    Worker,
    /// A worker's own helper: hired by the worker for a part of its task,
    /// working in the worker's worktree, gone when it sends done.
    Helper,
    Researcher,
}

/// How much outside content an agent has read: `Soft` after a researcher's
/// report (only sensitive changes need the person), `Full` after reading the
/// outside world itself (every change needs the person).
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub enum Taint {
    Clean,
    Soft,
    Full,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Phase {
    Plan,
    Impl,
    Test,
}

impl Phase {
    pub fn of(progress: u8) -> Phase {
        match progress {
            0..20 => Phase::Plan,
            20..70 => Phase::Impl,
            _ => Phase::Test,
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Phase::Plan => "plan",
            Phase::Impl => "impl",
            Phase::Test => "test",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Status {
    Idle,
    Working,
    /// Sent an `ask` and waits for the answer.
    Asking {
        to: AgentId,
        until: u64,
    },
    /// Sent `blocked`: waits for a researcher.
    Blocked,
    /// A sensitive change or report waits in the person's inbox.
    WaitingHuman,
    Researching {
        for_agent: AgentId,
        until: u64,
    },
    /// A live agent's session ended; telling it something starts a new one.
    Stopped,
    /// Remembered from an earlier run of imece, without a session: it costs
    /// nothing until the person speaks to it or a teammate writes to it.
    Asleep,
}

impl Status {
    /// Statuses that spend tokens.
    pub fn busy(self) -> bool {
        matches!(self, Status::Working | Status::Researching { .. })
    }
}

/// The envelope type of a message between agents. Only the type drives
/// routing and waking; the body is never interpreted.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum MsgKind {
    Assign,
    Ask,
    Answer,
    Inform,
    Handoff,
    Done,
    Blocked,
    Progress,
    Notice,
    Cancel,
}

impl MsgKind {
    pub const ALL: [MsgKind; 10] = [
        MsgKind::Assign,
        MsgKind::Ask,
        MsgKind::Answer,
        MsgKind::Inform,
        MsgKind::Handoff,
        MsgKind::Done,
        MsgKind::Blocked,
        MsgKind::Progress,
        MsgKind::Notice,
        MsgKind::Cancel,
    ];

    /// The name agents use for it.
    pub fn wire(self) -> &'static str {
        match self {
            MsgKind::Assign => "assign",
            MsgKind::Ask => "ask",
            MsgKind::Answer => "answer",
            MsgKind::Inform => "inform",
            MsgKind::Handoff => "handoff",
            MsgKind::Done => "done",
            MsgKind::Blocked => "blocked",
            MsgKind::Progress => "progress",
            MsgKind::Notice => "notice",
            MsgKind::Cancel => "cancel",
        }
    }

    pub fn from_wire(s: &str) -> Option<MsgKind> {
        MsgKind::ALL.into_iter().find(|k| k.wire() == s)
    }

    /// Whether it starts a turn of the agent it goes to. The others wait
    /// for that agent's next turn.
    pub fn wakes(self) -> bool {
        !matches!(self, MsgKind::Inform | MsgKind::Progress | MsgKind::Notice)
    }
}

/// Who an agent is talking with: the other end of its newest exchange on
/// its salon row; also the two ends of a conversation line.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum Party {
    Agent(AgentId),
    Person,
}

/// An agent's newest exchange: a message it sent or got, the person's
/// words to it, what it said at the end of a turn. Its salon row ends its
/// status with it (`→ claude-2 "which parser?"`, `← you "fix the parser"`)
/// until the next exchange replaces it; `until` only says how long the
/// row counts as news (it does not fold while the talk is fresh).
#[derive(Clone, Debug)]
pub struct Talk {
    pub with: Party,
    /// The agent said it (`→`), or was told it (`←`).
    pub mine: bool,
    pub text: String,
}

/// A message's number on the bus: given in order from 1 across the
/// world, kept between runs, so an agent can say which message it
/// replies to (`re`). 0 is a message saved before numbers were given.
pub type MsgId = u32;

/// A message waiting for an agent's next turn: its number, and the one
/// it replies to.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct Mail {
    pub from: AgentId,
    pub kind: MsgKind,
    pub body: String,
    pub id: MsgId,
    pub re: Option<MsgId>,
    /// The paths an `assign` gives its card (record 114); nothing on
    /// the other types.
    pub paths: Vec<Owned>,
    /// The checks an `assign` gives its card (record 127); nothing on the
    /// other types.
    pub checks: Vec<String>,
    /// The card an `assign` hands again (record 126): its receiver takes
    /// it with the turn the mail starts, instead of a new one.
    pub card: Option<CardId>,
    /// What a worker's `done` came to (record 126), for its heading.
    pub outcome: Outcome,
}

/// What a worker's `done` came to (record 126); `Ok` on every other
/// message.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Outcome {
    #[default]
    Ok,
    /// It said its card failed (none when it held none): no change came
    /// of it.
    Failed(Option<CardId>),
    /// Its card had gone to another try: it closed nothing.
    Late(CardId),
}

/// A Claude Code session an agent had, kept to go on with it after imece
/// starts again: Claude Code's id for it, and what Claude Code had reported
/// as the session's whole cost (millionths of a dollar) when last seen,
/// and each model's part of it (record 134; None when not known: a save
/// older than that).
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct Resumable {
    pub session: String,
    pub spent: u64,
    pub models: Option<imece_agent::ByModel>,
}

/// A card an agent put on the board: who, and what it asks for.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Post {
    pub by: AgentId,
    pub body: String,
}

/// Why a project could not be opened, closed or renamed.
#[derive(Debug, PartialEq)]
pub enum ProjectRefusal {
    /// The path is not inside a git checkout: the path and git's words.
    NotCheckout(String),
    /// An open project has that name.
    NameTaken(String),
    /// The checkout is open already, as the project named.
    PathOpen(String),
    /// The name is not one a project may have.
    BadName(String),
    /// The leader could not get a socket.
    Bus(String),
    /// The last open project stays open.
    Last,
    /// A solo's `[MODEL][@EFFORT]` asks for an effort Claude Code lacks.
    Choice(String),
    /// A team on a folder that is no git checkout with a first commit:
    /// the folder.
    NoGit(String),
    /// An archive's folder is not there any more: the folder.
    Gone(String),
}

impl ProjectRefusal {
    /// The refusal in words, for the command line.
    pub fn describe(&self) -> String {
        match self {
            ProjectRefusal::NotCheckout(why) => why.clone(),
            ProjectRefusal::NoGit(path) => no_git_team(path),
            ProjectRefusal::NameTaken(name) => format!("a project named {name} is open already"),
            ProjectRefusal::PathOpen(name) => format!("that checkout is open already, as {name}"),
            ProjectRefusal::BadName(name) => {
                format!("{name:?} can not name a project: 1-24 of letters, digits, . _ -")
            }
            ProjectRefusal::Bus(why) => format!("the leader could not get a socket: {why}"),
            ProjectRefusal::Last => "the last open project stays open".into(),
            ProjectRefusal::Choice(why) => why.clone(),
            ProjectRefusal::Gone(path) => format!("{path} is not there any more"),
        }
    }
}

/// Whether `name` may name a project: 1-24 ASCII letters, digits, `.`,
/// `_` or `-`, so `lead-NAME` is one word the composer can address.
/// Why a team does not open on the folder `path`: it is no git checkout
/// with a first commit, and what works there instead.
pub fn no_git_team(path: &str) -> String {
    format!(
        "{path} is not a git checkout with a commit: a team works in git worktrees. \
         A solo works here without git (--solo claude, :solo); or run \
         git init && git commit --allow-empty -m init first"
    )
}

pub fn valid_project_name(name: &str) -> bool {
    (1..=24).contains(&name.chars().count())
        && name
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, '.' | '_' | '-'))
}

/// Whether `name` can be a Lonca repository's: it reaches a line of the
/// protocol and the store's words, so it is plain.
pub fn valid_repo(name: &str) -> bool {
    (1..=64).contains(&name.chars().count())
        && name
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, '.' | '_' | '-'))
}

/// `stem` when no open project goes by it, else the first of `stem-2`,
/// `stem-3`, … that none does, cut so the whole stays a project name: a
/// second solo on a checkout. The search is bounded by the number of
/// projects.
pub fn free_project_name(w: &World, stem: &str) -> String {
    if w.project_by_name(stem).is_none() {
        return stem.to_string();
    }
    (2..=w.projects.len() + 2)
        .map(|n| {
            let suffix = format!("-{n}");
            let keep: String = stem.chars().take(24 - suffix.len()).collect();
            format!("{keep}{suffix}")
        })
        .find(|name| w.project_by_name(name).is_none())
        .expect("some number is free")
}

/// The checkout's directory name, `repo` for the root.
pub fn dir_name(repo: &Path) -> String {
    repo.file_name()
        .map_or("repo".into(), |n| n.to_string_lossy().into_owned())
}

/// The name a project takes from its checkout's directory: the name with
/// every character a project name may not have replaced by `-`, cut to 24.
pub fn project_name_of(repo: &Path) -> String {
    dir_name(repo)
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || matches!(c, '.' | '_' | '-') {
                c
            } else {
                '-'
            }
        })
        .take(24)
        .collect()
}

/// A project the person archived (`:archive`): put away with all it had,
/// to be looked at again or brought back; a fresh project took its place.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct Archived {
    /// The tick it was archived at.
    pub at: u64,
    /// What the person called it (else its name and its days).
    pub title: String,
}

/// A standing yes the person gave a project's agents: a program and, when
/// the command it was given for had one, its first argument (`npm run`,
/// `dotnet ef`); `sub` none for the program alone (`make`). A command
/// matches when it starts with exactly these words; an option before the
/// first argument never matches one with a `sub`. With `whole`, `program`
/// is one whole line a shell reads, and only those bytes match: what cannot
/// be said as a program and a word (`live::standing`).
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct Standing {
    pub program: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub sub: Option<String>,
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub whole: bool,
}

impl Standing {
    /// The yes from now on to exactly `line`.
    pub fn exactly(line: &str) -> Self {
        Standing {
            program: line.trim().to_string(),
            sub: None,
            whole: true,
        }
    }

    /// How the screen and the journal name it: `npm run …`, `make` (the
    /// program with nothing after it), or `` `rm -rf target` `` (that line).
    pub fn line(&self) -> String {
        match &self.sub {
            _ if self.whole => format!("`{}`", self.program),
            Some(sub) => format!("{} {sub} …", self.program),
            None => self.program.clone(),
        }
    }
}

#[derive(Clone, Debug)]
pub struct Project {
    pub id: ProjectId,
    pub name: String,
    pub leader: AgentId,
    /// The last change the merge queue landed; `None` without a merge queue
    /// (live agents keep their work in their own worktrees).
    pub trunk: Option<u32>,
    /// The project's files and the symbols agents lease in them (the
    /// demo's; live has none until leases are built).
    pub repo: &'static [(&'static str, &'static [&'static str])],
    /// The checkout's files as git lists them, for completion in a
    /// message (live: read on the git thread, at most [`FILES_MAX`]; not
    /// saved). The demo's files are `repo`'s (`World::files_of`).
    pub files: Vec<String>,
    /// Closed by the person (`:close`): its agents left and it is not
    /// drawn; kept so ids stay stable, and opened again in place.
    pub closed: bool,
    /// Pinned by the person (`:pin`): the salon shows it above the ones
    /// that are not.
    pub pinned: bool,
    /// Paths git ignores that the person listed (`:copy`) to be copied
    /// from the checkout into each worker's worktree.
    pub copies: Vec<String>,
    /// The commands the person said yes to from now on for this project
    /// (`A` on an ask, `:allow`): an agent's ask that only runs these (and
    /// routine commands) is allowed without them. Sorted, saved.
    pub allowed: Vec<Standing>,
    /// The plan gate (record 114): whether the leader must have the
    /// person's yes to a plan before it hires a second worker, and where
    /// that stands. Saved.
    pub plan: PlanGate,
    /// The folder is not a git checkout with a first commit, so its solo
    /// works straight in it: no worktree, no change to review. Found at
    /// each open and start, not saved.
    pub no_git: bool,
    /// The Lonca repository the person named for this project (`:lonca`,
    /// record 116) and what its daemon said last.
    pub lonca: Option<OnLonca>,
    /// Archived by the person (`:archive`): closed, but its agents are
    /// not gone; they sleep with their sessions, mail and changes until
    /// it is brought back. Never opened again in place.
    pub archived: Option<Archived>,
}

/// A project on Lonca: its repository (saved) and what the daemon said
/// last (memory only: read again from the first look on).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct OnLonca {
    pub repo: String,
    pub seen: StoreSeen,
    /// What is held in it now, as the last look read it (record 123).
    pub leases: Vec<StoreLease>,
    /// Every proposal waiting in it for a human, as the last look read
    /// it (record 125, step 4): the verdict on a change counts the ones
    /// that are no change shown here.
    pub pending: Vec<StorePending>,
    /// The names of the checks its policy in force defines, as the last
    /// look read them (lonca#203); none while Lonca has not said, when a
    /// card's checks are judged at the yes, as before.
    pub policy_checks: Option<Vec<String>>,
}

/// A proposal waiting in a Lonca repository, anyone's: what the verdict on
/// a change of the project reads for the overlaps the project's own
/// changes can not show.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct StorePending {
    /// The number `approve` and `reject` name.
    pub seq: u64,
    /// The change's id in the store.
    pub change: String,
    /// The agent's name, as Lonca has it.
    pub name: String,
    /// The agent of this project that proposed it; none for anyone else.
    pub holder: Option<AgentId>,
    /// The paths it changes, as Lonca lists them.
    pub paths: Vec<String>,
    /// Its base is trunk as it stands: a stale one is merged by the queue
    /// after the yes.
    pub on_trunk: bool,
}

/// A symbol held in a Lonca repository: the code map's rows on Lonca.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct StoreLease {
    pub file: String,
    pub symbol: String,
    /// The agent of this project that holds it; none for anyone else.
    pub holder: Option<AgentId>,
    /// The holder's name, as Lonca has it: what the map says of anyone
    /// else; `?` for a session it does not list.
    pub name: String,
    /// Held `shared`: others may hold it too.
    pub shared: bool,
}

/// What imece knows of a project's Lonca repository.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub enum StoreSeen {
    /// Named, not answered yet: closed like a store that is down.
    #[default]
    Asking,
    /// The daemon answered: trunk's commit, how many operations wait for
    /// a human, how far the checkout is behind.
    Up {
        trunk: String,
        pending: usize,
        behind: Behind,
    },
    Down(StoreDown),
}

/// A checkout the person is asked to import into Lonca (record 117): what
/// the window shows is what goes.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ImportAsk {
    pub project: ProjectId,
    /// The repository it becomes.
    pub repo: String,
    pub checkout: std::path::PathBuf,
    /// The branch that goes, and its commit.
    pub branch: String,
    pub head: String,
}

/// What `:fork` hears of a worker on Lonca: a workspace has no HEAD to fork
/// from, and one session owns its proposals (record 119).
pub const ON_LONCA_FORK: &str = "a worker on Lonca does not fork yet: hire another one";

/// A `lonca` command the person asked for, which an engine runs off the
/// screen's thread.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum StoreJob {
    /// The person signs in, the program running in this project's checkout.
    Login(ProjectId),
    Import(ImportAsk),
    /// Trunk of `repo` into the checkout of `project` (`:sync`).
    Sync {
        project: ProjectId,
        repo: String,
    },
    /// The person decides worker `agent`'s proposal (record 120).
    Decide {
        project: ProjectId,
        repo: String,
        agent: AgentId,
        proposal: StoreProposal,
        verdict: Decision,
    },
}

/// What the person says of a proposal.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Decision {
    /// Yes, and the checks of the change's card Lonca runs too (record
    /// 127, `--also`).
    Approve(Vec<String>),
    /// No, and why: their words reach the worker through Lonca too.
    Reject(String),
}

impl StoreJob {
    /// The project it was asked for.
    pub fn project(&self) -> ProjectId {
        match self {
            StoreJob::Login(p) => *p,
            StoreJob::Import(ask) => ask.project,
            StoreJob::Sync { project, .. } | StoreJob::Decide { project, .. } => *project,
        }
    }

    /// Which act it is, for the line that says how it went.
    pub fn act(&self) -> StoreAct {
        match self {
            StoreJob::Login(_) => StoreAct::Login,
            StoreJob::Import(_) => StoreAct::Import,
            StoreJob::Sync { .. } => StoreAct::Sync,
            StoreJob::Decide { .. } => StoreAct::Decide,
        }
    }
}

/// What a `lonca` command of the person's did.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum StoreAct {
    Login,
    Import,
    Sync,
    Decide,
}

/// How far a project's checkout is behind trunk (record 118).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Behind {
    /// Not known: a folder without git, or the daemon did not say.
    Unknown,
    /// Its commit never was trunk: the person's own, nothing fetched yet.
    Never,
    /// By this many integrations; none when it is level.
    By(u64),
}

/// What `:sync` did to the checkout.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Synced {
    /// It was level with trunk.
    Even,
    /// Its branch moved forward to trunk.
    Moved,
    /// It has commits of its own: trunk is fetched, nothing moved.
    Apart,
}

/// Why a project's Lonca repository can not be read (record 116).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum StoreDown {
    NoDaemon,
    NotTheDaemon,
    Protocol,
    /// A daemon older than the Lonca imece needs (`lonca::LEAST`).
    Old,
    /// This machine's Lonca is a hardened install, which imece does not
    /// work with yet (record 120).
    Hardened,
    SignedOut,
    Forbidden,
    NoRepo,
    Refused,
    /// The repository's policy can not be said to hold a change for a
    /// person: without that it would integrate unseen (record 120).
    Unguarded,
    /// The daemon asked to be asked again, look after look (`busy`,
    /// `rate_limited`, `closed`, …): one such answer closes nothing.
    Busy,
    /// The repository is frozen (`status.frozen`): it takes no proposal and
    /// no decision until a human unfreezes it.
    Frozen,
}

impl StoreDown {
    /// Why a hire or a Δ decision is refused, to an agent or after a
    /// name on the screen.
    pub fn why(self) -> &'static str {
        match self {
            StoreDown::NoDaemon => "no Lonca daemon answers",
            StoreDown::NotTheDaemon => "what is on Lonca's socket is not the person's daemon",
            StoreDown::Protocol => "Lonca's daemon speaks another protocol",
            StoreDown::Old => "Lonca's daemon is older than 0.2.1, which imece needs",
            StoreDown::Hardened => {
                "Lonca is installed hardened here, which imece does not work with yet"
            }
            StoreDown::SignedOut => "the person is not signed in to Lonca",
            StoreDown::Forbidden => "the person is not enrolled in a Lonca repository of that name",
            StoreDown::NoRepo => "Lonca has no such repository",
            StoreDown::Refused => "Lonca's daemon refused",
            StoreDown::Unguarded => {
                "this repository's policy does not hold a change for the person \
                 (unobserved_is_tainted): set it true"
            }
            StoreDown::Busy => "Lonca's daemon is busy: it asks to be asked again",
            StoreDown::Frozen => {
                "Lonca froze this repository: a person unfreezes it (lonca status says why)"
            }
        }
    }
}

#[derive(Clone, Debug)]
pub struct Agent {
    pub id: AgentId,
    pub project: ProjectId,
    pub name: String,
    pub role: Role,
    pub status: Status,
    pub card: Option<CardId>,
    /// The worker a helper works for.
    pub parent: Option<AgentId>,
    pub progress: u8,
    pub cost: u64,
    pub taint: Taint,
    pub talk: Option<Talk>,
    /// The researcher a blocked worker waits for.
    pub researcher: Option<AgentId>,
    /// What a researcher researches, what a blocked worker waits for, or
    /// why a stopped session stopped (`■ stopped: …` on its row).
    pub topic: String,
    /// Tick of the last status change; the screen marks recent changes.
    pub changed_at: u64,
    pub trace: VecDeque<String>,
    /// Trace lines ever written, the kept ones being the last of them; the
    /// store numbers lines by it.
    pub traced: u64,
    /// Its trace lines in each of the last [`STIR_LONG`] ticks: the
    /// activity bar on its row (`▁▃▅▂▁`, the last [`STIR_TICKS`]) and on
    /// its focus (all of them). Memory only.
    pub stirs: Stirs,
    /// What it was doing in each of the last [`LANE_TICKS`] ticks: its
    /// lane on the timeline (record 100). Memory only.
    pub lane: Lane,
    /// What a live agent is doing right now (`Edit src/main.rs`), drawn
    /// instead of the progress bar; empty while it thinks.
    pub activity: Option<String>,
    /// Its session is being started (the process, a first worktree's
    /// checkout) and has not answered yet: `● starting…` on its row.
    pub starting: bool,
    /// The agent's own command line as its session reported it (`/clear`,
    /// `/compact` ...), imece's `/clear` added where Claude Code has none;
    /// empty until a session says. The composer completes with it.
    pub commands: Vec<String>,
    /// Where a worker's worktree stands next to its project's checkout, as
    /// git last read it; nothing to show until it is read.
    pub stand: Stand,
    /// The Lonca repository it works in a workspace of (record 119): none
    /// in a worktree. Read anew at each start, not saved.
    pub workspace: Option<String>,
    /// The start point a worker was hired from (`hire`'s `from`: a branch,
    /// tag or commit of the checkout, or `#N`, a pull request), as asked;
    /// none for one made at the checkout's HEAD. Such a worktree is made
    /// there and never follows HEAD; its Δ is against that point. Saved.
    pub start: Option<String>,
    /// The newest loopback URL the agent wrote in its text this session
    /// (`local_url`), drawn on its row as a link the terminal opens.
    pub local: Option<String>,
    /// Its own to-do list, as Claude Code's task tools last set it
    /// (`imece_agent::Event::Plan`): `≡ 3/7` on its row, the steps on its
    /// focus. Gone with the session; not kept between runs.
    pub plan: Vec<Step>,
    /// How full its context window is, as Claude Code last said
    /// (`imece_agent::Event::Context`): `◔ 72%` on its row from
    /// `CONTEXT_SHOWN`, the figure on its focus. Gone with the session;
    /// not kept between runs.
    pub context: Option<Fill>,
    /// When imece tries its failed turn again after a passing API
    /// error, and which try that is (`↻ 1/3` on its row); none when no
    /// retry waits. Memory only.
    pub retry: Option<Retry>,
    /// The tries of a turn that keeps failing on passing API errors:
    /// none after a good turn or the person's word. Memory only.
    pub tries: u8,
    /// The person said yes to everything this agent asks until its turn
    /// ends (`e` on an ask), and how many asks that yes let go on since;
    /// none otherwise. The turn's end or the session's takes it away.
    /// Memory only.
    pub turn_yes: Option<u32>,
    /// The messages that wait with it while it waits on the person
    /// (record 107), for its row and `:why`. Memory only: whoever drives
    /// the agent sets it again.
    pub held_mail: usize,
    /// The fault Claude Code said the turn now ending failed on, taken by
    /// that turn's end (`imece_agent::Event::Failed`). Memory only.
    pub fault: Option<Fault>,
    /// Where its spend on the change at hand stands (record 113).
    pub task: Task,
    /// What of its spending Claude Code did not price (record 132).
    pub unpriced: Unpriced,
    /// Which attempt at its card its turn works on (record 126): set as
    /// it takes the card, so a `done` of a turn that began before the
    /// card was handed again is told apart. Memory only: a load sets it
    /// from the card.
    pub attempt: u32,
    /// What imece follows of it for its health notices (record 128).
    /// Memory only.
    pub watch: crate::health::Watch,
    /// What of its spending is booked on the ledger, and where its next
    /// goes (record 133). Memory only.
    pub meter: crate::ledger::Meter,
    /// A researcher that finished; kept so ids stay stable.
    pub gone: bool,
}

/// What a worker (its helpers counted in) or a solo spent on a change
/// (record 113): the cost in ten-thousandths of a dollar, the ticks it
/// worked, and the rounds it took (each `done` that was sent back, or
/// replaced by a newer one, is a round).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Spend {
    pub cost: u64,
    pub ticks: u64,
    pub rounds: u32,
    /// Part of it Claude Code did not price (record 132): `cost` is what
    /// it came to at least.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub unpriced: bool,
}

impl Spend {
    /// Both together.
    pub fn plus(self, other: Spend) -> Spend {
        Spend {
            cost: self.cost + other.cost,
            ticks: self.ticks + other.ticks,
            rounds: self.rounds + other.rounds,
            unpriced: self.unpriced || other.unpriced,
        }
    }
}

/// What of an agent's spending Claude Code did not price (record 132):
/// a turn's result without its figure, a model priced at nothing though
/// it used tokens, a session that ended mid-turn. Its `cost` is then what
/// it spent at least, never all.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Unpriced {
    /// The turn ending now said so (`imece_agent::Event::Unpriced`); its
    /// `TurnDone` takes it.
    pub said: bool,
    /// The session's last turn left something unpriced. Claude Code's
    /// figure is the session's whole, so a later turn priced whole covers
    /// it.
    pub session: bool,
    /// Gaps no later figure covers: a session that ended mid-turn or
    /// unpriced. Kept in the store (a session open at quit counted in).
    pub gaps: u32,
}

impl Unpriced {
    /// Whether any of its spending is unpriced.
    pub fn any(&self) -> bool {
        self.session || self.gaps > 0
    }

    /// Its gaps, and what its session left unpriced as one more, since a
    /// resumed session's figure may or may not cover it: what the store
    /// keeps.
    pub fn count(&self) -> u32 {
        self.gaps + u32::from(self.session || self.said)
    }
}

/// Where an agent's spend on its change stands (record 113). A round
/// opens at the first prompt after the last one closed (`World::round_opens`)
/// and closes when a change of the agent's is settled (`World::settle_change`,
/// or `change_ready` with its turn over), which takes the cost since it
/// opened; a helper's round closes with its worker's change. What a change
/// came to is carried when it leaves unapplied (sent back, replaced, part
/// applied) into the next change of the agent's, and dropped once one is
/// applied whole or the worktree holds nothing.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Task {
    /// The round open now: the agent's cost when it opened, and the tick.
    pub round: Option<(u64, u64)>,
    /// What the earlier rounds of the change at hand came to.
    pub carried: Spend,
    /// The newest change settled: what the leader's mail says a `done`
    /// cost.
    pub last: Option<Spend>,
    /// The agent's unpriced gaps when the round opened (record 132): more
    /// at its close leave the round unpriced.
    #[serde(default, skip_serializing_if = "is_zero")]
    pub gaps: u32,
}

fn is_zero(n: &u32) -> bool {
    *n == 0
}

/// A failed turn imece tries again (record 79).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Retry {
    /// Which try this is, from 1 to [`RETRY_TICKS`]' length.
    pub attempt: u8,
    /// The tick it goes at.
    pub at: u64,
}

/// After a passing API error, a turn is tried again this many ticks
/// after it failed, one entry per try: 30 s, 2 min, 8 min. When they are
/// used up the error waits for the person.
pub const RETRY_TICKS: [u64; 3] = [6, 24, 96];

/// Why a turn failed on Claude Code's side, as far as imece acts on it
/// (`imece_agent::Fault`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Fault {
    /// Overloaded, a server error, a lost connection, a short rate
    /// limit: tried again later.
    Passing,
    /// The account (signed out, a key, a quota, a bill): the person's.
    Account,
}

/// One step of an agent's own to-do list (`imece_agent::Step`).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Step {
    pub text: String,
    pub state: StepState,
}

/// Where a [`Step`] stands.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum StepState {
    Todo,
    Doing,
    Done,
}

/// How full an agent's context window is: the tokens in it, and its size
/// when Claude Code says.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Fill {
    pub used: u64,
    pub size: Option<u64>,
}

/// The row shows the context from this percent on.
pub const CONTEXT_SHOWN: u8 = 70;
/// And in the attention colour from this one.
pub const CONTEXT_LOUD: u8 = 90;

impl Fill {
    /// Percent of the window used, at most 100; none without a size.
    pub fn percent(&self) -> Option<u8> {
        let size = self.size.filter(|&s| s > 0)?;
        Some((self.used.saturating_mul(100) / size).min(100) as u8)
    }
}

/// Where a worker's worktree stands: the branch an agent put it on (none
/// while detached, as imece makes it), its own commits the checkout does
/// not have and the checkout's it does not have, and the lines its
/// worktree changed that a done would hand over.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Stand {
    pub branch: Option<String>,
    pub ahead: u32,
    pub behind: u32,
    pub added: u32,
    pub removed: u32,
}

impl Agent {
    pub fn new(
        id: AgentId,
        project: ProjectId,
        name: String,
        role: Role,
        status: Status,
        tick: u64,
    ) -> Agent {
        Agent {
            id,
            project,
            name,
            role,
            status,
            card: None,
            parent: None,
            progress: 0,
            cost: 0,
            taint: Taint::Clean,
            talk: None,
            researcher: None,
            topic: String::new(),
            changed_at: tick,
            trace: VecDeque::new(),
            traced: 0,
            stirs: Stirs::default(),
            lane: Lane::default(),
            activity: None,
            starting: false,
            commands: Vec::new(),
            stand: Stand::default(),
            workspace: None,
            start: None,
            local: None,
            plan: Vec::new(),
            context: None,
            retry: None,
            tries: 0,
            turn_yes: None,
            held_mail: 0,
            fault: None,
            task: Task::default(),
            unpriced: Unpriced::default(),
            attempt: 0,
            watch: Default::default(),
            meter: Default::default(),
            gone: false,
        }
    }

    /// How far the agent's to-do list is: the steps done, and all of them.
    pub fn plan_count(&self) -> (usize, usize) {
        let done = self
            .plan
            .iter()
            .filter(|s| s.state == StepState::Done)
            .count();
        (done, self.plan.len())
    }

    /// The step of its to-do list it says it is on now (the first one in
    /// progress), as the agent wrote it; none when no step is.
    pub fn step(&self) -> Option<&str> {
        self.plan
            .iter()
            .find(|s| s.state == StepState::Doing)
            .map(|s| s.text.as_str())
            .filter(|s| !s.trim().is_empty())
    }

    pub fn set_status(&mut self, status: Status, tick: u64) {
        if self.status != status {
            self.status = status;
            self.changed_at = tick;
        }
    }

    /// The agent wrote `text`: the newest loopback URL in it, if any, is
    /// the one its row shows from now on.
    pub fn wrote(&mut self, text: &str) {
        if let Some(url) = local_url(text) {
            self.local = Some(url);
        }
    }

    pub fn trace(&mut self, tick: u64, line: impl Into<String>) {
        if self.trace.len() == TRACE_KEEP {
            self.trace.pop_front();
        }
        self.trace
            .push_back(format!("{} {}", clock_seconds(tick), line.into()));
        self.traced += 1;
        self.stirs.add(tick);
        self.watch.traced_at = tick;
    }
}

/// The ticks an agent's activity bar covers on its row, one column each:
/// 25 s live.
pub const STIR_TICKS: usize = 5;

/// The ticks its longer bar covers on its focus: two minutes live.
pub const STIR_LONG: usize = 24;

/// Ticks a lane keeps: an hour.
pub const LANE_TICKS: usize = 720;

/// What an agent was doing in a tick, for its lane on the timeline, the
/// least first: when several are seen in one tick the lane keeps the
/// most (a short edit in a tick of reading shows as writing).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, PartialOrd, Ord)]
pub enum LaneCell {
    /// Nothing known: before it was first seen.
    #[default]
    Gap,
    Idle,
    Starting,
    Thinking,
    /// A tool call that changes nothing: reading, searching, running.
    Reading,
    /// A tool call that writes files.
    Writing,
    /// Waiting for a teammate's answer.
    Asking,
    /// The tick a turn ended.
    Done,
    /// Something of it waits for the person.
    Waiting,
    /// Blocked, or its session stopped.
    Stuck,
}

impl LaneCell {
    /// Whether the agent was in a turn.
    pub fn busy(self) -> bool {
        matches!(
            self,
            LaneCell::Starting
                | LaneCell::Thinking
                | LaneCell::Reading
                | LaneCell::Writing
                | LaneCell::Asking
        )
    }
}

/// An agent's lane: one [`LaneCell`] per tick for the last [`LANE_TICKS`],
/// the newest at `at`.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Lane {
    at: u64,
    cells: VecDeque<LaneCell>,
}

impl Lane {
    /// The agent was seen doing `cell` at `tick`. The ticks between the
    /// last one seen and this one (live's clock can jump) take the last
    /// cell seen; within one tick the most is kept; a tick older than the
    /// newest counts as the newest.
    pub fn mark(&mut self, tick: u64, cell: LaneCell) {
        if self.cells.is_empty() {
            self.at = tick;
            self.cells.push_back(cell);
            return;
        }
        let last = *self.cells.back().expect("not empty");
        let gone = tick.saturating_sub(self.at).min(LANE_TICKS as u64);
        for n in 0..gone {
            // The tick a turn ended is one tick, not the ones after it.
            let fill = if n + 1 == gone { cell } else { last.after() };
            self.cells.push_back(fill);
            if self.cells.len() > LANE_TICKS {
                self.cells.pop_front();
            }
        }
        self.at = self.at.max(tick);
        if gone == 0 {
            let newest = self.cells.back_mut().expect("not empty");
            *newest = (*newest).max(cell);
        }
    }

    /// The newest cell seen; a gap when none was.
    pub fn last(&self) -> LaneCell {
        self.cells.back().copied().unwrap_or_default()
    }

    /// The `n` ticks up to `tick`, the oldest first: a gap before the
    /// lane began and for the ticks since it was last seen.
    pub fn at(&self, tick: u64, n: usize) -> Vec<LaneCell> {
        let mut out = vec![LaneCell::Gap; n];
        for (k, slot) in out.iter_mut().enumerate() {
            // The tick this slot stands for, `n - 1 - k` before `tick`.
            let Some(t) = tick.checked_sub((n - 1 - k) as u64) else {
                continue;
            };
            if t > self.at {
                continue;
            }
            let back = (self.at - t) as usize;
            if let Some(i) = self.cells.len().checked_sub(back + 1) {
                *slot = self.cells[i];
            }
        }
        out
    }
}

impl LaneCell {
    /// What the ticks after this one were, until the next is seen: the
    /// same, but a turn's end is over at once.
    fn after(self) -> LaneCell {
        match self {
            LaneCell::Done => LaneCell::Idle,
            c => c,
        }
    }
}

/// How many trace lines an agent wrote in each of the [`STIR_LONG`]
/// ticks up to `at`, the oldest first; its row shows the last
/// [`STIR_TICKS`] of them.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Stirs {
    at: u64,
    counts: [u16; STIR_LONG],
}

impl Stirs {
    /// A line was written at `tick`; one from before the newest tick it
    /// has counts as the newest.
    pub fn add(&mut self, tick: u64) {
        self.counts = self.long(tick);
        self.at = self.at.max(tick);
        let last = &mut self.counts[STIR_LONG - 1];
        *last = last.saturating_add(1);
    }

    /// The row's columns as they stand at `tick`, the oldest first: the
    /// ticks since the newest line are empty columns.
    pub fn at(&self, tick: u64) -> [u16; STIR_TICKS] {
        let mut counts = [0; STIR_TICKS];
        counts.copy_from_slice(&self.long(tick)[STIR_LONG - STIR_TICKS..]);
        counts
    }

    /// The focus's columns as they stand at `tick`, the oldest first.
    pub fn long(&self, tick: u64) -> [u16; STIR_LONG] {
        let gone = tick.saturating_sub(self.at).min(STIR_LONG as u64) as usize;
        let mut counts = [0; STIR_LONG];
        counts[..STIR_LONG - gone].copy_from_slice(&self.counts[gone..]);
        counts
    }
}

/// Where a card stands (record 126). A card follows its owner's change:
/// it works until a `done`, its change waits for the person in `Review`,
/// sits in Lonca's merge queue in `Verifying`, and is `Done` once the
/// change is in whole (or the `done` left nothing to take). Sent back, it
/// works again. A `done` that says it failed makes no change: `Failed`,
/// for the leader to hand again.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Column {
    Waiting,
    Working,
    Verifying,
    Done,
    Review,
    Failed,
    /// The leader withdrew it (record 126): through, nothing comes of it,
    /// and what waits on it never begins.
    Withdrawn,
}

/// The tries a card has before the person decides on another (record
/// 126, F9): the leader hands a failed card again at most this many tries
/// in all.
pub const TRIES: u32 = 3;
/// The rounds a card's change has before the person decides on another:
/// it comes back to work by itself (Lonca's queue or a human there said
/// no) at most this many times.
pub const ROUNDS: u32 = 3;

/// Which of a card's caps stopped what would have gone on by itself
/// (record 126, F9).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Cap {
    /// It failed its last try: the leader may not hand it again.
    Tries,
    /// Its change came back its last time: its worker is not woken.
    Rounds,
}

impl Cap {
    /// The kind of the item that waits for the person.
    pub fn kind(self) -> InboxKind {
        match self {
            Cap::Tries => InboxKind::Tries,
            Cap::Rounds => InboxKind::Rounds,
        }
    }

    /// The cap an item of `kind` is about, if it is about one.
    pub fn of(kind: InboxKind) -> Option<Cap> {
        match kind {
            InboxKind::Tries => Some(Cap::Tries),
            InboxKind::Rounds => Some(Cap::Rounds),
            _ => None,
        }
    }
}

/// What a card's circuit breaker counts and what the person said past it
/// (record 126, F9).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(default)]
pub struct Breaker {
    /// Times its change came back to work: the person's reject, notes or
    /// clash sent back, or Lonca's queue or a human there saying no.
    pub sent_back: u32,
    /// Tries the person allowed past [`TRIES`].
    pub more_tries: u32,
    /// Rounds the person allowed past [`ROUNDS`].
    pub more_rounds: u32,
    /// The person failed it at the breaker: nobody hands it again.
    pub stopped: bool,
}

/// The most cards one card waits on (record 126, F10).
pub const AFTER_MAX: usize = 8;

/// What a worker's `done` is for (record 126).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DoneFor {
    /// The card it holds at the attempt its turn took.
    Card(CardId),
    /// A card handed again since its turn took it: the done closes nothing.
    Late(CardId),
    /// It holds no card.
    Nothing,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Symbol {
    pub file: &'static str,
    pub name: &'static str,
}

/// What happened on a card, in the order it happened.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum StoryEvent {
    Took {
        agent: AgentId,
    },
    Decision(String),
    WritingTests,
    Asked {
        from: AgentId,
        to: AgentId,
        text: String,
    },
    Answered {
        from: AgentId,
        text: String,
    },
    Assumption(String),
    BlockedOn(String),
    ResearchArrived {
        researcher: AgentId,
        topic: String,
    },
    ResearchNeedsYou {
        researcher: AgentId,
    },
    FromCache(String),
    Change {
        files: usize,
        plus: u64,
        minus: u64,
    },
    Sensitive(String),
    Verified {
        tests: u64,
        trunk: u32,
    },
    QueueFailed,
    YouApproved {
        research: bool,
    },
    YouRejected {
        research: bool,
    },
    YouSaid(String),
    /// A live agent's answer at the end of its turn.
    Replied(String),
    /// A live agent sent the person a message over the bus (`to: you`).
    ToYou {
        kind: MsgKind,
        text: String,
    },
    /// A live agent changed this file.
    Touched(String),
    /// The owner changed this file, which the card's paths do not name
    /// (record 114); once per file.
    Strayed(String),
    /// A live agent's turn or session ended without an answer.
    Stopped(String),
    /// imece was closed while the agent's turn ran; the turn is not run
    /// again, the agent hears of it when it wakes.
    Interrupted,
    /// A live agent sent a teammate a message: its number on the bus and
    /// the one it replies to (none in a file from before numbers: 0).
    Sent {
        from: AgentId,
        to: AgentId,
        kind: MsgKind,
        text: String,
        #[serde(default)]
        id: MsgId,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        re: Option<MsgId>,
    },
    /// A live agent put this card on the board; `text` is what it asks for.
    Posted {
        by: AgentId,
        text: String,
    },
    /// The leader handed the card to `to` as its `attempt`th try, taking
    /// it from `from` (record 126).
    Handed {
        from: Option<AgentId>,
        to: AgentId,
        attempt: u32,
    },
    /// An assign gave the card to `to` while it waited on the cards `on`
    /// (record 126, F10): it goes once they are done.
    Held {
        to: AgentId,
        on: Vec<CardId>,
    },
    /// The owner's `done` said the card failed, and why: no change.
    Failed(String),
    /// The leader withdrew it, and why (record 126).
    Withdrawn(String),
    /// What the card asks Lonca to run at the yes became `checks` (record
    /// 127): the leader's, handing it again (`by`), else the person's.
    ChecksSet {
        by: Option<AgentId>,
        checks: Vec<String>,
    },
    /// A `done` came from `from` after the card was handed again: it
    /// closed nothing.
    LateDone {
        from: AgentId,
    },
    /// Its `cap` stopped what would have gone on by itself, after `count`
    /// tries or rounds: the person decides (record 126, F9).
    Capped {
        cap: Cap,
        count: u32,
    },
    /// The person allowed one more try or round past `cap`, or failed the
    /// card.
    CapAnswered {
        cap: Cap,
        yes: bool,
    },
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum StoryKind {
    Intent,
    Assumption,
    Decision,
    Talk,
    Evidence,
    Change,
    Person,
}

impl StoryEvent {
    pub fn kind(&self) -> StoryKind {
        match self {
            StoryEvent::Took { .. }
            | StoryEvent::WritingTests
            | StoryEvent::BlockedOn(_)
            | StoryEvent::Posted { .. }
            | StoryEvent::Handed { .. }
            | StoryEvent::Held { .. } => StoryKind::Intent,
            StoryEvent::Decision(_) | StoryEvent::Withdrawn(_) | StoryEvent::ChecksSet { .. } => {
                StoryKind::Decision
            }
            StoryEvent::Asked { .. }
            | StoryEvent::Answered { .. }
            | StoryEvent::Replied(_)
            | StoryEvent::Sent { .. }
            | StoryEvent::ToYou { .. } => StoryKind::Talk,
            StoryEvent::Assumption(_) => StoryKind::Assumption,
            StoryEvent::ResearchArrived { .. }
            | StoryEvent::ResearchNeedsYou { .. }
            | StoryEvent::FromCache(_)
            | StoryEvent::Verified { .. }
            | StoryEvent::QueueFailed
            | StoryEvent::Stopped(_)
            | StoryEvent::Failed(_)
            | StoryEvent::LateDone { .. }
            | StoryEvent::Capped { .. }
            | StoryEvent::Interrupted => StoryKind::Evidence,
            StoryEvent::Change { .. }
            | StoryEvent::Sensitive(_)
            | StoryEvent::Touched(_)
            | StoryEvent::Strayed(_) => StoryKind::Change,
            StoryEvent::YouApproved { .. }
            | StoryEvent::YouRejected { .. }
            | StoryEvent::CapAnswered { .. }
            | StoryEvent::YouSaid(_) => StoryKind::Person,
        }
    }
}

#[derive(Clone, Debug)]
pub struct Story {
    pub tick: u64,
    pub event: StoryEvent,
}

#[derive(Clone, Debug)]
pub struct Card {
    pub id: CardId,
    pub project: ProjectId,
    pub title: String,
    pub owner: Option<AgentId>,
    pub column: Column,
    pub story: Vec<Story>,
    /// The symbols the owner leased.
    pub symbols: Vec<Symbol>,
    /// The paths the card owns (record 114): given with the `assign` or
    /// `post` that made it; none means no bounds.
    pub paths: Vec<Owned>,
    /// The checks of Lonca's policy it asks for (record 127), sorted:
    /// given with the `assign` or `post` that made it, never changed; the
    /// person's yes to its change asks Lonca to run them too.
    pub checks: Vec<String>,
    pub started: u64,
    pub finished: u64,
    /// How many times the card was taken (record 126): by the turn an
    /// `assign` opened, a `claim`, or an `assign` naming it to hand it
    /// again; none while it waits on the board untaken.
    pub attempt: u32,
    /// Its circuit breaker (record 126, F9).
    pub breaker: Breaker,
    /// The cards it waits on (record 126, F10): it is not offered, claimed
    /// or begun until each of them is done. Given with the `assign` or
    /// `post` that made it, never changed; only older cards of its
    /// project, so no card waits on itself through others.
    pub after: Vec<CardId>,
}

/// The part an agent had in a card, for the card's screen.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Part {
    Posted,
    Owner,
    Helper,
    Talked,
}

/// Where `text` names card `c` as the person sees it (`#323`, `c` being
/// 322): byte ranges of each `#` and its number, where no digit follows,
/// so `#32` is not found in `#323`.
/// The longest loopback URL a row links to; a longer one is not shown, so
/// what the person clicks is all on the row.
pub const LOCAL_URL_MAX: usize = 60;

/// The last loopback URL in `text` (`http://localhost:5173/app`), without
/// its query and fragment, or nothing. Strict, as it becomes a link the
/// person clicks: the scheme is `http` or `https`, the host `localhost`,
/// `127.0.0.1` or `[::1]` and nothing else (`localhost.evil.com` and
/// `localhost@evil.com` are not it), the port 1–65535, the path plain
/// ASCII without quotes, brackets, backslashes or control characters;
/// sentence punctuation after it is not part of it.
pub fn local_url(text: &str) -> Option<String> {
    text.match_indices("http")
        .filter_map(|(i, _)| local_url_at(&text[i..]))
        .last()
}

/// The loopback URL `s` starts with, as `local_url` takes it.
fn local_url_at(s: &str) -> Option<String> {
    let rest = s
        .strip_prefix("http://")
        .or_else(|| s.strip_prefix("https://"))?;
    let host = ["localhost", "127.0.0.1", "[::1]"]
        .into_iter()
        .find(|h| rest.starts_with(h))?;
    let after = &rest[host.len()..];
    let run = after
        .find(|c: char| !url_char(c))
        .map_or(after, |end| &after[..end]);
    // The query and the fragment are left out of the link.
    let run = run.find(['?', '#']).map_or(run, |end| &run[..end]);
    let run = run.trim_end_matches(['.', ',', ':', '!']);
    let (port, path) = match run.strip_prefix(':') {
        Some(p) => {
            let end = p.find('/').unwrap_or(p.len());
            (Some(&p[..end]), &p[end..])
        }
        None => (None, run),
    };
    if !path.is_empty() && !path.starts_with('/') {
        // `localhost.evil.com`, `localhost@evil.com`, `localhostx`.
        return None;
    }
    if let Some(port) = port {
        let digits = port.len() <= 5 && port.bytes().all(|b| b.is_ascii_digit());
        if !digits || !(1..=65535).contains(&port.parse::<u32>().ok()?) {
            return None;
        }
    }
    let scheme_len = s.len() - rest.len();
    let url = &s[..scheme_len + host.len() + run.len()];
    (url.len() <= LOCAL_URL_MAX).then(|| url.to_string())
}

/// The most setup script approvals kept; the oldest go first.
pub const SETUP_KEPT: usize = 64;

/// The longest web address the screen makes a link of, in bytes.
pub const WEB_URL_MAX: usize = 2000;

/// The web address `s` starts with (`https://example.com/a?b=1`), or
/// nothing. Strict, as it becomes a link the person clicks: the scheme is
/// `http` or `https` and nothing else, the host starts with a letter, a
/// digit, every character is one [`url_char`] allows or a bracket
/// (ASCII, no quotes, spaces, `;`, `\` or control characters), at most
/// [`WEB_URL_MAX`] bytes; sentence punctuation after it is not part of it,
/// nor a closing bracket it did not open.
pub fn web_url_at(s: &str) -> Option<&str> {
    let rest = s
        .strip_prefix("http://")
        .or_else(|| s.strip_prefix("https://"))?;
    if !rest.starts_with(|c: char| c.is_ascii_alphanumeric()) {
        return None;
    }
    let end = rest
        .find(|c: char| !url_char(c) && c != '(' && c != ')')
        .unwrap_or(rest.len());
    let mut url = &s[..s.len() - rest.len() + end];
    // Bounded whatever a mutant does to the steps: each round that goes
    // on takes a byte off.
    for _ in 0..s.len() {
        let trimmed = url.trim_end_matches(['.', ',', ':', '!', '?', '*']);
        let more = |close, open| trimmed.matches(close).count() > trimmed.matches(open).count();
        let cut = match trimmed.chars().last() {
            Some(')') => more(')', '('),
            Some(']') => more(']', '['),
            _ => false,
        };
        url = if cut {
            &trimmed[..trimmed.len() - 1]
        } else {
            trimmed
        };
        if !cut {
            break;
        }
    }
    (url.len() <= WEB_URL_MAX).then_some(url)
}

/// Whether `s` is one web address whole, as [`web_url_at`] takes it.
pub fn is_web_url(s: &str) -> bool {
    web_url_at(s) == Some(s)
}

/// A character a loopback URL's port and path may have: ASCII letters,
/// digits and `-._~:/?#[]@!$&*+,=%`; no quotes, brackets, `;`, `\` or
/// anything that could end the terminal's link sequence.
fn url_char(c: char) -> bool {
    c.is_ascii_alphanumeric() || "-._~:/?#[]@!$&*+,=%".contains(c)
}

pub fn mentions(text: &str, c: CardId) -> Vec<std::ops::Range<usize>> {
    let tag = format!("#{}", c + 1);
    text.match_indices(&tag)
        .map(|(at, _)| at..at + tag.len())
        .filter(|r| !text[r.end..].starts_with(|ch: char| ch.is_ascii_digit()))
        .collect()
}

impl Card {
    /// Leases count while the change is not merged; a change waiting for
    /// the person lets them go, as the demo's done always did.
    pub fn holds_leases(&self) -> bool {
        matches!(self.column, Column::Working | Column::Verifying)
    }

    /// Whether the card is through, well or not: nothing more comes of
    /// it unless it is handed again.
    pub fn over(&self) -> bool {
        matches!(
            self.column,
            Column::Done | Column::Failed | Column::Withdrawn
        )
    }

    /// Whether its tries are out (record 126, F9): another needs the
    /// person's yes.
    pub fn tries_out(&self) -> bool {
        self.attempt >= TRIES + self.breaker.more_tries
    }

    /// Whether its rounds are out: its change came back as often as it
    /// may by itself.
    pub fn rounds_out(&self) -> bool {
        self.breaker.sent_back >= ROUNDS + self.breaker.more_rounds
    }
}

/// Why a remembered agent woke in a new session, not the one it had.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum Fresh {
    /// It had none to go on with: never one, or forgotten, or cleared.
    #[default]
    NoSession,
    /// Its session was not where Claude Code keeps it (a transcript
    /// gone, an id that is no session's).
    Missing,
    /// Claude Code would not go on with it; its words, cut.
    Refused(String),
}

/// An event of the whole guild, for the journal.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Event {
    Plan {
        leader: AgentId,
        request: String,
    },
    Assign {
        agent: AgentId,
        card: CardId,
    },
    Talk {
        from: AgentId,
        to: AgentId,
        text: String,
    },
    Assumption {
        agent: AgentId,
        text: String,
    },
    Blocked {
        agent: AgentId,
        topic: String,
        joined: Option<AgentId>,
    },
    ResearchOpened {
        researcher: AgentId,
        topic: String,
    },
    ResearchDelivered {
        researcher: AgentId,
        topic: String,
    },
    ResearchNeedsYou {
        researcher: AgentId,
        topic: String,
    },
    FromCache {
        agent: AgentId,
        topic: String,
    },
    Sensitive {
        agent: AgentId,
        what: String,
    },
    Merged {
        agent: AgentId,
        card: CardId,
    },
    Failed {
        agent: AgentId,
    },
    Approved {
        agent: AgentId,
        research: bool,
    },
    Rejected {
        agent: AgentId,
    },
    YouSaid {
        to: AgentId,
        text: String,
    },
    /// The person allowed or refused what a live agent's session asked
    /// before doing it (a command, or the tool and what it touches).
    Decided {
        agent: AgentId,
        what: String,
        allowed: bool,
    },
    /// The person said yes to everything `agent` asks until its turn
    /// ends.
    TurnYes {
        agent: AgentId,
    },
    /// `agent`'s turn (or session) ended with the person's yes for it,
    /// which let `allowed` of its asks go on without asking.
    TurnYesEnded {
        agent: AgentId,
        allowed: u32,
    },
    /// The person said yes from now on to `rules` for the project
    /// `leader` heads (`added`), or took them back.
    Standing {
        leader: AgentId,
        rules: Vec<String>,
        added: bool,
    },
    /// A live agent sent the person a message over the bus (`to: you`);
    /// an `ask` waits in their inbox, the rest is for the journal. An
    /// ask may offer answers to pick with one key (record 125).
    ToYou {
        from: AgentId,
        kind: MsgKind,
        text: String,
        #[serde(default)]
        options: Vec<String>,
    },
    /// A live agent answered the person.
    Replied {
        agent: AgentId,
        text: String,
    },
    /// A live agent's turn or session ended without an answer.
    Stopped {
        agent: AgentId,
        why: String,
    },
    /// A live agent's turn failed on a passing API error (`why` in the
    /// Claude Code's words); imece tries it again `seconds` later, the
    /// `attempt`th time.
    Retrying {
        agent: AgentId,
        why: String,
        attempt: u8,
        seconds: u64,
    },
    /// imece was closed while the agent's turn ran (or it died); the turn
    /// is not run again, the agent hears of it when it wakes.
    Interrupted {
        agent: AgentId,
    },
    /// A live agent sent a teammate a message (its first line): its
    /// number on the bus and the one it replies to (none in a file from
    /// before numbers: 0).
    Sent {
        from: AgentId,
        to: AgentId,
        kind: MsgKind,
        text: String,
        #[serde(default)]
        id: MsgId,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        re: Option<MsgId>,
    },
    /// A live agent put a card on the board.
    Posted {
        agent: AgentId,
        card: CardId,
    },
    /// A live worker took a card from the board.
    Claimed {
        agent: AgentId,
        card: CardId,
    },
    /// The leader handed card `card` to `agent` as its `attempt`th try,
    /// taking it from `from` when someone held it (record 126).
    Handed {
        agent: AgentId,
        card: CardId,
        from: Option<AgentId>,
        attempt: u32,
    },
    /// An assign gave card `card` to `agent` while it waited on the cards
    /// `on` (record 126, F10): held until they are done.
    Held {
        agent: AgentId,
        card: CardId,
        on: Vec<CardId>,
    },
    /// What held card `card` waited on is done: `agent` begins it now.
    Released {
        agent: AgentId,
        card: CardId,
    },
    /// Card `card`'s checks became `checks` (record 127): set by `agent`,
    /// the leader handing it again, or by the person (`you`) on the
    /// change of `agent`, its worker.
    ChecksSet {
        agent: AgentId,
        card: CardId,
        checks: Vec<String>,
        #[serde(default, skip_serializing_if = "std::ops::Not::not")]
        you: bool,
    },
    /// imece saw `sign` of agent `agent` and told its leader, or only
    /// the person when it leads (record 128).
    Health {
        agent: AgentId,
        sign: crate::health::Sign,
    },
    /// The person ended process `pid`, named `name`, under agent
    /// `agent`'s session (`:ps`, record 129).
    Killed {
        agent: AgentId,
        pid: u32,
        name: String,
    },
    /// Leader `agent` withdrew card `card`, and why (record 126).
    Withdrawn {
        agent: AgentId,
        card: CardId,
        why: String,
    },
    /// Worker `agent`'s `done` said card `card` failed; `why` is its first
    /// line. No change comes of it.
    CardFailed {
        agent: AgentId,
        card: CardId,
        why: String,
    },
    /// Worker `agent` sent a `done` after card `card` was handed again
    /// (or taken from it): it closed nothing and showed no change.
    LateDone {
        agent: AgentId,
        card: CardId,
    },
    /// Card `card`'s `cap` stopped what would have gone on by itself after
    /// `count` tries or rounds; `agent` (the leader that would hand it, or
    /// the worker whose change came back) waits for the person (record
    /// 126, F9).
    Capped {
        agent: AgentId,
        card: CardId,
        cap: Cap,
        count: u32,
    },
    /// The person allowed card `card` one more try or round past `cap`,
    /// or failed it.
    CapAnswered {
        agent: AgentId,
        card: CardId,
        cap: Cap,
        yes: bool,
    },
    /// A live leader hired a worker; `model` is the one it was given, or
    /// none for Claude Code's own default; `from` the start point its
    /// worktree was asked to start from, none for the checkout's HEAD.
    Hired {
        by: AgentId,
        agent: AgentId,
        model: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        from: Option<String>,
    },
    /// The person forked worker `from` (`:fork`): `agent` is a new worker
    /// that goes on from `from`'s conversation and uncommitted change, in
    /// a worktree of its own (record 94).
    Forked {
        from: AgentId,
        agent: AgentId,
    },
    /// A helper sent done to its worker and left.
    Left {
        agent: AgentId,
    },
    /// A live agent's model was not available here; it runs on the
    /// Claude Code's default from now on.
    Fallback {
        agent: AgentId,
        model: String,
    },
    /// A remembered agent woke in the session it had when imece closed,
    /// with everything it knew.
    Resumed {
        agent: AgentId,
    },
    /// A remembered agent woke in a new session, told where it left off,
    /// and why not in its old one.
    Restarted {
        agent: AgentId,
        #[serde(default)]
        why: Fresh,
    },
    /// The person changed the budget (`:budget`), in ten-thousandths of a
    /// dollar; `leader` is the one of the project they were looking at.
    Budget {
        leader: AgentId,
        from: u64,
        to: u64,
    },
    /// At the rate of the last [`RATE_WINDOW`] (ten-thousandths of a dollar
    /// an hour), the pot lasts `minutes` more; `leader` is the first open
    /// project's. Said once, until the person changes the budget.
    PotLow {
        leader: AgentId,
        rate: u64,
        minutes: u64,
    },
    /// The person ran a command in their own shell (`!command`), in the
    /// checkout; `leader` is the one of the project they were looking at.
    /// `took` is how long it ran, in milliseconds (none in a journal
    /// saved before it was kept).
    Shell {
        leader: AgentId,
        command: String,
        ended: Ended,
        #[serde(default)]
        took: Option<u64>,
    },
    /// The person opened the checkout at `path` as a project (`:open`);
    /// `leader` is its new leader.
    Opened {
        leader: AgentId,
        path: String,
    },
    /// The solo `leader` works straight in the folder at `path`: it is no
    /// git checkout with a first commit, so there is no worktree and no
    /// change to review. Said at its open and at each start.
    NoGit {
        leader: AgentId,
        path: String,
    },
    /// The Lonca repository `repo` of `leader`'s project can not be read:
    /// said once, until it comes back.
    StoreDown {
        leader: AgentId,
        repo: String,
        why: StoreDown,
    },
    /// The repository `repo` of `leader`'s project answers again.
    StoreBack {
        leader: AgentId,
        repo: String,
    },
    /// The person signed in to Lonca (`:lonca login`), as `human`; empty
    /// when the program did not say. `leader` is of the project at hand.
    StoreSignedIn {
        leader: AgentId,
        human: String,
    },
    /// The checkout of `leader`'s project went into Lonca as `repo`, with
    /// what Lonca warned of, its policy from Lonca's hardened profile when
    /// `hardened`.
    StoreImported {
        leader: AgentId,
        repo: String,
        warnings: Vec<String>,
        #[serde(default)]
        hardened: bool,
    },
    /// Repository `repo`, imported for `leader`'s project, still refuses a
    /// change to a critical path (record 121), for `why`.
    StoreCriticalKept {
        leader: AgentId,
        repo: String,
        why: Option<String>,
    },
    /// `:sync` brought repository `repo` to `leader`'s project's checkout.
    StoreSynced {
        leader: AgentId,
        repo: String,
        how: Synced,
    },
    /// A `lonca` command of the person's did not go through: what it said,
    /// none when it said nothing (no program, out of time).
    StoreFailed {
        leader: AgentId,
        act: StoreAct,
        why: Option<String>,
    },
    /// The person closed a project (`:close`); its agents left.
    Closed {
        leader: AgentId,
    },
    /// The person archived the project `was` (`:archive`) as `title`;
    /// `leader` heads the fresh project that took its place, so the line
    /// stays in sight with it.
    Archived {
        leader: AgentId,
        was: AgentId,
        title: String,
    },
    /// The person brought the archive `title` back (`:restore`); `leader`
    /// heads it. The team (or the solo of its name) open on its checkout
    /// was archived in its place as `swapped`, or closed (its own line)
    /// when it had done nothing.
    Restored {
        leader: AgentId,
        title: String,
        swapped: Option<String>,
    },
    /// The person renamed a project (`:rename`), `from` to `to`.
    Renamed {
        leader: AgentId,
        from: String,
        to: String,
    },
    /// The person made an agent forget its session (`:forget`, or every
    /// agent's with `--fresh`): the next message starts a new one, told
    /// where it left off, and what Claude Code kept of the old one is gone.
    Forgotten {
        agent: AgentId,
    },
    /// The person changed what a solo runs on (`:model`, `:effort`):
    /// `choice` is Claude Code and the `[MODEL][@EFFORT]`, as the row shows
    /// it. Its session starts again on it.
    Chose {
        agent: AgentId,
        choice: String,
    },
    /// Agent `agent`'s session said nothing about its own command line
    /// where Claude Code's format puts it (not an empty list: no list): the
    /// format may have changed, and only imece's `/clear` completes. Once
    /// a session.
    CommandsMissing {
        agent: AgentId,
    },
    /// imece started with the confine dial at `level` (`standard` or
    /// `open`, never strict: that has nothing to say): the agents' sessions
    /// get Claude Code's shell and web, and isolation is a container's
    /// job, not imece's. Once per start, on the project imece runs in.
    Confine {
        leader: AgentId,
        level: String,
    },
    /// The worktree setup script ran in agent `agent`'s new worktree:
    /// whether it went well, how it ended (`exit 0, 3.2 s`), its last
    /// lines (cleaned when drawn).
    SetupRan {
        agent: AgentId,
        ok: bool,
        end: String,
        lines: Vec<String>,
    },
    /// The setup script could not be read or started for agent `agent`.
    SetupFailed {
        agent: AgentId,
        why: String,
    },
    /// The person said yes (or no) to the setup script asked about for
    /// agent `agent`.
    SetupAnswered {
        agent: AgentId,
        yes: bool,
    },
    /// The repository's check (record 112) ran on agent `agent`'s change:
    /// whether it passed, how it ended, its last lines.
    Checked {
        agent: AgentId,
        ok: bool,
        end: String,
        lines: Vec<String>,
    },
    /// The check could not be read or started on agent `agent`'s change.
    CheckFailed {
        agent: AgentId,
        why: String,
    },
    /// Agent `agent` wrote `path`, which its card `card` does not own
    /// (record 114); it is told so in its next prompt, never stopped.
    Strayed {
        agent: AgentId,
        card: CardId,
        path: String,
    },
    /// The leader `leader` made a plan of `cards` cards it expects to
    /// cost `cost` (ten-thousandths of a dollar); the person reads it
    /// (record 114).
    Planned {
        leader: AgentId,
        cards: usize,
        cost: u64,
    },
    /// The person said yes (or no) to leader `leader`'s plan.
    PlanAnswered {
        leader: AgentId,
        yes: bool,
    },
    /// The person said yes (or no) to the check asked about for agent
    /// `agent`'s change.
    CheckAnswered {
        agent: AgentId,
        yes: bool,
    },
    /// What waits went to the person's phone (`:notify push`) in these
    /// words, `agent` being who the first of it is about; in the demo it
    /// was only shown (`sent` false).
    Pushed {
        agent: AgentId,
        words: String,
        sent: bool,
    },
    /// A push did not go: why (curl's own first line, cleaned). Said
    /// once until one goes again.
    PushFailed {
        agent: AgentId,
        why: String,
    },
    /// A live agent's worktree was clean at an older commit and was moved
    /// to the checkout's HEAD when its session started.
    WorktreeMoved {
        agent: AgentId,
        from: String,
        to: String,
    },
    /// A live agent's worktree has work of its own (`commits`: commits the
    /// checkout does not have; else uncommitted changes) and stays at `at`
    /// while the checkout's HEAD is `head`.
    WorktreeKept {
        agent: AgentId,
        at: String,
        head: String,
        commits: bool,
    },
    /// A live worker finished with a change in its worktree; it waits for
    /// the person.
    ChangeReady {
        agent: AgentId,
        files: usize,
        added: u32,
        removed: u32,
        /// What it cost, when known as it is read (the agent's turn over).
        spend: Option<Spend>,
    },
    /// What agent `agent`'s waiting change cost, known once its turn ended
    /// (record 113).
    ChangeCost {
        agent: AgentId,
        spend: Spend,
    },
    /// Worker `agent`'s proposal left Lonca's waiting list (record 120).
    Landed {
        agent: AgentId,
        how: Landed,
    },
    /// What Lonca's queue checked on `agent`'s proposal (record 122).
    Verified {
        agent: AgentId,
        ok: bool,
        checks: Vec<Attested>,
    },
    /// The person applied a live worker's change to their checkout, where
    /// it is staged; `mark` is the commit that stands for it in the
    /// worktree.
    Applied {
        agent: AgentId,
        files: usize,
        mark: String,
    },
    /// A live worker's change did not fit the checkout; it still waits.
    ApplyFailed {
        agent: AgentId,
        why: String,
    },
    /// `model` suggested a commit message for what was staged in the
    /// checkout of `leader`'s project (`:commit`), for `cost`
    /// (ten-thousandths of a dollar, from the pot).
    Suggested {
        leader: AgentId,
        model: String,
        cost: u64,
    },
    /// No commit message came for `leader`'s project: why, and what the
    /// try cost.
    SuggestFailed {
        leader: AgentId,
        why: String,
        cost: u64,
    },
    /// `by`'s `model` read worker `agent`'s change (the patch whose
    /// [`patch_sum`] is `patch`) and said `text`, for `cost`
    /// (ten-thousandths of a dollar, from the pot): a note on the change,
    /// never a gate (`:review`, record 106).
    Reviewed {
        agent: AgentId,
        model: String,
        patch: String,
        text: String,
        cost: u64,
    },
    /// No review came for worker `agent`'s change: why, and what the try
    /// cost.
    ReviewFailed {
        agent: AgentId,
        why: String,
        cost: u64,
    },
    /// The paths the person listed (`:copy`), or else those the
    /// checkout's `.worktreeinclude` names (`included`), that worker
    /// `agent`'s worktree lacked were copied into it from the checkout,
    /// `bytes` in all.
    Copied {
        agent: AgentId,
        paths: Vec<String>,
        bytes: u64,
        #[serde(default)]
        included: bool,
    },
    /// The checkout of `leader`'s project lists more files than imece
    /// keeps for completion: the first `kept` complete.
    FilesCut {
        leader: AgentId,
        kept: usize,
    },
    /// A listed path, or one the checkout's `.worktreeinclude` names
    /// (`included`), was not copied into worker `agent`'s worktree: why.
    CopySkipped {
        agent: AgentId,
        path: String,
        why: String,
        #[serde(default)]
        included: bool,
    },
    /// A part of the checkout's `.worktreeinclude` was not followed at
    /// worker `agent`'s start: one of its lines (its number, its text),
    /// or the whole file (`line` none); why.
    IncludeRefused {
        agent: AgentId,
        line: Option<(usize, String)>,
        why: String,
    },
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum EntryKind {
    Assign,
    Plan,
    Talk,
    Assumption,
    Blocked,
    Research,
    Sensitive,
    Merged,
    Failed,
    Person,
}

impl Event {
    pub fn kind(&self) -> EntryKind {
        match self {
            Event::Plan { .. }
            | Event::Posted { .. }
            | Event::Withdrawn { .. }
            | Event::Hired { .. }
            | Event::Forked { .. }
            | Event::Planned { .. }
            | Event::Left { .. } => EntryKind::Plan,
            Event::Assign { .. }
            | Event::Claimed { .. }
            | Event::Handed { .. }
            | Event::Held { .. }
            | Event::ChecksSet { .. }
            | Event::Released { .. } => EntryKind::Assign,
            Event::Talk { .. }
            | Event::Replied { .. }
            | Event::Sent { .. }
            | Event::ToYou { .. } => EntryKind::Talk,
            Event::Assumption { .. }
            | Event::Fallback { .. }
            | Event::Resumed { .. }
            | Event::Restarted { .. }
            | Event::Pushed { .. }
            | Event::SetupRan { ok: true, .. }
            | Event::Checked { ok: true, .. }
            | Event::Verified { ok: true, .. }
            | Event::CommandsMissing { .. }
            | Event::Confine { .. }
            | Event::WorktreeMoved { .. }
            | Event::Copied { .. }
            | Event::FilesCut { .. }
            | Event::NoGit { .. }
            | Event::StoreDown { .. }
            | Event::StoreBack { .. }
            | Event::StoreSignedIn { .. }
            | Event::StoreImported { .. }
            | Event::StoreCriticalKept { .. }
            | Event::StoreSynced { .. }
            | Event::StoreFailed { .. }
            | Event::Retrying { .. }
            | Event::ChangeCost { .. }
            | Event::LateDone { .. }
            | Event::WorktreeKept { .. } => EntryKind::Assumption,
            Event::Blocked { .. } | Event::Capped { .. } => EntryKind::Blocked,
            Event::Health { sign, .. } if sign.wakes() => EntryKind::Blocked,
            Event::Health { .. } => EntryKind::Assumption,
            Event::ResearchOpened { .. }
            | Event::ResearchDelivered { .. }
            | Event::FromCache { .. } => EntryKind::Research,
            Event::ResearchNeedsYou { .. }
            | Event::Sensitive { .. }
            | Event::ChangeReady { .. }
            | Event::Suggested { .. }
            | Event::Reviewed { .. }
            | Event::Strayed { .. }
            | Event::PotLow { .. } => EntryKind::Sensitive,
            Event::Merged { .. }
            | Event::Applied { .. }
            | Event::Landed {
                how: Landed::Integrated(_),
                ..
            } => EntryKind::Merged,
            Event::Failed { .. }
            | Event::CardFailed { .. }
            | Event::Stopped { .. }
            | Event::Interrupted { .. }
            | Event::SuggestFailed { .. }
            | Event::ReviewFailed { .. }
            | Event::CopySkipped { .. }
            | Event::IncludeRefused { .. }
            | Event::PushFailed { .. }
            | Event::SetupRan { ok: false, .. }
            | Event::SetupFailed { .. }
            | Event::Checked { ok: false, .. }
            | Event::Verified { ok: false, .. }
            | Event::CheckFailed { .. }
            | Event::Landed {
                how: Landed::Conflicted(_) | Landed::Rejected(_),
                ..
            }
            | Event::ApplyFailed { .. } => EntryKind::Failed,
            Event::Approved { .. }
            | Event::Rejected { .. }
            | Event::Landed {
                how: Landed::Queued | Landed::Withdrawn | Landed::Replaced,
                ..
            }
            | Event::YouSaid { .. }
            | Event::Decided { .. }
            | Event::TurnYes { .. }
            | Event::TurnYesEnded { .. }
            | Event::SetupAnswered { .. }
            | Event::CheckAnswered { .. }
            | Event::PlanAnswered { .. }
            | Event::CapAnswered { .. }
            | Event::Standing { .. }
            | Event::Budget { .. }
            | Event::Shell { .. }
            | Event::Opened { .. }
            | Event::Closed { .. }
            | Event::Archived { .. }
            | Event::Restored { .. }
            | Event::Renamed { .. }
            | Event::Forgotten { .. }
            | Event::Killed { .. }
            | Event::Chose { .. } => EntryKind::Person,
        }
    }

    /// The card the event names, if it names one.
    pub fn card(&self) -> Option<CardId> {
        match *self {
            Event::Assign { card, .. }
            | Event::Merged { card, .. }
            | Event::Posted { card, .. }
            | Event::Handed { card, .. }
            | Event::Held { card, .. }
            | Event::Released { card, .. }
            | Event::ChecksSet { card, .. }
            | Event::Withdrawn { card, .. }
            | Event::CardFailed { card, .. }
            | Event::LateDone { card, .. }
            | Event::Capped { card, .. }
            | Event::CapAnswered { card, .. }
            | Event::Claimed { card, .. } => Some(card),
            Event::Health { ref sign, .. } => sign.card(),
            _ => None,
        }
    }

    /// The agent the event is about, for its project.
    pub fn agent(&self) -> AgentId {
        match *self {
            Event::Plan { leader, .. }
            | Event::FilesCut { leader, .. }
            | Event::NoGit { leader, .. }
            | Event::StoreDown { leader, .. }
            | Event::StoreBack { leader, .. }
            | Event::StoreSignedIn { leader, .. }
            | Event::StoreImported { leader, .. }
            | Event::StoreCriticalKept { leader, .. }
            | Event::StoreSynced { leader, .. }
            | Event::StoreFailed { leader, .. }
            | Event::Budget { leader, .. }
            | Event::PotLow { leader, .. }
            | Event::Suggested { leader, .. }
            | Event::SuggestFailed { leader, .. }
            | Event::Shell { leader, .. }
            | Event::Confine { leader, .. }
            | Event::Opened { leader, .. }
            | Event::Closed { leader }
            | Event::Archived { leader, .. }
            | Event::Restored { leader, .. }
            | Event::Standing { leader, .. }
            | Event::Renamed { leader, .. } => leader,
            Event::Assign { agent, .. }
            | Event::Assumption { agent, .. }
            | Event::Blocked { agent, .. }
            | Event::FromCache { agent, .. }
            | Event::Sensitive { agent, .. }
            | Event::Merged { agent, .. }
            | Event::Failed { agent }
            | Event::Approved { agent, .. }
            | Event::Rejected { agent }
            | Event::Decided { agent, .. }
            | Event::TurnYes { agent }
            | Event::TurnYesEnded { agent, .. }
            | Event::Replied { agent, .. }
            | Event::Stopped { agent, .. }
            | Event::Retrying { agent, .. }
            | Event::Interrupted { agent }
            | Event::Posted { agent, .. }
            | Event::Claimed { agent, .. }
            | Event::Hired { agent, .. }
            | Event::Forked { agent, .. }
            | Event::Left { agent }
            | Event::Forgotten { agent }
            | Event::Chose { agent, .. }
            | Event::Fallback { agent, .. }
            | Event::Resumed { agent }
            | Event::Restarted { agent, .. }
            | Event::Pushed { agent, .. }
            | Event::PushFailed { agent, .. }
            | Event::SetupRan { agent, .. }
            | Event::SetupFailed { agent, .. }
            | Event::SetupAnswered { agent, .. }
            | Event::Checked { agent, .. }
            | Event::CheckFailed { agent, .. }
            | Event::CheckAnswered { agent, .. }
            | Event::Strayed { agent, .. }
            | Event::Handed { agent, .. }
            | Event::Held { agent, .. }
            | Event::Released { agent, .. }
            | Event::ChecksSet { agent, .. }
            | Event::Withdrawn { agent, .. }
            | Event::Health { agent, .. }
            | Event::Killed { agent, .. }
            | Event::CardFailed { agent, .. }
            | Event::LateDone { agent, .. }
            | Event::Capped { agent, .. }
            | Event::CapAnswered { agent, .. }
            | Event::Planned { leader: agent, .. }
            | Event::PlanAnswered { leader: agent, .. }
            | Event::CommandsMissing { agent }
            | Event::WorktreeMoved { agent, .. }
            | Event::WorktreeKept { agent, .. }
            | Event::Copied { agent, .. }
            | Event::CopySkipped { agent, .. }
            | Event::IncludeRefused { agent, .. }
            | Event::ChangeReady { agent, .. }
            | Event::ChangeCost { agent, .. }
            | Event::Reviewed { agent, .. }
            | Event::ReviewFailed { agent, .. }
            | Event::Applied { agent, .. }
            | Event::Landed { agent, .. }
            | Event::Verified { agent, .. }
            | Event::ApplyFailed { agent, .. } => agent,
            Event::Talk { from, .. } | Event::Sent { from, .. } | Event::ToYou { from, .. } => from,
            Event::ResearchOpened { researcher, .. }
            | Event::ResearchDelivered { researcher, .. }
            | Event::ResearchNeedsYou { researcher, .. } => researcher,
            Event::YouSaid { to, .. } => to,
        }
    }
}

/// How a command of the person's own ended: its exit code, or the signal
/// that stopped it.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Ended {
    Exit(i32),
    Signal(i32),
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Entry {
    pub tick: u64,
    pub event: Event,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum InboxKind {
    /// A change to dependencies, CI, build scripts or network settings.
    Sensitive,
    /// A research report that proposes something sensitive.
    Research,
    /// An agent answered the person and waits for them; the detail is the
    /// answer's first line.
    Reply,
    /// A live worker finished and its worktree holds a change: the person
    /// looks at the diff, applies it to their checkout or rejects it. The
    /// detail is the done message's first line.
    Change,
    /// A live agent asked the person over the bus and waits for their
    /// answer; the detail is the question's first line. Closes when the
    /// person writes back to that agent.
    Question,
    /// A live agent's session waits for the person's yes before a tool
    /// call (a command, or the tool and what it touches, in the detail);
    /// the person allows or refuses it. Closes with the decision or when
    /// the session ends.
    Approval,
    /// A live agent's turn failed on a API error only the person mends
    /// (the account: signed out, a key, a quota, a bill), or on a passing
    /// one imece tried again until its tries ran out; the detail is the
    /// Claude Code's words. Closes when the person writes to the agent.
    Fault,
    /// The checkout's worktree setup script (record 92) waits for the
    /// person's yes to its exact bytes before it runs in a new worktree;
    /// the agent is the one whose worktree it would run in first, the
    /// detail the script's path in the checkout. Closes with the answer,
    /// and at the next start.
    Script,
    /// The repository's check (`.imece/check`, record 112) waits for the
    /// person's yes to its exact bytes before it runs on a change; the
    /// agent is the worker whose change went unchecked first, the detail
    /// the script's path in the checkout. Closes with the answer, and at
    /// the next start. Its worker waits for nothing: its mail goes on.
    Check,
    /// The leader's plan (record 114) waits for the person's yes before
    /// a second worker is hired; the agent is the leader, the detail the
    /// first card's title. Closes with the answer, or with the person's
    /// word to the leader.
    Plan,
    /// A failed card's tries are out (record 126, F9): the leader may not
    /// hand it again unless the person allows one more try, or they fail
    /// it for good. The agent is the leader, the card the item's, the
    /// detail its title. The leader's mail goes on.
    Tries,
    /// A card's change came back to work as often as it may by itself:
    /// its worker is not woken unless the person allows one more round,
    /// or they fail the card. The agent is the worker, the card the
    /// item's, the detail its title. Its mail waits.
    Rounds,
}

impl InboxKind {
    /// Whether an open item of this kind means its agent waits on the
    /// person, so that its mail waits too (record 107). A [`Reply`] does
    /// not, it is only to be read; nor does a [`Change`]: its worker is
    /// finished and needs no answer to go on, and a team must not move at
    /// the speed of the review.
    ///
    /// [`Reply`]: InboxKind::Reply
    /// [`Change`]: InboxKind::Change
    pub fn holds_mail(self) -> bool {
        !matches!(
            self,
            InboxKind::Reply | InboxKind::Change | InboxKind::Check | InboxKind::Tries
        )
    }

    /// Whether the person's word to its agent closes an item of this kind
    /// ([`World::said`]).
    pub fn closes_on_word(self) -> bool {
        matches!(
            self,
            InboxKind::Reply | InboxKind::Question | InboxKind::Fault | InboxKind::Plan
        )
    }
}

/// What the person cleared off the salon's screen (`:clear`, record 90):
/// nothing is deleted, and a card's screen, `:export`, the archives and
/// `W` still see it all.
#[derive(Clone, Debug, Default, PartialEq, Eq, Hash)]
pub struct Cleared {
    /// The journal lines before this index are not listed, but for the
    /// marked ones.
    pub journal: usize,
    /// The inbox items not listed in TO DECIDE; while open they are
    /// counted in its title, and `W` brings one back as it opens it.
    pub inbox: BTreeSet<usize>,
}

#[derive(Clone, Debug)]
pub struct InboxItem {
    pub kind: InboxKind,
    /// The agent that waits for the answer.
    pub agent: AgentId,
    /// What the change does, or the report's topic.
    pub detail: String,
    pub researcher: Option<AgentId>,
    pub open: bool,
    pub tick: u64,
    /// The card the agent worked on when the item came, for the card's
    /// screen: a worker's change stays tied to the card it finished even
    /// after the worker took the next one.
    pub card: Option<CardId>,
}

/// One file of a [`Change`].
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ChangedFile {
    pub path: String,
    /// Lines added and removed; none for a binary file.
    pub added: u32,
    pub removed: u32,
    pub binary: bool,
}

/// What a worker changed in its worktree, as the person sees it: the
/// patch, file by file, with what the worker said it did.
#[derive(Clone, Debug)]
pub struct Change {
    pub agent: AgentId,
    /// The done message's first line.
    pub title: String,
    pub files: Vec<ChangedFile>,
    /// The unified diff, as `git diff` wrote it.
    pub patch: String,
    /// imece's `run`s on exactly the tree the change makes, the last of
    /// each command, oldest first: what says it was built or tested as it
    /// is. A Claude Code's own shell does not count; imece can not see what
    /// that ran on, nor always how it ended.
    pub runs: Vec<Ran>,
    /// The repository's own check on this change (`.imece/check`, record
    /// 112), run by imece in the worktree once the change was read.
    pub check: Check,
    /// What the worker spent on it from its first prompt after the last
    /// change to the `done` (record 113); none until its turn ended.
    pub spend: Option<Spend>,
}

/// How many answers an ask to the person may offer, and how long each
/// may be (record 125): one key picks one.
pub const OPTIONS_MAX: usize = 4;
pub const OPTION_MAX: usize = 60;

/// An agent's open question to the person, whole, with the answers it
/// offered.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Question<'a> {
    pub text: &'a str,
    pub options: &'a [String],
}

/// One thing that proved a change as it is (record 125's verdict): the
/// repository's check, or a run on exactly its tree.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Proof {
    Check { ok: bool, end: String },
    Run(Ran),
}

impl Proof {
    pub fn ok(&self) -> bool {
        match self {
            Proof::Check { ok, .. } => *ok,
            Proof::Run(r) => r.ok(),
        }
    }
}

/// Another change waiting that touches a file this one touches.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Overlap {
    pub with: Other,
    /// The files both touch, in this change's order.
    pub files: Vec<String>,
}

/// What a change overlaps: another change waiting in the project, or a
/// proposal waiting in the project's Lonca repository that no change here
/// shows (someone else's, or a worker's whose turn is not over).
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Other {
    Change {
        agent: AgentId,
        /// The card that change was made for.
        card: Option<CardId>,
    },
    Proposal {
        seq: u64,
        /// The agent's name, as Lonca has it.
        name: String,
    },
}

/// The Δ gate in one line (record 125): whether a change is ready to
/// approve, and the facts that say so. Built by [`World::verdict`] from
/// what the change, its card, the other changes waiting and the store
/// say; worded at draw time.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Verdict {
    /// Proved clean, inside its card's paths, overlapping nothing, fitting
    /// the checkout, shown whole.
    pub ready: bool,
    /// The check when it ran, then each run on the tree, oldest first;
    /// empty when nothing ran.
    pub proof: Vec<Proof>,
    pub files: usize,
    /// The card whose paths it is judged by, when it owns any.
    pub card: Option<CardId>,
    /// The files outside those paths.
    pub outside: Vec<String>,
    /// Other changes waiting in the project.
    pub others: usize,
    pub overlaps: Vec<Overlap>,
    /// It does not fit the person's checkout (record 108).
    pub clashes: bool,
    /// The patch shown is not the whole change: Lonca redacted or cut it.
    pub partial: bool,
    /// A proposal based before trunk as it stands: Lonca's queue merges it
    /// after the yes.
    pub stale: bool,
    /// What Lonca runs after the person's yes (record 122); none off
    /// Lonca.
    pub verifies: Option<Runs>,
    /// The checks its card asks for that `verifies` does not list (record
    /// 127): the yes asks Lonca to run them too. None off Lonca.
    pub asks: Vec<String>,
    /// The checks its card asks for that the policy in force no longer
    /// defines (#390: a newer policy was signed after the card was
    /// written): Lonca would refuse the yes, so it is not ready. None
    /// while Lonca has not said what the policy has.
    pub unknown: Vec<String>,
}

/// What Lonca's merge queue runs on a proposal after the person's yes, as
/// the store says it (`runs`, record 122): its words, `build`, `tests`,
/// `extended`, then each named check the change owes.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub enum Runs {
    These(Vec<String>),
    /// Lonca did not say: a policy it can not read, or a daemon from
    /// before named checks. Nothing promises a check then.
    #[default]
    Unsaid,
}

impl From<Option<Vec<String>>> for Runs {
    fn from(said: Option<Vec<String>>) -> Runs {
        said.map_or(Runs::Unsaid, Runs::These)
    }
}

/// What became of a proposal, as the store says it (record 120).
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Landed {
    /// A human approved it and it waits in the merge queue.
    Queued,
    /// On trunk, as this commit.
    Integrated(String),
    /// The queue could not merge these symbols.
    Conflicted(Vec<String>),
    /// A human or the verifier said no: the reasons.
    Rejected(Vec<String>),
    /// Its worker took it back.
    Withdrawn,
    /// Its worker proposed again: Lonca withdrew it for the newer one,
    /// which names it (`replaces:`, Lonca 0.2.1).
    Replaced,
}

/// One check Lonca's merge queue ran on a proposal.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Attested {
    /// Lonca's word for it.
    pub check: String,
    pub ok: bool,
    /// How it ended (`exit 0, 41.2 s`); empty for a check with no command.
    pub end: String,
    /// The last lines of a failed one's output that say something.
    pub lines: Vec<String>,
}

/// A worker's change as Lonca holds it (record 120): what `a` approves
/// and `r` rejects. The change itself is read from the store and shown as
/// any other [`Change`]; this says which one it is and what is apart.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct StoreProposal {
    /// The number `approve` and `reject` name.
    pub seq: u64,
    /// The change's id in the store.
    pub change: String,
    /// Why it waits, as the policy said it.
    pub why: Vec<String>,
    /// Files its patch leaves out: a blob of theirs was redacted.
    pub redacted: Vec<String>,
    /// The daemon cut its patch.
    pub cut: bool,
    /// What Lonca's queue runs on it after the person's yes (record 122).
    pub verifies: Runs,
    /// Its base is trunk as the last look saw it; a stale one is merged
    /// by the queue after the yes (record 125, step 4).
    pub on_trunk: bool,
}

/// Where the repository's check (`.imece/check`, record 112) stands on a
/// change: a note for the person, never a gate.
#[derive(Clone, Debug, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Check {
    /// No check: the repository has none, or the person said no to it
    /// this run.
    #[default]
    None,
    /// The script waits for the person's yes to its bytes (an item in
    /// TO DECIDE).
    Asked,
    /// It runs in the worktree now.
    Running,
    /// It ran: how it ended.
    Ran(Checked),
    /// It could not be read or started: why.
    Failed(String),
}

/// How the repository's check ended on a change.
#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Checked {
    /// Exit 0, not killed.
    pub ok: bool,
    /// How it ended, in words (`exit 0, 41.2 s`).
    pub end: String,
    /// The last lines of its output that say something, cleaned.
    pub lines: Vec<String>,
}

/// A command imece ran for an agent, and how it ended.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Ran {
    /// The command line as the agent gave it.
    pub line: String,
    /// The exit code; none when a signal ended it.
    pub code: Option<i32>,
    /// Whether the deadline ended it.
    pub timed_out: bool,
}

impl Ran {
    pub fn ok(&self) -> bool {
        self.code == Some(0)
    }
}

/// What a fresh Claude Code session said of a worker's change (`:review`,
/// record 106): advisory, never a gate. Kept while the change it read
/// waits; a newer change of the worker is not the one it read.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Review {
    /// The patch it read, as [`patch_sum`] gives it.
    pub patch: String,
    pub model: String,
    pub text: String,
    /// Ten-thousandths of a dollar, from the pot.
    pub cost: u64,
}

/// A patch's name that stays the same from run to run: the first 16 hex
/// digits of its SHA-256. A review names the patch it read by it.
pub fn patch_sum(patch: &str) -> String {
    let mut sum = crate::sha::hex(patch.as_bytes());
    sum.truncate(16);
    sum
}

impl Change {
    /// What was proved on the tree the change makes: the repository's
    /// check when it ran, else imece's runs; true when every one ended
    /// clean, false when one did not, none when nothing ran.
    pub fn proof(&self) -> Option<bool> {
        match &self.check {
            Check::Ran(d) => Some(d.ok),
            _ if self.runs.is_empty() => None,
            _ => Some(self.runs.iter().all(Ran::ok)),
        }
    }

    /// The files the change touches outside the paths `card` owns
    /// (record 114); none without a card, or when it owns no paths.
    pub fn outside<'a>(&'a self, card: Option<&Card>) -> Vec<&'a str> {
        let Some(card) = card else {
            return Vec::new();
        };
        (self.files.iter())
            .filter(|f| crate::paths::strays(&card.paths, &f.path))
            .map(|f| f.path.as_str())
            .collect()
    }

    /// Easy to approve: proved clean and inside its card's paths.
    pub fn easy(&self, card: Option<&Card>) -> bool {
        self.proof() == Some(true) && self.outside(card).is_empty()
    }

    /// What proved it, each with how it ended: the check when it ran,
    /// then the runs.
    pub fn proofs(&self) -> Vec<Proof> {
        let mut out = Vec::new();
        if let Check::Ran(d) = &self.check {
            out.push(Proof::Check {
                ok: d.ok,
                end: d.end.clone(),
            });
        }
        out.extend(self.runs.iter().cloned().map(Proof::Run));
        out
    }

    /// The files both this change and `other` touch, in this one's order.
    pub fn shared_files(&self, other: &Change) -> Vec<String> {
        self.shared_paths(other.files.iter().map(|f| f.path.as_str()))
    }

    /// The files this change touches that are among `paths`, in this
    /// change's order.
    pub fn shared_paths<'a>(
        &self,
        paths: impl IntoIterator<Item = &'a str> + Clone,
    ) -> Vec<String> {
        (self.files.iter())
            .filter(|f| paths.clone().into_iter().any(|o| o == f.path))
            .map(|f| f.path.clone())
            .collect()
    }

    /// Lines added and removed over all files.
    pub fn counts(&self) -> (u32, u32) {
        self.files
            .iter()
            .fold((0, 0), |(a, r), f| (a + f.added, r + f.removed))
    }
}

/// A request from the person that a leader is breaking into cards.
#[derive(Clone, Debug)]
pub struct Plan {
    pub project: ProjectId,
    pub text: String,
    pub ready: u64,
}

/// One card of a leader's plan (record 114): what it means to assign or
/// post, on which tier, owning which paths.
#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Planned {
    pub title: String,
    /// The tier's wire word: `cheap`, `standard` or `strong`.
    pub tier: String,
    pub paths: Vec<Owned>,
    /// The checks it will ask for (record 127).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub checks: Vec<String>,
    pub body: String,
    /// The cards of the same plan it waits on (record 126, F10), by their
    /// place in it from 1, each an earlier one.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub after: Vec<usize>,
}

/// A leader's plan as the person reads it before a second worker is
/// hired (record 114): the cards, what the leader expects them to cost
/// (ten-thousandths of a dollar) and when it came.
#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Proposal {
    pub cards: Vec<Planned>,
    pub cost: u64,
    pub tick: u64,
}

/// A project's plan gate (record 114): on by default; `approved` while
/// the person's last answer to a plan was yes (a new plan takes it back);
/// the plan itself while one waits or was approved.
#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct PlanGate {
    pub on: bool,
    pub approved: bool,
    pub proposal: Option<Proposal>,
}

impl Default for PlanGate {
    fn default() -> PlanGate {
        PlanGate {
            on: true,
            approved: false,
            proposal: None,
        }
    }
}

/// Why the leader may not hire a second worker now (record 114).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PlanHold {
    /// No plan was made yet.
    NoPlan,
    /// The plan waits for the person in TO DECIDE.
    Reading,
    /// The person answered the plan in words, not with a yes.
    Unanswered,
}

/// What the team spent lately, for the rate: the total each time it
/// changed, oldest first, back to the start of [`RATE_WINDOW`]; and
/// whether the journal said the pot runs low.
#[derive(Clone, Debug, Default)]
pub struct Spending {
    pub seen: VecDeque<(u64, u64)>,
    pub warned: bool,
}

pub struct World {
    pub tick: u64,
    /// When the world began, in seconds since the Unix epoch: tick 0. A
    /// world remembered from an earlier run keeps its first start, so ticks
    /// stay monotonic across runs; the simulation has none (0).
    pub started_at: u64,
    /// The simulation's world (`--demo`, `sim.rs`): its agents go through
    /// made-up progress phases (`Phase`); a live agent has none.
    pub demo: bool,
    /// What was spent when this run of imece began, for the rate of this
    /// run alone.
    pub run_spent: u64,
    pub spending: Spending,
    pub projects: Vec<Project>,
    pub agents: Vec<Agent>,
    pub cards: Vec<Card>,
    pub journal: Vec<Entry>,
    pub inbox: Vec<InboxItem>,
    /// What all agents together may spend, in ten-thousandths of a dollar;
    /// one pot, kept between runs.
    pub budget: u64,
    /// Claude Code is signed in through a plan (record 132): its figures
    /// are API-equivalent, said with `≈`. Memory only, from each start's
    /// look.
    pub estimated: bool,
    /// Researchers that may run at once, across all projects.
    pub max_researchers: usize,
    /// Blocked workers waiting for a free researcher slot.
    pub research_queue: VecDeque<AgentId>,
    pub plans: Vec<Plan>,
    /// Workers' changes waiting for the person, one per worker at most.
    pub changes: Vec<Change>,
    /// The journal lines the person marked to come back to (`m`), by index.
    pub marks: BTreeSet<usize>,
    /// What the person cleared off the salon's screen (`:clear`).
    pub cleared: Cleared,
    /// The worktree setup scripts the person said yes to, each by what
    /// runs (`setup::Script::key`: how, the bytes' hash, the real path),
    /// oldest first, at most [`SETUP_KEPT`].
    pub setup_approved: Vec<String>,
    /// What Claude Code last said of the plan's usage windows, one entry
    /// per window. Not kept between runs: a session
    /// says it again.
    pub limits: Vec<Limit>,
    /// What imece's own paid calls cost (the suggested commit messages),
    /// in ten-thousandths of a dollar: part of the pot, beside what the
    /// agents spent. A call counts at its cap from before it is made
    /// until it says what it cost.
    pub paid: u64,
    /// A commit message suggested for a project's staged changes, until
    /// the screen puts it on the command line.
    pub suggested: Option<Suggestion>,
    /// What a fresh session said of each waiting change, by worker
    /// (`:review`): only of the change that waits now.
    pub reviews: HashMap<AgentId, Review>,
    /// What of a worker's waiting change did not fit the checkout, when
    /// git last refused it (record 108). Memory only: `a` finds it again.
    pub clashes: HashMap<AgentId, Clashes>,
    /// Which proposal in Lonca a worker's waiting change is (record 120).
    /// Memory only: the next look finds it again.
    pub proposals: HashMap<AgentId, StoreProposal>,
    /// The worktrees no agent owns, as the engine last looked (`:tidy`).
    pub orphans: Orphans,
    /// When imece tells a leader of a stall, an idle worker or a loop
    /// (`:health`, record 128); prefs keep it, the world does not.
    pub health: crate::health::Health,
    /// The sessions' processes, as the engine last read them (`:ps`).
    pub procs: crate::procs::Procs,
    /// The number the last message on the bus was given ([`MsgId`]).
    pub messages: MsgId,
    /// The lines the person sent, oldest first, for `↑` `↓` and `^r` on
    /// the command line; kept between runs (the newest [`SENT_KEEP`]).
    pub sent: Vec<Sent>,
    /// The cost ledger (record 133): one row per attempt at a card,
    /// oldest first; kept in the store.
    pub ledger: Vec<crate::ledger::Attempt>,
    /// Claude Code's version as last asked (record 75), for the ledger's
    /// rows. Memory only.
    pub version: Option<String>,
}

/// A line the person sent: a message, a `:` command or a `!cmd`, as it
/// was typed, and when.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Sent {
    pub tick: u64,
    pub text: String,
}

/// How many sent lines a load brings back and a tidy keeps.
pub const SENT_KEEP: usize = 500;

/// A commit message a cheap model suggested for what is staged in a
/// project's checkout (`:commit`), cleaned.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Suggestion {
    pub project: ProjectId,
    pub text: String,
}

/// A worktree under imece's state that no agent owns (`:tidy`, record
/// 96): left by a crash, a store removed or a fork never started.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Orphan {
    /// Its directory's name: the agent's it was made for.
    pub name: String,
    pub path: std::path::PathBuf,
    /// The checkout it was added to.
    pub checkout: std::path::PathBuf,
    /// Files changed or not tracked in it.
    pub changes: usize,
    /// Commits of its own no branch, tag or remote has.
    pub commits: usize,
    /// A Lonca workspace `lonca diff` can not count: it has changes and its
    /// session is over. It holds work, of a size nobody can say.
    pub unread: bool,
}

impl Orphan {
    /// Whether removing it loses nothing.
    pub fn clean(&self) -> bool {
        !self.unread && self.changes == 0 && self.commits == 0
    }
}

/// Why a workspace was not removed without the person's yes.
pub const WORKSPACE_MAY_HOLD: &str = "it may hold work: its session is over or it has changes";

/// What the engine knows of the worktrees no agent owns.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Orphans {
    /// What the last look found, by path; none before one answered.
    pub found: Option<Vec<Orphan>>,
    /// A look or a removal is under way.
    pub busy: bool,
    /// What the last removal came to, until the screen says it.
    pub tidied: Option<Tidied>,
}

/// What a removal came to: the names removed, and the ones not with
/// why: git's words, or none when an agent owns it again; and the
/// teardown script before each, where its checkout has one.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Tidied {
    pub removed: Vec<String>,
    pub kept: Vec<(String, Option<String>)>,
    pub torn: Vec<(String, TornDown)>,
}

/// How a worktree's teardown script went before its removal.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum TornDown {
    /// It ran: whether it went well, how it ended (`exit 0, 0.4 s`), its
    /// last line that says something, cleaned.
    Ran {
        ok: bool,
        end: String,
        last: Option<String>,
    },
    /// It could not be read or started: why.
    Failed(String),
    /// The person said no to it this run.
    Refused,
}

/// One window of the plan's usage limit, as Claude Code last said it
/// (`imece_agent::Limit`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Limit {
    /// Its length in minutes (300: five hours).
    pub minutes: u32,
    /// Percent used, at most 100.
    pub used: u8,
    /// When it starts again, in seconds since the Unix epoch (the world's
    /// seconds, `World::now`).
    pub resets_at: Option<u64>,
}

impl World {
    /// Now as the world counts it, in seconds since the Unix epoch: its
    /// first start and the ticks since (the demo starts at 0).
    pub fn now(&self) -> u64 {
        self.started_at + self.tick * SECONDS_PER_TICK
    }

    /// The files of project `p` a message may name: the code map's where
    /// it has one (the demo), else what git listed.
    pub fn files_of(&self, p: ProjectId) -> Vec<&str> {
        let project = &self.projects[p];
        if project.repo.is_empty() {
            project.files.iter().map(String::as_str).collect()
        } else {
            project.repo.iter().map(|(f, _)| *f).collect()
        }
    }

    /// The person sent `text` now: kept for recall, unless it is the last
    /// line again (a shell keeps one of a run of the same line).
    pub fn recall_line(&mut self, text: &str) {
        if self.sent.last().is_some_and(|s| s.text == text) {
            return;
        }
        self.sent.push(Sent {
            tick: self.tick,
            text: text.to_string(),
        });
    }

    /// Claude Code said how much of a window is used: that window is as it
    /// says now; the others stay as they were.
    pub fn limit(&mut self, l: Limit) {
        match self.limits.iter_mut().find(|k| k.minutes == l.minutes) {
            Some(k) => *k = l,
            None => self.limits.push(l),
        }
    }

    /// The window nearest to full: the most used, the shorter one on a tie.
    pub fn tightest(&self) -> Option<Limit> {
        (self.limits.iter())
            .max_by_key(|l| (l.used, std::cmp::Reverse(l.minutes)))
            .copied()
    }

    /// The agent called `name` that can be reached: not gone, and not in
    /// a project closed or archived (an archived one's agents keep their
    /// names, and a fresh project's may have the same).
    pub fn agent_by_name(&self, name: &str) -> Option<AgentId> {
        self.agents
            .iter()
            .find(|a| !a.gone && a.name == name && !self.put_away(a.id))
            .map(|a| a.id)
    }

    /// Whether an agent that is not gone has the name `name`, an archived
    /// one's too: a new agent's name (and so its worktree's) is never one
    /// of these.
    pub fn name_taken(&self, name: &str) -> bool {
        self.agents.iter().any(|a| !a.gone && a.name == name)
    }

    /// Marks journal line `i` to come back to, or takes its mark away;
    /// says whether it is marked now. A line the journal does not have
    /// stays unmarked.
    pub fn mark(&mut self, i: usize) -> bool {
        if i >= self.journal.len() {
            return false;
        }
        if self.marks.remove(&i) {
            return false;
        }
        self.marks.insert(i)
    }

    /// What the pot has paid: what every agent spent and imece's own
    /// calls.
    pub fn spent(&self) -> u64 {
        self.agents.iter().map(|a| a.cost).sum::<u64>() + self.paid
    }

    /// Whether some of what the pot paid was never priced (record 132):
    /// [`World::spent`] is then what it paid at least.
    pub fn unpriced(&self) -> bool {
        self.agents.iter().any(|a| a.unpriced.any())
    }

    /// A figure of Claude Code's as the person reads it (record 132): `≈`
    /// first on a plan's login, whose figures are API-equivalent; `+?`
    /// after it when part of it was never priced, `?` alone when nothing
    /// of it was. Never `$0.00` for what is not known.
    pub fn money(&self, cost: u64, unpriced: bool) -> String {
        money(cost, unpriced, self.estimated)
    }

    /// What agent `id` spent, as [`World::money`] says it.
    pub fn spent_by(&self, id: AgentId) -> String {
        let a = &self.agents[id];
        self.money(a.cost, a.unpriced.any())
    }

    /// What the agents of project `p` spent, as [`World::money`] says it.
    pub fn project_spent(&self, p: ProjectId) -> String {
        let of = || self.agents.iter().filter(|a| a.project == p);
        let cost = of().map(|a| a.cost).sum();
        self.money(cost, of().any(|a| a.unpriced.any()))
    }

    /// Every agent's [`Unpriced::count`] together.
    pub fn unpriced_count(&self) -> u32 {
        self.agents.iter().map(|a| a.unpriced.count()).sum()
    }

    /// What the pot paid, as [`World::money`] says it.
    pub fn spent_words(&self) -> String {
        self.money(self.spent(), self.unpriced())
    }

    /// Agent `id`'s turn about to end did not say all it cost (record
    /// 132): `model` priced at nothing though it used tokens, or no figure
    /// at all. Its end ([`World::turn_priced`]) takes it.
    pub fn turn_unpriced(&mut self, id: AgentId, model: Option<&str>) {
        let t = self.tick;
        let a = &mut self.agents[id];
        a.unpriced.said = true;
        a.trace(
            t,
            match model {
                Some(m) => format!("cost unknown: {m} used tokens priced at nothing"),
                None => "cost unknown: the turn's result said none".to_string(),
            },
        );
    }

    /// Agent `id`'s turn ended with its figure booked: the session's
    /// figure is its whole, so priced now, what an earlier turn of it left
    /// unpriced is too; else the session stays unpriced (record 132).
    pub fn turn_priced(&mut self, id: AgentId) {
        let u = &mut self.agents[id].unpriced;
        u.session = std::mem::take(&mut u.said);
    }

    /// Agent `id`'s session is over (record 132); `cut` when it ended
    /// mid-turn, whose spending no figure will say. What it left unpriced
    /// becomes a gap for good.
    pub fn session_over(&mut self, id: AgentId, cut: bool) {
        let u = &mut self.agents[id].unpriced;
        if cut || u.session || u.said {
            u.gaps += 1;
        }
        u.session = false;
        u.said = false;
        // The gap goes on the row it was spent on (record 133).
        self.book(id);
    }

    /// The projects that are open, in id order.
    pub fn open_projects(&self) -> impl Iterator<Item = &Project> {
        self.projects.iter().filter(|p| !p.closed)
    }

    /// The open projects in the salon's order: the pinned ones first,
    /// each group in the order they were opened.
    pub fn salon_order(&self) -> Vec<ProjectId> {
        let (mut pinned, rest): (Vec<ProjectId>, Vec<ProjectId>) = (self.open_projects())
            .map(|p| p.id)
            .partition(|&p| self.projects[p].pinned);
        pinned.extend(rest);
        pinned
    }

    /// The open project called `name`, exactly.
    pub fn project_by_name(&self, name: &str) -> Option<ProjectId> {
        self.open_projects().find(|p| p.name == name).map(|p| p.id)
    }

    /// Agent `id` leaves the team: idle, nothing shown, gone; its name may
    /// be taken again.
    pub fn leave(&mut self, id: AgentId) {
        let t = self.tick;
        let a = &mut self.agents[id];
        a.activity = None;
        a.talk = None;
        a.plan.clear();
        a.context = None;
        a.set_status(Status::Idle, t);
        a.gone = true;
        a.trace(t, "left");
    }

    /// What the head of project `project`, named `name`, is called: `lead`
    /// for a leader (`solo` for a solo agent) in the project imece started
    /// in, `lead-NAME` (`solo-NAME`) elsewhere.
    pub fn head_name(project: ProjectId, name: &str, role: Role) -> String {
        let stem = World::head_stem(role);
        if project == 0 {
            stem.into()
        } else {
            format!("{stem}-{name}")
        }
    }

    /// `lead` for a leader, `solo` for a solo agent: the first word of a
    /// project head's name.
    pub fn head_stem(role: Role) -> &'static str {
        if role == Role::Solo { "solo" } else { "lead" }
    }

    /// Whether project `p` is a solo session: its head is the agent the
    /// person drives, not a leader.
    pub fn is_solo(&self, p: ProjectId) -> bool {
        self.agents[self.projects[p].leader].role == Role::Solo
    }

    /// Project `reuse` opens again, else a new project is added, named
    /// `name`, its checkout at `path`, led by a new agent with the next
    /// id. Returns the project's id.
    pub fn open_project(
        &mut self,
        reuse: Option<ProjectId>,
        name: String,
        path: String,
        role: Role,
    ) -> ProjectId {
        let p = self.add_project(reuse, name, role);
        let leader = self.projects[p].leader;
        self.note(Event::Opened { leader, path });
        p
    }

    /// [`open_project`](Self::open_project) without the journal's line.
    fn add_project(&mut self, reuse: Option<ProjectId>, name: String, role: Role) -> ProjectId {
        let p = reuse.unwrap_or(self.projects.len());
        let leader = self.agents.len();
        let lead = World::head_name(p, &name, role);
        self.agents
            .push(Agent::new(leader, p, lead, role, Status::Idle, self.tick));
        match reuse {
            Some(p) => {
                let project = &mut self.projects[p];
                project.name = name;
                project.leader = leader;
                project.closed = false;
            }
            None => self.projects.push(Project {
                id: p,
                name,
                leader,
                trunk: None,
                files: Vec::new(),
                repo: &[],
                closed: false,
                pinned: false,
                copies: Vec::new(),
                allowed: Vec::new(),
                plan: PlanGate::default(),
                no_git: false,
                lonca: None,
                archived: None,
            }),
        }
        p
    }

    /// Project `p`'s folder is a git checkout with a first commit (`None`)
    /// or it is not (the folder's path): then its solo works straight in
    /// the folder, which the journal says.
    pub fn set_no_git(&mut self, p: ProjectId, folder: Option<&str>) {
        self.projects[p].no_git = folder.is_some();
        if let Some(path) = folder {
            let leader = self.projects[p].leader;
            let path = path.to_string();
            self.note(Event::NoGit { leader, path });
        }
    }

    /// The person names Lonca repository `repo` for project `p`, or none:
    /// nothing is known of it until the next look.
    pub fn on_lonca(&mut self, p: ProjectId, repo: Option<String>) {
        self.projects[p].lonca = repo.map(|repo| OnLonca {
            repo,
            seen: StoreSeen::Asking,
            leases: Vec::new(),
            pending: Vec::new(),
            policy_checks: None,
        });
    }

    /// The person signed in as `human`, asked from project `p`.
    pub fn store_signed_in(&mut self, p: ProjectId, human: String) {
        let leader = self.projects[p].leader;
        self.note(Event::StoreSignedIn { leader, human });
    }

    /// The import asked went through: the project is on the repository,
    /// unless the person named another while it ran (the repository still
    /// exists, and the line says so).
    pub fn store_imported(&mut self, ask: &ImportAsk, warnings: Vec<String>, hardened: bool) {
        let p = ask.project;
        if self.projects[p].lonca.is_none() {
            self.on_lonca(p, Some(ask.repo.clone()));
        }
        let leader = self.projects[p].leader;
        let repo = ask.repo.clone();
        self.note(Event::StoreImported {
            leader,
            repo,
            warnings,
            hardened,
        });
    }

    /// The repository `ask` made still refuses critical paths, for `why`.
    pub fn store_critical_kept(&mut self, ask: &ImportAsk, why: Option<String>) {
        let leader = self.projects[ask.project].leader;
        let repo = ask.repo.clone();
        self.note(Event::StoreCriticalKept { leader, repo, why });
    }

    /// `:sync` of `repo` did what `how` says to project `p`'s checkout.
    pub fn store_synced(&mut self, p: ProjectId, repo: &str, how: Synced) {
        let leader = self.projects[p].leader;
        let repo = repo.to_string();
        self.note(Event::StoreSynced { leader, repo, how });
    }

    /// `job` did not go through, and `why` is what it said.
    pub fn store_failed(&mut self, job: &StoreJob, why: Option<String>) {
        let leader = self.projects[job.project()].leader;
        let act = job.act();
        self.note(Event::StoreFailed { leader, act, why });
    }

    /// What the daemon said of repository `repo` for project `p`, unless
    /// the person named another since. A store that goes down is said
    /// once, with its reason, and so is one that comes back. Whether the
    /// screen changed.
    pub fn store_seen(&mut self, p: ProjectId, repo: &str, seen: StoreSeen) -> bool {
        let leader = self.projects[p].leader;
        let Some(on) = self.projects[p].lonca.as_mut().filter(|l| l.repo == repo) else {
            return false;
        };
        if on.seen == seen {
            return false;
        }
        let was_down = matches!(on.seen, StoreSeen::Down(_));
        on.seen = seen.clone();
        if matches!(seen, StoreSeen::Down(_)) {
            on.leases.clear();
            on.pending.clear();
            on.policy_checks = None;
        }
        let repo = repo.to_string();
        match seen {
            StoreSeen::Down(why) => self.note(Event::StoreDown { leader, repo, why }),
            StoreSeen::Up { .. } if was_down => self.note(Event::StoreBack { leader, repo }),
            _ => {}
        }
        true
    }

    /// What is held in project `p`'s repository `repo`. Whether it moved.
    pub fn store_leases(&mut self, p: ProjectId, repo: &str, leases: Vec<StoreLease>) -> bool {
        let on = self.projects[p].lonca.as_mut().filter(|l| l.repo == repo);
        let Some(on) = on.filter(|l| l.leases != leases) else {
            return false;
        };
        on.leases = leases;
        true
    }

    /// What waits for a human in project `p`'s repository `repo`. Whether
    /// it moved.
    pub fn store_pending(&mut self, p: ProjectId, repo: &str, pending: Vec<StorePending>) -> bool {
        let on = self.projects[p].lonca.as_mut().filter(|l| l.repo == repo);
        let Some(on) = on.filter(|l| l.pending != pending) else {
            return false;
        };
        on.pending = pending;
        true
    }

    /// The checks the policy in force of project `p`'s repository `repo`
    /// defines, or that Lonca did not say. Whether it moved.
    pub fn store_checks(&mut self, p: ProjectId, repo: &str, checks: Option<Vec<String>>) -> bool {
        let on = self.projects[p].lonca.as_mut().filter(|l| l.repo == repo);
        let Some(on) = on.filter(|l| l.policy_checks != checks) else {
            return false;
        };
        on.policy_checks = checks;
        true
    }

    /// The checks the policy of project `p`'s repository defines; none off
    /// Lonca or while Lonca has not said.
    pub fn policy_checks(&self, p: ProjectId) -> Option<&[String]> {
        self.projects[p].lonca.as_ref()?.policy_checks.as_deref()
    }

    /// Why project `p` is closed to hires and Δ decisions: it is on Lonca
    /// and the store has not answered (record 115: no daemon, no hire and
    /// no decision). None for a project not on Lonca, or one that answers.
    pub fn store_closed(&self, p: ProjectId) -> Option<&'static str> {
        match &self.projects[p].lonca.as_ref()?.seen {
            StoreSeen::Up { .. } => None,
            StoreSeen::Asking => Some("imece is still asking Lonca"),
            StoreSeen::Down(why) => Some(why.why()),
        }
    }

    /// The open projects on Lonca, each with its repository.
    pub fn stores(&self) -> Vec<(ProjectId, String)> {
        self.open_projects()
            .filter_map(|q| Some((q.id, q.lonca.as_ref()?.repo.clone())))
            .collect()
    }

    /// Project `p` closes: every agent of it leaves, what waited of theirs
    /// is closed, and it is not drawn any more.
    pub fn close_project(&mut self, p: ProjectId) {
        let ids: Vec<AgentId> = self
            .agents
            .iter()
            .filter(|a| a.project == p && !a.gone)
            .map(|a| a.id)
            .collect();
        for id in ids {
            self.leave(id);
        }
        for item in &mut self.inbox {
            if self.agents[item.agent].project == p {
                item.open = false;
            }
        }
        self.changes.retain(|c| self.agents[c.agent].project != p);
        self.projects[p].closed = true;
        let leader = self.projects[p].leader;
        self.note(Event::Closed { leader });
    }

    /// Project `p` is archived as `title`: closed, its agents asleep with
    /// all they had (their inbox items, changes and cards wait out of
    /// sight), and a fresh project takes its place: the same kind, name,
    /// pin and copies, its head called the same.
    /// Returns the fresh project's id.
    pub fn archive_project(&mut self, p: ProjectId, title: String) -> ProjectId {
        let was = self.projects[p].leader;
        let (role, head) = (self.agents[was].role, self.agents[was].name.clone());
        self.shelve(p, title.clone());
        let q = &self.projects[p];
        let (name, pinned, copies) = (q.name.clone(), q.pinned, q.copies.clone());
        let lonca = q.lonca.clone();
        let fresh = self.add_project(None, name, role);
        let leader = self.projects[fresh].leader;
        self.agents[leader].name = head;
        self.projects[fresh].pinned = pinned;
        self.projects[fresh].copies = copies;
        self.projects[fresh].lonca = lonca;
        self.note(Event::Archived { leader, was, title });
        fresh
    }

    /// Project `p` is put away as `title`, with no line and nothing in its
    /// place: closed, its agents asleep with all they had.
    pub fn shelve(&mut self, p: ProjectId, title: String) {
        let t = self.tick;
        for a in self.agents.iter_mut() {
            if a.project == p && !a.gone {
                a.activity = None;
                // A failed turn is not tried again in an archive.
                a.retry = None;
                a.tries = 0;
                a.set_status(Status::Asleep, t);
            }
        }
        let q = &mut self.projects[p];
        q.closed = true;
        q.archived = Some(Archived { at: t, title });
    }

    /// The archive `p` comes back (`:restore`): open, its agents still
    /// asleep with all they had, their inbox items, changes and cards in
    /// sight again. `swapped` is the title the project open in its place
    /// was archived as, for the line. Nothing happens to a project that is
    /// no archive.
    pub fn restore_project(&mut self, p: ProjectId, swapped: Option<String>) {
        let Some(archived) = self.projects[p].archived.take() else {
            return;
        };
        self.projects[p].closed = false;
        let leader = self.projects[p].leader;
        self.note(Event::Restored {
            leader,
            title: archived.title,
            swapped,
        });
    }

    /// The archive `:restore ARG` names: the newest one titled `arg`, else
    /// the newest of the project called `arg`; with nothing named, the
    /// newest of the name project `at_hand` has. Newest: archived last,
    /// then the higher id.
    pub fn archive_named(&self, arg: &str, at_hand: ProjectId) -> Option<ProjectId> {
        let newest = |by: &dyn Fn(&Project, &Archived) -> bool| {
            (self.projects.iter())
                .filter_map(|q| {
                    q.archived
                        .as_ref()
                        .filter(|a| by(q, a))
                        .map(|a| (a.at, q.id))
                })
                .max()
                .map(|(_, id)| id)
        };
        if arg.is_empty() {
            let name = &self.projects[at_hand].name;
            return newest(&|q, _| q.name == *name);
        }
        newest(&|_, a| a.title == arg).or_else(|| newest(&|q, _| q.name == arg))
    }

    /// Project `p`'s journal lines (its agents'), oldest first, and which
    /// of them are marked, by their index in what is returned: the demo's
    /// archive view reads them from memory (live reads them from the
    /// store, which keeps more).
    pub fn journal_of(&self, p: ProjectId) -> (Vec<Entry>, BTreeSet<usize>) {
        let mut lines = Vec::new();
        let mut marks = BTreeSet::new();
        for (i, e) in self.journal.iter().enumerate() {
            if self.agents[e.event.agent()].project != p {
                continue;
            }
            if self.marks.contains(&i) {
                marks.insert(lines.len());
            }
            lines.push(e.clone());
        }
        (lines, marks)
    }

    /// The world archive `p`'s read-only view shows: `p` open as it was
    /// when it was put away, with `journal` (its lines, oldest first) and
    /// `marks` (indexes in `journal`); every other project put away as an
    /// archive is, so the salon, the board, the journal, TO DECIDE
    /// and the names hold only `p`'s. Every project, agent and card is
    /// kept (the journal's events name them by id); the pot and the
    /// clock are this world's. Nothing in it runs: no limits, no queue,
    /// no suggestion.
    pub fn view_of(&self, p: ProjectId, journal: Vec<Entry>, marks: BTreeSet<usize>) -> World {
        let mut projects = self.projects.clone();
        for q in &mut projects {
            if q.id == p {
                q.closed = false;
                q.archived = None;
            } else {
                q.closed = true;
                q.archived.get_or_insert(Archived {
                    at: 0,
                    title: String::new(),
                });
            }
        }
        World {
            tick: self.tick,
            started_at: self.started_at,
            demo: self.demo,
            run_spent: self.run_spent,
            spending: self.spending.clone(),
            projects,
            agents: self.agents.clone(),
            cards: self.cards.clone(),
            journal,
            inbox: self.inbox.clone(),
            budget: self.budget,
            estimated: self.estimated,
            max_researchers: self.max_researchers,
            research_queue: VecDeque::new(),
            plans: self.plans.clone(),
            changes: self.changes.clone(),
            marks,
            cleared: Cleared::default(),
            setup_approved: Vec::new(),
            limits: Vec::new(),
            paid: self.paid,
            suggested: None,
            reviews: Default::default(),
            clashes: Default::default(),
            proposals: Default::default(),
            orphans: crate::model::Orphans::default(),
            health: Default::default(),
            procs: Default::default(),
            messages: self.messages,
            sent: self.sent.clone(),
            ledger: self.ledger.clone(),
            version: self.version.clone(),
        }
    }

    /// The archives, newest first (as [`archive_named`](Self::archive_named)
    /// orders them).
    pub fn archives(&self) -> Vec<ProjectId> {
        let mut found: Vec<(u64, ProjectId)> = (self.projects.iter())
            .filter_map(|q| q.archived.as_ref().map(|a| (a.at, q.id)))
            .collect();
        found.sort_unstable_by(|a, b| b.cmp(a));
        found.into_iter().map(|(_, id)| id).collect()
    }

    /// Whether project `p` did anything the world shows: a card, an agent
    /// besides its head, something spent, an inbox item, or a journal
    /// line of its agents other than the person opening, archiving,
    /// bringing back, renaming or closing it (and what imece said at its
    /// opening). A `:restore` closes, rather than archives, the project in
    /// its place that did nothing.
    pub fn did_anything(&self, p: ProjectId) -> bool {
        let head = self.projects[p].leader;
        let of_p = |id: AgentId| self.agents.get(id).is_some_and(|a| a.project == p);
        self.cards.iter().any(|c| c.project == p)
            || (self.agents.iter()).any(|a| a.project == p && (a.id != head || a.cost > 0))
            || self.inbox.iter().any(|i| of_p(i.agent))
            || self.journal.iter().any(|e| {
                of_p(e.event.agent())
                    && !matches!(
                        e.event,
                        Event::Opened { .. }
                            | Event::NoGit { .. }
                            | Event::StoreDown { .. }
                            | Event::StoreBack { .. }
                            | Event::StoreSignedIn { .. }
                            | Event::StoreImported { .. }
                            | Event::StoreCriticalKept { .. }
                            | Event::StoreSynced { .. }
                            | Event::StoreFailed { .. }
                            | Event::Confine { .. }
                            | Event::Archived { .. }
                            | Event::Restored { .. }
                            | Event::Renamed { .. }
                            | Event::Closed { .. }
                    )
            })
    }

    /// What `:archive` calls project `p` when the person gives no title:
    /// its name and the days its oldest journal line and now fall on.
    pub fn archive_title(&self, p: ProjectId) -> String {
        let name = &self.projects[p].name;
        let to = date(day(self.tick));
        let from = (self.journal.iter())
            .find(|e| {
                self.agents
                    .get(e.event.agent())
                    .is_some_and(|a| a.project == p)
            })
            .map(|e| date(day(e.tick)));
        match from {
            Some(from) if from != to => format!("{name} {from} → {to}"),
            _ => format!("{name} {to}"),
        }
    }

    /// Whether agent `id`'s project is archived: its journal lines are out
    /// of sight until it is brought back.
    pub fn in_archive(&self, id: AgentId) -> bool {
        self.agents
            .get(id)
            .and_then(|a| self.projects.get(a.project))
            .is_some_and(|q| q.archived.is_some())
    }

    /// Whether agent `id`'s project is closed or archived: out of sight,
    /// out of reach by name, and acting on nothing.
    pub fn put_away(&self, id: AgentId) -> bool {
        self.agents
            .get(id)
            .and_then(|a| self.projects.get(a.project))
            .is_some_and(|q| q.closed)
    }

    /// Project `p` is called `name` from now on, and its head `lead-NAME`
    /// (`solo-NAME` for a solo session), project 0's too.
    pub fn rename_project(&mut self, p: ProjectId, name: String) {
        let leader = self.projects[p].leader;
        let from = std::mem::replace(&mut self.projects[p].name, name.clone());
        let stem = World::head_stem(self.agents[leader].role);
        self.agents[leader].name = format!("{stem}-{name}");
        self.note(Event::Renamed {
            leader,
            from,
            to: name,
        });
    }

    /// What the last [`RATE_WINDOW`] cost, per hour; a run younger than
    /// that spent nothing before it began.
    pub fn rate_per_hour(&self) -> u64 {
        let from = self.tick.saturating_sub(RATE_WINDOW);
        let then = (self.spending.seen.iter().rev())
            .find(|s| s.0 <= from)
            .map_or(self.run_spent, |s| s.1);
        self.spent().saturating_sub(then) * 3600 / (RATE_WINDOW * SECONDS_PER_TICK)
    }

    /// How many minutes the pot lasts at [`World::rate_per_hour`]; none
    /// when nothing is being spent or the pot is already spent.
    pub fn pot_lasts(&self) -> Option<u64> {
        let rate = self.rate_per_hour();
        let left = self.budget.saturating_sub(self.spent());
        (rate > 0 && left > 0).then(|| left * 60 / rate)
    }

    pub fn pot_low(&self) -> bool {
        self.pot_lasts().is_some_and(|m| m < POT_LOW_MINUTES)
    }

    /// Notes what was spent by now, for the rate, and says once in the
    /// journal when the pot runs low. Called every step.
    pub fn watch_spending(&mut self) {
        let spent = self.spent();
        let last = self.spending.seen.back().map_or(self.run_spent, |s| s.1);
        if spent != last {
            self.spending.seen.push_back((self.tick, spent));
        }
        // The newest total from before the window stays: it is what the
        // window starts from.
        let from = self.tick.saturating_sub(RATE_WINDOW);
        while self.spending.seen.get(1).is_some_and(|s| s.0 <= from) {
            self.spending.seen.pop_front();
        }
        if self.spending.warned || !self.pot_low() {
            return;
        }
        let Some(leader) = self.open_projects().next().map(|p| p.leader) else {
            return;
        };
        self.spending.warned = true;
        let (rate, minutes) = (self.rate_per_hour(), self.pot_lasts().unwrap_or(0));
        self.note(Event::PotLow {
            leader,
            rate,
            minutes,
        });
    }

    /// Agents in the middle of a turn, spending tokens: what quitting now
    /// would cut short.
    pub fn mid_turn(&self) -> Vec<AgentId> {
        self.agents
            .iter()
            .filter(|a| !a.gone && a.status.busy())
            .map(|a| a.id)
            .collect()
    }

    /// The open inbox items that are workers' changes, not yet reviewed.
    pub fn open_changes(&self) -> Vec<usize> {
        self.open_inbox()
            .into_iter()
            .filter(|&i| self.inbox[i].kind == InboxKind::Change)
            .collect()
    }

    /// The helpers worker `id` has right now.
    pub fn helpers_of(&self, id: AgentId) -> impl Iterator<Item = &Agent> {
        self.agents
            .iter()
            .filter(move |a| a.role == Role::Helper && a.parent == Some(id) && !a.gone)
    }

    pub fn active_researchers(&self) -> usize {
        self.agents
            .iter()
            .filter(|a| a.role == Role::Researcher && !a.gone)
            .count()
    }

    /// Indexes of the open inbox items, oldest first; an archived
    /// project's wait out of sight, open, until it is brought back.
    pub fn open_inbox(&self) -> Vec<usize> {
        self.inbox
            .iter()
            .enumerate()
            .filter(|(_, i)| i.open)
            .filter(|(_, i)| !self.in_archive(i.agent))
            .map(|(n, _)| n)
            .collect()
    }

    /// How many open items ask the person to decide something: all but
    /// the replies, which are read (the header's `! N to decide`).
    pub fn open_decisions(&self) -> usize {
        (self.open_inbox().into_iter())
            .filter(|&i| self.inbox[i].kind != InboxKind::Reply)
            .count()
    }

    /// The card agent `id` works on: its own, or for a helper its
    /// worker's.
    pub fn card_of(&self, id: AgentId) -> Option<CardId> {
        let a = &self.agents[id];
        match a.parent {
            Some(p) if a.role == Role::Helper => self.agents[p].card,
            _ => a.card,
        }
    }

    /// The open inbox items TO DECIDE lists, oldest first: those
    /// not cleared off it.
    pub fn shown_inbox(&self) -> Vec<usize> {
        let mut open = self.open_inbox();
        open.retain(|i| !self.cleared.inbox.contains(i));
        open
    }

    /// How many open inbox items are cleared off TO DECIDE.
    pub fn hidden_inbox(&self) -> usize {
        self.open_inbox()
            .iter()
            .filter(|i| self.cleared.inbox.contains(i))
            .count()
    }

    /// The open inbox items of card `c`, oldest first: what came from the
    /// agents working on it (its owner, the owner's helpers) while they
    /// did.
    pub fn card_inbox(&self, c: CardId) -> Vec<usize> {
        self.open_inbox()
            .into_iter()
            .filter(|&i| self.inbox[i].card == Some(c))
            .collect()
    }

    /// Who had a part in card `c`, each once, with the first part they
    /// had: who posted it, its owner, the owner's helpers while it works
    /// on it, then anyone its story says talked about it.
    pub fn card_people(&self, c: CardId) -> Vec<(AgentId, Part)> {
        let card = &self.cards[c];
        let mut out: Vec<(AgentId, Part)> = Vec::new();
        let mut add = |id: AgentId, part: Part| {
            if !out.iter().any(|&(a, _)| a == id) {
                out.push((id, part));
            }
        };
        for s in &card.story {
            if let StoryEvent::Posted { by, .. } = s.event {
                add(by, Part::Posted);
            }
        }
        if let Some(owner) = card.owner {
            add(owner, Part::Owner);
            if self.agents[owner].card == Some(c) {
                for h in self.helpers_of(owner) {
                    add(h.id, Part::Helper);
                }
            }
        }
        for s in &card.story {
            match s.event {
                StoryEvent::Asked { from, to, .. } | StoryEvent::Sent { from, to, .. } => {
                    add(from, Part::Talked);
                    add(to, Part::Talked);
                }
                StoryEvent::Answered { from, .. } => add(from, Part::Talked),
                StoryEvent::ResearchArrived { researcher, .. } => add(researcher, Part::Talked),
                _ => {}
            }
        }
        out
    }

    /// The cards in sight: an archived project's wait in its archive.
    pub fn shown_cards(&self) -> impl Iterator<Item = &Card> {
        (self.cards.iter()).filter(|c| self.projects[c.project].archived.is_none())
    }

    /// The next message's number on the bus.
    pub fn number(&mut self) -> MsgId {
        self.messages += 1;
        self.messages
    }

    /// Who sent message `id` on the bus, to whom, and its first line, as
    /// the journal has it; none when it is not there (never numbered, or
    /// older than the journal keeps).
    pub fn message(&self, id: MsgId) -> Option<(AgentId, AgentId, &str)> {
        if id == 0 {
            return None;
        }
        self.journal.iter().rev().find_map(|e| match &e.event {
            Event::Sent {
                from,
                to,
                text,
                id: n,
                ..
            } if *n == id => Some((*from, *to, text.as_str())),
            _ => None,
        })
    }

    pub fn note(&mut self, event: Event) {
        self.journal.push(Entry {
            tick: self.tick,
            event,
        });
    }

    /// Agent `id` wrote `target`: its card, when it is a worker's whose
    /// card owns paths none of which names the file, and the file was not
    /// noted before (record 114); the card's story and the journal get
    /// it. Else nothing.
    pub fn strayed(&mut self, id: AgentId, target: &str) -> Option<CardId> {
        let a = &self.agents[id];
        let card = a.card.filter(|_| a.role == Role::Worker)?;
        let c = &self.cards[card];
        let noted =
            (c.story.iter()).any(|s| matches!(&s.event, StoryEvent::Strayed(f) if f == target));
        if noted || !crate::paths::strays(&c.paths, target) {
            return None;
        }
        self.story(Some(card), StoryEvent::Strayed(target.to_string()));
        self.note(Event::Strayed {
            agent: id,
            card,
            path: target.to_string(),
        });
        Some(card)
    }

    pub fn story(&mut self, card: Option<CardId>, event: StoryEvent) {
        if let Some(c) = card {
            let tick = self.tick;
            self.cards[c].story.push(Story { tick, event });
        }
    }

    pub fn approve(&mut self, item: usize) {
        let Some((kind, agent)) = self.close(item) else {
            return;
        };
        let research = kind == InboxKind::Research;
        let card = self.agents[agent].card;
        // A report the person reviewed no longer taints its reader.
        self.story(card, StoryEvent::YouApproved { research });
        self.note(Event::Approved { agent, research });
        let tick = self.tick;
        self.agents[agent].set_status(Status::Working, tick);
    }

    pub fn reject(&mut self, item: usize) {
        let Some((kind, agent)) = self.close(item) else {
            return;
        };
        let research = kind == InboxKind::Research;
        let card = self.agents[agent].card;
        self.story(card, StoryEvent::YouRejected { research });
        if !research {
            let a = &mut self.agents[agent];
            a.progress = a.progress.saturating_sub(10);
        }
        self.note(Event::Rejected { agent });
        let tick = self.tick;
        self.agents[agent].set_status(Status::Working, tick);
    }

    /// Agent `id` answered the person: its newest answer waits for them,
    /// an older unread one goes.
    pub fn replied(&mut self, id: AgentId, text: &str) {
        self.close_replies(id);
        self.inbox.push(InboxItem {
            kind: InboxKind::Reply,
            agent: id,
            detail: text
                .lines()
                .find(|l| !l.trim().is_empty())
                .unwrap_or_default()
                .to_string(),
            researcher: None,
            open: true,
            tick: self.tick,
            card: self.card_of(id),
        });
    }

    /// The change of worker `id` that waits for the person.
    /// What worker `id` said of its work under the first line of its
    /// newest `done`, the first line being the change's title; none when
    /// that is all it said. Live's journal keeps a message's first line
    /// only; the whole of it is in its cards' stories, by its number.
    pub fn done_why(&self, id: AgentId) -> Option<&str> {
        let (text, number) = self.journal.iter().rev().find_map(|e| match &e.event {
            Event::Sent {
                from,
                kind: MsgKind::Done,
                text,
                id: number,
                ..
            } if *from == id => Some((text, *number)),
            _ => None,
        })?;
        let whole = || {
            (self.cards.iter().rev())
                .flat_map(|c| c.story.iter().rev())
                .find_map(|s| match &s.event {
                    StoryEvent::Sent {
                        from,
                        kind: MsgKind::Done,
                        text,
                        id: n,
                        ..
                    } if *from == id && *n == number => Some(text),
                    _ => None,
                })
        };
        let text = if text.contains('\n') { text } else { whole()? };
        let (_, rest) = text.split_once('\n')?;
        Some(rest.trim()).filter(|why| !why.is_empty())
    }

    pub fn change_of(&self, id: AgentId) -> Option<&Change> {
        self.changes.iter().find(|c| c.agent == id)
    }

    /// Agent `id`'s open question to the person, whole, with the answers
    /// it offered: the newest ask of its in the journal; the item's first
    /// line alone when the journal no longer holds it.
    pub fn question_of(&self, id: AgentId) -> Option<Question<'_>> {
        let item = self.open_item(id, InboxKind::Question)?;
        let asked = self.journal.iter().rev().find_map(|e| match &e.event {
            Event::ToYou {
                from,
                kind: MsgKind::Ask,
                text,
                options,
            } if *from == id => Some(Question {
                text,
                options: options.as_slice(),
            }),
            _ => None,
        });
        Some(asked.unwrap_or(Question {
            text: &item.detail,
            options: &[],
        }))
    }

    /// The card worker `id`'s waiting change was made for: the one its
    /// item carries (the worker may have moved on), else the worker's.
    pub fn change_card(&self, id: AgentId) -> Option<CardId> {
        self.open_item(id, InboxKind::Change)
            .and_then(|i| i.card)
            .or(self.agents[id].card)
    }

    /// The checks the card of worker `id`'s waiting change asks for
    /// (record 127): what the person's yes to it asks Lonca to run too.
    pub fn asked_checks(&self, id: AgentId) -> &[String] {
        match self.change_card(id).and_then(|c| self.cards.get(c)) {
            Some(card) => &card.checks,
            None => &[],
        }
    }

    /// The verdict on worker `id`'s waiting change (record 125): ready
    /// when it is easy (proved clean, inside its card's paths), overlaps
    /// no other change waiting in its project, fits the checkout and is
    /// shown whole.
    pub fn verdict(&self, id: AgentId) -> Option<Verdict> {
        let c = self.change_of(id)?;
        let card = self.change_card(id).and_then(|c| self.cards.get(c));
        let project = self.agents[id].project;
        let waiting = (self.changes.iter())
            .filter(|o| o.agent != id && self.agents[o.agent].project == project);
        let pending = self.pending_elsewhere(id);
        let others = waiting.clone().count() + pending.len();
        let mut overlaps = waiting
            .map(|o| Overlap {
                with: Other::Change {
                    agent: o.agent,
                    card: self.change_card(o.agent),
                },
                files: c.shared_files(o),
            })
            .filter(|o| !o.files.is_empty())
            .collect::<Vec<_>>();
        overlaps.extend(pending.into_iter().filter_map(|p| {
            let files = c.shared_paths(p.paths.iter().map(String::as_str));
            (!files.is_empty()).then(|| Overlap {
                with: Other::Proposal {
                    seq: p.seq,
                    name: p.name.clone(),
                },
                files,
            })
        }));
        let proposal = self.proposal_of(id);
        let asked = self.asked_checks(id);
        let unknown = match proposal.and(self.policy_checks(project)) {
            Some(known) => crate::checks::unknown(asked, known),
            None => Vec::new(),
        };
        let asks = match proposal.map(|p| &p.verifies) {
            Some(Runs::These(runs)) => (asked.iter())
                .filter(|c| !runs.contains(c) && !unknown.contains(c))
                .cloned()
                .collect(),
            Some(Runs::Unsaid) => crate::checks::unknown(asked, &unknown),
            None => Vec::new(),
        };
        let clashes = self.clashes_of(id).is_some();
        let partial = proposal.is_some_and(|p| !p.redacted.is_empty() || p.cut);
        Some(Verdict {
            ready: c.easy(card)
                && overlaps.is_empty()
                && !clashes
                && !partial
                && unknown.is_empty(),
            proof: c.proofs(),
            files: c.files.len(),
            card: card.filter(|c| !c.paths.is_empty()).map(|c| c.id),
            outside: c.outside(card).iter().map(|f| f.to_string()).collect(),
            others,
            overlaps,
            clashes,
            partial,
            stale: proposal.is_some_and(|p| !p.on_trunk),
            verifies: proposal.map(|p| p.verifies.clone()),
            asks,
            unknown,
        })
    }

    /// The proposals waiting in worker `id`'s repository that no change of
    /// its project shows: anyone else's, and a worker's whose turn is not
    /// over (its own, and another worker's whose change is shown, are
    /// judged by the changes themselves).
    fn pending_elsewhere(&self, id: AgentId) -> Vec<&StorePending> {
        let project = self.agents[id].project;
        let Some(on) = &self.projects[project].lonca else {
            return Vec::new();
        };
        (on.pending.iter())
            .filter(|p| match p.holder {
                Some(holder) => holder != id && self.change_of(holder).is_none(),
                None => true,
            })
            .collect()
    }

    /// Worker `change.agent` finished with a change in its worktree: it
    /// waits for the person. An older change of the same worker is part
    /// of it and goes.
    pub fn change_ready(&mut self, mut change: Change) {
        let agent = change.agent;
        self.carry_spend(agent);
        self.forget_change(agent);
        self.move_cards(agent, Column::Working, Column::Review);
        let cards = self.owned_in(agent, Column::Review);
        self.count_round(agent, &cards, false);
        let (added, removed) = change.counts();
        let files = change.files.len();
        // Read with its turn over (a solo's, or a part's rest): what it
        // cost is known now; a worker's done comes mid-turn and waits for
        // the turn's end (`settle_change`).
        if self.agents[agent].status != Status::Working {
            change.spend = Some(self.spend_of(agent));
        }
        let spend = change.spend;
        self.inbox.push(InboxItem {
            kind: InboxKind::Change,
            agent,
            detail: change.title.clone(),
            researcher: None,
            open: true,
            tick: self.tick,
            card: self.card_of(agent),
        });
        self.changes.push(change);
        self.note(Event::ChangeReady {
            agent,
            files,
            added,
            removed,
            spend,
        });
    }

    /// A row of a change with nothing behind it closes: what a Lonca
    /// worker proposed is not saved (record 120), and the first look brings
    /// back what still waits. Run once, at a start.
    pub fn close_empty_changes(&mut self) {
        let waiting: Vec<AgentId> = self.changes.iter().map(|c| c.agent).collect();
        for i in &mut self.inbox {
            if i.kind == InboxKind::Change && !waiting.contains(&i.agent) {
                i.open = false;
            }
        }
    }

    /// Which proposal in Lonca worker `id`'s waiting change is.
    pub fn proposal_of(&self, id: AgentId) -> Option<&StoreProposal> {
        self.proposals.get(&id)
    }

    /// A proposal of worker `change.agent` waits in Lonca: it is its
    /// change now, an older one of the worker's going as ever.
    pub fn change_proposed(&mut self, change: Change, proposal: StoreProposal) {
        let agent = change.agent;
        self.change_ready(change);
        self.proposals.insert(agent, proposal);
    }

    /// Worker `id`'s proposal left the store's waiting list, by whoever's
    /// hand, and `how` is what became of it. What it came to is carried
    /// on, until it is on trunk.
    pub fn proposal_left(&mut self, id: AgentId, how: Landed) {
        self.carry_spend(id);
        self.forget_change(id);
        self.cards_landed(id, Column::Review, &how);
        self.landed(id, how);
    }

    /// What became of worker `id`'s proposal, which left its waiting
    /// change earlier if it did, in the queue: on trunk, what it cost is
    /// settled, and the journal says it.
    /// Lonca's queue checked worker `id`'s proposal: the journal says what
    /// came of it, the checks it skipped being none. No checks, nothing.
    pub fn verified(&mut self, id: AgentId, checks: Vec<Attested>) {
        if checks.is_empty() {
            return;
        }
        let ok = checks.iter().all(|c| c.ok);
        self.note(Event::Verified {
            agent: id,
            ok,
            checks,
        });
    }

    pub fn proposal_landed(&mut self, id: AgentId, how: Landed) {
        self.cards_landed(id, Column::Verifying, &how);
        self.landed(id, how);
    }

    /// The cards of worker `id`'s that stood in `from` follow what became
    /// of its proposal (record 126): into the queue, on trunk, or back to
    /// work.
    /// A conflict or a no sends them back (record 126, F9): a round more;
    /// a worker that withdrew its own proposal was sent back by nobody.
    fn cards_landed(&mut self, id: AgentId, from: Column, how: &Landed) {
        let (to, back) = match how {
            Landed::Queued => (Column::Verifying, false),
            Landed::Integrated(_) => (Column::Done, false),
            Landed::Conflicted(_) | Landed::Rejected(_) => (Column::Working, true),
            Landed::Withdrawn | Landed::Replaced => (Column::Working, false),
        };
        let moved = self.move_cards(id, from, to);
        if back {
            self.sent_back(&moved);
            self.count_round(id, &moved, true);
        }
        if matches!(how, Landed::Integrated(_)) {
            self.close_rows(id, &moved, crate::ledger::Outcome::Applied);
        }
    }

    /// Cards `cards` came back to work: a round more each.
    fn sent_back(&mut self, cards: &[CardId]) {
        for &c in cards {
            self.cards[c].breaker.sent_back += 1;
        }
    }

    fn landed(&mut self, id: AgentId, how: Landed) {
        if matches!(how, Landed::Integrated(_)) {
            self.agents[id].task.carried = Spend::default();
        }
        self.note(Event::Landed { agent: id, how });
    }

    /// Agent `id`'s turn began: its round opens now when none is open
    /// (record 113); a leader's spend is nobody's change.
    pub fn round_opens(&mut self, id: AgentId) {
        let tick = self.tick;
        let a = &mut self.agents[id];
        if a.role != Role::Leader && a.task.round.is_none() {
            a.task.round = Some((a.cost, tick));
            a.task.gaps = a.unpriced.gaps;
        }
    }

    /// What agent `id`'s change comes to now: what its earlier rounds came
    /// to, plus the round open (its cost since it opened, the ticks, one
    /// round) and its helpers' open rounds; every round closed, and the
    /// figure kept as its newest.
    fn spend_of(&mut self, id: AgentId) -> Spend {
        let tick = self.tick;
        let mut round = Spend::default();
        let party: Vec<AgentId> = std::iter::once(id)
            .chain(
                (self.agents.iter())
                    .filter(|h| h.parent == Some(id))
                    .map(|h| h.id),
            )
            .collect();
        for h in party {
            let a = &mut self.agents[h];
            if let Some((from, began)) = a.task.round.take() {
                round.cost += a.cost.saturating_sub(from);
                round.unpriced |= a.unpriced.session || a.unpriced.gaps > a.task.gaps;
                if h == id {
                    round.ticks = tick.saturating_sub(began);
                    round.rounds = 1;
                }
            }
        }
        let task = &mut self.agents[id].task;
        let spend = task.carried.plus(round);
        task.last = Some(spend);
        spend
    }

    /// Agent `id`'s turn ended: its waiting change not yet settled takes
    /// what it cost, said in the journal; false when none waits for it.
    pub fn settle_change(&mut self, id: AgentId) -> bool {
        let Some(i) = (self.changes.iter()).position(|c| c.agent == id && c.spend.is_none()) else {
            return false;
        };
        let spend = self.spend_of(id);
        self.changes[i].spend = Some(spend);
        self.note(Event::ChangeCost { agent: id, spend });
        true
    }

    /// Agent `id`'s worktree holds no change (a done with nothing in it,
    /// or the rest of a part applied was nothing): nothing is carried on.
    pub fn change_none(&mut self, id: AgentId) {
        let mut done = self.move_cards(id, Column::Working, Column::Done);
        done.extend(self.move_cards(id, Column::Review, Column::Done));
        // A turn of a solo's with nothing changed ends no row of its own.
        if !done.is_empty() {
            self.close_rows(id, &done, crate::ledger::Outcome::Done);
        }
        let task = &mut self.agents[id].task;
        task.carried = Spend::default();
        // A done with nothing in it costs the leader's mail no figure.
        task.last = None;
    }

    /// Agent `id`'s waiting change leaves unapplied: what it came to is
    /// carried into its next change.
    fn carry_spend(&mut self, id: AgentId) {
        if let Some(spend) = self.change_of(id).and_then(|c| c.spend) {
            self.agents[id].task.carried = spend;
        }
    }

    /// The person applied worker `id`'s change to their checkout; `mark`
    /// is the commit that stands for it in the worktree.
    /// `whole` says all of it went: what it cost is settled and dropped;
    /// a part applied carries it into the rest, read as a change again.
    pub fn change_applied(&mut self, id: AgentId, mark: String, whole: bool) {
        let files = self.change_of(id).map_or(0, |c| c.files.len());
        if whole {
            self.agents[id].task.carried = Spend::default();
            let done = self.move_cards(id, Column::Review, Column::Done);
            self.close_rows(id, &done, crate::ledger::Outcome::Applied);
        } else {
            self.carry_spend(id);
        }
        self.forget_change(id);
        self.note(Event::Applied {
            agent: id,
            files,
            mark,
        });
    }

    /// The person rejected worker `id`'s change; it stays in the worktree.
    pub fn change_rejected(&mut self, id: AgentId) {
        self.carry_spend(id);
        self.forget_change(id);
        let moved = self.move_cards(id, Column::Review, Column::Working);
        self.sent_back(&moved);
        self.count_round(id, &moved, true);
        self.note(Event::Rejected { agent: id });
    }

    /// Worker `id`'s cards standing in `from` move to `to` (record 126):
    /// a worker's worktree holds the work of every card it owns that is
    /// not through, so its change is theirs. A card through is finished
    /// now.
    /// A card back at work has lost the leases another card took while
    /// it waited, as a lapsed lease in Lonca; the paths it owns stay.
    /// The cards moved.
    fn move_cards(&mut self, id: AgentId, from: Column, to: Column) -> Vec<CardId> {
        let tick = self.tick;
        let moving: Vec<CardId> = (self.cards.iter())
            .filter(|c| c.owner == Some(id) && c.column == from)
            .map(|c| c.id)
            .collect();
        for &m in &moving {
            let held: Vec<Symbol> = (self.cards.iter())
                .filter(|o| o.project == self.cards[m].project && o.holds_leases())
                .flat_map(|o| o.symbols.iter().copied())
                .collect();
            let c = &mut self.cards[m];
            c.column = to;
            if c.over() {
                c.finished = tick;
            }
            if c.holds_leases() {
                c.symbols.retain(|s| !held.contains(s));
            }
        }
        moving
    }

    /// What a `done` of agent `from` is for (record 126): its card, when it
    /// still owns it at the attempt its turn took; else a late one.
    pub fn done_for(&self, from: AgentId) -> DoneFor {
        let a = &self.agents[from];
        match a.card {
            None => DoneFor::Nothing,
            Some(c) => {
                let card = &self.cards[c];
                if card.owner == Some(from)
                    && card.attempt == a.attempt
                    && card.column != Column::Withdrawn
                {
                    DoneFor::Card(c)
                } else {
                    DoneFor::Late(c)
                }
            }
        }
    }

    /// Agent `id` takes card `c` for its turn, at the card's attempt now.
    pub fn takes(&mut self, id: AgentId, c: CardId) {
        self.agents[id].card = Some(c);
        self.agents[id].attempt = self.cards[c].attempt;
        self.row_takes(id, c);
    }

    /// Worker `id`'s cards standing in `column`, by number.
    fn owned_in(&self, id: AgentId, column: Column) -> Vec<CardId> {
        (self.cards.iter())
            .filter(|c| c.owner == Some(id) && c.column == column)
            .map(|c| c.id)
            .collect()
    }

    /// The leader hands card `c` to `to` (record 126): its next attempt,
    /// at work again, taken from whoever held it, whose later `done`
    /// closes nothing. `to` takes it with the turn the assign starts.
    pub fn hand(&mut self, c: CardId, to: AgentId) {
        // The try at work ends here (record 133).
        self.close_card_row(c, crate::ledger::Outcome::HandedOn);
        let card = &mut self.cards[c];
        let from = card.owner;
        card.owner = Some(to);
        card.attempt += 1;
        card.column = Column::Working;
        card.finished = 0;
        let attempt = card.attempt;
        self.story(Some(c), StoryEvent::Handed { from, to, attempt });
        self.note(Event::Handed {
            agent: to,
            card: c,
            from,
            attempt,
        });
    }

    /// Card `c` asks Lonca for `checks` at the yes from now on (record
    /// 127), set by the leader `by` handing it again, else by the person
    /// on its worker's change. Nothing is noted when the list stays as it
    /// was. Whoever calls has checked the names.
    pub fn set_checks(&mut self, c: CardId, checks: Vec<String>, by: Option<AgentId>) {
        if self.cards[c].checks == checks {
            return;
        }
        self.cards[c].checks = checks.clone();
        let you = by.is_none();
        let Some(agent) = by.or(self.cards[c].owner) else {
            return;
        };
        let story = StoryEvent::ChecksSet {
            by,
            checks: checks.clone(),
        };
        self.story(Some(c), story);
        self.note(Event::ChecksSet {
            agent,
            card: c,
            checks,
            you,
        });
    }

    /// Whether card `c` may begin (record 126, F10): every card it waits
    /// on is done.
    pub fn ready(&self, c: CardId) -> bool {
        (self.cards[c].after.iter()).all(|&d| self.cards[d].column == Column::Done)
    }

    /// The cards card `c` still waits on, oldest first.
    pub fn waits_on(&self, c: CardId) -> Vec<CardId> {
        (self.cards[c].after.iter().copied())
            .filter(|&d| self.cards[d].column != Column::Done)
            .collect()
    }

    /// The card that holds `c` back (record 126, F10): one it waits on,
    /// or one those wait on, that failed (until it is handed again and
    /// done) or was withdrawn (for good). The nearest first, then by
    /// number; nothing when all it waits on may still be done.
    pub fn blocked_on(&self, c: CardId) -> Option<CardId> {
        let mut seen = vec![false; self.cards.len()];
        let mut next: std::collections::VecDeque<CardId> =
            self.cards[c].after.iter().copied().collect();
        while let Some(d) = next.pop_front() {
            if std::mem::replace(&mut seen[d], true) {
                continue;
            }
            match self.cards[d].column {
                Column::Failed | Column::Withdrawn => return Some(d),
                Column::Done => {}
                _ => next.extend(self.cards[d].after.iter().copied()),
            }
        }
        None
    }

    /// The cards not through that wait on card `c`, through others too,
    /// by number.
    pub fn dependents(&self, c: CardId) -> Vec<CardId> {
        let mut hit = vec![false; self.cards.len()];
        hit[c] = true;
        let mut out = Vec::new();
        // A card waits only on older ones: one pass in number order.
        for d in &self.cards {
            if !d.over() && d.after.iter().any(|&e| hit[e]) {
                hit[d.id] = true;
                out.push(d.id);
            }
        }
        out
    }

    /// Leader `by` withdraws card `c`, saying `why` (record 126): through,
    /// nothing comes of it; a `done` for it closes nothing.
    pub fn withdraw(&mut self, c: CardId, by: AgentId, why: &str) {
        let tick = self.tick;
        self.close_card_row(c, crate::ledger::Outcome::Withdrawn);
        let card = &mut self.cards[c];
        card.column = Column::Withdrawn;
        card.finished = tick;
        self.story(Some(c), StoryEvent::Withdrawn(why.to_string()));
        self.note(Event::Withdrawn {
            agent: by,
            card: c,
            why: why.to_string(),
        });
    }

    /// An assign gives card `c` to `to` before what it waits on is done
    /// (record 126, F10): the card is `to`'s, still waiting, untried; the
    /// assign goes with it once [`ready`](World::ready).
    pub fn hold(&mut self, c: CardId, to: AgentId) {
        self.cards[c].owner = Some(to);
        let on = self.waits_on(c);
        self.story(Some(c), StoryEvent::Held { to, on: on.clone() });
        self.note(Event::Held {
            agent: to,
            card: c,
            on,
        });
    }

    /// Agent `id` begins held card `c`, what it waited on done: its first
    /// try (or next), at work from now.
    pub fn begin(&mut self, c: CardId, id: AgentId) {
        let tick = self.tick;
        let card = &mut self.cards[c];
        card.owner = Some(id);
        card.column = Column::Working;
        card.started = tick;
        card.attempt += 1;
        self.story(Some(c), StoryEvent::Took { agent: id });
        self.note(Event::Released { agent: id, card: c });
    }

    /// Agent `id`'s `done` said card `c` failed: through, no change; `why`
    /// is the done's first line.
    pub fn card_failed(&mut self, id: AgentId, c: CardId, why: &str) {
        self.close_rows(id, &[c], crate::ledger::Outcome::Failed);
        let tick = self.tick;
        let card = &mut self.cards[c];
        card.column = Column::Failed;
        card.finished = tick;
        self.story(Some(c), StoryEvent::Failed(why.to_string()));
        self.note(Event::CardFailed {
            agent: id,
            card: c,
            why: why.to_string(),
        });
    }

    /// Card `c`'s `cap` stopped what would have gone on by itself (record
    /// 126, F9): an item waits for the person, `agent`'s (the leader that
    /// would hand it, or the worker whose change came back), one at a time
    /// a card and cap. True when one opened.
    pub fn capped(&mut self, cap: Cap, agent: AgentId, c: CardId) -> bool {
        let kind = cap.kind();
        if (self.inbox.iter()).any(|i| i.open && i.kind == kind && i.card == Some(c)) {
            return false;
        }
        let card = &self.cards[c];
        let count = match cap {
            Cap::Tries => card.attempt,
            Cap::Rounds => card.breaker.sent_back,
        };
        self.inbox.push(InboxItem {
            kind,
            agent,
            detail: card.title.clone(),
            researcher: None,
            open: true,
            tick: self.tick,
            card: Some(c),
        });
        self.story(Some(c), StoryEvent::Capped { cap, count });
        self.note(Event::Capped {
            agent,
            card: c,
            cap,
            count,
        });
        true
    }

    /// The person's answer to cap item `item`: yes allows one more try or
    /// round; no fails the card for good, and nobody hands it again. Who
    /// waited, the card and the cap; nothing when no such item is open.
    pub fn decide_cap(&mut self, item: usize, yes: bool) -> Option<(AgentId, CardId, Cap)> {
        let it = self.inbox.get(item).filter(|i| i.open)?;
        let cap = Cap::of(it.kind)?;
        let (agent, c) = (it.agent, it.card?);
        self.inbox[item].open = false;
        let tick = self.tick;
        let card = &mut self.cards[c];
        match (cap, yes) {
            (Cap::Tries, true) => card.breaker.more_tries += 1,
            (Cap::Rounds, true) => card.breaker.more_rounds += 1,
            (_, false) => {
                card.breaker.stopped = true;
                card.column = Column::Failed;
                card.finished = tick;
                self.close_card_row(c, crate::ledger::Outcome::Failed);
            }
        }
        self.story(Some(c), StoryEvent::CapAnswered { cap, yes });
        self.note(Event::CapAnswered {
            agent,
            card: c,
            cap,
            yes,
        });
        Some((agent, c, cap))
    }

    /// The card of worker `id`'s at work whose rounds are out: its change
    /// came back as often as it may by itself (record 126, F9).
    pub fn rounds_out_of(&self, id: AgentId) -> Option<CardId> {
        (self.cards.iter())
            .find(|c| c.owner == Some(id) && c.column == Column::Working && c.rounds_out())
            .map(|c| c.id)
    }

    /// Agent `id` sent a `done` for card `c` after it was handed again.
    pub fn late_done(&mut self, id: AgentId, c: CardId) {
        self.row_late(id, c, self.agents[id].attempt);
        self.story(Some(c), StoryEvent::LateDone { from: id });
        self.note(Event::LateDone { agent: id, card: c });
    }

    /// Worker `id`'s change did not apply; it still waits.
    pub fn change_failed(&mut self, id: AgentId, why: String) {
        self.note(Event::ApplyFailed { agent: id, why });
    }

    /// What git found of worker `id`'s change that does not fit the
    /// checkout: kept while there is any, else what was kept goes. Nothing
    /// clashing means nothing the worker can resolve.
    pub fn change_clashed(&mut self, id: AgentId, clashes: Clashes) {
        match clashes.hunks.is_empty() {
            true => self.clashes.remove(&id),
            false => self.clashes.insert(id, clashes),
        };
    }

    /// What of worker `id`'s waiting change does not fit the checkout.
    pub fn clashes_of(&self, id: AgentId) -> Option<&Clashes> {
        self.change_of(id)?;
        self.clashes.get(&id)
    }

    /// The review of worker `id`'s change that waits, if one came.
    pub fn review_of(&self, id: AgentId) -> Option<&Review> {
        self.change_of(id)?;
        self.reviews.get(&id)
    }

    /// `review` of worker `agent`'s change came: journaled, and kept on
    /// the change when it is still the one that was read (none waits, or
    /// a newer one, and it is only journaled).
    pub fn reviewed(&mut self, agent: AgentId, review: Review) {
        let read = self
            .change_of(agent)
            .is_some_and(|c| patch_sum(&c.patch) == review.patch);
        self.note(Event::Reviewed {
            agent,
            model: review.model.clone(),
            patch: review.patch.clone(),
            text: review.text.clone(),
            cost: review.cost,
        });
        if read {
            self.reviews.insert(agent, review);
        }
    }

    /// The newest review in the journal of each waiting change it read: a
    /// world remembered from an earlier run has its changes' reviews back.
    pub fn reviews_from_journal(&mut self) {
        for c in &self.changes {
            if self.reviews.contains_key(&c.agent) {
                continue;
            }
            let sum = patch_sum(&c.patch);
            let found = self.journal.iter().rev().find_map(|e| match &e.event {
                Event::Reviewed {
                    agent,
                    model,
                    patch,
                    text,
                    cost,
                } if *agent == c.agent && *patch == sum => Some(Review {
                    patch: patch.clone(),
                    model: model.clone(),
                    text: text.clone(),
                    cost: *cost,
                }),
                _ => None,
            });
            if let Some(r) = found {
                self.reviews.insert(c.agent, r);
            }
        }
    }

    fn forget_change(&mut self, id: AgentId) {
        self.reviews.remove(&id);
        self.clashes.remove(&id);
        self.proposals.remove(&id);
        self.changes.retain(|c| c.agent != id);
        for i in &mut self.inbox {
            if i.kind == InboxKind::Change && i.agent == id {
                i.open = false;
            }
        }
    }

    /// The person read or answered agent `id`'s answers.
    pub fn close_replies(&mut self, id: AgentId) {
        for i in &mut self.inbox {
            if i.kind == InboxKind::Reply && i.agent == id {
                i.open = false;
            }
        }
    }

    /// Agent `id` asked the person over the bus: its question waits for
    /// them, an older unanswered one goes (the conversation keeps it).
    pub fn asked_you(&mut self, id: AgentId, text: &str) {
        self.close_questions(id);
        self.inbox.push(InboxItem {
            kind: InboxKind::Question,
            agent: id,
            detail: text
                .lines()
                .find(|l| !l.trim().is_empty())
                .unwrap_or_default()
                .to_string(),
            researcher: None,
            open: true,
            tick: self.tick,
            card: self.card_of(id),
        });
    }

    /// The person answered agent `id` (or will not): its questions close.
    pub fn close_questions(&mut self, id: AgentId) {
        for i in &mut self.inbox {
            if i.kind == InboxKind::Question && i.agent == id {
                i.open = false;
            }
        }
    }

    /// A turn of agent `id` failed, `why` in Claude Code's words, `fault`
    /// its kind when Claude Code said: a passing one is tried again after
    /// the next of [`RETRY_TICKS`], until they are used up; that one and
    /// an account's wait for the person in the inbox; any failure else
    /// is said in the journal alone. The caller has made the agent idle.
    pub fn turn_failed(&mut self, id: AgentId, why: &str, fault: Option<Fault>) {
        let t = self.tick;
        let a = &mut self.agents[id];
        a.retry = None;
        let tries = usize::from(a.tries);
        if fault == Some(Fault::Passing) && tries < RETRY_TICKS.len() {
            let after = RETRY_TICKS[tries];
            a.tries += 1;
            a.retry = Some(Retry {
                attempt: a.tries,
                at: t + after,
            });
            let attempt = a.tries;
            self.note(Event::Retrying {
                agent: id,
                why: why.into(),
                attempt,
                seconds: after * SECONDS_PER_TICK,
            });
            return;
        }
        if fault.is_some() {
            a.tries = 0;
            self.close_faults(id);
            self.inbox.push(InboxItem {
                kind: InboxKind::Fault,
                agent: id,
                detail: why
                    .lines()
                    .find(|l| !l.trim().is_empty())
                    .unwrap_or_default()
                    .to_string(),
                researcher: None,
                open: true,
                tick: t,
                card: self.card_of(id),
            });
        }
        self.note(Event::Stopped {
            agent: id,
            why: why.into(),
        });
    }

    /// The agents whose failed turn is due to be tried again, in id
    /// order: its tick has come and nothing else woke them (idle, here).
    pub fn retries_due(&self) -> Vec<AgentId> {
        self.agents
            .iter()
            .filter(|a| {
                !a.gone && a.status == Status::Idle && a.retry.is_some_and(|r| r.at <= self.tick)
            })
            .map(|a| a.id)
            .collect()
    }

    /// The person wrote to agent `id`, or its session ended: what it
    /// waited on for a API error closes.
    pub fn close_faults(&mut self, id: AgentId) {
        for i in &mut self.inbox {
            if i.kind == InboxKind::Fault && i.agent == id {
                i.open = false;
            }
        }
    }

    /// The person wrote to leader `id`: a plan of its that waited closes,
    /// unanswered (record 114); the leader hears what they wrote and
    /// plans again.
    pub fn close_plans(&mut self, id: AgentId) {
        for i in &mut self.inbox {
            if i.kind == InboxKind::Plan && i.agent == id {
                i.open = false;
            }
        }
    }

    /// The leader of `project` made `proposal` (record 114): the plan it
    /// had waiting closes (a new plan replaces it), the gate waits for a
    /// yes again, and the plan goes to TO DECIDE. The item's index.
    pub fn propose(&mut self, project: ProjectId, proposal: Proposal) -> usize {
        let leader = self.projects[project].leader;
        self.close_plans(leader);
        let item = self.inbox.len();
        self.inbox.push(InboxItem {
            kind: InboxKind::Plan,
            agent: leader,
            detail: proposal
                .cards
                .first()
                .map_or(String::new(), |c| c.title.clone()),
            researcher: None,
            open: true,
            tick: self.tick,
            card: None,
        });
        self.note(Event::Planned {
            leader,
            cards: proposal.cards.len(),
            cost: proposal.cost,
        });
        let gate = &mut self.projects[project].plan;
        gate.approved = false;
        gate.proposal = Some(proposal);
        item
    }

    /// The person's answer to plan item `item`: yes opens the gate until
    /// the next plan; no drops the plan. The leader, or nothing when no
    /// plan waits there.
    pub fn decide_plan(&mut self, item: usize, yes: bool) -> Option<AgentId> {
        let it = self
            .inbox
            .get(item)
            .filter(|i| i.open && i.kind == InboxKind::Plan)?;
        let leader = it.agent;
        self.inbox[item].open = false;
        let gate = &mut self.projects[self.agents[leader].project].plan;
        gate.approved = yes;
        if !yes {
            gate.proposal = None;
        }
        self.note(Event::PlanAnswered { leader, yes });
        Some(leader)
    }

    /// The plan of `project` the person reads now: its open item.
    pub fn plan_item(&self, project: ProjectId) -> Option<usize> {
        let leader = self.projects[project].leader;
        (self.inbox.iter()).position(|i| i.open && i.kind == InboxKind::Plan && i.agent == leader)
    }

    /// Why the leader of `project` may not hire a second worker now
    /// (record 114): the gate is on, a worker is there already, and the
    /// person's yes to a plan is not; nothing when it may.
    pub fn plan_hold(&self, project: ProjectId) -> Option<PlanHold> {
        let gate = &self.projects[project].plan;
        let workers = (self.agents.iter())
            .filter(|a| a.project == project && a.role == Role::Worker && !a.gone)
            .count();
        if !gate.on || workers == 0 || gate.approved {
            return None;
        }
        Some(
            match (gate.proposal.is_some(), self.plan_item(project).is_some()) {
                (false, _) => PlanHold::NoPlan,
                (true, true) => PlanHold::Reading,
                (true, false) => PlanHold::Unanswered,
            },
        )
    }

    /// Whether agent `id` waits for the person's answer to a question.
    pub fn has_question(&self, id: AgentId) -> bool {
        self.open_item(id, InboxKind::Question).is_some()
    }

    /// The kinds of the open items that hold agent `id`'s mail.
    fn holding(&self, id: AgentId) -> impl Iterator<Item = InboxKind> + '_ {
        (self.inbox.iter())
            .filter(move |i| i.open && i.agent == id && i.kind.holds_mail())
            .map(|i| i.kind)
    }

    /// Whether agent `id` waits on the person: what comes for it stays
    /// queued and starts no turn, but for a cancel (record 107).
    pub fn waits_on_person(&self, id: AgentId) -> bool {
        self.agents[id].status == Status::WaitingHuman || self.holding(id).next().is_some()
    }

    /// Whether the person's word to agent `id` ends what it waits on: then
    /// the mail that waited goes with that word, not after it.
    pub fn word_frees(&self, id: AgentId) -> bool {
        let mut open = self.holding(id).peekable();
        open.peek().is_some() && open.all(InboxKind::closes_on_word)
    }

    /// The oldest open item of `kind` that agent `id` waits on: the one its
    /// row quotes.
    pub fn open_item(&self, id: AgentId, kind: InboxKind) -> Option<&InboxItem> {
        self.inbox
            .iter()
            .find(|i| i.open && i.kind == kind && i.agent == id)
    }

    /// Agent `id`'s session asks the person's yes for `what` before it
    /// does it; every ask is an item of its own, in the order they came.
    pub fn asks_you(&mut self, id: AgentId, what: &str) {
        self.inbox.push(InboxItem {
            kind: InboxKind::Approval,
            agent: id,
            detail: what.to_string(),
            researcher: None,
            open: true,
            tick: self.tick,
            card: self.card_of(id),
        });
    }

    /// The person says yes to everything agent `id` asks until its turn
    /// ends; the journal says so.
    pub fn yes_this_turn(&mut self, id: AgentId) {
        if self.agents[id].turn_yes.is_none() {
            self.agents[id].turn_yes = Some(0);
            self.note(Event::TurnYes { agent: id });
        }
    }

    /// Whether agent `id`'s ask goes on by the person's yes for its turn;
    /// counted when it does.
    pub fn allowed_this_turn(&mut self, id: AgentId) -> bool {
        match &mut self.agents[id].turn_yes {
            Some(n) => {
                *n = n.saturating_add(1);
                true
            }
            None => false,
        }
    }

    /// Agent `id`'s turn or session ended: a yes for it ends too, and the
    /// journal says how many asks it let go on.
    pub fn turn_over(&mut self, id: AgentId) {
        if let Some(allowed) = self.agents[id].turn_yes.take() {
            self.note(Event::TurnYesEnded { agent: id, allowed });
        }
    }

    /// Adds `rules` to project `p`'s standing yes, sorted and once each;
    /// the journal says which were new. The new ones are given back.
    pub fn stand(&mut self, p: ProjectId, rules: &[Standing]) -> Vec<Standing> {
        let allowed = &mut self.projects[p].allowed;
        let mut added: Vec<Standing> = Vec::new();
        for r in rules {
            if !allowed.contains(r) && !added.contains(r) {
                added.push(r.clone());
            }
        }
        if !added.is_empty() {
            allowed.extend(added.iter().cloned());
            allowed.sort();
            let leader = self.projects[p].leader;
            self.note(Event::Standing {
                leader,
                rules: added.iter().map(Standing::line).collect(),
                added: true,
            });
        }
        added
    }

    /// Takes `rule` out of project `p`'s standing yes; false when it was
    /// not there.
    pub fn unstand(&mut self, p: ProjectId, rule: &Standing) -> bool {
        let allowed = &mut self.projects[p].allowed;
        let Some(i) = allowed.iter().position(|r| r == rule) else {
            return false;
        };
        allowed.remove(i);
        let leader = self.projects[p].leader;
        self.note(Event::Standing {
            leader,
            rules: vec![rule.line()],
            added: false,
        });
        true
    }

    /// The person allowed or refused what open item `item` asks: it
    /// closes and the journal says so. Anything but an open approval is
    /// left alone.
    pub fn decided(&mut self, item: usize, allowed: bool) {
        let Some(it) = self
            .inbox
            .get_mut(item)
            .filter(|i| i.open && i.kind == InboxKind::Approval)
        else {
            return;
        };
        it.open = false;
        let (agent, what) = (it.agent, it.detail.clone());
        self.note(Event::Decided {
            agent,
            what,
            allowed,
        });
    }

    /// Agent `id`'s session is over: what it asked waits for nobody.
    /// The person said yes to the setup script `key`: kept, the newest
    /// last, the oldest let go past [`SETUP_KEPT`].
    pub fn approve_setup(&mut self, key: String) {
        self.setup_approved.retain(|k| *k != key);
        self.setup_approved.push(key);
        let over = self.setup_approved.len().saturating_sub(SETUP_KEPT);
        self.setup_approved.drain(..over);
    }

    /// Setup script asks are of the run that made them: its bytes are not
    /// kept, so a start closes them; the next new worktree asks again.
    pub fn close_scripts(&mut self) {
        for i in &mut self.inbox {
            if matches!(i.kind, InboxKind::Script | InboxKind::Check) {
                i.open = false;
            }
        }
    }

    /// The repository's check on worker `id`'s waiting change stands at
    /// `check` now; false when no change of theirs waits.
    pub fn checked(&mut self, id: AgentId, check: Check) -> bool {
        match self.changes.iter_mut().find(|c| c.agent == id) {
            Some(c) => {
                c.check = check;
                true
            }
            None => false,
        }
    }

    pub fn close_approvals(&mut self, id: AgentId) {
        for i in &mut self.inbox {
            if i.kind == InboxKind::Approval && i.agent == id {
                i.open = false;
            }
        }
    }

    /// Whether agent `id`'s session waits for the person's yes.
    #[cfg(test)]
    pub fn has_approval(&self, id: AgentId) -> bool {
        self.open_item(id, InboxKind::Approval).is_some()
    }

    fn close(&mut self, item: usize) -> Option<(InboxKind, AgentId)> {
        let it = self.inbox.get_mut(item).filter(|i| i.open)?;
        it.open = false;
        Some((it.kind, it.agent))
    }

    /// The person says something to an agent: the journal, the card's story,
    /// the salon's talk line and the trace record it. Whoever drives the
    /// agent (the simulation or a live session) acts on it.
    pub fn said(&mut self, id: AgentId, text: &str) {
        let t = self.tick;
        self.close_replies(id);
        self.close_questions(id);
        self.close_faults(id);
        self.close_plans(id);
        // The person's word is the next try: a new count from it.
        self.agents[id].retry = None;
        self.agents[id].tries = 0;
        self.note(Event::YouSaid {
            to: id,
            text: text.into(),
        });
        let card = self.agents[id].card;
        self.story(card, StoryEvent::YouSaid(text.into()));
        let a = &mut self.agents[id];
        a.talk = Some(Talk {
            with: Party::Person,
            mine: false,
            text: text.into(),
        });
        a.trace(t, format!("session/prompt (person) {text:?}"));
    }
}

/// The wall clock of a tick, `HH:MM`.
pub fn clock(tick: u64) -> String {
    let s = start_seconds() + tick * SECONDS_PER_TICK;
    format!("{:02}:{:02}", s / 3600 % 24, s / 60 % 60)
}

/// A tick's day and clock, `YYYY-MM-DD HH:MM`: for text that leaves the
/// screen, where "today" means nothing.
pub fn stamp(tick: u64) -> String {
    format!("{} {}", date(day(tick)), clock(tick))
}

/// A tick's clock, with its date in front when it is not of `today` (a
/// day as [`day`] counts them).
pub fn when(tick: u64, today: u64) -> String {
    if day(tick) == today {
        clock(tick)
    } else {
        stamp(tick)
    }
}

fn clock_seconds(tick: u64) -> String {
    let s = start_seconds() + tick * SECONDS_PER_TICK;
    format!("{:02}:{:02}:{:02}", s / 3600 % 24, s / 60 % 60, s % 60)
}

/// Ten-thousandths of a dollar as `$D.CC`; a cost under a cent that is not
/// zero reads `<$0.01`, so cheap work never looks free.
/// Whether `s` may go to Claude Code as a model choice (`[MODEL][@EFFORT]`):
/// 1-64 of letters, digits and `-._[]@`, not starting with `-`, so it can
/// never pass as a flag. Model names as Claude Code takes them: `sonnet`,
/// `claude-opus-5-5`, `opus[1m]`, `gpt-5.6-terra@high`.
pub fn valid_model(s: &str) -> bool {
    (1..=64).contains(&s.len())
        && !s.starts_with('-')
        && s.bytes()
            .all(|b| b.is_ascii_alphanumeric() || b"-._[]@".contains(&b))
}

/// `claude[:MODEL][@EFFORT]` as `--solo` and `:solo` take it (`claude`,
/// `claude:opus`, `claude@xhigh`): the `[MODEL][@EFFORT]` choice after the
/// word, none for Claude Code's own. None when the word is not there, or
/// the choice is not a model choice: the word keeps a choice apart from a
/// path or a name.
pub fn solo_choice(s: &str) -> Option<Option<String>> {
    let cut = s.find([':', '@']).unwrap_or(s.len());
    if s[..cut] != *CLAUDE {
        return None;
    }
    let rest = &s[cut..];
    let choice = rest.strip_prefix(':').unwrap_or(rest);
    if choice.is_empty() {
        return Some(None);
    }
    valid_model(choice).then(|| Some(choice.to_string()))
}

/// Dollars as typed (`5`, `2.50`), in ten-thousandths of a dollar.
pub fn parse_dollars(s: &str) -> Option<u64> {
    let (whole, cents) = s.split_once('.').unwrap_or((s, ""));
    if whole.is_empty() || whole.len() > 4 || cents.len() > 2 {
        return None;
    }
    if !(whole.bytes().chain(cents.bytes())).all(|b| b.is_ascii_digit()) {
        return None;
    }
    let cents: u64 = format!("{cents:0<2}").parse().ok()?;
    Some(whole.parse::<u64>().ok()? * 10_000 + cents * 100)
}

/// [`World::money`], `estimated` said apart.
pub fn money(cost: u64, unpriced: bool, estimated: bool) -> String {
    let about = if estimated { "≈" } else { "" };
    match (unpriced, cost) {
        (true, 0) => "?".to_string(),
        (true, _) => format!("{about}{}+?", dollars(cost)),
        (false, _) => format!("{about}{}", dollars(cost)),
    }
}

pub fn dollars(cost: u64) -> String {
    if (1..100).contains(&cost) {
        return "<$0.01".to_string();
    }
    format!("${}.{:02}", cost / 10_000, cost / 100 % 100)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_yes_for_a_turn_is_given_once_counts_what_it_lets_go_and_ends_said() {
        let mut w = crate::live::testing::team(2);
        let journal = w.journal.len();
        assert!(!w.allowed_this_turn(1), "none given");
        assert_eq!(w.agents[1].turn_yes, None);
        w.turn_over(1);
        assert_eq!(w.journal.len(), journal, "no yes, nothing to say");
        w.yes_this_turn(1);
        assert_eq!(w.agents[1].turn_yes, Some(0));
        assert_eq!(w.journal.last().unwrap().event, Event::TurnYes { agent: 1 });
        assert_eq!(w.journal.last().unwrap().event.agent(), 1);
        assert_eq!(w.journal.last().unwrap().event.kind(), EntryKind::Person);
        assert!(w.allowed_this_turn(1) && w.allowed_this_turn(1));
        assert_eq!(w.agents[1].turn_yes, Some(2));
        assert!(!w.allowed_this_turn(2), "another agent's is its own");
        // Given again while it holds: the count stays, said once.
        w.yes_this_turn(1);
        assert_eq!(w.agents[1].turn_yes, Some(2));
        assert_eq!(w.journal.len(), journal + 1);
        w.agents[2].turn_yes = Some(u32::MAX);
        assert!(w.allowed_this_turn(2));
        assert_eq!(w.agents[2].turn_yes, Some(u32::MAX), "saturates");
        w.turn_over(1);
        assert_eq!(w.agents[1].turn_yes, None);
        let ended = &w.journal.last().unwrap().event;
        assert_eq!(
            *ended,
            Event::TurnYesEnded {
                agent: 1,
                allowed: 2
            }
        );
        assert_eq!((ended.agent(), ended.kind()), (1, EntryKind::Person));
        w.turn_over(1);
        assert_eq!(w.journal.len(), journal + 2, "said once");
    }

    #[test]
    fn a_fill_s_percent_needs_a_size_and_stops_at_a_hundred() {
        let fill = |used, size| Fill { used, size };
        assert_eq!(fill(45_506, None).percent(), None);
        assert_eq!(fill(45_506, Some(0)).percent(), None);
        assert_eq!(fill(45_506, Some(200_000)).percent(), Some(22));
        assert_eq!(fill(139_999, Some(200_000)).percent(), Some(69));
        assert_eq!(fill(140_000, Some(200_000)).percent(), Some(70));
        assert_eq!(fill(200_000, Some(200_000)).percent(), Some(100));
        assert_eq!(fill(250_000, Some(200_000)).percent(), Some(100));
        assert_eq!(fill(0, Some(200_000)).percent(), Some(0));
        assert_eq!((CONTEXT_SHOWN, CONTEXT_LOUD), (70, 90));
        // Leaving forgets it.
        let mut a = Agent::new(0, 0, "a".into(), Role::Worker, Status::Idle, 0);
        assert_eq!(a.context, None);
        a.context = Some(fill(1, Some(2)));
        let mut w = world(vec![a]);
        w.leave(0);
        assert_eq!(w.agents[0].context, None);
    }

    #[test]
    fn plan_count_counts_the_done_steps_and_leaving_clears_the_list() {
        let step = |state| Step {
            text: "s".into(),
            state,
        };
        let mut a = Agent::new(0, 0, "a".into(), Role::Worker, Status::Idle, 0);
        assert_eq!(a.plan_count(), (0, 0));
        a.plan = vec![
            step(StepState::Done),
            step(StepState::Doing),
            step(StepState::Done),
            step(StepState::Done),
        ];
        assert_eq!(a.plan_count(), (3, 4), "done, not the rest");
        let mut w = world(vec![a]);
        w.leave(0);
        assert!(w.agents[0].plan.is_empty());
    }

    #[test]
    fn a_loopback_url_is_found_with_its_port_and_path_and_nothing_after() {
        let url = |t: &str| local_url(t);
        let some = |u: &str| Some(u.to_string());
        assert_eq!(
            url("up at http://localhost:5173/ now"),
            some("http://localhost:5173/")
        );
        assert_eq!(
            url("https://127.0.0.1:8443/app"),
            some("https://127.0.0.1:8443/app")
        );
        assert_eq!(url("http://[::1]:3000"), some("http://[::1]:3000"));
        assert_eq!(url("http://localhost"), some("http://localhost"));
        assert_eq!(url("http://localhost/x"), some("http://localhost/x"));
        // Sentence punctuation and brackets around it are not part of it.
        assert_eq!(
            url("(see http://localhost:3000)."),
            some("http://localhost:3000")
        );
        for end in [".", ",", ":", "!", "\"", "'", ";", "`", ">"] {
            assert_eq!(
                url(&format!("http://localhost:3000{end}")),
                some("http://localhost:3000"),
                "{end}"
            );
        }
        assert_eq!(url("at http://localhost: then"), some("http://localhost"));
        // The query and fragment are dropped.
        assert_eq!(
            url("http://localhost:8888/lab?token=abc#x"),
            some("http://localhost:8888/lab")
        );
        assert_eq!(
            url("http://localhost:8888#top"),
            some("http://localhost:8888")
        );
        // The newest one counts.
        assert_eq!(
            url("http://localhost:1/ moved to http://localhost:2/ and https://example.com"),
            some("http://localhost:2/")
        );
        assert_eq!(url("httphttp://localhost:9/"), some("http://localhost:9/"));
        // Ports: 1 to 65535, five digits at most.
        assert_eq!(url("http://localhost:1"), some("http://localhost:1"));
        assert_eq!(
            url("http://localhost:65535/"),
            some("http://localhost:65535/")
        );
        assert_eq!(
            url("http://localhost:00080"),
            some("http://localhost:00080")
        );
        // Nothing that could end the terminal's sequence or leave the host.
        assert_eq!(
            url("http://localhost:3000\u{1b}]8;;http://evil.com\u{1b}\\"),
            some("http://localhost:3000")
        );
        assert_eq!(
            url("http://localhost:3000\\@evil.com"),
            some("http://localhost:3000")
        );
        assert_eq!(
            url("http://localhost:3000/ü/x"),
            some("http://localhost:3000/")
        );
        assert_eq!(
            url("http://localhost:3000/a b"),
            some("http://localhost:3000/a")
        );
    }

    #[test]
    fn a_web_address_is_strict_and_leaves_the_sentence_out() {
        let at = |s: &str| web_url_at(s).map(str::to_string);
        for (text, url) in [
            ("https://a.b/c d", "https://a.b/c"),
            ("http://a.b/c.", "http://a.b/c"),
            ("https://a.b/c?x=1&y=2#z,", "https://a.b/c?x=1&y=2#z"),
            ("https://a.b/c!?:*", "https://a.b/c"),
            (
                "https://en.x.org/wiki/A_(b))",
                "https://en.x.org/wiki/A_(b)",
            ),
            ("https://a.b/c)", "https://a.b/c"),
            ("https://a.b/c).", "https://a.b/c"),
            ("https://a.b/[1]]", "https://a.b/[1]"),
            ("https://a.b/c]", "https://a.b/c"),
            ("https://a.b/c\"x", "https://a.b/c"),
            ("https://a.b/c;x", "https://a.b/c"),
            ("https://a.b/c\u{7}x", "https://a.b/c"),
            ("https://a.b/çx", "https://a.b/"),
            ("http://9.9.9.9:80/", "http://9.9.9.9:80/"),
        ] {
            assert_eq!(at(text).as_deref(), Some(url), "{text}");
            assert!(is_web_url(url), "{url}");
        }
        for text in [
            "ftp://a.b",
            "javascript:x",
            "http://",
            "https:// a",
            "http:///a",
            "http://-a",
            "x https://a.b",
        ] {
            assert_eq!(at(text), None, "{text}");
        }
        let long = format!("https://a.b/{}", "x".repeat(WEB_URL_MAX - 12));
        assert_eq!(long.len(), WEB_URL_MAX);
        assert_eq!(at(&long), Some(long.clone()));
        assert_eq!(at(&format!("{long}x")), None);
        assert!(!is_web_url("https://a.b/c."));
        assert!(!is_web_url("see https://a.b"));
    }

    #[test]
    fn only_a_loopback_host_and_a_real_port_make_a_url_and_a_long_one_is_left_out() {
        for text in [
            "no url here",
            "https://example.com:5173/",
            "ftp://localhost/",
            "HTTP://LOCALHOST/",
            "http://localhost.evil.com/",
            "http://localhost@evil.com/",
            "http://evil.com@localhost/",
            "http://localhostx/",
            "http://localhost-x/",
            "http://127.0.0.1.nip.io/",
            "http://[::1].evil/",
            "http://localhost:0/",
            "http://localhost:65536/",
            "http://localhost:000080/",
            "http://localhost:80a/",
            "http://localhost:80:81/",
            "http://localhost:3000@evil.com/",
            "http://localhost:/x",
        ] {
            assert_eq!(local_url(text), None, "{text}");
        }
        // As long as the row allows, not a character more.
        let head = "http://localhost:5173/";
        let fits = format!("{head}{}", "a".repeat(LOCAL_URL_MAX - head.len()));
        assert_eq!(fits.len(), 60);
        assert_eq!(local_url(&fits), Some(fits.clone()));
        assert_eq!(local_url(&format!("{fits}a")), None);
        // Dropped parts do not count against it.
        assert_eq!(local_url(&format!("{fits}?q=1")), Some(fits));
    }

    #[test]
    fn what_an_agent_writes_keeps_its_newest_loopback_url() {
        let mut a = Agent::new(0, 0, "w".into(), Role::Worker, Status::Idle, 0);
        a.wrote("thinking");
        assert_eq!(a.local, None);
        a.wrote("runs at http://localhost:5173/");
        assert_eq!(a.local.as_deref(), Some("http://localhost:5173/"));
        a.wrote("nothing new");
        assert_eq!(a.local.as_deref(), Some("http://localhost:5173/"), "kept");
        a.wrote("now http://127.0.0.1:8080");
        assert_eq!(a.local.as_deref(), Some("http://127.0.0.1:8080"));
    }

    #[test]
    fn the_pot_has_paid_the_agents_and_imeces_own_calls() {
        let mut w = crate::live::testing::team(2);
        w.agents.iter_mut().for_each(|a| a.cost = 0);
        w.agents[1].cost = 300;
        w.agents[2].cost = 20;
        w.paid = 5;
        assert_eq!(w.spent(), 325);
    }

    #[test]
    fn a_cost_under_a_cent_is_not_shown_as_free() {
        assert_eq!(dollars(0), "$0.00");
        assert_eq!(dollars(3), "<$0.01");
        assert_eq!(dollars(99), "<$0.01");
        assert_eq!(dollars(100), "$0.01");
        assert_eq!(dollars(123_456), "$12.34");
    }

    fn agent(id: AgentId, role: Role, status: Status) -> Agent {
        Agent::new(id, 0, format!("a{id}"), role, status, 0)
    }

    fn world(agents: Vec<Agent>) -> World {
        World {
            marks: Default::default(),
            cleared: Default::default(),
            setup_approved: Vec::new(),
            sent: Vec::new(),
            limits: Vec::new(),
            tick: 0,
            started_at: 0,
            demo: false,
            run_spent: 0,
            spending: Spending::default(),
            projects: vec![Project {
                id: 0,
                name: "p".into(),
                leader: 0,
                trunk: None,
                files: Vec::new(),
                repo: &[],
                closed: false,
                pinned: false,
                copies: Vec::new(),
                allowed: Vec::new(),
                plan: PlanGate::default(),
                no_git: false,
                lonca: None,
                archived: None,
            }],
            agents,
            cards: Vec::new(),
            journal: Vec::new(),
            inbox: Vec::new(),
            budget: 10_000,
            estimated: false,
            max_researchers: 0,
            research_queue: Default::default(),
            changes: Vec::new(),
            paid: 0,
            suggested: None,
            reviews: Default::default(),
            clashes: Default::default(),
            proposals: Default::default(),
            orphans: crate::model::Orphans::default(),
            health: Default::default(),
            procs: Default::default(),
            ledger: Vec::new(),
            version: None,
            messages: 0,
            plans: Vec::new(),
        }
    }

    #[test]
    fn an_archive_view_holds_its_lines_and_puts_every_other_project_away() {
        let mut w = world(vec![
            agent(0, Role::Leader, Status::Asleep),
            agent(1, Role::Worker, Status::Asleep),
            agent(2, Role::Leader, Status::Idle),
            agent(3, Role::Leader, Status::Asleep),
        ]);
        let project = |id, leader| Project {
            id,
            leader,
            ..w.projects[0].clone()
        };
        let (one, two) = (project(1, 2), project(2, 3));
        w.projects.extend([one, two]);
        w.agents[2].project = 1;
        w.agents[3].project = 2;
        let archived = |at, title: &str| Archived {
            at,
            title: title.into(),
        };
        w.projects[0].closed = true;
        w.projects[0].archived = Some(archived(7, "epic"));
        w.projects[2].closed = true;
        w.projects[2].archived = Some(archived(3, "old"));
        (
            w.tick,
            w.started_at,
            w.run_spent,
            w.paid,
            w.messages,
            w.budget,
        ) = (40, 9, 11, 13, 5, 99);
        w.spending.warned = true;
        w.research_queue.push_back(2);
        w.sent.push(Sent {
            tick: 1,
            text: "hi".into(),
        });
        w.limits.push(Limit {
            minutes: 300,
            used: 50,
            resets_at: None,
        });
        w.suggested = Some(Suggestion {
            project: 1,
            text: "x".into(),
        });
        for agent in [0, 2, 1, 2] {
            w.tick += 1;
            w.note(Event::Failed { agent });
        }
        w.marks = [1, 2].into();
        for agent in [0, 2] {
            w.inbox.push(InboxItem {
                kind: InboxKind::Question,
                agent,
                detail: "q".into(),
                researcher: None,
                open: true,
                tick: 0,
                card: None,
            });
        }
        // Each project's lines, its marks counted in them.
        let (lines, marks) = w.journal_of(0);
        assert_eq!(lines, [w.journal[0].clone(), w.journal[2].clone()]);
        assert_eq!(marks, [1].into());
        let (theirs, their_marks) = w.journal_of(1);
        assert_eq!(theirs, [w.journal[1].clone(), w.journal[3].clone()]);
        assert_eq!(their_marks, [0].into());
        let v = w.view_of(0, lines.clone(), marks.clone());
        assert!(!v.projects[0].closed && v.projects[0].archived.is_none());
        assert!(v.projects[1].closed);
        assert_eq!(v.projects[1].archived, Some(archived(0, "")));
        assert_eq!(
            v.projects[2].archived,
            Some(archived(3, "old")),
            "its own kept"
        );
        assert_eq!((&v.journal, &v.marks), (&lines, &marks));
        assert_eq!(v.agents.len(), 4);
        assert_eq!(v.inbox.len(), 2);
        assert_eq!(v.open_inbox(), [0], "only the archive's waits");
        assert_eq!(v.agent_by_name("a2"), None);
        assert_eq!(v.agent_by_name("a0"), Some(0));
        assert_eq!(
            (
                v.tick,
                v.started_at,
                v.run_spent,
                v.paid,
                v.messages,
                v.budget
            ),
            (44, 9, 11, 13, 5, 99)
        );
        assert!(v.spending.warned);
        assert_eq!(v.sent, w.sent);
        assert_eq!(v.max_researchers, w.max_researchers);
        assert!(v.research_queue.is_empty() && v.limits.is_empty() && v.suggested.is_none());
        assert_eq!(
            (v.cards.len(), v.plans.len()),
            (w.cards.len(), w.plans.len())
        );
    }

    #[test]
    fn messages_are_numbered_in_order_and_found_by_their_number() {
        let mut w = world(vec![
            agent(0, Role::Leader, Status::Idle),
            agent(1, Role::Worker, Status::Idle),
        ]);
        w.messages = 4;
        assert_eq!((w.number(), w.number(), w.messages), (5, 6, 6));
        let sent = |from, to, text: &str, id| Event::Sent {
            from,
            to,
            kind: MsgKind::Ask,
            text: text.into(),
            id,
            re: None,
        };
        // One saved before numbers (0), then two numbered.
        w.note(sent(1, 0, "old", 0));
        w.note(sent(0, 1, "first", 5));
        w.note(Event::Left { agent: 1 });
        w.note(sent(1, 0, "second", 6));
        assert_eq!(w.message(5), Some((0, 1, "first")));
        assert_eq!(w.message(6), Some((1, 0, "second")));
        assert_eq!(w.message(7), None);
        assert_eq!(w.message(0), None, "0 is no number");
    }

    #[test]
    fn a_message_saved_before_numbers_reads_as_number_0_replying_to_none() {
        let old = r#"{"Sent":{"from":1,"to":0,"kind":"Done","text":"x"}}"#;
        let e: Event = serde_json::from_str(old).unwrap();
        assert!(
            matches!(
                e,
                Event::Sent {
                    id: 0,
                    re: None,
                    ..
                }
            ),
            "{e:?}"
        );
        let s: StoryEvent = serde_json::from_str(old).unwrap();
        assert!(
            matches!(
                s,
                StoryEvent::Sent {
                    id: 0,
                    re: None,
                    ..
                }
            ),
            "{s:?}"
        );
        let e = Event::Sent {
            from: 1,
            to: 0,
            kind: MsgKind::Done,
            text: "x".into(),
            id: 3,
            re: Some(2),
        };
        let back: Event = serde_json::from_str(&serde_json::to_string(&e).unwrap()).unwrap();
        assert_eq!(back, e);
    }

    #[test]
    fn a_mark_comes_and_goes_and_only_on_a_line_the_journal_has() {
        let mut w = world(vec![agent(0, Role::Leader, Status::Idle)]);
        for _ in 0..2 {
            w.note(Event::Confine {
                leader: 0,
                level: "standard".into(),
            });
        }
        assert!(w.mark(1));
        assert_eq!(w.marks, BTreeSet::from([1]));
        assert!(w.mark(0));
        assert_eq!(w.marks, BTreeSet::from([0, 1]));
        assert!(!w.mark(1), "again: unmarked");
        assert_eq!(w.marks, BTreeSet::from([0]));
        assert!(!w.mark(2), "past the journal's end");
        assert_eq!(w.marks, BTreeSet::from([0]));
    }

    #[test]
    fn a_window_said_again_replaces_its_old_self_and_the_tightest_shows() {
        let mut w = world(vec![agent(0, Role::Leader, Status::Idle)]);
        let at = |minutes, used| Limit {
            minutes,
            used,
            resets_at: Some(u64::from(minutes)),
        };
        assert_eq!(w.tightest(), None);
        w.limit(at(300, 10));
        w.limit(at(10_080, 40));
        // The same window again: replaced, not added.
        w.limit(at(300, 30));
        assert_eq!(w.limits.len(), 2);
        assert_eq!(w.tightest(), Some(at(10_080, 40)));
        // The five hours fill up past the week: it is the tightest.
        w.limit(at(300, 41));
        assert_eq!(w.tightest(), Some(at(300, 41)));
        // A later, emptier window does not take the place.
        w.limit(at(10_080, 29));
        assert_eq!(w.tightest(), Some(at(300, 41)));
        // A tie: the shorter.
        w.limit(at(60, 41));
        assert_eq!(w.tightest(), Some(at(60, 41)), "a tie: the shorter");
        // Its clock: the first start and the ticks since.
        w.started_at = 1_000;
        w.tick = 3;
        assert_eq!(w.now(), 1_015);
    }

    fn item(kind: InboxKind, agent: AgentId, open: bool) -> InboxItem {
        InboxItem {
            kind,
            agent,
            detail: String::new(),
            researcher: None,
            open,
            tick: 0,
            card: None,
        }
    }

    #[test]
    fn phases_follow_progress() {
        assert_eq!(Phase::of(0), Phase::Plan);
        assert_eq!(Phase::of(19), Phase::Plan);
        assert_eq!(Phase::of(20), Phase::Impl);
        assert_eq!(Phase::of(69), Phase::Impl);
        assert_eq!(Phase::of(70), Phase::Test);
        assert_eq!(Phase::of(100), Phase::Test);
        assert_eq!(Phase::Plan.label(), "plan");
        assert_eq!(Phase::Impl.label(), "impl");
        assert_eq!(Phase::Test.label(), "test");
    }

    #[test]
    fn statuses_know_whether_they_spend_and_whether_they_need_the_person() {
        let spends = [
            Status::Working,
            Status::Researching {
                for_agent: 1,
                until: 1,
            },
        ];
        let rest = [
            Status::Blocked,
            Status::WaitingHuman,
            Status::Stopped,
            Status::Idle,
            Status::Asking { to: 1, until: 1 },
        ];
        for s in spends {
            assert!(s.busy(), "{s:?}");
        }
        for s in rest {
            assert!(!s.busy(), "{s:?}");
        }
    }

    #[test]
    fn the_trace_keeps_the_newest_lines_only() {
        let mut a = agent(1, Role::Worker, Status::Working);
        for i in 0..=TRACE_KEEP {
            a.trace(0, format!("line {i}"));
        }
        assert_eq!(a.trace.len(), TRACE_KEEP);
        assert!(a.trace.front().unwrap().ends_with("line 1"));
        assert!(
            a.trace
                .back()
                .unwrap()
                .ends_with(&format!("line {TRACE_KEEP}"))
        );
    }

    #[test]
    fn leases_are_held_while_the_change_is_not_merged() {
        let mut c = Card {
            id: 0,
            project: 0,
            title: "t".into(),
            owner: Some(1),
            column: Column::Waiting,
            story: Vec::new(),
            symbols: Vec::new(),
            paths: Vec::new(),
            checks: Vec::new(),
            started: 0,
            finished: 0,
            attempt: 1,
            breaker: Default::default(),
            after: Vec::new(),
        };
        for (column, held, over) in [
            (Column::Waiting, false, false),
            (Column::Working, true, false),
            (Column::Review, false, false),
            (Column::Verifying, true, false),
            (Column::Done, false, true),
            (Column::Failed, false, true),
        ] {
            c.column = column;
            assert_eq!(c.holds_leases(), held, "{column:?}");
            assert_eq!(c.over(), over, "{column:?}");
        }
    }

    #[test]
    fn an_event_names_its_agent() {
        assert_eq!(Event::Failed { agent: 3 }.agent(), 3);
        assert_eq!(Event::Rejected { agent: 4 }.agent(), 4);
        assert_eq!(
            Event::Replied {
                agent: 5,
                text: String::new()
            }
            .agent(),
            5
        );
        let opened = Event::Opened {
            leader: 6,
            path: "/a".into(),
        };
        let closed = Event::Closed { leader: 7 };
        let renamed = Event::Renamed {
            leader: 8,
            from: "a".into(),
            to: "b".into(),
        };
        assert_eq!(
            (opened.agent(), closed.agent(), renamed.agent()),
            (6, 7, 8),
            "the person's project events belong to the project's leader"
        );
        let forgotten = Event::Forgotten { agent: 9 };
        assert_eq!(forgotten.agent(), 9);
        for e in [&opened, &closed, &renamed, &forgotten] {
            assert_eq!(e.kind(), EntryKind::Person);
        }
        let unlisted = Event::CommandsMissing { agent: 11 };
        assert_eq!(
            (unlisted.agent(), unlisted.kind()),
            (11, EntryKind::Assumption)
        );
        let level = Event::Confine {
            leader: 3,
            level: "open".into(),
        };
        assert_eq!((level.agent(), level.kind()), (3, EntryKind::Assumption));
        let no_git = Event::NoGit {
            leader: 12,
            path: "/n".into(),
        };
        assert_eq!((no_git.agent(), no_git.kind()), (12, EntryKind::Assumption));
    }

    #[test]
    fn a_folder_without_git_is_marked_and_said() {
        let mut w = world(vec![]);
        w.projects.clear();
        let p = w.open_project(None, "notes".into(), "/n".into(), Role::Solo);
        let leader = w.projects[p].leader;
        let journal = w.journal.len();
        w.set_no_git(p, None);
        assert!(!w.projects[p].no_git);
        assert_eq!(w.journal.len(), journal, "a checkout: nothing to say");
        w.set_no_git(p, Some("/n"));
        assert!(w.projects[p].no_git);
        assert_eq!(
            w.journal.last().unwrap().event,
            Event::NoGit {
                leader,
                path: "/n".into()
            }
        );
        w.set_no_git(p, None);
        assert!(!w.projects[p].no_git, "a first commit since");
        assert!(no_git_team("/n").starts_with("/n is not a git checkout with a commit"));
    }

    #[test]
    fn opening_a_project_adds_it_with_a_leader_and_reopening_keeps_its_id() {
        let mut w = world(vec![]);
        w.projects.clear();
        w.tick = 9;
        assert_eq!(
            w.open_project(None, "home".into(), "/h".into(), Role::Leader),
            0
        );
        assert_eq!(w.agents[0].name, "lead", "the first project's leader");
        assert_eq!(
            w.open_project(None, "api".into(), "/a".into(), Role::Leader),
            1
        );
        let a = &w.agents[1];
        assert_eq!(
            (
                a.id,
                a.project,
                a.name.as_str(),
                a.role,
                a.status,
                a.changed_at
            ),
            (1, 1, "lead-api", Role::Leader, Status::Idle, 9)
        );
        let p = &w.projects[1];
        assert_eq!(
            (
                p.id,
                p.name.as_str(),
                p.leader,
                p.trunk,
                p.repo.len(),
                p.closed
            ),
            (1, "api", 1, None, 0, false)
        );
        assert!(matches!(
            &w.journal.last().unwrap().event,
            Event::Opened { leader: 1, path } if path == "/a"
        ));
        w.close_project(1);
        assert_eq!(
            w.open_project(Some(1), "api2".into(), "/a".into(), Role::Leader),
            1
        );
        assert_eq!(w.projects.len(), 2, "reopened in place");
        let p = &w.projects[1];
        assert_eq!((p.name.as_str(), p.leader, p.closed), ("api2", 2, false));
        assert_eq!(w.agents[2].name, "lead-api2");
        assert!(w.agents[1].gone, "the old leader stays gone");
    }

    #[test]
    fn closing_a_project_sends_its_agents_away_and_closes_what_waited() {
        let mut w = world(vec![
            agent(0, Role::Leader, Status::Idle),
            agent(1, Role::Worker, Status::Working),
        ]);
        w.open_project(None, "api".into(), "/a".into(), Role::Leader);
        let mut worker = agent(3, Role::Worker, Status::Working);
        worker.project = 1;
        w.agents.push(worker);
        w.tick = 4;
        w.said(3, "hi");
        w.agents[3].activity = Some("Edit x".into());
        w.replied(1, "one");
        w.replied(3, "two");
        for agent in [1, 3] {
            w.change_ready(Change {
                agent,
                title: "t".into(),
                files: Vec::new(),
                patch: String::new(),
                runs: Vec::new(),
                check: Check::None,
                spend: None,
            });
        }
        w.close_project(1);
        for id in [2, 3] {
            let a = &w.agents[id];
            assert!(a.gone, "{id} gone");
            assert_eq!(a.status, Status::Idle, "{id}");
            assert!(a.activity.is_none() && a.talk.is_none(), "{id}");
            assert!(a.trace.back().unwrap().ends_with(" left"), "{id}");
        }
        assert_eq!(w.agents[3].changed_at, 4, "the worker changed now");
        let other = &w.agents[1];
        assert!(!other.gone && other.status == Status::Working);
        let open = w.open_inbox();
        assert_eq!(open.len(), 2, "the other project's items stay");
        assert!(open.iter().all(|&i| w.inbox[i].agent == 1));
        assert_eq!(w.changes.len(), 1);
        assert_eq!(w.changes[0].agent, 1, "the other project's change stays");
        assert!(w.projects[1].closed && !w.projects[0].closed);
        assert!(matches!(
            w.journal.last().unwrap().event,
            Event::Closed { leader: 2 }
        ));
        assert_eq!(w.open_projects().map(|p| p.id).collect::<Vec<_>>(), [0]);
        assert_eq!(w.project_by_name("p"), Some(0));
        assert_eq!(w.project_by_name("api"), None, "closed");
        assert_eq!(w.project_by_name("P"), None, "exact");
    }

    #[test]
    fn renaming_a_project_renames_its_leader_and_says_so() {
        let mut w = world(vec![agent(0, Role::Leader, Status::Idle)]);
        w.rename_project(0, "shop".into());
        assert_eq!(w.projects[0].name, "shop");
        assert_eq!(w.agents[0].name, "lead-shop");
        assert!(matches!(
            &w.journal.last().unwrap().event,
            Event::Renamed { leader: 0, from, to } if from == "p" && to == "shop"
        ));
        assert_eq!(World::head_name(0, "x", Role::Leader), "lead");
        assert_eq!(World::head_name(1, "x", Role::Leader), "lead-x");
        assert_eq!(World::head_name(0, "x", Role::Solo), "solo");
        assert_eq!(World::head_name(2, "x", Role::Solo), "solo-x");
        assert_eq!(
            World::head_name(2, "x", Role::Worker),
            "lead-x",
            "only a solo is solo"
        );
    }

    #[test]
    fn a_free_project_name_numbers_from_two_and_stays_a_project_name() {
        let mut w = world(vec![agent(0, Role::Leader, Status::Idle)]);
        assert_eq!(free_project_name(&w, "shop"), "shop");
        assert_eq!(free_project_name(&w, "p"), "p-2", "p is open");
        w.open_project(None, "p-2".into(), "/r".into(), Role::Solo);
        assert_eq!(free_project_name(&w, "p"), "p-3");
        let long = "a".repeat(24);
        w.open_project(None, long.clone(), "/r".into(), Role::Solo);
        let next = free_project_name(&w, &long);
        assert_eq!(next, format!("{}-2", "a".repeat(22)));
        assert!(valid_project_name(&next));
        w.close_project(1);
        assert_eq!(
            free_project_name(&w, "p"),
            "p-2",
            "a closed one does not count"
        );
    }

    #[test]
    fn a_solo_choice_is_claude_then_a_model_and_or_an_effort() {
        let some = |s: &str| Some(Some(s.to_string()));
        assert_eq!(solo_choice("claude"), Some(None));
        assert_eq!(solo_choice("claude:"), Some(None));
        assert_eq!(solo_choice("claude:opus"), some("opus"));
        assert_eq!(solo_choice("claude@xhigh"), some("@xhigh"));
        assert_eq!(solo_choice("claude:@max"), some("@max"));
        assert_eq!(solo_choice("claude:opus@high"), some("opus@high"));
        for bad in [
            "",
            "codex",
            "gemini:pro",
            "claude:-x",
            "claude:a b",
            "Claude",
            "notes",
            "claudes",
        ] {
            assert_eq!(solo_choice(bad), None, "{bad:?}");
        }
    }

    #[test]
    fn a_model_choice_can_not_pass_as_a_flag() {
        assert!(valid_model("sonnet") && valid_model("opus[1m]") && valid_model("claude-opus-5-5"));
        assert!(valid_model("@low") && valid_model("claude-opus-5-5@xhigh"));
        for bad in [
            "",
            "--dangerously-skip-permissions",
            "a b",
            "x;y",
            "m/../n",
            "claude:x",
        ] {
            assert!(!valid_model(bad), "{bad}");
        }
        assert!(!valid_model(&"a".repeat(65)) && valid_model(&"a".repeat(64)));
    }

    #[test]
    fn a_solo_project_s_head_is_the_solo_agent_and_renames_as_one() {
        let mut w = world(vec![agent(0, Role::Leader, Status::Idle)]);
        let p = w.open_project(None, "notes".into(), "/r".into(), Role::Solo);
        assert_eq!(p, 1);
        let head = w.projects[p].leader;
        let a = &w.agents[head];
        assert_eq!(
            (a.name.as_str(), a.role, a.project, a.status),
            ("solo-notes", Role::Solo, 1, Status::Idle)
        );
        assert!(w.is_solo(1) && !w.is_solo(0));
        w.rename_project(1, "ideas".into());
        assert_eq!(w.agents[head].name, "solo-ideas");
        // A head's stem is its role's; project 0's head is the bare stem
        // until a rename, which names it like any other.
        assert_eq!(World::head_stem(Role::Solo), "solo");
        for r in [Role::Leader, Role::Worker, Role::Helper, Role::Researcher] {
            assert_eq!(World::head_stem(r), "lead");
        }
        w.agents[0].role = Role::Solo;
        w.rename_project(0, "mine".into());
        assert_eq!(w.agents[0].name, "solo-mine");
    }

    #[test]
    fn the_counters_count_what_they_say() {
        let mut gone = agent(3, Role::Worker, Status::Working);
        gone.gone = true;
        let mut gone_researcher = agent(5, Role::Researcher, Status::Working);
        gone_researcher.gone = true;
        let mut w = world(vec![
            agent(0, Role::Leader, Status::Idle),
            agent(1, Role::Worker, Status::Working),
            agent(2, Role::Worker, Status::Idle),
            gone,
            agent(4, Role::Researcher, Status::Working),
            gone_researcher,
            agent(6, Role::Worker, Status::Working),
            agent(7, Role::Researcher, Status::Working),
        ]);
        // Two researchers at work: not the one that is gone.
        assert_eq!(w.active_researchers(), 2);
        assert!(!Status::Asleep.busy());
        w.inbox = vec![
            item(InboxKind::Sensitive, 1, true),
            item(InboxKind::Sensitive, 1, false),
            item(InboxKind::Reply, 2, true),
        ];
        assert_eq!(w.open_inbox(), [0, 2]);
        // An open reply is read, not decided; a closed item is neither.
        assert_eq!(w.open_decisions(), 1);
        w.inbox[1].kind = InboxKind::Reply;
        assert_eq!(w.open_decisions(), 1);
        w.inbox[2].kind = InboxKind::Question;
        assert_eq!(w.open_decisions(), 2);
        w.inbox[1].kind = InboxKind::Sensitive;
        w.inbox[2].kind = InboxKind::Reply;
        let card = |column| Card {
            id: 0,
            project: 0,
            title: "t".into(),
            owner: None,
            column,
            story: Vec::new(),
            symbols: Vec::new(),
            paths: Vec::new(),
            checks: Vec::new(),
            started: 0,
            finished: 0,
            attempt: 1,
            breaker: Default::default(),
            after: Vec::new(),
        };
        w.cards = vec![
            card(Column::Done),
            card(Column::Working),
            card(Column::Done),
        ];
        assert_eq!(
            w.shown_cards().filter(|c| c.column == Column::Done).count(),
            2
        );
    }

    #[test]
    fn mid_turn_is_who_spends_now_and_open_changes_only_open_changes() {
        let mut gone = agent(3, Role::Worker, Status::Working);
        gone.gone = true;
        let researching = Status::Researching {
            for_agent: 1,
            until: 9,
        };
        let mut w = world(vec![
            agent(0, Role::Leader, Status::Working),
            agent(1, Role::Worker, Status::Asking { to: 0, until: 9 }),
            agent(2, Role::Worker, Status::Idle),
            gone,
            agent(4, Role::Researcher, researching),
            agent(5, Role::Worker, Status::Stopped),
            agent(6, Role::Worker, Status::WaitingHuman),
        ]);
        assert_eq!(w.mid_turn(), [0, 4]);
        w.inbox = vec![
            item(InboxKind::Change, 1, true),
            item(InboxKind::Change, 2, false),
            item(InboxKind::Reply, 2, true),
            item(InboxKind::Change, 6, true),
        ];
        assert_eq!(w.open_changes(), [0, 3]);
    }

    #[test]
    fn the_rate_is_what_the_last_ten_minutes_cost_per_hour() {
        assert_eq!(RATE_WINDOW * SECONDS_PER_TICK, 600, "ten minutes");
        let mut w = world(vec![
            agent(0, Role::Leader, Status::Idle),
            agent(1, Role::Worker, Status::Working),
        ]);
        w.watch_spending();
        assert_eq!(w.rate_per_hour(), 0, "nothing spent, no rate");
        assert!(w.spending.seen.is_empty(), "nothing changed, nothing noted");
        w.tick = 10;
        w.agents[0].cost = 60;
        w.agents[1].cost = 40;
        w.watch_spending();
        assert_eq!(w.spending.seen, [(10, 100)]);
        assert_eq!(
            w.rate_per_hour(),
            600,
            "a cent in ten minutes is six an hour"
        );
        // The turn stays in the window until the window starts at it.
        w.tick = RATE_WINDOW + 9;
        assert_eq!(w.rate_per_hour(), 600);
        w.tick = RATE_WINDOW + 10;
        assert_eq!(w.rate_per_hour(), 0, "an idle ten minutes cost nothing");
        w.tick = RATE_WINDOW + 50;
        assert_eq!(
            w.rate_per_hour(),
            0,
            "the window starts from what was spent before it"
        );
        w.watch_spending();
        assert_eq!(
            w.spending.seen,
            [(10, 100)],
            "the start of the window stays"
        );

        // What earlier runs spent is not this run's rate.
        let mut w = world(vec![agent(0, Role::Leader, Status::Idle)]);
        w.tick = 500;
        w.run_spent = 1_000;
        w.agents[0].cost = 1_000;
        w.watch_spending();
        assert_eq!((w.rate_per_hour(), w.spending.seen.len()), (0, 0));
        w.agents[0].cost = 1_300;
        w.watch_spending();
        assert_eq!(w.rate_per_hour(), 1_800);
        w.run_spent = 2_000;
        w.spending.seen.clear();
        assert_eq!(w.rate_per_hour(), 0, "never underflows");
    }

    #[test]
    fn spending_keeps_only_what_the_window_starts_from() {
        let mut w = world(vec![agent(0, Role::Leader, Status::Idle)]);
        for tick in 1..=3 * RATE_WINDOW {
            w.tick = tick;
            w.agents[0].cost = 2 * tick;
            w.watch_spending();
        }
        // Two a tick, 120 ticks in the window: 240 in ten minutes.
        assert_eq!(w.rate_per_hour(), 240 * 6);
        assert_eq!(w.spending.seen.len() as u64, RATE_WINDOW + 1);
        assert_eq!(
            w.spending.seen.front(),
            Some(&(2 * RATE_WINDOW, 4 * RATE_WINDOW))
        );
    }

    #[test]
    fn the_pot_lasts_what_is_left_at_the_rate() {
        let mut w = world(vec![agent(0, Role::Leader, Status::Idle)]);
        w.budget = 10_000;
        assert_eq!(w.pot_lasts(), None, "nothing spent, no end in sight");
        w.tick = 1;
        w.agents[0].cost = 100;
        w.watch_spending();
        // $0.06/h with $0.99 left: 990 minutes.
        assert_eq!(w.pot_lasts(), Some(990));
        assert!(!w.pot_low());
        w.budget = 100 + 300;
        assert_eq!(w.pot_lasts(), Some(30));
        assert!(!w.pot_low(), "half an hour is not low yet");
        w.budget = 100 + 299;
        assert_eq!(w.pot_lasts(), Some(29));
        assert!(w.pot_low());
        w.budget = 100;
        assert_eq!(w.pot_lasts(), None, "a spent pot has no time left to show");
        assert!(!w.pot_low());
        w.budget = 50;
        assert_eq!(w.pot_lasts(), None);
    }

    #[test]
    fn the_journal_says_once_that_the_pot_runs_low() {
        let mut w = world(vec![
            agent(0, Role::Leader, Status::Idle),
            agent(1, Role::Leader, Status::Idle),
        ]);
        w.projects.push(Project {
            id: 1,
            name: "q".into(),
            leader: 1,
            trunk: None,
            files: Vec::new(),
            repo: &[],
            closed: false,
            pinned: false,
            copies: Vec::new(),
            allowed: Vec::new(),
            plan: PlanGate::default(),
            no_git: false,
            lonca: None,
            archived: None,
        });
        w.projects[0].closed = true;
        w.budget = 400;
        w.tick = 1;
        w.agents[0].cost = 100;
        w.watch_spending();
        assert!(w.journal.is_empty(), "thirty minutes left is not low");
        w.budget = 399;
        w.watch_spending();
        w.watch_spending();
        let low = |w: &World| {
            (w.journal.iter())
                .filter(|e| matches!(e.event, Event::PotLow { .. }))
                .count()
        };
        assert_eq!(low(&w), 1, "once");
        assert_eq!(
            w.journal[0].event,
            Event::PotLow {
                leader: 1,
                rate: 600,
                minutes: 29,
            },
            "the first open project's leader"
        );
        assert_eq!(w.journal[0].event.kind(), EntryKind::Sensitive);
        assert_eq!(w.journal[0].event.agent(), 1);
        w.spending.warned = false;
        w.watch_spending();
        assert_eq!(low(&w), 2, "again after a new pot");
        // No open project, nobody to say it to.
        w.projects[1].closed = true;
        w.spending.warned = false;
        w.watch_spending();
        assert_eq!(low(&w), 2);
        assert!(!w.spending.warned);
    }

    #[test]
    fn stirs_count_the_lines_of_each_of_the_last_ticks() {
        let mut s = Stirs::default();
        assert_eq!(s.at(0), [0; STIR_TICKS]);
        s.add(3);
        s.add(3);
        s.add(5);
        assert_eq!(s.at(5), [0, 0, 2, 0, 1]);
        // The ticks go by: the columns move left, empty ones come in.
        assert_eq!(s.at(6), [0, 2, 0, 1, 0]);
        assert_eq!(s.at(7), [2, 0, 1, 0, 0]);
        assert_eq!(s.at(9), [1, 0, 0, 0, 0], "the newest, four ticks on");
        assert_eq!(s.at(10), [0; STIR_TICKS], "five ticks on: all gone");
        assert_eq!(s.at(1_000), [0; STIR_TICKS]);
        // A tick before the newest is not looked back at.
        assert_eq!(s.at(4), [0, 0, 2, 0, 1]);
        // A line from before the newest tick counts as the newest.
        s.add(4);
        assert_eq!(s.at(5), [0, 0, 2, 0, 2]);
        // A line after a gap: the old columns moved, the new one counted.
        s.add(7);
        assert_eq!(s.at(7), [2, 0, 2, 0, 1]);
        s.add(20);
        assert_eq!(s.at(20), [0, 0, 0, 0, 1]);
        // A column does not overflow.
        let mut s = Stirs::default();
        for _ in 0..u16::MAX as u32 + 3 {
            s.add(1);
        }
        assert_eq!(s.at(1), [0, 0, 0, 0, u16::MAX]);
        // The focus's bar keeps two minutes: the row's five are its end.
        // Lines at ticks 1, 1, 4 and 20; at tick `t` the line of tick
        // `k` is in column 23 - (t - k).
        let mut s = Stirs::default();
        for tick in [1, 1, 4, 20] {
            s.add(tick);
        }
        let cols = |pairs: &[(usize, u16)]| {
            let mut c = [0; STIR_LONG];
            for &(i, n) in pairs {
                c[i] = n;
            }
            c
        };
        assert_eq!(s.long(20), cols(&[(4, 2), (7, 1), (23, 1)]));
        assert_eq!(s.at(20), [0, 0, 0, 0, 1]);
        assert_eq!(
            s.long(24),
            cols(&[(0, 2), (3, 1), (19, 1)]),
            "tick 1 the oldest"
        );
        assert_eq!(s.at(24), [1, 0, 0, 0, 0], "tick 20 the row's oldest");
        assert_eq!(s.at(25), [0; STIR_TICKS], "gone from the row");
        assert_eq!(s.long(25), cols(&[(2, 1), (18, 1)]), "still on the focus");
        assert_eq!(s.long(43), cols(&[(0, 1)]));
        assert_eq!(s.long(44), [0; STIR_LONG], "two minutes on: all gone");
        // Every trace line is counted at its tick.
        let mut a = agent(1, Role::Worker, Status::Working);
        a.trace(8, "tool_use Read");
        a.trace(9, "text \"hm\"");
        a.trace(9, "tool_use Edit");
        assert_eq!(a.stirs.at(9), [0, 0, 0, 1, 2]);
    }

    #[test]
    fn what_waits_on_the_person_holds_mail_and_what_a_word_ends() {
        use InboxKind::*;
        for (kind, holds, word) in [
            (Sensitive, true, false),
            (Research, true, false),
            (Reply, false, true),
            (Change, false, false),
            (Question, true, true),
            (Approval, true, false),
            (Fault, true, true),
            (Script, true, false),
            (Check, false, false),
            (Plan, true, true),
            (Tries, false, false),
            (Rounds, true, false),
        ] {
            assert_eq!(kind.holds_mail(), holds, "{kind:?}");
            assert_eq!(kind.closes_on_word(), word, "{kind:?}");
        }
    }

    #[test]
    fn an_agent_waits_on_the_person_on_an_open_item_of_its_own_or_its_status() {
        let mut w = world(vec![
            agent(0, Role::Worker, Status::Idle),
            agent(1, Role::Worker, Status::Idle),
        ]);
        assert!(!w.waits_on_person(0) && !w.word_frees(0));
        w.inbox = vec![
            item(InboxKind::Reply, 0, true),
            item(InboxKind::Question, 0, false),
            item(InboxKind::Change, 0, true),
            item(InboxKind::Sensitive, 1, true),
        ];
        assert!(!w.waits_on_person(0) && !w.word_frees(0), "nothing of its");
        assert!(
            w.waits_on_person(1) && !w.word_frees(1),
            "a word leaves this"
        );
        w.agents[0].status = Status::WaitingHuman;
        assert!(w.waits_on_person(0));
        w.agents[0].status = Status::Idle;
        // A word frees it only when it ends all that holds it.
        w.inbox.push(item(InboxKind::Question, 0, true));
        assert!(w.waits_on_person(0) && w.word_frees(0));
        w.inbox.push(item(InboxKind::Fault, 0, true));
        assert!(w.word_frees(0));
        w.inbox.push(item(InboxKind::Approval, 0, true));
        assert!(!w.word_frees(0), "the last one too");
    }

    #[test]
    fn the_trace_keeps_its_tail_and_counts_every_line() {
        let mut a = agent(1, Role::Worker, Status::Working);
        for i in 0..(TRACE_KEEP as u64 + 5) {
            a.trace(0, format!("line {i}"));
        }
        assert_eq!(a.trace.len(), TRACE_KEEP);
        assert_eq!(a.traced, TRACE_KEEP as u64 + 5);
        assert!(a.trace[0].ends_with("line 5"), "{}", a.trace[0]);
    }

    #[test]
    fn an_interrupted_turn_is_evidence_on_the_card_and_a_failure_in_the_journal() {
        assert_eq!(StoryEvent::Interrupted.kind(), StoryKind::Evidence);
        let e = Event::Interrupted { agent: 3 };
        assert_eq!(e.kind(), EntryKind::Failed);
        assert_eq!(e.agent(), 3);
    }

    #[test]
    fn approving_and_rejecting_close_the_item_and_tell_the_agent() {
        let mut w = world(vec![
            agent(0, Role::Leader, Status::Idle),
            agent(1, Role::Worker, Status::WaitingHuman),
        ]);
        w.agents[1].progress = 50;
        w.inbox = vec![
            item(InboxKind::Research, 1, true),
            item(InboxKind::Sensitive, 1, true),
            item(InboxKind::Research, 1, true),
        ];
        w.approve(0);
        assert!(!w.inbox[0].open);
        assert!(matches!(
            w.journal.last().unwrap().event,
            Event::Approved {
                agent: 1,
                research: true
            }
        ));
        assert_eq!(w.agents[1].status, Status::Working);
        w.agents[1].status = Status::WaitingHuman;
        w.reject(1);
        assert!(!w.inbox[1].open);
        assert_eq!(w.agents[1].progress, 40, "a rejected change loses ground");
        assert!(matches!(
            w.journal.last().unwrap().event,
            Event::Rejected { agent: 1 }
        ));
        assert_eq!(w.agents[1].status, Status::Working);
        w.reject(2);
        assert_eq!(
            w.agents[1].progress, 40,
            "a rejected report costs no ground"
        );
        // Closed is closed: nothing more happens.
        let before = w.journal.len();
        w.approve(0);
        w.reject(1);
        assert_eq!(w.journal.len(), before);
    }

    #[test]
    fn what_the_person_says_shows_for_a_moment() {
        let mut w = world(vec![agent(0, Role::Leader, Status::Idle)]);
        w.tick = 3;
        w.said(0, "hello");
        let talk = w.agents[0].talk.as_ref().unwrap();
        assert_eq!(talk.with, Party::Person);
        assert!(!talk.mine, "the person's words, not the agent's");
        assert_eq!(talk.text, "hello");
    }

    #[test]
    fn closing_replies_touches_only_that_agents_replies() {
        let mut w = world(vec![
            agent(0, Role::Leader, Status::Idle),
            agent(1, Role::Worker, Status::Idle),
        ]);
        w.inbox = vec![
            item(InboxKind::Reply, 1, true),
            item(InboxKind::Reply, 0, true),
            item(InboxKind::Sensitive, 1, true),
        ];
        w.close_replies(1);
        let open: Vec<bool> = w.inbox.iter().map(|i| i.open).collect();
        assert_eq!(open, [false, true, true]);
    }

    #[test]
    fn a_workers_cards_follow_its_change_to_review_back_to_work_and_done() {
        use crate::live::testing::team;
        let mut w = team(2);
        let card = |w: &mut World, owner, column| {
            let id = w.cards.len();
            w.cards.push(Card {
                id,
                project: 0,
                title: format!("c{id}"),
                owner: Some(owner),
                column,
                story: Vec::new(),
                symbols: Vec::new(),
                paths: Vec::new(),
                checks: Vec::new(),
                started: 0,
                finished: 0,
                attempt: 1,
                breaker: Default::default(),
                after: Vec::new(),
            });
            id
        };
        let a = card(&mut w, 1, Column::Working);
        let b = card(&mut w, 1, Column::Working);
        let theirs = card(&mut w, 2, Column::Working);
        let done = card(&mut w, 1, Column::Done);
        w.cards[done].finished = 5;
        let columns = |w: &World| -> Vec<Column> { w.cards.iter().map(|c| c.column).collect() };
        w.change_ready(change(1, "Fix", &[("a.rs", 1, 0)]));
        use Column::*;
        assert_eq!(columns(&w), [Review, Review, Working, Done]);
        w.change_rejected(1);
        assert_eq!(columns(&w), [Working, Working, Working, Done], "sent back");
        w.change_ready(change(1, "Fix", &[("a.rs", 1, 0)]));
        w.tick = 40;
        w.change_applied(1, "abc".into(), false);
        assert_eq!(
            columns(&w),
            [Review, Review, Working, Done],
            "a part: the rest waits"
        );
        assert_eq!(w.cards[a].finished, 0);
        w.change_applied(1, "abc".into(), true);
        assert_eq!(columns(&w), [Done, Done, Working, Done]);
        assert_eq!((w.cards[a].finished, w.cards[b].finished), (40, 40));
        assert_eq!(
            w.cards[done].finished, 5,
            "one through before stays as it was"
        );
        // A done with nothing in it closes what is at work or waits.
        let c = card(&mut w, 2, Column::Review);
        w.tick = 50;
        w.change_none(2);
        assert_eq!((w.cards[theirs].column, w.cards[c].column), (Done, Done));
        assert_eq!((w.cards[theirs].finished, w.cards[c].finished), (50, 50));
    }

    #[test]
    fn a_card_back_at_work_has_lost_the_leases_another_took_but_not_its_paths() {
        use crate::live::testing::{team, with_card};
        let mut w = team(2);
        let s = |name| Symbol { file: "a.rs", name };
        let a = with_card(&mut w, 1);
        w.cards[a].symbols = vec![s("A"), s("B")];
        w.cards[a].paths = vec![Owned::new("a.rs")];
        w.change_ready(change(1, "Fix", &[("a.rs", 1, 0)]));
        let b = with_card(&mut w, 2);
        w.cards[b].symbols = vec![s("B")];
        // Another project's lease is not in the way.
        let other = with_card(&mut w, 2);
        w.cards[other].project = 1;
        w.cards[other].symbols = vec![s("A")];
        w.change_rejected(1);
        assert_eq!(w.cards[a].symbols, [s("A")]);
        assert_eq!(w.cards[a].paths, [Owned::new("a.rs")]);
        // Through, it keeps what it had.
        w.cards[a].symbols = vec![s("A"), s("B")];
        w.change_ready(change(1, "Fix", &[("a.rs", 1, 0)]));
        w.change_applied(1, "abc".into(), true);
        assert_eq!(w.cards[a].symbols, [s("A"), s("B")]);
    }

    #[test]
    fn a_proposal_moves_its_cards_by_what_became_of_it() {
        use crate::live::testing::{team, with_card};
        use Column::*;
        let cases = [
            (Landed::Queued, Verifying),
            (Landed::Integrated("c1".into()), Done),
            (Landed::Conflicted(Vec::new()), Working),
            (Landed::Rejected(Vec::new()), Working),
            (Landed::Withdrawn, Working),
            (Landed::Replaced, Working),
        ];
        for (how, to) in cases.clone() {
            // Left the waiting list: from review.
            let mut w = team(1);
            let c = with_card(&mut w, 1);
            let queued = with_card(&mut w, 1);
            w.cards[queued].column = Verifying;
            w.cards[c].column = Review;
            w.tick = 9;
            w.proposal_left(1, how.clone());
            assert_eq!(w.cards[c].column, to, "{how:?}");
            // A conflict or a no is a round sent back (record 126, F9).
            let back = matches!(how, Landed::Conflicted(_) | Landed::Rejected(_));
            assert_eq!(w.cards[c].breaker.sent_back, u32::from(back), "{how:?}");
            assert_eq!(w.cards[queued].breaker.sent_back, 0, "{how:?}");
            assert_eq!(
                w.cards[c].finished,
                if to == Done { 9 } else { 0 },
                "{how:?}"
            );
            assert_eq!(
                w.cards[queued].column, Verifying,
                "{how:?}: the queue's own"
            );
        }
        for (how, to) in cases {
            // What the queue made of it: from the queue.
            let mut w = team(1);
            let c = with_card(&mut w, 1);
            let review = with_card(&mut w, 1);
            w.cards[c].column = Verifying;
            w.cards[review].column = Review;
            w.proposal_landed(1, how.clone());
            assert_eq!(w.cards[c].column, to, "{how:?}");
            let back = matches!(how, Landed::Conflicted(_) | Landed::Rejected(_));
            assert_eq!(w.cards[c].breaker.sent_back, u32::from(back), "{how:?}");
            assert_eq!(w.cards[review].column, Review, "{how:?}: a newer one waits");
        }
    }

    #[test]
    fn a_reject_sends_back_every_card_in_review_a_round_each() {
        use crate::live::testing::{team, with_card};
        let mut w = team(2);
        let (a, b, other) = (
            with_card(&mut w, 1),
            with_card(&mut w, 1),
            with_card(&mut w, 2),
        );
        for c in [a, b, other] {
            w.cards[c].column = Column::Review;
        }
        w.cards[b].breaker.sent_back = 2;
        w.change_rejected(1);
        let back: Vec<u32> = [a, b, other].map(|c| w.cards[c].breaker.sent_back).to_vec();
        assert_eq!(back, [1, 3, 0]);
        w.change_rejected(1);
        assert_eq!(w.cards[a].breaker.sent_back, 1, "none in review now");
    }

    #[test]
    fn a_card_s_tries_and_rounds_run_out_at_three_and_one_more_each_the_person_allows() {
        use crate::live::testing::{team, with_card};
        let mut w = team(1);
        let c = with_card(&mut w, 1);
        let card = |w: &World| (w.cards[c].tries_out(), w.cards[c].rounds_out());
        w.cards[c].attempt = 2;
        w.cards[c].breaker.sent_back = 2;
        assert_eq!(card(&w), (false, false));
        w.cards[c].attempt = 3;
        assert_eq!(card(&w), (true, false), "the third try is the last");
        w.cards[c].breaker.sent_back = 3;
        assert_eq!(card(&w), (true, true), "the third time back is the last");
        w.cards[c].breaker.more_tries = 1;
        assert_eq!(card(&w), (false, true));
        w.cards[c].breaker.more_rounds = 2;
        assert_eq!(card(&w), (false, false));
        w.cards[c].attempt = 4;
        w.cards[c].breaker.sent_back = 5;
        assert_eq!(card(&w), (true, true));
        assert_eq!((TRIES, ROUNDS), (3, 3));
        // The card of a worker's at work whose rounds are out.
        assert_eq!(w.rounds_out_of(1), Some(c));
        assert_eq!(w.rounds_out_of(2), None, "not its owner");
        w.cards[c].breaker.more_rounds = 3;
        assert_eq!(w.rounds_out_of(1), None, "a round left");
        w.cards[c].breaker.more_rounds = 2;
        w.cards[c].column = Column::Failed;
        assert_eq!(w.rounds_out_of(1), None, "not at work");
    }

    #[test]
    fn a_cap_opens_one_item_for_the_person_and_their_answer_moves_the_card() {
        use crate::live::testing::{team, with_card};
        let mut w = team(2);
        let (c, d) = (with_card(&mut w, 1), with_card(&mut w, 2));
        w.cards[c].attempt = 3;
        w.cards[c].breaker.sent_back = 2;
        w.cards[c].column = Column::Failed;
        w.tick = 40;
        assert!(w.capped(Cap::Tries, 0, c));
        assert!(!w.capped(Cap::Tries, 0, c), "one open at a time");
        let it = w.inbox.last().unwrap();
        assert_eq!(
            (
                it.kind,
                it.agent,
                it.card,
                it.open,
                it.tick,
                it.detail.as_str()
            ),
            (
                InboxKind::Tries,
                0,
                Some(c),
                true,
                40,
                w.cards[c].title.as_str()
            )
        );
        assert!(matches!(
            w.cards[c].story.last().unwrap().event,
            StoryEvent::Capped {
                cap: Cap::Tries,
                count: 3
            }
        ));
        let capped = Event::Capped {
            agent: 0,
            card: c,
            cap: Cap::Tries,
            count: 3,
        };
        assert_eq!(w.journal.last().unwrap().event, capped);
        assert_eq!(
            (capped.kind(), capped.agent(), capped.card()),
            (EntryKind::Blocked, 0, Some(c))
        );
        // Another cap or card has its own; rounds count what came back.
        assert!(w.capped(Cap::Rounds, 1, c));
        assert!(matches!(
            w.cards[c].story.last().unwrap().event,
            StoryEvent::Capped {
                cap: Cap::Rounds,
                count: 2
            }
        ));
        assert!(w.capped(Cap::Tries, 0, d));
        let items: Vec<usize> = (0..w.inbox.len()).collect();
        let [tries, rounds, other] = items[..] else {
            panic!("three items")
        };
        // Yes: one more try; the item closes, a second answer is nothing.
        assert_eq!(w.decide_cap(tries, true), Some((0, c, Cap::Tries)));
        assert_eq!(w.decide_cap(tries, true), None, "closed");
        assert_eq!(w.cards[c].breaker.more_tries, 1);
        assert_eq!(w.cards[c].breaker.more_rounds, 0);
        let answered = Event::CapAnswered {
            agent: 0,
            card: c,
            cap: Cap::Tries,
            yes: true,
        };
        assert_eq!(w.journal.last().unwrap().event, answered);
        assert_eq!(
            (answered.kind(), answered.agent(), answered.card()),
            (EntryKind::Person, 0, Some(c))
        );
        assert!(matches!(
            w.cards[c].story.last().unwrap().event,
            StoryEvent::CapAnswered {
                cap: Cap::Tries,
                yes: true
            }
        ));
        // Yes to a round: one more round.
        w.cards[c].column = Column::Working;
        assert_eq!(w.decide_cap(rounds, true), Some((1, c, Cap::Rounds)));
        assert_eq!(
            (
                w.cards[c].breaker.more_tries,
                w.cards[c].breaker.more_rounds
            ),
            (1, 1)
        );
        assert_eq!(w.cards[c].column, Column::Working);
        // No: failed for good, now.
        w.tick = 50;
        assert_eq!(w.decide_cap(other, false), Some((0, d, Cap::Tries)));
        let card = &w.cards[d];
        assert_eq!(
            (card.column, card.finished, card.breaker.stopped),
            (Column::Failed, 50, true)
        );
        assert_eq!(card.breaker.more_tries, 0);
        assert!(!w.cards[c].breaker.stopped);
        // An item of another kind, or none, is no cap.
        w.inbox.push(item(InboxKind::Plan, 0, true));
        assert_eq!(w.decide_cap(w.inbox.len() - 1, true), None);
        let mut cardless = item(InboxKind::Rounds, 1, true);
        cardless.card = None;
        w.inbox.push(cardless);
        assert_eq!(w.decide_cap(w.inbox.len() - 1, true), None);
        assert_eq!(w.decide_cap(99, true), None);
        assert_eq!(
            (
                Cap::of(InboxKind::Tries),
                Cap::of(InboxKind::Rounds),
                Cap::of(InboxKind::Plan)
            ),
            (Some(Cap::Tries), Some(Cap::Rounds), None)
        );
        assert_eq!(
            (Cap::Tries.kind(), Cap::Rounds.kind()),
            (InboxKind::Tries, InboxKind::Rounds)
        );
        assert!(w.capped(Cap::Tries, 0, c), "one answered is no longer open");
    }

    #[test]
    fn a_done_is_for_the_card_its_turn_took_and_late_once_it_was_handed_again() {
        use crate::live::testing::{team, with_card};
        let mut w = team(2);
        assert_eq!(w.done_for(1), DoneFor::Nothing);
        let c = with_card(&mut w, 1);
        w.agents[1].attempt = 1;
        assert_eq!(w.done_for(1), DoneFor::Card(c));
        w.cards[c].attempt = 2;
        assert_eq!(w.done_for(1), DoneFor::Late(c), "a try since");
        w.takes(1, c);
        assert_eq!((w.agents[1].card, w.agents[1].attempt), (Some(c), 2));
        assert_eq!(w.done_for(1), DoneFor::Card(c));
        // Handed to another: its next try, from whoever had it.
        w.cards[c].column = Column::Failed;
        w.cards[c].finished = 7;
        w.hand(c, 2);
        let card = &w.cards[c];
        assert_eq!(
            (card.owner, card.attempt, card.column, card.finished),
            (Some(2), 3, Column::Working, 0)
        );
        assert_eq!(w.done_for(1), DoneFor::Late(c));
        assert_eq!(w.done_for(2), DoneFor::Nothing, "taken with its turn");
        w.takes(2, c);
        assert_eq!(w.done_for(2), DoneFor::Card(c));
        assert_eq!(
            card_story(&w, c).last(),
            Some(&StoryEvent::Handed {
                from: Some(1),
                to: 2,
                attempt: 3
            })
        );
        let e = Event::Handed {
            agent: 2,
            card: c,
            from: Some(1),
            attempt: 3,
        };
        assert_eq!(w.journal.last().unwrap().event, e);
        assert_eq!(
            (e.kind(), e.card(), e.agent()),
            (EntryKind::Assign, Some(c), 2)
        );
        // A late done says so, and closes nothing.
        w.late_done(1, c);
        assert_eq!(
            card_story(&w, c).last(),
            Some(&StoryEvent::LateDone { from: 1 })
        );
        let e = Event::LateDone { agent: 1, card: c };
        assert_eq!(w.journal.last().unwrap().event, e);
        assert_eq!(
            (e.kind(), e.card(), e.agent()),
            (EntryKind::Assumption, Some(c), 1)
        );
        assert_eq!(w.cards[c].column, Column::Working);
        // One that failed is through, with its why.
        w.tick = 30;
        w.card_failed(2, c, "Cannot");
        assert_eq!(
            (w.cards[c].column, w.cards[c].finished),
            (Column::Failed, 30)
        );
        assert_eq!(
            card_story(&w, c).last(),
            Some(&StoryEvent::Failed("Cannot".into()))
        );
        let e = Event::CardFailed {
            agent: 2,
            card: c,
            why: "Cannot".into(),
        };
        assert_eq!(w.journal.last().unwrap().event, e);
        assert_eq!(
            (e.kind(), e.card(), e.agent()),
            (EntryKind::Failed, Some(c), 2)
        );
        for (event, kind) in [
            (
                StoryEvent::Handed {
                    from: None,
                    to: 1,
                    attempt: 1,
                },
                StoryKind::Intent,
            ),
            (StoryEvent::Failed(String::new()), StoryKind::Evidence),
            (StoryEvent::LateDone { from: 1 }, StoryKind::Evidence),
        ] {
            assert_eq!(event.kind(), kind, "{event:?}");
        }
    }

    fn card_story(w: &World, c: CardId) -> Vec<StoryEvent> {
        w.cards[c].story.iter().map(|s| s.event.clone()).collect()
    }

    #[test]
    fn a_figure_reads_api_equivalent_on_a_plan_and_never_zero_when_unknown() {
        assert_eq!(money(12_300, false, false), "$1.23");
        assert_eq!(money(12_300, false, true), "≈$1.23");
        assert_eq!(money(12_300, true, false), "$1.23+?");
        assert_eq!(money(12_300, true, true), "≈$1.23+?");
        assert_eq!(money(0, true, true), "?", "nothing known");
        assert_eq!(money(0, false, false), "$0.00", "nothing spent");
        let mut w = crate::live::testing::team(2);
        w.agents[1].cost = 10_000;
        w.agents[2].cost = 2_000;
        assert_eq!(w.money(5_000, false), "$0.50");
        w.estimated = true;
        assert_eq!(w.money(5_000, false), "≈$0.50");
        assert_eq!(
            (w.spent_by(1), w.spent_words()),
            ("≈$1.00".into(), "≈$1.20".into())
        );
        assert!(!w.unpriced());
        // One agent's gap makes its figure, its project's and the pot's
        // what they came to at least; the others' stay whole.
        w.agents[2].unpriced.gaps = 1;
        assert!(w.unpriced());
        assert_eq!(w.spent_by(1), "≈$1.00");
        assert_eq!(w.spent_by(2), "≈$0.20+?");
        assert_eq!(w.spent_words(), "≈$1.20+?");
        assert_eq!(w.project_spent(0), "≈$1.20+?");
        w.agents[2].unpriced = Unpriced::default();
        w.agents[2].unpriced.session = true;
        assert_eq!(w.spent_by(2), "≈$0.20+?", "a session's own gap too");
        // An agent of another project counts in its own alone.
        w.agents[2].project = 1;
        w.agents[2].cost = 0;
        assert_eq!(w.project_spent(0), "≈$1.00");
        assert_eq!(w.project_spent(1), "?");
    }

    #[test]
    fn a_turn_unpriced_stays_so_until_a_whole_figure_and_a_session_s_gap_is_kept_at_its_end() {
        let mut w = crate::live::testing::team(1);
        w.turn_unpriced(1, Some("claude-new"));
        assert_eq!(
            w.agents[1].unpriced,
            Unpriced {
                said: true,
                session: false,
                gaps: 0
            }
        );
        assert!(
            (w.agents[1].trace.back().unwrap())
                .ends_with("cost unknown: claude-new used tokens priced at nothing")
        );
        w.turn_priced(1);
        assert_eq!(
            w.agents[1].unpriced,
            Unpriced {
                said: false,
                session: true,
                gaps: 0
            }
        );
        // A whole figure: the session's figure covers what came before.
        w.turn_priced(1);
        assert!(!w.agents[1].unpriced.any());
        // A session ends: whole and not cut, nothing; cut, or with its
        // last turn unpriced, or one said and not yet booked, a gap each.
        w.session_over(1, false);
        assert_eq!(w.agents[1].unpriced.gaps, 0);
        w.session_over(1, true);
        assert_eq!(w.agents[1].unpriced.gaps, 1);
        w.agents[1].unpriced.session = true;
        w.session_over(1, false);
        w.agents[1].unpriced.said = true;
        w.session_over(1, false);
        assert_eq!(
            w.agents[1].unpriced,
            Unpriced {
                said: false,
                session: false,
                gaps: 3
            }
        );
    }

    #[test]
    fn a_round_is_unpriced_when_its_agent_or_a_helper_left_a_gap_since_it_opened() {
        use crate::live::testing::{team, with_helper};
        let mut w = team(1);
        let h = with_helper(&mut w, 1);
        let settle = |w: &mut World| {
            w.change_ready(change(1, "Fix", &[("a.rs", 1, 0)]));
            w.settle_change(1);
            let s = w.change_of(1).unwrap().spend.unwrap();
            w.change_applied(1, "m".into(), true);
            s.unpriced
        };
        // Gaps from before the round are not its own.
        w.agents[1].unpriced.gaps = 2;
        w.round_opens(1);
        assert_eq!(w.agents[1].task.gaps, 2);
        assert!(!settle(&mut w), "as many as when it opened");
        w.round_opens(1);
        w.agents[1].unpriced.gaps = 3;
        assert!(settle(&mut w), "one more since");
        w.round_opens(1);
        w.agents[1].unpriced.session = true;
        assert!(settle(&mut w), "its session's last turn");
        w.agents[1].unpriced.session = false;
        w.round_opens(1);
        w.round_opens(h);
        w.agents[h].unpriced.gaps = 1;
        assert!(settle(&mut w), "its helper's");
        // Both: still unpriced.
        w.round_opens(1);
        w.round_opens(h);
        w.agents[1].unpriced.session = true;
        w.agents[h].unpriced.gaps = 2;
        assert!(settle(&mut w), "both");
        w.agents[1].unpriced.session = false;
        w.round_opens(1);
        w.agents[h].unpriced.session = true;
        assert!(
            !settle(&mut w),
            "a helper with no round open is not counted"
        );
    }

    #[test]
    fn a_change_s_cost_is_settled_at_the_turn_s_end_carried_unapplied_and_dropped_once_applied() {
        use crate::live::testing::{team, with_helper};
        let mut w = team(1);
        let h = with_helper(&mut w, 1);
        // A leader's spend is nobody's change.
        w.round_opens(0);
        assert_eq!(w.agents[0].task.round, None);
        // A round opens at the first prompt, once: a later one keeps it.
        w.tick = 10;
        w.agents[1].cost = 1_000;
        w.round_opens(1);
        assert_eq!(w.agents[1].task.round, Some((1_000, 10)));
        w.tick = 12;
        w.agents[1].cost = 1_500;
        w.round_opens(1);
        assert_eq!(w.agents[1].task.round, Some((1_000, 10)), "kept");
        // The helper's own round, from its cost then.
        w.tick = 20;
        w.agents[h].cost = 200;
        w.round_opens(h);
        w.agents[h].cost = 500;
        // Read mid-turn: what it cost is not known yet.
        w.tick = 50;
        w.agents[1].cost = 9_400;
        w.change_ready(change(1, "Fix", &[("a.rs", 1, 0)]));
        assert_eq!(w.change_of(1).unwrap().spend, None);
        assert!(matches!(
            w.journal.last().unwrap().event,
            Event::ChangeReady {
                agent: 1,
                spend: None,
                ..
            }
        ));
        // The turn ends: the worker's cost since its round opened, its
        // helper's since its own, the worker's ticks, one round.
        w.tick = 70;
        w.agents[1].cost = 10_000;
        assert!(w.settle_change(1));
        let s = Spend {
            cost: 9_000 + 300,
            ticks: 60,
            rounds: 1,
            unpriced: false,
        };
        assert_eq!(w.change_of(1).unwrap().spend, Some(s));
        let last = w.journal.last().unwrap();
        assert_eq!(last.event, Event::ChangeCost { agent: 1, spend: s });
        assert_eq!(last.event.kind(), EntryKind::Assumption);
        assert_eq!(last.event.agent(), 1);
        assert_eq!(w.agents[1].task.round, None, "closed");
        assert_eq!(w.agents[h].task.round, None, "the helper's too");
        assert_eq!(w.agents[1].task.last, Some(s));
        assert!(!w.settle_change(1), "settled once");
        // Rejected: carried into the next change, which adds its round.
        w.change_rejected(1);
        assert_eq!(w.agents[1].task.carried, s);
        w.tick = 100;
        w.round_opens(1);
        w.tick = 130;
        w.agents[1].cost = 10_500;
        // Read with the turn over: settled as it is read, in one line.
        w.agents[1].status = Status::Idle;
        let journaled = w.journal.len();
        w.change_ready(change(1, "Fix, again", &[("a.rs", 2, 0)]));
        let again = Spend {
            cost: 9_800,
            ticks: 90,
            rounds: 2,
            unpriced: false,
        };
        assert_eq!(w.change_of(1).unwrap().spend, Some(again));
        assert_eq!(w.journal.len(), journaled + 1);
        assert!(matches!(
            w.journal.last().unwrap().event,
            Event::ChangeReady { agent: 1, spend: Some(x), .. } if x == again
        ));
        // A newer change takes the older's place and its cost along.
        w.tick = 131;
        w.round_opens(1);
        w.tick = 133;
        w.agents[1].cost = 10_600;
        w.change_ready(change(1, "Fix, more", &[("a.rs", 3, 0)]));
        let more = Spend {
            cost: 9_900,
            ticks: 92,
            rounds: 3,
            unpriced: false,
        };
        assert_eq!(w.change_of(1).unwrap().spend, Some(more));
        // Part applied: the rest, read again, costs what the whole did.
        w.change_applied(1, "m1".into(), false);
        assert_eq!(w.agents[1].task.carried, more);
        w.change_ready(change(1, "Fix, more", &[("a.rs", 1, 0)]));
        assert_eq!(w.change_of(1).unwrap().spend, Some(more), "no round open");
        // Applied whole: nothing carried on.
        w.change_applied(1, "m2".into(), true);
        assert_eq!(w.agents[1].task.carried, Spend::default());
        assert_eq!(
            w.agents[1].task.last,
            Some(more),
            "the leader's mail still says"
        );
        // The worktree holds nothing: nothing carried, no figure for the mail.
        w.agents[1].task.carried = s;
        w.change_none(1);
        assert_eq!(w.agents[1].task, Task::default());
        assert_eq!(
            Spend {
                cost: 1,
                ticks: 2,
                rounds: 3,
                unpriced: false,
            }
            .plus(Spend {
                cost: 10,
                ticks: 20,
                rounds: 30,
                unpriced: true,
            }),
            Spend {
                cost: 11,
                ticks: 22,
                rounds: 33,
                unpriced: true,
            },
            "a part unpriced leaves the whole unpriced"
        );
    }

    fn change(agent: AgentId, title: &str, files: &[(&str, u32, u32)]) -> Change {
        Change {
            agent,
            title: title.into(),
            files: files
                .iter()
                .map(|&(path, added, removed)| ChangedFile {
                    path: path.into(),
                    added,
                    removed,
                    binary: false,
                })
                .collect(),
            patch: "diff --git a/x b/x\n".into(),
            runs: Vec::new(),
            check: Check::None,
            spend: None,
        }
    }

    fn review(patch: &str, text: &str) -> Review {
        Review {
            patch: patch_sum(patch),
            model: "gpt-6-luna".into(),
            text: text.into(),
            cost: 7,
        }
    }

    #[test]
    fn a_patch_s_sum_is_the_start_of_its_sha_256() {
        assert_eq!(patch_sum(""), "e3b0c44298fc1c14");
        assert_eq!(patch_sum("abc"), "ba7816bf8f01cfea");
        assert_ne!(patch_sum("abc\n"), patch_sum("abc"));
    }

    #[test]
    fn a_review_is_kept_on_the_change_it_read_and_journaled_either_way() {
        let mut w = world(vec![
            agent(0, Role::Leader, Status::Idle),
            agent(1, Role::Worker, Status::Idle),
            agent(2, Role::Worker, Status::Idle),
        ]);
        w.change_ready(change(1, "Fix", &[("x", 1, 0)]));
        let read = review("diff --git a/x b/x\n", "1. x: no test.");
        w.reviewed(1, read.clone());
        assert_eq!(w.review_of(1), Some(&read));
        assert_eq!(
            w.journal.last().unwrap().event,
            Event::Reviewed {
                agent: 1,
                model: "gpt-6-luna".into(),
                patch: read.patch.clone(),
                text: "1. x: no test.".into(),
                cost: 7,
            }
        );
        // Of another patch (the worker's newer change): journaled, not kept.
        w.change_ready(change(2, "Add", &[("y", 2, 0)]));
        w.reviewed(2, review("another patch", "late"));
        assert_eq!(w.review_of(2), None);
        assert!(matches!(
            w.journal.last().unwrap().event,
            Event::Reviewed { agent: 2, .. }
        ));
        // Of no change at all: the same.
        w.reviewed(0, review("diff --git a/x b/x\n", "none waits"));
        assert_eq!(w.review_of(0), None);
        assert!(!w.reviews.contains_key(&0));
        // A newer change of the worker's drops it, as applying or
        // rejecting does.
        w.change_ready(change(1, "Fix again", &[("x", 2, 0)]));
        assert_eq!(w.review_of(1), None);
        w.reviewed(1, read.clone());
        w.change_rejected(1);
        assert_eq!(w.review_of(1), None);
        assert!(w.reviews.is_empty());
        w.change_ready(change(1, "Fix", &[("x", 1, 0)]));
        w.reviewed(1, read.clone());
        w.change_applied(1, "abc".into(), true);
        assert!(w.reviews.is_empty());
    }

    #[test]
    fn a_remembered_change_gets_the_newest_review_of_its_own_patch_back() {
        let mut w = world(vec![
            agent(0, Role::Leader, Status::Idle),
            agent(1, Role::Worker, Status::Idle),
            agent(2, Role::Worker, Status::Idle),
        ]);
        w.change_ready(change(1, "Fix", &[("x", 1, 0)]));
        w.change_ready(change(2, "Add", &[("y", 1, 0)]));
        let mine = "diff --git a/x b/x\n";
        w.reviewed(1, review(mine, "older"));
        w.reviewed(1, review(mine, "newer"));
        w.reviewed(1, review("not mine", "other patch"));
        w.reviewed(0, review(mine, "another agent"));
        // As a load finds it: the journal, no reviews.
        w.reviews.clear();
        w.reviews_from_journal();
        assert_eq!(w.review_of(1).map(|r| r.text.as_str()), Some("newer"));
        assert_eq!(w.review_of(2), None, "none of its own");
        assert_eq!(w.reviews.len(), 1);
        // One kept is not taken over by the journal's.
        w.reviews.insert(1, review(mine, "kept"));
        w.reviews_from_journal();
        assert_eq!(w.review_of(1).map(|r| r.text.as_str()), Some("kept"));
    }

    #[test]
    fn the_question_is_the_newest_ask_in_the_journal_with_its_options_else_the_item_s_line() {
        let mut w = world(vec![
            agent(0, Role::Leader, Status::Idle),
            agent(1, Role::Worker, Status::Idle),
            agent(2, Role::Worker, Status::Idle),
        ]);
        assert_eq!(w.question_of(1), None, "no question");
        let ask = |w: &mut World, from, text: &str, options: &[&str]| {
            w.note(Event::ToYou {
                from,
                kind: MsgKind::Ask,
                text: text.into(),
                options: options.iter().map(|o| o.to_string()).collect(),
            });
            w.asked_you(from, text);
        };
        ask(&mut w, 1, "Which?\nMore.", &["a", "b"]);
        let q = w.question_of(1).unwrap();
        assert_eq!(q.text, "Which?\nMore.");
        assert_eq!(q.options, ["a", "b"]);
        // Another agent's ask and a later inform of its own change nothing.
        ask(&mut w, 2, "Mine?", &["c"]);
        w.note(Event::ToYou {
            from: 1,
            kind: MsgKind::Inform,
            text: "Halfway.".into(),
            options: Vec::new(),
        });
        assert_eq!(w.question_of(1).unwrap().text, "Which?\nMore.");
        assert_eq!(w.question_of(2).unwrap().options, ["c"]);
        // A newer ask takes the older's place.
        ask(&mut w, 1, "Or this?", &[]);
        let q = w.question_of(1).unwrap();
        assert_eq!((q.text, q.options.len()), ("Or this?", 0));
        // The journal tidied away: the item's first line, no options.
        w.journal.clear();
        ask(&mut w, 2, "Still?\nYes.", &["d"]);
        w.journal.clear();
        let q = w.question_of(2).unwrap();
        assert_eq!((q.text, q.options.len()), ("Still?", 0));
        // Answered: no question.
        w.said(1, "b");
        assert_eq!(w.question_of(1), None);
        assert!(w.question_of(2).is_some());
    }

    #[test]
    fn an_ask_journaled_before_options_reads_back_without_them() {
        let v = serde_json::json!({"ToYou": {"from": 1, "kind": "Ask", "text": "x"}});
        let e: Event = serde_json::from_value(v).unwrap();
        assert_eq!(
            e,
            Event::ToYou {
                from: 1,
                kind: MsgKind::Ask,
                text: "x".into(),
                options: Vec::new()
            }
        );
    }

    #[test]
    fn a_passing_fault_is_tried_again_three_times_later_each_then_waits_for_the_person() {
        let mut w = world(vec![
            agent(0, Role::Leader, Status::Idle),
            agent(1, Role::Worker, Status::Idle),
            agent(2, Role::Worker, Status::Idle),
        ]);
        w.cards.push(Card {
            id: 0,
            project: 0,
            title: "Fix".into(),
            owner: Some(1),
            column: Column::Working,
            story: Vec::new(),
            symbols: Vec::new(),
            paths: Vec::new(),
            checks: Vec::new(),
            started: 0,
            finished: 0,
            attempt: 1,
            breaker: Default::default(),
            after: Vec::new(),
        });
        w.agents[1].card = Some(0);
        w.tick = 10;
        let retrying = |w: &World, attempt, seconds| {
            assert_eq!(
                w.journal.last().unwrap().event,
                Event::Retrying {
                    agent: 1,
                    why: "overloaded".into(),
                    attempt,
                    seconds
                }
            );
        };
        w.turn_failed(1, "overloaded", Some(Fault::Passing));
        assert_eq!(w.agents[1].retry, Some(Retry { attempt: 1, at: 16 }));
        assert_eq!(w.agents[1].tries, 1);
        retrying(&w, 1, 30);
        w.tick = 20;
        w.turn_failed(1, "overloaded", Some(Fault::Passing));
        assert_eq!(w.agents[1].retry, Some(Retry { attempt: 2, at: 44 }));
        retrying(&w, 2, 120);
        w.tick = 50;
        w.turn_failed(1, "overloaded", Some(Fault::Passing));
        assert_eq!(
            w.agents[1].retry,
            Some(Retry {
                attempt: 3,
                at: 146
            })
        );
        retrying(&w, 3, 480);
        assert!(
            w.open_inbox().is_empty(),
            "nothing waits for the person yet"
        );
        // The tries are used up: the person's now, once.
        w.tick = 150;
        w.turn_failed(1, "\n \nstill overloaded\nmore", Some(Fault::Passing));
        assert_eq!((w.agents[1].retry, w.agents[1].tries), (None, 0));
        let it = &w.inbox[0];
        assert_eq!(
            (
                it.kind,
                it.agent,
                it.detail.as_str(),
                it.open,
                it.tick,
                it.card
            ),
            (InboxKind::Fault, 1, "still overloaded", true, 150, Some(0))
        );
        assert_eq!(
            w.journal.last().unwrap().event,
            Event::Stopped {
                agent: 1,
                why: "\n \nstill overloaded\nmore".into()
            }
        );
        // An account's fault goes to the person at once, in the place of
        // the one that waited.
        w.agents[1].tries = 2;
        w.agents[1].retry = Some(Retry {
            attempt: 2,
            at: 999,
        });
        w.turn_failed(1, "Invalid API key", Some(Fault::Account));
        assert_eq!((w.agents[1].retry, w.agents[1].tries), (None, 0));
        assert_eq!(w.open_inbox(), [1]);
        assert_eq!(w.inbox[1].detail, "Invalid API key");
        // A failure of no kind imece knows is said and nothing more.
        let items = w.inbox.len();
        w.agents[2].tries = 2;
        w.agents[2].retry = Some(Retry {
            attempt: 2,
            at: 999,
        });
        w.turn_failed(2, "prompt is too long", None);
        assert_eq!(w.inbox.len(), items);
        assert_eq!((w.agents[2].retry, w.agents[2].tries), (None, 2));
        assert!(matches!(
            w.journal.last().unwrap().event,
            Event::Stopped { agent: 2, .. }
        ));
        // The person's word closes what waits and counts again from it.
        w.agents[1].tries = 1;
        w.agents[1].retry = Some(Retry {
            attempt: 1,
            at: 999,
        });
        w.said(1, "the key is mended");
        assert!(w.open_inbox().is_empty());
        assert_eq!((w.agents[1].retry, w.agents[1].tries), (None, 0));
        assert_eq!(
            Event::Retrying {
                agent: 2,
                why: String::new(),
                attempt: 1,
                seconds: 30
            }
            .agent(),
            2
        );
        assert_eq!(
            Event::Retrying {
                agent: 2,
                why: String::new(),
                attempt: 1,
                seconds: 30
            }
            .kind(),
            EntryKind::Assumption
        );
    }

    #[test]
    fn a_retry_is_due_at_its_tick_for_an_idle_agent_still_here() {
        let mut w = world(vec![
            agent(0, Role::Leader, Status::Idle),
            agent(1, Role::Worker, Status::Idle),
            agent(2, Role::Worker, Status::Working),
            agent(3, Role::Worker, Status::Idle),
            agent(4, Role::Worker, Status::Idle),
        ]);
        for id in 1..=4 {
            w.agents[id].retry = Some(Retry { attempt: 1, at: 10 });
        }
        w.agents[3].gone = true;
        w.agents[4].retry = Some(Retry { attempt: 1, at: 11 });
        w.tick = 9;
        assert!(w.retries_due().is_empty());
        w.tick = 10;
        assert_eq!(w.retries_due(), [1], "mid-turn, gone, later: not due");
        w.tick = 11;
        assert_eq!(w.retries_due(), [1, 4]);
        // Only its own API errors close for an agent.
        w.inbox.push(item(InboxKind::Fault, 1, true));
        w.inbox.push(item(InboxKind::Question, 1, true));
        w.inbox.push(item(InboxKind::Fault, 4, true));
        w.close_faults(1);
        assert_eq!(w.open_inbox(), [1, 2]);
    }

    #[test]
    fn a_question_to_the_person_waits_once_and_the_answer_closes_it() {
        let mut w = world(vec![
            agent(0, Role::Leader, Status::Idle),
            agent(1, Role::Worker, Status::Idle),
            agent(2, Role::Worker, Status::Idle),
        ]);
        w.tick = 4;
        assert!(!w.has_question(1));
        w.asked_you(1, "\n  \nWhich one?\nmore");
        w.asked_you(2, "Mine too");
        let it = &w.inbox[0];
        assert_eq!(
            (
                it.kind,
                it.agent,
                it.detail.as_str(),
                it.open,
                it.tick,
                it.researcher
            ),
            (InboxKind::Question, 1, "Which one?", true, 4, None)
        );
        assert!(w.has_question(1) && w.has_question(2));
        w.asked_you(1, "Or that?");
        assert_eq!(
            w.open_inbox(),
            [1, 2],
            "the newer question takes the older's place"
        );
        w.change_ready(change(1, "Docs", &[("README.md", 1, 0)]));
        w.close_questions(1);
        assert!(!w.has_question(1) && w.has_question(2), "only that agent's");
        assert_eq!(w.open_inbox(), [1, 3], "and only its questions");
        w.said(2, "The first.");
        assert!(!w.has_question(2));
        assert_eq!(w.open_inbox(), [3], "the change still waits");
        let e = Event::ToYou {
            from: 2,
            kind: MsgKind::Ask,
            text: "q".into(),
            options: Vec::new(),
        };
        assert_eq!((e.kind(), e.agent()), (EntryKind::Talk, 2));
        assert_eq!(
            StoryEvent::ToYou {
                kind: MsgKind::Inform,
                text: "x".into()
            }
            .kind(),
            StoryKind::Talk
        );
    }

    #[test]
    fn a_setup_yes_is_kept_once_the_newest_last_and_the_oldest_go_past_the_cap() {
        let mut w = world(vec![]);
        w.approve_setup("a".into());
        w.approve_setup("b".into());
        w.approve_setup("a".into());
        assert_eq!(w.setup_approved, ["b", "a"]);
        for n in 0..SETUP_KEPT {
            w.approve_setup(n.to_string());
        }
        assert_eq!(w.setup_approved.len(), SETUP_KEPT);
        assert_eq!(w.setup_approved[0], "0");
        assert_eq!(
            w.setup_approved.last().unwrap(),
            &(SETUP_KEPT - 1).to_string()
        );
        assert_eq!(SETUP_KEPT, 64);
        // Asks are the run's: a start closes them, and only them.
        let item = |kind| InboxItem {
            kind,
            agent: 0,
            detail: String::new(),
            researcher: None,
            open: true,
            tick: 0,
            card: None,
        };
        w.inbox = vec![
            item(InboxKind::Script),
            item(InboxKind::Question),
            item(InboxKind::Script),
        ];
        w.close_scripts();
        let open: Vec<bool> = w.inbox.iter().map(|i| i.open).collect();
        assert_eq!(open, [false, true, false]);
    }

    #[test]
    fn each_ask_waits_on_its_own_until_the_person_decides_or_the_session_ends() {
        let mut w = world(vec![
            agent(0, Role::Leader, Status::Idle),
            agent(1, Role::Worker, Status::Working),
            agent(2, Role::Worker, Status::Working),
        ]);
        w.tick = 7;
        assert!(!w.has_approval(1));
        w.asks_you(1, "Bash: make a\nmake b");
        w.asks_you(1, "Bash: make c");
        w.asks_you(2, "WebFetch: https://x");
        let it = &w.inbox[0];
        assert_eq!(
            (
                it.kind,
                it.agent,
                it.detail.as_str(),
                it.open,
                it.tick,
                it.researcher
            ),
            (
                InboxKind::Approval,
                1,
                "Bash: make a\nmake b",
                true,
                7,
                None
            ),
            "the whole command, every line of it"
        );
        assert_eq!(w.open_inbox(), [0, 1, 2], "each ask is its own item");
        assert!(w.has_approval(1) && w.has_approval(2));

        w.decided(1, false);
        assert_eq!(w.open_inbox(), [0, 2]);
        assert!(matches!(
            &w.journal.last().unwrap().event,
            Event::Decided { agent: 1, what, allowed: false } if what == "Bash: make c"
        ));
        let journal = w.journal.len();
        w.decided(1, true);
        assert_eq!(w.journal.len(), journal, "a closed item is decided once");
        w.decided(0, true);
        assert!(matches!(
            &w.journal.last().unwrap().event,
            Event::Decided {
                agent: 1,
                allowed: true,
                ..
            }
        ));
        assert!(!w.has_approval(1) && w.has_approval(2));
        // Neither a question nor a change is decided here.
        w.asked_you(2, "Which?");
        let journal = w.journal.len();
        w.decided(3, true);
        w.decided(99, true);
        assert!(w.inbox[3].open);
        assert_eq!(w.journal.len(), journal);

        // The session's end closes its asks alone.
        w.asks_you(1, "Bash: make d");
        w.close_approvals(2);
        assert!(!w.has_approval(2) && w.has_approval(1));
        assert!(w.has_question(2), "its question still waits");
        assert!(w.inbox[3].open);

        let e = Event::Decided {
            agent: 2,
            what: "x".into(),
            allowed: true,
        };
        assert_eq!((e.kind(), e.agent()), (EntryKind::Person, 2));
    }

    #[test]
    fn a_worker_s_change_waits_once_and_the_newer_one_takes_the_older_s_place() {
        let mut w = world(vec![
            agent(0, Role::Leader, Status::Idle),
            agent(1, Role::Worker, Status::Working),
            agent(2, Role::Worker, Status::Working),
        ]);
        w.change_ready(change(
            1,
            "Fix the parser",
            &[("a.rs", 3, 1), ("b.rs", 2, 0)],
        ));
        w.change_ready(change(2, "Docs", &[("README.md", 1, 0)]));
        assert_eq!(w.open_inbox().len(), 2);
        assert_eq!(w.inbox[0].kind, InboxKind::Change);
        assert_eq!(w.inbox[0].detail, "Fix the parser");
        assert_eq!(w.change_of(1).unwrap().counts(), (5, 1));
        assert!(matches!(
            w.journal[0].event,
            Event::ChangeReady {
                agent: 1,
                files: 2,
                added: 5,
                removed: 1,
                spend: None
            }
        ));
        assert_eq!(w.journal[0].event.kind(), EntryKind::Sensitive);
        assert_eq!(w.journal[0].event.agent(), 1);
        // The worker finishes again: one change of its waits, the newest.
        w.change_ready(change(1, "Fix the parser, again", &[("a.rs", 4, 1)]));
        let open: Vec<(AgentId, &str)> = w
            .open_inbox()
            .iter()
            .map(|&i| (w.inbox[i].agent, w.inbox[i].detail.as_str()))
            .collect();
        assert_eq!(open, [(2, "Docs"), (1, "Fix the parser, again")]);
        assert_eq!(w.changes.len(), 2);
        assert_eq!(w.change_of(1).unwrap().title, "Fix the parser, again");
        assert_eq!(w.change_of(2).unwrap().title, "Docs");
        assert!(w.change_of(0).is_none());

        // Applied: the item closes, the change goes, the journal says so.
        w.change_applied(1, "abc1234".into(), true);
        assert_eq!(w.open_inbox().len(), 1);
        assert!(w.change_of(1).is_none());
        assert!(matches!(
            &w.journal.last().unwrap().event,
            Event::Applied { agent: 1, files: 1, mark } if mark == "abc1234"
        ));
        assert_eq!(w.journal.last().unwrap().event.kind(), EntryKind::Merged);
        // Failed: it still waits.
        w.change_failed(2, "does not apply".into());
        assert_eq!(w.open_inbox().len(), 1);
        assert!(w.change_of(2).is_some());
        assert!(matches!(
            &w.journal.last().unwrap().event,
            Event::ApplyFailed { agent: 2, why } if why == "does not apply"
        ));
        assert_eq!(w.journal.last().unwrap().event.kind(), EntryKind::Failed);
        assert_eq!(w.journal.last().unwrap().event.agent(), 2);
        // Rejected: gone from the inbox and the world.
        w.change_rejected(2);
        assert!(w.open_inbox().is_empty());
        assert!(w.changes.is_empty());
        assert!(matches!(
            w.journal.last().unwrap().event,
            Event::Rejected { agent: 2 }
        ));
    }

    #[test]
    fn what_clashes_is_kept_with_its_change_and_goes_with_it() {
        let clashes = |files: &[&str]| Clashes {
            hunks: (files.iter())
                .map(|f| Clash {
                    file: f.to_string(),
                    ..Clash::default()
                })
                .collect(),
            ..Clashes::default()
        };
        let mut w = crate::live::testing::team(2);
        // No change waits: nothing to say, whatever was found.
        w.change_clashed(1, clashes(&["a"]));
        assert_eq!(w.clashes_of(1), None);
        w.change_ready(change(1, "Fix", &[("a", 1, 0)]));
        assert_eq!(w.clashes_of(1), None, "of the older change, if any");
        w.change_clashed(1, clashes(&["a"]));
        assert_eq!(w.clashes_of(1), Some(&clashes(&["a"])));
        assert_eq!(w.clashes_of(2), None);
        // Found again, it is what was found last; nothing found: none.
        w.change_clashed(1, clashes(&["b"]));
        assert_eq!(w.clashes_of(1), Some(&clashes(&["b"])));
        w.change_clashed(1, Clashes::default());
        assert_eq!(w.clashes_of(1), None);
        assert!(w.clashes.is_empty());
        // What was found goes with its change: rejected, applied, or
        // when a newer one takes its place.
        let gone: [fn(&mut World); 3] = [
            |w| w.change_rejected(1),
            |w| w.change_applied(1, "abc".into(), true),
            |w| w.change_ready(change(1, "Again", &[("a", 2, 0)])),
        ];
        for gone in gone {
            w.change_ready(change(1, "Fix", &[("a", 1, 0)]));
            w.change_clashed(1, clashes(&["a"]));
            gone(&mut w);
            assert!(w.clashes.is_empty());
        }
    }

    #[test]
    fn the_clock_reads_from_the_start_of_the_day() {
        // One test for every clock reading: the start is shared state.
        assert_eq!(clock(0), "14:00");
        assert_eq!(clock(13), "14:01");
        assert_eq!(clock_seconds(13), "14:01:05");
        assert_eq!(day(0), 0, "the simulation's day is the epoch's");
        assert_eq!(date(day(0)), "1970-01-01");
        start_clock_at(86_400 + 60, 20_756);
        assert_eq!(clock(0), "00:01", "wraps at midnight");
        assert_eq!(clock_seconds(1), "00:01:05");
        assert_eq!(day(0), 20_756, "the seconds wrap, the day does not move");
        // 2026-10-30 at 23:59:00: a tick later it is the 31st.
        start_clock_at(23 * 3600 + 59 * 60, 20_756);
        assert_eq!(day(0), 20_756);
        assert_eq!(day(11), 20_756, "23:59:55");
        assert_eq!(day(12), 20_757, "00:00:00 of the next day");
        assert_eq!(clock(12), "00:00");
        assert_eq!(stamp(11), "2026-10-30 23:59");
        assert_eq!(stamp(12), "2026-10-31 00:00");
        assert_eq!(when(11, 20_756), "23:59", "today: the clock alone");
        assert_eq!(when(12, 20_756), "2026-10-31 00:00", "another day: dated");
        assert_eq!(when(12, 20_757), "00:00");
        start_clock_at(14 * 3600, 0);
        assert_eq!(clock(0), "14:00");
    }

    #[test]
    fn days_and_dates_agree_with_the_calendar() {
        assert_eq!(days_from_civil(1970, 1, 1), 0);
        assert_eq!(days_from_civil(1970, 1, 2), 1);
        assert_eq!(days_from_civil(1969, 12, 31), -1);
        assert_eq!(days_from_civil(2000, 3, 1), 11_017, "after a leap day");
        assert_eq!(days_from_civil(2026, 10, 30), 20_756);
        assert_eq!(days_from_civil(2024, 2, 29), 19_782, "a leap day");
        assert_eq!(days_from_civil(2100, 3, 1), 47_541, "2100 is no leap year");
        for (days, text) in [
            (0, "1970-01-01"),
            (59, "1970-03-01"),
            (11_016, "2000-02-29"),
            (11_017, "2000-03-01"),
            (19_782, "2024-02-29"),
            (20_756, "2026-10-30"),
            (20_790, "2026-12-03"),
            (20_818, "2026-12-31"),
            (20_819, "2027-01-01"),
            (47_540, "2100-02-28"),
            (47_541, "2100-03-01"),
        ] {
            assert_eq!(date(days), text);
            let (y, m, d) = civil_from_days(days as i64);
            assert_eq!(days_from_civil(y, m, d), days as i64, "{text} round trip");
        }
        // Every day of forty years goes round and comes back, in order.
        let mut last = (1969, 12, 31);
        for z in 0..(365 * 40 + 10) {
            let c = civil_from_days(z);
            assert!(c > last, "{c:?} after {last:?}");
            assert_eq!(days_from_civil(c.0, c.1, c.2), z);
            last = c;
        }
        // Before the year 0 too, where the eras are counted the other way
        // (imece never gets there; the arithmetic must still be right):
        // 0000-03-01 begins the era, the day before is the leap day of
        // year 0, and January of year 0 belongs to the year -1's era.
        assert_eq!(days_from_civil(0, 3, 1), -719_468);
        assert_eq!(days_from_civil(0, 2, 29), -719_469);
        assert_eq!(days_from_civil(0, 1, 1), -719_528);
        assert_eq!(days_from_civil(-1, 12, 31), -719_529);
        assert_eq!(days_from_civil(-1, 1, 1), -719_893, "year -1 has 365 days");
        assert_eq!(civil_from_days(-719_468), (0, 3, 1));
        assert_eq!(civil_from_days(-719_469), (0, 2, 29));
        assert_eq!(civil_from_days(-719_528), (0, 1, 1));
        assert_eq!(civil_from_days(-719_529), (-1, 12, 31));
        let mut last = civil_from_days(-720_001);
        for z in -720_000..-719_000 {
            let c = civil_from_days(z);
            assert!(c > last, "{c:?} after {last:?}");
            assert_eq!(days_from_civil(c.0, c.1, c.2), z);
            last = c;
        }
    }

    fn card(id: CardId, owner: Option<AgentId>, story: Vec<StoryEvent>) -> Card {
        Card {
            id,
            project: 0,
            title: format!("c{id}"),
            owner,
            column: Column::Working,
            story: story
                .into_iter()
                .map(|event| Story { tick: 0, event })
                .collect(),
            symbols: Vec::new(),
            paths: Vec::new(),
            checks: Vec::new(),
            started: 0,
            finished: 0,
            attempt: 1,
            breaker: Default::default(),
            after: Vec::new(),
        }
    }

    #[test]
    fn a_card_is_named_by_its_number_where_no_digit_follows() {
        // Card 2 is #3 to the person.
        let text = "#3 then #30, #3; x#3";
        assert_eq!(mentions(text, 2), [0..2, 13..15, 18..20]);
        let thirty = mentions("#30", 29);
        assert_eq!((thirty.len(), thirty.first()), (1, Some(&(0..3))));
        assert!(mentions("#30", 2).is_empty());
        assert!(mentions("3 or # 3", 2).is_empty());
        assert!(mentions("", 2).is_empty());
    }

    #[test]
    fn the_events_that_name_a_card_say_which() {
        let named = [
            Event::Assign { agent: 1, card: 4 },
            Event::Merged { agent: 1, card: 5 },
            Event::Posted { agent: 1, card: 6 },
            Event::Claimed { agent: 1, card: 7 },
        ];
        let cards: Vec<_> = named.iter().map(Event::card).collect();
        assert_eq!(cards, [Some(4), Some(5), Some(6), Some(7)]);
        assert_eq!(Event::Failed { agent: 1 }.card(), None);
    }

    #[test]
    fn a_helper_works_on_its_worker_s_card() {
        let mut w = world(vec![
            agent(0, Role::Leader, Status::Idle),
            agent(1, Role::Worker, Status::Working),
            agent(2, Role::Helper, Status::Working),
            agent(3, Role::Worker, Status::Working),
        ]);
        w.agents[1].card = Some(5);
        w.agents[2].parent = Some(1);
        w.agents[2].card = Some(9);
        w.agents[3].card = Some(6);
        // Not a helper: its own card, whatever its parent says.
        w.agents[3].parent = Some(1);
        assert_eq!(w.card_of(1), Some(5));
        assert_eq!(w.card_of(2), Some(5));
        assert_eq!(w.card_of(3), Some(6));
        assert_eq!(w.card_of(0), None);
    }

    #[test]
    fn an_inbox_item_keeps_the_card_it_came_on() {
        let mut w = world(vec![
            agent(0, Role::Leader, Status::Idle),
            agent(1, Role::Worker, Status::Working),
            agent(2, Role::Helper, Status::Working),
        ]);
        w.agents[1].card = Some(4);
        w.agents[2].parent = Some(1);
        w.replied(1, "done");
        w.asked_you(2, "which?");
        w.asks_you(2, "Bash: make");
        w.change_ready(Change {
            agent: 1,
            title: "did it".into(),
            files: Vec::new(),
            patch: String::new(),
            runs: Vec::new(),
            check: Check::None,
            spend: None,
        });
        // The worker took the next card: what came stays with the first.
        w.agents[1].card = Some(5);
        w.replied(0, "planned");
        let cards: Vec<_> = w.inbox.iter().map(|i| i.card).collect();
        assert_eq!(cards, [Some(4), Some(4), Some(4), Some(4), None]);
        assert_eq!(w.card_inbox(4), [0, 1, 2, 3]);
        assert!(w.card_inbox(5).is_empty());
        // Only the open ones.
        w.close_replies(1);
        assert_eq!(w.card_inbox(4), [1, 2, 3]);
    }

    #[test]
    fn a_card_s_people_come_once_each_with_the_first_part_they_had() {
        let mut w = world(
            (0..8)
                .map(|i| agent(i, Role::Worker, Status::Working))
                .collect(),
        );
        w.agents[3].role = Role::Helper;
        w.agents[3].parent = Some(1);
        w.agents[1].card = Some(0);
        let said = |from, to| StoryEvent::Sent {
            from,
            to,
            kind: MsgKind::Ask,
            text: String::new(),
            id: 0,
            re: None,
        };
        w.cards.push(card(
            0,
            Some(1),
            vec![
                StoryEvent::Took { agent: 1 },
                StoryEvent::Asked {
                    from: 1,
                    to: 2,
                    text: String::new(),
                },
                said(4, 1),
                StoryEvent::Answered {
                    from: 5,
                    text: String::new(),
                },
                StoryEvent::ResearchArrived {
                    researcher: 6,
                    topic: String::new(),
                },
                said(0, 7),
                // Posted after the rest, still first.
                StoryEvent::Posted {
                    by: 0,
                    text: String::new(),
                },
            ],
        ));
        assert_eq!(
            w.card_people(0),
            [
                (0, Part::Posted),
                (1, Part::Owner),
                (3, Part::Helper),
                (2, Part::Talked),
                (4, Part::Talked),
                (5, Part::Talked),
                (6, Part::Talked),
                (7, Part::Talked),
            ]
        );
        // The owner on another card now: its helpers are that card's.
        w.agents[1].card = Some(1);
        assert!(!w.card_people(0).contains(&(3, Part::Helper)));
        // Nobody took it: who posted it.
        w.cards.push(card(
            1,
            None,
            vec![StoryEvent::Posted {
                by: 0,
                text: String::new(),
            }],
        ));
        assert_eq!(w.card_people(1), [(0, Part::Posted)]);
    }
    #[test]
    fn archiving_a_project_puts_what_it_had_out_of_sight_and_a_fresh_one_in_its_place() {
        let mut w = world(vec![
            agent(0, Role::Leader, Status::Idle),
            agent(1, Role::Worker, Status::Blocked),
            agent(2, Role::Helper, Status::Working),
        ]);
        w.agents[0].name = "lead".into();
        w.agents[1].name = "claude-1".into();
        w.agents[1].activity = Some("Edit a.rs".into());
        w.agents[1].retry = Some(Retry { attempt: 2, at: 9 });
        w.agents[1].tries = 2;
        w.projects[0].pinned = true;
        w.projects[0].copies = vec![".env".into()];
        w.projects.push(Project {
            id: 1,
            name: "q".into(),
            leader: 3,
            trunk: None,
            files: Vec::new(),
            repo: &[],
            closed: false,
            pinned: false,
            copies: Vec::new(),
            allowed: Vec::new(),
            plan: PlanGate::default(),
            no_git: false,
            lonca: None,
            archived: None,
        });
        w.agents.push(Agent::new(
            3,
            1,
            "lead-q".into(),
            Role::Leader,
            Status::Idle,
            0,
        ));
        w.inbox = vec![
            item(InboxKind::Reply, 1, true),
            item(InboxKind::Reply, 3, true),
            item(InboxKind::Reply, 1, false),
        ];
        w.cards = vec![card(0, Some(1), Vec::new()), card(1, Some(3), Vec::new())];
        w.cards[1].project = 1;
        for c in &mut w.cards {
            c.column = Column::Done;
        }
        w.journal.push(Entry {
            tick: 0,
            event: Event::Failed { agent: 1 },
        });
        w.tick = 50;
        assert_eq!(
            w.shown_cards().filter(|c| c.column == Column::Done).count(),
            2
        );

        assert_eq!(w.archive_project(0, "first".into()), 2);
        let was = &w.projects[0];
        assert!(was.closed);
        assert_eq!(
            was.archived,
            Some(Archived {
                at: 50,
                title: "first".into()
            })
        );
        let fresh = &w.projects[2];
        assert_eq!(
            (
                fresh.id,
                fresh.name.as_str(),
                fresh.leader,
                fresh.closed,
                fresh.pinned
            ),
            (2, "p", 4, false, true)
        );
        assert_eq!(fresh.copies, [".env"]);
        assert!(fresh.archived.is_none());
        let lead = &w.agents[4];
        assert_eq!(
            (lead.name.as_str(), lead.project, lead.role),
            ("lead", 2, Role::Leader)
        );
        for id in 0..3 {
            assert_eq!(
                (w.agents[id].status, w.agents[id].gone),
                (Status::Asleep, false)
            );
        }
        let a = &w.agents[1];
        assert_eq!((a.activity.as_deref(), a.retry, a.tries), (None, None, 0));
        assert_eq!(w.agents[3].status, Status::Idle, "the other project's");
        assert_eq!(w.journal.len(), 2, "one line, none of an opening");
        assert_eq!(
            w.journal[1].event,
            Event::Archived {
                leader: 4,
                was: 0,
                title: "first".into()
            }
        );
        // Out of sight, and out of reach by name.
        assert_eq!(w.agent_by_name("lead"), Some(4), "not the archive's");
        assert_eq!(w.agent_by_name("claude-1"), None, "only the archive has it");
        assert_eq!(w.agent_by_name("lead-q"), Some(3));
        assert!(
            w.name_taken("claude-1"),
            "a new worker is never called that"
        );
        assert!(!w.name_taken("claude-9"));
        assert_eq!(w.open_inbox(), [1]);
        assert!(w.inbox[0].open, "it waits, open, in the archive");
        assert_eq!(
            w.shown_cards().filter(|c| c.column == Column::Done).count(),
            1
        );
        assert_eq!(w.shown_cards().map(|c| c.id).collect::<Vec<_>>(), [1]);
        assert!(w.in_archive(1) && w.put_away(1));
        assert!(!w.in_archive(3) && !w.put_away(3) && !w.in_archive(4));
        assert!(!w.in_archive(99) && !w.put_away(99), "no such agent");
        // A closed project is put away, not archived: its lines and items
        // are its own business.
        w.projects[1].closed = true;
        assert!(w.put_away(3) && !w.in_archive(3));
        assert_eq!(w.open_inbox(), [1]);
        assert_eq!(w.agent_by_name("lead-q"), None);
    }

    #[test]
    fn an_archive_comes_back_with_all_it_had_and_says_what_gave_way() {
        let mut w = world(vec![
            agent(0, Role::Leader, Status::Idle),
            agent(1, Role::Worker, Status::Idle),
        ]);
        w.agents[0].name = "lead".into();
        w.inbox = vec![item(InboxKind::Reply, 1, true)];
        w.cards = vec![card(0, Some(1), Vec::new())];
        w.tick = 10;
        assert_eq!(w.archive_project(0, "first".into()), 1);
        assert_eq!(w.agent_by_name("lead"), Some(2));
        // The fresh one gives way, put away with no line of its own.
        w.tick = 20;
        let lines = w.journal.len();
        w.shelve(1, "later".into());
        assert_eq!(w.journal.len(), lines);
        assert!(w.projects[1].closed);
        assert_eq!(
            w.projects[1].archived,
            Some(Archived {
                at: 20,
                title: "later".into()
            })
        );
        assert_eq!(w.agents[2].status, Status::Asleep);

        w.restore_project(0, Some("later".into()));
        let q = &w.projects[0];
        assert!(!q.closed && q.archived.is_none());
        assert_eq!(w.agent_by_name("lead"), Some(0), "the archive's head again");
        for id in 0..2 {
            assert_eq!(
                (w.agents[id].status, w.agents[id].gone),
                (Status::Asleep, false),
                "asleep until written to"
            );
        }
        assert_eq!(w.open_inbox(), [0]);
        assert_eq!(w.shown_cards().count(), 1);
        assert_eq!(
            w.journal.last().map(|e| &e.event),
            Some(&Event::Restored {
                leader: 0,
                title: "first".into(),
                swapped: Some("later".into()),
            })
        );
        assert_eq!(w.journal.last().map(|e| e.event.agent()), Some(0));
        assert_eq!(
            w.journal.last().map(|e| e.event.kind()),
            Some(EntryKind::Person)
        );
        // Words, both ways.
        use crate::i18n::Lang;
        let said =
            |w: &World, lang| crate::ui::words::event(lang, w, &w.journal.last().unwrap().event);
        assert_eq!(
            said(&w, Lang::En),
            "you brought back p “first”; the one open there is archived as “later”"
        );
        assert_eq!(
            said(&w, Lang::Tr),
            "p projesinin “first” arşivini geri getirdin; orada açık olan “later” adıyla \
             arşivlendi"
        );
        // What is no archive does not come back: nothing happens.
        let lines = w.journal.len();
        w.restore_project(0, None);
        assert_eq!(w.journal.len(), lines);
        w.projects[1].archived = None;
        w.restore_project(1, None);
        assert!(w.projects[1].closed, "closed, not archived: stays closed");
        // Brought back with nothing in its place.
        w.projects[1].archived = Some(Archived {
            at: 20,
            title: "later".into(),
        });
        w.restore_project(1, None);
        assert_eq!(said(&w, Lang::En), "you brought back p “later”");
        assert_eq!(
            said(&w, Lang::Tr),
            "p projesinin “later” arşivini geri getirdin"
        );
    }

    #[test]
    fn restore_names_an_archive_by_title_then_by_project_newest_first() {
        let mut w = world(vec![agent(0, Role::Leader, Status::Idle)]);
        let put = |w: &mut World, name: &str, at: u64, title: &str| {
            let id = w.projects.len();
            w.projects.push(Project {
                id,
                name: name.into(),
                leader: 0,
                trunk: None,
                files: Vec::new(),
                repo: &[],
                closed: true,
                pinned: false,
                copies: Vec::new(),
                allowed: Vec::new(),
                plan: PlanGate::default(),
                no_git: false,
                lonca: None,
                archived: Some(Archived {
                    at,
                    title: title.into(),
                }),
            });
        };
        w.projects[0].name = "kule".into();
        put(&mut w, "kule", 10, "api");
        put(&mut w, "kule", 20, "first epic");
        put(&mut w, "api", 20, "first epic");
        put(&mut w, "kule", 5, "old");
        // Newest first; archived at the same tick, the higher id first.
        assert_eq!(w.archives(), [3, 2, 1, 4]);
        assert_eq!(w.archive_named("first epic", 0), Some(3), "the higher id");
        assert_eq!(w.archive_named("old", 0), Some(4));
        assert_eq!(w.archive_named("kule", 0), Some(2), "archived last");
        assert_eq!(w.archive_named("api", 0), Some(1), "a title before a name");
        assert_eq!(w.archive_named("", 0), Some(2), "the project at hand's");
        assert_eq!(w.archive_named("", 3), Some(3));
        assert_eq!(w.archive_named("nothing", 0), None);
        assert_eq!(w.archive_named("First epic", 0), None, "exactly");
        w.projects[2].archived = None;
        assert_eq!(w.archive_named("kule", 0), Some(1));
        assert_eq!(w.archives(), [3, 1, 4]);
    }

    #[test]
    fn a_project_did_something_when_the_world_shows_anything_of_it() {
        let mut w = world(vec![agent(0, Role::Leader, Status::Idle)]);
        w.projects.push(Project {
            id: 1,
            name: "q".into(),
            leader: 1,
            trunk: None,
            files: Vec::new(),
            repo: &[],
            closed: false,
            pinned: false,
            copies: Vec::new(),
            allowed: Vec::new(),
            plan: PlanGate::default(),
            no_git: false,
            lonca: None,
            archived: None,
        });
        w.agents.push(Agent::new(
            1,
            1,
            "lead-q".into(),
            Role::Leader,
            Status::Idle,
            0,
        ));
        // Another project's of every kind.
        w.agents[1].cost = 9;
        w.agents.push(Agent::new(
            2,
            1,
            "claude-1".into(),
            Role::Worker,
            Status::Idle,
            0,
        ));
        w.cards = vec![card(1, None, Vec::new())];
        w.cards[0].project = 1;
        w.inbox = vec![item(InboxKind::Reply, 1, true)];
        // Its opening, archiving, bringing back, renaming and closing.
        for event in [
            Event::Failed { agent: 1 },
            Event::Opened {
                leader: 0,
                path: "/p".into(),
            },
            Event::NoGit {
                leader: 0,
                path: "/p".into(),
            },
            Event::Confine {
                leader: 0,
                level: "open".into(),
            },
            Event::Archived {
                leader: 0,
                was: 1,
                title: "t".into(),
            },
            Event::Restored {
                leader: 0,
                title: "t".into(),
                swapped: None,
            },
            Event::Renamed {
                leader: 0,
                from: "a".into(),
                to: "b".into(),
            },
            Event::Closed { leader: 0 },
        ] {
            w.journal.push(Entry { tick: 0, event });
        }
        assert!(!w.did_anything(0));
        assert!(w.did_anything(1));
        w.agents[0].cost = 1;
        assert!(w.did_anything(0), "it spent");
        w.agents[0].cost = 0;
        w.agents[2].project = 0;
        assert!(w.did_anything(0), "an agent besides its head");
        w.agents[2].project = 1;
        w.cards[0].project = 0;
        assert!(w.did_anything(0), "a card");
        w.cards[0].project = 1;
        w.inbox[0].agent = 0;
        assert!(w.did_anything(0), "an inbox item");
        w.inbox[0].agent = 1;
        assert!(!w.did_anything(0));
        w.journal.push(Entry {
            tick: 0,
            event: Event::Failed { agent: 0 },
        });
        assert!(w.did_anything(0), "a line of its own");
    }

    #[test]
    fn an_archive_without_a_title_is_called_by_its_name_and_its_days() {
        let mut w = world(vec![agent(0, Role::Leader, Status::Idle)]);
        let today = date(day(0));
        assert_eq!(w.archive_title(0), format!("p {today}"), "no line yet");
        w.journal.push(Entry {
            tick: 0,
            event: Event::Failed { agent: 0 },
        });
        assert_eq!(w.archive_title(0), format!("p {today}"), "one day");
        // Two days of five-second ticks on, from its first line.
        w.tick = 2 * 86_400 / SECONDS_PER_TICK;
        let then = date(day(w.tick));
        assert_ne!(then, today);
        assert_eq!(w.archive_title(0), format!("p {today} → {then}"));
        // Only its own lines count.
        w.agents
            .push(Agent::new(1, 1, "x".into(), Role::Leader, Status::Idle, 0));
        w.journal[0].event = Event::Failed { agent: 1 };
        w.journal.push(Entry {
            tick: w.tick,
            event: Event::Failed { agent: 0 },
        });
        assert_eq!(w.archive_title(0), format!("p {then}"));
    }
    #[test]
    fn the_plan_gate_holds_a_second_worker_until_a_yes_and_says_where_it_stands() {
        let mut w = world(vec![
            agent(0, Role::Leader, Status::Idle),
            agent(1, Role::Worker, Status::Idle),
        ]);
        assert_eq!(w.projects[0].plan, PlanGate::default());
        assert!(w.projects[0].plan.on, "on by default");
        assert_eq!(w.plan_hold(0), Some(PlanHold::NoPlan));
        w.agents[1].gone = true;
        assert_eq!(w.plan_hold(0), None, "no worker there");
        w.agents[1].gone = false;
        w.projects[0].plan.on = false;
        assert_eq!(w.plan_hold(0), None, "off");
        w.projects[0].plan.on = true;
        let plan = |n: usize| Proposal {
            cards: (0..n)
                .map(|i| Planned {
                    title: format!("c{i}"),
                    tier: "standard".into(),
                    paths: Vec::new(),
                    checks: Vec::new(),
                    body: "b".into(),
                    after: Vec::new(),
                })
                .collect(),
            cost: 1_234,
            tick: 0,
        };
        let first = w.propose(0, plan(2));
        assert_eq!(first, 0);
        let it = &w.inbox[first];
        assert_eq!(
            (it.kind, it.agent, it.open, it.detail.as_str()),
            (InboxKind::Plan, 0, true, "c0")
        );
        assert_eq!(w.plan_item(0), Some(first));
        assert_eq!(w.plan_hold(0), Some(PlanHold::Reading));
        assert!(matches!(
            w.journal.last().unwrap().event,
            Event::Planned {
                leader: 0,
                cards: 2,
                cost: 1_234
            }
        ));
        // A new plan closes the first, unanswered.
        let second = w.propose(0, plan(1));
        assert!(!w.inbox[first].open && w.inbox[second].open);
        assert_eq!(w.plan_item(0), Some(second));
        assert_eq!(
            w.projects[0].plan.proposal.as_ref().map(|p| p.cards.len()),
            Some(1)
        );
        // Answered in words: the item closes, the gate stays shut; another
        // agent's plan and another kind of the leader's stay.
        w.inbox.push(item(InboxKind::Plan, 1, true));
        w.inbox.push(item(InboxKind::Approval, 0, true));
        w.said(0, "hmm");
        assert!(!w.inbox[second].open);
        assert!(w.inbox[second + 1].open && w.inbox[second + 2].open);
        assert_eq!(w.plan_hold(0), Some(PlanHold::Unanswered));
        assert_eq!(w.decide_plan(second, true), None, "closed already");
        // A yes, then a no.
        let third = w.propose(0, plan(3));
        assert_eq!(w.decide_plan(third, true), Some(0));
        assert!(w.projects[0].plan.approved);
        assert_eq!(w.plan_hold(0), None);
        assert!(w.projects[0].plan.proposal.is_some(), "kept on a yes");
        assert!(matches!(
            w.journal.last().unwrap().event,
            Event::PlanAnswered {
                leader: 0,
                yes: true
            }
        ));
        let fourth = w.propose(0, plan(1));
        assert!(
            !w.projects[0].plan.approved,
            "a new plan takes the yes back"
        );
        assert_eq!(w.decide_plan(fourth, false), Some(0));
        assert_eq!(w.projects[0].plan.proposal, None);
        assert_eq!(w.plan_hold(0), Some(PlanHold::NoPlan));
        assert_eq!(w.decide_plan(99, true), None, "no such item");
        // Not a plan: nothing decided.
        w.inbox.push(item(InboxKind::Question, 0, true));
        let q = w.inbox.len() - 1;
        assert_eq!(w.decide_plan(q, true), None);
        assert!(w.inbox[q].open);
        // The events' places.
        let planned = Event::Planned {
            leader: 0,
            cards: 1,
            cost: 0,
        };
        assert_eq!((planned.kind(), planned.agent()), (EntryKind::Plan, 0));
        let answered = Event::PlanAnswered {
            leader: 0,
            yes: false,
        };
        assert_eq!((answered.kind(), answered.agent()), (EntryKind::Person, 0));
    }

    #[test]
    fn a_repository_name_is_plain_and_of_a_length() {
        assert!(valid_repo("a"));
        assert!(valid_repo("my.repo_1-x"));
        assert!(valid_repo(&"a".repeat(64)));
        assert!(!valid_repo(""));
        assert!(!valid_repo(&"a".repeat(65)));
        for bad in ["a b", "a/b", "ü", "a\x1b"] {
            assert!(!valid_repo(bad), "{bad:?}");
        }
    }

    fn up(pending: usize) -> StoreSeen {
        StoreSeen::Up {
            trunk: "t".into(),
            pending,
            behind: Behind::By(0),
        }
    }

    fn lead(id: AgentId) -> Agent {
        Agent::new(id, 0, "lead".into(), Role::Leader, Status::Idle, 0)
    }

    fn second(w: &mut World, name: &str) -> ProjectId {
        w.open_project(None, name.into(), "/x".into(), Role::Leader)
    }

    fn store_lines(w: &World) -> usize {
        (w.journal.iter())
            .filter(|e| matches!(e.event, Event::StoreDown { .. } | Event::StoreBack { .. }))
            .count()
    }

    #[test]
    fn a_store_going_down_and_back_is_said_once_each_and_an_answer_changes_the_screen() {
        let mut w = world(vec![lead(0)]);
        let down = |why| StoreSeen::Down(why);
        // No repository named: nothing to see.
        assert!(!w.store_seen(0, "r", up(0)));
        w.on_lonca(0, Some("r".into()));
        assert_eq!(
            w.projects[0].lonca.as_ref().unwrap().seen,
            StoreSeen::Asking
        );
        // Another's answer is late: the person named this one since.
        assert!(!w.store_seen(0, "older", up(0)));
        assert_eq!(
            w.projects[0].lonca.as_ref().unwrap().seen,
            StoreSeen::Asking
        );
        // A first answer is said on the project's line alone.
        assert!(w.store_seen(0, "r", up(0)));
        assert_eq!(store_lines(&w), 0);
        // The same again changes nothing, a new count does.
        assert!(!w.store_seen(0, "r", up(0)));
        assert!(w.store_seen(0, "r", up(1)));
        assert_eq!(store_lines(&w), 0);
        assert_eq!(w.projects[0].lonca.as_ref().unwrap().seen, up(1));
        // Down: once, however many looks find it down.
        assert!(w.store_seen(0, "r", down(StoreDown::NoDaemon)));
        assert!(!w.store_seen(0, "r", down(StoreDown::NoDaemon)));
        assert_eq!(store_lines(&w), 1);
        assert!(matches!(
            &w.journal.last().unwrap().event,
            Event::StoreDown { leader: 0, repo, why: StoreDown::NoDaemon } if repo == "r"
        ));
        // Another reason is news.
        assert!(w.store_seen(0, "r", down(StoreDown::SignedOut)));
        assert_eq!(store_lines(&w), 2);
        // Back.
        assert!(w.store_seen(0, "r", up(1)));
        assert_eq!(store_lines(&w), 3);
        assert!(matches!(
            &w.journal.last().unwrap().event,
            Event::StoreBack { leader: 0, repo } if repo == "r"
        ));
        // Up to up is no return.
        assert!(w.store_seen(0, "r", up(2)));
        assert_eq!(store_lines(&w), 3);
        // Named anew, it starts over and says nothing of the old.
        w.on_lonca(0, Some("s".into()));
        assert!(w.store_seen(0, "s", down(StoreDown::NoRepo)));
        assert_eq!(store_lines(&w), 4);
        w.on_lonca(0, None);
        assert!(w.projects[0].lonca.is_none());
    }

    #[test]
    fn a_project_is_closed_until_its_store_answers() {
        let mut w = world(vec![lead(0)]);
        assert_eq!(w.store_closed(0), None, "not on Lonca");
        w.on_lonca(0, Some("r".into()));
        assert_eq!(w.store_closed(0), Some("imece is still asking Lonca"));
        w.store_seen(0, "r", StoreSeen::Down(StoreDown::Forbidden));
        assert_eq!(w.store_closed(0), Some(StoreDown::Forbidden.why()));
        w.store_seen(0, "r", up(3));
        assert_eq!(w.store_closed(0), None);
    }

    #[test]
    fn every_reason_a_store_is_down_says_its_own() {
        let all = [
            StoreDown::NoDaemon,
            StoreDown::NotTheDaemon,
            StoreDown::Protocol,
            StoreDown::Old,
            StoreDown::Hardened,
            StoreDown::SignedOut,
            StoreDown::Forbidden,
            StoreDown::NoRepo,
            StoreDown::Refused,
            StoreDown::Busy,
            StoreDown::Frozen,
        ];
        let said: std::collections::HashSet<&str> = all.iter().map(|d| d.why()).collect();
        assert_eq!(said.len(), all.len());
        assert!(
            said.iter()
                .all(|s| s.contains("Lonca") || s.contains("daemon"))
        );
    }

    #[test]
    fn a_change_s_why_is_what_its_newest_done_says_under_its_first_line() {
        let (mut w, _) = crate::sim::world(1, 2, 0, 1, crate::i18n::Lang::En);
        let done = |from, text: &str, kind| Event::Sent {
            from,
            to: 0,
            kind,
            text: text.into(),
            id: 1,
            re: None,
        };
        assert_eq!(w.done_why(1), None);
        w.note(done(1, "Title\n\nOld reason.", MsgKind::Done));
        assert_eq!(w.done_why(1), Some("Old reason."));
        w.note(done(1, "Other kind\nnot this", MsgKind::Inform));
        w.note(done(2, "Theirs\nnot this", MsgKind::Done));
        assert_eq!(w.done_why(1), Some("Old reason."));
        w.note(done(1, "Title only", MsgKind::Done));
        assert_eq!(w.done_why(1), None, "the newest says nothing more");
        w.note(done(1, "Title\n  \n", MsgKind::Done));
        assert_eq!(w.done_why(1), None, "blank is nothing");
        // Live's journal has the first line; the card's story the whole
        // message, found by its number.
        let story = |from, text: &str, kind, id| StoryEvent::Sent {
            from,
            to: 0,
            kind,
            text: text.into(),
            id,
            re: None,
        };
        w.story(
            Some(0),
            story(1, "Title\n\nWhole reason.", MsgKind::Done, 7),
        );
        w.story(
            Some(0),
            story(1, "Title\n\nNot this number.", MsgKind::Done, 8),
        );
        w.story(Some(0), story(2, "Title\n\nNot theirs.", MsgKind::Done, 7));
        w.story(
            Some(0),
            story(1, "Title\n\nNot this kind.", MsgKind::Inform, 7),
        );
        let first = |id| Event::Sent {
            from: 1,
            to: 0,
            kind: MsgKind::Done,
            text: "Title".into(),
            id,
            re: None,
        };
        w.note(first(7));
        assert_eq!(w.done_why(1), Some("Whole reason."));
        w.note(first(9));
        assert_eq!(w.done_why(1), None, "no story of that message");
    }

    #[test]
    fn only_open_projects_on_lonca_are_looked_at() {
        let mut w = world(vec![lead(0)]);
        assert!(w.stores().is_empty());
        let (a, b) = (second(&mut w, "a"), second(&mut w, "b"));
        w.on_lonca(a, Some("ra".into()));
        w.on_lonca(b, Some("rb".into()));
        assert_eq!(w.stores(), [(a, "ra".to_string()), (b, "rb".to_string())]);
        w.close_project(a);
        assert_eq!(w.stores(), [(b, "rb".to_string())]);
        let fresh = w.archive_project(b, "t".into());
        // The fresh one is on the same store and asks anew; the archive
        // is not looked at.
        assert_eq!(w.stores(), [(fresh, "rb".to_string())]);
        assert_eq!(w.projects[fresh].lonca.as_ref().unwrap().repo, "rb");
    }

    #[test]
    fn a_stores_words_belong_to_the_project_and_are_no_work_done() {
        let mut w = world(vec![lead(0)]);
        let p = second(&mut w, "a");
        let leader = w.projects[p].leader;
        assert!(!w.did_anything(p));
        w.on_lonca(p, Some("r".into()));
        w.store_seen(p, "r", StoreSeen::Down(StoreDown::NoDaemon));
        w.store_seen(p, "r", up(0));
        assert!(!w.did_anything(p));
        let down = Event::StoreDown {
            leader,
            repo: "r".into(),
            why: StoreDown::NoDaemon,
        };
        let back = Event::StoreBack {
            leader,
            repo: "r".into(),
        };
        assert_eq!((down.agent(), down.kind()), (leader, EntryKind::Assumption));
        assert_eq!((back.agent(), back.kind()), (leader, EntryKind::Assumption));
    }

    fn ask(project: ProjectId) -> ImportAsk {
        ImportAsk {
            project,
            repo: "r".into(),
            checkout: "/c".into(),
            branch: "main".into(),
            head: "1".repeat(40),
        }
    }

    #[test]
    fn a_lonca_commands_lines_belong_to_its_project_and_are_no_work_done() {
        let mut w = world(vec![lead(0)]);
        let p = second(&mut w, "a");
        let leader = w.projects[p].leader;
        w.store_signed_in(p, "ayse".into());
        w.store_imported(&ask(p), vec!["big".into()], true);
        w.store_failed(&StoreJob::Login(p), Some("no key".into()));
        w.store_failed(&StoreJob::Import(ask(p)), None);
        assert!(!w.did_anything(p));
        let lines = &w.journal[w.journal.len() - 4..];
        for e in lines {
            assert_eq!(
                (e.event.agent(), e.event.kind()),
                (leader, EntryKind::Assumption)
            );
        }
        assert_eq!(
            lines[0].event,
            Event::StoreSignedIn {
                leader,
                human: "ayse".into()
            }
        );
        assert_eq!(
            lines[1].event,
            Event::StoreImported {
                leader,
                repo: "r".into(),
                warnings: vec!["big".into()],
                hardened: true,
            }
        );
        assert_eq!(
            lines[2].event,
            Event::StoreFailed {
                leader,
                act: StoreAct::Login,
                why: Some("no key".into())
            }
        );
        assert_eq!(
            lines[3].event,
            Event::StoreFailed {
                leader,
                act: StoreAct::Import,
                why: None
            }
        );
    }

    #[test]
    fn a_proposal_verified_is_said_with_its_checks_and_none_is_not() {
        let mut w = world(vec![lead(0), agent(1, Role::Worker, Status::Idle)]);
        let at_first = w.journal.len();
        w.verified(1, vec![]);
        assert_eq!(w.journal.len(), at_first);
        let check = |ok| Attested {
            check: "build".into(),
            ok,
            end: "exit 0, 1.0 s".into(),
            lines: vec![],
        };
        let last = |w: &World| w.journal.last().unwrap().event.clone();
        w.verified(1, vec![check(true), check(true)]);
        assert!(
            matches!(last(&w), Event::Verified { agent: 1, ok: true, checks } if checks.len() == 2)
        );
        w.verified(1, vec![check(true), check(false)]);
        assert!(matches!(last(&w), Event::Verified { ok: false, .. }));
        w.verified(1, vec![check(false), check(true)]);
        assert!(matches!(last(&w), Event::Verified { ok: false, .. }));
    }

    #[test]
    fn leases_are_of_the_repository_named_and_go_with_a_store_that_is_down() {
        let mut w = world(vec![lead(0)]);
        let lease = |symbol: &str| StoreLease {
            file: "a.rs".into(),
            symbol: symbol.into(),
            holder: None,
            name: "x".into(),
            shared: false,
        };
        assert!(
            !w.store_leases(0, "r", vec![lease("f")]),
            "on no repository"
        );
        w.on_lonca(0, Some("r".into()));
        assert!(!w.store_leases(0, "other", vec![lease("f")]), "not its");
        assert!(w.projects[0].lonca.as_ref().unwrap().leases.is_empty());
        assert!(w.store_leases(0, "r", vec![lease("f")]));
        assert!(!w.store_leases(0, "r", vec![lease("f")]), "the same");
        assert!(w.store_leases(0, "r", vec![lease("g")]));
        assert_eq!(w.projects[0].lonca.as_ref().unwrap().leases, [lease("g")]);
        w.store_seen(
            0,
            "r",
            StoreSeen::Up {
                trunk: "t".into(),
                pending: 0,
                behind: Behind::Unknown,
            },
        );
        assert_eq!(w.projects[0].lonca.as_ref().unwrap().leases.len(), 1);
        w.store_seen(0, "r", StoreSeen::Down(StoreDown::NoDaemon));
        assert!(w.projects[0].lonca.as_ref().unwrap().leases.is_empty());
    }

    fn a_pending(seq: u64, name: &str, holder: Option<AgentId>, paths: &[&str]) -> StorePending {
        StorePending {
            seq,
            change: format!("c{seq}"),
            name: name.into(),
            holder,
            paths: paths.iter().map(|p| p.to_string()).collect(),
            on_trunk: true,
        }
    }

    #[test]
    fn the_policy_s_checks_are_of_its_repository_and_unsaid_with_a_store_that_is_down() {
        let mut w = world(vec![lead(0)]);
        let names = |n: &[&str]| Some(n.iter().map(|c| c.to_string()).collect::<Vec<_>>());
        assert!(!w.store_checks(0, "r", names(&["api"])), "on no repository");
        assert_eq!(w.policy_checks(0), None, "off Lonca");
        w.on_lonca(0, Some("r".into()));
        assert!(!w.store_checks(0, "other", names(&["api"])), "not its");
        assert_eq!(w.policy_checks(0), None, "not said yet");
        assert!(w.store_checks(0, "r", names(&["api"])));
        assert!(!w.store_checks(0, "r", names(&["api"])), "the same");
        assert!(w.store_checks(0, "r", names(&[])));
        assert_eq!(w.policy_checks(0), Some(&[][..]), "a policy with none");
        assert!(w.store_checks(0, "r", names(&["api", "e2e"])));
        assert_eq!(
            w.policy_checks(0),
            Some(&["api".to_string(), "e2e".into()][..])
        );
        w.store_seen(0, "r", StoreSeen::Down(StoreDown::NoDaemon));
        assert_eq!(w.policy_checks(0), None);
    }

    #[test]
    fn what_waits_in_the_repository_is_of_it_and_goes_with_a_store_that_is_down() {
        let mut w = world(vec![lead(0)]);
        let waits = |seq| vec![a_pending(seq, "x", None, &["a"])];
        assert!(!w.store_pending(0, "r", waits(5)), "on no repository");
        w.on_lonca(0, Some("r".into()));
        assert!(!w.store_pending(0, "other", waits(5)), "not its");
        assert!(w.projects[0].lonca.as_ref().unwrap().pending.is_empty());
        assert!(w.store_pending(0, "r", waits(5)));
        assert!(!w.store_pending(0, "r", waits(5)), "the same");
        assert!(w.store_pending(0, "r", waits(6)));
        assert_eq!(w.projects[0].lonca.as_ref().unwrap().pending, waits(6));
        w.store_seen(0, "r", StoreSeen::Down(StoreDown::NoDaemon));
        assert!(w.projects[0].lonca.as_ref().unwrap().pending.is_empty());
    }

    #[test]
    fn the_verdict_counts_the_proposals_waiting_in_the_repository_that_no_change_shows() {
        let mut w = crate::live::testing::team(3);
        w.on_lonca(0, Some("r".into()));
        w.change_ready(change(1, "T", &[("a", 1, 0), ("b", 1, 0)]));
        w.changes[0].check = Check::Ran(Checked {
            ok: true,
            end: "exit 0".into(),
            lines: Vec::new(),
        });
        assert!(w.verdict(1).unwrap().ready);
        w.store_pending(
            0,
            "r",
            vec![
                // Its own: judged by the change itself.
                a_pending(3, "w1", Some(1), &["a"]),
                // A stranger's.
                a_pending(12, "ayse", None, &["z", "b"]),
                // A worker's whose turn is not over.
                a_pending(8, "w2", Some(2), &["a", "q"]),
                // One that shares nothing.
                a_pending(9, "w3", Some(3), &["q"]),
            ],
        );
        let v = w.verdict(1).unwrap();
        assert_eq!(v.others, 3);
        assert_eq!(
            v.overlaps,
            [
                Overlap {
                    with: Other::Proposal {
                        seq: 12,
                        name: "ayse".into()
                    },
                    files: vec!["b".into()],
                },
                Overlap {
                    with: Other::Proposal {
                        seq: 8,
                        name: "w2".into()
                    },
                    files: vec!["a".into()],
                },
            ]
        );
        assert!(!v.ready);
        // Worker 2's change is shown now: its proposal is judged as that.
        w.change_ready(change(2, "U", &[("q", 1, 0)]));
        let v = w.verdict(1).unwrap();
        assert_eq!(v.others, 3, "one change and two proposals");
        assert_eq!(v.overlaps.len(), 1);
        assert_eq!(
            v.overlaps[0].with,
            Other::Proposal {
                seq: 12,
                name: "ayse".into()
            }
        );
        // Off Lonca nothing of it counts.
        w.on_lonca(0, None);
        let v = w.verdict(1).unwrap();
        assert_eq!((v.others, v.overlaps.len()), (1, 0));
        assert!(v.ready);
    }

    #[test]
    fn a_kept_refusal_of_critical_paths_is_said_by_the_project() {
        let mut w = world(vec![lead(0)]);
        w.store_critical_kept(&ask(0), Some("no".into()));
        assert_eq!(
            w.journal.last().unwrap().event,
            Event::StoreCriticalKept {
                leader: 0,
                repo: "r".into(),
                why: Some("no".into())
            }
        );
    }

    #[test]
    fn an_import_does_not_replace_a_repository_named_while_it_ran() {
        let mut w = world(vec![lead(0)]);
        w.on_lonca(0, Some("other".into()));
        w.store_imported(&ask(0), vec![], false);
        assert_eq!(w.projects[0].lonca.as_ref().unwrap().repo, "other");
        assert!(matches!(
            &w.journal.last().unwrap().event,
            Event::StoreImported { repo, .. } if repo == "r"
        ));
        w.on_lonca(0, None);
        w.store_imported(&ask(0), vec![], false);
        assert_eq!(w.projects[0].lonca.as_ref().unwrap().repo, "r");
    }

    #[test]
    fn a_sync_line_belongs_to_its_project_and_is_no_work_done() {
        let mut w = world(vec![lead(0)]);
        let p = second(&mut w, "a");
        let leader = w.projects[p].leader;
        w.store_synced(p, "r", Synced::Moved);
        assert!(!w.did_anything(p));
        let e = &w.journal.last().unwrap().event;
        assert_eq!((e.agent(), e.kind()), (leader, EntryKind::Assumption));
        assert_eq!(
            *e,
            Event::StoreSynced {
                leader,
                repo: "r".into(),
                how: Synced::Moved
            }
        );
        let job = StoreJob::Sync {
            project: 5,
            repo: "r".into(),
        };
        assert_eq!((job.project(), job.act()), (5, StoreAct::Sync));
    }

    #[test]
    fn a_store_job_knows_its_project_and_act() {
        assert_eq!(StoreJob::Login(3).project(), 3);
        assert_eq!(StoreJob::Import(ask(4)).project(), 4);
        assert_eq!(StoreJob::Login(3).act(), StoreAct::Login);
        assert_eq!(StoreJob::Import(ask(4)).act(), StoreAct::Import);
    }

    fn a_store_proposal(seq: u64) -> StoreProposal {
        StoreProposal {
            seq,
            change: format!("c{seq}"),
            why: Vec::new(),
            redacted: Vec::new(),
            cut: false,
            verifies: Runs::default(),
            on_trunk: true,
        }
    }

    #[test]
    fn a_proposal_is_its_workers_change_until_it_leaves() {
        let mut w = crate::live::testing::team(1);
        w.change_proposed(change(1, "T", &[("a", 1, 0)]), a_store_proposal(5));
        assert_eq!(w.proposal_of(1).map(|p| p.seq), Some(5));
        assert_eq!(w.open_inbox().len(), 1);
        // A newer change takes both the row and the proposal's place.
        w.change_proposed(change(1, "U", &[("a", 1, 0)]), a_store_proposal(6));
        assert_eq!(w.proposal_of(1).map(|p| p.seq), Some(6));
        assert_eq!(w.open_inbox().len(), 1);
        w.proposal_left(1, Landed::Withdrawn);
        assert!(w.proposal_of(1).is_none() && w.change_of(1).is_none());
        assert!(w.open_inbox().is_empty());
        assert!(matches!(
            w.journal.last().unwrap().event,
            Event::Landed {
                agent: 1,
                how: Landed::Withdrawn
            }
        ));
    }

    #[test]
    fn the_verdict_is_ready_when_easy_alone_fitting_and_whole() {
        let run = |code| Ran {
            line: "cargo test".into(),
            code: Some(code),
            timed_out: false,
        };
        let mut w = crate::live::testing::team(2);
        assert!(w.verdict(1).is_none(), "no change");
        let mut c = change(1, "T", &[("src/a.rs", 1, 0), ("Cargo.toml", 1, 0)]);
        c.runs = vec![run(0)];
        w.change_ready(c);
        let v = w.verdict(1).unwrap();
        assert_eq!(
            v,
            Verdict {
                ready: true,
                proof: vec![Proof::Run(run(0))],
                files: 2,
                card: None,
                outside: Vec::new(),
                others: 0,
                overlaps: Vec::new(),
                clashes: false,
                partial: false,
                stale: false,
                verifies: None,
                asks: Vec::new(),
                unknown: Vec::new(),
            }
        );
        // Not proved clean: not ready.
        w.changes[0].runs = vec![run(0), run(1)];
        let v = w.verdict(1).unwrap();
        assert!(!v.ready);
        assert_eq!(v.proof, [Proof::Run(run(0)), Proof::Run(run(1))]);
        w.changes[0].runs.clear();
        assert_eq!(w.verdict(1).unwrap().proof, []);
        assert!(!w.verdict(1).unwrap().ready, "nothing ran");
        w.changes[0].check = Check::Ran(Checked {
            ok: true,
            end: "exit 0".into(),
            lines: Vec::new(),
        });
        w.changes[0].runs = vec![run(1)];
        let v = w.verdict(1).unwrap();
        assert_eq!(
            v.proof,
            [
                Proof::Check {
                    ok: true,
                    end: "exit 0".into()
                },
                Proof::Run(run(1))
            ],
            "the check first, then the runs, as they ran"
        );
        assert!(v.ready, "the check speaks over the runs");
        assert!(
            Proof::Check {
                ok: true,
                end: String::new()
            }
            .ok()
        );
        assert!(!Proof::Run(run(1)).ok());
    }

    #[test]
    fn the_verdict_counts_the_other_changes_waiting_in_the_project_and_what_they_share() {
        let mut w = crate::live::testing::team(2);
        w.change_ready(change(1, "T", &[("src/a.rs", 1, 0), ("Cargo.toml", 1, 0)]));
        w.changes[0].check = Check::Ran(Checked {
            ok: true,
            end: "exit 0".into(),
            lines: Vec::new(),
        });
        w.change_ready(change(
            2,
            "U",
            &[("src/b.rs", 1, 0), ("src/a.rs.bak", 1, 0)],
        ));
        let v = w.verdict(1).unwrap();
        assert_eq!(
            (v.others, v.overlaps.len()),
            (1, 0),
            "a name alike is no overlap"
        );
        assert!(v.ready);
        // The same file: an overlap, in this change's order, and not ready.
        w.change_ready(change(
            2,
            "U",
            &[("Cargo.toml", 1, 0), ("src/a.rs", 2, 2), ("x", 0, 1)],
        ));
        let v = w.verdict(1).unwrap();
        assert_eq!(
            v.overlaps,
            [Overlap {
                with: Other::Change {
                    agent: 2,
                    card: None
                },
                files: vec!["src/a.rs".into(), "Cargo.toml".into()]
            }]
        );
        assert!(!v.ready);
        assert_eq!(
            w.verdict(2).unwrap().overlaps[0].files,
            ["Cargo.toml", "src/a.rs"],
            "seen from the other side"
        );
        assert!(matches!(
            w.verdict(2).unwrap().overlaps[0].with,
            Other::Change { agent: 1, .. }
        ));
        // Another project's change is nobody's business here.
        w.agents[2].project = 7;
        let v = w.verdict(1).unwrap();
        assert_eq!((v.others, v.overlaps.len()), (0, 0));
        assert!(v.ready);
    }

    #[test]
    fn the_verdict_is_judged_by_the_card_the_change_was_made_for() {
        let mut w = crate::live::testing::team(2);
        w.change_ready(change(1, "T", &[("src/a.rs", 1, 0), ("Cargo.toml", 1, 0)]));
        w.changes[0].check = Check::Ran(Checked {
            ok: true,
            end: "exit 0".into(),
            lines: Vec::new(),
        });
        let card = |id: CardId, paths: &[&str]| Card {
            id,
            project: 0,
            title: format!("c{id}"),
            owner: Some(1),
            column: Column::Done,
            story: Vec::new(),
            symbols: Vec::new(),
            paths: crate::paths::parse(&serde_json::json!(paths)).unwrap(),
            checks: Vec::new(),
            started: 0,
            finished: 0,
            attempt: 1,
            breaker: Default::default(),
            after: Vec::new(),
        };
        w.cards.push(card(0, &[]));
        w.cards.push(card(1, &["src/"]));
        w.cards.push(card(2, &["src/", "Cargo.toml"]));
        // The worker's card, when the item carries none.
        w.agents[1].card = Some(0);
        let v = w.verdict(1).unwrap();
        assert_eq!((v.card, v.outside.len()), (None, 0), "owns no paths");
        assert!(v.ready);
        w.agents[1].card = Some(1);
        let v = w.verdict(1).unwrap();
        assert_eq!(
            (v.card, &v.outside[..]),
            (Some(1), &["Cargo.toml".to_string()][..])
        );
        assert!(!v.ready, "a file outside");
        // The item's card over the worker's: the worker moved on.
        let i = w.inbox.len() - 1;
        w.inbox[i].card = Some(2);
        assert_eq!(w.change_card(1), Some(2));
        let v = w.verdict(1).unwrap();
        assert_eq!((v.card, v.outside.len()), (Some(2), 0));
        assert!(v.ready);
        w.inbox[i].open = false;
        assert_eq!(w.change_card(1), Some(1), "a closed item says nothing");
        // The other change's card too.
        w.inbox[i].open = true;
        w.change_ready(change(2, "U", &[("src/a.rs", 1, 0)]));
        w.agents[2].card = Some(1);
        assert!(matches!(
            w.verdict(1).unwrap().overlaps[0].with,
            Other::Change { card: Some(1), .. }
        ));
    }

    #[test]
    fn the_verdict_says_a_clash_a_patch_not_whole_and_what_lonca_runs() {
        let mut w = crate::live::testing::team(2);
        w.change_ready(change(1, "T", &[("a", 1, 0)]));
        w.changes[0].runs = vec![Ran {
            line: "make".into(),
            code: Some(0),
            timed_out: false,
        }];
        assert!(w.verdict(1).unwrap().ready);
        w.change_clashed(
            1,
            Clashes {
                hunks: vec![Clash {
                    file: "a".into(),
                    ..Clash::default()
                }],
                ..Clashes::default()
            },
        );
        let v = w.verdict(1).unwrap();
        assert!(v.clashes && !v.ready);
        w.change_clashed(1, Clashes::default());
        let v = w.verdict(1).unwrap();
        assert!(!v.clashes && v.ready);
        // On Lonca: what runs after the yes; a patch redacted or cut.
        let mut p = a_store_proposal(5);
        p.verifies = Runs::These(vec!["build".into()]);
        let mut c = change(1, "T", &[("a", 1, 0)]);
        c.runs = w.changes[0].runs.clone();
        w.change_proposed(c.clone(), p.clone());
        let v = w.verdict(1).unwrap();
        assert_eq!(v.verifies, Some(Runs::These(vec!["build".to_string()])));
        assert!(!v.partial && !v.stale && v.ready);
        // Based before trunk: said, and still ready, for the queue merges it.
        p.on_trunk = false;
        w.change_proposed(c.clone(), p.clone());
        let v = w.verdict(1).unwrap();
        assert!(v.stale && v.ready);
        p.on_trunk = true;
        p.redacted = vec!["x.env".into()];
        w.change_proposed(c.clone(), p.clone());
        let v = w.verdict(1).unwrap();
        assert!(v.partial && !v.ready);
        p.redacted.clear();
        p.cut = true;
        w.change_proposed(c, p);
        let v = w.verdict(1).unwrap();
        assert!(v.partial && !v.ready);
    }

    #[test]
    fn the_verdict_says_which_of_its_card_s_checks_the_yes_asks_for() {
        let mut w = crate::live::testing::team(2);
        let names = |n: &[&str]| n.iter().map(|s| s.to_string()).collect::<Vec<String>>();
        let c = change(1, "T", &[("a", 1, 0)]);
        // Off Lonca nothing asks: nothing would run them.
        w.change_ready(c.clone());
        assert!(w.asked_checks(1).is_empty(), "no card yet");
        w.cards.push(card(0, Some(1), Vec::new()));
        w.cards[0].checks = names(&["api", "e2e"]);
        w.agents[1].card = Some(0);
        assert_eq!(w.asked_checks(1), names(&["api", "e2e"]));
        assert_eq!(w.asked_checks(2), Vec::<String>::new(), "another worker's");
        assert!(w.verdict(1).unwrap().asks.is_empty());
        // On Lonca: those its runs leave out; all of them when Lonca did
        // not say what runs.
        let mut p = a_store_proposal(5);
        p.verifies = Runs::These(names(&["build", "api"]));
        w.change_proposed(c.clone(), p.clone());
        let v = w.verdict(1).unwrap();
        assert_eq!(v.asks, ["e2e"]);
        p.verifies = Runs::These(names(&["api", "e2e"]));
        w.change_proposed(c.clone(), p.clone());
        assert!(w.verdict(1).unwrap().asks.is_empty(), "both run anyway");
        p.verifies = Runs::Unsaid;
        w.change_proposed(c.clone(), p.clone());
        assert_eq!(w.verdict(1).unwrap().asks, ["api", "e2e"]);
        assert!(
            w.verdict(1).unwrap().unknown.is_empty(),
            "Lonca has not said"
        );
    }

    #[test]
    fn a_check_its_card_asks_for_that_the_policy_dropped_keeps_a_change_from_ready() {
        let mut w = crate::live::testing::team(2);
        let names = |n: &[&str]| n.iter().map(|s| s.to_string()).collect::<Vec<String>>();
        let mut c = change(1, "T", &[("a", 1, 0)]);
        c.runs = vec![Ran {
            line: "cargo test".into(),
            code: Some(0),
            timed_out: false,
        }];
        w.cards.push(card(0, Some(1), Vec::new()));
        w.cards[0].checks = names(&["api", "e2e"]);
        w.agents[1].card = Some(0);
        // Off Lonca the policy says nothing.
        w.change_ready(c.clone());
        assert!(w.verdict(1).unwrap().unknown.is_empty());
        w.on_lonca(0, Some("demo".into()));
        w.store_checks(0, "demo", Some(names(&["api"])));
        assert!(w.verdict(1).unwrap().unknown.is_empty(), "no proposal");
        let mut p = a_store_proposal(5);
        p.verifies = Runs::These(names(&["build"]));
        w.change_proposed(c.clone(), p.clone());
        let v = w.verdict(1).unwrap();
        assert_eq!(
            (v.unknown.as_slice(), v.asks.as_slice()),
            (&names(&["e2e"])[..], &names(&["api"])[..])
        );
        assert!(!v.ready, "Lonca would refuse the yes");
        p.verifies = Runs::Unsaid;
        w.change_proposed(c.clone(), p.clone());
        let v = w.verdict(1).unwrap();
        assert_eq!(
            (v.unknown.as_slice(), v.asks.as_slice()),
            (&names(&["e2e"])[..], &names(&["api"])[..])
        );
        // The policy has both again: ready, both asked for.
        w.store_checks(0, "demo", Some(names(&["api", "e2e"])));
        p.verifies = Runs::These(names(&["build"]));
        w.change_proposed(c, p);
        let v = w.verdict(1).unwrap();
        assert!(v.unknown.is_empty() && v.ready, "{v:?}");
        assert_eq!(v.asks, names(&["api", "e2e"]));
    }

    #[test]
    fn what_a_proposal_cost_is_carried_until_it_is_on_trunk() {
        let spend = Spend {
            cost: 40,
            ticks: 3,
            rounds: 1,
            unpriced: false,
        };
        let mut w = crate::live::testing::team(1);
        w.change_proposed(change(1, "T", &[("a", 1, 0)]), a_store_proposal(5));
        w.changes[0].spend = Some(spend);
        w.proposal_left(1, Landed::Rejected(vec!["no".into()]));
        assert_eq!(w.agents[1].task.carried, spend);
        w.proposal_landed(1, Landed::Conflicted(Vec::new()));
        assert_eq!(w.agents[1].task.carried, spend, "not on trunk");
        w.proposal_landed(1, Landed::Integrated("c1".into()));
        assert_eq!(w.agents[1].task.carried, Spend::default());
    }

    #[test]
    fn a_landed_proposal_counts_as_what_it_came_to() {
        let kind = |how| Event::Landed { agent: 1, how }.kind();
        assert_eq!(kind(Landed::Integrated("c".into())), EntryKind::Merged);
        assert_eq!(kind(Landed::Conflicted(Vec::new())), EntryKind::Failed);
        assert_eq!(kind(Landed::Rejected(Vec::new())), EntryKind::Failed);
        assert_eq!(kind(Landed::Queued), Event::Rejected { agent: 1 }.kind());
        assert_eq!(kind(Landed::Withdrawn), Event::Rejected { agent: 1 }.kind());
        assert_eq!(kind(Landed::Replaced), Event::Rejected { agent: 1 }.kind());
        assert_eq!(
            Event::Landed {
                agent: 3,
                how: Landed::Queued
            }
            .agent(),
            3
        );
    }

    #[test]
    fn a_decision_is_a_job_of_its_project() {
        let job = StoreJob::Decide {
            project: 4,
            repo: "r".into(),
            agent: 1,
            proposal: a_store_proposal(5),
            verdict: Decision::Approve(Vec::new()),
        };
        assert_eq!((job.project(), job.act()), (4, StoreAct::Decide));
    }

    #[test]
    fn only_a_row_of_a_change_with_nothing_behind_it_closes_at_a_start() {
        let mut w = crate::live::testing::team(2);
        w.change_ready(change(1, "T", &[("a", 1, 0)]));
        w.change_ready(change(2, "U", &[("a", 1, 0)]));
        w.changes.retain(|c| c.agent == 1);
        w.asked_you(2, "Which?");
        w.close_empty_changes();
        let open: Vec<(InboxKind, AgentId)> = (w.open_inbox().into_iter())
            .map(|i| (w.inbox[i].kind, w.inbox[i].agent))
            .collect();
        assert_eq!(open.len(), 2, "{open:?}");
        assert!(open.contains(&(InboxKind::Change, 1)));
        assert!(open.contains(&(InboxKind::Question, 2)));
    }
}

#[cfg(test)]
mod change_proof_tests {
    use super::*;

    fn ran(code: i32) -> Ran {
        Ran {
            line: "cargo test".into(),
            code: Some(code),
            timed_out: false,
        }
    }

    fn change(runs: Vec<Ran>, check: Check) -> Change {
        Change {
            agent: 0,
            title: "t".into(),
            files: vec![
                ChangedFile {
                    path: "src/a.rs".into(),
                    added: 1,
                    removed: 0,
                    binary: false,
                },
                ChangedFile {
                    path: "docs/b.md".into(),
                    added: 1,
                    removed: 0,
                    binary: false,
                },
            ],
            patch: String::new(),
            runs,
            check,
            spend: None,
        }
    }

    fn checked(ok: bool) -> Check {
        Check::Ran(Checked {
            ok,
            end: "exit".into(),
            lines: Vec::new(),
        })
    }

    fn card(paths: &[&str]) -> Card {
        let v = serde_json::json!(paths);
        Card {
            id: 3,
            project: 0,
            title: "c".into(),
            column: Column::Waiting,
            owner: None,
            symbols: Vec::new(),
            story: Vec::new(),
            paths: crate::paths::parse(&v).unwrap(),
            checks: Vec::new(),
            started: 0,
            finished: 0,
            attempt: 1,
            breaker: Default::default(),
            after: Vec::new(),
        }
    }

    #[test]
    fn the_proof_is_the_check_when_it_ran_else_the_runs_else_nothing() {
        assert_eq!(change(Vec::new(), Check::None).proof(), None);
        assert_eq!(change(Vec::new(), Check::Asked).proof(), None);
        assert_eq!(change(Vec::new(), Check::Running).proof(), None);
        assert_eq!(change(Vec::new(), Check::Failed("x".into())).proof(), None);
        assert_eq!(change(vec![ran(0)], Check::None).proof(), Some(true));
        assert_eq!(
            change(vec![ran(0), ran(0)], Check::None).proof(),
            Some(true)
        );
        assert_eq!(
            change(vec![ran(0), ran(1)], Check::None).proof(),
            Some(false)
        );
        assert_eq!(change(vec![ran(1)], Check::Asked).proof(), Some(false));
        assert_eq!(change(Vec::new(), checked(true)).proof(), Some(true));
        assert_eq!(change(Vec::new(), checked(false)).proof(), Some(false));
        // The check speaks over the runs, either way.
        assert_eq!(change(vec![ran(1)], checked(true)).proof(), Some(true));
        assert_eq!(change(vec![ran(0)], checked(false)).proof(), Some(false));
    }

    #[test]
    fn the_files_outside_are_those_the_card_s_paths_do_not_name() {
        let c = change(Vec::new(), Check::None);
        assert!(c.outside(None).is_empty(), "no card");
        assert!(c.outside(Some(&card(&[]))).is_empty(), "owns no paths");
        assert_eq!(c.outside(Some(&card(&["src/"]))), ["docs/b.md"]);
        assert_eq!(
            c.outside(Some(&card(&["tests/"]))),
            ["src/a.rs", "docs/b.md"]
        );
        assert!(c.outside(Some(&card(&["src/", "docs/"]))).is_empty());
    }

    #[test]
    fn easy_means_proved_clean_and_inside() {
        let inside = card(&["src/", "docs/"]);
        let part = card(&["src/"]);
        assert!(change(vec![ran(0)], Check::None).easy(Some(&inside)));
        assert!(change(vec![ran(0)], Check::None).easy(None));
        assert!(!change(vec![ran(0)], Check::None).easy(Some(&part)));
        assert!(!change(Vec::new(), Check::None).easy(Some(&inside)));
        assert!(!change(vec![ran(1)], Check::None).easy(Some(&inside)));
        assert!(change(Vec::new(), checked(true)).easy(Some(&inside)));
        assert!(!change(Vec::new(), checked(false)).easy(None));
    }

    #[test]
    fn a_card_is_held_back_through_what_it_waits_on_and_its_dependents_are_found_through_others() {
        let mut w = crate::live::testing::team(2);
        let card = |id, column, after: Vec<CardId>| Card {
            id,
            project: 0,
            title: format!("c{id}"),
            owner: None,
            column,
            story: Vec::new(),
            symbols: Vec::new(),
            paths: Vec::new(),
            checks: Vec::new(),
            started: 0,
            finished: 0,
            attempt: 0,
            breaker: Default::default(),
            after,
        };
        w.cards = vec![
            card(0, Column::Working, vec![]),
            card(1, Column::Waiting, vec![0]),
            card(2, Column::Done, vec![]),
            card(3, Column::Waiting, vec![1, 2]),
            card(4, Column::Waiting, vec![1, 3]),
            card(5, Column::Done, vec![2]),
            card(6, Column::Waiting, vec![2]),
        ];
        assert_eq!(
            (0..7).map(|c| w.blocked_on(c)).collect::<Vec<_>>(),
            [None; 7]
        );
        w.cards[0].column = Column::Failed;
        let blocked: Vec<Option<CardId>> = (0..7).map(|c| w.blocked_on(c)).collect();
        assert_eq!(blocked, [None, Some(0), None, Some(0), Some(0), None, None]);
        // The nearest first: one it waits on itself before one further.
        w.cards[3].column = Column::Withdrawn;
        assert_eq!(w.blocked_on(4), Some(3));
        // A done card is not looked through.
        w.cards[2].after = vec![0];
        assert_eq!(w.blocked_on(6), None);
        w.cards[3].column = Column::Waiting;
        // What waits on #1, through others; a card through is left out.
        assert_eq!(w.dependents(0), [1, 3, 4]);
        assert_eq!(w.dependents(1), [3, 4]);
        assert_eq!(w.dependents(4), Vec::<CardId>::new());
        w.cards[4].column = Column::Withdrawn;
        assert_eq!(w.dependents(0), [1, 3]);
    }

    #[test]
    fn a_withdrawn_card_is_through_and_a_done_for_it_comes_late() {
        let mut w = crate::live::testing::team(2);
        let id = 1;
        let c = w.cards.len();
        w.cards.push(Card {
            id: c,
            project: 0,
            title: "t".into(),
            owner: Some(id),
            column: Column::Working,
            story: Vec::new(),
            symbols: Vec::new(),
            paths: Vec::new(),
            checks: Vec::new(),
            started: 0,
            finished: 0,
            attempt: 1,
            breaker: Default::default(),
            after: Vec::new(),
        });
        w.takes(id, c);
        assert_eq!(w.done_for(id), DoneFor::Card(c));
        w.tick = 42;
        w.withdraw(c, 0, "why");
        let card = &w.cards[c];
        assert_eq!((card.column, card.finished), (Column::Withdrawn, 42));
        assert!(card.over());
        assert!(
            matches!(card.story.last().map(|s| &s.event), Some(StoryEvent::Withdrawn(y)) if y == "why")
        );
        assert_eq!(w.done_for(id), DoneFor::Late(c));
        let e = Event::Withdrawn {
            agent: 0,
            card: c,
            why: "why".into(),
        };
        assert_eq!(
            (e.card(), e.agent(), e.kind()),
            (Some(c), 0, EntryKind::Plan)
        );
        assert_eq!(
            StoryEvent::Withdrawn("x".into()).kind(),
            StoryKind::Decision
        );
    }
}
