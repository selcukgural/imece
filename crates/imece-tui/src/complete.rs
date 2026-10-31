//! Completion for the composer, the way a shell does it: what is being
//! typed is matched against what imece knows (agent names after `@`,
//! imece's own commands after `:`, a vendor's command line after `/`,
//! cards after `#`),
//! the matches are shown while typing, and `tab` (or the arrows) steps
//! through them with the text following. Pure: the screen asks `suggest`
//! after every change and keeps the `Menu` it gets.

use std::path::Path;

use crate::model::{Agent, CardId, ProjectId, World};

/// The commands imece itself takes after `:`: its own, and a name for
/// every key that acts (not the ones that move).
pub const COMMANDS: [&str; 26] = [
    "board", "budget", "close", "code", "defer", "export", "faster", "fold", "forget", "help",
    "lang", "lead", "needs", "next", "open", "pause", "pending", "quit", "rename", "salon",
    "slower", "talk", "theme", "trace", "unfold", "waiting",
];

/// The commands only the demo takes: live agents run in real time.
pub const DEMO_ONLY: [&str; 3] = ["faster", "pause", "slower"];

/// The short names a command also answers to. Typed whole, one is a
/// command already: no menu, so `enter` runs it.
pub const SHORT: [(&str, &str); 6] = [
    ("b", "budget"),
    ("c", "close"),
    ("f", "forget"),
    ("o", "open"),
    ("q", "quit"),
    ("t", "theme"),
];

/// The command a word typed after `:` names: itself, or the one it is
/// short for.
pub fn long(word: &str) -> &str {
    SHORT
        .iter()
        .find(|(short, _)| *short == word)
        .map_or(word, |(_, long)| long)
}

/// What `:lang` takes.
pub const LANGS: [&str; 2] = ["en", "tr"];

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
    /// What is drawn after each item and never inserted (a command's
    /// value now, `$` when it raises the bill); empty for none.
    pub notes: Vec<String>,
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
            notes: Vec::new(),
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

/// Cards offered after `#` at most.
pub const CARDS_SHOWN: usize = 30;

/// The cards of open projects for what follows `#`, each as `#N title`
/// (the whole input is replaced): digits match the start of the number,
/// by number (so that very number comes first); anything else matches inside the title, case aside, newest first;
/// nothing typed offers every card, newest first.
fn cards(rest: &str, w: &World) -> Option<Menu> {
    let open: Vec<CardId> = w
        .cards
        .iter()
        .filter(|c| !w.projects[c.project].closed)
        .map(|c| c.id)
        .collect();
    let mut found: Vec<CardId> = if !rest.is_empty() && rest.bytes().all(|b| b.is_ascii_digit()) {
        open.into_iter()
            .filter(|c| (c + 1).to_string().starts_with(rest))
            .collect()
    } else {
        let word = rest.to_lowercase();
        open.into_iter()
            .rev()
            .filter(|&c| w.cards[c].title.to_lowercase().contains(&word))
            .collect()
    };
    found.truncate(CARDS_SHOWN);
    let items = found
        .into_iter()
        .map(|c| format!("#{} {}", c + 1, w.cards[c].title))
        .collect();
    Menu::new(0, items, "")
}

/// The card a line typed after `#` names: the number it starts with, if a
/// card of an open project has it.
pub fn card_named(input: &str, w: &World) -> Option<CardId> {
    let rest = input.strip_prefix('#')?;
    let digits = rest.split(|c: char| !c.is_ascii_digit()).next()?;
    let c = digits.parse::<CardId>().ok()?.checked_sub(1)?;
    w.cards
        .get(c)
        .filter(|card| !w.projects[card.project].closed)
        .map(|card| card.id)
}

