//! A worker's block that its leader let lie (record 143): the world
//! finds them (`crate::blocked`), this tells the leader, with its next
//! prompt, that the person has it now.

use super::*;

impl Live {
    /// A turn of agent `id` ended with no prompt left to it (`stopped`:
    /// its session ended): the blocks it was to solve and did not come
    /// to the person, and a leader still there hears so with its next
    /// prompt, which this does not start.
    pub(super) fn unsolved(&mut self, w: &mut World, id: AgentId, stopped: bool) {
        for worker in w.blocks_unsolved(id, stopped) {
            let said = unsolved_note(w, worker, stopped);
            w.agents[id].trace(w.tick, format!("told: {said}"));
            if !stopped {
                self.notes[id].push(said);
            }
        }
    }

    /// The blocks older than `:health stall` allows come to the person;
    /// their leader hears so with its next prompt.
    pub(super) fn look_blocks(&mut self, w: &mut World) {
        for worker in w.look_blocks() {
            let Some(leader) = w.agents[worker].block.as_ref().map(|b| b.leader) else {
                continue;
            };
            let name = &w.agents[worker].name;
            let said = format!(
                "{name}'s blocked waited too long and is before the person now. Leave it \
                 to them unless you have the way on: your word to {name} still ends it."
            );
            w.agents[leader].trace(w.tick, format!("told: {said}"));
            self.notes[leader].push(said);
        }
    }

    /// The workers whose session ended or whose turn failed just now
    /// (record 146): their leader hears it, and what it may do, with a
    /// prompt that comes now.
    pub(super) fn look_stops(&mut self, w: &mut World) {
        for worker in std::mem::take(&mut w.stops) {
            let Some(b) = w.agents[worker].block.as_ref() else {
                continue;
            };
            let leader = b.leader;
            let said = stop_note(w, worker, b);
            w.agents[leader].trace(w.tick, format!("told: {said}"));
            self.notes[leader].push(said);
            self.lost[leader] = true;
        }
    }
}

/// What a leader is told of worker `worker`'s stop `b`: what happened,
/// and the ways on it has.
fn stop_note(w: &World, worker: AgentId, b: &crate::blocked::Block) -> String {
    use crate::blocked::What;
    let name = &w.agents[worker].name;
    let on = b.card.map_or(String::new(), |c| {
        format!(" on #{} {}", c + 1, w.cards[c].title)
    });
    let n = b.msg;
    let ask = format!(
        "or ask the person (an ask to you with re: {n}, what you tried, and the risk if \
         there is one)"
    );
    match b.what {
        What::Failed => format!(
            "{name}'s turn failed{on}: {}. Its session lives. It is yours to solve (stop \
             {n}): tell {name} how to go on, hand its card to another worker (assign with \
             card), {ask}.",
            b.said
        ),
        _ if w.agents[worker].revived => format!(
            "{name}'s session ended again{on} after you started it: {}. It is not started \
             a second time. It is yours to solve (stop {n}): hand its card to another \
             worker (assign with card), {ask}.",
            b.said
        ),
        _ => format!(
            "{name}'s session ended{on}: {}. It is yours to solve (stop {n}): your next \
             message to {name} starts it again where it left off, once; or hand its card \
             to another worker (assign with card), {ask}.",
            b.said
        ),
    }
}

/// What a leader is told of worker `worker`'s block it did not solve.
fn unsolved_note(w: &World, worker: AgentId, stopped: bool) -> String {
    let name = &w.agents[worker].name;
    if stopped {
        return format!("{name}'s blocked is before the person: its leader stopped");
    }
    format!(
        "{name}'s blocked is before the person now: your turn ended with neither a word \
         to {name} nor a question to the person about it. Leave it to them unless you \
         have the way on: your word to {name} still ends it."
    )
}

#[cfg(test)]
mod tests {
    use super::testing::*;
    use super::*;
    use crate::blocked::{Held, Stage, Unsolved, What};
    use serde_json::json;

