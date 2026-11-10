//! What stops a whole team (record 146, part 2): the pot spent, the
//! team's turns on each other's messages used up since the person last
//! spoke (`MAX_WOKEN`), Lonca not answering, and the leader's own session
//! ended. None of these is a leader's to solve; each goes straight to the
//! person as one stop of its project, first on Home, with what waits
//! behind it and imece's own fixed choices as digits (S1). imece lifts
//! none of them by itself: it never raises the pot, lets the team go on
//! or starts the leader unasked.
//!
//! The engine (live, or the demo) says each step what it holds back and
//! why ([`Held`]), for only it knows its mail and its wakes;
//! [`World::look_halts`] makes the stops of it and of the world (the
//! store, the leader), keeps when each began, and journals each that
//! begins or ends.

use serde::{Deserialize, Serialize};

use crate::model::{AgentId, Event, ProjectId, Role, Status, StoreSeen, World};

/// Turns an agent takes on its teammates' messages before the person has
/// to speak again: agents can not keep each other busy forever.
pub const MAX_WOKEN: u32 = 12;

/// What the leader is told when the person starts it again from a stop.
pub const LEAD_AGAIN: &str = "Your session ended and the person started it again: carry on \
                              where you left off. While it was down your workers could not \
                              reach you.";

/// What stops a team.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
pub enum Halt {
    /// The pot is spent, and work waits on it.
    Pot,
    /// Agents of the team took [`MAX_WOKEN`] turns on messages since the
    /// person last spoke, and work waits on it.
    Woken,
    /// The project is on Lonca and Lonca does not answer.
    Store,
    /// The leader's session ended.
    Lead,
}

/// What one agent waits with behind a stop.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Waits {
    /// Messages for it, not delivered.
    Mail(usize),
    /// A wake imece holds (a health note, a lost helper, a stop told).
    Wake,
    /// Its retry after an API error.
    Retry,
    /// Messages of its that the bus refused.
    Refused(usize),
    /// It can not report to its leader or to the store.
    Report,
}

/// What the engine holds back this step: agent, why, what.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Held {
    pub agent: AgentId,
    pub by: Halt,
    pub waits: Waits,
}

/// An agent behind a stop, with all it waits with.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Behind {
    pub agent: AgentId,
    pub waits: Vec<Waits>,
}

/// A team's stop.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Stop {
    pub project: ProjectId,
    pub halt: Halt,
    /// The tick it began.
    pub since: u64,
    /// Why, in the words of whatever stopped (the store's, the session's);
    /// empty for the pot and the turns.
    pub why: String,
    pub behind: Vec<Behind>,
}

/// The stops, and what the engine held back the last step. Memory only:
/// a start finds them again from what it holds.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Halts {
    pub stops: Vec<Stop>,
}

/// One of imece's choices on a stop, a digit each in this order.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Choice {
    /// The pot raised by this much, in ten-thousandths of a dollar.
    Raise(u64),
    /// Nothing new starts; the row goes after the rest.
    Rest,
    /// The team may take its turns on messages again.
    GoOn,
    /// Ask Lonca again now.
    LookAgain,
    /// The Lonca window.
    Store,
    /// The leader's session again, where it left off.
    StartLead,
}

/// A dollar in the pot's units.
const DOLLAR: u64 = 10_000;

impl Halt {
    /// The choices on a stop of this kind, the one that changes least
    /// first.
    pub fn choices(self) -> &'static [Choice] {
        match self {
            Halt::Pot => &[
                Choice::Raise(50 * DOLLAR),
                Choice::Raise(20 * DOLLAR),
                Choice::Rest,
            ],
            Halt::Woken => &[Choice::GoOn],
            Halt::Store => &[Choice::LookAgain, Choice::Store],
            Halt::Lead => &[Choice::StartLead],
        }
    }
}

impl Stop {
    /// How many wait behind it.
    pub fn waiting(&self) -> usize {
        self.behind.len()
    }
}

/// What `held` holds of project `p`'s agents by `halt`, one row per agent
/// in id order.
fn behind_of(w: &World, held: &[Held], p: ProjectId, halt: Halt) -> Vec<Behind> {
    let mut behind: Vec<Behind> = Vec::new();
    for h in held.iter().filter(|h| h.by == halt) {
        if w.agents.get(h.agent).is_none_or(|a| a.project != p) {
            continue;
        }
        match behind.iter_mut().find(|b| b.agent == h.agent) {
            Some(b) if !b.waits.contains(&h.waits) => b.waits.push(h.waits),
            Some(_) => {}
            None => behind.push(Behind {
                agent: h.agent,
                waits: vec![h.waits],
            }),
        }
    }
    behind.sort_by_key(|b| b.agent);
    behind
}

