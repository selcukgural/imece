//! Screen state and keys: which view, how deep, what is selected, which
//! projects are folded, the language, and what the person is typing.

use std::cell::{Cell, RefCell};
use std::collections::BTreeSet;
use std::time::{Duration, Instant};

use ratatui::crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

use crate::i18n::{Lang, fill};
use crate::live::{Live, Refusal};
use crate::model::*;
use crate::prefs::{SIDE_DEFAULT, SIDE_MAX, SIDE_MIN};
use crate::sim::{self, Sim};
use crate::theme;
use crate::ui;

/// What drives the agents.
pub enum Engine {
    Sim(Sim),
    Live(Box<Live>),
}

/// After this long without a key, the next key first shows what happened.
const AWAY: Duration = Duration::from_secs(20);
/// How long a short notice stays in the bottom line.
pub const FLASH: Duration = Duration::from_secs(3);

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum View {
    Salon,
    Board,
    Code,
}

/// How deep the person looks: the whole guild, one agent's card, the
/// agent's raw stream, or a worker's change waiting for them.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Zoom {
    None,
    Focus(AgentId),
    Trace(AgentId),
    Diff(AgentId),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Pane {
    Salon,
    Inbox,
    Journal,
}

/// A conversation opened from the journal: every line between its two
/// parties, oldest first, whole.
pub struct Thread {
    /// Journal indexes.
    pub entries: Vec<usize>,
    /// The selected line, which `c` copies.
    pub sel: usize,
    /// First visible row; the drawing keeps the selected line in view.
    pub scroll: Cell<u16>,
    /// The selection moved: the next drawing scrolls to it.
    pub follow: Cell<bool>,
    /// Rows the last drawing had room for, for paging.
    pub page: Cell<u16>,
}

/// A selectable salon line: a project's header or an agent.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Sel {
    Project(ProjectId),
    Agent(AgentId),
}

/// What happened while the person was away.
pub struct Digest {
    pub minutes: u64,
    pub merged: usize,
    pub blocked: usize,
    pub assumptions: usize,
    pub talks: usize,
    pub waiting: usize,
    pub highlights: Vec<Entry>,
}

impl Digest {
    pub fn since(w: &World, tick: u64) -> Option<Digest> {
        let new: Vec<&Entry> = w.journal.iter().filter(|e| e.tick > tick).collect();
        if new.is_empty() {
            return None;
        }
        let count = |k: EntryKind| new.iter().filter(|e| e.event.kind() == k).count();
        let important = [
            EntryKind::Merged,
            EntryKind::Sensitive,
            EntryKind::Blocked,
            EntryKind::Failed,
            EntryKind::Assumption,
        ];
        let mut highlights: Vec<Entry> = new
            .iter()
            .filter(|e| important.contains(&e.event.kind()))
            .map(|e| (*e).clone())
            .collect();
        let keep = highlights.len().saturating_sub(8);
        highlights.drain(..keep);
        Some(Digest {
            minutes: (w.tick - tick) * SECONDS_PER_TICK / 60,
            merged: count(EntryKind::Merged),
            blocked: count(EntryKind::Blocked),
            assumptions: count(EntryKind::Assumption),
            talks: count(EntryKind::Talk),
            waiting: w.open_inbox().len(),
            highlights,
        })
    }
}

pub struct App {
    pub world: World,
    engine: Engine,
    pub lang: Lang,
    pub view: View,
    pub zoom: Zoom,
    pub pane: Pane,
    pub selected: Sel,
    /// Folded projects show only their header line.
    pub collapsed: Vec<bool>,
    /// Workers whose helpers are folded away.
    pub folded_workers: BTreeSet<AgentId>,
    /// Position in the open inbox items.
    pub inbox_sel: usize,
    /// The first patch line the open change shows.
    pub diff_scroll: usize,
    /// Rows the last drawing of a change had, for paging.
    pub diff_page: Cell<u16>,
    /// The selected journal line (a journal index).
    pub journal_sel: Option<usize>,
    pub thread: Option<Thread>,
    /// The side panel's width, in percent.
    pub side: u16,
    /// Text the person asked to copy; the main loop hands it to the
    /// clipboard.
    pub copy: Option<String>,
    /// The first key of a two-key chord (`g`, `z`), waiting for the second.
    pub pending: Option<char>,
    pub typing: bool,
    pub input: String,
    pub paused: bool,
    pub speed: u32,
    pub help: bool,
    /// The vendor that leads from the next start on without being asked
    /// (`claude`), as the preferences keep it; none asks when there is a
    /// choice.
    pub primary: Option<String>,
    pub digest: Option<Digest>,
    pub flash: Option<(String, Instant)>,
    pub quit: bool,
    /// Salon lines in the order they were shown last, for moving the
    /// selection.
    pub shown: RefCell<Vec<Sel>>,
    last_key: Instant,
    last_key_tick: u64,
}

