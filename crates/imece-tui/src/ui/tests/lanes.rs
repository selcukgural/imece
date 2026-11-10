use super::*;

use crate::model::{LANE_TICKS, Lane, LaneCell};

#[test]
fn a_lane_keeps_the_most_of_a_tick_fills_a_jump_and_forgets_past_an_hour() {
    use LaneCell::*;
    let mut lane = Lane::default();
    assert_eq!(lane.last(), Gap);
    assert_eq!(lane.at(10, 3), [Gap, Gap, Gap], "nothing seen yet");
    lane.mark(10, Reading);
    lane.mark(10, Thinking);
    assert_eq!(lane.last(), Reading, "the most of a tick");
    lane.mark(10, Writing);
    assert_eq!(lane.last(), Writing);
    // A jump of three ticks: the two between take what was last.
    lane.mark(13, Asking);
    assert_eq!(lane.at(13, 5), [Gap, Writing, Writing, Writing, Asking]);
    // An older tick counts as the newest.
    lane.mark(12, Stuck);
    assert_eq!(lane.at(13, 2), [Writing, Stuck]);
    // A turn's end is one tick: the ticks after it are idle.
    lane.mark(14, Done);
    lane.mark(17, Thinking);
    assert_eq!(lane.at(17, 4), [Done, Idle, Idle, Thinking]);
    // Ticks after the newest seen, and before tick 0, are gaps.
    assert_eq!(lane.at(19, 3), [Thinking, Gap, Gap]);
    assert_eq!(Lane::default().at(1, 3), [Gap, Gap, Gap]);
    let mut early = Lane::default();
    early.mark(1, Idle);
    assert_eq!(early.at(1, 4), [Gap, Gap, Gap, Idle]);
    // An hour is kept, no more.
    let mut long = Lane::default();
    long.mark(0, Writing);
    long.mark(LANE_TICKS as u64, Reading);
    let all = long.at(LANE_TICKS as u64, LANE_TICKS + 1);
    assert_eq!(all[0], Gap, "the first tick is gone");
    assert_eq!(all[1], Writing);
    assert_eq!(all[LANE_TICKS], Reading);
    // A jump of more than an hour keeps an hour of what was last.
    long.mark(10 * LANE_TICKS as u64, Idle);
    let all = long.at(10 * LANE_TICKS as u64, LANE_TICKS);
    assert_eq!((all[0], all[LANE_TICKS - 1]), (Reading, Idle));
    // In a turn: starting, thinking, reading, writing, asking.
    let busy: Vec<bool> = [
        Gap, Idle, Starting, Thinking, Reading, Writing, Asking, Done, Waiting, Stuck,
    ]
    .iter()
    .map(|c| c.busy())
    .collect();
    assert_eq!(
        busy,
        [
            false, false, true, true, true, true, true, false, false, false
        ]
    );
}

#[test]
fn a_lane_says_what_the_agent_does_in_a_turn_and_waiting_only_out_of_one() {
    use crate::app::{cell_of, writes};
    let (mut app, w, _) = calm_team();
    app.world.inbox.clear();
    let a = w[0];
    let cell = |app: &App, waits| cell_of(&app.world, &app.world.agents[a], waits);
    let set = |app: &mut App, status: Status, doing: Option<&str>| {
        app.world.agents[a].status = status;
        app.world.agents[a].activity = doing.map(str::to_string);
    };
    set(&mut app, Status::Working, None);
    assert_eq!(cell(&app, true), LaneCell::Thinking, "in a turn, its work");
    set(&mut app, Status::Working, Some(""));
    assert_eq!(cell(&app, false), LaneCell::Thinking);
    set(&mut app, Status::Working, Some("Edit src/x.rs"));
    assert_eq!(cell(&app, false), LaneCell::Writing);
    set(&mut app, Status::Working, Some("Read src/x.rs"));
    assert_eq!(cell(&app, false), LaneCell::Reading);
    app.world.agents[a].starting = true;
    assert_eq!(cell(&app, false), LaneCell::Starting);
    app.world.agents[a].starting = false;
    app.world.inbox.push(InboxItem {
        kind: InboxKind::Approval,
        agent: a,
        detail: "cargo test".into(),
        researcher: None,
        open: true,
        tick: 0,
        card: None,
    });
    assert_eq!(cell(&app, true), LaneCell::Waiting, "a yes it waits for");
    app.world.inbox.clear();
    set(&mut app, Status::Asking { to: w[1], until: 0 }, None);
    assert_eq!(cell(&app, true), LaneCell::Asking);
    set(
        &mut app,
        Status::Researching {
            for_agent: w[1],
            until: 0,
        },
        None,
    );
    assert_eq!(cell(&app, true), LaneCell::Reading);
    set(&mut app, Status::Blocked, None);
    assert_eq!(cell(&app, false), LaneCell::Stuck);
    set(&mut app, Status::Stopped, None);
    assert_eq!(cell(&app, false), LaneCell::Stuck);
    set(&mut app, Status::WaitingHuman, None);
    assert_eq!(cell(&app, false), LaneCell::Waiting);
    for status in [Status::Idle, Status::Asleep] {
        set(&mut app, status, None);
        app.world.agents[a].lane = Lane::default();
        assert_eq!(
            cell(&app, true),
            LaneCell::Waiting,
            "out of a turn, what waits"
        );
        assert_eq!(cell(&app, false), LaneCell::Idle);
        app.world.agents[a].lane.mark(1, LaneCell::Reading);
        assert_eq!(cell(&app, false), LaneCell::Done, "the tick a turn ended");
    }
    for (doing, yes) in [
        ("Edit a", true),
        ("Write a", true),
        ("MultiEdit a", true),
        ("NotebookEdit a", true),
        ("Read a", false),
        ("run cargo test", false),
        ("Editor a", false),
        ("", false),
    ] {
        assert_eq!(writes(doing), yes, "{doing}");
    }
}

