//! A stop of a whole team on Home (record 146, part 2): its row first in
//! TO DECIDE, a digit for each of imece's choices on it, `s` to let it
//! rest after the rest, `enter` to what stopped.

use super::*;
use crate::halt::{Choice, FAILED_AGAIN, Halt, LEAD_AGAIN, SOLO_AGAIN};

impl App {
    /// The stops Home lists for `scope`, in the world's order, each with
    /// whether the person let it rest (`s`, or the choice to rest): those
    /// go after everything else waiting.
    pub(crate) fn halts_in(&self, scope: Option<ProjectId>) -> Vec<(bool, ProjectId, Halt)> {
        let w = &self.world;
        (w.halts.stops.iter())
            .filter(|s| scope.is_none_or(|p| p == s.project))
            .filter(|s| !w.projects[s.project].closed)
            .map(|s| {
                let rests = self.rested.contains(&(s.project, s.halt));
                (rests, s.project, s.halt)
            })
            .collect()
    }

    /// The stop at hand on Home.
    pub(crate) fn halt_at_hand(&self) -> Option<(ProjectId, Halt)> {
        if !self.home_keys() {
            return None;
        }
        match self.home_at()? {
            HomeRow::Stop(p, halt) => Some((p, halt)),
            HomeRow::Item(_) | HomeRow::Agent(_) | HomeRow::Project(_) => None,
        }
    }

    /// What digit `d` chooses on the stop at hand, counted from 1.
    pub(super) fn choice_at_hand(&self, d: char) -> Option<Choice> {
        let (p, halt) = self.halt_at_hand()?;
        self.world.halt_of(p, halt)?;
        let n = usize::try_from(d.to_digit(10)?).ok()?.checked_sub(1)?;
        halt.choices().get(n).copied()
    }

    /// Digit `d` on the stop at hand: imece does what the choice says,
    /// nothing more; the row goes once the stop is over.
    pub(super) fn choose(&mut self, d: char) {
        let (Some((p, halt)), Some(c)) = (self.halt_at_hand(), self.choice_at_hand(d)) else {
            return;
        };
        let t = self.lang.text();
        match c {
            Choice::Raise(by) => self.set_budget(self.world.budget + by),
            Choice::Rest => self.rest(p, halt),
            Choice::GoOn => {
                match &mut self.engine {
                    Engine::Live(l) => l.go_on(),
                    Engine::Sim(s) => s.go_on(),
                    Engine::View(_) => return self.read_only(),
                }
                let more = crate::halt::MAX_WOKEN.to_string();
                self.flash(fill(t.halt_going_on, &[&more]));
            }
            Choice::LookAgain => {
                match &mut self.engine {
                    Engine::Live(l) => l.look_now(),
                    Engine::Sim(_) => {}
                    Engine::View(_) => return self.read_only(),
                }
                self.flash(t.halt_looking);
            }
            Choice::Store => {
                self.selected = Sel::Project(p);
                self.open_lonca(p);
            }
            Choice::StartLead => {
                let head = self.world.projects[p].leader;
                let again = match self.world.agents[head].role {
                    Role::Solo => SOLO_AGAIN,
                    _ => LEAD_AGAIN,
                };
                self.tell(head, again);
            }
            Choice::TryNow => {
                self.world.try_now();
                self.flash(t.halt_trying);
            }
            Choice::EndTurn => {
                let head = self.world.projects[p].leader;
                let refused = match &mut self.engine {
                    Engine::Live(l) => l.end_turn(&mut self.world, head).err(),
                    Engine::Sim(s) => s.end_turn(&mut self.world, head).err(),
                    Engine::View(_) => return self.read_only(),
                };
                self.report(head, crate::halt::HUNG_AGAIN, refused);
            }
            Choice::Again => {
                let head = self.world.projects[p].leader;
                self.tell(head, FAILED_AGAIN);
            }
        }
        self.home_decided();
    }

    /// The stop of `halt` in project `p` goes after everything else
    /// waiting, and stays there while it holds.
    pub(super) fn rest(&mut self, p: ProjectId, halt: Halt) {
        if !self.rested.contains(&(p, halt)) {
            self.rested.push((p, halt));
        }
        self.flash(self.lang.text().does_rest);
        self.at_home.at = None;
        self.home_go(self.at_home.row);
    }

