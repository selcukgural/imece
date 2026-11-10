//! Where a key goes, and the person's key file.

use super::*;

impl App {
    /// The keys a start found (`keys::Start`), the key file at `file` as
    /// it was before it was read (`seen`); a file refused is said.
    pub fn start_keys(&mut self, file: Option<PathBuf>, seen: keys::Stamp, start: keys::Start) {
        self.keys = start.map;
        self.keys_file = file;
        self.keys_seen = seen;
        self.keys_new = false;
        self.keys_good = start.good;
        if let (Some(why), Some(path)) = (start.refused, &self.keys_file) {
            self.flash(keys::refusal(self.lang, path, &why));
        }
    }

    /// `:keys`: the key file read again. A good one is taken and said
    /// (with the keys of imece's nothing does now), none leaves imece's
    /// own keys, a bad one leaves the keys in use and says why.
    pub(super) fn read_keys(&mut self) {
        let t = self.lang.text();
        let Some(path) = self.keys_file.clone() else {
            return self.flash(t.keys_nowhere);
        };
        let shown = self.shown_path(&path);
        self.keys_seen = keys::stamp(&path);
        let good = match keys::load(&path) {
            keys::Read::Missing => {
                self.keys = Keymap::default();
                self.flash(fill(t.keys_none, &[&shown]));
                None
            }
            keys::Read::Good(map, text) => {
                self.flash(keys::taken(self.lang, &path, &map));
                self.keys = map;
                Some(text)
            }
            keys::Read::Bad(why) => return self.flash(keys::refusal(self.lang, &path, &why)),
        };
        if good != self.keys_good {
            self.keys_good = good;
            self.keys_new = true;
        }
    }

    /// The key file looked at at `now`, at most every [`keys::EVERY`]:
    /// changed since it was last read (another text, saved over, made or
    /// removed), it is read again as on `:keys`, and what came of it is
    /// said; a bad edit leaves the keys in use.
    pub fn watch_keys(&mut self, now: Instant) {
        if self
            .keys_looked
            .is_some_and(|at| now.duration_since(at) < keys::EVERY)
        {
            return;
        }
        self.keys_looked = Some(now);
        let Some(path) = &self.keys_file else {
            return;
        };
        if keys::stamp(path) != self.keys_seen {
            self.read_keys();
        }
    }

    pub fn on_key(&mut self, k: KeyEvent) {
        let ctrl_c = k.modifiers.contains(KeyModifiers::CONTROL) && k.code == KeyCode::Char('c');
        if self.too_small() && !ctrl_c {
            return;
        }
        if self.viewing.is_some() && !ctrl_c {
            return self.view_key(k);
        }
        // The mark's coming never holds a key up.
        self.mark_from.set(None);
        self.around(|app| app.key(k));
    }

    /// Whether the screen last drawn was under the floor (record 131),
    /// so it showed nothing a key could act on.
    pub fn too_small(&self) -> bool {
        self.drawn_size
            .get()
            .is_some_and(|size| !crate::ui::fits_floor(size))
    }

    /// A key while an archive's view is open: `esc` or `q` where the view
    /// has nothing open closes it; any other goes to the view.
    fn view_key(&mut self, k: KeyEvent) {
        let Some(v) = self.viewing.as_mut() else {
            return;
        };
        let leaves = matches!(k.code, KeyCode::Esc | KeyCode::Char('q'));
        if leaves && k.modifiers.is_empty() && v.at_rest() {
            self.viewing = None;
            return;
        }
        v.on_key(k);
        self.after_view();
    }

    /// What a key in an archive's view asked of this screen: text to
    /// copy, to leave the view (`q q`, `:quit`), to bring the archive back
    /// (`:restore`).
    pub(super) fn after_view(&mut self) {
        let Some(v) = self.viewing.as_mut() else {
            return;
        };
        if let Some(text) = v.copy.take() {
            self.copy = Some(text);
        }
        if v.quit {
            self.viewing = None;
        } else if v.restore_asked {
            let Some(p) = v.viewed().map(|viewed| viewed.project) else {
                return;
            };
            self.viewing = None;
            self.restore_at(p);
        }
    }

    /// Whether nothing is open on this screen that `esc` would close: no
    /// window, line, chord, conversation, card, focus or change; the
    /// salon or Home as they are.
    pub(super) fn at_rest(&self) -> bool {
        use keys::Scope;
        !self.typing
            && !self.help
            && self.pending.is_none()
            && self.keys_held.is_none()
            && self.noting.is_none()
            && self.card.is_none()
            && self.zoom == Zoom::None
            && self.scope_below() == Some(Scope::Salon)
    }

    /// `enter` on an archive (or one of its agents, `agent`) in the
    /// browser: its read-only view opens over this screen, from what the
    /// store keeps of its journal (the demo: what it has in memory).
    pub(super) fn view_archive(&mut self, p: ProjectId, agent: Option<AgentId>) {
        let t = self.lang.text();
        let Some(archived) = self.world.projects[p].archived.clone() else {
            return;
        };
        let read = match &self.engine {
            Engine::Live(l) => l.project_journal(&self.world, p),
            Engine::Sim(_) => Ok(self.world.journal_of(p)),
            Engine::View(_) => return self.read_only(),
        };
        let (journal, marks) = match read {
            Ok(read) => read,
            Err(why) => return self.flash(fill(t.view_failed, &[&why])),
        };
        let world = self.world.view_of(p, journal, marks);
        let viewed = Viewed {
            project: p,
            title: archived.title,
            at: archived.at,
        };
        let mut v = App::with(world, Engine::View(viewed), self.lang);
        v.keys = self.keys.clone();
        v.shift_enter = self.shift_enter;
        v.home = self.home.clone();
        v.fold_talk = self.fold_talk;
        if let Some(id) = agent {
            v.selected = Sel::Agent(id);
        }
        self.viewing = Some(Box::new(v));
    }

