//! The windows over the screen: peek, scripts, plans, tidy, welcome, news, hints, help.

use super::*;

impl App {
    /// A key while a peek is open: `j k` (the arrows) the next / previous
    /// agent of the salon, `enter` its whole focus, `@` a message to it,
    /// `esc` or `K` close it, `?` the help; the rest do nothing, so the
    /// view under it does not move.
    pub(super) fn peek_key(&mut self, id: AgentId, k: KeyEvent) {
        match k.code {
            KeyCode::Down | KeyCode::Char('j') => self.peek = Some(self.peek_step(id, true)),
            KeyCode::Up | KeyCode::Char('k') => self.peek = Some(self.peek_step(id, false)),
            KeyCode::Enter => {
                self.peek = None;
                self.zoom = Zoom::Focus(id);
            }
            KeyCode::Char('@') => {
                self.peek = None;
                self.typing = true;
                self.input = format!("@{} ", self.world.agents[id].name);
                self.menu = self.suggest();
            }
            KeyCode::Esc | KeyCode::Char('K') => self.peek = None,
            KeyCode::Char('?') => self.open_help(),
            _ => {}
        }
    }

    /// A key while a setup script's window is open: `j k` (the arrows),
    /// `ctrl-d ctrl-u` (page keys) and `g G` move through it, `a` says yes
    /// and `r` no to it, `esc` or `q` close it, `W` and `S` close it and
    /// go to the next item to decide (`S` putting this one off), `?` the
    /// help; the rest do nothing.
    pub(super) fn script_key(&mut self, item: usize, at: usize, k: KeyEvent) {
        let ctrl = k.modifiers.contains(KeyModifiers::CONTROL);
        let page = SCRIPT_PAGE;
        let to = match k.code {
            KeyCode::Down | KeyCode::Char('j') => at + 1,
            KeyCode::Up | KeyCode::Char('k') => at.saturating_sub(1),
            KeyCode::PageDown => at + page,
            KeyCode::Char('d') if ctrl => at + page,
            KeyCode::PageUp => at.saturating_sub(page),
            KeyCode::Char('u') if ctrl => at.saturating_sub(page),
            KeyCode::Char('g') => 0,
            KeyCode::Char('G') => usize::MAX,
            KeyCode::Char('a') => return self.answer_script(item, true),
            KeyCode::Char('r') => return self.answer_script(item, false),
            KeyCode::Esc | KeyCode::Char('q') => {
                self.script = None;
                return;
            }
            KeyCode::Char(c @ ('W' | 'S')) => {
                self.script = None;
                return self.next_waiting(c == 'S');
            }
            KeyCode::Char('?') => return self.open_help(),
            _ => return,
        };
        // Never past the last line's being the top one.
        let last = self
            .script_text(item)
            .map_or(0, |s| s.text().lines().count());
        self.script = Some((item, to.min(last.saturating_sub(1))));
    }

    /// The script setup ask `item` is about, as the engine read it.
    pub fn script_text(&self, item: usize) -> Option<crate::live::setup::Script> {
        match &self.engine {
            Engine::Live(l) => l.setup_asked(item).cloned(),
            Engine::Sim(s) => s.setup_asked(&self.world, item),
            Engine::View(_) => None,
        }
    }

    /// The person's answer to setup script ask `item`: the engine keeps a
    /// yes and runs the bytes asked about, or holds a no for the run; the
    /// window closes and the flash says which. An ask no longer there
    /// (its run ended) is said so.
    pub(super) fn answer_script(&mut self, item: usize, yes: bool) {
        let t = self.lang.text();
        let took = match &mut self.engine {
            Engine::Live(l) => l.decide_setup(&mut self.world, item, yes),
            Engine::Sim(s) => s.decide_setup(&mut self.world, item, yes),
            Engine::View(_) => return self.read_only(),
        };
        self.script = None;
        self.flash(match (took, yes) {
            (false, _) => t.setup_gone,
            (true, true) => t.setup_yes,
            (true, false) => t.setup_no,
        });
        let open = self.inbox_rows().len();
        self.inbox_sel = self.inbox_sel.min(open.saturating_sub(1));
    }

    /// The keys of the plan window (record 114): `j k` and the pages
    /// scroll its cards, `a` says yes, `r` no, `n` opens the composer to
    /// the leader (the item closes when the message goes), `esc` closes
    /// the window.
    pub(super) fn plan_key(&mut self, item: usize, at: usize, k: KeyEvent) {
        let ctrl = k.modifiers.contains(KeyModifiers::CONTROL);
        let page = SCRIPT_PAGE;
        let to = match k.code {
            KeyCode::Down | KeyCode::Char('j') => at + 1,
            KeyCode::Up | KeyCode::Char('k') => at.saturating_sub(1),
            KeyCode::PageDown => at + page,
            KeyCode::Char('d') if ctrl => at + page,
            KeyCode::PageUp => at.saturating_sub(page),
            KeyCode::Char('u') if ctrl => at.saturating_sub(page),
            KeyCode::Char('g') => 0,
            KeyCode::Char('G') => usize::MAX,
            KeyCode::Char('a') => return self.answer_plan(item, true),
            KeyCode::Char('r') => return self.answer_plan(item, false),
            KeyCode::Char('n') => return self.plan_notes(item),
            KeyCode::Esc | KeyCode::Char('q') => {
                self.plan_window = None;
                return;
            }
            KeyCode::Char('?') => return self.open_help(),
            _ => return,
        };
        // Never past the last line's being the top one.
        let last = self.plan_lines(item);
        self.plan_window = Some((item, to.min(last.saturating_sub(1))));
    }

    /// How many lines the plan of item `item` lists (its cards), for the
    /// window's scroll.
    fn plan_lines(&self, item: usize) -> usize {
        let leader = self.world.inbox[item].agent;
        let p = self.world.agents[leader].project;
        (self.world.projects[p].plan.proposal.as_ref())
            .map_or(0, |pr| ui::overlay::plan_lines(self, pr).len())
    }

