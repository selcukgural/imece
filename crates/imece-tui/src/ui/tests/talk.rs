use super::*;

#[test]
fn a_conversation_shows_its_speakers_in_their_colours_and_folds() {
    let mut app = app_after(1, 4, 50, Lang::En);
    let lead = app.world.projects[0].leader;
    let worker = app
        .world
        .agents
        .iter()
        .find(|a| a.role == Role::Worker)
        .unwrap()
        .id;
    for (from, to, text) in [
        (lead, worker, "take the parser"),
        (worker, lead, "on it"),
        (lead, worker, "newest-line"),
    ] {
        app.world.note(Event::Sent {
            from,
            to,
            kind: MsgKind::Inform,
            text: text.into(),
            id: 0,
            re: None,
        });
    }
    app.open_window(Window::Journal);
    let mut t = Terminal::new(TestBackend::new(140, 40)).unwrap();
    t.draw(|f| draw(f, &app)).unwrap();
    let buf = t.backend().buffer();
    let row = (0..buf.area.height)
        .find(|&y| {
            (0..buf.area.width)
                .map(|x| buf[(x, y)].symbol())
                .collect::<String>()
                .contains("newest-line")
        })
        .expect("the newest line is in the journal");
    let text: String = (0..buf.area.width)
        .map(|x| buf[(x, row)].symbol())
        .collect();
    assert!(
        text.contains("+2"),
        "the older two lines fold under it: {text}"
    );
    let full = render(&app, 140, 40);
    assert!(!full.contains("on it"), "folded lines stay folded:\n{full}");
    let name = &app.world.agents[lead].name;
    let at = text.find(name.as_str()).unwrap();
    let x = u16::try_from(text[..at].chars().count()).unwrap();
    assert_eq!(buf[(x, row)].fg, speaker_color(lead), "{text}");
}

#[test]
fn a_long_answer_wraps_and_its_end_stays_in_view() {
    let mut app = app_after(1, 4, 50, Lang::En);
    let id = app
        .world
        .agents
        .iter()
        .find(|a| a.card.is_some())
        .unwrap()
        .id;
    let card = app.world.agents[id].card;
    let long = format!("{} THE-END", "word ".repeat(400));
    for _ in 0..30 {
        app.world.story(card, StoryEvent::Decision("filler".into()));
    }
    app.world.story(card, StoryEvent::Replied(long.clone()));
    app.zoom = Zoom::Focus(id);
    assert!(render(&app, 100, 30).contains("THE-END"));
    app.world.agents[id].trace(0, long);
    app.zoom = Zoom::Trace(id);
    assert!(render(&app, 100, 30).contains("THE-END"));
}

#[test]
fn c_copies_a_line_and_capital_c_the_conversation_as_shown() {
    let mut app = talking();
    app.world.note(Event::Replied {
        agent: 0,
        text: "token sk-abc123 \u{1b}[31mred".into(),
    });
    app.open_window(Window::Journal);
    app.journal_sel = Some(5);
    key(&mut app, KeyCode::Char('c'));
    let one = app.copy.take().unwrap();
    assert!(
        !one.contains("sk-abc123") && !one.contains('\u{1b}'),
        "{one}"
    );
    key(&mut app, KeyCode::Enter);
    key(&mut app, KeyCode::Char('C'));
    let all = app.copy.take().unwrap();
    assert!(
        all.contains("1970-01-01 14:00 you → lead\ncheck the readme"),
        "dated: the text leaves the screen\n{all}"
    );
    assert!(all.contains("lead → you\n# Readme"), "copied as written");
    assert!(!all.contains("between workers"));
}

#[test]
fn the_conversation_dates_the_messages_of_other_days() {
    let mut app = talking_over_two_days();
    app.open_thread(4);
    let th = app.thread.as_ref().unwrap();
    let (rows, ranges) = overlay::thread_rows(app.lang, &app.world, th, 60);
    let lines = text(&rows);
    assert_eq!(
        lines[0], "  ─ 1970-01-01",
        "the first message is not of today"
    );
    assert_eq!(ranges[0].0, 1, "the date is no part of the message");
    assert!(lines[1].starts_with("  14:00 you → lead"), "{}", lines[1]);
    let (second_day, _) = ranges[2];
    assert_eq!(lines[second_day - 1], "  ─ 1970-01-02");
    assert!(
        lines[second_day].starts_with("  00:00 you → lead"),
        "{}",
        lines[second_day]
    );
    assert_eq!(
        lines.iter().filter(|l| l.starts_with("  ─ ")).count(),
        2,
        "two days, two rows"
    );
    let screen = render(&app, 120, 30);
    assert!(screen.contains("─ 1970-01-02"), "{screen}");
}

