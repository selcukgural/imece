use super::*;

/// A demo team after a while, card #1's screen opened by typing it.
fn on_card_one(lang: Lang) -> App {
    let mut app = App::new(1, 4, 4, 2, lang);
    for _ in 0..140 {
        app.step();
    }
    app.open_card(0);
    assert_eq!(app.card, Some(0));
    app
}

#[test]
fn a_card_s_screen_is_its_owner_s_detail_under_the_frame() {
    let app = on_card_one(Lang::En);
    let owner = app.world.cards[0].owner.unwrap();
    assert_eq!((app.pane, app.zoom), (Pane::Salon, Zoom::Focus(owner)));
    assert_eq!(app.detail(), Some((owner, crate::app::Tab::Log)));
    assert_eq!(
        app.detail_card(owner),
        Some(0),
        "the card, not the owner's next"
    );
    // The card's block has the keys' colour and no title of its own:
    // the detail's head names the card.
    let buf = buffer(&app, 140, 36);
    let s = render(&app, 140, 36);
    let y = s.lines().position(|l| l.starts_with('┌')).unwrap();
    assert!(y > 0, "under the head: {s}");
    assert_eq!(buf[(0, y as u16)].style().fg, Some(pal().accent));
    assert!(s.lines().nth(y).unwrap().starts_with("┌───"), "{s}");
}

#[test]
fn a_card_s_screen_shows_where_it_stands_who_had_a_part_and_its_lines() {
    let app = on_card_one(Lang::En);
    let s = render(&app, 140, 38);
    let card = &app.world.cards[0];
    assert!(!card.symbols.is_empty() && card.column == Column::Review);
    let owner = &app.world.agents[card.owner.unwrap()].name;
    let owns = format!("owns   {}", crate::paths::listed(&card.paths));
    assert!(!card.paths.is_empty());
    for want in [
        &format!("#1 {}", card.title),
        "shop-api · TO REVIEW · ",
        " min",
        "editing  ",
        &owns,
        "WHO",
        "posted    ! ▎lead ",
        "owner     ",
        "talked    ",
        "STORY",
        &format!("◎ intent     {owner}: {}", card.title),
        "n p  next / previous card",
    ] {
        assert!(s.contains(want), "{want} missing:\n{s}");
    }
    assert!(
        !s.contains("TEAM") && !s.contains("TO DECIDE · #1") && !s.contains("JOURNAL · #1"),
        "the card is the detail's body, no panes beside it:\n{s}"
    );
    // Its change waits for the person: `!`, whatever it does meanwhile.
    assert!(s.contains(&format!("owner     ! ▎{owner}")), "{s}");
    // Who had a part are the lines to move among.
    let shown = app.shown.borrow().clone();
    let people: Vec<Sel> = app
        .world
        .card_people(0)
        .iter()
        .map(|&(id, _)| Sel::Agent(id))
        .collect();
    assert_eq!(shown, people);
}

#[test]
fn a_card_s_row_keeps_its_cost_in_the_pane_and_quotes_only_what_is_left() {
    let mut app = on_card_one(Lang::En);
    let owner = app.world.cards[0].owner.unwrap();
    let name = app.world.agents[owner].name.clone();
    let a = &mut app.world.agents[owner];
    a.status = Status::Idle;
    a.activity = None;
    a.cost = 12_345;
    a.changed_at = app.world.tick;
    a.stand = Stand::default();
    a.local = None;
    a.plan.clear();
    a.context = None;
    a.talk = Some(Talk {
        with: Party::Person,
        mine: true,
        text: "a".repeat(60),
    });
    app.world.inbox.clear();
    app.unseen.clear();
    let s = render(&app, 140, 36);
    let row = s
        .lines()
        .find(|l| l.contains(&format!("▎{name}")))
        .unwrap_or_else(|| panic!("{s}"));
    let pane = row.split("││").next().unwrap();
    assert!(
        pane.contains("→ you \"aaaa") && pane.contains("…\"") && pane.contains("$1.23"),
        "the quote is cut to the row inside the card's borders, after the part column: {pane}"
    );
}

