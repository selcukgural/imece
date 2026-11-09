//! What is remembered between runs: the store, the wake briefing,
//! resuming sessions, and forgetting them.

use super::*;
use crate::recall::{Hit, Line};

impl Live {
    /// What earlier runs left in the store, if anything was remembered.
    pub fn left_off(&self) -> Option<LeftBehind> {
        self.left_off
    }

    /// The last word to the store when imece quits in order: when this run
    /// ended, for the next start's "where you left off". A failure is
    /// nobody's business now; the file then says when it last changed.
    pub fn leave(&mut self) {
        if let Some(store) = &mut self.store {
            let _ = store.leave(now());
        }
    }

    #[cfg(test)]
    pub(crate) fn set_left_off(&mut self, left: LeftBehind) {
        self.left_off = Some(left);
    }

    /// When the store was last written, as it says, for tests.
    #[cfg(test)]
    pub(crate) fn saved_at(&self) -> Option<u64> {
        self.store.as_ref().and_then(|s| {
            s.raw()
                .query_row("SELECT value FROM meta WHERE key = 'saved_at'", [], |r| {
                    r.get::<_, String>(0)
                })
                .ok()
                .and_then(|v| v.parse().ok())
        })
    }

    /// Project `p`'s journal lines, oldest first, and which are marked
    /// (by index in them), for an archive's read-only view: all the store
    /// keeps of it, which is more than the newest lines `w` loaded;
    /// without a store, `w`'s.
    pub fn project_journal(
        &self,
        w: &World,
        p: ProjectId,
    ) -> Result<(Vec<Entry>, std::collections::BTreeSet<usize>), String> {
        match &self.store {
            Some(s) => s.project_journal(p).map_err(|e| e.to_string()),
            None => Ok(w.journal_of(p)),
        }
    }