/// A conversation between the lead (0) and agent 1 on the bus: an
/// ask, a progress, the answer to the ask and an answer to a message
/// the journal no longer has, numbered 5 on.
fn replies() -> App {
    let mut app = app_after(1, 4, 0, Lang::En);
    app.world.journal.clear();
    app.world.messages = 4;
    let lines = [
        (0, 1, MsgKind::Ask, "Which parser?\nThe old one?", None),
        (1, 0, MsgKind::Progress, "Half way.", None),
        (1, 0, MsgKind::Answer, "The new one.", Some(5)),
        (1, 0, MsgKind::Answer, "Late.", Some(2)),
    ];
    for (from, to, kind, text, re) in lines {
        let id = app.world.number();
        app.world.note(Event::Sent {
            from,
            to,
            kind,
            text: text.into(),
            id,
            re,
        });
    }
    app.open_thread(0);
    assert_eq!(app.thread.as_ref().unwrap().entries, [0, 1, 2, 3]);
    app
}

#[test]
fn a_table_in_a_conversation_wraps_to_its_width_and_copy_mode_moves_over_what_is_drawn() {
    let mut app = app_after(1, 4, 0, Lang::En);
    app.world.journal.clear();
    let id = app.world.number();
    let wide = "| # | What |\n|---|---|\n| 1 | a cell of many words that cannot fit in the window at all |";
    app.world.note(Event::Sent {
        from: 1,
        to: 0,
        kind: MsgKind::Inform,
        text: wide.into(),
        id,
        re: None,
    });
    app.open_thread(0);
    assert_eq!(
        app.thread.as_ref().unwrap().room.get(),
        usize::MAX,
        "not drawn yet"
    );
    // On a narrow screen the window is as wide as it (record 131):
    // 56, its border and gutter take 4.
    let s = render(&app, 56, 30);
    let th = app.thread.as_ref().unwrap();
    assert_eq!(th.room.get(), 52);
    assert!(
        s.contains("▌   1 │ a cell of many words that cannot fit in the   │"),
        "{s}"
    );
    assert!(s.contains("▌     │ window at all     "), "{s}");
    // Copy mode's lines are the drawn ones: one each.
    let lines = thread_lines(Lang::En, &app.world, &th.entries, th.room.get());
    let (_, _, drawn) = overlay::thread_layout(Lang::En, &app.world, &[], th, 54);
    assert_eq!(lines.len(), drawn.len());
    assert!(drawn.iter().all(|&(a, b)| b == a + 1), "{drawn:?}");
    assert_eq!(lines.len(), 5, "the head, the header, the rule, two");
}

#[test]
fn a_conversation_s_copy_lines_are_heads_reply_lines_and_what_was_written() {
    let app = replies();
    let worker = app.world.agents[1].name.clone();
    let th = app.thread.as_ref().unwrap();
    let lines = thread_lines(Lang::En, &app.world, &th.entries, usize::MAX);
    let entries: Vec<usize> = lines.iter().map(|l| l.entry).collect();
    assert_eq!(entries, [0, 0, 0, 1, 1, 2, 2, 2, 3, 3, 3]);
    let head = &lines[3].text;
    assert!(
        head.ends_with(&format!("{worker} → lead [progress]")),
        "{head}"
    );
    assert_eq!(head.trim(), head, "no blanks around");
    assert_eq!(lines[6].text, "↩ re 5: Which parser?");
    assert_eq!(lines[7].text, "The new one.");
    assert_eq!(
        overlay::copy_lines(&lines, 2, 4),
        format!("The old one?\n\n{head}\nHalf way.")
    );
    assert_eq!(
        overlay::copy_lines(&lines, 1, 2),
        "Which parser?\nThe old one?"
    );
    assert_eq!(overlay::copy_lines(&lines, 7, 7), "The new one.");
    let tr = thread_lines(Lang::Tr, &app.world, &th.entries, usize::MAX);
    assert_eq!(tr[6].text, "↩ 5 yanıtı: Which parser?");
}

