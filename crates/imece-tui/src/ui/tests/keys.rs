use super::*;

#[test]
fn capital_t_cycles_the_theme_and_says_which() {
    let mut app = app_after(1, 4, 50, Lang::En);
    key(&mut app, KeyCode::Char('T'));
    assert_eq!(crate::theme::pal().name, "dark");
    assert!(render(&app, 140, 40).contains("theme: dark"));
    key(&mut app, KeyCode::Char('T'));
    assert_eq!(crate::theme::pal().name, "light");
    key(&mut app, KeyCode::Char('T'));
    assert_eq!(crate::theme::pal().name, "mono");
    key(&mut app, KeyCode::Char('T'));
    assert_eq!(crate::theme::pal().name, "dracula");
    for _ in 0..crate::theme::builtins().count() - 4 {
        key(&mut app, KeyCode::Char('T'));
    }
    assert_eq!(crate::theme::pal().name, "terminal", "around the end");
}

#[test]
fn the_views_name_themselves() {
    let mut app = app_after(2, 12, 300, Lang::En);
    app.window = Some(Window::Board);
    let s = render(&app, 140, 40);
    assert!(s.contains("IN PROGRESS"), "{s}");
    let cards = app.world.shown_cards().count();
    assert!(s.contains(&format!(" BOARD · cards: {cards} ")), "{s}");
    app.window = Some(Window::Code);
    assert!(render(&app, 140, 40).contains("CODE MAP"));
    app.window = None;
    app.zoom = Zoom::Focus(3);
    assert!(render(&app, 140, 40).contains("FOCUS"));
}

#[test]
fn the_key_file_s_first_key_shows_while_it_waits_for_its_second() {
    let mut app = app_after(2, 12, 300, Lang::En);
    app.keys_held = Some((
        KeyEvent::new(
            KeyCode::Char('o'),
            ratatui::crossterm::event::KeyModifiers::CONTROL,
        ),
        std::time::Instant::now(),
    ));
    let s = render(&app, 140, 40);
    let foot = s.lines().last().unwrap();
    assert!(foot.starts_with(" ^o…   "), "{foot}");
    app.keys_held = None;
    let s = render(&app, 140, 40);
    assert!(!s.lines().last().unwrap().contains('…'), "nothing held");
}

#[test]
fn gg_and_capital_g_jump_in_every_list_and_the_chord_shows() {
    let mut app = app_after(2, 12, 300, Lang::En);
    render(&app, 140, 40);
    key(&mut app, KeyCode::Char('g'));
    let s = render(&app, 140, 40);
    let foot = s.lines().last().unwrap();
    assert!(
        foot.starts_with(" g…   "),
        "the pending key shows first, never cut off: {foot}"
    );
    key(&mut app, KeyCode::Char('g'));
    assert_eq!(app.at_home.row, 0);
    key(&mut app, KeyCode::Char('G'));
    assert_eq!(app.at_home.row, app.home_rows().len() - 1);
    app.pending = None;
    // The journal, newest first: gg is the newest line.
    app.open_window(Window::Journal);
    key(&mut app, KeyCode::Char('G'));
    let rows = journal_rows(&app.world);
    assert_eq!(app.journal_sel, Some(rows.last().unwrap().0));
    key(&mut app, KeyCode::Char('g'));
    key(&mut app, KeyCode::Char('g'));
    assert_eq!(app.journal_sel, Some(rows[0].0));
    // Inside a conversation too.
    let mut app = talking();
    app.open_thread(4);
    key(&mut app, KeyCode::Char('g'));
    key(&mut app, KeyCode::Char('g'));
    assert_eq!(app.thread.as_ref().unwrap().sel, 0);
    key(&mut app, KeyCode::Char('G'));
    assert_eq!(app.thread.as_ref().unwrap().sel, 3);
}

#[test]
fn z_chords_fold_one_project_all_or_none() {
    let mut app = app_after(3, 12, 100, Lang::En);
    render(&app, 140, 40);
    app.selected = Sel::Agent(app.world.projects[1].leader);
    key(&mut app, KeyCode::Char('z'));
    key(&mut app, KeyCode::Char('a'));
    assert_eq!(app.collapsed, [false, true, false]);
    assert_eq!(app.selected, Sel::Project(1), "the selection stays visible");
    key(&mut app, KeyCode::Char('z'));
    key(&mut app, KeyCode::Char('a'));
    assert_eq!(app.collapsed, [false, false, false]);
    key(&mut app, KeyCode::Char('z'));
    key(&mut app, KeyCode::Char('M'));
    assert_eq!(app.collapsed, [true, true, true]);
    key(&mut app, KeyCode::Char('z'));
    key(&mut app, KeyCode::Char('R'));
    assert_eq!(app.collapsed, [false, false, false]);
    // A chord that means nothing is dropped, not kept pending.
    key(&mut app, KeyCode::Char('z'));
    key(&mut app, KeyCode::Char('x'));
    assert_eq!(app.pending, None);
    key(&mut app, KeyCode::Char('j'));
    assert_ne!(app.selected, Sel::Project(0), "j moved as usual");
}