/// Project `p`'s agents that are not its leader and not gone: whoever
/// can not report while the store or the leader is down.
fn team_of(w: &World, p: ProjectId) -> Vec<Behind> {
    let leader = w.projects[p].leader;
    (w.agents.iter())
        .filter(|a| a.project == p && a.id != leader && !a.gone && a.role != Role::Leader)
        .map(|a| Behind {
            agent: a.id,
            waits: vec![Waits::Report],
        })
        .collect()
}

impl World {
    /// The stops of project `p` as they stand now, from what the engine
    /// held back (`held`) and what the world says, in [`Halt`]'s order;
    /// each `since` this tick.
    fn halts_now(&self, held: &[Held], p: ProjectId) -> Vec<Stop> {
        let q = &self.projects[p];
        let stop = |halt, why: String, behind| Stop {
            project: p,
            halt,
            since: self.tick,
            why,
            behind,
        };
        let mut stops = Vec::new();
        for halt in [Halt::Pot, Halt::Woken] {
            let behind = behind_of(self, held, p, halt);
            let holds = match halt {
                Halt::Pot => self.spent() >= self.budget,
                _ => true,
            };
            if holds && !behind.is_empty() {
                stops.push(stop(halt, String::new(), behind));
            }
        }
        if let Some(StoreSeen::Down(why)) = q.lonca.as_ref().map(|on| &on.seen) {
            stops.push(stop(Halt::Store, why.why().to_string(), team_of(self, p)));
        }
        let lead = &self.agents[q.leader];
        if lead.role == Role::Leader && !lead.gone && lead.status == Status::Stopped {
            stops.push(stop(Halt::Lead, lead.topic.clone(), team_of(self, p)));
        }
        stops
    }

    /// Brings the stops up to date with what the engine held back this
    /// step: a stop of an open project that began is journaled
    /// (`Event::Halted`), one that is over too (`Event::Unhalted`); one
    /// that goes on keeps when it began.
    pub fn look_halts(&mut self, held: &[Held]) {
        let open: Vec<ProjectId> = self.open_projects().map(|q| q.id).collect();
        let mut now = Vec::new();
        for p in open {
            now.extend(self.halts_now(held, p));
        }
        let before = std::mem::take(&mut self.halts.stops);
        for s in &mut now {
            match before
                .iter()
                .find(|b| b.project == s.project && b.halt == s.halt)
            {
                Some(b) => s.since = b.since,
                None => {
                    let leader = self.projects[s.project].leader;
                    self.note(Event::Halted {
                        leader,
                        halt: s.halt,
                        behind: s.waiting(),
                        why: s.why.clone(),
                    });
                }
            }
        }
        for b in &before {
            let goes_on = now
                .iter()
                .any(|s| s.project == b.project && s.halt == b.halt);
            // A project closed takes its stops along unsaid.
            if !goes_on && self.projects.get(b.project).is_some_and(|q| !q.closed) {
                let leader = self.projects[b.project].leader;
                self.note(Event::Unhalted {
                    leader,
                    halt: b.halt,
                    ticks: self.tick.saturating_sub(b.since),
                });
            }
        }
        self.halts.stops = now;
    }

    /// What waits for the person to decide: the open items that are no
    /// reply, and every stop of a whole team.
    pub fn to_decide(&self) -> usize {
        self.open_decisions() + self.halts.stops.len()
    }

    /// The stop `halt` of project `p`, while it holds.
    pub fn halt_of(&self, p: ProjectId, halt: Halt) -> Option<&Stop> {
        (self.halts.stops.iter()).find(|s| s.project == p && s.halt == halt)
    }