/// What the composer could complete `input` with. `project` is the one
/// at hand: its agents come first, and a `/` alone goes to its leader;
/// `home` stands for `~` in a path.
pub fn suggest(input: &str, w: &World, project: ProjectId, home: Option<&Path>) -> Option<Menu> {
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
    if let Some(rest) = input.strip_prefix('#') {
        return cards(rest, w);
    }
    if input.starts_with('/') {
        let leader = w.projects[project].leader;
        return commands(input, 0, input, &w.agents[leader]);
    }
    if let Some(rest) = input.strip_prefix(':') {
        return match rest.split_once(' ') {
            None if long(rest) != rest => None,
            None => menu(1, rest, matching(rest, COMMANDS.into_iter()), " "),
            Some(("lang", arg)) => {
                let start = input.len() - arg.len();
                menu(start, arg, matching(arg, LANGS.into_iter()), "")
            }
            Some(("lead", arg)) => {
                let start = input.len() - arg.len();
                menu(start, arg, matching(arg, LEADS.into_iter()), "")
            }
            // Built-in names only; a path matches none of them.
            Some(("theme", arg)) => {
                let start = input.len() - arg.len();
                menu(start, arg, matching(arg, crate::theme::builtins()), "")
            }
            Some(("close", arg)) => {
                let start = input.len() - arg.len();
                let names = w.open_projects().map(|p| p.name.as_str());
                menu(start, arg, matching(arg, names), "")
            }
            // Agents of every open project, with or without the `@`.
            Some(("forget" | "talk" | "trace", arg)) => {
                let word = arg.strip_prefix('@').unwrap_or(arg);
                let start = input.len() - word.len();
                let names = w
                    .agents
                    .iter()
                    .filter(|a| !a.gone && !w.projects[a.project].closed)
                    .map(|a| a.name.as_str());
                menu(start, word, matching(word, names), "")
            }
            Some(("open" | "export", arg)) => dirs(input.len() - arg.len(), arg, home, &list_dirs),
            Some(_) => None,
        };
    }
    None
}

/// Directories for the path being typed after `:open`: the ones in its
/// parent that start with its last segment, hidden ones only when that
/// segment starts with `.`; `~` stands for `home`, as in a shell. The
/// items are whole paths (the word is replaced), followed by `/` so `tab`
/// steps into the chosen one. `list` names what a directory holds.
pub fn dirs(
    start: usize,
    arg: &str,
    home: Option<&Path>,
    list: &dyn Fn(&Path) -> Vec<String>,
) -> Option<Menu> {
    let (dir, last) = match arg.rsplit_once('/') {
        Some((d, l)) => (format!("{d}/"), l),
        None => (String::new(), arg),
    };
    let real = crate::app::expand_tilde(if dir.is_empty() { "." } else { &dir }, home);
    let items: Vec<String> = list(&real)
        .iter()
        .filter(|n| n.starts_with(last) && (last.starts_with('.') || !n.starts_with('.')))
        .map(|n| format!("{dir}{n}"))
        .collect();
    menu(start, arg, items, "/")
}