    /// `enter` on a stop: what stopped. Lonca's window for the store, the
    /// leader's work for the rest.
    pub(super) fn open_halt(&mut self, p: ProjectId, halt: Halt) {
        match halt {
            Halt::Store => {
                self.selected = Sel::Project(p);
                self.open_lonca(p);
            }
            Halt::Pot | Halt::Woken | Halt::Limit | Halt::Lead | Halt::Hung | Halt::Failed => {
                self.zoom = Zoom::Focus(self.world.projects[p].leader);
            }
        }
    }

    /// The pot is `to` from now: what `:budget` and the stop's raise do.
    pub(super) fn set_budget(&mut self, to: u64) {
        let t = self.lang.text();
        let from = self.world.budget;
        self.world.budget = to;
        // A new pot is worth a new warning.
        self.world.spending.warned = false;
        let leader = self.world.projects[self.current_project()].leader;
        self.world.note(Event::Budget { leader, from, to });
        self.flash(fill(t.budget_is, &[&dollars(to)]));
    }

    /// Forgets the rests of stops that are over: a stop that comes again
    /// is first again.
    pub(super) fn watch_rests(&mut self) {
        let w = &self.world;
        self.rested
            .retain(|&(p, halt)| w.halt_of(p, halt).is_some());
    }
}

#[cfg(test)]
mod tests {
    use super::super::tests::*;
    use super::*;
    use crate::halt::{Held, Waits};

    /// A quiet demo team of four, nothing waiting, and `halts` holding
    /// with one behind each held one.
    fn stopped(halts: &[Halt]) -> App {
        let mut app = quiet();
        let w = &mut app.world;
        let mut held = Vec::new();
        for &halt in halts {
            match halt {
                Halt::Pot => {
                    w.budget = w.spent();
                    held.push(Held {
                        agent: 1,
                        by: Halt::Pot,
                        waits: Waits::Retry,
                    });
                }
                Halt::Woken => held.push(Held {
                    agent: 2,
                    by: Halt::Woken,
                    waits: Waits::Wake,
                }),
                Halt::Store => {
                    w.projects[0].lonca = Some(OnLonca {
                        repo: "shop".into(),
                        seen: StoreSeen::Down(StoreDown::NoDaemon),
                        leases: Vec::new(),
                        pending: Vec::new(),
                        policy_checks: None,
                    });
                }
                Halt::Lead => {
                    let leader = w.projects[0].leader;
                    w.agents[leader].status = Status::Stopped;
                }
                Halt::Limit => w.limit_hit(1, "Claude usage limit reached"),
                Halt::Hung => {
                    let leader = w.projects[0].leader;
                    let a = &mut w.agents[leader];
                    a.status = Status::Working;
                    a.starting = false;
                    a.retry = None;
                    a.changed_at = 0;
                    a.watch.traced_at = 0;
                    w.tick = w.tick.max(1_000);
                }
                Halt::Failed => {
                    let leader = w.projects[0].leader;
                    w.agents[leader].status = Status::Idle;
                    w.agents[leader].changed_at = 0;
                    w.head_failed(leader, "error_max_turns");
                }
            }
        }
        w.look_halts(&held);
        app.home_go(0);
        app
    }

    fn rows(app: &App) -> Vec<(HomeGroup, HomeRow)> {
        app.home_rows()
    }

    #[test]
    fn a_stop_is_first_on_home_and_one_let_rest_comes_after_everything_waiting() {
        let mut app = stopped(&[Halt::Pot]);
        app.world.inbox.push(item(1, true));
        assert_eq!(
            rows(&app)[0],
            (HomeGroup::Stopped, HomeRow::Stop(0, Halt::Pot))
        );
        assert_eq!(rows(&app)[1].1, HomeRow::Item(0));
        app.home_go(0);
        app.home_key(key(KeyCode::Char('s')));
        assert_eq!(app.rested, vec![(0, Halt::Pot)]);
        assert_eq!(rows(&app)[0].1, HomeRow::Item(0));
        assert_eq!(
            rows(&app)[1],
            (HomeGroup::Later, HomeRow::Stop(0, Halt::Pot))
        );
        // A rest lasts while its stop holds: one that comes again is
        // first again.
        app.world.halts.stops.clear();
        app.watch_rests();
        assert!(app.rested.is_empty());
    }

