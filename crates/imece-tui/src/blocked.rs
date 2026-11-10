//! A worker's `blocked` to its leader (record 143): the leader tries to
//! solve it itself, and the person hears of it only when the leader asks
//! them (it is risky, or the leader could not), or when the leader let it
//! lie: its turn ended with no answer to the worker, or the block grew
//! older than `:health stall` allows. Nothing here interprets a body: a
//! block is the `blocked` envelope, and what ends it is an envelope too.
//!
//! A worker whose session ended, or whose turn failed with nothing imece
//! tries again, is its leader's to solve the same way (record 146): the
//! block is imece's then ([`What`]), with a number of its own, and the
//! leader's word to a worker whose session ended starts it again, once.

use serde::{Deserialize, Serialize};

use crate::model::{
    AgentId, CardId, Event, InboxItem, InboxKind, MsgId, Role, SECONDS_PER_TICK, World,
};

/// How long the leader's own words on a block may be (what it tried, why
/// it is risky), in characters.
pub const TRIED_MAX: usize = 600;
pub const RISK_MAX: usize = 120;

/// A worker's open block: it told its leader it is stuck and nothing has
/// ended that yet. Memory only: a start forgets the ones the leader was
/// on (health's stalled notice finds such a worker again); one that came
/// to the person is an inbox item, which is saved.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Block {
    /// The `blocked` message's number.
    pub msg: MsgId,
    /// What the worker said, whole.
    pub said: String,
    /// The card it was on.
    pub card: Option<CardId>,
    pub leader: AgentId,
    /// The tick it was sent at.
    pub at: u64,
    /// A turn of the leader's has taken the message.
    pub read: bool,
    pub stage: Stage,
    pub what: What,
}

/// What a block is of.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum What {
    /// The worker said it is blocked.
    #[default]
    Said,
    /// Its session ended (record 146).
    Ended,
    /// Its turn failed and its session lives (record 146).
    Failed,
}

/// Where a block stands.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Stage {
    /// The leader's to solve.
    Leader,
    /// The leader asked the person about it: what it tried, and why it is
    /// risky (none: it could not solve it).
    Asked { tried: String, risk: Option<String> },
    /// The leader let it lie: it waits for the person.
    Yours(Unsolved),
}

/// Why a block came to the person without the leader asking.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Unsolved {
    /// The leader's turn ended with no word to the worker and no question
    /// to the person.
    TurnEnded,
    /// It is older than `:health stall` allows.
    TooLong,
    /// The leader's session stopped.
    LeaderStopped,
}

/// What a blocked worker's row says of who has its block.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Held {
    /// The leader is on it.
    Leader,
    /// The leader asked the person, who has not answered.
    Asked,
    /// It waits for the person.
    Yours,
}

/// Ticks in `minutes`.
fn ticks_in(minutes: u16) -> u64 {
    u64::from(minutes) * 60 / SECONDS_PER_TICK
}

/// Whole minutes in `ticks`.
fn minutes_in(ticks: u64) -> u16 {
    (ticks * SECONDS_PER_TICK / 60).min(u64::from(u16::MAX)) as u16
}

impl What {
    /// The one of `words` (for a block said, a session ended, a turn
    /// failed) that is this.
    pub fn of<T: Copy>(self, words: [T; 3]) -> T {
        match self {
            What::Said => words[0],
            What::Ended => words[1],
            What::Failed => words[2],
        }
    }
}

impl World {
    /// Worker `id` told `leader` it is blocked, in message `msg`: the
    /// block is the leader's now. An older one of its goes, with what
    /// waited for the person on it.
    pub fn blocked(&mut self, id: AgentId, leader: AgentId, msg: MsgId, said: &str) {
        self.close_blocked(id);
        self.agents[id].block = Some(Block {
            msg,
            said: said.to_string(),
            card: self.agents[id].card,
            leader,
            at: self.tick,
            read: false,
            stage: Stage::Leader,
            what: What::Said,
        });
    }

