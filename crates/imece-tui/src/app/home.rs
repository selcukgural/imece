//! Home: what is the person's turn, across projects, beside what is at
//! work (record 124). A list and a preview of what is at hand, so reading
//! takes no key. It keeps no selection of its own for acting: `home_settle`
//! puts what is at hand where every other key looks (the pane, the waiting
//! panel's selection, the salon's), by the row's identity, before a key acts.

use super::*;

/// A row of Home: an item waiting for the person, by its index in
/// `World::inbox`, or an agent.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum HomeRow {
    Item(usize),
    Agent(AgentId),
}

/// The groups of Home's list, in their order (record 125): what is to
/// decide by what deciding costs (a permission is one key, a question is
/// read and answered, a change is a diff read; what the person put off
/// with `s` last), then who is at work.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum HomeGroup {
    Permission,
    Question,
    Change,
    Later,
    Stuck,
    Work,
    Quiet,
}

/// The two headings of Home's list: TO DECIDE and AT WORK.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Heading {
    Decide,
    Work,
}

impl HomeGroup {
    /// The group an item waiting for the person is listed in.
    pub fn of(kind: InboxKind) -> HomeGroup {
        match kind {
            InboxKind::Approval | InboxKind::Script | InboxKind::Check => HomeGroup::Permission,
            InboxKind::Question
            | InboxKind::Research
            | InboxKind::Reply
            | InboxKind::Fault
            | InboxKind::Plan => HomeGroup::Question,
            InboxKind::Change | InboxKind::Sensitive => HomeGroup::Change,
        }
    }

    /// The heading the group stands under.
    pub fn heading(self) -> Heading {
        match self {
            HomeGroup::Permission | HomeGroup::Question | HomeGroup::Change | HomeGroup::Later => {
                Heading::Decide
            }
            HomeGroup::Stuck | HomeGroup::Work | HomeGroup::Quiet => Heading::Work,
        }
    }
}

/// What Home holds on to: the row at hand, by what it is, and the place
/// it stood in, for when that row is gone; the project it is narrowed to.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Home {
    pub at: Option<HomeRow>,
    pub row: usize,
    pub scope: Option<ProjectId>,
}

impl App {
    /// The group item `i` is listed in: what `s` put off last, else its
    /// kind's.
    pub fn home_group(&self, i: usize) -> HomeGroup {
        if self.deferred.contains(&i) {
            HomeGroup::Later
        } else {
            HomeGroup::of(self.world.inbox[i].kind)
        }
    }

    /// Home's rows for `scope`, group by group: what waits in `W`'s
    /// order (permissions, questions, changes; among the changes the
    /// easy ones first; what was put off last), then what is stuck, at
    /// work and quiet. An agent with an ask or a question open is that
    /// item; one with a change waiting that went on to other work is
    /// both, for they are two pieces of work.
    pub fn home_rows_in(&self, scope: Option<ProjectId>) -> Vec<(HomeGroup, HomeRow)> {
        let w = &self.world;
        let ours = |a: AgentId| scope.is_none_or(|p| w.agents[a].project == p);
        let mut items = w.shown_inbox();
        items.retain(|&i| ours(w.inbox[i].agent));
        items.sort_by_key(|&i| self.waiting_rank(i));
        let mut rows: Vec<_> = items
            .iter()
            .map(|&i| (self.home_group(i), HomeRow::Item(i)))
            .collect();
        let rungs = self.rungs();
        let waits_on_item = |id: AgentId| {
            let open = |kind| w.open_item(id, kind).is_some();
            open(InboxKind::Approval) || open(InboxKind::Question)
        };
        let mut agents: Vec<_> = (w.agents.iter())
            .filter(|a| ours(a.id) && !a.gone && !w.put_away(a.id))
            .filter(|a| !w.projects[a.project].closed)
            .filter_map(|a| {
                let at_work = a.status.busy() || matches!(a.status, Status::Asking { .. });
                let group = if rungs[a.id] == Rung::Stuck {
                    HomeGroup::Stuck
                } else if waits_on_item(a.id) {
                    return None;
                } else if at_work {
                    HomeGroup::Work
                } else if a.role == Role::Leader || items.iter().any(|&i| w.inbox[i].agent == a.id)
                {
                    return None;
                } else {
                    HomeGroup::Quiet
                };
                Some((group, HomeRow::Agent(a.id)))
            })
            .collect();
        agents.sort_by_key(|&(group, _)| group);
        rows.extend(agents);
        rows
    }

