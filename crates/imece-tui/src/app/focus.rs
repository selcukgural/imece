//! An agent's focus and raw stream: reading them, and deciding there what it waits for.

use super::*;

impl App {
    /// The agent whose focus is open with nothing over it.
    pub(super) fn focus_zoom(&self) -> Option<AgentId> {
        match self.zoom {
            Zoom::Focus(id) if self.thread.is_none() => Some(id),
            _ => None,
        }
    }

    /// Where the person reads agent `id`, afresh when it was another's.
    pub(super) fn reading_for(&mut self, id: AgentId) {
        if self.reading.agent != Some(id) {
            self.reading = Reading {
                agent: Some(id),
                ..Reading::default()
            };
        }
    }

    /// What is read follows the agent of the focus or the raw stream:
    /// another agent's starts afresh, leaving both forgets it. A change
    /// opened from a line of the log keeps the line, for the way back.
    pub(super) fn keep_reading(&mut self) {
        match self.zoom {
            Zoom::Focus(id) | Zoom::Trace(id) => self.reading_for(id),
            Zoom::Diff(id) if self.pointed.is_some_and(|p| p.worker == id) => {}
            _ => self.reading = Reading::default(),
        }
    }

    /// How many entries agent `id`'s focus tells: its card's story.
    fn story_len(&self, id: AgentId) -> usize {
        (self.world.agents[id].card).map_or(0, |c| self.world.cards[c].story.len())
    }

    /// The entry at hand on agent `id`'s focus, read afresh when it was
    /// another's.
    pub(super) fn focus_at(&mut self, id: AgentId) -> usize {
        self.reading_for(id);
        self.reading.at(self.story_len(id))
    }

    /// Agent `id`'s focus scrolled `step` rows down (or up), the entry at
    /// hand no longer the newest followed.
    pub(super) fn focus_page(&mut self, id: AgentId, down: bool, step: usize) {
        let at = self.focus_at(id);
        let r = &mut self.reading;
        r.sel = Some(at);
        r.follow.set(false);
        let top = r.scroll.get();
        r.scroll.set(match down {
            true => top.saturating_add(step),
            false => top.saturating_sub(step),
        });
    }

    /// The raw stream scrolled `step` rows down (or up).
    pub(super) fn trace_page(&mut self, down: bool, step: usize) {
        let by = isize::try_from(step).unwrap_or(isize::MAX);
        self.reading.trace_by(if down { by } else { -by });
    }

    /// A key on agent `id`'s focus (record 102): `j k` the story entry at
    /// hand, pages scroll, `h l` cut it to its first lines or show it
    /// whole; `false` leaves the key to the rest of the screen.
    pub(super) fn focus_key(&mut self, id: AgentId, k: KeyEvent) -> bool {
        let entries = self.story_len(id);
        let at = self.focus_at(id);
        let page = self.reading.page.get();
        let r = &mut self.reading;
        match k.code {
            KeyCode::Up | KeyCode::Char('k') => r.sel = Some(at.saturating_sub(1)),
            KeyCode::Down | KeyCode::Char('j') => {
                r.sel = (at + 1 < entries.saturating_sub(1)).then_some(at + 1)
            }
            KeyCode::Home => r.sel = Some(0),
            KeyCode::End => r.sel = None,
            KeyCode::Left | KeyCode::Char('h') => r.folds.set(at, true),
            KeyCode::Right | KeyCode::Char('l') => r.folds.set(at, false),
            KeyCode::PageUp | KeyCode::PageDown => {
                self.focus_page(id, k.code == KeyCode::PageDown, page);
                return true;
            }
            _ => return false,
        }
        self.reading.follow.set(true);
        true
    }

    /// A key on agent `id`'s raw stream: `j k` and pages scroll it, back
    /// to following the newest at its foot; `false` leaves the key to the
    /// rest of the screen.
    pub(super) fn trace_key(&mut self, id: AgentId, k: KeyEvent) -> bool {
        self.reading_for(id);
        let page = self.reading.trace_page.get();
        match k.code {
            KeyCode::Up | KeyCode::Char('k') => self.trace_page(false, 1),
            KeyCode::Down | KeyCode::Char('j') => self.trace_page(true, 1),
            KeyCode::PageUp => self.trace_page(false, page),
            KeyCode::PageDown => self.trace_page(true, page),
            KeyCode::Home => self.reading.trace_top = Some(0),
            KeyCode::End => self.reading.trace_top = None,
            _ => return false,
        }
        true
    }

