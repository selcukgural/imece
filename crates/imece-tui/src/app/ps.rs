//! `:ps` (record 129): each agent's session's processes in a window over
//! the screen, read again every [`PS_EVERY`] while it is open; `d` ends
//! the process at hand after a `y`, its session going on. Under them the
//! processes left from an earlier run of imece: `d` ends one, `a` all,
//! each after a `y`; a start says how many there are.

use super::*;
use crate::procs::{Proc, Refusal as ProcRefusal};

/// How often an open `:ps` window reads the processes again.
pub(crate) const PS_EVERY: Duration = Duration::from_secs(3);

/// Whose a process on the `:ps` window is.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Whose {
    /// An agent's session's.
    Agent(AgentId),
    /// The tree at this index of those left from an earlier run.
    Left(usize),
}

/// What a `y` on the `:ps` window would end.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Sure {
    /// The process `pid`, whose it is.
    One(Whose, u32),
    /// Every process left from an earlier run.
    AllLeft,
}

/// The `:ps` window: the row at hand, and what a `y` would end.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct PsWin {
    pub at: usize,
    pub sure: Option<Sure>,
}

/// Every process listed, in the window's order: each agent's session,
/// then those under it; then each tree left from an earlier run.
pub(crate) fn ps_rows(w: &World) -> Vec<(Whose, &Proc)> {
    let agents =
        (w.procs.agents.iter()).flat_map(|(id, ps)| ps.iter().map(move |p| (Whose::Agent(*id), p)));
    let left = (w.procs.left.iter().enumerate())
        .flat_map(|(i, l)| l.procs.iter().map(move |p| (Whose::Left(i), p)));
    agents.chain(left).collect()
}

/// Every process left from an earlier run, each with the first of its
/// tree.
pub(crate) fn left_pids(w: &World) -> Vec<(u32, u32)> {
    (w.procs.left.iter())
        .flat_map(|l| {
            let root = l.procs.first().map_or(0, |p| p.pid);
            l.procs.iter().map(move |p| (p.pid, root))
        })
        .collect()
}

/// The name process `pid` had when last read, whose it is; empty when it
/// was not.
pub(crate) fn whose_name(w: &World, whose: Whose, pid: u32) -> String {
    (ps_rows(w).into_iter())
        .find(|&(o, p)| o == whose && p.pid == pid)
        .map_or(String::new(), |(_, p)| p.name.clone())
}

impl App {
    /// `:ps`: the window, and a look at once.
    pub(crate) fn open_ps(&mut self) {
        self.ps = Some(PsWin::default());
        self.ps_looked = None;
        self.watch_ps(Instant::now());
    }

    /// At a start: the engine looks at the processes, and a flash says
    /// how many an earlier run left when it found any.
    pub fn start_ps(&mut self) {
        self.ps_start = true;
        self.look_procs();
    }

    fn look_procs(&mut self) {
        match &mut self.engine {
            Engine::Live(l) => l.look_procs(&mut self.world),
            Engine::Sim(s) => s.look_procs(&mut self.world),
            Engine::View(_) => {}
        }
    }

    /// Once the start's look answered (and `:tidy`'s, whose flash this one
    /// joins): how many processes an earlier run left, unless the window
    /// shows them.
    fn said_left(&mut self) {
        if !self.ps_start || self.world.procs.looking || self.tidy_start {
            return;
        }
        self.ps_start = false;
        let n = left_pids(&self.world).len();
        if n == 0 || self.ps.is_some() {
            return;
        }
        let t = self.lang.text();
        let left = fill(t.ps_left_found, &[&counted(n, t.n_processes)]);
        let tidy = (self.world.orphans.found.as_ref())
            .map(|f| fill(t.tidy_found, &[&counted(f.len(), t.n_worktrees)]));
        match &mut self.flash {
            Some((said, _)) if Some(&*said) == tidy.as_ref() => {
                said.push_str(" · ");
                said.push_str(&left);
            }
            _ => self.flash(left),
        }
    }

