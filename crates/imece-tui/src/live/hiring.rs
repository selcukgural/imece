//! Hiring: Claude Code's tier ladder, `hire`,
//! and the fall-back to a default model.

use super::*;

impl Ladder {
    pub(super) fn model(&self, tier: Tier) -> Option<&str> {
        match tier {
            Tier::Cheap => &self.cheap,
            Tier::Standard => &self.standard,
            Tier::Strong => &self.strong,
        }
        .as_deref()
    }

    pub(super) fn set(&mut self, tier: Tier, model: String) {
        *match tier {
            Tier::Cheap => &mut self.cheap,
            Tier::Standard => &mut self.standard,
            Tier::Strong => &mut self.strong,
        } = Some(model);
    }
}

impl Hireable {
    /// Claude Code at `program`, on its own ladder, with the person's
    /// stand-ins `tiers` on top.
    pub(super) fn new(program: &str, tiers: &[(Tier, String)]) -> Hireable {
        let own = |tier| claude::model(tier).map(String::from);
        let mut ladder = Ladder {
            cheap: own(Tier::Cheap),
            standard: own(Tier::Standard),
            strong: own(Tier::Strong),
        };
        for (tier, model) in tiers {
            ladder.set(*tier, model.clone());
        }
        Hireable {
            program: program.to_string(),
            ladder,
        }
    }
}

/// Whether `program` runs from `repo` with a cleared environment.
pub(super) fn on_path(program: &str, repo: &Path) -> bool {
    child::command(program, repo, &[])
        .arg("--version")
        .output()
        .is_ok_and(|o| o.status.success())
}

/// A new worker's name (`claude-2`): the next number, counting the workers
/// that left, skipping names taken.
fn worker_name(w: &World) -> String {
    let n = (w.agents.iter()).filter(|o| o.role == Role::Worker).count();
    free_name(w, &format!("{CLAUDE}-"), n + 1)
}

/// `stem` and the first number from `next` on that names nobody. A
/// bounded search: there are fewer agents than the numbers it tries.
fn free_name(w: &World, stem: &str, next: usize) -> String {
    (next..=next.saturating_add(w.agents.len()))
        .map(|n| format!("{stem}{n}"))
        .find(|name| !w.name_taken(name))
        .expect("some number is free")
}

/// What a hirer hears of `from` on Lonca: a workspace is made of trunk.
const ON_LONCA_FROM: &str = "a worker on Lonca starts at trunk: from is for a worktree";

/// What a hirer hears while its project's store does not answer.
fn store_closed(why: &str) -> String {
    format!("nobody is hired now: {why}; only the person can bring Lonca back")
}

impl Live {
    /// Makes room for one more agent, on `model` (`tiered`: picked by
    /// tier, so it may fall back).
    pub(super) fn grow(&mut self, model: Option<String>, tiered: bool) {
        self.remembered.push(false);
        self.refused.push(None);
        self.interrupted.push(Vec::new());
        self.sessions.push(None);
        self.floor.push(0);
        self.resuming.push(None);
        self.forks.push(None);
        self.slots.push(Slot::None);
        self.generation.push(0);
        self.spent_before.push(0);
        self.models.push(model);
        self.tiered.push(tiered);
        self.turned.push(false);
        self.inflight.push(Vec::new());
        self.renewed.push(false);
        self.off_lonca.push(false);
        self.leaving.push(false);
        self.lost.push(false);
        self.notes.push(Vec::new());
        self.mail.push(Vec::new());
        self.turns.push(0);
        self.took.push(0);
        self.asked.push(false);
        self.woken.push(0);
        self.posted.push(0);
        self.cwds.push(None);
        self.running.push(run::Running::default());
        self.pending.push(Vec::new());
        self.reading.push(0);
        self.applying.push(false);
        self.placing.push(None);
        self.standing.push(None);
        self.stood_at.push(None);
    }

