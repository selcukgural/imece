//! Lonca's store as this engine sees it (record 116): one thread asks the
//! daemon how each project's repository stands, so the screen never waits
//! on a socket; what it said closes a project to hires and Δ decisions or
//! opens it ([`World::store_closed`]). imece only reads here.

use std::sync::mpsc::{self, Receiver, Sender};
use std::thread;
use std::time::{Duration, Instant};

use imece_agent::lonca::{Down, Place};

use super::Live;
use crate::model::{ProjectId, StoreDown, StoreSeen, World};

/// How often the store is asked. Each look is a connection and a
/// millisecond call per repository; a person sees a daemon gone or a
/// proposal come within this.
pub(super) const LOOK_EVERY: Duration = Duration::from_secs(5);

/// The repositories to look at, each with its project.
type Repos = Vec<(ProjectId, String)>;
/// What a look found of them.
type Found = Vec<(ProjectId, String, StoreSeen)>;

/// The thread that asks the daemon, and when it is next asked.
pub(super) struct Watch {
    ask: Sender<Repos>,
    back: Receiver<Found>,
    /// A look is under way: never a second, a slow daemon gets no queue.
    out: bool,
    /// When the next look is due; none: at once.
    next: Option<Instant>,
}

impl Watch {
    pub(super) fn start(place: Place) -> Watch {
        let (ask, asked) = mpsc::channel::<Repos>();
        let (answer, back) = mpsc::channel();
        thread::spawn(move || {
            for repos in asked {
                if answer.send(look(&place, &repos)).is_err() {
                    return;
                }
            }
        });
        Watch {
            ask,
            back,
            out: false,
            next: None,
        }
    }
}

/// How `repos` stand: one connection, a status for each. A daemon that can
/// not be opened leaves them all down for the same reason.
fn look(place: &Place, repos: &Repos) -> Found {
    let mut client = place.open();
    (repos.iter())
        .map(|(p, repo)| {
            let told = client.as_mut().map_err(|down| *down);
            let seen = match told.and_then(|c| c.status(repo)) {
                Ok(status) => StoreSeen::Up {
                    trunk: status.trunk,
                    pending: status.pending.len(),
                },
                Err(down) => StoreSeen::Down(down_of(down)),
            };
            (*p, repo.clone(), seen)
        })
        .collect()
}

/// The screen's word for why the client can not read the store.
fn down_of(down: Down) -> StoreDown {
    match down {
        Down::NoDaemon => StoreDown::NoDaemon,
        Down::NotTheDaemon => StoreDown::NotTheDaemon,
        Down::Protocol => StoreDown::Protocol,
        Down::SignedOut => StoreDown::SignedOut,
        Down::Forbidden => StoreDown::Forbidden,
        Down::NoRepo => StoreDown::NoRepo,
        Down::Refused => StoreDown::Refused,
    }
}

impl Live {
    /// The store is asked when a project is on Lonca and it is time.
    pub(super) fn lonca_due(&mut self, w: &World, now: Instant) {
        let watch = &mut self.lonca;
        if watch.out || watch.next.is_some_and(|at| now < at) {
            return;
        }
        let repos = w.stores();
        if !repos.is_empty() && watch.ask.send(repos).is_ok() {
            watch.out = true;
            watch.next = Some(now + LOOK_EVERY);
        }
    }

    /// What the last look found. Whether the screen changed.
    pub(super) fn take_lonca(&mut self, w: &mut World) -> bool {
        let Ok(found) = self.lonca.back.try_recv() else {
            return false;
        };
        self.lonca.out = false;
        let mut changed = false;
        for (p, repo, seen) in found {
            changed |= w.store_seen(p, &repo, seen);
        }
        changed
    }

    /// The next step asks at once: the person just named a repository.
    pub fn look_now(&mut self) {
        self.lonca.next = None;
    }

    /// A look was just made, for a test.
    #[cfg(test)]
    pub fn lonca_after_a_look(&mut self) {
        self.lonca.next = Some(Instant::now() + LOOK_EVERY);
    }

    /// Whether the next step asks, for a test.
    #[cfg(test)]
    pub fn lonca_is_due(&self) -> bool {
        self.lonca.next.is_none()
    }
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use imece_agent::lonca::PROTOCOL;
    use imece_agent::lonca_fake::{Daemon, TRUNK, status};

