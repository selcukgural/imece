use super::*;

#[test]
fn the_salon_order_puts_the_pinned_open_projects_first() {
    let mut w = app_after(3, 3, 0, Lang::En).world;
    assert_eq!(w.salon_order(), [0, 1, 2]);
    w.projects[2].pinned = true;
    assert_eq!(w.salon_order(), [2, 0, 1]);
    w.projects[0].pinned = true;
    assert_eq!(w.salon_order(), [0, 2, 1], "each group as opened");
    w.projects[2].closed = true;
    assert_eq!(w.salon_order(), [0, 1], "a closed one is not shown");
}

#[test]
fn colon_pin_pins_and_unpins_the_project_at_hand_or_named() {
    let mut app = app_after(2, 2, 0, Lang::En);
    let (one, two) = (
        app.world.projects[0].name.clone(),
        app.world.projects[1].name.clone(),
    );
    app.selected = Sel::Project(0);
    let typed = |app: &mut App, line: &str| {
        for c in line.chars() {
            key(app, KeyCode::Char(c));
        }
    };
    typed(&mut app, ":pi");
    assert_eq!(app.menu.as_ref().unwrap().notes, [format!("({one})")]);
    typed(&mut app, "n");
    key(&mut app, KeyCode::Enter);
    assert!(app.world.projects[0].pinned);
    assert!(render(&app, 140, 40).contains(&format!("{one} is pinned to the top of the team")));
    typed(&mut app, ":pi");
    assert_eq!(app.menu.as_ref().unwrap().notes, [format!("(▴ {one})")]);
    key(&mut app, KeyCode::Esc);
    key(&mut app, KeyCode::Esc);
    typed(&mut app, ":pin");
    key(&mut app, KeyCode::Enter);
    assert!(!app.world.projects[0].pinned);
    assert!(render(&app, 140, 40).contains(&format!("{one} is not pinned any more")));
    typed(&mut app, &format!(":pin {two}"));
    key(&mut app, KeyCode::Enter);
    assert!(app.world.projects[1].pinned && !app.world.projects[0].pinned);
    typed(&mut app, ":pin nowhere");
    key(&mut app, KeyCode::Enter);
    assert!(render(&app, 140, 40).contains("no open project named nowhere"));
    assert!(app.world.projects[1].pinned, "nothing else changed");
}