    /// The person's answer to plan item `item` (record 114): the engine
    /// opens the gate on a yes, drops the plan on a no, and the leader
    /// hears either; the window closes and the flash says which. A plan
    /// no longer waiting is said so.
    pub(super) fn answer_plan(&mut self, item: usize, yes: bool) {
        let t = self.lang.text();
        let took = match &mut self.engine {
            Engine::Live(l) => l.decide_plan(&mut self.world, item, yes),
            Engine::Sim(s) => s.decide_plan(&mut self.world, item, yes),
            Engine::View(_) => return self.read_only(),
        };
        self.plan_window = None;
        self.flash(match (took, yes) {
            (false, _) => t.plan_gone,
            (true, true) => t.plan_yes,
            (true, false) => t.plan_no,
        });
        let open = self.inbox_rows().len();
        self.inbox_sel = self.inbox_sel.min(open.saturating_sub(1));
    }

    /// `n` on a plan: the composer opens to the leader; what the person
    /// writes closes the plan unanswered and starts the leader's turn,
    /// which plans again from it.
    fn plan_notes(&mut self, item: usize) {
        if self.viewing_archive() {
            return self.read_only();
        }
        let name = self.world.agents[self.world.inbox[item].agent].name.clone();
        self.plan_window = None;
        self.typing = true;
        self.input = format!("@{name} ");
        self.menu = None;
    }

    /// `:plan [on|off]` (record 114): the project at hand's plan gate;
    /// bare, it says where it stands.
    pub(super) fn plan_command(&mut self, arg: &str) {
        let t = self.lang.text();
        let p = self.current_project();
        let name = self.world.projects[p].name.clone();
        match arg.trim() {
            "" => {}
            "on" | "off" => {
                if self.viewing_archive() {
                    return self.read_only();
                }
                self.world.projects[p].plan.on = arg.trim() == "on";
            }
            other => return self.flash(fill(t.plan_bad, &[other])),
        }
        let text = if self.world.projects[p].plan.on {
            t.plan_on
        } else {
            t.plan_off
        };
        self.flash(fill(text, &[&name]));
    }

    /// At a start: the engine looks for worktrees no agent owns, and a
    /// flash says how many when it found any ([`App::watch_tidy`]).
    pub fn start_tidy(&mut self) {
        self.tidy_start = true;
        self.look_orphans();
    }

    fn look_orphans(&mut self) {
        match &mut self.engine {
            Engine::Live(l) => l.look_orphans(&mut self.world),
            Engine::Sim(s) => s.look_orphans(&mut self.world),
            Engine::View(_) => {}
        }
    }

    /// `:tidy`: the worktrees no agent owns in a window, looked for again.
    pub(crate) fn open_tidy(&mut self) {
        self.tidy = Some(TidyWin::default());
        self.look_orphans();
    }

    /// What a look or a removal answered: a removal's end is said once;
    /// the start's look says how many it found, unless the window shows
    /// them. The selection stays on the list.
    pub(super) fn watch_tidy(&mut self) {
        let t = self.lang.text();
        if let Some(tidied) = self.world.orphans.tidied.take() {
            self.flash(ui::words::tidied(self.lang, &tidied));
        }
        let Some(found) = self.world.orphans.found.as_ref() else {
            return;
        };
        let n = found.len();
        if let Some(win) = &mut self.tidy {
            win.at = win.at.min(n.saturating_sub(1));
        }
        if self.tidy_start && !self.world.orphans.busy {
            self.tidy_start = false;
            if n > 0 && self.tidy.is_none() {
                self.flash(fill(t.tidy_found, &[&n.to_string()]));
            }
        }
    }

    /// A key while the `:tidy` window is open: `j k` (the arrows), `g G`
    /// move, `a` removes the clean ones, `d` the one at hand (one that
    /// holds work after a `y` for it: any other key keeps it), `esc` or
    /// `q` close, `?` the help; the rest do nothing.
    pub(super) fn tidy_key(&mut self, win: TidyWin, k: KeyEvent) {
        let found = self.world.orphans.found.clone().unwrap_or_default();
        if win.sure {
            self.tidy = Some(TidyWin { sure: false, ..win });
            if k.code == KeyCode::Char('y') {
                self.tidy_remove(vec![win.at], true);
            }
            return;
        }
        let last = found.len().saturating_sub(1);
        let at = match k.code {
            KeyCode::Down | KeyCode::Char('j') => (win.at + 1).min(last),
            KeyCode::Up | KeyCode::Char('k') => win.at.saturating_sub(1),
            KeyCode::Char('g') | KeyCode::Home => 0,
            KeyCode::Char('G') | KeyCode::End => last,
            KeyCode::Char('a') => {
                let clean: Vec<usize> = (0..found.len()).filter(|&i| found[i].clean()).collect();
                if clean.is_empty() {
                    return self.flash(self.lang.text().tidy_no_clean);
                }
                return self.tidy_remove(clean, false);
            }
            KeyCode::Char('d') => {
                let Some(o) = found.get(win.at) else {
                    return;
                };
                if o.clean() {
                    return self.tidy_remove(vec![win.at], false);
                }
                self.tidy = Some(TidyWin { sure: true, ..win });
                return;
            }
            KeyCode::Esc | KeyCode::Char('q') => {
                self.tidy = None;
                return;
            }
            KeyCode::Char('?') => return self.open_help(),
            _ => return,
        };
        self.tidy = Some(TidyWin { at, ..win });
    }

    /// The worktrees at `at` of the list go, those that hold work only
    /// when `force`; a refusal is said.
    fn tidy_remove(&mut self, at: Vec<usize>, force: bool) {
        let t = self.lang.text();
        let done = match &mut self.engine {
            Engine::Live(l) => l.tidy(&mut self.world, &at, force),
            Engine::Sim(s) => s.tidy(&mut self.world, &at, force),
            Engine::View(_) => return self.read_only(),
        };
        match done {
            Ok(()) => {}
            Err(TidyRefusal::Busy) => self.flash(t.tidy_busy),
            Err(TidyRefusal::Nothing) => self.flash(t.tidy_none),
            Err(TidyRefusal::Asks(script)) => {
                self.teardown = Some(TeardownAsk {
                    script: *script,
                    scroll: 0,
                    at,
                    force,
                })
            }
        }
    }

