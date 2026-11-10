//! What live holds back for the pot, the usage limit or the team's turns
//! on messages (record 146, part 2): said to the world each step, so a stop that
//! halts the whole team is the person's at once, with who waits behind
//! it; and the wakes owed once such a stop is over.

use super::*;
use crate::halt::{HUNG_AGAIN, Halt, Held, MAX_WOKEN, Waits};

impl Live {
    /// What this step leaves undone for the pot, the usage limit or the
    /// turns: mail not delivered, wakes and retries not taken while the
    /// pot is spent or the limit holds (the pot first), a
    /// wake not taken by an agent at [`MAX_WOKEN`], and every agent whose
    /// waking message the bus refused for either since the person last
    /// spoke. Nobody gone, put away, in a turn, stopped, asleep or
    /// waiting on the person is held back by these.
    pub(super) fn held_back(&self, w: &World) -> Vec<Held> {
        let pot = self.over_budget(w);
        let by = if pot {
            Some(Halt::Pot)
        } else if w.limit_holds() {
            Some(Halt::Limit)
        } else {
            None
        };
        let mut held = Vec::new();
        for (id, a) in w.agents.iter().enumerate() {
            if a.gone || w.put_away(id) {
                continue;
            }
            let mut hold = |by, waits| {
                held.push(Held {
                    agent: id,
                    by,
                    waits,
                })
            };
            let [by_pot, by_turns] = self.held_sends[id];
            if by_pot > 0 {
                hold(Halt::Pot, Waits::Refused(by_pot as usize));
            }
            if by_turns > 0 {
                hold(Halt::Woken, Waits::Refused(by_turns as usize));
            }
            let free = self.turns[id] == 0
                && !matches!(a.status, Status::Stopped | Status::Asleep)
                && !w.waits_on_person(id);
            if !free {
                continue;
            }
            if let Some(by) = by {
                let mail = (self.mail[id].iter())
                    .filter(|m| m.kind.wakes() && !Self::waits(w, m))
                    .count();
                if mail > 0 {
                    hold(by, Waits::Mail(mail));
                }
                if self.lost[id] {
                    hold(by, Waits::Wake);
                }
                if a.retry.is_some() {
                    hold(by, Waits::Retry);
                }
            } else if self.lost[id] && self.woken[id] >= MAX_WOKEN {
                hold(Halt::Woken, Waits::Wake);
            }
        }
        held
    }

    /// Once the pot has room again, whoever's message it refused is woken
    /// to go on: the message never went.
    pub(super) fn after_halts(&mut self, w: &World) {
        if self.over_budget(w) {
            return;
        }
        for id in 0..self.held_sends.len() {
            if std::mem::take(&mut self.held_sends[id][0]) > 0 && !w.agents[id].gone {
                self.lost[id] = true;
            }
        }
    }

    /// The person's choice on a head's hung turn: its session ends, cutting
    /// the turn, and resumes on [`HUNG_AGAIN`] where it left off. Refused
    /// as the person's word is (the pot).
    pub fn end_turn(&mut self, w: &mut World, id: AgentId) -> Result<(), Refusal> {
        if self.over_budget(w) {
            return Err(Refusal::Budget);
        }
        if self.turns[id] > 0 && matches!(self.slots[id], Slot::Running(_)) {
            self.end(w, id);
            w.agents[id].trace(w.tick, "the person ended a turn that gave no sign of life");
        }
        self.told(w, id, HUNG_AGAIN)
    }

