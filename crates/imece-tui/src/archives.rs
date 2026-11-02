//! The archives browser (`A`, `:archives`): every archive as a tree,
//! checkout → archive → its agents, with the projects open on each
//! checkout now at the top of it. `/` filters by every word (case and
//! Turkish letters folded), `h l za` fold, `r` brings the archive at hand
//! back. What it lists is frozen when it opens: nothing changes an
//! archive but a restore, and a restore closes it.

use std::collections::BTreeSet;

use ratatui::crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

use crate::model::{AgentId, ProjectId, World};
use crate::ui::fold;

/// The archives of one checkout, newest first, and the projects open on
/// it now.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Group {
    /// The checkout as the screen writes it (`~/src/kule`; the demo's:
    /// the project's name).
    pub checkout: String,
    pub open: Vec<ProjectId>,
    pub archives: Vec<ProjectId>,
}

/// A row of the browser.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Row {
    /// A checkout: its index in [`Browser::groups`].
    Checkout(usize),
    /// A project open on it now.
    Open(ProjectId),
    Archive(ProjectId),
    /// An agent of an archive unfolded.
    Agent(AgentId),
}

/// What a key in the browser asks of the screen.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Act {
    Close,
    Restore(ProjectId),
}

/// The archives grouped by checkout: the checkout of `first` first (the
/// one at hand), then in the order of their newest archive; in each, the
/// open projects by id and the archives newest first. A checkout with no
/// archive is left out.
pub fn groups(
    w: &World,
    checkout: &dyn Fn(ProjectId) -> String,
    first: Option<ProjectId>,
) -> Vec<Group> {
    let mut groups: Vec<Group> = Vec::new();
    for p in w.archives() {
        let at = checkout(p);
        match groups.iter_mut().find(|g| g.checkout == at) {
            Some(g) => g.archives.push(p),
            None => groups.push(Group {
                checkout: at,
                open: Vec::new(),
                archives: vec![p],
            }),
        }
    }
    for q in w.projects.iter().filter(|q| !q.closed) {
        let at = checkout(q.id);
        if let Some(g) = groups.iter_mut().find(|g| g.checkout == at) {
            g.open.push(q.id);
        }
    }
    if let Some(at) = first.map(checkout)
        && let Some(i) = groups.iter().position(|g| g.checkout == at)
    {
        let g = groups.remove(i);
        groups.insert(0, g);
    }
    groups
}

/// The browser's state while it is open.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Browser {
    pub groups: Vec<Group>,
    /// The selected row, an index in [`Browser::rows`].
    pub sel: usize,
    /// The filter (`/`), and whether it is being typed.
    pub find: String,
    pub typing: bool,
    /// Checkouts folded to their row (by index in `groups`).
    pub folded: BTreeSet<usize>,
    /// Archives showing their agents.
    pub unfolded: BTreeSet<ProjectId>,
    /// `g` or `z` waiting for the second key of `gg` or `za`.
    pending: Option<char>,
}

impl Browser {
    /// Open on `groups`, the first archive selected.
    pub fn new(w: &World, groups: Vec<Group>) -> Browser {
        let mut b = Browser {
            groups,
            sel: 0,
            find: String::new(),
            typing: false,
            folded: BTreeSet::new(),
            unfolded: BTreeSet::new(),
            pending: None,
        };
        b.sel = (b.rows(w).iter())
            .position(|r| matches!(r, Row::Archive(_)))
            .unwrap_or(0);
        b
    }

    /// The words archive `p` is found by: its title, its project's name,
    /// its checkout and its agents' names.
    fn words(w: &World, checkout: &str, p: ProjectId) -> String {
        let q = &w.projects[p];
        let mut hay = format!("{} {} {checkout}", q.name, title(w, p));
        for a in w.agents.iter().filter(|a| a.project == p) {
            hay.push(' ');
            hay.push_str(&a.name);
        }
        fold(&hay)
    }

