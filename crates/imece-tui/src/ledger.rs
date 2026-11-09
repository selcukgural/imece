//! The cost ledger (record 133, G2 of #60, part two): one row per attempt
//! at a card, what it cost and what became of it, so the person sees what
//! was applied and what was spent for nothing, card by card.
//!
//! A row is a worker's attempt at a card: the card, its try, the worker,
//! the model Claude Code said it ran on, what it spent (its helpers'
//! counted in), the changes it put before the person and how many came
//! back, when it began and ended, Claude Code's version when it last
//! spent, and its [`Outcome`]. A worker's spending with no card (a solo,
//! a worker told directly) goes on a row of its own with no card, closed
//! when a change of it is applied. The leader's spending is nobody's row:
//! the window says it apart.
//!
//! What is booked is what Claude Code said: each time an agent's figure
//! grows (a turn's end live, a busy tick in the demo), the growth goes on
//! the row its worker spends on now ([`World::book`]); what Claude Code
//! did not price marks the row (record 132). Nothing is counted twice:
//! every dollar of a worker or a helper is on exactly one row. What each
//! model's figure grew goes on the same row with it (record 134), so a
//! row says what its worker, its helpers and Claude Code's background
//! models spent, model by model.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use crate::model::{AgentId, CardId, ProjectId, Role, World};

/// What became of an attempt.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Outcome {
    /// Still at work, or its change waits.
    #[default]
    Open,
    /// Its change was applied (on trunk, on Lonca).
    Applied,
    /// Its card was done with no change to apply.
    Done,
    /// Its worker said it failed, or the person said no more tries.
    Failed,
    /// The leader withdrew its card.
    Withdrawn,
    /// The card was handed to another try while this one was at work.
    HandedOn,
    /// Handed on, and its worker still sent a `done` for it.
    Late,
}

impl Outcome {
    /// Whether what was spent on it went into something kept.
    pub fn kept(self) -> bool {
        matches!(self, Outcome::Applied | Outcome::Done)
    }
}

/// One attempt at a card (or a worker's work with none).
#[derive(Clone, Debug, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Attempt {
    pub project: ProjectId,
    pub card: Option<CardId>,
    /// The card's try, from 1; 0 with no card.
    pub attempt: u32,
    pub agent: AgentId,
    /// The model Claude Code said the worker ran on, as it named it; empty
    /// before it said.
    pub model: String,
    /// What it spent, in ten-thousandths of a dollar (as `Agent.cost`).
    pub cost: u64,
    /// Part of it Claude Code did not price: `cost` is at least.
    pub unpriced: bool,
    /// The changes it put before the person.
    pub rounds: u32,
    /// How many of them came back (the person's no, Lonca's).
    pub sent_back: u32,
    pub outcome: Outcome,
    /// The tick it began, and the tick it ended (as `began` while open).
    pub began: u64,
    pub ended: u64,
    /// Claude Code's version when it last spent; none when not known.
    pub version: Option<String>,
    /// What each model spent of `cost` (record 134), in ten-thousandths
    /// of a dollar, by the name Claude Code gives it; what no model's
    /// figure said (a turn's growth Claude Code did not split, an older
    /// save) is not in it.
    pub models: BTreeMap<String, u64>,
}

impl Attempt {
    /// What of its cost no model's figure said (record 134).
    pub fn unsplit(&self) -> u64 {
        self.cost.saturating_sub(self.models.values().sum())
    }

    /// How long it took, in ticks: until now while open.
    pub fn ticks(&self, now: u64) -> u64 {
        let end = if self.outcome == Outcome::Open {
            now
        } else {
            self.ended
        };
        end.saturating_sub(self.began)
    }
}

/// What an agent's spending was booked up to (memory only: a load sets it
/// at the agent's figure, which the store keeps with the rows).
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Meter {
    /// The model Claude Code said its session runs on.
    pub model: String,
    /// Its figure when last booked.
    pub booked: u64,
    /// Its unpriced gaps when last booked.
    pub gaps: u32,
    /// The row its spending goes on now (a worker's; a helper's goes on
    /// its worker's).
    pub row: Option<usize>,
    /// What each model's figure grew since last booked, in millionths of
    /// a dollar (as Claude Code says it): booked in ten-thousandths, the
    /// rest kept for the next booking.
    pub models: imece_agent::ByModel,
}

/// A project's figures, as the window says them.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Totals {
    /// On attempts whose work was kept.
    pub kept: u64,
    /// On attempts nothing came of.
    pub wasted: u64,
    /// On attempts still at work.
    pub open: u64,
    /// The leader's own.
    pub lead: u64,
    /// Whether any of each part was not priced: kept, wasted, open, lead.
    pub unpriced: [bool; 4],
}

