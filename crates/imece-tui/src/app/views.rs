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
        self.window = Some(win);
    }

    /// A key while a window is open: `esc` or `q` close it, `?` opens the
    /// help; on the timeline `j k` (the arrows), `g G` (Home, End) move
    /// over the lanes, `[ ]` change the scale, `enter` opens the selected
    /// agent's focus and `@` writes to it, both closing the window; the
    /// rest do nothing, so the screen under the window does not move.
    pub(super) fn window_key(&mut self, win: Window, k: KeyEvent) {
        match k.code {
            KeyCode::Esc | KeyCode::Char('q') => self.window = None,
            KeyCode::Char('?') => self.open_help(),
            _ if win == Window::Timeline => self.timeline_key(k),
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

    #[test]
    fn a_view_shown_closes_the_window_and_the_board_window_breathes() {
        let mut app = team();
        app.command("board");
        app.command("home");
        assert_eq!((app.window, app.view), (None, View::Home));
        app.command("code");
        app.command("salon");
        assert_eq!((app.window, app.view), (None, View::Salon));
        // Not at rest with a window open: `esc` has it to close.
        app.command("timeline");
        assert!(!app.at_rest());
        app.on_key(key(KeyCode::Esc));
        assert!(app.at_rest());
    }
}