#[test]
fn archiving_puts_a_project_away_after_its_grace_and_a_fresh_one_opens() {
    let typed = |app: &mut App, line: &str| {
        for c in line.chars() {
            key(app, KeyCode::Char(c));
        }
        app.menu = None;
        key(app, KeyCode::Enter);
    };
    let flashed = |app: &App| app.flash.as_ref().map(|f| f.0.clone()).unwrap_or_default();
    for (lang, grace, cut, undone, done, line) in [
        (
            Lang::En,
            "◷ archiving shop as “first epic” in 10 s · u keeps it",
            "◷ archiving shop as “first epic” in 10 s, cutting 1 turn short · u keeps it",
            "shop stays as it is",
            "archived shop as “first epic”: its agents sleep with all they had; a fresh \
             one opens",
            "you archived shop as “first epic”; a fresh shop takes its place",
        ),
        (
            Lang::Tr,
            "◷ shop “first epic” adıyla arşivleniyor, 10 sn sonra · u tutar",
            "◷ shop “first epic” adıyla arşivleniyor, 10 sn sonra; 1 tur yarıda kesilecek · \
             u tutar",
            "shop olduğu gibi kalıyor",
            "shop “first epic” adıyla arşivlendi: agent'ları her şeyiyle uyuyor; yerine \
             yenisi açıldı",
            "shop projesini “first epic” adıyla arşivledin; yerine yeni bir shop açıldı",
        ),
    ] {
        let mut app = app_after(1, 1, 0, lang);
        app.home = None;
        typed(&mut app, ":open ~/work/shop");
        let ours: Vec<AgentId> = (app.world.agents.iter())
            .filter(|a| a.project == 1)
            .map(|a| a.id)
            .collect();
        let worker = ours[1];
        let t = app.world.tick;
        for &id in &ours {
            app.world.agents[id].set_status(Status::Idle, t);
        }
        // Another project's turns are not this archive's to cut.
        for id in 0..ours[0] {
            app.world.agents[id].set_status(Status::Working, t);
        }
        app.at_home.scope = Some(1);
        typed(&mut app, ":archive first epic");
        assert_eq!(app.grace.map(|g| g.0), Some(Undoable::Archive(1)));
        assert_eq!(app.archive_title, "first epic");
        assert!(render(&app, 160, 40).contains(grace), "{lang:?}");
        app.world.agents[worker].set_status(Status::Working, t);
        assert!(render(&app, 160, 40).contains(cut), "{lang:?}");
        key(&mut app, KeyCode::Char('u'));
        assert!(app.grace.is_none());
        assert_eq!(flashed(&app), undone);
        assert!(app.world.projects[1].archived.is_none());

        app.world.agents[worker].set_status(Status::Idle, t);
        app.unseen.insert(worker);
        app.settled.insert(worker);
        // Another project's stay.
        app.unseen.insert(0);
        app.settled.insert(0);
        app.zoom = Zoom::Focus(worker);
        app.inbox_sel = 5;
        app.card = app.world.cards.iter().position(|c| c.project == 1);
        assert!(app.card.is_some());
        let theirs = |app: &App| {
            (journal_rows(&app.world).iter())
                .filter(|&&(i, _)| {
                    app.world.agents[app.world.journal[i].event.agent()].project == 1
                })
                .count()
        };
        assert!(theirs(&app) > 0, "its lines are in sight before");
        typed(&mut app, ":archive first epic");
        app.end_grace();
        let w = &app.world;
        assert_eq!(
            w.projects[1].archived.as_ref().map(|a| a.title.as_str()),
            Some("first epic")
        );
        assert_eq!(w.projects.len(), 3);
        assert_eq!(
            (w.projects[2].name.as_str(), w.projects[2].closed),
            ("shop", false)
        );
        assert_eq!(app.selected, Sel::Agent(w.projects[2].leader));
        assert_eq!((app.zoom, app.card), (Zoom::None, None));
        assert_eq!(app.inbox_sel, app.inbox_rows().len().saturating_sub(1));
        assert_eq!(app.collapsed.len(), 3);
        assert!(!app.unseen.contains(&worker) && !app.settled.contains(&worker));
        assert!(app.unseen.contains(&0) && app.settled.contains(&0));
        assert_eq!(flashed(&app), done);
        assert_eq!(theirs(&app), 0, "its lines wait in the archive");
        assert!(w.shown_cards().all(|c| c.project != 1));
        app.open_window(Window::Journal);
        assert!(render(&app, 300, 40).contains(line), "{lang:?}");
        app.close_window();
        // Its agents sleep: the demo's steps leave them be.
        let before: Vec<(u8, u64)> = (ours.iter())
            .map(|&id| (app.world.agents[id].progress, app.world.agents[id].cost))
            .collect();
        for _ in 0..30 {
            app.step();
        }
        let after: Vec<(u8, u64)> = (ours.iter())
            .map(|&id| (app.world.agents[id].progress, app.world.agents[id].cost))
            .collect();
        assert_eq!(before, after);
        assert!(
            ours.iter()
                .all(|&id| app.world.agents[id].status == Status::Asleep)
        );
        // A selected header moves to the fresh head too.
        app.at_home.scope = Some(2);
        typed(&mut app, ":archive again");
        app.end_grace();
        assert!(app.world.projects[2].archived.is_some());
        assert_eq!(app.selected, Sel::Agent(app.world.projects[3].leader));
        // Without a title: its name and days; a long one is cut.
        typed(&mut app, ":archive");
        assert_eq!(app.archive_title, app.world.archive_title(3));
        typed(&mut app, &format!(":archive {}", "x".repeat(70)));
        assert_eq!(app.archive_title, "x".repeat(60));
    }
}