impl App {
    pub fn new(projects: usize, workers: usize, researchers: usize, seed: u64, lang: Lang) -> App {
        let (world, sim) = sim::world(projects, workers, researchers, seed, lang);
        App::with(world, Engine::Sim(sim), lang)
    }

    pub fn with(world: World, engine: Engine, lang: Lang) -> App {
        let collapsed = vec![false; world.projects.len()];
        let selected = match engine {
            // A live team starts with the leader selected: it is who to talk to.
            Engine::Live(_) => Sel::Agent(world.projects[0].leader),
            Engine::Sim(_) => Sel::Project(0),
        };
        App {
            world,
            engine,
            lang,
            view: View::Salon,
            zoom: Zoom::None,
            pane: Pane::Salon,
            selected,
            collapsed,
            folded_workers: BTreeSet::new(),
            inbox_sel: 0,
            diff_scroll: 0,
            diff_page: Cell::new(10),
            journal_sel: None,
            thread: None,
            side: SIDE_DEFAULT,
            copy: None,
            pending: None,
            typing: false,
            input: String::new(),
            paused: false,
            speed: 1,
            help: false,
            primary: None,
            digest: None,
            flash: None,
            quit: false,
            shown: RefCell::new(Vec::new()),
            last_key: Instant::now(),
            last_key_tick: 0,
        }
    }

    pub fn step(&mut self) {
        match &mut self.engine {
            Engine::Sim(s) => s.step(&mut self.world),
            Engine::Live(l) => l.step(&mut self.world),
        }
    }

    pub fn live(&self) -> bool {
        matches!(self.engine, Engine::Live(_))
    }

    pub fn tick_interval(&self) -> Duration {
        if self.live() {
            // Real time: often enough that events show up at once.
            return Duration::from_millis(100);
        }
        Duration::from_millis(500) / self.speed
    }

    fn flash(&mut self, msg: impl Into<String>) {
        self.flash = Some((msg.into(), Instant::now()));
    }

    /// The project the person is looking at: of the selected line, the
    /// focused agent or the selected inbox item.
    pub fn current_project(&self) -> ProjectId {
        if let Zoom::Focus(id) | Zoom::Trace(id) | Zoom::Diff(id) = self.zoom {
            return self.world.agents[id].project;
        }
        if self.pane == Pane::Inbox
            && let Some(&i) = self.world.open_inbox().get(self.inbox_sel)
        {
            return self.world.agents[self.world.inbox[i].agent].project;
        }
        if self.pane == Pane::Journal
            && let Some(e) = self.journal_sel.and_then(|i| self.world.journal.get(i))
        {
            return self.world.agents[e.event.agent()].project;
        }
        match self.selected {
            Sel::Project(p) => p,
            Sel::Agent(a) => self.world.agents[a].project,
        }
    }