    /// Worker `id`'s session ended, or its turn failed (`what`), `why` in
    /// the session's words: its leader's to solve, when it has one that
    /// runs. The leader, when the block is its now; else the stop is the
    /// person's as it ever was. Live tells the leader ([`World::stops`]).
    pub fn stood(&mut self, id: AgentId, what: What, why: &str) -> Option<AgentId> {
        let a = &self.agents[id];
        let leader = self.projects[a.project].leader;
        let l = &self.agents[leader];
        if a.role != Role::Worker
            || a.gone
            || leader == id
            || l.gone
            || matches!(l.status, crate::model::Status::Stopped)
            || self.put_away(id)
        {
            return None;
        }
        self.close_blocked(id);
        let msg = self.number();
        self.agents[id].block = Some(Block {
            msg,
            said: why.to_string(),
            card: self.agents[id].card,
            leader,
            at: self.tick,
            read: false,
            stage: Stage::Leader,
            what,
        });
        self.stops.push(id);
        Some(leader)
    }

    /// Leader `leader`'s next prompt takes what imece told it: the stops
    /// of its workers are its to solve from here.
    pub fn stops_read(&mut self, leader: AgentId) {
        for a in &mut self.agents {
            if let Some(b) = a.block.as_mut().filter(|b| b.leader == leader) {
                b.read |= b.what != What::Said;
            }
        }
    }

    /// Whether `by`'s word may start worker `id`, whose session ended,
    /// again: its leader's, once for a stop (a worker that stops again
    /// before a turn of its ended well is not started a second time).
    pub fn may_revive(&self, id: AgentId, by: AgentId) -> bool {
        let a = &self.agents[id];
        !a.revived && (a.block.as_ref()).is_some_and(|b| b.leader == by && b.what == What::Ended)
    }

    /// Whether a `blocked` from `from` to `to` opens a block: a worker's,
    /// to the leader of its project.
    pub fn opens_block(&self, from: AgentId, to: AgentId) -> bool {
        let a = &self.agents[from];
        a.role == Role::Worker && self.projects[a.project].leader == to
    }

    /// A turn of the leader's took worker `id`'s message `msg`.
    pub fn block_read(&mut self, id: AgentId, msg: MsgId) {
        if let Some(b) = self.agents[id].block.as_mut().filter(|b| b.msg == msg) {
            b.read = true;
        }
    }

    /// Worker `id`'s block is over, and nothing says so: the person
    /// wrote to it, its card went elsewhere, it left.
    pub fn block_over(&mut self, id: AgentId) {
        self.agents[id].block = None;
        self.close_blocked(id);
    }

    /// Leader `by` sent worker `id` word: its block, if it has one, is
    /// solved, and the journal says so with the word's first line.
    /// Whether there was one.
    pub fn unblocked(&mut self, id: AgentId, by: AgentId, text: &str) -> bool {
        let Some(b) = self.agents[id].block.take() else {
            return false;
        };
        self.close_blocked(id);
        // Started again by its leader: not a second time for this stop.
        self.agents[id].revived |= b.what == What::Ended;
        self.note(Event::Unblocked {
            agent: id,
            by,
            text: text.lines().next().unwrap_or_default().to_string(),
            minutes: minutes_in(self.tick.saturating_sub(b.at)),
            what: b.what,
        });
        true
    }

    /// Card `c` went to `to`, or to nobody: a block on it of anyone else
    /// is over, the card's story says what became of it.
    pub fn blocks_on(&mut self, c: CardId, to: Option<AgentId>) {
        let over: Vec<AgentId> = (self.agents.iter())
            .filter(|a| Some(a.id) != to)
            .filter(|a| a.block.as_ref().is_some_and(|b| b.card == Some(c)))
            .map(|a| a.id)
            .collect();
        for id in over {
            // A worker whose session ended and whose card went elsewhere
            // waits for nothing: it sleeps until someone writes to it.
            let ended = (self.agents[id].block.as_ref()).is_some_and(|b| b.what == What::Ended);
            if ended && self.agents[id].status == crate::model::Status::Stopped {
                let t = self.tick;
                self.agents[id].set_status(crate::model::Status::Asleep, t);
            }
            self.block_over(id);
        }
    }

