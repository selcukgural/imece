//! The salon: its story, rows, selection, panes and cards.

use super::*;

impl App {
    /// Whether the side panel tells the story now: it is on, and no card's
    /// screen shows its own journal.
    pub fn story_on(&self) -> bool {
        self.story && !self.story_away && self.card.is_none()
    }

    /// `J`: the side panel tells the story, or shows the journal again;
    /// from a line of the story shown in the journal, back to the story.
    pub(super) fn toggle_story(&mut self) {
        if self.story_away {
            return self.story_back();
        }
        self.story = !self.story;
        self.story_sel = 0;
        if self.story {
            self.journal_sel = None;
        } else if self.pane == Pane::Journal {
            self.select_journal(None);
        }
        let t = self.lang.text();
        self.flash(if self.story { t.story_on } else { t.story_off });
    }

    /// The story's selected item, while it is shown.
    fn story_item(&self) -> Option<ui::story::Item> {
        if !self.story_on() {
            return None;
        }
        self.story_items.borrow().get(self.story_sel).copied()
    }

    /// `enter` on the story: a card opens its screen, a line of no card is
    /// shown in the journal.
    pub(super) fn open_story_item(&mut self) {
        match self.story_item() {
            Some(ui::story::Item::Card(c)) => self.open_card(c),
            Some(ui::story::Item::Line(i)) => {
                self.story_away = true;
                self.journal_sel = Some(i);
                self.flash(self.lang.text().story_away);
            }
            None => {}
        }
    }

    /// `esc` (or `J`) in the journal shown in the story's place: the story
    /// again, its item at hand as it was.
    pub(super) fn story_back(&mut self) {
        self.story_away = false;
        self.journal_sel = None;
        self.flash(self.lang.text().story_on);
    }

    /// The agents agent `id`'s salon row stands for: itself, its helpers
    /// and the researcher working for it, which hang under it.
    pub(super) fn row_group(&self, id: AgentId) -> Vec<AgentId> {
        let w = &self.world;
        let under = w.agents.iter().filter(|a| {
            !a.gone
                && match a.role {
                    Role::Helper => a.parent == Some(id),
                    Role::Researcher => {
                        matches!(a.status, Status::Researching { for_agent, .. } if for_agent == id)
                    }
                    _ => false,
                }
        });
        std::iter::once(id).chain(under.map(|a| a.id)).collect()
    }

    /// The agents a settled project stands for: all of its own.
    fn project_group(&self, p: ProjectId) -> Vec<AgentId> {
        let w = &self.world;
        (w.agents.iter().filter(|a| a.project == p))
            .map(|a| a.id)
            .collect()
    }

    /// What was settled comes back to the salon when it needs the person
    /// again: an agent when something of its row waits for them, is
    /// stuck or finished a turn not seen; a project when any of its
    /// agents does. Agents gone and projects closed are let go.
    pub(super) fn wake_settled(&mut self) {
        let need = self.needs();
        let calm = |group: Vec<AgentId>| group.iter().all(|&a| need[a] == Need::Nothing);
        let agents = (self.settled.iter().copied())
            .filter(|&id| !self.world.agents[id].gone && calm(self.row_group(id)))
            .collect();
        let projects = (self.settled_projects.iter().copied())
            .filter(|&p| !self.world.projects[p].closed && calm(self.project_group(p)))
            .collect();
        self.settled = agents;
        self.settled_projects = projects;
    }

    /// `s` or `:settle`: the selected worker's row (its helpers with it),
    /// or the selected project (its header or its leader) out of the salon
    /// until it needs the person again; refused while something of it
    /// waits for them or is stuck. Its turns finished count as seen, and
    /// the selection goes on to the next row still shown.
    pub(super) fn settle_at_hand(&mut self) {
        let t = self.lang.text();
        let w = &self.world;
        // What goes: the row's agent, or the project of a header or a
        // leader's row.
        let target = match self.selected {
            Sel::Agent(id) if w.projects[w.agents[id].project].leader == id => {
                Sel::Project(w.agents[id].project)
            }
            sel => sel,
        };
        let (group, name) = match target {
            Sel::Agent(id) => (self.row_group(id), w.agents[id].name.clone()),
            Sel::Project(p) => (self.project_group(p), w.projects[p].name.clone()),
        };
        let need = self.needs();
        if let Some(&loud) = group.iter().find(|&&a| need[a] > Need::Done) {
            let who = self.world.agents[loud].name.clone();
            return self.flash(fill(t.settle_refused, &[&who]));
        }
        for a in &group {
            self.unseen.remove(a);
        }
        match target {
            Sel::Agent(id) => self.settled.insert(id),
            Sel::Project(p) => self.settled_projects.insert(p),
        };
        self.flash(fill(t.settled_one, &[&name]));
    }

    /// `:unsettle`: everything settled back in the team.
    pub(super) fn unsettle(&mut self) {
        let n = self.settled.len() + self.settled_projects.len();
        self.settled.clear();
        self.settled_projects.clear();
        let t = self.lang.text();
        self.flash(fill(t.unsettled, &[&n.to_string()]));
    }