#[test]
fn every_step_notes_each_agent_in_its_lane_but_a_gone_one() {
    let (mut app, w, _) = calm_team();
    app.world.inbox.clear();
    app.world.agents[w[0]].activity = Some("Edit x".into());
    app.world.agents[w[1]].gone = true;
    app.step();
    let t = app.world.tick;
    assert_eq!(app.world.agents[w[0]].lane.at(t, 1)[0], LaneCell::Writing);
    assert_eq!(app.world.agents[w[1]].lane.last(), LaneCell::Gap);
    app.world.inbox.push(item(InboxKind::Question, w[2]));
    app.world.agents[w[2]].status = Status::Idle;
    app.watch_lanes();
    let t = app.world.tick;
    assert_eq!(app.world.agents[w[2]].lane.at(t, 1)[0], LaneCell::Waiting);
    // A reply to read is no wait on the person: its agent is idle.
    app.world.inbox.clear();
    app.world.agents[w[2]].lane = Lane::default();
    let mut reply = item(InboxKind::Reply, w[2]);
    reply.open = true;
    app.world.inbox.push(reply);
    app.world.tick += 1;
    app.watch_lanes();
    let t = app.world.tick;
    assert_eq!(app.world.agents[w[2]].lane.at(t, 1)[0], LaneCell::Idle);
}

#[test]
fn columns_keep_the_most_of_their_ticks_and_messages_count_by_column() {
    use LaneCell::*;
    let mut lane = Lane::default();
    for (t, c) in [
        (1, Idle),
        (2, Writing),
        (3, Idle),
        (4, Thinking),
        (5, Idle),
        (6, Idle),
    ] {
        lane.mark(t, c);
    }
    assert_eq!(timeline::columns(&lane, 6, 3, 2), [Writing, Thinking, Idle]);
    assert_eq!(timeline::columns(&lane, 6, 2, 1), [Idle, Idle]);
    let (mut app, w, _) = calm_team();
    app.world.tick = 100;
    let lead = app.world.projects[0].leader;
    let at = |tick, from| Entry {
        tick,
        event: sent(from, lead, MsgKind::Inform, 1, None),
    };
    app.world.journal = vec![
        at(93, w[0]),
        at(94, w[0]),
        at(95, w[1]),
        at(95, w[1]),
        at(99, w[2]),
        at(100, w[3]),
        at(100, w[3]),
        at(100, w[3]),
    ];
    // Three columns of two ticks: 95-96, 97-98, 99-100.
    assert_eq!(timeline::messages(&app.world, 0, 100, 3, 2), [2, 0, 4]);
    assert_eq!(timeline::messages(&app.world, 0, 100, 4, 2), [2, 2, 0, 4]);
    assert_eq!(timeline::messages(&app.world, 1, 100, 3, 2), [0, 0, 0]);
    assert_eq!(timeline::messages(&app.world, 0, 100, 1, 1), [3]);
}

