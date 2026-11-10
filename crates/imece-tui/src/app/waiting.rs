//! What needs the person: the rungs, what waits in what order, and the answers to it.

use super::*;

/// An item's place in `W`'s order (record 125): its group on Home
/// (permissions, questions, changes, then what was put off), whether a
/// change is not easy (proved clean and inside its card's paths come
/// first), then its age (or its place among those put off).
pub(super) type Rank = (u8, bool, usize);

impl App {
    /// The agents the person has open: zoomed in on, or a party of the
    /// open conversation.
    fn opened(&self) -> Vec<AgentId> {
        if let Some(id) = self.peek {
            return vec![id];
        }
        if let Zoom::Focus(id) | Zoom::Trace(id) | Zoom::Diff(id) = self.zoom {
            return vec![id];
        }
        let Some(&first) = self.thread.as_ref().and_then(|th| th.entries.first()) else {
            return Vec::new();
        };
        let Some((a, b)) = ui::words::said(&self.world.journal[first].event).map(|s| s.pair())
        else {
            return Vec::new();
        };
        [a, b]
            .into_iter()
            .filter_map(|p| match p {
                Party::Agent(id) => Some(id),
                Party::Person => None,
            })
            .collect()
    }

    /// What the person has open is seen.
    pub(super) fn seen(&mut self) {
        for id in self.opened() {
            self.unseen.remove(&id);
        }
    }

    /// Agent `id` finished a turn the person has not looked at since, and
    /// nothing of it waits for them (an answer, a question, a change: its
    /// item says more).
    pub fn done_unseen(&self, id: AgentId) -> bool {
        let w = &self.world;
        self.unseen.contains(&id)
            && w.agents[id].status == Status::Idle
            && !w.open_inbox().iter().any(|&i| w.inbox[i].agent == id)
    }

    /// The tick agent `id`'s state began, for how long its row says it has
    /// been in it: when something of its waits for the person, the oldest
    /// open item's; else its last status change.
    pub fn since(&self, id: AgentId) -> u64 {
        let w = &self.world;
        w.open_inbox()
            .into_iter()
            .map(|i| &w.inbox[i])
            .filter(|it| it.agent == id)
            .map(|it| it.tick)
            .min()
            .unwrap_or(w.agents[id].changed_at)
    }

    /// What each agent needs of the person, by id. Gone agents need
    /// nothing.
    pub fn needs(&self) -> Vec<Need> {
        self.rungs().into_iter().map(Rung::need).collect()
    }

    /// Every agent's rung on the state ladder (by id).
    /// `:why`: agent `id`'s rung, since when (the tick [`App::since`]
    /// says), and the journal's line that put it there: its newest line
    /// from then or before, if the journal still has one.
    pub fn why(&self, id: AgentId) -> (Rung, u64, Option<usize>) {
        let since = self.since(id);
        let line = (self.world.journal.iter().enumerate().rev())
            .find(|(_, e)| e.tick <= since && e.event.agent() == id)
            .map(|(i, _)| i);
        (self.rungs()[id], since, line)
    }

    pub fn rungs(&self) -> Vec<Rung> {
        let w = &self.world;
        let mut waits = vec![false; w.agents.len()];
        for i in w.open_inbox() {
            waits[w.inbox[i].agent] = true;
        }
        w.agents
            .iter()
            .map(|a| {
                if a.gone {
                    Rung::Idle
                } else if matches!(a.status, Status::Blocked | Status::Stopped) {
                    Rung::Stuck
                } else if waits[a.id] || a.status == Status::WaitingHuman {
                    Rung::Waiting
                } else if a.block.is_some() && !a.status.busy() {
                    // Blocked, its leader has it (record 143): neither
                    // done nor the person's turn.
                    Rung::Idle
                } else if a.status == Status::Idle && self.unseen.contains(&a.id) {
                    // `done_unseen`, its inbox already looked at.
                    Rung::Done
                } else if a.starting {
                    Rung::Starting
                } else if a.status.busy() || matches!(a.status, Status::Asking { .. }) {
                    Rung::Working
                } else {
                    Rung::Idle
                }
            })
            .collect()
    }

    /// What the tab's progress and the title's glyph say: the top of the
    /// ladder over the agents of the projects that are open (a closed or
    /// archived one is not on the screen), waiting also when anything is
    /// in TO DECIDE, as the title counts it. Done, not seen, is calm:
    /// the person sees it when back.
    pub fn progress(&self) -> Progress {
        let w = &self.world;
        let rungs = self.rungs();
        let top = (w.agents.iter())
            .filter(|a| !w.put_away(a.id))
            .map(|a| rungs[a.id])
            .max();
        match top {
            Some(Rung::Stuck) => Progress::Stuck,
            _ if !w.open_inbox().is_empty() => Progress::Waiting,
            Some(Rung::Waiting) => Progress::Waiting,
            Some(Rung::Working | Rung::Starting) => Progress::Working,
            _ => Progress::Calm,
        }
    }

    /// The loudest of `members`: the one on the highest rung, the first
    /// among equals; none of an empty group.
    pub fn loudest(&self, members: impl IntoIterator<Item = AgentId>) -> Option<AgentId> {
        let rungs = self.rungs();
        let mut loud: Option<AgentId> = None;
        for id in members {
            if loud.is_none_or(|l| rungs[id] > rungs[l]) {
                loud = Some(id);
            }
        }
        loud
    }

    /// Where open inbox item `i` comes in `W`'s order: the items not put
    /// off first, oldest first, then the ones put off, in the order they
    /// were.
    pub(super) fn waiting_rank(&self, i: usize) -> Rank {
        let w = &self.world;
        let item = &w.inbox[i];
        let group = self.home_group(i) as u8;
        match self.deferred.iter().position(|&d| d == i) {
            None => {
                let hard = item.kind == InboxKind::Change
                    && !w.verdict(item.agent).is_some_and(|v| v.ready);
                (group, hard, i)
            }
            Some(n) => (group, false, n),
        }
    }

    /// The inbox item the person has at hand: the one `W` last opened,
    /// open or closed since (opening a reply reads it, an answer closes
    /// it, and the panel's selection slides onto the next row when an item
    /// above it closes), else the one selected in the waiting panel.
    /// Moving the selection by hand forgets what `W` opened.
    pub(super) fn waiting_at_hand(&self) -> Option<usize> {
        self.waiting_at
    }

    /// The first open item whose place in `W`'s order comes after
    /// `after`, or the first one when none does (or `after` is none);
    /// never `skip` (the one just put off: it comes round again, after
    /// its group's rest) unless it is the only one.
    fn waiting_after(&self, after: Option<Rank>, skip: Option<usize>) -> Option<usize> {
        let open = self.world.open_inbox();
        let first = |items: &mut dyn Iterator<Item = usize>| {
            items
                .filter(|&i| Some(i) != skip)
                .min_by_key(|&i| self.waiting_rank(i))
        };
        first(
            &mut open
                .iter()
                .copied()
                .filter(|&i| after.is_none_or(|a| self.waiting_rank(i) > a)),
        )
        .or_else(|| first(&mut open.iter().copied()))
        .or_else(|| open.iter().copied().find(|&i| Some(i) == skip))
    }

