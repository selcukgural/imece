use super::*;

#[test]
fn a_focus_footer_names_the_keys_that_decide_what_its_agent_waits_for() {
    let mut app = app_after(1, 2, 0, Lang::En);
    let foot = |app: &App| render(app, 160, 30).lines().last().unwrap().to_string();
    let id = app.world.projects[0].leader + 1;
    app.zoom = Zoom::Focus(id);
    let calm = foot(&app);
    assert!(
        calm.contains("   enter  trace   @  write to this agent   esc  back   ?  help"),
        "{calm}"
    );
    for gone in [" a ", " A ", " r "] {
        assert!(!calm.contains(gone), "{calm}");
    }
    // An ask of another agent says nothing here.
    app.world.asks_you(id + 1, "Bash: make");
    assert_eq!(foot(&app), calm);
    app.world.asks_you(id, "Bash: make");
    let asked = foot(&app);
    // Three keys for the ask, the way back and the help; `A` is in the help.
    assert!(
        asked.contains("   a  allow once   r  refuse   e  yes this turn   esc  back   ?  help"),
        "{asked}"
    );
    assert!(!asked.contains(" A "), "{asked}");
    app.zoom = Zoom::Trace(id);
    let traced = foot(&app);
    assert!(
        traced.contains("   a  allow once   r  refuse   e  yes this turn   esc  back   ?  help"),
        "{traced}"
    );
    // A window over it has keys of its own.
    app.peek = Some(id);
    assert!(!foot(&app).contains("allow once"), "{}", foot(&app));
    app.peek = None;
    // Its change, once its ask is decided.
    app.world.decided(1, true);
    app.world.change_ready(crate::model::Change {
        agent: id,
        title: "did it".into(),
        files: Vec::new(),
        patch: String::new(),
        runs: Vec::new(),
        check: Check::None,
        spend: None,
    });
    app.zoom = Zoom::Focus(id);
    let change = foot(&app);
    assert!(
        change.contains("   a  approve   r  send back   esc  back   ?  help"),
        "{change}"
    );
    app.lang = Lang::Tr;
    app.world.asks_you(id, "Bash: make");
    let tr = foot(&app);
    assert!(
        tr.contains("a  bir kez izin ver   r  reddet   e  bu tur evet   esc  geri   ?  yardım"),
        "{tr}"
    );
}

#[test]
fn a_question_and_a_fault_name_the_keys_they_have() {
    let mut app = app_after(1, 2, 0, Lang::En);
    let agent = app.world.agents[1].id;
    app.pane = Pane::Inbox;
    app.world.inbox.clear();
    app.world.inbox.push(item(InboxKind::Question, agent));
    let s = render(&app, 160, 40);
    assert!(s.contains(" a  answer   enter  open"), "{s}");
    assert!(!s.contains("a  approve"), "{s}");
    app.world.inbox[0] = item(InboxKind::Fault, agent);
    let s = render(&app, 160, 40);
    assert!(s.contains(" a  answer   enter  open"), "{s}");
    assert!(!s.contains("a  approve"), "{s}");
    app.world.inbox[0] = item(InboxKind::Sensitive, agent);
    assert!(render(&app, 160, 40).contains(" a  yes   r  no"));
}

#[test]
fn an_idle_worker_with_a_change_waiting_says_so_and_not_idle() {
    let mut app = app_after(1, 2, 0, Lang::En);
    let a = app.world.agents[1].id;
    app.world.agents[a].status = Status::Idle;
    let text = |app: &App| {
        let spans = doing_status(Lang::En, &app.world, &app.world.agents[a]);
        spans.iter().map(|s| s.content.as_ref()).collect::<String>()
    };
    assert_eq!(text(&app), "idle");
    app.world.inbox.push(item(InboxKind::Change, a));
    assert_eq!(text(&app), "change ready");
}

#[test]
fn the_footer_leads_with_the_next_decision_while_one_waits() {
    let mut app = app_after(1, 2, 0, Lang::En);
    let agent = app.world.agents[1].id;
    app.world.inbox.clear();
    assert!(!foot(&app, 200).contains("next for you"));
    app.world.inbox.push(item(InboxKind::Question, agent));
    let row = foot(&app, 200);
    assert!(
        row.contains("   a  answer   enter  open   s  later   ?  help"),
        "{row}"
    );
    // The demo's pause is not a key of the footer: the header says PAUSED.
    assert!(!row.contains("space"), "{row}");
}

