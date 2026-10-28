//! Live agents: real Claude Code sessions feed the model the simulation
//! feeds. One project (the checkout imece runs in), a leader that reads it,
//! and workers that each change their own git worktree of it.
//!
//! Money is reserved up front: every agent gets an equal share of the
//! budget as its session's hard cap (`--max-budget-usd`), so all of them
//! together can never spend more than the budget. An agent that spent its
//! share is not prompted again.
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

use std::collections::HashMap;
use std::io;
use std::path::{Path, PathBuf};
use std::sync::mpsc::{self, Receiver, Sender};
use std::thread;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use imece_agent::claude::{self, Mcp, Session, Spec};
use imece_agent::mcp::{Answer, Server, Tool};
use imece_agent::{Event as AgentEvent, child, worktree};
use serde_json::{Value, json};

use crate::model::*;

pub struct Options {
    pub repo: PathBuf,
    pub workers: usize,
    pub model: Option<String>,
    /// Ten-thousandths of a dollar, for all agents together.
    pub budget: u64,
}

/// Why a message could not reach an agent.
#[derive(Debug, PartialEq)]
pub enum Refusal {
    Budget,
    Start(String),
}

/// An agent's session, if it has one.
enum Slot {
    None,
    /// Being started on its own thread (a first worktree checks out the
    /// whole tree); the person's messages wait here.
    Starting(Vec<String>),
    Running(Session),
}

/// A started session, or why it could not start.
type Ready = Result<(Session, PathBuf), String>;

/// A tool call of an agent (tool name, arguments), and where its answer
/// goes.
type Call = (AgentId, String, Value, Sender<Answer>);

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

/// A message waiting for an agent's next turn.
#[derive(Clone, Debug)]
struct Mail {
    from: AgentId,
    kind: MsgKind,
    body: String,
}

/// A card an agent put on the board.
#[derive(Clone, Debug)]
struct Post {
    by: AgentId,
    body: String,
}

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

pub struct Live {
    started: Instant,
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
    program: String,
    repo: PathBuf,
    /// Where the workers' worktrees live, outside the checkout.
    root: PathBuf,
    model: Option<String>,
    /// Each agent's share of the budget, in ten-thousandths of a dollar.
    share: u64,
    bus: Option<Bus>,
    calls_tx: Sender<Call>,
    calls_rx: Receiver<Call>,
    /// Messages waiting for each agent's next turn.
    mail: Vec<Vec<Mail>>,
    /// Prompts each session was given and has not finished.
    turns: Vec<usize>,
    /// Turns each agent took on messages since the person last spoke.
    woken: Vec<u32>,
    /// The board's cards, by the card they made.
    posts: HashMap<CardId, Post>,
    /// Workers each open card was offered to, in order.
    offered: HashMap<CardId, Vec<AgentId>>,
    /// Cards each agent posted since the person last spoke.
    posted: Vec<u32>,
}

/// Checks that `claude` runs, then builds the team for the checkout `repo`
/// is in.
pub fn world(o: Options) -> Result<(World, Live), String> {
    let repo = worktree::top(&o.repo).map_err(|e| format!("{}: {e}", o.repo.display()))?;
    let program = "claude".to_string();
    let version = child::command(&program, &repo, claude::ENV)
        .arg("--version")
        .output()
        .map_err(|e| format!("{program}: {e} (is Claude Code installed and on PATH?)"))?;
    if !version.status.success() {
        return Err(format!("{program} --version failed"));
    }
    let name = repo
        .file_name()
        .map_or("repo".into(), |n| n.to_string_lossy().into_owned());
    let root = state_dir().join("worktrees").join(format!(
        "{name}-{:08x}",
        fnv(repo.as_os_str().as_encoded_bytes())
    ));

    let mut w = World {
        tick: 0,
        projects: vec![Project {
            id: 0,
            name,
            leader: 0,
            trunk: 0,
            repo: &[],
        }],
        agents: vec![Agent::new(
            0,
            0,
            "lead".into(),
            Vendor::Claude,
            Role::Leader,
            Status::Idle,
            0,
        )],
        cards: Vec::new(),
        journal: Vec::new(),
        inbox: Vec::new(),
        daily_budget: o.budget,
        max_researchers: 0,
        research_queue: Default::default(),
        plans: Vec::new(),
    };
    for i in 1..=o.workers {
        w.agents.push(Agent::new(
            i,
            0,
            format!("claude-{i}"),
            Vendor::Claude,
            Role::Worker,
            Status::Idle,
            0,
        ));
    }
    let n = w.agents.len();
    start_clock_at(local_seconds_after_midnight());
    let mut live = Live::new(n, program, repo, root, o.model, o.budget / n as u64);
    let relay = std::env::current_exe().map_err(|e| format!("imece's own path: {e}"))?;
    live.open_bus(&w, relay.to_string_lossy().into_owned())
        .map_err(|e| format!("the agents' message bus: {e}"))?;
    Ok((w, live))
}