#[test]
fn a_lane_s_width_leaves_room_for_the_name_and_the_status_or_drops_the_status() {
    // Name 20, lane at least 10, status 28, a space each side.
    assert_eq!(timeline::lane_width(60), (10, true));
    assert_eq!(timeline::lane_width(100), (50, true));
    assert_eq!(timeline::lane_width(59), (38, false));
    assert_eq!(timeline::lane_width(30), (9, false));
    assert_eq!(timeline::lane_width(5), (1, false));
    let glyphs: String = [
        LaneCell::Gap,
        LaneCell::Idle,
        LaneCell::Starting,
        LaneCell::Thinking,
        LaneCell::Reading,
        LaneCell::Writing,
        LaneCell::Asking,
        LaneCell::Done,
        LaneCell::Waiting,
        LaneCell::Stuck,
    ]
    .iter()
    .map(|&c| timeline::cell_glyph(c).0)
    .collect();
    assert_eq!(glyphs, " ·●░▒█?✓⚑■");
}

#[test]
fn the_timeline_shows_each_open_project_s_agents_over_time_and_its_keys_work() {
    let mut app = app_after(2, 2, 30, Lang::En);
    let (worker, [h1, h2]) = with_helpers(&mut app);
    let p = app.world.agents[worker].project;
    let lead = app.world.projects[p].leader;
    let shown = timeline::agents(&app);
    let at = shown.iter().position(|&(id, _)| id == worker).unwrap();
    assert_eq!(&shown[at..at + 3], [(worker, 0), (h1, 1), (h2, 1)]);
    assert_eq!(shown.iter().filter(|&&(id, _)| id == h1).count(), 1, "once");
    assert!(shown.contains(&(lead, 0)));
    // Gone, settled, of a closed project: not there.
    app.world.agents[h2].gone = true;
    app.settled.insert(h1);
    assert!(
        !timeline::agents(&app)
            .iter()
            .any(|&(id, _)| id == h1 || id == h2)
    );
    let other = 1 - p;
    app.world.projects[other].closed = true;
    assert!(
        timeline::agents(&app)
            .iter()
            .all(|&(id, _)| app.world.agents[id].project == p)
    );
    app.world.projects[other].closed = false;
    // A project put out of the salon is not there either, open as it is.
    app.settled_projects.insert(other);
    assert!(
        timeline::agents(&app)
            .iter()
            .all(|&(id, _)| app.world.agents[id].project == p)
    );
    app.settled_projects.clear();
    assert!(
        timeline::agents(&app)
            .iter()
            .any(|&(id, _)| app.world.agents[id].project == other)
    );
    app.settled.clear();
    app.world.agents[h2].gone = false;
    // `]` in the salon is a panel, not the scale.
    key(&mut app, KeyCode::Char(']'));
    assert_eq!(app.timeline_scale, 0);
    for c in ":timeline".chars() {
        key(&mut app, KeyCode::Char(c));
    }
    key(&mut app, KeyCode::Enter);
    assert_eq!(app.window, Some(Window::Timeline));
    assert_eq!(app.key_scope(), Some(crate::keys::Scope::Timeline));
    let s = render(&app, 160, 40);
    assert!(s.contains("TIMELINE · a column is 5 s"), "{s}");
    assert!(s.contains("█ writing"), "the legend: {s}");
    assert!(s.contains(" now"), "{s}");
    assert!(s.contains("  messages"), "{s}");
    assert!(
        s.lines().last().unwrap().trim_start().starts_with("[ ]"),
        "the footer: {s}"
    );
    let name = app.world.agents[h1].name.clone();
    let row = s.lines().find(|l| l.contains(&name)).unwrap();
    assert!(
        row.contains(&format!("  {name}")),
        "a helper is indented: {row}"
    );
    // The scale, held within its bounds.
    key(&mut app, KeyCode::Char(']'));
    assert!(render(&app, 160, 40).contains("a column is 30 s"));
    key(&mut app, KeyCode::Char(']'));
    key(&mut app, KeyCode::Char(']'));
    assert_eq!(app.timeline_scale, 2);
    assert!(render(&app, 160, 40).contains("a column is 1 min"));
    key(&mut app, KeyCode::Char('['));
    key(&mut app, KeyCode::Char('['));
    key(&mut app, KeyCode::Char('['));
    assert_eq!(app.timeline_scale, 0);
    // j k gg G, the selected row reversed; enter its focus, K a peek.
    let n = timeline::agents(&app).len();
    key(&mut app, KeyCode::Char('j'));
    assert_eq!(app.timeline_sel, 1);
    key(&mut app, KeyCode::Char('G'));
    assert_eq!(app.timeline_sel, n - 1);
    key(&mut app, KeyCode::Char('j'));
    assert_eq!(app.timeline_sel, n - 1, "the last stays the last");
    key(&mut app, KeyCode::Char('g'));
    key(&mut app, KeyCode::Char('g'));
    assert_eq!(app.timeline_sel, 0);
    key(&mut app, KeyCode::Char('k'));
    assert_eq!(app.timeline_sel, 0);
    app.timeline_sel = at;
    let s = render(&app, 160, 40);
    let wname = app.world.agents[worker].name.clone();
    // The lane, under Home's preview that names the worker too.
    let y = s
        .lines()
        .collect::<Vec<_>>()
        .into_iter()
        .rposition(|l| l.contains(&format!(" {wname} ")))
        .unwrap();
    let x = s
        .lines()
        .nth(y)
        .unwrap()
        .chars()
        .position(|c| c == '▎' || c.is_alphanumeric())
        .unwrap();
    assert!(
        buffer(&app, 160, 40)[(x as u16, y as u16)]
            .modifier
            .contains(Modifier::REVERSED)
    );
    let y2 = s
        .lines()
        .position(|l| l.contains(&format!("  {name}")))
        .unwrap();
    assert!(
        !buffer(&app, 160, 40)[(x as u16, y2 as u16)]
            .modifier
            .contains(Modifier::REVERSED)
    );
    // The window takes `K`; `enter` opens the focus in its place.
    key(&mut app, KeyCode::Char('K'));
    assert_eq!(app.peek, None);
    key(&mut app, KeyCode::Enter);
    assert_eq!((app.window, app.zoom), (None, Zoom::Focus(worker)));
    key(&mut app, KeyCode::Esc);
    assert_eq!((app.zoom), (Zoom::None));
    // A narrow screen: the status goes; Turkish words.
    app.window = Some(Window::Timeline);
    app.lang = Lang::Tr;
    let s = render(&app, 55, 30);
    assert!(s.contains("ZAMAN ŞERİDİ · bir sütun 5 sn"), "{s}");
    assert!(!s.contains("koordine ediyor"), "{s}");
    // :timeline typed from the salon.
    key(&mut app, KeyCode::Esc);
    assert_eq!(app.window, None);
    for c in ":timeline".chars() {
        key(&mut app, KeyCode::Char(c));
    }
    key(&mut app, KeyCode::Enter);
    assert_eq!(app.window, Some(Window::Timeline));
}

