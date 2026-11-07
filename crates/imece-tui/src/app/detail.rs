//! The detail screen (record 125): one piece of work, an agent's, with
//! three tabs: diff (its change), talk (the conversation with it) and
//! log (its focus, or its raw stream). The bodies are the screens imece
//! had; this names them, frames them and moves between them.

use super::*;

/// A tab of the detail.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Tab {
    Diff,
    Talk,
    Log,
}

impl Tab {
    pub const ALL: [Tab; 3] = [Tab::Diff, Tab::Talk, Tab::Log];

    /// The tab after this one, round; `back`, the one before.
    pub fn next(self, back: bool) -> Tab {
        match (self, back) {
            (Tab::Diff, false) | (Tab::Log, true) => Tab::Talk,
            (Tab::Talk, false) | (Tab::Diff, true) => Tab::Log,
            (Tab::Log, false) | (Tab::Talk, true) => Tab::Diff,
        }
    }
}

impl App {
    /// The piece of work on screen and the tab it is open at: the agent
    /// whose conversation, change or focus is open; none on a view.
    pub fn detail(&self) -> Option<(AgentId, Tab)> {
        if let Some(id) = self.thread_agent() {
            return Some((id, Tab::Talk));
        }
        match self.zoom {
            Zoom::Diff(id) => Some((id, Tab::Diff)),
            Zoom::Focus(id) | Zoom::Trace(id) => Some((id, Tab::Log)),
            Zoom::None => None,
        }
    }

    /// The agent the open conversation is with: the one that wrote its
    /// first line, else the one it was written to.
    pub fn thread_agent(&self) -> Option<AgentId> {
        let th = self.thread.as_ref()?;
        let first = &self.world.journal[*th.entries.first()?].event;
        let s = ui::words::said(first)?;
        match (s.from, s.to) {
            (Party::Agent(id), _) | (Party::Person, Party::Agent(id)) => Some(id),
            (Party::Person, Party::Person) => None,
        }
    }

    /// Whether `tab` has anything to show of agent `id`: a change for the
    /// diff, a line said for the talk; the log always.
    pub fn tab_has(&self, id: AgentId, tab: Tab) -> bool {
        match tab {
            Tab::Diff => self.world.change_of(id).is_some(),
            Tab::Talk => self.last_talk(id).is_some(),
            Tab::Log => true,
        }
    }

    /// The journal's newest line between agent `id` and the person, else
    /// its newest with anyone.
    fn last_talk(&self, id: AgentId) -> Option<usize> {
        let me = Party::Agent(id);
        let pair = (me.min(Party::Person), me.max(Party::Person));
        let newest = |with_person: bool| {
            (self.world.journal.iter()).rposition(|e| {
                ui::words::said(&e.event).is_some_and(|s| {
                    if with_person {
                        s.pair() == pair
                    } else {
                        s.from == me || s.to == me
                    }
                })
            })
        };
        newest(true).or_else(|| newest(false))
    }

    /// `tab` on the detail: the next tab of the same piece of work that
    /// has something to show; nothing moves when no other has.
    pub(super) fn next_tab(&mut self, back: bool) {
        let Some((id, at)) = self.detail() else {
            return;
        };
        let mut tab = at.next(back);
        while tab != at && !self.tab_has(id, tab) {
            tab = tab.next(back);
        }
        if tab != at {
            self.open_tab(id, tab);
        }
    }

    /// Opens tab `tab` of agent `id`'s detail: the diff and the log close
    /// a conversation; the talk opens over what was there, as `esc` then
    /// comes back to it.
    pub(super) fn open_tab(&mut self, id: AgentId, tab: Tab) {
        match tab {
            Tab::Diff => {
                self.thread = None;
                if self.zoom != Zoom::Diff(id) {
                    self.diff_scroll = 0;
                }
                self.zoom = Zoom::Diff(id);
            }
            Tab::Talk => {
                if let Some(i) = self.last_talk(id) {
                    self.open_thread(i);
                }
            }
            Tab::Log => {
                self.thread = None;
                if !matches!(self.zoom, Zoom::Focus(a) | Zoom::Trace(a) if a == id) {
                    self.zoom = Zoom::Focus(id);
                }
            }
        }
    }

    /// The card the piece of work is about: the one an open item of the
    /// agent's came on (a change stays with the card it was made for),
    /// else the agent's.
    pub fn detail_card(&self, id: AgentId) -> Option<CardId> {
        let w = &self.world;
        // A card's own screen is about that card.
        if let Some(c) = self.card {
            return Some(c);
        }
        (w.open_inbox().into_iter())
            .map(|i| &w.inbox[i])
            .filter(|it| it.agent == id)
            .find_map(|it| it.card)
            .or(w.agents[id].card)
    }
}

#[cfg(test)]
mod tests {
    use super::super::tests::*;
    use super::*;

