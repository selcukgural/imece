//! Cards that wait for what never comes (record 146, part 3b): a card
//! that waits on one that failed or was withdrawn, and a card on the
//! board nobody takes. Each is its leader's to solve, as a worker's stop
//! is (`crate::blocked`): the leader is told and woken, and it solves it
//! (the card is taken, the card it waits on is handed again, it is
//! withdrawn), asks the person with choices, or lets it lie, and then the
//! card comes to the person. Nothing here interprets a body: a wait is
//! the cards' columns, owners and what they wait on.

use serde::{Deserialize, Serialize};

use crate::blocked::{Held, Stage, Unsolved};
use crate::model::{
    AgentId, CardId, Column, Event, InboxItem, InboxKind, MsgId, Role, SECONDS_PER_TICK, Status,
    World,
};

/// What a card waits for that never comes.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Why {
    /// A card it waits on, or one those wait on, failed or was
    /// withdrawn: it never begins as it is.
    After(CardId),
    /// It is on the board, nothing holds it back, and nobody took it for
    /// `:health stall`.
    Untaken,
}

/// A card's wait, its leader's to solve. Memory only: a start finds the
/// waits again from the cards (the leader is told once more); one that
/// came to the person is an inbox item, which is saved.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CardWait {
    pub card: CardId,
    pub why: Why,
    pub leader: AgentId,
    /// The wait's number, as a message's: the leader names it with `re`
    /// in an `ask` to the person about it.
    pub msg: MsgId,
    /// The tick the leader was told.
    pub at: u64,
    /// The tick the wait began: the card it waits on was through, or it
    /// was ready on the board.
    pub since: u64,
    /// A prompt of the leader's has taken it.
    pub read: bool,
    pub stage: Stage,
}

/// Ticks in `minutes`.
fn ticks_in(minutes: u16) -> u64 {
    u64::from(minutes) * 60 / SECONDS_PER_TICK
}

/// Whole minutes in `ticks`.
pub fn minutes_in(ticks: u64) -> u16 {
    (ticks * SECONDS_PER_TICK / 60).min(u64::from(u16::MAX)) as u16
}

impl World {
    /// What card `c` waits for that never comes now, and since when;
    /// none when it waits for nothing, or for what still comes.
    pub fn wait_of(&self, c: CardId) -> Option<(Why, u64)> {
        let card = &self.cards[c];
        if card.column != Column::Waiting {
            return None;
        }
        if let Some(d) = self.blocked_on(c) {
            return Some((Why::After(d), self.cards[d].finished));
        }
        // What goes to the leader alone is silenced by `stall off`.
        let stall = self.health.stall?;
        if card.owner.is_some() || !self.ready(c) {
            return None;
        }
        // On the board since it was posted, or since the last it waited on
        // was done.
        let since = (card.after.iter())
            .map(|&d| self.cards[d].finished)
            .fold(card.started, u64::max);
        (self.tick.saturating_sub(since) >= ticks_in(stall)).then_some((Why::Untaken, since))
    }

    /// Whether card `c` still waits for what `why` says.
    fn still_waits(&self, c: CardId, why: Why) -> bool {
        let card = &self.cards[c];
        card.column == Column::Waiting
            && match why {
                Why::After(d) => self.blocked_on(c) == Some(d),
                Why::Untaken => card.owner.is_none() && self.ready(c),
            }
    }

    /// The leader that is told of a wait of card `c` now: its project's,
    /// open, running, and free to take it. None while something else
    /// stops the team (record 146, part 2: that stop says it), while the
    /// leader waits on the person, and for a solo's project.
    fn told_of_card(&self, c: CardId) -> Option<AgentId> {
        let p = self.cards[c].project;
        let project = &self.projects[p];
        let leader = project.leader;
        let l = &self.agents[leader];
        let free = !project.closed
            && project.archived.is_none()
            && l.role == Role::Leader
            && !l.gone
            && !matches!(l.status, Status::Stopped)
            && !self.halts.stops.iter().any(|s| s.project == p)
            && !self.limit_holds()
            && !self.waits_on_person(leader);
        free.then_some(leader)
    }