    /// Agent `id`'s tier model is not available here: its session ends and
    /// a new one on the standard model gets the prompts the old
    /// one had.
    pub(super) fn fall_back(&mut self, w: &mut World, id: AgentId) {
        let model = self.models[id].take().unwrap_or_default();
        self.models[id] = self.claude.ladder.standard.clone();
        self.tiered[id] = false;
        let texts = std::mem::take(&mut self.inflight[id]);
        // The turn that failed said what it cost: no turn is cut.
        self.turns[id] = self.turns[id].saturating_sub(1);
        self.end(w, id);
        self.forget(w, id, true);
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
    pub(super) fn renew(&mut self, w: &mut World, id: AgentId) {
        let texts = std::mem::take(&mut self.inflight[id]);
        // The turn at the cap said what it cost: no turn is cut.
        self.turns[id] = self.turns[id].saturating_sub(1);
        self.end(w, id);
        self.forget(w, id, true);
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

    /// How many agent `from` has hired that are still here and how many
    /// it may have, the leader's workers or a worker's (or solo's)
    /// helpers; why not when it may hire none more. Helpers and
    /// researchers hire nobody.
    pub(super) fn hire_room(&self, w: &World, from: AgentId) -> Result<(usize, usize), String> {
        let hirer = &w.agents[from];
        if let Some(why) = w.store_closed(hirer.project) {
            return Err(store_closed(why));
        }
        match hirer.role {
            Role::Leader => {
                let workers = w
                    .agents
                    .iter()
                    .filter(|o| o.project == hirer.project && o.role == Role::Worker && !o.gone)
                    .count();
                if self.max_workers == 0 {
                    return Err("the person runs you without workers (--agents 0); answer \
                                from what you read"
                        .into());
                }
                if workers >= self.max_workers {
                    return Err(format!(
                        "the team is full: {workers} workers, the most the person allowed \
                         (--agents {}); give the work to one of them",
                        self.max_workers
                    ));
                }
                // The plan gate (record 114): a second worker waits for
                // the person's yes to a plan.
                match w.plan_hold(hirer.project) {
                    None => {}
                    Some(PlanHold::NoPlan) => {
                        return Err("make a plan first (the plan tool): the person reads it \
                                    before a second worker is hired"
                            .into());
                    }
                    Some(PlanHold::Reading) => {
                        return Err("the person is reading your plan; end your turn, their \
                                    yes starts your next"
                            .into());
                    }
                    Some(PlanHold::Unanswered) => {
                        return Err("the person answered your plan in words, not with a \
                                    yes: make a new plan from what they said (the plan tool)"
                            .into());
                    }
                }
                Ok((workers, self.max_workers))
            }
            Role::Worker | Role::Solo => {
                let helpers = w.helpers_of(from).count();
                if self.max_helpers == 0 {
                    return Err(
                        "the person runs you without helpers (--helpers 0); do it yourself".into(),
                    );
                }
                if helpers >= self.max_helpers {
                    return Err(format!(
                        "you have {helpers} helpers, the most the person allowed (--helpers \
                         {}); wait for one to send done",
                        self.max_helpers
                    ));
                }
                Ok((helpers, self.max_helpers))
            }
            _ => Err("only the leader and workers hire; ask yours with send".into()),
        }
    }

    /// `whoami`'s answer for agent `from`: its name, role and project, who
    /// it answers to, what is left of the pot, whether it may hire (as
    /// `hire` would answer), and the tests its project's files call for
    /// (record 136), when they call for any.
    pub(super) fn whoami(&self, w: &World, from: AgentId) -> String {
        let a = &w.agents[from];
        let project = &w.projects[a.project];
        let role = match a.role {
            Role::Leader => "the leader",
            Role::Worker => "a worker",
            Role::Helper => "a helper",
            Role::Researcher => "a researcher",
            Role::Solo => "a solo session the person drives",
        };
        let over = match (a.role, a.parent) {
            (Role::Helper, Some(p)) => format!("{}, who hired you", w.agents[p].name),
            (Role::Leader | Role::Solo, _) => "the person (send to: you)".to_string(),
            _ => format!("the leader {}", w.agents[project.leader].name),
        };
        let left = w.budget.saturating_sub(w.spent());
        let hiring = match self.hire_room(w, from) {
            Ok((had, cap)) => {
                let whom = match (a.role == Role::Leader, cap - had) {
                    (true, 1) => "worker",
                    (true, _) => "workers",
                    (false, 1) => "helper",
                    (false, _) => "helpers",
                };
                format!(
                    "yes, {} more {whom} (you have {had} of at most {cap})",
                    cap - had
                )
            }
            Err(why) => format!("no: {why}"),
        };
        let mut out = format!(
            "name: {}\nrole: {role}\nproject: {}\nyou answer to: {over}\nbudget left: {} of \
             {} (one pot for every agent)\nmay hire: {hiring}",
            a.name,
            project.name,
            dollars(left),
            dollars(w.budget),
        );
        if a.role == Role::Leader {
            let free: Vec<&str> = (w.free_workers(a.project).into_iter())
                .map(|id| w.agents[id].name.as_str())
                .collect();
            out.push_str(&match free.as_slice() {
                [] => "\nfree workers: none".to_string(),
                names => format!(
                    "\nfree workers: {} (idle, no card at work: give one the next card \
                     before you hire)",
                    names.join(", ")
                ),
            });
        }
        if project.runners.any() {
            out.push_str(&format!(
                "\ntests that run at once, through the run tool too: {}",
                project.runners.lines().join(", ")
            ));
        }
        out
    }

    /// The leader `from` adds a worker, or the worker `from` adds a helper
    /// (the `hire` tool's arguments). The answer names it.
    pub(super) fn hire(&mut self, w: &mut World, from: AgentId, args: &Value) -> Answer {
        let hirer = &w.agents[from];
        let project = hirer.project;
        let role = match hirer.role {
            Role::Leader => Role::Worker,
            Role::Worker | Role::Solo => Role::Helper,
            _ => return Err("only the leader and workers hire; ask yours with send".into()),
        };
        let (so_far, cap) = self.hire_room(w, from)?;
        let arg = |k: &str| args[k].as_str().map(str::trim).unwrap_or_default();
        let tier = match arg("tier") {
            "" => Tier::Standard,
            t => Tier::from_wire(t).ok_or("tier must be cheap, standard or strong")?,
        };
        let start = match arg("from") {
            "" => None,
            _ if role != Role::Worker => {
                return Err("from is for the leader's workers: a helper works in your \
                            worktree, wherever it started"
                    .into());
            }
            _ if w.projects[project].lonca.is_some() => {
                return Err(ON_LONCA_FROM.into());
            }
            f => Some(self.start_point(project, f)?),
        };
        let model = self.claude.ladder.model(tier).map(str::to_string);
        // A free worker of the same tier takes the next card: a new one
        // costs a session and a worktree for nothing.
        if role == Role::Worker
            && start.is_none()
            && let Some(free) = (w.free_workers(project).into_iter())
                .find(|&id| self.models.get(id).is_some_and(|m| *m == model))
        {
            return Err(format!(
                "{} is free (idle, no card at work) on that tier: give it the next card \
                 with send (type assign) instead of hiring",
                w.agents[free].name
            ));
        }
        // Only a model other than the standard one has something to fall
        // back to.
        let tiered = model != self.claude.ladder.standard;
        // The next number, skipping names already taken: a worker's
        // `claude-2`, a helper's after its worker
        // (`claude-2.1`), counting the ones that left.
        let name = if role == Role::Worker {
            worker_name(w)
        } else {
            let n = w
                .agents
                .iter()
                .filter(|o| o.role == Role::Helper && o.parent == Some(from))
                .count();
            free_name(w, &format!("{}.", w.agents[from].name), n + 1)
        };
        let id = w.agents.len();
        // The socket first: an agent without one could never start.
        self.bind(id, role)
            .map_err(|e| format!("{name} could not get a socket: {e}"))?;
        self.grow(model.clone(), tiered);
        let t = w.tick;
        let mut agent = Agent::new(id, project, name.clone(), role, Status::Idle, t);
        agent.parent = (role == Role::Helper).then_some(from);
        agent.start.clone_from(&start);
        w.agents.push(agent);
        let shown = model.clone().unwrap_or_else(|| "the default model".into());
        let by = w.agents[from].name.clone();
        w.agents[from].trace(t, format!("hire {name} {} {shown}", tier.wire()));
        w.agents[id].trace(t, format!("hired by {by} · {shown}"));
        let starting = match &start {
            Some(s) => {
                w.agents[id].trace(t, format!("worktree from {s}"));
                format!(", its worktree from {s}")
            }
            None => String::new(),
        };
        w.note(Event::Hired {
            by: from,
            agent: id,
            model,
            from: start,
        });
        Ok(format!(
            "{name} joined ({}, {shown}){starting}. Give it work with send (type assign). \
             {} more may be hired.",
            CLAUDE,
            cap - so_far - 1
        ))
    }

    /// The leader `from` puts its plan before the person (the `plan`
    /// tool's arguments, record 114): the cards it means to assign or
    /// post, each checked as an assign's would be (a title of one line,
    /// a body, a tier, paths; no two cards of the plan owning the same
    /// path), and what it expects them to cost. With the gate off for the
    /// project, nothing waits: the leader is told to hire as it sees fit.
    pub(super) fn plan(&mut self, w: &mut World, from: AgentId, args: &Value) -> Answer {
        let hirer = &w.agents[from];
        if hirer.role != Role::Leader {
            return Err("only the leader plans; it hires for the team".into());
        }
        let project = hirer.project;
        let Some(list) = args["cards"].as_array() else {
            return Err("cards must be an array of cards".into());
        };
        if list.is_empty() || list.len() > MAX_PLANNED {
            return Err(format!("a plan has 1 to {MAX_PLANNED} cards"));
        }
        let mut cards: Vec<Planned> = Vec::new();
        for (n, c) in list.iter().enumerate() {
            let arg = |k: &str| c[k].as_str().map(str::trim).unwrap_or_default();
            let (title, body) = (arg("title"), arg("body"));
            if title.is_empty() || body.is_empty() {
                return Err(format!("card {} needs a title and a body", n + 1));
            }
            if title.contains('\n') || title.chars().count() > MAX_TITLE {
                return Err(format!(
                    "card {}'s title must be one line of at most {MAX_TITLE} characters",
                    n + 1
                ));
            }
            if body.chars().count() > MAX_BODY {
                return Err(format!(
                    "card {}'s body is longer than {MAX_BODY} characters",
                    n + 1
                ));
            }
            let tier = match arg("tier") {
                "" => Tier::Standard,
                t => Tier::from_wire(t).ok_or(format!(
                    "card {}: tier must be cheap, standard or strong",
                    n + 1
                ))?,
            };
            let paths =
                paths::parse(&c["paths"]).map_err(|why| format!("card {}: {why}", n + 1))?;
            let checks = checks::parse(&c["checks"])
                .and_then(|checks| checks_run(w, project, &checks).map(|_| checks))
                .map_err(|why| format!("card {}: {why}", n + 1))?;
            let after = planned_after(&c["after"], n)?;
            for (m, other) in cards.iter().enumerate() {
                if let Some((theirs, mine)) = paths::overlap(&other.paths, &paths) {
                    return Err(format!(
                        "cards {} and {} both own a path: {theirs:?} and {mine:?}; give one \
                         other paths, or end one with the word shared",
                        m + 1,
                        n + 1
                    ));
                }
            }
            cards.push(Planned {
                title: title.to_string(),
                tier: tier.wire().to_string(),
                paths,
                checks,
                body: body.to_string(),
                after,
            });
        }
        let Some(cost) = args["cost"].as_f64().filter(|c| c.is_finite() && *c >= 0.0) else {
            return Err("cost must be a number of dollars, 0 or more".into());
        };
        // Ten-thousandths of a dollar, as the pot counts; a figure past
        // any pot saturates, not refused.
        let cost = (cost * 10_000.0).round() as u64;
        if !w.projects[project].plan.on {
            return Ok(
                "the person turned the plan gate off for this project: hire as you \
                       see fit; your plan was not kept"
                    .into(),
            );
        }
        let n = cards.len();
        let t = w.tick;
        let item = w.propose(
            project,
            Proposal {
                cards,
                cost,
                tick: t,
            },
        );
        w.agents[from].trace(t, format!("plan {n} cards {} (item {item})", dollars(cost)));
        Ok(format!(
            "the person reads your plan ({n} cards, {}) in TO DECIDE; end your turn, \
             their yes starts your next",
            dollars(cost)
        ))
    }

    /// The person's answer to plan item `item` (record 114): the world
    /// opens or shuts the gate, and the leader hears it as soon as it is
    /// free (a note with the next prompt, which comes at once when its
    /// turn is over). False when no plan waits there.
    pub fn decide_plan(&mut self, w: &mut World, item: usize, yes: bool) -> bool {
        let Some(leader) = w.decide_plan(item, yes) else {
            return false;
        };
        let said = if yes { PLAN_YES } else { PLAN_NO };
        self.notes[leader].push(said.to_string());
        self.lost[leader] = true;
        w.agents[leader].trace(w.tick, format!("plan {}", if yes { "yes" } else { "no" }));
        true
    }

    /// `:fork` (record 94): a new worker that goes on from worker `from`'s
    /// conversation and uncommitted change, on its model, in a worktree
    /// of its own made at `from`'s HEAD; asleep until it is told
    /// something, when its session forks from `from`'s as it is then and
    /// its worktree takes `from`'s change. Counted in `--agents` as any
    /// worker, its turns in the pot. Refused, with why, for what has no
    /// session or worktree to fork, a worker mid-turn, or a full team.
    pub fn fork(&mut self, w: &mut World, from: AgentId) -> Result<AgentId, String> {
        let a = &w.agents[from];
        if a.role != Role::Worker || a.gone {
            return Err(
                "only a worker forks: it has a conversation and a worktree of its own".into(),
            );
        }
        if let Some(why) = w.store_closed(a.project) {
            return Err(store_closed(why));
        }
        if self.sessions[from].is_none() {
            return Err("it has no session to fork yet: tell it something first".into());
        }
        if self.turns[from] > 0 || !matches!(self.slots[from], Slot::None | Slot::Running(_)) {
            return Err("it is mid-turn: fork it when its turn ends".into());
        }
        let project = a.project;
        let workers = (w.agents.iter())
            .filter(|o| o.project == project && o.role == Role::Worker && !o.gone)
            .count();
        if workers >= self.max_workers {
            return Err(format!(
                "the team has {workers} workers, the most --agents allows ({})",
                self.max_workers
            ));
        }
        let cwd = self.cwd_of(w, from);
        if imece_agent::lonca::is_workspace(&cwd) {
            return Err(ON_LONCA_FORK.into());
        }
        let head = worktree::resolve(&cwd, "HEAD").map_err(|e| format!("its worktree: {e}"))?;
        let name = worker_name(w);
        let id = w.agents.len();
        self.bind(id, Role::Worker)
            .map_err(|e| format!("{name} could not get a socket: {e}"))?;
        self.grow(self.models[from].clone(), self.tiered[from]);
        let t = w.tick;
        let mut agent = Agent::new(id, project, name.clone(), Role::Worker, Status::Idle, t);
        agent.start = Some(head.clone());
        w.agents.push(agent);
        self.forks[id] = Some(from);
        self.remembered[id] = true;
        let was = w.agents[from].name.clone();
        w.agents[from].trace(t, format!("forked into {name}"));
        w.agents[id].trace(t, format!("forked from {was} · worktree from {head}"));
        w.note(Event::Forked { from, agent: id });
        Ok(id)
    }

    /// `hire`'s `from` for a worker of `project`, checked: one name, and a
    /// commit the checkout has now (`#N`, a pull request, is fetched when
    /// the worker starts; one that is not there stops it with git's words).
    fn start_point(&self, project: ProjectId, from: &str) -> Result<String, String> {
        if !worktree::valid_start(from) {
            return Err(format!(
                "from must be one branch, tag or commit of the checkout, or #N for pull \
                 request N of origin; {from:?} is not"
            ));
        }
        if !from.starts_with('#') {
            worktree::resolve(&self.places[project].repo, from).map_err(|e| {
                format!("{e}; give a branch, tag or commit the checkout has, or #N")
            })?;
        }
        Ok(from.to_string())
    }
}

/// What card `n` (from 0) of a plan waits on (record 126, F10): the
/// places of earlier cards of the plan, from 1, each once, oldest first.
fn planned_after(v: &Value, n: usize) -> Result<Vec<usize>, String> {
    if v.is_null() {
        return Ok(Vec::new());
    }
    let bad = || {
        format!(
            "card {}: after names earlier cards of this plan by their place (1 to {n})",
            n + 1
        )
    };
    let list = v.as_array().ok_or_else(bad)?;
    let mut out = Vec::with_capacity(list.len());
    for item in list {
        let at = (item.as_u64())
            .and_then(|m| usize::try_from(m).ok())
            .filter(|&m| (1..=n).contains(&m))
            .ok_or_else(bad)?;
        out.push(at);
    }
    out.sort_unstable();
    out.dedup();
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::live::testing::*;

    /// A stand-in for `claude` that adds its arguments, one per line and
    /// `---` after them, to `args.txt` where it runs, and exits.
    fn appending_claude(dir: &Path) -> String {
        use std::os::unix::fs::PermissionsExt;
        let path = dir.join("appending-claude");
        std::fs::write(
            &path,
            "#!/bin/sh\nprintf '%s\\n' \"$@\" >> args.txt\necho --- >> args.txt\n",
        )
        .unwrap();
        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o700)).unwrap();
        path.to_string_lossy().into_owned()
    }