#[test]
fn lines_chosen_to_copy_carry_the_mark_and_the_cursor_an_arrow() {
    let mut app = replies();
    let th = app.thread.as_mut().unwrap();
    th.sel = 0;
    th.copy = Some(crate::app::Copying { anchor: 1, at: 3 });
    let th = app.thread.as_ref().unwrap();
    let (rows, ranges) = overlay::thread_rows(Lang::En, &app.world, th, 80);
    let bar = |row: usize| rows[row].spans[0].content.to_string();
    let (first, second) = (ranges[0].0, ranges[1].0);
    assert_eq!(bar(first), " ", "the selected message's head is not chosen");
    assert_eq!(rows[first].style, Style::new());
    assert_eq!([bar(first + 1), bar(first + 2)], ["▌", "▌"]);
    assert_eq!(rows[first + 1].style, changed());
    assert_eq!(bar(second), "▶", "the cursor");
    assert_eq!(rows[second].style, changed());
    assert_eq!(bar(second + 1), " ");
}

#[test]
fn choosing_lines_of_a_conversation_titles_it_and_keeps_the_cursor_in_view() {
    let mut app = app_after(1, 4, 0, Lang::En);
    let long: Vec<String> = (0..40).map(|i| format!("row{i}")).collect();
    app.world.said(1, &long.join("\n"));
    app.open_thread(app.world.journal.len() - 1);
    let s = render(&app, 124, 20);
    assert!(s.contains("choose") && !s.contains("row37"), "{s}");
    let th = app.thread.as_mut().unwrap();
    th.copy = Some(crate::app::Copying { anchor: 38, at: 38 });
    th.follow.set(true);
    let s = render(&app, 124, 20);
    assert!(s.contains("COPY · 1 chosen") && s.contains("row37"), "{s}");
    assert!(s.contains("leave") && s.contains("w b h l  word"), "{s}");
    assert!(s.contains("s  start here") && s.contains("c  note"), "{s}");
    assert!(!s.contains("send notes"), "{s}");
    app.lang = Lang::Tr;
    let s = render(&app, 124, 20);
    assert!(s.contains("KOPYALA · 1 seçili") && s.contains("çık"), "{s}");
}

#[test]
fn choosing_lines_of_a_change_titles_it_with_the_keys() {
    let mut app = app_after(1, 4, 50, Lang::En);
    app.world.inbox.clear();
    app.world.change_ready(a_change(1));
    app.zoom = Zoom::Diff(1);
    let patch = crate::app::patch_key(&app.world.change_of(1).unwrap().patch);
    app.diff_copy = Some((1, patch, crate::app::Copying { anchor: 0, at: 2 }));
    let s = render(&app, 160, 40);
    assert!(
        s.contains("COPY · 3 chosen · y as written · Y the new side"),
        "{s}"
    );
    assert!(!s.contains("CHANGE ·"), "{s}");
    assert!(
        s.contains(" y  copy   v esc  leave   Y  copy the new side   ?  help"),
        "{s}"
    );
    assert!(
        !s.contains("note · send notes"),
        "the change's keys rest: {s}"
    );
    app.diff_copy = None;
    let s = render(&app, 160, 40);
    assert!(
        !s.contains("COPY ·") && s.contains(" c  note   esc  back"),
        "{s}"
    );
    app.lang = Lang::Tr;
    app.diff_copy = Some((1, patch, crate::app::Copying { anchor: 0, at: 2 }));
    let s = render(&app, 160, 40);
    assert!(
        s.contains("KOPYALA · 3 seçili · y yazıldığı gibi") && s.contains("yeni hali kopyala"),
        "{s}"
    );
}

#[test]
fn markdown_keeps_each_drawn_line_s_source_for_copying() {
    let md = "# Head\n```rust\nlet x = 1;\n```\n| a | b |\n|---|---|\n| 1 | 2 |\n- item **b**";
    let src: Vec<String> = markdown_src(md, "  ", usize::MAX)
        .into_iter()
        .map(|(_, s)| s)
        .collect();
    assert_eq!(
        src,
        [
            "# Head",
            "let x = 1;",
            "| a | b |",
            "|---|---|",
            "| 1 | 2 |",
            "- item **b**"
        ]
    );
    assert_eq!(
        markdown_src(md, "  ", usize::MAX).len(),
        markdown(md, "  ").len()
    );
}