    /// `W`: from wherever the person is, the next item waiting for them,
    /// selected in the salon's waiting panel and opened as `enter` opens
    /// it. With `defer` (`S`) the item at hand is put off first: it goes
    /// after everything else.
    pub(super) fn next_waiting(&mut self, defer: bool) {
        let at = self.waiting_at_hand();
        // Where it was before it was put off: what came after it then
        // comes next.
        let after = at.map(|i| self.waiting_rank(i));
        let open = |i: usize| self.world.inbox.get(i).is_some_and(|x| x.open);
        let mut skip = None;
        if defer && let Some(i) = at.filter(|&i| open(i)) {
            self.deferred.retain(|&d| d != i && open(d));
            self.deferred.push(i);
            skip = Some(i);
        }
        let Some(i) = self.waiting_after(after, skip) else {
            self.flash(self.lang.text().nothing_needed);
            return;
        };
        self.open_waiting(i);
    }

    /// Item `i` waiting for the person, selected in the salon's waiting
    /// panel and opened as `enter` opens it.
    pub(super) fn open_waiting(&mut self, i: usize) {
        // Cleared off the list: back on it, as it is dealt with.
        self.world.cleared.inbox.remove(&i);
        self.card = None;
        self.thread = None;
        self.window = None;
        self.at_home.at = Some(HomeRow::Item(i));
        self.zoom = Zoom::None;
        self.pane = Pane::Inbox;
        self.inbox_sel = self.inbox_rows().iter().position(|&o| o == i).unwrap_or(0);
        self.waiting_at = Some(i);
        self.deeper();
    }

    /// Something quitting now would cut short or leave unreviewed.
    pub(super) fn in_flight(&self) -> bool {
        !self.world.mid_turn().is_empty() || !self.world.open_changes().is_empty()
    }

    /// The open approval selected in waiting for you, if that is what
    /// is selected.
    pub(super) fn ask_at_hand(&self) -> Option<usize> {
        let item = *self.inbox_rows().get(self.inbox_sel)?;
        (self.world.inbox[item].kind == InboxKind::Approval).then_some(item)
    }

    /// `A` on an ask: yes from now on to its commands for its project
    /// (`Live::always`); the ask and the project's others they cover go
    /// on. What is not a command is said, and nothing is decided.
    pub(super) fn answer_always(&mut self) {
        if let Some(item) = self.ask_at_hand() {
            self.answer_always_to(item);
        }
    }

    /// `e` or `:yes` on approval `item` (an index into `World.inbox`):
    /// yes to everything its agent asks until its turn ends (record 104);
    /// the ask and the agent's other open ones go on now.
    pub(super) fn answer_turn(&mut self, item: usize) {
        if self.viewing_archive() {
            return self.read_only();
        }
        let id = self.world.inbox[item].agent;
        let done = match &mut self.engine {
            Engine::Live(l) => l.yes_turn(&mut self.world, item),
            Engine::Sim(s) => s.yes_turn(&mut self.world, item),
            Engine::View(_) => return self.read_only(),
        };
        if done {
            let t = self.lang.text();
            self.flash(fill(t.turn_yes_said, &[&self.world.agents[id].name]));
        }
        let open = self.inbox_rows().len();
        self.inbox_sel = self.inbox_sel.min(open.saturating_sub(1));
    }

    /// `A` on approval `item` (an index into `World.inbox`), in waiting
    /// for you or on its agent's focus.
    pub(super) fn answer_always_to(&mut self, item: usize) {
        if self.viewing_archive() {
            return self.read_only();
        }
        let t = self.lang.text();
        let p = self.world.agents[self.world.inbox[item].agent].project;
        let got = match &mut self.engine {
            Engine::Live(l) => l.always(&mut self.world, item),
            Engine::Sim(s) => s.always(&mut self.world, item),
            Engine::View(_) => return self.read_only(),
        };
        let name = self.world.projects[p].name.clone();
        let said = match got {
            Ok(added) if added.is_empty() => fill(t.always_had, &[&name]),
            Ok(added) => {
                let lines: Vec<String> = added.iter().map(Standing::line).collect();
                fill(t.always_added, &[&name, &lines.join(", ")])
            }
            Err(why) => self.standing_why(&why),
        };
        self.flash(said);
        let open = self.inbox_rows().len();
        self.inbox_sel = self.inbox_sel.min(open.saturating_sub(1));
    }

    /// Why no yes from now on could be made, in the person's words.
    fn standing_why(&self, why: &standing::Why) -> String {
        let t = self.lang.text();
        match why {
            standing::Why::NotACommand => t.always_not_command.to_string(),
            standing::Why::NotPlain => t.always_not_plain.to_string(),
            standing::Why::OptionFirst(c) => fill(t.always_option, &[c]),
        }
    }

    /// `:allow [PROGRAM [WORD]]`: the project at hand's standing yes, or
    /// a rule added to it (the project's open asks it covers go on) or,
    /// named again, taken out; a yes to one whole line (`A`) is named by
    /// that line, and only taken out.
    pub(super) fn allow(&mut self, arg: &str) {
        let t = self.lang.text();
        let p = self.current_project();
        let name = self.world.projects[p].name.clone();
        if arg.trim().is_empty() {
            let rules = &self.world.projects[p].allowed;
            let said = if rules.is_empty() {
                fill(t.allow_none, &[&name])
            } else {
                let lines: Vec<String> = rules.iter().map(Standing::line).collect();
                fill(t.allow_list, &[&name, &lines.join(", ")])
            };
            return self.flash(said);
        }
        if self.viewing_archive() {
            return self.read_only();
        }
        // A line as it is, or as the list shows it.
        let arg = arg.trim();
        let shown = (arg.strip_prefix('`')).and_then(|a| a.strip_suffix('`'));
        let line = [Some(arg), shown]
            .into_iter()
            .flatten()
            .map(Standing::exactly)
            .find(|r| self.world.projects[p].allowed.contains(r));
        let rule = match line.map_or_else(|| standing::named(arg), Ok) {
            Ok(rule) => rule,
            Err(why) => return self.flash(self.standing_why(&why)),
        };
        if self.world.unstand(p, &rule) {
            return self.flash(fill(t.allow_removed, &[&name, &rule.line()]));
        }
        self.world.stand(p, std::slice::from_ref(&rule));
        match &mut self.engine {
            Engine::Live(l) => l.go_on_allowed(&mut self.world, p),
            Engine::Sim(s) => s.go_on_allowed(&mut self.world, p),
            Engine::View(_) => {}
        }
        self.flash(fill(t.always_added, &[&name, &rule.line()]));
    }