    /// The first of two keys of the person's, held since [`keys::WAIT`]
    /// before `now` with no second: it acts as itself alone.
    pub fn keys_due(&mut self, now: Instant) {
        if let Some(v) = &mut self.viewing {
            v.keys_due(now);
            self.changed |= std::mem::take(&mut v.changed);
            return self.after_view();
        }
        if self
            .keys_held
            .is_some_and(|(_, at)| now.duration_since(at) >= keys::WAIT)
        {
            self.around(|app| {
                if let Some((first, _)) = app.keys_held.take() {
                    let keys = app.keys.at(app.key_scope());
                    app.act_all(keys.alone(first), None);
                }
            });
            // What the key did shows, as after any key.
            self.changed = true;
        }
    }

    /// What a key does, `act`, and what follows any key: the place left
    /// behind, the theme previewed, what is seen and kept.
    fn around(&mut self, act: impl FnOnce(&mut Self)) {
        let was = self.place();
        act(self);
        // A key that took the person elsewhere leaves the place behind;
        // `^o` going back does not.
        // Lines of a change being chosen to copy go with the change.
        if self
            .diff_copy
            .is_some_and(|(id, _, _)| self.zoom != Zoom::Diff(id))
        {
            self.diff_copy = None;
        }
        // Lines pointed at from the log go with the change they are in.
        if self
            .pointed
            .is_some_and(|p| self.zoom != Zoom::Diff(p.worker))
        {
            self.pointed = None;
        }
        self.keep_reading();
        let back = std::mem::take(&mut self.went_back);
        if !back && self.place() != was {
            self.been.push(was);
            if self.been.len() > PLACES {
                self.been.remove(0);
            }
        }
        self.preview_theme();
        self.seen();
        self.follow_thread();
        self.persist();
    }

    pub(super) fn key(&mut self, k: KeyEvent) {
        let away = (self.away.over(self.last_key.elapsed())).then_some(self.last_key_tick);
        self.last_key = Instant::now();
        self.last_key_tick = self.world.tick;

        if k.modifiers.contains(KeyModifiers::CONTROL) && k.code == KeyCode::Char('c') {
            self.quit = true;
            return;
        }
        // The person's own keys, everywhere but where keys are text: a
        // first key held goes with this one when the file gives the two,
        // else it acts alone first.
        let mut away = away;
        if let Some((first, _)) = self.keys_held.take() {
            let keys = self.keys.at(self.key_scope());
            if let Some(both) = keys.pair(first, k) {
                self.act_all(both, away);
                return;
            }
            self.act_all(keys.alone(first), away.take());
        }
        if self.typing || (self.help && self.help_typing) {
            self.act(k, away);
            return;
        }
        // The keys of the place the screen is at, after the first acted.
        let keys = self.keys.at(self.key_scope());
        if self.pending.is_none() && keys.starts(k) {
            // imece's own chord waits for its second key first.
            self.keys_held = Some((k, self.last_key));
        } else {
            self.act_all(keys.alone(k), away);
        }
    }

    /// The place of the screen whose part of the key file holds now
    /// (`[change]`), in the order `act` takes keys; none on the windows
    /// and questions over it.
    pub fn key_scope(&self) -> Option<keys::Scope> {
        let below = self.scope_below()?;
        Some(if self.help { keys::Scope::Help } else { below })
    }

    /// [`App::key_scope`] under the help: where `?` was pressed.
    pub fn scope_below(&self) -> Option<keys::Scope> {
        use keys::Scope;
        if self.leaving
            || !self.news.is_empty()
            || self.welcome
            || self.digest.is_some()
            || self.left_off.is_some()
            || self.solo_pick.is_some()
            || self.archives.is_some()
        {
            return None;
        }
        if let Some(win) = self.window {
            return Some(win.scope());
        }
        Some(if self.peek.is_some() {
            Scope::Peek
        } else if self.copying() {
            Scope::Choose
        } else if self.thread.is_some() {
            Scope::Talk
        } else if let Zoom::Diff(_) = self.zoom {
            Scope::Change
        } else {
            Scope::Salon
        })
    }

    /// Each of `keys` as if pressed, one after the other; the second goes
    /// where the first left the screen. `away` (the tick the person left
    /// at) only for the first.
    fn act_all(&mut self, keys: Vec<KeyEvent>, mut away: Option<u64>) {
        for k in keys {
            self.act(k, away.take());
        }
    }