#[test]
fn t_opens_the_trace_and_v_the_conversation_of_the_agent_at_hand() {
    let mut app = talking();
    app.zoom = Zoom::Focus(0);
    key(&mut app, KeyCode::Char('t'));
    assert_eq!(app.zoom, Zoom::Trace(0));
    key(&mut app, KeyCode::Char('v'));
    let th = app.thread.as_ref().unwrap();
    assert_eq!(th.entries, [0, 1, 3, 4], "you ⇄ lead, from the trace too");
    app.thread = None;
    app.zoom = Zoom::None;
    app.at_home.at = Some(HomeRow::Agent(2));
    key(&mut app, KeyCode::Char('v'));
    assert!(app.thread.is_none());
    let screen = render(&app, 140, 40);
    assert!(screen.contains("no conversation with"), "{screen}");
    // In the leader's focus, the leader is at hand.
    app.zoom = Zoom::Focus(0);
    key(&mut app, KeyCode::Char('v'));
    assert!(app.thread.is_some());
}

#[test]
fn ctrl_d_and_u_page_the_conversation_and_the_lists() {
    let mut app = talking();
    app.open_thread(4);
    render(&app, 100, 20);
    let th = app.thread.as_ref().unwrap();
    th.scroll.set(0);
    ctrl(&mut app, 'd');
    let after = app.thread.as_ref().unwrap().scroll.get();
    assert!(after > 0, "scrolled half a page");
    ctrl(&mut app, 'u');
    assert_eq!(app.thread.as_ref().unwrap().scroll.get(), 0);
    app.thread = None;
    let mut app = app_after(1, 20, 100, Lang::En);
    render(&app, 140, 60);
    ctrl(&mut app, 'd');
    assert_eq!(app.at_home.row, 5);
    ctrl(&mut app, 'u');
    assert_eq!(app.at_home.row, 0);
}

#[test]
fn keys_do_only_what_they_do_where_they_do_it() {
    let mut app = app_after(2, 8, 100, Lang::En);
    render(&app, 140, 40);
    key(&mut app, KeyCode::Char('?'));
    assert!(app.help);
    key(&mut app, KeyCode::Esc);
    assert!(!app.help, "any key closes the help");
    // Panels cycle only on the salon, outside a focus: a window over
    // it takes the keys.
    app.window = Some(Window::Board);
    let pane = app.pane;
    for code in [
        KeyCode::Tab,
        KeyCode::BackTab,
        KeyCode::Char('['),
        KeyCode::Char(']'),
        KeyCode::Char('w'),
    ] {
        key(&mut app, code);
        assert_eq!(app.pane, pane, "{code:?} in a window");
    }
    app.window = None;
    app.zoom = Zoom::Focus(1);
    // `tab` in a focus is the detail's: it moves to another tab of
    // the same agent's work, never a pane.
    key(&mut app, KeyCode::Tab);
    assert_eq!(app.pane, pane, "tab in a focus");
    assert_ne!(
        app.detail().map(|d| d.1),
        Some(crate::app::Tab::Log),
        "another tab"
    );
    app.thread = None;
    app.zoom = Zoom::None;
    // Copying is the journal's; chords that fold are the salon's. A
    // stale journal selection must not make c copy elsewhere.
    app.journal_sel = Some(0);
    key(&mut app, KeyCode::Char('c'));
    assert!(app.copy.is_none(), "c in the salon copies nothing");
    app.pane = Pane::Journal;
    app.zoom = Zoom::Focus(1);
    key(&mut app, KeyCode::Char('c'));
    assert!(app.copy.is_none(), "c in a focus copies nothing");
    app.zoom = Zoom::None;
    // On Home the chords fold the row at hand's project, or all.
    key(&mut app, KeyCode::Char('z'));
    key(&mut app, KeyCode::Char('a'));
    assert_ne!(app.collapsed, [false, false], "za on Home folds");
    app.collapsed = vec![false, false];
    key(&mut app, KeyCode::Char('z'));
    key(&mut app, KeyCode::Char('M'));
    assert_eq!(app.collapsed, [true, true], "zM on Home folds all");
    app.collapsed = vec![false, false];
    app.collapsed = vec![true, true];
    key(&mut app, KeyCode::Char('z'));
    key(&mut app, KeyCode::Char('R'));
    assert_eq!(app.collapsed, [false, false], "zR on Home unfolds too");
    app.zoom = Zoom::Focus(1);
    app.collapsed = vec![true, true];
    key(&mut app, KeyCode::Char('z'));
    key(&mut app, KeyCode::Char('R'));
    assert_eq!(app.collapsed, [true, true], "zR in a focus over Home");
    app.collapsed = vec![false, false];
    key(&mut app, KeyCode::Char('z'));
    key(&mut app, KeyCode::Char('M'));
    assert_eq!(app.collapsed, [false, false], "zM in a focus over Home");
    app.zoom = Zoom::None;
    app.pane = Pane::Salon;
    key(&mut app, KeyCode::Char('z'));
    key(&mut app, KeyCode::Char('R'));
    assert_eq!(app.collapsed, [false, false], "zR in the salon");
    key(&mut app, KeyCode::Char('q'));
    key(&mut app, KeyCode::Char('q'));
    assert!(app.quit);
}

