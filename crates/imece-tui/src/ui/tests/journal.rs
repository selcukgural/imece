use super::*;

#[test]
fn a_line_belongs_to_the_card_its_agent_was_on_then() {
    let (mut app, w) = story_team();
    let mut a = a_card(0, 0, Some(w[0]), Column::Done);
    a.finished = 10;
    let mut b = a_card(1, 0, Some(w[0]), Column::Done);
    b.finished = 20;
    let c = a_card(2, 0, Some(w[0]), Column::Working);
    let waiting = a_card(3, 0, Some(w[0]), Column::Waiting);
    let other = a_card(4, 0, Some(w[1]), Column::Working);
    app.world.cards = vec![a, b, c, waiting, other];
    let held = story::Held::of(&app.world);
    let at = |tick| held.at(&app.world, w[0], tick);
    assert_eq!(
        [at(5), at(10), at(11), at(20), at(21)],
        [Some(0), Some(0), Some(1), Some(1), Some(2)]
    );
    assert_eq!(held.at(&app.world, w[1], 5), Some(4));
    assert_eq!(held.at(&app.world, w[2], 5), None);
    // A helper's is its worker's.
    let (worker, [h, _]) = with_helpers(&mut app);
    let held = story::Held::of(&app.world);
    assert!(held.at(&app.world, worker, 5).is_some());
    assert_eq!(held.at(&app.world, h, 5), held.at(&app.world, worker, 5));
}

fn a_card(id: CardId, project: ProjectId, owner: Option<AgentId>, column: Column) -> Card {
    Card {
        id,
        project,
        title: format!("card {}", id + 1),
        owner,
        column,
        story: Vec::new(),
        symbols: Vec::new(),
        paths: Vec::new(),
        checks: Vec::new(),
        closes: None,
        started: 0,
        finished: 0,
        attempt: 1,
        breaker: Default::default(),
        item: Default::default(),
        after: Vec::new(),
    }
}

/// A calm team with no journal, no inbox, no cards, at tick 1000.
fn story_team() -> (App, Vec<AgentId>) {
    let (mut app, w, _) = calm_team();
    app.world.journal.clear();
    app.world.inbox.clear();
    app.world.cards.clear();
    app.world.tick = 1000;
    for &a in &w {
        app.world.agents[a].card = None;
        app.world.agents[a].plan.clear();
    }
    (app, w)
}

#[test]
fn what_was_said_goes_to_the_asker_s_card_and_setting_up_to_none() {
    let (mut app, w) = story_team();
    app.world.cards = vec![
        a_card(0, 0, Some(w[0]), Column::Working),
        a_card(1, 0, Some(w[1]), Column::Working),
    ];
    let held = story::Held::of(&app.world);
    let lead = app.world.projects[0].leader;
    let of = |event: Event| story::card_for(&app.world, &held, &Entry { tick: 1000, event });
    assert_eq!(
        of(Event::Claimed {
            agent: w[1],
            card: 0
        }),
        Some(0),
        "named"
    );
    assert_eq!(of(sent(w[0], w[1], MsgKind::Ask, 3, None)), Some(0));
    assert_eq!(of(sent(w[1], w[0], MsgKind::Answer, 4, Some(3))), Some(0));
    assert_eq!(of(sent(lead, w[1], MsgKind::Inform, 5, None)), Some(1));
    assert_eq!(of(sent(w[1], lead, MsgKind::Answer, 6, Some(5))), Some(1));
    assert_eq!(of(sent(lead, w[3], MsgKind::Inform, 7, None)), None);
    assert_eq!(
        of(Event::Replied {
            agent: w[1],
            text: "x".into()
        }),
        Some(1),
        "to the person"
    );
    assert_eq!(
        of(Event::ChangeReady {
            agent: w[1],
            files: 1,
            added: 1,
            removed: 0,
            spend: None
        }),
        Some(1)
    );
    assert_eq!(
        of(Event::Stopped {
            agent: w[0],
            why: "x".into()
        }),
        Some(0)
    );
    assert_eq!(of(Event::Failed { agent: w[0] }), Some(0));
    assert_eq!(
        of(Event::Copied {
            agent: w[0],
            paths: vec![".env".into()],
            bytes: 1,
            included: false
        }),
        None
    );
    assert_eq!(
        of(Event::CopySkipped {
            agent: w[0],
            path: ".env".into(),
            why: "x".into(),
            included: false
        }),
        None,
        "a failure of setting up is no card's"
    );
}

