//! The views that became windows (record 125, step 5): the board, the code
//! map and the timeline open over the screen by their `:` names alone and
//! take every key until `esc` closes them.

use super::*;

impl Window {
    /// Where a key file's keys apply while the window is open: the names
    /// the views had (`[board]`, `[code]`, `[timeline]`), so a file that
    /// names them still works.
    pub fn scope(self) -> keys::Scope {
        match self {
            Window::Board => keys::Scope::Board,
            Window::Code => keys::Scope::Code,
            Window::Timeline => keys::Scope::Timeline,
            Window::Journal => keys::Scope::Journal,
        }
    }
}

impl App {
    /// `:board`, `:code`, `:timeline`: the window over whatever is on the
    /// screen; the timeline starts on its first lane.
    pub(super) fn open_window(&mut self, win: Window) {
        if win == Window::Timeline {
            self.timeline_sel = 0;
        }
        // The journal's keys are the pane's: the pane is where they look.
        if win == Window::Journal {
            self.pane = Pane::Journal;
            if !self.story_on() {
                self.select_journal(None);
            }
        }
        self.window = Some(win);
    }

    /// A window closed: the journal's pane goes with it.
    pub(super) fn close_window(&mut self) {
        if self.window == Some(Window::Journal) {
            self.pane = Pane::Salon;
        }
        self.window = None;
    }

    /// A key while a window is open: `esc` or `q` close it, `?` opens the
    /// help; on the timeline `j k` (the arrows), `g G` (Home, End) move
    /// over the lanes, `[ ]` change the scale, `enter` opens the selected
    /// agent's focus and `@` writes to it, both closing the window; the
    /// rest do nothing, so the screen under the window does not move.
    pub(super) fn window_key(&mut self, win: Window, k: KeyEvent) {
        match k.code {
            // In the journal shown in the story's place, `esc` is the
            // story's first.
            KeyCode::Esc if win == Window::Journal && self.story_away => self.story_back(),
            KeyCode::Esc | KeyCode::Char('q') => self.close_window(),
            KeyCode::Char('?') => self.open_help(),
            _ if win == Window::Timeline => self.timeline_key(k),
            _ if win == Window::Journal => self.journal_key(k),
            _ => {}
        }
    }

    /// The journal window's keys (record 125, step 5): the pane's as they
    /// were. `j k` (the arrows) step, `g G` (Home, End) go to the ends,
    /// `^d ^u ^f ^b` page; `J` tells the story or shows the lines again;
    /// `m` marks the line at hand, `'` goes to the next mark; `c` `y` copy
    /// the line; `enter` opens the line's conversation in the window's
    /// place, or, on the story, a card's detail or the line of no card in
    /// the journal; the rest do nothing.
    fn journal_key(&mut self, k: KeyEvent) {
        if k.modifiers.contains(KeyModifiers::CONTROL) {
            match k.code {
                KeyCode::Char('d') => self.page(true, true),
                KeyCode::Char('u') => self.page(false, true),
                KeyCode::Char('f') => self.page(true, false),
                KeyCode::Char('b') => self.page(false, false),
                _ => {}
            }
            return;
        }
        match k.code {
            KeyCode::Down | KeyCode::Char('j') => self.move_selection(true),
            KeyCode::Up | KeyCode::Char('k') => self.move_selection(false),
            KeyCode::Char('g') | KeyCode::Home => self.jump(true),
            KeyCode::Char('G') | KeyCode::End => self.jump(false),
            KeyCode::Char('J') => self.toggle_story(),
            KeyCode::Char('m') => self.toggle_mark(),
            KeyCode::Char('\'') => self.next_mark(),
            KeyCode::Char('c') | KeyCode::Char('y') => {
                if let Some(i) = self.journal_sel.filter(|_| !self.story_on()) {
                    self.copy = Some(ui::copy_text(self.lang, &self.world, &[i]));
                }
            }
            KeyCode::Enter if self.story_on() => self.open_story_item(),
            KeyCode::Enter => {
                if let Some(i) = self.journal_sel {
                    self.close_window();
                    self.open_thread(i);
                }
            }
            _ => {}
        }
    }