    /// `:recall`: the journal and trace lines of every project and
    /// archive that have the terms of `words`, the newest first and at
    /// most `cap`, from the store's index (record 93); a journal line
    /// says where it is in `w`'s journal only when the line there is the
    /// one found. Without a store, what `w` holds.
    pub fn recall(&self, w: &World, words: &str, cap: usize) -> Result<Vec<Hit>, String> {
        let Some(store) = &self.store else {
            return Ok(crate::recall::in_world(w, words, cap));
        };
        let mut hits = store.recall(words, cap).map_err(|e| e.to_string())?;
        for h in &mut hits {
            if let Line::Journal { entry, at } = &mut h.line
                && at.is_some_and(|i| w.journal.get(i) != Some(&*entry))
            {
                *at = None;
            }
        }
        Ok(hits)
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
            s.raw()
                .execute_batch("DROP TABLE agents; DROP TABLE journal")
                .unwrap();
        }
    }

    /// Takes in what the store remembers of earlier runs, beside the world
    /// it loaded: every agent sleeps (no session, nothing spent until
    /// something wakes it), its mail, notes and offers wait, a turn that
    /// was cut short is marked on its card and told at its wake. A project
    /// whose folder is gone closes, and so does a team whose folder is no
    /// git checkout with a first commit any more; a solo there works
    /// straight in its folder, which the journal says. `skipped` lines of
    /// the journal or a story could not be read.
    pub(super) fn adopt(&mut self, w: &mut World, found: Found, skipped: usize) {
        for repo in &found.repos {
            let root = root_for(&self.state, repo);
            // Found again at each start: a folder may have got its first
            // commit since, or lost its git.
            let git = own_git(repo);
            self.places.push(Place {
                repo: repo.clone(),
                root,
                git,
            });
        }
        w.close_scripts();
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
            self.forks[id] = found.forks.get(id).copied().flatten();
            // The session that asked is gone with the process before.
            w.close_approvals(id);
            if w.agents[id].gone {
                continue;
            }
            let t = w.tick;
            w.agents[id].set_status(Status::Asleep, t);
            self.remembered[id] = true;
            let cut = found.inflight.get(id).cloned().unwrap_or_default();
            if w.in_archive(id) {
                // Kept as it was archived, and said when it is brought
                // back.
                self.inflight[id] = cut;
                continue;
            }
            if !cut.is_empty() {
                self.interrupted[id] = cut;
                // Its session ended mid-turn: no figure says what that
                // turn cost (record 132).
                w.agents[id].unpriced.gaps += 1;
                let card = w.agents[id].card;
                w.agents[id].trace(w.tick, "turn cut short when imece closed");
                w.story(card, StoryEvent::Interrupted);
                w.note(Event::Interrupted { agent: id });
            }
        }
        if self.fresh {
            for id in 0..w.agents.len() {
                // An archive keeps its sessions to go on with when it is
                // brought back.
                if self.sessions[id].is_none() || w.in_archive(id) {
                    continue;
                }
                w.agents[id].trace(
                    w.tick,
                    "started with --fresh: the session is forgotten, a new one is told where \
                     it left off",
                );
                self.forget(w, id, false);
                self.drop_scratch(w, id);
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
                runs: found.runs.get(&id).cloned().unwrap_or_default(),
                check: found.checks.get(&id).cloned().unwrap_or_default(),
                spend: found.spends.get(&id).copied(),
            });
        }
        w.close_empty_changes();
        // A waiting change's review, as the journal kept it.
        w.reviews_from_journal();
        self.changes = found.changes;
        // A check that was asked about or running when the earlier run
        // ended is asked about or run again (its ask closed above).
        let unchecked: Vec<AgentId> = (w.changes.iter())
            .filter(|c| matches!(c.check, Check::Asked | Check::Running))
            .map(|c| c.agent)
            .collect();
        for id in unchecked {
            w.checked(id, Check::None);
            self.check_change(w, id);
        }
        self.loaded_cards = w.cards.len();
        let gone: Vec<ProjectId> = w
            .open_projects()
            .filter(|q| {
                self.places
                    .get(q.id)
                    .is_none_or(|pl| !pl.repo.is_dir() || (!pl.git && !w.is_solo(q.id)))
            })
            .map(|q| q.id)
            .collect();
        for p in gone {
            self.posts.retain(|c, _| w.cards[*c].project != p);
            self.offered.retain(|c, _| w.cards[*c].project != p);
            self.changes.retain(|a, _| w.agents[*a].project != p);
            w.close_project(p);
        }
        let bare: Vec<(ProjectId, String)> = (w.open_projects())
            .filter_map(|q| self.places.get(q.id).map(|pl| (q.id, pl)))
            .filter(|(_, pl)| !pl.git)
            .map(|(p, pl)| (p, pl.repo.display().to_string()))
            .collect();
        for (p, folder) in bare {
            w.set_no_git(p, Some(&folder));
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
            forks: &self.forks,
            posts: &self.posts,
            offered: &self.offered,
            changes: &self.changes,
            checks: (w.changes.iter())
                .map(|c| (c.agent, c.check.clone()))
                .collect(),
            spends: (w.changes.iter())
                .filter_map(|c| c.spend.map(|s| (c.agent, s)))
                .collect(),
        };
        store.save(w, &kept).map_err(|e| e.to_string())
    }

    /// Tells a remembered agent where it left off, ahead of its first
    /// prompt this run. A session that goes on (`resumed`) knows its own
    /// story and hears only what happened since: a turn that was cut
    /// short, mail that waits. A new session knows nothing of the old
    /// one and is told from what the world kept (its card's story, the
    /// cut turn, mail, its worktree's changes, the board). Every line
    /// cleaned and the whole capped. What a worker's or helper's worktree
    /// holds is read with its session, off the screen's loop: the second
    /// value is where that line goes in the briefing, room kept for it.
    pub(super) fn brief(
        &mut self,
        w: &World,
        id: AgentId,
        resumed: bool,
    ) -> (String, Option<usize>) {
        let a = &w.agents[id];
        if let Some(from) = self.forks[id].filter(|_| resumed) {
            let (me, from) = (&a.name, &w.agents[from].name);
            let text = format!(
                "You are {me}, a fork of {from}: this session goes on from {from}'s, so you \
                 remember everything {from} did and was told. From now on you are {me}, a \
                 worker of your own: you work in a worktree of your own, which started with \
                 {from}'s uncommitted change, and {from} goes on in its own. The person forked \
                 you to try another way; what follows says which."
            );
            return (cut(&text, MAX_BRIEFING).to_string(), None);
        }
        let mut lines = vec![if resumed {
            "imece started again since your last turn; this session goes on where it was, \
             so you remember everything up to then. Since then:"
                .to_string()
        } else {
            "imece started again since your last turn, so this is a new session that \
             remembers nothing of the old one. Where you left off:"
                .to_string()
        }];
        let mut line = |text: String| lines.push(briefing_line(&text));
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
            return (cut(&lines.join("\n"), MAX_BRIEFING).to_string(), None);
        }
        if let Some(c) = a.card {
            let card = &w.cards[c];
            // Where it stands now (record 126); one handed to another is
            // not this agent's to finish.
            let state = match (card.column, card.owner) {
                (_, Some(o)) if o != id => format!("handed to {}", w.agents[o].name),
                (Column::Done, _) => "done".to_string(),
                (Column::Review, _) => "done, its change waits for the person".to_string(),
                (Column::Verifying, _) => "in Lonca's merge queue".to_string(),
                (Column::Failed, _) => "failed".to_string(),
                (Column::Withdrawn, _) => {
                    "withdrawn by the leader; a done for it closes nothing".to_string()
                }
                _ => "not finished".to_string(),
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
        // What waits on other cards (record 126, F10): an assign held for
        // this agent, and, for a leader, its project's cards.
        let held_for = (self.mail[id].iter())
            .filter(|m| Self::waits(w, m))
            .filter_map(|m| m.card);
        for c in held_for {
            line(format!(
                "#{} {} is yours once what it waits on is done ({}); its assign comes then.{}",
                c + 1,
                w.cards[c].title,
                words::card_refs(&w.waits_on(c)),
                blocked_words(w, c)
            ));
        }
        if a.role == Role::Leader {
            let waiting: Vec<CardId> = (w.cards.iter())
                .filter(|c| c.project == a.project && !c.over() && !w.ready(c.id))
                .map(|c| c.id)
                .collect();
            if !waiting.is_empty() {
                line(format!("{} cards wait on others:", waiting.len()));
            }
            for &c in waiting.iter().take(10) {
                line(format!(
                    "  #{} {} waits on {}.{}",
                    c + 1,
                    w.cards[c].title,
                    words::card_refs(&w.waits_on(c)),
                    blocked_words(w, c)
                ));
            }
        }
        drop(line);
        let held = (a.role != Role::Leader).then(|| lines.join("\n").len());
        let mut line = |text: String| lines.push(briefing_line(&text));
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
        drop(line);
        // The worktree's line may come: room for it.
        let room = MAX_BRIEFING - held.map_or(0, |_| MAX_BRIEFING_LINE + 3);
        let text = cut(&lines.join("\n"), room).to_string();
        let held = held.map(|at| at.min(text.len()));
        (text, held)
    }

    /// Agent `id`'s session is forgotten: its next prompt starts a new
    /// one, and its transcript goes, on a thread of its own; the trace says
    /// how it went. `settle` first gives the session's process, just asked
    /// to stop, time to be gone.
    pub(super) fn forget(&mut self, w: &World, id: AgentId, settle: bool) {
        let Some(session) = self.sessions[id].take().map(|r| r.session) else {
            return;
        };
        let cwd = self.cwd_of(w, id);
        let claude_home = self.claude_home.clone();
        let wait = if settle { self.settle } else { Duration::ZERO };
        let tx = self.tx.clone();
        let generation = self.generation[id];
        thread::spawn(move || {
            thread::sleep(wait);
            let note = match claude::forget(&claude_home, &cwd, &session) {
                Ok(true) => format!("forgot session {session}: its transcript was removed"),
                Ok(false) => format!("forgot session {session}: it had no transcript left"),
                Err(e) => format!("could not remove its transcript of session {session}: {e}"),
            };
            let _ = tx.send((id, generation, AgentEvent::Note(note)));
        });
    }

    /// Whether agent `id` has a session to forget, running or remembered.
    pub fn can_forget(&self, w: &World, id: AgentId) -> Result<(), Refusal> {
        let running = matches!(self.slots[id], Slot::Running(_));
        if w.agents[id].gone || (!running && self.sessions[id].is_none()) {
            return Err(Refusal::Command("it has no session to forget".into()));
        }
        Ok(())
    }

    /// The person made agent `id` forget its session (`:forget`): the one
    /// running ends (its unfinished prompts are told at the wake, not run
    /// again), the one remembered goes with its transcript, and the
    /// next message starts a new session, told where it left off. An
    /// agent with no session to forget is refused.
    pub fn forget_agent(&mut self, w: &mut World, id: AgentId) -> Result<(), Refusal> {
        self.can_forget(w, id)?;
        let running = matches!(self.slots[id], Slot::Running(_));
        self.running[id].kill_all();
        if let Slot::Running(s) = &mut self.slots[id] {
            s.terminate();
        }
        let cut = std::mem::take(&mut self.inflight[id]);
        self.end(w, id);
        self.forget(w, id, running);
        self.drop_scratch(w, id);
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
    /// the session it had: its id is shaped like one and the session's
    /// transcript is still where Claude Code keeps it for the directory
    /// the agent runs in. Said once, at the wake; a session that is gone
    /// is forgotten.
    pub(super) fn resumable(&mut self, w: &World, id: AgentId) -> Option<Resumable> {
        // A fork goes on from its worker's session as it is at the wake,
        // found where that worker runs; one that is gone is no fork.
        if let Some(from) = self.forks[id] {
            let r = self.sessions[from].clone();
            let there = r.as_ref().is_some_and(|r| {
                imece_agent::valid_uuid(&r.session)
                    && claude::transcript(&self.claude_home, &self.cwd_of(w, from), &r.session)
                        .is_file()
            });
            if !there {
                self.forks[id] = None;
            }
            return r.filter(|_| there);
        }
        let r = self.sessions[id].clone()?;
        let there = imece_agent::valid_uuid(&r.session)
            && claude::transcript(&self.claude_home, &self.cwd_of(w, id), &r.session).is_file();
        if !there {
            self.sessions[id] = None;
        }
        there.then_some(r)
    }

    /// The session agent `id`'s next process goes on with, if its wake
    /// asked for one: the old session above what it had cost.
    pub(super) fn resume_of(&self, id: AgentId) -> Option<Resumable> {
        self.resuming[id].as_ref().map(|r| Resumable {
            session: r.session.clone(),
            spent: self.floor[id],
            models: r.models.clone(),
        })
    }
}

/// One line of a briefing: `text`'s first line, cleaned and capped.
fn briefing_line(text: &str) -> String {
    let text = clean(text.lines().next().unwrap_or_default());
    format!("- {}", cut(&text, MAX_BRIEFING_LINE))
}

/// The briefing's line on what a worker's worktree holds.
pub(super) fn holds(c: &worktree::Change) -> String {
    let (added, removed) = c.counts();
    briefing_line(&format!(
        "Your worktree holds changes the person has not applied: {} files, \
         +{added} −{removed}: {}",
        c.files.len(),
        c.files
            .iter()
            .take(20)
            .map(|f| f.path.as_str())
            .collect::<Vec<_>>()
            .join(", ")
    ))
}

/// What holds card `c` back for good or until handed again, as a
/// briefing says it after what the card waits on: nothing when nothing
/// does.
fn blocked_words(w: &World, c: CardId) -> String {
    match w.blocked_on(c) {
        None => String::new(),
        Some(f) if w.cards[f].column == Column::Withdrawn => {
            format!(" #{} was withdrawn: it never begins.", f + 1)
        }
        Some(f) => format!(
            " #{} failed: it begins only once that is handed again and done.",
            f + 1
        ),
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;
    use std::sync::atomic::{AtomicBool, Ordering};

    use super::*;
    use crate::live::testing::*;

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
            Role::Leader,
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
            paths: Vec::new(),
            checks: Vec::new(),
            started: 0,
            finished: 0,
            attempt: 1,
            breaker: Default::default(),
            after: Vec::new(),
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
        let ran = Ran {
            line: "cargo test".into(),
            code: Some(0),
            timed_out: false,
        };
        // The review of that change, as the journal kept it.
        w.note(Event::Reviewed {
            agent: 1,
            model: "default".into(),
            patch: patch_sum("diff"),
            text: "1. a.txt: no test".into(),
            cost: 0,
        });
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
                    id: 0,
                    re: None,
                    paths: Vec::new(),
                    checks: Vec::new(),
                    card: None,
                    outcome: Outcome::Ok,
                }],
                Vec::new(),
                Vec::new(),
            ],
            notes: vec![vec!["a note".into()], Vec::new(), Vec::new(), Vec::new()],
            forks: Vec::new(),
            sessions: vec![
                Some(Resumable {
                    session: "686c6d27-d50b-4ae2-ad6a-159bf2984671".into(),
                    spent: 524,
                    models: None,
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
            runs: HashMap::from([(1, vec![ran.clone()])]),
            checks: HashMap::from([(1, Check::Running), (lead_gone, Check::Asked)]),
            spends: HashMap::from([(
                1,
                Spend {
                    cost: 8_400,
                    ticks: 144,
                    rounds: 2,
                    unpriced: false,
                },
            )]),
        };
        let mut live = Live::bare(claude_at("x"));
        live.state = state.clone();
        // What a session asked went with the process.
        w.asks_you(1, "Bash: make");
        let journal = w.journal.len();
        live.adopt(&mut w, found, 3);
        // A check that was running or asked about when the earlier run
        // ended is looked at again: no script in the checkout, none.
        assert_eq!(w.change_of(1).unwrap().check, Check::None);
        assert!(
            w.inbox
                .iter()
                .all(|i| i.kind != InboxKind::Check || !i.open)
        );
        assert!(!w.has_approval(1));
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
        // What the cut turn cost no figure will say (record 132).
        assert_eq!(
            w.agents.iter().map(|a| a.unpriced.gaps).collect::<Vec<_>>(),
            [0, 1, 0, 0]
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
        assert_eq!(w.changes[0].runs, [ran]);
        assert_eq!(
            w.review_of(1).map(|r| r.text.as_str()),
            Some("1. a.txt: no test"),
            "its review back from the journal"
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
                id: 0,
                re: None,
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
                id: 0,
                re: None,
                paths: Vec::new(),
                checks: Vec::new(),
                card: None,
                outcome: Outcome::Ok,
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
        // A finished card is said to be, and the person's new line waits
        // as the worker's own task, with no card until a change shows
        // (record 126).
        let mut w = team(1);
        let c = with_card(&mut w, 1);
        w.cards[c].column = Column::Done;
        let mut live = idle_live(2);
        live.remembered[1] = true;
        w.agents[1].status = Status::Asleep;
        live.told(&mut w, 1, "next").unwrap();
        assert!(queued(&live, 1)[0].contains("#1 fix it (done)"));
        assert_eq!(w.agents[1].card, None);
        assert_eq!(w.cards.len(), 1);
        assert_eq!(w.agents[1].task.own.as_ref().unwrap().title, "next");
        // Where its card stands now (record 126).
        for (column, owner, said) in [
            (Column::Review, 1, "(done, its change waits for the person)"),
            (Column::Verifying, 1, "(in Lonca's merge queue)"),
            (Column::Failed, 1, "(failed)"),
            (
                Column::Withdrawn,
                1,
                "(withdrawn by the leader; a done for it closes nothing)",
            ),
            (Column::Working, 1, "(not finished)"),
            (Column::Working, 0, "(handed to lead)"),
            (Column::Done, 0, "(handed to lead)"),
        ] {
            let mut w = team(1);
            let c = with_card(&mut w, 1);
            (w.cards[c].column, w.cards[c].owner) = (column, Some(owner));
            let mut live = idle_live(2);
            live.remembered[1] = true;
            w.agents[1].status = Status::Asleep;
            live.told(&mut w, 1, "next").unwrap();
            let text = &queued(&live, 1)[0];
            assert!(
                text.contains(&format!("#1 fix it {said}")),
                "{column:?}: {text}"
            );
        }
    }

    #[test]
    fn a_remembered_worker_is_told_what_its_worktree_holds() {
        use std::os::unix::fs::PermissionsExt;
        let repo = checkout("brief-tree");
        let root = trees("brief-tree");
        // A `claude` that keeps what it is given.
        let heard = root.with_extension("heard");
        let program = root.with_extension("claude");
        std::fs::write(
            &program,
            format!("#!/bin/sh\ncat > '{}'\n", heard.display()),
        )
        .unwrap();
        std::fs::set_permissions(&program, std::fs::Permissions::from_mode(0o700)).unwrap();
        let mut w = team(1);
        let mut live = Live::new(
            2,
            claude_at(&program.to_string_lossy()),
            repo.clone(),
            root.clone(),
        );
        let tree = worktree::ensure(&repo, &root, "claude-1", None)
            .unwrap()
            .path;
        std::fs::write(tree.join("a.txt"), "changed\n").unwrap();
        std::fs::write(tree.join("b.txt"), "new\n").unwrap();
        live.post(&mut w, 0, &json!({"title": "write docs", "body": "b"}))
            .unwrap();
        live.mail[1].push(Mail {
            from: 0,
            kind: MsgKind::Inform,
            body: "x".into(),
            id: 0,
            re: None,
            paths: Vec::new(),
            checks: Vec::new(),
            card: None,
            outcome: Outcome::Ok,
        });
        live.remembered[1] = true;
        w.agents[1].status = Status::Asleep;
        w.agents[1].card = None;
        live.told(&mut w, 1, "go on").unwrap();
        // Git is not asked on the screen's loop: the line comes with the
        // session, where the briefing kept its place.
        assert!(!queued(&live, 1)[0].contains("worktree"));
        let line = "\n- 1 messages wait for you; they come with a turn of their own.\
                    \n- Your worktree holds changes the person has not applied: 2 files, \
                    +2 −1: a.txt, b.txt\
                    \n- The board has 1 open cards:";
        let mut given = String::new();
        for _ in 0..500 {
            live.step(&mut w);
            given = std::fs::read_to_string(&heard)
                .unwrap_or_default()
                .replace("\\n", "\n");
            if given.contains("go on") {
                break;
            }
            thread::sleep(Duration::from_millis(10));
        }
        assert!(given.contains(line), "{given}");
        assert!(live.inflight[1][0].contains(line), "{:?}", live.inflight[1]);
        live.dismiss(&mut w, 1);
        // The leader has no worktree, and a session that goes on knows
        // its own: no place for the line.
        assert_eq!(live.brief(&w, 0, false).1, None);
        assert_eq!(live.brief(&w, 1, true).1, None);
        let _ = std::fs::remove_dir_all(&repo);
        let _ = std::fs::remove_dir_all(&root);
        let _ = std::fs::remove_file(&heard);
        let _ = std::fs::remove_file(&program);
    }

    #[test]
    fn the_worktree_s_line_goes_into_the_first_prompt_as_given_and_as_remembered() {
        let mut w = team(1);
        let mut live = idle_live(2);
        live.slots[1] = Slot::Starting(vec!["a\nb".into(), "cd".into()]);
        live.inflight[1] = vec!["a\nb".into(), "cd".into()];
        let held = Some((1, "- holds".to_string()));
        live.ready(
            &mut w,
            1,
            Ok((
                Box::new(Stub),
                PathBuf::from("/w"),
                None,
                held,
                imece_agent::copy::Brought::default(),
                setup::Done::None,
                None,
                Moved::Stayed,
            )),
        );
        assert_eq!(live.inflight[1], ["a\n- holds\nb", "cd"]);
        // Not at a character's boundary: left out rather than broken.
        let mut live = idle_live(2);
        live.slots[1] = Slot::Starting(vec!["é".into()]);
        let held = Some((1, "- holds".to_string()));
        live.ready(
            &mut w,
            1,
            Ok((
                Box::new(Stub),
                PathBuf::from("/w"),
                None,
                held,
                imece_agent::copy::Brought::default(),
                setup::Done::None,
                None,
                Moved::Stayed,
            )),
        );
        assert!(matches!(live.slots[1], Slot::Running(_)));
    }

    #[test]
    fn a_briefing_says_what_waits_on_other_cards_and_what_holds_it_back() {
        let mut w = team(2);
        let mut live = idle_live(3);
        let base = with_card(&mut w, 2);
        let mut args = json!({ "to": "claude-1", "type": "assign", "body": "Tests" });
        args["after"] = json!([base + 1]);
        let (tx, _rx) = std::sync::mpsc::channel();
        live.send(&mut w, 0, &args, tx);
        let held = w.cards.len() - 1;
        let (text, _) = live.brief(&w, 1, false);
        assert!(
            text.contains(
                "#2 Tests is yours once what it waits on is done (#1); its assign comes then."
            ),
            "{text}"
        );
        let (text, _) = live.brief(&w, 0, false);
        assert!(
            text.contains("1 cards wait on others:\n-   #2 Tests waits on #1."),
            "{text}"
        );
        // What holds it back: a failed card, then a withdrawn one.
        w.cards[base].column = Column::Failed;
        let (text, _) = live.brief(&w, 1, false);
        assert!(
            text.contains("its assign comes then. #1 failed: it begins only once that is handed again and done."),
            "{text}"
        );
        w.cards[base].column = Column::Withdrawn;
        let (text, _) = live.brief(&w, 0, false);
        assert!(
            text.contains("waits on #1. #1 was withdrawn: it never begins."),
            "{text}"
        );
        // Ready, or through: nothing said; a worker hears no list.
        w.cards[base].column = Column::Done;
        let (text, _) = live.brief(&w, 0, false);
        assert!(!text.contains("wait on others"), "{text}");
        let (text, _) = live.brief(&w, 1, false);
        assert!(!text.contains("is yours once"), "{text}");
        w.cards[base].column = Column::Working;
        w.cards[held].column = Column::Withdrawn;
        let (text, _) = live.brief(&w, 0, false);
        assert!(!text.contains("wait on others"), "{text}");
        let (text, _) = live.brief(&w, 2, false);
        assert!(!text.contains("wait on others"), "a worker: {text}");
        assert_eq!(blocked_words(&w, base), "");
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
        // A worker's keeps room for its worktree's line, which goes where
        // the cut ended; a leader's does not.
        let long: Vec<String> = (0..40)
            .map(|i| format!("{i} {}", "x".repeat(500)))
            .collect();
        live.interrupted[1].clone_from(&long);
        let (text, at) = live.brief(&w, 1, false);
        assert_eq!(text.chars().count(), 6000 - 303);
        assert_eq!(at, Some(text.len()));
        live.interrupted[0] = long;
        let (text, at) = live.brief(&w, 0, false);
        assert_eq!((text.chars().count(), at), (6000, None));
        assert_eq!(cut("héllo", 2), "hé");
        assert_eq!(cut("ab", 5), "ab");
        assert_eq!(cut("", 0), "");
    }

    #[test]
    fn a_worker_told_again_goes_on_with_an_unfinished_card_and_a_new_line_opens_one_only_with_a_change()
     {
        let mut w = team(1);
        let mut live = idle_live(2);
        let c = with_card(&mut w, 1);
        w.agents[1].status = Status::Stopped;
        live.told(&mut w, 1, "try again").unwrap();
        assert_eq!(w.agents[1].card, Some(c));
        assert_eq!(w.cards.len(), 1, "the stopped task goes on");
        assert_eq!(w.agents[1].task.own, None, "its card holds the line");
        // Its card done, the person's next line opens no card (record
        // 126): it waits as the worker's own task.
        w.cards[c].column = Column::Done;
        w.agents[1].status = Status::Idle;
        live.turns[1] = 0;
        w.tick = 40;
        live.told(&mut w, 1, "are you there?\nAnswer in one word.")
            .unwrap();
        assert_eq!(w.cards.len(), 1, "no card for a line");
        assert_eq!(w.agents[1].card, None, "the done card is not its own now");
        let own = |w: &World| w.agents[1].task.own.clone();
        assert_eq!(
            own(&w),
            Some(Own {
                title: "are you there?".into(),
                tick: 40,
            })
        );
        // Its turn ends with an answer and nothing to show: no card ever.
        w.agents[1].status = Status::Idle;
        live.turns[1] = 0;
        // A done with nothing in its worktree: still none, and the line
        // is through.
        w.change_none(1);
        assert_eq!(w.cards.len(), 1);
        assert_eq!(own(&w), None);
        // The next line waits; one more while it waits is part of it.
        w.tick = 50;
        live.told(&mut w, 1, "fix the footer").unwrap();
        w.tick = 52;
        live.told(&mut w, 1, "and its test").unwrap();
        assert_eq!(own(&w).unwrap().title, "fix the footer");
        assert_eq!(w.cards.len(), 1);
        // A change of it shows: its card opens now, titled by the first
        // line, the worker's, from when it was written, and waits with
        // the change.
        w.tick = 60;
        w.change_ready(Change {
            agent: 1,
            title: "Footer fixed".into(),
            files: Vec::new(),
            patch: String::new(),
            runs: Vec::new(),
            check: Check::None,
            spend: None,
        });
        assert_eq!(w.cards.len(), 2);
        let card = &w.cards[1];
        assert_eq!(card.title, "fix the footer");
        assert_eq!(
            (card.owner, card.column, card.attempt, card.started),
            (Some(1), Column::Review, 1, 50)
        );
        assert!(matches!(card.story[0].event, StoryEvent::Took { agent: 1 }));
        assert_eq!(w.agents[1].card, Some(1));
        assert_eq!(own(&w), None, "the card holds it now");
        assert_eq!(w.inbox.last().unwrap().card, Some(1));
    }

    #[test]
    fn a_word_to_a_worker_whose_change_waits_goes_on_with_its_card() {
        let mut w = team(1);
        let mut live = idle_live(2);
        let c = with_card(&mut w, 1);
        w.change_ready(Change {
            agent: 1,
            title: "find by tag".into(),
            files: Vec::new(),
            patch: String::new(),
            runs: Vec::new(),
            check: Check::None,
            spend: None,
        });
        w.agents[1].status = Status::Idle;
        assert_eq!(w.cards[c].column, Column::Review);
        live.told(&mut w, 1, "also mention tags in the help")
            .unwrap();
        assert_eq!(w.cards.len(), 1, "no card of its own");
        assert_eq!(w.agents[1].card, Some(c));
        assert_eq!(w.cards[c].column, Column::Working, "back at work");
        assert_eq!(w.cards[c].breaker.sent_back, 0, "not a send-back");
        assert!(w.change_of(1).is_some(), "the change still waits");
        // Its next change takes the card to review again.
        w.change_ready(Change {
            agent: 1,
            title: "find by tag, with help".into(),
            files: Vec::new(),
            patch: String::new(),
            runs: Vec::new(),
            check: Check::None,
            spend: None,
        });
        assert_eq!(w.cards[c].column, Column::Review);
        // The leader's helper or a solo has no card to go on with; a
        // worker with nothing waiting gets none for a line (record 126),
        // only once a change of it shows.
        assert!(w.word_goes_on(0).is_empty());
        w.change_applied(1, "m".into(), true);
        assert_eq!(w.cards[c].column, Column::Done);
        assert!(w.word_goes_on(1).is_empty());
        w.agents[1].status = Status::Idle;
        live.turns[1] = 0;
        live.told(&mut w, 1, "next").unwrap();
        assert_eq!(w.cards.len(), 1);
        assert!(w.agents[1].task.own.is_some());
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
            id: 0,
            re: None,
            paths: Vec::new(),
            checks: Vec::new(),
            card: None,
            outcome: Outcome::Ok,
        });
        live.step(&mut w);
        assert_eq!(live.turns[1], 0, "mail that waited through does not wake");
        assert_eq!(w.agents[1].status, Status::Asleep);
        // Mail that does not wake an idle agent does not wake a sleeper.
        sent(&mut live, &mut w, 0, &msg("claude-2", "inform", "fyi")).unwrap();
        assert_eq!(w.agents[2].status, Status::Asleep);
        live.step(&mut w);
        assert_eq!(live.turns[2], 0);
        // New mail wakes, and the old rides along.
        let answer = sent(&mut live, &mut w, 0, &msg("claude-1", "assign", "new")).unwrap();
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
    fn an_archives_journal_is_read_from_the_store_else_from_the_world() {
        let mut w = team(1);
        let mut live = idle_live(2);
        w.note(Event::Failed { agent: 1 });
        assert_eq!(live.project_journal(&w, 0), Ok(w.journal_of(0)));
        live.store_in_memory();
        live.persist(&w).unwrap();
        let saved = w.journal_of(0);
        assert!(!saved.0.is_empty());
        // What only the world has is not the store's.
        w.note(Event::Failed { agent: 0 });
        assert_eq!(live.project_journal(&w, 0), Ok(saved));
        live.break_store();
        let e = live.project_journal(&w, 0).unwrap_err();
        assert!(e.contains("no such table"), "{e}");
    }

    #[test]
    fn recall_reads_the_store_and_says_where_a_line_is_only_when_it_is_that_line() {
        let mut w = team(1);
        let mut live = idle_live(2);
        let said = |text: &str| Event::Replied {
            agent: 1,
            text: text.into(),
        };
        w.journal.push(Entry {
            tick: 4,
            event: said("the parser is fixed"),
        });
        let at = w.journal.len() - 1;
        // No store: what the world holds.
        let hits = live.recall(&w, "parser", 10).unwrap();
        assert_eq!(hits, crate::recall::in_world(&w, "parser", 10));
        assert_eq!(hits.len(), 1);
        live.store_in_memory();
        live.persist(&w).unwrap();
        // Only in the store: what the world does not hold any more.
        let journal = std::mem::take(&mut w.journal);
        let hits = live.recall(&w, "parser", 10).unwrap();
        assert_eq!(hits.len(), 1, "from the store");
        assert!(matches!(hits[0].line, Line::Journal { at: None, .. }));
        w.journal = journal;
        let hits = live.recall(&w, "parser", 10).unwrap();
        assert!(matches!(hits[0].line, Line::Journal { at: Some(i), .. } if i == at));
        // Another line where the found one should be: not that line.
        w.journal[at].event = said("something else");
        let hits = live.recall(&w, "parser", 10).unwrap();
        assert!(matches!(hits[0].line, Line::Journal { at: None, .. }));
        assert_eq!(live.recall(&w, "parser", 0).unwrap(), [], "the cap");
        live.break_store();
        assert!(live.recall(&w, "parser", 10).is_err());
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
            tiers: Vec::new(),
            budget,
            fresh: false,
            run_programs: Vec::new(),
            run_timeout: DEFAULT_RUN_TIMEOUT,
            instructions: None,
            own_notes: None,
            confine: Confine::Standard,
            solo: None,
            claude: None,
        };
        let (mut w, mut live) = world(options(&repo, None)).unwrap();
        assert_eq!(w.projects.len(), 1);
        assert_eq!(w.agents[0].status, Status::Idle, "new, not remembered");
        // The standard level says so once, on the project imece runs in.
        let said = Event::Confine {
            leader: 0,
            level: "standard".into(),
        };
        assert_eq!(w.journal.iter().filter(|e| e.event == said).count(), 1);
        assert_eq!(live.confine, Confine::Standard);
        let line = words::event(Lang::En, &w, &said);
        assert!(
            line.starts_with("confine standard:") && line.contains("not imece's"),
            "{line}"
        );
        let open = words::event(
            Lang::Tr,
            &w,
            &Event::Confine {
                leader: 0,
                level: "open".into(),
            },
        );
        assert!(
            open.starts_with("confine open:") && open.contains("hook"),
            "{open}"
        );
        assert_eq!(live.left_off(), None, "nothing was remembered");
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
            models: None,
        });
        assert_eq!(live.persist(&w), Ok(true));
        let journal_before = w.journal.len();
        assert!(journal_before > 0, "the opening is journaled");
        drop(live);
        // As if the first run had ended 100 seconds ago.
        let file = state.join(store::FILE);
        rusqlite::Connection::open(&file)
            .unwrap()
            .execute(
                "UPDATE meta SET value = ?1 WHERE key = 'saved_at'",
                [(now() - 100).to_string()],
            )
            .unwrap();

        // The next run: asleep, the pot as the flag says, the turn cut.
        let (w, mut live) = world(options(&repo, Some(20_000))).unwrap();
        assert_eq!(w.projects.len(), 1);
        assert_eq!(w.agents[0].status, Status::Asleep);
        let left = live.left_off().expect("remembered");
        assert_eq!(left.journal, journal_before, "the earlier run's lines");
        assert!(
            (100..=102).contains(&left.ago.unwrap()),
            "100 seconds ago, not {:?}",
            left.ago
        );
        assert_eq!(
            w.journal[journal_before].event,
            Event::Interrupted { agent: 0 },
            "this start's lines come after"
        );
        // Leaving writes the time, and nothing else.
        rusqlite::Connection::open(&file)
            .unwrap()
            .execute("UPDATE meta SET value = '1' WHERE key = 'saved_at'", [])
            .unwrap();
        live.leave();
        let saved_at: String = live
            .store
            .as_ref()
            .unwrap()
            .raw()
            .query_row("SELECT value FROM meta WHERE key = 'saved_at'", [], |r| {
                r.get(0)
            })
            .unwrap();
        assert!(saved_at.parse::<u64>().unwrap().abs_diff(now()) <= 2);
        assert_eq!((w.agents[0].cost, live.spent_before[0]), (1_234, 1_234));
        assert_eq!(
            live.sessions[0],
            Some(Resumable {
                session: "686c6d27-d50b-4ae2-ad6a-159bf2984671".into(),
                spent: 524,
                models: None,
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
        assert_eq!(
            w.rate_per_hour(),
            0,
            "what earlier runs spent is not this run's rate"
        );
        assert!(
            !w.agents[0].trace.iter().any(|l| l.contains("skipped")),
            "nothing skipped, nothing said: {:?}",
            w.agents[0].trace
        );
        assert_eq!(live.persist(&w), Ok(true));
        drop(live);

        // Without the flag, the pot is as it was left; from another
        // checkout, that one is a project of its own and leads.
        let (w, live) = world(options(&other, None)).unwrap();
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
        let (_w, _live) = world(options(&repo, None)).unwrap();
        let e = world(options(&repo, None)).err().expect("refused");
        assert!(e.contains("another imece is running"), "{e}");
        let _ = std::fs::remove_dir_all(&repo);
        let _ = std::fs::remove_dir_all(&other);
        let _ = std::fs::remove_dir_all(&state);
    }

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
        worktree::ensure(&repo, &root, "claude-1", None).unwrap();
        live.sessions[1] = Some(Resumable {
            session: UUID.into(),
            spent: 524,
            models: None,
        });
        live.remembered[1] = true;
        live.interrupted[1] = vec!["finish it".into()];
        live.mail[1].push(Mail {
            from: 0,
            kind: MsgKind::Inform,
            body: "one".into(),
            id: 0,
            re: None,
            paths: Vec::new(),
            checks: Vec::new(),
            card: None,
            outcome: Outcome::Ok,
        });
        w.agents[1].status = Status::Asleep;
        if there {
            let file = claude::transcript(&live.claude_home, &live.cwd_of(&w, 1), UUID);
            std::fs::create_dir_all(file.parent().unwrap()).unwrap();
            std::fs::write(&file, "{}\n").unwrap();
        }
        (w, live, repo, root)
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
                models: None,
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
        assert_eq!(restarted(&w, 1).len(), 0);
        assert_eq!(journaled(&w, &Event::Resumed { agent: 1 }), 0);
        // What each model of it had cost goes with it (record 134).
        let had = imece_agent::ByModel::from([("a".into(), 300)]);
        live.resuming[1].as_mut().unwrap().models = Some(had.clone());
        assert_eq!(live.resume_of(1).unwrap().models, Some(had.clone()));
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
                models: Some(had),
            })
        );
        // A turn's figures by model: what a resume counts from next time,
        // and what grew for the ledger.
        live.tx
            .send((
                1,
                live.generation[1],
                AgentEvent::Models {
                    said: [("a".into(), 400), ("b".into(), 36)].into(),
                    grew: [("a".into(), 100), ("b".into(), 36)].into(),
                },
            ))
            .unwrap();
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
        assert_eq!(
            live.sessions[1].as_ref().unwrap().models,
            Some([("a".into(), 400), ("b".into(), 36)].into())
        );
        assert_eq!(w.agents[1].cost, 1, "136 millionths in ten-thousandths");
        // Booked with it: a's one whole ten-thousandth on the row, the rest waits.
        let row = w.agents[1].meter.row.unwrap();
        assert_eq!(w.ledger[row].models, [("a".into(), 1)].into());
        assert_eq!(w.agents[1].meter.models, [("b".into(), 36)].into());
        // Claude Code's /clear brings a new session id: that is the one to
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
                models: Some(imece_agent::ByModel::new()),
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
        assert_eq!(
            restarted(&w, 1),
            [Fresh::Refused(
                "stopped: the session could not resume session 686c… (error_during_execution)"
                    .into()
            )]
        );
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
        assert_eq!(restarted(&w, 1), [Fresh::Missing]);
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
                // A new session's figures by model start at nothing.
                models: Some(imece_agent::ByModel::new()),
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
            models: None,
        });
        assert_eq!(live.resumable(&w, 1), ok);
        assert_eq!(live.sessions[1], ok, "still remembered");
        // An id that is not a UUID never reaches a command line or a path.
        live.sessions[1] = Some(Resumable {
            session: "../x".into(),
            spent: 0,
            models: None,
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

    #[test]
    fn a_session_renewed_after_its_cap_forgets_the_capped_one() {
        let mut w = team(1);
        let mut live = idle_live(2);
        live.ready(
            &mut w,
            1,
            Ok((
                Box::new(Stub),
                PathBuf::from("/w"),
                None,
                None,
                imece_agent::copy::Brought::default(),
                setup::Done::None,
                None,
                Moved::Stayed,
            )),
        );
        live.told(&mut w, 1, "go").unwrap();
        live.claude_home = trees("renew-home");
        live.sessions[1] = Some(Resumable {
            session: UUID.into(),
            spent: 0,
            models: None,
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
    fn closing_a_project_removes_the_transcripts_of_its_agents() {
        let repo = checkout("forget-close-repo");
        let dir = trees("forget-close");
        std::fs::create_dir_all(&dir).unwrap();
        let mut w = team(0);
        let mut live = Live::new(
            1,
            Hireable::new("/nonexistent/claude", &[]),
            std::env::temp_dir(),
            dir.join("trees"),
        );
        let (cut, _) = mpsc::channel();
        live.ready_tx = cut;
        live.claude_home = dir.join("claude");
        live.open_bus(&w, "/opt/imece".into()).unwrap();
        assert_eq!(
            live.open_project(&mut w, &repo, Some("api"), Open::Team),
            Ok(1)
        );
        live.hire(&mut w, 1, &json!({})).unwrap();
        assert_eq!(w.agents[2].name, "claude-1");
        live.sessions[2] = Some(Resumable {
            session: UUID.into(),
            spent: 1,
            models: None,
        });
        let transcript = transcript_of(&live, &w, 2, UUID);
        // A leader that was never a session: nothing to forget, nothing said.
        live.settle = Duration::from_millis(400);
        assert_eq!(live.close_project(&mut w, 1), Ok(()));
        assert_eq!(live.sessions[2], None);
        thread::sleep(Duration::from_millis(100));
        assert!(
            transcript.is_file(),
            "the processes are given time to be gone first"
        );
        assert!(gone_soon(&transcript), "the worker's transcript");
        assert!(traced_soon(
            &mut live,
            &mut w,
            2,
            &format!("forgot session {UUID}: its transcript was removed")
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
        assert_eq!(restarted(&w, 1), [Fresh::NoSession]);
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
            models: None,
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
            Ok((
                Box::new(Ends(ended.clone())),
                PathBuf::from("/w"),
                None,
                None,
                imece_agent::copy::Brought::default(),
                setup::Done::None,
                None,
                Moved::Stayed,
            )),
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
            id: 0,
            re: None,
            paths: Vec::new(),
            checks: Vec::new(),
            card: None,
            outcome: Outcome::Ok,
        });
        assert_eq!(live.mail[1].len(), 1);
    }

    #[test]
    fn forgetting_removes_the_scratch_directory_and_nothing_is_said_without_one() {
        let (mut w, mut live, repo, root) = remembered_worker("forget-scratch", false);
        let dir = live.scratch_of(&w, 1).unwrap();
        std::fs::create_dir_all(dir.join("logs")).unwrap();
        std::fs::write(dir.join("logs/run.txt"), "x").unwrap();
        assert_eq!(live.forget_agent(&mut w, 1), Ok(()));
        assert!(!dir.exists(), "moved aside at once");
        assert!(traced_soon(
            &mut live,
            &mut w,
            1,
            "its scratch directory was removed"
        ));
        let parent = dir.parent().unwrap();
        assert_eq!(
            std::fs::read_dir(parent).unwrap().count(),
            0,
            "none left aside"
        );
        // Nothing there: nothing said.
        let before = w.agents[1].trace.len();
        live.drop_scratch(&w, 1);
        thread::sleep(Duration::from_millis(50));
        live.step(&mut w);
        assert!(
            !(w.agents[1].trace.iter().skip(before)).any(|l| l.contains("scratch")),
            "{:?}",
            w.agents[1].trace
        );
        // One that cannot be moved aside is said, and stays.
        use std::os::unix::fs::PermissionsExt;
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::set_permissions(parent, std::fs::Permissions::from_mode(0o500)).unwrap();
        live.drop_scratch(&w, 1);
        let said = traced_soon(
            &mut live,
            &mut w,
            1,
            "could not remove its scratch directory",
        );
        std::fs::set_permissions(parent, std::fs::Permissions::from_mode(0o700)).unwrap();
        assert!(said && dir.exists());
        // A leader has none to drop.
        live.drop_scratch(&w, 0);
        for d in [repo, root, live.claude_home.clone(), parent.to_path_buf()] {
            let _ = std::fs::remove_dir_all(d);
        }
    }

    #[test]
    fn a_transcript_that_can_not_be_removed_is_said_so() {
        let (mut w, mut live, repo, root) = remembered_worker("forget-stuck", false);
        let file = claude::transcript(&live.claude_home, &live.cwd_of(&w, 1), UUID);
        std::fs::create_dir_all(file.join("inside")).unwrap();
        assert_eq!(live.forget_agent(&mut w, 1), Ok(()));
        assert!(traced_soon(
            &mut live,
            &mut w,
            1,
            &format!("could not remove its transcript of session {UUID}: ")
        ));
        assert!(file.is_dir(), "left as it was");
        let home = live.claude_home.clone();
        for d in [repo, root, home] {
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
                        models: None,
                    }),
                    Some(Resumable {
                        session: other.into(),
                        spent: 0,
                        models: None,
                    }),
                ],
                ..Default::default()
            };
            let scratch = live.scratch_of(&w, 1).unwrap();
            std::fs::create_dir_all(&scratch).unwrap();
            live.adopt(&mut w, found, 0);
            assert_eq!(scratch.exists(), !fresh, "--fresh drops the scratch too");
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
            let scratch = scratch.parent().unwrap().to_path_buf();
            for d in [repo, root, home, scratch] {
                let _ = std::fs::remove_dir_all(d);
            }
        }
    }

    #[test]
    fn an_archive_is_taken_in_as_it_was_put_away_and_fresh_leaves_its_sessions() {
        let mut w = team(1);
        w.projects.push(Project {
            id: 1,
            name: "old".into(),
            leader: 2,
            trunk: None,
            files: Vec::new(),
            repo: &[],
            closed: true,
            pinned: false,
            copies: Vec::new(),
            allowed: Vec::new(),
            plan: PlanGate::default(),
            no_git: false,
            runners: Runners::default(),
            lonca: None,
            archived: Some(Archived {
                at: 1,
                title: "old".into(),
            }),
        });
        w.agents.push(Agent::new(
            2,
            1,
            "lead".into(),
            Role::Leader,
            Status::Idle,
            0,
        ));
        let mut live = Live::new(
            0,
            claude_at("/nonexistent/claude"),
            PathBuf::from("/nonexistent"),
            PathBuf::from("/nonexistent-trees"),
        );
        live.places.push(Place {
            repo: PathBuf::from("/nonexistent"),
            root: PathBuf::from("/nonexistent-trees"),
            git: true,
        });
        live.fresh = true;
        let found = Found {
            sessions: vec![
                None,
                None,
                Some(Resumable {
                    session: UUID.into(),
                    spent: 3,
                    models: None,
                }),
            ],
            inflight: vec![Vec::new(), vec!["cut a".into()], vec!["cut b".into()]],
            ..Default::default()
        };
        live.adopt(&mut w, found, 0);
        // The open project's cut turn is told at the wake, as ever.
        assert_eq!(live.interrupted[1], ["cut a"]);
        assert_eq!(journaled(&w, &Event::Interrupted { agent: 1 }), 1);
        // The archive's waits as it was put away, said when it comes back.
        assert_eq!(w.agents[2].status, Status::Asleep);
        assert!(live.remembered[2]);
        assert_eq!(live.inflight[2], ["cut b"]);
        assert!(live.interrupted[2].is_empty());
        assert_eq!(journaled(&w, &Event::Interrupted { agent: 2 }), 0);
        // --fresh leaves its session, to go on with when it comes back.
        assert_eq!(live.sessions[2].as_ref().map(|s| s.spent), Some(3));
        assert_eq!(journaled(&w, &Event::Forgotten { agent: 2 }), 0);
        // Kept for the next save as it was found.
        assert!(w.projects[1].closed, "an archive is not closed as gone");
    }

    #[test]
    fn a_world_the_store_cannot_load_is_refused_and_the_file_kept() {
        let repo = checkout("unloadable");
        let state = trees("unloadable-state");
        let options = || Options {
            repo: repo.clone(),
            state: state.clone(),
            max_workers: 2,
            max_helpers: 1,
            tiers: Vec::new(),
            budget: None,
            fresh: false,
            run_programs: Vec::new(),
            run_timeout: DEFAULT_RUN_TIMEOUT,
            instructions: None,
            own_notes: None,
            confine: Confine::Standard,
            solo: None,
            claude: None,
        };
        let (w, mut live) = world(options()).unwrap();
        assert_eq!(live.persist(&w), Ok(true));
        drop(live);
        let file = state.join(store::FILE);
        let role = || {
            let st = Store::open(&file).unwrap();
            st.raw()
                .query_row("SELECT role FROM agents WHERE id = 0", [], |r| {
                    r.get::<_, String>(0)
                })
                .unwrap()
        };
        {
            let st = Store::open(&file).unwrap();
            st.raw()
                .execute("UPDATE agents SET role = 'Clod' WHERE id = 0", [])
                .unwrap();
        }
        let e = world(options()).err().expect("refused");
        assert!(e.starts_with(&format!("{}: ", file.display())), "{e}");
        assert!(e.contains("\"Clod\""), "{e}");
        assert!(
            e.contains("\nimece left the file as it was. To start afresh"),
            "{e}"
        );
        assert_eq!(role(), "Clod", "the row stays as it was");
        for d in [repo, state] {
            let _ = std::fs::remove_dir_all(d);
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
            tiers: Vec::new(),
            budget: None,
            fresh,
            run_programs: Vec::new(),
            run_timeout: DEFAULT_RUN_TIMEOUT,
            instructions: None,
            own_notes: None,
            confine: Confine::Strict,
            solo: None,
            claude: None,
        };
        let (w, mut live) = world(options(false)).unwrap();
        assert_eq!(live.confine, Confine::Strict);
        assert!(
            !w.journal
                .iter()
                .any(|e| matches!(e.event, Event::Confine { .. })),
            "strict has nothing to say"
        );
        live.sessions[0] = Some(Resumable {
            session: UUID.into(),
            spent: 1,
            models: None,
        });
        assert_eq!(live.persist(&w), Ok(true));
        drop(live);
        let (w, live) = world(options(false)).unwrap();
        assert!(live.sessions[0].is_some(), "kept without the flag");
        assert_eq!(journaled(&w, &Event::Forgotten { agent: 0 }), 0);
        drop(live);
        let (w, live) = world(options(true)).unwrap();
        assert!(live.fresh);
        assert_eq!(live.sessions[0], None);
        assert_eq!(journaled(&w, &Event::Forgotten { agent: 0 }), 1);
        drop(live);
        let _ = std::fs::remove_dir_all(&repo);
        let _ = std::fs::remove_dir_all(&state);
    }

    #[test]
    fn a_folder_without_git_keeps_its_solo_and_closes_its_team() {
        let repo = checkout("adopt-git");
        let plain = plain_folder("adopt-plain", false);
        let unborn = plain_folder("adopt-unborn", true);
        let mut w = team(0);
        for (name, path, role) in [
            ("plain", &plain, Role::Leader),
            ("unborn", &unborn, Role::Solo),
            ("gone", &PathBuf::from("/nonexistent/gone"), Role::Solo),
        ] {
            let path = path.display().to_string();
            w.open_project(None, name.into(), path, role);
        }
        let found = Found {
            repos: vec![
                repo.clone(),
                plain.clone(),
                unborn.clone(),
                "/nonexistent/gone".into(),
            ],
            ..Found::default()
        };
        let state = plain_folder("adopt-state", false);
        let mut live = Live::bare(claude_at("x"));
        live.state = state.clone();
        let journal = w.journal.len();
        live.adopt(&mut w, found, 0);
        let git: Vec<bool> = live.places.iter().map(|pl| pl.git).collect();
        assert_eq!(git, [true, false, false, false]);
        let open: Vec<bool> = w.projects.iter().map(|q| !q.closed).collect();
        assert_eq!(open, [true, false, true, false], "a team needs git");
        let no_git: Vec<bool> = w.projects.iter().map(|q| q.no_git).collect();
        assert_eq!(no_git, [false, false, true, false]);
        let said: Vec<&Event> = (w.journal[journal..].iter())
            .map(|e| &e.event)
            .filter(|e| matches!(e, Event::NoGit { .. }))
            .collect();
        let path = unborn.display().to_string();
        assert_eq!(
            said,
            [&Event::NoGit {
                leader: w.projects[2].leader,
                path
            }]
        );
        for dir in [&repo, &plain, &unborn, &state] {
            let _ = std::fs::remove_dir_all(dir);
        }
    }
}