    /// A key while a teardown script is asked about: `j k` (the arrows),
    /// `ctrl-d ctrl-u` (page keys) and `g G` move through it, `a` says
    /// yes and `r` no to it and the removal asked goes on, `esc` or `q`
    /// close it and nothing is removed, `?` the help; the rest do nothing.
    pub(super) fn teardown_key(&mut self, k: KeyEvent) {
        let Some(ask) = self.teardown.as_mut() else {
            return;
        };
        let ctrl = k.modifiers.contains(KeyModifiers::CONTROL);
        let (at, page) = (ask.scroll, SCRIPT_PAGE);
        let to = match k.code {
            KeyCode::Down | KeyCode::Char('j') => at + 1,
            KeyCode::Up | KeyCode::Char('k') => at.saturating_sub(1),
            KeyCode::PageDown => at + page,
            KeyCode::Char('d') if ctrl => at + page,
            KeyCode::PageUp => at.saturating_sub(page),
            KeyCode::Char('u') if ctrl => at.saturating_sub(page),
            KeyCode::Char('g') => 0,
            KeyCode::Char('G') => usize::MAX,
            KeyCode::Char('a') => return self.answer_teardown(true),
            KeyCode::Char('r') => return self.answer_teardown(false),
            KeyCode::Esc | KeyCode::Char('q') => {
                self.teardown = None;
                return self.flash(self.lang.text().teardown_held);
            }
            KeyCode::Char('?') => return self.open_help(),
            _ => return,
        };
        // Never past the last line's being the top one.
        let last = ask.script.text().lines().count();
        ask.scroll = to.min(last.saturating_sub(1));
    }

    /// The person's answer to the teardown script asked about: the
    /// engine keeps it, and the removal it held goes on (asking about the
    /// next checkout's, if another has one).
    fn answer_teardown(&mut self, yes: bool) {
        let Some(ask) = self.teardown.take() else {
            return;
        };
        match &mut self.engine {
            Engine::Live(l) => l.decide_teardown(&mut self.world, &ask.script, yes),
            Engine::Sim(s) => s.decide_teardown(&mut self.world, &ask.script, yes),
            Engine::View(_) => return self.read_only(),
        }
        self.tidy_remove(ask.at, ask.force);
    }

    /// Whether a window any key closes is open: what is new, the
    /// first-run window, the away digest, where the person left off.
    pub(super) fn window_open(&self) -> bool {
        !self.news.is_empty() || self.welcome || self.digest.is_some() || self.left_off.is_some()
    }

    /// `:welcome`: the first-run window, Claude Code asked again (live).
    pub fn open_welcome(&mut self) {
        self.welcome = true;
        self.look_again();
    }

    /// Asks Claude Code about itself again, live: its line says `checking…`
    /// until the answer comes ([`App::looked`]).
    fn look_again(&mut self) {
        let Some(look) = self.look else {
            return;
        };
        self.seen = Seen::Checking;
        self.looking = Some(look());
    }

    /// A live start: the first ever opens the first-run window; any other
    /// asks only whether Claude Code is signed in, and opens it when not
    /// ([`App::looked`]).
    pub fn start_welcome(&mut self, first: bool) {
        if first {
            return self.open_welcome();
        }
        self.lead_check = true;
        self.look_again();
    }

    /// The answer Claude Code gave since last time, on its line. Not
    /// signed in at the start opens the window; while the person writes, a
    /// line says so instead.
    pub fn looked(&mut self) {
        let Some(rx) = &self.looking else {
            return;
        };
        let seen = match rx.try_recv() {
            Ok(seen) => seen,
            Err(TryRecvError::Empty) => return,
            Err(TryRecvError::Disconnected) => {
                self.looking = None;
                return;
            }
        };
        self.looking = None;
        // Its line on the welcome window, if open.
        self.changed = true;
        let out = matches!(
            seen,
            Seen::Here {
                login: Login::No,
                ..
            }
        );
        self.seen = seen;
        if !std::mem::take(&mut self.lead_check) || !out {
            return;
        }
        if self.typing {
            let t = self.lang.text();
            self.flash(fill(t.not_signed_in, &[CLAUDE]));
        } else {
            self.welcome = true;
        }
    }

    /// `:news`: what is new in the latest versions.
    pub(super) fn open_news(&mut self) {
        let news = self.lang.text().news;
        self.news = news.iter().take(welcome::NEWS_SHOWN).map(|n| n.0).collect();
    }

    /// The agent after (`forward`) or before agent `id` in the salon's
    /// order; the first when `id` has no row there; `id` itself at
    /// either end.
    fn peek_step(&self, id: AgentId, forward: bool) -> AgentId {
        // Home's agents, in its order.
        let agents: Vec<AgentId> = (self.home_rows().iter())
            .filter_map(|r| match r.1 {
                HomeRow::Agent(a) => Some(a),
                _ => None,
            })
            .collect();
        let next = match agents.iter().position(|&a| a == id) {
            Some(i) if forward => agents.get(i + 1),
            Some(i) => i.checked_sub(1).and_then(|i| agents.get(i)),
            None => agents.first(),
        };
        next.copied().unwrap_or(id)
    }

    /// `?` or `:help`: the help, all of it.
    pub(super) fn open_help(&mut self) {
        self.help = true;
        self.help_find.clear();
        self.help_scroll = 0;
        self.help_all = false;
    }

    /// Whether the help lists this screen's keys alone: not after `?`
    /// again, and not while a filter is typed or kept (a search is over
    /// every key).
    pub fn help_scoped(&self) -> bool {
        !self.help_all && !self.help_typing && self.help_find.is_empty()
    }

