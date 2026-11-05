//! Lonca's store as this engine sees it (record 116): one thread asks the
//! daemon how each project's repository stands, so the screen never waits
//! on a socket; what it said closes a project to hires and Δ decisions or
//! opens it ([`World::store_closed`]). imece only reads here; what must be
//! signed the `lonca` program does, on a thread of its own, when the person
//! asks (record 117).

use std::path::Path;
use std::sync::mpsc::{self, Receiver, Sender, TryRecvError};
use std::thread;
use std::time::{Duration, Instant};

use imece_agent::lonca::{Down, Failed, Place};
use imece_agent::worktree;

use super::Live;
use crate::model::{ImportAsk, ProjectId, StoreDown, StoreJob, StoreSeen, World};

/// How often the store is asked. Each look is a connection and a
/// millisecond call per repository; a person sees a daemon gone or a
/// proposal come within this.
pub(super) const LOOK_EVERY: Duration = Duration::from_secs(5);

/// The repositories to look at, each with its project.
type Repos = Vec<(ProjectId, String)>;
/// What a look found of them.
type Found = Vec<(ProjectId, String, StoreSeen)>;

/// What a job came back with.
enum Done {
    Login(Result<String, Failed>),
    Import(Result<Vec<String>, Failed>),
}

/// `job` run by the `lonca` program: login in the project's checkout `dir`.
/// A daemon with no repository yet can not sign anyone in, so the first
/// import comes before any session; the same yes signs the person in then
/// (what fails here shows as `⌀ no lonca` with its word, record 117).
fn run(place: &Place, dir: &Path, job: &StoreJob) -> Done {
    match job {
        StoreJob::Login(_) => Done::Login(place.login(dir)),
        StoreJob::Import(ask) => {
            let done = place.import(&ask.repo, &ask.checkout, &ask.branch);
            if done.is_ok() && matches!(place.open(), Err(Down::SignedOut)) {
                let _ = place.login(dir);
            }
            Done::Import(done)
        }
    }
}

/// What the world's line says of why a command did not go through.
fn said(failed: Failed) -> Option<String> {
    match failed {
        Failed::NoAnswer => None,
        Failed::Said(words) => Some(words),
    }
}