    /// While the window is open, the processes are read again every
    /// [`PS_EVERY`]; the row at hand stays on the list.
    pub(crate) fn watch_ps(&mut self, now: Instant) {
        self.said_left();
        let Some(win) = &mut self.ps else {
            return;
        };
        let last = ps_rows(&self.world).len().saturating_sub(1);
        win.at = win.at.min(last);
        if self
            .ps_looked
            .is_some_and(|at| now.duration_since(at) < PS_EVERY)
        {
            return;
        }
        self.ps_looked = Some(now);
        self.look_procs();
    }

    /// A key while `:ps` is open: `j k` (the arrows), `g G` move, `d` asks
    /// to end the process at hand and `a` every one left from an earlier
    /// run, and `y` ends it (any other key keeps it), `esc` or `q` close,
    /// `?` the help; the rest do nothing.
    pub(super) fn ps_key(&mut self, win: PsWin, k: KeyEvent) {
        if let Some(sure) = win.sure {
            self.ps = Some(PsWin { sure: None, ..win });
            if k.code == KeyCode::Char('y') {
                self.end_sure(sure);
            }
            return;
        }
        let rows = ps_rows(&self.world);
        let last = rows.len().saturating_sub(1);
        let at = match k.code {
            KeyCode::Down | KeyCode::Char('j') => (win.at + 1).min(last),
            KeyCode::Up | KeyCode::Char('k') => win.at.saturating_sub(1),
            KeyCode::Char('g') | KeyCode::Home => 0,
            KeyCode::Char('G') | KeyCode::End => last,
            KeyCode::Char('d') => {
                let sure = rows.get(win.at).map(|&(o, p)| Sure::One(o, p.pid));
                self.ps = Some(PsWin { sure, ..win });
                return;
            }
            KeyCode::Char('a') => {
                if self.world.procs.left.is_empty() {
                    return self.flash(self.lang.text().ps_no_left);
                }
                let sure = Some(Sure::AllLeft);
                self.ps = Some(PsWin { sure, ..win });
                return;
            }
            KeyCode::Esc | KeyCode::Char('q') => {
                self.ps = None;
                return;
            }
            KeyCode::Char('?') => return self.open_help(),
            _ => return,
        };
        self.ps = Some(PsWin { at, ..win });
    }

    /// What a `y` ends: an agent's process, one left from an earlier run,
    /// or every one left.
    fn end_sure(&mut self, sure: Sure) {
        match sure {
            Sure::One(Whose::Agent(id), pid) => self.end_proc(id, pid),
            Sure::One(Whose::Left(i), pid) => {
                let root = (self.world.procs.left.get(i))
                    .and_then(|l| l.procs.first())
                    .map_or(0, |p| p.pid);
                let name = whose_name(&self.world, Whose::Left(i), pid);
                let t = self.lang.text();
                let said = match self.end_left(&[(pid, root)]) {
                    Some(0) => t.ps_left_gone.to_string(),
                    Some(_) => fill(t.ps_left_ended_one, &[&name, &pid.to_string()]),
                    None => return,
                };
                self.flash(said);
            }
            Sure::AllLeft => {
                let asked = left_pids(&self.world);
                let t = self.lang.text();
                let said = match self.end_left(&asked) {
                    Some(0) => t.ps_left_gone.to_string(),
                    Some(n) => fill(t.ps_left_ended, &[&n.to_string()]),
                    None => return,
                };
                self.flash(said);
            }
        }
    }

    /// Ends the processes `asked` left from an earlier run: how many were;
    /// none in an archive's view.
    fn end_left(&mut self, asked: &[(u32, u32)]) -> Option<usize> {
        match &mut self.engine {
            Engine::Live(l) => Some(l.end_left(&mut self.world, asked)),
            Engine::Sim(s) => Some(s.end_left(&mut self.world, asked)),
            Engine::View(_) => {
                self.read_only();
                None
            }
        }
    }

