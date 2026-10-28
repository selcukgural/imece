//! Screen state and keys: which view, how deep, what is selected, which
//! projects are folded, the language, and what the person is typing.

use std::cell::RefCell;
use std::time::{Duration, Instant};

use ratatui::crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

use crate::i18n::{Lang, fill};
use crate::live::{Live, Refusal};
use crate::model::*;
use crate::sim::{self, Sim};

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

/// How deep the person looks: the whole guild, one agent's card, or the
/// agent's raw stream.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Zoom {
    None,
    Focus(AgentId),
    Trace(AgentId),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Pane {
    Salon,
    Inbox,
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
    /// Position in the open inbox items.
    pub inbox_sel: usize,
    pub typing: bool,
    pub input: String,
    pub paused: bool,
    pub speed: u32,
    pub help: bool,
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
            inbox_sel: 0,
            typing: false,
            input: String::new(),
            paused: false,
            speed: 1,
            help: false,
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
        if let Zoom::Focus(id) | Zoom::Trace(id) = self.zoom {
            return self.world.agents[id].project;
        }
        if self.pane == Pane::Inbox
            && let Some(&i) = self.world.open_inbox().get(self.inbox_sel)
        {
            return self.world.agents[self.world.inbox[i].agent].project;
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

        match k.code {
            KeyCode::Char('q') => self.quit = true,
            KeyCode::Char('/') => self.typing = true,
            KeyCode::Char('@') => {
                self.typing = true;
                self.input = match self.zoom {
                    Zoom::Focus(id) | Zoom::Trace(id) => {
                        format!("@{} ", self.world.agents[id].name)
                    }
                    Zoom::None => "@".into(),
                };
            }
            KeyCode::Char('1') => self.show(View::Salon),
            KeyCode::Char('2') => self.show(View::Board),
            KeyCode::Char('3') => self.show(View::Code),
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
            KeyCode::Tab if self.view == View::Salon && self.zoom == Zoom::None => {
                self.pane = match self.pane {
                    Pane::Salon => Pane::Inbox,
                    Pane::Inbox => Pane::Salon,
                };
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

    /// Folds or unfolds the selected project; folding moves the selection to
    /// its header so it does not vanish.
    fn fold(&mut self, fold: bool) {
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
    /// a message that reached the agent is recorded as said.
    fn tell(&mut self, id: AgentId, text: &str) {
        let t = self.lang.text();
        let name = self.world.agents[id].name.clone();
        let refused = match &mut self.engine {
            Engine::Sim(s) => {
                s.told(&mut self.world, id, text);
                None
            }
            Engine::Live(l) => l.told(&mut self.world, id, text).err(),
        };
        match refused {
            None => {
                self.world.said(id, text);
                self.flash(fill(t.sent, &[&name]));
            }
            Some(Refusal::Budget) => self.flash(fill(t.budget_spent, &[&name])),
            Some(Refusal::Start(why)) => self.flash(fill(t.cannot_start, &[&name, &why])),
        }
    }

    fn move_selection(&mut self, down: bool) {
        match self.pane {
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
                (Pane::Inbox, _) => self
                    .world
                    .open_inbox()
                    .get(self.inbox_sel)
                    .map(|&i| Zoom::Focus(self.world.inbox[i].agent)),
                (Pane::Salon, Sel::Project(p)) => {
                    self.collapsed[p] = !self.collapsed[p];
                    None
                }
                (Pane::Salon, Sel::Agent(id)) => Some(Zoom::Focus(id)),
            },
            Zoom::Focus(id) | Zoom::Trace(id) => Some(Zoom::Trace(id)),
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
        if approve {
            self.world.approve(item);
            self.flash(t.approved);
        } else {
            self.world.reject(item);
            self.flash(t.rejected);
        }
        self.inbox_sel = self.inbox_sel.min(open.len().saturating_sub(2));
    }
}
