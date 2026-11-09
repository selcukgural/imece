//! `:doctor` (record 138): `imece doctor`'s report in a window over the
//! screen, for the project at hand. A live run looks on a thread of its
//! own ([`doctor::look`]) and the window says so until the facts come;
//! the demo has a machine of its own ([`doctor::demo`]). The facts are
//! kept and worded when drawn, so the window follows the language.

use super::*;
use crate::doctor::{self, Ask};

/// The `:doctor` window: its first line shown, and whether every finding
/// is listed (else only those of a category with an issue).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct DoctorWin {
    pub top: usize,
    pub verbose: bool,
}

impl App {
    /// `:doctor`: the window, and a look at once.
    pub(crate) fn open_doctor(&mut self) {
        self.doctor = Some(DoctorWin::default());
        self.look_doctor();
    }

    /// Looks at the machine for the project at hand; one look at a time.
    /// What the last look found stays on the window meanwhile.
    fn look_doctor(&mut self) {
        if self.doctor_looking.is_some() {
            return;
        }
        let p = self.current_project();
        match (&self.engine, self.doctor_look) {
            (Engine::Live(live), Some(look)) => {
                let ask = Ask {
                    repo: live.repo_of(p).to_path_buf(),
                    state: live.state().to_path_buf(),
                    terminal: self.terminal.clone(),
                };
                self.doctor_looking = Some(look(ask));
            }
            (Engine::Sim(_), _) => {
                let checkout = self.project_folder(p).unwrap_or_default();
                let repo = (self.world.projects[p].lonca.as_ref()).map(|l| l.repo.clone());
                self.doctor_facts = Some(doctor::demo(self.seen.clone(), checkout, repo));
            }
            (Engine::Live(_) | Engine::View(_), _) => {}
        }
    }

    /// Whether a look is under way.
    pub fn doctor_looking(&self) -> bool {
        self.doctor_looking.is_some()
    }

    /// The facts a look brought since last time, onto the window.
    pub fn doctored(&mut self) {
        let Some(rx) = &self.doctor_looking else {
            return;
        };
        match rx.try_recv() {
            Ok(facts) => self.doctor_facts = Some(facts),
            Err(TryRecvError::Empty) => return,
            // A look whose thread died found nothing: what was known stays.
            Err(TryRecvError::Disconnected) => {}
        }
        self.doctor_looking = None;
        self.changed = true;
    }

    /// A key while `:doctor` is open: `j k` (the arrows) a line, `^d ^u`
    /// (PgDn, PgUp) half a window, `g G` (Home, End) the ends, `v` every
    /// finding or only what has an issue, `r` looks again, `esc` or `q`
    /// close, `?` the help; the rest do nothing, so the screen under it
    /// does not move.
    pub(super) fn doctor_key(&mut self, win: DoctorWin, k: KeyEvent) {
        let (lines, room) = self.doctor_room.get();
        let last = lines.saturating_sub(room);
        let half = (room / 2).max(1);
        let ctrl = k.modifiers.contains(KeyModifiers::CONTROL);
        let top = match k.code {
            KeyCode::Char('d') if ctrl => win.top.saturating_add(half).min(last),
            KeyCode::Char('u') if ctrl => win.top.saturating_sub(half),
            _ if ctrl => return,
            KeyCode::PageDown => win.top.saturating_add(half).min(last),
            KeyCode::PageUp => win.top.saturating_sub(half),
            KeyCode::Down | KeyCode::Char('j') => win.top.saturating_add(1).min(last),
            KeyCode::Up | KeyCode::Char('k') => win.top.saturating_sub(1),
            KeyCode::Char('g') | KeyCode::Home => 0,
            KeyCode::Char('G') | KeyCode::End => last,
            KeyCode::Char('v') => {
                self.doctor = Some(DoctorWin {
                    top: 0,
                    verbose: !win.verbose,
                });
                return;
            }
            KeyCode::Char('r') => return self.look_doctor(),
            KeyCode::Esc | KeyCode::Char('q') => {
                self.doctor = None;
                return;
            }
            KeyCode::Char('?') => return self.open_help(),
            _ => return,
        };
        self.doctor = Some(DoctorWin { top, ..win });
    }
}