    /// Home's rows as it stands.
    pub fn home_rows(&self) -> Vec<(HomeGroup, HomeRow)> {
        self.home_rows_in(self.at_home.scope)
    }

    /// The row at hand: the one the person chose while it is listed, else
    /// the one now in its place.
    pub fn home_at(&self) -> Option<HomeRow> {
        let rows = self.home_rows();
        let chosen = self.at_home.at.filter(|&at| rows.iter().any(|r| r.1 == at));
        let in_place = || rows.get(self.at_home.row.min(rows.len().saturating_sub(1)));
        chosen.or_else(|| in_place().map(|r| r.1))
    }

    /// imece opens on Home (the person, 2026-11-07), `main` says so: the
    /// tests of every other view start on the salon they were written for.
    pub fn open_home(&mut self) {
        self.show(View::Home);
    }

    /// Keys go to Home's list: nothing is open over it.
    pub(crate) fn home_keys(&self) -> bool {
        self.view == View::Home
            && self.zoom == Zoom::None
            && self.thread.is_none()
            && self.card.is_none()
    }

    /// What is at hand on Home, put where every key looks for it. True
    /// when the row the person chose is gone: what is at hand now is not
    /// what they had read.
    pub(crate) fn home_settle(&mut self) -> bool {
        let rows = self.home_rows();
        let lost = self
            .at_home
            .at
            .is_some_and(|at| !rows.iter().any(|r| r.1 == at));
        let at = self.home_at();
        self.at_home.at = at;
        if let Some(row) = rows.iter().position(|r| Some(r.1) == at) {
            self.at_home.row = row;
        }
        self.waiting_at = None;
        match at {
            Some(HomeRow::Item(i)) => {
                self.pane = Pane::Inbox;
                let shown = self.inbox_rows();
                self.inbox_sel = shown.iter().position(|&o| o == i).unwrap_or(0);
                self.selected = Sel::Agent(self.world.inbox[i].agent);
            }
            Some(HomeRow::Agent(id)) => {
                self.pane = Pane::Salon;
                self.selected = Sel::Agent(id);
            }
            None => {
                self.pane = Pane::Salon;
                if let Some(p) = self.at_home.scope {
                    self.selected = Sel::Project(p);
                }
            }
        }
        lost
    }

    /// Moves Home's selection to row `row` of the list, or its last.
    pub(crate) fn home_go(&mut self, row: usize) {
        let rows = self.home_rows();
        self.at_home.row = row.min(rows.len().saturating_sub(1));
        self.at_home.at = rows.get(self.at_home.row).map(|r| r.1);
        self.home_settle();
    }

    /// `gd` `gw`: the first row under a heading, TO DECIDE or AT WORK;
    /// nothing stands under it, the list does not move.
    pub(super) fn home_go_heading(&mut self, heading: Heading) {
        let rows = self.home_rows();
        if let Some(row) = rows.iter().position(|r| r.0.heading() == heading) {
            self.home_go(row);
        }
    }

    /// `[` `]`: all the projects, then each open one, round.
    fn home_scope(&mut self, forward: bool) {
        let mut scopes = vec![None];
        scopes.extend(self.world.open_projects().map(|p| Some(p.id)));
        let at = scopes.iter().position(|&s| s == self.at_home.scope);
        let n = scopes.len();
        let next = match at {
            Some(i) if forward => (i + 1) % n,
            Some(i) => (i + n - 1) % n,
            None => 0,
        };
        self.at_home.scope = scopes[next];
        self.at_home.at = None;
        self.home_go(0);
    }