    /// The timeline's own keys, while its window is open.
    fn timeline_key(&mut self, k: KeyEvent) {
        let last = ui::timeline::agents(self).len().saturating_sub(1);
        let scales = ui::timeline::SCALES.len() - 1;
        match k.code {
            KeyCode::Down | KeyCode::Char('j') => {
                self.timeline_sel = (self.timeline_sel + 1).min(last);
            }
            KeyCode::Up | KeyCode::Char('k') => {
                self.timeline_sel = self.timeline_sel.saturating_sub(1);
            }
            KeyCode::Char('g') | KeyCode::Home => self.timeline_sel = 0,
            KeyCode::Char('G') | KeyCode::End => self.timeline_sel = last,
            // A column of 5 s, 30 s, a minute.
            KeyCode::Char(']') => self.timeline_scale = (self.timeline_scale + 1).min(scales),
            KeyCode::Char('[') => self.timeline_scale = self.timeline_scale.saturating_sub(1),
            KeyCode::Enter => {
                if let Some(id) = self.timeline_agent() {
                    self.window = None;
                    self.zoom = Zoom::Focus(id);
                }
            }
            KeyCode::Char('@') => {
                if let Some(id) = self.timeline_agent() {
                    self.window = None;
                    self.typing = true;
                    self.input = format!("@{} ", self.world.agents[id].name);
                    self.menu = self.suggest();
                }
            }
            _ => {}
        }
    }
}

#[cfg(test)]
mod tests {
    use super::super::tests::*;
    use super::*;

    /// A demo team with lanes to show: two projects, workers at work.
    fn team() -> App {
        let mut app = App::new(2, 6, 2, 1, Lang::En);
        for _ in 0..40 {
            app.step();
        }
        app
    }

    #[test]
    fn the_colon_names_open_the_windows_and_the_digits_open_nothing() {
        let mut app = App::new(1, 1, 0, 1, Lang::En);
        for (name, win) in [
            ("board", Window::Board),
            ("code", Window::Code),
            ("timeline", Window::Timeline),
        ] {
            app.command(name);
            assert_eq!(app.window, Some(win), "{name}");
            assert_eq!(app.key_scope(), Some(win.scope()), "{name}");
            assert_eq!(app.view, View::Salon, "{name}: the view under it stays");
            app.on_key(key(KeyCode::Esc));
            assert_eq!(app.window, None, "{name}: esc closes it");
        }
        for c in ['1', '2', '3', '4', '5'] {
            app.on_key(key(KeyCode::Char(c)));
            assert_eq!((app.window, app.view), (None, View::Salon), "{c}");
        }
        app.command("timeline");
        app.on_key(key(KeyCode::Char('q')));
        assert_eq!(app.window, None, "q closes it too");
    }

    #[test]
    fn a_window_takes_every_key_and_the_help_opens_over_it() {
        let mut app = team();
        for win in [Window::Board, Window::Code] {
            app.window = Some(win);
            let before = (app.selected, app.pane, app.zoom, app.timeline_sel);
            for code in [
                KeyCode::Char('j'),
                KeyCode::Enter,
                KeyCode::Char('/'),
                KeyCode::Char('W'),
                KeyCode::Tab,
                KeyCode::Char(']'),
            ] {
                app.on_key(key(code));
                assert_eq!(app.window, Some(win), "{win:?} {code:?}");
                assert_eq!(
                    (app.selected, app.pane, app.zoom, app.timeline_sel),
                    before,
                    "{win:?} {code:?}: nothing under it moved"
                );
                assert!(!app.typing, "{win:?} {code:?}");
            }
            app.on_key(key(KeyCode::Char('?')));
            assert!(app.help, "{win:?}");
            assert_eq!(app.key_scope(), Some(keys::Scope::Help));
            assert_eq!(app.scope_below(), Some(win.scope()));
            app.on_key(key(KeyCode::Esc));
            assert!(!app.help);
            assert_eq!(app.window, Some(win), "the help closed, not the window");
            app.window = None;
        }
    }

