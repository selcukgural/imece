//! A project's round of work, and whether it is finished (record 145):
//! the one state the screen says when every card the leader handed out is
//! through, so the person does not work it out row by row.
//!
//! A round is the cards opened since the project last had none open: a
//! card begins a new round when every earlier card of its project was
//! through before it came. Nothing is stored for it: it is read from the
//! cards as they are, so a start finds the same round.
//!
//! Two steps. The team finished ([`Finished::Team`]): no card waits or is
//! at work, nobody of the project is in a turn, and something still waits
//! for the person (a change, a question of what is next). All done
//! ([`Finished::All`]): nothing waits for the person either. A card that
//! failed or was withdrawn does not hold either back, and is always
//! counted apart.

use crate::model::{Card, CardId, Column, InboxKind, ProjectId, Role, World};

/// A finished round's figures.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Round {
    /// Its first card.
    pub from: CardId,
    pub cards: usize,
    /// Done: applied, or done with nothing to apply.
    pub landed: usize,
    pub failed: usize,
    pub withdrawn: usize,
    /// What waits for the person: the project's open items to decide, at
    /// least the cards whose change waits.
    pub waits: usize,
    /// What the cards' tries cost, in ten-thousandths of a dollar; the
    /// leader's own spending is not in it.
    pub cost: u64,
    /// Part of it Claude Code did not price: `cost` is at least.
    pub unpriced: bool,
    /// From its first card to its last card's end (to now while a change
    /// waits).
    pub ticks: u64,
}

impl Round {
    /// One more project's round counted in.
    pub fn and(self, o: Round) -> Round {
        Round {
            from: self.from.min(o.from),
            cards: self.cards + o.cards,
            landed: self.landed + o.landed,
            failed: self.failed + o.failed,
            withdrawn: self.withdrawn + o.withdrawn,
            waits: self.waits + o.waits,
            cost: self.cost + o.cost,
            unpriced: self.unpriced || o.unpriced,
            ticks: self.ticks.max(o.ticks),
        }
    }
}

/// Where a project whose team is at rest stands.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Finished {
    /// The team is through; the rest is the person's.
    Team(Round),
    /// Nothing is left for the team or the person.
    All(Round),
}

impl Finished {
    pub fn round(self) -> Round {
        match self {
            Finished::Team(r) | Finished::All(r) => r,
        }
    }

    pub fn all(self) -> bool {
        matches!(self, Finished::All(_))
    }

    /// Two projects' states as one: all done only when both are.
    pub fn and(self, o: Finished) -> Finished {
        let r = self.round().and(o.round());
        match self.all() && o.all() {
            true => Finished::All(r),
            false => Finished::Team(r),
        }
    }
}

/// The tick a card came at: its first story line, or when its work began
/// if that was earlier (a card opened for work already under way).
fn born(c: &Card) -> u64 {
    let told = c.story.first().map(|s| s.tick);
    let began = (c.started > 0).then_some(c.started);
    match (told, began) {
        (Some(a), Some(b)) => a.min(b),
        (a, b) => a.or(b).unwrap_or(0),
    }
}

impl World {
    /// The first card of project `p`'s round: the newest card that came
    /// after every earlier one of the project was through; its first card
    /// when none did. None for a project with no card.
    pub fn round_from(&self, p: ProjectId) -> Option<CardId> {
        let mut from = None;
        // Whether every card so far is through, and when the last ended.
        let (mut over, mut ended) = (true, 0);
        for c in self.cards.iter().filter(|c| c.project == p) {
            if from.is_none() || (over && born(c) > ended) {
                from = Some(c.id);
            }
            over &= c.over();
            ended = ended.max(c.finished);
        }
        from
    }

    /// Whether project `p`'s round is finished, and how far: none while a
    /// card of it waits, is at work or is being verified, while anybody
    /// of the project is in a turn, starting or about to try a failed
    /// turn again, for a project with no card, without a leader (a solo
    /// hands out nothing), closed or archived.
    pub fn finished(&self, p: ProjectId) -> Option<Finished> {
        let q = self.projects.get(p)?;
        if q.closed || q.archived.is_some() || self.agents.get(q.leader)?.role != Role::Leader {
            return None;
        }
        let from = self.round_from(p)?;
        let mine = |c: &&Card| c.project == p && c.id >= from;
        let count = |col: Column| {
            self.cards
                .iter()
                .filter(mine)
                .filter(move |c| c.column == col)
        };
        let open = [Column::Waiting, Column::Working, Column::Verifying];
        if open.iter().any(|&col| count(col).next().is_some()) {
            return None;
        }
        let team = || self.agents.iter().filter(|a| a.project == p && !a.gone);
        if team().any(|a| a.status.busy() || a.starting || a.retry.is_some()) {
            return None;
        }
        let review = count(Column::Review).count();
        let asks = (self.open_inbox().into_iter())
            .map(|i| &self.inbox[i])
            .filter(|i| i.kind != InboxKind::Reply && self.agents[i.agent].project == p)
            .count();
        let rows =
            || (self.ledger.iter()).filter(|r| r.project == p && r.card.is_some_and(|c| c >= from));
        let start = self.cards.iter().filter(mine).map(born).min().unwrap_or(0);
        let waits = asks.max(review);
        let end = match waits {
            0 => (self.cards.iter().filter(mine))
                .map(|c| c.finished)
                .max()
                .unwrap_or(0),
            _ => self.tick,
        };
        let round = Round {
            from,
            cards: self.cards.iter().filter(mine).count(),
            landed: count(Column::Done).count(),
            failed: count(Column::Failed).count(),
            withdrawn: count(Column::Withdrawn).count(),
            waits,
            cost: rows().map(|r| r.cost).sum(),
            unpriced: rows().any(|r| r.unpriced),
            ticks: end.saturating_sub(start),
        };
        Some(match waits {
            0 => Finished::All(round),
            _ => Finished::Team(round),
        })
    }