    /// Ends process `pid` under agent `id`'s session; what came of it is
    /// said.
    fn end_proc(&mut self, id: AgentId, pid: u32) {
        let t = self.lang.text();
        let name = crate::live::proc_name(&self.world, id, pid);
        let done = match &mut self.engine {
            Engine::Live(l) => l.end_proc(&mut self.world, id, pid),
            Engine::Sim(s) => s.end_proc(&mut self.world, id, pid),
            Engine::View(_) => return self.read_only(),
        };
        let agent = self.world.agents[id].name.clone();
        let said = match done {
            Ok(()) => fill(t.ev_killed, &[&name, &pid.to_string(), &agent]),
            Err(ProcRefusal::Session) => fill(t.ps_session, &[&agent]),
            Err(ProcRefusal::Bus) => fill(t.ps_bus, &[&agent]),
            Err(ProcRefusal::NotUnder) => fill(t.ps_gone, &[&agent]),
        };
        self.flash(said);
    }
}

#[cfg(test)]
mod tests {
    use super::super::tests::*;
    use super::*;

    /// A demo of one project and two workers, a few ticks in, with `:ps`
    /// open.
    fn opened() -> App {
        let mut app = App::new(1, 2, 0, 1, Lang::En);
        for _ in 0..30 {
            app.step();
        }
        app.command("ps");
        app
    }

    fn rows(app: &App) -> Vec<(Whose, u32, String)> {
        (ps_rows(&app.world).into_iter())
            .map(|(id, p)| (id, p.pid, p.name.clone()))
            .collect()
    }

    fn at(app: &App) -> usize {
        app.ps.unwrap().at
    }

    #[test]
    fn ps_lists_each_session_and_moves_over_its_processes() {
        let mut app = opened();
        let all = rows(&app);
        assert!(all.len() >= 4, "{all:?}");
        assert_eq!(all[0].2, "claude");
        assert_eq!(all[1].2, "imece");
        app.on_key(key(KeyCode::Char('j')));
        assert_eq!(at(&app), 1);
        app.on_key(key(KeyCode::Down));
        assert_eq!(at(&app), 2);
        app.on_key(key(KeyCode::Char('k')));
        app.on_key(key(KeyCode::Up));
        app.on_key(key(KeyCode::Up));
        assert_eq!(at(&app), 0, "not above the first");
        app.on_key(key(KeyCode::Char('G')));
        assert_eq!(at(&app), all.len() - 1);
        app.on_key(key(KeyCode::Char('j')));
        assert_eq!(at(&app), all.len() - 1, "not past the last");
        app.on_key(key(KeyCode::Char('g')));
        assert_eq!(at(&app), 0);
        app.on_key(key(KeyCode::End));
        assert_eq!(at(&app), all.len() - 1);
        app.on_key(key(KeyCode::Home));
        assert_eq!(at(&app), 0);
        app.on_key(key(KeyCode::Char('x')));
        assert_eq!(app.ps, Some(PsWin::default()), "nothing else acts");
        app.on_key(key(KeyCode::Char('?')));
        assert!(app.help);
        app.help = false;
        app.on_key(key(KeyCode::Char('q')));
        assert_eq!(app.ps, None);
        app.command("ps");
        app.on_key(key(KeyCode::Esc));
        assert_eq!(app.ps, None);
    }