    /// The file the entry at hand on agent `id`'s focus says it changed
    /// (`changed src/a.rs`, in or outside its card's paths); none on any
    /// other line, and on a card's screen, whose story has no line at
    /// hand.
    pub fn change_line(&self, id: AgentId) -> Option<&str> {
        if self.on_card() || self.thread.is_some() {
            return None;
        }
        let story = &self.world.cards[self.world.agents[id].card?].story;
        let at = match self.reading.agent == Some(id) {
            true => self.reading.at(story.len()),
            false => story.len().saturating_sub(1),
        };
        match &story.get(at)?.event {
            StoryEvent::Touched(file) | StoryEvent::Strayed(file) => Some(file),
            _ => None,
        }
    }

    /// `enter` on a change line of agent `id`'s log (record 141): its
    /// change opens at that file's part of the patch, unfolded, the lines
    /// marked as the ones asked for ([`App::pointed`]) and lit for a
    /// moment where motion moves. With no change waiting, or one that no
    /// longer has the file, a line says so and the log stays.
    pub(super) fn open_change_line(&mut self, id: AgentId) {
        let Some(file) = self.change_line(id).map(str::to_string) else {
            return;
        };
        let t = self.lang.text();
        let name = &self.world.agents[id].name;
        let Some(c) = self.world.change_of(id) else {
            self.flash(fill(t.line_no_change, &[name, &file]));
            return;
        };
        let heads = (c.patch.lines().enumerate()).filter(|(_, l)| l.starts_with("diff --git "));
        let found = heads
            .enumerate()
            .find(|(_, (_, l))| ui::head_path(l) == file)
            .map(|(n, (line, _))| (n, line));
        let Some((n, line)) = found else {
            self.flash(fill(t.line_not_in_change, &[&file, name]));
            return;
        };
        let patch = patch_key(&c.patch);
        let mut folds = self.diff_folds_of(id);
        folds.set(n, false);
        self.diff_folds = Some((id, patch, folds));
        self.diff_scroll = line;
        self.zoom = Zoom::Diff(id);
        self.pointed = Some(Pointed {
            worker: id,
            patch,
            file: n,
        });
        self.light(GlowOn::Lines, Glow::Point, Instant::now());
    }

    /// The file of agent `id`'s open change the log pointed at, by its
    /// number in the patch; none once the change is another.
    pub fn pointed_in(&self, id: AgentId) -> Option<usize> {
        let p = self.pointed.filter(|p| p.worker == id)?;
        let c = self.world.change_of(id)?;
        (patch_key(&c.patch) == p.patch).then_some(p.file)
    }

    /// What agent `id` waits for the person to decide, as its focus
    /// shows it: its oldest open ask (the one its status quotes), else
    /// its change; an index into `World.inbox`.
    pub fn focus_ask(&self, id: AgentId) -> Option<usize> {
        let w = &self.world;
        let first = |kind| {
            w.inbox
                .iter()
                .position(|i| i.open && i.kind == kind && i.agent == id)
        };
        first(InboxKind::Approval).or_else(|| first(InboxKind::Change))
    }

    /// The agent whose focus or raw stream is open with no conversation
    /// over it: the one `a`, `r` and `A` decide for there.
    pub fn zoomed_agent(&self) -> Option<AgentId> {
        match self.zoom {
            Zoom::Focus(id) | Zoom::Trace(id) if self.thread.is_none() => Some(id),
            _ => None,
        }
    }