    fn team() -> App {
        let mut app = App::new(1, 2, 0, 1, Lang::En);
        app.world.inbox.clear();
        for a in &mut app.world.agents {
            a.status = Status::Idle;
            a.starting = false;
        }
        app
    }

    fn workers(app: &App) -> (AgentId, AgentId) {
        let p = app.current_project();
        let lead = app.world.projects[p].leader;
        (lead + 1, lead + 2)
    }

    fn change(agent: AgentId) -> Change {
        Change {
            agent,
            title: "Paging".into(),
            files: vec![ChangedFile {
                path: "src/a.rs".into(),
                added: 2,
                removed: 1,
                binary: false,
            }],
            patch: "diff --git a/x b/x\n@@ -1 +1 @@\n-a\n+b\n".into(),
            runs: Vec::new(),
            check: Check::None,
            spend: None,
        }
    }

    #[test]
    fn the_tabs_go_round_both_ways() {
        assert_eq!(Tab::Diff.next(false), Tab::Talk);
        assert_eq!(Tab::Talk.next(false), Tab::Log);
        assert_eq!(Tab::Log.next(false), Tab::Diff);
        assert_eq!(Tab::Diff.next(true), Tab::Log);
        assert_eq!(Tab::Talk.next(true), Tab::Diff);
        assert_eq!(Tab::Log.next(true), Tab::Talk);
        assert_eq!(Tab::ALL, [Tab::Diff, Tab::Talk, Tab::Log]);
    }

    #[test]
    fn the_detail_is_the_conversation_first_then_the_zoom_and_names_its_agent() {
        let mut app = team();
        let (one, two) = workers(&app);
        assert_eq!(app.detail(), None, "a view");
        app.zoom = Zoom::Diff(one);
        assert_eq!(app.detail(), Some((one, Tab::Diff)));
        app.zoom = Zoom::Focus(two);
        assert_eq!(app.detail(), Some((two, Tab::Log)));
        app.zoom = Zoom::Trace(two);
        assert_eq!(app.detail(), Some((two, Tab::Log)));
        // A conversation over a zoom is the talk tab of the agent that
        // wrote its first line, else the one written to.
        app.world.said(one, "go on");
        app.open_thread(app.world.journal.len() - 1);
        assert_eq!(app.thread_agent(), Some(one), "the person wrote to one");
        assert_eq!(app.detail(), Some((one, Tab::Talk)));
        app.thread = None;
        app.world.note(Event::Sent {
            from: two,
            to: one,
            kind: MsgKind::Ask,
            text: "Which?".into(),
            id: 1,
            re: None,
        });
        app.open_thread(app.world.journal.len() - 1);
        assert_eq!(app.thread_agent(), Some(two), "two wrote first");
        app.thread = None;
        app.world.asked_you(one, "May I?");
        app.world.note(Event::ToYou {
            from: one,
            kind: MsgKind::Ask,
            text: "May I?".into(),
            options: Vec::new(),
        });
        app.open_thread(app.world.journal.len() - 1);
        assert_eq!(app.thread_agent(), Some(one));
    }

    #[test]
    fn a_tab_has_something_to_show_or_is_skipped() {
        let mut app = team();
        let (one, two) = workers(&app);
        assert!(!app.tab_has(one, Tab::Diff), "no change");
        assert!(!app.tab_has(one, Tab::Talk), "nothing said");
        assert!(app.tab_has(one, Tab::Log), "the log always");
        app.world.change_ready(change(one));
        assert!(app.tab_has(one, Tab::Diff));
        assert!(!app.tab_has(two, Tab::Diff), "another's change");
        app.world.said(two, "hi");
        assert!(app.tab_has(two, Tab::Talk));
        assert!(!app.tab_has(one, Tab::Talk), "said to another");
        // From the diff, with nothing said: the log, then round to the
        // diff; back goes the other way.
        app.zoom = Zoom::Diff(one);
        app.diff_scroll = 3;
        app.next_tab(false);
        assert_eq!(app.detail(), Some((one, Tab::Log)));
        assert_eq!(app.zoom, Zoom::Focus(one));
        app.next_tab(false);
        assert_eq!(app.detail(), Some((one, Tab::Diff)));
        assert_eq!(app.diff_scroll, 0, "opened again: from the top");
        app.next_tab(true);
        assert_eq!(app.detail(), Some((one, Tab::Log)));
        app.next_tab(true);
        assert_eq!(app.detail(), Some((one, Tab::Diff)));
        // The log alone: nothing moves.
        app.zoom = Zoom::Focus(two);
        app.world.changes.clear();
        app.world.journal.clear();
        app.next_tab(false);
        assert_eq!(app.detail(), Some((two, Tab::Log)));
        assert_eq!(app.zoom, Zoom::Focus(two));
    }