    #[test]
    fn the_timeline_s_keys_move_over_its_lanes_and_scale_it() {
        let mut app = team();
        app.timeline_sel = 3;
        app.command("board");
        assert_eq!(app.timeline_sel, 3, "another window leaves the lane");
        app.command("timeline");
        assert_eq!(app.timeline_sel, 0, "opens on the first lane");
        let n = ui::timeline::agents(&app).len();
        assert!(n >= 3, "{n} lanes");
        app.on_key(key(KeyCode::Char('j')));
        assert_eq!(app.timeline_sel, 1);
        app.on_key(key(KeyCode::Down));
        assert_eq!(app.timeline_sel, 2);
        app.on_key(key(KeyCode::Char('G')));
        assert_eq!(app.timeline_sel, n - 1);
        app.on_key(key(KeyCode::Char('j')));
        assert_eq!(app.timeline_sel, n - 1, "stays on the last");
        app.on_key(key(KeyCode::Char('k')));
        assert_eq!(app.timeline_sel, n - 2);
        app.on_key(key(KeyCode::Up));
        assert_eq!(app.timeline_sel, n - 3);
        app.on_key(key(KeyCode::End));
        assert_eq!(app.timeline_sel, n - 1);
        app.on_key(key(KeyCode::Char('g')));
        assert_eq!(app.timeline_sel, 0);
        app.on_key(key(KeyCode::Char('k')));
        assert_eq!(app.timeline_sel, 0, "stays on the first");
        app.timeline_sel = 1;
        app.on_key(key(KeyCode::Home));
        assert_eq!(app.timeline_sel, 0);
        // The scale: 5 s, 30 s, a minute, no further; back to 5 s.
        assert_eq!(app.timeline_scale, 0);
        app.on_key(key(KeyCode::Char(']')));
        assert_eq!(app.timeline_scale, 1);
        app.on_key(key(KeyCode::Char(']')));
        assert_eq!(app.timeline_scale, 2);
        app.on_key(key(KeyCode::Char(']')));
        assert_eq!(app.timeline_scale, 2, "the last scale");
        app.on_key(key(KeyCode::Char('[')));
        assert_eq!(app.timeline_scale, 1);
        app.on_key(key(KeyCode::Char('[')));
        app.on_key(key(KeyCode::Char('[')));
        assert_eq!(app.timeline_scale, 0, "the first scale");
        // The selected agent is the one at hand while the window is open.
        app.on_key(key(KeyCode::Char('j')));
        let at = app.timeline_agent().unwrap();
        assert_eq!(app.agent_at_hand(), Some(at));
        assert_eq!(ui::timeline::agents(&app)[1].0, at);
        // `enter`: its focus, the window closed.
        app.on_key(key(KeyCode::Enter));
        assert_eq!((app.window, app.zoom), (None, Zoom::Focus(at)));
        app.zoom = Zoom::None;
        // `@`: a message to it, the window closed.
        app.command("timeline");
        app.on_key(key(KeyCode::Char('j')));
        // A stale menu from an earlier line: `@` asks for the line's own.
        app.input = "@".into();
        app.menu = app.suggest();
        assert!(app.menu.is_some());
        app.input.clear();
        app.on_key(key(KeyCode::Char('@')));
        assert_eq!(app.window, None);
        assert!(app.typing);
        assert_eq!(app.input, format!("@{} ", app.world.agents[at].name));
        assert_eq!(
            app.menu,
            app.suggest(),
            "the line's own menu, not the stale one"
        );
        app.typing = false;
        app.input.clear();
        // With no lane, `enter` and `@` do nothing.
        for a in &mut app.world.agents {
            a.gone = true;
        }
        app.command("timeline");
        app.on_key(key(KeyCode::Enter));
        assert_eq!((app.window, app.zoom), (Some(Window::Timeline), Zoom::None));
        app.on_key(key(KeyCode::Char('@')));
        assert_eq!(app.window, Some(Window::Timeline));
        assert!(!app.typing);
    }