#[test]
fn restore_brings_an_archive_back_selects_its_head_and_says_what_was_missing() {
    let typed = |app: &mut App, line: &str| {
        for c in line.chars() {
            key(app, KeyCode::Char(c));
        }
        app.menu = None;
        key(app, KeyCode::Enter);
    };
    let flashed = |app: &App| app.flash.as_ref().map(|f| f.0.clone()).unwrap_or_default();
    for (lang, done, line, none, none_of) in [
        (
            Lang::En,
            "brought shop “first epic” back: its agents sleep until you write to them",
            "you brought back shop “first epic”",
            "no archive is titled or named nothing",
            "shop has no archive",
        ),
        (
            Lang::Tr,
            "shop “first epic” geri geldi: agent'ları sen yazana kadar uyuyor",
            "shop projesinin “first epic” arşivini geri getirdin",
            "nothing başlıklı ya da adlı arşiv yok",
            "shop projesinin arşivi yok",
        ),
    ] {
        let mut app = app_after(1, 1, 0, lang);
        app.home = None;
        typed(&mut app, ":open ~/work/shop");
        app.at_home.scope = Some(1);
        typed(&mut app, ":restore");
        assert_eq!(flashed(&app), none_of, "{lang:?}");
        typed(&mut app, ":restore nothing");
        assert_eq!(flashed(&app), none, "{lang:?}");
        typed(&mut app, ":archive first epic");
        app.end_grace();
        let fresh = 2;
        let head = app.world.projects[fresh].leader;
        app.zoom = Zoom::Focus(head);
        app.unseen.insert(head);
        // An `:archive` waiting acts first.
        typed(&mut app, ":archive x");
        assert!(app.grace.is_some());
        typed(&mut app, ":restore first epic");
        assert!(app.grace.is_none());
        let w = &app.world;
        assert!(
            w.projects[fresh].archived.is_some(),
            "the `:archive x` acted"
        );
        // `x` took 2's place as 3; it did nothing, so it is closed.
        assert!(w.projects[3].closed && w.projects[3].archived.is_none());
        assert!(!w.projects[1].closed && w.projects[1].archived.is_none());
        assert_eq!(app.selected, Sel::Agent(w.projects[1].leader));
        assert_eq!(flashed(&app), done, "{lang:?}");
        app.open_window(Window::Journal);
        assert!(render(&app, 300, 40).contains(line), "{lang:?}");
        app.close_window();
        // What gave way leaves the screen: its focus and its unseen.
        assert_eq!(app.zoom, Zoom::None);
        assert!(!app.unseen.contains(&head));
        // A bare `:restore` brings back the newest of the name at hand.
        typed(&mut app, ":restore");
        assert!(app.world.projects[fresh].archived.is_none());
        assert!(app.world.projects[1].archived.is_some(), "it was swapped");
        assert_eq!(app.selected, Sel::Agent(app.world.projects[fresh].leader));
        // A refusal is said: a solo has the name, the team is renamed.
        app.world.projects[fresh].name = "shop3".into();
        (app.world).open_project(None, "shop".into(), "/s".into(), Role::Solo);
        let row = app.at_home.row;
        typed(&mut app, ":restore shop");
        assert_eq!(flashed(&app), "a project named shop is open already");
        assert!(app.world.projects[1].archived.is_some());
        assert_eq!(app.at_home.row, row, "nothing moved");
    }
}

#[test]
fn an_archive_opens_read_only_under_its_banner_and_exports_what_it_shows() {
    let dir = scratch("archive-view");
    let typed = |app: &mut App, line: &str| {
        for c in line.chars() {
            key(app, KeyCode::Char(c));
        }
        if let Some(v) = app.viewing.as_mut() {
            v.menu = None;
        }
        key(app, KeyCode::Enter);
    };
    let mut app = app_after(2, 2, 120, Lang::En);
    app.selected = Sel::Agent(app.world.projects[1].leader);
    typed(&mut app, ":archive first epic");
    app.end_grace();
    // Something waits on the screen behind: the banner counts it.
    app.world.inbox.push(InboxItem {
        kind: InboxKind::Question,
        agent: app.world.projects[0].leader,
        detail: "x".into(),
        researcher: None,
        open: true,
        tick: app.world.tick,
        card: None,
    });
    // A reply is to read, not to decide: said apart.
    app.world.inbox.push(InboxItem {
        kind: InboxKind::Reply,
        agent: app.world.projects[0].leader,
        detail: "y".into(),
        researcher: None,
        open: true,
        tick: app.world.tick,
        card: None,
    });
    let waiting = app.world.open_decisions();
    assert_eq!(waiting + 1, app.world.open_inbox().len());
    let read = (app.world.open_inbox().into_iter())
        .filter(|&i| app.world.inbox[i].kind == InboxKind::Reply)
        .count();
    let read = match read {
        1 => "1 reply".to_string(),
        n => format!("{n} replies"),
    };
    key(&mut app, KeyCode::Char('A'));
    key(&mut app, KeyCode::Enter);
    let at = app.world.projects[1].archived.as_ref().unwrap().at;
    let (name, other) = (
        app.world.projects[1].name.clone(),
        app.world.projects[0].name.clone(),
    );
    let s = render(&app, 160, 40);
    let banner = format!(
        " ARCHIVE · {name} “first epic” · archived {} · {waiting} to decide, {read} to read now · esc back ",
        date(day(at))
    );
    let top = s.lines().next().unwrap();
    assert!(top.starts_with(&banner), "{s}");
    for (i, l) in s.lines().enumerate() {
        if l.contains(other.as_str()) {
            eprintln!("DBGL {i}: {l}");
        }
    }
    assert!(!s.contains(other.as_str()), "only the archive: {s}");
    // A conversation of it, exported.
    let v = app.viewing.as_mut().unwrap();
    let said = (v.world.journal.iter())
        .position(|e| words::said(&e.event).is_some())
        .expect("its agents talked");
    v.open_thread(said);
    assert!(v.thread.is_some());
    let file = dir.join("archived.md");
    typed(&mut app, &format!(":export {}", file.display()));
    let text = std::fs::read_to_string(&file).unwrap();
    assert!(text.starts_with("# Conversation: "), "{text}");
    // In Turkish.
    key(&mut app, KeyCode::Esc);
    key(&mut app, KeyCode::Char('L'));
    let s = render(&app, 160, 40);
    let banner = format!(
        " ARŞİV · {name} “first epic” · {} arşivlendi · şu an {waiting} karar bekliyor, 1 yanıt okunacak · esc geri ",
        date(day(at))
    );
    assert!(s.lines().next().unwrap().starts_with(&banner), "{s}");
    // Back: the screen behind as it was.
    key(&mut app, KeyCode::Esc);
    assert!(app.viewing.is_none());
    let s = render(&app, 160, 40);
    assert!(
        !s.contains("ARCHIVE ·") && s.contains(other.as_str()),
        "{s}"
    );
    std::fs::remove_dir_all(&dir).unwrap();
}