    /// Whether project `p`'s leader is what stops its team: its row is
    /// that stop, not a stuck agent's.
    pub fn lead_halted(&self, p: ProjectId) -> bool {
        self.halt_of(p, Halt::Lead).is_some()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{OnLonca, StoreDown};

    fn held(agent: AgentId, by: Halt, waits: Waits) -> Held {
        Held { agent, by, waits }
    }

    /// A world of one project: a leader and two workers, the pot not
    /// spent.
    fn team() -> World {
        let mut w = crate::live::testing::team(2);
        w.budget = 100 * DOLLAR;
        w
    }

    fn halted(w: &World) -> Vec<(ProjectId, Halt, usize)> {
        (w.halts.stops.iter())
            .map(|s| (s.project, s.halt, s.waiting()))
            .collect()
    }

    #[test]
    fn a_spent_pot_stops_the_team_only_while_something_waits_behind_it() {
        let mut w = team();
        let pot = [held(1, Halt::Pot, Waits::Mail(2))];
        // Not spent: what the engine holds is not the pot's.
        w.look_halts(&pot);
        assert_eq!(halted(&w), vec![]);
        w.budget = w.spent();
        // Spent, nothing waits: no stop.
        w.look_halts(&[]);
        assert_eq!(halted(&w), vec![]);
        w.look_halts(&pot);
        assert_eq!(halted(&w), vec![(0, Halt::Pot, 1)]);
        let said = (w.journal.iter()).filter(|e| matches!(e.event, Event::Halted { .. }));
        assert_eq!(said.count(), 1);
    }

    #[test]
    fn what_waits_is_one_row_an_agent_with_all_it_waits_with_in_id_order() {
        let mut w = team();
        w.budget = w.spent();
        w.look_halts(&[
            held(2, Halt::Pot, Waits::Retry),
            held(1, Halt::Pot, Waits::Mail(3)),
            held(2, Halt::Pot, Waits::Wake),
            held(2, Halt::Pot, Waits::Wake),
            held(1, Halt::Woken, Waits::Refused(1)),
        ]);
        let pot = w.halt_of(0, Halt::Pot).expect("the pot stops it");
        assert_eq!(
            pot.behind,
            vec![
                Behind {
                    agent: 1,
                    waits: vec![Waits::Mail(3)]
                },
                Behind {
                    agent: 2,
                    waits: vec![Waits::Retry, Waits::Wake]
                },
            ]
        );
        let woken = w.halt_of(0, Halt::Woken).expect("and the turns");
        assert_eq!(woken.waiting(), 1);
    }

    #[test]
    fn a_stop_keeps_when_it_began_and_its_end_is_journaled_with_how_long() {
        let mut w = team();
        let woken = [held(1, Halt::Woken, Waits::Wake)];
        w.tick = 10;
        w.look_halts(&woken);
        w.tick = 13;
        w.look_halts(&woken);
        assert_eq!(w.halt_of(0, Halt::Woken).map(|s| s.since), Some(10));
        w.tick = 17;
        w.look_halts(&[]);
        assert_eq!(halted(&w), vec![]);
        let ended = w.journal.iter().find_map(|e| match e.event {
            Event::Unhalted { halt, ticks, .. } => Some((halt, ticks)),
            _ => None,
        });
        assert_eq!(ended, Some((Halt::Woken, 7)));
        let begun = (w.journal.iter()).filter(|e| matches!(e.event, Event::Halted { .. }));
        assert_eq!(begun.count(), 1, "said once, not each step");
    }

    #[test]
    fn the_store_down_and_the_leader_stopped_hold_the_team_behind_them() {
        let mut w = team();
        w.projects[0].lonca = Some(OnLonca {
            repo: "shop".into(),
            seen: StoreSeen::Asking,
            leases: Vec::new(),
            pending: Vec::new(),
            policy_checks: None,
        });
        w.look_halts(&[]);
        assert_eq!(halted(&w), vec![], "still asking is no stop");
        w.projects[0].lonca.as_mut().unwrap().seen = StoreSeen::Down(StoreDown::NoDaemon);
        w.agents[2].gone = true;
        w.agents[0].status = Status::Stopped;
        w.agents[0].topic = "exited with code 1".into();
        w.look_halts(&[]);
        assert_eq!(halted(&w), vec![(0, Halt::Store, 1), (0, Halt::Lead, 1)]);
        let lead = w.halt_of(0, Halt::Lead).unwrap();
        assert_eq!(lead.why, "exited with code 1");
        assert_eq!(lead.behind[0].waits, vec![Waits::Report]);
        assert_eq!(
            w.halt_of(0, Halt::Store).unwrap().why,
            StoreDown::NoDaemon.why()
        );
        assert!(w.lead_halted(0));
        assert!(!w.lead_halted(1));
    }

    #[test]
    fn a_closed_project_has_no_stop_and_takes_its_own_along_unsaid() {
        let mut w = team();
        w.agents[0].status = Status::Stopped;
        w.look_halts(&[]);
        assert_eq!(halted(&w), vec![(0, Halt::Lead, 2)]);
        w.projects[0].closed = true;
        w.look_halts(&[]);
        assert_eq!(halted(&w), vec![]);
        let ended = (w.journal.iter()).filter(|e| matches!(e.event, Event::Unhalted { .. }));
        assert_eq!(ended.count(), 0);
    }

    #[test]
    fn a_held_agent_of_another_project_is_not_this_one_s() {
        let mut w = team();
        w.budget = w.spent();
        w.look_halts(&[held(9, Halt::Pot, Waits::Wake)]);
        assert_eq!(halted(&w), vec![]);
    }

    #[test]
    fn every_stop_has_a_choice_and_the_pot_s_raise_by_their_dollars() {
        for halt in [Halt::Pot, Halt::Woken, Halt::Store, Halt::Lead] {
            assert!(!halt.choices().is_empty(), "{halt:?}");
        }
        assert_eq!(
            Halt::Pot.choices(),
            &[Choice::Raise(500_000), Choice::Raise(200_000), Choice::Rest]
        );
        assert_eq!(Halt::Woken.choices(), &[Choice::GoOn]);
        assert_eq!(Halt::Store.choices(), &[Choice::LookAgain, Choice::Store]);
        assert_eq!(Halt::Lead.choices(), &[Choice::StartLead]);
    }
}