    pub(super) fn answer(&mut self, approve: bool) {
        if let Some(&item) = self.inbox_rows().get(self.inbox_sel) {
            self.answer_item(item, approve);
        }
    }

    /// `a` or `r` on inbox item `item` (an index into `World.inbox`),
    /// wherever the person decided it: in waiting for you or on its
    /// agent's focus.
    pub(super) fn answer_item(&mut self, item: usize, approve: bool) {
        if self.viewing_archive() {
            return self.read_only();
        }
        let open = self.inbox_rows();
        let t = self.lang.text();
        if self.world.inbox[item].kind == InboxKind::Change {
            self.decide_change(self.world.inbox[item].agent, approve);
            return;
        }
        if matches!(
            self.world.inbox[item].kind,
            InboxKind::Script | InboxKind::Check
        ) {
            return self.answer_script(item, approve);
        }
        if self.world.inbox[item].kind == InboxKind::Plan {
            return self.answer_plan(item, approve);
        }
        if Cap::of(self.world.inbox[item].kind).is_some() {
            return self.answer_cap(item, approve);
        }
        if self.world.inbox[item].kind == InboxKind::Close {
            return self.answer_close(item, approve);
        }
        if self.world.inbox[item].kind == InboxKind::Approval {
            let name = self.world.agents[self.world.inbox[item].agent].name.clone();
            match &mut self.engine {
                Engine::Live(l) => l.decide(&mut self.world, item, approve),
                // The demo's session goes on once the item is closed.
                Engine::Sim(_) => self.world.decided(item, approve),
                Engine::View(_) => return self.read_only(),
            }
            let how = if approve { t.allowed } else { t.refused };
            self.flash(fill(how, &[&name]));
            self.inbox_sel = self.inbox_sel.min(open.len().saturating_sub(2));
            return;
        }
        if matches!(
            self.world.inbox[item].kind,
            InboxKind::Question | InboxKind::Fault | InboxKind::Blocked
        ) {
            // A question is answered by writing back, and an API error
            // the person mended is told the same way: the composer opens
            // to the agent, and the item closes when the message goes.
            let name = self.world.agents[self.world.inbox[item].agent].name.clone();
            self.compose(format!("@{name} "));
            return;
        }
        if self.world.inbox[item].kind == InboxKind::Reply {
            self.world.close_replies(self.world.inbox[item].agent);
        } else if approve {
            self.world.approve(item);
            self.flash(t.approved);
        } else {
            self.world.reject(item);
            self.flash(t.rejected);
        }
        self.inbox_sel = self.inbox_sel.min(open.len().saturating_sub(2));
    }
}

#[cfg(test)]
mod tests {
    use super::super::tests::*;
    use super::*;

    #[test]
    fn e_says_yes_to_all_an_agent_asks_until_its_turn_ends_from_the_inbox_a_focus_or_colon_yes() {
        let mut app = App::new(1, 2, 0, 1, Lang::En);
        let t = Lang::En.text();
        let flash = |app: &App| app.flash.as_ref().map(|(m, _)| m.clone());
        let p = app.current_project();
        let (one, two) = (
            app.world.projects[p].leader + 1,
            app.world.projects[p].leader + 2,
        );
        let (n1, n2) = (
            app.world.agents[one].name.clone(),
            app.world.agents[two].name.clone(),
        );
        let open = |app: &App| -> Vec<String> {
            (app.world.open_inbox().iter())
                .map(|&i| app.world.inbox[i].detail.clone())
                .collect()
        };
        app.world.asks_you(two, "Bash: make other");
        app.world.asks_you(one, "Bash: make a");
        app.world.asks_you(one, "Edit: x");
        // On Home, the second row: agent one's.
        app.at_home.at = Some(HomeRow::Item(app.inbox_rows()[1]));
        app.on_key(key(KeyCode::Char('e')));
        assert_eq!(flash(&app), Some(fill(t.turn_yes_said, &[&n1])));
        assert_eq!(open(&app), ["Bash: make other"]);
        assert_eq!(app.world.agents[one].turn_yes, Some(1));
        // Not on an ask: nothing.
        app.world.asked_you(two, "Which?");
        app.inbox_sel = 1;
        app.flash = None;
        app.on_key(key(KeyCode::Char('e')));
        assert_eq!(flash(&app), None);
        assert_eq!(app.world.agents[two].turn_yes, None);
        // Nor off waiting for you.
        app.pane = Pane::Salon;
        app.inbox_sel = 0;
        app.on_key(key(KeyCode::Char('e')));
        assert_eq!(app.world.agents[two].turn_yes, None);
        // On a focus: its own agent's, whatever is selected elsewhere.
        app.zoom = Zoom::Focus(one);
        app.flash = None;
        app.on_key(key(KeyCode::Char('e')));
        assert_eq!(flash(&app), Some(fill(t.turn_yes_none, &[&n1])));
        app.zoom = Zoom::Trace(two);
        app.on_key(key(KeyCode::Char('e')));
        assert_eq!(flash(&app), Some(fill(t.turn_yes_said, &[&n2])));
        assert_eq!(open(&app), ["Which?"]);
        assert_eq!(app.world.agents[two].turn_yes, Some(0));
        app.zoom = Zoom::None;
        // `:yes @NAME`, and the agent at hand.
        app.world.agents[two].turn_yes = None;
        app.world.asks_you(two, "Bash: make b");
        app.command(&format!("yes @{n1}"));
        assert_eq!(flash(&app), Some(fill(t.turn_yes_none, &[&n1])));
        app.command(&format!("yes @{n2}"));
        assert_eq!(flash(&app), Some(fill(t.turn_yes_said, &[&n2])));
        assert_eq!(open(&app), ["Which?"]);
        app.selected = Sel::Agent(two);
        app.pane = Pane::Salon;
        assert_eq!(app.command_note("yes"), format!("({n2})"));
        app.command("yes");
        assert_eq!(flash(&app), Some(fill(t.turn_yes_none, &[&n2])));
    }

    #[test]
    fn a_and_r_decide_an_ask_only_in_waiting_for_you() {
        let mut app = App::new(1, 2, 0, 1, Lang::En);
        let p = app.current_project();
        let one = app.world.projects[p].leader + 1;
        app.world.asks_you(one, "Bash: make a");
        let open = |app: &App| app.world.open_inbox().len();
        // An agent's row at hand decides nothing; the item's row does.
        app.at_home.at = Some(HomeRow::Agent(one + 1));
        app.on_key(key(KeyCode::Char('a')));
        assert_eq!(open(&app), 1, "an agent's row: a decides nothing here");
        app.on_key(key(KeyCode::Char('r')));
        assert_eq!(open(&app), 1, "an agent's row: r decides nothing here");
        app.at_home.at = Some(HomeRow::Item(0));
        app.pane = Pane::Inbox;
        app.inbox_sel = 0;
        app.on_key(key(KeyCode::Char('r')));
        assert_eq!(open(&app), 0);
    }