impl Live {
    fn new(
        agents: usize,
        program: String,
        repo: PathBuf,
        root: PathBuf,
        model: Option<String>,
        share: u64,
    ) -> Live {
        let (tx, rx) = mpsc::channel();
        let (ready_tx, ready_rx) = mpsc::channel();
        let (calls_tx, calls_rx) = mpsc::channel();
        Live {
            started: Instant::now(),
            tx,
            rx,
            ready_tx,
            ready_rx,
            slots: (0..agents).map(|_| Slot::None).collect(),
            generation: vec![0; agents],
            spent_before: vec![0; agents],
            program,
            repo,
            root,
            model,
            share,
            bus: None,
            calls_tx,
            calls_rx,
            mail: vec![Vec::new(); agents],
            turns: vec![0; agents],
            woken: vec![0; agents],
            posts: HashMap::new(),
            offered: HashMap::new(),
            posted: vec![0; agents],
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
        let mut bus = Bus {
            dir,
            relay,
            servers: Vec::new(),
        };
        for id in 0..self.slots.len() {
            let tx = self.calls_tx.clone();
            let tools = tools_of(w.agents[id].role);
            let server = Server::bind(&bus.dir.join(format!("a{id}.sock")), tools, {
                move |name, args| {
                    let (reply, answer) = mpsc::channel();
                    tx.send((id, name.to_string(), args.clone(), reply))
                        .map_err(|_| "imece is closing".to_string())?;
                    answer
                        .recv_timeout(CALL_WAIT)
                        .map_err(|_| "imece did not answer in time; try again".to_string())?
                }
            })?;
            bus.servers.push(server);
        }
        self.bus = Some(bus);
        Ok(())
    }

    /// Moves the clock and applies what the sessions reported.
    pub fn step(&mut self, w: &mut World) {
        w.tick = self.started.elapsed().as_secs() / SECONDS_PER_TICK;
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
        while let Ok((id, generation, e)) = self.rx.try_recv() {
            if generation != self.generation[id] {
                continue;
            }
            let turn_done = matches!(e, AgentEvent::TurnDone { .. });
            if let AgentEvent::Stopped(_) = e {
                self.end(w, id);
            }
            apply(w, id, e, self.spent_before[id]);
            if turn_done {
                self.turns[id] = self.turns[id].saturating_sub(1);
                if self.turns[id] > 0 {
                    // The session goes on with a prompt it already has.
                    w.agents[id].set_status(Status::Working, w.tick);
                }
            }
        }
        while let Ok((from, tool, args, reply)) = self.calls_rx.try_recv() {
            let answer = match tool.as_str() {
                "send" => self.send(w, from, &args),
                "post" => self.post(w, from, &args),
                "board" => Ok(self.board(w, from)),
                "claim" => self.claim(w, from, &args),
                _ => Err(format!("there is no tool {tool:?}")),
            };
            let _ = reply.send(answer);
        }
        for id in 0..self.mail.len() {
            let waking = self.mail[id].iter().any(|m| m.kind.wakes());
            if waking && self.turns[id] == 0 && w.agents[id].status != Status::Stopped {
                self.deliver(w, id);
            }
        }
        self.offer(w);
    }

    /// A session ended: the next message starts a new one.
    fn end(&mut self, w: &World, id: AgentId) {
        self.spent_before[id] = w.agents[id].cost;
        self.slots[id] = Slot::None;
        self.generation[id] += 1;
        self.turns[id] = 0;
    }

    /// Agent `from` sends a message (the `send` tool's arguments). The
    /// answer tells the sender what became of it.
    fn send(&mut self, w: &mut World, from: AgentId, args: &Value) -> Answer {
        let arg = |k: &str| args[k].as_str().map(str::trim).unwrap_or_default();
        let kind = MsgKind::from_wire(arg("type")).ok_or_else(|| {
            let kinds: Vec<&str> = MsgKind::ALL.iter().map(|k| k.wire()).collect();
            format!("type must be one of {}", kinds.join(", "))
        })?;
        let body = arg("body");
        if body.is_empty() {
            return Err("the body is empty".into());
        }
        if body.chars().count() > MAX_BODY {
            return Err(format!("the body is longer than {MAX_BODY} characters"));
        }
        let project = w.agents[from].project;
        let to = w
            .agent_by_name(arg("to"))
            .filter(|&t| t != from && w.agents[t].project == project)
            .ok_or_else(|| {
                let team: Vec<&str> = w
                    .agents
                    .iter()
                    .filter(|a| a.id != from && a.project == project)
                    .map(|a| a.name.as_str())
                    .collect();
                format!(
                    "no teammate is called {:?}; the team is {}",
                    arg("to"),
                    team.join(", ")
                )
            })?;
        let name = w.agents[to].name.clone();
        if w.agents[to].status == Status::Stopped {
            return Err(format!(
                "{name} has stopped; it runs again when the person talks to it"
            ));
        }
        if kind.wakes() && self.over_budget(w, to) {
            return Err(format!("{name} has spent its share of the budget"));
        }
        if kind.wakes() && self.woken[to] >= MAX_WOKEN {
            return Err(format!(
                "{name} took {MAX_WOKEN} turns on messages since the person last spoke; \
                 stop and tell the person what you need"
            ));
        }
        if self.mail[to].len() >= MAX_MAIL {
            return Err(format!(
                "{name} has {MAX_MAIL} unread messages; wait for it"
            ));
        }
        self.mail[to].push(Mail {
            from,
            kind,
            body: body.to_string(),
        });

        let t = w.tick;
        let first = body.lines().next().unwrap_or_default().to_string();
        let story = StoryEvent::Sent {
            from,
            to,
            kind,
            text: body.to_string(),
        };
        let (mine, theirs) = (w.agents[from].card, w.agents[to].card);
        w.story(mine, story.clone());
        if theirs != mine {
            w.story(theirs, story);
        }
        w.note(Event::Sent {
            from,
            to,
            kind,
            text: first.clone(),
        });
        let sender = w.agents[from].name.clone();
        w.agents[from].trace(t, format!("send to={name} type={}", kind.wire()));
        w.agents[to].trace(t, format!("mail from={sender} type={}", kind.wire()));
        for (a, other) in [(from, to), (to, from)] {
            w.agents[a].talk = Some(Talk {
                with: Party::Agent(other),
                text: first.clone(),
                until: t + 8,
            });
        }
        Ok(match (kind.wakes(), self.turns[to] > 0) {
            (true, false) => format!("delivered; {name} starts on it now"),
            (true, true) => format!("queued; {name} is busy and gets it when its turn ends"),
            (false, _) => format!(
                "queued for {name}'s next turn ({} does not wake it)",
                kind.wire()
            ),
        })
    }

    /// Agent `from` puts a card on the board (the `post` tool's arguments).
    fn post(&mut self, w: &mut World, from: AgentId, args: &Value) -> Answer {
        let arg = |k: &str| args[k].as_str().map(str::trim).unwrap_or_default();
        let (title, body) = (arg("title"), arg("body"));
        if title.is_empty() || body.is_empty() {
            return Err("a card needs a title and a body".into());
        }
        if title.contains('\n') || title.chars().count() > MAX_TITLE {
            return Err(format!(
                "the title must be one line of at most {MAX_TITLE} characters"
            ));
        }
        if body.chars().count() > MAX_BODY {
            return Err(format!("the body is longer than {MAX_BODY} characters"));
        }
        let project = w.agents[from].project;
        if self.open(w, project).count() >= MAX_OPEN {
            return Err(format!(
                "the board has {MAX_OPEN} open cards; wait until workers take some"
            ));
        }
        if self.posted[from] >= MAX_POSTS {
            return Err(format!(
                "you posted {MAX_POSTS} cards since the person last spoke; \
                 stop and tell the person what is left"
            ));
        }
        self.posted[from] += 1;
        let (t, card) = (w.tick, w.cards.len());
        w.cards.push(Card {
            id: card,
            project,
            title: title.to_string(),
            owner: None,
            column: Column::Waiting,
            story: Vec::new(),
            symbols: Vec::new(),
            started: t,
            finished: 0,
        });
        self.posts.insert(
            card,
            Post {
                by: from,
                body: body.to_string(),
            },
        );
        w.story(
            Some(card),
            StoryEvent::Posted {
                by: from,
                text: body.to_string(),
            },
        );
        w.note(Event::Posted { agent: from, card });
        w.agents[from].trace(t, format!("post #{} {title:?}", card + 1));
        Ok(format!(
            "posted as #{}; an idle worker will be offered it",
            card + 1
        ))
    }

    /// The board of agent `from`'s project, for the `board` tool.
    fn board(&self, w: &World, from: AgentId) -> String {
        let project = w.agents[from].project;
        let mut open = Vec::new();
        let mut taken = Vec::new();
        for c in w.cards.iter().filter(|c| c.project == project) {
            let Some(post) = self.posts.get(&c.id) else {
                continue;
            };
            let by = &w.agents[post.by].name;
            match (c.column, c.owner) {
                (Column::Waiting, _) => {
                    open.push(format!("#{} {} (posted by {by})", c.id + 1, c.title))
                }
                (Column::Done, Some(o)) => taken.push(format!(
                    "#{} {} (done by {})",
                    c.id + 1,
                    c.title,
                    w.agents[o].name
                )),
                (_, Some(o)) => taken.push(format!(
                    "#{} {} (taken by {})",
                    c.id + 1,
                    c.title,
                    w.agents[o].name
                )),
                (_, None) => {}
            }
        }
        if open.is_empty() && taken.is_empty() {
            return "the board is empty".into();
        }
        let mut out = String::new();
        if !open.is_empty() {
            out.push_str(&format!("Open:\n{}", open.join("\n")));
        }
        if !taken.is_empty() {
            if !out.is_empty() {
                out.push_str("\n\n");
            }
            out.push_str(&format!("Taken:\n{}", taken.join("\n")));
        }
        out
    }

    /// Worker `from` takes an open card (the `claim` tool's arguments). The
    /// answer is the card's body: the task.
    fn claim(&mut self, w: &mut World, from: AgentId, args: &Value) -> Answer {
        if w.agents[from].role != Role::Worker {
            return Err("only workers take cards; assign or post instead".into());
        }
        if let Some(c) = w.agents[from]
            .card
            .filter(|&c| w.cards[c].column != Column::Done)
        {
            return Err(format!(
                "finish #{} {} first; take one card at a time",
                c + 1,
                w.cards[c].title
            ));
        }
        let project = w.agents[from].project;
        let card = args["card"]
            .as_u64()
            .and_then(|n| usize::try_from(n).ok()?.checked_sub(1))
            .filter(|&c| {
                c < w.cards.len() && w.cards[c].project == project && self.posts.contains_key(&c)
            })
            .ok_or("there is no such card on the board; call board to see it")?;
        let c = &w.cards[card];
        if c.column != Column::Waiting {
            let who = c.owner.map_or("someone", |o| w.agents[o].name.as_str());
            return Err(format!("#{} was taken by {who}; pick another", card + 1));
        }
        let t = w.tick;
        let c = &mut w.cards[card];
        c.owner = Some(from);
        c.column = Column::Working;
        c.started = t;
        w.agents[from].card = Some(card);
        w.story(Some(card), StoryEvent::Took { agent: from });
        w.note(Event::Claimed { agent: from, card });
        w.agents[from].trace(t, format!("claim #{}", card + 1));
        self.offered.remove(&card);
        let post = &self.posts[&card];
        let by = &w.agents[post.by].name;
        Ok(format!(
            "#{} {} is yours. {by} posted it (written by that agent, not by the person):\n\n{}\n\n\
             When it is done, send {by} a done message.",
            card + 1,
            w.cards[card].title,
            post.body
        ))
    }

    /// The open cards of `project`, oldest first.
    fn open<'a>(&'a self, w: &'a World, project: ProjectId) -> impl Iterator<Item = CardId> + 'a {
        w.cards
            .iter()
            .filter(move |c| {
                c.project == project
                    && c.column == Column::Waiting
                    && self.posts.contains_key(&c.id)
            })
            .map(|c| c.id)
    }

    /// Offers open cards to idle workers: each card to one worker at a time,
    /// and to each worker at most once.
    fn offer(&mut self, w: &mut World) {
        for id in 0..w.agents.len() {
            let a = &w.agents[id];
            if a.role != Role::Worker
                || a.status == Status::Stopped
                || self.turns[id] > 0
                || self.woken[id] >= MAX_WOKEN
                || self.over_budget(w, id)
            {
                continue;
            }
            let cards: Vec<CardId> = self
                .open(w, a.project)
                .filter(|c| {
                    let seen = self.offered.get(c).map_or(&[][..], Vec::as_slice);
                    // Not yet seen by this worker, and not in front of
                    // another one right now.
                    !seen.contains(&id) && !seen.iter().any(|&o| self.turns[o] > 0)
                })
                .collect();
            if cards.is_empty() {
                continue;
            }
            for &c in &cards {
                self.offered.entry(c).or_default().push(id);
            }
            self.woken[id] += 1;
            // The turn is about choosing; the card it takes is its card.
            w.agents[id].card = None;
            let listed: Vec<(CardId, AgentId)> =
                cards.iter().map(|c| (*c, self.posts[c].by)).collect();
            if let Err(why) = self.prompt(w, id, board_prompt(w, &listed), None) {
                apply(w, id, AgentEvent::Stopped(why), self.spent_before[id]);
            }
        }
    }

    /// Starts agent `id`'s next turn with all its mail.
    fn deliver(&mut self, w: &mut World, id: AgentId) {
        if self.over_budget(w, id) {
            return;
        }
        let mail = std::mem::take(&mut self.mail[id]);
        self.woken[id] += 1;
        let title = mail
            .iter()
            .find(|m| m.kind == MsgKind::Assign)
            .unwrap_or(&mail[0])
            .body
            .lines()
            .next()
            .unwrap_or_default()
            .to_string();
        if let Err(why) = self.prompt(w, id, mail_prompt(w, &mail), Some(&title)) {
            apply(w, id, AgentEvent::Stopped(why), self.spent_before[id]);
        }
    }

    fn over_budget(&self, w: &World, id: AgentId) -> bool {
        w.agents[id].cost >= self.share || w.spent() >= w.daily_budget
    }

    fn ready(&mut self, w: &mut World, id: AgentId, ready: Ready) {
        let waiting = match std::mem::replace(&mut self.slots[id], Slot::None) {
            Slot::Starting(q) => q,
            _ => Vec::new(),
        };
        let mut session = match ready {
            Ok((session, cwd)) => {
                w.agents[id].trace(w.tick, format!("session in {}", cwd.display()));
                session
            }
            Err(why) => {
                self.end(w, id);
                return apply(w, id, AgentEvent::Stopped(why), self.spent_before[id]);
            }
        };
        for text in waiting {
            if let Err(e) = session.prompt(&text) {
                self.end(w, id);
                return apply(
                    w,
                    id,
                    AgentEvent::Stopped(e.to_string()),
                    self.spent_before[id],
                );
            }
        }
        self.slots[id] = Slot::Running(session);
    }

    /// Gives agent `id` the person's message, starting its session first
    /// if it has none. An idle worker gets a new card for it.
    pub fn told(&mut self, w: &mut World, id: AgentId, text: &str) -> Result<(), Refusal> {
        if self.over_budget(w, id) {
            return Err(Refusal::Budget);
        }
        // The person is here again: agents may wake each other anew.
        self.woken.iter_mut().for_each(|n| *n = 0);
        self.posted.iter_mut().for_each(|n| *n = 0);
        let title = text.lines().next().unwrap_or_default().to_string();
        self.prompt(w, id, text.to_string(), Some(&title))
            .map_err(Refusal::Start)
    }

    /// Gives agent `id` a prompt, starting its session first if it has
    /// none. An idle worker gets a new card titled `title`, if there is one.
    fn prompt(
        &mut self,
        w: &mut World,
        id: AgentId,
        text: String,
        title: Option<&str>,
    ) -> Result<(), String> {
        let t = w.tick;
        match &mut self.slots[id] {
            Slot::None => {
                self.start(w, id);
                self.slots[id] = Slot::Starting(vec![text]);
            }
            Slot::Starting(q) => q.push(text),
            Slot::Running(s) => {
                if let Err(e) = s.prompt(&text) {
                    self.end(w, id);
                    return Err(e.to_string());
                }
            }
        }
        self.turns[id] += 1;
        let a = &w.agents[id];
        if let Some(title) = title
            && a.role == Role::Worker
            && a.status != Status::Working
        {
            let card = w.cards.len();
            w.cards.push(Card {
                id: card,
                project: a.project,
                title: title.to_string(),
                owner: Some(id),
                column: Column::Working,
                story: Vec::new(),
                symbols: Vec::new(),
                started: t,
                finished: 0,
            });
            w.agents[id].card = Some(card);
            w.story(Some(card), StoryEvent::Took { agent: id });
        }
        let a = &mut w.agents[id];
        a.set_status(Status::Working, t);
        if a.activity.is_none() {
            a.activity = Some(String::new());
        }
        Ok(())
    }

    /// Starts agent `id`'s session on its own thread; `step` picks it up.
    fn start(&self, w: &World, id: AgentId) {
        let a = &w.agents[id];
        let leader = a.role == Role::Leader;
        let tools = if leader {
            claude::READ_TOOLS
        } else {
            claude::EDIT_TOOLS
        };
        let mut spec = Spec {
            program: self.program.clone(),
            cwd: self.repo.clone(),
            name: format!("imece {}", a.name),
            tools: tools.iter().map(|t| t.to_string()).collect(),
            model: self.model.clone(),
            // In millionths of a dollar: what is left of the agent's share.
            budget: self.share.saturating_sub(a.cost) * 100,
            system: system_prompt(w, id),
            mcp: self.bus.as_ref().map(|b| Mcp {
                command: b.relay.clone(),
                args: vec![
                    "mcp-relay".into(),
                    b.servers[id].path().to_string_lossy().into_owned(),
                ],
                tools: tools_of(a.role).into_iter().map(|t| t.name).collect(),
            }),
        };
        let generation = self.generation[id];
        let (repo, root, name) = (self.repo.clone(), self.root.clone(), a.name.clone());
        let (tx, ready) = (self.tx.clone(), self.ready_tx.clone());
        thread::spawn(move || {
            let started = (|| {
                if !leader {
                    spec.cwd = worktree::ensure(&repo, &root, &name).map_err(|e| e.to_string())?;
                }
                let session = Session::start(&spec, move |e| {
                    let _ = tx.send((id, generation, e));
                })
                .map_err(|e| format!("{}: {e}", spec.program))?;
                Ok((session, spec.cwd.clone()))
            })();
            // Nobody to take it (imece is closing): dropping it stops it.
            let _ = ready.send((id, generation, started));
        });
    }
}

impl Drop for Live {
    fn drop(&mut self) {
        // Ask every session to stop first, so they wind down together.
        for s in &mut self.slots {
            if let Slot::Running(s) = s {
                s.terminate();
            }
        }
    }
}

/// Applies one event of agent `id`'s session. `before` is what its earlier
/// sessions cost, in ten-thousandths of a dollar.
pub fn apply(w: &mut World, id: AgentId, e: AgentEvent, before: u64) {
    let t = w.tick;
    let card = w.agents[id].card;
    match e {
        AgentEvent::Started { session, model } => {
            w.agents[id].trace(t, format!("session {session} · {model}"));
        }
        AgentEvent::Text(text) => w.agents[id].trace(t, format!("text {text:?}")),
        AgentEvent::Tool { name, target } => {
            let mcp = claude::mcp_tool("");
            let name = name.strip_prefix(&mcp).map_or(name.clone(), str::to_string);
            let changes = matches!(name.as_str(), "Edit" | "Write");
            let a = &mut w.agents[id];
            a.trace(t, format!("tool_use {name} {target}"));
            a.activity = Some(format!("{name} {target}").trim_end().to_string());
            let seen = card.is_some_and(|c| {
                w.cards[c]
                    .story
                    .iter()
                    .any(|s| matches!(&s.event, StoryEvent::Touched(f) if *f == target))
            });
            if changes && !seen {
                w.story(card, StoryEvent::Touched(target));
            }
        }
        AgentEvent::ToolFailed { name } => w.agents[id].trace(t, format!("tool_result {name} ✕")),
        AgentEvent::TurnDone {
            ok,
            text,
            cost,
            denied,
        } => {
            let a = &mut w.agents[id];
            a.cost = before + cost / 100;
            a.activity = None;
            a.trace(
                t,
                format!(
                    "result {} {} denied {denied}",
                    if ok { "ok" } else { "✕" },
                    dollars(a.cost)
                ),
            );
            a.set_status(Status::Idle, t);
            if ok {
                a.talk = Some(Talk {
                    with: Party::Person,
                    text: text.lines().next().unwrap_or_default().to_string(),
                    until: t + 12,
                });
                w.story(card, StoryEvent::Replied(text.clone()));
                w.note(Event::Replied { agent: id, text });
                if let Some(c) = card {
                    w.cards[c].column = Column::Done;
                    w.cards[c].finished = t;
                }
            } else {
                w.story(card, StoryEvent::Stopped(text.clone()));
                w.note(Event::Stopped {
                    agent: id,
                    why: text,
                });
            }
        }
        AgentEvent::Stopped(why) => {
            let a = &mut w.agents[id];
            a.trace(t, format!("stopped {why:?}"));
            a.activity = None;
            a.set_status(Status::Stopped, t);
            w.story(card, StoryEvent::Stopped(why.clone()));
            w.note(Event::Stopped { agent: id, why });
        }
    }
}

/// imece's tools for an agent of `role`: workers alone take cards.
fn tools_of(role: Role) -> Vec<Tool> {
    let mut tools = vec![send_tool(), post_tool(), board_tool()];
    if role == Role::Worker {
        tools.push(claim_tool());
    }
    tools
}

fn post_tool() -> Tool {
    Tool {
        name: "post".into(),
        description: "Puts a card on imece's board for any worker to take. Use it for \
                      work nobody has yet; for work one teammate should do, send assign."
            .into(),
        input: json!({
            "type": "object",
            "properties": {
                "title": { "type": "string", "description": "One line.", "maxLength": MAX_TITLE },
                "body": {
                    "type": "string",
                    "description": "The task: what to do, which files, when it is done.",
                    "maxLength": MAX_BODY,
                },
            },
            "required": ["title", "body"],
            "additionalProperties": false,
        }),
    }
}

fn board_tool() -> Tool {
    Tool {
        name: "board".into(),
        description: "Lists the cards on imece's board: open ones, and who took the others.".into(),
        input: json!({ "type": "object", "properties": {}, "additionalProperties": false }),
    }
}

fn claim_tool() -> Tool {
    Tool {
        name: "claim".into(),
        description: "Takes an open card from imece's board; the answer is its task. \
                      One card at a time."
            .into(),
        input: json!({
            "type": "object",
            "properties": {
                "card": { "type": "integer", "minimum": 1, "description": "The card's number, without #." },
            },
            "required": ["card"],
            "additionalProperties": false,
        }),
    }
}

/// The prompt that offers a worker the open `cards`, each with who posted
/// it.
fn board_prompt(w: &World, cards: &[(CardId, AgentId)]) -> String {
    let mut out = String::from(
        "Open cards on imece's board (posted by your teammates, not by the person):\n",
    );
    for &(c, by) in cards {
        let by = &w.agents[by].name;
        out.push_str(&format!(
            "\n#{} {} (posted by {by})",
            c + 1,
            w.cards[c].title
        ));
    }
    out.push_str(
        "\n\nIf one fits you, take it with claim and do it. If none does, end your \
         turn with one line saying why.",
    );
    out
}

/// imece's `send` tool, as agents see it.
fn send_tool() -> Tool {
    let kinds: Vec<&str> = MsgKind::ALL.iter().map(|k| k.wire()).collect();
    Tool {
        name: "send".into(),
        description: "Sends a message to a teammate in imece. It arrives as the start of \
                      their next turn. assign, ask, answer, handoff, done, blocked and \
                      cancel start that turn as soon as they are free; inform, progress \
                      and notice wait until something else does."
            .into(),
        input: json!({
            "type": "object",
            "properties": {
                "to": { "type": "string", "description": "The teammate's name." },
                "type": { "type": "string", "enum": kinds },
                "body": {
                    "type": "string",
                    "description": "The message. For assign: the task, the files it \
                                    touches and when it is done.",
                    "maxLength": MAX_BODY,
                },
            },
            "required": ["to", "type", "body"],
            "additionalProperties": false,
        }),
    }
}

/// The prompt that hands an agent its mail.
fn mail_prompt(w: &World, mail: &[Mail]) -> String {
    let mut out = String::from(
        "Messages from your teammates in imece (written by those agents, not by the person):",
    );
    for m in mail {
        out.push_str(&format!(
            "\n\n[{}] from {}:\n{}",
            m.kind.wire(),
            w.agents[m.from].name,
            m.body
        ));
    }
    out
}

fn system_prompt(w: &World, id: AgentId) -> String {
    let a = &w.agents[id];
    let workers: Vec<&str> = w
        .agents
        .iter()
        .filter(|o| o.role == Role::Worker)
        .map(|o| o.name.as_str())
        .collect();
    let leader = &w.agents[w.projects[a.project].leader].name;
    match a.role {
        Role::Leader => format!(
            "You are {}, the leader of a team of coding agents in imece, on the \
             repository in your current directory. The workers are {}; each \
             works in its own git worktree of it. You can read files but not \
             change them or run commands. The person talks to you. When work \
             should be done, split it and give each worker its part with the \
             send tool (type assign: what to do, which files, when it is \
             done); give different workers different files where you can. \
             Work that any worker could pick up may go on the board instead \
             (post); idle workers take it themselves. Workers answer with \
             done, blocked or ask; their messages start your next turn. Answer the person briefly: what you assigned, and \
             when all is done, what was done and what is left.",
            a.name,
            workers.join(", ")
        ),
        _ => format!(
            "You are {}, a coding agent in imece, a team led by {leader}. You work \
             in your own git worktree of the repository (your current \
             directory); the others work in theirs. You can read and change \
             files here but not run commands. Do not commit. Tasks come from \
             {leader} or from the person. When you finish a task from \
             {leader}, send it a done message with a short summary of what \
             you changed and what is left open; if you are stuck, send blocked \
             or ask instead of guessing. imece may offer you open cards from \
             the board; take one that fits with claim, and send whoever posted \
             it a done message when it is done. Work you find outside your \
             task goes on the board (post), not into your change. Use these \
             tools only for that, not for chatter.",
            a.name
        ),
    }
}

/// `$XDG_STATE_HOME/imece`, or `~/.local/state/imece`.
fn state_dir() -> PathBuf {
    std::env::var_os("XDG_STATE_HOME")
        .map(PathBuf::from)
        .filter(|p| p.is_absolute())
        .or_else(|| std::env::var_os("HOME").map(|h| Path::new(&h).join(".local/state")))
        .unwrap_or_else(std::env::temp_dir)
        .join("imece")
}

/// FNV-1a: tells checkouts with the same name apart. Not for security.
fn fnv(bytes: &[u8]) -> u32 {
    bytes.iter().fold(0x811c_9dc5, |h, &b| {
        (h ^ u32::from(b)).wrapping_mul(0x0100_0193)
    })
}

fn local_seconds_after_midnight() -> u64 {
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |d| d.as_secs());
    let secs = now as libc::time_t;
    // SAFETY: `tm` is plain data that localtime_r fills; it keeps no pointer.
    let mut tm: libc::tm = unsafe { std::mem::zeroed() };
    if unsafe { libc::localtime_r(&secs, &mut tm) }.is_null() {
        return now % 86_400;
    }
    (tm.tm_hour as u64) * 3600 + (tm.tm_min as u64) * 60 + tm.tm_sec as u64
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A team without sessions, to apply events to.
    fn team(workers: usize) -> World {
        let mut w = World {
            tick: 3,
            projects: vec![Project {
                id: 0,
                name: "repo".into(),
                leader: 0,
                trunk: 0,
                repo: &[],
            }],
            agents: Vec::new(),
            cards: Vec::new(),
            journal: Vec::new(),
            inbox: Vec::new(),
            daily_budget: 10_000,
            max_researchers: 0,
            research_queue: Default::default(),
            plans: Vec::new(),
        };
        w.agents.push(Agent::new(
            0,
            0,
            "lead".into(),
            Vendor::Claude,
            Role::Leader,
            Status::Idle,
            0,
        ));
        for i in 1..=workers {
            w.agents.push(Agent::new(
                i,
                0,
                format!("claude-{i}"),
                Vendor::Claude,
                Role::Worker,
                Status::Working,
                0,
            ));
        }
        w
    }

