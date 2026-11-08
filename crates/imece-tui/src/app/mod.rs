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
mod detail;
mod dispatch;
mod focus;
mod frames;
mod grace;
mod home;
mod jump;
mod line;
mod lonca;
mod notices;
mod projects;
mod ps;
mod salon;
mod talk;
mod views;
mod waiting;
mod windows;

pub use detail::Tab;
pub use home::{Heading, Home, HomeGroup, HomeRow};

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
pub use self::ps::{PsWin, Sure, Whose};
pub(crate) use self::ps::{left_pids, ps_rows, whose_name};

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct TidyWin {
    pub at: usize,
    pub sure: bool,
}

/// The `:lonca` window (record 125, step 4): the row at hand among the
/// proposals waiting in the project's repository.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct LoncaWin {
    pub at: usize,
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
    "quit", "next", "defer", "waiting", "fold", "unfold", "trace", "peek", "news", "settle",
    "unsettle", "talk", "why", "story", "journal",
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
    /// TO DECIDE and the journal.
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
    /// The spinner where imece waits for an answer.
    Spin,
    /// The spinner by the header's fleet (full motion).
    HeadSpin,
    /// imece's mark at the start, a dot a beat (`App::mark_dots`).
    Mark,
}

/// A view that became a window over the screen (record 125, step 5):
/// the board, the code map, the timeline (one lane per agent over time,
/// record 100). Opened by its `:` name alone, closed by `esc`; it takes
/// every key while it is open.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Window {
    Board,
    Code,
    Timeline,
    /// The journal, told as the story or line by line (record 125, step
    /// 5: the salon's side panel in a window).
    Journal,
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
    /// Something of it waits in TO DECIDE.
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
    /// Something of it waits in TO DECIDE.
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
                    .filter(|c| c.project == q.id && !c.over())
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
    pub zoom: Zoom,
    pub pane: Pane,
    pub selected: Sel,
    /// Folded projects show only their header line.
    pub collapsed: Vec<bool>,
    /// Workers whose helpers are folded away.
    pub folded_workers: BTreeSet<AgentId>,
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
    /// Each agent's session's processes, in a window over the screen
    /// (`:ps`, record 129), and when they were last asked for.
    pub ps: Option<PsWin>,
    pub ps_looked: Option<Instant>,
    /// The start's look at the processes has not been said yet.
    ps_start: bool,
    /// The project at hand's Lonca repository in a window over the screen
    /// (bare `:lonca`, record 125): what waits for a human, what is held.
    pub lonca_win: Option<LoncaWin>,
    /// The board, the code map or the timeline in a window over the
    /// screen (`:board` `:code` `:timeline`, record 125).
    pub window: Option<Window>,
    /// The board window shows only the cards no other card holds back
    /// (record 126, F10): `r` there; memory only.
    pub board_ready: bool,
    /// A checkout the person is asked to import into Lonca, in a window
    /// over everything (record 117).
    pub importing: Option<ImportAsk>,
    /// A checkout's teardown script a removal asked about, in a window
    /// over the `:tidy` one.
    pub teardown: Option<TeardownAsk>,
    /// The start's look for worktrees no agent owns, until it answered
    /// and its count was said.
    tidy_start: bool,
    /// The agents that were mid-turn at the last step.
    turning: BTreeSet<AgentId>,
    /// Position in the open inbox items.
    pub inbox_sel: usize,
    /// Inbox items the person put off with `S`, in the order they were put
    /// off: `W` comes to them after everything else. Kept for this run
    /// only.
    pub deferred: Vec<usize>,
    /// Home's row at hand and the project it is narrowed to.
    pub at_home: Home,
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
    /// `?` again in the help: every key, not only this screen's.
    pub help_all: bool,
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
            ps: None,
            ps_looked: None,
            ps_start: false,
            lonca_win: None,
            window: None,
            board_ready: false,
            importing: None,
            teardown: None,
            tidy_start: false,
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
            at_home: Home::default(),
            card: None,
            been: Vec::new(),
            jumps: Vec::new(),
            recall: None,
            searching: false,
            recalling: false,
            went_back: false,
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
            help_all: false,
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
        self.watch_ps(Instant::now());
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
pub(crate) mod tests;
