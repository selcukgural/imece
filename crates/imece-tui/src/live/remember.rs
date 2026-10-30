//! What is remembered between runs: the store, the wake briefing,
//! resuming sessions, and forgetting them.

use super::*;

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
    pub(super) fn adopt(&mut self, w: &mut World, found: Found, skipped: usize) {
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
            // The session that asked is gone with the process before.
            w.close_approvals(id);
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
                runs: found.runs.get(&id).cloned().unwrap_or_default(),
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
    pub(super) fn brief(&mut self, w: &World, id: AgentId, resumed: bool) -> String {
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

    /// Agent `id`'s session is forgotten: its next prompt starts a new
    /// one, and what the vendor kept of the old one goes (Claude's
    /// transcript, Codex's thread, Gemini's chat), on a thread of its own
    /// since a vendor may take a process of its own to do it; the trace
    /// says how it went. `settle` first gives the session's process, just
    /// asked to stop, time to be gone. `whole` takes everything the
    /// vendor kept for the agent (Gemini's private directory too): the
    /// agent is leaving.
    pub(super) fn forget(&mut self, w: &World, id: AgentId, whole: bool, settle: bool) {
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
    pub(super) fn resumable(&mut self, w: &World, id: AgentId) -> Option<Resumable> {
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
    pub(super) fn resume_of(&self, id: AgentId) -> Option<Resumable> {
        self.resuming[id].as_ref().map(|r| Resumable {
            session: r.session.clone(),
            spent: self.floor[id],
        })
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
        let ran = Ran {
            line: "cargo test".into(),
            code: Some(0),
            timed_out: false,
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
            runs: HashMap::from([(1, vec![ran.clone()])]),
        };
        let mut live = Live::bare(claude_at("x"), Vendor::Claude);
        live.state = state.clone();
        // What a session asked went with the process.
        w.asks_you(1, "Bash: make");
        let journal = w.journal.len();
        live.adopt(&mut w, found, 3);
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
            confine: Confine::Standard,
        };
        let present = [(Vendor::Claude, true)];
        let (mut w, mut live) = world(options(&repo, None), &present).unwrap();
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
        let (w, mut live) = world(options(&repo, Some(20_000)), &present).unwrap();
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
        // imece's /clear on a Gemini session nothing is remembered of:
        // nothing to remove, and the private directory stays.
        live.ready(&mut w, 1, Ok((Box::new(Stub), private.clone(), None)));
        assert_eq!(live.sessions[1], None);
        live.command(&mut w, 1, "/clear").unwrap();
        thread::sleep(Duration::from_millis(200));
        assert!(private.is_dir(), "no session, nothing of it to remove");
        assert!(kept.is_dir());
        let home = live.claude_home.clone();
        for d in [repo, root, home, live.gemini_config.clone()] {
            let _ = std::fs::remove_dir_all(d);
        }
    }

    #[test]
    fn forgetting_the_whole_of_a_gemini_agent_says_what_it_found() {
        // (session remembered, kept directory, private directory) → the note.
        let cases: [(bool, bool, Option<&str>, &str); 5] = [
            (
                false,
                true,
                Some("dir"),
                "forgotten: its chats and private directory were removed",
            ),
            (
                true,
                true,
                Some("dir"),
                "its chats and private directory were removed",
            ),
            (
                true,
                true,
                None,
                "its chats and private directory were removed",
            ),
            (false, false, None, "forgotten: it had no chats left"),
            (
                true,
                true,
                Some("file"),
                "could not remove its chat of session",
            ),
        ];
        for (i, (remembered, kept, private, note)) in cases.into_iter().enumerate() {
            let (mut w, mut live, repo, root) =
                remembered_vendor_worker(&format!("forget-whole-{i}"), Vendor::Gemini);
            live.gemini_config = trees(&format!("forget-whole-{i}-config"));
            let name = w.agents[1].name.clone();
            let home = live.gemini_home(0, Role::Worker, &name);
            let kept_dir = if kept {
                Some(gemini_kept(
                    &live.gemini_config,
                    &home,
                    "g",
                    &[("session-x.jsonl", "")],
                ))
            } else {
                None
            };
            match private {
                Some("dir") => std::fs::create_dir_all(&home).unwrap(),
                Some(_) => {
                    std::fs::create_dir_all(home.parent().unwrap()).unwrap();
                    std::fs::write(&home, "").unwrap();
                }
                None => {}
            }
            if !remembered {
                live.sessions[1] = None;
            }
            live.forget(&w, 1, true, false);
            assert_eq!(live.sessions[1], None);
            assert!(traced_soon(&mut live, &mut w, 1, note), "case {i}");
            if remembered {
                assert!(
                    traced_soon(&mut live, &mut w, 1, &format!("session {UUID}")),
                    "case {i} names the session"
                );
            }
            let failed = private == Some("file");
            assert!(!home.is_dir(), "case {i}");
            assert_eq!(home.exists(), failed, "case {i}: a file is left as it is");
            if let Some(k) = &kept_dir {
                assert!(!k.exists(), "case {i}: what Gemini kept goes first");
            }
            let claude_home = live.claude_home.clone();
            for d in [repo, root, claude_home, live.gemini_config.clone()] {
                let _ = std::fs::remove_dir_all(d);
            }
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
            confine: Confine::Strict,
        };
        let present = [(Vendor::Claude, true)];
        let (w, mut live) = world(options(false), &present).unwrap();
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