    /// `:waiting`: Home, its first row to decide.
    pub(super) fn go_waiting(&mut self) {
        self.go_home();
        self.home_go_heading(Heading::Decide);
    }

    /// `gc` / `:changes`: Home, its first change waiting; a flash when
    /// none waits.
    pub(super) fn go_changes(&mut self) {
        self.go_home();
        self.first_change();
    }

    /// On Home, the first change waiting, or a flash that none does.
    pub(super) fn first_change(&mut self) {
        if !self.home_go_group(HomeGroup::Change) {
            self.flash(self.lang.text().no_change_waits);
        }
    }

    /// `:salon`: the team, Home's lower half (record 125, step 5).
    pub(super) fn go_team(&mut self) {
        self.go_home();
        self.home_go_heading(Heading::Work);
    }

    /// Folds or unfolds every project.
    pub(super) fn fold_all(&mut self, fold: bool) {
        self.collapsed.iter_mut().for_each(|c| *c = fold);
        if fold {
            self.selected = Sel::Project(self.current_project());
        }
    }

    /// The timeline's selected agent.
    pub(super) fn timeline_agent(&self) -> Option<AgentId> {
        let shown = ui::timeline::agents(self);
        let at = self.timeline_sel.min(shown.len().saturating_sub(1));
        shown.get(at).map(|&(id, _)| id)
    }

    /// The agent the person is looking at: focused, selected in the salon,
    /// or the one behind the selected inbox item.
    pub(crate) fn agent_at_hand(&self) -> Option<AgentId> {
        match self.zoom {
            Zoom::Focus(id) | Zoom::Trace(id) | Zoom::Diff(id) => return Some(id),
            Zoom::None => {}
        }
        if self.window == Some(Window::Timeline) {
            return self.timeline_agent();
        }
        match self.pane {
            Pane::Salon => match self.selected {
                Sel::Agent(id) => Some(id),
                Sel::Project(p) => Some(self.world.projects[p].leader),
            },
            Pane::Inbox => self
                .inbox_rows()
                .get(self.inbox_sel)
                .map(|&i| self.world.inbox[i].agent),
            Pane::Journal if self.story_on() => match self.story_item()? {
                ui::story::Item::Card(c) => self.world.cards[c].owner,
                ui::story::Item::Line(i) => Some(self.world.journal[i].event.agent()),
            },
            Pane::Journal => self
                .journal_sel
                .map(|i| self.world.journal[i].event.agent()),
        }
    }

    /// To the first or last line of what the person is looking at.
    pub(super) fn jump(&mut self, first: bool) {
        if self.copy_move(|_, last, _| if first { 0 } else { last }) {
            return;
        }
        if let Zoom::Diff(id) = self.zoom {
            let lines = self
                .world
                .change_of(id)
                .map_or(0, |c| c.patch.lines().count());
            let last = self.diff_shown(id).step(lines, 0, true);
            self.diff_scroll = if first { 0 } else { last };
            return;
        }
        if let Some(th) = &mut self.thread {
            th.sel = if first {
                0
            } else {
                th.entries.len().saturating_sub(1)
            };
            th.follow.set(true);
            return;
        }
        if let Some(id) = self.focus_zoom() {
            self.reading_for(id);
            self.reading.sel = first.then_some(0);
            self.reading.follow.set(true);
            return;
        }
        if let Zoom::Trace(id) = self.zoom {
            self.reading_for(id);
            self.reading.trace_top = first.then_some(0);
            return;
        }
        match self.pane {
            Pane::Salon => {
                let shown = self.shown.borrow();
                if let Some(&s) = if first { shown.first() } else { shown.last() } {
                    self.selected = s;
                }
            }
            Pane::Inbox => {
                self.waiting_at = None;
                let n = self.inbox_rows().len();
                self.inbox_sel = if first { 0 } else { n.saturating_sub(1) };
            }
            Pane::Journal if self.story_on() => {
                let n = self.story_items.borrow().len();
                self.story_sel = if first { 0 } else { n.saturating_sub(1) };
            }
            Pane::Journal => {
                let rows = self.journal_rows();
                let row = if first { rows.first() } else { rows.last() };
                self.journal_sel = row.map(|&(i, _)| i);
            }
        }
    }

