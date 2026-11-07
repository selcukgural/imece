//! A project on Lonca: `:lonca`, `:sync` and the import.

use super::*;

impl App {
    /// `:lonca` says how the project at hand stands on Lonca, `:lonca
    /// NAME` names its repository (the daemon is asked at once, and until
    /// it answers the project is closed: record 116), `:lonca off` leaves
    /// it, `:lonca login` signs the person in and `:lonca import [NAME]`
    /// asks to make a repository of the checkout (record 117).
    pub(super) fn lonca_command(&mut self, arg: &str) {
        let t = self.lang.text();
        let p = self.current_project();
        let name = self.world.projects[p].name.clone();
        match arg {
            "" => self.flash(self.lonca_state(p)),
            "off" => {
                self.world.on_lonca(p, None);
                self.flash(fill(t.lonca_off, &[&name]));
            }
            "login" => self.store_start(StoreJob::Login(p), t.lonca_signing_in.to_string()),
            "import" => self.lonca_import(p, &name),
            word if word.starts_with("import ") => {
                self.lonca_import(p, word["import ".len()..].trim());
            }
            repo if !valid_repo(repo) => self.flash(fill(t.lonca_bad, &[repo])),
            repo => {
                self.world.on_lonca(p, Some(repo.to_string()));
                if let Engine::Live(l) = &mut self.engine {
                    l.look_now();
                }
                self.flash(fill(t.lonca_named, &[&name, repo]));
            }
        }
    }

    /// `:sync` brings trunk to the checkout of the project at hand: a
    /// project on Lonca whose store answers, with a checkout to bring it
    /// to. `lonca fetch`, then a fast-forward; nothing else moves.
    pub(super) fn sync_command(&mut self) {
        let t = self.lang.text();
        let p = self.current_project();
        let project = &self.world.projects[p];
        let name = project.name.clone();
        let Some(on) = &project.lonca else {
            return self.flash(fill(t.lonca_none, &[&name]));
        };
        if project.no_git {
            return self.flash(fill(t.sync_no_git, &[&name]));
        }
        let repo = on.repo.clone();
        if self.world.store_closed(p).is_some() {
            return self.flash(self.lonca_state(p));
        }
        let word = fill(t.syncing, &[&repo]);
        self.store_start(StoreJob::Sync { project: p, repo }, word);
    }

    /// `:lonca import`: the window that asks, unless the checkout can not
    /// go. It goes as `repo`.
    fn lonca_import(&mut self, p: ProjectId, repo: &str) {
        let t = self.lang.text();
        let project = &self.world.projects[p];
        let name = project.name.clone();
        if let Some(on) = &project.lonca {
            return self.flash(fill(t.lonca_import_on, &[&name, &on.repo]));
        }
        if project.no_git {
            return self.flash(fill(t.lonca_import_no_git, &[&name]));
        }
        if !valid_repo(repo) {
            return self.flash(fill(t.lonca_bad, &[repo]));
        }
        let (asked, busy) = match &self.engine {
            Engine::Live(l) => (l.import_ask(p, repo), l.store_busy()),
            Engine::Sim(s) => (s.import_ask(&self.world, p, repo), s.store_busy()),
            Engine::View(_) => return self.read_only(),
        };
        if busy {
            return self.flash(t.lonca_busy);
        }
        match asked {
            Some(ask) => self.importing = Some(ask),
            None => self.flash(fill(t.lonca_import_no_branch, &[&name])),
        }
    }

    /// A key while an import is asked about: `a` is the person's yes, `esc`
    /// or `q` closes the window and nothing goes, `?` the help; the rest do
    /// nothing, the same as on the other windows that ask.
    pub(super) fn import_key(&mut self, k: KeyEvent) {
        match k.code {
            KeyCode::Char('a') => {
                let Some(ask) = self.importing.take() else {
                    return;
                };
                let word = fill(self.lang.text().lonca_importing, &[&ask.repo]);
                self.store_start(StoreJob::Import(ask), word);
            }
            KeyCode::Esc | KeyCode::Char('q') => {
                self.importing = None;
                self.flash(self.lang.text().lonca_import_held);
            }
            KeyCode::Char('?') => self.open_help(),
            _ => {}
        }
    }

    /// The engine starts `job` and the bottom line says `word` while it
    /// runs; one at a time.
    fn store_start(&mut self, job: StoreJob, word: String) {
        let started = match &mut self.engine {
            Engine::Live(l) => l.store_job(job),
            Engine::Sim(s) => s.store_job(&self.world, job),
            Engine::View(_) => return self.read_only(),
        };
        if started {
            self.asking = Some(word.clone());
            self.flash(word);
        } else {
            self.flash(self.lang.text().lonca_busy);
        }
    }

