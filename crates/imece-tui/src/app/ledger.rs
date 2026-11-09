//! The cost ledger's window (`:ledger`, record 133): what each attempt at
//! a card cost, by project, the row at hand opening its card.

use super::*;

impl App {
    /// `:ledger`: the window, at its first row.
    pub(super) fn open_ledger(&mut self) {
        self.ledger_win = Some(0);
    }

    /// The window's keys: `j k` (the arrows) step, `g G` (Home, End) go to
    /// the ends, `enter` opens the row's card (a row with no card: its
    /// worker's focus), `esc` `q` close, `?` the help; the rest do nothing,
    /// so the screen under it does not move.
    pub(super) fn ledger_key(&mut self, at: usize, k: KeyEvent) {
        let rows = ui::ledger::rows(&self.world);
        let last = rows.len().saturating_sub(1);
        let at = match k.code {
            KeyCode::Down | KeyCode::Char('j') => (at + 1).min(last),
            KeyCode::Up | KeyCode::Char('k') => at.saturating_sub(1),
            KeyCode::Char('g') | KeyCode::Home => 0,
            KeyCode::Char('G') | KeyCode::End => last,
            KeyCode::Enter => return self.ledger_open(rows.get(at).copied()),
            KeyCode::Esc | KeyCode::Char('q') => {
                self.ledger_win = None;
                return;
            }
            KeyCode::Char('?') => return self.open_help(),
            _ => return,
        };
        self.ledger_win = Some(at);
    }

    /// `enter` on ledger row `row`: its card's screen, else its worker's
    /// focus; the window closes.
    fn ledger_open(&mut self, row: Option<usize>) {
        let Some(row) = row else {
            return;
        };
        self.ledger_win = None;
        let r = &self.world.ledger[row];
        match r.card {
            Some(c) => self.open_card(c),
            None => {
                let id = r.agent;
                self.selected = Sel::Agent(id);
                self.zoom = Zoom::Focus(id);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::super::tests::*;
    use super::*;
    use crate::ledger::{Attempt, Outcome};

    #[test]
    fn the_ledger_window_steps_its_rows_and_enter_opens_a_card_or_a_worker() {
        let mut app = App::new(1, 1, 0, 1, Lang::En);
        let p = app.world.salon_order()[0];
        let lead = app.world.projects[p].leader;
        // Nothing listed: enter does nothing, the window stays.
        app.command("ledger");
        app.on_key(key(KeyCode::Enter));
        assert_eq!(app.ledger_win, Some(0));
        app.on_key(key(KeyCode::Char('j')));
        assert_eq!(app.ledger_win, Some(0), "no row to go to");
        let c = crate::live::testing::with_card(&mut app.world, lead);
        let row = |card| Attempt {
            project: p,
            card,
            agent: lead,
            ..Attempt::default()
        };
        // Newest first: 2 (no card), 1 (the card), 0 (no card).
        app.world.ledger = vec![row(None), row(Some(c)), row(None)];
        for (code, at) in [
            (KeyCode::Char('j'), 1),
            (KeyCode::Down, 2),
            (KeyCode::Char('j'), 2),
            (KeyCode::Char('k'), 1),
            (KeyCode::Up, 0),
            (KeyCode::Char('k'), 0),
            (KeyCode::Char('G'), 2),
            (KeyCode::Char('g'), 0),
            (KeyCode::End, 2),
            (KeyCode::Home, 0),
            (KeyCode::Char('x'), 0),
        ] {
            app.on_key(key(code));
            assert_eq!(app.ledger_win, Some(at), "{code:?}");
        }
        assert!(!app.help);
        app.on_key(key(KeyCode::Char('?')));
        assert!(app.help, "the help over it");
        app.on_key(key(KeyCode::Esc));
        assert_eq!(app.ledger_win, Some(0), "the help closed, the window stays");
        // A row with a card: the card's screen.
        app.on_key(key(KeyCode::Char('j')));
        app.on_key(key(KeyCode::Enter));
        assert_eq!((app.ledger_win, app.card), (None, Some(c)));
        // One with none: its worker's focus.
        app.card = None;
        app.zoom = Zoom::None;
        app.command("ledger");
        app.on_key(key(KeyCode::Enter));
        assert_eq!(app.ledger_win, None);
        assert_eq!((app.zoom, app.card), (Zoom::Focus(lead), None));
        assert_eq!(app.selected, Sel::Agent(lead));
        // esc and q close it.
        for code in [KeyCode::Esc, KeyCode::Char('q')] {
            app.zoom = Zoom::None;
            app.command("ledger");
            app.on_key(key(code));
            assert_eq!(app.ledger_win, None, "{code:?}");
            assert_eq!(app.zoom, Zoom::None);
        }
    }

    #[test]
    fn the_demo_books_every_dollar_of_a_worker_on_exactly_one_row() {
        use crate::model::Role;
        let mut app = App::new(2, 4, 2, 1, Lang::En);
        for _ in 0..400 {
            app.step();
        }
        let w = &app.world;
        assert!(w.ledger.len() > 2, "{:?}", w.ledger);
        assert!(
            w.ledger
                .iter()
                .any(|r| r.card.is_some() && r.outcome != Outcome::Open)
        );
        assert!(
            (w.ledger.iter()).all(|r| r.model.starts_with("claude-")),
            "the model its session said"
        );
        for p in 0..w.projects.len() {
            let rows: u64 = (w.ledger.iter())
                .filter(|r| r.project == p)
                .map(|r| r.cost)
                .sum();
            let spent: u64 = (w.agents.iter())
                .filter(|a| {
                    a.project == p && matches!(a.role, Role::Worker | Role::Helper | Role::Solo)
                })
                .map(|a| a.cost)
                .sum();
            assert_eq!(rows, spent, "project {p}");
        }
    }
}