    /// A quiet team on Home with a journal of `n` lines said by the first
    /// worker to the person.
    fn with_journal(n: usize) -> (App, AgentId) {
        let mut app = App::new(1, 2, 0, 1, Lang::En);
        app.world.inbox.clear();
        for a in &mut app.world.agents {
            a.status = Status::Idle;
        }
        app.view = View::Home;
        let worker = app.world.projects[0].leader + 1;
        app.world.journal.clear();
        // The oldest line is said to the person; the rest are events, a
        // row each (lines of one conversation fold under the first).
        app.world.said(worker, "line 0");
        for _ in 1..n {
            app.world.note(Event::Failed { agent: worker });
        }
        (app, worker)
    }

    #[test]
    fn the_journal_window_opens_by_name_or_j_with_a_line_at_hand_and_its_keys_move_it() {
        let (mut app, worker) = with_journal(4);
        assert_eq!(app.pane, Pane::Salon);
        app.on_key(key(KeyCode::Char('J')));
        assert_eq!(app.window, Some(Window::Journal));
        assert_eq!(app.key_scope(), Some(keys::Scope::Journal));
        assert_eq!(app.pane, Pane::Journal, "the pane's keys");
        let rows = app.journal_rows();
        assert_eq!(app.journal_sel, Some(rows[0].0), "the newest line at hand");
        app.on_key(key(KeyCode::Char('j')));
        assert_eq!(app.journal_sel, Some(rows[1].0));
        app.on_key(key(KeyCode::Down));
        assert_eq!(app.journal_sel, Some(rows[2].0));
        app.on_key(key(KeyCode::Char('G')));
        assert_eq!(app.journal_sel, Some(rows[3].0));
        app.on_key(key(KeyCode::Char('j')));
        assert_eq!(app.journal_sel, Some(rows[3].0), "the last stays");
        app.on_key(key(KeyCode::Char('k')));
        assert_eq!(app.journal_sel, Some(rows[2].0));
        app.on_key(key(KeyCode::Up));
        assert_eq!(app.journal_sel, Some(rows[1].0));
        app.on_key(key(KeyCode::Char('g')));
        assert_eq!(app.journal_sel, Some(rows[0].0));
        app.on_key(key(KeyCode::End));
        assert_eq!(app.journal_sel, Some(rows[3].0));
        app.on_key(key(KeyCode::Home));
        assert_eq!(app.journal_sel, Some(rows[0].0));
        app.on_key(ctrl('d'));
        assert_ne!(app.journal_sel, Some(rows[0].0), "a half page down");
        app.on_key(ctrl('u'));
        assert_eq!(app.journal_sel, Some(rows[0].0));
        app.on_key(ctrl('f'));
        assert_ne!(app.journal_sel, Some(rows[0].0), "a page down");
        app.on_key(ctrl('b'));
        assert_eq!(app.journal_sel, Some(rows[0].0));
        // `c` copies the line; the keys of Home under it do nothing.
        app.on_key(key(KeyCode::Char('G')));
        app.on_key(key(KeyCode::Char('c')));
        assert!(
            app.copy.as_deref().is_some_and(|c| c.contains("line 0")),
            "{:?}",
            app.copy
        );
        app.copy = None;
        app.on_key(key(KeyCode::Char('y')));
        assert!(app.copy.is_some());
        app.on_key(key(KeyCode::Char('g')));
        let at = app.at_home.at;
        for code in [
            KeyCode::Char('a'),
            KeyCode::Char('/'),
            KeyCode::Char('W'),
            KeyCode::Tab,
        ] {
            app.on_key(key(code));
            assert_eq!(app.window, Some(Window::Journal), "{code:?}");
            assert!(!app.typing, "{code:?}");
        }
        assert_eq!(app.at_home.at, at);
        // `q` closes it and gives the pane back; `:journal` opens it again.
        app.on_key(key(KeyCode::Char('q')));
        assert_eq!((app.window, app.pane), (None, Pane::Salon));
        app.command("journal");
        assert_eq!(
            (app.window, app.pane),
            (Some(Window::Journal), Pane::Journal)
        );
        app.on_key(key(KeyCode::Esc));
        assert_eq!((app.window, app.pane), (None, Pane::Salon));
        // `enter` opens the line's conversation in the window's place.
        app.command("journal");
        app.on_key(key(KeyCode::Char('G')));
        app.on_key(key(KeyCode::Enter));
        assert_eq!(app.window, None);
        assert!(app.thread.is_some(), "the conversation");
        assert_eq!(app.thread_agent(), Some(worker));
        assert_eq!(app.detail(), Some((worker, Tab::Talk)));
    }