#[test]
fn a_live_checkout_is_written_with_a_tilde_for_the_home() {
    let mut w = crate::live::testing::team(1);
    let home = std::env::temp_dir().join("imece-archives-home");
    let repo = home.join("src").join("kule");
    let live = crate::live::testing::idle_live_in(&repo, 2);
    w.projects[0].closed = true;
    w.projects[0].archived = Some(Archived {
        at: 0,
        title: "epic".into(),
    });
    let mut app = App::with(w, crate::app::Engine::Live(Box::new(live)), Lang::En);
    app.home = Some(home.clone());
    key(&mut app, KeyCode::Char('A'));
    let checkout = |app: &App| app.archives.as_ref().unwrap().groups[0].checkout.clone();
    assert_eq!(checkout(&app), "~/src/kule");
    key(&mut app, KeyCode::Esc);
    // Not under the home: the whole path.
    app.home = Some(home.join("other"));
    key(&mut app, KeyCode::Char('A'));
    assert_eq!(checkout(&app), repo.display().to_string());
}

#[test]
fn restore_completes_the_archives_titles_newest_first_with_name_and_day() {
    let mut app = app_after(1, 1, 0, Lang::En);
    app.world.tick = 10;
    app.world.archive_project(0, "first epic".into());
    app.world.tick = 20;
    app.world.archive_project(1, "second".into());
    let day = crate::model::date(crate::model::day(20));
    let menu = |input: &str| {
        crate::complete::suggest(input, &app.world, 2, None).map(|m| (m.start, m.items, m.notes))
    };
    let name = app.world.projects[0].name.clone();
    let then = crate::model::date(crate::model::day(10));
    assert_eq!(
        menu(":restore "),
        Some((
            9,
            vec!["second".to_string(), "first epic".to_string()],
            vec![format!("{name} · {day}"), format!("{name} · {then}")]
        ))
    );
    assert_eq!(
        menu(":restore FI"),
        Some((
            9,
            vec!["first epic".to_string()],
            vec![format!("{name} · {then}")]
        ))
    );
    assert_eq!(menu(":restore first epic"), None, "typed whole");
    assert_eq!(menu(":restore z"), None);
}

#[test]
fn an_archive_s_cards_leave_the_board_the_queue_and_the_header() {
    let mut app = app_after(1, 2, 0, Lang::En);
    let name = app.world.projects[0].name.clone();
    let theirs: Vec<String> = (app.world.cards.iter()).map(|c| c.title.clone()).collect();
    assert!(theirs.len() >= 2, "the demo has cards");
    app.world.cards[0].column = Column::Done;
    app.world.cards[1].column = Column::Verifying;
    app.window = Some(Window::Board);
    let before = render(&app, 200, 40);
    assert!(before.contains(&theirs[0]));
    let fresh = app.world.archive_project(0, "x".into());
    app.collapsed.push(false);
    let lead = app.world.projects[fresh].leader;
    let c = app.world.cards.len();
    app.world.cards.push(Card {
        id: c,
        project: fresh,
        title: "the next epic".into(),
        owner: Some(lead),
        column: Column::Working,
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
    });
    let board = render(&app, 200, 40);
    for title in &theirs {
        assert!(!board.contains(title.as_str()), "{title}: {board}");
    }
    assert!(board.contains("the next epic"));
    // One project in sight: the cards do not name it.
    let row = (board.lines())
        .skip_while(|l| !l.contains("the next epic"))
        .nth(1)
        .unwrap();
    assert!(!row.contains(&name), "{row}");
    assert!(
        board.contains("IN PROGRESS (1)") && board.contains("DONE (0)"),
        "{board}"
    );
    app.world.cards.last_mut().unwrap().column = Column::Working;
    // Two projects in sight: each card names its own.
    app.world.projects[0].archived = None;
    app.window = Some(Window::Board);
    let board = render(&app, 200, 40);
    let row = (board.lines())
        .skip_while(|l| !l.contains("the next epic"))
        .nth(1)
        .unwrap();
    assert!(row.contains(&name), "{row}");
}