#[test]
fn a_reply_shows_what_it_answers_and_percent_goes_there_and_back() {
    let mut app = replies();
    let worker = app.world.agents[1].name.clone();
    let th = app.thread.as_ref().unwrap();
    let (rows, ranges) = overlay::thread_rows(Lang::En, &app.world, th, 80);
    let lines = text(&rows);
    let head = |n: usize| lines[ranges[n].0].clone();
    assert!(
        head(2).ends_with(&format!("{worker} → lead [answer] ↩")),
        "{}",
        head(2)
    );
    assert!(
        head(0).ends_with(
            "lead → {worker} [ask]"
                .replace("{worker}", &worker)
                .as_str()
        )
    );
    assert_eq!(lines[ranges[2].0 + 1], "    ↩ re 5: Which parser?");
    assert_eq!(lines[ranges[2].0 + 2], "    The new one.");
    assert_eq!(
        lines[ranges[3].0 + 1],
        "    ↩ re 2, which the journal no longer has"
    );
    assert_eq!(
        lines[ranges[1].0 + 1],
        "    Half way.",
        "no reply, no ↩ row"
    );
    let th = app.thread.as_ref().unwrap();
    let tr = text(&overlay::thread_rows(Lang::Tr, &app.world, th, 80).0);
    assert_eq!(tr[ranges[2].0 + 1], "    ↩ 5 yanıtı: Which parser?");
    assert_eq!(tr[ranges[3].0 + 1], "    ↩ 2 yanıtı; günlükte artık yok");
    // % from the answer to the ask, and from the ask to its answer.
    let press = |app: &mut App, from: usize| {
        let th = app.thread.as_mut().unwrap();
        th.sel = from;
        th.follow.set(false);
        key(app, KeyCode::Char('%'));
        let th = app.thread.as_ref().unwrap();
        (th.sel, th.follow.get())
    };
    assert_eq!(press(&mut app, 2), (0, true));
    assert_eq!(press(&mut app, 0), (2, true));
    assert_eq!(press(&mut app, 1), (1, false), "neither: it stays");
    assert_eq!(press(&mut app, 3), (3, false), "what it answers is gone");
    assert!(app.thread.is_some(), "% does not close it");
}

#[test]
fn the_conversation_marks_every_row_of_the_selected_message() {
    let mut app = talking();
    app.open_thread(4);
    let th = app.thread.as_ref().unwrap();
    let (rows, ranges) = overlay::thread_rows(app.lang, &app.world, th, 60);
    assert_eq!(ranges.len(), 4);
    let (start, end) = ranges[3];
    assert!(end - start > 10, "the long message wraps into many rows");
    assert_eq!(end, rows.len() - 1, "a blank row closes it");
    let marked = |n: usize| rows[n].spans.first().is_some_and(|s| s.content == "▌");
    assert!((start..end).all(marked), "every row of it carries the mark");
    assert!(
        (start..end).all(|n| rows[n].style == changed()),
        "and the selection background"
    );
    assert!((0..start).all(|n| !marked(n) && rows[n].style == Style::new()));
    assert!(
        rows.iter().all(|r| r.width() <= 60),
        "{:?}",
        rows.iter().map(Line::width).max()
    );
    // The head: when, who to whom; the text under it, indented.
    let lines = text(&rows);
    assert!(
        lines[start].starts_with("▌ 14:00 lead → you"),
        "{}",
        lines[start]
    );
    assert!(
        lines[start + 1].starts_with("▌   long long"),
        "{}",
        lines[start + 1]
    );
    // The person's message, not selected: a plain gutter.
    assert!(lines[0].starts_with("  14:00 you → lead"), "{}", lines[0]);
    assert_eq!(lines[1], "    check the readme");
    // Title and hints on the screen, in both languages.
    let screen = render(&app, 120, 30);
    assert!(
        screen.contains("CONVERSATION · lead ⇄ you · 4/4"),
        "{screen}"
    );
    assert!(
        screen.contains(" c C  copy   v  choose   n  send notes   esc  close"),
        "{screen}"
    );
    assert!(
        screen.contains("message   [ ]  turn   %  reply   h l zM zR  fold   c C"),
        "{screen}"
    );
    app.lang = Lang::Tr;
    let screen = render(&app, 120, 30);
    assert!(screen.contains("KONUŞMA · lead ⇄ sen · 4/4"), "{screen}");
    assert!(
        screen.contains(" c C  kopyala   v  seç   n  notları gönder   esc  kapat"),
        "{screen}"
    );
    assert!(
        screen.contains("mesaj   [ ]  sıra   %  yanıt   h l zM zR  katla   c C"),
        "{screen}"
    );
}