    /// `a`, `r` or `A` on agent `id`'s focus or raw stream: they decide
    /// what that agent waits for (`focus_ask`), never what happens to be
    /// selected in waiting for you, which the person does not see from
    /// here; `A` with no ask opens the archives, as anywhere. `false`
    /// leaves another key to the rest of the screen.
    pub(super) fn zoom_answer_key(&mut self, id: AgentId, k: KeyEvent) -> bool {
        let approve = match k.code {
            KeyCode::Char('a') => true,
            KeyCode::Char('r') => false,
            KeyCode::Char('e') => {
                match self.focus_ask(id) {
                    Some(item) if self.world.inbox[item].kind == InboxKind::Approval => {
                        self.answer_turn(item)
                    }
                    _ => {
                        let t = self.lang.text();
                        self.flash(fill(t.turn_yes_none, &[&self.world.agents[id].name]));
                    }
                }
                return true;
            }
            KeyCode::Char('A') => {
                match self.focus_ask(id) {
                    Some(item) if self.world.inbox[item].kind == InboxKind::Approval => {
                        self.answer_always_to(item)
                    }
                    // Not the selection in waiting for you either.
                    _ => self.open_archives(),
                }
                return true;
            }
            _ => return false,
        };
        match self.focus_ask(id) {
            Some(item) => self.answer_item(item, approve),
            None => {
                let t = self.lang.text();
                self.flash(fill(t.focus_no_ask, &[&self.world.agents[id].name]));
            }
        }
        true
    }
}

#[cfg(test)]
mod tests {
    use super::super::tests::*;
    use super::*;

    /// A change of two files, `x` (its header line 1) and `y` (line 5).
    const TWO: &str = "note\ndiff --git a/x b/x\nnew file mode 1\n@@ -0 +1 @@\n+one\n\
                       diff --git a/y b/y\nnew file mode 1\n@@ -0 +1 @@\n+two\n+three\n";

    /// A worker whose change of `TWO` waits, its log open: entry 0 a
    /// decision, 1 `changed x`, 2 `changed y` outside its card's paths,
    /// 3 `changed z`, which the change does not have.
    fn log_with_change() -> (App, AgentId) {
        let (mut app, w) = with_change(TWO);
        let c = app.world.cards[0].id;
        app.world.agents[w].card = Some(c);
        app.world.cards[c].story.clear();
        for e in [
            StoryEvent::Decision("x first".into()),
            StoryEvent::Touched("x".into()),
            StoryEvent::Strayed("y".into()),
            StoryEvent::Touched("z".into()),
        ] {
            app.world.story(Some(c), e);
        }
        app.zoom = Zoom::Focus(w);
        app.motion = Motion::Calm;
        (app, w)
    }

    #[test]
    fn enter_on_a_change_line_of_the_log_opens_the_change_at_that_file() {
        let (mut app, w) = log_with_change();
        let key_of = patch_key(TWO);
        // The second file, folded: opened, unfolded, at its header.
        let mut folds = Folds::default();
        folds.set(1, true);
        app.diff_folds = Some((w, key_of, folds));
        app.reading_for(w);
        app.reading.sel = Some(2);
        assert_eq!(app.change_line(w), Some("y"));
        app.on_key(key(KeyCode::Enter));
        assert_eq!((app.zoom, app.diff_scroll), (Zoom::Diff(w), 5));
        let pointed = Pointed {
            worker: w,
            patch: key_of,
            file: 1,
        };
        assert_eq!(app.pointed, Some(pointed));
        assert_eq!(app.pointed_in(w), Some(1));
        assert!(!app.diff_folds_of(w).folded(1), "its lines are shown");
        assert_eq!(flashed(&app), None);
        // Its lines light from the next beat on.
        let lit = (app.glows.iter()).find(|g| g.0 == GlowOn::Lines).unwrap();
        assert_eq!(lit.1, Glow::Point);
        // Reading on keeps the mark; `esc` goes back to the log, at the
        // line it was opened from, and the mark goes with the change.
        app.on_key(key(KeyCode::Char('j')));
        assert_eq!((app.diff_scroll, app.pointed), (6, Some(pointed)));
        app.on_key(key(KeyCode::Esc));
        assert_eq!((app.zoom, app.reading.sel), (Zoom::Focus(w), Some(2)));
        assert_eq!(app.pointed, None);
        // The first file's line: the first header, not the patch's top.
        app.reading.sel = Some(1);
        assert_eq!(app.change_line(w), Some("x"));
        app.on_key(key(KeyCode::Enter));
        assert_eq!((app.zoom, app.diff_scroll), (Zoom::Diff(w), 1));
        assert_eq!(app.pointed_in(w), Some(0));
        // Another change of the worker's is not the one pointed at, nor
        // is another worker's.
        assert_eq!(app.pointed_in(w + 1), None);
        app.world.changes[0].patch.push_str("+more\n");
        assert_eq!(app.pointed_in(w), None);
        // Opened as any change is, `esc` leaves to Home as before.
        app.pointed = None;
        app.on_key(key(KeyCode::Esc));
        assert_eq!(app.zoom, Zoom::None);
    }