    /// `s`: the item at hand after everything else waiting, staying on
    /// the list; the row in its place comes to hand.
    fn home_later(&mut self) {
        let Some(HomeRow::Item(i)) = self.at_home.at else {
            return;
        };
        self.deferred.retain(|&d| d != i);
        self.deferred.push(i);
        self.at_home.at = None;
        self.home_go(self.at_home.row);
    }

    /// A key on Home's list; false leaves it to the keys every view has,
    /// which act on what `home_settle` put at hand.
    pub(super) fn home_key(&mut self, k: KeyEvent) -> bool {
        let lost = self.home_settle();
        match k.code {
            KeyCode::Down | KeyCode::Char('j') => self.home_go(self.at_home.row + 1),
            KeyCode::Up | KeyCode::Char('k') => self.home_go(self.at_home.row.saturating_sub(1)),
            KeyCode::Char('G') => self.home_go(usize::MAX),
            KeyCode::Char(']') => self.home_scope(true),
            KeyCode::Char('[') => self.home_scope(false),
            KeyCode::Char('s') => self.home_later(),
            // A decision is on what was read: the row it was read on went,
            // and the one in its place is new.
            KeyCode::Char('a' | 'r' | 'e' | 'A') if lost => {
                self.flash(self.lang.text().home_moved);
            }
            _ => return false,
        }
        true
    }

    /// After a decision key acted on Home's row: what is at hand now is
    /// the row in its place, by what it is, so the next key decides it
    /// (a yes moves down, record 125). A row that went by itself before
    /// the key is still told, by `home_key`.
    pub(super) fn home_decided(&mut self) {
        self.home_settle();
    }
}

#[cfg(test)]
mod tests {
    use super::super::tests::*;
    use super::*;

    fn item(kind: InboxKind, agent: AgentId, open: bool) -> InboxItem {
        InboxItem {
            kind,
            agent,
            detail: "the words".into(),
            researcher: None,
            open,
            tick: 0,
            card: None,
        }
    }

    /// A demo of two projects of two workers each, all quiet, on Home.
    fn quiet() -> App {
        let (world, sim) = crate::sim::world(2, 4, 0, 1, Lang::En);
        let mut app = App::with(world, Engine::Sim(sim), Lang::En);
        app.world.inbox.clear();
        for a in &mut app.world.agents {
            a.status = Status::Idle;
            a.starting = false;
        }
        app.view = View::Home;
        app
    }

    fn workers(app: &App) -> Vec<AgentId> {
        (app.world.agents.iter())
            .filter(|a| a.role == Role::Worker)
            .map(|a| a.id)
            .collect()
    }

    fn groups(app: &App) -> Vec<HomeGroup> {
        app.home_rows().iter().map(|r| r.0).collect()
    }

    #[test]
    fn the_list_goes_your_turn_stuck_at_work_quiet() {
        let mut app = quiet();
        let w = workers(&app);
        app.world.agents[w[3]].status = Status::Working;
        app.world.agents[w[2]].status = Status::Blocked;
        app.world.inbox.push(item(InboxKind::Change, w[1], true));
        let rows = app.home_rows();
        assert_eq!(rows[0], (HomeGroup::Change, HomeRow::Item(0)));
        assert_eq!(rows[1], (HomeGroup::Stuck, HomeRow::Agent(w[2])));
        assert_eq!(rows[2], (HomeGroup::Work, HomeRow::Agent(w[3])));
        assert_eq!(rows[3].0, HomeGroup::Quiet);
        assert!(!rows.contains(&(HomeGroup::Quiet, HomeRow::Agent(w[1]))));
        // The leaders stand on the list only while they work.
        let lead = app.world.projects[0].leader;
        assert!(!rows.iter().any(|r| r.1 == HomeRow::Agent(lead)));
        app.world.agents[lead].status = Status::Working;
        assert!(
            app.home_rows()
                .contains(&(HomeGroup::Work, HomeRow::Agent(lead)))
        );
    }