#[test]
fn without_a_merge_queue_there_is_no_trunk_or_lease() {
    let mut app = app_after(1, 4, 300, Lang::En);
    app.world.projects[0].trunk = None;
    app.world.projects[0].repo = &[];
    for c in &mut app.world.cards {
        c.symbols.clear();
    }
    app.world.max_researchers = 0;
    let salon = render(&app, 140, 40);
    assert!(!salon.contains("trunk #"), "{salon}");
    assert!(!salon.contains("researchers"), "{salon}");
    app.zoom = Zoom::Focus(1);
    assert!(!render(&app, 140, 40).contains("lease"));
    app.zoom = Zoom::None;
    app.window = Some(Window::Code);
    assert!(render(&app, 140, 40).contains("no shared trunk yet"));
    // Before any card exists there is no "0/0 cards" either.
    app.window = None;
    app.world.cards.clear();
    app.world.journal.clear();
    app.world.inbox.clear();
    app.world.plans.clear();
    for a in &mut app.world.agents {
        a.card = None;
    }
    let screen = render(&app, 140, 40);
    assert!(!screen.contains("cards"), "{screen}");
}

#[test]
fn colon_open_close_and_rename_manage_projects_from_the_screen() {
    use crate::app::Zoom;
    use crate::live::testing::{checkout, idle_live_in, team};
    let one = checkout("ui-one");
    let two = checkout("ui-two");
    let three = checkout("ui-three");
    let four = checkout("ui-four");
    let three_name = three.file_name().unwrap().to_str().unwrap().to_string();
    let mut app = App::with(
        team(1),
        crate::app::Engine::Live(Box::new(idle_live_in(&one, 2))),
        Lang::En,
    );
    app.home = Some(three.parent().unwrap().to_path_buf());
    let typed = |app: &mut App, line: &str| {
        for c in line.chars() {
            key(app, KeyCode::Char(c));
        }
        key(app, KeyCode::Enter);
    };
    let flashed = |app: &App| app.flash.as_ref().map(|f| f.0.clone()).unwrap_or_default();
    let journaled = |app: &App, needle: &str| {
        (app.world.journal.iter())
            .any(|e| words::event(Lang::En, &app.world, &e.event).contains(needle))
    };
    // The header names the open projects (record 125).
    let named = |app: &App| -> Vec<String> {
        let header = render(app, 140, 40).lines().next().unwrap().to_string();
        (app.world.open_projects())
            .filter(|p| header.contains(&p.name))
            .map(|p| p.name.clone())
            .collect()
    };

    typed(&mut app, ":open");
    assert!(
        flashed(&app).starts_with(":open PATH [NAME] opens"),
        "{}",
        flashed(&app)
    );
    typed(&mut app, &format!(":open {} api", two.display()));
    assert_eq!(app.world.projects.len(), 2);
    assert_eq!(app.collapsed, [false, false]);
    let lead_api = app.world.projects[1].leader;
    assert_eq!(app.selected, Sel::Agent(lead_api));
    assert_eq!(app.zoom, Zoom::None);
    let said = flashed(&app);
    assert!(
        said.starts_with("opened api at ") && said.ends_with("; write to lead-api"),
        "{said}"
    );
    let s = render(&app, 140, 40);
    assert!(s.contains("lead-api"), "{s}");
    assert!(journaled(&app, "you opened api at"));
    assert_eq!(named(&app).len(), 2, "both in the header");

    // The short form, `~`, the name from the directory.
    typed(&mut app, &format!(":o ~/{three_name}"));
    assert_eq!(app.world.projects.len(), 3);
    assert_eq!(app.world.projects[2].name, three_name);
    assert_eq!(
        app.world.agents[app.world.projects[2].leader].name,
        format!("lead-{three_name}")
    );
    assert_eq!(app.collapsed.len(), 3);

    // Refusals, each in its own words; nothing opened.
    typed(&mut app, ":open /nonexistent-dir");
    assert!(
        flashed(&app).starts_with("could not open: /nonexistent-dir"),
        "{}",
        flashed(&app)
    );
    typed(&mut app, &format!(":open {} other", two.display()));
    assert!(
        flashed(&app).ends_with(" is open already, as api"),
        "{}",
        flashed(&app)
    );
    typed(&mut app, &format!(":open {} api", four.display()));
    assert_eq!(flashed(&app), "a project named api is open already");
    typed(&mut app, &format!(":open {} a/b", four.display()));
    assert!(
        flashed(&app).starts_with("\"a/b\" can not name a project"),
        "{}",
        flashed(&app)
    );
    assert_eq!(app.world.projects.len(), 3);

    // Rename the one at hand (Home narrowed to it).
    app.at_home.scope = Some(1);
    typed(&mut app, ":rename shop");
    assert_eq!(app.world.projects[1].name, "shop");
    assert_eq!(app.world.agents[lead_api].name, "lead-shop");
    assert_eq!(flashed(&app), "api is now shop; write to lead-shop");
    assert!(journaled(&app, "you renamed api to shop"));
    typed(&mut app, &format!(":rename {three_name}"));
    assert_eq!(
        flashed(&app),
        format!("a project named {three_name} is open already")
    );
    typed(&mut app, ":rename a b");
    assert!(
        flashed(&app).starts_with("\"a b\" can not name a project"),
        "{}",
        flashed(&app)
    );
    assert_eq!(app.world.projects[1].name, "shop", "refused: unchanged");

    // Close by name: gone from the salon; a zoom on it ends, a
    // selection elsewhere stays, the inbox selection fits what is left.
    let lead3 = app.world.projects[2].leader;
    app.zoom = Zoom::Focus(lead_api);
    app.selected = Sel::Agent(lead3);
    app.inbox_sel = 5;
    typed(&mut app, ":close shop");
    app.end_grace();
    assert!(app.world.projects[1].closed);
    assert_eq!(app.zoom, Zoom::None);
    assert_eq!(app.selected, Sel::Agent(lead3), "not in the closed project");
    assert_eq!(app.inbox_sel, 0);
    assert_eq!(
        flashed(&app),
        "closed shop: its agents left; their worktrees stay on disk"
    );
    assert_eq!(named(&app).len(), 2, "the two left, in the header");
    assert!(journaled(&app, "you closed shop"));
    typed(&mut app, ":close nope");
    assert_eq!(flashed(&app), "no open project named nope");
    // Opened again in place: same id, the fold undone, its new lead selected.
    app.collapsed[1] = true;
    typed(&mut app, &format!(":open {}", two.display()));
    assert_eq!(app.world.projects.len(), 3);
    assert_eq!(app.collapsed, [false, false, false]);
    assert!(!app.world.projects[1].closed);
    assert_ne!(app.world.projects[1].leader, lead_api);
    assert_eq!(app.selected, Sel::Agent(app.world.projects[1].leader));
    typed(&mut app, ":close");
    app.end_grace();
    assert!(
        app.world.projects[1].closed,
        "the one at hand, by the selection: {} {:?}",
        flashed(&app),
        app.selected
    );
    // A selection (and a zoom) in the closed project moves to the first open one.
    app.selected = Sel::Project(0);
    app.zoom = Zoom::Trace(lead3);
    typed(&mut app, ":close repo");
    app.end_grace();
    assert!(app.world.projects[0].closed);
    assert_eq!(app.selected, Sel::Agent(lead3));
    assert_eq!(app.zoom, Zoom::Trace(lead3), "a zoom elsewhere stays");
    // The one at hand (`:c` alone would take the menu's `close`), the last one.
    typed(&mut app, ":c ");
    assert_eq!(
        flashed(&app),
        format!("{three_name} is the last open project; q q quits")
    );
    assert!(!app.world.projects[2].closed);
    assert_eq!(app.current_project(), 2);

    // Turkish wording of the journal, by the projects' current names.
    app.lang = Lang::Tr;
    let said: Vec<String> = app
        .world
        .journal
        .iter()
        .map(|e| words::event(Lang::Tr, &app.world, &e.event))
        .collect();
    let two_name = two.file_name().unwrap().to_str().unwrap();
    assert!(
        said.iter()
            .any(|s| s.starts_with(&format!("{two_name} projesini açtın ("))),
        "{said:?}"
    );
    assert!(
        said.contains(&"api projesine shop adını verdin".to_string()),
        "{said:?}"
    );
    assert!(
        said.contains(&format!("{two_name} projesini kapattın")),
        "{said:?}"
    );
    assert!(
        said.contains(&"repo projesini kapattın".to_string()),
        "{said:?}"
    );
    for dir in [&one, &two, &three, &four] {
        let _ = std::fs::remove_dir_all(dir);
    }
}

