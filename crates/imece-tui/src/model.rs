//! What the screen shows: projects, agents, cards, the journal, the inbox and
//! the merge queue. Fake events (`sim`) or live agents (`live`) fill it.
//!
//! Events are stored as data, not sentences: the screen words them in the
//! person's language when it draws them.

use std::collections::{BTreeSet, VecDeque};
use std::path::Path;
use std::sync::atomic::{AtomicU64, Ordering};

use serde::{Deserialize, Serialize};

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

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Vendor {
    Claude,
    Codex,
    Gemini,
    Qwen,
    Local,
}

impl Vendor {
    /// The vendors imece can be asked to lead with, by label.
    pub fn parse(s: &str) -> Option<Vendor> {
        [Vendor::Claude, Vendor::Codex, Vendor::Gemini]
            .into_iter()
            .find(|v| v.label() == s)
    }

    pub fn label(self) -> &'static str {
        match self {
            Vendor::Claude => "claude",
            Vendor::Codex => "codex",
            Vendor::Gemini => "gemini",
            Vendor::Qwen => "qwen",
            Vendor::Local => "local",
        }
    }

    /// Fake cost of one busy tick, in ten-thousandths of a dollar.
    pub fn cost_per_tick(self) -> u64 {
        match self {
            Vendor::Claude => 8,
            Vendor::Codex => 7,
            Vendor::Gemini => 5,
            Vendor::Qwen => 3,
            Vendor::Local => 0,
        }
    }
}

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
    pub until: u64,
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
}

/// A vendor session an agent had, kept to go on with it after imece
/// starts again: the vendor's id for it, and what the vendor had reported
/// as the session's whole cost (millionths of a dollar) when last seen.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct Resumable {
    pub session: String,
    pub spent: u64,
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
    /// A solo on a vendor that is not here.
    Vendor(String),
    /// A solo's `[MODEL][@EFFORT]` asks for an effort its vendor lacks.
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
            ProjectRefusal::Vendor(v) => {
                format!("{v} is not on PATH here, or imece does not drive it")
            }
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
         A solo works here without git (--solo VENDOR, :solo); or run \
         git init && git commit --allow-empty -m init first"
    )
}

pub fn valid_project_name(name: &str) -> bool {
    (1..=24).contains(&name.chars().count())
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
/// first argument never matches one with a `sub`.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct Standing {
    pub program: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub sub: Option<String>,
}