    fn with_card(w: &mut World, id: AgentId) -> CardId {
        let c = w.cards.len();
        w.cards.push(Card {
            id: c,
            project: 0,
            title: "fix it".into(),
            owner: Some(id),
            column: Column::Working,
            story: Vec::new(),
            symbols: Vec::new(),
            started: 0,
            finished: 0,
        });
        w.agents[id].card = Some(c);
        c
    }

    #[test]
    fn a_finished_turn_answers_the_person_and_closes_the_card() {
        let mut w = team(1);
        let c = with_card(&mut w, 1);
        let edit = |target: &str| AgentEvent::Tool {
            name: "Edit".into(),
            target: target.into(),
        };
        apply(&mut w, 1, edit("src/a.rs"), 0);
        assert_eq!(w.agents[1].activity.as_deref(), Some("Edit src/a.rs"));
        apply(&mut w, 1, edit("src/a.rs"), 0);
        apply(&mut w, 1, edit("src/b.rs"), 0);
        apply(
            &mut w,
            1,
            AgentEvent::TurnDone {
                ok: true,
                text: "Fixed both.\nDetails.".into(),
                cost: 123_400,
                denied: 0,
            },
            50,
        );
        let a = &w.agents[1];
        assert_eq!(a.status, Status::Idle);
        assert_eq!(a.cost, 50 + 1234);
        assert_eq!(a.activity, None);
        assert_eq!(a.talk.as_ref().unwrap().text, "Fixed both.");
        let touched = w.cards[c]
            .story
            .iter()
            .filter(|s| matches!(s.event, StoryEvent::Touched(_)))
            .count();
        assert_eq!(touched, 2, "a file is listed once per card");
        assert_eq!(w.cards[c].column, Column::Done);
        assert!(matches!(
            w.journal.last().unwrap().event,
            Event::Replied { agent: 1, .. }
        ));
    }