    /// A key while the help is open: `/` starts (or goes back to) a
    /// filter, which takes the keys typed until `enter` keeps it or `esc`
    /// drops it (backspace past its start leaves it too); the moving keys
    /// scroll it where it does not fit the screen; any other key closes
    /// the help.
    pub(super) fn help_key(&mut self, k: KeyEvent) {
        if !self.help_typing {
            let (room, last) = self.help_room.get();
            let half = (room / 2).max(1);
            let ctrl = k.modifiers.contains(KeyModifiers::CONTROL);
            let at = self.help_scroll.min(last);
            let g = self.pending.take() == Some('g');
            self.help_scroll = match k.code {
                KeyCode::Char('/') => {
                    self.help_typing = true;
                    return;
                }
                KeyCode::Char('?') => {
                    self.help_all = !self.help_all;
                    0
                }
                KeyCode::Char('d') if ctrl => at + half,
                KeyCode::Char('u') if ctrl => at.saturating_sub(half),
                KeyCode::Char('f') if ctrl => at + room,
                KeyCode::Char('b') if ctrl => at.saturating_sub(room),
                _ if ctrl => return self.help = false,
                KeyCode::Down | KeyCode::Char('j') => at + 1,
                KeyCode::Up | KeyCode::Char('k') => at.saturating_sub(1),
                KeyCode::PageDown => at + room,
                KeyCode::PageUp => at.saturating_sub(room),
                KeyCode::Char('g') if g => 0,
                KeyCode::Char('g') => {
                    self.pending = Some('g');
                    return;
                }
                KeyCode::Home => 0,
                KeyCode::Char('G') | KeyCode::End => last,
                _ => return self.help = false,
            }
            .min(last);
            return;
        }
        // The filter changes what is shown: from its top again.
        self.help_scroll = 0;
        let plain = !k.modifiers.contains(KeyModifiers::CONTROL)
            && !k.modifiers.contains(KeyModifiers::ALT);
        match k.code {
            KeyCode::Enter => self.help_typing = false,
            KeyCode::Esc => {
                self.help_find.clear();
                self.help_typing = false;
            }
            KeyCode::Backspace => {
                if self.help_find.pop().is_none() {
                    self.help_typing = false;
                }
            }
            KeyCode::Char('u') if k.modifiers.contains(KeyModifiers::CONTROL) => {
                self.help_find.clear()
            }
            KeyCode::Char(c) if plain => self.help_find.push(c),
            _ => {}
        }
    }
}

#[cfg(test)]
mod tests {
    use super::super::tests::*;
    use super::*;

    /// ... not signed in.
    fn signed_out() -> welcome::Answers {
        let (tx, rx) = std::sync::mpsc::channel();
        tx.send(here("0.9", Login::No)).unwrap();
        rx
    }

    #[test]
    fn the_first_start_ever_opens_the_welcome_and_asks_claude_code() {
        let mut app = live_welcome(fake_look);
        assert!(!app.welcome);
        assert_eq!(app.seen, Seen::Checking);
        app.start_welcome(true);
        assert!(app.welcome);
        assert!(!app.lead_check, "the window is open already");
        app.looked();
        assert_eq!(app.seen, here("2.1.3", Login::Yes));
        assert!(app.looking.is_none(), "the answer came");
        // Any key closes it and does nothing else.
        app.on_key(key(KeyCode::Char('?')));
        assert!(!app.welcome && !app.help);
        // `:welcome` opens it again and asks again.
        app.seen = Seen::Missing;
        app.input = ":welcome".into();
        app.typing = true;
        app.on_key(key(KeyCode::Enter));
        assert!(app.welcome);
        assert_eq!(app.seen, Seen::Checking);
        app.looked();
        assert_eq!(app.seen, here("2.1.3", Login::Yes));
    }

    #[test]
    fn a_later_start_opens_the_welcome_only_when_it_is_not_signed_in() {
        // Signed in: nothing opens.
        let mut app = live_welcome(fake_look);
        app.start_welcome(false);
        assert!(app.lead_check);
        assert!(!app.welcome);
        app.looked();
        assert!(!app.welcome);
        assert!(!app.lead_check);
        assert_eq!(app.seen, here("2.1.3", Login::Yes));
        // Not signed in: the window opens.
        let mut app = live_welcome(signed_out);
        app.start_welcome(false);
        assert!(app.lead_check);
        assert_eq!(app.seen, Seen::Checking);
        app.looked();
        assert!(app.welcome);
        assert!(!app.lead_check);
        assert_eq!(app.seen, here("0.9", Login::No), "its answer stays");
        // While the person writes, a line says so instead.
        let mut app = live_welcome(signed_out);
        app.start_welcome(false);
        app.typing = true;
        app.looked();
        assert!(!app.welcome);
        let said = &app.flash.as_ref().expect("said").0;
        assert_eq!(said, "claude is not signed in: :welcome says how");
        // `:welcome` asked by hand opens nothing more, and says nothing.
        let mut app = live_welcome(signed_out);
        app.look_again();
        app.looked();
        assert!(!app.welcome && app.flash.is_none());
        // Nothing asked, nothing changes.
        let mut app = live_welcome(signed_out);
        app.looked();
        assert!(!app.welcome && app.flash.is_none());
    }

    #[test]
    fn the_demo_s_welcome_says_what_it_says_of_itself_and_news_shows_the_latest() {
        let mut app = App::new(1, 1, 0, 1, Lang::En);
        assert_eq!(app.seen, crate::sim::seen());
        let Seen::Here { version, login } = &app.seen else {
            panic!("{:?}", app.seen);
        };
        assert!(version.ends_with("· demo"), "{version}");
        assert_eq!(*login, Login::Yes);
        assert!(!app.welcome && app.news.is_empty() && app.look.is_none());
        app.input = ":welcome".into();
        app.typing = true;
        app.on_key(key(KeyCode::Enter));
        assert!(app.welcome);
        assert_eq!(app.seen, crate::sim::seen(), "nothing asked");
        app.on_key(key(KeyCode::Char('x')));
        assert!(!app.welcome);
        app.input = ":news".into();
        app.typing = true;
        app.on_key(key(KeyCode::Enter));
        let latest: Vec<&str> = Lang::En.text().news.iter().take(3).map(|n| n.0).collect();
        assert_eq!(app.news, latest);
        assert_eq!(app.news[0], welcome::VERSION);
        assert_eq!(app.command_note("news"), format!("({})", welcome::VERSION));
        // The news first, then the welcome: one key each.
        app.welcome = true;
        app.on_key(key(KeyCode::Char('x')));
        assert!(app.news.is_empty() && app.welcome);
        app.on_key(key(KeyCode::Char('x')));
        assert!(!app.welcome);
    }