    /// A leader (0) and two idle workers (1, 2), no sessions, no git.
    fn idle_team() -> (World, Live) {
        let mut w = team(2);
        w.agents[1].status = Status::Idle;
        w.agents[2].status = Status::Idle;
        let mut live = idle_live(3);
        live.places[0].git = false;
        (w, live)
    }

    fn blocked(live: &mut Live, w: &mut World) {
        let said = msg("lead", "blocked", "No clock to fake.\nI own only model.rs.");
        sent(live, w, 1, &said).unwrap();
    }

    fn open(w: &World) -> Vec<(InboxKind, AgentId)> {
        (w.open_inbox().into_iter())
            .map(|i| (w.inbox[i].kind, w.inbox[i].agent))
            .collect()
    }

    fn with(mut m: Value, key: &str, value: Value) -> Value {
        m[key] = value;
        m
    }

    #[test]
    fn a_worker_s_blocked_is_its_leader_s_and_the_leader_s_word_ends_it() {
        let (mut w, mut live) = idle_team();
        // To a teammate it is mail like any other.
        sent(
            &mut live,
            &mut w,
            1,
            &msg("claude-2", "blocked", "You hold it."),
        )
        .unwrap();
        assert_eq!(w.agents[1].block, None);
        blocked(&mut live, &mut w);
        let b = w.agents[1].block.clone().unwrap();
        assert_eq!((b.msg, b.leader, b.read), (2, 0, false));
        assert_eq!(b.said, "No clock to fake.\nI own only model.rs.");
        // The leader's turn takes it: its to solve.
        live.step(&mut w);
        assert_eq!(live.turns[0], 1);
        assert!(w.agents[1].block.as_ref().unwrap().read);
        // What does not wake is no way on.
        sent(&mut live, &mut w, 0, &msg("claude-1", "inform", "Looking.")).unwrap();
        assert!(w.agents[1].block.is_some());
        // A teammate's word is not its leader's.
        sent(
            &mut live,
            &mut w,
            2,
            &msg("claude-1", "answer", "Try this."),
        )
        .unwrap();
        assert!(w.agents[1].block.is_some());
        let way = with(
            msg("claude-1", "answer", "World::at is there."),
            "re",
            json!(2),
        );
        sent(&mut live, &mut w, 0, &way).unwrap();
        assert_eq!(w.agents[1].block, None);
        let said = w.journal.iter().any(|e| {
            e.event
                == Event::Unblocked {
                    agent: 1,
                    by: 0,
                    text: "World::at is there.".into(),
                    minutes: 0,
                    what: What::Said,
                }
        });
        assert!(said, "{:?}", w.journal);
        assert!(
            w.agents[1]
                .trace
                .iter()
                .any(|l| l.ends_with("unblocked by message 5")),
            "{:?}",
            w.agents[1].trace
        );
        finish(&mut live, &mut w, 0, "Told it.");
        assert!(open(&w).iter().all(|(k, _)| *k != InboxKind::Blocked));
        assert!(live.notes[0].is_empty());
    }

    #[test]
    fn a_turn_that_ends_with_a_block_it_read_and_no_word_leaves_it_to_the_person() {
        let (mut w, mut live) = idle_team();
        // The turn that runs when the block comes has not read it.
        live.turns[0] = 1;
        blocked(&mut live, &mut w);
        finish(&mut live, &mut w, 0, "Something else.");
        assert_eq!(w.block_held(1), Some(Held::Leader));
        assert!(w.agents[1].block.as_ref().unwrap().read, "delivered since");
        assert_eq!(live.turns[0], 1);
        assert!(live.notes[0].is_empty());
        // The turn that read it ends with no word.
        finish(&mut live, &mut w, 0, "Nothing to add.");
        assert_eq!(w.block_held(1), Some(Held::Yours));
        assert_eq!(
            w.agents[1].block.as_ref().unwrap().stage,
            Stage::Yours(Unsolved::TurnEnded)
        );
        assert!(open(&w).contains(&(InboxKind::Blocked, 1)));
        assert_eq!(live.notes[0], [unsolved_note(&w, 1, false)]);
        assert_eq!(live.turns[0], 0, "told with its next prompt, not woken");
        // The leader's word still ends it, and is not held with the item.
        let got = sent(
            &mut live,
            &mut w,
            0,
            &msg("claude-1", "answer", "Use World::at."),
        );
        assert!(got.unwrap().contains("claude-1 starts on it now"));
        assert_eq!(w.agents[1].block, None);
        assert!(!open(&w).contains(&(InboxKind::Blocked, 1)));
    }