    #[test]
    fn a_stopped_session_needs_the_person() {
        let mut w = team(1);
        with_card(&mut w, 1);
        apply(
            &mut w,
            1,
            AgentEvent::Stopped("exited with code 1".into()),
            0,
        );
        assert_eq!(w.agents[1].status, Status::Stopped);
        assert!(w.agents[1].status.needs_attention());
        assert!(matches!(
            &w.journal.last().unwrap().event,
            Event::Stopped { agent: 1, why } if why == "exited with code 1"
        ));
    }

    #[test]
    fn a_failed_turn_is_not_an_answer() {
        let mut w = team(1);
        let c = with_card(&mut w, 1);
        apply(
            &mut w,
            1,
            AgentEvent::TurnDone {
                ok: false,
                text: "error_max_budget_usd".into(),
                cost: 0,
                denied: 0,
            },
            0,
        );
        assert_ne!(w.cards[c].column, Column::Done);
        assert!(w.agents[1].talk.is_none());
    }

    fn wait_for(live: &mut Live, w: &mut World, done: impl Fn(&World) -> bool) {
        let start = Instant::now();
        while !done(w) {
            assert!(start.elapsed().as_secs() < 10, "timed out");
            live.step(w);
            thread::sleep(std::time::Duration::from_millis(10));
        }
    }