    #[test]
    fn an_agent_waiting_on_an_ask_is_that_item_and_one_with_a_change_goes_on_at_work() {
        let mut app = quiet();
        let w = workers(&app);
        for &a in &w[..3] {
            app.world.agents[a].status = Status::Working;
        }
        app.world.inbox.push(item(InboxKind::Approval, w[0], true));
        app.world.inbox.push(item(InboxKind::Question, w[1], true));
        app.world.inbox.push(item(InboxKind::Change, w[2], true));
        app.world.inbox.push(item(InboxKind::Approval, w[3], false));
        let rows = app.home_rows();
        assert!(!rows.iter().any(|r| r.1 == HomeRow::Agent(w[0])));
        assert!(!rows.iter().any(|r| r.1 == HomeRow::Agent(w[1])));
        assert!(rows.contains(&(HomeGroup::Work, HomeRow::Agent(w[2]))));
        assert!(!rows.iter().any(|r| r.1 == HomeRow::Item(3)), "closed");
    }

    #[test]
    fn asking_a_teammate_is_at_work_and_the_gone_and_the_closed_are_not_listed() {
        let mut app = quiet();
        let w = workers(&app);
        app.world.agents[w[0]].status = Status::Asking { to: w[1], until: 0 };
        assert!(
            app.home_rows()
                .contains(&(HomeGroup::Work, HomeRow::Agent(w[0])))
        );
        app.world.agents[w[1]].gone = true;
        assert!(!app.home_rows().iter().any(|r| r.1 == HomeRow::Agent(w[1])));
        let p = app.world.agents[w[2]].project;
        app.world.projects[p].closed = true;
        assert!(!app.home_rows().iter().any(|r| r.1 == HomeRow::Agent(w[2])));
    }

    #[test]
    fn a_scope_lists_only_its_project_and_brackets_go_round() {
        let mut app = quiet();
        let w = workers(&app);
        let (a, b) = (
            app.world.agents[w[0]].project,
            app.world.agents[w[3]].project,
        );
        assert_ne!(a, b);
        assert!(app.home_key(key(KeyCode::Char(']'))));
        assert_eq!(app.at_home.scope, Some(0));
        let all = app.home_rows_in(None).len();
        let of_p: Vec<_> = app.home_rows();
        assert!(of_p.len() < all);
        let shown = |r: &(HomeGroup, HomeRow)| match r.1 {
            HomeRow::Agent(id) => app.world.agents[id].project == 0,
            HomeRow::Item(_) => false,
        };
        assert!(of_p.iter().all(shown));
        app.home_key(key(KeyCode::Char(']')));
        assert_eq!(app.at_home.scope, Some(1));
        app.home_key(key(KeyCode::Char(']')));
        assert_eq!(app.at_home.scope, None);
        app.home_key(key(KeyCode::Char('[')));
        assert_eq!(app.at_home.scope, Some(1));
        app.home_key(key(KeyCode::Char('[')));
        assert_eq!(app.at_home.scope, Some(0));
        app.home_key(key(KeyCode::Char('[')));
        assert_eq!(app.at_home.scope, None);
        // Narrowed to a project with nothing listed, it is the project at hand.
        let mut app = quiet();
        for a in &mut app.world.agents {
            a.gone = a.project == 1;
        }
        app.at_home.scope = Some(1);
        app.home_settle();
        assert_eq!(app.selected, Sel::Project(1));
    }

