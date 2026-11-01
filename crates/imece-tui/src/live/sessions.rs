//! Sessions: starting one, prompting it, ending, restarting and
//! dismissing it, and what its events do to the world (`apply`).

use super::*;

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
        AgentEvent::Text(text) => {
            let a = &mut w.agents[id];
            a.wrote(&text);
            a.trace(t, format!("text {text:?}"));
        }
        AgentEvent::Note(text) => w.agents[id].trace(t, text),
        // The account's windows, not the agent's: kept by vendor.
        AgentEvent::Limits(limits) => {
            let vendor = w.agents[id].vendor;
            for l in limits {
                w.limit(Limit {
                    vendor,
                    minutes: l.minutes,
                    used: l.used,
                    resets_at: l.resets_at,
                });
            }
        }
        AgentEvent::CommandsMissing => {
            w.agents[id].trace(
                t,
                "commands not listed: the vendor's format may have changed",
            );
            w.note(Event::CommandsMissing { agent: id });
        }
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
        // Its own to-do list, whole; the trace says how far it is and
        // what it has in hand.
        AgentEvent::Plan(steps) => {
            let a = &mut w.agents[id];
            a.plan = steps
                .into_iter()
                .map(|s| Step {
                    text: s.text,
                    state: match s.state {
                        imece_agent::StepState::Todo => StepState::Todo,
                        imece_agent::StepState::Doing => StepState::Doing,
                        imece_agent::StepState::Done => StepState::Done,
                    },
                })
                .collect();
            let (done, all) = a.plan_count();
            let doing = a
                .plan
                .iter()
                .find(|s| s.state == StepState::Doing)
                .map_or(String::new(), |s| format!(" {:?}", s.text));
            a.trace(t, format!("plan {done}/{all}{doing}"));
        }
        // How full its context is; traced the first time and then only as
        // the figure moves a tenth, so a turn of many requests does not
        // fill the trace.
        AgentEvent::Context { used, size } => {
            let a = &mut w.agents[id];
            let fill = Fill { used, size };
            let tenth = |f: Fill| f.percent().map(|p| p / 10);
            let before = a.context.replace(fill);
            if before.is_none_or(|b| tenth(b) != tenth(fill)) {
                a.trace(t, format!("context {}", words::fill_words(&fill)));
            }
        }
        // `Live::step` takes these to the person before anything is applied.
        AgentEvent::Asks { .. } | AgentEvent::Withdrawn { .. } => {}
        AgentEvent::TurnDone {
            ok,
            text,
            cost,
            denied,
        } => {
            let a = &mut w.agents[id];
            a.cost = before + cost / 100;
            a.activity = None;
            a.wrote(&text);
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
                    mine: true,
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
            a.topic = why.lines().next().unwrap_or_default().to_string();
            a.set_status(Status::Stopped, t);
            w.story(card, StoryEvent::Stopped(why.clone()));
            w.note(Event::Stopped { agent: id, why });
        }
    }
}

impl Live {
    /// A session ended: the next message starts a new one.
    pub(super) fn end(&mut self, w: &mut World, id: AgentId) {
        // What it asked waits for nobody now.
        for p in self.pending[id].drain(..) {
            if let Reply::Socket { to, .. } = p.reply {
                let _ = to.send(Err("the session ended".into()));
            }
        }
        w.close_approvals(id);
        // What it served, its to-do list and its context went with its
        // session.
        w.agents[id].local = None;
        w.agents[id].plan.clear();
        w.agents[id].context = None;
        w.agents[id].starting = false;
        self.spent_before[id] = w.agents[id].cost;
        self.slots[id] = Slot::None;
        self.cwds[id] = None;
        self.generation[id] += 1;
        self.turns[id] = 0;
        self.turned[id] = false;
        self.inflight[id].clear();
        self.floor[id] = 0;
        self.resuming[id] = None;
        self.placing[id] = None;
    }