    #[test]
    fn an_agent_over_its_share_is_not_prompted() {
        let mut w = team(1);
        let mut live = Live::new(
            2,
            // Never runs: the budget check comes first.
            "/nonexistent/claude".into(),
            PathBuf::from("/nonexistent"),
            PathBuf::from("/nonexistent"),
            None,
            100,
        );
        w.agents[1].cost = 100;
        assert_eq!(live.told(&mut w, 1, "go"), Err(Refusal::Budget));
        w.agents[1].cost = 0;
        w.agents[0].cost = w.daily_budget;
        assert_eq!(live.told(&mut w, 1, "go"), Err(Refusal::Budget));
        assert!(w.cards.is_empty());
    }

    #[test]
    fn a_session_that_can_not_start_stops_the_agent_and_the_next_message_retries() {
        let mut w = team(1);
        let mut live = Live::new(
            2,
            "/nonexistent/claude".into(),
            std::env::temp_dir(),
            PathBuf::from("/nonexistent"),
            None,
            5_000,
        );
        live.told(&mut w, 0, "first").unwrap();
        // The message waits while the session starts on its own thread.
        assert!(matches!(&live.slots[0], Slot::Starting(q) if q == &["first".to_string()]));
        live.told(&mut w, 0, "second").unwrap();
        wait_for(&mut live, &mut w, |w| w.agents[0].status == Status::Stopped);
        assert!(matches!(live.slots[0], Slot::None));
        assert!(matches!(
            &w.journal.last().unwrap().event,
            Event::Stopped { agent: 0, why } if why.contains("/nonexistent/claude")
        ));
        // Talking to it again tries a new session.
        live.told(&mut w, 0, "third").unwrap();
        assert!(matches!(live.slots[0], Slot::Starting(_)));
    }