#[test]
fn export_writes_a_conversation_as_markdown_with_the_day_on_every_time() {
    let mut app = talking();
    app.world.note(Event::YouSaid {
        to: 0,
        text: "my TOKEN=abc123 \x1b[31m".into(),
    });
    let lead = app.world.agents[0].name.clone();
    let text = export::thread(Lang::En, &app.world, &[0, 1, 5]);
    assert!(
        text.starts_with(&format!("# Conversation: {lead} ⇄ you\n\n"))
            || text.starts_with(&format!("# Conversation: you ⇄ {lead}\n\n")),
        "{text}"
    );
    let first = format!("### {} you → {lead}\n\ncheck the readme\n\n", stamp(0));
    assert!(text.contains(&first), "{text}");
    assert!(text.contains(&format!("### {} {lead} → you\n\n# Readme\n", stamp(0))));
    assert!(text.contains("```\ncargo run\n```\n"), "{text}");
    assert!(!text.contains("abc123") && !text.contains('\x1b'), "{text}");
    // A kind is named; a line that is no message is a list item.
    app.world.note(Event::Sent {
        from: 0,
        to: 1,
        kind: MsgKind::Ask,
        text: "which?".into(),
        id: 0,
        re: None,
    });
    app.world.note(Event::Budget {
        leader: 0,
        from: 10_000,
        to: 20_000,
    });
    let worker = app.world.agents[1].name.clone();
    let text = export::thread(Lang::En, &app.world, &[6]);
    assert!(
        text.contains(&format!("{lead} → {worker} [ask]\n\nwhich?")),
        "{text}"
    );
    let text = export::thread(Lang::En, &app.world, &[7]);
    assert_eq!(
        text,
        format!(
            "# Conversation: \n\n- {} you set the budget to $2.00 (was $1.00)\n",
            stamp(0)
        )
    );
    // List items follow each other; a message stands apart.
    let text = export::thread(Lang::En, &app.world, &[7, 7, 6, 7]);
    let budget = format!("- {} you set the budget to $2.00 (was $1.00)\n", stamp(0));
    let ask = format!("### {} {lead} → {worker} [ask]\n\nwhich?\n", stamp(0));
    assert_eq!(
        text,
        format!("# Conversation: {lead} ⇄ {worker}\n\n{budget}{budget}\n{ask}\n{budget}")
    );
    let tr = export::thread(Lang::Tr, &app.world, &[0]);
    assert!(
        tr.starts_with("# Konuşma: ") && tr.contains(" sen → "),
        "{tr}"
    );
}

#[test]
fn export_writes_a_cards_screen_as_markdown() {
    let app = on_card_one(Lang::En);
    let w = &app.world;
    let card = &w.cards[0];
    let owner = &w.agents[card.owner.unwrap()].name;
    let text = export::card(&app, 0);
    let minutes = card::minutes(w, card);
    for want in [
        format!("# #1 {}\n\n", card.title),
        format!("shop-api · TO REVIEW · {minutes} min\n\n"),
        "editing  ".to_string(),
        "## WHO\n\n- posted: ".to_string(),
        format!("- owner: {owner}\n"),
        "- talked: ".to_string(),
        "\n## STORY\n\n- ".to_string(),
        format!("{owner}: {}", card.title),
        "\n## To decide\n\n- ".to_string(),
        format!("{owner} finished: {}", card.title),
        format!("- 14:00 ◎ intent {owner}: {}\n", card.title),
        "\n## Journal\n\n".to_string(),
    ] {
        assert!(text.contains(&want), "{want:?} missing:\n{text}");
    }
    // Every journal line of the card, oldest first, none folded.
    let lines = salon::card_lines(w, 0);
    assert!(!lines.is_empty() && lines.len() < w.journal.len());
    assert!(lines.windows(2).all(|p| p[0] < p[1]), "{lines:?}");
    let folded: Vec<usize> = salon::card_rows(w, 0).iter().map(|r| r.0).collect();
    assert!(folded.iter().all(|i| lines.contains(i)));
    let journal = &text[text.find("## Journal").unwrap()..];
    let at: Vec<usize> = (lines.iter())
        .map(|&i| journal.find(&stamp(w.journal[i].tick)).unwrap())
        .collect();
    assert!(
        at.windows(2).all(|p| p[0] <= p[1]),
        "oldest first:\n{journal}"
    );
    // No story, nothing waiting, no lines: said or left out.
    let mut app = on_card_one(Lang::Tr);
    app.world.cards[0].story.clear();
    app.world.cards[0].symbols.clear();
    app.world.inbox.iter_mut().for_each(|i| i.open = false);
    app.world.journal.clear();
    let text = export::card(&app, 0);
    assert!(text.contains("## KİM\n"), "{text}");
    assert!(text.contains("## HİKÂYE\n\nHenüz hikâye yok.\n"), "{text}");
    for gone in ["## Seni bekleyenler", "## Defter", "lease"] {
        assert!(!text.contains(gone), "{gone}:\n{text}");
    }
}

