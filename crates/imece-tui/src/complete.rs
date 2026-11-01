//! Completion for the composer, the way a shell does it: what is being
//! typed is matched against what imece knows (agent names after `@`,
//! imece's own commands after `:`, a vendor's command line after `/`,
//! cards after `#`),
//! the matches are shown while typing, and `tab` (or the arrows) steps
//! through them with the text following. Pure: the screen asks `suggest`
//! after every change and keeps the `Menu` it gets.

use std::path::Path;

use crate::model::{Agent, CardId, ProjectId, Sent, Vendor, World};
use crate::ui::fold;

/// The commands imece itself takes after `:`: its own, and a name for
/// every key that acts (not the ones that move).
pub const COMMANDS: [&str; 46] = [
    "awake", "board", "budget", "close", "code", "commit", "copy", "defer", "effort", "export",
    "faster", "fold", "forget", "goto", "help", "hints", "history", "keys", "lang", "lead", "mark",
    "model", "needs", "news", "next", "notify", "open", "output", "pause", "peek", "pending",
    "pin", "quit", "rename", "salon", "settle", "slower", "solo", "talk", "theme", "trace", "undo",
    "unfold", "unsettle", "waiting", "welcome",
];

/// The word for a vendor's own model or effort after `:model`, `:effort`
/// and in a `:solo` choice.
pub const DEFAULT: &str = "default";

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

/// Where `&` may go: an agent or a project, by name; the words the menu
/// shows after it (an agent's glyph and status); whether it needs the
/// person, for the `!` filter.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Jump {
    pub name: String,
    pub note: String,
    pub needs: bool,
}

/// The targets for what follows `&`, numbered as `list` stands (the list
/// is frozen when the line opens, so the numbers hold while it is open):
/// digits match the start of a number; `!` first keeps only what needs the
/// person; anything else matches inside the name, case and Turkish
/// letters folded, every word; nothing typed offers the whole list. Each
/// item is `&N name`, the whole input replaced.
pub fn jumps(rest: &str, list: &[Jump]) -> Option<Menu> {
    let (needs, rest) = match rest.strip_prefix('!') {
        Some(r) => (true, r),
        None => (false, rest),
    };
    let numbered = list.iter().enumerate().filter(|(_, j)| !needs || j.needs);
    let found: Vec<(usize, &Jump)> = if !rest.is_empty() && rest.bytes().all(|b| b.is_ascii_digit())
    {
        numbered
            .filter(|(i, _)| (i + 1).to_string().starts_with(rest))
            .collect()
    } else {
        let words: Vec<String> = rest.split_whitespace().map(fold).collect();
        numbered
            .filter(|(_, j)| {
                let hay = fold(&j.name);
                words.iter().all(|w| hay.contains(w.as_str()))
            })
            .collect()
    };
    let items = found
        .iter()
        .map(|(i, j)| format!("&{} {}", i + 1, j.name))
        .collect();
    let mut m = Menu::new(0, items, "")?;
    m.notes = found.iter().map(|(_, j)| j.note.clone()).collect();
    Some(m)
}

/// The name a line typed after `&` goes to: the one numbered as it starts
/// (`&3`, `&3 claude-1`), else the words themselves (`&claude-1`), the
/// `!` filter dropped; nothing for a bare `&`.
pub fn jump_named(input: &str, list: &[Jump]) -> Option<String> {
    let rest = input.strip_prefix('&')?;
    let rest = rest.strip_prefix('!').unwrap_or(rest).trim();
    let digits: String = rest.chars().take_while(char::is_ascii_digit).collect();
    if !digits.is_empty()
        && digits.len() == rest.split_whitespace().next().unwrap_or_default().len()
    {
        return digits
            .parse::<usize>()
            .ok()?
            .checked_sub(1)
            .and_then(|i| list.get(i))
            .map(|j| j.name.clone());
    }
    (!rest.is_empty()).then(|| rest.to_string())
}

/// Files (and symbols) offered for a word at most.
pub const FILES_SHOWN: usize = 30;

