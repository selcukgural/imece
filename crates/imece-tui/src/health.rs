//! Health notices to the leader (record 128, F6 of #59): a card whose
//! worker has stalled, a worker with nothing to do, an agent going round
//! on a call that keeps failing. imece looks; the leader is told, the
//! journal says it, and nothing is stopped: what is done about it is the
//! leader's or the person's.
//!
//! When it is said is the person's (`:health`, kept in prefs): minutes
//! before a stall and an idle worker are told, failures of one call in a
//! turn before a loop is; each can be off.

use serde::{Deserialize, Serialize};

use crate::model::{AgentId, CardId, Column, Event, Role, SECONDS_PER_TICK, Status, World};

/// The longest wait `:health` takes, in minutes: a day.
pub const MOST_MINUTES: u16 = 24 * 60;

/// The most failures of one call `:health loop` takes.
pub const MOST_FAILS: u8 = 20;

/// What `:health` offers after `stall ` and `idle `.
pub const MINUTE_NAMES: [&str; 4] = ["5m", "10m", "20m", "off"];

/// What `:health` offers after `loop `.
pub const FAIL_NAMES: [&str; 4] = ["2", "3", "5", "off"];

/// When imece tells the leader: none, never.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Health {
    /// Minutes a card's worker is idle, waits on an answer, or says
    /// nothing in its turn before its leader hears the card has stalled.
    pub stall: Option<u16>,
    /// Minutes a worker with no card is idle before its leader hears.
    pub idle: Option<u16>,
    /// Failures of one call (a tool on one target) in a turn before the
    /// leader hears the agent may be going round.
    pub fails: Option<u8>,
}

impl Default for Health {
    /// Ten minutes, ten minutes, three failures (the person, 2026-11-08).
    fn default() -> Health {
        Health {
            stall: Some(10),
            idle: Some(10),
            fails: Some(3),
        }
    }
}

/// One of the three things `:health` sets.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Knob {
    Stall,
    Idle,
    Loop,
}

impl Knob {
    pub const ALL: [Knob; 3] = [Knob::Stall, Knob::Idle, Knob::Loop];

    /// Its word on the command line and in prefs.
    pub fn word(self) -> &'static str {
        match self {
            Knob::Stall => "stall",
            Knob::Idle => "idle",
            Knob::Loop => "loop",
        }
    }

    pub fn of(word: &str) -> Option<Knob> {
        Knob::ALL.into_iter().find(|k| k.word() == word)
    }

    /// What the menu offers after it.
    pub fn values(self) -> &'static [&'static str] {
        match self {
            Knob::Stall | Knob::Idle => &MINUTE_NAMES,
            Knob::Loop => &FAIL_NAMES,
        }
    }
}

/// `off`, minutes (`10`, `10m`) or hours (`1h`), one minute to a day.
fn minutes(s: &str) -> Option<Option<u16>> {
    if s == "off" {
        return Some(None);
    }
    let (number, per) = match s.strip_suffix('h') {
        Some(n) => (n, 60),
        None => (s.strip_suffix('m').unwrap_or(s), 1),
    };
    let n: u16 = number.parse().ok()?;
    let m = n.checked_mul(per)?;
    (1..=MOST_MINUTES).contains(&m).then_some(Some(m))
}

/// `off`, or a number of failures from two to [`MOST_FAILS`].
fn failures(s: &str) -> Option<Option<u8>> {
    if s == "off" {
        return Some(None);
    }
    let n: u8 = s.parse().ok()?;
    (2..=MOST_FAILS).contains(&n).then_some(Some(n))
}

fn minutes_name(m: Option<u16>) -> String {
    match m {
        None => "off".into(),
        Some(m) if m % 60 == 0 => format!("{}h", m / 60),
        Some(m) => format!("{m}m"),
    }
}