#[test]
fn under_a_conversation_the_footer_has_no_keys_of_the_salon() {
    let mut app = talking();
    assert!(foot(&app, 200).contains("open"));
    app.open_thread(4);
    let row = foot(&app, 200);
    assert!(row.contains("esc") && row.contains("help"), "{row}");
    assert!(!row.contains("write to the lead"), "{row}");
    assert!(!row.contains("space"), "{row}");
}

#[test]
fn a_short_footer_loses_the_last_keys_first_and_never_the_way_back_or_the_help() {
    let hints = vec![("a", "one"), ("b", "two"), ("?", "help")];
    let all = spans_width(&hint_spans(&hints));
    let names = |h: &[(&str, &str)]| h.iter().map(|k| k.0.to_string()).collect::<Vec<_>>();
    // Exactly as wide as the room: everything stays.
    assert_eq!(names(&fit_hints(hints.clone(), all)), ["a", "b", "?"]);
    // A cell short: the one before the help goes.
    assert_eq!(names(&fit_hints(hints.clone(), all - 1)), ["a", "?"]);
    // The way back and the help stay, however short the room.
    assert_eq!(names(&fit_hints(hints, 0)), ["?"]);
    let h = fit_hints(vec![("esc", "back"), ("b", "two"), ("?", "help")], 0);
    assert_eq!(names(&h), ["esc", "?"]);
}

#[test]
fn the_status_line_names_home_s_row_at_hand_and_elsewhere_the_agent_at_hand() {
    let mut app = app_after(1, 2, 0, Lang::En);
    let agent = app.world.agents[1].id;
    let other = app.world.agents[2].id;
    // A card's id is its place in the list.
    let second_id = app.world.cards.len();
    let second_shown = second_id + 1;
    let mut second = app.world.cards[0].clone();
    second.id = second_id;
    second.title = "the other card".into();
    app.world.cards.push(second);
    app.world.agents[agent].card = Some(app.world.cards[0].id);
    app.world.agents[other].card = Some(second_id);
    let first = app.world.cards[0].id;
    app.world.inbox.clear();
    for a in &mut app.world.agents {
        a.status = Status::Idle;
        a.starting = false;
    }
    // A screen over Home is about its own agent, not the row's.
    app.zoom = Zoom::Focus(other);
    let row = foot(&app, 200);
    assert!(
        row.starts_with(&format!(" focus · shop-api · #{second_shown} ")),
        "{row}"
    );
    app.zoom = Zoom::None;
    // A row that is an agent, not an item, names its card too.
    app.world.inbox.clear();
    app.world.agents[agent].status = Status::Working;
    let row = foot(&app, 200);
    assert!(
        row.starts_with(&format!(" home · shop-api · #{} ", first + 1)),
        "{row}"
    );
}

