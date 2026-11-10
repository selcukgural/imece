use super::*;

#[test]
fn waiting_cleared_off_the_screen_is_counted_in_its_title() {
    let mut app = app_after(1, 4, 0, Lang::En);
    app.world.inbox.clear();
    for agent in [1, 2, 3] {
        app.world.inbox.push(crate::model::InboxItem {
            kind: InboxKind::Question,
            agent,
            detail: "x".into(),
            researcher: None,
            open: true,
            tick: 0,
            card: None,
        });
    }
    app.world.cleared.inbox = [0, 1].into();
    let s = render(&app, 140, 40);
    assert!(
        s.contains("TO DECIDE 1 question · 2 hidden (:unclear)"),
        "{s}"
    );
    app.world.cleared.inbox.insert(2);
    let s = render(&app, 140, 40);
    assert!(s.contains("TO DECIDE 3 hidden (:unclear)"), "{s}");
    // A card's screen lists them, so says nothing of it.
    app.world.inbox[0].card = Some(0);
    app.open_card(0);
    let s = render(&app, 140, 40);
    assert!(!s.contains("hidden"), "{s}");
    app.leave_card();
    app.world.inbox[1].card = Some(0);
    app.world.inbox.iter_mut().for_each(|i| i.open = false);
    let s = render(&app, 140, 40);
    assert!(!s.contains("TO DECIDE") && !s.contains("hidden"), "{s}");
    // The grace counts down in the bottom line, for each kind.
    let at = std::time::Instant::now() + crate::app::UNDO_GRACE;
    for (what, words) in [
        (
            crate::app::Clear::All,
            "clearing TO DECIDE and the journal off the screen in 10 s",
        ),
        (
            crate::app::Clear::Inbox,
            "clearing TO DECIDE off the screen in 10 s",
        ),
        (
            crate::app::Clear::Journal,
            "clearing the journal off the screen in 10 s",
        ),
    ] {
        app.grace = Some((crate::app::Undoable::Clear(what), at));
        let s = render(&app, 140, 40);
        assert!(s.contains(words), "{s}");
    }
}

#[test]
fn an_answer_is_in_the_inbox_and_enter_reads_it_whole() {
    let mut app = app_after(1, 4, 50, Lang::En);
    let answer = "Short first line.\nTHE-REST";
    app.world.note(Event::Replied {
        agent: 0,
        text: answer.into(),
    });
    app.world.replied(0, answer);
    let screen = render(&app, 140, 40);
    assert!(screen.contains("answered: Short first line."), "{screen}");
    app.at_home.at = Some(HomeRow::Item(*app.world.open_inbox().last().unwrap()));
    key(&mut app, KeyCode::Enter);
    assert!(app.thread.is_some(), "the conversation opens");
    assert!(render(&app, 140, 40).contains("THE-REST"));
    assert!(
        app.world
            .open_inbox()
            .iter()
            .all(|&i| app.world.inbox[i].kind != InboxKind::Reply)
    );
}

#[test]
fn a_vendor_error_shows_its_retry_on_the_row_then_waits_for_the_person_in_the_inbox() {
    let mut app = app_after(1, 4, 50, Lang::En);
    let worker = app
        .world
        .agents
        .iter()
        .find(|a| a.role == Role::Worker)
        .unwrap()
        .id;
    let name = app.world.agents[worker].name.clone();
    app.world.agents[worker].status = Status::Idle;
    let words = |app: &App, lang| {
        status_spans(lang, &app.world, &app.world.agents[worker])
            .iter()
            .map(|s| s.content.to_string())
            .collect::<String>()
    };
    // A retry 2 of 3, five ticks (25 s) and then 25 ticks off.
    let now = app.world.tick;
    app.world.agents[worker].retry = Some(Retry {
        attempt: 2,
        at: now + 5,
    });
    assert_eq!(words(&app, Lang::En), "↻ 2/3 in 25 s");
    assert_eq!(words(&app, Lang::Tr), "↻ 2/3 · 25 sn sonra");
    app.world.agents[worker].retry = Some(Retry {
        attempt: 3,
        at: now + 25,
    });
    assert_eq!(words(&app, Lang::En), "↻ 3/3 in 2 min 5 s");
    app.world.agents[worker].retry = Some(Retry {
        attempt: 1,
        at: now.saturating_sub(1),
    });
    assert!(
        words(&app, Lang::En).starts_with("↻ 1/3 in 0"),
        "{}",
        words(&app, Lang::En)
    );
    // A mid-turn agent's row is its work, a retry or not.
    app.world.agents[worker].status = Status::Working;
    assert!(!words(&app, Lang::En).contains('↻'));
    app.world.agents[worker].status = Status::Idle;
    // Then the person's: on the row, in the inbox, its keys write to it.
    app.world.agents[worker].retry = None;
    app.world.turn_failed(
        worker,
        "Invalid API key · Fix external API key",
        Some(Fault::Account),
    );
    assert_eq!(
        words(&app, Lang::En),
        "■ API error \"Invalid API key · Fix ext…\"",
        "cut to the status's width"
    );
    assert!(words(&app, Lang::Tr).starts_with("■ API hatası \"Invalid"));
    let i = *app.world.open_inbox().last().unwrap();
    assert_eq!(inbox_line(&app, i).spans[0].content, "■ ");
    assert_eq!(
        words::inbox(Lang::En, &app.world, &app.world.inbox[i]),
        format!(
            "{name} API error, write to it once mended: Invalid API key · Fix external API key"
        )
    );
    app.at_home.at = Some(HomeRow::Item(*app.world.open_inbox().last().unwrap()));
    for k in ['a', 'r'] {
        key(&mut app, KeyCode::Char(k));
        assert!(
            app.typing && app.input == format!("@{name} "),
            "{k}: {:?}",
            app.input
        );
        app.typing = false;
        app.input.clear();
    }
    assert!(app.world.open_item(worker, InboxKind::Fault).is_some());
    app.world.said(worker, "Mended.");
    assert!(app.world.open_item(worker, InboxKind::Fault).is_none());
}

