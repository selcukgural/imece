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

    /// Statuses the person should see first.
    pub fn needs_attention(self) -> bool {
        matches!(
            self,
            Status::Blocked | Status::WaitingHuman | Status::Stopped
        )
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

/// Who an agent is talking with right now, for the salon's `⇄` line; also
/// the two ends of a conversation line.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum Party {
    Agent(AgentId),
    Person,
}

#[derive(Clone, Debug)]
pub struct Talk {
    pub with: Party,
    pub text: String,
    pub until: u64,
}

/// A message waiting for an agent's next turn.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct Mail {
    pub from: AgentId,
    pub kind: MsgKind,
    pub body: String,
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
}

impl ProjectRefusal {
    /// The refusal in words, for the command line.
    pub fn describe(&self) -> String {
        match self {
            ProjectRefusal::NotCheckout(why) => why.clone(),
            ProjectRefusal::NameTaken(name) => format!("a project named {name} is open already"),
            ProjectRefusal::PathOpen(name) => format!("that checkout is open already, as {name}"),
            ProjectRefusal::BadName(name) => {
                format!("{name:?} can not name a project: 1-24 of letters, digits, . _ -")
            }
            ProjectRefusal::Bus(why) => format!("the leader could not get a socket: {why}"),
            ProjectRefusal::Last => "the last open project stays open".into(),
        }
    }
}

/// Whether `name` may name a project: 1-24 ASCII letters, digits, `.`,
/// `_` or `-`, so `lead-NAME` is one word the composer can address.
pub fn valid_project_name(name: &str) -> bool {
    (1..=24).contains(&name.chars().count())
        && name
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, '.' | '_' | '-'))
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