    #[test]
    fn a_on_an_ask_says_yes_from_now_on_and_colon_allow_lists_adds_and_drops() {
        let mut app = App::new(1, 2, 0, 1, Lang::En);
        let flash = |app: &App| app.flash.as_ref().map(|(m, _)| m.clone()).unwrap();
        let p = app.current_project();
        let name = app.world.projects[p].name.clone();
        let (one, two) = (
            app.world.projects[p].leader + 1,
            app.world.projects[p].leader + 2,
        );
        assert_eq!(app.command_note("allow"), "");
        app.command("allow");
        assert_eq!(
            flash(&app),
            format!("{name}: no yes from now on; A on an ask, or :allow PROGRAM [WORD]")
        );
        // Two asks decided before: the open ones are not the first items.
        for _ in 0..2 {
            app.world.asks_you(one, "Bash: npm install old");
            let i = app.world.inbox.len() - 1;
            app.world.decided(i, false);
        }
        app.world.asks_you(one, "Bash: npm install zod@3");
        app.world.asks_you(two, "Bash: npm install left-pad");
        app.world.asks_you(two, "Edit: package.json");
        app.pane = Pane::Inbox;
        app.inbox_sel = 0;
        app.on_key(key(KeyCode::Char('A')));
        assert_eq!(
            flash(&app),
            format!("{name}: yes from now on to npm install …")
        );
        assert!(app.archives.is_none(), "an ask at hand: not the archives");
        let open: Vec<String> = (app.world.open_inbox().iter())
            .map(|&i| app.world.inbox[i].detail.clone())
            .collect();
        assert_eq!(
            open,
            ["Edit: package.json"],
            "the other npm install went on too"
        );
        assert_eq!(app.command_note("allow"), "(npm install …)");
        // An edit can have none: said, and nothing decided.
        app.inbox_sel = 0;
        app.on_key(key(KeyCode::Char('A')));
        assert_eq!(
            flash(&app),
            "a yes from now on is for commands only: a allows this once"
        );
        assert_eq!(app.world.open_inbox().len(), 1);
        // An ask whose commands have a yes already (added without letting
        // it go on): allowed, nothing new.
        app.world.asks_you(one, "Bash: make");
        app.world.stand(
            p,
            &[Standing {
                program: "make".into(),
                sub: None,
                whole: false,
            }],
        );
        let make = app
            .inbox_rows()
            .iter()
            .position(|&i| app.world.inbox[i].detail == "Bash: make");
        app.at_home.at = Some(HomeRow::Item(app.inbox_rows()[make.unwrap()]));
        app.on_key(key(KeyCode::Char('A')));
        assert_eq!(
            flash(&app),
            format!("{name}: allowed; its commands had a yes from now on already")
        );
        app.world.unstand(
            p,
            &Standing {
                program: "make".into(),
                sub: None,
                whole: false,
            },
        );
        // :allow adds by name, again takes it out; a bad name is said.
        app.command("allow make");
        assert_eq!(flash(&app), format!("{name}: yes from now on to make"));
        app.command("allow");
        assert_eq!(
            flash(&app),
            format!("{name}: yes from now on to make, npm install …")
        );
        app.command("allow npm install");
        assert_eq!(
            flash(&app),
            format!("{name}: no yes from now on to npm install … any more")
        );
        app.command("allow rm -rf");
        assert_eq!(
            flash(&app),
            "`rm -rf` starts with an option: a yes from now on needs what it runs first; a allows this once"
        );
        app.command("allow a && b");
        assert!(flash(&app).starts_with("this line runs more than plain commands"));
        assert_eq!(app.world.projects[p].allowed.len(), 1);
        // A Python module by its interpreter and -m (record 136).
        app.command("allow python3 -m pip");
        assert_eq!(
            flash(&app),
            format!("{name}: yes from now on to python3 -m pip …")
        );
        app.command("allow python3 -m pip");
        assert_eq!(
            flash(&app),
            format!("{name}: no yes from now on to python3 -m pip … any more")
        );
        // A in waiting for you on a line no rule can hold: that line alone,
        // its ask goes on; `:allow` with the line, as listed or bare, takes
        // it back, and never adds one.
        app.world.asks_you(one, "Bash: echo `id`");
        app.pane = Pane::Inbox;
        let at = |app: &App| {
            (app.inbox_rows().iter()).position(|&i| app.world.inbox[i].detail == "Bash: echo `id`")
        };
        app.at_home.at = Some(HomeRow::Item(app.inbox_rows()[at(&app).unwrap()]));
        app.on_key(key(KeyCode::Char('A')));
        assert_eq!(
            flash(&app),
            format!("{name}: yes from now on to `echo `id``")
        );
        assert_eq!(at(&app), None, "it went on");
        let line = Standing::exactly("echo `id`");
        assert!(app.world.projects[p].allowed.contains(&line));
        app.command("allow echo `id`");
        assert_eq!(
            flash(&app),
            format!("{name}: no yes from now on to `echo `id`` any more")
        );
        assert!(!app.world.projects[p].allowed.contains(&line));
        app.command("allow echo `id`");
        assert!(flash(&app).starts_with("this line runs more than plain commands"));
        assert!(!app.world.projects[p].allowed.contains(&line));
        app.world.stand(p, std::slice::from_ref(&line));
        app.command("allow `echo `id``");
        assert!(!app.world.projects[p].allowed.contains(&line), "as listed");
        // A in waiting for you on something that is not an ask, or
        // elsewhere: the archives, as before.
        app.pane = Pane::Salon;
        app.on_key(key(KeyCode::Char('A')));
        assert_eq!(
            flash(&app),
            app.lang.text().no_archives,
            "the archives, as before"
        );
    }

    #[test]
    fn what_an_agent_needs_of_the_person_is_ranked() {
        let mut app = quiet();
        assert_eq!(app.world.agents.len(), 5, "the lead and four workers");
        assert_eq!(app.needs(), [Need::Nothing; 5], "idle, nothing open");
        // An open item waits, a closed one does not; one that left needs
        // nothing.
        app.world.inbox.push(item(1, true));
        app.world.inbox.push(item(2, false));
        app.world.inbox.push(item(3, true));
        app.world.agents[3].gone = true;
        assert_eq!(
            app.needs(),
            [
                Need::Nothing,
                Need::Waiting,
                Need::Nothing,
                Need::Nothing,
                Need::Nothing
            ]
        );
        app.world.inbox.clear();
        app.world.agents[3].gone = false;
        app.world.agents[1].status = Status::WaitingHuman;
        app.world.agents[2].status = Status::Blocked;
        app.world.agents[3].status = Status::Stopped;
        app.unseen.extend([0, 4]);
        assert_eq!(
            app.needs(),
            [
                Need::Done,
                Need::Waiting,
                Need::Stuck,
                Need::Stuck,
                Need::Done
            ]
        );
        // Done is a finished turn: at work again it is no news; an item
        // says more than done; stuck says more than an item.
        app.world.agents[4].status = Status::Working;
        app.world.inbox.push(item(0, true));
        app.world.inbox.push(item(3, true));
        assert_eq!(app.needs()[4], Need::Nothing);
        assert_eq!(app.needs()[0], Need::Waiting);
        assert_eq!(app.needs()[3], Need::Stuck, "stopped, its item waiting");
        app.world.agents[2].gone = true;
        assert_eq!(app.needs()[2], Need::Nothing, "gone and blocked");
        assert!(Need::Stuck > Need::Waiting && Need::Waiting > Need::Done);
        assert!(Need::Done > Need::Nothing);
    }