    pub fn on_key(&mut self, k: KeyEvent) {
        let away = self.last_key.elapsed() >= AWAY;
        let since = self.last_key_tick;
        self.last_key = Instant::now();
        self.last_key_tick = self.world.tick;

        if k.modifiers.contains(KeyModifiers::CONTROL) && k.code == KeyCode::Char('c') {
            self.quit = true;
            return;
        }
        // An overlay takes the key that closes it.
        if self.digest.take().is_some() || std::mem::take(&mut self.help) {
            return;
        }
        if away && let Some(d) = Digest::since(&self.world, since) {
            self.digest = Some(d);
            return;
        }
        if self.typing {
            self.type_key(k);
            return;
        }
        if let Some(first) = self.pending.take() {
            self.chord(first, k);
            return;
        }
        if k.modifiers.contains(KeyModifiers::CONTROL) {
            match k.code {
                KeyCode::Char('d') => self.page(true, true),
                KeyCode::Char('u') => self.page(false, true),
                KeyCode::Char('f') => self.page(true, false),
                KeyCode::Char('b') => self.page(false, false),
                _ => {}
            }
            return;
        }
        if self.thread.is_some() && self.thread_key(k) {
            return;
        }
        if let Zoom::Diff(id) = self.zoom
            && self.diff_key(id, k)
        {
            return;
        }

        match k.code {
            KeyCode::Char('q') => self.quit = true,
            // `i` as in vim: start writing.
            KeyCode::Char('/') | KeyCode::Char('i') => self.typing = true,
            KeyCode::Char(':') => {
                self.typing = true;
                self.input = ":".into();
            }
            KeyCode::Char('g') | KeyCode::Char('z') => {
                if let KeyCode::Char(c) = k.code {
                    self.pending = Some(c);
                }
            }
            KeyCode::Char('G') => self.jump(false),
            KeyCode::Char('t') => {
                if let Some(id) = self.agent_at_hand() {
                    self.zoom = Zoom::Trace(id);
                }
            }
            KeyCode::Char('v') => {
                if let Some(id) = self.agent_at_hand() {
                    self.open_talk_with(id);
                }
            }
            KeyCode::Char('@') => {
                self.typing = true;
                self.input = match self.zoom {
                    Zoom::Focus(id) | Zoom::Trace(id) | Zoom::Diff(id) => {
                        format!("@{} ", self.world.agents[id].name)
                    }
                    Zoom::None => "@".into(),
                };
            }
            KeyCode::Char('1') => self.show(View::Salon),
            KeyCode::Char('2') => self.show(View::Board),
            KeyCode::Char('3') => self.show(View::Code),
            KeyCode::Char('T') => {
                let name = theme::next().name;
                self.flash(fill(self.lang.text().theme_is, &[name]));
            }
            KeyCode::Char('L') => {
                self.lang = self.lang.other();
                if let Engine::Sim(s) = &mut self.engine {
                    s.set_lang(self.lang);
                }
                self.flash(self.lang.text().language);
            }
            // Live agents run in real time: nothing to pause or speed up.
            KeyCode::Char(' ') if !self.live() => self.paused = !self.paused,
            KeyCode::Char('+') | KeyCode::Char('=') if !self.live() => {
                self.speed = (self.speed * 2).min(8)
            }
            KeyCode::Char('-') if !self.live() => self.speed = (self.speed / 2).max(1),
            KeyCode::Char('?') => self.help = true,
            KeyCode::Tab | KeyCode::Char(']') if self.pane_keys() => self.cycle_pane(true),
            KeyCode::BackTab | KeyCode::Char('[') if self.pane_keys() => self.cycle_pane(false),
            KeyCode::Char('!') if self.pane_keys() => self.pane = Pane::Inbox,
            // `,` and `.` are the same keys without shift.
            KeyCode::Char('<') | KeyCode::Char(',') => self.resize(true),
            KeyCode::Char('>') | KeyCode::Char('.') => self.resize(false),
            KeyCode::Char('c') | KeyCode::Char('y')
                if self.pane == Pane::Journal && self.zoom == Zoom::None =>
            {
                if let Some(i) = self.journal_sel {
                    self.copy = Some(ui::copy_text(self.lang, &self.world, &[i]));
                }
            }
            KeyCode::Up | KeyCode::Char('k') => self.move_selection(false),
            KeyCode::Down | KeyCode::Char('j') => self.move_selection(true),
            KeyCode::Left | KeyCode::Char('h') if self.salon_keys() => self.fold(true),
            KeyCode::Right | KeyCode::Char('l') if self.salon_keys() => self.fold(false),
            KeyCode::Enter => self.deeper(),
            KeyCode::Esc => {
                self.zoom = match self.zoom {
                    Zoom::Trace(id) => Zoom::Focus(id),
                    _ => Zoom::None,
                };
            }
            KeyCode::Char('a') if self.pane == Pane::Inbox => self.answer(true),
            KeyCode::Char('r') if self.pane == Pane::Inbox => self.answer(false),
            _ => {}
        }
    }

    /// The second key of a chord that began with `first`.
    fn chord(&mut self, first: char, k: KeyEvent) {
        match (first, k.code) {
            ('g', KeyCode::Char('g')) => self.jump(true),
            ('z', KeyCode::Char('a')) if self.salon_keys() => {
                let folded = match self.worker_at_hand() {
                    Some(id) => self.folded_workers.contains(&id),
                    None => self.collapsed[self.current_project()],
                };
                self.fold(!folded);
            }
            ('z', KeyCode::Char('M')) if self.salon_keys() => self.fold_all(true),
            ('z', KeyCode::Char('R')) if self.salon_keys() => self.fold_all(false),
            _ => {}
        }
    }

    /// Folds or unfolds every project and every worker's helpers.
    fn fold_all(&mut self, fold: bool) {
        self.collapsed.iter_mut().for_each(|c| *c = fold);
        self.folded_workers.clear();
        if fold {
            let workers = self
                .world
                .agents
                .iter()
                .filter(|a| a.role == Role::Worker && self.world.helpers_of(a.id).next().is_some())
                .map(|a| a.id);
            self.folded_workers.extend(workers);
            self.selected = Sel::Project(self.current_project());
        }
    }

    /// The worker whose helpers the fold keys act on: the selected worker
    /// if it has helpers, or the selected helper's worker.
    fn worker_at_hand(&self) -> Option<AgentId> {
        let Sel::Agent(id) = self.selected else {
            return None;
        };
        let a = &self.world.agents[id];
        let worker = match a.role {
            Role::Worker => id,
            Role::Helper => a.parent?,
            _ => return None,
        };
        self.world.helpers_of(worker).next().map(|_| worker)
    }

    /// Panels cycle only on the salon, outside a focus.
    fn pane_keys(&self) -> bool {
        self.view == View::Salon && self.zoom == Zoom::None
    }

    fn cycle_pane(&mut self, forward: bool) {
        const ORDER: [Pane; 3] = [Pane::Salon, Pane::Inbox, Pane::Journal];
        let at = ORDER.iter().position(|&p| p == self.pane).unwrap_or(0);
        let next = if forward {
            at + 1
        } else {
            at + ORDER.len() - 1
        } % ORDER.len();
        self.pane = ORDER[next];
        if self.pane == Pane::Journal {
            self.select_journal(None);
        }
    }