#[cfg(test)]
mod tests {
    use super::super::tests::*;
    use super::*;
    use crate::doctor::{Facts, Mark};

    #[test]
    fn the_demo_s_doctor_opens_with_its_machine_for_the_project_at_hand() {
        let mut app = App::new(2, 1, 0, 1, Lang::En);
        assert!(app.doctor.is_none() && app.doctor_facts.is_none());
        app.command("doctor");
        assert_eq!(app.doctor, Some(DoctorWin::default()));
        assert!(!app.doctor_looking(), "the demo has its facts at once");
        let facts = app.doctor_facts.as_ref().unwrap();
        let p = app.current_project();
        assert_eq!(Some(&facts.checkout), app.project_folder(p).as_ref());
        // What the demo's welcome says of Claude Code is what its doctor says.
        assert_eq!(facts.claude.seen, app.seen);
        let report = doctor::report(app.lang, facts);
        assert_eq!((report.issues(), report.failed()), (1, false));
        assert_eq!(report.categories[0].mark(), Mark::Warn);
        // A project on Lonca says its repository.
        app.world.on_lonca(p, Some("shop".into()));
        app.on_key(key(KeyCode::Char('r')));
        let lonca = app.doctor_facts.as_ref().unwrap().lonca.as_ref().unwrap();
        assert_eq!(lonca.repo.as_deref(), Some("shop"));
    }

    #[test]
    fn the_doctor_window_scrolls_within_its_lines_and_takes_every_key() {
        let mut app = App::new(1, 1, 0, 1, Lang::En);
        app.command("doctor");
        let at = |app: &App| app.doctor.unwrap().top;
        // Not drawn yet: one line of room, none to go to.
        app.on_key(key(KeyCode::Char('j')));
        assert_eq!(at(&app), 0);
        // 30 lines in a room of 10: the last first line is 20.
        app.doctor_room.set((30, 10));
        for (k, top) in [
            (key(KeyCode::Char('j')), 1),
            (key(KeyCode::Down), 2),
            (key(KeyCode::Char('k')), 1),
            (key(KeyCode::Up), 0),
            (key(KeyCode::Char('k')), 0),
            (ctrl('d'), 5),
            (key(KeyCode::PageDown), 10),
            (ctrl('u'), 5),
            (key(KeyCode::PageUp), 0),
            (ctrl('u'), 0),
            (key(KeyCode::Char('G')), 20),
            (key(KeyCode::Char('j')), 20),
            (ctrl('d'), 20),
            (key(KeyCode::Char('g')), 0),
            (key(KeyCode::End), 20),
            (key(KeyCode::Home), 0),
            // Another control key, and a key of the screen under it.
            (ctrl('f'), 0),
            (key(KeyCode::Char('W')), 0),
        ] {
            app.on_key(k);
            assert_eq!(at(&app), top, "{k:?}");
        }
        assert_eq!(app.zoom, Zoom::None, "the screen under it did not move");
        // A room of one line pages a line at a time.
        app.doctor_room.set((3, 1));
        app.on_key(ctrl('d'));
        assert_eq!(at(&app), 1);
        // `v` lists every finding, from the top, and back.
        app.on_key(key(KeyCode::Char('v')));
        assert_eq!(
            app.doctor,
            Some(DoctorWin {
                top: 0,
                verbose: true
            })
        );
        app.on_key(key(KeyCode::Char('j')));
        app.on_key(key(KeyCode::Char('v')));
        assert_eq!(app.doctor, Some(DoctorWin::default()));
        // The help opens over it and closes back to it.
        app.on_key(key(KeyCode::Char('?')));
        assert!(app.help && app.doctor.is_some());
        app.on_key(key(KeyCode::Esc));
        assert!(!app.help && app.doctor.is_some());
        for close in [KeyCode::Esc, KeyCode::Char('q')] {
            app.command("doctor");
            app.on_key(key(close));
            assert!(app.doctor.is_none(), "{close:?}");
        }
        assert!(!app.quit, "q closed the window, not imece");
    }