#[test]
fn the_footer_has_at_most_three_keys_for_the_selected_thing_then_the_way_back_and_the_help() {
    let mut app = app_after(1, 2, 0, Lang::En);
    let agent = app.world.agents[1].id;
    app.world.agents[agent].card = Some(app.world.cards[0].id);
    app.world.inbox.clear();
    let badges = |row: &str| -> Vec<String> {
        // A key badge is ` k ` between two spaces of the label before it.
        let keys: Vec<&str> = row.split("   ").collect();
        keys.iter()
            .skip(1)
            .filter_map(|k| k.split("  ").next())
            .map(|k| k.trim().to_string())
            .filter(|k| !k.is_empty())
            .collect()
    };
    app.zoom = Zoom::Diff(agent);
    assert_eq!(badges(&foot(&app, 200)), ["a", "r", "c", "esc", "?"]);
    // A clash's `b` comes first and the note goes, so there are still three.
    app.world.change_ready(crate::model::Change {
        agent,
        title: "did it".into(),
        files: Vec::new(),
        patch: String::new(),
        runs: Vec::new(),
        check: Check::None,
        spend: None,
    });
    app.world.change_clashed(
        agent,
        Clashes {
            head: "abc1234".into(),
            hunks: vec![Clash {
                file: "x.rs".into(),
                ..Clash::default()
            }],
            ..Clashes::default()
        },
    );
    assert_eq!(badges(&foot(&app, 200)), ["b", "a", "r", "esc", "?"]);
    app.world.change_clashed(agent, Clashes::default());
    app.world.inbox.clear();
    app.zoom = Zoom::None;
    // A window has no place before its keys: the badges start the line.
    let win_badges = |app: &App| badges(&format!("x {}", foot(app, 200)));
    app.window = Some(Window::Timeline);
    assert_eq!(win_badges(&app), ["[ ]", "enter", "@", "esc", "?"]);
    app.window = Some(Window::Board);
    assert_eq!(win_badges(&app), ["esc", "?"]);
    app.window = Some(Window::Code);
    assert_eq!(win_badges(&app), ["esc", "?"]);
    app.window = None;
    assert_eq!(badges(&foot(&app, 200)), ["enter", "s", "@", "?"]);
    app.world.inbox.push(item(InboxKind::Question, agent));
    assert_eq!(badges(&foot(&app, 200)), ["a", "enter", "s", "?"]);
    app.card = Some(app.world.card_of(agent).unwrap());
    assert_eq!(badges(&foot(&app, 200)), ["enter", "n p", "esc", "?"]);
    app.card = None;
    app.peek = Some(agent);
    assert_eq!(badges(&foot(&app, 200)), ["enter", "@", "esc", "?"]);
}

#[test]
fn a_chord_s_first_key_lists_its_second_keys_as_which_key_does() {
    let mut app = app_after(1, 2, 0, Lang::En);
    assert!(!foot(&app, 200).contains('…'));
    app.pending = Some('g');
    assert_eq!(
        foot(&app, 200).trim_end(),
        " g…   g  first line   d  to decide   w  at work"
    );
    app.pending = Some('z');
    assert_eq!(
        foot(&app, 200).trim_end(),
        " z…   a  fold · unfold   M  fold all   R  unfold all"
    );
    app.pending = Some('q');
    assert_eq!(
        foot(&app, 200).trim_end(),
        " q…   q  quit   s  what quitting leaves"
    );
    app.lang = Lang::Tr;
    assert_eq!(
        foot(&app, 200).trim_end(),
        " q…   q  çık   s  çıkınca ne kalır"
    );
    app.lang = Lang::En;
    // The person's file: its pairs that start with the key, as what they act as.
    app.keys = crate::keys::Keymap::read("gt = G\ngn = j\nx = gg\n").unwrap();
    app.pending = Some('g');
    assert_eq!(
        foot(&app, 200).trim_end(),
        " g…   g  first line   d  to decide   w  at work   t  as G   n  as j"
    );
    // A first key of the file's alone, held: only the file's pairs.
    app.pending = None;
    app.keys = crate::keys::Keymap::read("xa = G\n").unwrap();
    app.keys_held = Some((
        KeyEvent::new(KeyCode::Char('x'), KeyModifiers::NONE),
        std::time::Instant::now(),
    ));
    assert_eq!(foot(&app, 200).trim_end(), " x…   a  as G");
    app.keys_held = None;
    assert!(!foot(&app, 200).contains('…'));
}

