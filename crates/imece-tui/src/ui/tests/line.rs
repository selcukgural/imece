use super::*;

/// The row above the footer, where the completions are.
fn menu_row(app: &App, width: u16) -> String {
    let s = render(app, width, 40);
    let mut rows = s.lines().rev();
    rows.next();
    rows.next().unwrap_or_default().to_string()
}

#[test]
fn every_command_says_what_it_does_in_both_languages_in_the_menus_order() {
    for lang in [Lang::En, Lang::Tr] {
        let names: Vec<&str> = lang.text().command_hints.iter().map(|(c, _)| *c).collect();
        assert_eq!(names, crate::complete::COMMANDS, "{lang:?}");
        for (c, what) in lang.text().command_hints {
            assert!(
                !what.is_empty() && what.chars().count() <= 80,
                "{c}: {what}"
            );
        }
    }
}

#[test]
fn the_colon_menu_says_how_many_are_out_of_view_and_what_the_one_put_in_does() {
    let mut app = app_after(1, 1, 0, Lang::En);
    let foot = |app: &App| render(app, 140, 40).lines().last().unwrap().to_string();
    let n = crate::complete::COMMANDS.len();
    key(&mut app, KeyCode::Char(':'));
    let row = menu_row(&app, 140);
    assert!(row.contains(" more › "), "more than fit, said: {row}");
    assert!(!row.contains("‹"), "{row}");
    let f = foot(&app);
    assert!(
        f.contains(&format!("tab next · enter take · esc hide · 1 of {n}")),
        "{f}"
    );
    assert!(!f.contains("yes from now on"), "nothing put in yet: {f}");
    // tab puts the first one in: what it does follows the keys.
    key(&mut app, KeyCode::Tab);
    assert_eq!(app.input, ":allow ");
    let f = foot(&app);
    assert!(
        f.contains(&format!(
            "esc hide · 1 of {n}   the project's yes from now on; PROGRAM [WORD] adds or drops one"
        )),
        "{f}"
    );
    key(&mut app, KeyCode::Tab);
    let f = foot(&app);
    assert!(
        f.contains(&format!("2 of {n}   put the project at hand away")),
        "{f}"
    );
    // Far on, the ones before are counted too.
    for _ in 0..40 {
        key(&mut app, KeyCode::Tab);
    }
    let row = menu_row(&app, 140);
    assert!(row.starts_with(" ‹ "), "{row}");
    // In Turkish.
    app.lang = Lang::Tr;
    let f = foot(&app);
    let (c, what) = Lang::Tr.text().command_hints[41];
    assert_eq!(app.input, format!(":{c} "));
    assert!(f.contains(&format!("42/{n}   {what}")), "{f}");
    app.lang = Lang::En;
    // Writing on: gone.
    key(&mut app, KeyCode::Char('x'));
    assert!(
        !foot(&app).contains(Lang::En.text().command_hints[41].1),
        "{}",
        foot(&app)
    );
    // One item: no count.
    app.input.clear();
    app.after = 0;
    for c in ":unse".chars() {
        key(&mut app, KeyCode::Char(c));
    }
    assert_eq!(app.menu.as_ref().map(|m| m.items.len()), Some(1));
    let f = foot(&app);
    assert!(f.contains("esc hide") && !f.contains(" of 1"), "{f}");
    // An argument's menu and an @ menu are not commands.
    app.input.clear();
    app.after = 0;
    for c in ":lang ".chars() {
        key(&mut app, KeyCode::Char(c));
    }
    key(&mut app, KeyCode::Tab);
    assert_eq!(app.input, ":lang en");
    assert!(!foot(&app).contains("Türkçe / English"), "{}", foot(&app));
    key(&mut app, KeyCode::Esc);
    key(&mut app, KeyCode::Esc);
    key(&mut app, KeyCode::Char('@'));
    key(&mut app, KeyCode::Tab);
    assert!(
        app.menu
            .as_ref()
            .is_some_and(|m| m.inserted && m.start == 1)
    );
    let f = foot(&app);
    assert!(f.contains("esc hide · 1 of "), "{f}");
    assert!(!f.contains("   the "), "{f}");
}

#[test]
fn an_at_offers_the_agents_and_tab_steps_through_them_with_the_text_following() {
    let mut app = app_after(1, 3, 0, Lang::En);
    let names: Vec<String> = app.world.agents.iter().map(|a| a.name.clone()).collect();
    key(&mut app, KeyCode::Char('@'));
    let m = app.menu.clone().expect("@ alone offers everyone");
    assert_eq!(m.items, names);
    let row = menu_row(&app, 140);
    for n in &names {
        assert!(row.contains(n.as_str()), "{row}");
    }
    let s = render(&app, 140, 40);
    assert!(s.contains("tab next · enter take · esc hide"), "{s}");
    // tab puts the first one in, and again steps on; ⇧tab steps back.
    key(&mut app, KeyCode::Tab);
    assert_eq!(app.input, format!("@{} ", names[0]));
    key(&mut app, KeyCode::Tab);
    assert_eq!(app.input, format!("@{} ", names[1]));
    key(&mut app, KeyCode::BackTab);
    assert_eq!(app.input, format!("@{} ", names[0]));
    key(&mut app, KeyCode::Down);
    assert_eq!(app.input, format!("@{} ", names[1]));
    key(&mut app, KeyCode::Up);
    assert_eq!(app.input, format!("@{} ", names[0]));
    ctrl(&mut app, 'n');
    assert_eq!(app.input, format!("@{} ", names[1]));
    ctrl(&mut app, 'p');
    assert_eq!(app.input, format!("@{} ", names[0]));
    ctrl(&mut app, 'x');
    assert_eq!(
        app.input,
        format!("@{} ", names[0]),
        "other ctrl keys do nothing"
    );
    // Typing on continues from the inserted name, menu gone.
    for c in "hi".chars() {
        key(&mut app, KeyCode::Char(c));
    }
    assert_eq!(app.menu, None);
    assert!(
        !render(&app, 140, 40).contains("enter send"),
        "the keys to send go once something is written"
    );
    key(&mut app, KeyCode::Enter);
    assert!(!app.typing);
    assert!(
        app.world
            .journal
            .iter()
            .any(|e| matches!(&e.event, Event::YouSaid { to, text } if *to == 0 && text == "hi"))
    );
}

