//! What the screen shows: projects, agents, cards, the journal, the inbox and
//! the merge queue. Fake events (`sim`) or live agents (`live`) fill it.
//!
//! Events are stored as data, not sentences: the screen words them in the
//! person's language when it draws them.

use std::collections::VecDeque;
use std::sync::atomic::{AtomicU64, Ordering};

pub type AgentId = usize;
pub type CardId = usize;
pub type ProjectId = usize;

/// Seconds per tick.
pub const SECONDS_PER_TICK: u64 = 5;
/// The clock at tick 0, in seconds after midnight: 14:00 for the
/// simulation, the local time at start for live agents.
static START_SECONDS: AtomicU64 = AtomicU64::new(14 * 3600);

pub fn start_clock_at(seconds_after_midnight: u64) {
    START_SECONDS.store(seconds_after_midnight % 86_400, Ordering::Relaxed);
}

fn start_seconds() -> u64 {
    START_SECONDS.load(Ordering::Relaxed)
}
/// Trace lines kept per agent.
const TRACE_KEEP: usize = 200;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Vendor {
    Claude,
    Codex,
    Gemini,
    Qwen,
    Local,
}

impl Vendor {
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

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Role {
    Leader,
    Worker,
    Researcher,
}

/// How much outside content an agent has read: `Soft` after a researcher's
/// report (only sensitive changes need the person), `Full` after reading the
/// outside world itself (every change needs the person).
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
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
    /// The leader breaks a request into cards.
    Planning {
        until: u64,
    },
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
    /// The card's change is in the merge queue.
    Verifying {
        until: u64,
    },
    Researching {
        for_agent: AgentId,
        until: u64,
    },
    /// A live agent's session ended; telling it something starts a new one.
    Stopped,
}

impl Status {
    /// Statuses that spend tokens.
    pub fn busy(self) -> bool {
        matches!(
            self,
            Status::Planning { .. } | Status::Working | Status::Researching { .. }
        )
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
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
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

/// Who an agent is talking with right now, for the salon's `⇄` line.
#[derive(Clone, Debug)]
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

pub struct Project {
    pub id: ProjectId,
    pub name: String,
    pub leader: AgentId,
    pub trunk: u32,
    /// The project's files and the symbols agents lease in them.
    pub repo: &'static [(&'static str, &'static [&'static str])],
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
    /// What a live agent is doing right now (`Edit src/main.rs`), drawn
    /// instead of the progress bar; empty while it thinks.
    pub activity: Option<String>,
    /// A researcher that finished; kept so ids stay stable.
    pub gone: bool,
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
            progress: 0,
            cost: 0,
            taint: Taint::Clean,
            talk: None,
            researcher: None,
            topic: String::new(),
            changed_at: tick,
            trace: VecDeque::new(),
            activity: None,
            gone: false,
        }
    }

    pub fn set_status(&mut self, status: Status, tick: u64) {
        if self.status != status {
            self.status = status;
            self.changed_at = tick;
        }
    }

    pub fn trace(&mut self, tick: u64, line: impl Into<String>) {
        if self.trace.len() == TRACE_KEEP {
            self.trace.pop_front();
        }
        self.trace
            .push_back(format!("{} {}", clock_seconds(tick), line.into()));
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
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
#[derive(Clone, Debug)]
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
    /// A live agent changed this file.
    Touched(String),
    /// A live agent's turn or session ended without an answer.
    Stopped(String),
    /// A live agent sent a teammate a message.
    Sent {
        from: AgentId,
        to: AgentId,
        kind: MsgKind,
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
            StoryEvent::Took { .. } | StoryEvent::WritingTests | StoryEvent::BlockedOn(_) => {
                StoryKind::Intent
            }
            StoryEvent::Decision(_) => StoryKind::Decision,
            StoryEvent::Asked { .. }
            | StoryEvent::Answered { .. }
            | StoryEvent::Replied(_)
            | StoryEvent::Sent { .. } => StoryKind::Talk,
            StoryEvent::Assumption(_) => StoryKind::Assumption,
            StoryEvent::ResearchArrived { .. }
            | StoryEvent::ResearchNeedsYou { .. }
            | StoryEvent::FromCache(_)
            | StoryEvent::Verified { .. }
            | StoryEvent::QueueFailed
            | StoryEvent::Stopped(_) => StoryKind::Evidence,
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

impl Card {
    /// Leases count while the change is not merged.
    pub fn holds_leases(&self) -> bool {
        matches!(self.column, Column::Working | Column::Verifying)
    }
}

/// An event of the whole guild, for the journal.
#[derive(Clone, Debug)]
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
    /// A live agent sent a teammate a message.
    Sent {
        from: AgentId,
        to: AgentId,
        kind: MsgKind,
        text: String,
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
            Event::Plan { .. } => EntryKind::Plan,
            Event::Assign { .. } => EntryKind::Assign,
            Event::Talk { .. } | Event::Replied { .. } | Event::Sent { .. } => EntryKind::Talk,
            Event::Assumption { .. } => EntryKind::Assumption,
            Event::Blocked { .. } => EntryKind::Blocked,
            Event::ResearchOpened { .. }
            | Event::ResearchDelivered { .. }
            | Event::FromCache { .. } => EntryKind::Research,
            Event::ResearchNeedsYou { .. } | Event::Sensitive { .. } => EntryKind::Sensitive,
            Event::Merged { .. } => EntryKind::Merged,
            Event::Failed { .. } | Event::Stopped { .. } => EntryKind::Failed,
            Event::Approved { .. } | Event::Rejected { .. } | Event::YouSaid { .. } => {
                EntryKind::Person
            }
        }
    }

    /// The agent the event is about, for its project.
    pub fn agent(&self) -> AgentId {
        match *self {
            Event::Plan { leader, .. } => leader,
            Event::Assign { agent, .. }
            | Event::Assumption { agent, .. }
            | Event::Blocked { agent, .. }
            | Event::FromCache { agent, .. }
            | Event::Sensitive { agent, .. }
            | Event::Merged { agent, .. }
            | Event::Failed { agent }
            | Event::Approved { agent, .. }
            | Event::Rejected { agent }
            | Event::Replied { agent, .. }
            | Event::Stopped { agent, .. } => agent,
            Event::Talk { from, .. } | Event::Sent { from, .. } => from,
            Event::ResearchOpened { researcher, .. }
            | Event::ResearchDelivered { researcher, .. }
            | Event::ResearchNeedsYou { researcher, .. } => researcher,
            Event::YouSaid { to, .. } => to,
        }
    }
}