    #[test]
    fn a_prompt_still_to_run_does_not_end_the_leader_s_turn() {
        let (mut w, mut live) = idle_team();
        blocked(&mut live, &mut w);
        live.step(&mut w);
        // Another prompt joined its session: the end of one is not the
        // end of its work.
        live.turns[0] = 2;
        finish(&mut live, &mut w, 0, "One down.");
        assert_eq!(w.block_held(1), Some(Held::Leader));
        finish(&mut live, &mut w, 0, "Both down.");
        assert_eq!(w.block_held(1), Some(Held::Yours));
    }

    #[test]
    fn the_leader_s_question_about_a_block_carries_what_it_tried_and_the_risk() {
        let (mut w, mut live) = idle_team();
        blocked(&mut live, &mut w);
        live.step(&mut w);
        let ask = || msg("you", "ask", "Let it into live/mod.rs?");
        let about = |tried: Value, risk: Value| {
            let m = with(ask(), "re", json!(1));
            with(with(m, "tried", tried), "risk", risk)
        };
        let refused =
            |live: &mut Live, w: &mut World, from, m: &Value| sent(live, w, from, m).unwrap_err();
        let told = with(msg("claude-1", "answer", "x"), "tried", json!("Looked."));
        assert_eq!(
            refused(&mut live, &mut w, 0, &told),
            "tried and risk go with an ask to the person about a worker's blocked (to: you, \
             re: the blocked's number)"
        );
        let risky = with(msg("you", "inform", "x"), "risk", json!("It may."));
        assert!(refused(&mut live, &mut w, 0, &risky).starts_with("tried and risk go with"));
        assert_eq!(
            refused(&mut live, &mut w, 0, &with(ask(), "re", json!(7))),
            "message 7 is no open blocked of a worker of yours: to the person, re goes only \
             on an ask about such a blocked, and their own messages have no number"
        );
        // Another agent's is not its.
        assert!(
            refused(&mut live, &mut w, 2, &with(ask(), "re", json!(1)))
                .starts_with("message 1 is no open blocked")
        );
        assert_eq!(
            refused(
                &mut live,
                &mut w,
                2,
                &with(ask(), "tried", json!("Looked."))
            ),
            "tried and risk are for an ask about a worker's blocked: name it with re, its \
             number"
        );
        assert_eq!(
            refused(&mut live, &mut w, 0, &about(json!(3), json!(null))),
            "tried must be a string"
        );
        let long = "x".repeat(crate::blocked::RISK_MAX + 1);
        assert_eq!(
            refused(&mut live, &mut w, 0, &about(json!("Looked."), json!(long))),
            "risk is longer than 120 characters"
        );
        assert_eq!(
            refused(
                &mut live,
                &mut w,
                0,
                &about(json!("a\u{1b}[2J"), json!(null))
            ),
            "tried has a control character"
        );
        assert!(open(&w).is_empty(), "a refused ask asks nothing");

        let m = with(
            about(json!(" Looked.\nNo clock. "), json!("widens #1")),
            "options",
            json!(["let it", "wait"]),
        );
        sent(&mut live, &mut w, 0, &m).unwrap();
        let (worker, b) = w.asked_block(0).unwrap();
        assert_eq!(worker, 1);
        assert_eq!(
            b.stage,
            Stage::Asked {
                tried: "Looked.\nNo clock.".into(),
                risk: Some("widens #1".into()),
            }
        );
        assert_eq!(w.block_held(1), Some(Held::Asked));
        assert!(
            w.agents[0]
                .trace
                .iter()
                .any(|l| l.ends_with("asks the person about claude-1's blocked"))
        );
        // The person has it: the turn's end brings nothing more.
        finish(&mut live, &mut w, 0, "");
        assert_eq!(open(&w), [(InboxKind::Question, 0)]);
        // They answer, the leader's next turn passes nothing on: theirs.
        live.told(&mut w, 0, "wait").unwrap();
        w.said(0, "wait");
        assert_eq!(w.block_held(1), Some(Held::Leader));
        finish(&mut live, &mut w, 0, "Noted.");
        assert!(
            open(&w).contains(&(InboxKind::Blocked, 1)),
            "{:?}",
            open(&w)
        );
    }

