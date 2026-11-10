use super::*;

#[test]
fn the_header_lets_the_least_wanted_part_go_first_and_never_a_kept_one() {
    let part = |text: &str, glued: bool, keep: u8| HeadPart {
        spans: vec![Span::raw(text.to_string())],
        glued,
        keep,
    };
    let parts = || {
        vec![
            part("AAAA", false, KEEP),
            part("BB", false, 2),
            part("CCC", false, 5),
            part("dd", true, 2),
            part("E", false, 1),
        ]
    };
    let text = |width: usize| spans_text(&fit_header(parts(), width).spans);
    // The brand's eight, twelve of text, three separators of three.
    assert_eq!(text(29), " imece  AAAA │ BB │ CCCdd │ E");
    assert_eq!(text(28), " imece  AAAA │ BB │ CCCdd", "the lowest goes");
    assert_eq!(text(25), " imece  AAAA │ BB │ CCCdd");
    assert_eq!(text(24), " imece  AAAA │ BB │ CCC", "of equals the last");
    assert_eq!(text(23), " imece  AAAA │ BB │ CCC");
    assert_eq!(text(22), " imece  AAAA │ CCC", "with its separator");
    assert_eq!(text(18), " imece  AAAA │ CCC");
    assert_eq!(text(17), " imece  AAAA");
    assert_eq!(text(12), " imece  AAAA", "the brand while it fits");
    assert_eq!(text(11), " AAAA", "then the brand goes, a kept part never");
    assert_eq!(text(3), " AAAA", "a kept part never goes");
}

#[test]
fn the_header_puts_what_waits_first_and_keeps_a_low_pot_and_a_full_window() {
    let mut app = app_after(1, 2, 0, Lang::En);
    let keeps = |app: &App, words: &str| -> Option<(usize, u8)> {
        let parts = header_parts(app);
        (parts.iter().enumerate())
            .find(|(_, p)| spans_text(&p.spans).contains(words))
            .map(|(i, p)| (i, p.keep))
    };
    // Nothing waits, the demo's clock at its own speed; the last ten
    // minutes, the cards, the researchers and the rate are no parts at
    // all (record 125: the header has four things).
    app.world.inbox.clear();
    app.world.max_researchers = 2;
    app.speed = 1;
    let to = app.world.agents[0].id;
    app.world.journal.push(Entry {
        tick: app.world.tick,
        event: Event::Sent {
            from: 1,
            to,
            kind: MsgKind::Inform,
            text: "x".into(),
            id: 1,
            re: None,
        },
    });
    for words in [
        "waiting for you",
        "10 min:",
        "cards",
        "researchers",
        "×",
        "/h",
    ] {
        assert_eq!(keeps(&app, words), None, "{words}");
    }
    app.speed = 2;
    assert_eq!(keeps(&app, "×2").map(|k| k.1), Some(3));
    waiting(&mut app, 1);
    // A reply asks for nothing: read, not decided, and the first to
    // go on a narrow screen after the projects.
    assert_eq!(keeps(&app, "to decide"), None);
    assert_eq!(keeps(&app, " 1 to read "), Some((1, 7)));
    app.world.inbox[0].kind = InboxKind::Question;
    assert_eq!(keeps(&app, "to read"), None);
    // The projects first, then what is to decide.
    let name = app.world.projects[0].name.clone();
    assert_eq!(keeps(&app, &name), Some((0, 8)), "the projects");
    assert_eq!(keeps(&app, " ! 1 to decide "), Some((1, KEEP)));
    waiting(&mut app, 1);
    assert_eq!(keeps(&app, " ! 1 to decide "), Some((1, KEEP)));
    assert_eq!(keeps(&app, " 1 to read "), Some((2, 7)));
    app.world.inbox.pop();
    assert_eq!(keeps(&app, " of $").map(|k| k.1), Some(KEEP));
    let window = |used| Limit {
        minutes: 300,
        used,
        resets_at: None,
    };
    // A window short of loud is the `:budget` note's, not the header's.
    app.world.limits = vec![window(LIMIT_LOUD - 1)];
    assert_eq!(keeps(&app, "claude"), None);
    app.world.limits = vec![window(LIMIT_LOUD)];
    assert_eq!(keeps(&app, "claude").map(|k| k.1), Some(KEEP));
    // A pot running low stays, with how long it lasts.
    for a in &mut app.world.agents {
        a.cost = 0;
    }
    app.world.tick = 1;
    app.world.agents[0].cost = 100;
    app.world.watch_spending();
    app.world.budget = 100 + 299;
    assert!(app.world.pot_low());
    assert_eq!(keeps(&app, " of $").map(|k| k.1), Some(KEEP));
    assert_eq!(keeps(&app, "left").map(|k| k.1), Some(KEEP));
    assert_eq!(keeps(&app, "/h"), None);
    // On a narrow screen the projects go first; what waits and the
    // pot are still there, and the brand goes before they would be
    // cut (record 131).
    let s = render(&app, 70, 20);
    let top = s.lines().next().unwrap();
    assert!(top.starts_with("  ! 1 to decide"), "{top}");
    assert!(top.contains(" of $") && top.contains("10% left"), "{top}");
    assert!(!top.contains(&name) && !top.contains("imece"), "{top}");
    let s = render(&app, 90, 20);
    let top = s.lines().next().unwrap();
    assert!(
        top.starts_with(" imece  ") && top.contains("! 1 to decide"),
        "{top}"
    );
    // Spent, a cent over or to the last of it: said, and kept.
    assert_eq!(keeps(&app, "pot spent"), None);
    for over in [0, 1] {
        app.world.budget = app.world.spent() - over;
        assert_eq!(keeps(&app, "pot spent").map(|k| k.1), Some(KEEP), "{over}");
    }
    app.world.budget = app.world.spent() + 1;
    assert_eq!(keeps(&app, "pot spent"), None);
}

