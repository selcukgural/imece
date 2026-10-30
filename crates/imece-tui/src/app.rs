//! Screen state and keys: which view, how deep, what is selected, which
//! projects are folded, the language, and what the person is typing.

use std::cell::{Cell, RefCell};
use std::collections::BTreeSet;
use std::io;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use ratatui::crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

use crate::complete::{self, Menu};
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

/// Where the person left off: what an earlier run of imece left behind,
/// told once at the start, before anything moves.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LeftOff {
    /// Seconds since the earlier run last wrote the store; none when the
    /// file does not say.
    pub ago: Option<u64>,
    /// What all agents spent so far, and the pot.
    pub spent: u64,
    pub budget: u64,
    /// Every open project: its name, its agents (the ones that are still
    /// there, asleep) and its cards that are not done.
    pub projects: Vec<(String, usize, usize)>,
    /// The oldest open inbox items (indexes), at most [`LEFT_SHOWN`], and
    /// how many there are in all.
    pub waiting: Vec<usize>,
    pub waiting_total: usize,
    /// Agents whose turn the earlier run cut short.
    pub interrupted: Vec<AgentId>,
    /// The earlier runs' last journal lines, at most [`LEFT_SHOWN`].
    pub last: Vec<Entry>,
}

/// Lines of one kind the "where you left off" window shows, at most.
pub const LEFT_SHOWN: usize = 6;

impl LeftOff {
    /// From a world the store gave back, the first `journal` lines of
    /// whose journal are the earlier runs' (the rest this start's), the
    /// store last written `ago` seconds ago. None when there is nothing to
    /// tell: no lines from before, nothing waiting.
    pub fn of(w: &World, journal: usize, ago: Option<u64>) -> Option<LeftOff> {
        let journal = journal.min(w.journal.len());
        let open = w.open_inbox();
        if journal == 0 && open.is_empty() {
            return None;
        }
        let projects = w
            .open_projects()
            .map(|q| {
                let agents = w
                    .agents
                    .iter()
                    .filter(|a| a.project == q.id && !a.gone)
                    .count();
                let cards = w
                    .cards
                    .iter()
                    .filter(|c| c.project == q.id && c.column != Column::Done)
                    .count();
                (q.name.clone(), agents, cards)
            })
            .collect();
        let interrupted = w.journal[journal..]
            .iter()
            .filter_map(|e| match e.event {
                Event::Interrupted { agent } => Some(agent),
                _ => None,
            })
            .collect();
        let last = w.journal[journal.saturating_sub(LEFT_SHOWN)..journal].to_vec();
        Some(LeftOff {
            ago,
            spent: w.spent(),
            budget: w.budget,
            projects,
            waiting_total: open.len(),
            waiting: open.into_iter().take(LEFT_SHOWN).collect(),
            interrupted,
            last,
        })
    }
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
    /// A command the person typed after `!`; the main loop steps the
    /// screen aside and runs it in their shell, then calls `ran`.
    pub shell: Option<String>,
    /// The person's home directory, for `~` in a path they type.
    pub home: Option<PathBuf>,
    /// The first key of a two-key chord (`g`, `z`), waiting for the second.
    pub pending: Option<char>,
    pub typing: bool,
    pub input: String,
    /// What the composer offers for the word being typed, if anything.
    pub menu: Option<Menu>,
    pub paused: bool,
    pub speed: u32,
    pub help: bool,
    /// The vendor that leads from the next start on without being asked
    /// (`claude`), as the preferences keep it; none asks when there is a
    /// choice.
    pub primary: Option<String>,
    pub digest: Option<Digest>,
    /// What an earlier run left behind, shown once at the start.
    pub left_off: Option<LeftOff>,
    pub flash: Option<(String, Instant)>,
    pub quit: bool,
    /// The store's last failure, flashed once.
    store_error: Option<String>,
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
        let selected = match &engine {
            // A live team starts with the leader of the checkout at hand
            // selected: it is who to talk to.
            Engine::Live(l) => Sel::Agent(world.projects[l.home()].leader),
            Engine::Sim(_) => Sel::Project(0),
        };
        let left_off = match &engine {
            Engine::Live(l) => l
                .left_off()
                .and_then(|left| LeftOff::of(&world, left.journal, left.ago)),
            Engine::Sim(_) => None,
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
            shell: None,
            home: std::env::var_os("HOME").map(PathBuf::from),
            pending: None,
            typing: false,
            input: String::new(),
            menu: None,
            paused: false,
            speed: 1,
            help: false,
            primary: None,
            digest: None,
            left_off,
            flash: None,
            quit: false,
            store_error: None,
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
        self.world.watch_spending();
        self.follow_thread();
        self.persist();
    }

    /// Saves what moved, for the next run. A store that fails is said
    /// once per failure, not on every step.
    pub fn persist(&mut self) {
        let Engine::Live(l) = &mut self.engine else {
            return;
        };
        match l.persist(&self.world) {
            Ok(_) => self.store_error = None,
            Err(e) => {
                if self.store_error.as_ref() != Some(&e) {
                    let t = self.lang.text();
                    self.flash(fill(t.store_failed, &[&e]));
                    self.store_error = Some(e);
                }
            }
        }
    }

