//! Live agents: real Claude Code sessions feed the model the simulation
//! feeds. One project (the checkout imece runs in), a leader that reads it,
//! and workers that each change their own git worktree of it.
//!
//! Money is reserved up front: every agent gets an equal share of the
//! budget as its session's hard cap (`--max-budget-usd`), so all of them
//! together can never spend more than the budget. An agent that spent its
//! share is not prompted again.

use std::path::{Path, PathBuf};
use std::sync::mpsc::{self, Receiver, Sender};
use std::thread;
use std::time::{Instant, SystemTime, UNIX_EPOCH};

use imece_agent::claude::{self, Session, Spec};
use imece_agent::{Event as AgentEvent, child, worktree};

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
    let live = Live::new(n, program, repo, root, o.model, o.budget / n as u64);
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
        }
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
            if let AgentEvent::Stopped(_) = e {
                self.end(w, id);
            }
            apply(w, id, e, self.spent_before[id]);
        }
    }

    /// A session ended: the next message starts a new one.
    fn end(&mut self, w: &World, id: AgentId) {
        self.spent_before[id] = w.agents[id].cost;
        self.slots[id] = Slot::None;
        self.generation[id] += 1;
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
        if w.agents[id].cost >= self.share || w.spent() >= w.daily_budget {
            return Err(Refusal::Budget);
        }
        let t = w.tick;
        match &mut self.slots[id] {
            Slot::None => {
                self.start(w, id);
                self.slots[id] = Slot::Starting(vec![text.to_string()]);
            }
            Slot::Starting(q) => q.push(text.to_string()),
            Slot::Running(s) => {
                if let Err(e) = s.prompt(text) {
                    self.end(w, id);
                    return Err(Refusal::Start(e.to_string()));
                }
            }
        }
        let a = &w.agents[id];
        if a.role == Role::Worker && a.status != Status::Working {
            let card = w.cards.len();
            w.cards.push(Card {
                id: card,
                project: a.project,
                title: text.lines().next().unwrap_or_default().to_string(),
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
             change them or run commands. The person talks to you: answer them \
             directly and briefly. You can not message the workers yet, so when \
             work should be done, say which worker should do what, and the \
             person passes it on.",
            a.name,
            workers.join(", ")
        ),
        _ => format!(
            "You are {}, a coding agent in imece, a team led by {leader}. You work \
             in your own git worktree of the repository (your current \
             directory); the others work in theirs. You can read and change \
             files here but not run commands. Do not commit. End every turn \
             with a short summary of what you changed and what is left open.",
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