    #[test]
    fn j_k_and_g_move_and_stop_at_the_ends() {
        let mut app = quiet();
        let last = app.home_rows().len() - 1;
        app.home_key(key(KeyCode::Char('k')));
        assert_eq!(app.at_home.row, 0);
        app.home_key(key(KeyCode::Char('j')));
        assert_eq!(app.at_home.row, 1);
        app.home_key(key(KeyCode::Down));
        assert_eq!(app.at_home.row, 2);
        app.home_key(key(KeyCode::Up));
        assert_eq!(app.at_home.row, 1);
        app.home_key(key(KeyCode::Char('G')));
        assert_eq!(app.at_home.row, last);
        assert_eq!(app.at_home.at, Some(app.home_rows()[last].1));
        app.home_key(key(KeyCode::Char('j')));
        assert_eq!(app.at_home.row, last);
    }

    #[test]
    fn what_is_at_hand_is_where_every_key_looks_for_it() {
        let mut app = quiet();
        let w = workers(&app);
        app.world.inbox.push(item(InboxKind::Reply, w[0], false));
        app.world.inbox.push(item(InboxKind::Approval, w[1], true));
        app.home_settle();
        assert_eq!(app.pane, Pane::Inbox);
        assert_eq!(app.inbox_rows()[app.inbox_sel], 1);
        assert_eq!(app.selected, Sel::Agent(w[1]));
        assert_eq!(app.waiting_at, None);
        app.home_go(1);
        assert_eq!(app.pane, Pane::Salon);
        let Some(HomeRow::Agent(id)) = app.at_home.at else {
            panic!("an agent")
        };
        assert_eq!(app.selected, Sel::Agent(id));
        assert_eq!(app.agent_at_hand(), Some(id));
    }

    #[test]
    fn a_decision_never_lands_on_a_row_that_slid_into_place() {
        let mut app = quiet();
        let w = workers(&app);
        app.world.inbox.push(item(InboxKind::Approval, w[0], true));
        app.world.inbox.push(item(InboxKind::Approval, w[1], true));
        app.home_settle();
        assert_eq!(app.at_home.at, Some(HomeRow::Item(0)));
        // The first closes behind the person's back.
        app.world.inbox[0].open = false;
        for c in ['a', 'r', 'e', 'A'] {
            let mut other = quiet();
            other.world.inbox = app.world.inbox.clone();
            other.at_home.at = Some(HomeRow::Item(0));
            assert!(other.home_key(key(KeyCode::Char(c))), "{c}");
            assert!(other.world.inbox[1].open, "{c}");
            assert_eq!(other.at_home.at, Some(HomeRow::Item(1)), "{c}");
        }
        // Seen once, the next key decides it.
        app.home_key(key(KeyCode::Char('a')));
        assert!(!app.home_key(key(KeyCode::Char('a'))));
    }

    #[test]
    fn a_row_stays_at_hand_by_what_it_is_when_the_list_moves() {
        let mut app = quiet();
        let w = workers(&app);
        app.world.inbox.push(item(InboxKind::Approval, w[0], true));
        app.world.inbox.push(item(InboxKind::Approval, w[1], true));
        app.home_go(1);
        assert_eq!(app.at_home.at, Some(HomeRow::Item(1)));
        app.deferred.push(1);
        // It went to its turn's end, and stays the row at hand.
        assert_eq!(app.home_at(), Some(HomeRow::Item(1)));
        app.deferred.clear();
        app.world.inbox[1].open = false;
        let place = app.home_rows()[1].1;
        assert_eq!(app.home_at(), Some(place), "the row in its place");
    }

    #[test]
    fn the_row_at_hand_follows_when_the_rows_above_it_go() {
        let mut app = quiet();
        let w = workers(&app);
        app.world.inbox.push(item(InboxKind::Approval, w[0], true));
        app.world.inbox.push(item(InboxKind::Approval, w[1], true));
        app.home_go(1);
        assert_eq!(app.at_home.row, 1);
        app.world.inbox[0].open = false;
        app.home_settle();
        assert_eq!(app.at_home.at, Some(HomeRow::Item(1)));
        assert_eq!(app.at_home.row, 0);
    }