    /// The rows shown: each checkout with an archive the filter finds,
    /// and, unless it is folded, the projects open on it and the archives
    /// found, each unfolded one with its agents.
    pub fn rows(&self, w: &World) -> Vec<Row> {
        let words: Vec<String> = self.find.split_whitespace().map(fold).collect();
        let mut rows = Vec::new();
        for (i, g) in self.groups.iter().enumerate() {
            let found: Vec<ProjectId> = (g.archives.iter().copied())
                .filter(|&p| {
                    let hay = Browser::words(w, &g.checkout, p);
                    words.iter().all(|word| hay.contains(word.as_str()))
                })
                .collect();
            if found.is_empty() {
                continue;
            }
            rows.push(Row::Checkout(i));
            if self.folded.contains(&i) {
                continue;
            }
            rows.extend(g.open.iter().map(|&p| Row::Open(p)));
            for p in found {
                rows.push(Row::Archive(p));
                if self.unfolded.contains(&p) {
                    rows.extend(agents_of(w, p).map(Row::Agent));
                }
            }
        }
        rows
    }

    /// The row selected, if any is shown.
    pub fn at(&self, w: &World) -> Option<Row> {
        self.rows(w).get(self.sel).copied()
    }

    /// The archive a row belongs to: its own, or its agent's.
    fn archive_of(w: &World, row: Row) -> Option<ProjectId> {
        match row {
            Row::Archive(p) => Some(p),
            Row::Agent(a) => Some(w.agents[a].project),
            Row::Checkout(_) | Row::Open(_) => None,
        }
    }

    /// The checkout a row is under.
    fn group_of(&self, w: &World, row: Row) -> Option<usize> {
        match row {
            Row::Checkout(i) => Some(i),
            Row::Open(p) => self.groups.iter().position(|g| g.open.contains(&p)),
            Row::Archive(_) | Row::Agent(_) => {
                let p = Browser::archive_of(w, row)?;
                self.groups.iter().position(|g| g.archives.contains(&p))
            }
        }
    }

    /// Selects `row` (one shown).
    fn select(&mut self, w: &World, row: Row) {
        if let Some(i) = self.rows(w).iter().position(|&r| r == row) {
            self.sel = i;
        }
    }

    /// Folds (`true`) or unfolds what the row at hand opens: a checkout
    /// its archives, an archive its agents. Folding on an agent folds its
    /// archive; on an archive already folded or an open project, its
    /// checkout; the selection goes to what was folded.
    fn fold_at(&mut self, w: &World, fold: bool) {
        let Some(row) = self.at(w) else {
            return;
        };
        let to = match (row, fold) {
            (Row::Checkout(i), true) => {
                self.folded.insert(i);
                row
            }
            (Row::Checkout(i), false) => {
                self.folded.remove(&i);
                row
            }
            (Row::Archive(p), false) => {
                self.unfolded.insert(p);
                row
            }
            (Row::Archive(p), true) if self.unfolded.remove(&p) => row,
            (Row::Agent(a), true) => {
                let p = w.agents[a].project;
                self.unfolded.remove(&p);
                Row::Archive(p)
            }
            (Row::Archive(_) | Row::Open(_), true) => match self.group_of(w, row) {
                Some(i) => {
                    self.folded.insert(i);
                    Row::Checkout(i)
                }
                None => row,
            },
            (Row::Open(_) | Row::Agent(_), false) => row,
        };
        self.select(w, to);
    }

    /// `za` (and `enter`): the row at hand's checkout or archive folds
    /// if open, opens if folded.
    fn toggle(&mut self, w: &World) {
        let open = match self.at(w) {
            Some(Row::Checkout(i)) => !self.folded.contains(&i),
            Some(Row::Archive(p)) => self.unfolded.contains(&p),
            Some(Row::Agent(_)) => true,
            Some(Row::Open(_)) | None => return,
        };
        self.fold_at(w, open);
    }