#[test]
fn the_person_holds_up_who_waits_for_them_and_who_waits_for_those() {
    let (mut app, w) = story_team();
    assert!(story::held(&app.world).is_empty());
    waiting(&mut app, w[2]);
    waiting(&mut app, w[2]);
    app.world.agents[w[3]].status = Status::WaitingHuman;
    app.world.agents[w[0]].status = Status::Asking { to: w[1], until: 0 };
    app.world.agents[w[1]].status = Status::Asking { to: w[2], until: 0 };
    assert_eq!(
        story::held(&app.world),
        [
            (w[2], None),
            (w[3], None),
            (w[1], Some(w[2])),
            (w[0], Some(w[1]))
        ]
    );
    app.world.agents[w[3]].gone = true;
    assert!(!story::held(&app.world).contains(&(w[3], None)));
    // Asking one nobody holds up holds nobody up.
    let lead = app.world.projects[0].leader;
    app.world.agents[lead].status = Status::Asking { to: w[3], until: 0 };
    assert!(story::held(&app.world).iter().all(|&(a, _)| a != lead));
    app.story = true;
    app.open_window(Window::Journal);
    let s = render(&app, 240, 50);
    assert!(
        s.contains(&format!(
            "! needs you (3): {}, {} via {}, {} via {}",
            app.world.agents[w[2]].name,
            app.world.agents[w[1]].name,
            app.world.agents[w[2]].name,
            app.world.agents[w[0]].name,
            app.world.agents[w[1]].name
        )),
        "{s}"
    );
}

/// The screen with the story in the side panel, wide enough for its lines.
fn render_story(app: &mut App) -> String {
    app.open_window(Window::Journal);
    assert!(app.story_on());
    render(app, 200, 50)
}

#[test]
fn the_story_tells_each_card_newest_first_and_folds_the_rest_by_kind() {
    let (mut app, w) = story_team();
    app.story = true;
    let mut done = a_card(0, 0, Some(w[0]), Column::Done);
    done.finished = 995;
    app.world.cards = vec![done, a_card(1, 0, Some(w[1]), Column::Working)];
    app.world.agents[w[1]].card = Some(1);
    plan_of(&mut app, w[1]);
    let lead = app.world.projects[0].leader;
    let at = |tick, event| Entry { tick, event };
    let copied = |agent| Event::Copied {
        agent,
        paths: vec!["node_modules".into()],
        bytes: 1,
        included: false,
    };
    app.world.journal = vec![
        at(900, sent(w[0], lead, MsgKind::Progress, 1, None)),
        at(950, sent(w[0], lead, MsgKind::Progress, 2, None)),
        at(960, sent(w[0], lead, MsgKind::Progress, 3, None)),
        at(990, sent(w[0], lead, MsgKind::Done, 4, None)),
        at(991, copied(w[2])),
        at(992, copied(w[3])),
        at(976, sent(w[1], w[3], MsgKind::Ask, 6, None)),
        at(999, sent(w[1], w[2], MsgKind::Ask, 7, None)),
        at(1000, sent(w[2], w[1], MsgKind::Answer, 8, Some(7))),
    ];
    // The journal is in time order; the asks of card 2 came last.
    app.world.journal.sort_by_key(|e| e.tick);
    let s = render_story(&mut app);
    let at_line = |words: &str| s.lines().position(|l| l.contains(words));
    let two = at_line("#2 card 2").expect("card 2");
    let one = at_line("#1 card 1").expect("card 1");
    assert!(two < one, "newest first: {s}");
    assert!(s.contains("#1 card 1 ·") && s.contains("✓ done"), "{s}");
    // Three newest lines, oldest first; the oldest left out.
    assert!(!s.contains("[progress]: m1"), "{s}");
    let m2 = at_line("[progress]: m2").unwrap();
    let m4 = at_line("[done]: m4").unwrap();
    assert!(one < m2 && m2 < m4, "{s}");
    // An ask answered says nothing more; one not: since when.
    let ask = |id: &str| {
        s.lines()
            .find(|l| l.contains(&format!("[ask]: {id}")))
            .unwrap_or("")
            .to_string()
    };
    assert!(!ask("m7").contains("no answer"), "{s}");
    assert!(ask("m6").contains("no answer · 2 min"), "{s}");
    // Its owner now, and how far its list is.
    assert!(s.contains("#2 card 2 ·") && s.contains("≡ 1/4"), "{s}");
    assert!(s.contains("now ▸ Run the tests"), "{s}");
    // The rest by kind, the newest of each with how many.
    let elsewhere = at_line("elsewhere").expect("elsewhere");
    assert!(elsewhere > one, "{s}");
    assert!(
        s.lines()
            .any(|l| l.contains(&app.world.agents[w[3]].name) && l.contains("×2")),
        "{s}"
    );
    // A change waiting on a done card says both.
    app.world.inbox.push(InboxItem {
        kind: InboxKind::Change,
        agent: w[0],
        detail: "x".into(),
        researcher: None,
        open: true,
        tick: 0,
        card: Some(0),
    });
    assert!(render_story(&mut app).contains("✓ done · change ready"));
    // Cleared lines are not told, a marked one still is.
    app.world.inbox.clear();
    app.world.cleared.journal = app.world.journal.len();
    let s = render_story(&mut app);
    assert!(s.contains("Nothing has happened yet"), "{s}");
    app.world.marks.insert(app.world.journal.len() - 1);
    assert!(render_story(&mut app).contains("[answer]"));
    // Cleared up to the last line: the last is still told.
    app.world.marks.clear();
    app.world.cleared.journal = app.world.journal.len() - 1;
    let s = render_story(&mut app);
    assert!(
        s.contains("[answer] ↩: m8") && !s.contains("[ask]: m7"),
        "{s}"
    );
    app.world.cleared.journal = 0;
    // An archived project's lines are its archive's.
    app.world.projects[0].archived = Some(Archived {
        at: 0,
        title: "old".into(),
    });
    assert!(render_story(&mut app).contains("Nothing has happened yet"));
}