#[test]
fn a_question_in_the_inbox_opens_the_conversation_and_the_composer_to_its_agent() {
    let mut app = app_after(1, 4, 50, Lang::En);
    let worker = app
        .world
        .agents
        .iter()
        .find(|a| a.role == Role::Worker)
        .unwrap()
        .id;
    let name = app.world.agents[worker].name.clone();
    app.world.agents[worker].status = Status::Idle;
    app.world.note(Event::ToYou {
        from: worker,
        kind: MsgKind::Ask,
        text: "Which API?\nTHE-REST".into(),
        options: Vec::new(),
    });
    app.world.asked_you(worker, "Which API?\nTHE-REST");
    let screen = render(&app, 140, 40);
    assert!(screen.contains("Which API?"), "{screen}");
    // The worker's line says it waits for the person.
    let words = |app: &App, lang| {
        status_spans(lang, &app.world, &app.world.agents[worker])
            .iter()
            .map(|s| s.content.to_string())
            .collect::<String>()
    };
    assert!(
        words(&app, Lang::En).ends_with(" ? → you \"Which API?\""),
        "{:?}",
        words(&app, Lang::En)
    );
    assert!(
        words(&app, Lang::Tr).ends_with(" ? → sen \"Which API?\""),
        "{:?}",
        words(&app, Lang::Tr)
    );
    let i = *app.world.open_inbox().last().unwrap();
    app.at_home.at = Some(HomeRow::Item(i));
    key(&mut app, KeyCode::Enter);
    assert!(app.thread.is_some(), "the conversation opens");
    assert!(render(&app, 140, 40).contains("THE-REST"));
    assert!(app.world.has_question(worker), "reading is not answering");
    app.thread = None;
    for k in ['a', 'r'] {
        key(&mut app, KeyCode::Char(k));
        assert!(
            app.typing && app.input == format!("@{name} "),
            "{k}: {:?}",
            app.input
        );
        app.typing = false;
        app.input.clear();
    }
    assert!(app.world.has_question(worker));
    // Writing back closes it.
    app.world.said(worker, "This one.");
    assert!(!app.world.has_question(worker));
    assert_eq!(words(&app, Lang::En), "idle");
}

#[test]
fn an_item_put_off_says_later_up_front_in_both_languages() {
    let mut app = app_after(1, 4, 0, Lang::En);
    app.world.inbox.clear();
    for n in 0..2 {
        app.world.inbox.push(InboxItem {
            kind: InboxKind::Sensitive,
            agent: 1,
            detail: format!("item {n}"),
            researcher: None,
            open: true,
            tick: 0,
            card: None,
        });
    }
    app.deferred = vec![1];
    let third = |app: &App, i| inbox_line(app, i).spans[2].clone();
    assert!(
        third(&app, 0).content.ends_with("item 0"),
        "{:?}",
        third(&app, 0)
    );
    assert_eq!(
        (third(&app, 1).content.as_ref(), third(&app, 1).style),
        ("later · ", dim()),
        "after the icon and the time"
    );
    app.lang = Lang::Tr;
    assert_eq!(third(&app, 1).content, "sonra · ");
}

/// The project's name stands on an inbox line only when there are
/// several projects to tell apart.
#[test]
fn an_inbox_line_names_the_project_only_among_several() {
    for (projects, named) in [(1, false), (2, true)] {
        let mut app = app_after(projects, 4, 0, Lang::En);
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
        let line = text(&[inbox_line(&app, app.world.inbox.len() - 1)]).remove(0);
        let name = &app.world.projects[0].name;
        assert_eq!(
            line.contains(&format!(" {name} ")),
            named,
            "{projects} projects: {line}"
        );
        assert!(line.starts_with("! 14:00 "), "{line}");
    }
}
