//! Projects: `:open`, `:close` and `:rename`, each with its own place
//! for worktrees.

use super::*;

/// Where the worktrees of the checkout `repo` live: under `state`, named
/// after the checkout and a hash of its path, so two checkouts with one
/// name do not share.
pub(super) fn root_for(state: &Path, repo: &Path) -> PathBuf {
    state.join("worktrees").join(format!(
        "{}-{:08x}",
        dir_name(repo),
        fnv(repo.as_os_str().as_encoded_bytes())
    ))
}

impl Live {
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
        self.bind(id, Role::Leader, self.lead)
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
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;
    use std::sync::atomic::{AtomicBool, Ordering};

    use super::*;
    use crate::live::testing::*;

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
        live.bind(4, Role::Leader, Vendor::Claude).unwrap();
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
        sent(&mut live, &mut w, 2, &msg("lead-two", "done", "Changed a")).unwrap();
        live.apply_change(&w, 2).unwrap();
        settle(&mut live, &mut w);
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
        let p1 = system_prompt(&w, 2, 4, 2, Confine::Strict);
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
        let p0 = system_prompt(&w, 0, 4, 2, Confine::Strict);
        assert!(p0.contains("The workers so far are claude-1."), "{p0}");
        let p1 = system_prompt(&w, 2, 4, 2, Confine::Strict);
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
        assert_eq!(names, ["send", "post", "board", "run", "hire", "approve"]);
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
}