    /// The team may take its turns on messages again (the person spoke,
    /// or chose so on the stop): every count starts anew, and whoever's
    /// message the count refused is woken to go on.
    pub fn go_on(&mut self) {
        self.woken.iter_mut().for_each(|n| *n = 0);
        self.posted.iter_mut().for_each(|n| *n = 0);
        for id in 0..self.held_sends.len() {
            if std::mem::take(&mut self.held_sends[id][1]) > 0 {
                self.lost[id] = true;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::live::testing::{hear, idle_live, msg, sent, team};

    fn mail(kind: MsgKind) -> Mail {
        Mail {
            from: 0,
            kind,
            body: "go".into(),
            id: 1,
            re: None,
            paths: Vec::new(),
            checks: Vec::new(),
            closes: None,
            card: None,
            outcome: Outcome::Ok,
        }
    }

    fn ready(workers: usize) -> (Live, World) {
        let mut w = team(workers);
        for a in &mut w.agents {
            a.status = Status::Idle;
        }
        (idle_live(workers + 1), w)
    }

    fn of(held: &[Held], id: AgentId) -> Vec<(Halt, Waits)> {
        (held.iter())
            .filter(|h| h.agent == id)
            .map(|h| (h.by, h.waits))
            .collect()
    }

    #[test]
    fn a_spent_pot_holds_the_mail_the_wakes_and_the_retries_of_whoever_is_free() {
        let (mut live, mut w) = ready(3);
        live.lost[1] = true;
        w.agents[2].retry = Some(Retry { attempt: 1, at: 9 });
        live.mail[3].push(mail(MsgKind::Assign));
        live.mail[3].push(mail(MsgKind::Inform));
        // The pot has room: nothing is the pot's.
        assert_eq!(live.held_back(&w), vec![]);
        w.budget = w.spent();
        let held = live.held_back(&w);
        assert_eq!(of(&held, 1), vec![(Halt::Pot, Waits::Wake)]);
        assert_eq!(of(&held, 2), vec![(Halt::Pot, Waits::Retry)]);
        // An inform wakes nobody: only the assign waits.
        assert_eq!(of(&held, 3), vec![(Halt::Pot, Waits::Mail(1))]);
        // In a turn, stopped, asleep or gone: held by nothing of these.
        live.turns[1] = 1;
        w.agents[2].status = Status::Stopped;
        w.agents[3].gone = true;
        assert_eq!(live.held_back(&w), vec![]);
        live.turns[1] = 0;
        w.agents[1].status = Status::Asleep;
        assert_eq!(live.held_back(&w), vec![]);
    }

    #[test]
    fn the_turns_used_up_hold_a_wake_and_the_messages_refused_by_either_are_said() {
        let (mut live, w) = ready(2);
        live.lost[1] = true;
        live.woken[1] = MAX_WOKEN - 1;
        assert_eq!(live.held_back(&w), vec![], "one turn is left");
        live.woken[1] = MAX_WOKEN;
        live.held_sends[2] = [2, 3];
        let held = live.held_back(&w);
        assert_eq!(of(&held, 1), vec![(Halt::Woken, Waits::Wake)]);
        assert_eq!(
            of(&held, 2),
            vec![
                (Halt::Pot, Waits::Refused(2)),
                (Halt::Woken, Waits::Refused(3))
            ]
        );
    }

    #[test]
    fn going_on_counts_anew_and_wakes_whoever_the_count_refused() {
        let (mut live, _) = ready(2);
        live.woken = vec![MAX_WOKEN, 4, 1];
        live.posted = vec![2, 0, 1];
        live.held_sends[1] = [1, 2];
        live.go_on();
        assert_eq!(live.woken, vec![0, 0, 0]);
        assert_eq!(live.posted, vec![0, 0, 0]);
        assert_eq!(live.held_sends[1], [1, 0], "the pot's stays the pot's");
        assert_eq!(live.lost, vec![false, true, false]);
    }

    #[test]
    fn a_pot_with_room_again_wakes_whoever_it_refused_and_a_spent_one_does_not() {
        let (mut live, mut w) = ready(2);
        live.held_sends[1] = [2, 1];
        live.held_sends[2] = [1, 0];
        w.agents[2].gone = true;
        w.budget = w.spent();
        live.after_halts(&w);
        assert_eq!(live.lost, vec![false, false, false]);
        w.budget = w.spent() + 1;
        live.after_halts(&w);
        assert_eq!(live.lost, vec![false, true, false], "not the gone one");
        assert_eq!(live.held_sends[1], [0, 1]);
        assert_eq!(live.held_sends[2], [0, 0]);
    }

    #[test]
    fn a_step_with_the_pot_spent_stops_the_team_and_one_with_room_again_goes_on() {
        let (mut live, mut w) = ready(2);
        w.budget = w.spent();
        assert!(sent(&mut live, &mut w, 1, &msg("lead", "ask", "which?")).is_err());
        live.step(&mut w);
        let stop = w.halt_of(0, Halt::Pot).expect("the pot stops the team");
        assert_eq!(stop.behind.len(), 1);
        assert_eq!(stop.behind[0].agent, 1);
        assert!((w.journal.iter()).any(|e| matches!(
            e.event,
            Event::Halted {
                halt: Halt::Pot,
                ..
            }
        )));
        // The person raises it: the stop is over and the one it refused
        // is woken.
        w.budget = w.spent() + 100_000;
        live.step(&mut w);
        assert!(w.halt_of(0, Halt::Pot).is_none());
        assert_eq!(live.held_sends[1], [0, 0]);
        assert!((w.journal.iter()).any(|e| matches!(
            e.event,
            Event::Unhalted {
                halt: Halt::Pot,
                ..
            }
        )));
    }

    #[test]
    fn the_usage_limit_holds_the_mail_wakes_and_retries_of_whoever_is_free_after_the_pot() {
        let (mut live, mut w) = ready(3);
        live.lost[1] = true;
        w.agents[2].retry = Some(Retry { attempt: 1, at: 9 });
        live.mail[3].push(mail(MsgKind::Assign));
        w.limit_hit(3, "Claude usage limit reached");
        // Its own retry waits as the world says (attempt 0): live holds it
        // too, by the limit.
        let held = live.held_back(&w);
        assert_eq!(of(&held, 1), vec![(Halt::Limit, Waits::Wake)]);
        assert_eq!(of(&held, 2), vec![(Halt::Limit, Waits::Retry)]);
        assert_eq!(
            of(&held, 3),
            vec![(Halt::Limit, Waits::Mail(1)), (Halt::Limit, Waits::Retry)]
        );
        // The pot spent says it first.
        w.budget = w.spent();
        assert_eq!(of(&live.held_back(&w), 1), vec![(Halt::Pot, Waits::Wake)]);
        // Open again: nothing is the limit's.
        w.budget = w.spent() + 1;
        w.try_now();
        assert_eq!(live.held_back(&w), vec![]);
    }

    #[test]
    fn while_the_limit_holds_a_step_delivers_no_mail_wakes_nobody_and_offers_no_card() {
        let (mut live, mut w) = ready(2);
        w.limit_hit(2, "Claude usage limit reached");
        live.mail[1].push(mail(MsgKind::Assign));
        live.lost[0] = true;
        live.step(&mut w);
        assert_eq!(live.mail[1].len(), 1, "the mail waits");
        assert_eq!(live.turns, vec![0, 0, 0]);
        assert!(live.lost[0], "the wake waits");
        let stop = w.halt_of(0, Halt::Limit).expect("the team stops");
        let behind: Vec<AgentId> = stop.behind.iter().map(|b| b.agent).collect();
        assert_eq!(behind, vec![0, 1, 2]);
        // Tried now, the mail goes (its session fails to start here, but
        // the delivery was made).
        w.try_now();
        live.step(&mut w);
        assert!(live.mail[1].is_empty(), "delivered");
    }

    #[test]
    fn a_turn_that_went_through_opens_the_limit_and_lets_the_cut_turns_go() {
        let (mut live, mut w) = ready(2);
        w.limit_hit(2, "Claude usage limit reached");
        live.turns[1] = 1;
        w.agents[1].status = Status::Working;
        hear(
            &live,
            1,
            AgentEvent::TurnDone {
                ok: true,
                text: "done".into(),
                cost: 0,
                denied: 0,
            },
        );
        live.step(&mut w);
        assert_eq!(w.halts.limit, None);
        assert!(w.halt_of(0, Halt::Limit).is_none());
        // A failed turn opens nothing.
        w.limit_hit(2, "Claude usage limit reached");
        live.turns[1] = 1;
        w.agents[1].status = Status::Working;
        hear(
            &live,
            1,
            AgentEvent::TurnDone {
                ok: false,
                text: "no".into(),
                cost: 0,
                denied: 0,
            },
        );
        live.step(&mut w);
        assert!(w.halts.limit.is_some());
    }

    #[test]
    fn ending_a_hung_turn_is_refused_over_budget_and_else_tells_the_head_to_go_on() {
        let (mut live, mut w) = ready(1);
        w.budget = w.spent();
        assert!(matches!(live.end_turn(&mut w, 0), Err(Refusal::Budget)));
        w.budget = w.spent() + 100_000;
        // No session runs here: the word starts one (and it fails to start
        // in a test), nothing is ended.
        let _ = live.end_turn(&mut w, 0);
        assert!(!w.agents[0].trace.iter().any(|l| l.contains("ended a turn")));
    }

    #[test]
    fn a_send_refused_for_the_pot_or_the_turns_is_counted_for_its_sender() {
        let (mut live, mut w) = ready(2);
        live.woken[2] = MAX_WOKEN;
        let ask = msg("claude-2", "ask", "which port?");
        assert!(sent(&mut live, &mut w, 1, &ask).is_err());
        assert_eq!(live.held_sends[1], [0, 1]);
        // What wakes nobody is not refused.
        sent(&mut live, &mut w, 1, &msg("claude-2", "inform", "fyi")).unwrap();
        assert_eq!(live.held_sends[1], [0, 1]);
        w.budget = w.spent();
        assert!(sent(&mut live, &mut w, 1, &ask).is_err());
        assert_eq!(live.held_sends[1], [1, 1]);
    }
}