/// The files of the project that have `word` in their path (case and
/// Turkish letters folded), the ones whose name starts with it first,
/// then the shorter path, then by name; after them the symbols leased on
/// the code map whose name starts with the word, as `file::name` (a
/// word with `::` names the file's part and the symbol's); at most
/// [`FILES_SHOWN`], each with `note` (who holds the file). Nothing for a
/// word without a `/` unless `forced` (`tab` asked), so writing is not
/// interrupted; the item replaces the word, a space after it.
pub fn files(
    start: usize,
    word: &str,
    forced: bool,
    paths: &[&str],
    symbols: &[(&str, &str)],
    note: impl Fn(&str) -> String,
) -> Option<Menu> {
    if word.is_empty() || (!forced && !word.contains('/')) {
        return None;
    }
    let want = fold(word);
    let base = |p: &str| p.rsplit('/').next().unwrap_or(p).to_string();
    let mut found: Vec<(usize, &str)> = paths
        .iter()
        .copied()
        .filter(|p| fold(p).contains(&want))
        .map(|p| (usize::from(!fold(&base(p)).starts_with(&want)), p))
        .collect();
    found.sort_by(|a, b| {
        a.0.cmp(&b.0)
            .then(a.1.len().cmp(&b.1.len()))
            .then(a.1.cmp(b.1))
    });
    let mut items: Vec<(String, String)> = found
        .iter()
        .map(|(_, p)| (p.to_string(), note(p)))
        .collect();
    let (file_part, sym_part) = match word.split_once("::") {
        Some((f, s)) => (fold(f), fold(s)),
        None => (String::new(), want.clone()),
    };
    for (file, name) in symbols {
        if fold(name).starts_with(&sym_part) && fold(file).contains(&file_part) {
            items.push((format!("{file}::{name}"), note(file)));
        }
    }
    items.truncate(FILES_SHOWN);
    let (items, notes): (Vec<String>, Vec<String>) = items.into_iter().unzip();
    let mut m = menu(start, word, items, " ")?;
    m.notes = notes;
    Some(m)
}

/// A sent line as a menu shows it: on one line (`⏎` between its lines),
/// at most this many characters.
pub const LINE_SHOWN: usize = 60;

/// The lines sent that have every word of `rest` in them (case and
/// Turkish letters folded), newest first, each text once: their places
/// in `sent`. Nothing typed is every line.
pub fn history_picks(rest: &str, sent: &[Sent]) -> Vec<usize> {
    let words: Vec<String> = rest.split_whitespace().map(fold).collect();
    let mut seen = std::collections::HashSet::new();
    sent.iter()
        .enumerate()
        .rev()
        .filter(|(_, s)| {
            let hay = fold(&s.text);
            words.iter().all(|w| hay.contains(w.as_str()))
        })
        .filter(|(_, s)| seen.insert(s.text.as_str()))
        .map(|(i, _)| i)
        .collect()
}

/// The `^r` menu over `sent`: the picks for `rest`, each on one line
/// with the note `when` gives it (how long ago); none when nothing has
/// the words.
pub fn history(rest: &str, sent: &[Sent], when: impl Fn(&Sent) -> String) -> Option<Menu> {
    let picks = history_picks(rest, sent);
    let items = picks.iter().map(|&i| one_line(&sent[i].text)).collect();
    let mut m = Menu::new(0, items, "")?;
    m.notes = picks.iter().map(|&i| when(&sent[i])).collect();
    Some(m)
}