    #[test]
    fn marks_are_set_and_followed_in_the_journal_window() {
        let (mut app, _) = with_journal(3);
        let rows = app.journal_rows();
        // Off the window `m` has nothing at hand.
        app.on_key(key(KeyCode::Char('m')));
        assert!(app.world.marks.is_empty());
        assert_eq!(flashed(&app).as_deref(), Some(Lang::En.text().mark_where));
        app.command("journal");
        app.on_key(key(KeyCode::Char('j')));
        app.on_key(key(KeyCode::Char('m')));
        assert_eq!(app.world.marks, BTreeSet::from([rows[1].0]));
        app.on_key(key(KeyCode::Esc));
        // `'` from Home opens the window at the next mark.
        app.on_key(key(KeyCode::Char('\'')));
        assert_eq!(app.window, Some(Window::Journal));
        assert_eq!(app.journal_sel, Some(rows[1].0));
        // With the story on, the lines show in its place until `esc`.
        app.on_key(key(KeyCode::Esc));
        app.story = true;
        app.on_key(key(KeyCode::Char('\'')));
        assert!(app.story_away && !app.story_on());
        assert_eq!(app.journal_sel, Some(rows[1].0));
        app.on_key(key(KeyCode::Esc));
        assert!(app.story_on(), "the story again, the window still open");
        assert_eq!(app.window, Some(Window::Journal));
    }

    #[test]
    fn a_card_s_number_opens_its_detail_and_the_team_commands_go_home() {
        let mut app = App::new(1, 2, 0, 1, Lang::En);
        for _ in 0..60 {
            app.step();
        }
        app.view = View::Home;
        app.world.inbox.clear();
        for a in &mut app.world.agents {
            a.status = Status::Idle;
        }
        app.command("board");
        assert!(!app.world.cards.is_empty(), "the demo has cards");
        let c = 0;
        let owner = app.world.projects[0].leader + 1;
        app.world.cards[c].owner = Some(owner);
        app.open_card(c);
        assert_eq!(app.window, None, "a window closes for the card");
        assert_eq!(
            (app.card, app.zoom, app.pane),
            (Some(c), Zoom::Focus(owner), Pane::Salon)
        );
        assert_eq!(app.detail(), Some((owner, Tab::Log)));
        assert_eq!(app.detail_card(owner), Some(c));
        assert_eq!(app.card_at_hand(), Some(c));
        assert!(app.on_card());
        app.on_key(key(KeyCode::Esc));
        assert_eq!(
            (app.card, app.zoom, app.view),
            (None, Zoom::None, View::Home)
        );
        // `:salon` is Home's team, `:waiting` its first row to decide.
        app.world.asks_you(owner, "Bash: ls");
        app.command("salon");
        assert_eq!(app.view, View::Home);
        assert!(
            matches!(
                app.at_home.at,
                Some(HomeRow::Agent(_) | HomeRow::Project(_))
            ),
            "{:?}",
            app.at_home.at
        );
        app.command("waiting");
        assert_eq!(app.view, View::Home);
        assert!(
            matches!(app.at_home.at, Some(HomeRow::Item(_))),
            "{:?}",
            app.at_home.at
        );
        // `W` from a window: the window closes, Home has the item.
        app.command("code");
        app.window = None;
        app.command("timeline");
        app.open_waiting(app.world.open_inbox()[0]);
        assert_eq!((app.window, app.view), (None, View::Home));
    }

    #[test]
    fn a_view_shown_closes_the_window_and_the_board_window_breathes() {
        let mut app = team();
        app.command("board");
        app.command("home");
        assert_eq!((app.window, app.view), (None, View::Home));
        app.command("code");
        app.command("salon");
        assert_eq!(
            (app.window, app.view),
            (None, View::Home),
            "the team is Home's"
        );
        // Not at rest with a window open: `esc` has it to close.
        app.command("timeline");
        assert!(!app.at_rest());
        app.on_key(key(KeyCode::Esc));
        assert!(app.at_rest());
    }
}