impl Health {
    /// `knob` set to `value` (`:health stall 15m`); none when the value
    /// is not one it takes.
    pub fn with(self, knob: Knob, value: &str) -> Option<Health> {
        let value = value.trim().to_ascii_lowercase();
        let mut h = self;
        match knob {
            Knob::Stall => h.stall = minutes(&value)?,
            Knob::Idle => h.idle = minutes(&value)?,
            Knob::Loop => h.fails = failures(&value)?,
        }
        Some(h)
    }

    /// One knob's value as `with` takes it.
    pub fn value(self, knob: Knob) -> String {
        match knob {
            Knob::Stall => minutes_name(self.stall),
            Knob::Idle => minutes_name(self.idle),
            Knob::Loop => self.fails.map_or("off".into(), |n| n.to_string()),
        }
    }

    /// All of it, as prefs keep it and `parse` reads it:
    /// `stall 10m idle 10m loop 3`.
    pub fn name(self) -> String {
        (Knob::ALL.iter())
            .map(|&k| format!("{} {}", k.word(), self.value(k)))
            .collect::<Vec<_>>()
            .join(" ")
    }

    /// What `name` wrote; a knob missing keeps its default, anything
    /// else not read refuses the whole.
    pub fn parse(s: &str) -> Option<Health> {
        let words: Vec<&str> = s.split_whitespace().collect();
        if !words.len().is_multiple_of(2) {
            return None;
        }
        let mut h = Health::default();
        for pair in words.chunks(2) {
            h = h.with(Knob::of(pair[0])?, pair[1])?;
        }
        Some(h)
    }
}

/// What imece saw (the journal keeps it; the leader is told in words of
/// its own).
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Sign {
    /// The worker of `card` has been idle `minutes` without a done.
    Stalled { card: CardId, minutes: u16 },
    /// The worker of `card` has waited `minutes` on `to`'s answer.
    Waiting {
        card: CardId,
        to: AgentId,
        minutes: u16,
    },
    /// The agent has said nothing for `minutes` in its turn; `doing` is
    /// its last call, empty when none is known.
    Silent {
        card: Option<CardId>,
        minutes: u16,
        doing: String,
    },
    /// A worker with no card has been idle `minutes`.
    Free { minutes: u16 },
    /// `call` (a tool and its target) failed `times` in one turn.
    Looping { call: String, times: u8 },
}

impl Sign {
    /// The card it is about.
    pub fn card(&self) -> Option<CardId> {
        match *self {
            Sign::Stalled { card, .. } | Sign::Waiting { card, .. } => Some(card),
            Sign::Silent { card, .. } => card,
            Sign::Free { .. } | Sign::Looping { .. } => None,
        }
    }

    /// Whether the leader is woken for it: a free worker costs nothing
    /// and waits for the leader's next turn (the person, 2026-11-08).
    pub fn wakes(&self) -> bool {
        !matches!(self, Sign::Free { .. })
    }
}

/// What imece follows of an agent for its health notices. Memory only.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Watch {
    /// The tick of its newest trace line.
    pub traced_at: u64,
    /// This turn's calls: the newest target of each tool.
    calls: Vec<(String, String)>,
    /// This turn's failures, per call (`tool target`).
    fails: Vec<(String, u8)>,
    /// The quiet already told: the tick it began at (its status change,
    /// or its last trace line while it works).
    told: Option<u64>,
    /// Signs seen as they came (a loop), for the next look.
    due: Vec<Sign>,
}

impl Watch {
    /// The turn is over: its calls and failures go.
    pub fn turn_over(&mut self) {
        self.calls.clear();
        self.fails.clear();
    }
}

/// Whole minutes in `ticks`.
fn minutes_in(ticks: u64) -> u16 {
    (ticks * SECONDS_PER_TICK / 60).min(u64::from(u16::MAX)) as u16
}

/// Ticks in `minutes`.
fn ticks_in(minutes: u16) -> u64 {
    u64::from(minutes) * 60 / SECONDS_PER_TICK
}

