//! Screen state and keys: which view, how deep, what is selected, which
//! projects are folded, the language, and what the person is typing.

use std::cell::{Cell, RefCell};
use std::collections::BTreeSet;
use std::io;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use ratatui::crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

use crate::complete::{self, Menu};
use crate::edit;
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

/// How much an agent needs the person, least first: the salon puts the
/// workers that need more first and `f` shows only those that need
/// something.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum Need {
    Nothing,
    /// Finished a turn the person has not looked at.
    Done,
    /// Blocked, or its session stopped.
    Stuck,
    /// Something of it waits in WAITING FOR YOU.
    Waiting,
}

/// What happened while the person was away.
#[derive(Default)]
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
#[derive(Clone, Debug, Default, PartialEq, Eq)]
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
    /// The salon shows only the agents that need the person (`f`). Kept
    /// for this run only.
    pub needs_only: bool,
    /// Agents that finished a turn since the person last opened them.
    pub unseen: BTreeSet<AgentId>,
    /// The agents that were mid-turn at the last step.
    turning: BTreeSet<AgentId>,
    /// Position in the open inbox items.
    pub inbox_sel: usize,
    /// Inbox items the person put off with `S`, in the order they were put
    /// off: `W` comes to them after everything else. Kept for this run
    /// only.
    pub deferred: Vec<usize>,
    /// The item `W` or `S` last opened, so the next `W` goes on after it
    /// even when opening it closed it (a reply is read when opened).
    pub waiting_at: Option<usize>,
    /// The first patch line the open change shows.
    pub diff_scroll: usize,
    /// Rows the last drawing of a change had, for paging.
    pub diff_page: Cell<u16>,
    /// The selected journal line (a journal index).
    pub journal_sel: Option<usize>,
    pub thread: Option<Thread>,
    /// The card whose screen takes the salon's place (`#323`): its story
    /// and who had a part in it, what waits for the person on it and its
    /// journal lines.
    pub card: Option<CardId>,
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
    /// Where the cursor is in `input`, as the bytes after it: 0 is the
    /// end, so text put in from elsewhere starts with the cursor there.
    pub after: usize,
    /// What the last ctrl-w / ctrl-u / ctrl-k (or alt-backspace, alt-d)
    /// cut, for ctrl-y.
    pub cut: String,
    /// What the composer offers for the word being typed, if anything.
    pub menu: Option<Menu>,
    /// Whether the terminal tells shift-enter from enter (it speaks the
    /// kitty keyboard protocol): `?` and the footer name shift-enter for
    /// a new line, else `\` enter.
    pub shift_enter: bool,
    pub paused: bool,
    pub speed: u32,
    pub help: bool,
    /// What the help is filtered by (`/` in it), empty each time the help
    /// opens, and whether the person is still typing it (the help closes
    /// only after `enter` or `esc` ends that).
    pub help_find: String,
    pub help_typing: bool,
    /// The vendor that leads from the next start on without being asked
    /// (`claude`), as the preferences keep it; none asks when there is a
    /// choice.
    pub primary: Option<String>,
    pub digest: Option<Digest>,
    /// What an earlier run left behind, shown once at the start.
    pub left_off: Option<LeftOff>,
    /// What quitting now would leave behind, open over the screen (`q s`).
    pub leaving: bool,
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
            needs_only: false,
            unseen: BTreeSet::new(),
            turning: BTreeSet::new(),
            inbox_sel: 0,
            deferred: Vec::new(),
            waiting_at: None,
            diff_scroll: 0,
            diff_page: Cell::new(10),
            journal_sel: None,
            thread: None,
            card: None,
            side: SIDE_DEFAULT,
            copy: None,
            shell: None,
            home: std::env::var_os("HOME").map(PathBuf::from),
            pending: None,
            typing: false,
            input: String::new(),
            after: 0,
            shift_enter: false,
            cut: String::new(),
            menu: None,
            paused: false,
            speed: 1,
            help: false,
            help_find: String::new(),
            help_typing: false,
            primary: None,
            digest: None,
            left_off,
            leaving: false,
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
        self.watch_turns();
        self.settle();
        self.follow_thread();
        self.persist();
    }

    /// Marks the agents whose turn ended in this step, idle now, as done
    /// and not yet seen; one that is mid-turn again is not, and neither is
    /// one the person has open.
    fn watch_turns(&mut self) {
        let now: BTreeSet<AgentId> = self.world.mid_turn().into_iter().collect();
        for &id in self.turning.difference(&now) {
            if self.world.agents[id].status == Status::Idle {
                self.unseen.insert(id);
            }
        }
        self.unseen.retain(|id| !now.contains(id));
        self.turning = now;
        self.seen();
    }

    /// The agents the person has open: zoomed in on, or a party of the
    /// open conversation.
    fn opened(&self) -> Vec<AgentId> {
        if let Zoom::Focus(id) | Zoom::Trace(id) | Zoom::Diff(id) = self.zoom {
            return vec![id];
        }
        let Some(&first) = self.thread.as_ref().and_then(|th| th.entries.first()) else {
            return Vec::new();
        };
        let Some((a, b)) = ui::words::said(&self.world.journal[first].event).map(|s| s.pair())
        else {
            return Vec::new();
        };
        [a, b]
            .into_iter()
            .filter_map(|p| match p {
                Party::Agent(id) => Some(id),
                Party::Person => None,
            })
            .collect()
    }

    /// What the person has open is seen.
    fn seen(&mut self) {
        for id in self.opened() {
            self.unseen.remove(&id);
        }
    }

    /// Agent `id` finished a turn the person has not looked at since, and
    /// nothing of it waits for them (an answer, a question, a change: its
    /// item says more).
    pub fn done_unseen(&self, id: AgentId) -> bool {
        let w = &self.world;
        self.unseen.contains(&id)
            && w.agents[id].status == Status::Idle
            && !w.open_inbox().iter().any(|&i| w.inbox[i].agent == id)
    }

    /// What each agent needs of the person, by id. Gone agents need
    /// nothing.
    pub fn needs(&self) -> Vec<Need> {
        let w = &self.world;
        let mut waits = vec![false; w.agents.len()];
        for i in w.open_inbox() {
            waits[w.inbox[i].agent] = true;
        }
        w.agents
            .iter()
            .map(|a| {
                if a.gone {
                    Need::Nothing
                } else if waits[a.id] || a.status == Status::WaitingHuman {
                    Need::Waiting
                } else if a.status.needs_attention() {
                    // Blocked, or its session stopped.
                    Need::Stuck
                } else if self.unseen.contains(&a.id) && a.status == Status::Idle {
                    // `done_unseen`, its inbox already looked at.
                    Need::Done
                } else {
                    Need::Nothing
                }
            })
            .collect()
    }

    /// `f`: the salon shows only what needs the person, or everyone again.
    fn toggle_needs(&mut self) {
        self.needs_only = !self.needs_only;
        let t = self.lang.text();
        self.flash(if self.needs_only {
            t.needs_on
        } else {
            t.needs_off
        });
    }

    /// While the salon shows only what needs the person, a selection whose
    /// row left it (the agent needs nothing any more) moves to the first
    /// row of its project still shown, else to the first row; nothing
    /// shown leaves it where it is. Not while the person writes: who they
    /// write to stays put.
    fn settle(&mut self) {
        if !self.needs_only || self.typing {
            return;
        }
        let shown: Vec<Sel> = ui::salon::rows(self, usize::MAX)
            .iter()
            .filter_map(ui::salon::Row::sel)
            .collect();
        if shown.is_empty() || shown.contains(&self.selected) {
            return;
        }
        let of = |s: &Sel| match *s {
            Sel::Project(p) => p,
            Sel::Agent(a) => self.world.agents[a].project,
        };
        let p = of(&self.selected);
        self.selected = shown
            .iter()
            .find(|s| of(s) == p)
            .copied()
            .unwrap_or(shown[0]);
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
            && let Some(&i) = self.inbox_rows().get(self.inbox_sel)
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
        self.seen();
        self.settle();
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
        // What quitting would leave: `q` quits after all, `W` goes to what
        // waits, anything else stays.
        if std::mem::take(&mut self.leaving) {
            match k.code {
                KeyCode::Char('q') => self.quit = true,
                KeyCode::Char('W') => self.next_waiting(false),
                _ => {}
            }
            return;
        }
        // An overlay takes the key that closes it.
        if self.digest.take().is_some() || self.left_off.take().is_some() {
            return;
        }
        if self.help {
            self.help_key(k);
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
            KeyCode::Char('q') => self.ask_quit(),
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
            KeyCode::Char('#') => {
                self.typing = true;
                self.input = match self.card_at_hand().filter(|&c| Some(c) != self.card) {
                    Some(c) => format!("#{}", c + 1),
                    None => "#".into(),
                };
                self.menu = self.suggest();
            }
            KeyCode::Char('n') | KeyCode::Char('p') if self.on_card() => {
                self.step_card(k.code == KeyCode::Char('n'))
            }
            KeyCode::Char('g') | KeyCode::Char('z') => {
                if let KeyCode::Char(c) = k.code {
                    self.pending = Some(c);
                }
            }
            KeyCode::Char('G') => self.jump(false),
            KeyCode::Char('t') => {
                if let Some(id) = self.agent_at_hand() {
                    self.trace(id);
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
            KeyCode::Char('T') => self.next_theme(),
            KeyCode::Char('L') => self.set_lang(self.lang.other()),
            // Live agents run in real time: nothing to pause or speed up.
            KeyCode::Char(' ') if !self.live() => self.pause(),
            KeyCode::Char('+') | KeyCode::Char('=') if !self.live() => self.faster(),
            KeyCode::Char('-') if !self.live() => self.slower(),
            KeyCode::Char('?') => self.open_help(),
            KeyCode::Tab | KeyCode::Char(']') if self.pane_keys() => self.cycle_pane(true),
            KeyCode::BackTab | KeyCode::Char('[') if self.pane_keys() => self.cycle_pane(false),
            KeyCode::Char('w') if self.pane_keys() => self.pane = Pane::Inbox,
            // From anywhere: what waits for the person, one item after
            // the other; `S` puts this one off first.
            KeyCode::Char('W') => self.next_waiting(false),
            KeyCode::Char('S') => self.next_waiting(true),
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
            KeyCode::Char('f') if self.salon_keys() => self.toggle_needs(),
            KeyCode::Enter => self.deeper(),
            KeyCode::Esc if self.on_card() => self.leave_card(),
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
            ('q', KeyCode::Char('s')) => self.leaving = self.in_flight(),
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

    /// `q` or `:quit`: the quit question; a `q` after it quits.
    fn ask_quit(&mut self) {
        self.pending = Some('q');
        let ask = self.quit_ask();
        self.flash(ask);
    }

    /// `?` or `:help`: the help, all of it.
    fn open_help(&mut self) {
        self.help = true;
        self.help_find.clear();
    }

    /// `T` or `:theme` alone: the next built-in theme.
    fn next_theme(&mut self) {
        let name = theme::next().name.clone();
        self.flash(fill(self.lang.text().theme_is, &[&name]));
    }

    /// `L` or `:lang`: the screen speaks `lang` from now on.
    fn set_lang(&mut self, lang: Lang) {
        self.lang = lang;
        if let Engine::Sim(s) = &mut self.engine {
            s.set_lang(self.lang);
        }
        self.flash(self.lang.text().language);
    }

    /// `t` or `:trace`: the raw stream of agent `id`.
    fn trace(&mut self, id: AgentId) {
        self.zoom = Zoom::Trace(id);
    }

    /// `space` or `:pause` in the demo: the simulation stops or goes on.
    fn pause(&mut self) {
        self.paused = !self.paused;
    }

    /// `+` or `:faster` in the demo: twice the speed, eight times at most.
    fn faster(&mut self) {
        self.speed = (self.speed * 2).min(8);
    }

    /// `-` or `:slower` in the demo: half the speed, never below one.
    fn slower(&mut self) {
        self.speed = (self.speed / 2).max(1);
    }

    /// `:waiting` (`w` in the salon): the salon's waiting panel.
    fn go_waiting(&mut self) {
        self.show(View::Salon);
        self.pane = Pane::Inbox;
    }

    /// The salon's agent panel, for the `:` names of keys that act there
    /// (`:needs`, `:fold`, `:unfold`).
    fn go_salon(&mut self) {
        self.show(View::Salon);
        self.pane = Pane::Salon;
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

    /// Where open inbox item `i` comes in `W`'s order: the items not put
    /// off first, oldest first, then the ones put off, in the order they
    /// were.
    fn waiting_rank(&self, i: usize) -> (usize, usize) {
        match self.deferred.iter().position(|&d| d == i) {
            None => (0, i),
            Some(n) => (1, n),
        }
    }

    /// The inbox item the person has at hand: the one `W` last opened,
    /// open or closed since (opening a reply reads it, an answer closes
    /// it, and the panel's selection slides onto the next row when an item
    /// above it closes), else the one selected in the waiting panel.
    /// Moving the selection by hand forgets what `W` opened.
    fn waiting_at_hand(&self) -> Option<usize> {
        let selected = || match self.pane {
            Pane::Inbox => self.inbox_rows().get(self.inbox_sel).copied(),
            _ => None,
        };
        self.waiting_at.or_else(selected)
    }

    /// The first open item whose place in `W`'s order comes after
    /// `after`, or the first one when none does (or `after` is none).
    fn waiting_after(&self, after: Option<(usize, usize)>) -> Option<usize> {
        let open = self.world.open_inbox();
        let first =
            |items: &mut dyn Iterator<Item = usize>| items.min_by_key(|&i| self.waiting_rank(i));
        first(
            &mut open
                .iter()
                .copied()
                .filter(|&i| after.is_none_or(|a| self.waiting_rank(i) > a)),
        )
        .or_else(|| first(&mut open.iter().copied()))
    }

    /// `W`: from wherever the person is, the next item waiting for them,
    /// selected in the salon's waiting panel and opened as `enter` opens
    /// it. With `defer` (`S`) the item at hand is put off first: it goes
    /// after everything else.
    fn next_waiting(&mut self, defer: bool) {
        let at = self.waiting_at_hand();
        // Where it was before it was put off: what came after it then
        // comes next.
        let after = at.map(|i| self.waiting_rank(i));
        let open = |i: usize| self.world.inbox.get(i).is_some_and(|x| x.open);
        if defer && let Some(i) = at.filter(|&i| open(i)) {
            self.deferred.retain(|&d| d != i && open(d));
            self.deferred.push(i);
        }
        let Some(i) = self.waiting_after(after) else {
            self.flash(self.lang.text().nothing_needed);
            return;
        };
        self.card = None;
        self.thread = None;
        self.view = View::Salon;
        self.zoom = Zoom::None;
        self.pane = Pane::Inbox;
        self.inbox_sel = self.inbox_rows().iter().position(|&o| o == i).unwrap_or(0);
        self.waiting_at = Some(i);
        self.deeper();
    }

    /// Something quitting now would cut short or leave unreviewed.
    fn in_flight(&self) -> bool {
        !self.world.mid_turn().is_empty() || !self.world.open_changes().is_empty()
    }

    /// The quit question, with what is in flight counted.
    fn quit_ask(&self) -> String {
        let t = self.lang.text();
        let counts = [
            (t.quit_mid_turn, self.world.mid_turn().len()),
            (t.quit_changes, self.world.open_changes().len()),
        ];
        let said: Vec<String> = counts
            .iter()
            .filter(|(_, n)| *n > 0)
            .map(|(words, n)| fill(words, &[&n.to_string()]))
            .collect();
        if said.is_empty() {
            t.quit_ask.to_string()
        } else {
            fill(t.quit_ask_busy, &[&said.join(", ")])
        }
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
                .inbox_rows()
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
                self.waiting_at = None;
                let n = self.inbox_rows().len();
                self.inbox_sel = if first { 0 } else { n.saturating_sub(1) };
            }
            Pane::Journal => {
                let rows = self.journal_rows();
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
        let rows = self.journal_rows();
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
        self.view == View::Salon
            && self.zoom == Zoom::None
            && self.pane == Pane::Salon
            && self.card.is_none()
    }

    fn show(&mut self, view: View) {
        self.view = view;
        self.zoom = Zoom::None;
        self.leave_card();
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
    /// before it cancels the writing. The rest are readline's editing
    /// keys (`edit::key`); the menu is offered only with the cursor at
    /// the end, since a completion replaces the word the line ends with.
    /// Shift-enter, alt-enter and enter after a `\` start a new line;
    /// `↑` `↓` go between lines while no menu is shown.
    fn type_key(&mut self, k: KeyEvent) {
        let ctrl = k.modifiers.contains(KeyModifiers::CONTROL);
        let newline =
            k.modifiers.contains(KeyModifiers::SHIFT) || k.modifiers.contains(KeyModifiers::ALT);
        match k.code {
            KeyCode::Char('n') if ctrl => self.pick(true),
            KeyCode::Char('p') if ctrl => self.pick(false),
            KeyCode::Enter if newline => {
                self.edit(k);
            }
            KeyCode::Up | KeyCode::Down if self.menu.is_none() => {
                self.edit(k);
            }
            KeyCode::Enter => match &mut self.menu {
                Some(m) if !m.inserted => {
                    m.insert(&mut self.input);
                    // A card is all there is to take: it opens at once.
                    if self.typing_card() {
                        self.submit();
                    }
                }
                // Enter after a `\` is a new line, not a send.
                _ => {
                    if !self.edit(k) {
                        self.submit();
                    }
                }
            },
            KeyCode::Esc if self.menu.is_some() => self.menu = None,
            KeyCode::Esc => {
                self.typing = false;
                self.input.clear();
                self.after = 0;
            }
            KeyCode::Tab => match &mut self.menu {
                None if self.after > 0 => {}
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
            _ => {
                self.edit(k);
            }
        }
    }

    /// Where the cursor is in `input`, as a byte offset on a character's
    /// edge (the next edge on, should `after` be stale).
    pub fn cursor(&self) -> usize {
        let end = self.input.len();
        let at = end.saturating_sub(self.after);
        (at..end)
            .find(|&i| self.input.is_char_boundary(i))
            .unwrap_or(end)
    }

    /// An editing key: the line or the cursor changes, and with them
    /// what the menu offers. False for a key that is not an editing key.
    fn edit(&mut self, k: KeyEvent) -> bool {
        let (was, at) = (self.input.clone(), self.cursor());
        let Some(to) = edit::key(k, &mut self.input, at, &mut self.cut) else {
            return false;
        };
        self.moved(&was, at, to);
        true
    }

    /// The line was `was` with the cursor at `at`; now the cursor is at
    /// `to`. A change or a move asks the menu again, at the end only.
    fn moved(&mut self, was: &str, at: usize, to: usize) {
        self.after = self.input.len() - to;
        if to != at || self.input != was {
            self.menu = if self.after == 0 {
                self.suggest()
            } else {
                None
            };
        }
    }

    /// A key while the help is open: `/` starts (or goes back to) a
    /// filter, which takes the keys typed until `enter` keeps it or `esc`
    /// drops it (backspace past its start leaves it too); any other key
    /// closes the help.
    fn help_key(&mut self, k: KeyEvent) {
        if !self.help_typing {
            match k.code {
                KeyCode::Char('/') => self.help_typing = true,
                _ => self.help = false,
            }
            return;
        }
        let plain = !k.modifiers.contains(KeyModifiers::CONTROL)
            && !k.modifiers.contains(KeyModifiers::ALT);
        match k.code {
            KeyCode::Enter => self.help_typing = false,
            KeyCode::Esc => {
                self.help_find.clear();
                self.help_typing = false;
            }
            KeyCode::Backspace => {
                if self.help_find.pop().is_none() {
                    self.help_typing = false;
                }
            }
            KeyCode::Char('u') if k.modifiers.contains(KeyModifiers::CONTROL) => {
                self.help_find.clear()
            }
            KeyCode::Char(c) if plain => self.help_find.push(c),
            _ => {}
        }
    }

    /// Text the terminal says was pasted (bracketed paste): it goes into
    /// the line at the cursor whole, its new lines kept, so a paste's
    /// line ends do not send it a line at a time. Only while writing;
    /// elsewhere its characters would be taken as keys, so it is left
    /// out and the person told how to write first.
    pub fn paste(&mut self, text: &str) {
        // Into the help's filter while it is typed, on one line.
        if self.help && self.help_typing {
            let text = pasted(text).replace(['\n', '\t'], " ");
            return self.help_find.push_str(&text);
        }
        let open = self.help || self.digest.is_some() || self.left_off.is_some() || self.leaving;
        if !self.typing || open {
            return self.flash(self.lang.text().paste_not_writing);
        }
        let text = pasted(text);
        let (was, at) = (self.input.clone(), self.cursor());
        self.input.insert_str(at, &text);
        self.moved(&was, at, at + text.len());
    }

    /// Highlights the next (or previous) match and puts it in.
    fn pick(&mut self, forward: bool) {
        if let Some(m) = &mut self.menu {
            m.step(forward);
            m.insert(&mut self.input);
        }
    }

    /// What the composer could complete the input with, now. Nothing once
    /// the text runs over lines: a name or a command is on one.
    fn suggest(&self) -> Option<Menu> {
        if self.input.contains('\n') {
            return None;
        }
        let mut m = complete::suggest(
            &self.input,
            &self.world,
            self.current_project(),
            self.home.as_deref(),
        )?;
        // The command names: the demo's own left out live, each with its
        // value.
        if self.typing_command() && !self.input.contains(' ') {
            if self.live() {
                m.items
                    .retain(|i| !complete::DEMO_ONLY.contains(&i.as_str()));
                if m.items.is_empty() {
                    return None;
                }
            }
            m.notes = m.items.iter().map(|i| self.command_note(i)).collect();
        }
        Some(m)
    }

    /// What the `:` menu shows after command `name`: what it stands at
    /// now or would act on, in brackets, and `$` when using it raises the
    /// next bill (`:open` a new leader session, `:forget` a new session
    /// that pays the whole briefing again, without the cache).
    fn command_note(&self, name: &str) -> String {
        let t = self.lang.text();
        let on_off = |on: bool| if on { t.value_on } else { t.value_off }.to_string();
        let value = match name {
            "budget" => Some(fill(
                t.budget,
                &[&dollars(self.world.spent()), &dollars(self.world.budget)],
            )),
            "lead" => Some(self.primary.clone().unwrap_or_else(|| "ask".to_string())),
            "theme" => Some(theme::pal().name.clone()),
            "lang" => Some(t.language.to_string()),
            "close" | "rename" => Some(self.world.projects[self.current_project()].name.clone()),
            "forget" | "talk" | "trace" => self
                .agent_at_hand()
                .map(|id| self.world.agents[id].name.clone()),
            "next" | "waiting" => {
                let open = self.world.open_inbox().len().to_string();
                Some(fill(t.value_waiting, &[&open]))
            }
            "needs" => Some(on_off(self.needs_only)),
            "pause" => Some(on_off(self.paused)),
            "faster" | "slower" => Some(format!("×{}", self.speed)),
            _ => None,
        };
        let bill = matches!(name, "forget" | "open");
        match (value, bill) {
            (Some(v), true) => format!("({v}) $"),
            (Some(v), false) => format!("({v})"),
            (None, true) => "$".to_string(),
            (None, false) => String::new(),
        }
    }

    /// Whether what is being typed is a command to imece (`:budget 80`),
    /// not a message to an agent.
    pub fn typing_command(&self) -> bool {
        self.input.starts_with(':')
    }

    /// Whether what is being typed names a card to open (`#323`), not a
    /// message to an agent.
    pub fn typing_card(&self) -> bool {
        self.input.starts_with('#')
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
            Some(rest) => rest
                .split(char::is_whitespace)
                .next()
                .unwrap_or_default()
                .to_string(),
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
        self.after = 0;
        self.typing = false;
        self.menu = None;
        if text.is_empty() {
            return;
        }
        let t = self.lang.text();
        if let Some(line) = text.strip_prefix(':') {
            return self.command(line.trim());
        }
        if text.starts_with('#') {
            return match complete::card_named(&text, &self.world) {
                Some(c) => self.open_card(c),
                None => self.flash(fill(t.no_card_found, &[&text])),
            };
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
        let msg = rest
            .split_once(char::is_whitespace)
            .map_or("", |(_, m)| m)
            .trim();
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
            Engine::Sim(s) if command => s.command(&mut self.world, id, text).err(),
            Engine::Sim(s) => s.told(&mut self.world, id, text).err(),
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
    ///
    /// Every key that acts has a name here too (`:salon`, `:lang`,
    /// `:quit` ...; `complete::COMMANDS`), doing what the key does from
    /// wherever the person is.
    fn command(&mut self, line: &str) {
        let t = self.lang.text();
        let (name, arg) = line
            .split_once(char::is_whitespace)
            .map_or((line, ""), |(n, a)| (n, a.trim()));
        match complete::long(name) {
            "budget" => match parse_dollars(arg) {
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
            "theme" if arg.is_empty() => self.next_theme(),
            "theme" => match theme::choose(arg) {
                Ok(theme) => self.flash(fill(t.theme_is, &[&theme.name])),
                Err(why) => self.flash(fill(t.bad_theme, &[arg, &why])),
            },
            "open" => self.open_project(arg),
            "close" => self.close_project(arg),
            "rename" => self.rename_project(arg),
            "forget" => self.forget_agent(arg),
            "salon" => self.show(View::Salon),
            "board" => self.show(View::Board),
            "code" => self.show(View::Code),
            "lang" if arg.is_empty() => self.set_lang(self.lang.other()),
            "lang" => match Lang::parse(arg) {
                Some(lang) => self.set_lang(lang),
                None => self.flash(fill(t.bad_lang, &[arg])),
            },
            "help" => self.open_help(),
            "quit" => self.ask_quit(),
            "pending" => self.leaving = self.in_flight(),
            "next" => self.next_waiting(false),
            "defer" => self.next_waiting(true),
            "waiting" => self.go_waiting(),
            "needs" => {
                self.go_salon();
                self.toggle_needs();
            }
            which @ ("fold" | "unfold") => {
                self.go_salon();
                self.fold_all(which == "fold");
            }
            "trace" => {
                if let Some(id) = self.agent_named(arg) {
                    self.trace(id);
                }
            }
            "talk" => {
                if let Some(id) = self.agent_named(arg) {
                    self.open_talk_with(id);
                }
            }
            "pause" | "faster" | "slower" if self.live() => self.flash(fill(t.demo_only, &[name])),
            "pause" => self.pause(),
            "faster" => self.faster(),
            "slower" => self.slower(),
            _ => self.flash(fill(t.no_command, &[name])),
        }
    }

    /// The agent a command's argument names, `@` or not; none named is
    /// the agent at hand. Nobody by that name is said.
    fn agent_named(&mut self, arg: &str) -> Option<AgentId> {
        let word = arg.strip_prefix('@').unwrap_or(arg).trim();
        let id = if word.is_empty() {
            self.agent_at_hand()
        } else {
            self.world.agent_by_name(word)
        };
        if id.is_none() {
            let t = self.lang.text();
            self.flash(fill(t.no_agent, &[word]));
        }
        id
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
        if self.card.is_some_and(|c| self.world.cards[c].project == p) {
            self.card = None;
        }
        let open = self.inbox_rows().len();
        self.inbox_sel = self.inbox_sel.min(open.saturating_sub(1));
        self.flash(fill(t.closed, &[&name]));
    }

    /// `:forget [@NAME]`: the agent (named, else the one at hand) forgets
    /// its session; its next message starts a new one, told where it left
    /// off.
    fn forget_agent(&mut self, arg: &str) {
        let t = self.lang.text();
        let Some(id) = self.agent_named(arg) else {
            return;
        };
        let name = self.world.agents[id].name.clone();
        let forgot = match &mut self.engine {
            Engine::Live(live) => live.forget_agent(&mut self.world, id),
            Engine::Sim(sim) => sim.forget(&mut self.world, id),
        };
        match forgot {
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

    /// Whether a card's screen is what the person looks at (it is only
    /// ever open in the salon view).
    fn on_card(&self) -> bool {
        self.card.is_some() && self.zoom == Zoom::None
    }

    /// The open inbox items the side panel lists: a card's, on its
    /// screen, else all.
    pub fn inbox_rows(&self) -> Vec<usize> {
        match self.card {
            Some(c) => self.world.card_inbox(c),
            None => self.world.open_inbox(),
        }
    }

    /// The journal rows the side panel lists: a card's, on its screen,
    /// else the whole journal's.
    pub fn journal_rows(&self) -> Vec<(usize, usize)> {
        match self.card {
            Some(c) => ui::card_rows(&self.world, c),
            None => ui::journal_rows(&self.world),
        }
    }

    /// Card `c`'s screen takes the salon's place, its journal selected.
    pub fn open_card(&mut self, c: CardId) {
        self.card = Some(c);
        self.view = View::Salon;
        self.zoom = Zoom::None;
        self.thread = None;
        self.pane = Pane::Journal;
        self.inbox_sel = 0;
        self.journal_sel = None;
        self.select_journal(None);
        if let Some(&(id, _)) = self.world.card_people(c).first() {
            self.selected = Sel::Agent(self.world.cards[c].owner.unwrap_or(id));
        }
    }

    /// Back from a card's screen to the salon.
    fn leave_card(&mut self) {
        if self.card.take().is_some() {
            self.inbox_sel = 0;
            self.select_journal(None);
        }
    }

    /// The next card (or the previous one) of an open project, by number.
    fn step_card(&mut self, next: bool) {
        let Some(c) = self.card else {
            return;
        };
        let w = &self.world;
        let mut open = w.cards.iter().filter(|x| !w.projects[x.project].closed);
        let to = if next {
            open.find(|x| x.id > c)
        } else {
            open.rfind(|x| x.id < c)
        };
        if let Some(to) = to.map(|x| x.id) {
            self.open_card(to);
        }
    }

    /// The card the person is looking at: the zoomed agent's, the selected
    /// agent's, the one the selected inbox item came on, or the one a
    /// selected journal line names (or mentions, `#323`).
    fn card_at_hand(&self) -> Option<CardId> {
        let w = &self.world;
        if let Zoom::Focus(id) | Zoom::Trace(id) | Zoom::Diff(id) = self.zoom {
            return w.card_of(id);
        }
        match self.pane {
            Pane::Salon => match self.selected {
                Sel::Agent(id) => w.card_of(id),
                Sel::Project(_) => None,
            },
            Pane::Inbox => self
                .inbox_rows()
                .get(self.inbox_sel)
                .and_then(|&i| w.inbox[i].card),
            Pane::Journal => {
                let e = &w.journal[self.journal_sel?].event;
                e.card().or_else(|| {
                    let text = ui::words::said(e)?.text;
                    text.match_indices('#')
                        .find_map(|(at, _)| complete::card_named(&text[at..], w))
                })
            }
        }
    }

    fn move_selection(&mut self, down: bool) {
        match self.pane {
            Pane::Journal => self.select_journal(Some(down)),
            Pane::Inbox => {
                // The person picks: `W` goes on from what they picked.
                self.waiting_at = None;
                let n = self.inbox_rows().len();
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
                    let open = self.inbox_rows();
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
                (Pane::Salon, Sel::Project(p)) if self.card.is_none() => {
                    self.collapsed[p] = !self.collapsed[p];
                    None
                }
                (Pane::Salon, Sel::Agent(id)) => Some(Zoom::Focus(id)),
                (Pane::Salon, Sel::Project(_)) => None,
            },
            Zoom::Focus(id) | Zoom::Trace(id) => Some(Zoom::Trace(id)),
            Zoom::Diff(_) => None,
        };
        if let Some(z) = zoom {
            self.zoom = z;
        }
    }

    fn answer(&mut self, approve: bool) {
        let open = self.inbox_rows();
        let Some(&item) = open.get(self.inbox_sel) else {
            return;
        };
        let t = self.lang.text();
        if self.world.inbox[item].kind == InboxKind::Change {
            return self.decide_change(self.world.inbox[item].agent, approve);
        }
        if self.world.inbox[item].kind == InboxKind::Approval {
            let name = self.world.agents[self.world.inbox[item].agent].name.clone();
            match &mut self.engine {
                Engine::Live(l) => l.decide(&mut self.world, item, approve),
                // The demo's session goes on once the item is closed.
                Engine::Sim(_) => self.world.decided(item, approve),
            }
            let how = if approve { t.allowed } else { t.refused };
            self.flash(fill(how, &[&name]));
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
        let open = self.inbox_rows().len();
        let decided = match &mut self.engine {
            Engine::Live(l) if apply => l.apply_change(&self.world, id),
            Engine::Sim(s) if apply => s.apply_change(&self.world, id),
            Engine::Live(l) => l.reject_change(&mut self.world, id),
            Engine::Sim(s) => s.reject_change(&mut self.world, id),
        };
        match decided {
            Ok(()) if apply => self.flash(fill(t.applying, &[&name])),
            Err(why) if apply => self.flash(fill(t.apply_failed, &[&name, &why])),
            Ok(()) => {
                self.flash(fill(t.rejected_change, &[&name]));
                self.typing = true;
                self.input = format!("@{name} ");
            }
            Err(why) => self.flash(fill(t.reject_failed, &[&name, &why])),
        }
        if self.zoom == Zoom::Diff(id) {
            self.zoom = Zoom::None;
            self.pane = Pane::Inbox;
        }
        self.inbox_sel = self.inbox_sel.min(open.saturating_sub(2));
    }
}

/// Pasted text as the line takes it: line ends as `\n` (a terminal
/// pastes them as `\r`, Windows text as `\r\n`), tabs kept, every other
/// control character (an escape sequence's ESC among them) left out.
fn pasted(text: &str) -> String {
    text.replace("\r\n", "\n")
        .replace('\r', "\n")
        .chars()
        .filter(|&c| c == '\n' || c == '\t' || !c.is_control())
        .collect()
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
        let t = Lang::En.text();
        let flash = |app: &App| app.flash.as_ref().map(|(m, _)| m.clone());
        for c in ['a', 'r'] {
            let (mut app, worker) = with_change(&two_files());
            let name = app.world.agents[worker].name.clone();
            app.on_key(key(KeyCode::Char(c)));
            assert_eq!(app.zoom, Zoom::None, "{c} closes the change");
            assert_eq!(app.pane, Pane::Inbox, "{c} returns to the inbox");
            if c == 'a' {
                // The demo applies it as git's thread does: the next step
                // says how it went.
                assert_eq!(flash(&app), Some(fill(t.applying, &[&name])));
                assert!(app.world.change_of(worker).is_some() && !app.typing);
                app.on_key(key(KeyCode::Char('a')));
                assert_eq!(
                    flash(&app),
                    Some(fill(
                        t.apply_failed,
                        &[&name, "their change is being applied"]
                    )),
                    "once is enough"
                );
                app.step();
                assert!(app.world.journal.iter().any(|e| matches!(
                    e.event,
                    Event::Applied { agent, .. } | Event::ApplyFailed { agent, .. } if agent == worker
                )));
            } else {
                assert_eq!(flash(&app), Some(fill(t.rejected_change, &[&name])));
                assert!(app.world.change_of(worker).is_none());
                assert!(app.typing, "the composer opens for the reasons");
                assert_eq!(app.input, format!("@{name} "));
            }
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
                card: None,
            });
            app.inbox_sel = 1;
            let name = app.world.agents[worker].name.clone();
            app.on_key(key(KeyCode::Char(c)));
            assert_eq!(app.zoom, Zoom::None, "{c} closes the change");
            let flash = app.flash.as_ref().map(|(m, _)| m.clone());
            let said = if c == 'a' {
                Lang::En.text().applying
            } else {
                Lang::En.text().rejected_change
            };
            assert_eq!(flash, Some(fill(said, &[&name])));
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
    fn the_colon_menu_notes_each_commands_value_and_what_raises_the_bill() {
        let mut app = App::new(1, 1, 0, 1, Lang::En);
        app.world.agents.iter_mut().for_each(|a| a.cost = 0);
        app.world.agents[0].cost = 31_000;
        app.world.budget = 200_000;
        assert_eq!(app.command_note("budget"), "(spent $3.10 of $20.00)");
        assert_eq!(app.command_note("lead"), "(ask)");
        app.primary = Some("codex".into());
        assert_eq!(app.command_note("lead"), "(codex)");
        theme::choose("light").unwrap();
        assert_eq!(app.command_note("theme"), "(light)");
        assert_eq!(app.command_note("lang"), "(English)");
        app.lang = Lang::Tr;
        assert_eq!(app.command_note("lang"), "(Türkçe)");
        assert_eq!(app.command_note("needs"), "(kapalı)");
        app.lang = Lang::En;
        let project = format!("({})", app.world.projects[0].name);
        assert_eq!(app.command_note("close"), project);
        assert_eq!(app.command_note("rename"), project);
        // The agent a bare command acts on; `$` on what raises the bill.
        app.selected = Sel::Agent(1);
        let name = app.world.agents[1].name.clone();
        assert_eq!(app.command_note("forget"), format!("({name}) $"));
        assert_eq!(app.command_note("talk"), format!("({name})"));
        assert_eq!(app.command_note("trace"), format!("({name})"));
        assert_eq!(app.command_note("open"), "$");
        app.pane = Pane::Inbox;
        assert_eq!(app.command_note("forget"), "$", "no agent at hand");
        assert_eq!(app.command_note("talk"), "");
        assert_eq!(app.command_note("next"), "(0 waiting)");
        app.world.asks_you(1, "Bash: make deploy");
        app.world.asks_you(1, "Bash: make test");
        assert_eq!(app.command_note("next"), "(2 waiting)");
        assert_eq!(app.command_note("waiting"), "(2 waiting)");
        assert_eq!(app.command_note("needs"), "(off)");
        app.needs_only = true;
        assert_eq!(app.command_note("needs"), "(on)");
        assert_eq!(app.command_note("pause"), "(off)");
        app.paused = true;
        assert_eq!(app.command_note("pause"), "(on)");
        app.speed = 4;
        assert_eq!(app.command_note("faster"), "(×4)");
        assert_eq!(app.command_note("slower"), "(×4)");
        for plain in ["help", "quit", "salon", "fold", "defer"] {
            assert_eq!(app.command_note(plain), "", "{plain}");
        }
    }

    #[test]
    fn the_colon_menu_carries_the_notes_of_its_items() {
        let mut app = App::new(1, 1, 0, 1, Lang::En);
        app.on_key(key(KeyCode::Char(':')));
        for c in "op".chars() {
            app.on_key(key(KeyCode::Char(c)));
        }
        let m = app.menu.clone().unwrap();
        assert_eq!(m.items, ["open"]);
        assert_eq!(m.notes, ["$"]);
        // An argument's menu has no notes, nor do names after `@`.
        app.input = ":lead c".into();
        app.menu = app.suggest();
        assert_eq!(app.menu.as_ref().unwrap().items, ["claude", "codex"]);
        assert!(app.menu.as_ref().unwrap().notes.is_empty());
        app.input = "@".into();
        app.menu = app.suggest();
        assert!(!app.menu.as_ref().unwrap().items.is_empty());
        assert!(app.menu.as_ref().unwrap().notes.is_empty());
    }

    #[test]
    fn the_view_keys_and_l_do_what_their_colon_names_do() {
        let mut app = App::new(1, 1, 0, 1, Lang::En);
        for (c, view) in [('3', View::Code), ('2', View::Board), ('1', View::Salon)] {
            app.on_key(key(KeyCode::Char(c)));
            assert_eq!(app.view, view, "{c}");
        }
        app.on_key(key(KeyCode::Char('L')));
        assert_eq!(app.lang, Lang::Tr);
        assert_eq!(app.flash.take().unwrap().0, "Türkçe");
        app.on_key(key(KeyCode::Char('L')));
        assert_eq!(app.lang, Lang::En);
    }

    #[test]
    fn every_key_that_acts_has_a_colon_name_doing_the_same() {
        let mut app = App::new(1, 1, 0, 1, Lang::En);
        let said = |app: &mut App| app.flash.take().map(|f| f.0).unwrap_or_default();
        app.command("board");
        assert_eq!(app.view, View::Board);
        app.command("code");
        assert_eq!(app.view, View::Code);
        app.command("salon");
        assert_eq!(app.view, View::Salon);
        // The language: turned over, or named.
        app.command("lang");
        assert_eq!(app.lang, Lang::Tr);
        assert_eq!(said(&mut app), "Türkçe");
        app.command("lang tr");
        assert_eq!(app.lang, Lang::Tr);
        app.command("lang en");
        assert_eq!(app.lang, Lang::En);
        app.command("lang xx");
        assert_eq!(app.lang, Lang::En);
        assert_eq!(said(&mut app), "xx is not a language: en or tr");
        // `:theme` alone is `T`.
        theme::choose("dark").unwrap();
        app.command("theme");
        let next = "light";
        assert_eq!(theme::pal().name, next);
        assert_eq!(said(&mut app), format!("theme: {next}"));
        app.help_find = "old".into();
        app.command("help");
        assert!(app.help && app.help_find.is_empty());
        app.help = false;
        // `:quit` and `:q` ask as `q` does.
        for line in ["quit", "q"] {
            app.pending = None;
            app.command(line);
            assert_eq!(app.pending, Some('q'), "{line}");
            assert_eq!(said(&mut app), app.quit_ask());
        }
        app.pending = None;
        app.leaving = true;
        app.command("pending");
        assert!(!app.leaving, "nothing in flight");
        app.world.asks_you(1, "Bash: one");
        app.world.asks_you(1, "Bash: two");
        app.command("next");
        assert_eq!(app.waiting_at, Some(0));
        app.command("defer");
        assert_eq!(app.waiting_at, Some(1));
        app.show(View::Board);
        app.command("waiting");
        assert_eq!((app.view, app.pane), (View::Salon, Pane::Inbox));
        app.show(View::Board);
        app.pane = Pane::Inbox;
        app.command("needs");
        assert_eq!((app.view, app.pane), (View::Salon, Pane::Salon));
        assert!(app.needs_only);
        app.show(View::Code);
        app.command("fold");
        assert_eq!(app.view, View::Salon);
        assert!(app.collapsed.iter().all(|&c| c));
        app.command("unfold");
        assert!(app.collapsed.iter().all(|&c| !c));
        // An agent: the one at hand, or named, `@` or not.
        let name = app.world.agents[1].name.clone();
        app.selected = Sel::Agent(0);
        app.command("trace");
        assert_eq!(app.zoom, Zoom::Trace(0));
        app.command(&format!("trace @{name}"));
        assert_eq!(app.zoom, Zoom::Trace(1));
        app.zoom = Zoom::None;
        app.command("trace nobody");
        assert_eq!(app.zoom, Zoom::None);
        assert_eq!(said(&mut app), "no agent named \"nobody\"");
        app.command(&format!("talk {name}"));
        let t = app.lang.text();
        assert_eq!(said(&mut app), fill(t.no_talk_yet, &[&name]));
        app.command("talk nobody");
        assert_eq!(said(&mut app), "no agent named \"nobody\"");
        // The demo's clock.
        app.command("pause");
        assert!(app.paused);
        app.command("faster");
        assert_eq!(app.speed, 2);
        app.command("faster");
        app.command("slower");
        assert_eq!(app.speed, 2);
        app.command("nope");
        assert_eq!(said(&mut app), "no command nope; : and tab list them");
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
        // a demo agent has a session once it took a turn, as a live one.
        let t = Lang::En.text();
        app.command("forget nobody");
        assert_eq!(said(&mut app), "no agent named \"nobody\"");
        app.command("forget @nobody");
        assert_eq!(said(&mut app), "no agent named \"nobody\"");
        let lead = app.world.agents[0].name.clone();
        let none = fill(t.forget_refused, &[&lead, "it has no session to forget"]);
        app.command(&format!("forget @{lead}"));
        assert_eq!(said(&mut app), none);
        app.tell(0, "hello");
        app.command(&format!("f {lead}"));
        assert_eq!(said(&mut app), fill(t.forgot, &[&lead]));
        assert_eq!(app.world.agents[0].status, Status::Asleep);
        app.selected = Sel::Agent(0);
        app.command("forget");
        assert_eq!(said(&mut app), none, "forgotten already");
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
        // What waits in the inbox leads only while the inbox has the keys.
        app.world.projects[0].closed = false;
        app.world.replied(app.world.projects[0].leader, "an answer");
        assert_eq!(app.current_project(), 1, "the salon's selection");
        app.pane = Pane::Inbox;
        assert_eq!(app.current_project(), 0, "the inbox's selection");
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
            card: None,
        }
    }

    #[test]
    fn what_an_agent_needs_of_the_person_is_ranked() {
        let mut app = quiet();
        assert_eq!(app.world.agents.len(), 5, "the lead and four workers");
        assert_eq!(app.needs(), [Need::Nothing; 5], "idle, nothing open");
        // An open item waits, a closed one does not; one that left needs
        // nothing.
        app.world.inbox.push(item(1, true));
        app.world.inbox.push(item(2, false));
        app.world.inbox.push(item(3, true));
        app.world.agents[3].gone = true;
        assert_eq!(
            app.needs(),
            [
                Need::Nothing,
                Need::Waiting,
                Need::Nothing,
                Need::Nothing,
                Need::Nothing
            ]
        );
        app.world.inbox.clear();
        app.world.agents[3].gone = false;
        app.world.agents[1].status = Status::WaitingHuman;
        app.world.agents[2].status = Status::Blocked;
        app.world.agents[3].status = Status::Stopped;
        app.unseen.extend([0, 4]);
        assert_eq!(
            app.needs(),
            [
                Need::Done,
                Need::Waiting,
                Need::Stuck,
                Need::Stuck,
                Need::Done
            ]
        );
        // Done is a finished turn: at work again it is no news; an item
        // says more than done.
        app.world.agents[4].status = Status::Working;
        app.world.inbox.push(item(0, true));
        assert_eq!(app.needs()[4], Need::Nothing);
        assert_eq!(app.needs()[0], Need::Waiting);
        app.world.agents[2].gone = true;
        assert_eq!(app.needs()[2], Need::Nothing, "gone and blocked");
        assert!(Need::Waiting > Need::Stuck && Need::Stuck > Need::Done);
        assert!(Need::Done > Need::Nothing);
    }

    /// The salon's selectable rows.
    fn shown(app: &App) -> Vec<Sel> {
        ui::salon::rows(app, 100)
            .iter()
            .filter_map(ui::salon::Row::sel)
            .collect()
    }

    /// Two projects, everyone idle with nothing open; the first two
    /// workers of project 0 and the first of project 1.
    fn two_quiet_projects() -> (App, [AgentId; 3]) {
        let mut app = App::new(2, 4, 0, 1, Lang::En);
        app.world.inbox.clear();
        for a in &mut app.world.agents {
            a.status = Status::Idle;
        }
        let of = |p| {
            app.world
                .agents
                .iter()
                .filter(move |a| a.project == p && a.role == Role::Worker)
                .map(|a| a.id)
        };
        let mut zero = of(0);
        let (a, b) = (zero.next().unwrap(), zero.next().unwrap());
        let c = of(1).next().unwrap();
        (app, [a, b, c])
    }

    #[test]
    fn f_shows_only_what_needs_you_and_a_project_with_none_is_left_out() {
        let (mut app, [a, b, c]) = two_quiet_projects();
        app.world.agents[c].status = Status::Blocked;
        // Not from another pane.
        app.pane = Pane::Inbox;
        app.on_key(key(KeyCode::Char('f')));
        assert!(!app.needs_only);
        app.pane = Pane::Salon;
        app.on_key(key(KeyCode::Char('f')));
        assert!(app.needs_only);
        assert_eq!(app.flash.as_ref().unwrap().0, Lang::En.text().needs_on);
        // Only project 1, and of it only its stuck worker: not its lead.
        assert_eq!(shown(&app), [Sel::Project(1), Sel::Agent(c)]);
        // A folded project shows its header alone.
        app.collapsed[1] = true;
        assert_eq!(shown(&app), [Sel::Project(1)]);
        app.collapsed[1] = false;
        app.world.agents[b].status = Status::Blocked;
        assert_eq!(
            shown(&app),
            [
                Sel::Project(0),
                Sel::Agent(b),
                Sel::Project(1),
                Sel::Agent(c)
            ]
        );
        assert!(!shown(&app).contains(&Sel::Agent(a)));
        app.on_key(key(KeyCode::Char('f')));
        assert!(!app.needs_only);
        assert_eq!(app.flash.as_ref().unwrap().0, Lang::En.text().needs_off);
        assert!(shown(&app).contains(&Sel::Agent(a)), "everyone again");
    }

    #[test]
    fn with_only_what_needs_you_shown_the_selection_stays_on_a_shown_row() {
        let (mut app, [a, b, c]) = two_quiet_projects();
        for id in [a, b, c] {
            app.world.agents[id].status = Status::Blocked;
        }
        app.selected = Sel::Agent(b);
        app.on_key(key(KeyCode::Char('f')));
        assert_eq!(app.selected, Sel::Agent(b), "shown: it stays");
        // b needs nothing now: to the first row of its project.
        app.world.agents[b].status = Status::Idle;
        app.on_key(key(KeyCode::Char('x')));
        assert_eq!(app.selected, Sel::Project(0));
        // Nothing left in project 1: to the first row there is.
        app.selected = Sel::Agent(c);
        app.world.agents[c].status = Status::Idle;
        app.on_key(key(KeyCode::Char('x')));
        assert_eq!(app.selected, Sel::Project(0));
        // Not while the person writes: who they write to stays put.
        app.selected = Sel::Agent(a);
        app.on_key(key(KeyCode::Char('i')));
        app.world.agents[a].status = Status::Idle;
        app.on_key(key(KeyCode::Char('z')));
        assert_eq!(app.selected, Sel::Agent(a));
        assert_eq!(app.input, "z");
        // Nothing shown at all: it stays too.
        app.on_key(key(KeyCode::Esc));
        assert!(!app.typing && shown(&app).is_empty());
        assert_eq!(app.selected, Sel::Agent(a));
        // Showing everyone, a selection off the rows is left alone.
        app.needs_only = false;
        app.collapsed[0] = true;
        app.on_key(key(KeyCode::Char('x')));
        assert_eq!(app.selected, Sel::Agent(a));
    }

    #[test]
    fn a_step_moves_a_selection_whose_row_left_the_filter() {
        let (mut app, [a, b, c]) = two_quiet_projects();
        for id in [a, c] {
            app.world.agents[id].status = Status::Blocked;
        }
        app.needs_only = true;
        app.world.agents[b].gone = true;
        app.selected = Sel::Agent(b);
        app.step();
        assert_ne!(app.selected, Sel::Agent(b));
        assert!(shown(&app).contains(&app.selected), "{:?}", app.selected);
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
    fn the_demo_pauses_and_changes_speed_by_halves_and_doubles_and_live_does_not() {
        let mut app = App::new(1, 1, 0, 1, Lang::En);
        assert_eq!((app.speed, app.paused), (1, false));
        let mut speeds = vec![];
        for c in "+=++----".chars() {
            app.on_key(key(KeyCode::Char(c)));
            speeds.push(app.speed);
        }
        assert_eq!(
            speeds,
            [2, 4, 8, 8, 4, 2, 1, 1],
            "doubled to 8, halved to 1"
        );
        app.on_key(key(KeyCode::Char(' ')));
        assert!(app.paused);
        app.on_key(key(KeyCode::Char(' ')));
        assert!(!app.paused);
        // Live agents run in real time.
        let w = crate::live::testing::team(1);
        let live = crate::live::testing::idle_live(2);
        let mut app = App::with(w, Engine::Live(Box::new(live)), Lang::En);
        app.speed = 4;
        for c in "+=- ".chars() {
            app.on_key(key(KeyCode::Char(c)));
            assert_eq!((app.speed, app.paused), (4, false), "{c:?}");
        }
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

    /// The demo after a while, its cards on the board.
    /// A demo team at its start with nobody at work and nothing waiting.
    fn quiet() -> App {
        let mut app = App::new(1, 4, 0, 1, Lang::En);
        app.world.inbox.clear();
        for a in &mut app.world.agents {
            a.status = Status::Idle;
        }
        app
    }

    fn a_change(agent: AgentId) -> Change {
        Change {
            agent,
            title: "did it".into(),
            files: Vec::new(),
            patch: "+x\n".into(),
            runs: Vec::new(),
        }
    }

    /// What one `q` asks.
    fn asked(app: &mut App) -> String {
        app.on_key(key(KeyCode::Char('q')));
        assert_eq!(app.pending, Some('q'));
        let said = app.flash.take().map(|(m, _)| m).unwrap_or_default();
        app.pending = None;
        said
    }

    #[test]
    fn the_quit_question_counts_what_is_mid_turn_and_not_reviewed() {
        let mut app = quiet();
        let t = Lang::En.text();
        assert_eq!(asked(&mut app), t.quit_ask);
        app.world.agents[1].status = Status::Working;
        assert_eq!(
            asked(&mut app),
            "quit imece? agents mid-turn: 1 · q again quits · s shows them · any other key stays"
        );
        app.world.agents[2].status = Status::Researching {
            for_agent: 1,
            until: 9,
        };
        app.world.change_ready(a_change(3));
        app.world.agents[3].status = Status::Idle;
        assert_eq!(
            asked(&mut app),
            "quit imece? agents mid-turn: 2, changes not reviewed: 1 · q again quits · s shows them · any other key stays"
        );
        app.world.agents[1].status = Status::Idle;
        app.world.agents[2].status = Status::Idle;
        assert_eq!(
            asked(&mut app),
            "quit imece? changes not reviewed: 1 · q again quits · s shows them · any other key stays"
        );
        app.lang = Lang::Tr;
        assert_eq!(
            asked(&mut app),
            "imece kapatılsın mı? incelenmemiş değişiklik: 1 · bir daha q çıkar · s gösterir · başka bir tuş kalır"
        );
        // Another key stays.
        app.on_key(key(KeyCode::Char('q')));
        app.on_key(key(KeyCode::Char('x')));
        assert!(!app.quit && app.pending.is_none());
    }

    #[test]
    fn q_s_shows_what_quitting_would_leave_and_q_there_quits() {
        let mut app = quiet();
        // Nothing in flight: nothing to show, nothing quits.
        app.on_key(key(KeyCode::Char('q')));
        app.on_key(key(KeyCode::Char('s')));
        assert!(!app.leaving && !app.quit);
        // A turn under way: shown; another key goes back.
        app.world.agents[1].status = Status::Working;
        app.on_key(key(KeyCode::Char('q')));
        app.on_key(key(KeyCode::Char('s')));
        assert!(
            app.leaving && app.flash.is_none(),
            "the question is answered"
        );
        app.on_key(key(KeyCode::Char('x')));
        assert!(!app.leaving && !app.quit && app.pending.is_none());
        // q in the window quits at once.
        app.on_key(key(KeyCode::Char('q')));
        app.on_key(key(KeyCode::Char('s')));
        app.on_key(key(KeyCode::Char('q')));
        assert!(app.quit && !app.leaving);
        // A change alone is shown too, and W there opens it.
        let mut app = quiet();
        app.world.change_ready(a_change(3));
        app.world.agents[3].status = Status::Idle;
        app.view = View::Board;
        app.on_key(key(KeyCode::Char('q')));
        app.on_key(key(KeyCode::Char('s')));
        assert!(app.leaving);
        app.on_key(key(KeyCode::Char('W')));
        assert!(!app.leaving && !app.quit);
        assert_eq!((app.view, app.zoom), (View::Salon, Zoom::Diff(3)));
    }

    #[test]
    fn a_turn_that_ends_unseen_is_marked_until_it_is_opened() {
        let mut app = quiet();
        app.world.agents[1].status = Status::Working;
        app.world.agents[2].status = Status::Researching {
            for_agent: 1,
            until: 9,
        };
        app.world.agents[3].status = Status::Working;
        app.watch_turns();
        assert!(app.unseen.is_empty(), "still at work");
        app.world.agents[1].status = Status::Idle;
        app.world.agents[2].status = Status::Idle;
        app.world.agents[3].status = Status::Asking { to: 1, until: 9 };
        app.watch_turns();
        assert_eq!(app.unseen, BTreeSet::from([1, 2]), "asking is not done");
        // At work again: the earlier turn is no news any more.
        app.world.agents[1].status = Status::Working;
        app.watch_turns();
        assert_eq!(app.unseen, BTreeSet::from([2]));
        // Opened: seen.
        app.zoom = Zoom::Focus(2);
        app.on_key(key(KeyCode::Char('x')));
        assert!(app.unseen.is_empty());
        // A turn that ends while the person looks at it is seen.
        app.zoom = Zoom::Trace(1);
        app.world.agents[1].status = Status::Idle;
        app.watch_turns();
        assert!(app.unseen.is_empty(), "{:?}", app.unseen);
    }

    #[test]
    fn the_demo_marks_the_turns_that_end_as_it_steps() {
        let mut app = App::new(1, 4, 0, 7, Lang::En);
        // A bounded range: the demo's workers finish turns well within it.
        let marked = (0..400).position(|_| {
            app.step();
            let ids: Vec<AgentId> = app.world.agents.iter().map(|a| a.id).collect();
            ids.into_iter().any(|id| app.done_unseen(id))
        });
        assert!(marked.is_some(), "a turn ended and nobody looked");
    }

    #[test]
    fn what_is_open_is_the_zoomed_agent_or_the_conversations_agents() {
        let mut app = quiet();
        assert!(app.opened().is_empty());
        for (zoom, id) in [(Zoom::Focus(1), 1), (Zoom::Trace(2), 2), (Zoom::Diff(3), 3)] {
            app.zoom = zoom;
            assert_eq!(app.opened(), [id]);
        }
        app.zoom = Zoom::None;
        app.world.note(Event::YouSaid {
            to: 2,
            text: "hi".into(),
        });
        app.open_thread(app.world.journal.len() - 1);
        assert_eq!(app.opened(), [2], "the person is not an agent");
        app.world.note(Event::Talk {
            from: 3,
            to: 1,
            text: "hi".into(),
        });
        app.open_thread(app.world.journal.len() - 1);
        let mut both = app.opened();
        both.sort();
        assert_eq!(both, [1, 3]);
        app.unseen = BTreeSet::from([1, 2, 3]);
        app.on_key(key(KeyCode::Char('j')));
        assert_eq!(app.unseen, BTreeSet::from([2]));
    }

    #[test]
    fn done_unseen_is_an_idle_agent_marked_with_nothing_waiting() {
        let mut app = quiet();
        assert!(!app.done_unseen(1), "not marked");
        app.unseen.insert(1);
        assert!(app.done_unseen(1));
        app.world.agents[1].status = Status::Asleep;
        assert!(!app.done_unseen(1), "only an idle one");
        app.world.agents[1].status = Status::Idle;
        app.world.inbox.push(InboxItem {
            kind: InboxKind::Question,
            agent: 1,
            detail: "which?".into(),
            researcher: None,
            open: true,
            tick: 0,
            card: None,
        });
        assert!(!app.done_unseen(1), "its question says more");
        app.world.inbox[0].agent = 2;
        assert!(app.done_unseen(1), "another's item is not its");
        app.world.inbox[0].agent = 1;
        app.world.inbox[0].open = false;
        assert!(app.done_unseen(1), "a closed item waits no more");
    }

    /// A demo team at its start whose inbox holds only `kinds`, one item
    /// each, oldest first, item `n` from agent `n + 1`.
    fn waiting(kinds: &[InboxKind]) -> App {
        let mut app = App::new(1, 4, 0, 1, Lang::En);
        app.world.inbox.clear();
        for (n, &kind) in kinds.iter().enumerate() {
            app.world.inbox.push(InboxItem {
                kind,
                agent: n + 1,
                detail: format!("item {n}"),
                researcher: None,
                open: true,
                tick: n as u64,
                card: None,
            });
        }
        app
    }

    /// Presses `c` and says which inbox item is open and selected.
    fn press(app: &mut App, c: char) -> Option<usize> {
        app.on_key(key(KeyCode::Char(c)));
        assert_eq!((app.view, app.pane), (View::Salon, Pane::Inbox));
        let at = app.waiting_at?;
        assert_eq!(
            app.zoom,
            Zoom::Focus(app.world.inbox[at].agent),
            "opened as enter opens it"
        );
        Some(at)
    }

    #[test]
    fn capital_w_opens_what_waits_one_after_the_other_from_anywhere() {
        let mut app = with_cards(1);
        app.world.inbox.clear();
        for n in 0..3 {
            app.world.inbox.push(InboxItem {
                kind: InboxKind::Sensitive,
                agent: n + 1,
                detail: format!("item {n}"),
                researcher: None,
                open: true,
                tick: n as u64,
                card: None,
            });
        }
        // A card's screen, the board, a conversation and a raw stream.
        app.open_card(0);
        app.view = View::Board;
        app.zoom = Zoom::Trace(0);
        app.thread = Some(Thread {
            entries: vec![0],
            sel: 0,
            scroll: Cell::new(0),
            follow: Cell::new(false),
            page: Cell::new(10),
        });
        assert_eq!(press(&mut app, 'W'), Some(0));
        assert!(app.card.is_none() && app.thread.is_none());
        assert_eq!(app.inbox_sel, 0);
        assert_eq!(press(&mut app, 'W'), Some(1));
        assert_eq!(app.inbox_sel, 1);
        assert_eq!(press(&mut app, 'W'), Some(2));
        assert_eq!(app.inbox_sel, 2);
        assert_eq!(press(&mut app, 'W'), Some(0), "round again");
        assert_eq!(app.inbox_sel, 0);
    }

    #[test]
    fn capital_s_puts_the_item_off_and_w_comes_to_it_after_the_rest() {
        let mut app = waiting(&[InboxKind::Sensitive; 4]);
        assert_eq!(press(&mut app, 'W'), Some(0));
        assert_eq!(press(&mut app, 'W'), Some(1));
        assert_eq!(press(&mut app, 'S'), Some(2), "1 put off, the next open");
        assert_eq!(app.deferred, [1]);
        assert_eq!(press(&mut app, 'W'), Some(3));
        assert_eq!(press(&mut app, 'W'), Some(1), "what was put off comes last");
        assert_eq!(press(&mut app, 'S'), Some(0), "put off again, round");
        assert_eq!(app.deferred, [1]);
        assert_eq!(press(&mut app, 'S'), Some(2));
        assert_eq!(app.deferred, [1, 0], "in the order they were put off");
        assert_eq!(press(&mut app, 'W'), Some(3));
        assert_eq!(press(&mut app, 'W'), Some(1), "not by age");
        assert_eq!(press(&mut app, 'W'), Some(0));
        assert_eq!(press(&mut app, 'W'), Some(2));
        // What was put off and closed since is forgotten.
        app.world.inbox[1].open = false;
        assert_eq!(press(&mut app, 'S'), Some(3));
        assert_eq!(app.deferred, [0, 2]);
    }

    #[test]
    fn capital_w_goes_on_after_what_it_opened_or_what_the_person_picked() {
        use InboxKind::{Reply, Sensitive};
        // A reply is read when opened: W goes on after it all the same.
        let mut app = waiting(&[Reply, Sensitive, Sensitive, Sensitive]);
        app.on_key(key(KeyCode::Char('W')));
        assert!(!app.world.inbox[0].open, "read");
        assert_eq!(app.waiting_at, Some(0));
        assert_eq!(press(&mut app, 'W'), Some(1));
        // Put off: nothing closed is put off.
        let mut app = waiting(&[Reply, Sensitive, Sensitive, Sensitive]);
        app.on_key(key(KeyCode::Char('W')));
        assert_eq!(press(&mut app, 'S'), Some(1));
        assert!(app.deferred.is_empty(), "{:?}", app.deferred);
        // The person picks another row by hand: W goes on from there.
        let mut app = waiting(&[Reply, Sensitive, Sensitive, Sensitive]);
        app.on_key(key(KeyCode::Char('W')));
        app.on_key(key(KeyCode::Char('j')));
        assert_eq!(app.waiting_at, None);
        assert_eq!(press(&mut app, 'W'), Some(3), "after the picked 2");
        let mut app = waiting(&[Reply, Sensitive, Sensitive, Sensitive]);
        app.on_key(key(KeyCode::Char('W')));
        app.on_key(key(KeyCode::Char('G')));
        assert_eq!(app.waiting_at, None);
        app.on_key(key(KeyCode::Char('g')));
        app.on_key(key(KeyCode::Char('g')));
        assert_eq!(press(&mut app, 'W'), Some(2), "after the picked 1");
        // The person looked elsewhere: W goes on after what it opened, not
        // after a selection the waiting panel no longer has in hand.
        let mut app = waiting(&[Sensitive; 4]);
        app.on_key(key(KeyCode::Char('W')));
        app.on_key(key(KeyCode::Char('W')));
        app.zoom = Zoom::None;
        app.pane = Pane::Journal;
        app.inbox_sel = 2;
        assert_eq!(press(&mut app, 'W'), Some(2), "after 1, not after 2");
        // Nothing at hand: the first.
        let mut app = waiting(&[Sensitive; 3]);
        app.inbox_sel = 2;
        assert_eq!(press(&mut app, 'W'), Some(0));
    }

    #[test]
    fn capital_w_with_nothing_waiting_says_so_and_stays() {
        let mut app = waiting(&[InboxKind::Sensitive]);
        app.world.inbox[0].open = false;
        app.view = View::Board;
        for c in ['W', 'S'] {
            app.on_key(key(KeyCode::Char(c)));
            assert_eq!(
                (app.view, app.pane, app.zoom),
                (View::Board, Pane::Salon, Zoom::None)
            );
            let said = app.flash.as_ref().map(|(m, _)| m.as_str());
            assert_eq!(said, Some(Lang::En.text().nothing_needed));
            app.flash = None;
        }
        assert!(app.deferred.is_empty());
    }

    fn with_cards(projects: usize) -> App {
        let mut app = App::new(projects, 4, 4, 7, Lang::En);
        for _ in 0..140 * projects {
            app.step();
        }
        assert!(app.world.cards.len() >= 4, "{}", app.world.cards.len());
        app
    }

    fn type_in(app: &mut App, text: &str) {
        for c in text.chars() {
            app.on_key(key(KeyCode::Char(c)));
        }
    }

    /// What `#` writes for the composer, the composer closed again.
    fn hash(app: &mut App) -> String {
        app.on_key(key(KeyCode::Char('#')));
        assert!(app.typing);
        app.typing = false;
        app.menu = None;
        std::mem::take(&mut app.input)
    }

    #[test]
    fn a_hash_writes_the_card_at_hand() {
        let mut app = with_cards(1);
        let w = &app.world;
        let (worker, card) = w
            .agents
            .iter()
            .find_map(|a| Some((a.id, a.card?)))
            .expect("a worker on a card");
        let mine = format!("#{}", card + 1);
        app.selected = Sel::Project(0);
        assert_eq!(hash(&mut app), "#");
        app.selected = Sel::Agent(worker);
        assert_eq!(hash(&mut app), mine);
        app.zoom = Zoom::Focus(worker);
        app.selected = Sel::Project(0);
        assert_eq!(hash(&mut app), mine, "the zoomed agent's");
        app.zoom = Zoom::None;
        // The card an inbox item came on, not the agent's now.
        app.pane = Pane::Inbox;
        let open = app.world.open_inbox();
        app.inbox_sel = 1;
        app.world.inbox[open[1]].card = Some(2);
        assert_eq!(hash(&mut app), "#3");
        app.world.inbox[open[1]].card = None;
        assert_eq!(hash(&mut app), "#");
        // A journal line that names a card, one that mentions it, one
        // that does neither.
        app.pane = Pane::Journal;
        app.world.note(Event::Posted { agent: 0, card: 3 });
        app.journal_sel = Some(app.world.journal.len() - 1);
        assert_eq!(hash(&mut app), "#4");
        let said = |text: &str| Event::Sent {
            from: 0,
            to: worker,
            kind: MsgKind::Inform,
            text: text.into(),
        };
        app.world.note(said("about #, #x, #99 and #2 then #3"));
        app.journal_sel = Some(app.world.journal.len() - 1);
        assert_eq!(hash(&mut app), "#2", "the first card it names");
        app.world.note(Event::Failed { agent: worker });
        app.journal_sel = Some(app.world.journal.len() - 1);
        assert_eq!(hash(&mut app), "#");
        // On a card's screen its own card is no news.
        app.open_card(3);
        app.journal_sel = app
            .world
            .journal
            .iter()
            .rposition(|e| e.event.card() == Some(3));
        assert_eq!(hash(&mut app), "#");
        app.journal_sel = Some(app.world.journal.len() - 2);
        assert_eq!(hash(&mut app), "#2");
    }

    #[test]
    fn enter_opens_the_card_typed_or_offered_at_once() {
        let mut app = with_cards(1);
        app.thread = Some(Thread {
            entries: vec![0],
            sel: 0,
            scroll: Cell::new(0),
            follow: Cell::new(false),
            page: Cell::new(10),
        });
        app.inbox_sel = 3;
        app.selected = Sel::Project(0);
        app.on_key(key(KeyCode::Char('#')));
        type_in(&mut app, "2");
        app.on_key(key(KeyCode::Enter));
        assert_eq!(app.card, Some(1));
        assert!(!app.typing && app.input.is_empty() && app.menu.is_none());
        assert!(app.thread.is_none());
        assert_eq!(
            (app.view, app.zoom, app.pane),
            (View::Salon, Zoom::None, Pane::Journal)
        );
        assert_eq!(app.inbox_sel, 0);
        let first = ui::card_rows(&app.world, 1)[0].0;
        assert_eq!(app.journal_sel, Some(first));
        let owner = app.world.cards[1].owner.expect("taken");
        assert_eq!(app.selected, Sel::Agent(owner));
        // A word of the title: the newest card that has it.
        let title = app.world.cards[0].title.clone();
        let word = title.split(' ').next().unwrap().to_lowercase();
        let newest = app
            .world
            .cards
            .iter()
            .rev()
            .find(|c| c.title.to_lowercase().contains(&word))
            .unwrap()
            .id;
        app.on_key(key(KeyCode::Char('#')));
        type_in(&mut app, &word);
        app.on_key(key(KeyCode::Enter));
        assert_eq!(app.card, Some(newest));
        // No such card: said, and the screen stays.
        app.on_key(key(KeyCode::Char('#')));
        type_in(&mut app, "x99");
        app.on_key(key(KeyCode::Enter));
        assert_eq!(app.card, Some(newest));
        assert_eq!(app.flash.take().unwrap().0, "no card #x99");
        // A name is put in by enter, not sent.
        app.on_key(key(KeyCode::Char('@')));
        app.on_key(key(KeyCode::Char('l')));
        app.on_key(key(KeyCode::Enter));
        assert!(app.typing, "{}", app.input);
    }

    #[test]
    fn a_card_nobody_took_selects_who_posted_it() {
        let mut app = with_cards(1);
        let c = app.world.cards.len();
        let mut card = app.world.cards[0].clone();
        card.id = c;
        card.owner = None;
        card.story = vec![Story {
            tick: 0,
            event: StoryEvent::Posted {
                by: 0,
                text: String::new(),
            },
        }];
        app.world.cards.push(card);
        app.selected = Sel::Project(0);
        app.open_card(c);
        assert_eq!(app.selected, Sel::Agent(0));
        // Nobody at all: the selection stays.
        app.world.cards[c].story.clear();
        app.selected = Sel::Project(0);
        app.open_card(c);
        assert_eq!(app.selected, Sel::Project(0));
    }

    #[test]
    fn a_card_s_screen_lists_its_own_waiting_items_and_journal_lines() {
        let mut app = with_cards(1);
        let first = app.world.open_inbox()[0];
        app.world.inbox[first].card = Some(0);
        app.open_card(0);
        assert_eq!(app.inbox_rows(), app.world.card_inbox(0));
        assert_eq!(
            app.inbox_rows().len(),
            2,
            "its change, and the one put on it"
        );
        assert_eq!(app.journal_rows(), ui::card_rows(&app.world, 0));
        app.on_key(key(KeyCode::Esc));
        assert_eq!(app.inbox_rows(), app.world.open_inbox());
        assert_eq!(app.journal_rows(), ui::journal_rows(&app.world));
    }

    #[test]
    fn esc_goes_back_to_the_card_then_to_the_salon() {
        let mut app = with_cards(1);
        let owner = app.world.cards[0].owner.unwrap();
        app.open_card(0);
        app.pane = Pane::Salon;
        app.on_key(key(KeyCode::Enter));
        assert_eq!(app.zoom, Zoom::Focus(owner));
        app.on_key(key(KeyCode::Enter));
        app.on_key(key(KeyCode::Esc));
        assert_eq!(app.zoom, Zoom::Focus(owner), "the raw stream, then focus");
        app.on_key(key(KeyCode::Esc));
        assert_eq!((app.zoom, app.card), (Zoom::None, Some(0)));
        app.pane = Pane::Inbox;
        app.inbox_sel = 1;
        app.on_key(key(KeyCode::Esc));
        assert_eq!(app.card, None);
        assert_eq!(app.inbox_sel, 0);
        app.on_key(key(KeyCode::Esc));
        assert_eq!((app.zoom, app.card), (Zoom::None, None), "nothing to leave");
        // Another view leaves it too.
        app.open_card(0);
        app.on_key(key(KeyCode::Char('2')));
        assert_eq!((app.view, app.card), (View::Board, None));
    }

    #[test]
    fn leaving_a_card_s_screen_keeps_the_journal_selection_on_a_shown_line() {
        let mut app = with_cards(1);
        // A claim is the card's line, not the whole journal's.
        app.world.note(Event::Claimed { agent: 1, card: 0 });
        app.open_card(0);
        let card_line = app.journal_sel.unwrap();
        assert_eq!(card_line, app.world.journal.len() - 1);
        app.on_key(key(KeyCode::Esc));
        let rows = ui::journal_rows(&app.world);
        let sel = app.journal_sel.expect("a line");
        assert!(rows.iter().any(|&(r, _)| r == sel), "{card_line} → {sel}");
    }

    #[test]
    fn n_and_p_step_through_the_open_projects_cards() {
        let mut app = with_cards(2);
        let of = |app: &App, p: usize| -> Vec<CardId> {
            app.world
                .cards
                .iter()
                .filter(|c| c.project == p)
                .map(|c| c.id)
                .collect()
        };
        assert!(of(&app, 1).len() >= 2);
        let all: Vec<CardId> = (0..app.world.cards.len()).collect();
        app.open_card(0);
        app.on_key(key(KeyCode::Char('p')));
        assert_eq!(app.card, Some(0), "no card before the first");
        for &c in &all[1..] {
            app.on_key(key(KeyCode::Char('n')));
            assert_eq!(app.card, Some(c));
        }
        app.on_key(key(KeyCode::Char('n')));
        assert_eq!(app.card, all.last().copied(), "none after the last");
        app.on_key(key(KeyCode::Char('p')));
        assert_eq!(app.card, Some(all[all.len() - 2]));
        // A closed project's cards are skipped.
        app.world.projects[1].closed = true;
        let open = of(&app, 0);
        app.open_card(open[0]);
        for &c in &open[1..] {
            app.on_key(key(KeyCode::Char('n')));
            assert_eq!(app.card, Some(c));
        }
        for &c in open[..open.len() - 1].iter().rev() {
            app.on_key(key(KeyCode::Char('p')));
            assert_eq!(app.card, Some(c));
        }
        // Not on a card's screen: n and p do nothing.
        app.zoom = Zoom::Focus(0);
        app.on_key(key(KeyCode::Char('n')));
        assert_eq!(app.card, Some(open[0]));
    }

    #[test]
    fn a_card_s_screen_does_not_fold_the_salon() {
        let mut app = with_cards(1);
        app.open_card(0);
        app.pane = Pane::Salon;
        app.selected = Sel::Project(0);
        for code in [KeyCode::Char('h'), KeyCode::Enter, KeyCode::Char('z')] {
            app.on_key(key(code));
        }
        app.on_key(key(KeyCode::Char('M')));
        assert_eq!(app.collapsed, [false]);
        assert!(app.folded_workers.is_empty());
        assert_eq!((app.zoom, app.card), (Zoom::None, Some(0)));
        // Back in the salon the keys fold again, in its own pane only.
        app.on_key(key(KeyCode::Esc));
        app.pane = Pane::Journal;
        app.on_key(key(KeyCode::Char('h')));
        assert_eq!(app.collapsed, [false]);
        app.pane = Pane::Salon;
        app.selected = Sel::Project(0);
        app.on_key(key(KeyCode::Enter));
        assert_eq!(app.collapsed, [true]);
    }

    #[test]
    fn closing_a_project_closes_its_card_s_screen() {
        let mut app = with_cards(2);
        let theirs = app.world.cards.iter().find(|c| c.project == 1).unwrap().id;
        let ours = app.world.cards.iter().find(|c| c.project == 0).unwrap().id;
        let other = app.world.projects[1].name.clone();
        app.open_card(theirs);
        app.command(&format!("close {other}"));
        assert_eq!(app.card, None);
        // Another project closing leaves it open.
        app.home = None;
        app.command("open ~/x more");
        app.open_card(ours);
        app.command("close more");
        assert_eq!(app.card, Some(ours));
    }
}