#[test]
fn home_s_footer_leads_with_what_the_row_at_hand_takes() {
    let mut app = app_after(1, 2, 0, Lang::En);
    let agent = app.world.agents[1].id;
    for a in &mut app.world.agents {
        a.status = Status::Idle;
        a.starting = false;
    }
    let row = |app: &mut App, kind| {
        app.world.inbox.clear();
        app.world.inbox.push(item(kind, agent));
        foot(app, 200).trim_start().to_string()
    };
    assert!(
        row(&mut app, InboxKind::Change)
            .contains("   enter  read the diff   a  approve   r  send back   ?  help")
    );
    assert!(
        row(&mut app, InboxKind::Question)
            .contains("   a  answer   enter  open   s  later   ?  help")
    );
    assert!(
        row(&mut app, InboxKind::Fault).contains("   a  answer   enter  open   s  later   ?  help")
    );
    assert!(
        row(&mut app, InboxKind::Approval)
            .contains("   a  allow once   r  refuse   e  yes this turn   ?  help")
    );
    assert!(
        row(&mut app, InboxKind::Reply)
            .contains("   enter  open   r  dismiss   s  later   ?  help")
    );
    assert!(row(&mut app, InboxKind::Plan).contains("   enter  open   a  yes   r  no   ?  help"));
    for kind in [InboxKind::Tries, InboxKind::Rounds] {
        assert!(
            row(&mut app, kind).contains("   enter  open   a  one more   r  fail it   ?  help"),
            "{kind:?}"
        );
    }
    // An issue to close has nothing to open: its two answers (record 142).
    let close = row(&mut app, InboxKind::Close);
    assert!(
        close.contains("   a  close   r  leave open   ?  help"),
        "{close}"
    );
    assert!(!close.contains("enter  open"), "{close}");
    app.world.inbox.clear();
    let agent_row = foot(&app, 200);
    assert!(
        agent_row.contains("   enter  open   s  hide   @  write to this agent   ?  help"),
        "{agent_row}"
    );
}

#[test]
fn a_change_s_footer_keeps_the_help_at_any_width() {
    let mut app = app_after(1, 2, 0, Lang::En);
    app.zoom = Zoom::Diff(app.world.agents[1].id);
    assert!(foot(&app, 120).contains("help"), "{}", foot(&app, 120));
    assert!(foot(&app, 60).contains("help"), "{}", foot(&app, 60));
}

#[test]
fn which_key_lists_gd_and_gw_on_home_s_list_alone() {
    let mut app = App::new(1, 2, 0, 1, Lang::En);
    app.pending = Some('g');
    let line = which_key(&app).unwrap();
    let text: String = line.spans.iter().map(|s| s.content.as_ref()).collect();
    assert!(text.contains(" g  first line"), "{text}");
    assert!(text.contains(" d  to decide"), "{text}");
    assert!(text.contains(" w  at work"), "{text}");
    app.lang = Lang::Tr;
    let line = which_key(&app).unwrap();
    let text: String = line.spans.iter().map(|s| s.content.as_ref()).collect();
    assert!(
        text.contains(" d  karar bekleyen") && text.contains(" w  çalışan"),
        "{text}"
    );
    // Over a screen on Home, or off Home, `g…` has the one key.
    app.lang = Lang::En;
    app.zoom = Zoom::Focus(1);
    let line = which_key(&app).unwrap();
    let text: String = line.spans.iter().map(|s| s.content.as_ref()).collect();
    assert!(
        text.contains(" g  first line") && !text.contains("to decide"),
        "{text}"
    );
    app.zoom = Zoom::None;
    app.pending = Some('z');
    let line = which_key(&app).unwrap();
    let text: String = line.spans.iter().map(|s| s.content.as_ref()).collect();
    assert!(!text.contains("to decide"), "{text}");
}

#[test]
fn the_status_line_names_the_card_an_item_came_on_not_the_one_its_agent_has_now() {
    let mut app = App::new(1, 2, 0, 1, Lang::En);
    let agent = app.world.agents[1].id;
    let second_id = app.world.cards.len();
    let mut second = app.world.cards[0].clone();
    second.id = second_id;
    second.title = "the other card".into();
    app.world.cards.push(second);
    app.world.agents[agent].card = Some(second_id);
    app.world.inbox.clear();
    let mut it = item(InboxKind::Check, agent);
    it.card = Some(0);
    app.world.inbox.push(it);
    let row = foot(&app, 200);
    assert!(row.starts_with(" home · shop-api · #1 "), "{row}");
    assert!(!row.contains("the other card"), "{row}");
    // Without a card of its own, the agent's.
    app.world.inbox[0].card = None;
    let row = foot(&app, 200);
    assert!(
        row.starts_with(&format!(
            " home · shop-api · #{} the other card",
            second_id + 1
        )),
        "{row}"
    );
    // A detail opened on the item names the item's card, as its head
    // does, not the one the agent has since taken.
    app.world.inbox[0].card = Some(0);
    app.zoom = Zoom::Focus(agent);
    let row = foot(&app, 200);
    assert!(row.starts_with(" focus · shop-api · #1 "), "{row}");
    assert!(!row.contains("the other card"), "{row}");
    app.zoom = Zoom::Diff(agent);
    assert!(foot(&app, 200).starts_with(" change · shop-api · #1 "));
    // A card's own screen is the card's, and says so.
    app.card = Some(second_id);
    app.zoom = Zoom::Focus(agent);
    let row = foot(&app, 200);
    assert!(
        row.starts_with(&format!(
            " card · shop-api · #{} the other card",
            second_id + 1
        )),
        "{row}"
    );
}

