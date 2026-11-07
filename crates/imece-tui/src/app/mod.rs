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
use crate::welcome::{self, Seen};
use imece_agent::{Choice, Login};
use std::sync::mpsc::TryRecvError;

mod change;
mod copy;
mod dispatch;
mod focus;
mod frames;
mod grace;
mod jump;
mod line;
mod projects;
mod salon;
mod talk;
mod waiting;
mod windows;

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

/// The screen a bare `:solo` opens: the model (Claude Code's own or an
/// alias), then the effort (its own or one of its levels). The path and
/// name typed with `:solo` wait here for the choice.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SoloPick {
    pub path: Option<String>,
    pub name: Option<String>,
    pub step: PickStep,
    /// The highlighted row of each step.
    pub model: usize,
    pub effort: usize,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PickStep {
    Model,
    Effort,
}

/// How the solo pick screen ended.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Picked {
    /// Given up.
    Nothing,
    /// The last step answered: the choice, none for Claude Code's own.
    Choice(Option<String>),
}

impl SoloPick {
    pub fn new(path: Option<String>, name: Option<String>) -> SoloPick {
        SoloPick {
            path,
            name,
            step: PickStep::Model,
            model: 0,
            effort: 0,
        }
    }

    /// The rows of the step at hand: `default` and the model aliases, or
    /// `default` and the efforts.
    pub fn items(&self) -> Vec<String> {
        let mut items = vec![complete::DEFAULT.to_string()];
        match self.step {
            PickStep::Model => items.extend(live::aliases().iter().map(|a| a.to_string())),
            PickStep::Effort => items.extend(live::efforts().iter().map(|e| e.to_string())),
        }
        items
    }

    /// The highlighted row of the step at hand.
    pub fn sel(&self) -> usize {
        match self.step {
            PickStep::Model => self.model,
            PickStep::Effort => self.effort,
        }
    }

    fn sel_mut(&mut self) -> &mut usize {
        match self.step {
            PickStep::Model => &mut self.model,
            PickStep::Effort => &mut self.effort,
        }
    }

    /// The choice picked so far, as `:solo` would take it: none for Claude
    /// Code's own model and effort.
    pub fn choice(&self) -> Option<String> {
        let pick = |items: Vec<&str>, i: usize| items.get(i).copied().map(str::to_string);
        let c = Choice {
            model: pick(live::aliases(), self.model.saturating_sub(1)).filter(|_| self.model > 0),
            effort: pick(live::efforts().to_vec(), self.effort.saturating_sub(1))
                .filter(|_| self.effort > 0),
        };
        Some(c.wire()).filter(|w| !w.is_empty())
    }