    #[test]
    fn d_asks_y_ends_and_the_session_and_its_bus_stay() {
        let mut app = opened();
        let all = rows(&app);
        let (whose, root, _) = all[0].clone();
        let Whose::Agent(id) = whose else {
            unreachable!()
        };
        let name = app.world.agents[id].name.clone();
        // Its session: asked, then refused.
        app.on_key(key(KeyCode::Char('d')));
        assert_eq!(app.ps.unwrap().sure, Some(Sure::One(whose, root)));
        app.on_key(key(KeyCode::Char('n')));
        assert_eq!(app.ps.unwrap().sure, None, "any other key keeps it");
        assert_eq!(rows(&app), all);
        app.on_key(key(KeyCode::Char('d')));
        app.on_key(key(KeyCode::Char('y')));
        assert_eq!(
            said(&app),
            format!("that is {name}'s session itself: :forget or a stop ends it, not :ps")
        );
        // Its relay.
        app.on_key(key(KeyCode::Char('j')));
        app.on_key(key(KeyCode::Char('d')));
        app.on_key(key(KeyCode::Char('y')));
        assert_eq!(
            said(&app),
            format!("that is imece's bus for {name}: it stays")
        );
        assert_eq!(rows(&app), all);
        // A command under it ends, and stays gone.
        let i = (all.iter())
            .position(|r| r.0 == whose && !["claude", "imece"].contains(&r.2.as_str()))
            .expect("a working agent runs a command");
        let (_, pid, cmd) = all[i].clone();
        app.ps = Some(PsWin { at: i, sure: None });
        app.on_key(key(KeyCode::Char('d')));
        app.on_key(key(KeyCode::Char('y')));
        assert_eq!(said(&app), format!("you ended {cmd} ({pid}) under {name}"));
        assert!(!rows(&app).iter().any(|r| r.1 == pid));
        assert!((app.world.journal.iter()).any(|e| e.event
            == Event::Killed {
                agent: id,
                pid,
                name: cmd.clone()
            }));
        // One no longer there is said so.
        app.ps = Some(PsWin {
            at: 0,
            sure: Some(Sure::One(whose, pid)),
        });
        app.on_key(key(KeyCode::Char('y')));
        assert_eq!(
            said(&app),
            format!("that process is no longer under {name}'s session")
        );
    }

    #[test]
    fn an_open_window_reads_again_every_few_seconds_and_a_shut_one_never() {
        let mut app = opened();
        let t0 = Instant::now();
        app.ps_looked = Some(t0);
        app.world.procs.agents.clear();
        app.world.procs.left.clear();
        app.ps = Some(PsWin { at: 7, sure: None });
        app.watch_ps(t0 + PS_EVERY - Duration::from_millis(1));
        assert!(app.world.procs.agents.is_empty());
        assert_eq!(at(&app), 0, "the row at hand stays on the list");
        app.watch_ps(t0 + PS_EVERY);
        assert!(!app.world.procs.agents.is_empty());
        assert_eq!(app.ps_looked, Some(t0 + PS_EVERY));
        app.ps = None;
        app.world.procs.agents.clear();
        app.watch_ps(t0 + PS_EVERY * 9);
        assert!(app.world.procs.agents.is_empty());
        assert_eq!(app.ps_looked, Some(t0 + PS_EVERY));
    }
    #[test]
    fn what_an_earlier_run_left_ends_one_or_all_after_a_y() {
        let mut app = opened();
        let all = rows(&app);
        let first = (all.iter())
            .position(|r| matches!(r.0, Whose::Left(_)))
            .expect("the demo left some");
        assert!(all[..first].iter().all(|r| matches!(r.0, Whose::Agent(_))));
        assert_eq!(all.len() - first, 4, "{all:?}");
        assert_eq!(left_pids(&app.world).len(), 4);
        assert_eq!(
            left_pids(&app.world)[..3],
            [(39_000, 39_000), (39_001, 39_000), (39_002, 39_000)]
        );
        assert_eq!(whose_name(&app.world, Whose::Left(0), 39_002), "cargo");
        assert_eq!(whose_name(&app.world, Whose::Left(1), 39_002), "");
        assert_eq!(whose_name(&app.world, Whose::Left(1), 39_100), "node");
        // `d` on one of them, kept, then ended.
        let cargo = first + 2;
        app.ps = Some(PsWin {
            at: cargo,
            sure: None,
        });
        app.on_key(key(KeyCode::Char('d')));
        assert_eq!(
            app.ps.unwrap().sure,
            Some(Sure::One(Whose::Left(0), 39_002))
        );
        app.on_key(key(KeyCode::Char('n')));
        assert_eq!(rows(&app), all, "kept");
        app.on_key(key(KeyCode::Char('d')));
        app.on_key(key(KeyCode::Char('y')));
        assert_eq!(
            said(&app),
            "you ended cargo (39002), left from an earlier run"
        );
        assert_eq!(rows(&app).len(), all.len() - 1);
        // Asked again, it is no longer there.
        app.ps = Some(PsWin {
            at: 0,
            sure: Some(Sure::One(Whose::Left(0), 39_002)),
        });
        app.on_key(key(KeyCode::Char('y')));
        assert_eq!(said(&app), "that is no longer left from an earlier run");
        // `a`: all of them, kept by another key.
        app.on_key(key(KeyCode::Char('a')));
        assert_eq!(app.ps.unwrap().sure, Some(Sure::AllLeft));
        app.on_key(key(KeyCode::Esc));
        assert!(app.ps.is_some(), "esc keeps it and the window");
        assert_eq!(left_pids(&app.world).len(), 3);
        app.on_key(key(KeyCode::Char('a')));
        app.on_key(key(KeyCode::Char('y')));
        assert_eq!(said(&app), "you ended 3 processes left from an earlier run");
        assert_eq!(app.world.procs.left, vec![]);
        assert_eq!(rows(&app).len(), first, "the agents' stay");
        app.on_key(key(KeyCode::Char('a')));
        assert_eq!(app.ps.unwrap().sure, None);
        assert_eq!(said(&app), "nothing is left from an earlier run");
        app.ps = Some(PsWin {
            at: 0,
            sure: Some(Sure::AllLeft),
        });
        app.on_key(key(KeyCode::Char('y')));
        assert_eq!(said(&app), "that is no longer left from an earlier run");
        assert!(
            !(app.world.journal.iter()).any(|e| matches!(e.event, Event::Killed { .. })),
            "the agents' journal says nothing of them"
        );
    }