    use super::*;
    use crate::live::testing::{idle_live, team};

    const WAIT: Duration = Duration::from_secs(10);

    fn up_daemon() -> Daemon {
        let d = Daemon::start(PROTOCOL, |asked| {
            if asked["repo"] == "demo" {
                Ok(status(&[7, 8]))
            } else {
                Err("not_found")
            }
        });
        d.sign_in();
        d
    }

    fn seen_of(w: &World, p: ProjectId) -> StoreSeen {
        w.projects[p].lonca.as_ref().unwrap().seen.clone()
    }

    /// Steps until the look under way came back, what it brought kept.
    fn settle(live: &mut Live, w: &mut World) {
        let till = Instant::now() + WAIT;
        while live.lonca.out {
            live.take_lonca(w);
            assert!(Instant::now() < till, "no answer");
            std::thread::sleep(Duration::from_millis(2));
        }
    }

    #[test]
    fn a_look_reads_every_repository_on_one_connection() {
        let d = up_daemon();
        let repos = vec![(0, "demo".to_string()), (3, "other".to_string())];
        let found = look(&d.place, &repos);
        assert_eq!(found.len(), 2);
        let up = StoreSeen::Up {
            trunk: TRUNK.into(),
            pending: 2,
        };
        assert_eq!(found[0], (0, "demo".to_string(), up));
        let none = StoreSeen::Down(StoreDown::NoRepo);
        assert_eq!(found[1], (3, "other".to_string(), none));
    }

    #[test]
    fn a_daemon_that_can_not_be_opened_leaves_every_repository_down() {
        let d = up_daemon();
        let mut place = d.place.clone();
        place.socket = d.dir.join("nobody");
        let repos = vec![(0, "demo".to_string()), (1, "other".to_string())];
        let found = look(&place, &repos);
        let down = StoreSeen::Down(StoreDown::NoDaemon);
        assert_eq!(found[0].2, down);
        assert_eq!(found[1].2, down);
    }

    #[test]
    fn every_reason_keeps_its_word() {
        for (down, word) in [
            (Down::NoDaemon, StoreDown::NoDaemon),
            (Down::NotTheDaemon, StoreDown::NotTheDaemon),
            (Down::Protocol, StoreDown::Protocol),
            (Down::SignedOut, StoreDown::SignedOut),
            (Down::Forbidden, StoreDown::Forbidden),
            (Down::NoRepo, StoreDown::NoRepo),
            (Down::Refused, StoreDown::Refused),
        ] {
            assert_eq!(down_of(down), word);
        }
    }

    #[test]
    fn the_store_is_asked_at_its_pace_and_never_twice_at_once() {
        let d = up_daemon();
        let mut w = team(0);
        let mut live = idle_live(1);
        live.lonca = Watch::start(d.place.clone());
        let t0 = Instant::now();
        // Nothing on Lonca: nothing is asked, and it stays due.
        live.lonca_due(&w, t0);
        assert!(!live.lonca.out);
        assert_eq!(live.lonca.next, None);

        w.on_lonca(0, Some("demo".into()));
        live.lonca_due(&w, t0);
        assert!(live.lonca.out);
        assert_eq!(live.lonca.next, Some(t0 + LOOK_EVERY));
        // Under way: no second look, even when it is time.
        live.lonca_due(&w, t0 + LOOK_EVERY);
        assert_eq!(live.lonca.next, Some(t0 + LOOK_EVERY));
        settle(&mut live, &mut w);
        assert_eq!(
            seen_of(&w, 0),
            StoreSeen::Up {
                trunk: TRUNK.into(),
                pending: 2
            }
        );
        // Before its time, none; at its time, one.
        live.lonca_due(&w, t0 + LOOK_EVERY - Duration::from_millis(1));
        assert!(!live.lonca.out);
        live.lonca_due(&w, t0 + LOOK_EVERY);
        assert!(live.lonca.out);
        settle(&mut live, &mut w);
        assert_eq!(d.heard.lock().unwrap().len(), 2);
        // The person named a repository: at once.
        live.look_now();
        live.lonca_due(&w, t0 + LOOK_EVERY + Duration::from_millis(1));
        assert!(live.lonca.out);
        settle(&mut live, &mut w);
    }