    #[test]
    fn enter_on_a_setup_ask_shows_its_bytes_and_a_or_r_answer_there() {
        let (mut app, item, id) = asking_setup();
        app.on_key(key(KeyCode::Enter));
        assert_eq!(app.script, Some((item, 0)));
        let text = app.script_text(item).unwrap().text();
        let lines = text.lines().count();
        assert_eq!(lines, 6);
        // Moving: never past the last line's being the top one.
        for (k, at) in [
            (key(KeyCode::Char('j')), 1),
            (key(KeyCode::Down), 2),
            (key(KeyCode::Char('k')), 1),
            (key(KeyCode::Up), 0),
            (key(KeyCode::Up), 0),
            (key(KeyCode::Char('G')), lines - 1),
            (key(KeyCode::Char('j')), lines - 1),
            (key(KeyCode::Char('g')), 0),
            (key(KeyCode::PageDown), lines - 1),
            (key(KeyCode::PageUp), 0),
            (ctrl('d'), lines - 1),
            (ctrl('u'), 0),
            (key(KeyCode::Char('d')), 0),
            (key(KeyCode::Char('x')), 0),
            (key(KeyCode::Char('j')), 1),
            (key(KeyCode::Char('u')), 1),
        ] {
            app.on_key(k);
            assert_eq!(app.script, Some((item, at)), "{k:?}");
        }
        assert_eq!(SCRIPT_PAGE, 10);
        // `?` opens the help over it; esc closes the window.
        app.on_key(key(KeyCode::Char('?')));
        assert!(app.help);
        app.help = false;
        app.on_key(key(KeyCode::Esc));
        assert_eq!(app.script, None);
        app.on_key(key(KeyCode::Enter));
        app.on_key(key(KeyCode::Char('q')));
        assert_eq!(app.script, None);
        // `a` in the window: yes, it ran, the window closed.
        app.on_key(key(KeyCode::Enter));
        app.on_key(key(KeyCode::Char('a')));
        assert_eq!(app.script, None);
        assert!(!app.world.inbox[item].open);
        assert_eq!(flashed(&app).as_deref(), Some(app.lang.text().setup_yes));
        assert!(
            app.world
                .journal
                .iter()
                .any(|e| matches!(e.event, Event::SetupRan { agent, .. } if agent == id))
        );
        // Answered already: said so.
        app.script = Some((item, 0));
        app.on_key(key(KeyCode::Char('r')));
        assert_eq!(flashed(&app).as_deref(), Some(app.lang.text().setup_gone));
        assert_eq!(app.script, None);
    }

    #[test]
    fn r_on_a_setup_ask_in_waiting_for_you_says_no() {
        let (mut app, item, _) = asking_setup();
        app.on_key(key(KeyCode::Char('r')));
        assert!(!app.world.inbox[item].open);
        assert_eq!(flashed(&app).as_deref(), Some(app.lang.text().setup_no));
        assert!(app.world.setup_approved.is_empty());
        let (mut app, item, _) = asking_setup();
        app.on_key(key(KeyCode::Char('a')));
        assert!(!app.world.inbox[item].open);
        assert_eq!(app.world.setup_approved.len(), 1);
        assert_eq!(app.inbox_sel, 0);
    }

    #[test]
    fn a_peek_off_the_salons_rows_steps_to_its_first_agent() {
        let (mut app, [a, ..]) = two_quiet_projects();
        app.collapsed[0] = true;
        app.peek = Some(a);
        app.on_key(key(KeyCode::Char('j')));
        // Project 0 is folded: Home's first agent is project 1's first.
        let first = (app.home_rows().iter())
            .find_map(|r| match r.1 {
                HomeRow::Agent(id) => Some(id),
                _ => None,
            })
            .unwrap();
        assert_eq!(app.world.agents[first].project, 1);
        assert_eq!(app.peek, Some(first));
        // No agent shown at all: it stays where it is.
        app.collapsed[1] = true;
        app.peek = Some(a);
        app.on_key(key(KeyCode::Char('k')));
        assert_eq!(app.peek, Some(a));
    }

    #[test]
    fn colon_peek_names_the_agent_or_says_there_is_none() {
        let mut app = quiet();
        let name = app.world.agents[3].name.clone();
        app.command(&format!("peek @{name}"));
        assert_eq!(app.peek, Some(3));
        app.peek = None;
        app.command("peek nobody");
        assert_eq!(app.peek, None);
        assert_eq!(
            flashed(&app).unwrap_or_default(),
            "no agent named \"nobody\""
        );
    }

    /// The demo with its look for worktrees no agent owns answered, its
    /// teardown script said yes to.
    fn tidied_demo() -> App {
        let mut app = App::new(1, 1, 0, 1, Lang::En);
        app.command("tidy");
        app.step();
        let checkout = app.world.orphans.found.as_ref().unwrap()[0]
            .checkout
            .clone();
        app.world
            .approve_setup(sim::teardown_script(&checkout).key());
        app
    }

    fn orphan_names(app: &App) -> Vec<String> {
        let found = app.world.orphans.found.as_deref().unwrap_or_default();
        found.iter().map(|o| o.name.clone()).collect()
    }