impl World {
    /// Agent `id` called `tool` on `target`.
    pub fn tool_called(&mut self, id: AgentId, tool: &str, target: &str) {
        let calls = &mut self.agents[id].watch.calls;
        match calls.iter_mut().find(|(t, _)| t == tool) {
            Some(c) => c.1 = target.to_string(),
            None => calls.push((tool.to_string(), target.to_string())),
        }
    }

    /// Agent `id`'s call of `tool` failed: the newest it made of that
    /// tool. At the person's count of failures in one turn, a sign for
    /// the next look (once a turn and call); none before, after or off.
    pub fn tool_failed(&mut self, id: AgentId, tool: &str) {
        let most = self.health.fails;
        let w = &mut self.agents[id].watch;
        let target = (w.calls.iter())
            .find(|(t, _)| t == tool)
            .map_or("", |(_, target)| target.as_str());
        let call = format!("{tool} {target}").trim_end().to_string();
        let times = match w.fails.iter_mut().find(|(c, _)| *c == call) {
            Some(f) => {
                f.1 = f.1.saturating_add(1);
                f.1
            }
            None => {
                w.fails.push((call.clone(), 1));
                1
            }
        };
        if Some(times) == most {
            self.agents[id]
                .watch
                .due
                .push(Sign::Looping { call, times });
        }
    }

    /// Agent `id`'s sign goes in the journal, and to the caller, which
    /// tells the leader ([`told_of`](Self::told_of)).
    fn sign(&mut self, id: AgentId, sign: Sign) -> (AgentId, Sign) {
        self.note(Event::Health {
            agent: id,
            sign: sign.clone(),
        });
        (id, sign)
    }

    /// The leader that hears of agent `id`; none when that is itself or
    /// there is none (a solo's project).
    pub fn told_of(&self, id: AgentId) -> Option<AgentId> {
        let a = &self.agents[id];
        let leader = self.projects.get(a.project)?.leader;
        (leader != id && self.agents[leader].role == Role::Leader).then_some(leader)
    }

    /// The loops seen since the last look, and the stalls and idle
    /// workers now past the person's minutes, each once until its agent
    /// moves again: the journal has them, and the caller tells the
    /// leaders.
    pub fn look_health(&mut self) -> Vec<(AgentId, Sign)> {
        let (stall, idle) = (self.health.stall, self.health.idle);
        let mut signs = Vec::new();
        for id in 0..self.agents.len() {
            for sign in std::mem::take(&mut self.agents[id].watch.due) {
                signs.push(self.sign(id, sign));
            }
            let Some((since, sign)) = self.quiet(id, stall, idle) else {
                continue;
            };
            if self.agents[id].watch.told == Some(since) {
                continue;
            }
            self.agents[id].watch.told = Some(since);
            signs.push(self.sign(id, sign));
        }
        signs
    }