    /// What one key does where the screen is.
    fn act(&mut self, k: KeyEvent, away: Option<u64>) {
        // What quitting would leave: `q` quits after all, `W` goes to what
        // waits, anything else stays.
        if std::mem::take(&mut self.leaving) {
            match k.code {
                KeyCode::Char('q') => self.quit = true,
                KeyCode::Char('W') => self.next_waiting(false),
                _ => {}
            }
            return;
        }
        // An overlay takes the key that closes it.
        if !std::mem::take(&mut self.news).is_empty()
            || std::mem::take(&mut self.welcome)
            || self.digest.take().is_some()
            || self.left_off.take().is_some()
        {
            return;
        }
        if self.solo_pick.is_some() {
            self.solo_pick_key(k.code);
            return;
        }
        if self.archives.is_some() {
            self.archives_key(k);
            return;
        }
        if self.help {
            self.help_key(k);
            return;
        }
        if let Some(since) = away
            && let Some(d) = Digest::since(&self.world, since)
        {
            self.digest = Some(d);
            return;
        }
        if self.typing {
            self.type_key(k);
            return;
        }
        if let Some(id) = self.peek {
            self.peek_key(id, k);
            return;
        }
        if let Some((item, at)) = self.script {
            self.script_key(item, at, k);
            return;
        }
        if let Some((item, at)) = self.plan_window {
            self.plan_key(item, at, k);
            return;
        }
        if self.importing.is_some() {
            self.import_key(k);
            return;
        }
        if self.teardown.is_some() {
            self.teardown_key(k);
            return;
        }
        if let Some(win) = self.tidy {
            self.tidy_key(win, k);
            return;
        }
        if let Some(win) = self.ps {
            self.ps_key(win, k);
            return;
        }
        if let Some(win) = self.lonca_win {
            self.lonca_key(win, k);
            return;
        }
        if let Some(at) = self.ledger_win {
            self.ledger_key(at, k);
            return;
        }
        if let Some(win) = self.doctor {
            self.doctor_key(win, k);
            return;
        }
        if let Some(win) = self.window {
            self.window_key(win, k);
            return;
        }
        if let Some(first) = self.pending.take() {
            self.chord(first, k);
            return;
        }
        if self.copy_key(k) {
            return;
        }
        if k.modifiers.contains(KeyModifiers::CONTROL) {
            match k.code {
                KeyCode::Char('d') => self.page(true, true),
                KeyCode::Char('u') => self.page(false, true),
                KeyCode::Char('f') => self.page(true, false),
                KeyCode::Char('b') => self.page(false, false),
                KeyCode::Char('o') => self.go_back(),
                KeyCode::Char('r') => self.search_history(""),
                _ => {}
            }
            return;
        }
        // On a piece of work `tab` moves between its tabs; a digit picks
        // one of the answers its question offers and `/` writes to it as
        // on Home, below (`question_at_hand` covers the detail).
        if self.detail().is_some() && matches!(k.code, KeyCode::Tab | KeyCode::BackTab) {
            self.next_tab(k.code == KeyCode::BackTab);
            return;
        }
        if self.thread.is_some() && self.thread_key(k) {
            return;
        }
        if let Some(id) = self.zoomed_agent()
            && self.zoom_answer_key(id, k)
        {
            return;
        }
        if let Some(id) = self.focus_zoom()
            && self.focus_key(id, k)
        {
            return;
        }
        if let (Zoom::Trace(id), None) = (self.zoom, &self.thread)
            && self.trace_key(id, k)
        {
            return;
        }
        if let Zoom::Diff(id) = self.zoom
            && self.diff_key(id, k)
        {
            return;
        }
        if self.home_keys() && self.home_key(k) {
            return;
        }
        // A decision on Home's row: once it acted, the row in its place
        // comes to hand (a yes moves down).
        let decides = self.home_keys() && matches!(k.code, KeyCode::Char('a' | 'r' | 'e' | 'A'));

        match k.code {
            // `q` asks; `qq` quits (ctrl-c still quits at once).
            KeyCode::Char('q') => self.ask_quit(),
            // `i` as in vim: start writing.
            // On Home, `/` on a question writes the answer in words: the
            // composer opens to its agent, as `a` does.
            KeyCode::Char('/') if self.question_at_hand().is_some() => {
                let i = self.question_at_hand().unwrap_or_default();
                self.answer_item(i, true);
            }
            KeyCode::Char('/') | KeyCode::Char('i') => self.typing = true,
            KeyCode::Char(':') => {
                self.typing = true;
                self.input = ":".into();
                self.menu = self.suggest();
            }
            KeyCode::Char('!') => {
                self.typing = true;
                self.input = "!".into();
                self.menu = None;
            }
            KeyCode::Char('#') => self.hash_line(),
            KeyCode::Char('&') => self.open_jump(),
            KeyCode::Char('n') | KeyCode::Char('p') if self.on_card() => {
                self.step_card(k.code == KeyCode::Char('n'))
            }
            KeyCode::Char('g') | KeyCode::Char('z') => {
                if let KeyCode::Char(c) = k.code {
                    self.pending = Some(c);
                }
            }
            KeyCode::Char('G') => self.jump(false),
            KeyCode::Char('t') => {
                if let Some(id) = self.agent_at_hand() {
                    self.trace(id);
                }
            }
            KeyCode::Char('K') => self.peek = self.agent_at_hand(),
            // On an ask in waiting for you: yes from now on; else the
            // archives.
            KeyCode::Char('A') if self.pane == Pane::Inbox && self.ask_at_hand().is_some() => {
                self.answer_always();
            }
            // On an ask in waiting for you: yes to all its agent asks
            // until its turn ends.
            KeyCode::Char('e') if self.pane == Pane::Inbox => {
                if let Some(item) = self.ask_at_hand() {
                    self.answer_turn(item);
                }
            }
            KeyCode::Char('A') => self.open_archives(),
            KeyCode::Char('H') => self.search_all(""),
            KeyCode::Char('v') => {
                if let Some(id) = self.agent_at_hand() {
                    self.open_talk_with(id);
                }
            }
            KeyCode::Char('@') => {
                self.typing = true;
                self.input = match self.zoom {
                    Zoom::Focus(id) | Zoom::Trace(id) | Zoom::Diff(id) => {
                        format!("@{} ", self.world.agents[id].name)
                    }
                    Zoom::None => "@".into(),
                };
                self.after = 0;
                self.menu = self.suggest();
            }
            // A digit picks one of the answers a question offers (on Home
            // or its detail); the digits do nothing else (record 125: the
            // views they opened are `:` windows).
            KeyCode::Char(d @ '1'..='9') if self.option_at_hand(d).is_some() => {
                self.answer_option(d);
            }
            KeyCode::Char('T') => self.next_theme(),
            KeyCode::Char('L') => self.set_lang(self.lang.other()),
            // Live agents run in real time: nothing to pause or speed up.
            KeyCode::Char(' ') if self.demo() => self.pause(),
            KeyCode::Char('+') | KeyCode::Char('=') if self.demo() => self.faster(),
            KeyCode::Char('-') if self.demo() => self.slower(),
            KeyCode::Char('?') => self.open_help(),
            // From anywhere: what waits for the person, one item after
            // the other; `S` puts this one off first.
            KeyCode::Char('W') => self.next_waiting(false),
            // The journal in a window; `J` there turns the story on and off.
            KeyCode::Char('J') => self.open_window(Window::Journal),
            KeyCode::Char('S') => self.next_waiting(true),
            KeyCode::Char('m') => self.toggle_mark(),
            KeyCode::Char('u') => self.undo(),
            KeyCode::Char('U') => self.grace_now(),
            KeyCode::Char('\'') => self.next_mark(),
            KeyCode::Char('O') => self.quote_output(),
            KeyCode::Enter => self.deeper(),
            KeyCode::Esc if self.on_card() => self.leave_card(),
            KeyCode::Esc => {
                self.zoom = match self.zoom {
                    Zoom::Trace(id) => Zoom::Focus(id),
                    _ => Zoom::None,
                };
            }
            // What is selected under a conversation is not what is on
            // screen: nothing is decided unseen.
            KeyCode::Char('a' | 'r') if self.thread.is_some() => {
                self.flash(self.lang.text().talk_decides_nothing);
            }
            KeyCode::Char('a') if self.pane == Pane::Inbox => self.answer(true),
            KeyCode::Char('r') if self.pane == Pane::Inbox => self.answer(false),
            // Nothing at hand to approve while a change still goes in:
            // said, so `a` is never silent then.
            KeyCode::Char('a') if self.applying_besides(None) => {
                self.flash(self.lang.text().apply_under_way);
            }
            _ => {}
        }
        if decides {
            self.home_decided();
        }
    }