    /// The directory agent `id`'s session runs in: the checkout for a
    /// leader and for everyone of a folder without git, its (or its
    /// worker's) worktree for the rest, as the vendor sees it (a worktree
    /// is canonical once it exists).
    pub(super) fn cwd_of(&self, w: &World, id: AgentId) -> PathBuf {
        let a = &w.agents[id];
        let place = &self.places[a.project];
        if a.role == Role::Leader || !place.git {
            return place.repo.clone();
        }
        let path = place.root.join(Self::tree_of(w, id));
        path.canonicalize().unwrap_or(path)
    }

    /// The name of the worktree agent `id` works in: a worker's own name,
    /// a helper's worker's; a solo's is `solo-<project id>`, so a renamed
    /// solo project (which renames the agent) keeps its worktree.
    pub(super) fn tree_of(w: &World, id: AgentId) -> String {
        let a = &w.agents[id];
        let owner = a.parent.map_or(a, |p| &w.agents[p]);
        if owner.role == Role::Solo {
            format!("solo-{}", owner.project)
        } else {
            owner.name.clone()
        }
    }

    /// Agent `id`'s earlier session could not be resumed (`why`): a new
    /// session, told where it left off, gets the prompts the old one was
    /// given, the first as it was before imece's notes.
    pub(super) fn restart(&mut self, w: &mut World, id: AgentId, a: Attempt, why: &str) {
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
    pub(super) fn release(&mut self, w: &mut World, id: AgentId) {
        self.dismiss(w, id);
        w.leave(id);
        w.note(Event::Left { agent: id });
    }

    /// Agent `id`'s session and runs end, and what waited on it goes: its
    /// mail, imece's notes to it, its change and the person's question.
    /// Its slot stays, for the screen's ids.
    pub(super) fn dismiss(&mut self, w: &mut World, id: AgentId) {
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

    pub(super) fn ready(&mut self, w: &mut World, id: AgentId, ready: Ready) {
        let mut waiting = match std::mem::replace(&mut self.slots[id], Slot::None) {
            Slot::Starting(q) => q,
            _ => Vec::new(),
        };
        w.agents[id].starting = false;
        let mut session = match ready {
            Ok((session, cwd, placed, held, copied)) => {
                self.copied(w, id, copied);
                // The briefing's line on the worktree, in the first prompt
                // as it is given and as it is remembered.
                if let Some((at, line)) = held {
                    for first in [waiting.first_mut(), self.inflight[id].first_mut()]
                        .into_iter()
                        .flatten()
                    {
                        if first.is_char_boundary(at) {
                            first.insert_str(at, &format!("\n{line}"));
                        }
                    }
                }
                w.agents[id].trace(w.tick, format!("session in {}", cwd.display()));
                self.cwds[id] = Some(cwd);
                self.stand(w, id);
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
    pub(super) fn prompt(
        &mut self,
        w: &mut World,
        id: AgentId,
        text: String,
        title: Option<&str>,
    ) -> Result<(), String> {
        let t = w.tick;
        self.follow(w, id);
        // Where the line on what its worktree holds goes in the prompt.
        let mut held = None;
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
            let (briefing, at) = self.brief(w, id, resumed.is_some());
            held = at.map(|at| FROM_IMECE.len() + at);
            self.notes[id].insert(0, briefing);
        }
        let text = self.with_notes(id, text);
        match &mut self.slots[id] {
            Slot::None => {
                self.start(w, id, held);
                self.slots[id] = Slot::Starting(vec![text.clone()]);
                w.agents[id].starting = true;
            }
            Slot::Starting(q) => q.push(text.clone()),
            // Git is moving its worktree: the prompt waits for that.
            Slot::Running(_) if let Some(q) = &mut self.placing[id] => q.push(text.clone()),
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

    /// What became of the listed paths at agent `id`'s start, in the
    /// journal: the ones copied in one line, each skipped one with why;
    /// the ones the worktree had say nothing.
    pub(super) fn copied(&self, w: &mut World, id: AgentId, copied: Vec<copy::Copied>) {
        let (mut paths, mut bytes) = (Vec::new(), 0);
        for c in copied {
            match c {
                copy::Copied::Copied { path, bytes: b } => {
                    paths.push(path);
                    bytes += b;
                }
                copy::Copied::Skipped { path, why } => {
                    w.note(Event::CopySkipped {
                        agent: id,
                        path,
                        why,
                    });
                }
                copy::Copied::There { .. } => {}
            }
        }
        if !paths.is_empty() {
            w.note(Event::Copied {
                agent: id,
                paths,
                bytes,
            });
        }
    }

    /// Whether `path` may be copied into project `p`'s worktrees (see
    /// [`copy::checkable`]); why not.
    pub fn may_copy(&self, p: ProjectId, path: &str) -> Result<(), String> {
        copy::checkable(&self.places[p].repo, path)
    }

    /// What imece has to tell agent `id`, ahead of `text`.
    pub(super) fn with_notes(&mut self, id: AgentId, text: String) -> String {
        let notes = std::mem::take(&mut self.notes[id]);
        if notes.is_empty() {
            return text;
        }
        let mut out = String::from(FROM_IMECE);
        out.push_str(&notes.join("\n- "));
        out.push_str("\n\n");
        out + &text
    }

    /// Starts agent `id`'s session on its own thread; `step` picks it up.
    /// With `held`, the thread also reads what its worktree holds, for
    /// the line that goes there in its first prompt.
    pub(super) fn start(&self, w: &mut World, id: AgentId, held: Option<usize>) {
        let a = &w.agents[id];
        // Everyone but a leader writes, in a worktree; in a folder without
        // git, straight in the folder.
        let leader = a.role == Role::Leader;
        let direct = leader || !self.places[a.project].git;
        let confine = self.confine;
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
        let mut system = system_prompt(w, id, self.max_workers, self.max_helpers, confine);
        let notes = repo_notes(&place.repo, a.vendor);
        if let Some(n) = &notes {
            system.push_str(&n.text);
        }
        let (program, model) = (vendor.program.clone(), self.models[id].clone());
        let name = format!("imece {}", a.name);
        let resume = self.resume_of(id);
        // In millionths of a dollar: what is left of the budget.
        let budget = w.budget.saturating_sub(w.spent()) * 100;
        let kind = a.vendor;
        let generation = self.generation[id];
        // A helper changes the same files as its worker: its worktree.
        let tree = Self::tree_of(w, id);
        let (repo, root) = (place.repo.clone(), place.root.clone());
        let copies = w.projects[a.project].copies.clone();
        let home = self.gemini_home(a.project, a.role, &tree);
        let (tx, ready) = (self.tx.clone(), self.ready_tx.clone());
        thread::spawn(move || {
            let started = (|| {
                let (cwd, placed, copied) = if direct {
                    (repo, None, Vec::new())
                } else {
                    let w = worktree::ensure(&repo, &root, &tree).map_err(|e| e.to_string())?;
                    let copied = copy::into(&repo, &w.path, &copies, COPY_BUDGET);
                    (w.path, Some(w.placed), copied)
                };
                let held = held.and_then(|at| match worktree::change(&cwd, MAX_PATCH) {
                    Ok(Some(c)) => Some((at, remember::holds(&c))),
                    _ => None,
                });
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
                                confine,
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
                                confine,
                                system,
                                mcp,
                                resume: resume.map(|r| r.session),
                            },
                            on,
                        )
                        .map_err(|e| format!("{program}: {e}"))?,
                    ),
                    _ => {
                        let Choice { model, effort } =
                            model.as_deref().map(Choice::parse).unwrap_or_default();
                        Box::new(
                            claude::Session::start(
                                &claude::Spec {
                                    program: program.clone(),
                                    cwd: cwd.clone(),
                                    name,
                                    tools: claude::tools(!leader, confine),
                                    confine,
                                    write: !leader,
                                    model,
                                    effort,
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
                Ok((session, cwd, placed, held, copied))
            })();
            // Nobody to take it (imece is closing): dropping it stops it.
            let _ = ready.send((id, generation, started));
        });
        if let Some(Notes {
            file,
            size,
            cut: true,
            ..
        }) = notes
        {
            w.note(Event::NotesCut {
                agent: id,
                file: file.to_string(),
                size,
            });
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::live::testing::*;

    #[test]
    fn a_solo_s_worktree_is_named_after_its_project_so_a_rename_keeps_it() {
        let mut w = team(1);
        let worker_helper = with_helper(&mut w, 1);
        w.open_project(
            None,
            "notes".into(),
            "/n".into(),
            Vendor::Claude,
            Role::Solo,
        );
        let solo = w.projects[1].leader;
        let solo_helper = with_helper(&mut w, solo);
        w.agents[solo_helper].project = 1;
        assert_eq!(Live::tree_of(&w, 1), "claude-1");
        assert_eq!(Live::tree_of(&w, worker_helper), "claude-1", "its worker's");
        assert_eq!(Live::tree_of(&w, solo), "solo-1");
        assert_eq!(Live::tree_of(&w, solo_helper), "solo-1", "its solo's");
        w.rename_project(1, "ideas".into());
        assert_eq!(
            Live::tree_of(&w, solo),
            "solo-1",
            "the name moved, the tree did not"
        );
        let mut live = idle_live(w.agents.len());
        live.places.push(Place {
            repo: PathBuf::from("/n"),
            root: PathBuf::from("/n-trees"),
            git: true,
        });
        assert_eq!(live.cwd_of(&w, solo), PathBuf::from("/n-trees/solo-1"));
        assert_eq!(
            live.cwd_of(&w, solo_helper),
            PathBuf::from("/n-trees/solo-1")
        );
        assert_eq!(
            live.cwd_of(&w, 0),
            live.places[0].repo,
            "a leader: the checkout"
        );
        assert!(
            live.gemini_home(1, Role::Solo, "solo-1")
                .ends_with("solo-1")
        );
    }

    #[test]
    fn what_was_copied_is_one_line_and_each_skip_another() {
        let mut w = team(1);
        let live = idle_live(2);
        live.copied(
            &mut w,
            1,
            vec![
                copy::Copied::Copied {
                    path: ".env".into(),
                    bytes: 5,
                },
                copy::Copied::There {
                    path: "target".into(),
                },
                copy::Copied::Skipped {
                    path: "dist".into(),
                    why: "not in the checkout".into(),
                },
                copy::Copied::Copied {
                    path: "node_modules".into(),
                    bytes: 7,
                },
            ],
        );
        let events: Vec<&Event> = w.journal.iter().map(|e| &e.event).collect();
        assert_eq!(
            events,
            [
                &Event::CopySkipped {
                    agent: 1,
                    path: "dist".into(),
                    why: "not in the checkout".into()
                },
                &Event::Copied {
                    agent: 1,
                    paths: vec![".env".into(), "node_modules".into()],
                    bytes: 12
                },
            ]
        );
        // What the worktree had says nothing.
        let mut w = team(1);
        let there = copy::Copied::There {
            path: ".env".into(),
        };
        live.copied(&mut w, 1, vec![there]);
        assert!(w.journal.is_empty());
    }

    #[test]
    fn a_workers_start_copies_the_listed_paths_into_its_worktree() {
        let repo = checkout("copy-start");
        std::fs::write(repo.join(".gitignore"), ".env\n").unwrap();
        std::fs::write(repo.join(".env"), "KEY=1").unwrap();
        let root = repo.with_extension("trees");
        let mut w = team(1);
        w.projects[0].copies = vec![".env".into()];
        let mut live = idle_live_in(&repo, 2);
        live.places[0].root = root.clone();
        assert_eq!(live.may_copy(0, ".env"), Ok(()));
        assert!(
            live.may_copy(0, "a.txt")
                .unwrap_err()
                .contains("does not ignore")
        );
        live.start(&mut w, 1, None);
        let copied = root.join("claude-1/.env");
        for _ in 0..500 {
            if copied.exists() {
                break;
            }
            thread::sleep(Duration::from_millis(10));
        }
        assert_eq!(std::fs::read_to_string(&copied).unwrap(), "KEY=1");
        let _ = std::fs::remove_dir_all(&repo);
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn in_a_folder_without_git_a_session_starts_in_the_folder_without_a_worktree() {
        let folder = plain_folder("start-nogit", false);
        let root = folder.with_extension("trees");
        let mut w = team(1);
        w.projects[0].copies = vec![".env".into()];
        let mut live = idle_live_in(&folder, 2);
        live.places[0].root = root.clone();
        live.places[0].git = false;
        let (tx, rx) = mpsc::channel();
        live.ready_tx = tx;
        live.start(&mut w, 1, None);
        let (id, _, started) = rx.recv_timeout(Duration::from_secs(10)).unwrap();
        // No worktree was asked of git: the vendor's program is what failed.
        let e = started.err().expect("no such program");
        assert_eq!(id, 1);
        assert!(e.starts_with("/nonexistent/claude"), "{e}");
        assert!(!root.exists(), "no worktree, nothing copied");
        let _ = std::fs::remove_dir_all(&folder);
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
        // No list at all: the trace and the journal say so, and the
        // commands it had stay.
        apply(&mut w, 1, AgentEvent::CommandsMissing, 0);
        assert!(
            (w.agents[1].trace.back().unwrap())
                .ends_with(" commands not listed: the vendor's format may have changed")
        );
        assert_eq!(
            w.journal.last().unwrap().event,
            Event::CommandsMissing { agent: 1 }
        );
        assert_eq!(w.agents[1].commands, ["/clear"]);
    }

    #[test]
    fn usage_windows_are_kept_by_the_agent_s_vendor() {
        let mut w = team(2);
        w.agents[1].vendor = Vendor::Codex;
        let window = |minutes, used| imece_agent::Limit {
            minutes,
            used,
            resets_at: Some(7),
        };
        apply(
            &mut w,
            1,
            AgentEvent::Limits(vec![window(300, 20), window(10_080, 5)]),
            0,
        );
        apply(&mut w, 2, AgentEvent::Limits(vec![window(300, 60)]), 0);
        let kept = |vendor, minutes, used| Limit {
            vendor,
            minutes,
            used,
            resets_at: Some(7),
        };
        assert_eq!(
            w.limits,
            [
                kept(Vendor::Codex, 300, 20),
                kept(Vendor::Codex, 10_080, 5),
                kept(w.agents[2].vendor, 300, 60),
            ]
        );
    }

    #[test]
    fn a_plan_is_the_agent_s_to_do_list_until_its_session_ends() {
        let mut w = team(1);
        let step = |text: &str, state| imece_agent::Step {
            text: text.into(),
            state,
        };
        let shown = |text: &str, state| Step {
            text: text.into(),
            state,
        };
        apply(
            &mut w,
            1,
            AgentEvent::Plan(vec![
                step("read", imece_agent::StepState::Done),
                step("change", imece_agent::StepState::Doing),
                step("test", imece_agent::StepState::Todo),
            ]),
            0,
        );
        let a = &w.agents[1];
        assert_eq!(
            a.plan,
            [
                shown("read", StepState::Done),
                shown("change", StepState::Doing),
                shown("test", StepState::Todo)
            ]
        );
        assert_eq!(a.plan_count(), (1, 3));
        let last = a.trace.back().unwrap();
        assert!(last.ends_with(r#" plan 1/3 "change""#), "{last}");
        // Nothing in hand: the count alone. The whole list each time.
        apply(
            &mut w,
            1,
            AgentEvent::Plan(vec![step("read", imece_agent::StepState::Done)]),
            0,
        );
        let last = w.agents[1].trace.back().unwrap();
        assert!(last.ends_with(" plan 1/1"), "{last}");
        assert_eq!(w.agents[1].plan.len(), 1);
        // A turn's end keeps it; the session's end takes it.
        apply(
            &mut w,
            1,
            AgentEvent::TurnDone {
                ok: true,
                text: "done".into(),
                cost: 0,
                denied: 0,
            },
            0,
        );
        assert_eq!(w.agents[1].plan.len(), 1);
        let mut live = idle_live(w.agents.len());
        live.end(&mut w, 1);
        assert!(w.agents[1].plan.is_empty());
    }

    #[test]
    fn the_context_s_fill_is_kept_and_traced_by_the_tenth_until_the_session_ends() {
        let mut w = team(1);
        let context = |used, size| AgentEvent::Context { used, size };
        apply(&mut w, 1, context(45_506, None), 0);
        assert_eq!(
            w.agents[1].context,
            Some(Fill {
                used: 45_506,
                size: None
            })
        );
        let last = w.agents[1].trace.back().unwrap().clone();
        assert!(last.ends_with(" context 45k tokens"), "{last}");
        // The size known: a percent; traced, a new tenth.
        apply(&mut w, 1, context(45_506, Some(200_000)), 0);
        let last = w.agents[1].trace.back().unwrap().clone();
        assert!(last.ends_with(" context 22% · 45k of 200k"), "{last}");
        // Within the tenth: kept, not traced.
        let traced = w.agents[1].trace.len();
        apply(&mut w, 1, context(58_000, Some(200_000)), 0);
        assert_eq!(w.agents[1].context.unwrap().used, 58_000);
        assert_eq!(
            w.agents[1].trace.len(),
            traced,
            "29%: the same tenth as 22%"
        );
        apply(&mut w, 1, context(60_000, Some(200_000)), 0);
        assert_eq!(w.agents[1].trace.len(), traced + 1, "30%: the next tenth");
        // A turn's end keeps it; the session's end takes it.
        apply(
            &mut w,
            1,
            AgentEvent::TurnDone {
                ok: true,
                text: "done".into(),
                cost: 0,
                denied: 0,
            },
            0,
        );
        assert!(w.agents[1].context.is_some());
        let mut live = idle_live(w.agents.len());
        live.end(&mut w, 1);
        assert_eq!(w.agents[1].context, None);
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
        let talk = a.talk.as_ref().unwrap();
        assert_eq!(
            (talk.text.as_str(), talk.with, talk.mine, talk.until),
            ("Fixed both.", Party::Person, true, w.tick + 12),
            "its own closing words, on its row"
        );
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
        assert!(matches!(
            &w.journal.last().unwrap().event,
            Event::Stopped { agent: 1, why } if why == "exited with code 1"
        ));
        assert_eq!(w.agents[1].topic, "exited with code 1", "why, for its row");
        apply(
            &mut w,
            1,
            AgentEvent::Stopped("first line\nthe rest".into()),
            0,
        );
        assert_eq!(w.agents[1].topic, "first line", "the first line only");
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
        // The message waits while the session starts on its own thread;
        // the row says so.
        assert!(matches!(&live.slots[0], Slot::Starting(q) if q == &["first".to_string()]));
        assert!(w.agents[0].starting && w.agents[0].status == Status::Working);
        live.told(&mut w, 0, "second").unwrap();
        wait_for(&mut live, &mut w, |w| w.agents[0].status == Status::Stopped);
        assert!(matches!(live.slots[0], Slot::None));
        assert!(!w.agents[0].starting, "ended: not starting any more");
        assert!(matches!(
            &w.journal.last().unwrap().event,
            Event::Stopped { agent: 0, why } if why.contains("/nonexistent/claude")
        ));
        // Talking to it again tries a new session.
        live.told(&mut w, 0, "third").unwrap();
        assert!(matches!(live.slots[0], Slot::Starting(_)));
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
        w.agents[1].starting = true;
        live.ready(
            &mut w,
            1,
            Ok((Box::new(Stub), cwd.clone(), None, None, Vec::new())),
        );
        assert!(!w.agents[1].starting, "up: not starting any more");
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
        live.ready(&mut w, 1, Ok((Box::new(Stub), cwd, None, None, Vec::new())));
        w.agents[1].cost = w.budget;
        assert_eq!(live.command(&mut w, 1, "/clear"), Err(Refusal::Budget));
    }

    #[test]
    fn a_message_lands_in_the_other_card_and_lifts_both_talk_lines() {
        let mut w = team(1);
        let c = with_card(&mut w, 1);
        let mut live = idle_live(2);
        // The leader has no card; the worker's card keeps the line.
        sent(
            &mut live,
            &mut w,
            0,
            &msg("claude-1", "inform", "first line\nmore"),
        )
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
    fn a_talk_stays_on_the_row_past_its_news_time_until_the_next_exchange() {
        let mut w = team(2);
        let mut live = idle_live(3);
        let talk = |until| Talk {
            with: Party::Person,
            mine: false,
            text: "hi".into(),
            until,
        };
        w.agents[1].talk = Some(talk(0));
        w.agents[2].talk = Some(talk(1_000_000));
        live.step(&mut w);
        assert!(w.agents[1].talk.is_some(), "stale, but still the newest");
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
        sent(&mut live, &mut w, 0, &msg("claude-1", "assign", "do it")).unwrap();
        live.step(&mut w);
        assert_eq!(live.woken[1], 1);
    }

    #[test]
    fn the_prompts_name_the_team() {
        let mut w = team(2);
        let lead = system_prompt(&w, 0, 4, 2, Confine::Strict);
        assert!(lead.starts_with("You are lead, the leader"), "{lead}");
        assert!(
            lead.contains("The workers so far are claude-1, claude-2."),
            "{lead}"
        );
        assert!(lead.contains("hire tool, up to 4;"), "{lead}");
        let worker = system_prompt(&w, 2, 4, 2, Confine::Strict);
        assert!(
            worker.starts_with("You are claude-2, a coding agent"),
            "{worker}"
        );
        assert!(worker.contains("led by lead"));
        assert!(
            worker.contains("hire helpers with the hire tool, up to 2 at once"),
            "{worker}"
        );
        let no_helpers = system_prompt(&w, 2, 4, 0, Confine::Strict);
        assert!(!no_helpers.contains("hire"), "{no_helpers}");
        let helper = with_helper(&mut w, 1);
        let helper = system_prompt(&w, helper, 4, 2, Confine::Strict);
        assert!(
            helper.starts_with("You are claude-1.1, a helper of claude-1"),
            "{helper}"
        );
        assert!(helper.contains("led by lead") && helper.contains("claude-1's git worktree"));
        assert!(!helper.contains("hire"), "{helper}");
        let alone = system_prompt(&team(0), 0, 4, 2, Confine::Strict);
        assert!(alone.contains("You have no workers yet."), "{alone}");
        let none = system_prompt(&team(0), 0, 0, 2, Confine::Strict);
        assert!(none.contains("without workers"), "{none}");
        assert!(!none.contains("hire"), "{none}");
    }

    #[test]
    fn under_open_a_claude_worker_gets_the_default_tools_and_the_leader_its_list() {
        let dir = std::env::temp_dir().join(format!("imece-open-{}", std::process::id()));
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
        live.confine = Confine::Open;
        live.told(&mut w, 0, "hello").unwrap();
        wait_for(&mut live, &mut w, |w| w.agents[0].status == Status::Stopped);
        let args = std::fs::read_to_string(repo.join("args.txt")).unwrap();
        let args: Vec<&str> = args.lines().collect();
        let after = |f: &str| args[args.iter().position(|a| *a == f).unwrap() + 1];
        // The leader never writes: its list, whatever the level.
        assert_eq!(
            after("--tools"),
            "Glob,Grep,Read,TaskCreate,TaskGet,TaskList,TaskUpdate,WebFetch,WebSearch"
        );
        assert!(!args.contains(&"--restricted"), "{args:?}");
        assert!(!args.contains(&"--disallowedTools"), "{args:?}");
        live.told(&mut w, 1, "hello").unwrap();
        wait_for(&mut live, &mut w, |w| w.agents[1].status == Status::Stopped);
        let tree = dir.join("worktrees").join("claude-1");
        let args = std::fs::read_to_string(tree.join("args.txt")).unwrap();
        let args: Vec<&str> = args.lines().collect();
        let after = |f: &str| args[args.iter().position(|a| *a == f).unwrap() + 1];
        assert_eq!(after("--tools"), "default");
        assert_eq!(after("--disallowedTools"), "Agent,Task");
        assert!(!args.contains(&"--restricted"), "{args:?}");
        assert!(!args.contains(&"--strict-mcp-config"), "{args:?}");
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
        assert_eq!(
            after("--tools"),
            "Bash,Edit,Glob,Grep,Read,TaskCreate,TaskGet,TaskList,TaskUpdate,WebFetch,WebSearch,Write"
        );
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
        assert_eq!(live.open_project(&mut w, &repo, None, Open::Team), Ok(1));
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
        assert_eq!(
            live.open_project(&mut w, &other, Some("api"), Open::Team),
            Ok(2)
        );
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
}