#[test]
fn a_prefix_narrows_the_offers_and_enter_takes_one_before_it_sends() {
    let mut app = app_after(1, 3, 0, Lang::En);
    let names: Vec<String> = app.world.agents.iter().map(|a| a.name.clone()).collect();
    let (head, _) = names[2].split_at(2);
    key(&mut app, KeyCode::Char('@'));
    for c in head.chars() {
        key(&mut app, KeyCode::Char(c));
    }
    let m = app.menu.clone().unwrap();
    assert!(m.items.iter().all(|n| n.starts_with(head)), "{:?}", m.items);
    assert!(m.items.contains(&names[2]));
    // enter takes the highlighted name instead of sending.
    key(&mut app, KeyCode::Enter);
    assert!(app.typing);
    assert_eq!(app.input, format!("@{} ", m.items[0]));
    assert!(
        !app.world
            .journal
            .iter()
            .any(|e| matches!(&e.event, Event::YouSaid { .. }))
    );
    // Backspace brings the menu back for the shorter word.
    key(&mut app, KeyCode::Backspace);
    key(&mut app, KeyCode::Backspace);
    assert!(app.menu.is_some(), "{}", app.input);
    // esc hides the menu first, then cancels the writing.
    key(&mut app, KeyCode::Esc);
    assert!(app.typing);
    assert_eq!(app.menu, None);
    key(&mut app, KeyCode::Esc);
    assert!(!app.typing);
    assert_eq!(app.input, "");
    // A line nothing completes has no menu and tab does nothing to it.
    key(&mut app, KeyCode::Char('i'));
    for c in "hello".chars() {
        key(&mut app, KeyCode::Char(c));
    }
    key(&mut app, KeyCode::Tab);
    assert_eq!(app.input, "hello");
    assert_eq!(app.menu, None);
    key(&mut app, KeyCode::Esc);
    // An agent offers the command line its session listed; the
    // demo's listed none.
    key(&mut app, KeyCode::Char('/'));
    key(&mut app, KeyCode::Char('/'));
    assert_eq!(app.menu, None);
    key(&mut app, KeyCode::Esc);
    app.world.agents[0].commands = vec!["/clear".into(), "/compact".into()];
    key(&mut app, KeyCode::Char('/'));
    key(&mut app, KeyCode::Char('/'));
    key(&mut app, KeyCode::Char('c'));
    assert_eq!(app.menu.as_ref().unwrap().items, ["/clear", "/compact"]);
    key(&mut app, KeyCode::Tab);
    key(&mut app, KeyCode::Tab);
    assert_eq!(app.input, "/compact ");
    key(&mut app, KeyCode::Esc);
    key(&mut app, KeyCode::Esc);
    key(&mut app, KeyCode::Char('!'));
    assert_eq!(app.menu, None);
}

#[test]
fn the_completion_row_draws_each_note_after_its_item() {
    let mut m = menu_of(&["budget", "open", "help"], 1);
    m.notes = vec!["(spent $1.00 of $5.00)".into(), "$".into(), String::new()];
    let (row, out) = completion_row(&m, 60);
    assert_eq!(
        row.trim_end(),
        " budget (spent $1.00 of $5.00)  open $  help",
        "an empty note adds nothing"
    );
    let open = row.find(" open").unwrap();
    assert!(
        out[open..open + 8].iter().all(|&o| o),
        "the note stands out too"
    );
    assert!(!out[open + 8], "{row}");
    // The window counts the notes: the highlighted item stays in view,
    // the one before it out, and said.
    let (row, _) = completion_row(&m, 19);
    assert_eq!(row, " ‹ 1  open $  help ", "an exact fit");
}

#[test]
fn live_the_colon_menu_leaves_out_the_demos_clock_and_its_names_say_so() {
    let w = crate::live::testing::team(1);
    let live = crate::live::testing::idle_live(2);
    let mut app = App::with(w, crate::app::Engine::Live(Box::new(live)), Lang::En);
    key(&mut app, KeyCode::Char(':'));
    let m = app.menu.clone().unwrap();
    let want: Vec<&str> = (crate::complete::COMMANDS.into_iter())
        .filter(|c| !crate::complete::DEMO_ONLY.contains(c))
        .collect();
    assert_eq!(m.items, want);
    assert_eq!(m.notes.len(), m.items.len());
    key(&mut app, KeyCode::Char('p'));
    assert_eq!(
        app.menu.as_ref().unwrap().items,
        ["peek", "pending", "pin", "plan", "ps"],
        "not pause"
    );
    key(&mut app, KeyCode::Backspace);
    for c in "fa".chars() {
        key(&mut app, KeyCode::Char(c));
    }
    assert_eq!(app.menu, None, "nothing left to offer");
    for c in "ster".chars() {
        key(&mut app, KeyCode::Char(c));
    }
    key(&mut app, KeyCode::Enter);
    assert_eq!(app.speed, 1);
    let s = render(&app, 140, 40);
    assert!(s.contains(":faster is for the demo"), "{s}");
    for c in ":pause".chars() {
        key(&mut app, KeyCode::Char(c));
    }
    key(&mut app, KeyCode::Enter);
    assert!(!app.paused);
}

#[test]
fn a_colon_offers_the_commands_to_the_end() {
    let mut app = app_after(1, 2, 0, Lang::En);
    key(&mut app, KeyCode::Char(':'));
    // The demo's own commands too.
    assert_eq!(app.menu.as_ref().unwrap().items, crate::complete::COMMANDS);
    let row = menu_row(&app, 140);
    assert!(row.contains("budget") && row.contains("board"), "{row}");
    key(&mut app, KeyCode::Char('b'));
    key(&mut app, KeyCode::Char('u'));
    key(&mut app, KeyCode::Tab);
    assert_eq!(app.input, ":budget ");
    key(&mut app, KeyCode::Enter);
    assert!(!app.typing);
    // Turkish hint too.
    app.lang = Lang::Tr;
    key(&mut app, KeyCode::Char(':'));
    assert!(render(&app, 140, 40).contains("tab sonraki · enter al · esc gizle"));
}

#[test]
fn the_menu_row_keeps_the_highlighted_item_in_view_when_they_do_not_all_fit() {
    let mut app = app_after(2, 8, 0, Lang::En);
    key(&mut app, KeyCode::Char('@'));
    let items = app.menu.as_ref().unwrap().items.clone();
    assert!(items.len() >= 10, "{}", items.len());
    let last = items.last().unwrap().clone();
    let row = menu_row(&app, 40);
    assert!(row.contains(&items[0]), "{row}");
    assert!(!row.contains(&last), "{row}");
    key(&mut app, KeyCode::BackTab);
    assert_eq!(app.input, format!("@{last} "));
    let row = menu_row(&app, 40);
    assert!(row.contains(&last), "{row}");
    assert!(!row.contains(&items[0]), "{row}");
    // A wide screen shows them all.
    let row = menu_row(&app, 400);
    for n in &items {
        assert!(row.contains(n.as_str()), "{row}");
    }
}