    fn close_blocked(&mut self, id: AgentId) {
        for i in &mut self.inbox {
            if i.kind == InboxKind::Blocked && i.agent == id {
                i.open = false;
            }
        }
    }

    /// The worker whose block message `msg` to `leader` opened and that
    /// is still open.
    pub fn block_by_message(&self, leader: AgentId, msg: MsgId) -> Option<AgentId> {
        (self.agents.iter())
            .find(|a| {
                !a.gone && (a.block.as_ref()).is_some_and(|b| b.leader == leader && b.msg == msg)
            })
            .map(|a| a.id)
    }

    /// The blocks `leader` has read and neither solved nor asked about.
    fn blocks_with(&self, leader: AgentId) -> Vec<AgentId> {
        (self.agents.iter())
            .filter(|a| !a.gone)
            .filter(|a| {
                (a.block.as_ref())
                    .is_some_and(|b| b.leader == leader && b.read && b.stage == Stage::Leader)
            })
            .map(|a| a.id)
            .collect()
    }

    /// `leader` asked the person, just now, about a block: the one
    /// message `about` opened, else the only one it has read and not
    /// settled (a leader that names none means the one it has). The
    /// question is the block's from here: it stands on the worker's card
    /// and says what the leader `tried` and the `risk`. The worker, when
    /// a block took it.
    pub fn block_asked(
        &mut self,
        leader: AgentId,
        about: Option<MsgId>,
        tried: &str,
        risk: Option<&str>,
    ) -> Option<AgentId> {
        let id = self.block_for(leader, about)?;
        let b = self.agents[id].block.as_mut()?;
        b.read = true;
        b.stage = Stage::Asked {
            tried: tried.to_string(),
            risk: risk.map(str::to_string),
        };
        let card = b.card;
        self.close_blocked(id);
        let asked = (self.inbox.iter_mut().rev())
            .find(|i| i.open && i.kind == InboxKind::Question && i.agent == leader);
        if let Some(item) = asked {
            item.card = card.or(item.card);
        }
        Some(id)
    }

    /// The worker whose block a question of `leader`'s would be about:
    /// the one message `about` opened, else the only one it has read and
    /// not settled.
    pub fn block_for(&self, leader: AgentId, about: Option<MsgId>) -> Option<AgentId> {
        match about {
            Some(msg) => self.block_by_message(leader, msg),
            None => match self.blocks_with(leader)[..] {
                [only] => Some(only),
                _ => None,
            },
        }
    }

    /// The block leader `id`'s open question to the person is about: the
    /// worker and its block.
    pub fn asked_block(&self, id: AgentId) -> Option<(AgentId, &Block)> {
        self.open_item(id, InboxKind::Question)?;
        (self.agents.iter())
            .filter(|a| !a.gone)
            .filter_map(|a| a.block.as_ref().map(|b| (a.id, b)))
            .find(|(_, b)| b.leader == id && matches!(b.stage, Stage::Asked { .. }))
    }

    /// What worker `id`'s open block is of; a `blocked` it said when it
    /// has none in memory (an item kept from an earlier run).
    pub fn block_what(&self, id: AgentId) -> What {
        self.agents[id]
            .block
            .as_ref()
            .map_or(What::Said, |b| b.what)
    }

    /// Who has worker `id`'s block, for its row: none when it has none.
    pub fn block_held(&self, id: AgentId) -> Option<Held> {
        self.agents[id].block.as_ref().map(|b| self.held_of(b))
    }