    /// A key while the browser is open; what it asks of the screen, if
    /// anything.
    pub fn on_key(&mut self, w: &World, k: KeyEvent) -> Option<Act> {
        if self.typing {
            self.filter_key(w, k);
            return None;
        }
        let last = self.rows(w).len().saturating_sub(1);
        match (self.pending.take(), k.code) {
            (Some('g'), KeyCode::Char('g')) => self.sel = 0,
            (Some('z'), KeyCode::Char('a')) => self.toggle(w),
            (Some(_), _) => {}
            (None, KeyCode::Char(c @ ('g' | 'z'))) => self.pending = Some(c),
            (None, KeyCode::Home) => self.sel = 0,
            (None, KeyCode::Char('G') | KeyCode::End) => self.sel = last,
            (None, KeyCode::Up | KeyCode::Char('k')) => self.sel = self.sel.saturating_sub(1),
            (None, KeyCode::Down | KeyCode::Char('j')) => self.sel = (self.sel + 1).min(last),
            (None, KeyCode::Left | KeyCode::Char('h')) => self.fold_at(w, true),
            (None, KeyCode::Right | KeyCode::Char('l')) => self.fold_at(w, false),
            (None, KeyCode::Enter) => self.toggle(w),
            (None, KeyCode::Char('/')) => self.typing = true,
            (None, KeyCode::Char('r')) => {
                return self
                    .at(w)
                    .and_then(|r| Browser::archive_of(w, r))
                    .map(Act::Restore);
            }
            (None, KeyCode::Esc) if !self.find.is_empty() => self.set_find(w, String::new()),
            (None, KeyCode::Esc | KeyCode::Char('q' | 'A')) => return Some(Act::Close),
            (None, _) => {}
        }
        None
    }

    /// A key while the filter is typed: `enter` keeps it, `esc` drops it,
    /// `backspace` on nothing stops typing.
    fn filter_key(&mut self, w: &World, k: KeyEvent) {
        let plain = !k
            .modifiers
            .intersects(KeyModifiers::CONTROL | KeyModifiers::ALT);
        match k.code {
            KeyCode::Enter => self.typing = false,
            KeyCode::Esc => {
                self.typing = false;
                self.set_find(w, String::new());
            }
            KeyCode::Backspace => {
                let mut find = self.find.clone();
                if find.pop().is_none() {
                    self.typing = false;
                }
                self.set_find(w, find);
            }
            KeyCode::Char('u') if k.modifiers.contains(KeyModifiers::CONTROL) => {
                self.set_find(w, String::new())
            }
            KeyCode::Char(c) if plain => {
                let mut find = self.find.clone();
                find.push(c);
                self.set_find(w, find);
            }
            _ => {}
        }
    }

    /// Text pasted into the filter while it is typed, on one line; false
    /// when it is not being typed.
    pub fn paste(&mut self, w: &World, text: &str) -> bool {
        if !self.typing {
            return false;
        }
        let find = format!("{}{}", self.find, text.replace(['\n', '\r', '\t'], " "));
        self.set_find(w, find);
        true
    }

    /// The filter is `find` now: the row selected stays selected when it
    /// is still shown, else the first archive found is.
    fn set_find(&mut self, w: &World, find: String) {
        let was = self.at(w);
        self.find = find;
        let rows = self.rows(w);
        self.sel = was
            .and_then(|r| rows.iter().position(|&x| x == r))
            .or_else(|| rows.iter().position(|r| matches!(r, Row::Archive(_))))
            .unwrap_or(0);
    }
}

/// Archive `p`'s agents that are not gone (a helper that finished is),
/// by id.
pub fn agents_of(w: &World, p: ProjectId) -> impl Iterator<Item = AgentId> + '_ {
    (w.agents.iter())
        .filter(move |a| a.project == p && !a.gone)
        .map(|a| a.id)
}