    #[test]
    fn a_question_with_no_number_is_the_one_read_block_s_and_empty_words_are_none() {
        let (mut w, mut live) = idle_team();
        blocked(&mut live, &mut w);
        live.step(&mut w);
        let m = with(
            with(msg("you", "ask", "Which?"), "tried", json!("  ")),
            "risk",
            json!(""),
        );
        sent(&mut live, &mut w, 0, &m).unwrap();
        assert_eq!(
            w.asked_block(0).unwrap().1.stage,
            Stage::Asked {
                tried: String::new(),
                risk: None,
            }
        );
    }

    #[test]
    fn a_leader_that_stops_leaves_its_blocks_to_the_person_and_hears_nothing() {
        let (mut w, mut live) = idle_team();
        blocked(&mut live, &mut w);
        hear(&live, 0, AgentEvent::Stopped("gone".into()));
        live.step(&mut w);
        assert_eq!(
            w.agents[1].block.as_ref().unwrap().stage,
            Stage::Yours(Unsolved::LeaderStopped)
        );
        assert!(open(&w).contains(&(InboxKind::Blocked, 1)));
        assert!(live.notes[0].is_empty());
        assert_eq!(
            unsolved_note(&w, 1, true),
            "claude-1's blocked is before the person: its leader stopped"
        );
    }

    #[test]
    fn a_block_that_waits_too_long_comes_to_the_person_and_the_leader_hears() {
        let (mut w, mut live) = idle_team();
        w.health.stall = Some(10);
        blocked(&mut live, &mut w);
        w.tick += 119;
        live.look_blocks(&mut w);
        assert!(open(&w).is_empty() && live.notes[0].is_empty());
        w.tick += 1;
        live.look_blocks(&mut w);
        assert_eq!(open(&w), [(InboxKind::Blocked, 1)]);
        assert_eq!(live.notes[0].len(), 1);
        assert!(
            live.notes[0][0].starts_with("claude-1's blocked waited too long"),
            "{:?}",
            live.notes[0]
        );
        assert!(
            w.agents[0]
                .trace
                .back()
                .unwrap()
                .contains("told: claude-1's")
        );
        live.look_blocks(&mut w);
        assert_eq!(live.notes[0].len(), 1, "once");
    }

    #[test]
    fn a_done_ends_a_block_as_it_ends_the_work() {
        for to in ["lead", "you"] {
            let (mut w, mut live) = idle_team();
            blocked(&mut live, &mut w);
            sent(&mut live, &mut w, 1, &msg(to, "done", "Found it myself.")).unwrap();
            assert_eq!(w.agents[1].block, None, "{to}");
            // Anything else it says leaves it blocked.
            blocked(&mut live, &mut w);
            sent(&mut live, &mut w, 1, &msg(to, "inform", "Still here.")).unwrap();
            assert!(w.agents[1].block.is_some(), "{to}");
        }
    }

