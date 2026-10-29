//! Live agents: real Claude Code sessions feed the model the simulation
//! feeds. One project (the checkout imece runs in), a leader that reads it,
//! and workers that each change their own git worktree of it.
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

use std::collections::HashMap;
use std::io;
use std::path::{Path, PathBuf};
use std::sync::mpsc::{self, Receiver, Sender};
use std::thread;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use imece_agent::claude::{self, Mcp, Session, Spec};
use imece_agent::mcp::{Answer, Server, Tool};
use imece_agent::{Event as AgentEvent, Tier, child, worktree};
use serde_json::{Value, json};

use crate::model::*;

pub struct Options {
    pub repo: PathBuf,
    /// Workers the leader may hire, at most.
    pub max_workers: usize,
    /// The leader's vendor.
    pub lead: Vendor,
    /// The model every agent runs on unless a tier says otherwise; none
    /// for each vendor's own default.
    pub model: Option<String>,
    /// Models that stand in for the vendors' own, by tier.
    pub tiers: Vec<(Tier, String)>,
    /// Ten-thousandths of a dollar, for all agents together.
    pub budget: u64,
}

/// The coding agents imece knows: their program on PATH, and whether imece
/// can drive them yet.
const KNOWN: &[(Vendor, &str, bool)] = &[
    (Vendor::Claude, claude::PROGRAM, true),
    (Vendor::Codex, "codex", false),
    (Vendor::Gemini, "gemini", false),
];

/// The models a vendor's tiers stand for; none is the vendor's own default.
#[derive(Clone, Debug, PartialEq)]
struct Ladder {
    cheap: Option<String>,
    standard: Option<String>,
    strong: Option<String>,
}

impl Ladder {
    fn model(&self, tier: Tier) -> Option<&str> {
        match tier {
            Tier::Cheap => &self.cheap,
            Tier::Standard => &self.standard,
            Tier::Strong => &self.strong,
        }
        .as_deref()
    }

    fn set(&mut self, tier: Tier, model: String) {
        *match tier {
            Tier::Cheap => &mut self.cheap,
            Tier::Standard => &mut self.standard,
            Tier::Strong => &mut self.strong,
        } = Some(model);
    }
}

/// A vendor imece can drive, found on PATH.
#[derive(Clone, Debug)]
struct Hireable {
    vendor: Vendor,
    program: String,
    ladder: Ladder,
}

impl Hireable {
    /// Claude Code at `program`; `standard` is the model the person named,
    /// if any, and `tiers` their stand-ins for the other tiers.
    fn claude(program: &str, standard: Option<String>, tiers: &[(Tier, String)]) -> Hireable {
        let mut ladder = Ladder {
            cheap: claude::model(Tier::Cheap).map(String::from),
            standard,
            strong: claude::model(Tier::Strong).map(String::from),
        };
        for (tier, model) in tiers {
            ladder.set(*tier, model.clone());
        }
        Hireable {
            vendor: Vendor::Claude,
            program: program.to_string(),
            ladder,
        }
    }
}

/// Whether `program` runs from `repo` with a cleared environment.
fn on_path(program: &str, repo: &Path) -> bool {
    child::command(program, repo, &[])
        .arg("--version")
        .output()
        .is_ok_and(|o| o.status.success())
}

/// Which of the vendors `present` on PATH (with whether imece drives them)
/// may be hired, given the leader must be `lead`; or why the team can not
/// start.
fn hireable(present: &[(Vendor, bool)], lead: Vendor) -> Result<Vec<Vendor>, String> {
    let names = |v: &[(Vendor, bool)]| {
        v.iter()
            .map(|(v, _)| v.label())
            .collect::<Vec<_>>()
            .join(", ")
    };
    if present.is_empty() {
        let known: Vec<&str> = KNOWN.iter().map(|k| k.1).collect();
        return Err(format!(
            "no coding agent on PATH (looked for {}); install Claude Code \
             (https://claude.com/claude-code) and log in",
            known.join(", ")
        ));
    }
    match present.iter().find(|(v, _)| *v == lead) {
        None => Err(format!(
            "{} is not on PATH (found {}); install it, or pick the leader with --lead",
            lead.label(),
            names(present)
        )),
        Some((_, false)) => Err(format!(
            "{} is on PATH, but imece can not drive it yet; start with --lead claude",
            lead.label()
        )),
        Some((_, true)) => Ok(present
            .iter()
            .filter(|(_, drives)| *drives)
            .map(|(v, _)| *v)
            .collect()),
    }
}

/// Why a message could not reach an agent.
/// Claude Code's result subtype when a session passed its `--max-budget-usd`.
const CAPPED: &str = "error_max_budget_usd";

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
    /// The vendors the leader may hire from.
    vendors: Vec<Hireable>,
    /// Workers the leader may hire, at most.
    max_workers: usize,
    repo: PathBuf,
    /// Where the workers' worktrees live, outside the checkout.
    root: PathBuf,
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
}