    #[test]
    fn tidy_opens_the_worktrees_no_agent_owns_and_its_keys_move_on_them() {
        let mut app = App::new(1, 1, 0, 1, Lang::En);
        app.command("tidy");
        assert_eq!(app.tidy, Some(TidyWin::default()));
        assert!(app.world.orphans.busy, "no look asked");
        assert_eq!(app.world.orphans.found, None);
        app.step();
        assert!(!app.world.orphans.busy);
        assert_eq!(orphan_names(&app), ["claude-31", "claude-32"]);
        let at = |app: &App| app.tidy.unwrap().at;
        app.key(key(KeyCode::Char('j')));
        assert_eq!(at(&app), 1);
        app.key(key(KeyCode::Down));
        assert_eq!(at(&app), 1, "past the last");
        app.key(key(KeyCode::Char('k')));
        assert_eq!(at(&app), 0);
        app.key(key(KeyCode::Up));
        assert_eq!(at(&app), 0);
        app.key(key(KeyCode::Char('G')));
        assert_eq!(at(&app), 1);
        app.key(key(KeyCode::Char('g')));
        assert_eq!(at(&app), 0);
        app.key(key(KeyCode::End));
        assert_eq!(at(&app), 1);
        app.key(key(KeyCode::Home));
        assert_eq!(at(&app), 0);
        // A key it does not take does nothing.
        app.key(key(KeyCode::Char('b')));
        assert_eq!(app.tidy, Some(TidyWin::default()));
        app.key(key(KeyCode::Char('?')));
        assert!(app.help);
        app.help = false;
        app.key(key(KeyCode::Esc));
        assert_eq!(app.tidy, None);
        app.command("tidy");
        app.key(key(KeyCode::Char('q')));
        assert_eq!(app.tidy, None);
    }

    #[test]
    fn a_removes_the_clean_worktrees_and_d_one_with_work_only_after_a_y() {
        let mut app = tidied_demo();
        let t = app.lang.text();
        app.key(key(KeyCode::Char('a')));
        assert!(app.world.orphans.busy);
        // While it goes, another is refused.
        app.key(key(KeyCode::Char('a')));
        assert_eq!(flashed(&app).as_deref(), Some(t.tidy_busy));
        app.step();
        assert_eq!(flashed(&app), Some(removed("claude-31")));
        assert_eq!(app.world.orphans.tidied, None, "said once");
        assert_eq!(orphan_names(&app), ["claude-32"]);
        app.key(key(KeyCode::Char('a')));
        assert_eq!(flashed(&app).as_deref(), Some(t.tidy_no_clean));
        assert!(!app.world.orphans.busy);
        // `d` on one with work asks first; any key but `y` keeps it.
        app.key(key(KeyCode::Char('d')));
        assert_eq!(app.tidy, Some(TidyWin { at: 0, sure: true }));
        app.key(key(KeyCode::Char('n')));
        assert_eq!(app.tidy, Some(TidyWin { at: 0, sure: false }));
        assert!(!app.world.orphans.busy, "removed without a y");
        app.key(key(KeyCode::Char('d')));
        app.key(key(KeyCode::Char('y')));
        assert!(app.world.orphans.busy);
        app.step();
        assert_eq!(flashed(&app), Some(removed("claude-32")));
        assert_eq!(orphan_names(&app), Vec::<String>::new());
        // Nothing left: `d` does nothing, `y` alone removes nothing.
        app.flash = None;
        app.key(key(KeyCode::Char('d')));
        app.key(key(KeyCode::Char('y')));
        assert_eq!(
            (app.tidy, app.world.orphans.busy),
            (Some(TidyWin::default()), false)
        );
        assert_eq!(flashed(&app), None);
        // An index not in the list is refused.
        app.tidy_remove(vec![4], false);
        assert_eq!(flashed(&app).as_deref(), Some(t.tidy_none));
    }

    #[test]
    fn a_removal_asks_about_the_teardown_script_first_and_goes_on_with_the_answer() {
        let mut app = App::new(1, 1, 0, 1, Lang::En);
        let t = app.lang.text();
        app.command("tidy");
        app.step();
        // Asked: the script's bytes in a window, nothing removed yet.
        app.key(key(KeyCode::Char('a')));
        let ask = app
            .teardown
            .clone()
            .expect("the teardown script is asked about");
        assert_eq!((ask.at.clone(), ask.force, ask.scroll), (vec![0], false, 0));
        assert_eq!(ask.script.kind, crate::live::setup::Kind::Teardown);
        assert!(ask.script.text().contains("db:drop"));
        assert!(!app.world.orphans.busy);
        // It scrolls, never past its last line being the top one.
        let lines = ask.script.text().lines().count();
        let scroll = |app: &App| app.teardown.as_ref().unwrap().scroll;
        app.key(key(KeyCode::Char('j')));
        assert_eq!(scroll(&app), 1);
        app.key(key(KeyCode::Char('k')));
        app.key(key(KeyCode::Up));
        assert_eq!(scroll(&app), 0);
        app.key(key(KeyCode::Down));
        assert_eq!(scroll(&app), 1);
        app.key(key(KeyCode::Char('G')));
        assert_eq!(scroll(&app), lines - 1);
        app.key(key(KeyCode::Char('g')));
        assert_eq!(scroll(&app), 0);
        app.key(key(KeyCode::PageDown));
        assert_eq!(scroll(&app), lines - 1);
        app.key(key(KeyCode::PageUp));
        assert_eq!(scroll(&app), 0);
        app.key(ctrl('d'));
        assert_eq!(scroll(&app), lines - 1);
        app.key(ctrl('u'));
        assert_eq!(scroll(&app), 0);
        // Without ctrl, d and u do nothing here.
        app.key(key(KeyCode::Char('d')));
        assert_eq!(scroll(&app), 0);
        app.key(key(KeyCode::Char('j')));
        app.key(key(KeyCode::Char('u')));
        assert_eq!(scroll(&app), 1);
        app.key(key(KeyCode::Char('k')));
        app.key(key(KeyCode::Char('x')));
        assert_eq!(app.teardown, Some(ask.clone()));
        app.key(key(KeyCode::Char('?')));
        assert!(app.help);
        app.help = false;
        // esc: nothing removed, said so; the list window stays.
        app.key(key(KeyCode::Esc));
        assert_eq!(app.teardown, None);
        assert_eq!(flashed(&app).as_deref(), Some(t.teardown_held));
        assert!(app.tidy.is_some());
        assert!(!app.world.orphans.busy);
        app.key(key(KeyCode::Char('a')));
        app.key(key(KeyCode::Char('q')));
        assert_eq!(app.teardown, None);
        // `r`: no for the run, removed without it, not asked again.
        app.key(key(KeyCode::Char('a')));
        app.key(key(KeyCode::Char('r')));
        assert_eq!(app.teardown, None);
        assert!(app.world.orphans.busy);
        app.step();
        assert_eq!(
            flashed(&app).as_deref(),
            Some("removed claude-31 · teardown not run in claude-31: you said no")
        );
        assert!(app.world.setup_approved.is_empty());
        app.key(key(KeyCode::Char('d')));
        app.key(key(KeyCode::Char('y')));
        assert_eq!(app.teardown, None, "asked again after a no");
        app.step();
        // `a` in another run: yes is kept, and the removal it held goes on.
        let mut app = App::new(1, 1, 0, 1, Lang::En);
        app.command("tidy");
        app.step();
        app.key(key(KeyCode::Char('d')));
        app.key(key(KeyCode::Char('y')));
        assert_eq!(app.teardown.as_ref().map(|a| a.at.clone()), Some(vec![0]));
        app.key(key(KeyCode::Char('a')));
        assert_eq!(app.world.setup_approved.len(), 1);
        assert!(app.world.setup_approved[0].starts_with("teardown /bin/sh "));
        assert!(app.world.orphans.busy);
        app.step();
        assert_eq!(flashed(&app), Some(removed("claude-31")));
    }