#[test]
fn backspace_takes_a_letter_back_and_the_offers_widen_again() {
    let mut app = app_after(1, 3, 0, Lang::En);
    let everyone = app.world.agents.len();
    key(&mut app, KeyCode::Char('@'));
    key(&mut app, KeyCode::Char('z'));
    assert_eq!(app.input, "@z");
    assert_eq!(app.menu, None, "nobody is called z…");
    key(&mut app, KeyCode::Backspace);
    assert_eq!(app.input, "@");
    assert_eq!(app.menu.as_ref().map(|m| m.items.len()), Some(everyone));
}

#[test]
fn writing_without_offers_takes_no_row_from_the_body() {
    // A short screen, so the body is full and a row less would show.
    let mut app = app_after(1, 3, 30, Lang::En);
    let before = render(&app, 140, 12);
    key(&mut app, KeyCode::Char('/'));
    for c in "hello".chars() {
        key(&mut app, KeyCode::Char(c));
    }
    assert!(app.typing);
    assert_eq!(app.menu, None);
    let after = render(&app, 140, 12);
    let body = |s: &str| s.lines().take(11).map(str::to_string).collect::<Vec<_>>();
    assert_eq!(body(&before), body(&after), "only the footer changed");
    assert_ne!(before, after);
}

#[test]
fn readline_keys_edit_the_line_at_the_cursor() {
    let mut app = app_after(1, 3, 0, Lang::En);
    key(&mut app, KeyCode::Char('/'));
    write(&mut app, "fix parser");
    ctrl(&mut app, 'a');
    assert_eq!((app.cursor(), app.after), (0, 10));
    write(&mut app, "please ");
    assert_eq!(app.input, "please fix parser");
    alt(&mut app, KeyCode::Char('f'));
    assert_eq!(app.cursor(), 10);
    ctrl(&mut app, 'k');
    assert_eq!(
        (app.input.as_str(), app.cut.as_str()),
        ("please fix", " parser")
    );
    alt(&mut app, KeyCode::Char('b'));
    ctrl(&mut app, 'w');
    assert_eq!((app.input.as_str(), app.cut.as_str()), ("fix", "please "));
    ctrl(&mut app, 'e');
    ctrl(&mut app, 'y');
    assert_eq!(app.input, "fixplease ");
    ctrl(&mut app, 'u');
    assert_eq!((app.input.as_str(), app.cut.as_str()), ("", "fixplease "));
    assert!(app.typing, "ctrl keys edit, they do not leave");
}

#[test]
fn the_menu_is_offered_only_with_the_cursor_at_the_end() {
    let mut app = app_after(1, 3, 0, Lang::En);
    key(&mut app, KeyCode::Char('@'));
    write(&mut app, "cl");
    assert!(app.menu.is_some());
    key(&mut app, KeyCode::Left);
    assert_eq!(app.menu, None, "moved off the end");
    key(&mut app, KeyCode::Tab);
    assert_eq!(
        (app.input.as_str(), app.menu.is_none()),
        ("@cl", true),
        "tab waits for the end"
    );
    key(&mut app, KeyCode::End);
    assert!(app.menu.is_some(), "back at the end");
    // A cut that leaves the cursor where it is but at the end now.
    key(&mut app, KeyCode::Left);
    write(&mut app, "x");
    assert_eq!((app.input.as_str(), app.menu.is_none()), ("@cxl", true));
    key(&mut app, KeyCode::Left);
    ctrl(&mut app, 'k');
    assert_eq!(app.input, "@c");
    assert!(
        app.menu.is_some(),
        "the line changed and ends at the cursor"
    );
    // A key that does nothing leaves the menu as it was.
    key(&mut app, KeyCode::Tab);
    let menu = app.menu.clone();
    assert!(menu.as_ref().unwrap().inserted);
    key(&mut app, KeyCode::Right);
    ctrl(&mut app, 'x');
    assert_eq!(app.menu, menu);
}

#[test]
fn tab_at_the_end_with_the_menu_hidden_offers_it_again() {
    let mut app = app_after(1, 3, 0, Lang::En);
    key(&mut app, KeyCode::Char('@'));
    write(&mut app, "cl");
    key(&mut app, KeyCode::Esc);
    assert!(app.typing && app.menu.is_none(), "esc hid the menu");
    key(&mut app, KeyCode::Tab);
    let m = app.menu.as_ref().unwrap();
    assert!(m.inserted);
    assert_eq!(app.input, format!("@{} ", m.items[0]));
}

#[test]
fn the_cursor_starts_at_the_end_after_esc_and_after_sending() {
    let mut app = app_after(1, 3, 0, Lang::En);
    key(&mut app, KeyCode::Char('/'));
    write(&mut app, "ab");
    key(&mut app, KeyCode::Left);
    key(&mut app, KeyCode::Esc);
    key(&mut app, KeyCode::Char(':'));
    write(&mut app, "b");
    assert_eq!(app.input, ":b");
    key(&mut app, KeyCode::Left);
    key(&mut app, KeyCode::Enter);
    assert!(!app.typing);
    key(&mut app, KeyCode::Char('!'));
    write(&mut app, "ls");
    assert_eq!(app.input, "!ls");
}

#[test]
fn a_stale_cursor_lands_on_the_next_character_edge() {
    let mut app = app_after(1, 3, 0, Lang::En);
    app.input = "aç".into();
    for (after, at) in [(0, 3), (1, 3), (2, 1), (3, 0), (9, 0)] {
        app.after = after;
        assert_eq!(app.cursor(), at, "{after}");
    }
}

/// What the person last said, to whom.
fn last_said(app: &App) -> Option<(usize, String)> {
    app.world.journal.iter().rev().find_map(|e| match &e.event {
        Event::YouSaid { to, text } => Some((*to, text.clone())),
        _ => None,
    })
}

