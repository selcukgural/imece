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
    /// another agent's starts afresh, leaving both forgets it.
    pub(super) fn keep_reading(&mut self) {
        match self.zoom {
            Zoom::Focus(id) | Zoom::Trace(id) => self.reading_for(id),
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