    /// The agent the person is looking at: focused, selected in the salon,
    /// or the one behind the selected inbox item.
    fn agent_at_hand(&self) -> Option<AgentId> {
        match self.zoom {
            Zoom::Focus(id) | Zoom::Trace(id) | Zoom::Diff(id) => return Some(id),
            Zoom::None => {}
        }
        match self.pane {
            Pane::Salon => match self.selected {
                Sel::Agent(id) => Some(id),
                Sel::Project(p) => Some(self.world.projects[p].leader),
            },
            Pane::Inbox => self
                .world
                .open_inbox()
                .get(self.inbox_sel)
                .map(|&i| self.world.inbox[i].agent),
            Pane::Journal => self
                .journal_sel
                .map(|i| self.world.journal[i].event.agent()),
        }
    }

    /// Opens the conversation between the person and agent `id`, if they
    /// have one.
    fn open_talk_with(&mut self, id: AgentId) {
        let me = Party::Agent(id);
        let pair = (me.min(Party::Person), me.max(Party::Person));
        let mine = |e: &Entry| ui::words::said(&e.event).is_some_and(|s| s.pair() == pair);
        match self.world.journal.iter().rposition(mine) {
            Some(i) => self.open_thread(i),
            None => {
                let name = self.world.agents[id].name.clone();
                let t = self.lang.text();
                self.flash(fill(t.no_talk_yet, &[&name]));
            }
        }
    }

    /// To the first or last line of what the person is looking at.
    fn jump(&mut self, first: bool) {
        if let Zoom::Diff(id) = self.zoom {
            let lines = self
                .world
                .change_of(id)
                .map_or(0, |c| c.patch.lines().count());
            self.diff_scroll = if first { 0 } else { lines.saturating_sub(1) };
            return;
        }
        if let Some(th) = &mut self.thread {
            th.sel = if first {
                0
            } else {
                th.entries.len().saturating_sub(1)
            };
            th.follow.set(true);
            return;
        }
        match self.pane {
            Pane::Salon => {
                let shown = self.shown.borrow();
                if let Some(&s) = if first { shown.first() } else { shown.last() } {
                    self.selected = s;
                }
            }
            Pane::Inbox => {
                let n = self.world.open_inbox().len();
                self.inbox_sel = if first { 0 } else { n.saturating_sub(1) };
            }
            Pane::Journal => {
                let rows = ui::journal_rows(&self.world);
                let row = if first { rows.first() } else { rows.last() };
                self.journal_sel = row.map(|&(i, _)| i);
            }
        }
    }

    /// A page (or half of one) down or up: the open conversation scrolls,
    /// a list moves its selection.
    fn page(&mut self, down: bool, half: bool) {
        if let Zoom::Diff(id) = self.zoom {
            let page = usize::from(self.diff_page.get()).max(1);
            let step = if half { page / 2 } else { page }.max(1);
            let lines = self
                .world
                .change_of(id)
                .map_or(0, |c| c.patch.lines().count());
            self.diff_scroll = if down {
                (self.diff_scroll + step).min(lines.saturating_sub(1))
            } else {
                self.diff_scroll.saturating_sub(step)
            };
            return;
        }
        if let Some(th) = &self.thread {
            let step = if half {
                th.page.get() / 2
            } else {
                th.page.get()
            }
            .max(1);
            let at = th.scroll.get();
            th.scroll.set(if down {
                at.saturating_add(step)
            } else {
                at.saturating_sub(step)
            });
            return;
        }
        for _ in 0..if half { 5 } else { 10 } {
            self.move_selection(down);
        }
    }

    /// Makes the side panel wider or narrower, and says how wide it is.
    fn resize(&mut self, wider: bool) {
        self.side = if wider {
            (self.side + 4).min(SIDE_MAX)
        } else {
            self.side.saturating_sub(4).max(SIDE_MIN)
        };
        let t = self.lang.text();
        self.flash(fill(t.side_width, &[&self.side.to_string()]));
    }