impl World {
    /// What each model's figure of agent `id`'s session grew in the turn
    /// about to end (record 134): booked with the turn's figure.
    pub fn models_grew(&mut self, id: AgentId, grew: &imece_agent::ByModel) {
        let m = &mut self.agents[id].meter.models;
        for (model, more) in grew {
            *m.entry(model.clone()).or_default() += more;
        }
    }

    /// Agent `id`'s figure grew, or it left something unpriced: the growth
    /// goes on the row its worker spends on now, a new one with no card
    /// when there is none. A leader's or a researcher's spending is
    /// nobody's row.
    pub fn book(&mut self, id: AgentId) {
        let a = &mut self.agents[id];
        let grew = a.cost.saturating_sub(a.meter.booked);
        let unpriced = a.unpriced.session || a.unpriced.gaps > a.meter.gaps;
        a.meter.booked = a.cost;
        a.meter.gaps = a.unpriced.gaps;
        if grew == 0 && !unpriced {
            return;
        }
        let worker = match a.role {
            Role::Worker | Role::Solo => Some(id),
            Role::Helper => a.parent,
            Role::Leader | Role::Researcher => None,
        };
        let Some(worker) = worker else {
            // Nobody's row: its figures by model are its own.
            a.meter.models.clear();
            return;
        };
        // Whole ten-thousandths go on the row, the rest waits.
        let mut by_model = Vec::new();
        for (model, more) in a.meter.models.iter_mut() {
            if *more >= 100 {
                by_model.push((model.clone(), *more / 100));
                *more %= 100;
            }
        }
        a.meter.models.retain(|_, more| *more > 0);
        let i = self.row_of(worker);
        let tick = self.tick;
        let model = self.agents[worker].meter.model.clone();
        let row = &mut self.ledger[i];
        row.cost += grew;
        row.unpriced |= unpriced;
        for (model, more) in by_model {
            *row.models.entry(model).or_default() += more;
        }
        if !model.is_empty() {
            row.model = model;
        }
        if self.version.is_some() {
            row.version.clone_from(&self.version);
        }
        if row.outcome == Outcome::Open {
            row.ended = tick;
        }
    }

    /// The row worker `id` spends on now: the one it took, else its open
    /// row with no card, else a new one.
    fn row_of(&mut self, id: AgentId) -> usize {
        if let Some(i) = self.agents[id].meter.row {
            return i;
        }
        let found = (self.ledger.iter())
            .rposition(|r| r.agent == id && r.card.is_none() && r.outcome == Outcome::Open);
        let i = match found {
            Some(i) => i,
            None => self.open_row(id, None),
        };
        self.agents[id].meter.row = Some(i);
        i
    }

    /// Worker `id`'s open row with no card becomes its try at card `c`,
    /// which a change of the person's own line opened (record 126): what
    /// it spent on the line is the card's.
    pub(crate) fn row_to_card(&mut self, id: AgentId, c: CardId) {
        let found = (self.ledger.iter())
            .rposition(|r| r.agent == id && r.card.is_none() && r.outcome == Outcome::Open);
        if let Some(i) = found {
            let row = &mut self.ledger[i];
            row.card = Some(c);
            row.attempt = self.cards[c].attempt;
        }
    }

    /// A new row of worker `id`'s, on card `card` at the try it is on.
    fn open_row(&mut self, id: AgentId, card: Option<CardId>) -> usize {
        let a = &self.agents[id];
        let attempt = card.map_or(0, |c| self.cards[c].attempt);
        self.ledger.push(Attempt {
            project: a.project,
            card,
            attempt,
            agent: id,
            model: a.meter.model.clone(),
            began: self.tick,
            ended: self.tick,
            version: self.version.clone(),
            ..Attempt::default()
        });
        self.ledger.len() - 1
    }

    /// The row of worker `id`'s try `attempt` at card `c`.
    pub fn row_at(&self, id: AgentId, c: CardId, attempt: u32) -> Option<usize> {
        (self.ledger.iter())
            .rposition(|r| r.agent == id && r.card == Some(c) && r.attempt == attempt)
    }

    /// Worker `id` takes card `c` for its turn: what it spends goes on
    /// its try at it from now, a row opened when it has none.
    pub(crate) fn row_takes(&mut self, id: AgentId, c: CardId) {
        let i = match self.row_at(id, c, self.cards[c].attempt) {
            Some(i) => i,
            None => self.open_row(id, Some(c)),
        };
        self.agents[id].meter.row = Some(i);
    }