    /// Each step: the waits that are over end (the journal says how long
    /// they were), and the cards that wait for what never comes now open
    /// one, the leader's to solve. The cards whose leader is to be told
    /// now, oldest first.
    pub fn look_card_waits(&mut self) -> Vec<CardId> {
        let mut i = 0;
        while i < self.card_waits.len() {
            let (c, why) = (self.card_waits[i].card, self.card_waits[i].why);
            let p = self.cards[c].project;
            if self.projects[p].closed {
                let cw = self.card_waits.remove(i);
                self.close_waits(cw.leader, c);
                continue;
            }
            if !self.still_waits(c, why) {
                let cw = self.card_waits.remove(i);
                self.wait_over(cw);
                continue;
            }
            i += 1;
        }
        let mut waits = vec![false; self.cards.len()];
        for w in &self.card_waits {
            waits[w.card] = true;
        }
        let mut told = Vec::new();
        for (c, waits) in waits.into_iter().enumerate() {
            if waits {
                continue;
            }
            let Some((why, since)) = self.wait_of(c) else {
                continue;
            };
            let Some(leader) = self.told_of_card(c) else {
                continue;
            };
            let msg = self.number();
            self.card_waits.push(CardWait {
                card: c,
                why,
                leader,
                msg,
                at: self.tick,
                since,
                read: false,
                stage: Stage::Leader,
            });
            self.note(Event::CardWaits {
                leader,
                card: c,
                why,
                minutes: minutes_in(self.tick.saturating_sub(since)),
            });
            told.push(c);
        }
        told
    }

    /// Wait `cw` is over: what it waited for came, or the card went on or
    /// is through. What waited for the person on it closes.
    fn wait_over(&mut self, cw: CardWait) {
        self.close_waits(cw.leader, cw.card);
        let card = &self.cards[cw.card];
        self.note(Event::CardWaitOver {
            leader: cw.leader,
            card: cw.card,
            why: cw.why,
            minutes: minutes_in(self.tick.saturating_sub(cw.at)),
            to: card.owner.filter(|_| card.column == Column::Working),
        });
    }

    /// The items about card `c`'s wait that `leader` let lie close.
    fn close_waits(&mut self, leader: AgentId, c: CardId) {
        for i in &mut self.inbox {
            if i.kind == InboxKind::Waits && i.agent == leader && i.card == Some(c) {
                i.open = false;
            }
        }
    }

    /// The wait of card `c`, if it has one.
    pub fn card_wait(&self, c: CardId) -> Option<&CardWait> {
        self.card_waits.iter().find(|w| w.card == c)
    }

    /// Who has wait `cw`: the person the leader asked only while that
    /// question is open; once they answered, the leader again.
    pub fn wait_held(&self, cw: &CardWait) -> Held {
        match cw.stage {
            Stage::Yours(_) => Held::Yours,
            Stage::Asked { .. } if self.has_question(cw.leader) => Held::Asked,
            Stage::Leader | Stage::Asked { .. } => Held::Leader,
        }
    }

    /// The cards of project `p` that wait for what never comes and that
    /// the leader has (or asked the person about), by number: the rows of
    /// Home's AT WORK.
    pub fn waits_with_leader(&self, p: crate::model::ProjectId) -> Vec<CardId> {
        let mut cards: Vec<CardId> = (self.card_waits.iter())
            .filter(|w| self.cards[w.card].project == p)
            .filter(|w| matches!(self.wait_held(w), Held::Leader | Held::Asked))
            .map(|w| w.card)
            .collect();
        cards.sort_unstable();
        cards
    }