    /// Who has block `b`: the person its leader asked only while that
    /// question is open; once they answered, the leader again.
    pub fn held_of(&self, b: &Block) -> Held {
        match b.stage {
            Stage::Yours(_) => Held::Yours,
            Stage::Asked { .. } if self.open_item(b.leader, InboxKind::Question).is_some() => {
                Held::Asked
            }
            Stage::Leader | Stage::Asked { .. } => Held::Leader,
        }
    }

    /// Whether worker `id` is blocked and the person is not asked to do
    /// anything about it yet: its leader has it.
    pub fn blocked_with_leader(&self, id: AgentId) -> bool {
        matches!(self.block_held(id), Some(Held::Leader | Held::Asked))
    }

    /// Worker `id`'s block comes to the person, `why`: an item that
    /// waits for them, and the journal says so.
    fn block_yours(&mut self, id: AgentId, why: Unsolved) {
        let Some(b) = self.agents[id].block.as_mut() else {
            return;
        };
        b.stage = Stage::Yours(why);
        let (leader, card, what) = (b.leader, b.card, b.what);
        let first = (b.said.lines())
            .find(|l| !l.trim().is_empty())
            .unwrap_or_default()
            .to_string();
        self.close_blocked(id);
        self.inbox.push(InboxItem {
            kind: InboxKind::Blocked,
            agent: id,
            detail: first.clone(),
            researcher: None,
            open: true,
            tick: self.tick,
            card,
        });
        self.note(Event::StillBlocked {
            agent: id,
            leader,
            why,
            text: first,
            what,
        });
    }

    /// A turn of `leader`'s ended, with no prompt left to it: the blocks
    /// it had read and neither answered nor asked the person about come
    /// to the person, as do the ones it asked about and heard the answer
    /// to. `stopped`: its session ended, and every block of its that the
    /// person has not got comes. The workers they are of.
    pub fn blocks_unsolved(&mut self, leader: AgentId, stopped: bool) -> Vec<AgentId> {
        let asks = self.open_item(leader, InboxKind::Question).is_some();
        let due: Vec<AgentId> = (self.agents.iter())
            .filter(|a| !a.gone)
            .filter(|a| {
                (a.block.as_ref()).is_some_and(|b| {
                    b.leader == leader
                        && match b.stage {
                            Stage::Yours(_) => false,
                            // The person has the question: theirs to answer.
                            Stage::Asked { .. } if asks => false,
                            _ => stopped || b.read,
                        }
                })
            })
            .map(|a| a.id)
            .collect();
        let why = if stopped {
            Unsolved::LeaderStopped
        } else {
            Unsolved::TurnEnded
        };
        for &id in &due {
            self.block_yours(id, why);
        }
        due
    }