    /// A page (or half of one) down or up: the open conversation scrolls,
    /// a list moves its selection.
    pub(super) fn page(&mut self, down: bool, half: bool) {
        let by = |page: usize| if half { page / 2 } else { page }.max(1);
        if self.copy_move(|at, _, page| match down {
            true => at + by(page),
            false => at.saturating_sub(by(page)),
        }) {
            return;
        }
        // Home's list: five rows a half page, ten a page.
        if self.home_keys() {
            let step = by(10);
            let row = match down {
                true => self.at_home.row + step,
                false => self.at_home.row.saturating_sub(step),
            };
            self.home_go(row);
            return;
        }
        if let Zoom::Diff(id) = self.zoom {
            let page = usize::from(self.diff_page.get()).max(1);
            let step = if half { page / 2 } else { page }.max(1);
            let lines = self
                .world
                .change_of(id)
                .map_or(0, |c| c.patch.lines().count());
            self.diff_scroll = if down {
                (self.diff_scroll + step).min(lines.saturating_sub(1))
            } else {
                self.diff_scroll.saturating_sub(step)
            };
            return;
        }
        if let Some(th) = &self.thread {
            let step = if half {
                th.page.get() / 2
            } else {
                th.page.get()
            }
            .max(1);
            let at = th.scroll.get();
            th.scroll.set(if down {
                at.saturating_add(step)
            } else {
                at.saturating_sub(step)
            });
            return;
        }
        if let Some(id) = self.focus_zoom() {
            self.focus_page(id, down, by(self.reading.page.get()));
            return;
        }
        if let Zoom::Trace(id) = self.zoom {
            self.reading_for(id);
            let step = by(self.reading.trace_page.get());
            self.trace_page(down, step);
            return;
        }
        for _ in 0..if half { 5 } else { 10 } {
            self.move_selection(down);
        }
    }

    /// Keeps the journal selection on a line that is still shown, moving by
    /// `step` lines (newest first) when given.
    pub(super) fn select_journal(&mut self, step: Option<bool>) {
        let rows = self.journal_rows();
        if rows.is_empty() {
            self.journal_sel = None;
            return;
        }
        let pos = self
            .journal_sel
            .and_then(|i| rows.iter().position(|&(r, _)| r == i));
        let pos = match (pos, step) {
            (None, _) => 0,
            (Some(p), None) => p,
            (Some(p), Some(down)) if down => (p + 1).min(rows.len() - 1),
            (Some(p), Some(_)) => p.saturating_sub(1),
        };
        self.journal_sel = Some(rows[pos].0);
    }

    /// Home, whole: whatever was open over it (a focus, a change, a card,
    /// a window) closes.
    pub(crate) fn go_home(&mut self) {
        self.zoom = Zoom::None;
        self.window = None;
        self.leave_card();
        self.pane = Pane::Salon;
    }

    /// Whether a card's screen is what the person looks at (it is only
    /// ever open in the salon view).
    pub(super) fn on_card(&self) -> bool {
        self.card.is_some() && matches!(self.zoom, Zoom::None | Zoom::Focus(_))
    }

    /// The open inbox items the side panel lists: a card's, on its
    /// screen, else all.
    pub fn inbox_rows(&self) -> Vec<usize> {
        match self.card {
            Some(c) => self.world.card_inbox(c),
            None => self.world.shown_inbox(),
        }
    }

    /// The journal rows the side panel lists: a card's, on its screen,
    /// else the whole journal's.
    pub fn journal_rows(&self) -> Vec<(usize, usize)> {
        match self.card {
            Some(c) => ui::card_rows(&self.world, c),
            None => ui::journal_rows(&self.world),
        }
    }

    /// Card `c`'s screen takes the salon's place, its journal selected.
    /// `#`: the card line, with the card at hand already typed (its own
    /// card on a card's screen is no news): from Home's row, a focus, or
    /// the journal window's line, which closes for it.
    pub(super) fn hash_line(&mut self) {
        let card = self.card_at_hand().filter(|&c| Some(c) != self.card);
        if self.window == Some(Window::Journal) {
            self.close_window();
        }
        self.typing = true;
        self.input = match card {
            Some(c) => format!("#{}", c + 1),
            None => "#".into(),
        };
        self.after = 0;
        self.menu = self.suggest();
    }

    /// `#N`: the card's screen as the log tab of its detail (record 125,
    /// step 5): the piece of work is its owner's, else the first who had a
    /// part in it, else its project's leader; a window over the screen
    /// closes.
    pub fn open_card(&mut self, c: CardId) {
        let w = &self.world;
        let card = &w.cards[c];
        let who = (card.owner)
            .or_else(|| w.card_people(c).first().map(|&(id, _)| id))
            .unwrap_or(w.projects[card.project].leader);
        self.card = Some(c);
        self.close_window();
        self.thread = None;
        self.pane = Pane::Salon;
        self.inbox_sel = 0;
        self.journal_sel = None;
        self.select_journal(None);
        self.selected = Sel::Agent(who);
        self.zoom = Zoom::Focus(who);
    }

    /// Back from a card's screen to the salon.
    pub(crate) fn leave_card(&mut self) {
        if self.card.take().is_some() {
            self.inbox_sel = 0;
            self.select_journal(None);
            if matches!(self.zoom, Zoom::Focus(_)) {
                self.zoom = Zoom::None;
            }
        }
    }