#[test]
fn a_folder_without_git_is_said_in_the_journal_and_on_its_header() {
    let mut app = App::new(1, 1, 0, 1, Lang::En);
    let lead = app.world.agents[0].name.clone();
    let e = Event::NoGit {
        leader: 0,
        path: "/n".into(),
    };
    for lang in [Lang::En, Lang::Tr] {
        assert_eq!(
            words::event(lang, &app.world, &e),
            fill(lang.text().ev_no_git, &[&lead, "/n"])
        );
    }
    assert!(
        words::event(Lang::En, &app.world, &e)
            .starts_with(&format!("{lead} works straight in /n: no git here"))
    );
    let s = render(&app, 140, 30);
    assert!(!s.contains("⌀ no git"), "{s}");
    app.world.projects[0].no_git = true;
    let s = render(&app, 140, 30);
    assert!(s.contains("⌀ no git"), "{s}");
    app.lang = Lang::Tr;
    let s = render(&app, 140, 30);
    assert!(s.contains("⌀ git yok"), "{s}");
}

#[test]
fn a_stores_line_on_the_project_says_how_it_stands() {
    let mut app = App::with(
        crate::live::testing::team(0),
        crate::app::Engine::Live(Box::new(crate::live::testing::idle_live(1))),
        Lang::En,
    );
    let s = render(&app, 160, 30);
    assert!(!s.contains("lonca"), "{s}");
    app.world.on_lonca(0, Some("demo".into()));
    assert!(render(&app, 160, 30).contains("lonca demo …"));
    let trunk = "693d62952cd8f7ce58b0de780cf5d998b2f5c1a6".to_string();
    let seen = |pending| StoreSeen::Up {
        trunk: trunk.clone(),
        pending,
        behind: Behind::By(0),
    };
    app.world.store_seen(0, "demo", seen(0));
    let s = render(&app, 160, 30);
    assert!(s.contains("lonca demo 693d629"), "{s}");
    assert!(!s.contains("pending"), "{s}");
    app.world.store_seen(0, "demo", seen(2));
    assert!(render(&app, 160, 30).contains("lonca demo 693d629 · 2 pending"));
    app.lang = Lang::Tr;
    assert!(render(&app, 160, 30).contains("lonca demo 693d629 · 2 bekliyor"));
    app.world
        .store_seen(0, "demo", StoreSeen::Down(StoreDown::NoDaemon));
    assert!(render(&app, 160, 30).contains("⌀ lonca yok"));
    app.lang = Lang::En;
    assert!(render(&app, 160, 30).contains("⌀ no lonca"));
}