#[test]
fn the_story_marks_only_numbered_asks_folds_four_kinds_and_says_a_change() {
    let (mut app, w) = story_team();
    app.story = true;
    app.world.cards = vec![a_card(0, 0, Some(w[0]), Column::Working)];
    app.world.agents[w[0]].card = Some(0);
    let lead = app.world.projects[0].leader;
    let at = |tick, event| Entry { tick, event };
    app.world.journal = vec![
        // An ask from before numbers (0) is not one that waits.
        at(990, sent(w[0], w[1], MsgKind::Ask, 0, None)),
        // One under a minute old waits "yet".
        at(996, sent(w[0], w[1], MsgKind::Ask, 9, None)),
        at(991, Event::Failed { agent: lead }),
        at(
            992,
            Event::Stopped {
                agent: lead,
                why: "x".into(),
            },
        ),
        at(
            993,
            Event::Copied {
                agent: w[2],
                paths: vec!["x".into()],
                bytes: 1,
                included: false,
            },
        ),
        at(
            994,
            Event::Hired {
                by: lead,
                agent: w[3],
                model: None,
                from: None,
            },
        ),
        at(
            995,
            Event::Retrying {
                agent: w[2],
                why: "x".into(),
                attempt: 1,
                seconds: 30,
            },
        ),
    ];
    let s = render_story(&mut app);
    let ask = s.lines().find(|l| l.contains("[ask]: m0")).unwrap();
    assert!(!ask.contains("no answer"), "{s}");
    let ask = s.lines().find(|l| l.contains("[ask]: m9")).unwrap();
    assert!(ask.contains("no answer yet"), "{s}");
    let start = s.lines().position(|l| l.contains("elsewhere")).unwrap();
    let folded = (s.lines().skip(start + 1))
        .take_while(|l| l.contains(":") && l.contains("│  1"))
        .count();
    assert_eq!(folded, story::STORY_OTHER, "{s}");
    assert!(!s.contains("×1"), "{s}");
    // Another card's change, or this card's item that is no change,
    // says nothing on its header.
    plan_of(&mut app, w[0]);
    for (kind, card) in [(InboxKind::Change, Some(1)), (InboxKind::Reply, Some(0))] {
        app.world.inbox = vec![InboxItem {
            kind,
            agent: w[0],
            detail: "x".into(),
            researcher: None,
            open: true,
            tick: 0,
            card,
        }];
        let s = render_story(&mut app);
        // The window's line, under Home's preview that names the card.
        let head = s.lines().rev().find(|l| l.contains("#1 card 1")).unwrap();
        assert!(!head.contains("Δ") && head.contains('≡'), "{head}");
    }
    // A done card says done, not its owner's list.
    app.world.inbox.clear();
    app.world.cards[0].column = Column::Done;
    app.world.cards[0].finished = 1000;
    let s = render_story(&mut app);
    let head = s.lines().rev().find(|l| l.contains("#1 card 1")).unwrap();
    assert!(head.contains("✓ done") && !head.contains('≡'), "{head}");
    // A failed one says so, and its owner has no line of now.
    app.world.cards[0].column = Column::Failed;
    let s = render_story(&mut app);
    let head = s.lines().rev().find(|l| l.contains("#1 card 1")).unwrap();
    assert!(
        head.contains("✕ failed") && !head.contains("✓ done"),
        "{head}"
    );
    assert!(!head.contains('≡') && !s.contains("now ▸"), "{s}");
    // A withdrawn one says so, dim (record 126).
    app.world.cards[0].column = Column::Withdrawn;
    let s = render_story(&mut app);
    let head = s.lines().rev().find(|l| l.contains("#1 card 1")).unwrap();
    assert!(
        head.contains("– withdrawn") && !head.contains("✕"),
        "{head}"
    );
    app.world.cards[0].column = Column::Working;
    // A change waiting on a card not done: that, not its list; the
    // owner off the card has no line of now.
    app.world.inbox.push(InboxItem {
        kind: InboxKind::Change,
        agent: w[0],
        detail: "x".into(),
        researcher: None,
        open: true,
        tick: 0,
        card: Some(0),
    });
    let s = render_story(&mut app);
    let head = s.lines().rev().find(|l| l.contains("#1 card 1")).unwrap();
    assert!(
        head.contains(" ·  change ready") || head.contains("  change ready"),
        "{head}"
    );
    assert!(!head.contains('≡') && !head.contains("✓ done"), "{head}");
    assert!(s.contains("now ▸ Run the tests"), "{s}");
    app.world.agents[w[0]].card = None;
    assert!(!render_story(&mut app).contains("now ▸"));
}