    /// The open projects' states as one, for what speaks of all of them
    /// (the tab's title): none while a team that has cards is not
    /// finished; projects with no card and solos say nothing either way.
    pub fn finished_all(&self) -> Option<Finished> {
        self.finished_of(self.open_projects().map(|q| q.id))
    }

    /// The states of `projects` as one: see [`World::finished_all`].
    pub fn finished_of(&self, projects: impl Iterator<Item = ProjectId>) -> Option<Finished> {
        let mut all: Option<Finished> = None;
        for p in projects {
            let q = &self.projects[p];
            let team = self
                .agents
                .get(q.leader)
                .is_some_and(|a| a.role == Role::Leader);
            if !team || self.round_from(p).is_none() {
                continue;
            }
            let f = self.finished(p)?;
            all = Some(all.map_or(f, |a| a.and(f)));
        }
        all
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ledger::Attempt;
    use crate::live::testing::{team, with_card};
    use crate::model::{InboxItem, Status, Story, StoryEvent};

    /// A team of two workers at rest, with `columns.len()` cards.
    fn rested(columns: &[Column]) -> World {
        let mut w = team(2);
        for a in &mut w.agents {
            a.status = Status::Idle;
        }
        for &col in columns {
            let c = with_card(&mut w, 1);
            w.cards[c].column = col;
        }
        w.agents[1].card = None;
        w
    }

    fn item(kind: InboxKind, agent: usize) -> InboxItem {
        InboxItem {
            kind,
            agent,
            detail: String::new(),
            researcher: None,
            open: true,
            tick: 0,
            card: None,
        }
    }

    #[test]
    fn every_card_through_and_nothing_waiting_is_all_done() {
        let w = rested(&[
            Column::Done,
            Column::Done,
            Column::Failed,
            Column::Withdrawn,
        ]);
        let Some(Finished::All(r)) = w.finished(0) else {
            panic!("{:?}", w.finished(0));
        };
        assert_eq!((r.cards, r.landed, r.failed, r.withdrawn), (4, 2, 1, 1));
        assert_eq!(r.waits, 0);
    }

    #[test]
    fn a_card_still_open_says_nothing() {
        for col in [Column::Waiting, Column::Working, Column::Verifying] {
            assert_eq!(rested(&[Column::Done, col]).finished(0), None, "{col:?}");
        }
        assert_eq!(rested(&[]).finished(0), None, "no card, no round");
    }

    #[test]
    fn a_change_or_an_ask_waiting_is_the_team_finished() {
        let w = rested(&[Column::Done, Column::Review, Column::Review]);
        let Some(Finished::Team(r)) = w.finished(0) else {
            panic!("{:?}", w.finished(0));
        };
        assert_eq!((r.cards, r.landed, r.waits), (3, 1, 2));
        // An item to decide with every card through: still the person's.
        let mut w = rested(&[Column::Failed]);
        w.inbox.push(item(InboxKind::Tries, 0));
        assert!(matches!(w.finished(0), Some(Finished::Team(r)) if r.waits == 1));
        // A reply is read, not decided; a closed item is gone.
        let mut w = rested(&[Column::Done]);
        w.inbox.push(item(InboxKind::Reply, 0));
        w.inbox.push(InboxItem {
            open: false,
            ..item(InboxKind::Question, 1)
        });
        assert!(w.finished(0).is_some_and(Finished::all));
    }

    #[test]
    fn anybody_in_a_turn_starting_or_retrying_says_nothing() {
        let done = || rested(&[Column::Done]);
        let mut w = done();
        w.agents[0].status = Status::Working;
        assert_eq!(w.finished(0), None, "the leader may hand out more");
        let mut w = done();
        w.agents[2].status = Status::Working;
        assert_eq!(w.finished(0), None);
        let mut w = done();
        w.agents[1].starting = true;
        assert_eq!(w.finished(0), None);
        let mut w = done();
        w.agents[1].retry = Some(crate::model::Retry { attempt: 1, at: 9 });
        assert_eq!(w.finished(0), None);
        // One who left holds nothing back.
        let mut w = done();
        w.agents[2].status = Status::Working;
        w.agents[2].gone = true;
        assert!(w.finished(0).is_some());
    }

    #[test]
    fn a_solo_and_a_closed_project_say_nothing() {
        let mut w = rested(&[Column::Done]);
        w.agents[0].role = Role::Solo;
        assert_eq!(w.finished(0), None);
        assert_eq!(w.finished_all(), None);
        let mut w = rested(&[Column::Done]);
        w.projects[0].closed = true;
        assert_eq!(w.finished(0), None);
        assert_eq!(w.finished(9), None, "no such project");
    }

    /// Card `c` came at `born` and ended at `finished`.
    fn at(w: &mut World, c: CardId, born: u64, finished: u64) {
        w.cards[c].story = vec![Story {
            tick: born,
            event: StoryEvent::Took { agent: 1 },
        }];
        w.cards[c].finished = finished;
    }

    #[test]
    fn a_round_begins_at_the_card_that_came_after_all_before_it_were_through() {
        let mut w = rested(&[Column::Done, Column::Failed, Column::Done, Column::Done]);
        at(&mut w, 0, 1, 10);
        at(&mut w, 1, 2, 20);
        // Came while card 1 was at work: the same round.
        at(&mut w, 2, 15, 30);
        // Came after all three ended: a new round.
        at(&mut w, 3, 31, 40);
        assert_eq!(w.round_from(0), Some(3));
        let r = w.finished(0).unwrap().round();
        assert_eq!((r.from, r.cards, r.landed, r.failed), (3, 1, 1, 0));
        assert_eq!(r.ticks, 9, "from its card's coming to its end");
        // At the very tick the last one ended: not after it, one round.
        at(&mut w, 3, 30, 40);
        assert_eq!(w.round_from(0), Some(0));
        assert_eq!(w.finished(0).unwrap().round().ticks, 39);
        // An earlier card handed again is not through: one round again.
        at(&mut w, 3, 31, 40);
        w.cards[1].column = Column::Review;
        assert_eq!(w.round_from(0), Some(0));
        assert_eq!(w.round_from(5), None);
    }

    #[test]
    fn work_begun_before_the_card_opened_counts_from_its_beginning() {
        let mut w = rested(&[Column::Done]);
        at(&mut w, 0, 8, 20);
        w.cards[0].started = 5;
        assert_eq!(w.finished(0).unwrap().round().ticks, 15);
        w.cards[0].started = 12;
        assert_eq!(w.finished(0).unwrap().round().ticks, 12);
    }

    #[test]
    fn a_round_with_a_change_waiting_runs_until_now() {
        let mut w = rested(&[Column::Done, Column::Review]);
        at(&mut w, 0, 4, 10);
        at(&mut w, 1, 6, 0);
        w.tick = 50;
        assert_eq!(w.finished(0).unwrap().round().ticks, 46);
    }

    #[test]
    fn the_cost_is_the_round_s_cards_tries() {
        let mut w = rested(&[Column::Done, Column::Done]);
        at(&mut w, 0, 1, 5);
        at(&mut w, 1, 9, 12);
        let row = |card, cost, unpriced| Attempt {
            card,
            cost,
            unpriced,
            ..Attempt::default()
        };
        w.ledger = vec![
            row(Some(0), 500, true),
            row(Some(1), 70, false),
            row(Some(1), 30, false),
            row(None, 9, false),
            Attempt {
                project: 1,
                ..row(Some(1), 4000, true)
            },
        ];
        let r = w.finished(0).unwrap().round();
        assert_eq!((r.from, r.cost, r.unpriced), (1, 100, false));
        w.ledger[2].unpriced = true;
        assert!(w.finished(0).unwrap().round().unpriced);
    }

    #[test]
    fn several_projects_are_all_done_only_when_each_team_with_cards_is() {
        let mut w = rested(&[Column::Done, Column::Failed]);
        let lead = w.agents[0].clone();
        let p = w.projects.len();
        let mut q = w.projects[0].clone();
        q.id = p;
        q.leader = w.agents.len();
        w.projects.push(q);
        w.agents.push(crate::model::Agent {
            id: w.agents.len(),
            project: p,
            ..lead
        });
        // The second has no card: it says nothing either way.
        assert!(w.finished_all().is_some_and(Finished::all));
        let c = with_card(&mut w, 1);
        w.cards[c].project = p;
        w.agents[1].card = None;
        assert_eq!(w.finished_all(), None, "its card is at work");
        w.cards[c].column = Column::Review;
        let Some(Finished::Team(r)) = w.finished_all() else {
            panic!("{:?}", w.finished_all());
        };
        assert_eq!((r.cards, r.landed, r.failed, r.waits), (3, 1, 1, 1));
        w.cards[c].column = Column::Done;
        assert!(w.finished_all().is_some_and(Finished::all));
        assert_eq!(w.finished_of([p].into_iter()).unwrap().round().cards, 1);
        w.projects[p].closed = true;
        assert_eq!(w.finished_all().unwrap().round().cards, 2);
    }
}
