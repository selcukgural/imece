//! Completion for the composer, the way a shell does it: what is being
//! typed is matched against what imece knows (agent names after `@`,
//! imece's own commands after `:`, a vendor's command line after `/`),
//! the matches are shown while typing, and `tab` (or the arrows) steps
//! through them with the text following. Pure: the screen asks `suggest`
//! after every change and keeps the `Menu` it gets.

use crate::model::{Agent, ProjectId, World};

/// The commands imece itself takes after `:`.
pub const COMMANDS: [&str; 2] = ["budget", "lead"];

/// What `:lead` takes.
pub const LEADS: [&str; 4] = ["claude", "codex", "gemini", "ask"];

/// The matches for the word being typed.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Menu {
    /// Where the word starts in the input (a byte offset).
    pub start: usize,
    pub items: Vec<String>,
    /// The highlighted item.
    pub chosen: usize,
    /// What follows an inserted item: a space when something comes next.
    pub tail: &'static str,
    /// Whether the highlighted item is in the input already (then `tab`
    /// steps on instead of inserting).
    pub inserted: bool,
}

impl Menu {
    fn new(start: usize, items: Vec<String>, tail: &'static str) -> Option<Menu> {
        if items.is_empty() {
            return None;
        }
        Some(Menu {
            start,
            items,
            chosen: 0,
            tail,
            inserted: false,
        })
    }

    /// Puts the highlighted item into `input` in place of the word.
    pub fn insert(&mut self, input: &mut String) {
        input.truncate(self.start);
        input.push_str(&self.items[self.chosen]);
        input.push_str(self.tail);
        self.inserted = true;
    }

    /// Moves the highlight one on (or back), around the ends.
    pub fn step(&mut self, forward: bool) {
        let n = self.items.len();
        self.chosen = if forward {
            (self.chosen + 1) % n
        } else {
            (self.chosen + n - 1) % n
        };
    }
}

/// The items that start with `word`, case aside.
fn matching<'a>(word: &str, items: impl Iterator<Item = &'a str>) -> Vec<String> {
    let word = word.to_lowercase();
    items
        .filter(|i| i.to_lowercase().starts_with(&word))
        .map(str::to_string)
        .collect()
}

/// A menu for `word` (at `start`), unless the word is the one match
/// already: then there is nothing to offer.
fn menu(start: usize, word: &str, items: Vec<String>, tail: &'static str) -> Option<Menu> {
    if items.len() == 1 && items[0] == word {
        return None;
    }
    Menu::new(start, items, tail)
}

/// What the composer could complete `input` with. `project` is the one
/// at hand: its agents come first, and a `/` alone goes to its leader.
pub fn suggest(input: &str, w: &World, project: ProjectId) -> Option<Menu> {
    if let Some(rest) = input.strip_prefix('@') {
        return match rest.split_once(' ') {
            None => {
                let mine = w.agents.iter().filter(|a| a.project == project);
                let others = w.agents.iter().filter(|a| a.project != project);
                let names = mine
                    .chain(others)
                    .filter(|a| !a.gone)
                    .map(|a| a.name.as_str());
                menu(1, rest, matching(rest, names), " ")
            }
            Some((name, after)) => {
                let id = w.agent_by_name(name)?;
                let start = 1 + name.len() + 1;
                commands(input, start, after, &w.agents[id])
            }
        };
    }
    if input.starts_with('/') {
        let leader = w.projects[project].leader;
        return commands(input, 0, input, &w.agents[leader]);
    }
    if let Some(rest) = input.strip_prefix(':') {
        return match rest.split_once(' ') {
            None => menu(1, rest, matching(rest, COMMANDS.into_iter()), " "),
            Some(("lead", arg)) => {
                let start = input.len() - arg.len();
                menu(start, arg, matching(arg, LEADS.into_iter()), "")
            }
            Some(_) => None,
        };
    }
    None
}