    /// The next card (or the previous one) of an open project, by number.
    pub(super) fn step_card(&mut self, next: bool) {
        let Some(c) = self.card else {
            return;
        };
        let w = &self.world;
        let mut open = w.cards.iter().filter(|x| !w.projects[x.project].closed);
        let to = if next {
            open.find(|x| x.id > c)
        } else {
            open.rfind(|x| x.id < c)
        };
        if let Some(to) = to.map(|x| x.id) {
            self.open_card(to);
        }
    }

    /// The card the person is looking at: the zoomed agent's, the selected
    /// agent's, the one the selected inbox item came on, or the one a
    /// selected journal line names (or mentions, `#323`).
    pub(crate) fn card_at_hand(&self) -> Option<CardId> {
        let w = &self.world;
        // A card's own screen is about that card.
        if let Some(c) = self.card {
            return Some(c);
        }
        if let Zoom::Focus(id) | Zoom::Trace(id) | Zoom::Diff(id) = self.zoom {
            return w.card_of(id);
        }
        match self.pane {
            Pane::Salon => match self.selected {
                Sel::Agent(id) => w.card_of(id),
                Sel::Project(_) => None,
            },
            Pane::Inbox => self
                .inbox_rows()
                .get(self.inbox_sel)
                .and_then(|&i| w.inbox[i].card),
            Pane::Journal if self.story_on() => match self.story_item()? {
                ui::story::Item::Card(c) => Some(c),
                ui::story::Item::Line(_) => None,
            },
            Pane::Journal => {
                let e = &w.journal[self.journal_sel?].event;
                e.card().or_else(|| {
                    let text = ui::words::said(e)?.text;
                    text.match_indices('#')
                        .find_map(|(at, _)| complete::card_named(&text[at..], w))
                })
            }
        }
    }

    pub(super) fn move_selection(&mut self, down: bool) {
        match self.pane {
            Pane::Journal if self.story_on() => {
                let n = self.story_items.borrow().len();
                self.story_sel = match down {
                    true => (self.story_sel + 1).min(n.saturating_sub(1)),
                    false => self.story_sel.saturating_sub(1),
                };
            }
            Pane::Journal => self.select_journal(Some(down)),
            Pane::Inbox => {
                // The person picks: `W` goes on from what they picked.
                self.waiting_at = None;
                let n = self.inbox_rows().len();
                if n > 0 {
                    self.inbox_sel = if down {
                        (self.inbox_sel + 1).min(n - 1)
                    } else {
                        self.inbox_sel.saturating_sub(1)
                    };
                }
            }
            Pane::Salon => {
                let next = {
                    let shown = self.shown.borrow();
                    if shown.is_empty() {
                        return;
                    }
                    let pos = shown.iter().position(|&s| s == self.selected).unwrap_or(0);
                    let pos = if down {
                        (pos + 1).min(shown.len() - 1)
                    } else {
                        pos.saturating_sub(1)
                    };
                    shown[pos]
                };
                self.selected = next;
            }
        }
    }

