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

use std::collections::HashMap;
use std::io;
use std::path::{Path, PathBuf};
use std::sync::mpsc::{self, Receiver, Sender};
use std::thread;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use imece_agent::mcp::{Answer, Server, Tool};
use imece_agent::{
    Agent as Session, Event as AgentEvent, Mcp, Tier, child, claude, codex, gemini, run, worktree,
};
use serde_json::{Value, json};

use crate::i18n::{Lang, clean};
use crate::model::*;
use crate::store::{self, Found, Kept, Store};
use crate::ui::words;

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
    /// `vendor` at `program`, on its own ladder, with the person's
    /// stand-ins `tiers` (for this vendor) on top.
    fn new(vendor: Vendor, program: &str, tiers: &[(Tier, String)]) -> Hireable {
        let own = |tier| {
            match vendor {
                Vendor::Codex => codex::model(tier),
                Vendor::Gemini => gemini::model(tier),
                _ => claude::model(tier),
            }
            .map(String::from)
        };
        let mut ladder = Ladder {
            cheap: own(Tier::Cheap),
            standard: own(Tier::Standard),
            strong: own(Tier::Strong),
        };
        for (tier, model) in tiers {
            ladder.set(*tier, model.clone());
        }
        Hireable {
            vendor,
            program: program.to_string(),
            ladder,
        }
    }

    /// Claude Code at `program`, with `tiers` on top of its own ladder.
    #[cfg(test)]
    fn claude(program: &str, tiers: &[(Tier, String)]) -> Hireable {
        Hireable::new(Vendor::Claude, program, tiers)
    }

    /// Codex at `program`, with `tiers` on top of its own ladder.
    #[cfg(test)]
    fn codex(program: &str, tiers: &[(Tier, String)]) -> Hireable {
        Hireable::new(Vendor::Codex, program, tiers)
    }

    /// Gemini CLI at `program`, with `tiers` on top of its own ladder.
    #[cfg(test)]
    fn gemini(program: &str, tiers: &[(Tier, String)]) -> Hireable {
        Hireable::new(Vendor::Gemini, program, tiers)
    }
}

/// The hireable `vendors`, each at its program from [`KNOWN`] on its own
/// ladder, with the person's stand-ins `tiers` on top: the ones named for
/// the vendor, and the bare ones for the leader's vendor `lead`.
fn team_vendors(
    vendors: Vec<Vendor>,
    lead: Vendor,
    tiers: &[(Option<Vendor>, Tier, String)],
) -> Vec<Hireable> {
    vendors
        .into_iter()
        .map(|v| {
            let program = KNOWN.iter().find(|k| k.0 == v).map_or("", |k| k.1);
            let own: Vec<(Tier, String)> = tiers
                .iter()
                .filter(|(for_, _, _)| for_.map_or(v == lead, |f| f == v))
                .map(|(_, t, m)| (*t, m.clone()))
                .collect();
            Hireable::new(v, program, &own)
        })
        .collect()
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
        Some((_, false)) => {
            let could: Vec<&str> = present
                .iter()
                .filter(|(_, drives)| *drives)
                .map(|(v, _)| v.label())
                .collect();
            Err(format!(
                "{} is on PATH, but imece can not drive it yet; start with --lead {}",
                lead.label(),
                if could.is_empty() {
                    "claude".to_string()
                } else {
                    could.join(" or --lead ")
                }
            ))
        }
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
    /// The agent's session took no such command: why.
    Command(String),
}

/// A project's checkout and where its workers' worktrees live.
struct Place {
    repo: PathBuf,
    root: PathBuf,
}

/// Where the worktrees of the checkout `repo` live: under `state`, named
/// after the checkout and a hash of its path, so two checkouts with one
/// name do not share.
fn root_for(state: &Path, repo: &Path) -> PathBuf {
    state.join("worktrees").join(format!(
        "{}-{:08x}",
        dir_name(repo),
        fnv(repo.as_os_str().as_encoded_bytes())
    ))
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
/// why it could not start.
type Ready = Result<(Box<dyn Session>, PathBuf, Option<worktree::Placed>), String>;

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
/// Cargo commands nobody runs through imece: they change what the project
/// depends on or reach outside it, which is the person's call.
const CARGO_DENIED: &[&str] = &[
    "add",
    "install",
    "login",
    "logout",
    "owner",
    "publish",
    "remove",
    "rm",
    "uninstall",
    "yank",
];
/// Cargo commands a leader may not run either: they change files.
const CARGO_WRITES: &[&str] = &["clean", "fix", "fmt", "init", "new"];
/// Git commands that only read.
const GIT_READ: &[&str] = &[
    "blame",
    "diff",
    "grep",
    "log",
    "ls-files",
    "rev-parse",
    "show",
    "status",
];

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
}

