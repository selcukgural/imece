//! Home: what is the person's turn, across projects, beside what is at
//! work (record 124). A list and a preview of what is at hand, so reading
//! takes no key. It keeps no selection of its own for acting: `home_settle`
//! puts what is at hand where every other key looks (the pane, the waiting
//! panel's selection, the salon's), by the row's identity, before a key acts.

use super::*;

/// A row of Home: an item waiting for the person, by its index in
/// `World::inbox`, an agent, or a project's heading over its agents
/// (record 125, step 5: the team is Home's lower half).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum HomeRow {
    Item(usize),
    Agent(AgentId),
    Project(ProjectId),
}

/// The groups of Home's list, in their order (record 125): what is to
/// decide by what deciding costs (a permission is one key, a question is
/// read and answered, a change is a diff read; what the person put off
/// with `s` last), then who is at work: a project's heading, then its
/// agents stuck, at work and quiet.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum HomeGroup {
    Permission,
    Question,
    Change,
    Later,
    Team,
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
            | InboxKind::Plan
            | InboxKind::Tries
            | InboxKind::Rounds => HomeGroup::Question,
            InboxKind::Change | InboxKind::Sensitive => HomeGroup::Change,
        }
    }

    /// The heading the group stands under.
    pub fn heading(self) -> Heading {
        match self {
            HomeGroup::Permission | HomeGroup::Question | HomeGroup::Change | HomeGroup::Later => {
                Heading::Decide
            }
            HomeGroup::Team | HomeGroup::Stuck | HomeGroup::Work | HomeGroup::Quiet => {
                Heading::Work
            }
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
    /// easy ones first; what was put off last), then the team, project by
    /// project in the salon's order: a heading when more than one project
    /// is in view (a folded project is its heading alone), its workers
    /// and solos stuck, at work and quiet, each with its helpers and
    /// researcher under it, then the helpers whose worker has no row. An
    /// agent with an ask or a question open is that item; one with a
    /// change waiting that went on to other work is both, for they are
    /// two pieces of work; a leader has no row unless it is stuck; an
    /// agent `s` hid is out until it needs the person.
    pub fn home_rows_in(&self, scope: Option<ProjectId>) -> Vec<(HomeGroup, HomeRow)> {
        let w = &self.world;
        let ours = |a: AgentId| {
            scope.is_none_or(|p| w.agents[a].project == p)
                && !w.put_away(a)
                && !w.projects[w.agents[a].project].closed
        };
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
        // Hidden by `s`: the agent, or the worker it hangs under.
        let hidden = |a: &Agent| {
            let worker = match (a.role, a.status) {
                (Role::Helper, _) => a.parent,
                (Role::Researcher, Status::Researching { for_agent, .. }) => Some(for_agent),
                _ => None,
            };
            self.settled.contains(&a.id) || worker.is_some_and(|id| self.settled.contains(&id))
        };
        let group_of = |a: &Agent| -> Option<HomeGroup> {
            if a.gone || w.put_away(a.id) || hidden(a) {
                return None;
            }
            let at_work = a.status.busy() || matches!(a.status, Status::Asking { .. });
            if rungs[a.id] == Rung::Stuck {
                Some(HomeGroup::Stuck)
            } else if waits_on_item(a.id) {
                None
            } else if at_work {
                Some(HomeGroup::Work)
            } else if a.role == Role::Leader || items.iter().any(|&i| w.inbox[i].agent == a.id) {
                None
            } else {
                Some(HomeGroup::Quiet)
            }
        };
        let in_view = |p: &Project| {
            !p.closed && !self.settled_projects.contains(&p.id) && scope.is_none_or(|s| s == p.id)
        };
        let headed = scope.is_none() && w.projects.iter().filter(|p| in_view(p)).count() > 1;
        for p in w.salon_order() {
            if !in_view(&w.projects[p]) {
                continue;
            }
            let mut under = Vec::new();
            let of_p = |a: &&Agent| a.project == p;
            let mut tops: Vec<(HomeGroup, AgentId)> = (w.agents.iter().filter(of_p))
                .filter(|a| matches!(a.role, Role::Worker | Role::Solo))
                .filter_map(|a| group_of(a).map(|g| (g, a.id)))
                .collect();
            tops.sort_by_key(|&(group, _)| group);
            let mut listed = Vec::new();
            for (group, id) in tops {
                under.push((group, HomeRow::Agent(id)));
                listed.push(id);
                for hung in self.row_group(id).into_iter().skip(1) {
                    if let Some(g) = group_of(&w.agents[hung]) {
                        under.push((g, HomeRow::Agent(hung)));
                        listed.push(hung);
                    }
                }
            }
            let mut loose: Vec<(HomeGroup, AgentId)> = (w.agents.iter().filter(of_p))
                .filter(|a| {
                    !listed.contains(&a.id) && a.role != Role::Worker && a.role != Role::Solo
                })
                .filter_map(|a| group_of(a).map(|g| (g, a.id)))
                .collect();
            loose.sort_by_key(|&(group, _)| group);
            under.extend(loose.into_iter().map(|(g, id)| (g, HomeRow::Agent(id))));
            // A heading stands over rows, or folded over the rows it hides;
            // a project with nobody on the list has none.
            if under.is_empty() {
                continue;
            }
            if headed {
                rows.push((HomeGroup::Team, HomeRow::Project(p)));
                if self.collapsed[p] {
                    continue;
                }
            }
            rows.extend(under);
        }
        rows
    }

    /// Whether Home heads its agents with their projects: more than one
    /// in view, none narrowed to.
    pub(crate) fn home_headed(&self) -> bool {
        self.home_rows()
            .iter()
            .any(|r| matches!(r.1, HomeRow::Project(_)))
    }

    /// The project the row at hand belongs to.
    fn home_project(&self) -> Option<ProjectId> {
        let w = &self.world;
        Some(match self.home_at()? {
            HomeRow::Project(p) => p,
            HomeRow::Agent(id) => w.agents[id].project,
            HomeRow::Item(i) => w.agents[w.inbox[i].agent].project,
        })
    }

    /// `h` `l` (`za`) on Home: the row at hand's project folds to its
    /// heading or unfolds, when the projects head their rows; folding
    /// brings the heading to hand, so the cursor does not vanish with the
    /// rows.
    pub(super) fn home_fold(&mut self, fold: bool) {
        let Some(p) = self.home_project() else {
            return;
        };
        if !self.home_headed() {
            return;
        }
        self.collapsed[p] = fold;
        if fold {
            self.at_home.at = Some(HomeRow::Project(p));
        }
        self.home_settle();
    }

    /// `za` on Home: the row at hand's project, the other way.
    pub(super) fn home_fold_toggle(&mut self) {
        if let Some(p) = self.home_project() {
            self.home_fold(!self.collapsed[p]);
        }
    }

    /// `s` on an agent's row or a project's heading: hidden until it
    /// needs the person (`:settle`); the row in its place comes to hand.
    pub(super) fn home_hide(&mut self) {
        self.settle_at_hand();
        self.at_home.at = None;
        self.home_go(self.at_home.row);
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

    /// Keys go to Home's list: nothing is open over it.
    pub(crate) fn home_keys(&self) -> bool {
        self.zoom == Zoom::None
            && self.window.is_none()
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
            Some(HomeRow::Project(p)) => {
                self.pane = Pane::Salon;
                self.selected = Sel::Project(p);
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
        // The person picked: `W` goes on from the item they stand on.
        self.waiting_at = match self.at_home.at {
            Some(HomeRow::Item(i)) => Some(i),
            _ => None,
        };
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
    /// the list; an agent or a project hidden instead (`home_hide`); the
    /// row in its place comes to hand.
    fn home_later(&mut self) {
        let Some(HomeRow::Item(i)) = self.at_home.at else {
            return self.home_hide();
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
            KeyCode::Left | KeyCode::Char('h') => self.home_fold(true),
            KeyCode::Right | KeyCode::Char('l') => self.home_fold(false),
            // A project's heading opens its leader's work.
            KeyCode::Enter if matches!(self.at_home.at, Some(HomeRow::Project(_))) => {
                if let Some(HomeRow::Project(p)) = self.at_home.at {
                    self.zoom = Zoom::Focus(self.world.projects[p].leader);
                }
            }
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
    /// The open question at hand, as its inbox index: the row at hand on
    /// Home, or the question of the agent whose detail is open.
    pub(super) fn question_at_hand(&self) -> Option<usize> {
        let w = &self.world;
        if let Some((id, _)) = self.detail() {
            return (w.open_inbox().into_iter())
                .find(|&i| w.inbox[i].kind == InboxKind::Question && w.inbox[i].agent == id);
        }
        match self.home_at()? {
            HomeRow::Item(i) if w.inbox[i].kind == InboxKind::Question => Some(i),
            HomeRow::Item(_) | HomeRow::Agent(_) | HomeRow::Project(_) => None,
        }
    }

    /// What digit `d` would answer on Home (record 125): the question at
    /// hand's agent and the option the digit names, counted from 1.
    pub(super) fn option_at_hand(&self, d: char) -> Option<(AgentId, String)> {
        let i = self.question_at_hand()?;
        let agent = self.world.inbox[i].agent;
        let n = usize::try_from(d.to_digit(10)?).ok()?.checked_sub(1)?;
        let option = self.world.question_of(agent)?.options.get(n)?.clone();
        Some((agent, option))
    }

    /// A digit on Home answers the question at hand with the option it
    /// names: the words go to the agent as the person's, which closes the
    /// question and starts its next turn; the row settles as any decision.
    pub(super) fn answer_option(&mut self, d: char) {
        let Some((agent, option)) = self.option_at_hand(d) else {
            return;
        };
        if self.tell(agent, &option) {
            self.home_decided();
        }
    }

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
        // Two projects in view: each heads its own, stuck, at work, quiet;
        // w[1] is its change's row, not an agent's.
        let projects: Vec<ProjectId> = app.world.agents.iter().map(|a| a.project).collect();
        let project = |id: AgentId| projects[id];
        let mut want = vec![(HomeGroup::Change, HomeRow::Item(0))];
        for p in app.world.salon_order() {
            let mine: Vec<_> = [
                (HomeGroup::Stuck, w[2]),
                (HomeGroup::Work, w[3]),
                (HomeGroup::Quiet, w[0]),
            ]
            .into_iter()
            .filter(|&(_, id)| project(id) == p)
            .map(|(g, id)| (g, HomeRow::Agent(id)))
            .collect();
            if !mine.is_empty() {
                want.push((HomeGroup::Team, HomeRow::Project(p)));
                want.extend(mine);
            }
        }
        assert_eq!(rows, want);
        assert_eq!(rows.iter().filter(|r| r.0 == HomeGroup::Team).count(), 2);
        assert!(!rows.contains(&(HomeGroup::Quiet, HomeRow::Agent(w[1]))));
        // The leaders stand on the list only while they work, after
        // their project's workers, before the next project's heading.
        let lead = app.world.projects[0].leader;
        assert!(!rows.iter().any(|r| r.1 == HomeRow::Agent(lead)));
        app.world.agents[lead].status = Status::Working;
        let rows = app.home_rows();
        let at = |row: HomeRow| rows.iter().position(|r| r.1 == row).unwrap();
        assert_eq!(rows[at(HomeRow::Agent(lead))].0, HomeGroup::Work);
        let heads: Vec<usize> = (rows.iter().enumerate())
            .filter(|(_, r)| r.0 == HomeGroup::Team)
            .map(|(i, _)| i)
            .collect();
        let mine = at(HomeRow::Project(0));
        let next = heads
            .iter()
            .find(|&&h| h > mine)
            .copied()
            .unwrap_or(rows.len());
        assert!(mine < at(HomeRow::Agent(lead)) && at(HomeRow::Agent(lead)) < next);
        assert_eq!(at(HomeRow::Agent(lead)), next - 1, "last of its project");
        // Narrowed to one project, or with one open: no heading.
        app.at_home.scope = Some(project(w[2]));
        let rows = app.home_rows();
        assert_eq!(rows[0], (HomeGroup::Stuck, HomeRow::Agent(w[2])));
        assert!(!rows.iter().any(|r| matches!(r.1, HomeRow::Project(_))));
        app.at_home.scope = None;
        app.world.projects[1].closed = true;
        assert!(
            !app.home_rows()
                .iter()
                .any(|r| matches!(r.1, HomeRow::Project(_)))
        );
        // A project with nobody on the list has no heading.
        app.world.projects[1].closed = false;
        for a in &mut app.world.agents {
            if a.project == 0 {
                a.gone = true;
            }
        }
        assert!(!app.home_rows().iter().any(|r| r.1 == HomeRow::Project(0)));
        assert!(app.home_rows().iter().any(|r| r.1 == HomeRow::Project(1)));
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
            HomeRow::Item(_) | HomeRow::Project(_) => false,
        };
        assert!(of_p.iter().all(shown), "narrowed: no heading either");
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
        assert_eq!(app.at_home.at, Some(HomeRow::Project(0)), "the heading");
        assert_eq!(app.selected, Sel::Project(0));
        app.home_go(2);
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
        app.at_home.at = None;
        // The first row is the project's heading: its leader's work opens.
        app.key(key(KeyCode::Enter));
        assert_eq!(app.zoom, Zoom::Focus(app.world.projects[0].leader));
        app.key(key(KeyCode::Esc));
        app.home_go(1);
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
        assert_eq!((app.zoom), (Zoom::None));
    }

    #[test]
    fn w_opens_the_next_item_and_esc_comes_back_home() {
        let mut app = quiet();
        let w = workers(&app);
        app.world.inbox.push(item(InboxKind::Sensitive, w[0], true));
        app.key(key(KeyCode::Char('W')));
        assert_eq!(app.at_home.at, Some(HomeRow::Item(0)));
        // A window over the screen takes the key: nothing opens.
        let mut app = quiet();
        app.world
            .inbox
            .push(item(InboxKind::Sensitive, workers(&app)[0], true));
        app.window = Some(Window::Board);
        app.key(key(KeyCode::Char('W')));
        assert_eq!((app.window), (Some(Window::Board)));
        assert_eq!(app.at_home.at, None);
    }

    #[test]
    fn imece_opens_on_home_and_the_first_key_acts_on_the_row_shown() {
        let mut app = quiet();
        let w = workers(&app);
        app.world.inbox.push(item(InboxKind::Approval, w[0], true));
        assert_eq!(app.home_at(), Some(HomeRow::Item(0)));
        app.key(key(KeyCode::Char('a')));
        assert!(!app.world.inbox[0].open);
    }

    #[test]
    fn a_digit_on_home_answers_the_question_at_hand_with_the_option_it_names() {
        let mut app = quiet();
        let w = workers(&app);
        let ask = |app: &mut App, from, options: &[&str]| {
            app.world.note(Event::ToYou {
                from,
                kind: MsgKind::Ask,
                text: "Which?".into(),
                options: options.iter().map(|o| o.to_string()).collect(),
            });
            app.world.asked_you(from, "Which?");
        };
        ask(&mut app, w[0], &["the card", "store credit"]);
        assert_eq!(app.home_at(), Some(HomeRow::Item(0)));
        assert_eq!(
            app.option_at_hand('2'),
            Some((w[0], "store credit".to_string()))
        );
        assert_eq!(
            app.option_at_hand('1').map(|o| o.1),
            Some("the card".into())
        );
        assert_eq!(app.option_at_hand('3'), None, "no third answer");
        assert_eq!(app.option_at_hand('0'), None);
        assert_eq!(app.option_at_hand('a'), None);
        // `3` names no answer and does nothing; `2` answers and Home stays.
        app.key(key(KeyCode::Char('3')));
        assert_eq!((app.window), (None));
        assert!(app.world.inbox[0].open, "not answered");
        app.key(key(KeyCode::Char('2')));
        assert!(!app.world.inbox[0].open, "answered");
        assert!(
            matches!(
                &app.world.journal.last().unwrap().event,
                Event::YouSaid { to, text, .. } if *to == w[0] && text == "store credit"
            ),
            "{:?}",
            app.world.journal.last()
        );
        assert_eq!(app.option_at_hand('1'), None, "nothing at hand");
        // Off Home the digits do nothing, question or not.
        ask(&mut app, w[1], &["a"]);
        assert_eq!(app.option_at_hand('1'), None);
        app.key(key(KeyCode::Char('1')));
        assert_eq!((app.window), (None));
        assert!(app.world.has_question(w[1]));
        // A question without answers to pick: `1` does nothing either.
        ask(&mut app, w[1], &[]);
        assert_eq!(app.option_at_hand('1'), None);
        app.key(key(KeyCode::Char('1')));
        assert_eq!((app.window), (None));
        assert!(app.world.has_question(w[1]));
    }

    #[test]
    fn slash_on_a_question_at_hand_writes_the_answer_to_its_agent() {
        let mut app = quiet();
        let w = workers(&app);
        app.world.asked_you(w[0], "Which?");
        assert_eq!(app.question_at_hand(), Some(0));
        app.key(key(KeyCode::Char('/')));
        assert!(app.typing);
        assert_eq!(app.input, format!("@{} ", app.world.agents[w[0]].name));
        // Not on a question: `/` writes to nobody in particular, as ever.
        let mut app = quiet();
        app.world.inbox.push(item(InboxKind::Approval, w[0], true));
        assert_eq!(app.question_at_hand(), None);
        app.key(key(KeyCode::Char('/')));
        assert!(app.typing);
        assert_eq!(app.input, "");
        app.typing = false;
        app.world.asked_you(w[0], "Which?");
        assert_eq!(app.question_at_hand(), None, "Home only");
    }

    #[test]
    fn colon_home_opens_it_and_five_does_not() {
        let mut app = quiet();
        app.key(key(KeyCode::Char('5')));
        app.command("home");
        assert_eq!(groups(&app)[0], HomeGroup::Team);
        assert_eq!(groups(&app)[1], HomeGroup::Quiet);
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
            (InboxKind::Tries, HomeGroup::Question),
            (InboxKind::Rounds, HomeGroup::Question),
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
        // Under AT WORK the first row is a heading, the working worker's
        // project's (the two with questions are their items); `j` goes on
        // to the worker, first of its project as it works.
        keys(&mut app, "gw");
        let p = app.world.agents[w[2]].project;
        assert_eq!(app.at_home.at, Some(HomeRow::Project(p)));
        assert_eq!(app.at_home.row, 2);
        keys(&mut app, "j");
        assert_eq!(app.at_home.at, Some(HomeRow::Agent(w[2])));
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
        assert!(matches!(app.at_home.at, Some(HomeRow::Project(_))));
        // A row gone by itself is still told once.
        app.world.inbox.push(item(InboxKind::Approval, w[0]));
        app.world.inbox.push(item(InboxKind::Approval, w[1]));
        app.home_go(0);
        assert_eq!(app.at_home.at, Some(HomeRow::Item(5)));
        app.world.inbox[5].open = false;
        keys(&mut app, "a");
        assert!(app.world.inbox[6].open, "not decided unseen");
        assert_eq!(app.flash.as_ref().map(|f| f.0.as_str()), Some(moved));
    }
}

#[cfg(test)]
mod team_tests {
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

    /// Two quiet projects of two workers each on Home, the first worker
    /// of project 0 with a helper and a researcher under it.
    fn team() -> (App, Vec<AgentId>, AgentId, AgentId) {
        let (world, sim) = crate::sim::world(2, 4, 2, 1, Lang::En);
        let mut app = App::with(world, Engine::Sim(sim), Lang::En);
        app.world.inbox.clear();
        for a in &mut app.world.agents {
            a.status = Status::Idle;
            a.starting = false;
        }
        let w: Vec<AgentId> = (app.world.agents.iter())
            .filter(|a| a.role == Role::Worker && a.project == 0)
            .map(|a| a.id)
            .collect();
        let worker = w[0];
        let n = app.world.agents.len();
        let mut helper = app.world.agents[worker].clone();
        helper.id = n;
        helper.name = "claude-9.1".into();
        helper.role = Role::Helper;
        helper.parent = Some(worker);
        helper.status = Status::Working;
        app.world.agents.push(helper);
        let mut researcher = app.world.agents[worker].clone();
        researcher.id = n + 1;
        researcher.name = "scout".into();
        researcher.role = Role::Researcher;
        researcher.parent = None;
        researcher.status = Status::Researching {
            for_agent: worker,
            until: 99,
        };
        app.world.agents.push(researcher);
        (app, w, n, n + 1)
    }

    fn rows(app: &App) -> Vec<HomeRow> {
        app.home_rows().iter().map(|r| r.1).collect()
    }

    fn keys(app: &mut App, text: &str) {
        for c in text.chars() {
            app.key(key(KeyCode::Char(c)));
        }
    }

    #[test]
    fn helpers_and_researchers_hang_under_their_worker_or_stand_alone_after() {
        let (mut app, w, helper, researcher) = team();
        let worker = w[0];
        let got = rows(&app);
        let at = |row: HomeRow| got.iter().position(|&r| r == row).unwrap();
        assert_eq!(got[0], HomeRow::Project(0));
        assert_eq!(at(HomeRow::Agent(helper)), at(HomeRow::Agent(worker)) + 1);
        assert_eq!(
            at(HomeRow::Agent(researcher)),
            at(HomeRow::Agent(worker)) + 2
        );
        assert_eq!(
            app.home_rows()[at(HomeRow::Agent(helper))].0,
            HomeGroup::Work
        );
        // The worker waits on an ask: its item is its row, the helper and
        // the researcher stand on their own after the project's workers.
        app.world.inbox.push(item(InboxKind::Approval, worker));
        let got = rows(&app);
        assert_eq!(got[0], HomeRow::Item(0));
        assert!(!got.contains(&HomeRow::Agent(worker)));
        let heading = got.iter().position(|&r| r == HomeRow::Project(0)).unwrap();
        let next = (got.iter().enumerate())
            .find(|(i, r)| *i > heading && matches!(r, HomeRow::Project(_)))
            .map_or(got.len(), |(i, _)| i);
        let at = |row: HomeRow| got.iter().position(|&r| r == row).unwrap();
        assert!(at(HomeRow::Agent(helper)) < next && at(HomeRow::Agent(researcher)) < next);
        assert!(
            at(HomeRow::Agent(w[1])) < at(HomeRow::Agent(helper)),
            "workers first"
        );
        assert_eq!(
            at(HomeRow::Agent(researcher)),
            at(HomeRow::Agent(helper)) + 1
        );
        // A hidden helper is left out, its worker stays.
        app.world.inbox.clear();
        app.settled.insert(helper);
        let got = rows(&app);
        assert!(!got.contains(&HomeRow::Agent(helper)));
        assert!(got.contains(&HomeRow::Agent(worker)));
        assert!(got.contains(&HomeRow::Agent(researcher)));
    }

    #[test]
    fn h_l_and_za_fold_the_row_at_hand_s_project_to_its_heading() {
        let (mut app, w, helper, _) = team();
        app.home_go(1);
        assert_eq!(app.at_home.at, Some(HomeRow::Agent(w[0])));
        keys(&mut app, "h");
        assert!(app.collapsed[0]);
        assert_eq!(
            app.at_home.at,
            Some(HomeRow::Project(0)),
            "the heading comes to hand"
        );
        let got = rows(&app);
        assert_eq!(got[0], HomeRow::Project(0));
        assert_eq!(got[1], HomeRow::Project(1), "folded: its rows are gone");
        assert!(!got.contains(&HomeRow::Agent(helper)));
        keys(&mut app, "l");
        assert!(!app.collapsed[0]);
        assert_eq!(rows(&app)[1], HomeRow::Agent(w[0]));
        assert_eq!(app.at_home.at, Some(HomeRow::Project(0)));
        app.key(key(KeyCode::Left));
        assert!(app.collapsed[0], "the arrows too");
        app.key(key(KeyCode::Right));
        assert!(!app.collapsed[0]);
        keys(&mut app, "za");
        assert!(app.collapsed[0], "za folds");
        keys(&mut app, "za");
        assert!(!app.collapsed[0], "and unfolds");
        // From an item's row: its agent's project folds.
        app.world.inbox.push(item(InboxKind::Question, w[1]));
        app.home_go(0);
        assert_eq!(app.at_home.at, Some(HomeRow::Item(0)));
        keys(&mut app, "h");
        assert_eq!(app.collapsed, [true, false]);
        assert_eq!(app.at_home.at, Some(HomeRow::Project(0)));
        // zM and zR fold and unfold every project, as :fold and :unfold
        // do, without leaving Home.
        keys(&mut app, "zR");
        assert_eq!(app.collapsed, [false, false]);
        keys(&mut app, "zM");
        assert_eq!(app.collapsed, [true, true]);
        app.command("unfold");
        assert_eq!(app.collapsed, [false, false]);
        app.command("fold");
        assert_eq!(app.collapsed, [true, true]);
        app.collapsed = vec![false, false];
        // One project in view: nothing heads the rows, so nothing folds.
        app.at_home.scope = Some(0);
        app.home_go(0);
        keys(&mut app, "h");
        assert_eq!(app.collapsed, [false, false], "no heading to fold to");
        assert!(!app.home_headed());
        app.at_home.scope = None;
        assert!(app.home_headed());
    }

    #[test]
    fn s_hides_an_agent_or_a_project_until_it_needs_the_person() {
        let (mut app, w, helper, researcher) = team();
        app.home_go(1);
        assert_eq!(app.at_home.at, Some(HomeRow::Agent(w[0])));
        keys(&mut app, "s");
        assert!(app.settled.contains(&w[0]));
        let name = app.world.agents[w[0]].name.clone();
        assert_eq!(
            flashed(&app).as_deref(),
            Some(format!("{name} hidden until it needs you").as_str())
        );
        let got = rows(&app);
        for id in [w[0], helper, researcher] {
            assert!(!got.contains(&HomeRow::Agent(id)), "{id} hangs under it");
        }
        assert_eq!(
            app.at_home.at,
            Some(HomeRow::Agent(w[1])),
            "the row in its place"
        );
        // The heading: the whole project.
        app.home_go(0);
        assert_eq!(app.at_home.at, Some(HomeRow::Project(0)));
        keys(&mut app, "s");
        assert!(app.settled_projects.contains(&0));
        // One project left in view: no heading over it.
        assert!(!app.home_headed());
        let first = rows(&app)[0];
        assert!(matches!(first, HomeRow::Agent(id) if app.world.agents[id].project == 1));
        assert_eq!(app.at_home.at, Some(first));
        // `:unsettle` brings them back and stays on Home.
        app.command("unsettle");
        assert!(app.settled.is_empty() && app.settled_projects.is_empty());
        assert_eq!(rows(&app)[0], HomeRow::Project(0));
        // Refused for one that needs the person: its row stays.
        app.world.agents[w[1]].status = Status::Blocked;
        let stuck = rows(&app)
            .iter()
            .position(|&r| r == HomeRow::Agent(w[1]))
            .unwrap();
        assert_eq!(stuck, 1, "stuck: first of its project, under the heading");
        app.home_go(stuck);
        assert_eq!(app.at_home.at, Some(HomeRow::Agent(w[1])));
        keys(&mut app, "s");
        assert!(!app.settled.contains(&w[1]));
        let name = app.world.agents[w[1]].name.clone();
        assert_eq!(
            flashed(&app).as_deref(),
            Some(format!("not hidden: {name} needs you").as_str())
        );
        // `:settle` on Home is `s` there.
        app.world.agents[w[1]].status = Status::Idle;
        app.command("settle");
        assert!(app.settled.contains(&w[1]));
    }

    #[test]
    fn enter_on_a_heading_opens_the_leader_and_the_preview_names_the_project() {
        let (mut app, _, _, _) = team();
        app.home_go(0);
        assert_eq!(app.home_at(), Some(HomeRow::Project(0)));
        assert_eq!(app.agent_at_hand(), Some(app.world.projects[0].leader));
        assert_eq!(app.question_at_hand(), None);
        app.key(key(KeyCode::Enter));
        assert_eq!(app.zoom, Zoom::Focus(app.world.projects[0].leader));
    }
}