#[test]
fn shift_enter_alt_enter_and_a_backslash_enter_write_a_new_line_and_enter_sends_it_all() {
    let mut app = app_after(1, 3, 0, Lang::En);
    let lead = app.world.projects[0].leader;
    key(&mut app, KeyCode::Char('/'));
    write(&mut app, "fix");
    with(&mut app, KeyCode::Enter, KeyModifiers::SHIFT);
    write(&mut app, "the");
    with(&mut app, KeyCode::Enter, KeyModifiers::ALT);
    write(&mut app, "parser\\");
    key(&mut app, KeyCode::Enter);
    assert!(app.typing, "a backslash enter does not send");
    write(&mut app, "now");
    assert_eq!(app.input, "fix\nthe\nparser\nnow");
    key(&mut app, KeyCode::Enter);
    assert!(!app.typing);
    assert_eq!(
        last_said(&app),
        Some((lead, "fix\nthe\nparser\nnow".to_string()))
    );
    // To one agent: the name ends at the new line too.
    let name = app.world.agents[1].name.clone();
    key(&mut app, KeyCode::Char('@'));
    write(&mut app, &name);
    with(&mut app, KeyCode::Enter, KeyModifiers::SHIFT);
    write(&mut app, "hi");
    assert_eq!(app.typing_target(), name);
    key(&mut app, KeyCode::Enter);
    assert_eq!(last_said(&app), Some((1, "hi".to_string())));
}

#[test]
fn a_new_line_hides_the_menu_and_up_and_down_go_between_lines_without_one() {
    let mut app = app_after(1, 3, 0, Lang::En);
    key(&mut app, KeyCode::Char('@'));
    assert!(app.menu.is_some());
    with(&mut app, KeyCode::Enter, KeyModifiers::SHIFT);
    assert_eq!((app.input.as_str(), app.menu.is_none()), ("@\n", true));
    assert!(app.typing, "the menu did not take the enter");
    // Alt-enter as well, the menu open again.
    key(&mut app, KeyCode::Backspace);
    assert!(app.menu.is_some());
    with(&mut app, KeyCode::Enter, KeyModifiers::ALT);
    assert_eq!((app.input.as_str(), app.menu.is_none()), ("@\n", true));
    write(&mut app, "abc");
    key(&mut app, KeyCode::Up);
    assert_eq!((app.cursor(), app.after), (1, 4), "the first line's end");
    key(&mut app, KeyCode::Down);
    assert_eq!(app.cursor(), 3, "column 1, under it");
    key(&mut app, KeyCode::Down);
    assert_eq!(app.cursor(), 3, "the last line: it stays");
    assert!(app.typing && app.menu.is_none());
}

#[test]
fn a_paste_goes_into_the_line_whole_and_only_while_writing() {
    let mut app = app_after(1, 3, 0, Lang::En);
    app.paste("ls");
    assert!(!app.typing && app.input.is_empty(), "not writing: left out");
    let said = Lang::En.text().paste_not_writing;
    assert!(render(&app, 140, 40).contains(said));
    key(&mut app, KeyCode::Char('/'));
    write(&mut app, "ab");
    key(&mut app, KeyCode::Left);
    app.paste("one\r\ntwo\rthree\x1b[1m\tç\x07");
    assert_eq!(app.input, "aone\ntwo\nthree[1m\tçb");
    assert_eq!(
        app.after, 1,
        "the cursor after what was pasted, before the b"
    );
    assert!(app.typing, "a paste never sends");
    // An overlay over the line takes nothing either.
    let line = app.input.clone();
    for open in 0..6 {
        match open {
            0 => app.help = true,
            1 => app.digest = Some(Default::default()),
            2 => app.left_off = Some(Default::default()),
            3 => app.news = vec!["0.1.0"],
            4 => app.welcome = true,
            _ => app.leaving = true,
        }
        app.flash = None;
        app.paste("x");
        assert_eq!(app.input, line, "overlay {open}");
        assert!(app.flash.is_some(), "overlay {open}");
        (app.help, app.digest, app.left_off, app.leaving) = (false, None, None, false);
        (app.news, app.welcome) = (Vec::new(), false);
    }
    app.paste("x");
    assert_ne!(app.input, line, "and with none open it goes in");
}

#[test]
fn a_row_is_cut_cell_by_cell_and_the_cursor_found_in_it() {
    let cut = |t: &str, room, caret| wrap_cells(t, room, caret);
    assert_eq!(cut("", 4, Some(0)), (vec![String::new()], (0, 0)));
    assert_eq!(cut("", 4, None), (vec![String::new()], (0, 0)));
    assert_eq!(cut("abc", 4, Some(3)), (vec!["abc".into()], (0, 3)));
    assert_eq!(cut("abcd", 4, None), (vec!["abcd".into()], (0, 0)));
    // At the end of a full row the cursor is on the next, made for it.
    assert_eq!(
        cut("abcd", 4, Some(4)),
        (vec!["abcd".into(), String::new()], (1, 0))
    );
    assert_eq!(
        cut("abcdefghij", 4, Some(5)),
        (vec!["abcd".into(), "efgh".into(), "ij".into()], (1, 1))
    );
    assert_eq!(
        cut("abcdefgh", 4, Some(4)),
        (vec!["abcd".into(), "efgh".into()], (1, 0)),
        "before e: the start of its row"
    );
    assert_eq!(cut("abcde", 4, Some(3)).1, (0, 3));
    // A wide character that does not fit goes on to the next row.
    assert_eq!(
        cut("abc中d", 4, Some(3)),
        (vec!["abc".into(), "中d".into()], (1, 0))
    );
    assert_eq!(
        cut("中中中", 4, Some(9)),
        (vec!["中中".into(), "中".into()], (1, 2))
    );
    // Too narrow for one: one character a row, never none.
    assert_eq!(cut("中a", 1, None).0, ["中", "a"]);
    assert_eq!(
        cut("ab", 0, Some(1)),
        (vec!["a".into(), "b".into()], (1, 0))
    );
}

/// The column `needle` starts at on row `y` of `app` drawn 140 by 40;
/// every character here is one cell wide.
fn column_of(app: &App, y: usize, needle: &str) -> u16 {
    let screen = render(app, 140, 40);
    let row = screen.lines().nth(y).unwrap();
    let at = row
        .find(needle)
        .unwrap_or_else(|| panic!("{needle} in {row}"));
    u16::try_from(row[..at].chars().count()).unwrap()
}