#[test]
fn j_tells_the_story_and_its_keys_move_and_open_what_it_shows() {
    let (mut app, w) = story_team();
    app.world.cards = vec![
        a_card(0, 0, Some(w[0]), Column::Working),
        a_card(1, 0, Some(w[1]), Column::Working),
    ];
    app.world.agents[w[0]].card = Some(0);
    app.world.agents[w[1]].card = Some(1);
    let at = |tick, event| Entry { tick, event };
    app.world.journal = vec![
        at(
            990,
            Event::Copied {
                agent: w[2],
                paths: vec!["x".into()],
                bytes: 1,
                included: false,
            },
        ),
        at(995, sent(w[0], w[1], MsgKind::Inform, 1, None)),
        at(999, sent(w[1], w[0], MsgKind::Inform, 2, None)),
    ];
    assert!(!app.story);
    // `J` opens the journal's window; `J` there tells the story.
    key(&mut app, KeyCode::Char('J'));
    assert_eq!(app.window, Some(Window::Journal));
    assert!(!app.story);
    key(&mut app, KeyCode::Char('J'));
    assert!(app.story);
    let s = render(&app, 200, 50);
    assert!(s.contains("STORY · J journal"), "{s}");
    assert!(s.contains("the journal tells the story by card"), "{s}");
    // The window's pane is the journal's; no journal line is selected
    // under the story.
    assert_eq!((app.pane, app.journal_sel), (Pane::Journal, None));
    // Card 2, card 1, then the line of no card.
    render(&app, 200, 50);
    assert_eq!(app.story_items.borrow().len(), 3);
    key(&mut app, KeyCode::Char('j'));
    key(&mut app, KeyCode::Char('j'));
    key(&mut app, KeyCode::Char('j'));
    assert_eq!(app.story_sel, 2, "the last stays the last");
    key(&mut app, KeyCode::Char('k'));
    assert_eq!(app.story_sel, 1);
    let s = render(&app, 200, 50);
    let cell = |words: &str| {
        let y = s
            .lines()
            .collect::<Vec<_>>()
            .into_iter()
            .rposition(|l| l.contains(words))
            .unwrap();
        let x = s
            .lines()
            .nth(y)
            .unwrap()
            .chars()
            .position(|c| c == '#')
            .unwrap();
        buffer(&app, 200, 50)[(x as u16, y as u16)].modifier
    };
    assert!(cell("#1 card 1").contains(Modifier::REVERSED), "{s}");
    assert!(!cell("#2 card 2").contains(Modifier::REVERSED), "{s}");
    key(&mut app, KeyCode::Char('g'));
    assert_eq!(app.story_sel, 0);
    key(&mut app, KeyCode::Char('G'));
    assert_eq!(app.story_sel, 2);
    key(&mut app, KeyCode::Char('k'));
    // The agent at hand is the card's owner, or the line's agent; the
    // card at hand the item's.
    assert_eq!(app.agent_at_hand(), Some(w[0]));
    app.story_sel = 2;
    assert_eq!(app.agent_at_hand(), Some(w[2]));
    assert_eq!(app.card_at_hand(), None, "a line of no card");
    app.story_sel = 1;
    assert_eq!(app.card_at_hand(), Some(0));
    // enter on a card: its detail, the window closed; `J` there opens
    // the window again, the story still on.
    key(&mut app, KeyCode::Enter);
    assert_eq!((app.card, app.window), (Some(0), None));
    assert!(!app.story_on(), "a card's screen is its own");
    key(&mut app, KeyCode::Char('J'));
    assert_eq!(app.window, Some(Window::Journal));
    assert!(app.story);
    key(&mut app, KeyCode::Esc);
    assert_eq!(app.window, None);
    key(&mut app, KeyCode::Esc);
    assert_eq!(app.card, None);
    // enter on a line of no card: the journal, at that line, in the
    // story's place; what is kept says so.
    key(&mut app, KeyCode::Char('J'));
    render(&app, 200, 50);
    app.story_sel = 2;
    key(&mut app, KeyCode::Enter);
    assert!(app.story && app.story_away && !app.story_on());
    assert_eq!(app.journal_sel, Some(0));
    let shown = render(&app, 200, 50);
    assert!(shown.contains("JOURNAL · esc story"), "{shown}");
    assert!(shown.contains("the line in the journal · esc: back to the story"));
    // esc goes back to the story, at the item it left; the window stays.
    key(&mut app, KeyCode::Esc);
    assert!(app.story_on() && !app.story_away);
    assert_eq!(app.window, Some(Window::Journal));
    assert_eq!((app.journal_sel, app.story_sel), (None, 2));
    assert!(render(&app, 200, 50).contains("the journal tells the story by card"));
    // So does J, and it leaves the story on.
    key(&mut app, KeyCode::Enter);
    assert!(app.story_away);
    key(&mut app, KeyCode::Char('J'));
    assert!(app.story_on() && !app.story_away);
    // A second esc closes the window; a focus under it is untouched.
    app.zoom = Zoom::Focus(w[0]);
    key(&mut app, KeyCode::Enter);
    assert!(app.story_away);
    key(&mut app, KeyCode::Esc);
    assert!(!app.story_away && app.window.is_some());
    key(&mut app, KeyCode::Esc);
    assert_eq!((app.window, app.zoom), (None, Zoom::Focus(w[0])));
    app.zoom = Zoom::None;
    // :story opens the window and turns the story off and on.
    app.command("story");
    assert_eq!(app.window, Some(Window::Journal));
    assert!(!app.story);
    app.command("story");
    assert!(app.story);
    assert_eq!((app.journal_sel, app.story_sel), (None, 0));
    // Off again: a journal line is selected, and j k move it, not the
    // story's.
    key(&mut app, KeyCode::Char('J'));
    assert!(!app.story);
    assert!(app.journal_sel.is_some());
    let before = app.journal_sel;
    key(&mut app, KeyCode::Char('j'));
    assert_ne!(app.journal_sel, before);
    assert_eq!(app.story_sel, 0);
    let shown = render(&app, 200, 50);
    assert!(shown.contains("the journal line by line"));
    assert!(shown.contains("JOURNAL · J story"), "the key back, off");
    app.flash = None;
    // In Turkish too.
    app.lang = Lang::Tr;
    assert!(render(&app, 200, 50).contains("DEFTER · J hikâye"));
    app.story_away = true;
    assert!(render(&app, 200, 50).contains("DEFTER · esc hikâye"));
}

