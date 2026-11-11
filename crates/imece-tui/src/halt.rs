//! What stops a whole team (record 146, part 2): the pot spent, the
//! team's turns on each other's messages used up since the person last
//! spoke (`MAX_WOKEN`), the plan's usage limit reached, Lonca not
//! answering, and the head itself (the leader, or a solo): its session
//! ended, its turn hung or failed. None of these is a leader's to solve;
//! each goes straight to the person as one stop of its project, first on
//! Home, with what waits behind it and imece's own fixed choices as
//! digits (S1). imece lifts none of them by itself: it never raises the
//! pot, lets the team go on or starts the head unasked. The one it waits
//! out is the usage limit: at the time it opens, the turns it cut are
//! tried again.
//!
//! The engine (live, or the demo) says each step what it holds back and
//! why ([`Held`]), for only it knows its mail and its wakes;
//! [`World::look_halts`] makes the stops of it and of the world (the
//! store, the leader), keeps when each began, and journals each that
//! begins or ends.

use serde::{Deserialize, Serialize};

use crate::model::{
    AgentId, Event, ProjectId, Retry, Role, SECONDS_PER_TICK, Status, StoreSeen, World,
};

/// Turns an agent takes on its teammates' messages before the person has
/// to speak again: agents can not keep each other busy forever.
pub const MAX_WOKEN: u32 = 12;

/// What the leader is told when the person starts it again from a stop.
pub const LEAD_AGAIN: &str = "Your session ended and the person started it again: carry on \
                              where you left off. While it was down your workers could not \
                              reach you.";

/// What a solo is told when the person starts it again from a stop.
pub const SOLO_AGAIN: &str = "Your session ended and the person started it again: carry on \
                              where you left off.";

/// What the head is told when the person ends a turn of its that gave no
/// sign of life: its session resumes on it.
pub const HUNG_AGAIN: &str = "Your last turn gave no sign of life for a long while and the \
                              person ended it. Carry on where you left off; if a command \
                              hung, do not run it the same way again.";

/// What the head is told when the person sends it on after a failed turn.
pub const FAILED_AGAIN: &str = "Your last turn failed. Carry on where you left off.";

/// Whether `text` is one of imece's words that start the head again
/// from a stop of its own: it goes on, it plans nothing new for it.
pub fn is_again(text: &str) -> bool {
    [LEAD_AGAIN, SOLO_AGAIN, HUNG_AGAIN, FAILED_AGAIN].contains(&text)
}

/// Ticks imece waits before it tries a turn the usage limit cut again,
/// when Claude Code did not say when the limit opens: 15 minutes.
pub const LIMIT_AGAIN: u64 = 15 * 60 / SECONDS_PER_TICK;

/// The minutes a head's turn may give no sign of life before it stops
/// its team, while `:health stall` is off: that knob silences what goes
/// to a leader, never what goes to the person.
pub const HUNG_MINUTES: u16 = 10;

/// What stops a team.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
pub enum Halt {
    /// The pot is spent, and work waits on it.
    Pot,
    /// Agents of the team took [`MAX_WOKEN`] turns on messages since the
    /// person last spoke, and work waits on it.
    Woken,
    /// The plan's usage limit is reached, and a turn it cut or work
    /// waits on it.
    Limit,
    /// The project is on Lonca and Lonca does not answer.
    Store,
    /// The head's session ended: the leader's, or a solo's.
    Lead,
    /// A turn of the head's gave no sign of life for the stall's minutes.
    Hung,
    /// A turn of the head's failed with nothing imece tries again, its
    /// session alive.
    Failed,
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
    /// The usage limit, while it is reached.
    pub limit: Option<Limited>,
    /// The heads whose last turn failed, until their next one.
    pub failed: Vec<HeadFailed>,
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
    /// `:lonca login`: the store said the person is not signed in.
    Login,
    /// The head's session again, where it left off.
    StartLead,
    /// The turns the usage limit cut, tried again now.
    TryNow,
    /// The head's hung turn ended, its session resumed on it.
    EndTurn,
    /// The head told to go on after its failed turn.
    Again,
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
            Halt::Limit => &[Choice::TryNow],
            Halt::Store => &[Choice::LookAgain, Choice::Store],
            Halt::Lead => &[Choice::StartLead],
            Halt::Hung => &[Choice::EndTurn],
            Halt::Failed => &[Choice::Again],
        }
    }

    /// The choices on this stop when the store said the person is not
    /// signed in (`signed_out`): signing in comes first, since looking
    /// again finds the same (acceptance, 2026-11-11: a session file left
    /// by another daemon stopped a team that offered only that).
    pub fn choices_when(self, signed_out: bool) -> &'static [Choice] {
        match self {
            Halt::Store if signed_out => &[Choice::Login, Choice::LookAgain, Choice::Store],
            _ => self.choices(),
        }
    }
}

