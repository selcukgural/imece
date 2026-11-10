//! What imece tells a leader of its team's health (record 128): the
//! world finds the signs (`crate::health`), this turns them into notes on
//! the leader's next prompt, woken for all but an idle worker; and mail
//! to an agent that leaves goes back to its senders, never dropped.

use super::*;
use crate::health::Sign;

/// How much of an agent's call a note to its leader quotes.
const CALL_KEPT: usize = 120;

impl Live {
    /// Looks at the team's health: each sign's leader hears it with its
    /// next prompt, which comes now for what [`Sign::wakes`]. A leader's
    /// own sign, or a solo's, is the person's alone (the journal).
    pub(super) fn look_health(&mut self, w: &mut World) {
        for (id, sign) in w.look_health() {
            let Some(leader) = w.told_of(id) else {
                continue;
            };
            self.notes[leader].push(health_note(w, id, &sign));
            if sign.wakes() {
                self.lost[leader] = true;
            }
            let name = w.agents[id].name.clone();
            w.agents[leader].trace(w.tick, format!("told: health of {name}"));
        }
    }

    /// Agent `id` leaves with mail it never read: each sender still on
    /// the team hears which of its messages went unread, with its next
    /// prompt.
    pub(super) fn unread(&mut self, w: &World, id: AgentId) {
        let name = &w.agents[id].name;
        for m in &self.mail[id] {
            if m.from == id || w.agents[m.from].gone {
                continue;
            }
            self.notes[m.from].push(format!(
                "Your {} (message {}) to {name} was not read: {name} left. Send it to \
                 someone else if it still matters.",
                m.kind.wire(),
                m.id
            ));
        }
    }
}

/// What agent `id`'s leader is told of `sign`.
fn health_note(w: &World, id: AgentId, sign: &Sign) -> String {
    let name = &w.agents[id].name;
    let card = |c: CardId| format!("#{} {}", c + 1, w.cards[c].title);
    let leader = w.told_of(id);
    match sign {
        Sign::Stalled { card: c, minutes } => format!(
            "{name} has been idle {minutes} min on {} without a done: ask it where it \
             stands, hand the card again, or withdraw it.",
            card(*c)
        ),
        Sign::Waiting {
            card: c,
            to,
            minutes,
        } if Some(*to) == leader => format!(
            "{name} has waited {minutes} min on your answer for {}: answer it.",
            card(*c)
        ),
        Sign::Waiting {
            card: c,
            to,
            minutes,
        } => format!(
            "{name} has waited {minutes} min on {}'s answer for {}: answer it yourself, \
             or get {} to.",
            w.agents[*to].name,
            card(*c),
            w.agents[*to].name
        ),
        Sign::Silent {
            card: c,
            minutes,
            doing,
        } => {
            let on = c.map_or(String::new(), |c| format!(" on {}", card(c)));
            let last = match doing.is_empty() {
                true => String::new(),
                false => format!(" (its last call: {})", cut(doing, CALL_KEPT)),
            };
            format!(
                "{name} has said nothing for {minutes} min in its turn{on}{last}: it may be \
                 stuck, or running something long. Cancel it if it is stuck, or tell the \
                 person."
            )
        }
        Sign::Free { minutes } => format!(
            "{name} has been idle {minutes} min with no card: give it one, or leave it (an \
             idle worker costs nothing)."
        ),
        Sign::Looping { call, times } => format!(
            "{name}'s call {} failed {times} times this turn: it may be going round. Cancel \
             it, or tell it another way.",
            cut(call, CALL_KEPT)
        ),
    }
}

#[cfg(test)]
mod tests {
    use super::super::testing::*;
    use super::*;

    fn send(live: &Live, id: AgentId, e: AgentEvent) {
        live.tx.send((id, live.generation[id], e)).unwrap();
    }