#[test]
fn the_story_keeps_the_selected_card_in_view() {
    let (mut app, w) = story_team();
    app.story = true;
    app.pane = Pane::Journal;
    let n = 20;
    app.world.cards = (0..n)
        .map(|c| a_card(c, 0, Some(w[c % 4]), Column::Done))
        .collect();
    // Two lines a card: a block takes three rows.
    app.world.journal = (0..n)
        .flat_map(|c| {
            let (tick, agent) = (900 + c as u64, w[c % 4]);
            [
                Entry {
                    tick,
                    event: Event::Claimed { agent, card: c },
                },
                Entry {
                    tick,
                    event: Event::Merged { agent, card: c },
                },
            ]
        })
        .collect();
    app.open_window(Window::Journal);
    render(&app, 200, 30);
    let shown = app.story_items.borrow().len();
    // The window's 24 rows hold half as many cards and one more.
    assert_eq!(shown, 24 / 2 + 1);
    app.story_sel = shown - 1;
    let s = render(&app, 200, 30);
    let last = match app.story_items.borrow()[shown - 1] {
        story::Item::Card(c) => c,
        story::Item::Line(_) => unreachable!(),
    };
    assert!(
        s.contains(&format!("#{} card {}", last + 1, last + 1)),
        "{s}"
    );
    assert!(!s.contains("#20 card 20 ·"), "the first scrolled off: {s}");
}