    #[test]
    fn the_pot_s_digits_raise_it_by_their_dollars_or_let_it_rest() {
        let mut app = stopped(&[Halt::Pot]);
        let before = app.world.budget;
        assert_eq!(app.choice_at_hand('1'), Some(Choice::Raise(500_000)));
        assert_eq!(app.choice_at_hand('4'), None, "three choices");
        assert_eq!(app.choice_at_hand('0'), None);
        app.on_key(key(KeyCode::Char('1')));
        assert_eq!(app.world.budget, before + 500_000);
        let raised = app.world.journal.iter().rev().find_map(|e| match e.event {
            Event::Budget { from, to, .. } => Some((from, to)),
            _ => None,
        });
        assert_eq!(raised, Some((before, before + 500_000)));
        let mut app = stopped(&[Halt::Pot]);
        let before = app.world.budget;
        app.on_key(key(KeyCode::Char('2')));
        assert_eq!(app.world.budget, before + 200_000);
        let mut app = stopped(&[Halt::Pot]);
        app.on_key(key(KeyCode::Char('3')));
        assert_eq!(app.rested, vec![(0, Halt::Pot)]);
        assert_eq!(app.world.budget, before, "a rest raises nothing");
    }

    #[test]
    fn a_digit_on_anything_but_a_stop_chooses_nothing_of_it() {
        let mut app = stopped(&[Halt::Pot]);
        app.world.inbox.push(item(1, true));
        app.home_go(1);
        assert_eq!(app.choice_at_hand('1'), None);
        let before = app.world.budget;
        app.on_key(key(KeyCode::Char('1')));
        assert_eq!(app.world.budget, before);
        // Over Home, a window open: nothing either.
        app.home_go(0);
        app.zoom = Zoom::Focus(1);
        assert_eq!(app.choice_at_hand('1'), None);
    }

    #[test]
    fn letting_a_capped_team_go_on_tells_the_engine_and_starting_the_lead_tells_it() {
        let mut app = stopped(&[Halt::Woken]);
        assert_eq!(app.choice_at_hand('1'), Some(Choice::GoOn));
        app.on_key(key(KeyCode::Char('1')));
        assert_eq!(
            flashed(&app).as_deref(),
            Some("the team goes on: 12 turns more on messages")
        );
        let mut app = stopped(&[Halt::Lead]);
        let leader = app.world.projects[0].leader;
        assert_eq!(app.choice_at_hand('1'), Some(Choice::StartLead));
        app.on_key(key(KeyCode::Char('1')));
        let said = app.world.journal.iter().rev().find_map(|e| match &e.event {
            Event::YouSaid { to, text } => Some((*to, text.clone())),
            _ => None,
        });
        assert_eq!(said, Some((leader, LEAD_AGAIN.to_string())));
    }

    #[test]
    fn lonca_down_looks_again_or_opens_its_window_and_enter_opens_what_stopped() {
        let mut app = stopped(&[Halt::Store]);
        app.on_key(key(KeyCode::Char('1')));
        assert_eq!(flashed(&app).as_deref(), Some("asking Lonca again"));
        app.on_key(key(KeyCode::Char('2')));
        assert!(app.lonca_win.is_some());
        let mut app = stopped(&[Halt::Store]);
        app.on_key(key(KeyCode::Enter));
        assert!(app.lonca_win.is_some());
        let mut app = stopped(&[Halt::Lead]);
        app.on_key(key(KeyCode::Enter));
        assert_eq!(app.zoom, Zoom::Focus(app.world.projects[0].leader));
    }