    #[test]
    fn every_agent_stands_on_one_rung_of_the_ladder() {
        let mut app = quiet();
        assert_eq!(app.rungs(), [Rung::Idle; 5]);
        let a = &mut app.world.agents;
        a[0].status = Status::Stopped;
        a[1].status = Status::WaitingHuman;
        a[2].status = Status::Working;
        a[3].status = Status::Working;
        a[3].starting = true;
        a[4].status = Status::Asleep;
        assert_eq!(
            app.rungs(),
            [
                Rung::Stuck,
                Rung::Waiting,
                Rung::Working,
                Rung::Starting,
                Rung::Idle
            ]
        );
        // Blocked is stuck too; asking and researching are work; an item
        // waiting outranks work and starting; done, not seen is idle with
        // a turn unseen; gone is nothing at all.
        let a = &mut app.world.agents;
        a[0].status = Status::Blocked;
        a[1].status = Status::Asking { to: 0, until: 9 };
        a[2].status = Status::Researching {
            for_agent: 0,
            until: 9,
        };
        a[4].status = Status::Idle;
        app.unseen.insert(4);
        app.world.inbox.push(item(2, true));
        app.world.inbox.push(item(3, true));
        assert_eq!(
            app.rungs(),
            [
                Rung::Stuck,
                Rung::Working,
                Rung::Waiting,
                Rung::Waiting,
                Rung::Done
            ]
        );
        app.world.inbox.clear();
        assert_eq!(app.rungs()[2], Rung::Working, "researching");
        assert_eq!(app.rungs()[3], Rung::Starting);
        app.world.agents[3].gone = true;
        assert_eq!(app.rungs()[3], Rung::Idle, "gone");
        app.world.agents[4].status = Status::Working;
        assert_eq!(app.rungs()[4], Rung::Working, "at work again: no news");
        // The ladder, lowest first, and what each rung needs.
        let ladder = [
            Rung::Idle,
            Rung::Done,
            Rung::Starting,
            Rung::Working,
            Rung::Waiting,
            Rung::Stuck,
        ];
        assert!(ladder.windows(2).all(|w| w[0] < w[1]));
        assert_eq!(
            ladder.map(Rung::need),
            [
                Need::Nothing,
                Need::Done,
                Need::Nothing,
                Need::Nothing,
                Need::Waiting,
                Need::Stuck
            ]
        );
    }

    #[test]
    fn the_loudest_of_a_group_is_its_highest_rung_the_first_among_equals() {
        let mut app = quiet();
        assert_eq!(app.loudest([]), None);
        assert_eq!(app.loudest([3, 1, 2]), Some(3), "all idle: the first");
        app.world.agents[2].status = Status::Working;
        assert_eq!(app.loudest([3, 1, 2]), Some(2));
        app.world.agents[1].status = Status::Working;
        assert_eq!(app.loudest([3, 1, 2]), Some(1), "equals: the first");
        app.world.agents[3].status = Status::Working;
        app.world.agents[3].starting = true;
        assert_eq!(app.loudest([3, 1, 2]), Some(1), "working over starting");
        app.unseen.insert(4);
        assert_eq!(app.loudest([4, 2]), Some(2), "working over done");
        app.world.inbox.push(item(4, true));
        assert_eq!(app.loudest([1, 4, 2]), Some(4), "waiting over working");
        app.world.agents[2].status = Status::Stopped;
        assert_eq!(app.loudest([1, 4, 2]), Some(2), "stuck over waiting");
    }

    #[test]
    fn the_tab_and_the_title_say_the_top_of_the_open_projects_ladder() {
        let mut app = quiet();
        assert_eq!(app.progress(), Progress::Calm);
        app.unseen.insert(4);
        assert_eq!(app.progress(), Progress::Calm, "done, not seen, is calm");
        app.world.agents[3].starting = true;
        assert_eq!(app.progress(), Progress::Working, "starting");
        app.world.agents[3].starting = false;
        app.world.agents[2].status = Status::Working;
        assert_eq!(app.progress(), Progress::Working);
        app.world.agents[1].status = Status::WaitingHuman;
        assert_eq!(app.progress(), Progress::Waiting);
        app.world.agents[0].status = Status::Blocked;
        assert_eq!(app.progress(), Progress::Stuck, "stuck over all");
        app.world.agents[0].status = Status::Stopped;
        assert_eq!(app.progress(), Progress::Stuck);
        app.world.agents[0].gone = true;
        assert_eq!(app.progress(), Progress::Waiting, "gone is not stuck");
        // An item waiting is waiting, whoever it is of, as the title counts.
        let mut app = quiet();
        app.world.agents[2].status = Status::Working;
        app.world.inbox.push(item(4, true));
        app.world.agents[4].gone = true;
        assert_eq!(app.progress(), Progress::Waiting);
        app.world.agents[0].status = Status::Stopped;
        assert_eq!(app.progress(), Progress::Stuck, "stuck over waiting");
        // A closed project's agents are not on the screen.
        let (mut app, [a, _, other]) = two_quiet_projects();
        app.world.agents[other].status = Status::Blocked;
        assert_eq!(app.progress(), Progress::Stuck);
        let p = app.world.agents[other].project;
        app.world.projects[p].closed = true;
        assert_eq!(app.progress(), Progress::Calm, "closed");
        app.world.agents[a].status = Status::Working;
        assert_eq!(app.progress(), Progress::Working);
        // The title starts with its glyph.
        let title = |w, p| ui::words::title(Lang::En, w, p, None);
        assert_eq!(title(0, Progress::Calm), "⁙ imece");
        assert_eq!(title(0, Progress::Working), "● imece");
        assert_eq!(title(2, Progress::Waiting), "! imece · 2 to decide");
        assert_eq!(title(1, Progress::Stuck), "■ imece · 1 to decide");
        assert_eq!(
            ui::words::title(Lang::Tr, 3, Progress::Waiting, None),
            "! imece · 3 karar bekliyor"
        );
    }

