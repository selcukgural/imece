//! What a stand-in leader does with the person's task (record 130): with
//! the habit of cutting it into cards, a card per file of its directory
//! the task names best, each owning that file and its test. It hands the
//! cards out in their order: to the workers it has, then to workers it
//! hires; when imece holds a hire for a plan (record 114) it puts the
//! cards left before the person with the `plan` tool, ends its turn, and
//! goes on with them at the person's yes; when the team is full it posts
//! what is left on the board, for an idle worker to take.

use std::collections::VecDeque;

use serde_json::{Value, json};

use super::work::{code_for, test_for};
use super::{Hands, Mind, first_line};

/// How imece tells a leader the person's answer to its plan, as far as
/// the stand-in reads it (`PLAN_YES`, `PLAN_NO` in the screen's crate).
pub const PLAN_YES: &str = "The person said yes to your plan";
pub const PLAN_NO: &str = "The person said no to your plan";
/// What imece wakes an agent with when it has only notes for it (`GO_ON`
/// in the screen's crate): no task of the person's.
pub const GO_ON: &str = "Go on with your task.";

/// The most cards a task is cut into, whatever the habits say.
pub const CARDS_MOST: u64 = 8;
/// The longest title of a card, in characters (imece's `MAX_TITLE`).
pub const TITLE_MAX: usize = 120;
/// What a plan says a card costs, in cents.
pub const CARD_CENTS: u64 = 5;

/// A card a leader hands out: its title, its task and the paths it owns.
#[derive(Clone, Debug, PartialEq)]
pub struct Part {
    pub title: String,
    pub body: String,
    pub paths: Vec<String>,
}

impl Part {
    pub fn to_json(&self) -> Value {
        let mut card = json!({ "title": self.title, "body": self.body, "paths": self.paths });
        if let Some(n) = closes_of(&self.body) {
            card["closes"] = json!(n);
        }
        card
    }

    /// What a transcript kept; none without a title or a task.
    pub fn from_json(v: &Value) -> Option<Part> {
        let text = |v: &Value| v.as_str().map(String::from);
        Some(Part {
            title: text(&v["title"])?,
            body: text(&v["body"])?,
            paths: (v["paths"].as_array().into_iter().flatten())
                .filter_map(text)
                .collect(),
        })
    }
}

/// Task `task` cut into at most `most` cards over `files`: one per file
/// of code, those the task names best first, each owning its file and
/// the test that names it (a test no earlier card owns), its task the
/// file and the person's words. None when fewer than two can be made.
pub fn parts(files: &[String], task: &str, most: u64) -> Vec<Part> {
    let most = usize::try_from(most.min(CARDS_MOST)).unwrap_or(0);
    let code = code_for(files, task);
    if most < 2 || code.len() < 2 {
        return Vec::new();
    }
    let mut taken: Vec<&str> = Vec::new();
    (code.into_iter().take(most))
        .map(|main| {
            let test = test_for(files, main).filter(|t| !taken.contains(t));
            taken.extend(test);
            let body = format!("{main}: {task}");
            Part {
                title: first_line(&body).chars().take(TITLE_MAX).collect(),
                body,
                paths: std::iter::once(main)
                    .chain(test)
                    .map(String::from)
                    .collect(),
            }
        })
        .collect()
}

/// What became of a task's cards in a turn, for its last words.
#[derive(Debug, Default, PartialEq)]
struct Handed {
    /// Who got which card, by its title.
    given: Vec<(String, String)>,
    posted: usize,
    planned: usize,
    /// What did not go, each with why.
    failed: Vec<String>,
}

impl Handed {
    fn words(&self) -> String {
        let mut said: Vec<String> = (self.given.iter())
            .map(|(who, title)| format!("{who} has {title}"))
            .collect();
        if self.posted > 0 {
            said.push(format!("{} on the board", cards(self.posted)));
        }
        if self.planned > 0 {
            let n = self.planned;
            let what = if n == 1 { "card" } else { "cards" };
            said.push(format!("my plan for {n} more {what} waits for your yes"));
        }
        said.extend(self.failed.iter().cloned());
        if said.is_empty() {
            return "Nothing to hand out.".into();
        }
        format!("{}.", said.join("; "))
    }
}