    /// A key for the open change of worker `id`; `false` leaves it to the
    /// rest of the screen (typing, switching views, quitting).
    fn diff_key(&mut self, id: AgentId, k: KeyEvent) -> bool {
        let lines = self
            .world
            .change_of(id)
            .map_or(0, |c| c.patch.lines().count());
        let last = lines.saturating_sub(1);
        let page = usize::from(self.diff_page.get()).max(1);
        match k.code {
            KeyCode::Esc => {
                self.zoom = Zoom::None;
                self.pane = Pane::Inbox;
            }
            KeyCode::Up | KeyCode::Char('k') => {
                self.diff_scroll = self.diff_scroll.saturating_sub(1)
            }
            KeyCode::Down | KeyCode::Char('j') => {
                self.diff_scroll = (self.diff_scroll + 1).min(last)
            }
            KeyCode::PageUp => self.diff_scroll = self.diff_scroll.saturating_sub(page),
            KeyCode::PageDown | KeyCode::Char(' ') => {
                self.diff_scroll = (self.diff_scroll + page).min(last)
            }
            KeyCode::Home => self.diff_scroll = 0,
            KeyCode::End | KeyCode::Char('G') => self.diff_scroll = last,
            KeyCode::Char('[') | KeyCode::Char(']') => {
                let starts = self
                    .world
                    .change_of(id)
                    .map(|c| ui::file_starts(&c.patch))
                    .unwrap_or_default();
                let at = self.diff_scroll;
                let to = if k.code == KeyCode::Char(']') {
                    starts.iter().find(|&&s| s > at).copied()
                } else {
                    starts.iter().rev().find(|&&s| s < at).copied()
                };
                if let Some(to) = to {
                    self.diff_scroll = to;
                }
            }
            KeyCode::Char('a') => self.decide_change(id, true),
            KeyCode::Char('r') => self.decide_change(id, false),
            // Writing, views, language, help and quitting work as anywhere.
            _ => return false,
        }
        true
    }

    /// A key for the open conversation; `false` leaves it to the rest of
    /// the screen (typing, switching views, quitting).
    fn thread_key(&mut self, k: KeyEvent) -> bool {
        let Some(th) = &mut self.thread else {
            return false;
        };
        let last = th.entries.len().saturating_sub(1);
        match k.code {
            KeyCode::Esc => self.thread = None,
            KeyCode::Up | KeyCode::Char('k') => {
                th.sel = th.sel.saturating_sub(1);
                th.follow.set(true);
            }
            KeyCode::Down | KeyCode::Char('j') => {
                th.sel = (th.sel + 1).min(last);
                th.follow.set(true);
            }
            KeyCode::PageUp => th.scroll.set(th.scroll.get().saturating_sub(th.page.get())),
            KeyCode::PageDown | KeyCode::Char(' ') => {
                th.scroll.set(th.scroll.get().saturating_add(th.page.get()))
            }
            KeyCode::Home => {
                th.sel = 0;
                th.follow.set(true);
            }
            KeyCode::End | KeyCode::Char('G') => {
                th.sel = last;
                th.follow.set(true);
            }
            KeyCode::Char('c') | KeyCode::Char('y') => {
                let one = th.entries[th.sel];
                self.copy = Some(ui::copy_text(self.lang, &self.world, &[one]));
            }
            KeyCode::Char('C') | KeyCode::Char('Y') => {
                self.copy = Some(ui::copy_text(self.lang, &self.world, &th.entries));
            }
            // Writing, views, language, help and quitting work as anywhere.
            _ => return false,
        }
        true
    }

    /// Opens the conversation that journal line `i` belongs to, with that
    /// line selected.
    pub fn open_thread(&mut self, i: usize) {
        let entries = ui::thread_of(&self.world, i);
        let sel = entries.iter().position(|&e| e == i).unwrap_or(0);
        self.thread = Some(Thread {
            entries,
            sel,
            scroll: Cell::new(0),
            follow: Cell::new(true),
            page: Cell::new(10),
        });
    }

    /// The person's copy request went through, or did not.
    pub fn copied(&mut self, how: Result<&str, String>) {
        let t = self.lang.text();
        let msg = match how {
            Ok(how) => fill(t.copied, &[how]),
            Err(why) => fill(t.copy_failed, &[&why]),
        };
        self.flash(msg);
    }

    /// Keeps the journal selection on a line that is still shown, moving by
    /// `step` lines (newest first) when given.
    fn select_journal(&mut self, step: Option<bool>) {
        let rows = ui::journal_rows(&self.world);
        if rows.is_empty() {
            self.journal_sel = None;
            return;
        }
        let pos = self
            .journal_sel
            .and_then(|i| rows.iter().position(|&(r, _)| r == i));
        let pos = match (pos, step) {
            (None, _) => 0,
            (Some(p), None) => p,
            (Some(p), Some(down)) if down => (p + 1).min(rows.len() - 1),
            (Some(p), Some(_)) => p.saturating_sub(1),
        };
        self.journal_sel = Some(rows[pos].0);
    }

    fn salon_keys(&self) -> bool {
        self.view == View::Salon && self.zoom == Zoom::None && self.pane == Pane::Salon
    }

    fn show(&mut self, view: View) {
        self.view = view;
        self.zoom = Zoom::None;
        if view != View::Salon {
            self.pane = Pane::Salon;
        }
    }

    /// Folds or unfolds the selected worker's helpers, when that changes
    /// something, else the selected project; folding moves the selection to
    /// the worker or the project's header so it does not vanish.
    fn fold(&mut self, fold: bool) {
        if let Some(id) = self.worker_at_hand()
            && fold != self.folded_workers.contains(&id)
        {
            if fold {
                self.folded_workers.insert(id);
                self.selected = Sel::Agent(id);
            } else {
                self.folded_workers.remove(&id);
            }
            return;
        }
        let p = self.current_project();
        self.collapsed[p] = fold;
        if fold {
            self.selected = Sel::Project(p);
        }
    }

