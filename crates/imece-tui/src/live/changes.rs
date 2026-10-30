//! A worker's change: read from its worktree on `done`, applied to
//! the person's checkout or rejected, and the worktree following the
//! checkout between turns.

use super::*;

/// Runs the git jobs one after another, in the order they came, until
/// imece drops its end.
pub(super) fn git_thread(jobs: Receiver<GitJob>, done: Sender<GitDone>) {
    for job in jobs {
        let out = match job {
            GitJob::Read {
                agent,
                path,
                title,
                said,
                reply,
            } => GitDone::Read {
                agent,
                title,
                said,
                reply,
                read: worktree::change(&path, MAX_PATCH),
            },
            GitJob::Apply {
                agent,
                repo,
                change,
                title,
            } => {
                let applied = worktree::apply(&repo, &change.patch);
                let mark = match &applied {
                    Ok(()) => {
                        worktree::applied(&change, &format!("imece: the person applied: {title}"))
                    }
                    Err(_) => Ok(String::new()),
                };
                GitDone::Applied {
                    agent,
                    title,
                    applied,
                    mark,
                }
            }
            GitJob::Place {
                agent,
                generation,
                repo,
                path,
            } => GitDone::Placed {
                agent,
                generation,
                placed: worktree::place(&repo, &path),
            },
        };
        if done.send(out).is_err() {
            return;
        }
    }
}

impl Live {
    /// Has git read what worker `id`'s worktree holds, for the person: it
    /// waits in their inbox until they apply or reject it. The worker
    /// hears `said` and what the change is on `reply` once it is read; at
    /// once when it has no directory to read.
    pub(super) fn snapshot(
        &mut self,
        w: &mut World,
        id: AgentId,
        title: String,
        said: String,
        reply: Sender<Answer>,
    ) {
        let Some(path) = self.cwds[id].clone() else {
            let _ = reply.send(Ok(said));
            return;
        };
        self.reading[id] += 1;
        let job = GitJob::Read {
            agent: id,
            path,
            title,
            said,
            reply,
        };
        if let Err(mpsc::SendError(GitJob::Read { said, reply, .. })) = self.git.send(job) {
            self.reading[id] -= 1;
            w.agents[id].trace(w.tick, "change: git is gone");
            let _ = reply.send(Ok(said + "; imece could not read your change"));
        }
    }

    /// What the git thread finished since the last step.
    pub(super) fn take_git(&mut self, w: &mut World) {
        while let Ok(done) = self.git_back.try_recv() {
            match done {
                GitDone::Read {
                    agent,
                    title,
                    said,
                    reply,
                    read,
                } => {
                    self.reading[agent] -= 1;
                    let words = self.read(w, agent, &title, read);
                    let _ = reply.send(Ok(said + &words));
                }
                GitDone::Applied {
                    agent,
                    title,
                    applied,
                    mark,
                } => {
                    self.applying[agent] = false;
                    self.applied(w, agent, &title, applied, mark);
                }
                GitDone::Placed {
                    agent,
                    generation,
                    placed,
                } => self.placed(w, agent, generation, placed),
            }
        }
    }