    pub(super) fn deeper(&mut self) {
        let zoom = match self.zoom {
            Zoom::None => match (self.pane, self.selected) {
                (Pane::Inbox, _) => {
                    let open = self.inbox_rows();
                    // A setup script's ask: its bytes in a window.
                    if let Some(&i) = open.get(self.inbox_sel)
                        && matches!(
                            self.world.inbox[i].kind,
                            InboxKind::Script | InboxKind::Check
                        )
                    {
                        self.script = Some((i, 0));
                        return;
                    }
                    // The leader's plan: its cards in a window.
                    if let Some(&i) = open.get(self.inbox_sel)
                        && self.world.inbox[i].kind == InboxKind::Plan
                    {
                        self.plan_window = Some((i, 0));
                        return;
                    }
                    let item = open.get(self.inbox_sel).map(|&i| &self.world.inbox[i]);
                    match item {
                        Some(item) if item.kind == InboxKind::Change => {
                            self.diff_scroll = 0;
                            Some(Zoom::Diff(item.agent))
                        }
                        Some(item) if item.kind == InboxKind::Question => {
                            // The conversation shows the whole question; it
                            // waits until the person writes back.
                            let agent = item.agent;
                            if let Some(i) = self.world.journal.iter().rposition(|e| {
                                matches!(e.event, Event::ToYou { from, kind: MsgKind::Ask, .. } if from == agent)
                            }) {
                                self.open_thread(i);
                                return;
                            }
                            Some(Zoom::Focus(agent))
                        }
                        Some(item) if item.kind == InboxKind::Reply => {
                            // Read now: the conversation shows the whole answer.
                            let agent = item.agent;
                            self.world.close_replies(agent);
                            self.inbox_sel = self.inbox_sel.min(open.len().saturating_sub(2));
                            if let Some(i) = self.world.journal.iter().rposition(|e| {
                                matches!(e.event, Event::Replied { agent: a, .. } if a == agent)
                            }) {
                                self.open_thread(i);
                                return;
                            }
                            Some(Zoom::Focus(agent))
                        }
                        Some(item) => Some(Zoom::Focus(item.agent)),
                        None => None,
                    }
                }
                (Pane::Journal, _) => {
                    if let Some(i) = self.journal_sel {
                        self.open_thread(i);
                    }
                    None
                }
                (Pane::Salon, Sel::Project(p)) if self.card.is_none() => {
                    self.collapsed[p] = !self.collapsed[p];
                    None
                }
                (Pane::Salon, Sel::Agent(id)) => Some(Zoom::Focus(id)),
                (Pane::Salon, Sel::Project(_)) => None,
            },
            // A line of the log that says a file changed opens the change
            // at that file (record 141); the raw stream is `t`'s there.
            Zoom::Focus(id) if self.change_line(id).is_some() => {
                self.open_change_line(id);
                None
            }
            Zoom::Focus(id) | Zoom::Trace(id) => Some(Zoom::Trace(id)),
            Zoom::Diff(_) => None,
        };
        if let Some(z) = zoom {
            self.zoom = z;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::super::tests::*;
    use super::*;

    #[test]
    fn the_journal_selection_steps_stays_and_stops_at_its_ends() {
        let (mut app, [a, ..]) = two_quiet_projects();
        for text in ["one", "two", "three", "four"] {
            app.world.said(a, text);
        }
        let rows = app.journal_rows();
        let last = rows.len() - 1;
        assert!(last >= 3);
        let at = |n: usize| Some(rows[n].0);
        app.select_journal(None);
        assert_eq!(app.journal_sel, at(0), "none yet: the first");
        app.journal_sel = at(2);
        app.select_journal(None);
        assert_eq!(app.journal_sel, at(2), "no step: where it is");
        app.select_journal(Some(true));
        assert_eq!(app.journal_sel, at(3));
        app.select_journal(Some(false));
        assert_eq!(app.journal_sel, at(2));
        app.journal_sel = at(last);
        app.select_journal(Some(true));
        assert_eq!(app.journal_sel, at(last), "the last stays");
        app.journal_sel = at(0);
        app.select_journal(Some(false));
        assert_eq!(app.journal_sel, at(0), "the first stays");
    }

    /// What `#` writes for the composer, the composer closed again.
    fn hash(app: &mut App) -> String {
        app.on_key(key(KeyCode::Char('#')));
        assert!(app.typing);
        app.typing = false;
        app.menu = None;
        std::mem::take(&mut app.input)
    }

    #[test]
    fn a_hash_writes_the_card_at_hand() {
        let mut app = with_cards(1);
        let w = &app.world;
        let (worker, card) = w
            .agents
            .iter()
            .find_map(|a| Some((a.id, a.card?)))
            .expect("a worker on a card");
        let mine = format!("#{}", card + 1);
        // Home's row at hand: the worker's card; one without a card, none.
        app.at_home.at = Some(HomeRow::Agent(worker));
        assert_eq!(hash(&mut app), mine);
        app.world.agents[worker].card = None;
        assert_eq!(hash(&mut app), "#");
        app.world.agents[worker].card = Some(card);
        app.zoom = Zoom::Focus(worker);
        app.selected = Sel::Project(0);
        assert_eq!(hash(&mut app), mine, "the zoomed agent's");
        app.zoom = Zoom::None;
        // The card an inbox item came on, not the agent's now.
        let open = app.world.open_inbox();
        app.at_home.at = Some(HomeRow::Item(open[1]));
        app.world.inbox[open[1]].card = Some(2);
        assert_eq!(hash(&mut app), "#3");
        app.world.inbox[open[1]].card = None;
        assert_eq!(hash(&mut app), "#");
        // A journal line that names a card, one that mentions it, one
        // that does neither.
        app.world.note(Event::Posted { agent: 0, card: 3 });
        app.journal_sel = Some(app.world.journal.len() - 1);
        app.open_window(Window::Journal);
        assert_eq!(hash(&mut app), "#4");
        let said = |text: &str| Event::Sent {
            from: 0,
            to: worker,
            kind: MsgKind::Inform,
            text: text.into(),
            id: 0,
            re: None,
        };
        app.world.note(said("about #, #x, #99 and #2 then #3"));
        app.journal_sel = Some(app.world.journal.len() - 1);
        app.open_window(Window::Journal);
        assert_eq!(hash(&mut app), "#2", "the first card it names");
        app.world.note(Event::Failed { agent: worker });
        app.journal_sel = Some(app.world.journal.len() - 1);
        app.open_window(Window::Journal);
        assert_eq!(hash(&mut app), "#");
        // On a card's screen its own card is no news, whatever the owner
        // has moved on to.
        app.open_card(3);
        assert_eq!(hash(&mut app), "#");
    }

    #[test]
    fn enter_opens_the_card_typed_or_offered_at_once() {
        let mut app = with_cards(1);
        app.thread = Some(Thread {
            entries: vec![0],
            sel: 0,
            scroll: Cell::new(0),
            follow: Cell::new(false),
            page: Cell::new(10),
            copy: None,
            words: None,
            folds: Folds::default(),
            room: Cell::new(usize::MAX),
            about: None,
        });
        app.inbox_sel = 3;
        app.selected = Sel::Project(0);
        app.on_key(key(KeyCode::Char('#')));
        type_in(&mut app, "2");
        app.on_key(key(KeyCode::Enter));
        assert_eq!(app.card, Some(1));
        assert!(!app.typing && app.input.is_empty() && app.menu.is_none());
        assert!(app.thread.is_none());
        // The card's screen is its owner's detail (record 125).
        let owner = app.world.cards[1].owner.unwrap();
        assert_eq!((app.zoom, app.pane), (Zoom::Focus(owner), Pane::Salon));
        assert_eq!(app.inbox_sel, 0);
        let first = ui::card_rows(&app.world, 1)[0].0;
        assert_eq!(app.journal_sel, Some(first));
        let owner = app.world.cards[1].owner.expect("taken");
        assert_eq!(app.selected, Sel::Agent(owner));
        // A word of the title: the newest card that has it.
        let title = app.world.cards[0].title.clone();
        let word = title.split(' ').next().unwrap().to_lowercase();
        let newest = app
            .world
            .cards
            .iter()
            .rev()
            .find(|c| c.title.to_lowercase().contains(&word))
            .unwrap()
            .id;
        app.on_key(key(KeyCode::Char('#')));
        type_in(&mut app, &word);
        app.on_key(key(KeyCode::Enter));
        assert_eq!(app.card, Some(newest));
        // No such card: said, and the screen stays.
        app.on_key(key(KeyCode::Char('#')));
        type_in(&mut app, "x99");
        app.on_key(key(KeyCode::Enter));
        assert_eq!(app.card, Some(newest));
        assert_eq!(app.flash.take().unwrap().0, "no card #x99");
        // Off the card: a name is put in by enter, not sent.
        app.on_key(key(KeyCode::Esc));
        assert_eq!(app.card, None);
        app.on_key(key(KeyCode::Char('@')));
        app.on_key(key(KeyCode::Char('l')));
        app.on_key(key(KeyCode::Enter));
        assert!(app.typing, "{}", app.input);
    }

    #[test]
    fn a_card_nobody_took_selects_who_posted_it() {
        let mut app = with_cards(1);
        let c = app.world.cards.len();
        let mut card = app.world.cards[0].clone();
        card.id = c;
        card.owner = None;
        card.story = vec![Story {
            tick: 0,
            event: StoryEvent::Posted {
                by: 0,
                text: String::new(),
            },
        }];
        app.world.cards.push(card);
        app.selected = Sel::Project(0);
        app.open_card(c);
        assert_eq!(app.selected, Sel::Agent(0));
        assert_eq!(app.zoom, Zoom::Focus(0), "its detail");
        // Nobody at all: its project's leader.
        app.world.cards[c].story.clear();
        app.selected = Sel::Project(0);
        app.open_card(c);
        let lead = app.world.projects[0].leader;
        assert_eq!(
            (app.selected, app.zoom),
            (Sel::Agent(lead), Zoom::Focus(lead))
        );
    }

    #[test]
    fn a_card_s_screen_lists_its_own_waiting_items_and_journal_lines() {
        let mut app = with_cards(1);
        let first = app.world.open_inbox()[0];
        app.world.inbox[first].card = Some(0);
        app.open_card(0);
        assert_eq!(app.inbox_rows(), app.world.card_inbox(0));
        assert_eq!(
            app.inbox_rows().len(),
            2,
            "its change, and the one put on it"
        );
        assert_eq!(app.journal_rows(), ui::card_rows(&app.world, 0));
        app.on_key(key(KeyCode::Esc));
        assert_eq!(app.inbox_rows(), app.world.open_inbox());
        assert_eq!(app.journal_rows(), ui::journal_rows(&app.world));
    }

    #[test]
    fn esc_goes_back_to_the_card_then_to_the_salon() {
        let mut app = with_cards(1);
        let owner = app.world.cards[0].owner.unwrap();
        app.open_card(0);
        // The card's screen is its owner's detail; `enter` opens the raw
        // stream, `esc` comes back to the card, then leaves it.
        assert_eq!(app.zoom, Zoom::Focus(owner));
        app.on_key(key(KeyCode::Enter));
        assert_eq!(app.zoom, Zoom::Trace(owner));
        app.on_key(key(KeyCode::Esc));
        assert_eq!(
            (app.zoom, app.card),
            (Zoom::Focus(owner), Some(0)),
            "the card again"
        );
        app.inbox_sel = 1;
        app.on_key(key(KeyCode::Esc));
        assert_eq!((app.zoom, app.card), (Zoom::None, None));
        assert_eq!(app.inbox_sel, 0);
        app.on_key(key(KeyCode::Esc));
        assert_eq!((app.zoom, app.card), (Zoom::None, None), "nothing to leave");
        // Another view leaves it too.
        app.open_card(0);
        app.command("home");
        assert_eq!((app.card), (None));
    }

    #[test]
    fn leaving_a_card_s_screen_keeps_the_journal_selection_on_a_shown_line() {
        let mut app = with_cards(1);
        // A claim is the card's line, not the whole journal's.
        app.world.note(Event::Claimed { agent: 1, card: 0 });
        app.open_card(0);
        let card_line = app.journal_sel.unwrap();
        assert_eq!(card_line, app.world.journal.len() - 1);
        app.on_key(key(KeyCode::Esc));
        let rows = ui::journal_rows(&app.world);
        let sel = app.journal_sel.expect("a line");
        assert!(rows.iter().any(|&(r, _)| r == sel), "{card_line} → {sel}");
    }

    #[test]
    fn n_and_p_step_through_the_open_projects_cards() {
        let mut app = with_cards(2);
        let of = |app: &App, p: usize| -> Vec<CardId> {
            app.world
                .cards
                .iter()
                .filter(|c| c.project == p)
                .map(|c| c.id)
                .collect()
        };
        assert!(of(&app, 1).len() >= 2);
        let all: Vec<CardId> = (0..app.world.cards.len()).collect();
        app.open_card(0);
        app.on_key(key(KeyCode::Char('p')));
        assert_eq!(app.card, Some(0), "no card before the first");
        for &c in &all[1..] {
            app.on_key(key(KeyCode::Char('n')));
            assert_eq!(app.card, Some(c));
        }
        app.on_key(key(KeyCode::Char('n')));
        assert_eq!(app.card, all.last().copied(), "none after the last");
        app.on_key(key(KeyCode::Char('p')));
        assert_eq!(app.card, Some(all[all.len() - 2]));
        // A closed project's cards are skipped.
        app.world.projects[1].closed = true;
        let open = of(&app, 0);
        app.open_card(open[0]);
        for &c in &open[1..] {
            app.on_key(key(KeyCode::Char('n')));
            assert_eq!(app.card, Some(c));
        }
        for &c in open[..open.len() - 1].iter().rev() {
            app.on_key(key(KeyCode::Char('p')));
            assert_eq!(app.card, Some(c));
        }
        // Off the card's screen (its diff, its raw stream): n and p do
        // nothing.
        for zoom in [Zoom::Diff(0), Zoom::Trace(0)] {
            app.zoom = zoom;
            app.on_key(key(KeyCode::Char('n')));
            assert_eq!(app.card, Some(open[0]), "{zoom:?}");
        }
    }

    #[test]
    fn a_card_s_screen_does_not_fold_the_salon() {
        let mut app = with_cards(1);
        app.open_card(0);
        app.pane = Pane::Salon;
        app.selected = Sel::Project(0);
        for code in [KeyCode::Char('h'), KeyCode::Enter, KeyCode::Char('z')] {
            app.on_key(key(code));
        }
        app.on_key(key(KeyCode::Char('M')));
        assert_eq!(app.collapsed, [false]);
        let owner = app.world.cards[0].owner.unwrap();
        assert_eq!(
            (app.zoom, app.card),
            (Zoom::Trace(owner), Some(0)),
            "enter: its raw stream"
        );
        // Back on Home: one project heads no rows, so `h` folds nothing.
        app.on_key(key(KeyCode::Esc));
        app.on_key(key(KeyCode::Esc));
        assert_eq!((app.zoom, app.card), (Zoom::None, None));
        app.on_key(key(KeyCode::Char('h')));
        assert_eq!(app.collapsed, [false]);
    }

    /// `quiet()` with agent 3 a helper of worker 1 and agent 4 researching
    /// for it: worker 1's row stands for three.
    fn with_a_row_of_three() -> App {
        let mut app = quiet();
        let a = &mut app.world.agents;
        a[3].role = Role::Helper;
        a[3].parent = Some(1);
        a[4].role = Role::Researcher;
        a[4].status = Status::Researching {
            for_agent: 1,
            until: 9,
        };
        app
    }

    #[test]
    fn a_row_stands_for_its_agent_its_helpers_and_its_researcher() {
        let mut app = with_a_row_of_three();
        assert_eq!(app.row_group(1), [1, 3, 4]);
        assert_eq!(app.row_group(2), [2], "a worker of its own");
        assert_eq!(app.row_group(3), [3], "a helper has none under it");
        // A worker is no helper whatever its parent; a helper gone is
        // not there; a researcher for another is another's.
        app.world.agents[2].parent = Some(1);
        app.world.agents[3].gone = true;
        assert_eq!(app.row_group(1), [1, 4]);
        app.world.agents[4].status = Status::Researching {
            for_agent: 2,
            until: 9,
        };
        assert_eq!(app.row_group(1), [1]);
        assert_eq!(app.project_group(0), [0, 1, 2, 3, 4]);
        assert_eq!(app.project_group(1), Vec::<AgentId>::new());
    }

    #[test]
    fn what_waits_for_you_or_is_stuck_is_not_settled() {
        let mut app = with_a_row_of_three();
        app.at_home.at = Some(HomeRow::Agent(1));
        // Its helper waits: the row stays.
        app.world.inbox.push(item(3, true));
        app.on_key(key(KeyCode::Char('s')));
        assert!(app.settled.is_empty());
        let helper = app.world.agents[3].name.clone();
        assert_eq!(
            flashed(&app).unwrap_or_default(),
            format!("not hidden: {helper} needs you")
        );
        app.world.inbox.clear();
        app.world.agents[1].status = Status::Blocked;
        app.at_home.at = Some(HomeRow::Agent(1));
        app.on_key(key(KeyCode::Char('s')));
        assert!(app.settled.is_empty());
        let worker = app.world.agents[1].name.clone();
        assert_eq!(
            flashed(&app).unwrap_or_default(),
            format!("not hidden: {worker} needs you")
        );
        // A project with one agent that needs the person stays too
        // (`s` on its heading, two projects heading the rows).
        let (mut app, [a, _, _]) = two_quiet_projects();
        app.world.agents[a].status = Status::WaitingHuman;
        let p = app.world.agents[a].project;
        app.at_home.at = Some(HomeRow::Project(p));
        assert!(app.home_headed());
        app.on_key(key(KeyCode::Char('s')));
        assert!(app.settled_projects.is_empty());
        let other = app.world.agents[a].name.clone();
        assert_eq!(
            flashed(&app).unwrap_or_default(),
            format!("not hidden: {other} needs you")
        );
        assert_eq!(app.at_home.at, Some(HomeRow::Project(p)));
    }
    #[test]
    fn what_was_settled_comes_back_when_it_needs_you_and_goes_when_gone() {
        let mut app = with_a_row_of_three();
        app.settled.extend([1, 2]);
        app.settled_projects.insert(0);
        // Nothing needs the person: all stay settled.
        app.wake_settled();
        assert_eq!(app.settled, BTreeSet::from([1, 2]));
        assert_eq!(app.settled_projects, BTreeSet::from([0]));
        // Worker 1's helper waits for the person: its row comes back, and
        // the project it is in; worker 2 stays.
        app.world.inbox.push(item(3, true));
        app.wake_settled();
        assert_eq!(app.settled, BTreeSet::from([2]));
        assert!(app.settled_projects.is_empty());
        // A worker gone and a project closed are let go though calm.
        app.world.inbox.clear();
        app.settled_projects.insert(0);
        app.world.agents[2].gone = true;
        app.wake_settled();
        assert!(app.settled.is_empty());
        assert_eq!(app.settled_projects, BTreeSet::from([0]));
        app.world.projects[0].closed = true;
        app.wake_settled();
        assert!(app.settled_projects.is_empty());
    }

    #[test]
    fn unsettle_and_goto_bring_back_what_was_settled() {
        let mut app = with_a_row_of_three();
        app.settled.extend([1, 2]);
        app.settled_projects.insert(0);
        assert_eq!(app.command_note("unsettle"), "(3)");
        app.pane = Pane::Inbox;
        app.command("unsettle");
        assert!(app.settled.is_empty() && app.settled_projects.is_empty());
        assert_eq!(flashed(&app).unwrap_or_default(), "3 back in the team");
        app.settled.insert(1);
        app.zoom = Zoom::Focus(1);
        app.command("unsettle");
        assert!(app.settled.is_empty());
        assert_eq!(app.zoom, Zoom::Focus(1), "it stays where it is");
        // To a helper: the worker's row it hangs under comes back, and
        // its project; another row stays settled.
        app.settled.extend([1, 2]);
        app.settled_projects.insert(0);
        let helper = app.world.agents[3].name.clone();
        app.command(&format!("goto {helper}"));
        assert_eq!(app.selected, Sel::Agent(3));
        assert_eq!(app.settled, BTreeSet::from([2]));
        assert!(app.settled_projects.is_empty());
        // To a project: on Home its first row (one project: no heading).
        app.settled_projects.insert(0);
        let project = app.world.projects[0].name.clone();
        app.command(&format!("goto {project}"));
        assert!(app.settled_projects.is_empty());
        assert!(
            matches!(app.at_home.at, Some(HomeRow::Agent(id)) if app.world.agents[id].project == 0),
            "{:?}",
            app.at_home.at
        );
    }

    #[test]
    fn the_colon_menu_names_what_settle_and_peek_act_on() {
        let mut app = quiet();
        app.selected = Sel::Agent(2);
        let name = app.world.agents[2].name.clone();
        assert_eq!(app.command_note("settle"), format!("({name})"));
        assert_eq!(app.command_note("peek"), format!("({name})"));
        assert_eq!(app.command_note("why"), format!("({name})"));
        app.selected = Sel::Project(0);
        let project = app.world.projects[0].name.clone();
        assert_eq!(app.command_note("settle"), format!("({project})"));
        assert_eq!(app.command_note("unsettle"), "(0)");
    }
}