    #[test]
    fn a_change_line_with_nothing_to_open_says_so_and_another_line_opens_the_raw_stream() {
        let (mut app, w) = log_with_change();
        let t = Lang::En.text();
        let name = app.world.agents[w].name.clone();
        app.reading_for(w);
        // A file the change no longer has.
        app.reading.sel = Some(3);
        app.on_key(key(KeyCode::Enter));
        assert_eq!(app.zoom, Zoom::Focus(w), "the log stays");
        assert_eq!(
            flashed(&app),
            Some(fill(t.line_not_in_change, &["z", &name]))
        );
        assert_eq!(
            flashed(&app).unwrap(),
            format!("z is not in the change {name} shows now")
        );
        assert_eq!(app.pointed, None);
        assert!(app.glows.iter().all(|g| g.0 != GlowOn::Lines));
        // No change waiting.
        app.world.changes.clear();
        app.reading.sel = Some(1);
        app.on_key(key(KeyCode::Enter));
        assert_eq!(app.zoom, Zoom::Focus(w));
        assert_eq!(
            flashed(&app).unwrap(),
            format!("{name} shows no change yet: x opens here once one waits")
        );
        assert_eq!(app.pointed, None);
        // A line that is no change: the raw stream, as before; and from
        // there `enter` opens nothing.
        app.reading.sel = Some(0);
        assert_eq!(app.change_line(w), None);
        app.on_key(key(KeyCode::Enter));
        assert_eq!(app.zoom, Zoom::Trace(w));
        app.zoom = Zoom::Focus(w);
        // The newest line is the one at hand when none was chosen, also
        // for an agent not read yet.
        app.reading = Reading::default();
        assert_eq!(app.change_line(w), Some("z"));
        app.reading.agent = Some(w);
        assert_eq!(app.change_line(w), Some("z"));
        // A conversation over the log, a card's screen: no line at hand.
        app.card = Some(app.world.cards[0].id);
        assert_eq!(app.change_line(w), None);
        app.card = None;
        assert_eq!(app.change_line(w), Some("z"));
        // An agent without a card has no log line.
        app.world.agents[w].card = None;
        assert_eq!(app.change_line(w), None);
    }

    #[test]
    fn the_demo_s_change_opens_from_the_log_line_of_a_file_its_worker_edited() {
        let mut app = App::new(1, 3, 0, 7, Lang::En);
        // Until a worker's change waits that has a file its log names.
        let found = (0..4000).find_map(|_| {
            app.step();
            let w = &app.world;
            w.changes.iter().find_map(|c| {
                let story = &w.cards[w.agents[c.agent].card?].story;
                let at = story.iter().position(|s| {
                    matches!(&s.event, StoryEvent::Touched(f)
                        if c.files.iter().any(|file| file.path == *f))
                })?;
                Some((c.agent, at))
            })
        });
        let (id, at) = found.expect("the demo shows a change of a file its worker edited");
        let file = match &app.world.cards[app.world.agents[id].card.unwrap()].story[at].event {
            StoryEvent::Touched(f) => f.clone(),
            other => panic!("{other:?}"),
        };
        app.zoom = Zoom::Focus(id);
        app.reading_for(id);
        app.reading.sel = Some(at);
        app.on_key(key(KeyCode::Enter));
        assert_eq!(app.zoom, Zoom::Diff(id), "{:?}", flashed(&app));
        let patch = &app.world.change_of(id).unwrap().patch;
        let head = patch.lines().nth(app.diff_scroll).unwrap();
        assert_eq!(head, format!("diff --git a/{file} b/{file}"));
        assert_eq!(app.pointed_in(id), ui::file_of(patch, app.diff_scroll));
    }