    fn idle_live(agents: usize) -> Live {
        Live::new(
            agents,
            // Sessions never start: what counts is what was queued for them.
            "/nonexistent/claude".into(),
            std::env::temp_dir(),
            PathBuf::from("/nonexistent"),
            None,
            5_000,
        )
    }

    fn msg(to: &str, kind: &str, body: &str) -> Value {
        json!({ "to": to, "type": kind, "body": body })
    }

    fn queued(live: &Live, id: AgentId) -> Vec<String> {
        match &live.slots[id] {
            Slot::Starting(q) => q.clone(),
            _ => Vec::new(),
        }
    }

    #[test]
    fn an_assign_to_an_idle_worker_starts_its_turn_with_a_card() {
        let mut w = team(2);
        w.agents[1].status = Status::Idle;
        let mut live = idle_live(3);
        let got = live.send(
            &mut w,
            0,
            &msg("claude-1", "assign", "Fix the parser\nin src/p.rs"),
        );
        assert!(got.unwrap().starts_with("delivered"));
        live.step(&mut w);
        let q = queued(&live, 1);
        assert_eq!(q.len(), 1);
        assert!(
            q[0].contains("[assign] from lead:\nFix the parser\nin src/p.rs"),
            "{q:?}"
        );
        assert!(q[0].contains("not by the person"));
        let card = w.agents[1].card.expect("a card");
        assert_eq!(w.cards[card].title, "Fix the parser");
        assert!(live.mail[1].is_empty());
        assert!(matches!(
            w.journal.last().unwrap().event,
            Event::Sent {
                from: 0,
                to: 1,
                kind: MsgKind::Assign,
                ..
            }
        ));
    }