    /// How project `p` stands on Lonca, in words.
    fn lonca_state(&self, p: ProjectId) -> String {
        let t = self.lang.text();
        let project = &self.world.projects[p];
        let name = &project.name;
        let Some(on) = &project.lonca else {
            return fill(t.lonca_none, &[name]);
        };
        match &on.seen {
            StoreSeen::Asking => fill(t.lonca_named, &[name, &on.repo]),
            StoreSeen::Up {
                trunk,
                pending,
                behind,
            } => {
                let up = fill(
                    t.lonca_state_up,
                    &[
                        name,
                        &on.repo,
                        ui::words::short_commit(trunk),
                        &pending.to_string(),
                    ],
                );
                match behind {
                    Behind::By(0) | Behind::Unknown => up,
                    Behind::By(n) => fill(t.lonca_state_behind, &[&up, &n.to_string()]),
                    Behind::Never => fill(t.lonca_state_never, &[&up]),
                }
            }
            StoreSeen::Down(why) => fill(
                t.lonca_state_down,
                &[name, &on.repo, ui::words::store_down(t, *why)],
            ),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::super::tests::*;
    use super::*;

    #[test]
    fn lonca_names_a_repository_says_how_it_stands_and_leaves_it() {
        let mut app = App::with(
            crate::live::testing::team(0),
            Engine::Live(Box::new(crate::live::testing::idle_live(1))),
            Lang::En,
        );
        let name = app.world.projects[0].name.clone();
        assert_eq!(app.command_note("lonca"), "");
        app.command("lonca");
        assert_eq!(
            flashed(&app).unwrap(),
            format!(
                "{name} is on no Lonca repository; :lonca NAME names one, :lonca import makes one"
            )
        );
        app.command("lonca a/b");
        assert_eq!(flashed(&app).unwrap(), "a/b can not be a repository's name");
        assert!(app.world.projects[0].lonca.is_none());
        // Named: the next step asks at once, the project closed till then.
        if let Engine::Live(l) = &mut app.engine {
            l.look_now();
        }
        app.command("lonca  demo ");
        let asked = format!("{name} is on Lonca repository demo now; imece asks its daemon");
        assert_eq!(flashed(&app).unwrap(), asked);
        assert_eq!(app.command_note("lonca"), "(demo)");
        assert!(app.world.store_closed(0).is_some());
        app.command("lonca");
        assert_eq!(flashed(&app).unwrap(), asked, "still asking");
        let up = StoreSeen::Up {
            trunk: "693d62952cd8f7ce58b0de780cf5d998b2f5c1a6".into(),
            pending: 2,
            behind: Behind::By(0),
        };
        app.world.store_seen(0, "demo", up);
        app.command("lonca");
        assert_eq!(
            flashed(&app).unwrap(),
            format!("{name} is on Lonca repository demo: trunk 693d629, 2 waiting for a human")
        );
        app.world
            .store_seen(0, "demo", StoreSeen::Down(StoreDown::SignedOut));
        app.command("lonca");
        assert_eq!(
            flashed(&app).unwrap(),
            format!("{name} is on Lonca repository demo: you are not signed in (:lonca login)")
        );
        app.command("lonca off");
        assert_eq!(
            flashed(&app).unwrap(),
            format!("{name} is on no Lonca repository now")
        );
        assert!(app.world.projects[0].lonca.is_none());
    }

    #[test]
    fn naming_a_repository_makes_the_next_step_ask_at_once() {
        let mut app = App::with(
            crate::live::testing::team(0),
            Engine::Live(Box::new(crate::live::testing::idle_live(1))),
            Lang::En,
        );
        if let Engine::Live(l) = &mut app.engine {
            l.lonca_after_a_look();
            assert!(!l.lonca_is_due());
        }
        app.command("lonca demo");
        if let Engine::Live(l) = &app.engine {
            assert!(l.lonca_is_due());
        }
    }

    #[test]
    fn lonca_is_refused_in_an_archives_view() {
        assert!(!VIEW_COMMANDS.contains(&"lonca"));
    }

    #[test]
    fn the_demo_takes_lonca_as_live_does() {
        let (world, sim) = crate::sim::world(1, 1, 0, 1, Lang::En);
        let mut app = App::with(world, Engine::Sim(sim), Lang::En);
        app.command("lonca nobody");
        assert_eq!(app.world.stores().len(), 1);
    }

    /// A demo of one project, on no repository.
    fn off_lonca() -> App {
        let (mut world, sim) = crate::sim::world(1, 1, 0, 1, Lang::En);
        world.on_lonca(0, None);
        App::with(world, Engine::Sim(sim), Lang::En)
    }

    /// Steps until the engine has nothing the person asked for.
    fn store_job_ends(app: &mut App) {
        for _ in 0..8 {
            app.step();
            if app.asking.is_none() {
                return;
            }
        }
        panic!("the job never ended");
    }

    /// A demo whose first project is on repository `shop`, up.
    fn on_shop() -> App {
        let mut app = off_lonca();
        app.world.on_lonca(0, Some("shop".into()));
        app.world.store_seen(0, "shop", shop_up(Behind::By(2)));
        app
    }

    fn shop_up(behind: Behind) -> StoreSeen {
        StoreSeen::Up {
            trunk: "693d62952cd8".into(),
            pending: 1,
            behind,
        }
    }

    #[test]
    fn sync_brings_trunk_to_the_project_at_hand() {
        let mut app = on_shop();
        app.command("sync");
        assert_eq!(
            flashed(&app).as_deref(),
            Some("bringing trunk of shop to the checkout…")
        );
        assert!(app.asking.is_some());
        app.command("sync");
        assert_eq!(
            flashed(&app).as_deref(),
            Some("a lonca command is under way")
        );
    }

    #[test]
    fn sync_is_refused_off_lonca_without_git_and_while_the_store_is_down() {
        let mut app = off_lonca();
        let name = app.world.projects[0].name.clone();
        app.command("sync");
        assert!(
            flashed(&app)
                .unwrap()
                .starts_with(&format!("{name} is on no Lonca"))
        );
        assert!(app.asking.is_none());

        let mut app = on_shop();
        app.world.projects[0].no_git = true;
        app.command("sync");
        assert_eq!(
            flashed(&app).unwrap(),
            format!("{name} is no git checkout: there is nothing to bring trunk to")
        );
        assert!(app.asking.is_none());

        let mut app = on_shop();
        app.world
            .store_seen(0, "shop", StoreSeen::Down(StoreDown::NoDaemon));
        app.command("sync");
        assert_eq!(
            flashed(&app).unwrap(),
            format!("{name} is on Lonca repository shop: no daemon answers (lonca daemon)")
        );
        assert!(app.asking.is_none());
        assert!(!VIEW_COMMANDS.contains(&"sync"));
    }

    #[test]
    fn lonca_says_how_far_behind_the_checkout_is_and_the_menu_too() {
        let mut app = on_shop();
        let name = app.world.projects[0].name.clone();
        let up =
            format!("{name} is on Lonca repository shop: trunk 693d629, 1 waiting for a human");
        for (behind, said, note) in [
            (
                Behind::By(2),
                format!("{up}; your checkout is 2 behind (:sync)"),
                "(↓2)",
            ),
            (
                Behind::Never,
                format!("{up}; your checkout has commits trunk never had"),
                "(↓?)",
            ),
            (Behind::By(0), up.clone(), ""),
            (Behind::Unknown, up.clone(), ""),
        ] {
            app.world.store_seen(0, "shop", shop_up(behind));
            app.command("lonca");
            assert_eq!(flashed(&app).unwrap(), said);
            assert_eq!(app.command_note("sync"), note);
        }
        app.world.on_lonca(0, None);
        assert_eq!(app.command_note("sync"), "");
    }

    #[test]
    fn lonca_import_asks_first_and_only_a_yes_runs_it() {
        let mut app = off_lonca();
        let project = app.world.projects[0].name.clone();
        app.command("lonca import shop");
        let ask = app.importing.clone().expect("the window");
        assert_eq!(ask.repo, "shop");
        assert_eq!(ask.project, 0);
        assert!(app.asking.is_none(), "nothing runs yet");
        // Nothing but its keys does anything.
        app.key(key(KeyCode::Char('j')));
        assert_eq!(app.importing, Some(ask.clone()));
        app.key(key(KeyCode::Esc));
        assert_eq!(app.importing, None);
        assert_eq!(flashed(&app).as_deref(), Some("nothing went into Lonca"));
        store_job_ends(&mut app);
        assert!(app.world.projects[0].lonca.is_none());

        app.command("lonca import");
        assert_eq!(
            app.importing.as_ref().unwrap().repo,
            project,
            "its own name"
        );
        app.key(key(KeyCode::Char('q')));
        assert_eq!(app.importing, None);

        app.command("lonca  import   shop ");
        app.key(key(KeyCode::Char('a')));
        assert_eq!(app.importing, None);
        let word = "importing into Lonca as shop…";
        assert_eq!(flashed(&app).as_deref(), Some(word));
        assert_eq!(app.asking.as_deref(), Some(word));
        store_job_ends(&mut app);
        assert_eq!(app.world.projects[0].lonca.as_ref().unwrap().repo, "shop");
    }

    #[test]
    fn lonca_import_is_refused_when_the_checkout_can_not_go() {
        let mut app = off_lonca();
        let name = app.world.projects[0].name.clone();
        app.command("lonca import a/b");
        assert_eq!(app.importing, None);
        assert_eq!(flashed(&app).unwrap(), "a/b can not be a repository's name");
        app.world.projects[0].no_git = true;
        app.command("lonca import shop");
        assert_eq!(app.importing, None);
        assert_eq!(
            flashed(&app).unwrap(),
            format!("{name} is no git checkout: there is nothing to import")
        );
        app.world.projects[0].no_git = false;
        app.world.on_lonca(0, Some("shop".into()));
        app.command("lonca import other");
        assert_eq!(app.importing, None);
        assert_eq!(
            flashed(&app).unwrap(),
            format!("{name} is on Lonca repository shop already")
        );
    }

    #[test]
    fn a_detached_checkout_is_told_to_take_a_branch() {
        let repo = crate::live::testing::checkout("import-detached");
        let out = std::process::Command::new("git")
            .args(["checkout", "--quiet", "--detach"])
            .current_dir(&repo)
            .output()
            .unwrap();
        assert!(out.status.success());
        let mut app = App::with(
            crate::live::testing::team(0),
            Engine::Live(Box::new(crate::live::testing::idle_live_in(&repo, 1))),
            Lang::En,
        );
        let name = app.world.projects[0].name.clone();
        app.command("lonca import shop");
        assert_eq!(app.importing, None);
        assert_eq!(
            flashed(&app).unwrap(),
            format!("{name} is on no branch: check one out to import it")
        );
        std::fs::remove_dir_all(&repo).unwrap();
    }

    #[test]
    fn one_lonca_command_at_a_time() {
        let mut app = off_lonca();
        app.command("lonca login");
        assert_eq!(flashed(&app).as_deref(), Some("signing in to Lonca…"));
        assert_eq!(app.asking.as_deref(), Some("signing in to Lonca…"));
        app.command("lonca login");
        assert_eq!(
            flashed(&app).as_deref(),
            Some("a lonca command is under way")
        );
        app.command("lonca import shop");
        assert_eq!(app.importing, None, "no window for what can not start");
        assert_eq!(
            flashed(&app).as_deref(),
            Some("a lonca command is under way")
        );
        store_job_ends(&mut app);
        assert!(matches!(
            &app.world.journal.last().unwrap().event,
            Event::StoreSignedIn { human, .. } if human == "demo"
        ));
    }

    #[test]
    fn a_yes_while_a_job_has_started_since_says_so() {
        let mut app = off_lonca();
        app.command("lonca import shop");
        if let Engine::Sim(s) = &mut app.engine {
            assert!(s.store_job(&app.world, StoreJob::Login(0)));
        }
        app.key(key(KeyCode::Char('a')));
        assert_eq!(
            flashed(&app).as_deref(),
            Some("a lonca command is under way")
        );
        assert_eq!(app.importing, None);
        assert!(app.asking.is_none());
    }

    #[test]
    fn a_window_that_asks_about_an_import_opens_the_help_on_its_key() {
        let mut app = off_lonca();
        app.command("lonca import shop");
        app.key(key(KeyCode::Char('?')));
        assert!(app.help);
        assert!(app.importing.is_some());
    }

    #[test]
    fn an_archive_view_runs_no_lonca_command() {
        let (world, sim) = crate::sim::world(1, 1, 0, 1, Lang::En);
        let mut app = App::with(world, Engine::Sim(sim), Lang::En);
        app.engine = Engine::View(Viewed {
            project: 0,
            title: String::new(),
            at: 0,
        });
        app.store_start(StoreJob::Login(0), "x".into());
        assert_eq!(app.asking, None);
        app.lonca_import(0, "shop");
        assert_eq!(app.importing, None);
    }
}