#[test]
fn the_footer_grows_with_the_lines_and_keeps_the_cursor_s_in_view() {
    let mut app = app_after(1, 3, 0, Lang::En);
    let lead = app.world.agents[app.world.projects[0].leader].name.clone();
    let to = format!(" › to {lead}: ");
    key(&mut app, KeyCode::Char('/'));
    // Nothing written yet: the keys to send, after who it goes to.
    let empty = render(&app, 140, 40);
    let row = empty.lines().nth(39).unwrap();
    assert!(
        row.starts_with(&format!(
            "{to}   enter send · \\ enter new line · esc cancel"
        )),
        "{row}"
    );
    app.shift_enter = true;
    assert!(render(&app, 140, 40).contains("enter send · ⇧enter new line · esc cancel"));
    app.shift_enter = false;
    write(&mut app, "one");
    with(&mut app, KeyCode::Enter, KeyModifiers::SHIFT);
    write(&mut app, "two");
    let s = render(&app, 140, 40);
    let rows: Vec<&str> = s.lines().collect();
    let (first, last) = (rows[38], rows[39]);
    assert!(first.starts_with(&format!("{to}one")), "{first}");
    let indent = " ".repeat(to.chars().count());
    assert!(last.starts_with(&format!("{indent}two")), "{last}");
    assert!(!s.contains("enter send"), "gone once something is written");
    assert!(!s.contains("/2"), "all lines fit: no count");

    // Twelve lines: eight rows on a tall screen, four on a short one.
    for n in 3..=12 {
        with(&mut app, KeyCode::Enter, KeyModifiers::SHIFT);
        write(&mut app, &format!("line{n}"));
    }
    let s = render(&app, 140, 40);
    let rows: Vec<&str> = s.lines().collect();
    assert!(rows[32].starts_with(&format!("{to}line5")), "{}", rows[32]);
    assert!(
        rows[39].starts_with(&format!("{indent}line12")),
        "{}",
        rows[39]
    );
    assert!(rows[39].contains("12/12"), "{}", rows[39]);
    assert!(!s.contains("line4"), "out of view above");
    let short = render(&app, 140, 12);
    let rows: Vec<&str> = short.lines().collect();
    assert!(rows[8].starts_with(&format!("{to}line9")), "{}", rows[8]);
    assert!(!short.contains("line8"));
    // The cursor goes to the top: the rows follow it.
    for _ in 0..11 {
        key(&mut app, KeyCode::Up);
    }
    let s = render(&app, 140, 40);
    let rows: Vec<&str> = s.lines().collect();
    assert!(rows[32].starts_with(&format!("{to}one")), "{}", rows[32]);
    assert!(
        rows[39].starts_with(&format!("{indent}line8")),
        "{}",
        rows[39]
    );
    assert!(rows[39].contains("1/12"), "{}", rows[39]);
    assert!(!s.contains("line9"));
    // Not writing: one row again.
    key(&mut app, KeyCode::Esc);
    let s = render(&app, 140, 40);
    assert!(!s.lines().nth(38).unwrap().contains("line"));
}

