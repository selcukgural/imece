//! The helpers the tests of `app` share, and the tests of what `mod.rs` keeps.

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
        options: Vec::new(),
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
        let mut asked = item(i % 3, true);
        asked.kind = InboxKind::Question;
        w.inbox.push(asked);
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
    w.inbox[0].kind = InboxKind::Question;
    let l = LeftOff::of(w, 0, Some(5)).unwrap();
    assert_eq!((l.waiting.as_slice(), l.waiting_total), (&[0][..], 1));
    assert!(l.last.is_empty());
    assert_eq!(l.replies, 0);
    // A reply is to read, not to decide: counted apart.
    w.inbox[0].kind = InboxKind::Reply;
    let l = LeftOff::of(w, 0, Some(5)).unwrap();
    assert_eq!((l.waiting.len(), l.waiting_total, l.replies), (0, 0, 1));
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
    tx.send(here("2.1.3", Login::Yes { subscription: true }))
        .unwrap();
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

/// What the run spent is unpriced by a gap since it began or a turn the
/// quit cuts, never by one from before (record 132).
#[test]
fn the_run_s_figure_is_unpriced_by_a_gap_since_its_start_or_a_turn_the_quit_cuts() {
    let mut w = crate::live::testing::team(1);
    w.agents[1].unpriced.gaps = 2;
    w.agents[1].cost = 1_000;
    let live = crate::live::testing::idle_live(2);
    let mut app = App::with(w, Engine::Live(Box::new(live)), Lang::En);
    for a in &mut app.world.agents {
        a.status = Status::Idle;
    }
    assert_eq!(app.spent_this_run(), (0, false), "the gaps were there");
    app.world.agents[1].cost = 1_500;
    assert_eq!(app.spent_this_run(), (500, false));
    app.world.agents[0].status = Status::Working;
    assert_eq!(app.spent_this_run(), (500, true), "cut by the quit");
    app.world.agents[0].status = Status::Idle;
    app.world.agents[0].unpriced.session = true;
    assert_eq!(app.spent_this_run(), (500, true), "a gap since");
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
    assert_eq!((app.pane), (Pane::Inbox));
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
fn a_suggested_commit_names_the_issues_its_applied_cards_close_until_it_ran() {
    let mut app = App::new(1, 2, 0, 1, Lang::En);
    assert!(app.world.cards.len() >= 2, "the demo has cards");
    for (c, n) in [(0, 58), (1, 61)] {
        app.world.cards[c].project = 0;
        app.world.cards[c].closes = Some(n);
        app.world.cards[c].column = Column::Done;
    }
    app.world.projects[0].lonca = None;
    let suggest = |app: &mut App| {
        app.typing = false;
        app.world.suggested = Some(Suggestion {
            project: 0,
            text: "Fix it".into(),
        });
        app.show_suggestion();
        app.input.clone()
    };
    let both = "!git commit --dry-run -m 'Fix it\n\nCloses #58\nCloses #61'";
    assert_eq!(suggest(&mut app), both);
    // Off: no line at all.
    app.world.projects[0].closing = false;
    assert_eq!(suggest(&mut app), "!git commit --dry-run -m 'Fix it'");
    app.world.projects[0].closing = true;
    // A commit that failed, or a dry run, names nothing yet.
    app.ran(
        "git commit -m 'Fix it\n\nCloses #58'",
        caught(b"", Ended::Exit(1)),
    );
    app.ran(&both[1..], caught(b"", Ended::Exit(0)));
    assert_eq!(suggest(&mut app), both);
    // One that ran names what its message says: the next leaves it out.
    app.ran(
        "git commit -m 'Fix it\n\nCloses #58'",
        caught(b"", Ended::Exit(0)),
    );
    assert_eq!(app.world.cards[0].item, ItemState::Named);
    assert_eq!(app.world.cards[1].item, ItemState::Waits);
    assert_eq!(
        suggest(&mut app),
        "!git commit --dry-run -m 'Fix it\n\nCloses #61'"
    );
}

#[test]
fn o_quotes_the_last_output_of_the_project_to_the_agent_at_hand() {
    let mut app = App::new(2, 2, 0, 1, Lang::En);
    let leader = app.world.projects[1].leader;
    let other = (0..app.world.agents.len())
        .find(|&a| app.world.agents[a].project == 1 && a != leader)
        .expect("project 1 has an agent besides its leader");
    // Nothing ran yet: it says so and nothing is written.
    app.at_home.at = Some(HomeRow::Project(1));
    app.home_settle();
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
    app.at_home.at = Some(HomeRow::Project(0));
    app.home_settle();
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

/// A demo app whose hired worker's new worktree asks about the setup
/// script, the ask selected in TO DECIDE.
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

pub(super) fn dir(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("imece-app-keys-{name}-{}", std::process::id()));
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

/// A demo app whose leader's plan waits in TO DECIDE, selected
/// in the inbox pane: the app, the item and the leader.
pub(crate) fn asking_plan() -> (App, usize, AgentId) {
    let mut app = waiting(&[]);
    let leader = app.world.projects[0].leader;
    let cards = vec![
        Planned {
            title: "Parser".into(),
            tier: "cheap".into(),
            paths: vec![Owned::new("src/p.rs"), Owned::shared("docs")],
            checks: Vec::new(),
            closes: None,
            body: "Fix it.\nThen test.".into(),
            after: Vec::new(),
        },
        Planned {
            title: "Docs".into(),
            tier: "standard".into(),
            paths: Vec::new(),
            checks: Vec::new(),
            closes: None,
            body: "Write them.".into(),
            after: Vec::new(),
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

/// The acceptance run's words: a message refused for the pot named the
/// leader ("lead has spent its budget"); the pot is the team's.
#[test]
fn a_message_refused_for_the_pot_names_the_pot_and_budget() {
    let mut app = App::new(1, 1, 0, 1, Lang::En);
    app.world.budget = app.world.spent();
    let lead = app.world.agents[app.world.projects[0].leader].name.clone();
    app.typing = true;
    app.input = "hello".into();
    app.submit();
    let said = flashed(&app).unwrap();
    assert_eq!(said, crate::ui::words::pot_spent(Lang::En, &app.world));
    assert!(
        said.contains("pot is spent") && said.contains(":budget"),
        "{said}"
    );
    assert!(!said.contains(&lead), "{said}");
}