    #[test]
    fn a_busy_agent_gets_its_mail_when_its_turn_ends() {
        let mut w = team(1);
        let mut live = idle_live(2);
        live.turns[0] = 1;
        let got = live.send(&mut w, 1, &msg("lead", "done", "Parser fixed."));
        assert!(got.unwrap().contains("busy"));
        live.step(&mut w);
        assert!(queued(&live, 0).is_empty(), "not while it works");
        live.tx
            .send((
                0,
                0,
                AgentEvent::TurnDone {
                    ok: true,
                    text: "ok".into(),
                    cost: 0,
                    denied: 0,
                },
            ))
            .unwrap();
        live.step(&mut w);
        assert_eq!(live.turns[0], 1);
        assert!(queued(&live, 0)[0].contains("[done] from claude-1:\nParser fixed."));
    }

    #[test]
    fn an_inform_waits_for_a_turn_something_else_starts() {
        let mut w = team(2);
        let mut live = idle_live(3);
        live.send(&mut w, 2, &msg("lead", "progress", "Half done."))
            .unwrap();
        live.step(&mut w);
        assert!(queued(&live, 0).is_empty());
        live.send(&mut w, 1, &msg("lead", "ask", "Which file?"))
            .unwrap();
        live.step(&mut w);
        let q = queued(&live, 0);
        assert_eq!(q.len(), 1, "both in one turn");
        assert!(q[0].contains("[progress] from claude-2") && q[0].contains("[ask] from claude-1"));
    }

    #[test]
    fn bad_messages_are_refused_with_a_reason() {
        let mut w = team(1);
        let mut live = idle_live(2);
        let refused = |live: &mut Live, w: &mut World, args: Value| {
            live.send(w, 1, &args).expect_err("refused")
        };
        assert!(refused(&mut live, &mut w, msg("ghost", "ask", "hi")).contains("the team is lead"));
        assert!(refused(&mut live, &mut w, msg("claude-1", "ask", "me")).contains("no teammate"));
        assert!(refused(&mut live, &mut w, msg("lead", "shout", "hi")).contains("one of assign"));
        assert!(refused(&mut live, &mut w, msg("lead", "ask", "  ")).contains("empty"));
        let long = "x".repeat(MAX_BODY + 1);
        assert!(refused(&mut live, &mut w, msg("lead", "ask", &long)).contains("longer"));
        assert!(refused(&mut live, &mut w, json!({ "to": "lead" })).contains("type"));
        w.agents[0].status = Status::Stopped;
        assert!(refused(&mut live, &mut w, msg("lead", "ask", "hi")).contains("stopped"));
        assert!(live.mail[0].is_empty());
        assert!(w.journal.is_empty());
    }

    #[test]
    fn agents_can_not_keep_each_other_busy_without_the_person() {
        let mut w = team(1);
        let mut live = idle_live(2);
        live.woken[0] = MAX_WOKEN;
        let err = live
            .send(&mut w, 1, &msg("lead", "ask", "again?"))
            .unwrap_err();
        assert!(err.contains("tell the person"), "{err}");
        // What does not wake still goes through.
        live.send(&mut w, 1, &msg("lead", "inform", "fyi")).unwrap();
        // The person speaking resets the count.
        live.told(&mut w, 1, "carry on").unwrap();
        assert_eq!(live.woken[0], 0);
        live.send(&mut w, 1, &msg("lead", "ask", "again?")).unwrap();
        live.mail[0].clear();
        for _ in 0..MAX_MAIL {
            live.send(&mut w, 1, &msg("lead", "inform", "x")).unwrap();
        }
        assert!(
            live.send(&mut w, 1, &msg("lead", "inform", "x"))
                .unwrap_err()
                .contains("unread")
        );
    }

    #[test]
    fn an_agent_over_its_share_gets_no_waking_mail() {
        let mut w = team(1);
        let mut live = idle_live(2);
        w.agents[0].cost = 5_000;
        assert!(
            live.send(&mut w, 1, &msg("lead", "done", "x"))
                .unwrap_err()
                .contains("budget")
        );
    }