    #[test]
    fn a_start_says_what_an_earlier_run_left_after_tidy_s_word() {
        let mut app = App::new(1, 2, 0, 1, Lang::En);
        app.start_tidy();
        app.start_ps();
        assert!(app.ps_start, "the demo answers at the next step");
        app.step();
        let tidy = fill(
            Lang::En.text().tidy_found,
            &[&counted(
                app.world.orphans.found.as_ref().unwrap().len(),
                Lang::En.text().n_worktrees,
            )],
        );
        assert_eq!(
            said(&app),
            format!("{tidy} · 4 processes left from an earlier run: :ps lists them")
        );
        assert!(!app.ps_start);
        app.step();
        assert_eq!(
            said(&app),
            format!("{tidy} · 4 processes left from an earlier run: :ps lists them"),
            "said once"
        );
        // Alone when tidy said nothing; nothing while the window shows
        // them, nothing while the look is under way or nothing is left.
        let mut app = App::new(1, 2, 0, 1, Lang::En);
        app.start_ps();
        app.step();
        assert_eq!(
            said(&app),
            "4 processes left from an earlier run: :ps lists them"
        );
        let mut app = App::new(1, 2, 0, 1, Lang::En);
        app.start_ps();
        app.command("ps");
        app.flash = None;
        app.step();
        assert_eq!(app.flash, None);
        assert!(!app.ps_start);
        let mut app = App::new(1, 2, 0, 1, Lang::En);
        app.start_ps();
        app.tidy_start = true;
        app.said_left();
        assert!(app.ps_start, "waits for tidy's word");
        app.tidy_start = false;
        app.world.procs.looking = true;
        app.said_left();
        assert!(app.ps_start, "waits for the look");
        app.world.procs.looking = false;
        app.world.procs.left.clear();
        app.said_left();
        assert!(!app.ps_start);
        assert_eq!(app.flash, None);
        // Another flash meanwhile stays, the count after it.
        let mut app = App::new(1, 2, 0, 1, Lang::En);
        app.start_ps();
        app.flash("something else");
        app.step();
        assert_eq!(
            said(&app),
            "4 processes left from an earlier run: :ps lists them"
        );
    }
}