    /// The second key of a chord that began with `first`.
    fn chord(&mut self, first: char, k: KeyEvent) {
        if first == 'q' {
            // The question is answered either way.
            self.flash = None;
        }
        match (first, k.code) {
            ('q', KeyCode::Char('q')) => self.quit = true,
            ('q', KeyCode::Char('s')) => self.leaving = self.in_flight(),
            ('g', KeyCode::Char('g')) if self.home_keys() => self.home_go(0),
            ('g', KeyCode::Char('d')) if self.home_keys() => self.home_go_heading(Heading::Decide),
            ('g', KeyCode::Char('w')) if self.home_keys() => self.home_go_heading(Heading::Work),
            ('g', KeyCode::Char('c')) if self.home_keys() => self.first_change(),
            ('g', KeyCode::Char('g')) => self.jump(true),
            ('z', KeyCode::Char('a')) if self.thread.is_some() => self.toggle_fold(),
            ('z', KeyCode::Char('a')) if self.diff_zoom().is_some() => {
                if let Some(id) = self.diff_zoom() {
                    self.fold_files(id, |f, n| {
                        if let Some(n) = n {
                            f.set(n, !f.folded(n))
                        }
                    });
                }
            }
            ('z', KeyCode::Char(c @ ('M' | 'R'))) if self.diff_zoom().is_some() => {
                if let Some(id) = self.diff_zoom() {
                    self.fold_files(id, |f, _| *f = Folds::every(c == 'M'));
                }
            }
            ('z', KeyCode::Char('M')) if self.thread.is_some() => self.fold_talk(true),
            ('z', KeyCode::Char('R')) if self.thread.is_some() => self.fold_talk(false),
            ('z', KeyCode::Char('a')) if self.focus_zoom().is_some() => {
                if let Some(id) = self.focus_zoom() {
                    let at = self.focus_at(id);
                    let r = &mut self.reading;
                    r.folds.set(at, !r.folds.folded(at));
                    r.follow.set(true);
                }
            }
            ('z', KeyCode::Char(c @ ('M' | 'R'))) if self.focus_zoom().is_some() => {
                self.reading.folds = Folds::every(c == 'M');
                self.reading.follow.set(true);
            }
            ('z', KeyCode::Char('a')) if self.home_keys() => self.home_fold_toggle(),
            ('z', KeyCode::Char('M')) if self.home_keys() => self.fold_all(true),
            ('z', KeyCode::Char('R')) if self.home_keys() => self.fold_all(false),
            _ => {}
        }
    }
}

#[cfg(test)]
mod tests {
    use super::super::tests::*;
    use super::*;

    #[test]
    fn the_digits_open_no_view_and_l_does_what_its_colon_name_does() {
        let mut app = App::new(1, 1, 0, 1, Lang::En);
        for c in ['1', '2', '3', '4', '5'] {
            app.on_key(key(KeyCode::Char(c)));
            assert_eq!((app.window), (None), "{c}");
        }
        app.on_key(key(KeyCode::Char('L')));
        assert_eq!(app.lang, Lang::Tr);
        assert_eq!(app.flash.take().unwrap().0, "Türkçe");
        app.on_key(key(KeyCode::Char('L')));
        assert_eq!(app.lang, Lang::En);
    }

    #[test]
    fn big_k_peeks_from_home_and_a_trace_s_z_m_z_r_fold_no_project() {
        let mut app = with_cards(1);
        app.on_key(key(KeyCode::Char('K')));
        assert!(app.peek.is_some());
        assert_eq!(app.peek, app.agent_at_hand());
        app.on_key(key(KeyCode::Esc));
        assert!(app.peek.is_none());
        // `zM` `zR` are Home's (and a focus's own): a raw stream over
        // Home folds nothing.
        let id = app.world.projects[0].leader + 1;
        app.zoom = Zoom::Trace(id);
        assert!(app.collapsed.iter().all(|&c| !c));
        app.on_key(key(KeyCode::Char('z')));
        app.on_key(key(KeyCode::Char('M')));
        assert!(
            app.collapsed.iter().all(|&c| !c),
            "a trace's zM folds no project"
        );
        app.collapsed.iter_mut().for_each(|c| *c = true);
        app.on_key(key(KeyCode::Char('z')));
        app.on_key(key(KeyCode::Char('R')));
        assert!(
            app.collapsed.iter().all(|&c| c),
            "nor does its zR unfold one"
        );
        app.zoom = Zoom::None;
        app.on_key(key(KeyCode::Char('z')));
        app.on_key(key(KeyCode::Char('R')));
        assert!(app.collapsed.iter().all(|&c| !c), "on Home it does");
    }