    fn type_key(&mut self, k: KeyEvent) {
        match k.code {
            KeyCode::Enter => self.submit(),
            KeyCode::Esc => {
                self.typing = false;
                self.input.clear();
            }
            KeyCode::Backspace => {
                self.input.pop();
            }
            KeyCode::Char(c) => self.input.push(c),
            _ => {}
        }
    }

    /// Who a message being typed goes to: the named agent, or the leader of
    /// the current project.
    /// Whether what is being typed is a command to imece (`:budget 80`),
    /// not a message to an agent.
    pub fn typing_command(&self) -> bool {
        self.input.starts_with(':')
    }

    pub fn typing_target(&self) -> String {
        match self.input.strip_prefix('@') {
            Some(rest) => rest.split(' ').next().unwrap_or_default().to_string(),
            None => {
                let leader = self.world.projects[self.current_project()].leader;
                self.world.agents[leader].name.clone()
            }
        }
    }

    fn submit(&mut self) {
        let text = self.input.trim().to_string();
        let target = self.typing_target();
        self.input.clear();
        self.typing = false;
        if text.is_empty() {
            return;
        }
        let t = self.lang.text();
        if let Some(line) = text.strip_prefix(':') {
            return self.command(line.trim());
        }
        let Some(rest) = text.strip_prefix('@') else {
            let leader = self.world.projects[self.current_project()].leader;
            return self.tell(leader, &text);
        };
        let msg = rest.split_once(' ').map_or("", |(_, m)| m).trim();
        match self.world.agent_by_name(&target) {
            Some(_) if msg.is_empty() => self.flash(t.empty_message),
            Some(id) => self.tell(id, msg),
            None => self.flash(fill(t.no_agent, &[&target])),
        }
    }

    /// The person's message to agent `id`: the engine gets it first, and only
    /// a message that reached the agent is recorded as said. A line that
    /// starts with `/` is a command of the agent's own command line
    /// (`/clear`, `/compact`, `/cost`), not a message.
    fn tell(&mut self, id: AgentId, text: &str) {
        let command = text.starts_with('/');
        let refused = match &mut self.engine {
            Engine::Sim(_) if command => Some(Refusal::Command(
                self.lang.text().demo_no_command_line.to_string(),
            )),
            Engine::Sim(s) => {
                s.told(&mut self.world, id, text);
                None
            }
            Engine::Live(l) if command => l.command(&mut self.world, id, text).err(),
            Engine::Live(l) => l.told(&mut self.world, id, text).err(),
        };
        self.report(id, text, refused);
    }

    /// Flashes how the person's `text` to agent `id` fared.
    fn report(&mut self, id: AgentId, text: &str, refused: Option<Refusal>) {
        let t = self.lang.text();
        let name = self.world.agents[id].name.clone();
        match refused {
            None => {
                self.world.said(id, text);
                self.flash(fill(t.sent, &[&name]));
            }
            Some(Refusal::Budget) => self.flash(fill(t.budget_spent, &[&name])),
            Some(Refusal::Start(why)) => self.flash(fill(t.cannot_start, &[&name, &why])),
            Some(Refusal::Command(why)) => self.flash(fill(t.command_refused, &[&name, &why])),
        }
    }

    /// A command to imece itself, typed after `:`. `budget USD` (or `b`)
    /// sets what all agents together may spend; a running turn finishes
    /// on its old cap, the next one starts on the new remainder. `lead
    /// VENDOR` makes that vendor the primary one, leading from the next
    /// start on without asking; `lead ask` forgets it.
    fn command(&mut self, line: &str) {
        let t = self.lang.text();
        let (name, arg) = line
            .split_once(char::is_whitespace)
            .map_or((line, ""), |(n, a)| (n, a.trim()));
        match name {
            "budget" | "b" => match parse_dollars(arg) {
                Some(to) => {
                    let from = self.world.daily_budget;
                    self.world.daily_budget = to;
                    let leader = self.world.projects[self.current_project()].leader;
                    self.world.note(Event::Budget { leader, from, to });
                    self.flash(fill(t.budget_is, &[&dollars(to)]));
                }
                None => self.flash(fill(t.bad_budget, &[arg])),
            },
            "lead" => match (arg, Vendor::parse(arg)) {
                ("ask", _) => {
                    self.primary = None;
                    self.flash(t.lead_asks);
                }
                (_, Some(v)) => {
                    self.primary = Some(v.label().to_string());
                    self.flash(fill(t.lead_is, &[v.label()]));
                }
                (_, None) => self.flash(fill(t.bad_lead, &[arg])),
            },
            _ => self.flash(fill(t.no_command, &[name])),
        }
    }

