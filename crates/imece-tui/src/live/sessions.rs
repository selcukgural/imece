//! Sessions: starting one, prompting it, ending, restarting and
//! dismissing it, and what its events do to the world (`apply`).

use super::*;

/// Whether the tool `name` writes a file: Claude Code's Edit, Write and
/// MultiEdit (the target is the file).
pub(super) fn writes(name: &str) -> bool {
    matches!(name, "Edit" | "Write" | "MultiEdit")
}

/// What opens a worker's card with a prompt: the card's title, the paths
/// it owns (record 114) and the checks it asks for (record 127).
pub(super) struct Opening<'a> {
    pub title: &'a str,
    pub paths: &'a [Owned],
    pub checks: &'a [String],
    /// The outside item the card closes (record 142).
    pub closes: Option<Item>,
    /// The card an assign hands again (record 126): taken instead of a
    /// new one.
    pub card: Option<CardId>,
    /// The person's own word: to a worker whose change waits, it goes on
    /// with that change's card (`World::word_goes_on`).
    pub person: bool,
    /// It opens no card by itself (record 126): the person's line, or a
    /// teammate's mail that is no assign, waits as the worker's own task
    /// until a change of it shows (`World::own_line`).
    pub lazy: bool,
}

/// Applies one event of agent `id`'s session. `before` is what its earlier
/// sessions cost, in ten-thousandths of a dollar.
pub fn apply(w: &mut World, id: AgentId, e: AgentEvent, before: u64) {
    let t = w.tick;
    let card = w.agents[id].card;
    match e {
        AgentEvent::Started { session, model } => {
            w.agents[id].trace(t, format!("session {session} · {model}"));
            // The ledger's rows say it (record 133).
            w.agents[id].meter.model = model;
        }
        AgentEvent::Commands(mut commands) => {
            // imece's own /clear ends a session whose CLI has none.
            if !commands.iter().any(|c| c == "/clear") {
                commands.push("/clear".into());
            }
            // The trace says how many, not which: the list is the
            // person's own setup (their skills and commands), noise on
            // the screen and private in a recording; the agent keeps it.
            w.agents[id].trace(t, format!("commands: {}", commands.len()));
            w.agents[id].commands = commands;
        }
        AgentEvent::Text(text) => {
            let a = &mut w.agents[id];
            a.wrote(&text);
            a.trace(t, format!("text {text:?}"));
        }
        AgentEvent::Note(text) => w.agents[id].trace(t, format!("note {text}")),
        // Counted by `Live::step`, which never brings it here.
        AgentEvent::Took => {}
        // Its `TurnDone` comes next and takes it (record 132).
        AgentEvent::Unpriced(model) => w.turn_unpriced(id, model.as_deref()),
        // Booked with what the `TurnDone` after it says (record 134).
        AgentEvent::Models { grew, .. } => w.models_grew(id, &grew),
        // The account's windows, not the agent's.
        AgentEvent::Limits(limits) => {
            for l in limits {
                w.limit(Limit {
                    minutes: l.minutes,
                    used: l.used,
                    resets_at: l.resets_at,
                });
            }
        }
        AgentEvent::CommandsMissing => {
            w.agents[id].trace(
                t,
                "commands not listed: Claude Code's format may have changed",
            );
            w.note(Event::CommandsMissing { agent: id });
        }
        AgentEvent::Tool { name, target } => {
            let mcp = imece_agent::mcp_tool("");
            let name = name.strip_prefix(&mcp).map_or(name.clone(), str::to_string);
            let changes = writes(&name);
            w.tool_called(id, &name, &target);
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
        AgentEvent::ToolFailed { name } => {
            w.agents[id].trace(t, format!("tool_result {name} ✕"));
            // Named as its call was (`Tool` above).
            let mcp = imece_agent::mcp_tool("");
            w.tool_failed(id, name.strip_prefix(&mcp).unwrap_or(&name));
        }
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
        // What the turn's end does with its failure.
        AgentEvent::Failed(f) => {
            w.agents[id].fault = Some(match f {
                imece_agent::Fault::Passing => Fault::Passing,
                imece_agent::Fault::Account => Fault::Account,
            });
        }
        AgentEvent::TurnDone {
            ok,
            text,
            cost,
            denied,
        } => {
            w.agents[id].cost = before + cost / 100;
            w.turn_priced(id);
            // On the ledger's row it was spent on (record 133), before a
            // done's change settles.
            w.book(id);
            // A done's change waited for this: what the turn cost is known.
            w.settle_change(id);
            let a = &mut w.agents[id];
            a.activity = None;
            a.watch.turn_over();
            let fault = a.fault.take();
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
                a.tries = 0;
                a.talk = Some(Talk {
                    with: Party::Person,
                    mine: true,
                    text: text.lines().next().unwrap_or_default().to_string(),
                });
                // The story keeps it; the journal gets it only when it
                // goes to the person (`Live::step` decides). The card goes
                // on: its change, or a done with none, closes it (record
                // 126), not the turn's end.
                w.story(card, StoryEvent::Replied(text));
            } else {
                w.story(card, StoryEvent::Stopped(text.clone()));
                w.turn_failed(id, &text, fault);
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

/// How much of Claude Code's words on a session it would not resume the
/// journal keeps.
const REFUSAL_KEPT: usize = 160;

/// A Claude Code's words on a session it would not resume, as the journal
/// keeps them: the first line, at most [`REFUSAL_KEPT`] characters
/// (cleaned when drawn, as all an agent's text is).
fn refusal(why: &str) -> String {
    let line = why.lines().next().unwrap_or_default();
    line.chars().take(REFUSAL_KEPT).collect()
}

impl Live {
    /// A session ended: the next message starts a new one.
    pub(super) fn end(&mut self, w: &mut World, id: AgentId) {
        // What it asked waits for nobody now.
        for p in self.pending[id].drain(..) {
            let _ = p.reply.to.send(Err("the session ended".into()));
        }
        w.close_approvals(id);
        // A yes for its turn ends with the session.
        w.turn_over(id);
        // What it served, its to-do list and its context went with its
        // session.
        w.agents[id].local = None;
        w.agents[id].watch.turn_over();
        w.agents[id].plan.clear();
        w.agents[id].context = None;
        w.agents[id].starting = false;
        // A failed turn is not tried again in a session gone.
        w.agents[id].retry = None;
        w.agents[id].tries = 0;
        w.agents[id].fault = None;
        // A turn its process had and did not end will never say what it
        // cost (record 132); one still waiting for the process to start
        // spent nothing.
        let cut = self.turns[id] > 0 && matches!(self.slots[id], Slot::Running(_));
        w.session_over(id, cut);
        self.spent_before[id] = w.agents[id].cost;
        self.slots[id] = Slot::None;
        self.cwds[id] = None;
        self.generation[id] += 1;
        self.turns[id] = 0;
        self.took[id] = 0;
        self.turned[id] = false;
        self.inflight[id].clear();
        self.floor[id] = 0;
        self.resuming[id] = None;
        // A fork whose session ended before it started (a refusal, a
        // process gone at its first prompt) goes on as a new session in
        // the worktree it has, never forking its worker again later.
        self.forks[id] = None;
        self.placing[id] = None;
    }

    /// The directory agent `id`'s session runs in: the checkout for a
    /// leader and for everyone of a folder without git, its (or its
    /// worker's) worktree for the rest, as Claude Code sees it (a worktree
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

    /// The folder agent `id` works in, as [`cwd_of`](Self::cwd_of) but
    /// not looked up on the disk (the screen draws it), and the checkout
    /// when that is a worktree of it.
    pub fn folder_of(&self, w: &World, id: AgentId) -> (PathBuf, Option<&Path>) {
        let a = &w.agents[id];
        let place = &self.places[a.project];
        if a.role == Role::Leader || !place.git {
            return (place.repo.clone(), None);
        }
        (place.root.join(Self::tree_of(w, id)), Some(&place.repo))
    }

    /// Agent `id`'s scratch directory (record 105): its own, outside the
    /// files it changes, for notes, logs and downloads that are no part
    /// of its change; `<worktrees>-scratch/<name>`; a solo's by its project's
    /// id, as its worktree, since an
    /// archived solo and the fresh one after it share a name. None for a
    /// leader, which writes nothing.
    pub fn scratch_of(&self, w: &World, id: AgentId) -> Option<PathBuf> {
        (w.agents[id].role != Role::Leader).then(|| self.own_dir(w, id, "-scratch"))
    }

    /// Agent `id`'s notes directory (record 110): imece's, its
    /// `CLAUDE.md` rewritten at every start; one per agent, so what an
    /// agent leaves there reaches no other. `<worktrees>-notes/<name>`.
    pub(super) fn notes_of(&self, w: &World, id: AgentId) -> PathBuf {
        self.own_dir(w, id, "-notes")
    }

    /// A directory of agent `id`'s beside the worktrees: `<worktrees>
    /// <kind>/<name>`, a solo's by its project's id.
    fn own_dir(&self, w: &World, id: AgentId, kind: &str) -> PathBuf {
        let a = &w.agents[id];
        let name = match a.role {
            Role::Solo => format!("solo-{}", a.project),
            _ => a.name.clone(),
        };
        let mut dir = self.places[a.project].root.clone().into_os_string();
        dir.push(kind);
        PathBuf::from(dir).join(name)
    }

    /// Agent `id`'s scratch directory goes (record 105): `:forget`,
    /// `--fresh`, an agent that leaves; not a new session that goes on
    /// with the same work (a fallback, a renewal, `/clear`). Moved aside at
    /// once, so a session started right after makes a new one, and removed
    /// on a thread of its own; the trace says so when there was one.
    pub(super) fn drop_scratch(&self, w: &World, id: AgentId) {
        // Its notes go too: a copy of the person's file, nothing of its own.
        let _ = std::fs::remove_dir_all(self.notes_of(w, id));
        let Some(dir) = self.scratch_of(w, id) else {
            return;
        };
        let mut aside = dir.clone().into_os_string();
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_or(0, |d| d.as_nanos());
        aside.push(format!(".gone-{nanos:x}"));
        let aside = PathBuf::from(aside);
        let note = match std::fs::rename(&dir, &aside) {
            Ok(()) => None,
            Err(e) if e.kind() == io::ErrorKind::NotFound => return,
            Err(e) => Some(format!("could not remove its scratch directory: {e}")),
        };
        let (tx, generation) = (self.tx.clone(), self.generation[id]);
        thread::spawn(move || {
            let note = note.unwrap_or_else(|| match std::fs::remove_dir_all(&aside) {
                Ok(()) => "its scratch directory was removed".into(),
                Err(e) => format!("could not remove its scratch directory: {e}"),
            });
            let _ = tx.send((id, generation, AgentEvent::Note(note)));
        });
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
        // The session asked for never started: nothing it was given ran.
        self.turns[id] = 0;
        self.end(w, id);
        self.sessions[id] = None;
        self.notes[id] = a.notes;
        self.remembered[id] = true;
        self.refused[id] = Some(refusal(why));
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

    /// Helper `id` leaves: its session ends, its senders hear of the mail
    /// it never read (record 128), and it is gone from the screen; its
    /// name may be taken again.
    pub(super) fn release(&mut self, w: &mut World, id: AgentId) {
        self.unread(w, id);
        self.dismiss(w, id);
        w.leave(id);
        w.note(Event::Left { agent: id });
    }

    /// Agent `id` leaves its Lonca workspace: its session ends on a thread
    /// of its own, its leases with it, its proposals staying. A helper
    /// leaves its worker's session as it was. The trace says how it went.
    fn stand_up(&self, w: &World, id: AgentId) {
        let a = &w.agents[id];
        if a.role == Role::Helper || a.workspace.is_none() {
            return;
        }
        let (place, at) = (self.lonca.place().clone(), self.cwd_of(w, id));
        let (tx, generation) = (self.tx.clone(), self.generation[id]);
        thread::spawn(move || {
            let note = match place.stand_up(&at) {
                Ok(()) => "its Lonca session ended".to_string(),
                Err(_) => "its Lonca session had ended, or could not be told".to_string(),
            };
            let _ = tx.send((id, generation, AgentEvent::Note(note)));
        });
    }

    /// Agent `id`'s session and runs end, and what waited on it goes: its
    /// mail, imece's notes to it, its change and the person's question.
    /// Its slot stays, for the screen's ids.
    pub(super) fn dismiss(&mut self, w: &mut World, id: AgentId) {
        self.stand_up(w, id);
        self.running[id].kill_all();
        if let Slot::Running(s) = &mut self.slots[id] {
            s.terminate();
        }
        self.end(w, id);
        self.forget(w, id, true);
        self.drop_scratch(w, id);
        self.leaving[id] = false;
        self.lost[id] = false;
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
            Ok((session, cwd, placed, held, copied, set_up, workspace, moved)) => {
                self.copied(w, id, copied);
                self.moved(w, id, moved);
                if let Some(repo) = &workspace {
                    w.agents[id].trace(w.tick, format!("on Lonca: a workspace of {repo}"));
                }
                w.agents[id].workspace = workspace;
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
                w.agents[id].trace(w.tick, format!("session in {}", home_tilde(&cwd)));
                self.cwds[id] = Some(cwd);
                self.setup_done(w, id, set_up);
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
                    Some(worktree::Placed::From { at }) => {
                        w.agents[id].trace(w.tick, format!("worktree made at {at}"));
                    }
                    Some(
                        worktree::Placed::Fresh | worktree::Placed::AtHead | worktree::Placed::Own,
                    )
                    | None => {}
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
    /// if it has none. An idle worker gets no card for it: the line waits
    /// as its own task until a change of it shows (record 126). Mail that
    /// waited for this word goes with it (record 107).
    pub fn told(&mut self, w: &mut World, id: AgentId, text: &str) -> Result<(), Refusal> {
        if self.over_budget(w) {
            return Err(Refusal::Budget);
        }
        // The person is here again: agents may wake each other anew.
        self.woken.iter_mut().for_each(|n| *n = 0);
        self.posted.iter_mut().for_each(|n| *n = 0);
        let title = text.lines().next().unwrap_or_default().to_string();
        let mail = if self.turns[id] == 0 && w.word_frees(id) {
            std::mem::take(&mut self.mail[id])
        } else {
            Vec::new()
        };
        let prompt = if mail.is_empty() {
            text.to_string()
        } else {
            format!("{text}\n\n{}", mail_prompt(w, &mail))
        };
        let opening = Opening {
            title: &title,
            paths: &[],
            checks: &[],
            closes: None,
            card: None,
            person: true,
            lazy: true,
        };
        if let Err(why) = self.prompt(w, id, prompt, Some(opening)) {
            self.mail[id] = mail;
            return Err(Refusal::Start(why));
        }
        self.asked[id] = true;
        Ok(())
    }

    /// Gives agent `id` one line of Claude Code's own command line (`/clear`,
    /// `/compact`, `/cost` ...), typed by the person. It runs as a turn and
    /// its answer reaches the person. A session without `/clear` of its own
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
                self.forget(w, id, true);
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
    /// none. An idle worker gets a new card from `opening`, if there is one
    /// and it is not the person's own line (`World::own_line`).
    pub(super) fn prompt(
        &mut self,
        w: &mut World,
        id: AgentId,
        text: String,
        opening: Option<Opening<'_>>,
    ) -> Result<(), String> {
        let t = w.tick;
        self.onto_lonca(w, id);
        self.renew_seat(w, id, Instant::now());
        // Whatever goes in now is the next try: no retry waits beside it.
        w.agents[id].retry = None;
        self.follow(w, id);
        // Where the line on what its worktree holds goes in the prompt.
        let mut held = None;
        if std::mem::take(&mut self.remembered[id]) {
            let had = self.sessions[id].is_some() || self.forks[id].is_some();
            let mut resumed = self.resumable(w, id);
            // A fork's new session gets an id of imece's choosing, checked
            // when it starts; without one there is no fork.
            let fork = match self.forks[id] {
                Some(_) => imece_agent::new_uuid().ok(),
                None => None,
            };
            if self.forks[id].is_some() && fork.is_none() {
                self.forks[id] = None;
                resumed = None;
            }
            if let Some(r) = &resumed {
                // A fork reports what its worker's session had cost as
                // well (record 94): the floor as for a resume.
                self.floor[id] = r.spent;
                self.resuming[id] = Some(Attempt {
                    session: r.session.clone(),
                    fork,
                    text: text.clone(),
                    notes: self.notes[id].clone(),
                    models: r.models.clone(),
                });
            } else {
                let why = match self.refused[id].take() {
                    Some(said) => Fresh::Refused(said),
                    None if had => Fresh::Missing,
                    None => Fresh::NoSession,
                };
                w.note(Event::Restarted { agent: id, why });
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
                // Its first calls may come before the next step.
                self.own_sockets();
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
        // The person's word to a worker whose change waits goes on with
        // that change's card, back at work (record 126); the briefing
        // above said where it stood.
        if opening.as_ref().is_some_and(|o| o.person) {
            w.word_goes_on(id);
        }
        let a = &w.agents[id];
        // A worker between tasks gets a card for this one; a task it has
        // not finished (its session stopped or slept under it, or it
        // ended a turn without a done) goes on. A card handed again is
        // taken at the attempt it is on (record 126). The person's own
        // line opens none: it waits as the worker's own task until a
        // change of it shows (`World::own_line`).
        let unfinished = a.card.is_some_and(|c| {
            let c = &w.cards[c];
            c.column == Column::Working && c.owner == Some(id)
        });
        let handed = opening.as_ref().and_then(|o| o.card);
        if let Some(c) = handed
            && a.role == Role::Worker
        {
            w.takes(id, c);
        } else if let Some(o) = opening.as_ref().filter(|o| o.lazy)
            && a.role == Role::Worker
            && a.status != Status::Working
            && !unfinished
        {
            w.own_line(id, o.title);
        } else if let Some(o) = opening
            && a.role == Role::Worker
            && a.status != Status::Working
            && !unfinished
        {
            let card = w.cards.len();
            w.cards.push(Card {
                id: card,
                project: a.project,
                title: o.title.to_string(),
                owner: Some(id),
                column: Column::Working,
                story: Vec::new(),
                symbols: Vec::new(),
                paths: o.paths.to_vec(),
                checks: o.checks.to_vec(),
                closes: o.closes,
                started: t,
                finished: 0,
                attempt: 1,
                breaker: Default::default(),
                after: Vec::new(),
            });
            w.takes(id, card);
            w.story(Some(card), StoryEvent::Took { agent: id });
        }
        let a = &mut w.agents[id];
        a.set_status(Status::Working, t);
        if a.activity.is_none() {
            a.activity = Some(String::new());
        }
        w.round_opens(id);
        Ok(())
    }

    /// Worker `id` wrote `target`: when its card owns paths and none names
    /// the file, the first time for that file, the card's story and the
    /// journal say so and its next prompt tells it (record 114); never
    /// stopped. Whether it strayed.
    pub(super) fn strayed(&mut self, w: &mut World, id: AgentId, target: &str) -> bool {
        let Some(card) = w.strayed(id, target) else {
            return false;
        };
        let leader = &w.agents[w.projects[w.agents[id].project].leader].name;
        self.notes[id].push(format!(
            "you changed {target}, which #{} does not own (it owns {}); say so in your \
             done, or ask {leader} to widen the card",
            card + 1,
            paths::listed(&w.cards[card].paths)
        ));
        true
    }

    /// What became of the listed paths at agent `id`'s start, and of
    /// those the checkout's `.worktreeinclude` names, in the journal: what
    /// of the file was not followed, each skipped path with why, the ones
    /// copied in one line for the list and one for the file; the ones the
    /// worktree had say nothing.
    pub(super) fn copied(&self, w: &mut World, id: AgentId, brought: copy::Brought) {
        for r in brought.refused {
            w.note(Event::IncludeRefused {
                agent: id,
                line: r.line,
                why: r.why,
            });
        }
        // The list's, then the file's.
        let mut copied = [(Vec::new(), 0), (Vec::new(), 0)];
        for (i, c) in brought.copied.into_iter().enumerate() {
            let included = i >= brought.listed;
            match c {
                copy::Copied::Copied { path, bytes } => {
                    let (paths, all) = &mut copied[usize::from(included)];
                    paths.push(path);
                    *all += bytes;
                }
                copy::Copied::Skipped { path, why } => {
                    w.note(Event::CopySkipped {
                        agent: id,
                        path,
                        why,
                        included,
                    });
                }
                copy::Copied::There { .. } => {}
            }
        }
        for (included, (paths, bytes)) in [false, true].into_iter().zip(copied) {
            if !paths.is_empty() {
                w.note(Event::Copied {
                    agent: id,
                    paths,
                    bytes,
                    included,
                });
            }
        }
    }

    /// Whether project `p`'s checkout has a `.worktreeinclude`.
    pub fn includes(&self, p: ProjectId) -> bool {
        copy::has_include(&self.places[p].repo)
    }

    /// Whether `path` may be copied into project `p`'s worktrees (see
    /// [`copy::checkable`]); why not.
    pub fn may_copy(&self, p: ProjectId, path: &str) -> Result<(), String> {
        copy::checkable(&self.places[p].repo, path)
    }

    /// Helper `id`'s session stopped, or its last turn failed for good,
    /// before it sent done (`why`, Claude Code's or imece's words): its
    /// worker is told, at its next turn, that the part is not done, and
    /// an idle worker is woken for it (`lost`), so it does not wait on a
    /// helper that will not answer. Anyone else's end tells nobody.
    pub(super) fn helper_lost(&mut self, w: &mut World, id: AgentId, why: &str) {
        let a = &w.agents[id];
        let (Role::Helper, Some(p)) = (a.role, a.parent) else {
            return;
        };
        if w.agents[p].gone || self.leaving[id] {
            return;
        }
        let said: String = why
            .lines()
            .map(str::trim)
            .find(|l| !l.is_empty())
            .unwrap_or_default()
            .chars()
            .take(LOST_SAID)
            .collect();
        let name = a.name.clone();
        self.notes[p].push(format!(
            "Your helper {name} stopped before it sent you done ({said}). The part you gave \
             it is not done: do it yourself, or hire another helper for it."
        ));
        self.lost[p] = true;
        w.agents[p].trace(w.tick, format!("told: {name} stopped"));
    }

    /// What imece has to tell agent `id`, ahead of `text`.
    pub(super) fn with_notes(&mut self, id: AgentId, text: String) -> String {
        self.lost[id] = false;
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
        let mcp = self.bus.as_ref().map(|b| Mcp {
            command: b.relay.clone(),
            args: vec![
                "mcp-relay".into(),
                b.servers[id].path().to_string_lossy().into_owned(),
            ],
            tools: tools_of(a.role, &self.run_programs, self.run_timeout)
                .into_iter()
                .map(|t| t.name)
                .collect(),
        });
        let place = &self.places[a.project];
        let mut system = system_prompt(w, id, self.max_workers, self.max_helpers, confine);
        let (program, model) = (self.claude.program.clone(), self.models[id].clone());
        let name = format!("imece {}", a.name);
        let resume = self.resume_of(id);
        let fork = self.resuming[id].as_ref().and_then(|r| r.fork.clone());
        // A fork's worktree, made now, starts with its worker's change.
        let carry = self.forks[id].map(|from| (w.agents[from].name.clone(), self.cwd_of(w, from)));
        // In millionths of a dollar: what is left of the budget.
        let budget = w.budget.saturating_sub(w.spent()) * 100;
        let generation = self.generation[id];
        // A helper changes the same files as its worker: its worktree.
        let tree = Self::tree_of(w, id);
        // A helper's tree is its worker's, started where the worker's was.
        let start = w.agents[a.parent.unwrap_or(id)].start.clone();
        let (repo, root) = (place.repo.clone(), place.root.clone());
        let copies = w.projects[a.project].copies.clone();
        let scratch = self.scratch_of(w, id);
        let store = (w.projects[a.project].lonca.as_ref()).map(|on| on.repo.clone());
        // A helper's session is its worker's: a session goes by the owner.
        let owner = &w.agents[a.parent.unwrap_or(id)].name;
        let (place_on, agent) = (self.lonca.place().clone(), owner.clone());
        // A session whose hooks Lonca did not hear is never sat in again
        // (lonca#190).
        let renew = self.seat_short(w, id, Instant::now()) || self.unheard(w, id).is_some();
        // A worker hired before its project went on Lonca leaves its
        // worktree for a workspace when the worktree holds nothing.
        let onto = store.is_some() && self.may_move(w, id);
        let notes = self.notes_of(w, id);
        let (own, instructions) = (self.own_notes.clone(), self.instructions.clone());
        let checkout = place.repo.clone();
        let plan = if direct {
            setup::Plan::None
        } else {
            self.setup_plan(w, id)
        };
        let running = self.running[id].clone();
        let removing = self.tidy.removing.clone();
        let (tx, ready) = (self.tx.clone(), self.ready_tx.clone());
        thread::spawn(move || {
            let started = (|| {
                let mut moved = Moved::Stayed;
                let scratch = match scratch {
                    Some(dir) => {
                        let dir = make_private(&dir).map_err(|e| {
                            format!("its scratch directory {}: {e}", home_tilde(&dir))
                        })?;
                        system.push_str(&scratch_note(&dir));
                        Some(dir)
                    }
                    None => None,
                };
                let at = root.join(&tree);
                let seated = if direct {
                    None
                } else {
                    // A worktree `:tidy` removes is made anew once it is
                    // gone, never reused while it goes.
                    removing.wait(&at);
                    if onto {
                        moved = lonca::move_off(&at)?;
                    }
                    let (asked, own) = (store.as_deref(), scratch.as_deref());
                    lonca::seat(&place_on, asked, &at, &agent, own, renew)?
                };
                let (cwd, placed, copied, set_up) = if direct {
                    (repo, None, copy::Brought::default(), setup::Done::None)
                } else if let Some(made) = seated {
                    // Lonca's workspace in place of a worktree: the copies
                    // and the setup as ever, git having no part in them.
                    let at = (at.canonicalize())
                        .map_err(|e| format!("its workspace {}: {e}", home_tilde(&at)))?;
                    let copied = copy::bring(&repo, &at, &copies, COPY_BUDGET);
                    let set_up = plan.carry_out(made, &repo, &at, &running);
                    (at, None, copied, set_up)
                } else {
                    // A fork's is made where its worker's HEAD is now, the
                    // change it takes being of now too.
                    let start = match &carry {
                        Some((from, path)) => Some(
                            worktree::resolve(path, "HEAD")
                                .map_err(|e| format!("{from}'s worktree: {e}"))?,
                        ),
                        None => start,
                    };
                    let w = worktree::ensure(&repo, &root, &tree, start.as_deref())
                        .map_err(|e| e.to_string())?;
                    let copied = copy::bring(&repo, &w.path, &copies, COPY_BUDGET);
                    // A worktree made just now is set up before the
                    // session starts in it, after its copies.
                    let made = matches!(
                        w.placed,
                        worktree::Placed::Fresh | worktree::Placed::From { .. }
                    );
                    if let (true, Some((from, path))) = (made, &carry) {
                        worktree::carry(path, &w.path)
                            .map_err(|e| format!("could not carry {from}'s change: {e}"))?;
                    }
                    let set_up = plan.carry_out(made, &repo, &w.path, &running);
                    (w.path, Some(w.placed), copied, set_up)
                };
                let held = held
                    .filter(|_| seated.is_none())
                    .and_then(|at| match worktree::change(&cwd, MAX_PATCH) {
                        Ok(Some(c)) => Some((at, remember::holds(&c))),
                        _ => None,
                    });
                let workspace = store.filter(|_| seated.is_some());
                if let Some(repo) = &workspace {
                    system.push_str(&lonca_note(repo));
                }
                let text = notes_text(
                    own.as_deref(),
                    instructions.as_deref(),
                    &cwd,
                    &checkout,
                    confine,
                );
                let notes = write_notes(&notes, text.as_deref())
                    .map_err(|e| format!("its notes directory {}: {e}", home_tilde(&notes)))?;
                let on = move |e| {
                    let _ = tx.send((id, generation, e));
                };
                let Choice { model, effort } =
                    model.as_deref().map(Choice::parse).unwrap_or_default();
                let session: Box<dyn Session> = Box::new(
                    claude::Session::start(
                        &claude::Spec {
                            scratch: scratch.clone(),
                            notes,
                            store: workspace.as_ref().map(|_| claude::Store {
                                program: place_on.program.clone(),
                                socket: place_on.socket.clone(),
                            }),
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
                                fork,
                                models: r.models,
                            }),
                        },
                        on,
                    )
                    .map_err(|e| format!("{program}: {e}"))?,
                );
                Ok((session, cwd, placed, held, copied, set_up, workspace, moved))
            })();
            // Nobody to take it (imece is closing): dropping it stops it.
            let _ = ready.send((id, generation, started));
        });
    }
}

/// Puts `text` in the notes directory `dir` (record 110), rewritten at
/// every start, the file private (0600): its canonical path, none and
/// nothing made when there is no text. A writer can reach the directory,
/// so what is there goes first and the file is made new, never written
/// through a link left in its place.
pub(super) fn write_notes(dir: &Path, text: Option<&str>) -> io::Result<Option<PathBuf>> {
    use std::io::Write;
    use std::os::unix::fs::OpenOptionsExt;
    let Some(text) = text else {
        return Ok(None);
    };
    let dir = make_private(dir)?;
    let file = dir.join(NOTES_FILE);
    // Whatever cannot go is refused below: `create_new` takes no file there.
    let _ = std::fs::remove_file(&file);
    (std::fs::OpenOptions::new().write(true).create_new(true))
        .mode(0o600)
        .open(&file)?
        .write_all(text.as_bytes())?;
    Ok(Some(dir))
}

/// Makes an agent's own directory (a scratch one, record 105; its notes,
/// record 110) when it is not there, private (0700), its parents too; one
/// that is there must be a directory, not a link to one. Its canonical
/// path, the one Claude Code is given.
pub(super) fn make_private(dir: &Path) -> io::Result<PathBuf> {
    use std::os::unix::fs::DirBuilderExt;
    if let Some(parent) = dir.parent() {
        std::fs::create_dir_all(parent)?;
    }
    match std::fs::DirBuilder::new().mode(0o700).create(dir) {
        Ok(()) => {}
        Err(e) if e.kind() == io::ErrorKind::AlreadyExists => {
            if !std::fs::symlink_metadata(dir)?.is_dir() {
                return Err(io::Error::other("it is there and is not a directory"));
            }
        }
        Err(e) => return Err(e),
    }
    dir.canonicalize()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::live::testing::*;

    #[test]
    fn a_vendor_s_refusal_is_kept_as_its_first_line_cut() {
        assert_eq!(refusal("no rollout\nstack trace"), "no rollout");
        let long = "x".repeat(REFUSAL_KEPT + 40);
        assert_eq!(refusal(&long).chars().count(), REFUSAL_KEPT);
        assert_eq!(refusal("é".repeat(161).as_str()), "é".repeat(160));
        assert_eq!(refusal(""), "");
    }

    #[test]
    fn an_old_journal_line_of_a_new_session_reads_as_having_had_none() {
        let old: Event = serde_json::from_str(r#"{"Restarted":{"agent":1}}"#).unwrap();
        assert_eq!(
            old,
            Event::Restarted {
                agent: 1,
                why: Fresh::NoSession
            }
        );
        let e = Event::Restarted {
            agent: 1,
            why: Fresh::Refused("gone".into()),
        };
        let back: Event = serde_json::from_str(&serde_json::to_string(&e).unwrap()).unwrap();
        assert_eq!(back, e);
    }

    #[test]
    fn a_solo_s_worktree_is_named_after_its_project_so_a_rename_keeps_it() {
        let mut w = team(1);
        let worker_helper = with_helper(&mut w, 1);
        w.open_project(None, "notes".into(), "/n".into(), Role::Solo);
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
    }

    #[test]
    fn what_was_copied_is_one_line_and_each_skip_another() {
        let mut w = team(1);
        let live = idle_live(2);
        assert!(!live.includes(0), "no .worktreeinclude in the temp dir");
        let copied = |path: &str, bytes| copy::Copied::Copied {
            path: path.into(),
            bytes,
        };
        let skipped = |path: &str| copy::Copied::Skipped {
            path: path.into(),
            why: "not in the checkout".into(),
        };
        let there = |path: &str| copy::Copied::There { path: path.into() };
        live.copied(
            &mut w,
            1,
            copy::Brought {
                copied: vec![
                    copied(".env", 5),
                    there("target"),
                    skipped("dist"),
                    copied("node_modules", 7),
                    // From here what only the file names.
                    copied("vendor", 11),
                    skipped("cache"),
                    there("build"),
                    copied(".env.local", 13),
                ],
                listed: 4,
                refused: vec![
                    copy::Refused {
                        line: Some((3, "../x".into())),
                        why: "reaches outside the checkout".into(),
                    },
                    copy::Refused {
                        line: None,
                        why: "matches more than 100 paths".into(),
                    },
                ],
            },
        );
        let events: Vec<&Event> = w.journal.iter().map(|e| &e.event).collect();
        let skip = |path: &str, included| Event::CopySkipped {
            agent: 1,
            path: path.into(),
            why: "not in the checkout".into(),
            included,
        };
        assert_eq!(
            events,
            [
                &Event::IncludeRefused {
                    agent: 1,
                    line: Some((3, "../x".into())),
                    why: "reaches outside the checkout".into()
                },
                &Event::IncludeRefused {
                    agent: 1,
                    line: None,
                    why: "matches more than 100 paths".into()
                },
                &skip("dist", false),
                &skip("cache", true),
                &Event::Copied {
                    agent: 1,
                    paths: vec![".env".into(), "node_modules".into()],
                    bytes: 12,
                    included: false
                },
                &Event::Copied {
                    agent: 1,
                    paths: vec!["vendor".into(), ".env.local".into()],
                    bytes: 24,
                    included: true
                },
            ]
        );
        // What the worktree had says nothing; only the file's: one line.
        let mut w = team(1);
        let brought = copy::Brought {
            copied: vec![there(".env"), copied("vendor", 2)],
            listed: 1,
            refused: Vec::new(),
        };
        live.copied(&mut w, 1, brought);
        let events: Vec<&Event> = w.journal.iter().map(|e| &e.event).collect();
        assert_eq!(
            events,
            [&Event::Copied {
                agent: 1,
                paths: vec!["vendor".into()],
                bytes: 2,
                included: true
            }]
        );
    }

    #[test]
    fn a_workers_start_copies_the_listed_paths_into_its_worktree() {
        let repo = checkout("copy-start");
        std::fs::write(repo.join(".gitignore"), ".env\n.env.local\n").unwrap();
        std::fs::write(repo.join(".env"), "KEY=1").unwrap();
        // And what the checkout's own file names.
        std::fs::write(repo.join(".env.local"), "LOCAL=1").unwrap();
        std::fs::write(repo.join(".worktreeinclude"), ".env.local\n").unwrap();
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
        // Until it is written, not only made: the copy runs on a thread.
        for _ in 0..500 {
            if std::fs::read_to_string(&copied).is_ok_and(|s| s == "KEY=1") {
                break;
            }
            thread::sleep(Duration::from_millis(10));
        }
        assert_eq!(std::fs::read_to_string(&copied).unwrap(), "KEY=1");
        let local = root.join("claude-1/.env.local");
        for _ in 0..500 {
            if std::fs::read_to_string(&local).is_ok_and(|s| s == "LOCAL=1") {
                break;
            }
            thread::sleep(Duration::from_millis(10));
        }
        assert_eq!(std::fs::read_to_string(&local).unwrap(), "LOCAL=1");
        assert!(live.includes(0));
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
        // No worktree was asked of git: Claude Code's program is what failed.
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
                .any(|l| l.ends_with(" commands: 3")),
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
        // The trace counts them and names none: the person's own setup.
        assert!((w.agents[1].trace.back().unwrap()).ends_with(" commands: 2"));
        assert!(
            !(w.agents[1].trace.iter()).any(|l| l.contains("/compact") || l.contains("/stats")),
            "{:?}",
            w.agents[1].trace
        );
        apply(&mut w, 1, AgentEvent::Commands(vec![]), 0);
        assert_eq!(w.agents[1].commands, ["/clear"]);
        // No list at all: the trace and the journal say so, and the
        // commands it had stay.
        apply(&mut w, 1, AgentEvent::CommandsMissing, 0);
        assert!(
            (w.agents[1].trace.back().unwrap())
                .ends_with(" commands not listed: Claude Code's format may have changed")
        );
        assert_eq!(
            w.journal.last().unwrap().event,
            Event::CommandsMissing { agent: 1 }
        );
        assert_eq!(w.agents[1].commands, ["/clear"]);
    }

    #[test]
    fn usage_windows_are_kept_by_their_length() {
        let mut w = team(2);
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
        let kept = |minutes, used| Limit {
            minutes,
            used,
            resets_at: Some(7),
        };
        assert_eq!(w.limits, [kept(300, 60), kept(10_080, 5),]);
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
    fn a_turn_s_cost_goes_on_the_ledger_under_the_model_its_session_said() {
        let mut w = team(1);
        let c = crate::live::testing::with_card(&mut w, 1);
        w.takes(1, c);
        apply(
            &mut w,
            1,
            AgentEvent::Started {
                session: "s1".into(),
                model: "claude-opus-5-5".into(),
            },
            0,
        );
        assert_eq!(w.agents[1].meter.model, "claude-opus-5-5");
        let done = |cost| AgentEvent::TurnDone {
            ok: true,
            text: "done".into(),
            cost,
            denied: 0,
        };
        // Millionths of a dollar on top of what was spent before.
        apply(&mut w, 1, done(1_200_000), 300);
        let r = &w.ledger[0];
        assert_eq!((r.card, r.cost), (Some(c), 12_300));
        assert_eq!(r.model, "claude-opus-5-5");
        // Unpriced: the row says so.
        apply(&mut w, 1, AgentEvent::Unpriced(None), 0);
        apply(&mut w, 1, done(1_200_000), 300);
        assert_eq!(w.ledger[0].cost, 12_300);
        assert!(w.ledger[0].unpriced);
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
    fn a_finished_turn_answers_the_person_and_leaves_the_card_open() {
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
            (talk.text.as_str(), talk.with, talk.mine),
            ("Fixed both.", Party::Person, true),
            "its own closing words, on its row"
        );
        let touched = w.cards[c]
            .story
            .iter()
            .filter(|s| matches!(s.event, StoryEvent::Touched(_)))
            .count();
        assert_eq!(touched, 2, "a file is listed once per card");
        assert_eq!(
            w.cards[c].column,
            Column::Working,
            "a turn's end closes no card: its change does (record 126)"
        );
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

    /// A team of one worker with one helper, the worker's session
    /// running on `Heard`; what the worker was prompted with.
    fn helped() -> (World, Live, AgentId, Arc<std::sync::Mutex<Vec<String>>>) {
        let mut w = team(1);
        let helper = with_helper(&mut w, 1);
        let mut live = idle_live(3);
        // No git: the worktree is not moved between turns.
        live.places[0].git = false;
        let heard = Arc::new(std::sync::Mutex::new(Vec::new()));
        let session = Box::new(Heard(heard.clone(), false));
        live.ready(
            &mut w,
            1,
            Ok((
                session,
                std::env::temp_dir(),
                None,
                None,
                imece_agent::copy::Brought::default(),
                setup::Done::None,
                None,
                Moved::Stayed,
            )),
        );
        w.agents[1].status = Status::Idle;
        (w, live, helper, heard)
    }

    #[test]
    fn a_prompt_opens_the_round_a_change_is_costed_by_and_the_turn_s_end_settles_it() {
        let (mut w, mut live, helper, _) = helped();
        w.tick = 40;
        w.agents[1].cost = 700;
        live.prompt(&mut w, 1, "fix it".into(), None).unwrap();
        assert_eq!(w.agents[1].task.round, Some((700, 40)));
        assert_eq!(
            w.agents[helper].task.round, None,
            "its own prompt opens its own"
        );
        // Its done mid-turn: the change waits unsettled; the turn's end
        // (Claude Code's cost in millionths of a dollar) settles it. The
        // step reads the clock: 12 ticks on, or 13 across a boundary.
        w.started_at = crate::live::now() - 52 * SECONDS_PER_TICK;
        w.change_ready(Change {
            agent: 1,
            title: "fix".into(),
            files: Vec::new(),
            patch: String::new(),
            runs: Vec::new(),
            check: Check::None,
            spend: None,
        });
        assert_eq!(w.change_of(1).unwrap().spend, None);
        // A model priced at nothing in it: the change's figure is what it
        // came to at least (record 132).
        hear(&live, 1, AgentEvent::Unpriced(Some("claude-new".into())));
        hear(
            &live,
            1,
            AgentEvent::TurnDone {
                ok: true,
                text: "done".into(),
                cost: 250_000,
                denied: 0,
            },
        );
        live.step(&mut w);
        assert_eq!(w.agents[1].cost, 2_500);
        let s = w.change_of(1).unwrap().spend.unwrap();
        assert_eq!((s.cost, s.rounds, s.unpriced), (1_800, 1, true));
        assert!((12..=13).contains(&s.ticks), "{s:?}");
        assert_eq!(w.agents[1].task.round, None);
        // A failed turn's end settles too: what it cost is spent.
        live.prompt(&mut w, 1, "again".into(), None).unwrap();
        w.change_ready(Change {
            agent: 1,
            title: "fix".into(),
            files: Vec::new(),
            patch: String::new(),
            runs: Vec::new(),
            check: Check::None,
            spend: None,
        });
        hear(
            &live,
            1,
            AgentEvent::TurnDone {
                ok: false,
                text: "x".into(),
                cost: 300_000,
                denied: 0,
            },
        );
        live.step(&mut w);
        assert!(matches!(
            w.change_of(1).unwrap().spend,
            Some(Spend {
                cost: 2_300,
                rounds: 2,
                ..
            })
        ));
    }

    fn lost_note(helper: &str, said: &str) -> String {
        format!(
            "From imece (not from the person or a teammate):\n- Your helper {helper} stopped \
             before it sent you done ({said}). The part you gave it is not done: do it \
             yourself, or hire another helper for it.\n\nGo on with your task."
        )
    }

    #[test]
    fn a_helper_that_stops_before_done_is_told_to_its_idle_worker_at_once() {
        let (mut w, mut live, helper, heard) = helped();
        let name = w.agents[helper].name.clone();
        hear(
            &live,
            helper,
            AgentEvent::Stopped("\n  rate limited\nstack".into()),
        );
        live.step(&mut w);
        assert_eq!(*heard.lock().unwrap(), [lost_note(&name, "rate limited")]);
        assert_eq!((live.woken[1], live.lost[1], live.turns[1]), (1, false, 1));
        assert_eq!(
            (live.woken[0], live.turns[0]),
            (0, 0),
            "nobody else is woken"
        );
        assert!(
            w.agents[1]
                .trace
                .iter()
                .any(|l| l.ends_with(&format!("told: {name} stopped")))
        );
        // Once.
        live.step(&mut w);
        assert_eq!(heard.lock().unwrap().len(), 1);
        // What it is told is cut.
        let (mut w, mut live, helper, heard) = helped();
        let long = "é".repeat(LOST_SAID + 5);
        hear(&live, helper, AgentEvent::Stopped(long));
        live.step(&mut w);
        let said = &heard.lock().unwrap()[0];
        assert!(
            said.contains(&format!("({})", "é".repeat(LOST_SAID))),
            "{said}"
        );
    }

    #[test]
    fn a_worker_that_waits_on_the_person_hears_of_its_helper_when_that_ends() {
        let (mut w, mut live, helper, heard) = helped();
        w.inbox.push(item(InboxKind::Question, 1, "Which?"));
        hear(&live, helper, AgentEvent::Stopped("gone".into()));
        live.step(&mut w);
        assert!(heard.lock().unwrap().is_empty() && live.lost[1]);
        w.inbox[0].open = false;
        live.step(&mut w);
        assert_eq!(heard.lock().unwrap().len(), 1);
    }

    #[test]
    fn a_busy_worker_hears_of_its_helper_when_its_turn_ends_and_only_once() {
        let (mut w, mut live, helper, heard) = helped();
        let name = w.agents[helper].name.clone();
        live.prompt(&mut w, 1, "fix it".into(), None).unwrap();
        hear(&live, helper, AgentEvent::Stopped("gone".into()));
        live.step(&mut w);
        assert_eq!(heard.lock().unwrap().len(), 1, "not mid-turn");
        assert!(live.lost[1]);
        let done = || AgentEvent::TurnDone {
            ok: true,
            text: "done".into(),
            cost: 0,
            denied: 0,
        };
        hear(&live, 1, done());
        live.step(&mut w);
        assert_eq!(heard.lock().unwrap()[1], lost_note(&name, "gone"));
        // Told with the next prompt instead: nothing more to wake it for.
        let (mut w, mut live, helper, heard) = helped();
        live.prompt(&mut w, 1, "fix it".into(), None).unwrap();
        hear(&live, helper, AgentEvent::Stopped("gone".into()));
        live.step(&mut w);
        live.prompt(&mut w, 1, "and this".into(), None).unwrap();
        assert!(!live.lost[1]);
        assert!(heard.lock().unwrap()[1].contains("Your helper"));
        for _ in 0..2 {
            hear(&live, 1, done());
            live.step(&mut w);
        }
        assert_eq!(heard.lock().unwrap().len(), 2);
    }

    #[test]
    fn a_helper_s_last_turn_failing_tells_its_worker_but_not_a_retry_or_one_queued() {
        let failed = |text: &str| AgentEvent::TurnDone {
            ok: false,
            text: text.into(),
            cost: 0,
            denied: 0,
        };
        // Its last turn, failed for good.
        let (mut w, mut live, helper, heard) = helped();
        live.turns[helper] = 1;
        hear(&live, helper, failed("credit balance too low"));
        live.step(&mut w);
        let name = w.agents[helper].name.clone();
        assert_eq!(
            *heard.lock().unwrap(),
            [lost_note(&name, "credit balance too low")]
        );
        // Another turn queued after it: it goes on.
        let (mut w, mut live, helper, heard) = helped();
        live.turns[helper] = 2;
        hear(&live, helper, failed("x"));
        live.step(&mut w);
        assert!(heard.lock().unwrap().is_empty() && !live.lost[1]);
        // Both prompts taken into the one turn that failed: it was the
        // last; one of three still queued after it.
        let (mut w, mut live, helper, heard) = helped();
        live.turns[helper] = 2;
        hear(&live, helper, AgentEvent::Took);
        hear(&live, helper, AgentEvent::Took);
        hear(&live, helper, failed("x"));
        live.step(&mut w);
        assert_eq!(heard.lock().unwrap().len(), 1);
        // More echoed than imece counted: still its last.
        let (mut w, mut live, helper, heard) = helped();
        live.turns[helper] = 1;
        hear(&live, helper, AgentEvent::Took);
        hear(&live, helper, AgentEvent::Took);
        hear(&live, helper, failed("x"));
        live.step(&mut w);
        assert_eq!(heard.lock().unwrap().len(), 1);
        let (mut w, mut live, helper, heard) = helped();
        live.turns[helper] = 3;
        hear(&live, helper, AgentEvent::Took);
        hear(&live, helper, AgentEvent::Took);
        hear(&live, helper, failed("x"));
        live.step(&mut w);
        assert!(heard.lock().unwrap().is_empty() && !live.lost[1]);
        // A passing error is tried again: not lost yet.
        let (mut w, mut live, helper, heard) = helped();
        live.turns[helper] = 1;
        hear(
            &live,
            helper,
            AgentEvent::Failed(imece_agent::Fault::Passing),
        );
        hear(&live, helper, failed("overloaded"));
        live.step(&mut w);
        assert!(w.agents[helper].retry.is_some());
        assert!(heard.lock().unwrap().is_empty() && !live.lost[1]);
    }

    #[test]
    fn nobody_is_told_of_a_helper_that_sent_done_a_worker_or_a_worker_gone_and_a_sleeper_waits() {
        // A helper that sent done leaves; its end is no loss.
        let (mut w, mut live, helper, heard) = helped();
        live.leaving[helper] = true;
        hear(&live, helper, AgentEvent::Stopped("gone".into()));
        live.step(&mut w);
        assert!(heard.lock().unwrap().is_empty() && !live.lost[1]);
        // A worker's end tells its leader nothing.
        let (mut w, mut live, _, _) = helped();
        hear(&live, 1, AgentEvent::Stopped("gone".into()));
        live.step(&mut w);
        assert!(live.notes[0].is_empty() && !live.lost[0]);
        // The worker gone: nobody to tell.
        let (mut w, mut live, helper, _) = helped();
        w.agents[1].gone = true;
        hear(&live, helper, AgentEvent::Stopped("gone".into()));
        live.step(&mut w);
        assert!(live.notes[1].is_empty());
        // Asleep, stopped, or woken to the cap: told, not woken.
        for status in [Status::Asleep, Status::Stopped] {
            let (mut w, mut live, helper, heard) = helped();
            w.agents[1].status = status;
            hear(&live, helper, AgentEvent::Stopped("gone".into()));
            live.step(&mut w);
            assert!(
                heard.lock().unwrap().is_empty() && live.lost[1],
                "{status:?}"
            );
            assert_eq!(live.notes[1].len(), 1);
        }
        let (mut w, mut live, helper, heard) = helped();
        live.woken[1] = MAX_WOKEN;
        hear(&live, helper, AgentEvent::Stopped("gone".into()));
        live.step(&mut w);
        assert!(heard.lock().unwrap().is_empty() && live.lost[1]);
        live.woken[1] = MAX_WOKEN - 1;
        live.step(&mut w);
        assert_eq!(heard.lock().unwrap().len(), 1, "under the cap");
        assert_eq!(live.woken[1], MAX_WOKEN);
    }

    #[test]
    fn a_passing_vendor_error_is_tried_again_when_due_with_a_word_to_go_on() {
        let mut w = team(1);
        let mut live = idle_live(2);
        let heard = Arc::new(std::sync::Mutex::new(Vec::new()));
        let tmp = std::env::temp_dir();
        live.ready(
            &mut w,
            0,
            Ok((
                Box::new(Heard(heard.clone(), false)),
                tmp,
                None,
                None,
                imece_agent::copy::Brought::default(),
                setup::Done::None,
                None,
                Moved::Stayed,
            )),
        );
        live.prompt(&mut w, 0, "fix it".into(), None).unwrap();
        assert_eq!(live.turns[0], 1);
        let fail = |live: &mut Live, w: &mut World, f: imece_agent::Fault| {
            hear(live, 0, AgentEvent::Failed(f));
            hear(
                live,
                0,
                AgentEvent::TurnDone {
                    ok: false,
                    text: "API Error: overloaded".into(),
                    cost: 0,
                    denied: 0,
                },
            );
            live.step(w);
        };
        fail(&mut live, &mut w, imece_agent::Fault::Passing);
        assert_eq!(w.tick, 0);
        assert_eq!(w.agents[0].retry, Some(Retry { attempt: 1, at: 6 }));
        assert_eq!(w.agents[0].fault, None, "taken by the turn's end");
        assert_eq!(w.agents[0].status, Status::Idle);
        assert_eq!(live.turns[0], 0);
        let at = |live: &mut Live, w: &mut World, tick: u64| {
            w.started_at = now() - tick * SECONDS_PER_TICK;
            live.step(w);
            assert_eq!(w.tick, tick);
        };
        at(&mut live, &mut w, 5);
        assert_eq!(heard.lock().unwrap().len(), 1, "not due yet");
        at(&mut live, &mut w, 6);
        assert_eq!(heard.lock().unwrap()[1], RETRY_PROMPT);
        assert_eq!(w.agents[0].retry, None);
        assert_eq!(live.turns[0], 1);
        assert!(
            w.agents[0]
                .trace
                .iter()
                .any(|l| l.ends_with("retry 1/3 after an API error"))
        );
        // It fails again: the next try is further off; over budget it
        // waits.
        fail(&mut live, &mut w, imece_agent::Fault::Passing);
        assert_eq!(w.agents[0].retry, Some(Retry { attempt: 2, at: 30 }));
        w.agents[0].cost = w.budget;
        at(&mut live, &mut w, 30);
        assert_eq!(heard.lock().unwrap().len(), 2, "the pot is spent");
        w.agents[0].cost = 0;
        // Mail goes in first: it is the next try, none waits beside it.
        live.prompt(&mut w, 0, "mail".into(), None).unwrap();
        assert_eq!(w.agents[0].retry, None);
        assert_eq!(heard.lock().unwrap()[2], "mail");
        // A good turn counts from nothing again.
        hear(
            &live,
            0,
            AgentEvent::TurnDone {
                ok: true,
                text: "done".into(),
                cost: 0,
                denied: 0,
            },
        );
        live.step(&mut w);
        assert_eq!(w.agents[0].tries, 0);
        // The account's fault waits for the person.
        live.prompt(&mut w, 0, "again".into(), None).unwrap();
        fail(&mut live, &mut w, imece_agent::Fault::Account);
        assert_eq!(w.agents[0].retry, None);
        assert_eq!(
            w.open_item(0, InboxKind::Fault).unwrap().detail,
            "API Error: overloaded"
        );
        // A session that ends takes its retry and its count with it.
        w.agents[0].retry = Some(Retry { attempt: 1, at: 99 });
        w.agents[0].tries = 1;
        w.agents[0].fault = Some(Fault::Passing);
        live.end(&mut w, 0);
        assert_eq!(
            (w.agents[0].retry, w.agents[0].tries, w.agents[0].fault),
            (None, 0, None)
        );
    }

    #[test]
    fn a_vendor_fault_on_a_first_turn_is_tried_again_not_taken_for_a_missing_model() {
        let mut w = team(0);
        let mut live = idle_live(1);
        live.claude.ladder.standard = Some("sonnet".into());
        live.hire(&mut w, 0, &json!({ "tier": "strong" })).unwrap();
        live.turns[1] = 1;
        hear(&live, 1, AgentEvent::Failed(imece_agent::Fault::Passing));
        hear(
            &live,
            1,
            AgentEvent::TurnDone {
                ok: false,
                text: "overloaded".into(),
                cost: 0,
                denied: 0,
            },
        );
        live.step(&mut w);
        assert_eq!(live.models[1].as_deref(), Some("opus"), "no fallback");
        assert!(live.tiered[1]);
        assert_eq!(w.agents[1].retry.map(|r| r.attempt), Some(1));
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
    fn a_turn_that_did_not_say_its_cost_leaves_its_agent_unpriced_until_one_says_the_whole() {
        let mut w = team(1);
        let done = |cost| AgentEvent::TurnDone {
            ok: true,
            text: "ok".into(),
            cost,
            denied: 0,
        };
        apply(&mut w, 1, AgentEvent::Unpriced(None), 0);
        assert!(w.agents[1].unpriced.said);
        let last = w.agents[1].trace.back().unwrap();
        assert!(
            last.ends_with("cost unknown: the turn's result said none"),
            "{last}"
        );
        apply(&mut w, 1, done(500_000), 100);
        assert_eq!(w.agents[1].cost, 5_100);
        assert_eq!(
            (w.agents[1].unpriced.said, w.agents[1].unpriced.session),
            (false, true)
        );
        apply(&mut w, 1, done(600_000), 100);
        assert!(!w.agents[1].unpriced.any(), "the session's whole said");
    }

    #[test]
    fn a_session_ended_mid_turn_leaves_a_gap_and_one_not_started_or_that_said_its_cost_none() {
        let mut w = team(1);
        let mut live = idle_live(2);
        // Mid-turn in a running process: that turn never says its cost.
        live.slots[1] = Slot::Running(Box::new(Stub));
        live.turns[1] = 1;
        live.end(&mut w, 1);
        assert_eq!(w.agents[1].unpriced.gaps, 1);
        // Between turns: nothing.
        live.slots[1] = Slot::Running(Box::new(Stub));
        live.end(&mut w, 1);
        assert_eq!(w.agents[1].unpriced.gaps, 1);
        // Its process still starting: nothing ran.
        live.slots[1] = Slot::Starting(vec!["x".into()]);
        live.turns[1] = 1;
        live.end(&mut w, 1);
        assert_eq!(w.agents[1].unpriced.gaps, 1);
        // A turn at the cap said its cost before its session was renewed.
        w.budget = 100 * 10_000;
        live.slots[0] = Slot::Running(Box::new(Stub));
        live.inflight[0] = vec!["hello".into()];
        live.turns[0] = 1;
        capped(&live, 0, 20);
        live.step(&mut w);
        assert_eq!(live.generation[0], 1, "renewed");
        assert_eq!(w.agents[0].unpriced.gaps, 0);
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
            Ok((
                Box::new(Stub),
                cwd.clone(),
                None,
                None,
                imece_agent::copy::Brought::default(),
                setup::Done::None,
                None,
                Moved::Stayed,
            )),
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
        // /clear without one of Claude Code's own: the session ends, and
        // is not one to go on with later.
        live.sessions[1] = Some(Resumable {
            session: "686c6d27-d50b-4ae2-ad6a-159bf2984671".into(),
            spent: 5,
            models: None,
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
        live.ready(
            &mut w,
            1,
            Ok((
                Box::new(Stub),
                cwd,
                None,
                None,
                imece_agent::copy::Brought::default(),
                setup::Done::None,
                None,
                Moved::Stayed,
            )),
        );
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
        use std::os::unix::fs::PermissionsExt;
        // The helper's stand-in alone writes its arguments: once it stops,
        // its worker is woken (record 09) and runs in the same tree.
        let fake = dir.join("fake-helper");
        std::fs::write(
            &fake,
            "#!/bin/sh\ncase \"$*\" in *'imece claude-1.1'*) printf '%s\\n' \"$@\" > helper-args.txt;; esac\n",
        )
        .unwrap();
        std::fs::set_permissions(&fake, std::fs::Permissions::from_mode(0o700)).unwrap();
        let mut w = team(1);
        let mut live = Live::new(
            2,
            claude_at(&fake.to_string_lossy()),
            repo.clone(),
            dir.join("worktrees"),
        );
        live.hire(&mut w, 1, &json!({})).unwrap();
        live.told(&mut w, 2, "hello").unwrap();
        wait_for(&mut live, &mut w, |w| w.agents[2].status == Status::Stopped);
        // The stand-in wrote its arguments where it ran: the worker's tree.
        let tree = dir.join("worktrees").join("claude-1");
        let args = std::fs::read_to_string(tree.join("helper-args.txt")).unwrap();
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
        // A scratch directory of its own, private, for its file tools and
        // named in its instructions.
        let scratch = dir.join("worktrees-scratch").join("claude-1.1");
        let scratch = scratch.canonicalize().unwrap();
        assert_eq!(after("--add-dir"), scratch.to_string_lossy());
        let mode = std::fs::metadata(&scratch).unwrap().permissions().mode();
        assert_eq!(mode & 0o777, 0o700);
        // The instructions go on over several lines.
        let note = format!("Your scratch directory is {}: ", scratch.display());
        assert!(args.iter().any(|l| l.contains(&note)), "{args:?}");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn notes_are_rewritten_private_and_never_through_a_link() {
        use std::os::unix::fs::PermissionsExt;
        let at = std::env::temp_dir().join(format!("imece-write-notes-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&at);
        let dir = at.join("notes").join("claude-1");
        assert_eq!(write_notes(&dir, None).unwrap(), None);
        assert!(!dir.exists(), "nothing to say, nothing made");
        let made = write_notes(&dir, Some("first")).unwrap().unwrap();
        assert_eq!(made, dir.canonicalize().unwrap());
        let file = made.join(NOTES_FILE);
        assert_eq!(std::fs::read_to_string(&file).unwrap(), "first");
        let mode = |p: &Path| std::fs::metadata(p).unwrap().permissions().mode() & 0o777;
        assert_eq!((mode(&made), mode(&file)), (0o700, 0o600));
        write_notes(&dir, Some("second")).unwrap();
        assert_eq!(std::fs::read_to_string(&file).unwrap(), "second");
        // A link left in its place: the link goes, what it named stays.
        let person = at.join("person's");
        std::fs::write(&person, "theirs").unwrap();
        std::fs::remove_file(&file).unwrap();
        std::os::unix::fs::symlink(&person, &file).unwrap();
        write_notes(&dir, Some("third")).unwrap();
        assert_eq!(std::fs::read_to_string(&person).unwrap(), "theirs");
        assert!(!std::fs::symlink_metadata(&file).unwrap().is_symlink());
        assert_eq!(std::fs::read_to_string(&file).unwrap(), "third");
        // What cannot go is said: a directory in its place.
        std::fs::remove_file(&file).unwrap();
        std::fs::create_dir(&file).unwrap();
        assert!(write_notes(&dir, Some("fourth")).is_err());
        let _ = std::fs::remove_dir_all(&at);
    }

    #[test]
    fn notes_are_each_agent_s_own_and_go_with_it() {
        let mut w = team(2);
        let at = std::env::temp_dir().join(format!("imece-notes-of-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&at);
        let live = Live::new(3, claude_at("claude"), at.join("r"), at.join("w"));
        let root = at.join("w-notes");
        assert_eq!(live.notes_of(&w, 0), root.join(&w.agents[0].name));
        assert_eq!(live.notes_of(&w, 1), root.join("claude-1"));
        w.agents[1].role = Role::Solo;
        assert_eq!(live.notes_of(&w, 1), root.join("solo-0"));
        std::fs::create_dir_all(live.notes_of(&w, 0)).unwrap();
        std::fs::create_dir_all(live.notes_of(&w, 2)).unwrap();
        live.drop_scratch(&w, 0);
        assert!(!live.notes_of(&w, 0).exists(), "a leader's too");
        assert!(live.notes_of(&w, 2).exists(), "another's stays");
        let _ = std::fs::remove_dir_all(&at);
    }

    #[test]
    fn a_scratch_directory_is_each_writer_s_own_and_a_leader_has_none() {
        let mut w = team(2);
        let live = Live::new(
            3,
            claude_at("/nonexistent/claude"),
            PathBuf::from("/r"),
            PathBuf::from("/state/worktrees/r-1"),
        );
        let root = Path::new("/state/worktrees/r-1-scratch");
        assert_eq!(live.scratch_of(&w, 0), None, "the leader writes nothing");
        assert_eq!(live.scratch_of(&w, 1), Some(root.join("claude-1")));
        assert_eq!(live.scratch_of(&w, 2), Some(root.join("claude-2")));
        // A helper has its own, not its worker's; a solo's goes by its
        // project, as its worktree does.
        w.agents[2].role = Role::Helper;
        w.agents[2].parent = Some(1);
        assert_eq!(live.scratch_of(&w, 2), Some(root.join("claude-2")));
        w.agents[1].role = Role::Solo;
        assert_eq!(live.scratch_of(&w, 1), Some(root.join("solo-0")));
    }

    #[test]
    fn a_scratch_directory_is_made_private_once_and_must_be_a_directory() {
        use std::os::unix::fs::PermissionsExt;
        let base = trees("scratch-make");
        let dir = base.join("a").join("claude-1");
        let made = make_private(&dir).unwrap();
        assert_eq!(made, dir.canonicalize().unwrap());
        let mode = std::fs::metadata(&dir).unwrap().permissions().mode();
        assert_eq!(mode & 0o777, 0o700);
        std::fs::write(dir.join("notes.md"), "kept").unwrap();
        assert_eq!(make_private(&dir).unwrap(), made, "there already");
        assert!(dir.join("notes.md").exists());
        let file = base.join("a").join("file");
        std::fs::write(&file, "").unwrap();
        assert!(make_private(&file).is_err(), "a file");
        let link = base.join("a").join("link");
        std::os::unix::fs::symlink(&dir, &link).unwrap();
        assert!(make_private(&link).is_err(), "a link to a directory");
        // Any other failure is that failure, not taken for one that is
        // there: a parent it may not write to.
        let shut = base.join("shut");
        std::fs::create_dir_all(&shut).unwrap();
        std::fs::set_permissions(&shut, std::fs::Permissions::from_mode(0o500)).unwrap();
        let e = make_private(&shut.join("claude-1")).unwrap_err();
        std::fs::set_permissions(&shut, std::fs::Permissions::from_mode(0o700)).unwrap();
        assert_eq!(e.kind(), io::ErrorKind::PermissionDenied, "{e}");
        let _ = std::fs::remove_dir_all(&base);
    }

    #[test]
    fn a_leaving_agent_s_scratch_directory_goes_with_it() {
        let repo = checkout("scratch-leave");
        let root = trees("scratch-leave");
        let mut w = team(1);
        let mut live = Live::new(
            2,
            claude_at("/nonexistent/claude"),
            repo.clone(),
            root.clone(),
        );
        let dir = live.scratch_of(&w, 1).unwrap();
        std::fs::create_dir_all(&dir).unwrap();
        live.dismiss(&mut w, 1);
        assert!(!dir.exists());
        assert!(traced_soon(
            &mut live,
            &mut w,
            1,
            "its scratch directory was removed"
        ));
        for d in [repo, root, dir.parent().unwrap().to_path_buf()] {
            let _ = std::fs::remove_dir_all(d);
        }
    }

    /// A stand-in for `claude` whose `args.txt` is there whole or not at
    /// all, so a test may read it the moment it is there, and that stays up
    /// until its stdin closes: a session that ends at once wakes others (a
    /// lost helper's worker), who run `lonca` under the test.
    fn whole_args(dir: &Path) -> String {
        use std::os::unix::fs::PermissionsExt;
        let path = dir.join("whole-claude");
        let body =
            "#!/bin/sh\nprintf '%s\\n' \"$@\" > args.tmp\nmv args.tmp args.txt\ncat > /dev/null\n";
        std::fs::write(&path, body).unwrap();
        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o700)).unwrap();
        path.to_string_lossy().into_owned()
    }

    #[test]
    fn a_worker_on_lonca_works_in_a_workspace_with_lonca_beside_the_bus() {
        let repo = checkout("seat-repo");
        let root = trees("seat");
        let dir = std::env::temp_dir().join(format!("imece-seat-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let (place, fake) = imece_agent::lonca_fake::program(
            r#"prev=""
               for a in "$@"; do
                 if [ "$prev" = --workspace ]; then
                   mkdir -p "$a/.lonca"
                   echo '{"repo":"demo"}' > "$a/.lonca/workspace.json"
                 fi
                 prev="$a"
               done
               echo '{}'"#,
        );
        let mut w = team(1);
        w.on_lonca(0, Some("demo".into()));
        let mut live = Live::new(2, claude_at(&whole_args(&dir)), repo.clone(), root.clone());
        live.lonca = lonca::Watch::start(place.clone());
        live.slots[1] = Slot::Starting(Vec::new());
        live.start(&mut w, 1, None);
        wait_for(&mut live, &mut w, |w| w.agents[1].workspace.is_some());
        assert_eq!(w.agents[1].workspace.as_deref(), Some("demo"));
        let trace = &w.agents[1].trace;
        assert!(
            trace
                .iter()
                .any(|l| l.contains("on Lonca: a workspace of demo"))
        );

        let tree = root.join("claude-1").canonicalize().unwrap();
        assert!(!tree.join(".git").exists(), "no worktree under it");
        assert_eq!(live.cwd_of(&w, 1), tree, "a session runs where it did");
        let file = tree.join("args.txt");
        wait_for(&mut live, &mut w, |_| file.exists());
        let args = std::fs::read_to_string(&file).unwrap();
        let after = |flag: &str| {
            let mut it = args.lines();
            it.find(|l| *l == flag).expect(flag);
            it.next().unwrap().to_string()
        };
        assert!(after("--settings").contains(&place.program), "{args}");
        let config = after("--mcp-config");
        assert!(config.contains(r#""lonca""#), "{config}");
        assert!(
            config.contains(r#""imece""#) || live.bus.is_none(),
            "{config}"
        );
        assert!(
            args.contains("Lonca workspace of repository demo"),
            "{args}"
        );
        let _ = std::fs::remove_dir_all(&repo);
        let _ = std::fs::remove_dir_all(&root);
        let _ = std::fs::remove_dir_all(&dir);
        let _ = std::fs::remove_dir_all(&fake);
    }

    /// A worker hired before `:lonca import` starts again: its clean
    /// worktree goes and a workspace takes its place (record 119).
    #[test]
    fn a_worker_s_clean_worktree_gives_way_to_a_workspace_at_its_start() {
        let repo = checkout("onto-repo");
        let root = trees("onto");
        let dir = std::env::temp_dir().join(format!("imece-onto-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let (place, fake) = imece_agent::lonca_fake::program(
            r#"prev=""
               for a in "$@"; do
                 if [ "$prev" = --workspace ]; then
                   mkdir -p "$a/.lonca"
                   echo '{"repo":"demo"}' > "$a/.lonca/workspace.json"
                 fi
                 prev="$a"
               done
               echo '{}'"#,
        );
        let tree = worktree::ensure(&repo, &root, "claude-1", None)
            .unwrap()
            .path;
        assert!(tree.join(".git").exists(), "a worktree before the import");
        let mut w = team(1);
        w.on_lonca(0, Some("demo".into()));
        let up = StoreSeen::Up {
            trunk: "t".into(),
            pending: 0,
            behind: Behind::Unknown,
        };
        w.store_seen(0, "demo", up);
        let mut live = Live::new(2, claude_at(&whole_args(&dir)), repo.clone(), root.clone());
        live.lonca = lonca::Watch::start(place.clone());
        live.slots[1] = Slot::Starting(Vec::new());
        live.start(&mut w, 1, None);
        wait_for(&mut live, &mut w, |w| w.agents[1].workspace.is_some());
        assert!(!tree.join(".git").exists(), "the worktree went");
        assert!(tree.join(".lonca/workspace.json").exists());
        assert!(w.journal.iter().any(|e| matches!(
            &e.event,
            Event::OntoLonca { agent: 1, repo } if repo == "demo"
        )));
        assert!(live.notes[1].iter().any(|n| n == lonca::ONTO_LONCA));
        let _ = std::fs::remove_dir_all(&repo);
        let _ = std::fs::remove_dir_all(&root);
        let _ = std::fs::remove_dir_all(&dir);
        let _ = std::fs::remove_dir_all(&fake);
    }

    #[test]
    fn a_prompt_to_a_worker_whose_lonca_session_runs_out_goes_to_a_new_one() {
        let repo = checkout("renew-repo");
        let root = trees("renew");
        let dir = std::env::temp_dir().join(format!("imece-renew-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        // A live heartbeat: only the time left asks for a new session.
        let (place, fake) = imece_agent::lonca_fake::program("echo '{}'");
        let tree = root.join("claude-1");
        std::fs::create_dir_all(tree.join(".lonca")).unwrap();
        std::fs::write(tree.join(".lonca/workspace.json"), r#"{"repo":"demo"}"#).unwrap();
        let mut w = team(1);
        w.on_lonca(0, Some("demo".into()));
        w.agents[1].workspace = Some("demo".into());
        let mut live = Live::new(2, claude_at(&whole_args(&dir)), repo.clone(), root.clone());
        live.lonca = lonca::Watch::start(place);
        let name = w.agents[1].name.clone();
        live.lonca
            .ends
            .insert(("demo".into(), name), Instant::now());
        live.slots[1] = Slot::Running(Box::new(Stub));
        live.prompt(&mut w, 1, "go on".into(), None).unwrap();
        assert!(matches!(live.slots[1], Slot::Starting(_)), "a new harness");
        wait_for(&mut live, &mut w, |w| !w.agents[1].starting);
        let told = imece_agent::lonca_fake::seen(&fake);
        assert!(told.contains("--continue"), "{told}");
        assert!(told.contains("--agent=claude-1"), "{told}");
        let _ = std::fs::remove_dir_all(&repo);
        let _ = std::fs::remove_dir_all(&root);
        let _ = std::fs::remove_dir_all(&dir);
        let _ = std::fs::remove_dir_all(&fake);
    }

    #[test]
    fn a_worker_whose_hooks_lonca_did_not_hear_goes_on_under_a_new_session() {
        let repo = checkout("unheard-repo");
        let root = trees("unheard");
        let dir = std::env::temp_dir().join(format!("imece-unheard-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        // A live heartbeat and time to spare: only the hooks ask for a new
        // session.
        let (place, fake) = imece_agent::lonca_fake::program("echo '{}'");
        let tree = root.join("claude-1");
        std::fs::create_dir_all(tree.join(".lonca")).unwrap();
        std::fs::write(tree.join(".lonca/workspace.json"), r#"{"repo":"demo"}"#).unwrap();
        let mut w = team(1);
        w.on_lonca(0, Some("demo".into()));
        w.agents[1].workspace = Some("demo".into());
        let mut live = Live::new(2, claude_at(&whole_args(&dir)), repo.clone(), root.clone());
        live.lonca = lonca::Watch::start(place);
        let name = w.agents[1].name.clone();
        live.lonca
            .unheard
            .insert(("demo".into(), name), imece_agent::lonca::Hooks::Unbound);
        live.prompt(&mut w, 1, "go on".into(), None).unwrap();
        wait_for(&mut live, &mut w, |w| !w.agents[1].starting);
        let told = imece_agent::lonca_fake::seen(&fake);
        assert!(told.contains("--continue"), "{told}");
        let _ = std::fs::remove_dir_all(&repo);
        let _ = std::fs::remove_dir_all(&root);
        let _ = std::fs::remove_dir_all(&dir);
        let _ = std::fs::remove_dir_all(&fake);
    }

    #[test]
    fn a_helper_seated_first_takes_the_session_in_its_workers_name() {
        let repo = checkout("helper-seat-repo");
        let root = trees("helper-seat");
        let dir = std::env::temp_dir().join(format!("imece-hs-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let (place, fake) = imece_agent::lonca_fake::program(
            r#"case "$*" in *heartbeat*) echo 'lonca: session_inactive: over' >&2; exit 1;; esac
               echo '{}'"#,
        );
        let tree = root.join("claude-1");
        std::fs::create_dir_all(tree.join(".lonca")).unwrap();
        std::fs::write(tree.join(".lonca/workspace.json"), r#"{"repo":"demo"}"#).unwrap();
        let mut w = team(1);
        w.on_lonca(0, Some("demo".into()));
        let helper = with_helper(&mut w, 1);
        let mut live = Live::new(3, claude_at(&whole_args(&dir)), repo.clone(), root.clone());
        live.lonca = lonca::Watch::start(place);
        live.slots[helper] = Slot::Starting(Vec::new());
        live.start(&mut w, helper, None);
        wait_for(&mut live, &mut w, |w| w.agents[helper].workspace.is_some());
        let told = imece_agent::lonca_fake::seen(&fake);
        assert!(told.contains("--agent=claude-1 "), "{told}");
        let _ = std::fs::remove_dir_all(&repo);
        let _ = std::fs::remove_dir_all(&root);
        let _ = std::fs::remove_dir_all(&dir);
        let _ = std::fs::remove_dir_all(&fake);
    }

    #[test]
    fn a_worker_that_leaves_its_workspace_ends_its_session_a_helper_does_not() {
        let (place, fake) =
            imece_agent::lonca_fake::program(r#"echo "$PWD|$*" >> "$(dirname "$0")/all""#);
        let ran = |fake: &Path| std::fs::read_to_string(fake.join("all")).unwrap_or_default();
        let repo = checkout("leave-repo");
        let root = trees("leave");
        std::fs::create_dir_all(root.join("claude-1")).unwrap();
        let tree = root.join("claude-1").canonicalize().unwrap();
        let mut w = team(1);
        w.agents[1].workspace = Some("demo".into());
        let helper = with_helper(&mut w, 1);
        w.agents[helper].workspace = Some("demo".into());
        let mut live = Live::new(3, claude_at("no-such-claude"), repo.clone(), root.clone());
        live.lonca = lonca::Watch::start(place);
        live.stand_up(&w, helper);
        w.agents[1].workspace = None;
        live.stand_up(&w, 1);
        w.agents[1].workspace = Some("demo".into());
        live.stand_up(&w, 1);
        assert!(traced_soon(&mut live, &mut w, 1, "its Lonca session ended"));
        assert_eq!(
            ran(&fake).trim_end(),
            format!("{}|session end --json", tree.display()),
            "the worker's alone"
        );
        let _ = std::fs::remove_dir_all(&repo);
        let _ = std::fs::remove_dir_all(&root);
        let _ = std::fs::remove_dir_all(&fake);
    }

    #[test]
    fn a_worker_off_lonca_gets_no_word_of_it() {
        let repo = checkout("no-seat-repo");
        let root = trees("no-seat");
        let dir = std::env::temp_dir().join(format!("imece-no-seat-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let mut w = team(1);
        let mut live = Live::new(2, claude_at(&whole_args(&dir)), repo.clone(), root.clone());
        live.slots[1] = Slot::Starting(Vec::new());
        w.agents[1].starting = true;
        live.start(&mut w, 1, None);
        wait_for(&mut live, &mut w, |w| !w.agents[1].starting);
        let tree = root.join("claude-1").canonicalize().unwrap();
        assert!(tree.join(".git").exists());
        assert_eq!(w.agents[1].workspace, None);
        let file = tree.join("args.txt");
        wait_for(&mut live, &mut w, |_| file.exists());
        let args = std::fs::read_to_string(&file).unwrap();
        assert!(!args.contains("--settings"), "{args}");
        assert!(!args.contains("Lonca"), "{args}");
        let _ = std::fs::remove_dir_all(&repo);
        let _ = std::fs::remove_dir_all(&root);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn a_worker_hired_from_a_start_point_works_there() {
        let repo = checkout("from-repo");
        let root = trees("from");
        let old = git_in(&repo, &["rev-parse", "HEAD"]);
        git_in(&repo, &["branch", "old"]);
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
        let dir = std::env::temp_dir().join(format!("imece-from-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let mut w = team(1);
        let mut live = Live::new(2, claude_at(&fake_claude(&dir)), repo.clone(), root.clone());
        // Refused before anyone is hired: not one name, not a commit, or
        // asked by a worker for its helper.
        for (from, said) in [
            (
                "-x",
                "from must be one branch, tag or commit of the checkout",
            ),
            ("a b", "from must be one branch"),
            ("nowhere", "nowhere names no commit in "),
        ] {
            let e = live.hire(&mut w, 0, &json!({ "from": from })).unwrap_err();
            assert!(e.contains(said), "{from:?}: {e}");
        }
        let e = live.hire(&mut w, 1, &json!({ "from": "old" })).unwrap_err();
        assert!(e.starts_with("from is for the leader's workers"), "{e}");
        assert_eq!(w.agents.len(), 2, "nobody joined");
        let got = live.hire(&mut w, 0, &json!({ "from": " old " })).unwrap();
        assert!(
            got.contains("), its worktree from old. Give it work"),
            "{got}"
        );
        let id = w.agents.len() - 1;
        assert_eq!(w.agents[id].start.as_deref(), Some("old"));
        assert!(w.journal.iter().any(|e| matches!(
            &e.event,
            Event::Hired { agent, from: Some(f), .. } if *agent == id && f == "old"
        )));
        // A pull request is fetched when the worker starts, not now.
        let pr = live.hire(&mut w, 0, &json!({ "from": "#41" })).unwrap();
        assert!(pr.contains("its worktree from #41."), "{pr}");
        live.told(&mut w, id, "hello").unwrap();
        wait_for(&mut live, &mut w, |w| {
            w.agents[id].status == Status::Stopped
        });
        let tree = root.join(&w.agents[id].name);
        assert_eq!(git_in(&tree, &["rev-parse", "HEAD"]), old);
        assert!(!tree.join("b.txt").exists(), "the start point's files");
        assert!(
            w.agents[id]
                .trace
                .iter()
                .any(|l| l.ends_with(&format!("worktree made at {}", &old[..7]))),
            "{:?}",
            w.agents[id].trace
        );
        // Its pull request is not there: the worker stops with git's words.
        let pr = w.agents.len() - 1;
        live.told(&mut w, pr, "hello").unwrap();
        wait_for(&mut live, &mut w, |w| {
            w.agents[pr].status == Status::Stopped
        });
        assert!(
            w.agents[pr].topic.contains("pull request #41 of origin"),
            "{}",
            w.agents[pr].topic
        );
        let _ = std::fs::remove_dir_all(&dir);
        let _ = std::fs::remove_dir_all(&root);
        let _ = std::fs::remove_dir_all(&repo);
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
            claude_at("/nonexistent/claude"),
            std::env::temp_dir(),
            PathBuf::from("/nonexistent"),
        );
        live.claude.ladder.standard = Some("sonnet".into());
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
            (a.name.as_str(), a.project, a.role, a.status),
            (
                format!("lead-{name}").as_str(),
                1,
                Role::Leader,
                Status::Idle
            )
        );
        assert_eq!(
            live.models[1].as_deref(),
            Some("sonnet"),
            "the standard model"
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
        // A name given, another checkout.
        let other = checkout("open-other");
        assert_eq!(
            live.open_project(&mut w, &other, Some("api"), Open::Team),
            Ok(2)
        );
        assert_eq!(w.projects[2].name, "api");
        assert_eq!(w.agents[2].name, "lead-api");
        assert_eq!(live.places.len(), 3);
        let _ = std::fs::remove_dir_all(&repo);
        let _ = std::fs::remove_dir_all(&other);
    }
    #[test]
    fn a_write_outside_the_card_s_paths_is_noted_once_a_file_and_told_at_the_next_prompt() {
        let mut w = team(1);
        let c = with_card(&mut w, 1);
        let mut live = idle_live(2);
        assert!(
            !live.strayed(&mut w, 1, "src/b.rs"),
            "a card without paths has no bounds"
        );
        w.cards[c].paths = vec![Owned::new("src/a.rs"), Owned::shared("docs")];
        assert!(!live.strayed(&mut w, 1, "src/a.rs"));
        assert!(!live.strayed(&mut w, 1, "docs/x.md"));
        assert!(live.notes[1].is_empty() && w.journal.is_empty());
        assert!(live.strayed(&mut w, 1, "src/b.rs"));
        assert_eq!(
            live.notes[1],
            [
                "you changed src/b.rs, which #1 does not own (it owns src/a.rs, docs (shared)); \
              say so in your done, or ask lead to widen the card"
            ]
        );
        assert!(matches!(
            w.journal.last().unwrap().event,
            Event::Strayed { agent: 1, card, ref path } if card == c && path == "src/b.rs"
        ));
        assert!(matches!(
            w.cards[c].story.last().unwrap().event,
            StoryEvent::Strayed(ref f) if f == "src/b.rs"
        ));
        // The same file again says nothing more; another file does.
        assert!(!live.strayed(&mut w, 1, "src/b.rs"));
        assert_eq!((live.notes[1].len(), w.journal.len()), (1, 1));
        assert!(live.strayed(&mut w, 1, "src/c.rs"));
        assert_eq!((live.notes[1].len(), w.journal.len()), (2, 2));
        // Only a worker's writes are bounded: the leader and a helper
        // with a card are not.
        w.agents[0].card = Some(c);
        assert!(!live.strayed(&mut w, 0, "src/d.rs"));
        let mut other = team(1);
        let h = with_helper(&mut other, 1);
        let hc = with_card(&mut other, h);
        other.cards[hc].paths = vec![Owned::new("src/a.rs")];
        assert!(other.strayed(h, "src/d.rs").is_none());
        other.agents[h].role = Role::Worker;
        assert_eq!(other.strayed(h, "src/d.rs"), Some(hc), "a worker's is");
        other.agents[h].card = None;
        assert!(other.strayed(h, "src/e.rs").is_none(), "without a card");
        // The note rides the next prompt.
        let opening = Opening {
            title: "next",
            paths: &[],
            checks: &[],
            closes: None,
            card: None,
            person: false,
            lazy: false,
        };
        live.prompt(&mut w, 1, "go on".into(), Some(opening))
            .unwrap();
        let q = queued(&live, 1);
        assert!(
            q[0].contains("- you changed src/c.rs, which #1 does not own"),
            "{q:?}"
        );
        assert!(live.notes[1].is_empty());
    }

    #[test]
    fn a_handed_card_is_taken_at_its_try_and_an_unfinished_one_is_only_the_workers_own() {
        let open = |card| Opening {
            title: "next",
            paths: &[],
            checks: &[],
            closes: None,
            card,
            person: false,
            lazy: false,
        };
        // An assign naming a card: the worker takes it at its try now,
        // whatever it held, and no card is made.
        let mut w = team(2);
        let mut live = idle_live(3);
        let mine = with_card(&mut w, 1);
        let c = with_card(&mut w, 2);
        w.cards[c].attempt = 3;
        w.cards[c].owner = Some(1);
        live.prompt(&mut w, 1, "go".into(), Some(open(Some(c))))
            .unwrap();
        assert_eq!((w.agents[1].card, w.agents[1].attempt), (Some(c), 3));
        assert_eq!(w.cards.len(), 2);
        assert_eq!(w.cards[mine].column, Column::Working);
        // Only a worker takes a card so: a helper's turn makes none.
        let mut w = team(1);
        let mut live = idle_live(3);
        let h = with_helper(&mut w, 1);
        let c = with_card(&mut w, 1);
        live.prompt(&mut w, h, "go".into(), Some(open(Some(c))))
            .unwrap();
        assert_eq!(w.agents[h].card, None);
        // A card at work but no longer its own is not unfinished: a new
        // card, its first try.
        let mut w = team(2);
        let mut live = idle_live(3);
        let c = with_card(&mut w, 1);
        w.cards[c].owner = Some(2);
        w.agents[1].status = Status::Idle;
        live.prompt(&mut w, 1, "go".into(), Some(open(None)))
            .unwrap();
        assert_eq!(w.cards.len(), 2);
        assert_eq!((w.agents[1].card, w.agents[1].attempt), (Some(1), 1));
        assert_eq!((w.cards[1].attempt, w.cards[1].title.as_str()), (1, "next"));
        // Nor is one whose change waits, or one that failed.
        for column in [Column::Review, Column::Failed, Column::Done] {
            let mut w = team(1);
            let mut live = idle_live(2);
            let c = with_card(&mut w, 1);
            w.cards[c].column = column;
            w.agents[1].status = Status::Idle;
            live.prompt(&mut w, 1, "go".into(), Some(open(None)))
                .unwrap();
            assert_eq!(w.agents[1].card, Some(1), "{column:?}");
        }
        // Its own card at work goes on: no new one.
        let mut w = team(1);
        let mut live = idle_live(2);
        let c = with_card(&mut w, 1);
        w.agents[1].status = Status::Idle;
        live.prompt(&mut w, 1, "go".into(), Some(open(None)))
            .unwrap();
        assert_eq!((w.cards.len(), w.agents[1].card), (1, Some(c)));
        // Mid-turn, a prompt opens no card.
        let mut w = team(1);
        let mut live = idle_live(2);
        live.prompt(&mut w, 1, "go".into(), Some(open(None)))
            .unwrap();
        assert_eq!((w.cards.len(), w.agents[1].card), (0, None));
    }

    #[test]
    fn the_tools_that_write_a_file_are_edit_write_and_multiedit() {
        for name in ["Edit", "Write", "MultiEdit"] {
            assert!(writes(name), "{name}");
        }
        for name in ["Read", "Bash", "NotebookEdit", "Grep", "edit", ""] {
            assert!(!writes(name), "{name}");
        }
    }
}