    /// Row `i` ends, with `outcome`; a row ended already keeps how it
    /// ended. A worker whose work was kept spends on no row of it any more.
    fn close_row(&mut self, i: usize, outcome: Outcome) {
        let tick = self.tick;
        let row = &mut self.ledger[i];
        if row.outcome != Outcome::Open {
            return;
        }
        row.outcome = outcome;
        row.ended = tick;
        let a = &mut self.agents[row.agent].meter;
        if outcome.kept() && a.row == Some(i) {
            a.row = None;
        }
    }

    /// Worker `id`'s tries at cards `cards` end with `outcome`; with none,
    /// its row with no card, when it has one open.
    pub(crate) fn close_rows(&mut self, id: AgentId, cards: &[CardId], outcome: Outcome) {
        let rows: Vec<usize> = match cards {
            [] => (self.ledger.iter())
                .rposition(|r| r.agent == id && r.card.is_none() && r.outcome == Outcome::Open)
                .into_iter()
                .collect(),
            _ => (cards.iter())
                .filter_map(|&c| self.row_at(id, c, self.cards[c].attempt))
                .collect(),
        };
        for i in rows {
            self.close_row(i, outcome);
        }
    }

    /// The owner of card `c` ends its try at it with `outcome`.
    pub(crate) fn close_card_row(&mut self, c: CardId, outcome: Outcome) {
        if let Some(owner) = self.cards[c].owner {
            self.close_rows(owner, &[c], outcome);
        }
    }

    /// Worker `id` put a change before the person (`back` false), or one
    /// came back: on its tries at `cards`, else on its row with no card.
    pub(crate) fn count_round(&mut self, id: AgentId, cards: &[CardId], back: bool) {
        let rows: Vec<usize> = match cards {
            [] => vec![self.row_of(id)],
            _ => (cards.iter())
                .map(|&c| match self.row_at(id, c, self.cards[c].attempt) {
                    Some(i) => i,
                    None => self.open_row(id, Some(c)),
                })
                .collect(),
        };
        for i in rows {
            match back {
                true => self.ledger[i].sent_back += 1,
                false => self.ledger[i].rounds += 1,
            }
        }
    }

    /// Worker `id` sent a `done` for its try at card `c` after the card
    /// was handed on: that try was late.
    pub(crate) fn row_late(&mut self, id: AgentId, c: CardId, attempt: u32) {
        if let Some(i) = self.row_at(id, c, attempt)
            && self.ledger[i].outcome == Outcome::HandedOn
        {
            self.ledger[i].outcome = Outcome::Late;
        }
    }

    /// After a load: each agent's spending is booked up to its figure, and
    /// a worker spends on its newest row that did not end kept.
    pub fn meters_after_load(&mut self) {
        for a in &mut self.agents {
            a.meter.booked = a.cost;
            a.meter.gaps = a.unpriced.gaps;
            a.meter.row = (self.ledger.iter())
                .rposition(|r| r.agent == a.id)
                .filter(|&i| !self.ledger[i].outcome.kept());
        }
    }

    /// Project `p`'s rows, newest first.
    pub fn ledger_of(&self, p: ProjectId) -> Vec<usize> {
        (0..self.ledger.len())
            .rev()
            .filter(|&i| self.ledger[i].project == p)
            .collect()
    }

    /// What project `p`'s rows spent by model (record 134), and what of
    /// them no model's figure said.
    pub fn models_of(&self, p: ProjectId) -> (BTreeMap<String, u64>, u64) {
        let mut by = BTreeMap::new();
        let mut unsplit = 0;
        for r in self.ledger.iter().filter(|r| r.project == p) {
            for (model, cost) in &r.models {
                *by.entry(model.clone()).or_default() += cost;
            }
            unsplit += r.unsplit();
        }
        (by, unsplit)
    }