    /// `line` written in the view and sent, its menu put away first.
    fn view_typed(app: &mut App, line: &str) {
        // A plain message: writing starts with `i`.
        if !line.starts_with([':', '!', '@']) {
            app.on_key(key(KeyCode::Char('i')));
        }
        for c in line.chars() {
            app.on_key(key(KeyCode::Char(c)));
        }
        if let Some(v) = app.viewing.as_mut() {
            v.menu = None;
        }
        app.on_key(key(KeyCode::Enter));
    }

    #[test]
    fn an_archive_s_view_opens_no_editor() {
        let mut app = viewing();
        app.on_key(key(KeyCode::Char('i')));
        app.on_key(key(KeyCode::Char('a')));
        app.on_key(ctrl('x'));
        app.on_key(ctrl('e'));
        assert!(view(&app).editor.is_none() && app.editor.is_none());
        assert_eq!(said(view(&app)), Lang::En.text().read_only);
    }

    #[test]
    fn keys_go_to_the_view_and_esc_q_and_ctrl_c_leave_it() {
        let mut app = viewing();
        // ctrl-r searches the view's lines sent, not this screen's.
        app.on_key(ctrl('r'));
        assert!(view(&app).searching && !app.searching);
        // `esc` ends that first, the view stays; then it closes the view.
        app.viewing.as_mut().unwrap().menu = None;
        app.on_key(key(KeyCode::Esc));
        assert!(!view(&app).typing);
        app.on_key(KeyEvent::new(KeyCode::Esc, KeyModifiers::SHIFT));
        app.on_key(key(KeyCode::Char('x')));
        assert!(app.viewing.is_some(), "only a plain esc or q leaves");
        app.on_key(key(KeyCode::Esc));
        assert!(app.viewing.is_none());
        assert!(!app.quit);
        // `q` leaves it too.
        app.view_archive(1, None);
        app.on_key(key(KeyCode::Char('q')));
        assert!(app.viewing.is_none() && !app.quit);
        // `:quit` and its `q` leave the view, not imece.
        app.view_archive(1, None);
        view_typed(&mut app, ":quit");
        assert!(app.viewing.is_some());
        app.on_key(key(KeyCode::Char('q')));
        assert!(app.viewing.is_none() && !app.quit);
        // ctrl-c quits imece from the view.
        app.view_archive(1, None);
        app.on_key(ctrl('c'));
        assert!(app.quit && app.viewing.is_some());
    }

    #[test]
    fn what_the_view_copies_pastes_and_flashes_goes_through_the_screen_behind() {
        let mut app = viewing();
        app.viewing.as_mut().unwrap().copy = Some("words".into());
        app.on_key(key(KeyCode::Char('x')));
        assert_eq!(app.copy.as_deref(), Some("words"));
        assert!(view(&app).copy.is_none());
        // A paste goes into what the view writes.
        app.on_key(key(KeyCode::Char(':')));
        app.paste("board");
        assert_eq!(view(&app).input, ":board");
        assert!(app.input.is_empty());
        // A key the key file holds back is let through in the view, and
        // what it did is drawn.
        let now = Instant::now();
        app.on_key(key(KeyCode::Esc));
        app.changed = false;
        app.viewing.as_mut().unwrap().keys_held = Some((key(KeyCode::Char(':')), now - keys::WAIT));
        app.keys_due(now);
        assert!(view(&app).keys_held.is_none() && view(&app).typing);
        assert!(app.changed && !view(&app).changed);
        app.on_key(key(KeyCode::Esc));
        // What was to be drawn still is.
        app.viewing.as_mut().unwrap().keys_held = Some((key(KeyCode::Char(':')), now - keys::WAIT));
        app.keys_due(now);
        assert!(app.changed && view(&app).typing);
        app.on_key(key(KeyCode::Esc));
        // The view's flash is the screen's clock.
        app.drawn(now);
        assert!(!app.draw_due(now));
        app.viewing.as_mut().unwrap().flash = Some(("x".into(), now));
        assert!(app.draw_due(now));
    }