    #[test]
    fn a_stall_wakes_the_leader_and_an_idle_worker_waits_for_its_turn() {
        let mut w = team(2);
        let c = with_card(&mut w, 1);
        w.agents[1].status = Status::Idle;
        w.agents[2].status = Status::Idle;
        let mut live = idle_live(3);
        w.tick = 119;
        live.look_health(&mut w);
        assert!(live.notes[0].is_empty() && !live.lost[0]);
        w.tick = 120;
        live.look_health(&mut w);
        let stalled = health_note(
            &w,
            1,
            &Sign::Stalled {
                card: c,
                minutes: 10,
            },
        );
        let free = health_note(&w, 2, &Sign::Free { minutes: 10 });
        assert_eq!(live.notes[0], vec![stalled, free]);
        assert!(live.lost[0], "a stall wakes it");
        assert!(
            w.agents[0]
                .trace
                .back()
                .unwrap()
                .ends_with("told: health of claude-2")
        );
        assert!(live.notes[1].is_empty() && !live.lost[1]);
        // A free worker alone waits for the leader's next turn.
        let mut w = team(1);
        w.agents[1].status = Status::Idle;
        let mut live = idle_live(2);
        w.tick = 120;
        live.look_health(&mut w);
        assert_eq!(live.notes[0].len(), 1);
        assert!(!live.lost[0]);
        // The leader's own loop is the person's: nobody is told.
        let mut w = team(1);
        let mut live = idle_live(2);
        for _ in 0..3 {
            w.tool_failed(0, "Bash");
        }
        live.look_health(&mut w);
        assert!(live.notes.iter().all(Vec::is_empty));
        assert!(!live.lost[0]);
        assert_eq!(
            (w.journal.iter())
                .filter(|e| matches!(e.event, Event::Health { agent: 0, .. }))
                .count(),
            1
        );
    }

    #[test]
    fn a_tool_failing_on_the_session_counts_by_its_call() {
        let mut w = team(1);
        let live = idle_live(2);
        let mcp = imece_agent::mcp_tool("send");
        for _ in 0..3 {
            send(
                &live,
                1,
                AgentEvent::Tool {
                    name: mcp.clone(),
                    target: "lead".into(),
                },
            );
            send(&live, 1, AgentEvent::ToolFailed { name: mcp.clone() });
        }
        let mut live = live;
        live.step(&mut w);
        let sign = Sign::Looping {
            call: "send lead".into(),
            times: 3,
        };
        assert!(
            (w.journal.iter()).any(|e| e.event
                == Event::Health {
                    agent: 1,
                    sign: sign.clone()
                }),
            "{:?}",
            w.journal
        );
        // Its leader was told (and woken, which takes the note along).
        assert!(
            w.agents[0]
                .trace
                .iter()
                .any(|l| l.ends_with("told: health of claude-1"))
        );
        // The turn's end starts the count again.
        send(
            &live,
            1,
            AgentEvent::Tool {
                name: "Bash".into(),
                target: "ls".into(),
            },
        );
        send(
            &live,
            1,
            AgentEvent::ToolFailed {
                name: "Bash".into(),
            },
        );
        send(
            &live,
            1,
            AgentEvent::ToolFailed {
                name: "Bash".into(),
            },
        );
        send(
            &live,
            1,
            AgentEvent::TurnDone {
                ok: true,
                text: "ok".into(),
                cost: 0,
                denied: 0,
            },
        );
        send(
            &live,
            1,
            AgentEvent::Tool {
                name: "Bash".into(),
                target: "ls".into(),
            },
        );
        send(
            &live,
            1,
            AgentEvent::ToolFailed {
                name: "Bash".into(),
            },
        );
        let before = w.journal.len();
        live.step(&mut w);
        assert!(
            !(w.journal[before..].iter()).any(|e| matches!(e.event, Event::Health { .. })),
            "{:?}",
            &w.journal[before..]
        );
    }

    #[test]
    fn a_session_s_end_starts_the_count_again() {
        let mut w = team(1);
        let mut live = idle_live(2);
        w.tool_called(1, "Bash", "ls");
        w.tool_failed(1, "Bash");
        w.tool_failed(1, "Bash");
        live.end(&mut w, 1);
        w.tool_failed(1, "Bash");
        assert_eq!(w.look_health(), vec![]);
    }

