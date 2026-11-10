use ratatui::Terminal;
use ratatui::backend::TestBackend;

use ratatui::crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

use super::footer::*;
use super::header::*;
use super::*;
use crate::app::{App, HomeRow, Pane, Sel};
use crate::i18n::Lang;

mod cards;
mod change;
mod footer;
mod header;
mod inbox;
mod journal;
mod keys;
mod lanes;
mod line;
mod links;
mod markdown;
mod motion;
mod projects;
mod rows;
mod talk;
mod theme;
mod windows;

fn render(app: &App, width: u16, height: u16) -> String {
    let mut t = Terminal::new(TestBackend::new(width, height)).unwrap();
    t.draw(|f| draw(f, app)).unwrap();
    let buf = t.backend().buffer();
    buf.content
        .chunks(usize::from(width))
        .map(|row| row.iter().map(|c| c.symbol()).collect::<String>())
        .collect::<Vec<_>>()
        .join("\n")
}

fn app_after(projects: usize, workers: usize, ticks: usize, lang: Lang) -> App {
    let mut app = App::new(projects, workers, 4, 7, lang);
    for _ in 0..ticks {
        app.step();
    }
    app
}

/// A fresh directory of this test's own under the system's temp one.
fn scratch(name: &str) -> std::path::PathBuf {
    let dir = std::env::temp_dir().join(format!("imece-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

/// Two helpers of the first worker of `app`'s first project; the worker
/// and the helpers' ids.
fn with_helpers(app: &mut App) -> (AgentId, [AgentId; 2]) {
    let worker = app
        .world
        .agents
        .iter()
        .find(|a| a.project == 0 && a.role == Role::Worker)
        .unwrap()
        .id;
    let name = app.world.agents[worker].name.clone();
    let mut ids = [0; 2];
    for (n, id) in ids.iter_mut().enumerate() {
        *id = app.world.agents.len();
        let mut h = Agent::new(
            *id,
            0,
            format!("{name}.{}", n + 1),
            Role::Helper,
            Status::Working,
            app.world.tick,
        );
        h.parent = Some(worker);
        h.activity = Some("Edit src/x.rs".into());
        app.world.agents.push(h);
    }
    (worker, ids)
}

/// One project whose agents are all calm and working for a long time:
/// the leader, the workers in `w`, the researchers in `r`.
fn calm_team() -> (App, Vec<AgentId>, Vec<AgentId>) {
    let mut app = app_after(1, 4, 0, Lang::En);
    let id = app.world.agents.len();
    app.world.agents.push(Agent::new(
        id,
        0,
        "res-1".into(),
        Role::Researcher,
        Status::Working,
        0,
    ));
    app.world.tick = 100;
    app.selected = Sel::Project(0);
    for a in &mut app.world.agents {
        a.status = Status::Working;
        a.talk = None;
        a.changed_at = 0;
        a.gone = false;
    }
    let of = |app: &App, role: Role| -> Vec<AgentId> {
        app.world
            .agents
            .iter()
            .filter(|a| a.role == role)
            .map(|a| a.id)
            .collect()
    };
    let w = of(&app, Role::Worker);
    let r = of(&app, Role::Researcher);
    assert!(w.len() == 4 && !r.is_empty());
    (app, w, r)
}

/// An open answer of `agent` in TO DECIDE.
fn waiting(app: &mut App, agent: AgentId) {
    app.world.inbox.push(InboxItem {
        kind: InboxKind::Reply,
        agent,
        detail: "an answer".into(),
        researcher: None,
        open: true,
        tick: 0,
        card: None,
    });
}

fn plan_of(app: &mut App, a: AgentId) {
    app.world.agents[a].plan = vec![
        Step {
            text: "Read the code".into(),
            state: StepState::Done,
        },
        Step {
            text: "Run the tests".into(),
            state: StepState::Doing,
        },
        Step {
            text: "Write it up".into(),
            state: StepState::Doing,
        },
        Step {
            text: "Ship".into(),
            state: StepState::Todo,
        },
    ];
}

/// The screen of `app` drawn whole on a terminal of `width` columns:
/// its cells and the cursor.
fn whole(app: &App, width: u16) -> (Buffer, Option<Position>) {
    let mut t = Terminal::new(TestBackend::new(width, 40)).unwrap();
    t.draw(|f| draw(f, app)).unwrap();
    let cursor = app.caret_at.get().map(|_| t.get_cursor_position().unwrap());
    (t.backend().buffer().clone(), cursor)
}

fn key(app: &mut App, code: KeyCode) {
    app.on_key(KeyEvent::from(code));
}

fn a_change(agent: AgentId) -> Change {
    Change {
        agent,
        title: "Fix the parser".into(),
        files: vec![
            ChangedFile {
                path: "src/p.rs".into(),
                added: 2,
                removed: 1,
                binary: false,
            },
            ChangedFile {
                path: "src/q.rs".into(),
                added: 1,
                removed: 0,
                binary: false,
            },
        ],
        patch: "diff --git a/src/p.rs b/src/p.rs\n--- a/src/p.rs\n+++ b/src/p.rs\n\
                @@ -1,2 +1,3 @@\n context\n-OLD-LINE\n+NEW-LINE\n+ANOTHER\n\
                diff --git a/src/q.rs b/src/q.rs\n--- a/src/q.rs\n+++ b/src/q.rs\n\
                @@ -1 +1,2 @@\n q\n+Q-LINE\x07\n"
            .into(),
        runs: Vec::new(),
        check: Check::None,
        spend: None,
    }
}

fn spans_text(spans: &[Span<'_>]) -> String {
    spans.iter().map(|s| s.content.as_ref()).collect()
}

fn sent(from: AgentId, to: AgentId, kind: MsgKind, id: MsgId, re: Option<MsgId>) -> Event {
    Event::Sent {
        from,
        to,
        kind,
        text: format!("m{id}"),
        id,
        re,
    }
}

/// The last row of a drawing of `app`, as wide as `width`.
fn foot(app: &App, width: u16) -> String {
    render(app, width, 40).lines().last().unwrap().to_string()
}

fn item(kind: InboxKind, agent: AgentId) -> InboxItem {
    InboxItem {
        kind,
        agent,
        detail: "the words".into(),
        researcher: None,
        open: true,
        tick: 0,
        card: None,
    }
}

/// Whether the cell at `(x, y)` links to `url` (the web addresses' id).
pub(crate) fn links_to(buf: &Buffer, x: u16, y: u16, url: &str) -> bool {
    let open = format!("\x1b]8;id=imece-{LINK_ID};{url}\x1b\\");
    buf[(x, y)].symbol().starts_with(&open)
}

/// A world whose journal is only a conversation of the person and the
/// leader, with another pair talking in between.
fn talking() -> App {
    let mut app = app_after(1, 4, 0, Lang::En);
    app.world.journal.clear();
    let you = |text: &str| Event::YouSaid {
        to: 0,
        text: text.into(),
    };
    let lead = |text: &str| Event::Replied {
        agent: 0,
        text: text.into(),
    };
    app.world.note(you("check the readme"));
    app.world.note(lead(
        "# Readme\nIt is one line: `# demo`.\n- no usage\n```\ncargo run\n```",
    ));
    app.world.note(Event::Talk {
        from: 1,
        to: 2,
        text: "between workers".into(),
    });
    app.world.note(you("assign it"));
    app.world
        .note(lead(&format!("{} THE-END", "long ".repeat(300))));
    app
}

/// `talking()` with its first two messages on the simulation's first
/// day and the rest on the second (the sim's clock starts at 14:00, so
/// tick 7200 is the next midnight); the world is in the second day.
fn talking_over_two_days() -> App {
    let mut app = talking();
    for (i, e) in app.world.journal.iter_mut().enumerate() {
        e.tick = if i < 3 { 0 } else { 7200 };
    }
    app.world.tick = 7300;
    app
}

fn alt(app: &mut App, code: KeyCode) {
    app.on_key(KeyEvent::new(
        code,
        ratatui::crossterm::event::KeyModifiers::ALT,
    ));
}

fn write(app: &mut App, text: &str) {
    for c in text.chars() {
        key(app, KeyCode::Char(c));
    }
}

fn with(app: &mut App, code: KeyCode, mods: KeyModifiers) {
    app.on_key(KeyEvent::new(code, mods));
}

/// Where the terminal's cursor is after `app` is drawn on a screen of
/// `width` by `height`; none when it is hidden.
fn caret_of(app: &App, width: u16, height: u16) -> Option<(u16, u16)> {
    let mut t = Terminal::new(TestBackend::new(width, height)).unwrap();
    t.draw(|f| draw(f, app)).unwrap();
    let b = t.backend();
    b.cursor_visible()
        .then(|| (b.cursor_position().x, b.cursor_position().y))
}

fn ctrl(app: &mut App, c: char) {
    app.on_key(KeyEvent::new(
        KeyCode::Char(c),
        ratatui::crossterm::event::KeyModifiers::CONTROL,
    ));
}

fn buffer(app: &App, width: u16, height: u16) -> ratatui::buffer::Buffer {
    let mut t = Terminal::new(TestBackend::new(width, height)).unwrap();
    t.draw(|f| draw(f, app)).unwrap();
    t.backend().buffer().clone()
}

fn text(rows: &[Line]) -> Vec<String> {
    rows.iter()
        .map(|l| l.spans.iter().map(|s| s.content.as_ref()).collect())
        .collect()
}

/// A quiet team at tick 100: everyone idle since now, nothing said,
/// nothing waiting, nothing new.
fn quiet_rows() -> App {
    let mut app = app_after(1, 4, 50, Lang::En);
    app.world.tick = 100;
    app.world.inbox.clear();
    app.unseen.clear();
    for a in &mut app.world.agents {
        a.status = Status::Idle;
        a.activity = None;
        a.talk = None;
        a.topic.clear();
        a.starting = false;
        a.changed_at = 100;
        a.local = None;
        a.plan.clear();
        a.context = None;
        a.cost = 0;
    }
    app
}

/// Agent `id`'s row, `width` wide, as one string.
fn row(app: &App, id: AgentId, width: usize) -> String {
    salon::agent_line(app, &app.world.agents[id], "", &app.rungs(), width)
        .spans
        .iter()
        .map(|s| s.content.as_ref())
        .collect()
}

fn parts(spans: &[Span]) -> Vec<(String, Style)> {
    spans
        .iter()
        .map(|s| (s.content.to_string(), s.style))
        .collect()
}