    #[test]
    fn a_worker_on_lonca_starts_at_trunk_and_does_not_fork() {
        let repo = checkout("on-lonca");
        let root = trees("on-lonca");
        let mut w = team(1);
        w.agents[1].status = Status::Idle;
        w.on_lonca(0, Some("demo".into()));
        let up = StoreSeen::Up {
            trunk: "t".into(),
            pending: 0,
            behind: Behind::Unknown,
        };
        w.store_seen(0, "demo", up);
        let mut live = Live::new(2, claude_at("no-such-claude"), repo.clone(), root.clone());
        live.max_workers = 3;
        let e = live
            .hire(&mut w, 0, &json!({ "from": "main" }))
            .unwrap_err();
        assert_eq!(e, ON_LONCA_FROM);
        assert_eq!(w.agents.len(), 2, "nobody joined");

        live.sessions[1] = Some(Resumable {
            session: "s".into(),
            spent: 0,
            models: None,
        });
        std::fs::create_dir_all(root.join("claude-1/.lonca")).unwrap();
        std::fs::write(root.join("claude-1/.lonca/workspace.json"), "{}").unwrap();
        assert_eq!(live.fork(&mut w, 1), Err(ON_LONCA_FORK.to_string()));
        assert_eq!(w.agents.len(), 2, "nobody joined");
        let _ = std::fs::remove_dir_all(&repo);
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn a_fork_is_refused_for_what_cannot_fork_and_says_why() {
        let repo = checkout("fork-no");
        let root = trees("fork-no");
        let mut w = team(1);
        w.agents[1].status = Status::Idle;
        let mut live = Live::new(
            2,
            claude_at("/nonexistent/claude"),
            repo.clone(),
            root.clone(),
        );
        live.max_workers = 2;
        let no = |live: &mut Live, w: &mut World, id| live.fork(w, id).unwrap_err();
        let only = "only a worker forks: it has a conversation and a worktree of its own";
        assert_eq!(no(&mut live, &mut w, 0), only, "the leader");
        assert_eq!(
            no(&mut live, &mut w, 1),
            "it has no session to fork yet: tell it something first"
        );
        worker_in(&mut live, &mut w, 1, &repo, &root);
        live.sessions[1] = Some(Resumable {
            session: UUID.into(),
            spent: 0,
            models: None,
        });
        live.turns[1] = 1;
        assert_eq!(
            no(&mut live, &mut w, 1),
            "it is mid-turn: fork it when its turn ends"
        );
        live.turns[1] = 0;
        live.slots[1] = Slot::Starting(Vec::new());
        assert_eq!(
            no(&mut live, &mut w, 1),
            "it is mid-turn: fork it when its turn ends"
        );
        live.slots[1] = Slot::None;
        live.max_workers = 1;
        assert_eq!(
            no(&mut live, &mut w, 1),
            "the team has 1 workers, the most --agents allows (1)"
        );
        live.max_workers = 2;
        w.agents[1].gone = true;
        assert_eq!(no(&mut live, &mut w, 1), only, "gone");
        w.agents[1].gone = false;
        // A worktree that is not there any more.
        std::fs::remove_dir_all(root.join("claude-1")).unwrap();
        let e = no(&mut live, &mut w, 1);
        assert!(e.starts_with("its worktree: "), "{e}");
        assert_eq!(w.agents.len(), 2, "nobody joined");
        assert!(
            !w.journal
                .iter()
                .any(|e| matches!(e.event, Event::Forked { .. }))
        );
        // A worker forks, its team at the cap but for the fork: a helper
        // is not a worker.
        git_in(&repo, &["worktree", "prune"]);
        with_helper(&mut w, 1);
        live.grow(None, false);
        let tree = worker_in(&mut live, &mut w, 1, &repo, &root);
        assert!(tree.exists());
        let new = live.fork(&mut w, 1).unwrap();
        assert_eq!(w.agents[new].name, "claude-2", "the second worker");
        let _ = std::fs::remove_dir_all(&repo);
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn a_fork_goes_on_from_its_workers_session_and_change_in_a_worktree_of_its_own() {
        let repo = checkout("fork-repo");
        let root = trees("fork");
        let dir = std::env::temp_dir().join(format!("imece-fork-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let mut w = team(1);
        w.agents[1].status = Status::Idle;
        let mut live = Live::new(
            2,
            claude_at(&appending_claude(&dir)),
            repo.clone(),
            root.clone(),
        );
        live.max_workers = 2;
        live.claude_home = trees("fork-home");
        live.models[1] = Some("haiku@low".into());
        live.tiered[1] = true;
        let tree = worker_in(&mut live, &mut w, 1, &repo, &root);
        live.sessions[1] = Some(Resumable {
            session: UUID.into(),
            spent: 700,
            models: None,
        });
        transcript_of(&live, &w, 1, UUID);
        // Its change, and the person's checkout moved on since.
        std::fs::write(tree.join("a.txt"), "a, the worker's way").unwrap();
        std::fs::write(tree.join("new.txt"), "new").unwrap();
        let head = git_in(&tree, &["rev-parse", "HEAD"]);
        std::fs::write(repo.join("b.txt"), "b").unwrap();
        commit_all(&repo);
        live.slots[1] = Slot::None;
        let new = live.fork(&mut w, 1).unwrap();
        assert_eq!(new, 2);
        let a = &w.agents[new];
        assert_eq!(
            (a.name.as_str(), a.role, a.project, a.status),
            ("claude-2", Role::Worker, 0, Status::Idle)
        );
        assert_eq!(a.start.as_deref(), Some(head.as_str()), "its worker's HEAD");
        assert_eq!((live.forks[new], live.remembered[new]), (Some(1), true));
        assert_eq!(
            (live.models[new].as_deref(), live.tiered[new]),
            (Some("haiku@low"), true)
        );
        assert_eq!(live.sessions[new], None, "none of its own yet");
        assert_eq!(
            journaled(
                &w,
                &Event::Forked {
                    from: 1,
                    agent: new
                }
            ),
            1
        );
        assert!(
            w.agents[1]
                .trace
                .back()
                .unwrap()
                .ends_with("forked into claude-2")
        );
        assert!(
            (w.agents[new].trace.back().unwrap())
                .ends_with(&format!("forked from claude-1 · worktree from {head}"))
        );
        // Its wake forks the worker's session as it is then, from the floor
        // of what that had cost, and takes its worktree as it is then: the
        // worker committed and changed more since the fork was asked for.
        live.sessions[1].as_mut().unwrap().spent = 900;
        std::fs::write(tree.join("committed.txt"), "c").unwrap();
        git_in(&tree, &["add", "committed.txt"]);
        git_in(
            &tree,
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
                "more",
            ],
        );
        let head = git_in(&tree, &["rev-parse", "HEAD"]);
        std::fs::write(tree.join("later.txt"), "later").unwrap();
        live.told(&mut w, new, "try it with a cache").unwrap();
        let a = live.resuming[new].as_ref().unwrap();
        assert_eq!(a.session, UUID);
        let fork = a.fork.clone().unwrap();
        assert!(imece_agent::valid_uuid(&fork) && fork != UUID, "{fork}");
        assert_eq!(live.floor[new], 900);
        assert_eq!(restarted(&w, new).len(), 0, "a fork, not a new session");
        // The stand-in exits: no session came, so it goes on as a new one.
        wait_for(&mut live, &mut w, |w| {
            w.agents[new].status == Status::Stopped
        });
        assert_eq!(live.forks[new], None);
        let fork_tree = root.join("claude-2");
        let args = std::fs::read_to_string(fork_tree.join("args.txt")).unwrap();
        let first: Vec<&str> = args.lines().take_while(|l| *l != "---").collect();
        let at = first.iter().position(|a| *a == "--resume").unwrap();
        assert_eq!(
            first[at..at + 5],
            [
                "--resume",
                UUID,
                "--fork-session",
                "--session-id",
                fork.as_str()
            ]
        );
        let after = |f: &str| first[first.iter().position(|a| *a == f).unwrap() + 1];
        assert_eq!(after("--name"), "imece claude-2");
        assert_eq!(after("--model"), "haiku");
        assert!(after("--append-system-prompt").starts_with("You are claude-2"));
        // Its worktree: at its worker's HEAD, with its worker's change.
        assert_eq!(git_in(&fork_tree, &["rev-parse", "HEAD"]), head);
        let read = |p: &str| std::fs::read_to_string(fork_tree.join(p)).ok();
        assert_eq!(read("a.txt").as_deref(), Some("a, the worker's way"));
        assert_eq!(read("new.txt").as_deref(), Some("new"));
        assert_eq!(read("b.txt"), None, "not the person's later commit");
        assert_eq!(read("committed.txt").as_deref(), Some("c"), "in its HEAD");
        assert_eq!(read("later.txt").as_deref(), Some("later"));
        let status = git_in(&fork_tree, &["status", "--porcelain"]);
        assert!(
            !status.contains("committed.txt"),
            "not a change of its own: {status}"
        );
        assert_eq!(
            std::fs::read_to_string(tree.join("a.txt")).unwrap(),
            "a, the worker's way",
            "the worker's is as it was"
        );
        let _ = std::fs::remove_dir_all(&dir);
        let _ = std::fs::remove_dir_all(&repo);
        let _ = std::fs::remove_dir_all(&root);
        let _ = std::fs::remove_dir_all(&live.claude_home);
    }

    #[test]
    fn a_fork_is_briefed_as_one_and_its_session_once_started_is_its_own() {
        let mut w = team(1);
        let mut live = idle_live(2);
        live.grow(None, false);
        w.agents.push(Agent::new(
            2,
            0,
            "claude-2".into(),
            Role::Worker,
            Status::Idle,
            0,
        ));
        live.forks[2] = Some(1);
        let (text, at) = live.brief(&w, 2, true);
        assert_eq!(at, None);
        assert!(
            text.starts_with(
                "You are claude-2, a fork of claude-1: this session goes on from claude-1's"
            ),
            "{text}"
        );
        assert!(
            text.contains("which started with claude-1's uncommitted change"),
            "{text}"
        );
        let (fresh, _) = live.brief(&w, 2, false);
        assert!(
            !fresh.contains("a fork of"),
            "a new session is briefed in full: {fresh}"
        );
        // Its session starts: its own from now on; no resume is journaled.
        live.resuming[2] = Some(Attempt {
            session: UUID.into(),
            fork: Some("7d1c3f2e-5b6a-4c8d-9e0f-1a2b3c4d5e6f".into()),
            text: "try".into(),
            notes: Vec::new(),
            models: None,
        });
        live.floor[2] = 900;
        hear(
            &live,
            2,
            AgentEvent::Started {
                session: "7d1c3f2e-5b6a-4c8d-9e0f-1a2b3c4d5e6f".into(),
                model: "m".into(),
            },
        );
        live.step(&mut w);
        assert_eq!(live.forks[2], None);
        assert!(live.resuming[2].is_none());
        assert_eq!(
            live.sessions[2],
            Some(Resumable {
                session: "7d1c3f2e-5b6a-4c8d-9e0f-1a2b3c4d5e6f".into(),
                spent: 900,
                models: None,
            })
        );
        assert_eq!(journaled(&w, &Event::Resumed { agent: 2 }), 0);
        // A fork whose process is gone at its first prompt (before any
        // `init`) is no fork any more: never its worker's session again.
        live.forks[2] = Some(1);
        live.resuming[2] = Some(Attempt {
            session: UUID.into(),
            fork: Some("7d1c3f2e-5b6a-4c8d-9e0f-1a2b3c4d5e6f".into()),
            text: "try".into(),
            notes: Vec::new(),
            models: None,
        });
        live.slots[2] = Slot::Starting(vec!["try".into()]);
        live.ready(
            &mut w,
            2,
            Ok((
                Box::new(Heard(Default::default(), true)),
                std::env::temp_dir(),
                None,
                None,
                imece_agent::copy::Brought::default(),
                setup::Done::None,
                None,
                Moved::Stayed,
            )),
        );
        assert_eq!(w.agents[2].status, Status::Stopped);
        assert_eq!((live.forks[2], live.resuming[2].is_none()), (None, true));
    }

    #[test]
    fn a_fork_whose_workers_session_is_gone_starts_as_a_new_session() {
        let mut w = team(1);
        let mut live = idle_live(2);
        live.claude_home = trees("fork-gone-home");
        live.grow(None, false);
        w.agents.push(Agent::new(
            2,
            0,
            "claude-2".into(),
            Role::Worker,
            Status::Idle,
            0,
        ));
        live.forks[2] = Some(1);
        // No session of its worker: no fork.
        assert_eq!(live.resumable(&w, 2), None);
        assert_eq!(live.forks[2], None);
        // One whose transcript is not where its worker runs: no fork.
        live.forks[2] = Some(1);
        live.sessions[1] = Some(Resumable {
            session: UUID.into(),
            spent: 5,
            models: None,
        });
        assert_eq!(live.resumable(&w, 2), None);
        assert_eq!(live.forks[2], None);
        // There: the worker's session, which stays the worker's.
        live.forks[2] = Some(1);
        let file = transcript_of(&live, &w, 1, UUID);
        assert_eq!(live.resumable(&w, 2), live.sessions[1].clone());
        assert_eq!(live.forks[2], Some(1));
        std::fs::remove_file(&file).unwrap();
        // Not a UUID: no fork.
        live.forks[2] = Some(1);
        live.sessions[1].as_mut().unwrap().session = "not-one".into();
        assert_eq!(live.resumable(&w, 2), None);
        // Its wake then journals why it is a new session.
        live.forks[2] = Some(1);
        live.remembered[2] = true;
        live.told(&mut w, 2, "go").unwrap();
        assert_eq!(restarted(&w, 2), [Fresh::Missing]);
        assert_eq!(
            live.sessions[1].as_ref().unwrap().session,
            "not-one",
            "the worker's stays"
        );
        let _ = std::fs::remove_dir_all(&live.claude_home);
    }

    /// The acceptance run: the leader hired new workers while one sat
    /// idle with its card done. whoami says who is free, and hire sends
    /// the leader to it on the same tier.
    #[test]
    fn a_free_worker_of_the_tier_takes_the_next_card_before_anyone_is_hired() {
        let mut w = team(1);
        let mut live = idle_live(2);
        live.max_workers = 4;
        w.agents[1].set_status(Status::Working, 0);
        assert!(live.whoami(&w, 0).ends_with("\nfree workers: none"));
        live.hire(&mut w, 0, &json!({})).unwrap();
        let free = w.agents.len() - 1;
        let name = w.agents[free].name.clone();
        assert_eq!(w.free_workers(0), [free]);
        let said = live.whoami(&w, 0);
        assert!(
            said.contains(&format!("\nfree workers: {name} (idle")),
            "{said}"
        );
        let err = live.hire(&mut w, 0, &json!({})).unwrap_err();
        assert!(err.starts_with(&format!("{name} is free")), "{err}");
        assert!(err.contains("type assign"), "{err}");
        // Another tier is another worker.
        live.hire(&mut w, 0, &json!({ "tier": "strong" })).unwrap();
        let strong = w.agents.len() - 1;
        w.agents[strong].set_status(Status::Working, 0);
        // A card done is no card at work; one at work is.
        let c = with_card(&mut w, free);
        w.cards[c].column = Column::Done;
        assert!(live.hire(&mut w, 0, &json!({})).is_err());
        w.cards[c].column = Column::Working;
        assert!(w.free_workers(0).is_empty());
        live.hire(&mut w, 0, &json!({})).unwrap();
        assert!(
            live.whoami(&w, 0).contains("free workers: claude-"),
            "the new one"
        );
        // Mid-turn, gone, or with a change waiting: not free.
        let newest = w.agents.len() - 1;
        w.agents[newest].set_status(Status::Working, 0);
        assert!(w.free_workers(0).is_empty());
        w.agents[newest].set_status(Status::Idle, 0);
        w.agents[newest].gone = true;
        assert!(w.free_workers(0).is_empty());
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
    fn whoami_says_name_role_project_who_is_over_it_the_pot_and_whether_it_may_hire() {
        let mut w = team(2);
        let helper = with_helper(&mut w, 1);
        let mut live = idle_live(4);
        (live.max_workers, live.max_helpers) = (3, 2);
        w.agents[1].cost = 2_500;
        w.paid = 1_000;
        let pot = "budget left: $0.65 of $1.00 (one pot for every agent)";
        assert_eq!(
            live.whoami(&w, 0),
            format!(
                "name: lead\nrole: the leader\nproject: repo\nyou answer to: the person \
                 (send to: you)\n{pot}\nmay hire: yes, 1 more worker (you have 2 of at most 3)\n\
                 free workers: none"
            )
        );
        assert_eq!(
            live.whoami(&w, 1),
            format!(
                "name: claude-1\nrole: a worker\nproject: repo\nyou answer to: the leader \
                 lead\n{pot}\nmay hire: yes, 1 more helper (you have 1 of at most 2)"
            )
        );
        assert_eq!(
            live.whoami(&w, helper),
            format!(
                "name: claude-1.1\nrole: a helper\nproject: repo\nyou answer to: claude-1, \
                 who hired you\n{pot}\nmay hire: no: only the leader and workers hire; ask yours \
                 with send"
            )
        );
        // As hire would answer: the cap reached, or none allowed.
        live.max_workers = 2;
        let said = live.whoami(&w, 0);
        assert!(said.ends_with("may hire: no: the team is full: 2 workers, the most the person allowed (--agents 2); give the work to one of them\nfree workers: none"), "{said}");
        live.max_helpers = 1;
        assert!(
            live.whoami(&w, 1)
                .contains("may hire: no: you have 1 helpers")
        );
        assert_eq!(live.hire_room(&w, 2), Ok((0, 1)), "the other worker's own");
        live.max_helpers = 0;
        assert!(live.whoami(&w, 2).contains("without helpers"));
        // The tests the project's files call for, last, when there are any.
        assert!(!live.whoami(&w, 2).contains("tests"));
        w.projects[0].runners = crate::runners::Runners {
            go: true,
            ..Default::default()
        };
        assert!(
            live.whoami(&w, 2)
                .ends_with("\ntests that run at once, through the run tool too: go test …"),
            "{}",
            live.whoami(&w, 2)
        );
        // Spent past the pot: nothing left, never below.
        w.paid = 20_000;
        assert!(live.whoami(&w, 2).contains("budget left: $0.00 of $1.00"));
        w.agents[0].role = Role::Researcher;
        assert!(
            live.whoami(&w, 0)
                .contains("role: a researcher\nproject: repo\nyou answer to: the leader lead")
        );
    }

    #[test]
    fn whoami_is_answered_on_the_bus_and_does_nothing() {
        let mut w = team(1);
        let mut live = idle_live(2);
        let said = call(&mut live, &mut w, 1, "whoami", json!({})).unwrap();
        assert!(
            said.starts_with("name: claude-1\nrole: a worker\n"),
            "{said}"
        );
        assert_eq!(w.agents.len(), 2);
        assert!(w.journal.is_empty());
    }

    #[test]
    fn a_solo_answers_to_the_person_and_hires_helpers() {
        let mut w = team(0);
        w.agents[0].role = Role::Solo;
        let mut live = idle_live(1);
        live.max_helpers = 2;
        let said = live.whoami(&w, 0);
        assert!(
            said.contains("role: a solo session the person drives\n"),
            "{said}"
        );
        assert!(
            said.contains("you answer to: the person (send to: you)"),
            "{said}"
        );
        assert!(
            said.ends_with("yes, 2 more helpers (you have 0 of at most 2)"),
            "{said}"
        );
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
        live.max_workers = 0;
        let err = live.hire(&mut w, 0, &json!({})).unwrap_err();
        assert!(err.contains("without workers"), "{err}");
        assert_eq!(w.agents.len(), 3);
        assert!(w.journal.is_empty());
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
            Some(Event::Hired { by: 1, agent: 3, model: Some(m), .. }) if m == "haiku"
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
        sent(&mut live, &mut w, 2, &msg("lead", "done", "fyi")).unwrap();
        assert!(!live.leaving[2]);
        let got = sent(
            &mut live,
            &mut w,
            2,
            &msg("claude-1", "done", "Tests written."),
        )
        .unwrap();
        assert!(
            got.starts_with("queued as message 2; claude-1 is busy"),
            "{got}"
        );
        assert!(live.leaving[2], "leaves when the turn ends, not now");
        assert!(!w.agents[2].gone);
        // Mail that came for it meanwhile goes nowhere.
        live.mail[2].push(Mail {
            from: 1,
            kind: MsgKind::Inform,
            body: "x".into(),
            id: 0,
            re: None,
            paths: Vec::new(),
            checks: Vec::new(),
            card: None,
            outcome: Outcome::Ok,
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
            id: 0,
            re: None,
            paths: Vec::new(),
            checks: Vec::new(),
            card: None,
            outcome: Outcome::Ok,
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
        sock.set_read_timeout(Some(Duration::from_secs(10)))
            .unwrap();
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
        assert_eq!(
            names,
            [
                "send", "post", "board", "run", "whoami", "claim", "hire", "approve"
            ]
        );
    }

    #[test]
    fn a_tier_model_that_fails_its_first_turn_gives_way_to_the_standard_one() {
        let mut w = team(0);
        let mut live = idle_live(1);
        live.claude.ladder.standard = Some("sonnet".into());
        live.hire(&mut w, 0, &json!({ "tier": "strong" })).unwrap();
        assert_eq!(live.models[1].as_deref(), Some("opus"));
        sent(&mut live, &mut w, 0, &msg("claude-1", "assign", "Fix it")).unwrap();
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
                models: None,
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
            models: None,
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
    fn the_ladder_is_claude_code_s_unless_the_person_says_otherwise() {
        let h = Hireable::new("claude", &[]);
        assert_eq!(h.ladder.model(Tier::Cheap), Some("haiku"));
        assert_eq!(h.ladder.model(Tier::Standard), None);
        assert_eq!(h.ladder.model(Tier::Strong), Some("opus"));
        assert_eq!(h.program, "claude");
        let h = Hireable::new(
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
        let h = Hireable::new("claude", &[(Tier::Standard, "opus".into())]);
        assert_eq!(h.ladder.model(Tier::Standard), Some("opus"));
    }

    #[test]
    fn a_choice_may_ask_only_for_an_effort_claude_code_has() {
        assert_eq!(efforts(), ["low", "medium", "high", "xhigh", "max"]);
        assert_eq!(valid_choice("opus@max"), Ok(()));
        assert_eq!(valid_choice("@xhigh"), Ok(()));
        assert_eq!(valid_choice("sonnet"), Ok(()));
        assert_eq!(
            valid_choice("opus@bogus"),
            Err("claude has no effort \"bogus\"; one of low, medium, high, xhigh, max".into())
        );
    }

    #[test]
    fn a_solo_hires_helpers_named_after_it_and_a_helper_hires_nobody() {
        let mut w = team(0);
        let mut live = idle_live(1);
        w.open_project(None, "notes".into(), "/n".into(), Role::Solo);
        live.grow(None, false);
        live.max_helpers = 1;
        let got = live.hire(&mut w, 1, &json!({ "tier": "cheap" })).unwrap();
        assert!(
            got.starts_with("solo-notes.1 joined (claude, haiku)"),
            "{got}"
        );
        let h = &w.agents[2];
        assert_eq!(
            (h.name.as_str(), h.role, h.parent, h.project),
            ("solo-notes.1", Role::Helper, Some(1), 1)
        );
        let e = live.hire(&mut w, 1, &json!({})).unwrap_err();
        assert!(
            e.contains("you have 1 helpers") && e.contains("--helpers 1"),
            "{e}"
        );
        let e = live.hire(&mut w, 2, &json!({})).unwrap_err();
        assert!(e.starts_with("only the leader and workers hire"), "{e}");
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
        let from = Event::Hired {
            by: 0,
            agent: 1,
            model: None,
            from: Some("#41".into()),
        };
        assert_eq!(
            event(Lang::En, &w, &from),
            "lead hired claude-1 (default model), starting from #41"
        );
        assert_eq!(
            event(Lang::Tr, &w, &from),
            "lead işe aldı: claude-1 (varsayılan model), başlangıç noktası #41"
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
        assert_eq!(
            after("--tools"),
            "Glob,Grep,Read,TaskCreate,TaskGet,TaskList,TaskUpdate,WebFetch,WebSearch"
        );
        assert_eq!(after("--allowedTools"), "WebFetch,WebSearch");
        assert!(args.contains(&"--restricted"), "standard still restricts");
        assert_eq!(after("--name"), "imece lead");
        assert!(after("--append-system-prompt").contains("hire tool, up to 3;"));
        // The prompt spans lines; the stand-in wrote one argument per line.
        let raw = std::fs::read_to_string(dir.join("args.txt")).unwrap();
        assert!(
            !raw.contains("Run cargo fmt"),
            "Claude Code reads them: {raw}"
        );
        assert_eq!(
            after("--add-dir"),
            dir.to_str().unwrap(),
            "from its own place"
        );
        let dirs = args.iter().filter(|a| **a == "--add-dir").count();
        assert_eq!(dirs, 1, "imece has nothing to add to them");
        assert!(!args.contains(&"--mcp-config"), "no bus, no MCP server");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn a_claude_agent_s_choice_splits_into_its_model_and_its_effort() {
        let dir = std::env::temp_dir().join(format!("imece-effort-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let start = |model: &str| {
            let mut w = team(0);
            let mut live = Live::new(
                1,
                claude_at(&fake_claude(&dir)),
                dir.clone(),
                dir.join("worktrees"),
            );
            live.models[0] = Some(model.into());
            live.told(&mut w, 0, "hello").unwrap();
            wait_for(&mut live, &mut w, |w| w.agents[0].status == Status::Stopped);
            let args = std::fs::read_to_string(dir.join("args.txt")).unwrap();
            let args: Vec<String> = args.lines().map(String::from).collect();
            let after = |f: &str| {
                args.iter()
                    .position(|a| a == f)
                    .map(|i| args[i + 1].clone())
            };
            (after("--model"), after("--effort"))
        };
        assert_eq!(
            start("opus@xhigh"),
            (Some("opus".into()), Some("xhigh".into()))
        );
        assert_eq!(start("@max"), (None, Some("max".into())), "an effort alone");
        assert_eq!(start("sonnet"), (Some("sonnet".into()), None));
        let _ = std::fs::remove_dir_all(&dir);
    }
    fn plan_args(cards: Value, cost: Value) -> Value {
        json!({ "cards": cards, "cost": cost })
    }

    fn two_cards() -> Value {
        json!([
            { "title": "Parser", "body": "Fix it.", "tier": "cheap", "paths": ["src/p.rs"] },
            { "title": "Docs", "body": "Write them.\nTwo lines." },
        ])
    }

    #[test]
    fn a_second_worker_waits_for_the_person_s_yes_to_a_plan() {
        let mut w = team(1);
        w.projects[0].plan.on = true;
        let mut live = idle_live(2);
        let err = live.hire(&mut w, 0, &json!({})).unwrap_err();
        assert_eq!(
            err,
            "make a plan first (the plan tool): the person reads it before a second worker \
             is hired"
        );
        assert!(
            live.whoami(&w, 0)
                .contains("may hire: no: make a plan first")
        );
        assert_eq!(w.agents.len(), 2, "nobody joined");
        let got = live
            .plan(&mut w, 0, &plan_args(two_cards(), json!(1.25)))
            .unwrap();
        assert_eq!(
            got,
            "the person reads your plan (2 cards, $1.25) in TO DECIDE; end your turn, \
             their yes starts your next"
        );
        let item = w.plan_item(0).expect("it waits");
        let it = &w.inbox[item];
        assert_eq!(
            (it.kind, it.agent, it.detail.as_str(), it.open),
            (InboxKind::Plan, 0, "Parser", true)
        );
        assert_eq!(it.card, None);
        let gate = &w.projects[0].plan;
        assert!(!gate.approved);
        let p = gate.proposal.clone().expect("kept");
        assert_eq!(p.cost, 12_500);
        assert_eq!(p.tick, w.tick);
        assert_eq!(
            p.cards,
            [
                Planned {
                    title: "Parser".into(),
                    tier: "cheap".into(),
                    paths: vec![Owned::new("src/p.rs")],
                    checks: Vec::new(),
                    body: "Fix it.".into(),
                    after: Vec::new(),
                },
                Planned {
                    title: "Docs".into(),
                    tier: "standard".into(),
                    paths: Vec::new(),
                    checks: Vec::new(),
                    body: "Write them.\nTwo lines.".into(),
                    after: Vec::new(),
                },
            ]
        );
        assert_eq!(
            journaled(
                &w,
                &Event::Planned {
                    leader: 0,
                    cards: 2,
                    cost: 12_500
                }
            ),
            1
        );
        assert!(
            w.agents[0]
                .trace
                .back()
                .unwrap()
                .ends_with(&format!("plan 2 cards $1.25 (item {item})"))
        );
        let err = live.hire(&mut w, 0, &json!({})).unwrap_err();
        assert_eq!(
            err,
            "the person is reading your plan; end your turn, their yes starts your next"
        );
        // A new plan replaces the one waiting.
        let one = json!([{ "title": "All", "body": "Everything." }]);
        live.plan(&mut w, 0, &plan_args(one, json!(0))).unwrap();
        assert!(!w.inbox[item].open, "the old one closed");
        let item = w.plan_item(0).expect("the new one waits");
        assert_eq!(w.projects[0].plan.proposal.as_ref().unwrap().cost, 0);
        // The yes: the leader hears it and may hire, this time and the next.
        assert!(live.decide_plan(&mut w, item, true));
        assert!(!live.decide_plan(&mut w, item, true), "answered already");
        assert!(!w.inbox[item].open);
        assert!(w.projects[0].plan.approved);
        assert_eq!(
            journaled(
                &w,
                &Event::PlanAnswered {
                    leader: 0,
                    yes: true
                }
            ),
            1
        );
        assert_eq!(live.notes[0], [PLAN_YES]);
        assert!(live.lost[0], "woken when free");
        assert!(w.agents[0].trace.back().unwrap().ends_with("plan yes"));
        assert_eq!(w.plan_hold(0), None);
        live.notes[0].clear();
        assert!(live.hire(&mut w, 0, &json!({})).is_ok());
        // At work on a card: no free worker to give the next one.
        let hired = w.agents.len() - 1;
        with_card(&mut w, hired);
        assert!(
            live.hire(&mut w, 0, &json!({})).is_ok(),
            "the yes holds until the next plan"
        );
        assert_eq!(w.agents.len(), 4);
        // A new plan takes the yes back; a no drops the plan.
        live.plan(&mut w, 0, &plan_args(two_cards(), json!(2)))
            .unwrap();
        assert!(!w.projects[0].plan.approved);
        assert!(
            live.hire(&mut w, 0, &json!({}))
                .unwrap_err()
                .starts_with("the person is reading")
        );
        let item = w.plan_item(0).unwrap();
        assert!(live.decide_plan(&mut w, item, false));
        assert_eq!(w.projects[0].plan.proposal, None);
        assert_eq!(live.notes[0], [PLAN_NO]);
        assert_eq!(
            journaled(
                &w,
                &Event::PlanAnswered {
                    leader: 0,
                    yes: false
                }
            ),
            1
        );
        assert!(
            live.hire(&mut w, 0, &json!({}))
                .unwrap_err()
                .starts_with("make a plan first")
        );
        // The person's word to the leader closes a plan unanswered.
        live.plan(&mut w, 0, &plan_args(two_cards(), json!(2)))
            .unwrap();
        w.said(0, "hmm");
        assert_eq!(w.plan_item(0), None);
        assert_eq!(w.plan_hold(0), Some(PlanHold::Unanswered));
        let err = live.hire(&mut w, 0, &json!({})).unwrap_err();
        assert_eq!(
            err,
            "the person answered your plan in words, not with a yes: make a new plan from \
             what they said (the plan tool)"
        );
        // The gate off: a plan is not kept, and the hire comes.
        w.projects[0].plan.on = false;
        let got = live
            .plan(&mut w, 0, &plan_args(two_cards(), json!(2)))
            .unwrap();
        assert_eq!(
            got,
            "the person turned the plan gate off for this project: hire as you see fit; \
             your plan was not kept"
        );
        assert_eq!(w.plan_item(0), None);
        for a in &mut w.agents[1..] {
            a.set_status(Status::Working, 0);
        }
        assert!(live.hire(&mut w, 0, &json!({})).is_ok());
        // Only the leader plans.
        let err = live
            .plan(&mut w, 1, &plan_args(two_cards(), json!(2)))
            .unwrap_err();
        assert_eq!(err, "only the leader plans; it hires for the team");
    }

    #[test]
    fn the_first_worker_needs_no_plan_and_a_gone_one_does_not_count() {
        let mut w = team(1);
        w.projects[0].plan.on = true;
        w.agents[1].gone = true;
        let mut live = idle_live(2);
        assert_eq!(w.plan_hold(0), None);
        assert!(live.hire(&mut w, 0, &json!({})).is_ok(), "the first worker");
        assert_eq!(w.plan_hold(0), Some(PlanHold::NoPlan), "the second waits");
        // The cap comes first: a full team is said so, plan or not.
        live.max_workers = 1;
        let err = live.hire(&mut w, 0, &json!({})).unwrap_err();
        assert!(err.starts_with("the team is full"), "{err}");
    }

    #[test]
    fn a_plan_s_card_waits_on_earlier_cards_of_it_by_their_place() {
        assert_eq!(planned_after(&Value::Null, 0), Ok(Vec::new()));
        assert_eq!(planned_after(&json!([2, 1, 2]), 2), Ok(vec![1, 2]));
        assert_eq!(planned_after(&json!([3]), 3), Ok(vec![3]));
        let why = "card 3: after names earlier cards of this plan by their place (1 to 2)";
        for bad in [json!([3]), json!([0]), json!(["1"]), json!(1), json!([-1])] {
            assert_eq!(planned_after(&bad, 2), Err(why.to_string()), "{bad}");
        }
        assert!(
            planned_after(&json!([1]), 0).is_err(),
            "the first waits on none"
        );
        let mut w = team(1);
        w.projects[0].plan.on = true;
        let mut live = idle_live(2);
        let cards = json!([
            { "title": "Parser", "body": "Fix it." },
            { "title": "Tests", "body": "Test it.", "after": [1] },
            { "title": "Docs", "body": "Say it.", "after": [2, 1] },
        ]);
        live.plan(&mut w, 0, &plan_args(cards, json!(1))).unwrap();
        let p = w.projects[0].plan.proposal.as_ref().expect("a plan");
        let after: Vec<&[usize]> = p.cards.iter().map(|c| &c.after[..]).collect();
        assert_eq!(after, [&[][..], &[1][..], &[1, 2][..]]);
        let cards = json!([
            { "title": "Parser", "body": "Fix it.", "after": [1] },
        ]);
        assert_eq!(
            live.plan(&mut w, 0, &plan_args(cards, json!(1)))
                .unwrap_err(),
            "card 1: after names earlier cards of this plan by their place (1 to 0)"
        );
    }

    #[test]
    fn a_plan_s_cards_ask_for_checks_on_lonca_only() {
        let mut w = team(1);
        w.projects[0].plan.on = true;
        let mut live = idle_live(2);
        let cards = json!([
            { "title": "Parser", "body": "Fix it." },
            { "title": "Api", "body": "Serve it.", "checks": ["e2e", "api"] },
        ]);
        assert_eq!(
            live.plan(&mut w, 0, &plan_args(cards.clone(), json!(1)))
                .unwrap_err(),
            "card 2: checks run in Lonca's merge queue, and this project is not on Lonca: \
             leave checks out, and say in the body what to run"
        );
        assert_eq!(w.projects[0].plan.proposal, None, "nothing kept");
        w.on_lonca(0, Some("demo".into()));
        live.plan(&mut w, 0, &plan_args(cards, json!(1))).unwrap();
        let p = w.projects[0].plan.proposal.as_ref().expect("a plan");
        let checks: Vec<&[String]> = p.cards.iter().map(|c| &c.checks[..]).collect();
        assert_eq!(
            checks,
            [&[][..], &["api".to_string(), "e2e".to_string()][..]]
        );
        let bad = json!([{ "title": "t", "body": "b", "checks": ["api", "api"] }]);
        assert_eq!(
            live.plan(&mut w, 0, &plan_args(bad, json!(1))).unwrap_err(),
            "card 1: checks lists \"api\" twice"
        );
        // Once Lonca says what its policy has, a name it lacks is refused.
        w.store_checks(0, "demo", Some(vec!["api".into(), "e2e".into()]));
        let typo =
            json!([{ "title": "t", "body": "b" }, { "title": "u", "body": "b", "checks": ["e2"] }]);
        assert_eq!(
            live.plan(&mut w, 0, &plan_args(typo, json!(1)))
                .unwrap_err(),
            "card 2: \"e2\" is not a check of Lonca's policy; it has api, e2e. Did you mean e2e?"
        );
    }

    #[test]
    fn bad_plans_are_refused_with_why() {
        let mut w = team(1);
        w.projects[0].plan.on = true;
        let mut live = idle_live(2);
        let refused = |live: &mut Live, w: &mut World, args: Value| {
            live.plan(w, 0, &args).expect_err("refused")
        };
        let card = |title: &str, body: &str| json!({ "title": title, "body": body });
        assert_eq!(
            refused(&mut live, &mut w, json!({ "cost": 1 })),
            "cards must be an array of cards"
        );
        assert_eq!(
            refused(&mut live, &mut w, plan_args(json!([]), json!(1))),
            "a plan has 1 to 20 cards"
        );
        let many: Vec<Value> = (0..21).map(|i| card(&format!("t{i}"), "b")).collect();
        assert_eq!(
            refused(&mut live, &mut w, plan_args(json!(many), json!(1))),
            "a plan has 1 to 20 cards"
        );
        let twenty: Vec<Value> = (0..20).map(|i| card(&format!("t{i}"), "b")).collect();
        assert!(
            live.plan(&mut w, 0, &plan_args(json!(twenty), json!(1)))
                .is_ok()
        );
        assert_eq!(
            refused(
                &mut live,
                &mut w,
                plan_args(json!([card("t", "b"), card("", "b")]), json!(1))
            ),
            "card 2 needs a title and a body"
        );
        assert_eq!(
            refused(
                &mut live,
                &mut w,
                plan_args(json!([card("t", " ")]), json!(1))
            ),
            "card 1 needs a title and a body"
        );
        assert_eq!(
            refused(
                &mut live,
                &mut w,
                plan_args(json!([card("two\nlines", "b")]), json!(1))
            ),
            "card 1's title must be one line of at most 120 characters"
        );
        let long = "x".repeat(MAX_TITLE + 1);
        assert_eq!(
            refused(
                &mut live,
                &mut w,
                plan_args(json!([card(&long, "b")]), json!(1))
            ),
            "card 1's title must be one line of at most 120 characters"
        );
        let body = "x".repeat(MAX_BODY + 1);
        assert_eq!(
            refused(
                &mut live,
                &mut w,
                plan_args(json!([card("t", &body)]), json!(1))
            ),
            "card 1's body is longer than 4000 characters"
        );
        let tier = json!([{ "title": "t", "body": "b", "tier": "huge" }]);
        assert_eq!(
            refused(&mut live, &mut w, plan_args(tier, json!(1))),
            "card 1: tier must be cheap, standard or strong"
        );
        let paths = json!([card("t", "b"), { "title": "u", "body": "b", "paths": ["/x"] }]);
        assert_eq!(
            refused(&mut live, &mut w, plan_args(paths, json!(1))),
            "card 2: paths are relative to the repository; \"/x\" starts with /"
        );
        let overlap = json!([
            { "title": "t", "body": "b", "paths": ["src"] },
            { "title": "u", "body": "b", "paths": ["docs"] },
            { "title": "v", "body": "b", "paths": ["src/a.rs"] },
        ]);
        assert_eq!(
            refused(&mut live, &mut w, plan_args(overlap, json!(1))),
            "cards 1 and 3 both own a path: \"src\" and \"src/a.rs\"; give one other paths, \
             or end one with the word shared"
        );
        let shared = json!([
            { "title": "t", "body": "b", "paths": ["src shared"] },
            { "title": "v", "body": "b", "paths": ["src/a.rs"] },
        ]);
        assert!(live.plan(&mut w, 0, &plan_args(shared, json!(1))).is_ok());
        for cost in [json!(null), json!("1"), json!(-0.5)] {
            assert_eq!(
                refused(&mut live, &mut w, plan_args(json!([card("t", "b")]), cost)),
                "cost must be a number of dollars, 0 or more"
            );
        }
        live.plan(
            &mut w,
            0,
            &plan_args(json!([card("t", "b")]), json!(0.0123)),
        )
        .unwrap();
        assert_eq!(
            w.projects[0].plan.proposal.as_ref().unwrap().cost,
            123,
            "ten-thousandths"
        );
        live.plan(&mut w, 0, &plan_args(json!([card("t", "b")]), json!(1e300)))
            .unwrap();
        assert_eq!(
            w.projects[0].plan.proposal.as_ref().unwrap().cost,
            u64::MAX,
            "saturated, not refused"
        );
        // Exactly the caps pass.
        let exact = json!([card(&"x".repeat(MAX_TITLE), &"y".repeat(MAX_BODY))]);
        assert!(live.plan(&mut w, 0, &plan_args(exact, json!(1))).is_ok());
    }
}