pub(super) fn cards(n: usize) -> String {
    match n {
        1 => "1 card".into(),
        n => format!("{n} cards"),
    }
}

impl Mind {
    /// The cards `parts` handed out in their order: to the workers the
    /// leader has (with `own`), then to ones it hires; the ones left go
    /// in a plan when imece holds the hire for one, on the board when
    /// nobody more can be hired. What it says of it.
    pub(super) fn hand_out(
        &mut self,
        hands: &mut impl Hands,
        parts: Vec<Part>,
        own: bool,
    ) -> String {
        let mut free: VecDeque<String> = if own {
            self.hired.iter().cloned().collect()
        } else {
            VecDeque::new()
        };
        let mut parts = VecDeque::from(parts);
        let mut handed = Handed::default();
        while let Some(part) = parts.pop_front() {
            hands.pause();
            let who = match free.pop_front() {
                Some(name) => name,
                None => match hands.call("hire", json!({})) {
                    Ok(said) => {
                        let name = said.split(' ').next().unwrap_or_default().to_string();
                        self.hired.push(name.clone());
                        name
                    }
                    Err(why) if self.hired.is_empty() => {
                        return format!("I could not hire: {why}");
                    }
                    Err(why) => {
                        parts.push_front(part);
                        let left: Vec<Part> = parts.into();
                        if why.contains("plan") {
                            self.propose(hands, left, &mut handed);
                        } else {
                            post(hands, &left, &mut handed);
                        }
                        break;
                    }
                },
            };
            let mut assign = json!({
                "to": who, "type": "assign", "body": part.body, "paths": part.paths,
            });
            if let Some(n) = closes_of(&part.body) {
                assign["closes"] = json!(n);
            }
            match hands.call("send", assign) {
                Ok(_) => {
                    hands.say(&format!("I gave {who} {}.", part.paths.join(" and ")));
                    handed.given.push((who, part.title));
                }
                Err(why) => {
                    (handed.failed).push(format!("{who} did not get {}: {why}", part.title))
                }
            }
        }
        handed.words()
    }

    /// The cards `left` put before the person as a plan, kept until the
    /// answer; on the board when the plan is refused.
    fn propose(&mut self, hands: &mut impl Hands, left: Vec<Part>, handed: &mut Handed) {
        let listed: Vec<Value> = left.iter().map(Part::to_json).collect();
        let cost = (left.len() as u64 * CARD_CENTS) as f64 / 100.0;
        match hands.call("plan", json!({ "cards": listed, "cost": cost })) {
            Ok(_) => {
                handed.planned = left.len();
                self.planned = left;
            }
            Err(why) => {
                handed.failed.push(format!("my plan was refused: {why}"));
                post(hands, &left, handed);
            }
        }
    }
}

/// The issue a card answers, as the person's cue `[closes N]` names it
/// (record 142): the leader passes it on as the card's `closes`.
pub(super) fn closes_of(task: &str) -> Option<u64> {
    let rest = task.split("[closes ").nth(1)?;
    let (n, _) = rest.split_once(']')?;
    n.trim().parse().ok().filter(|&n| n > 0)
}