/// Finds the coding agents on PATH, then builds the team for the checkout
/// `repo` is in: the leader alone, who hires the rest.
pub fn world(o: Options) -> Result<(World, Live), String> {
    let repo = worktree::top(&o.repo).map_err(|e| format!("{}: {e}", o.repo.display()))?;
    let present: Vec<(Vendor, bool)> = KNOWN
        .iter()
        .filter(|(_, program, _)| on_path(program, &repo))
        .map(|&(vendor, _, drives)| (vendor, drives))
        .collect();
    let vendors: Vec<Hireable> = hireable(&present, o.lead)?
        .into_iter()
        // Claude Code is the only vendor imece drives so far (see `KNOWN`).
        .map(|_| Hireable::claude(claude::PROGRAM, o.model.clone(), &o.tiers))
        .collect();
    let name = repo
        .file_name()
        .map_or("repo".into(), |n| n.to_string_lossy().into_owned());
    let root = crate::prefs::state_dir().join("worktrees").join(format!(
        "{name}-{:08x}",
        fnv(repo.as_os_str().as_encoded_bytes())
    ));

    let w = World {
        tick: 0,
        projects: vec![Project {
            id: 0,
            name,
            leader: 0,
            trunk: None,
            repo: &[],
        }],
        agents: vec![Agent::new(
            0,
            0,
            "lead".into(),
            o.lead,
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
    start_clock_at(local_seconds_after_midnight());
    let mut live = Live::new(1, vendors, repo, root);
    live.max_workers = o.max_workers;
    live.models[0] = live.of(o.lead).ladder.standard.clone();
    let relay = std::env::current_exe().map_err(|e| format!("imece's own path: {e}"))?;
    live.open_bus(&w, relay.to_string_lossy().into_owned())
        .map_err(|e| format!("the agents' message bus: {e}"))?;
    Ok((w, live))
}

impl Live {
    /// `agents` places to begin with, all on their vendor's default model.
    /// `vendors` must hold every vendor an agent has.
    fn new(agents: usize, vendors: Vec<Hireable>, repo: PathBuf, root: PathBuf) -> Live {
        let (tx, rx) = mpsc::channel();
        let (ready_tx, ready_rx) = mpsc::channel();
        let (calls_tx, calls_rx) = mpsc::channel();
        let mut live = Live {
            started: Instant::now(),
            tx,
            rx,
            ready_tx,
            ready_rx,
            slots: Vec::new(),
            generation: Vec::new(),
            spent_before: Vec::new(),
            vendors,
            max_workers: usize::MAX,
            repo,
            root,
            models: Vec::new(),
            tiered: Vec::new(),
            turned: Vec::new(),
            inflight: Vec::new(),
            renewed: Vec::new(),
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
        };
        for _ in 0..agents {
            live.grow(None, false);
        }
        live
    }

    /// Makes room for one more agent, on `model` (`tiered`: picked by
    /// tier, so it may fall back).
    fn grow(&mut self, model: Option<String>, tiered: bool) {
        self.slots.push(Slot::None);
        self.generation.push(0);
        self.spent_before.push(0);
        self.models.push(model);
        self.tiered.push(tiered);
        self.turned.push(false);
        self.inflight.push(Vec::new());
        self.renewed.push(false);
        self.mail.push(Vec::new());
        self.turns.push(0);
        self.asked.push(false);
        self.woken.push(0);
        self.posted.push(0);
    }

    /// The vendor `v` as imece drives it. Every agent's vendor is one.
    fn of(&self, v: Vendor) -> &Hireable {
        self.vendors
            .iter()
            .find(|h| h.vendor == v)
            .unwrap_or_else(|| panic!("{} is not a vendor imece drives", v.label()))
    }

    /// The vendors' names, for the hire tool.
    fn vendor_names(&self) -> Vec<&str> {
        self.vendors.iter().map(|h| h.vendor.label()).collect()
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
            self.bind(id, w.agents[id].role)?;
        }
        Ok(())
    }

    /// Binds the socket of agent `id`, the next one to get a socket, with
    /// the tools of `role`. Without a bus (before it opens) there is
    /// nothing to bind.
    fn bind(&mut self, id: AgentId, role: Role) -> io::Result<()> {
        let tools = tools_of(role, &self.vendor_names());
        let Some(bus) = &mut self.bus else {
            return Ok(());
        };
        assert_eq!(bus.servers.len(), id, "sockets are bound in agent order");
        let tx = self.calls_tx.clone();
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
            if let AgentEvent::Stopped(_) = e {
                self.end(w, id);
            }
            apply(w, id, e, self.spent_before[id]);
            if turn_done {
                self.turns[id] = self.turns[id].saturating_sub(1);
                if self.turns[id] > 0 {
                    // The session goes on with a prompt it already has.
                    w.agents[id].set_status(Status::Working, w.tick);
                } else if let Some(text) = answer.filter(|_| self.asked[id] || self.reports(w, id))
                {
                    // The person waits for this: it goes to them, not
                    // only into the journal.
                    self.asked[id] = false;
                    w.replied(id, &text);
                }
            }
        }
        while let Ok((from, tool, args, reply)) = self.calls_rx.try_recv() {
            let answer = match tool.as_str() {
                "send" => self.send(w, from, &args),
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
            if waking && self.turns[id] == 0 && w.agents[id].status != Status::Stopped {
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

    /// A session ended: the next message starts a new one.
    fn end(&mut self, w: &World, id: AgentId) {
        self.spent_before[id] = w.agents[id].cost;
        self.slots[id] = Slot::None;
        self.generation[id] += 1;
        self.turns[id] = 0;
        self.turned[id] = false;
        self.inflight[id].clear();
    }

    /// Agent `id`'s tier model is not available here: its session ends and
    /// a new one on its vendor's standard model gets the prompts the old
    /// one had.
    fn fall_back(&mut self, w: &mut World, id: AgentId) {
        let model = self.models[id].take().unwrap_or_default();
        self.models[id] = self.of(w.agents[id].vendor).ladder.standard.clone();
        self.tiered[id] = false;
        let texts = std::mem::take(&mut self.inflight[id]);
        self.end(w, id);
        w.agents[id].trace(
            w.tick,
            format!("model {model} not available here; falling back to the default"),
        );
        w.note(Event::Fallback { agent: id, model });
        for text in texts {
            if let Err(why) = self.prompt(w, id, text, None) {
                return apply(w, id, AgentEvent::Stopped(why), self.spent_before[id]);
            }
        }
    }

    /// Agent `id`'s session hit its cap, but the budget has grown since the
    /// session started: a new session, capped at what is left now, gets the
    /// prompts the old one had.
    fn renew(&mut self, w: &mut World, id: AgentId) {
        let texts = std::mem::take(&mut self.inflight[id]);
        self.end(w, id);
        self.renewed[id] = true;
        w.agents[id].trace(
            w.tick,
            "session hit its cap; a new one takes what is left of the budget".to_string(),
        );
        for text in texts {
            if let Err(why) = self.prompt(w, id, text, None) {
                return apply(w, id, AgentEvent::Stopped(why), self.spent_before[id]);
            }
        }
    }

    /// The leader `from` adds a worker (the `hire` tool's arguments). The
    /// answer names it.
    fn hire(&mut self, w: &mut World, from: AgentId, args: &Value) -> Answer {
        let leader = &w.agents[from];
        if leader.role != Role::Leader {
            return Err("only the leader hires; ask it with send (type ask)".into());
        }
        let (project, own) = (leader.project, leader.vendor);
        let workers = w
            .agents
            .iter()
            .filter(|o| o.project == project && o.role == Role::Worker && !o.gone)
            .count();
        if self.max_workers == 0 {
            return Err(
                "the person runs you without workers (--agents 0); answer from what you read"
                    .into(),
            );
        }
        if workers >= self.max_workers {
            return Err(format!(
                "the team is full: {workers} workers, the most the person allowed \
                 (--agents {}); give the work to one of them",
                self.max_workers
            ));
        }
        let arg = |k: &str| args[k].as_str().map(str::trim).unwrap_or_default();
        let vendor = match arg("vendor") {
            "" => self.of(own).clone(),
            v => self
                .vendors
                .iter()
                .find(|h| h.vendor.label() == v)
                .cloned()
                .ok_or_else(|| {
                    format!(
                        "there is no vendor {v:?} here; you can hire {}",
                        self.vendor_names().join(", ")
                    )
                })?,
        };
        let tier = match arg("tier") {
            "" => Tier::Standard,
            t => Tier::from_wire(t).ok_or("tier must be cheap, standard or strong")?,
        };
        let model = vendor.ladder.model(tier).map(str::to_string);
        // Only a model other than the standard one has something to fall
        // back to.
        let tiered = model != vendor.ladder.standard;
        // The vendor's next number, skipping names already taken.
        let next = w
            .agents
            .iter()
            .filter(|o| o.vendor == vendor.vendor && o.role == Role::Worker)
            .count()
            + 1;
        let name = (next..)
            .map(|n| format!("{}-{n}", vendor.vendor.label()))
            .find(|name| w.agent_by_name(name).is_none())
            .expect("some number is free");
        let id = w.agents.len();
        // The socket first: an agent without one could never start.
        self.bind(id, Role::Worker)
            .map_err(|e| format!("{name} could not get a socket: {e}"))?;
        self.grow(model.clone(), tiered);
        let t = w.tick;
        w.agents.push(Agent::new(
            id,
            project,
            name.clone(),
            vendor.vendor,
            Role::Worker,
            Status::Idle,
            t,
        ));
        let shown = model.clone().unwrap_or_else(|| "the default model".into());
        let by = w.agents[from].name.clone();
        w.agents[from].trace(t, format!("hire {name} {} {shown}", tier.wire()));
        w.agents[id].trace(t, format!("hired by {by} · {shown}"));
        w.note(Event::Hired {
            by: from,
            agent: id,
            model,
        });
        Ok(format!(
            "{name} joined ({}, {shown}). Give it work with send (type assign). \
             {} more may be hired.",
            vendor.vendor.label(),
            self.max_workers - workers - 1
        ))
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
        if kind.wakes() && self.over_budget(w) {
            return Err("the budget is spent; nobody takes new turns".into());
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
                || self.over_budget(w)
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
        if self.over_budget(w) {
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

    /// Whether the team as a whole has spent the budget.
    fn over_budget(&self, w: &World) -> bool {
        w.spent() >= w.daily_budget
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
        if self.over_budget(w) {
            return Err(Refusal::Budget);
        }
        // The person is here again: agents may wake each other anew.
        self.woken.iter_mut().for_each(|n| *n = 0);
        self.posted.iter_mut().for_each(|n| *n = 0);
        let title = text.lines().next().unwrap_or_default().to_string();
        self.prompt(w, id, text.to_string(), Some(&title))
            .map_err(Refusal::Start)?;
        self.asked[id] = true;
        Ok(())
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
                self.slots[id] = Slot::Starting(vec![text.clone()]);
            }
            Slot::Starting(q) => q.push(text.clone()),
            Slot::Running(s) => {
                if let Err(e) = s.prompt(&text) {
                    self.end(w, id);
                    return Err(e.to_string());
                }
            }
        }
        self.inflight[id].push(text);
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
        let vendor = self.of(a.vendor);
        let mut spec = Spec {
            program: vendor.program.clone(),
            cwd: self.repo.clone(),
            name: format!("imece {}", a.name),
            tools: tools.iter().map(|t| t.to_string()).collect(),
            model: self.models[id].clone(),
            // In millionths of a dollar: what is left of the budget.
            budget: w.daily_budget.saturating_sub(w.spent()) * 100,
            system: system_prompt(w, id, self.max_workers),
            mcp: self.bus.as_ref().map(|b| Mcp {
                command: b.relay.clone(),
                args: vec![
                    "mcp-relay".into(),
                    b.servers[id].path().to_string_lossy().into_owned(),
                ],
                tools: tools_of(a.role, &self.vendor_names())
                    .into_iter()
                    .map(|t| t.name)
                    .collect(),
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

/// imece's tools for an agent of `role`: workers alone take cards, the
/// leader alone hires (from `vendors`).
fn tools_of(role: Role, vendors: &[&str]) -> Vec<Tool> {
    let mut tools = vec![send_tool(), post_tool(), board_tool()];
    match role {
        Role::Worker => tools.push(claim_tool()),
        Role::Leader => tools.push(hire_tool(vendors)),
        _ => {}
    }
    tools
}

fn hire_tool(vendors: &[&str]) -> Tool {
    let tiers: Vec<&str> = Tier::ALL.iter().map(|t| t.wire()).collect();
    Tool {
        name: "hire".into(),
        description: "Adds a worker to your team in imece: a coding agent in its own git \
                      worktree of the repository. The answer names it; then give it work \
                      with send (type assign). Hire for work that can run at once, not in \
                      advance: one worker takes several tasks in turn."
            .into(),
        input: json!({
            "type": "object",
            "properties": {
                "tier": {
                    "type": "string",
                    "enum": tiers,
                    "description": "cheap: small, well-specified mechanical work (renames, \
                                    tests of known behaviour, docs). standard (default): \
                                    ordinary fixes and features. strong: design that cuts \
                                    across the code, hard debugging, review.",
                },
                "vendor": {
                    "type": "string",
                    "enum": vendors,
                    "description": "Which coding agent (default: your own).",
                },
            },
            "additionalProperties": false,
        }),
    }
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

/// The system prompt of agent `id`; a leader may hire up to `max_workers`.
fn system_prompt(w: &World, id: AgentId, max_workers: usize) -> String {
    let a = &w.agents[id];
    let workers: Vec<&str> = w
        .agents
        .iter()
        .filter(|o| o.role == Role::Worker && !o.gone)
        .map(|o| o.name.as_str())
        .collect();
    let leader = &w.agents[w.projects[a.project].leader].name;
    match a.role {
        Role::Leader => {
            let team = if max_workers == 0 {
                "The person runs you without workers: answer from what you read, and \
                 say so when the work would need one."
                    .to_string()
            } else {
                let so_far = if workers.is_empty() {
                    "You have no workers yet.".to_string()
                } else {
                    format!("The workers so far are {}.", workers.join(", "))
                };
                format!(
                    "Workers are hired by you with the hire tool, up to {max_workers}; \
                     each is a coding agent in its own git worktree of the repository. \
                     {so_far} Hire for the work at hand, not in advance: one worker \
                     takes several tasks in turn, so hire a second only when two tasks \
                     can run at once. Pick a tier, not a model: cheap for small, \
                     well-specified mechanical work (renames, tests of known behaviour, \
                     docs), standard (the default) for ordinary fixes and features, \
                     strong for design that cuts across the code, hard debugging and \
                     review. Give each worker its part with the send tool (type \
                     assign: what to do, which files, when it is done); give different \
                     workers different files where you can. Work that any worker could \
                     pick up may go on the board instead (post); idle workers take it \
                     themselves. Workers answer with done, blocked or ask; their \
                     messages start your next turn."
                )
            };
            format!(
                "You are {}, the leader of a team of coding agents in imece, on the \
                 repository in your current directory. You can read files but not \
                 change them or run commands. The person talks to you. {team} You \
                 never change files yourself, so do not offer to; offer only work a \
                 worker will do. Answer the person briefly: what you found or \
                 assigned, and when all is done, what was done and what is left. \
                 When you need the person to decide, end with one clear question.",
                a.name
            )
        }
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
                trunk: None,
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
        assert_eq!(a.talk.as_ref().unwrap().until, w.tick + 12);
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

    /// A turn of agent `id` ended at Claude Code's cap, `usd` dollars spent
    /// in the session so far.
    fn capped(live: &Live, id: AgentId, usd: u64) {
        live.tx
            .send((
                id,
                live.generation[id],
                AgentEvent::TurnDone {
                    ok: false,
                    text: CAPPED.into(),
                    cost: usd * 1_000_000,
                    denied: 0,
                },
            ))
            .unwrap();
    }

    #[test]
    fn a_session_that_hit_its_cap_goes_on_in_a_new_one_when_the_budget_grew() {
        let mut w = team(1);
        let mut live = idle_live(2);
        // The leader is mid-prompt in its first session when the cap hits;
        // the person had raised the budget to $100 meanwhile.
        w.daily_budget = 100 * 10_000;
        live.inflight[0] = vec!["hello".into()];
        live.turns[0] = 1;
        capped(&live, 0, 20);
        live.step(&mut w);
        assert_eq!(
            w.agents[0].cost,
            20 * 10_000,
            "the capped turn still counts"
        );
        assert_eq!(live.generation[0], 1, "a new session");
        assert_eq!(live.spent_before[0], 20 * 10_000);
        assert_eq!(
            live.inflight[0],
            vec!["hello".to_string()],
            "same prompt again"
        );
        assert_eq!(live.turns[0], 1);
        assert!(live.renewed[0]);
        assert_ne!(w.agents[0].status, Status::Stopped);
        assert!(
            !w.journal
                .iter()
                .any(|e| matches!(&e.event, Event::Stopped { .. }))
        );

        // The new session hits its cap at once: that is final, the turn
        // ends like any failed one.
        capped(&live, 0, 20);
        live.step(&mut w);
        assert_eq!(live.generation[0], 1, "no third session");
        assert_eq!(live.turns[0], 0);
        assert!(
            w.journal
                .iter()
                .any(|e| matches!(&e.event, Event::Stopped { .. }))
        );

        // A finished turn clears the mark, so a later raise renews again.
        live.inflight[0] = vec!["more".into()];
        live.turns[0] = 1;
        live.tx
            .send((
                0,
                live.generation[0],
                AgentEvent::TurnDone {
                    ok: true,
                    text: "done".into(),
                    cost: 25_000_000,
                    denied: 0,
                },
            ))
            .unwrap();
        live.step(&mut w);
        assert!(!live.renewed[0]);
    }

    #[test]
    fn a_session_that_hit_its_cap_with_the_budget_spent_is_not_renewed() {
        let mut w = team(1);
        let mut live = idle_live(2);
        w.daily_budget = 20 * 10_000;
        live.inflight[0] = vec!["hello".into()];
        live.turns[0] = 1;
        capped(&live, 0, 20);
        live.step(&mut w);
        assert_eq!(live.generation[0], 0, "same session, nothing to renew into");
        assert_eq!(w.agents[0].cost, 20 * 10_000);
        assert!(!live.renewed[0]);
        assert!(live.inflight[0].is_empty());
    }

    fn wait_for(live: &mut Live, w: &mut World, done: impl Fn(&World) -> bool) {
        let start = Instant::now();
        while !done(w) {
            assert!(start.elapsed().as_secs() < 10, "timed out");
            live.step(w);
            thread::sleep(std::time::Duration::from_millis(10));
        }
    }

    /// Claude Code as `program`, on its default models.
    fn claude_at(program: &str) -> Vec<Hireable> {
        vec![Hireable::claude(program, None, &[])]
    }

    #[test]
    fn nobody_is_prompted_once_the_team_has_spent_the_budget() {
        let mut w = team(1);
        let mut live = Live::new(
            2,
            // Never runs while the budget check refuses first.
            claude_at("/nonexistent/claude"),
            PathBuf::from("/nonexistent"),
            PathBuf::from("/nonexistent"),
        );
        // The pot is shared: what the leader spent counts against the worker.
        w.agents[0].cost = w.daily_budget;
        assert_eq!(live.told(&mut w, 1, "go"), Err(Refusal::Budget));
        w.agents[0].cost = 6_000;
        w.agents[1].cost = w.daily_budget - 6_000;
        assert_eq!(live.told(&mut w, 1, "go"), Err(Refusal::Budget));
        assert!(w.cards.is_empty());
        // One ten-thousandth under, and the worker is prompted: its turn
        // is counted and its session is started (off this thread).
        w.agents[1].cost -= 1;
        assert_eq!(live.told(&mut w, 1, "go"), Ok(()));
        assert_eq!(live.turns[1], 1);
    }

    #[test]
    fn a_session_that_can_not_start_stops_the_agent_and_the_next_message_retries() {
        let mut w = team(1);
        let mut live = Live::new(
            2,
            claude_at("/nonexistent/claude"),
            std::env::temp_dir(),
            PathBuf::from("/nonexistent"),
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
            claude_at("/nonexistent/claude"),
            std::env::temp_dir(),
            PathBuf::from("/nonexistent"),
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

    fn finish(live: &mut Live, w: &mut World, id: AgentId, text: &str) {
        live.tx
            .send((
                id,
                0,
                AgentEvent::TurnDone {
                    ok: true,
                    text: text.into(),
                    cost: 0,
                    denied: 0,
                },
            ))
            .unwrap();
        live.step(w);
    }

    fn answers(w: &World) -> Vec<(AgentId, String)> {
        w.open_inbox()
            .iter()
            .map(|&i| &w.inbox[i])
            .filter(|i| i.kind == InboxKind::Reply)
            .map(|i| (i.agent, i.detail.clone()))
            .collect()
    }

    #[test]
    fn an_answer_to_the_person_waits_for_them_until_they_speak() {
        let mut w = team(1);
        let mut live = idle_live(2);
        w.said(0, "check the readme");
        live.asked[0] = true;
        live.turns[0] = 1;
        finish(
            &mut live,
            &mut w,
            0,
            "\nThe readme is one line.\nShall I assign more?",
        );
        assert_eq!(answers(&w), [(0, "The readme is one line.".to_string())]);
        // Speaking to the agent answers it.
        w.said(0, "yes");
        assert!(answers(&w).is_empty());
    }

    #[test]
    fn a_leader_reports_once_the_team_is_quiet() {
        let mut w = team(1);
        let mut live = idle_live(2);
        // The worker still works: the leader's turn on its mail is no report.
        live.turns[1] = 1;
        live.turns[0] = 1;
        finish(&mut live, &mut w, 0, "claude-1 is on it.");
        assert!(answers(&w).is_empty());
        // A worker's own turn is not an answer to the person either.
        finish(&mut live, &mut w, 1, "Done.");
        assert!(answers(&w).is_empty());
        live.turns[0] = 1;
        finish(
            &mut live,
            &mut w,
            0,
            "All done: README has a Usage section.",
        );
        assert_eq!(answers(&w).len(), 1);
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
    fn nobody_gets_waking_mail_once_the_budget_is_spent() {
        let mut w = team(1);
        let mut live = idle_live(2);
        w.agents[0].cost = w.daily_budget;
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
        // The leader's socket does not even offer claim; it alone offers hire.
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
        assert_eq!(names, ["send", "post", "board", "hire"]);
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
        // The session never starts here and may be gone by the next step,
        // so what counts is the bookkeeping, not the queue.
        live.step(&mut w);
        assert_eq!(live.offered[&0], [1], "offered once");
        assert_eq!(live.woken[1], 1);
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
    fn only_workers_get_the_claim_tool_and_only_the_leader_hires() {
        let names = |r: Role| -> Vec<String> {
            tools_of(r, &["claude"])
                .into_iter()
                .map(|t| t.name)
                .collect()
        };
        assert_eq!(names(Role::Leader), ["send", "post", "board", "hire"]);
        assert_eq!(names(Role::Worker), ["send", "post", "board", "claim"]);
        assert_eq!(names(Role::Researcher), ["send", "post", "board"]);
        let hire = &tools_of(Role::Leader, &["claude", "codex"])[3];
        assert_eq!(
            hire.input["properties"]["vendor"]["enum"],
            json!(["claude", "codex"])
        );
        assert_eq!(
            hire.input["properties"]["tier"]["enum"],
            json!(["cheap", "standard", "strong"])
        );
    }

    /// A tool call as the bus delivers it, answered by the next step.
    fn call(live: &mut Live, w: &mut World, from: AgentId, tool: &str, args: Value) -> Answer {
        let (tx, rx) = std::sync::mpsc::channel();
        live.calls_tx.send((from, tool.into(), args, tx)).unwrap();
        live.step(w);
        rx.recv_timeout(std::time::Duration::from_secs(5)).unwrap()
    }

    #[test]
    fn every_tool_reaches_the_bus_through_the_call_channel() {
        let mut w = team(2);
        let mut live = idle_live(3);
        assert_eq!(
            call(&mut live, &mut w, 1, "board", json!({})),
            Ok("the board is empty".into())
        );
        call(
            &mut live,
            &mut w,
            0,
            "post",
            card("Add a test", "for the parser"),
        )
        .expect("posted");
        let open = call(&mut live, &mut w, 1, "board", json!({})).unwrap();
        assert!(open.contains("Open:") && !open.contains("Taken:"), "{open}");
        // Off the board, no panic: 0, and one past the last card.
        for bad in [0, w.cards.len() as u64 + 1] {
            assert!(call(&mut live, &mut w, 1, "claim", json!({ "card": bad })).is_err());
        }
        call(&mut live, &mut w, 1, "claim", json!({ "card": 1 })).expect("claimed");
        let taken = call(&mut live, &mut w, 2, "board", json!({})).unwrap();
        assert!(
            taken.contains("Taken:") && !taken.contains("Open:"),
            "{taken}"
        );
        let unknown = call(&mut live, &mut w, 1, "nothing", json!({}));
        assert!(
            matches!(&unknown, Err(e) if e.contains("no tool")),
            "{unknown:?}"
        );
    }

    #[test]
    fn the_caps_allow_exactly_the_cap_and_not_one_more() {
        let mut w = team(1);
        let mut live = idle_live(2);
        let body = "x".repeat(MAX_BODY);
        assert!(live.send(&mut w, 1, &msg("lead", "inform", &body)).is_ok());
        let over = format!("{body}x");
        assert!(live.send(&mut w, 1, &msg("lead", "inform", &over)).is_err());
        let title = "t".repeat(MAX_TITLE);
        assert!(live.post(&mut w, 0, &card(&title, &body)).is_ok());
        assert!(
            live.post(&mut w, 0, &card(&format!("{title}t"), "b"))
                .is_err()
        );
        assert!(live.post(&mut w, 0, &card("t", &over)).is_err());
    }

    #[test]
    fn a_message_to_oneself_is_refused_and_the_team_listed_leaves_oneself_out() {
        let mut w = team(2);
        let mut live = idle_live(3);
        let got = live.send(&mut w, 1, &msg("claude-1", "inform", "hello me"));
        let Err(why) = got else {
            panic!("sent to itself: {got:?}");
        };
        let (_, team) = why.split_once("the team is ").expect("lists the team");
        assert_eq!(team, "lead, claude-2", "{why}");
    }

    #[test]
    fn the_board_shows_open_and_taken_cards_apart() {
        let mut w = team(2);
        let mut live = idle_live(3);
        live.post(&mut w, 0, &card("First", "one")).unwrap();
        live.step(&mut w);
        live.claim(&mut w, 1, &json!({ "card": 1 })).unwrap();
        live.post(&mut w, 0, &card("Second", "two")).unwrap();
        let both = live.board(&w, 2);
        assert!(
            both.starts_with(
                "Open:
"
            ),
            "{both}"
        );
        assert!(both.contains("Second"));
        assert!(
            both.contains(
                "

Taken:
"
            ),
            "{both}"
        );
        assert!(both.contains("First"));
    }

    #[test]
    fn a_delivery_takes_its_title_from_the_assignment_not_the_first_mail() {
        let mut w = team(1);
        w.agents[1].status = Status::Idle;
        let mut live = idle_live(2);
        live.send(&mut w, 0, &msg("claude-1", "inform", "a note first"))
            .unwrap();
        live.send(
            &mut w,
            0,
            &msg(
                "claude-1",
                "assign",
                "Fix the parser
in p.rs",
            ),
        )
        .unwrap();
        live.step(&mut w);
        let card = w.agents[1].card.expect("a card");
        assert_eq!(w.cards[card].title, "Fix the parser");
    }

    #[test]
    fn the_tick_is_the_time_since_the_start_in_whole_ticks() {
        let mut w = team(1);
        let mut live = idle_live(2);
        live.started = Instant::now() - std::time::Duration::from_secs(13);
        live.step(&mut w);
        assert_eq!(w.tick, 13 / SECONDS_PER_TICK);
    }

    #[test]
    fn a_message_lands_in_the_other_card_and_lifts_both_talk_lines() {
        let mut w = team(1);
        let c = with_card(&mut w, 1);
        let mut live = idle_live(2);
        // The leader has no card; the worker's card keeps the line.
        live.send(&mut w, 0, &msg("claude-1", "inform", "first line\nmore"))
            .unwrap();
        let last = &w.cards[c].story.last().unwrap().event;
        let said = crate::ui::words::said_in_story(1, last).expect("a conversation line");
        assert_eq!((said.from, said.to), (Party::Agent(0), Party::Agent(1)));
        for id in [0, 1] {
            let talk = w.agents[id].talk.as_ref().expect("a talk line");
            assert_eq!(talk.text, "first line");
            assert_eq!(talk.until, w.tick + 8);
            assert_eq!(talk.with, Party::Agent(1 - id));
        }
    }

    #[test]
    fn turns_still_queued_keep_the_agent_working() {
        let mut w = team(1);
        let mut live = idle_live(2);
        live.turns[1] = 2;
        finish(&mut live, &mut w, 1, "one down");
        assert_eq!(live.turns[1], 1);
        assert_eq!(w.agents[1].status, Status::Working);
    }

    #[test]
    fn an_expired_talk_line_goes_and_a_fresh_one_stays() {
        let mut w = team(2);
        let mut live = idle_live(3);
        let talk = |until| Talk {
            with: Party::Person,
            text: "hi".into(),
            until,
        };
        w.agents[1].talk = Some(talk(0));
        w.agents[2].talk = Some(talk(1_000_000));
        live.step(&mut w);
        assert!(w.agents[1].talk.is_none());
        assert!(w.agents[2].talk.is_some());
    }

    #[test]
    fn a_busy_or_worn_out_worker_is_not_offered_a_card() {
        for (turns, woken) in [(1, 0), (0, MAX_WOKEN)] {
            let mut w = team(2);
            let mut live = idle_live(3);
            live.turns[1] = turns;
            live.woken[1] = woken;
            live.post(&mut w, 0, &card("Add a test", "body")).unwrap();
            live.step(&mut w);
            assert!(queued(&live, 1).is_empty(), "turns {turns} woken {woken}");
            assert!(!queued(&live, 2).is_empty(), "turns {turns} woken {woken}");
        }
    }

    #[test]
    fn each_delivery_counts_one_turn_on_messages() {
        let mut w = team(1);
        w.agents[1].status = Status::Idle;
        let mut live = idle_live(2);
        live.send(&mut w, 0, &msg("claude-1", "assign", "do it"))
            .unwrap();
        live.step(&mut w);
        assert_eq!(live.woken[1], 1);
    }

    #[test]
    fn the_prompts_name_the_team() {
        let w = team(2);
        let lead = system_prompt(&w, 0, 4);
        assert!(lead.starts_with("You are lead, the leader"), "{lead}");
        assert!(
            lead.contains("The workers so far are claude-1, claude-2."),
            "{lead}"
        );
        assert!(lead.contains("hire tool, up to 4;"), "{lead}");
        let worker = system_prompt(&w, 2, 4);
        assert!(
            worker.starts_with("You are claude-2, a coding agent"),
            "{worker}"
        );
        assert!(worker.contains("led by lead"));
        assert!(!worker.contains("hire"), "{worker}");
        let alone = system_prompt(&team(0), 0, 4);
        assert!(alone.contains("You have no workers yet."), "{alone}");
        let none = system_prompt(&team(0), 0, 0);
        assert!(none.contains("without workers"), "{none}");
        assert!(!none.contains("hire"), "{none}");
    }

    #[test]
    fn the_leader_hires_a_worker_named_after_its_vendor_with_the_tier_s_model() {
        let mut w = team(0);
        let mut live = idle_live(1);
        live.max_workers = 2;
        let got = live.hire(&mut w, 0, &json!({ "tier": "cheap" })).unwrap();
        assert!(
            got.starts_with("claude-1 joined (claude, haiku).")
                && got.contains("1 more may be hired"),
            "{got}"
        );
        let a = &w.agents[1];
        assert_eq!(
            (a.name.as_str(), a.vendor, a.role),
            ("claude-1", Vendor::Claude, Role::Worker)
        );
        assert_eq!(a.status, Status::Idle);
        assert_eq!(a.project, 0);
        assert_eq!(live.models[1].as_deref(), Some("haiku"));
        assert!(live.tiered[1]);
        assert_eq!(live.slots.len(), 2, "every per-agent list grew");
        assert_eq!(live.mail.len(), 2);
        assert!(matches!(
            &w.journal.last().unwrap().event,
            Event::Hired { by: 0, agent: 1, model: Some(m) } if m == "haiku"
        ));
        assert!(
            w.agents[0]
                .trace
                .iter()
                .any(|l| l.contains("hire claude-1 cheap haiku"))
        );
        // The default tier is the vendor's own model, which has nothing to
        // fall back to.
        let got = live.hire(&mut w, 0, &json!({})).unwrap();
        assert!(
            got.starts_with("claude-2 joined (claude, the default model)."),
            "{got}"
        );
        assert!(got.contains("0 more may be hired"), "{got}");
        assert_eq!(live.models[2], None);
        assert!(!live.tiered[2]);
        // Exactly the cap, not one more.
        let err = live.hire(&mut w, 0, &json!({})).unwrap_err();
        assert!(
            err.contains("full: 2 workers") && err.contains("--agents 2"),
            "{err}"
        );
        assert_eq!(w.agents.len(), 3);
        // A worker is given to the leader by send; it is told so.
        live.send(&mut w, 0, &msg("claude-2", "assign", "Add a test"))
            .unwrap();
    }

    #[test]
    fn a_new_worker_s_name_skips_the_ones_taken() {
        let mut w = team(2);
        let mut live = idle_live(3);
        let got = live.hire(&mut w, 0, &json!({ "tier": "strong" })).unwrap();
        assert!(got.starts_with("claude-3 joined (claude, opus)."), "{got}");
        // A name that is not the count: the count is skipped over it.
        w.agents[3].name = "claude-4".into();
        let got = live.hire(&mut w, 0, &json!({})).unwrap();
        assert!(got.starts_with("claude-5 joined"), "{got}");
        assert_eq!(w.agent_by_name("claude-5"), Some(4));
    }

    #[test]
    fn bad_hires_are_refused_with_a_reason() {
        let mut w = team(1);
        let mut live = idle_live(2);
        let err = live.hire(&mut w, 1, &json!({})).unwrap_err();
        assert!(err.contains("only the leader"), "{err}");
        let err = live
            .hire(&mut w, 0, &json!({ "tier": "huge" }))
            .unwrap_err();
        assert!(err.contains("cheap, standard or strong"), "{err}");
        let err = live
            .hire(&mut w, 0, &json!({ "vendor": "codex" }))
            .unwrap_err();
        assert!(
            err.contains("no vendor \"codex\"") && err.contains("hire claude"),
            "{err}"
        );
        live.max_workers = 0;
        let err = live.hire(&mut w, 0, &json!({})).unwrap_err();
        assert!(err.contains("without workers"), "{err}");
        assert_eq!(w.agents.len(), 2);
        assert!(w.journal.is_empty());
    }

    #[test]
    fn a_hired_worker_gets_a_socket_with_the_worker_s_tools() {
        use std::io::{BufRead, BufReader, Write};
        use std::os::unix::net::UnixStream;
        let mut w = team(0);
        let mut live = idle_live(1);
        live.open_bus(&w, "/opt/imece".into()).unwrap();
        live.hire(&mut w, 0, &json!({})).unwrap();
        let path = live.bus.as_ref().unwrap().servers[1].path().to_path_buf();
        assert!(path.ends_with("a1.sock"), "{}", path.display());
        let mut sock = UnixStream::connect(path).unwrap();
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
        assert_eq!(names, ["send", "post", "board", "claim"]);
    }

    #[test]
    fn a_tier_model_that_fails_its_first_turn_gives_way_to_the_standard_one() {
        let mut w = team(0);
        let mut live = idle_live(1);
        live.vendors[0].ladder.standard = Some("sonnet".into());
        live.hire(&mut w, 0, &json!({ "tier": "strong" })).unwrap();
        assert_eq!(live.models[1].as_deref(), Some("opus"));
        live.send(&mut w, 0, &msg("claude-1", "assign", "Fix it"))
            .unwrap();
        live.step(&mut w);
        assert_eq!(live.inflight[1].len(), 1);
        let card = w.agents[1].card.expect("a card");
        let failed = |cost| AgentEvent::TurnDone {
            ok: false,
            text: "error_api".into(),
            cost,
            denied: 0,
        };
        live.tx.send((1, 0, failed(0))).unwrap();
        live.step(&mut w);
        assert_eq!(live.models[1].as_deref(), Some("sonnet"));
        assert!(!live.tiered[1]);
        assert_eq!(live.generation[1], 1, "a new session");
        assert_eq!(w.agents[1].status, Status::Working, "still on it");
        assert_eq!(w.agents[1].card, Some(card), "the same card");
        assert!(
            matches!(&live.slots[1], Slot::Starting(q) if q.len() == 1 && q[0].contains("Fix it"))
        );
        assert_eq!(live.turns[1], 1);
        assert!(matches!(
            &w.journal.last().unwrap().event,
            Event::Fallback { agent: 1, model } if model == "opus"
        ));
        assert!(
            !w.journal
                .iter()
                .any(|j| matches!(j.event, Event::Stopped { .. }))
        );
        // Once is enough: the standard model failing is a failure.
        live.tx.send((1, 1, failed(0))).unwrap();
        live.step(&mut w);
        assert_eq!(live.generation[1], 1);
        assert_eq!(w.agents[1].status, Status::Idle);
        assert!(matches!(
            &w.journal.last().unwrap().event,
            Event::Stopped { agent: 1, .. }
        ));
    }

    #[test]
    fn only_a_first_turn_that_cost_nothing_falls_back() {
        for (turned, cost) in [(true, 0), (false, 5)] {
            let mut w = team(0);
            let mut live = idle_live(1);
            live.hire(&mut w, 0, &json!({ "tier": "cheap" })).unwrap();
            live.turns[1] = 1;
            live.turned[1] = turned;
            live.tx
                .send((
                    1,
                    0,
                    AgentEvent::TurnDone {
                        ok: false,
                        text: "error".into(),
                        cost,
                        denied: 0,
                    },
                ))
                .unwrap();
            live.step(&mut w);
            assert_eq!(
                live.models[1].as_deref(),
                Some("haiku"),
                "turned {turned} cost {cost}"
            );
            assert!(live.tiered[1]);
            assert!(live.turned[1]);
            assert!(matches!(
                &w.journal.last().unwrap().event,
                Event::Stopped { agent: 1, .. }
            ));
        }
        // A turn that went well is a turn too: nothing to fall back from later.
        let mut w = team(0);
        let mut live = idle_live(1);
        live.hire(&mut w, 0, &json!({ "tier": "cheap" })).unwrap();
        live.turns[1] = 1;
        finish(&mut live, &mut w, 1, "done");
        assert!(live.turned[1]);
        assert!(live.inflight[1].is_empty());
    }

    #[test]
    fn the_ladder_is_the_vendor_s_unless_the_person_says_otherwise() {
        let h = Hireable::claude("claude", None, &[]);
        assert_eq!(h.ladder.model(Tier::Cheap), Some("haiku"));
        assert_eq!(h.ladder.model(Tier::Standard), None);
        assert_eq!(h.ladder.model(Tier::Strong), Some("opus"));
        assert_eq!(h.program, "claude");
        assert_eq!(h.vendor, Vendor::Claude);
        let h = Hireable::claude(
            "claude",
            Some("sonnet".into()),
            &[
                (Tier::Strong, "opus[1m]".into()),
                (Tier::Cheap, "sonnet".into()),
            ],
        );
        assert_eq!(h.ladder.model(Tier::Cheap), Some("sonnet"));
        assert_eq!(h.ladder.model(Tier::Standard), Some("sonnet"));
        assert_eq!(h.ladder.model(Tier::Strong), Some("opus[1m]"));
        let h = Hireable::claude("claude", None, &[(Tier::Standard, "opus".into())]);
        assert_eq!(h.ladder.model(Tier::Standard), Some("opus"));
    }

    #[test]
    fn the_team_needs_a_leader_imece_can_drive() {
        let err = hireable(&[], Vendor::Claude).unwrap_err();
        assert!(
            err.contains("no coding agent") && err.contains("claude, codex, gemini"),
            "{err}"
        );
        let err = hireable(&[(Vendor::Codex, false)], Vendor::Claude).unwrap_err();
        assert!(
            err.contains("claude is not on PATH (found codex)") && err.contains("--lead"),
            "{err}"
        );
        let err = hireable(
            &[(Vendor::Claude, true), (Vendor::Codex, false)],
            Vendor::Codex,
        )
        .unwrap_err();
        assert!(
            err.contains("codex is on PATH, but") && err.contains("--lead claude"),
            "{err}"
        );
        let ok = hireable(
            &[(Vendor::Claude, true), (Vendor::Codex, false)],
            Vendor::Claude,
        )
        .unwrap();
        assert_eq!(
            ok,
            [Vendor::Claude],
            "only vendors imece drives are hireable"
        );
    }

    #[test]
    fn hire_reaches_the_bus_through_the_call_channel() {
        let mut w = team(0);
        let mut live = idle_live(1);
        let got = call(&mut live, &mut w, 0, "hire", json!({ "tier": "strong" })).unwrap();
        assert!(got.starts_with("claude-1 joined (claude, opus)"), "{got}");
        assert_eq!(w.agents.len(), 2);
    }

    #[test]
    fn hiring_and_falling_back_are_worded_in_both_languages() {
        use crate::i18n::Lang;
        use crate::ui::words::event;
        let mut w = team(0);
        let mut live = idle_live(1);
        live.hire(&mut w, 0, &json!({ "tier": "cheap" })).unwrap();
        live.hire(&mut w, 0, &json!({})).unwrap();
        let hired: Vec<&Event> = w.journal.iter().map(|j| &j.event).collect();
        assert_eq!(event(Lang::En, &w, hired[0]), "lead hired claude-1 (haiku)");
        assert_eq!(
            event(Lang::Tr, &w, hired[0]),
            "lead işe aldı: claude-1 (haiku)"
        );
        assert_eq!(
            event(Lang::En, &w, hired[1]),
            "lead hired claude-2 (default model)"
        );
        assert_eq!(
            event(Lang::Tr, &w, hired[1]),
            "lead işe aldı: claude-2 (varsayılan model)"
        );
        let fell = Event::Fallback {
            agent: 1,
            model: "haiku".into(),
        };
        assert_eq!(
            event(Lang::En, &w, &fell),
            "claude-1: model haiku is not available here, using the default"
        );
        assert_eq!(
            event(Lang::Tr, &w, &fell),
            "claude-1: haiku modeli burada yok, varsayılanla devam"
        );
        assert_eq!(hired[0].kind(), EntryKind::Plan);
        assert_eq!(fell.kind(), EntryKind::Assumption);
        assert_eq!((hired[0].agent(), fell.agent()), (1, 1));
    }

    /// A stand-in for `claude` that writes its arguments, one per line, to
    /// `args.txt` in its directory and exits.
    fn fake_claude(dir: &Path) -> String {
        use std::os::unix::fs::PermissionsExt;
        let path = dir.join("fake-claude");
        std::fs::write(&path, "#!/bin/sh\nprintf '%s\\n' \"$@\" > args.txt\n").unwrap();
        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o700)).unwrap();
        path.to_string_lossy().into_owned()
    }

    #[test]
    fn a_session_starts_with_the_agent_s_model_what_is_left_of_the_budget_and_prompt() {
        let dir = std::env::temp_dir().join(format!("imece-start-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let mut w = team(0);
        let mut live = Live::new(
            1,
            claude_at(&fake_claude(&dir)),
            dir.clone(),
            dir.join("worktrees"),
        );
        live.max_workers = 3;
        live.models[0] = Some("sonnet".into());
        w.agents[0].cost = 1_000;
        live.told(&mut w, 0, "hello").unwrap();
        // The stand-in exits at once, which ends the session.
        wait_for(&mut live, &mut w, |w| w.agents[0].status == Status::Stopped);
        let args = std::fs::read_to_string(dir.join("args.txt")).unwrap();
        let args: Vec<&str> = args.lines().collect();
        let after = |f: &str| args[args.iter().position(|a| *a == f).unwrap() + 1];
        assert_eq!(after("--model"), "sonnet");
        // What is left of the budget, in dollars: (10000 - 1000) ten-thousandths.
        assert_eq!(after("--max-budget-usd"), "0.900000");
        assert_eq!(after("--tools"), "Glob,Grep,Read");
        assert_eq!(after("--name"), "imece lead");
        assert!(after("--append-system-prompt").contains("hire tool, up to 3;"));
        assert!(!args.contains(&"--mcp-config"), "no bus, no MCP server");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn a_program_is_on_path_when_its_version_runs() {
        let here = std::env::temp_dir();
        assert!(on_path("true", &here));
        assert!(!on_path("false", &here));
        assert!(!on_path("/nonexistent/claude", &here));
    }

    #[test]
    fn the_known_vendors_are_the_ones_the_leader_may_be() {
        for (vendor, program, _) in KNOWN {
            assert_eq!(Vendor::parse(program), Some(*vendor));
            assert_eq!(vendor.label(), *program);
        }
        assert_eq!(Vendor::parse("qwen"), None);
        assert!(
            KNOWN
                .iter()
                .any(|(v, _, drives)| *v == Vendor::Claude && *drives)
        );
    }

    #[test]
    fn fnv_is_fnv_1a() {
        assert_eq!(fnv(b""), 0x811c_9dc5);
        assert_eq!(fnv(b"a"), 0xe40c_292c);
        assert_ne!(fnv(b"a"), fnv(b"b"));
    }

    #[test]
    fn the_clock_starts_at_the_local_time() {
        let out = std::process::Command::new("date")
            .arg("+%H %M %S")
            .output()
            .unwrap();
        let p: Vec<u64> = String::from_utf8_lossy(&out.stdout)
            .split_whitespace()
            .map(|n| n.parse().unwrap())
            .collect();
        let expect = p[0] * 3600 + p[1] * 60 + p[2];
        let got = local_seconds_after_midnight();
        let apart = got.abs_diff(expect).min(86_400 - got.abs_diff(expect));
        assert!(apart <= 2, "{got} vs {expect}");
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