    fn move_selection(&mut self, down: bool) {
        match self.pane {
            Pane::Journal => self.select_journal(Some(down)),
            Pane::Inbox => {
                let n = self.world.open_inbox().len();
                if n > 0 {
                    self.inbox_sel = if down {
                        (self.inbox_sel + 1).min(n - 1)
                    } else {
                        self.inbox_sel.saturating_sub(1)
                    };
                }
            }
            Pane::Salon => {
                let next = {
                    let shown = self.shown.borrow();
                    if shown.is_empty() {
                        return;
                    }
                    let pos = shown.iter().position(|&s| s == self.selected).unwrap_or(0);
                    let pos = if down {
                        (pos + 1).min(shown.len() - 1)
                    } else {
                        pos.saturating_sub(1)
                    };
                    shown[pos]
                };
                self.selected = next;
            }
        }
    }

    fn deeper(&mut self) {
        let zoom = match self.zoom {
            Zoom::None => match (self.pane, self.selected) {
                (Pane::Inbox, _) => {
                    let open = self.world.open_inbox();
                    let item = open.get(self.inbox_sel).map(|&i| &self.world.inbox[i]);
                    match item {
                        Some(item) if item.kind == InboxKind::Change => {
                            self.diff_scroll = 0;
                            Some(Zoom::Diff(item.agent))
                        }
                        Some(item) if item.kind == InboxKind::Reply => {
                            // Read now: the conversation shows the whole answer.
                            let agent = item.agent;
                            self.world.close_replies(agent);
                            self.inbox_sel = self.inbox_sel.min(open.len().saturating_sub(2));
                            if let Some(i) = self.world.journal.iter().rposition(|e| {
                                matches!(e.event, Event::Replied { agent: a, .. } if a == agent)
                            }) {
                                self.open_thread(i);
                                return;
                            }
                            Some(Zoom::Focus(agent))
                        }
                        Some(item) => Some(Zoom::Focus(item.agent)),
                        None => None,
                    }
                }
                (Pane::Journal, _) => {
                    if let Some(i) = self.journal_sel {
                        self.open_thread(i);
                    }
                    None
                }
                (Pane::Salon, Sel::Project(p)) => {
                    self.collapsed[p] = !self.collapsed[p];
                    None
                }
                (Pane::Salon, Sel::Agent(id)) => Some(Zoom::Focus(id)),
            },
            Zoom::Focus(id) | Zoom::Trace(id) => Some(Zoom::Trace(id)),
            Zoom::Diff(_) => None,
        };
        if let Some(z) = zoom {
            self.zoom = z;
        }
    }

    fn answer(&mut self, approve: bool) {
        let open = self.world.open_inbox();
        let Some(&item) = open.get(self.inbox_sel) else {
            return;
        };
        let t = self.lang.text();
        if self.world.inbox[item].kind == InboxKind::Change {
            return self.decide_change(self.world.inbox[item].agent, approve);
        }
        if self.world.inbox[item].kind == InboxKind::Reply {
            self.world.close_replies(self.world.inbox[item].agent);
        } else if approve {
            self.world.approve(item);
            self.flash(t.approved);
        } else {
            self.world.reject(item);
            self.flash(t.rejected);
        }
        self.inbox_sel = self.inbox_sel.min(open.len().saturating_sub(2));
    }