impl Standing {
    /// How the screen and the journal name it: `npm run …`, or `make`
    /// (the program with nothing after it).
    pub fn line(&self) -> String {
        match &self.sub {
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
    /// The folder is not a git checkout with a first commit, so its solo
    /// works straight in it: no worktree, no change to review. Found at
    /// each open and start, not saved.
    pub no_git: bool,
    /// Archived by the person (`:archive`): closed, but its agents are
    /// not gone; they sleep with their sessions, mail and changes until
    /// it is brought back. Never opened again in place.
    pub archived: Option<Archived>,
}

#[derive(Clone, Debug)]
pub struct Agent {
    pub id: AgentId,
    pub project: ProjectId,
    pub name: String,
    pub vendor: Vendor,
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
    /// What a live agent is doing right now (`Edit src/main.rs`), drawn
    /// instead of the progress bar; empty while it thinks.
    pub activity: Option<String>,
    /// Its session is being started (the process, a first worktree's
    /// checkout) and has not answered yet: `◌ starting…` on its row.
    pub starting: bool,
    /// The agent's own command line as its session reported it (`/clear`,
    /// `/compact` ...), imece's `/clear` added where the vendor has none;
    /// empty until a session says. The composer completes with it.
    pub commands: Vec<String>,
    /// Where a worker's worktree stands next to its project's checkout, as
    /// git last read it; nothing to show until it is read.
    pub stand: Stand,
    /// The start point a worker was hired from (`hire`'s `from`: a branch,
    /// tag or commit of the checkout, or `#N`, a pull request), as asked;
    /// none for one made at the checkout's HEAD. Such a worktree is made
    /// there and never follows HEAD; its Δ is against that point. Saved.
    pub start: Option<String>,
    /// The newest loopback URL the agent wrote in its text this session
    /// (`local_url`), drawn on its row as a link the terminal opens.
    pub local: Option<String>,
    /// Its own to-do list, as its vendor's planning tool last set it
    /// (`imece_agent::Event::Plan`): `≡ 3/7` on its row, the steps on its
    /// focus. Gone with the session; not kept between runs.
    pub plan: Vec<Step>,
    /// How full its context window is, as its vendor last said
    /// (`imece_agent::Event::Context`): `◔ 72%` on its row from
    /// `CONTEXT_SHOWN`, the figure on its focus. Gone with the session;
    /// not kept between runs.
    pub context: Option<Fill>,
    /// When imece tries its failed turn again after a passing vendor
    /// error, and which try that is (`↻ 1/3` on its row); none when no
    /// retry waits. Memory only.
    pub retry: Option<Retry>,
    /// The tries of a turn that keeps failing on passing vendor errors:
    /// none after a good turn or the person's word. Memory only.
    pub tries: u8,
    /// The fault its vendor said the turn now ending failed on, taken by
    /// that turn's end (`imece_agent::Event::Failed`). Memory only.
    pub fault: Option<Fault>,
    /// A researcher that finished; kept so ids stay stable.
    pub gone: bool,
}

/// A failed turn imece tries again (record 79).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Retry {
    /// Which try this is, from 1 to [`RETRY_TICKS`]' length.
    pub attempt: u8,
    /// The tick it goes at.
    pub at: u64,
}

/// After a passing vendor error, a turn is tried again this many ticks
/// after it failed, one entry per try: 30 s, 2 min, 8 min. When they are
/// used up the error waits for the person.
pub const RETRY_TICKS: [u64; 3] = [6, 24, 96];

/// Why a turn failed on the vendor's side, as far as imece acts on it
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
/// when the vendor says.
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
        vendor: Vendor,
        role: Role,
        status: Status,
        tick: u64,
    ) -> Agent {
        Agent {
            id,
            project,
            name,
            vendor,
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
            activity: None,
            starting: false,
            commands: Vec::new(),
            stand: Stand::default(),
            start: None,
            local: None,
            plan: Vec::new(),
            context: None,
            retry: None,
            tries: 0,
            fault: None,
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
    }
}

/// The ticks an agent's activity bar covers on its row, one column each:
/// 25 s live.
pub const STIR_TICKS: usize = 5;

/// The ticks its longer bar covers on its focus: two minutes live.
pub const STIR_LONG: usize = 24;

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

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Column {
    Waiting,
    Working,
    Verifying,
    Done,
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
            | StoryEvent::Posted { .. } => StoryKind::Intent,
            StoryEvent::Decision(_) => StoryKind::Decision,
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
            | StoryEvent::Interrupted => StoryKind::Evidence,
            StoryEvent::Change { .. } | StoryEvent::Sensitive(_) | StoryEvent::Touched(_) => {
                StoryKind::Change
            }
            StoryEvent::YouApproved { .. }
            | StoryEvent::YouRejected { .. }
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
    pub started: u64,
    pub finished: u64,
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
    /// Leases count while the change is not merged.
    pub fn holds_leases(&self) -> bool {
        matches!(self.column, Column::Working | Column::Verifying)
    }
}