    #[test]
    fn a_helper_that_leaves_with_mail_unread_tells_each_sender() {
        let mut w = team(2);
        let mut helper = Agent::new(3, 0, "h-1".into(), Role::Helper, Status::Idle, 0);
        helper.parent = Some(1);
        w.agents.push(helper);
        let mut live = idle_live(4);
        let mail = |from, kind, id| Mail {
            from,
            kind,
            body: "x".into(),
            id,
            re: None,
            paths: Vec::new(),
            checks: Vec::new(),
            closes: None,
            card: None,
            outcome: Outcome::Ok,
        };
        live.mail[3] = vec![
            mail(1, MsgKind::Ask, 7),
            mail(2, MsgKind::Inform, 8),
            mail(3, MsgKind::Inform, 9),
        ];
        w.agents[2].gone = true;
        live.release(&mut w, 3);
        assert_eq!(
            live.notes[1],
            vec![
                "Your ask (message 7) to h-1 was not read: h-1 left. Send it to someone else \
                 if it still matters."
                    .to_string()
            ]
        );
        assert!(live.notes[2].is_empty(), "gone");
        assert!(live.notes[3].is_empty(), "its own");
        assert!(live.mail[3].is_empty());
    }

    #[test]
    fn a_leader_hears_each_sign_in_words_of_its_own() {
        let mut w = team(2);
        let c = with_card(&mut w, 1);
        let note = |w: &World, sign| health_note(w, 1, &sign);
        assert_eq!(
            note(
                &w,
                Sign::Stalled {
                    card: c,
                    minutes: 10
                }
            ),
            "claude-1 has been idle 10 min on #1 fix it without a done: ask it where it \
             stands, hand the card again, or withdraw it."
        );
        assert_eq!(
            note(
                &w,
                Sign::Waiting {
                    card: c,
                    to: 0,
                    minutes: 11
                }
            ),
            "claude-1 has waited 11 min on your answer for #1 fix it: answer it."
        );
        assert_eq!(
            note(
                &w,
                Sign::Waiting {
                    card: c,
                    to: 2,
                    minutes: 12
                }
            ),
            "claude-1 has waited 12 min on claude-2's answer for #1 fix it: answer it \
             yourself, or get claude-2 to."
        );
        assert_eq!(
            note(
                &w,
                Sign::Silent {
                    card: Some(c),
                    minutes: 13,
                    doing: "Bash cargo test".into()
                }
            ),
            "claude-1 has said nothing for 13 min in its turn on #1 fix it (its last call: \
             Bash cargo test): it may be stuck, or running something long. Cancel it if it \
             is stuck, or tell the person."
        );
        assert_eq!(
            note(
                &w,
                Sign::Silent {
                    card: None,
                    minutes: 14,
                    doing: String::new()
                }
            ),
            "claude-1 has said nothing for 14 min in its turn: it may be stuck, or running \
             something long. Cancel it if it is stuck, or tell the person."
        );
        assert_eq!(
            note(&w, Sign::Free { minutes: 15 }),
            "claude-1 has been idle 15 min with no card: give it one, or leave it (an idle \
             worker costs nothing)."
        );
        assert_eq!(
            note(
                &w,
                Sign::Looping {
                    call: "Bash cargo build".into(),
                    times: 3
                }
            ),
            "claude-1's call Bash cargo build failed 3 times this turn: it may be going \
             round. Cancel it, or tell it another way."
        );
        // A long call is cut.
        let long = "x".repeat(CALL_KEPT + 5);
        let said = note(
            &w,
            Sign::Looping {
                call: long.clone(),
                times: 3,
            },
        );
        assert!(said.contains(&format!("call {} failed", &long[..CALL_KEPT])));
        assert!(!said.contains(&long[..CALL_KEPT + 1]));
    }
}