    /// The person applies worker `id`'s change to their checkout, or
    /// rejects it; a rejection opens the composer to that worker, so they
    /// say why. The change's view closes either way.
    fn decide_change(&mut self, id: AgentId, apply: bool) {
        let t = self.lang.text();
        let name = self.world.agents[id].name.clone();
        let open = self.world.open_inbox().len();
        match &mut self.engine {
            Engine::Sim(_) => self.flash(t.demo_no_change),
            Engine::Live(l) if apply => match l.apply_change(&mut self.world, id) {
                Ok(()) => self.flash(fill(t.applied, &[&name])),
                Err(why) => self.flash(fill(t.apply_failed, &[&name, &why])),
            },
            Engine::Live(l) => {
                l.reject_change(&mut self.world, id);
                self.flash(fill(t.rejected_change, &[&name]));
                self.typing = true;
                self.input = format!("@{name} ");
            }
        }
        if self.zoom == Zoom::Diff(id) {
            self.zoom = Zoom::None;
            self.pane = Pane::Inbox;
        }
        self.inbox_sel = self.inbox_sel.min(open.saturating_sub(2));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn key(code: KeyCode) -> KeyEvent {
        KeyEvent::new(code, KeyModifiers::NONE)
    }

    fn ctrl(c: char) -> KeyEvent {
        KeyEvent::new(KeyCode::Char(c), KeyModifiers::CONTROL)
    }

    /// A demo team with one worker whose change, `patch`, waits for the
    /// person and is open on the screen.
    fn with_change(patch: &str) -> (App, AgentId) {
        let mut app = App::new(1, 1, 0, 1, Lang::En);
        let worker = app
            .world
            .agents
            .iter()
            .find(|a| a.role == Role::Worker)
            .map(|a| a.id)
            .expect("the demo team has a worker");
        app.world.change_ready(Change {
            agent: worker,
            title: "did it".into(),
            files: vec![ChangedFile {
                path: "x".into(),
                added: 1,
                removed: 0,
                binary: false,
            }],
            patch: patch.into(),
        });
        app.zoom = Zoom::Diff(worker);
        app.pane = Pane::Inbox;
        (app, worker)
    }

    fn two_files() -> String {
        let mut p = String::from("diff --git a/x b/x\n--- a/x\n+++ b/x\n+one\n");
        p.push_str("diff --git a/y b/y\n--- a/y\n+++ b/y\n+two\n+three\n");
        p
    }

    #[test]
    fn a_change_pages_by_whole_and_half_screens() {
        let patch: String = (0..30).map(|i| format!("+line {i}\n")).collect();
        let (mut app, _) = with_change(&patch);
        app.diff_page.set(10);
        app.on_key(ctrl('d'));
        assert_eq!(app.diff_scroll, 5, "half a page down");
        app.on_key(ctrl('f'));
        assert_eq!(app.diff_scroll, 15, "a whole page down");
        app.on_key(ctrl('u'));
        assert_eq!(app.diff_scroll, 10, "half a page up");
        app.on_key(ctrl('b'));
        assert_eq!(app.diff_scroll, 0, "a whole page up stops at the top");
        for _ in 0..5 {
            app.on_key(ctrl('f'));
        }
        assert_eq!(app.diff_scroll, 29, "paging stops at the last line");
    }

    #[test]
    fn bracket_keys_jump_between_file_headers() {
        let (mut app, _) = with_change(&two_files());
        assert_eq!(app.diff_scroll, 0);
        app.on_key(key(KeyCode::Char(']')));
        assert_eq!(
            app.diff_scroll, 4,
            "from a header, ] goes to the next file, not itself"
        );
        app.on_key(key(KeyCode::Char(']')));
        assert_eq!(app.diff_scroll, 4, "no file after the last one");
        app.on_key(key(KeyCode::Char('j')));
        app.on_key(key(KeyCode::Char('[')));
        assert_eq!(
            app.diff_scroll, 4,
            "[ goes back to the current file's header"
        );
        app.on_key(key(KeyCode::Char('[')));
        assert_eq!(app.diff_scroll, 0, "and then to the one before");
        app.on_key(key(KeyCode::Char('[')));
        assert_eq!(app.diff_scroll, 0, "no file before the first one");
    }

    #[test]
    fn a_and_r_decide_the_open_change_and_close_it() {
        for c in ['a', 'r'] {
            let (mut app, worker) = with_change(&two_files());
            app.on_key(key(KeyCode::Char(c)));
            assert_eq!(app.zoom, Zoom::None, "{c} closes the change");
            assert_eq!(app.pane, Pane::Inbox, "{c} returns to the inbox");
            let flash = app.flash.as_ref().map(|(m, _)| m.as_str());
            assert_eq!(
                flash,
                Some(Lang::En.text().demo_no_change),
                "{c} on the demo says there is nothing to apply"
            );
            assert!(
                app.world.change_of(worker).is_some(),
                "the demo keeps the change"
            );
        }
    }

    /// The inbox selection may have moved while the change was open (an
    /// earlier item closed, a new one came): `a` and `r` decide the change
    /// in view, not the item the selection landed on.
    #[test]
    fn a_and_r_decide_the_change_in_view_not_the_selected_inbox_item() {
        for c in ['a', 'r'] {
            let (mut app, worker) = with_change(&two_files());
            let leader = app.world.projects[0].leader;
            app.world.inbox.push(InboxItem {
                kind: InboxKind::Sensitive,
                agent: leader,
                detail: "bump a dependency".into(),
                researcher: None,
                open: true,
                tick: 0,
            });
            app.inbox_sel = 1;
            app.on_key(key(KeyCode::Char(c)));
            assert_eq!(app.zoom, Zoom::None, "{c} closes the change");
            let flash = app.flash.as_ref().map(|(m, _)| m.as_str());
            assert_eq!(flash, Some(Lang::En.text().demo_no_change));
            assert!(
                app.world.change_of(worker).is_some(),
                "the demo keeps the change"
            );
            let other = app
                .world
                .inbox
                .iter()
                .find(|i| i.kind == InboxKind::Sensitive);
            assert!(
                other.is_some_and(|i| i.open),
                "{c} must not decide the item the selection moved to"
            );
        }
    }

    #[test]
    fn esc_leaves_the_change_open_in_the_inbox() {
        let (mut app, worker) = with_change(&two_files());
        app.on_key(key(KeyCode::Char('G')));
        assert_eq!(app.diff_scroll, 8);
        app.on_key(key(KeyCode::Esc));
        assert_eq!(app.zoom, Zoom::None);
        assert_eq!(app.pane, Pane::Inbox);
        assert!(app.world.change_of(worker).is_some());
        assert!(app.flash.is_none());
    }
}