    /// Whether agent `id` has been quiet long enough to tell: since when,
    /// and what of.
    fn quiet(&self, id: AgentId, stall: Option<u16>, idle: Option<u16>) -> Option<(u64, Sign)> {
        let a = &self.agents[id];
        if a.gone
            || self.put_away(id)
            || self.told_of(id).is_none()
            || self.waits_on_person(id)
            || a.starting
            || a.retry.is_some()
            // A blocked worker's leader has heard, from the worker
            // (record 143).
            || a.block.is_some()
        {
            return None;
        }
        let card = a.card.filter(|&c| {
            let card = &self.cards[c];
            card.column == Column::Working && card.owner == Some(id)
        });
        let past = |since: u64, minutes: Option<u16>| {
            let quiet = self.tick.saturating_sub(since);
            minutes.is_some_and(|m| quiet >= ticks_in(m))
        };
        let quiet = minutes_in(self.tick.saturating_sub(a.changed_at));
        match (a.status, card) {
            (Status::Working, _) => past(a.watch.traced_at, stall).then(|| {
                let minutes = minutes_in(self.tick - a.watch.traced_at);
                let doing = a.activity.clone().unwrap_or_default();
                let sign = Sign::Silent {
                    card,
                    minutes,
                    doing,
                };
                (a.watch.traced_at, sign)
            }),
            // A helper's work is its worker's to wait on.
            (_, _) if a.role != Role::Worker => None,
            // Its helpers work for it.
            (_, Some(_)) if self.helpers_of(id).any(|h| h.status.busy()) => None,
            (Status::Idle, Some(card)) => past(a.changed_at, stall).then_some((
                a.changed_at,
                Sign::Stalled {
                    card,
                    minutes: quiet,
                },
            )),
            (Status::Asking { to, .. }, Some(card)) => past(a.changed_at, stall).then_some((
                a.changed_at,
                Sign::Waiting {
                    card,
                    to,
                    minutes: quiet,
                },
            )),
            (Status::Idle, None) if a.card.is_none_or(|c| self.cards[c].over()) => {
                past(a.changed_at, idle).then_some((a.changed_at, Sign::Free { minutes: quiet }))
            }
            _ => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::live::testing::{team, with_card};
    use crate::model::{Agent, EntryKind, Role};

    /// What the journal says of health, oldest first.
    fn told(w: &World) -> Vec<(AgentId, Sign)> {
        (w.journal.iter())
            .filter_map(|e| match &e.event {
                Event::Health { agent, sign } => Some((*agent, sign.clone())),
                _ => None,
            })
            .collect()
    }

    /// A team of two workers, the first idle on its card since tick 0.
    fn stalled() -> (World, CardId) {
        let mut w = team(2);
        let c = with_card(&mut w, 1);
        w.agents[1].status = Status::Idle;
        w.agents[2].status = Status::Idle;
        w.agents[2].trace(0, "x");
        // The second has work of its own: only the first is looked at.
        let d = with_card(&mut w, 2);
        w.cards[d].column = Column::Review;
        (w, c)
    }

    #[test]
    fn a_card_whose_worker_is_idle_is_told_once_at_the_minutes() {
        let (mut w, c) = stalled();
        w.tick = 119;
        assert_eq!(w.look_health(), vec![]);
        w.tick = 120;
        let sign = Sign::Stalled {
            card: c,
            minutes: 10,
        };
        assert_eq!(w.look_health(), vec![(1, sign.clone())]);
        assert_eq!(told(&w), vec![(1, sign)]);
        w.tick = 500;
        assert_eq!(w.look_health(), vec![], "once, until it moves");
        // It moves and stops again: told again, in minutes since then.
        w.agents[1].set_status(Status::Working, 501);
        w.agents[1].trace(501, "x");
        w.agents[1].set_status(Status::Idle, 502);
        w.tick = 502 + 119;
        assert_eq!(w.look_health(), vec![]);
        w.tick = 502 + 144;
        assert_eq!(
            w.look_health(),
            vec![(
                1,
                Sign::Stalled {
                    card: c,
                    minutes: 12
                }
            )]
        );
        // The stall's minutes, not the idle worker's.
        for (stall, idle, at, signs) in [
            (Some(20), Some(10), 239, 0),
            (Some(20), Some(10), 240, 1),
            (Some(10), None, 120, 1),
            (None, Some(10), 9_999, 0),
        ] {
            let (mut w, _) = stalled();
            w.health = Health {
                stall,
                idle,
                fails: None,
            };
            w.tick = at;
            assert_eq!(w.look_health().len(), signs, "{stall:?} {idle:?} {at}");
        }
    }

    #[test]
    fn a_card_waiting_on_an_answer_is_told_as_one() {
        let (mut w, c) = stalled();
        w.agents[1].status = Status::Asking { to: 2, until: 999 };
        w.tick = 119;
        assert_eq!(w.look_health(), vec![]);
        w.tick = 120;
        let sign = Sign::Waiting {
            card: c,
            to: 2,
            minutes: 10,
        };
        assert_eq!(w.look_health(), vec![(1, sign)]);
        w.health.stall = None;
        let (mut w, _) = stalled();
        w.agents[1].status = Status::Asking { to: 2, until: 999 };
        w.health.stall = None;
        w.tick = 9_999;
        assert_eq!(w.look_health(), vec![]);
    }

    #[test]
    fn an_agent_that_says_nothing_in_its_turn_is_told_by_its_last_line() {
        let mut w = team(1);
        let c = with_card(&mut w, 1);
        w.agents[1].trace(5, "tool_use Bash cargo test");
        w.agents[1].activity = Some("Bash cargo test".into());
        w.tick = 124;
        assert_eq!(w.look_health(), vec![]);
        w.tick = 125;
        let sign = Sign::Silent {
            card: Some(c),
            minutes: 10,
            doing: "Bash cargo test".into(),
        };
        assert_eq!(w.look_health(), vec![(1, sign)]);
        w.tick = 300;
        assert_eq!(w.look_health(), vec![], "once");
        // A line, and as long again without one: told again.
        w.agents[1].trace(300, "x");
        w.agents[1].activity = None;
        w.cards[c].column = Column::Review;
        w.tick = 300 + 132;
        let sign = Sign::Silent {
            card: None,
            minutes: 11,
            doing: String::new(),
        };
        assert_eq!(w.look_health(), vec![(1, sign)]);
        // Off, nothing; a helper is told of too, the leader never.
        let mut w = team(1);
        w.health.stall = None;
        w.tick = 9_999;
        assert_eq!(w.look_health(), vec![]);
        let mut w = team(1);
        let mut helper = Agent::new(2, 0, "h".into(), Role::Helper, Status::Working, 0);
        helper.parent = Some(1);
        w.agents.push(helper);
        w.agents[0].status = Status::Working;
        w.agents[1].trace(200, "x");
        w.tick = 200;
        let signs = w.look_health();
        assert_eq!(signs.len(), 1, "{signs:?}");
        assert_eq!(signs[0].0, 2);
    }

    #[test]
    fn a_worker_with_no_card_is_told_at_the_idle_minutes() {
        let mut w = team(1);
        w.agents[1].status = Status::Idle;
        w.health.stall = Some(1);
        w.tick = 119;
        assert_eq!(w.look_health(), vec![]);
        w.tick = 120;
        assert_eq!(w.look_health(), vec![(1, Sign::Free { minutes: 10 })]);
        // A card over is none; one held for it or with the person is
        // work.
        for (column, owner, told) in [
            (Column::Done, Some(1), 1),
            (Column::Withdrawn, Some(1), 1),
            (Column::Failed, None, 1),
            (Column::Review, Some(1), 0),
            (Column::Verifying, Some(1), 0),
            (Column::Waiting, Some(1), 0),
            (Column::Working, Some(2), 0),
        ] {
            let mut w = team(1);
            w.agents[1].status = Status::Idle;
            let c = with_card(&mut w, 1);
            (w.cards[c].column, w.cards[c].owner) = (column, owner);
            w.health.stall = None;
            w.tick = 120;
            assert_eq!(w.look_health().len(), told, "{column:?} {owner:?}");
        }
        let mut w = team(1);
        w.agents[1].status = Status::Idle;
        w.health.idle = None;
        w.tick = 9_999;
        assert_eq!(w.look_health(), vec![]);
        // Idle helpers and leaders are no one's to give work.
        let mut w = team(0);
        let mut helper = Agent::new(1, 0, "h".into(), Role::Helper, Status::Idle, 0);
        helper.parent = Some(0);
        w.agents.push(helper);
        w.tick = 9_999;
        assert_eq!(w.look_health(), vec![]);
    }

    #[test]
    fn what_waits_on_the_person_or_goes_on_by_itself_is_not_told() {
        let stalled_with = |f: &dyn Fn(&mut World)| {
            let (mut w, _) = stalled();
            f(&mut w);
            w.tick = 9_999;
            w.look_health()
        };
        assert_eq!(stalled_with(&|_| {}).len(), 1, "the fixture stalls");
        let quiet: [&dyn Fn(&mut World); 7] = [
            &|w| w.asks_you(1, "rm -rf target"),
            &|w| w.agents[1].gone = true,
            &|w| w.projects[0].closed = true,
            &|w| w.agents[1].starting = true,
            &|w| {
                w.agents[1].retry = Some(crate::model::Retry {
                    attempt: 1,
                    at: 99_999,
                })
            },
            &|w| w.agents[0].role = Role::Solo,
            &|w| {
                let mut helper = Agent::new(3, 0, "h".into(), Role::Helper, Status::Working, 0);
                helper.parent = Some(1);
                helper.trace(9_999, "x");
                w.agents.push(helper);
            },
        ];
        for (i, f) in quiet.iter().enumerate() {
            assert_eq!(stalled_with(*f), vec![], "case {i}");
        }
        // An idle helper does not hold its worker up.
        let signs = stalled_with(&|w| {
            let mut helper = Agent::new(3, 0, "h".into(), Role::Helper, Status::Idle, 0);
            helper.parent = Some(1);
            w.agents.push(helper);
        });
        assert_eq!(signs.len(), 1, "{signs:?}");
    }

    #[test]
    fn a_call_that_keeps_failing_in_a_turn_is_told_once_at_the_count() {
        let mut w = team(1);
        let fail = |w: &mut World, n| {
            for _ in 0..n {
                w.tool_failed(1, "Bash");
            }
        };
        w.tool_called(1, "Bash", "cargo test");
        w.tool_called(1, "Read", "a.rs");
        fail(&mut w, 2);
        assert_eq!(w.look_health(), vec![]);
        fail(&mut w, 1);
        let sign = Sign::Looping {
            call: "Bash cargo test".into(),
            times: 3,
        };
        assert_eq!(w.look_health(), vec![(1, sign.clone())]);
        assert_eq!(told(&w), vec![(1, sign)]);
        fail(&mut w, 5);
        assert_eq!(w.look_health(), vec![], "once a turn and call");
        // Another call counts apart, by its newest target.
        w.tool_called(1, "Bash", "ls");
        fail(&mut w, 3);
        let sign = Sign::Looping {
            call: "Bash ls".into(),
            times: 3,
        };
        assert_eq!(w.look_health(), vec![(1, sign)]);
        // A new turn starts again.
        w.agents[1].watch.turn_over();
        w.tool_called(1, "Bash", "ls");
        fail(&mut w, 2);
        assert_eq!(w.look_health(), vec![]);
        // A tool whose call is not known is named alone; the count is
        // the person's; off, never; the leader's own is in the journal.
        let mut w = team(1);
        w.health.fails = Some(2);
        for _ in 0..2 {
            w.tool_failed(0, "Read");
        }
        let sign = Sign::Looping {
            call: "Read".into(),
            times: 2,
        };
        assert_eq!(w.look_health(), vec![(0, sign)]);
        let mut w = team(1);
        w.health.fails = None;
        fail(&mut w, 30);
        assert_eq!(w.look_health(), vec![]);
    }

    #[test]
    fn the_leader_hears_of_its_team_and_the_journal_files_it() {
        let w = team(2);
        assert_eq!(w.told_of(1), Some(0));
        assert_eq!(w.told_of(0), None, "not of itself");
        let mut solo = team(1);
        solo.agents[0].role = Role::Solo;
        assert_eq!(solo.told_of(1), None);
        let free = Event::Health {
            agent: 2,
            sign: Sign::Free { minutes: 1 },
        };
        assert_eq!(free.kind(), EntryKind::Assumption);
        assert_eq!(free.agent(), 2);
        assert_eq!(free.card(), None);
        let stalled = Event::Health {
            agent: 1,
            sign: Sign::Stalled {
                card: 4,
                minutes: 1,
            },
        };
        assert_eq!(stalled.kind(), EntryKind::Blocked);
        assert_eq!(stalled.card(), Some(4));
    }

    #[test]
    fn the_knobs_read_minutes_hours_failures_and_off() {
        let h = Health::default();
        assert_eq!(h.name(), "stall 10m idle 10m loop 3");
        assert_eq!(Health::parse(&h.name()), Some(h));
        let set = |k, v: &str| h.with(k, v);
        assert_eq!(set(Knob::Stall, "15").unwrap().stall, Some(15));
        assert_eq!(set(Knob::Stall, "15m").unwrap().stall, Some(15));
        assert_eq!(set(Knob::Idle, "2h").unwrap().idle, Some(120));
        assert_eq!(set(Knob::Idle, " OFF ").unwrap().idle, None);
        assert_eq!(set(Knob::Stall, "1").unwrap().stall, Some(1));
        assert_eq!(set(Knob::Stall, "1440").unwrap().stall, Some(1440));
        assert_eq!(set(Knob::Stall, "24h").unwrap().stall, Some(1440));
        for no in ["0", "1441", "25h", "x", "", "-1", "99999h"] {
            assert_eq!(set(Knob::Stall, no), None, "{no:?}");
        }
        assert_eq!(set(Knob::Loop, "2").unwrap().fails, Some(2));
        assert_eq!(set(Knob::Loop, "20").unwrap().fails, Some(20));
        assert_eq!(set(Knob::Loop, "off").unwrap().fails, None);
        for no in ["1", "21", "3m", "x"] {
            assert_eq!(set(Knob::Loop, no), None, "{no:?}");
        }
        let all = Health {
            stall: Some(90),
            idle: None,
            fails: Some(5),
        };
        assert_eq!(all.name(), "stall 90m idle off loop 5");
        assert_eq!(Health::parse(&all.name()), Some(all));
        assert_eq!(
            Health {
                stall: Some(120),
                ..all
            }
            .value(Knob::Stall),
            "2h"
        );
        // What is missing keeps its default; what is wrong refuses all.
        assert_eq!(
            Health::parse("idle off"),
            Some(Health {
                idle: None,
                ..Health::default()
            })
        );
        assert_eq!(Health::parse(""), Some(Health::default()));
        for no in ["idle", "idle off loop", "sleep 3m", "loop 1"] {
            assert_eq!(Health::parse(no), None, "{no:?}");
        }
        for k in Knob::ALL {
            assert_eq!(Knob::of(k.word()), Some(k));
            for v in k.values() {
                assert!(h.with(k, v).is_some(), "{v}");
            }
        }
        assert_eq!(Knob::of("x"), None);
    }

    #[test]
    fn minutes_and_ticks_go_both_ways() {
        assert_eq!(ticks_in(10), 120);
        assert_eq!(ticks_in(1), 12);
        assert_eq!(minutes_in(120), 10);
        assert_eq!(minutes_in(131), 10);
        assert_eq!(minutes_in(132), 11);
        assert_eq!(minutes_in(u64::MAX / 10), u16::MAX);
    }

    #[test]
    fn only_a_free_worker_waits_for_the_leader_s_next_turn() {
        assert!(!Sign::Free { minutes: 1 }.wakes());
        for s in [
            Sign::Stalled {
                card: 0,
                minutes: 1,
            },
            Sign::Waiting {
                card: 0,
                to: 0,
                minutes: 1,
            },
            Sign::Silent {
                card: None,
                minutes: 1,
                doing: String::new(),
            },
            Sign::Looping {
                call: "Bash".into(),
                times: 3,
            },
        ] {
            assert!(s.wakes(), "{s:?}");
        }
        let silent = |card| Sign::Silent {
            card,
            minutes: 1,
            doing: String::new(),
        };
        assert_eq!(silent(Some(4)).card(), Some(4));
        assert_eq!(silent(None).card(), None);
        assert_eq!(
            Sign::Stalled {
                card: 2,
                minutes: 1
            }
            .card(),
            Some(2)
        );
        let waiting = Sign::Waiting {
            card: 3,
            to: 0,
            minutes: 1,
        };
        assert_eq!(waiting.card(), Some(3));
        assert_eq!(Sign::Free { minutes: 1 }.card(), None);
    }
}