/// How long a tool call waits for imece's answer: a run is answered when
/// the program ends, not by the screen's loop, so it waits for the run's
/// deadline and a little more; the rest are answered within a step.
fn call_wait(tool: &str, run_timeout: Duration) -> Duration {
    if tool == "run" {
        run_timeout + CALL_WAIT
    } else {
        CALL_WAIT
    }
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
    let (mut w, found) = match loaded {
        Some(l) => (l.world, Some((l.found, l.skipped))),
        None => (
            World {
                tick: 0,
                started_at: now,
                run_tick: 0,
                run_spent: 0,
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
    w.run_tick = w.tick;
    w.run_spent = w.spent();
    let mut live = Live::bare(vendors, o.lead);
    live.state = o.state;
    live.max_workers = o.max_workers;
    live.max_helpers = o.max_helpers;
    live.run_programs = o.run_programs;
    live.run_timeout = o.run_timeout;
    live.fresh = o.fresh;
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

    /// A store in memory, for tests of what saving does on the screen.
    #[cfg(test)]
    pub(crate) fn store_in_memory(&mut self) {
        self.store = Some(Store::in_memory());
    }

    /// Breaks the store so the next save fails, for tests.
    #[cfg(test)]
    pub(crate) fn break_store(&mut self) {
        if let Some(s) = &self.store {
            s.raw().execute_batch("DROP TABLE agents").unwrap();
        }
    }

    /// Takes in what the store remembers of earlier runs, beside the world
    /// it loaded: every agent sleeps (no session, nothing spent until
    /// something wakes it), its mail, notes and offers wait, a turn that
    /// was cut short is marked on its card and told at its wake. A project
    /// whose checkout is gone closes. `skipped` lines of the journal or a
    /// story could not be read.
    fn adopt(&mut self, w: &mut World, found: Found, skipped: usize) {
        for repo in &found.repos {
            let root = root_for(&self.state, repo);
            self.places.push(Place {
                repo: repo.clone(),
                root,
            });
        }
        for id in 0..w.agents.len() {
            self.grow(
                found.models.get(id).cloned().flatten(),
                found.tiered.get(id).copied().unwrap_or(false),
            );
            self.spent_before[id] = w.agents[id].cost;
            self.mail[id] = found.mail.get(id).cloned().unwrap_or_default();
            self.notes[id] = found.notes.get(id).cloned().unwrap_or_default();
            self.woken[id] = found.woken.get(id).copied().unwrap_or(0);
            self.posted[id] = found.posted.get(id).copied().unwrap_or(0);
            self.sessions[id] = found.sessions.get(id).cloned().flatten();
            if w.agents[id].gone {
                continue;
            }
            let t = w.tick;
            w.agents[id].set_status(Status::Asleep, t);
            self.remembered[id] = true;
            let cut = found.inflight.get(id).cloned().unwrap_or_default();
            if !cut.is_empty() {
                self.interrupted[id] = cut;
                let card = w.agents[id].card;
                w.agents[id].trace(w.tick, "turn cut short when imece closed");
                w.story(card, StoryEvent::Interrupted);
                w.note(Event::Interrupted { agent: id });
            }
        }
        if self.fresh {
            for id in 0..w.agents.len() {
                if self.sessions[id].is_none() {
                    continue;
                }
                w.agents[id].trace(
                    w.tick,
                    "started with --fresh: the session is forgotten, a new one is told where \
                     it left off",
                );
                self.forget(w, id, false, false);
                if !w.agents[id].gone {
                    w.note(Event::Forgotten { agent: id });
                }
            }
        }
        self.posts = found.posts;
        self.offered = found.offered;
        for (&id, c) in &found.changes {
            let title = w
                .inbox
                .iter()
                .rev()
                .find(|i| i.kind == InboxKind::Change && i.agent == id && i.open)
                .map(|i| i.detail.clone())
                .unwrap_or_default();
            w.changes.push(Change {
                agent: id,
                title,
                files: c
                    .files
                    .iter()
                    .map(|f| ChangedFile {
                        path: f.path.clone(),
                        added: f.added,
                        removed: f.removed,
                        binary: f.binary,
                    })
                    .collect(),
                patch: c.patch.clone(),
            });
        }
        self.changes = found.changes;
        self.loaded_cards = w.cards.len();
        let gone: Vec<ProjectId> = w
            .open_projects()
            .filter(|q| {
                self.places
                    .get(q.id)
                    .is_none_or(|pl| worktree::top(&pl.repo).is_err())
            })
            .map(|q| q.id)
            .collect();
        for p in gone {
            self.posts.retain(|c, _| w.cards[*c].project != p);
            self.offered.retain(|c, _| w.cards[*c].project != p);
            self.changes.retain(|a, _| w.agents[*a].project != p);
            w.close_project(p);
        }
        if skipped > 0 {
            let lead = w.projects.first().map(|p| p.leader);
            if let Some(lead) = lead {
                w.agents[lead].trace(
                    w.tick,
                    format!("store: {skipped} lines were written by another imece and skipped"),
                );
            }
        }
    }

    /// Saves what moved since the last save. Says whether anything was
    /// written; a failure is the store's words.
    pub fn persist(&mut self, w: &World) -> Result<bool, String> {
        let Some(store) = &mut self.store else {
            return Ok(false);
        };
        let kept = Kept {
            repos: self.places.iter().map(|pl| pl.repo.as_path()).collect(),
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
        };
        store.save(w, &kept).map_err(|e| e.to_string())
    }

    /// Tells a remembered agent where it left off, ahead of its first
    /// prompt this run. A session that goes on (`resumed`) knows its own
    /// story and hears only what happened since: a turn that was cut
    /// short, mail that waits. A new session knows nothing of the old
    /// one and is told from what the world kept (its card's story, the
    /// cut turn, mail, its worktree's changes, the board). Every line
    /// cleaned and the whole capped.
    fn brief(&mut self, w: &World, id: AgentId, resumed: bool) -> String {
        let a = &w.agents[id];
        let mut lines = vec![if resumed {
            "imece started again since your last turn; this session goes on where it was, \
             so you remember everything up to then. Since then:"
                .to_string()
        } else {
            "imece started again since your last turn, so this is a new session that \
             remembers nothing of the old one. Where you left off:"
                .to_string()
        }];
        let mut line = |text: String| {
            let text = clean(text.lines().next().unwrap_or_default());
            lines.push(format!("- {}", cut(&text, MAX_BRIEFING_LINE)));
        };
        if resumed {
            // Kept until the session confirms it is the old one; a new
            // one is briefed in full.
            for text in self.interrupted[id].clone() {
                line(format!(
                    "A turn of yours was cut short when imece closed, before it ended; it is \
                     not run again. Its prompt was: {text}"
                ));
            }
            if !self.mail[id].is_empty() {
                line(format!(
                    "{} messages wait for you; they come with a turn of their own.",
                    self.mail[id].len()
                ));
            }
            drop(line);
            if lines.len() == 1 {
                lines.push("- Nothing; go on.".to_string());
            }
            return cut(&lines.join("\n"), MAX_BRIEFING).to_string();
        }
        if let Some(c) = a.card {
            let card = &w.cards[c];
            let state = match card.column {
                Column::Done => "done",
                _ => "not finished",
            };
            line(format!(
                "Your task: #{} {} ({state}). Its story, latest last:",
                c + 1,
                card.title
            ));
            let skip = card.story.len().saturating_sub(12);
            for s in card.story.iter().skip(skip) {
                let text = match &s.event {
                    StoryEvent::Sent {
                        from,
                        to,
                        kind,
                        text,
                        ..
                    } => format!(
                        "{} → {} ({}): {text}",
                        w.agents[*from].name,
                        w.agents[*to].name,
                        kind.wire()
                    ),
                    StoryEvent::Asked { from, to, text } => {
                        format!(
                            "{} asked {}: {text}",
                            w.agents[*from].name, w.agents[*to].name
                        )
                    }
                    StoryEvent::Answered { from, text } => {
                        format!("{} answered: {text}", w.agents[*from].name)
                    }
                    StoryEvent::YouSaid(text) => format!("the person → you: {text}"),
                    StoryEvent::Replied(text) => format!("you answered: {text}"),
                    e => words::story(Lang::En, w, card, e),
                };
                if !text.is_empty() {
                    line(format!("  {} {text}", clock(s.tick)));
                }
            }
        } else {
            line("You had no task in hand.".to_string());
        }
        for text in std::mem::take(&mut self.interrupted[id]) {
            line(format!(
                "A turn of yours was cut short when imece closed, before it ended; it is \
                 not run again. Its prompt was: {text}"
            ));
        }
        if !self.mail[id].is_empty() {
            line(format!(
                "{} messages wait for you; they come with a turn of their own.",
                self.mail[id].len()
            ));
        }
        if a.role != Role::Leader {
            let tree = a.parent.map_or(&a.name, |p| &w.agents[p].name);
            let path = self.places[a.project].root.join(tree);
            if path.is_dir()
                && let Ok(Some(c)) = worktree::change(&path, MAX_PATCH)
            {
                let (added, removed) = c.counts();
                line(format!(
                    "Your worktree holds changes the person has not applied: {} files, \
                     +{added} −{removed}: {}",
                    c.files.len(),
                    c.files
                        .iter()
                        .take(20)
                        .map(|f| f.path.as_str())
                        .collect::<Vec<_>>()
                        .join(", ")
                ));
            }
        }
        let open: Vec<CardId> = self.open(w, a.project).collect();
        if !open.is_empty() {
            line(format!("The board has {} open cards:", open.len()));
            for c in open.iter().take(10) {
                line(format!(
                    "  #{} {} (posted by {})",
                    c + 1,
                    w.cards[*c].title,
                    w.agents[self.posts[c].by].name
                ));
            }
        }
        cut(&lines.join("\n"), MAX_BRIEFING).to_string()
    }

    /// No projects and no agents yet; every leader will run on `lead`.
    fn bare(vendors: Vec<Hireable>, lead: Vendor) -> Live {
        let (tx, rx) = mpsc::channel();
        let (ready_tx, ready_rx) = mpsc::channel();
        let (calls_tx, calls_rx) = mpsc::channel();
        Live {
            state: std::env::temp_dir(),
            store: None,
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
        }
    }

    /// Makes room for one more agent, on `model` (`tiered`: picked by
    /// tier, so it may fall back).
    fn grow(&mut self, model: Option<String>, tiered: bool) {
        self.remembered.push(false);
        self.interrupted.push(Vec::new());
        self.sessions.push(None);
        self.floor.push(0);
        self.resuming.push(None);
        self.slots.push(Slot::None);
        self.generation.push(0);
        self.spent_before.push(0);
        self.models.push(model);
        self.tiered.push(tiered);
        self.turned.push(false);
        self.inflight.push(Vec::new());
        self.renewed.push(false);
        self.leaving.push(false);
        self.notes.push(Vec::new());
        self.mail.push(Vec::new());
        self.turns.push(0);
        self.asked.push(false);
        self.woken.push(0);
        self.posted.push(0);
        self.cwds.push(None);
        self.running.push(run::Running::default());
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
            self.bind(id, w.agents[id].role)?;
        }
        Ok(())
    }

    /// Binds the socket of agent `id`, the next one to get a socket, with
    /// the tools of `role`. Without a bus (before it opens) there is
    /// nothing to bind.
    fn bind(&mut self, id: AgentId, role: Role) -> io::Result<()> {
        let tools = tools_of(
            role,
            &self.vendor_names(),
            &self.run_programs,
            self.run_timeout,
        );
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

    /// Agent `from` asked to run a command: checked here, run on a thread
    /// of its own in the agent's directory, answered when it ends. The
    /// trace gets the command and how it ended.
    fn run(&mut self, w: &mut World, from: AgentId, args: &Value, reply: Sender<Answer>) {
        let line = args["command"].as_str().unwrap_or_default().trim();
        let checked = (|| {
            if line.is_empty() {
                return Err("command: what to run, a program and its arguments".to_string());
            }
            if line.chars().count() > MAX_COMMAND {
                return Err(format!(
                    "the command is longer than {MAX_COMMAND} characters"
                ));
            }
            let argv = run::split(line)?;
            allowed(w.agents[from].role, &argv, &self.run_programs)?;
            let cwd = self.cwds[from]
                .clone()
                .ok_or_else(|| "the session has no directory yet".to_string())?;
            Ok((argv, cwd))
        })();
        let (argv, cwd) = match checked {
            Ok(c) => c,
            Err(e) => {
                w.agents[from].trace(w.tick, format!("run refused: {e}"));
                let _ = reply.send(Err(e));
                return;
            }
        };
        let line = line.to_string();
        w.agents[from].trace(w.tick, format!("run: {line}"));
        let (tx, generation) = (self.tx.clone(), self.generation[from]);
        let (running, timeout) = (self.running[from].clone(), self.run_timeout);
        thread::spawn(move || {
            let mut pgid = None;
            let outcome = run::run(&argv, &cwd, timeout, MAX_RUN_OUTPUT, |p| {
                running.add(p);
                pgid = Some(p);
            });
            if let Some(p) = pgid {
                running.remove(p);
            }
            let (note, answer) = match outcome {
                Ok(o) => (
                    format!("run: {line}: {}", run_ended(&o)),
                    Ok(run_answer(&o)),
                ),
                Err(e) => (
                    format!("run: {line}: could not start: {e}"),
                    Err(format!("could not start {}: {e}", argv[0])),
                ),
            };
            let _ = tx.send((from, generation, AgentEvent::Note(note)));
            let _ = reply.send(answer);
        });
    }

    /// A session ended: the next message starts a new one.
    fn end(&mut self, w: &World, id: AgentId) {
        self.spent_before[id] = w.agents[id].cost;
        self.slots[id] = Slot::None;
        self.cwds[id] = None;
        self.generation[id] += 1;
        self.turns[id] = 0;
        self.turned[id] = false;
        self.inflight[id].clear();
        self.floor[id] = 0;
        self.resuming[id] = None;
    }

    /// Agent `id`'s session is forgotten: its next prompt starts a new
    /// one, and what the vendor kept of the old one goes (Claude's
    /// transcript, Codex's thread, Gemini's chat), on a thread of its own
    /// since a vendor may take a process of its own to do it; the trace
    /// says how it went. `settle` first gives the session's process, just
    /// asked to stop, time to be gone. `whole` takes everything the
    /// vendor kept for the agent (Gemini's private directory too): the
    /// agent is leaving.
    fn forget(&mut self, w: &World, id: AgentId, whole: bool, settle: bool) {
        let session = self.sessions[id].take().map(|r| r.session);
        let a = &w.agents[id];
        let vendor = a.vendor;
        if session.is_none() && !(whole && vendor == Vendor::Gemini) {
            return;
        }
        let program = self.of(vendor).program.clone();
        let cwd = self.cwd_of(w, id);
        let claude_home = self.claude_home.clone();
        let gemini_config = self.gemini_config.clone();
        let gemini_home = self.gemini_home(a.project, a.role, &a.name);
        let wait = if settle { self.settle } else { Duration::ZERO };
        let tx = self.tx.clone();
        let generation = self.generation[id];
        thread::spawn(move || {
            thread::sleep(wait);
            let kept = match vendor {
                Vendor::Claude => "its transcript",
                Vendor::Codex => "its thread",
                _ => "its chat",
            };
            let done: io::Result<String> = match (vendor, &session) {
                (Vendor::Claude, Some(s)) => claude::forget(&claude_home, &cwd, s).map(|was| {
                    if was {
                        "its transcript was removed".to_string()
                    } else {
                        "it had no transcript left".to_string()
                    }
                }),
                (Vendor::Codex, Some(s)) => {
                    codex::delete(&program, s).map(|()| "codex deleted its thread".to_string())
                }
                (Vendor::Gemini, Some(s)) if !whole => {
                    gemini::forget(&gemini_config, &gemini_home, s)
                        .map(|n| format!("{n} chat files were removed"))
                }
                (Vendor::Gemini, _) => gemini::forget_all(&gemini_config, &gemini_home)
                    .and_then(|was| match std::fs::remove_dir_all(&gemini_home) {
                        Ok(()) => Ok(was),
                        Err(e) if e.kind() == io::ErrorKind::NotFound => Ok(was),
                        Err(e) => Err(e),
                    })
                    .map(|was| {
                        if was {
                            "its chats and private directory were removed".to_string()
                        } else {
                            "it had no chats left".to_string()
                        }
                    }),
                _ => return,
            };
            let session = session.unwrap_or_default();
            let note = match done {
                Ok(what) if session.is_empty() => format!("forgotten: {what}"),
                Ok(what) => format!("forgot session {session}: {what}"),
                Err(e) => format!("could not remove {kept} of session {session}: {e}"),
            };
            let _ = tx.send((id, generation, AgentEvent::Note(note)));
        });
    }

    /// The person made agent `id` forget its session (`:forget`): the one
    /// running ends (its unfinished prompts are told at the wake, not run
    /// again), the one remembered goes with what its vendor kept, and the
    /// next message starts a new session, told where it left off. An
    /// agent with no session to forget is refused.
    pub fn forget_agent(&mut self, w: &mut World, id: AgentId) -> Result<(), Refusal> {
        let running = matches!(self.slots[id], Slot::Running(_));
        if w.agents[id].gone || (!running && self.sessions[id].is_none()) {
            return Err(Refusal::Command("it has no session to forget".into()));
        }
        self.running[id].kill_all();
        if let Slot::Running(s) = &mut self.slots[id] {
            s.terminate();
        }
        let cut = std::mem::take(&mut self.inflight[id]);
        self.end(w, id);
        self.forget(w, id, false, running);
        if !cut.is_empty() {
            self.interrupted[id] = cut;
        }
        self.asked[id] = false;
        self.remembered[id] = true;
        let t = w.tick;
        w.agents[id].set_status(Status::Asleep, t);
        w.agents[id].trace(
            t,
            "the person made it forget its session; the next message starts a new one, told \
             where it left off",
        );
        w.note(Event::Forgotten { agent: id });
        Ok(())
    }

    /// Whether agent `id`, remembered from an earlier run, can go on in
    /// the session it had: its id is shaped like one and, for Claude, the
    /// session's transcript is still where Claude keeps it for the
    /// directory the agent runs in (a Codex rollout's place has the day
    /// in it and a Gemini chat's a registry-given name, so those two say
    /// themselves when one is gone, before any turn). Said once, at the
    /// wake; a session that is gone is forgotten.
    fn resumable(&mut self, w: &World, id: AgentId) -> Option<Resumable> {
        let r = self.sessions[id].clone()?;
        let there = imece_agent::valid_uuid(&r.session)
            && match w.agents[id].vendor {
                Vendor::Claude => {
                    claude::transcript(&self.claude_home, &self.cwd_of(w, id), &r.session).is_file()
                }
                Vendor::Codex | Vendor::Gemini => true,
                // Vendors imece does not drive yet remember no session.
                Vendor::Qwen | Vendor::Local => false,
            };
        if !there {
            self.sessions[id] = None;
        }
        there.then_some(r)
    }

    /// The session agent `id`'s next process goes on with, if its wake
    /// asked for one: the old session above what it had cost.
    fn resume_of(&self, id: AgentId) -> Option<Resumable> {
        self.resuming[id].as_ref().map(|r| Resumable {
            session: r.session.clone(),
            spent: self.floor[id],
        })
    }

    /// The directory agent `id`'s session runs in: the checkout for a
    /// leader, its (or its worker's) worktree for the rest, as the vendor
    /// sees it (a worktree is canonical once it exists).
    fn cwd_of(&self, w: &World, id: AgentId) -> PathBuf {
        let a = &w.agents[id];
        let place = &self.places[a.project];
        if a.role == Role::Leader {
            return place.repo.clone();
        }
        let tree = a.parent.map_or(&a.name, |p| &w.agents[p].name);
        let path = place.root.join(tree);
        path.canonicalize().unwrap_or(path)
    }

    /// Agent `id`'s earlier session could not be resumed (`why`): a new
    /// session, told where it left off, gets the prompts the old one was
    /// given, the first as it was before imece's notes.
    fn restart(&mut self, w: &mut World, id: AgentId, a: Attempt, why: &str) {
        let mut texts = std::mem::take(&mut self.inflight[id]);
        self.end(w, id);
        self.sessions[id] = None;
        self.notes[id] = a.notes;
        self.remembered[id] = true;
        if let Some(first) = texts.first_mut() {
            *first = a.text;
        }
        w.agents[id].trace(
            w.tick,
            format!(
                "session {} could not be resumed ({why}); a new one is told where it left off",
                a.session
            ),
        );
        for text in texts {
            if let Err(why) = self.prompt(w, id, text, None) {
                return apply(w, id, AgentEvent::Stopped(why), self.spent_before[id]);
            }
        }
    }

    /// Helper `id` leaves: its session ends, its mail is dropped, and it is
    /// gone from the screen; its name may be taken again.
    fn release(&mut self, w: &mut World, id: AgentId) {
        self.dismiss(w, id);
        w.leave(id);
        w.note(Event::Left { agent: id });
    }

    /// Agent `id`'s session and runs end, and what waited on it goes: its
    /// mail, imece's notes to it, its change and the person's question.
    /// Its slot stays, for the screen's ids.
    fn dismiss(&mut self, w: &World, id: AgentId) {
        self.running[id].kill_all();
        if let Slot::Running(s) = &mut self.slots[id] {
            s.terminate();
        }
        self.end(w, id);
        self.forget(w, id, true, true);
        self.leaving[id] = false;
        self.mail[id].clear();
        self.notes[id].clear();
        self.asked[id] = false;
        self.changes.remove(&id);
    }

    /// Opens the checkout `path` is in as a project named `name` (else
    /// after its directory), led by a new agent on the lead vendor. A
    /// closed project with the same checkout opens again in its place.
    pub fn open_project(
        &mut self,
        w: &mut World,
        path: &Path,
        name: Option<&str>,
    ) -> Result<ProjectId, ProjectRefusal> {
        let repo = worktree::top(path)
            .map_err(|e| ProjectRefusal::NotCheckout(format!("{}: {e}", path.display())))?;
        let name = match name {
            Some(n) if !valid_project_name(n) => {
                return Err(ProjectRefusal::BadName(n.to_string()));
            }
            Some(n) => n.to_string(),
            None => project_name_of(&repo),
        };
        let at = |q: &Project| self.places.get(q.id).is_some_and(|pl| pl.repo == repo);
        if let Some(q) = w.open_projects().find(|q| at(q)) {
            return Err(ProjectRefusal::PathOpen(q.name.clone()));
        }
        if w.project_by_name(&name).is_some() {
            return Err(ProjectRefusal::NameTaken(name));
        }
        let reuse = w.projects.iter().find(|q| q.closed && at(q)).map(|q| q.id);
        let id = w.agents.len();
        // The socket first: an agent without one could never start.
        self.bind(id, Role::Leader)
            .map_err(|e| ProjectRefusal::Bus(e.to_string()))?;
        let model = self.of(self.lead).ladder.standard.clone();
        self.grow(model, false);
        if reuse.is_none() {
            let root = root_for(&self.state, &repo);
            self.places.push(Place {
                repo: repo.clone(),
                root,
            });
        }
        Ok(w.open_project(reuse, name, repo.display().to_string(), self.lead))
    }

    /// Closes project `p`: its agents' sessions and runs end, their mail,
    /// offers and changes go; their worktrees stay on disk. The last open
    /// project stays.
    pub fn close_project(&mut self, w: &mut World, p: ProjectId) -> Result<(), ProjectRefusal> {
        if w.open_projects().count() <= 1 {
            return Err(ProjectRefusal::Last);
        }
        let ids: Vec<AgentId> = w
            .agents
            .iter()
            .filter(|a| a.project == p && !a.gone)
            .map(|a| a.id)
            .collect();
        for id in ids {
            self.dismiss(w, id);
        }
        self.posts.retain(|c, _| w.cards[*c].project != p);
        self.offered.retain(|c, _| w.cards[*c].project != p);
        w.close_project(p);
        Ok(())
    }

    /// Project `p` is called `name` from now on, its leader `lead-NAME`;
    /// its agents hear of it at their next turn, their sessions go on.
    pub fn rename_project(
        &mut self,
        w: &mut World,
        p: ProjectId,
        name: &str,
    ) -> Result<(), ProjectRefusal> {
        if !valid_project_name(name) {
            return Err(ProjectRefusal::BadName(name.to_string()));
        }
        if w.project_by_name(name).is_some_and(|q| q != p) {
            return Err(ProjectRefusal::NameTaken(name.to_string()));
        }
        let was = w.agents[w.projects[p].leader].name.clone();
        w.rename_project(p, name.to_string());
        let lead = w.agents[w.projects[p].leader].name.clone();
        for a in w.agents.iter().filter(|a| a.project == p && !a.gone) {
            self.notes[a.id].push(format!(
                "The person renamed the project: it is called {name} now, and its leader, \
                 {was} until now, is {lead}. Address it by that name."
            ));
        }
        Ok(())
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
        self.forget(w, id, false, true);
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
        self.forget(w, id, false, true);
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

    /// The leader `from` adds a worker, or the worker `from` adds a helper
    /// (the `hire` tool's arguments). The answer names it.
    fn hire(&mut self, w: &mut World, from: AgentId, args: &Value) -> Answer {
        let hirer = &w.agents[from];
        let (project, own) = (hirer.project, hirer.vendor);
        let role = match hirer.role {
            Role::Leader => Role::Worker,
            Role::Worker => Role::Helper,
            _ => return Err("only the leader and workers hire; ask yours with send".into()),
        };
        // How many there are and how many there may be.
        let (so_far, cap) = if role == Role::Worker {
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
            (workers, self.max_workers)
        } else {
            let helpers = w.helpers_of(from).count();
            if self.max_helpers == 0 {
                return Err(
                    "the person runs you without helpers (--helpers 0); do it yourself".into(),
                );
            }
            if helpers >= self.max_helpers {
                return Err(format!(
                    "you have {helpers} helpers, the most the person allowed (--helpers {}); \
                     wait for one to send done",
                    self.max_helpers
                ));
            }
            (helpers, self.max_helpers)
        };
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
        // The next number, skipping names already taken: a worker's after
        // its vendor (`claude-2`), a helper's after its worker
        // (`claude-2.1`), counting the ones that left.
        let (stem, next) = if role == Role::Worker {
            let n = w
                .agents
                .iter()
                .filter(|o| o.vendor == vendor.vendor && o.role == Role::Worker)
                .count();
            (format!("{}-", vendor.vendor.label()), n + 1)
        } else {
            let n = w
                .agents
                .iter()
                .filter(|o| o.role == Role::Helper && o.parent == Some(from))
                .count();
            (format!("{}.", w.agents[from].name), n + 1)
        };
        let name = (next..)
            .map(|n| format!("{stem}{n}"))
            .find(|name| w.agent_by_name(name).is_none())
            .expect("some number is free");
        let id = w.agents.len();
        // The socket first: an agent without one could never start.
        self.bind(id, role)
            .map_err(|e| format!("{name} could not get a socket: {e}"))?;
        self.grow(model.clone(), tiered);
        let t = w.tick;
        let mut agent = Agent::new(
            id,
            project,
            name.clone(),
            vendor.vendor,
            role,
            Status::Idle,
            t,
        );
        agent.parent = (role == Role::Helper).then_some(from);
        w.agents.push(agent);
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
            cap - so_far - 1
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
        if kind.wakes() && w.agents[to].status == Status::Asleep {
            // Something new since the start: a sleeper takes it like any
            // idle agent (its older mail rides along).
            let t = w.tick;
            w.agents[to].set_status(Status::Idle, t);
            w.agents[to].trace(t, "woken by mail");
        }
        if kind == MsgKind::Done && w.agents[from].parent == Some(to) {
            // A helper's work is its worker's: done to it, and it leaves
            // when this turn ends.
            self.leaving[from] = true;
        }

        let t = w.tick;
        let first = body.lines().next().unwrap_or_default().to_string();
        // A worker's done shows the person what its worktree holds.
        let change = if kind == MsgKind::Done && w.agents[from].role == Role::Worker {
            self.snapshot(w, from, &first)
        } else {
            String::new()
        };
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
        let delivered = match (kind.wakes(), self.turns[to] > 0) {
            (true, false) => format!("delivered; {name} starts on it now"),
            (true, true) => format!("queued; {name} is busy and gets it when its turn ends"),
            (false, _) => format!(
                "queued for {name}'s next turn ({} does not wake it)",
                kind.wire()
            ),
        };
        Ok(delivered + &change)
    }

    /// Reads what worker `id`'s worktree holds, for the person: it waits
    /// in their inbox until they apply or reject it. Says what the worker
    /// is told about it.
    fn snapshot(&mut self, w: &mut World, id: AgentId, title: &str) -> String {
        let Some(cwd) = self.cwds[id].clone() else {
            return String::new();
        };
        match worktree::change(&cwd, MAX_PATCH) {
            Ok(None) => {
                w.agents[id].trace(w.tick, "change: none");
                "; your worktree holds no change".to_string()
            }
            Ok(Some(c)) => {
                let (added, removed) = c.counts();
                let files = c.files.len();
                w.agents[id].trace(w.tick, format!("change: {files} files +{added} −{removed}"));
                w.change_ready(Change {
                    agent: id,
                    title: title.to_string(),
                    files: c
                        .files
                        .iter()
                        .map(|f| ChangedFile {
                            path: f.path.clone(),
                            added: f.added,
                            removed: f.removed,
                            binary: f.binary,
                        })
                        .collect(),
                    patch: c.patch.clone(),
                });
                self.changes.insert(id, c);
                format!(
                    "; the person sees your change ({files} files, +{added} −{removed}) and \
                     applies it to their checkout or rejects it; it stays in your worktree \
                     until then"
                )
            }
            Err(e) => {
                w.agents[id].trace(w.tick, format!("change: {e}"));
                format!("; imece could not read your change: {e}")
            }
        }
    }

    /// The person takes worker `id`'s change: it goes into the checkout,
    /// staged, and is committed in the worktree as a mark, so the next
    /// change holds only what comes after. A change that does not fit is
    /// refused with git's words and still waits.
    pub fn apply_change(&mut self, w: &mut World, id: AgentId) -> Result<(), String> {
        let Some(c) = self.changes.get(&id) else {
            return Err("no change of theirs waits".into());
        };
        let title = w.change_of(id).map(|c| c.title.clone()).unwrap_or_default();
        if let Err(e) = worktree::apply(&self.places[w.agents[id].project].repo, &c.patch) {
            let why = e.to_string();
            w.change_failed(id, why.clone());
            return Err(why);
        }
        let mark = match worktree::applied(c, &format!("imece: the person applied: {title}")) {
            Ok(mark) => mark,
            Err(e) => {
                w.change_failed(
                    id,
                    format!("applied, but the worktree could not be marked: {e}"),
                );
                String::new()
            }
        };
        self.changes.remove(&id);
        w.change_applied(id, mark.clone());
        self.notes[id].push(format!(
            "The person applied your change ({title}) to their checkout. Your worktree is \
             committed at {mark} as a mark of that; your files are as you left them. \
             Go on from here with the next task."
        ));
        Ok(())
    }

    /// The person rejects worker `id`'s change: it stays in the worktree,
    /// and the worker is told to wait for their reasons.
    pub fn reject_change(&mut self, w: &mut World, id: AgentId) {
        let title = w.change_of(id).map(|c| c.title.clone()).unwrap_or_default();
        self.changes.remove(&id);
        w.change_rejected(id);
        self.notes[id].push(format!(
            "The person rejected your change ({title}). It stays in your worktree, not \
             applied; they say why themselves. Do not send it again as it is."
        ));
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
                || a.gone
                || a.status == Status::Stopped
                || self.turns[id] > 0
                || self.woken[id] >= MAX_WOKEN
                || self.over_budget(w)
            {
                continue;
            }
            // A sleeper is woken by a card posted since the start, not by
            // one that waited through it.
            let asleep = a.status == Status::Asleep;
            let cards: Vec<CardId> = self
                .open(w, a.project)
                .filter(|&c| !asleep || c >= self.loaded_cards)
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
        w.spent() >= w.budget
    }

    fn ready(&mut self, w: &mut World, id: AgentId, ready: Ready) {
        let waiting = match std::mem::replace(&mut self.slots[id], Slot::None) {
            Slot::Starting(q) => q,
            _ => Vec::new(),
        };
        let mut session = match ready {
            Ok((session, cwd, placed)) => {
                w.agents[id].trace(w.tick, format!("session in {}", cwd.display()));
                self.cwds[id] = Some(cwd);
                match placed {
                    Some(worktree::Placed::Moved { from, to }) => {
                        w.note(Event::WorktreeMoved {
                            agent: id,
                            from,
                            to,
                        });
                    }
                    Some(worktree::Placed::Kept { at, head, why }) => {
                        w.note(Event::WorktreeKept {
                            agent: id,
                            at,
                            head,
                            commits: why == worktree::Why::Commits,
                        });
                    }
                    Some(worktree::Placed::Fresh | worktree::Placed::AtHead) | None => {}
                }
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

    /// Gives agent `id` one line of its vendor's own command line (`/clear`,
    /// `/compact`, `/cost` ...), typed by the person. It runs as a turn and
    /// its answer reaches the person. A vendor without `/clear` of its own
    /// gets imece's: the session ends, and the next message starts a new
    /// one.
    pub fn command(&mut self, w: &mut World, id: AgentId, line: &str) -> Result<(), Refusal> {
        if self.over_budget(w) {
            return Err(Refusal::Budget);
        }
        let line = line.trim();
        let Slot::Running(s) = &mut self.slots[id] else {
            return Err(Refusal::Command(
                "it has no session yet; say something to it first".into(),
            ));
        };
        match s.command(line) {
            Ok(()) => {
                self.inflight[id].push(line.to_string());
                self.turns[id] += 1;
                self.asked[id] = true;
                let t = w.tick;
                let a = &mut w.agents[id];
                a.set_status(Status::Working, t);
                a.trace(t, format!("command: {line}"));
                Ok(())
            }
            Err(e) if e.kind() == io::ErrorKind::Unsupported && line == "/clear" => {
                s.terminate();
                self.end(w, id);
                self.forget(w, id, false, true);
                w.agents[id].trace(
                    w.tick,
                    "session ended on /clear; the next message starts a new one",
                );
                Ok(())
            }
            Err(e) => Err(Refusal::Command(e.to_string())),
        }
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
        self.follow(w, id);
        if std::mem::take(&mut self.remembered[id]) {
            let resumed = self.resumable(w, id);
            if let Some(r) = &resumed {
                self.floor[id] = r.spent;
                self.resuming[id] = Some(Attempt {
                    session: r.session.clone(),
                    text: text.clone(),
                    notes: self.notes[id].clone(),
                });
            } else {
                w.note(Event::Restarted { agent: id });
            }
            let briefing = self.brief(w, id, resumed.is_some());
            self.notes[id].insert(0, briefing);
        }
        let text = self.with_notes(id, text);
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
        // A worker between tasks gets a card for this one; a task it has
        // not finished (its session stopped or slept under it) goes on.
        let unfinished = a.card.is_some_and(|c| w.cards[c].column != Column::Done);
        if let Some(title) = title
            && a.role == Role::Worker
            && a.status != Status::Working
            && !unfinished
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

    /// What imece has to tell agent `id`, ahead of `text`.
    fn with_notes(&mut self, id: AgentId, text: String) -> String {
        let notes = std::mem::take(&mut self.notes[id]);
        if notes.is_empty() {
            return text;
        }
        let mut out = String::from("From imece (not from the person or a teammate):");
        for n in &notes {
            out.push_str("\n- ");
            out.push_str(n);
        }
        out.push_str("\n\n");
        out + &text
    }

    /// Between turns, a worker's clean worktree follows the checkout's
    /// HEAD (as it does when its session starts), so a change the person
    /// applied and committed is under the worker's feet for the next task.
    /// Not while it or one of its helpers is mid-turn.
    fn follow(&mut self, w: &mut World, id: AgentId) {
        let a = &w.agents[id];
        let busy = self.turns[id] > 0 || w.helpers_of(id).any(|h| self.turns[h.id] > 0);
        if a.role != Role::Worker || busy || !matches!(self.slots[id], Slot::Running(_)) {
            return;
        }
        let Some(cwd) = self.cwds[id].clone() else {
            return;
        };
        match worktree::place(&self.places[a.project].repo, &cwd) {
            Ok(worktree::Placed::Moved { from, to }) => {
                w.note(Event::WorktreeMoved {
                    agent: id,
                    from,
                    to,
                });
            }
            Ok(_) => {}
            Err(e) => w.agents[id].trace(w.tick, format!("worktree: {e}")),
        }
    }

    /// Starts agent `id`'s session on its own thread; `step` picks it up.
    fn start(&self, w: &World, id: AgentId) {
        let a = &w.agents[id];
        let leader = a.role == Role::Leader;
        let vendor = self.of(a.vendor);
        let mcp = self.bus.as_ref().map(|b| Mcp {
            command: b.relay.clone(),
            args: vec![
                "mcp-relay".into(),
                b.servers[id].path().to_string_lossy().into_owned(),
            ],
            tools: tools_of(
                a.role,
                &self.vendor_names(),
                &self.run_programs,
                self.run_timeout,
            )
            .into_iter()
            .map(|t| t.name)
            .collect(),
        });
        let place = &self.places[a.project];
        let mut system = system_prompt(w, id, self.max_workers, self.max_helpers);
        if let Some(notes) = repo_notes(&place.repo, a.vendor) {
            system.push_str(&notes);
        }
        let (program, model) = (vendor.program.clone(), self.models[id].clone());
        let name = format!("imece {}", a.name);
        let resume = self.resume_of(id);
        // In millionths of a dollar: what is left of the budget.
        let budget = w.budget.saturating_sub(w.spent()) * 100;
        let kind = a.vendor;
        let generation = self.generation[id];
        // A helper changes the same files as its worker: its worktree.
        let tree = a.parent.map_or(&a.name, |p| &w.agents[p].name).clone();
        let (repo, root) = (place.repo.clone(), place.root.clone());
        let home = self.gemini_home(a.project, a.role, &a.name);
        let (tx, ready) = (self.tx.clone(), self.ready_tx.clone());
        thread::spawn(move || {
            let started = (|| {
                let (cwd, placed) = if leader {
                    (repo, None)
                } else {
                    let w = worktree::ensure(&repo, &root, &tree).map_err(|e| e.to_string())?;
                    (w.path, Some(w.placed))
                };
                let on = move |e| {
                    let _ = tx.send((id, generation, e));
                };
                let session: Box<dyn Session> = match kind {
                    Vendor::Gemini => Box::new(
                        gemini::Session::start(
                            &gemini::Spec {
                                program: program.clone(),
                                cwd: cwd.clone(),
                                home,
                                model,
                                write: !leader,
                                system,
                                mcp,
                                resume: resume.map(|r| r.session),
                            },
                            on,
                        )
                        .map_err(|e| format!("{program}: {e}"))?,
                    ),
                    Vendor::Codex => Box::new(
                        codex::Session::start(
                            &codex::Spec {
                                program: program.clone(),
                                cwd: cwd.clone(),
                                choice: model,
                                write: !leader,
                                system,
                                mcp,
                                resume: resume.map(|r| r.session),
                            },
                            on,
                        )
                        .map_err(|e| format!("{program}: {e}"))?,
                    ),
                    _ => {
                        let tools = if leader {
                            claude::READ_TOOLS
                        } else {
                            claude::EDIT_TOOLS
                        };
                        Box::new(
                            claude::Session::start(
                                &claude::Spec {
                                    program: program.clone(),
                                    cwd: cwd.clone(),
                                    name,
                                    tools: tools.iter().map(|t| t.to_string()).collect(),
                                    model,
                                    budget,
                                    system,
                                    mcp,
                                    resume: resume.map(|r| claude::Resume {
                                        session: r.session,
                                        spent: r.spent,
                                    }),
                                },
                                on,
                            )
                            .map_err(|e| format!("{program}: {e}"))?,
                        )
                    }
                };
                Ok((session, cwd, placed))
            })();
            // Nobody to take it (imece is closing): dropping it stops it.
            let _ = ready.send((id, generation, started));
        });
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

/// Applies one event of agent `id`'s session. `before` is what its earlier
/// sessions cost, in ten-thousandths of a dollar.
pub fn apply(w: &mut World, id: AgentId, e: AgentEvent, before: u64) {
    let t = w.tick;
    let card = w.agents[id].card;
    match e {
        AgentEvent::Started { session, model } => {
            w.agents[id].trace(t, format!("session {session} · {model}"));
        }
        AgentEvent::Commands(mut commands) => {
            // imece's own /clear ends a session whose vendor has none.
            if !commands.iter().any(|c| c == "/clear") {
                commands.push("/clear".into());
            }
            w.agents[id].trace(t, format!("commands {}", commands.join(" ")));
            w.agents[id].commands = commands;
        }
        AgentEvent::Text(text) => w.agents[id].trace(t, format!("text {text:?}")),
        AgentEvent::Note(text) => w.agents[id].trace(t, text),
        AgentEvent::Tool { name, target } => {
            let mcp = imece_agent::mcp_tool("");
            let name = name.strip_prefix(&mcp).map_or(name.clone(), str::to_string);
            let changes = matches!(name.as_str(), "Edit" | "Write" | codex::PATCH);
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
                // The story keeps it; the journal gets it only when it
                // goes to the person (`Live::step` decides).
                w.story(card, StoryEvent::Replied(text));
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
/// leader hires workers and a worker hires helpers (from `vendors`);
/// helpers and researchers hire nobody.
fn tools_of(role: Role, vendors: &[&str], programs: &[String], timeout: Duration) -> Vec<Tool> {
    let mut tools = vec![
        send_tool(),
        post_tool(),
        board_tool(),
        run_tool(role, programs, timeout),
    ];
    match role {
        Role::Worker => {
            tools.push(claim_tool());
            tools.push(hire_tool(vendors, HIRE_HELPER));
        }
        Role::Leader => tools.push(hire_tool(vendors, HIRE_WORKER)),
        Role::Helper | Role::Researcher => {}
    }
    tools
}

/// How one run ended, for the trace.
fn run_ended(o: &run::Outcome) -> String {
    let how = if o.timed_out {
        "killed at the deadline".to_string()
    } else {
        match o.code {
            Some(c) => format!("exit {c}"),
            None => "killed".to_string(),
        }
    };
    format!("{how}, {:.1} s, {} bytes", o.took.as_secs_f64(), o.bytes)
}

/// What the agent reads back from a run.
fn run_answer(o: &run::Outcome) -> String {
    let first = if o.timed_out {
        format!(
            "killed after {} s: it did not finish in time",
            o.took.as_secs()
        )
    } else {
        match o.code {
            Some(0) => "exit code 0".to_string(),
            Some(c) => format!("exit code {c} (failed)"),
            None => "killed by a signal".to_string(),
        }
    };
    if o.output.is_empty() {
        format!("{first}, no output")
    } else {
        format!("{first}\n{}", o.output)
    }
}

/// Whether an agent in `role` may run `argv`: cargo without the commands
/// that change dependencies (a leader: without the ones that change
/// files), git's reading commands, and for workers and helpers the
/// programs the person named. Nothing else.
fn allowed(role: Role, argv: &[String], programs: &[String]) -> Result<(), String> {
    let program = Path::new(&argv[0])
        .file_name()
        .map(|f| f.to_string_lossy().into_owned())
        .unwrap_or_default();
    if argv[0] != program {
        return Err(format!(
            "{}: name the program alone, on PATH; not a path",
            argv[0]
        ));
    }
    // The first word that is not an option (or a toolchain, `+nightly`).
    let sub = argv[1..]
        .iter()
        .find(|a| !a.starts_with('-') && !a.starts_with('+'))
        .map_or("", String::as_str);
    let leader = role == Role::Leader;
    match program.as_str() {
        "cargo" => {
            if CARGO_DENIED.contains(&sub) {
                return Err(format!(
                    "cargo {sub} is for the person: dependencies and what is outside the \
                     project are not changed through imece"
                ));
            }
            if leader && CARGO_WRITES.contains(&sub) && !argv.iter().any(|a| a == "--check") {
                return Err(format!(
                    "cargo {sub} changes files, and a leader changes none; a worker can"
                ));
            }
            Ok(())
        }
        "git" => {
            if GIT_READ.contains(&sub) {
                Ok(())
            } else {
                Err(format!(
                    "git {sub} is not run through imece; git only reads here: {}",
                    GIT_READ.join(", ")
                ))
            }
        }
        _ if !leader && programs.contains(&program) => Ok(()),
        _ => {
            let mut can = vec!["cargo".to_string(), "git".to_string()];
            if !leader {
                can.extend(programs.iter().cloned());
            }
            Err(format!(
                "{program} is not a program imece runs; these are: {} (the person adds \
                 one with --run NAME)",
                can.join(", ")
            ))
        }
    }
}

fn run_tool(role: Role, programs: &[String], timeout: Duration) -> Tool {
    let mut can = vec![
        "cargo (not add, remove, install or publish)".to_string(),
        format!("git ({} only)", GIT_READ.join(", ")),
    ];
    if role == Role::Leader {
        can[0] = "cargo (check, clippy, test, build, fmt --check; not the commands that \
                  change files)"
            .to_string();
    } else {
        can.extend(programs.iter().cloned());
    }
    Tool {
        name: "run".into(),
        description: format!(
            "Runs one program in your directory and gives back its exit code and \
             output (stdout and stderr together; lines of passing tests are left \
             out, very long output is cut in the middle). No shell: one program and \
             its arguments, no pipes, redirections or variables; quote an argument \
             with spaces. Programs: {}. A run is killed after {} s. Use it to check \
             your work (build, lint, test) before you say it is done.",
            can.join("; "),
            timeout.as_secs()
        ),
        input: json!({
            "type": "object",
            "properties": {
                "command": {
                    "type": "string",
                    "description": "The program and its arguments, as one line: \
                                    cargo test --workspace",
                },
            },
            "required": ["command"],
            "additionalProperties": false,
        }),
    }
}

/// The hire tool as the leader sees it.
const HIRE_WORKER: &str = "Adds a worker to your team in imece: a coding agent in its own git \
                           worktree of the repository. The answer names it; then give it work \
                           with send (type assign). Hire for work that can run at once, not in \
                           advance: one worker takes several tasks in turn.";
/// The hire tool as a worker sees it.
const HIRE_HELPER: &str = "Adds a helper of your own in imece: a coding agent in your worktree, \
                           changing the same files as you. The answer names it; then give it a \
                           part of your task with send (type assign): what to do, which files, \
                           when it is done. It sends you done and leaves. Hire for a part that \
                           can run beside your own work, in files you are not changing.";

fn hire_tool(vendors: &[&str], description: &str) -> Tool {
    let tiers: Vec<&str> = Tier::ALL.iter().map(|t| t.wire()).collect();
    Tool {
        name: "hire".into(),
        description: description.into(),
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

/// The system prompt of agent `id`; a leader may hire up to `max_workers`,
/// a worker may have up to `max_helpers` at once.
fn system_prompt(w: &World, id: AgentId, max_workers: usize, max_helpers: usize) -> String {
    let a = &w.agents[id];
    let workers: Vec<&str> = w
        .agents
        .iter()
        .filter(|o| o.role == Role::Worker && !o.gone && o.project == a.project)
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
                     themselves. A worker's done shows the person its change (its \
                     worktree's diff); they apply it to their checkout or reject it, and \
                     only an applied change is in the checkout you read. Workers answer \
                     with done, blocked or ask; their \
                     messages start your next turn."
                )
            };
            format!(
                "You are {}, the leader of a team of coding agents in imece, on the \
                 repository in your current directory. {} The person talks to you. \
                 {team} You never change files yourself, so do not offer to; offer \
                 only work a worker will do. Answer the person briefly: what you \
                 found or assigned, and when all is done, what was done and what is \
                 left. When you need the person to decide, end with one clear \
                 question.",
                a.name,
                abilities(a.vendor, Role::Leader)
            )
        }
        Role::Helper => {
            let parent = &w.agents[a.parent.expect("a helper has a worker")].name;
            format!(
                "You are {}, a helper of {parent}, a coding agent in imece, a team led by \
                 {leader}. You work in {parent}'s git worktree of the repository (your \
                 current directory), on the files {parent} gave you; it changes others \
                 at the same time, so stay in yours. {} Do not commit. Your task comes \
                 from {parent}. When it is done, send {parent} a done message with a \
                 short summary of what you changed and what is left open; you leave \
                 then. If you are stuck, send {parent} blocked or ask instead of \
                 guessing. Use these tools only for that, not for chatter.",
                a.name,
                abilities(a.vendor, Role::Helper)
            )
        }
        _ => {
            let helpers = if max_helpers == 0 {
                String::new()
            } else {
                format!(
                    " You may hire helpers with the hire tool, up to {max_helpers} at once: \
                     each is a coding agent in your worktree, changing the same files as \
                     you. Give one a part of your task with send (type assign: what to do, \
                     which files, when it is done), in files you are not changing yourself; \
                     it sends you done and leaves. Hire for a part that can run beside your \
                     own work, not for the whole task and not in advance."
                )
            };
            format!(
                "You are {}, a coding agent in imece, a team led by {leader}. You work \
                 in your own git worktree of the repository (your current \
                 directory); the others work in theirs. {} Do not commit. Tasks come \
                 from {leader} or from the person. When you finish a task from \
                 {leader}, send it a done message with a short summary of what \
                 you changed and what is left open; your done also shows the person \
                 your worktree's diff, which they apply to their checkout or reject, \
                 so finish (build, test) before you send it. If you are stuck, send blocked \
                 or ask instead of guessing. imece may offer you open cards from \
                 the board; take one that fits with claim, and send whoever posted \
                 it a done message when it is done. Work you find outside your \
                 task goes on the board (post), not into your change.{helpers} Use \
                 these tools only for that, not for chatter.",
                a.name,
                abilities(a.vendor, Role::Worker)
            )
        }
    }
}

/// The files a repository keeps its notes for coding agents in, in the
/// order they are looked for; the first one there is used.
const NOTES_FILES: &[&str] = &["AGENTS.md", "CLAUDE.md"];
/// The most of a notes file that goes into a prompt, in bytes.
const MAX_NOTES: usize = 16 << 10;

/// The repository's notes for coding agents (its `AGENTS.md` or
/// `CLAUDE.md`), to append to an agent's instructions. The vendors' CLIs
/// are started so that they load nothing from the repository (no hooks,
/// no settings), which drops these notes too; imece passes them itself.
/// Codex loads `AGENTS.md` from its directory on its own and gets it only
/// when the notes are in another file.
fn repo_notes(repo: &Path, vendor: Vendor) -> Option<String> {
    let (file, text) = NOTES_FILES
        .iter()
        .find_map(|f| std::fs::read_to_string(repo.join(f)).ok().map(|t| (*f, t)))?;
    if text.trim().is_empty() || (vendor == Vendor::Codex && file == "AGENTS.md") {
        return None;
    }
    let mut body = text.trim().to_string();
    if body.len() > MAX_NOTES {
        // The last character boundary at or before the cap.
        let cut = (0..=MAX_NOTES)
            .rev()
            .find(|&i| body.is_char_boundary(i))
            .unwrap_or(0);
        body.truncate(cut);
        body.push_str("\n[cut: the file goes on]");
    }
    Some(format!(
        "\n\nThe repository's notes for coding agents, from its {file}:\n\n{body}"
    ))
}

/// What an agent of `vendor` in `role` can do to the files, as its
/// session is set up: Claude Code gets file tools and no shell; Codex
/// always has a shell, in a sandbox that is read-only for a leader and
/// confined to the agent's directory for the rest, never with the network.
fn abilities(vendor: Vendor, role: Role) -> &'static str {
    match (vendor, role) {
        (Vendor::Codex, Role::Leader) => {
            "You can read files and run commands that only read; the sandbox refuses \
             writes and the network."
        }
        (Vendor::Codex, _) => {
            "You can read and change files here and run commands; the sandbox keeps \
             writes inside this directory and refuses the network."
        }
        (_, Role::Leader) => {
            "You can read files but not change them; the run tool runs checks (cargo \
             check, clippy, test; git diff, log) in the checkout."
        }
        _ => {
            "You can read and change files here; the run tool runs the build, the \
             tests and git's reading commands, nothing else."
        }
    }
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

/// The local time of day at `at` (seconds since the Unix epoch): what the
/// screen's clock starts at (`start_clock_at`) for a world begun then.
pub fn local_seconds_after_midnight(at: u64) -> u64 {
    let now = at;
    let secs = now as libc::time_t;
    // SAFETY: `tm` is plain data that localtime_r fills; it keeps no pointer.
    let mut tm: libc::tm = unsafe { std::mem::zeroed() };
    if unsafe { libc::localtime_r(&secs, &mut tm) }.is_null() {
        return now % 86_400;
    }
    (tm.tm_hour as u64) * 3600 + (tm.tm_min as u64) * 60 + tm.tm_sec as u64
}

/// Teams for the tests of this module and of the screen.
#[cfg(test)]
pub(crate) mod testing {
    use super::*;

    /// A team without sessions, to apply events to.
    pub(crate) fn team(workers: usize) -> World {
        let mut w = World {
            tick: 3,
            // Begun just now: a step puts the tick at 0.
            started_at: now(),
            run_tick: 0,
            run_spent: 0,
            projects: vec![Project {
                id: 0,
                name: "repo".into(),
                leader: 0,
                trunk: None,
                repo: &[],
                closed: false,
            }],
            agents: Vec::new(),
            cards: Vec::new(),
            journal: Vec::new(),
            inbox: Vec::new(),
            budget: 10_000,
            max_researchers: 0,
            research_queue: Default::default(),
            plans: Vec::new(),
            changes: Vec::new(),
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

    pub(super) fn claude_at(program: &str) -> Vec<Hireable> {
        vec![Hireable::claude(program, &[])]
    }

    /// A live team whose sessions never start: what counts is what was
    /// queued for them. A start still runs on its own thread and fails at
    /// once (there is no `/nonexistent/claude`); its report is cut off
    /// here, or it would race the events a test sends in the session's
    /// name and end the session under them.
    pub(crate) fn idle_live(agents: usize) -> Live {
        idle_live_in(&std::env::temp_dir(), agents)
    }

    /// [`idle_live`] with `repo` as the first project's checkout.
    pub(crate) fn idle_live_in(repo: &Path, agents: usize) -> Live {
        let mut live = Live::new(
            agents,
            claude_at("/nonexistent/claude"),
            repo.to_path_buf(),
            PathBuf::from("/nonexistent"),
        );
        let (cut, _) = mpsc::channel();
        live.ready_tx = cut;
        live
    }

    /// A fresh git checkout with one commit, under the temp dir.
    pub(crate) fn checkout(name: &str) -> std::path::PathBuf {
        let dir = std::env::temp_dir().join(format!("imece-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let git = |args: &[&str]| {
            let out = std::process::Command::new("git")
                .args(args)
                .current_dir(&dir)
                .output()
                .unwrap();
            assert!(out.status.success(), "git {args:?}");
        };
        git(&["init", "--quiet"]);
        std::fs::write(dir.join("a.txt"), "a").unwrap();
        git(&["add", "."]);
        git(&[
            "-c",
            "user.name=t",
            "-c",
            "user.email=t@t",
            "-c",
            "commit.gpgsign=false",
            "commit",
            "--quiet",
            "-m",
            "init",
        ]);
        dir
    }

    impl Live {
        /// `agents` places to begin with, all on their vendor's default model,
        /// in one project whose checkout is `repo` and whose worktrees go
        /// under `root`. `vendors` must hold every vendor an agent has; the
        /// first leads.
        pub(super) fn new(
            agents: usize,
            vendors: Vec<Hireable>,
            repo: PathBuf,
            root: PathBuf,
        ) -> Live {
            let lead = vendors.first().map_or(Vendor::Claude, |h| h.vendor);
            let mut live = Live::bare(vendors, lead);
            live.settle = Duration::ZERO;
            live.places.push(Place { repo, root });
            for _ in 0..agents {
                live.grow(None, false);
            }
            live
        }
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;
    use std::sync::atomic::{AtomicBool, Ordering};

    use super::testing::*;
    use super::*;

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
    fn the_command_line_a_session_lists_is_kept_with_imeces_clear_added() {
        let mut w = team(1);
        assert!(w.agents[1].commands.is_empty());
        apply(
            &mut w,
            1,
            AgentEvent::Commands(vec!["/compact".into(), "/cost".into()]),
            0,
        );
        assert_eq!(w.agents[1].commands, ["/compact", "/cost", "/clear"]);
        assert!(
            w.agents[1]
                .trace
                .iter()
                .any(|l| l.contains("commands /compact /cost /clear")),
            "{:?}",
            w.agents[1].trace
        );
        // A list with /clear of its own keeps it where it is; a new list
        // replaces the old one.
        apply(
            &mut w,
            1,
            AgentEvent::Commands(vec!["/clear".into(), "/stats".into()]),
            0,
        );
        assert_eq!(w.agents[1].commands, ["/clear", "/stats"]);
        apply(&mut w, 1, AgentEvent::Commands(vec![]), 0);
        assert_eq!(w.agents[1].commands, ["/clear"]);
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
        assert!(
            matches!(&w.cards[c].story.last().unwrap().event, StoryEvent::Replied(t) if t == "Fixed both.\nDetails."),
            "the story keeps the closing text"
        );
        assert!(
            !w.journal
                .iter()
                .any(|e| matches!(e.event, Event::Replied { .. })),
            "the journal does not: only a text that reaches the person is a reply"
        );
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
        w.budget = 100 * 10_000;
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
        w.budget = 20 * 10_000;
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
        let start = std::time::Instant::now();
        while !done(w) {
            assert!(start.elapsed().as_secs() < 10, "timed out");
            live.step(w);
            thread::sleep(std::time::Duration::from_millis(10));
        }
    }

    /// Claude Code as `program`, on its default models.
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
        w.agents[0].cost = w.budget;
        assert_eq!(live.told(&mut w, 1, "go"), Err(Refusal::Budget));
        w.agents[0].cost = 6_000;
        w.agents[1].cost = w.budget - 6_000;
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

    /// A session that takes prompts and does nothing with them.
    struct Stub;
    impl imece_agent::Agent for Stub {
        fn prompt(&mut self, _: &str) -> io::Result<()> {
            Ok(())
        }
        fn command(&mut self, line: &str) -> io::Result<()> {
            if line == "/known" {
                Ok(())
            } else {
                Err(io::Error::new(
                    io::ErrorKind::Unsupported,
                    "no such command",
                ))
            }
        }
        fn terminate(&mut self) {}
    }

    #[test]
    fn a_command_runs_as_a_turn_and_clear_ends_a_session_without_one_of_its_own() {
        let mut w = team(1);
        let mut live = idle_live(2);
        let e = live.command(&mut w, 1, "/known").unwrap_err();
        assert!(
            matches!(&e, Refusal::Command(why) if why.contains("no session yet")),
            "{e:?}"
        );
        let cwd = PathBuf::from("/work/claude-1");
        live.ready(&mut w, 1, Ok((Box::new(Stub), cwd.clone(), None)));
        live.command(&mut w, 1, " /known ").unwrap();
        assert_eq!(live.turns[1], 1);
        assert!(live.asked[1]);
        assert_eq!(live.inflight[1], ["/known"]);
        assert_eq!(w.agents[1].status, Status::Working);
        assert!(
            w.agents[1]
                .trace
                .iter()
                .any(|l| l.ends_with("command: /known"))
        );
        let e = live.command(&mut w, 1, "/other").unwrap_err();
        assert!(
            matches!(&e, Refusal::Command(why) if why.contains("no such command")),
            "{e:?}"
        );
        assert!(
            matches!(live.slots[1], Slot::Running(_)),
            "a refused command ends nothing"
        );
        // A command's empty answer is not a reply to the person.
        live.step(&mut w);
        let _ = live.tx.send((
            1,
            live.generation[1],
            AgentEvent::TurnDone {
                ok: true,
                text: String::new(),
                cost: 0,
                denied: 0,
            },
        ));
        live.step(&mut w);
        assert!(w.inbox.is_empty(), "{:?}", w.inbox);
        assert!(
            w.agents[1]
                .trace
                .iter()
                .any(|l| l.ends_with("nothing to say"))
        );
        assert_eq!(live.turns[1], 0);
        // /clear without one of the vendor's own: the session ends, and
        // is not one to go on with later.
        live.sessions[1] = Some(Resumable {
            session: "686c6d27-d50b-4ae2-ad6a-159bf2984671".into(),
            spent: 5,
        });
        live.claude_home = trees("clear-home");
        live.settle = Duration::from_millis(400);
        let file = transcript_of(&live, &w, 1, "686c6d27-d50b-4ae2-ad6a-159bf2984671");
        live.command(&mut w, 1, "/clear").unwrap();
        assert!(matches!(live.slots[1], Slot::None));
        assert_eq!(live.sessions[1], None);
        assert!(
            w.agents[1]
                .trace
                .iter()
                .any(|l| l.contains("session ended on /clear"))
        );
        assert_eq!(live.cwds[1], None);
        thread::sleep(Duration::from_millis(100));
        assert!(
            file.is_file(),
            "the process is given time to be gone before its transcript is touched"
        );
        assert!(
            gone_soon(&file),
            "the transcript of the cleared session goes"
        );
        assert!(traced_soon(
            &mut live,
            &mut w,
            1,
            "forgot session 686c6d27-d50b-4ae2-ad6a-159bf2984671: its transcript was removed"
        ));
        let _ = std::fs::remove_dir_all(&live.claude_home);
        // Over budget nothing runs, a clear included.
        live.ready(&mut w, 1, Ok((Box::new(Stub), cwd, None)));
        w.agents[1].cost = w.budget;
        assert_eq!(live.command(&mut w, 1, "/clear"), Err(Refusal::Budget));
    }

    fn argv(line: &str) -> Vec<String> {
        run::split(line).unwrap()
    }

    #[test]
    fn what_may_run_is_cargo_git_s_reads_and_the_person_s_programs_nothing_else() {
        let extra = vec!["npm".to_string(), "make".to_string()];
        for role in [Role::Leader, Role::Worker, Role::Helper] {
            let ok = |line: &str| allowed(role, &argv(line), &extra).unwrap();
            let no = |line: &str| allowed(role, &argv(line), &extra).unwrap_err();
            ok("cargo test --workspace");
            ok("cargo +nightly clippy -- -D warnings");
            ok("cargo fmt --all --check");
            ok("cargo --locked check");
            ok("git status --short");
            ok("git diff HEAD~1");
            ok("git log --oneline -5");
            for denied in [
                "add serde",
                "remove serde",
                "rm serde",
                "install x",
                "publish",
                "yank",
                "login",
                "logout",
                "owner --add x",
                "uninstall x",
            ] {
                let e = no(&format!("cargo {denied}"));
                assert!(e.contains("for the person"), "{e}");
            }
            for denied in [
                "push",
                "commit -m x",
                "checkout main",
                "reset --hard",
                "branch x",
                "stash",
                "fetch",
                "pull",
                "clean -fd",
            ] {
                let e = no(&format!("git {denied}"));
                assert!(e.contains("only reads"), "{e}");
            }
            assert!(no("git").contains("only reads"));
            assert!(no("/usr/bin/git status").contains("not a path"));
            assert!(no("./cargo test").contains("not a path"));
            for other in [
                "sh -c ls",
                "bash",
                "python3 x.py",
                "rm -rf x",
                "curl http://x",
                "ls",
            ] {
                let e = no(other);
                assert!(
                    e.contains("not a program imece runs") && e.contains("--run"),
                    "{e}"
                );
            }
        }
        // Leaders change nothing: cargo's writing commands and the person's
        // programs are for workers and helpers.
        for writes in [
            "fmt",
            "fmt --all",
            "+nightly fmt",
            "--locked +nightly fmt",
            "fix",
            "clean",
            "new x",
            "init",
        ] {
            let e = allowed(Role::Leader, &argv(&format!("cargo {writes}")), &extra).unwrap_err();
            assert!(e.contains("changes files"), "{e}");
            allowed(Role::Worker, &argv(&format!("cargo {writes}")), &extra).unwrap();
            allowed(Role::Helper, &argv(&format!("cargo {writes}")), &[]).unwrap();
        }
        for program in ["npm test", "make check"] {
            let e = allowed(Role::Leader, &argv(program), &extra).unwrap_err();
            assert!(e.contains("these are: cargo, git ("), "{e}");
            allowed(Role::Worker, &argv(program), &extra).unwrap();
            let e = allowed(Role::Worker, &argv(program), &[]).unwrap_err();
            assert!(e.contains("these are: cargo, git ("), "{e}");
        }
        let e = allowed(Role::Worker, &argv("yarn"), &extra).unwrap_err();
        assert!(e.contains("cargo, git, npm, make"), "{e}");
    }

    #[test]
    fn a_run_s_call_waits_past_its_deadline_and_the_rest_a_short_while() {
        let t = Duration::from_secs(600);
        assert_eq!(call_wait("run", t), Duration::from_secs(630));
        for tool in ["send", "post", "board", "claim", "hire"] {
            assert_eq!(call_wait(tool, t), CALL_WAIT, "{tool}");
        }
    }

    /// A session that says when it was ended.
    struct Ends(Arc<AtomicBool>);
    impl imece_agent::Agent for Ends {
        fn prompt(&mut self, _: &str) -> io::Result<()> {
            Ok(())
        }
        fn command(&mut self, _: &str) -> io::Result<()> {
            Ok(())
        }
        fn terminate(&mut self) {
            self.0.store(true, Ordering::SeqCst);
        }
    }

    #[test]
    fn dropping_imece_ends_every_session_and_kills_every_run() {
        use std::os::unix::process::{CommandExt, ExitStatusExt};
        let mut w = team(1);
        let mut live = idle_live(2);
        let ended = Arc::new(AtomicBool::new(false));
        live.ready(
            &mut w,
            1,
            Ok((Box::new(Ends(ended.clone())), PathBuf::from("/w"), None)),
        );
        let mut sleeper = std::process::Command::new("sleep")
            .arg("30")
            .process_group(0)
            .spawn()
            .unwrap();
        live.running[0].add(sleeper.id() as i32);
        let mut other = std::process::Command::new("sleep")
            .arg("30")
            .process_group(0)
            .spawn()
            .unwrap();
        live.running[1].add(other.id() as i32);
        drop(live);
        assert!(ended.load(Ordering::SeqCst), "the session was not ended");
        for s in [&mut sleeper, &mut other] {
            let status = s.wait().unwrap();
            assert_eq!(status.signal(), Some(libc::SIGKILL), "{status:?}");
        }
    }

    #[test]
    fn the_run_tool_names_what_each_role_may_run_and_the_deadline() {
        let extra = vec!["npm".to_string()];
        let t = Duration::from_secs(120);
        let lead = run_tool(Role::Leader, &extra, t);
        assert_eq!(lead.name, "run");
        assert!(
            lead.description.contains("fmt --check"),
            "{}",
            lead.description
        );
        assert!(!lead.description.contains("npm"));
        assert!(lead.description.contains("killed after 120 s"));
        assert_eq!(lead.input["required"], json!(["command"]));
        let worker = run_tool(Role::Worker, &extra, t);
        assert!(
            worker.description.contains("; npm."),
            "{}",
            worker.description
        );
        assert!(
            worker
                .description
                .contains("not add, remove, install or publish")
        );
        assert!(
            worker
                .description
                .contains("git (blame, diff, grep, log, ls-files, rev-parse, show, status only)")
        );
    }

    #[test]
    fn a_run_is_checked_on_the_screen_s_loop_and_answered_from_its_own_thread() {
        let mut w = team(2);
        let mut live = idle_live(3);
        live.run_programs = vec!["sh".into()];
        live.run_timeout = Duration::from_secs(5);
        // No session, no directory.
        let e = call(
            &mut live,
            &mut w,
            1,
            "run",
            json!({ "command": "git status" }),
        )
        .unwrap_err();
        assert!(e.contains("no directory"), "{e}");
        // A command may be MAX_COMMAND characters long, not one more.
        let long = |n: usize| json!({ "command": format!("git log {}", "x".repeat(n - 8)) });
        let e = call(&mut live, &mut w, 1, "run", long(MAX_COMMAND)).unwrap_err();
        assert!(e.contains("no directory"), "{e}");
        let e = call(&mut live, &mut w, 1, "run", long(MAX_COMMAND + 1)).unwrap_err();
        assert!(e.contains("longer than 1000"), "{e}");
        let dir = std::env::temp_dir().join(format!("imece-run-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        live.cwds[1] = Some(dir.clone());
        live.cwds[0] = Some(dir.clone());
        for (args, says) in [
            (json!({}), "what to run"),
            (json!({ "command": "   " }), "what to run"),
            (json!({ "command": "a".repeat(1001) }), "longer than 1000"),
            (json!({ "command": "sh -c 'ls' | head" }), "no shell"),
            (json!({ "command": "git push" }), "only reads"),
            (json!({ "command": "ls" }), "not a program imece runs"),
        ] {
            let e = call(&mut live, &mut w, 1, "run", args).unwrap_err();
            assert!(e.contains(says), "{e}");
        }
        assert!(
            w.agents[1]
                .trace
                .iter()
                .any(|l| l.contains("run refused: git push")),
            "{:?}",
            w.agents[1].trace
        );
        // The leader may not run the person's programs.
        let e = call(
            &mut live,
            &mut w,
            0,
            "run",
            json!({ "command": "sh -c 'pwd'" }),
        )
        .unwrap_err();
        assert!(e.contains("not a program imece runs"), "{e}");
        // A worker's run happens in its directory, and the answer carries
        // the exit code and the output.
        let got = call(
            &mut live,
            &mut w,
            1,
            "run",
            json!({ "command": "sh -c 'pwd; echo err >&2; exit 2'" }),
        )
        .unwrap();
        assert_eq!(
            got,
            format!(
                "exit code 2 (failed)\n{}\nerr",
                dir.canonicalize().unwrap().display()
            )
        );
        let got = call(
            &mut live,
            &mut w,
            1,
            "run",
            json!({ "command": "sh -c 'exit 0'" }),
        )
        .unwrap();
        assert_eq!(got, "exit code 0, no output");
        // The trace says what ran and how it ended, once the loop ran again.
        live.step(&mut w);
        let trace = w.agents[1]
            .trace
            .iter()
            .cloned()
            .collect::<Vec<_>>()
            .join("\n");
        assert!(trace.contains("run: sh -c 'exit 0'\n"), "{trace}");
        assert!(trace.contains("run: sh -c 'exit 0': exit 0, "), "{trace}");
        assert!(trace.contains(" s, 0 bytes"), "{trace}");
        assert!(live.running.iter().all(run::Running::is_empty));
        // A program that can not start is an error, not an exit code.
        live.run_programs.push("imece-no-such-program".into());
        let e = call(
            &mut live,
            &mut w,
            1,
            "run",
            json!({ "command": "imece-no-such-program" }),
        )
        .unwrap_err();
        assert!(e.contains("could not start imece-no-such-program"), "{e}");
        // Past the deadline the run is killed and says so.
        live.run_timeout = Duration::from_millis(200);
        let got = call(
            &mut live,
            &mut w,
            1,
            "run",
            json!({ "command": "sh -c 'echo a; sleep 20'" }),
        )
        .unwrap();
        assert!(
            got.starts_with("killed after 0 s: it did not finish in time\na"),
            "{got}"
        );
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn the_repository_s_notes_for_agents_go_into_the_prompt() {
        let dir = std::env::temp_dir().join(format!("imece-notes-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        assert_eq!(repo_notes(&dir, Vendor::Claude), None);
        std::fs::write(dir.join("CLAUDE.md"), "  \n").unwrap();
        assert_eq!(
            repo_notes(&dir, Vendor::Claude),
            None,
            "an empty file is none"
        );
        std::fs::write(dir.join("CLAUDE.md"), "# notes\n\nRun cargo fmt.\n").unwrap();
        let notes = repo_notes(&dir, Vendor::Claude).unwrap();
        assert!(notes.starts_with("\n\nThe repository's notes"), "{notes}");
        assert!(notes.contains("from its CLAUDE.md:\n\n# notes\n\nRun cargo fmt."));
        assert!(notes.ends_with("cargo fmt."), "{notes:?}");
        // Codex reads AGENTS.md on its own; the others get it from imece.
        assert!(
            repo_notes(&dir, Vendor::Codex)
                .unwrap()
                .contains("CLAUDE.md")
        );
        std::fs::write(dir.join("AGENTS.md"), "agents first").unwrap();
        for v in [Vendor::Claude, Vendor::Gemini] {
            let notes = repo_notes(&dir, v).unwrap();
            assert!(
                notes.contains("from its AGENTS.md:\n\nagents first"),
                "{notes}"
            );
        }
        assert_eq!(repo_notes(&dir, Vendor::Codex), None);
        // A file of exactly the cap is whole; one byte more is cut at a
        // character boundary, and says so.
        std::fs::write(dir.join("AGENTS.md"), "x".repeat(MAX_NOTES)).unwrap();
        let notes = repo_notes(&dir, Vendor::Claude).unwrap();
        assert!(!notes.contains("[cut"), "{}", &notes[notes.len() - 40..]);
        assert_eq!(notes.matches('x').count(), MAX_NOTES);
        let long = format!("{}é", "x".repeat(MAX_NOTES - 1));
        std::fs::write(dir.join("AGENTS.md"), &long).unwrap();
        let notes = repo_notes(&dir, Vendor::Claude).unwrap();
        assert!(
            notes.ends_with("x\n[cut: the file goes on]"),
            "{}",
            &notes[notes.len() - 40..]
        );
        assert!(!notes.contains('é'));
        assert_eq!(notes.matches('x').count(), MAX_NOTES - 1);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn where_a_worktree_stands_goes_into_the_journal() {
        let mut w = team(2);
        let mut live = idle_live(3);
        let cwd = PathBuf::from("/work/claude-1");
        let placed = |p| Ok((Box::new(Stub) as Box<dyn Session>, cwd.clone(), p));
        live.ready(&mut w, 1, placed(Some(worktree::Placed::Fresh)));
        live.ready(&mut w, 1, placed(Some(worktree::Placed::AtHead)));
        live.ready(&mut w, 0, placed(None));
        assert!(w.journal.is_empty(), "{:?}", w.journal);
        assert!(matches!(live.slots[1], Slot::Running(_)));
        live.ready(
            &mut w,
            1,
            placed(Some(worktree::Placed::Moved {
                from: "aaaaaaa".into(),
                to: "bbbbbbb".into(),
            })),
        );
        assert!(matches!(
            &w.journal.last().unwrap().event,
            Event::WorktreeMoved { agent: 1, from, to } if from == "aaaaaaa" && to == "bbbbbbb"
        ));
        for (why, commits) in [
            (worktree::Why::Changes, false),
            (worktree::Why::Commits, true),
        ] {
            live.ready(
                &mut w,
                2,
                placed(Some(worktree::Placed::Kept {
                    at: "aaaaaaa".into(),
                    head: "bbbbbbb".into(),
                    why,
                })),
            );
            assert!(matches!(
                &w.journal.last().unwrap().event,
                Event::WorktreeKept { agent: 2, at, head, commits: c }
                    if at == "aaaaaaa" && head == "bbbbbbb" && *c == commits
            ));
        }
        assert_eq!(w.journal.len(), 3);
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

    fn journaled_replies(w: &World) -> Vec<(AgentId, String)> {
        w.journal
            .iter()
            .filter_map(|e| match &e.event {
                Event::Replied { agent, text } => Some((*agent, text.clone())),
                _ => None,
            })
            .collect()
    }

    #[test]
    fn only_a_text_that_reaches_the_person_is_journaled_as_a_reply() {
        let mut w = team(1);
        let mut live = idle_live(2);
        // The worker's turn on its mail: it sent done (journaled as such)
        // and its closing text restates it.
        live.turns[1] = 1;
        live.send(&mut w, 1, &msg("lead", "done", "Parser fixed."))
            .unwrap();
        finish(&mut live, &mut w, 1, "Fixed the parser and told lead.");
        assert_eq!(answers(&w), []);
        assert_eq!(journaled_replies(&w), []);
        assert!(
            matches!(
                &w.journal.last().unwrap().event,
                Event::Sent { from: 1, to: 0, .. }
            ),
            "{:?}",
            w.journal.last()
        );
        // The person asked the leader: its answer is a reply, once.
        w.said(0, "how far?");
        live.asked[0] = true;
        live.turns[0] = 1;
        finish(&mut live, &mut w, 0, "Nearly there.");
        assert_eq!(answers(&w), [(0, "Nearly there.".to_string())]);
        assert_eq!(journaled_replies(&w), [(0, "Nearly there.".to_string())]);
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
        w.agents[0].cost = w.budget;
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
        assert_eq!(names, ["send", "post", "board", "run", "hire"]);
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
    fn only_workers_get_the_claim_tool_and_only_the_leader_and_workers_hire() {
        let names = |r: Role| -> Vec<String> {
            tools_of(r, &["claude"], &[], DEFAULT_RUN_TIMEOUT)
                .into_iter()
                .map(|t| t.name)
                .collect()
        };
        assert_eq!(
            names(Role::Leader),
            ["send", "post", "board", "run", "hire"]
        );
        assert_eq!(
            names(Role::Worker),
            ["send", "post", "board", "run", "claim", "hire"]
        );
        assert_eq!(names(Role::Helper), ["send", "post", "board", "run"]);
        assert_eq!(names(Role::Researcher), ["send", "post", "board", "run"]);
        // The same tool, told differently: a worker hires helpers.
        let worker_hire = &tools_of(Role::Worker, &["claude"], &[], DEFAULT_RUN_TIMEOUT)[5];
        assert!(worker_hire.description.starts_with("Adds a helper"));
        let hire = &tools_of(Role::Leader, &["claude", "codex"], &[], DEFAULT_RUN_TIMEOUT)[4];
        assert!(hire.description.starts_with("Adds a worker"));
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
    fn the_tick_is_the_time_since_the_world_began_in_whole_ticks() {
        let mut w = team(1);
        let mut live = idle_live(2);
        // The world began 13 seconds ago (in an earlier run, maybe).
        w.started_at = now() - 13;
        live.step(&mut w);
        assert_eq!(w.tick, 13 / SECONDS_PER_TICK);
        w.started_at = now() - 7 * 86_400;
        live.step(&mut w);
        assert_eq!(w.tick, 7 * 86_400 / SECONDS_PER_TICK);
        w.started_at = now() + 60;
        live.step(&mut w);
        assert_eq!(w.tick, 0, "a clock set back does not underflow");
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
        let mut w = team(2);
        let lead = system_prompt(&w, 0, 4, 2);
        assert!(lead.starts_with("You are lead, the leader"), "{lead}");
        assert!(
            lead.contains("The workers so far are claude-1, claude-2."),
            "{lead}"
        );
        assert!(lead.contains("hire tool, up to 4;"), "{lead}");
        let worker = system_prompt(&w, 2, 4, 2);
        assert!(
            worker.starts_with("You are claude-2, a coding agent"),
            "{worker}"
        );
        assert!(worker.contains("led by lead"));
        assert!(
            worker.contains("hire helpers with the hire tool, up to 2 at once"),
            "{worker}"
        );
        let no_helpers = system_prompt(&w, 2, 4, 0);
        assert!(!no_helpers.contains("hire"), "{no_helpers}");
        let helper = with_helper(&mut w, 1);
        let helper = system_prompt(&w, helper, 4, 2);
        assert!(
            helper.starts_with("You are claude-1.1, a helper of claude-1"),
            "{helper}"
        );
        assert!(helper.contains("led by lead") && helper.contains("claude-1's git worktree"));
        assert!(!helper.contains("hire"), "{helper}");
        let alone = system_prompt(&team(0), 0, 4, 2);
        assert!(alone.contains("You have no workers yet."), "{alone}");
        let none = system_prompt(&team(0), 0, 0, 2);
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
        let helper = with_helper(&mut w, 1);
        let mut live = idle_live(3);
        let err = live.hire(&mut w, helper, &json!({})).unwrap_err();
        assert!(err.contains("only the leader and workers"), "{err}");
        live.max_helpers = 0;
        let err = live.hire(&mut w, 1, &json!({})).unwrap_err();
        assert!(err.contains("without helpers"), "{err}");
        live.max_helpers = usize::MAX;
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
        assert_eq!(w.agents.len(), 3);
        assert!(w.journal.is_empty());
    }

    /// A helper of worker `parent`, as `hire` would add it; its id.
    fn with_helper(w: &mut World, parent: AgentId) -> AgentId {
        let id = w.agents.len();
        let name = format!(
            "{}.{}",
            w.agents[parent].name,
            w.helpers_of(parent).count() + 1
        );
        let mut a = Agent::new(id, 0, name, Vendor::Claude, Role::Helper, Status::Idle, 0);
        a.parent = Some(parent);
        w.agents.push(a);
        id
    }

    #[test]
    fn a_worker_hires_helpers_named_after_it_up_to_its_cap_and_they_hire_nobody() {
        let mut w = team(2);
        let mut live = idle_live(3);
        live.max_workers = 2;
        live.max_helpers = 2;
        let got = live.hire(&mut w, 1, &json!({ "tier": "cheap" })).unwrap();
        assert!(
            got.starts_with("claude-1.1 joined (claude, haiku)")
                && got.contains("1 more may be hired"),
            "{got}"
        );
        let h = &w.agents[3];
        assert_eq!(
            (h.name.as_str(), h.role, h.parent, h.project, h.card),
            ("claude-1.1", Role::Helper, Some(1), 0, None)
        );
        assert_eq!(live.models[3].as_deref(), Some("haiku"));
        assert!(matches!(
            w.journal.last().map(|e| &e.event),
            Some(Event::Hired { by: 1, agent: 3, model: Some(m) }) if m == "haiku"
        ));
        let got = live.hire(&mut w, 1, &json!({})).unwrap();
        assert!(
            got.starts_with("claude-1.2 joined") && got.contains("0 more"),
            "{got}"
        );
        // Exactly the cap, per worker: the other worker still may.
        let err = live.hire(&mut w, 1, &json!({})).unwrap_err();
        assert!(
            err.contains("you have 2 helpers") && err.contains("--helpers 2"),
            "{err}"
        );
        let got = live.hire(&mut w, 2, &json!({})).unwrap();
        assert!(got.starts_with("claude-2.1 joined"), "{got}");
        // Helpers are not workers: the leader's cap is untouched by them,
        // and they are not offered cards.
        let err = live.hire(&mut w, 0, &json!({})).unwrap_err();
        assert!(err.contains("the team is full: 2 workers"), "{err}");
        assert_eq!(w.helpers_of(1).count(), 2);
        assert_eq!(w.helpers_of(2).count(), 1);
        assert_eq!(w.helpers_of(0).count(), 0);
        let err = live.hire(&mut w, 3, &json!({})).unwrap_err();
        assert!(err.contains("only the leader and workers"), "{err}");
    }

    #[test]
    fn a_helper_that_sent_done_to_its_worker_leaves_when_its_turn_ends() {
        let mut w = team(1);
        let mut live = idle_live(2);
        live.hire(&mut w, 1, &json!({})).unwrap();
        // The worker is busy, so the mail waits and nothing starts.
        live.turns[1] = 1;
        live.turns[2] = 1;
        // done to someone else is a message like any other.
        live.send(&mut w, 2, &msg("lead", "done", "fyi")).unwrap();
        assert!(!live.leaving[2]);
        let got = live
            .send(&mut w, 2, &msg("claude-1", "done", "Tests written."))
            .unwrap();
        assert!(got.starts_with("queued; claude-1 is busy"), "{got}");
        assert!(live.leaving[2], "leaves when the turn ends, not now");
        assert!(!w.agents[2].gone);
        // Mail that came for it meanwhile goes nowhere.
        live.mail[2].push(Mail {
            from: 1,
            kind: MsgKind::Inform,
            body: "x".into(),
        });
        finish(&mut live, &mut w, 2, "done");
        let h = &w.agents[2];
        assert!(h.gone && h.status == Status::Idle && h.talk.is_none());
        assert!(!live.leaving[2]);
        assert!(live.mail[2].is_empty());
        assert_eq!(live.generation[2], 1, "its session ended");
        assert!(
            w.journal
                .iter()
                .any(|e| matches!(e.event, Event::Left { agent: 2 }))
        );
        assert!(
            h.trace.iter().any(|l| l.ends_with(" left")),
            "{:?}",
            h.trace
        );
        assert_eq!(w.agent_by_name("claude-1.1"), None);
        // A gone helper takes no mail and no turn.
        live.mail[2].push(Mail {
            from: 1,
            kind: MsgKind::Assign,
            body: "more".into(),
        });
        live.step(&mut w);
        assert_eq!(live.turns[2], 0);
        assert!(matches!(live.slots[2], Slot::None));
        // The worker still got the done, and the next helper is numbered on.
        assert_eq!(live.mail[1].len(), 1);
        assert_eq!(live.mail[1][0].kind, MsgKind::Done);
        let got = live.hire(&mut w, 1, &json!({})).unwrap();
        assert!(got.starts_with("claude-1.2 joined"), "{got}");
    }

    #[test]
    fn a_worker_that_stops_takes_its_helpers_with_it() {
        let mut w = team(2);
        let mut live = idle_live(3);
        live.hire(&mut w, 1, &json!({})).unwrap();
        live.hire(&mut w, 2, &json!({})).unwrap();
        live.tx
            .send((1, 0, AgentEvent::Stopped("gone".into())))
            .unwrap();
        live.step(&mut w);
        assert_eq!(w.agents[1].status, Status::Stopped);
        assert!(w.agents[3].gone, "claude-1's helper");
        assert!(!w.agents[4].gone, "claude-2's helper stays");
        assert!(
            w.journal
                .iter()
                .any(|e| matches!(e.event, Event::Left { agent: 3 }))
        );
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
        assert_eq!(names, ["send", "post", "board", "run", "claim", "hire"]);
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
            live.claude_home = trees(&format!("fallback-home-{turned}"));
            live.sessions[1] = Some(Resumable {
                session: UUID.into(),
                spent: 0,
            });
            let file = transcript_of(&live, &w, 1, UUID);
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
            assert!(file.is_file(), "no fallback, the session stays remembered");
            assert_eq!(
                live.sessions[1].as_ref().map(|s| s.session.as_str()),
                Some(UUID)
            );
            let _ = std::fs::remove_dir_all(&live.claude_home);
        }
        // A first turn that cost nothing: the model is not here, the
        // session on it is forgotten, transcript and all.
        let mut w = team(0);
        let mut live = idle_live(1);
        live.hire(&mut w, 0, &json!({ "tier": "cheap" })).unwrap();
        live.turns[1] = 1;
        live.claude_home = trees("fallback-home");
        live.sessions[1] = Some(Resumable {
            session: UUID.into(),
            spent: 0,
        });
        let file = transcript_of(&live, &w, 1, UUID);
        live.tx
            .send((
                1,
                0,
                AgentEvent::TurnDone {
                    ok: false,
                    text: "error".into(),
                    cost: 0,
                    denied: 0,
                },
            ))
            .unwrap();
        live.step(&mut w);
        assert_eq!(live.models[1], None, "the standard model");
        assert_eq!(live.sessions[1], None);
        assert!(
            gone_soon(&file),
            "the transcript of the failed session goes"
        );
        let _ = std::fs::remove_dir_all(&live.claude_home);
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
        let h = Hireable::claude("claude", &[]);
        assert_eq!(h.ladder.model(Tier::Cheap), Some("haiku"));
        assert_eq!(h.ladder.model(Tier::Standard), None);
        assert_eq!(h.ladder.model(Tier::Strong), Some("opus"));
        assert_eq!(h.program, "claude");
        assert_eq!(h.vendor, Vendor::Claude);
        let h = Hireable::claude(
            "claude",
            &[
                (Tier::Strong, "opus[1m]".into()),
                (Tier::Standard, "sonnet".into()),
                (Tier::Cheap, "sonnet".into()),
            ],
        );
        assert_eq!(h.ladder.model(Tier::Cheap), Some("sonnet"));
        assert_eq!(h.ladder.model(Tier::Standard), Some("sonnet"));
        assert_eq!(h.ladder.model(Tier::Strong), Some("opus[1m]"));
        let h = Hireable::claude("claude", &[(Tier::Standard, "opus".into())]);
        assert_eq!(h.ladder.model(Tier::Standard), Some("opus"));
        // Codex's ladder is efforts on its default model.
        let h = Hireable::codex("codex", &[]);
        assert_eq!(h.ladder.model(Tier::Cheap), Some("@low"));
        assert_eq!(h.ladder.model(Tier::Standard), None);
        assert_eq!(h.ladder.model(Tier::Strong), Some("@high"));
        assert_eq!((h.program.as_str(), h.vendor), ("codex", Vendor::Codex));
        // Gemini's ladder is its own model aliases.
        let h = Hireable::gemini("gemini", &[(Tier::Strong, "gemini-2.5-pro".into())]);
        assert_eq!(h.ladder.model(Tier::Cheap), Some("flash"));
        assert_eq!(h.ladder.model(Tier::Standard), None);
        assert_eq!(h.ladder.model(Tier::Strong), Some("gemini-2.5-pro"));
        assert_eq!((h.program.as_str(), h.vendor), ("gemini", Vendor::Gemini));
    }

    #[test]
    fn the_person_s_stand_ins_go_to_their_vendor_and_bare_ones_to_the_leader_s() {
        let tiers = vec![
            (None, Tier::Strong, "opus[1m]".to_string()),
            (
                Some(Vendor::Codex),
                Tier::Cheap,
                "gpt-5.6-luna@low".to_string(),
            ),
            (Some(Vendor::Claude), Tier::Standard, "sonnet".to_string()),
        ];
        let team = team_vendors(vec![Vendor::Claude, Vendor::Codex], Vendor::Claude, &tiers);
        assert_eq!(team.len(), 2);
        let (claude, codex) = (&team[0], &team[1]);
        assert_eq!(
            (claude.vendor, claude.program.as_str()),
            (Vendor::Claude, "claude")
        );
        assert_eq!(
            claude.ladder.model(Tier::Strong),
            Some("opus[1m]"),
            "bare: the leader's"
        );
        assert_eq!(claude.ladder.model(Tier::Standard), Some("sonnet"));
        assert_eq!(claude.ladder.model(Tier::Cheap), Some("haiku"));
        assert_eq!(
            (codex.vendor, codex.program.as_str()),
            (Vendor::Codex, "codex")
        );
        assert_eq!(codex.ladder.model(Tier::Cheap), Some("gpt-5.6-luna@low"));
        assert_eq!(
            codex.ladder.model(Tier::Strong),
            Some("@high"),
            "not the bare one"
        );
        // With codex leading, the bare stand-in is codex's.
        let team = team_vendors(vec![Vendor::Claude, Vendor::Codex], Vendor::Codex, &tiers);
        assert_eq!(team[1].ladder.model(Tier::Strong), Some("opus[1m]"));
        assert_eq!(team[0].ladder.model(Tier::Strong), Some("opus"));
        assert!(team_vendors(vec![], Vendor::Claude, &tiers).is_empty());
    }

    #[test]
    fn only_vendors_imece_drives_could_lead() {
        assert_eq!(
            leaders(&[
                (Vendor::Codex, false),
                (Vendor::Claude, true),
                (Vendor::Gemini, true)
            ]),
            [Vendor::Claude, Vendor::Gemini]
        );
        assert!(leaders(&[]).is_empty());
        assert!(leaders(&[(Vendor::Codex, false)]).is_empty());
    }

    #[test]
    fn the_team_needs_a_leader_imece_can_drive() {
        let err = hireable(&[], Vendor::Claude).unwrap_err();
        assert!(
            err.contains("no coding agent") && err.contains("claude, codex, gemini"),
            "{err}"
        );
        let err = hireable(&[(Vendor::Gemini, false)], Vendor::Claude).unwrap_err();
        assert!(
            err.contains("claude is not on PATH (found gemini)") && err.contains("--lead"),
            "{err}"
        );
        let err = hireable(
            &[(Vendor::Claude, true), (Vendor::Gemini, false)],
            Vendor::Gemini,
        )
        .unwrap_err();
        assert!(
            err.contains("gemini is on PATH, but") && err.contains("--lead claude"),
            "{err}"
        );
        let err = hireable(
            &[
                (Vendor::Claude, true),
                (Vendor::Codex, true),
                (Vendor::Gemini, false),
            ],
            Vendor::Gemini,
        )
        .unwrap_err();
        assert!(err.contains("--lead claude or --lead codex"), "{err}");
        let err = hireable(&[(Vendor::Gemini, false)], Vendor::Gemini).unwrap_err();
        assert!(err.contains("--lead claude"), "{err}");
        let ok = hireable(
            &[
                (Vendor::Claude, true),
                (Vendor::Codex, true),
                (Vendor::Gemini, false),
            ],
            Vendor::Codex,
        )
        .unwrap();
        assert_eq!(
            ok,
            [Vendor::Claude, Vendor::Codex],
            "only vendors imece drives are hireable"
        );
    }

    fn two_vendors(dir: &Path) -> Vec<Hireable> {
        vec![
            Hireable::claude(&fake_claude(dir), &[]),
            Hireable::codex(&fake_codex(dir), &[]),
        ]
    }

    #[test]
    fn the_leader_hires_a_codex_worker_named_after_codex_on_an_effort() {
        let mut w = team(0);
        let mut live = Live::new(
            1,
            two_vendors(&std::env::temp_dir()),
            std::env::temp_dir(),
            PathBuf::from("/nonexistent"),
        );
        let got = live
            .hire(&mut w, 0, &json!({ "vendor": "codex", "tier": "cheap" }))
            .unwrap();
        assert!(got.starts_with("codex-1 joined (codex, @low)."), "{got}");
        assert_eq!(w.agents[1].vendor, Vendor::Codex);
        assert_eq!(live.models[1].as_deref(), Some("@low"));
        assert!(live.tiered[1]);
        let got = live.hire(&mut w, 0, &json!({ "vendor": "codex" })).unwrap();
        assert!(
            got.starts_with("codex-2 joined (codex, the default model)."),
            "{got}"
        );
        assert!(!live.tiered[2]);
        // A claude worker counts its own.
        let got = live.hire(&mut w, 0, &json!({})).unwrap();
        assert!(got.starts_with("claude-1 joined"), "{got}");
        // The hire tool offers both.
        let hire = &tools_of(Role::Leader, &live.vendor_names(), &[], DEFAULT_RUN_TIMEOUT)[4];
        assert_eq!(
            hire.input["properties"]["vendor"]["enum"],
            json!(["claude", "codex"])
        );
        // A codex worker's prompt says what it can do.
        let worker = system_prompt(&w, 1, 4, 2);
        assert!(
            worker.contains("run commands; the sandbox keeps writes inside"),
            "{worker}"
        );
        let claude = system_prompt(&w, 3, 4, 2);
        assert!(
            claude.contains("the run tool runs the build, the tests"),
            "{claude}"
        );
    }

    /// A stand-in for `codex` that writes its arguments where it runs and
    /// exits, which ends the session.
    fn fake_codex(dir: &Path) -> String {
        use std::os::unix::fs::PermissionsExt;
        let path = dir.join("fake-codex");
        std::fs::write(&path, "#!/bin/sh\nprintf '%s\\n' \"$@\" > args.txt\n").unwrap();
        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o700)).unwrap();
        path.to_string_lossy().into_owned()
    }

    #[test]
    fn a_codex_leader_s_session_reads_only_and_a_worker_s_writes_its_worktree() {
        let dir = std::env::temp_dir().join(format!("imece-codex-lead-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let repo = checkout("codex-repo");
        let mut w = team(0);
        w.agents[0].vendor = Vendor::Codex;
        let mut live = Live::new(1, two_vendors(&dir), repo.clone(), dir.join("worktrees"));
        live.models[0] = Some("gpt-5.6-terra@high".into());
        live.told(&mut w, 0, "hello").unwrap();
        wait_for(&mut live, &mut w, |w| w.agents[0].status == Status::Stopped);
        let args = std::fs::read_to_string(repo.join("args.txt")).unwrap();
        assert!(args.starts_with("app-server\n"), "{args}");
        assert!(args.contains("sandbox_mode=\"read-only\""), "{args}");
        assert!(args.contains("approval_policy=\"never\""), "{args}");
        assert!(
            !args.contains("hello") && !args.contains("You are"),
            "no prompt in argv"
        );
        assert!(
            !args.contains("--model"),
            "the model goes to the thread, not argv"
        );
        let _ = std::fs::remove_file(repo.join("args.txt"));

        live.hire(&mut w, 0, &json!({ "vendor": "codex" })).unwrap();
        live.told(&mut w, 1, "hello").unwrap();
        wait_for(&mut live, &mut w, |w| w.agents[1].status == Status::Stopped);
        let tree = dir.join("worktrees").join("codex-1");
        let args = std::fs::read_to_string(tree.join("args.txt")).unwrap();
        assert!(args.contains("sandbox_mode=\"workspace-write\""), "{args}");
        assert!(
            args.contains("sandbox_workspace_write.network_access=false"),
            "{args}"
        );
        let _ = std::fs::remove_dir_all(&dir);
        let _ = std::fs::remove_dir_all(&repo);
    }

    fn fake_gemini(dir: &Path) -> String {
        use std::os::unix::fs::PermissionsExt;
        let path = dir.join("fake-gemini");
        std::fs::write(&path, "#!/bin/sh\nprintf '%s\\n' \"$@\" > args.txt\n").unwrap();
        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o700)).unwrap();
        path.to_string_lossy().into_owned()
    }

    #[test]
    fn a_gemini_session_runs_in_a_private_directory_with_the_agent_s_files_included() {
        let dir = std::env::temp_dir().join(format!("imece-gemini-lead-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let repo = checkout("gemini-repo");
        let mut w = team(0);
        w.agents[0].vendor = Vendor::Gemini;
        let vendors = vec![
            Hireable::claude(&fake_claude(&dir), &[]),
            Hireable::gemini(&fake_gemini(&dir), &[]),
        ];
        let mut live = Live::new(1, vendors, repo.clone(), dir.join("worktrees"));
        live.open_bus(&w, "/opt/imece".into()).unwrap();
        live.models[0] = Some("pro".into());
        live.told(&mut w, 0, "hello").unwrap();
        wait_for(&mut live, &mut w, |w| w.agents[0].status == Status::Stopped);
        // Gemini ran in the private directory, not in the checkout.
        assert!(!repo.join("args.txt").exists());
        let home = dir.join("worktrees-gemini").join("lead");
        let args = std::fs::read_to_string(home.join("args.txt")).unwrap();
        assert!(args.starts_with("--acp\n"), "{args}");
        assert!(
            args.contains(&format!("--include-directories\n{}\n", repo.display())),
            "{args}"
        );
        assert!(
            args.contains(&format!("--policy\n{}\n", home.join("policy").display())),
            "{args}"
        );
        assert!(args.contains("-m\npro\n"), "{args}");
        assert!(
            !args.contains("hello") && !args.contains("You are"),
            "no prompt in argv"
        );
        let policy = std::fs::read_to_string(home.join("policy/imece.toml")).unwrap();
        assert!(
            policy.contains("\"read_file\"") && !policy.contains("write_file"),
            "{policy}"
        );
        assert!(
            policy.contains("\"mcp_imece_hire\"") && !policy.contains("claim"),
            "{policy}"
        );
        let instructions = std::fs::read_to_string(home.join("GEMINI.md")).unwrap();
        assert!(
            instructions.contains("You are lead")
                && instructions.contains(&repo.display().to_string()),
            "{instructions}"
        );

        live.hire(&mut w, 0, &json!({ "vendor": "gemini" }))
            .unwrap();
        live.told(&mut w, 1, "hello").unwrap();
        wait_for(&mut live, &mut w, |w| w.agents[1].status == Status::Stopped);
        let tree = dir.join("worktrees").join("gemini-1");
        let home = dir.join("worktrees-gemini").join("gemini-1");
        let args = std::fs::read_to_string(home.join("args.txt")).unwrap();
        assert!(
            args.contains(&format!(
                "--include-directories\n{}\n",
                tree.canonicalize().unwrap().display()
            )),
            "{args}"
        );
        let policy = std::fs::read_to_string(home.join("policy/imece.toml")).unwrap();
        assert!(
            policy.contains("\"write_file\"") && policy.contains("\"mcp_imece_claim\""),
            "{policy}"
        );
        let _ = std::fs::remove_dir_all(&dir);
        let _ = std::fs::remove_dir_all(&repo);
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
        std::fs::write(dir.join("CLAUDE.md"), "Run cargo fmt before you finish.").unwrap();
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
        // The prompt spans lines; the stand-in wrote one argument per line.
        let raw = std::fs::read_to_string(dir.join("args.txt")).unwrap();
        assert!(
            raw.contains("from its CLAUDE.md:\n\nRun cargo fmt before you finish.\n--model\n"),
            "the repository's notes end the prompt: {raw}"
        );
        assert!(!args.contains(&"--mcp-config"), "no bus, no MCP server");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn a_helper_s_session_runs_in_its_worker_s_worktree_with_edit_tools() {
        let dir = std::env::temp_dir().join(format!("imece-helper-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let repo = dir.join("repo");
        std::fs::create_dir_all(&repo).unwrap();
        let git = |args: &[&str]| {
            let out = std::process::Command::new("git")
                .args(args)
                .current_dir(&repo)
                .output()
                .unwrap();
            assert!(out.status.success(), "git {args:?}");
        };
        git(&["init", "--quiet"]);
        std::fs::write(repo.join("a.txt"), "a").unwrap();
        git(&["add", "."]);
        git(&[
            "-c",
            "user.name=t",
            "-c",
            "user.email=t@t",
            "-c",
            "commit.gpgsign=false",
            "commit",
            "--quiet",
            "-m",
            "init",
        ]);
        let mut w = team(1);
        let mut live = Live::new(
            2,
            claude_at(&fake_claude(&dir)),
            repo.clone(),
            dir.join("worktrees"),
        );
        live.hire(&mut w, 1, &json!({})).unwrap();
        live.told(&mut w, 2, "hello").unwrap();
        wait_for(&mut live, &mut w, |w| w.agents[2].status == Status::Stopped);
        // The stand-in wrote its arguments where it ran: the worker's tree.
        let tree = dir.join("worktrees").join("claude-1");
        let args = std::fs::read_to_string(tree.join("args.txt")).unwrap();
        let args: Vec<&str> = args.lines().collect();
        let after = |f: &str| args[args.iter().position(|a| *a == f).unwrap() + 1];
        assert_eq!(after("--tools"), "Edit,Glob,Grep,Read,Write");
        assert_eq!(after("--name"), "imece claude-1.1");
        assert!(after("--append-system-prompt").starts_with("You are claude-1.1, a helper"));
        assert!(
            !dir.join("worktrees").join("claude-1.1").exists(),
            "no tree of its own"
        );
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn a_program_is_on_path_when_its_version_runs() {
        let here = std::env::temp_dir();
        assert!(on_path("true", &here));
        assert!(!on_path("false", &here));
        assert!(!on_path("/nonexistent/claude", &here));
    }

    /// A fresh git checkout with one commit, under the temp directory.
    /// Git in `dir`, its first stdout line.
    fn git_in(dir: &Path, args: &[&str]) -> String {
        let out = std::process::Command::new("git")
            .args(args)
            .current_dir(dir)
            .env("LC_ALL", "C")
            .output()
            .unwrap();
        assert!(
            out.status.success(),
            "git {args:?}: {}",
            String::from_utf8_lossy(&out.stderr)
        );
        String::from_utf8_lossy(&out.stdout).trim().to_string()
    }

    /// A fresh directory for the worktrees of one test.
    fn trees(name: &str) -> PathBuf {
        let root = std::env::temp_dir().join(format!("imece-{name}-trees-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        root
    }

    /// A worker with a session in a worktree of `repo`, as after its
    /// first turn; gives the worktree.
    fn worker_in(live: &mut Live, w: &mut World, id: AgentId, repo: &Path, root: &Path) -> PathBuf {
        let name = w.agents[id].name.clone();
        let tree = worktree::ensure(repo, root, &name).unwrap().path;
        live.ready(w, id, Ok((Box::new(Stub), tree.clone(), None)));
        tree
    }

    #[test]
    fn a_worker_s_done_shows_the_person_its_change_which_apply_puts_in_the_checkout() {
        let repo = checkout("change-repo");
        let root = trees("change");
        let mut w = team(2);
        let mut live = Live::new(
            3,
            claude_at("/nonexistent/claude"),
            repo.clone(),
            root.clone(),
        );
        let tree = worker_in(&mut live, &mut w, 1, &repo, &root);
        // Nothing changed: the done says so, nothing waits.
        let got = live
            .send(&mut w, 1, &msg("lead", "done", "Nothing to do."))
            .unwrap();
        assert!(got.ends_with("; your worktree holds no change"), "{got}");
        assert!(w.open_inbox().is_empty());
        assert!(
            w.agents[1]
                .trace
                .iter()
                .any(|l| l.ends_with("change: none"))
        );
        // A change: the person sees it, the worker is told so.
        std::fs::write(tree.join("a.txt"), "a\nb\n").unwrap();
        std::fs::write(tree.join("new.txt"), "new\n").unwrap();
        let got = live
            .send(
                &mut w,
                1,
                &msg("lead", "done", "Added b and new.txt\nDetails."),
            )
            .unwrap();
        assert!(
            got.contains("the person sees your change (2 files, +3 −1)"),
            "{got}"
        );
        let open = w.open_inbox();
        assert_eq!(open.len(), 1);
        assert_eq!(w.inbox[open[0]].kind, InboxKind::Change);
        assert_eq!(w.inbox[open[0]].detail, "Added b and new.txt");
        let c = w.change_of(1).unwrap();
        assert_eq!(c.files.len(), 2);
        assert!(c.patch.contains("diff --git a/new.txt b/new.txt"));
        assert!(w.journal.iter().any(|e| matches!(
            e.event,
            Event::ChangeReady {
                agent: 1,
                files: 2,
                added: 3,
                removed: 1
            }
        )));
        assert!(
            w.agents[1]
                .trace
                .iter()
                .any(|l| l.ends_with("change: 2 files +3 −1"))
        );
        // The done still reached the leader.
        assert_eq!(live.mail[0].len(), 2);
        // A second done supersedes: one change waits, with everything.
        std::fs::write(tree.join("more.txt"), "more\n").unwrap();
        live.send(&mut w, 1, &msg("lead", "done", "And more."))
            .unwrap();
        assert_eq!(w.open_inbox().len(), 1);
        assert_eq!(w.change_of(1).unwrap().files.len(), 3);
        assert_eq!(live.changes[&1].files.len(), 3);

        // The person's own edit in a touched file: the change does not
        // apply, nothing is half done, it still waits.
        std::fs::write(repo.join("a.txt"), "theirs").unwrap();
        let e = live.apply_change(&mut w, 1).unwrap_err();
        assert!(e.contains("does not match index"), "{e}");
        assert_eq!(
            std::fs::read_to_string(repo.join("a.txt")).unwrap(),
            "theirs"
        );
        assert!(!repo.join("new.txt").exists());
        assert_eq!(w.open_inbox().len(), 1);
        assert!(live.changes.contains_key(&1));
        assert!(matches!(
            &w.journal.last().unwrap().event,
            Event::ApplyFailed { agent: 1, why } if why == &e
        ));
        assert!(live.notes[1].is_empty());
        std::fs::write(repo.join("a.txt"), "a").unwrap();

        // Applied: staged in the checkout, marked in the worktree, the
        // worker hears of it at its next turn.
        live.apply_change(&mut w, 1).unwrap();
        assert_eq!(
            std::fs::read_to_string(repo.join("a.txt")).unwrap(),
            "a\nb\n"
        );
        assert_eq!(
            git_in(&repo, &["diff", "--cached", "--name-only"]),
            "a.txt\nmore.txt\nnew.txt"
        );
        let mark = git_in(&tree, &["rev-parse", "--short=7", "HEAD"]);
        assert_eq!(
            git_in(&tree, &["log", "-1", "--format=%ae %s"]),
            "imece@localhost imece: the person applied: And more."
        );
        assert!(w.open_inbox().is_empty());
        assert!(w.change_of(1).is_none());
        assert!(!live.changes.contains_key(&1));
        assert!(matches!(
            &w.journal.last().unwrap().event,
            Event::Applied { agent: 1, files: 3, mark: m } if *m == mark
        ));
        assert_eq!(live.notes[1].len(), 1);
        assert!(live.notes[1][0].contains("applied your change (And more.)"));
        assert!(live.notes[1][0].contains(&mark));
        live.told(&mut w, 1, "Next task.").unwrap();
        let p = &live.inflight[1][0];
        assert!(
            p.starts_with("From imece (not from the person or a teammate):\n- The person applied"),
            "{p}"
        );
        assert!(p.ends_with("\n\nNext task."), "{p}");
        assert!(live.notes[1].is_empty(), "said once");
        live.told(&mut w, 1, "Again.").unwrap();
        assert_eq!(live.inflight[1][1], "Again.");
        // Nothing waits: applying again is refused, quietly.
        let e = live.apply_change(&mut w, 1).unwrap_err();
        assert!(e.contains("no change of theirs waits"), "{e}");
        let _ = std::fs::remove_dir_all(&root);
        let _ = std::fs::remove_dir_all(&repo);
    }

    #[test]
    fn a_rejected_change_stays_in_the_worktree_and_the_worker_waits_for_the_reasons() {
        let repo = checkout("reject-repo");
        let root = trees("reject");
        let mut w = team(1);
        let mut live = Live::new(
            2,
            claude_at("/nonexistent/claude"),
            repo.clone(),
            root.clone(),
        );
        let tree = worker_in(&mut live, &mut w, 1, &repo, &root);
        std::fs::write(tree.join("a.txt"), "changed").unwrap();
        live.send(&mut w, 1, &msg("lead", "done", "Changed a."))
            .unwrap();
        assert_eq!(w.open_inbox().len(), 1);
        live.reject_change(&mut w, 1);
        assert!(w.open_inbox().is_empty());
        assert!(w.change_of(1).is_none() && !live.changes.contains_key(&1));
        assert!(matches!(
            w.journal.last().unwrap().event,
            Event::Rejected { agent: 1 }
        ));
        assert_eq!(
            std::fs::read_to_string(tree.join("a.txt")).unwrap(),
            "changed"
        );
        assert_eq!(std::fs::read_to_string(repo.join("a.txt")).unwrap(), "a");
        assert_eq!(git_in(&tree, &["status", "--porcelain"]), "M a.txt");
        assert_eq!(live.notes[1].len(), 1);
        assert!(live.notes[1][0].contains("rejected your change (Changed a.)"));
        // Rejecting nothing is nothing.
        live.reject_change(&mut w, 1);
        assert_eq!(live.notes[1].len(), 2);
        let _ = std::fs::remove_dir_all(&root);
        let _ = std::fs::remove_dir_all(&repo);
    }

    #[test]
    fn only_a_worker_s_done_with_a_directory_is_read_as_a_change() {
        let repo = checkout("whose-repo");
        let root = trees("whose");
        let mut w = team(2);
        w.agents[2].role = Role::Helper;
        w.agents[2].parent = Some(1);
        w.agents[2].name = "claude-1.1".into();
        let mut live = Live::new(
            3,
            claude_at("/nonexistent/claude"),
            repo.clone(),
            root.clone(),
        );
        let tree = worker_in(&mut live, &mut w, 1, &repo, &root);
        live.ready(&mut w, 2, Ok((Box::new(Stub), tree.clone(), None)));
        live.ready(&mut w, 0, Ok((Box::new(Stub), repo.clone(), None)));
        std::fs::write(tree.join("a.txt"), "changed").unwrap();
        std::fs::write(repo.join("b.txt"), "b").unwrap();
        // A helper's done is its worker's business; a leader changes nothing;
        // a progress message is not a done.
        for (from, kind) in [(2, "done"), (0, "done"), (1, "progress")] {
            let to = if from == 0 { "claude-1" } else { "lead" };
            let got = live.send(&mut w, from, &msg(to, kind, "x")).unwrap();
            assert!(!got.contains("change"), "{from} {kind}: {got}");
        }
        assert!(w.open_inbox().is_empty());
        // A worker without a directory (no session yet) has nothing to read.
        live.cwds[1] = None;
        let got = live.send(&mut w, 1, &msg("lead", "done", "x")).unwrap();
        assert!(!got.contains("change"), "{got}");
        assert!(w.open_inbox().is_empty());
        // Too large to show: the worker is told, nothing waits.
        live.cwds[1] = Some(tree.clone());
        std::fs::write(tree.join("big.txt"), "x".repeat(MAX_PATCH + 1)).unwrap();
        let got = live.send(&mut w, 1, &msg("lead", "done", "x")).unwrap();
        assert!(got.contains("imece could not read your change"), "{got}");
        assert!(w.open_inbox().is_empty());
        assert!(
            w.agents[1]
                .trace
                .iter()
                .any(|l| l.contains("change: the change is"))
        );
        let _ = std::fs::remove_dir_all(&root);
        let _ = std::fs::remove_dir_all(&repo);
    }

    #[test]
    fn a_worker_s_clean_worktree_follows_the_checkout_between_turns() {
        let repo = checkout("follow-repo");
        let root = trees("follow");
        let mut w = team(2);
        w.agents[2].role = Role::Helper;
        w.agents[2].parent = Some(1);
        let mut live = Live::new(
            3,
            claude_at("/nonexistent/claude"),
            repo.clone(),
            root.clone(),
        );
        let tree = worker_in(&mut live, &mut w, 1, &repo, &root);
        let old = git_in(&repo, &["rev-parse", "HEAD"]);
        std::fs::write(repo.join("b.txt"), "b").unwrap();
        git_in(&repo, &["add", "."]);
        git_in(
            &repo,
            &[
                "-c",
                "user.name=t",
                "-c",
                "user.email=t@t",
                "-c",
                "commit.gpgsign=false",
                "commit",
                "-q",
                "-m",
                "second",
            ],
        );
        let new = git_in(&repo, &["rev-parse", "HEAD"]);
        let moved = |w: &World| {
            w.journal
                .iter()
                .filter(|e| matches!(e.event, Event::WorktreeMoved { agent: 1, .. }))
                .count()
        };
        // Mid-turn (its own or a helper's): not moved.
        live.turns[1] = 1;
        live.follow(&mut w, 1);
        live.turns[1] = 0;
        live.turns[2] = 1;
        live.follow(&mut w, 1);
        live.turns[2] = 0;
        assert_eq!(moved(&w), 0);
        assert_eq!(git_in(&tree, &["rev-parse", "HEAD"]), old);
        // The leader's directory is the checkout itself: never moved.
        live.ready(&mut w, 0, Ok((Box::new(Stub), repo.clone(), None)));
        live.follow(&mut w, 0);
        // Between turns: the next prompt finds the worktree at HEAD.
        live.told(&mut w, 1, "go").unwrap();
        assert_eq!(moved(&w), 1);
        assert_eq!(git_in(&tree, &["rev-parse", "HEAD"]), new);
        assert!(tree.join("b.txt").exists());
        // Already there: nothing more is said.
        live.turns[1] = 0;
        live.told(&mut w, 1, "go on").unwrap();
        assert_eq!(moved(&w), 1);
        // With changes of its own it stays, and a broken directory is a
        // trace line, not a crash.
        std::fs::write(repo.join("c.txt"), "c").unwrap();
        git_in(&repo, &["add", "."]);
        git_in(
            &repo,
            &[
                "-c",
                "user.name=t",
                "-c",
                "user.email=t@t",
                "-c",
                "commit.gpgsign=false",
                "commit",
                "-q",
                "-m",
                "third",
            ],
        );
        std::fs::write(tree.join("mine.txt"), "mine").unwrap();
        live.turns[1] = 0;
        live.told(&mut w, 1, "more").unwrap();
        assert_eq!(moved(&w), 1);
        assert_eq!(git_in(&tree, &["rev-parse", "HEAD"]), new);
        live.cwds[1] = Some(PathBuf::from("/nonexistent/tree"));
        live.turns[1] = 0;
        live.told(&mut w, 1, "still").unwrap();
        assert!(
            w.agents[1].trace.iter().any(|l| l.contains("worktree: ")),
            "{:?}",
            w.agents[1].trace
        );
        let _ = std::fs::remove_dir_all(&root);
        let _ = std::fs::remove_dir_all(&repo);
    }

    #[test]
    fn probing_finds_the_known_programs_that_run_from_the_checkout() {
        let repo = checkout("probe");
        let known: &[Known] = &[
            (Vendor::Claude, "true", true),
            (Vendor::Codex, "false", false),
            (Vendor::Gemini, "/nonexistent/gemini", false),
        ];
        assert_eq!(probe(&repo, known), Ok(vec![(Vendor::Claude, true)]));
        assert_eq!(probe(&repo, &[]), Ok(vec![]));
        // Outside a checkout there is nothing to run from: the error names it.
        let outside = std::env::temp_dir().join("imece-probe-nowhere");
        let err = probe(&outside, known).unwrap_err();
        assert!(err.contains("imece-probe-nowhere"), "{err}");
        let _ = std::fs::remove_dir_all(&repo);
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
    fn what_an_agent_can_do_depends_on_its_vendor_and_role() {
        assert!(
            abilities(Vendor::Claude, Role::Leader)
                .contains("not change them; the run tool runs checks")
        );
        assert!(abilities(Vendor::Claude, Role::Worker).contains("the run tool runs the build"));
        assert!(abilities(Vendor::Claude, Role::Helper).contains("the run tool runs the build"));
        assert!(abilities(Vendor::Gemini, Role::Worker).contains("the run tool runs the build"));
        assert!(abilities(Vendor::Codex, Role::Leader).contains("refuses writes and the network"));
        assert!(
            abilities(Vendor::Codex, Role::Worker).contains("keeps writes inside this directory")
        );
        assert!(
            abilities(Vendor::Codex, Role::Helper).contains("keeps writes inside this directory")
        );
        let mut w = team(0);
        w.agents[0].vendor = Vendor::Codex;
        assert!(system_prompt(&w, 0, 4, 2).contains("run commands that only read"));
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
        let got = local_seconds_after_midnight(now());
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

    #[test]
    fn opening_a_checkout_adds_a_project_with_its_own_leader_and_place() {
        let repo = checkout("open-repo");
        let top = worktree::top(&repo).unwrap();
        let mut w = team(0);
        let mut live = Live::new(
            1,
            two_vendors(&std::env::temp_dir()),
            std::env::temp_dir(),
            PathBuf::from("/nonexistent"),
        );
        live.vendors[0].ladder.standard = Some("sonnet".into());
        assert_eq!(live.open_project(&mut w, &repo, None), Ok(1));
        assert_eq!(w.projects.len(), 2);
        let p = &w.projects[1];
        let name = project_name_of(&top);
        assert_eq!(
            (p.id, p.name.as_str(), p.leader, p.trunk, p.closed),
            (1, name.as_str(), 1, None, false)
        );
        let a = &w.agents[1];
        assert_eq!(
            (a.name.as_str(), a.project, a.role, a.vendor, a.status),
            (
                format!("lead-{name}").as_str(),
                1,
                Role::Leader,
                Vendor::Claude,
                Status::Idle
            )
        );
        assert_eq!(
            live.models[1].as_deref(),
            Some("sonnet"),
            "the lead vendor's standard model"
        );
        assert!(!live.tiered[1]);
        assert_eq!(live.places.len(), 2);
        assert_eq!(live.places[1].repo, top);
        assert_eq!(live.places[1].root, root_for(&live.state, &top));
        assert_eq!(live.repo_of(1), top);
        assert_eq!(
            (live.slots.len(), live.running.len(), live.mail.len()),
            (2, 2, 2)
        );
        assert!(matches!(
            &w.journal.last().unwrap().event,
            Event::Opened { leader: 1, path } if *path == top.display().to_string()
        ));
        // A name given, another checkout, another lead vendor.
        let other = checkout("open-other");
        live.lead = Vendor::Codex;
        assert_eq!(live.open_project(&mut w, &other, Some("api")), Ok(2));
        assert_eq!(w.projects[2].name, "api");
        assert_eq!(w.agents[2].name, "lead-api");
        assert_eq!(w.agents[2].vendor, Vendor::Codex);
        assert_eq!(live.places.len(), 3);
        assert_eq!(
            live.gemini_home(2, Role::Leader, "lead-api"),
            PathBuf::from(format!("{}-gemini/lead", live.places[2].root.display()))
        );
        assert_ne!(
            live.gemini_home(1, Role::Worker, "x"),
            live.gemini_home(2, Role::Worker, "x")
        );
        assert!(
            live.gemini_home(1, Role::Worker, "x").ends_with("x"),
            "a worker's is named after it"
        );
        assert!(live.gemini_home(1, Role::Helper, "x.1").ends_with("x.1"));
        let _ = std::fs::remove_dir_all(&repo);
        let _ = std::fs::remove_dir_all(&other);
    }

    #[test]
    fn a_project_is_refused_when_its_name_or_checkout_is_open_already_or_is_no_checkout() {
        let repo = checkout("refuse-repo");
        let other = checkout("refuse-other");
        let mut w = team(0);
        let mut live = idle_live(1);
        let counts = |w: &World, live: &Live| {
            (
                w.projects.len(),
                w.agents.len(),
                live.slots.len(),
                live.places.len(),
                live.running.len(),
                w.journal.len(),
            )
        };
        let before = counts(&w, &live);
        let nowhere = std::env::temp_dir().join("imece-refuse-nowhere");
        std::fs::create_dir_all(&nowhere).unwrap();
        let err = live.open_project(&mut w, &nowhere, None).unwrap_err();
        assert!(
            matches!(&err, ProjectRefusal::NotCheckout(why) if why.contains("imece-refuse-nowhere")),
            "{err:?}"
        );
        assert_eq!(
            live.open_project(&mut w, &repo, Some("a b")),
            Err(ProjectRefusal::BadName("a b".into()))
        );
        assert_eq!(counts(&w, &live), before, "a refusal adds nothing");
        assert_eq!(live.open_project(&mut w, &repo, Some("api")), Ok(1));
        let after = counts(&w, &live);
        assert_eq!(
            live.open_project(&mut w, &repo, Some("other")),
            Err(ProjectRefusal::PathOpen("api".into()))
        );
        let sub = repo.join("sub");
        std::fs::create_dir_all(&sub).unwrap();
        assert_eq!(
            live.open_project(&mut w, &sub, None),
            Err(ProjectRefusal::PathOpen("api".into())),
            "a directory inside is the same checkout"
        );
        assert_eq!(
            live.open_project(&mut w, &other, Some("api")),
            Err(ProjectRefusal::NameTaken("api".into()))
        );
        assert_eq!(
            live.open_project(&mut w, &other, Some("repo")),
            Err(ProjectRefusal::NameTaken("repo".into())),
            "the first project's name too"
        );
        assert_eq!(counts(&w, &live), after);
        for r in [
            ProjectRefusal::NotCheckout("/x: not a repo".into()),
            ProjectRefusal::NameTaken("api".into()),
            ProjectRefusal::PathOpen("api".into()),
            ProjectRefusal::BadName("a b".into()),
            ProjectRefusal::Bus("no socket".into()),
        ] {
            let said = r.describe();
            let arg = match &r {
                ProjectRefusal::NotCheckout(s)
                | ProjectRefusal::NameTaken(s)
                | ProjectRefusal::PathOpen(s)
                | ProjectRefusal::BadName(s)
                | ProjectRefusal::Bus(s) => s.clone(),
                ProjectRefusal::Last => unreachable!(),
            };
            assert!(said.contains(&arg), "{said}");
        }
        assert_eq!(
            ProjectRefusal::NameTaken("api".into()).describe(),
            "a project named api is open already"
        );
        assert_eq!(
            ProjectRefusal::PathOpen("api".into()).describe(),
            "that checkout is open already, as api"
        );
        assert_eq!(
            ProjectRefusal::Bus("x".into()).describe(),
            "the leader could not get a socket: x"
        );
        assert_eq!(
            ProjectRefusal::Last.describe(),
            "the last open project stays open"
        );
        let _ = std::fs::remove_dir_all(&repo);
        let _ = std::fs::remove_dir_all(&other);
        let _ = std::fs::remove_dir_all(&nowhere);
    }

    #[test]
    fn closing_a_project_ends_its_agents_and_reopening_it_keeps_its_id_with_a_new_leader() {
        use std::os::unix::process::{CommandExt, ExitStatusExt};
        let repo = checkout("close-repo");
        let mut w = team(0);
        let mut live = idle_live(1);
        live.open_bus(&w, "/opt/imece".into()).unwrap();
        assert_eq!(live.open_project(&mut w, &repo, Some("api")), Ok(1));
        live.hire(&mut w, 1, &json!({})).unwrap();
        assert_eq!(
            (w.agents[2].project, w.agents[2].name.as_str()),
            (1, "claude-1")
        );
        let lead_ended = Arc::new(AtomicBool::new(false));
        live.ready(
            &mut w,
            1,
            Ok((Box::new(Ends(lead_ended.clone())), repo.clone(), None)),
        );
        let ended = Arc::new(AtomicBool::new(false));
        live.ready(
            &mut w,
            2,
            Ok((Box::new(Ends(ended.clone())), PathBuf::from("/w"), None)),
        );
        live.mail[2].push(Mail {
            from: 1,
            kind: MsgKind::Assign,
            body: "x".into(),
        });
        live.notes[2].push("x".into());
        live.asked[2] = true;
        live.leaving[2] = true;
        live.changes.insert(
            2,
            worktree::Change {
                path: PathBuf::from("/w"),
                tree: String::new(),
                files: Vec::new(),
                patch: String::new(),
            },
        );
        w.replied(2, "done");
        live.post(&mut w, 2, &json!({ "title": "t", "body": "b" }))
            .unwrap();
        live.offered.insert(0, vec![2]);
        live.post(&mut w, 0, &json!({ "title": "mine", "body": "b" }))
            .unwrap();
        live.offered.insert(1, Vec::new());
        let mut sleeper = std::process::Command::new("sleep")
            .arg("30")
            .process_group(0)
            .spawn()
            .unwrap();
        live.running[2].add(sleeper.id() as i32);
        let generation = live.generation[2];
        assert_eq!(live.close_project(&mut w, 1), Ok(()));
        assert!(lead_ended.load(Ordering::SeqCst) && ended.load(Ordering::SeqCst));
        assert_eq!(live.generation[2], generation + 1);
        assert!(matches!(live.slots[2], Slot::None));
        assert!(live.mail[2].is_empty() && live.notes[2].is_empty());
        assert!(!live.asked[2] && !live.leaving[2]);
        assert!(!live.changes.contains_key(&2));
        assert_eq!(
            live.posts.keys().collect::<Vec<_>>(),
            [&1],
            "the other project's card stays"
        );
        assert_eq!(live.offered.keys().collect::<Vec<_>>(), [&1]);
        let status = sleeper.wait().unwrap();
        assert_eq!(status.signal(), Some(libc::SIGKILL), "{status:?}");
        assert!(w.agents[1].gone && w.agents[2].gone);
        assert!(
            !w.agents[0].gone && live.generation[0] == 0,
            "the other project"
        );
        assert!(w.projects[1].closed);
        assert!(w.open_inbox().is_empty());
        // Opened again in place, with a new leader on the next socket.
        assert_eq!(live.open_project(&mut w, &repo, None), Ok(1));
        assert_eq!((w.projects.len(), live.places.len()), (2, 2));
        assert_eq!(w.projects[1].leader, 3);
        assert!(!w.projects[1].closed);
        assert!(w.agents[1].gone, "the old leader stays gone");
        let bus = live.bus.as_ref().unwrap();
        assert_eq!(bus.servers.len(), 4);
        assert!(bus.servers[3].path().ends_with("a3.sock"));
        // A closed project is reused only for its own checkout: another
        // checkout, with one closed, is a new project.
        let other = checkout("close-other");
        w.open_project(None, "third".into(), "/t".into(), Vendor::Claude);
        live.bind(4, Role::Leader).unwrap();
        live.grow(None, false);
        live.places.push(Place {
            repo: PathBuf::from("/t"),
            root: PathBuf::from("/t-trees"),
        });
        assert_eq!(live.close_project(&mut w, 1), Ok(()));
        assert_eq!(live.open_project(&mut w, &other, Some("other")), Ok(3));
        assert!(w.projects[1].closed, "not reused for another checkout");
        assert_eq!(live.places.len(), 4);
        let _ = std::fs::remove_dir_all(&other);
        let _ = std::fs::remove_dir_all(&repo);
    }

    #[test]
    fn the_last_open_project_can_not_be_closed() {
        let repo = checkout("last-repo");
        let mut w = team(0);
        let mut live = idle_live(1);
        assert_eq!(live.close_project(&mut w, 0), Err(ProjectRefusal::Last));
        assert!(!w.projects[0].closed && !w.agents[0].gone);
        live.open_project(&mut w, &repo, Some("api")).unwrap();
        assert_eq!(
            live.close_project(&mut w, 0),
            Ok(()),
            "the first project can go too"
        );
        assert_eq!(live.close_project(&mut w, 1), Err(ProjectRefusal::Last));
        assert!(!w.projects[1].closed && !w.agents[1].gone);
        let _ = std::fs::remove_dir_all(&repo);
    }

    #[test]
    fn a_gone_worker_is_not_offered_a_card() {
        let mut w = team(1);
        let mut live = idle_live(2);
        live.post(
            &mut w,
            0,
            &json!({ "title": "Add a test", "body": "Cover it" }),
        )
        .unwrap();
        w.agents[1].gone = true;
        live.step(&mut w);
        assert!(queued(&live, 1).is_empty());
        assert_eq!(live.offered.get(&0), None);
        assert_eq!(live.woken[1], 0);
        // Back among the living (a test's licence): offered.
        w.agents[1].gone = false;
        live.step(&mut w);
        assert_eq!(live.offered[&0], [1]);
    }

    #[test]
    fn each_project_s_leader_runs_in_its_own_checkout_with_its_own_notes() {
        let one = checkout("notes-one");
        let two = checkout("notes-two");
        std::fs::write(one.join("CLAUDE.md"), "Notes one.").unwrap();
        std::fs::write(two.join("CLAUDE.md"), "Notes two.").unwrap();
        let mut w = team(0);
        let mut live = Live::new(
            1,
            claude_at(&fake_claude(&one)),
            one.clone(),
            one.join("trees"),
        );
        live.open_project(&mut w, &two, Some("two")).unwrap();
        live.told(&mut w, 1, "hello").unwrap();
        wait_for(&mut live, &mut w, |w| w.agents[1].status == Status::Stopped);
        let raw = std::fs::read_to_string(two.join("args.txt")).unwrap();
        assert!(raw.contains("Notes two."), "{raw}");
        assert!(!raw.contains("Notes one."), "{raw}");
        assert!(raw.contains("--name\nimece lead-two\n"), "{raw}");
        assert!(!one.join("args.txt").exists(), "ran in its own checkout");
        let _ = std::fs::remove_dir_all(&one);
        let _ = std::fs::remove_dir_all(&two);
    }

    #[test]
    fn a_worker_s_change_goes_to_its_own_project_s_checkout() {
        let one = checkout("own-one");
        let two = checkout("own-two");
        let root = trees("own");
        let mut w = team(0);
        let mut live = Live::new(
            1,
            claude_at("/nonexistent/claude"),
            one.clone(),
            root.clone(),
        );
        // Worktrees of projects opened from the screen go under the state
        // directory imece was given, not the person's real one.
        std::fs::create_dir_all(&root).unwrap();
        live.state = root.canonicalize().unwrap();
        live.open_project(&mut w, &two, Some("two")).unwrap();
        live.hire(&mut w, 1, &json!({})).unwrap();
        let (repo, root) = (live.places[1].repo.clone(), live.places[1].root.clone());
        assert!(root.starts_with(&live.state), "{}", root.display());
        let tree = worker_in(&mut live, &mut w, 2, &repo, &root);
        assert!(tree.starts_with(&root), "{}", tree.display());
        std::fs::write(tree.join("a.txt"), "changed\n").unwrap();
        live.send(&mut w, 2, &msg("lead-two", "done", "Changed a"))
            .unwrap();
        live.apply_change(&mut w, 2).unwrap();
        assert_eq!(git_in(&two, &["diff", "--cached", "--name-only"]), "a.txt");
        assert_eq!(git_in(&one, &["status", "--porcelain"]), "");
        let _ = std::fs::remove_dir_all(&one);
        let _ = std::fs::remove_dir_all(&two);
    }

    #[test]
    fn a_worker_s_worktree_follows_its_own_project_s_checkout() {
        let one = checkout("ownfollow-one");
        let two = checkout("ownfollow-two");
        let root = trees("ownfollow");
        let mut w = team(0);
        let mut live = Live::new(1, claude_at("/nonexistent/claude"), one.clone(), root);
        live.open_project(&mut w, &two, Some("two")).unwrap();
        live.hire(&mut w, 1, &json!({})).unwrap();
        let (repo, root) = (live.places[1].repo.clone(), live.places[1].root.clone());
        worker_in(&mut live, &mut w, 2, &repo, &root);
        let commit = |dir: &Path| {
            std::fs::write(dir.join("b.txt"), "b").unwrap();
            git_in(dir, &["add", "."]);
            git_in(
                dir,
                &[
                    "-c",
                    "user.name=t",
                    "-c",
                    "user.email=t@t",
                    "-c",
                    "commit.gpgsign=false",
                    "commit",
                    "-q",
                    "-m",
                    "second",
                ],
            );
        };
        let moved = |w: &World| {
            w.journal
                .iter()
                .filter(|e| matches!(e.event, Event::WorktreeMoved { agent: 2, .. }))
                .count()
        };
        commit(&one);
        live.follow(&mut w, 2);
        assert_eq!(
            moved(&w),
            0,
            "the other project's checkout moved, not its own"
        );
        commit(&two);
        live.follow(&mut w, 2);
        assert_eq!(moved(&w), 1);
        let _ = std::fs::remove_dir_all(&one);
        let _ = std::fs::remove_dir_all(&two);
    }

    #[test]
    fn a_leader_s_prompt_names_only_its_own_project_s_workers() {
        let mut w = team(1);
        w.open_project(None, "api".into(), "/a".into(), Vendor::Claude);
        let p1 = system_prompt(&w, 2, 4, 2);
        assert!(p1.contains("You have no workers yet."), "{p1}");
        assert!(!p1.contains("claude-1"), "{p1}");
        w.agents.push(Agent::new(
            3,
            1,
            "claude-2".into(),
            Vendor::Claude,
            Role::Worker,
            Status::Idle,
            0,
        ));
        let p0 = system_prompt(&w, 0, 4, 2);
        assert!(p0.contains("The workers so far are claude-1."), "{p0}");
        let p1 = system_prompt(&w, 2, 4, 2);
        assert!(p1.contains("The workers so far are claude-2."), "{p1}");
    }

    #[test]
    fn an_opened_project_s_leader_gets_a_socket_in_agent_order_with_the_leader_s_tools() {
        use std::io::{BufRead, BufReader, Write};
        use std::os::unix::net::UnixStream;
        let repo = checkout("socket-repo");
        let mut w = team(0);
        let mut live = idle_live(1);
        live.open_bus(&w, "/opt/imece".into()).unwrap();
        live.open_project(&mut w, &repo, Some("api")).unwrap();
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
        assert_eq!(names, ["send", "post", "board", "run", "hire"]);
        let _ = std::fs::remove_dir_all(&repo);
    }

    #[test]
    fn a_project_s_name_and_worktrees_root_come_from_its_checkout() {
        assert_eq!(dir_name(Path::new("/x/imece")), "imece");
        assert_eq!(dir_name(Path::new("/")), "repo");
        assert_eq!(project_name_of(Path::new("/x/imece")), "imece");
        assert_eq!(
            project_name_of(Path::new("/x/my project (v2)")),
            "my-project--v2-"
        );
        assert_eq!(project_name_of(Path::new("/x/şirket")), "-irket");
        assert_eq!(
            project_name_of(Path::new("/x/abcdefghijklmnopqrstuvwxyz")),
            "abcdefghijklmnopqrstuvwx",
            "cut to 24"
        );
        assert_eq!(
            root_for(Path::new("/state"), Path::new("/x/imece")),
            PathBuf::from("/state/worktrees/imece-549e0114")
        );
        assert!(
            root_for(Path::new("/state"), Path::new("/"))
                .to_string_lossy()
                .starts_with("/state/worktrees/repo-")
        );
        assert_ne!(
            root_for(Path::new("/state"), Path::new("/x/imece")),
            root_for(Path::new("/state"), Path::new("/y/imece")),
            "one name, two checkouts"
        );
        for ok in ["a", "a.b_C-9", &"x".repeat(24)] {
            assert!(valid_project_name(ok), "{ok}");
        }
        for bad in ["", &"x".repeat(25), "a b", "a/b", "é", "a\tb"] {
            assert!(!valid_project_name(bad), "{bad:?}");
        }
    }

    #[test]
    fn renaming_a_project_renames_its_leader_and_tells_its_agents() {
        let mut w = team(1);
        let mut live = idle_live(2);
        w.open_project(None, "api".into(), "/a".into(), Vendor::Claude);
        live.grow(None, false);
        assert_eq!(
            live.rename_project(&mut w, 0, "a b"),
            Err(ProjectRefusal::BadName("a b".into()))
        );
        assert_eq!(
            live.rename_project(&mut w, 0, "api"),
            Err(ProjectRefusal::NameTaken("api".into()))
        );
        assert_eq!(w.projects[0].name, "repo", "refused: unchanged");
        assert!(live.notes.iter().all(Vec::is_empty));
        assert_eq!(
            live.rename_project(&mut w, 1, "api"),
            Ok(()),
            "its own name is no clash"
        );
        assert_eq!(live.notes[2].len(), 1);
        assert_eq!(live.rename_project(&mut w, 0, "shop"), Ok(()));
        assert_eq!(w.projects[0].name, "shop");
        assert_eq!(w.agents[0].name, "lead-shop");
        assert_eq!(
            (
                live.notes[0].len(),
                live.notes[1].len(),
                live.notes[2].len()
            ),
            (1, 1, 1),
            "its own agents, not the other project's"
        );
        assert_eq!(
            live.notes[1][0],
            "The person renamed the project: it is called shop now, and its leader, lead \
             until now, is lead-shop. Address it by that name."
        );
        assert!(matches!(
            &w.journal.last().unwrap().event,
            Event::Renamed { leader: 0, from, to } if from == "repo" && to == "shop"
        ));
        w.agents[1].gone = true;
        live.rename_project(&mut w, 0, "store").unwrap();
        assert_eq!(live.notes[1].len(), 1, "a gone agent hears nothing");
        assert_eq!(live.notes[0].len(), 2);
    }

    fn item(kind: InboxKind, agent: AgentId, detail: &str) -> InboxItem {
        InboxItem {
            kind,
            agent,
            detail: detail.into(),
            researcher: None,
            open: true,
            tick: 1,
        }
    }

    #[test]
    fn what_the_store_remembers_comes_back_asleep_with_its_mail_offers_and_cut_turns() {
        let repo = checkout("adopt");
        let state = trees("adopt-state");
        let mut w = team(2);
        w.agents[1].cost = 1_500;
        w.agents[2].gone = true;
        w.agents[2].status = Status::Idle;
        let c = with_card(&mut w, 1);
        // A second project whose checkout is gone since, with a card on
        // its board and a change of its own.
        w.open_project(
            None,
            "gone".into(),
            "/nonexistent/gone".into(),
            Vendor::Claude,
        );
        let lead_gone = 3;
        let other = w.cards.len();
        w.cards.push(Card {
            id: other,
            project: 1,
            title: "elsewhere".into(),
            owner: None,
            column: Column::Waiting,
            story: Vec::new(),
            symbols: Vec::new(),
            started: 0,
            finished: 0,
        });
        w.inbox.push(item(InboxKind::Reply, 1, "an old answer"));
        w.inbox.push(item(InboxKind::Change, 1, "an older change"));
        w.inbox.push(item(InboxKind::Change, 1, "Changed a"));
        w.inbox
            .push(item(InboxKind::Change, 1, "a later one, closed"));
        w.inbox[0].open = false;
        w.inbox[3].open = false;
        let change = |tree: &str| worktree::Change {
            path: "/trees/claude-1".into(),
            tree: tree.into(),
            files: vec![worktree::FileChange {
                path: "a.txt".into(),
                added: 2,
                removed: 1,
                binary: false,
            }],
            patch: "diff".into(),
        };
        let found = Found {
            repos: vec![repo.clone(), "/nonexistent/gone".into()],
            models: vec![None, Some("haiku".into()), None, None],
            tiered: vec![false, true, false, false],
            woken: vec![0, 2, 0, 0],
            posted: vec![1, 0, 0, 0],
            mail: vec![
                Vec::new(),
                vec![Mail {
                    from: 0,
                    kind: MsgKind::Assign,
                    body: "old mail".into(),
                }],
                Vec::new(),
                Vec::new(),
            ],
            notes: vec![vec!["a note".into()], Vec::new(), Vec::new(), Vec::new()],
            sessions: vec![
                Some(Resumable {
                    session: "686c6d27-d50b-4ae2-ad6a-159bf2984671".into(),
                    spent: 524,
                }),
                None,
                None,
                None,
            ],
            inflight: vec![
                Vec::new(),
                vec!["go on".into()],
                vec!["gone's".into()],
                Vec::new(),
            ],
            posts: HashMap::from([
                (
                    c,
                    Post {
                        by: 0,
                        body: "the body".into(),
                    },
                ),
                (
                    other,
                    Post {
                        by: lead_gone,
                        body: "elsewhere".into(),
                    },
                ),
            ]),
            offered: HashMap::from([(c, vec![2]), (other, vec![lead_gone])]),
            changes: HashMap::from([(1, change("abc")), (lead_gone, change("def"))]),
        };
        let mut live = Live::bare(claude_at("x"), Vendor::Claude);
        live.state = state.clone();
        let journal = w.journal.len();
        live.adopt(&mut w, found, 3);
        assert_eq!(live.places.len(), 2);
        assert_eq!(live.places[0].repo, repo);
        assert_eq!(live.places[0].root, root_for(&state, &repo));
        assert_eq!(
            w.agents.iter().map(|a| a.status).collect::<Vec<_>>(),
            [Status::Asleep, Status::Asleep, Status::Idle, Status::Idle],
            "asleep; gone stays idle; the gone project's leader left"
        );
        assert_eq!(live.remembered, [true, true, false, true]);
        assert_eq!(live.spent_before, [0, 1_500, 0, 0]);
        assert_eq!(live.models[1].as_deref(), Some("haiku"));
        assert_eq!(live.tiered, [false, true, false, false]);
        assert_eq!(live.woken, [0, 2, 0, 0]);
        assert_eq!(live.posted, [1, 0, 0, 0]);
        assert_eq!(live.mail[1].len(), 1);
        assert_eq!(live.notes[0], ["a note"]);
        assert!(live.inflight.iter().all(Vec::is_empty), "not run again");
        assert_eq!(live.interrupted[1], ["go on"]);
        assert!(
            live.interrupted[2].is_empty(),
            "a gone agent's turn is nobody's"
        );
        assert!(matches!(
            w.cards[c].story.last().map(|s| &s.event),
            Some(StoryEvent::Interrupted)
        ));
        assert!(
            w.journal[journal..]
                .iter()
                .any(|e| e.event == Event::Interrupted { agent: 1 })
        );
        assert!(
            w.agents[1]
                .trace
                .iter()
                .any(|l| l.contains("turn cut short")),
            "{:?}",
            w.agents[1].trace
        );
        assert_eq!(live.loaded_cards, 2);
        // The project whose checkout is gone closed, with what was its.
        assert!(!w.projects[0].closed);
        assert!(w.projects[1].closed);
        assert!(
            w.journal
                .iter()
                .any(|e| e.event == Event::Closed { leader: lead_gone })
        );
        assert_eq!(live.posts.keys().copied().collect::<Vec<_>>(), [c]);
        assert_eq!(live.offered.keys().copied().collect::<Vec<_>>(), [c]);
        assert_eq!(live.changes.keys().copied().collect::<Vec<_>>(), [1]);
        assert_eq!(live.changes[&1].tree, "abc");
        // The screen's copy of the change, titled by its inbox item.
        assert_eq!(w.changes.len(), 1);
        assert_eq!(
            (
                w.changes[0].agent,
                w.changes[0].title.as_str(),
                w.changes[0].files[0].added
            ),
            (1, "Changed a", 2)
        );
        assert!(
            w.agents[0].trace.iter().any(|l| l.contains("3 lines")),
            "{:?}",
            w.agents[0].trace
        );
        let _ = std::fs::remove_dir_all(&repo);
    }

    #[test]
    fn a_remembered_agent_hears_where_it_left_off_once_and_keeps_its_card() {
        let mut w = team(2);
        let c = with_card(&mut w, 1);
        for e in [
            StoryEvent::Sent {
                from: 0,
                to: 1,
                kind: MsgKind::Assign,
                text: "do it\nproperly".into(),
            },
            StoryEvent::YouSaid("hi there".into()),
            StoryEvent::Touched("src/a.rs".into()),
            StoryEvent::Replied("ok done".into()),
            StoryEvent::Asked {
                from: 1,
                to: 0,
                text: "which".into(),
            },
            StoryEvent::Answered {
                from: 0,
                text: "that".into(),
            },
        ] {
            w.story(Some(c), e);
        }
        let mut live = idle_live(3);
        live.post(
            &mut w,
            0,
            &json!({"title": "write docs", "body": "the README"}),
        )
        .unwrap();
        live.remembered[1] = true;
        live.interrupted[1] = vec!["finish TOKEN=abc the tests".into()];
        for body in ["one", "two"] {
            live.mail[1].push(Mail {
                from: 0,
                kind: MsgKind::Inform,
                body: body.into(),
            });
        }
        w.agents[1].status = Status::Asleep;
        let cards = w.cards.len();
        live.told(&mut w, 1, "go on").unwrap();
        let q = queued(&live, 1);
        let text = &q[0];
        assert!(
            text.starts_with("From imece (not from the person or a teammate):"),
            "{text}"
        );
        for part in [
            "Where you left off",
            "Your task: #1 fix it (not finished)",
            "lead → claude-1 (assign): do it",
            "the person → you: hi there",
            "changed src/a.rs",
            "you answered: ok done",
            "claude-1 asked lead: which",
            "lead answered: that",
            "cut short when imece closed",
            "Its prompt was: finish [***] the tests",
            "2 messages wait for you",
            "The board has 1 open cards:",
            "#2 write docs (posted by lead)",
        ] {
            assert!(text.contains(part), "{part:?} in {text}");
        }
        assert!(!text.contains("TOKEN=abc"));
        assert!(!text.contains("properly"), "first lines only");
        assert!(text.ends_with("\n\ngo on"), "{text}");
        assert!(!live.remembered[1]);
        assert!(live.interrupted[1].is_empty());
        assert_eq!(w.cards.len(), cards, "the unfinished card goes on");
        assert_eq!(w.agents[1].card, Some(c));
        assert_eq!(w.agents[1].status, Status::Working);
        // Told again: no second briefing. The mail still waits (it comes
        // as a turn of its own once this one ends).
        live.told(&mut w, 1, "more").unwrap();
        assert_eq!(queued(&live, 1)[1], "more");
        assert_eq!(live.mail[1].len(), 2);
        // No card, no board, no mail: said so, briefly.
        live.remembered[2] = true;
        w.agents[2].status = Status::Asleep;
        w.agents[2].card = None;
        live.told(&mut w, 2, "x").unwrap();
        let text = &queued(&live, 2)[0];
        assert!(text.contains("You had no task in hand."), "{text}");
        assert!(!text.contains("messages wait") && text.contains("The board has 1 open"));
        // A finished card is said to be, and a new task gets a new card.
        let mut w = team(1);
        let c = with_card(&mut w, 1);
        w.cards[c].column = Column::Done;
        let mut live = idle_live(2);
        live.remembered[1] = true;
        w.agents[1].status = Status::Asleep;
        live.told(&mut w, 1, "next").unwrap();
        assert!(queued(&live, 1)[0].contains("#1 fix it (done)"));
        assert_eq!(w.agents[1].card, Some(1));
    }

    #[test]
    fn a_remembered_worker_is_told_what_its_worktree_holds() {
        let repo = checkout("brief-tree");
        let root = trees("brief-tree");
        let mut w = team(1);
        let mut live = Live::new(
            2,
            claude_at("/nonexistent/claude"),
            repo.clone(),
            root.clone(),
        );
        let tree = worktree::ensure(&repo, &root, "claude-1").unwrap().path;
        std::fs::write(tree.join("a.txt"), "changed\n").unwrap();
        std::fs::write(tree.join("b.txt"), "new\n").unwrap();
        live.remembered[1] = true;
        w.agents[1].status = Status::Asleep;
        live.told(&mut w, 1, "go on").unwrap();
        let text = &queued(&live, 1)[0];
        assert!(
            text.contains("Your worktree holds changes the person has not applied: 2 files, +2 −1: a.txt, b.txt"),
            "{text}"
        );
        // The leader has no worktree: nothing said of one.
        live.remembered[0] = true;
        w.agents[0].status = Status::Asleep;
        live.told(&mut w, 0, "go on").unwrap();
        assert!(!queued(&live, 0)[0].contains("worktree"));
        let _ = std::fs::remove_dir_all(&repo);
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn a_briefing_is_capped_line_by_line_and_as_a_whole() {
        let mut w = team(1);
        let mut live = idle_live(2);
        live.remembered[1] = true;
        live.interrupted[1] = (0..40)
            .map(|i| format!("{i} {}", "x".repeat(500)))
            .collect();
        w.agents[1].status = Status::Asleep;
        live.told(&mut w, 1, "go on").unwrap();
        let text = &queued(&live, 1)[0];
        let longest = text.lines().map(|l| l.chars().count()).max().unwrap();
        assert!(longest <= MAX_BRIEFING_LINE + 2, "{longest}");
        assert!(
            text.chars().count() <= MAX_BRIEFING + 80,
            "{}",
            text.chars().count()
        );
        assert!(text.contains("- A turn of yours was cut short"));
        assert_eq!(cut("héllo", 2), "hé");
        assert_eq!(cut("ab", 5), "ab");
        assert_eq!(cut("", 0), "");
    }

    #[test]
    fn a_worker_told_again_goes_on_with_an_unfinished_card_and_gets_a_new_one_after_a_done_one() {
        let mut w = team(1);
        let mut live = idle_live(2);
        let c = with_card(&mut w, 1);
        w.agents[1].status = Status::Stopped;
        live.told(&mut w, 1, "try again").unwrap();
        assert_eq!(w.agents[1].card, Some(c));
        assert_eq!(w.cards.len(), 1, "the stopped task goes on");
        w.cards[c].column = Column::Done;
        w.agents[1].status = Status::Idle;
        live.turns[1] = 0;
        live.told(&mut w, 1, "next").unwrap();
        assert_eq!(w.agents[1].card, Some(1));
        assert_eq!(w.cards[1].title, "next");
    }

    #[test]
    fn a_sleeper_wakes_to_new_mail_and_new_cards_but_not_to_what_waited() {
        let mut w = team(2);
        let mut live = idle_live(3);
        for id in [1, 2] {
            w.agents[id].status = Status::Asleep;
        }
        live.mail[1].push(Mail {
            from: 0,
            kind: MsgKind::Assign,
            body: "old".into(),
        });
        live.step(&mut w);
        assert_eq!(live.turns[1], 0, "mail that waited through does not wake");
        assert_eq!(w.agents[1].status, Status::Asleep);
        // Mail that does not wake an idle agent does not wake a sleeper.
        live.send(&mut w, 0, &msg("claude-2", "inform", "fyi"))
            .unwrap();
        assert_eq!(w.agents[2].status, Status::Asleep);
        live.step(&mut w);
        assert_eq!(live.turns[2], 0);
        // New mail wakes, and the old rides along.
        let answer = live
            .send(&mut w, 0, &msg("claude-1", "assign", "new"))
            .unwrap();
        assert!(answer.contains("starts on it now"), "{answer}");
        assert_eq!(w.agents[1].status, Status::Idle);
        assert!(
            w.agents[1]
                .trace
                .iter()
                .any(|l| l.contains("woken by mail"))
        );
        live.step(&mut w);
        assert_eq!(live.turns[1], 1);
        let text = &queued(&live, 1)[0];
        assert!(text.contains("old") && text.contains("new"), "{text}");
        // A card that waited through is not offered; one posted since is.
        live.post(&mut w, 0, &json!({"title": "old card", "body": "b"}))
            .unwrap();
        live.loaded_cards = w.cards.len();
        live.step(&mut w);
        assert_eq!(live.turns[2], 0);
        assert!(live.offered.is_empty());
        live.post(&mut w, 0, &json!({"title": "new card", "body": "b"}))
            .unwrap();
        live.step(&mut w);
        assert_eq!(live.turns[2], 1);
        assert_eq!(
            live.offered
                .iter()
                .map(|(c, a)| (*c, a.clone()))
                .collect::<Vec<_>>(),
            [(2, vec![2])],
            "the new card alone (card 1 is claude-1's task)"
        );
        let text = &queued(&live, 2)[0];
        assert!(
            text.contains("new card") && !text.contains("old card"),
            "{text}"
        );
    }

    #[test]
    fn persist_writes_what_moved_when_there_is_a_store() {
        let mut w = team(1);
        let mut live = idle_live(2);
        assert_eq!(live.persist(&w), Ok(false), "nothing to write to");
        live.store_in_memory();
        assert_eq!(live.persist(&w), Ok(true));
        assert_eq!(live.persist(&w), Ok(false));
        w.agents[1].cost = 5;
        assert_eq!(live.persist(&w), Ok(true));
        live.break_store();
        w.agents[1].cost = 6;
        let e = live.persist(&w).unwrap_err();
        assert!(e.contains("no such table"), "{e}");
    }

    #[test]
    fn the_world_is_remembered_between_runs_and_the_checkout_at_hand_leads() {
        let repo = checkout("remember");
        let other = checkout("remember-other");
        let state = trees("remember-state");
        let options = |repo: &Path, budget| Options {
            repo: repo.to_path_buf(),
            state: state.clone(),
            max_workers: 2,
            max_helpers: 1,
            lead: Vendor::Claude,
            tiers: Vec::new(),
            budget,
            fresh: false,
            run_programs: Vec::new(),
            run_timeout: DEFAULT_RUN_TIMEOUT,
        };
        let present = [(Vendor::Claude, true)];
        let (mut w, mut live) = world(options(&repo, None), &present).unwrap();
        assert_eq!(w.projects.len(), 1);
        assert_eq!(w.agents[0].status, Status::Idle, "new, not remembered");
        assert_eq!(w.budget, 500_000, "$50 the first time");
        assert!(w.started_at.abs_diff(now()) <= 2);
        assert_eq!((w.tick, live.home()), (0, 0));
        assert!(live.store.is_some());
        // As if the world had begun 100 seconds earlier.
        w.started_at -= 100;
        let began = w.started_at;
        w.budget = 70_000;
        w.agents[0].cost = 1_234;
        live.inflight[0].push("hello".into());
        live.sessions[0] = Some(Resumable {
            session: "686c6d27-d50b-4ae2-ad6a-159bf2984671".into(),
            spent: 524,
        });
        assert_eq!(live.persist(&w), Ok(true));
        drop(live);

        // The next run: asleep, the pot as the flag says, the turn cut.
        let (w, mut live) = world(options(&repo, Some(20_000)), &present).unwrap();
        assert_eq!(w.projects.len(), 1);
        assert_eq!(w.agents[0].status, Status::Asleep);
        assert_eq!((w.agents[0].cost, live.spent_before[0]), (1_234, 1_234));
        assert_eq!(
            live.sessions[0],
            Some(Resumable {
                session: "686c6d27-d50b-4ae2-ad6a-159bf2984671".into(),
                spent: 524,
            })
        );
        assert_eq!(w.budget, 20_000);
        assert_eq!((w.started_at, w.run_spent), (began, 1_234));
        assert!(live.remembered[0]);
        assert_eq!(live.interrupted[0], ["hello"]);
        assert!(
            w.journal
                .iter()
                .any(|e| e.event == Event::Interrupted { agent: 0 })
        );
        assert_eq!(live.home(), 0);
        assert!(
            (19..=21).contains(&w.tick),
            "100 seconds in whole ticks, not {}",
            w.tick
        );
        assert_eq!(w.run_tick, w.tick);
        assert!(
            !w.agents[0].trace.iter().any(|l| l.contains("skipped")),
            "nothing skipped, nothing said: {:?}",
            w.agents[0].trace
        );
        assert_eq!(live.persist(&w), Ok(true));
        drop(live);

        // Without the flag, the pot is as it was left; from another
        // checkout, that one is a project of its own and leads.
        let (w, live) = world(options(&other, None), &present).unwrap();
        assert_eq!(w.budget, 20_000);
        assert_eq!(w.projects.len(), 2);
        assert_eq!(live.home(), 1);
        assert_eq!(
            w.agents[1].name,
            format!("lead-{}", project_name_of(&other))
        );
        assert_eq!(w.agents[0].status, Status::Asleep);
        assert_eq!(w.agents[1].status, Status::Idle);
        drop(live);
        // While one runs, a second is refused.
        let (_w, _live) = world(options(&repo, None), &present).unwrap();
        let e = world(options(&repo, None), &present)
            .err()
            .expect("refused");
        assert!(e.contains("another imece is running"), "{e}");
        let _ = std::fs::remove_dir_all(&repo);
        let _ = std::fs::remove_dir_all(&other);
        let _ = std::fs::remove_dir_all(&state);
    }

    const UUID: &str = "686c6d27-d50b-4ae2-ad6a-159bf2984671";

    /// A worker remembered with session [`UUID`], a turn cut short and one
    /// message waiting, in a team whose Claude transcripts live under a
    /// temp dir; `there` puts the session's transcript where Claude would
    /// have it for the worker's worktree.
    fn remembered_worker(name: &str, there: bool) -> (World, Live, PathBuf, PathBuf) {
        let repo = checkout(name);
        let root = trees(name);
        let mut w = team(1);
        let mut live = Live::new(
            2,
            claude_at("/nonexistent/claude"),
            repo.clone(),
            root.clone(),
        );
        live.claude_home = trees(&format!("{name}-home"));
        // Made first, so its path is the canonical one the session has.
        worktree::ensure(&repo, &root, "claude-1").unwrap();
        live.sessions[1] = Some(Resumable {
            session: UUID.into(),
            spent: 524,
        });
        live.remembered[1] = true;
        live.interrupted[1] = vec!["finish it".into()];
        live.mail[1].push(Mail {
            from: 0,
            kind: MsgKind::Inform,
            body: "one".into(),
        });
        w.agents[1].status = Status::Asleep;
        if there {
            let file = claude::transcript(&live.claude_home, &live.cwd_of(&w, 1), UUID);
            std::fs::create_dir_all(file.parent().unwrap()).unwrap();
            std::fs::write(&file, "{}\n").unwrap();
        }
        (w, live, repo, root)
    }

    fn journaled(w: &World, e: &Event) -> usize {
        w.journal.iter().filter(|j| j.event == *e).count()
    }

    #[test]
    fn a_remembered_claude_agent_goes_on_in_its_session_when_its_transcript_is_there() {
        let (mut w, mut live, repo, root) = remembered_worker("resume", true);
        let home = live.claude_home.clone();
        with_card(&mut w, 1);
        live.told(&mut w, 1, "go on").unwrap();
        // The short briefing: what happened since, not the story it knows.
        let text = &queued(&live, 1)[0];
        assert!(
            text.starts_with("From imece (not from the person or a teammate):"),
            "{text}"
        );
        for part in [
            "this session goes on where it was",
            "cut short when imece closed",
            "Its prompt was: finish it",
            "1 messages wait for you",
        ] {
            assert!(text.contains(part), "{part:?} in {text}");
        }
        for not in [
            "Where you left off",
            "Your task",
            "fix it",
            "Nothing; go on",
        ] {
            assert!(!text.contains(not), "{not:?} in {text}");
        }
        assert!(text.ends_with("\n\ngo on"), "{text}");
        assert_eq!(
            live.resume_of(1),
            Some(Resumable {
                session: UUID.into(),
                spent: 524,
            })
        );
        assert_eq!(live.resume_of(0), None);
        assert_eq!(live.floor[1], 524);
        let a = live.resuming[1].as_ref().unwrap();
        assert_eq!((a.session.as_str(), a.text.as_str()), (UUID, "go on"));
        assert!(a.notes.is_empty());
        assert_eq!(
            live.interrupted[1],
            ["finish it"],
            "kept until the session confirms"
        );
        assert_eq!(journaled(&w, &Event::Restarted { agent: 1 }), 0);
        assert_eq!(journaled(&w, &Event::Resumed { agent: 1 }), 0);
        // The session confirms it is the old one.
        live.tx
            .send((
                1,
                live.generation[1],
                AgentEvent::Started {
                    session: UUID.into(),
                    model: "m".into(),
                },
            ))
            .unwrap();
        live.step(&mut w);
        assert_eq!(journaled(&w, &Event::Resumed { agent: 1 }), 1);
        assert!(live.resuming[1].is_none());
        assert!(live.interrupted[1].is_empty());
        assert_eq!(live.resume_of(1), None);
        assert_eq!(
            live.sessions[1],
            Some(Resumable {
                session: UUID.into(),
                spent: 524,
            })
        );
        // A turn's cost is what the session reports above its floor; the
        // session is remembered at its whole cost.
        live.tx
            .send((
                1,
                live.generation[1],
                AgentEvent::TurnDone {
                    ok: true,
                    text: "done".into(),
                    cost: 136,
                    denied: 0,
                },
            ))
            .unwrap();
        live.step(&mut w);
        assert_eq!(live.sessions[1].as_ref().unwrap().spent, 660);
        assert_eq!(w.agents[1].cost, 1, "136 millionths in ten-thousandths");
        // The vendor's /clear brings a new session id: that is the one to
        // go on with, from nothing; no second resume is journaled.
        live.tx
            .send((
                1,
                live.generation[1],
                AgentEvent::Started {
                    session: "s2".into(),
                    model: "m".into(),
                },
            ))
            .unwrap();
        live.step(&mut w);
        assert_eq!(
            live.sessions[1],
            Some(Resumable {
                session: "s2".into(),
                spent: 524,
            })
        );
        assert_eq!(journaled(&w, &Event::Resumed { agent: 1 }), 1);
        // Told again: no second briefing, no second attempt.
        live.told(&mut w, 1, "more").unwrap();
        assert!(live.resuming[1].is_none());
        // Nothing happened since: said so, briefly.
        let (mut w, mut live, repo2, root2) = remembered_worker("resume-quiet", true);
        let home2 = live.claude_home.clone();
        live.interrupted[1].clear();
        live.mail[1].clear();
        live.told(&mut w, 1, "go on").unwrap();
        let text = &queued(&live, 1)[0];
        assert!(
            text.contains("Since then:\n- Nothing; go on.\n\ngo on"),
            "{text}"
        );
        // A process that could not start is no resume to fall back from:
        // the next start is a plain one.
        live.ready(&mut w, 1, Err("no such program".into()));
        assert_eq!(w.agents[1].status, Status::Stopped);
        assert!(live.resuming[1].is_none());
        assert_eq!(live.resume_of(1), None);
        assert_eq!(live.floor[1], 0);
        for d in [repo, root, home, repo2, root2, home2] {
            let _ = std::fs::remove_dir_all(d);
        }
    }

    #[test]
    fn a_session_the_vendor_can_not_find_is_started_over_with_the_full_briefing() {
        let (mut w, mut live, repo, root) = remembered_worker("resume-gone", true);
        let home = live.claude_home.clone();
        with_card(&mut w, 1);
        live.notes[1].push("the person applied it".into());
        live.told(&mut w, 1, "go on").unwrap();
        assert!(queued(&live, 1)[0].contains("the person applied it"));
        live.tx
            .send((
                1,
                live.generation[1],
                AgentEvent::Stopped(
                    "stopped: the session could not resume session 686c… (error_during_execution)"
                        .into(),
                ),
            ))
            .unwrap();
        live.step(&mut w);
        // A new session, started over: the old one forgotten, the first
        // prompt as it was, briefed in full, imece's older note kept.
        assert_eq!(live.generation[1], 1, "one session ended, a new one starts");
        assert_eq!(live.sessions[1], None);
        assert_eq!(live.floor[1], 0);
        assert!(live.resuming[1].is_none());
        assert_eq!(live.resume_of(1), None);
        assert!(!live.remembered[1]);
        assert!(live.interrupted[1].is_empty());
        let q = queued(&live, 1);
        assert_eq!(q.len(), 1);
        let text = &q[0];
        for part in [
            "a new session that remembers nothing",
            "Where you left off",
            "Your task: #1 fix it (not finished)",
            "Its prompt was: finish it",
            "1 messages wait for you",
            "the person applied it",
        ] {
            assert!(text.contains(part), "{part:?} in {text}");
        }
        assert_eq!(text.matches("From imece").count(), 1, "{text}");
        assert!(!text.contains("goes on where it was"), "{text}");
        assert!(text.ends_with("\n\ngo on"), "{text}");
        assert_eq!(live.inflight[1], q);
        assert_eq!(live.turns[1], 1);
        assert_eq!(w.agents[1].status, Status::Working);
        assert_eq!(journaled(&w, &Event::Restarted { agent: 1 }), 1);
        assert_eq!(journaled(&w, &Event::Resumed { agent: 1 }), 0);
        assert!(
            !w.journal
                .iter()
                .any(|j| matches!(j.event, Event::Stopped { agent: 1, .. })),
            "not a stop: {:?}",
            w.journal
        );
        assert!(
            w.agents[1]
                .trace
                .iter()
                .any(|l| l.contains("could not be resumed")
                    && l.contains(UUID)
                    && l.contains("error_during_execution")),
            "{:?}",
            w.agents[1].trace
        );
        // A stop of the new session is a stop.
        live.tx
            .send((1, live.generation[1], AgentEvent::Stopped("gone".into())))
            .unwrap();
        live.step(&mut w);
        assert_eq!(w.agents[1].status, Status::Stopped);
        assert_eq!(live.generation[1], 2);
        for d in [repo, root, home] {
            let _ = std::fs::remove_dir_all(d);
        }
    }

    /// [`remembered_worker`] whose worker is a Codex agent (a thread id
    /// instead of a transcript; Codex itself says when a rollout is
    /// gone), the start's own report cut off so it never races the
    /// events a test sends.
    fn remembered_codex_worker(name: &str) -> (World, Live, PathBuf, PathBuf) {
        remembered_vendor_worker(name, Vendor::Codex)
    }

    /// [`remembered_worker`] whose worker runs on `vendor`, a vendor that
    /// says itself when a session is gone, the start's own report cut off
    /// so it never races the events a test sends.
    fn remembered_vendor_worker(name: &str, vendor: Vendor) -> (World, Live, PathBuf, PathBuf) {
        let (mut w, mut live, repo, root) = remembered_worker(name, false);
        live.vendors.push(match vendor {
            Vendor::Codex => Hireable::codex("/nonexistent/codex", &[]),
            Vendor::Gemini => Hireable::gemini("/nonexistent/gemini", &[]),
            Vendor::Claude => Hireable::claude("/nonexistent/claude", &[]),
            other => unreachable!("{other:?} is not driven"),
        });
        w.agents[1].vendor = vendor;
        let (cut, _) = mpsc::channel();
        live.ready_tx = cut;
        (w, live, repo, root)
    }

    #[test]
    fn a_remembered_codex_agent_goes_on_in_its_thread_until_codex_says_it_is_gone() {
        let (mut w, mut live, repo, root) = remembered_codex_worker("resume-codex");
        let home = live.claude_home.clone();
        with_card(&mut w, 1);
        // No transcript to look for: the thread id is what is asked for.
        live.told(&mut w, 1, "go on").unwrap();
        assert_eq!(
            live.resume_of(1),
            Some(Resumable {
                session: UUID.into(),
                spent: 524,
            })
        );
        assert_eq!(live.floor[1], 524);
        assert_eq!(
            live.sessions[1].as_ref().map(|s| s.session.as_str()),
            Some(UUID)
        );
        let text = &queued(&live, 1)[0];
        assert!(text.contains("goes on where it was"), "{text}");
        assert!(text.contains("Its prompt was: finish it"), "{text}");
        assert!(!text.contains("Where you left off"), "{text}");
        assert_eq!(journaled(&w, &Event::Restarted { agent: 1 }), 0);
        // Codex answered with the thread asked for.
        live.tx
            .send((
                1,
                live.generation[1],
                AgentEvent::Started {
                    session: UUID.into(),
                    model: "gpt-6-luna".into(),
                },
            ))
            .unwrap();
        live.step(&mut w);
        assert_eq!(journaled(&w, &Event::Resumed { agent: 1 }), 1);
        assert_eq!(journaled(&w, &Event::Restarted { agent: 1 }), 0);
        assert!(live.resuming[1].is_none());
        assert!(live.interrupted[1].is_empty());
        assert_eq!(live.generation[1], 0);
        for d in [repo, root, home] {
            let _ = std::fs::remove_dir_all(d);
        }

        // Codex found no rollout: the session stops before any turn, and
        // a new one is told where it left off, in full.
        let (mut w, mut live, repo, root) = remembered_codex_worker("resume-codex-gone");
        let home = live.claude_home.clone();
        with_card(&mut w, 1);
        live.told(&mut w, 1, "go on").unwrap();
        live.tx
            .send((
                1,
                live.generation[1],
                AgentEvent::Stopped(
                    "stopped: the session refused Thread: no rollout found for thread id 686c…"
                        .into(),
                ),
            ))
            .unwrap();
        live.step(&mut w);
        assert_eq!(live.generation[1], 1);
        assert_eq!(live.sessions[1], None);
        assert_eq!(live.floor[1], 0);
        assert!(live.resuming[1].is_none());
        assert_eq!(journaled(&w, &Event::Restarted { agent: 1 }), 1);
        assert_eq!(journaled(&w, &Event::Resumed { agent: 1 }), 0);
        assert_eq!(w.agents[1].status, Status::Working);
        let q = queued(&live, 1);
        assert_eq!(q.len(), 1);
        assert!(q[0].contains("Where you left off"), "{}", q[0]);
        assert!(
            q[0].contains("Your task: #1 fix it (not finished)"),
            "{}",
            q[0]
        );
        assert!(!q[0].contains("goes on where it was"), "{}", q[0]);
        assert!(q[0].ends_with("\n\ngo on"), "{}", q[0]);
        assert!(
            w.agents[1]
                .trace
                .iter()
                .any(|l| l.contains("could not be resumed") && l.contains("no rollout found")),
            "{:?}",
            w.agents[1].trace
        );
        for d in [repo, root, home] {
            let _ = std::fs::remove_dir_all(d);
        }
    }

    #[test]
    fn only_a_session_id_shaped_like_a_uuid_is_asked_for() {
        for (name, vendor) in [
            ("resume-codex-bad", Vendor::Codex),
            ("resume-gemini-bad", Vendor::Gemini),
        ] {
            let (mut w, mut live, repo, root) = remembered_vendor_worker(name, vendor);
            let home = live.claude_home.clone();
            live.sessions[1] = Some(Resumable {
                session: "t1; rm -rf /".into(),
                spent: 0,
            });
            live.told(&mut w, 1, "go on").unwrap();
            assert_eq!(live.sessions[1], None, "forgotten");
            assert!(live.resuming[1].is_none());
            assert_eq!(live.resume_of(1), None);
            assert_eq!(journaled(&w, &Event::Restarted { agent: 1 }), 1);
            assert!(queued(&live, 1)[0].contains("Where you left off"));
            for d in [repo, root, home] {
                let _ = std::fs::remove_dir_all(d);
            }
        }
    }

    #[test]
    fn a_remembered_gemini_agent_goes_on_in_its_session_until_gemini_says_it_is_gone() {
        let (mut w, mut live, repo, root) =
            remembered_vendor_worker("resume-gemini", Vendor::Gemini);
        let home = live.claude_home.clone();
        with_card(&mut w, 1);
        // No transcript to look for: the session id is what is asked for.
        live.told(&mut w, 1, "go on").unwrap();
        assert_eq!(
            live.resume_of(1),
            Some(Resumable {
                session: UUID.into(),
                spent: 524,
            })
        );
        assert_eq!(live.floor[1], 524);
        assert_eq!(
            live.sessions[1].as_ref().map(|s| s.session.as_str()),
            Some(UUID)
        );
        let text = &queued(&live, 1)[0];
        assert!(text.contains("goes on where it was"), "{text}");
        assert!(!text.contains("Where you left off"), "{text}");
        assert_eq!(journaled(&w, &Event::Restarted { agent: 1 }), 0);
        // Gemini loaded the session asked for; a Gemini process counts
        // only its own cost, so the floor stays under it.
        live.tx
            .send((
                1,
                live.generation[1],
                AgentEvent::Started {
                    session: UUID.into(),
                    model: "auto".into(),
                },
            ))
            .unwrap();
        live.tx
            .send((
                1,
                live.generation[1],
                AgentEvent::TurnDone {
                    ok: true,
                    text: "done".into(),
                    cost: 7,
                    denied: 0,
                },
            ))
            .unwrap();
        live.step(&mut w);
        assert_eq!(journaled(&w, &Event::Resumed { agent: 1 }), 1);
        assert_eq!(journaled(&w, &Event::Restarted { agent: 1 }), 0);
        assert!(live.resuming[1].is_none());
        assert!(live.interrupted[1].is_empty());
        assert_eq!(live.generation[1], 0);
        assert_eq!(
            live.sessions[1],
            Some(Resumable {
                session: UUID.into(),
                spent: 531,
            })
        );
        for d in [repo, root, home] {
            let _ = std::fs::remove_dir_all(d);
        }

        // Gemini found no chat of it: the session stops before any turn,
        // and a new one is told where it left off, in full.
        let (mut w, mut live, repo, root) =
            remembered_vendor_worker("resume-gemini-gone", Vendor::Gemini);
        let home = live.claude_home.clone();
        with_card(&mut w, 1);
        live.told(&mut w, 1, "go on").unwrap();
        live.tx
            .send((
                1,
                live.generation[1],
                AgentEvent::Stopped(
                    "stopped: the session refused Session: Internal error (Invalid session \
                     identifier \"86935e2a…\".)"
                        .into(),
                ),
            ))
            .unwrap();
        live.step(&mut w);
        assert_eq!(live.generation[1], 1);
        assert_eq!(live.sessions[1], None);
        assert_eq!(live.floor[1], 0);
        assert!(live.resuming[1].is_none());
        assert_eq!(journaled(&w, &Event::Restarted { agent: 1 }), 1);
        assert_eq!(journaled(&w, &Event::Resumed { agent: 1 }), 0);
        assert_eq!(w.agents[1].status, Status::Working);
        let q = queued(&live, 1);
        assert_eq!(q.len(), 1);
        assert!(q[0].contains("Where you left off"), "{}", q[0]);
        assert!(
            q[0].contains("Your task: #1 fix it (not finished)"),
            "{}",
            q[0]
        );
        assert!(!q[0].contains("goes on where it was"), "{}", q[0]);
        assert!(q[0].ends_with("\n\ngo on"), "{}", q[0]);
        assert!(
            w.agents[1].trace.iter().any(|l| {
                l.contains("could not be resumed") && l.contains("Invalid session identifier")
            }),
            "{:?}",
            w.agents[1].trace
        );
        for d in [repo, root, home] {
            let _ = std::fs::remove_dir_all(d);
        }
    }

    #[test]
    fn without_the_transcript_a_remembered_agent_starts_over_and_the_session_is_forgotten() {
        let (mut w, mut live, repo, root) = remembered_worker("resume-none", false);
        let home = live.claude_home.clone();
        live.told(&mut w, 1, "go on").unwrap();
        let text = &queued(&live, 1)[0];
        assert!(text.contains("Where you left off"), "{text}");
        assert!(!text.contains("goes on where it was"), "{text}");
        assert_eq!(live.sessions[1], None);
        assert!(live.resuming[1].is_none());
        assert_eq!(live.floor[1], 0);
        assert!(live.interrupted[1].is_empty());
        assert_eq!(journaled(&w, &Event::Restarted { agent: 1 }), 1);
        // The new session's id is the one remembered from now on.
        live.tx
            .send((
                1,
                live.generation[1],
                AgentEvent::Started {
                    session: "s7".into(),
                    model: "m".into(),
                },
            ))
            .unwrap();
        live.step(&mut w);
        assert_eq!(
            live.sessions[1],
            Some(Resumable {
                session: "s7".into(),
                spent: 0,
            })
        );
        assert_eq!(journaled(&w, &Event::Resumed { agent: 1 }), 0);
        for d in [repo, root, home] {
            let _ = std::fs::remove_dir_all(d);
        }
    }

    #[test]
    fn only_a_claude_session_with_a_uuid_and_its_file_is_resumable() {
        let (mut w, mut live, repo, root) = remembered_worker("resume-who", true);
        let home = live.claude_home.clone();
        let ok = Some(Resumable {
            session: UUID.into(),
            spent: 524,
        });
        assert_eq!(live.resumable(&w, 1), ok);
        assert_eq!(live.sessions[1], ok, "still remembered");
        // A Codex thread and a Gemini session need no file of their own
        // found (those vendors say when one is gone); a vendor imece does
        // not drive remembers none.
        for v in [Vendor::Codex, Vendor::Gemini] {
            w.agents[1].vendor = v;
            assert_eq!(live.resumable(&w, 1), ok, "{v:?}");
            assert_eq!(live.sessions[1], ok, "still remembered");
        }
        w.agents[1].vendor = Vendor::Qwen;
        assert_eq!(live.resumable(&w, 1), None);
        assert_eq!(live.sessions[1], None, "forgotten");
        live.sessions[1] = ok.clone();
        w.agents[1].vendor = Vendor::Claude;
        // An id that is not a UUID never reaches a command line or a path.
        live.sessions[1] = Some(Resumable {
            session: "../x".into(),
            spent: 0,
        });
        assert_eq!(live.resumable(&w, 1), None);
        // The leader runs in the checkout; the worker's file is not its.
        live.sessions[0] = ok.clone();
        assert_eq!(live.resumable(&w, 0), None);
        assert_eq!(live.cwd_of(&w, 0), repo);
        // A helper runs in its worker's worktree.
        w.agents.push(Agent::new(
            2,
            0,
            "claude-1.1".into(),
            Vendor::Claude,
            Role::Helper,
            Status::Asleep,
            0,
        ));
        w.agents[2].parent = Some(1);
        live.grow(None, false);
        assert_eq!(live.cwd_of(&w, 2), live.cwd_of(&w, 1));
        assert_eq!(
            live.cwd_of(&w, 1),
            root.join("claude-1").canonicalize().unwrap()
        );
        live.sessions[2] = ok.clone();
        assert_eq!(live.resumable(&w, 2), ok);
        // Nothing remembered: nothing to resume.
        live.sessions[1] = None;
        assert_eq!(live.resumable(&w, 1), None);
        for d in [repo, root, home] {
            let _ = std::fs::remove_dir_all(d);
        }
    }

    /// Claude's transcript of session `id` for agent `id`'s directory,
    /// written under the live's Claude home.
    fn transcript_of(live: &Live, w: &World, agent: AgentId, id: &str) -> PathBuf {
        let file = claude::transcript(&live.claude_home, &live.cwd_of(w, agent), id);
        std::fs::create_dir_all(file.parent().unwrap()).unwrap();
        std::fs::write(&file, "{}\n").unwrap();
        file
    }

    /// Whether `path` goes within a few seconds: removing is done on a
    /// thread of its own.
    fn gone_soon(path: &Path) -> bool {
        for _ in 0..400 {
            if !path.exists() {
                return true;
            }
            thread::sleep(Duration::from_millis(25));
        }
        false
    }

    /// Whether agent `id`'s trace gets a line with `needle` within a few
    /// seconds of stepping.
    fn traced_soon(live: &mut Live, w: &mut World, id: AgentId, needle: &str) -> bool {
        for _ in 0..400 {
            live.step(w);
            if w.agents[id].trace.iter().any(|l| l.contains(needle)) {
                return true;
            }
            thread::sleep(Duration::from_millis(25));
        }
        eprintln!("{:?}", w.agents[id].trace);
        false
    }

    /// Gemini's registry in `config` naming `cwd` `name`, with a chats
    /// directory holding `files`; gives the kept directory.
    fn gemini_kept(config: &Path, cwd: &Path, name: &str, files: &[(&str, &str)]) -> PathBuf {
        std::fs::create_dir_all(config).unwrap();
        std::fs::write(
            config.join("projects.json"),
            json!({ "projects": { cwd.to_string_lossy(): name } }).to_string(),
        )
        .unwrap();
        let kept = config.join("tmp").join(name);
        std::fs::create_dir_all(kept.join("chats")).unwrap();
        for (file, text) in files {
            std::fs::write(kept.join("chats").join(file), text).unwrap();
        }
        kept
    }

    #[test]
    fn a_session_renewed_after_its_cap_forgets_the_capped_one() {
        let mut w = team(1);
        let mut live = idle_live(2);
        live.ready(&mut w, 1, Ok((Box::new(Stub), PathBuf::from("/w"), None)));
        live.told(&mut w, 1, "go").unwrap();
        live.claude_home = trees("renew-home");
        live.sessions[1] = Some(Resumable {
            session: UUID.into(),
            spent: 0,
        });
        let file = transcript_of(&live, &w, 1, UUID);
        live.tx
            .send((
                1,
                live.generation[1],
                AgentEvent::TurnDone {
                    ok: false,
                    text: CAPPED.into(),
                    cost: 0,
                    denied: 0,
                },
            ))
            .unwrap();
        live.step(&mut w);
        assert!(live.renewed[1]);
        assert_eq!(live.sessions[1], None);
        assert!(gone_soon(&file), "the capped session's transcript goes");
        let _ = std::fs::remove_dir_all(&live.claude_home);
    }

    #[test]
    fn closing_a_project_removes_what_the_vendors_kept_of_its_agents() {
        let repo = checkout("forget-close-repo");
        let dir = trees("forget-close");
        std::fs::create_dir_all(&dir).unwrap();
        let mut w = team(0);
        let mut live = Live::new(
            1,
            vec![
                Hireable::claude("/nonexistent/claude", &[]),
                Hireable::gemini("/nonexistent/gemini", &[]),
            ],
            std::env::temp_dir(),
            dir.join("trees"),
        );
        let (cut, _) = mpsc::channel();
        live.ready_tx = cut;
        live.claude_home = dir.join("claude");
        live.gemini_config = dir.join("gemini");
        live.open_bus(&w, "/opt/imece".into()).unwrap();
        assert_eq!(live.open_project(&mut w, &repo, Some("api")), Ok(1));
        live.hire(&mut w, 1, &json!({ "vendor": "gemini" }))
            .unwrap();
        live.hire(&mut w, 1, &json!({})).unwrap();
        assert_eq!(
            (w.agents[2].name.as_str(), w.agents[3].name.as_str()),
            ("gemini-1", "claude-1")
        );
        // The Claude worker's remembered session has a transcript; the
        // Gemini worker never had a session this run, but its private
        // directory and chats are there from before.
        live.sessions[3] = Some(Resumable {
            session: UUID.into(),
            spent: 1,
        });
        let transcript = transcript_of(&live, &w, 3, UUID);
        let private = live.gemini_home(1, Role::Worker, "gemini-1");
        std::fs::create_dir_all(private.join("policy")).unwrap();
        std::fs::write(private.join("GEMINI.md"), "role").unwrap();
        let kept = gemini_kept(
            &live.gemini_config,
            &private,
            "gemini-1",
            &[("session-2026-10-30T10-00-aaaaaaaa.jsonl", "{}")],
        );
        // A leader that was never a session: nothing to forget, nothing said.
        live.settle = Duration::from_millis(400);
        assert_eq!(live.close_project(&mut w, 1), Ok(()));
        assert_eq!(live.sessions[3], None);
        thread::sleep(Duration::from_millis(100));
        assert!(
            transcript.is_file() && private.is_dir(),
            "the processes are given time to be gone first"
        );
        assert!(gone_soon(&transcript), "the Claude worker's transcript");
        assert!(gone_soon(&private), "the Gemini worker's private directory");
        assert!(gone_soon(&kept), "and what Gemini kept for it");
        assert!(traced_soon(
            &mut live,
            &mut w,
            3,
            &format!("forgot session {UUID}: its transcript was removed")
        ));
        assert!(traced_soon(
            &mut live,
            &mut w,
            2,
            "forgotten: its chats and private directory were removed"
        ));
        assert!(
            !w.agents[1].trace.iter().any(|l| l.contains("forgot")),
            "{:?}",
            w.agents[1].trace
        );
        let _ = std::fs::remove_dir_all(&dir);
        let _ = std::fs::remove_dir_all(&repo);
    }

    #[test]
    fn the_person_can_make_a_remembered_agent_forget_its_session() {
        let (mut w, mut live, repo, root) = remembered_worker("forget", true);
        // No process ran: nothing to wait for.
        live.settle = Duration::from_secs(30);
        let file = claude::transcript(&live.claude_home, &live.cwd_of(&w, 1), UUID);
        assert!(file.is_file());
        // The leader never had a session: nothing to forget.
        let e = live.forget_agent(&mut w, 0).unwrap_err();
        assert!(
            matches!(&e, Refusal::Command(why) if why == "it has no session to forget"),
            "{e:?}"
        );
        assert_eq!(journaled(&w, &Event::Forgotten { agent: 0 }), 0);

        assert_eq!(live.forget_agent(&mut w, 1), Ok(()));
        assert_eq!(live.sessions[1], None);
        assert!(gone_soon(&file));
        assert_eq!(journaled(&w, &Event::Forgotten { agent: 1 }), 1);
        assert_eq!(w.agents[1].status, Status::Asleep);
        assert!(live.remembered[1]);
        assert_eq!(
            live.interrupted[1],
            ["finish it"],
            "the cut turn is still told"
        );
        assert!(
            w.agents[1]
                .trace
                .iter()
                .any(|l| l.contains("the person made it forget its session"))
        );
        // Twice is nothing to forget.
        assert!(live.forget_agent(&mut w, 1).is_err());
        assert_eq!(journaled(&w, &Event::Forgotten { agent: 1 }), 1);
        // Its next message starts a new session, briefed in full.
        live.told(&mut w, 1, "go on").unwrap();
        assert_eq!(journaled(&w, &Event::Restarted { agent: 1 }), 1);
        assert_eq!(journaled(&w, &Event::Resumed { agent: 1 }), 0);
        let q = queued(&live, 1);
        assert!(
            q[0].contains("Where you left off") && q[0].contains("finish it"),
            "{}",
            q[0]
        );
        assert!(live.resuming[1].is_none());
        // A gone agent has nothing.
        w.agents[1].gone = true;
        live.sessions[1] = Some(Resumable {
            session: UUID.into(),
            spent: 0,
        });
        assert!(live.forget_agent(&mut w, 1).is_err());
        let home = live.claude_home.clone();
        for d in [repo, root, home] {
            let _ = std::fs::remove_dir_all(d);
        }
    }

    #[test]
    fn forgetting_a_running_agent_ends_its_session_and_keeps_its_cut_prompts_to_tell() {
        use std::os::unix::process::CommandExt;
        let mut w = team(1);
        let mut live = idle_live(2);
        let ended = Arc::new(AtomicBool::new(false));
        live.ready(
            &mut w,
            1,
            Ok((Box::new(Ends(ended.clone())), PathBuf::from("/w"), None)),
        );
        live.told(&mut w, 1, "do x").unwrap();
        live.told(&mut w, 1, "then y").unwrap();
        assert_eq!(live.inflight[1], ["do x", "then y"]);
        assert!(live.asked[1]);
        assert_eq!(w.agents[1].status, Status::Working);
        let generation = live.generation[1];
        let mut sleeper = std::process::Command::new("sleep")
            .arg("30")
            .process_group(0)
            .spawn()
            .unwrap();
        live.running[1].add(sleeper.id() as i32);
        assert_eq!(live.forget_agent(&mut w, 1), Ok(()));
        assert!(
            ended.load(Ordering::SeqCst),
            "the session was asked to stop"
        );
        assert!(matches!(live.slots[1], Slot::None));
        assert_eq!(live.generation[1], generation + 1);
        assert!(live.inflight[1].is_empty());
        assert_eq!(live.interrupted[1], ["do x", "then y"]);
        assert!(!live.asked[1]);
        assert!(live.remembered[1]);
        assert_eq!(w.agents[1].status, Status::Asleep);
        assert_eq!(live.turns[1], 0);
        assert_eq!(journaled(&w, &Event::Forgotten { agent: 1 }), 1);
        for _ in 0..200 {
            if sleeper.try_wait().unwrap().is_some() {
                break;
            }
            thread::sleep(Duration::from_millis(25));
        }
        assert!(sleeper.try_wait().unwrap().is_some(), "its run was killed");
        // Mail, notes and helpers stay: only the session is gone.
        live.mail[1].push(Mail {
            from: 0,
            kind: MsgKind::Inform,
            body: "kept".into(),
        });
        assert_eq!(live.mail[1].len(), 1);
    }

    #[test]
    fn forgetting_a_codex_or_gemini_agent_takes_what_its_vendor_kept() {
        let (mut w, mut live, repo, root) = remembered_vendor_worker("forget-codex", Vendor::Codex);
        assert_eq!(live.forget_agent(&mut w, 1), Ok(()));
        assert_eq!(live.sessions[1], None);
        // Codex is asked through a process of its own; here there is none.
        assert!(traced_soon(
            &mut live,
            &mut w,
            1,
            &format!("could not remove its thread of session {UUID}: ")
        ));
        let home = live.claude_home.clone();
        for d in [repo, root, home] {
            let _ = std::fs::remove_dir_all(d);
        }

        let (mut w, mut live, repo, root) =
            remembered_vendor_worker("forget-gemini", Vendor::Gemini);
        live.gemini_config = trees("forget-gemini-config");
        let name = w.agents[1].name.clone();
        let private = live.gemini_home(0, Role::Worker, &name);
        std::fs::create_dir_all(&private).unwrap();
        let header = json!({ "sessionId": UUID, "kind": "main" }).to_string();
        let kept = gemini_kept(
            &live.gemini_config,
            &private,
            "g",
            &[
                (
                    &format!("session-2026-10-30T10-00-{}.jsonl", &UUID[..8]),
                    &header,
                ),
                ("session-2026-10-30T10-00-bbbbbbbb.jsonl", ""),
                (
                    "session-2026-10-30T10-01-cccccccc.jsonl",
                    "{\"sessionId\":\"other\"}",
                ),
            ],
        );
        assert_eq!(live.forget_agent(&mut w, 1), Ok(()));
        assert!(traced_soon(
            &mut live,
            &mut w,
            1,
            &format!("forgot session {UUID}: 2 chat files were removed")
        ));
        assert!(
            kept.join("chats")
                .join("session-2026-10-30T10-01-cccccccc.jsonl")
                .is_file(),
            "another session's chat stays"
        );
        assert!(
            private.is_dir(),
            "the agent stays, so its private directory does"
        );
        let home = live.claude_home.clone();
        for d in [repo, root, home, live.gemini_config.clone()] {
            let _ = std::fs::remove_dir_all(d);
        }
    }

    #[test]
    fn fresh_forgets_every_remembered_session_when_the_world_is_taken_in() {
        for fresh in [false, true] {
            let repo = checkout(&format!("fresh-{fresh}"));
            let root = trees(&format!("fresh-{fresh}"));
            let mut w = team(2);
            let mut live = Live::new(
                0,
                claude_at("/nonexistent/claude"),
                repo.clone(),
                root.clone(),
            );
            live.claude_home = trees(&format!("fresh-{fresh}-home"));
            live.fresh = fresh;
            // No process ran at the start: nothing to wait for.
            live.settle = Duration::from_secs(30);
            let file = transcript_of(&live, &w, 1, UUID);
            // A worker that is gone with a session still remembered: it
            // goes too, with no journal line for an agent that is gone.
            w.agents[2].gone = true;
            let other = "aaaaaaaa-0000-4000-8000-000000000000";
            let left = transcript_of(&live, &w, 2, other);
            let found = Found {
                sessions: vec![
                    None,
                    Some(Resumable {
                        session: UUID.into(),
                        spent: 3,
                    }),
                    Some(Resumable {
                        session: other.into(),
                        spent: 0,
                    }),
                ],
                ..Default::default()
            };
            live.adopt(&mut w, found, 0);
            assert_eq!(w.agents[1].status, Status::Asleep);
            assert!(live.remembered[1]);
            if fresh {
                assert_eq!(live.sessions[1], None);
                assert_eq!(live.sessions[2], None);
                assert!(gone_soon(&file), "the transcript goes");
                assert!(gone_soon(&left), "the gone worker's too");
                assert_eq!(
                    journaled(&w, &Event::Forgotten { agent: 2 }),
                    0,
                    "it is gone"
                );
                assert_eq!(journaled(&w, &Event::Forgotten { agent: 1 }), 1);
                assert_eq!(
                    journaled(&w, &Event::Forgotten { agent: 0 }),
                    0,
                    "it had none"
                );
                assert!(
                    w.agents[1]
                        .trace
                        .iter()
                        .any(|l| l.contains("started with --fresh"))
                );
            } else {
                assert_eq!(live.sessions[1].as_ref().map(|s| s.spent), Some(3));
                thread::sleep(Duration::from_millis(100));
                assert!(file.is_file(), "kept");
                assert_eq!(journaled(&w, &Event::Forgotten { agent: 1 }), 0);
            }
            let home = live.claude_home.clone();
            for d in [repo, root, home] {
                let _ = std::fs::remove_dir_all(d);
            }
        }
    }

    #[test]
    fn the_fresh_flag_reaches_the_world_that_is_loaded() {
        let repo = checkout("fresh-flag");
        let state = trees("fresh-flag-state");
        let options = |fresh| Options {
            repo: repo.clone(),
            state: state.clone(),
            max_workers: 2,
            max_helpers: 1,
            lead: Vendor::Claude,
            tiers: Vec::new(),
            budget: None,
            fresh,
            run_programs: Vec::new(),
            run_timeout: DEFAULT_RUN_TIMEOUT,
        };
        let present = [(Vendor::Claude, true)];
        let (w, mut live) = world(options(false), &present).unwrap();
        live.sessions[0] = Some(Resumable {
            session: UUID.into(),
            spent: 1,
        });
        assert_eq!(live.persist(&w), Ok(true));
        drop(live);
        let (w, live) = world(options(false), &present).unwrap();
        assert!(live.sessions[0].is_some(), "kept without the flag");
        assert_eq!(journaled(&w, &Event::Forgotten { agent: 0 }), 0);
        drop(live);
        let (w, live) = world(options(true), &present).unwrap();
        assert!(live.fresh);
        assert_eq!(live.sessions[0], None);
        assert_eq!(journaled(&w, &Event::Forgotten { agent: 0 }), 1);
        drop(live);
        let _ = std::fs::remove_dir_all(&repo);
        let _ = std::fs::remove_dir_all(&state);
    }
}