    #[test]
    fn a_leader_whose_session_ended_is_its_team_s_stop_not_a_stuck_row() {
        let app = stopped(&[Halt::Lead]);
        let leader = app.world.projects[0].leader;
        let got = rows(&app);
        assert_eq!(got[0], (HomeGroup::Stopped, HomeRow::Stop(0, Halt::Lead)));
        assert!(
            !got.iter().any(|r| r.1 == HomeRow::Agent(leader)),
            "{got:?}"
        );
        // Without the stop, it is stuck as before.
        let mut app = app;
        app.world.halts.stops.clear();
        assert!(rows(&app).contains(&(HomeGroup::Stuck, HomeRow::Agent(leader))));
    }

    #[test]
    fn the_usage_limit_s_digit_tries_the_cut_turns_now_and_enter_opens_the_head() {
        let mut app = stopped(&[Halt::Limit]);
        assert_eq!(
            rows(&app)[0],
            (HomeGroup::Stopped, HomeRow::Stop(0, Halt::Limit))
        );
        assert!(app.world.limit_holds());
        assert_eq!(app.choice_at_hand('1'), Some(Choice::TryNow));
        assert_eq!(app.choice_at_hand('2'), None);
        app.on_key(key(KeyCode::Char('1')));
        assert!(!app.world.limit_holds());
        assert_eq!(
            app.world.agents[1].retry.map(|r| r.at),
            Some(app.world.tick)
        );
        assert_eq!(
            flashed(&app).as_deref(),
            Some("trying the turns the usage limit cut again")
        );
        let mut app = stopped(&[Halt::Limit]);
        app.on_key(key(KeyCode::Enter));
        assert_eq!(app.zoom, Zoom::Focus(app.world.projects[0].leader));
    }

    #[test]
    fn a_hung_head_s_digit_ends_its_turn_and_a_failed_one_s_tells_it_to_go_on() {
        let mut app = stopped(&[Halt::Hung]);
        let leader = app.world.projects[0].leader;
        assert_eq!(
            rows(&app)[0],
            (HomeGroup::Stopped, HomeRow::Stop(0, Halt::Hung))
        );
        assert_eq!(app.choice_at_hand('1'), Some(Choice::EndTurn));
        app.on_key(key(KeyCode::Char('1')));
        let said = |app: &App| {
            app.world.journal.iter().rev().find_map(|e| match &e.event {
                Event::YouSaid { to, text } => Some((*to, text.clone())),
                _ => None,
            })
        };
        assert_eq!(
            said(&app),
            Some((leader, crate::halt::HUNG_AGAIN.to_string()))
        );
        let mut app = stopped(&[Halt::Failed]);
        assert_eq!(
            rows(&app)[0],
            (HomeGroup::Stopped, HomeRow::Stop(0, Halt::Failed))
        );
        assert_eq!(app.choice_at_hand('1'), Some(Choice::Again));
        app.on_key(key(KeyCode::Char('1')));
        assert_eq!(said(&app), Some((leader, FAILED_AGAIN.to_string())));
        // A spent pot refuses the hung turn's end as it refuses a word.
        let mut app = stopped(&[Halt::Hung]);
        app.world.budget = app.world.spent();
        app.on_key(key(KeyCode::Char('1')));
        assert_eq!(said(&app), None);
        assert!(
            flashed(&app).is_some_and(|f| f.contains("pot")),
            "{:?}",
            flashed(&app)
        );
    }

    #[test]
    fn a_solo_whose_session_ended_is_started_with_a_solo_s_words() {
        let mut app = stopped(&[Halt::Lead]);
        let head = app.world.projects[0].leader;
        app.world.agents[head].role = Role::Solo;
        app.on_key(key(KeyCode::Char('1')));
        let said = app.world.journal.iter().rev().find_map(|e| match &e.event {
            Event::YouSaid { to, text } => Some((*to, text.clone())),
            _ => None,
        });
        assert_eq!(said, Some((head, SOLO_AGAIN.to_string())));
    }

    #[test]
    fn a_stop_counts_among_what_is_to_decide_and_makes_the_tab_stuck() {
        let app = stopped(&[Halt::Woken, Halt::Store]);
        assert_eq!(app.world.to_decide(), 2);
        assert_eq!(app.progress(), crate::screen::Progress::Stuck);
        assert_eq!(quiet().world.to_decide(), 0);
    }
}