    #[test]
    fn the_talk_tab_is_the_conversation_with_the_person_else_the_newest_with_anyone() {
        let mut app = team();
        let (one, two) = workers(&app);
        app.world.said(one, "first, with the person");
        let with_person = app.world.journal.len() - 1;
        app.world.note(Event::Sent {
            from: one,
            to: two,
            kind: MsgKind::Inform,
            text: "newer, to a teammate".into(),
            id: 1,
            re: None,
        });
        app.zoom = Zoom::Focus(one);
        app.next_tab(true);
        assert_eq!(app.detail(), Some((one, Tab::Talk)));
        let th = app.thread.as_ref().unwrap();
        assert!(th.entries.contains(&with_person), "{:?}", th.entries);
        assert_eq!(
            app.zoom,
            Zoom::Focus(one),
            "the talk opens over what was there"
        );
        // Only teammates' lines: those.
        app.thread = None;
        app.world.journal.clear();
        app.world.note(Event::Sent {
            from: two,
            to: one,
            kind: MsgKind::Ask,
            text: "to one".into(),
            id: 2,
            re: None,
        });
        app.open_tab(one, Tab::Talk);
        assert_eq!(app.thread_agent(), Some(two), "the first line's writer");
        assert_eq!(app.detail(), Some((two, Tab::Talk)));
        // The log keeps a raw stream of the same agent; the diff and the
        // log close a conversation.
        app.zoom = Zoom::Trace(one);
        app.open_tab(one, Tab::Log);
        assert_eq!((app.zoom, app.thread.is_none()), (Zoom::Trace(one), true));
        app.open_tab(two, Tab::Log);
        assert_eq!(app.zoom, Zoom::Focus(two));
        app.world.change_ready(change(one));
        app.diff_scroll = 4;
        app.open_tab(one, Tab::Diff);
        assert_eq!((app.zoom, app.diff_scroll), (Zoom::Diff(one), 0));
        app.diff_scroll = 4;
        app.open_tab(one, Tab::Diff);
        assert_eq!(app.diff_scroll, 4, "the same diff keeps its place");
    }

    #[test]
    fn the_detail_s_card_is_the_open_item_s_else_the_agent_s() {
        let mut app = team();
        let (one, _) = workers(&app);
        app.world.agents[one].card = Some(0);
        assert_eq!(app.detail_card(one), Some(0));
        app.world.asked_you(one, "Which?");
        let i = app.world.inbox.len() - 1;
        app.world.inbox[i].card = Some(1);
        assert_eq!(app.detail_card(one), Some(1), "the item's card");
        app.world.inbox[i].open = false;
        assert_eq!(app.detail_card(one), Some(0), "a closed item says nothing");
        app.world.agents[one].card = None;
        assert_eq!(app.detail_card(one), None);
    }

    #[test]
    fn tab_a_digit_and_slash_act_on_the_detail() {
        let mut app = team();
        let (one, _) = workers(&app);
        app.world.change_ready(change(one));
        app.world.note(Event::ToYou {
            from: one,
            kind: MsgKind::Ask,
            text: "Which?".into(),
            options: vec!["the card".into(), "credit".into()],
        });
        app.world.asked_you(one, "Which?");
        app.zoom = Zoom::Diff(one);
        app.on_key(key(KeyCode::Tab));
        assert_eq!(app.detail(), Some((one, Tab::Talk)));
        app.on_key(key(KeyCode::BackTab));
        assert_eq!(app.detail(), Some((one, Tab::Diff)), "back");
        // Off Home, a digit still picks the detail's question's answer.
        assert_eq!(app.question_at_hand(), Some(app.world.inbox.len() - 1));
        app.on_key(key(KeyCode::Char('2')));
        assert!(!app.world.has_question(one), "answered");
        assert!(matches!(
            &app.world.journal.last().unwrap().event,
            Event::YouSaid { to, text, .. } if *to == one && text == "credit"
        ));
        // `/` on a question's detail writes to its agent.
        app.world.asked_you(one, "Again?");
        app.on_key(key(KeyCode::Char('/')));
        assert!(app.typing);
        assert_eq!(app.input, format!("@{} ", app.world.agents[one].name));
        // On the talk tab too, a conversation open: an answer by digit,
        // and a digit naming no answer is a view's as ever.
        app.typing = false;
        app.input.clear();
        app.open_tab(one, Tab::Talk);
        assert!(app.thread.is_some());
        app.on_key(key(KeyCode::Char('3')));
        assert!(app.world.has_question(one), "no third answer");
        app.open_tab(one, Tab::Talk);
        app.on_key(key(KeyCode::Char('1')));
        assert!(!app.world.has_question(one), "answered from the talk tab");
        app.world.asked_you(one, "Once more?");
        app.on_key(key(KeyCode::Char('/')));
        assert_eq!(app.input, format!("@{} ", app.world.agents[one].name));
        // Without a question, `/` is writing as ever.
        app.typing = false;
        app.input.clear();
        app.world.inbox.clear();
        app.on_key(key(KeyCode::Char('/')));
        assert!(app.typing);
        assert_eq!(app.input, "");
    }
}