#[test]
fn the_terminal_s_cursor_stands_on_its_own_line_at_its_own_character() {
    let mut app = app_after(1, 3, 0, Lang::En);
    assert_eq!(
        caret_of(&app, 140, 40),
        None,
        "hidden while nothing is typed"
    );
    key(&mut app, KeyCode::Char('/'));
    app.input = "one\ntwo\nthree".into();
    app.after = 4; // before the h of three
    assert_eq!(
        caret_of(&app, 140, 40),
        Some((column_of(&app, 39, "three") + 1, 39))
    );
    app.after = 8; // before the w of two
    assert_eq!(
        caret_of(&app, 140, 40),
        Some((column_of(&app, 38, "two") + 1, 38))
    );
    app.after = 0; // the end
    assert_eq!(
        caret_of(&app, 140, 40),
        Some((column_of(&app, 39, "three") + 5, 39))
    );
    // No reversed letter or block of imece's own any more.
    let buf = buffer(&app, 140, 40);
    let reversed = (37..40)
        .flat_map(|y| (0..140).map(move |x| (x, y)))
        .filter(|&(x, y)| buf[(x, y)].modifier.contains(Modifier::REVERSED))
        .count();
    assert_eq!(reversed, 0);
    assert!(!render(&app, 140, 40).contains('█'));
}

#[test]
fn the_cursor_stands_before_its_byte_and_after_a_hidden_token() {
    assert_eq!(caret_line("açb", 1), ("açb".to_string(), 1));
    assert_eq!(
        caret_line("açb", 3),
        ("açb".to_string(), 3),
        "ç is two bytes"
    );
    assert_eq!(
        caret_line("a中b", 4),
        ("a中b".to_string(), 4),
        "中 is three bytes"
    );
    assert_eq!(caret_line("ab", 2), ("ab".to_string(), 2));
    assert_eq!(caret_line("ab", 0), ("ab".to_string(), 0));
    // A line `clean` changes is drawn cleaned, the cursor at its end
    // wherever it is.
    assert_eq!(caret_line("x sk-abc", 3), ("x [***]".to_string(), 7));
    assert_eq!(caret_line("x sk-abc", 0), ("x [***]".to_string(), 7));
}

#[test]
fn a_cursor_past_the_edge_stays_on_the_last_column_and_none_is_off_the_area() {
    let area = Rect::new(2, 3, 5, 2);
    assert_eq!(caret_in(area, 0, 0), Some(Position::new(2, 3)));
    assert_eq!(caret_in(area, 1, 4), Some(Position::new(6, 4)));
    assert_eq!(caret_in(area, 1, 5), Some(Position::new(6, 4)), "cut there");
    assert_eq!(caret_in(area, 0, 99), Some(Position::new(6, 3)));
    assert_eq!(caret_in(area, 2, 0), None, "a row it does not have");
    assert_eq!(caret_in(Rect::new(2, 3, 0, 2), 0, 0), None, "no column");
    assert_eq!(caret_in(Rect::new(2, 3, 5, 0), 0, 0), None, "no row");
}

#[test]
fn the_cursor_s_look_says_what_is_typed() {
    let mut app = app_after(1, 3, 0, Lang::En);
    assert_eq!(look(&app), None);
    app.typing = true;
    app.input = "hello".into();
    let bar = Look {
        shape: Shape::BlinkingBar,
        colour: None,
    };
    assert_eq!(look(&app), Some(bar));
    app.input = ":theme".into();
    assert_eq!(
        look(&app),
        Some(Look {
            shape: Shape::SteadyBar,
            colour: rgb(pal().accent),
        })
    );
    app.input = "#3".into();
    assert_eq!(
        look(&app),
        Some(Look {
            shape: Shape::BlinkingUnderline,
            colour: None,
        })
    );
    assert_eq!(rgb(Color::Rgb(1, 2, 3)), Some([1, 2, 3]));
    assert_eq!(rgb(Color::Indexed(4)), None, "the terminal's own");
    assert_eq!(rgb(Color::Cyan), None);
    assert_eq!(rgb(Color::Reset), None);
}

#[test]
fn the_filters_of_the_help_and_the_archives_have_the_cursor_after_them() {
    let mut app = app_after(1, 3, 0, Lang::En);
    key(&mut app, KeyCode::Char('?'));
    key(&mut app, KeyCode::Char('/'));
    write(&mut app, "esc");
    let screen = render(&app, 140, 40);
    let (y, row) = (screen.lines().enumerate())
        .find(|(_, r)| r.contains(" /esc"))
        .unwrap_or_else(|| panic!("{screen}"));
    let at = row.find(" /esc").unwrap();
    let x = u16::try_from(row[..at].chars().count() + 5).unwrap();
    let y = u16::try_from(y).unwrap();
    assert_eq!(caret_of(&app, 140, 40), Some((x, y)));
    assert!(!screen.contains('▏'), "no bar of imece's own");
    key(&mut app, KeyCode::Enter);
    assert!(!app.help_typing);
    assert_eq!(caret_of(&app, 140, 40), None, "kept, not typed");
    // A short screen: the line saying how much is below comes first.
    let mut app = app_after(1, 3, 0, Lang::En);
    key(&mut app, KeyCode::Char('?'));
    key(&mut app, KeyCode::Char('/'));
    let screen = render(&app, 70, 20);
    let (y, row) = (screen.lines().enumerate())
        .find(|(_, r)| r.contains(" more · "))
        .unwrap_or_else(|| panic!("{screen}"));
    let at = row.find(" more · ").unwrap();
    let at = at + row[at..].find(" /").unwrap();
    let x = u16::try_from(row[..at].chars().count() + 2).unwrap();
    assert_eq!(caret_of(&app, 70, 20), Some((x, u16::try_from(y).unwrap())));
    // The archives.
    let mut app = app_after(1, 3, 0, Lang::En);
    let mut b = crate::archives::Browser::new(&app.world, Vec::new());
    b.typing = true;
    b.find = "ab".into();
    app.archives = Some(b);
    let screen = render(&app, 140, 40);
    let (y, row) = (screen.lines().enumerate())
        .find(|(_, r)| r.contains(" /ab"))
        .unwrap_or_else(|| panic!("{screen}"));
    let at = row.find(" /ab").unwrap();
    let x = u16::try_from(row[..at].chars().count() + 4).unwrap();
    assert_eq!(
        caret_of(&app, 140, 40),
        Some((x, u16::try_from(y).unwrap()))
    );
    assert!(!screen.contains('▏'));
    app.archives.as_mut().unwrap().typing = false;
    assert_eq!(caret_of(&app, 140, 40), None);
}