#[test]
fn one_q_asks_and_only_a_second_q_quits() {
    let mut app = app_after(1, 4, 50, Lang::En);
    render(&app, 140, 40);
    let before = app.at_home.row;
    key(&mut app, KeyCode::Char('q'));
    assert!(!app.quit, "one q does not quit");
    assert_eq!(app.pending, Some('q'));
    let s = render(&app, 140, 40);
    // The demo's agents are at work by now: the question counts them.
    assert!(
        s.contains("quit imece? agents mid-turn: ") && s.contains("· q again quits"),
        "{s}"
    );
    // Any other key stays, and is spent on the answer.
    key(&mut app, KeyCode::Char('j'));
    assert!(!app.quit);
    assert_eq!(app.pending, None);
    assert_eq!(
        app.at_home.row, before,
        "j answered the question, it did not move"
    );
    assert!(app.flash.is_none(), "the question is gone");
    key(&mut app, KeyCode::Esc);
    key(&mut app, KeyCode::Char('q'));
    key(&mut app, KeyCode::Esc);
    assert!(!app.quit && app.pending.is_none(), "esc stays too");
    key(&mut app, KeyCode::Char('q'));
    key(&mut app, KeyCode::Char('q'));
    assert!(app.quit, "qq quits");
    // In Turkish too, and with a conversation open.
    let mut app = talking();
    app.lang = Lang::Tr;
    app.open_thread(1);
    key(&mut app, KeyCode::Char('q'));
    assert!(render(&app, 140, 40).contains("imece kapatılsın mı?"));
    assert!(app.thread.is_some());
    key(&mut app, KeyCode::Char('q'));
    assert!(app.quit);
}

#[test]
fn ctrl_f_and_b_page_whole_pages_and_the_conversation_takes_half_and_whole() {
    let mut app = app_after(1, 20, 100, Lang::En);
    render(&app, 140, 60);
    ctrl(&mut app, 'f');
    let pos = |app: &App| app.at_home.row;
    assert_eq!(pos(&app), 10);
    ctrl(&mut app, 'b');
    assert_eq!(pos(&app), 0);
    let mut app = talking();
    app.open_thread(4);
    render(&app, 100, 20);
    let th = app.thread.as_ref().unwrap();
    th.scroll.set(0);
    let page = th.page.get();
    assert!(page > 2, "{page}");
    ctrl(&mut app, 'd');
    assert_eq!(app.thread.as_ref().unwrap().scroll.get(), page / 2);
    ctrl(&mut app, 'u');
    ctrl(&mut app, 'f');
    assert_eq!(app.thread.as_ref().unwrap().scroll.get(), page);
    key(&mut app, KeyCode::PageDown);
    assert_eq!(app.thread.as_ref().unwrap().scroll.get(), 2 * page);
    key(&mut app, KeyCode::Home);
    assert_eq!(app.thread.as_ref().unwrap().sel, 0);
    key(&mut app, KeyCode::End);
    assert_eq!(app.thread.as_ref().unwrap().sel, 3);
    key(&mut app, KeyCode::Char('c'));
    assert!(
        app.copy.take().is_some(),
        "c copies the line in a conversation"
    );
}