    #[test]
    fn a_conversation_over_a_focus_keeps_its_keys_from_the_focus() {
        let (mut app, worker) = with_thread();
        app.zoom = Zoom::Focus(worker);
        assert_eq!(app.focus_zoom(), None);
        app.thread = None;
        assert_eq!(app.focus_zoom(), Some(worker));
    }

    #[test]
    fn a_r_and_capital_a_on_a_focus_decide_what_its_agent_waits_for() {
        let mut app = App::new(1, 2, 0, 1, Lang::En);
        let t = Lang::En.text();
        let flash = |app: &App| app.flash.as_ref().map(|(m, _)| m.clone());
        let p = app.current_project();
        let (one, two) = (
            app.world.projects[p].leader + 1,
            app.world.projects[p].leader + 2,
        );
        let name = app.world.agents[one].name.clone();
        // An ask decided before: the open ones are not the first items.
        app.world.asks_you(one, "Bash: npm install old");
        app.world.decided(0, false);
        app.world.asks_you(two, "Bash: make other");
        app.world.asks_you(one, "Bash: make first");
        app.world.asks_you(one, "Bash: make second");
        app.world.asks_you(one, "Bash: npm install zod@3");
        // Waiting for you has the other agent's ask selected: the focus
        // never decides that one, which it does not show.
        app.pane = Pane::Inbox;
        app.inbox_sel = 0;
        app.zoom = Zoom::Focus(one);
        assert_eq!(app.zoomed_agent(), Some(one));
        assert_eq!(app.focus_ask(one), Some(2), "its oldest open ask");
        assert_eq!(app.focus_ask(two), Some(1));
        let open = |app: &App| -> Vec<String> {
            (app.world.open_inbox().iter())
                .map(|&i| app.world.inbox[i].detail.clone())
                .collect()
        };
        app.on_key(key(KeyCode::Char('a')));
        assert_eq!(flash(&app), Some(fill(t.allowed, &[&name])));
        assert_eq!(
            open(&app),
            [
                "Bash: make other",
                "Bash: make second",
                "Bash: npm install zod@3"
            ]
        );
        assert!(app.world.journal.iter().any(|e| matches!(
            &e.event,
            Event::Decided { agent, allowed: true, what } if *agent == one && what == "Bash: make first"
        )));
        // On its raw stream too; `r` refuses.
        app.zoom = Zoom::Trace(one);
        app.on_key(key(KeyCode::Char('r')));
        assert_eq!(flash(&app), Some(fill(t.refused, &[&name])));
        assert!(app.world.journal.iter().any(|e| matches!(
            &e.event,
            Event::Decided { agent, allowed: false, what } if *agent == one && what == "Bash: make second"
        )));
        assert_eq!(open(&app), ["Bash: make other", "Bash: npm install zod@3"]);
        // `A`: yes from now on to it, not the archives.
        app.zoom = Zoom::Focus(one);
        app.on_key(key(KeyCode::Char('A')));
        assert!(app.archives.is_none());
        assert_eq!(
            flash(&app),
            Some(format!(
                "{}: yes from now on to npm install …",
                app.world.projects[p].name
            ))
        );
        assert_eq!(open(&app), ["Bash: make other"]);
        assert_eq!(app.focus_ask(one), None);
        // Nothing of its own waits: said, and the other agent's ask stays.
        app.flash = None;
        app.on_key(key(KeyCode::Char('a')));
        assert_eq!(flash(&app), Some(fill(t.focus_no_ask, &[&name])));
        app.flash = None;
        app.on_key(key(KeyCode::Char('r')));
        assert_eq!(flash(&app), Some(fill(t.focus_no_ask, &[&name])));
        assert_eq!(open(&app), ["Bash: make other"]);
        // `A` with no ask is the archives, as anywhere (none here).
        app.on_key(key(KeyCode::Char('A')));
        assert_eq!(flash(&app), Some(t.no_archives.to_string()));
        // A conversation over the focus takes the keys: nothing decided.
        app.zoom = Zoom::Focus(two);
        app.open_thread(0);
        assert!(app.thread.is_some());
        assert_eq!(app.zoomed_agent(), None);
        app.thread = None;
        assert_eq!(app.zoomed_agent(), Some(two));
        app.zoom = Zoom::Diff(two);
        assert_eq!(app.zoomed_agent(), None);
        app.zoom = Zoom::None;
        assert_eq!(app.zoomed_agent(), None);
        // What a shell reads more of: a yes to that line, on a focus too.
        app.world.asks_you(one, "Bash: rm -rf dist > log");
        app.zoom = Zoom::Focus(one);
        app.on_key(key(KeyCode::Char('A')));
        assert!(app.archives.is_none());
        assert_eq!(
            flash(&app),
            Some(format!(
                "{}: yes from now on to `rm -rf dist > log`",
                app.world.projects[p].name
            ))
        );
        assert_eq!(app.focus_ask(one), None);
        // An edit can have no yes from now on: said, nothing decided.
        app.world.asks_you(one, "Edit: package.json");
        app.zoom = Zoom::Focus(one);
        app.on_key(key(KeyCode::Char('A')));
        assert!(app.archives.is_none());
        assert_eq!(flash(&app), Some(t.always_not_command.to_string()));
        assert_eq!(open(&app).len(), 2);
    }