#[test]
fn a_long_line_goes_on_in_the_next_row_and_the_footer_grows() {
    let mut app = app_after(1, 3, 0, Lang::En);
    let lead = app.world.agents[app.world.projects[0].leader].name.clone();
    let to = format!(" › to {lead}: ");
    let indent = " ".repeat(to.chars().count());
    let room = 60 - to.chars().count();
    key(&mut app, KeyCode::Char('/'));
    let long: String = (0..room * 2 + 5)
        .map(|i| char::from(b'a' + (i % 26) as u8))
        .collect();
    write(&mut app, &long);
    let s = render(&app, 60, 30);
    let rows: Vec<&str> = s.lines().collect();
    assert_eq!(rows[27], format!("{to}{}", &long[..room]));
    assert_eq!(
        rows[28].trim_end(),
        format!("{indent}{}", &long[room..room * 2])
    );
    assert_eq!(
        rows[29].trim_end(),
        format!("{indent}{}", &long[room * 2..])
    );
    assert_eq!(
        caret_of(&app, 60, 30),
        Some((u16::try_from(to.chars().count() + 5).unwrap(), 29))
    );
    assert_eq!(input_rows(&app, 60, 30), 3);
    // A row exactly full puts the cursor on a new one below it.
    let mut app = app_after(1, 3, 0, Lang::En);
    key(&mut app, KeyCode::Char('/'));
    write(&mut app, &long[..room]);
    assert_eq!(input_rows(&app, 60, 30), 2);
    assert_eq!(
        caret_of(&app, 60, 30),
        Some((u16::try_from(to.chars().count()).unwrap(), 29))
    );
    key(&mut app, KeyCode::Left);
    assert_eq!(input_rows(&app, 60, 30), 1, "not at its end: one row");
    // More rows than the footer has: the cursor's stays, counted.
    let mut app = app_after(1, 3, 0, Lang::En);
    key(&mut app, KeyCode::Char('/'));
    write(&mut app, &long.repeat(5));
    let s = render(&app, 60, 30);
    let last = s.lines().nth(29).unwrap();
    assert!(last.contains("11/11"), "{last}");
    assert_eq!(input_rows(&app, 60, 30), 8);
    // The rows above went out of view: the cursor is on the last row.
    assert_eq!(caret_of(&app, 60, 30).map(|(_, y)| y), Some(29));
}

#[test]
fn the_bottom_line_counts_a_close_or_forget_down() {
    // A line written and sent, as the person does (the menu closed
    // first, so enter sends the line as written).
    let typed = |app: &mut App, line: &str| {
        for c in line.chars() {
            key(app, KeyCode::Char(c));
        }
        app.menu = None;
        key(app, KeyCode::Enter);
    };
    for (lang, close, forget) in [
        (
            Lang::En,
            "◷ closing shop in 10 s · u keeps it open",
            "◷ {} forgets its session in 10 s · u keeps it",
        ),
        (
            Lang::Tr,
            "◷ shop 10 sn sonra kapanıyor · u açık tutar",
            "◷ {} 10 sn sonra oturumunu unutuyor · u tutar",
        ),
    ] {
        let mut app = app_after(1, 1, 0, lang);
        app.home = None;
        typed(&mut app, ":open ~/work/shop");
        typed(&mut app, ":close shop");
        assert!(app.grace.is_some(), "{:?}", app.flash);
        let shown = render(&app, 140, 40);
        assert!(shown.contains(close), "{shown}");
        app.flash = Some(("something else".into(), std::time::Instant::now()));
        let shown = render(&app, 140, 40);
        assert!(
            shown.contains("something else") && !shown.contains("◷"),
            "{shown}"
        );
        app.flash = None;
        app.grace = None;
        let lead = app.world.agents[0].name.clone();
        typed(&mut app, &format!("/@{lead} hello"));
        typed(&mut app, &format!(":forget @{lead}"));
        let shown = render(&app, 140, 40);
        assert!(shown.contains(&forget.replace("{}", &lead)), "{shown}");
    }
}