    /// Leader `leader`'s prompt takes the waits it was told of.
    pub fn waits_read(&mut self, leader: AgentId) {
        for w in &mut self.card_waits {
            w.read |= w.leader == leader;
        }
    }

    /// Wait `i` comes to the person, `why`: an item that waits for them,
    /// on the leader, and the journal says so.
    fn wait_yours(&mut self, i: usize, why: Unsolved) {
        let cw = &mut self.card_waits[i];
        cw.stage = Stage::Yours(why);
        let (leader, c, what) = (cw.leader, cw.card, cw.why);
        self.close_waits(leader, c);
        self.inbox.push(InboxItem {
            kind: InboxKind::Waits,
            agent: leader,
            detail: self.cards[c].title.clone(),
            researcher: None,
            open: true,
            tick: self.tick,
            card: Some(c),
        });
        self.note(Event::CardStillWaits {
            leader,
            card: c,
            why: what,
            unsolved: why,
        });
    }

    /// A turn of `leader`'s ended with no prompt left to it: the waits it
    /// read and neither solved nor asked the person about come to the
    /// person, as do the ones it asked about and heard the answer to.
    /// `stopped`: its session ended, and every wait of its the person has
    /// not got comes. The cards they are of.
    pub fn waits_unsolved(&mut self, leader: AgentId, stopped: bool) -> Vec<CardId> {
        let asks = self.has_question(leader);
        let why = if stopped {
            Unsolved::LeaderStopped
        } else {
            Unsolved::TurnEnded
        };
        let mut due = Vec::new();
        for i in 0..self.card_waits.len() {
            let cw = &self.card_waits[i];
            let comes = cw.leader == leader
                && match cw.stage {
                    Stage::Yours(_) => false,
                    // The person has the question: theirs to answer.
                    Stage::Asked { .. } if asks => false,
                    _ => stopped || cw.read,
                };
            if comes {
                due.push(cw.card);
                self.wait_yours(i, why);
            }
        }
        due
    }

    /// The waits the leader has had longer than `:health stall` allows
    /// come to the person; one it asked the person about waits on them
    /// as long as they take. The cards they are of.
    pub fn waits_too_long(&mut self) -> Vec<CardId> {
        let Some(stall) = self.health.stall else {
            return Vec::new();
        };
        let mut due = Vec::new();
        for i in 0..self.card_waits.len() {
            let cw = &self.card_waits[i];
            if self.wait_held(cw) == Held::Leader
                && self.tick.saturating_sub(cw.at) >= ticks_in(stall)
            {
                due.push(cw.card);
                self.wait_yours(i, Unsolved::TooLong);
            }
        }
        due
    }

    /// `leader` asked the person, just now, about a card's wait: the one
    /// numbered `about`, else, when it names none and has no worker's
    /// block to ask about, the only wait it read and has not settled. The
    /// question stands on that card. The card, when a wait took it.
    pub fn wait_asked(
        &mut self,
        leader: AgentId,
        about: Option<MsgId>,
        tried: &str,
        risk: Option<&str>,
    ) -> Option<CardId> {
        let i = self.wait_for(leader, about)?;
        let cw = &mut self.card_waits[i];
        cw.read = true;
        cw.stage = Stage::Asked {
            tried: tried.to_string(),
            risk: risk.map(str::to_string),
        };
        let c = cw.card;
        self.close_waits(leader, c);
        let asked = (self.inbox.iter_mut().rev())
            .find(|i| i.open && i.kind == InboxKind::Question && i.agent == leader);
        if let Some(item) = asked {
            item.card = Some(c);
        }
        Some(c)
    }