    #[test]
    fn d_removes_a_clean_worktree_at_once_and_the_selection_stays_on_the_list() {
        let mut app = tidied_demo();
        app.key(key(KeyCode::Char('G')));
        app.key(key(KeyCode::Char('k')));
        app.key(key(KeyCode::Char('d')));
        assert_eq!(app.tidy, Some(TidyWin { at: 0, sure: false }));
        assert!(app.world.orphans.busy);
        app.step();
        assert_eq!(orphan_names(&app), ["claude-32"]);
        // Rows gone, the one at hand moves up onto the list.
        app.command("tidy");
        assert_eq!(app.tidy, Some(TidyWin::default()));
        app.tidy = Some(TidyWin { at: 3, sure: false });
        app.step();
        assert_eq!(app.tidy.unwrap().at, 0);
    }

    #[test]
    fn a_start_says_how_many_worktrees_no_agent_owns_once_unless_shown() {
        let mut app = App::new(1, 1, 0, 1, Lang::En);
        app.start_tidy();
        app.step();
        assert_eq!(
            flashed(&app).as_deref(),
            Some("2 worktrees no agent owns: :tidy lists them")
        );
        // Said once: a later look says nothing at the start's place.
        app.flash = None;
        app.command("tidy");
        app.tidy = None;
        app.step();
        assert_eq!(flashed(&app), None);
        // The window open: it shows them, no flash.
        let mut app = App::new(1, 1, 0, 1, Lang::En);
        app.start_tidy();
        app.tidy = Some(TidyWin::default());
        app.step();
        assert_eq!(flashed(&app), None);
        assert!(!app.tidy_start);
        // None found: nothing said.
        let mut app = App::new(1, 1, 0, 1, Lang::En);
        app.start_tidy();
        if let Engine::Sim(s) = &mut app.engine {
            s.forget_orphans();
        }
        app.step();
        assert_eq!(flashed(&app), None);
        assert!(!app.tidy_start);
    }

    #[test]
    fn enter_on_a_check_ask_shows_its_bytes_and_a_says_yes_there() {
        let mut app = waiting(&[]);
        let Engine::Sim(s) = &mut app.engine else {
            unreachable!()
        };
        let id = s.hire_for_test(&mut app.world, 0);
        app.world.change_ready(Change {
            agent: id,
            title: "a change".into(),
            files: Vec::new(),
            patch: String::new(),
            runs: Vec::new(),
            check: Check::None,
            spend: None,
        });
        s.check(&mut app.world, id);
        let item = (app.world.inbox.iter())
            .position(|i| i.kind == InboxKind::Check)
            .expect("asked");
        app.at_home.at = Some(HomeRow::Item(item));
        app.on_key(key(KeyCode::Enter));
        assert_eq!(app.script, Some((item, 0)));
        let script = app.script_text(item).unwrap();
        assert_eq!(script.kind, crate::live::setup::Kind::Check);
        app.on_key(key(KeyCode::Char('a')));
        assert_eq!(app.script, None);
        assert!(!app.world.inbox[item].open);
        assert_eq!(flashed(&app).as_deref(), Some(app.lang.text().setup_yes));
        assert_eq!(app.world.change_of(id).unwrap().check, Check::Running);
        // `r` on the selected ask answers it too.
        let Engine::Sim(s) = &mut app.engine else {
            unreachable!()
        };
        app.world.setup_approved.clear();
        app.world.change_ready(Change {
            agent: id,
            title: "another".into(),
            files: Vec::new(),
            patch: String::new(),
            runs: Vec::new(),
            check: Check::None,
            spend: None,
        });
        s.check(&mut app.world, id);
        let item = (app.world.inbox.iter())
            .position(|i| i.kind == InboxKind::Check && i.open)
            .expect("asked again");
        app.at_home.at = Some(HomeRow::Item(item));
        app.on_key(key(KeyCode::Char('r')));
        assert!(!app.world.inbox[item].open);
        assert_eq!(app.world.change_of(id).unwrap().check, Check::None);
    }