/// A row's characters, each with its style (the row's own under it).
fn styled(row: &Line) -> Vec<(char, Style)> {
    (row.spans.iter())
        .flat_map(|s| {
            s.content
                .chars()
                .map(move |c| (c, row.style.patch(s.style)))
        })
        .collect()
}

/// The styles of the characters of `word` where it first stands in `row`.
fn word_styles(row: &Line, word: &str) -> Vec<Style> {
    let chars = styled(row);
    let text: Vec<char> = chars.iter().map(|&(c, _)| c).collect();
    let want: Vec<char> = word.chars().collect();
    let at = (0..text.len())
        .find(|&i| text[i..].starts_with(&want))
        .unwrap_or_else(|| panic!("{word} in {text:?}"));
    chars[at..at + want.len()].iter().map(|&(_, s)| s).collect()
}

#[test]
fn a_drawn_line_s_words_skip_what_only_leads_it() {
    let line = |spans: Vec<Span<'static>>| Line::from(spans);
    let words = |l: &Line| overlay::line_words(l);
    let plain = line(vec![
        Span::raw("  "),
        Span::raw("• "),
        Span::raw("alpha "),
        Span::styled("be", bold(pal().strong)),
        Span::raw("ta  gamma "),
    ]);
    assert_eq!(words(&plain), ["alpha", "beta", "gamma"]);
    let code = line(vec![Span::raw("  "), Span::raw("│ "), Span::raw("x = 1")]);
    assert_eq!(words(&code), ["x", "=", "1"]);
    assert!(words(&line(vec![Span::raw("  ")])).is_empty());
    let head = line(vec![Span::raw("14:00 "), Span::raw("lead")]);
    assert_eq!(words(&head), ["14:00", "lead"], "a clock is a word");
}

#[test]
fn painting_words_chooses_them_and_the_blanks_between_them() {
    let base = Style::new().fg(Color::Red);
    let line = Line::from(vec![
        Span::raw("  "),
        Span::styled("alpha beta ", base),
        Span::raw("gamma delta"),
    ])
    .style(Style::new().fg(Color::Blue));
    let painted = overlay::paint_words(line.clone(), 1, 2, Some(2));
    assert_eq!(painted.style, line.style, "the line's own style stays");
    assert_eq!(painted.spans[0], Span::raw("  "), "the lead untouched");
    let text: String = painted.spans.iter().map(|s| s.content.as_ref()).collect();
    assert_eq!(text, "  alpha beta gamma delta");
    let chars: Vec<Style> = (painted.spans.iter())
        .flat_map(|s| s.content.chars().map(move |_| s.style))
        .collect();
    let chosen = base.patch(changed());
    let cursor = Style::new()
        .patch(changed())
        .add_modifier(Modifier::REVERSED);
    // "  alpha beta gamma delta"
    //  01234567890123456789012
    assert_eq!(&chars[2..7], [base; 5], "alpha: before the choice");
    assert_eq!(chars[7], base, "the blank before the first chosen word");
    assert_eq!(&chars[8..12], [chosen; 4], "beta");
    assert_eq!(chars[12], chosen, "the blank between two chosen words");
    assert_eq!(&chars[13..18], [cursor; 5], "gamma, the cursor's");
    assert_eq!(chars[18], Style::new(), "the blank after the last");
    assert_eq!(&chars[19..], [Style::new(); 5], "delta: after it");
    // The last word chosen, the cursor not on it: chosen all the same.
    let painted = overlay::paint_words(line.clone(), 1, 2, None);
    let chars: Vec<Style> = (painted.spans.iter())
        .flat_map(|s| s.content.chars().map(move |_| s.style))
        .collect();
    assert_eq!(&chars[13..18], [changed(); 5], "gamma");
    assert_eq!(chars[18], Style::new());
    // No cursor: nothing reversed; a choice past the end ends there.
    let painted = overlay::paint_words(line.clone(), 3, usize::MAX, None);
    let last = painted.spans.last().unwrap();
    assert_eq!(last.content, "delta");
    assert_eq!(last.style, changed());
    assert!((painted.spans.iter()).all(|s| !s.style.add_modifier.contains(Modifier::REVERSED)));
    // Spans of one style stay one span.
    let painted = overlay::paint_words(line, 5, 5, None);
    assert_eq!(painted.spans.len(), 3, "{:?}", painted.spans);
}