#[test]
fn the_cursor_is_not_shown_under_a_conversation_or_a_peek() {
    let mut app = app_after(1, 4, 50, Lang::En);
    app.open_thread(4);
    assert!(app.thread.is_some());
    app.typing = true;
    app.input = "one".into();
    assert_eq!(
        caret_of(&app, 140, 40).map(|(_, y)| y),
        Some(39),
        "the foot is clear of the window"
    );
    app.input = "one\ntwo\nthree".into();
    app.after = 8; // on the middle line: the window stays above the composer
    assert_eq!(caret_of(&app, 140, 40).map(|(_, y)| y), Some(38));
    app.thread = None;
    assert_eq!(caret_of(&app, 140, 40).map(|(_, y)| y), Some(38));
    app.peek = Some(1);
    app.after = 0;
    assert_eq!(caret_of(&app, 140, 40).map(|(_, y)| y), Some(39));
    // The peek leaves three rows below it: a footer of five lines has
    // its first under it.
    app.input = "1\n2\n3\n4\n5".into();
    app.after = 8; // the first line
    let rows = caret_of(&app, 140, 40);
    assert_eq!(rows, None, "under the peek");
}

/// `completions` alone: a row of `width` cells with `m` drawn in it,
/// and whether the cell at `x` stands out.
fn completion_row(m: &crate::complete::Menu, width: u16) -> (String, Vec<bool>) {
    let mut t = Terminal::new(TestBackend::new(width, 1)).unwrap();
    t.draw(|f| completions(f, f.area(), m, Lang::En)).unwrap();
    let buf = t.backend().buffer();
    let row = (0..width)
        .map(|x| buf[(x, 0)].symbol().to_string())
        .collect::<String>();
    let out = (0..width)
        .map(|x| buf[(x, 0)].modifier.contains(Modifier::REVERSED))
        .collect();
    (row, out)
}

fn menu_of(items: &[&str], chosen: usize) -> crate::complete::Menu {
    crate::complete::Menu {
        start: 1,
        items: items.iter().map(|s| s.to_string()).collect(),
        chosen,
        tail: " ",
        inserted: false,
        notes: Vec::new(),
    }
}

#[test]
fn the_completion_row_fits_what_it_can_around_the_highlighted_item() {
    // Each item takes its letters and a space either side: 4 5 6 4 3.
    let items = ["aa", "bbb", "cccc", "dd", "e"];
    let (row, out) = completion_row(&menu_of(&items, 0), 22);
    assert_eq!(
        row, " aa  bbb  cccc  dd  e ",
        "an exact fit shows all, no marks"
    );
    assert!(out[1] && out[2], "the highlighted one stands out");
    assert!(out[..4].iter().all(|&o| o));
    assert!(out[4..].iter().all(|&o| !o), "{out:?}");
    // One cell short: what does not fit is counted, never cut.
    let (row, _) = completion_row(&menu_of(&items, 0), 21);
    assert_eq!(row, " aa  bbb  +3 more ›  ");
    let (row, _) = completion_row(&menu_of(&items, 0), 20);
    assert_eq!(row, " aa  bbb  +3 more › ", "the mark fits exactly");
    let (row, _) = completion_row(&menu_of(&items, 0), 19);
    assert_eq!(row, " aa  +4 more ›     ", "one cell short of it");
    // The last one highlighted: the window starts where the earlier
    // ones still fit before it, the ones before counted.
    let (row, out) = completion_row(&menu_of(&items, 4), 21);
    assert_eq!(row, " ‹ 2  cccc  dd  e    ");
    assert!(out[15] && out[16] && out[17] && !out[14] && !out[18]);
    // In the middle: both marks.
    let seven = ["x1", "x2", "x3", "x4", "x5", "x6", "x7"];
    let (row, out) = completion_row(&menu_of(&seven, 3), 20);
    assert_eq!(row, " ‹ 3  x4  +3 more › ");
    assert!(out[5] && out[8] && !out[4] && !out[9]);
    // Pulling the window back looks at the item before, not the one
    // counted already: a big one before a small highlighted one.
    let (row, out) = completion_row(&menu_of(&["xxxxxxxx", "a"], 1), 12);
    assert_eq!(row, " ‹ 1  a     ");
    assert!(out[5] && out[6] && out[7] && !out[4] && !out[8]);
    // The marks' widths are those of their counts: a count of two
    // digits takes a cell more (7 items, 4 after: one digit).
    let (row, _) = completion_row(&menu_of(&seven, 0), 23);
    assert_eq!(row, " x1  x2  x3  +4 more › ");
    let twelve: Vec<String> = (1..=12).map(|i| format!("y{i}")).collect();
    let twelve: Vec<&str> = twelve.iter().map(String::as_str).collect();
    let (row, _) = completion_row(&menu_of(&twelve, 0), 19);
    assert_eq!(row, " y1  +11 more ›    ", "10 after would not fit");
    let (row, _) = completion_row(&menu_of(&twelve, 0), 20);
    assert_eq!(row, " y1  y2  +10 more › ");
    // Pulled back to the first: no mark before, its room is the items'.
    let (row, _) = completion_row(&menu_of(&seven, 1), 19);
    assert_eq!(row, " x1  x2  +5 more › ");
    // Turkish says it in its words.
    let mut t = Terminal::new(TestBackend::new(21, 1)).unwrap();
    t.draw(|f| completions(f, f.area(), &menu_of(&items, 0), Lang::Tr))
        .unwrap();
    let row: String = (0..21)
        .map(|x| t.backend().buffer()[(x, 0)].symbol().to_string())
        .collect();
    assert_eq!(row, " aa  +4 tane daha ›  ");
}

#[test]
fn a_colon_command_sets_the_budget_and_the_journal_says_so() {
    let mut app = app_after(1, 2, 0, Lang::En);
    let was = app.world.budget;
    app.world.spending.warned = true;
    key(&mut app, KeyCode::Char(':'));
    assert!(app.typing);
    assert_eq!(app.input, ":");
    assert!(render(&app, 140, 40).contains("› command:"));
    for c in "budget 80".chars() {
        key(&mut app, KeyCode::Char(c));
    }
    key(&mut app, KeyCode::Enter);
    assert_eq!(app.world.budget, 80 * 10_000);
    assert!(
        !app.world.spending.warned,
        "a new pot is worth a new warning"
    );
    app.world.spending.warned = true;
    app.open_window(Window::Journal);
    let s = render(&app, 140, 40);
    assert!(s.contains("of $80.00"), "{s}");
    assert!(s.contains("the budget is $80.00 now"), "{s}");
    let said = format!("you set the budget to $80.00 (was {})", dollars(was));
    assert!(s.contains(&said), "{s}");
    app.close_window();
    assert!(
        !app.world
            .journal
            .iter()
            .any(|e| matches!(&e.event, Event::YouSaid { .. })),
        "a command is not a message to the lead"
    );

    // Not dollars: nothing changes, the screen says why.
    for c in ":b lots".chars() {
        key(&mut app, KeyCode::Char(c));
    }
    key(&mut app, KeyCode::Enter);
    assert_eq!(app.world.budget, 80 * 10_000);
    assert!(app.world.spending.warned, "the pot did not change");
    assert!(render(&app, 140, 40).contains("lots is not dollars"));

    // An unknown command.
    for c in ":nope".chars() {
        key(&mut app, KeyCode::Char(c));
    }
    key(&mut app, KeyCode::Enter);
    assert!(render(&app, 140, 40).contains("no command nope"));

    // Plain writing is to the lead, not a command.
    key(&mut app, KeyCode::Char('i'));
    let s = render(&app, 140, 40);
    assert!(s.contains("› to lead"), "{s}");
    assert!(!s.contains("› command:"), "{s}");
}