    #[test]
    fn a_turn_that_ends_unseen_is_marked_until_it_is_opened() {
        let mut app = quiet();
        app.world.agents[1].status = Status::Working;
        app.world.agents[2].status = Status::Researching {
            for_agent: 1,
            until: 9,
        };
        app.world.agents[3].status = Status::Working;
        app.watch_turns();
        assert!(app.unseen.is_empty(), "still at work");
        app.world.agents[1].status = Status::Idle;
        app.world.agents[2].status = Status::Idle;
        app.world.agents[3].status = Status::Asking { to: 1, until: 9 };
        app.watch_turns();
        assert_eq!(app.unseen, BTreeSet::from([1, 2]), "asking is not done");
        // At work again: the earlier turn is no news any more.
        app.world.agents[1].status = Status::Working;
        app.watch_turns();
        assert_eq!(app.unseen, BTreeSet::from([2]));
        // Opened: seen.
        app.zoom = Zoom::Focus(2);
        app.on_key(key(KeyCode::Char('x')));
        assert!(app.unseen.is_empty());
        // A turn that ends while the person looks at it is seen.
        app.zoom = Zoom::Trace(1);
        app.world.agents[1].status = Status::Idle;
        app.watch_turns();
        assert!(app.unseen.is_empty(), "{:?}", app.unseen);
    }

    #[test]
    fn the_demo_marks_the_turns_that_end_as_it_steps() {
        let mut app = App::new(1, 4, 0, 7, Lang::En);
        // A bounded range: the demo's workers finish turns well within it.
        let marked = (0..400).position(|_| {
            app.step();
            let ids: Vec<AgentId> = app.world.agents.iter().map(|a| a.id).collect();
            ids.into_iter().any(|id| app.done_unseen(id))
        });
        assert!(marked.is_some(), "a turn ended and nobody looked");
    }

    #[test]
    fn what_is_open_is_the_zoomed_agent_or_the_conversations_agents() {
        let mut app = quiet();
        assert!(app.opened().is_empty());
        for (zoom, id) in [(Zoom::Focus(1), 1), (Zoom::Trace(2), 2), (Zoom::Diff(3), 3)] {
            app.zoom = zoom;
            assert_eq!(app.opened(), [id]);
        }
        app.zoom = Zoom::None;
        app.world.note(Event::YouSaid {
            to: 2,
            text: "hi".into(),
        });
        app.open_thread(app.world.journal.len() - 1);
        assert_eq!(app.opened(), [2], "the person is not an agent");
        app.world.note(Event::Talk {
            from: 3,
            to: 1,
            text: "hi".into(),
        });
        app.open_thread(app.world.journal.len() - 1);
        let mut both = app.opened();
        both.sort();
        assert_eq!(both, [1, 3]);
        app.unseen = BTreeSet::from([1, 2, 3]);
        app.on_key(key(KeyCode::Char('j')));
        assert_eq!(app.unseen, BTreeSet::from([2]));
    }

    #[test]
    fn since_is_the_oldest_open_item_s_tick_else_the_last_status_change() {
        let mut app = quiet();
        app.world.tick = 50;
        app.world.agents[1].changed_at = 5;
        assert_eq!(app.since(1), 5);
        let item = |agent, tick, open| InboxItem {
            kind: InboxKind::Question,
            agent,
            detail: "q".into(),
            researcher: None,
            open,
            tick,
            card: None,
        };
        app.world.inbox.push(item(1, 9, true));
        assert_eq!(app.since(1), 9, "the item, even after the status change");
        app.world.inbox.push(item(1, 7, true));
        assert_eq!(app.since(1), 7, "the oldest");
        app.world.inbox.push(item(1, 3, false));
        assert_eq!(app.since(1), 7, "a closed one does not count");
        app.world.inbox.push(item(2, 2, true));
        assert_eq!(app.since(1), 7, "another's does not count");
        assert_eq!(app.since(2), 2);
    }

    #[test]
    fn done_unseen_is_an_idle_agent_marked_with_nothing_waiting() {
        let mut app = quiet();
        assert!(!app.done_unseen(1), "not marked");
        app.unseen.insert(1);
        assert!(app.done_unseen(1));
        app.world.agents[1].status = Status::Asleep;
        assert!(!app.done_unseen(1), "only an idle one");
        app.world.agents[1].status = Status::Idle;
        app.world.inbox.push(InboxItem {
            kind: InboxKind::Question,
            agent: 1,
            detail: "which?".into(),
            researcher: None,
            open: true,
            tick: 0,
            card: None,
        });
        assert!(!app.done_unseen(1), "its question says more");
        app.world.inbox[0].agent = 2;
        assert!(app.done_unseen(1), "another's item is not its");
        app.world.inbox[0].agent = 1;
        app.world.inbox[0].open = false;
        assert!(app.done_unseen(1), "a closed item waits no more");
    }