#[test]
fn the_journal_says_where_an_earlier_day_begins() {
    let mut app = talking_over_two_days();
    app.open_window(Window::Journal);
    let screen = render(&app, 140, 40);
    let lines: Vec<&str> = screen.lines().collect();
    let at = |needle: &str| {
        lines
            .iter()
            .position(|l| l.contains(needle))
            .unwrap_or_else(|| panic!("{needle} not on\n{screen}"))
    };
    let day_row = at("─ 1970-01-01");
    assert_eq!(
        screen.matches("─ 1970-0").count(),
        1,
        "one day row: today's lines need none\n{screen}"
    );
    // Newest first: today's conversation, then yesterday under its date.
    assert!(at("00:00 ") < day_row, "{screen}");
    assert_eq!(at("14:00 ⇄"), day_row + 1, "the workers' talk\n{screen}");
    assert!(lines[day_row + 2].contains("14:00 "), "{screen}");
    // Nothing of today: the newest line stands under its own date.
    let mut app = app;
    for e in &mut app.world.journal {
        e.tick = 0;
    }
    let screen = render(&app, 140, 40);
    let lines: Vec<&str> = screen.lines().collect();
    let day_row = lines
        .iter()
        .position(|l| l.contains("─ 1970-01-01"))
        .unwrap();
    assert!(lines[day_row - 1].contains("JOURNAL"), "{screen}");
    assert!(lines[day_row + 1].contains("14:00 "), "{screen}");
    // All of today: no date at all.
    app.world.tick = 10;
    assert!(!render(&app, 140, 40).contains("─ 1970"));
}

#[test]
fn the_journal_highlights_the_selected_line_and_not_the_first() {
    let mut app = talking();
    app.open_window(Window::Journal);
    // Rows newest first: the conversation (4 over 3), the workers'
    // talk (2), the older conversation (1 over 0).
    app.journal_sel = Some(2);
    let screen = render(&app, 140, 40);
    let lines: Vec<&str> = screen.lines().collect();
    let keys = lines
        .iter()
        .position(|l| l.contains("enter open the conversation"))
        .unwrap_or_else(|| panic!("{screen}"));
    assert!(
        lines[keys - 1].contains("⇄") && lines[keys - 1].contains("claude-1"),
        "the keys stand under the talk, the selected line\n{screen}"
    );
}

