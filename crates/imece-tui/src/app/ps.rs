//! `:ps` (record 129): each agent's session's processes in a window over
//! the screen, read again every [`PS_EVERY`] while it is open; `d` ends
//! the process at hand after a `y`, its session going on.

use super::*;
use crate::procs::{Proc, Refusal as ProcRefusal};

/// How often an open `:ps` window reads the processes again.
pub(crate) const PS_EVERY: Duration = Duration::from_secs(3);

/// The `:ps` window: the row at hand, and the process a `y` would end.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct PsWin {
    pub at: usize,
    pub sure: Option<(AgentId, u32)>,
}

/// Every process listed, in the window's order: each agent's session,
/// then those under it.
pub(crate) fn ps_rows(w: &World) -> Vec<(AgentId, &Proc)> {
    (w.procs.agents.iter())
        .flat_map(|(id, ps)| ps.iter().map(move |p| (*id, p)))
        .collect()
}

impl App {
    /// `:ps`: the window, and a look at once.
    pub(crate) fn open_ps(&mut self) {
        self.ps = Some(PsWin::default());
        self.ps_looked = None;
        self.watch_ps(Instant::now());
    }

    /// While the window is open, the processes are read again every
    /// [`PS_EVERY`]; the row at hand stays on the list.
    pub(crate) fn watch_ps(&mut self, now: Instant) {
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
        match &mut self.engine {
            Engine::Live(l) => l.look_procs(&mut self.world),
            Engine::Sim(s) => s.look_procs(&mut self.world),
            Engine::View(_) => {}
        }
    }

    /// A key while `:ps` is open: `j k` (the arrows), `g G` move, `d` asks
    /// to end the process at hand and `y` ends it (any other key keeps
    /// it), `esc` or `q` close, `?` the help; the rest do nothing.
    pub(super) fn ps_key(&mut self, win: PsWin, k: KeyEvent) {
        if let Some((id, pid)) = win.sure {
            self.ps = Some(PsWin { sure: None, ..win });
            if k.code == KeyCode::Char('y') {
                self.end_proc(id, pid);
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
                let sure = rows.get(win.at).map(|&(id, p)| (id, p.pid));
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

    fn rows(app: &App) -> Vec<(AgentId, u32, String)> {
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
        let (id, root, _) = all[0].clone();
        let name = app.world.agents[id].name.clone();
        // Its session: asked, then refused.
        app.on_key(key(KeyCode::Char('d')));
        assert_eq!(app.ps.unwrap().sure, Some((id, root)));
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
            .position(|r| r.0 == id && !["claude", "imece"].contains(&r.2.as_str()))
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
            sure: Some((id, pid)),
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
}