    #[test]
    fn capital_w_opens_what_waits_one_after_the_other_from_anywhere() {
        let mut app = with_cards(1);
        app.world.inbox.clear();
        for n in 0..3 {
            app.world.inbox.push(InboxItem {
                kind: InboxKind::Sensitive,
                agent: n + 1,
                detail: format!("item {n}"),
                researcher: None,
                open: true,
                tick: n as u64,
                card: None,
            });
        }
        // A card's screen, a conversation and a raw stream.
        app.open_card(0);
        app.zoom = Zoom::Trace(0);
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
            passing: false,
        });
        assert_eq!(press(&mut app, 'W'), Some(0));
        assert!(app.card.is_none() && app.thread.is_none());
        assert_eq!(app.inbox_sel, 0);
        assert_eq!(press(&mut app, 'W'), Some(1));
        assert_eq!(app.inbox_sel, 1);
        assert_eq!(press(&mut app, 'W'), Some(2));
        assert_eq!(app.inbox_sel, 2);
        assert_eq!(press(&mut app, 'W'), Some(0), "round again");
        assert_eq!(app.inbox_sel, 0);
    }

    #[test]
    fn capital_s_puts_the_item_off_and_w_comes_to_it_after_the_rest() {
        let mut app = waiting(&[InboxKind::Sensitive; 4]);
        assert_eq!(press(&mut app, 'W'), Some(0));
        assert_eq!(press(&mut app, 'W'), Some(1));
        assert_eq!(press(&mut app, 'S'), Some(2), "1 put off, the next open");
        assert_eq!(app.deferred, [1]);
        assert_eq!(press(&mut app, 'W'), Some(3));
        assert_eq!(press(&mut app, 'W'), Some(1), "what was put off comes last");
        assert_eq!(press(&mut app, 'S'), Some(0), "put off again, round");
        assert_eq!(app.deferred, [1]);
        assert_eq!(press(&mut app, 'S'), Some(2));
        assert_eq!(app.deferred, [1, 0], "in the order they were put off");
        assert_eq!(press(&mut app, 'W'), Some(3));
        assert_eq!(press(&mut app, 'W'), Some(1), "not by age");
        assert_eq!(press(&mut app, 'W'), Some(0));
        assert_eq!(press(&mut app, 'W'), Some(2));
        // What was put off and closed since is forgotten.
        app.world.inbox[1].open = false;
        assert_eq!(press(&mut app, 'S'), Some(3));
        assert_eq!(app.deferred, [0, 2]);
    }

    #[test]
    fn capital_w_goes_on_after_what_it_opened_or_what_the_person_picked() {
        use InboxKind::{Reply, Sensitive};
        // Replies alone: one waits after every decision (record 125).
        // A reply is read when opened: W goes on after it all the same.
        let mut app = waiting(&[Reply; 4]);
        app.on_key(key(KeyCode::Char('W')));
        assert!(!app.world.inbox[0].open, "read");
        assert_eq!(app.waiting_at, Some(0));
        assert_eq!(press(&mut app, 'W'), Some(1));
        // Put off: nothing closed is put off.
        let mut app = waiting(&[Reply; 4]);
        app.on_key(key(KeyCode::Char('W')));
        assert_eq!(press(&mut app, 'S'), Some(1));
        assert!(app.deferred.is_empty(), "{:?}", app.deferred);
        // The person picks another row by hand: W goes on from there. The
        // focus W opened takes `j k gg G` itself: back to the panel first.
        let mut app = waiting(&[Reply; 4]);
        app.on_key(key(KeyCode::Char('W')));
        app.on_key(key(KeyCode::Char('j')));
        assert_eq!(app.waiting_at, Some(0), "the focus stepped");
        app.on_key(key(KeyCode::Esc));
        app.on_key(key(KeyCode::Char('j')));
        assert_eq!(app.waiting_at, Some(2), "picked 2 (0 was read and went)");
        assert_eq!(press(&mut app, 'W'), Some(3), "after the picked 2");
        let mut app = waiting(&[Reply; 4]);
        app.on_key(key(KeyCode::Char('W')));
        app.on_key(key(KeyCode::Esc));
        app.on_key(key(KeyCode::Char('G')));
        assert_eq!(app.waiting_at, None);
        app.on_key(key(KeyCode::Char('g')));
        app.on_key(key(KeyCode::Char('g')));
        assert_eq!(press(&mut app, 'W'), Some(2), "after the picked 1");
        // The person looked elsewhere: W goes on after what it opened, not
        // after a selection the waiting panel no longer has in hand.
        let mut app = waiting(&[Sensitive; 4]);
        app.on_key(key(KeyCode::Char('W')));
        app.on_key(key(KeyCode::Char('W')));
        app.zoom = Zoom::None;
        app.pane = Pane::Journal;
        app.inbox_sel = 2;
        assert_eq!(press(&mut app, 'W'), Some(2), "after 1, not after 2");
        // Nothing at hand: the first.
        let mut app = waiting(&[Sensitive; 3]);
        app.inbox_sel = 2;
        assert_eq!(press(&mut app, 'W'), Some(0));
    }

    #[test]
    fn capital_w_with_nothing_waiting_says_so_and_stays() {
        let mut app = waiting(&[InboxKind::Sensitive]);
        app.world.inbox[0].open = false;
        for c in ['W', 'S'] {
            app.on_key(key(KeyCode::Char(c)));
            assert_eq!((app.pane, app.zoom), (Pane::Salon, Zoom::None));
            let said = app.flash.as_ref().map(|(m, _)| m.as_str());
            assert_eq!(said, Some(Lang::En.text().nothing_needed));
            app.flash = None;
        }
        assert!(app.deferred.is_empty());
    }

    #[test]
    fn colon_why_says_the_rung_since_when_and_the_journal_line_that_put_it_there() {
        use crate::model::clock;
        let mut app = quiet();
        let (a, b) = (2, 3);
        let name = app.world.agents[a].name.clone();
        app.world.journal.clear();
        app.world.tick = 20;
        app.world.agents[a].changed_at = 10;
        // Nothing in the journal: said so.
        assert_eq!(app.why(a), (Rung::Idle, 10, None));
        app.command(&format!("why @{name}"));
        let unsaid = format!(
            "{name}: idle · since {} · the journal does not say why",
            clock(10)
        );
        assert_eq!(flashed(&app).unwrap_or_default(), unsaid);
        // Its newest line from then or before; not a later one, not
        // another agent's.
        let stopped = |agent, why: &str| Event::Stopped {
            agent,
            why: why.into(),
        };
        for (tick, e) in [
            (4, stopped(a, "old")),
            (10, stopped(a, "gone")),
            (11, stopped(b, "other")),
            (12, stopped(a, "later")),
        ] {
            app.world.journal.push(Entry { tick, event: e });
        }
        app.world.agents[a].status = Status::Stopped;
        assert_eq!(app.why(a), (Rung::Stuck, 10, Some(1)), "the line of then");
        app.world.agents[a].changed_at = 9;
        assert_eq!(app.why(a), (Rung::Stuck, 9, Some(0)), "the one before");
        app.world.agents[a].changed_at = 10;
        app.selected = Sel::Agent(a);
        app.command("why");
        let line = ui::words::event(Lang::En, &app.world, &app.world.journal[1].event);
        let said = format!("{name}: stuck · since {} · {line}", clock(10));
        assert_eq!(flashed(&app).unwrap_or_default(), said, "the agent at hand");
        // Waiting: since its oldest open item.
        app.world.agents[a].status = Status::Idle;
        app.world.inbox.push(InboxItem {
            kind: InboxKind::Question,
            agent: a,
            detail: "which?".into(),
            researcher: None,
            open: true,
            tick: 8,
            card: None,
        });
        assert_eq!(app.why(a), (Rung::Waiting, 8, Some(0)));
        // In Turkish; a name nobody has.
        app.lang = Lang::Tr;
        app.command(&format!("why {name}"));
        let line = ui::words::event(Lang::Tr, &app.world, &app.world.journal[0].event);
        let said = format!("{name}: seni bekliyor · {} itibarıyla · {line}", clock(8));
        assert_eq!(flashed(&app).unwrap_or_default(), said);
        app.command("why nobody");
        assert_eq!(
            flashed(&app).unwrap_or_default(),
            "\"nobody\" adında bir agent yok"
        );
    }

    #[test]
    fn colon_why_says_how_many_messages_wait_with_the_agent() {
        use crate::model::clock;
        let mut app = quiet();
        let a = 2;
        let name = app.world.agents[a].name.clone();
        app.world.journal.clear();
        app.world.tick = 20;
        app.world.agents[a].changed_at = 10;
        app.world.agents[a].held_mail = 2;
        app.command(&format!("why @{name}"));
        let said = format!(
            "{name}: idle · since {} · the journal does not say why · messages queued until \
             you answer: 2",
            clock(10)
        );
        assert_eq!(flashed(&app).unwrap_or_default(), said);
        app.lang = Lang::Tr;
        app.command(&format!("why @{name}"));
        assert!(
            (flashed(&app).unwrap_or_default())
                .ends_with("sen yanıtlayana dek kuyrukta bekleyen mesaj: 2")
        );
    }

    #[test]
    fn every_rung_has_its_words_in_both_languages() {
        let app = quiet();
        let name = &app.world.agents[0].name;
        for (rung, en, tr) in [
            (Rung::Idle, "idle", "boşta"),
            (Rung::Done, "done, not seen", "bitti, bakılmadı"),
            (Rung::Starting, "starting", "başlıyor"),
            (Rung::Working, "working", "çalışıyor"),
            (Rung::Waiting, "needs you", "seni bekliyor"),
            (Rung::Stuck, "stuck", "takıldı"),
        ] {
            let said = |lang| ui::words::why(lang, &app.world, 0, (rung, 0, None));
            assert!(
                said(Lang::En).starts_with(&format!("{name}: {en} · ")),
                "{rung:?}"
            );
            assert!(
                said(Lang::Tr).starts_with(&format!("{name}: {tr} · ")),
                "{rung:?}"
            );
        }
    }
}

