//! Screen state and keys: which view, how deep, what is selected, which
//! projects are folded, the language, and what the person is typing.

use std::cell::{Cell, RefCell};
use std::collections::{BTreeMap, BTreeSet};
use std::io;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use ratatui::crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

use crate::archives::{self, Browser};
use crate::away::Away;
use crate::complete::{self, Jump, Menu};
use crate::edit;
use crate::editor;
use crate::i18n::{Lang, clean, fill};
use crate::keys::{self, Keymap};
use crate::live::{self, Live, Open, Refusal, TidyRefusal, standing};
use crate::model::*;
use crate::motion::{self, Glow, GlowOn, Lit, Motion};
use crate::notify::{self, Note, Notify};
use crate::prefs::{SIDE_DEFAULT, SIDE_MAX, SIDE_MIN};
use crate::recall;
use crate::screen::Progress;
use crate::shell::{Caught, Output, Ran};
use crate::sim::{self, Sim};
use crate::theme;
use crate::ui;
use crate::welcome::{self, LISTED, Seen};
use imece_agent::{Choice, Login};
use std::sync::mpsc::TryRecvError;

/// What drives the agents.
pub enum Engine {
    Sim(Sim),
    Live(Box<Live>),
    /// Nothing: an archive's read-only view (`enter` in the archives
    /// browser), a world of its own that never moves. Everything that
    /// would act on an agent, a project or the store refuses
    /// ([`App::read_only`]).
    View(Viewed),
}

/// The `:tidy` window: the row at hand, and whether a `y` is asked for
/// removing it with what it holds.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct TidyWin {
    pub at: usize,
    pub sure: bool,
}

/// A teardown script a removal waits on the person's answer to: its
/// bytes, how far down they are scrolled, and the removal asked (its
/// rows of the list, with work or not).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TeardownAsk {
    pub script: crate::live::setup::Script,
    pub scroll: usize,
    pub at: Vec<usize>,
    pub force: bool,
}

/// The archive a read-only view shows.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Viewed {
    pub project: ProjectId,
    pub title: String,
    /// When it was archived (a tick).
    pub at: u64,
}

/// The `:` commands an archive's view runs: they only look, go or change
/// how the screen looks. Every other is refused there (deny by default:
/// a new command is refused in the view until it is listed here);
/// `:restore` asks the screen behind to bring the archive back.
const VIEW_COMMANDS: &[&str] = &[
    "goto", "history", "export", "salon", "board", "code", "timeline", "lang", "theme", "help",
    "quit", "next", "defer", "waiting", "needs", "fold", "unfold", "trace", "peek", "hints",
    "news", "settle", "unsettle", "talk", "why", "story",
];

/// Whether a line sent is one `↑` and `^r` bring back: a message, a
/// command or a shell line; not the lines that only go somewhere (`#12`,
/// `&3`, `:goto`, `:history`, `:recall`).
fn recalled(text: &str) -> bool {
    let word = text
        .strip_prefix(':')
        .map(|r| r.split_whitespace().next().unwrap_or_default());
    !text.starts_with('#')
        && !text.starts_with('&')
        && !matches!(word, Some("goto" | "history" | "recall"))
}
/// While the terminal does not have the focus, what changed is drawn at
/// most this often unless the person did something: ten times less often
/// than live's tick.
pub const BLURRED_DRAW: Duration = Duration::from_secs(1);

/// How many notifications sent are remembered for a click on one: more
/// than can pile up in a desktop's list in a sitting away.
/// How many lines `ctrl-d`/`ctrl-u` (page keys) move a setup script's
/// window.
pub const SCRIPT_PAGE: usize = 10;

/// A push that ended: the agent it was about, its words, and whether it
/// went (else why not).
type Pushed = (AgentId, String, Result<(), String>);

pub const NOTICED: usize = 16;

/// What the screen shows of time: the world's clock, the flash on show
/// (by when it was said) and the seconds a `:close` or `:forget` still
/// waits. A drawing is due when it moved.
type Clock = (u64, Option<Instant>, Option<u64>);
/// How long `:close` and `:forget` wait before they act; `u` (`:undo`)
/// meanwhile leaves things as they are.
pub const UNDO_GRACE: Duration = Duration::from_secs(10);

/// A `:close` or `:forget` asked for and not done yet.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Undoable {
    Close(ProjectId),
    Forget(AgentId),
    /// The title waits in [`App::archive_title`].
    Archive(ProjectId),
    Clear(Clear),
}

/// What `:clear` takes off the salon's screen (record 90).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Clear {
    /// WAITING FOR YOU and the journal.
    All,
    Inbox,
    Journal,
}

impl Clear {
    /// Nothing for both, `inbox` or `journal`.
    pub fn parse(s: &str) -> Option<Clear> {
        match s.trim().to_lowercase().as_str() {
            "" => Some(Clear::All),
            "inbox" => Some(Clear::Inbox),
            "journal" => Some(Clear::Journal),
            _ => None,
        }
    }

    fn inbox(self) -> bool {
        self != Clear::Journal
    }

    fn journal(self) -> bool {
        self != Clear::Inbox
    }

    /// Its place in the words' lists: both, the inbox, the journal.
    pub fn index(self) -> usize {
        match self {
            Clear::All => 0,
            Clear::Inbox => 1,
            Clear::Journal => 2,
        }
    }
}

/// What `:clear` and `:unclear` take: the words their menus offer.
pub const CLEARS: [&str; 2] = ["inbox", "journal"];

/// The most of a title `:archive` keeps, in characters.
pub const ARCHIVE_TITLE_MAX: usize = 60;

/// How long a short notice stays in the bottom line.
pub const FLASH: Duration = Duration::from_secs(3);

/// What moves on the screen by itself (record 84).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Moves {
    /// A working row's glyph.
    Breath,
    /// The dots after `starting`.
    Dots,
    /// The spinner where imece waits for an answer.
    Spin,
    /// The spinner by the header's fleet (full motion).
    HeadSpin,
    /// imece's mark at the start, a dot a beat (`App::mark_dots`).
    Mark,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum View {
    Salon,
    Board,
    Code,
    /// One lane per agent over time (record 100).
    Timeline,
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
    /// Lines being chosen to copy (`v`), over `ui::thread_lines`.
    pub copy: Option<Copying>,
    /// The choice word by word (`w b h l`): the word of the anchor's line
    /// it starts from and the cursor's word, each counted in its own line
    /// (`ui::CopyLine::words`). None: whole lines.
    pub words: Option<(usize, usize)>,
    /// Which messages show as one row.
    pub folds: Folds,
    /// How wide its text was last drawn, which its lines are as copy mode
    /// moves over them (a table wraps to it); none drawn yet, unbounded.
    pub room: Cell<usize>,
}

impl Thread {
    /// The words chosen on line `i` (of `ui::thread_lines`): the first and
    /// the last, counted in the line; the last may lie past the line's
    /// end, which means to its end. None: nothing of it is chosen.
    pub fn part(&self, i: usize) -> Option<(usize, usize)> {
        let c = self.copy?;
        let (from, to) = match self.words {
            Some((aw, cw)) => {
                let (a, b) = ((c.anchor, aw), (c.at, cw));
                (a.min(b), a.max(b))
            }
            None => {
                let (f, t) = c.span();
                ((f, 0), (t, usize::MAX))
            }
        };
        if i < from.0 || i > to.0 {
            return None;
        }
        let first = if i == from.0 { from.1 } else { 0 };
        let last = if i == to.0 { to.1 } else { usize::MAX };
        Some((first, last))
    }
}

/// Which messages of a conversation are folded to one row: all of them
/// or none (`zM` `zR`), less or more the ones folded or opened one by one
/// since (`h l za`), by their place in the conversation, so a message
/// that comes later folds as the rest do.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Folds {
    pub all: bool,
    pub except: BTreeSet<usize>,
}

impl Folds {
    /// All folded (`all`) or all open.
    pub fn every(all: bool) -> Self {
        Folds {
            all,
            except: BTreeSet::new(),
        }
    }

    /// Whether message `n` shows as one row.
    pub fn folded(&self, n: usize) -> bool {
        self.all != self.except.contains(&n)
    }

    /// Message `n` folded (`fold`) or open.
    pub fn set(&mut self, n: usize, fold: bool) {
        if fold == self.all {
            self.except.remove(&n);
        } else {
            self.except.insert(n);
        }
    }
}

/// Where the person reads an agent up close (record 102): the story
/// entry at hand on its focus and how far down the focus and its raw
/// stream are scrolled, which of the focus's entries are cut, for the
/// agent of `agent`; another agent starts afresh.
#[derive(Debug)]
pub struct Reading {
    pub agent: Option<AgentId>,
    /// The focus's story entry at hand; none: the newest, followed as
    /// more come (`G`).
    pub sel: Option<usize>,
    /// The focus's first row shown, as last drawn.
    pub scroll: Cell<usize>,
    /// The entry at hand moved: the next drawing scrolls to it.
    pub follow: Cell<bool>,
    /// Rows the focus last had room for, for paging.
    pub page: Cell<usize>,
    /// Which entries are cut to their first lines (`ui::focus::STORY_ROWS`):
    /// all at first, `l` opens one whole.
    pub folds: Folds,
    /// The raw stream's first row shown; none: the newest in view.
    pub trace_top: Option<usize>,
    /// Its first row shown, as last drawn.
    pub trace_scroll: Cell<usize>,
    /// Its rows below which nothing more comes, as last drawn: the first
    /// row of its last page.
    pub trace_last: Cell<usize>,
    /// Rows it last had room for.
    pub trace_page: Cell<usize>,
}

impl Default for Reading {
    fn default() -> Self {
        Reading {
            agent: None,
            sel: None,
            scroll: Cell::new(0),
            follow: Cell::new(false),
            page: Cell::new(10),
            folds: Folds::every(true),
            trace_top: None,
            trace_scroll: Cell::new(0),
            trace_last: Cell::new(0),
            trace_page: Cell::new(10),
        }
    }
}

impl Reading {
    /// The focus's entry at hand among `entries`.
    pub fn at(&self, entries: usize) -> usize {
        let last = entries.saturating_sub(1);
        self.sel.map_or(last, |s| s.min(last))
    }

    /// The focus's first row shown of `rows` in `height`, the entry at
    /// hand's rows `at`: the last page while the newest is followed, the
    /// entry kept in view after it moved, else where it was.
    pub fn focus_top(&self, rows: usize, height: usize, at: (usize, usize)) -> usize {
        let last = rows.saturating_sub(height);
        let top = match (self.sel, self.follow.take()) {
            (None, _) => last,
            (Some(_), true) => ui::in_view(at.0, at.1, self.scroll.get(), height),
            (Some(_), false) => self.scroll.get(),
        };
        let top = top.min(last);
        self.scroll.set(top);
        self.page.set(height.saturating_sub(1).max(1));
        top
    }

    /// The raw stream's first row shown of `rows` in `height`: the last
    /// page while the newest is followed, else where it was put.
    pub fn trace_top(&self, rows: usize, height: usize) -> usize {
        let last = rows.saturating_sub(height);
        let top = self.trace_top.unwrap_or(last).min(last);
        self.trace_scroll.set(top);
        self.trace_last.set(last);
        self.trace_page.set(height.saturating_sub(1).max(1));
        top
    }

    /// The raw stream moved `by` rows down (or up): at its last page or
    /// past it, the newest followed again.
    pub fn trace_by(&mut self, by: isize) {
        let at = self.trace_scroll.get().saturating_add_signed(by);
        self.trace_top = (at < self.trace_last.get()).then_some(at);
    }
}

/// The person's note on words of a conversation (`c` while choosing),
/// kept until it is sent with the others (`n`): the agent it goes to,
/// the journal line the words start in, the words as the screen showed
/// them, the note.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TalkNote {
    pub to: AgentId,
    pub entry: usize,
    pub quote: String,
    pub text: String,
}

/// Where the person types, so where the terminal's cursor stands
/// ([`App::caret`]).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Field {
    /// The command line at the foot.
    Line,
    /// The help's `/` filter.
    Help,
    /// The archives' `/` filter.
    Archives,
}

/// What is typed, which the cursor's shape says: a message or a `!`
/// command, a `:` command to imece, or words that find something (a
/// filter, `^r`, `#`, `&`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Caret {
    Write,
    Command,
    Find,
}

/// What the note being written goes on: a line of worker `.0`'s change,
/// or words of a conversation (a [`TalkNote`] with no text yet).
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Noting {
    Line(AgentId, usize),
    Words(TalkNote),
}

/// Lines being chosen to copy (`v`): the line the choice started on and
/// the line the cursor is on, of a conversation (`ui::thread_lines`) or
/// a change's patch.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Copying {
    pub anchor: usize,
    pub at: usize,
}

impl Copying {
    /// A choice of one line, `at`.
    pub fn new(at: usize) -> Self {
        Copying { anchor: at, at }
    }

    /// The first and the last line chosen.
    pub fn span(self) -> (usize, usize) {
        (self.anchor.min(self.at), self.anchor.max(self.at))
    }

    /// How many lines are chosen.
    pub fn len(self) -> usize {
        let (from, to) = self.span();
        to - from + 1
    }

    /// Whether line `i` is chosen.
    pub fn holds(self, i: usize) -> bool {
        let (from, to) = self.span();
        from <= i && i <= to
    }
}

/// The person's note on a line of a worker's change (`c` in the diff),
/// kept until it is sent with the others (`n`) or the change goes.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DiffNote {
    pub agent: AgentId,
    /// The patch it was written on, by its hash: a newer change from the
    /// same worker has other lines.
    pub patch: u64,
    pub line: usize,
    pub text: String,
}

/// What a note is kept against: the patch's hash.
pub fn patch_key(patch: &str) -> u64 {
    use std::hash::{DefaultHasher, Hash, Hasher};
    let mut h = DefaultHasher::new();
    patch.hash(&mut h);
    h.finish()
}

/// Where the person is: the view, the zoom, the salon's panel and the
/// card whose screen is open. `^o` goes back through the places left.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Place {
    pub view: View,
    pub zoom: Zoom,
    pub pane: Pane,
    pub card: Option<CardId>,
}

/// The most places `^o` remembers.
const PLACES: usize = 50;

/// A selectable salon line: a project's header or an agent.
/// Which part of a solo's choice a command sets.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Part {
    Effort,
    Model,
}

/// The screen a bare `:solo` opens: the vendor, then the model (the
/// vendor's own or an alias), then the effort (the vendor's own or one of
/// its levels; skipped for a vendor without any). The path and name typed
/// with `:solo` wait here for the choice.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SoloPick {
    pub path: Option<String>,
    pub name: Option<String>,
    pub vendors: Vec<Vendor>,
    pub step: PickStep,
    /// The highlighted row of each step.
    pub vendor: usize,
    pub model: usize,
    pub effort: usize,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PickStep {
    Vendor,
    Model,
    Effort,
}

impl SoloPick {
    pub fn new(path: Option<String>, name: Option<String>, vendors: Vec<Vendor>) -> SoloPick {
        SoloPick {
            path,
            name,
            vendors,
            step: PickStep::Vendor,
            vendor: 0,
            model: 0,
            effort: 0,
        }
    }

    /// The vendor highlighted.
    pub fn chosen_vendor(&self) -> Vendor {
        self.vendors[self.vendor.min(self.vendors.len().saturating_sub(1))]
    }

    /// The rows of the step at hand: the vendors; `default` and the
    /// vendor's model aliases; `default` and the vendor's efforts.
    pub fn items(&self) -> Vec<String> {
        let own = || vec![complete::DEFAULT.to_string()];
        match self.step {
            PickStep::Vendor => self.vendors.iter().map(|v| v.label().to_string()).collect(),
            PickStep::Model => {
                let mut items = own();
                items.extend(
                    live::aliases(self.chosen_vendor())
                        .iter()
                        .map(|a| a.to_string()),
                );
                items
            }
            PickStep::Effort => {
                let mut items = own();
                items.extend(
                    live::efforts(self.chosen_vendor())
                        .iter()
                        .map(|e| e.to_string()),
                );
                items
            }
        }
    }

    /// The highlighted row of the step at hand.
    pub fn sel(&self) -> usize {
        match self.step {
            PickStep::Vendor => self.vendor,
            PickStep::Model => self.model,
            PickStep::Effort => self.effort,
        }
    }

    fn sel_mut(&mut self) -> &mut usize {
        match self.step {
            PickStep::Vendor => &mut self.vendor,
            PickStep::Model => &mut self.model,
            PickStep::Effort => &mut self.effort,
        }
    }

    /// The choice picked so far, as `:solo` would take it: none for the
    /// vendor's own model and effort.
    pub fn choice(&self) -> Option<String> {
        let v = self.chosen_vendor();
        let pick = |items: Vec<&str>, i: usize| items.get(i).copied().map(str::to_string);
        let c = Choice {
            model: pick(live::aliases(v), self.model.saturating_sub(1)).filter(|_| self.model > 0),
            effort: pick(live::efforts(v).to_vec(), self.effort.saturating_sub(1))
                .filter(|_| self.effort > 0),
        };
        Some(c.wire()).filter(|w| !w.is_empty())
    }

    /// A key: `Some(Some(..))` once the last step is answered (the vendor
    /// and the choice), `Some(None)` when the screen is given up.
    pub fn on_key(&mut self, code: KeyCode) -> Option<Option<(Vendor, Option<String>)>> {
        let last = self.items().len().saturating_sub(1);
        match code {
            KeyCode::Up | KeyCode::Char('k') => *self.sel_mut() = self.sel().saturating_sub(1),
            KeyCode::Down | KeyCode::Char('j') => *self.sel_mut() = (self.sel() + 1).min(last),
            KeyCode::Char('g') | KeyCode::Home => *self.sel_mut() = 0,
            KeyCode::Char('G') | KeyCode::End => *self.sel_mut() = last,
            KeyCode::Char(c @ '1'..='9') => {
                let n = usize::from(u8::try_from(c).unwrap_or(b'1') - b'1');
                if n <= last {
                    *self.sel_mut() = n;
                }
            }
            KeyCode::Enter => match self.step {
                PickStep::Vendor => {
                    // Another vendor: its own models and efforts.
                    self.model = 0;
                    self.effort = 0;
                    self.step = PickStep::Model;
                }
                PickStep::Model if live::efforts(self.chosen_vendor()).is_empty() => {
                    return Some(Some((self.chosen_vendor(), self.choice())));
                }
                PickStep::Model => self.step = PickStep::Effort,
                PickStep::Effort => return Some(Some((self.chosen_vendor(), self.choice()))),
            },
            KeyCode::Esc => match self.step {
                PickStep::Vendor => return Some(None),
                PickStep::Model => self.step = PickStep::Vendor,
                PickStep::Effort => self.step = PickStep::Model,
            },
            KeyCode::Char('q') => return Some(None),
            _ => {}
        }
        None
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Sel {
    Project(ProjectId),
    Agent(AgentId),
}

/// A row the hint letters (`F`) can name: a salon row (a project's
/// header, an agent, also the people on a card's screen), an item of
/// WAITING FOR YOU (its index in `World::inbox`) or a journal line (its
/// index in `World::journal`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Target {
    Row(Sel),
    Waiting(usize),
    Line(usize),
}

/// The hint letters on the screen: each row's label, given as `F`
/// opened them and kept while they are open (the drawing puts a label
/// where its row is now), and the letters typed so far.
pub struct Hints {
    pub labels: Vec<(String, Target)>,
    pub typed: String,
}

/// The letters labels are made of, the home row first.
pub const HINT_LETTERS: &[u8; 26] = b"asdfghjklqwertyuiopzxcvbnm";

/// Labels for `n` rows (at most 26 × 26), the first rows first: as many
/// of one letter as there can be, and two letters from the last letters
/// on, as few of those as hold the rest; a letter that starts a label of
/// two is never a label itself, so no label starts another.
pub fn hint_labels(n: usize) -> Vec<String> {
    let letter = |i: usize| char::from(HINT_LETTERS[i]);
    let k = HINT_LETTERS.len();
    let n = n.min(k * k);
    // Each letter turned into a start of two gives k labels for its one.
    let starts = n.saturating_sub(k).div_ceil(k - 1);
    let ones = (0..k - starts).map(|i| letter(i).to_string());
    let twos =
        (k - starts..k).flat_map(|a| (0..k).map(move |b| [letter(a), letter(b)].iter().collect()));
    ones.chain(twos).take(n).collect()
}

/// How much an agent needs the person, least first: the salon puts the
/// workers that need more first and `f` shows only those that need
/// something. The top of the [`Rung`] ladder, the calm rungs folded into
/// `Nothing` so that an agent going from idle to working and back does
/// not move on the screen.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum Need {
    Nothing,
    /// Finished a turn the person has not looked at.
    Done,
    /// Something of it waits in WAITING FOR YOU.
    Waiting,
    /// Blocked, or its session stopped: it goes nowhere until the person
    /// writes to it.
    Stuck,
}

/// An agent's state as one rung of a ladder, lowest first: stuck > waiting
/// for the person > working > starting > done, not seen > idle. Every rung
/// has a glyph of its own shape, so the mono theme tells them apart, and a
/// folded group (a project, a worker's helpers, the calm workers folded
/// away) shows its loudest member: the one on the highest rung, the first
/// in order among equals.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum Rung {
    /// Idle, asleep, or gone.
    Idle,
    /// Finished a turn the person has not looked at.
    Done,
    /// Its session is being started.
    Starting,
    /// Mid-turn, asking a teammate, researching.
    Working,
    /// Something of it waits in WAITING FOR YOU.
    Waiting,
    /// Blocked, or its session stopped.
    Stuck,
}

/// What agent `a` is doing for its lane on the timeline (record 100),
/// `waits` whether something of it waits for the person: stuck; in a turn,
/// what it does there (waiting for a yes to a tool call, starting,
/// writing files, reading or running something, thinking, asking a
/// teammate, researching), even while an older item of it waits; out of
/// one, waiting for the person when something does, else the tick its turn
/// ended (its lane says it was busy) or idle.
pub(crate) fn cell_of(w: &World, a: &Agent, waits: bool) -> LaneCell {
    match a.status {
        Status::Blocked | Status::Stopped => LaneCell::Stuck,
        Status::WaitingHuman => LaneCell::Waiting,
        Status::Working if w.open_item(a.id, InboxKind::Approval).is_some() => LaneCell::Waiting,
        Status::Working if a.starting => LaneCell::Starting,
        Status::Working => match a.activity.as_deref() {
            None | Some("") => LaneCell::Thinking,
            Some(doing) if writes(doing) => LaneCell::Writing,
            Some(_) => LaneCell::Reading,
        },
        Status::Asking { .. } => LaneCell::Asking,
        Status::Researching { .. } => LaneCell::Reading,
        Status::Idle | Status::Asleep if waits => LaneCell::Waiting,
        Status::Idle | Status::Asleep if a.lane.last().busy() => LaneCell::Done,
        Status::Idle | Status::Asleep => LaneCell::Idle,
    }
}

/// Whether a tool call, as a row says it (`Edit src/x.rs`), writes files.
pub(crate) fn writes(doing: &str) -> bool {
    matches!(
        doing.split_whitespace().next(),
        Some("Edit" | "Write" | "MultiEdit" | "NotebookEdit" | imece_agent::codex::PATCH)
    )
}

impl Rung {
    /// What the rung needs of the person.
    pub fn need(self) -> Need {
        match self {
            Rung::Stuck => Need::Stuck,
            Rung::Waiting => Need::Waiting,
            Rung::Done => Need::Done,
            Rung::Working | Rung::Starting | Rung::Idle => Need::Nothing,
        }
    }
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
    /// The side panel tells the story in the journal's place (`J`,
    /// `:story`; record 99). Off in `App::new`; `main` sets the saved one,
    /// on by default.
    pub story: bool,
    /// The journal shows in the story's place for now: `enter` on a line
    /// of no card went to it there; `esc` or `J` goes back to the story.
    /// Not kept, so the story stays the side panel's.
    pub story_away: bool,
    /// The selected item of the story, among those it shows.
    pub story_sel: usize,
    /// What the story shows that can be selected, as last drawn.
    pub story_items: RefCell<Vec<ui::story::Item>>,
    /// The timeline's selected agent, among those it shows, and its scale
    /// (an index into `ui::timeline::SCALES`).
    pub timeline_sel: usize,
    pub timeline_scale: usize,
    /// Agents that finished a turn since the person last opened them.
    pub unseen: BTreeSet<AgentId>,
    /// Agents (`s` on a worker's row) and projects (on a header or its
    /// leader) put out of the salon until they need the person again:
    /// something of theirs waits, they are stuck, or they finished a turn
    /// not seen (`wake_settled`). Kept for this run only.
    pub settled: BTreeSet<AgentId>,
    pub settled_projects: BTreeSet<ProjectId>,
    /// The agent whose focus is open in a window over the view (`K`).
    pub peek: Option<AgentId>,
    /// The worktree setup script an ask is about, in a window over the
    /// screen (`enter` on the ask, record 92): its inbox item and how far
    /// down it is scrolled.
    pub script: Option<(usize, usize)>,
    /// The worktrees no agent owns, in a window over the screen (`:tidy`,
    /// record 96).
    pub tidy: Option<TidyWin>,
    /// A checkout's teardown script a removal asked about, in a window
    /// over the `:tidy` one.
    pub teardown: Option<TeardownAsk>,
    /// The start's look for worktrees no agent owns, until it answered
    /// and its count was said.
    tidy_start: bool,
    /// The hint letters, while they are on the screen (`F`).
    pub hints: Option<Hints>,
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
    /// The first patch line the open change shows: the line at hand,
    /// which `c` writes a note on and `>` quotes the hunk of.
    pub diff_scroll: usize,
    /// Where the person reads the agent of the focus or the raw stream.
    pub reading: Reading,
    /// Whether a change's long lines wrap (`w`) instead of being cut.
    pub diff_wrap: bool,
    /// The person's notes on lines of workers' changes, not sent yet.
    pub diff_notes: Vec<DiffNote>,
    /// The note being written: on a line of a change, or on words of a
    /// conversation.
    pub noting: Option<Noting>,
    /// The person's notes on words of conversations, not sent yet.
    pub talk_notes: Vec<TalkNote>,
    /// Whether a conversation opens with its messages folded: the last
    /// `zM` or `zR` (`:fold` `:unfold` in one) says, for this run.
    pub fold_talk: bool,
    /// Hunks the person chose to apply alone (`x`): whose change, its
    /// patch's hash, the hunk's `@@` line.
    pub chosen: Vec<(AgentId, u64, usize)>,
    /// Lines of a change being chosen to copy (`v`): whose change, its
    /// patch's hash, the choice.
    pub diff_copy: Option<(AgentId, u64, Copying)>,
    /// The files of a change folded to their headers (`h l za zM zR`):
    /// whose change, by its patch, and which of its files.
    pub diff_folds: Option<(AgentId, u64, Folds)>,
    /// Rows the last drawing of a change had, for paging.
    pub diff_page: Cell<u16>,
    /// The selected journal line (a journal index).
    pub journal_sel: Option<usize>,
    /// The last `!` command's output in each project, for `O`; in memory
    /// only, gone when imece ends.
    pub outputs: BTreeMap<ProjectId, Output>,
    pub thread: Option<Thread>,
    /// The card whose screen takes the salon's place (`#323`): its story
    /// and who had a part in it, what waits for the person on it and its
    /// journal lines.
    pub card: Option<CardId>,
    /// The places left, oldest first, for `^o`.
    pub been: Vec<Place>,
    /// Where `&` may go, frozen as the line opened (what needs the person
    /// first), so the numbers hold while the person types; empty when the
    /// line is not open.
    pub jumps: Vec<Jump>,
    /// `↑` on the command line brought a sent line back: which (its place
    /// in `World::sent`), and what was being written before it, to come
    /// back to past the newest; none while nothing is brought back.
    recall: Option<(usize, String)>,
    /// `^r`: the menu lists the lines sent that have the words typed.
    pub searching: bool,
    /// `H`, `:recall`: while searching, the menu lists the journal and
    /// trace lines of every project and archive instead (record 93).
    pub recalling: bool,
    /// The last key was `^o`: the place it left is not one to go back to.
    went_back: bool,
    /// The side panel's width, in percent.
    pub side: u16,
    /// Text the person asked to copy; the main loop hands it to the
    /// clipboard.
    pub copy: Option<String>,
    /// A command the person typed after `!`; the main loop steps the
    /// screen aside and runs it in their shell, then calls `ran`.
    pub shell: Option<String>,
    /// The draft `^x ^e` or `:edit` opens in the person's editor; the main
    /// loop steps the screen aside, runs it and calls `edited`.
    pub editor: Option<String>,
    /// The last key while writing was `^x`: a `^e` now opens the editor.
    ctrl_x: bool,
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
    /// The first of the help's lines in sight when they do not all fit
    /// (`j k ^d ^u pgup pgdn gg G` in it), and what the last drawing of
    /// it left room for: lines in sight and the first line at most.
    pub help_scroll: usize,
    pub help_room: Cell<(usize, usize)>,
    /// The person's own keys (`keys.rs`): the key file's, else imece's.
    pub keys: Keymap,
    /// Where the key file is; none without a home.
    pub keys_file: Option<PathBuf>,
    /// The text of the good key file in use, kept for the next start; and
    /// whether it changed since it was last kept.
    pub keys_good: Option<String>,
    pub keys_new: bool,
    /// The key file as it was last read (`keys::stamp`), and when it was
    /// last looked at for a change; none looks at once.
    keys_seen: keys::Stamp,
    keys_looked: Option<Instant>,
    /// The first of two keys the key file gives (`gt = G`), waiting for
    /// the second since then; at most [`keys::WAIT`].
    pub keys_held: Option<(KeyEvent, Instant)>,
    /// A bare `:solo` asks the vendor, the model and the effort on a
    /// screen of its own; the words it came with wait in it.
    pub solo_pick: Option<SoloPick>,
    /// The archives browser (`A`, `:archives`), while it is open.
    pub archives: Option<Browser>,
    /// An archive's read-only view, while it is open: a screen of its own
    /// over a world of its own ([`Engine::View`]) that keys and drawing go
    /// to; this one goes on stepping behind it.
    pub viewing: Option<Box<App>>,
    /// In a view: `:restore` asked the screen behind to bring the archive
    /// back.
    pub restore_asked: bool,
    /// The vendor that leads from the next start on without being asked
    /// (`claude`), as the preferences keep it; none asks when there is a
    /// choice.
    pub primary: Option<String>,
    pub digest: Option<Digest>,
    /// What an earlier run left behind, shown once at the start.
    pub left_off: Option<LeftOff>,
    /// The first-run window is open (`:welcome`).
    pub welcome: bool,
    /// What the window lists of each vendor's CLI, in [`LISTED`]'s order.
    pub vendors: Vec<(Vendor, Seen)>,
    /// How a live run asks the vendors' CLIs ([`live::look`]); none in the
    /// demo, whose vendors are its own.
    pub look: Option<welcome::Look>,
    /// The answers still to come.
    looking: Option<welcome::Answers>,
    /// The leader's vendor, while a start waits to hear whether it is
    /// signed in: not opens the window.
    lead_check: Option<Vendor>,
    /// The versions whose news are open, newest first; none: closed
    /// (`:news`).
    pub news: Vec<&'static str>,
    /// What quitting now would leave behind, open over the screen (`q s`).
    pub leaving: bool,
    /// The theme in use while another is previewed (`:theme NAME` being
    /// written): put back unless the line is sent.
    pub theme_was: Option<String>,
    pub flash: Option<(String, Instant)>,
    /// A `:close` or `:forget` waiting out its [`UNDO_GRACE`], and when
    /// it acts.
    pub grace: Option<(Undoable, Instant)>,
    /// The title the `:archive` waiting gives its project.
    pub archive_title: String,
    pub quit: bool,
    /// The store's last failure, flashed once.
    store_error: Option<String>,
    /// Salon lines in the order they were shown last, for moving the
    /// selection.
    pub shown: RefCell<Vec<Sel>>,
    /// The rows the last drawing showed that hint letters can name, in
    /// the order drawn, and where each starts on the screen (column, row).
    pub hintable: RefCell<Vec<(Target, u16, u16)>>,
    /// Where the last drawing put the glyphs that breathe, every one of
    /// them, so that a frame where only the breath changed can be drawn
    /// from that drawing ([`App::patch_due`], `ui::patch`); none when
    /// that drawing cannot be patched (a window over it, a glyph cut).
    pub breathing: RefCell<Option<Vec<ui::Glyph>>>,
    /// Where the last drawing put the terminal's own cursor.
    pub caret_at: Cell<Option<ratatui::layout::Position>>,
    last_key: Instant,
    last_key_tick: u64,
    /// When the terminal lost the focus, and the tick then.
    blurred: Option<(Instant, u64)>,
    /// The tick the person last left the window at, else the run's first:
    /// where `:digest` starts.
    left_tick: u64,
    /// How long away opens "while you were away" by itself (`:away`).
    pub away: Away,
    /// When the screen was last drawn and what its clock showed then;
    /// whether something came from the terminal since (a key, a paste, a
    /// resize, the focus), and whether something else the screen shows
    /// changed (an engine's step that brought news, a vendor looked at,
    /// a held key let through).
    drawn_at: Option<Instant>,
    drawn_clock: Option<Clock>,
    pub stirred: bool,
    pub changed: bool,
    /// The hold on the machine's sleep while an agent is mid-turn
    /// (`--awake`, `:awake`).
    pub awake: crate::awake::Awake,
    /// How the person is told, in another window, that something waits
    /// (`:notify`), and what rang already.
    pub notify: Notify,
    notices: notify::Notices,
    /// The notifications sent, newest last, each with its number and what
    /// it was about, so kitty's report of a click opens that
    /// ([`App::clicked`]); the last [`NOTICED`]. The number the next one
    /// gets.
    noticed: Vec<(u64, Vec<Note>)>,
    notice_count: u64,
    /// Where `:notify push` sends, as read from the person's `push` file
    /// (none in the demo, which only shows its pushes), and the pushes on
    /// their way: each is sent on a thread of its own and says here how it
    /// went ([`App::pushed`]).
    pub push: Option<crate::push::Push>,
    /// Where the push settings are read from (`push::file`).
    pub push_file: Option<PathBuf>,
    pushes: (
        std::sync::mpsc::Sender<Pushed>,
        std::sync::mpsc::Receiver<Pushed>,
    ),
    /// The last push failed and that was said: the next failures are
    /// not, until one goes.
    push_failing: bool,
    /// How much moves on the screen (`:motion`), whether imece runs over
    /// SSH (full is calm there), and when motion's frames count from.
    pub motion: Motion,
    pub ssh: bool,
    epoch: Instant,
    /// The moment the frame being drawn shows: what moves is drawn as it
    /// is then.
    pub frame: Instant,
    /// The rows lit for a moment (newest last), each with when it lit; the
    /// rungs and the inbox's length the last step saw, to tell what is
    /// new.
    glows: Vec<(GlowOn, Glow, Instant)>,
    glow_rungs: Vec<Rung>,
    glow_items: usize,
    /// When the frame after the one drawn last is due: something on it
    /// moves.
    drawn_motion: Option<Instant>,
    /// What imece waits for an answer to (`:commit`), said on the bottom
    /// line with a spinner until it comes.
    pub asking: Option<String>,
    /// Whether the drawing being made and the one before it showed
    /// imece's mark (`ui::brand`), and the beat its dots started coming
    /// on, the first drawing of it after one without; none: whole (a key).
    pub mark_drawn: Cell<bool>,
    pub mark_before: Cell<bool>,
    pub mark_from: Cell<Option<Instant>>,
    /// What the pot had paid when this run began: the farewell says what
    /// the run spent.
    pub spent_at_start: u64,
}

impl App {
    pub fn new(projects: usize, workers: usize, researchers: usize, seed: u64, lang: Lang) -> App {
        let (world, sim) = sim::world(projects, workers, researchers, seed, lang);
        App::with(world, Engine::Sim(sim), lang)
    }

    pub fn with(world: World, engine: Engine, lang: Lang) -> App {
        let collapsed = vec![false; world.projects.len()];
        // What waits already at the start does not light up as new.
        let glow_items = world.inbox.len();
        let selected = match &engine {
            // A live team starts with the leader of the checkout at hand
            // selected: it is who to talk to.
            Engine::Live(l) => Sel::Agent(world.projects[l.home()].leader),
            Engine::Sim(_) => Sel::Project(0),
            // The archive's head: who led it.
            Engine::View(v) => Sel::Agent(world.projects[v.project].leader),
        };
        let left_off = match &engine {
            Engine::Live(l) => l
                .left_off()
                .and_then(|left| LeftOff::of(&world, left.journal, left.ago)),
            Engine::Sim(_) | Engine::View(_) => None,
        };
        // Live, every vendor is asked when the window opens; the demo's
        // are its own.
        let vendors = match &engine {
            Engine::Live(_) => LISTED.map(|v| (v, Seen::Checking)).to_vec(),
            Engine::Sim(_) => sim::vendors(),
            Engine::View(_) => Vec::new(),
        };
        let begun = world.tick;
        let spent_at_start = world.spent();
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
            settled: BTreeSet::new(),
            settled_projects: BTreeSet::new(),
            peek: None,
            script: None,
            tidy: None,
            teardown: None,
            tidy_start: false,
            hints: None,
            needs_only: false,
            story: false,
            story_away: false,
            story_sel: 0,
            story_items: RefCell::new(Vec::new()),
            timeline_sel: 0,
            timeline_scale: 0,
            unseen: BTreeSet::new(),
            turning: BTreeSet::new(),
            inbox_sel: 0,
            deferred: Vec::new(),
            waiting_at: None,
            diff_scroll: 0,
            reading: Reading::default(),
            diff_wrap: false,
            diff_notes: Vec::new(),
            noting: None,
            talk_notes: Vec::new(),
            fold_talk: false,
            chosen: Vec::new(),
            diff_copy: None,
            diff_folds: None,
            diff_page: Cell::new(10),
            journal_sel: None,
            outputs: BTreeMap::new(),
            thread: None,
            card: None,
            been: Vec::new(),
            jumps: Vec::new(),
            recall: None,
            searching: false,
            recalling: false,
            went_back: false,
            side: SIDE_DEFAULT,
            copy: None,
            shell: None,
            editor: None,
            ctrl_x: false,
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
            help_scroll: 0,
            help_room: Cell::new((1, 0)),
            keys: Keymap::default(),
            keys_file: None,
            keys_good: None,
            keys_new: false,
            keys_seen: keys::Stamp::default(),
            keys_held: None,
            keys_looked: None,
            solo_pick: None,
            archives: None,
            viewing: None,
            restore_asked: false,
            primary: None,
            digest: None,
            left_off,
            welcome: false,
            vendors,
            look: None,
            looking: None,
            lead_check: None,
            news: Vec::new(),
            leaving: false,
            theme_was: None,
            flash: None,
            grace: None,
            archive_title: String::new(),
            quit: false,
            store_error: None,
            shown: RefCell::new(Vec::new()),
            hintable: RefCell::new(Vec::new()),
            breathing: RefCell::new(None),
            caret_at: Cell::new(None),
            last_key: Instant::now(),
            last_key_tick: 0,
            blurred: None,
            left_tick: begun,
            away: Away::default(),
            drawn_at: None,
            drawn_clock: None,
            changed: false,
            stirred: false,
            awake: crate::awake::Awake::new(false, crate::awake::command(std::process::id())),
            notify: Notify::Off,
            notices: notify::Notices::default(),
            noticed: Vec::new(),
            notice_count: 0,
            push: None,
            push_file: crate::push::file(),
            pushes: std::sync::mpsc::channel(),
            push_failing: false,
            motion: Motion::Off,
            ssh: false,
            epoch: Instant::now(),
            frame: Instant::now(),
            glows: Vec::new(),
            glow_rungs: Vec::new(),
            glow_items,
            drawn_motion: None,
            asking: None,
            mark_drawn: Cell::new(false),
            mark_before: Cell::new(false),
            mark_from: Cell::new(None),
            spent_at_start,
        }
    }

    pub fn step(&mut self) {
        let came = match &mut self.engine {
            // The demo's world moves on every step.
            Engine::Sim(s) => {
                s.step(&mut self.world);
                true
            }
            Engine::Live(l) => l.step(&mut self.world),
            // An archive never moves.
            Engine::View(_) => false,
        };
        self.changed |= came;
        self.show_suggestion();
        self.watch_tidy();
        self.world.watch_spending();
        self.watch_turns();
        self.watch_lanes();
        self.watch_glows(Instant::now());
        let asking = match &self.engine {
            Engine::Live(l) => l.asking(),
            Engine::Sim(s) => s.asking(),
            Engine::View(_) => false,
        };
        if !asking {
            self.asking = None;
        }
        self.wake_settled();
        self.keep_awake();
        self.settle();
        self.follow_thread();
        self.persist();
    }

    /// Marks the agents whose turn ended in this step, idle now, as done
    /// and not yet seen; one that is mid-turn again is not, and neither is
    /// one the person has open.
    /// Notes in every agent's lane what it does now, at every step: the
    /// demo and live alike. A gone agent's lane stops.
    pub(crate) fn watch_lanes(&mut self) {
        let w = &self.world;
        let t = w.tick;
        let waiting: BTreeSet<AgentId> = (w.open_inbox().into_iter())
            .map(|i| w.inbox[i].agent)
            .collect();
        let cells: Vec<Option<LaneCell>> = (w.agents.iter())
            .map(|a| (!a.gone).then(|| cell_of(w, a, waiting.contains(&a.id))))
            .collect();
        for (a, cell) in self.world.agents.iter_mut().zip(cells) {
            if let Some(cell) = cell {
                a.lane.mark(t, cell);
            }
        }
    }

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
        if let Some(id) = self.peek {
            return vec![id];
        }
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

    /// The tick agent `id`'s state began, for how long its row says it has
    /// been in it: when something of its waits for the person, the oldest
    /// open item's; else its last status change.
    pub fn since(&self, id: AgentId) -> u64 {
        let w = &self.world;
        w.open_inbox()
            .into_iter()
            .map(|i| &w.inbox[i])
            .filter(|it| it.agent == id)
            .map(|it| it.tick)
            .min()
            .unwrap_or(w.agents[id].changed_at)
    }

    /// What each agent needs of the person, by id. Gone agents need
    /// nothing.
    pub fn needs(&self) -> Vec<Need> {
        self.rungs().into_iter().map(Rung::need).collect()
    }

    /// Every agent's rung on the state ladder (by id).
    /// `:why`: agent `id`'s rung, since when (the tick [`App::since`]
    /// says), and the journal's line that put it there: its newest line
    /// from then or before, if the journal still has one.
    pub fn why(&self, id: AgentId) -> (Rung, u64, Option<usize>) {
        let since = self.since(id);
        let line = (self.world.journal.iter().enumerate().rev())
            .find(|(_, e)| e.tick <= since && e.event.agent() == id)
            .map(|(i, _)| i);
        (self.rungs()[id], since, line)
    }

    pub fn rungs(&self) -> Vec<Rung> {
        let w = &self.world;
        let mut waits = vec![false; w.agents.len()];
        for i in w.open_inbox() {
            waits[w.inbox[i].agent] = true;
        }
        w.agents
            .iter()
            .map(|a| {
                if a.gone {
                    Rung::Idle
                } else if matches!(a.status, Status::Blocked | Status::Stopped) {
                    Rung::Stuck
                } else if waits[a.id] || a.status == Status::WaitingHuman {
                    Rung::Waiting
                } else if a.status == Status::Idle && self.unseen.contains(&a.id) {
                    // `done_unseen`, its inbox already looked at.
                    Rung::Done
                } else if a.starting {
                    Rung::Starting
                } else if a.status.busy() || matches!(a.status, Status::Asking { .. }) {
                    Rung::Working
                } else {
                    Rung::Idle
                }
            })
            .collect()
    }

    /// What the tab's progress and the title's glyph say: the top of the
    /// ladder over the agents of the projects that are open (a closed or
    /// archived one is not on the screen), waiting also when anything is
    /// in WAITING FOR YOU, as the title counts it. Done, not seen, is calm:
    /// the person sees it when back.
    pub fn progress(&self) -> Progress {
        let w = &self.world;
        let rungs = self.rungs();
        let top = (w.agents.iter())
            .filter(|a| !w.put_away(a.id))
            .map(|a| rungs[a.id])
            .max();
        match top {
            Some(Rung::Stuck) => Progress::Stuck,
            _ if !w.open_inbox().is_empty() => Progress::Waiting,
            Some(Rung::Waiting) => Progress::Waiting,
            Some(Rung::Working | Rung::Starting) => Progress::Working,
            _ => Progress::Calm,
        }
    }

    /// The loudest of `members`: the one on the highest rung, the first
    /// among equals; none of an empty group.
    pub fn loudest(&self, members: impl IntoIterator<Item = AgentId>) -> Option<AgentId> {
        let rungs = self.rungs();
        let mut loud: Option<AgentId> = None;
        for id in members {
            if loud.is_none_or(|l| rungs[id] > rungs[l]) {
                loud = Some(id);
            }
        }
        loud
    }

    /// Whether the side panel tells the story now: it is on, and no card's
    /// screen shows its own journal.
    pub fn story_on(&self) -> bool {
        self.story && !self.story_away && self.card.is_none()
    }

    /// `J`: the side panel tells the story, or shows the journal again;
    /// from a line of the story shown in the journal, back to the story.
    fn toggle_story(&mut self) {
        if self.story_away {
            return self.story_back();
        }
        self.story = !self.story;
        self.story_sel = 0;
        if self.story {
            self.journal_sel = None;
        } else if self.pane == Pane::Journal {
            self.select_journal(None);
        }
        let t = self.lang.text();
        self.flash(if self.story { t.story_on } else { t.story_off });
    }

    /// The story's selected item, while it is shown.
    fn story_item(&self) -> Option<ui::story::Item> {
        if !self.story_on() {
            return None;
        }
        self.story_items.borrow().get(self.story_sel).copied()
    }

    /// `enter` on the story: a card opens its screen, a line of no card is
    /// shown in the journal.
    fn open_story_item(&mut self) {
        match self.story_item() {
            Some(ui::story::Item::Card(c)) => self.open_card(c),
            Some(ui::story::Item::Line(i)) => {
                self.story_away = true;
                self.journal_sel = Some(i);
                self.flash(self.lang.text().story_away);
            }
            None => {}
        }
    }

    /// `esc` (or `J`) in the journal shown in the story's place: the story
    /// again, its item at hand as it was.
    fn story_back(&mut self) {
        self.story_away = false;
        self.journal_sel = None;
        self.flash(self.lang.text().story_on);
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

    /// The agents agent `id`'s salon row stands for: itself, its helpers
    /// and the researcher working for it, which hang under it.
    fn row_group(&self, id: AgentId) -> Vec<AgentId> {
        let w = &self.world;
        let under = w.agents.iter().filter(|a| {
            !a.gone
                && match a.role {
                    Role::Helper => a.parent == Some(id),
                    Role::Researcher => {
                        matches!(a.status, Status::Researching { for_agent, .. } if for_agent == id)
                    }
                    _ => false,
                }
        });
        std::iter::once(id).chain(under.map(|a| a.id)).collect()
    }

    /// The agents a settled project stands for: all of its own.
    fn project_group(&self, p: ProjectId) -> Vec<AgentId> {
        let w = &self.world;
        (w.agents.iter().filter(|a| a.project == p))
            .map(|a| a.id)
            .collect()
    }

    /// What was settled comes back to the salon when it needs the person
    /// again: an agent when something of its row waits for them, is
    /// stuck or finished a turn not seen; a project when any of its
    /// agents does. Agents gone and projects closed are let go.
    fn wake_settled(&mut self) {
        let need = self.needs();
        let calm = |group: Vec<AgentId>| group.iter().all(|&a| need[a] == Need::Nothing);
        let agents = (self.settled.iter().copied())
            .filter(|&id| !self.world.agents[id].gone && calm(self.row_group(id)))
            .collect();
        let projects = (self.settled_projects.iter().copied())
            .filter(|&p| !self.world.projects[p].closed && calm(self.project_group(p)))
            .collect();
        self.settled = agents;
        self.settled_projects = projects;
    }

    /// `s` or `:settle`: the selected worker's row (its helpers with it),
    /// or the selected project (its header or its leader) out of the salon
    /// until it needs the person again; refused while something of it
    /// waits for them or is stuck. Its turns finished count as seen, and
    /// the selection goes on to the next row still shown.
    fn settle_at_hand(&mut self) {
        let t = self.lang.text();
        let w = &self.world;
        // What goes: the row's agent, or the project of a header or a
        // leader's row.
        let target = match self.selected {
            Sel::Agent(id) if w.projects[w.agents[id].project].leader == id => {
                Sel::Project(w.agents[id].project)
            }
            sel => sel,
        };
        let (group, name) = match target {
            Sel::Agent(id) => (self.row_group(id), w.agents[id].name.clone()),
            Sel::Project(p) => (self.project_group(p), w.projects[p].name.clone()),
        };
        let need = self.needs();
        if let Some(&loud) = group.iter().find(|&&a| need[a] > Need::Done) {
            let who = self.world.agents[loud].name.clone();
            return self.flash(fill(t.settle_refused, &[&who]));
        }
        let rows = |app: &App| -> Vec<Sel> {
            (ui::salon::rows(app, usize::MAX).iter())
                .filter_map(ui::salon::Row::sel)
                .collect()
        };
        let before = rows(self);
        for a in &group {
            self.unseen.remove(a);
        }
        match target {
            Sel::Agent(id) => self.settled.insert(id),
            Sel::Project(p) => self.settled_projects.insert(p),
        };
        let after = rows(self);
        // The rows after it, then those before it, nearest first; the
        // selected row itself is gone from `after`.
        if let Some(at) = before.iter().position(|s| *s == self.selected) {
            let (head, tail) = before.split_at(at);
            let next = (tail.iter().chain(head.iter().rev())).find(|s| after.contains(s));
            if let Some(&s) = next {
                self.selected = s;
            }
        }
        self.flash(fill(t.settled_one, &[&name]));
    }

    /// `:unsettle`: everything settled back in the salon.
    fn unsettle(&mut self) {
        let n = self.settled.len() + self.settled_projects.len();
        self.settled.clear();
        self.settled_projects.clear();
        self.go_salon();
        let t = self.lang.text();
        self.flash(fill(t.unsettled, &[&n.to_string()]));
    }

    /// A key while a peek is open: `j k` (the arrows) the next / previous
    /// agent of the salon, `enter` its whole focus, `@` a message to it,
    /// `esc` or `K` close it, `?` the help; the rest do nothing, so the
    /// view under it does not move.
    fn peek_key(&mut self, id: AgentId, k: KeyEvent) {
        match k.code {
            KeyCode::Down | KeyCode::Char('j') => self.peek = Some(self.peek_step(id, true)),
            KeyCode::Up | KeyCode::Char('k') => self.peek = Some(self.peek_step(id, false)),
            KeyCode::Enter => {
                self.peek = None;
                self.zoom = Zoom::Focus(id);
            }
            KeyCode::Char('@') => {
                self.peek = None;
                self.typing = true;
                self.input = format!("@{} ", self.world.agents[id].name);
                self.menu = self.suggest();
            }
            KeyCode::Esc | KeyCode::Char('K') => self.peek = None,
            KeyCode::Char('?') => self.open_help(),
            _ => {}
        }
    }

    /// A key while a setup script's window is open: `j k` (the arrows),
    /// `ctrl-d ctrl-u` (page keys) and `g G` move through it, `a` says yes
    /// and `r` no to it, `esc` or `q` close it, `?` the help; the rest do
    /// nothing.
    fn script_key(&mut self, item: usize, at: usize, k: KeyEvent) {
        let ctrl = k.modifiers.contains(KeyModifiers::CONTROL);
        let page = SCRIPT_PAGE;
        let to = match k.code {
            KeyCode::Down | KeyCode::Char('j') => at + 1,
            KeyCode::Up | KeyCode::Char('k') => at.saturating_sub(1),
            KeyCode::PageDown => at + page,
            KeyCode::Char('d') if ctrl => at + page,
            KeyCode::PageUp => at.saturating_sub(page),
            KeyCode::Char('u') if ctrl => at.saturating_sub(page),
            KeyCode::Char('g') => 0,
            KeyCode::Char('G') => usize::MAX,
            KeyCode::Char('a') => return self.answer_script(item, true),
            KeyCode::Char('r') => return self.answer_script(item, false),
            KeyCode::Esc | KeyCode::Char('q') => {
                self.script = None;
                return;
            }
            KeyCode::Char('?') => return self.open_help(),
            _ => return,
        };
        // Never past the last line's being the top one.
        let last = self
            .script_text(item)
            .map_or(0, |s| s.text().lines().count());
        self.script = Some((item, to.min(last.saturating_sub(1))));
    }

    /// The script setup ask `item` is about, as the engine read it.
    pub fn script_text(&self, item: usize) -> Option<crate::live::setup::Script> {
        match &self.engine {
            Engine::Live(l) => l.setup_asked(item).cloned(),
            Engine::Sim(s) => s.setup_asked(&self.world, item),
            Engine::View(_) => None,
        }
    }

    /// The person's answer to setup script ask `item`: the engine keeps a
    /// yes and runs the bytes asked about, or holds a no for the run; the
    /// window closes and the flash says which. An ask no longer there
    /// (its run ended) is said so.
    fn answer_script(&mut self, item: usize, yes: bool) {
        let t = self.lang.text();
        let took = match &mut self.engine {
            Engine::Live(l) => l.decide_setup(&mut self.world, item, yes),
            Engine::Sim(s) => s.decide_setup(&mut self.world, item, yes),
            Engine::View(_) => return self.read_only(),
        };
        self.script = None;
        self.flash(match (took, yes) {
            (false, _) => t.setup_gone,
            (true, true) => t.setup_yes,
            (true, false) => t.setup_no,
        });
        let open = self.inbox_rows().len();
        self.inbox_sel = self.inbox_sel.min(open.saturating_sub(1));
    }

    /// At a start: the engine looks for worktrees no agent owns, and a
    /// flash says how many when it found any ([`App::watch_tidy`]).
    pub fn start_tidy(&mut self) {
        self.tidy_start = true;
        self.look_orphans();
    }

    fn look_orphans(&mut self) {
        match &mut self.engine {
            Engine::Live(l) => l.look_orphans(&mut self.world),
            Engine::Sim(s) => s.look_orphans(&mut self.world),
            Engine::View(_) => {}
        }
    }

    /// `:tidy`: the worktrees no agent owns in a window, looked for again.
    pub(crate) fn open_tidy(&mut self) {
        self.tidy = Some(TidyWin::default());
        self.look_orphans();
    }

    /// What a look or a removal answered: a removal's end is said once;
    /// the start's look says how many it found, unless the window shows
    /// them. The selection stays on the list.
    fn watch_tidy(&mut self) {
        let t = self.lang.text();
        if let Some(tidied) = self.world.orphans.tidied.take() {
            self.flash(ui::words::tidied(self.lang, &tidied));
        }
        let Some(found) = self.world.orphans.found.as_ref() else {
            return;
        };
        let n = found.len();
        if let Some(win) = &mut self.tidy {
            win.at = win.at.min(n.saturating_sub(1));
        }
        if self.tidy_start && !self.world.orphans.busy {
            self.tidy_start = false;
            if n > 0 && self.tidy.is_none() {
                self.flash(fill(t.tidy_found, &[&n.to_string()]));
            }
        }
    }

    /// A key while the `:tidy` window is open: `j k` (the arrows), `g G`
    /// move, `a` removes the clean ones, `d` the one at hand (one that
    /// holds work after a `y` for it: any other key keeps it), `esc` or
    /// `q` close, `?` the help; the rest do nothing.
    fn tidy_key(&mut self, win: TidyWin, k: KeyEvent) {
        let found = self.world.orphans.found.clone().unwrap_or_default();
        if win.sure {
            self.tidy = Some(TidyWin { sure: false, ..win });
            if k.code == KeyCode::Char('y') {
                self.tidy_remove(vec![win.at], true);
            }
            return;
        }
        let last = found.len().saturating_sub(1);
        let at = match k.code {
            KeyCode::Down | KeyCode::Char('j') => (win.at + 1).min(last),
            KeyCode::Up | KeyCode::Char('k') => win.at.saturating_sub(1),
            KeyCode::Char('g') | KeyCode::Home => 0,
            KeyCode::Char('G') | KeyCode::End => last,
            KeyCode::Char('a') => {
                let clean: Vec<usize> = (0..found.len()).filter(|&i| found[i].clean()).collect();
                if clean.is_empty() {
                    return self.flash(self.lang.text().tidy_no_clean);
                }
                return self.tidy_remove(clean, false);
            }
            KeyCode::Char('d') => {
                let Some(o) = found.get(win.at) else {
                    return;
                };
                if o.clean() {
                    return self.tidy_remove(vec![win.at], false);
                }
                self.tidy = Some(TidyWin { sure: true, ..win });
                return;
            }
            KeyCode::Esc | KeyCode::Char('q') => {
                self.tidy = None;
                return;
            }
            KeyCode::Char('?') => return self.open_help(),
            _ => return,
        };
        self.tidy = Some(TidyWin { at, ..win });
    }

    /// The worktrees at `at` of the list go, those that hold work only
    /// when `force`; a refusal is said.
    fn tidy_remove(&mut self, at: Vec<usize>, force: bool) {
        let t = self.lang.text();
        let done = match &mut self.engine {
            Engine::Live(l) => l.tidy(&mut self.world, &at, force),
            Engine::Sim(s) => s.tidy(&mut self.world, &at, force),
            Engine::View(_) => return self.read_only(),
        };
        match done {
            Ok(()) => {}
            Err(TidyRefusal::Busy) => self.flash(t.tidy_busy),
            Err(TidyRefusal::Nothing) => self.flash(t.tidy_none),
            Err(TidyRefusal::Asks(script)) => {
                self.teardown = Some(TeardownAsk {
                    script: *script,
                    scroll: 0,
                    at,
                    force,
                })
            }
        }
    }

    /// A key while a teardown script is asked about: `j k` (the arrows),
    /// `ctrl-d ctrl-u` (page keys) and `g G` move through it, `a` says
    /// yes and `r` no to it and the removal asked goes on, `esc` or `q`
    /// close it and nothing is removed, `?` the help; the rest do nothing.
    fn teardown_key(&mut self, k: KeyEvent) {
        let Some(ask) = self.teardown.as_mut() else {
            return;
        };
        let ctrl = k.modifiers.contains(KeyModifiers::CONTROL);
        let (at, page) = (ask.scroll, SCRIPT_PAGE);
        let to = match k.code {
            KeyCode::Down | KeyCode::Char('j') => at + 1,
            KeyCode::Up | KeyCode::Char('k') => at.saturating_sub(1),
            KeyCode::PageDown => at + page,
            KeyCode::Char('d') if ctrl => at + page,
            KeyCode::PageUp => at.saturating_sub(page),
            KeyCode::Char('u') if ctrl => at.saturating_sub(page),
            KeyCode::Char('g') => 0,
            KeyCode::Char('G') => usize::MAX,
            KeyCode::Char('a') => return self.answer_teardown(true),
            KeyCode::Char('r') => return self.answer_teardown(false),
            KeyCode::Esc | KeyCode::Char('q') => {
                self.teardown = None;
                return self.flash(self.lang.text().teardown_held);
            }
            KeyCode::Char('?') => return self.open_help(),
            _ => return,
        };
        // Never past the last line's being the top one.
        let last = ask.script.text().lines().count();
        ask.scroll = to.min(last.saturating_sub(1));
    }

    /// The person's answer to the teardown script asked about: the
    /// engine keeps it, and the removal it held goes on (asking about the
    /// next checkout's, if another has one).
    fn answer_teardown(&mut self, yes: bool) {
        let Some(ask) = self.teardown.take() else {
            return;
        };
        match &mut self.engine {
            Engine::Live(l) => l.decide_teardown(&mut self.world, &ask.script, yes),
            Engine::Sim(s) => s.decide_teardown(&mut self.world, &ask.script, yes),
            Engine::View(_) => return self.read_only(),
        }
        self.tidy_remove(ask.at, ask.force);
    }

    /// `F` or `:hints`: a label on every row the salon's screen showed
    /// last that can be selected, in the order drawn. Only there, with no
    /// conversation over it: elsewhere the screen has no such rows.
    /// Whether a window any key closes is open: what is new, the
    /// first-run window, the away digest, where the person left off.
    fn window_open(&self) -> bool {
        !self.news.is_empty() || self.welcome || self.digest.is_some() || self.left_off.is_some()
    }

    /// `:welcome`: the first-run window, every vendor asked again (live).
    pub fn open_welcome(&mut self) {
        self.welcome = true;
        self.look_at(&LISTED);
    }

    /// Asks the vendors `which` about themselves again, live: their lines
    /// say `checking…` until the answers come ([`App::looked`]).
    fn look_at(&mut self, which: &[Vendor]) {
        let Some(look) = self.look else {
            return;
        };
        for (v, seen) in &mut self.vendors {
            if which.contains(v) {
                *seen = Seen::Checking;
            }
        }
        self.looking = Some(look(which));
    }

    /// A live start: the first ever opens the first-run window; any other
    /// asks only whether the leader's vendor is signed in, and opens it
    /// when not ([`App::looked`]).
    pub fn start_welcome(&mut self, first: bool) {
        if first {
            return self.open_welcome();
        }
        let p = &self.world.projects[self.current_project()];
        let lead = self.world.agents[p.leader].vendor;
        self.lead_check = Some(lead);
        self.look_at(&[lead]);
    }

    /// The answers the vendors gave since last time, each on its line. The
    /// leader's vendor not signed in at the start opens the window, the
    /// others asked then; while the person writes, a line says so instead.
    pub fn looked(&mut self) {
        let (mut got, mut done) = (Vec::new(), false);
        if let Some(rx) = &self.looking {
            loop {
                match rx.try_recv() {
                    Ok(answer) => got.push(answer),
                    Err(TryRecvError::Empty) => break,
                    Err(TryRecvError::Disconnected) => {
                        done = true;
                        break;
                    }
                }
            }
        }
        if done {
            self.looking = None;
        }
        // A vendor's line on the welcome window, if open.
        self.changed |= !got.is_empty();
        for (v, seen) in got {
            let out = matches!(
                seen,
                Seen::Here {
                    login: Login::No,
                    ..
                }
            );
            if let Some(line) = self.vendors.iter_mut().find(|(w, _)| *w == v) {
                line.1 = seen;
            }
            if self.lead_check != Some(v) {
                continue;
            }
            self.lead_check = None;
            if !out {
                continue;
            }
            if self.typing {
                let t = self.lang.text();
                self.flash(fill(t.not_signed_in, &[v.label()]));
            } else {
                self.welcome = true;
                let others: Vec<Vendor> = LISTED.into_iter().filter(|&w| w != v).collect();
                self.look_at(&others);
            }
        }
    }

    /// `:news`: what is new in the latest versions.
    fn open_news(&mut self) {
        let news = self.lang.text().news;
        self.news = news.iter().take(welcome::NEWS_SHOWN).map(|n| n.0).collect();
    }

    fn open_hints(&mut self) {
        let targets: Vec<Target> = if self.pane_keys() && self.thread.is_none() {
            self.hintable.borrow().iter().map(|&(t, _, _)| t).collect()
        } else {
            Vec::new()
        };
        if targets.is_empty() {
            let t = self.lang.text();
            self.flash(t.no_hints.to_string());
            return;
        }
        let labels = hint_labels(targets.len())
            .into_iter()
            .zip(targets)
            .collect();
        self.hints = Some(Hints {
            labels,
            typed: String::new(),
        });
    }

    /// A key while the hint letters are on the screen: a letter narrows
    /// them to the labels it goes on; the last letter of a label selects
    /// its row, in capitals opens it too (as `enter` would). A letter no
    /// label goes on does nothing; backspace takes the last letter back,
    /// `esc` closes them.
    fn hint_key(&mut self, k: KeyEvent) {
        let Some(h) = &mut self.hints else {
            return;
        };
        match k.code {
            KeyCode::Esc => self.hints = None,
            KeyCode::Backspace => {
                h.typed.pop();
            }
            KeyCode::Char('?') => self.open_help(),
            // Labels are letters: anything else goes on none.
            KeyCode::Char(c) => {
                let typed = format!("{}{}", h.typed, c.to_ascii_lowercase());
                if let Some(&(_, target)) = h.labels.iter().find(|(l, _)| *l == typed) {
                    self.hints = None;
                    self.hint_go(target, c.is_ascii_uppercase());
                } else if h.labels.iter().any(|(l, _)| l.starts_with(&typed)) {
                    h.typed = typed;
                }
            }
            _ => {}
        }
    }

    /// Selects `target` in its panel, and opens it when `open`; a row
    /// gone from the screen since the labels were given is said so.
    fn hint_go(&mut self, target: Target, open: bool) {
        let here = match target {
            Target::Row(sel) => self.shown.borrow().contains(&sel).then(|| {
                self.pane = Pane::Salon;
                self.selected = sel;
            }),
            Target::Waiting(i) => self.inbox_rows().iter().position(|&r| r == i).map(|at| {
                self.pane = Pane::Inbox;
                self.inbox_sel = at;
                // The person picks: `W` goes on from what they picked.
                self.waiting_at = None;
            }),
            Target::Line(i) => (self.journal_rows().iter().any(|&(r, _)| r == i)).then(|| {
                self.pane = Pane::Journal;
                self.journal_sel = Some(i);
            }),
        };
        match here {
            Some(()) if open => self.deeper(),
            Some(()) => {}
            None => {
                let t = self.lang.text();
                self.flash(t.hint_gone.to_string());
            }
        }
    }

    /// The agent after (`forward`) or before agent `id` in the salon's
    /// order; the first when `id` has no row there; `id` itself at
    /// either end.
    fn peek_step(&self, id: AgentId, forward: bool) -> AgentId {
        let agents: Vec<AgentId> = (ui::salon::rows(self, usize::MAX).iter())
            .filter_map(|r| match r.sel() {
                Some(Sel::Agent(a)) => Some(a),
                _ => None,
            })
            .collect();
        let next = match agents.iter().position(|&a| a == id) {
            Some(i) if forward => agents.get(i + 1),
            Some(i) => i.checked_sub(1).and_then(|i| agents.get(i)),
            None => agents.first(),
        };
        next.copied().unwrap_or(id)
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

    /// Whether the demo drives the agents (it pauses and changes speed).
    pub fn demo(&self) -> bool {
        matches!(self.engine, Engine::Sim(_))
    }

    /// Whether this screen is an archive's read-only view.
    pub fn viewing_archive(&self) -> bool {
        matches!(self.engine, Engine::View(_))
    }

    /// The archive this screen is the read-only view of, if it is one.
    pub fn viewed(&self) -> Option<&Viewed> {
        match &self.engine {
            Engine::View(v) => Some(v),
            Engine::Sim(_) | Engine::Live(_) => None,
        }
    }

    /// In an archive's view, what would act says it cannot.
    fn read_only(&mut self) {
        self.flash(self.lang.text().read_only);
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

    /// The keys a start found (`keys::Start`), the key file at `file` as
    /// it was before it was read (`seen`); a file refused is said.
    pub fn start_keys(&mut self, file: Option<PathBuf>, seen: keys::Stamp, start: keys::Start) {
        self.keys = start.map;
        self.keys_file = file;
        self.keys_seen = seen;
        self.keys_new = false;
        self.keys_good = start.good;
        if let (Some(why), Some(path)) = (start.refused, &self.keys_file) {
            self.flash(keys::refusal(self.lang, path, &why));
        }
    }

    /// `:keys`: the key file read again. A good one is taken and said
    /// (with the keys of imece's nothing does now), none leaves imece's
    /// own keys, a bad one leaves the keys in use and says why.
    fn read_keys(&mut self) {
        let t = self.lang.text();
        let Some(path) = self.keys_file.clone() else {
            return self.flash(t.keys_nowhere);
        };
        let shown = path.display().to_string();
        self.keys_seen = keys::stamp(&path);
        let good = match keys::load(&path) {
            keys::Read::Missing => {
                self.keys = Keymap::default();
                self.flash(fill(t.keys_none, &[&shown]));
                None
            }
            keys::Read::Good(map, text) => {
                self.flash(keys::taken(self.lang, &path, &map));
                self.keys = map;
                Some(text)
            }
            keys::Read::Bad(why) => return self.flash(keys::refusal(self.lang, &path, &why)),
        };
        if good != self.keys_good {
            self.keys_good = good;
            self.keys_new = true;
        }
    }

    /// The key file looked at at `now`, at most every [`keys::EVERY`]:
    /// changed since it was last read (another text, saved over, made or
    /// removed), it is read again as on `:keys`, and what came of it is
    /// said; a bad edit leaves the keys in use.
    pub fn watch_keys(&mut self, now: Instant) {
        if self
            .keys_looked
            .is_some_and(|at| now.duration_since(at) < keys::EVERY)
        {
            return;
        }
        self.keys_looked = Some(now);
        let Some(path) = &self.keys_file else {
            return;
        };
        if keys::stamp(path) != self.keys_seen {
            self.read_keys();
        }
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
    pub(crate) fn project_at_hand(&self) -> ProjectId {
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
        let ctrl_c = k.modifiers.contains(KeyModifiers::CONTROL) && k.code == KeyCode::Char('c');
        if self.viewing.is_some() && !ctrl_c {
            return self.view_key(k);
        }
        // The mark's coming never holds a key up.
        self.mark_from.set(None);
        self.around(|app| app.key(k));
    }

    /// A key while an archive's view is open: `esc` or `q` where the view
    /// has nothing open closes it; any other goes to the view.
    fn view_key(&mut self, k: KeyEvent) {
        let Some(v) = self.viewing.as_mut() else {
            return;
        };
        let leaves = matches!(k.code, KeyCode::Esc | KeyCode::Char('q'));
        if leaves && k.modifiers.is_empty() && v.at_rest() {
            self.viewing = None;
            return;
        }
        v.on_key(k);
        self.after_view();
    }

    /// What a key in an archive's view asked of this screen: text to
    /// copy, to leave the view (`q q`, `:quit`), to bring the archive back
    /// (`:restore`).
    fn after_view(&mut self) {
        let Some(v) = self.viewing.as_mut() else {
            return;
        };
        if let Some(text) = v.copy.take() {
            self.copy = Some(text);
        }
        if v.quit {
            self.viewing = None;
        } else if v.restore_asked {
            let Some(p) = v.viewed().map(|viewed| viewed.project) else {
                return;
            };
            self.viewing = None;
            self.restore_at(p);
        }
    }

    /// Whether nothing is open on this screen that `esc` would close: no
    /// window, line, chord, conversation, card, focus or change; the
    /// salon, the board or the code map as they are.
    fn at_rest(&self) -> bool {
        use keys::Scope;
        !self.typing
            && !self.help
            && self.pending.is_none()
            && self.keys_held.is_none()
            && self.hints.is_none()
            && self.noting.is_none()
            && self.card.is_none()
            && self.zoom == Zoom::None
            && matches!(
                self.scope_below(),
                Some(Scope::Salon | Scope::Board | Scope::Code | Scope::Timeline)
            )
    }

    /// `enter` on an archive (or one of its agents, `agent`) in the
    /// browser: its read-only view opens over this screen, from what the
    /// store keeps of its journal (the demo: what it has in memory).
    fn view_archive(&mut self, p: ProjectId, agent: Option<AgentId>) {
        let t = self.lang.text();
        let Some(archived) = self.world.projects[p].archived.clone() else {
            return;
        };
        let read = match &self.engine {
            Engine::Live(l) => l.project_journal(&self.world, p),
            Engine::Sim(_) => Ok(self.world.journal_of(p)),
            Engine::View(_) => return self.read_only(),
        };
        let (journal, marks) = match read {
            Ok(read) => read,
            Err(why) => return self.flash(fill(t.view_failed, &[&why])),
        };
        let world = self.world.view_of(p, journal, marks);
        let viewed = Viewed {
            project: p,
            title: archived.title,
            at: archived.at,
        };
        let mut v = App::with(world, Engine::View(viewed), self.lang);
        v.keys = self.keys.clone();
        v.side = self.side;
        v.shift_enter = self.shift_enter;
        v.home = self.home.clone();
        v.fold_talk = self.fold_talk;
        if let Some(id) = agent {
            v.selected = Sel::Agent(id);
        }
        self.viewing = Some(Box::new(v));
    }

    /// The first of two keys of the person's, held since [`keys::WAIT`]
    /// before `now` with no second: it acts as itself alone.
    pub fn keys_due(&mut self, now: Instant) {
        if let Some(v) = &mut self.viewing {
            v.keys_due(now);
            self.changed |= std::mem::take(&mut v.changed);
            return self.after_view();
        }
        if self
            .keys_held
            .is_some_and(|(_, at)| now.duration_since(at) >= keys::WAIT)
        {
            self.around(|app| {
                if let Some((first, _)) = app.keys_held.take() {
                    let keys = app.keys.at(app.key_scope());
                    app.act_all(keys.alone(first), None);
                }
            });
            // What the key did shows, as after any key.
            self.changed = true;
        }
    }

    /// What a key does, `act`, and what follows any key: the place left
    /// behind, the theme previewed, what is seen and kept.
    fn around(&mut self, act: impl FnOnce(&mut Self)) {
        let was = self.place();
        act(self);
        // A key that took the person elsewhere leaves the place behind;
        // `^o` going back does not.
        // Lines of a change being chosen to copy go with the change.
        if self
            .diff_copy
            .is_some_and(|(id, _, _)| self.zoom != Zoom::Diff(id))
        {
            self.diff_copy = None;
        }
        self.keep_reading();
        let back = std::mem::take(&mut self.went_back);
        if !back && self.place() != was {
            self.been.push(was);
            if self.been.len() > PLACES {
                self.been.remove(0);
            }
        }
        self.preview_theme();
        self.seen();
        self.settle();
        self.follow_thread();
        self.persist();
    }

    /// While `:theme NAME` is being written with NAME a built-in theme
    /// (typed, or stepped to in its menu), the screen wears it; the theme
    /// in use comes back when the line no longer names one or is given
    /// up. `submit` puts it back before the command runs, so `:theme
    /// NAME` keeps NAME and anything else keeps what was.
    fn preview_theme(&mut self) {
        let named = (self.input.strip_prefix(":theme ").filter(|_| self.typing))
            .map(str::trim)
            .filter(|n| theme::builtins().any(|b| b == *n));
        match named {
            Some(name) => {
                if self.theme_was.is_none() {
                    self.theme_was = Some(theme::chosen());
                }
                let _ = theme::choose(name);
            }
            None => self.end_preview(),
        }
    }

    /// The theme in use before a preview, back.
    fn end_preview(&mut self) {
        if let Some(was) = self.theme_was.take() {
            // It was in use a moment ago: a file gone since leaves the
            // preview, nothing worse.
            let _ = theme::choose(&was);
        }
    }

    /// The theme chosen, not one being previewed: what is remembered for
    /// the next run.
    pub fn theme(&self) -> String {
        self.theme_was.clone().unwrap_or_else(theme::chosen)
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
        let away = (self.away.over(self.last_key.elapsed())).then_some(self.last_key_tick);
        self.last_key = Instant::now();
        self.last_key_tick = self.world.tick;

        if k.modifiers.contains(KeyModifiers::CONTROL) && k.code == KeyCode::Char('c') {
            self.quit = true;
            return;
        }
        // The person's own keys, everywhere but where keys are text: a
        // first key held goes with this one when the file gives the two,
        // else it acts alone first.
        let mut away = away;
        if let Some((first, _)) = self.keys_held.take() {
            let keys = self.keys.at(self.key_scope());
            if let Some(both) = keys.pair(first, k) {
                self.act_all(both, away);
                return;
            }
            self.act_all(keys.alone(first), away.take());
        }
        if self.typing || self.hints.is_some() || (self.help && self.help_typing) {
            self.act(k, away);
            return;
        }
        // The keys of the place the screen is at, after the first acted.
        let keys = self.keys.at(self.key_scope());
        if self.pending.is_none() && keys.starts(k) {
            // imece's own chord waits for its second key first.
            self.keys_held = Some((k, self.last_key));
        } else {
            self.act_all(keys.alone(k), away);
        }
    }

    /// The place of the screen whose part of the key file holds now
    /// (`[change]`), in the order `act` takes keys; none on the windows
    /// and questions over it.
    pub fn key_scope(&self) -> Option<keys::Scope> {
        let below = self.scope_below()?;
        Some(if self.help { keys::Scope::Help } else { below })
    }

    /// [`App::key_scope`] under the help: where `?` was pressed.
    pub fn scope_below(&self) -> Option<keys::Scope> {
        use keys::Scope;
        if self.leaving
            || !self.news.is_empty()
            || self.welcome
            || self.digest.is_some()
            || self.left_off.is_some()
            || self.solo_pick.is_some()
            || self.archives.is_some()
        {
            return None;
        }
        Some(if self.peek.is_some() {
            Scope::Peek
        } else if self.copying() {
            Scope::Choose
        } else if self.thread.is_some() {
            Scope::Talk
        } else if let Zoom::Diff(_) = self.zoom {
            Scope::Change
        } else {
            match self.view {
                View::Salon => Scope::Salon,
                View::Board => Scope::Board,
                View::Code => Scope::Code,
                View::Timeline => Scope::Timeline,
            }
        })
    }

    /// Each of `keys` as if pressed, one after the other; the second goes
    /// where the first left the screen. `away` (the tick the person left
    /// at) only for the first.
    fn act_all(&mut self, keys: Vec<KeyEvent>, mut away: Option<u64>) {
        for k in keys {
            self.act(k, away.take());
        }
    }

    /// What one key does where the screen is.
    fn act(&mut self, k: KeyEvent, away: Option<u64>) {
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
        if !std::mem::take(&mut self.news).is_empty()
            || std::mem::take(&mut self.welcome)
            || self.digest.take().is_some()
            || self.left_off.take().is_some()
        {
            return;
        }
        if self.solo_pick.is_some() {
            self.solo_pick_key(k.code);
            return;
        }
        if self.archives.is_some() {
            self.archives_key(k);
            return;
        }
        if self.help {
            self.help_key(k);
            return;
        }
        if let Some(since) = away
            && let Some(d) = Digest::since(&self.world, since)
        {
            self.digest = Some(d);
            return;
        }
        if self.typing {
            self.type_key(k);
            return;
        }
        if let Some(id) = self.peek {
            self.peek_key(id, k);
            return;
        }
        if let Some((item, at)) = self.script {
            self.script_key(item, at, k);
            return;
        }
        if self.teardown.is_some() {
            self.teardown_key(k);
            return;
        }
        if let Some(win) = self.tidy {
            self.tidy_key(win, k);
            return;
        }
        if self.hints.is_some() {
            self.hint_key(k);
            return;
        }
        if let Some(first) = self.pending.take() {
            self.chord(first, k);
            return;
        }
        if self.copy_key(k) {
            return;
        }
        if k.modifiers.contains(KeyModifiers::CONTROL) {
            match k.code {
                KeyCode::Char('d') => self.page(true, true),
                KeyCode::Char('u') => self.page(false, true),
                KeyCode::Char('f') => self.page(true, false),
                KeyCode::Char('b') => self.page(false, false),
                KeyCode::Char('o') => self.go_back(),
                KeyCode::Char('r') => self.search_history(""),
                _ => {}
            }
            return;
        }
        if self.thread.is_some() && self.thread_key(k) {
            return;
        }
        if let Some(id) = self.zoomed_agent()
            && self.zoom_answer_key(id, k)
        {
            return;
        }
        if let Some(id) = self.focus_zoom()
            && self.focus_key(id, k)
        {
            return;
        }
        if let (Zoom::Trace(id), None) = (self.zoom, &self.thread)
            && self.trace_key(id, k)
        {
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
            KeyCode::Char('&') => self.open_jump(),
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
            KeyCode::Char('K') => self.peek = self.agent_at_hand(),
            KeyCode::Char('F') => self.open_hints(),
            // On an ask in waiting for you: yes from now on; else the
            // archives.
            KeyCode::Char('A') if self.pane == Pane::Inbox && self.ask_at_hand().is_some() => {
                self.answer_always();
            }
            // On an ask in waiting for you: yes to all its agent asks
            // until its turn ends.
            KeyCode::Char('e') if self.pane == Pane::Inbox => {
                if let Some(item) = self.ask_at_hand() {
                    self.answer_turn(item);
                }
            }
            KeyCode::Char('A') => self.open_archives(),
            KeyCode::Char('H') => self.search_all(""),
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
            KeyCode::Char('4') => self.show(View::Timeline),
            // The timeline's scale: a column of 5 s, 30 s, a minute.
            KeyCode::Char(c @ ('[' | ']'))
                if self.view == View::Timeline && self.zoom == Zoom::None =>
            {
                let last = ui::timeline::SCALES.len() - 1;
                self.timeline_scale = match c {
                    ']' => (self.timeline_scale + 1).min(last),
                    _ => self.timeline_scale.saturating_sub(1),
                };
            }
            KeyCode::Char('T') => self.next_theme(),
            KeyCode::Char('L') => self.set_lang(self.lang.other()),
            // Live agents run in real time: nothing to pause or speed up.
            KeyCode::Char(' ') if self.demo() => self.pause(),
            KeyCode::Char('+') | KeyCode::Char('=') if self.demo() => self.faster(),
            KeyCode::Char('-') if self.demo() => self.slower(),
            KeyCode::Char('?') => self.open_help(),
            KeyCode::Tab | KeyCode::Char(']') if self.pane_keys() => self.cycle_pane(true),
            KeyCode::BackTab | KeyCode::Char('[') if self.pane_keys() => self.cycle_pane(false),
            KeyCode::Char('w') if self.pane_keys() => self.pane = Pane::Inbox,
            // From anywhere: what waits for the person, one item after
            // the other; `S` puts this one off first.
            KeyCode::Char('W') => self.next_waiting(false),
            KeyCode::Char('J') if self.view == View::Salon && self.card.is_none() => {
                self.toggle_story()
            }
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
            KeyCode::Char('m') => self.toggle_mark(),
            KeyCode::Char('u') => self.undo(),
            KeyCode::Char('U') => self.grace_now(),
            KeyCode::Char('\'') => self.next_mark(),
            KeyCode::Char('O') => self.quote_output(),
            KeyCode::Up | KeyCode::Char('k') => self.move_selection(false),
            KeyCode::Down | KeyCode::Char('j') => self.move_selection(true),
            KeyCode::Left | KeyCode::Char('h') if self.salon_keys() => self.fold(true),
            KeyCode::Right | KeyCode::Char('l') if self.salon_keys() => self.fold(false),
            KeyCode::Char('f') if self.salon_keys() => self.toggle_needs(),
            KeyCode::Char('s') if self.salon_keys() => self.settle_at_hand(),
            KeyCode::Enter if self.pane == Pane::Journal && self.story_on() => {
                self.open_story_item()
            }
            KeyCode::Enter => self.deeper(),
            KeyCode::Esc if self.on_card() => self.leave_card(),
            KeyCode::Esc if self.story_away && self.zoom == Zoom::None => self.story_back(),
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
            ('z', KeyCode::Char('a')) if self.thread.is_some() => self.toggle_fold(),
            ('z', KeyCode::Char('a')) if self.diff_zoom().is_some() => {
                if let Some(id) = self.diff_zoom() {
                    self.fold_files(id, |f, n| {
                        if let Some(n) = n {
                            f.set(n, !f.folded(n))
                        }
                    });
                }
            }
            ('z', KeyCode::Char(c @ ('M' | 'R'))) if self.diff_zoom().is_some() => {
                if let Some(id) = self.diff_zoom() {
                    self.fold_files(id, |f, _| *f = Folds::every(c == 'M'));
                }
            }
            ('z', KeyCode::Char('M')) if self.thread.is_some() => self.fold_talk(true),
            ('z', KeyCode::Char('R')) if self.thread.is_some() => self.fold_talk(false),
            ('z', KeyCode::Char('a')) if self.focus_zoom().is_some() => {
                if let Some(id) = self.focus_zoom() {
                    let at = self.focus_at(id);
                    let r = &mut self.reading;
                    r.folds.set(at, !r.folds.folded(at));
                    r.follow.set(true);
                }
            }
            ('z', KeyCode::Char(c @ ('M' | 'R'))) if self.focus_zoom().is_some() => {
                self.reading.folds = Folds::every(c == 'M');
                self.reading.follow.set(true);
            }
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
        self.help_scroll = 0;
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
        self.open_waiting(i);
    }

    /// Item `i` waiting for the person, selected in the salon's waiting
    /// panel and opened as `enter` opens it.
    fn open_waiting(&mut self, i: usize) {
        // Cleared off the list: back on it, as it is dealt with.
        self.world.cleared.inbox.remove(&i);
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
        if self.pane == Pane::Journal && !self.story_on() {
            self.select_journal(None);
        }
    }

    /// The timeline's selected agent.
    fn timeline_agent(&self) -> Option<AgentId> {
        let shown = ui::timeline::agents(self);
        let at = self.timeline_sel.min(shown.len().saturating_sub(1));
        shown.get(at).map(|&(id, _)| id)
    }

    /// The agent the person is looking at: focused, selected in the salon,
    /// or the one behind the selected inbox item.
    fn agent_at_hand(&self) -> Option<AgentId> {
        match self.zoom {
            Zoom::Focus(id) | Zoom::Trace(id) | Zoom::Diff(id) => return Some(id),
            Zoom::None => {}
        }
        if self.view == View::Timeline {
            return self.timeline_agent();
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
            Pane::Journal if self.story_on() => match self.story_item()? {
                ui::story::Item::Card(c) => self.world.cards[c].owner,
                ui::story::Item::Line(i) => Some(self.world.journal[i].event.agent()),
            },
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
        if self.copy_move(|_, last, _| if first { 0 } else { last }) {
            return;
        }
        if let Zoom::Diff(id) = self.zoom {
            let lines = self
                .world
                .change_of(id)
                .map_or(0, |c| c.patch.lines().count());
            let last = self.diff_shown(id).step(lines, 0, true);
            self.diff_scroll = if first { 0 } else { last };
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
        if let Some(id) = self.focus_zoom() {
            self.reading_for(id);
            self.reading.sel = first.then_some(0);
            self.reading.follow.set(true);
            return;
        }
        if let Zoom::Trace(id) = self.zoom {
            self.reading_for(id);
            self.reading.trace_top = first.then_some(0);
            return;
        }
        if self.view == View::Timeline && self.zoom == Zoom::None {
            let n = ui::timeline::agents(self).len();
            self.timeline_sel = if first { 0 } else { n.saturating_sub(1) };
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
            Pane::Journal if self.story_on() => {
                let n = self.story_items.borrow().len();
                self.story_sel = if first { 0 } else { n.saturating_sub(1) };
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
        let by = |page: usize| if half { page / 2 } else { page }.max(1);
        if self.copy_move(|at, _, page| match down {
            true => at + by(page),
            false => at.saturating_sub(by(page)),
        }) {
            return;
        }
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
        if let Some(id) = self.focus_zoom() {
            self.focus_page(id, down, by(self.reading.page.get()));
            return;
        }
        if let Zoom::Trace(id) = self.zoom {
            self.reading_for(id);
            let step = by(self.reading.trace_page.get());
            self.trace_page(down, step);
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
        // The scroll goes over the lines shown: a folded file is its header.
        let shown = self.diff_shown(id);
        let step = |at: usize, by: usize, on: bool| shown.step(at, by, on);
        match k.code {
            KeyCode::Esc => {
                self.zoom = Zoom::None;
                self.pane = Pane::Inbox;
            }
            KeyCode::Up | KeyCode::Char('k') => self.diff_scroll = step(self.diff_scroll, 1, false),
            KeyCode::Down | KeyCode::Char('j') => {
                self.diff_scroll = step(self.diff_scroll, 1, true)
            }
            KeyCode::PageUp => self.diff_scroll = step(self.diff_scroll, page, false),
            KeyCode::PageDown | KeyCode::Char(' ') => {
                self.diff_scroll = step(self.diff_scroll, page, true)
            }
            KeyCode::Home => self.diff_scroll = 0,
            KeyCode::End | KeyCode::Char('G') => self.diff_scroll = step(last, 0, true),
            KeyCode::Left | KeyCode::Char('h') => self.fold_files(id, |f, n| {
                if let Some(n) = n {
                    f.set(n, true)
                }
            }),
            KeyCode::Right | KeyCode::Char('l') => self.fold_files(id, |f, n| {
                if let Some(n) = n {
                    f.set(n, false)
                }
            }),
            KeyCode::Char('[') | KeyCode::Char(']') => {
                let starts = self
                    .world
                    .change_of(id)
                    .map(|c| ui::file_starts(&c.patch))
                    .unwrap_or_default();
                if let Some(to) = jump(&starts, self.diff_scroll, k.code == KeyCode::Char(']')) {
                    self.diff_scroll = to;
                }
            }
            KeyCode::Char('a') => self.decide_change(id, true),
            KeyCode::Char('r') => self.decide_change(id, false),
            KeyCode::Char('>') => self.quote_hunk(id),
            KeyCode::Char('c') => self.note_line(id),
            KeyCode::Char('x') => self.choose_hunk(id),
            KeyCode::Char('w') => self.diff_wrap = !self.diff_wrap,
            KeyCode::Char('n') => self.send_notes(id),
            KeyCode::Char('v') => {
                // Lines are chosen from the whole patch: its files open.
                self.diff_folds = None;
                if let Some(c) = self.world.change_of(id) {
                    let at = Copying::new(self.diff_scroll.min(last));
                    self.diff_copy = Some((id, patch_key(&c.patch), at));
                }
            }
            // Writing, views, language, help and quitting work as anywhere.
            _ => return false,
        }
        true
    }

    /// The agent whose focus is open with nothing over it.
    fn focus_zoom(&self) -> Option<AgentId> {
        match self.zoom {
            Zoom::Focus(id) if self.thread.is_none() => Some(id),
            _ => None,
        }
    }

    /// Where the person reads agent `id`, afresh when it was another's.
    fn reading_for(&mut self, id: AgentId) {
        if self.reading.agent != Some(id) {
            self.reading = Reading {
                agent: Some(id),
                ..Reading::default()
            };
        }
    }

    /// What is read follows the agent of the focus or the raw stream:
    /// another agent's starts afresh, leaving both forgets it.
    fn keep_reading(&mut self) {
        match self.zoom {
            Zoom::Focus(id) | Zoom::Trace(id) => self.reading_for(id),
            _ if self.reading.agent.is_some() => self.reading = Reading::default(),
            _ => {}
        }
    }

    /// How many entries agent `id`'s focus tells: its card's story.
    fn story_len(&self, id: AgentId) -> usize {
        (self.world.agents[id].card).map_or(0, |c| self.world.cards[c].story.len())
    }

    /// The entry at hand on agent `id`'s focus, read afresh when it was
    /// another's.
    fn focus_at(&mut self, id: AgentId) -> usize {
        self.reading_for(id);
        self.reading.at(self.story_len(id))
    }

    /// Agent `id`'s focus scrolled `step` rows down (or up), the entry at
    /// hand no longer the newest followed.
    fn focus_page(&mut self, id: AgentId, down: bool, step: usize) {
        let at = self.focus_at(id);
        let r = &mut self.reading;
        r.sel = Some(at);
        r.follow.set(false);
        let top = r.scroll.get();
        r.scroll.set(match down {
            true => top.saturating_add(step),
            false => top.saturating_sub(step),
        });
    }

    /// The raw stream scrolled `step` rows down (or up).
    fn trace_page(&mut self, down: bool, step: usize) {
        let by = isize::try_from(step).unwrap_or(isize::MAX);
        self.reading.trace_by(if down { by } else { -by });
    }

    /// A key on agent `id`'s focus (record 102): `j k` the story entry at
    /// hand, pages scroll, `h l` cut it to its first lines or show it
    /// whole; `false` leaves the key to the rest of the screen.
    fn focus_key(&mut self, id: AgentId, k: KeyEvent) -> bool {
        let entries = self.story_len(id);
        let at = self.focus_at(id);
        let page = self.reading.page.get();
        let r = &mut self.reading;
        match k.code {
            KeyCode::Up | KeyCode::Char('k') => r.sel = Some(at.saturating_sub(1)),
            KeyCode::Down | KeyCode::Char('j') => {
                r.sel = (at + 1 < entries.saturating_sub(1)).then_some(at + 1)
            }
            KeyCode::Home => r.sel = Some(0),
            KeyCode::End => r.sel = None,
            KeyCode::Left | KeyCode::Char('h') => r.folds.set(at, true),
            KeyCode::Right | KeyCode::Char('l') => r.folds.set(at, false),
            KeyCode::PageUp | KeyCode::PageDown => {
                self.focus_page(id, k.code == KeyCode::PageDown, page);
                return true;
            }
            _ => return false,
        }
        self.reading.follow.set(true);
        true
    }

    /// A key on agent `id`'s raw stream: `j k` and pages scroll it, back
    /// to following the newest at its foot; `false` leaves the key to the
    /// rest of the screen.
    fn trace_key(&mut self, id: AgentId, k: KeyEvent) -> bool {
        self.reading_for(id);
        let page = self.reading.trace_page.get();
        match k.code {
            KeyCode::Up | KeyCode::Char('k') => self.trace_page(false, 1),
            KeyCode::Down | KeyCode::Char('j') => self.trace_page(true, 1),
            KeyCode::PageUp => self.trace_page(false, page),
            KeyCode::PageDown => self.trace_page(true, page),
            KeyCode::Home => self.reading.trace_top = Some(0),
            KeyCode::End => self.reading.trace_top = None,
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
            KeyCode::Char('[') | KeyCode::Char(']') => {
                let starts = ui::turn_starts(&self.world, &th.entries);
                if let Some(to) = jump(&starts, th.sel, k.code == KeyCode::Char(']')) {
                    th.sel = to;
                    th.follow.set(true);
                }
            }
            KeyCode::Char('%') => {
                if let Some(to) = ui::reply_jump(&self.world, &th.entries, th.sel) {
                    th.sel = to;
                    th.follow.set(true);
                }
            }
            KeyCode::Char('c') | KeyCode::Char('y') => {
                let one = th.entries[th.sel];
                self.copy = Some(ui::copy_text(self.lang, &self.world, &[one]));
            }
            KeyCode::Char('C') | KeyCode::Char('Y') => {
                self.copy = Some(ui::copy_text(self.lang, &self.world, &th.entries));
            }
            KeyCode::Char('v') => {
                let lines = ui::thread_lines(self.lang, &self.world, &th.entries, th.room.get());
                let first = lines.iter().position(|l| l.entry == th.sel);
                th.copy = Some(Copying::new(first.unwrap_or(0)));
                th.words = None;
                th.follow.set(true);
            }
            KeyCode::Left | KeyCode::Char('h') => {
                th.folds.set(th.sel, true);
                th.follow.set(true);
            }
            KeyCode::Right | KeyCode::Char('l') => {
                th.folds.set(th.sel, false);
                th.follow.set(true);
            }
            KeyCode::Char('n') => self.send_talk_notes(),
            // Writing, views, language, help and quitting work as anywhere.
            _ => return false,
        }
        true
    }

    /// Whether lines are being chosen to copy: in the open conversation,
    /// else in the open change.
    fn copying(&self) -> bool {
        match &self.thread {
            Some(th) => th.copy.is_some(),
            None => matches!(self.zoom, Zoom::Diff(id) if self.diff_copy_of(id).is_some()),
        }
    }

    /// The lines of worker `id`'s change as it is now being chosen to
    /// copy, if they are.
    /// Which files of agent `id`'s change are folded: none unless they
    /// were folded on this very patch.
    pub fn diff_folds_of(&self, id: AgentId) -> Folds {
        let patch = self.world.change_of(id).map(|c| patch_key(&c.patch));
        (self.diff_folds.as_ref())
            .filter(|(a, p, _)| *a == id && Some(*p) == patch)
            .map(|(_, _, f)| f.clone())
            .unwrap_or_default()
    }

    /// The lines agent `id`'s change shows now.
    fn diff_shown(&self, id: AgentId) -> ui::Shown {
        match self.world.change_of(id) {
            Some(c) => ui::Shown::of(&c.patch, &self.diff_folds_of(id)),
            None => ui::Shown::default(),
        }
    }

    /// The files of agent `id`'s change folded as `change` makes them
    /// from what they are; the line at hand, hidden, goes to its file's
    /// header.
    fn fold_files(&mut self, id: AgentId, change: impl FnOnce(&mut Folds, Option<usize>)) {
        let Some(c) = self.world.change_of(id) else {
            return;
        };
        let (key, file) = (patch_key(&c.patch), ui::file_of(&c.patch, self.diff_scroll));
        let mut folds = self.diff_folds_of(id);
        change(&mut folds, file);
        self.diff_folds = Some((id, key, folds));
        let shown = self.diff_shown(id);
        if !shown.lines.contains(&self.diff_scroll) {
            self.diff_scroll = shown.step(self.diff_scroll, 0, true);
        }
    }

    pub fn diff_copy_of(&self, id: AgentId) -> Option<Copying> {
        let patch = self.world.change_of(id).map(|c| patch_key(&c.patch))?;
        (self.diff_copy)
            .filter(|&(a, p, _)| a == id && p == patch)
            .map(|(_, _, c)| c)
    }

    /// A key while lines are being chosen to copy (`v`): the cursor moves
    /// (`j k`, a page, `gg G`), `y` copies, `Y` in a change copies the
    /// new side, `v` and esc leave. In a conversation the cursor also
    /// moves word by word (`w b h l`, the choice turns word-wise), `s`
    /// starts the choice where it is and `c` notes the words chosen. `?` and `q` work as anywhere; any
    /// other key does nothing, so one meant for the change (`a` applies)
    /// is not taken by mistake. `false` when nothing is being chosen.
    fn copy_key(&mut self, k: KeyEvent) -> bool {
        if !self.copying() || k.modifiers.contains(KeyModifiers::CONTROL) {
            return false;
        }
        match k.code {
            KeyCode::Up | KeyCode::Char('k') => {
                self.copy_move(|at, _, _| at.saturating_sub(1));
            }
            KeyCode::Down | KeyCode::Char('j') => {
                self.copy_move(|at, _, _| at + 1);
            }
            KeyCode::PageUp => self.page(false, false),
            KeyCode::PageDown | KeyCode::Char(' ') => self.page(true, false),
            KeyCode::Home => self.jump(true),
            KeyCode::End | KeyCode::Char('G') => self.jump(false),
            KeyCode::Char('g') => self.pending = Some('g'),
            KeyCode::Char('y') => self.copy_chosen(false),
            KeyCode::Char('Y') => self.copy_chosen(true),
            KeyCode::Char('w') | KeyCode::Char('l') | KeyCode::Right => self.word_step(true),
            KeyCode::Char('b') | KeyCode::Char('h') | KeyCode::Left => self.word_step(false),
            KeyCode::Char('s') => self.choose_from_here(),
            KeyCode::Char('c') => self.note_words(),
            KeyCode::Esc | KeyCode::Char('v') => {
                if let Some(th) = &mut self.thread {
                    th.copy = None;
                }
                self.diff_copy = None;
            }
            KeyCode::Char('?') | KeyCode::Char('q') => return false,
            _ => {}
        }
        true
    }

    /// Moves the cursor of the lines being chosen to copy: `to` gets the
    /// cursor, the last line and a page of lines and says where it goes
    /// (no further than the last). The conversation selects the message
    /// the cursor is in and keeps it in view; the change scrolls as
    /// little as it can. `false` when nothing is being chosen.
    fn copy_move(&mut self, to: impl FnOnce(usize, usize, usize) -> usize) -> bool {
        if let Some(th) = &mut self.thread {
            let Some(c) = &mut th.copy else {
                return false;
            };
            let lines = ui::thread_lines(self.lang, &self.world, &th.entries, th.room.get());
            let last = lines.len().saturating_sub(1);
            c.at = to(c.at, last, usize::from(th.page.get()).max(1)).min(last);
            if let Some(l) = lines.get(c.at) {
                th.sel = l.entry;
                if let Some((_, word)) = &mut th.words {
                    *word = (*word).min(l.words.len().saturating_sub(1));
                }
            }
            th.follow.set(true);
            return true;
        }
        let Zoom::Diff(id) = self.zoom else {
            return false;
        };
        let (Some(mut c), Some(change)) = (self.diff_copy_of(id), self.world.change_of(id)) else {
            return false;
        };
        let last = change.patch.lines().count().saturating_sub(1);
        let page = usize::from(self.diff_page.get()).max(1);
        c.at = to(c.at, last, page).min(last);
        self.diff_scroll = ui::in_view(c.at, c.at + 1, self.diff_scroll, page);
        self.diff_copy = Some((id, patch_key(&change.patch), c));
        true
    }

    /// Copies the lines chosen and leaves the choosing: a conversation's
    /// as [`ui::copy_lines`] reads them, a change's as written or, with
    /// `new_side`, as the code reads after it. A new side with nothing
    /// on it is said, and the choice stays to be changed.
    fn copy_chosen(&mut self, new_side: bool) {
        if let Some(th) = &mut self.thread {
            if th.copy.is_none() {
                return;
            }
            let lines = ui::thread_lines(self.lang, &self.world, &th.entries, th.room.get());
            self.copy = Some(ui::chosen_text(&lines, th));
            th.copy = None;
            return;
        }
        let Zoom::Diff(id) = self.zoom else {
            return;
        };
        let (Some(c), Some(change)) = (self.diff_copy_of(id), self.world.change_of(id)) else {
            return;
        };
        let (from, to) = c.span();
        match ui::copy_patch(&change.patch, from, to, new_side) {
            Some(text) => {
                self.copy = Some(text);
                self.diff_copy = None;
            }
            None => {
                let t = self.lang.text();
                self.flash(t.copy_no_new.to_string());
            }
        }
    }

    /// The change open, when no conversation is over it.
    fn diff_zoom(&self) -> Option<AgentId> {
        match self.zoom {
            Zoom::Diff(id) if self.thread.is_none() => Some(id),
            _ => None,
        }
    }

    /// `za` in a conversation: the selected message folded, or open.
    fn toggle_fold(&mut self) {
        if let Some(th) = &mut self.thread {
            let folded = th.folds.folded(th.sel);
            th.folds.set(th.sel, !folded);
            th.follow.set(true);
        }
    }

    /// `zM` (`fold`) or `zR` in a conversation, `:fold` `:unfold` with one
    /// open: every message folded, or open; the conversations opened
    /// after it in this run start so too.
    fn fold_talk(&mut self, fold: bool) {
        self.fold_talk = fold;
        if let Some(th) = &mut self.thread {
            th.folds = Folds::every(fold);
            th.follow.set(true);
        }
    }

    /// `w l →` (`on`) or `b h ←` while choosing in a conversation: the
    /// cursor goes a word on or back, over the end of a line to the next
    /// or the one before; the choice turns word-wise, from the end it
    /// had (the start of its first line, the end of its last).
    fn word_step(&mut self, on: bool) {
        let Some(th) = &mut self.thread else {
            return;
        };
        let Some(c) = &mut th.copy else {
            return;
        };
        let lines = ui::thread_lines(self.lang, &self.world, &th.entries, th.room.get());
        let counts: Vec<usize> = lines.iter().map(|l| l.words.len()).collect();
        let last = |l: usize| counts.get(l).map_or(0, |n| n.saturating_sub(1));
        let (aw, cw) = th.words.unwrap_or(match c.anchor.cmp(&c.at) {
            std::cmp::Ordering::Less => (0, last(c.at)),
            std::cmp::Ordering::Greater => (last(c.anchor), 0),
            std::cmp::Ordering::Equal => (0, 0),
        });
        let (line, word) = ui::word_step(&counts, (c.at, cw), on);
        c.at = line;
        th.words = Some((aw, word));
        if let Some(l) = lines.get(line) {
            th.sel = l.entry;
        }
        th.follow.set(true);
    }

    /// `s` while choosing in a conversation: the choice starts again
    /// where the cursor is, the cursor's word alone when it is word-wise.
    fn choose_from_here(&mut self) {
        if let Some(th) = &mut self.thread
            && let Some(c) = &mut th.copy
        {
            c.anchor = c.at;
            if let Some((aw, cw)) = &mut th.words {
                *aw = *cw;
            }
        }
    }

    /// `c` while choosing in a conversation: a note on the words chosen,
    /// written in the command line; one on the same words is written
    /// again. The choosing ends. Blank words take no note. The note goes
    /// to the agent of the line the words start in: whoever wrote it, or,
    /// the person's own line, the agent it went to (`Event::agent`).
    fn note_words(&mut self) {
        let Some(th) = &mut self.thread else {
            return;
        };
        let Some(c) = th.copy else {
            return;
        };
        let lines = ui::thread_lines(self.lang, &self.world, &th.entries, th.room.get());
        let quote = ui::chosen_text(&lines, th);
        if quote.trim().is_empty() {
            return self.flash(self.lang.text().note_blank);
        }
        let Some(&entry) = (lines.get(c.span().0)).and_then(|l| th.entries.get(l.entry)) else {
            return;
        };
        th.copy = None;
        let to = self.world.journal[entry].event.agent();
        let had = (self.talk_notes.iter())
            .find(|n| n.entry == entry && n.quote == quote)
            .map(|n| n.text.clone());
        self.input = had.unwrap_or_default();
        self.after = 0;
        self.typing = true;
        self.menu = None;
        self.noting = Some(Noting::Words(TalkNote {
            to,
            entry,
            quote,
            text: String::new(),
        }));
    }

    /// The notes on words of the open conversation, oldest line first.
    pub fn talk_notes_here(&self) -> Vec<&TalkNote> {
        let Some(th) = &self.thread else {
            return Vec::new();
        };
        let mut notes: Vec<&TalkNote> = (self.talk_notes.iter())
            .filter(|n| th.entries.contains(&n.entry))
            .collect();
        notes.sort_by_key(|n| n.entry);
        notes
    }

    /// The note written on `note`'s words is kept (in place of one there
    /// was); an empty one takes it away.
    fn keep_talk_note(&mut self, mut note: TalkNote, text: String) {
        let t = self.lang.text();
        self.talk_notes
            .retain(|n| !(n.entry == note.entry && n.quote == note.quote));
        if text.is_empty() {
            return self.flash(t.note_dropped);
        }
        note.text = text;
        self.talk_notes.push(note);
        let n = self.talk_notes_here().len().to_string();
        self.flash(fill(t.talk_noted, &[&n]));
    }

    /// `n` in a conversation: its notes go to the agents they are for,
    /// one message each, every note under the words it is about. A note
    /// whose message could not go stays.
    fn send_talk_notes(&mut self) {
        let t = self.lang.text();
        let here: Vec<TalkNote> = self.talk_notes_here().into_iter().cloned().collect();
        if here.is_empty() {
            return self.flash(t.no_talk_notes);
        }
        let mut by: BTreeMap<AgentId, Vec<&TalkNote>> = BTreeMap::new();
        for n in &here {
            by.entry(n.to).or_default().push(n);
        }
        let (mut sent, mut names) = (0, Vec::new());
        for (to, notes) in by {
            let message = ui::talk_notes_message(&self.world, &notes);
            if self.tell(to, &message) {
                self.talk_notes.retain(|n| !notes.contains(&n));
                sent += notes.len();
                names.push(self.world.agents[to].name.clone());
            }
        }
        if sent > 0 {
            let t = self.lang.text();
            self.flash(fill(
                t.talk_notes_sent,
                &[&sent.to_string(), &names.join(", ")],
            ));
        }
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
            copy: None,
            words: None,
            folds: Folds::every(self.fold_talk),
            room: Cell::new(usize::MAX),
        });
    }

    /// Where a command of the person's own runs: the checkout, or where
    /// the demo was started.
    pub fn cwd(&self) -> PathBuf {
        match &self.engine {
            Engine::Live(l) => l.repo_of(self.current_project()).to_path_buf(),
            Engine::Sim(_) | Engine::View(_) => PathBuf::from("."),
        }
    }

    /// `:copy [PATH]`: the paths git ignores that are copied from the
    /// project at hand's checkout into its workers' worktrees at their
    /// start (said when none is given, with a word on its
    /// `.worktreeinclude` when it has one), one more (checked: literal,
    /// inside, ignored; the demo checks only the first), or one fewer
    /// when it is listed already. A trailing `/` is the same path.
    fn copy_path(&mut self, arg: &str) {
        let t = self.lang.text();
        let p = self.current_project();
        let name = self.world.projects[p].name.clone();
        if arg.is_empty() {
            let includes = match &self.engine {
                Engine::Live(l) => l.includes(p),
                Engine::Sim(s) => s.includes(p),
                Engine::View(_) => false,
            };
            let copies = &self.world.projects[p].copies;
            let said = match (copies.is_empty(), includes) {
                (true, false) => fill(t.copy_none, &[&name]),
                (true, true) => fill(t.copy_file, &[&name]),
                (false, false) => fill(t.copy_list, &[&name, &copies.join(", ")]),
                (false, true) => fill(
                    t.copy_also,
                    &[&fill(t.copy_list, &[&name, &copies.join(", ")])],
                ),
            };
            return self.flash(said);
        }
        let path = arg.trim_end_matches('/');
        let copies = &mut self.world.projects[p].copies;
        if let Some(i) = copies.iter().position(|c| c == path) {
            copies.remove(i);
            return self.flash(fill(t.copy_removed, &[path]));
        }
        let checked = match &self.engine {
            Engine::Live(l) => l.may_copy(p, path),
            Engine::Sim(_) => imece_agent::copy::literal(path),
            Engine::View(_) => return self.read_only(),
        };
        match checked {
            Ok(()) => {
                self.world.projects[p].copies.push(path.to_string());
                self.flash(fill(t.copy_added, &[path, &name]));
            }
            Err(why) => self.flash(fill(t.copy_refused, &[path, &why])),
        }
    }

    /// `:commit`: a cheap model is asked for a commit message for what is
    /// staged in the project at hand's checkout; it comes to the command
    /// line when it answers (see [`App::show_suggestion`]).
    fn suggest_commit(&mut self) {
        let t = self.lang.text();
        let p = self.current_project();
        let asked = match &mut self.engine {
            Engine::Live(l) => l.suggest(&self.world, p),
            Engine::Sim(s) => s.suggest(&self.world, p),
            Engine::View(_) => return self.read_only(),
        };
        match asked {
            Ok(model) => {
                let name = self.world.projects[p].name.clone();
                let asking = fill(t.suggesting, &[&model, &name]);
                self.asking = Some(asking.clone());
                self.flash(asking);
            }
            Err(why) => self.flash(fill(t.suggest_refused, &[&why])),
        }
    }

    /// A suggested commit message, once the command line is free: it
    /// opens as a `!git commit` of the message to edit and run, in the
    /// project's checkout (`-C` when that is not the one at hand). The
    /// demo's commits nothing (`--dry-run`): its message was made up.
    fn show_suggestion(&mut self) {
        if self.typing {
            return;
        }
        let Some(s) = self.world.suggested.take() else {
            return;
        };
        let dir = match &self.engine {
            Engine::Live(l) if s.project != self.current_project() => {
                Some(l.repo_of(s.project).to_path_buf())
            }
            _ => None,
        };
        self.input = commit_line(&s.text, dir.as_deref(), !self.live());
        self.after = 0;
        self.typing = true;
        self.menu = None;
    }

    /// `^x ^e`, `:edit`: `draft` goes to the person's editor; the main
    /// loop takes it from `editor`. Not in an archive's view, whose line
    /// sends nothing.
    fn open_editor(&mut self, draft: String) {
        if self.viewing_archive() {
            return self.read_only();
        }
        self.menu = None;
        self.editor = Some(draft);
    }

    /// The line under what an editor that gave the draft back left on the
    /// screen, before the screen comes back.
    pub fn editor_back(&self, editor: &str, why: &editor::Why) -> String {
        let why = ui::words::editor_why(self.lang, editor, why);
        fill(self.lang.text().editor_back, &[&why])
    }

    /// The editor closed on `draft`: what it left is the line now, the
    /// cursor at its end, to send with `enter` or change first; nothing is
    /// sent. When it failed (`editor_back` said why) the draft is the line
    /// again, as it was.
    pub fn edited(&mut self, draft: &str, back: Result<String, editor::Why>) {
        self.typing = true;
        self.input = match back {
            Ok(text) => pasted(&text),
            Err(_) => draft.to_string(),
        };
        self.after = 0;
        self.recall = None;
        self.searching = false;
        self.recalling = false;
        self.menu = self.suggest();
    }

    /// The line under a `!` command's output, before the screen comes
    /// back.
    pub fn shell_back(&self, ran: Ran) -> String {
        ui::words::shell_back(self.lang, ran.ended, millis(ran.took))
    }

    /// A `!` command ran (or could not be started): the journal keeps it
    /// under the project at hand, the footer says how it went, and its
    /// output is the project's for `O`.
    pub fn ran(&mut self, command: &str, how: io::Result<Caught>) {
        match how {
            Ok(caught) => {
                let Ran { ended, took } = caught.ran;
                let project = self.current_project();
                self.outputs.insert(project, Output::of(command, &caught));
                let leader = self.world.projects[project].leader;
                let took = Some(millis(took));
                self.world.note(Event::Shell {
                    leader,
                    command: command.to_string(),
                    ended,
                    took,
                });
                self.flash(ui::words::shell(self.lang, command, ended, took));
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
        // `^x ^e`, as bash and zsh have it: the line in the editor. Any
        // other key after `^x` does what it does alone.
        let after_x = std::mem::take(&mut self.ctrl_x);
        match k.code {
            KeyCode::Char('e') if ctrl && after_x => self.open_editor(self.input.clone()),
            KeyCode::Char('x') if ctrl => self.ctrl_x = true,
            KeyCode::Char('n') if ctrl => self.pick(true),
            KeyCode::Char('p') if ctrl => self.pick(false),
            KeyCode::Char('r') if ctrl => {
                let words = self.input.clone();
                self.search_history(&words);
            }
            KeyCode::Enter if newline => {
                self.edit(k);
            }
            // Searching: enter takes the line under the highlight (the words
            // typed stay when nothing has them), tab steps on.
            KeyCode::Enter if self.searching && self.recalling => self.take_recall(),
            KeyCode::Enter if self.searching => self.take_sent(),
            KeyCode::Tab if self.searching => {
                if let Some(m) = &mut self.menu {
                    m.step(true);
                }
            }
            // At the text's first or last line, up and down bring back the
            // lines sent, as a shell does; between lines they move.
            KeyCode::Up | KeyCode::Down if self.menu.is_none() => {
                let (back, at) = (k.code == KeyCode::Up, self.cursor());
                let at_end = if back {
                    edit::line_start(&self.input, at) == 0
                } else {
                    edit::line_end(&self.input, at) == self.input.len()
                };
                if at_end {
                    self.recall(back);
                } else {
                    self.edit(k);
                }
            }
            KeyCode::Enter => match &mut self.menu {
                Some(m) if !m.inserted => {
                    m.insert(&mut self.input);
                    // A card or a place to go is all there is to take: it
                    // opens at once.
                    if self.typing_card() || self.typing_jump() {
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
                self.noting = None;
                self.input.clear();
                self.after = 0;
                self.jumps.clear();
                self.recall = None;
                self.searching = false;
            }
            // Right after `&`: a digit goes to the nth at once, `#` turns
            // to the cards.
            KeyCode::Char(d) if ('1'..='9').contains(&d) && self.input == "&" => {
                self.input.push(d);
                self.submit();
            }
            KeyCode::Char('#') if self.input == "&" => {
                self.input = "#".into();
                self.jumps.clear();
                self.menu = self.suggest();
            }
            KeyCode::Tab => match &mut self.menu {
                None if self.after > 0 => {}
                None => {
                    self.menu = self.suggest_with(true);
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
        if self.input != was {
            // Written on: the line is the person's own again.
            self.recall = None;
        }
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
    /// drops it (backspace past its start leaves it too); the moving keys
    /// scroll it where it does not fit the screen; any other key closes
    /// the help.
    fn help_key(&mut self, k: KeyEvent) {
        if !self.help_typing {
            let (room, last) = self.help_room.get();
            let half = (room / 2).max(1);
            let ctrl = k.modifiers.contains(KeyModifiers::CONTROL);
            let at = self.help_scroll.min(last);
            let g = self.pending.take() == Some('g');
            self.help_scroll = match k.code {
                KeyCode::Char('/') => {
                    self.help_typing = true;
                    return;
                }
                KeyCode::Char('d') if ctrl => at + half,
                KeyCode::Char('u') if ctrl => at.saturating_sub(half),
                KeyCode::Char('f') if ctrl => at + room,
                KeyCode::Char('b') if ctrl => at.saturating_sub(room),
                _ if ctrl => return self.help = false,
                KeyCode::Down | KeyCode::Char('j') => at + 1,
                KeyCode::Up | KeyCode::Char('k') => at.saturating_sub(1),
                KeyCode::PageDown => at + room,
                KeyCode::PageUp => at.saturating_sub(room),
                KeyCode::Char('g') if g => 0,
                KeyCode::Char('g') => {
                    self.pending = Some('g');
                    return;
                }
                KeyCode::Home => 0,
                KeyCode::Char('G') | KeyCode::End => last,
                _ => return self.help = false,
            }
            .min(last);
            return;
        }
        // The filter changes what is shown: from its top again.
        self.help_scroll = 0;
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
        if let Some(v) = &mut self.viewing {
            v.paste(text);
            return self.after_view();
        }
        self.paste_in(text);
        self.preview_theme();
    }

    fn paste_in(&mut self, text: &str) {
        // Into the help's filter while it is typed, on one line.
        if self.help && self.help_typing {
            let text = pasted(text).replace(['\n', '\t'], " ");
            return self.help_find.push_str(&text);
        }
        if let Some(b) = &mut self.archives {
            if !b.paste(&self.world, &pasted(text)) {
                self.flash(self.lang.text().paste_not_writing);
            }
            return;
        }
        let open = self.help || self.solo_pick.is_some() || self.window_open() || self.leaving;
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
        self.suggest_with(false)
    }

    /// [`suggest`](Self::suggest); `forced` when `tab` asked, which
    /// completes any word of a message to a file.
    fn suggest_with(&self, forced: bool) -> Option<Menu> {
        if self.input.contains('\n') || self.noting.is_some() {
            return None;
        }
        if self.searching && self.recalling {
            return self.recall_menu();
        }
        if self.searching {
            let w = &self.world;
            let lang = self.lang;
            return complete::history(&self.input, &w.sent, |s| {
                ui::words::ago(lang, w.tick.saturating_sub(s.tick) * SECONDS_PER_TICK)
            });
        }
        if let Some(rest) = self.input.strip_prefix('&') {
            return complete::jumps(rest, &self.jumps);
        }
        if let Some(m) = self.choice_menu() {
            return Some(m);
        }
        let Some(mut m) = complete::suggest(
            &self.input,
            &self.world,
            self.current_project(),
            self.home.as_deref(),
        ) else {
            return self.file_menu(forced);
        };
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
            "edit" => Some(editor::of_env()),
            "theme" => Some(theme::pal().name.clone()),
            "lang" => Some(t.language.to_string()),
            "close" | "rename" => Some(self.world.projects[self.current_project()].name.clone()),
            // The project at hand, marked as the salon marks it when
            // pinned (then `:pin` unpins it).
            "pin" => {
                let p = &self.world.projects[self.current_project()];
                let mark = if p.pinned { "▴ " } else { "" };
                Some(format!("{mark}{}", p.name))
            }
            "forget" | "fork" | "talk" | "trace" | "peek" | "why" | "yes" => self
                .agent_at_hand()
                .map(|id| self.world.agents[id].name.clone()),
            "next" | "waiting" => {
                let open = self.world.open_inbox().len().to_string();
                Some(fill(t.value_waiting, &[&open]))
            }
            "needs" => Some(on_off(self.needs_only)),
            "story" => Some(on_off(self.story)),
            "news" => Some(welcome::VERSION.to_string()),
            "settle" => Some(match self.selected {
                Sel::Agent(id) => self.world.agents[id].name.clone(),
                Sel::Project(p) => self.world.projects[p].name.clone(),
            }),
            "unsettle" => Some((self.settled.len() + self.settled_projects.len()).to_string()),
            "awake" => Some(on_off(self.awake.on)),
            "notify" => Some(self.notify.name().to_string()),
            "motion" => Some(self.motion.name().to_string()),
            "away" => Some(self.away.name()),
            "keys" => Some(self.keys.len().to_string()),
            "pause" => Some(on_off(self.paused)),
            "faster" | "slower" => Some(format!("×{}", self.speed)),
            "export" if self.thread.is_some() => Some(t.value_thread.to_string()),
            "export" => self.card.map(|c| format!("#{}", c + 1)),
            // The command whose output `:output` quotes.
            "output" => (self.outputs.get(&self.current_project())).map(|o| o.command.clone()),
            "copy" => {
                let copies = &self.world.projects[self.current_project()].copies;
                (!copies.is_empty()).then(|| copies.join(", "))
            }
            "allow" => {
                let rules = &self.world.projects[self.current_project()].allowed;
                let lines: Vec<String> = rules.iter().map(Standing::line).collect();
                (!lines.is_empty()).then(|| lines.join(", "))
            }
            // What the solo at hand runs on, the part the command sets.
            "effort" | "model" => self.solo_at_hand().map(|id| {
                let c = Choice::parse(self.model_of(id).as_deref().unwrap_or_default());
                let part = if name == "effort" { c.effort } else { c.model };
                part.unwrap_or_else(|| complete::DEFAULT.to_string())
            }),
            _ => None,
        };
        let bill = matches!(name, "forget" | "fork" | "open" | "solo" | "commit");
        match (value, bill) {
            (Some(v), true) => format!("({v}) $"),
            (Some(v), false) => format!("({v})"),
            (None, true) => "$".to_string(),
            (None, false) => String::new(),
        }
    }

    /// The last word of a message or a `!` line, completed to a file of
    /// the project at hand, or a symbol leased on its code map
    /// (`file::name`), the holder of the file as the note (see
    /// [`complete::files`]). Not on the lines that are something else
    /// (`:` `#` `&`, a search), nor for a name (`@`) or a command (`/`).
    fn file_menu(&self, forced: bool) -> Option<Menu> {
        if self.input.starts_with([':', '#', '&']) {
            return None;
        }
        let word = self
            .input
            .rsplit(char::is_whitespace)
            .next()
            .unwrap_or_default();
        let word = if word.len() == self.input.len() {
            word.strip_prefix('!').unwrap_or(word)
        } else {
            word
        };
        if word.is_empty() || word.starts_with(['@', '/']) {
            return None;
        }
        let start = self.input.len() - word.len();
        let w = &self.world;
        let p = self.current_project();
        let paths = w.files_of(p);
        let held: Vec<(&str, &str, AgentId)> = w
            .cards
            .iter()
            .filter(|c| c.project == p && c.holds_leases())
            .filter_map(|c| c.owner.map(|o| (c, o)))
            .flat_map(|(c, o)| c.symbols.iter().map(move |s| (s.file, s.name, o)))
            .collect();
        let note = |file: &str| -> String {
            held.iter()
                .find(|h| h.0 == file)
                .map(|h| {
                    let a = &w.agents[h.2];
                    format!("{} {}", ui::glyph(a).0, a.name)
                })
                .unwrap_or_default()
        };
        let symbols: Vec<(&str, &str)> = held.iter().map(|h| (h.0, h.1)).collect();
        complete::files(start, word, forced, &paths, &symbols, note)
    }

    /// Where the terminal's own cursor stands and what it looks like, while
    /// the person types: the window on top takes it (the help's or the
    /// archives' `/` filter), a window on top that takes no text hides it,
    /// else the command line has it while it is open. `None`: hidden.
    pub fn caret(&self) -> Option<(Field, Caret)> {
        if let Some(v) = &self.viewing {
            return v.caret();
        }
        if self.solo_pick.is_some() {
            return None;
        }
        if let Some(b) = &self.archives {
            return b.typing.then_some((Field::Archives, Caret::Find));
        }
        if self.help {
            return self.help_typing.then_some((Field::Help, Caret::Find));
        }
        let window = !self.news.is_empty()
            || self.welcome
            || self.digest.is_some()
            || self.left_off.is_some()
            || self.leaving;
        if window || !self.typing {
            return None;
        }
        // In the order the line's heading names what is written
        // (`ui::writing`).
        let caret = if self.noting.is_some() {
            Caret::Write
        } else if self.searching {
            Caret::Find
        } else if self.typing_shell() {
            Caret::Write
        } else if self.typing_card() || self.typing_jump() {
            Caret::Find
        } else if self.typing_command() {
            Caret::Command
        } else {
            Caret::Write
        };
        Some((Field::Line, caret))
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

    /// Whether what is being typed names a place to go (`&claude-1`, `&3`).
    pub fn typing_jump(&self) -> bool {
        self.input.starts_with('&')
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
        self.end_preview();
        let text = self.input.trim().to_string();
        let target = self.typing_target();
        self.input.clear();
        self.after = 0;
        self.typing = false;
        self.menu = None;
        self.recall = None;
        self.searching = false;
        match self.noting.take() {
            Some(Noting::Line(id, line)) => return self.keep_note(id, line, text),
            Some(Noting::Words(note)) => return self.keep_talk_note(note, text),
            None => {}
        }
        if text.is_empty() {
            return;
        }
        if recalled(&text) {
            self.world.recall_line(&text);
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
        if let Some(rest) = text.strip_prefix('&') {
            let list = std::mem::take(&mut self.jumps);
            return match complete::jump_named(&text, &list) {
                Some(name) => self.goto(&name),
                // A number past the list's end is nowhere; a bare `&` is
                // nothing to do.
                None => {
                    let rest = rest.trim_start_matches('!').trim();
                    if !rest.is_empty() {
                        self.flash(fill(self.lang.text().no_jump, &[rest]));
                    }
                }
            };
        }
        if let Some(command) = text.strip_prefix('!') {
            let command = command.trim();
            if command.is_empty() {
                return self.flash(t.shell_empty);
            }
            if self.viewing_archive() {
                return self.read_only();
            }
            self.shell = Some(command.to_string());
            return;
        }
        let Some(rest) = text.strip_prefix('@') else {
            let leader = self.world.projects[self.current_project()].leader;
            self.tell(leader, &text);
            return;
        };
        let msg = rest
            .split_once(char::is_whitespace)
            .map_or("", |(_, m)| m)
            .trim();
        match self.world.agent_by_name(&target) {
            Some(_) if msg.is_empty() => self.flash(t.empty_message),
            Some(id) => {
                self.tell(id, msg);
            }
            None => self.flash(fill(t.no_agent, &[&target])),
        }
    }

    /// The person's message to agent `id`: the engine gets it first, and only
    /// a message that reached the agent is recorded as said. A line that
    /// starts with `/` is a command of the agent's own command line
    /// (`/clear`, `/compact`, `/cost`), not a message. Whether it went.
    fn tell(&mut self, id: AgentId, text: &str) -> bool {
        let command = text.starts_with('/');
        let refused = match &mut self.engine {
            Engine::Sim(s) if command => s.command(&mut self.world, id, text).err(),
            Engine::Sim(s) => s.told(&mut self.world, id, text).err(),
            Engine::Live(l) if command => l.command(&mut self.world, id, text).err(),
            Engine::Live(l) => l.told(&mut self.world, id, text).err(),
            Engine::View(_) => {
                self.read_only();
                return false;
            }
        };
        let went = refused.is_none();
        self.report(id, text, refused);
        went
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
        let long = complete::long(name);
        if self.viewing_archive() && complete::COMMANDS.contains(&long) {
            if long == "restore" {
                self.restore_asked = true;
                return;
            }
            if !VIEW_COMMANDS.contains(&long) {
                return self.read_only();
            }
        }
        match long {
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
            "solo" => self.solo_project(arg),
            "effort" => self.rechoose(Part::Effort, arg),
            "model" => self.rechoose(Part::Model, arg),
            "close" => self.close_project(arg),
            "archive" => self.archive_project(arg),
            "restore" => self.restore_project(arg),
            "archives" => self.open_archives(),
            "undo" => self.undo(),
            "now" => self.grace_now(),
            "rename" => self.rename_project(arg),
            "forget" => self.forget_agent(arg),
            "fork" => self.fork_agent(arg),
            "goto" => self.goto(arg),
            "history" => self.search_history(arg),
            "recall" => self.search_all(arg),
            "export" => self.export(arg),
            "commit" => self.suggest_commit(),
            "copy" => self.copy_path(arg),
            "allow" => self.allow(arg),
            "pin" => self.pin(arg),
            "mark" => self.toggle_mark(),
            "output" => self.quote_output(),
            "edit" => self.open_editor(arg.to_string()),
            "salon" => self.show(View::Salon),
            "board" => self.show(View::Board),
            "code" => self.show(View::Code),
            "timeline" => self.show(View::Timeline),
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
            "story" => {
                self.show(View::Salon);
                self.toggle_story();
            }
            which @ ("fold" | "unfold") if self.thread.is_none() && self.diff_zoom().is_some() => {
                if let Some(id) = self.diff_zoom() {
                    self.fold_files(id, |f, _| *f = Folds::every(which == "fold"));
                }
            }
            which @ ("fold" | "unfold") if self.thread.is_some() => {
                self.fold_talk(which == "fold");
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
            "peek" => {
                if let Some(id) = self.agent_named(arg) {
                    self.peek = Some(id);
                }
            }
            "yes" => {
                if let Some(id) = self.agent_named(arg) {
                    match self.focus_ask(id) {
                        Some(item) if self.world.inbox[item].kind == InboxKind::Approval => {
                            self.answer_turn(item)
                        }
                        _ => {
                            let t = self.lang.text();
                            self.flash(fill(t.turn_yes_none, &[&self.world.agents[id].name]));
                        }
                    }
                }
            }
            "why" => {
                if let Some(id) = self.agent_named(arg) {
                    let said = ui::words::why(self.lang, &self.world, id, self.why(id));
                    self.flash(said);
                }
            }
            "hints" => self.open_hints(),
            "tidy" => self.open_tidy(),
            "welcome" => self.open_welcome(),
            "news" => self.open_news(),
            "settle" => {
                self.go_salon();
                self.settle_at_hand();
            }
            "unsettle" => self.unsettle(),
            "clear" => self.clear(arg),
            "unclear" => self.unclear(arg),
            "talk" => {
                if let Some(id) = self.agent_named(arg) {
                    self.open_talk_with(id);
                }
            }
            "pause" | "faster" | "slower" if self.live() => self.flash(fill(t.demo_only, &[name])),
            "pause" => self.pause(),
            "faster" => self.faster(),
            "slower" => self.slower(),
            "awake" => {
                self.awake.on = !self.awake.on;
                let t = self.lang.text();
                self.flash(if self.awake.on {
                    t.awake_on
                } else {
                    t.awake_off
                });
                self.keep_awake();
            }
            "keys" => self.read_keys(),
            "notify" => match (arg, Notify::parse(arg)) {
                ("", _) => self.set_notify(self.notify.next()),
                (_, Some(how)) => self.set_notify(how),
                (_, None) => self.flash(fill(t.bad_notify, &[arg])),
            },
            "motion" => match (arg, Motion::parse(arg)) {
                ("", _) => self.set_motion(self.motion.next()),
                (_, Some(how)) => self.set_motion(how),
                (_, None) => self.flash(fill(t.bad_motion, &[arg])),
            },
            "away" => match (arg, Away::parse(arg)) {
                ("", _) => self.set_away(self.away.next()),
                (_, Some(away)) => self.set_away(away),
                (_, None) => self.flash(fill(t.bad_away, &[arg])),
            },
            "digest" => self.open_digest(),
            _ => self.flash(fill(t.no_command, &[name])),
        }
    }

    /// `&`: the line that goes to an agent or a project by name, its menu
    /// the places as they stand now, what needs the person first.
    fn open_jump(&mut self) {
        self.jumps = self.jump_list();
        self.typing = true;
        self.noting = None;
        self.input = "&".into();
        self.after = 0;
        self.menu = self.suggest();
    }

    /// Where `&` may go, in the order its menu offers: the agents of open
    /// projects that need the person first (stuck, waiting, done and not
    /// seen), the rest by their rung (working before idle), the salon's
    /// order among equals; then the open projects. Each agent with its
    /// glyph and status words.
    fn jump_list(&self) -> Vec<Jump> {
        let w = &self.world;
        let needs = self.needs();
        let rungs = self.rungs();
        let order = w.salon_order();
        let place = |p: ProjectId| order.iter().position(|&o| o == p).unwrap_or(usize::MAX);
        let mut agents: Vec<&Agent> = w
            .agents
            .iter()
            .filter(|a| !a.gone && !w.projects[a.project].closed)
            .collect();
        agents.sort_by_key(|a| {
            (
                std::cmp::Reverse(needs[a.id]),
                std::cmp::Reverse(rungs[a.id]),
                place(a.project),
                a.id,
            )
        });
        let mut out: Vec<Jump> = agents
            .iter()
            .map(|a| {
                let glyph = ui::rung_glyph(a, rungs[a.id]).0;
                // As its row words it: done and not seen says so.
                let status: String = if self.done_unseen(a.id) {
                    self.lang.text().done_unseen.to_string()
                } else {
                    ui::status_spans(self.lang, w, a)
                        .iter()
                        .map(|s| s.content.as_ref())
                        .collect::<String>()
                        .split_whitespace()
                        .collect::<Vec<_>>()
                        .join(" ")
                };
                let status: String = status.chars().take(30).collect();
                Jump {
                    name: a.name.clone(),
                    note: format!("{glyph} {status}").trim_end().to_string(),
                    needs: needs[a.id] != Need::Nothing,
                }
            })
            .collect();
        let t = self.lang.text();
        out.extend(order.iter().map(|&p| Jump {
            name: w.projects[p].name.clone(),
            note: t.jump_project.to_string(),
            needs: false,
        }));
        out
    }

    /// `:goto NAME`, or what `&` was given: to the agent of that name, else
    /// the open project; a bare one opens the `&` line.
    fn goto(&mut self, name: &str) {
        let name = name.strip_prefix('@').unwrap_or(name).trim();
        if name.is_empty() {
            return self.open_jump();
        }
        // As the menu matches: case and Turkish letters aside.
        let want = ui::fold(name);
        let agent = self
            .world
            .agents
            .iter()
            .find(|a| {
                !a.gone && !self.world.projects[a.project].closed && ui::fold(&a.name) == want
            })
            .map(|a| a.id);
        if let Some(id) = agent {
            return self.go_agent(id);
        }
        let project = self
            .world
            .open_projects()
            .find(|p| ui::fold(&p.name) == want)
            .map(|p| p.id);
        match project {
            Some(p) => {
                self.go_salon();
                self.thread = None;
                self.settled_projects.remove(&p);
                self.selected = Sel::Project(p);
            }
            None => self.flash(fill(self.lang.text().no_jump, &[name])),
        }
    }

    /// `↑` (`back`) and `↓` on the command line: the line sent before the
    /// one shown (the newest first), or the one after; past the newest,
    /// what was being written comes back; before the oldest it stays.
    fn recall(&mut self, back: bool) {
        let n = self.world.sent.len();
        let shown = self.recall.as_ref().map(|(i, _)| *i);
        let next = match (shown, back) {
            (None, true) => n.checked_sub(1),
            (None, false) => None,
            (Some(i), true) => i.checked_sub(1),
            (Some(i), false) => (i + 1 < n).then_some(i + 1),
        };
        match next {
            Some(i) => {
                match &mut self.recall {
                    Some((at, _)) => *at = i,
                    None => self.recall = Some((i, self.input.clone())),
                }
                self.input = self.world.sent[i].text.clone();
            }
            None if !back => {
                let Some((_, mine)) = self.recall.take() else {
                    return;
                };
                self.input = mine;
            }
            None => return,
        }
        self.after = 0;
        self.menu = None;
        self.searching = false;
    }

    /// `^r`, `:history WORDS`: the line open with `words`, the menu listing
    /// the lines sent that have them; said when nothing was sent yet.
    fn search_history(&mut self, words: &str) {
        if self.world.sent.is_empty() {
            return self.flash(self.lang.text().no_history.to_string());
        }
        self.typing = true;
        self.noting = None;
        self.searching = true;
        self.recalling = false;
        self.recall = None;
        self.input = words.trim().to_string();
        self.after = 0;
        self.menu = self.suggest();
    }

    /// `H`, `:recall WORDS`: the line open with `words`, the menu listing
    /// the journal and trace lines of every project and archive that
    /// have them, the newest first (record 93). An archive's view only
    /// looks at its own.
    fn search_all(&mut self, words: &str) {
        if matches!(self.engine, Engine::View(_)) {
            return self.read_only();
        }
        self.typing = true;
        self.noting = None;
        self.searching = true;
        self.recalling = true;
        self.recall = None;
        self.input = words.trim().to_string();
        self.after = 0;
        self.menu = self.suggest();
    }

    /// The lines that have the terms of `words`: the store's (live), what
    /// the world holds (the demo). The store's name agents and projects
    /// by the world's ids: both are saved together and never shrink.
    fn recall_hits(&self, words: &str) -> Result<Vec<recall::Hit>, String> {
        match &self.engine {
            Engine::Live(l) => l.recall(&self.world, words, recall::SHOWN),
            Engine::Sim(_) => Ok(recall::in_world(&self.world, words, recall::SHOWN)),
            Engine::View(_) => Ok(Vec::new()),
        }
    }

    /// The `:recall` menu for what is typed: each line as the journal
    /// words it (a trace line cleaned), and who, where and when.
    fn recall_menu(&self) -> Option<Menu> {
        let hits = self.recall_hits(&self.input).ok()?;
        let (w, t) = (&self.world, self.lang.text());
        let today = day(w.tick);
        complete::recall(
            &hits,
            |h| match &h.line {
                recall::Line::Journal { entry, .. } => ui::words::event(self.lang, w, &entry.event),
                recall::Line::Trace(text) => clean(text),
            },
            |h| {
                let (who, place) = (&w.agents[h.agent].name, &w.projects[h.project].name);
                match h.tick {
                    Some(tick) => fill(t.recall_note, &[who, place, &when(tick, today)]),
                    None => fill(t.recall_note_untimed, &[who, place]),
                }
            },
        )
    }

    /// Enter while recalling: to the line under the highlight; said when
    /// nothing has the words.
    fn take_recall(&mut self) {
        let chosen = self.menu.as_ref().map(|m| m.chosen);
        let words = std::mem::take(&mut self.input);
        let hits = self.recall_hits(&words);
        self.typing = false;
        self.searching = false;
        self.recalling = false;
        self.menu = None;
        self.after = 0;
        let t = self.lang.text();
        match hits {
            Err(why) => self.flash(fill(t.recall_failed, &[&why])),
            Ok(hits) => match chosen.and_then(|c| hits.into_iter().nth(c)) {
                Some(hit) => self.go_recalled(hit),
                None => self.flash(fill(t.recall_none, &[words.trim()])),
            },
        }
    }

    /// To a line `:recall` found: a journal line's conversation (its
    /// line selected in the journal when it shows there), a trace line's
    /// agent's raw stream; in an archive's view when its project is put
    /// away. A line the screen does not hold (older than what was loaded,
    /// or of a closed project) is said instead.
    fn go_recalled(&mut self, hit: recall::Hit) {
        let p = hit.project;
        if self.world.projects[p].archived.is_some() {
            self.view_archive(p, Some(hit.agent));
            if let Some(v) = &mut self.viewing {
                match hit.line {
                    recall::Line::Journal { entry, .. } => {
                        if let Some(i) = v.world.journal.iter().rposition(|e| *e == entry) {
                            v.show_line(i);
                        }
                    }
                    recall::Line::Trace(_) => v.trace(hit.agent),
                }
            }
            return;
        }
        match hit.line {
            recall::Line::Journal { at: Some(i), .. } => self.show_line(i),
            recall::Line::Trace(_) if !self.world.projects[p].closed => {
                self.go_agent(hit.agent);
                self.trace(hit.agent);
            }
            _ => {
                let t = self.lang.text();
                self.flash(t.recall_away);
            }
        }
    }

    /// Journal line `i` in its conversation, selected in the salon's
    /// journal when it shows there.
    fn show_line(&mut self, i: usize) {
        self.leave_card();
        self.go_salon();
        self.zoom = Zoom::None;
        if self.journal_rows().iter().any(|&(r, _)| r == i) {
            self.pane = Pane::Journal;
            self.journal_sel = Some(i);
        }
        self.open_thread(i);
    }

    /// Enter while searching: the line under the highlight becomes the
    /// line written, whole (its own lines back), to send or go on with.
    fn take_sent(&mut self) {
        if let Some(m) = &self.menu {
            let picks = complete::history_picks(&self.input, &self.world.sent);
            if let Some(&i) = picks.get(m.chosen) {
                self.input = self.world.sent[i].text.clone();
            }
        }
        self.searching = false;
        self.menu = None;
        self.after = 0;
    }

    /// The salon with agent `id`'s row selected: its project unfolded, its
    /// worker's helpers too when it is one, and everyone shown again when
    /// `f` would hide it.
    fn go_agent(&mut self, id: AgentId) {
        self.go_salon();
        self.thread = None;
        let a = &self.world.agents[id];
        self.collapsed[a.project] = false;
        self.settled_projects.remove(&a.project);
        if let Some(p) = a.parent {
            self.folded_workers.remove(&p);
        }
        // Its own row, or the worker's row it hangs under.
        let rows: Vec<AgentId> = (self.settled.iter().copied())
            .filter(|&s| self.row_group(s).contains(&id))
            .collect();
        for s in rows {
            self.settled.remove(&s);
        }
        if self.needs_only && self.needs()[id] == Need::Nothing {
            self.needs_only = false;
        }
        self.selected = Sel::Agent(id);
    }

    /// `>` in worker `id`'s change: a message to it opens with the hunk at
    /// the top of the view quoted under the line the person writes on.
    fn quote_hunk(&mut self, id: AgentId) {
        let change = self.world.change_of(id);
        let Some(quote) = change.and_then(|c| ui::quote(&c.patch, self.diff_scroll)) else {
            return self.flash(self.lang.text().no_hunk);
        };
        let head = format!("@{} ", self.world.agents[id].name);
        self.input = format!("{head}\n{quote}");
        self.after = self.input.len() - head.len();
        self.typing = true;
        self.menu = None;
    }

    /// `O` or `:output`: a message opens to the agent at hand (else the
    /// project's head) with the last `!` command of its project quoted
    /// under the line the person writes on: how it ran and the last lines
    /// it showed. Nothing is sent until the person sends it.
    fn quote_output(&mut self) {
        let project = self.current_project();
        let Some(output) = self.outputs.get(&project) else {
            return self.flash(self.lang.text().no_output);
        };
        let to = (self.agent_at_hand())
            .filter(|&id| self.world.agents[id].project == project)
            .unwrap_or(self.world.projects[project].leader);
        let quote = ui::words::output_quote(self.lang, output);
        let head = format!("@{} ", self.world.agents[to].name);
        self.input = format!("{head}\n{quote}");
        self.after = self.input.len() - head.len();
        self.typing = true;
        self.menu = None;
    }

    /// The notes on worker `id`'s change as it is now, by line.
    pub fn notes_of(&self, id: AgentId) -> Vec<&DiffNote> {
        let Some(patch) = self.world.change_of(id).map(|c| patch_key(&c.patch)) else {
            return Vec::new();
        };
        let mut notes: Vec<&DiffNote> = (self.diff_notes.iter())
            .filter(|n| n.agent == id && n.patch == patch)
            .collect();
        notes.sort_by_key(|n| n.line);
        notes
    }

    /// The hunks of worker `id`'s change as it is now that the person
    /// chose to apply alone, by their `@@` lines, in order.
    pub fn chosen_of(&self, id: AgentId) -> Vec<usize> {
        let Some(patch) = self.world.change_of(id).map(|c| patch_key(&c.patch)) else {
            return Vec::new();
        };
        let mut hunks: Vec<usize> = (self.chosen.iter())
            .filter(|&&(a, p, _)| a == id && p == patch)
            .map(|&(_, _, h)| h)
            .collect();
        hunks.sort_unstable();
        hunks
    }

    /// `x` in worker `id`'s change: the hunk at hand is chosen to be
    /// applied alone (with the others chosen), or no longer.
    fn choose_hunk(&mut self, id: AgentId) {
        let t = self.lang.text();
        let Some(c) = self.world.change_of(id) else {
            return;
        };
        let Some(hunk) = ui::hunk_at(&c.patch, self.diff_scroll) else {
            return self.flash(t.no_hunk);
        };
        let all = c.patch.lines().filter(|l| l.starts_with("@@")).count();
        let key = (id, patch_key(&c.patch), hunk);
        let was = self.chosen.len();
        self.chosen.retain(|&k| k != key);
        if self.chosen.len() == was {
            self.chosen.push(key);
        }
        let n = self.chosen_of(id).len();
        let said = match n {
            0 => t.unchosen.to_string(),
            _ => fill(t.chosen, &[&n.to_string(), &all.to_string()]),
        };
        self.flash(said);
    }

    /// `c` in worker `id`'s change: a note on the line at hand (the top
    /// one), written in the command line; one there already is written
    /// again. Only a line of a hunk takes one.
    fn note_line(&mut self, id: AgentId) {
        let line = self.diff_scroll;
        let Some(c) = self.world.change_of(id) else {
            return;
        };
        if ui::place(&c.patch, line).is_none() {
            return self.flash(self.lang.text().note_where);
        }
        let patch = patch_key(&c.patch);
        let had = (self.diff_notes.iter())
            .find(|n| n.agent == id && n.patch == patch && n.line == line)
            .map(|n| n.text.clone());
        self.input = had.unwrap_or_default();
        self.after = 0;
        self.typing = true;
        self.menu = None;
        self.noting = Some(Noting::Line(id, line));
    }

    /// The note written on `line` of worker `id`'s change is kept (in
    /// place of one there was); an empty one takes it away.
    fn keep_note(&mut self, id: AgentId, line: usize, text: String) {
        let Some(patch) = self.world.change_of(id).map(|c| patch_key(&c.patch)) else {
            return;
        };
        let t = self.lang.text();
        self.diff_notes
            .retain(|n| !(n.agent == id && n.patch == patch && n.line == line));
        if text.is_empty() {
            return self.flash(t.note_dropped);
        }
        self.diff_notes.push(DiffNote {
            agent: id,
            patch,
            line,
            text,
        });
        let n = self.notes_of(id).len().to_string();
        self.flash(fill(t.noted, &[&n]));
    }

    /// `n` in worker `id`'s change: its notes go to it in one message and
    /// the change goes back, not applied, for it to change and send again:
    /// the third answer beside apply and reject.
    fn send_notes(&mut self, id: AgentId) {
        let t = self.lang.text();
        let notes: Vec<(usize, String)> = (self.notes_of(id).iter())
            .map(|n| (n.line, n.text.clone()))
            .collect();
        let Some(patch) = self.world.change_of(id).map(|c| c.patch.clone()) else {
            return;
        };
        if notes.is_empty() {
            return self.flash(t.no_notes);
        }
        let name = self.world.agents[id].name.clone();
        let refs: Vec<(usize, &str)> = notes.iter().map(|(l, n)| (*l, n.as_str())).collect();
        let message = ui::notes_message(&patch, &refs);
        let returned = match &mut self.engine {
            Engine::Live(l) => l.reject_change(&mut self.world, id),
            Engine::Sim(s) => s.reject_change(&mut self.world, id),
            Engine::View(_) => return self.read_only(),
        };
        if let Err(why) = returned {
            return self.flash(fill(t.reject_failed, &[&name, &why]));
        }
        self.diff_notes.retain(|n| n.agent != id);
        self.tell(id, &message);
        self.flash(fill(t.notes_sent, &[&notes.len().to_string(), &name]));
        let open = self.inbox_rows().len();
        if self.zoom == Zoom::Diff(id) {
            self.zoom = Zoom::None;
            self.pane = Pane::Inbox;
        }
        self.inbox_sel = self.inbox_sel.min(open.saturating_sub(1));
    }

    /// Whether the screen is drawn at `now`: on a change, never on a
    /// timer. At once when something came from the terminal since the
    /// last drawing; when something else changed or the screen's clock
    /// moved ([`Clock`]), at once while the terminal has the focus or
    /// never says (no focus reports), else [`BLURRED_DRAW`] after the
    /// last drawing at the soonest.
    pub fn draw_due(&self, now: Instant) -> bool {
        if self.stirred {
            return true;
        }
        let moved = self.changed
            || self.drawn_clock != Some(self.clock(now))
            || self.drawn_motion.is_some_and(|at| now >= at);
        moved
            && (self.blurred.is_none()
                || (self.drawn_at).is_none_or(|at| now.duration_since(at) >= BLURRED_DRAW))
    }

    /// The screen was drawn at `now`.
    pub fn drawn(&mut self, now: Instant) {
        self.drawn_at = Some(now);
        self.drawn_clock = Some(self.clock(now));
        self.stirred = false;
        self.changed = false;
        // A glow not yet begun (it waits for its beat) is kept.
        self.glows
            .retain(|&(_, glow, at)| now.saturating_duration_since(at) < glow.lasts());
        self.drawn_motion = self.next_motion(now);
    }

    /// What the screen shows of time at `now`.
    fn clock(&self, now: Instant) -> Clock {
        // A view's flash is the one on the screen.
        let shown = self.viewing.as_ref().map_or(&self.flash, |v| &v.flash);
        let flash = (shown.as_ref())
            .filter(|(_, at)| now.saturating_duration_since(*at) < FLASH)
            .map(|(_, at)| *at);
        let grace = self.grace_left(now).map(|(_, left)| left);
        (self.world.tick, flash, grace)
    }

    /// The terminal lost the focus (`gained` false) at `now`, or got it
    /// back: after `:away`'s time or more, what happened meanwhile shows
    /// at once, as after a key that long after the last (built from the
    /// journal, no model asked). Not over what the person is writing or
    /// another window.
    pub fn focus(&mut self, gained: bool, now: Instant) {
        // Back from the editor: the key file is looked at at once.
        self.keys_looked = None;
        if !gained {
            self.blurred = Some((now, self.world.tick));
            self.left_tick = self.world.tick;
            // Nothing moves in another window: no frame is waited for.
            self.drawn_motion = None;
            return;
        }
        let Some((at, tick)) = self.blurred.take() else {
            return;
        };
        let busy = self.typing
            || self.help
            || self.solo_pick.is_some()
            || self.archives.is_some()
            || self.viewing.is_some()
            || self.leaving
            || self.window_open();
        if busy || !self.away.over(now.duration_since(at)) {
            return;
        }
        self.digest = Digest::since(&self.world, tick);
    }

    /// `:notify`: how the person is told in another window.
    fn set_notify(&mut self, how: Notify) {
        let t = self.lang.text();
        let said = match how {
            Notify::Off => t.notify_off.to_string(),
            Notify::Bell => t.notify_bell.to_string(),
            Notify::Text => t.notify_text.to_string(),
            Notify::Push if self.demo() => t.notify_push_demo.to_string(),
            Notify::Push => match self.read_push() {
                Ok(server) => fill(t.notify_push, &[&server]),
                Err(why) => return self.flash(why),
            },
        };
        self.notify = how;
        self.flash(said);
    }

    /// Reads the person's push settings for `:notify push`: the server
    /// they go to, or why not (the settings as they were are kept then).
    fn read_push(&mut self) -> Result<String, String> {
        match crate::push::read(self.push_file.as_deref()) {
            Ok(p) => {
                let server = p.server.clone();
                self.push = Some(p);
                self.push_failing = false;
                Ok(server)
            }
            Err(why) => Err(ui::words::push_why(self.lang, &why)),
        }
    }

    /// At start, `:notify push` kept from the last run: its settings read
    /// again; when they no longer hold, nothing is pushed (`:notify off`)
    /// and the flash says why.
    pub fn start_push(&mut self) {
        if self.notify != Notify::Push || self.demo() {
            return;
        }
        if let Err(why) = self.read_push() {
            self.notify = Notify::Off;
            self.flash(why);
        }
    }

    /// Sends `word` to the person's phone when `:notify push` is on: on a
    /// thread of its own, its end told to [`App::pushed`]; the demo only
    /// shows it in the journal.
    pub fn push_word(&mut self, word: &notify::Word) {
        if self.notify != Notify::Push {
            return;
        }
        let Some(agent) = word.agent else {
            return;
        };
        if self.demo() {
            self.world.note(Event::Pushed {
                agent,
                words: word.body.clone(),
                sent: false,
            });
            self.changed = true;
            return;
        }
        let Some(p) = self.push.clone() else {
            return;
        };
        let (tx, body, loud) = (self.pushes.0.clone(), word.body.clone(), word.loud);
        std::thread::spawn(move || {
            let went = crate::push::send(&p, &body, loud);
            let _ = tx.send((agent, body, went));
        });
    }

    /// The pushes that ended since last time, in the journal: each that
    /// went; one that failed only when the one before did not, so a server
    /// that is down is said once, not at every push.
    pub fn pushed(&mut self) {
        while let Ok((agent, words, went)) = self.pushes.1.try_recv() {
            match went {
                Ok(()) => {
                    self.push_failing = false;
                    let sent = true;
                    self.world.note(Event::Pushed { agent, words, sent });
                }
                Err(_) if self.push_failing => continue,
                Err(why) => {
                    self.push_failing = true;
                    self.world.note(Event::PushFailed { agent, why });
                }
            }
            self.changed = true;
        }
    }

    /// `:motion`: how much moves on the screen. Full over SSH says it
    /// runs as calm.
    /// `:away`: how long away opens the digest by itself.
    fn set_away(&mut self, away: Away) {
        self.away = away;
        let t = self.lang.text();
        match away.time() {
            Some(_) => self.flash(fill(t.away_set, &[&away.name()])),
            None => self.flash(t.away_off),
        }
    }

    /// `:digest`: what happened since the person last left the window, or
    /// since the run began.
    fn open_digest(&mut self) {
        self.digest = Digest::since(&self.world, self.left_tick);
        if self.digest.is_none() {
            self.flash(self.lang.text().digest_none);
        }
    }

    fn set_motion(&mut self, how: Motion) {
        self.motion = how;
        let t = self.lang.text();
        self.flash(match (how, how.under(self.ssh)) {
            (Motion::Off, _) => t.motion_off,
            (Motion::Calm, _) => t.motion_calm,
            (Motion::Full, Motion::Full) => t.motion_full,
            (Motion::Full, _) => t.motion_ssh,
        });
    }

    /// What moves now: nothing in another window, and over SSH no more
    /// than calm.
    pub fn moving(&self) -> Motion {
        if self.blurred.is_some() {
            return Motion::Off;
        }
        self.motion.under(self.ssh)
    }

    /// How far into motion's round the frame being drawn is.
    fn motion_age(&self) -> std::time::Duration {
        self.frame.saturating_duration_since(self.epoch)
    }

    /// Whether a working row's glyph shows its second tone in this frame.
    pub fn breath(&self) -> bool {
        self.moving() != Motion::Off && motion::frame(self.motion_age(), motion::BREATH, 2) == 1
    }

    /// The dots after `starting` in this frame; none when nothing moves.
    pub fn dots(&self) -> Option<&'static str> {
        (self.moving() != Motion::Off).then(|| {
            let at = motion::frame(self.motion_age(), motion::DOTS, motion::DOT_FRAMES.len());
            motion::DOT_FRAMES[at]
        })
    }

    /// The mark is in the drawing being made: when the one before had
    /// none, its dots start coming on the next beat.
    pub fn mark_shown(&self) {
        if !self.mark_before.get() && !self.mark_drawn.get() {
            let since = self.frame.saturating_duration_since(self.epoch);
            self.mark_from
                .set(Some(self.epoch + motion::on_beat(since)));
        }
        self.mark_drawn.set(true);
    }

    /// How many of the mark's dots this frame shows: one more each beat
    /// from when it came on the screen until all are there; all at once
    /// after a key, with motion off, in another window or over SSH.
    pub fn mark_dots(&self) -> usize {
        let Some(from) = self.mark_from.get() else {
            return ui::brand::DOTS;
        };
        if self.ssh || self.moving() == Motion::Off {
            return ui::brand::DOTS;
        }
        let beats = self.frame.saturating_duration_since(from).as_nanos() / motion::BEAT.as_nanos();
        usize::try_from(beats).map_or(ui::brand::DOTS, |b| b.min(ui::brand::DOTS))
    }

    /// The spinner's frame where imece waits for an answer; none when
    /// nothing moves.
    pub fn spin(&self) -> Option<&'static str> {
        let period = self.moving().spin()?;
        Some(motion::SPIN[motion::frame(self.motion_age(), period, motion::SPIN.len())])
    }

    /// The spinner by the header's fleet: full motion only, while anyone
    /// is mid-turn.
    pub fn head_spin(&self) -> Option<&'static str> {
        (self.moving() == Motion::Full && !self.world.mid_turn().is_empty()).then(|| {
            motion::SPIN[motion::frame(self.motion_age(), motion::SPIN_FULL, motion::SPIN.len())]
        })
    }

    /// How `on` lights in this frame, if it does.
    pub fn lit(&self, on: GlowOn) -> Option<Lit> {
        if self.moving() == Motion::Off {
            return None;
        }
        let &(_, glow, at) = self.glows.iter().find(|g| g.0 == on)?;
        glow.lit(self.frame.saturating_duration_since(at))
    }

    /// What imece waits for an answer to (`:commit`), while it waits.
    pub fn asking(&self) -> Option<&str> {
        self.asking.as_deref()
    }

    /// Whether the bottom line shows the spinner at `now`: imece waits for
    /// an answer and nothing else is said there.
    fn footer_spins(&self, now: Instant) -> bool {
        self.asking.is_some()
            && !self.typing
            && (self.flash.as_ref())
                .is_none_or(|(_, at)| now.saturating_duration_since(*at) >= FLASH)
            && self.grace_left(now).is_none()
    }

    /// What moves on the screen at `now`, each with how often it moves:
    /// nothing when nothing moves (motion off, another window, an
    /// archive's view). Glows are apart ([`App::glows`]).
    fn moving_parts(&self, now: Instant) -> Vec<(Moves, Duration)> {
        let moving = self.moving();
        if moving == Motion::Off || self.viewing.is_some() {
            return Vec::new();
        }
        let w = &self.world;
        let mut parts = Vec::new();
        if self.zoom == Zoom::None && self.view == View::Salon {
            let rungs = self.rungs();
            let shown = |a: &&Agent| !a.gone && !w.put_away(a.id);
            if (w.agents.iter().filter(shown)).any(|a| ui::breathes(a, rungs[a.id])) {
                parts.push((Moves::Breath, motion::BREATH));
            }
            if (w.agents.iter().filter(shown)).any(|a| a.starting && a.status == Status::Working) {
                parts.push((Moves::Dots, motion::DOTS));
            }
        }
        // The board shows the owners of the cards taken.
        if self.zoom == Zoom::None && self.view == View::Board {
            let rungs = self.rungs();
            let owners = (w.shown_cards())
                .filter(|c| c.column != Column::Waiting)
                .filter_map(|c| c.owner);
            if owners
                .map(|id| &w.agents[id])
                .any(|a| !a.gone && ui::breathes(a, rungs[a.id]))
            {
                parts.push((Moves::Breath, motion::BREATH));
            }
        }
        let checking = self.welcome && self.vendors.iter().any(|(_, s)| *s == Seen::Checking);
        if let Some(spin) = moving.spin()
            && (self.footer_spins(now) || checking)
        {
            parts.push((Moves::Spin, spin));
        }
        if moving == Motion::Full && !w.mid_turn().is_empty() {
            parts.push((Moves::HeadSpin, motion::SPIN_FULL));
        }
        // The mark still coming, where the last drawing showed it; as
        // `mark_dots` counts at `now` (its start on a beat).
        if let Some(from) = self.mark_from.get()
            && self.mark_drawn.get()
            && !self.ssh
            && now < from + motion::BEAT * ui::brand::DOTS as u32
        {
            parts.push((Moves::Mark, motion::BEAT));
        }
        parts
    }

    /// When what moves on the screen next looks different, after `now`:
    /// none when nothing moves (motion off, another window, an archive's
    /// view, nothing on the screen that moves, every glow over).
    pub fn next_motion(&self, now: Instant) -> Option<Instant> {
        if self.moving() == Motion::Off || self.viewing.is_some() {
            return None;
        }
        let parts = self.moving_parts(now);
        let since = now.saturating_duration_since(self.epoch);
        // A glow that starts on the next beat is lit from now until the
        // beat after it.
        let glows = (self.glows.iter()).filter_map(|&(_, glow, at)| {
            (glow.next(now.saturating_duration_since(at))).map(|after| now.max(at) + after)
        });
        (parts.into_iter())
            .map(|(_, p)| now + motion::next_frame(since, p))
            .chain(glows)
            .min()
    }

    /// Whether the frame due at `now` may be drawn as the last drawing
    /// with only its breath changed (`ui::patch`): nothing came from the
    /// terminal, nothing changed, the clock shows the same, the breath is
    /// all that moves (no glow, dots or spinner), and that drawing knew
    /// where every glyph that breathes is.
    pub fn patch_due(&self, now: Instant) -> bool {
        !self.stirred
            && !self.changed
            && self.drawn_clock == Some(self.clock(now))
            && self.glows.is_empty()
            && self.moving_parts(now) == [(Moves::Breath, motion::BREATH)]
            && self.patchable()
    }

    /// Whether the last drawing can be patched: it knew where every glyph
    /// that breathes is, and some do.
    pub fn patchable(&self) -> bool {
        self.breathing
            .borrow()
            .as_ref()
            .is_some_and(|b| !b.is_empty())
    }

    /// The frame drawn `after` motion's start (tests).
    #[cfg(test)]
    pub fn frame_after(&mut self, after: Duration) {
        self.frame = self.epoch + after;
    }

    /// The moment `after` motion's start (tests).
    #[cfg(test)]
    pub fn after(&self, after: Duration) -> Instant {
        self.epoch + after
    }

    /// `on` lights with `glow` from the frame's beat (tests).
    #[cfg(test)]
    pub fn light_now(&mut self, on: GlowOn, glow: Glow) {
        self.light(on, glow, self.frame);
    }

    /// How long the main loop may wait for the next frame of what moves.
    pub fn motion_wait(&self, now: Instant) -> Option<Duration> {
        self.drawn_motion
            .map(|at| at.saturating_duration_since(now))
    }

    /// How long the main loop waits at `now` when the next tick is
    /// `tick` away: until the next frame of what moves only when it comes
    /// more than [`motion::LATE`] before the tick; else the tick's wake
    /// draws it, a little late, and one frame is drawn, not two.
    pub fn wait(&self, now: Instant, tick: Duration) -> Duration {
        match self.motion_wait(now) {
            Some(frame) if frame + motion::LATE < tick => frame,
            _ => tick,
        }
    }

    /// What changed since the last step that lights a row for a moment:
    /// an agent that came to wait for the person, or a new item in
    /// WAITING FOR YOU, blinks; one whose turn is done, or whose session
    /// started, settles. Only what happens while motion moves lights.
    fn watch_glows(&mut self, now: Instant) {
        let rungs = self.rungs();
        if self.moving() != Motion::Off {
            for (id, &rung) in rungs.iter().enumerate() {
                let Some(&was) = self.glow_rungs.get(id) else {
                    continue;
                };
                let glow = match rung {
                    _ if was == rung => continue,
                    Rung::Waiting => Glow::Arrive,
                    Rung::Done => Glow::Settle,
                    _ if was == Rung::Starting => Glow::Settle,
                    _ => continue,
                };
                self.light(GlowOn::Agent(id), glow, now);
            }
            let new = self.glow_items;
            for i in (self.world.open_inbox().into_iter()).filter(|&i| i >= new) {
                self.light(GlowOn::Item(i), Glow::Arrive, now);
            }
        }
        self.glow_rungs = rungs;
        self.glow_items = self.world.inbox.len();
    }

    /// `on` lights with `glow` from `now`, in place of a glow it had.
    /// Its frames count from the first beat at or after `now`, with the
    /// rest of what moves.
    fn light(&mut self, on: GlowOn, glow: Glow, now: Instant) {
        let at = self.epoch + motion::on_beat(now.saturating_duration_since(self.epoch));
        self.glows.retain(|g| g.0 != on);
        self.glows.push((on, glow, at));
    }

    /// What to say to the person in another window right now, in the
    /// words of [`crate::notify`]: what came to wait for them while the
    /// terminal had no focus and is still there [`notify::AFTER`] later,
    /// once each; several at once as a count. Nothing while `:notify` is
    /// off, in the window, or for what was on screen before leaving.
    pub fn notices(&mut self, now: Instant) -> Option<notify::Word> {
        if self.notify == Notify::Off {
            return None;
        }
        let w = &self.world;
        let open = w.open_inbox();
        // A stuck agent whose item waits in the inbox is noted once.
        let stuck = |(id, n): (AgentId, Need)| {
            (n == Need::Stuck && !open.iter().any(|&i| w.inbox[i].agent == id)).then_some(id)
        };
        let waiting: Vec<Note> = open
            .iter()
            .copied()
            .map(Note::Item)
            .chain(
                self.needs()
                    .into_iter()
                    .enumerate()
                    .filter_map(stuck)
                    .map(Note::Stuck),
            )
            .collect();
        // Away from the machine the terminal may still have the focus: a
        // push also counts the keys' quiet, as long as `:away` says.
        let idle = self.notify == Notify::Push
            && self.away.over(now.saturating_duration_since(self.last_key));
        let due = self
            .notices
            .due(&waiting, self.blurred.is_some() || idle, now);
        let t = self.lang.text();
        let body = match due[..] {
            [] => return None,
            [one] if self.notify == Notify::Push => ui::words::pushed(self.lang, &self.world, one),
            [one] => ui::words::notice(self.lang, &self.world, one),
            _ => fill(t.notify_many, &[&due.len().to_string()]),
        };
        let agent = due.first().map(|&n| match n {
            Note::Stuck(id) => id,
            Note::Item(i) => self.world.inbox[i].agent,
        });
        // Louder for what is stuck or failed.
        let loud = due.iter().any(|&n| match n {
            Note::Stuck(_) => true,
            Note::Item(i) => self.world.inbox[i].kind == InboxKind::Fault,
        });
        self.notice_count += 1;
        let id = self.notice_count;
        self.noticed.push((id, due));
        let over = self.noticed.len().saturating_sub(NOTICED);
        self.noticed.drain(..over);
        Some(notify::Word {
            body,
            id,
            loud,
            agent,
        })
    }

    /// kitty reported notification `id` clicked: what it was about opens,
    /// over whatever window was open: the item that waits, the agent that
    /// is stuck, or for several the next item waiting (`W`). A number imece
    /// did not send, or one sent too long ago, does nothing.
    pub fn clicked(&mut self, id: u64) {
        let Some((_, notes)) = self.noticed.iter().find(|(n, _)| *n == id).cloned() else {
            return;
        };
        self.viewing = None;
        self.help = false;
        self.archives = None;
        self.peek = None;
        self.hints = None;
        self.news.clear();
        self.welcome = false;
        self.digest = None;
        self.left_off = None;
        self.leaving = false;
        let open = |i: usize| self.world.inbox.get(i).is_some_and(|x| x.open);
        match notes[..] {
            [Note::Item(i)] if open(i) => self.open_waiting(i),
            [Note::Stuck(a)] => self.go_agent(a),
            _ => self.next_waiting(false),
        }
    }

    /// The machine is held awake while an agent is mid-turn and the person
    /// asked for it (`--awake`, `:awake`), and let go otherwise; a hold
    /// that fails is said once.
    fn keep_awake(&mut self) {
        let busy = !self.world.mid_turn().is_empty();
        if let Err(why) = self.awake.hold(busy) {
            let t = self.lang.text();
            self.flash(fill(t.awake_failed, &[&why]));
        }
    }

    /// Where the person is now.
    pub fn place(&self) -> Place {
        Place {
            view: self.view,
            zoom: self.zoom,
            pane: self.pane,
            card: self.card,
        }
    }

    /// `^o`: back to the place the person was before, as vim's jump list
    /// goes; an open conversation closes first. A place that is not there
    /// any more (a gone agent's focus, a change applied since) is passed
    /// over, and with none left it says so.
    fn go_back(&mut self) {
        self.went_back = true;
        if self.thread.take().is_some() {
            return;
        }
        while let Some(p) = self.been.pop() {
            let agent_ok = |id: AgentId| self.world.agents.get(id).is_some_and(|a| !a.gone);
            let zoom_ok = match p.zoom {
                Zoom::None => true,
                Zoom::Focus(id) | Zoom::Trace(id) => agent_ok(id),
                Zoom::Diff(id) => self.world.change_of(id).is_some(),
            };
            let card_ok = p.card.is_none_or(|c| c < self.world.cards.len());
            if zoom_ok && card_ok && p != self.place() {
                self.view = p.view;
                self.zoom = p.zoom;
                self.pane = p.pane;
                self.card = p.card;
                if self.pane == Pane::Journal {
                    self.select_journal(None);
                }
                return;
            }
        }
        self.flash(self.lang.text().no_back);
    }

    /// `m` or `:mark`: the line at hand (the open conversation's selected
    /// line, else the journal's) is marked to come back to, or loses its
    /// mark.
    fn toggle_mark(&mut self) {
        // An archive's marks are as it left them.
        if self.viewing_archive() {
            return self.read_only();
        }
        let journal = self.view == View::Salon && self.zoom == Zoom::None;
        let at = match &self.thread {
            Some(th) => th.entries.get(th.sel).copied(),
            None if journal && self.pane == Pane::Journal => self.journal_sel,
            None => None,
        };
        let t = self.lang.text();
        let said = match at {
            Some(i) if self.world.mark(i) => t.marked,
            Some(_) => t.unmarked,
            None => t.mark_where,
        };
        self.flash(said);
    }

    /// `'`: the next marked line after the one at hand, round to the first
    /// again: among the open conversation's lines, else in the journal
    /// (the card's, on a card's screen) in the order it lists them, newest
    /// first; from anywhere else the journal is where it goes.
    fn next_mark(&mut self) {
        let marks = &self.world.marks;
        let found = match &mut self.thread {
            Some(th) => {
                let marked: Vec<usize> = (0..th.entries.len())
                    .filter(|&n| marks.contains(&th.entries[n]))
                    .collect();
                let to = next_of(&marked, Some(th.sel));
                if let Some(n) = to {
                    th.sel = n;
                    th.follow.set(true);
                }
                to.is_some()
            }
            None => {
                self.view = View::Salon;
                self.zoom = Zoom::None;
                self.pane = Pane::Journal;
                let rows = self.journal_rows();
                let marked: Vec<usize> = (0..rows.len())
                    .filter(|&n| marks.contains(&rows[n].0))
                    .collect();
                let at = (self.journal_sel).and_then(|i| rows.iter().position(|&(r, _)| r == i));
                let to = next_of(&marked, at);
                if let Some(n) = to {
                    self.journal_sel = Some(rows[n].0);
                } else {
                    self.select_journal(None);
                }
                to.is_some()
            }
        };
        if !found {
            self.flash(self.lang.text().no_marks);
        }
    }

    /// `:pin [NAME]`: the named project (else the one at hand) is pinned
    /// to the top of the salon, or unpinned when it was.
    fn pin(&mut self, arg: &str) {
        let t = self.lang.text();
        let p = if arg.is_empty() {
            self.current_project()
        } else {
            match self.world.project_by_name(arg) {
                Some(p) => p,
                None => return self.flash(fill(t.no_project, &[arg])),
            }
        };
        let project = &mut self.world.projects[p];
        project.pinned = !project.pinned;
        let said = if project.pinned { t.pinned } else { t.unpinned };
        let said = fill(said, &[&project.name]);
        self.flash(said);
    }

    /// `:export [PATH]`: the open conversation, else the card's screen,
    /// as Markdown in PATH (a file, or a directory to name one in), else
    /// in the directory imece was started from. Nothing is written over.
    fn export(&mut self, arg: &str) {
        let t = self.lang.text();
        let (what, text) = match (&self.thread, self.card) {
            (Some(th), _) => (
                "conversation".to_string(),
                ui::export::thread(self.lang, &self.world, &th.entries),
            ),
            (None, Some(c)) => (format!("card-{}", c + 1), ui::export::card(self, c)),
            (None, None) => return self.flash(t.nothing_to_export),
        };
        let name = ui::export::file_name(&what, self.world.tick);
        let target = (!arg.is_empty()).then(|| expand_tilde(arg, self.home.as_deref()));
        let here = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
        match ui::export::write(target.as_deref(), &here, &name, &text) {
            Ok(path) => self.flash(fill(t.exported, &[&path.display().to_string()])),
            Err(e) => {
                let path = target.unwrap_or_else(|| here.join(&name));
                self.flash(fill(
                    t.export_failed,
                    &[&path.display().to_string(), &e.to_string()],
                ))
            }
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
            Engine::Live(live) => live.open_project(&mut self.world, &full, name, Open::Team),
            Engine::Sim(sim) => sim.open_project(&mut self.world, path, name),
            Engine::View(_) => return self.read_only(),
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

    /// `:solo [PATH] [VENDOR[:MODEL][@EFFORT]] [NAME]`, the words in any
    /// order: a solo session on the checkout at PATH (else the project at
    /// hand's), on the vendor and choice, as a project named NAME (else
    /// after the directory, numbered when taken). A word with a `/`, or
    /// starting with `~` or `.`, is the path; one that starts with a
    /// vendor's name is the choice; the one left is the name. Without a
    /// choice, a screen asks the vendor, the model and the effort first.
    /// Selected when opened.
    fn solo_project(&mut self, arg: &str) {
        let t = self.lang.text();
        let (mut path, mut choice, mut name) = (None, None, None);
        for word in arg.split_whitespace() {
            let slot = if word.contains('/') || word.starts_with(['~', '.']) {
                &mut path
            } else if let Some(c) = vendor_choice(word) {
                if choice.replace(c).is_some() {
                    return self.flash(t.solo_usage);
                }
                continue;
            } else {
                &mut name
            };
            if slot.replace(word.to_string()).is_some() {
                return self.flash(t.solo_usage);
            }
        }
        match choice {
            Some((vendor, choice)) => self.open_solo(path, name, Some(vendor), choice),
            None => self.solo_pick = Some(SoloPick::new(path, name, self.vendors())),
        }
    }

    /// The vendors a solo may run on here: the engine's, the lead vendor
    /// first; the demo's three.
    fn vendors(&self) -> Vec<Vendor> {
        match &self.engine {
            Engine::Live(live) => live.vendors(),
            Engine::Sim(_) => vec![Vendor::Claude, Vendor::Codex, Vendor::Gemini],
            Engine::View(_) => Vec::new(),
        }
    }

    /// A key on the solo pick screen: a step answered opens the solo
    /// with what was picked; `q` or `esc` at the first step drops it.
    fn solo_pick_key(&mut self, code: KeyCode) {
        let Some(pick) = &mut self.solo_pick else {
            return;
        };
        match pick.on_key(code) {
            None => {}
            Some(None) => self.solo_pick = None,
            Some(Some(picked)) => {
                let pick = self.solo_pick.take().expect("open");
                self.open_solo(pick.path, pick.name, Some(picked.0), picked.1);
            }
        }
    }

    /// Opens a solo on the checkout at `path` (none: the project at hand's)
    /// named `name`, on `vendor` (none: the lead vendor) and `choice`;
    /// selected when opened, the flash says what it runs on.
    fn open_solo(
        &mut self,
        path: Option<String>,
        name: Option<String>,
        vendor: Option<Vendor>,
        choice: Option<String>,
    ) {
        let t = self.lang.text();
        let here = self.current_project();
        let full = match &path {
            Some(p) => expand_tilde(p, self.home.as_deref()),
            None => match &self.engine {
                Engine::Live(live) => live.repo_of(here).to_path_buf(),
                Engine::Sim(_) => PathBuf::from(&self.world.projects[here].name),
                Engine::View(_) => return self.read_only(),
            },
        };
        let shown = full.display().to_string();
        let name = name.as_deref();
        let opened = match &mut self.engine {
            Engine::Live(live) => live.open_project(
                &mut self.world,
                &full,
                name,
                Open::Solo {
                    vendor,
                    choice: choice.clone(),
                },
            ),
            Engine::Sim(sim) => sim.open_solo(&mut self.world, &shown, vendor, choice, name),
            Engine::View(_) => return self.read_only(),
        };
        match opened {
            Ok(p) => {
                if p == self.collapsed.len() {
                    self.collapsed.push(false);
                } else {
                    self.collapsed[p] = false;
                }
                let project = &self.world.projects[p];
                let head = project.leader;
                let a = &self.world.agents[head];
                let runs = match self.model_of(head) {
                    Some(m) => format!("{} {m}", a.vendor.label()),
                    None => a.vendor.label().to_string(),
                };
                let said = fill(t.solo_opened, &[&project.name, &shown, &a.name, &runs]);
                self.zoom = Zoom::None;
                self.selected = Sel::Agent(head);
                self.flash(said);
            }
            Err(ProjectRefusal::NameTaken(n)) => self.flash(fill(t.project_exists, &[&n])),
            Err(ProjectRefusal::BadName(n)) => self.flash(fill(t.bad_project_name, &[&n])),
            Err(e) => self.flash(fill(t.bad_open, &[&e.describe()])),
        }
    }

    /// The solo session the person looks at: the agent at hand when it is
    /// one, else the project at hand's head when that is one.
    fn solo_at_hand(&self) -> Option<AgentId> {
        let w = &self.world;
        let at_hand = self
            .agent_at_hand()
            .filter(|&id| w.agents[id].role == Role::Solo);
        at_hand.or_else(|| {
            let p = self.current_project();
            w.is_solo(p).then(|| w.projects[p].leader)
        })
    }

    /// `:effort LEVEL` / `:model NAME`: the solo at hand runs on it from
    /// its next turn; `default` is the vendor's own. The other part of its
    /// choice stays.
    fn rechoose(&mut self, part: Part, arg: &str) {
        let t = self.lang.text();
        let word = arg.trim();
        if word.is_empty() {
            return self.flash(match part {
                Part::Effort => t.effort_usage,
                Part::Model => t.model_usage,
            });
        }
        let Some(id) = self.solo_at_hand() else {
            return self.flash(t.no_solo);
        };
        let name = self.world.agents[id].name.clone();
        let value = (word != complete::DEFAULT).then(|| word.to_string());
        if value
            .as_deref()
            .is_some_and(|v| !valid_model(v) || v.contains('@'))
        {
            return self.flash(fill(
                t.chose_refused,
                &[
                    &name,
                    &format!(
                        "{word:?} is not {}",
                        match part {
                            Part::Effort => "an effort level",
                            Part::Model => "a model name",
                        }
                    ),
                ],
            ));
        }
        let mut choice = Choice::parse(self.model_of(id).as_deref().unwrap_or_default());
        match part {
            Part::Effort => choice.effort = value,
            Part::Model => choice.model = value,
        }
        let choice = Some(choice.wire()).filter(|c| !c.is_empty());
        let changed = match &mut self.engine {
            Engine::Live(live) => live.rechoose(&mut self.world, id, choice),
            Engine::Sim(sim) => sim.rechoose(&mut self.world, id, choice),
            Engine::View(_) => return self.read_only(),
        };
        match changed {
            Ok(runs) => self.flash(fill(t.chose, &[&name, &runs])),
            Err(why) => self.flash(fill(t.chose_refused, &[&name, &why])),
        }
    }

    /// The menus of `:solo`, `:effort` and `:model`: what the engine can
    /// run here, by vendor.
    fn choice_menu(&self) -> Option<Menu> {
        let rest = self.input.strip_prefix(':')?;
        let (cmd, arg) = rest.split_once(' ')?;
        let word = arg.rsplit(' ').next().unwrap_or_default();
        let start = self.input.len() - word.len();
        match complete::long(cmd) {
            "solo" => {
                let vendors = self.vendors();
                complete::solo_words(
                    start,
                    word,
                    self.home.as_deref(),
                    &vendors,
                    &|v| live::efforts(v).to_vec(),
                    &complete::list_dirs,
                )
            }
            "effort" => {
                let v = self.world.agents[self.solo_at_hand()?].vendor;
                complete::choice_words(start, arg, live::efforts(v))
            }
            "model" => {
                let v = self.world.agents[self.solo_at_hand()?].vendor;
                complete::choice_words(start, arg, &live::aliases(v))
            }
            _ => None,
        }
    }

    /// The `[MODEL][@EFFORT]` choice agent `id` runs on, none for its
    /// vendor's own default: a solo's row shows it.
    pub fn model_of(&self, id: AgentId) -> Option<String> {
        match &self.engine {
            Engine::Live(live) => live.model_of(id).map(str::to_string),
            Engine::Sim(sim) => sim.model_of(id).map(str::to_string),
            // What it ran on is not kept with an archive.
            Engine::View(_) => None,
        }
    }

    /// `:close [NAME]`: the named project (else the one at hand) closes:
    /// its agents leave, their worktrees stay. The last one stays open.
    fn close_project(&mut self, arg: &str) {
        let t = self.lang.text();
        // One waits at a time: the one before acts now.
        self.end_grace();
        let p = if arg.is_empty() {
            self.current_project()
        } else {
            match self.world.project_by_name(arg) {
                Some(p) => p,
                None => return self.flash(fill(t.no_project, &[arg])),
            }
        };
        if self.world.open_projects().count() < 2 {
            let name = &self.world.projects[p].name;
            return self.flash(fill(t.last_project, &[name]));
        }
        self.wait_grace(Undoable::Close(p));
    }

    /// A `:close` or `:forget` waits [`UNDO_GRACE`] from now; the bottom
    /// line counts it down.
    fn wait_grace(&mut self, what: Undoable) {
        self.flash = None;
        self.grace = Some((what, Instant::now() + UNDO_GRACE));
    }

    /// `u` or `:undo`: the `:close` or `:forget` waiting does not happen.
    fn undo(&mut self) {
        let t = self.lang.text();
        let said = match self.grace.take() {
            Some((Undoable::Close(p), _)) => fill(t.undone_close, &[&self.world.projects[p].name]),
            Some((Undoable::Forget(id), _)) => {
                fill(t.undone_forget, &[&self.world.agents[id].name])
            }
            Some((Undoable::Archive(p), _)) => {
                fill(t.undone_archive, &[&self.world.projects[p].name])
            }
            Some((Undoable::Clear(_), _)) => t.undone_clear.to_string(),
            None => t.nothing_to_undo.to_string(),
        };
        self.flash(said);
    }

    /// `U` or `:now`: what counts down acts at once, as when its time is
    /// up; the person need not wait it out.
    fn grace_now(&mut self) {
        if self.grace.is_none() {
            return self.flash(self.lang.text().nothing_now);
        }
        self.end_grace();
        self.persist();
    }

    /// What waits out its grace at `now`, and the seconds it has left,
    /// rounded up.
    pub fn grace_left(&self, now: Instant) -> Option<(Undoable, u64)> {
        let (what, at) = self.grace?;
        let left = at.saturating_duration_since(now).as_millis();
        Some((what, left.div_ceil(1000) as u64))
    }

    /// The `:close` or `:forget` whose grace ran out by `now` acts.
    pub fn grace_due(&mut self, now: Instant) {
        if self.grace.is_some_and(|(_, at)| now >= at) {
            self.end_grace();
            self.persist();
        }
    }

    /// The `:close` or `:forget` waiting acts now: before another, and
    /// when imece quits.
    pub fn end_grace(&mut self) {
        match self.grace.take() {
            Some((Undoable::Close(p), _)) => self.close_now(p),
            Some((Undoable::Forget(id), _)) => self.forget_now(id),
            Some((Undoable::Archive(p), _)) => self.archive_now(p),
            Some((Undoable::Clear(what), _)) => self.clear_now(what),
            None => {}
        }
    }

    /// `:clear [inbox|journal]`: what WAITING FOR YOU lists and the
    /// journal's lines go off the salon's screen after the grace, both
    /// without a word. Nothing is deleted; what still waits is counted.
    fn clear(&mut self, arg: &str) {
        let t = self.lang.text();
        let Some(what) = Clear::parse(arg) else {
            return self.flash(fill(t.bad_clear, &[arg]));
        };
        // One waits at a time: the one before acts now.
        self.end_grace();
        if self.clearable(what) == (0, 0) {
            return self.flash(t.nothing_to_clear);
        }
        self.wait_grace(Undoable::Clear(what));
    }

    /// What `what` takes off now: the items WAITING FOR YOU lists and the
    /// journal's rows not marked.
    fn clearable(&self, what: Clear) -> (usize, usize) {
        let w = &self.world;
        let inbox = match what.inbox() {
            true => w.shown_inbox().len(),
            false => 0,
        };
        let lines = match what.journal() {
            true => (ui::journal_rows(w).iter())
                .filter(|(i, _)| !w.marks.contains(i))
                .count(),
            false => 0,
        };
        (inbox, lines)
    }

    /// `:clear`'s grace ran out: off the screen now.
    fn clear_now(&mut self, what: Clear) {
        let t = self.lang.text();
        let (inbox, lines) = self.clearable(what);
        if what.inbox() {
            let shown = self.world.shown_inbox();
            self.world.cleared.inbox.extend(shown);
            self.inbox_sel = 0;
        }
        if what.journal() {
            self.world.cleared.journal = self.world.journal.len();
            let shown = self.journal_rows();
            self.journal_sel = self
                .journal_sel
                .filter(|i| shown.iter().any(|(r, _)| r == i));
        }
        let (inbox, lines) = (inbox.to_string(), lines.to_string());
        let said = match what {
            Clear::All => fill(t.cleared[0], &[&inbox, &lines]),
            Clear::Inbox => fill(t.cleared[1], &[&inbox]),
            Clear::Journal => fill(t.cleared[2], &[&lines]),
        };
        self.flash(said);
    }

    /// `:unclear [inbox|journal]`: what `:clear` took off comes back.
    fn unclear(&mut self, arg: &str) {
        let t = self.lang.text();
        let Some(what) = Clear::parse(arg) else {
            return self.flash(fill(t.bad_clear, &[arg]));
        };
        let (inbox, lines) = (self.world.shown_inbox().len(), self.journal_rows().len());
        if what.inbox() {
            self.world.cleared.inbox.clear();
        }
        if what.journal() {
            self.world.cleared.journal = 0;
        }
        let inbox = self.world.shown_inbox().len() - inbox;
        let lines = self.journal_rows().len() - lines;
        if inbox + lines == 0 {
            return self.flash(t.nothing_cleared);
        }
        self.flash(fill(t.uncleared, &[&inbox.to_string(), &lines.to_string()]));
        self.persist();
    }

    /// Project `p` closes: its agents leave, what pointed into it moves.
    fn close_now(&mut self, p: ProjectId) {
        let t = self.lang.text();
        let name = self.world.projects[p].name.clone();
        let closed = match &mut self.engine {
            Engine::Live(live) => live.close_project(&mut self.world, p),
            Engine::Sim(sim) => sim.close_project(&mut self.world, p),
            Engine::View(_) => return self.read_only(),
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

    /// `:archive [TITLE]`: the project at hand is put away as TITLE (else
    /// its name and days) after the grace, and a fresh one opens in its
    /// place.
    fn archive_project(&mut self, arg: &str) {
        // One waits at a time: the one before acts now.
        self.end_grace();
        let p = self.current_project();
        let title: String = (arg.lines().next().unwrap_or_default().trim())
            .chars()
            .take(ARCHIVE_TITLE_MAX)
            .collect();
        self.archive_title = match title.is_empty() {
            true => self.world.archive_title(p),
            false => title,
        };
        self.wait_grace(Undoable::Archive(p));
    }

    /// Project `p` is archived now and a fresh one takes its place: what
    /// pointed into it moves to the fresh one.
    fn archive_now(&mut self, p: ProjectId) {
        let t = self.lang.text();
        let name = self.world.projects[p].name.clone();
        let title = std::mem::take(&mut self.archive_title);
        let archived = match &mut self.engine {
            Engine::Live(live) => live.archive_project(&mut self.world, p, title.clone()),
            Engine::Sim(sim) => sim.archive_project(&mut self.world, p, title.clone()),
            Engine::View(_) => return self.read_only(),
        };
        let fresh = match archived {
            Ok(fresh) => fresh,
            Err(why) => return self.flash(why.describe()),
        };
        self.collapsed
            .push(self.collapsed.get(p).copied().unwrap_or(false));
        self.put_away(p, fresh);
        self.flash(fill(t.archived, &[&name, &title]));
    }

    /// Project `p` went out of sight, archived or closed, and `to` is in
    /// its place: what the screen held of `p` moves to `to`'s head.
    fn put_away(&mut self, p: ProjectId, to: ProjectId) {
        // What the archive's agents left unseen waits in it.
        let w = &self.world;
        self.unseen.retain(|&a| w.agents[a].project != p);
        self.settled.retain(|&a| w.agents[a].project != p);
        if let Zoom::Focus(a) | Zoom::Trace(a) | Zoom::Diff(a) = self.zoom
            && self.world.agents[a].project == p
        {
            self.zoom = Zoom::None;
        }
        let was_in = match self.selected {
            Sel::Project(q) => q == p,
            Sel::Agent(a) => self.world.agents[a].project == p,
        };
        if was_in {
            self.selected = Sel::Agent(self.world.projects[to].leader);
        }
        if self.card.is_some_and(|c| self.world.cards[c].project == p) {
            self.card = None;
        }
        let open = self.inbox_rows().len();
        self.inbox_sel = self.inbox_sel.min(open.saturating_sub(1));
    }

    /// `:restore [NAME]`: the archive titled NAME (else the newest of the
    /// project called NAME; with no NAME, the newest of the project at
    /// hand's name) comes back, its head selected; the project open in
    /// its place gives way.
    fn restore_project(&mut self, arg: &str) {
        let t = self.lang.text();
        // One waits at a time: the one before acts now.
        self.end_grace();
        let arg = arg.trim();
        let at = self.current_project();
        let Some(p) = self.world.archive_named(arg, at) else {
            return self.flash(if arg.is_empty() {
                fill(t.no_archive_of, &[&self.world.projects[at].name])
            } else {
                fill(t.no_archive, &[arg])
            });
        };
        self.restore_at(p);
    }

    /// Archive `p` comes back, its head selected; the project open in its
    /// place gives way.
    fn restore_at(&mut self, p: ProjectId) {
        let t = self.lang.text();
        let title = (self.world.projects[p].archived.as_ref())
            .map(|a| a.title.clone())
            .unwrap_or_default();
        let restored = match &mut self.engine {
            Engine::Live(live) => live.restore_project(&mut self.world, p),
            Engine::Sim(sim) => sim.restore_project(&mut self.world, p),
            Engine::View(_) => return self.read_only(),
        };
        let gave_way = match restored {
            Ok(q) => q,
            Err(why) => return self.flash(why.describe()),
        };
        if let Some(q) = gave_way {
            self.put_away(q, p);
        }
        self.selected = Sel::Agent(self.world.projects[p].leader);
        let name = &self.world.projects[p].name;
        self.flash(fill(t.restored, &[name, &title]));
    }

    /// `A`, `:archives`: the archives as a tree, the checkout at hand's
    /// first; nothing archived says so.
    fn open_archives(&mut self) {
        // The view's other projects are put away only to hide them.
        if self.viewing_archive() {
            return self.read_only();
        }
        self.end_grace();
        if self.world.archives().is_empty() {
            return self.flash(self.lang.text().no_archives);
        }
        let at = self.current_project();
        let groups = archives::groups(&self.world, &|p| self.checkout_shown(p), Some(at));
        self.archives = Some(Browser::new(&self.world, groups));
    }

    /// Project `p`'s checkout as the archives browser writes it: live, its
    /// folder with `~` for the home; the demo's, the project's name (its
    /// stand-in for a checkout).
    fn checkout_shown(&self, p: ProjectId) -> String {
        match &self.engine {
            Engine::Live(live) => {
                let repo = live.repo_of(p);
                match self.home.as_deref().and_then(|h| repo.strip_prefix(h).ok()) {
                    Some(rest) => format!("~/{}", rest.display()),
                    None => repo.display().to_string(),
                }
            }
            Engine::Sim(_) | Engine::View(_) => self.world.projects[p].name.clone(),
        }
    }

    /// `path` as the screen shows it: `~` for the home, cleaned.
    fn shown_path(&self, path: &Path) -> String {
        let said = match self.home.as_deref().and_then(|h| path.strip_prefix(h).ok()) {
            Some(rest) if rest.as_os_str().is_empty() => "~".to_string(),
            Some(rest) => format!("~/{}", rest.display()),
            None => path.display().to_string(),
        };
        clean(&said)
    }

    /// The folder project `p` is, as the salon's project line ends: live,
    /// its checkout (the folder itself without git); the demo's, the one
    /// `:open` or `:solo` was given, else `~/src/NAME`. None in an
    /// archive's view.
    pub fn project_folder(&self, p: ProjectId) -> Option<String> {
        match &self.engine {
            Engine::Live(live) => Some(self.shown_path(live.repo_of(p))),
            Engine::Sim(sim) => Some(match sim.folder_of(p) {
                Some(path) => clean(path),
                None => format!("~/src/{}", clean(&self.world.projects[p].name)),
            }),
            Engine::View(_) => None,
        }
    }

    /// The folder agent `id` works in, as its focus says it, and the
    /// checkout when that is a worktree of it: a leader works in the
    /// checkout, a worker, its helpers and a solo in their worktree, and
    /// all of them in the folder itself without git.
    pub fn folder_of(&self, id: AgentId) -> Option<(String, Option<String>)> {
        match &self.engine {
            Engine::Live(live) => {
                let (folder, of) = live.folder_of(&self.world, id);
                Some((self.shown_path(&folder), of.map(|c| self.shown_path(c))))
            }
            Engine::Sim(_) => {
                let a = &self.world.agents[id];
                let checkout = self.project_folder(a.project)?;
                if a.role == Role::Leader || self.world.projects[a.project].no_git {
                    return Some((checkout, None));
                }
                let owner = a.parent.map_or(a, |p| &self.world.agents[p]);
                let tree = match owner.role {
                    Role::Solo => format!("solo-{}", owner.project),
                    _ => clean(&owner.name),
                };
                let name = clean(&self.world.projects[a.project].name);
                let folder = format!("~/.local/state/imece/worktrees/{name}-5e1f0a2c/{tree}");
                Some((folder, Some(checkout)))
            }
            Engine::View(_) => None,
        }
    }

    /// A key in the archives browser.
    fn archives_key(&mut self, k: KeyEvent) {
        let Some(b) = &mut self.archives else {
            return;
        };
        match b.on_key(&self.world, k) {
            None => {}
            Some(archives::Act::Close) => self.archives = None,
            Some(archives::Act::View(p, agent)) => {
                self.archives = None;
                self.view_archive(p, agent);
            }
            Some(archives::Act::Restore(p)) => {
                self.archives = None;
                self.restore_at(p);
            }
        }
    }

    /// `:forget [@NAME]`: the agent (named, else the one at hand) forgets
    /// its session; its next message starts a new one, told where it left
    /// off.
    fn forget_agent(&mut self, arg: &str) {
        // One waits at a time: the one before acts now.
        self.end_grace();
        let Some(id) = self.agent_named(arg) else {
            return;
        };
        let can = match &self.engine {
            Engine::Live(live) => live.can_forget(&self.world, id),
            Engine::Sim(sim) => sim.can_forget(&self.world, id),
            Engine::View(_) => return self.read_only(),
        };
        match can {
            Ok(()) => self.wait_grace(Undoable::Forget(id)),
            Err(why) => self.forget_refused(id, why),
        }
    }

    /// `:fork [@NAME]` (record 94): a new worker goes on from the one
    /// named, else the one at hand, to try another way; its row selected
    /// and the line opened to it, to say what. Refused with why.
    fn fork_agent(&mut self, arg: &str) {
        let Some(id) = self.agent_named(arg) else {
            return;
        };
        let forked = match &mut self.engine {
            Engine::Live(live) => live.fork(&mut self.world, id),
            Engine::Sim(sim) => sim.fork(&mut self.world, id),
            Engine::View(_) => return self.read_only(),
        };
        let t = self.lang.text();
        let was = self.world.agents[id].name.clone();
        match forked {
            Ok(new) => {
                let name = self.world.agents[new].name.clone();
                self.go_agent(new);
                self.typing = true;
                self.noting = None;
                self.input = format!("@{name} ");
                self.after = 0;
                self.menu = None;
                self.flash(fill(t.forked, &[&name, &was]));
            }
            Err(why) => self.flash(fill(t.fork_refused, &[&was, &why])),
        }
    }

    /// Agent `id` forgets its session now.
    fn forget_now(&mut self, id: AgentId) {
        let t = self.lang.text();
        let name = self.world.agents[id].name.clone();
        let forgot = match &mut self.engine {
            Engine::Live(live) => live.forget_agent(&mut self.world, id),
            Engine::Sim(sim) => sim.forget(&mut self.world, id),
            Engine::View(_) => return self.read_only(),
        };
        match forgot {
            Ok(()) => self.flash(fill(t.forgot, &[&name])),
            Err(why) => self.forget_refused(id, why),
        }
    }

    /// Why agent `id` cannot forget its session, said.
    fn forget_refused(&mut self, id: AgentId, why: Refusal) {
        let t = self.lang.text();
        let name = self.world.agents[id].name.clone();
        match why {
            Refusal::Command(why) => self.flash(fill(t.forget_refused, &[&name, &why])),
            why => self.flash(fill(t.forget_refused, &[&name, &format!("{why:?}")])),
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
            Engine::View(_) => return self.read_only(),
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
            None => self.world.shown_inbox(),
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
            Pane::Journal if self.story_on() => match self.story_item()? {
                ui::story::Item::Card(c) => Some(c),
                ui::story::Item::Line(_) => None,
            },
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
        if self.view == View::Timeline {
            let n = ui::timeline::agents(self).len();
            self.timeline_sel = match down {
                true => (self.timeline_sel + 1).min(n.saturating_sub(1)),
                false => self.timeline_sel.saturating_sub(1),
            };
            return;
        }
        match self.pane {
            Pane::Journal if self.story_on() => {
                let n = self.story_items.borrow().len();
                self.story_sel = match down {
                    true => (self.story_sel + 1).min(n.saturating_sub(1)),
                    false => self.story_sel.saturating_sub(1),
                };
            }
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
        if self.view == View::Timeline && self.zoom == Zoom::None {
            if let Some(id) = self.timeline_agent() {
                self.zoom = Zoom::Focus(id);
            }
            return;
        }
        let zoom = match self.zoom {
            Zoom::None => match (self.pane, self.selected) {
                (Pane::Inbox, _) => {
                    let open = self.inbox_rows();
                    // A setup script's ask: its bytes in a window.
                    if let Some(&i) = open.get(self.inbox_sel)
                        && self.world.inbox[i].kind == InboxKind::Script
                    {
                        self.script = Some((i, 0));
                        return;
                    }
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

    /// What agent `id` waits for the person to decide, as its focus
    /// shows it: its oldest open ask (the one its status quotes), else
    /// its change; an index into `World.inbox`.
    pub fn focus_ask(&self, id: AgentId) -> Option<usize> {
        let w = &self.world;
        let first = |kind| {
            w.inbox
                .iter()
                .position(|i| i.open && i.kind == kind && i.agent == id)
        };
        first(InboxKind::Approval).or_else(|| first(InboxKind::Change))
    }

    /// The agent whose focus or raw stream is open with no conversation
    /// over it: the one `a`, `r` and `A` decide for there.
    pub fn zoomed_agent(&self) -> Option<AgentId> {
        match self.zoom {
            Zoom::Focus(id) | Zoom::Trace(id) if self.thread.is_none() => Some(id),
            _ => None,
        }
    }

    /// `a`, `r` or `A` on agent `id`'s focus or raw stream: they decide
    /// what that agent waits for (`focus_ask`), never what happens to be
    /// selected in waiting for you, which the person does not see from
    /// here; `A` with no ask opens the archives, as anywhere. `false`
    /// leaves another key to the rest of the screen.
    fn zoom_answer_key(&mut self, id: AgentId, k: KeyEvent) -> bool {
        let approve = match k.code {
            KeyCode::Char('a') => true,
            KeyCode::Char('r') => false,
            KeyCode::Char('e') => {
                match self.focus_ask(id) {
                    Some(item) if self.world.inbox[item].kind == InboxKind::Approval => {
                        self.answer_turn(item)
                    }
                    _ => {
                        let t = self.lang.text();
                        self.flash(fill(t.turn_yes_none, &[&self.world.agents[id].name]));
                    }
                }
                return true;
            }
            KeyCode::Char('A') => {
                match self.focus_ask(id) {
                    Some(item) if self.world.inbox[item].kind == InboxKind::Approval => {
                        self.answer_always_to(item)
                    }
                    // Not the selection in waiting for you either.
                    _ => self.open_archives(),
                }
                return true;
            }
            _ => return false,
        };
        match self.focus_ask(id) {
            Some(item) => self.answer_item(item, approve),
            None => {
                let t = self.lang.text();
                self.flash(fill(t.focus_no_ask, &[&self.world.agents[id].name]));
            }
        }
        true
    }

    /// The open approval selected in waiting for you, if that is what
    /// is selected.
    fn ask_at_hand(&self) -> Option<usize> {
        let item = *self.inbox_rows().get(self.inbox_sel)?;
        (self.world.inbox[item].kind == InboxKind::Approval).then_some(item)
    }

    /// `A` on an ask: yes from now on to its commands for its project
    /// (`Live::always`); the ask and the project's others they cover go
    /// on. What cannot have one is said, and nothing is decided.
    fn answer_always(&mut self) {
        if let Some(item) = self.ask_at_hand() {
            self.answer_always_to(item);
        }
    }

    /// `e` or `:yes` on approval `item` (an index into `World.inbox`):
    /// yes to everything its agent asks until its turn ends (record 104);
    /// the ask and the agent's other open ones go on now.
    fn answer_turn(&mut self, item: usize) {
        if self.viewing_archive() {
            return self.read_only();
        }
        let id = self.world.inbox[item].agent;
        let done = match &mut self.engine {
            Engine::Live(l) => l.yes_turn(&mut self.world, item),
            Engine::Sim(s) => s.yes_turn(&mut self.world, item),
            Engine::View(_) => return self.read_only(),
        };
        if done {
            let t = self.lang.text();
            self.flash(fill(t.turn_yes_said, &[&self.world.agents[id].name]));
        }
        let open = self.inbox_rows().len();
        self.inbox_sel = self.inbox_sel.min(open.saturating_sub(1));
    }

    /// `A` on approval `item` (an index into `World.inbox`), in waiting
    /// for you or on its agent's focus.
    fn answer_always_to(&mut self, item: usize) {
        if self.viewing_archive() {
            return self.read_only();
        }
        let t = self.lang.text();
        let p = self.world.agents[self.world.inbox[item].agent].project;
        let got = match &mut self.engine {
            Engine::Live(l) => l.always(&mut self.world, item),
            Engine::Sim(s) => s.always(&mut self.world, item),
            Engine::View(_) => return self.read_only(),
        };
        let name = self.world.projects[p].name.clone();
        let said = match got {
            Ok(added) if added.is_empty() => fill(t.always_had, &[&name]),
            Ok(added) => {
                let lines: Vec<String> = added.iter().map(Standing::line).collect();
                fill(t.always_added, &[&name, &lines.join(", ")])
            }
            Err(why) => self.standing_why(&why),
        };
        self.flash(said);
        let open = self.inbox_rows().len();
        self.inbox_sel = self.inbox_sel.min(open.saturating_sub(1));
    }

    /// Why no yes from now on could be made, in the person's words.
    fn standing_why(&self, why: &standing::Why) -> String {
        let t = self.lang.text();
        match why {
            standing::Why::NotACommand => t.always_not_command.to_string(),
            standing::Why::NotPlain => t.always_not_plain.to_string(),
            standing::Why::OptionFirst(c) => fill(t.always_option, &[c]),
        }
    }

    /// `:allow [PROGRAM [WORD]]`: the project at hand's standing yes, or
    /// a rule added to it (the project's open asks it covers go on) or,
    /// named again, taken out.
    fn allow(&mut self, arg: &str) {
        let t = self.lang.text();
        let p = self.current_project();
        let name = self.world.projects[p].name.clone();
        if arg.trim().is_empty() {
            let rules = &self.world.projects[p].allowed;
            let said = if rules.is_empty() {
                fill(t.allow_none, &[&name])
            } else {
                let lines: Vec<String> = rules.iter().map(Standing::line).collect();
                fill(t.allow_list, &[&name, &lines.join(", ")])
            };
            return self.flash(said);
        }
        if self.viewing_archive() {
            return self.read_only();
        }
        let rule = match standing::named(arg) {
            Ok(rule) => rule,
            Err(why) => return self.flash(self.standing_why(&why)),
        };
        if self.world.unstand(p, &rule) {
            return self.flash(fill(t.allow_removed, &[&name, &rule.line()]));
        }
        self.world.stand(p, std::slice::from_ref(&rule));
        match &mut self.engine {
            Engine::Live(l) => l.go_on_allowed(&mut self.world, p),
            Engine::Sim(s) => s.go_on_allowed(&mut self.world, p),
            Engine::View(_) => {}
        }
        self.flash(fill(t.always_added, &[&name, &rule.line()]));
    }

    fn answer(&mut self, approve: bool) {
        if let Some(&item) = self.inbox_rows().get(self.inbox_sel) {
            self.answer_item(item, approve);
        }
    }

    /// `a` or `r` on inbox item `item` (an index into `World.inbox`),
    /// wherever the person decided it: in waiting for you or on its
    /// agent's focus.
    fn answer_item(&mut self, item: usize, approve: bool) {
        if self.viewing_archive() {
            return self.read_only();
        }
        let open = self.inbox_rows();
        let t = self.lang.text();
        if self.world.inbox[item].kind == InboxKind::Change {
            return self.decide_change(self.world.inbox[item].agent, approve);
        }
        if self.world.inbox[item].kind == InboxKind::Script {
            return self.answer_script(item, approve);
        }
        if self.world.inbox[item].kind == InboxKind::Approval {
            let name = self.world.agents[self.world.inbox[item].agent].name.clone();
            match &mut self.engine {
                Engine::Live(l) => l.decide(&mut self.world, item, approve),
                // The demo's session goes on once the item is closed.
                Engine::Sim(_) => self.world.decided(item, approve),
                Engine::View(_) => return self.read_only(),
            }
            let how = if approve { t.allowed } else { t.refused };
            self.flash(fill(how, &[&name]));
            self.inbox_sel = self.inbox_sel.min(open.len().saturating_sub(2));
            return;
        }
        if matches!(
            self.world.inbox[item].kind,
            InboxKind::Question | InboxKind::Fault
        ) {
            // A question is answered by writing back, and a vendor error
            // the person mended is told the same way: the composer opens
            // to the agent, and the item closes when the message goes.
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
        let hunks = self.chosen_of(id);
        let decided = match &mut self.engine {
            Engine::Live(l) if apply => l.apply_change(&self.world, id, hunks),
            Engine::Sim(s) if apply => s.apply_change(&self.world, id, hunks),
            Engine::Live(l) => l.reject_change(&mut self.world, id),
            Engine::Sim(s) => s.reject_change(&mut self.world, id),
            Engine::View(_) => return self.read_only(),
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

/// `text` for a POSIX shell (and fish): in single quotes, each of its own
/// as `'\''`, so nothing in it is read by the shell.
pub fn sh_quote(text: &str) -> String {
    format!("'{}'", text.replace('\'', "'\\''"))
}

/// The command line that commits what is staged with `message`: in `dir`
/// when given, only tried (`--dry-run`) when `dry`.
pub fn commit_line(message: &str, dir: Option<&Path>, dry: bool) -> String {
    let mut line = "!git".to_string();
    if let Some(d) = dir {
        line.push_str(&format!(" -C {}", sh_quote(&d.to_string_lossy())));
    }
    line.push_str(" commit");
    if dry {
        line.push_str(" --dry-run");
    }
    line.push_str(&format!(" -m {}", sh_quote(message)));
    line
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

/// The first of `marked` (positions, ascending) after `at`, or the first
/// of them when none is after it or nothing is at hand.
fn next_of(marked: &[usize], at: Option<usize>) -> Option<usize> {
    let after = marked.iter().find(|&&p| at.is_none_or(|a| p > a));
    after.or(marked.first()).copied()
}

/// Where `[` or `]` goes from `at`: the nearest of `starts` (in order)
/// before it, or after it when `forward`; none past the first or last.
fn jump(starts: &[usize], at: usize, forward: bool) -> Option<usize> {
    if forward {
        starts.iter().find(|&&s| s > at).copied()
    } else {
        starts.iter().rev().find(|&&s| s < at).copied()
    }
}

/// A run's time in whole milliseconds, as the journal keeps it.
pub fn millis(d: Duration) -> u64 {
    u64::try_from(d.as_millis()).unwrap_or(u64::MAX)
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
pub(crate) mod tests {
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
    fn a_change_s_files_fold_to_their_headers_and_the_scroll_skips_them() {
        // Lines 0-3 are x (header 0), 4-8 are y (header 4).
        let (mut app, worker) = with_change(&two_files());
        let k = |app: &mut App, c: char| app.on_key(key(KeyCode::Char(c)));
        k(&mut app, 'h');
        assert!(app.diff_folds_of(worker).folded(0), "the file at hand");
        assert!(!app.diff_folds_of(worker).folded(1));
        k(&mut app, 'j');
        assert_eq!(app.diff_scroll, 4, "over the folded file");
        k(&mut app, 'k');
        assert_eq!(app.diff_scroll, 0);
        app.on_key(key(KeyCode::Right));
        assert!(!app.diff_folds_of(worker).folded(0), "l → open it");
        k(&mut app, 'j');
        assert_eq!(app.diff_scroll, 1);
        // Folding the file of a hidden line puts the line at hand on its header.
        app.diff_scroll = 6;
        app.on_key(key(KeyCode::Left));
        assert_eq!(app.diff_scroll, 4);
        assert!(app.diff_folds_of(worker).folded(1));
        k(&mut app, 'l');
        // za toggles, zM folds them all, zR opens them.
        app.diff_scroll = 2;
        k(&mut app, 'z');
        k(&mut app, 'a');
        assert!(app.diff_folds_of(worker).folded(0));
        assert_eq!(app.diff_scroll, 0);
        k(&mut app, 'z');
        k(&mut app, 'a');
        assert!(!app.diff_folds_of(worker).folded(0));
        app.diff_scroll = 7;
        k(&mut app, 'z');
        k(&mut app, 'M');
        assert!(app.diff_folds_of(worker).folded(0) && app.diff_folds_of(worker).folded(1));
        assert_eq!(app.diff_scroll, 4);
        k(&mut app, 'G');
        assert_eq!(app.diff_scroll, 4, "the last line shown");
        app.on_key(key(KeyCode::PageUp));
        assert_eq!(app.diff_scroll, 0);
        app.on_key(key(KeyCode::PageDown));
        assert_eq!(app.diff_scroll, 4);
        k(&mut app, 'g');
        k(&mut app, 'g');
        assert_eq!(app.diff_scroll, 0);
        k(&mut app, 'z');
        k(&mut app, 'R');
        assert!(!app.diff_folds_of(worker).folded(1));
        k(&mut app, 'G');
        assert_eq!(app.diff_scroll, 8);
        // :fold and :unfold do it for the change open.
        app.command("fold");
        assert!(app.diff_folds_of(worker).folded(1));
        assert_eq!(app.diff_scroll, 4);
        app.command("unfold");
        assert!(!app.diff_folds_of(worker).folded(1));
        // `v` chooses from the whole patch: its files open.
        k(&mut app, 'h');
        k(&mut app, 'v');
        assert_eq!(app.diff_folds_of(worker), Folds::default());
        assert!(app.diff_copy_of(worker).is_some());
        app.on_key(key(KeyCode::Esc));
        // Another patch starts open.
        k(&mut app, 'h');
        assert!(app.diff_folds_of(worker).folded(1));
        let c = app.world.change_of(worker).unwrap().clone();
        app.world.change_ready(Change {
            patch: format!("{}+four\n", c.patch),
            ..c
        });
        assert_eq!(app.diff_folds_of(worker), Folds::default());
        // A conversation over the change takes the fold keys.
        assert_eq!(app.diff_zoom(), Some(worker));
        app.thread = Some(Thread {
            entries: vec![0],
            sel: 0,
            scroll: Cell::new(0),
            follow: Cell::new(false),
            page: Cell::new(10),
            copy: None,
            words: None,
            folds: Folds::default(),
            room: Cell::new(usize::MAX),
        });
        assert_eq!(app.diff_zoom(), None);
        app.command("fold");
        assert!(app.thread.as_ref().unwrap().folds.all, "the conversation's");
        assert!(!app.diff_folds_of(worker).folded(0));
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
    fn a_copy_choice_spans_its_anchor_and_cursor_either_way() {
        let down = Copying { anchor: 3, at: 5 };
        let up = Copying { anchor: 5, at: 3 };
        for c in [down, up] {
            assert_eq!(c.span(), (3, 5));
            assert_eq!(c.len(), 3);
            assert!(c.holds(3) && c.holds(4) && c.holds(5), "{c:?}");
            assert!(!c.holds(2) && !c.holds(6), "{c:?}");
        }
        assert_eq!(Copying::new(7), Copying { anchor: 7, at: 7 });
        assert_eq!(Copying::new(7).len(), 1);
    }

    #[test]
    fn v_in_a_change_chooses_lines_from_the_top_one_and_moves_like_the_view() {
        let patch: String = (0..30).map(|i| format!("+line {i}\n")).collect();
        let (mut app, worker) = with_change(&patch);
        app.diff_page.set(10);
        app.diff_scroll = 4;
        app.on_key(key(KeyCode::Char('v')));
        assert_eq!(app.diff_copy_of(worker), Some(Copying::new(4)));
        let at = |app: &App| (app.diff_copy_of(worker).unwrap().at, app.diff_scroll);
        app.on_key(key(KeyCode::Char('j')));
        app.on_key(key(KeyCode::Down));
        assert_eq!(at(&app), (6, 4), "the view stays while the cursor is in it");
        app.on_key(key(KeyCode::Char('k')));
        app.on_key(key(KeyCode::Up));
        app.on_key(key(KeyCode::Up));
        assert_eq!(at(&app), (3, 3), "above the view: it scrolls up to it");
        app.on_key(ctrl('d'));
        assert_eq!(at(&app), (8, 3), "half a page");
        app.on_key(ctrl('f'));
        assert_eq!(at(&app), (18, 9), "a page, the view just far enough");
        app.on_key(ctrl('u'));
        assert_eq!(at(&app), (13, 9));
        app.on_key(ctrl('b'));
        assert_eq!(at(&app), (3, 3));
        app.on_key(key(KeyCode::PageDown));
        assert_eq!(at(&app).0, 13);
        app.on_key(key(KeyCode::Char(' ')));
        assert_eq!(at(&app).0, 23);
        app.on_key(key(KeyCode::PageUp));
        assert_eq!(at(&app).0, 13);
        app.on_key(key(KeyCode::Char('G')));
        assert_eq!(at(&app), (29, 20), "the last line");
        app.on_key(key(KeyCode::Char('j')));
        assert_eq!(at(&app).0, 29, "no further");
        app.on_key(key(KeyCode::Home));
        assert_eq!(at(&app), (0, 0));
        app.on_key(key(KeyCode::Char('k')));
        assert_eq!(at(&app).0, 0, "no further up");
        app.on_key(key(KeyCode::End));
        assert_eq!(at(&app).0, 29);
        app.on_key(key(KeyCode::Char('g')));
        app.on_key(key(KeyCode::Char('g')));
        assert_eq!(at(&app), (0, 0), "gg");
        assert_eq!(
            app.diff_copy_of(worker).unwrap().anchor,
            4,
            "the anchor stays"
        );
        // `y` copies the lines as written and leaves.
        app.on_key(key(KeyCode::Char('y')));
        assert_eq!(
            app.copy.as_deref(),
            Some("+line 0\n+line 1\n+line 2\n+line 3\n+line 4")
        );
        assert_eq!(app.diff_copy_of(worker), None);
        assert_eq!(app.zoom, Zoom::Diff(worker), "the change stays open");
    }

    #[test]
    fn v_in_a_change_starts_on_its_last_line_at_most() {
        let (mut app, worker) = with_change("+a\n+b\n+c\n");
        app.diff_scroll = 9;
        app.on_key(key(KeyCode::Char('v')));
        assert_eq!(app.diff_copy_of(worker), Some(Copying::new(2)));
    }

    #[test]
    fn big_y_in_a_change_copies_the_new_side_or_says_there_is_none() {
        let patch = "diff --git a/x b/x\n--- a/x\n+++ b/x\n@@ -1,2 +1,2 @@\n keep\n-old\n+new\n";
        let (mut app, worker) = with_change(patch);
        app.diff_scroll = 5;
        app.on_key(key(KeyCode::Char('v')));
        app.on_key(key(KeyCode::Char('Y')));
        assert_eq!(app.copy, None, "the line chosen was removed");
        assert_eq!(flashed(&app).as_deref(), Some(Lang::En.text().copy_no_new));
        assert_eq!(
            app.diff_copy_of(worker),
            Some(Copying::new(5)),
            "still choosing"
        );
        app.on_key(key(KeyCode::Char('g')));
        app.on_key(key(KeyCode::Char('g')));
        app.on_key(key(KeyCode::Char('Y')));
        assert_eq!(app.copy.take().as_deref(), Some("keep"), "heads left out");
        assert_eq!(app.diff_copy_of(worker), None);
        app.on_key(key(KeyCode::Char('v')));
        app.on_key(key(KeyCode::Char('G')));
        app.on_key(key(KeyCode::Char('Y')));
        assert_eq!(app.copy.as_deref(), Some("keep\nnew"));
    }

    #[test]
    fn while_choosing_lines_other_keys_do_nothing_but_help_and_quit() {
        let (mut app, worker) = with_change("+a\n+b\n");
        app.on_key(key(KeyCode::Char('v')));
        for c in ['a', 'r', 'c', 'x', 'n', '>', 'w', 't'] {
            app.on_key(key(KeyCode::Char(c)));
        }
        assert!(app.world.change_of(worker).is_some(), "not applied");
        assert!(!app.typing && !app.diff_wrap && app.diff_notes.is_empty());
        assert_eq!(app.zoom, Zoom::Diff(worker));
        assert_eq!(app.diff_copy_of(worker), Some(Copying::new(0)));
        app.on_key(key(KeyCode::Char('?')));
        assert!(app.help, "? opens the help");
        app.help = false;
        app.on_key(key(KeyCode::Char('q')));
        assert_eq!(app.pending, Some('q'), "q asks to quit");
        app.pending = None;
        app.on_key(key(KeyCode::Esc));
        assert_eq!(app.diff_copy_of(worker), None, "esc leaves the choosing");
        assert_eq!(app.zoom, Zoom::Diff(worker), "and only that");
        app.on_key(key(KeyCode::Char('v')));
        app.on_key(key(KeyCode::Char('v')));
        assert_eq!(app.diff_copy_of(worker), None, "v again leaves too");
    }

    #[test]
    fn a_copy_choice_goes_with_its_change_and_its_view() {
        let (mut app, worker) = with_change("+a\n+b\n");
        app.on_key(key(KeyCode::Char('v')));
        // A newer change from the worker has other lines.
        let mut newer = a_change(worker);
        newer.patch = "+c\n".into();
        app.world.change_ready(newer);
        assert_eq!(app.diff_copy_of(worker), None);
        app.on_key(key(KeyCode::Char('v')));
        assert!(app.diff_copy_of(worker).is_some());
        // Another view: gone with the next key.
        app.zoom = Zoom::None;
        app.on_key(key(KeyCode::Char('j')));
        assert_eq!(app.diff_copy, None);
        // The patch's hash alone does not stand for another worker.
        let other = (app.world.agents.iter())
            .find(|a| a.id != worker)
            .map(|a| a.id)
            .unwrap();
        let key_of = patch_key(&app.world.change_of(worker).unwrap().patch);
        app.diff_copy = Some((other, key_of, Copying::new(0)));
        assert_eq!(app.diff_copy_of(worker), None);
    }

    #[test]
    fn v_in_a_conversation_chooses_lines_across_messages_and_y_copies_them() {
        let (mut app, worker) = with_thread();
        app.world.said(worker, "one\ntwo");
        app.world.said(worker, "```\nthree\n```");
        app.on_key(key(KeyCode::Char('G')));
        let th = app.thread.as_ref().unwrap();
        assert_eq!((th.entries.len(), th.sel), (3, 2));
        let lines = ui::thread_lines(app.lang, &app.world, &th.entries, th.room.get());
        let texts: Vec<&str> = lines.iter().map(|l| l.text.as_str()).collect();
        assert_eq!(texts.len(), 7, "{texts:?}");
        assert_eq!(
            [texts[1], texts[3], texts[4], texts[6]],
            ["first", "one", "two", "three"],
            "a fence is not a line"
        );
        let head = texts[5].to_string();
        assert!(head.contains(&app.world.agents[worker].name), "{head}");
        let th = |app: &App| {
            let th = app.thread.as_ref().unwrap();
            (th.copy, th.sel)
        };
        app.on_key(key(KeyCode::Char('v')));
        assert_eq!(th(&app), (Some(Copying::new(5)), 2), "the message's head");
        app.on_key(key(KeyCode::Char('j')));
        app.on_key(key(KeyCode::Char('j')));
        assert_eq!(th(&app).0.unwrap().at, 6, "no further than the last");
        for _ in 0..3 {
            app.on_key(key(KeyCode::Char('k')));
        }
        assert_eq!(
            th(&app),
            (Some(Copying { anchor: 5, at: 3 }), 1),
            "the message follows"
        );
        app.on_key(key(KeyCode::Char('r')));
        assert_eq!(app.copy, None, "r is not taken while choosing");
        app.on_key(key(KeyCode::Char('j')));
        app.on_key(key(KeyCode::Char('y')));
        assert_eq!(app.copy, Some(format!("two\n\n{head}")));
        assert_eq!(th(&app).0, None, "y leaves the choosing");
        assert!(app.thread.is_some(), "not the conversation");
    }

    /// A conversation of the person with a worker: the person's "first",
    /// then the worker's answer `alpha **beta** gamma` / `delta epsilon`,
    /// selected. Its lines: 0 and 2 heads, 1 "first", 3 and 4 the answer.
    fn talk_with_words() -> (App, AgentId) {
        let (mut app, worker) = with_thread();
        app.world.note(Event::ToYou {
            from: worker,
            kind: MsgKind::Answer,
            text: "alpha **beta** gamma\ndelta epsilon".into(),
        });
        app.on_key(key(KeyCode::Char('G')));
        assert_eq!(app.thread.as_ref().unwrap().sel, 1);
        (app, worker)
    }

    fn keys(app: &mut App, keys: &str) {
        for c in keys.chars() {
            app.on_key(key(KeyCode::Char(c)));
        }
    }

    /// The choice of the open conversation: the cursor's line, the anchor's,
    /// the words.
    fn choice(app: &App) -> (usize, usize, Option<(usize, usize)>) {
        let th = app.thread.as_ref().unwrap();
        let c = th.copy.unwrap();
        (c.at, c.anchor, th.words)
    }

    #[test]
    fn choosing_word_by_word_steps_over_words_and_lines_and_copies_them() {
        let (mut app, _) = talk_with_words();
        // From one line chosen whole: the cursor on its first word, then on.
        keys(&mut app, "vjs");
        assert_eq!(choice(&app), (3, 3, None));
        keys(&mut app, "w");
        assert_eq!(choice(&app), (3, 3, Some((0, 1))));
        keys(&mut app, "y");
        assert_eq!(
            app.copy.as_deref(),
            Some("alpha beta"),
            "the words as drawn"
        );
        assert!(app.thread.as_ref().unwrap().copy.is_none(), "y leaves");
        // The whole line is its source; over its end, the next line.
        keys(&mut app, "vjswl");
        assert_eq!(choice(&app), (3, 3, Some((0, 2))));
        keys(&mut app, "y");
        assert_eq!(app.copy.as_deref(), Some("alpha **beta** gamma"));
        keys(&mut app, "vjsww");
        app.on_key(key(KeyCode::Right));
        assert_eq!(choice(&app), (4, 3, Some((0, 0))));
        keys(&mut app, "y");
        assert_eq!(app.copy.as_deref(), Some("alpha **beta** gamma\ndelta"));
        // Back over the start of a line, to the last word of the one above.
        keys(&mut app, "vjjs");
        assert_eq!(choice(&app), (4, 4, None));
        keys(&mut app, "b");
        assert_eq!(choice(&app), (3, 4, Some((0, 2))));
        keys(&mut app, "h");
        app.on_key(key(KeyCode::Left));
        assert_eq!(choice(&app), (3, 4, Some((0, 0))));
        keys(&mut app, "y");
        assert_eq!(app.copy.as_deref(), Some("alpha **beta** gamma\ndelta"));
        // Into the message above: it is the one selected, kept in view.
        keys(&mut app, "vjs");
        app.thread.as_ref().unwrap().follow.set(false);
        keys(&mut app, "bbbbbb");
        let th = app.thread.as_ref().unwrap();
        assert_eq!((choice(&app), th.sel), ((1, 3, Some((0, 0))), 0));
        assert!(th.follow.get());
    }

    #[test]
    fn a_choice_of_lines_turns_word_wise_from_the_ends_it_had() {
        let (mut app, worker) = talk_with_words();
        let name = app.world.agents[worker].name.clone();
        // Downward: from the start of its first line; the cursor at the
        // end of its last, so `b` gives that word back.
        keys(&mut app, "vjj");
        assert_eq!(choice(&app), (4, 2, None));
        keys(&mut app, "b");
        assert_eq!(choice(&app), (4, 2, Some((0, 0))));
        keys(&mut app, "y");
        let head = format!("{name} → you [answer]");
        let copied = app.copy.clone().unwrap();
        assert!(
            copied.ends_with(&format!("{head}\nalpha **beta** gamma\ndelta")),
            "{copied}"
        );
        // Upward: from the end of its last line; the cursor at the start
        // of its first.
        keys(&mut app, "vjjjsk");
        assert_eq!(choice(&app), (3, 4, None));
        keys(&mut app, "w");
        assert_eq!(choice(&app), (3, 4, Some((1, 1))));
        keys(&mut app, "y");
        assert_eq!(app.copy.as_deref(), Some("beta gamma\ndelta epsilon"));
        // Word-wise, j and k keep the cursor on a word of the line it goes to.
        keys(&mut app, "vjswwj");
        assert_eq!(choice(&app), (4, 3, Some((0, 1))), "two words, not three");
        keys(&mut app, "k");
        assert_eq!(choice(&app), (3, 3, Some((0, 1))));
        // `s` starts again at the cursor's word.
        keys(&mut app, "ws");
        assert_eq!(choice(&app), (3, 3, Some((2, 2))));
        keys(&mut app, "y");
        assert_eq!(app.copy.as_deref(), Some("gamma"));
        // `v` starts whole lines again.
        keys(&mut app, "v");
        assert_eq!(choice(&app), (2, 2, None));
    }

    #[test]
    fn c_notes_the_words_chosen_and_n_sends_them_to_who_wrote_them() {
        let (mut app, worker) = talk_with_words();
        let answer = app.world.journal.len() - 1;
        keys(&mut app, "vjswc");
        assert!(app.typing && app.input.is_empty());
        assert!(app.thread.as_ref().unwrap().copy.is_none(), "c leaves");
        let note = TalkNote {
            to: worker,
            entry: answer,
            quote: "alpha beta".into(),
            text: String::new(),
        };
        assert_eq!(app.noting, Some(Noting::Words(note.clone())));
        app.input = "say why".into();
        app.on_key(key(KeyCode::Enter));
        assert!(!app.typing && app.noting.is_none());
        let kept = TalkNote {
            text: "say why".into(),
            ..note.clone()
        };
        assert_eq!(app.talk_notes, std::slice::from_ref(&kept));
        assert_eq!(
            app.flash.as_ref().unwrap().0,
            fill(app.lang.text().talk_noted, &["1"])
        );
        // The same words again: the note to write over.
        keys(&mut app, "vjswc");
        assert_eq!(app.input, "say why");
        app.input = "say why, shortly".into();
        app.on_key(key(KeyCode::Enter));
        assert_eq!(app.talk_notes.len(), 1, "in place of the one there was");
        // Other words of the same message, or the same words of another
        // message: a note of their own.
        keys(&mut app, "vjjsc");
        app.input = "other words".into();
        app.on_key(key(KeyCode::Enter));
        app.talk_notes.push(TalkNote {
            entry: answer - 1,
            ..app.talk_notes[0].clone()
        });
        assert_eq!(app.talk_notes.len(), 3);
        keys(&mut app, "vjswc");
        app.input.clear();
        app.on_key(key(KeyCode::Enter));
        assert_eq!(app.talk_notes.len(), 2, "written empty, only that one goes");
        assert_eq!(app.talk_notes[0].quote, "delta epsilon");
        app.talk_notes.clear();
        keys(&mut app, "vjswc");
        app.input = "say why, shortly".into();
        app.on_key(key(KeyCode::Enter));
        // The person's own line: to the agent it went to.
        keys(&mut app, "ggvjsc");
        app.input = "and this".into();
        app.on_key(key(KeyCode::Enter));
        assert_eq!(app.talk_notes[1].to, worker);
        assert_eq!(app.talk_notes[1].quote, "first");
        let was = app.world.journal.len();
        keys(&mut app, "n");
        assert!(app.talk_notes.is_empty());
        let Event::YouSaid { to, text } = &app.world.journal[was].event else {
            panic!("{:?}", app.world.journal[was].event);
        };
        assert_eq!(*to, worker);
        let message = "Notes on words from our conversation, each under the words it is \
                       about.\n\n> first\nand this\n\n> alpha beta\nsay why, shortly";
        assert_eq!(text, message, "one message, the older line first");
        let name = app.world.agents[worker].name.clone();
        assert_eq!(
            app.flash.as_ref().unwrap().0,
            fill(app.lang.text().talk_notes_sent, &["2", &name])
        );
        keys(&mut app, "n");
        assert_eq!(app.flash.as_ref().unwrap().0, app.lang.text().no_talk_notes);
    }

    #[test]
    fn notes_on_a_talk_between_agents_go_each_to_its_writer() {
        let mut app = App::new(1, 2, 0, 1, Lang::En);
        app.world.journal.clear();
        for (from, to, text) in [(0, 1, "do the parser"), (1, 0, "done the parser")] {
            let id = app.world.number();
            app.world.note(Event::Sent {
                from,
                to,
                kind: MsgKind::Inform,
                text: text.into(),
                id,
                re: None,
            });
        }
        app.open_thread(0);
        // The lead's line, then the worker's.
        keys(&mut app, "vjsc");
        app.input = "to the lead".into();
        app.on_key(key(KeyCode::Enter));
        keys(&mut app, "jvjsc");
        app.input = "to the worker".into();
        app.on_key(key(KeyCode::Enter));
        let to: Vec<AgentId> = app.talk_notes.iter().map(|n| n.to).collect();
        assert_eq!(to, [0, 1]);
        keys(&mut app, "n");
        let said: Vec<(AgentId, String)> = (app.world.journal.iter())
            .filter_map(|e| match &e.event {
                Event::YouSaid { to, text } => Some((*to, text.clone())),
                _ => None,
            })
            .collect();
        assert_eq!(said.len(), 2, "{said:?}");
        assert_eq!(said[0].0, 0);
        assert!(
            said[0]
                .1
                .ends_with("On message 1:\n> do the parser\nto the lead"),
            "{said:?}"
        );
        assert_eq!(said[1].0, 1);
        assert!(
            said[1]
                .1
                .ends_with("On message 2:\n> done the parser\nto the worker")
        );
        let names = format!("{}, {}", app.world.agents[0].name, app.world.agents[1].name);
        assert_eq!(
            app.flash.as_ref().unwrap().0,
            fill(app.lang.text().talk_notes_sent, &["2", &names])
        );
    }

    #[test]
    fn a_note_that_could_not_go_stays_and_an_empty_one_goes_away() {
        let (mut app, _) = talk_with_words();
        keys(&mut app, "vjc");
        app.input = "keep me".into();
        app.on_key(key(KeyCode::Enter));
        app.world.budget = 0;
        keys(&mut app, "n");
        assert_eq!(app.talk_notes.len(), 1, "the budget is spent");
        assert!(!app.flash.as_ref().unwrap().0.contains("went to"));
        // Written empty, the note is taken away.
        keys(&mut app, "vjc");
        assert_eq!(app.input, "keep me");
        app.input.clear();
        app.on_key(key(KeyCode::Enter));
        assert!(app.talk_notes.is_empty());
        assert_eq!(app.flash.as_ref().unwrap().0, app.lang.text().note_dropped);
        // Blank words take none, and the choice stays.
        app.world.note(Event::ToYou {
            from: 1,
            kind: MsgKind::Answer,
            text: "a\n\nb".into(),
        });
        keys(&mut app, "Gvjjsc");
        assert!(!app.typing && app.noting.is_none());
        assert_eq!(app.flash.as_ref().unwrap().0, app.lang.text().note_blank);
        assert!(app.thread.as_ref().unwrap().copy.is_some());
        // Esc while writing one keeps nothing.
        keys(&mut app, "kc");
        app.on_key(key(KeyCode::Esc));
        assert!(app.noting.is_none() && app.talk_notes.is_empty());
    }

    #[test]
    fn h_l_and_z_fold_messages_of_a_conversation_and_the_choice_is_kept() {
        let (mut app, _) = talk_with_words();
        let collapsed = app.collapsed.clone();
        let folded = |app: &App| {
            let th = app.thread.as_ref().unwrap();
            (th.folds.folded(0), th.folds.folded(1))
        };
        let follows = |app: &App| app.thread.as_ref().unwrap().follow.take();
        follows(&app);
        keys(&mut app, "h");
        assert_eq!(folded(&app), (false, true));
        assert!(follows(&app), "the selected message stays in view");
        keys(&mut app, "l");
        assert_eq!(folded(&app), (false, false));
        assert!(follows(&app));
        app.on_key(key(KeyCode::Left));
        assert_eq!(folded(&app), (false, true));
        app.on_key(key(KeyCode::Right));
        assert_eq!(folded(&app), (false, false));
        keys(&mut app, "za");
        assert_eq!(folded(&app), (false, true));
        assert!(follows(&app));
        keys(&mut app, "za");
        assert_eq!(folded(&app), (false, false));
        assert_eq!(app.collapsed, collapsed, "not the salon's folds");
        keys(&mut app, "zM");
        assert_eq!(folded(&app), (true, true));
        assert!(follows(&app));
        keys(&mut app, "l");
        assert_eq!(folded(&app), (true, false), "one opened of all folded");
        // The next conversation opens folded too, until zR.
        app.on_key(key(KeyCode::Esc));
        app.open_thread(0);
        assert_eq!(folded(&app), (true, true));
        keys(&mut app, "zR");
        assert_eq!(folded(&app), (false, false));
        assert!(!app.fold_talk);
        app.on_key(key(KeyCode::Esc));
        app.open_thread(0);
        assert_eq!(folded(&app), (false, false));
        // `:fold` and `:unfold` with a conversation open fold it.
        app.command("fold");
        assert_eq!(folded(&app), (true, true));
        assert!(app.thread.is_some() && app.fold_talk);
        app.command("unfold");
        assert_eq!(folded(&app), (false, false));
        assert_eq!(app.collapsed, collapsed);
    }

    #[test]
    fn a_message_to_an_agent_with_no_text_is_not_sent() {
        let (mut app, worker) = with_thread();
        let name = app.world.agents[worker].name.clone();
        let was = app.world.journal.len();
        app.typing = true;
        app.input = format!("@{name}   ");
        app.on_key(key(KeyCode::Enter));
        assert_eq!(app.world.journal.len(), was, "nothing said");
        assert_eq!(app.flash.as_ref().unwrap().0, app.lang.text().empty_message);
    }

    #[test]
    fn folds_say_which_messages_are_one_row() {
        let mut f = Folds::every(true);
        assert!(f.folded(0) && f.folded(7));
        f.set(7, false);
        assert!(f.folded(0) && !f.folded(7));
        f.set(7, true);
        assert_eq!(f, Folds::every(true), "back as it was");
        f.set(3, true);
        assert_eq!(f, Folds::every(true));
        let mut f = Folds::every(false);
        assert!(!f.folded(2));
        f.set(2, true);
        assert!(f.folded(2) && !f.folded(1));
        f.set(2, false);
        assert_eq!(f, Folds::default());
    }

    #[test]
    fn a_thread_s_part_is_the_words_chosen_on_a_line() {
        let (mut app, _) = with_thread();
        let th = app.thread.as_mut().unwrap();
        assert_eq!(th.part(0), None, "nothing chosen");
        th.copy = Some(Copying { anchor: 4, at: 2 });
        assert_eq!(th.part(1), None);
        assert_eq!(th.part(2), Some((0, usize::MAX)));
        assert_eq!(th.part(3), Some((0, usize::MAX)));
        assert_eq!(th.part(4), Some((0, usize::MAX)));
        assert_eq!(th.part(5), None);
        // The anchor's word at 4, the cursor's at 2: from 2.1 to 4.3.
        th.words = Some((3, 1));
        assert_eq!(th.part(1), None);
        assert_eq!(th.part(2), Some((1, usize::MAX)));
        assert_eq!(th.part(3), Some((0, usize::MAX)));
        assert_eq!(th.part(4), Some((0, 3)));
        assert_eq!(th.part(5), None);
        // On one line, either way round.
        th.copy = Some(Copying { anchor: 2, at: 2 });
        assert_eq!(th.part(2), Some((1, 3)));
        th.words = Some((1, 3));
        assert_eq!(th.part(2), Some((1, 3)));
    }

    #[test]
    fn choosing_lines_of_a_conversation_pages_and_leaves_with_esc() {
        let (mut app, worker) = with_thread();
        let long: Vec<String> = (0..20).map(|i| format!("l{i}")).collect();
        app.world.said(worker, &long.join("\n"));
        app.on_key(key(KeyCode::Char('G')));
        app.thread.as_ref().unwrap().page.set(4);
        app.on_key(key(KeyCode::Char('v')));
        let at = |app: &App| app.thread.as_ref().unwrap().copy.unwrap().at;
        assert_eq!(at(&app), 2, "head and line of the first, then this head");
        app.on_key(ctrl('d'));
        assert_eq!(at(&app), 4, "half a page of lines");
        app.on_key(ctrl('f'));
        assert_eq!(at(&app), 8);
        app.on_key(key(KeyCode::PageDown));
        assert_eq!(at(&app), 12);
        app.on_key(ctrl('u'));
        assert_eq!(at(&app), 10);
        app.on_key(ctrl('b'));
        assert_eq!(at(&app), 6);
        app.on_key(key(KeyCode::PageUp));
        assert_eq!(at(&app), 2);
        app.on_key(key(KeyCode::End));
        assert_eq!(at(&app), 22);
        app.on_key(key(KeyCode::Char('g')));
        app.on_key(key(KeyCode::Char('g')));
        assert_eq!(at(&app), 0);
        assert_eq!(app.thread.as_ref().unwrap().sel, 0, "the first message");
        app.on_key(key(KeyCode::Esc));
        assert_eq!(app.thread.as_ref().unwrap().copy, None);
        assert!(app.thread.is_some(), "esc leaves the choosing first");
        app.on_key(key(KeyCode::Esc));
        assert!(app.thread.is_none());
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
    fn a_r_and_capital_a_on_a_focus_decide_what_its_agent_waits_for() {
        let mut app = App::new(1, 2, 0, 1, Lang::En);
        let t = Lang::En.text();
        let flash = |app: &App| app.flash.as_ref().map(|(m, _)| m.clone());
        let p = app.current_project();
        let (one, two) = (
            app.world.projects[p].leader + 1,
            app.world.projects[p].leader + 2,
        );
        let name = app.world.agents[one].name.clone();
        // An ask decided before: the open ones are not the first items.
        app.world.asks_you(one, "Bash: npm install old");
        app.world.decided(0, false);
        app.world.asks_you(two, "Bash: make other");
        app.world.asks_you(one, "Bash: make first");
        app.world.asks_you(one, "Bash: make second");
        app.world.asks_you(one, "Bash: npm install zod@3");
        // Waiting for you has the other agent's ask selected: the focus
        // never decides that one, which it does not show.
        app.pane = Pane::Inbox;
        app.inbox_sel = 0;
        app.zoom = Zoom::Focus(one);
        assert_eq!(app.zoomed_agent(), Some(one));
        assert_eq!(app.focus_ask(one), Some(2), "its oldest open ask");
        assert_eq!(app.focus_ask(two), Some(1));
        let open = |app: &App| -> Vec<String> {
            (app.world.open_inbox().iter())
                .map(|&i| app.world.inbox[i].detail.clone())
                .collect()
        };
        app.on_key(key(KeyCode::Char('a')));
        assert_eq!(flash(&app), Some(fill(t.allowed, &[&name])));
        assert_eq!(
            open(&app),
            [
                "Bash: make other",
                "Bash: make second",
                "Bash: npm install zod@3"
            ]
        );
        assert!(app.world.journal.iter().any(|e| matches!(
            &e.event,
            Event::Decided { agent, allowed: true, what } if *agent == one && what == "Bash: make first"
        )));
        // On its raw stream too; `r` refuses.
        app.zoom = Zoom::Trace(one);
        app.on_key(key(KeyCode::Char('r')));
        assert_eq!(flash(&app), Some(fill(t.refused, &[&name])));
        assert!(app.world.journal.iter().any(|e| matches!(
            &e.event,
            Event::Decided { agent, allowed: false, what } if *agent == one && what == "Bash: make second"
        )));
        assert_eq!(open(&app), ["Bash: make other", "Bash: npm install zod@3"]);
        // `A`: yes from now on to it, not the archives.
        app.zoom = Zoom::Focus(one);
        app.on_key(key(KeyCode::Char('A')));
        assert!(app.archives.is_none());
        assert_eq!(
            flash(&app),
            Some(format!(
                "{}: yes from now on to npm install …",
                app.world.projects[p].name
            ))
        );
        assert_eq!(open(&app), ["Bash: make other"]);
        assert_eq!(app.focus_ask(one), None);
        // Nothing of its own waits: said, and the other agent's ask stays.
        app.flash = None;
        app.on_key(key(KeyCode::Char('a')));
        assert_eq!(flash(&app), Some(fill(t.focus_no_ask, &[&name])));
        app.flash = None;
        app.on_key(key(KeyCode::Char('r')));
        assert_eq!(flash(&app), Some(fill(t.focus_no_ask, &[&name])));
        assert_eq!(open(&app), ["Bash: make other"]);
        // `A` with no ask is the archives, as anywhere (none here).
        app.on_key(key(KeyCode::Char('A')));
        assert_eq!(flash(&app), Some(t.no_archives.to_string()));
        // A conversation over the focus takes the keys: nothing decided.
        app.zoom = Zoom::Focus(two);
        app.open_thread(0);
        assert!(app.thread.is_some());
        assert_eq!(app.zoomed_agent(), None);
        app.thread = None;
        assert_eq!(app.zoomed_agent(), Some(two));
        app.zoom = Zoom::Diff(two);
        assert_eq!(app.zoomed_agent(), None);
        app.zoom = Zoom::None;
        assert_eq!(app.zoomed_agent(), None);
        // An edit can have no yes from now on: said, nothing decided.
        app.world.asks_you(one, "Edit: package.json");
        app.zoom = Zoom::Focus(one);
        app.on_key(key(KeyCode::Char('A')));
        assert!(app.archives.is_none());
        assert_eq!(flash(&app), Some(t.always_not_command.to_string()));
        assert_eq!(open(&app).len(), 2);
    }

    #[test]
    fn e_says_yes_to_all_an_agent_asks_until_its_turn_ends_from_the_inbox_a_focus_or_colon_yes() {
        let mut app = App::new(1, 2, 0, 1, Lang::En);
        let t = Lang::En.text();
        let flash = |app: &App| app.flash.as_ref().map(|(m, _)| m.clone());
        let p = app.current_project();
        let (one, two) = (
            app.world.projects[p].leader + 1,
            app.world.projects[p].leader + 2,
        );
        let (n1, n2) = (
            app.world.agents[one].name.clone(),
            app.world.agents[two].name.clone(),
        );
        let open = |app: &App| -> Vec<String> {
            (app.world.open_inbox().iter())
                .map(|&i| app.world.inbox[i].detail.clone())
                .collect()
        };
        app.world.asks_you(two, "Bash: make other");
        app.world.asks_you(one, "Bash: make a");
        app.world.asks_you(one, "Edit: x");
        // In waiting for you, on the second row: agent one's.
        app.pane = Pane::Inbox;
        app.inbox_sel = 1;
        app.on_key(key(KeyCode::Char('e')));
        assert_eq!(flash(&app), Some(fill(t.turn_yes_said, &[&n1])));
        assert_eq!(open(&app), ["Bash: make other"]);
        assert_eq!(app.world.agents[one].turn_yes, Some(1));
        assert_eq!(app.inbox_sel, 0, "the selection stays on a row");
        // Not on an ask: nothing.
        app.world.asked_you(two, "Which?");
        app.inbox_sel = 1;
        app.flash = None;
        app.on_key(key(KeyCode::Char('e')));
        assert_eq!(flash(&app), None);
        assert_eq!(app.world.agents[two].turn_yes, None);
        // Nor off waiting for you.
        app.pane = Pane::Salon;
        app.inbox_sel = 0;
        app.on_key(key(KeyCode::Char('e')));
        assert_eq!(app.world.agents[two].turn_yes, None);
        // On a focus: its own agent's, whatever is selected elsewhere.
        app.zoom = Zoom::Focus(one);
        app.flash = None;
        app.on_key(key(KeyCode::Char('e')));
        assert_eq!(flash(&app), Some(fill(t.turn_yes_none, &[&n1])));
        app.zoom = Zoom::Trace(two);
        app.on_key(key(KeyCode::Char('e')));
        assert_eq!(flash(&app), Some(fill(t.turn_yes_said, &[&n2])));
        assert_eq!(open(&app), ["Which?"]);
        assert_eq!(app.world.agents[two].turn_yes, Some(0));
        app.zoom = Zoom::None;
        // `:yes @NAME`, and the agent at hand.
        app.world.agents[two].turn_yes = None;
        app.world.asks_you(two, "Bash: make b");
        app.command(&format!("yes @{n1}"));
        assert_eq!(flash(&app), Some(fill(t.turn_yes_none, &[&n1])));
        app.command(&format!("yes @{n2}"));
        assert_eq!(flash(&app), Some(fill(t.turn_yes_said, &[&n2])));
        assert_eq!(open(&app), ["Which?"]);
        app.selected = Sel::Agent(two);
        app.pane = Pane::Salon;
        assert_eq!(app.command_note("yes"), format!("({n2})"));
        app.command("yes");
        assert_eq!(flash(&app), Some(fill(t.turn_yes_none, &[&n2])));
    }

    #[test]
    fn a_and_r_on_a_focus_decide_its_workers_change_after_its_asks() {
        use crate::live::testing::{hold_change, idle_live, team};
        let t = Lang::En.text();
        let flash = |app: &App| app.flash.as_ref().map(|(m, _)| m.clone());
        let mut w = team(1);
        w.change_ready(Change {
            agent: 1,
            title: "did it".into(),
            files: Vec::new(),
            patch: String::new(),
            runs: Vec::new(),
        });
        w.asks_you(1, "Bash: make");
        let mut live = idle_live(2);
        hold_change(&mut live, 1);
        let mut app = App::with(w, Engine::Live(Box::new(live)), Lang::En);
        app.zoom = Zoom::Focus(1);
        let change = app
            .world
            .inbox
            .iter()
            .position(|i| i.kind == InboxKind::Change);
        let ask = app
            .world
            .inbox
            .iter()
            .position(|i| i.kind == InboxKind::Approval);
        assert_eq!(app.focus_ask(1), ask, "the ask first, though it came later");
        // `A` on a change is not a yes from now on: the archives.
        app.world.inbox[ask.unwrap()].open = false;
        assert_eq!(app.focus_ask(1), change);
        app.on_key(key(KeyCode::Char('A')));
        assert_eq!(flash(&app), Some(t.no_archives.to_string()));
        assert!(app.world.change_of(1).is_some());
        // A change is no ask: `e` and `:yes` give no yes for the turn.
        app.on_key(key(KeyCode::Char('e')));
        assert_eq!(flash(&app), Some(fill(t.turn_yes_none, &["claude-1"])));
        app.flash = None;
        app.command("yes @claude-1");
        assert_eq!(flash(&app), Some(fill(t.turn_yes_none, &["claude-1"])));
        assert_eq!(app.world.agents[1].turn_yes, None);
        assert!(app.world.change_of(1).is_some());
        app.on_key(key(KeyCode::Char('a')));
        assert_eq!(flash(&app), Some(fill(t.applying, &["claude-1"])));
        assert_eq!(app.zoom, Zoom::Focus(1), "the focus stays open");
        let mut live = idle_live(2);
        hold_change(&mut live, 1);
        let mut w = team(1);
        w.change_ready(Change {
            agent: 1,
            title: "did it".into(),
            files: Vec::new(),
            patch: String::new(),
            runs: Vec::new(),
        });
        let mut app = App::with(w, Engine::Live(Box::new(live)), Lang::En);
        app.zoom = Zoom::Trace(1);
        app.on_key(key(KeyCode::Char('r')));
        assert_eq!(flash(&app), Some(fill(t.rejected_change, &["claude-1"])));
        assert!(app.world.change_of(1).is_none());
    }

    #[test]
    fn the_folder_an_agent_works_in_is_its_checkout_or_its_worktree_with_home_as_tilde() {
        use crate::live::testing::{idle_live_in, team};
        let mut w = team(1);
        w.agents.push(Agent::new(
            2,
            0,
            "claude-1.1".into(),
            Vendor::Claude,
            Role::Helper,
            Status::Working,
            0,
        ));
        w.agents[2].parent = Some(1);
        let live = idle_live_in(Path::new("/home/me/src/repo"), 3);
        let mut app = App::with(w, Engine::Live(Box::new(live)), Lang::En);
        app.home = Some(PathBuf::from("/home/me"));
        assert_eq!(app.project_folder(0).as_deref(), Some("~/src/repo"));
        assert_eq!(
            app.folder_of(0),
            Some(("~/src/repo".into(), None)),
            "the lead"
        );
        let tree = Some(("/nonexistent/claude-1".into(), Some("~/src/repo".into())));
        assert_eq!(app.folder_of(1), tree);
        assert_eq!(app.folder_of(2), tree, "a helper works in its worker's");
        // Not under the home: as it is; the home itself is `~`.
        app.home = Some(PathBuf::from("/home/other"));
        assert_eq!(app.project_folder(0).as_deref(), Some("/home/me/src/repo"));
        app.home = Some(PathBuf::from("/home/me/src/repo"));
        assert_eq!(app.project_folder(0).as_deref(), Some("~"));
        app.home = None;
        assert_eq!(app.project_folder(0).as_deref(), Some("/home/me/src/repo"));
        // Without git, everyone works in the folder itself.
        if let Engine::Live(l) = &mut app.engine {
            l.set_git(0, false);
        }
        assert_eq!(app.folder_of(1), Some(("/home/me/src/repo".into(), None)));
        // A control character in a path is never drawn.
        let live = idle_live_in(Path::new("/x/\x1b[31mred"), 1);
        let app = App::with(team(0), Engine::Live(Box::new(live)), Lang::En);
        assert!(!app.project_folder(0).unwrap().contains('\x1b'));
    }

    #[test]
    fn the_demo_says_the_folders_as_live_does() {
        let mut app = App::new(1, 2, 0, 1, Lang::En);
        let lead = app.world.projects[0].leader;
        let name = app.world.projects[0].name.clone();
        let checkout = format!("~/src/{name}");
        assert_eq!(app.project_folder(0), Some(checkout.clone()));
        assert_eq!(app.folder_of(lead), Some((checkout.clone(), None)));
        let worker = lead + 1;
        let worker_name = app.world.agents[worker].name.clone();
        assert_eq!(
            app.folder_of(worker),
            Some((
                format!("~/.local/state/imece/worktrees/{name}-5e1f0a2c/{worker_name}"),
                Some(checkout.clone())
            ))
        );
        // A helper works in its worker's tree.
        app.world.agents[worker + 1].parent = Some(worker);
        assert_eq!(app.folder_of(worker + 1), app.folder_of(worker));
        // A project opened at a path is there.
        app.home = None;
        app.command("open ~/work/shop");
        let p = app.world.project_by_name("shop").unwrap();
        assert_eq!(app.project_folder(p).as_deref(), Some("~/work/shop"));
        // A solo's own tree; without git, the folder itself.
        app.command("solo ~/work/alone claude");
        let solo = app
            .world
            .agents
            .iter()
            .rfind(|a| a.role == Role::Solo)
            .unwrap()
            .id;
        let sp = app.world.agents[solo].project;
        assert_eq!(
            app.folder_of(solo),
            Some((
                format!(
                    "~/.local/state/imece/worktrees/{}-5e1f0a2c/solo-{sp}",
                    app.world.projects[sp].name
                ),
                Some("~/work/alone".into())
            ))
        );
        app.world.projects[sp].no_git = true;
        assert_eq!(app.folder_of(solo), Some(("~/work/alone".into(), None)));
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
        assert_eq!(app.command_note("story"), "(off)");
        app.story = true;
        assert_eq!(app.command_note("story"), "(on)");
        app.story = false;
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
    fn a_runs_time_is_kept_in_whole_milliseconds() {
        assert_eq!(millis(Duration::from_micros(1_234_567)), 1_234);
        assert_eq!(millis(Duration::ZERO), 0);
        assert_eq!(millis(Duration::MAX), u64::MAX, "too long to count");
    }

    #[test]
    fn writing_colon_theme_name_previews_it_and_only_sending_keeps_it() {
        let mut app = App::new(1, 1, 0, 1, Lang::En);
        theme::choose("terminal").unwrap();
        let typed =
            |app: &mut App, s: &str| s.chars().for_each(|c| app.on_key(key(KeyCode::Char(c))));
        typed(&mut app, ":theme ");
        assert_eq!(
            (pal_name(), app.theme_was.clone()),
            ("terminal".into(), None)
        );
        typed(&mut app, "dark");
        assert_eq!(pal_name(), "dark", "previewed");
        assert_eq!(app.theme(), "terminal", "not chosen");
        app.on_key(key(KeyCode::Backspace));
        assert_eq!(
            (pal_name(), app.theme_was.clone()),
            ("terminal".into(), None)
        );
        typed(&mut app, "k");
        assert_eq!(pal_name(), "dark");
        // Given up: back to what was.
        app.on_key(key(KeyCode::Esc));
        app.on_key(key(KeyCode::Esc));
        assert!(!app.typing);
        assert_eq!(
            (pal_name(), app.theme()),
            ("terminal".into(), "terminal".into())
        );
        // Stepping through the menu wears each one in turn.
        typed(&mut app, ":theme ");
        app.on_key(key(KeyCode::Tab));
        let first = app.input.strip_prefix(":theme ").unwrap().to_string();
        assert_eq!(pal_name(), first);
        app.on_key(key(KeyCode::Tab));
        let second = app.input.strip_prefix(":theme ").unwrap().to_string();
        assert_ne!(first, second);
        assert_eq!(pal_name(), second);
        app.on_key(key(KeyCode::Tab));
        let third = app.input.strip_prefix(":theme ").unwrap().to_string();
        assert_eq!(pal_name(), third);
        assert_eq!(app.theme(), "terminal", "what was before the first one");
        // Sent: kept.
        app.on_key(key(KeyCode::Esc));
        app.on_key(key(KeyCode::Esc));
        assert_eq!(pal_name(), "terminal");
        typed(&mut app, ":theme light");
        assert_eq!(
            (pal_name(), app.theme()),
            ("light".into(), "terminal".into())
        );
        app.on_key(key(KeyCode::Enter));
        assert_eq!((pal_name(), app.theme()), ("light".into(), "light".into()));
        assert_eq!(app.theme_was, None);
        // A paste names one too.
        typed(&mut app, ":theme ");
        app.paste("dark");
        assert_eq!((pal_name(), app.theme()), ("dark".into(), "light".into()));
        // Only after `:theme `, and only while writing.
        app.on_key(key(KeyCode::Esc));
        app.on_key(key(KeyCode::Esc));
        app.input = ":theme dark".into();
        app.preview_theme();
        assert_eq!(pal_name(), "light", "not writing");
        app.typing = true;
        app.input = "theme dark".into();
        app.preview_theme();
        assert_eq!(pal_name(), "light");
        theme::choose("terminal").unwrap();
    }

    fn pal_name() -> String {
        theme::pal().name.clone()
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

    /// A quiet world of one project and four idle workers.
    fn four_idle() -> (App, Vec<AgentId>) {
        let mut app = App::new(1, 4, 0, 1, Lang::En);
        app.world.inbox.clear();
        app.unseen.clear();
        for a in &mut app.world.agents {
            a.status = Status::Idle;
            a.talk = None;
            a.starting = false;
        }
        let workers: Vec<AgentId> = app
            .world
            .agents
            .iter()
            .filter(|a| a.role == Role::Worker)
            .map(|a| a.id)
            .collect();
        assert_eq!(workers.len(), 4);
        (app, workers)
    }

    #[test]
    fn ampersand_lists_where_to_go_what_needs_you_first_frozen_and_a_digit_goes_at_once() {
        let (mut app, w) = four_idle();
        let t = Lang::En.text();
        app.world.agents[w[2]].status = Status::Blocked;
        app.world.agents[w[2]].topic = "the JWT clock, the refresh and the cookie".into();
        app.world.inbox.push(InboxItem {
            kind: InboxKind::Question,
            agent: w[1],
            detail: "which index?".into(),
            researcher: None,
            open: true,
            tick: 0,
            card: None,
        });
        app.world.agents[w[1]].status = Status::WaitingHuman;
        app.unseen.insert(w[3]);
        app.world.agents[w[0]].status = Status::Working;
        let name = |app: &App, id: AgentId| app.world.agents[id].name.clone();
        app.on_key(KeyEvent::from(KeyCode::Char('&')));
        assert!(app.typing && app.input == "&", "{}", app.input);
        // Stuck, waiting, done and not seen; then the working one before the
        // idle ones; the project last.
        let names: Vec<String> = app.jumps.iter().map(|j| j.name.clone()).collect();
        assert_eq!(names[0], name(&app, w[2]));
        assert_eq!(names[1], name(&app, w[1]));
        assert_eq!(names[2], name(&app, w[3]));
        assert_eq!(names[3], name(&app, w[0]));
        assert_eq!(names.last().unwrap(), &app.world.projects[0].name);
        assert_eq!(app.jumps.len(), app.world.agents.len() + 1);
        assert_eq!(
            app.jumps.iter().filter(|j| j.needs).count(),
            3,
            "stuck, waiting and done need the person"
        );
        // Each agent with its glyph and status; a project with its word.
        assert!(
            app.jumps[0].note.starts_with("✕ BLOCKED \"the J"),
            "{}",
            app.jumps[0].note
        );
        assert_eq!(app.jumps[1].note, format!("! {}", t.waiting_human));
        assert_eq!(app.jumps[2].note, format!("✓ {}", t.done_unseen));
        assert!(app.jumps[3].note.starts_with("● "), "{}", app.jumps[3].note);
        assert_eq!(
            app.jumps[0].note.chars().count(),
            32,
            "the glyph, a space and thirty of the status: {}",
            app.jumps[0].note
        );
        assert!(app.jumps.iter().all(|j| j.note.chars().count() <= 32));
        assert_eq!(app.jumps.last().unwrap().note, t.jump_project);
        let m = app.menu.as_ref().expect("the menu");
        assert_eq!(m.items[0], format!("&1 {}", names[0]));
        assert_eq!(m.notes[0], app.jumps[0].note);
        // Frozen: a change in the world does not reorder the open list.
        app.world.agents[w[0]].status = Status::Blocked;
        type_in(&mut app, "c");
        assert_eq!(
            app.jumps.iter().map(|j| j.name.clone()).collect::<Vec<_>>(),
            names
        );
        assert_eq!(
            app.menu.as_ref().unwrap().items[0],
            format!("&1 {}", names[0])
        );
        // `!` keeps only what needed the person when the list was made.
        app.input = "&!".into();
        app.menu = app.suggest();
        assert_eq!(app.menu.as_ref().unwrap().items.len(), 3);
        // Esc closes the menu, then the line, which drops the list.
        app.on_key(KeyEvent::from(KeyCode::Esc));
        assert!(app.typing && app.menu.is_none());
        app.on_key(KeyEvent::from(KeyCode::Esc));
        assert!(app.jumps.is_empty() && !app.typing, "esc drops the list");
        // A digit right after `&` goes at once.
        app.zoom = Zoom::Focus(w[0]);
        // A fresh list: the two stuck first, by id; `2` is the second.
        app.on_key(KeyEvent::from(KeyCode::Char('&')));
        assert_eq!(
            app.jumps
                .iter()
                .take(3)
                .map(|j| j.name.clone())
                .collect::<Vec<_>>(),
            [name(&app, w[0]), name(&app, w[2]), name(&app, w[1])]
        );
        app.on_key(KeyEvent::from(KeyCode::Char('2')));
        assert!(!app.typing);
        assert_eq!(app.selected, Sel::Agent(w[2]));
        assert_eq!(
            (app.view, app.zoom, app.pane),
            (View::Salon, Zoom::None, Pane::Salon)
        );
        assert!(app.jumps.is_empty(), "the list went with the line");
        // `0` only filters.
        app.on_key(KeyEvent::from(KeyCode::Char('&')));
        app.on_key(KeyEvent::from(KeyCode::Char('0')));
        assert!(app.typing && app.input == "&0");
        assert_eq!(app.menu, None, "no tenth");
        app.on_key(KeyEvent::from(KeyCode::Esc));
        // `9` goes at once too; past the list's end (six here) it says so
        // and the line closes.
        app.on_key(KeyEvent::from(KeyCode::Char('&')));
        assert_eq!(app.jumps.len(), 6);
        app.on_key(KeyEvent::from(KeyCode::Char('9')));
        assert!(!app.typing && app.jumps.is_empty());
        assert_eq!(
            app.flash.take().map(|f| f.0).unwrap_or_default(),
            "nothing called \"9\" to go to: an agent or a project"
        );
        assert_eq!(app.selected, Sel::Agent(w[2]), "nothing moved");
        // A bare `&` and enter: nothing said, nothing moved.
        app.on_key(KeyEvent::from(KeyCode::Char('&')));
        app.on_key(KeyEvent::from(KeyCode::Enter));
        assert!(!app.typing && app.flash.is_none());
        // Someone gone is not on the list.
        app.world.agents[w[3]].gone = true;
        app.on_key(KeyEvent::from(KeyCode::Char('&')));
        assert_eq!(app.jumps.len(), 5);
        assert!(app.jumps.iter().all(|j| j.name != name(&app, w[3])));
        app.on_key(KeyEvent::from(KeyCode::Esc));
        // `f` stays on when the one gone to needs the person: its row shows.
        app.needs_only = true;
        app.command(&format!("goto {}", name(&app, w[1])));
        assert!(app.needs_only && app.selected == Sel::Agent(w[1]));
        app.command(&format!("goto {}", name(&app, w[0])));
        assert!(app.needs_only, "stuck needs the person too");
        app.world.agents[w[0]].status = Status::Idle;
        app.command(&format!("goto {}", name(&app, w[0])));
        assert!(!app.needs_only, "idle would be hidden: everyone shown");
    }

    #[test]
    fn a_word_of_a_message_completes_to_a_file_of_the_project_by_itself_with_a_slash_else_on_tab() {
        let (mut app, w) = four_idle();
        let name = app.world.agents[w[0]].name.clone();
        let tab = |app: &mut App| app.on_key(KeyEvent::from(KeyCode::Tab));
        let menu = |app: &App| {
            app.menu
                .as_ref()
                .map(|m| m.items.clone())
                .unwrap_or_default()
        };
        // The demo's files are its code map's.
        let files = app.world.files_of(0);
        assert!(files.contains(&"src/Auth/Session.cs"), "{files:?}");
        // A word with a `/` opens the menu by itself, at the word; enter
        // puts the file in its place, a space after it; enter sends.
        app.on_key(KeyEvent::from(KeyCode::Char('/')));
        type_in(&mut app, "see src/Auth/Se");
        assert_eq!(menu(&app), ["src/Auth/Session.cs"]);
        assert_eq!(app.menu.as_ref().unwrap().start, 4);
        app.on_key(KeyEvent::from(KeyCode::Enter));
        assert_eq!(app.input, "see src/Auth/Session.cs ");
        assert!(app.typing);
        app.on_key(KeyEvent::from(KeyCode::Enter));
        assert!(!app.typing);
        // Without a `/`, nothing until tab, which takes the first match.
        app.on_key(KeyEvent::from(KeyCode::Char('/')));
        type_in(&mut app, "fix Rates");
        assert!(app.menu.is_none());
        tab(&mut app);
        assert_eq!(app.input, "fix src/Shipping/Rates.cs ");
        app.on_key(KeyEvent::from(KeyCode::Esc));
        app.on_key(KeyEvent::from(KeyCode::Esc));
        // In a message to one agent, and on a `!` line (its `!` is not
        // part of the first word).
        type_in(&mut app, &format!("@{name} src/Orders/Mo"));
        assert_eq!(menu(&app), ["src/Orders/Money.cs"]);
        app.on_key(KeyEvent::from(KeyCode::Esc));
        app.on_key(KeyEvent::from(KeyCode::Esc));
        type_in(&mut app, "!src/Orders/Mo");
        assert_eq!(menu(&app), ["src/Orders/Money.cs"]);
        assert_eq!(app.menu.as_ref().unwrap().start, 1);
        app.on_key(KeyEvent::from(KeyCode::Enter));
        assert_eq!(app.input, "!src/Orders/Money.cs ");
        app.on_key(KeyEvent::from(KeyCode::Esc));
        // Not a name, not a command, not on the lines that are something
        // else, not after a space, not over several lines.
        type_in(&mut app, "@cl");
        assert!(
            menu(&app).iter().all(|i| !i.contains('/')),
            "{:?}",
            menu(&app)
        );
        app.on_key(KeyEvent::from(KeyCode::Esc));
        app.on_key(KeyEvent::from(KeyCode::Esc));
        for line in ["#src/", "&src/", "//src/"] {
            type_in(&mut app, line);
            assert!(
                menu(&app).iter().all(|i| !i.ends_with(".cs")),
                "{line}: {:?}",
                menu(&app)
            );
            app.on_key(KeyEvent::from(KeyCode::Esc));
            app.on_key(KeyEvent::from(KeyCode::Esc));
        }
        app.on_key(KeyEvent::from(KeyCode::Char('/')));
        type_in(&mut app, "src/Or ");
        assert!(app.menu.is_none());
        tab(&mut app);
        assert!(app.menu.is_none() && app.input == "src/Or ");
        type_in(&mut app, "x\\");
        app.on_key(KeyEvent::from(KeyCode::Enter));
        type_in(&mut app, "src/Or");
        assert!(app.menu.is_none(), "over lines");
        app.on_key(KeyEvent::from(KeyCode::Esc));
        // A symbol leased on the code map: `file::name`, the holder as the
        // note of the file and of its symbols.
        let card = app
            .world
            .cards
            .iter()
            .position(|c| c.project == 0)
            .expect("a card");
        app.world.cards[card].owner = Some(w[1]);
        app.world.cards[card].column = Column::Working;
        app.world.cards[card].symbols = vec![Symbol {
            file: "src/Auth/Session.cs",
            name: "Session.Start",
        }];
        for c in app.world.cards.iter_mut().filter(|c| c.id != card) {
            c.symbols.clear();
        }
        // A card that holds no lease any more (done) offers nothing.
        let done = app.world.cards.iter().position(|c| c.id != card).unwrap();
        app.world.cards[done].project = 0;
        app.world.cards[done].owner = Some(w[2]);
        app.world.cards[done].column = Column::Done;
        app.world.cards[done].symbols = vec![Symbol {
            file: "src/Auth/Session.cs",
            name: "Session.End",
        }];
        app.on_key(KeyEvent::from(KeyCode::Char('/')));
        type_in(&mut app, "Session.");
        tab(&mut app);
        let m = app.menu.as_ref().expect("the menu");
        assert_eq!(
            m.items,
            ["src/Auth/Session.cs", "src/Auth/Session.cs::Session.Start"]
        );
        let holder = format!("○ {}", app.world.agents[w[1]].name);
        assert_eq!(m.notes, [holder.clone(), holder]);
        assert_eq!(app.input, "src/Auth/Session.cs ");
        tab(&mut app);
        assert_eq!(app.input, "src/Auth/Session.cs::Session.Start ");
    }

    #[test]
    fn a_line_sent_is_kept_for_recall_unless_it_only_went_somewhere() {
        assert!(recalled("@claude-1 fix the clock"));
        assert!(recalled(":budget 80"));
        assert!(recalled("!cargo test"));
        assert!(recalled("hello"));
        assert!(!recalled("#12"));
        assert!(!recalled("&3 claude-1"));
        assert!(!recalled(":goto claude-1"));
        assert!(!recalled(":history fix"));
        assert!(!recalled(":history"));
        assert!(recalled(":gotten"), "the word, not its start");
        let (mut app, w) = four_idle();
        let name = app.world.agents[w[0]].name.clone();
        let send = |app: &mut App, line: &str| {
            type_in(app, line);
            app.on_key(KeyEvent::from(KeyCode::Enter));
        };
        app.world.tick = 4;
        send(&mut app, &format!("@{name} fix the clock"));
        send(&mut app, "#1");
        app.on_key(KeyEvent::from(KeyCode::Esc));
        send(&mut app, ":budget 80");
        send(&mut app, ":budget 80");
        app.command("goto nobody");
        app.flash = None;
        let texts = |app: &App| {
            app.world
                .sent
                .iter()
                .map(|s| s.text.clone())
                .collect::<Vec<_>>()
        };
        assert_eq!(
            texts(&app),
            [format!("@{name} fix the clock"), ":budget 80".into()]
        );
        assert_eq!(app.world.sent[0].tick, 4);
        // The line as typed, trimmed; `!` lines too.
        send(&mut app, "  !cargo test  ");
        assert_eq!(texts(&app).last().unwrap(), "!cargo test");
        assert_eq!(app.world.sent.len(), 3);
    }

    #[test]
    fn up_and_down_bring_the_lines_sent_back_and_the_draft_after_the_newest() {
        let (mut app, _) = four_idle();
        let up = |app: &mut App| app.on_key(KeyEvent::from(KeyCode::Up));
        let down = |app: &mut App| app.on_key(KeyEvent::from(KeyCode::Down));
        // Nothing sent: up and down leave the line as it is.
        app.on_key(KeyEvent::from(KeyCode::Char('/')));
        type_in(&mut app, "draft");
        up(&mut app);
        down(&mut app);
        assert_eq!(app.input, "draft");
        app.on_key(KeyEvent::from(KeyCode::Esc));
        for line in ["first", "second", "third"] {
            app.world.recall_line(line);
        }
        app.on_key(KeyEvent::from(KeyCode::Char('/')));
        type_in(&mut app, "draft");
        up(&mut app);
        assert_eq!((app.input.as_str(), app.after), ("third", 0));
        assert!(app.menu.is_none());
        up(&mut app);
        assert_eq!(app.input, "second");
        up(&mut app);
        assert_eq!(app.input, "first");
        up(&mut app);
        assert_eq!(app.input, "first", "the oldest stays");
        down(&mut app);
        assert_eq!(app.input, "second");
        down(&mut app);
        down(&mut app);
        assert_eq!(app.input, "draft", "past the newest: the draft is back");
        assert!(app.recall.is_none());
        down(&mut app);
        assert_eq!(app.input, "draft");
        // Writing on a line brought back makes it the person's own: up
        // starts from the newest again, and down brings the written one.
        up(&mut app);
        up(&mut app);
        assert_eq!(app.input, "second");
        type_in(&mut app, "!");
        assert!(app.recall.is_none());
        up(&mut app);
        assert_eq!(app.input, "third");
        down(&mut app);
        assert_eq!(app.input, "second!");
        // Sent, it is the newest; the line is empty again.
        app.on_key(KeyEvent::from(KeyCode::Enter));
        assert_eq!(app.world.sent.last().unwrap().text, "second!");
        assert!(!app.typing && app.recall.is_none());
        // Over several lines, up and down move between them first; at the
        // first line up brings a line back, at the last line down does.
        app.world.recall_line("a\nb");
        app.on_key(KeyEvent::from(KeyCode::Char('/')));
        type_in(&mut app, "x\\");
        app.on_key(KeyEvent::from(KeyCode::Enter));
        type_in(&mut app, "y");
        assert_eq!(app.input, "x\ny");
        up(&mut app);
        assert_eq!(app.input, "x\ny", "moved up a line, nothing brought back");
        assert_eq!(app.cursor(), 1);
        up(&mut app);
        assert_eq!(app.input, "a\nb", "at the first line: the newest sent");
        assert_eq!(app.cursor(), 3);
        app.on_key(KeyEvent::from(KeyCode::Up));
        assert_eq!(app.cursor(), 1, "up within the line brought back");
        down(&mut app);
        assert_eq!((app.input.as_str(), app.cursor()), ("a\nb", 3));
        down(&mut app);
        assert_eq!(app.input, "x\ny", "at the last line: the draft");
        // Esc drops it all.
        up(&mut app);
        app.on_key(KeyEvent::from(KeyCode::Esc));
        assert!(!app.typing && app.recall.is_none() && app.input.is_empty());
    }

    #[test]
    fn fork_makes_a_new_worker_from_the_one_at_hand_and_opens_the_line_to_it() {
        let (mut app, w) = four_idle();
        let said = |app: &mut App| app.flash.take().map(|f| f.0).unwrap_or_default();
        let id = w[0];
        // The one Claude worker, numbered past its count: a fork is
        // claude-2, counted, not claude-1, the first name free.
        for (i, &o) in w.iter().enumerate() {
            app.world.agents[o].vendor = Vendor::Codex;
            app.world.agents[o].name = format!("codex-{}", i + 1);
        }
        app.world.agents[id].vendor = Vendor::Claude;
        app.world.agents[id].name = "claude-7".into();
        // Workers are counted, not the rest: none of them is Claude.
        let lead = app.world.projects[0].leader;
        app.world.agents[lead].vendor = Vendor::Codex;
        let name = app.world.agents[id].name.clone();
        let lead = app.world.projects[0].leader;
        app.command(&format!("fork @{}", app.world.agents[lead].name));
        assert_eq!(
            said(&mut app),
            format!(
                "{} does not fork: only a worker forks: it has a conversation and a worktree \
                 of its own",
                app.world.agents[lead].name
            )
        );
        app.selected = Sel::Agent(id);
        assert_eq!(app.command_note("fork"), format!("({name}) $"));
        app.command("fork");
        assert_eq!(
            said(&mut app),
            format!("{name} does not fork: it has no session to fork yet: tell it something first")
        );
        let Engine::Sim(sim) = &mut app.engine else {
            unreachable!()
        };
        sim.told(&mut app.world, id, "make it fast").unwrap();
        app.command("fork");
        assert_eq!(
            said(&mut app),
            format!("{name} does not fork: it is mid-turn: fork it when its turn ends")
        );
        app.world.agents[id].status = Status::Idle;
        app.world.agents[id].vendor = Vendor::Gemini;
        app.command("fork");
        assert_eq!(
            said(&mut app),
            format!(
                "{name} does not fork: gemini sessions can not be forked (Gemini CLI 0.65.0's \
                 ACP has no fork)"
            )
        );
        app.world.agents[id].vendor = Vendor::Claude;
        let before = app.world.agents.len();
        app.command("fork");
        assert_eq!(app.world.agents.len(), before + 1);
        let new = before;
        let fork = app.world.agents[new].name.clone();
        assert_eq!(fork, "claude-2");
        let a = &app.world.agents[new];
        assert_eq!(
            (a.role, a.vendor, a.project, a.status),
            (Role::Worker, Vendor::Claude, 0, Status::Idle)
        );
        assert_eq!(
            said(&mut app),
            format!("{fork} is a fork of {name}: write what it should try")
        );
        assert!(app.typing && app.menu.is_none());
        assert_eq!(app.input, format!("@{fork} "));
        assert_eq!(app.selected, Sel::Agent(new));
        assert!(app.world.journal.iter().any(|e| e.event
            == Event::Forked {
                from: id,
                agent: new
            }));
        assert!(
            app.world.agents[new]
                .trace
                .back()
                .unwrap()
                .contains(&format!("forked from {name}"))
        );
        // What it is told goes on in its own session, as a copy.
        type_in(&mut app, "use a cache");
        app.on_key(key(KeyCode::Enter));
        assert_eq!(app.world.agents[new].status, Status::Working);
        assert!(!app.world.journal.iter().any(|e| e.event
            == Event::Restarted {
                agent: new,
                why: Fresh::NoSession
            }));
    }

    #[test]
    fn capital_h_recalls_every_journal_and_trace_line_and_enter_goes_there() {
        let (mut app, w) = four_idle();
        let said = |app: &mut App| app.flash.take().map(|f| f.0).unwrap_or_default();
        let lead = app.world.projects[0].leader;
        let (worker, other) = (w[0], w[1]);
        app.world.tick = 20;
        app.world.journal.push(Entry {
            tick: 3,
            event: Event::Talk {
                from: worker,
                to: lead,
                text: "Deploy the site".into(),
            },
        });
        let line = app.world.journal.len() - 1;
        app.world.agents[other].trace(9, "run: deploy.sh");
        app.on_key(key(KeyCode::Char('H')));
        assert!(app.typing && app.searching && app.recalling);
        assert!(app.input.is_empty() && app.menu.is_none(), "nothing typed");
        type_in(&mut app, "DEPLO");
        let m = app.menu.clone().expect("two lines have it");
        let event = ui::words::event(Lang::En, &app.world, &app.world.journal[line].event);
        let trace = app.world.agents[other].trace.back().unwrap().clone();
        assert_eq!(m.items, [complete::one_line(&event), trace]);
        let (wn, on) = (
            &app.world.agents[worker].name,
            &app.world.agents[other].name,
        );
        let project = &app.world.projects[0].name;
        assert_eq!(
            m.notes,
            [
                format!("{wn} · {project} · {}", clock(3)),
                format!("{on} · {project}")
            ]
        );
        // Enter: the journal line's conversation, the line selected.
        app.on_key(key(KeyCode::Enter));
        assert!(!app.typing && !app.searching && !app.recalling);
        assert!(app.input.is_empty() && app.menu.is_none());
        let thread = app.thread.as_ref().expect("its conversation");
        assert_eq!(thread.entries[thread.sel], line);
        assert_eq!((app.pane, app.journal_sel), (Pane::Journal, Some(line)));
        // A line `:clear` took off the journal: its conversation, nothing
        // selected in the journal that does not show it.
        app.thread = None;
        app.pane = Pane::Salon;
        app.journal_sel = None;
        app.world.journal.push(Entry {
            tick: 4,
            event: Event::Stopped {
                agent: other,
                why: "later".into(),
            },
        });
        app.world.cleared.journal = line + 1;
        app.command("recall deploy the site");
        app.on_key(key(KeyCode::Enter));
        let thread = app.thread.as_ref().expect("its conversation");
        assert_eq!(thread.entries[thread.sel], line);
        assert_eq!((app.pane, app.journal_sel), (Pane::Salon, None));
        app.world.cleared.journal = 0;
        // Tab to the trace line: its agent's raw stream.
        app.thread = None;
        app.on_key(key(KeyCode::Char('H')));
        type_in(&mut app, "deploy");
        app.on_key(key(KeyCode::Tab));
        app.on_key(key(KeyCode::Enter));
        assert_eq!(app.zoom, Zoom::Trace(other));
        assert_eq!(app.selected, Sel::Agent(other));
        // Of a closed project: said, not gone to.
        app.zoom = Zoom::None;
        app.world.projects[0].closed = true;
        app.command("recall deploy.sh");
        assert!(app.recalling && app.input == "deploy.sh");
        app.on_key(key(KeyCode::Enter));
        assert_eq!(app.zoom, Zoom::None);
        assert_eq!(
            said(&mut app),
            "that line is not on the screen: its project is closed, or it is older than \
             what was loaded"
        );
        app.world.projects[0].closed = false;
        // Nothing has the words: said.
        app.on_key(key(KeyCode::Char('H')));
        type_in(&mut app, "zebra ");
        app.on_key(key(KeyCode::Enter));
        assert_eq!(said(&mut app), "no journal or trace line has zebra");
        assert!(!app.typing && !app.recalling);
        // ^r is the lines sent again; esc leaves.
        app.world.recall_line("deploy it");
        app.on_key(key(KeyCode::Char('H')));
        app.on_key(KeyCode::Esc.into());
        assert!(!app.typing && !app.searching);
        app.on_key(ctrl('r'));
        assert!(app.searching && !app.recalling);
        // `:recall` is no line to bring back.
        assert!(!recalled(":recall deploy") && recalled(":budget 80"));
    }

    #[test]
    fn a_line_recalled_of_an_archive_opens_in_its_view() {
        let mut app = App::new(2, 2, 0, 7, Lang::En);
        let worker = (app.world.agents.iter())
            .find(|a| a.project == 1 && a.role == Role::Worker)
            .map(|a| a.id)
            .unwrap();
        let lead = app.world.projects[1].leader;
        app.world.journal.push(Entry {
            tick: 1,
            event: Event::Talk {
                from: worker,
                to: lead,
                text: "the zebra crossing".into(),
            },
        });
        app.world.agents[worker].trace(1, "zebra stripes");
        app.selected = Sel::Agent(lead);
        app.command("archive epic");
        app.end_grace();
        assert!(app.world.projects[1].archived.is_some());
        app.command("recall zebra crossing");
        app.on_key(key(KeyCode::Enter));
        let v = view(&app);
        let thread = v.thread.as_ref().expect("its conversation, in the view");
        let at = thread.entries[thread.sel];
        assert!(
            matches!(&v.world.journal[at].event, Event::Talk { text, .. } if text == "the zebra crossing")
        );
        assert_eq!(app.thread.as_ref().map(|t| t.sel), None, "not behind it");
        // In the view, recall is the screen behind's.
        app.on_key(key(KeyCode::Esc));
        app.on_key(key(KeyCode::Char('H')));
        assert!(!view(&app).typing);
        assert_eq!(
            view(&app).flash.as_ref().map(|f| f.0.as_str()),
            Some(Lang::En.text().read_only)
        );
        app.viewing = None;
        // A trace line of the archive: its agent's stream, in the view.
        app.command("recall zebra stripes");
        app.on_key(key(KeyCode::Enter));
        assert_eq!(view(&app).zoom, Zoom::Trace(worker));
    }

    #[test]
    fn ctrl_x_ctrl_e_hands_the_line_to_the_editor_and_its_text_comes_back_unsent() {
        let (mut app, _) = four_idle();
        app.on_key(key(KeyCode::Char('i')));
        type_in(&mut app, "@claude-1 a long");
        app.after = 5;
        app.on_key(ctrl('x'));
        assert!(app.editor.is_none(), "^x alone waits");
        app.on_key(ctrl('e'));
        assert_eq!(
            app.editor.as_deref(),
            Some("@claude-1 a long"),
            "the whole line"
        );
        assert!(app.menu.is_none());
        let draft = app.editor.take().unwrap();
        // What the editor left is the line, cleaned, the cursor at its end;
        // nothing is sent and nothing is kept as sent.
        let sent = app.world.sent.len();
        app.edited(&draft, Ok("@claude-1 a long\nmessage\t\u{1b}[2J".into()));
        assert!(app.typing && app.editor.is_none());
        assert_eq!(app.input, "@claude-1 a long\nmessage\t[2J");
        assert_eq!(app.after, 0);
        assert_eq!(app.world.sent.len(), sent);
        // An editor that failed gives the draft back as it was; why was
        // said on the terminal before the screen came back.
        app.input = "changed".into();
        app.after = 2;
        let why = editor::Why::Ended(Ended::Exit(1));
        assert_eq!(
            app.editor_back("vim", &why),
            "[imece] `vim` ended with 1: the draft is kept · any key: back to the screen\n"
        );
        app.edited("the draft", Err(why));
        assert_eq!((app.input.as_str(), app.after), ("the draft", 0));
        assert!(app.typing);
        app.lang = Lang::Tr;
        assert_eq!(
            app.editor_back("vim", &editor::Why::NotText),
            "[imece] `vim` UTF-8 metin olmayan bir şey bıraktı: taslak duruyor · herhangi bir tuş: ekrana dön\n"
        );
    }

    #[test]
    fn ctrl_x_then_another_key_does_what_that_key_does_alone() {
        let (mut app, _) = four_idle();
        app.on_key(key(KeyCode::Char('i')));
        type_in(&mut app, "ab");
        app.on_key(ctrl('x'));
        app.on_key(ctrl('a'));
        assert_eq!(app.cursor(), 0, "^a went to the start");
        // The chord is gone: a later ^e is only the end of the line.
        app.on_key(ctrl('e'));
        assert!(app.editor.is_none());
        assert_eq!(app.cursor(), 2);
        // A plain e after ^x is a letter, and the chord goes with it.
        app.on_key(ctrl('x'));
        app.on_key(key(KeyCode::Char('e')));
        assert_eq!(app.input, "abe");
        app.on_key(ctrl('e'));
        assert!(app.editor.is_none());
        // ^e without ^x before it never opens the editor, and x without
        // ctrl is a letter, no chord.
        app.on_key(ctrl('e'));
        assert!(app.editor.is_none());
        app.on_key(key(KeyCode::Char('x')));
        assert_eq!(app.input, "abex");
        app.on_key(ctrl('e'));
        assert!(app.editor.is_none());
    }

    #[test]
    fn edit_opens_the_editor_on_what_follows_it_and_its_menu_names_the_editor() {
        let (mut app, _) = four_idle();
        app.command("edit");
        assert_eq!(app.editor.as_deref(), Some(""));
        app.editor = None;
        app.command("edit dear leader");
        assert_eq!(app.editor.as_deref(), Some("dear leader"));
        assert!(complete::COMMANDS.contains(&"edit"));
        assert_eq!(app.command_note("edit"), format!("({})", editor::of_env()));
        assert!(!VIEW_COMMANDS.contains(&"edit"));
        // Every way it fails is worded, in both languages.
        let t = |lang, why| ui::words::editor_why(lang, "ed", &why);
        assert_eq!(
            t(Lang::En, editor::Why::Ended(Ended::Exit(127))),
            "no editor `ed`: set $VISUAL or $EDITOR; the draft is kept"
        );
        assert_eq!(
            t(Lang::En, editor::Why::Ended(Ended::Exit(126))),
            "`ed` ended with 126: the draft is kept"
        );
        assert_eq!(
            t(Lang::En, editor::Why::Ended(Ended::Signal(9))),
            "`ed` was stopped by signal 9: the draft is kept"
        );
        assert_eq!(
            t(Lang::En, editor::Why::Long),
            "`ed` left more than 1024 KiB: the draft is kept"
        );
        assert_eq!(
            t(Lang::En, editor::Why::NotText),
            "`ed` left what is not UTF-8 text: the draft is kept"
        );
        let io = io::Error::other("disk full");
        assert_eq!(
            t(Lang::En, editor::Why::Io(io)),
            "could not open the draft in `ed`: disk full; it is kept"
        );
        assert_eq!(
            t(Lang::Tr, editor::Why::Ended(Ended::Exit(2))),
            "`ed` 2 ile bitti: taslak duruyor"
        );
    }

    #[test]
    fn ctrl_r_searches_the_lines_sent_and_enter_takes_one_to_the_line() {
        let (mut app, _) = four_idle();
        let said = |app: &mut App| app.flash.take().map(|f| f.0).unwrap_or_default();
        // Nothing sent yet: said, the line stays closed.
        app.on_key(ctrl('r'));
        assert!(!app.typing && !app.searching);
        assert_eq!(said(&mut app), "nothing sent yet to bring back");
        app.world.tick = 0;
        app.world.recall_line("@claude-1 fix the clock");
        app.world.recall_line(":budget 80");
        app.world.tick = 24;
        app.world.recall_line("@codex-1 İşler\nnasıl?");
        app.world.tick = 36;
        // From the salon: the whole list, newest first, each with its age.
        app.on_key(ctrl('r'));
        assert!(app.typing && app.searching && app.input.is_empty());
        let m = app.menu.clone().expect("the menu");
        assert_eq!(
            m.items,
            [
                "@codex-1 İşler ⏎ nasıl?",
                ":budget 80",
                "@claude-1 fix the clock"
            ]
        );
        assert_eq!(m.notes, ["1 min", "3 min", "3 min"]);
        assert_eq!(m.start, 0);
        // Typing filters; tab and the arrows step; enter takes the line
        // whole, its own lines back, and the search ends.
        type_in(&mut app, "İŞ");
        assert_eq!(app.menu.as_ref().unwrap().items.len(), 1);
        app.on_key(KeyEvent::from(KeyCode::Enter));
        assert_eq!(app.input, "@codex-1 İşler\nnasıl?");
        assert!(app.typing && !app.searching && app.menu.is_none());
        assert_eq!(app.after, 0);
        app.on_key(KeyEvent::from(KeyCode::Esc));
        app.on_key(ctrl('r'));
        app.on_key(KeyEvent::from(KeyCode::Tab));
        assert_eq!(app.menu.as_ref().unwrap().chosen, 1);
        assert_eq!(app.input, "", "tab steps, it does not insert");
        app.on_key(KeyEvent::from(KeyCode::Enter));
        assert_eq!(app.input, ":budget 80");
        // Enter again sends it, as any line.
        app.on_key(KeyEvent::from(KeyCode::Enter));
        assert!(!app.typing);
        assert_eq!(app.world.sent.last().unwrap().text, ":budget 80");
        // While writing, ^r takes the words written as the search.
        app.on_key(KeyEvent::from(KeyCode::Char('/')));
        type_in(&mut app, "fix clock");
        app.on_key(ctrl('r'));
        assert!(app.searching && app.input == "fix clock");
        assert_eq!(
            app.menu.as_ref().unwrap().items,
            ["@claude-1 fix the clock"]
        );
        // Nothing has the words: no menu; enter keeps the words, ends the
        // search, sends nothing.
        type_in(&mut app, " zzz");
        assert!(app.menu.is_none() && app.searching);
        app.on_key(KeyEvent::from(KeyCode::Enter));
        assert!(app.typing && !app.searching);
        assert_eq!(app.input, "fix clock zzz");
        assert_eq!(app.world.sent.len(), 4);
        app.on_key(KeyEvent::from(KeyCode::Esc));
        // Esc closes the menu first, the search goes on with the next key;
        // esc again closes the line.
        app.on_key(ctrl('r'));
        app.on_key(KeyEvent::from(KeyCode::Esc));
        assert!(app.menu.is_none() && app.searching && app.typing);
        type_in(&mut app, "bud");
        assert_eq!(app.menu.as_ref().unwrap().items, [":budget 80"]);
        app.on_key(KeyEvent::from(KeyCode::Esc));
        app.on_key(KeyEvent::from(KeyCode::Esc));
        assert!(!app.typing && !app.searching);
        // Up while searching with nothing matched leaves the search for
        // the newest line.
        app.on_key(ctrl('r'));
        type_in(&mut app, "zzz");
        app.on_key(KeyEvent::from(KeyCode::Up));
        assert!(!app.searching);
        assert_eq!(app.input, ":budget 80");
        app.on_key(KeyEvent::from(KeyCode::Esc));
        // `:history WORDS` is the same by name, the words trimmed.
        app.command("history  fix ");
        assert!(app.typing && app.searching && app.input == "fix");
        assert_eq!(app.menu.as_ref().unwrap().items.len(), 1);
        app.on_key(KeyEvent::from(KeyCode::Esc));
        app.on_key(KeyEvent::from(KeyCode::Esc));
        app.world.sent.clear();
        app.command("history");
        assert_eq!(said(&mut app), "nothing sent yet to bring back");
        assert!(!app.typing);
        assert!(complete::COMMANDS.contains(&"history"));
    }

    #[test]
    fn the_jump_list_keeps_the_salons_project_order_among_equals() {
        let mut app = App::new(2, 1, 0, 1, Lang::En);
        app.world.inbox.clear();
        app.unseen.clear();
        for a in &mut app.world.agents {
            a.status = Status::Idle;
            a.talk = None;
            a.starting = false;
        }
        let order = app.world.salon_order();
        assert_eq!(order.len(), 2);
        // The second project pinned: its agents come first, then its row.
        app.world.projects[1].pinned = true;
        let order = app.world.salon_order();
        assert_eq!(order, [1, 0]);
        app.on_key(KeyEvent::from(KeyCode::Char('&')));
        let projects: Vec<ProjectId> = app
            .jumps
            .iter()
            .filter_map(|j| app.world.agent_by_name(&j.name))
            .map(|id| app.world.agents[id].project)
            .collect();
        assert_eq!(projects.len(), app.world.agents.len());
        let first = projects.iter().filter(|&&p| p == 1).count();
        assert!(
            first > 0 && projects[..first].iter().all(|&p| p == 1),
            "{projects:?}"
        );
        assert!(projects[first..].iter().all(|&p| p == 0), "{projects:?}");
        let names: Vec<&str> = app.jumps.iter().map(|j| j.name.as_str()).collect();
        let n = names.len();
        assert_eq!(names[n - 2], app.world.projects[1].name);
        assert_eq!(names[n - 1], app.world.projects[0].name);
    }

    #[test]
    fn the_jump_line_takes_a_name_a_number_a_project_or_says_what_it_did_not_find() {
        let (mut app, w) = four_idle();
        let t = Lang::En.text();
        let said = |app: &mut App| app.flash.take().map(|f| f.0).unwrap_or_default();
        let name = app.world.agents[w[3]].name.clone();
        // `&3 name` as the menu inserts it: the name is what counts.
        app.on_key(KeyEvent::from(KeyCode::Char('&')));
        let third = app.jumps[2].name.clone();
        type_in(&mut app, &format!("3 {third}"));
        app.on_key(KeyEvent::from(KeyCode::Enter));
        assert_eq!(
            app.selected,
            Sel::Agent(app.world.agent_by_name(&third).unwrap())
        );
        // A name typed in any case.
        app.on_key(KeyEvent::from(KeyCode::Char('&')));
        type_in(&mut app, &name.to_uppercase());
        app.on_key(KeyEvent::from(KeyCode::Enter));
        assert_eq!(app.selected, Sel::Agent(w[3]));
        // Enter on the highlighted item takes it at once.
        app.on_key(KeyEvent::from(KeyCode::Char('&')));
        let second = app.world.agents[w[1]].name.clone();
        type_in(&mut app, &second);
        app.on_key(KeyEvent::from(KeyCode::Down));
        app.on_key(KeyEvent::from(KeyCode::Enter));
        assert_eq!(app.selected, Sel::Agent(w[1]));
        assert!(!app.typing);
        // A project: its header.
        let project = app.world.projects[0].name.clone();
        app.on_key(KeyEvent::from(KeyCode::Char('&')));
        type_in(&mut app, &project);
        app.on_key(KeyEvent::from(KeyCode::Enter));
        assert_eq!(app.selected, Sel::Project(0));
        assert_eq!(app.pane, Pane::Salon);
        // Nothing of the name: said, nothing moves.
        app.on_key(KeyEvent::from(KeyCode::Char('&')));
        type_in(&mut app, "zzz");
        app.on_key(KeyEvent::from(KeyCode::Enter));
        assert_eq!(
            said(&mut app),
            "nothing called \"zzz\" to go to: an agent or a project"
        );
        assert_eq!(app.selected, Sel::Project(0));
        // A gone agent is nobody.
        app.world.agents[w[2]].gone = true;
        app.command(&format!("goto {}", app.world.agents[w[2]].name.clone()));
        assert!(said(&mut app).starts_with("nothing called"));
        // `&#` turns to the cards.
        app.on_key(KeyEvent::from(KeyCode::Char('&')));
        app.on_key(KeyEvent::from(KeyCode::Char('#')));
        assert_eq!(app.input, "#");
        assert!(app.jumps.is_empty());
        assert!(
            app.menu
                .as_ref()
                .is_some_and(|m| m.items[0].starts_with('#'))
        );
        app.on_key(KeyEvent::from(KeyCode::Esc));
        app.on_key(KeyEvent::from(KeyCode::Esc));
        assert!(!app.typing);
        // `#` anywhere else is a letter: a message keeps it.
        type_in(&mut app, "@");
        type_in(&mut app, &format!("{name} see #"));
        assert_eq!(app.input, format!("@{name} see #"));
        app.on_key(KeyEvent::from(KeyCode::Esc));
        // `:goto` with a name goes, with `@` too; bare, it opens the line.
        app.command(&format!("goto @{name}"));
        assert_eq!(app.selected, Sel::Agent(w[3]));
        app.command("goto");
        assert!(app.typing && app.input == "&" && !app.jumps.is_empty());
        assert_eq!(t.typing_jump, " › go to: ");
        assert!(complete::COMMANDS.contains(&"goto"));
    }

    #[test]
    fn colon_solo_places_its_words_opens_a_solo_and_selects_it() {
        let mut app = App::new(1, 1, 0, 1, Lang::En);
        app.home = None;
        let t = Lang::En.text();
        let said = |app: &mut App| app.flash.take().map(|f| f.0).unwrap_or_default();
        // The words in any order: a path, a vendor choice, a name.
        app.command("solo notes codex@high ~/x");
        assert_eq!(app.world.projects.len(), 2);
        let p = &app.world.projects[1];
        let head = p.leader;
        let a = &app.world.agents[head];
        assert_eq!(p.name, "notes");
        assert_eq!(
            (a.name.as_str(), a.role, a.vendor),
            ("solo-notes", Role::Solo, Vendor::Codex)
        );
        assert_eq!(app.model_of(head).as_deref(), Some("@high"));
        assert_eq!(app.selected, Sel::Agent(head));
        assert_eq!(app.collapsed, [false, false]);
        assert_eq!(
            said(&mut app),
            "opened notes at ~/x as a solo session; write to solo-notes (codex @high)"
        );
        assert!(app.world.is_solo(1));
        // Typing goes to the solo now, as to a leader.
        assert_eq!(app.typing_target(), "solo-notes");
        // Bare: the pick screen asks the vendor, the model and the effort;
        // the project at hand's checkout, named after it with a number (the
        // demo names its path after the project).
        app.command("solo");
        assert_eq!(app.world.projects.len(), 2, "nothing yet");
        let pick = app.solo_pick.as_ref().expect("the pick screen");
        assert_eq!((pick.path.as_deref(), pick.name.as_deref()), (None, None));
        assert_eq!(
            pick.vendors,
            [Vendor::Claude, Vendor::Codex, Vendor::Gemini]
        );
        for _ in 0..3 {
            app.on_key(key(KeyCode::Enter));
        }
        assert!(app.solo_pick.is_none());
        assert_eq!(app.world.projects.len(), 3);
        assert_eq!(app.world.projects[2].name, "notes-2");
        assert_eq!(
            app.world.agents[app.world.projects[2].leader].vendor,
            Vendor::Claude
        );
        assert_eq!(app.model_of(app.world.projects[2].leader), None);
        assert_eq!(
            said(&mut app),
            "opened notes-2 at notes as a solo session; write to solo-notes-2 (claude)"
        );
        // A path is told by a `/` alone, a `~` alone or a `.` alone.
        for (n, (word, at)) in [("a/b", "a/b"), ("~", "~"), (".", ".")]
            .into_iter()
            .enumerate()
        {
            app.command(&format!("solo claude {word} p-{n}"));
            let flash = said(&mut app);
            assert!(
                flash.contains(&format!(" at {at} as a solo session")),
                "{word}: {flash}"
            );
        }
        assert_eq!(app.world.projects.len(), 6);
        // With a path and a name but no choice, they wait for the pick.
        app.command("solo ~/y ideas");
        let pick = app.solo_pick.as_ref().expect("the pick screen");
        assert_eq!(
            (pick.path.as_deref(), pick.name.as_deref()),
            (Some("~/y"), Some("ideas"))
        );
        app.on_key(key(KeyCode::Char('q')));
        assert!(
            app.solo_pick.is_none() && app.world.projects.len() == 6,
            "given up"
        );
        // Two names, two paths or two choices: the usage line, nothing opened.
        for bad in ["solo a b", "solo ~/x ./y", "solo claude codex"] {
            app.command(bad);
            assert_eq!(said(&mut app), t.solo_usage, "{bad}");
            assert_eq!(app.world.projects.len(), 6, "{bad}");
        }
        // A name that is taken, a name that can not be one.
        app.command("solo claude notes");
        assert_eq!(said(&mut app), fill(t.project_exists, &["notes"]));
        app.command("solo claude a:b");
        assert_eq!(said(&mut app), fill(t.bad_project_name, &["a:b"]));
        // `:solo` raises the next bill, as `:open` does.
        assert_eq!(app.command_note("solo"), "$");
    }

    #[test]
    fn the_menus_after_solo_effort_and_model_offer_what_the_engine_can_run() {
        let mut app = App::new(1, 1, 0, 1, Lang::En);
        app.home = None;
        let items = |app: &mut App, line: &str| -> Vec<String> {
            app.input = line.to_string();
            app.typing = true;
            app.suggest().map(|m| m.items).unwrap_or_default()
        };
        assert_eq!(items(&mut app, ":solo "), ["claude", "codex", "gemini"]);
        assert_eq!(items(&mut app, ":solo co"), ["codex"]);
        // The menu replaces the word being typed, where it starts.
        app.input = ":solo notes co".into();
        assert_eq!(app.suggest().unwrap().start, 12);
        app.input = ":solo co".into();
        assert_eq!(app.suggest().unwrap().start, 6);
        assert_eq!(
            items(&mut app, ":solo notes claude:opus@x"),
            ["claude:opus@xhigh"]
        );
        assert_eq!(
            items(&mut app, ":solo gemini@"),
            Vec::<String>::new(),
            "no efforts"
        );
        // No solo at hand: the effort and model menus have nothing.
        assert!(items(&mut app, ":effort ").is_empty());
        app.command("solo codex notes");
        assert_eq!(
            items(&mut app, ":effort "),
            ["default", "low", "medium", "high", "xhigh"]
        );
        assert_eq!(items(&mut app, ":effort x"), ["xhigh"]);
        assert_eq!(
            items(&mut app, ":model "),
            ["default"],
            "Codex names models in full"
        );
        app.command("solo claude ideas");
        assert_eq!(
            items(&mut app, ":model "),
            ["default", "fable", "opus", "sonnet", "haiku"]
        );
        assert_eq!(items(&mut app, ":model s"), ["sonnet"]);
    }

    #[test]
    fn the_solo_pick_screen_walks_vendor_model_effort_and_composes_the_choice() {
        let vendors = vec![Vendor::Claude, Vendor::Codex, Vendor::Gemini];
        let mut p = SoloPick::new(Some("~/x".into()), None, vendors.clone());
        assert_eq!((p.step, p.sel()), (PickStep::Vendor, 0));
        assert_eq!(p.items(), ["claude", "codex", "gemini"]);
        // j k g G and digits move within the step; nothing past its ends.
        assert_eq!(p.on_key(KeyCode::Char('j')), None);
        assert_eq!(p.sel(), 1);
        p.on_key(KeyCode::Char('G'));
        assert_eq!(p.sel(), 2);
        p.on_key(KeyCode::Char('j'));
        assert_eq!(p.sel(), 2, "the last");
        p.on_key(KeyCode::Char('k'));
        assert_eq!(p.sel(), 1, "k goes up one");
        p.on_key(KeyCode::Up);
        assert_eq!(p.sel(), 0);
        p.on_key(KeyCode::Char('G'));
        p.on_key(KeyCode::Char('g'));
        assert_eq!(p.sel(), 0, "g: the first");
        p.on_key(KeyCode::End);
        assert_eq!(p.sel(), 2);
        p.on_key(KeyCode::Home);
        assert_eq!(p.sel(), 0, "home: the first");
        p.on_key(KeyCode::Char('k'));
        assert_eq!(p.sel(), 0, "not past the first");
        p.on_key(KeyCode::Char('9'));
        assert_eq!(p.sel(), 0, "no ninth");
        p.on_key(KeyCode::Char('1'));
        assert_eq!(p.chosen_vendor(), Vendor::Claude);
        // Enter: the model step, default then Claude's aliases.
        assert_eq!(p.on_key(KeyCode::Enter), None);
        assert_eq!(p.step, PickStep::Model);
        assert_eq!(p.items(), ["default", "fable", "opus", "sonnet", "haiku"]);
        assert_eq!(p.choice(), None, "default: the vendor's own");
        p.on_key(KeyCode::Char('3'));
        assert_eq!(p.choice().as_deref(), Some("opus"));
        // Then the effort step; the choice carries both.
        assert_eq!(p.on_key(KeyCode::Enter), None);
        assert_eq!(p.step, PickStep::Effort);
        assert_eq!(
            p.items(),
            ["default", "low", "medium", "high", "xhigh", "max"]
        );
        p.on_key(KeyCode::Char('5'));
        assert_eq!(p.choice().as_deref(), Some("opus@xhigh"));
        // Esc goes back a step and keeps what was picked there.
        assert_eq!(p.on_key(KeyCode::Esc), None);
        assert_eq!((p.step, p.sel()), (PickStep::Model, 2));
        assert_eq!(p.on_key(KeyCode::Enter), None);
        assert_eq!(
            p.on_key(KeyCode::Enter),
            Some(Some((Vendor::Claude, Some("opus@xhigh".into()))))
        );
        // Another vendor picked: its own lists, from their defaults.
        let mut p = SoloPick::new(None, None, vendors.clone());
        p.on_key(KeyCode::Char('3'));
        assert_eq!(p.on_key(KeyCode::Enter), None);
        assert_eq!(p.items(), ["default", "auto", "pro", "flash", "flash-lite"]);
        p.on_key(KeyCode::Char('3'));
        // Gemini has no efforts: the model step is the last.
        assert_eq!(
            p.on_key(KeyCode::Enter),
            Some(Some((Vendor::Gemini, Some("pro".into()))))
        );
        let mut p = SoloPick::new(None, None, vendors.clone());
        p.on_key(KeyCode::Char('2'));
        p.on_key(KeyCode::Enter);
        assert_eq!(p.items(), ["default"], "Codex has no aliases");
        p.on_key(KeyCode::Enter);
        assert_eq!(p.items(), ["default", "low", "medium", "high", "xhigh"]);
        p.on_key(KeyCode::Char('2'));
        assert_eq!(
            p.on_key(KeyCode::Enter),
            Some(Some((Vendor::Codex, Some("@low".into()))))
        );
        // Given up: esc at the first step, q anywhere.
        let mut p = SoloPick::new(None, None, vendors.clone());
        assert_eq!(p.on_key(KeyCode::Esc), Some(None));
        let mut p = SoloPick::new(None, None, vendors);
        p.on_key(KeyCode::Enter);
        assert_eq!(p.on_key(KeyCode::Char('q')), Some(None));
        assert_eq!(
            p.on_key(KeyCode::Char('x')),
            None,
            "an unknown key does nothing"
        );
    }

    #[test]
    fn colon_effort_and_colon_model_retune_the_solo_at_hand() {
        let mut app = App::new(1, 1, 0, 1, Lang::En);
        app.home = None;
        let t = Lang::En.text();
        let said = |app: &mut App| app.flash.take().map(|f| f.0).unwrap_or_default();
        // No solo at hand: said so; nothing to do without a value.
        app.command("effort high");
        assert_eq!(said(&mut app), t.no_solo);
        app.command("solo claude:opus@high notes");
        said(&mut app);
        let head = app.world.projects[1].leader;
        assert_eq!(app.selected, Sel::Agent(head));
        app.command("effort");
        assert_eq!(said(&mut app), t.effort_usage);
        app.command("model");
        assert_eq!(said(&mut app), t.model_usage);
        // One part changes, the other stays.
        app.command("effort max");
        assert_eq!(
            said(&mut app),
            fill(t.chose, &["solo-notes", "claude opus@max"])
        );
        assert_eq!(app.model_of(head).as_deref(), Some("opus@max"));
        app.command("model sonnet");
        assert_eq!(app.model_of(head).as_deref(), Some("sonnet@max"));
        app.command("effort default");
        assert_eq!(app.model_of(head).as_deref(), Some("sonnet"));
        app.command("model default");
        assert_eq!(app.model_of(head), None);
        assert_eq!(said(&mut app), fill(t.chose, &["solo-notes", "claude"]));
        assert!(app.world.journal.iter().any(|e| matches!(
            &e.event, Event::Chose { agent, choice } if *agent == head && choice == "claude"
        )));
        // Refused: an effort the vendor lacks, a word that is no model name,
        // a solo mid-turn.
        app.command("effort turbo");
        assert!(
            said(&mut app)
                .starts_with("could not change solo-notes: claude has no effort \"turbo\""),
        );
        app.command("model -x");
        assert!(said(&mut app).contains("is not a model name"));
        app.command("effort a@b");
        assert!(said(&mut app).contains("is not an effort level"));
        app.tell(head, "work");
        app.command("effort low");
        assert_eq!(
            said(&mut app),
            fill(
                t.chose_refused,
                &["solo-notes", "solo-notes is mid-turn; wait for its answer"]
            )
        );
        // The project at hand's solo counts when a line of its is at hand;
        // a team's agent never does.
        app.selected = Sel::Project(1);
        assert_eq!(app.solo_at_hand(), Some(head));
        app.selected = Sel::Project(0);
        assert_eq!(app.solo_at_hand(), None);
        app.command("model opus");
        assert_eq!(said(&mut app), t.no_solo);
        // The menu notes the part as it stands, each its own.
        app.selected = Sel::Agent(head);
        assert_eq!(app.command_note("effort"), "(default)");
        assert_eq!(app.command_note("model"), "(default)");
        app.command("solo codex:gpt-5.6-terra@xhigh other");
        assert_eq!(app.command_note("effort"), "(xhigh)");
        assert_eq!(app.command_note("model"), "(gpt-5.6-terra)");
        app.selected = Sel::Project(0);
        assert_eq!(app.command_note("effort"), "", "no solo at hand");
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
        assert!(!app.world.projects[1].closed, "not before its grace");
        app.end_grace();
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
        assert_eq!(app.grace.map(|g| g.0), Some(Undoable::Forget(0)));
        app.end_grace();
        assert_eq!(said(&mut app), fill(t.forgot, &[&lead]));
        assert_eq!(app.world.agents[0].status, Status::Asleep);
        app.selected = Sel::Agent(0);
        app.command("forget");
        assert_eq!(said(&mut app), none, "forgotten already");
    }

    /// A demo app with projects `shop-api`, `shop` and `more` open.
    fn three_projects() -> App {
        let mut app = App::new(1, 1, 0, 1, Lang::En);
        app.home = None;
        app.command("open ~/work/shop");
        app.command("open ~/work/more");
        assert_eq!(app.world.open_projects().count(), 3);
        app.flash = None;
        app
    }

    #[test]
    fn a_close_waits_out_its_grace_and_u_keeps_it_open() {
        let mut app = three_projects();
        let said = |app: &mut App| app.flash.take().map(|f| f.0).unwrap_or_default();
        app.flash("before");
        app.command("close shop");
        assert_eq!(app.flash, None, "the count-down shows instead");
        let (what, at) = app.grace.unwrap();
        assert_eq!(what, Undoable::Close(1));
        assert!(!app.world.projects[1].closed);
        let ms = Duration::from_millis;
        assert_eq!(app.grace_left(at - UNDO_GRACE), Some((what, 10)));
        assert_eq!(app.grace_left(at - UNDO_GRACE + ms(1)), Some((what, 10)));
        assert_eq!(app.grace_left(at - ms(1001)), Some((what, 2)));
        assert_eq!(app.grace_left(at - ms(1)), Some((what, 1)));
        assert_eq!(app.grace_left(at), Some((what, 0)));
        app.grace_due(at - ms(1));
        assert!(!app.world.projects[1].closed, "not yet");
        app.grace_due(at);
        assert!(app.world.projects[1].closed, "its time");
        assert_eq!(app.grace, None);
        assert_eq!(
            said(&mut app),
            "closed shop: its agents left; their worktrees stay on disk"
        );
        assert_eq!(app.grace_left(at), None);
        // `u` keeps it open; again, nothing to undo.
        app.command("close more");
        app.on_key(key(KeyCode::Char('u')));
        assert_eq!(app.grace, None);
        assert_eq!(said(&mut app), "more stays open");
        app.grace_due(at + UNDO_GRACE * 2);
        assert!(!app.world.projects[2].closed);
        app.command("undo");
        assert_eq!(said(&mut app), Lang::En.text().nothing_to_undo);
        app.on_key(key(KeyCode::Char('u')));
        assert_eq!(said(&mut app), Lang::En.text().nothing_to_undo);
        // Quitting: what was asked happens.
        app.command("close more");
        app.end_grace();
        assert!(app.world.projects[2].closed);
        assert_eq!(app.grace, None);
        app.end_grace();
        assert_eq!(app.grace, None, "nothing waits, nothing happens");
    }

    #[test]
    fn capital_u_and_colon_now_do_what_counts_down_at_once() {
        let mut app = three_projects();
        let said = |app: &mut App| app.flash.take().map(|f| f.0).unwrap_or_default();
        app.command("close shop");
        app.on_key(key(KeyCode::Char('U')));
        assert!(app.world.projects[1].closed, "no wait");
        assert_eq!(app.grace, None);
        assert_eq!(
            said(&mut app),
            "closed shop: its agents left; their worktrees stay on disk"
        );
        app.command("close more");
        assert_eq!(app.grace.map(|g| g.0), Some(Undoable::Close(2)));
        app.command("now");
        assert_eq!(app.grace, None);
        assert!(app.world.projects[2].closed);
        // Nothing counts down: said, nothing happens.
        app.flash = None;
        app.on_key(key(KeyCode::Char('U')));
        assert_eq!(said(&mut app), Lang::En.text().nothing_now);
        app.command("now");
        assert_eq!(said(&mut app), Lang::En.text().nothing_now);
        assert!(!app.world.projects[0].closed);
    }

    #[test]
    fn one_close_or_forget_waits_at_a_time() {
        let mut app = three_projects();
        app.command("close shop");
        app.command("close more");
        assert!(app.world.projects[1].closed, "the one before acts");
        assert_eq!(app.grace.map(|g| g.0), Some(Undoable::Close(2)));
        // The last open project is refused at once, after the one waiting.
        app.command("close shop-api");
        assert!(app.world.projects[2].closed);
        assert_eq!(app.grace, None);
        assert_eq!(
            app.flash.take().map(|f| f.0).as_deref(),
            Some("shop-api is the last open project; q q quits")
        );
        // A :forget after a :close: the close acts.
        let mut app = three_projects();
        app.tell(0, "hello");
        app.command("close more");
        let lead = app.world.agents[0].name.clone();
        app.command(&format!("forget @{lead}"));
        assert!(app.world.projects[2].closed);
        assert_eq!(app.grace.map(|g| g.0), Some(Undoable::Forget(0)));
        app.on_key(key(KeyCode::Char('u')));
        assert_eq!(
            app.flash.take().map(|f| f.0),
            Some(format!("{lead} keeps its session"))
        );
        assert_ne!(app.world.agents[0].status, Status::Asleep, "kept");
        // A refused :forget waits for nothing.
        app.command("forget @nobody");
        assert_eq!(app.grace, None);
        app.command(&format!("forget @{}", app.world.agents[1].name.clone()));
        assert_eq!(app.grace, None, "no session yet");
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
    fn a_commit_line_quotes_the_message_for_the_shell() {
        assert_eq!(sh_quote("it's"), r"'it'\''s'");
        assert_eq!(sh_quote("$(rm -rf ~) `x`"), "'$(rm -rf ~) `x`'");
        assert_eq!(
            commit_line("Fix it\n\nWhy", None, false),
            "!git commit -m 'Fix it\n\nWhy'"
        );
        assert_eq!(
            commit_line("A", Some(Path::new("/w/it's")), true),
            r"!git -C '/w/it'\''s' commit --dry-run -m 'A'"
        );
    }

    #[test]
    fn a_suggestion_opens_the_command_line_once_it_is_free() {
        let mut app = App::new(1, 1, 0, 1, Lang::En);
        let suggestion = Suggestion {
            project: 0,
            text: "Fix it".into(),
        };
        app.world.suggested = Some(suggestion.clone());
        app.typing = true;
        app.input = "@lead hi".into();
        app.show_suggestion();
        assert_eq!(app.input, "@lead hi", "not over what the person writes");
        assert_eq!(app.world.suggested, Some(suggestion));
        app.typing = false;
        app.input.clear();
        app.after = 3;
        app.show_suggestion();
        // The demo's message is made up: it only tries.
        assert_eq!(app.input, "!git commit --dry-run -m 'Fix it'");
        assert!(app.typing);
        assert_eq!(app.after, 0, "the cursor at the end");
        assert!(app.menu.is_none());
        assert_eq!(app.world.suggested, None);
        // Nothing waits: nothing opens.
        app.typing = false;
        app.input.clear();
        app.show_suggestion();
        assert!(!app.typing);
        assert!(app.input.is_empty());
    }

    #[test]
    fn a_live_suggestion_commits_in_its_own_projects_checkout() {
        let one = crate::live::testing::checkout("sugg-one");
        let two = crate::live::testing::checkout("sugg-two");
        let w = crate::live::testing::team(0);
        let live = crate::live::testing::idle_live_in(&one, 1);
        let mut app = App::with(w, Engine::Live(Box::new(live)), Lang::En);
        app.command(&format!("open {} two", two.display()));
        app.selected = Sel::Agent(0);
        app.typing = false;
        app.world.suggested = Some(Suggestion {
            project: 0,
            text: "Here".into(),
        });
        app.show_suggestion();
        assert_eq!(app.input, "!git commit -m 'Here'");
        app.typing = false;
        app.world.suggested = Some(Suggestion {
            project: 1,
            text: "There".into(),
        });
        app.show_suggestion();
        let top = imece_agent::worktree::top(&two).unwrap();
        assert_eq!(
            app.input,
            format!(
                "!git -C {} commit -m 'There'",
                sh_quote(&top.to_string_lossy())
            )
        );
        let _ = std::fs::remove_dir_all(&one);
        let _ = std::fs::remove_dir_all(&two);
    }

    #[test]
    fn colon_copy_lists_adds_and_drops_paths_of_the_project_at_hand() {
        let mut app = App::new(1, 1, 0, 1, Lang::En);
        let flash = |app: &App| app.flash.as_ref().map(|(m, _)| m.clone()).unwrap();
        let p = app.current_project();
        let name = app.world.projects[p].name.clone();
        assert_eq!(app.command_note("copy"), "");
        app.command("copy");
        assert_eq!(
            flash(&app),
            format!(
                "nothing is copied into {name}'s worktrees; :copy PATH adds a path git ignores"
            )
        );
        app.command("copy .env");
        assert_eq!(
            flash(&app),
            format!(".env is copied into {name}'s worktrees that lack it, at a worker's start")
        );
        app.command("copy target/");
        assert_eq!(app.world.projects[p].copies, [".env", "target"]);
        assert_eq!(app.command_note("copy"), "(.env, target)");
        app.command("copy");
        assert_eq!(
            flash(&app),
            format!("copied into {name}'s worktrees: .env, target")
        );
        app.command("copy ../secret");
        assert!(flash(&app).starts_with("../secret can not be copied: a relative path"));
        app.command("copy .env");
        assert_eq!(flash(&app), ".env is no longer copied");
        assert_eq!(app.world.projects[p].copies, ["target"]);
    }

    #[test]
    fn colon_copy_says_so_when_the_checkout_has_a_worktreeinclude() {
        // The second demo project's checkout acts as if it had one.
        let mut app = App::new(2, 2, 0, 1, Lang::En);
        let flash = |app: &App| app.flash.as_ref().map(|(m, _)| m.clone()).unwrap();
        app.selected = Sel::Agent(app.world.projects[1].leader);
        assert_eq!(app.current_project(), 1);
        let name = app.world.projects[1].name.clone();
        app.command("copy");
        assert_eq!(
            flash(&app),
            format!(
                "{name}'s .worktreeinclude names what is copied into its worktrees; :copy PATH adds a path git ignores"
            )
        );
        app.command("copy .env");
        app.command("copy");
        assert_eq!(
            flash(&app),
            format!("copied into {name}'s worktrees: .env; and what its .worktreeinclude names")
        );
        app.lang = Lang::Tr;
        app.command("copy");
        assert_eq!(
            flash(&app),
            format!(
                "{name} çalışma ağaçlarına kopyalananlar: .env; bir de .worktreeinclude'un saydıkları"
            )
        );
    }

    #[test]
    fn a_on_an_ask_says_yes_from_now_on_and_colon_allow_lists_adds_and_drops() {
        let mut app = App::new(1, 2, 0, 1, Lang::En);
        let flash = |app: &App| app.flash.as_ref().map(|(m, _)| m.clone()).unwrap();
        let p = app.current_project();
        let name = app.world.projects[p].name.clone();
        let (one, two) = (
            app.world.projects[p].leader + 1,
            app.world.projects[p].leader + 2,
        );
        assert_eq!(app.command_note("allow"), "");
        app.command("allow");
        assert_eq!(
            flash(&app),
            format!("{name}: no yes from now on; A on an ask, or :allow PROGRAM [WORD]")
        );
        // Two asks decided before: the open ones are not the first items.
        for _ in 0..2 {
            app.world.asks_you(one, "Bash: npm install old");
            let i = app.world.inbox.len() - 1;
            app.world.decided(i, false);
        }
        app.world.asks_you(one, "Bash: npm install zod@3");
        app.world.asks_you(two, "Bash: npm install left-pad");
        app.world.asks_you(two, "Edit: package.json");
        app.pane = Pane::Inbox;
        app.inbox_sel = 0;
        app.on_key(key(KeyCode::Char('A')));
        assert_eq!(
            flash(&app),
            format!("{name}: yes from now on to npm install …")
        );
        assert!(app.archives.is_none(), "an ask at hand: not the archives");
        let open: Vec<String> = (app.world.open_inbox().iter())
            .map(|&i| app.world.inbox[i].detail.clone())
            .collect();
        assert_eq!(
            open,
            ["Edit: package.json"],
            "the other npm install went on too"
        );
        assert_eq!(app.command_note("allow"), "(npm install …)");
        // An edit can have none: said, and nothing decided.
        app.inbox_sel = 0;
        app.on_key(key(KeyCode::Char('A')));
        assert_eq!(
            flash(&app),
            "a yes from now on is for commands only: a allows this once"
        );
        assert_eq!(app.world.open_inbox().len(), 1);
        // An ask whose commands have a yes already (added without letting
        // it go on): allowed, nothing new.
        app.world.asks_you(one, "Bash: make");
        app.world.stand(
            p,
            &[Standing {
                program: "make".into(),
                sub: None,
            }],
        );
        let make = app
            .inbox_rows()
            .iter()
            .position(|&i| app.world.inbox[i].detail == "Bash: make");
        app.inbox_sel = make.unwrap();
        app.on_key(key(KeyCode::Char('A')));
        assert_eq!(
            flash(&app),
            format!("{name}: allowed; its commands had a yes from now on already")
        );
        app.world.unstand(
            p,
            &Standing {
                program: "make".into(),
                sub: None,
            },
        );
        // :allow adds by name, again takes it out; a bad name is said.
        app.command("allow make");
        assert_eq!(flash(&app), format!("{name}: yes from now on to make"));
        app.command("allow");
        assert_eq!(
            flash(&app),
            format!("{name}: yes from now on to make, npm install …")
        );
        app.command("allow npm install");
        assert_eq!(
            flash(&app),
            format!("{name}: no yes from now on to npm install … any more")
        );
        app.command("allow rm -rf");
        assert_eq!(
            flash(&app),
            "`rm -rf` starts with an option: a yes from now on needs what it runs first; a allows this once"
        );
        app.command("allow a && b");
        assert!(flash(&app).starts_with("this line runs more than plain commands"));
        assert_eq!(app.world.projects[p].allowed.len(), 1);
        // A in waiting for you on something that is not an ask, or
        // elsewhere: the archives, as before.
        app.pane = Pane::Salon;
        app.on_key(key(KeyCode::Char('A')));
        assert_eq!(
            flash(&app),
            app.lang.text().no_archives,
            "the archives, as before"
        );
    }

    #[test]
    fn colon_copy_live_asks_git_whether_it_ignores_the_path() {
        let repo = crate::live::testing::checkout("app-copy");
        std::fs::write(repo.join(".gitignore"), ".env\n").unwrap();
        std::fs::write(repo.join(".env"), "KEY=1").unwrap();
        let w = crate::live::testing::team(0);
        let live = crate::live::testing::idle_live_in(&repo, 1);
        let mut app = App::with(w, Engine::Live(Box::new(live)), Lang::En);
        app.command("copy a.txt");
        let said = app.flash.as_ref().map(|(m, _)| m.clone()).unwrap();
        assert!(said.contains("git does not ignore it"), "{said}");
        app.command("copy .env");
        assert_eq!(app.world.projects[0].copies, [".env"]);
        let _ = std::fs::remove_dir_all(&repo);
    }

    #[test]
    fn colon_commit_asks_and_says_so_or_why_not() {
        let mut app = App::new(1, 1, 0, 1, Lang::En);
        let flash = |app: &App| app.flash.as_ref().map(|(m, _)| m.clone());
        let name = app.world.projects[app.current_project()].name.clone();
        app.command("commit");
        assert_eq!(
            flash(&app),
            Some(format!(
                "asking haiku for a commit message for what is staged in {name}"
            ))
        );
        app.command("commit");
        assert_eq!(
            flash(&app),
            Some("no commit message: a suggestion is under way".into())
        );
        assert_eq!(app.command_note("commit"), "$");
        // Live without Claude Code: refused, said why.
        let w = crate::live::testing::team(0);
        let live = crate::live::testing::idle_live(1);
        let mut app = App::with(w, Engine::Live(Box::new(live)), Lang::En);
        app.world.budget = 0;
        app.command("commit");
        assert_eq!(
            flash(&app),
            Some("no commit message: the pot has less than $0.05 left".into())
        );
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
        live.open_project(&mut w, &two, Some("two"), Open::Team)
            .unwrap();
        live.set_home(1);
        let app = App::with(w, Engine::Live(Box::new(live)), Lang::En);
        assert_eq!(app.selected, Sel::Agent(1), "the second project's leader");
        let _ = std::fs::remove_dir_all(&one);
        let _ = std::fs::remove_dir_all(&two);
    }

    #[test]
    fn a_live_archive_is_viewed_from_the_store_and_a_store_that_fails_says_so() {
        let mut w = crate::live::testing::team(1);
        w.note(Event::Failed { agent: 1 });
        w.projects[0].archived = Some(Archived {
            at: 3,
            title: "t".into(),
        });
        let mut live = crate::live::testing::idle_live(2);
        live.store_in_memory();
        live.persist(&w).unwrap();
        let mut app = App::with(w, Engine::Live(Box::new(live)), Lang::En);
        // A line the store does not have is not the archive's.
        app.world.note(Event::Failed { agent: 0 });
        app.view_archive(0, None);
        let (lines, _) = app.world.journal_of(0);
        assert_eq!(view(&app).world.journal[..], lines[..lines.len() - 1]);
        app.viewing = None;
        let Engine::Live(l) = &mut app.engine else {
            unreachable!()
        };
        l.break_store();
        app.view_archive(0, None);
        assert!(app.viewing.is_none());
        assert!(
            said(&app).starts_with("the archive's journal could not be read: no such table"),
            "{}",
            said(&app)
        );
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
        // says more than done; stuck says more than an item.
        app.world.agents[4].status = Status::Working;
        app.world.inbox.push(item(0, true));
        app.world.inbox.push(item(3, true));
        assert_eq!(app.needs()[4], Need::Nothing);
        assert_eq!(app.needs()[0], Need::Waiting);
        assert_eq!(app.needs()[3], Need::Stuck, "stopped, its item waiting");
        app.world.agents[2].gone = true;
        assert_eq!(app.needs()[2], Need::Nothing, "gone and blocked");
        assert!(Need::Stuck > Need::Waiting && Need::Waiting > Need::Done);
        assert!(Need::Done > Need::Nothing);
    }

    #[test]
    fn every_agent_stands_on_one_rung_of_the_ladder() {
        let mut app = quiet();
        assert_eq!(app.rungs(), [Rung::Idle; 5]);
        let a = &mut app.world.agents;
        a[0].status = Status::Stopped;
        a[1].status = Status::WaitingHuman;
        a[2].status = Status::Working;
        a[3].status = Status::Working;
        a[3].starting = true;
        a[4].status = Status::Asleep;
        assert_eq!(
            app.rungs(),
            [
                Rung::Stuck,
                Rung::Waiting,
                Rung::Working,
                Rung::Starting,
                Rung::Idle
            ]
        );
        // Blocked is stuck too; asking and researching are work; an item
        // waiting outranks work and starting; done, not seen is idle with
        // a turn unseen; gone is nothing at all.
        let a = &mut app.world.agents;
        a[0].status = Status::Blocked;
        a[1].status = Status::Asking { to: 0, until: 9 };
        a[2].status = Status::Researching {
            for_agent: 0,
            until: 9,
        };
        a[4].status = Status::Idle;
        app.unseen.insert(4);
        app.world.inbox.push(item(2, true));
        app.world.inbox.push(item(3, true));
        assert_eq!(
            app.rungs(),
            [
                Rung::Stuck,
                Rung::Working,
                Rung::Waiting,
                Rung::Waiting,
                Rung::Done
            ]
        );
        app.world.inbox.clear();
        assert_eq!(app.rungs()[2], Rung::Working, "researching");
        assert_eq!(app.rungs()[3], Rung::Starting);
        app.world.agents[3].gone = true;
        assert_eq!(app.rungs()[3], Rung::Idle, "gone");
        app.world.agents[4].status = Status::Working;
        assert_eq!(app.rungs()[4], Rung::Working, "at work again: no news");
        // The ladder, lowest first, and what each rung needs.
        let ladder = [
            Rung::Idle,
            Rung::Done,
            Rung::Starting,
            Rung::Working,
            Rung::Waiting,
            Rung::Stuck,
        ];
        assert!(ladder.windows(2).all(|w| w[0] < w[1]));
        assert_eq!(
            ladder.map(Rung::need),
            [
                Need::Nothing,
                Need::Done,
                Need::Nothing,
                Need::Nothing,
                Need::Waiting,
                Need::Stuck
            ]
        );
    }

    #[test]
    fn the_loudest_of_a_group_is_its_highest_rung_the_first_among_equals() {
        let mut app = quiet();
        assert_eq!(app.loudest([]), None);
        assert_eq!(app.loudest([3, 1, 2]), Some(3), "all idle: the first");
        app.world.agents[2].status = Status::Working;
        assert_eq!(app.loudest([3, 1, 2]), Some(2));
        app.world.agents[1].status = Status::Working;
        assert_eq!(app.loudest([3, 1, 2]), Some(1), "equals: the first");
        app.world.agents[3].status = Status::Working;
        app.world.agents[3].starting = true;
        assert_eq!(app.loudest([3, 1, 2]), Some(1), "working over starting");
        app.unseen.insert(4);
        assert_eq!(app.loudest([4, 2]), Some(2), "working over done");
        app.world.inbox.push(item(4, true));
        assert_eq!(app.loudest([1, 4, 2]), Some(4), "waiting over working");
        app.world.agents[2].status = Status::Stopped;
        assert_eq!(app.loudest([1, 4, 2]), Some(2), "stuck over waiting");
    }

    #[test]
    fn the_tab_and_the_title_say_the_top_of_the_open_projects_ladder() {
        let mut app = quiet();
        assert_eq!(app.progress(), Progress::Calm);
        app.unseen.insert(4);
        assert_eq!(app.progress(), Progress::Calm, "done, not seen, is calm");
        app.world.agents[3].starting = true;
        assert_eq!(app.progress(), Progress::Working, "starting");
        app.world.agents[3].starting = false;
        app.world.agents[2].status = Status::Working;
        assert_eq!(app.progress(), Progress::Working);
        app.world.agents[1].status = Status::WaitingHuman;
        assert_eq!(app.progress(), Progress::Waiting);
        app.world.agents[0].status = Status::Blocked;
        assert_eq!(app.progress(), Progress::Stuck, "stuck over all");
        app.world.agents[0].status = Status::Stopped;
        assert_eq!(app.progress(), Progress::Stuck);
        app.world.agents[0].gone = true;
        assert_eq!(app.progress(), Progress::Waiting, "gone is not stuck");
        // An item waiting is waiting, whoever it is of, as the title counts.
        let mut app = quiet();
        app.world.agents[2].status = Status::Working;
        app.world.inbox.push(item(4, true));
        app.world.agents[4].gone = true;
        assert_eq!(app.progress(), Progress::Waiting);
        app.world.agents[0].status = Status::Stopped;
        assert_eq!(app.progress(), Progress::Stuck, "stuck over waiting");
        // A closed project's agents are not on the screen.
        let (mut app, [a, _, other]) = two_quiet_projects();
        app.world.agents[other].status = Status::Blocked;
        assert_eq!(app.progress(), Progress::Stuck);
        let p = app.world.agents[other].project;
        app.world.projects[p].closed = true;
        assert_eq!(app.progress(), Progress::Calm, "closed");
        app.world.agents[a].status = Status::Working;
        assert_eq!(app.progress(), Progress::Working);
        // The title starts with its glyph.
        let title = |w, p| ui::words::title(Lang::En, w, p);
        assert_eq!(title(0, Progress::Calm), "⁙ imece");
        assert_eq!(title(0, Progress::Working), "● imece");
        assert_eq!(title(2, Progress::Waiting), "! imece · 2 waiting");
        assert_eq!(title(1, Progress::Stuck), "✕ imece · 1 waiting");
        assert_eq!(
            ui::words::title(Lang::Tr, 3, Progress::Waiting),
            "! imece · 3 seni bekliyor"
        );
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

    /// What `live::look` would answer, without running anything: Claude
    /// Code signed in, Codex not, Gemini CLI not there; only the vendors
    /// asked answer.
    fn fake_look(which: &[Vendor]) -> welcome::Answers {
        let (tx, rx) = std::sync::mpsc::channel();
        for &v in which {
            let seen = match v {
                Vendor::Claude => here("2.1.3", Login::Yes),
                Vendor::Codex => here("0.9", Login::No),
                _ => Seen::Missing,
            };
            tx.send((v, seen)).unwrap();
        }
        rx
    }

    fn here(version: &str, login: Login) -> Seen {
        Seen::Here {
            version: version.into(),
            login,
        }
    }

    /// A live app whose leader is of `lead`, asking the fake vendors.
    fn live_welcome(lead: Vendor) -> App {
        let mut w = crate::live::testing::team(1);
        let leader = w.projects[0].leader;
        w.agents[leader].vendor = lead;
        let live = crate::live::testing::idle_live(2);
        let mut app = App::with(w, Engine::Live(Box::new(live)), Lang::En);
        app.look = Some(fake_look);
        app
    }

    #[test]
    fn the_first_start_ever_opens_the_welcome_and_asks_every_vendor() {
        let mut app = live_welcome(Vendor::Claude);
        assert!(!app.welcome);
        assert_eq!(app.vendors.len(), 3);
        assert!(app.vendors.iter().all(|(_, s)| *s == Seen::Checking));
        app.start_welcome(true);
        assert!(app.welcome);
        assert_eq!(app.lead_check, None, "the window is open already");
        app.looked();
        assert_eq!(
            app.vendors,
            [
                (Vendor::Claude, here("2.1.3", Login::Yes)),
                (Vendor::Codex, here("0.9", Login::No)),
                (Vendor::Gemini, Seen::Missing),
            ]
        );
        assert!(app.looking.is_none(), "every answer came");
        // Any key closes it and does nothing else.
        app.on_key(key(KeyCode::Char('?')));
        assert!(!app.welcome && !app.help);
        // `:welcome` opens it again and asks again.
        app.vendors[0].1 = Seen::Missing;
        app.input = ":welcome".into();
        app.typing = true;
        app.on_key(key(KeyCode::Enter));
        assert!(app.welcome);
        assert_eq!(app.vendors[0].1, Seen::Checking);
        app.looked();
        assert_eq!(app.vendors[0].1, here("2.1.3", Login::Yes));
    }

    #[test]
    fn a_later_start_asks_only_the_leader_s_vendor_and_opens_when_it_is_not_signed_in() {
        // Signed in: nothing opens, the others are not asked.
        let mut app = live_welcome(Vendor::Claude);
        app.vendors[1].1 = Seen::Missing;
        app.start_welcome(false);
        assert_eq!(app.lead_check, Some(Vendor::Claude));
        assert!(!app.welcome);
        app.looked();
        assert!(!app.welcome);
        assert_eq!(app.lead_check, None);
        assert_eq!(app.vendors[0].1, here("2.1.3", Login::Yes));
        assert_eq!(app.vendors[1].1, Seen::Missing, "not asked");
        assert_eq!(app.vendors[2].1, Seen::Checking, "not asked");
        // Not signed in: the window opens and the others are asked.
        let mut app = live_welcome(Vendor::Codex);
        app.start_welcome(false);
        assert_eq!(app.lead_check, Some(Vendor::Codex));
        assert_eq!(app.vendors[0].1, Seen::Checking);
        app.looked();
        assert!(app.welcome);
        assert_eq!(app.lead_check, None);
        assert_eq!(app.vendors[1].1, here("0.9", Login::No), "its answer stays");
        app.looked();
        assert_eq!(app.vendors[0].1, here("2.1.3", Login::Yes));
        assert_eq!(app.vendors[2].1, Seen::Missing);
        // While the person writes, a line says so instead.
        let mut app = live_welcome(Vendor::Codex);
        app.start_welcome(false);
        app.typing = true;
        app.looked();
        assert!(!app.welcome);
        let said = &app.flash.as_ref().expect("said").0;
        assert_eq!(said, "codex is not signed in: :welcome says how");
        // Nothing asked, nothing changes.
        let mut app = live_welcome(Vendor::Codex);
        app.looked();
        assert!(!app.welcome && app.flash.is_none());
    }

    #[test]
    fn the_demo_s_welcome_lists_its_own_vendors_and_news_shows_the_latest() {
        let mut app = App::new(1, 1, 0, 1, Lang::En);
        assert_eq!(app.vendors, crate::sim::vendors());
        // All three there, as live lists them; Gemini's sign-in not asked.
        let logins: Vec<(Vendor, Option<Login>)> = app
            .vendors
            .iter()
            .map(|(v, seen)| match seen {
                Seen::Here { version, login } => {
                    assert!(version.ends_with("· demo"), "{version}");
                    (*v, Some(*login))
                }
                _ => (*v, None),
            })
            .collect();
        assert_eq!(
            logins,
            [
                (Vendor::Claude, Some(Login::Yes)),
                (Vendor::Codex, Some(Login::Yes)),
                (Vendor::Gemini, Some(Login::Unknown)),
            ]
        );
        assert!(!app.welcome && app.news.is_empty() && app.look.is_none());
        app.input = ":welcome".into();
        app.typing = true;
        app.on_key(key(KeyCode::Enter));
        assert!(app.welcome);
        assert_eq!(app.vendors, crate::sim::vendors(), "nothing asked");
        app.on_key(key(KeyCode::Char('x')));
        assert!(!app.welcome);
        app.input = ":news".into();
        app.typing = true;
        app.on_key(key(KeyCode::Enter));
        let latest: Vec<&str> = Lang::En.text().news.iter().take(3).map(|n| n.0).collect();
        assert_eq!(app.news, latest);
        assert_eq!(app.news[0], welcome::VERSION);
        assert_eq!(app.command_note("news"), format!("({})", welcome::VERSION));
        // The news first, then the welcome: one key each.
        app.welcome = true;
        app.on_key(key(KeyCode::Char('x')));
        assert!(app.news.is_empty() && app.welcome);
        app.on_key(key(KeyCode::Char('x')));
        assert!(!app.welcome);
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
    fn brackets_go_from_turn_to_turn_in_a_conversation() {
        // Speakers: you you · worker · you · worker worker.
        let (mut app, worker) = with_thread();
        app.world.said(worker, "second");
        let reply = |app: &mut App, text: &str| {
            app.world.note(Event::Replied {
                agent: worker,
                text: text.into(),
            })
        };
        reply(&mut app, "one");
        app.world.said(worker, "third");
        reply(&mut app, "two");
        reply(&mut app, "three");
        app.follow_thread();
        let entries = app.thread.as_ref().unwrap().entries.clone();
        assert_eq!(entries.len(), 6);
        assert_eq!(ui::turn_starts(&app.world, &entries), [0, 2, 3, 4]);
        let pane = app.pane;
        let press = |app: &mut App, c: char| {
            app.thread.as_mut().unwrap().follow.set(false);
            app.on_key(key(KeyCode::Char(c)));
            let th = app.thread.as_ref().unwrap();
            assert!(th.follow.get(), "{c}: the drawing scrolls to it");
            th.sel
        };
        app.thread.as_mut().unwrap().sel = 0;
        assert_eq!(press(&mut app, ']'), 2, "past the person's second line");
        assert_eq!(press(&mut app, ']'), 3);
        assert_eq!(press(&mut app, ']'), 4);
        app.thread.as_mut().unwrap().follow.set(false);
        app.on_key(key(KeyCode::Char(']')));
        let th = app.thread.as_ref().unwrap();
        assert_eq!(th.sel, 4, "no turn after the last one");
        assert!(!th.follow.get());
        assert_eq!(
            press(&mut app, '['),
            3,
            "from a turn's start: the one before"
        );
        app.thread.as_mut().unwrap().sel = 5;
        assert_eq!(press(&mut app, '['), 4, "from inside a turn: its start");
        assert_eq!(press(&mut app, '['), 3);
        assert_eq!(press(&mut app, '['), 2);
        assert_eq!(press(&mut app, '['), 0);
        app.on_key(key(KeyCode::Char('[')));
        assert_eq!(app.thread.as_ref().unwrap().sel, 0, "none before the first");
        assert_eq!(app.pane, pane, "the conversation keeps [ ], no panel moves");
    }

    #[test]
    fn a_conversation_of_one_speaker_is_one_turn_and_other_lines_have_none() {
        let (mut app, worker) = with_thread();
        app.world.said(worker, "second");
        app.follow_thread();
        let entries = app.thread.as_ref().unwrap().entries.clone();
        assert_eq!(ui::turn_starts(&app.world, &entries), [0]);
        app.world.note(Event::Forgotten { agent: worker });
        let lone = app.world.journal.len() - 1;
        assert_eq!(ui::turn_starts(&app.world, &[lone]), [0]);
        assert_eq!(ui::turn_starts(&app.world, &[lone, lone]), [0]);
        assert_eq!(ui::turn_starts(&app.world, &[lone, entries[0]]), [0, 1]);
        assert!(ui::turn_starts(&app.world, &[]).is_empty());
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
    fn since_is_the_oldest_open_item_s_tick_else_the_last_status_change() {
        let mut app = quiet();
        app.world.tick = 50;
        app.world.agents[1].changed_at = 5;
        assert_eq!(app.since(1), 5);
        let item = |agent, tick, open| InboxItem {
            kind: InboxKind::Question,
            agent,
            detail: "q".into(),
            researcher: None,
            open,
            tick,
            card: None,
        };
        app.world.inbox.push(item(1, 9, true));
        assert_eq!(app.since(1), 9, "the item, even after the status change");
        app.world.inbox.push(item(1, 7, true));
        assert_eq!(app.since(1), 7, "the oldest");
        app.world.inbox.push(item(1, 3, false));
        assert_eq!(app.since(1), 7, "a closed one does not count");
        app.world.inbox.push(item(2, 2, true));
        assert_eq!(app.since(1), 7, "another's does not count");
        assert_eq!(app.since(2), 2);
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
            copy: None,
            words: None,
            folds: Folds::default(),
            room: Cell::new(usize::MAX),
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
    fn c_writes_a_note_on_the_top_line_and_again_rewrites_or_takes_it_away() {
        let patch = "diff --git a/x b/x\n--- a/x\n+++ b/x\n@@ -1,2 +1,2 @@\n-a\n+b\n c\n";
        let (mut app, worker) = with_change(patch);
        let t = app.lang.text();
        let press = |app: &mut App, c: char| app.on_key(key(KeyCode::Char(c)));
        // Off a hunk's lines: no note.
        app.diff_scroll = 3;
        press(&mut app, 'c');
        assert!(!app.typing && app.noting.is_none());
        assert_eq!(flashed(&app).as_deref(), Some(t.note_where));
        // On one: written in the command line, kept on enter.
        app.diff_scroll = 5;
        press(&mut app, 'c');
        assert!(app.typing);
        assert_eq!(app.noting, Some(Noting::Line(worker, 5)));
        assert_eq!(app.menu, None);
        // A note is no command: `:` offers none.
        press(&mut app, ':');
        assert_eq!(app.menu, None, "no commands offered in a note");
        for c in "b is wrong".chars() {
            press(&mut app, c);
        }
        app.on_key(key(KeyCode::Enter));
        assert!(!app.typing && app.noting.is_none());
        assert_eq!(
            flashed(&app).as_deref(),
            Some("note kept (1 on this change); n sends them")
        );
        let notes: Vec<(usize, String)> = (app.notes_of(worker).iter())
            .map(|n| (n.line, n.text.clone()))
            .collect();
        assert_eq!(notes, [(5, ":b is wrong".to_string())]);
        // Again on the same line: the note to write over.
        press(&mut app, 'c');
        assert_eq!(app.input, ":b is wrong");
        for _ in 0..11 {
            app.on_key(key(KeyCode::Backspace));
        }
        app.on_key(key(KeyCode::Enter));
        assert!(app.notes_of(worker).is_empty());
        assert_eq!(flashed(&app).as_deref(), Some(t.note_dropped));
        // Esc gives a note up.
        press(&mut app, 'c');
        press(&mut app, 'x');
        app.on_key(key(KeyCode::Esc));
        assert!(app.noting.is_none() && app.notes_of(worker).is_empty());
        // A note on another patch of the same worker, or on another
        // worker's, is not this one's; nor is one on another line.
        let here = patch_key(patch);
        let other = |agent, patch, line, text: &str| DiffNote {
            agent,
            patch,
            line,
            text: text.into(),
        };
        app.diff_notes.extend([
            other(worker, patch_key("older"), 5, "old"),
            other(0, here, 5, "not mine"),
            other(worker, here, 4, "above"),
        ]);
        let notes: Vec<&str> = (app.notes_of(worker).iter())
            .map(|n| n.text.as_str())
            .collect();
        assert_eq!(notes, ["above"]);
        press(&mut app, 'c');
        assert_eq!(app.input, "", "line 5 has none of its own");
        press(&mut app, 'y');
        app.on_key(key(KeyCode::Enter));
        assert_eq!(app.diff_notes.len(), 4, "kept beside the others");
        // Taking it away leaves the others.
        press(&mut app, 'c');
        assert_eq!(app.input, "y");
        app.on_key(key(KeyCode::Backspace));
        app.on_key(key(KeyCode::Enter));
        assert_eq!(app.diff_notes.len(), 3);
        app.diff_notes.retain(|n| n.text != "above");
        press(&mut app, 'n');
        assert_eq!(flashed(&app).as_deref(), Some(t.no_notes));
        assert_eq!(app.zoom, Zoom::Diff(worker), "nothing sent, still there");
    }

    #[test]
    fn w_in_a_change_wraps_its_long_lines_and_cuts_them_again() {
        let (mut app, _) = with_change("diff --git a/x b/x\n@@ -1 +1 @@\n-a\n+b\n");
        assert!(!app.diff_wrap);
        app.on_key(key(KeyCode::Char('w')));
        assert!(app.diff_wrap);
        assert_eq!(
            app.pane,
            Pane::Inbox,
            "w is the diff's here, not the panel's"
        );
        app.on_key(key(KeyCode::Char('w')));
        assert!(!app.diff_wrap);
    }

    #[test]
    fn x_chooses_the_hunk_at_hand_and_a_applies_only_the_chosen() {
        let patch =
            "diff --git a/x b/x\n--- a/x\n+++ b/x\n@@ -1 +1 @@\n-a\n+b\n@@ -9 +9 @@\n-c\n+d\n";
        let (mut app, worker) = with_change(patch);
        let t = app.lang.text();
        // On the header: the first hunk; inside the second: that one.
        app.diff_scroll = 0;
        app.on_key(key(KeyCode::Char('x')));
        assert_eq!(app.chosen_of(worker), [3]);
        assert_eq!(
            flashed(&app).as_deref(),
            Some("1 of 2 hunks chosen; a applies only them, the rest waits")
        );
        app.diff_scroll = 8;
        app.on_key(key(KeyCode::Char('x')));
        assert_eq!(app.chosen_of(worker), [3, 6]);
        // Again: no longer chosen.
        app.on_key(key(KeyCode::Char('x')));
        app.diff_scroll = 4;
        app.on_key(key(KeyCode::Char('x')));
        assert!(app.chosen_of(worker).is_empty());
        assert_eq!(flashed(&app).as_deref(), Some(t.unchosen));
        // Another change's or another worker's choice is not this one's.
        app.chosen
            .extend([(worker, patch_key("older"), 3), (0, patch_key(patch), 3)]);
        assert!(app.chosen_of(worker).is_empty());
        app.diff_scroll = 6;
        app.on_key(key(KeyCode::Char('x')));
        app.on_key(key(KeyCode::Char('a')));
        let Engine::Sim(s) = &app.engine else {
            panic!("the demo")
        };
        assert_eq!(s.applying_now(), [(worker, vec![6])]);
        // A change with no hunk: nothing to choose.
        let (mut app, worker) = with_change("diff --git a/b b/b\nBinary files differ\n");
        app.on_key(key(KeyCode::Char('x')));
        assert!(app.chosen_of(worker).is_empty());
        assert_eq!(flashed(&app).as_deref(), Some(t.no_hunk));
    }

    #[test]
    fn n_sends_the_notes_in_one_message_and_the_change_goes_back() {
        let patch = "diff --git a/x b/x\n--- a/x\n+++ b/x\n@@ -1,2 +1,2 @@\n-a\n+b\n c\n";
        let (mut app, worker) = with_change(patch);
        let name = app.world.agents[worker].name.clone();
        for (line, text) in [(6, "and here"), (4, "why")] {
            app.diff_scroll = line;
            app.on_key(key(KeyCode::Char('c')));
            for c in text.chars() {
                app.on_key(key(KeyCode::Char(c)));
            }
            app.on_key(key(KeyCode::Enter));
        }
        assert_eq!(app.notes_of(worker).len(), 2);
        app.on_key(key(KeyCode::Char('n')));
        assert!(app.world.change_of(worker).is_none(), "it went back");
        assert!(app.diff_notes.is_empty());
        assert_eq!((app.zoom, app.pane), (Zoom::None, Pane::Inbox));
        assert_eq!(
            flashed(&app),
            Some(format!(
                "2 notes went to {name}; the change went back to them"
            ))
        );
        let said = app.world.journal.iter().rev().find_map(|e| match &e.event {
            Event::YouSaid { to, text } if *to == worker => Some(text.clone()),
            _ => None,
        });
        assert_eq!(
            said,
            Some(ui::notes_message(patch, &[(4, "why"), (6, "and here")]))
        );
    }

    #[test]
    fn quote_in_a_change_writes_to_its_worker_with_the_hunk_under_the_line() {
        let patch = "diff --git a/x b/x\n--- a/x\n+++ b/x\n@@ -1 +1 @@\n-a\n+b\n";
        let (mut app, worker) = with_change(patch);
        let name = app.world.agents[worker].name.clone();
        app.diff_scroll = 4;
        app.on_key(key(KeyCode::Char('>')));
        assert!(app.typing);
        let head = format!("@{name} ");
        let quoted = "```diff\n--- a/x\n+++ b/x\n@@ -1 +1 @@\n-a\n+b\n```";
        assert_eq!(app.input, format!("{head}\n{quoted}"));
        assert_eq!(
            app.cursor(),
            head.len(),
            "on the first line, after the name"
        );
        assert_eq!(app.menu, None);
        for c in "why b".chars() {
            app.on_key(key(KeyCode::Char(c)));
        }
        app.on_key(key(KeyCode::Enter));
        assert!(!app.typing);
        let said = app.world.journal.iter().rev().find_map(|e| match &e.event {
            Event::YouSaid { to, text } if *to == worker => Some(text.clone()),
            _ => None,
        });
        assert_eq!(said, Some(format!("why b\n{quoted}")));
        // No hunk to quote: it says so and nothing is written.
        let (mut app, _) = with_change("diff --git a/b b/b\nBinary files differ\n");
        app.on_key(key(KeyCode::Char('>')));
        assert!(!app.typing && app.input.is_empty());
        assert_eq!(flashed(&app).as_deref(), Some(app.lang.text().no_hunk));
    }

    fn caught(kept: &[u8], ended: Ended) -> io::Result<Caught> {
        Ok(Caught {
            ran: Ran {
                ended,
                took: Duration::from_millis(3_000),
            },
            kept: kept.to_vec(),
            lines: kept.iter().filter(|&&b| b == b'\n').count(),
        })
    }

    #[test]
    fn o_quotes_the_last_output_of_the_project_to_the_agent_at_hand() {
        let mut app = App::new(2, 2, 0, 1, Lang::En);
        let leader = app.world.projects[1].leader;
        let other = (0..app.world.agents.len())
            .find(|&a| app.world.agents[a].project == 1 && a != leader)
            .expect("project 1 has an agent besides its leader");
        // Nothing ran yet: it says so and nothing is written.
        app.selected = Sel::Project(1);
        app.on_key(key(KeyCode::Char('O')));
        assert!(!app.typing && app.input.is_empty());
        assert_eq!(flashed(&app).as_deref(), Some(app.lang.text().no_output));
        // A command ran with project 1 at hand: its output is project 1's.
        app.ran(
            "make",
            caught(b"\x1b[31mok\x1b[0m\r\nfail\r\n", Ended::Exit(2)),
        );
        assert!(app.outputs.contains_key(&1) && !app.outputs.contains_key(&0));
        let quoted = "I ran `make` → exit 2 · 3 s\n```text\nok\nfail\n```";
        app.on_key(key(KeyCode::Char('O')));
        let head = format!("@{} ", app.world.agents[leader].name);
        assert_eq!(app.input, format!("{head}\n{quoted}"), "the project's head");
        assert!(app.typing);
        assert_eq!(app.cursor(), head.len(), "after the name");
        assert_eq!(app.menu, None);
        // The agent at hand when it is of the project.
        app.on_key(key(KeyCode::Esc));
        app.input.clear();
        app.selected = Sel::Agent(other);
        app.command("output");
        let head = format!("@{} ", app.world.agents[other].name);
        assert_eq!(app.input, format!("{head}\n{quoted}"));
        // Sent, it is the person's message to that agent, as written.
        app.on_key(key(KeyCode::Enter));
        let said = app.world.journal.iter().rev().find_map(|e| match &e.event {
            Event::YouSaid { to, text } if *to == other => Some(text.clone()),
            _ => None,
        });
        assert_eq!(said.as_deref(), Some(quoted));
        // Another project has its own, or none.
        app.selected = Sel::Project(0);
        app.on_key(key(KeyCode::Char('O')));
        assert!(!app.typing);
        assert_eq!(flashed(&app).as_deref(), Some(app.lang.text().no_output));
        // A newer run takes the place of the older one.
        app.selected = Sel::Project(1);
        app.ran("ls", caught(b"", Ended::Signal(2)));
        assert_eq!(app.outputs[&1].command, "ls");
        assert_eq!(app.command_note("output"), "(ls)");
        app.selected = Sel::Project(0);
        assert_eq!(app.command_note("output"), "");
        // A shell that did not start leaves the last output as it was.
        app.selected = Sel::Project(1);
        app.ran("x", Err(io::Error::other("no shell")));
        assert_eq!(app.outputs[&1].command, "ls");
    }

    #[test]
    fn an_output_quote_is_fenced_past_its_backticks_and_counts_what_is_above() {
        use crate::ui::words::output_quote;
        let mut o = Output {
            command: "cargo test".into(),
            ended: Ended::Exit(101),
            took: 61_000,
            lines: vec!["a ``` b".into(), "c".into()],
            above: 12,
        };
        assert_eq!(
            output_quote(Lang::En, &o),
            "I ran `cargo test` → exit 101 · 1 min 1 s\n````text\n… lines above: 12\na ``` b\nc\n````"
        );
        o.lines = vec!["x `y` ````z".into()];
        o.above = 0;
        o.ended = Ended::Signal(2);
        assert_eq!(
            output_quote(Lang::Tr, &o),
            "`cargo test` çalıştırdım → 2 sinyali durdurdu · 1 dk 1 sn sürdü\n`````text\nx `y` ````z\n`````"
        );
        o.lines = vec!["``".into()];
        o.above = 1;
        assert_eq!(
            output_quote(Lang::En, &o),
            "I ran `cargo test` → stopped by signal 2 · 1 min 1 s\n```text\n… lines above: 1\n``\n```"
        );
        // Nothing shown: only how it ran.
        o.lines.clear();
        assert_eq!(
            output_quote(Lang::En, &o),
            "I ran `cargo test` → stopped by signal 2 · 1 min 1 s"
        );
        // The command is cleaned; the lines were cleaned when kept.
        o.command = "echo TOKEN=abc\x1b".into();
        assert!(!output_quote(Lang::En, &o).contains("abc"));
    }

    /// A demo whose journal is only `n` lines that stand alone, with the
    /// journal panel of the salon at hand.
    fn journal_of(n: usize) -> App {
        let mut app = App::new(1, 1, 0, 1, Lang::En);
        app.world.journal.clear();
        for _ in 0..n {
            app.world.note(Event::Failed { agent: 0 });
        }
        app.pane = Pane::Journal;
        app
    }

    fn flashed(app: &App) -> Option<String> {
        app.flash.as_ref().map(|(m, _)| m.clone())
    }

    #[test]
    fn m_marks_the_line_at_hand_and_again_takes_the_mark_away() {
        let mut app = journal_of(3);
        let t = app.lang.text();
        app.journal_sel = Some(1);
        app.on_key(key(KeyCode::Char('m')));
        assert_eq!(app.world.marks, BTreeSet::from([1]));
        assert_eq!(flashed(&app).as_deref(), Some(t.marked));
        app.on_key(key(KeyCode::Char('m')));
        assert!(app.world.marks.is_empty());
        assert_eq!(flashed(&app).as_deref(), Some(t.unmarked));
        // `:mark` is the same.
        app.command("mark");
        assert_eq!(app.world.marks, BTreeSet::from([1]));
        // Nothing at hand: another panel, a focus, another view.
        app.world.marks.clear();
        for at in [
            |a: &mut App| a.pane = Pane::Salon,
            |a: &mut App| a.zoom = Zoom::Focus(0),
            |a: &mut App| a.view = View::Board,
        ] {
            let mut app = journal_of(3);
            app.journal_sel = Some(0);
            at(&mut app);
            app.on_key(key(KeyCode::Char('m')));
            assert!(app.world.marks.is_empty());
            assert_eq!(flashed(&app).as_deref(), Some(t.mark_where));
        }
        // In a conversation, its selected line, whatever the panel.
        let mut app = journal_of(3);
        app.pane = Pane::Salon;
        app.open_thread(2);
        app.on_key(key(KeyCode::Char('m')));
        assert_eq!(app.world.marks, BTreeSet::from([2]));
        assert!(app.thread.is_some(), "it stays open");
    }

    #[test]
    fn quote_goes_to_the_next_marked_row_of_the_journal_round_to_the_first() {
        let mut app = journal_of(5);
        let t = app.lang.text();
        // Rows newest first: journal lines 4 3 2 1 0.
        app.world.marks.extend([3, 1]);
        app.view = View::Board;
        app.zoom = Zoom::Focus(0);
        app.pane = Pane::Salon;
        app.on_key(key(KeyCode::Char('\'')));
        assert_eq!(
            (app.view, app.zoom, app.pane),
            (View::Salon, Zoom::None, Pane::Journal)
        );
        assert_eq!(
            app.journal_sel,
            Some(3),
            "nothing selected: the first marked"
        );
        app.on_key(key(KeyCode::Char('\'')));
        assert_eq!(app.journal_sel, Some(1));
        app.on_key(key(KeyCode::Char('\'')));
        assert_eq!(app.journal_sel, Some(3), "round again");
        app.journal_sel = Some(2);
        app.on_key(key(KeyCode::Char('\'')));
        assert_eq!(app.journal_sel, Some(1), "from between two: the one after");
        assert!(flashed(&app).is_none());
        // None marked: the selection stays on a row, and it says so.
        app.world.marks.clear();
        app.journal_sel = None;
        app.on_key(key(KeyCode::Char('\'')));
        assert_eq!(app.journal_sel, Some(4), "the first row");
        assert_eq!(flashed(&app).as_deref(), Some(t.no_marks));
    }

    #[test]
    fn quote_in_a_conversation_goes_among_its_own_marked_lines() {
        let mut app = App::new(1, 1, 0, 1, Lang::En);
        app.world.journal.clear();
        let said = |from, to| Event::Sent {
            from,
            to,
            kind: MsgKind::Ask,
            text: "x".into(),
            id: 0,
            re: None,
        };
        for _ in 0..2 {
            app.world.note(said(0, 1));
            app.world.note(Event::Failed { agent: 0 });
        }
        app.world.note(said(1, 0));
        // The conversation is lines 0 2 4; line 1 is marked but not in it.
        app.world.marks.extend([1, 4]);
        app.open_thread(0);
        assert_eq!(app.thread.as_ref().unwrap().entries, [0, 2, 4]);
        app.on_key(key(KeyCode::Char('\'')));
        let th = app.thread.as_ref().unwrap();
        assert_eq!(th.sel, 2);
        assert!(th.follow.get(), "the drawing scrolls to it");
        // At the only mark: it stays there.
        app.on_key(key(KeyCode::Char('\'')));
        assert_eq!(app.thread.as_ref().unwrap().sel, 2);
        // None in it: the selection stays, and it says so.
        app.world.marks.remove(&4);
        app.thread.as_mut().unwrap().sel = 1;
        app.thread.as_ref().unwrap().follow.set(false);
        app.on_key(key(KeyCode::Char('\'')));
        let th = app.thread.as_ref().unwrap();
        assert_eq!(th.sel, 1);
        assert!(!th.follow.get());
        assert_eq!(flashed(&app).as_deref(), Some(app.lang.text().no_marks));
        assert_eq!(app.pane, Pane::Salon, "the journal is not where it went");
    }

    #[test]
    fn notify_is_off_bell_or_text_by_word_or_round_and_the_menu_says_which() {
        let mut app = App::new(1, 1, 0, 1, Lang::En);
        let t = app.lang.text();
        assert_eq!(app.notify, Notify::Off);
        assert_eq!(app.command_note("notify"), "(off)");
        app.command("notify");
        assert_eq!(app.notify, Notify::Bell);
        assert_eq!(flashed(&app).as_deref(), Some(t.notify_bell));
        assert_eq!(app.command_note("notify"), "(bell)");
        app.command("notify");
        assert_eq!(app.notify, Notify::Text);
        assert_eq!(flashed(&app).as_deref(), Some(t.notify_text));
        app.command("notify");
        assert_eq!(app.notify, Notify::Off);
        assert_eq!(flashed(&app).as_deref(), Some(t.notify_off));
        app.command("notify text");
        assert_eq!(app.notify, Notify::Text);
        assert_eq!(app.command_note("notify"), "(text)");
        app.command("notify beep");
        assert_eq!(app.notify, Notify::Text, "as it was");
        assert_eq!(
            flashed(&app).as_deref(),
            Some("beep is not off, bell, text or push")
        );
    }

    #[test]
    fn in_another_window_what_came_to_wait_is_said_once_after_the_wait() {
        let mut app = waiting(&[InboxKind::Change]);
        let t0 = Instant::now();
        let at = |s: u64| t0 + Duration::from_secs(s);
        let item = |kind, agent| InboxItem {
            kind,
            agent,
            detail: "x".into(),
            researcher: None,
            open: true,
            tick: 0,
            card: None,
        };
        // Off: nothing, whatever came while away.
        app.focus(false, at(0));
        app.world.inbox.push(item(InboxKind::Question, 2));
        assert_eq!(app.notices(at(9)), None);
        app.focus(true, at(10));
        app.command("notify bell");
        // In the window: nothing, and what is there is seen.
        assert_eq!(app.notices(at(11)), None);
        app.focus(false, at(12));
        assert_eq!(app.notices(at(60)), None, "seen before leaving");
        // A question while away: said once, after the wait, in imece's
        // words with the agent's name, never the agent's.
        app.world.inbox.push(item(InboxKind::Question, 3));
        assert_eq!(app.notices(at(100)), None);
        assert_eq!(app.notices(at(104)), None, "under the wait");
        let name = app.world.agents[3].name.clone();
        assert_eq!(
            app.notices(at(105)),
            Some(notify::Word {
                body: format!("imece · {name} asks you"),
                id: 1,
                loud: false,
                agent: Some(3),
            }),
            "numbered from 1; a question is not loud"
        );
        assert_eq!(app.notices(at(200)), None, "once");
        // Its question answered while away: no news, and an idle agent
        // without an item is not stuck.
        let q3 = app.world.inbox.len() - 1;
        assert_eq!(app.world.inbox[q3].agent, 3);
        app.world.inbox[q3].open = false;
        assert_eq!(app.notices(at(201)), None);
        assert_eq!(app.notices(at(260)), None, "nothing new");
        // Several at once: the count. An agent stuck counts too, an agent
        // that waits in the inbox only once: the change of agent 4 and
        // agent 0 blocked; not agent 4 again as stuck, nor agent 1, blocked
        // with its change (the first item) long known.
        app.world.inbox.push(item(InboxKind::Change, 4));
        app.world.agents[0].status = Status::Blocked;
        app.world.agents[1].status = Status::Blocked;
        app.world.agents[4].status = Status::Stopped;
        assert_eq!(app.world.inbox[0].agent, 1);
        assert_eq!(app.notices(at(300)), None);
        assert_eq!(
            app.notices(at(305)),
            Some(notify::Word {
                body: "imece · 2 things wait for you".into(),
                id: 2,
                loud: true,
                agent: Some(4),
            }),
            "an agent stuck among them: loud"
        );
        // Back in the window: what was about to ring is dropped.
        app.world.inbox.push(item(InboxKind::Reply, 2));
        assert_eq!(app.notices(at(400)), None);
        app.focus(true, at(401));
        assert_eq!(app.notices(at(402)), None);
        app.focus(false, at(403));
        assert_eq!(app.notices(at(500)), None, "on screen when back");
        // Closed: gone from what could ring.
        app.world.inbox.push(item(InboxKind::Approval, 3));
        assert_eq!(app.notices(at(600)), None);
        app.world.inbox.last_mut().unwrap().open = false;
        assert_eq!(app.notices(at(700)), None);
    }

    #[test]
    fn a_push_says_the_kind_the_agent_and_the_project_and_the_demo_only_shows_it() {
        let mut app = waiting(&[InboxKind::Change]);
        let t0 = Instant::now();
        let at = |s: u64| t0 + Duration::from_secs(s);
        app.command("notify");
        app.command("notify");
        app.command("notify");
        assert_eq!(app.notify, Notify::Off, "push is never in the round");
        app.command("notify push");
        assert_eq!(app.notify, Notify::Push);
        assert_eq!(
            flashed(&app).as_deref(),
            Some(app.lang.text().notify_push_demo)
        );
        assert_eq!(app.push, None, "the demo reads no settings");
        app.focus(true, at(0));
        app.last_key = at(0);
        assert_eq!(app.notices(at(1)), None);
        // Away from the machine, its terminal still focused: the keys'
        // quiet counts, as long as `:away` says (30 min).
        let away = 30 * 60;
        assert_eq!(app.notices(at(away - 1)), None, "not away yet");
        app.world.inbox.push(InboxItem {
            kind: InboxKind::Question,
            agent: 3,
            detail: "the secret plan".into(),
            researcher: None,
            open: true,
            tick: 0,
            card: None,
        });
        assert_eq!(app.notices(at(away)), None, "away from now");
        assert_eq!(app.notices(at(away + 4)), None, "under the wait");
        let word = app.notices(at(away + 5)).expect("pushed");
        let (name, project) = (
            app.world.agents[3].name.clone(),
            app.world.projects[app.world.agents[3].project].name.clone(),
        );
        assert_eq!(word.body, format!("imece · {name} asks you · {project}"));
        assert!(!word.body.contains("secret"), "never what an agent wrote");
        assert_eq!(word.agent, Some(3));
        let before = app.world.journal.len();
        app.push_word(&word);
        assert_eq!(app.world.journal.len(), before + 1);
        assert_eq!(
            app.world.journal[before].event,
            Event::Pushed {
                agent: 3,
                words: word.body.clone(),
                sent: false
            }
        );
        let line = ui::words::event(Lang::En, &app.world, &app.world.journal[before].event);
        assert_eq!(
            line,
            format!(
                "a push to your phone would say (the demo sends nothing): {}",
                word.body
            )
        );
        // Not pushed: with another way, or a word about no one.
        app.notify = Notify::Text;
        app.push_word(&word);
        app.notify = Notify::Push;
        app.push_word(&notify::Word {
            agent: None,
            ..word
        });
        assert_eq!(app.world.journal.len(), before + 1);
        // The keys' quiet counts only for a push.
        app.notify = Notify::Text;
        app.world.inbox.push(InboxItem {
            kind: InboxKind::Reply,
            agent: 2,
            detail: "x".into(),
            researcher: None,
            open: true,
            tick: 0,
            card: None,
        });
        assert_eq!(app.notices(at(away + 100)), None);
        assert_eq!(app.notices(at(away + 200)), None, "in the window");
        // `:away off`: only another window.
        app.notify = Notify::Push;
        app.away = Away::parse("off").unwrap();
        app.world.inbox.push(InboxItem {
            kind: InboxKind::Change,
            agent: 4,
            detail: "x".into(),
            researcher: None,
            open: true,
            tick: 0,
            card: None,
        });
        assert_eq!(app.notices(at(away + 300)), None);
        assert_eq!(app.notices(at(away * 3)), None, "never away by the keys");
    }

    #[test]
    fn live_push_settings_are_read_at_push_and_a_failure_is_said_once() {
        let dir = std::env::temp_dir().join(format!("imece-app-push-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let file = dir.join("push");
        let w = crate::live::testing::team(1);
        let live = crate::live::testing::idle_live(2);
        let mut app = App::with(w, Engine::Live(Box::new(live)), Lang::En);
        app.push_file = Some(file.clone());
        // No settings: refused, as it was.
        app.command("notify push");
        assert_eq!(app.notify, Notify::Off);
        assert_eq!(
            flashed(&app),
            Some(format!(
                "no push settings: write {} (server, topic, token)",
                file.display()
            ))
        );
        std::fs::write(&file, "topic = short\n").unwrap();
        app.command("notify push");
        assert_eq!(app.notify, Notify::Off);
        assert!(
            flashed(&app)
                .unwrap()
                .contains("on ntfy.sh a topic of at least 16")
        );
        std::fs::write(&file, "server = http://127.0.0.1:9\ntopic = t\n").unwrap();
        app.command("notify push");
        assert_eq!(app.notify, Notify::Push);
        assert_eq!(
            flashed(&app).as_deref(),
            Some(
                "a push to your phone through http://127.0.0.1:9 when something waits and you are away"
            )
        );
        assert_eq!(app.push.as_ref().map(|p| p.topic.as_str()), Some("t"));
        // How pushes end, in the journal: a failure once until one goes.
        let tx = app.pushes.0.clone();
        let before = app.world.journal.len();
        let ends: [Result<(), String>; 5] = [
            Ok(()),
            Err("down".into()),
            Err("down".into()),
            Ok(()),
            Err("refused".into()),
        ];
        for end in ends {
            tx.send((1, "imece · w".into(), end)).unwrap();
        }
        app.changed = false;
        app.pushed();
        assert!(app.changed);
        let got: Vec<Event> = app.world.journal[before..]
            .iter()
            .map(|e| e.event.clone())
            .collect();
        let sent = |agent| Event::Pushed {
            agent,
            words: "imece · w".into(),
            sent: true,
        };
        let failed = |why: &str| Event::PushFailed {
            agent: 1,
            why: why.into(),
        };
        assert_eq!(got, [sent(1), failed("down"), sent(1), failed("refused")]);
        app.changed = false;
        app.pushed();
        assert!(!app.changed, "nothing new");
        // A push sent for real: curl to a port no one listens on fails.
        app.push_failing = false;
        let word = notify::Word {
            body: "imece · x".into(),
            id: 1,
            loud: true,
            agent: Some(0),
        };
        app.push_word(&word);
        let before = app.world.journal.len();
        for _ in 0..300 {
            app.pushed();
            if app.world.journal.len() > before {
                break;
            }
            std::thread::sleep(Duration::from_millis(50));
        }
        assert!(
            matches!(
                &app.world.journal[before].event,
                Event::PushFailed { agent: 0, .. }
            ),
            "{:?}",
            app.world.journal.get(before)
        );
        // The settings read again at the next start: gone, nothing pushed.
        std::fs::remove_file(&file).unwrap();
        app.start_push();
        assert_eq!(app.notify, Notify::Off);
        assert!(flashed(&app).unwrap().starts_with("no push settings"));
        // Kept and still good: as it was, said nothing.
        std::fs::write(&file, "server = http://127.0.0.1:9\ntopic = t\n").unwrap();
        app.notify = Notify::Push;
        app.flash = None;
        app.start_push();
        assert_eq!((app.notify, flashed(&app)), (Notify::Push, None));
        // Another way kept: nothing read.
        std::fs::remove_file(&file).unwrap();
        app.notify = Notify::Bell;
        app.start_push();
        assert_eq!((app.notify, flashed(&app)), (Notify::Bell, None));
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// A demo app whose hired worker's new worktree asks about the setup
    /// script, the ask selected in WAITING FOR YOU.
    pub(crate) fn asking_setup() -> (App, usize, AgentId) {
        let mut app = waiting(&[]);
        let Engine::Sim(s) = &mut app.engine else {
            unreachable!()
        };
        let id = s.hire_for_test(&mut app.world, 0);
        let item = (app.world.inbox.iter())
            .position(|i| i.kind == InboxKind::Script)
            .expect("asked");
        app.pane = Pane::Inbox;
        app.inbox_sel = app.inbox_rows().iter().position(|&i| i == item).unwrap();
        (app, item, id)
    }

    #[test]
    fn enter_on_a_setup_ask_shows_its_bytes_and_a_or_r_answer_there() {
        let (mut app, item, id) = asking_setup();
        app.on_key(key(KeyCode::Enter));
        assert_eq!(app.script, Some((item, 0)));
        let text = app.script_text(item).unwrap().text();
        let lines = text.lines().count();
        assert_eq!(lines, 6);
        // Moving: never past the last line's being the top one.
        for (k, at) in [
            (key(KeyCode::Char('j')), 1),
            (key(KeyCode::Down), 2),
            (key(KeyCode::Char('k')), 1),
            (key(KeyCode::Up), 0),
            (key(KeyCode::Up), 0),
            (key(KeyCode::Char('G')), lines - 1),
            (key(KeyCode::Char('j')), lines - 1),
            (key(KeyCode::Char('g')), 0),
            (key(KeyCode::PageDown), lines - 1),
            (key(KeyCode::PageUp), 0),
            (ctrl('d'), lines - 1),
            (ctrl('u'), 0),
            (key(KeyCode::Char('d')), 0),
            (key(KeyCode::Char('x')), 0),
            (key(KeyCode::Char('j')), 1),
            (key(KeyCode::Char('u')), 1),
        ] {
            app.on_key(k);
            assert_eq!(app.script, Some((item, at)), "{k:?}");
        }
        assert_eq!(SCRIPT_PAGE, 10);
        // `?` opens the help over it; esc closes the window.
        app.on_key(key(KeyCode::Char('?')));
        assert!(app.help);
        app.help = false;
        app.on_key(key(KeyCode::Esc));
        assert_eq!(app.script, None);
        app.on_key(key(KeyCode::Enter));
        app.on_key(key(KeyCode::Char('q')));
        assert_eq!(app.script, None);
        // `a` in the window: yes, it ran, the window closed.
        app.on_key(key(KeyCode::Enter));
        app.on_key(key(KeyCode::Char('a')));
        assert_eq!(app.script, None);
        assert!(!app.world.inbox[item].open);
        assert_eq!(flashed(&app).as_deref(), Some(app.lang.text().setup_yes));
        assert!(
            app.world
                .journal
                .iter()
                .any(|e| matches!(e.event, Event::SetupRan { agent, .. } if agent == id))
        );
        // Answered already: said so.
        app.script = Some((item, 0));
        app.on_key(key(KeyCode::Char('r')));
        assert_eq!(flashed(&app).as_deref(), Some(app.lang.text().setup_gone));
        assert_eq!(app.script, None);
    }

    #[test]
    fn r_on_a_setup_ask_in_waiting_for_you_says_no() {
        let (mut app, item, _) = asking_setup();
        app.on_key(key(KeyCode::Char('r')));
        assert!(!app.world.inbox[item].open);
        assert_eq!(flashed(&app).as_deref(), Some(app.lang.text().setup_no));
        assert!(app.world.setup_approved.is_empty());
        let (mut app, item, _) = asking_setup();
        app.on_key(key(KeyCode::Char('a')));
        assert!(!app.world.inbox[item].open);
        assert_eq!(app.world.setup_approved.len(), 1);
        assert_eq!(app.inbox_sel, 0);
    }

    #[test]
    fn the_demo_keeps_push_as_it_was_at_start() {
        let mut app = waiting(&[]);
        app.push_file = None;
        app.notify = Notify::Push;
        app.start_push();
        assert_eq!((app.notify, flashed(&app)), (Notify::Push, None));
    }

    #[test]
    fn a_click_on_a_notification_opens_what_it_was_about() {
        let mut app = waiting(&[InboxKind::Change]);
        let t0 = Instant::now();
        let at = |s: u64| t0 + Duration::from_secs(s);
        let item = |kind, agent| InboxItem {
            kind,
            agent,
            detail: "x".into(),
            researcher: None,
            open: true,
            tick: 0,
            card: None,
        };
        app.command("notify text");
        app.focus(true, at(0));
        assert_eq!(app.notices(at(1)), None);
        app.focus(false, at(2));
        // One question: its notification opens it.
        app.world.inbox.push(item(InboxKind::Question, 3));
        let q = app.world.inbox.len() - 1;
        assert_eq!(app.notices(at(3)), None);
        let asked = app.notices(at(9)).unwrap();
        // A fault is loud on its own.
        app.world.inbox.push(item(InboxKind::Fault, 2));
        assert_eq!(app.notices(at(10)), None);
        let fault = app.notices(at(16)).unwrap();
        assert!(fault.loud && !asked.loud);
        assert_eq!(fault.id, asked.id + 1);
        // Agent 0 stuck, alone.
        app.world.agents[0].status = Status::Blocked;
        assert_eq!(app.notices(at(17)), None);
        let stuck = app.notices(at(23)).unwrap();
        assert!(stuck.loud);
        // Over windows that would hide it.
        app.help = true;
        app.peek = Some(1);
        app.welcome = true;
        app.leaving = true;
        app.archives = Some(Browser::new(&app.world, Vec::new()));
        app.viewing = Some(Box::new(App::new(1, 1, 0, 1, Lang::En)));
        app.clicked(asked.id);
        assert!(!app.help && app.peek.is_none() && !app.welcome && !app.leaving);
        assert!(app.archives.is_none() && app.viewing.is_none());
        assert_eq!(app.waiting_at, Some(q), "the question it was about");
        assert_eq!((app.view, app.pane), (View::Salon, Pane::Inbox));
        app.clicked(stuck.id);
        assert_eq!(app.selected, Sel::Agent(0), "the stuck agent");
        // The question answered meanwhile: the next item waiting instead.
        app.world.inbox[q].open = false;
        app.waiting_at = None;
        app.clicked(asked.id);
        assert_ne!(app.waiting_at, Some(q));
        assert!(app.waiting_at.is_some(), "{:?}", app.waiting_at);
        // A number imece did not send does nothing.
        app.selected = Sel::Agent(2);
        app.help = true;
        app.clicked(999);
        assert!(app.help && app.selected == Sel::Agent(2));
        // Only the last NOTICED are kept.
        for n in 0..NOTICED + 2 {
            app.noticed.push((100 + n as u64, vec![Note::Stuck(1)]));
        }
        let over = app.noticed.len() - NOTICED;
        app.noticed.drain(..over);
        app.clicked(100);
        assert_eq!(app.selected, Sel::Agent(2), "forgotten");
        app.clicked(101 + NOTICED as u64);
        assert_eq!(app.selected, Sel::Agent(1));
    }

    #[test]
    fn several_notices_at_once_are_one_that_opens_the_next_waiting() {
        let mut app = waiting(&[InboxKind::Change, InboxKind::Question]);
        let t0 = Instant::now();
        let at = |s: u64| t0 + Duration::from_secs(s);
        app.command("notify text");
        app.focus(false, at(0));
        // The two items came while away: one notice for both.
        let word = app.notices(at(1));
        assert_eq!(word, None);
        let word = app.notices(at(7)).unwrap();
        assert!(word.body.contains('2'), "{}", word.body);
        app.help = true;
        app.clicked(word.id);
        assert!(!app.help);
        assert!(app.waiting_at.is_some());
        // The list keeps NOTICED notices, the oldest going first.
        for n in 0..NOTICED + 3 {
            app.world.inbox.push(InboxItem {
                kind: InboxKind::Reply,
                agent: n % 4,
                detail: "x".into(),
                researcher: None,
                open: true,
                tick: 0,
                card: None,
            });
            let s = 10 + 10 * n as u64;
            assert_eq!(app.notices(at(s)), None);
            assert!(app.notices(at(s + 6)).is_some());
        }
        assert_eq!(app.noticed.len(), NOTICED);
        assert_eq!(app.noticed[0].0, word.id + 4, "the oldest gone");
    }

    #[test]
    fn a_notice_names_the_kind_of_what_waits_and_its_agent() {
        use crate::notify::Note;
        let mut app = waiting(&[
            InboxKind::Sensitive,
            InboxKind::Reply,
            InboxKind::Change,
            InboxKind::Question,
        ]);
        // Two more kinds, from the first two agents again.
        for (n, kind) in [(1, InboxKind::Research), (2, InboxKind::Approval)] {
            let mut item = app.world.inbox[0].clone();
            item.kind = kind;
            item.agent = n;
            app.world.inbox.push(item);
        }
        let w = &app.world;
        let name = |n: usize| w.agents[n].name.clone();
        let said: Vec<String> = (0..6)
            .map(|i| ui::words::notice(Lang::En, w, Note::Item(i)))
            .collect();
        assert_eq!(
            said,
            [
                format!("imece · {}: something sensitive waits for you", name(1)),
                format!("imece · {} answered you", name(2)),
                format!("imece · {} finished: a change waits for you", name(3)),
                format!("imece · {} asks you", name(4)),
                format!("imece · {}: something sensitive waits for you", name(1)),
                format!("imece · {} waits for your yes", name(2)),
            ]
        );
        assert_eq!(
            ui::words::notice(Lang::Tr, w, Note::Stuck(2)),
            format!("imece · {} takıldı", name(2))
        );
        // A push adds the project of whom it is about.
        let project = |id: AgentId| w.projects[w.agents[id].project].name.clone();
        assert_eq!(
            ui::words::pushed(Lang::En, w, Note::Stuck(2)),
            format!("imece · {} is stuck · {}", name(2), project(2))
        );
        assert_eq!(
            ui::words::pushed(Lang::En, w, Note::Item(3)),
            format!("imece · {} asks you · {}", name(4), project(4))
        );
    }

    #[test]
    fn each_refusal_of_push_settings_says_what_to_mend() {
        use crate::push::Why;
        let say = |why: Why| ui::words::push_why(Lang::En, &why);
        assert_eq!(
            say(Why::NoFile("/c/push".into())),
            "no push settings: write /c/push (server, topic, token)"
        );
        assert_eq!(
            say(Why::Unreadable("not text".into())),
            "push settings: not text"
        );
        assert!(say(Why::Line(3)).starts_with("push settings, line 3: "));
        assert_eq!(say(Why::NoTopic), "push settings: no topic");
        assert!(
            say(Why::Server("http://a.b".into())).starts_with("push settings: http://a.b is not")
        );
        assert!(say(Why::Topic).ends_with("at most 64"));
        assert!(say(Why::Guessable).contains("at least 16 with a digit"));
        assert_eq!(
            say(Why::Token("/t: not one token".into())),
            "push settings: the token file /t: not one token"
        );
        assert!(ui::words::push_why(Lang::Tr, &Why::NoTopic).contains("topic yok"));
    }

    #[test]
    fn the_machine_is_held_awake_while_an_agent_is_mid_turn_when_asked() {
        let mut app = App::new(1, 1, 0, 1, Lang::En);
        let t = app.lang.text();
        let sleeper = || Some(vec!["sleep".to_string(), "30".to_string()]);
        app.awake = crate::awake::Awake::new(false, sleeper());
        let tick = app.world.tick;
        for a in &mut app.world.agents {
            a.set_status(Status::Idle, tick);
        }
        assert_eq!(app.command_note("awake"), "(off)");
        app.world.agents[0].set_status(Status::Working, tick);
        app.keep_awake();
        assert!(!app.awake.holding(), "not asked for");
        app.command("awake");
        assert_eq!(flashed(&app).as_deref(), Some(t.awake_on));
        assert_eq!(app.command_note("awake"), "(on)");
        assert!(app.awake.holding(), "at once, mid-turn");
        app.world.agents[0].set_status(Status::Idle, tick);
        app.keep_awake();
        assert!(!app.awake.holding(), "nothing mid-turn");
        app.world.agents[0].set_status(Status::Working, tick);
        app.keep_awake();
        assert!(app.awake.holding());
        app.command("awake");
        assert_eq!(flashed(&app).as_deref(), Some(t.awake_off));
        assert!(!app.awake.holding(), "turned off: let go at once");
        // One that can not start says so.
        app.awake = crate::awake::Awake::new(true, Some(vec!["/nonexistent/caffeinate".into()]));
        app.keep_awake();
        let said = flashed(&app).unwrap();
        assert!(
            said.starts_with("could not keep the machine awake: /nonexistent/caffeinate: "),
            "{said}"
        );
    }

    /// A demo of two projects that ran a while, project 1 archived as
    /// "epic", and its read-only view open from the archives browser.
    fn viewing() -> App {
        let mut app = App::new(2, 2, 0, 7, Lang::En);
        for _ in 0..120 {
            app.step();
        }
        app.selected = Sel::Agent(app.world.projects[1].leader);
        app.command("archive epic");
        app.end_grace();
        app.world.recall_line("a line sent");
        app.on_key(key(KeyCode::Char('A')));
        app.on_key(key(KeyCode::Enter));
        assert!(app.viewing.is_some(), "{:?}", app.flash);
        app
    }

    fn view(app: &App) -> &App {
        app.viewing.as_deref().expect("the view is open")
    }

    /// `line` written in the view and sent, its menu put away first.
    fn view_typed(app: &mut App, line: &str) {
        // A plain message: writing starts with `i`.
        if !line.starts_with([':', '!', '@']) {
            app.on_key(key(KeyCode::Char('i')));
        }
        for c in line.chars() {
            app.on_key(key(KeyCode::Char(c)));
        }
        if let Some(v) = app.viewing.as_mut() {
            v.menu = None;
        }
        app.on_key(key(KeyCode::Enter));
    }

    fn said(app: &App) -> &str {
        app.flash.as_ref().map_or("", |f| f.0.as_str())
    }

    #[test]
    fn an_archive_s_view_opens_no_editor() {
        let mut app = viewing();
        app.on_key(key(KeyCode::Char('i')));
        app.on_key(key(KeyCode::Char('a')));
        app.on_key(ctrl('x'));
        app.on_key(ctrl('e'));
        assert!(view(&app).editor.is_none() && app.editor.is_none());
        assert_eq!(said(view(&app)), Lang::En.text().read_only);
    }

    #[test]
    fn enter_in_the_archives_opens_its_view_of_only_that_archive() {
        let app = viewing();
        let v = view(&app);
        let at = app.world.projects[1].archived.as_ref().unwrap().at;
        assert_eq!(
            v.viewed(),
            Some(&Viewed {
                project: 1,
                title: "epic".into(),
                at,
            })
        );
        assert!(app.archives.is_none(), "the browser gave way");
        let (lines, _) = app.world.journal_of(1);
        assert!(!lines.is_empty());
        assert_eq!(v.world.journal, lines, "its lines, and only those");
        assert!(!v.world.projects[1].closed && v.world.projects[1].archived.is_none());
        assert_eq!(v.selected, Sel::Agent(v.world.projects[1].leader));
        assert_eq!(v.lang, app.lang);
        // The screen behind goes on; the view does not move.
        let tick = v.world.tick;
        let mut app = app;
        app.step();
        app.step();
        assert!(app.world.tick > tick);
        assert_eq!(view(&app).world.tick, tick);
        // `enter` on one of its agents opens it with that agent selected.
        app.viewing = None;
        let worker = (app.world.agents.iter())
            .find(|a| a.project == 1 && a.role == Role::Worker)
            .map(|a| a.id)
            .unwrap();
        app.view_archive(1, Some(worker));
        assert_eq!(view(&app).selected, Sel::Agent(worker));
        // What is not an archive opens nothing.
        app.viewing = None;
        app.view_archive(0, None);
        assert!(app.viewing.is_none());
    }

    #[test]
    fn a_view_is_at_rest_only_with_nothing_open_that_esc_closes() {
        let mut rest = App::new(1, 1, 0, 1, Lang::En);
        assert!(rest.at_rest());
        for v in [View::Board, View::Code] {
            rest.view = v;
            assert!(rest.at_rest(), "{v:?}");
        }
        let busy: [fn(&mut App); 11] = [
            |a| a.typing = true,
            |a| a.help = true,
            |a| a.pending = Some('g'),
            |a| a.keys_held = Some((key(KeyCode::Char('x')), Instant::now())),
            |a| {
                a.hints = Some(Hints {
                    labels: Vec::new(),
                    typed: String::new(),
                })
            },
            |a| a.noting = Some(Noting::Line(0, 0)),
            |a| a.card = Some(0),
            |a| a.zoom = Zoom::Focus(0),
            |a| a.zoom = Zoom::Diff(0),
            |a| a.peek = Some(0),
            |a| a.archives = Some(Browser::new(&a.world, Vec::new())),
        ];
        for (i, busy) in busy.iter().enumerate() {
            let mut a = App::new(1, 1, 0, 1, Lang::En);
            busy(&mut a);
            assert!(!a.at_rest(), "{i}");
        }
    }

    #[test]
    fn the_cursor_stands_where_the_person_types_and_its_shape_says_what() {
        let typed = |input: &str, set: fn(&mut App)| {
            let mut a = App::new(1, 1, 0, 1, Lang::En);
            a.typing = true;
            a.input = input.into();
            set(&mut a);
            a.caret()
        };
        let line = |c| Some((Field::Line, c));
        let as_is = |_: &mut App| {};
        assert_eq!(typed("", |a| a.typing = false), None, "nothing typed");
        assert_eq!(typed("hello", as_is), line(Caret::Write));
        assert_eq!(typed("@lead hi", as_is), line(Caret::Write));
        assert_eq!(typed(":budget 80", as_is), line(Caret::Command));
        assert_eq!(typed("!cargo test", as_is), line(Caret::Write));
        assert_eq!(typed("#323", as_is), line(Caret::Find));
        assert_eq!(typed("&claude-1", as_is), line(Caret::Find));
        // The heading's order: a note, then `^r`, then `!`, `#` `&`, `:`.
        let note = |a: &mut App| a.noting = Some(Noting::Line(0, 0));
        let search = |a: &mut App| a.searching = true;
        assert_eq!(typed(":x", note), line(Caret::Write));
        assert_eq!(typed("#x", note), line(Caret::Write));
        assert_eq!(typed(":x", search), line(Caret::Find));
        assert_eq!(typed("!x", search), line(Caret::Find));
        assert_eq!(typed("x", search), line(Caret::Find));
        // A window on top that takes no text hides it, each on its own.
        let windows: [fn(&mut App); 7] = [
            |a| a.news = vec!["x"],
            |a| a.welcome = true,
            |a| {
                a.digest = Some(Digest {
                    minutes: 4,
                    merged: 0,
                    blocked: 0,
                    assumptions: 0,
                    talks: 0,
                    waiting: 0,
                    highlights: Vec::new(),
                })
            },
            |a| a.left_off = Some(LeftOff::default()),
            |a| a.leaving = true,
            |a| a.solo_pick = Some(SoloPick::new(None, None, Vec::new())),
            |a| a.help = true,
        ];
        for (i, window) in windows.iter().enumerate() {
            assert_eq!(typed("hello", *window), None, "{i}");
        }
        // The filters of the help and the archives take it while typed.
        let help = |a: &mut App| {
            a.help = true;
            a.help_typing = true;
        };
        assert_eq!(typed("hello", help), Some((Field::Help, Caret::Find)));
        let browsing = |a: &mut App| a.archives = Some(Browser::new(&a.world, Vec::new()));
        assert_eq!(typed("hello", browsing), None, "browsing, not filtering");
        let filtering = |a: &mut App| {
            let mut b = Browser::new(&a.world, Vec::new());
            b.typing = true;
            a.archives = Some(b);
            a.help = true;
            a.help_typing = true;
        };
        assert_eq!(
            typed("", filtering),
            Some((Field::Archives, Caret::Find)),
            "the archives' window is over the help's"
        );
        // An archive's view has the cursor of its own line.
        let mut app = App::new(1, 1, 0, 1, Lang::En);
        app.typing = true;
        app.input = "hello".into();
        let mut v = App::new(1, 1, 0, 1, Lang::En);
        v.typing = true;
        v.input = ":quit".into();
        app.viewing = Some(Box::new(v));
        assert_eq!(app.caret(), line(Caret::Command));
        app.viewing.as_mut().unwrap().typing = false;
        assert_eq!(app.caret(), None, "the view types nothing");
    }

    #[test]
    fn keys_go_to_the_view_and_esc_q_and_ctrl_c_leave_it() {
        let mut app = viewing();
        // ctrl-r searches the view's lines sent, not this screen's.
        app.on_key(ctrl('r'));
        assert!(view(&app).searching && !app.searching);
        // `esc` ends that first, the view stays; then it closes the view.
        app.viewing.as_mut().unwrap().menu = None;
        app.on_key(key(KeyCode::Esc));
        assert!(!view(&app).typing);
        app.on_key(KeyEvent::new(KeyCode::Esc, KeyModifiers::SHIFT));
        app.on_key(key(KeyCode::Char('x')));
        assert!(app.viewing.is_some(), "only a plain esc or q leaves");
        app.on_key(key(KeyCode::Esc));
        assert!(app.viewing.is_none());
        assert!(!app.quit);
        // `q` leaves it too.
        app.view_archive(1, None);
        app.on_key(key(KeyCode::Char('q')));
        assert!(app.viewing.is_none() && !app.quit);
        // `:quit` and its `q` leave the view, not imece.
        app.view_archive(1, None);
        view_typed(&mut app, ":quit");
        assert!(app.viewing.is_some());
        app.on_key(key(KeyCode::Char('q')));
        assert!(app.viewing.is_none() && !app.quit);
        // ctrl-c quits imece from the view.
        app.view_archive(1, None);
        app.on_key(ctrl('c'));
        assert!(app.quit && app.viewing.is_some());
    }

    #[test]
    fn what_the_view_copies_pastes_and_flashes_goes_through_the_screen_behind() {
        let mut app = viewing();
        app.viewing.as_mut().unwrap().copy = Some("words".into());
        app.on_key(key(KeyCode::Char('x')));
        assert_eq!(app.copy.as_deref(), Some("words"));
        assert!(view(&app).copy.is_none());
        // A paste goes into what the view writes.
        app.on_key(key(KeyCode::Char(':')));
        app.paste("board");
        assert_eq!(view(&app).input, ":board");
        assert!(app.input.is_empty());
        // A key the key file holds back is let through in the view, and
        // what it did is drawn.
        let now = Instant::now();
        app.on_key(key(KeyCode::Esc));
        app.changed = false;
        app.viewing.as_mut().unwrap().keys_held = Some((key(KeyCode::Char(':')), now - keys::WAIT));
        app.keys_due(now);
        assert!(view(&app).keys_held.is_none() && view(&app).typing);
        assert!(app.changed && !view(&app).changed);
        app.on_key(key(KeyCode::Esc));
        // What was to be drawn still is.
        app.viewing.as_mut().unwrap().keys_held = Some((key(KeyCode::Char(':')), now - keys::WAIT));
        app.keys_due(now);
        assert!(app.changed && view(&app).typing);
        app.on_key(key(KeyCode::Esc));
        // The view's flash is the screen's clock.
        app.drawn(now);
        assert!(!app.draw_due(now));
        app.viewing.as_mut().unwrap().flash = Some(("x".into(), now));
        assert!(app.draw_due(now));
    }

    #[test]
    fn the_view_refuses_what_would_act_and_runs_what_only_looks() {
        let mut app = viewing();
        let read_only = Lang::En.text().read_only;
        let look = |a: &App| {
            let w = &view(a).world;
            let projects: Vec<_> = (w.projects.iter())
                .map(|q| {
                    (
                        q.name.clone(),
                        q.closed,
                        q.archived.clone(),
                        q.copies.clone(),
                    )
                })
                .collect();
            let agents: Vec<_> = (w.agents.iter()).map(|a| (a.status, a.gone)).collect();
            (w.journal.len(), w.marks.clone(), w.budget, projects, agents)
        };
        let was = look(&app);
        let lead = view(&app).world.projects[1].leader;
        let to_lead = format!("@{} hi", view(&app).world.agents[lead].name);
        for line in [
            ":budget 5",
            "hello there",
            &to_lead,
            "!ls",
            ":archives",
            ":mark",
            ":close",
        ] {
            app.viewing.as_mut().unwrap().flash = None;
            view_typed(&mut app, line);
            assert_eq!(said(view(&app)), read_only, "{line}");
            assert!(app.shell.is_none() && view(&app).shell.is_none());
        }
        for k in ['m', 'A', 'a', 'r'] {
            let v = app.viewing.as_mut().unwrap();
            v.flash = None;
            v.pane = if k == 'm' { Pane::Journal } else { Pane::Inbox };
            v.journal_sel = Some(0);
            app.on_key(key(KeyCode::Char(k)));
            assert_eq!(said(view(&app)), read_only, "{k}");
            assert!(view(&app).archives.is_none());
        }
        // Every way to the engine says the same, and changes nothing.
        let calls: [fn(&mut App, AgentId); 10] = [
            |v, _| v.copy_path("target"),
            |v, _| v.suggest_commit(),
            |v, id| {
                assert!(!v.tell(id, "hi"));
            },
            |v, _| v.open_project("/tmp/elsewhere"),
            |v, _| v.close_now(1),
            |v, _| v.archive_now(1),
            |v, _| v.restore_at(1),
            |v, id| v.forget_now(id),
            |v, _| v.forget_agent(""),
            |v, id| v.decide_change(id, true),
        ];
        for (i, call) in calls.iter().enumerate() {
            let v = app.viewing.as_mut().unwrap();
            v.flash = None;
            call(v, lead);
            assert_eq!(said(view(&app)), read_only, "{i}");
        }
        assert_eq!(look(&app), was);
        assert_eq!(view(&app).model_of(lead), None);
        assert!(view(&app).vendors().is_empty());
        // An unknown command is one, not a refusal.
        view_typed(&mut app, ":nosuch");
        assert_eq!(said(view(&app)), "no command nosuch; : and tab list them");
        // What only looks or goes runs; nothing pauses.
        view_typed(&mut app, ":board");
        assert_eq!(view(&app).view, View::Board);
        app.on_key(key(KeyCode::Char('1')));
        app.on_key(key(KeyCode::Char(' ')));
        assert!(!view(&app).paused);
        view_typed(&mut app, ":lang tr");
        assert_eq!((view(&app).lang, app.lang), (Lang::Tr, Lang::En));
        // `:restore` brings the archive back from behind it.
        view_typed(&mut app, ":restore");
        assert!(app.viewing.is_none());
        assert!(app.world.projects[1].archived.is_none());
        assert!(said(&app).starts_with("brought "), "{}", said(&app));
    }

    #[test]
    fn coming_back_after_half_an_hour_away_shows_what_happened_at_once() {
        let mut app = App::new(1, 1, 0, 1, Lang::En);
        let t0 = Instant::now();
        let away = |app: &mut App, secs: u64| {
            app.focus(false, t0);
            app.world.tick += 1;
            app.world.note(Event::Failed { agent: 0 });
            app.focus(true, t0 + Duration::from_secs(secs));
        };
        // Not long enough: nothing, and the blur is forgotten.
        away(&mut app, 1799);
        assert!(app.digest.is_none() && app.blurred.is_none());
        // Half an hour: what happened since the blur.
        away(&mut app, 1800);
        let d = app.digest.take().expect("a digest");
        assert_eq!(d.highlights.len(), 1, "only what came after leaving");
        // Focus back without a blur first: nothing.
        app.focus(true, t0 + Duration::from_secs(3600));
        assert!(app.digest.is_none());
        // Nothing new: nothing to tell.
        app.focus(false, t0);
        app.focus(true, t0 + Duration::from_secs(3600));
        assert!(app.digest.is_none());
        // Not over writing or another window.
        for busy in [
            |a: &mut App| a.typing = true,
            |a: &mut App| a.help = true,
            |a: &mut App| a.leaving = true,
            |a: &mut App| a.left_off = Some(LeftOff::default()),
            |a: &mut App| a.solo_pick = Some(SoloPick::new(None, None, Vec::new())),
            |a: &mut App| a.archives = Some(Browser::new(&a.world, Vec::new())),
            |a: &mut App| a.viewing = Some(Box::new(App::new(1, 1, 0, 1, Lang::En))),
        ] {
            let mut app = App::new(1, 1, 0, 1, Lang::En);
            busy(&mut app);
            away(&mut app, 3600);
            assert!(app.digest.is_none());
        }
        let mut app = App::new(1, 1, 0, 1, Lang::En);
        app.digest = Some(Digest {
            minutes: 42,
            ..Digest::default()
        });
        away(&mut app, 3600);
        assert_eq!(
            app.digest.as_ref().unwrap().minutes,
            42,
            "the one open stays"
        );
    }

    #[test]
    fn away_sets_how_long_until_what_happened_shows_and_off_never() {
        let mut app = App::new(1, 1, 0, 1, Lang::En);
        let t0 = Instant::now();
        let away = |app: &mut App, secs: u64| {
            app.focus(false, t0);
            app.world.tick += 1;
            app.world.note(Event::Failed { agent: 0 });
            app.focus(true, t0 + Duration::from_secs(secs));
            app.digest.take().is_some()
        };
        assert_eq!(app.command_note("away"), "(30m)");
        app.command("away 5m");
        assert_eq!(
            said(&app),
            "back after 5m or more away, what happened shows at once"
        );
        assert_eq!(app.command_note("away"), "(5m)");
        assert!(!away(&mut app, 299));
        assert!(away(&mut app, 300));
        // Bare: the next one round.
        app.command("away");
        assert_eq!(app.away, Away::parse("15m").unwrap());
        app.command("away 1h");
        app.command("away");
        assert_eq!(app.away, Away::parse("off").unwrap());
        assert_eq!(
            said(&app),
            "what happened never shows by itself; :digest shows it"
        );
        assert!(!away(&mut app, 86_400 * 7), "off: never");
        app.command("away soon");
        assert_eq!(
            said(&app),
            "soon is not off, minutes (30m) or hours (1h), up to a day"
        );
        assert_eq!(app.away, Away::parse("off").unwrap(), "kept");
    }

    #[test]
    fn a_key_after_half_an_hour_without_one_shows_what_happened_first() {
        let mut app = App::new(1, 1, 0, 1, Lang::En);
        let ago = |secs| {
            Instant::now()
                .checked_sub(Duration::from_secs(secs))
                .unwrap()
        };
        let idle = |app: &mut App, secs: u64| {
            app.last_key = ago(secs);
            app.last_key_tick = app.world.tick;
            app.world.tick += 1;
            app.world.note(Event::Failed { agent: 0 });
            app.on_key(key(KeyCode::Char('j')));
            app.digest.take().is_some()
        };
        assert!(!idle(&mut app, 1790), "under half an hour");
        assert!(idle(&mut app, 1801));
        // The key that opened it did nothing else; the next acts.
        assert!(!idle(&mut app, 0));
        app.command("away 2m");
        assert!(!idle(&mut app, 110));
        assert!(idle(&mut app, 130));
        app.command("away off");
        assert!(!idle(&mut app, 86_400));
    }

    #[test]
    fn clear_takes_waiting_and_the_journal_off_the_screen_after_the_grace() {
        let mut app = waiting(&[InboxKind::Change, InboxKind::Question]);
        for _ in 0..3 {
            app.world.tick += 1;
            app.world.note(Event::Failed { agent: 0 });
        }
        let marked = app.world.journal.len() - 2;
        app.world.marks.insert(marked);
        let rows = app.journal_rows().len();
        assert!(rows >= 3);
        // A line of the journal at hand, and the marked one.
        let first = app.journal_rows()[0].0;
        assert_ne!(first, marked);
        app.journal_sel = Some(first);
        app.inbox_sel = 1;
        app.command("clear");
        assert!(matches!(app.grace, Some((Undoable::Clear(Clear::All), _))));
        assert_eq!(app.inbox_rows().len(), 2, "not before the grace");
        app.grace_due(Instant::now() + UNDO_GRACE);
        assert!(app.grace.is_none());
        assert_eq!(
            said(&app),
            format!(
                "2 waiting and {} journal lines off the screen; :unclear brings them back",
                rows - 1
            )
        );
        assert!(app.inbox_rows().is_empty());
        assert_eq!((app.inbox_sel, app.world.hidden_inbox()), (0, 2));
        assert_eq!(app.world.open_inbox(), [0, 1], "nothing closed");
        assert_eq!(app.journal_rows(), [(marked, 0)], "a mark stays");
        assert_eq!(app.journal_sel, None, "its line went");
        app.journal_sel = Some(marked);
        // What comes after shows.
        app.world.inbox.push(InboxItem {
            kind: InboxKind::Reply,
            agent: 3,
            detail: "new".into(),
            researcher: None,
            open: true,
            tick: 9,
            card: None,
        });
        app.world.note(Event::Failed { agent: 0 });
        assert_eq!(app.inbox_rows(), [2]);
        let newest = app.world.journal.len() - 1;
        assert_eq!(app.journal_rows(), [(newest, 0), (marked, 0)]);
        assert_eq!(app.world.hidden_inbox(), 2);
        // A card's screen still lists what was cleared.
        app.world.inbox[0].card = Some(0);
        app.card = Some(0);
        assert_eq!(app.inbox_rows(), [0]);
        app.card = None;
        // A closed one is not counted.
        app.world.inbox[1].open = false;
        assert_eq!(app.world.hidden_inbox(), 1);
        app.world.inbox[1].open = true;
        // `W` opens a cleared one and puts it back on the list.
        app.on_key(key(KeyCode::Char('W')));
        let at = app.waiting_at.expect("W opened one");
        assert!(!app.world.cleared.inbox.contains(&at));
        assert_eq!(app.inbox_rows()[app.inbox_sel], at);
        assert_eq!(app.world.hidden_inbox(), 1);
    }

    #[test]
    fn clear_one_side_undo_and_unclear_say_what_they_did() {
        let mut app = waiting(&[InboxKind::Reply]);
        app.world.tick += 1;
        app.world.note(Event::Failed { agent: 0 });
        let lines = app.journal_rows().len();
        app.command("clear inbox");
        app.command("undo");
        assert_eq!(said(&app), "the screen stays as it is");
        assert!(app.grace.is_none());
        assert_eq!(app.world.cleared, Cleared::default());
        app.command("clear INBOX");
        assert!(matches!(
            app.grace,
            Some((Undoable::Clear(Clear::Inbox), _))
        ));
        app.grace_due(Instant::now() + UNDO_GRACE);
        assert_eq!(
            said(&app),
            "1 waiting off the screen, still counted; :unclear brings them back"
        );
        assert_eq!(app.journal_rows().len(), lines, "the journal stays");
        app.command("clear inbox");
        assert_eq!(said(&app), "nothing on the screen to clear");
        assert!(app.grace.is_none());
        app.command("clear journal");
        assert!(matches!(
            app.grace,
            Some((Undoable::Clear(Clear::Journal), _))
        ));
        // Another clear acts on the one waiting first.
        app.command("clear");
        assert_eq!(said(&app), "nothing on the screen to clear");
        assert!(app.journal_rows().is_empty());
        assert_eq!(app.world.cleared.inbox, [0].into());
        app.command("clear everything");
        assert_eq!(said(&app), "everything is not inbox or journal");
        app.command("unclear journal");
        assert_eq!(
            said(&app),
            format!("back on the screen: 0 waiting, {lines} journal lines")
        );
        assert!(app.inbox_rows().is_empty(), "the inbox stays cleared");
        app.command("clear journal");
        app.grace_due(Instant::now() + UNDO_GRACE);
        assert_eq!(
            said(&app),
            format!("{lines} journal lines off the screen; :unclear brings them back")
        );
        app.command("unclear inbox");
        assert_eq!(said(&app), "back on the screen: 1 waiting, 0 journal lines");
        assert!(app.journal_rows().is_empty(), "the journal stays cleared");
        app.command("unclear");
        assert_eq!(
            said(&app),
            format!("back on the screen: 0 waiting, {lines} journal lines")
        );
        app.command("unclear");
        assert_eq!(said(&app), "nothing is cleared off the screen");
        app.command("unclear x");
        assert_eq!(said(&app), "x is not inbox or journal");
        assert_eq!(app.world.cleared, Cleared::default());
    }

    #[test]
    fn clears_words_are_read_and_each_has_its_place() {
        assert_eq!(Clear::parse(""), Some(Clear::All));
        assert_eq!(Clear::parse(" Journal "), Some(Clear::Journal));
        assert_eq!(Clear::parse("inbox"), Some(Clear::Inbox));
        assert_eq!(Clear::parse("all"), None);
        for (c, i, inbox, journal) in [
            (Clear::All, 0, true, true),
            (Clear::Inbox, 1, true, false),
            (Clear::Journal, 2, false, true),
        ] {
            assert_eq!((c.index(), c.inbox(), c.journal()), (i, inbox, journal));
        }
        assert!(CLEARS.iter().all(|w| Clear::parse(w).is_some()));
    }

    #[test]
    fn digest_shows_what_happened_since_the_person_last_left() {
        let mut app = App::new(1, 1, 0, 1, Lang::En);
        // Nothing yet this run.
        app.command("digest");
        assert!(app.digest.is_none());
        assert_eq!(said(&app), "nothing new since you last left");
        app.world.tick += 1;
        app.world.note(Event::Failed { agent: 0 });
        app.command("digest");
        let d = app.digest.take().expect("since the run began");
        assert_eq!(d.highlights.len(), 1);
        // Left and back at once: since leaving, not since the start.
        let t0 = Instant::now();
        app.focus(false, t0);
        app.world.tick += 1;
        app.world.note(Event::Failed { agent: 0 });
        app.world.note(Event::Failed { agent: 0 });
        app.focus(true, t0 + Duration::from_secs(1));
        assert!(app.digest.is_none(), "too short to open by itself");
        app.command("digest");
        let d = app.digest.take().expect("since leaving");
        assert_eq!(d.highlights.len(), 2);
        assert_eq!(d.minutes, SECONDS_PER_TICK / 60);
    }

    #[test]
    fn the_screen_is_drawn_on_a_change_never_on_a_timer() {
        let mut app = App::new(1, 1, 0, 1, Lang::En);
        let t0 = Instant::now();
        let s = Duration::from_secs;
        let ms = Duration::from_millis;
        assert!(app.draw_due(t0), "never drawn");
        app.drawn(t0);
        assert!(!app.draw_due(t0 + s(60)), "nothing changed: no timer");
        app.stirred = true;
        assert!(app.draw_due(t0), "a key, a resize");
        app.drawn(t0);
        assert!(!app.stirred, "drawn, stirred no more");
        app.changed = true;
        assert!(app.draw_due(t0), "news from an engine, at once");
        app.drawn(t0);
        assert!(!app.changed && !app.draw_due(t0));
        // The clock: the world's step, a flash coming and going, the
        // seconds a :close waits.
        app.world.tick += 1;
        assert!(app.draw_due(t0), "the world's clock");
        app.drawn(t0);
        let t1 = t0 + s(20);
        app.flash = Some(("x".into(), t1));
        assert!(app.draw_due(t1), "a flash");
        app.drawn(t1);
        assert!(!app.draw_due(t1 + FLASH - ms(1)), "still on show");
        assert!(app.draw_due(t1 + FLASH), "gone");
        app.drawn(t1 + FLASH);
        assert!(!app.draw_due(t1 + FLASH + s(9)));
        let t2 = t1 + s(20);
        app.grace = Some((Undoable::Close(0), t2 + UNDO_GRACE));
        assert!(app.draw_due(t2), "the count starts");
        app.drawn(t2);
        assert!(!app.draw_due(t2 + ms(999)), "10 s still");
        assert!(app.draw_due(t2 + s(1)), "9 s");
        // A step of the demo moves its world.
        app.drawn(t2 + s(1));
        app.step();
        assert!(app.changed && app.draw_due(t2 + s(1)));
    }

    #[test]
    fn another_window_draws_what_changed_once_a_second_and_a_key_at_once() {
        let mut app = App::new(1, 1, 0, 1, Lang::En);
        let t0 = Instant::now();
        let ms = Duration::from_millis;
        // Blurred before the first drawing: drawn.
        app.focus(false, t0);
        assert!(app.draw_due(t0));
        app.drawn(t0);
        assert!(!app.draw_due(t0 + BLURRED_DRAW), "nothing changed");
        app.changed = true;
        assert!(!app.draw_due(t0 + ms(1)));
        assert!(!app.draw_due(t0 + BLURRED_DRAW - ms(1)), "not yet");
        assert!(app.draw_due(t0 + BLURRED_DRAW), "a second on");
        app.drawn(t0 + BLURRED_DRAW);
        app.world.tick += 1;
        assert!(
            !app.draw_due(t0 + BLURRED_DRAW + ms(1)),
            "the clock waits too"
        );
        app.stirred = true;
        assert!(app.draw_due(t0 + BLURRED_DRAW + ms(1)), "a key, at once");
        app.drawn(t0 + BLURRED_DRAW + ms(1));
        app.changed = true;
        let t1 = t0 + BLURRED_DRAW + ms(2);
        assert!(!app.draw_due(t1));
        app.focus(true, t1);
        assert!(app.draw_due(t1), "back: what waited, at once");
    }

    #[test]
    fn a_live_step_draws_only_when_something_came() {
        let mut w = crate::live::testing::team(1);
        // A clock far from now's: the first step moves it.
        w.tick = 1_000;
        let live = crate::live::testing::idle_live(2);
        let mut app = App::with(w, Engine::Live(Box::new(live)), Lang::En);
        let t0 = Instant::now();
        app.step();
        assert!(app.changed, "the clock moved");
        // The file list the first step asked git for comes back first.
        let deadline = t0 + Duration::from_secs(60);
        while !matches!(&app.engine, Engine::Live(l) if crate::live::testing::git_quiet(l)) {
            assert!(Instant::now() < deadline, "git never answered");
            std::thread::sleep(Duration::from_millis(10));
            app.step();
        }
        // A step that leaves the clock where it was says what came.
        let step = |app: &mut App, said: Option<&str>| {
            for _ in 0..3 {
                app.step();
                app.drawn(t0);
                let before = app.world.tick;
                if let (Some(text), Engine::Live(l)) = (said, &app.engine) {
                    let e = imece_agent::Event::Text(text.into());
                    crate::live::testing::hear(l, 1, e);
                }
                app.step();
                if app.world.tick == before {
                    return app.changed;
                }
            }
            panic!("the clock moved on every try");
        };
        assert!(!step(&mut app, None), "nothing came");
        assert!(step(&mut app, Some("hello")), "a session's word");
        // Something waiting to be drawn still is after a quiet step, and
        // after one that brought more.
        app.changed = true;
        app.step();
        assert!(app.changed);
        if let Engine::Live(l) = &app.engine {
            crate::live::testing::hear(l, 1, imece_agent::Event::Text("more".into()));
        }
        app.step();
        assert!(app.changed);
    }

    #[test]
    fn a_vendor_s_answer_and_a_held_key_let_through_are_drawn() {
        let mut app = live_welcome(Vendor::Claude);
        app.start_welcome(true);
        app.looked();
        assert!(app.changed, "the answers");
        app.changed = false;
        app.looked();
        assert!(!app.changed, "nothing more came");
        app.changed = true;
        app.vendors[0].1 = Seen::Missing;
        app.input = ":welcome".into();
        app.typing = true;
        app.on_key(key(KeyCode::Enter));
        app.looked();
        assert!(app.changed, "still, with more");
        let mut app = App::new(1, 1, 0, 1, Lang::En);
        let now = Instant::now();
        app.keys_held = Some((key(KeyCode::Char('2')), now));
        app.keys_due(now);
        assert!(!app.changed, "still held");
        app.keys_due(now + keys::WAIT);
        assert!(app.changed && app.view == View::Board);
    }

    #[test]
    fn ctrl_o_goes_back_through_the_places_left_and_not_further() {
        let mut app = App::new(1, 1, 0, 1, Lang::En);
        let t = app.lang.text();
        let salon = app.place();
        app.on_key(key(KeyCode::Char('2')));
        let board = app.place();
        app.on_key(key(KeyCode::Char('3')));
        assert_eq!(app.been, [salon, board]);
        // A key that goes nowhere leaves nothing.
        app.on_key(key(KeyCode::Char('j')));
        assert_eq!(app.been.len(), 2);
        app.on_key(ctrl('o'));
        assert_eq!(app.place(), board);
        assert_eq!(app.been, [salon], "going back leaves nothing behind");
        app.on_key(ctrl('o'));
        assert_eq!(app.place(), salon);
        app.on_key(ctrl('o'));
        assert_eq!(app.place(), salon);
        assert_eq!(flashed(&app).as_deref(), Some(t.no_back));
        // The journal panel comes back with a row selected.
        app.pane = Pane::Journal;
        app.journal_sel = None;
        app.been.push(app.place());
        app.pane = Pane::Salon;
        app.on_key(ctrl('o'));
        assert_eq!(app.pane, Pane::Journal);
        assert!(app.journal_sel.is_some());
    }

    #[test]
    fn ctrl_o_closes_a_conversation_first_and_passes_over_places_gone() {
        let (mut app, worker) = with_change("diff --git a/x b/x\n@@ -1 +1 @@\n-a\n+b\n");
        app.zoom = Zoom::None;
        app.pane = Pane::Salon;
        let here = app.place();
        app.open_thread(0);
        app.on_key(ctrl('o'));
        assert!(app.thread.is_none());
        assert_eq!(app.place(), here);
        let at = |zoom, card| Place {
            view: View::Salon,
            zoom,
            pane: Pane::Inbox,
            card,
        };
        let lead = app.world.projects[0].leader;
        let cards = app.world.cards.len();
        app.been = vec![
            at(Zoom::None, Some(0)),
            at(Zoom::Focus(lead), None),
            at(Zoom::Diff(worker), None),
            here,
            at(Zoom::None, Some(cards)),
            at(Zoom::Diff(lead), None),
            at(Zoom::Trace(worker), None),
        ];
        app.world.agents[worker].gone = true;
        // The gone worker's stream, a change that is not there, a card
        // that is not, where it is already: passed over.
        app.on_key(ctrl('o'));
        assert_eq!(app.place(), at(Zoom::Diff(worker), None));
        app.on_key(ctrl('o'));
        assert_eq!(app.place(), at(Zoom::Focus(lead), None));
        app.on_key(ctrl('o'));
        assert_eq!(app.place(), at(Zoom::None, Some(0)), "a card that is there");
        assert!(app.been.is_empty());
    }

    #[test]
    fn the_places_left_are_kept_up_to_fifty_the_oldest_going_first() {
        let mut app = App::new(1, 1, 0, 1, Lang::En);
        for n in 0..60 {
            let c = ['2', '1'][n % 2];
            app.on_key(key(KeyCode::Char(c)));
        }
        assert_eq!(app.been.len(), 50);
        // The last place left was the board, the one before the salon.
        assert_eq!(app.been[49].view, View::Board);
        assert_eq!(app.been[48].view, View::Salon);
        assert_eq!(app.been[0].view, View::Salon);
    }

    #[test]
    fn next_of_is_the_first_after_or_the_first_of_all() {
        assert_eq!(next_of(&[1, 3], None), Some(1));
        assert_eq!(next_of(&[1, 3], Some(0)), Some(1));
        assert_eq!(next_of(&[1, 3], Some(1)), Some(3));
        assert_eq!(next_of(&[1, 3], Some(2)), Some(3));
        assert_eq!(next_of(&[1, 3], Some(3)), Some(1));
        assert_eq!(next_of(&[], Some(3)), None);
        assert_eq!(next_of(&[], None), None);
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
        // The person picks another row by hand: W goes on from there. The
        // focus W opened takes `j k gg G` itself: back to the panel first.
        let mut app = waiting(&[Reply, Sensitive, Sensitive, Sensitive]);
        app.on_key(key(KeyCode::Char('W')));
        app.on_key(key(KeyCode::Char('j')));
        assert_eq!(app.waiting_at, Some(0), "the focus stepped");
        app.on_key(key(KeyCode::Esc));
        app.on_key(key(KeyCode::Char('j')));
        assert_eq!(app.waiting_at, None);
        assert_eq!(press(&mut app, 'W'), Some(3), "after the picked 2");
        let mut app = waiting(&[Reply, Sensitive, Sensitive, Sensitive]);
        app.on_key(key(KeyCode::Char('W')));
        app.on_key(key(KeyCode::Esc));
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
            id: 0,
            re: None,
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
            copy: None,
            words: None,
            folds: Folds::default(),
            room: Cell::new(usize::MAX),
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
        app.end_grace();
        assert_eq!(app.card, None);
        // Another project closing leaves it open.
        app.home = None;
        app.command("open ~/x more");
        app.open_card(ours);
        app.command("close more");
        app.end_grace();
        assert_eq!(app.card, Some(ours));
    }

    /// `quiet()` with agent 3 a helper of worker 1 and agent 4 researching
    /// for it: worker 1's row stands for three.
    fn with_a_row_of_three() -> App {
        let mut app = quiet();
        let a = &mut app.world.agents;
        a[3].role = Role::Helper;
        a[3].parent = Some(1);
        a[4].role = Role::Researcher;
        a[4].status = Status::Researching {
            for_agent: 1,
            until: 9,
        };
        app
    }

    #[test]
    fn a_row_stands_for_its_agent_its_helpers_and_its_researcher() {
        let mut app = with_a_row_of_three();
        assert_eq!(app.row_group(1), [1, 3, 4]);
        assert_eq!(app.row_group(2), [2], "a worker of its own");
        assert_eq!(app.row_group(3), [3], "a helper has none under it");
        // A worker is no helper whatever its parent; a helper gone is
        // not there; a researcher for another is another's.
        app.world.agents[2].parent = Some(1);
        app.world.agents[3].gone = true;
        assert_eq!(app.row_group(1), [1, 4]);
        app.world.agents[4].status = Status::Researching {
            for_agent: 2,
            until: 9,
        };
        assert_eq!(app.row_group(1), [1]);
        assert_eq!(app.project_group(0), [0, 1, 2, 3, 4]);
        assert_eq!(app.project_group(1), Vec::<AgentId>::new());
    }

    #[test]
    fn s_settles_a_worker_row_out_of_the_salon_and_moves_on() {
        let mut app = with_a_row_of_three();
        app.selected = Sel::Agent(1);
        app.unseen.extend([1, 2, 3]);
        // Not from another pane.
        app.pane = Pane::Inbox;
        app.on_key(key(KeyCode::Char('s')));
        assert!(app.settled.is_empty());
        app.pane = Pane::Salon;
        app.on_key(key(KeyCode::Char('s')));
        assert_eq!(app.settled, BTreeSet::from([1]));
        assert!(app.settled_projects.is_empty());
        // Its row's turns are seen (done, it could go); another's not.
        assert_eq!(app.unseen, BTreeSet::from([2]));
        let name = app.world.agents[1].name.clone();
        assert_eq!(
            flashed(&app).unwrap_or_default(),
            format!("{name} settled: back when it needs you")
        );
        // Gone with its helper and researcher; on to the next row.
        assert_eq!(shown(&app), [Sel::Project(0), Sel::Agent(0), Sel::Agent(2)]);
        assert_eq!(app.selected, Sel::Agent(2));
        // The last row settled: to the nearest one before it.
        app.on_key(key(KeyCode::Char('s')));
        assert_eq!(app.settled, BTreeSet::from([1, 2]));
        assert_eq!(shown(&app), [Sel::Project(0), Sel::Agent(0)]);
        assert_eq!(app.selected, Sel::Agent(0));
        // `:settle` from elsewhere: the salon's selection.
        app.selected = Sel::Agent(3);
        app.view = View::Board;
        app.pane = Pane::Journal;
        app.command("settle");
        assert_eq!(app.settled, BTreeSet::from([1, 2, 3]));
        assert_eq!((app.view, app.pane), (View::Salon, Pane::Salon));
    }

    #[test]
    fn what_waits_for_you_or_is_stuck_is_not_settled() {
        let mut app = with_a_row_of_three();
        app.selected = Sel::Agent(1);
        // Its helper waits: the row stays.
        app.world.inbox.push(item(3, true));
        app.on_key(key(KeyCode::Char('s')));
        assert!(app.settled.is_empty());
        let helper = app.world.agents[3].name.clone();
        assert_eq!(
            flashed(&app).unwrap_or_default(),
            format!("not settled: {helper} needs you")
        );
        app.world.inbox.clear();
        app.world.agents[1].status = Status::Blocked;
        app.on_key(key(KeyCode::Char('s')));
        assert!(app.settled.is_empty());
        let worker = app.world.agents[1].name.clone();
        assert_eq!(
            flashed(&app).unwrap_or_default(),
            format!("not settled: {worker} needs you")
        );
        // A project with one agent that needs the person stays too.
        app.world.agents[1].status = Status::Idle;
        app.world.agents[2].status = Status::WaitingHuman;
        app.selected = Sel::Project(0);
        app.on_key(key(KeyCode::Char('s')));
        assert!(app.settled_projects.is_empty());
        let other = app.world.agents[2].name.clone();
        assert_eq!(
            flashed(&app).unwrap_or_default(),
            format!("not settled: {other} needs you")
        );
        assert_eq!(app.selected, Sel::Project(0));
    }

    #[test]
    fn s_on_a_header_or_its_leader_settles_the_project() {
        let (mut app, [_, _, c]) = two_quiet_projects();
        let lead = app.world.projects[1].leader;
        app.unseen.extend([lead, c]);
        app.selected = Sel::Agent(lead);
        app.on_key(key(KeyCode::Char('s')));
        assert_eq!(app.settled_projects, BTreeSet::from([1]));
        assert!(app.settled.is_empty() && app.unseen.is_empty());
        let rows = shown(&app);
        assert!(!rows.contains(&Sel::Project(1)) && !rows.contains(&Sel::Agent(c)));
        let name = app.world.projects[1].name.clone();
        assert_eq!(
            flashed(&app).unwrap_or_default(),
            format!("{name} settled: back when it needs you")
        );
        // Nothing after it: project 0's last row.
        assert_eq!(app.selected, *rows.last().unwrap());
        app.selected = Sel::Project(0);
        app.on_key(key(KeyCode::Char('s')));
        assert_eq!(app.settled_projects, BTreeSet::from([0, 1]));
        assert!(shown(&app).is_empty());
        assert_eq!(app.selected, Sel::Project(0), "nowhere to go: it stays");
    }

    #[test]
    fn what_is_settled_comes_back_when_it_needs_you() {
        let (mut app, [a, b, c]) = two_quiet_projects();
        app.settled.extend([a, b]);
        app.settled_projects.insert(1);
        app.wake_settled();
        assert_eq!(app.settled, BTreeSet::from([a, b]), "calm: they stay");
        assert_eq!(app.settled_projects, BTreeSet::from([1]));
        // A turn finished not seen brings a worker back; a project comes
        // back when any of its agents needs the person.
        app.unseen.insert(a);
        app.world.agents[c].status = Status::Blocked;
        app.wake_settled();
        assert_eq!(app.settled, BTreeSet::from([b]));
        assert!(app.settled_projects.is_empty());
        // Gone and closed are let go.
        app.world.agents[c].status = Status::Idle;
        app.settled_projects.insert(1);
        app.world.projects[1].closed = true;
        app.world.agents[b].gone = true;
        app.wake_settled();
        assert!(app.settled.is_empty() && app.settled_projects.is_empty());
        // What hangs under a row brings it back.
        let mut app = with_a_row_of_three();
        app.settled.insert(1);
        app.world.inbox.push(item(4, true));
        app.wake_settled();
        assert!(app.settled.is_empty());
    }

    #[test]
    fn unsettle_and_goto_bring_back_what_was_settled() {
        let mut app = with_a_row_of_three();
        app.settled.extend([1, 2]);
        app.settled_projects.insert(0);
        assert_eq!(app.command_note("unsettle"), "(3)");
        app.view = View::Board;
        app.pane = Pane::Inbox;
        app.command("unsettle");
        assert!(app.settled.is_empty() && app.settled_projects.is_empty());
        assert_eq!((app.view, app.pane), (View::Salon, Pane::Salon));
        assert_eq!(flashed(&app).unwrap_or_default(), "3 back in the salon");
        // To a helper: the worker's row it hangs under comes back, and
        // its project; another row stays settled.
        app.settled.extend([1, 2]);
        app.settled_projects.insert(0);
        let helper = app.world.agents[3].name.clone();
        app.command(&format!("goto {helper}"));
        assert_eq!(app.selected, Sel::Agent(3));
        assert_eq!(app.settled, BTreeSet::from([2]));
        assert!(app.settled_projects.is_empty());
        // To a project.
        app.settled_projects.insert(0);
        let project = app.world.projects[0].name.clone();
        app.command(&format!("goto {project}"));
        assert_eq!(app.selected, Sel::Project(0));
        assert!(app.settled_projects.is_empty());
    }

    #[test]
    fn the_colon_menu_names_what_settle_and_peek_act_on() {
        let mut app = quiet();
        app.selected = Sel::Agent(2);
        let name = app.world.agents[2].name.clone();
        assert_eq!(app.command_note("settle"), format!("({name})"));
        assert_eq!(app.command_note("peek"), format!("({name})"));
        assert_eq!(app.command_note("why"), format!("({name})"));
        app.selected = Sel::Project(0);
        let project = app.world.projects[0].name.clone();
        assert_eq!(app.command_note("settle"), format!("({project})"));
        assert_eq!(app.command_note("unsettle"), "(0)");
    }

    #[test]
    fn colon_why_says_the_rung_since_when_and_the_journal_line_that_put_it_there() {
        use crate::model::clock;
        let mut app = quiet();
        let (a, b) = (2, 3);
        let name = app.world.agents[a].name.clone();
        app.world.journal.clear();
        app.world.tick = 20;
        app.world.agents[a].changed_at = 10;
        // Nothing in the journal: said so.
        assert_eq!(app.why(a), (Rung::Idle, 10, None));
        app.command(&format!("why @{name}"));
        let unsaid = format!(
            "{name}: idle · since {} · the journal does not say why",
            clock(10)
        );
        assert_eq!(flashed(&app).unwrap_or_default(), unsaid);
        // Its newest line from then or before; not a later one, not
        // another agent's.
        let stopped = |agent, why: &str| Event::Stopped {
            agent,
            why: why.into(),
        };
        for (tick, e) in [
            (4, stopped(a, "old")),
            (10, stopped(a, "gone")),
            (11, stopped(b, "other")),
            (12, stopped(a, "later")),
        ] {
            app.world.journal.push(Entry { tick, event: e });
        }
        app.world.agents[a].status = Status::Stopped;
        assert_eq!(app.why(a), (Rung::Stuck, 10, Some(1)), "the line of then");
        app.world.agents[a].changed_at = 9;
        assert_eq!(app.why(a), (Rung::Stuck, 9, Some(0)), "the one before");
        app.world.agents[a].changed_at = 10;
        app.selected = Sel::Agent(a);
        app.command("why");
        let line = ui::words::event(Lang::En, &app.world, &app.world.journal[1].event);
        let said = format!("{name}: stuck · since {} · {line}", clock(10));
        assert_eq!(flashed(&app).unwrap_or_default(), said, "the agent at hand");
        // Waiting: since its oldest open item.
        app.world.agents[a].status = Status::Idle;
        app.world.inbox.push(InboxItem {
            kind: InboxKind::Question,
            agent: a,
            detail: "which?".into(),
            researcher: None,
            open: true,
            tick: 8,
            card: None,
        });
        assert_eq!(app.why(a), (Rung::Waiting, 8, Some(0)));
        // In Turkish; a name nobody has.
        app.lang = Lang::Tr;
        app.command(&format!("why {name}"));
        let line = ui::words::event(Lang::Tr, &app.world, &app.world.journal[0].event);
        let said = format!(
            "{name}: onayını bekliyor · {} itibarıyla · {line}",
            clock(8)
        );
        assert_eq!(flashed(&app).unwrap_or_default(), said);
        app.command("why nobody");
        assert_eq!(
            flashed(&app).unwrap_or_default(),
            "\"nobody\" adında bir agent yok"
        );
    }

    #[test]
    fn every_rung_has_its_words_in_both_languages() {
        let app = quiet();
        let name = &app.world.agents[0].name;
        for (rung, en, tr) in [
            (Rung::Idle, "idle", "boşta"),
            (Rung::Done, "done, not seen", "bitti, bakılmadı"),
            (Rung::Starting, "starting", "başlıyor"),
            (Rung::Working, "working", "çalışıyor"),
            (Rung::Waiting, "waiting for you", "onayını bekliyor"),
            (Rung::Stuck, "stuck", "takıldı"),
        ] {
            let said = |lang| ui::words::why(lang, &app.world, 0, (rung, 0, None));
            assert!(
                said(Lang::En).starts_with(&format!("{name}: {en} · ")),
                "{rung:?}"
            );
            assert!(
                said(Lang::Tr).starts_with(&format!("{name}: {tr} · ")),
                "{rung:?}"
            );
        }
    }

    #[test]
    fn k_peeks_at_the_agent_at_hand_over_the_view_and_j_k_step() {
        let mut app = quiet();
        app.view = View::Board;
        app.selected = Sel::Agent(2);
        app.unseen.insert(2);
        app.on_key(key(KeyCode::Char('K')));
        assert_eq!(app.peek, Some(2));
        assert!(!app.unseen.contains(&2), "looked at: seen");
        assert_eq!((app.view, app.zoom), (View::Board, Zoom::None));
        // The salon's order: the lead, then the workers.
        let mut steps = Vec::new();
        for code in [
            KeyCode::Char('j'),
            KeyCode::Down,
            KeyCode::Char('j'),
            KeyCode::Char('k'),
            KeyCode::Up,
            KeyCode::Up,
            KeyCode::Char('k'),
            KeyCode::Char('k'),
        ] {
            app.on_key(key(code));
            steps.push(app.peek.unwrap());
        }
        assert_eq!(steps, [3, 4, 4, 3, 2, 1, 0, 0]);
        // Other keys wait: no chord, no quit question, the view stays.
        for c in ['g', 'q', '2', 's', 'f'] {
            app.on_key(key(KeyCode::Char(c)));
        }
        assert_eq!(app.pending, None);
        assert_eq!(app.view, View::Board);
        assert!(app.settled.is_empty() && !app.needs_only);
        assert_eq!(app.peek, Some(0));
        // The help over it; closing the help leaves the peek.
        app.on_key(key(KeyCode::Char('?')));
        assert!(app.help);
        app.on_key(key(KeyCode::Esc));
        assert!(!app.help);
        assert_eq!(app.peek, Some(0));
        app.on_key(key(KeyCode::Esc));
        assert_eq!(app.peek, None);
        app.on_key(key(KeyCode::Char('K')));
        assert_eq!(app.peek, Some(2));
        app.on_key(key(KeyCode::Char('K')));
        assert_eq!(app.peek, None);
        // enter: its whole focus, over the same view.
        app.on_key(key(KeyCode::Char('K')));
        app.on_key(key(KeyCode::Enter));
        assert_eq!((app.peek, app.zoom), (None, Zoom::Focus(2)));
        assert_eq!(app.view, View::Board);
        // @: a message to it.
        app.on_key(key(KeyCode::Esc));
        app.on_key(key(KeyCode::Char('K')));
        app.on_key(key(KeyCode::Char('@')));
        let name = app.world.agents[2].name.clone();
        assert_eq!(app.peek, None);
        assert!(app.typing);
        assert_eq!(app.input, format!("@{name} "));
    }

    #[test]
    fn a_peek_off_the_salons_rows_steps_to_its_first_agent() {
        let (mut app, [a, ..]) = two_quiet_projects();
        let lead = app.world.projects[1].leader;
        app.collapsed[0] = true;
        app.peek = Some(a);
        app.on_key(key(KeyCode::Char('j')));
        assert_eq!(app.peek, Some(lead));
        // No agent shown at all: it stays where it is.
        app.collapsed[1] = true;
        app.peek = Some(a);
        app.on_key(key(KeyCode::Char('k')));
        assert_eq!(app.peek, Some(a));
    }

    #[test]
    fn colon_peek_names_the_agent_or_says_there_is_none() {
        let mut app = quiet();
        let name = app.world.agents[3].name.clone();
        app.command(&format!("peek @{name}"));
        assert_eq!(app.peek, Some(3));
        app.peek = None;
        app.command("peek nobody");
        assert_eq!(app.peek, None);
        assert_eq!(
            flashed(&app).unwrap_or_default(),
            "no agent named \"nobody\""
        );
    }

    #[test]
    fn hint_labels_are_one_letter_while_they_go_round_and_two_from_the_last_letters() {
        let all: Vec<String> = HINT_LETTERS
            .iter()
            .map(|&b| char::from(b).to_string())
            .collect();
        assert!(hint_labels(0).is_empty());
        assert_eq!(hint_labels(1), ["a"]);
        assert_eq!(hint_labels(26), all);
        // One more: the last letter starts labels of two instead.
        let l = hint_labels(27);
        assert_eq!(l[..25], all[..25]);
        assert_eq!(l[25..], ["ma", "ms"]);
        // One start of two holds 25 + 26 rows; the next row needs another.
        let l = hint_labels(51);
        assert_eq!((l.len(), l[24].as_str(), l[50].as_str()), (51, "n", "mm"));
        let l = hint_labels(52);
        assert_eq!(l.len(), 52);
        assert_eq!(l[..24], all[..24]);
        assert_eq!((l[24].as_str(), l[49].as_str()), ("na", "nm"));
        assert_eq!(l[50..], ["ma", "ms"]);
        // All of two letters, and no more than there are.
        let l = hint_labels(676);
        assert_eq!((l.len(), l[0].as_str(), l[675].as_str()), (676, "aa", "mm"));
        assert_eq!(hint_labels(1000).len(), 676);
        for n in [27, 52, 300, 676] {
            let l = hint_labels(n);
            let unique: BTreeSet<&String> = l.iter().collect();
            assert_eq!(unique.len(), n, "{n}: one label per row");
            for a in &l {
                for b in &l {
                    assert!(a == b || !b.starts_with(a.as_str()), "{n}: {a} starts {b}");
                }
            }
        }
    }

    /// A quiet team whose salon showed its header and its agents, two
    /// open items of WAITING FOR YOU (after a closed one) and two journal
    /// lines, as a drawing leaves them for `F`.
    fn drawn_for_hints() -> (App, Vec<Target>) {
        let mut app = quiet();
        app.world.inbox.push(item(1, false));
        for agent in [2, 3] {
            app.world.inbox.push(InboxItem {
                kind: InboxKind::Approval,
                ..item(agent, true)
            });
        }
        let lines: Vec<usize> = app.journal_rows().iter().map(|&(i, _)| i).take(2).collect();
        assert_eq!(lines.len(), 2, "the demo's start has journal lines");
        let rows = [Sel::Project(0), Sel::Agent(0), Sel::Agent(1), Sel::Agent(2)];
        *app.shown.borrow_mut() = rows.to_vec();
        let mut targets: Vec<Target> = rows.into_iter().map(Target::Row).collect();
        targets.extend([Target::Waiting(1), Target::Waiting(2)]);
        targets.extend(lines.into_iter().map(Target::Line));
        *app.hintable.borrow_mut() = (targets.iter().zip(0..)).map(|(&t, y)| (t, 1, y)).collect();
        (app, targets)
    }

    fn shift(c: char) -> KeyEvent {
        KeyEvent::new(KeyCode::Char(c), KeyModifiers::SHIFT)
    }

    #[test]
    fn f_labels_the_rows_drawn_and_a_label_selects_its_row_in_its_panel() {
        let (mut app, targets) = drawn_for_hints();
        app.on_key(shift('F'));
        let h = app.hints.as_ref().expect("F gives the labels");
        let labels: Vec<(&str, Target)> = h.labels.iter().map(|(l, t)| (l.as_str(), *t)).collect();
        let want: Vec<(&str, Target)> = ["a", "s", "d", "f", "g", "h", "j", "k"]
            .into_iter()
            .zip(targets.iter().copied())
            .collect();
        assert_eq!(labels, want);
        assert_eq!(h.typed, "");
        // An agent's row: selected, not opened.
        app.on_key(key(KeyCode::Char('d')));
        assert!(app.hints.is_none());
        assert_eq!(
            (app.pane, app.selected, app.zoom),
            (Pane::Salon, Sel::Agent(1), Zoom::None)
        );
        // An item: its place among the open ones, and `W` goes on from it.
        app.waiting_at = Some(0);
        app.on_key(shift('F'));
        app.on_key(key(KeyCode::Char('h')));
        assert_eq!(
            (app.pane, app.inbox_sel, app.waiting_at),
            (Pane::Inbox, 1, None)
        );
        assert_eq!(app.zoom, Zoom::None);
        // A journal line.
        let Target::Line(line) = targets[7] else {
            unreachable!()
        };
        app.on_key(shift('F'));
        app.on_key(key(KeyCode::Char('k')));
        assert_eq!((app.pane, app.journal_sel), (Pane::Journal, Some(line)));
        // A header: back in the salon.
        app.on_key(shift('F'));
        app.on_key(key(KeyCode::Char('a')));
        assert_eq!((app.pane, app.selected), (Pane::Salon, Sel::Project(0)));
        assert!(app.flash.is_none(), "nothing to say");
    }

    #[test]
    fn a_label_s_last_letter_in_capitals_opens_its_row_too() {
        let (mut app, _) = drawn_for_hints();
        app.on_key(shift('F'));
        app.on_key(shift('S'));
        assert_eq!((app.pane, app.zoom), (Pane::Salon, Zoom::Focus(0)));
        app.on_key(key(KeyCode::Esc));
        app.on_key(shift('F'));
        app.on_key(shift('H'));
        assert_eq!((app.pane, app.inbox_sel), (Pane::Inbox, 1));
        assert_eq!(app.zoom, Zoom::Focus(3), "the ask's agent");
    }

    #[test]
    fn labels_of_two_letters_narrow_and_only_the_last_letter_opens() {
        let (mut app, _) = drawn_for_hints();
        let agents: Vec<Sel> = (0..30).map(|_| Sel::Agent(4)).collect();
        // Thirty rows: the last letter, `m`, starts labels of two.
        let mut at: Vec<(Target, u16, u16)> = vec![(Target::Row(Sel::Agent(0)), 1, 0); 25];
        at.extend([
            (Target::Row(Sel::Agent(4)), 1, 25),
            (Target::Row(Sel::Agent(3)), 1, 26),
        ]);
        at.extend(vec![(Target::Row(Sel::Agent(0)), 1, 27); 3]);
        *app.hintable.borrow_mut() = at;
        app.shown.borrow_mut().extend(agents);
        app.shown.borrow_mut().push(Sel::Agent(3));
        app.on_key(shift('F'));
        app.on_key(shift('M'));
        assert_eq!(
            app.hints.as_ref().map(|h| h.typed.as_str()),
            Some("m"),
            "a capital first letter narrows"
        );
        // A letter no label goes on does nothing; backspace takes one back.
        for c in ['m', 'q', '1', ' '] {
            app.on_key(key(KeyCode::Char(c)));
        }
        app.on_key(key(KeyCode::Enter));
        assert_eq!(app.hints.as_ref().map(|h| h.typed.as_str()), Some("m"));
        app.on_key(key(KeyCode::Backspace));
        assert_eq!(app.hints.as_ref().map(|h| h.typed.as_str()), Some(""));
        app.on_key(key(KeyCode::Backspace));
        assert_eq!(
            app.hints.as_ref().map(|h| h.typed.as_str()),
            Some(""),
            "nothing left to take"
        );
        app.on_key(key(KeyCode::Char('m')));
        app.on_key(key(KeyCode::Char('s')));
        assert!(app.hints.is_none());
        assert_eq!(
            (app.selected, app.zoom),
            (Sel::Agent(3), Zoom::None),
            "ms: selected only"
        );
        assert_eq!(app.view, View::Salon);
        assert_eq!(app.pending, None, "no quit question from q");
    }

    #[test]
    fn esc_takes_the_labels_away_and_the_help_comes_over_them() {
        let (mut app, _) = drawn_for_hints();
        app.on_key(shift('F'));
        app.on_key(key(KeyCode::Char('?')));
        assert!(app.help && app.hints.is_some());
        app.on_key(key(KeyCode::Esc));
        assert!(!app.help && app.hints.is_some(), "the help closes first");
        app.on_key(key(KeyCode::Esc));
        assert!(app.hints.is_none());
        assert_eq!((app.pane, app.selected), (Pane::Salon, Sel::Project(0)));
        // `:hints` gives them too.
        app.command("hints");
        assert_eq!(app.hints.as_ref().map(|h| h.labels.len()), Some(8));
    }

    #[test]
    fn a_label_whose_row_left_the_screen_is_said_so() {
        let (mut app, targets) = drawn_for_hints();
        let gone = Some(app.lang.text().hint_gone.to_string());
        // The agent's row no longer shown.
        app.shown.borrow_mut().retain(|&s| s != Sel::Agent(1));
        app.on_key(shift('F'));
        app.on_key(shift('D'));
        assert_eq!((app.selected, app.zoom), (Sel::Project(0), Zoom::None));
        assert_eq!(flashed(&app), gone);
        // The item answered.
        app.flash = None;
        app.world.inbox[2].open = false;
        app.on_key(shift('F'));
        app.on_key(key(KeyCode::Char('h')));
        assert_eq!(app.pane, Pane::Salon);
        assert_eq!(flashed(&app), gone);
        // A line the journal does not show.
        app.flash = None;
        let past = app.world.journal.len();
        assert!(targets.iter().all(|&t| t != Target::Line(past)));
        app.hints = Some(Hints {
            labels: vec![("a".into(), Target::Line(past))],
            typed: String::new(),
        });
        app.on_key(key(KeyCode::Char('a')));
        assert_eq!((app.pane, app.journal_sel), (Pane::Salon, None));
        assert_eq!(flashed(&app), gone);
    }

    #[test]
    fn f_has_no_labels_off_the_salons_screen() {
        let (mut app, _) = drawn_for_hints();
        let none = Some(app.lang.text().no_hints.to_string());
        app.zoom = Zoom::Focus(1);
        app.on_key(shift('F'));
        assert!(app.hints.is_none());
        assert_eq!(flashed(&app), none);
        app.zoom = Zoom::None;
        app.view = View::Board;
        app.flash = None;
        app.on_key(shift('F'));
        assert!(app.hints.is_none());
        assert_eq!(flashed(&app), none);
        app.view = View::Salon;
        app.open_thread(app.journal_rows()[0].0);
        app.flash = None;
        app.command("hints");
        assert!(app.hints.is_none(), "a conversation lies over the rows");
        assert_eq!(flashed(&app), none);
        app.thread = None;
        app.hintable.borrow_mut().clear();
        app.flash = None;
        app.on_key(shift('F'));
        assert!(app.hints.is_none(), "nothing drawn to label");
        assert_eq!(flashed(&app), none);
    }

    fn keymap(text: &str) -> Keymap {
        Keymap::read(text).unwrap()
    }

    #[test]
    fn the_key_file_turns_keys_but_not_writing_the_help_filter_or_hints() {
        let (mut app, _) = drawn_for_hints();
        app.keys = keymap("^n = 2\nd = 3\ne = /");
        app.on_key(ctrl('n'));
        assert_eq!(app.view, View::Board, "^n is 2");
        app.on_key(key(KeyCode::Char('d')));
        assert_eq!(app.view, View::Code, "d is 3");
        app.on_key(key(KeyCode::Char('1')));
        assert_eq!(app.view, View::Salon, "a key not named is itself");
        // Writing takes the letter.
        app.on_key(key(KeyCode::Char('/')));
        app.on_key(key(KeyCode::Char('d')));
        assert_eq!(app.input, "d");
        app.on_key(key(KeyCode::Esc));
        assert!(!app.typing);
        // So does the help's filter, but the help itself turns keys.
        app.on_key(key(KeyCode::Char('?')));
        app.on_key(key(KeyCode::Char('/')));
        app.on_key(key(KeyCode::Char('d')));
        assert_eq!(app.help_find, "d");
        app.on_key(key(KeyCode::Enter));
        assert!(app.help && !app.help_typing);
        app.on_key(key(KeyCode::Char('e')));
        assert!(app.help_typing, "e is /: the filter again, not closed");
        app.on_key(key(KeyCode::Char('e')));
        assert_eq!(app.help_find, "de", "and then a letter");
        app.on_key(key(KeyCode::Enter));
        app.on_key(key(KeyCode::Char('d')));
        assert!(!app.help, "any key closes it");
        assert_eq!(app.view, View::Salon, "and does nothing else");
        // A hint letter is a letter.
        app.on_key(shift('F'));
        assert!(app.hints.is_some());
        app.on_key(key(KeyCode::Char('d')));
        assert!(app.hints.is_none(), "d was a label");
        assert_eq!(app.view, View::Salon);
        assert_eq!(app.selected, Sel::Agent(1), "the third row");
        // ctrl-c quits whatever the file says.
        app.on_key(ctrl('c'));
        assert!(app.quit);
    }

    #[test]
    fn a_turned_key_acts_with_its_ctrl() {
        let mut app = App::new(1, 4, 0, 1, Lang::En);
        app.keys = keymap("x = ^o");
        app.on_key(key(KeyCode::Char('2')));
        app.on_key(key(KeyCode::Char('x')));
        assert_eq!(app.view, View::Salon, "x is ^o: back where the person was");
    }

    /// A demo app whose salon shows three rows.
    fn three_rows() -> App {
        let app = quiet();
        *app.shown.borrow_mut() = vec![Sel::Project(0), Sel::Agent(0), Sel::Agent(1)];
        app
    }

    #[test]
    fn two_keys_of_the_person_act_as_one() {
        let mut app = three_rows();
        app.keys = keymap("2x = 3\ngt = 2");
        app.on_key(key(KeyCode::Char('2')));
        assert_eq!(app.view, View::Salon, "2 waits for its second");
        assert!(app.keys_held.is_some());
        app.on_key(key(KeyCode::Char('x')));
        assert_eq!(app.view, View::Code, "2x is 3");
        assert!(app.keys_held.is_none());
        // Another key: the first acts alone, then the other.
        app.on_key(key(KeyCode::Char('1')));
        app.on_key(key(KeyCode::Char('2')));
        app.on_key(key(KeyCode::Char('?')));
        assert_eq!(app.view, View::Board, "2 alone is 2");
        assert!(app.help, "and ? acted after it");
        assert!(app.keys_held.is_none());
        app.on_key(key(KeyCode::Esc));
        app.on_key(key(KeyCode::Char('1')));
        let top = app.selected;
        // g waits too, but imece's gg is not kept waiting.
        app.on_key(key(KeyCode::Char('G')));
        let bottom = app.selected;
        assert_ne!(bottom, top);
        app.on_key(key(KeyCode::Char('g')));
        assert_eq!(app.pending, None, "held, not imece's yet");
        app.on_key(key(KeyCode::Char('g')));
        assert_eq!(app.selected, top, "gg at once");
        assert!(
            app.keys_held.is_none(),
            "the second g went to imece's chord"
        );
        app.on_key(key(KeyCode::Char('g')));
        app.on_key(key(KeyCode::Char('t')));
        assert_eq!(app.view, View::Board, "gt is 2");
        assert_eq!(app.pending, None);
    }

    #[test]
    fn a_first_key_alone_acts_after_its_wait() {
        let mut app = App::new(1, 4, 0, 1, Lang::En);
        app.keys = keymap("2x = 3");
        app.keys_due(Instant::now());
        app.on_key(key(KeyCode::Char('2')));
        let (_, at) = app.keys_held.unwrap();
        app.keys_due(at + keys::WAIT - Duration::from_millis(1));
        assert_eq!(app.view, View::Salon, "still waiting");
        app.keys_due(at + keys::WAIT);
        assert_eq!(app.view, View::Board, "2 alone");
        assert!(app.keys_held.is_none());
        app.on_key(ctrl('o'));
        assert_eq!(app.view, View::Salon, "the place it left is behind");
        app.keys_due(at + keys::WAIT * 3);
        assert_eq!(app.view, View::Salon, "nothing held, nothing to do");
    }

    #[test]
    fn one_key_of_the_person_acts_as_two() {
        let mut app = three_rows();
        let top = app.selected;
        app.keys = keymap("x = gg\ny = :q\nG = j");
        app.on_key(key(KeyCode::Char('G')));
        assert_ne!(app.selected, top, "G is j");
        app.on_key(key(KeyCode::Char('x')));
        assert_eq!(app.selected, top, "x is gg");
        assert_eq!(app.pending, None);
        // The second goes where the first left the screen: writing.
        app.on_key(key(KeyCode::Char('y')));
        assert!(app.typing);
        assert_eq!(app.input, ":q");
    }

    #[test]
    fn the_key_file_s_place_is_where_the_screen_is() {
        use keys::Scope;
        let mut app = App::new(1, 1, 0, 1, Lang::En);
        assert_eq!(app.key_scope(), Some(Scope::Salon));
        app.view = View::Board;
        assert_eq!(app.key_scope(), Some(Scope::Board));
        app.view = View::Code;
        assert_eq!(app.key_scope(), Some(Scope::Code));
        app.view = View::Salon;
        app.help = true;
        assert_eq!(app.key_scope(), Some(Scope::Help));
        assert_eq!(app.scope_below(), Some(Scope::Salon), "where ? was pressed");
        app.help = false;
        app.peek = Some(0);
        assert_eq!(app.key_scope(), Some(Scope::Peek));
        app.peek = None;
        // A window or a question over the screen has none.
        for over in [
            |a: &mut App| a.leaving = true,
            |a: &mut App| a.welcome = true,
            |a: &mut App| a.news = vec!["new"],
            |a: &mut App| a.digest = Some(Digest::default()),
            |a: &mut App| a.left_off = Some(LeftOff::default()),
            |a: &mut App| a.solo_pick = Some(SoloPick::new(None, None, Vec::new())),
            |a: &mut App| a.archives = Some(Browser::new(&a.world, Vec::new())),
        ] {
            let mut app = App::new(1, 1, 0, 1, Lang::En);
            over(&mut app);
            assert_eq!(app.key_scope(), None);
            app.help = true;
            assert_eq!(app.key_scope(), None, "the window takes the key first");
        }
        let (mut app, worker) = with_change("+one\n+two\n");
        assert_eq!(app.key_scope(), Some(Scope::Change));
        app.on_key(key(KeyCode::Char('v')));
        assert!(app.diff_copy_of(worker).is_some());
        assert_eq!(app.key_scope(), Some(Scope::Choose));
        let (mut app, _) = with_thread();
        assert_eq!(app.key_scope(), Some(Scope::Talk));
        app.zoom = Zoom::Diff(0);
        assert_eq!(
            app.key_scope(),
            Some(Scope::Talk),
            "the conversation over it"
        );
        app.thread.as_mut().unwrap().copy = Some(Copying::new(0));
        assert_eq!(app.key_scope(), Some(Scope::Choose));
    }

    #[test]
    fn a_place_s_keys_hold_only_there() {
        let mut app = three_rows();
        app.keys = keymap("d = 3\n[board]\nd = 1\ngt = 3\n[code]\ne = 2");
        app.on_key(key(KeyCode::Char('d')));
        assert_eq!(app.view, View::Code, "d is 3 where no place says else");
        app.on_key(key(KeyCode::Char('e')));
        assert_eq!(app.view, View::Board, "e is 2 in the code map");
        app.on_key(key(KeyCode::Char('d')));
        assert_eq!(app.view, View::Salon, "d is 1 on the board");
        app.on_key(key(KeyCode::Char('e')));
        assert_eq!(app.view, View::Salon, "e is nothing in the salon");
        app.on_key(key(KeyCode::Char('g')));
        assert!(app.keys_held.is_none(), "gt is the board's");
        app.on_key(key(KeyCode::Char('2')));
        assert_eq!(app.view, View::Salon, "g2 is no chord");
        app.on_key(key(KeyCode::Char('2')));
        app.on_key(key(KeyCode::Char('g')));
        assert!(app.keys_held.is_some(), "the board's g waits");
        app.keys_due(Instant::now() + keys::WAIT * 2);
        assert_eq!(app.pending, Some('g'), "and goes alone as the board's");
        app.on_key(key(KeyCode::Char('g')));
        app.on_key(key(KeyCode::Char('g')));
        app.on_key(key(KeyCode::Char('t')));
        assert_eq!(app.view, View::Code, "gt is 3 on the board");
        assert_eq!(app.command_note("keys"), "(4)", "every line counts");
    }

    #[test]
    fn a_held_key_that_starts_writing_leaves_the_next_one_as_text() {
        let mut app = App::new(1, 4, 0, 1, Lang::En);
        app.keys = keymap("/x = 3\n2y = 1");
        app.on_key(key(KeyCode::Char('/')));
        assert!(!app.typing, "/ waits");
        app.on_key(key(KeyCode::Char('2')));
        assert!(app.typing, "/ alone writes");
        assert_eq!(app.input, "2", "and 2 is a letter, not held");
        assert!(app.keys_held.is_none());
        app.on_key(key(KeyCode::Char('x')));
        assert_eq!(app.input, "2x", "writing is never turned");
    }

    fn dir(name: &str) -> PathBuf {
        let dir =
            std::env::temp_dir().join(format!("imece-app-keys-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn colon_keys_reads_the_file_again_and_a_bad_one_leaves_the_keys() {
        let dir = dir("read");
        let path = dir.join("keys");
        let mut app = App::new(1, 1, 0, 1, Lang::En);
        app.command("keys");
        assert_eq!(
            flashed(&app).as_deref(),
            Some("no home folder, so no key file")
        );
        app.keys_file = Some(path.clone());
        let shown = path.display().to_string();
        app.command("keys");
        assert_eq!(
            flashed(&app),
            Some(format!(
                "no key file; KEY = KEY lines in {shown} give you keys of your own"
            ))
        );
        assert!(!app.keys_new, "nothing was kept, nothing to keep");
        assert_eq!(app.command_note("keys"), "(0)");
        std::fs::write(&path, "^n = j\nj = G\n").unwrap();
        app.command("keys");
        assert_eq!(flashed(&app), Some(format!("{shown}: keys taken (2)")));
        assert_eq!(app.keys, keymap("^n = j\nj = G"));
        assert_eq!(app.keys_good.as_deref(), Some("^n = j\nj = G\n"));
        assert!(std::mem::take(&mut app.keys_new), "to keep");
        assert_eq!(app.command_note("keys"), "(2)");
        app.command("keys");
        assert!(!app.keys_new, "the same file again: nothing new to keep");
        std::fs::write(&path, "^n = j\nnope\n").unwrap();
        app.command("keys");
        assert_eq!(
            flashed(&app),
            Some(format!(
                "{shown}: line 2: write it as KEY = KEY; the keys you had stay"
            ))
        );
        assert_eq!(app.keys, keymap("^n = j\nj = G"), "they stay");
        assert_eq!(app.keys_good.as_deref(), Some("^n = j\nj = G\n"));
        assert!(!app.keys_new);
        std::fs::remove_file(&path).unwrap();
        app.command("keys");
        assert_eq!(app.keys, Keymap::default(), "no file: imece's own keys");
        assert_eq!(app.keys_good, None);
        assert!(app.keys_new, "forgotten for the next start too");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn a_start_says_why_the_key_file_was_refused() {
        let mut app = App::new(1, 1, 0, 1, Lang::En);
        app.keys_new = true;
        let path = PathBuf::from("/c/imece/keys");
        let start = keys::Start {
            map: keymap("^p = k"),
            good: Some("^p = k".into()),
            refused: Some(keys::Why::Quits(2)),
        };
        app.start_keys(Some(path.clone()), keys::Stamp::default(), start);
        assert_eq!(app.keys, keymap("^p = k"));
        assert_eq!(app.keys_good.as_deref(), Some("^p = k"));
        assert_eq!(app.keys_file, Some(path.clone()));
        assert!(!app.keys_new, "main keeps what a start found");
        assert_eq!(
            flashed(&app).as_deref(),
            Some("/c/imece/keys: line 2: ctrl-c always quits; the keys you had stay")
        );
        let mut app = App::new(1, 1, 0, 1, Lang::En);
        app.start_keys(Some(path), keys::Stamp::default(), keys::Start::default());
        assert_eq!(flashed(&app), None, "a good start says nothing");
        let mut app = App::new(1, 1, 0, 1, Lang::En);
        let start = keys::Start {
            refused: Some(keys::Why::TooBig),
            ..keys::Start::default()
        };
        app.start_keys(None, keys::Stamp::default(), start);
        assert_eq!(flashed(&app), None, "no file to name");
    }

    #[test]
    fn a_changed_key_file_is_read_again_every_few_seconds() {
        let dir = dir("watch");
        let path = dir.join("keys");
        let shown = path.display().to_string();
        std::fs::write(&path, "^n = j\n").unwrap();
        let mut app = App::new(1, 1, 0, 1, Lang::En);
        let seen = keys::stamp(&path);
        let start = keys::Start::new(Some(keys::load(&path)), None);
        app.start_keys(Some(path.clone()), seen, start);
        let at = Instant::now();
        let after = |n: u32| at + keys::EVERY * n;
        app.watch_keys(at);
        assert_eq!(flashed(&app), None, "as the start read it: not read");
        assert!(!app.keys_new);
        std::fs::write(&path, "^n = j\nj = G\n").unwrap();
        app.watch_keys(after(1) - Duration::from_millis(1));
        assert_eq!(app.keys, keymap("^n = j"), "not looked at yet");
        app.watch_keys(after(1));
        assert_eq!(app.keys, keymap("^n = j\nj = G"), "looked at, read again");
        assert_eq!(flashed(&app), Some(format!("{shown}: keys taken (2)")));
        assert_eq!(app.keys_good.as_deref(), Some("^n = j\nj = G\n"));
        assert!(std::mem::take(&mut app.keys_new), "to keep");
        app.flash = None;
        app.watch_keys(after(2));
        assert_eq!(flashed(&app), None, "read once for each change");
        std::fs::write(&path, "^n = j\nnope\n").unwrap();
        app.watch_keys(after(3));
        assert_eq!(
            flashed(&app),
            Some(format!(
                "{shown}: line 2: write it as KEY = KEY; the keys you had stay"
            ))
        );
        assert_eq!(app.keys, keymap("^n = j\nj = G"), "a bad edit: they stay");
        assert!(!app.keys_new);
        app.flash = None;
        app.watch_keys(after(4));
        assert_eq!(flashed(&app), None, "said once, not every few seconds");
        std::fs::remove_file(&path).unwrap();
        app.watch_keys(after(5));
        assert_eq!(app.keys, Keymap::default(), "removed: imece's own keys");
        assert!(std::mem::take(&mut app.keys_new));
        std::fs::write(&path, "^p = k\n").unwrap();
        app.watch_keys(after(6));
        assert_eq!(app.keys, keymap("^p = k"), "made again");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn back_in_the_window_the_key_file_is_looked_at_at_once() {
        let dir = dir("focus");
        let path = dir.join("keys");
        let mut app = App::new(1, 1, 0, 1, Lang::En);
        app.start_keys(
            Some(path.clone()),
            keys::Stamp::default(),
            keys::Start::default(),
        );
        let at = Instant::now();
        app.watch_keys(at);
        assert_eq!(flashed(&app), None, "no file, as at the start");
        app.focus(false, at);
        std::fs::write(&path, "^n = j\n").unwrap();
        let back = at + Duration::from_millis(5);
        app.focus(true, back);
        app.watch_keys(back);
        assert_eq!(app.keys, keymap("^n = j"));
        std::fs::write(&path, "^n = j\nj = G\n").unwrap();
        app.command("keys");
        assert_eq!(app.keys, keymap("^n = j\nj = G"));
        app.flash = None;
        app.focus(true, back);
        app.watch_keys(back);
        assert_eq!(flashed(&app), None, "`:keys` read it: nothing new");
        let mut app = App::new(1, 1, 0, 1, Lang::En);
        app.watch_keys(at);
        assert_eq!(flashed(&app), None, "no home, no file to look at");
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// A demo of one project and one worker, everyone idle, motion `m`;
    /// the worker's id.
    fn still(m: Motion) -> (App, AgentId) {
        let mut app = App::new(1, 1, 0, 1, Lang::En);
        for a in &mut app.world.agents {
            a.status = Status::Idle;
            a.starting = false;
        }
        app.world.inbox.clear();
        app.motion = m;
        let w = (app.world.agents.iter())
            .find(|a| a.role == Role::Worker)
            .unwrap()
            .id;
        (app, w)
    }

    #[test]
    fn motion_is_off_calm_or_full_by_word_or_round_and_full_is_calm_over_ssh() {
        let (mut app, _) = still(Motion::Off);
        let t = app.lang.text();
        assert_eq!(
            App::new(1, 1, 0, 1, Lang::En).motion,
            Motion::Off,
            "until main says"
        );
        assert_eq!(app.command_note("motion"), "(off)");
        app.command("motion");
        assert_eq!(app.motion, Motion::Calm);
        assert_eq!(flashed(&app).as_deref(), Some(t.motion_calm));
        assert_eq!(app.command_note("motion"), "(calm)");
        app.command("motion");
        assert_eq!(app.motion, Motion::Full);
        assert_eq!(flashed(&app).as_deref(), Some(t.motion_full));
        assert_eq!(app.moving(), Motion::Full);
        app.command("motion");
        assert_eq!(app.motion, Motion::Off);
        assert_eq!(flashed(&app).as_deref(), Some(t.motion_off));
        assert_eq!(app.moving(), Motion::Off);
        app.ssh = true;
        app.command("motion full");
        assert_eq!(app.motion, Motion::Full, "chosen, kept");
        assert_eq!(flashed(&app).as_deref(), Some(t.motion_ssh));
        assert_eq!(app.moving(), Motion::Calm, "runs as calm");
        app.command("motion calm");
        assert_eq!(flashed(&app).as_deref(), Some(t.motion_calm));
        app.command("motion off");
        assert_eq!(flashed(&app).as_deref(), Some(t.motion_off));
        app.command("motion wild");
        assert_eq!(app.motion, Motion::Off, "as it was");
        assert_eq!(
            flashed(&app).as_deref(),
            Some("wild is not off, calm or full")
        );
        app.ssh = false;
        app.motion = Motion::Full;
        app.focus(false, Instant::now());
        assert_eq!(app.moving(), Motion::Off, "nothing moves in another window");
    }

    #[test]
    fn the_mark_comes_a_dot_a_beat_from_when_it_shows_and_whole_when_nothing_moves() {
        let (mut app, _) = still(Motion::Calm);
        let ms = Duration::from_millis;
        let all = ui::brand::DOTS;
        assert_eq!(app.mark_dots(), all, "never shown: whole");
        // First drawn 100 ms in: its dots count from the next beat.
        app.frame_after(ms(100));
        app.mark_shown();
        assert_eq!(app.mark_from.get(), Some(app.after(ms(300))));
        assert!(app.mark_drawn.get());
        for (at, dots) in [
            (100, 0),
            (299, 0),
            (300, 0),
            (599, 0),
            (600, 1),
            (1199, 2),
            (1800, 5),
            (2099, 5),
            (2100, 6),
            (9000, 6),
        ] {
            app.frame_after(ms(at));
            assert_eq!(app.mark_dots(), dots, "at {at} ms");
        }
        // Drawn again in the same drawing, or the next: still coming.
        app.frame_after(ms(900));
        app.mark_shown();
        app.mark_before.set(true);
        app.mark_drawn.set(false);
        app.mark_shown();
        assert_eq!(app.mark_from.get(), Some(app.after(ms(300))), "kept");
        // Gone for a drawing, then back: it comes again.
        app.mark_before.set(false);
        app.mark_drawn.set(false);
        app.frame_after(ms(1200));
        app.mark_shown();
        assert_eq!(
            app.mark_from.get(),
            Some(app.after(ms(1200))),
            "on the beat itself"
        );
        app.frame_after(ms(1800));
        assert_eq!(app.mark_dots(), 2);
        app.motion = Motion::Full;
        assert_eq!(app.mark_dots(), 2, "full comes as calm");
        app.motion = Motion::Off;
        assert_eq!(app.mark_dots(), all, "off: whole");
        app.motion = Motion::Calm;
        app.ssh = true;
        assert_eq!(app.mark_dots(), all, "over SSH: whole");
        app.ssh = false;
        app.focus(false, Instant::now());
        assert_eq!(app.mark_dots(), all, "another window: whole");
        app.focus(true, Instant::now());
        assert_eq!(app.mark_dots(), 2);
        app.on_key(KeyEvent::from(KeyCode::Char('j')));
        assert_eq!(app.mark_dots(), all, "a key: whole");
        assert_eq!(app.mark_from.get(), None);
    }

    #[test]
    fn the_mark_moves_only_where_it_was_drawn_and_until_it_is_whole() {
        let (app, _) = still(Motion::Calm);
        let ms = Duration::from_millis;
        app.mark_from.set(Some(app.after(ms(300))));
        assert_eq!(app.next_motion(app.after(ms(0))), None, "not drawn");
        app.mark_drawn.set(true);
        assert_eq!(
            app.moving_parts(app.after(ms(0))),
            [(Moves::Mark, motion::BEAT)]
        );
        assert_eq!(app.next_motion(app.after(ms(0))), Some(app.after(ms(300))));
        assert_eq!(
            app.next_motion(app.after(ms(1800))),
            Some(app.after(ms(2100))),
            "the last dot"
        );
        assert_eq!(
            app.next_motion(app.after(ms(2099))),
            Some(app.after(ms(2100)))
        );
        assert_eq!(app.next_motion(app.after(ms(2100))), None, "whole");
        app.mark_from.set(None);
        assert_eq!(
            app.next_motion(app.after(ms(0))),
            None,
            "a key made it whole"
        );
        app.mark_from.set(Some(app.after(ms(300))));
        let mut app = app;
        app.ssh = true;
        assert_eq!(app.next_motion(app.after(ms(0))), None, "SSH");
    }

    #[test]
    fn each_thing_that_moves_is_drawn_as_it_is_at_the_frame() {
        let (mut app, w) = still(Motion::Off);
        let ms = Duration::from_millis;
        let e = app.epoch;
        let at = |app: &mut App, n: u64| app.frame = e + ms(n);
        at(&mut app, 700);
        app.world.agents[w].status = Status::Working;
        assert!(!app.breath());
        assert_eq!(
            (app.dots(), app.spin(), app.head_spin()),
            (None, None, None)
        );
        app.motion = Motion::Calm;
        at(&mut app, 599);
        assert!(!app.breath());
        assert_eq!(app.dots(), Some("   "));
        assert_eq!(app.spin(), Some(motion::SPIN[1]));
        assert_eq!(app.head_spin(), None, "calm has none");
        at(&mut app, 600);
        assert!(app.breath(), "the second tone");
        assert_eq!(app.dots(), Some(".  "));
        assert_eq!(app.spin(), Some(motion::SPIN[2]));
        at(&mut app, 1200);
        assert!(!app.breath());
        assert_eq!(app.dots(), Some(".. "));
        at(&mut app, 1800);
        assert!(app.breath());
        assert_eq!(app.dots(), Some("..."));
        at(&mut app, 2400);
        assert_eq!(app.dots(), Some("   "), "round again");
        assert_eq!(app.spin(), Some(motion::SPIN[8]));
        at(&mut app, 3300);
        assert_eq!(app.spin(), Some(motion::SPIN[1]), "eleven beats: round");
        app.motion = Motion::Full;
        at(&mut app, 250);
        assert_eq!(
            app.spin(),
            Some(motion::SPIN[2]),
            "a tenth of a second each"
        );
        assert_eq!(app.head_spin(), Some(motion::SPIN[2]));
        at(&mut app, 1150);
        assert_eq!(app.head_spin(), Some(motion::SPIN[1]));
        app.world.agents[w].status = Status::Idle;
        assert_eq!(app.head_spin(), None, "no one mid-turn");
        app.world.agents[w].status = Status::Working;
        app.ssh = true;
        assert_eq!(app.head_spin(), None, "calm over SSH");
        assert_eq!(app.spin(), Some(motion::SPIN[3]));
        app.ssh = false;
        app.focus(false, Instant::now());
        assert!(!app.breath());
        assert_eq!(
            (app.dots(), app.spin(), app.head_spin()),
            (None, None, None)
        );
    }

    #[test]
    fn what_comes_to_wait_blinks_and_what_is_done_or_started_settles() {
        let (mut app, w) = still(Motion::Calm);
        let ms = Duration::from_millis;
        let e = app.epoch;
        let me = GlowOn::Agent(w);
        app.watch_glows(e);
        assert!(app.glows.is_empty(), "the first look lights nothing");
        // A question: its row and its item blink from the next beat.
        app.world.inbox.push(InboxItem {
            kind: InboxKind::Question,
            agent: w,
            detail: "which?".into(),
            researcher: None,
            open: true,
            tick: 0,
            card: None,
        });
        app.watch_glows(e + ms(1000));
        assert_eq!(
            app.glows,
            [
                (me, Glow::Arrive, e + ms(1200)),
                (GlowOn::Item(0), Glow::Arrive, e + ms(1200)),
            ]
        );
        let lit = |app: &mut App, n: u64, on: GlowOn| {
            app.frame = e + ms(n);
            app.lit(on)
        };
        assert_eq!(lit(&mut app, 1000, me), Some(Lit::Reversed), "at once");
        assert_eq!(lit(&mut app, 1499, me), Some(Lit::Reversed));
        assert_eq!(lit(&mut app, 1500, me), None);
        assert_eq!(lit(&mut app, 1800, GlowOn::Item(0)), Some(Lit::Reversed));
        assert_eq!(lit(&mut app, 3000, me), None, "over");
        assert_eq!(lit(&mut app, 1000, GlowOn::Item(1)), None);
        app.watch_glows(e + ms(1100));
        assert_eq!(app.glows.len(), 2, "nothing new, nothing lit");
        // Answered and done: it settles; an item seen once is not new.
        app.world.inbox[0].open = false;
        app.unseen.insert(w);
        app.watch_glows(e + ms(2000));
        assert_eq!(app.glows[1], (me, Glow::Settle, e + ms(2100)));
        assert_eq!(lit(&mut app, 2400, me), Some(Lit::Settle(1)));
        // Seen, then starting, then started: it settles again.
        app.unseen.clear();
        app.world.agents[w].status = Status::Working;
        app.world.agents[w].starting = true;
        app.watch_glows(e + ms(4000));
        assert_eq!(app.glows.len(), 2, "idle to starting lights nothing");
        assert_eq!(app.glows[1], (me, Glow::Settle, e + ms(2100)), "as it was");
        app.world.agents[w].starting = false;
        app.watch_glows(e + ms(4200));
        assert_eq!(
            app.glows[1],
            (me, Glow::Settle, e + ms(4200)),
            "on the beat"
        );
        assert_eq!(app.glows.len(), 2, "in place of its last");
        // Working to idle, nothing to say.
        app.world.agents[w].status = Status::Idle;
        app.watch_glows(e + ms(5000));
        assert_eq!(app.glows.len(), 2);
        assert_eq!(app.glows[1], (me, Glow::Settle, e + ms(4200)), "as it was");
        // Motion off, or another window: nothing lights, and what changed
        // meanwhile does not light later.
        app.motion = Motion::Off;
        app.unseen.insert(w);
        app.world.inbox.push(app.world.inbox[0].clone());
        app.world.inbox[1].open = true;
        app.watch_glows(e + ms(6000));
        assert_eq!(app.glows.len(), 2);
        assert_eq!(lit(&mut app, 4300, me), None, "off: none shown");
        app.motion = Motion::Calm;
        app.watch_glows(e + ms(7000));
        assert_eq!(app.glows.len(), 2, "already seen");
        // An app that starts with items waiting lights none of them.
        let (mut world, sim) = sim::world(1, 1, 0, 1, Lang::En);
        world.inbox.push(app.world.inbox[1].clone());
        let mut fresh = App::with(world, Engine::Sim(sim), Lang::En);
        fresh.motion = Motion::Calm;
        fresh.watch_glows(e);
        assert!(fresh.glows.is_empty());
    }

    #[test]
    fn the_next_frame_is_when_what_is_shown_moves_and_never_when_nothing_does() {
        let (mut app, w) = still(Motion::Calm);
        let ms = Duration::from_millis;
        let e = app.epoch;
        let now = e + ms(100);
        assert_eq!(app.next_motion(now), None, "nothing moves");
        app.world.agents[w].status = Status::Working;
        assert_eq!(app.next_motion(now), Some(e + ms(600)), "it breathes");
        assert_eq!(app.next_motion(e + ms(600)), Some(e + ms(1200)));
        app.view = View::Board;
        app.world.cards.clear();
        assert_eq!(app.next_motion(now), None, "not on the screen");
        // On the board, the owner of a card taken breathes.
        app.world.cards.push(Card {
            id: 0,
            project: 0,
            title: "t".into(),
            owner: Some(w),
            column: Column::Working,
            story: Vec::new(),
            symbols: Vec::new(),
            started: 0,
            finished: 0,
        });
        assert_eq!(app.next_motion(now), Some(e + ms(600)), "its card's owner");
        app.world.cards[0].column = Column::Done;
        assert_eq!(
            app.next_motion(now),
            Some(e + ms(600)),
            "done, still at work"
        );
        app.world.cards[0].column = Column::Waiting;
        assert_eq!(app.next_motion(now), None, "no owner shown");
        app.world.cards[0].column = Column::Working;
        app.world.cards[0].owner = None;
        assert_eq!(app.next_motion(now), None, "nobody's");
        app.world.cards[0].owner = Some(w);
        app.world.agents[w].gone = true;
        assert_eq!(app.next_motion(now), None, "its owner gone");
        app.world.agents[w].gone = false;
        app.world.projects[0].archived = Some(Archived {
            at: 0,
            title: "t".into(),
        });
        assert_eq!(app.next_motion(now), None, "in its archive");
        app.world.projects[0].archived = None;
        app.zoom = Zoom::Focus(w);
        assert_eq!(app.next_motion(now), None, "the board under a zoom");
        app.zoom = Zoom::None;
        app.view = View::Code;
        assert_eq!(app.next_motion(now), None, "the code map");
        app.view = View::Salon;
        app.zoom = Zoom::Focus(w);
        assert_eq!(app.next_motion(now), None);
        app.zoom = Zoom::None;
        app.world.agents[w].gone = true;
        assert_eq!(app.next_motion(now), None, "gone");
        app.world.agents[w].gone = false;
        app.world.projects[0].closed = true;
        assert_eq!(app.next_motion(now), None, "put away");
        app.world.projects[0].closed = false;
        app.motion = Motion::Off;
        assert_eq!(app.next_motion(now), None, "off");
        app.motion = Motion::Calm;
        // A leader working does not breathe; starting counts its dots.
        app.world.agents[w].status = Status::Idle;
        let lead = app.world.projects[0].leader;
        app.world.agents[lead].status = Status::Working;
        assert_eq!(app.next_motion(now), None);
        app.world.agents[lead].status = Status::Idle;
        app.world.agents[w].starting = true;
        assert_eq!(app.next_motion(now), None, "starting, not working");
        app.world.agents[w].status = Status::Working;
        assert_eq!(app.rungs()[w], Rung::Starting);
        assert_eq!(app.next_motion(now), Some(e + ms(600)), "its dots");
        app.world.agents[w].starting = false;
        app.world.agents[w].status = Status::Idle;
        // Waiting for an answer: the bottom line spins when nothing else
        // is said there.
        app.asking = Some("asking".into());
        assert_eq!(app.next_motion(now), Some(e + ms(300)));
        app.flash = Some(("x".into(), now));
        assert_eq!(app.next_motion(now), None, "a flash says something");
        assert_eq!(
            app.next_motion(now + FLASH),
            Some(e + ms(3300)),
            "gone at its end"
        );
        app.flash = None;
        app.typing = true;
        assert_eq!(app.next_motion(now), None, "writing");
        app.typing = false;
        app.grace = Some((Undoable::Close(0), now + UNDO_GRACE));
        assert_eq!(app.next_motion(now), None, "a countdown");
        app.grace = None;
        app.asking = None;
        // The first-run window checking a vendor spins.
        app.welcome = true;
        app.vendors = vec![(Vendor::Claude, Seen::Checking)];
        assert_eq!(app.next_motion(now), Some(e + ms(300)));
        app.vendors = vec![(Vendor::Claude, Seen::Missing)];
        assert_eq!(app.next_motion(now), None, "checked");
        app.vendors = vec![(Vendor::Claude, Seen::Checking)];
        app.welcome = false;
        assert_eq!(app.next_motion(now), None, "not shown");
        // Full: the header spins while anyone works, on any screen.
        app.motion = Motion::Full;
        app.view = View::Board;
        assert_eq!(app.next_motion(now), None, "no one works");
        app.world.agents[lead].status = Status::Working;
        assert_eq!(app.next_motion(now), Some(e + ms(200)));
        app.view = View::Salon;
        app.world.agents[lead].status = Status::Idle;
        app.motion = Motion::Calm;
        // A glow: from its beat to its end, then nothing.
        app.light(GlowOn::Agent(w), Glow::Settle, now);
        assert_eq!(app.glows[0].2, e + ms(300));
        assert_eq!(
            app.next_motion(now),
            Some(e + ms(600)),
            "the beat after its own"
        );
        assert_eq!(app.next_motion(e + ms(700)), Some(e + ms(900)));
        assert_eq!(app.next_motion(e + ms(1100)), Some(e + ms(1200)), "its end");
        assert_eq!(app.next_motion(e + ms(1200)), None);
        // The soonest of all.
        app.world.agents[w].status = Status::Working;
        assert_eq!(app.next_motion(e + ms(700)), Some(e + ms(900)));
        assert_eq!(app.next_motion(e + ms(1000)), Some(e + ms(1200)));
        app.viewing = Some(Box::new(App::new(1, 1, 0, 1, Lang::En)));
        assert_eq!(app.next_motion(now), None, "an archive's view over it");
        app.viewing = None;
        app.focus(false, now);
        assert_eq!(app.next_motion(now), None, "another window");
    }

    #[test]
    fn a_step_looks_for_what_lights() {
        let (mut app, _) = still(Motion::Calm);
        assert!(app.glow_rungs.is_empty());
        app.step();
        assert_eq!(
            app.glow_rungs,
            app.rungs(),
            "seen, to tell what is new next"
        );
        assert_eq!(app.glow_items, app.world.inbox.len());
    }

    #[test]
    fn what_moves_is_drawn_on_its_next_frame_and_never_in_another_window() {
        let (mut app, w) = still(Motion::Calm);
        let ms = Duration::from_millis;
        let e = app.epoch;
        app.world.agents[w].status = Status::Working;
        let t0 = e + ms(100);
        assert!(app.draw_due(t0));
        app.drawn(t0);
        assert_eq!(app.motion_wait(t0), Some(ms(500)));
        assert_eq!(app.motion_wait(t0 + ms(700)), Some(ms(0)), "late: at once");
        assert!(!app.draw_due(t0 + ms(499)));
        assert!(app.draw_due(t0 + ms(500)), "the next tone");
        app.drawn(t0 + ms(500));
        assert_eq!(app.motion_wait(t0 + ms(500)), Some(ms(600)));
        // A glow is kept to its end, then let go.
        app.light(GlowOn::Agent(w), Glow::Arrive, t0 + ms(500));
        app.drawn(e + ms(2399));
        assert_eq!(app.glows.len(), 1);
        app.drawn(e + ms(2400));
        assert!(app.glows.is_empty(), "over");
        // Not yet begun: kept.
        app.light(GlowOn::Agent(w), Glow::Arrive, e + ms(2500));
        app.drawn(e + ms(2500));
        assert_eq!(app.glows.len(), 1);
        // In another window nothing waits for a frame.
        app.focus(false, e + ms(2600));
        assert_eq!(app.motion_wait(e + ms(2600)), None);
        assert!(!app.draw_due(e + ms(4000)));
        app.drawn(e + ms(2600));
        assert_eq!(app.motion_wait(e + ms(2600)), None);
        app.motion = Motion::Off;
        app.focus(true, e + ms(2700));
        app.drawn(e + ms(2700));
        assert_eq!(app.motion_wait(e + ms(2700)), None, "off");
    }

    #[test]
    fn a_frame_is_patched_only_when_the_breath_alone_moved() {
        let (mut app, w) = still(Motion::Calm);
        let ms = Duration::from_millis;
        app.world.agents[w].status = Status::Working;
        let glyph = ui::Glyph {
            x: 5,
            y: 4,
            width: 2,
        };
        *app.breathing.borrow_mut() = Some(vec![glyph]);
        let t0 = app.after(ms(100));
        app.drawn(t0);
        let t1 = app.after(ms(600));
        assert!(app.patchable());
        assert!(app.patch_due(t1), "the breath alone");
        app.stirred = true;
        assert!(!app.patch_due(t1), "a key");
        app.stirred = false;
        app.changed = true;
        assert!(!app.patch_due(t1), "the world");
        app.changed = false;
        app.world.tick += 1;
        assert!(!app.patch_due(t1), "the clock");
        app.world.tick -= 1;
        app.light(GlowOn::Agent(w), Glow::Arrive, t0);
        assert!(!app.patch_due(t1), "a glow");
        app.glows.clear();
        app.world.agents[w].starting = true;
        assert!(!app.patch_due(t1), "dots");
        app.world.agents[w].starting = false;
        *app.breathing.borrow_mut() = Some(Vec::new());
        assert!(!app.patchable());
        assert!(!app.patch_due(t1), "nothing known breathes");
        *app.breathing.borrow_mut() = None;
        assert!(!app.patchable());
        assert!(!app.patch_due(t1), "not known");
        *app.breathing.borrow_mut() = Some(vec![glyph]);
        app.motion = Motion::Off;
        assert!(!app.patch_due(t1), "off");
        app.motion = Motion::Calm;
        assert!(app.patch_due(t1), "again");
    }

    #[test]
    fn a_frame_near_the_tick_is_drawn_with_it() {
        let (mut app, w) = still(Motion::Calm);
        let ms = Duration::from_millis;
        let t0 = app.epoch + ms(100);
        app.world.agents[w].status = Status::Working;
        app.drawn(t0);
        // The next tone 500 ms on.
        assert_eq!(app.wait(t0, ms(601)), ms(500), "far from the tick: its own");
        assert_eq!(app.wait(t0, ms(600)), ms(600), "100 ms before: the tick's");
        assert_eq!(app.wait(t0, ms(550)), ms(550));
        assert_eq!(app.wait(t0, ms(400)), ms(400), "after the tick");
        app.motion = Motion::Off;
        app.drawn(t0);
        assert_eq!(app.wait(t0, ms(700)), ms(700), "nothing moves");
    }

    #[test]
    fn a_commit_message_asked_for_spins_until_it_comes() {
        let (mut app, _) = still(Motion::Calm);
        let t = app.lang.text();
        app.command("commit");
        let asked = app.asking().map(str::to_string);
        assert!(asked.is_some(), "{:?}", flashed(&app));
        assert_eq!(flashed(&app), asked, "said first, then spun");
        assert!(asked.unwrap().contains(&fill(t.suggesting, &["", ""])[..6]));
        app.step();
        assert!(app.asking().is_some(), "not yet");
        app.step();
        assert_eq!(app.asking(), None, "it came");
        // A refusal is only said.
        app.world.budget = app.world.spent();
        app.command("commit");
        assert_eq!(app.asking(), None);
    }

    /// The demo with its look for worktrees no agent owns answered, its
    /// teardown script said yes to.
    fn tidied_demo() -> App {
        let mut app = App::new(1, 1, 0, 1, Lang::En);
        app.command("tidy");
        app.step();
        let checkout = app.world.orphans.found.as_ref().unwrap()[0]
            .checkout
            .clone();
        app.world
            .approve_setup(sim::teardown_script(&checkout).key());
        app
    }

    /// The flash of a removal of `name` after the demo's teardown ran.
    fn removed(name: &str) -> String {
        format!("removed {name} · teardown in {name}: exit 0, 0.4 s · dropped database {name}")
    }

    fn orphan_names(app: &App) -> Vec<String> {
        let found = app.world.orphans.found.as_deref().unwrap_or_default();
        found.iter().map(|o| o.name.clone()).collect()
    }

    #[test]
    fn tidy_opens_the_worktrees_no_agent_owns_and_its_keys_move_on_them() {
        let mut app = App::new(1, 1, 0, 1, Lang::En);
        app.command("tidy");
        assert_eq!(app.tidy, Some(TidyWin::default()));
        assert!(app.world.orphans.busy, "no look asked");
        assert_eq!(app.world.orphans.found, None);
        app.step();
        assert!(!app.world.orphans.busy);
        assert_eq!(orphan_names(&app), ["claude-31", "codex-32"]);
        let at = |app: &App| app.tidy.unwrap().at;
        app.key(key(KeyCode::Char('j')));
        assert_eq!(at(&app), 1);
        app.key(key(KeyCode::Down));
        assert_eq!(at(&app), 1, "past the last");
        app.key(key(KeyCode::Char('k')));
        assert_eq!(at(&app), 0);
        app.key(key(KeyCode::Up));
        assert_eq!(at(&app), 0);
        app.key(key(KeyCode::Char('G')));
        assert_eq!(at(&app), 1);
        app.key(key(KeyCode::Char('g')));
        assert_eq!(at(&app), 0);
        app.key(key(KeyCode::End));
        assert_eq!(at(&app), 1);
        app.key(key(KeyCode::Home));
        assert_eq!(at(&app), 0);
        // A key it does not take does nothing, and the view stays.
        let view = app.view;
        app.key(key(KeyCode::Char('b')));
        assert_eq!((app.tidy, app.view), (Some(TidyWin::default()), view));
        app.key(key(KeyCode::Char('?')));
        assert!(app.help);
        app.help = false;
        app.key(key(KeyCode::Esc));
        assert_eq!(app.tidy, None);
        app.command("tidy");
        app.key(key(KeyCode::Char('q')));
        assert_eq!(app.tidy, None);
    }

    #[test]
    fn a_removes_the_clean_worktrees_and_d_one_with_work_only_after_a_y() {
        let mut app = tidied_demo();
        let t = app.lang.text();
        app.key(key(KeyCode::Char('a')));
        assert!(app.world.orphans.busy);
        // While it goes, another is refused.
        app.key(key(KeyCode::Char('a')));
        assert_eq!(flashed(&app).as_deref(), Some(t.tidy_busy));
        app.step();
        assert_eq!(flashed(&app), Some(removed("claude-31")));
        assert_eq!(app.world.orphans.tidied, None, "said once");
        assert_eq!(orphan_names(&app), ["codex-32"]);
        app.key(key(KeyCode::Char('a')));
        assert_eq!(flashed(&app).as_deref(), Some(t.tidy_no_clean));
        assert!(!app.world.orphans.busy);
        // `d` on one with work asks first; any key but `y` keeps it.
        app.key(key(KeyCode::Char('d')));
        assert_eq!(app.tidy, Some(TidyWin { at: 0, sure: true }));
        app.key(key(KeyCode::Char('n')));
        assert_eq!(app.tidy, Some(TidyWin { at: 0, sure: false }));
        assert!(!app.world.orphans.busy, "removed without a y");
        app.key(key(KeyCode::Char('d')));
        app.key(key(KeyCode::Char('y')));
        assert!(app.world.orphans.busy);
        app.step();
        assert_eq!(flashed(&app), Some(removed("codex-32")));
        assert_eq!(orphan_names(&app), Vec::<String>::new());
        // Nothing left: `d` does nothing, `y` alone removes nothing.
        app.flash = None;
        app.key(key(KeyCode::Char('d')));
        app.key(key(KeyCode::Char('y')));
        assert_eq!(
            (app.tidy, app.world.orphans.busy),
            (Some(TidyWin::default()), false)
        );
        assert_eq!(flashed(&app), None);
        // An index not in the list is refused.
        app.tidy_remove(vec![4], false);
        assert_eq!(flashed(&app).as_deref(), Some(t.tidy_none));
    }

    #[test]
    fn a_removal_asks_about_the_teardown_script_first_and_goes_on_with_the_answer() {
        let mut app = App::new(1, 1, 0, 1, Lang::En);
        let t = app.lang.text();
        app.command("tidy");
        app.step();
        // Asked: the script's bytes in a window, nothing removed yet.
        app.key(key(KeyCode::Char('a')));
        let ask = app
            .teardown
            .clone()
            .expect("the teardown script is asked about");
        assert_eq!((ask.at.clone(), ask.force, ask.scroll), (vec![0], false, 0));
        assert_eq!(ask.script.kind, crate::live::setup::Kind::Teardown);
        assert!(ask.script.text().contains("db:drop"));
        assert!(!app.world.orphans.busy);
        // It scrolls, never past its last line being the top one.
        let lines = ask.script.text().lines().count();
        let scroll = |app: &App| app.teardown.as_ref().unwrap().scroll;
        app.key(key(KeyCode::Char('j')));
        assert_eq!(scroll(&app), 1);
        app.key(key(KeyCode::Char('k')));
        app.key(key(KeyCode::Up));
        assert_eq!(scroll(&app), 0);
        app.key(key(KeyCode::Down));
        assert_eq!(scroll(&app), 1);
        app.key(key(KeyCode::Char('G')));
        assert_eq!(scroll(&app), lines - 1);
        app.key(key(KeyCode::Char('g')));
        assert_eq!(scroll(&app), 0);
        app.key(key(KeyCode::PageDown));
        assert_eq!(scroll(&app), lines - 1);
        app.key(key(KeyCode::PageUp));
        assert_eq!(scroll(&app), 0);
        app.key(ctrl('d'));
        assert_eq!(scroll(&app), lines - 1);
        app.key(ctrl('u'));
        assert_eq!(scroll(&app), 0);
        // Without ctrl, d and u do nothing here.
        app.key(key(KeyCode::Char('d')));
        assert_eq!(scroll(&app), 0);
        app.key(key(KeyCode::Char('j')));
        app.key(key(KeyCode::Char('u')));
        assert_eq!(scroll(&app), 1);
        app.key(key(KeyCode::Char('k')));
        app.key(key(KeyCode::Char('x')));
        assert_eq!(app.teardown, Some(ask.clone()));
        app.key(key(KeyCode::Char('?')));
        assert!(app.help);
        app.help = false;
        // esc: nothing removed, said so; the list window stays.
        app.key(key(KeyCode::Esc));
        assert_eq!(app.teardown, None);
        assert_eq!(flashed(&app).as_deref(), Some(t.teardown_held));
        assert!(app.tidy.is_some());
        assert!(!app.world.orphans.busy);
        app.key(key(KeyCode::Char('a')));
        app.key(key(KeyCode::Char('q')));
        assert_eq!(app.teardown, None);
        // `r`: no for the run, removed without it, not asked again.
        app.key(key(KeyCode::Char('a')));
        app.key(key(KeyCode::Char('r')));
        assert_eq!(app.teardown, None);
        assert!(app.world.orphans.busy);
        app.step();
        assert_eq!(
            flashed(&app).as_deref(),
            Some("removed claude-31 · teardown not run in claude-31: you said no")
        );
        assert!(app.world.setup_approved.is_empty());
        app.key(key(KeyCode::Char('d')));
        app.key(key(KeyCode::Char('y')));
        assert_eq!(app.teardown, None, "asked again after a no");
        app.step();
        // `a` in another run: yes is kept, and the removal it held goes on.
        let mut app = App::new(1, 1, 0, 1, Lang::En);
        app.command("tidy");
        app.step();
        app.key(key(KeyCode::Char('d')));
        app.key(key(KeyCode::Char('y')));
        assert_eq!(app.teardown.as_ref().map(|a| a.at.clone()), Some(vec![0]));
        app.key(key(KeyCode::Char('a')));
        assert_eq!(app.world.setup_approved.len(), 1);
        assert!(app.world.setup_approved[0].starts_with("teardown /bin/sh "));
        assert!(app.world.orphans.busy);
        app.step();
        assert_eq!(flashed(&app), Some(removed("claude-31")));
    }

    #[test]
    fn d_removes_a_clean_worktree_at_once_and_the_selection_stays_on_the_list() {
        let mut app = tidied_demo();
        app.key(key(KeyCode::Char('G')));
        app.key(key(KeyCode::Char('k')));
        app.key(key(KeyCode::Char('d')));
        assert_eq!(app.tidy, Some(TidyWin { at: 0, sure: false }));
        assert!(app.world.orphans.busy);
        app.step();
        assert_eq!(orphan_names(&app), ["codex-32"]);
        // Rows gone, the one at hand moves up onto the list.
        app.command("tidy");
        assert_eq!(app.tidy, Some(TidyWin::default()));
        app.tidy = Some(TidyWin { at: 3, sure: false });
        app.step();
        assert_eq!(app.tidy.unwrap().at, 0);
    }

    #[test]
    fn a_start_says_how_many_worktrees_no_agent_owns_once_unless_shown() {
        let mut app = App::new(1, 1, 0, 1, Lang::En);
        app.start_tidy();
        app.step();
        assert_eq!(
            flashed(&app).as_deref(),
            Some("2 worktrees no agent owns: :tidy lists them")
        );
        // Said once: a later look says nothing at the start's place.
        app.flash = None;
        app.command("tidy");
        app.tidy = None;
        app.step();
        assert_eq!(flashed(&app), None);
        // The window open: it shows them, no flash.
        let mut app = App::new(1, 1, 0, 1, Lang::En);
        app.start_tidy();
        app.tidy = Some(TidyWin::default());
        app.step();
        assert_eq!(flashed(&app), None);
        assert!(!app.tidy_start);
        // None found: nothing said.
        let mut app = App::new(1, 1, 0, 1, Lang::En);
        app.start_tidy();
        if let Engine::Sim(s) = &mut app.engine {
            s.forget_orphans();
        }
        app.step();
        assert_eq!(flashed(&app), None);
        assert!(!app.tidy_start);
    }
}