pub struct Project {
    pub id: ProjectId,
    pub name: String,
    pub leader: AgentId,
    /// The last change the merge queue landed; `None` without a merge queue
    /// (live agents keep their work in their own worktrees).
    pub trunk: Option<u32>,
    /// The project's files and the symbols agents lease in them.
    pub repo: &'static [(&'static str, &'static [&'static str])],
    /// Closed by the person (`:close`): its agents left and it is not
    /// drawn; kept so ids stay stable, and opened again in place.
    pub closed: bool,
    /// Pinned by the person (`:pin`): the salon shows it above the ones
    /// that are not.
    pub pinned: bool,
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
    /// What a researcher researches, or what a blocked worker waits for.
    pub topic: String,
    /// Tick of the last status change; the screen marks recent changes.
    pub changed_at: u64,
    pub trace: VecDeque<String>,
    /// Trace lines ever written, the kept ones being the last of them; the
    /// store numbers lines by it.
    pub traced: u64,
    /// What a live agent is doing right now (`Edit src/main.rs`), drawn
    /// instead of the progress bar; empty while it thinks.
    pub activity: Option<String>,
    /// The agent's own command line as its session reported it (`/clear`,
    /// `/compact` ...), imece's `/clear` added where the vendor has none;
    /// empty until a session says. The composer completes with it.
    pub commands: Vec<String>,
    /// Where a worker's worktree stands next to its project's checkout, as
    /// git last read it; nothing to show until it is read.
    pub stand: Stand,
    /// The newest loopback URL the agent wrote in its text this session
    /// (`local_url`), drawn on its row as a link the terminal opens.
    pub local: Option<String>,
    /// A researcher that finished; kept so ids stay stable.
    pub gone: bool,
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
            activity: None,
            commands: Vec::new(),
            stand: Stand::default(),
            local: None,
            gone: false,
        }
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
    /// A live agent sent a teammate a message.
    Sent {
        from: AgentId,
        to: AgentId,
        kind: MsgKind,
        text: String,
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
    /// imece was closed while the agent's turn ran (or it died); the turn
    /// is not run again, the agent hears of it when it wakes.
    Interrupted {
        agent: AgentId,
    },
    /// A live agent sent a teammate a message.
    Sent {
        from: AgentId,
        to: AgentId,
        kind: MsgKind,
        text: String,
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
    /// none for the vendor's own default.
    Hired {
        by: AgentId,
        agent: AgentId,
        model: Option<String>,
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
    /// A remembered agent woke in a new session, told where it left off.
    Restarted {
        agent: AgentId,
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
    /// The person closed a project (`:close`); its agents left.
    Closed {
        leader: AgentId,
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
            | Event::Confine { .. }
            | Event::WorktreeMoved { .. }
            | Event::WorktreeKept { .. } => EntryKind::Assumption,
            Event::Blocked { .. } => EntryKind::Blocked,
            Event::ResearchOpened { .. }
            | Event::ResearchDelivered { .. }
            | Event::FromCache { .. } => EntryKind::Research,
            Event::ResearchNeedsYou { .. }
            | Event::Sensitive { .. }
            | Event::ChangeReady { .. }
            | Event::PotLow { .. } => EntryKind::Sensitive,
            Event::Merged { .. } | Event::Applied { .. } => EntryKind::Merged,
            Event::Failed { .. }
            | Event::Stopped { .. }
            | Event::Interrupted { .. }
            | Event::ApplyFailed { .. } => EntryKind::Failed,
            Event::Approved { .. }
            | Event::Rejected { .. }
            | Event::YouSaid { .. }
            | Event::Decided { .. }
            | Event::Budget { .. }
            | Event::Shell { .. }
            | Event::Opened { .. }
            | Event::Closed { .. }
            | Event::Renamed { .. }
            | Event::Forgotten { .. } => EntryKind::Person,
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
            | Event::Budget { leader, .. }
            | Event::PotLow { leader, .. }
            | Event::Shell { leader, .. }
            | Event::Confine { leader, .. }
            | Event::Opened { leader, .. }
            | Event::Closed { leader }
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
            | Event::Interrupted { agent }
            | Event::Posted { agent, .. }
            | Event::Claimed { agent, .. }
            | Event::Hired { agent, .. }
            | Event::Left { agent }
            | Event::Forgotten { agent }
            | Event::Fallback { agent, .. }
            | Event::Resumed { agent }
            | Event::Restarted { agent }
            | Event::NotesCut { agent, .. }
            | Event::WorktreeMoved { agent, .. }
            | Event::WorktreeKept { agent, .. }
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
    /// What the vendors last said of their plans' usage windows, one
    /// entry per vendor and window. Not kept between runs: a session
    /// says it again.
    pub limits: Vec<Limit>,
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

    pub fn agent_by_name(&self, name: &str) -> Option<AgentId> {
        self.agents
            .iter()
            .find(|a| !a.gone && a.name == name)
            .map(|a| a.id)
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

    pub fn spent(&self) -> u64 {
        self.agents.iter().map(|a| a.cost).sum()
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
        a.set_status(Status::Idle, t);
        a.gone = true;
        a.trace(t, "left");
    }

    /// What the leader of project `project`, named `name`, is called:
    /// `lead` in the project imece started in, `lead-NAME` elsewhere.
    pub fn leader_name(project: ProjectId, name: &str) -> String {
        if project == 0 {
            "lead".into()
        } else {
            format!("lead-{name}")
        }
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
    ) -> ProjectId {
        let p = reuse.unwrap_or(self.projects.len());
        let leader = self.agents.len();
        let lead = World::leader_name(p, &name);
        self.agents.push(Agent::new(
            leader,
            p,
            lead,
            vendor,
            Role::Leader,
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
                repo: &[],
                closed: false,
                pinned: false,
            }),
        }
        self.note(Event::Opened { leader, path });
        p
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

    /// Project `p` is called `name` from now on, and its leader `lead-NAME`.
    pub fn rename_project(&mut self, p: ProjectId, name: String) {
        let leader = self.projects[p].leader;
        let from = std::mem::replace(&mut self.projects[p].name, name.clone());
        self.agents[leader].name = format!("lead-{name}");
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

    /// Indexes of the open inbox items, oldest first.
    pub fn open_inbox(&self) -> Vec<usize> {
        self.inbox
            .iter()
            .enumerate()
            .filter(|(_, i)| i.open)
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
        self.cards
            .iter()
            .filter(|c| c.column == Column::Done)
            .count()
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

    /// Whether agent `id` waits for the person's answer to a question.
    pub fn has_question(&self, id: AgentId) -> bool {
        self.inbox
            .iter()
            .any(|i| i.open && i.kind == InboxKind::Question && i.agent == id)
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
    pub fn close_approvals(&mut self, id: AgentId) {
        for i in &mut self.inbox {
            if i.kind == InboxKind::Approval && i.agent == id {
                i.open = false;
            }
        }
    }

    /// Whether agent `id`'s session waits for the person's yes.
    pub fn has_approval(&self, id: AgentId) -> bool {
        self.inbox
            .iter()
            .any(|i| i.open && i.kind == InboxKind::Approval && i.agent == id)
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
        self.note(Event::YouSaid {
            to: id,
            text: text.into(),
        });
        let card = self.agents[id].card;
        self.story(card, StoryEvent::YouSaid(text.into()));
        let a = &mut self.agents[id];
        a.talk = Some(Talk {
            with: Party::Person,
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
                repo: &[],
                closed: false,
                pinned: false,
            }],
            agents,
            cards: Vec::new(),
            journal: Vec::new(),
            inbox: Vec::new(),
            budget: 10_000,
            max_researchers: 0,
            research_queue: Default::default(),
            changes: Vec::new(),
            plans: Vec::new(),
        }
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
        let waits = [Status::Blocked, Status::WaitingHuman, Status::Stopped];
        let rest = [Status::Idle, Status::Asking { to: 1, until: 1 }];
        for s in spends {
            assert!(s.busy() && !s.needs_attention(), "{s:?}");
        }
        for s in waits {
            assert!(!s.busy() && s.needs_attention(), "{s:?}");
        }
        for s in rest {
            assert!(!s.busy() && !s.needs_attention(), "{s:?}");
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
        let level = Event::Confine {
            leader: 3,
            level: "open".into(),
        };
        assert_eq!((level.agent(), level.kind()), (3, EntryKind::Assumption));
    }

    #[test]
    fn opening_a_project_adds_it_with_a_leader_and_reopening_keeps_its_id() {
        let mut w = world(vec![]);
        w.projects.clear();
        w.tick = 9;
        assert_eq!(
            w.open_project(None, "home".into(), "/h".into(), Vendor::Claude),
            0
        );
        assert_eq!(w.agents[0].name, "lead", "the first project's leader");
        assert_eq!(
            w.open_project(None, "api".into(), "/a".into(), Vendor::Codex),
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
            w.open_project(Some(1), "api2".into(), "/a".into(), Vendor::Claude),
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
        w.open_project(None, "api".into(), "/a".into(), Vendor::Claude);
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
        assert_eq!(World::leader_name(0, "x"), "lead");
        assert_eq!(World::leader_name(1, "x"), "lead-x");
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
        assert!(!Status::Asleep.busy() && !Status::Asleep.needs_attention());
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
            repo: &[],
            closed: false,
            pinned: false,
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
}