    /// A key: how the screen ended, when it did.
    pub fn on_key(&mut self, code: KeyCode) -> Option<Picked> {
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
                PickStep::Model => self.step = PickStep::Effort,
                PickStep::Effort => return Some(Picked::Choice(self.choice())),
            },
            KeyCode::Esc => match self.step {
                PickStep::Model => return Some(Picked::Nothing),
                PickStep::Effort => self.step = PickStep::Model,
            },
            KeyCode::Char('q') => return Some(Picked::Nothing),
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
        Some("Edit" | "Write" | "MultiEdit" | "NotebookEdit")
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
    /// Every change that comes to wait gets a review from a fresh session
    /// (`:review on|off`; record 106). Off in `App::new`; `main` sets the
    /// saved one, off by default.
    pub review: bool,
    /// The inbox items seen by `:review on`: the ones past it are new.
    review_items: usize,
    /// A change's review shown whole, not its first lines (`R`).
    pub review_whole: bool,
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
    /// The leader's plan an item is about, in a window over the screen
    /// (`enter` on the item, record 114): the item and how far down its
    /// cards are scrolled.
    pub plan_window: Option<(usize, usize)>,
    /// The worktrees no agent owns, in a window over the screen (`:tidy`,
    /// record 96).
    pub tidy: Option<TidyWin>,
    /// A checkout the person is asked to import into Lonca, in a window
    /// over everything (record 117).
    pub importing: Option<ImportAsk>,
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
    /// The worker whose proposal the line being written rejects: its words
    /// are the reason Lonca wants (record 120).
    pub rejecting: Option<AgentId>,
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
    /// A bare `:solo` asks Claude Code, the model and the effort on a
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
    pub digest: Option<Digest>,
    /// What an earlier run left behind, shown once at the start.
    pub left_off: Option<LeftOff>,
    /// The first-run window is open (`:welcome`).
    pub welcome: bool,
    /// What the window says of Claude Code.
    pub seen: Seen,
    /// How a live run asks Claude Code ([`live::look`]); none in the demo,
    /// which says what it says of itself.
    pub look: Option<welcome::Look>,
    /// The answer still to come.
    looking: Option<welcome::Answers>,
    /// A start waits to hear whether Claude Code is signed in: not opens
    /// the window.
    lead_check: bool,
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
    /// changed (an engine's step that brought news, Claude Code looked at,
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
        // Live, Claude Code is asked when the window opens; the demo says
        // what it says of itself.
        let seen = match &engine {
            Engine::Sim(_) => sim::seen(),
            Engine::Live(_) | Engine::View(_) => Seen::Checking,
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
            plan_window: None,
            tidy: None,
            importing: None,
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
            rejecting: None,
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
            digest: None,
            left_off,
            welcome: false,
            seen,
            look: None,
            looking: None,
            lead_check: false,
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
            review: false,
            review_items: glow_items,
            review_whole: false,
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
        self.watch_reviews();
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

    /// `:lonca` says how the project at hand stands on Lonca, `:lonca
    /// NAME` names its repository (the daemon is asked at once, and until
    /// it answers the project is closed: record 116), `:lonca off` leaves
    /// it, `:lonca login` signs the person in and `:lonca import [NAME]`
    /// asks to make a repository of the checkout (record 117).
    fn lonca_command(&mut self, arg: &str) {
        let t = self.lang.text();
        let p = self.current_project();
        let name = self.world.projects[p].name.clone();
        match arg {
            "" => self.flash(self.lonca_state(p)),
            "off" => {
                self.world.on_lonca(p, None);
                self.flash(fill(t.lonca_off, &[&name]));
            }
            "login" => self.store_start(StoreJob::Login(p), t.lonca_signing_in.to_string()),
            "import" => self.lonca_import(p, &name),
            word if word.starts_with("import ") => {
                self.lonca_import(p, word["import ".len()..].trim());
            }
            repo if !valid_repo(repo) => self.flash(fill(t.lonca_bad, &[repo])),
            repo => {
                self.world.on_lonca(p, Some(repo.to_string()));
                if let Engine::Live(l) = &mut self.engine {
                    l.look_now();
                }
                self.flash(fill(t.lonca_named, &[&name, repo]));
            }
        }
    }

    /// `:sync` brings trunk to the checkout of the project at hand: a
    /// project on Lonca whose store answers, with a checkout to bring it
    /// to. `lonca fetch`, then a fast-forward; nothing else moves.
    fn sync_command(&mut self) {
        let t = self.lang.text();
        let p = self.current_project();
        let project = &self.world.projects[p];
        let name = project.name.clone();
        let Some(on) = &project.lonca else {
            return self.flash(fill(t.lonca_none, &[&name]));
        };
        if project.no_git {
            return self.flash(fill(t.sync_no_git, &[&name]));
        }
        let repo = on.repo.clone();
        if self.world.store_closed(p).is_some() {
            return self.flash(self.lonca_state(p));
        }
        let word = fill(t.syncing, &[&repo]);
        self.store_start(StoreJob::Sync { project: p, repo }, word);
    }

    /// `:lonca import`: the window that asks, unless the checkout can not
    /// go. It goes as `repo`.
    fn lonca_import(&mut self, p: ProjectId, repo: &str) {
        let t = self.lang.text();
        let project = &self.world.projects[p];
        let name = project.name.clone();
        if let Some(on) = &project.lonca {
            return self.flash(fill(t.lonca_import_on, &[&name, &on.repo]));
        }
        if project.no_git {
            return self.flash(fill(t.lonca_import_no_git, &[&name]));
        }
        if !valid_repo(repo) {
            return self.flash(fill(t.lonca_bad, &[repo]));
        }
        let (asked, busy) = match &self.engine {
            Engine::Live(l) => (l.import_ask(p, repo), l.store_busy()),
            Engine::Sim(s) => (s.import_ask(&self.world, p, repo), s.store_busy()),
            Engine::View(_) => return self.read_only(),
        };
        if busy {
            return self.flash(t.lonca_busy);
        }
        match asked {
            Some(ask) => self.importing = Some(ask),
            None => self.flash(fill(t.lonca_import_no_branch, &[&name])),
        }
    }

    /// A key while an import is asked about: `a` is the person's yes, `esc`
    /// or `q` closes the window and nothing goes, `?` the help; the rest do
    /// nothing, the same as on the other windows that ask.
    fn import_key(&mut self, k: KeyEvent) {
        match k.code {
            KeyCode::Char('a') => {
                let Some(ask) = self.importing.take() else {
                    return;
                };
                let word = fill(self.lang.text().lonca_importing, &[&ask.repo]);
                self.store_start(StoreJob::Import(ask), word);
            }
            KeyCode::Esc | KeyCode::Char('q') => {
                self.importing = None;
                self.flash(self.lang.text().lonca_import_held);
            }
            KeyCode::Char('?') => self.open_help(),
            _ => {}
        }
    }

    /// The engine starts `job` and the bottom line says `word` while it
    /// runs; one at a time.
    fn store_start(&mut self, job: StoreJob, word: String) {
        let started = match &mut self.engine {
            Engine::Live(l) => l.store_job(job),
            Engine::Sim(s) => s.store_job(&self.world, job),
            Engine::View(_) => return self.read_only(),
        };
        if started {
            self.asking = Some(word.clone());
            self.flash(word);
        } else {
            self.flash(self.lang.text().lonca_busy);
        }
    }

    /// How project `p` stands on Lonca, in words.
    fn lonca_state(&self, p: ProjectId) -> String {
        let t = self.lang.text();
        let project = &self.world.projects[p];
        let name = &project.name;
        let Some(on) = &project.lonca else {
            return fill(t.lonca_none, &[name]);
        };
        match &on.seen {
            StoreSeen::Asking => fill(t.lonca_named, &[name, &on.repo]),
            StoreSeen::Up {
                trunk,
                pending,
                behind,
            } => {
                let up = fill(
                    t.lonca_state_up,
                    &[
                        name,
                        &on.repo,
                        ui::words::short_commit(trunk),
                        &pending.to_string(),
                    ],
                );
                match behind {
                    Behind::By(0) | Behind::Unknown => up,
                    Behind::By(n) => fill(t.lonca_state_behind, &[&up, &n.to_string()]),
                    Behind::Never => fill(t.lonca_state_never, &[&up]),
                }
            }
            StoreSeen::Down(why) => fill(
                t.lonca_state_down,
                &[name, &on.repo, ui::words::store_down(t, *why)],
            ),
        }
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

    /// Where a command of the person's own runs: the checkout, or where
    /// the demo was started.
    pub fn cwd(&self) -> PathBuf {
        match &self.engine {
            Engine::Live(l) => l.repo_of(self.current_project()).to_path_buf(),
            Engine::Sim(_) | Engine::View(_) => PathBuf::from("."),
        }
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

    pub(super) fn key(code: KeyCode) -> KeyEvent {
        KeyEvent::new(code, KeyModifiers::NONE)
    }

    pub(super) fn ctrl(c: char) -> KeyEvent {
        KeyEvent::new(KeyCode::Char(c), KeyModifiers::CONTROL)
    }

    /// A demo team with one worker whose change, `patch`, waits for the
    /// person and is open on the screen.
    pub(super) fn with_change(patch: &str) -> (App, AgentId) {
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
            check: Check::None,
            spend: None,
        });
        app.zoom = Zoom::Diff(worker);
        app.pane = Pane::Inbox;
        (app, worker)
    }

    /// A conversation of the person with a worker: the person's "first",
    /// then the worker's answer `alpha **beta** gamma` / `delta epsilon`,
    /// selected. Its lines: 0 and 2 heads, 1 "first", 3 and 4 the answer.
    pub(super) fn talk_with_words() -> (App, AgentId) {
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

    pub(super) fn keys(app: &mut App, keys: &str) {
        for c in keys.chars() {
            app.on_key(key(KeyCode::Char(c)));
        }
    }

    /// The choice of the open conversation: the cursor's line, the anchor's,
    /// the words.
    pub(super) fn choice(app: &App) -> (usize, usize, Option<(usize, usize)>) {
        let th = app.thread.as_ref().unwrap();
        let c = th.copy.unwrap();
        (c.at, c.anchor, th.words)
    }

    #[test]
    fn a_runs_time_is_kept_in_whole_milliseconds() {
        assert_eq!(millis(Duration::from_micros(1_234_567)), 1_234);
        assert_eq!(millis(Duration::ZERO), 0);
        assert_eq!(millis(Duration::MAX), u64::MAX, "too long to count");
    }

    /// A quiet world of one project and four idle workers.
    pub(super) fn four_idle() -> (App, Vec<AgentId>) {
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

    pub(super) fn item(agent: AgentId, open: bool) -> InboxItem {
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

    /// The salon's selectable rows.
    pub(super) fn shown(app: &App) -> Vec<Sel> {
        ui::salon::rows(app, 100)
            .iter()
            .filter_map(ui::salon::Row::sel)
            .collect()
    }

    /// Two projects, everyone idle with nothing open; the first two
    /// workers of project 0 and the first of project 1.
    pub(super) fn two_quiet_projects() -> (App, [AgentId; 3]) {
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
    /// Code signed in.
    pub(super) fn fake_look() -> welcome::Answers {
        let (tx, rx) = std::sync::mpsc::channel();
        tx.send(here("2.1.3", Login::Yes)).unwrap();
        rx
    }

    pub(super) fn here(version: &str, login: Login) -> Seen {
        Seen::Here {
            version: version.into(),
            login,
        }
    }

    /// A live app asking `look`.
    pub(super) fn live_welcome(look: welcome::Look) -> App {
        let w = crate::live::testing::team(1);
        let live = crate::live::testing::idle_live(2);
        let mut app = App::with(w, Engine::Live(Box::new(live)), Lang::En);
        app.look = Some(look);
        app
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
    pub(super) fn with_thread() -> (App, AgentId) {
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

    /// The demo after a while, its cards on the board.
    /// A demo team at its start with nobody at work and nothing waiting.
    pub(super) fn quiet() -> App {
        let mut app = App::new(1, 4, 0, 1, Lang::En);
        app.world.inbox.clear();
        for a in &mut app.world.agents {
            a.status = Status::Idle;
        }
        app
    }

    pub(super) fn a_change(agent: AgentId) -> Change {
        Change {
            agent,
            title: "did it".into(),
            files: Vec::new(),
            patch: "+x\n".into(),
            runs: Vec::new(),
            check: Check::None,
            spend: None,
        }
    }

    /// What one `q` asks.
    pub(super) fn asked(app: &mut App) -> String {
        app.on_key(key(KeyCode::Char('q')));
        assert_eq!(app.pending, Some('q'));
        let said = app.flash.take().map(|(m, _)| m).unwrap_or_default();
        app.pending = None;
        said
    }

    /// A demo team at its start whose inbox holds only `kinds`, one item
    /// each, oldest first, item `n` from agent `n + 1`.
    pub(super) fn waiting(kinds: &[InboxKind]) -> App {
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
    pub(super) fn press(app: &mut App, c: char) -> Option<usize> {
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

    pub(super) fn flashed(app: &App) -> Option<String> {
        app.flash.as_ref().map(|(m, _)| m.clone())
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
    pub(super) fn viewing() -> App {
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

    pub(super) fn view(app: &App) -> &App {
        app.viewing.as_deref().expect("the view is open")
    }

    pub(super) fn said(app: &App) -> &str {
        app.flash.as_ref().map_or("", |f| f.0.as_str())
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
            |a: &mut App| a.solo_pick = Some(SoloPick::new(None, None)),
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

    pub(super) fn with_cards(projects: usize) -> App {
        let mut app = App::new(projects, 4, 4, 7, Lang::En);
        for _ in 0..140 * projects {
            app.step();
        }
        assert!(app.world.cards.len() >= 4, "{}", app.world.cards.len());
        app
    }

    pub(super) fn type_in(app: &mut App, text: &str) {
        for c in text.chars() {
            app.on_key(key(KeyCode::Char(c)));
        }
    }

    /// A quiet team whose salon showed its header and its agents, two
    /// open items of WAITING FOR YOU (after a closed one) and two journal
    /// lines, as a drawing leaves them for `F`.
    pub(super) fn drawn_for_hints() -> (App, Vec<Target>) {
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

    pub(super) fn shift(c: char) -> KeyEvent {
        KeyEvent::new(KeyCode::Char(c), KeyModifiers::SHIFT)
    }

    pub(super) fn dir(name: &str) -> PathBuf {
        let dir =
            std::env::temp_dir().join(format!("imece-app-keys-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    /// A demo of one project and one worker, everyone idle, motion `m`;
    /// the worker's id.
    pub(super) fn still(m: Motion) -> (App, AgentId) {
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

    /// The flash of a removal of `name` after the demo's teardown ran.
    pub(super) fn removed(name: &str) -> String {
        format!("removed {name} · teardown in {name}: exit 0, 0.4 s · dropped database {name}")
    }

    /// A demo app whose leader's plan waits in WAITING FOR YOU, selected
    /// in the inbox pane: the app, the item and the leader.
    pub(crate) fn asking_plan() -> (App, usize, AgentId) {
        let mut app = waiting(&[]);
        let leader = app.world.projects[0].leader;
        let cards = vec![
            Planned {
                title: "Parser".into(),
                tier: "cheap".into(),
                paths: vec![Owned::new("src/p.rs"), Owned::shared("docs")],
                body: "Fix it.\nThen test.".into(),
            },
            Planned {
                title: "Docs".into(),
                tier: "standard".into(),
                paths: Vec::new(),
                body: "Write them.".into(),
            },
        ];
        let item = app.world.propose(
            0,
            Proposal {
                cards,
                cost: 12_500,
                tick: 0,
            },
        );
        app.pane = Pane::Inbox;
        app.inbox_sel = app.inbox_rows().iter().position(|&i| i == item).unwrap();
        (app, item, leader)
    }

    #[test]
    fn lonca_names_a_repository_says_how_it_stands_and_leaves_it() {
        let mut app = App::with(
            crate::live::testing::team(0),
            Engine::Live(Box::new(crate::live::testing::idle_live(1))),
            Lang::En,
        );
        let name = app.world.projects[0].name.clone();
        assert_eq!(app.command_note("lonca"), "");
        app.command("lonca");
        assert_eq!(
            flashed(&app).unwrap(),
            format!(
                "{name} is on no Lonca repository; :lonca NAME names one, :lonca import makes one"
            )
        );
        app.command("lonca a/b");
        assert_eq!(flashed(&app).unwrap(), "a/b can not be a repository's name");
        assert!(app.world.projects[0].lonca.is_none());
        // Named: the next step asks at once, the project closed till then.
        if let Engine::Live(l) = &mut app.engine {
            l.look_now();
        }
        app.command("lonca  demo ");
        let asked = format!("{name} is on Lonca repository demo now; imece asks its daemon");
        assert_eq!(flashed(&app).unwrap(), asked);
        assert_eq!(app.command_note("lonca"), "(demo)");
        assert!(app.world.store_closed(0).is_some());
        app.command("lonca");
        assert_eq!(flashed(&app).unwrap(), asked, "still asking");
        let up = StoreSeen::Up {
            trunk: "693d62952cd8f7ce58b0de780cf5d998b2f5c1a6".into(),
            pending: 2,
            behind: Behind::By(0),
        };
        app.world.store_seen(0, "demo", up);
        app.command("lonca");
        assert_eq!(
            flashed(&app).unwrap(),
            format!("{name} is on Lonca repository demo: trunk 693d629, 2 waiting for a human")
        );
        app.world
            .store_seen(0, "demo", StoreSeen::Down(StoreDown::SignedOut));
        app.command("lonca");
        assert_eq!(
            flashed(&app).unwrap(),
            format!("{name} is on Lonca repository demo: you are not signed in (:lonca login)")
        );
        app.command("lonca off");
        assert_eq!(
            flashed(&app).unwrap(),
            format!("{name} is on no Lonca repository now")
        );
        assert!(app.world.projects[0].lonca.is_none());
    }

    #[test]
    fn naming_a_repository_makes_the_next_step_ask_at_once() {
        let mut app = App::with(
            crate::live::testing::team(0),
            Engine::Live(Box::new(crate::live::testing::idle_live(1))),
            Lang::En,
        );
        if let Engine::Live(l) = &mut app.engine {
            l.lonca_after_a_look();
            assert!(!l.lonca_is_due());
        }
        app.command("lonca demo");
        if let Engine::Live(l) = &app.engine {
            assert!(l.lonca_is_due());
        }
    }

    #[test]
    fn lonca_is_refused_in_an_archives_view() {
        assert!(!VIEW_COMMANDS.contains(&"lonca"));
    }

    #[test]
    fn the_demo_takes_lonca_as_live_does() {
        let (world, sim) = crate::sim::world(1, 1, 0, 1, Lang::En);
        let mut app = App::with(world, Engine::Sim(sim), Lang::En);
        app.command("lonca nobody");
        assert_eq!(app.world.stores().len(), 1);
    }

    /// A demo of one project, on no repository.
    fn off_lonca() -> App {
        let (mut world, sim) = crate::sim::world(1, 1, 0, 1, Lang::En);
        world.on_lonca(0, None);
        App::with(world, Engine::Sim(sim), Lang::En)
    }

    /// Steps until the engine has nothing the person asked for.
    fn store_job_ends(app: &mut App) {
        for _ in 0..8 {
            app.step();
            if app.asking.is_none() {
                return;
            }
        }
        panic!("the job never ended");
    }

    /// A demo whose first project is on repository `shop`, up.
    fn on_shop() -> App {
        let mut app = off_lonca();
        app.world.on_lonca(0, Some("shop".into()));
        app.world.store_seen(0, "shop", shop_up(Behind::By(2)));
        app
    }

    fn shop_up(behind: Behind) -> StoreSeen {
        StoreSeen::Up {
            trunk: "693d62952cd8".into(),
            pending: 1,
            behind,
        }
    }

    #[test]
    fn sync_brings_trunk_to_the_project_at_hand() {
        let mut app = on_shop();
        app.command("sync");
        assert_eq!(
            flashed(&app).as_deref(),
            Some("bringing trunk of shop to the checkout…")
        );
        assert!(app.asking.is_some());
        app.command("sync");
        assert_eq!(
            flashed(&app).as_deref(),
            Some("a lonca command is under way")
        );
    }

    #[test]
    fn sync_is_refused_off_lonca_without_git_and_while_the_store_is_down() {
        let mut app = off_lonca();
        let name = app.world.projects[0].name.clone();
        app.command("sync");
        assert!(
            flashed(&app)
                .unwrap()
                .starts_with(&format!("{name} is on no Lonca"))
        );
        assert!(app.asking.is_none());

        let mut app = on_shop();
        app.world.projects[0].no_git = true;
        app.command("sync");
        assert_eq!(
            flashed(&app).unwrap(),
            format!("{name} is no git checkout: there is nothing to bring trunk to")
        );
        assert!(app.asking.is_none());

        let mut app = on_shop();
        app.world
            .store_seen(0, "shop", StoreSeen::Down(StoreDown::NoDaemon));
        app.command("sync");
        assert_eq!(
            flashed(&app).unwrap(),
            format!("{name} is on Lonca repository shop: no daemon answers (lonca daemon)")
        );
        assert!(app.asking.is_none());
        assert!(!VIEW_COMMANDS.contains(&"sync"));
    }

    #[test]
    fn lonca_says_how_far_behind_the_checkout_is_and_the_menu_too() {
        let mut app = on_shop();
        let name = app.world.projects[0].name.clone();
        let up =
            format!("{name} is on Lonca repository shop: trunk 693d629, 1 waiting for a human");
        for (behind, said, note) in [
            (
                Behind::By(2),
                format!("{up}; your checkout is 2 behind (:sync)"),
                "(↓2)",
            ),
            (
                Behind::Never,
                format!("{up}; your checkout has commits trunk never had"),
                "(↓?)",
            ),
            (Behind::By(0), up.clone(), ""),
            (Behind::Unknown, up.clone(), ""),
        ] {
            app.world.store_seen(0, "shop", shop_up(behind));
            app.command("lonca");
            assert_eq!(flashed(&app).unwrap(), said);
            assert_eq!(app.command_note("sync"), note);
        }
        app.world.on_lonca(0, None);
        assert_eq!(app.command_note("sync"), "");
    }

    #[test]
    fn lonca_import_asks_first_and_only_a_yes_runs_it() {
        let mut app = off_lonca();
        let project = app.world.projects[0].name.clone();
        app.command("lonca import shop");
        let ask = app.importing.clone().expect("the window");
        assert_eq!(ask.repo, "shop");
        assert_eq!(ask.project, 0);
        assert!(app.asking.is_none(), "nothing runs yet");
        // Nothing but its keys does anything.
        app.key(key(KeyCode::Char('j')));
        assert_eq!(app.importing, Some(ask.clone()));
        app.key(key(KeyCode::Esc));
        assert_eq!(app.importing, None);
        assert_eq!(flashed(&app).as_deref(), Some("nothing went into Lonca"));
        store_job_ends(&mut app);
        assert!(app.world.projects[0].lonca.is_none());

        app.command("lonca import");
        assert_eq!(
            app.importing.as_ref().unwrap().repo,
            project,
            "its own name"
        );
        app.key(key(KeyCode::Char('q')));
        assert_eq!(app.importing, None);

        app.command("lonca  import   shop ");
        app.key(key(KeyCode::Char('a')));
        assert_eq!(app.importing, None);
        let word = "importing into Lonca as shop…";
        assert_eq!(flashed(&app).as_deref(), Some(word));
        assert_eq!(app.asking.as_deref(), Some(word));
        store_job_ends(&mut app);
        assert_eq!(app.world.projects[0].lonca.as_ref().unwrap().repo, "shop");
    }

    #[test]
    fn lonca_import_is_refused_when_the_checkout_can_not_go() {
        let mut app = off_lonca();
        let name = app.world.projects[0].name.clone();
        app.command("lonca import a/b");
        assert_eq!(app.importing, None);
        assert_eq!(flashed(&app).unwrap(), "a/b can not be a repository's name");
        app.world.projects[0].no_git = true;
        app.command("lonca import shop");
        assert_eq!(app.importing, None);
        assert_eq!(
            flashed(&app).unwrap(),
            format!("{name} is no git checkout: there is nothing to import")
        );
        app.world.projects[0].no_git = false;
        app.world.on_lonca(0, Some("shop".into()));
        app.command("lonca import other");
        assert_eq!(app.importing, None);
        assert_eq!(
            flashed(&app).unwrap(),
            format!("{name} is on Lonca repository shop already")
        );
    }

    #[test]
    fn a_detached_checkout_is_told_to_take_a_branch() {
        let repo = crate::live::testing::checkout("import-detached");
        let out = std::process::Command::new("git")
            .args(["checkout", "--quiet", "--detach"])
            .current_dir(&repo)
            .output()
            .unwrap();
        assert!(out.status.success());
        let mut app = App::with(
            crate::live::testing::team(0),
            Engine::Live(Box::new(crate::live::testing::idle_live_in(&repo, 1))),
            Lang::En,
        );
        let name = app.world.projects[0].name.clone();
        app.command("lonca import shop");
        assert_eq!(app.importing, None);
        assert_eq!(
            flashed(&app).unwrap(),
            format!("{name} is on no branch: check one out to import it")
        );
        std::fs::remove_dir_all(&repo).unwrap();
    }

    #[test]
    fn one_lonca_command_at_a_time() {
        let mut app = off_lonca();
        app.command("lonca login");
        assert_eq!(flashed(&app).as_deref(), Some("signing in to Lonca…"));
        assert_eq!(app.asking.as_deref(), Some("signing in to Lonca…"));
        app.command("lonca login");
        assert_eq!(
            flashed(&app).as_deref(),
            Some("a lonca command is under way")
        );
        app.command("lonca import shop");
        assert_eq!(app.importing, None, "no window for what can not start");
        assert_eq!(
            flashed(&app).as_deref(),
            Some("a lonca command is under way")
        );
        store_job_ends(&mut app);
        assert!(matches!(
            &app.world.journal.last().unwrap().event,
            Event::StoreSignedIn { human, .. } if human == "demo"
        ));
    }

    #[test]
    fn a_yes_while_a_job_has_started_since_says_so() {
        let mut app = off_lonca();
        app.command("lonca import shop");
        if let Engine::Sim(s) = &mut app.engine {
            assert!(s.store_job(&app.world, StoreJob::Login(0)));
        }
        app.key(key(KeyCode::Char('a')));
        assert_eq!(
            flashed(&app).as_deref(),
            Some("a lonca command is under way")
        );
        assert_eq!(app.importing, None);
        assert!(app.asking.is_none());
    }

    #[test]
    fn a_window_that_asks_about_an_import_opens_the_help_on_its_key() {
        let mut app = off_lonca();
        app.command("lonca import shop");
        app.key(key(KeyCode::Char('?')));
        assert!(app.help);
        assert!(app.importing.is_some());
    }

    #[test]
    fn an_archive_view_runs_no_lonca_command() {
        let (world, sim) = crate::sim::world(1, 1, 0, 1, Lang::En);
        let mut app = App::with(world, Engine::Sim(sim), Lang::En);
        app.engine = Engine::View(Viewed {
            project: 0,
            title: String::new(),
            at: 0,
        });
        app.store_start(StoreJob::Login(0), "x".into());
        assert_eq!(app.asking, None);
        app.lonca_import(0, "shop");
        assert_eq!(app.importing, None);
    }
}
