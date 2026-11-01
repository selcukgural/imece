//! A worker's change: read from its worktree on `done`, applied to
//! the person's checkout or rejected, and the worktree following the
//! checkout between turns, and where each worker's worktree stands.

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
                hunks,
            } => {
                // Some hunks only: the part they make is what is applied
                // and marked; the rest stays in the worktree.
                let part = (!hunks.is_empty()).then_some(hunks.len());
                let change = match part {
                    Some(_) => worktree::part(&change, &worktree::subset(&change.patch, &hunks)),
                    None => Ok(change),
                };
                let applied = change
                    .as_ref()
                    .map_err(|e| io::Error::other(e.to_string()))
                    .and_then(|c| worktree::apply(&repo, &c.patch));
                let mark = match (&applied, &change) {
                    (Ok(()), Ok(c)) => {
                        worktree::applied(c, &format!("imece: the person applied: {title}"))
                    }
                    _ => Ok(String::new()),
                };
                GitDone::Applied {
                    agent,
                    title,
                    applied,
                    mark,
                    part,
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
            GitJob::Stand { agent, repo, path } => GitDone::Stood {
                agent,
                stand: worktree::stand(&repo, &path),
            },
            GitJob::Staged { project, repo } => GitDone::Staged {
                project,
                staged: worktree::staged_change(&repo, SUGGEST_PATCH),
            },
            GitJob::Files { project, repo } => GitDone::Files {
                project,
                files: worktree::files(&repo, FILES_MAX),
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

    /// Runs that ended since the last look, kept with their trees; the
    /// oldest past [`RAN_KEEP`] go.
    fn take_ran(&mut self) {
        while let Ok(r) = self.ran_back.try_recv() {
            self.ran.push_back(r);
        }
        let extra = self.ran.len().saturating_sub(RAN_KEEP);
        self.ran.drain(..extra);
    }

    /// What the git thread finished since the last step, and the runs that
    /// ended.
    pub(super) fn take_git(&mut self, w: &mut World) {
        self.take_ran();
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
                    part,
                } => {
                    self.applying[agent] = false;
                    self.applied(w, agent, &title, applied, mark, part);
                }
                GitDone::Placed {
                    agent,
                    generation,
                    placed,
                } => self.placed(w, agent, generation, placed),
                GitDone::Stood { agent, stand } => self.stood(w, agent, stand),
                GitDone::Staged { project, staged } => self.take_staged(w, project, staged),
                GitDone::Files { project, files } => self.filed(w, project, files),
            }
        }
    }

    /// Has git list project `p`'s files, for completion in a message;
    /// not while it is listing them already.
    pub(super) fn files(&mut self, p: ProjectId) {
        if self.filing.contains(&p) {
            return;
        }
        let Some(place) = self.places.get(p) else {
            return;
        };
        let job = GitJob::Files {
            project: p,
            repo: place.repo.clone(),
        };
        if self.git.send(job).is_ok() {
            self.filing.insert(p);
            self.filed_at.insert(p, Instant::now());
        }
    }

    /// Every open project's files are listed once it opens (or opens
    /// again), and again every [`FILES_EVERY`] (as of `now`) while one of
    /// its agents is mid-turn, when new files may come; between turns
    /// the list is read when a change is applied.
    pub(super) fn files_due(&mut self, w: &World, now: Instant) {
        let open: Vec<ProjectId> = w.open_projects().map(|p| p.id).collect();
        for p in open {
            let busy = w
                .agents
                .iter()
                .any(|a| a.project == p && self.turns[a.id] > 0);
            let due = match self.filed_at.get(&p) {
                None => true,
                Some(at) => busy && now.duration_since(*at) >= FILES_EVERY,
            };
            if due {
                self.files(p);
            }
        }
    }

    /// What git listed for project `p`: the files for completion; a
    /// checkout with more than it keeps is said once in the journal; a
    /// listing that failed keeps the last one and is traced.
    fn filed(&mut self, w: &mut World, p: ProjectId, files: io::Result<(Vec<String>, bool)>) {
        self.filing.remove(&p);
        let leader = w.projects[p].leader;
        match files {
            Ok((files, cut)) => {
                w.projects[p].files = files;
                if cut && self.files_cut.insert(p) {
                    w.note(Event::FilesCut {
                        leader,
                        kept: FILES_MAX,
                    });
                }
            }
            Err(e) => {
                let t = w.tick;
                w.agents[leader].trace(t, format!("files: {e}"));
            }
        }
    }

    /// Has git read where worker `id`'s worktree stands, for its row; a
    /// helper's worktree is its worker's. One read at a time per worker:
    /// asked again meanwhile, another follows it, so the row ends with
    /// what the worktree holds after the last ask.
    pub(super) fn stand(&mut self, w: &World, id: AgentId) {
        let a = &w.agents[id];
        let id = match (a.role, a.parent) {
            (Role::Worker | Role::Solo, _) => id,
            (Role::Helper, Some(worker)) => worker,
            _ => return,
        };
        if let Some(again) = &mut self.standing[id] {
            *again = true;
            return;
        }
        let Some(path) = self.cwds[id].clone() else {
            return;
        };
        let job = GitJob::Stand {
            agent: id,
            repo: self.places[w.agents[id].project].repo.clone(),
            path,
        };
        if self.git.send(job).is_ok() {
            self.standing[id] = Some(false);
            self.stood_at[id] = Some(Instant::now());
        }
    }

    /// Where git found worker `id`'s worktree; asked again meanwhile, it
    /// reads it once more.
    fn stood(&mut self, w: &mut World, id: AgentId, stand: io::Result<worktree::Stand>) {
        let again = self.standing[id].take() == Some(true);
        match stand {
            Ok(s) => {
                w.agents[id].stand = Stand {
                    branch: s.branch,
                    ahead: s.ahead,
                    behind: s.behind,
                    added: s.added,
                    removed: s.removed,
                };
            }
            Err(e) => w.agents[id].trace(w.tick, format!("stand: {e}")),
        }
        if again {
            self.stand(w, id);
        }
    }

    /// A worker that works, or whose helper works, learns where its
    /// worktree stands every [`STAND_EVERY`] (as of `now`); its row
    /// counts the lines as they are written.
    pub(super) fn stand_busy(&mut self, w: &World, now: Instant) {
        for a in (w.agents.iter()).filter(|a| matches!(a.role, Role::Worker | Role::Solo)) {
            let busy = self.turns[a.id] > 0 || w.helpers_of(a.id).any(|h| self.turns[h.id] > 0);
            let due = self.stood_at[a.id].is_none_or(|at| now.duration_since(at) >= STAND_EVERY);
            if busy && due && self.standing[a.id].is_none() {
                self.stand(w, a.id);
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
                let runs = self.runs_on(&c.tree);
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
                    runs,
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

    /// The runs that ran on exactly `tree`, the last of each command,
    /// oldest first. The runs' threads sent theirs before they answered,
    /// so every run a done came after is in.
    fn runs_on(&mut self, tree: &str) -> Vec<Ran> {
        self.take_ran();
        let mut runs: Vec<Ran> = Vec::new();
        for (_, r) in self.ran.iter().filter(|(t, _)| t == tree) {
            runs.retain(|o| o.line != r.line);
            runs.push(r.clone());
        }
        runs
    }

    /// The person takes worker `id`'s change: git puts it into the
    /// checkout, staged, and commits it in the worktree as a mark, so the
    /// next change holds only what comes after; the journal says how it
    /// went. Refused while it is being applied, or while a newer change of
    /// the worker's is still being read.
    pub fn apply_change(
        &mut self,
        w: &World,
        id: AgentId,
        hunks: Vec<usize>,
    ) -> Result<(), String> {
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
            hunks,
        };
        self.git
            .send(job)
            .map_err(|_| "imece's git thread is gone".to_string())?;
        self.applying[id] = true;
        Ok(())
    }

    /// How applying worker `id`'s change went. One that did not fit is
    /// refused with git's words and still waits; one applied but not
    /// marked is applied all the same. When only `part` of its hunks
    /// went, the rest is read again and waits as a change of its own.
    fn applied(
        &mut self,
        w: &mut World,
        id: AgentId,
        title: &str,
        applied: io::Result<()>,
        mark: io::Result<String>,
        part: Option<usize>,
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
        self.stand(w, id);
        // The checkout has new files now, maybe: the list is read again.
        self.files(w.agents[id].project);
        let Some(n) = part else {
            self.notes[id].push(format!(
                "The person applied your change ({title}) to their checkout. Your worktree is \
                 committed at {mark} as a mark of that; your files are as you left them. \
                 Go on from here with the next task."
            ));
            return;
        };
        self.notes[id].push(format!(
            "The person applied part of your change ({title}) to their checkout: {n} of its \
             hunks. Your worktree is committed at {mark} with only that part; your files are \
             as you left them, and the rest is not committed and waits for the person as a \
             change again."
        ));
        // Nobody waits on this reading: the worker's done was answered.
        let (said, _) = mpsc::channel();
        self.snapshot(w, id, title.to_string(), String::new(), said);
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
        if !matches!(a.role, Role::Worker | Role::Solo)
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
        self.stand(w, id);
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
    fn chosen_hunks_go_to_the_checkout_and_the_rest_waits_as_a_change_again() {
        let repo = checkout("part-repo");
        let root = trees("part");
        let mut w = team(2);
        let mut live = Live::new(
            3,
            claude_at("/nonexistent/claude"),
            repo.clone(),
            root.clone(),
        );
        let tree = worker_in(&mut live, &mut w, 1, &repo, &root);
        std::fs::write(tree.join("a.txt"), "a\nb\n").unwrap();
        std::fs::write(tree.join("new.txt"), "new\n").unwrap();
        sent(&mut live, &mut w, 1, &msg("lead", "done", "Two things.")).unwrap();
        settle(&mut live, &mut w);
        let patch = w.change_of(1).unwrap().patch.clone();
        let hunks: Vec<usize> = (patch.lines().enumerate())
            .filter(|(_, l)| l.starts_with("@@"))
            .map(|(i, _)| i)
            .collect();
        assert_eq!(hunks.len(), 2, "{patch}");
        // new.txt's hunk alone.
        let new = hunks[1];
        assert_eq!(patch.lines().nth(new - 1), Some("+++ b/new.txt"), "{patch}");
        live.apply_change(&w, 1, vec![new]).unwrap();
        settle(&mut live, &mut w);
        assert_eq!(
            std::fs::read_to_string(repo.join("new.txt")).unwrap(),
            "new\n"
        );
        assert_eq!(std::fs::read_to_string(repo.join("a.txt")).unwrap(), "a");
        assert_eq!(
            git_in(&repo, &["diff", "--cached", "--name-only"]),
            "new.txt"
        );
        assert!(matches!(
            w.journal
                .iter()
                .rev()
                .find(|e| matches!(e.event, Event::Applied { .. }))
                .unwrap()
                .event,
            Event::Applied {
                agent: 1,
                files: 2,
                ..
            }
        ));
        // The worker hears it was part; its files are as they were.
        assert_eq!(live.notes[1].len(), 1);
        assert!(live.notes[1][0].contains("applied part of your change (Two things.)"));
        assert!(live.notes[1][0].contains("1 of its hunks"));
        assert_eq!(
            std::fs::read_to_string(tree.join("a.txt")).unwrap(),
            "a\nb\n"
        );
        // The rest waits as a change again, a.txt's alone.
        let rest = w.change_of(1).expect("the rest waits");
        assert_eq!(rest.title, "Two things.");
        let files: Vec<&str> = rest.files.iter().map(|f| f.path.as_str()).collect();
        assert_eq!(files, ["a.txt"]);
        assert_eq!(live.changes[&1].files.len(), 1);
        assert_eq!(w.open_inbox().len(), 1);
        // Applied whole, nothing waits after.
        live.apply_change(&w, 1, Vec::new()).unwrap();
        settle(&mut live, &mut w);
        assert!(w.change_of(1).is_none() && w.open_inbox().is_empty());
        assert_eq!(
            std::fs::read_to_string(repo.join("a.txt")).unwrap(),
            "a\nb\n"
        );
        // A chosen hunk the patch does not have refuses, and it still waits.
        std::fs::write(tree.join("a.txt"), "a\nb\nc\n").unwrap();
        sent(&mut live, &mut w, 1, &msg("lead", "done", "One more.")).unwrap();
        settle(&mut live, &mut w);
        live.apply_change(&w, 1, vec![0]).unwrap();
        settle(&mut live, &mut w);
        assert!(matches!(
            w.journal.last().unwrap().event,
            Event::ApplyFailed { agent: 1, .. }
        ));
        assert!(w.change_of(1).is_some());
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
        live.apply_change(&w, 1, Vec::new()).unwrap();
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
        live.apply_change(&w, 1, Vec::new()).unwrap();
        assert_eq!(
            live.apply_change(&w, 1, Vec::new()),
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
        let e = live.apply_change(&w, 1, Vec::new()).unwrap_err();
        assert!(e.contains("no change of theirs waits"), "{e}");
        let _ = std::fs::remove_dir_all(&root);
        let _ = std::fs::remove_dir_all(&repo);
    }

    #[test]
    fn a_change_holds_the_runs_that_ran_on_exactly_its_tree() {
        let repo = checkout("ran-repo");
        let root = trees("ran");
        let mut w = team(1);
        let mut live = Live::new(
            3,
            claude_at("/nonexistent/claude"),
            repo.clone(),
            root.clone(),
        );
        let tree = worker_in(&mut live, &mut w, 1, &repo, &root);
        let helper = with_helper(&mut w, 1);
        live.ready(
            &mut w,
            helper,
            Ok((Box::new(Stub), tree.clone(), None, None, Vec::new())),
        );
        live.ready(
            &mut w,
            0,
            Ok((Box::new(Stub), repo.clone(), None, None, Vec::new())),
        );
        live.run_programs.push("sh".into());
        // A script outside the worktree, so changing it changes no tree.
        let outside = std::env::temp_dir().join(format!("imece-ran-{}", std::process::id()));
        std::fs::create_dir_all(&outside).unwrap();
        let script = outside.join("check.sh");
        let check = format!("sh {}", script.display());
        let mut run = |w: &mut World, who: AgentId, line: &str| {
            call(&mut live, w, who, "run", json!({ "command": line })).unwrap();
        };
        // Before the last edit: not on the tree the done hands in.
        run(&mut w, 1, "sh -c 'exit 0'");
        std::fs::write(tree.join("a.txt"), "a\nb\n").unwrap();
        // A run that changed the tree ran on none of them.
        run(&mut w, 1, "sh -c 'echo x > made.txt'");
        std::fs::remove_file(tree.join("made.txt")).unwrap();
        // The last of each command counts: failed first, clean after.
        std::fs::write(&script, "exit 1").unwrap();
        run(&mut w, 1, &check);
        std::fs::write(&script, "exit 0").unwrap();
        run(&mut w, 1, &check);
        // A helper's run in the worker's worktree counts; the leader's in
        // the checkout is not kept at all.
        run(&mut w, helper, "sh -c 'exit 3'");
        run(&mut w, 0, "git status");
        live.step(&mut w);
        assert_eq!(
            live.ran.len(),
            4,
            "neither the one that changed the tree nor the leader's: {:?}",
            live.ran
        );
        sent(&mut live, &mut w, 1, &msg("lead", "done", "b")).unwrap();
        let ran = |line: &str, code| Ran {
            line: line.into(),
            code: Some(code),
            timed_out: false,
        };
        assert_eq!(
            w.change_of(1).unwrap().runs,
            [ran(&check, 0), ran("sh -c 'exit 3'", 3)]
        );
        // Another edit: what ran before it does not count for what came.
        std::fs::write(tree.join("a.txt"), "a\nc\n").unwrap();
        sent(&mut live, &mut w, 1, &msg("lead", "done", "c")).unwrap();
        assert_eq!(w.change_of(1).unwrap().runs, []);
        let _ = std::fs::remove_dir_all(&outside);
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn runs_past_the_cap_let_go_of_the_oldest() {
        let mut live = idle_live(1);
        let ran = |n: usize| Ran {
            line: format!("run {n}"),
            code: Some(0),
            timed_out: false,
        };
        for n in 0..RAN_KEEP {
            live.ran_tx.send(("t".into(), ran(n))).unwrap();
        }
        live.take_ran();
        assert_eq!(live.ran.len(), RAN_KEEP);
        live.ran_tx.send(("t".into(), ran(RAN_KEEP))).unwrap();
        live.take_ran();
        assert_eq!(live.ran.len(), RAN_KEEP);
        assert_eq!(live.ran.front().unwrap().1, ran(1));
        assert_eq!(live.ran.back().unwrap().1, ran(RAN_KEEP));
        assert_eq!(live.runs_on("t").len(), RAN_KEEP);
        assert_eq!(live.runs_on("other"), []);
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
            live.apply_change(&w, 1, Vec::new()),
            Err("imece is still reading their newer change".into())
        );
        live.reading[1] = 0;
        // Applied, then a done right after: git reads it once the apply is
        // in, so the new change holds only what came after.
        live.apply_change(&w, 1, Vec::new()).unwrap();
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
    fn a_solo_s_turn_end_shows_the_person_what_its_worktree_holds() {
        let repo = checkout("solo-turn-repo");
        let root = trees("solo-turn");
        let mut w = team(1);
        w.open_project(
            None,
            "notes".into(),
            repo.display().to_string(),
            Vendor::Claude,
            Role::Solo,
        );
        let solo = w.projects[1].leader;
        let mut live = Live::new(
            3,
            claude_at("/nonexistent/claude"),
            repo.clone(),
            root.clone(),
        );
        live.places.push(Place {
            repo: repo.clone(),
            root: root.clone(),
        });
        let tree = worktree::ensure(&repo, &root, "solo-1").unwrap().path;
        live.ready(
            &mut w,
            solo,
            Ok((Box::new(Stub), tree.clone(), None, None, Vec::new())),
        );
        assert_eq!(
            live.standing[solo],
            Some(false),
            "its row learns where it stands"
        );
        let worker_tree = worker_in(&mut live, &mut w, 1, &repo, &root);
        // Busy, its row learns where it stands as a worker's does.
        live.standing[solo] = None;
        live.stood_at[solo] = None;
        live.turns[solo] = 1;
        live.stand_busy(&w, Instant::now());
        assert_eq!(live.standing[solo], Some(false));
        live.turns[solo] = 0;
        settle(&mut live, &mut w);
        // Between turns its clean worktree follows the checkout, as a
        // worker's does: git is asked, the prompts wait meanwhile.
        live.follow(&mut w, solo);
        assert!(live.placing[solo].is_some());
        settle(&mut live, &mut w);
        let turn_ends = |live: &mut Live, w: &mut World, id: AgentId, text: &str| {
            live.turns[id] = 1;
            // Not asked by the person: a solo's turn is still its report
            // to them, as a leader's is.
            live.asked[id] = false;
            live.tx
                .send((
                    id,
                    live.generation[id],
                    AgentEvent::TurnDone {
                        ok: true,
                        text: text.into(),
                        cost: 0,
                        denied: 0,
                    },
                ))
                .unwrap();
            live.step(w);
            settle(live, w);
        };
        // Nothing changed: the answer reaches the person, no change waits.
        turn_ends(&mut live, &mut w, solo, "Read it.\nNothing to change.");
        assert!(w.journal.iter().any(|e| matches!(
            &e.event, Event::Replied { agent, text } if *agent == solo && text == "Read it.\nNothing to change."
        )));
        assert!(w.change_of(solo).is_none());
        assert!(
            w.agents[solo]
                .trace
                .iter()
                .any(|l| l.ends_with("change: none"))
        );
        // A change: the turn's first line titles it, the person sees it.
        std::fs::write(tree.join("a.txt"), "a\nb\n").unwrap();
        turn_ends(&mut live, &mut w, solo, "Added b.\nMore words.");
        let c = w.change_of(solo).expect("a change waits");
        assert_eq!((c.title.as_str(), c.files.len()), ("Added b.", 1));
        // Next to the answer itself, the change waits for the person.
        let changes: Vec<usize> = (w.open_inbox().into_iter())
            .filter(|&i| w.inbox[i].kind == InboxKind::Change)
            .collect();
        assert_eq!(changes.len(), 1);
        assert_eq!(w.inbox[changes[0]].agent, solo);
        assert!(
            w.open_inbox()
                .iter()
                .any(|&i| w.inbox[i].kind == InboxKind::Reply)
        );
        // A worker's turn end shows nothing: its done does.
        std::fs::write(worker_tree.join("a.txt"), "changed").unwrap();
        turn_ends(&mut live, &mut w, 1, "Changed a.");
        assert!(w.change_of(1).is_none());
        // A turn that ends with nothing to say (a command) reads nothing.
        std::fs::write(tree.join("a.txt"), "a\nb\nc\n").unwrap();
        turn_ends(&mut live, &mut w, solo, "");
        assert_eq!(w.change_of(solo).unwrap().title, "Added b.", "the old one");
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
        live.ready(
            &mut w,
            helper,
            Ok((Box::new(Stub), tree.clone(), None, None, Vec::new())),
        );
        live.ready(
            &mut w,
            0,
            Ok((Box::new(Stub), repo.clone(), None, None, Vec::new())),
        );
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
            live.apply_change(&w, 1, Vec::new()),
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
        live.ready(
            &mut w,
            2,
            Ok((Box::new(Stub), tree.clone(), None, None, Vec::new())),
        );
        live.ready(
            &mut w,
            0,
            Ok((Box::new(Stub), repo.clone(), None, None, Vec::new())),
        );
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
        live.ready(
            &mut w,
            1,
            Ok((session(false), tree.clone(), None, None, Vec::new())),
        );
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
        live.ready(
            &mut w,
            1,
            Ok((session(true), tree.clone(), None, None, Vec::new())),
        );
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
    fn a_projects_files_are_listed_at_its_opening_then_while_busy_and_a_cut_list_is_said_once() {
        let repo = checkout("files-repo");
        let mut w = team(1);
        let mut live = idle_live_in(&repo, 2);
        let now = Instant::now();
        // Never read: read at once, whoever is busy.
        live.files_due(&w, now);
        assert!(live.filing.contains(&0) && live.filed_at.contains_key(&0));
        live.files_due(&w, now);
        assert_eq!(live.filing.len(), 1, "one listing at a time");
        for _ in 0..500 {
            live.take_git(&mut w);
            if !live.filing.contains(&0) {
                break;
            }
            thread::sleep(Duration::from_millis(10));
        }
        assert_eq!(w.projects[0].files, ["a.txt"]);
        assert!(w.journal.is_empty(), "not cut");
        // Read: again only while an agent is mid-turn, every FILES_EVERY.
        std::fs::write(repo.join("b.txt"), "b").unwrap();
        live.filed_at.insert(0, now);
        live.files_due(&w, now + FILES_EVERY);
        assert!(!live.filing.contains(&0), "nobody busy");
        live.turns[1] = 1;
        live.files_due(&w, now + FILES_EVERY - Duration::from_millis(1));
        assert!(!live.filing.contains(&0), "not yet");
        live.files_due(&w, now + FILES_EVERY);
        assert!(live.filing.contains(&0));
        for _ in 0..500 {
            live.take_git(&mut w);
            if !live.filing.contains(&0) {
                break;
            }
            thread::sleep(Duration::from_millis(10));
        }
        assert_eq!(w.projects[0].files, ["a.txt", "b.txt"]);
        assert_eq!(FILES_EVERY, Duration::from_secs(60));
        // A list cut at FILES_MAX is said once in the journal; a listing
        // that fails keeps the last list and is traced on the leader.
        live.filed(&mut w, 0, Ok((vec!["a.txt".into()], true)));
        live.filed(&mut w, 0, Ok((vec!["a.txt".into()], true)));
        let cuts: Vec<&Event> = w
            .journal
            .iter()
            .map(|e| &e.event)
            .filter(|e| matches!(e, Event::FilesCut { .. }))
            .collect();
        assert_eq!(
            cuts,
            [&Event::FilesCut {
                leader: 0,
                kept: FILES_MAX
            }]
        );
        assert_eq!(FILES_MAX, 20_000);
        live.filed(&mut w, 0, Err(io::Error::other("boom")));
        assert_eq!(w.projects[0].files, ["a.txt"]);
        assert!(
            w.agents[0].trace.iter().any(|l| l.ends_with("files: boom")),
            "{:?}",
            w.agents[0].trace
        );
        // A project without a place (not opened live) is not read.
        live.files(7);
        assert!(!live.filing.contains(&7));
        let _ = std::fs::remove_dir_all(&repo);
    }

    #[test]
    fn a_worker_s_row_learns_where_its_worktree_stands() {
        let repo = checkout("stand-repo");
        let root = trees("stand");
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
        assert_eq!(live.standing[1], Some(false), "as its session starts");
        settle(&mut live, &mut w);
        assert_eq!(w.agents[1].stand, Stand::default());
        // A helper's turn ends: its worker's row counts what it wrote.
        std::fs::write(tree.join("new.txt"), "1\n2\n").unwrap();
        live.turns[2] = 1;
        live.tx
            .send((
                2,
                live.generation[2],
                AgentEvent::TurnDone {
                    ok: true,
                    text: String::new(),
                    cost: 0,
                    denied: 0,
                },
            ))
            .unwrap();
        live.step(&mut w);
        settle(&mut live, &mut w);
        assert_eq!(
            w.agents[1].stand,
            Stand {
                added: 2,
                ..Stand::default()
            }
        );
        assert_eq!(w.agents[2].stand, Stand::default(), "the helper has none");
        // A leader has no worktree to count, even in a directory.
        live.cwds[0] = Some(repo.clone());
        live.stand(&w, 0);
        assert_eq!(live.standing[0], None);

        // Asked again while git reads: once more after, never lost.
        live.stand(&w, 2);
        assert_eq!(live.standing[1], Some(false));
        live.stand(&w, 1);
        assert_eq!(live.standing[1], Some(true));
        settle(&mut live, &mut w);
        live.standing[1] = Some(true);
        let read = worktree::Stand {
            branch: Some("fix".into()),
            ahead: 1,
            behind: 2,
            added: 3,
            removed: 4,
        };
        live.stood(&mut w, 1, Ok(read));
        assert_eq!(
            w.agents[1].stand,
            Stand {
                branch: Some("fix".into()),
                ahead: 1,
                behind: 2,
                added: 3,
                removed: 4,
            }
        );
        assert_eq!(live.standing[1], Some(false), "read once more");
        settle(&mut live, &mut w);
        assert_eq!(w.agents[1].stand.added, 2);
        live.standing[1] = Some(false);
        live.stood(&mut w, 1, Err(io::Error::other("boom")));
        assert_eq!(live.standing[1], None);
        assert_eq!(w.agents[1].stand.added, 2, "a failed read keeps the last");
        assert!(
            w.agents[1].trace.iter().any(|l| l.ends_with("stand: boom")),
            "{:?}",
            w.agents[1].trace
        );

        // While it or a helper works, every STAND_EVERY; between turns,
        // at the turn's end only.
        let now = Instant::now();
        live.stood_at[1] = Some(now - STAND_EVERY);
        live.stand_busy(&w, now);
        assert_eq!(live.standing[1], None, "between turns");
        live.turns[2] = 1;
        live.stood_at[1] = Some(now - STAND_EVERY + Duration::from_millis(1));
        live.stand_busy(&w, now);
        assert_eq!(live.standing[1], None, "not yet");
        live.stood_at[1] = Some(now - STAND_EVERY);
        live.stand_busy(&w, now);
        assert_eq!(live.standing[1], Some(false), "its helper works");
        live.stand_busy(&w, now);
        assert_eq!(
            live.standing[1],
            Some(false),
            "one read under way is enough"
        );
        settle(&mut live, &mut w);
        live.turns[2] = 0;
        live.turns[1] = 1;
        live.stood_at[1] = None;
        live.step(&mut w);
        assert_eq!(
            live.standing[1],
            Some(false),
            "it works, never read: each step looks"
        );
        settle(&mut live, &mut w);

        // After an apply and after git placed the worktree.
        live.applied(&mut w, 1, "t", Ok(()), Ok("abc1234".into()), None);
        assert_eq!(live.standing[1], Some(false), "applied");
        settle(&mut live, &mut w);
        let generation = live.generation[1];
        live.placed(&mut w, 1, generation + 1, Ok(worktree::Placed::AtHead));
        assert_eq!(live.standing[1], None, "a session that ended hears nothing");
        live.placed(&mut w, 1, generation, Ok(worktree::Placed::AtHead));
        assert_eq!(live.standing[1], Some(false), "placed");
        settle(&mut live, &mut w);
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
        live.ready(
            &mut w,
            0,
            Ok((Box::new(Stub), repo.clone(), None, None, Vec::new())),
        );
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