    #[test]
    fn the_view_refuses_what_would_act_and_runs_what_only_looks() {
        let mut app = viewing();
        let read_only = Lang::En.text().read_only;
        let look = |a: &App| {
            let w = &view(a).world;
            let projects: Vec<_> = (w.projects.iter())
                .map(|q| {
                    (
                        q.name.clone(),
                        q.closed,
                        q.archived.clone(),
                        q.copies.clone(),
                    )
                })
                .collect();
            let agents: Vec<_> = (w.agents.iter()).map(|a| (a.status, a.gone)).collect();
            (w.journal.len(), w.marks.clone(), w.budget, projects, agents)
        };
        let was = look(&app);
        let lead = view(&app).world.projects[1].leader;
        let to_lead = format!("@{} hi", view(&app).world.agents[lead].name);
        for line in [
            ":budget 5",
            "hello there",
            &to_lead,
            "!ls",
            ":archives",
            ":mark",
            ":close",
        ] {
            app.viewing.as_mut().unwrap().flash = None;
            view_typed(&mut app, line);
            assert_eq!(said(view(&app)), read_only, "{line}");
            assert!(app.shell.is_none() && view(&app).shell.is_none());
        }
        for k in ['m', 'A', 'a', 'r'] {
            let v = app.viewing.as_mut().unwrap();
            v.flash = None;
            v.pane = if k == 'm' { Pane::Journal } else { Pane::Inbox };
            v.journal_sel = Some(0);
            app.on_key(key(KeyCode::Char(k)));
            assert_eq!(said(view(&app)), read_only, "{k}");
            assert!(view(&app).archives.is_none());
        }
        // Every way to the engine says the same, and changes nothing.
        let calls: [fn(&mut App, AgentId); 10] = [
            |v, _| v.copy_path("target"),
            |v, _| v.suggest_commit(),
            |v, id| {
                assert!(!v.tell(id, "hi"));
            },
            |v, _| v.open_project("/tmp/elsewhere"),
            |v, _| v.close_now(1),
            |v, _| v.archive_now(1),
            |v, _| v.restore_at(1),
            |v, id| v.forget_now(id),
            |v, _| v.forget_agent(""),
            |v, id| {
                v.decide_change(id, true);
            },
        ];
        for (i, call) in calls.iter().enumerate() {
            let v = app.viewing.as_mut().unwrap();
            v.flash = None;
            call(v, lead);
            assert_eq!(said(view(&app)), read_only, "{i}");
        }
        assert_eq!(look(&app), was);
        assert_eq!(view(&app).model_of(lead), None);
        // An unknown command is one, not a refusal.
        view_typed(&mut app, ":nosuch");
        assert_eq!(said(view(&app)), "no command nosuch; : and tab list them");
        // What only looks or goes runs; nothing pauses.
        view_typed(&mut app, ":board");
        assert_eq!(view(&app).window, Some(Window::Board));
        app.on_key(key(KeyCode::Esc));
        assert_eq!(view(&app).window, None);
        app.on_key(key(KeyCode::Char(' ')));
        assert!(!view(&app).paused);
        view_typed(&mut app, ":lang tr");
        assert_eq!((view(&app).lang, app.lang), (Lang::Tr, Lang::En));
        // `:restore` brings the archive back from behind it.
        view_typed(&mut app, ":restore");
        assert!(app.viewing.is_none());
        assert!(app.world.projects[1].archived.is_none());
        assert!(said(&app).starts_with("brought "), "{}", said(&app));
    }

    fn keymap(text: &str) -> Keymap {
        Keymap::read(text).unwrap()
    }

    #[test]
    fn a_turned_key_acts_with_its_ctrl() {
        let mut app = App::new(1, 4, 0, 1, Lang::En);
        app.keys = keymap("x = ^o");
        app.on_key(key(KeyCode::Enter));
        assert!(matches!(app.zoom, Zoom::Focus(_)), "Home's row opened");
        app.on_key(key(KeyCode::Char('x')));
        assert_eq!(app.zoom, Zoom::None, "x is ^o: back where the person was");
    }

    /// A demo app whose salon shows three rows.
    fn three_rows() -> App {
        let app = quiet();
        *app.shown.borrow_mut() = vec![Sel::Project(0), Sel::Agent(0), Sel::Agent(1)];
        app
    }

    #[test]
    fn two_keys_of_the_person_act_as_one() {
        let mut app = three_rows();
        app.keys = keymap("fx = L\ngt = f");
        app.on_key(key(KeyCode::Char('f')));
        assert!(app.keys_held.is_some());
        app.on_key(key(KeyCode::Char('x')));
        assert_eq!(app.lang, Lang::Tr, "fx is L");
        assert!(app.keys_held.is_none());
        app.lang = Lang::En;
        // Another key: the first acts alone, then the other.
        app.on_key(key(KeyCode::Char('f')));
        app.on_key(key(KeyCode::Char('?')));
        assert!(app.help, "and ? acted after it");
        assert!(app.keys_held.is_none());
        app.on_key(key(KeyCode::Esc));
        let top = app.selected;
        // g waits too, but imece's gg is not kept waiting.
        app.on_key(key(KeyCode::Char('G')));
        let bottom = app.selected;
        assert_ne!(bottom, top);
        app.on_key(key(KeyCode::Char('g')));
        assert_eq!(app.pending, None, "held, not imece's yet");
        app.on_key(key(KeyCode::Char('g')));
        assert_eq!(app.selected, top, "gg at once");
        assert!(
            app.keys_held.is_none(),
            "the second g went to imece's chord"
        );
        app.on_key(key(KeyCode::Char('g')));
        app.on_key(key(KeyCode::Char('t')));
        assert_eq!(app.pending, None);
    }

    #[test]
    fn a_first_key_alone_acts_after_its_wait() {
        let mut app = App::new(1, 4, 0, 1, Lang::En);
        app.keys = keymap("fx = L");
        app.keys_due(Instant::now());
        app.on_key(key(KeyCode::Char('f')));
        let (_, at) = app.keys_held.unwrap();
        app.keys_due(at + keys::WAIT - Duration::from_millis(1));
        app.keys_due(at + keys::WAIT);
        assert!(app.keys_held.is_none());
        app.keys_due(at + keys::WAIT * 3);
    }

    #[test]
    fn one_key_of_the_person_acts_as_two() {
        let mut app = three_rows();
        app.keys = keymap("x = gg\ny = :q\nG = j");
        app.on_key(key(KeyCode::Char('G')));
        assert_eq!(app.at_home.row, 1, "G is j");
        app.on_key(key(KeyCode::Char('x')));
        assert_eq!(app.at_home.row, 0, "x is gg");
        assert_eq!(app.pending, None);
        // The second goes where the first left the screen: writing.
        app.on_key(key(KeyCode::Char('y')));
        assert!(app.typing);
        assert_eq!(app.input, ":q");
    }