#[test]
fn the_timeline_keeps_the_selected_agent_in_view() {
    let mut app = app_after(1, 12, 0, Lang::En);
    app.window = Some(Window::Timeline);
    let n = timeline::agents(&app).len();
    app.timeline_sel = n - 1;
    let s = render(&app, 120, 14);
    let last = timeline::agents(&app)[n - 1].0;
    assert!(s.contains(&app.world.agents[last].name), "{s}");
    let first = timeline::agents(&app)[0].0;
    // In the window: under it Home's last row is the leader's line.
    let lanes: Vec<&str> = (s.lines())
        .skip_while(|l| !l.contains('┌'))
        .take_while(|l| !l.contains('└'))
        .collect();
    assert!(lanes.len() > 3, "{s}");
    assert!(
        !lanes
            .iter()
            .any(|l| l.contains(&format!(" {} ", app.world.agents[first].name))),
        "{s}"
    );
    app.world.projects[0].closed = true;
    assert!(render(&app, 120, 14).contains("No agent to show."));
}

#[test]
fn the_axis_says_when_its_first_and_middle_columns_were_and_now() {
    let t = 1000;
    let row = timeline::axis(Lang::En, t, 40, 1);
    assert_eq!(row.chars().count(), 40);
    assert!(row.starts_with(&clock(t - 39)), "{row}");
    assert_eq!(&row[18..23], clock(t - 21));
    assert!(row.ends_with("now"), "{row}");
    // Five minutes a column: the middle clock is 21 columns back, not 17.
    let row = timeline::axis(Lang::En, 10_000, 40, 60);
    assert_eq!(&row[18..23], clock(10_000 - 21 * 60), "{row}");
    let row = timeline::axis(Lang::Tr, t, 40, 6);
    assert!(
        row.starts_with(&clock(t - 39 * 6)) && row.ends_with("şimdi"),
        "{row}"
    );
    let short = timeline::axis(Lang::En, t, 23, 1);
    assert_eq!(short.trim(), "now", "too short for clocks");
    assert!(timeline::axis(Lang::En, t, 24, 1).starts_with(&clock(t - 23)));
    // A lane's glyphs in runs of one look.
    let spans = timeline::lane_spans(&[LaneCell::Writing, LaneCell::Writing, LaneCell::Idle]);
    let text: Vec<&str> = spans.iter().map(|s| s.content.as_ref()).collect();
    assert_eq!(text, ["██", "·"]);
}