    #[test]
    fn the_leader_is_told_to_solve_a_block_first_and_what_is_risky() {
        let w = team(1);
        let said = prompts::system_prompt(&w, 0, 4, 0, Confine::Standard);
        for part in [
            "A worker's blocked is yours to solve first",
            "Ask the person only when the way on is risky, or when you found none.",
            "deleting or overwriting work",
            "reaching outside the repository (credentials, installs, network services, money)",
            "widening what the card owns or its task",
            "dropping a requirement or skipping a test",
            "changing something the person decided",
            "re the blocked's number, tried",
        ] {
            assert!(said.contains(part), "{part}: {said}");
        }
        let worker = prompts::system_prompt(&w, 1, 4, 0, Confine::Standard);
        assert!(!worker.contains("yours to solve first"), "{worker}");
    }

    fn ended(live: &Live, id: AgentId, why: &str) {
        hear(live, id, AgentEvent::Stopped(why.into()));
    }

    fn failed(live: &Live, id: AgentId, why: &str) {
        let done = AgentEvent::TurnDone {
            ok: false,
            text: why.into(),
            cost: 5,
            denied: 0,
        };
        hear(live, id, done);
    }

    fn told(live: &Live) -> String {
        queued(live, 0).join("\n")
    }

    #[test]
    fn a_worker_whose_session_ended_is_its_leader_s_and_its_word_starts_it_once() {
        let (mut w, mut live) = idle_team();
        let card = with_card(&mut w, 1);
        ended(&live, 1, "exited with code 1: out of memory\nmore");
        live.step(&mut w);
        assert_eq!(w.agents[1].status, Status::Stopped);
        let b = w.agents[1].block.clone().expect("its leader's to solve");
        assert_eq!(
            (b.what, b.leader, b.card, b.said.as_str()),
            (
                What::Ended,
                0,
                Some(card),
                "exited with code 1: out of memory"
            )
        );
        assert!(w.blocked_with_leader(1), "not the person's yet");
        assert!(open(&w).is_empty());
        assert!(w.stops.is_empty(), "taken");
        // The leader is told now, with what it may do, and woken.
        assert_eq!(live.turns[0], 1);
        let note = told(&live);
        assert!(note.contains("claude-1's session ended on #1"), "{note}");
        assert!(note.contains(&format!("(stop {})", b.msg)), "{note}");
        assert!(
            note.contains("starts it again where it left off, once"),
            "{note}"
        );
        assert!(
            w.agents[1].block.as_ref().unwrap().read,
            "its prompt read it"
        );
        // Whoever starts it again goes on with the session it had.
        assert!(live.remembered[1]);
        // A word that does not wake starts nothing.
        let inform = sent(&mut live, &mut w, 0, &msg("claude-1", "inform", "Hm."));
        assert!(inform.unwrap_err().contains("does not start it again"));
        // A teammate's word waits for the person, as before.
        let other = sent(&mut live, &mut w, 2, &msg("claude-1", "ask", "Up?"));
        assert!(other.unwrap_err().contains("when the person talks to it"));
        // The leader's starts it, once.
        sent(&mut live, &mut w, 0, &msg("claude-1", "answer", "Go on.")).unwrap();
        assert_eq!(w.agents[1].block, None);
        assert!(w.agents[1].revived);
        assert_eq!(w.agents[1].status, Status::Idle);
        let revived = Event::Unblocked {
            agent: 1,
            by: 0,
            text: "Go on.".into(),
            minutes: 0,
            what: What::Ended,
        };
        assert_eq!(journaled(&w, &revived), 1);
        live.step(&mut w);
        assert_eq!(live.turns[1], 1, "the mail started it");
        // It stops again: its leader hears so, and may not start it.
        ended(&live, 1, "exited with code 1");
        live.step(&mut w);
        assert_eq!(w.agents[1].block.as_ref().unwrap().what, What::Ended);
        let again = sent(&mut live, &mut w, 0, &msg("claude-1", "answer", "Again."));
        assert!(again.unwrap_err().contains("not started a second time"));
        assert_eq!(w.agents[1].status, Status::Stopped);
    }