    #[test]
    fn s_puts_an_item_after_the_rest_and_nothing_else() {
        let mut app = quiet();
        let w = workers(&app);
        app.world.inbox.push(item(InboxKind::Approval, w[0], true));
        app.world.inbox.push(item(InboxKind::Approval, w[1], true));
        app.home_settle();
        app.home_key(key(KeyCode::Char('s')));
        assert_eq!(app.deferred, [0]);
        assert_eq!(app.at_home.at, Some(HomeRow::Item(1)));
        assert_eq!(app.home_rows()[1].1, HomeRow::Item(0));
        // Again on the same: once in the list.
        app.home_key(key(KeyCode::Char('j')));
        assert_eq!(app.at_home.at, Some(HomeRow::Item(0)));
        app.home_key(key(KeyCode::Char('s')));
        assert_eq!(app.deferred, [0]);
        // An agent has no later.
        app.home_go(2);
        app.home_key(key(KeyCode::Char('s')));
        assert_eq!(app.deferred, [0]);
    }

    #[test]
    fn enter_and_a_work_through_the_keys_every_view_has() {
        let mut app = quiet();
        let w = workers(&app);
        app.world.inbox.push(item(InboxKind::Approval, w[0], true));
        app.key(key(KeyCode::Char('a')));
        assert!(!app.world.inbox[0].open, "allowed");
        assert_eq!(app.view, View::Home);
        app.at_home.at = None;
        app.key(key(KeyCode::Enter));
        let Zoom::Focus(id) = app.zoom else {
            panic!("an agent opens: {:?}", app.zoom)
        };
        assert!(!app.home_keys());
        assert_eq!(
            id,
            match app.at_home.at {
                Some(HomeRow::Agent(id)) => id,
                _ => panic!("an agent at hand"),
            }
        );
        app.key(key(KeyCode::Esc));
        assert_eq!((app.view, app.zoom), (View::Home, Zoom::None));
    }

    #[test]
    fn w_opens_the_next_item_and_esc_comes_back_home() {
        let mut app = quiet();
        let w = workers(&app);
        app.world.inbox.push(item(InboxKind::Sensitive, w[0], true));
        app.key(key(KeyCode::Char('W')));
        assert_eq!(app.view, View::Home);
        assert_eq!(app.at_home.at, Some(HomeRow::Item(0)));
        // From another view it stays the salon.
        let mut app = quiet();
        app.view = View::Board;
        app.world
            .inbox
            .push(item(InboxKind::Sensitive, workers(&app)[0], true));
        app.key(key(KeyCode::Char('W')));
        assert_eq!(app.view, View::Salon);
    }

    #[test]
    fn imece_opens_on_home_and_the_first_key_acts_on_the_row_shown() {
        let mut app = quiet();
        app.view = View::Salon;
        app.open_home();
        assert_eq!(app.view, View::Home);
        let w = workers(&app);
        app.world.inbox.push(item(InboxKind::Approval, w[0], true));
        assert_eq!(app.home_at(), Some(HomeRow::Item(0)));
        app.key(key(KeyCode::Char('a')));
        assert!(!app.world.inbox[0].open);
    }

    #[test]
    fn five_and_colon_home_open_it() {
        let mut app = quiet();
        app.view = View::Salon;
        app.key(key(KeyCode::Char('5')));
        assert_eq!(app.view, View::Home);
        app.view = View::Salon;
        app.command("home");
        assert_eq!(app.view, View::Home);
        assert_eq!(groups(&app)[0], HomeGroup::Quiet);
        assert_eq!(app.key_scope(), Some(keys::Scope::Salon));
    }
}

#[cfg(test)]
mod triage_tests {
    use super::super::tests::*;
    use super::*;