#[test]
fn the_projects_stand_in_the_header_and_the_one_home_is_narrowed_to_is_lit() {
    let text = |app: &App| -> String {
        projects_spans(app)
            .iter()
            .map(|s| s.content.as_ref())
            .collect()
    };
    let mut app = App::new(2, 2, 0, 1, Lang::En);
    let names: Vec<String> = app.world.projects.iter().map(|p| p.name.clone()).collect();
    assert_eq!(text(&app), format!("{} · {}", names[0], names[1]));
    let lit = |app: &App, n: usize| {
        projects_spans(app)[n * 2]
            .style
            .add_modifier
            .contains(Modifier::REVERSED)
    };
    assert!(!lit(&app, 0) && !lit(&app, 1));
    app.at_home.scope = Some(1);
    assert!(!lit(&app, 0) && lit(&app, 1));
    // A closed project is not named.
    app.world.projects[0].closed = true;
    assert_eq!(text(&app), names[1]);
    assert_eq!(projects_spans(&app)[0].content, names[1]);
}

#[test]
fn the_header_says_the_pot_and_how_long_it_lasts_only_once_it_runs_low() {
    let mut app = app_after(1, 2, 0, Lang::En);
    for a in &mut app.world.agents {
        a.cost = 0;
    }
    app.world.run_spent = 0;
    app.world.budget = 10_000;
    let head = |app: &App| render(app, 160, 40).lines().next().unwrap().to_string();
    let at = |app: &App, word: &str| {
        let row = head(app);
        let x = row.chars().take(row.find(word).unwrap()).count() as u16;
        let c = buffer(app, 160, 40)[(x, 0)].clone();
        (c.fg, c.modifier)
    };
    let looks = |s: Style| (s.fg.unwrap_or(Color::Reset), s.add_modifier);
    assert_ne!(looks(attention()), looks(Style::default()));
    let row = head(&app);
    assert!(row.contains("$0.00 of $1.00  "), "{row}");
    assert!(!row.contains("spent"), "{row}");
    assert!(!row.contains("/h") && !row.contains("left"), "{row}");
    app.world.tick = 1;
    app.world.agents[0].cost = 100;
    app.world.watch_spending();
    let row = head(&app);
    assert!(row.contains("$0.01 of $1.00  "), "{row}");
    assert!(
        !row.contains("left"),
        "the rate and the time left are :budget's: {row}"
    );
    assert_eq!(at(&app, "$0.01"), looks(Style::default()));
    app.world.budget = 100 + 299;
    let row = head(&app);
    assert!(row.contains("$0.01 of $0.03, ~29 min left"), "{row}");
    assert_eq!(at(&app, "$0.01"), looks(attention()));
    assert_eq!(at(&app, "left"), looks(attention()));
    app.lang = Lang::Tr;
    let row = head(&app);
    assert!(row.contains("$0.01 / $0.03, ~29 dk kaldı"), "{row}");
}

#[test]
fn the_header_shows_what_is_spent_of_the_budget_and_the_footer_no_longer_names_the_views() {
    let mut app = app_after(1, 2, 0, Lang::En);
    app.world.budget = 10_000;
    for a in &mut app.world.agents {
        a.cost = 0;
    }
    app.world.agents[0].cost = 2_500;
    app.world.agents[1].cost = 2_500;
    let screen = render(&app, 170, 40);
    assert!(screen.contains("imece"));
    assert!(screen.contains("│ $0.50 of $1.00"), "{screen}");
    // On a plan, with a turn never priced: what it spent at least, its
    // API-equivalent (record 132); the budget is the person's own.
    app.world.estimated = true;
    app.world.agents[1].unpriced.gaps = 1;
    let screen = render(&app, 170, 40);
    assert!(screen.contains("│ ≈$0.50+? of $1.00"), "{screen}");
    // The footer names no view and no pause: the status line says where
    // you are, the header says PAUSED.
    let footer = screen.lines().last().unwrap().to_string();
    assert!(
        !footer.contains("space") && !footer.contains("1 salon"),
        "{footer}"
    );
}

#[test]
fn the_header_shows_the_tightest_window_loud_when_nearly_full() {
    let mut app = App::new(1, 1, 0, 1, Lang::En);
    let at = |minutes, used| Limit {
        minutes,
        used,
        resets_at: None,
    };
    app.world.limits = vec![at(300, 52), at(10_080, 90)];
    let screen = render(&app, 200, 20);
    let top = screen.lines().next().unwrap();
    assert!(top.contains(" │ claude 7d: 10% left"), "{top}");
    assert!(!top.contains("claude 5h"), "{top}");
    // Short of loud, no window is in the header at all.
    app.world.limits = vec![at(300, 52), at(10_080, 89)];
    let top = render(&app, 200, 20).lines().next().unwrap().to_string();
    assert!(!top.contains("claude"), "{top}");
    let span = |used| limit_span(Lang::En, &at(300, used), 0);
    assert_eq!(span(89).style, Style::new());
    assert_eq!(span(90).style, attention());
    assert_eq!(span(90).content, "claude 5h: 10% left");
}
