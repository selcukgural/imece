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

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
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

pub struct Project {
    pub id: ProjectId,
    pub name: String,
    pub leader: AgentId,
    /// The last change the merge queue landed; `None` without a merge queue
    /// (live agents keep their work in their own worktrees).
    pub trunk: Option<u32>,
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
            parent: None,
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
    /// The person changed the budget (`:budget`), in ten-thousandths of a
    /// dollar; `leader` is the one of the project they were looking at.
    Budget {
        leader: AgentId,
        from: u64,
        to: u64,
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
            Event::Talk { .. } | Event::Replied { .. } | Event::Sent { .. } => EntryKind::Talk,
            Event::Assumption { .. }
            | Event::Fallback { .. }
            | Event::WorktreeMoved { .. }
            | Event::WorktreeKept { .. } => EntryKind::Assumption,
            Event::Blocked { .. } => EntryKind::Blocked,
            Event::ResearchOpened { .. }
            | Event::ResearchDelivered { .. }
            | Event::FromCache { .. } => EntryKind::Research,
            Event::ResearchNeedsYou { .. } | Event::Sensitive { .. } => EntryKind::Sensitive,
            Event::Merged { .. } => EntryKind::Merged,
            Event::Failed { .. } | Event::Stopped { .. } => EntryKind::Failed,
            Event::Approved { .. }
            | Event::Rejected { .. }
            | Event::YouSaid { .. }
            | Event::Budget { .. } => EntryKind::Person,
        }
    }

    /// The agent the event is about, for its project.
    pub fn agent(&self) -> AgentId {
        match *self {
            Event::Plan { leader, .. } | Event::Budget { leader, .. } => leader,
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
            | Event::Stopped { agent, .. }
            | Event::Posted { agent, .. }
            | Event::Claimed { agent, .. }
            | Event::Hired { agent, .. }
            | Event::Left { agent }
            | Event::Fallback { agent, .. }
            | Event::WorktreeMoved { agent, .. }
            | Event::WorktreeKept { agent, .. } => agent,
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
    /// An agent answered the person and waits for them; the detail is the
    /// answer's first line.
    Reply,
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
        });
    }

    /// The person read or answered agent `id`'s answers.
    pub fn close_replies(&mut self, id: AgentId) {
        for i in &mut self.inbox {
            if i.kind == InboxKind::Reply && i.agent == id {
                i.open = false;
            }
        }
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
            tick: 0,
            projects: vec![Project {
                id: 0,
                name: "p".into(),
                leader: 0,
                trunk: None,
                repo: &[],
            }],
            agents,
            cards: Vec::new(),
            journal: Vec::new(),
            inbox: Vec::new(),
            daily_budget: 10_000,
            max_researchers: 0,
            research_queue: Default::default(),
            plans: Vec::new(),
        }
    }

    fn item(kind: InboxKind, agent: AgentId, open: bool) -> InboxItem {
        InboxItem {
            kind,
            agent,
            detail: String::new(),
            researcher: None,
            open,
            tick: 0,
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
            Status::Planning { until: 1 },
            Status::Working,
            Status::Researching {
                for_agent: 1,
                until: 1,
            },
        ];
        let waits = [Status::Blocked, Status::WaitingHuman, Status::Stopped];
        let rest = [
            Status::Idle,
            Status::Asking { to: 1, until: 1 },
            Status::Verifying { until: 1 },
        ];
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
        // Two working workers and two researchers: not the two idle ones,
        // not the two that are gone.
        assert_eq!(w.active_agents(), 4);
        assert_eq!(w.active_researchers(), 2);
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
    fn the_rate_is_what_was_spent_per_hour() {
        let mut w = world(vec![
            agent(0, Role::Leader, Status::Idle),
            agent(1, Role::Worker, Status::Working),
        ]);
        w.agents[0].cost = 100;
        w.agents[1].cost = 100;
        assert_eq!(w.rate_per_hour(), 0, "no time, no rate");
        w.tick = 3600 / SECONDS_PER_TICK;
        assert_eq!(w.rate_per_hour(), 200);
        w.tick = 1800 / SECONDS_PER_TICK;
        assert_eq!(w.rate_per_hour(), 400);
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

    #[test]
    fn the_clock_reads_from_the_start_of_the_day() {
        // One test for every clock reading: the start is shared state.
        assert_eq!(clock(0), "14:00");
        assert_eq!(clock(13), "14:01");
        assert_eq!(clock_seconds(13), "14:01:05");
        start_clock_at(86_400 + 60);
        assert_eq!(clock(0), "00:01", "wraps at midnight");
        assert_eq!(clock_seconds(1), "00:01:05");
        start_clock_at(14 * 3600);
        assert_eq!(clock(0), "14:00");
    }
}