    /// Project `p`'s figures: what its rows came to by how they ended,
    /// and its leaders' own.
    pub fn totals(&self, p: ProjectId) -> Totals {
        let mut t = Totals::default();
        for r in self.ledger.iter().filter(|r| r.project == p) {
            let (sum, part) = match r.outcome {
                Outcome::Open => (&mut t.open, 2),
                o if o.kept() => (&mut t.kept, 0),
                _ => (&mut t.wasted, 1),
            };
            *sum += r.cost;
            t.unpriced[part] |= r.unpriced;
        }
        for a in (self.agents.iter()).filter(|a| a.project == p && a.role == Role::Leader) {
            t.lead += a.cost;
            t.unpriced[3] |= a.unpriced.any();
        }
        t
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::live::testing::{team, with_card, with_helper};
    use crate::model::{Agent, Column, Status};

    /// A change of agent `id`'s of one file.
    fn change(agent: AgentId) -> crate::model::Change {
        crate::model::Change {
            agent,
            title: "Fix".into(),
            files: Vec::new(),
            patch: "diff --git a/x b/x\n".into(),
            runs: Vec::new(),
            check: crate::model::Check::None,
            spend: None,
        }
    }

    /// Agent `id` spends up to `cost` now, and it is booked.
    fn spend(w: &mut World, id: AgentId, cost: u64) {
        w.agents[id].cost = cost;
        w.book(id);
    }

    #[test]
    fn what_an_agent_spends_goes_on_its_worker_s_row_and_a_leader_s_on_none() {
        let mut w = team(1);
        let h = with_helper(&mut w, 1);
        w.agents.push(Agent::new(
            3,
            0,
            "r".into(),
            Role::Researcher,
            Status::Idle,
            0,
        ));
        // A leader's and a researcher's spending is nobody's row.
        spend(&mut w, 0, 500);
        spend(&mut w, 3, 70);
        assert!(w.ledger.is_empty());
        assert_eq!(w.agents[0].meter.booked, 500, "booked all the same");
        // Nothing grew and nothing unpriced: no row.
        w.book(1);
        assert!(w.ledger.is_empty());
        // A worker with no card: a row with none, at the tick it began.
        w.tick = 7;
        w.agents[1].meter.model = "claude-opus-5-5".into();
        w.version = Some("2.1.4 (Claude Code)".into());
        spend(&mut w, 1, 300);
        assert_eq!(w.ledger.len(), 1);
        let r = &w.ledger[0];
        assert_eq!(
            (
                r.project, r.card, r.attempt, r.agent, r.cost, r.began, r.ended
            ),
            (0, None, 0, 1, 300, 7, 7)
        );
        assert_eq!(r.model, "claude-opus-5-5");
        assert_eq!(r.version.as_deref(), Some("2.1.4 (Claude Code)"));
        assert_eq!(w.agents[1].meter.row, Some(0));
        // Only the growth: 300 then 450 is 150 more; its helper's too,
        // under the helper's own figure.
        w.tick = 9;
        w.version = None;
        w.agents[h].meter.model = "claude-haiku-5-5".into();
        spend(&mut w, 1, 450);
        spend(&mut w, h, 40);
        let r = &w.ledger[0];
        assert_eq!((r.cost, r.ended), (490, 9));
        assert_eq!(r.model, "claude-opus-5-5", "the worker's, not its helper's");
        assert_eq!(
            r.version.as_deref(),
            Some("2.1.4 (Claude Code)"),
            "a version not known keeps the last"
        );
        // A figure that went down books nothing and starts again from it.
        spend(&mut w, 1, 100);
        assert_eq!(w.ledger[0].cost, 490);
        assert_eq!(w.agents[1].meter.booked, 100);
        spend(&mut w, 1, 160);
        assert_eq!(w.ledger[0].cost, 550);
        // A worker's model not said yet leaves the row's.
        w.agents[1].meter.model.clear();
        spend(&mut w, 1, 170);
        assert_eq!(w.ledger[0].model, "claude-opus-5-5");
        // A helper with no worker books nowhere.
        w.agents[h].parent = None;
        spend(&mut w, h, 99);
        assert_eq!(w.ledger.len(), 1);
        assert_eq!(w.ledger[0].cost, 560);
    }

    fn grew(pairs: &[(&str, u64)]) -> imece_agent::ByModel {
        pairs.iter().map(|(m, c)| (m.to_string(), *c)).collect()
    }

    fn models(w: &World, i: usize) -> Vec<(&str, u64)> {
        (w.ledger[i].models.iter())
            .map(|(m, c)| (m.as_str(), *c))
            .collect()
    }

    #[test]
    fn what_each_model_spent_goes_on_the_row_with_the_figure_whole_ten_thousandths_at_a_time() {
        let mut w = team(1);
        let h = with_helper(&mut w, 1);
        // 2.5 ten-thousandths of a and 1.2 of b: 2 and 1 go on the row,
        // the rest waits for the next booking.
        w.models_grew(1, &grew(&[("a", 250), ("b", 120)]));
        w.models_grew(1, &grew(&[("a", 0)]));
        spend(&mut w, 1, 4);
        assert_eq!(models(&w, 0), [("a", 2), ("b", 1)]);
        assert_eq!(w.ledger[0].unsplit(), 1);
        assert_eq!(
            w.agents[1].meter.models,
            grew(&[("a", 50), ("b", 20)]),
            "a model with nothing left is not kept"
        );
        // Exactly one more: 50 + 50 of a is one whole; b's 20 + 79 is not.
        w.models_grew(1, &grew(&[("a", 50), ("b", 79)]));
        spend(&mut w, 1, 5);
        assert_eq!(models(&w, 0), [("a", 3), ("b", 1)]);
        assert_eq!(w.agents[1].meter.models, grew(&[("b", 99)]));
        // A helper's go on its worker's row, under their own names.
        w.models_grew(h, &grew(&[("c", 300)]));
        spend(&mut w, h, 3);
        assert_eq!(models(&w, 0), [("a", 3), ("b", 1), ("c", 3)]);
        assert!(w.agents[h].meter.models.is_empty());
        assert_eq!(w.ledger[0].unsplit(), 1);
        // Nothing grew: what waits keeps waiting.
        w.models_grew(1, &grew(&[("b", 1)]));
        w.book(1);
        assert_eq!(models(&w, 0), [("a", 3), ("b", 1), ("c", 3)]);
        assert_eq!(w.agents[1].meter.models, grew(&[("b", 100)]));
        // A leader's are nobody's row, and not kept.
        w.models_grew(0, &grew(&[("a", 900)]));
        spend(&mut w, 0, 9);
        assert!(w.agents[0].meter.models.is_empty());
        assert_eq!(w.ledger.len(), 1);
        // More said by model than the figure grew: nothing is not split.
        w.ledger[0].models.insert("d".into(), 50);
        assert_eq!(w.ledger[0].unsplit(), 0);
    }

    #[test]
    fn what_a_worker_spent_on_the_person_s_line_is_the_try_of_the_card_its_change_opens() {
        let mut w = team(2);
        // Its card failed: its row ended so, and the person's next line
        // spends on a row with no card (record 126).
        let c = with_card(&mut w, 1);
        w.takes(1, c);
        spend(&mut w, 1, 4);
        w.card_failed(1, c, "no");
        w.own_line(1, "fix the footer");
        spend(&mut w, 1, 10);
        assert_eq!(w.ledger.len(), 2);
        assert_eq!(
            (w.ledger[0].cost, w.ledger[0].outcome),
            (4, Outcome::Failed)
        );
        assert_eq!((w.ledger[1].card, w.ledger[1].cost), (None, 6));
        // Another worker's row with no card stays its own.
        spend(&mut w, 2, 3);
        // Its change opens the card: that row is the card's first try.
        w.change_ready(crate::model::Change {
            agent: 1,
            title: "Footer".into(),
            files: Vec::new(),
            patch: String::new(),
            runs: Vec::new(),
            check: crate::model::Check::None,
            spend: None,
        });
        let card = w.agents[1].card.unwrap();
        assert_ne!(card, c);
        assert_eq!(
            (w.ledger[1].card, w.ledger[1].attempt, w.ledger[1].rounds),
            (Some(card), 1, 1)
        );
        assert_eq!(w.ledger[2].card, None, "the other worker's");
        spend(&mut w, 1, 12);
        assert_eq!(w.ledger.len(), 3);
        assert_eq!(w.ledger[1].cost, 8);
    }

    #[test]
    fn a_project_s_spending_by_model_sums_its_rows_alone_and_what_was_not_split() {
        let mut w = team(2);
        let p = w.agents[1].project;
        w.models_grew(1, &grew(&[("a", 300), ("b", 100)]));
        spend(&mut w, 1, 9);
        // Another row of the project: its own.
        let c = with_card(&mut w, 1);
        w.takes(1, c);
        w.models_grew(1, &grew(&[("a", 200)]));
        spend(&mut w, 1, 12);
        assert_eq!(w.ledger.len(), 2);
        // Another project's row counts not.
        w.ledger.push(Attempt {
            project: p + 1,
            cost: 70,
            models: BTreeMap::from([("a".into(), 70)]),
            ..Attempt::default()
        });
        let (by, unsplit) = w.models_of(p);
        assert_eq!(by, BTreeMap::from([("a".into(), 5), ("b".into(), 1)]));
        // 9 - 4 on the first, 3 - 2 on the second.
        assert_eq!(unsplit, 6);
        assert_eq!(w.models_of(p + 1), (BTreeMap::from([("a".into(), 70)]), 0));
    }

    #[test]
    fn a_worker_s_row_with_no_card_ends_only_when_it_is_its_own_open_one() {
        let mut w = team(2);
        let c = with_card(&mut w, 1);
        let row = |agent, card, outcome| Attempt {
            agent,
            card,
            outcome,
            ..Attempt::default()
        };
        w.ledger = vec![
            row(1, None, Outcome::Open),
            // Newer, each unlike it in one way: another's with no card,
            // its own on a card, its own with no card ended already.
            row(2, None, Outcome::Open),
            row(1, Some(c), Outcome::Open),
            row(1, None, Outcome::Applied),
        ];
        w.close_rows(1, &[], Outcome::Failed);
        let ended: Vec<Outcome> = w.ledger.iter().map(|r| r.outcome).collect();
        assert_eq!(
            ended,
            [
                Outcome::Failed,
                Outcome::Open,
                Outcome::Open,
                Outcome::Applied
            ]
        );
    }

    #[test]
    fn what_claude_code_did_not_price_marks_the_row() {
        let mut w = team(1);
        spend(&mut w, 1, 10);
        assert!(!w.ledger[0].unpriced);
        // The same gaps as last booked: priced.
        w.agents[1].unpriced.gaps = 2;
        w.agents[1].meter.gaps = 2;
        spend(&mut w, 1, 20);
        assert!(!w.ledger[0].unpriced);
        // One more gap, nothing grown: still booked, as unpriced.
        w.agents[1].unpriced.gaps = 3;
        w.book(1);
        assert!(w.ledger[0].unpriced);
        assert_eq!(w.agents[1].meter.gaps, 3);
        // A session's turn unpriced, on a row of its own.
        let c = with_card(&mut w, 1);
        w.takes(1, c);
        w.agents[1].unpriced.session = true;
        w.book(1);
        assert!(w.ledger[1].unpriced);
        // A session that ended mid-turn: its gap goes on the row too.
        let mut w = team(1);
        spend(&mut w, 1, 10);
        w.session_over(1, true);
        assert!(w.ledger[0].unpriced);
    }

    #[test]
    fn a_card_s_try_has_a_row_of_its_own_taken_again_at_the_same_try() {
        let mut w = team(2);
        spend(&mut w, 1, 100);
        let c = with_card(&mut w, 1);
        w.tick = 4;
        w.takes(1, c);
        assert_eq!(w.ledger.len(), 2, "a row for the card's try");
        assert_eq!(
            (w.ledger[1].card, w.ledger[1].attempt, w.ledger[1].began),
            (Some(c), 1, 4)
        );
        spend(&mut w, 1, 250);
        assert_eq!((w.ledger[0].cost, w.ledger[1].cost), (100, 150));
        // Taken again at the same try: the same row.
        w.takes(1, c);
        assert_eq!(w.ledger.len(), 2);
        assert_eq!(w.agents[1].meter.row, Some(1));
        // Handed on: the try ends, the next is another worker's row.
        w.tick = 8;
        w.hand(c, 2);
        assert_eq!(
            (w.ledger[1].outcome, w.ledger[1].ended),
            (Outcome::HandedOn, 8)
        );
        w.takes(2, c);
        assert_eq!(
            (w.ledger[2].agent, w.ledger[2].attempt),
            (2, 2),
            "the next try"
        );
        // What the first worker still spends goes on its try, and its done
        // for it makes it late.
        spend(&mut w, 1, 300);
        assert_eq!(w.ledger[1].cost, 200);
        w.late_done(1, c);
        assert_eq!(w.ledger[1].outcome, Outcome::Late);
        // Late only once handed on.
        w.late_done(2, c);
        assert_eq!(w.ledger[2].outcome, Outcome::Open);
    }

    #[test]
    fn a_row_ends_once_and_a_kept_one_takes_no_more() {
        let mut w = team(1);
        let c = with_card(&mut w, 1);
        w.takes(1, c);
        spend(&mut w, 1, 100);
        w.tick = 5;
        w.close_rows(1, &[c], Outcome::Failed);
        assert_eq!(
            (w.ledger[0].outcome, w.ledger[0].ended),
            (Outcome::Failed, 5)
        );
        assert_eq!(
            w.agents[1].meter.row,
            Some(0),
            "a failed try takes what follows"
        );
        // Ended already: how it ended stays.
        w.tick = 6;
        w.close_rows(1, &[c], Outcome::Applied);
        assert_eq!(
            (w.ledger[0].outcome, w.ledger[0].ended),
            (Outcome::Failed, 5)
        );
        // Spending after keeps the end, and adds to the cost.
        spend(&mut w, 1, 130);
        assert_eq!((w.ledger[0].cost, w.ledger[0].ended), (130, 5));
        // A kept one: what follows goes on a row with no card.
        let d = with_card(&mut w, 1);
        w.takes(1, d);
        w.close_rows(1, &[d], Outcome::Applied);
        assert_eq!(w.agents[1].meter.row, None);
        spend(&mut w, 1, 140);
        assert_eq!(w.ledger[2].card, None);
        // A row with no card ends by `[]`, the newest open one.
        w.close_rows(1, &[], Outcome::Done);
        assert_eq!(w.ledger[2].outcome, Outcome::Done);
        assert_eq!(w.agents[1].meter.row, None);
        // The next spending opens another: the old one is not taken again.
        spend(&mut w, 1, 150);
        assert_eq!((w.ledger.len(), w.ledger[3].cost), (4, 10));
        // An agent spending on another row is not moved off it.
        w.agents[1].meter.row = Some(0);
        w.close_rows(1, &[], Outcome::Applied);
        assert_eq!(w.agents[1].meter.row, Some(0));
        // A card nobody owns ends no row.
        w.cards[c].owner = None;
        w.close_card_row(c, Outcome::Withdrawn);
        assert_eq!(w.ledger[0].outcome, Outcome::Failed);
    }

    #[test]
    fn a_row_is_found_by_its_worker_card_and_try_and_one_with_no_card_by_its_worker() {
        let mut w = team(2);
        let c = with_card(&mut w, 1);
        w.takes(1, c);
        assert_eq!(w.row_at(1, c, 1), Some(0));
        assert_eq!(w.row_at(1, c, 2), None, "another try");
        assert_eq!(w.row_at(2, c, 1), None, "another worker");
        assert_eq!(w.row_at(1, c + 1, 1), None, "another card");
        // Spending with no row taken: not the card's try, a row with none.
        w.agents[1].meter.row = None;
        spend(&mut w, 1, 10);
        assert_eq!(w.ledger[1].card, None);
        // Another worker's open row with no card is not its own.
        w.agents[2].meter.row = None;
        spend(&mut w, 2, 10);
        assert_eq!((w.ledger.len(), w.ledger[2].agent), (3, 2));
        // Ending `[]` ends its own row with no card, not another's nor a
        // card's.
        w.close_rows(2, &[], Outcome::Applied);
        assert_eq!(
            [0, 1, 2].map(|i| w.ledger[i].outcome),
            [Outcome::Open, Outcome::Open, Outcome::Applied]
        );
        // Taken again with no row: the open one with none.
        w.agents[1].meter.row = None;
        spend(&mut w, 1, 20);
        assert_eq!((w.ledger.len(), w.ledger[1].cost), (3, 20));
        // A change of another worker's card counts no round on this one.
        let d = with_card(&mut w, 2);
        w.cards[d].column = Column::Review;
        w.cards[c].column = Column::Working;
        w.agents[1].status = Status::Working;
        w.change_ready(change(1));
        assert_eq!(w.ledger[0].rounds, 1);
        assert_eq!(
            w.row_at(1, d, 1),
            None,
            "the other's card has no row of its"
        );
        assert_eq!(w.ledger.len(), 3);
    }

    #[test]
    fn rounds_and_what_came_back_count_on_the_card_s_try_or_the_row_at_hand() {
        let mut w = team(1);
        let c = with_card(&mut w, 1);
        w.count_round(1, &[c], false);
        assert_eq!(w.ledger.len(), 1, "a try with no row yet gets one");
        w.count_round(1, &[c], true);
        w.count_round(1, &[c], false);
        assert_eq!((w.ledger[0].rounds, w.ledger[0].sent_back), (2, 1));
        // No card: the row it spends on.
        w.count_round(1, &[], true);
        assert_eq!(w.ledger.len(), 2);
        assert_eq!((w.ledger[1].rounds, w.ledger[1].sent_back), (0, 1));
    }

    #[test]
    fn a_load_books_each_agent_up_to_its_figure_on_its_newest_row_not_kept() {
        let mut w = team(2);
        let row = |agent, outcome| Attempt {
            agent,
            outcome,
            ..Attempt::default()
        };
        w.ledger = vec![
            row(1, Outcome::Open),
            row(2, Outcome::Open),
            row(1, Outcome::Failed),
            row(2, Outcome::Applied),
        ];
        w.agents[1].cost = 900;
        w.agents[1].unpriced.gaps = 4;
        w.meters_after_load();
        let m = &w.agents[1].meter;
        assert_eq!((m.booked, m.gaps, m.row), (900, 4, Some(2)));
        assert_eq!(w.agents[2].meter.row, None, "its newest was kept");
        assert_eq!(w.agents[0].meter.row, None);
    }

    #[test]
    fn a_project_s_figures_are_its_rows_by_how_they_ended_and_its_leader_s() {
        let mut w = team(1);
        let row = |project, cost, outcome, unpriced| Attempt {
            project,
            cost,
            outcome,
            unpriced,
            ..Attempt::default()
        };
        w.ledger = vec![
            row(0, 100, Outcome::Applied, false),
            row(0, 20, Outcome::Done, false),
            row(0, 7, Outcome::Failed, true),
            row(0, 3, Outcome::Withdrawn, false),
            row(0, 1_000, Outcome::HandedOn, false),
            row(0, 10_000, Outcome::Late, false),
            row(0, 50, Outcome::Open, false),
            row(1, 99_999, Outcome::Applied, true),
        ];
        w.agents[0].cost = 40;
        w.agents[1].cost = 5;
        let t = w.totals(0);
        assert_eq!((t.kept, t.wasted, t.open, t.lead), (120, 11_010, 50, 40));
        assert_eq!(t.unpriced, [false, true, false, false]);
        w.agents[0].unpriced.gaps = 1;
        assert_eq!(w.totals(0).unpriced, [false, true, false, true]);
        let t = w.totals(1);
        assert_eq!((t.kept, t.lead, t.unpriced[0]), (99_999, 0, true));
        assert!(!w.totals(1).unpriced[3], "another project's leader");
        // Newest first, one project's.
        assert_eq!(w.ledger_of(0), vec![6, 5, 4, 3, 2, 1, 0]);
        assert_eq!(w.ledger_of(1), vec![7]);
        assert!(w.ledger_of(2).is_empty());
    }

    #[test]
    fn an_attempt_lasts_until_now_while_open_and_until_its_end_after() {
        let mut r = Attempt {
            began: 10,
            ended: 15,
            ..Attempt::default()
        };
        assert_eq!(r.ticks(40), 30);
        assert_eq!(r.ticks(5), 0);
        r.outcome = Outcome::Failed;
        assert_eq!(r.ticks(40), 5);
        let kept = [Outcome::Applied, Outcome::Done];
        for o in [
            Outcome::Open,
            Outcome::Applied,
            Outcome::Done,
            Outcome::Failed,
            Outcome::Withdrawn,
            Outcome::HandedOn,
            Outcome::Late,
        ] {
            assert_eq!(o.kept(), kept.contains(&o), "{o:?}");
        }
    }

    #[test]
    fn a_card_s_try_ends_with_what_became_of_it() {
        use crate::model::{InboxItem, InboxKind, Landed};
        let mut w = team(1);
        // Failed by its worker.
        let c = with_card(&mut w, 1);
        w.takes(1, c);
        w.card_failed(1, c, "no");
        assert_eq!(w.ledger[0].outcome, Outcome::Failed);
        // Withdrawn by the leader.
        let c = with_card(&mut w, 1);
        w.takes(1, c);
        w.withdraw(c, 0, "moot");
        assert_eq!(w.ledger[1].outcome, Outcome::Withdrawn);
        // Failed for good by the person's no to one more.
        let c = with_card(&mut w, 1);
        w.takes(1, c);
        w.inbox.push(InboxItem {
            kind: InboxKind::Tries,
            agent: 0,
            detail: String::new(),
            researcher: None,
            open: true,
            tick: 0,
            card: Some(c),
        });
        w.decide_cap(w.inbox.len() - 1, false);
        assert_eq!(w.ledger[2].outcome, Outcome::Failed);
        // A change put before the person, sent back, then applied whole.
        let c = with_card(&mut w, 1);
        w.takes(1, c);
        w.agents[1].status = Status::Working;
        w.change_ready(change(1));
        assert_eq!(w.cards[c].column, Column::Review);
        assert_eq!(w.ledger[3].rounds, 1);
        w.change_rejected(1);
        assert_eq!(w.ledger[3].sent_back, 1);
        w.change_ready(change(1));
        // A part applied ends nothing.
        w.change_applied(1, "m".into(), false);
        assert_eq!(w.ledger[3].outcome, Outcome::Open);
        w.change_ready(change(1));
        w.change_applied(1, "m".into(), true);
        assert_eq!(w.ledger[3].outcome, Outcome::Applied);
        assert_eq!(w.ledger[3].rounds, 3);
        // Done with nothing to apply.
        let c = with_card(&mut w, 1);
        w.takes(1, c);
        w.change_none(1);
        assert_eq!(w.ledger[4].outcome, Outcome::Done);
        // A worker with no card whose turn changed nothing ends no row.
        w.agents[1].card = None;
        w.agents[1].meter.row = None;
        spend(&mut w, 1, 10);
        w.change_none(1);
        assert_eq!(w.ledger[5].outcome, Outcome::Open);
        // On Lonca: one sent back is a round back, one on trunk applied.
        let c = with_card(&mut w, 1);
        w.takes(1, c);
        w.cards[c].column = Column::Review;
        w.proposal_left(1, Landed::Conflicted(Vec::new()));
        assert_eq!(w.ledger[6].sent_back, 1);
        assert_eq!(w.ledger[6].outcome, Outcome::Open);
        w.cards[c].column = Column::Verifying;
        w.proposal_landed(1, Landed::Integrated(String::new()));
        assert_eq!(w.ledger[6].outcome, Outcome::Applied);
        // A worker that withdrew its own: no round back.
        let c = with_card(&mut w, 1);
        w.takes(1, c);
        w.cards[c].column = Column::Review;
        w.proposal_left(1, Landed::Withdrawn);
        assert_eq!(
            (w.ledger[7].sent_back, w.ledger[7].outcome),
            (0, Outcome::Open)
        );
    }
}