    #[test]
    fn a_and_r_on_a_focus_decide_its_workers_change_after_its_asks() {
        use crate::live::testing::{hold_change, idle_live, team};
        let t = Lang::En.text();
        let flash = |app: &App| app.flash.as_ref().map(|(m, _)| m.clone());
        let mut w = team(1);
        w.change_ready(Change {
            agent: 1,
            title: "did it".into(),
            files: Vec::new(),
            patch: String::new(),
            runs: Vec::new(),
            check: Check::None,
            spend: None,
        });
        w.asks_you(1, "Bash: make");
        let mut live = idle_live(2);
        hold_change(&mut live, 1);
        let mut app = App::with(w, Engine::Live(Box::new(live)), Lang::En);
        app.zoom = Zoom::Focus(1);
        let change = app
            .world
            .inbox
            .iter()
            .position(|i| i.kind == InboxKind::Change);
        let ask = app
            .world
            .inbox
            .iter()
            .position(|i| i.kind == InboxKind::Approval);
        assert_eq!(app.focus_ask(1), ask, "the ask first, though it came later");
        // `A` on a change is not a yes from now on: the archives.
        app.world.inbox[ask.unwrap()].open = false;
        assert_eq!(app.focus_ask(1), change);
        app.on_key(key(KeyCode::Char('A')));
        assert_eq!(flash(&app), Some(t.no_archives.to_string()));
        assert!(app.world.change_of(1).is_some());
        // A change is no ask: `e` and `:yes` give no yes for the turn.
        app.on_key(key(KeyCode::Char('e')));
        assert_eq!(flash(&app), Some(fill(t.turn_yes_none, &["claude-1"])));
        app.flash = None;
        app.command("yes @claude-1");
        assert_eq!(flash(&app), Some(fill(t.turn_yes_none, &["claude-1"])));
        assert_eq!(app.world.agents[1].turn_yes, None);
        assert!(app.world.change_of(1).is_some());
        app.on_key(key(KeyCode::Char('a')));
        assert_eq!(flash(&app), Some(fill(t.applying, &["claude-1"])));
        assert_eq!(app.zoom, Zoom::Focus(1), "the focus stays open");
        let mut live = idle_live(2);
        hold_change(&mut live, 1);
        let mut w = team(1);
        w.change_ready(Change {
            agent: 1,
            title: "did it".into(),
            files: Vec::new(),
            patch: String::new(),
            runs: Vec::new(),
            check: Check::None,
            spend: None,
        });
        let mut app = App::with(w, Engine::Live(Box::new(live)), Lang::En);
        app.zoom = Zoom::Trace(1);
        app.on_key(key(KeyCode::Char('r')));
        assert_eq!(flash(&app), Some(fill(t.rejected_change, &["claude-1"])));
        assert!(app.world.change_of(1).is_none());
    }
}