#[test]
fn the_word_cursor_steps_over_line_ends_and_stops_at_the_edges() {
    let counts = [2, 0, 3];
    let on = |at| overlay::word_step(&counts, at, true);
    let back = |at| overlay::word_step(&counts, at, false);
    assert_eq!(on((0, 0)), (0, 1));
    assert_eq!(on((0, 1)), (1, 0), "past the last word, the next line");
    assert_eq!(on((1, 0)), (2, 0), "an empty line is passed too");
    assert_eq!(on((2, 2)), (2, 2), "the last word of the last line");
    assert_eq!(on((0, 9)), (1, 0), "a word past the end is the last");
    assert_eq!(back((2, 2)), (2, 1));
    assert_eq!(back((2, 0)), (1, 0), "before the first, the line above");
    assert_eq!(back((1, 0)), (0, 1), "to its last word");
    assert_eq!(back((0, 0)), (0, 0), "the first word of the first line");
    assert_eq!(back((2, 9)), (2, 1));
    assert_eq!(overlay::word_step(&[], (0, 0), true), (0, 0));
}

/// The person and the lead: "assign it" (one line), "a\nb" (two
/// short ones), "" (none), the lead's long line, the lead's readme.
fn folding() -> App {
    let mut app = app_after(1, 4, 0, Lang::En);
    app.world.journal.clear();
    let you = |text: &str| Event::YouSaid {
        to: 0,
        text: text.into(),
    };
    app.world.note(you("assign it"));
    app.world.note(you("a\nb"));
    app.world.note(you(""));
    app.world.note(Event::Replied {
        agent: 0,
        text: format!("{} THE-END", "long ".repeat(40)),
    });
    app.world.note(Event::Replied {
        agent: 0,
        text: "# Readme\nIt is one line.\n- no usage".into(),
    });
    app.open_thread(0);
    app
}

#[test]
fn a_folded_message_is_one_row_that_says_more_is_there() {
    let mut app = folding();
    let th = app.thread.as_mut().unwrap();
    th.folds = crate::app::Folds::every(true);
    th.sel = 3;
    let (rows, ranges) = overlay::thread_rows(Lang::En, &app.world, th, 60);
    assert_eq!(ranges, [(0, 1), (2, 3), (4, 5), (6, 7), (8, 9)]);
    let lines = text(&rows);
    assert_eq!(
        lines[0], "  14:00 you → lead: assign it",
        "nothing more to see"
    );
    assert_eq!(lines[2], "  14:00 you → lead: a b ▸ 2 lines", "two lines");
    assert_eq!(lines[4], "  14:00 you → lead: ");
    assert!(lines[6].ends_with(" long long… ▸ 1 line"), "{}", lines[6]);
    assert!(rows[6].width() >= 59, "cut to fit, not shorter");
    assert_eq!(
        lines[8],
        "  14:00 lead → you: Readme It is one line. no usa… ▸ 3 lines"
    );
    assert!(rows.iter().all(|r| r.width() <= 60));
    // The selected one carries the mark and the background.
    assert_eq!(rows[6].spans[0].content, "▌");
    assert_eq!(rows[6].style, changed());
    assert!([0, 2, 4, 8].iter().all(|&n| rows[n].style == Style::new()));
    // One message opened; the rest stay folded.
    th.folds.set(4, false);
    let (_, ranges) = overlay::thread_rows(Lang::En, &app.world, th, 60);
    assert_eq!(ranges[4], (8, 12));
    // While choosing, every message is open.
    th.copy = Some(crate::app::Copying::new(0));
    let (_, ranges) = overlay::thread_rows(Lang::En, &app.world, th, 60);
    assert_eq!(ranges[0], (0, 2));
    // In Turkish.
    th.copy = None;
    let tr = text(&overlay::thread_rows(Lang::Tr, &app.world, th, 60).0);
    assert!(tr[2].ends_with("▸ 2 satır") && tr[6].ends_with("▸ 1 satır"));
}