/// The directories in `dir`, sorted; none when it can not be read.
fn list_dirs(dir: &Path) -> Vec<String> {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return Vec::new();
    };
    let mut names: Vec<String> = entries
        .flatten()
        .filter(|e| e.path().is_dir())
        .map(|e| e.file_name().to_string_lossy().into_owned())
        .collect();
    names.sort();
    names
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
        let all = suggest("@", &w, 0, None).unwrap();
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
        let flipped = suggest("@", &w, 1, None).unwrap();
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
        let m = suggest(&format!("@{}", head.to_uppercase()), &w, 0, None).unwrap();
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
            assert_eq!(suggest(&format!("@{lead}"), &w, 0, None), None);
        }
        assert_eq!(suggest("@zzz", &w, 0, None), None);
        // A gone agent is not offered.
        let mut w = world();
        let name = w.agents[1].name.clone();
        w.agents[1].gone = true;
        assert!(!names(&suggest("@", &w, 0, None)).contains(&name.as_str()));
    }

    #[test]
    fn after_the_name_only_the_command_line_its_session_listed_is_completed() {
        let mut w = world();
        let lead = w.projects[0].leader;
        let name = w.agents[lead].name.clone();
        assert_eq!(
            suggest(&format!("@{name} /"), &w, 0, None),
            None,
            "none listed yet"
        );
        assert_eq!(suggest("/", &w, 0, None), None);
        let cmds = ["/clear", "/compact", "/cost", "/context"];
        w.agents[lead].commands = cmds.iter().map(|c| c.to_string()).collect();
        assert_eq!(suggest(&format!("@{name} hello"), &w, 0, None), None);
        assert_eq!(
            suggest(&format!("@{name} "), &w, 0, None),
            None,
            "a message is about to be written, not a command"
        );
        let m = suggest(&format!("@{name} /"), &w, 0, None).unwrap();
        assert_eq!(m.items, cmds);
        assert_eq!(m.start, name.len() + 2);
        assert_eq!(m.tail, " ");
        let m = suggest(&format!("@{name} /c"), &w, 0, None).unwrap();
        assert_eq!(m.items, ["/clear", "/compact", "/cost", "/context"]);
        let m = suggest(&format!("@{name} /CO"), &w, 0, None).unwrap();
        assert_eq!(m.items, ["/compact", "/cost", "/context"]);
        assert_eq!(suggest(&format!("@{name} /clear x"), &w, 0, None), None);
        assert_eq!(suggest("@nobody /c", &w, 0, None), None);
        // A `/` alone is the leader's; another agent's list is its own.
        let m = suggest("/", &w, 0, None).unwrap();
        assert_eq!(m.items, cmds);
        assert_eq!(m.start, 0);
        let other = w.agents[w.projects[1].leader].name.clone();
        assert_eq!(suggest(&format!("@{other} /c"), &w, 0, None), None);
        assert_eq!(suggest("/", &w, 1, None), None);
        assert_eq!(suggest("/clear", &w, 0, None), None, "whole already");
        assert_eq!(suggest("/zzz", &w, 0, None), None);
    }

    #[test]
    fn a_short_name_is_its_command_and_offers_no_menu() {
        let w = world();
        for (short, whole) in SHORT {
            assert_eq!(long(short), whole);
            assert_eq!(suggest(&format!(":{short}"), &w, 0, None), None, "{short}");
        }
        assert_eq!(long("quit"), "quit");
        assert_eq!(long("x"), "x");
        assert_eq!(names(&suggest(":qu", &w, 0, None)), ["quit"]);
        assert_eq!(names(&suggest(":fo", &w, 0, None)), ["fold", "forget"]);
        // Every short name stands for a command, every demo one is one.
        assert!(SHORT.iter().all(|(_, c)| COMMANDS.contains(c)));
        assert!(DEMO_ONLY.iter().all(|c| COMMANDS.contains(c)));
        let m = suggest(":", &w, 0, None).unwrap();
        assert!(m.notes.is_empty(), "notes are the screen's to add");
    }

    #[test]
    fn lang_offers_the_languages_and_trace_and_talk_the_agents() {
        let w = world();
        let m = suggest(":lang ", &w, 0, None).unwrap();
        assert_eq!(
            (m.items, m.start, m.tail),
            (LANGS.map(String::from).to_vec(), 6, "")
        );
        let m = suggest(":lang t", &w, 0, None).unwrap();
        assert_eq!(
            (m.items, m.start),
            (vec!["tr".to_string()], 6),
            "the word's start"
        );
        assert_eq!(suggest(":lang tr", &w, 0, None), None);
        let name = &w.agents[1].name;
        let head = &name[..3];
        for cmd in ["talk", "trace", "forget"] {
            let m = suggest(&format!(":{cmd} @{head}"), &w, 0, None).unwrap();
            assert!(m.items.contains(name), "{cmd}");
            assert_eq!(m.start, cmd.len() + 3, "{cmd}: after the @");
            let m = suggest(&format!(":{cmd} {head}"), &w, 0, None).unwrap();
            assert_eq!(m.start, cmd.len() + 2, "{cmd}");
        }
    }

    #[test]
    fn a_colon_offers_imeces_commands_and_lead_its_vendors() {
        let w = world();
        let m = suggest(":", &w, 0, None).unwrap();
        assert_eq!(m.items, COMMANDS);
        assert_eq!((m.start, m.tail), (1, " "));
        assert_eq!(names(&suggest(":B", &w, 0, None)), ["board", "budget"]);
        assert_eq!(names(&suggest(":Bu", &w, 0, None)), ["budget"]);
        assert_eq!(suggest(":budget", &w, 0, None), None);
        assert_eq!(suggest(":budget 8", &w, 0, None), None);
        assert_eq!(suggest(":x", &w, 0, None), None);
        let m = suggest(":lead ", &w, 0, None).unwrap();
        assert_eq!(m.items, LEADS);
        assert_eq!((m.start, m.tail), (6, ""));
        let m = suggest(":lead c", &w, 0, None).unwrap();
        assert_eq!(m.items, ["claude", "codex"]);
        assert_eq!(m.start, 6);
        assert_eq!(suggest(":lead codex", &w, 0, None), None);
        assert_eq!(suggest(":lead codex ", &w, 0, None), None);
        assert_eq!(suggest(":lead codex x", &w, 0, None), None);
        assert_eq!(suggest(":b 80", &w, 0, None), None);
        let m = suggest(":theme ", &w, 0, None).unwrap();
        assert_eq!(m.items, crate::theme::builtins().collect::<Vec<_>>());
        assert_eq!((m.start, m.tail), (7, ""));
        let m = suggest(":theme D", &w, 0, None).unwrap();
        assert_eq!(names(&Some(m.clone())), ["dark", "dracula"]);
        assert_eq!(m.start, 7, "the word starts after the command");
        assert_eq!(suggest(":theme dark", &w, 0, None), None);
        assert_eq!(
            suggest(":theme ./dark", &w, 0, None),
            None,
            "a path is not completed"
        );
        assert_eq!(suggest(":theme dark x", &w, 0, None), None);
        // Plain text, and a shell command, get no offers.
        assert_eq!(suggest("hello", &w, 0, None), None);
        assert_eq!(suggest("!cargo", &w, 0, None), None);
        assert_eq!(suggest("", &w, 0, None), None);
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

    #[test]
    fn open_completes_directories_and_close_the_open_projects() {
        let tree = |dir: &Path| -> Vec<String> {
            match dir.to_str().unwrap() {
                "/t" | "/t/" => vec![".hidden".into(), "alpha".into(), "beta".into()],
                "/home/me" | "/home/me/" => vec!["src".into()],
                "." => vec!["here".into()],
                _ => Vec::new(),
            }
        };
        let m = dirs(6, "/t/", None, &tree).unwrap();
        assert_eq!(m.items, ["/t/alpha", "/t/beta"], "no hidden ones unasked");
        assert_eq!((m.start, m.tail), (6, "/"));
        assert_eq!(names(&dirs(6, "/t/.", None, &tree)), ["/t/.hidden"]);
        assert_eq!(names(&dirs(6, "/t/al", None, &tree)), ["/t/alpha"]);
        assert_eq!(dirs(6, "/t/alpha", None, &tree), None, "whole already");
        assert_eq!(dirs(6, "/t/x", None, &tree), None);
        assert_eq!(
            names(&dirs(6, "", None, &tree)),
            ["here"],
            "nothing typed: here"
        );
        assert_eq!(names(&dirs(6, "h", None, &tree)), ["here"]);
        let home = Some(Path::new("/home/me"));
        assert_eq!(names(&dirs(6, "~/", home, &tree)), ["~/src"]);
        assert_eq!(names(&dirs(6, "~/s", home, &tree)), ["~/src"]);
        assert_eq!(dirs(6, "~/", None, &tree), None, "no home, no ~");

        // Through `suggest`, on the real filesystem.
        let dir = std::env::temp_dir().join(format!("imece-complete-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        for d in ["alpha", "beta", ".hidden"] {
            std::fs::create_dir_all(dir.join(d)).unwrap();
        }
        std::fs::write(dir.join("file"), "").unwrap();
        let w = world();
        let at = |s: &str| format!("{}/{s}", dir.display());
        let m = suggest(&format!(":open {}", at("")), &w, 0, None).unwrap();
        assert_eq!(m.items, [at("alpha"), at("beta")], "directories only");
        assert_eq!((m.start, m.tail), (6, "/"));
        assert_eq!(
            names(&suggest(&format!(":open {}", at("b")), &w, 0, None)),
            [at("beta")]
        );
        let m = suggest(&format!(":export {}", at("")), &w, 0, None).unwrap();
        assert_eq!(m.items, [at("alpha"), at("beta")], ":export too");
        assert_eq!(m.start, 8);
        let m = suggest(":open ~/", &w, 0, Some(&dir)).unwrap();
        assert_eq!(m.items, ["~/alpha", "~/beta"]);
        assert_eq!(suggest(":open /nonexistent-dir/", &w, 0, None), None);
        assert_eq!(
            suggest(":open", &w, 0, None),
            None,
            "the command itself, whole"
        );
        assert_eq!(
            suggest(&format!(":o {}", at("")), &w, 0, None),
            None,
            "short forms are not completed"
        );
        let _ = std::fs::remove_dir_all(&dir);

        let m = suggest(":close ", &w, 0, None).unwrap();
        assert_eq!(m.items, ["shop-api", "mobile-app"]);
        assert_eq!((m.start, m.tail), (7, ""));
        let m = suggest(":close M", &w, 0, None).unwrap();
        assert_eq!(m.items, ["mobile-app"]);
        // :forget takes an agent of any open project, @ or not.
        let live: Vec<&str> = w
            .agents
            .iter()
            .filter(|a| !a.gone)
            .map(|a| a.name.as_str())
            .collect();
        let m = suggest(":forget ", &w, 0, None).unwrap();
        assert_eq!(m.items, live);
        assert_eq!((m.start, m.tail), (8, ""));
        let m = suggest(":forget @", &w, 0, None).unwrap();
        assert_eq!(m.items, live);
        assert_eq!(m.start, 9, "the word after the @");
        let first = live[0];
        let m = suggest(
            &format!(":forget @{}", first[..2].to_uppercase()),
            &w,
            0,
            None,
        )
        .unwrap();
        assert!(m.items.contains(&first.to_string()), "{:?}", m.items);
        assert_eq!(m.start, 9, "the word, not the @, is replaced");
        assert!(
            m.items
                .iter()
                .all(|n| n[..2].eq_ignore_ascii_case(&first[..2]))
        );
        assert_eq!(suggest(&format!(":forget {first}"), &w, 0, None), None);
        let m = suggest(":close M", &w, 0, None).unwrap();
        assert_eq!(m.start, 7, "the word starts after the command");
        assert_eq!(suggest(":close shop-api", &w, 0, None), None);
        let mut w = w;
        w.projects[1].closed = true;
        let gone = w.projects[0].leader;
        w.agents[gone].gone = true;
        let m = suggest(":forget ", &w, 0, None).unwrap();
        assert!(
            m.items
                .iter()
                .all(|n| w.agents[w.agent_by_name(n).unwrap()].project == 0),
            "a closed project's agents are not offered: {:?}",
            m.items
        );
        assert!(
            !m.items.contains(&w.agents[gone].name),
            "nor a gone one: {:?}",
            m.items
        );
        assert!(!m.items.is_empty());
        w.agents[gone].gone = false;
        assert_eq!(
            names(&suggest(":close ", &w, 0, None)),
            ["shop-api"],
            "open ones"
        );
        assert_eq!(suggest(":c ", &w, 0, None), None);
        assert_eq!(suggest(":rename ", &w, 0, None), None);
    }

    /// The demo's world with cards of the given titles, numbered from #1,
    /// the ones at `closed` in the second project, which is closed.
    fn with_cards(titles: &[&str], closed: &[usize]) -> World {
        let mut w = world();
        w.cards.clear();
        for (id, title) in titles.iter().enumerate() {
            w.cards.push(crate::model::Card {
                id,
                project: usize::from(closed.contains(&id)),
                title: title.to_string(),
                owner: None,
                column: crate::model::Column::Waiting,
                story: Vec::new(),
                symbols: Vec::new(),
                started: 0,
                finished: 0,
            });
        }
        w.projects[1].closed = true;
        w
    }

    #[test]
    fn a_hash_offers_the_cards_by_number_or_title() {
        // #1 .. #13; #12 is in the closed project.
        let titles: Vec<String> = (1..=13).map(|n| format!("Card {n}")).collect();
        let mut titles: Vec<&str> = titles.iter().map(String::as_str).collect();
        titles[0] = "Shipping cost";
        titles[2] = "Ship it";
        let w = with_cards(&titles, &[11]);
        let m = suggest("#", &w, 0, None).unwrap();
        assert_eq!((m.start, m.tail, m.chosen, m.inserted), (0, "", 0, false));
        assert_eq!(m.items.len(), 12);
        assert_eq!(m.items[..2], ["#13 Card 13", "#11 Card 11"], "newest first");
        assert_eq!(m.items[11], "#1 Shipping cost");
        // Digits: the start of the number, by number.
        assert_eq!(
            names(&suggest("#1", &w, 0, None)),
            [
                "#1 Shipping cost",
                "#10 Card 10",
                "#11 Card 11",
                "#13 Card 13"
            ]
        );
        assert_eq!(names(&suggest("#13", &w, 0, None)), ["#13 Card 13"]);
        assert_eq!(names(&suggest("#12", &w, 0, None)), Vec::<&str>::new());
        // Anything else: inside the title, case aside, newest first.
        assert_eq!(
            names(&suggest("#SHIP", &w, 0, None)),
            ["#3 Ship it", "#1 Shipping cost"]
        );
        assert_eq!(
            names(&suggest("#ping c", &w, 0, None)),
            ["#1 Shipping cost"]
        );
        assert_eq!(suggest("#nothing", &w, 0, None), None);
    }

    #[test]
    fn a_hash_offers_a_few_dozen_cards_at_most() {
        let titles = vec!["x"; CARDS_SHOWN + 1];
        let w = with_cards(&titles, &[]);
        let m = suggest("#", &w, 0, None).unwrap();
        assert_eq!(m.items.len(), 30);
        assert_eq!(m.items[0], "#31 x");
    }

    #[test]
    fn a_typed_line_names_a_card_by_the_number_it_starts_with() {
        let w = with_cards(&["a", "b", "c"], &[1]);
        assert_eq!(card_named("#1", &w), Some(0));
        assert_eq!(card_named("#3 c, and more", &w), Some(2));
        assert_eq!(card_named("#3x", &w), Some(2));
        assert_eq!(card_named("#2", &w), None, "a closed project's");
        assert_eq!(card_named("#4", &w), None, "no such card");
        assert_eq!(card_named("#0", &w), None);
        assert_eq!(card_named("#", &w), None);
        assert_eq!(card_named("#c", &w), None);
        assert_eq!(card_named("1", &w), None);
    }
}