    #[test]
    fn the_bus_answers_through_the_socket_of_the_caller() {
        use std::io::{BufRead, BufReader, Write};
        use std::os::unix::net::UnixStream;
        let mut w = team(1);
        let mut live = idle_live(2);
        live.open_bus(&w, "/opt/imece".into()).unwrap();
        let dir = live.bus.as_ref().unwrap().dir.clone();
        use std::os::unix::fs::PermissionsExt;
        assert_eq!(
            std::fs::metadata(&dir).unwrap().permissions().mode() & 0o777,
            0o700
        );
        let path = live.bus.as_ref().unwrap().servers[1].path().to_path_buf();
        let caller = thread::spawn(move || {
            let mut sock = UnixStream::connect(path).unwrap();
            let call = json!({"jsonrpc":"2.0","id":1,"method":"tools/call","params":{"name":"send",
                "arguments":{"to":"lead","type":"inform","body":"from the socket"}}});
            writeln!(sock, "{call}").unwrap();
            let mut line = String::new();
            BufReader::new(&sock).read_line(&mut line).unwrap();
            line
        });
        wait_for(&mut live, &mut w, |w| !w.journal.is_empty());
        let reply: Value = serde_json::from_str(&caller.join().unwrap()).unwrap();
        assert_eq!(reply["result"]["isError"], false, "{reply}");
        // The sender is whose socket it was.
        assert!(matches!(
            w.journal[0].event,
            Event::Sent { from: 1, to: 0, .. }
        ));
        // The leader's socket does not even offer claim.
        let lead = live.bus.as_ref().unwrap().servers[0].path().to_path_buf();
        let mut sock = UnixStream::connect(lead).unwrap();
        writeln!(sock, r#"{{"jsonrpc":"2.0","id":1,"method":"tools/list"}}"#).unwrap();
        let mut line = String::new();
        BufReader::new(&sock).read_line(&mut line).unwrap();
        let listed: Value = serde_json::from_str(&line).unwrap();
        let names: Vec<&str> = listed["result"]["tools"]
            .as_array()
            .unwrap()
            .iter()
            .map(|t| t["name"].as_str().unwrap())
            .collect();
        assert_eq!(names, ["send", "post", "board"]);
        drop(sock);
        drop(live);
        assert!(!dir.exists());
    }

    fn card(title: &str, body: &str) -> Value {
        json!({ "title": title, "body": body })
    }

    #[test]
    fn a_posted_card_is_offered_to_one_idle_worker_without_a_card_of_its_own() {
        let mut w = team(2);
        let mut live = idle_live(3);
        let got = live.post(&mut w, 0, &card("Add a test", "Cover sub() in src/m.rs."));
        assert!(got.unwrap().starts_with("posted as #1"));
        assert_eq!(w.cards[0].column, Column::Waiting);
        assert_eq!(w.cards[0].owner, None);
        assert!(matches!(
            w.journal.last().unwrap().event,
            Event::Posted { agent: 0, card: 0 }
        ));
        live.step(&mut w);
        let q = queued(&live, 1);
        assert_eq!(q.len(), 1);
        assert!(q[0].contains("#1 Add a test (posted by lead)"), "{q:?}");
        assert!(q[0].contains("not by the person"));
        assert!(
            !q[0].contains("Cover sub()"),
            "the body comes with the claim"
        );
        assert_eq!(w.cards.len(), 1, "choosing is not a card");
        assert_eq!(w.agents[1].card, None);
        // Not in front of two workers at once.
        assert!(queued(&live, 2).is_empty());
        live.step(&mut w);
        assert_eq!(queued(&live, 1).len(), 1, "offered once");
    }

    #[test]
    fn a_card_goes_to_the_next_worker_when_the_first_lets_it_be() {
        let mut w = team(2);
        let mut live = idle_live(3);
        live.post(&mut w, 0, &card("Add a test", "x")).unwrap();
        live.step(&mut w);
        assert_eq!(live.offered[&0], [1]);
        // claude-1's turn ended without taking it.
        live.turns[1] = 0;
        live.step(&mut w);
        assert!(queued(&live, 2)[0].contains("#1 Add a test"));
        assert_eq!(live.offered[&0], [1, 2]);
        live.turns[2] = 0;
        live.step(&mut w);
        assert_eq!(live.offered[&0], [1, 2], "each worker sees a card once");
    }

    #[test]
    fn a_claim_makes_the_card_the_workers_and_hands_it_the_task() {
        let mut w = team(2);
        let mut live = idle_live(3);
        live.post(&mut w, 0, &card("Add a test", "Cover sub()."))
            .unwrap();
        let got = live.claim(&mut w, 1, &json!({ "card": 1 })).unwrap();
        assert!(
            got.contains("Cover sub().") && got.contains("send lead a done"),
            "{got}"
        );
        assert_eq!(w.cards[0].owner, Some(1));
        assert_eq!(w.cards[0].column, Column::Working);
        assert_eq!(w.agents[1].card, Some(0));
        assert!(matches!(
            w.journal.last().unwrap().event,
            Event::Claimed { agent: 1, card: 0 }
        ));
        let err = live.claim(&mut w, 2, &json!({ "card": 1 })).unwrap_err();
        assert!(err.contains("taken by claude-1"), "{err}");
        assert!(
            live.board(&w, 2)
                .contains("#1 Add a test (taken by claude-1)")
        );
        // The turn ends: the card is done, and stays off the open list.
        live.step(&mut w);
        assert!(
            queued(&live, 2).is_empty(),
            "a taken card is offered to nobody"
        );
        apply(
            &mut w,
            1,
            AgentEvent::TurnDone {
                ok: true,
                text: "Added.".into(),
                cost: 0,
                denied: 0,
            },
            0,
        );
        assert_eq!(w.cards[0].column, Column::Done);
        assert!(live.board(&w, 0).contains("(done by claude-1)"));
    }

    #[test]
    fn bad_claims_are_refused_with_a_reason() {
        let mut w = team(1);
        let mut live = idle_live(2);
        live.post(&mut w, 1, &card("Split p.rs", "x")).unwrap();
        let err = live.claim(&mut w, 0, &json!({ "card": 1 })).unwrap_err();
        assert!(err.contains("only workers"), "{err}");
        for bad in [
            json!({ "card": 0 }),
            json!({ "card": 9 }),
            json!({ "card": "1" }),
            json!({}),
        ] {
            let err = live.claim(&mut w, 1, &bad).unwrap_err();
            assert!(err.contains("no such card"), "{bad} {err}");
        }
        // A card that did not come from the board is not one to take.
        with_card(&mut w, 0);
        w.cards[1].column = Column::Waiting;
        assert!(live.claim(&mut w, 1, &json!({ "card": 2 })).is_err());
        // One card at a time.
        let mine = with_card(&mut w, 1);
        let err = live.claim(&mut w, 1, &json!({ "card": 1 })).unwrap_err();
        assert!(err.contains(&format!("finish #{}", mine + 1)), "{err}");
        assert_eq!(w.cards[0].column, Column::Waiting);
    }

    #[test]
    fn bad_posts_are_refused_and_the_board_is_capped() {
        let mut w = team(1);
        let mut live = idle_live(2);
        let refused = |live: &mut Live, w: &mut World, args: Value| {
            live.post(w, 0, &args).expect_err("refused")
        };
        assert!(refused(&mut live, &mut w, card("", "x")).contains("title and a body"));
        assert!(refused(&mut live, &mut w, card("t", " ")).contains("title and a body"));
        assert!(refused(&mut live, &mut w, card("two\nlines", "x")).contains("one line"));
        let long = "x".repeat(MAX_TITLE + 1);
        assert!(refused(&mut live, &mut w, card(&long, "x")).contains("one line"));
        let body = "x".repeat(MAX_BODY + 1);
        assert!(refused(&mut live, &mut w, card("t", &body)).contains("longer"));
        assert!(w.cards.is_empty());

        for i in 0..MAX_POSTS {
            live.post(&mut w, 0, &card(&format!("t{i}"), "x")).unwrap();
        }
        assert!(refused(&mut live, &mut w, card("t", "x")).contains("tell the person"));
        // The person speaking resets what an agent may post, not the board.
        live.told(&mut w, 0, "go on").unwrap();
        for i in 0..MAX_OPEN as u32 - MAX_POSTS {
            live.post(&mut w, 0, &card(&format!("u{i}"), "x")).unwrap();
        }
        assert!(refused(&mut live, &mut w, card("t", "x")).contains("open cards"));
    }

    #[test]
    fn only_workers_get_the_claim_tool() {
        let names = |r: Role| -> Vec<String> { tools_of(r).into_iter().map(|t| t.name).collect() };
        assert_eq!(names(Role::Leader), ["send", "post", "board"]);
        assert_eq!(names(Role::Worker), ["send", "post", "board", "claim"]);
    }

    #[test]
    fn a_late_event_of_an_ended_session_is_ignored() {
        let mut w = team(1);
        let mut live = Live::new(2, "x".into(), PathBuf::new(), PathBuf::new(), None, 5_000);
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