    /// Worker `id`'s change as git read it: it waits for the person, an
    /// older one of the worker's goes. Says what the worker is told.
    fn read(
        &mut self,
        w: &mut World,
        id: AgentId,
        title: &str,
        read: io::Result<Option<worktree::Change>>,
    ) -> String {
        match read {
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

    /// The person takes worker `id`'s change: git puts it into the
    /// checkout, staged, and commits it in the worktree as a mark, so the
    /// next change holds only what comes after; the journal says how it
    /// went. Refused while it is being applied, or while a newer change of
    /// the worker's is still being read.
    pub fn apply_change(&mut self, w: &World, id: AgentId) -> Result<(), String> {
        if self.applying[id] {
            return Err("their change is being applied".into());
        }
        if self.reading[id] > 0 {
            return Err("imece is still reading their newer change".into());
        }
        let Some(c) = self.changes.get(&id) else {
            return Err("no change of theirs waits".into());
        };
        let title = w.change_of(id).map(|c| c.title.clone()).unwrap_or_default();
        let job = GitJob::Apply {
            agent: id,
            repo: self.places[w.agents[id].project].repo.clone(),
            change: c.clone(),
            title,
        };
        self.git
            .send(job)
            .map_err(|_| "imece's git thread is gone".to_string())?;
        self.applying[id] = true;
        Ok(())
    }

    /// How applying worker `id`'s change went. One that did not fit is
    /// refused with git's words and still waits; one applied but not
    /// marked is applied all the same.
    fn applied(
        &mut self,
        w: &mut World,
        id: AgentId,
        title: &str,
        applied: io::Result<()>,
        mark: io::Result<String>,
    ) {
        if let Err(e) = applied {
            w.change_failed(id, e.to_string());
            return;
        }
        let mark = match mark {
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
    }

    /// The person rejects worker `id`'s change: it stays in the worktree,
    /// and the worker is told to wait for their reasons. Not while it is
    /// being applied.
    pub fn reject_change(&mut self, w: &mut World, id: AgentId) -> Result<(), String> {
        if self.applying[id] {
            return Err("their change is being applied".into());
        }
        let title = w.change_of(id).map(|c| c.title.clone()).unwrap_or_default();
        self.changes.remove(&id);
        w.change_rejected(id);
        self.notes[id].push(format!(
            "The person rejected your change ({title}). It stays in your worktree, not \
             applied; they say why themselves. Do not send it again as it is."
        ));
        Ok(())
    }

    /// Between turns, a worker's clean worktree follows the checkout's
    /// HEAD (as it does when its session starts), so a change the person
    /// applied and committed is under the worker's feet for the next task.
    /// Not while it or one of its helpers is mid-turn. Git moves it on
    /// its thread; the prompts wait in `placing` until it is done.
    pub(super) fn follow(&mut self, w: &mut World, id: AgentId) {
        let a = &w.agents[id];
        let busy = self.turns[id] > 0 || w.helpers_of(id).any(|h| self.turns[h.id] > 0);
        if a.role != Role::Worker
            || busy
            || self.placing[id].is_some()
            || !matches!(self.slots[id], Slot::Running(_))
        {
            return;
        }
        let Some(path) = self.cwds[id].clone() else {
            return;
        };
        let job = GitJob::Place {
            agent: id,
            generation: self.generation[id],
            repo: self.places[a.project].repo.clone(),
            path,
        };
        if self.git.send(job).is_ok() {
            self.placing[id] = Some(Vec::new());
        } else {
            w.agents[id].trace(w.tick, "worktree: git is gone");
        }
    }

    /// Where git put worker `id`'s worktree; then its session gets the
    /// prompts that waited, in order. Nothing for a session that ended
    /// meanwhile.
    fn placed(
        &mut self,
        w: &mut World,
        id: AgentId,
        generation: u64,
        placed: io::Result<worktree::Placed>,
    ) {
        if generation != self.generation[id] {
            return;
        }
        match placed {
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
        for text in self.placing[id].take().unwrap_or_default() {
            let Slot::Running(s) = &mut self.slots[id] else {
                return;
            };
            if let Err(e) = s.prompt(&text) {
                self.end(w, id);
                return apply(
                    w,
                    id,
                    AgentEvent::Stopped(e.to_string()),
                    self.spent_before[id],
                );
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use std::sync::{Arc, Mutex};

    use super::*;
    use crate::live::testing::*;

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
        let got = sent(&mut live, &mut w, 1, &msg("lead", "done", "Nothing to do.")).unwrap();
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
        let got = sent(
            &mut live,
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
        sent(&mut live, &mut w, 1, &msg("lead", "done", "And more.")).unwrap();
        assert_eq!(w.open_inbox().len(), 1);
        assert_eq!(w.change_of(1).unwrap().files.len(), 3);
        assert_eq!(live.changes[&1].files.len(), 3);

        // The person's own edit in a touched file: the change does not
        // apply, nothing is half done, it still waits.
        std::fs::write(repo.join("a.txt"), "theirs").unwrap();
        live.apply_change(&w, 1).unwrap();
        settle(&mut live, &mut w);
        let Event::ApplyFailed { agent: 1, why: e } = &w.journal.last().unwrap().event else {
            panic!("{:?}", w.journal.last());
        };
        let e = e.clone();
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
        // worker hears of it at its next turn. Git does it on its own
        // thread: until it is done, the change waits as it was and is
        // neither applied nor rejected again.
        live.apply_change(&w, 1).unwrap();
        assert_eq!(
            live.apply_change(&w, 1),
            Err("their change is being applied".into())
        );
        assert_eq!(
            live.reject_change(&mut w, 1),
            Err("their change is being applied".into())
        );
        assert!(live.changes.contains_key(&1) && live.notes[1].is_empty());
        settle(&mut live, &mut w);
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
        let e = live.apply_change(&w, 1).unwrap_err();
        assert!(e.contains("no change of theirs waits"), "{e}");
        let _ = std::fs::remove_dir_all(&root);
        let _ = std::fs::remove_dir_all(&repo);
    }

    #[test]
    fn git_takes_the_changes_in_order_and_an_apply_waits_for_a_read() {
        let repo = checkout("order-repo");
        let root = trees("order");
        let mut w = team(1);
        let mut live = Live::new(
            2,
            claude_at("/nonexistent/claude"),
            repo.clone(),
            root.clone(),
        );
        let tree = worker_in(&mut live, &mut w, 1, &repo, &root);
        std::fs::write(tree.join("a.txt"), "changed").unwrap();
        sent(&mut live, &mut w, 1, &msg("lead", "done", "Changed a.")).unwrap();
        // A newer change still being read is not passed over.
        live.reading[1] = 1;
        assert_eq!(
            live.apply_change(&w, 1),
            Err("imece is still reading their newer change".into())
        );
        live.reading[1] = 0;
        // Applied, then a done right after: git reads it once the apply is
        // in, so the new change holds only what came after.
        live.apply_change(&w, 1).unwrap();
        std::fs::write(tree.join("new.txt"), "new\n").unwrap();
        let (tx, rx) = mpsc::channel();
        live.send(&mut w, 1, &msg("lead", "done", "Added new."), tx);
        assert!(rx.try_recv().is_err(), "the answer waits for git");
        assert_eq!(live.reading[1], 1);
        settle(&mut live, &mut w);
        let got = rx.try_recv().unwrap().unwrap();
        assert!(got.contains("(1 files, +1 −0)"), "{got}");
        let applied = w
            .journal
            .iter()
            .position(|e| matches!(e.event, Event::Applied { agent: 1, .. }))
            .unwrap();
        let ready = w
            .journal
            .iter()
            .rposition(|e| matches!(e.event, Event::ChangeReady { agent: 1, .. }))
            .unwrap();
        assert!(applied < ready, "{applied} {ready}");
        let files: Vec<&str> = w
            .change_of(1)
            .unwrap()
            .files
            .iter()
            .map(|f| f.path.as_str())
            .collect();
        assert_eq!(files, ["new.txt"]);
        assert_eq!(live.changes[&1].files.len(), 1);
        let _ = std::fs::remove_dir_all(&root);
        let _ = std::fs::remove_dir_all(&repo);
    }

    #[test]
    fn a_worker_s_done_to_the_person_shows_its_change_on_the_screen_s_loop() {
        let repo = checkout("you-repo");
        let root = trees("you");
        let mut w = team(1);
        let mut live = Live::new(
            3,
            claude_at("/nonexistent/claude"),
            repo.clone(),
            root.clone(),
        );
        let tree = worker_in(&mut live, &mut w, 1, &repo, &root);
        let helper = with_helper(&mut w, 1);
        live.ready(&mut w, helper, Ok((Box::new(Stub), tree.clone(), None)));
        live.ready(&mut w, 0, Ok((Box::new(Stub), repo.clone(), None)));
        std::fs::write(tree.join("a.txt"), "changed").unwrap();
        // Only a worker's done: not its inform, not a helper's or the
        // leader's done.
        for (from, kind) in [(1, "inform"), (helper, "done"), (0, "done")] {
            let got = sent(&mut live, &mut w, from, &msg("you", kind, "x")).unwrap();
            assert!(!got.contains("change"), "{from} {kind}: {got}");
        }
        assert!(w.change_of(1).is_none());
        // Through the bus: the answer comes on a later step, once git has
        // read the change.
        let (tx, rx) = mpsc::channel();
        live.calls_tx
            .send((1, "send".into(), msg("you", "done", "Changed a."), tx))
            .unwrap();
        let got = (0..500)
            .find_map(|_| {
                live.step(&mut w);
                thread::sleep(Duration::from_millis(10));
                rx.try_recv().ok()
            })
            .expect("answered in time")
            .unwrap();
        assert!(
            got.starts_with("noted; the person sees it in the journal"),
            "{got}"
        );
        assert!(
            got.contains("the person sees your change (1 files, +1 −1)"),
            "{got}"
        );
        assert_eq!(w.change_of(1).unwrap().title, "Changed a.");
        let _ = std::fs::remove_dir_all(&root);
        let _ = std::fs::remove_dir_all(&repo);
    }

    #[test]
    fn without_its_git_thread_a_done_is_answered_and_nothing_is_applied() {
        let repo = checkout("gone-repo");
        let root = trees("gone");
        let mut w = team(1);
        let mut live = Live::new(
            2,
            claude_at("/nonexistent/claude"),
            repo.clone(),
            root.clone(),
        );
        let tree = worker_in(&mut live, &mut w, 1, &repo, &root);
        let (gone, _) = mpsc::channel();
        live.git = gone;
        std::fs::write(tree.join("a.txt"), "changed").unwrap();
        let got = sent(&mut live, &mut w, 1, &msg("lead", "done", "Changed a.")).unwrap();
        assert!(got.ends_with("; imece could not read your change"), "{got}");
        assert_eq!(live.reading[1], 0);
        assert!(w.open_inbox().is_empty());
        assert!(
            w.agents[1]
                .trace
                .iter()
                .any(|l| l.ends_with("change: git is gone"))
        );
        hold_change(&mut live, 1);
        assert_eq!(
            live.apply_change(&w, 1),
            Err("imece's git thread is gone".into())
        );
        assert!(!live.applying[1]);
        // Nor is the worktree moved: the prompt goes on without that.
        live.told(&mut w, 1, "go").unwrap();
        assert_eq!(live.placing[1], None);
        assert_eq!(live.inflight[1], ["go"]);
        assert!(
            w.agents[1]
                .trace
                .iter()
                .any(|l| l.ends_with("worktree: git is gone"))
        );
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
        sent(&mut live, &mut w, 1, &msg("lead", "done", "Changed a.")).unwrap();
        assert_eq!(w.open_inbox().len(), 1);
        live.reject_change(&mut w, 1).unwrap();
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
        live.reject_change(&mut w, 1).unwrap();
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
            let got = sent(&mut live, &mut w, from, &msg(to, kind, "x")).unwrap();
            assert!(!got.contains("change"), "{from} {kind}: {got}");
        }
        assert!(w.open_inbox().is_empty());
        // A worker without a directory (no session yet) has nothing to read.
        live.cwds[1] = None;
        let got = sent(&mut live, &mut w, 1, &msg("lead", "done", "x")).unwrap();
        assert!(!got.contains("change"), "{got}");
        assert!(w.open_inbox().is_empty());
        // Too large to show: the worker is told, nothing waits.
        live.cwds[1] = Some(tree.clone());
        std::fs::write(tree.join("big.txt"), "x".repeat(MAX_PATCH + 1)).unwrap();
        let got = sent(&mut live, &mut w, 1, &msg("lead", "done", "x")).unwrap();
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

    /// A session that keeps what it is prompted with; one that fails
    /// refuses every prompt.
    struct Heard(Arc<Mutex<Vec<String>>>, bool);

    impl imece_agent::Agent for Heard {
        fn prompt(&mut self, text: &str) -> io::Result<()> {
            if self.1 {
                return Err(io::Error::new(io::ErrorKind::BrokenPipe, "gone"));
            }
            self.0.lock().unwrap().push(text.to_string());
            Ok(())
        }
        fn command(&mut self, _: &str) -> io::Result<()> {
            Ok(())
        }
        fn terminate(&mut self) {}
    }

    #[test]
    fn prompts_wait_while_git_moves_the_worktree_and_go_in_order() {
        let repo = checkout("placing-repo");
        let root = trees("placing");
        let mut w = team(1);
        let mut live = Live::new(
            2,
            claude_at("/nonexistent/claude"),
            repo.clone(),
            root.clone(),
        );
        let tree = worktree::ensure(&repo, &root, "claude-1").unwrap().path;
        let heard = Arc::new(Mutex::new(Vec::new()));
        let session = |fails| Box::new(Heard(heard.clone(), fails));
        live.ready(&mut w, 1, Ok((session(false), tree.clone(), None)));
        let commit = |name: &str| {
            std::fs::write(repo.join(name), name).unwrap();
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
                    name,
                ],
            );
        };
        let moved = |w: &World| {
            w.journal
                .iter()
                .filter(|e| matches!(e.event, Event::WorktreeMoved { agent: 1, .. }))
                .count()
        };
        commit("b.txt");
        // The prompt waits for git; the one after it waits behind it.
        live.told(&mut w, 1, "go").unwrap();
        live.told(&mut w, 1, "more").unwrap();
        assert_eq!(
            live.placing[1],
            Some(vec!["go".to_string(), "more".to_string()])
        );
        assert_eq!(live.turns[1], 2);
        assert!(heard.lock().unwrap().is_empty());
        settle(&mut live, &mut w);
        assert_eq!(*heard.lock().unwrap(), ["go", "more"]);
        assert_eq!(moved(&w), 1);
        assert!(tree.join("b.txt").exists());
        // A session that ended meanwhile hears nothing, and nothing is said.
        commit("c.txt");
        live.turns[1] = 0;
        live.told(&mut w, 1, "late").unwrap();
        live.end(&mut w, 1);
        assert_eq!(live.placing[1], None);
        settle(&mut live, &mut w);
        assert_eq!(heard.lock().unwrap().len(), 2);
        assert_eq!(moved(&w), 1);
        // A session that refuses the prompt that waited is stopped.
        live.ready(&mut w, 1, Ok((session(true), tree.clone(), None)));
        commit("d.txt");
        live.told(&mut w, 1, "refused").unwrap();
        settle(&mut live, &mut w);
        assert_eq!(moved(&w), 2);
        assert_eq!(w.agents[1].status, Status::Stopped);
        assert!(matches!(live.slots[1], Slot::None));
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
        // Mid-turn (its own or a helper's), or with no running session:
        // git is not even asked.
        live.turns[1] = 1;
        live.follow(&mut w, 1);
        assert_eq!(live.placing[1], None);
        live.turns[1] = 0;
        live.turns[2] = 1;
        live.follow(&mut w, 1);
        assert_eq!(live.placing[1], None);
        live.turns[2] = 0;
        let running = std::mem::replace(&mut live.slots[1], Slot::Starting(Vec::new()));
        live.follow(&mut w, 1);
        assert_eq!(live.placing[1], None);
        live.slots[1] = running;
        settle(&mut live, &mut w);
        assert_eq!(moved(&w), 0);
        assert_eq!(git_in(&tree, &["rev-parse", "HEAD"]), old);
        // The leader's directory is the checkout itself: never moved.
        live.ready(&mut w, 0, Ok((Box::new(Stub), repo.clone(), None)));
        live.follow(&mut w, 0);
        // Between turns: the next prompt finds the worktree at HEAD.
        live.told(&mut w, 1, "go").unwrap();
        settle(&mut live, &mut w);
        assert_eq!(moved(&w), 1);
        assert_eq!(git_in(&tree, &["rev-parse", "HEAD"]), new);
        assert!(tree.join("b.txt").exists());
        // Already there: nothing more is said.
        live.turns[1] = 0;
        live.told(&mut w, 1, "go on").unwrap();
        settle(&mut live, &mut w);
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
        settle(&mut live, &mut w);
        assert_eq!(moved(&w), 1);
        assert_eq!(git_in(&tree, &["rev-parse", "HEAD"]), new);
        live.cwds[1] = Some(PathBuf::from("/nonexistent/tree"));
        live.turns[1] = 0;
        live.told(&mut w, 1, "still").unwrap();
        settle(&mut live, &mut w);
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
}