    #[test]
    fn the_key_file_s_place_is_where_the_screen_is() {
        use keys::Scope;
        let mut app = App::new(1, 1, 0, 1, Lang::En);
        assert_eq!(app.key_scope(), Some(Scope::Salon));
        assert_eq!(
            app.key_scope(),
            Some(Scope::Salon),
            "Home has the team's keys"
        );
        app.window = Some(Window::Board);
        assert_eq!(app.key_scope(), Some(Scope::Board));
        app.window = Some(Window::Code);
        assert_eq!(app.key_scope(), Some(Scope::Code));
        app.window = Some(Window::Timeline);
        assert_eq!(app.key_scope(), Some(Scope::Timeline));
        app.window = None;
        app.help = true;
        assert_eq!(app.key_scope(), Some(Scope::Help));
        assert_eq!(app.scope_below(), Some(Scope::Salon), "where ? was pressed");
        app.help = false;
        app.peek = Some(0);
        assert_eq!(app.key_scope(), Some(Scope::Peek));
        app.peek = None;
        // A window or a question over the screen has none.
        for over in [
            |a: &mut App| a.leaving = true,
            |a: &mut App| a.welcome = true,
            |a: &mut App| a.news = vec!["new"],
            |a: &mut App| a.digest = Some(Digest::default()),
            |a: &mut App| a.left_off = Some(LeftOff::default()),
            |a: &mut App| a.solo_pick = Some(SoloPick::new(None, None)),
            |a: &mut App| a.archives = Some(Browser::new(&a.world, Vec::new())),
        ] {
            let mut app = App::new(1, 1, 0, 1, Lang::En);
            over(&mut app);
            assert_eq!(app.key_scope(), None);
            app.help = true;
            assert_eq!(app.key_scope(), None, "the window takes the key first");
        }
        let (mut app, worker) = with_change("+one\n+two\n");
        assert_eq!(app.key_scope(), Some(Scope::Change));
        app.on_key(key(KeyCode::Char('v')));
        assert!(app.diff_copy_of(worker).is_some());
        assert_eq!(app.key_scope(), Some(Scope::Choose));
        let (mut app, _) = with_thread();
        assert_eq!(app.key_scope(), Some(Scope::Talk));
        app.zoom = Zoom::Diff(0);
        assert_eq!(
            app.key_scope(),
            Some(Scope::Talk),
            "the conversation over it"
        );
        app.thread.as_mut().unwrap().copy = Some(Copying::new(0));
        assert_eq!(app.key_scope(), Some(Scope::Choose));
    }

    #[test]
    fn a_place_s_keys_hold_only_there() {
        let mut app = three_rows();
        app.keys = keymap("d = L\n[board]\nd = q\ngt = esc\n[timeline]\ne = j");
        app.on_key(key(KeyCode::Char('d')));
        assert_eq!(app.lang, Lang::Tr, "d is L where no place says else");
        app.command("timeline");
        assert!(ui::timeline::agents(&app).len() > 1);
        app.on_key(key(KeyCode::Char('e')));
        assert_eq!(app.timeline_sel, 1, "e is j on the timeline");
        app.on_key(key(KeyCode::Char('d')));
        assert_eq!(
            app.lang,
            Lang::Tr,
            "d is L on the timeline, which takes no L"
        );
        assert_eq!(app.window, Some(Window::Timeline));
        app.on_key(key(KeyCode::Esc));
        app.command("board");
        app.on_key(key(KeyCode::Char('d')));
        assert_eq!(app.window, None, "d is q on the board");
        app.on_key(key(KeyCode::Char('e')));
        assert_eq!(app.timeline_sel, 1, "e is nothing in the salon");
        app.on_key(key(KeyCode::Char('g')));
        assert!(app.keys_held.is_none(), "gt is the board's");
        app.on_key(key(KeyCode::Char('L')));
        assert_eq!(app.lang, Lang::Tr, "gL is no chord");
        app.command("board");
        app.on_key(key(KeyCode::Char('g')));
        assert!(app.keys_held.is_some(), "the board's g waits");
        app.keys_due(Instant::now() + keys::WAIT * 2);
        assert_eq!(app.pending, None, "goes alone, and the window takes it");
        assert_eq!(app.window, Some(Window::Board));
        app.on_key(key(KeyCode::Char('g')));
        app.on_key(key(KeyCode::Char('t')));
        assert_eq!(app.window, None, "gt is esc on the board");
        assert_eq!(app.command_note("keys"), "(4)", "every line counts");
    }

    #[test]
    fn a_held_key_that_starts_writing_leaves_the_next_one_as_text() {
        let mut app = App::new(1, 4, 0, 1, Lang::En);
        app.keys = keymap("/x = 3\n2y = 1");
        app.on_key(key(KeyCode::Char('/')));
        assert!(!app.typing, "/ waits");
        app.on_key(key(KeyCode::Char('2')));
        assert!(app.typing, "/ alone writes");
        assert_eq!(app.input, "2", "and 2 is a letter, not held");
        assert!(app.keys_held.is_none());
        app.on_key(key(KeyCode::Char('x')));
        assert_eq!(app.input, "2x", "writing is never turned");
    }