#[test]
fn a_stores_events_are_worded_in_both_languages_with_their_reason() {
    let app = App::with(
        crate::live::testing::team(0),
        crate::app::Engine::Live(Box::new(crate::live::testing::idle_live(1))),
        Lang::En,
    );
    let project = app.world.projects[0].name.clone();
    let all = [
        (StoreDown::NoDaemon, "lonca daemon"),
        (StoreDown::NotTheDaemon, "not your daemon"),
        (StoreDown::Protocol, "another protocol"),
        (StoreDown::Old, "brew upgrade lonca"),
        (StoreDown::Hardened, "hardened"),
        (StoreDown::SignedOut, ":lonca login"),
        (StoreDown::Forbidden, "not enrolled"),
        (StoreDown::NoRepo, "no such repository"),
        (StoreDown::Refused, "refused"),
        (StoreDown::Busy, "busy"),
        (StoreDown::Frozen, "froze"),
    ];
    for (why, word) in all {
        let e = Event::StoreDown {
            leader: 0,
            repo: "demo".into(),
            why,
        };
        for lang in [Lang::En, Lang::Tr] {
            let t = lang.text();
            assert_eq!(
                words::event(lang, &app.world, &e),
                fill(
                    t.ev_store_down,
                    &[&project, "demo", words::store_down(t, why)]
                )
            );
        }
        assert!(words::store_down(Lang::En.text(), why).contains(word));
    }
    let back = Event::StoreBack {
        leader: 0,
        repo: "demo".into(),
    };
    assert_eq!(
        words::event(Lang::En, &app.world, &back),
        format!("{project}'s Lonca repository demo answers again")
    );
}

#[test]
fn how_far_behind_trunk_is_a_word_on_the_project_line() {
    use crate::model::{Behind, StoreSeen};
    assert_eq!(words::behind(Behind::By(3)).as_deref(), Some("↓3"));
    assert_eq!(words::behind(Behind::Never).as_deref(), Some("↓?"));
    assert_eq!(words::behind(Behind::By(0)), None);
    assert_eq!(words::behind(Behind::Unknown), None);
    let mut app = app_after(1, 1, 0, Lang::En);
    app.world.on_lonca(0, Some("shop".into()));
    let up = StoreSeen::Up {
        trunk: "693d62952cd8".into(),
        pending: 2,
        behind: Behind::By(3),
    };
    app.world.store_seen(0, "shop", up);
    let s = render(&app, 160, 40);
    assert!(s.contains("lonca shop 693d629 ↓3 · 2 pending"), "{s}");
}

#[test]
fn a_sync_says_how_the_checkout_stood() {
    use crate::model::{StoreAct, Synced};
    let app = app_after(1, 1, 0, Lang::En);
    let project = app.world.projects[0].name.clone();
    let line = |how| {
        let e = Event::StoreSynced {
            leader: 0,
            repo: "shop".into(),
            how,
        };
        words::event(Lang::En, &app.world, &e)
    };
    assert_eq!(
        line(Synced::Moved),
        format!("{project} moved forward to trunk of shop")
    );
    assert_eq!(
        line(Synced::Even),
        format!("{project} is level with trunk of shop")
    );
    assert_eq!(
        line(Synced::Apart),
        format!(
            "{project} has commits of its own: trunk of shop is in refs/lonca/trunk, nothing moved"
        )
    );
    let failed = Event::StoreFailed {
        leader: 0,
        act: StoreAct::Sync,
        why: Some("redacted".into()),
    };
    assert_eq!(
        words::event(Lang::En, &app.world, &failed),
        "bringing trunk to the checkout failed: redacted"
    );
}