    fn live_app() -> App {
        let w = crate::live::testing::team(1);
        let live = crate::live::testing::idle_live(2);
        App::with(w, Engine::Live(Box::new(live)), Lang::En)
    }

    /// A look whose answer the test holds: sent when it says so.
    fn held(_: Ask) -> doctor::Answers {
        let (tx, rx) = std::sync::mpsc::channel();
        HELD.with(|h| h.borrow_mut().push(tx));
        rx
    }

    thread_local! {
        static HELD: RefCell<Vec<std::sync::mpsc::Sender<Facts>>> =
            const { RefCell::new(Vec::new()) };
    }

    fn facts(checkout: &str) -> Facts {
        doctor::demo(Seen::Missing, checkout.into(), None)
    }

    #[test]
    fn a_live_look_comes_on_its_own_time_and_one_at_a_time() {
        let mut app = live_app();
        app.doctor_look = Some(held);
        app.command("doctor");
        assert!(app.doctor.is_some() && app.doctor_looking());
        assert!(app.doctor_facts.is_none(), "nothing known yet");
        // Asked again meanwhile: the look under way is the one.
        app.on_key(key(KeyCode::Char('r')));
        assert_eq!(HELD.with(|h| h.borrow().len()), 1);
        app.changed = false;
        app.doctored();
        assert!(app.doctor_looking() && !app.changed, "no answer yet");
        HELD.with(|h| h.borrow()[0].send(facts("~/one")).unwrap());
        app.doctored();
        assert!(!app.doctor_looking() && app.changed, "drawn again with it");
        assert_eq!(app.doctor_facts.as_ref().unwrap().checkout, "~/one");
        // `r` looks again; what was found stays until the new comes.
        app.on_key(key(KeyCode::Char('r')));
        assert!(app.doctor_looking());
        assert_eq!(app.doctor_facts.as_ref().unwrap().checkout, "~/one");
        HELD.with(|h| h.borrow()[1].send(facts("~/two")).unwrap());
        app.doctored();
        assert_eq!(app.doctor_facts.as_ref().unwrap().checkout, "~/two");
        // A look that died leaves what was known, and the next can start.
        app.on_key(key(KeyCode::Char('r')));
        HELD.with(|h| h.borrow_mut().clear());
        app.changed = false;
        app.doctored();
        assert!(!app.doctor_looking() && app.changed);
        assert_eq!(app.doctor_facts.as_ref().unwrap().checkout, "~/two");
        // Without a way to look (a test's live app), nothing is asked.
        app.doctor_look = None;
        app.on_key(key(KeyCode::Char('r')));
        assert!(!app.doctor_looking());
    }

    #[test]
    fn a_live_look_is_asked_about_the_project_s_checkout_and_imece_s_state() {
        thread_local! {
            static ASKED: RefCell<Option<Ask>> = const { RefCell::new(None) };
        }
        fn noted(ask: Ask) -> doctor::Answers {
            ASKED.with(|a| *a.borrow_mut() = Some(ask));
            std::sync::mpsc::channel().1
        }
        let mut app = live_app();
        app.doctor_look = Some(noted);
        app.command("doctor");
        let ask = ASKED.with(|a| a.borrow_mut().take()).unwrap();
        let Engine::Live(live) = &app.engine else {
            panic!("a live app");
        };
        assert_eq!(ask.repo, live.repo_of(app.current_project()));
        assert_eq!(ask.state, live.state());
        assert!(ask.terminal.is_none(), "a test has no probe");
    }

    #[test]
    fn an_archive_s_view_has_no_doctor() {
        assert!(!VIEW_COMMANDS.contains(&"doctor"));
    }
}