#[test]
fn the_journal_scrolled_to_an_old_line_knows_the_day_above_its_first_row() {
    // Forty events that never fold (the talk is between two pairs in
    // turn), two a day, newest first: rows 2k and 2k+1 share a day. The
    // first row shown gets a day row exactly when it is the first of
    // its pair, whatever the window's height makes the first row.
    let mut app = app_after(1, 4, 0, Lang::En);
    app.world.journal.clear();
    for i in 0..40u64 {
        app.world.journal.push(Entry {
            tick: (i / 2) * 17_280,
            event: Event::Talk {
                from: 1 + (i % 2) as usize,
                to: 3,
                text: format!("m{i}"),
            },
        });
    }
    app.world.tick = 19 * 17_280 + 100;
    app.open_window(Window::Journal);
    app.journal_sel = Some(0);
    let mut parities = std::collections::BTreeSet::new();
    for height in [30u16, 31, 32, 33, 34, 35] {
        let screen = render(&app, 140, height);
        let lines: Vec<&str> = screen.lines().collect();
        let head = lines
            .iter()
            .position(|l| l.contains("JOURNAL"))
            .unwrap_or_else(|| panic!("{screen}"));
        let dated = lines[head + 1].contains("─ 1970-");
        let first = lines[head + 1 + usize::from(dated)];
        let i: usize = first
            .split("m")
            .nth(1)
            .and_then(|t| t.split(|c: char| !c.is_ascii_digit()).next())
            .and_then(|n| n.parse().ok())
            .unwrap_or_else(|| panic!("no entry on {first:?}\n{screen}"));
        let row = 39 - i;
        assert_eq!(
            dated,
            row.is_multiple_of(2),
            "row {row} first at height {height}\n{screen}"
        );
        assert!(
            lines.iter().any(|l| l.contains(": m0 ")),
            "the selected, oldest line is in view\n{screen}"
        );
        parities.insert(row % 2);
    }
    assert_eq!(parities.len(), 2, "both cases were seen");
}

#[test]
fn a_marked_line_shows_its_mark_once_in_the_gutter_and_in_the_journal() {
    let mut app = talking();
    app.open_thread(4);
    let entries = app.thread.as_ref().unwrap().entries.clone();
    // The long last message, selected, and the first one, not.
    app.world.marks.extend([entries[0], entries[3]]);
    let th = app.thread.as_ref().unwrap();
    let (rows, ranges) = overlay::thread_rows(app.lang, &app.world, th, 60);
    let gutter = |n: usize| -> String {
        let r: &Line = &rows[n];
        r.spans.iter().take(2).map(|s| s.content.as_ref()).collect()
    };
    for (at, first) in [(0, " ✱"), (3, "▌✱")] {
        let (start, end) = ranges[at];
        assert_eq!(gutter(start), first);
        assert_eq!(rows[start].spans[1].style, bold(pal().accent));
        for n in start + 1..end {
            assert!(!gutter(n).contains(MARK), "only its first row: {n}");
        }
    }
    let (start, _) = ranges[1];
    assert_eq!(gutter(start), "  ", "an unmarked line");
    assert!(rows.iter().all(|r| r.width() <= 60));
    // In the journal the mark takes the space after the clock.
    let e = &app.world.journal[entries[0]];
    let plain = entry_line(&app, e);
    let line = bookmarked(entry_line(&app, e));
    assert_eq!(line.width(), plain.width(), "aligned with the others");
    let clock = plain.spans[0].content.trim_end().to_string();
    assert_eq!(line.spans[0].content, clock);
    assert_eq!(line.spans[1].content, MARK);
    assert_eq!(line.spans[1].style, bold(pal().accent));
    assert_eq!(line.spans[2..], plain.spans[1..]);
}

#[test]
fn the_journal_row_shows_a_message_as_plain_words() {
    let app = app_after(1, 2, 10, Lang::En);
    let e = Event::Replied {
        agent: 0,
        text: "**Done**\n\n- `a.rs`\n- b".into(),
    };
    let said = words::said(&e).unwrap();
    let text = said_spans(Lang::En, &app.world, &said)
        .iter()
        .map(|s| s.content.to_string())
        .collect::<String>();
    assert!(text.ends_with(": Done a.rs b"), "{text}");
}