    /// The last word to the store on the way out, after the last save.
    pub fn leave(&mut self) {
        if let Engine::Live(l) = &mut self.engine {
            l.leave();
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
        let p = self.project_at_hand();
        if self.world.projects[p].closed {
            // A closed project's journal lines still lead to it.
            self.world.open_projects().next().map_or(p, |q| q.id)
        } else {
            p
        }
    }

    /// The project the zoom, the inbox or journal selection, or the salon
    /// selection points at.
    fn project_at_hand(&self) -> ProjectId {
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
        self.key(k);
        self.follow_thread();
        self.persist();
    }

    /// Keeps an open conversation current: the journal lines of its two
    /// parties that came since it was opened (or last followed) are
    /// appended. The person's own line, just written from the window, is
    /// selected, and so is a line that came while the last one was
    /// selected, so the window follows the talk; a selection further up
    /// stays where the person is reading.
    pub(crate) fn follow_thread(&mut self) {
        let Some(th) = &mut self.thread else {
            return;
        };
        let Some(&first) = th.entries.first() else {
            return;
        };
        let Some(pair) = ui::words::said(&self.world.journal[first].event).map(|s| s.pair()) else {
            return;
        };
        let from = th.entries.last().map_or(0, |&l| l + 1);
        let at_end = th.sel + 1 == th.entries.len();
        let mut written = false;
        for (i, e) in self.world.journal.iter().enumerate().skip(from) {
            if ui::words::said(&e.event).is_some_and(|s| s.pair() == pair) {
                th.entries.push(i);
                written |= matches!(e.event, Event::YouSaid { .. });
            }
        }
        if (written || at_end) && th.sel + 1 < th.entries.len() {
            th.sel = th.entries.len() - 1;
            th.follow.set(true);
        }
    }

    fn key(&mut self, k: KeyEvent) {
        let away = self.last_key.elapsed() >= AWAY;
        let since = self.last_key_tick;
        self.last_key = Instant::now();
        self.last_key_tick = self.world.tick;

        if k.modifiers.contains(KeyModifiers::CONTROL) && k.code == KeyCode::Char('c') {
            self.quit = true;
            return;
        }
        // An overlay takes the key that closes it.
        if self.digest.take().is_some()
            || self.left_off.take().is_some()
            || std::mem::take(&mut self.help)
        {
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
            // `q` asks; `qq` quits (ctrl-c still quits at once).
            KeyCode::Char('q') => {
                self.pending = Some('q');
                self.flash(self.lang.text().quit_ask);
            }
            // `i` as in vim: start writing.
            KeyCode::Char('/') | KeyCode::Char('i') => self.typing = true,
            KeyCode::Char(':') => {
                self.typing = true;
                self.input = ":".into();
                self.menu = self.suggest();
            }
            KeyCode::Char('!') => {
                self.typing = true;
                self.input = "!".into();
                self.menu = None;
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
                self.menu = self.suggest();
            }
            KeyCode::Char('1') => self.show(View::Salon),
            KeyCode::Char('2') => self.show(View::Board),
            KeyCode::Char('3') => self.show(View::Code),
            KeyCode::Char('T') => {
                let name = theme::next().name.clone();
                self.flash(fill(self.lang.text().theme_is, &[&name]));
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
            KeyCode::Char('w') if self.pane_keys() => self.pane = Pane::Inbox,
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
        if first == 'q' {
            // The question is answered either way.
            self.flash = None;
        }
        match (first, k.code) {
            ('q', KeyCode::Char('q')) => self.quit = true,
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

    /// Where a command of the person's own runs: the checkout, or where
    /// the demo was started.
    pub fn cwd(&self) -> PathBuf {
        match &self.engine {
            Engine::Live(l) => l.repo_of(self.current_project()).to_path_buf(),
            Engine::Sim(_) => PathBuf::from("."),
        }
    }

    /// The line under a `!` command's output, before the screen comes
    /// back.
    pub fn shell_back(&self, ended: Ended) -> String {
        ui::words::shell_back(self.lang, ended)
    }

    /// A `!` command ran (or could not be started): the journal keeps it
    /// under the project at hand, and the footer says how it went.
    pub fn ran(&mut self, command: &str, how: io::Result<Ended>) {
        match how {
            Ok(ended) => {
                let leader = self.world.projects[self.current_project()].leader;
                self.world.note(Event::Shell {
                    leader,
                    command: command.to_string(),
                    ended,
                });
                self.flash(ui::words::shell(self.lang, command, ended));
            }
            Err(e) => {
                let t = self.lang.text();
                self.flash(fill(t.shell_failed, &[command, &e.to_string()]));
            }
        }
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

    /// A key while writing. The menu of completions works as a shell's:
    /// it shows what matches as the text changes, `tab` puts the first
    /// match in and then steps on, `⇧tab` steps back, the arrows (and
    /// ctrl-n / ctrl-p) step too, `enter` takes the highlighted match
    /// when the word is not in yet, else sends, and `esc` hides the menu
    /// before it cancels the writing.
    fn type_key(&mut self, k: KeyEvent) {
        if k.modifiers.contains(KeyModifiers::CONTROL) {
            match k.code {
                KeyCode::Char('n') => self.pick(true),
                KeyCode::Char('p') => self.pick(false),
                _ => {}
            }
            return;
        }
        match k.code {
            KeyCode::Enter => match &mut self.menu {
                Some(m) if !m.inserted => m.insert(&mut self.input),
                _ => self.submit(),
            },
            KeyCode::Esc if self.menu.is_some() => self.menu = None,
            KeyCode::Esc => {
                self.typing = false;
                self.input.clear();
            }
            KeyCode::Backspace => {
                self.input.pop();
                self.menu = self.suggest();
            }
            KeyCode::Tab => match &mut self.menu {
                None => {
                    self.menu = self.suggest();
                    if let Some(m) = &mut self.menu {
                        m.insert(&mut self.input);
                    }
                }
                Some(m) if !m.inserted => m.insert(&mut self.input),
                Some(_) => self.pick(true),
            },
            KeyCode::BackTab | KeyCode::Up => self.pick(false),
            KeyCode::Down => self.pick(true),
            KeyCode::Char(c) => {
                self.input.push(c);
                self.menu = self.suggest();
            }
            _ => {}
        }
    }

    /// Highlights the next (or previous) match and puts it in.
    fn pick(&mut self, forward: bool) {
        if let Some(m) = &mut self.menu {
            m.step(forward);
            m.insert(&mut self.input);
        }
    }

    /// What the composer could complete the input with, now.
    fn suggest(&self) -> Option<Menu> {
        complete::suggest(
            &self.input,
            &self.world,
            self.current_project(),
            self.home.as_deref(),
        )
    }

    /// Whether what is being typed is a command to imece (`:budget 80`),
    /// not a message to an agent.
    pub fn typing_command(&self) -> bool {
        self.input.starts_with(':')
    }

    /// Whether what is being typed is a command for the person's own shell
    /// (`!cargo test`), not a message to an agent.
    pub fn typing_shell(&self) -> bool {
        self.input.starts_with('!')
    }

    /// Who a message being typed goes to: the named agent, or the leader of
    /// the current project.
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
        self.menu = None;
        if text.is_empty() {
            return;
        }
        let t = self.lang.text();
        if let Some(line) = text.strip_prefix(':') {
            return self.command(line.trim());
        }
        if let Some(command) = text.strip_prefix('!') {
            let command = command.trim();
            if command.is_empty() {
                return self.flash(t.shell_empty);
            }
            self.shell = Some(command.to_string());
            return;
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
    /// start on without asking; `lead ask` forgets it. `theme NAME` (or
    /// `t`) switches to a built-in theme or a colour file.
    fn command(&mut self, line: &str) {
        let t = self.lang.text();
        let (name, arg) = line
            .split_once(char::is_whitespace)
            .map_or((line, ""), |(n, a)| (n, a.trim()));
        match name {
            "budget" | "b" => match parse_dollars(arg) {
                Some(to) => {
                    let from = self.world.budget;
                    self.world.budget = to;
                    // A new pot is worth a new warning.
                    self.world.spending.warned = false;
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
            "theme" | "t" => match theme::choose(arg) {
                Ok(theme) => self.flash(fill(t.theme_is, &[&theme.name])),
                Err(why) => self.flash(fill(t.bad_theme, &[arg, &why])),
            },
            "open" | "o" => self.open_project(arg),
            "close" | "c" => self.close_project(arg),
            "rename" => self.rename_project(arg),
            "forget" | "f" => self.forget_agent(arg),
            _ => self.flash(fill(t.no_command, &[name])),
        }
    }

    /// `:open PATH [NAME]`: the checkout at PATH becomes a project of its
    /// own, with its own leader, which is selected.
    fn open_project(&mut self, arg: &str) {
        let t = self.lang.text();
        let (path, name) = arg
            .split_once(char::is_whitespace)
            .map_or((arg, None), |(p, n)| (p, Some(n.trim())));
        if path.is_empty() {
            return self.flash(t.open_usage);
        }
        let full = expand_tilde(path, self.home.as_deref());
        let opened = match &mut self.engine {
            Engine::Live(live) => live.open_project(&mut self.world, &full, name),
            Engine::Sim(sim) => sim.open_project(&mut self.world, path, name),
        };
        match opened {
            Ok(p) => {
                if p == self.collapsed.len() {
                    self.collapsed.push(false);
                } else {
                    self.collapsed[p] = false;
                }
                let project = &self.world.projects[p];
                let leader = project.leader;
                let said = fill(
                    t.opened,
                    &[
                        &project.name,
                        &full.display().to_string(),
                        &self.world.agents[leader].name,
                    ],
                );
                self.zoom = Zoom::None;
                self.selected = Sel::Agent(leader);
                self.flash(said);
            }
            Err(ProjectRefusal::NameTaken(n)) => self.flash(fill(t.project_exists, &[&n])),
            Err(ProjectRefusal::PathOpen(n)) => self.flash(fill(t.project_open, &[path, &n])),
            Err(ProjectRefusal::BadName(n)) => self.flash(fill(t.bad_project_name, &[&n])),
            Err(e) => self.flash(fill(t.bad_open, &[&e.describe()])),
        }
    }

    /// `:close [NAME]`: the named project (else the one at hand) closes:
    /// its agents leave, their worktrees stay. The last one stays open.
    fn close_project(&mut self, arg: &str) {
        let t = self.lang.text();
        let p = if arg.is_empty() {
            self.current_project()
        } else {
            match self.world.project_by_name(arg) {
                Some(p) => p,
                None => return self.flash(fill(t.no_project, &[arg])),
            }
        };
        let name = self.world.projects[p].name.clone();
        let closed = match &mut self.engine {
            Engine::Live(live) => live.close_project(&mut self.world, p),
            Engine::Sim(sim) => sim.close_project(&mut self.world, p),
        };
        if closed.is_err() {
            return self.flash(fill(t.last_project, &[&name]));
        }
        if let Zoom::Focus(a) | Zoom::Trace(a) | Zoom::Diff(a) = self.zoom
            && self.world.agents[a].project == p
        {
            self.zoom = Zoom::None;
        }
        let was_in = match self.selected {
            Sel::Project(q) => q == p,
            Sel::Agent(a) => self.world.agents[a].project == p,
        };
        if was_in && let Some(home) = self.world.open_projects().next() {
            self.selected = Sel::Agent(home.leader);
        }
        let open = self.world.open_inbox().len();
        self.inbox_sel = self.inbox_sel.min(open.saturating_sub(1));
        self.flash(fill(t.closed, &[&name]));
    }

    /// `:forget [@NAME]`: the agent (named, else the one at hand) forgets
    /// its session; its next message starts a new one, told where it left
    /// off. Live only: the demo's agents have no sessions.
    fn forget_agent(&mut self, arg: &str) {
        let t = self.lang.text();
        let word = arg.strip_prefix('@').unwrap_or(arg).trim();
        let id = if word.is_empty() {
            self.agent_at_hand()
        } else {
            self.world.agent_by_name(word)
        };
        let Some(id) = id else {
            return self.flash(fill(t.no_agent, &[word]));
        };
        let name = self.world.agents[id].name.clone();
        let Engine::Live(live) = &mut self.engine else {
            return self.flash(t.forget_demo);
        };
        match live.forget_agent(&mut self.world, id) {
            Ok(()) => self.flash(fill(t.forgot, &[&name])),
            Err(Refusal::Command(why)) => self.flash(fill(t.forget_refused, &[&name, &why])),
            Err(why) => self.flash(fill(t.forget_refused, &[&name, &format!("{why:?}")])),
        }
    }

    /// `:rename NAME`: the project at hand is called NAME from now on, and
    /// its leader `lead-NAME`.
    fn rename_project(&mut self, arg: &str) {
        let t = self.lang.text();
        let p = self.current_project();
        let was = self.world.projects[p].name.clone();
        let renamed = match &mut self.engine {
            Engine::Live(live) => live.rename_project(&mut self.world, p, arg),
            Engine::Sim(sim) => sim.rename_project(&mut self.world, p, arg),
        };
        match renamed {
            Ok(()) => {
                let leader = &self.world.agents[self.world.projects[p].leader].name;
                let said = fill(t.renamed, &[&was, arg, leader]);
                self.flash(said);
            }
            Err(ProjectRefusal::NameTaken(n)) => self.flash(fill(t.project_exists, &[&n])),
            Err(_) => self.flash(fill(t.bad_project_name, &[arg])),
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
                        Some(item) if item.kind == InboxKind::Question => {
                            // The conversation shows the whole question; it
                            // waits until the person writes back.
                            let agent = item.agent;
                            if let Some(i) = self.world.journal.iter().rposition(|e| {
                                matches!(e.event, Event::ToYou { from, kind: MsgKind::Ask, .. } if from == agent)
                            }) {
                                self.open_thread(i);
                                return;
                            }
                            Some(Zoom::Focus(agent))
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
        if self.world.inbox[item].kind == InboxKind::Approval {
            // Only a live session asks; the demo has none.
            if let Engine::Live(l) = &mut self.engine {
                let name = self.world.agents[self.world.inbox[item].agent].name.clone();
                l.decide(&mut self.world, item, approve);
                let how = if approve { t.allowed } else { t.refused };
                self.flash(fill(how, &[&name]));
            }
            self.inbox_sel = self.inbox_sel.min(open.len().saturating_sub(2));
            return;
        }
        if self.world.inbox[item].kind == InboxKind::Question {
            // A question is answered by writing back: the composer opens
            // to the agent, and the item closes when the answer goes.
            let name = self.world.agents[self.world.inbox[item].agent].name.clone();
            self.typing = true;
            self.input = format!("@{name} ");
            self.menu = None;
            return;
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

    /// The person has worker `id`'s change applied to their checkout (git
    /// does it on a thread of its own; the journal says how it went), or
    /// rejects it; a rejection opens the composer to that worker, so they
    /// say why. The change's view closes either way.
    fn decide_change(&mut self, id: AgentId, apply: bool) {
        let t = self.lang.text();
        let name = self.world.agents[id].name.clone();
        let open = self.world.open_inbox().len();
        match &mut self.engine {
            Engine::Sim(_) => self.flash(t.demo_no_change),
            Engine::Live(l) if apply => match l.apply_change(&self.world, id) {
                Ok(()) => self.flash(fill(t.applying, &[&name])),
                Err(why) => self.flash(fill(t.apply_failed, &[&name, &why])),
            },
            Engine::Live(l) => match l.reject_change(&mut self.world, id) {
                Ok(()) => {
                    self.flash(fill(t.rejected_change, &[&name]));
                    self.typing = true;
                    self.input = format!("@{name} ");
                }
                Err(why) => self.flash(fill(t.reject_failed, &[&name, &why])),
            },
        }
        if self.zoom == Zoom::Diff(id) {
            self.zoom = Zoom::None;
            self.pane = Pane::Inbox;
        }
        self.inbox_sel = self.inbox_sel.min(open.saturating_sub(2));
    }
}

/// `path` with a leading `~` or `~/` replaced by `home`; anything else as
/// it is, and so is `~` when there is no home.
pub fn expand_tilde(path: &str, home: Option<&Path>) -> PathBuf {
    match (path, home) {
        ("~", Some(home)) => home.to_path_buf(),
        (p, Some(home)) if p.starts_with("~/") => home.join(&p[2..]),
        _ => PathBuf::from(path),
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
            runs: Vec::new(),
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

    #[test]
    fn a_and_r_on_a_live_change_say_what_became_of_it() {
        use crate::live::testing::{as_if_applying, hold_change, idle_live, team};
        let t = Lang::En.text();
        let app_with = |live: crate::live::Live| {
            let mut w = team(1);
            w.change_ready(Change {
                agent: 1,
                title: "did it".into(),
                files: Vec::new(),
                patch: String::new(),
                runs: Vec::new(),
            });
            let mut app = App::with(w, Engine::Live(Box::new(live)), Lang::En);
            app.zoom = Zoom::Diff(1);
            app.pane = Pane::Inbox;
            app
        };
        let flash = |app: &App| app.flash.as_ref().map(|(m, _)| m.clone());
        // Nothing imece holds: refused, with why.
        let mut app = app_with(idle_live(2));
        app.on_key(key(KeyCode::Char('a')));
        assert_eq!(
            flash(&app),
            Some(fill(
                t.apply_failed,
                &["claude-1", "no change of theirs waits"]
            ))
        );
        // A change it holds: git applies it, the journal will say.
        let mut live = idle_live(2);
        hold_change(&mut live, 1);
        let mut app = app_with(live);
        app.on_key(key(KeyCode::Char('a')));
        assert_eq!(flash(&app), Some(fill(t.applying, &["claude-1"])));
        // Rejected while being applied: refused, the change still waits,
        // no composer.
        let mut live = idle_live(2);
        as_if_applying(&mut live, 1);
        let mut app = app_with(live);
        app.on_key(key(KeyCode::Char('r')));
        assert_eq!(
            flash(&app),
            Some(fill(
                t.reject_failed,
                &["claude-1", "their change is being applied"]
            ))
        );
        assert!(app.world.change_of(1).is_some() && !app.typing);
        // Rejected: the composer opens to the worker for the reasons.
        let mut app = app_with(idle_live(2));
        app.on_key(key(KeyCode::Char('r')));
        assert_eq!(flash(&app), Some(fill(t.rejected_change, &["claude-1"])));
        assert!(app.world.change_of(1).is_none());
        assert!(app.typing);
        assert_eq!(app.input, "@claude-1 ");
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

    #[test]
    fn the_demo_opens_closes_and_renames_projects_too() {
        let mut app = App::new(1, 1, 0, 1, Lang::En);
        app.home = None;
        let said = |app: &mut App| app.flash.take().map(|f| f.0).unwrap_or_default();
        app.command("open ~/work/shop");
        assert_eq!(app.world.projects.len(), 2);
        let leader = app.world.projects[1].leader;
        assert_eq!(app.world.agents[leader].name, "lead-shop");
        assert_eq!(app.selected, Sel::Agent(leader));
        assert_eq!(app.collapsed, [false, false]);
        assert_eq!(
            said(&mut app),
            "opened shop at ~/work/shop; write to lead-shop"
        );
        assert_eq!(
            app.cwd(),
            PathBuf::from("."),
            "the demo runs where it started"
        );
        app.command("rename store");
        assert_eq!(app.world.projects[1].name, "store");
        assert_eq!(said(&mut app), "shop is now store; write to lead-store");
        app.command("close store");
        assert!(app.world.projects[1].closed);
        assert_eq!(app.selected, Sel::Agent(0));
        assert_eq!(
            said(&mut app),
            "closed store: its agents left; their worktrees stay on disk"
        );
        app.command("close");
        assert_eq!(
            said(&mut app),
            "shop-api is the last open project; q q quits"
        );
        assert!(!app.world.projects[0].closed);
        // :forget names an agent, @ or not, else takes the one at hand;
        // the demo's agents have no sessions.
        app.command("forget nobody");
        assert_eq!(said(&mut app), "no agent named \"nobody\"");
        app.command("forget @nobody");
        assert_eq!(said(&mut app), "no agent named \"nobody\"");
        let lead = app.world.agents[0].name.clone();
        app.command(&format!("forget @{lead}"));
        assert_eq!(
            said(&mut app),
            "the demo's agents have no sessions to forget"
        );
        app.command(&format!("f {lead}"));
        assert_eq!(
            said(&mut app),
            "the demo's agents have no sessions to forget"
        );
        app.selected = Sel::Agent(0);
        app.command("forget");
        assert_eq!(
            said(&mut app),
            "the demo's agents have no sessions to forget"
        );
    }

    #[test]
    fn the_project_at_hand_falls_back_to_an_open_one_when_its_project_closed() {
        let mut app = App::new(2, 2, 0, 1, Lang::En);
        app.selected = Sel::Project(1);
        assert_eq!(app.current_project(), 1);
        app.world.projects[1].closed = true;
        assert_eq!(app.current_project(), 0);
        app.selected = Sel::Agent(app.world.projects[1].leader);
        assert_eq!(app.current_project(), 0);
        app.world.projects[1].closed = false;
        app.world.projects[0].closed = true;
        app.selected = Sel::Project(0);
        assert_eq!(app.current_project(), 1, "the first open one");
        app.selected = Sel::Project(1);
        assert_eq!(app.current_project(), 1);
    }

    #[test]
    fn cwd_is_the_checkout_of_the_project_at_hand() {
        let one = crate::live::testing::checkout("cwd-one");
        let two = crate::live::testing::checkout("cwd-two");
        let w = crate::live::testing::team(0);
        let live = crate::live::testing::idle_live_in(&one, 1);
        let mut app = App::with(w, Engine::Live(Box::new(live)), Lang::En);
        assert_eq!(app.cwd(), one);
        app.command(&format!("open {} two", two.display()));
        assert_eq!(app.selected, Sel::Agent(1));
        assert_eq!(app.cwd(), imece_agent::worktree::top(&two).unwrap());
        app.selected = Sel::Agent(0);
        assert_eq!(app.cwd(), one);
        let _ = std::fs::remove_dir_all(&one);
        let _ = std::fs::remove_dir_all(&two);
    }

    #[test]
    fn a_live_app_starts_on_the_leader_of_the_checkout_at_hand() {
        let one = crate::live::testing::checkout("home-one");
        let two = crate::live::testing::checkout("home-two");
        let mut w = crate::live::testing::team(0);
        let mut live = crate::live::testing::idle_live_in(&one, 1);
        live.open_project(&mut w, &two, Some("two")).unwrap();
        live.set_home(1);
        let app = App::with(w, Engine::Live(Box::new(live)), Lang::En);
        assert_eq!(app.selected, Sel::Agent(1), "the second project's leader");
        let _ = std::fs::remove_dir_all(&one);
        let _ = std::fs::remove_dir_all(&two);
    }

    #[test]
    fn a_store_that_fails_is_said_once_and_a_save_that_works_clears_it() {
        let w = crate::live::testing::team(1);
        let mut live = crate::live::testing::idle_live(2);
        live.store_in_memory();
        let mut app = App::with(w, Engine::Live(Box::new(live)), Lang::En);
        app.persist();
        assert!(app.flash.is_none(), "a save that works says nothing");
        assert!(app.store_error.is_none());
        let Engine::Live(l) = &mut app.engine else {
            unreachable!()
        };
        l.break_store();
        app.world.agents[1].cost = 5;
        app.persist();
        let said = app.flash.take().map(|f| f.0).unwrap_or_default();
        assert!(
            said.starts_with("not saved for the next run: ") && said.contains("no such table"),
            "{said}"
        );
        assert!(app.store_error.is_some());
        app.world.agents[1].cost = 6;
        app.on_key(key(KeyCode::Char('j')));
        assert!(app.flash.is_none(), "the same failure is not said again");
        app.step();
        assert!(app.flash.is_none());
        // A store that works again clears it. Every key and every step
        // saves: after either, nothing is left to write.
        let mut live = crate::live::testing::idle_live(2);
        live.store_in_memory();
        let mut app = App::with(
            crate::live::testing::team(1),
            Engine::Live(Box::new(live)),
            Lang::En,
        );
        app.store_error = Some("was broken".into());
        app.world.agents[1].cost = 7;
        app.on_key(key(KeyCode::Char('j')));
        assert!(app.store_error.is_none());
        let Engine::Live(l) = &mut app.engine else {
            unreachable!()
        };
        assert_eq!(l.persist(&app.world), Ok(false), "the key saved it");
        app.world.agents[1].cost = 8;
        app.step();
        let Engine::Live(l) = &mut app.engine else {
            unreachable!()
        };
        assert_eq!(l.persist(&app.world), Ok(false), "the step saved it");
        assert_eq!(
            app.world.spending.seen.back().map(|s| s.1),
            Some(app.world.spent()),
            "the step noted what was spent, for the rate"
        );
        // The demo has no store: nothing to say, ever.
        let mut demo = App::new(1, 1, 0, 1, Lang::En);
        demo.persist();
        assert!(demo.flash.is_none() && demo.store_error.is_none());
    }

    #[test]
    fn a_tilde_stands_for_home_in_a_typed_path() {
        let home = Some(Path::new("/home/me"));
        assert_eq!(expand_tilde("~", home), PathBuf::from("/home/me"));
        assert_eq!(expand_tilde("~/x/y", home), PathBuf::from("/home/me/x/y"));
        assert_eq!(expand_tilde("~x", home), PathBuf::from("~x"));
        assert_eq!(expand_tilde("/a/~", home), PathBuf::from("/a/~"));
        assert_eq!(expand_tilde("~", None), PathBuf::from("~"));
        assert_eq!(expand_tilde("~/x", None), PathBuf::from("~/x"));
    }

    fn item(agent: AgentId, open: bool) -> InboxItem {
        InboxItem {
            kind: InboxKind::Reply,
            agent,
            detail: "an answer".into(),
            researcher: None,
            open,
            tick: 0,
        }
    }

    #[test]
    fn where_you_left_off_is_what_the_earlier_runs_left() {
        let mut app = App::new(2, 4, 0, 1, Lang::En);
        let w = &mut app.world;
        w.budget = 500_000;
        for (i, a) in w.agents.iter_mut().enumerate() {
            a.cost = 100 * (i as u64 + 1);
        }
        let spent = w.spent();
        // Eight lines of before, then this start's: two turns cut short
        // and a forgotten session, which is no interruption.
        w.journal.clear();
        for tick in 0..8 {
            w.journal.push(Entry {
                tick,
                event: Event::Failed { agent: 0 },
            });
        }
        let before = w.journal.len();
        w.note(Event::Interrupted { agent: 2 });
        w.note(Event::Forgotten { agent: 1 });
        w.note(Event::Interrupted { agent: 1 });
        // Seven items wait, a closed one does not count.
        w.inbox.clear();
        w.inbox.push(item(0, false));
        for i in 0..7 {
            w.inbox.push(item(i % 3, true));
        }
        // Project 1 is closed; of project 0, one agent is gone and one card
        // done.
        w.projects[1].closed = true;
        let of_zero: Vec<AgentId> = w
            .agents
            .iter()
            .filter(|a| a.project == 0)
            .map(|a| a.id)
            .collect();
        w.agents[*of_zero.last().unwrap()].gone = true;
        let cards_of_zero = w.cards.iter().filter(|c| c.project == 0).count();
        assert!(cards_of_zero >= 2, "the demo has cards");
        let done = w.cards.iter().position(|c| c.project == 0).unwrap();
        w.cards[done].column = Column::Done;
        let name = w.projects[0].name.clone();

        let l = LeftOff::of(w, before, Some(321)).expect("something to tell");
        assert_eq!((l.ago, l.spent, l.budget), (Some(321), spent, 500_000));
        assert_eq!(
            l.projects,
            [(name, of_zero.len() - 1, cards_of_zero - 1)],
            "open projects only, agents that are still there, cards not done"
        );
        assert_eq!(l.waiting, [1, 2, 3, 4, 5, 6], "the oldest six open items");
        assert_eq!(l.waiting_total, 7);
        assert_eq!(l.interrupted, [2, 1], "in the order they were found");
        assert_eq!(
            l.last,
            w.journal[2..8].to_vec(),
            "the six before this start"
        );

        // More lines of before than the journal has: all of it is before.
        let l = LeftOff::of(w, 100, None).unwrap();
        assert_eq!(l.ago, None);
        assert!(l.interrupted.is_empty());
        assert_eq!(l.last, w.journal[5..11].to_vec());
        // Nothing from before and nothing waiting: nothing to tell; either
        // alone is.
        for i in &mut w.inbox {
            i.open = false;
        }
        assert!(LeftOff::of(w, 0, Some(5)).is_none());
        assert!(LeftOff::of(w, 1, Some(5)).is_some(), "a line from before");
        w.inbox[0].open = true;
        let l = LeftOff::of(w, 0, Some(5)).unwrap();
        assert_eq!((l.waiting.as_slice(), l.waiting_total), (&[0][..], 1));
        assert!(l.last.is_empty());
    }

    #[test]
    fn a_live_app_starts_with_where_you_left_off_and_any_key_closes_it() {
        let mut w = crate::live::testing::team(1);
        w.note(Event::Failed { agent: 1 });
        let mut live = crate::live::testing::idle_live(2);
        live.set_left_off(crate::live::LeftBehind {
            journal: 1,
            ago: Some(90),
        });
        let mut app = App::with(w, Engine::Live(Box::new(live)), Lang::En);
        let l = app.left_off.as_ref().expect("told at the start");
        assert_eq!((l.ago, l.last.len()), (Some(90), 1));
        app.on_key(key(KeyCode::Char('?')));
        assert!(app.left_off.is_none(), "the key closes it");
        assert!(!app.help, "and does nothing else");
        app.on_key(key(KeyCode::Char('?')));
        assert!(app.help);
        // A live team with nothing remembered, and the demo, start without it.
        let w = crate::live::testing::team(1);
        let live = crate::live::testing::idle_live(2);
        let app = App::with(w, Engine::Live(Box::new(live)), Lang::En);
        assert!(app.left_off.is_none());
        assert!(App::new(1, 1, 0, 1, Lang::En).left_off.is_none());
    }

    /// Leaving tells the store when, so the next start can say how long
    /// ago; the demo has no store to tell.
    #[test]
    fn leaving_marks_the_store_with_the_time() {
        let w = crate::live::testing::team(1);
        let mut live = crate::live::testing::idle_live(2);
        live.store_in_memory();
        let mut app = App::with(w, Engine::Live(Box::new(live)), Lang::En);
        let Engine::Live(l) = &app.engine else {
            unreachable!()
        };
        assert_eq!(l.saved_at(), None, "nothing written yet");
        app.leave();
        let Engine::Live(l) = &app.engine else {
            unreachable!()
        };
        let at = l.saved_at().expect("written at the leave");
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs();
        assert!(at.abs_diff(now) <= 2);
        App::new(1, 1, 0, 1, Lang::En).leave();
    }

    /// A demo app with a conversation between the person and worker 1
    /// open on its only line.
    fn with_thread() -> (App, AgentId) {
        let mut app = App::new(1, 2, 0, 1, Lang::En);
        let worker = app
            .world
            .agents
            .iter()
            .find(|a| a.role == Role::Worker)
            .map(|a| a.id)
            .unwrap();
        app.world.said(worker, "first");
        let i = app.world.journal.len() - 1;
        app.open_thread(i);
        assert_eq!(app.thread.as_ref().unwrap().entries, [i]);
        (app, worker)
    }

    #[test]
    fn a_message_written_from_the_conversation_window_lands_in_it_selected() {
        let (mut app, worker) = with_thread();
        let name = app.world.agents[worker].name.clone();
        app.thread.as_mut().unwrap().follow.set(false);
        app.typing = true;
        app.input = format!("@{name} second");
        app.on_key(key(KeyCode::Enter));
        let i = app.world.journal.len() - 1;
        assert!(matches!(app.world.journal[i].event, Event::YouSaid { .. }));
        let th = app.thread.as_ref().unwrap();
        assert_eq!(th.entries.len(), 2);
        assert_eq!(th.entries[1], i);
        assert_eq!(th.sel, 1, "the line just written is selected");
        assert!(th.follow.get());
    }

    #[test]
    fn the_persons_own_line_is_selected_even_when_reading_further_up() {
        let (mut app, worker) = with_thread();
        let name = app.world.agents[worker].name.clone();
        app.world.said(worker, "second");
        app.follow_thread();
        let th = app.thread.as_mut().unwrap();
        th.sel = 0;
        th.follow.set(false);
        app.typing = true;
        app.input = format!("@{name} third");
        app.on_key(key(KeyCode::Enter));
        let th = app.thread.as_ref().unwrap();
        assert_eq!(th.entries.len(), 3);
        assert_eq!(th.sel, 2, "what the person wrote is selected");
        assert!(th.follow.get());
    }

    #[test]
    fn nothing_new_moves_nothing() {
        let (mut app, _) = with_thread();
        app.thread.as_mut().unwrap().follow.set(false);
        app.follow_thread();
        let th = app.thread.as_ref().unwrap();
        assert_eq!((th.entries.len(), th.sel), (1, 0));
        assert!(!th.follow.get(), "at the end with nothing new: no scroll");
    }

    #[test]
    fn a_reply_that_comes_while_the_last_line_is_selected_is_followed() {
        let (mut app, worker) = with_thread();
        app.thread.as_mut().unwrap().follow.set(false);
        app.world.note(Event::Replied {
            agent: worker,
            text: "yes".into(),
        });
        let i = app.world.journal.len() - 1;
        app.step();
        let th = app.thread.as_ref().unwrap();
        assert!(th.entries.contains(&i), "{:?}", th.entries);
        assert_eq!(th.entries[0], th.entries[0].min(i));
        assert_eq!(th.sel + 1, th.entries.len(), "still at the end");
        assert!(th.follow.get());
    }

    #[test]
    fn a_reply_that_comes_while_the_person_reads_further_up_leaves_the_selection() {
        let (mut app, worker) = with_thread();
        app.world.said(worker, "second");
        app.follow_thread();
        let th = app.thread.as_mut().unwrap();
        assert_eq!(th.entries.len(), 2);
        th.sel = 0;
        th.follow.set(false);
        app.world.note(Event::Replied {
            agent: worker,
            text: "yes".into(),
        });
        let i = app.world.journal.len() - 1;
        app.follow_thread();
        let th = app.thread.as_ref().unwrap();
        assert_eq!(th.entries.len(), 3);
        assert_eq!(th.entries[2], i);
        assert_eq!(th.sel, 0, "reading up: the selection stays");
        assert!(!th.follow.get());
        // Followed once: a second pass adds nothing.
        app.follow_thread();
        assert_eq!(app.thread.as_ref().unwrap().entries.len(), 3);
    }

    #[test]
    fn a_line_of_another_conversation_stays_out_of_the_open_one() {
        let (mut app, worker) = with_thread();
        let other = (0..app.world.agents.len())
            .find(|&id| id != worker && app.world.agents[id].role == Role::Worker)
            .unwrap();
        app.world.said(other, "elsewhere");
        app.follow_thread();
        assert_eq!(app.thread.as_ref().unwrap().entries.len(), 1);
        // A thread opened on a line that is not a conversation follows nothing.
        app.world.note(Event::Forgotten { agent: worker });
        let i = app.world.journal.len() - 1;
        app.open_thread(i);
        app.world.said(worker, "third");
        app.follow_thread();
        assert_eq!(app.thread.as_ref().unwrap().entries, [i]);
        app.thread = None;
        app.follow_thread();
        assert!(app.thread.is_none());
    }
}