    #[test]
    fn enter_on_a_plan_shows_its_cards_and_a_r_n_answer_it_there() {
        let (mut app, item, leader) = asking_plan();
        app.on_key(key(KeyCode::Enter));
        assert_eq!(app.plan_window, Some((item, 0)));
        // Two cards: a head line, a paths line and two body lines, a
        // blank; a head line, one body line, a blank.
        assert_eq!(app.plan_lines(item), 8);
        // Eight lines, so a page (10) from anywhere lands on the last,
        // and a page from the top is a page, not nothing (0 + 10, not
        // 0 × 10). A plain `d` or `u` (no ctrl) is not a page.
        const { assert!(SCRIPT_PAGE > 7) };
        for (k, at) in [
            (key(KeyCode::Char('j')), 1),
            (key(KeyCode::Down), 2),
            (key(KeyCode::Char('k')), 1),
            (key(KeyCode::PageDown), 7),
            (key(KeyCode::Char('j')), 7),
            (key(KeyCode::Char('g')), 0),
            (key(KeyCode::Char('G')), 7),
            (key(KeyCode::Up), 6),
            (key(KeyCode::Char('u')), 6),
            (ctrl('u'), 0),
            (key(KeyCode::Char('d')), 0),
            (ctrl('d'), 7),
            (key(KeyCode::PageUp), 0),
            (key(KeyCode::PageDown), 7),
            (key(KeyCode::Char('x')), 7),
        ] {
            app.on_key(k);
            assert_eq!(app.plan_window, Some((item, at)), "{k:?}");
        }
        // ?: the help, over the window, which stays.
        app.on_key(key(KeyCode::Char('?')));
        assert!(app.help);
        assert_eq!(app.plan_window, Some((item, 7)));
        app.help = false;
        app.on_key(key(KeyCode::Esc));
        assert_eq!(app.plan_window, None);
        assert!(app.world.inbox[item].open, "esc answers nothing");
        app.on_key(key(KeyCode::Enter));
        app.on_key(key(KeyCode::Char('q')));
        assert_eq!(app.plan_window, None);
        // n: the composer opens to the leader.
        app.on_key(key(KeyCode::Enter));
        app.on_key(key(KeyCode::Char('n')));
        assert_eq!(app.plan_window, None);
        assert!(app.typing);
        assert_eq!(app.input, format!("@{} ", app.world.agents[leader].name));
        app.typing = false;
        app.input.clear();
        // a: yes, the window closes, the gate opens.
        app.on_key(key(KeyCode::Enter));
        app.on_key(key(KeyCode::Char('a')));
        assert_eq!(app.plan_window, None);
        assert!(!app.world.inbox[item].open);
        assert!(app.world.projects[0].plan.approved);
        assert_eq!(
            flashed(&app).as_deref(),
            Some("yes to the plan: the leader hires and assigns as planned")
        );
        // Answered already: said so.
        app.plan_window = Some((item, 0));
        app.on_key(key(KeyCode::Char('r')));
        assert_eq!(
            flashed(&app).as_deref(),
            Some("that plan is no longer waiting")
        );
        assert!(app.world.projects[0].plan.approved, "nothing changed");
        // r on the row itself, without the window: no.
        let (mut app, item, _) = asking_plan();
        app.on_key(key(KeyCode::Char('r')));
        assert!(!app.world.inbox[item].open);
        assert_eq!(app.world.projects[0].plan.proposal, None);
        assert_eq!(
            flashed(&app).as_deref(),
            Some("no to the plan: the leader hears it and asks you, or plans again")
        );
    }

    #[test]
    fn colon_plan_turns_the_gate_on_and_off_for_the_project_at_hand() {
        let mut app = waiting(&[]);
        assert!(app.world.projects[0].plan.on);
        app.command("plan");
        assert_eq!(
            flashed(&app).as_deref(),
            Some("plan gate on for shop-api: a plan with your yes before the second worker")
        );
        app.command("plan off");
        assert!(!app.world.projects[0].plan.on);
        assert_eq!(
            flashed(&app).as_deref(),
            Some("plan gate off for shop-api: the leader hires as it sees fit")
        );
        app.command("plan");
        assert!(flashed(&app).unwrap().starts_with("plan gate off"));
        app.command("plan maybe");
        assert_eq!(flashed(&app).as_deref(), Some("maybe is not on or off"));
        assert!(!app.world.projects[0].plan.on);
        app.command("plan on");
        assert!(app.world.projects[0].plan.on);
        assert!(flashed(&app).unwrap().starts_with("plan gate on"));
    }
}

#[cfg(test)]
mod script_next_tests {
    use super::super::tests::*;
    use super::*;

    /// The demo stepped until the repository's check asks for its yes,
    /// on Home, with its window open.
    fn check_window() -> (App, usize) {
        let mut app = App::new(1, 3, 0, 1, Lang::En);
        for _ in 0..400 {
            app.step();
            if let Some(i) =
                (app.world.inbox.iter()).position(|i| i.kind == InboxKind::Check && i.open)
            {
                // As `W` opens it: its window over Home.
                app.open_waiting(i);
                assert_eq!(app.script, Some((i, 0)));
                return (app, i);
            }
        }
        panic!("the demo never asked about its check");
    }

    #[test]
    fn w_in_a_script_window_closes_it_and_opens_the_next_item_to_decide() {
        let (mut app, i) = check_window();
        app.world.inbox.push(InboxItem {
            kind: InboxKind::Question,
            agent: app.world.inbox[i].agent,
            detail: "which?".into(),
            researcher: None,
            open: true,
            tick: 0,
            card: None,
        });
        let question = app.world.inbox.len() - 1;
        app.on_key(key(KeyCode::Char('W')));
        assert_eq!(app.script, None);
        assert_eq!(app.waiting_at, Some(question));
        assert!(app.deferred.is_empty());
        assert!(app.world.inbox[i].open, "nothing decided");
        // `S` puts the script's ask off first.
        app.open_waiting(i);
        app.on_key(key(KeyCode::Char('S')));
        assert_eq!(app.script, None);
        assert_eq!(app.deferred, [i]);
        assert_eq!(app.waiting_at, Some(question));
        // The only item put off comes round to itself.
        app.world.inbox[question].open = false;
        for n in 0..i {
            app.world.inbox[n].open = false;
        }
        app.open_waiting(i);
        app.on_key(key(KeyCode::Char('S')));
        assert_eq!(app.waiting_at, Some(i));
        assert_eq!(app.script, Some((i, 0)));
        // Any other letter still does nothing.
        app.script = Some((i, 0));
        app.on_key(key(KeyCode::Char('x')));
        assert_eq!(app.script, Some((i, 0)));
    }
}