#[test]
fn a_folded_line_says_more_only_when_it_was_cut() {
    let mut app = folding();
    let th = app.thread.as_mut().unwrap();
    th.folds = crate::app::Folds::every(true);
    th.sel = 1;
    // "  14:00 you → lead: assign it" is 29 wide.
    let row =
        |w: u16, n: usize| text(&overlay::thread_rows(Lang::En, &app.world, th, w).0)[n].clone();
    assert_eq!(row(29, 0), "  14:00 you → lead: assign it");
    assert_eq!(row(28, 0), "  14:00 you → lead… ▸ 1 line");
    // Nothing to open: cut, but no lines to count.
    assert_eq!(row(12, 4), "  14:00 you…");
    // Its notes' count takes room too: 33 wide with it.
    let notes = [crate::app::TalkNote {
        to: 0,
        entry: 0,
        quote: "assign".into(),
        text: "now".into(),
    }];
    let row = |w: u16| {
        let th = app.thread.as_ref().unwrap();
        text(&overlay::thread_layout(Lang::En, &app.world, &notes, th, w).0)[0].clone()
    };
    assert_eq!(row(33), "  14:00 you → lead: assign it ✎ 1");
    assert_eq!(row(32), "  14:00 you → lead… ▸ 1 line ✎ 1");
}

#[test]
fn notes_show_under_their_message_and_count_on_a_folded_one() {
    let mut app = folding();
    let note = |quote: &str, text: &str| crate::app::TalkNote {
        to: 0,
        entry: 4,
        quote: quote.into(),
        text: text.into(),
    };
    app.talk_notes = vec![
        note("It is\none", "why one?"),
        note("no usage", "\u{1b}[2Jadd it"),
    ];
    let th = app.thread.as_mut().unwrap();
    let (rows, _, _) = overlay::thread_layout(Lang::En, &app.world, &app.talk_notes, th, 60);
    let lines = text(&rows);
    let at = lines.iter().position(|l| l.contains('✎')).unwrap();
    assert_eq!(lines[at], "      ✎ \"It is one\" why one?");
    assert!(lines[at + 1].starts_with("      ✎ \"no usage\" "));
    assert!(lines[at + 1].ends_with("[2Jadd it") && !lines[at + 1].contains('\u{1b}'));
    assert_eq!(lines[at + 2], "", "the message ends after them");
    assert!(lines[at - 1].contains("no usage"), "under the message");
    let (narrow, _, _) = overlay::thread_layout(Lang::En, &app.world, &app.talk_notes, th, 16);
    assert!(text(&narrow).iter().any(|l| l == "      ✎ \"It is…"));
    // Folded: counted at the end of its row.
    th.folds = crate::app::Folds::every(true);
    let (rows, _, _) = overlay::thread_layout(Lang::En, &app.world, &app.talk_notes, th, 90);
    let lines = text(&rows);
    assert!(lines[8].ends_with("no usage ▸ 3 lines ✎ 2"), "{}", lines[8]);
    assert!(!lines[0].contains('✎'));
    // A message folded with nothing more than its notes: counted alone.
    app.talk_notes = vec![crate::app::TalkNote {
        entry: 0,
        ..note("assign", "now")
    }];
    let th = app.thread.as_ref().unwrap();
    let (rows, _, _) = overlay::thread_layout(Lang::En, &app.world, &app.talk_notes, th, 90);
    assert_eq!(text(&rows)[0], "▌ 14:00 you → lead: assign it ✎ 1");
}

#[test]
fn the_conversation_title_counts_its_notes_and_the_line_names_the_words() {
    let mut app = folding();
    let s = render(&app, 120, 30);
    assert!(!s.contains('✎'), "{s}");
    app.talk_notes.push(crate::app::TalkNote {
        to: 0,
        entry: 4,
        quote: "It is one line.".into(),
        text: "why?".into(),
    });
    app.talk_notes.push(crate::app::TalkNote {
        to: 0,
        entry: 99,
        quote: "elsewhere".into(),
        text: "not here".into(),
    });
    let s = render(&app, 120, 30);
    assert!(s.contains("· 1/5 ✎ 1 "), "{s}");
    app.typing = true;
    app.noting = Some(crate::app::Noting::Words(crate::app::TalkNote {
        to: 0,
        entry: 4,
        quote: "x".repeat(30),
        text: String::new(),
    }));
    let s = render(&app, 120, 30);
    let quote = format!("note on \"{}…\" ▸ ", "x".repeat(NOTE_QUOTE - 1));
    assert!(s.contains(&quote), "{s}");
    app.lang = Lang::Tr;
    let s = render(&app, 120, 30);
    assert!(s.contains("…\" için not ▸ "), "{s}");
}