#[test]
fn an_exports_file_is_named_after_what_and_when_and_never_written_over() {
    let digits: String = stamp(42).chars().filter(char::is_ascii_digit).collect();
    assert_eq!(digits.len(), 12);
    assert_eq!(
        export::file_name("card-12", 42),
        format!("imece-card-12-{}-{}.md", &digits[..8], &digits[8..])
    );
    let dir = scratch("export-write");
    // No target: in the directory given.
    let path = export::write(None, &dir, "a.md", "one").unwrap();
    assert_eq!(path, dir.join("a.md"));
    assert_eq!(std::fs::read_to_string(&path).unwrap(), "one");
    let e = export::write(None, &dir, "a.md", "two").unwrap_err();
    assert_eq!(e.kind(), std::io::ErrorKind::AlreadyExists);
    assert_eq!(std::fs::read_to_string(&path).unwrap(), "one", "kept");
    // A directory: named in it; a file: that file.
    let sub = dir.join("sub");
    std::fs::create_dir(&sub).unwrap();
    let path = export::write(Some(&sub), &dir, "b.md", "three").unwrap();
    assert_eq!(path, sub.join("b.md"));
    let named = dir.join("mine.md");
    let path = export::write(Some(&named), &dir, "c.md", "four").unwrap();
    assert_eq!(path, named);
    assert_eq!(std::fs::read_to_string(&named).unwrap(), "four");
    assert!(!dir.join("c.md").exists());
    std::fs::remove_dir_all(&dir).unwrap();
}

#[test]
fn colon_export_writes_what_is_open_and_says_where() {
    let dir = scratch("export-command");
    let typed = |app: &mut App, line: &str| {
        for c in line.chars() {
            key(app, KeyCode::Char(c));
        }
    };
    let mut app = talking();
    typed(&mut app, ":export");
    assert_eq!(app.menu, None, "whole already");
    key(&mut app, KeyCode::Enter);
    let s = render(&app, 160, 40);
    assert!(
        s.contains("nothing to export: open a conversation (v)"),
        "{s}"
    );
    // The conversation open: the menu says so, the file has it.
    app.open_thread(4);
    typed(&mut app, ":exp");
    assert_eq!(app.menu.as_ref().unwrap().notes, ["(the conversation)"]);
    let file = dir.join("talk.md");
    typed(&mut app, &format!("ort {}", file.display()));
    key(&mut app, KeyCode::Enter);
    let text = std::fs::read_to_string(&file).unwrap();
    assert!(text.starts_with("# Conversation: "), "{text}");
    let s = render(&app, 200, 40);
    assert!(s.contains(&format!("written to {}", file.display())), "{s}");
    // Again: not written over, and said.
    typed(&mut app, &format!(":export {}", file.display()));
    key(&mut app, KeyCode::Enter);
    let s = render(&app, 200, 40);
    assert!(
        s.contains(&format!("could not write {}: ", file.display())),
        "{s}"
    );
    // A card's screen, into a directory.
    let mut app = on_card_one(Lang::En);
    typed(&mut app, ":exp");
    assert_eq!(app.menu.as_ref().unwrap().notes, ["(#1)"]);
    typed(&mut app, &format!("ort {}", dir.display()));
    key(&mut app, KeyCode::Enter);
    let name = export::file_name("card-1", app.world.tick);
    let text = std::fs::read_to_string(dir.join(&name)).unwrap();
    assert!(text.starts_with("# #1 "), "{text}");
    let mut app = app_after(1, 1, 0, Lang::En);
    typed(&mut app, ":exp");
    assert_eq!(app.menu.as_ref().unwrap().notes, [""], "nothing open");
    std::fs::remove_dir_all(&dir).unwrap();
}