    fn item(kind: InboxKind, agent: AgentId) -> InboxItem {
        InboxItem {
            kind,
            agent,
            detail: "the words".into(),
            researcher: None,
            open: true,
            tick: 0,
            card: None,
        }
    }

    /// Two projects of two workers each, all quiet, on Home.
    fn quiet() -> App {
        let (world, sim) = crate::sim::world(2, 4, 0, 1, Lang::En);
        let mut app = App::with(world, Engine::Sim(sim), Lang::En);
        app.world.inbox.clear();
        for a in &mut app.world.agents {
            a.status = Status::Idle;
            a.starting = false;
        }
        app.view = View::Home;
        app
    }

    fn workers(app: &App) -> Vec<AgentId> {
        (app.world.agents.iter())
            .filter(|a| a.role == Role::Worker)
            .map(|a| a.id)
            .collect()
    }

    #[test]
    fn every_kind_of_item_has_its_group_and_every_group_its_heading() {
        for (kind, group) in [
            (InboxKind::Approval, HomeGroup::Permission),
            (InboxKind::Script, HomeGroup::Permission),
            (InboxKind::Check, HomeGroup::Permission),
            (InboxKind::Question, HomeGroup::Question),
            (InboxKind::Research, HomeGroup::Question),
            (InboxKind::Reply, HomeGroup::Question),
            (InboxKind::Fault, HomeGroup::Question),
            (InboxKind::Plan, HomeGroup::Question),
            (InboxKind::Change, HomeGroup::Change),
            (InboxKind::Sensitive, HomeGroup::Change),
        ] {
            assert_eq!(HomeGroup::of(kind), group, "{kind:?}");
        }
        for (group, heading) in [
            (HomeGroup::Permission, Heading::Decide),
            (HomeGroup::Question, Heading::Decide),
            (HomeGroup::Change, Heading::Decide),
            (HomeGroup::Later, Heading::Decide),
            (HomeGroup::Stuck, Heading::Work),
            (HomeGroup::Work, Heading::Work),
            (HomeGroup::Quiet, Heading::Work),
        ] {
            assert_eq!(group.heading(), heading, "{group:?}");
        }
        assert!(HomeGroup::Permission < HomeGroup::Question);
        assert!(HomeGroup::Question < HomeGroup::Change);
        assert!(HomeGroup::Change < HomeGroup::Later);
        assert!(HomeGroup::Later < HomeGroup::Stuck);
        assert!(HomeGroup::Stuck < HomeGroup::Work);
        assert!(HomeGroup::Work < HomeGroup::Quiet);
    }

    #[test]
    fn what_is_to_decide_goes_permissions_questions_changes_whatever_their_age() {
        let mut app = quiet();
        let w = workers(&app);
        app.world.inbox.push(item(InboxKind::Change, w[0]));
        app.world.inbox.push(item(InboxKind::Question, w[1]));
        app.world.inbox.push(item(InboxKind::Approval, w[2]));
        app.world.inbox.push(item(InboxKind::Check, w[3]));
        let rows = app.home_rows();
        assert_eq!(
            rows[..4],
            [
                (HomeGroup::Permission, HomeRow::Item(2)),
                (HomeGroup::Permission, HomeRow::Item(3)),
                (HomeGroup::Question, HomeRow::Item(1)),
                (HomeGroup::Change, HomeRow::Item(0)),
            ]
        );
        // One put off goes last of what is to decide, in its own group.
        app.deferred.push(2);
        let rows = app.home_rows();
        assert_eq!(rows[0], (HomeGroup::Permission, HomeRow::Item(3)));
        assert_eq!(rows[1], (HomeGroup::Question, HomeRow::Item(1)));
        assert_eq!(rows[2], (HomeGroup::Change, HomeRow::Item(0)));
        assert_eq!(rows[3], (HomeGroup::Later, HomeRow::Item(2)));
        assert_eq!(app.home_group(2), HomeGroup::Later);
        assert_eq!(app.home_group(3), HomeGroup::Permission);
    }