#[test]
fn words_chosen_carry_the_background_and_the_cursor_word_is_reversed() {
    let mut app = folding();
    let th = app.thread.as_mut().unwrap();
    let lines = thread_lines(Lang::En, &app.world, &th.entries, usize::MAX);
    let at = lines
        .iter()
        .position(|l| l.text == "It is one line.")
        .unwrap();
    th.copy = Some(crate::app::Copying { anchor: at, at });
    th.words = Some((1, 2));
    let (rows, _, line_ranges) = overlay::thread_layout(Lang::En, &app.world, &[], th, 60);
    let row = &rows[line_ranges[at].0];
    assert_eq!(row.spans[0].content, "▶");
    assert_ne!(row.style, changed(), "part of the line: not the row");
    assert!(word_styles(row, "It").iter().all(|s| s.bg != changed().bg));
    assert!(word_styles(row, "is").iter().all(|s| s.bg == changed().bg));
    let one = word_styles(row, "one");
    assert!(
        one.iter()
            .all(|s| s.add_modifier.contains(Modifier::REVERSED))
    );
    assert!(
        word_styles(row, "line.")
            .iter()
            .all(|s| s.bg != changed().bg)
    );
    // The line chosen whole: the row's background.
    th.words = Some((0, 3));
    let (rows, _, line_ranges) = overlay::thread_layout(Lang::En, &app.world, &[], th, 60);
    assert_eq!(rows[line_ranges[at].0].style, changed());
    // A line between the ends is whole; the ones around it are not chosen.
    th.copy = Some(crate::app::Copying {
        anchor: at - 1,
        at: at + 1,
    });
    th.words = Some((1, 0));
    let (rows, _, line_ranges) = overlay::thread_layout(Lang::En, &app.world, &[], th, 60);
    assert_eq!(rows[line_ranges[at].0].style, changed());
    assert_eq!(rows[line_ranges[at].0].spans[0].content, "▌");
    assert_ne!(rows[line_ranges[at - 1].0].style, changed());
    assert_eq!(rows[line_ranges[at - 2].0].spans[0].content, " ");
}

#[test]
fn the_conversation_scrolls_only_as_far_as_the_selection_needs() {
    let mut app = talking();
    app.open_thread(4);
    render(&app, 100, 20);
    let th = app.thread.as_ref().unwrap();
    let (rows, ranges) = overlay::thread_rows(app.lang, &app.world, th, 96);
    let (start, end) = ranges[3];
    assert_eq!(
        usize::from(th.scroll.get()),
        start,
        "the long last message shows from its start"
    );
    let last_row = rows.len() - 1;
    assert!(end > 17 && last_row < 2 * 17, "{end} {last_row}");
    key(&mut app, KeyCode::Up);
    render(&app, 100, 20);
    assert_eq!(
        usize::from(app.thread.as_ref().unwrap().scroll.get()),
        ranges[2].0,
        "the message above was out of view: scrolled up to it"
    );
    key(&mut app, KeyCode::Up);
    render(&app, 100, 20);
    assert_eq!(
        usize::from(app.thread.as_ref().unwrap().scroll.get()),
        ranges[1].0,
        "the readme (6 rows) was above too"
    );
    key(&mut app, KeyCode::Down);
    render(&app, 100, 20);
    assert_eq!(
        usize::from(app.thread.as_ref().unwrap().scroll.get()),
        ranges[1].0,
        "going back down: still in view, no scroll"
    );
    // Paging stops at the last row; a page is the body less one row of
    // overlap (20 rows: 2 bars, 3 of the detail's frame, 2 border, 1
    // hints: 12 of text).
    let th = app.thread.as_ref().unwrap();
    assert_eq!(th.page.get(), 11);
    for _ in 0..9 {
        key(&mut app, KeyCode::PageDown);
    }
    render(&app, 100, 20);
    assert_eq!(
        usize::from(app.thread.as_ref().unwrap().scroll.get()),
        rows.len() - 12,
        "{} rows, 12 shown",
        rows.len()
    );
}

#[test]
fn a_conversation_takes_the_whole_body_of_a_narrow_screen() {
    let app = replies();
    let left = |width: u16| {
        let buf = buffer(&app, width, 30);
        let y = (0..30)
            .find(|&y| {
                (0..width)
                    .map(|x| buf[(x, y)].symbol())
                    .collect::<String>()
                    .contains("CONVERSATION")
            })
            .unwrap();
        let right = (0..width).rev().find(|&x| buf[(x, y)].symbol() == "┐");
        ((0..width).find(|&x| buf[(x, y)].symbol() == "┌"), right)
    };
    assert_eq!(left(79), (Some(0), Some(78)), "edge to edge");
    assert_eq!(left(80), (Some(2), Some(77)), "two columns either side");
}