    #[test]
    fn a_turn_s_good_end_lets_its_leader_start_it_again_next_time() {
        let (mut w, mut live) = idle_team();
        w.agents[1].revived = true;
        hear(
            &live,
            1,
            AgentEvent::TurnDone {
                ok: true,
                text: "Done.".into(),
                cost: 5,
                denied: 0,
            },
        );
        live.step(&mut w);
        assert!(!w.agents[1].revived);
        assert!(!w.may_revive(1, 0), "nothing to start");
    }

    #[test]
    fn a_session_imece_stopped_for_its_settings_is_the_person_s_alone() {
        let (mut w, mut live) = idle_team();
        let why = format!(
            "{}did not run with the tools asked",
            imece_agent::claude::CHECK_STOPPED
        );
        ended(&live, 1, &why);
        live.step(&mut w);
        assert_eq!(w.agents[1].status, Status::Stopped);
        assert_eq!(w.agents[1].block, None);
        assert_eq!(live.turns[0], 0, "the leader is not woken");
        assert!(live.notes[0].is_empty());
    }

    #[test]
    fn a_stop_under_a_stopped_leader_or_of_a_leader_is_the_person_s() {
        let (mut w, mut live) = idle_team();
        ended(&live, 0, "gone");
        live.step(&mut w);
        assert_eq!(w.agents[0].block, None, "a leader has no leader");
        ended(&live, 1, "gone too");
        live.step(&mut w);
        assert_eq!(w.agents[1].block, None, "nobody runs to solve it");
        assert_eq!(w.agents[1].status, Status::Stopped);
    }

    #[test]
    fn a_failed_turn_with_nothing_to_try_again_is_its_leader_s() {
        let (mut w, mut live) = idle_team();
        failed(&live, 1, "\nerror_max_turns\nmore");
        live.step(&mut w);
        assert_eq!(w.agents[1].status, Status::Idle, "its session lives");
        let b = w.agents[1].block.clone().unwrap();
        assert_eq!((b.what, b.said.as_str()), (What::Failed, "error_max_turns"));
        let event = Event::TurnFailed {
            agent: 1,
            why: "\nerror_max_turns\nmore".into(),
        };
        assert_eq!(journaled(&w, &event), 1);
        assert!(
            !w.journal
                .iter()
                .any(|e| matches!(e.event, Event::Stopped { .. })),
            "no session ended"
        );
        let note = told(&live);
        assert!(
            note.contains("claude-1's turn failed: error_max_turns. Its session lives."),
            "{note}"
        );
        // The leader's word gets it going; it says so.
        sent(
            &mut live,
            &mut w,
            0,
            &msg("claude-1", "answer", "Split it."),
        )
        .unwrap();
        assert_eq!(w.agents[1].block, None);
        assert!(!w.agents[1].revived, "it never stopped");
        let going = Event::Unblocked {
            agent: 1,
            by: 0,
            text: "Split it.".into(),
            minutes: 0,
            what: What::Failed,
        };
        assert_eq!(journaled(&w, &going), 1);
    }

    #[test]
    fn a_stop_the_leader_let_lie_comes_to_the_person_said_as_a_stop() {
        let (mut w, mut live) = idle_team();
        ended(&live, 1, "exited with code 1");
        live.step(&mut w);
        finish(&mut live, &mut w, 0, "Looked.");
        // Beside the leader's closing words to the person.
        assert_eq!(open(&w), [(InboxKind::Blocked, 1), (InboxKind::Reply, 0)]);
        assert_eq!(w.block_held(1), Some(Held::Yours));
        assert!(w.journal.iter().any(|e| matches!(
            e.event,
            Event::StillBlocked {
                agent: 1,
                why: Unsolved::TurnEnded,
                what: What::Ended,
                ..
            }
        )));
        // The person's word starts it, as ever, and lets its leader start
        // it once again after its next stop.
        w.agents[1].revived = true;
        w.said(1, "Go on.");
        assert!(!w.agents[1].revived);
        assert_eq!(w.agents[1].block, None);
        assert_eq!(open(&w), [(InboxKind::Reply, 0)]);
    }
}