    #[test]
    fn what_a_look_found_says_whether_the_screen_changed() {
        let d = up_daemon();
        let mut w = team(0);
        let mut live = idle_live(1);
        live.lonca = Watch::start(d.place.clone());
        assert!(!live.take_lonca(&mut w), "nothing out");
        w.on_lonca(0, Some("demo".into()));
        let ask = |live: &mut Live, w: &World| {
            live.look_now();
            live.lonca_due(w, Instant::now());
        };
        let wait = |live: &mut Live, w: &mut World| {
            let till = Instant::now() + WAIT;
            loop {
                let changed = live.take_lonca(w);
                if !live.lonca.out {
                    return changed;
                }
                assert!(Instant::now() < till, "no answer");
                std::thread::sleep(Duration::from_millis(2));
            }
        };
        ask(&mut live, &w);
        assert!(wait(&mut live, &mut w), "the first answer");
        ask(&mut live, &w);
        assert!(!wait(&mut live, &mut w), "the same again");
    }

    #[test]
    fn a_look_that_changed_any_project_changed_the_screen() {
        let d = up_daemon();
        let mut w = team(0);
        let second = w.open_project(None, "b".into(), "/b".into(), crate::model::Role::Leader);
        let mut live = idle_live(1);
        live.lonca = Watch::start(d.place.clone());
        w.on_lonca(0, Some("demo".into()));
        w.on_lonca(second, Some("demo".into()));
        live.lonca_due(&w, Instant::now());
        let till = Instant::now() + WAIT;
        let mut changed = false;
        while live.lonca.out {
            changed |= live.take_lonca(&mut w);
            assert!(Instant::now() < till, "no answer");
            std::thread::sleep(Duration::from_millis(2));
        }
        assert!(changed, "both changed, not one undoing the other");
        assert_eq!(seen_of(&w, second), seen_of(&w, 0));
    }

    #[test]
    fn a_step_asks_and_the_step_that_brings_the_answer_says_something_came() {
        let d = up_daemon();
        let mut w = team(0);
        let mut live = idle_live(1);
        live.lonca = Watch::start(d.place.clone());
        w.on_lonca(0, Some("demo".into()));
        live.step(&mut w);
        assert!(live.lonca.out, "the first step asks");
        let till = Instant::now() + WAIT;
        loop {
            let came = live.step(&mut w);
            if w.store_closed(0).is_none() {
                assert!(came, "the answer was not said to have come");
                break;
            }
            assert!(Instant::now() < till, "no answer");
            std::thread::sleep(Duration::from_millis(2));
        }
    }

    #[test]
    fn a_project_on_a_store_that_does_not_answer_hires_nobody() {
        let mut w = team(1);
        let live = idle_live(2);
        assert!(live.hire_room(&w, 0).is_ok());
        w.on_lonca(0, Some("demo".into()));
        let said = live.hire_room(&w, 0).unwrap_err();
        assert!(said.contains("imece is still asking Lonca"), "{said}");
        w.store_seen(0, "demo", StoreSeen::Down(StoreDown::NoDaemon));
        let said = live.hire_room(&w, 0).unwrap_err();
        assert_eq!(
            said,
            "nobody is hired now: no Lonca daemon answers; only the person can bring Lonca back"
        );
        // A worker's helpers too, and `whoami` says what `hire` would.
        assert!(live.hire_room(&w, 1).is_err());
        assert!(
            live.whoami(&w, 0)
                .contains("may hire: no: nobody is hired now")
        );
        let up = StoreSeen::Up {
            trunk: TRUNK.into(),
            pending: 0,
        };
        w.store_seen(0, "demo", up);
        assert!(live.hire_room(&w, 0).is_ok());
    }

    #[test]
    fn a_project_on_a_store_that_does_not_answer_decides_no_change_and_forks_nobody() {
        let mut w = team(1);
        let mut live = idle_live(2);
        w.on_lonca(0, Some("demo".into()));
        w.store_seen(0, "demo", StoreSeen::Down(StoreDown::NoDaemon));
        let why = Err(StoreDown::NoDaemon.why().to_string());
        assert_eq!(live.apply_change(&w, 1, Vec::new()), why);
        assert_eq!(live.reject_change(&mut w, 1), why);
        assert_eq!(
            live.fork(&mut w, 1).unwrap_err(),
            "nobody is hired now: no Lonca daemon answers; only the person can bring Lonca back"
        );
    }
}