#[test]
fn a_note_being_written_says_where_it_goes() {
    let mut app = App::new(1, 1, 0, 1, Lang::En);
    let worker = (app.world.agents.iter())
        .find(|a| a.role == Role::Worker)
        .unwrap()
        .id;
    app.world.change_ready(Change {
        agent: worker,
        title: "did it".into(),
        files: Vec::new(),
        patch: "diff --git a/x b/x\n@@ -1 +1 @@\n-a\n+b\n".into(),
        runs: Vec::new(),
        check: Check::None,
        spend: None,
    });
    app.zoom = Zoom::Diff(worker);
    app.diff_scroll = 3;
    let screen = render(&app, 120, 20);
    assert!(screen.contains(" c  note   esc  back"), "{screen}");
    key(&mut app, KeyCode::Char('c'));
    let screen = render(&app, 120, 20);
    assert!(screen.contains("note on x:1 ▸"), "{screen}");
    assert!(screen.contains("enter keep · "), "{screen}");
    assert!(!screen.contains("enter send"), "{screen}");
    app.lang = Lang::Tr;
    assert!(render(&app, 120, 20).contains("x:1 için not ▸"));
}

#[test]
fn hint_spans_put_each_key_on_a_badge() {
    let spans = hint_spans(&[("c", "copy"), ("esc", "close")]);
    let text: Vec<&str> = spans.iter().map(|s| s.content.as_ref()).collect();
    assert_eq!(text, [" c ", " copy  ", " esc ", " close  "]);
    assert_eq!(spans[0].style, key_style());
    assert_eq!(spans[1].style, Style::new());
    assert!(hint_spans(&[]).is_empty());
}

#[test]
fn the_status_line_keeps_every_key_and_puts_the_place_in_what_they_leave() {
    let hints = vec![
        ("a", "approve"),
        ("r", "send back"),
        ("esc", "back"),
        ("?", "help"),
    ];
    let keys = spans_width(&hint_spans(&hints));
    let place = |words: &str| vec![Span::styled(words.to_string(), dim())];
    let words = "home · shop-api · #1 Shipping cost";
    let n = words.chars().count();
    let parts = |width: usize| status_parts(place(words), hints.clone(), width);
    // A blank, the place, three blanks, the keys.
    let (p, h) = parts(1 + n + 3 + keys);
    assert_eq!(p.unwrap().content, words, "whole where it fits");
    assert_eq!(h, hints);
    assert_eq!(
        p_style(&parts(1 + n + 3 + keys)),
        Some(dim()),
        "its style kept"
    );
    let (p, h) = parts(n + 3 + keys);
    assert_eq!(p.unwrap().content, "home · shop-api · #1 Shipping co…");
    assert_eq!(h, hints, "the place is cut, never a key");
    let (p, h) = parts(1 + PLACE_MIN + 3 + keys);
    assert_eq!(p.unwrap().content, "home · shop…", "at its least");
    assert_eq!(h, hints);
    let (p, h) = parts(PLACE_MIN + 3 + keys);
    assert!(p.is_none(), "under it the place goes");
    assert_eq!(h, hints, "every key still");
    // Then keys go from the end, never `esc` or `?`.
    let (p, h) = parts(1 + keys - 1);
    assert!(p.is_none());
    assert_eq!(h, vec![("a", "approve"), ("esc", "back"), ("?", "help")]);
    // No place: the keys alone.
    let (p, h) = status_parts(place(""), hints.clone(), 200);
    assert!(p.is_none());
    assert_eq!(h, hints);
    let (p, _) = status_parts(Vec::new(), hints.clone(), 200);
    assert!(p.is_none());
}

fn p_style(parts: &(Option<Span<'static>>, Vec<(&str, &str)>)) -> Option<Style> {
    parts.0.as_ref().map(|p| p.style)
}

#[test]
fn a_narrow_screen_s_status_line_has_the_row_s_keys_not_the_place() {
    let mut app = app_after(1, 4, 0, Lang::En);
    let a = app.world.projects[0].leader + 1;
    app.world.inbox.clear();
    app.world.asked_you(a, "Which parser should the tests use?");
    app.home_settle();
    let s = render(&app, 48, 20);
    let foot = s.lines().last().unwrap();
    assert!(foot.contains(" a  ") && foot.contains("?  help"), "{foot}");
    assert!(!foot.contains("home ·"), "{foot}");
    let s = render(&app, 120, 20);
    let foot = s.lines().last().unwrap();
    assert!(foot.contains("home ·") && foot.contains(" a  "), "{foot}");
}