#[cfg(test)]
mod rank_tests {
    use super::*;

    fn item(kind: InboxKind, agent: AgentId, card: Option<CardId>) -> InboxItem {
        InboxItem {
            kind,
            agent,
            detail: "x".into(),
            researcher: None,
            open: true,
            tick: 0,
            card,
        }
    }

    fn change(agent: AgentId, ok: bool, path: &str) -> Change {
        Change {
            agent,
            title: "t".into(),
            files: vec![ChangedFile {
                path: path.into(),
                added: 1,
                removed: 0,
                binary: false,
            }],
            patch: String::new(),
            runs: vec![crate::model::Ran {
                line: "cargo test".into(),
                code: Some(if ok { 0 } else { 1 }),
                timed_out: false,
            }],
            check: Check::None,
            spend: None,
        }
    }

    #[test]
    fn the_rank_is_group_then_put_off_then_hard_then_age() {
        let mut app = App::new(1, 4, 0, 1, Lang::En);
        app.world.inbox.clear();
        app.world.inbox.push(item(InboxKind::Change, 1, None));
        app.world.inbox.push(item(InboxKind::Question, 2, None));
        app.world.inbox.push(item(InboxKind::Approval, 3, None));
        app.world.inbox.push(item(InboxKind::Approval, 4, None));
        assert_eq!(app.waiting_rank(2), (0, false, 2));
        assert_eq!(app.waiting_rank(3), (0, false, 3));
        assert_eq!(app.waiting_rank(1), (1, false, 1));
        // A change without a proof is hard; one proved clean and inside
        // its card's paths is not.
        assert_eq!(app.waiting_rank(0), (2, true, 0));
        app.world.changes.push(change(1, true, "src/a.rs"));
        assert_eq!(app.waiting_rank(0), (2, false, 0));
        app.world.changes[0].runs[0].code = Some(1);
        assert_eq!(app.waiting_rank(0), (2, true, 0));
        app.world.changes[0].runs[0].code = Some(0);
        app.world.cards[0].paths = crate::paths::parse(&serde_json::json!(["docs/"])).unwrap();
        app.world.inbox[0].card = Some(0);
        assert_eq!(app.waiting_rank(0), (2, true, 0), "outside its card");
        app.world.cards[0].paths = crate::paths::parse(&serde_json::json!(["src/"])).unwrap();
        assert_eq!(app.waiting_rank(0), (2, false, 0));
        // Overlapping another change waiting in the project: hard.
        app.world.changes.push(change(2, true, "src/a.rs"));
        assert_eq!(app.waiting_rank(0), (2, true, 0), "overlaps");
        app.world.changes.pop();
        assert_eq!(app.waiting_rank(0), (2, false, 0));
        // Put off: after everything, in the order put off.
        app.deferred = vec![3, 2];
        assert_eq!(app.waiting_rank(3), (4, false, 0));
        assert_eq!(app.waiting_rank(2), (4, false, 1));
        assert!(app.waiting_rank(3) < app.waiting_rank(2));
        assert!(
            app.waiting_rank(0) < app.waiting_rank(3),
            "after the changes"
        );
    }

    #[test]
    fn w_takes_a_permission_before_an_older_question_and_an_easy_change_before_a_hard_one() {
        let mut app = App::new(1, 4, 0, 1, Lang::En);
        app.world.inbox.clear();
        app.world.inbox.push(item(InboxKind::Question, 1, None));
        app.world.inbox.push(item(InboxKind::Change, 2, None));
        app.world.inbox.push(item(InboxKind::Change, 3, None));
        app.world.inbox.push(item(InboxKind::Approval, 4, None));
        app.world.changes.push(change(3, true, "src/a.rs"));
        app.next_waiting(false);
        assert_eq!(app.waiting_at, Some(3), "the permission first");
        app.next_waiting(false);
        assert_eq!(app.waiting_at, Some(0), "then the question");
        app.next_waiting(false);
        assert_eq!(app.waiting_at, Some(2), "then the easy change");
        app.next_waiting(false);
        assert_eq!(app.waiting_at, Some(1));
        app.next_waiting(false);
        assert_eq!(app.waiting_at, Some(3), "round");
        // Put off, a permission is not the next item: the other
        // permission is, then the question; the put-off ones come last.
        app.world.inbox.push(item(InboxKind::Approval, 1, None));
        app.next_waiting(true);
        assert_eq!(app.deferred, [3]);
        assert_eq!(app.waiting_at, Some(4), "the other permission");
        app.next_waiting(true);
        assert_eq!(app.deferred, [3, 4]);
        assert_eq!(app.waiting_at, Some(0), "the question, not a put-off one");
        for _ in 0..3 {
            app.next_waiting(false);
        }
        assert_eq!(app.waiting_at, Some(3), "the first put off, after the rest");
        app.next_waiting(false);
        assert_eq!(app.waiting_at, Some(4));
    }

    #[test]
    fn the_last_item_put_off_goes_round_to_the_first_not_to_itself() {
        let mut app = App::new(1, 2, 0, 1, Lang::En);
        app.world.inbox.clear();
        app.world.inbox.push(item(InboxKind::Approval, 1, None));
        app.world.inbox.push(item(InboxKind::Question, 2, None));
        app.waiting_at = Some(1);
        app.next_waiting(true);
        assert_eq!(app.deferred, [1]);
        assert_eq!(app.waiting_at, Some(0), "round to the first");
        // Alone, it comes round to itself.
        app.world.inbox[0].open = false;
        app.waiting_at = Some(1);
        app.next_waiting(true);
        assert_eq!(app.waiting_at, Some(1));
    }
}