    /// The blocks older than `:health stall` allows come to the person,
    /// read by the leader or not; one the leader asked the person about
    /// waits on them as long as they take. The workers they are of.
    pub fn look_blocks(&mut self) -> Vec<AgentId> {
        let Some(stall) = self.health.stall else {
            return Vec::new();
        };
        let due: Vec<AgentId> = (self.agents.iter())
            .filter(|a| !a.gone && !self.put_away(a.id))
            .filter(|a| self.block_held(a.id) == Some(Held::Leader))
            .filter(|a| {
                let at = a.block.as_ref().map_or(self.tick, |b| b.at);
                self.tick.saturating_sub(at) >= ticks_in(stall)
            })
            .map(|a| a.id)
            .collect();
        for &id in &due {
            self.block_yours(id, Unsolved::TooLong);
        }
        due
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::live::testing::{team, with_card};

    /// A leader (0) and two workers (1, 2), the first on a card.
    fn world() -> World {
        let mut w = team(2);
        with_card(&mut w, 1);
        w
    }

    fn open(w: &World) -> Vec<(InboxKind, AgentId)> {
        (w.open_inbox().into_iter())
            .map(|i| (w.inbox[i].kind, w.inbox[i].agent))
            .collect()
    }

    #[test]
    fn only_a_worker_s_blocked_to_its_own_leader_opens_a_block() {
        let w = world();
        assert!(w.opens_block(1, 0));
        assert!(!w.opens_block(1, 2), "a teammate is no leader");
        assert!(!w.opens_block(0, 1), "a leader is no worker");
    }

    #[test]
    fn a_block_is_the_leader_s_until_its_turn_ends_without_a_word() {
        let mut w = world();
        w.tick = 7;
        w.blocked(1, 0, 4, "No clock to fake.\nI own only model.rs.");
        let b = w.agents[1].block.clone().unwrap();
        assert_eq!((b.msg, b.leader, b.at, b.read), (4, 0, 7, false));
        assert_eq!(b.card, w.agents[1].card);
        assert_eq!(b.said, "No clock to fake.\nI own only model.rs.");
        assert_eq!(w.block_held(1), Some(Held::Leader));
        assert!(w.blocked_with_leader(1));
        assert_eq!(w.block_held(2), None);
        assert!(!w.blocked_with_leader(2));

        // The turn that was running when it came did not read it.
        assert_eq!(w.blocks_unsolved(0, false), Vec::<AgentId>::new());
        w.block_read(1, 5);
        assert!(!w.agents[1].block.as_ref().unwrap().read, "another message");
        w.block_read(1, 4);
        assert!(w.agents[1].block.as_ref().unwrap().read);
        // Another leader's turn ends nothing here.
        assert_eq!(w.blocks_unsolved(2, false), Vec::<AgentId>::new());

        assert_eq!(w.blocks_unsolved(0, false), [1]);
        assert_eq!(w.block_held(1), Some(Held::Yours));
        assert!(!w.blocked_with_leader(1));
        assert_eq!(open(&w), [(InboxKind::Blocked, 1)]);
        let item = &w.inbox[w.open_inbox()[0]];
        assert_eq!(
            (item.detail.as_str(), item.card),
            ("No clock to fake.", b.card)
        );
        assert_eq!(
            w.journal.last().unwrap().event,
            Event::StillBlocked {
                agent: 1,
                leader: 0,
                why: Unsolved::TurnEnded,
                text: "No clock to fake.".into(),
                what: What::Said,
            }
        );
        // Once the person's, a turn's end brings it no second time.
        assert_eq!(w.blocks_unsolved(0, false), Vec::<AgentId>::new());
        assert_eq!(w.blocks_unsolved(0, true), Vec::<AgentId>::new());
        assert_eq!(open(&w).len(), 1);
    }

    #[test]
    fn a_leader_that_stops_leaves_every_block_of_its_to_the_person() {
        let mut w = world();
        w.blocked(1, 0, 4, "Stuck.");
        assert_eq!(w.blocks_unsolved(0, true), [1], "read or not");
        assert_eq!(
            w.agents[1].block.as_ref().unwrap().stage,
            Stage::Yours(Unsolved::LeaderStopped)
        );
    }

    #[test]
    fn the_leader_s_word_ends_a_block_and_the_journal_says_how_long_it_took() {
        let mut w = world();
        assert!(!w.unblocked(1, 0, "Nothing."), "no block, nothing said");
        assert!(w.journal.is_empty());
        w.tick = 10;
        w.blocked(1, 0, 4, "Stuck.");
        w.block_read(1, 4);
        w.blocks_unsolved(0, false);
        w.tick = 10 + 12 * 3;
        assert!(w.unblocked(1, 0, "World::at is there.\nSee model.rs."));
        assert_eq!(w.agents[1].block, None);
        assert!(open(&w).is_empty(), "what waited for the person closed");
        assert_eq!(
            w.journal.last().unwrap().event,
            Event::Unblocked {
                agent: 1,
                by: 0,
                text: "World::at is there.".into(),
                minutes: 3,
                what: What::Said,
            }
        );
    }

    #[test]
    fn a_block_that_is_over_closes_what_waited_and_says_nothing() {
        let mut w = world();
        w.blocked(1, 0, 4, "Stuck.");
        w.blocks_unsolved(0, true);
        let said = w.journal.len();
        w.block_over(1);
        assert_eq!(w.agents[1].block, None);
        assert!(open(&w).is_empty());
        assert_eq!(w.journal.len(), said);
        // A new block replaces an older one and what it left waiting.
        w.blocked(1, 0, 4, "Stuck.");
        w.blocks_unsolved(0, true);
        w.blocked(1, 0, 9, "Stuck again.");
        assert!(open(&w).is_empty());
        assert_eq!(w.block_held(1), Some(Held::Leader));
        assert_eq!(w.agents[1].block.as_ref().unwrap().msg, 9);
    }

    #[test]
    fn a_card_that_goes_elsewhere_ends_the_block_on_it_but_not_its_new_owner_s() {
        let mut w = world();
        let c = w.agents[1].card.unwrap();
        w.blocked(1, 0, 4, "Stuck.");
        w.blocked(2, 0, 5, "Stuck on nothing.");
        w.blocks_on(c, Some(1));
        assert!(w.agents[1].block.is_some(), "its own card, handed to it");
        w.blocks_on(c, Some(2));
        assert_eq!(w.agents[1].block, None);
        assert!(w.agents[2].block.is_some(), "blocked on no card");
        w.blocked(1, 0, 6, "Stuck.");
        w.blocks_on(c + 1, None);
        assert!(w.agents[1].block.is_some(), "another card");
        w.blocks_on(c, None);
        assert_eq!(w.agents[1].block, None);
    }

    #[test]
    fn a_question_about_a_block_stands_on_its_card_and_keeps_the_leader_s_words() {
        let mut w = world();
        w.blocked(1, 0, 4, "Stuck.");
        w.asked_you(0, "Let it into live/mod.rs?");
        // Not read, not named: it is not taken for the block's.
        assert_eq!(w.block_asked(0, None, "Looked.", None), None);
        assert_eq!(w.asked_block(0), None);
        assert_eq!(w.block_asked(0, Some(5), "Looked.", None), None, "no such");
        assert_eq!(w.block_asked(2, Some(4), "Looked.", None), None, "not its");
        assert_eq!(
            w.block_asked(0, Some(4), "Looked.", Some("widens #1")),
            Some(1)
        );
        let (worker, b) = w.asked_block(0).unwrap();
        assert_eq!(worker, 1);
        assert_eq!(
            b.stage,
            Stage::Asked {
                tried: "Looked.".into(),
                risk: Some("widens #1".into()),
            }
        );
        assert!(b.read, "a leader that asks has read it");
        let item = w.open_item(0, InboxKind::Question).unwrap();
        assert_eq!(item.card, w.agents[1].card);
        assert_eq!(w.block_held(1), Some(Held::Asked));
        assert!(w.blocked_with_leader(1));
        // While the person has the question, the turn's end and the
        // clock bring nothing more.
        w.tick = 10_000;
        assert_eq!(w.blocks_unsolved(0, false), Vec::<AgentId>::new());
        assert_eq!(w.look_blocks(), Vec::<AgentId>::new());
        // Answered: the leader's again, and its next turn's end counts.
        w.close_questions(0);
        assert_eq!(w.asked_block(0), None);
        assert_eq!(w.block_held(1), Some(Held::Leader));
        assert_eq!(w.blocks_unsolved(0, false), [1]);
    }

    #[test]
    fn a_question_that_names_no_block_is_the_only_read_one_s() {
        let mut w = world();
        w.blocked(1, 0, 4, "Stuck.");
        w.block_read(1, 4);
        w.asked_you(0, "Which?");
        assert_eq!(w.block_asked(0, None, "", None), Some(1));
        assert_eq!(
            w.agents[1].block.as_ref().unwrap().stage,
            Stage::Asked {
                tried: String::new(),
                risk: None,
            }
        );
        // Two read blocks: which one it means is not guessed.
        let mut w = world();
        w.blocked(1, 0, 4, "Stuck.");
        w.blocked(2, 0, 5, "Stuck too.");
        w.block_read(1, 4);
        w.block_read(2, 5);
        w.asked_you(0, "Which?");
        assert_eq!(w.block_asked(0, None, "", None), None);
        assert_eq!(w.block_by_message(0, 5), Some(2));
        w.agents[2].gone = true;
        assert_eq!(w.block_by_message(0, 5), None, "gone");
        assert_eq!(w.block_asked(0, None, "", None), Some(1), "one is left");
    }

    #[test]
    fn a_block_older_than_the_stall_time_comes_to_the_person() {
        let mut w = world();
        w.health.stall = Some(10);
        w.tick = 100;
        w.blocked(1, 0, 4, "Stuck.");
        w.tick = 100 + 119;
        assert_eq!(w.look_blocks(), Vec::<AgentId>::new(), "a tick short");
        w.tick = 100 + 120;
        w.health.stall = None;
        assert_eq!(w.look_blocks(), Vec::<AgentId>::new(), "switched off");
        w.health.stall = Some(10);
        assert_eq!(w.look_blocks(), [1], "not read, yet too long");
        assert_eq!(
            w.agents[1].block.as_ref().unwrap().stage,
            Stage::Yours(Unsolved::TooLong)
        );
        assert_eq!(open(&w), [(InboxKind::Blocked, 1)]);
        assert_eq!(w.look_blocks(), Vec::<AgentId>::new(), "once");
    }

    #[test]
    fn the_person_s_word_a_card_gone_and_leaving_end_a_block_in_silence() {
        for end in 0..4 {
            let mut w = world();
            let c = w.agents[1].card.unwrap();
            w.blocked(1, 0, 4, "Stuck.");
            w.blocks_unsolved(0, true);
            let said = w.journal.len();
            match end {
                0 => w.said(1, "Use the fake."),
                1 => w.hand(c, 2),
                2 => w.withdraw(c, 0, "not needed"),
                _ => w.leave(1),
            }
            assert_eq!(w.agents[1].block, None, "{end}");
            assert!(open(&w).is_empty(), "{end}");
            let unblocked = |e: &crate::model::Entry| matches!(e.event, Event::Unblocked { .. });
            assert!(!w.journal[said..].iter().any(unblocked), "{end}");
        }
    }

    #[test]
    fn a_blocked_worker_is_no_stalled_one_its_leader_has_heard_from_it() {
        let mut w = world();
        w.agents[1].status = crate::model::Status::Idle;
        w.agents[1].changed_at = 0;
        w.agents[2].gone = true;
        w.tick = 10_000;
        w.blocked(1, 0, 4, "Stuck.");
        w.block_asked(0, Some(4), "", None);
        assert_eq!(w.look_health(), Vec::new());
        w.agents[1].block = None;
        assert_eq!(w.look_health().len(), 1);
    }

    #[test]
    fn a_blocked_item_holds_its_worker_s_mail_and_the_person_s_word_frees_it() {
        assert!(InboxKind::Blocked.holds_mail());
        assert!(InboxKind::Blocked.closes_on_word());
        let mut w = world();
        w.blocked(1, 0, 4, "Stuck.");
        assert!(!w.waits_on_person(1), "its leader has it");
        w.blocks_unsolved(0, true);
        assert!(w.waits_on_person(1));
        assert!(w.word_frees(1));
    }

    #[test]
    fn minutes_and_ticks_are_counted_by_the_tick_s_seconds() {
        assert_eq!(ticks_in(10), 120);
        assert_eq!(minutes_in(35), 2);
        assert_eq!(minutes_in(36), 3);
        assert_eq!(minutes_in(u64::MAX / 8), u16::MAX);
    }
}