    #[test]
    fn gd_and_gw_go_to_the_first_row_under_a_heading_and_move_nothing_without_one() {
        let mut app = quiet();
        let w = workers(&app);
        app.world.inbox.push(item(InboxKind::Question, w[0]));
        app.world.inbox.push(item(InboxKind::Question, w[1]));
        app.world.agents[w[2]].status = Status::Working;
        keys(&mut app, "gw");
        assert_eq!(app.at_home.at, Some(HomeRow::Agent(w[2])));
        assert_eq!(app.at_home.row, 2);
        keys(&mut app, "jgd");
        assert_eq!(app.at_home.at, Some(HomeRow::Item(0)));
        assert_eq!(app.at_home.row, 0);
        keys(&mut app, "j");
        assert_eq!(app.at_home.row, 1);
        // Nothing to decide: `gd` stays where it is.
        app.world.inbox.clear();
        keys(&mut app, "gd");
        assert_eq!(app.at_home.row, 1);
        assert_eq!(app.pending, None);
        // Off Home the chord does nothing of Home's.
        app.view = View::Salon;
        app.world.inbox.push(item(InboxKind::Question, w[0]));
        app.at_home.row = 0;
        app.at_home.at = None;
        keys(&mut app, "gw");
        assert_eq!(app.at_home.row, 0);
    }

    #[test]
    fn a_yes_moves_down_and_the_next_key_decides_the_next_row() {
        let mut app = quiet();
        let w = workers(&app);
        for &a in &[w[0], w[1], w[2], w[3], w[0]] {
            let mut it = item(InboxKind::Approval, a);
            it.detail = "Bash: cargo test".into();
            app.world.inbox.push(it);
        }
        keys(&mut app, "a");
        assert!(!app.world.inbox[0].open);
        assert_eq!(app.at_home.at, Some(HomeRow::Item(1)), "moved down");
        assert_eq!(app.at_home.row, 0);
        let moved = app.lang.text().home_moved;
        // Each key that decides: the next key decides the next row.
        for (n, k) in ["r", "e", "A"].into_iter().enumerate() {
            keys(&mut app, k);
            assert!(!app.world.inbox[n + 1].open, "{k} decided the next row");
            assert_eq!(app.at_home.at, Some(HomeRow::Item(n + 2)), "{k}");
            assert!(
                app.flash.as_ref().is_none_or(|f| f.0 != moved),
                "{k}: {:?}",
                app.flash
            );
        }
        // The last one: the row in its place is whatever is next.
        keys(&mut app, "a");
        assert!(!app.world.inbox[4].open);
        assert!(matches!(app.at_home.at, Some(HomeRow::Agent(_))));
        // A row gone by itself is still told once.
        app.world.inbox.push(item(InboxKind::Approval, w[0]));
        app.world.inbox.push(item(InboxKind::Approval, w[1]));
        app.home_settle();
        assert_eq!(app.at_home.at, Some(HomeRow::Item(5)));
        app.world.inbox[5].open = false;
        keys(&mut app, "a");
        assert!(app.world.inbox[6].open, "not decided unseen");
        assert_eq!(app.flash.as_ref().map(|f| f.0.as_str()), Some(moved));
    }

    #[test]
    fn a_decision_off_home_settles_nothing_of_home_s() {
        let mut app = quiet();
        let w = workers(&app);
        app.world.inbox.push(item(InboxKind::Approval, w[0]));
        app.world.inbox.push(item(InboxKind::Approval, w[1]));
        app.view = View::Salon;
        app.pane = Pane::Inbox;
        app.inbox_sel = 0;
        keys(&mut app, "a");
        assert!(!app.world.inbox[0].open);
        assert_eq!(app.at_home.at, None);
    }
}