/// The cards `left` posted on the board.
fn post(hands: &mut impl Hands, left: &[Part], handed: &mut Handed) {
    for part in left {
        match hands.call("post", part.to_json()) {
            Ok(_) => handed.posted += 1,
            Err(why) => (handed.failed).push(format!("{} is not on the board: {why}", part.title)),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::super::tests::{LEADER, fake, strings};
    use super::*;

    #[test]
    fn the_person_s_cue_names_the_issue_a_card_closes() {
        assert_eq!(closes_of("mend it [closes 58]"), Some(58));
        assert_eq!(closes_of("[closes  7 ] mend"), Some(7));
        assert_eq!(closes_of("mend it"), None);
        assert_eq!(closes_of("mend [closes 0]"), None);
        assert_eq!(closes_of("mend [closes x]"), None);
        assert_eq!(closes_of("mend [closes 5"), None);
        let part = |body: &str| Part {
            title: "t".into(),
            body: body.into(),
            paths: vec![],
        };
        assert_eq!(part("a [closes 3]").to_json()["closes"], 3);
        assert_eq!(part("a").to_json().get("closes"), None);
    }

    #[test]
    fn a_card_handed_out_carries_the_issue_the_person_named() {
        let mut f = fake(LEADER);
        files(&mut f, REPO);
        f.first = vec![("hire".into(), joined("claude-1"))];
        Mind::default().turn(&mut f, "From imece: x\n\nRound up [closes 9]");
        let sent = (f.calls.iter())
            .find(|(tool, _)| tool == "send")
            .expect("an assign");
        assert_eq!(sent.1["closes"], 9);
        assert_eq!(sent.1["type"], "assign");
    }

    fn files(f: &mut super::super::tests::Fake, names: &[&str]) {
        for n in names {
            f.files.insert(n.to_string(), String::new());
        }
    }

    const REPO: &[&str] = &[
        "src/alpha.rs",
        "src/alpha_test.rs",
        "src/beta.rs",
        "src/delta.rs",
        "src/gamma.rs",
        "notes.json",
    ];

    fn part(main: &str, test: Option<&str>) -> Part {
        Part {
            title: format!("{main}: Round up"),
            body: format!("{main}: Round up\nwell"),
            paths: std::iter::once(main)
                .chain(test)
                .map(String::from)
                .collect(),
        }
    }

    #[test]
    fn a_task_is_cut_into_a_card_per_file_each_owning_it_and_its_test() {
        let all = strings(REPO);
        let task = "Round up\nwell";
        assert_eq!(
            parts(&all, task, 3),
            [
                part("src/alpha.rs", Some("src/alpha_test.rs")),
                part("src/beta.rs", None),
                part("src/delta.rs", None),
            ]
        );
        assert_eq!(parts(&all, task, 9).len(), 4, "a card per file of code");
        assert_eq!(parts(&all, task, 2).len(), 2);
        // What the task names comes first.
        assert_eq!(parts(&all, "Mend gamma.rs", 2)[0].paths, ["src/gamma.rs"]);
        // Under two cards, or two files: none, the task goes whole.
        assert_eq!(parts(&all, task, 1), []);
        assert_eq!(parts(&all, task, 0), []);
        assert_eq!(
            parts(&strings(&["a.rs", "a_test.rs", "b.json"]), task, 3),
            []
        );
        // At most CARDS_MOST, whatever the habits say.
        let many: Vec<String> = (0..12).map(|n| format!("f{n:02}.rs")).collect();
        assert_eq!(parts(&many, task, u64::MAX).len(), 8);
        // A test two files name is the first one's alone.
        let shared = strings(&["money.rs", "money_fmt.rs", "money_test.rs"]);
        let cut = parts(&shared, task, 2);
        assert_eq!(cut[0].paths, ["money.rs", "money_test.rs"]);
        assert_eq!(cut[1].paths, ["money_fmt.rs"]);
        // A title is one line of at most TITLE_MAX characters.
        let long = "é".repeat(200);
        let cut = parts(&all, &format!("{long}\nmore"), 2);
        assert_eq!(cut[0].title.chars().count(), 120);
        assert!(cut[0].title.starts_with("src/alpha.rs: éé"));
        assert_eq!(cut[0].body, format!("src/alpha.rs: {long}\nmore"));
    }

    #[test]
    fn a_card_is_kept_as_json_and_one_without_its_text_is_left_out() {
        let p = part("src/alpha.rs", Some("src/alpha_test.rs"));
        assert_eq!(Part::from_json(&p.to_json()), Some(p.clone()));
        assert_eq!(
            p.to_json(),
            json!({ "title": "src/alpha.rs: Round up", "body": "src/alpha.rs: Round up\nwell",
                    "paths": ["src/alpha.rs", "src/alpha_test.rs"] })
        );
        assert_eq!(Part::from_json(&json!({ "title": "t", "paths": [] })), None);
        assert_eq!(Part::from_json(&json!({ "body": "b" })), None);
        assert_eq!(
            Part::from_json(&json!({ "title": "t", "body": "b", "paths": ["a", 3] })),
            Some(Part {
                title: "t".into(),
                body: "b".into(),
                paths: vec!["a".into()]
            })
        );
    }

    fn joined(name: &str) -> Result<String, String> {
        Ok(format!("{name} joined (Claude Code, sonnet)."))
    }

    const NO_PLAN: &str = "make a plan first (the plan tool): the person reads it";
    const FULL: &str = "the team is full: 2 workers";

    #[test]
    fn the_person_s_cue_cuts_the_task_into_its_cards_whatever_the_habit() {
        let mut f = fake(LEADER);
        files(&mut f, REPO);
        f.first = vec![
            ("hire".into(), joined("claude-1")),
            ("hire".into(), joined("claude-2")),
        ];
        let said = Mind::default().turn(&mut f, "From imece: x\n\nRound up [cards 2]");
        assert_eq!(
            said,
            "claude-1 has src/alpha.rs: Round up [cards 2]; claude-2 has src/beta.rs: Round up \
             [cards 2]."
        );
        // Without it and without the habit the task goes whole.
        let mut f = fake(LEADER);
        files(&mut f, REPO);
        f.first = vec![("hire".into(), joined("claude-1"))];
        let said = Mind::default().turn(&mut f, "From imece: x\n\nRound up");
        assert_eq!(said, "claude-1 has it: Round up");
    }

    #[test]
    fn a_leader_hires_one_plans_the_rest_and_hands_them_out_at_the_yes() {
        let mut f = fake(LEADER);
        files(&mut f, REPO);
        f.habits.cards = 4;
        f.first = vec![
            ("hire".into(), joined("claude-1")),
            ("hire".into(), Err(NO_PLAN.into())),
        ];
        let mut m = Mind::default();
        let said = m.turn(&mut f, "From imece: x\n\nRound up\nwell");
        assert_eq!(
            said,
            "claude-1 has src/alpha.rs: Round up; my plan for 3 more cards waits for your yes."
        );
        assert_eq!(
            f.said,
            strings(&["I gave claude-1 src/alpha.rs and src/alpha_test.rs."])
        );
        assert_eq!(f.pauses, 2, "before each card it tries to hand out");
        let left = vec![
            part("src/beta.rs", None),
            part("src/delta.rs", None),
            part("src/gamma.rs", None),
        ];
        assert_eq!(m.planned, left);
        assert_eq!(
            f.calls[1..],
            [
                ("hire".into(), json!({})),
                (
                    "send".into(),
                    json!({ "to": "claude-1", "type": "assign",
                            "body": "src/alpha.rs: Round up\nwell",
                            "paths": ["src/alpha.rs", "src/alpha_test.rs"] })
                ),
                ("hire".into(), json!({})),
                (
                    "plan".into(),
                    json!({ "cards": left.iter().map(Part::to_json).collect::<Vec<_>>(),
                            "cost": 0.15 })
                ),
            ]
        );
        // Mail, or a wake with nothing in it, leaves the plan waiting.
        f.calls.clear();
        assert_eq!(
            m.turn(
                &mut f,
                &format!("{}:\n\n[done 4] from claude-1:\nok", super::super::MAIL)
            ),
            "claude-1 said done."
        );
        assert_eq!(
            m.turn(&mut f, "From imece: note\n\nGo on with your task."),
            "Nothing new for me to do."
        );
        assert_eq!((m.planned.len(), f.calls.len()), (3, 0));
        // The yes, with mail beside it: a worker hired for the next card,
        // and with the team full the rest on the board; the workers it
        // has keep their cards.
        f.first = vec![
            ("hire".into(), joined("claude-2")),
            ("hire".into(), Err(FULL.into())),
            ("post".into(), Err("the board has 20 open cards".into())),
        ];
        let yes = format!(
            "From imece:\n- {PLAN_YES}: hire and assign as planned.\n\n{}:\n\n[done 4] from \
             claude-1:\nok",
            super::super::MAIL
        );
        assert_eq!(
            m.turn(&mut f, &yes),
            "claude-2 has src/beta.rs: Round up; 1 card on the board; src/delta.rs: Round up is not \
             on the board: the board has 20 open cards."
        );
        assert_eq!(m.planned, []);
        assert_eq!(m.hired, strings(&["claude-1", "claude-2"]));
        let called: Vec<&str> = f.calls.iter().map(|(t, _)| t.as_str()).collect();
        assert_eq!(called, ["hire", "send", "hire", "post", "post"]);
        assert_eq!(f.calls[1].1["to"], "claude-2");
        assert_eq!(f.calls[1].1["paths"], json!(["src/beta.rs"]));
        assert_eq!(f.calls[4].1, part("src/gamma.rs", None).to_json());
    }

    #[test]
    fn a_no_drops_the_plan_and_a_plan_refused_goes_on_the_board() {
        let mut f = fake(LEADER);
        files(&mut f, REPO);
        f.habits.cards = 3;
        f.first = vec![
            ("hire".into(), joined("claude-1")),
            ("hire".into(), Err(NO_PLAN.into())),
        ];
        let mut m = Mind::default();
        m.turn(&mut f, "Round up");
        assert_eq!(m.planned.len(), 2);
        f.calls.clear();
        let no = format!("From imece:\n- {PLAN_NO}: ask them\n\nGo on with your task.");
        assert_eq!(m.turn(&mut f, &no), "I dropped my plan of 2 cards.");
        assert_eq!((m.planned.len(), f.calls.len()), (0, 0));
        // With no plan waiting, a yes is no plan's: only a wake.
        let yes = format!("From imece:\n- {PLAN_YES}\n\nGo on with your task.");
        assert_eq!(m.turn(&mut f, &yes), "Nothing new for me to do.");
        // The next task goes to the worker it has first; the plan
        // refused, what is left is posted.
        f.first = vec![
            ("hire".into(), Err(NO_PLAN.into())),
            ("plan".into(), Err("cost must be a number".into())),
        ];
        assert_eq!(
            m.turn(&mut f, "Round down"),
            "claude-1 has src/alpha.rs: Round down; 2 cards on the board; my plan was refused: \
             cost must be a number."
        );
        assert_eq!(m.planned, []);
        let called: Vec<&str> = f.calls.iter().map(|(t, _)| t.as_str()).collect();
        assert_eq!(called, ["send", "hire", "plan", "post", "post"]);
    }

    #[test]
    fn a_leader_that_can_hire_nobody_or_hand_nothing_on_says_so() {
        let mut f = fake(LEADER);
        files(&mut f, REPO);
        f.habits.cards = 2;
        f.answers
            .insert("hire".into(), Err("the pot is empty".into()));
        let mut m = Mind::default();
        assert_eq!(
            m.turn(&mut f, "Round up"),
            "I could not hire: the pot is empty"
        );
        assert_eq!(
            f.calls.len(),
            2,
            "whoami and the hire: nothing posted or planned"
        );
        // A worker that does not take its card: said, and the next goes on.
        let mut f = fake(LEADER);
        files(&mut f, REPO);
        f.habits.cards = 2;
        f.first = vec![
            ("hire".into(), joined("claude-1")),
            ("hire".into(), joined("claude-2")),
            ("send".into(), Err("an open card owns src/alpha.rs".into())),
        ];
        assert_eq!(
            Mind::default().turn(&mut f, "Round up"),
            "claude-2 has src/beta.rs: Round up; claude-1 did not get src/alpha.rs: Round up: an \
             open card owns src/alpha.rs."
        );
        assert_eq!(f.said, strings(&["I gave claude-2 src/beta.rs."]));
        assert_eq!(Handed::default().words(), "Nothing to hand out.");
    }

    #[test]
    fn a_leader_without_the_habit_or_the_files_gives_the_task_whole() {
        for (cards, names) in [
            (0, REPO),
            (1, REPO),
            (4, &["src/alpha.rs", "notes.json"][..]),
        ] {
            let mut f = fake(LEADER);
            files(&mut f, names);
            f.habits.cards = cards;
            f.answers.insert("hire".into(), joined("claude-1"));
            assert_eq!(
                Mind::default().turn(&mut f, "Round up"),
                "claude-1 has it: Round up",
                "{cards}"
            );
            assert_eq!(
                f.calls[2].1,
                json!({ "to": "claude-1", "type": "assign", "body": "Round up" })
            );
        }
    }
}