#[derive(Clone, Debug)]
pub struct Entry {
    pub tick: u64,
    pub event: Event,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum InboxKind {
    /// A change to dependencies, CI, build scripts or network settings.
    Sensitive,
    /// A research report that proposes something sensitive.
    Research,
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
}

/// A request from the person that a leader is breaking into cards.
#[derive(Clone, Debug)]
pub struct Plan {
    pub project: ProjectId,
    pub text: String,
    pub ready: u64,
}

pub struct World {
    pub tick: u64,
    pub projects: Vec<Project>,
    pub agents: Vec<Agent>,
    pub cards: Vec<Card>,
    pub journal: Vec<Entry>,
    pub inbox: Vec<InboxItem>,
    /// Ten-thousandths of a dollar.
    pub daily_budget: u64,
    /// Researchers that may run at once, across all projects.
    pub max_researchers: usize,
    /// Blocked workers waiting for a free researcher slot.
    pub research_queue: VecDeque<AgentId>,
    pub plans: Vec<Plan>,
}

impl World {
    pub fn agent_by_name(&self, name: &str) -> Option<AgentId> {
        self.agents
            .iter()
            .find(|a| !a.gone && a.name == name)
            .map(|a| a.id)
    }

    pub fn spent(&self) -> u64 {
        self.agents.iter().map(|a| a.cost).sum()
    }

    pub fn rate_per_hour(&self) -> u64 {
        if self.tick == 0 {
            0
        } else {
            self.spent() * 3600 / (self.tick * SECONDS_PER_TICK)
        }
    }

    pub fn active_agents(&self) -> usize {
        self.agents
            .iter()
            .filter(|a| !a.gone && a.status != Status::Idle)
            .count()
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

fn clock_seconds(tick: u64) -> String {
    let s = start_seconds() + tick * SECONDS_PER_TICK;
    format!("{:02}:{:02}:{:02}", s / 3600 % 24, s / 60 % 60, s % 60)
}

/// Ten-thousandths of a dollar as `$D.CC`.
pub fn dollars(cost: u64) -> String {
    format!("${}.{:02}", cost / 10_000, cost / 100 % 100)
}