/// The agent's own commands for `after` (what follows its name, or the
/// whole input for the leader), when it is one command being typed. An
/// agent whose session has not listed any (the demo's, one not started
/// yet) offers none.
fn commands(input: &str, start: usize, after: &str, agent: &Agent) -> Option<Menu> {
    if !after.starts_with('/') || after.contains(' ') {
        return None;
    }
    debug_assert_eq!(&input[start..], after);
    let known = agent.commands.iter().map(String::as_str);
    menu(start, after, matching(after, known), " ")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::i18n::Lang;

    fn world() -> World {
        crate::sim::world(2, 3, 0, 7, Lang::En).0
    }

    fn names(m: &Option<Menu>) -> Vec<&str> {
        m.as_ref()
            .map(|m| m.items.iter().map(String::as_str).collect())
            .unwrap_or_default()
    }

    #[test]
    fn an_at_offers_the_agents_of_the_project_at_hand_first_then_the_others() {
        let w = world();
        let all = suggest("@", &w, 0).unwrap();
        assert_eq!(all.start, 1);
        assert_eq!(all.tail, " ");
        assert_eq!(all.chosen, 0);
        assert!(!all.inserted);
        let total = w.agents.iter().filter(|a| !a.gone).count();
        assert_eq!(all.items.len(), total);
        let first_of_other = all
            .items
            .iter()
            .position(|n| w.agents[w.agent_by_name(n).unwrap()].project == 1)
            .unwrap();
        assert!(
            all.items[..first_of_other]
                .iter()
                .all(|n| w.agents[w.agent_by_name(n).unwrap()].project == 0),
            "{:?}",
            all.items
        );
        assert!(
            all.items[first_of_other..]
                .iter()
                .all(|n| w.agents[w.agent_by_name(n).unwrap()].project == 1)
        );
        // Looked at from the other project, its agents come first.
        let flipped = suggest("@", &w, 1).unwrap();
        assert_eq!(
            w.agents[w.agent_by_name(&flipped.items[0]).unwrap()].project,
            1
        );
        assert_eq!(flipped.items.len(), total);
    }

    #[test]
    fn a_prefix_narrows_the_names_case_aside_and_a_whole_name_offers_nothing() {
        let w = world();
        let lead = w.agents[w.projects[0].leader].name.clone();
        let (head, _) = lead.split_at(2);
        let m = suggest(&format!("@{}", head.to_uppercase()), &w, 0).unwrap();
        assert!(m.items.contains(&lead), "{:?}", m.items);
        assert!(m.items.iter().all(|n| n.starts_with(head)));
        assert_eq!(m.start, 1);
        // The whole name, alone, is complete: no menu.
        let alone: Vec<_> = w
            .agents
            .iter()
            .filter(|a| !a.gone && a.name.starts_with(&lead))
            .collect();
        if alone.len() == 1 {
            assert_eq!(suggest(&format!("@{lead}"), &w, 0), None);
        }
        assert_eq!(suggest("@zzz", &w, 0), None);
        // A gone agent is not offered.
        let mut w = world();
        let name = w.agents[1].name.clone();
        w.agents[1].gone = true;
        assert!(!names(&suggest("@", &w, 0)).contains(&name.as_str()));
    }

    #[test]
    fn after_the_name_only_the_command_line_its_session_listed_is_completed() {
        let mut w = world();
        let lead = w.projects[0].leader;
        let name = w.agents[lead].name.clone();
        assert_eq!(
            suggest(&format!("@{name} /"), &w, 0),
            None,
            "none listed yet"
        );
        assert_eq!(suggest("/", &w, 0), None);
        let cmds = ["/clear", "/compact", "/cost", "/context"];
        w.agents[lead].commands = cmds.iter().map(|c| c.to_string()).collect();
        assert_eq!(suggest(&format!("@{name} hello"), &w, 0), None);
        assert_eq!(
            suggest(&format!("@{name} "), &w, 0),
            None,
            "a message is about to be written, not a command"
        );
        let m = suggest(&format!("@{name} /"), &w, 0).unwrap();
        assert_eq!(m.items, cmds);
        assert_eq!(m.start, name.len() + 2);
        assert_eq!(m.tail, " ");
        let m = suggest(&format!("@{name} /c"), &w, 0).unwrap();
        assert_eq!(m.items, ["/clear", "/compact", "/cost", "/context"]);
        let m = suggest(&format!("@{name} /CO"), &w, 0).unwrap();
        assert_eq!(m.items, ["/compact", "/cost", "/context"]);
        assert_eq!(suggest(&format!("@{name} /clear x"), &w, 0), None);
        assert_eq!(suggest("@nobody /c", &w, 0), None);
        // A `/` alone is the leader's; another agent's list is its own.
        let m = suggest("/", &w, 0).unwrap();
        assert_eq!(m.items, cmds);
        assert_eq!(m.start, 0);
        let other = w.agents[w.projects[1].leader].name.clone();
        assert_eq!(suggest(&format!("@{other} /c"), &w, 0), None);
        assert_eq!(suggest("/", &w, 1), None);
        assert_eq!(suggest("/clear", &w, 0), None, "whole already");
        assert_eq!(suggest("/zzz", &w, 0), None);
    }

    #[test]
    fn a_colon_offers_imeces_commands_and_lead_its_vendors() {
        let w = world();
        let m = suggest(":", &w, 0).unwrap();
        assert_eq!(m.items, COMMANDS);
        assert_eq!((m.start, m.tail), (1, " "));
        assert_eq!(names(&suggest(":B", &w, 0)), ["budget"]);
        assert_eq!(suggest(":budget", &w, 0), None);
        assert_eq!(suggest(":budget 8", &w, 0), None);
        assert_eq!(suggest(":x", &w, 0), None);
        let m = suggest(":lead ", &w, 0).unwrap();
        assert_eq!(m.items, LEADS);
        assert_eq!((m.start, m.tail), (6, ""));
        let m = suggest(":lead c", &w, 0).unwrap();
        assert_eq!(m.items, ["claude", "codex"]);
        assert_eq!(m.start, 6);
        assert_eq!(suggest(":lead codex", &w, 0), None);
        assert_eq!(suggest(":lead codex ", &w, 0), None);
        assert_eq!(suggest(":lead codex x", &w, 0), None);
        assert_eq!(suggest(":b 80", &w, 0), None);
        // Plain text, and a shell command, get no offers.
        assert_eq!(suggest("hello", &w, 0), None);
        assert_eq!(suggest("!cargo", &w, 0), None);
        assert_eq!(suggest("", &w, 0), None);
    }

    #[test]
    fn inserting_replaces_the_word_and_stepping_goes_around() {
        let mut m = Menu::new(1, vec!["claude-1".into(), "codex-1".into()], " ").unwrap();
        let mut input = String::from("@c");
        m.insert(&mut input);
        assert_eq!(input, "@claude-1 ");
        assert!(m.inserted);
        m.step(true);
        assert_eq!(m.chosen, 1);
        m.insert(&mut input);
        assert_eq!(input, "@codex-1 ");
        m.step(true);
        assert_eq!(m.chosen, 0, "around the end");
        m.step(false);
        assert_eq!(m.chosen, 1, "around the start");
        m.step(false);
        assert_eq!(m.chosen, 0);
        let mut m = Menu::new(6, vec!["ask".into()], "").unwrap();
        let mut input = String::from(":lead a");
        m.insert(&mut input);
        assert_eq!(input, ":lead ask");
        assert_eq!(Menu::new(0, vec![], " "), None);
    }
}