#[test]
fn a_lonca_commands_lines_say_how_it_went() {
    use crate::model::StoreAct;
    let app = app_after(1, 1, 0, Lang::En);
    let project = app.world.projects[0].name.clone();
    let line = |e: Event| words::event(Lang::En, &app.world, &e);
    let signed = |human: &str| Event::StoreSignedIn {
        leader: 0,
        human: human.into(),
    };
    assert_eq!(line(signed("")), "signed in to Lonca");
    assert_eq!(line(signed("ayse")), "signed in to Lonca as ayse");
    let imported = |warnings: Vec<String>, hardened| Event::StoreImported {
        leader: 0,
        repo: "demo".into(),
        warnings,
        hardened,
    };
    let went = format!("{project} went into Lonca as repository demo");
    assert_eq!(line(imported(vec![], false)), went);
    assert_eq!(
        line(imported(
            vec!["big.bin".into(), "x".into(), "y".into()],
            false
        )),
        format!("{went}; Lonca warns: big.bin (+2 more)")
    );
    let hard =
        format!("{went}, from Lonca's hardened profile (lonca doctor says what it asks for)");
    assert_eq!(line(imported(vec![], true)), hard);
    assert_eq!(
        line(imported(vec!["big.bin".into()], true)),
        format!("{hard}; Lonca warns: big.bin (+0 more)")
    );
    let verified = |checks: Vec<Attested>| Event::Verified {
        agent: 1,
        ok: false,
        checks,
    };
    let check = |name: &str, ok, end: &str, lines: &[&str]| Attested {
        check: name.into(),
        ok,
        end: end.into(),
        lines: lines.iter().map(|l| l.to_string()).collect(),
    };
    assert!(
        line(verified(vec![
            check("syntax", true, "", &[]),
            check("build", true, "exit 0, 1.1 s", &[]),
            check("tests", false, "exit 1, 2.0 s", &["first", "last"]),
        ]))
        .ends_with(
            ": Lonca's checks · ✓ syntax · ✓ build exit 0, 1.1 s · \
             ✗ tests exit 1, 2.0 s · last"
        )
    );
    let kept = |why: Option<&str>| Event::StoreCriticalKept {
        leader: 0,
        repo: "demo".into(),
        why: why.map(str::to_string),
    };
    let stays = format!("{project} on demo: a worker's change to a critical path stays refused");
    assert_eq!(line(kept(None)), stays);
    assert_eq!(line(kept(Some("denied"))), format!("{stays}: denied"));
    let failed = |act, why: Option<&str>| Event::StoreFailed {
        leader: 0,
        act,
        why: why.map(str::to_string),
    };
    assert_eq!(
        line(failed(StoreAct::Login, Some("no key"))),
        "signing in to Lonca failed: no key"
    );
    assert_eq!(
        line(failed(StoreAct::Import, None)),
        "importing into Lonca failed: lonca did not answer: no such program, or it took too long"
    );
    // A refusal's reasons as a person says them (record 127); words
    // that are not one stay as they came.
    let yes = |why| line(failed(StoreAct::Decide, Some(why)));
    assert_eq!(
        yes("denied: unknown_check:api, invalid_checks"),
        format!(
            "{}: Lonca's policy has no api check, the card's checks are not a list \
             Lonca takes · :checks NAMES changes what the card asks for",
            Lang::En.text().store_act_decide
        )
    );
    // Each refusal of the card's checks says how to change them, once;
    // one for checks given with no change does not.
    for why in ["denied: unknown_check:api", "denied: invalid_checks"] {
        assert!(
            yes(why).ends_with(" · :checks NAMES changes what the card asks for"),
            "{why}"
        );
    }
    assert_eq!(
        yes("denied: checks_not_a_change"),
        format!(
            "{}: checks go only with a proposed change",
            Lang::En.text().store_act_decide
        )
    );
    assert!(!yes("denied: unknown_checker").contains(":checks"));
    assert_eq!(
        yes("denied:unknown_check:api"),
        format!(
            "{}: denied:unknown_check:api",
            Lang::En.text().store_act_decide
        )
    );
    assert_eq!(
        yes("no answer"),
        format!("{}: no answer", Lang::En.text().store_act_decide)
    );
}

#[test]
fn a_commit_is_read_by_its_first_digits() {
    assert_eq!(words::short_commit("693d62952cd8"), "693d629");
    assert_eq!(words::short_commit("693d629"), "693d629");
    assert_eq!(words::short_commit("69"), "69");
}