/// The thread that asks the daemon, and when it is next asked.
pub(super) struct Watch {
    /// Where the daemon and the `lonca` program are, for a job.
    place: Place,
    /// The job under way: one at a time, never on the screen's thread.
    job: Option<(StoreJob, Receiver<Done>)>,
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
        let kept = place.clone();
        thread::spawn(move || {
            for repos in asked {
                if answer.send(look(&place, &repos)).is_err() {
                    return;
                }
            }
        });
        Watch {
            place: kept,
            job: None,
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

    /// What the person is asked before project `p`'s checkout goes into
    /// Lonca as `repo`; none while it is on no branch.
    pub fn import_ask(&self, p: ProjectId, repo: &str) -> Option<ImportAsk> {
        let checkout = self.places[p].repo.clone();
        let (branch, head) = worktree::on_branch(&checkout)?;
        Some(ImportAsk {
            project: p,
            repo: repo.to_string(),
            checkout,
            branch,
            head,
        })
    }

    /// Whether a `lonca` command of the person's is under way.
    pub fn store_busy(&self) -> bool {
        self.lonca.job.is_some()
    }

    /// `job` starts, unless one is under way: the screen goes on.
    pub fn store_job(&mut self, job: StoreJob) -> bool {
        if self.lonca.job.is_some() {
            return false;
        }
        let place = self.lonca.place.clone();
        let dir = self.places[job.project()].repo.clone();
        let (tell, told) = mpsc::channel();
        let asked = job.clone();
        thread::spawn(move || {
            let _ = tell.send(run(&place, &dir, &asked));
        });
        self.lonca.job = Some((job, told));
        true
    }

    /// What the job under way came to, once it has: the journal says it,
    /// and the store is asked at once. Whether the screen changed.
    fn take_store_job(&mut self, w: &mut World) -> bool {
        let Some((job, told)) = &self.lonca.job else {
            return false;
        };
        let done = match told.try_recv() {
            Ok(done) => Some(done),
            Err(TryRecvError::Empty) => return false,
            Err(TryRecvError::Disconnected) => None,
        };
        match (done, job) {
            (Some(Done::Login(Ok(human))), _) => w.store_signed_in(job.project(), human),
            (Some(Done::Import(Ok(warned))), StoreJob::Import(ask)) => {
                w.store_imported(ask, warned);
            }
            (Some(Done::Login(Err(e)) | Done::Import(Err(e))), _) => {
                w.store_failed(job, said(e));
            }
            _ => w.store_failed(job, None),
        }
        self.lonca.job = None;
        self.look_now();
        true
    }

    /// A job's end, else what the last look found (a look that came at the
    /// same time is read at the next step). Whether the screen changed.
    pub(super) fn take_lonca(&mut self, w: &mut World) -> bool {
        if self.take_store_job(w) {
            return true;
        }
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
    use crate::live::testing::{idle_live, idle_live_in, team};
    use crate::model::{Event, StoreAct};

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

    /// The job comes back: `take_lonca` is asked until it has.
    fn job_ends(live: &mut Live, w: &mut World) {
        let started = Instant::now();
        while !live.take_lonca(w) {
            assert!(started.elapsed() < Duration::from_secs(10), "no end");
            thread::sleep(Duration::from_millis(5));
        }
    }

    fn last(w: &World) -> &Event {
        &w.journal.last().unwrap().event
    }

    fn live_with(program_body: &str) -> (Live, std::path::PathBuf) {
        let (place, dir) = imece_agent::lonca_fake::program(program_body);
        let mut live = idle_live_in(&dir, 1);
        live.lonca = Watch::start(place);
        (live, dir)
    }

    #[test]
    fn a_login_runs_in_the_checkout_one_at_a_time_and_is_told() {
        let (mut live, dir) = live_with("printf '{\"human\":\"ayse\"}\\n'");
        let mut w = team(0);
        live.lonca_after_a_look();
        assert!(!live.store_busy() && !live.asking());
        assert!(!live.take_lonca(&mut w), "nothing under way");
        assert!(live.store_job(StoreJob::Login(0)));
        assert!(live.store_busy() && live.asking());
        assert!(!live.store_job(StoreJob::Login(0)), "one at a time");
        job_ends(&mut live, &mut w);
        assert!(!live.store_busy() && !live.asking());
        assert!(live.lonca_is_due(), "asked at once");
        assert!(matches!(
            last(&w),
            Event::StoreSignedIn { leader: 0, human } if human == "ayse"
        ));
        let told = imece_agent::lonca_fake::seen(&dir);
        assert!(
            told.starts_with(&format!("{}|human login", dir.display())),
            "{told}"
        );
        std::fs::remove_dir_all(dir).unwrap();
    }

    fn ask_for(checkout: std::path::PathBuf) -> ImportAsk {
        ImportAsk {
            project: 0,
            repo: "demo".into(),
            checkout,
            branch: "main".into(),
            head: "1".repeat(40),
        }
    }

    #[test]
    fn an_import_puts_the_project_on_its_repository_with_what_lonca_warned() {
        let (mut live, dir) = live_with("echo 'warning: big' >&2");
        let mut w = team(0);
        assert!(live.store_job(StoreJob::Import(ask_for(dir.clone()))));
        job_ends(&mut live, &mut w);
        assert_eq!(w.projects[0].lonca.as_ref().unwrap().repo, "demo");
        assert_eq!(seen_of(&w, 0), StoreSeen::Asking);
        assert!(matches!(
            last(&w),
            Event::StoreImported { repo, warnings, .. }
                if repo == "demo" && warnings == &["big".to_string()]
        ));
        assert!(!live.store_busy());
        std::fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn the_first_import_signs_the_person_in_and_a_later_one_does_not() {
        let (fake, dir) = imece_agent::lonca_fake::program("exit 0");
        let d = up_daemon();
        let place = Place {
            program: fake.program,
            ..d.place.clone()
        };
        let job = StoreJob::Import(ask_for(dir.clone()));
        let last_run = || imece_agent::lonca_fake::seen(&dir);
        assert!(matches!(run(&place, &dir, &job), Done::Import(Ok(_))));
        assert!(last_run().contains("|init --import"), "{}", last_run());
        std::fs::remove_file(place.humans.join("ayse.session")).unwrap();
        assert!(matches!(run(&place, &dir, &job), Done::Import(Ok(_))));
        let login = last_run();
        assert!(login.contains("|human login --json|"), "{login}");
        std::fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn a_refused_import_signs_nobody_in() {
        let (fake, dir) = imece_agent::lonca_fake::program("exit 1");
        let d = up_daemon();
        let place = Place {
            program: fake.program,
            ..d.place.clone()
        };
        let job = StoreJob::Import(ask_for(dir.clone()));
        assert!(matches!(run(&place, &dir, &job), Done::Import(Err(_))));
        let told = imece_agent::lonca_fake::seen(&dir);
        assert!(told.contains("|init --import"), "{told}");
        std::fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn a_refused_import_is_told_and_leaves_the_project_off_lonca() {
        let (mut live, dir) = live_with("echo 'lonca: conflict: it exists' >&2\nexit 1");
        let mut w = team(0);
        live.lonca_after_a_look();
        assert!(live.store_job(StoreJob::Import(ask_for(dir.clone()))));
        job_ends(&mut live, &mut w);
        assert!(w.projects[0].lonca.is_none());
        assert!(live.lonca_is_due());
        assert!(matches!(
            last(&w),
            Event::StoreFailed { act: StoreAct::Import, why: Some(why), .. }
                if why == "conflict: it exists"
        ));
        std::fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn a_login_that_fails_says_what_it_said_and_no_program_says_nothing() {
        let (mut live, dir) = live_with("echo 'lonca: no key' >&2\nexit 1");
        let mut w = team(0);
        assert!(live.store_job(StoreJob::Login(0)));
        job_ends(&mut live, &mut w);
        assert!(matches!(
            last(&w),
            Event::StoreFailed { act: StoreAct::Login, why: Some(why), .. } if why == "no key"
        ));
        let mut place = live.lonca.place.clone();
        place.program = dir.join("nothing").display().to_string();
        live.lonca = Watch::start(place);
        assert!(live.store_job(StoreJob::Login(0)));
        job_ends(&mut live, &mut w);
        assert!(matches!(
            last(&w),
            Event::StoreFailed {
                act: StoreAct::Login,
                why: None,
                ..
            }
        ));
        std::fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn a_job_whose_thread_died_is_a_failed_one() {
        let (mut live, dir) = live_with("exit 0");
        let mut w = team(0);
        let (tell, told) = mpsc::channel();
        live.lonca.job = Some((StoreJob::Login(0), told));
        assert!(!live.take_lonca(&mut w), "still under way");
        drop(tell);
        assert!(live.take_lonca(&mut w));
        assert!(matches!(last(&w), Event::StoreFailed { why: None, .. }));
        assert!(!live.store_busy());
        std::fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn an_import_is_asked_of_a_branch_and_never_of_a_detached_head() {
        let repo = crate::live::testing::checkout("import-ask");
        let live = idle_live_in(&repo, 1);
        let git = |args: &[&str]| {
            let out = std::process::Command::new("git")
                .args(args)
                .current_dir(&repo)
                .output()
                .unwrap();
            assert!(out.status.success());
            String::from_utf8_lossy(&out.stdout).trim().to_string()
        };
        git(&["checkout", "--quiet", "-b", "trunk-to-be"]);
        let ask = live.import_ask(0, "demo").expect("on a branch");
        assert_eq!(ask.project, 0);
        assert_eq!(ask.repo, "demo");
        assert_eq!(ask.checkout, repo);
        assert_eq!(ask.branch, "trunk-to-be");
        assert_eq!(ask.head, git(&["rev-parse", "HEAD"]));
        git(&["checkout", "--quiet", "--detach"]);
        assert_eq!(live.import_ask(0, "demo"), None);
        std::fs::remove_dir_all(&repo).unwrap();
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