    #[test]
    fn colon_keys_reads_the_file_again_and_a_bad_one_leaves_the_keys() {
        let dir = dir("read");
        let path = dir.join("keys");
        let mut app = App::new(1, 1, 0, 1, Lang::En);
        app.command("keys");
        assert_eq!(
            flashed(&app).as_deref(),
            Some("no home folder, so no key file")
        );
        app.keys_file = Some(path.clone());
        let shown = path.display().to_string();
        app.command("keys");
        assert_eq!(
            flashed(&app),
            Some(format!(
                "no key file; KEY = KEY lines in {shown} give you keys of your own"
            ))
        );
        assert!(!app.keys_new, "nothing was kept, nothing to keep");
        assert_eq!(app.command_note("keys"), "(0)");
        std::fs::write(&path, "^n = j\nj = G\n").unwrap();
        app.command("keys");
        assert_eq!(flashed(&app), Some(format!("{shown}: keys taken (2)")));
        assert_eq!(app.keys, keymap("^n = j\nj = G"));
        assert_eq!(app.keys_good.as_deref(), Some("^n = j\nj = G\n"));
        assert!(std::mem::take(&mut app.keys_new), "to keep");
        assert_eq!(app.command_note("keys"), "(2)");
        app.command("keys");
        assert!(!app.keys_new, "the same file again: nothing new to keep");
        std::fs::write(&path, "^n = j\nnope\n").unwrap();
        app.command("keys");
        assert_eq!(
            flashed(&app),
            Some(format!(
                "{shown}: line 2: write it as KEY = KEY; the keys you had stay"
            ))
        );
        assert_eq!(app.keys, keymap("^n = j\nj = G"), "they stay");
        assert_eq!(app.keys_good.as_deref(), Some("^n = j\nj = G\n"));
        assert!(!app.keys_new);
        std::fs::remove_file(&path).unwrap();
        app.command("keys");
        assert_eq!(app.keys, Keymap::default(), "no file: imece's own keys");
        assert_eq!(app.keys_good, None);
        assert!(app.keys_new, "forgotten for the next start too");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn a_start_says_why_the_key_file_was_refused() {
        let mut app = App::new(1, 1, 0, 1, Lang::En);
        app.keys_new = true;
        let path = PathBuf::from("/c/imece/keys");
        let start = keys::Start {
            map: keymap("^p = k"),
            good: Some("^p = k".into()),
            refused: Some(keys::Why::Quits(2)),
        };
        app.start_keys(Some(path.clone()), keys::Stamp::default(), start);
        assert_eq!(app.keys, keymap("^p = k"));
        assert_eq!(app.keys_good.as_deref(), Some("^p = k"));
        assert_eq!(app.keys_file, Some(path.clone()));
        assert!(!app.keys_new, "main keeps what a start found");
        assert_eq!(
            flashed(&app).as_deref(),
            Some("/c/imece/keys: line 2: ctrl-c always quits; the keys you had stay")
        );
        let mut app = App::new(1, 1, 0, 1, Lang::En);
        app.start_keys(Some(path), keys::Stamp::default(), keys::Start::default());
        assert_eq!(flashed(&app), None, "a good start says nothing");
        let mut app = App::new(1, 1, 0, 1, Lang::En);
        let start = keys::Start {
            refused: Some(keys::Why::TooBig),
            ..keys::Start::default()
        };
        app.start_keys(None, keys::Stamp::default(), start);
        assert_eq!(flashed(&app), None, "no file to name");
    }

    #[test]
    fn a_changed_key_file_is_read_again_every_few_seconds() {
        let dir = dir("watch");
        let path = dir.join("keys");
        let shown = path.display().to_string();
        std::fs::write(&path, "^n = j\n").unwrap();
        let mut app = App::new(1, 1, 0, 1, Lang::En);
        let seen = keys::stamp(&path);
        let start = keys::Start::new(Some(keys::load(&path)), None);
        app.start_keys(Some(path.clone()), seen, start);
        let at = Instant::now();
        let after = |n: u32| at + keys::EVERY * n;
        app.watch_keys(at);
        assert_eq!(flashed(&app), None, "as the start read it: not read");
        assert!(!app.keys_new);
        std::fs::write(&path, "^n = j\nj = G\n").unwrap();
        app.watch_keys(after(1) - Duration::from_millis(1));
        assert_eq!(app.keys, keymap("^n = j"), "not looked at yet");
        app.watch_keys(after(1));
        assert_eq!(app.keys, keymap("^n = j\nj = G"), "looked at, read again");
        assert_eq!(flashed(&app), Some(format!("{shown}: keys taken (2)")));
        assert_eq!(app.keys_good.as_deref(), Some("^n = j\nj = G\n"));
        assert!(std::mem::take(&mut app.keys_new), "to keep");
        app.flash = None;
        app.watch_keys(after(2));
        assert_eq!(flashed(&app), None, "read once for each change");
        std::fs::write(&path, "^n = j\nnope\n").unwrap();
        app.watch_keys(after(3));
        assert_eq!(
            flashed(&app),
            Some(format!(
                "{shown}: line 2: write it as KEY = KEY; the keys you had stay"
            ))
        );
        assert_eq!(app.keys, keymap("^n = j\nj = G"), "a bad edit: they stay");
        assert!(!app.keys_new);
        app.flash = None;
        app.watch_keys(after(4));
        assert_eq!(flashed(&app), None, "said once, not every few seconds");
        std::fs::remove_file(&path).unwrap();
        app.watch_keys(after(5));
        assert_eq!(app.keys, Keymap::default(), "removed: imece's own keys");
        assert!(std::mem::take(&mut app.keys_new));
        std::fs::write(&path, "^p = k\n").unwrap();
        app.watch_keys(after(6));
        assert_eq!(app.keys, keymap("^p = k"), "made again");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn back_in_the_window_the_key_file_is_looked_at_at_once() {
        let dir = dir("focus");
        let path = dir.join("keys");
        let mut app = App::new(1, 1, 0, 1, Lang::En);
        app.start_keys(
            Some(path.clone()),
            keys::Stamp::default(),
            keys::Start::default(),
        );
        let at = Instant::now();
        app.watch_keys(at);
        assert_eq!(flashed(&app), None, "no file, as at the start");
        app.focus(false, at);
        std::fs::write(&path, "^n = j\n").unwrap();
        let back = at + Duration::from_millis(5);
        app.focus(true, back);
        app.watch_keys(back);
        assert_eq!(app.keys, keymap("^n = j"));
        std::fs::write(&path, "^n = j\nj = G\n").unwrap();
        app.command("keys");
        assert_eq!(app.keys, keymap("^n = j\nj = G"));
        app.flash = None;
        app.focus(true, back);
        app.watch_keys(back);
        assert_eq!(flashed(&app), None, "`:keys` read it: nothing new");
        let mut app = App::new(1, 1, 0, 1, Lang::En);
        app.watch_keys(at);
        assert_eq!(flashed(&app), None, "no home, no file to look at");
        let _ = std::fs::remove_dir_all(&dir);
    }
}