/// Why a remembered agent woke in a new session, not the one it had.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum Fresh {
    /// It had none to go on with: never one, or forgotten, or cleared.
    #[default]
    NoSession,
    /// Its session was not where its vendor keeps it (a Claude transcript
    /// gone, an id that is no session's).
    Missing,
    /// Its vendor would not go on with it; the vendor's words, cut.
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
    /// The person said yes from now on to `rules` for the project
    /// `leader` heads (`added`), or took them back.
    Standing {
        leader: AgentId,
        rules: Vec<String>,
        added: bool,
    },
    /// A live agent sent the person a message over the bus (`to: you`);
    /// an `ask` waits in their inbox, the rest is for the journal.
    ToYou {
        from: AgentId,
        kind: MsgKind,
        text: String,
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
    /// A live agent's turn failed on a passing vendor error (`why` in the
    /// vendor's words); imece tries it again `seconds` later, the
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
    /// A live leader hired a worker; `model` is the one it was given, or
    /// none for the vendor's own default; `from` the start point its
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
    /// vendor's default from now on.
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
    /// where it left off, and what the vendor kept of the old one is gone.
    Forgotten {
        agent: AgentId,
    },
    /// The person changed what a solo runs on (`:model`, `:effort`):
    /// `choice` is the vendor and the `[MODEL][@EFFORT]`, as the row shows
    /// it. Its session starts again on it.
    Chose {
        agent: AgentId,
        choice: String,
    },
    /// Agent `agent`'s session said nothing about its own command line
    /// where its vendor's format puts it (not an empty list: no list): the
    /// format may have changed, and only imece's `/clear` completes. Once
    /// a session.
    CommandsMissing {
        agent: AgentId,
    },
    /// imece started with the confine dial at `level` (`standard` or
    /// `open`, never strict: that has nothing to say): the agents' sessions
    /// get their vendor's shell and web, and isolation is a container's
    /// job, not imece's. Once per start, on the project imece runs in.
    Confine {
        leader: AgentId,
        level: String,
    },
    /// The repository's notes for coding agents (`AGENTS.md` or
    /// `CLAUDE.md`, `size` bytes) were longer than what goes into an
    /// agent's instructions, so the agent's session started with the
    /// file's head only.
    NotesCut {
        agent: AgentId,
        file: String,
        size: usize,
    },
    /// The person's own instructions for every project (`path`, `size`
    /// bytes) were longer than what goes into an agent's instructions,
    /// so the agent's session started with the file's head only.
    InstructionsCut {
        agent: AgentId,
        path: String,
        size: usize,
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
            | Event::Hired { .. }
            | Event::Forked { .. }
            | Event::Left { .. } => EntryKind::Plan,
            Event::Assign { .. } | Event::Claimed { .. } => EntryKind::Assign,
            Event::Talk { .. }
            | Event::Replied { .. }
            | Event::Sent { .. }
            | Event::ToYou { .. } => EntryKind::Talk,
            Event::Assumption { .. }
            | Event::Fallback { .. }
            | Event::Resumed { .. }
            | Event::Restarted { .. }
            | Event::NotesCut { .. }
            | Event::InstructionsCut { .. }
            | Event::Pushed { .. }
            | Event::SetupRan { ok: true, .. }
            | Event::CommandsMissing { .. }
            | Event::Confine { .. }
            | Event::WorktreeMoved { .. }
            | Event::Copied { .. }
            | Event::FilesCut { .. }
            | Event::NoGit { .. }
            | Event::Retrying { .. }
            | Event::WorktreeKept { .. } => EntryKind::Assumption,
            Event::Blocked { .. } => EntryKind::Blocked,
            Event::ResearchOpened { .. }
            | Event::ResearchDelivered { .. }
            | Event::FromCache { .. } => EntryKind::Research,
            Event::ResearchNeedsYou { .. }
            | Event::Sensitive { .. }
            | Event::ChangeReady { .. }
            | Event::Suggested { .. }
            | Event::PotLow { .. } => EntryKind::Sensitive,
            Event::Merged { .. } | Event::Applied { .. } => EntryKind::Merged,
            Event::Failed { .. }
            | Event::Stopped { .. }
            | Event::Interrupted { .. }
            | Event::SuggestFailed { .. }
            | Event::CopySkipped { .. }
            | Event::IncludeRefused { .. }
            | Event::PushFailed { .. }
            | Event::SetupRan { ok: false, .. }
            | Event::SetupFailed { .. }
            | Event::ApplyFailed { .. } => EntryKind::Failed,
            Event::Approved { .. }
            | Event::Rejected { .. }
            | Event::YouSaid { .. }
            | Event::Decided { .. }
            | Event::SetupAnswered { .. }
            | Event::Standing { .. }
            | Event::Budget { .. }
            | Event::Shell { .. }
            | Event::Opened { .. }
            | Event::Closed { .. }
            | Event::Archived { .. }
            | Event::Restored { .. }
            | Event::Renamed { .. }
            | Event::Forgotten { .. }
            | Event::Chose { .. } => EntryKind::Person,
        }
    }

    /// The card the event names, if it names one.
    pub fn card(&self) -> Option<CardId> {
        match *self {
            Event::Assign { card, .. }
            | Event::Merged { card, .. }
            | Event::Posted { card, .. }
            | Event::Claimed { card, .. } => Some(card),
            _ => None,
        }
    }

    /// The agent the event is about, for its project.
    pub fn agent(&self) -> AgentId {
        match *self {
            Event::Plan { leader, .. }
            | Event::FilesCut { leader, .. }
            | Event::NoGit { leader, .. }
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
            | Event::NotesCut { agent, .. }
            | Event::InstructionsCut { agent, .. }
            | Event::Pushed { agent, .. }
            | Event::PushFailed { agent, .. }
            | Event::SetupRan { agent, .. }
            | Event::SetupFailed { agent, .. }
            | Event::SetupAnswered { agent, .. }
            | Event::CommandsMissing { agent }
            | Event::WorktreeMoved { agent, .. }
            | Event::WorktreeKept { agent, .. }
            | Event::Copied { agent, .. }
            | Event::CopySkipped { agent, .. }
            | Event::IncludeRefused { agent, .. }
            | Event::ChangeReady { agent, .. }
            | Event::Applied { agent, .. }
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
    /// A live agent's turn failed on a vendor error only the person mends
    /// (the account: signed out, a key, a quota, a bill), or on a passing
    /// one imece tried again until its tries ran out; the detail is the
    /// vendor's words. Closes when the person writes to the agent.
    Fault,
    /// The checkout's worktree setup script (record 92) waits for the
    /// person's yes to its exact bytes before it runs in a new worktree;
    /// the agent is the one whose worktree it would run in first, the
    /// detail the script's path in the checkout. Closes with the answer,
    /// and at the next start.
    Script,
}

/// What the person cleared off the salon's screen (`:clear`, record 90):
/// nothing is deleted, and a card's screen, `:export`, the archives and
/// `W` still see it all.
#[derive(Clone, Debug, Default, PartialEq, Eq, Hash)]
pub struct Cleared {
    /// The journal lines before this index are not listed, but for the
    /// marked ones.
    pub journal: usize,
    /// The inbox items not listed in WAITING FOR YOU; while open they are
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
    /// is. A vendor's own shell does not count; imece can not see what
    /// that ran on, nor always how it ended.
    pub runs: Vec<Ran>,
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

impl Change {
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
    /// What the vendors last said of their plans' usage windows, one
    /// entry per vendor and window. Not kept between runs: a session
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
    /// The worktrees no agent owns, as the engine last looked (`:tidy`).
    pub orphans: Orphans,
    /// The number the last message on the bus was given ([`MsgId`]).
    pub messages: MsgId,
    /// The lines the person sent, oldest first, for `↑` `↓` and `^r` on
    /// the command line; kept between runs (the newest [`SENT_KEEP`]).
    pub sent: Vec<Sent>,
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
}

impl Orphan {
    /// Whether removing it loses nothing.
    pub fn clean(&self) -> bool {
        self.changes == 0 && self.commits == 0
    }
}

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

/// One window of a vendor plan's usage limit, as the vendor last said it
/// (`imece_agent::Limit`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Limit {
    pub vendor: Vendor,
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

    /// A vendor said how much of one of its windows is used: that window
    /// is as it says now; the others stay as they were.
    pub fn limit(&mut self, l: Limit) {
        let same = |k: &&mut Limit| k.vendor == l.vendor && k.minutes == l.minutes;
        match self.limits.iter_mut().find(same) {
            Some(k) => *k = l,
            None => self.limits.push(l),
        }
    }

    /// Each vendor's window nearest to full (the most used; the shorter
    /// one on a tie), vendors in the order they first said one.
    pub fn tightest(&self) -> Vec<Limit> {
        let mut vendors: Vec<Vendor> = Vec::new();
        for l in &self.limits {
            if !vendors.contains(&l.vendor) {
                vendors.push(l.vendor);
            }
        }
        let fullest = |v: Vendor| {
            (self.limits.iter())
                .filter(|l| l.vendor == v)
                .max_by_key(|l| (l.used, std::cmp::Reverse(l.minutes)))
                .copied()
        };
        vendors.into_iter().filter_map(fullest).collect()
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
    /// `name`, its checkout at `path`, led by a new agent on `vendor` with
    /// the next id. Returns the project's id.
    pub fn open_project(
        &mut self,
        reuse: Option<ProjectId>,
        name: String,
        path: String,
        vendor: Vendor,
        role: Role,
    ) -> ProjectId {
        let p = self.add_project(reuse, name, vendor, role);
        let leader = self.projects[p].leader;
        self.note(Event::Opened { leader, path });
        p
    }

    /// [`open_project`](Self::open_project) without the journal's line.
    fn add_project(
        &mut self,
        reuse: Option<ProjectId>,
        name: String,
        vendor: Vendor,
        role: Role,
    ) -> ProjectId {
        let p = reuse.unwrap_or(self.projects.len());
        let leader = self.agents.len();
        let lead = World::head_name(p, &name, role);
        self.agents.push(Agent::new(
            leader,
            p,
            lead,
            vendor,
            role,
            Status::Idle,
            self.tick,
        ));
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
                no_git: false,
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
    /// pin and copies, its head on the same vendor and called the same.
    /// Returns the fresh project's id.
    pub fn archive_project(&mut self, p: ProjectId, title: String) -> ProjectId {
        let was = self.projects[p].leader;
        let (vendor, role, head) = {
            let a = &self.agents[was];
            (a.vendor, a.role, a.name.clone())
        };
        self.shelve(p, title.clone());
        let q = &self.projects[p];
        let (name, pinned, copies) = (q.name.clone(), q.pinned, q.copies.clone());
        let fresh = self.add_project(None, name, vendor, role);
        let leader = self.projects[fresh].leader;
        self.agents[leader].name = head;
        self.projects[fresh].pinned = pinned;
        self.projects[fresh].copies = copies;
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
    /// archive is, so the salon, the board, the journal, WAITING FOR YOU
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
            run_spent: self.run_spent,
            spending: self.spending.clone(),
            projects,
            agents: self.agents.clone(),
            cards: self.cards.clone(),
            journal,
            inbox: self.inbox.clone(),
            budget: self.budget,
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
            orphans: crate::model::Orphans::default(),
            messages: self.messages,
            sent: self.sent.clone(),
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

    /// The card agent `id` works on: its own, or for a helper its
    /// worker's.
    pub fn card_of(&self, id: AgentId) -> Option<CardId> {
        let a = &self.agents[id];
        match a.parent {
            Some(p) if a.role == Role::Helper => self.agents[p].card,
            _ => a.card,
        }
    }

    /// The open inbox items WAITING FOR YOU lists, oldest first: those
    /// not cleared off it.
    pub fn shown_inbox(&self) -> Vec<usize> {
        let mut open = self.open_inbox();
        open.retain(|i| !self.cleared.inbox.contains(i));
        open
    }

    /// How many open inbox items are cleared off WAITING FOR YOU.
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

    pub fn cards_done(&self) -> usize {
        self.shown_cards()
            .filter(|c| c.column == Column::Done)
            .count()
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
    pub fn change_of(&self, id: AgentId) -> Option<&Change> {
        self.changes.iter().find(|c| c.agent == id)
    }

    /// Worker `change.agent` finished with a change in its worktree: it
    /// waits for the person. An older change of the same worker is part
    /// of it and goes.
    pub fn change_ready(&mut self, change: Change) {
        let agent = change.agent;
        self.forget_change(agent);
        let (added, removed) = change.counts();
        let files = change.files.len();
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
        });
    }

    /// The person applied worker `id`'s change to their checkout; `mark`
    /// is the commit that stands for it in the worktree.
    pub fn change_applied(&mut self, id: AgentId, mark: String) {
        let files = self.change_of(id).map_or(0, |c| c.files.len());
        self.forget_change(id);
        self.note(Event::Applied {
            agent: id,
            files,
            mark,
        });
    }

    /// The person rejected worker `id`'s change; it stays in the worktree.
    pub fn change_rejected(&mut self, id: AgentId) {
        self.forget_change(id);
        self.note(Event::Rejected { agent: id });
    }

    /// Worker `id`'s change did not apply; it still waits.
    pub fn change_failed(&mut self, id: AgentId, why: String) {
        self.note(Event::ApplyFailed { agent: id, why });
    }

    fn forget_change(&mut self, id: AgentId) {
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

    /// A turn of agent `id` failed, `why` in the vendor's words, `fault`
    /// its kind when the vendor said: a passing one is tried again after
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
    /// waited on for a vendor error closes.
    pub fn close_faults(&mut self, id: AgentId) {
        for i in &mut self.inbox {
            if i.kind == InboxKind::Fault && i.agent == id {
                i.open = false;
            }
        }
    }

    /// Whether agent `id` waits for the person's answer to a question.
    pub fn has_question(&self, id: AgentId) -> bool {
        self.open_item(id, InboxKind::Question).is_some()
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

    /// What the `nth` of agent `id`'s open approvals asks waits no more
    /// (its session settled it): it closes, and nobody decided.
    pub fn withdrew(&mut self, id: AgentId, nth: usize) {
        if let Some(i) = self
            .inbox
            .iter_mut()
            .filter(|i| i.open && i.kind == InboxKind::Approval && i.agent == id)
            .nth(nth)
        {
            i.open = false;
        }
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
            if i.kind == InboxKind::Script {
                i.open = false;
            }
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
            until: t + 6,
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
/// Whether `s` may go to a vendor as a model choice (`[MODEL][@EFFORT]`):
/// 1-64 of letters, digits and `-._[]@`, not starting with `-`, so it can
/// never pass as a flag. Model names as the vendors take them: `sonnet`,
/// `claude-opus-5-5`, `opus[1m]`, `gpt-5.6-terra@high`.
pub fn valid_model(s: &str) -> bool {
    (1..=64).contains(&s.len())
        && !s.starts_with('-')
        && s.bytes()
            .all(|b| b.is_ascii_alphanumeric() || b"-._[]@".contains(&b))
}

/// `VENDOR[:MODEL][@EFFORT]` as `--solo` and `:solo` take it: the vendor,
/// and the `[MODEL][@EFFORT]` choice after it (`claude`, `claude:opus`,
/// `claude@xhigh`, `codex:gpt-5.6-terra@high`), none for the vendor's
/// own. None when the vendor is not one imece leads with, or the choice
/// is not a model choice.
pub fn vendor_choice(s: &str) -> Option<(Vendor, Option<String>)> {
    let cut = s.find([':', '@']).unwrap_or(s.len());
    let vendor = Vendor::parse(&s[..cut])?;
    let rest = &s[cut..];
    let choice = match rest.strip_prefix(':') {
        Some(c) => c,
        None => rest,
    };
    if choice.is_empty() {
        return Some((vendor, None));
    }
    valid_model(choice).then(|| (vendor, Some(choice.to_string())))
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
        let mut a = Agent::new(
            0,
            0,
            "a".into(),
            Vendor::Claude,
            Role::Worker,
            Status::Idle,
            0,
        );
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
        let mut a = Agent::new(
            0,
            0,
            "a".into(),
            Vendor::Claude,
            Role::Worker,
            Status::Idle,
            0,
        );
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
        let mut a = Agent::new(
            0,
            0,
            "w".into(),
            Vendor::Claude,
            Role::Worker,
            Status::Idle,
            0,
        );
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
        Agent::new(id, 0, format!("a{id}"), Vendor::Claude, role, status, 0)
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
                no_git: false,
                archived: None,
            }],
            agents,
            cards: Vec::new(),
            journal: Vec::new(),
            inbox: Vec::new(),
            budget: 10_000,
            max_researchers: 0,
            research_queue: Default::default(),
            changes: Vec::new(),
            paid: 0,
            suggested: None,
            orphans: crate::model::Orphans::default(),
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
            vendor: Vendor::Claude,
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
        let at = |vendor, minutes, used| Limit {
            vendor,
            minutes,
            used,
            resets_at: Some(u64::from(minutes)),
        };
        w.limit(at(Vendor::Codex, 300, 10));
        w.limit(at(Vendor::Claude, 300, 40));
        w.limit(at(Vendor::Claude, 10_080, 40));
        // The same window again: replaced, not added.
        w.limit(at(Vendor::Codex, 300, 30));
        assert_eq!(w.limits.len(), 3);
        // Codex first (it said one first); Claude's tie goes to the
        // shorter window.
        assert_eq!(
            w.tightest(),
            [at(Vendor::Codex, 300, 30), at(Vendor::Claude, 300, 40)]
        );
        // The week fills up past the five hours: it is the tightest.
        w.limit(at(Vendor::Claude, 10_080, 41));
        assert_eq!(w.tightest()[1], at(Vendor::Claude, 10_080, 41));
        // A later, emptier window does not take the place.
        w.limit(at(Vendor::Codex, 10_080, 29));
        assert_eq!(w.tightest()[0], at(Vendor::Codex, 300, 30));
        w.limit(at(Vendor::Codex, 60, 30));
        assert_eq!(
            w.tightest()[0],
            at(Vendor::Codex, 60, 30),
            "a tie: the shorter"
        );
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
            started: 0,
            finished: 0,
        };
        for (column, held) in [
            (Column::Waiting, false),
            (Column::Working, true),
            (Column::Verifying, true),
            (Column::Done, false),
        ] {
            c.column = column;
            assert_eq!(c.holds_leases(), held, "{column:?}");
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
        let cut = Event::NotesCut {
            agent: 10,
            file: "CLAUDE.md".into(),
            size: 1,
        };
        assert_eq!((cut.agent(), cut.kind()), (10, EntryKind::Assumption));
        let mine = Event::InstructionsCut {
            agent: 12,
            path: "/x".into(),
            size: 1,
        };
        assert_eq!((mine.agent(), mine.kind()), (12, EntryKind::Assumption));
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
        let p = w.open_project(
            None,
            "notes".into(),
            "/n".into(),
            Vendor::Claude,
            Role::Solo,
        );
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
            w.open_project(
                None,
                "home".into(),
                "/h".into(),
                Vendor::Claude,
                Role::Leader
            ),
            0
        );
        assert_eq!(w.agents[0].name, "lead", "the first project's leader");
        assert_eq!(
            w.open_project(None, "api".into(), "/a".into(), Vendor::Codex, Role::Leader),
            1
        );
        let a = &w.agents[1];
        assert_eq!(
            (
                a.id,
                a.project,
                a.name.as_str(),
                a.vendor,
                a.role,
                a.status,
                a.changed_at
            ),
            (
                1,
                1,
                "lead-api",
                Vendor::Codex,
                Role::Leader,
                Status::Idle,
                9
            )
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
            w.open_project(
                Some(1),
                "api2".into(),
                "/a".into(),
                Vendor::Claude,
                Role::Leader
            ),
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
        w.open_project(
            None,
            "api".into(),
            "/a".into(),
            Vendor::Claude,
            Role::Leader,
        );
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
        w.open_project(None, "p-2".into(), "/r".into(), Vendor::Claude, Role::Solo);
        assert_eq!(free_project_name(&w, "p"), "p-3");
        let long = "a".repeat(24);
        w.open_project(None, long.clone(), "/r".into(), Vendor::Claude, Role::Solo);
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
    fn a_vendor_choice_is_the_vendor_then_a_model_and_or_an_effort() {
        let some = |s: &str| Some(s.to_string());
        assert_eq!(vendor_choice("claude"), Some((Vendor::Claude, None)));
        assert_eq!(vendor_choice("claude:"), Some((Vendor::Claude, None)));
        assert_eq!(
            vendor_choice("claude:opus"),
            Some((Vendor::Claude, some("opus")))
        );
        assert_eq!(
            vendor_choice("claude@xhigh"),
            Some((Vendor::Claude, some("@xhigh")))
        );
        assert_eq!(
            vendor_choice("claude:@max"),
            Some((Vendor::Claude, some("@max")))
        );
        assert_eq!(
            vendor_choice("codex:gpt-5.6-terra@high"),
            Some((Vendor::Codex, some("gpt-5.6-terra@high")))
        );
        assert_eq!(
            vendor_choice("gemini:pro"),
            Some((Vendor::Gemini, some("pro")))
        );
        for bad in [
            "",
            "qwen",
            "qwen:big",
            "claude:-x",
            "claude:a b",
            "Claude",
            "notes",
        ] {
            assert_eq!(vendor_choice(bad), None, "{bad:?}");
        }
    }

    #[test]
    fn a_model_choice_can_not_pass_as_a_flag() {
        assert!(valid_model("sonnet") && valid_model("opus[1m]") && valid_model("claude-opus-5-5"));
        assert!(valid_model("@low") && valid_model("gpt-5.6-terra@xhigh"));
        for bad in [
            "",
            "--dangerously-skip-permissions",
            "a b",
            "x;y",
            "m/../n",
            "codex:x",
        ] {
            assert!(!valid_model(bad), "{bad}");
        }
        assert!(!valid_model(&"a".repeat(65)) && valid_model(&"a".repeat(64)));
    }

    #[test]
    fn a_solo_project_s_head_is_the_solo_agent_and_renames_as_one() {
        let mut w = world(vec![agent(0, Role::Leader, Status::Idle)]);
        let p = w.open_project(None, "notes".into(), "/r".into(), Vendor::Codex, Role::Solo);
        assert_eq!(p, 1);
        let head = w.projects[p].leader;
        let a = &w.agents[head];
        assert_eq!(
            (a.name.as_str(), a.role, a.vendor, a.project, a.status),
            ("solo-notes", Role::Solo, Vendor::Codex, 1, Status::Idle)
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
        let card = |column| Card {
            id: 0,
            project: 0,
            title: "t".into(),
            owner: None,
            column,
            story: Vec::new(),
            symbols: Vec::new(),
            started: 0,
            finished: 0,
        };
        w.cards = vec![
            card(Column::Done),
            card(Column::Working),
            card(Column::Done),
        ];
        assert_eq!(w.cards_done(), 2);
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
            no_git: false,
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
    fn vendors_cost_what_they_cost() {
        let costs: Vec<u64> = [
            Vendor::Claude,
            Vendor::Codex,
            Vendor::Gemini,
            Vendor::Qwen,
            Vendor::Local,
        ]
        .map(Vendor::cost_per_tick)
        .to_vec();
        assert_eq!(costs, [8, 7, 5, 3, 0]);
    }

    #[test]
    fn what_the_person_says_shows_for_a_moment() {
        let mut w = world(vec![agent(0, Role::Leader, Status::Idle)]);
        w.tick = 3;
        w.said(0, "hello");
        let talk = w.agents[0].talk.as_ref().unwrap();
        assert_eq!(talk.until, 3 + 6);
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
        }
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
            started: 0,
            finished: 0,
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
        // Only its own vendor errors close for an agent.
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
    fn a_withdrawn_ask_is_the_nth_open_one_of_its_agent_and_nobody_decided_it() {
        let mut w = world(vec![
            agent(0, Role::Leader, Status::Idle),
            agent(1, Role::Worker, Status::Working),
        ]);
        w.asks_you(1, "shell: make a");
        w.asked_you(1, "Which?");
        w.asks_you(0, "Bash: make lead");
        w.asks_you(1, "shell: make b");
        w.asks_you(1, "shell: make c");
        w.decided(0, true);
        let journal = w.journal.len();
        // The first open one of agent 1 is now `make b`: the decided one,
        // the question and the leader's ask do not count.
        w.withdrew(1, 0);
        let open: Vec<_> = w
            .open_inbox()
            .iter()
            .map(|&i| w.inbox[i].detail.as_str())
            .collect();
        assert_eq!(open, ["Which?", "Bash: make lead", "shell: make c"]);
        assert_eq!(w.journal.len(), journal);
        w.withdrew(1, 1);
        assert!(w.has_approval(1), "there is no second one");
        w.withdrew(1, 0);
        assert!(!w.has_approval(1) && w.has_approval(0) && w.has_question(1));
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
                removed: 1
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
        w.change_applied(1, "abc1234".into());
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
            started: 0,
            finished: 0,
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
            no_git: false,
            archived: None,
        });
        w.agents.push(Agent::new(
            3,
            1,
            "lead-q".into(),
            Vendor::Codex,
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
        assert_eq!(w.cards_done(), 2);

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
            (lead.name.as_str(), lead.project, lead.role, lead.vendor),
            ("lead", 2, Role::Leader, Vendor::Claude)
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
        assert_eq!(w.cards_done(), 1);
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
                no_git: false,
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
            no_git: false,
            archived: None,
        });
        w.agents.push(Agent::new(
            1,
            1,
            "lead-q".into(),
            Vendor::Claude,
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
            Vendor::Claude,
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
        w.agents.push(Agent::new(
            1,
            1,
            "x".into(),
            Vendor::Claude,
            Role::Leader,
            Status::Idle,
            0,
        ));
        w.journal[0].event = Event::Failed { agent: 1 };
        w.journal.push(Entry {
            tick: w.tick,
            event: Event::Failed { agent: 0 },
        });
        assert_eq!(w.archive_title(0), format!("p {then}"));
    }
}