/// Archive `p`'s title (empty when it is not one).
pub fn title(w: &World, p: ProjectId) -> &str {
    w.projects[p]
        .archived
        .as_ref()
        .map_or("", |a| a.title.as_str())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::i18n::Lang;
    use crate::sim;

    fn key(c: KeyCode) -> KeyEvent {
        KeyEvent::new(c, KeyModifiers::NONE)
    }

    /// A demo of two projects; `kule` (project 1) archived twice and
    /// project 0 once. The checkout is the project's name, as the demo's.
    fn archived() -> (World, Vec<ProjectId>) {
        let (mut w, mut s) = sim::world(2, 2, 0, 1, Lang::En);
        let name = w.projects[1].name.clone();
        let fresh = s.archive_project(&mut w, 1, "first epic".into()).unwrap();
        w.tick += 5;
        let again = s.archive_project(&mut w, fresh, "second".into()).unwrap();
        w.tick += 5;
        let other = s.archive_project(&mut w, 0, "çalışma".into()).unwrap();
        assert_eq!(w.projects[again].name, name);
        (w, vec![1, fresh, again, 0, other])
    }

    fn by_name(w: &World) -> impl Fn(ProjectId) -> String + '_ {
        |p| w.projects[p].name.clone()
    }

    #[test]
    fn archives_group_by_checkout_newest_first_with_what_is_open_there() {
        let (w, ids) = archived();
        let [first, second, kule_now, zero, zero_now] = ids[..] else {
            unreachable!()
        };
        let checkout = by_name(&w);
        let g = groups(&w, &checkout, None);
        // Project 0's archive is the newest, so its checkout comes first.
        assert_eq!(
            g,
            [
                Group {
                    checkout: w.projects[0].name.clone(),
                    open: vec![zero_now],
                    archives: vec![zero],
                },
                Group {
                    checkout: w.projects[1].name.clone(),
                    open: vec![kule_now],
                    archives: vec![second, first],
                },
            ]
        );
        // The one at hand's checkout goes first.
        let g = groups(&w, &checkout, Some(kule_now));
        assert_eq!(g[0].archives, [second, first]);
        assert_eq!(g[1].archives, [zero]);
        // A checkout with nothing archived is not listed.
        let (w, _) = sim::world(2, 2, 0, 1, Lang::En);
        assert!(groups(&w, &by_name(&w), Some(0)).is_empty());
    }

    #[test]
    fn rows_fold_and_unfold_and_the_selection_follows() {
        let (w, ids) = archived();
        let [_, second, kule_now, zero, zero_now] = ids[..] else {
            unreachable!()
        };
        let mut b = Browser::new(&w, groups(&w, &by_name(&w), None));
        let rows = b.rows(&w);
        assert_eq!(
            rows,
            [
                Row::Checkout(0),
                Row::Open(zero_now),
                Row::Archive(zero),
                Row::Checkout(1),
                Row::Open(kule_now),
                Row::Archive(second),
                Row::Archive(ids[0]),
            ]
        );
        assert_eq!(b.sel, 2, "the first archive");
        // `l` shows its agents, `h` on one of them folds them again.
        b.on_key(&w, key(KeyCode::Char('l')));
        let agents: Vec<Row> = agents_of(&w, zero).map(Row::Agent).collect();
        assert!(agents.len() >= 2, "a leader and its workers");
        assert_eq!(b.rows(&w)[3..3 + agents.len()], agents[..]);
        b.on_key(&w, key(KeyCode::Char('j')));
        assert_eq!(b.at(&w), Some(agents[0]));
        b.on_key(&w, key(KeyCode::Char('h')));
        assert_eq!(b.at(&w), Some(Row::Archive(zero)), "back on its archive");
        assert_eq!(b.rows(&w).len(), rows.len());
        // `h` on a folded archive folds its checkout, selected.
        b.on_key(&w, key(KeyCode::Char('h')));
        assert_eq!(b.at(&w), Some(Row::Checkout(0)));
        assert_eq!(b.rows(&w)[..2], [Row::Checkout(0), Row::Checkout(1)]);
        // `l` there opens it again; `h` on it folds it.
        b.on_key(&w, key(KeyCode::Right));
        assert_eq!(b.rows(&w), rows);
        assert_eq!(b.sel, 0);
        b.on_key(&w, key(KeyCode::Left));
        assert_eq!(b.rows(&w).len(), 5);
        // `za` and enter toggle the checkout at hand.
        b.on_key(&w, key(KeyCode::Char('z')));
        b.on_key(&w, key(KeyCode::Char('a')));
        assert_eq!(b.rows(&w), rows);
        b.on_key(&w, key(KeyCode::Enter));
        assert_eq!(b.rows(&w).len(), 5);
        b.on_key(&w, key(KeyCode::Enter));
        // On an open project: `h` folds its checkout, `l` and enter do nothing.
        b.sel = 4;
        b.on_key(&w, key(KeyCode::Char('l')));
        b.on_key(&w, key(KeyCode::Enter));
        assert_eq!(b.rows(&w), rows);
        assert_eq!(b.sel, 4);
        b.on_key(&w, key(KeyCode::Char('h')));
        assert_eq!(b.at(&w), Some(Row::Checkout(1)));
        assert_eq!(b.rows(&w).len(), 4);
        // On an agent, enter folds its archive; `l` on an agent does nothing.
        b.on_key(&w, key(KeyCode::Char('l')));
        b.sel = 5;
        b.on_key(&w, key(KeyCode::Enter));
        b.on_key(&w, key(KeyCode::Char('j')));
        let here = b.at(&w);
        assert!(matches!(here, Some(Row::Agent(_))), "{here:?}");
        b.on_key(&w, key(KeyCode::Char('l')));
        assert_eq!(b.at(&w), here);
        b.on_key(&w, key(KeyCode::Enter));
        assert_eq!(b.at(&w), Some(Row::Archive(second)));
        assert_eq!(b.rows(&w), rows);
    }

    #[test]
    fn the_selection_moves_within_the_rows_and_gg_g_go_to_the_ends() {
        let (w, _) = archived();
        let mut b = Browser::new(&w, groups(&w, &by_name(&w), None));
        let last = b.rows(&w).len() - 1;
        for _ in 0..20 {
            b.on_key(&w, key(KeyCode::Char('j')));
        }
        assert_eq!(b.sel, last, "j stops at the last row");
        b.on_key(&w, key(KeyCode::Up));
        assert_eq!(b.sel, last - 1);
        b.on_key(&w, key(KeyCode::Char('g')));
        assert_eq!(b.sel, last - 1, "g waits for the second");
        b.on_key(&w, key(KeyCode::Char('g')));
        assert_eq!(b.sel, 0);
        b.on_key(&w, key(KeyCode::Char('k')));
        assert_eq!(b.sel, 0, "k stops at the first");
        b.on_key(&w, key(KeyCode::Char('G')));
        assert_eq!(b.sel, last);
        b.on_key(&w, key(KeyCode::Home));
        assert_eq!(b.sel, 0);
        b.on_key(&w, key(KeyCode::End));
        assert_eq!(b.sel, last);
        b.on_key(&w, key(KeyCode::Down));
        assert_eq!(b.sel, last);
        // A key after `g` other than `g` does nothing, `j` included.
        b.on_key(&w, key(KeyCode::Char('g')));
        b.on_key(&w, key(KeyCode::Char('k')));
        assert_eq!(b.sel, last);
        b.on_key(&w, key(KeyCode::Char('z')));
        b.on_key(&w, key(KeyCode::Char('j')));
        assert_eq!(b.sel, last);
    }

    #[test]
    fn the_filter_finds_every_word_folded_and_keeps_the_selection() {
        let (w, ids) = archived();
        let [first, second, _, zero, _] = ids[..] else {
            unreachable!()
        };
        let mut b = Browser::new(&w, groups(&w, &by_name(&w), None));
        b.sel = 5;
        assert_eq!(b.at(&w), Some(Row::Archive(second)));
        b.on_key(&w, key(KeyCode::Char('/')));
        assert!(b.typing);
        // Typed letters filter at once; `q` while typing is a letter.
        for c in "EPIC".chars() {
            assert_eq!(b.on_key(&w, key(KeyCode::Char(c))), None);
        }
        let shown: Vec<Row> = b.rows(&w);
        assert_eq!(shown[2..], [Row::Archive(first)]);
        assert_eq!(b.at(&w), Some(Row::Archive(first)), "the one found");
        b.on_key(&w, key(KeyCode::Enter));
        assert!(!b.typing && b.find == "EPIC");
        // Esc drops a filter kept before it closes the browser.
        assert_eq!(b.on_key(&w, key(KeyCode::Esc)), None);
        assert_eq!(b.find, "");
        assert_eq!(b.at(&w), Some(Row::Archive(first)), "still at hand");
        // Turkish letters fold; every word must be there.
        b.on_key(&w, key(KeyCode::Char('/')));
        assert!(b.paste(&w, "calisma\n"));
        assert_eq!(b.rows(&w)[2..], [Row::Archive(zero)]);
        b.on_key(&w, key(KeyCode::Char('%')));
        assert!(b.rows(&w).is_empty(), "calisma %: nothing");
        assert_eq!(b.at(&w), None);
        b.on_key(&w, key(KeyCode::Enter));
        assert_eq!(
            b.on_key(&w, key(KeyCode::Char('r'))),
            None,
            "nothing at hand"
        );
        b.on_key(&w, key(KeyCode::Char('/')));
        b.on_key(&w, key(KeyCode::Backspace));
        assert_eq!(b.find, "calisma ");
        b.on_key(&w, KeyEvent::new(KeyCode::Char('u'), KeyModifiers::CONTROL));
        assert_eq!(b.find, "");
        assert!(b.typing);
        // Backspace on nothing stops typing; alt and ctrl letters are not typed.
        b.on_key(&w, KeyEvent::new(KeyCode::Char('w'), KeyModifiers::ALT));
        assert_eq!(b.find, "");
        b.on_key(&w, key(KeyCode::Backspace));
        assert!(!b.typing);
        assert!(!b.paste(&w, "x"), "not typed: not taken");
        // An agent's name finds its archive.
        let lead = &w.agents[w.projects[zero].leader].name;
        b.on_key(&w, key(KeyCode::Char('/')));
        b.paste(&w, lead);
        assert!(b.rows(&w).contains(&Row::Archive(zero)));
        b.on_key(&w, key(KeyCode::Esc));
        assert!(!b.typing && b.find.is_empty());
        assert_eq!(b.rows(&w).len(), 7);
    }

    #[test]
    fn r_restores_the_archive_at_hand_and_esc_q_a_close() {
        let (w, ids) = archived();
        let [_, second, ..] = ids[..] else {
            unreachable!()
        };
        let mut b = Browser::new(&w, groups(&w, &by_name(&w), None));
        b.sel = 5;
        assert_eq!(
            b.on_key(&w, key(KeyCode::Char('r'))),
            Some(Act::Restore(second))
        );
        // On one of its agents too.
        b.on_key(&w, key(KeyCode::Char('l')));
        b.on_key(&w, key(KeyCode::Char('j')));
        assert_eq!(
            b.on_key(&w, key(KeyCode::Char('r'))),
            Some(Act::Restore(second))
        );
        // Not on a checkout or an open project.
        b.sel = 3;
        assert_eq!(b.on_key(&w, key(KeyCode::Char('r'))), None);
        b.sel = 4;
        assert_eq!(b.on_key(&w, key(KeyCode::Char('r'))), None);
        for c in [KeyCode::Esc, KeyCode::Char('q'), KeyCode::Char('A')] {
            assert_eq!(b.on_key(&w, key(c)), Some(Act::Close), "{c:?}");
        }
        assert_eq!(b.on_key(&w, key(KeyCode::Char('x'))), None);
    }
}