/// `text` on one line, cut to [`LINE_SHOWN`] characters with `…`.
pub fn one_line(text: &str) -> String {
    let flat = text.lines().collect::<Vec<_>>().join(" ⏎ ");
    if flat.chars().count() <= LINE_SHOWN {
        return flat;
    }
    let mut cut: String = flat.chars().take(LINE_SHOWN - 1).collect();
    cut.push('…');
    cut
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

/// What `:solo` could complete the word `arg` with: a directory when it
/// looks like a path (`/`, `~`, `.`), else the vendors `vendors` and, once
/// a vendor and `@` are typed, that vendor's efforts (`efforts`).
pub fn solo_words(
    start: usize,
    arg: &str,
    home: Option<&Path>,
    vendors: &[Vendor],
    efforts: &dyn Fn(Vendor) -> Vec<&'static str>,
    list: &dyn Fn(&Path) -> Vec<String>,
) -> Option<Menu> {
    if arg.contains('/') || arg.starts_with(['~', '.']) {
        return dirs(start, arg, home, list);
    }
    if let Some((head, after)) = arg.rsplit_once('@') {
        let vendor = head.split(':').next().unwrap_or_default();
        let v = vendors.iter().find(|v| v.label() == vendor)?;
        let items: Vec<String> = efforts(*v)
            .into_iter()
            .filter(|e| e.starts_with(after))
            .map(|e| format!("{head}@{e}"))
            .collect();
        return menu(start, arg, items, " ");
    }
    let names = vendors.iter().map(|v| v.label());
    menu(start, arg, matching(arg, names), "")
}

/// What `:effort` or `:model` could complete `arg` with: `default`, then
/// `items` (a vendor's efforts, or its model aliases).
pub fn choice_words(start: usize, arg: &str, items: &[&str]) -> Option<Menu> {
    let all = std::iter::once(DEFAULT).chain(items.iter().copied());
    menu(start, arg, matching(arg, all), "")
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
            Some(("notify", arg)) => {
                let start = input.len() - arg.len();
                menu(
                    start,
                    arg,
                    matching(arg, crate::notify::NAMES.into_iter()),
                    "",
                )
            }
            // Built-in names only; a path matches none of them.
            Some(("theme", arg)) => {
                let start = input.len() - arg.len();
                menu(start, arg, matching(arg, crate::theme::builtins()), "")
            }
            Some(("close" | "pin", arg)) => {
                let start = input.len() - arg.len();
                let names = w.open_projects().map(|p| p.name.as_str());
                menu(start, arg, matching(arg, names), "")
            }
            // Agents of every open project, with or without the `@`.
            Some(("forget" | "talk" | "trace" | "peek", arg)) => {
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
pub fn list_dirs(dir: &Path) -> Vec<String> {
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

    #[test]
    fn solo_words_complete_vendors_then_efforts_and_paths_as_directories() {
        let vendors = [Vendor::Claude, Vendor::Codex];
        let efforts = |v: Vendor| -> Vec<&'static str> {
            match v {
                Vendor::Claude => vec!["low", "high"],
                _ => vec![],
            }
        };
        let list = |_: &std::path::Path| {
            vec!["src".to_string(), "docs".to_string(), ".hidden".to_string()]
        };
        let items = |m: Option<Menu>| m.map(|m| m.items).unwrap_or_default();
        assert_eq!(
            items(solo_words(6, "", None, &vendors, &efforts, &list)),
            ["claude", "codex"]
        );
        assert_eq!(
            items(solo_words(6, "cl", None, &vendors, &efforts, &list)),
            ["claude"]
        );
        assert_eq!(
            items(solo_words(6, "claude@", None, &vendors, &efforts, &list)),
            ["claude@low", "claude@high"]
        );
        assert_eq!(
            items(solo_words(
                6,
                "claude:opus@h",
                None,
                &vendors,
                &efforts,
                &list
            )),
            ["claude:opus@high"]
        );
        assert!(
            solo_words(6, "codex@", None, &vendors, &efforts, &list).is_none(),
            "no efforts"
        );
        assert!(
            solo_words(6, "gemini@", None, &vendors, &efforts, &list).is_none(),
            "not here"
        );
        let m = solo_words(6, "./s", None, &vendors, &efforts, &list).unwrap();
        assert_eq!((m.items, m.tail), (vec!["./src".to_string()], "/"));
        assert_eq!(
            items(solo_words(6, "~/", None, &vendors, &efforts, &list)).len(),
            2,
            "a path"
        );
        assert!(
            solo_words(6, "claude", None, &vendors, &efforts, &list).is_none(),
            "typed whole"
        );
        // A path by a `/` alone, or a `.` alone: directories, not vendors.
        assert_eq!(
            items(solo_words(6, "a/d", None, &vendors, &efforts, &list)),
            ["a/docs"]
        );
        assert_eq!(
            items(solo_words(6, ".", None, &vendors, &efforts, &list)),
            [".hidden"]
        );
        let m = solo_words(6, "c", None, &vendors, &efforts, &list).unwrap();
        assert_eq!((m.start, m.tail), (6, ""));
    }

    #[test]
    fn choice_words_offer_default_then_the_vendor_s_own() {
        let m = choice_words(8, "", &["low", "high"]).unwrap();
        assert_eq!(m.items, ["default", "low", "high"]);
        assert_eq!(m.start, 8);
        assert_eq!(choice_words(8, "d", &["low"]).unwrap().items, ["default"]);
        assert_eq!(
            choice_words(8, "h", &["low", "high"]).unwrap().items,
            ["high"]
        );
        assert!(choice_words(8, "x", &["low"]).is_none());
        assert!(choice_words(8, "default", &[]).is_none(), "typed whole");
        assert_eq!(DEFAULT, "default");
        assert!(COMMANDS.contains(&"effort") && COMMANDS.contains(&"model"));
    }
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
        let m = suggest(":notify ", &w, 0, None).unwrap();
        assert_eq!(
            (m.items, m.start, m.tail),
            (crate::notify::NAMES.map(String::from).to_vec(), 8, "")
        );
        let m = suggest(":notify b", &w, 0, None).unwrap();
        assert_eq!((m.items, m.start), (vec!["bell".to_string()], 8));
        assert_eq!(suggest(":notify bell", &w, 0, None), None);
        let name = &w.agents[1].name;
        let head = &name[..3];
        for cmd in ["talk", "trace", "forget", "peek"] {
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

    fn jump(name: &str, needs: bool) -> Jump {
        Jump {
            name: name.into(),
            note: format!("note {name}"),
            needs,
        }
    }

    #[test]
    fn the_jump_menu_numbers_the_list_as_it_stands_and_filters_by_number_need_or_name() {
        let list = [
            jump("claude-1", true),
            jump("codex-1", false),
            jump("işler", true),
        ];
        let items = |m: Option<Menu>| m.map(|m| (m.items, m.notes));
        // Nothing typed: the whole list, numbered, each with its note.
        assert_eq!(
            items(jumps("", &list)),
            Some((
                vec!["&1 claude-1".into(), "&2 codex-1".into(), "&3 işler".into()],
                vec![
                    "note claude-1".into(),
                    "note codex-1".into(),
                    "note işler".into()
                ]
            ))
        );
        // Digits: the start of a number; the numbers hold.
        assert_eq!(items(jumps("2", &list)).unwrap().0, ["&2 codex-1"]);
        // `!`: only what needs the person, still by their numbers.
        assert_eq!(
            items(jumps("!", &list)).unwrap().0,
            ["&1 claude-1", "&3 işler"]
        );
        assert_eq!(items(jumps("!3", &list)).unwrap().0, ["&3 işler"]);
        assert_eq!(items(jumps("!cod", &list)), None);
        // A name: inside, case and Turkish letters folded, every word.
        assert_eq!(items(jumps("ODEX", &list)).unwrap().0, ["&2 codex-1"]);
        assert_eq!(items(jumps("isler", &list)).unwrap().0, ["&3 işler"]);
        assert_eq!(items(jumps("İŞ", &list)).unwrap().0, ["&3 işler"]);
        assert_eq!(
            items(jumps("-1", &list)).unwrap().0,
            ["&1 claude-1", "&2 codex-1"]
        );
        assert_eq!(items(jumps("cla 1", &list)).unwrap().0, ["&1 claude-1"]);
        assert_eq!(items(jumps("cla x", &list)), None);
        assert_eq!(items(jumps("", &[])), None);
    }

    #[test]
    fn the_jump_line_names_its_target_by_number_or_by_the_words_typed() {
        let list = [jump("claude-1", true), jump("codex-1", false)];
        let named = |input: &str| jump_named(input, &list);
        assert_eq!(named("&2"), Some("codex-1".into()));
        assert_eq!(named("&2 codex-1"), Some("codex-1".into()));
        assert_eq!(named("&!1"), Some("claude-1".into()));
        assert_eq!(named("&3"), None, "no third");
        assert_eq!(named("&0"), None);
        assert_eq!(
            named("&CLAUDE-1"),
            Some("CLAUDE-1".into()),
            "as typed: the screen folds it"
        );
        assert_eq!(named("& codex-1 "), Some("codex-1".into()));
        assert_eq!(
            named("&12x"),
            Some("12x".into()),
            "digits glued to letters are a name"
        );
        assert_eq!(named("&"), None);
        assert_eq!(named("&!"), None);
        assert_eq!(named("#1"), None);
    }

    fn sent(tick: u64, text: &str) -> Sent {
        Sent {
            tick,
            text: text.into(),
        }
    }

    #[test]
    fn the_history_menu_lists_the_newest_first_each_line_once_with_every_word_typed() {
        let lines = [
            sent(1, "@claude-1 fix the clock"),
            sent(2, ":budget 80"),
            sent(3, "@claude-1 fix the clock"),
            sent(4, "@codex-1 İşler nasıl?\nthe second line"),
        ];
        // Newest first; the repeated line once, at its newest.
        assert_eq!(history_picks("", &lines), [3, 2, 1]);
        // Every word, case and Turkish letters folded, anywhere in the line.
        assert_eq!(history_picks("CLOCK fix", &lines), [2]);
        assert_eq!(history_picks("isler", &lines), [3]);
        assert_eq!(history_picks("second", &lines), [3]);
        assert_eq!(history_picks("fix budget", &lines), Vec::<usize>::new());
        assert_eq!(history_picks("", &[]), Vec::<usize>::new());
        // The menu: each on one line, the whole input replaced, its note.
        let m = history("", &lines, |s| format!("tick {}", s.tick)).unwrap();
        assert_eq!(m.start, 0);
        assert_eq!(
            m.items,
            [
                "@codex-1 İşler nasıl? ⏎ the second line",
                "@claude-1 fix the clock",
                ":budget 80"
            ]
        );
        assert_eq!(
            m.notes,
            ["tick 4", "tick 3", "tick 2"],
            "the repeated line at its newest"
        );
        assert!(history("zzz", &lines, |_| String::new()).is_none());
        // A long line is cut at sixty characters, the last one `…`.
        let long = "x".repeat(61);
        assert_eq!(one_line(&long).chars().count(), 60);
        assert!(one_line(&long).ends_with("x…"));
        assert_eq!(one_line(&"y".repeat(60)), "y".repeat(60));
        assert_eq!(one_line("a\nb\nc"), "a ⏎ b ⏎ c");
    }

    #[test]
    fn a_word_completes_to_the_files_that_have_it_names_first_then_the_symbols() {
        let paths = [
            "src/auth/session.rs",
            "src/auth.rs",
            "tests/auth_test.rs",
            "docs/Authors.md",
            "README.md",
            "src/b/author.rs",
        ];
        let symbols = [("src/auth.rs", "verify"), ("src/auth.rs", "Session.Start")];
        let note = |p: &str| {
            if p == "src/auth.rs" {
                "● claude-1".to_string()
            } else {
                String::new()
            }
        };
        let items = |m: Option<Menu>| m.map(|m| (m.start, m.items, m.notes, m.tail));
        // Without a `/`, only when asked (tab); then the names that start
        // with the word first (the shorter path first, then by name), the
        // rest by the same rule; the symbols after, by name.
        assert_eq!(items(files(3, "auth", false, &paths, &symbols, note)), None);
        assert_eq!(
            items(files(3, "auth", true, &paths, &symbols, note)),
            Some((
                3,
                vec![
                    "src/auth.rs".to_string(),
                    "docs/Authors.md".into(),
                    "src/b/author.rs".into(),
                    "tests/auth_test.rs".into(),
                    "src/auth/session.rs".into(),
                ],
                vec![
                    "● claude-1".to_string(),
                    "".into(),
                    "".into(),
                    "".into(),
                    "".into()
                ],
                " "
            ))
        );
        // A `/` opens the menu by itself; the word matches inside the path.
        assert_eq!(
            items(files(0, "h/se", false, &paths, &symbols, note))
                .unwrap()
                .1,
            ["src/auth/session.rs"]
        );
        // Case and Turkish letters aside.
        assert_eq!(
            items(files(0, "AUTHORS", true, &paths, &symbols, note))
                .unwrap()
                .1,
            ["docs/Authors.md"]
        );
        // Symbols: by the start of their name, after the files; with `::`
        // the file's part and the symbol's.
        assert_eq!(
            items(files(0, "ver", true, &paths, &symbols, note))
                .unwrap()
                .1,
            ["src/auth.rs::verify"]
        );
        assert_eq!(
            items(files(0, "sess", true, &paths, &symbols, note))
                .unwrap()
                .1,
            ["src/auth/session.rs", "src/auth.rs::Session.Start"]
        );
        assert_eq!(
            items(files(0, "auth.rs::", true, &paths, &symbols, note)).unwrap(),
            (
                0,
                vec![
                    "src/auth.rs::verify".to_string(),
                    "src/auth.rs::Session.Start".into()
                ],
                vec!["● claude-1".to_string(), "● claude-1".into()],
                " "
            )
        );
        assert_eq!(
            items(files(0, "b/::ver", true, &paths, &symbols, note)),
            None
        );
        // Nothing: no menu; the one match that is the word already: none.
        assert_eq!(items(files(0, "zzz", true, &paths, &symbols, note)), None);
        assert_eq!(items(files(0, "", true, &paths, &symbols, note)), None);
        assert_eq!(
            items(files(0, "README.md", true, &paths, &symbols, note)),
            None
        );
        // At most FILES_SHOWN.
        let many: Vec<String> = (0..40).map(|i| format!("d/f{i:02}.rs")).collect();
        let many: Vec<&str> = many.iter().map(String::as_str).collect();
        let m = files(0, "d/", false, &many, &[], |_| String::new()).unwrap();
        assert_eq!(m.items.len(), FILES_SHOWN);
        assert_eq!(FILES_SHOWN, 30);
        assert_eq!(m.notes.len(), 30);
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
        let m = suggest(":pin m", &w, 0, None).unwrap();
        assert_eq!((m.items, m.start), (vec!["mobile-app".to_string()], 5));
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