fn type_line(app: &mut App, line: &str) {
    key(app, KeyCode::Char('/'));
    for c in line.chars() {
        key(app, KeyCode::Char(c));
    }
    key(app, KeyCode::Enter);
}

fn you_said(app: &App) -> Vec<String> {
    app.world
        .journal
        .iter()
        .filter_map(|e| match &e.event {
            Event::YouSaid { text, .. } => Some(text.clone()),
            _ => None,
        })
        .collect()
}

#[test]
fn words_reach_the_agent_and_a_command_its_own_command_line_or_a_refusal() {
    // The demo, as live: a command needs a session, words start one.
    let mut app = app_after(1, 2, 0, Lang::En);
    app.world.journal.clear();
    type_line(&mut app, "/cost");
    assert!(you_said(&app).is_empty(), "a command is not said");
    let s = render(&app, 140, 40);
    assert!(
        s.contains("lead took no command: it has no session yet"),
        "{s}"
    );
    type_line(&mut app, "hello");
    assert_eq!(you_said(&app), ["hello"]);
    assert!(render(&app, 140, 40).contains("sent to lead"));
    type_line(&mut app, "/cost");
    assert_eq!(you_said(&app), ["hello", "/cost"], "taken, as live's");
    assert!(render(&app, 140, 40).contains("sent to lead"));
    type_line(&mut app, "/nope");
    let s = render(&app, 140, 40);
    assert!(
        s.contains("lead took no command: /nope: no such command"),
        "{s}"
    );

    // Live: a command needs a session, words start one.
    let w = crate::live::testing::team(1);
    let live = crate::live::testing::idle_live(2);
    let mut app = App::with(w, crate::app::Engine::Live(Box::new(live)), Lang::En);
    app.world.journal.clear();
    type_line(&mut app, "/cost");
    assert!(you_said(&app).is_empty(), "{:?}", you_said(&app));
    let s = render(&app, 140, 40);
    assert!(
        s.contains("lead took no command: it has no session yet"),
        "{s}"
    );
    type_line(&mut app, "hello");
    assert_eq!(you_said(&app), ["hello"]);
    assert!(render(&app, 140, 40).contains("sent to lead"));
}

#[test]
fn typing_works_with_the_conversation_open() {
    let mut app = talking();
    app.open_thread(1);
    key(&mut app, KeyCode::Char('/'));
    assert!(app.typing);
    key(&mut app, KeyCode::Char('x'));
    assert_eq!(app.input, "x");
    assert!(app.thread.is_some());
}

#[test]
fn a_note_or_a_search_that_starts_with_a_bang_keeps_its_own_lead() {
    let mut app = app_after(1, 2, 0, Lang::En);
    let t = Lang::En.text();
    app.typing = true;
    app.input = "!ls".into();
    assert_eq!(writing_to(&app).content, t.typing_shell);
    assert_eq!(writing_to(&app).style, shell());
    app.searching = true;
    assert_eq!(writing_to(&app).content, t.typing_history);
    assert_ne!(writing_to(&app).style, shell());
    app.recalling = true;
    assert_eq!(writing_to(&app).content, t.typing_recall);
    app.searching = false;
    assert_eq!(
        writing_to(&app).content,
        t.typing_shell,
        "recalling alone is no search"
    );
    app.recalling = false;
    app.noting = Some(Noting::Line(0, 0));
    assert_eq!(writing_to(&app).content, fill(t.typing_note, &[""]));
    assert_ne!(writing_to(&app).style, shell());
}

#[test]
fn a_bang_opens_the_shell_line_and_enter_hands_the_command_to_the_main_loop() {
    let mut app = app_after(1, 2, 0, Lang::En);
    key(&mut app, KeyCode::Char('!'));
    assert!(app.typing);
    assert_eq!(app.input, "!");
    assert!(app.typing_shell());
    assert!(!app.typing_command());
    let s = render(&app, 140, 40);
    assert!(s.contains("› shell:"), "{s}");
    assert!(!s.contains("› command:"), "{s}");
    // Its lead on the shell colour, the text after it plain; gone with
    // the `!`.
    let lead = |app: &App| {
        let buf = buffer(app, 140, 40);
        let at = |x: u16| buf[(x, 39)].style();
        (
            at(2),
            at(u16::try_from(" › shell: ".chars().count()).unwrap() + 1),
        )
    };
    let (on, text) = lead(&app);
    assert_eq!((on.bg, on.fg), (Some(pal().shell), Some(pal().on_badge)));
    assert!(on.add_modifier.contains(Modifier::BOLD));
    assert_ne!(text.bg, Some(pal().shell), "only the lead");
    key(&mut app, KeyCode::Backspace);
    assert!(app.typing && !app.typing_shell());
    assert_ne!(lead(&app).0.bg, Some(pal().shell), "a message again");
    key(&mut app, KeyCode::Char('!'));
    for c in "  cargo test -- --nocapture  ".chars() {
        key(&mut app, KeyCode::Char(c));
    }
    key(&mut app, KeyCode::Enter);
    assert_eq!(app.shell.as_deref(), Some("cargo test -- --nocapture"));
    assert!(!app.typing);
    assert_eq!(app.input, "");
    assert!(
        !app.world
            .journal
            .iter()
            .any(|e| matches!(&e.event, Event::YouSaid { .. })),
        "a shell command is not a message to the lead"
    );
    // A bare `!` runs nothing and says so.
    key(&mut app, KeyCode::Char('!'));
    key(&mut app, KeyCode::Enter);
    assert_eq!(app.shell.as_deref(), Some("cargo test -- --nocapture"));
    let s = render(&app, 140, 40);
    assert!(s.contains("the command is empty"), "{s}");
    key(&mut app, KeyCode::Char('!'));
    for c in "   ".chars() {
        key(&mut app, KeyCode::Char(c));
    }
    key(&mut app, KeyCode::Enter);
    assert_eq!(app.shell.as_deref(), Some("cargo test -- --nocapture"));
    // The demo runs commands where it was started; the shell line
    // is the same in Turkish.
    assert_eq!(app.cwd(), std::path::PathBuf::from("."));
    app.lang = Lang::Tr;
    key(&mut app, KeyCode::Char('!'));
    assert!(render(&app, 140, 40).contains("› kabuk:"));
    key(&mut app, KeyCode::Esc);
    // Both keys are in the help, whole, in both languages, `O` on the
    // `!` row.
    for (lang, bang, w) in [
        (
            Lang::En,
            "cmd in your shell; O quotes it; ^x^e $EDITOR",
            "home: first to decide · gc :changes a change",
        ),
        (
            Lang::Tr,
            "kmt kabuğunda; O alıntılar; ^x^e $EDITOR'da",
            "ana ekran: ilk karar · gc :changes değişiklik",
        ),
    ] {
        app.lang = lang;
        app.help = true;
        let s = render(&app, 140, 40);
        assert!(s.contains(bang), "{s}");
        let row = s.lines().find(|l| l.contains(w)).unwrap_or_default();
        assert!(row.contains(":waiting"), "{row}");
    }
}