/// The plan's usage limit, reached: when, until when imece waits, and
/// whose turns it cut. Memory only.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Limited {
    /// The tick it was first met.
    pub since: u64,
    /// The tick imece tries the cut turns again.
    pub until: u64,
    /// Whether `until` is when Claude Code said the limit opens (else it
    /// is [`LIMIT_AGAIN`] after the last failure).
    pub opens: bool,
    /// Claude Code's words, the first line.
    pub why: String,
    /// The agents whose turn it cut, in the order they met it.
    pub hit: Vec<AgentId>,
}

/// A head's failed turn that stops its team: the tick and why.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct HeadFailed {
    pub agent: AgentId,
    pub at: u64,
    pub why: String,
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
        if let Some(l) = &self.halts.limit {
            let mut behind = behind_of(self, held, p, Halt::Limit);
            for &id in &l.hit {
                let a = &self.agents[id];
                if a.project != p || a.gone || a.status == Status::Stopped {
                    continue;
                }
                match behind.iter_mut().find(|b| b.agent == id) {
                    Some(b) if !b.waits.contains(&Waits::Retry) => b.waits.insert(0, Waits::Retry),
                    Some(_) => {}
                    None => behind.push(Behind {
                        agent: id,
                        waits: vec![Waits::Retry],
                    }),
                }
            }
            behind.sort_by_key(|b| b.agent);
            if !behind.is_empty() {
                stops.push(Stop {
                    since: l.since,
                    ..stop(Halt::Limit, l.why.clone(), behind)
                });
            }
        }
        if let Some(StoreSeen::Down(why)) = q.lonca.as_ref().map(|on| &on.seen) {
            stops.push(stop(Halt::Store, why.why().to_string(), team_of(self, p)));
        }
        let head = &self.agents[q.leader];
        if !matches!(head.role, Role::Leader | Role::Solo) || head.gone {
            return stops;
        }
        if head.status == Status::Stopped {
            stops.push(stop(Halt::Lead, head.topic.clone(), team_of(self, p)));
        } else if let Some(since) = self.hung_since(q.leader) {
            let doing = head.activity.clone().unwrap_or_default();
            stops.push(Stop {
                since,
                ..stop(Halt::Hung, doing, team_of(self, p))
            });
        } else if let Some(f) = self.halts.failed.iter().find(|f| f.agent == q.leader) {
            stops.push(Stop {
                since: f.at,
                ..stop(Halt::Failed, f.why.clone(), team_of(self, p))
            });
        }
        stops
    }

    /// Since when head `id`'s turn has given no sign of life, once that
    /// is as long as `:health stall` (or [`HUNG_MINUTES`] while it is
    /// off): its newest trace line's tick. Not while it starts, waits on
    /// the person or for a retry.
    fn hung_since(&self, id: AgentId) -> Option<u64> {
        let a = &self.agents[id];
        if a.status != Status::Working
            || a.starting
            || a.retry.is_some()
            || self.waits_on_person(id)
        {
            return None;
        }
        let minutes = self.health.stall.unwrap_or(HUNG_MINUTES);
        let ticks = u64::from(minutes) * 60 / SECONDS_PER_TICK;
        let since = a.watch.traced_at.max(a.changed_at);
        (self.tick.saturating_sub(since) >= ticks).then_some(since)
    }

    /// A turn of agent `id` failed for the plan's usage limit (`why`, in
    /// Claude Code's words): it is tried again when the limit opens, the
    /// tightest window at 100% that says when, else [`LIMIT_AGAIN`] from
    /// now; the team's stop of it begins, or goes on. Nothing waits for
    /// the person per agent: the stop is one for the team.
    pub fn limit_hit(&mut self, id: AgentId, why: &str) {
        let now = self.now();
        let opens = (self.limits.iter())
            .filter(|l| l.used >= 100)
            .filter_map(|l| l.resets_at)
            .filter(|&r| r > now)
            .max();
        let until = match opens {
            Some(r) => self.tick + (r - now).div_ceil(SECONDS_PER_TICK),
            None => self.tick + LIMIT_AGAIN,
        };
        let first = why
            .lines()
            .find(|l| !l.trim().is_empty())
            .unwrap_or_default();
        let tick = self.tick;
        let l = self.halts.limit.get_or_insert_with(|| Limited {
            since: tick,
            until,
            opens: opens.is_some(),
            why: String::new(),
            hit: Vec::new(),
        });
        l.until = until;
        l.opens = opens.is_some();
        l.why = first.to_string();
        if !l.hit.contains(&id) {
            l.hit.push(id);
        }
        let a = &mut self.agents[id];
        a.tries = 0;
        a.retry = Some(Retry {
            attempt: 0,
            at: until,
        });
        a.trace(tick, format!("usage limit reached: {first}"));
    }

    /// Whether the usage limit holds now: imece starts no turn until it
    /// opens (the person's own word still goes).
    pub fn limit_holds(&self) -> bool {
        self.halts
            .limit
            .as_ref()
            .is_some_and(|l| self.tick < l.until)
    }

    /// A turn went through: the usage limit, if it was reached, is open
    /// again, and the turns it cut go at once.
    pub fn limit_lifted(&mut self) {
        let Some(l) = self.halts.limit.take() else {
            return;
        };
        let t = self.tick;
        for id in l.hit {
            if let Some(r) = self.agents[id].retry.as_mut().filter(|r| r.attempt == 0) {
                r.at = r.at.min(t);
            }
        }
    }

    /// The person's choice on the usage limit's stop: the turns it cut are
    /// tried again now. One that fails again says when the limit opens.
    pub fn try_now(&mut self) {
        let t = self.tick;
        let Some(l) = self.halts.limit.as_mut() else {
            return;
        };
        l.until = l.until.min(t);
        for &id in &l.hit {
            if let Some(r) = self.agents[id].retry.as_mut().filter(|r| r.attempt == 0) {
                r.at = r.at.min(t);
            }
        }
    }

    /// Head `id`'s turn failed with nothing imece tries again (`why`): its
    /// team's stop until its next turn. Anyone else's is not one.
    pub fn head_failed(&mut self, id: AgentId, why: &str) {
        let p = self.agents[id].project;
        let head = self.projects.get(p).is_some_and(|q| q.leader == id);
        if !head || !matches!(self.agents[id].role, Role::Leader | Role::Solo) {
            return;
        }
        let first = why
            .lines()
            .find(|l| !l.trim().is_empty())
            .unwrap_or_default();
        self.halts.failed.retain(|f| f.agent != id);
        self.halts.failed.push(HeadFailed {
            agent: id,
            at: self.tick,
            why: first.to_string(),
        });
    }

    /// Brings the stops up to date with what the engine held back this
    /// step: a stop of an open project that began is journaled
    /// (`Event::Halted`), one that is over too (`Event::Unhalted`); one
    /// that goes on keeps when it began.
    pub fn look_halts(&mut self, held: &[Held]) {
        // A failed head that took another turn, or is gone or stopped, is
        // no failed head now.
        let failed = std::mem::take(&mut self.halts.failed);
        self.halts.failed = (failed.into_iter())
            .filter(|f| {
                let a = &self.agents[f.agent];
                !a.gone && a.status == Status::Idle && a.changed_at <= f.at
            })
            .collect();
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

    #[cfg(test)]
    fn lead_halted_by(&self, halt: Halt) -> bool {
        self.halt_of(0, halt).is_some()
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
        assert_eq!(Halt::Store.choices_when(false), Halt::Store.choices());
        assert_eq!(
            Halt::Store.choices_when(true),
            &[Choice::Login, Choice::LookAgain, Choice::Store],
            "signing in first when the store says the person is not"
        );
        for halt in [
            Halt::Pot,
            Halt::Woken,
            Halt::Limit,
            Halt::Lead,
            Halt::Hung,
            Halt::Failed,
        ] {
            assert_eq!(halt.choices_when(true), halt.choices(), "{halt:?}");
        }
        assert_eq!(Halt::Lead.choices(), &[Choice::StartLead]);
        assert_eq!(Halt::Limit.choices(), &[Choice::TryNow]);
        assert_eq!(Halt::Hung.choices(), &[Choice::EndTurn]);
        assert_eq!(Halt::Failed.choices(), &[Choice::Again]);
    }

    const LIMIT: &str = "Claude usage limit reached\nmore";

    /// The window of five hours at `used` percent, opening `seconds` from
    /// now.
    fn window(w: &mut World, used: u8, seconds: u64) {
        let resets_at = Some(w.now() + seconds);
        w.limit(crate::model::Limit {
            minutes: 300,
            used,
            resets_at,
        });
    }

    #[test]
    fn the_usage_limit_is_one_stop_of_the_team_and_its_turns_wait_for_it_to_open() {
        let mut w = team();
        w.tick = 10;
        // Full, opening in 61 s: 13 ticks of 5 s, the last one whole.
        window(&mut w, 100, 61);
        w.turn_failed(1, LIMIT, Some(crate::model::Fault::Account));
        let l = w.halts.limit.clone().expect("the limit is reached");
        assert_eq!((l.since, l.until, l.opens), (10, 23, true));
        assert_eq!(l.why, "Claude usage limit reached");
        assert_eq!(l.hit, vec![1]);
        assert_eq!(w.agents[1].retry, Some(Retry { attempt: 0, at: 23 }));
        // No item per agent and no failed turn in the journal: the stop
        // says it once for the team.
        assert!(w.inbox.is_empty());
        let failed = (w.journal.iter()).filter(|e| matches!(e.event, Event::TurnFailed { .. }));
        assert_eq!(failed.count(), 0);
        w.look_halts(&[held(2, Halt::Limit, Waits::Mail(1))]);
        let stop = w.halt_of(0, Halt::Limit).expect("the team stops");
        assert_eq!(stop.since, 10);
        assert_eq!(
            stop.behind,
            vec![
                Behind {
                    agent: 1,
                    waits: vec![Waits::Retry]
                },
                Behind {
                    agent: 2,
                    waits: vec![Waits::Mail(1)]
                },
            ]
        );
        // It holds up to the tick it opens, not on it.
        w.tick = 22;
        assert!(w.limit_holds());
        w.tick = 23;
        assert!(!w.limit_holds());
        assert!(w.retries_due().is_empty(), "it works");
        w.agents[1].status = Status::Idle;
        assert_eq!(w.retries_due(), vec![1]);
    }

    #[test]
    fn a_limit_that_does_not_say_when_it_opens_is_tried_again_later_and_a_second_hit_moves_it() {
        let mut w = team();
        w.tick = 10;
        // A window not full says nothing of this one.
        window(&mut w, 99, 60);
        w.limit_hit(1, LIMIT);
        let l = w.halts.limit.clone().unwrap();
        assert_eq!((l.until, l.opens), (10 + LIMIT_AGAIN, false));
        assert_eq!(LIMIT_AGAIN, 180);
        // A window full but already open says nothing either.
        window(&mut w, 100, 0);
        w.tick = 12;
        w.limit_hit(2, "You've hit your limit");
        let l = w.halts.limit.clone().unwrap();
        assert_eq!(
            (l.since, l.until),
            (10, 12 + LIMIT_AGAIN),
            "begun at the first"
        );
        assert_eq!(l.hit, vec![1, 2]);
        assert_eq!(l.why, "You've hit your limit");
        w.limit_hit(1, LIMIT);
        assert_eq!(w.halts.limit.as_ref().unwrap().hit, vec![1, 2], "once each");
    }

    #[test]
    fn trying_now_or_a_turn_that_went_through_lets_the_cut_turns_go_at_once() {
        let mut w = team();
        w.tick = 10;
        w.limit_hit(1, LIMIT);
        w.agents[2].retry = Some(Retry { attempt: 2, at: 99 });
        w.halts.limit.as_mut().unwrap().hit.push(2);
        w.tick = 15;
        w.try_now();
        assert!(!w.limit_holds());
        assert_eq!(w.halts.limit.as_ref().map(|l| l.until), Some(15));
        assert_eq!(w.agents[1].retry.map(|r| r.at), Some(15));
        assert_eq!(
            w.agents[2].retry.map(|r| r.at),
            Some(99),
            "an API error's own"
        );
        // The stop holds while the cut turns run, until one goes through.
        w.look_halts(&[]);
        assert!(w.halt_of(0, Halt::Limit).is_some());
        w.tick = 16;
        w.limit_hit(1, LIMIT);
        w.tick = 20;
        w.limit_lifted();
        assert_eq!(w.halts.limit, None);
        assert_eq!(w.agents[1].retry.map(|r| r.at), Some(20));
        w.look_halts(&[]);
        assert!(w.halt_of(0, Halt::Limit).is_none());
        // Lifted twice, or with nothing reached, does nothing.
        w.limit_lifted();
        w.try_now();
        assert_eq!(w.halts.limit, None);
    }

    #[test]
    fn whoever_the_limit_cut_and_left_is_not_behind_it_and_nobody_behind_is_no_stop() {
        let mut w = team();
        w.limit_hit(1, LIMIT);
        w.agents[1].gone = true;
        w.look_halts(&[]);
        assert!(w.halt_of(0, Halt::Limit).is_none());
        w.agents[1].gone = false;
        w.agents[1].status = Status::Stopped;
        w.look_halts(&[]);
        assert!(w.halt_of(0, Halt::Limit).is_none());
        w.agents[1].status = Status::Idle;
        w.look_halts(&[held(1, Halt::Limit, Waits::Retry)]);
        assert_eq!(halted(&w), vec![(0, Halt::Limit, 1)]);
        assert_eq!(
            w.halt_of(0, Halt::Limit).unwrap().behind[0].waits,
            vec![Waits::Retry]
        );
    }

    #[test]
    fn an_account_fault_that_is_no_usage_limit_waits_for_the_person_as_before() {
        let mut w = team();
        w.turn_failed(1, "Not logged in", Some(crate::model::Fault::Account));
        assert_eq!(w.halts.limit, None);
        assert_eq!(w.inbox.len(), 1);
        // And a passing one with the words is tried as a passing one.
        w.turn_failed(2, LIMIT, Some(crate::model::Fault::Passing));
        assert_eq!(w.halts.limit, None);
        assert_eq!(w.agents[2].retry.map(|r| r.attempt), Some(1));
    }

    /// The leader works and its newest line is `quiet` ticks old.
    fn quiet_lead(w: &mut World, quiet: u64) {
        w.tick = 500;
        let a = &mut w.agents[0];
        a.status = Status::Working;
        a.changed_at = 400 - quiet;
        a.watch.traced_at = 500 - quiet;
        a.activity = Some("Bash \"cargo test\"".into());
    }

    #[test]
    fn a_head_s_turn_with_no_sign_of_life_for_the_stall_s_minutes_stops_its_team() {
        let mut w = team();
        // 10 minutes: 120 ticks.
        quiet_lead(&mut w, 119);
        w.look_halts(&[]);
        assert_eq!(halted(&w), vec![]);
        quiet_lead(&mut w, 120);
        w.look_halts(&[]);
        assert_eq!(halted(&w), vec![(0, Halt::Hung, 2)]);
        let stop = w.halt_of(0, Halt::Hung).unwrap();
        assert_eq!(
            (stop.since, stop.why.as_str()),
            (380, "Bash \"cargo test\"")
        );
        // `:health stall` sets the minutes; off, the person still hears at
        // HUNG_MINUTES.
        w.health.stall = Some(5);
        quiet_lead(&mut w, 60);
        w.look_halts(&[]);
        assert!(w.lead_halted_by(Halt::Hung));
        w.health.stall = None;
        quiet_lead(&mut w, 119);
        w.look_halts(&[]);
        assert!(!w.lead_halted_by(Halt::Hung));
        quiet_lead(&mut w, 120);
        w.look_halts(&[]);
        assert!(w.lead_halted_by(Halt::Hung));
        // Its status changed later than its last line: since then.
        quiet_lead(&mut w, 200);
        w.agents[0].changed_at = 490;
        w.look_halts(&[]);
        assert!(!w.lead_halted_by(Halt::Hung));
    }

    #[test]
    fn a_quiet_turn_that_starts_waits_on_a_retry_or_on_the_person_or_is_a_worker_s_is_no_stop() {
        let cases: [&dyn Fn(&mut World); 4] = [
            &|w| w.agents[0].starting = true,
            &|w| {
                w.agents[0].retry = Some(Retry {
                    attempt: 1,
                    at: 900,
                })
            },
            &|w| w.agents[0].status = Status::WaitingHuman,
            &|w| w.agents[0].role = Role::Worker,
        ];
        for (i, f) in cases.iter().enumerate() {
            let mut w = team();
            quiet_lead(&mut w, 300);
            f(&mut w);
            w.look_halts(&[]);
            assert!(!w.lead_halted_by(Halt::Hung), "case {i}");
        }
        // A worker as quiet is its leader's to hear of (health), not a stop.
        let mut w = team();
        quiet_lead(&mut w, 300);
        w.agents[0].status = Status::Idle;
        w.agents[1].watch.traced_at = 0;
        w.look_halts(&[]);
        assert_eq!(halted(&w), vec![]);
    }

    #[test]
    fn a_head_s_failed_turn_stops_its_team_until_its_next_turn() {
        let mut w = team();
        w.tick = 40;
        w.agents[0].changed_at = 40;
        w.turn_failed(0, "error_max_turns\nmore", None);
        w.look_halts(&[]);
        assert_eq!(halted(&w), vec![(0, Halt::Failed, 2)]);
        let stop = w.halt_of(0, Halt::Failed).unwrap();
        assert_eq!((stop.since, stop.why.as_str()), (40, "error_max_turns"));
        // Failing again keeps one, the newest.
        w.tick = 41;
        w.head_failed(0, "again");
        assert_eq!(w.halts.failed.len(), 1);
        w.look_halts(&[]);
        assert_eq!(w.halt_of(0, Halt::Failed).unwrap().why, "again");
        // Its next turn: over.
        w.tick = 42;
        w.agents[0].status = Status::Working;
        w.agents[0].changed_at = 42;
        w.look_halts(&[]);
        assert!(w.halt_of(0, Halt::Failed).is_none());
        assert!(w.halts.failed.is_empty());
        // Back idle, the failure does not come back.
        w.agents[0].status = Status::Idle;
        w.look_halts(&[]);
        assert!(w.halt_of(0, Halt::Failed).is_none());
        // A worker's failed turn is its leader's, never the team's stop.
        w.turn_failed(1, "error_max_turns", None);
        assert!(w.halts.failed.is_empty());
    }

    #[test]
    fn a_failed_head_that_is_gone_or_stopped_is_no_failed_head() {
        for f in [
            (|a: &mut crate::model::Agent| a.gone = true) as fn(&mut crate::model::Agent),
            |a| a.status = Status::Stopped,
        ] {
            let mut w = team();
            w.head_failed(0, "x");
            f(&mut w.agents[0]);
            w.look_halts(&[]);
            assert!(w.halts.failed.is_empty());
            assert!(w.halt_of(0, Halt::Failed).is_none());
        }
    }

    #[test]
    fn a_solo_is_its_own_head_its_stops_go_to_the_person() {
        let mut w = team();
        w.agents.truncate(1);
        w.agents[0].role = Role::Solo;
        w.agents[0].status = Status::Stopped;
        w.look_halts(&[]);
        assert_eq!(halted(&w), vec![(0, Halt::Lead, 0)]);
        w.agents[0].status = Status::Idle;
        w.head_failed(0, "error_max_turns");
        w.look_halts(&[]);
        assert_eq!(halted(&w), vec![(0, Halt::Failed, 0)]);
        let mut w = team();
        w.agents[0].role = Role::Solo;
        quiet_lead(&mut w, 200);
        w.look_halts(&[]);
        assert!(w.lead_halted_by(Halt::Hung));
        // A helper that is no head fails no team.
        let mut w = team();
        w.agents[1].role = Role::Helper;
        w.head_failed(1, "x");
        assert!(w.halts.failed.is_empty());
    }

    #[test]
    fn the_words_that_start_a_head_again_are_known_and_nothing_else_is() {
        for t in [LEAD_AGAIN, SOLO_AGAIN, HUNG_AGAIN, FAILED_AGAIN] {
            assert!(is_again(t), "{t}");
        }
        assert!(!is_again("carry on"));
        assert!(!is_again(&format!("{LEAD_AGAIN} ")));
    }
}