    /// The wait a question of `leader`'s would be about, as an index of
    /// `card_waits`: the one numbered `about`, else, when it names none
    /// and has no worker's block to ask about, the only one it read and
    /// has not settled.
    pub fn wait_for(&self, leader: AgentId, about: Option<MsgId>) -> Option<usize> {
        match about {
            Some(msg) => (self.card_waits.iter()).position(|w| w.leader == leader && w.msg == msg),
            None => {
                if !self.blocks_with(leader).is_empty() {
                    return None;
                }
                let mut read = (self.card_waits.iter().enumerate())
                    .filter(|(_, w)| w.leader == leader && w.read && w.stage == Stage::Leader);
                match (read.next(), read.next()) {
                    (Some((i, _)), None) => Some(i),
                    _ => None,
                }
            }
        }
    }

    /// The card's wait leader `id`'s open question to the person is
    /// about.
    pub fn asked_wait(&self, id: AgentId) -> Option<&CardWait> {
        self.open_item(id, InboxKind::Question)?;
        (self.card_waits.iter()).find(|w| w.leader == id && matches!(w.stage, Stage::Asked { .. }))
    }

    /// The person wrote to leader `id`: the waits that came to them are
    /// its again, with their word, from now (it is told nothing more:
    /// the word is the way on).
    pub fn waits_said(&mut self, id: AgentId) {
        let t = self.tick;
        for w in &mut self.card_waits {
            if w.leader == id && matches!(w.stage, Stage::Yours(_)) {
                w.stage = Stage::Leader;
                w.at = t;
                w.read = false;
            }
        }
        for i in &mut self.inbox {
            if i.kind == InboxKind::Waits && i.agent == id {
                i.open = false;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::live::testing::{team, with_card};
    use crate::model::{Card, Story};

    /// A leader (0) and two idle workers (1, 2); card 0 is worker 1's.
    fn world() -> World {
        let mut w = team(2);
        with_card(&mut w, 1);
        w.agents[1].status = Status::Idle;
        w.agents[2].status = Status::Idle;
        w.agents[0].status = Status::Idle;
        w
    }

    /// A card on the board of project 0, posted at `tick`, waiting on
    /// `after`.
    fn posted(w: &mut World, tick: u64, after: Vec<CardId>) -> CardId {
        let c = w.cards.len();
        w.cards.push(Card {
            id: c,
            project: 0,
            title: format!("Card {}", c + 1),
            owner: None,
            column: Column::Waiting,
            story: Vec::<Story>::new(),
            symbols: Vec::new(),
            paths: Vec::new(),
            checks: Vec::new(),
            closes: None,
            started: tick,
            finished: 0,
            attempt: 0,
            breaker: Default::default(),
            after,
            item: Default::default(),
        });
        c
    }

    fn journal(w: &World) -> Vec<Event> {
        (w.journal.iter())
            .map(|e| e.event.clone())
            .filter(|e| {
                matches!(
                    e,
                    Event::CardWaits { .. }
                        | Event::CardWaitOver { .. }
                        | Event::CardStillWaits { .. }
                )
            })
            .collect()
    }

    fn open_waits(w: &World) -> Vec<(AgentId, Option<CardId>)> {
        (w.open_inbox().into_iter())
            .filter(|&i| w.inbox[i].kind == InboxKind::Waits)
            .map(|i| (w.inbox[i].agent, w.inbox[i].card))
            .collect()
    }

    #[test]
    fn a_card_nobody_takes_is_its_leader_s_at_the_stall_and_ends_when_taken() {
        let mut w = world();
        w.tick = 30;
        let c = posted(&mut w, 30, Vec::new());
        // Ten minutes are 120 ticks: not at 149, at 150.
        w.tick = 149;
        assert_eq!(w.look_card_waits(), Vec::<CardId>::new());
        w.tick = 150;
        let before = w.messages;
        assert_eq!(w.look_card_waits(), [c]);
        let cw = w.card_wait(c).unwrap().clone();
        assert_eq!(
            (cw.why, cw.leader, cw.msg, cw.at, cw.since, cw.read),
            (Why::Untaken, 0, before + 1, 150, 30, false)
        );
        assert_eq!(cw.stage, Stage::Leader);
        assert_eq!(w.waits_with_leader(0), [c]);
        assert_eq!(
            journal(&w),
            [Event::CardWaits {
                leader: 0,
                card: c,
                why: Why::Untaken,
                minutes: 10
            }]
        );
        // Once: the next look tells nothing more.
        w.tick = 151;
        assert_eq!(w.look_card_waits(), Vec::<CardId>::new());
        // Taken: over, and the journal says by whom, after how long.
        w.cards[c].owner = Some(2);
        w.cards[c].column = Column::Working;
        w.tick = 150 + 24;
        assert_eq!(w.look_card_waits(), Vec::<CardId>::new());
        assert!(w.card_wait(c).is_none());
        assert_eq!(
            journal(&w).last(),
            Some(&Event::CardWaitOver {
                leader: 0,
                card: c,
                why: Why::Untaken,
                minutes: 2,
                to: Some(2)
            })
        );
    }

    #[test]
    fn the_stall_counts_from_when_the_card_was_ready_and_stall_off_says_nothing() {
        let mut w = world();
        let first = posted(&mut w, 0, Vec::new());
        w.cards[first].owner = Some(2);
        w.cards[first].column = Column::Working;
        let c = posted(&mut w, 0, vec![first]);
        // Not ready: it waits on a card still at work.
        w.tick = 500;
        assert_eq!(w.look_card_waits(), Vec::<CardId>::new());
        w.cards[first].column = Column::Done;
        w.cards[first].finished = 400;
        w.tick = 519;
        assert_eq!(w.look_card_waits(), Vec::<CardId>::new());
        w.tick = 520;
        assert_eq!(w.look_card_waits(), [c]);
        assert_eq!(w.card_wait(c).unwrap().since, 400);
        // Off: none opens for a card nobody takes.
        let mut w = world();
        w.health.stall = None;
        posted(&mut w, 0, Vec::new());
        w.tick = 9_999;
        assert_eq!(w.look_card_waits(), Vec::<CardId>::new());
        // A card held for a worker waits for it, not for a taker.
        let mut w = world();
        let c = posted(&mut w, 0, Vec::new());
        w.cards[c].owner = Some(2);
        w.tick = 9_999;
        assert_eq!(w.look_card_waits(), Vec::<CardId>::new());
    }

    #[test]
    fn a_card_after_a_failed_one_is_told_at_once_and_ends_when_it_is_handed_again() {
        let mut w = world();
        w.cards[0].column = Column::Failed;
        w.cards[0].finished = 7;
        let c = posted(&mut w, 0, vec![0]);
        // Stall off does not silence it: it is told whatever the minutes.
        w.health.stall = None;
        w.tick = 8;
        assert_eq!(w.look_card_waits(), [c]);
        let cw = w.card_wait(c).unwrap();
        assert_eq!((cw.why, cw.since), (Why::After(0), 7));
        // Through another card too, the nearest that holds it back.
        let d = posted(&mut w, 0, vec![c]);
        assert_eq!(w.look_card_waits(), [d]);
        assert_eq!(w.card_wait(d).unwrap().why, Why::After(0));
        // Handed again: both are over.
        w.cards[0].column = Column::Working;
        w.tick = 20;
        w.look_card_waits();
        assert!(w.card_waits.is_empty());
        assert!(matches!(
            journal(&w).last(),
            Some(Event::CardWaitOver { to: None, .. })
        ));
        // Withdrawn, the card that waits ends its wait too.
        w.cards[0].column = Column::Withdrawn;
        assert_eq!(w.look_card_waits(), [c, d]);
        w.cards[c].column = Column::Withdrawn;
        w.cards[d].column = Column::Withdrawn;
        w.look_card_waits();
        assert!(w.card_waits.is_empty());
    }

    #[test]
    fn no_wait_opens_while_the_leader_cannot_take_it() {
        let ways: [fn(&mut World); 5] = [
            |w| w.agents[0].status = Status::Stopped,
            |w| w.agents[0].gone = true,
            |w| w.projects[0].closed = true,
            |w| w.agents[0].role = Role::Solo,
            |w| {
                w.inbox.push(InboxItem {
                    kind: InboxKind::Question,
                    agent: 0,
                    detail: "?".into(),
                    researcher: None,
                    open: true,
                    tick: 0,
                    card: None,
                })
            },
        ];
        for way in ways {
            let mut w = world();
            w.cards[0].column = Column::Failed;
            posted(&mut w, 0, vec![0]);
            way(&mut w);
            assert_eq!(w.look_card_waits(), Vec::<CardId>::new());
        }
        // A team stop says it instead.
        let mut w = world();
        w.cards[0].column = Column::Failed;
        posted(&mut w, 0, vec![0]);
        w.halts.stops.push(crate::halt::Stop {
            project: 0,
            halt: crate::halt::Halt::Pot,
            since: 0,
            behind: Vec::new(),
            why: String::new(),
        });
        assert_eq!(w.look_card_waits(), Vec::<CardId>::new());
        w.halts.stops.clear();
        assert_eq!(w.look_card_waits().len(), 1);
        // A project closed drops its waits without a word.
        w.projects[0].closed = true;
        let said = journal(&w).len();
        w.look_card_waits();
        assert!(w.card_waits.is_empty());
        assert_eq!(journal(&w).len(), said);
    }

    #[test]
    fn a_wait_the_leader_read_and_let_lie_comes_to_the_person_and_their_word_gives_it_back() {
        let mut w = world();
        w.cards[0].column = Column::Failed;
        let c = posted(&mut w, 0, vec![0]);
        w.look_card_waits();
        // Not read yet: a turn's end leaves it with the leader.
        assert_eq!(w.waits_unsolved(0, false), Vec::<CardId>::new());
        w.waits_read(0);
        assert!(w.card_wait(c).unwrap().read);
        assert_eq!(w.waits_unsolved(0, false), [c]);
        assert_eq!(
            w.card_wait(c).unwrap().stage,
            Stage::Yours(Unsolved::TurnEnded)
        );
        assert_eq!(open_waits(&w), [(0, Some(c))]);
        assert_eq!(w.waits_with_leader(0), Vec::<CardId>::new());
        assert_eq!(w.wait_held(w.card_wait(c).unwrap()), Held::Yours);
        assert!(matches!(
            journal(&w).last(),
            Some(Event::CardStillWaits {
                unsolved: Unsolved::TurnEnded,
                ..
            })
        ));
        // Once: a second turn's end brings nothing more.
        assert_eq!(w.waits_unsolved(0, false), Vec::<CardId>::new());
        // The item holds no mail: the leader works on.
        assert!(!w.waits_on_person(0));
        // The person's word gives it back to the leader, from now.
        w.tick = 40;
        w.waits_said(0);
        let cw = w.card_wait(c).unwrap();
        assert_eq!(
            (cw.stage.clone(), cw.at, cw.read),
            (Stage::Leader, 40, false)
        );
        assert!(open_waits(&w).is_empty());
        // Solved after: the item stays closed, the wait is over.
        w.cards[0].column = Column::Working;
        w.look_card_waits();
        assert!(w.card_waits.is_empty());
        // A leader that stops leaves every wait of its to the person.
        let mut w = world();
        w.cards[0].column = Column::Failed;
        let c = posted(&mut w, 0, vec![0]);
        w.look_card_waits();
        assert_eq!(w.waits_unsolved(0, true), [c]);
        assert_eq!(
            w.card_wait(c).unwrap().stage,
            Stage::Yours(Unsolved::LeaderStopped)
        );
    }

    #[test]
    fn a_wait_too_old_comes_to_the_person_and_one_asked_about_waits_on_them() {
        let mut w = world();
        w.cards[0].column = Column::Failed;
        let c = posted(&mut w, 0, vec![0]);
        w.tick = 10;
        w.look_card_waits();
        w.tick = 129;
        assert_eq!(w.waits_too_long(), Vec::<CardId>::new());
        w.tick = 130;
        assert_eq!(w.waits_too_long(), [c]);
        assert_eq!(
            w.card_wait(c).unwrap().stage,
            Stage::Yours(Unsolved::TooLong)
        );
        assert_eq!(w.waits_too_long(), Vec::<CardId>::new(), "once");
        // Off: never too old.
        let mut w = world();
        w.cards[0].column = Column::Failed;
        posted(&mut w, 0, vec![0]);
        w.look_card_waits();
        w.health.stall = None;
        w.tick = 9_999;
        assert_eq!(w.waits_too_long(), Vec::<CardId>::new());
        // Asked about: the person has it while the question is open.
        let mut w = world();
        w.cards[0].column = Column::Failed;
        let c = posted(&mut w, 0, vec![0]);
        w.look_card_waits();
        let msg = w.card_wait(c).unwrap().msg;
        w.inbox.push(InboxItem {
            kind: InboxKind::Question,
            agent: 0,
            detail: "Withdraw #2?".into(),
            researcher: None,
            open: true,
            tick: 0,
            card: None,
        });
        assert_eq!(
            w.wait_asked(0, Some(msg), "Handed #1 once", Some("loses #1")),
            Some(c)
        );
        let cw = w.card_wait(c).unwrap();
        assert_eq!(
            cw.stage,
            Stage::Asked {
                tried: "Handed #1 once".into(),
                risk: Some("loses #1".into())
            }
        );
        assert_eq!(
            w.inbox.last().unwrap().card,
            Some(c),
            "the question is the card's"
        );
        assert_eq!(w.asked_wait(0).map(|w| w.card), Some(c));
        assert_eq!(w.waits_with_leader(0), [c], "on AT WORK: lead asks you");
        assert_eq!(w.wait_held(w.card_wait(c).unwrap()), Held::Asked);
        w.tick = 9_999;
        assert_eq!(w.waits_too_long(), Vec::<CardId>::new());
        assert_eq!(w.waits_unsolved(0, false), Vec::<CardId>::new());
        // Answered: the leader's again, and a turn that ends on it
        // without solving it leaves it to the person.
        w.close_questions(0);
        assert_eq!(w.asked_wait(0), None);
        assert_eq!(w.wait_held(w.card_wait(c).unwrap()), Held::Leader);
        assert_eq!(w.waits_unsolved(0, false), [c]);
    }

    #[test]
    fn a_question_naming_no_wait_is_the_only_one_read_and_never_a_block_s() {
        let mut w = world();
        w.cards[0].column = Column::Failed;
        let c = posted(&mut w, 0, vec![0]);
        let d = posted(&mut w, 0, vec![0]);
        w.look_card_waits();
        // Not read: none.
        assert_eq!(w.wait_asked(0, None, "", None), None);
        w.waits_read(0);
        // Two read: none either.
        assert_eq!(w.wait_asked(0, None, "", None), None);
        w.card_waits.retain(|w| w.card == c);
        assert_eq!(w.wait_asked(0, None, "", None), Some(c));
        // A number that is no wait's, or another leader's: none.
        let mut w = world();
        w.cards[0].column = Column::Failed;
        posted(&mut w, 0, vec![0]);
        w.look_card_waits();
        w.waits_read(0);
        let msg = w.card_waits[0].msg;
        assert_eq!(w.wait_asked(0, Some(msg + 1), "", None), None);
        assert_eq!(w.wait_asked(1, Some(msg), "", None), None);
        // A worker's block read and unsettled is what a bare question is
        // about: the wait is not taken.
        w.blocked(1, 0, 99, "No clock");
        w.block_read(1, 99);
        assert_eq!(w.wait_asked(0, None, "", None), None);
        let _ = d;
    }
}