#[test]
fn a_card_s_screen_speaks_turkish_and_says_when_there_is_no_story() {
    let mut app = on_card_one(Lang::Tr);
    app.world.cards[0].story.clear();
    let s = render(&app, 140, 36);
    for want in [
        "KİM",
        "HİKÂYE",
        "Henüz hikâye yok.",
        "sahibi    ",
        "sonraki / önceki iş",
    ] {
        assert!(s.contains(want), "{want} missing:\n{s}");
    }
}

#[test]
fn the_salon_offers_cards_and_says_what_is_typed_after_a_hash() {
    let mut app = app_after(1, 4, 140, Lang::En);
    assert!(render(&app, 140, 36).contains(" home · "));
    app.on_key(KeyEvent::from(KeyCode::Char('#')));
    let s = render(&app, 140, 36);
    assert!(s.contains(" › card: #"), "{s}");
    assert!(s.contains("#5 "), "the menu offers the cards:\n{s}");
}

#[test]
fn a_card_s_number_is_no_markdown_heading() {
    assert_eq!(heading("# Head"), Some("Head"));
    assert_eq!(heading("###### six"), Some("six"));
    assert_eq!(heading("##\t tab"), Some("tab"));
    assert_eq!(heading("#"), Some(""));
    assert_eq!(heading("####### seven"), None);
    assert_eq!(heading("#323 Shipping"), None);
    assert_eq!(heading("x # y"), None);
    assert_eq!(
        plain("#5 Money rounding\n## Done"),
        "#5 Money rounding Done"
    );
    let md = |text: &str| -> Vec<String> {
        markdown(text, "")
            .iter()
            .map(|l| l.spans.iter().map(|s| s.content.to_string()).collect())
            .collect()
    };
    assert_eq!(md("#5 Money\n# Head "), ["#5 Money", "Head"]);
    assert_eq!(markdown("# Head", "")[0].spans[1].style, bold(pal().strong));
}

#[test]
fn a_card_s_screen_says_the_checks_it_asks_for() {
    let mut app = on_card_one(Lang::En);
    app.world.cards[0].checks = Vec::new();
    assert!(!render(&app, 140, 38).contains("checks "));
    app.world.cards[0].checks = vec!["api".into(), "e2e".into()];
    let s = render(&app, 140, 38);
    assert!(s.contains("checks api, e2e"), "{s}");
    app.lang = Lang::Tr;
    assert!(render(&app, 140, 38).contains("denetim api, e2e"));
}

#[test]
fn a_card_s_screen_says_what_it_waits_on_done_failed_or_not_yet() {
    let mut app = on_card_one(Lang::En);
    let n = app.world.cards.len();
    assert!(n >= 4, "{n}");
    app.world.cards[1].column = Column::Done;
    app.world.cards[2].column = Column::Failed;
    app.world.cards[3].column = Column::Working;
    app.world.cards[0].after = vec![1, 2, 3];
    let s = render(&app, 140, 38);
    assert!(s.contains("after  #2 ✓, #3 ✕, #4 ⧗"), "{s}");
    let t = Lang::En.text();
    let spans = card::after_spans(t, &app.world, &app.world.cards[0]);
    let styles: Vec<Style> = spans.iter().map(|s| s.style).collect();
    assert_eq!(
        styles,
        [
            dim(),
            Style::default(),
            fg(pal().ok),
            dim(),
            Style::default(),
            fg(pal().bad),
            dim(),
            Style::default(),
            dim()
        ]
    );
    app.world.cards[0].after.clear();
    assert!(!render(&app, 140, 38).contains("after  "));
}
