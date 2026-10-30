//! A worker's change: read from its worktree on `done`, applied to
//! the person's checkout or rejected, and the worktree following the
//! checkout between turns.

use super::*;

impl Live {
    /// Reads what worker `id`'s worktree holds, for the person: it waits
    /// in their inbox until they apply or reject it. Says what the worker
    /// is told about it.
    pub(super) fn snapshot(&mut self, w: &mut World, id: AgentId, title: &str) -> String {
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

    /// Between turns, a worker's clean worktree follows the checkout's
    /// HEAD (as it does when its session starts), so a change the person
    /// applied and committed is under the worker's feet for the next task.
    /// Not while it or one of its helpers is mid-turn.
    pub(super) fn follow(&mut self, w: &mut World, id: AgentId) {
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
}

#[cfg(test)]
mod tests {
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
}