#[test]
fn a_command_that_ran_is_journaled_under_the_project_and_flashed() {
    use crate::shell::{Caught, Ran};
    use std::time::Duration;
    let back = |ended: Ended, ms: u64| Ran {
        ended,
        took: Duration::from_millis(ms),
    };
    let ran = |ended: Ended, ms: u64| {
        Ok(Caught {
            ran: back(ended, ms),
            kept: Vec::new(),
            lines: 0,
        })
    };
    let mut app = app_after(2, 2, 0, Lang::En);
    app.selected = Sel::Project(1);
    app.ran("cargo test", ran(Ended::Exit(0), 2_400));
    let e = app.world.journal.last().unwrap();
    let leader = app.world.projects[1].leader;
    assert!(
        matches!(&e.event, Event::Shell { leader: l, command, ended: Ended::Exit(0), took: Some(2_400) }
            if *l == leader && command == "cargo test"),
        "{:?}",
        e.event
    );
    assert_eq!(e.event.agent(), leader);
    assert_eq!(e.event.kind(), EntryKind::Person);
    let s = render(&app, 140, 40);
    assert!(s.contains("you ran `cargo test` → exit 0 · 2 s"), "{s}");
    app.ran("sleep 9", ran(Ended::Signal(2), 61_000));
    let s = render(&app, 140, 40);
    assert!(
        s.contains("you ran `sleep 9` → stopped by signal 2 · 1 min 1 s"),
        "{s}"
    );
    app.ran("make", ran(Ended::Exit(2), 40));
    assert!(render(&app, 140, 40).contains("you ran `make` → exit 2 · 40 ms"));
    // A shell that could not start is said, not journaled.
    let n = app.world.journal.len();
    app.ran("x", Err(std::io::Error::other("no shell")));
    assert_eq!(app.world.journal.len(), n);
    let s = render(&app, 140, 40);
    assert!(s.contains("could not run `x`: no shell"), "{s}");
    // Turkish words it the same way, and the line under the output.
    app.lang = Lang::Tr;
    app.ran("make", ran(Ended::Exit(1), 3_000));
    let s = render(&app, 140, 40);
    assert!(
        s.contains("`make` çalıştırdın → çıkış 1 · 3 sn sürdü"),
        "{s}"
    );
    assert_eq!(
        app.shell_back(back(Ended::Exit(3), 5_000)),
        "[imece] çıkış 3 · 5 sn sürdü · herhangi bir tuş: ekrana dön\n"
    );
    assert_eq!(
        app.shell_back(back(Ended::Signal(9), 5_000)),
        "[imece] 9 sinyali durdurdu · 5 sn sürdü · herhangi bir tuş: ekrana dön\n"
    );
    app.lang = Lang::En;
    assert_eq!(
        app.shell_back(back(Ended::Exit(0), 7)),
        "[imece] exit 0 · 7 ms · any key: back to the screen\n"
    );
    assert_eq!(
        app.shell_back(back(Ended::Signal(15), 1_500)),
        "[imece] stopped by signal 15 · 1 s · any key: back to the screen\n"
    );
    // A line saved before the time was kept says none.
    let old = Event::Shell {
        leader,
        command: "ls".into(),
        ended: Ended::Exit(0),
        took: None,
    };
    let line = crate::ui::words::event(Lang::En, &app.world, &old);
    assert_eq!(line, "you ran `ls` → exit 0");
    // What the person typed is cleaned like everything else drawn.
    app.ran("echo \x1b[31mred TOKEN=abc123", ran(Ended::Exit(0), 1));
    let s = render(&app, 140, 40);
    assert!(!s.contains("abc123") && !s.contains('\x1b'), "{s}");
}

#[test]
fn a_colon_lead_is_no_command_any_more() {
    let mut app = app_after(1, 2, 0, Lang::En);
    for c in ":lead claude".chars() {
        key(&mut app, KeyCode::Char(c));
    }
    key(&mut app, KeyCode::Enter);
    assert!(render(&app, 140, 40).contains("no command lead"));
    assert!(
        !app.world
            .journal
            .iter()
            .any(|e| matches!(&e.event, Event::YouSaid { .. })),
        "a command is not a message to the lead"
    );
}

#[test]
fn the_history_search_says_so_over_the_line_and_the_help_names_its_keys() {
    let mut app = quiet_rows();
    app.world.recall_line("@claude-1 fix the clock");
    key(&mut app, KeyCode::Char('/'));
    let s = render(&app, 160, 40);
    assert!(!s.contains(" › history: "), "{s}");
    app.on_key(KeyEvent::new(KeyCode::Char('r'), KeyModifiers::CONTROL));
    let s = render(&app, 160, 40);
    assert!(s.contains(" › history: "), "{s}");
    assert!(s.contains("@claude-1 fix the clock"), "{s}");
    app.lang = Lang::Tr;
    assert!(render(&app, 160, 40).contains(" › geçmiş: "));
    key(&mut app, KeyCode::Esc);
    key(&mut app, KeyCode::Esc);
    app.help = true;
    let help = render(&app, 140, 40);
    assert!(
        help.contains("tab ↑ ↓ ^r    yazarken: tamamla · gönderilenler · :history"),
        "{help}"
    );
    app.lang = Lang::En;
    let help = render(&app, 140, 40);
    assert!(
        help.contains("tab ↑ ↓ ^r    writing: complete · sent lines · :history"),
        "{help}"
    );
}
